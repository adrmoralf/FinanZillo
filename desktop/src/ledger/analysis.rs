//! Resumen del mes, proyección, insights, pulso de 12 meses y vista anual.

use std::collections::HashMap;

use chrono::Datelike;
use rusqlite::params;

use super::Core;
use crate::util;

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct CatTotal {
    pub id: Option<i64>,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub total_cents: i64,
    pub count: i64,
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct BudgetLine {
    pub id: i64,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub budget_cents: i64,
    pub spent_cents: i64,
}

impl BudgetLine {
    pub fn pct(&self) -> i64 {
        util::pct(self.spent_cents, self.budget_cents)
    }
}

#[derive(Clone, Debug, Default)]
pub struct MonthTotals {
    pub month: String,
    pub ingresos_cents: i64,
    pub gastos_cents: i64,
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct Summary {
    pub month: String,
    pub prev_month: String,
    pub ingresos_cents: i64,
    pub gastos_cents: i64,
    pub count: i64,
    pub spend_days: i64,
    pub prev_ingresos_cents: i64,
    pub prev_gastos_cents: i64,
    pub by_category: Vec<CatTotal>,
    pub by_category_ingreso: Vec<CatTotal>,
    pub prev_by_category: Vec<CatTotal>,
    /// Gasto por día del mes (índice 0 = día 1).
    pub daily: Vec<i64>,
    pub prev_daily: Vec<i64>,
    pub gastos_recurrentes_cents: i64,
    pub pending_recurring_cents: i64,
    pub budgets: Vec<BudgetLine>,
    pub global_budget_cents: i64,
    pub fixed_cents: i64,
    pub is_current: bool,
    pub elapsed_days: i64,
}

impl Summary {
    pub fn balance(&self) -> i64 {
        self.ingresos_cents - self.gastos_cents
    }

    /// Gasto proyectado a fin de mes (solo mes en curso): extrapola el gasto
    /// variable y suma los recurrentes que aún no han caído.
    pub fn projected_gastos(&self) -> Option<i64> {
        if !self.is_current || self.gastos_cents == 0 {
            return None;
        }
        let days = self.daily.len() as f64;
        let elapsed = self.elapsed_days.max(1) as f64;
        let variable = (self.gastos_cents - self.gastos_recurrentes_cents).max(0) as f64;
        let remaining = (days - elapsed).max(0.0);
        Some((self.gastos_cents as f64 + variable / elapsed * remaining + self.pending_recurring_cents as f64).round() as i64)
    }

    pub fn projected_balance(&self) -> Option<i64> {
        self.projected_gastos().map(|g| self.ingresos_cents - g)
    }

    pub fn daily_avg(&self) -> i64 {
        if self.elapsed_days > 0 { self.gastos_cents / self.elapsed_days } else { 0 }
    }

    pub fn days_without_spending(&self) -> i64 {
        (self.elapsed_days - self.spend_days).max(0)
    }

    /// Gasto medio por día de la semana (lunes = 0), solo días con gasto.
    pub fn weekday_avg(&self) -> [i64; 7] {
        let first = util::first_day(&self.month);
        let mut sums = [0i64; 7];
        let mut counts = [0i64; 7];
        for (i, c) in self.daily.iter().enumerate() {
            if *c > 0 {
                let dow = (first.weekday().num_days_from_monday() as usize + i) % 7;
                sums[dow] += c;
                counts[dow] += 1;
            }
        }
        let mut out = [0i64; 7];
        for i in 0..7 {
            out[i] = if counts[i] > 0 { sums[i] / counts[i] } else { 0 };
        }
        out
    }

    /// Frases automáticas comparando con el mes anterior.
    pub fn insights(&self) -> Vec<String> {
        let mut out = Vec::new();
        let prev = util::MESES[(util::month_parts(&self.prev_month).1 - 1) as usize];
        if self.prev_gastos_cents > 0 && self.gastos_cents > 0 {
            let diff = self.gastos_cents - self.prev_gastos_cents;
            let p = ((diff as f64 / self.prev_gastos_cents as f64) * 100.0).round() as i64;
            if p.abs() >= 1 {
                out.push(format!(
                    "Gastas un {}% {} que en {prev}{}",
                    p.abs(),
                    if diff < 0 { "menos" } else { "más" },
                    if self.is_current { " (a estas alturas el mes no ha terminado)" } else { "" }
                ));
            }
        }
        let prev_by: HashMap<Option<i64>, i64> = self.prev_by_category.iter().map(|c| (c.id, c.total_cents)).collect();
        let mut biggest: Option<(&CatTotal, i64)> = None;
        for c in &self.by_category {
            let before = *prev_by.get(&c.id).unwrap_or(&0);
            let delta = c.total_cents - before;
            if before > 0 && delta > 0 && biggest.is_none_or(|(_, d)| delta > d) {
                biggest = Some((c, delta));
            }
        }
        if let Some((c, delta)) = biggest {
            let before = c.total_cents - delta;
            let p = ((delta as f64 / before as f64) * 100.0).round() as i64;
            if p >= 15 {
                out.push(format!("{} sube un {p}% respecto a {prev}", c.name));
            }
        }
        if let Some(top) = self.by_category.first() {
            if self.gastos_cents > 0 {
                out.push(format!("Tu mayor gasto es {} ({}% del total)", top.name, util::pct(top.total_cents, self.gastos_cents)));
            }
        }
        for b in &self.budgets {
            if b.spent_cents > b.budget_cents {
                out.push(format!("{} supera su presupuesto en {}", b.name, util::eur(b.spent_cents - b.budget_cents)));
            }
        }
        out
    }
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct YearView {
    pub year: i32,
    pub months: Vec<MonthTotals>,
    pub ingresos_cents: i64,
    pub gastos_cents: i64,
    pub prev_ingresos_cents: i64,
    pub prev_gastos_cents: i64,
    pub by_category: Vec<CatTotal>,
}

/// Gasto diario de un mes, para el gráfico de pulsos.
#[derive(Clone, Debug, Default)]
pub struct PulseLine {
    pub month: String,
    pub daily: Vec<i64>,
    pub total: i64,
}

impl Core {
    fn month_totals(&self, month: &str) -> (i64, i64, i64, i64) {
        self.conn
            .query_row(
                "SELECT COALESCE(SUM(CASE WHEN type = 'ingreso' THEN amount_cents END), 0),
                        COALESCE(SUM(CASE WHEN type = 'gasto' THEN amount_cents END), 0),
                        COUNT(*), COUNT(DISTINCT CASE WHEN type = 'gasto' THEN date END)
                 FROM transactions WHERE strftime('%Y-%m', date) = ?",
                [month],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap_or((0, 0, 0, 0))
    }

    fn by_category(&self, month_or_year: &str, ty: &str, year: bool) -> Vec<CatTotal> {
        let fmt = if year { "%Y" } else { "%Y-%m" };
        let sql = format!(
            "SELECT t.category_id, COALESCE(c.name, 'Sin categoría'), COALESCE(c.icon, ''), COALESCE(c.color, '#898781'),
                    SUM(t.amount_cents) AS total, COUNT(*)
             FROM transactions t LEFT JOIN categories c ON c.id = t.category_id
             WHERE t.type = ? AND strftime('{fmt}', t.date) = ?
             GROUP BY t.category_id ORDER BY total DESC"
        );
        self.conn
            .prepare(&sql)
            .and_then(|mut st| {
                st.query_map(params![ty, month_or_year], |r| {
                    Ok(CatTotal {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        icon: r.get(2)?,
                        color: r.get(3)?,
                        total_cents: r.get(4)?,
                        count: r.get(5)?,
                    })
                })
                .map(|it| it.flatten().collect())
            })
            .unwrap_or_default()
    }

    fn daily_spend(&self, month: &str) -> Vec<i64> {
        let mut v = vec![0i64; util::days_in_month(month) as usize];
        if let Ok(mut st) = self.conn.prepare(
            "SELECT CAST(strftime('%d', date) AS INTEGER), SUM(amount_cents) FROM transactions
             WHERE type = 'gasto' AND strftime('%Y-%m', date) = ? GROUP BY date",
        ) {
            if let Ok(rows) = st.query_map([month], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))) {
                for (d, c) in rows.flatten() {
                    if d >= 1 && (d as usize) <= v.len() {
                        v[d as usize - 1] += c;
                    }
                }
            }
        }
        v
    }

    pub fn summary(&self, month: &str) -> Summary {
        let prev_month = util::add_months(month, -1);
        let (ing, gas, count, spend_days) = self.month_totals(month);
        let (ping, pgas, _, _) = self.month_totals(&prev_month);
        let by_category = self.by_category(month, "gasto", false);
        let is_current = month == util::current_month();
        let today = util::today();
        let days = util::days_in_month(month) as i64;
        let elapsed = if is_current {
            today.day() as i64
        } else if month > util::current_month().as_str() {
            0
        } else {
            days
        };

        let gastos_recurrentes: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(amount_cents), 0) FROM transactions
                 WHERE type = 'gasto' AND source = 'recurrente' AND strftime('%Y-%m', date) = ?",
                [month],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let mut pending = 0;
        if is_current {
            for r in self.recurring_list() {
                let counts = r.active && ((r.kind == "movimiento" && r.ty == "gasto") || (r.kind == "aportacion" && !r.from_account.is_empty()));
                let day = r.day_of_month.min(days);
                if counts && day > elapsed && r.last_applied.as_deref().is_none_or(|l| l < month) {
                    pending += r.amount_cents;
                }
            }
        }

        let spent_by: HashMap<Option<i64>, i64> = by_category.iter().map(|c| (c.id, c.total_cents)).collect();
        let cats = self.categories(true);
        let mut budgets: Vec<BudgetLine> = cats
            .iter()
            .filter(|c| c.ty == "gasto")
            .map(|c| BudgetLine {
                id: c.id,
                name: c.name.clone(),
                icon: c.icon.clone(),
                color: c.color.clone(),
                budget_cents: self.budget_for_month(c.id, month, c.budget_cents),
                spent_cents: *spent_by.get(&Some(c.id)).unwrap_or(&0),
            })
            .filter(|b| b.budget_cents > 0)
            .collect();
        budgets.sort_by(|a, b| b.pct().cmp(&a.pct()));
        let fixed_ids: Vec<i64> = cats.iter().filter(|c| c.fixed).map(|c| c.id).collect();
        let fixed_cents = by_category.iter().filter(|c| c.id.is_some_and(|id| fixed_ids.contains(&id))).map(|c| c.total_cents).sum();

        Summary {
            month: month.to_string(),
            prev_month: prev_month.clone(),
            ingresos_cents: ing,
            gastos_cents: gas,
            count,
            spend_days,
            prev_ingresos_cents: ping,
            prev_gastos_cents: pgas,
            by_category_ingreso: self.by_category(month, "ingreso", false),
            prev_by_category: self.by_category(&prev_month, "gasto", false),
            by_category,
            daily: self.daily_spend(month),
            prev_daily: self.daily_spend(&prev_month),
            gastos_recurrentes_cents: gastos_recurrentes,
            pending_recurring_cents: pending,
            budgets,
            global_budget_cents: self.budget_for_month(0, month, self.global_budget_cents()),
            fixed_cents,
            is_current,
            elapsed_days: elapsed,
        }
    }

    /// Gasto diario de los 12 meses que terminan en `month` (el más antiguo primero).
    pub fn pulse(&self, month: &str) -> Vec<PulseLine> {
        (0..12)
            .rev()
            .map(|i| {
                let m = util::add_months(month, -i);
                let daily = self.daily_spend(&m);
                let total = daily.iter().sum();
                PulseLine { month: m, daily, total }
            })
            .collect()
    }

    /// Ingresos y gastos de los 12 meses que terminan en `month`.
    pub fn monthly_series(&self, month: &str) -> Vec<MonthTotals> {
        (0..12)
            .rev()
            .map(|i| {
                let m = util::add_months(month, -i);
                let (ing, gas, _, _) = self.month_totals(&m);
                MonthTotals { month: m, ingresos_cents: ing, gastos_cents: gas }
            })
            .collect()
    }

    /// Serie de 12 meses de una categoría.
    pub fn category_monthly(&self, id: i64, month: &str) -> Vec<MonthTotals> {
        (0..12)
            .rev()
            .map(|i| {
                let m = util::add_months(month, -i);
                let total: i64 = self
                    .conn
                    .query_row(
                        "SELECT COALESCE(SUM(amount_cents), 0) FROM transactions WHERE category_id = ? AND strftime('%Y-%m', date) = ?",
                        params![id, m],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                MonthTotals { month: m, ingresos_cents: 0, gastos_cents: total }
            })
            .collect()
    }

    pub fn year_view(&self, year: i32) -> YearView {
        let per_month = |y: i32| -> Vec<MonthTotals> {
            (1..=12)
                .map(|m| {
                    let ms = util::month_str(y, m);
                    let (ing, gas, _, _) = self.month_totals(&ms);
                    MonthTotals { month: ms, ingresos_cents: ing, gastos_cents: gas }
                })
                .collect()
        };
        let months = per_month(year);
        let prev = per_month(year - 1);
        YearView {
            year,
            ingresos_cents: months.iter().map(|m| m.ingresos_cents).sum(),
            gastos_cents: months.iter().map(|m| m.gastos_cents).sum(),
            prev_ingresos_cents: prev.iter().map(|m| m.ingresos_cents).sum(),
            prev_gastos_cents: prev.iter().map(|m| m.gastos_cents).sum(),
            by_category: self.by_category(&year.to_string(), "gasto", true),
            months,
        }
    }

}

#[cfg(test)]
mod tests {
    use super::super::{TxInput, test_core};
    use super::*;

    #[test]
    fn resumen_y_presupuesto() {
        let mut c = test_core();
        let ocio = c.resolve_category("Ocio", "gasto").unwrap();
        c.save_category(Some(ocio), "Ocio", "gasto", "🎬", "#4a3aa7", 15000, false).unwrap();
        for (amt, d) in [(10000, "2026-11-02"), (6800, "2026-11-20")] {
            c.create_tx(&TxInput { ty: "gasto".into(), amount_cents: amt, category_id: Some(ocio), date: d.into(), ..Default::default() })
                .unwrap();
        }
        c.create_tx(&TxInput { ty: "ingreso".into(), amount_cents: 160300, date: "2026-11-21".into(), ..Default::default() }).unwrap();
        let s = c.summary("2026-11");
        assert_eq!(s.gastos_cents, 16800);
        assert_eq!(s.balance(), 160300 - 16800);
        assert_eq!(s.daily[1], 10000);
        assert_eq!(s.budgets[0].pct(), 112);
        assert!(s.insights().iter().any(|i| i.contains("supera su presupuesto")));
    }
}
