//! Esquema SQLite (el mismo que la app web, para poder importar su base sin
//! transformarla), migraciones y paso de memoria ↔ bytes.

use std::path::Path;

use rusqlite::{Connection, MAIN_DB, OpenFlags};

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS categories (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  name     TEXT NOT NULL COLLATE NOCASE,
  type     TEXT NOT NULL CHECK (type IN ('gasto','ingreso')),
  icon     TEXT NOT NULL DEFAULT '📦',
  color    TEXT NOT NULL DEFAULT '#898781',
  archived INTEGER NOT NULL DEFAULT 0,
  budget_cents INTEGER NOT NULL DEFAULT 0,
  fixed    INTEGER NOT NULL DEFAULT 0,
  UNIQUE (name, type)
);
CREATE TABLE IF NOT EXISTS transactions (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  type         TEXT NOT NULL CHECK (type IN ('gasto','ingreso')),
  amount_cents INTEGER NOT NULL CHECK (amount_cents > 0),
  category_id  INTEGER REFERENCES categories(id),
  note         TEXT NOT NULL DEFAULT '',
  account      TEXT NOT NULL DEFAULT '',
  date         TEXT NOT NULL,
  source       TEXT NOT NULL DEFAULT 'web',
  created_at   TEXT NOT NULL DEFAULT (datetime('now')),
  tags         TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_tx_date ON transactions(date);
CREATE INDEX IF NOT EXISTS idx_tx_category ON transactions(category_id);
CREATE TABLE IF NOT EXISTS recurring (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  type         TEXT NOT NULL CHECK (type IN ('gasto','ingreso')),
  amount_cents INTEGER NOT NULL CHECK (amount_cents > 0),
  category_id  INTEGER REFERENCES categories(id),
  note         TEXT NOT NULL DEFAULT '',
  account      TEXT NOT NULL DEFAULT '',
  day_of_month INTEGER NOT NULL DEFAULT 1 CHECK (day_of_month BETWEEN 1 AND 31),
  active       INTEGER NOT NULL DEFAULT 1,
  start_month  TEXT NOT NULL,
  last_applied TEXT,
  kind         TEXT NOT NULL DEFAULT 'movimiento',
  from_account TEXT NOT NULL DEFAULT '',
  to_account   TEXT NOT NULL DEFAULT '',
  investment_id INTEGER
);
CREATE TABLE IF NOT EXISTS account_balances (
  name          TEXT PRIMARY KEY COLLATE NOCASE,
  opening_cents INTEGER NOT NULL,
  opening_date  TEXT NOT NULL,
  apy           REAL NOT NULL DEFAULT 0,
  sort_order    INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS transfers (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  amount_cents INTEGER NOT NULL CHECK (amount_cents > 0),
  from_account TEXT NOT NULL,
  to_account   TEXT NOT NULL,
  note         TEXT NOT NULL DEFAULT '',
  date         TEXT NOT NULL,
  created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS investments (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  name         TEXT NOT NULL,
  provider     TEXT NOT NULL CHECK (provider IN ('yahoo','coingecko','manual')),
  symbol       TEXT NOT NULL DEFAULT '',
  units        REAL NOT NULL CHECK (units > 0),
  cost_cents   INTEGER NOT NULL DEFAULT 0,
  manual_price REAL,
  archived     INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS price_cache (
  key        TEXT PRIMARY KEY,
  price      REAL NOT NULL,
  fetched_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS goals (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  name         TEXT NOT NULL,
  icon         TEXT NOT NULL DEFAULT '🎯',
  target_cents INTEGER NOT NULL CHECK (target_cents > 0),
  saved_cents  INTEGER NOT NULL DEFAULT 0,
  archived     INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL DEFAULT (datetime('now')),
  deadline     TEXT
);
CREATE TABLE IF NOT EXISTS app_settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS networth_snapshots (
  date              TEXT PRIMARY KEY,
  investments_cents INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS templates (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  name         TEXT NOT NULL,
  type         TEXT NOT NULL CHECK (type IN ('gasto','ingreso')),
  amount_cents INTEGER NOT NULL CHECK (amount_cents > 0),
  category_id  INTEGER REFERENCES categories(id),
  note         TEXT NOT NULL DEFAULT '',
  account      TEXT NOT NULL DEFAULT '',
  created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS budget_history (
  category_id  INTEGER NOT NULL,
  month        TEXT NOT NULL,
  budget_cents INTEGER NOT NULL,
  PRIMARY KEY (category_id, month)
);
CREATE TABLE IF NOT EXISTS investment_ops (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  investment_id INTEGER NOT NULL REFERENCES investments(id) ON DELETE CASCADE,
  type          TEXT NOT NULL CHECK (type IN ('compra','venta')),
  units         REAL NOT NULL CHECK (units > 0),
  amount_cents  INTEGER NOT NULL CHECK (amount_cents >= 0),
  date          TEXT NOT NULL,
  created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS investment_prices (
  investment_id INTEGER NOT NULL REFERENCES investments(id) ON DELETE CASCADE,
  date          TEXT NOT NULL,
  price         REAL NOT NULL,
  PRIMARY KEY (investment_id, date)
);
CREATE TABLE IF NOT EXISTS interest_postings (
  account        TEXT NOT NULL COLLATE NOCASE,
  month          TEXT NOT NULL,
  transaction_id INTEGER,
  amount_cents   INTEGER NOT NULL,
  created_at     TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (account, month)
);
"#;

/// Categorías iniciales de una base nueva.
const SEED_CATEGORIES: &[(&str, &str, &str, &str)] = &[
    ("Alimentación", "gasto", "🛒", "#2a78d6"),
    ("Restaurante", "gasto", "🍽️", "#1baf7a"),
    ("Transporte", "gasto", "🚗", "#eda100"),
    ("Vivienda", "gasto", "🏠", "#008300"),
    ("Ocio", "gasto", "🎬", "#4a3aa7"),
    ("Salud", "gasto", "💊", "#e34948"),
    ("Ropa", "gasto", "👕", "#e87ba4"),
    ("Suscripciones", "gasto", "📺", "#eb6834"),
    ("Viajes", "gasto", "✈️", "#1c5cab"),
    ("Regalos", "gasto", "🎁", "#c98500"),
    ("Caprichos", "gasto", "🎯", "#e87ba4"),
    ("Desarrollo personal", "gasto", "📚", "#4a3aa7"),
    ("Inversiones", "gasto", "📈", "#008300"),
    ("Otros", "gasto", "📦", "#898781"),
    ("Nómina", "ingreso", "💼", "#2a78d6"),
    ("Extra", "ingreso", "➕", "#1baf7a"),
    ("Intereses", "ingreso", "🏦", "#4a3aa7"),
    ("Otros ingresos", "ingreso", "💶", "#898781"),
];

fn columns(conn: &Connection, table: &str) -> Vec<String> {
    let mut st = match conn.prepare(&format!("PRAGMA table_info({table})")) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    st.query_map([], |r| r.get::<_, String>(1)).map(|it| it.filter_map(Result::ok).collect()).unwrap_or_default()
}

fn add_column(conn: &Connection, table: &str, col: &str, ddl: &str) -> rusqlite::Result<()> {
    if !columns(conn, table).iter().any(|c| c == col) {
        conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {ddl}"))?;
    }
    Ok(())
}

/// Crea lo que falte y migra bases antiguas (p. ej. una importada de la web).
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    // Columnas añadidas con el tiempo en la app web: si la base es antigua,
    // se añaden antes de que CREATE TABLE IF NOT EXISTS las dé por hechas.
    if !columns(conn, "categories").is_empty() {
        add_column(conn, "categories", "budget_cents", "budget_cents INTEGER NOT NULL DEFAULT 0")?;
        add_column(conn, "categories", "fixed", "fixed INTEGER NOT NULL DEFAULT 0")?;
    }
    if !columns(conn, "transactions").is_empty() {
        add_column(conn, "transactions", "account", "account TEXT NOT NULL DEFAULT ''")?;
        add_column(conn, "transactions", "tags", "tags TEXT NOT NULL DEFAULT ''")?;
    }
    if !columns(conn, "account_balances").is_empty() {
        add_column(conn, "account_balances", "apy", "apy REAL NOT NULL DEFAULT 0")?;
        add_column(conn, "account_balances", "sort_order", "sort_order INTEGER NOT NULL DEFAULT 0")?;
    }
    if !columns(conn, "goals").is_empty() {
        add_column(conn, "goals", "deadline", "deadline TEXT")?;
    }
    if !columns(conn, "recurring").is_empty() {
        add_column(conn, "recurring", "kind", "kind TEXT NOT NULL DEFAULT 'movimiento'")?;
        add_column(conn, "recurring", "from_account", "from_account TEXT NOT NULL DEFAULT ''")?;
        add_column(conn, "recurring", "to_account", "to_account TEXT NOT NULL DEFAULT ''")?;
        add_column(conn, "recurring", "investment_id", "investment_id INTEGER")?;
        add_column(conn, "recurring", "account", "account TEXT NOT NULL DEFAULT ''")?;
    }
    conn.execute_batch(SCHEMA)?;

    let n: i64 = conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0))?;
    if n == 0 {
        let mut st = conn.prepare("INSERT INTO categories (name, type, icon, color) VALUES (?, ?, ?, ?)")?;
        for (name, ty, icon, color) in SEED_CATEGORIES {
            st.execute((name, ty, icon, color))?;
        }
    }
    Ok(())
}

/// Base nueva en memoria.
pub fn new_memory() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    migrate(&conn)?;
    Ok(conn)
}

/// Bytes 18 y 19 de la cabecera SQLite: 2 = modo WAL. Una base importada de la
/// web (que usa WAL) arrastra esa marca, y SQLite se niega a deserializar en
/// memoria una base marcada como WAL. Se normaliza a modo clásico (1).
fn normalize_header(bytes: &mut [u8]) {
    if bytes.len() > 19 && bytes.starts_with(b"SQLite format 3\0") {
        for i in [18, 19] {
            if bytes[i] == 2 {
                bytes[i] = 1;
            }
        }
    }
}

/// Carga en memoria una base serializada (el contenido descifrado del .fzl).
pub fn from_bytes(bytes: &[u8]) -> rusqlite::Result<Connection> {
    let mut fixed = zeroize::Zeroizing::new(bytes.to_vec());
    normalize_header(&mut fixed);
    let mut conn = Connection::open_in_memory()?;
    conn.deserialize_read_exact(MAIN_DB, &fixed[..], fixed.len(), false)?;
    migrate(&conn)?;
    Ok(conn)
}

/// Serializa la base en memoria a bytes (para cifrarla y guardarla).
pub fn to_bytes(conn: &Connection) -> rusqlite::Result<Vec<u8>> {
    let data = conn.serialize(MAIN_DB)?;
    let mut v = data.to_vec();
    normalize_header(&mut v);
    Ok(v)
}

/// Copia una base SQLite del disco (p. ej. la de la app web) a memoria, sin modificarla.
pub fn import_sqlite_file(path: &Path) -> Result<Connection, String> {
    let src = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| format!("No se pudo abrir {}: {e}", path.display()))?;
    let ok: String = src
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|e| format!("No parece una base de FinanZillo: {e}"))?;
    if ok != "ok" {
        return Err(format!("La base de datos tiene errores: {ok}"));
    }
    let has_tx: i64 = src
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'transactions'", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if has_tx == 0 {
        return Err("Ese archivo no es una base de datos de FinanZillo".into());
    }
    let mut dst = Connection::open_in_memory().map_err(|e| e.to_string())?;
    {
        let backup = rusqlite::backup::Backup::new(&src, &mut dst).map_err(|e| e.to_string())?;
        backup.run_to_completion(256, std::time::Duration::ZERO, None).map_err(|e| e.to_string())?;
    }
    // La base de la web usa WAL; en memoria no aplica.
    dst.execute_batch("PRAGMA journal_mode = MEMORY;").ok();
    migrate(&dst).map_err(|e| e.to_string())?;
    Ok(dst)
}

/// Base SQLite de FinanZillo para importar al crear el archivo: solo si se
/// indica con `FINANZILLO_IMPORT` (pruebas, demo). La app no busca en
/// ninguna carpeta: quien la instala nueva empieza de cero.
pub fn guess_web_db() -> Option<std::path::PathBuf> {
    std::env::var("FINANZILLO_IMPORT").ok().map(std::path::PathBuf::from).filter(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializa_y_recupera() {
        let c = new_memory().unwrap();
        c.execute(
            "INSERT INTO transactions (type, amount_cents, category_id, note, account, date) VALUES ('gasto', 1250, 1, 'café', 'Efectivo', '2026-11-24')",
            [],
        )
        .unwrap();
        let bytes = to_bytes(&c).unwrap();
        let c2 = from_bytes(&bytes).unwrap();
        let n: i64 = c2.query_row("SELECT amount_cents FROM transactions", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1250);
        let cats: i64 = c2.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0)).unwrap();
        assert_eq!(cats, 18);
    }

    /// Caso real: la base de la web está en modo WAL. Importarla, guardarla y
    /// volver a abrirla tiene que funcionar.
    #[test]
    fn importa_base_wal() {
        let dir = std::env::temp_dir().join(format!("fz-wal-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("web.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch("PRAGMA journal_mode = WAL;").unwrap();
            migrate(&c).unwrap();
            c.execute("INSERT INTO transactions (type, amount_cents, date) VALUES ('gasto', 4218, '2026-11-24')", []).unwrap();
        }
        let imported = import_sqlite_file(&path).unwrap();
        let bytes = to_bytes(&imported).unwrap();
        assert_eq!(bytes[18], 1);
        let reopened = from_bytes(&bytes).unwrap();
        let n: i64 = reopened.query_row("SELECT amount_cents FROM transactions", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 4218);
        // Un archivo guardado antes del arreglo (cabecera WAL) también abre.
        let mut old = bytes.clone();
        old[18] = 2;
        old[19] = 2;
        assert!(from_bytes(&old).is_ok());
        drop(imported);
        drop(reopened);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
