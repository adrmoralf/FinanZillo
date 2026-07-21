'use strict';

const { DatabaseSync } = require('node:sqlite');
const config = require('./config');

const db = new DatabaseSync(config.DB_FILE);

db.exec('PRAGMA journal_mode = WAL;');
db.exec('PRAGMA foreign_keys = ON;');

db.exec(`
CREATE TABLE IF NOT EXISTS categories (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  name     TEXT NOT NULL COLLATE NOCASE,
  type     TEXT NOT NULL CHECK (type IN ('gasto','ingreso')),
  icon     TEXT NOT NULL DEFAULT '📦',
  color    TEXT NOT NULL DEFAULT '#898781',
  archived INTEGER NOT NULL DEFAULT 0,
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
  created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_tx_date ON transactions(date);
CREATE INDEX IF NOT EXISTS idx_tx_category ON transactions(category_id);

CREATE TABLE IF NOT EXISTS recurring (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  type         TEXT NOT NULL CHECK (type IN ('gasto','ingreso')),
  amount_cents INTEGER NOT NULL CHECK (amount_cents > 0),
  category_id  INTEGER REFERENCES categories(id),
  note         TEXT NOT NULL DEFAULT '',
  day_of_month INTEGER NOT NULL DEFAULT 1 CHECK (day_of_month BETWEEN 1 AND 31),
  active       INTEGER NOT NULL DEFAULT 1,
  start_month  TEXT NOT NULL,
  last_applied TEXT
);

CREATE TABLE IF NOT EXISTS account_balances (
  name          TEXT PRIMARY KEY COLLATE NOCASE,
  opening_cents INTEGER NOT NULL,
  opening_date  TEXT NOT NULL
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
  created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS app_settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS networth_snapshots (
  date              TEXT PRIMARY KEY,
  investments_cents INTEGER NOT NULL
);
`);

// Migraciones sobre bases de datos anteriores.
const categoryColumns = db.prepare('PRAGMA table_info(categories)').all().map((c) => c.name);
if (!categoryColumns.includes('budget_cents')) {
  db.exec('ALTER TABLE categories ADD COLUMN budget_cents INTEGER NOT NULL DEFAULT 0');
}
const txColumns = db.prepare('PRAGMA table_info(transactions)').all().map((c) => c.name);
if (!txColumns.includes('account')) {
  db.exec("ALTER TABLE transactions ADD COLUMN account TEXT NOT NULL DEFAULT ''");
}
if (!categoryColumns.includes('fixed')) {
  db.exec('ALTER TABLE categories ADD COLUMN fixed INTEGER NOT NULL DEFAULT 0');
}
const accountColumns = db.prepare('PRAGMA table_info(account_balances)').all().map((c) => c.name);
if (!accountColumns.includes('apy')) {
  // TAE en % (cuentas remuneradas tipo Revolut); 0 = sin remuneración.
  db.exec('ALTER TABLE account_balances ADD COLUMN apy REAL NOT NULL DEFAULT 0');
}
if (!txColumns.includes('tags')) {
  // Etiquetas libres separadas por comas ("vacaciones2026,reembolsable").
  db.exec("ALTER TABLE transactions ADD COLUMN tags TEXT NOT NULL DEFAULT ''");
}
const goalColumns = db.prepare('PRAGMA table_info(goals)').all().map((c) => c.name);
if (!goalColumns.includes('deadline')) {
  // Fecha límite opcional del objetivo (YYYY-MM-DD) para sugerir la cuota mensual.
  db.exec('ALTER TABLE goals ADD COLUMN deadline TEXT');
}

// Plantillas rápidas de movimiento (favoritos de un toque).
db.exec(`
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
`);

// Historial de presupuestos: cada cambio guarda el valor vigente desde ese mes
// (category_id = 0 es el presupuesto global). El presupuesto efectivo de un mes
// es la última entrada con month <= mes; sin entradas, el valor actual.
db.exec(`
CREATE TABLE IF NOT EXISTS budget_history (
  category_id  INTEGER NOT NULL,
  month        TEXT NOT NULL,
  budget_cents INTEGER NOT NULL,
  PRIMARY KEY (category_id, month)
);
`);

// Historial de operaciones de inversión (compras/ventas parciales). Las
// unidades y el coste de investments se recalculan a partir de estas filas.
const hadOpsTable = Boolean(
  db.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'investment_ops'").get()
);
db.exec(`
CREATE TABLE IF NOT EXISTS investment_ops (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  investment_id INTEGER NOT NULL REFERENCES investments(id) ON DELETE CASCADE,
  type          TEXT NOT NULL CHECK (type IN ('compra','venta')),
  units         REAL NOT NULL CHECK (units > 0),
  amount_cents  INTEGER NOT NULL CHECK (amount_cents >= 0),
  date          TEXT NOT NULL,
  created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
`);
if (!hadOpsTable) {
  // Seed: la posición actual de cada inversión pasa a ser su compra inicial.
  const invs = db.prepare('SELECT id, units, cost_cents, created_at FROM investments').all();
  const ins = db.prepare(
    "INSERT INTO investment_ops (investment_id, type, units, amount_cents, date) VALUES (?, 'compra', ?, ?, ?)"
  );
  for (const inv of invs) {
    if (inv.units > 0) ins.run(inv.id, inv.units, inv.cost_cents, String(inv.created_at).slice(0, 10));
  }
}

// Foto diaria del precio de cada inversión (para las mini-gráficas de 30 días).
db.exec(`
CREATE TABLE IF NOT EXISTS investment_prices (
  investment_id INTEGER NOT NULL REFERENCES investments(id) ON DELETE CASCADE,
  date          TEXT NOT NULL,
  price         REAL NOT NULL,
  PRIMARY KEY (investment_id, date)
);
`);

const recurringColumns = db.prepare('PRAGMA table_info(recurring)').all().map((c) => c.name);
if (!recurringColumns.includes('kind')) {
  // Tipos de recurrente: 'movimiento' (gasto/ingreso), 'traspaso' (entre
  // cuentas) y 'aportacion' (a una inversión existente).
  db.exec("ALTER TABLE recurring ADD COLUMN kind TEXT NOT NULL DEFAULT 'movimiento'");
  db.exec("ALTER TABLE recurring ADD COLUMN from_account TEXT NOT NULL DEFAULT ''");
  db.exec("ALTER TABLE recurring ADD COLUMN to_account TEXT NOT NULL DEFAULT ''");
  db.exec('ALTER TABLE recurring ADD COLUMN investment_id INTEGER');
}

// Categorías iniciales — alineadas con las que el usuario ya montó en su atajo de iPhone.
// Colores de la paleta categórica validada (modo claro); el frontend los traduce
// a su variante oscura cuando aplica el tema oscuro.
const SEED_CATEGORIES = [
  ['Alimentación', 'gasto', '🛒', '#2a78d6'],
  ['Restaurante', 'gasto', '🍽️', '#1baf7a'],
  ['Transporte', 'gasto', '🚗', '#eda100'],
  ['Vivienda', 'gasto', '🏠', '#008300'],
  ['Ocio', 'gasto', '🎬', '#4a3aa7'],
  ['Salud', 'gasto', '💊', '#e34948'],
  ['Ropa', 'gasto', '👕', '#e87ba4'],
  ['Suscripciones', 'gasto', '📺', '#eb6834'],
  ['Viajes', 'gasto', '✈️', '#1c5cab'],
  ['Regalos', 'gasto', '🎁', '#c98500'],
  ['Tonterías', 'gasto', '🎯', '#e87ba4'],
  ['Desarrollo personal', 'gasto', '📚', '#4a3aa7'],
  ['Inversiones', 'gasto', '📈', '#008300'],
  ['Otros', 'gasto', '📦', '#898781'],
  ['Nómina', 'ingreso', '💼', '#2a78d6'],
  ['Extra', 'ingreso', '➕', '#1baf7a'],
  ['Intereses', 'ingreso', '🏦', '#4a3aa7'],
  ['Otros ingresos', 'ingreso', '💶', '#898781'],
];

const countCategories = db.prepare('SELECT COUNT(*) AS n FROM categories').get();
if (countCategories.n === 0) {
  const insert = db.prepare(
    'INSERT INTO categories (name, type, icon, color) VALUES (?, ?, ?, ?)'
  );
  for (const row of SEED_CATEGORIES) insert.run(...row);
}

// Copia de seguridad diaria (local + carpeta externa opcional, p. ej. OneDrive);
// se conservan las 14 más recientes en cada destino. Se llama al arrancar y
// periódicamente desde server.js por si el proceso vive varios días seguidos.
function runDailyBackup() {
  const fs = require('fs');
  const path = require('path');
  const pad = (n) => String(n).padStart(2, '0');
  const d = new Date();
  const stamp = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;

  const dirs = [path.join(config.DATA_DIR, 'backups')];
  if (config.EXTERNAL_BACKUP_DIR) dirs.push(config.EXTERNAL_BACKUP_DIR);

  let checkpointed = false;
  for (const dir of dirs) {
    try {
      fs.mkdirSync(dir, { recursive: true });
      const target = path.join(dir, `finanzillo-${stamp}.db`);
      if (fs.existsSync(target)) continue;
      if (!checkpointed) {
        db.exec('PRAGMA wal_checkpoint(TRUNCATE);');
        checkpointed = true;
      }
      fs.copyFileSync(config.DB_FILE, target);
      const old = fs.readdirSync(dir).filter((f) => /^finanzillo-\d{4}-\d{2}-\d{2}\.db$/.test(f)).sort();
      for (const f of old.slice(0, -14)) fs.unlinkSync(path.join(dir, f));
    } catch (e) {
      console.error(`[FinanZillo] No se pudo crear la copia de seguridad en ${dir}:`, e.message);
    }
  }
}

runDailyBackup();

module.exports = db;
module.exports.runDailyBackup = runDailyBackup;
