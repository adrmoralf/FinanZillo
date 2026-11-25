//! Datos de demostración ficticios (para probar la app y para las capturas del
//! README). Deterministas: misma fecha → mismos datos. Nada es real.
//!
//! `finanzillo --demo <ruta.db>` crea una base SQLite con un año de movimientos
//! que se puede importar al crear el archivo cifrado.

use crate::ledger::recurring::Recurring;
use crate::ledger::{Core, Transfer, TxInput};
use crate::util;
use chrono::{Datelike, Duration};
use rusqlite::params;

/// Generador pseudoaleatorio sencillo y reproducible (LCG).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    /// Entero en [lo, hi].
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i64
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.next() % 100 < pct
    }
    fn pick<'a>(&mut self, xs: &'a [&'a str]) -> &'a str {
        xs[(self.next() % xs.len() as u64) as usize]
    }
}

fn cat(core: &Core, name: &str, ty: &str) -> Option<i64> {
    core.resolve_category(name, ty)
}

fn tx(core: &mut Core, ty: &str, cents: i64, category: &str, note: &str, account: &str, date: &str, tags: &str) {
    let category_id = cat(core, category, ty);
    let _ = core.create_tx(&TxInput {
        ty: ty.into(),
        amount_cents: cents,
        category_id,
        note: note.into(),
        account: account.into(),
        date: date.into(),
        tags: tags.into(),
        source: "demo".into(),
    });
}

pub fn build() -> Result<Core, String> {
    let mut core = Core::new(crate::db::new_memory().map_err(|e| e.to_string())?);
    let mut rng = Rng(20261125);
    let today = util::today();
    let first_month = util::add_months(&util::current_month(), -11);
    let start = util::first_day(&first_month);
    let start_s = util::fmt_date(start);

    // Cuentas ficticias.
    core.save_account("Banco Principal", 210000, &start_s, 0.0)?;
    core.save_account("Cuenta Ahorro", 300000, &start_s, 2.0)?;
    core.save_account("Efectivo", 8000, &start_s, 0.0)?;
    core.save_account("Tarjeta", 0, &start_s, 0.0)?;
    core.reorder_accounts(&["Banco Principal".into(), "Cuenta Ahorro".into(), "Tarjeta".into(), "Efectivo".into()])?;
    core.set_setting("default_account", "Banco Principal")?;

    // Presupuestos (con historial desde el primer mes, para que valgan en todo el año).
    for (name, cents) in [("Alimentación", 32000), ("Restaurante", 12000), ("Ocio", 10000), ("Transporte", 9000)] {
        if let Some(id) = cat(&core, name, "gasto") {
            let c = core.category(id).unwrap();
            core.save_category(Some(id), &c.name, "gasto", &c.icon, &c.color, cents, false)?;
            core.conn
                .execute("UPDATE budget_history SET month = ? WHERE category_id = ?", params![first_month, id])
                .map_err(|e| e.to_string())?;
        }
    }
    core.set_global_budget(140000)?;
    core.conn.execute("UPDATE budget_history SET month = ? WHERE category_id = 0", [&first_month]).map_err(|e| e.to_string())?;
    for name in ["Vivienda", "Suscripciones"] {
        if let Some(id) = cat(&core, name, "gasto") {
            core.conn.execute("UPDATE categories SET fixed = 1 WHERE id = ?", [id]).map_err(|e| e.to_string())?;
        }
    }

    // Inversión con precio manual (sin red) y aportación mensual.
    let inv = core.save_investment(None, "Fondo indexado global (demo)", "manual", "", 1.0, 10000, Some(100.0))?;
    core.conn.execute("UPDATE investment_ops SET date = ? WHERE investment_id = ?", params![start_s, inv]).map_err(|e| e.to_string())?;

    let supers = ["Supermercado", "Frutería", "Supermercado", "Mercado municipal", "Panadería"];
    let restaurantes = ["Pizzería", "Menú del día", "Tapas con amigos", "Sushi", "Hamburguesería", "Cena de cumpleaños"];
    let ocio = ["Cine", "Concierto", "Libro", "Videojuego", "Museo", "Escape room", "Teatro"];

    let mut d = start;
    let mut price = 100.0f64;
    // Gasto con tarjeta por mes, para liquidarla desde el banco el mes siguiente.
    let mut card: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    while d <= today {
        let ds = util::fmt_date(d);
        let month = format!("{:04}-{:02}", d.year(), d.month());
        let (_, mnum) = util::month_parts(&month);
        let dow = d.weekday().num_days_from_monday();
        match d.day() {
            1 => {
                tx(&mut core, "ingreso", 185000, "Nómina", "Nómina", "Banco Principal", &ds, "");
                tx(&mut core, "gasto", 65000, "Vivienda", "Alquiler", "Banco Principal", &ds, "");
            }
            2 => {
                let _ = core.save_transfer(
                    None,
                    &Transfer { id: 0, amount_cents: 20000, from_account: "Banco Principal".into(), to_account: "Cuenta Ahorro".into(), note: "Ahorro mensual".into(), date: ds.clone() },
                );
            }
            3 => {
                // Aportación al fondo al precio del mes.
                let units = 100.0 / price;
                let _ = core.add_op(inv, "compra", units, 10000, &ds);
                tx(&mut core, "gasto", 10000, "Inversiones", "Aportación fondo indexado", "Banco Principal", &ds, "");
            }
            5 => tx(&mut core, "gasto", 3490, "Salud", "Gimnasio", "Tarjeta", &ds, ""),
            8 => tx(&mut core, "gasto", rng.range(3800, 6400), "Vivienda", "Luz y agua", "Banco Principal", &ds, ""),
            12 => {
                tx(&mut core, "gasto", 1299, "Suscripciones", "Plataforma de series", "Tarjeta", &ds, "");
                tx(&mut core, "gasto", 999, "Suscripciones", "Música en streaming", "Tarjeta", &ds, "");
            }
            15 => tx(&mut core, "gasto", 3500, "Vivienda", "Internet y móvil", "Banco Principal", &ds, ""),
            6 => {
                // Liquidación de la tarjeta: lo gastado con ella el mes anterior.
                let prev = util::add_months(&month, -1);
                if let Some(c) = card.get(&prev).copied().filter(|c| *c > 0) {
                    let _ = core.save_transfer(
                        None,
                        &Transfer { id: 0, amount_cents: c, from_account: "Banco Principal".into(), to_account: "Tarjeta".into(), note: "Liquidación tarjeta".into(), date: ds.clone() },
                    );
                }
            }
            10 => {
                let _ = core.save_transfer(
                    None,
                    &Transfer { id: 0, amount_cents: 2000, from_account: "Banco Principal".into(), to_account: "Efectivo".into(), note: "Cajero".into(), date: ds.clone() },
                );
            }
            _ => {}
        }
        // Compra del súper unas dos veces por semana.
        if (dow == 1 || dow == 4 || dow == 5) && rng.chance(70) {
            tx(&mut core, "gasto", rng.range(1800, 7400), "Alimentación", rng.pick(&supers), "Tarjeta", &ds, "");
        }
        // Comer fuera, sobre todo en fin de semana.
        if (dow >= 4 && rng.chance(38)) || rng.chance(5) {
            tx(&mut core, "gasto", rng.range(1100, 4200), "Restaurante", rng.pick(&restaurantes), "Tarjeta", &ds, "");
        }
        if rng.chance(9) {
            tx(&mut core, "gasto", rng.range(150, 450), "Restaurante", "Café", "Efectivo", &ds, "");
        }
        if dow < 5 && rng.chance(14) {
            tx(&mut core, "gasto", rng.range(150, 2200), "Transporte", if rng.chance(70) { "Metro y bus" } else { "Gasolina" }, "Tarjeta", &ds, "");
        }
        if rng.chance(8) {
            tx(&mut core, "gasto", rng.range(800, 3500), "Ocio", rng.pick(&ocio), "Tarjeta", &ds, "");
        }
        if rng.chance(3) {
            tx(&mut core, "gasto", rng.range(1990, 7990), "Ropa", "Ropa", "Tarjeta", &ds, "");
        }
        if mnum == 12 && (15..=23).contains(&d.day()) && rng.chance(45) {
            tx(&mut core, "gasto", rng.range(1500, 6000), "Regalos", "Regalo de Navidad", "Tarjeta", &ds, "navidad");
        }
        if mnum == 8 && (10..=20).contains(&d.day()) {
            tx(&mut core, "gasto", rng.range(4000, 13000), "Viajes", if d.day() == 10 { "Vuelos" } else { "Viaje de verano" }, "Tarjeta", &ds, "vacaciones");
        }
        if d.day() == 20 && rng.chance(30) {
            tx(&mut core, "ingreso", rng.range(5000, 25000), "Extra", "Trabajo extra", "Banco Principal", &ds, "");
        }
        let spent: i64 = core
            .conn
            .query_row("SELECT COALESCE(SUM(amount_cents), 0) FROM transactions WHERE account = 'Tarjeta' AND type = 'gasto' AND date = ?", [&ds], |r| r.get(0))
            .unwrap_or(0);
        *card.entry(month.clone()).or_insert(0) += spent;
        // Precio del fondo: deriva suave (≈ +5 % al año) con ruido; foto diaria los últimos 30 días.
        price *= 1.0 + (rng.range(-100, 103) as f64) / 10000.0;
        if (today - d).num_days() <= 30 {
            core.conn
                .execute("INSERT OR REPLACE INTO investment_prices (investment_id, date, price) VALUES (?, ?, ?)", params![inv, ds, price])
                .map_err(|e| e.to_string())?;
        }
        if d.day() == 1 || d == today {
            let units: f64 = core.conn.query_row("SELECT units FROM investments WHERE id = ?", [inv], |r| r.get(0)).unwrap_or(0.0);
            core.conn
                .execute("INSERT OR REPLACE INTO networth_snapshots (date, investments_cents) VALUES (?, ?)", params![ds, (units * price * 100.0).round() as i64])
                .map_err(|e| e.to_string())?;
        }
        d += Duration::days(1);
    }
    core.conn.execute("UPDATE investments SET manual_price = ? WHERE id = ?", params![(price * 100.0).round() / 100.0, inv]).map_err(|e| e.to_string())?;

    // Recurrentes ya al día (los movimientos de arriba los representan).
    let cm = util::current_month();
    for r in [
        Recurring { kind: "movimiento".into(), ty: "ingreso".into(), amount_cents: 185000, note: "Nómina".into(), account: "Banco Principal".into(), day_of_month: 1, category_id: cat(&core, "Nómina", "ingreso"), ..Default::default() },
        Recurring { kind: "movimiento".into(), ty: "gasto".into(), amount_cents: 65000, note: "Alquiler".into(), account: "Banco Principal".into(), day_of_month: 1, category_id: cat(&core, "Vivienda", "gasto"), ..Default::default() },
        Recurring { kind: "movimiento".into(), ty: "gasto".into(), amount_cents: 3490, note: "Gimnasio".into(), account: "Tarjeta".into(), day_of_month: 5, category_id: cat(&core, "Salud", "gasto"), ..Default::default() },
        Recurring { kind: "traspaso".into(), amount_cents: 20000, note: "Ahorro mensual".into(), from_account: "Banco Principal".into(), to_account: "Cuenta Ahorro".into(), day_of_month: 2, ..Default::default() },
    ] {
        let mut r = r;
        r.active = true;
        let id = core.save_recurring(None, &r)?;
        let last = if today.day() as i64 >= r.day_of_month { cm.clone() } else { util::add_months(&cm, -1) };
        core.conn.execute("UPDATE recurring SET last_applied = ?, start_month = ? WHERE id = ?", params![last, first_month, id]).map_err(|e| e.to_string())?;
    }
    // El alta de recurrentes pudo materializar el mes en curso: se quitan esos duplicados.
    core.conn.execute("DELETE FROM transactions WHERE source = 'recurrente'", []).map_err(|e| e.to_string())?;
    core.conn.execute("DELETE FROM transfers WHERE id NOT IN (SELECT MIN(id) FROM transfers GROUP BY date, from_account, to_account, amount_cents)", []).map_err(|e| e.to_string())?;

    // Objetivos y gastos rápidos.
    let next_year = today.year() + 1;
    core.save_goal(None, "Viaje a Japón", "🗾", 300000, Some(&format!("{next_year}-04-01")))?;
    core.save_goal(None, "Colchón de seguridad", "🛟", 600000, None)?;
    let goals = core.goals();
    core.goal_add(goals[0].id, 125000)?;
    core.goal_add(goals[1].id, 420000)?;
    for (name, cents, catn, acc) in [("Café", 160, "Restaurante", "Efectivo"), ("Metro", 150, "Transporte", "Tarjeta"), ("Pan", 120, "Alimentación", "Efectivo")] {
        let input = TxInput { ty: "gasto".into(), amount_cents: cents, category_id: cat(&core, catn, "gasto"), note: name.into(), account: acc.into(), ..Default::default() };
        core.create_template(name, &input)?;
    }
    core.apply_pending();
    Ok(core)
}

/// Escribe la base de demostración en `path` (SQLite normal, importable).
pub fn write(path: &std::path::Path) -> Result<(), String> {
    let core = build()?;
    let bytes = crate::db::to_bytes(&core.conn).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn demo_coherente() {
        let c = super::build().unwrap();
        let n = c.transactions(&crate::ledger::TxFilter { limit: 5000, ..Default::default() }).total_count;
        assert!(n > 300, "pocos movimientos: {n}");
        let s = c.summary(&crate::util::add_months(&crate::util::current_month(), -1));
        assert!(s.ingresos_cents >= 185000 && s.gastos_cents > 100000);
        assert_eq!(c.accounts().len(), 4);
        assert!(c.portfolio().value_cents > 0);
    }
}
