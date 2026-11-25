//! Cuentas: saldo = saldo inicial (a una fecha) + movimientos + transferencias.
//!
//! Cuentas con TAE: el interés se simula día a día (compuesto diario sobre el
//! saldo real del día anterior). Cada mes cerrado se apunta como ingreso en
//! «Intereses» el último día del mes; en esos meses ya no se simula: cuenta el
//! movimiento, que se puede editar para cuadrarlo con el banco. Borrarlo no lo
//! recrea (lo marca `interest_postings`).

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{OptionalExtension, params};

use super::{Core, R, SqlErr, err};
use crate::util;

#[derive(Clone, Debug, Default)]
pub struct Account {
    pub name: String,
    pub opening_cents: i64,
    pub opening_date: String,
    pub apy: f64,
    pub sort_order: i64,
    pub current_cents: i64,
    /// Interés estimado del mes en curso, aún sin apuntar.
    pub interest_cents: i64,
    /// true si tiene saldo inicial guardado (si no, solo existe por sus movimientos).
    pub stored: bool,
}

#[derive(Clone, Debug)]
pub struct NetPoint {
    pub date: String,
    pub accounts_cents: i64,
    pub investments_cents: i64,
}

impl NetPoint {
    pub fn total(&self) -> i64 {
        self.accounts_cents + self.investments_cents
    }
}

fn next_day(d: &str) -> String {
    util::parse_date(d).map(|nd| util::fmt_date(nd + Duration::days(1))).unwrap_or_else(|| d.to_string())
}

impl Core {
    fn account_nets(&self, name: &str, opening: &str, until: &str) -> i64 {
        let tx: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(CASE WHEN type = 'ingreso' THEN amount_cents ELSE -amount_cents END), 0)
                 FROM transactions WHERE account = ? COLLATE NOCASE AND date >= ? AND date <= ?",
                params![name, opening, until],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let tr: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(CASE WHEN to_account = ?1 COLLATE NOCASE THEN amount_cents ELSE 0 END), 0)
                      - COALESCE(SUM(CASE WHEN from_account = ?1 COLLATE NOCASE THEN amount_cents ELSE 0 END), 0)
                 FROM transfers WHERE date >= ?2 AND date <= ?3
                   AND (to_account = ?1 COLLATE NOCASE OR from_account = ?1 COLLATE NOCASE)",
                params![name, opening, until],
                |r| r.get(0),
            )
            .unwrap_or(0);
        tx + tr
    }

    fn posted_months(&self, name: &str) -> HashSet<String> {
        self.conn
            .prepare("SELECT month FROM interest_postings WHERE account = ? COLLATE NOCASE")
            .and_then(|mut st| st.query_map([name], |r| r.get::<_, String>(0)).map(|it| it.filter_map(Result::ok).collect()))
            .unwrap_or_default()
    }

    /// Movimientos netos por día de una cuenta entre dos fechas.
    fn per_day(&self, name: &str, opening: &str, end: &str) -> HashMap<String, i64> {
        let mut m: HashMap<String, i64> = HashMap::new();
        let q1 = "SELECT date, SUM(CASE WHEN type = 'ingreso' THEN amount_cents ELSE -amount_cents END)
                  FROM transactions WHERE account = ? COLLATE NOCASE AND date >= ? AND date <= ? GROUP BY date";
        let q2 = "SELECT date, SUM(CASE WHEN to_account = ?1 COLLATE NOCASE THEN amount_cents ELSE -amount_cents END)
                  FROM transfers WHERE date >= ?2 AND date <= ?3
                    AND (to_account = ?1 COLLATE NOCASE OR from_account = ?1 COLLATE NOCASE) GROUP BY date";
        for q in [q1, q2] {
            if let Ok(mut st) = self.conn.prepare(q) {
                if let Ok(rows) = st.query_map(params![name, opening, end], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))) {
                    for (d, v) in rows.flatten() {
                        *m.entry(d).or_insert(0) += v;
                    }
                }
            }
        }
        m
    }

    /// Simula una cuenta remunerada hasta `end`. Devuelve (saldo sin redondear, interés por mes).
    fn simulate(&self, name: &str, opening_cents: i64, opening: &str, end: &str, apy: f64) -> (f64, HashMap<String, f64>) {
        let daily = (1.0 + apy / 100.0).powf(1.0 / 365.0) - 1.0;
        let posted = self.posted_months(name);
        let per_day = self.per_day(name, opening, end);
        let mut bal = opening_cents as f64 + *per_day.get(opening).unwrap_or(&0) as f64;
        let mut accrued: HashMap<String, f64> = HashMap::new();
        let (Some(mut cur), Some(endd)) = (util::parse_date(opening), util::parse_date(end)) else {
            return (bal, accrued);
        };
        while cur < endd {
            cur += Duration::days(1);
            let month = format!("{:04}-{:02}", cur.year(), cur.month());
            if !posted.contains(&month) {
                let i = bal * daily;
                bal += i;
                *accrued.entry(month).or_insert(0.0) += i;
            }
            bal += *per_day.get(&util::fmt_date(cur)).unwrap_or(&0) as f64;
        }
        (bal, accrued)
    }

    /// Saldo de una cuenta a una fecha (None = hoy, contando también movimientos futuros).
    pub fn account_balance(&self, name: &str, opening_cents: i64, opening: &str, until: Option<&str>, apy: f64) -> i64 {
        let until_s = until.unwrap_or("9999-12-31");
        let end = until.map(str::to_string).unwrap_or_else(util::today_str);
        if apy <= 0.0 || end.as_str() <= opening {
            return opening_cents + self.account_nets(name, opening, until_s);
        }
        let (bal, _) = self.simulate(name, opening_cents, opening, &end, apy);
        let future = if until_s > end.as_str() { self.account_nets(name, &next_day(&end), until_s) } else { 0 };
        bal.round() as i64 + future
    }

    fn interest_category(&self) -> Option<i64> {
        let q = |n: &str| -> Option<i64> {
            self.conn
                .query_row("SELECT id FROM categories WHERE name = ? COLLATE NOCASE AND type = 'ingreso'", [n], |r| r.get(0))
                .optional()
                .ok()
                .flatten()
        };
        q("Intereses").or_else(|| q("Otros ingresos"))
    }

    /// Apunta como ingreso el interés de cada mes cerrado (idempotente).
    pub fn apply_interest(&mut self) -> usize {
        let accs: Vec<(String, i64, String, f64)> = self
            .conn
            .prepare("SELECT name, opening_cents, opening_date, apy FROM account_balances WHERE apy > 0")
            .and_then(|mut st| st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map(|it| it.flatten().collect()))
            .unwrap_or_default();
        if accs.is_empty() {
            return 0;
        }
        let today = util::today_str();
        let mut created = 0;
        for (name, opening_cents, opening, apy) in accs {
            let posted = self.posted_months(&name);
            let mut month = opening.get(0..7).unwrap_or("2000-01").to_string();
            while util::last_day_of_month(&month) <= today {
                let date = util::last_day_of_month(&month);
                if !posted.contains(&month) && date > opening {
                    let (_, accrued) = self.simulate(&name, opening_cents, &opening, &date, apy);
                    let cents = accrued.get(&month).copied().unwrap_or(0.0).round() as i64;
                    let tx_id: Option<i64> = if cents > 0 {
                        let note = format!("Intereses {name} ({}% TAE)", util::fmt_apy(apy));
                        let ok = self.conn.execute(
                            "INSERT INTO transactions (type, amount_cents, category_id, note, account, date, source)
                             VALUES ('ingreso', ?, ?, ?, ?, ?, 'intereses')",
                            params![cents, self.interest_category(), note, name, date],
                        );
                        if ok.is_err() {
                            break;
                        }
                        Some(self.conn.last_insert_rowid())
                    } else {
                        None
                    };
                    let _ = self.conn.execute(
                        "INSERT INTO interest_postings (account, month, transaction_id, amount_cents) VALUES (?, ?, ?, ?)",
                        params![name, month, tx_id, cents],
                    );
                    created += 1;
                }
                month = util::add_months(&month, 1);
            }
        }
        if created > 0 {
            self.touch();
        }
        created
    }

    /// Todas las cuentas: las guardadas y las que solo aparecen en movimientos.
    pub fn accounts(&self) -> Vec<Account> {
        let stored: Vec<(String, i64, String, f64, i64)> = self
            .conn
            .prepare("SELECT name, opening_cents, opening_date, apy, sort_order FROM account_balances ORDER BY sort_order, name COLLATE NOCASE")
            .and_then(|mut st| {
                st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).map(|it| it.flatten().collect())
            })
            .unwrap_or_default();
        let used: Vec<String> = self
            .conn
            .prepare(
                "SELECT DISTINCT account FROM transactions WHERE account != ''
                 UNION SELECT DISTINCT from_account FROM transfers UNION SELECT DISTINCT to_account FROM transfers",
            )
            .and_then(|mut st| st.query_map([], |r| r.get::<_, String>(0)).map(|it| it.flatten().collect()))
            .unwrap_or_default();

        let mut out: Vec<Account> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for (name, oc, od, apy, so) in stored {
            seen.insert(name.to_lowercase());
            out.push(Account { name, opening_cents: oc, opening_date: od, apy, sort_order: so, stored: true, ..Default::default() });
        }
        for name in used {
            if !name.trim().is_empty() && seen.insert(name.to_lowercase()) {
                out.push(Account { name, opening_date: "1970-01-01".into(), sort_order: 9999, ..Default::default() });
            }
        }
        for a in &mut out {
            a.current_cents = self.account_balance(&a.name, a.opening_cents, &a.opening_date, None, a.apy);
            a.interest_cents = if a.apy > 0.0 {
                a.current_cents - self.account_balance(&a.name, a.opening_cents, &a.opening_date, None, 0.0)
            } else {
                0
            };
        }
        out.sort_by(|a, b| a.sort_order.cmp(&b.sort_order).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        out
    }

    fn account_exists(&self, name: &str) -> bool {
        self.conn
            .query_row(
                "SELECT 1 FROM account_balances WHERE name = ?1 COLLATE NOCASE
                 UNION ALL SELECT 1 FROM transactions WHERE account = ?1 COLLATE NOCASE
                 UNION ALL SELECT 1 FROM transfers WHERE from_account = ?1 COLLATE NOCASE OR to_account = ?1 COLLATE NOCASE
                 LIMIT 1",
                [name],
                |_| Ok(()),
            )
            .optional()
            .ok()
            .flatten()
            .is_some()
    }

    /// Crea o actualiza el saldo inicial, la fecha y la TAE de una cuenta.
    pub fn save_account(&mut self, name: &str, opening_cents: i64, opening_date: &str, apy: f64) -> R<()> {
        let name: String = name.trim().chars().take(60).collect();
        if name.is_empty() {
            return err("Falta el nombre de la cuenta");
        }
        if !(0.0..=100.0).contains(&apy) || !apy.is_finite() {
            return err("TAE no válida (0-100)");
        }
        let date = if util::parse_date(opening_date).is_some() { opening_date.to_string() } else { util::today_str() };
        self.conn
            .execute(
                "INSERT INTO account_balances (name, opening_cents, opening_date, apy, sort_order)
                 VALUES (?, ?, ?, ?, COALESCE((SELECT MAX(sort_order)+1 FROM account_balances), 0))
                 ON CONFLICT(name) DO UPDATE SET opening_cents = excluded.opening_cents,
                   opening_date = excluded.opening_date, apy = excluded.apy",
                params![name, opening_cents, date, apy],
            )
            .e()?;
        self.touch();
        self.apply_interest();
        Ok(())
    }

    /// Borra el saldo inicial guardado (la cuenta sigue si tiene movimientos).
    pub fn delete_account(&mut self, name: &str) -> R<()> {
        self.conn.execute("DELETE FROM account_balances WHERE name = ? COLLATE NOCASE", [name]).e()?;
        self.touch();
        Ok(())
    }

    pub fn reorder_accounts(&mut self, names: &[String]) -> R<()> {
        let tx = self.conn.unchecked_transaction().e()?;
        for (i, n) in names.iter().enumerate() {
            tx.execute(
                "INSERT INTO account_balances (name, opening_cents, opening_date, apy, sort_order) VALUES (?, 0, '1970-01-01', 0, ?)
                 ON CONFLICT(name) DO UPDATE SET sort_order = excluded.sort_order",
                params![n, i as i64],
            )
            .e()?;
        }
        tx.commit().e()?;
        self.touch();
        Ok(())
    }

    /// Renombra en todas las tablas, en una transacción. No fusiona cuentas.
    pub fn rename_account(&mut self, old: &str, new: &str) -> R<()> {
        let old = old.trim();
        let new: String = new.trim().chars().take(60).collect();
        if new.is_empty() {
            return err("El nuevo nombre no puede estar vacío");
        }
        if old == new {
            return err("El nombre nuevo es igual al actual");
        }
        if !self.account_exists(old) {
            return err("Cuenta no encontrada");
        }
        if !old.eq_ignore_ascii_case(&new) && self.account_exists(&new) {
            return err(format!("Ya existe una cuenta con el nombre «{new}»"));
        }
        let tx = self.conn.unchecked_transaction().e()?;
        let existing: Option<(i64, String, f64, i64)> = tx
            .query_row(
                "SELECT opening_cents, opening_date, apy, sort_order FROM account_balances WHERE name = ? COLLATE NOCASE",
                [old],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .e()?;
        if let Some((oc, od, apy, so)) = existing {
            tx.execute("DELETE FROM account_balances WHERE name = ? COLLATE NOCASE", [old]).e()?;
            tx.execute(
                "INSERT INTO account_balances (name, opening_cents, opening_date, apy, sort_order) VALUES (?, ?, ?, ?, ?)",
                params![new, oc, od, apy, so],
            )
            .e()?;
        }
        for sql in [
            "UPDATE transactions SET account = ?1 WHERE account = ?2 COLLATE NOCASE",
            "UPDATE transfers SET from_account = ?1 WHERE from_account = ?2 COLLATE NOCASE",
            "UPDATE transfers SET to_account = ?1 WHERE to_account = ?2 COLLATE NOCASE",
            "UPDATE recurring SET account = ?1 WHERE account = ?2 COLLATE NOCASE",
            "UPDATE recurring SET from_account = ?1 WHERE from_account = ?2 COLLATE NOCASE",
            "UPDATE recurring SET to_account = ?1 WHERE to_account = ?2 COLLATE NOCASE",
            "UPDATE templates SET account = ?1 WHERE account = ?2 COLLATE NOCASE",
            "UPDATE interest_postings SET account = ?1 WHERE account = ?2 COLLATE NOCASE",
            "UPDATE app_settings SET value = ?1 WHERE key = 'default_account' AND value = ?2 COLLATE NOCASE",
        ] {
            tx.execute(sql, params![new, old]).e()?;
        }
        tx.commit().e()?;
        self.touch();
        Ok(())
    }

    /// Cuadrar saldo: apunta un ajuste por la diferencia con el saldo real.
    pub fn reconcile(&mut self, name: &str, real_cents: i64) -> R<i64> {
        let acc = self.accounts().into_iter().find(|a| a.name.eq_ignore_ascii_case(name)).ok_or("Cuenta no encontrada")?;
        let diff = real_cents - acc.current_cents;
        if diff == 0 {
            return Ok(0);
        }
        let ty = if diff > 0 { "ingreso" } else { "gasto" };
        let cat = self.resolve_category("", ty);
        self.create_tx(&super::TxInput {
            ty: ty.into(),
            amount_cents: diff.abs(),
            category_id: cat,
            note: "Ajuste de saldo".into(),
            account: acc.name,
            date: util::today_str(),
            tags: String::new(),
            source: "ajuste".into(),
        })?;
        Ok(diff)
    }

    /// Evolución del patrimonio: diaria (semanal si el rango pasa de ~400 días).
    pub fn networth(&self) -> Vec<NetPoint> {
        let openings: Vec<(String, i64, String, f64)> = self
            .conn
            .prepare("SELECT name, opening_cents, opening_date, apy FROM account_balances")
            .and_then(|mut st| st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map(|it| it.flatten().collect()))
            .unwrap_or_default();
        // Antes de la primera compra no puede haber cartera: una foto anterior
        // es un dato erróneo (p. ej. tomada con un ticker mal puesto) y se ignora.
        let snaps: Vec<(String, i64)> = self
            .conn
            .prepare(
                "SELECT date, investments_cents FROM networth_snapshots
                 WHERE date >= COALESCE((SELECT MIN(date) FROM investment_ops), '0000-00-00') ORDER BY date",
            )
            .and_then(|mut st| st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map(|it| it.flatten().collect()))
            .unwrap_or_default();
        let today = util::today();
        let mut start = today;
        for (_, _, od, _) in &openings {
            if let Some(d) = util::parse_date(od) {
                if d < start && d.year() > 1970 {
                    start = d;
                }
            }
        }
        if let Some((d, _)) = snaps.first() {
            if let Some(d) = util::parse_date(d) {
                start = start.min(d);
            }
        }
        let total_days = (today - start).num_days().max(1);
        let step = if total_days > 400 { 7 } else { 1 };
        let mut dates: Vec<NaiveDate> = Vec::new();
        let mut d = start;
        while d < today {
            dates.push(d);
            d += Duration::days(step);
        }
        dates.push(today);

        // Precalcula movimientos por día de cada cuenta para no consultar por fecha.
        let mut out = Vec::with_capacity(dates.len());
        let mut snap_i = 0;
        let mut last_snap = 0;
        for d in dates {
            let ds = util::fmt_date(d);
            let mut accounts = 0;
            for (name, oc, od, apy) in &openings {
                if od.as_str() <= ds.as_str() {
                    accounts += self.account_balance(name, *oc, od, Some(&ds), *apy);
                }
            }
            while snap_i < snaps.len() && snaps[snap_i].0 <= ds {
                last_snap = snaps[snap_i].1;
                snap_i += 1;
            }
            out.push(NetPoint { date: ds, accounts_cents: accounts, investments_cents: last_snap });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::{TxInput, test_core};

    #[test]
    fn tae_diaria_y_intereses_mensuales() {
        let mut c = test_core();
        // 10.000 € al 3 % TAE durante un año exacto → 10.300 €.
        c.conn
            .execute("INSERT INTO account_balances (name, opening_cents, opening_date, apy) VALUES ('A', 1000000, '2024-01-01', 3)", [])
            .unwrap();
        let bal = c.account_balance("A", 1000000, "2024-01-01", Some("2024-12-31"), 3.0);
        // 2024 es bisiesto: 365 días de interés hasta el 31-12.
        assert_eq!(bal, 1030000);

        // Apuntar intereses no cambia el saldo (los meses apuntados ya no se simulan).
        let before = c.account_balance("A", 1000000, "2024-01-01", None, 3.0);
        let n = c.apply_interest();
        assert!(n >= 12);
        let after = c.account_balance("A", 1000000, "2024-01-01", None, 3.0);
        assert!((before - after).abs() <= n as i64, "{before} vs {after}");
        // Idempotente.
        assert_eq!(c.apply_interest(), 0);
    }

    #[test]
    fn renombrar_no_fusiona() {
        let mut c = test_core();
        c.save_account("Banco", 1000, "2026-01-01", 0.0).unwrap();
        c.create_tx(&TxInput { ty: "gasto".into(), amount_cents: 100, account: "Tarjeta".into(), ..Default::default() }).unwrap();
        assert!(c.rename_account("Banco", "tarjeta").is_err());
        c.rename_account("Banco", "Banco Principal").unwrap();
        assert!(c.accounts().iter().any(|a| a.name == "Banco Principal" && a.current_cents == 1000));
        // Solo mayúsculas: permitido.
        c.rename_account("Banco Principal", "BANCO PRINCIPAL").unwrap();
    }

    #[test]
    fn cuadrar_saldo() {
        let mut c = test_core();
        c.save_account("Efectivo", 500, "2026-01-01", 0.0).unwrap();
        assert_eq!(c.reconcile("Efectivo", 300).unwrap(), -200);
        assert_eq!(c.accounts()[0].current_cents, 300);
    }
}
