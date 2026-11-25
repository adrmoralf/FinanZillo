//! Movimientos recurrentes: se materializan una vez por mes, en su día.

use rusqlite::{OptionalExtension, params};

use super::{Core, R, SqlErr, err};
use crate::util;

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct Recurring {
    pub id: i64,
    /// "movimiento" | "traspaso" | "aportacion"
    pub kind: String,
    pub ty: String,
    pub amount_cents: i64,
    pub category_id: Option<i64>,
    pub note: String,
    pub account: String,
    pub day_of_month: i64,
    pub active: bool,
    pub start_month: String,
    pub last_applied: Option<String>,
    pub from_account: String,
    pub to_account: String,
    pub investment_id: Option<i64>,
    pub category_name: String,
    pub category_icon: String,
    pub investment_name: String,
}

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub note: String,
    pub category_id: Option<i64>,
    pub category_name: String,
    pub amount_cents: i64,
    pub months_seen: i64,
    pub typical_day: i64,
}

const SELECT: &str = "SELECT r.id, r.kind, r.type, r.amount_cents, r.category_id, r.note, r.account, r.day_of_month,
       r.active, r.start_month, r.last_applied, r.from_account, r.to_account, r.investment_id,
       COALESCE(c.name, ''), COALESCE(c.icon, ''), COALESCE(i.name, '')
  FROM recurring r LEFT JOIN categories c ON c.id = r.category_id LEFT JOIN investments i ON i.id = r.investment_id";

impl Core {
    pub fn recurring_list(&self) -> Vec<Recurring> {
        self.conn
            .prepare(&format!("{SELECT} ORDER BY r.active DESC, r.day_of_month, r.id"))
            .and_then(|mut st| {
                st.query_map([], |r| {
                    Ok(Recurring {
                        id: r.get(0)?,
                        kind: r.get(1)?,
                        ty: r.get(2)?,
                        amount_cents: r.get(3)?,
                        category_id: r.get(4)?,
                        note: r.get(5)?,
                        account: r.get(6)?,
                        day_of_month: r.get(7)?,
                        active: r.get::<_, i64>(8)? != 0,
                        start_month: r.get(9)?,
                        last_applied: r.get(10)?,
                        from_account: r.get(11)?,
                        to_account: r.get(12)?,
                        investment_id: r.get(13)?,
                        category_name: r.get(14)?,
                        category_icon: r.get(15)?,
                        investment_name: r.get(16)?,
                    })
                })
                .map(|it| it.flatten().collect())
            })
            .unwrap_or_default()
    }

    fn validate_recurring(&self, r: &Recurring) -> R<Recurring> {
        let mut v = r.clone();
        if !["movimiento", "traspaso", "aportacion"].contains(&v.kind.as_str()) {
            return err("Tipo de recurrente no válido");
        }
        if v.amount_cents <= 0 {
            return err("Importe no válido");
        }
        if !(1..=31).contains(&v.day_of_month) {
            return err("El día debe estar entre 1 y 31");
        }
        v.note = v.note.trim().to_string();
        let clean = |s: &str| -> String { s.trim().chars().take(60).collect() };
        match v.kind.as_str() {
            "movimiento" => {
                if v.ty != "gasto" && v.ty != "ingreso" {
                    return err("El tipo debe ser gasto o ingreso");
                }
                v.category_id = self.valid_category_pub(v.category_id, &v.ty);
                v.account = clean(&v.account);
                if v.account.is_empty() {
                    return err("Elige la cuenta del movimiento recurrente");
                }
                v.from_account.clear();
                v.to_account.clear();
                v.investment_id = None;
            }
            "traspaso" => {
                v.ty = "gasto".into();
                v.from_account = clean(&v.from_account);
                v.to_account = clean(&v.to_account);
                if v.from_account.is_empty() || v.to_account.is_empty() {
                    return err("Faltan las cuentas de origen y destino");
                }
                if v.from_account.eq_ignore_ascii_case(&v.to_account) {
                    return err("Origen y destino no pueden ser la misma cuenta");
                }
                v.category_id = None;
                v.account.clear();
                v.investment_id = None;
            }
            _ => {
                v.ty = "gasto".into();
                let ok = self
                    .conn
                    .query_row("SELECT id FROM investments WHERE id = ? AND archived = 0", [v.investment_id.unwrap_or(0)], |r| {
                        r.get::<_, i64>(0)
                    })
                    .optional()
                    .ok()
                    .flatten();
                if ok.is_none() {
                    return err("Elige una inversión válida");
                }
                v.from_account = clean(&v.from_account);
                v.category_id = None;
                v.account.clear();
                v.to_account.clear();
            }
        }
        Ok(v)
    }

    pub fn valid_category_pub(&self, id: Option<i64>, ty: &str) -> Option<i64> {
        match id {
            Some(id) => self
                .conn
                .query_row("SELECT id FROM categories WHERE id = ? AND type = ?", params![id, ty], |r| r.get(0))
                .optional()
                .ok()
                .flatten()
                .or_else(|| self.resolve_category("", ty)),
            None => self.resolve_category("", ty),
        }
    }

    pub fn save_recurring(&mut self, id: Option<i64>, r: &Recurring) -> R<i64> {
        let v = self.validate_recurring(r)?;
        let rid = match id {
            None => {
                self.conn
                    .execute(
                        "INSERT INTO recurring (kind, type, amount_cents, category_id, note, account, day_of_month, start_month,
                                                from_account, to_account, investment_id)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        params![
                            v.kind, v.ty, v.amount_cents, v.category_id, v.note, v.account, v.day_of_month,
                            util::current_month(), v.from_account, v.to_account, v.investment_id
                        ],
                    )
                    .e()?;
                self.conn.last_insert_rowid()
            }
            Some(id) => {
                self.conn
                    .execute(
                        "UPDATE recurring SET kind = ?, type = ?, amount_cents = ?, category_id = ?, note = ?, account = ?,
                           day_of_month = ?, active = ?, from_account = ?, to_account = ?, investment_id = ? WHERE id = ?",
                        params![
                            v.kind, v.ty, v.amount_cents, v.category_id, v.note, v.account, v.day_of_month, v.active as i64,
                            v.from_account, v.to_account, v.investment_id, id
                        ],
                    )
                    .e()?;
                id
            }
        };
        self.touch();
        self.apply_pending();
        Ok(rid)
    }

    pub fn set_recurring_active(&mut self, id: i64, active: bool) -> R<()> {
        self.conn.execute("UPDATE recurring SET active = ? WHERE id = ?", params![active as i64, id]).e()?;
        self.touch();
        self.apply_pending();
        Ok(())
    }

    /// Borra la regla; los movimientos ya creados se conservan.
    pub fn delete_recurring(&mut self, id: i64) -> R<()> {
        self.conn.execute("DELETE FROM recurring WHERE id = ?", [id]).e()?;
        self.touch();
        Ok(())
    }

    /// Precio EUR/ud sin salir a internet: caché → manual → coste medio.
    pub fn cached_price_eur(&self, inv_id: i64) -> Option<f64> {
        let (provider, symbol, units, cost, manual): (String, String, f64, i64, Option<f64>) = self
            .conn
            .query_row("SELECT provider, symbol, units, cost_cents, manual_price FROM investments WHERE id = ?", [inv_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .ok()?;
        if provider == "manual" {
            return manual;
        }
        let key = format!("{provider}:{}", symbol.to_lowercase());
        if let Some(p) = self.conn.query_row("SELECT price FROM price_cache WHERE key = ?", [key], |r| r.get(0)).optional().ok().flatten() {
            return Some(p);
        }
        if units > 0.0 && cost > 0 { Some(cost as f64 / 100.0 / units) } else { None }
    }

    fn invest_category(&self) -> Option<i64> {
        let q = |n: &str| -> Option<i64> {
            self.conn
                .query_row("SELECT id FROM categories WHERE name = ? COLLATE NOCASE AND type = 'gasto'", [n], |r| r.get(0))
                .optional()
                .ok()
                .flatten()
        };
        q("Inversiones").or_else(|| q("Otros"))
    }

    /// Materializa recurrentes pendientes y luego intereses de meses cerrados.
    pub fn apply_pending(&mut self) -> usize {
        let a = self.apply_recurring();
        let b = self.apply_interest();
        a + b
    }

    fn apply_recurring(&mut self) -> usize {
        let rules: Vec<Recurring> = self.recurring_list().into_iter().filter(|r| r.active).collect();
        if rules.is_empty() {
            return 0;
        }
        let today = util::today();
        let now_month = util::current_month();
        let today_day = today.format("%d").to_string().parse::<i64>().unwrap_or(1);
        let mut created = 0;
        for rule in rules {
            let mut month = match &rule.last_applied {
                Some(m) => util::add_months(m, 1),
                None => rule.start_month.clone(),
            };
            while month <= now_month {
                let day = rule.day_of_month.min(util::days_in_month(&month) as i64);
                if month == now_month && today_day < day {
                    break;
                }
                let date = format!("{month}-{day:02}");
                let ok = match rule.kind.as_str() {
                    "traspaso" => self
                        .conn
                        .execute(
                            "INSERT INTO transfers (amount_cents, from_account, to_account, note, date) VALUES (?, ?, ?, ?, ?)",
                            params![rule.amount_cents, rule.from_account, rule.to_account, rule.note, date],
                        )
                        .is_ok(),
                    "aportacion" => {
                        let inv_ok: Option<String> = self
                            .conn
                            .query_row("SELECT name FROM investments WHERE id = ? AND archived = 0", [rule.investment_id.unwrap_or(0)], |r| {
                                r.get(0)
                            })
                            .optional()
                            .ok()
                            .flatten();
                        let Some(inv_name) = inv_ok else {
                            // La inversión ya no existe: se pausa la regla.
                            let _ = self.conn.execute("UPDATE recurring SET active = 0 WHERE id = ?", [rule.id]);
                            break;
                        };
                        let inv_id = rule.investment_id.unwrap_or(0);
                        let price = self.cached_price_eur(inv_id).unwrap_or(0.0);
                        let units = if price > 0.0 { rule.amount_cents as f64 / 100.0 / price } else { 0.0 };
                        let _ = self.conn.execute(
                            "UPDATE investments SET units = units + ?, cost_cents = cost_cents + ? WHERE id = ?",
                            params![units, rule.amount_cents, inv_id],
                        );
                        if units > 0.0 {
                            let _ = self.conn.execute(
                                "INSERT INTO investment_ops (investment_id, type, units, amount_cents, date) VALUES (?, 'compra', ?, ?, ?)",
                                params![inv_id, units, rule.amount_cents, date],
                            );
                        }
                        if !rule.from_account.is_empty() {
                            let note = if rule.note.is_empty() { format!("Aportación {inv_name}") } else { rule.note.clone() };
                            let _ = self.conn.execute(
                                "INSERT INTO transactions (type, amount_cents, category_id, note, account, date, source)
                                 VALUES ('gasto', ?, ?, ?, ?, ?, 'recurrente')",
                                params![rule.amount_cents, self.invest_category(), note, rule.from_account, date],
                            );
                        }
                        true
                    }
                    _ => self
                        .conn
                        .execute(
                            "INSERT INTO transactions (type, amount_cents, category_id, note, account, date, source)
                             VALUES (?, ?, ?, ?, ?, ?, 'recurrente')",
                            params![rule.ty, rule.amount_cents, rule.category_id, rule.note, rule.account, date],
                        )
                        .is_ok(),
                };
                if !ok {
                    break;
                }
                let _ = self.conn.execute("UPDATE recurring SET last_applied = ? WHERE id = ?", params![month, rule.id]);
                created += 1;
                month = util::add_months(&month, 1);
            }
        }
        if created > 0 {
            self.touch();
        }
        created
    }

    /// Pagos repetidos (misma nota, categoría e importe en 3+ de los últimos 5
    /// meses) que aún no son recurrentes: probables suscripciones.
    pub fn recurring_suggestions(&self) -> Vec<Suggestion> {
        let from = format!("{}-01", util::add_months(&util::current_month(), -4));
        let rows: Vec<Suggestion> = self
            .conn
            .prepare(
                "SELECT MIN(t.note), t.category_id, COALESCE(c.name, ''), t.amount_cents,
                        COUNT(DISTINCT strftime('%Y-%m', t.date)) AS months_seen,
                        CAST(ROUND(AVG(CAST(strftime('%d', t.date) AS INTEGER))) AS INTEGER)
                 FROM transactions t LEFT JOIN categories c ON c.id = t.category_id
                 WHERE t.type = 'gasto' AND t.source != 'recurrente' AND t.note != '' AND t.date >= ?
                 GROUP BY LOWER(TRIM(t.note)), t.category_id, t.amount_cents
                 HAVING months_seen >= 3
                 ORDER BY months_seen DESC, t.amount_cents DESC LIMIT 10",
            )
            .and_then(|mut st| {
                st.query_map([from], |r| {
                    Ok(Suggestion {
                        note: r.get(0)?,
                        category_id: r.get(1)?,
                        category_name: r.get(2)?,
                        amount_cents: r.get(3)?,
                        months_seen: r.get(4)?,
                        typical_day: r.get(5)?,
                    })
                })
                .map(|it| it.flatten().collect())
            })
            .unwrap_or_default();
        let covered: Vec<(i64, String)> = self
            .recurring_list()
            .into_iter()
            .filter(|r| r.active)
            .map(|r| (r.amount_cents, r.note.trim().to_lowercase()))
            .collect();
        rows.into_iter().filter(|s| !covered.contains(&(s.amount_cents, s.note.trim().to_lowercase()))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_core;
    use super::*;

    #[test]
    fn materializa_una_vez_por_mes() {
        let mut c = test_core();
        let r = Recurring {
            kind: "movimiento".into(),
            ty: "gasto".into(),
            amount_cents: 999,
            note: "Spotify".into(),
            account: "Tarjeta".into(),
            day_of_month: 1,
            active: true,
            ..Default::default()
        };
        c.save_recurring(None, &r).unwrap();
        // Forzar el inicio tres meses atrás.
        let start = util::add_months(&util::current_month(), -3);
        c.conn.execute("UPDATE recurring SET start_month = ?, last_applied = NULL", [&start]).unwrap();
        c.conn.execute("DELETE FROM transactions", []).unwrap();
        c.apply_pending();
        let n: i64 = c.conn.query_row("SELECT COUNT(*) FROM transactions WHERE source = 'recurrente'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 4);
        c.apply_pending();
        let n2: i64 = c.conn.query_row("SELECT COUNT(*) FROM transactions WHERE source = 'recurrente'", [], |r| r.get(0)).unwrap();
        assert_eq!(n2, 4);
    }
}
