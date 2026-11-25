//! Lógica de negocio (portada de la API de la app web). Todo trabaja sobre la
//! conexión SQLite en memoria; la UI solo llama a estos métodos.

pub mod accounts;
pub mod analysis;
pub mod csv;
pub mod investments;
pub mod recurring;
pub mod report;

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::util;

pub type R<T> = Result<T, String>;

pub fn err<T>(e: impl std::fmt::Display) -> R<T> {
    Err(e.to_string())
}

trait SqlErr<T> {
    fn e(self) -> R<T>;
}
impl<T> SqlErr<T> for rusqlite::Result<T> {
    fn e(self) -> R<T> {
        self.map_err(|e| format!("Error de base de datos: {e}"))
    }
}

pub struct Core {
    pub conn: Connection,
    /// Sube con cada cambio: la UI la usa para invalidar cachés y saber que hay que guardar.
    pub version: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub ty: String,
    pub icon: String,
    pub color: String,
    pub archived: bool,
    pub budget_cents: i64,
    pub fixed: bool,
    pub usage_count: i64,
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct Tx {
    pub id: i64,
    pub ty: String,
    pub amount_cents: i64,
    pub category_id: Option<i64>,
    pub note: String,
    pub account: String,
    pub date: String,
    pub source: String,
    pub tags: String,
    pub category_name: String,
    pub category_icon: String,
    pub category_color: String,
}

impl Tx {
    pub fn signed(&self) -> i64 {
        if self.ty == "ingreso" { self.amount_cents } else { -self.amount_cents }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TxInput {
    pub ty: String,
    pub amount_cents: i64,
    pub category_id: Option<i64>,
    pub note: String,
    pub account: String,
    pub date: String,
    pub tags: String,
    pub source: String,
}

#[derive(Clone, Debug, Default)]
pub struct Transfer {
    pub id: i64,
    pub amount_cents: i64,
    pub from_account: String,
    pub to_account: String,
    pub note: String,
    pub date: String,
}

#[derive(Clone, Debug, Default)]
pub struct TxFilter {
    pub month: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub ty: Option<String>,
    pub category_id: Option<i64>,
    pub account: Option<String>,
    pub q: Option<String>,
    pub amount_min: Option<i64>,
    pub amount_max: Option<i64>,
    pub limit: usize,
}

#[derive(Clone, Debug, Default)]
pub struct TxList {
    pub items: Vec<Tx>,
    pub total_count: i64,
    pub gastos_cents: i64,
    pub ingresos_cents: i64,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Template {
    pub id: i64,
    pub name: String,
    pub ty: String,
    pub amount_cents: i64,
    pub category_id: Option<i64>,
    pub note: String,
    pub account: String,
    pub category_name: String,
    pub category_icon: String,
}

#[derive(Clone, Debug)]
pub struct Goal {
    pub id: i64,
    pub name: String,
    pub icon: String,
    pub target_cents: i64,
    pub saved_cents: i64,
    pub deadline: Option<String>,
}

const TX_SELECT: &str = "SELECT t.id, t.type, t.amount_cents, t.category_id, t.note, t.account, t.date, t.source, t.tags,
       COALESCE(c.name, ''), COALESCE(c.icon, ''), COALESCE(c.color, '#898781')
  FROM transactions t LEFT JOIN categories c ON c.id = t.category_id";

fn tx_from_row(r: &Row) -> rusqlite::Result<Tx> {
    Ok(Tx {
        id: r.get(0)?,
        ty: r.get(1)?,
        amount_cents: r.get(2)?,
        category_id: r.get(3)?,
        note: r.get(4)?,
        account: r.get(5)?,
        date: r.get(6)?,
        source: r.get(7)?,
        tags: r.get(8)?,
        category_name: r.get(9)?,
        category_icon: r.get(10)?,
        category_color: r.get(11)?,
    })
}

fn category_from_row(r: &Row) -> rusqlite::Result<Category> {
    Ok(Category {
        id: r.get(0)?,
        name: r.get(1)?,
        ty: r.get(2)?,
        icon: r.get(3)?,
        color: r.get(4)?,
        archived: r.get::<_, i64>(5)? != 0,
        budget_cents: r.get(6)?,
        fixed: r.get::<_, i64>(7)? != 0,
        usage_count: r.get(8)?,
    })
}

/// Etiquetas libres: "a, b" o "#a #b" → "a,b" sin '#' ni duplicados (máx. 10).
pub fn normalize_tags(raw: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for t in raw.split(|c: char| c == ',' || c.is_whitespace()) {
        let t: String = t.trim_start_matches('#').trim().chars().take(30).collect();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out.truncate(10);
    out.join(",")
}

pub fn sanitize_icon(v: &str, fallback: &str) -> String {
    let s: String = v.chars().filter(|c| !"<>&\"'`".contains(*c)).collect::<String>().trim().chars().take(8).collect();
    if s.is_empty() { fallback.to_string() } else { s }
}

pub fn sanitize_color(v: &str, fallback: &str) -> String {
    let s = v.trim();
    if s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit()) {
        s.to_lowercase()
    } else {
        fallback.to_string()
    }
}

fn clean_account(s: &str) -> String {
    s.trim().chars().take(60).collect()
}

impl Core {
    pub fn new(conn: Connection) -> Self {
        Core { conn, version: 1 }
    }

    pub fn touch(&mut self) {
        self.version += 1;
    }

    // ---------- ajustes ----------

    pub fn setting(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM app_settings WHERE key = ?", [key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    pub fn set_setting(&mut self, key: &str, value: &str) -> R<()> {
        self.conn
            .execute(
                "INSERT INTO app_settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .e()?;
        self.touch();
        Ok(())
    }

    pub fn default_account(&self) -> String {
        self.setting("default_account").unwrap_or_default()
    }

    pub fn global_budget_cents(&self) -> i64 {
        self.setting("global_budget_cents").and_then(|v| v.parse().ok()).unwrap_or(0)
    }

    pub fn set_global_budget(&mut self, cents: i64) -> R<()> {
        if cents < 0 {
            return err("Presupuesto no válido");
        }
        self.set_setting("global_budget_cents", &cents.to_string())?;
        self.record_budget_history(0, cents)
    }

    // ---------- presupuestos con historial ----------

    fn record_budget_history(&mut self, category_id: i64, cents: i64) -> R<()> {
        self.conn
            .execute(
                "INSERT INTO budget_history (category_id, month, budget_cents) VALUES (?, ?, ?)
                 ON CONFLICT(category_id, month) DO UPDATE SET budget_cents = excluded.budget_cents",
                params![category_id, util::current_month(), cents],
            )
            .e()?;
        Ok(())
    }

    pub fn budget_for_month(&self, category_id: i64, month: &str, fallback: i64) -> i64 {
        self.conn
            .query_row(
                "SELECT budget_cents FROM budget_history WHERE category_id = ? AND month <= ? ORDER BY month DESC LIMIT 1",
                params![category_id, month],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten()
            .unwrap_or(fallback)
    }

    // ---------- categorías ----------

    pub fn categories(&self, include_archived: bool) -> Vec<Category> {
        let sql = format!(
            "SELECT c.id, c.name, c.type, c.icon, c.color, c.archived, c.budget_cents, c.fixed,
                    (SELECT COUNT(*) FROM transactions t WHERE t.category_id = c.id)
             FROM categories c {} ORDER BY c.type, c.name",
            if include_archived { "" } else { "WHERE c.archived = 0" }
        );
        let mut st = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        st.query_map([], category_from_row).map(|it| it.filter_map(Result::ok).collect()).unwrap_or_default()
    }

    pub fn category(&self, id: i64) -> Option<Category> {
        self.conn
            .query_row(
                "SELECT c.id, c.name, c.type, c.icon, c.color, c.archived, c.budget_cents, c.fixed,
                        (SELECT COUNT(*) FROM transactions t WHERE t.category_id = c.id)
                 FROM categories c WHERE c.id = ?",
                [id],
                category_from_row,
            )
            .optional()
            .ok()
            .flatten()
    }

    /// Categoría por nombre o, si no existe, "Otros" / "Otros ingresos".
    pub fn resolve_category(&self, name: &str, ty: &str) -> Option<i64> {
        let find = |n: &str| -> Option<i64> {
            self.conn
                .query_row("SELECT id FROM categories WHERE name = ? COLLATE NOCASE AND type = ?", params![n, ty], |r| {
                    r.get(0)
                })
                .optional()
                .ok()
                .flatten()
        };
        if !name.trim().is_empty() {
            if let Some(id) = find(name.trim()) {
                return Some(id);
            }
        }
        find(if ty == "gasto" { "Otros" } else { "Otros ingresos" })
    }

    fn valid_category(&self, id: Option<i64>, ty: &str) -> Option<i64> {
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

    pub fn save_category(
        &mut self,
        id: Option<i64>,
        name: &str,
        ty: &str,
        icon: &str,
        color: &str,
        budget_cents: i64,
        fixed: bool,
    ) -> R<i64> {
        let name = name.trim();
        if name.is_empty() {
            return err("Falta el nombre");
        }
        if ty != "gasto" && ty != "ingreso" {
            return err("El tipo debe ser gasto o ingreso");
        }
        if budget_cents < 0 {
            return err("Presupuesto no válido");
        }
        let icon = sanitize_icon(icon, "📦");
        let color = sanitize_color(color, "#898781");
        let budget = if ty == "gasto" { budget_cents } else { 0 };
        let fixed_new = (ty == "gasto" && fixed) as i64;
        let result = match id {
            None => {
                self.conn
                    .execute(
                        "INSERT INTO categories (name, type, icon, color, budget_cents, fixed) VALUES (?, ?, ?, ?, ?, ?)",
                        params![name, ty, icon, color, budget, fixed_new],
                    )
                    .map_err(|_| "Ya existe una categoría con ese nombre y tipo".to_string())?;
                let new_id = self.conn.last_insert_rowid();
                if budget > 0 {
                    self.record_budget_history(new_id, budget)?;
                }
                new_id
            }
            Some(id) => {
                let existing = self.category(id).ok_or("Categoría no encontrada")?;
                let protected = ["otros", "otros ingresos"];
                if protected.contains(&existing.name.to_lowercase().as_str()) && !existing.name.eq_ignore_ascii_case(name) {
                    return err(format!("«{}» no se puede renombrar: es la categoría de reserva", existing.name));
                }
                let budget = if existing.ty == "gasto" { budget_cents } else { 0 };
                let fixed = (existing.ty == "gasto" && fixed) as i64;
                self.conn
                    .execute(
                        "UPDATE categories SET name = ?, icon = ?, color = ?, budget_cents = ?, fixed = ? WHERE id = ?",
                        params![name, icon, color, budget, fixed, id],
                    )
                    .map_err(|_| "Ya existe una categoría con ese nombre y tipo".to_string())?;
                if budget != existing.budget_cents {
                    self.record_budget_history(id, budget)?;
                }
                id
            }
        };
        self.touch();
        Ok(result)
    }

    /// Con movimientos se archiva (no se pierde histórico); sin ninguno, se borra.
    pub fn delete_category(&mut self, id: i64) -> R<bool> {
        let cat = self.category(id).ok_or("Categoría no encontrada")?;
        if ["otros", "otros ingresos"].contains(&cat.name.to_lowercase().as_str()) {
            return err(format!("«{}» no se puede borrar: es la categoría de reserva", cat.name));
        }
        let archived = if cat.usage_count > 0 {
            self.conn.execute("UPDATE categories SET archived = 1 WHERE id = ?", [id]).e()?;
            true
        } else {
            self.conn.execute("DELETE FROM categories WHERE id = ?", [id]).e()?;
            false
        };
        self.touch();
        Ok(archived)
    }

    pub fn unarchive_category(&mut self, id: i64) -> R<()> {
        self.conn.execute("UPDATE categories SET archived = 0 WHERE id = ?", [id]).e()?;
        self.touch();
        Ok(())
    }

    // ---------- movimientos ----------

    pub fn transactions(&self, f: &TxFilter) -> TxList {
        let mut wh: Vec<&str> = Vec::new();
        let mut ps: Vec<rusqlite::types::Value> = Vec::new();
        use rusqlite::types::Value as V;
        if let Some(m) = &f.month {
            wh.push("strftime('%Y-%m', t.date) = ?");
            ps.push(V::Text(m.clone()));
        }
        if let Some(d) = &f.from {
            wh.push("t.date >= ?");
            ps.push(V::Text(d.clone()));
        }
        if let Some(d) = &f.to {
            wh.push("t.date <= ?");
            ps.push(V::Text(d.clone()));
        }
        if let Some(t) = &f.ty {
            wh.push("t.type = ?");
            ps.push(V::Text(t.clone()));
        }
        if let Some(c) = f.category_id {
            wh.push("t.category_id = ?");
            ps.push(V::Integer(c));
        }
        if let Some(a) = &f.account {
            wh.push("t.account = ? COLLATE NOCASE");
            ps.push(V::Text(a.trim().to_string()));
        }
        if let Some(q) = f.q.as_ref().filter(|q| !q.trim().is_empty()) {
            wh.push("(t.note LIKE ? OR c.name LIKE ? OR t.tags LIKE ?)");
            let like = format!("%{}%", q.trim().trim_start_matches('#'));
            ps.push(V::Text(like.clone()));
            ps.push(V::Text(like.clone()));
            ps.push(V::Text(like));
        }
        if let Some(v) = f.amount_min {
            wh.push("t.amount_cents >= ?");
            ps.push(V::Integer(v));
        }
        if let Some(v) = f.amount_max {
            wh.push("t.amount_cents <= ?");
            ps.push(V::Integer(v));
        }
        // Los fragmentos de WHERE son literales propios; los valores van como parámetros.
        let where_sql = if wh.is_empty() { String::new() } else { format!("WHERE {}", wh.join(" AND ")) };
        let limit = if f.limit == 0 { 500 } else { f.limit.min(5000) };
        let sql = format!("{TX_SELECT} {where_sql} ORDER BY t.date DESC, t.id DESC LIMIT {limit}");
        let items = self
            .conn
            .prepare(&sql)
            .and_then(|mut st| {
                st.query_map(rusqlite::params_from_iter(ps.iter()), tx_from_row).map(|it| it.filter_map(Result::ok).collect())
            })
            .unwrap_or_default();
        let totals_sql = format!(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN t.type = 'gasto' THEN t.amount_cents END), 0),
                    COALESCE(SUM(CASE WHEN t.type = 'ingreso' THEN t.amount_cents END), 0)
             FROM transactions t LEFT JOIN categories c ON c.id = t.category_id {where_sql}"
        );
        let (total_count, gastos_cents, ingresos_cents) = self
            .conn
            .query_row(&totals_sql, rusqlite::params_from_iter(ps.iter()), |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap_or((0, 0, 0));
        TxList { items, total_count, gastos_cents, ingresos_cents }
    }

    pub fn transaction(&self, id: i64) -> Option<Tx> {
        self.conn.query_row(&format!("{TX_SELECT} WHERE t.id = ?"), [id], tx_from_row).optional().ok().flatten()
    }

    /// Categoría y cuenta de la última vez que se usó este concepto.
    pub fn guess_from_note(&self, note: &str, ty: &str) -> Option<(Option<i64>, String)> {
        let note = note.trim();
        if note.len() < 2 {
            return None;
        }
        self.conn
            .query_row(
                "SELECT category_id, account FROM transactions WHERE note = ? COLLATE NOCASE AND type = ? ORDER BY date DESC, id DESC LIMIT 1",
                params![note, ty],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .ok()
            .flatten()
    }

    /// Conceptos usados que empiezan por `prefix` (para autocompletar), los más frecuentes primero.
    pub fn note_suggestions(&self, prefix: &str, ty: &str, n: usize) -> Vec<String> {
        let prefix = prefix.trim();
        if prefix.is_empty() {
            return vec![];
        }
        let like = format!("{}%", prefix.replace('%', "").replace('_', ""));
        self.conn
            .prepare(
                "SELECT note FROM transactions WHERE note LIKE ? AND type = ? AND note != ''
                 GROUP BY LOWER(note) ORDER BY COUNT(*) DESC, MAX(date) DESC LIMIT ?",
            )
            .and_then(|mut st| st.query_map(params![like, ty, n as i64], |r| r.get::<_, String>(0)).map(|it| it.flatten().collect()))
            .unwrap_or_default()
    }

    pub fn recent(&self, n: usize) -> Vec<Tx> {
        self.transactions(&TxFilter { limit: n, ..Default::default() }).items
    }

    fn validate_tx(&self, i: &TxInput) -> R<TxInput> {
        if i.ty != "gasto" && i.ty != "ingreso" {
            return err("El tipo debe ser gasto o ingreso");
        }
        if i.amount_cents <= 0 {
            return err("Importe no válido");
        }
        let date = if i.date.is_empty() {
            util::today_str()
        } else if util::parse_date(&i.date).is_some() {
            i.date.clone()
        } else {
            return err("Fecha no válida");
        };
        Ok(TxInput {
            ty: i.ty.clone(),
            amount_cents: i.amount_cents,
            category_id: self.valid_category(i.category_id, &i.ty),
            note: i.note.trim().to_string(),
            account: clean_account(&i.account),
            date,
            tags: normalize_tags(&i.tags),
            source: if i.source.trim().is_empty() { "escritorio".into() } else { i.source.trim().to_string() },
        })
    }

    /// ¿Hay ya un movimiento igual (fecha, importe, tipo y nota)?
    pub fn is_duplicate(&self, i: &TxInput) -> bool {
        let date = if i.date.is_empty() { util::today_str() } else { i.date.clone() };
        self.conn
            .query_row(
                "SELECT 1 FROM transactions WHERE date = ? AND amount_cents = ? AND type = ? AND note = ? COLLATE NOCASE LIMIT 1",
                params![date, i.amount_cents, i.ty, i.note.trim()],
                |_| Ok(()),
            )
            .optional()
            .ok()
            .flatten()
            .is_some()
    }

    pub fn create_tx(&mut self, i: &TxInput) -> R<i64> {
        let v = self.validate_tx(i)?;
        self.conn
            .execute(
                "INSERT INTO transactions (type, amount_cents, category_id, note, account, date, source, tags) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![v.ty, v.amount_cents, v.category_id, v.note, v.account, v.date, v.source, v.tags],
            )
            .e()?;
        let id = self.conn.last_insert_rowid();
        self.touch();
        Ok(id)
    }

    pub fn update_tx(&mut self, id: i64, i: &TxInput) -> R<()> {
        if self.transaction(id).is_none() {
            return err("Movimiento no encontrado");
        }
        let v = self.validate_tx(i)?;
        self.conn
            .execute(
                "UPDATE transactions SET type = ?, amount_cents = ?, category_id = ?, note = ?, account = ?, date = ?, tags = ? WHERE id = ?",
                params![v.ty, v.amount_cents, v.category_id, v.note, v.account, v.date, v.tags, id],
            )
            .e()?;
        self.touch();
        Ok(())
    }

    pub fn delete_tx(&mut self, id: i64) -> R<()> {
        let n = self.conn.execute("DELETE FROM transactions WHERE id = ?", [id]).e()?;
        if n == 0 {
            return err("Movimiento no encontrado");
        }
        self.touch();
        Ok(())
    }

    // ---------- transferencias ----------

    pub fn transfers(&self, month: Option<&str>, account: Option<&str>) -> Vec<Transfer> {
        let mut sql = "SELECT id, amount_cents, from_account, to_account, note, date FROM transfers WHERE 1 = 1".to_string();
        let mut ps: Vec<String> = Vec::new();
        if let Some(m) = month {
            sql.push_str(" AND strftime('%Y-%m', date) = ?");
            ps.push(m.to_string());
        }
        if let Some(a) = account {
            sql.push_str(" AND (from_account = ? COLLATE NOCASE OR to_account = ? COLLATE NOCASE)");
            ps.push(a.to_string());
            ps.push(a.to_string());
        }
        sql.push_str(" ORDER BY date DESC, id DESC LIMIT 1000");
        self.conn
            .prepare(&sql)
            .and_then(|mut st| {
                st.query_map(rusqlite::params_from_iter(ps.iter()), |r| {
                    Ok(Transfer {
                        id: r.get(0)?,
                        amount_cents: r.get(1)?,
                        from_account: r.get(2)?,
                        to_account: r.get(3)?,
                        note: r.get(4)?,
                        date: r.get(5)?,
                    })
                })
                .map(|it| it.filter_map(Result::ok).collect())
            })
            .unwrap_or_default()
    }

    pub fn save_transfer(&mut self, id: Option<i64>, t: &Transfer) -> R<i64> {
        if t.amount_cents <= 0 {
            return err("Importe no válido");
        }
        let from = clean_account(&t.from_account);
        let to = clean_account(&t.to_account);
        if from.is_empty() || to.is_empty() {
            return err("Faltan las cuentas de origen y destino");
        }
        if from.eq_ignore_ascii_case(&to) {
            return err("Origen y destino no pueden ser la misma cuenta");
        }
        let date = if t.date.is_empty() { util::today_str() } else { t.date.clone() };
        if util::parse_date(&date).is_none() {
            return err("Fecha no válida");
        }
        let note = t.note.trim();
        let rid = match id {
            None => {
                self.conn
                    .execute(
                        "INSERT INTO transfers (amount_cents, from_account, to_account, note, date) VALUES (?, ?, ?, ?, ?)",
                        params![t.amount_cents, from, to, note, date],
                    )
                    .e()?;
                self.conn.last_insert_rowid()
            }
            Some(id) => {
                let n = self
                    .conn
                    .execute(
                        "UPDATE transfers SET amount_cents = ?, from_account = ?, to_account = ?, note = ?, date = ? WHERE id = ?",
                        params![t.amount_cents, from, to, note, date, id],
                    )
                    .e()?;
                if n == 0 {
                    return err("Transferencia no encontrada");
                }
                id
            }
        };
        self.touch();
        Ok(rid)
    }

    pub fn delete_transfer(&mut self, id: i64) -> R<()> {
        self.conn.execute("DELETE FROM transfers WHERE id = ?", [id]).e()?;
        self.touch();
        Ok(())
    }

    // ---------- plantillas (gastos rápidos) ----------

    pub fn templates(&self) -> Vec<Template> {
        self.conn
            .prepare(
                "SELECT tp.id, tp.name, tp.type, tp.amount_cents, tp.category_id, tp.note, tp.account,
                        COALESCE(c.name, ''), COALESCE(c.icon, '')
                 FROM templates tp LEFT JOIN categories c ON c.id = tp.category_id ORDER BY tp.id",
            )
            .and_then(|mut st| {
                st.query_map([], |r| {
                    Ok(Template {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        ty: r.get(2)?,
                        amount_cents: r.get(3)?,
                        category_id: r.get(4)?,
                        note: r.get(5)?,
                        account: r.get(6)?,
                        category_name: r.get(7)?,
                        category_icon: r.get(8)?,
                    })
                })
                .map(|it| it.filter_map(Result::ok).collect())
            })
            .unwrap_or_default()
    }

    pub fn create_template(&mut self, name: &str, i: &TxInput) -> R<i64> {
        let v = self.validate_tx(i)?;
        let n: i64 = self.conn.query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0)).e()?;
        if n >= 20 {
            return err("Máximo 20 gastos rápidos: borra alguno primero");
        }
        let name: String = {
            let n = name.trim();
            let base = if n.is_empty() { v.note.as_str() } else { n };
            let s: String = base.chars().take(40).collect();
            if s.is_empty() { "Rápido".into() } else { s }
        };
        self.conn
            .execute(
                "INSERT INTO templates (name, type, amount_cents, category_id, note, account) VALUES (?, ?, ?, ?, ?, ?)",
                params![name, v.ty, v.amount_cents, v.category_id, v.note, v.account],
            )
            .e()?;
        self.touch();
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_template(&mut self, id: i64) -> R<()> {
        self.conn.execute("DELETE FROM templates WHERE id = ?", [id]).e()?;
        self.touch();
        Ok(())
    }

    /// Apunta hoy el movimiento de una plantilla.
    pub fn use_template(&mut self, id: i64) -> R<i64> {
        let t = self.templates().into_iter().find(|t| t.id == id).ok_or("Gasto rápido no encontrado")?;
        self.create_tx(&TxInput {
            ty: t.ty,
            amount_cents: t.amount_cents,
            category_id: t.category_id,
            note: t.note,
            account: t.account,
            date: util::today_str(),
            tags: String::new(),
            source: "rapido".into(),
        })
    }

    // ---------- objetivos ----------

    pub fn goals(&self) -> Vec<Goal> {
        self.conn
            .prepare("SELECT id, name, icon, target_cents, saved_cents, deadline FROM goals WHERE archived = 0 ORDER BY id")
            .and_then(|mut st| {
                st.query_map([], |r| {
                    Ok(Goal {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        icon: r.get(2)?,
                        target_cents: r.get(3)?,
                        saved_cents: r.get(4)?,
                        deadline: r.get(5)?,
                    })
                })
                .map(|it| it.filter_map(Result::ok).collect())
            })
            .unwrap_or_default()
    }

    pub fn save_goal(&mut self, id: Option<i64>, name: &str, icon: &str, target: i64, deadline: Option<&str>) -> R<i64> {
        let name = name.trim();
        if name.is_empty() {
            return err("Falta el nombre");
        }
        if target <= 0 {
            return err("Objetivo no válido");
        }
        let deadline = deadline.map(str::trim).filter(|d| !d.is_empty());
        if let Some(d) = deadline {
            if util::parse_date(d).is_none() {
                return err("Fecha límite no válida (AAAA-MM-DD)");
            }
        }
        let icon = sanitize_icon(icon, "🎯");
        let rid = match id {
            None => {
                self.conn
                    .execute(
                        "INSERT INTO goals (name, icon, target_cents, deadline) VALUES (?, ?, ?, ?)",
                        params![name, icon, target, deadline],
                    )
                    .e()?;
                self.conn.last_insert_rowid()
            }
            Some(id) => {
                self.conn
                    .execute(
                        "UPDATE goals SET name = ?, icon = ?, target_cents = ?, deadline = ? WHERE id = ?",
                        params![name, icon, target, deadline, id],
                    )
                    .e()?;
                id
            }
        };
        self.touch();
        Ok(rid)
    }

    /// Aportar (o retirar, con importe negativo).
    pub fn goal_add(&mut self, id: i64, delta: i64) -> R<()> {
        if delta == 0 {
            return err("Importe no válido");
        }
        let saved: i64 = self.conn.query_row("SELECT saved_cents FROM goals WHERE id = ?", [id], |r| r.get(0)).e()?;
        if saved + delta < 0 {
            return err("No puedes retirar más de lo ahorrado");
        }
        self.conn.execute("UPDATE goals SET saved_cents = ? WHERE id = ?", params![saved + delta, id]).e()?;
        self.touch();
        Ok(())
    }

    pub fn delete_goal(&mut self, id: i64) -> R<()> {
        self.conn.execute("DELETE FROM goals WHERE id = ?", [id]).e()?;
        self.touch();
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn test_core() -> Core {
    Core::new(crate::db::new_memory().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alta_y_filtros() {
        let mut c = test_core();
        let cat = c.resolve_category("Alimentación", "gasto");
        let id = c
            .create_tx(&TxInput {
                ty: "gasto".into(),
                amount_cents: 4218,
                category_id: cat,
                note: "Supermercado".into(),
                account: "Banco".into(),
                date: "2026-11-24".into(),
                tags: "#casa, casa super".into(),
                source: String::new(),
            })
            .unwrap();
        let tx = c.transaction(id).unwrap();
        assert_eq!(tx.tags, "casa,super");
        assert_eq!(tx.category_name, "Alimentación");
        let l = c.transactions(&TxFilter { q: Some("#casa".into()), ..Default::default() });
        assert_eq!(l.total_count, 1);
        assert_eq!(l.gastos_cents, 4218);
        // Categoría de otro tipo → cae en la de reserva.
        let id2 = c
            .create_tx(&TxInput { ty: "ingreso".into(), amount_cents: 100, category_id: cat, ..Default::default() })
            .unwrap();
        assert_eq!(c.transaction(id2).unwrap().category_name, "Otros ingresos");
    }

    #[test]
    fn reserva_protegida() {
        let mut c = test_core();
        let otros = c.resolve_category("Otros", "gasto").unwrap();
        assert!(c.delete_category(otros).is_err());
        assert!(c.save_category(Some(otros), "Varios", "gasto", "📦", "#898781", 0, false).is_err());
    }
}
