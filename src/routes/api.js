'use strict';

const express = require('express');
const db = require('../db');
const config = require('../config');
const auth = require('../auth');
const { applyRecurring, currentMonth } = require('../recurring');
const { getPriceEur, searchSymbols } = require('../prices');

const router = express.Router();

// ---------- utilidades ----------

function badRequest(res, message) {
  return res.status(400).json({ error: message });
}

// Acepta 12.5, "12,50", "1.234,56", "1234.56", "12€"...
function parseAmountCents(value) {
  if (typeof value === 'number' && isFinite(value)) {
    return Math.round(value * 100);
  }
  if (typeof value !== 'string') return NaN;
  let s = value.replace(/[€\s]/g, '').trim();
  if (!s) return NaN;
  if (s.includes(',')) {
    s = s.replace(/\./g, '').replace(',', '.');
  }
  const f = Number(s);
  return isFinite(f) ? Math.round(f * 100) : NaN;
}

function todayLocal() {
  const d = new Date();
  const pad = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function isValidDate(s) {
  return typeof s === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(s) && !isNaN(Date.parse(s));
}

// El icono es texto plano (emoji): fuera HTML y longitud limitada.
function sanitizeIcon(value, fallback = '📦') {
  const s = String(value || '').replace(/[<>&"'`]/g, '').trim().slice(0, 8);
  return s || fallback;
}

function sanitizeColor(value, fallback = '#898781') {
  const s = String(value || '').trim();
  return /^#[0-9a-fA-F]{6}$/.test(s) ? s.toLowerCase() : fallback;
}

// Presupuesto mensual: 0 = sin presupuesto.
function parseBudgetCents(value) {
  if (value == null || value === '' || value === 0 || value === '0') return 0;
  const cents = parseAmountCents(value);
  return Number.isInteger(cents) && cents > 0 ? cents : NaN;
}

function normalizeType(value) {
  const t = String(value || '').trim().toLowerCase();
  if (t === 'gasto' || t === 'expense') return 'gasto';
  if (t === 'ingreso' || t === 'income') return 'ingreso';
  return null;
}

// Los campos de movimiento aceptan alias en español (los que usa el atajo de
// iPhone del usuario: tipo/importe/categoria/descripcion/cuenta) además de los
// nombres en inglés de la API, para no obligar a reconstruir atajos ya montados.
function readType(body) {
  return body.type != null ? body.type : body.tipo;
}
function readAmount(body) {
  return body.amount != null ? body.amount : body.importe;
}
function readNote(body) {
  const v = body.note != null ? body.note : body.descripcion;
  return String(v || '').trim();
}
function readAccount(body) {
  const v = body.account != null ? body.account : body.cuenta;
  return String(v || '').trim().slice(0, 60);
}

// Etiquetas libres: admite string ("a, b" o "#a #b") o array; se guardan
// normalizadas separadas por comas, sin '#' y sin duplicados.
function readTags(body) {
  let raw = body.tags != null ? body.tags : body.etiquetas;
  if (raw == null) return '';
  if (Array.isArray(raw)) raw = raw.join(',');
  const tags = String(raw)
    .split(/[,\s]+/)
    .map((t) => t.replace(/^#/, '').trim().slice(0, 30))
    .filter(Boolean);
  return [...new Set(tags)].slice(0, 10).join(',');
}

// ---------- historial de presupuestos ----------
//
// Cada cambio de presupuesto (por categoría, o global con category_id = 0)
// registra el valor vigente desde el mes del cambio. El presupuesto efectivo
// de un mes pasado es la última entrada con month <= mes.

function recordBudgetHistory(categoryId, cents) {
  db.prepare(
    'INSERT INTO budget_history (category_id, month, budget_cents) VALUES (?, ?, ?) ON CONFLICT(category_id, month) DO UPDATE SET budget_cents = excluded.budget_cents'
  ).run(categoryId, currentMonth(), cents);
}

function budgetForMonth(categoryId, month, fallback) {
  const row = db
    .prepare('SELECT budget_cents FROM budget_history WHERE category_id = ? AND month <= ? ORDER BY month DESC LIMIT 1')
    .get(categoryId, month);
  return row ? row.budget_cents : fallback;
}

// Resuelve la categoría por id o por nombre; si no existe, cae en "Otros"
// (u "Otros ingresos") para que el atajo del iPhone nunca falle por categoría.
function resolveCategoryId(body, type) {
  if (body.category_id != null) {
    const cat = db
      .prepare('SELECT id FROM categories WHERE id = ? AND type = ?')
      .get(Number(body.category_id), type);
    return cat ? cat.id : null;
  }
  const name = String(body.category || body.categoria || '').trim();
  if (name) {
    const cat = db
      .prepare('SELECT id FROM categories WHERE name = ? COLLATE NOCASE AND type = ?')
      .get(name, type);
    if (cat) return cat.id;
  }
  const fallbackName = type === 'gasto' ? 'Otros' : 'Otros ingresos';
  const fallback = db
    .prepare('SELECT id FROM categories WHERE name = ? COLLATE NOCASE AND type = ?')
    .get(fallbackName, type);
  return fallback ? fallback.id : null;
}

const TX_SELECT = `
  SELECT t.id, t.type, t.amount_cents, t.category_id, t.note, t.account, t.date, t.source, t.tags, t.created_at,
         c.name AS category_name, c.icon AS category_icon, c.color AS category_color
  FROM transactions t
  LEFT JOIN categories c ON c.id = t.category_id
`;

// ---------- sesión ----------

router.post('/login', (req, res) => {
  const ip = req.ip || 'unknown';
  if (!auth.loginAllowed(ip)) {
    return res.status(429).json({ error: 'Demasiados intentos. Espera 15 minutos.' });
  }
  const password = String((req.body && req.body.password) || '');
  if (!auth.checkPassword(password)) {
    auth.registerFailure(ip);
    return res.status(401).json({ error: 'Contraseña incorrecta' });
  }
  auth.createSessionCookie(res);
  res.json({ ok: true });
});

router.post('/logout', (req, res) => {
  auth.clearSessionCookie(res);
  res.json({ ok: true });
});

router.get('/me', (req, res) => {
  res.json({ authenticated: auth.hasValidSession(req) });
});

// Todo lo que sigue requiere sesión web o token de API.
router.use(auth.requireAuth);

router.get('/ping', (req, res) => res.json({ ok: true, app: 'FinanZillo' }));

// Datos que la pantalla de Ajustes necesita para configurar el atajo.
router.get('/settings', (req, res) => {
  const row = db.prepare("SELECT value FROM app_settings WHERE key = 'global_budget_cents'").get();
  const acc = db.prepare("SELECT value FROM app_settings WHERE key = 'default_account'").get();
  res.json({
    api_token: config.API_TOKEN,
    port: config.PORT,
    global_budget_cents: row ? Number(row.value) : 0,
    default_account: acc ? acc.value : '',
  });
});

router.put('/settings', (req, res) => {
  const body = req.body || {};
  if (body.global_budget !== undefined) {
    const cents = parseBudgetCents(body.global_budget);
    if (Number.isNaN(cents)) return badRequest(res, 'Presupuesto no válido');
    db.prepare(
      "INSERT INTO app_settings (key, value) VALUES ('global_budget_cents', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value"
    ).run(String(cents));
    recordBudgetHistory(0, cents);
  }
  // Cuenta preseleccionada al crear un movimiento nuevo; siempre hay una.
  if (body.default_account !== undefined) {
    const name = String(body.default_account).trim().slice(0, 60);
    if (!name) return badRequest(res, 'Elige una cuenta');
    db.prepare(
      "INSERT INTO app_settings (key, value) VALUES ('default_account', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value"
    ).run(name);
  }
  const row = db.prepare("SELECT value FROM app_settings WHERE key = 'global_budget_cents'").get();
  const acc = db.prepare("SELECT value FROM app_settings WHERE key = 'default_account'").get();
  res.json({
    ok: true,
    global_budget_cents: row ? Number(row.value) : 0,
    default_account: acc ? acc.value : '',
  });
});

// ---------- categorías ----------

router.get('/categories', (req, res) => {
  const includeArchived = req.query.all === '1';
  const rows = db
    .prepare(
      `SELECT c.*, (SELECT COUNT(*) FROM transactions t WHERE t.category_id = c.id) AS usage_count
       FROM categories c ${includeArchived ? '' : 'WHERE c.archived = 0'}
       ORDER BY c.type, c.name`
    )
    .all();
  res.json(rows);
});

router.post('/categories', (req, res) => {
  const name = String((req.body && req.body.name) || '').trim();
  const type = normalizeType(req.body && req.body.type);
  if (!name) return badRequest(res, 'Falta el nombre');
  if (!type) return badRequest(res, "El tipo debe ser 'gasto' o 'ingreso'");
  const icon = sanitizeIcon(req.body && req.body.icon);
  const color = sanitizeColor(req.body && req.body.color);
  const budget = parseBudgetCents(req.body && req.body.budget);
  if (Number.isNaN(budget)) return badRequest(res, 'Presupuesto no válido');
  const fixed = req.body && req.body.fixed ? 1 : 0;
  try {
    const info = db
      .prepare('INSERT INTO categories (name, type, icon, color, budget_cents, fixed) VALUES (?, ?, ?, ?, ?, ?)')
      .run(name, type, icon, color, type === 'gasto' ? budget : 0, type === 'gasto' ? fixed : 0);
    if (type === 'gasto' && budget > 0) recordBudgetHistory(Number(info.lastInsertRowid), budget);
    res.status(201).json(db.prepare('SELECT * FROM categories WHERE id = ?').get(info.lastInsertRowid));
  } catch (e) {
    badRequest(res, 'Ya existe una categoría con ese nombre y tipo');
  }
});

router.put('/categories/:id', (req, res) => {
  const id = Number(req.params.id);
  const existing = db.prepare('SELECT * FROM categories WHERE id = ?').get(id);
  if (!existing) return res.status(404).json({ error: 'Categoría no encontrada' });
  const body = req.body || {};
  const name = body.name != null ? String(body.name).trim() : existing.name;
  const icon = body.icon != null ? sanitizeIcon(body.icon, existing.icon) : existing.icon;
  const color = body.color != null ? sanitizeColor(body.color, existing.color) : existing.color;
  const archived = body.archived != null ? (body.archived ? 1 : 0) : existing.archived;
  let budget = existing.budget_cents;
  if (body.budget !== undefined) {
    budget = parseBudgetCents(body.budget);
    if (Number.isNaN(budget)) return badRequest(res, 'Presupuesto no válido');
    if (existing.type !== 'gasto') budget = 0;
  }
  const fixed = body.fixed != null ? (body.fixed && existing.type === 'gasto' ? 1 : 0) : existing.fixed;
  if (!name) return badRequest(res, 'Falta el nombre');
  try {
    db.prepare('UPDATE categories SET name = ?, icon = ?, color = ?, archived = ?, budget_cents = ?, fixed = ? WHERE id = ?')
      .run(name, icon, color, archived, budget, fixed, id);
    if (body.budget !== undefined && budget !== existing.budget_cents) recordBudgetHistory(id, budget);
    res.json(db.prepare('SELECT * FROM categories WHERE id = ?').get(id));
  } catch (e) {
    badRequest(res, 'Ya existe una categoría con ese nombre y tipo');
  }
});

// Si la categoría tiene movimientos se archiva (para no perder histórico);
// si no tiene ninguno, se borra de verdad.
router.delete('/categories/:id', (req, res) => {
  const id = Number(req.params.id);
  const existing = db.prepare('SELECT * FROM categories WHERE id = ?').get(id);
  if (!existing) return res.status(404).json({ error: 'Categoría no encontrada' });
  const used = db
    .prepare('SELECT COUNT(*) AS n FROM transactions WHERE category_id = ?')
    .get(id).n;
  if (used > 0) {
    db.prepare('UPDATE categories SET archived = 1 WHERE id = ?').run(id);
    return res.json({ archived: true, deleted: false });
  }
  db.prepare('DELETE FROM categories WHERE id = ?').run(id);
  res.json({ archived: false, deleted: true });
});

// ---------- recurrentes ----------

const RECURRING_SELECT = `
  SELECT r.*, c.name AS category_name, c.icon AS category_icon, c.color AS category_color,
         i.name AS investment_name
  FROM recurring r
  LEFT JOIN categories c ON c.id = r.category_id
  LEFT JOIN investments i ON i.id = r.investment_id
`;

router.get('/recurring', (req, res) => {
  res.json(db.prepare(`${RECURRING_SELECT} ORDER BY r.active DESC, r.day_of_month, r.id`).all());
});

// Valida el cuerpo de una regla recurrente según su tipo:
// 'movimiento' (gasto/ingreso), 'traspaso' (entre cuentas), 'aportacion' (a inversión).
function validateRecurringBody(body, existing = null) {
  const kind = body.kind != null ? String(body.kind) : (existing && existing.kind) || 'movimiento';
  if (!['movimiento', 'traspaso', 'aportacion'].includes(kind)) {
    return { error: "El tipo debe ser 'movimiento', 'traspaso' o 'aportacion'" };
  }
  const amountCents = body.amount != null ? parseAmountCents(body.amount) : existing && existing.amount_cents;
  if (!Number.isInteger(amountCents) || amountCents <= 0) return { error: 'Importe no válido' };
  const day = body.day_of_month != null ? Number(body.day_of_month) : (existing && existing.day_of_month) || 1;
  if (!Number.isInteger(day) || day < 1 || day > 31) return { error: 'El día debe estar entre 1 y 31' };
  const note = body.note != null ? String(body.note).trim() : (existing && existing.note) || '';

  const r = { kind, amountCents, day, note, type: 'gasto', categoryId: null, from: '', to: '', investmentId: null };

  if (kind === 'movimiento') {
    const type = body.type != null ? normalizeType(body.type) : existing && existing.type;
    if (!type) return { error: "El tipo debe ser 'gasto' o 'ingreso'" };
    r.type = type;
    r.categoryId = body.category_id != null || body.category != null
      ? resolveCategoryId({ category_id: body.category_id, category: body.category }, type)
      : (existing && existing.category_id) || resolveCategoryId({}, type);
  } else if (kind === 'traspaso') {
    r.from = body.from_account != null ? String(body.from_account).trim().slice(0, 60) : (existing && existing.from_account) || '';
    r.to = body.to_account != null ? String(body.to_account).trim().slice(0, 60) : (existing && existing.to_account) || '';
    if (!r.from || !r.to) return { error: 'Faltan las cuentas de origen y destino' };
    if (r.from.toLowerCase() === r.to.toLowerCase()) return { error: 'Origen y destino no pueden ser la misma cuenta' };
  } else {
    r.investmentId = body.investment_id != null ? Number(body.investment_id) : existing && existing.investment_id;
    const inv = db.prepare('SELECT id FROM investments WHERE id = ? AND archived = 0').get(r.investmentId || 0);
    if (!inv) return { error: 'Elige una inversión válida' };
    r.from = body.from_account != null ? String(body.from_account).trim().slice(0, 60) : (existing && existing.from_account) || '';
  }
  return r;
}

router.post('/recurring', (req, res) => {
  const v = validateRecurringBody(req.body || {});
  if (v.error) return badRequest(res, v.error);
  const info = db
    .prepare(
      `INSERT INTO recurring (kind, type, amount_cents, category_id, note, day_of_month, start_month, from_account, to_account, investment_id)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
    )
    .run(v.kind, v.type, v.amountCents, v.categoryId, v.note, v.day, currentMonth(), v.from, v.to, v.investmentId);
  applyRecurring();
  res.status(201).json(db.prepare(`${RECURRING_SELECT} WHERE r.id = ?`).get(info.lastInsertRowid));
});

router.put('/recurring/:id', (req, res) => {
  const id = Number(req.params.id);
  const existing = db.prepare('SELECT * FROM recurring WHERE id = ?').get(id);
  if (!existing) return res.status(404).json({ error: 'Recurrente no encontrado' });
  const body = req.body || {};
  const v = validateRecurringBody(body, existing);
  if (v.error) return badRequest(res, v.error);
  const active = body.active != null ? (body.active ? 1 : 0) : existing.active;

  db.prepare(
    `UPDATE recurring SET kind = ?, type = ?, amount_cents = ?, category_id = ?, note = ?,
       day_of_month = ?, active = ?, from_account = ?, to_account = ?, investment_id = ? WHERE id = ?`
  ).run(v.kind, v.type, v.amountCents, v.categoryId, v.note, v.day, active, v.from, v.to, v.investmentId, id);
  applyRecurring();
  res.json(db.prepare(`${RECURRING_SELECT} WHERE r.id = ?`).get(id));
});

// Borra la regla; los movimientos ya creados se conservan.
router.delete('/recurring/:id', (req, res) => {
  const info = db.prepare('DELETE FROM recurring WHERE id = ?').run(Number(req.params.id));
  if (info.changes === 0) return res.status(404).json({ error: 'Recurrente no encontrado' });
  res.json({ deleted: true });
});

// ---------- cuentas ----------
//
// El saldo de cada cuenta es un valor inicial (a una fecha) + la suma de los
// movimientos con ese "account" desde esa fecha en adelante. No hay CRUD de
// cuentas: son simplemente los valores distintos usados en transactions.account,
// más las que ya tengan un saldo inicial guardado aunque no se hayan usado aún.

function accountNets(name, openingDate, untilDate) {
  const net = db
    .prepare(
      `SELECT COALESCE(SUM(CASE WHEN type = 'ingreso' THEN amount_cents ELSE -amount_cents END), 0) AS net
       FROM transactions WHERE account = ? COLLATE NOCASE AND date >= ? AND date <= ?`
    )
    .get(name, openingDate, untilDate).net;
  const transferNet = db
    .prepare(
      `SELECT COALESCE(SUM(CASE WHEN to_account = ?1 COLLATE NOCASE THEN amount_cents ELSE 0 END), 0)
            - COALESCE(SUM(CASE WHEN from_account = ?1 COLLATE NOCASE THEN amount_cents ELSE 0 END), 0) AS net
       FROM transfers WHERE date >= ?2 AND date <= ?3
         AND (to_account = ?1 COLLATE NOCASE OR from_account = ?1 COLLATE NOCASE)`
    )
    .get(name, openingDate, untilDate).net;
  return net + transferNet;
}

function computeAccountBalance(name, openingCents, openingDate, untilDate = '9999-12-31', apy = 0) {
  const end = untilDate === '9999-12-31' ? todayLocal() : untilDate;
  if (!apy || apy <= 0 || end <= openingDate) {
    return openingCents + accountNets(name, openingDate, untilDate);
  }

  // Cuenta remunerada: interés compuesto diario sobre el saldo real de cada
  // día. TAE → tipo diario equivalente: (1+TAE)^(1/365) - 1. El interés de un
  // día se abona sobre el cierre del día anterior (como hace Revolut).
  const dailyRate = Math.pow(1 + apy / 100, 1 / 365) - 1;
  const perDay = new Map();
  const txRows = db
    .prepare(
      `SELECT date, SUM(CASE WHEN type = 'ingreso' THEN amount_cents ELSE -amount_cents END) AS net
       FROM transactions WHERE account = ? COLLATE NOCASE AND date >= ? AND date <= ? GROUP BY date`
    )
    .all(name, openingDate, end);
  const trRows = db
    .prepare(
      `SELECT date, SUM(CASE WHEN to_account = ?1 COLLATE NOCASE THEN amount_cents ELSE -amount_cents END) AS net
       FROM transfers WHERE date >= ?2 AND date <= ?3
         AND (to_account = ?1 COLLATE NOCASE OR from_account = ?1 COLLATE NOCASE) GROUP BY date`
    )
    .all(name, openingDate, end);
  for (const r of [...txRows, ...trRows]) perDay.set(r.date, (perDay.get(r.date) || 0) + r.net);

  // Los movimientos del día de apertura cuentan (igual que sin TAE), pero ese
  // día 0 no devenga interés todavía.
  let bal = openingCents + (perDay.get(openingDate) || 0);
  const cursor = new Date(openingDate + 'T12:00:00');
  const endMs = new Date(end + 'T12:00:00').getTime();
  const pad = (n) => String(n).padStart(2, '0');
  while (cursor.getTime() < endMs) {
    cursor.setDate(cursor.getDate() + 1);
    bal *= 1 + dailyRate;
    const key = `${cursor.getFullYear()}-${pad(cursor.getMonth() + 1)}-${pad(cursor.getDate())}`;
    bal += perDay.get(key) || 0;
  }
  return Math.round(bal);
}

router.get('/accounts', (req, res) => {
  const openings = db.prepare('SELECT * FROM account_balances').all();
  const used = db
    .prepare(
      `SELECT DISTINCT account AS name FROM transactions WHERE account != ''
       UNION SELECT DISTINCT from_account FROM transfers
       UNION SELECT DISTINCT to_account FROM transfers`
    )
    .all()
    .map((r) => r.name);

  const names = new Map(); // nombre en minúsculas -> nombre "bonito" a mostrar
  for (const o of openings) names.set(o.name.toLowerCase(), o.name);
  for (const a of used) if (!names.has(a.toLowerCase())) names.set(a.toLowerCase(), a);

  const result = [...names.values()].map((name) => {
    const o = openings.find((x) => x.name.toLowerCase() === name.toLowerCase());
    const openingCents = o ? o.opening_cents : 0;
    const openingDate = o ? o.opening_date : '1970-01-01';
    const apy = o ? o.apy : 0;
    const current = computeAccountBalance(name, openingCents, openingDate, '9999-12-31', apy);
    return {
      name,
      opening_cents: openingCents,
      opening_date: openingDate,
      apy,
      current_cents: current,
      // Intereses acumulados desde el saldo inicial (diferencia con el cálculo sin TAE).
      interest_cents: apy > 0 ? current - computeAccountBalance(name, openingCents, openingDate) : 0,
    };
  });
  result.sort((a, b) => a.name.localeCompare(b.name, 'es'));
  res.json(result);
});

router.put('/accounts/:name', (req, res) => {
  const name = String(req.params.name || '').trim();
  if (!name) return badRequest(res, 'Falta el nombre de la cuenta');
  const body = req.body || {};
  const openingCents = parseAmountCents(body.opening);
  if (!Number.isInteger(openingCents)) return badRequest(res, 'Saldo inicial no válido');
  const openingDate = isValidDate(body.opening_date) ? body.opening_date : todayLocal();
  let apy = 0;
  if (body.apy != null && body.apy !== '') {
    apy = Number(String(body.apy).replace('%', '').replace(',', '.'));
    if (!isFinite(apy) || apy < 0 || apy > 100) return badRequest(res, 'TAE no válida (0-100)');
  }

  db.prepare(
    `INSERT INTO account_balances (name, opening_cents, opening_date, apy) VALUES (?, ?, ?, ?)
     ON CONFLICT(name) DO UPDATE SET opening_cents = excluded.opening_cents,
       opening_date = excluded.opening_date, apy = excluded.apy`
  ).run(name, openingCents, openingDate, apy);

  const current = computeAccountBalance(name, openingCents, openingDate, '9999-12-31', apy);
  res.json({
    name,
    opening_cents: openingCents,
    opening_date: openingDate,
    apy,
    current_cents: current,
    interest_cents: apy > 0 ? current - computeAccountBalance(name, openingCents, openingDate) : 0,
  });
});

router.delete('/accounts/:name', (req, res) => {
  const info = db.prepare('DELETE FROM account_balances WHERE name = ? COLLATE NOCASE').run(req.params.name);
  res.json({ deleted: info.changes > 0 });
});

// ---------- movimientos ----------

router.get('/transactions', (req, res) => {
  applyRecurring();
  const where = [];
  const params = [];
  if (req.query.month && /^\d{4}-\d{2}$/.test(req.query.month)) {
    where.push("strftime('%Y-%m', t.date) = ?");
    params.push(req.query.month);
  }
  if (isValidDate(req.query.from)) {
    where.push('t.date >= ?');
    params.push(req.query.from);
  }
  if (isValidDate(req.query.to)) {
    where.push('t.date <= ?');
    params.push(req.query.to);
  }
  const type = normalizeType(req.query.type);
  if (type) {
    where.push('t.type = ?');
    params.push(type);
  }
  if (req.query.category_id) {
    where.push('t.category_id = ?');
    params.push(Number(req.query.category_id));
  }
  if (req.query.account) {
    where.push('t.account = ? COLLATE NOCASE');
    params.push(String(req.query.account).trim());
  }
  if (req.query.q) {
    where.push('(t.note LIKE ? OR c.name LIKE ? OR t.tags LIKE ?)');
    const like = `%${String(req.query.q).trim().replace(/^#/, '')}%`;
    params.push(like, like, like);
  }
  const amountMin = req.query.amount_min != null && req.query.amount_min !== '' ? parseAmountCents(req.query.amount_min) : null;
  if (Number.isInteger(amountMin)) {
    where.push('t.amount_cents >= ?');
    params.push(amountMin);
  }
  const amountMax = req.query.amount_max != null && req.query.amount_max !== '' ? parseAmountCents(req.query.amount_max) : null;
  if (Number.isInteger(amountMax)) {
    where.push('t.amount_cents <= ?');
    params.push(amountMax);
  }
  const whereSql = where.length ? 'WHERE ' + where.join(' AND ') : '';
  const limit = Math.min(Number(req.query.limit) || 500, 1000);
  const offset = Math.max(Number(req.query.offset) || 0, 0);

  const items = db
    .prepare(`${TX_SELECT} ${whereSql} ORDER BY t.date DESC, t.id DESC LIMIT ? OFFSET ?`)
    .all(...params, limit, offset);
  const totals = db
    .prepare(
      `SELECT COUNT(*) AS total_count,
              COALESCE(SUM(CASE WHEN t.type = 'gasto' THEN t.amount_cents END), 0) AS gastos_cents,
              COALESCE(SUM(CASE WHEN t.type = 'ingreso' THEN t.amount_cents END), 0) AS ingresos_cents
       FROM transactions t LEFT JOIN categories c ON c.id = t.category_id ${whereSql}`
    )
    .get(...params);
  res.json({ items, ...totals });
});

// Valida e inserta un movimiento; compartido por el alta individual y el batch.
function createTransactionFromBody(body, { skipDuplicates = false } = {}) {
  const type = normalizeType(readType(body));
  if (!type) return { error: "El tipo debe ser 'gasto' o 'ingreso'" };
  const amountCents = parseAmountCents(readAmount(body));
  if (!Number.isInteger(amountCents) || amountCents <= 0) {
    return { error: 'Importe no válido' };
  }
  const date = isValidDate(body.date) ? body.date : todayLocal();
  const note = readNote(body);
  const account = readAccount(body);
  const tags = readTags(body);
  const source = String(body.source || 'web').trim() || 'web';
  const categoryId = resolveCategoryId(body, type);

  if (skipDuplicates) {
    const dup = db
      .prepare(
        'SELECT 1 FROM transactions WHERE date = ? AND amount_cents = ? AND type = ? AND note = ? COLLATE NOCASE LIMIT 1'
      )
      .get(date, amountCents, type, note);
    if (dup) return { duplicate: true };
  }

  const info = db
    .prepare(
      'INSERT INTO transactions (type, amount_cents, category_id, note, account, date, source, tags) VALUES (?, ?, ?, ?, ?, ?, ?, ?)'
    )
    .run(type, amountCents, categoryId, note, account, date, source, tags);
  return { created: db.prepare(`${TX_SELECT} WHERE t.id = ?`).get(info.lastInsertRowid) };
}

router.post('/transactions', (req, res) => {
  const result = createTransactionFromBody(req.body || {});
  if (result.error) return badRequest(res, result.error);
  res.status(201).json(result.created);
});

// Alta en lote: para el importador CSV y la cola offline del atajo.
// Acepta {items: [...], skip_duplicates}, un array JSON directo, o texto
// plano con un JSON por línea (lo que genera el atajo al vaciar su cola).
router.post('/transactions/batch', (req, res) => {
  let items = [];
  let skipDuplicates = false;
  if (typeof req.body === 'string') {
    for (const line of req.body.split('\n')) {
      const s = line.trim();
      if (!s) continue;
      try {
        items.push(JSON.parse(s));
      } catch (_) {
        return badRequest(res, `Línea no válida: ${s.slice(0, 80)}`);
      }
    }
    skipDuplicates = true; // la cola del atajo puede reintentar: nunca duplicar
  } else if (Array.isArray(req.body)) {
    items = req.body;
  } else if (req.body && Array.isArray(req.body.items)) {
    items = req.body.items;
    skipDuplicates = Boolean(req.body.skip_duplicates);
  }
  if (items.length === 0) return badRequest(res, 'No hay movimientos que importar');
  if (items.length > 5000) return badRequest(res, 'Máximo 5000 movimientos por lote');

  let created = 0;
  let duplicates = 0;
  const errors = [];
  db.exec('BEGIN');
  try {
    items.forEach((item, i) => {
      const result = createTransactionFromBody(item || {}, { skipDuplicates });
      if (result.error) errors.push({ index: i, error: result.error });
      else if (result.duplicate) duplicates += 1;
      else created += 1;
    });
    db.exec('COMMIT');
  } catch (e) {
    db.exec('ROLLBACK');
    throw e;
  }
  res.status(errors.length && created === 0 && duplicates === 0 ? 400 : 201).json({
    ok: true,
    created,
    skipped_duplicates: duplicates,
    errors,
  });
});

router.put('/transactions/:id', (req, res) => {
  const id = Number(req.params.id);
  const existing = db.prepare('SELECT * FROM transactions WHERE id = ?').get(id);
  if (!existing) return res.status(404).json({ error: 'Movimiento no encontrado' });
  const body = req.body || {};

  const rawType = readType(body);
  const type = rawType != null ? normalizeType(rawType) : existing.type;
  if (!type) return badRequest(res, "El tipo debe ser 'gasto' o 'ingreso'");
  let amountCents = existing.amount_cents;
  const rawAmount = readAmount(body);
  if (rawAmount != null) {
    amountCents = parseAmountCents(rawAmount);
    if (!Number.isInteger(amountCents) || amountCents <= 0) {
      return badRequest(res, 'Importe no válido');
    }
  }
  let date = existing.date;
  if (body.date != null) {
    if (!isValidDate(body.date)) return badRequest(res, 'Fecha no válida (YYYY-MM-DD)');
    date = body.date;
  }
  const note = body.note != null || body.descripcion != null ? readNote(body) : existing.note;
  const account = body.account != null || body.cuenta != null ? readAccount(body) : existing.account;
  const tags = body.tags != null || body.etiquetas != null ? readTags(body) : existing.tags;
  let categoryId = existing.category_id;
  const rawCategory = body.category != null ? body.category : body.categoria;
  if (body.category_id != null || rawCategory != null || rawType != null) {
    categoryId = resolveCategoryId(
      { category_id: body.category_id, category: rawCategory },
      type
    );
    if (body.category_id == null && rawCategory == null && type === existing.type) {
      categoryId = existing.category_id;
    }
  }

  db.prepare(
    'UPDATE transactions SET type = ?, amount_cents = ?, category_id = ?, note = ?, account = ?, date = ?, tags = ? WHERE id = ?'
  ).run(type, amountCents, categoryId, note, account, date, tags, id);
  res.json(db.prepare(`${TX_SELECT} WHERE t.id = ?`).get(id));
});

router.delete('/transactions/:id', (req, res) => {
  const id = Number(req.params.id);
  const info = db.prepare('DELETE FROM transactions WHERE id = ?').run(id);
  if (info.changes === 0) return res.status(404).json({ error: 'Movimiento no encontrado' });
  res.json({ deleted: true });
});

// ---------- resumen ----------

router.get('/summary', (req, res) => {
  applyRecurring();
  const month =
    req.query.month && /^\d{4}-\d{2}$/.test(req.query.month)
      ? req.query.month
      : todayLocal().slice(0, 7);

  const totals = db
    .prepare(
      `SELECT COALESCE(SUM(CASE WHEN type = 'ingreso' THEN amount_cents END), 0) AS ingresos_cents,
              COALESCE(SUM(CASE WHEN type = 'gasto' THEN amount_cents END), 0) AS gastos_cents,
              COUNT(*) AS count,
              COUNT(DISTINCT CASE WHEN type = 'gasto' THEN date END) AS spend_days
       FROM transactions WHERE strftime('%Y-%m', date) = ?`
    )
    .get(month);

  const byCategory = db
    .prepare(
      `SELECT c.id, c.name, c.icon, c.color, c.budget_cents,
              SUM(t.amount_cents) AS total_cents, COUNT(*) AS count
       FROM transactions t LEFT JOIN categories c ON c.id = t.category_id
       WHERE t.type = 'gasto' AND strftime('%Y-%m', t.date) = ?
       GROUP BY t.category_id ORDER BY total_cents DESC`
    )
    .all(month);

  // Mes anterior, para la comparativa de las tarjetas.
  const [py, pm] = month.split('-').map(Number);
  const prevDate = new Date(py, pm - 2, 1);
  const prevMonth = `${prevDate.getFullYear()}-${String(prevDate.getMonth() + 1).padStart(2, '0')}`;
  const prevTotals = db
    .prepare(
      `SELECT COALESCE(SUM(CASE WHEN type = 'ingreso' THEN amount_cents END), 0) AS ingresos_cents,
              COALESCE(SUM(CASE WHEN type = 'gasto' THEN amount_cents END), 0) AS gastos_cents
       FROM transactions WHERE strftime('%Y-%m', date) = ?`
    )
    .get(prevMonth);

  // Serie de los últimos 12 meses terminando en el mes consultado.
  const [y, m] = month.split('-').map(Number);
  const months = [];
  for (let i = 11; i >= 0; i--) {
    const d = new Date(y, m - 1 - i, 1);
    months.push(`${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`);
  }
  const seriesRows = db
    .prepare(
      `SELECT strftime('%Y-%m', date) AS month,
              COALESCE(SUM(CASE WHEN type = 'ingreso' THEN amount_cents END), 0) AS ingresos_cents,
              COALESCE(SUM(CASE WHEN type = 'gasto' THEN amount_cents END), 0) AS gastos_cents
       FROM transactions
       WHERE strftime('%Y-%m', date) BETWEEN ? AND ?
       GROUP BY strftime('%Y-%m', date)`
    )
    .all(months[0], months[11]);
  const byMonth = new Map(seriesRows.map((r) => [r.month, r]));
  const monthly = months.map(
    (mo) => byMonth.get(mo) || { month: mo, ingresos_cents: 0, gastos_cents: 0 }
  );

  const recent = db.prepare(`${TX_SELECT} ORDER BY t.date DESC, t.id DESC LIMIT 8`).all();

  // Gasto por día del mes (heatmap, ritmo acumulado y día de la semana).
  const dailySql = `SELECT date, SUM(amount_cents) AS cents FROM transactions
                    WHERE type = 'gasto' AND strftime('%Y-%m', date) = ? GROUP BY date`;
  const daily = db.prepare(dailySql).all(month);
  const prevDaily = db.prepare(dailySql).all(prevMonth);

  // Desglose del mes anterior por categoría (para los insights de cierre).
  const prevByCategory = db
    .prepare(
      `SELECT c.id, c.name, c.icon, SUM(t.amount_cents) AS total_cents
       FROM transactions t LEFT JOIN categories c ON c.id = t.category_id
       WHERE t.type = 'gasto' AND strftime('%Y-%m', t.date) = ?
       GROUP BY t.category_id ORDER BY total_cents DESC`
    )
    .all(prevMonth);

  // Gasto ya materializado por recurrentes este mes y recurrentes de gasto que
  // aún no han caído (para una proyección de fin de mes más realista).
  const gastosRecurrentes = db
    .prepare(
      `SELECT COALESCE(SUM(amount_cents), 0) AS cents FROM transactions
       WHERE type = 'gasto' AND source = 'recurrente' AND strftime('%Y-%m', date) = ?`
    )
    .get(month).cents;
  let pendingRecurring = 0;
  if (month === todayLocal().slice(0, 7)) {
    const todayDay = Number(todayLocal().slice(8, 10));
    const lastDay = new Date(Number(month.slice(0, 4)), Number(month.slice(5, 7)), 0).getDate();
    const rules = db
      .prepare(
        `SELECT * FROM recurring WHERE active = 1
         AND ((kind = 'movimiento' AND type = 'gasto') OR (kind = 'aportacion' AND from_account != ''))`
      )
      .all();
    for (const r of rules) {
      const day = Math.min(r.day_of_month, lastDay);
      if (day > todayDay && (!r.last_applied || r.last_applied < month)) pendingRecurring += r.amount_cents;
    }
  }

  // Presupuestos efectivos del mes consultado (con historial de cambios).
  const globalRow = db.prepare("SELECT value FROM app_settings WHERE key = 'global_budget_cents'").get();
  const globalNow = globalRow ? Number(globalRow.value) : 0;
  const budgets = db
    .prepare("SELECT id, name, icon, budget_cents FROM categories WHERE type = 'gasto'")
    .all()
    .map((c) => ({ id: c.id, name: c.name, icon: c.icon, budget_cents: budgetForMonth(c.id, month, c.budget_cents) }))
    .filter((c) => c.budget_cents > 0);

  res.json({
    month,
    ingresos_cents: totals.ingresos_cents,
    gastos_cents: totals.gastos_cents,
    balance_cents: totals.ingresos_cents - totals.gastos_cents,
    count: totals.count,
    spend_days: totals.spend_days,
    prev_month: prevMonth,
    prev_ingresos_cents: prevTotals.ingresos_cents,
    prev_gastos_cents: prevTotals.gastos_cents,
    by_category: byCategory,
    prev_by_category: prevByCategory,
    monthly,
    recent,
    daily,
    prev_daily: prevDaily,
    gastos_recurrentes_cents: gastosRecurrentes,
    pending_recurring_cents: pendingRecurring,
    budgets,
    global_budget_month_cents: budgetForMonth(0, month, globalNow),
  });
});

// Serie de 12 meses de una categoría (para el detalle al tocar el donut).
router.get('/categories/:id/monthly', (req, res) => {
  const id = Number(req.params.id);
  const cat = db.prepare('SELECT * FROM categories WHERE id = ?').get(id);
  if (!cat) return res.status(404).json({ error: 'Categoría no encontrada' });
  const month =
    req.query.month && /^\d{4}-\d{2}$/.test(req.query.month)
      ? req.query.month
      : todayLocal().slice(0, 7);
  const [y, m] = month.split('-').map(Number);
  const months = [];
  for (let i = 11; i >= 0; i--) {
    const d = new Date(y, m - 1 - i, 1);
    months.push(`${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`);
  }
  const rows = db
    .prepare(
      `SELECT strftime('%Y-%m', date) AS month, SUM(amount_cents) AS total_cents
       FROM transactions WHERE category_id = ? AND strftime('%Y-%m', date) BETWEEN ? AND ?
       GROUP BY strftime('%Y-%m', date)`
    )
    .all(id, months[0], months[11]);
  const byM = new Map(rows.map((r) => [r.month, r.total_cents]));
  res.json({
    category: { id: cat.id, name: cat.name, icon: cat.icon, color: cat.color, type: cat.type },
    months: months.map((mo) => ({ month: mo, total_cents: byM.get(mo) || 0 })),
  });
});

// ---------- plantillas rápidas ----------

router.get('/templates', (req, res) => {
  res.json(
    db
      .prepare(
        `SELECT tp.*, c.name AS category_name, c.icon AS category_icon
         FROM templates tp LEFT JOIN categories c ON c.id = tp.category_id ORDER BY tp.id`
      )
      .all()
  );
});

router.post('/templates', (req, res) => {
  const body = req.body || {};
  const type = normalizeType(readType(body));
  if (!type) return badRequest(res, "El tipo debe ser 'gasto' o 'ingreso'");
  const amountCents = parseAmountCents(readAmount(body));
  if (!Number.isInteger(amountCents) || amountCents <= 0) return badRequest(res, 'Importe no válido');
  const count = db.prepare('SELECT COUNT(*) AS n FROM templates').get().n;
  if (count >= 20) return badRequest(res, 'Máximo 20 plantillas — borra alguna primero');
  const note = readNote(body);
  const account = readAccount(body);
  const categoryId = resolveCategoryId(body, type);
  const name = String(body.name || '').trim().slice(0, 40) || note.slice(0, 40) || 'Plantilla';
  const info = db
    .prepare('INSERT INTO templates (name, type, amount_cents, category_id, note, account) VALUES (?, ?, ?, ?, ?, ?)')
    .run(name, type, amountCents, categoryId, note, account);
  res.status(201).json(
    db
      .prepare(
        `SELECT tp.*, c.name AS category_name, c.icon AS category_icon
         FROM templates tp LEFT JOIN categories c ON c.id = tp.category_id WHERE tp.id = ?`
      )
      .get(info.lastInsertRowid)
  );
});

router.delete('/templates/:id', (req, res) => {
  const info = db.prepare('DELETE FROM templates WHERE id = ?').run(Number(req.params.id));
  if (info.changes === 0) return res.status(404).json({ error: 'Plantilla no encontrada' });
  res.json({ deleted: true });
});

// ---------- sugerencias de recurrentes (suscripciones detectadas) ----------
//
// Pagos con la misma nota, categoría e importe repetidos en 3+ meses de los
// últimos 5 que no vengan ya de una regla recurrente: probables suscripciones.

router.get('/suggestions/recurring', (req, res) => {
  const d = new Date();
  const sinceDate = new Date(d.getFullYear(), d.getMonth() - 4, 1);
  const from = `${sinceDate.getFullYear()}-${String(sinceDate.getMonth() + 1).padStart(2, '0')}-01`;
  const rows = db
    .prepare(
      `SELECT MIN(t.note) AS note, t.category_id, t.amount_cents,
              c.name AS category_name, c.icon AS category_icon,
              COUNT(DISTINCT strftime('%Y-%m', t.date)) AS months_seen,
              CAST(ROUND(AVG(CAST(strftime('%d', t.date) AS INTEGER))) AS INTEGER) AS typical_day,
              MAX(t.date) AS last_date
       FROM transactions t LEFT JOIN categories c ON c.id = t.category_id
       WHERE t.type = 'gasto' AND t.source != 'recurrente' AND t.note != '' AND t.date >= ?
       GROUP BY LOWER(TRIM(t.note)), t.category_id, t.amount_cents
       HAVING months_seen >= 3
       ORDER BY months_seen DESC, t.amount_cents DESC LIMIT 10`
    )
    .all(from);
  const existing = db
    .prepare("SELECT amount_cents, LOWER(TRIM(note)) AS note FROM recurring WHERE active = 1")
    .all();
  const covered = new Set(existing.map((r) => `${r.amount_cents}|${r.note}`));
  res.json(rows.filter((r) => !covered.has(`${r.amount_cents}|${String(r.note).trim().toLowerCase()}`)));
});

// ---------- transferencias entre cuentas ----------
//
// No son gasto ni ingreso: solo mueven saldo de una cuenta a otra.

router.get('/transfers', (req, res) => {
  const where = [];
  const params = [];
  if (req.query.month && /^\d{4}-\d{2}$/.test(req.query.month)) {
    where.push("strftime('%Y-%m', date) = ?");
    params.push(req.query.month);
  }
  if (req.query.account) {
    where.push('(from_account = ? COLLATE NOCASE OR to_account = ? COLLATE NOCASE)');
    const a = String(req.query.account).trim();
    params.push(a, a);
  }
  const whereSql = where.length ? 'WHERE ' + where.join(' AND ') : '';
  res.json(db.prepare(`SELECT * FROM transfers ${whereSql} ORDER BY date DESC, id DESC LIMIT 500`).all(...params));
});

function validateTransferBody(body, existing = null) {
  const amountCents = body.amount != null ? parseAmountCents(body.amount) : existing && existing.amount_cents;
  if (!Number.isInteger(amountCents) || amountCents <= 0) return { error: 'Importe no válido' };
  const from = body.from_account != null ? String(body.from_account).trim().slice(0, 60) : existing && existing.from_account;
  const to = body.to_account != null ? String(body.to_account).trim().slice(0, 60) : existing && existing.to_account;
  if (!from || !to) return { error: 'Faltan las cuentas de origen y destino' };
  if (from.toLowerCase() === to.toLowerCase()) return { error: 'Origen y destino no pueden ser la misma cuenta' };
  let date = existing ? existing.date : todayLocal();
  if (body.date != null) {
    if (!isValidDate(body.date)) return { error: 'Fecha no válida (YYYY-MM-DD)' };
    date = body.date;
  }
  const note = body.note != null ? String(body.note).trim() : (existing && existing.note) || '';
  return { amountCents, from, to, date, note };
}

router.post('/transfers', (req, res) => {
  const v = validateTransferBody(req.body || {});
  if (v.error) return badRequest(res, v.error);
  const info = db
    .prepare('INSERT INTO transfers (amount_cents, from_account, to_account, note, date) VALUES (?, ?, ?, ?, ?)')
    .run(v.amountCents, v.from, v.to, v.note, v.date);
  res.status(201).json(db.prepare('SELECT * FROM transfers WHERE id = ?').get(info.lastInsertRowid));
});

router.put('/transfers/:id', (req, res) => {
  const id = Number(req.params.id);
  const existing = db.prepare('SELECT * FROM transfers WHERE id = ?').get(id);
  if (!existing) return res.status(404).json({ error: 'Transferencia no encontrada' });
  const v = validateTransferBody(req.body || {}, existing);
  if (v.error) return badRequest(res, v.error);
  db.prepare('UPDATE transfers SET amount_cents = ?, from_account = ?, to_account = ?, note = ?, date = ? WHERE id = ?')
    .run(v.amountCents, v.from, v.to, v.note, v.date, id);
  res.json(db.prepare('SELECT * FROM transfers WHERE id = ?').get(id));
});

router.delete('/transfers/:id', (req, res) => {
  const info = db.prepare('DELETE FROM transfers WHERE id = ?').run(Number(req.params.id));
  if (info.changes === 0) return res.status(404).json({ error: 'Transferencia no encontrada' });
  res.json({ deleted: true });
});

// ---------- inversiones ----------

function investmentTotals(rows) {
  let value = 0;
  let cost = 0;
  for (const r of rows) {
    if (r.value_cents != null) value += r.value_cents;
    cost += r.cost_cents;
  }
  return { value_cents: value, cost_cents: cost, gain_cents: value - cost };
}

async function listInvestmentsWithPrices(force = false) {
  const rows = db.prepare('SELECT * FROM investments WHERE archived = 0 ORDER BY name').all();
  const result = [];
  for (const inv of rows) {
    let price = null;
    let fetchedAt = null;
    let error = null;
    if (inv.provider === 'manual') {
      price = inv.manual_price;
    } else if (inv.symbol) {
      const p = await getPriceEur(inv.provider, inv.symbol, force);
      price = p.price;
      fetchedAt = p.fetched_at;
      error = p.error;
    }
    const valueCents = price != null ? Math.round(inv.units * price * 100) : null;
    // Foto diaria del precio, para la mini-gráfica de 30 días.
    if (price != null) {
      db.prepare(
        'INSERT INTO investment_prices (investment_id, date, price) VALUES (?, ?, ?) ON CONFLICT(investment_id, date) DO UPDATE SET price = excluded.price'
      ).run(inv.id, todayLocal(), price);
    }
    result.push({
      ...inv,
      price_eur: price,
      price_fetched_at: fetchedAt,
      price_error: error,
      value_cents: valueCents,
      gain_cents: valueCents != null ? valueCents - inv.cost_cents : null,
    });
  }
  // Historial de precios de los últimos 30 días de todas las posiciones.
  const cutoff = new Date(Date.now() - 31 * 86400000);
  const from = `${cutoff.getFullYear()}-${String(cutoff.getMonth() + 1).padStart(2, '0')}-${String(cutoff.getDate()).padStart(2, '0')}`;
  const histRows = db
    .prepare('SELECT investment_id, date, price FROM investment_prices WHERE date >= ? ORDER BY date')
    .all(from);
  const histByInv = new Map();
  for (const h of histRows) {
    if (!histByInv.has(h.investment_id)) histByInv.set(h.investment_id, []);
    histByInv.get(h.investment_id).push({ date: h.date, price: h.price });
  }
  for (const r of result) r.price_history = histByInv.get(r.id) || [];
  // Foto diaria del valor de la cartera, para la gráfica de patrimonio.
  const totals = investmentTotals(result);
  if (result.length > 0 && result.every((r) => r.value_cents != null)) {
    db.prepare(
      `INSERT INTO networth_snapshots (date, investments_cents) VALUES (?, ?)
       ON CONFLICT(date) DO UPDATE SET investments_cents = excluded.investments_cents`
    ).run(todayLocal(), totals.value_cents);
  }
  return { items: result, totals };
}

router.get('/investments', async (req, res, next) => {
  try {
    res.json(await listInvestmentsWithPrices(false));
  } catch (e) {
    next(e);
  }
});

router.post('/investments/refresh', async (req, res, next) => {
  try {
    res.json(await listInvestmentsWithPrices(true));
  } catch (e) {
    next(e);
  }
});

// Buscador de símbolos (Yahoo o CoinGecko) para el formulario.
router.get('/investments/search', async (req, res) => {
  const q = String(req.query.q || '').trim();
  const provider = req.query.provider === 'coingecko' ? 'coingecko' : 'yahoo';
  if (q.length < 2) return res.json([]);
  try {
    res.json(await searchSymbols(provider, q));
  } catch (e) {
    res.json([]); // sin internet o rate limit: sugerencias vacías, sin romper el formulario
  }
});

function validateInvestmentBody(body, existing = null) {
  const name = body.name != null ? String(body.name).trim() : existing && existing.name;
  if (!name) return { error: 'Falta el nombre' };
  const provider = body.provider != null ? String(body.provider) : existing && existing.provider;
  if (!['yahoo', 'coingecko', 'manual'].includes(provider)) {
    return { error: "El proveedor debe ser 'yahoo', 'coingecko' o 'manual'" };
  }
  const symbol = body.symbol != null ? String(body.symbol).trim().slice(0, 40) : (existing && existing.symbol) || '';
  if (provider !== 'manual' && !symbol) return { error: 'Falta el símbolo/ticker' };
  const units = body.units != null ? Number(String(body.units).replace(',', '.')) : existing && existing.units;
  if (!isFinite(units) || units <= 0) return { error: 'Unidades no válidas' };
  let costCents = existing ? existing.cost_cents : 0;
  if (body.cost != null && body.cost !== '') {
    costCents = parseAmountCents(body.cost);
    if (!Number.isInteger(costCents) || costCents < 0) return { error: 'Coste no válido' };
  }
  let manualPrice = existing ? existing.manual_price : null;
  if (body.manual_price != null && body.manual_price !== '') {
    manualPrice = parseAmountCents(body.manual_price);
    if (!Number.isInteger(manualPrice) || manualPrice < 0) return { error: 'Precio manual no válido' };
    manualPrice = manualPrice / 100;
  }
  return { name, provider, symbol, units, costCents, manualPrice };
}

router.post('/investments', (req, res) => {
  const v = validateInvestmentBody(req.body || {});
  if (v.error) return badRequest(res, v.error);
  const info = db
    .prepare('INSERT INTO investments (name, provider, symbol, units, cost_cents, manual_price) VALUES (?, ?, ?, ?, ?, ?)')
    .run(v.name, v.provider, v.symbol, v.units, v.costCents, v.manualPrice);
  // Lote inicial en el historial de operaciones (base para compras/ventas futuras).
  db.prepare("INSERT INTO investment_ops (investment_id, type, units, amount_cents, date) VALUES (?, 'compra', ?, ?, ?)")
    .run(info.lastInsertRowid, v.units, v.costCents, todayLocal());
  res.status(201).json(db.prepare('SELECT * FROM investments WHERE id = ?').get(info.lastInsertRowid));
});

router.put('/investments/:id', (req, res) => {
  const id = Number(req.params.id);
  const existing = db.prepare('SELECT * FROM investments WHERE id = ?').get(id);
  if (!existing) return res.status(404).json({ error: 'Inversión no encontrada' });
  const v = validateInvestmentBody(req.body || {}, existing);
  if (v.error) return badRequest(res, v.error);
  db.prepare('UPDATE investments SET name = ?, provider = ?, symbol = ?, units = ?, cost_cents = ?, manual_price = ? WHERE id = ?')
    .run(v.name, v.provider, v.symbol, v.units, v.costCents, v.manualPrice, id);
  // Editar la posición a mano reinicia el historial de operaciones a un único
  // lote con la posición nueva (evita que el ledger y la posición se desincronicen).
  if (v.units !== existing.units || v.costCents !== existing.cost_cents) {
    db.prepare('DELETE FROM investment_ops WHERE investment_id = ?').run(id);
    db.prepare("INSERT INTO investment_ops (investment_id, type, units, amount_cents, date) VALUES (?, 'compra', ?, ?, ?)")
      .run(id, v.units, v.costCents, todayLocal());
  }
  res.json(db.prepare('SELECT * FROM investments WHERE id = ?').get(id));
});

router.delete('/investments/:id', (req, res) => {
  const info = db.prepare('DELETE FROM investments WHERE id = ?').run(Number(req.params.id));
  if (info.changes === 0) return res.status(404).json({ error: 'Inversión no encontrada' });
  res.json({ deleted: true });
});

// ---------- operaciones de inversión (compras/ventas parciales) ----------
//
// Recorre las operaciones en orden cronológico con coste medio: una compra
// suma unidades y coste; una venta resta unidades y saca el coste medio
// (la diferencia con lo ingresado es ganancia realizada). Las unidades y el
// coste de la inversión se recalculan desde aquí en cada cambio.

function replayOps(investmentId) {
  const ops = db
    .prepare('SELECT * FROM investment_ops WHERE investment_id = ? ORDER BY date, id')
    .all(investmentId);
  let units = 0;
  let cost = 0;
  let realized = 0;
  for (const op of ops) {
    if (op.type === 'compra') {
      units += op.units;
      cost += op.amount_cents;
    } else {
      if (op.units > units + 1e-9) return { error: `La venta del ${op.date} deja las unidades en negativo` };
      const avg = units > 0 ? cost / units : 0;
      const costOut = Math.round(avg * op.units);
      realized += op.amount_cents - costOut;
      units -= op.units;
      cost -= costOut;
    }
  }
  units = Math.round(units * 1e9) / 1e9;
  return { units, cost_cents: Math.max(0, cost), realized_gain_cents: realized, ops };
}

function syncInvestmentFromOps(investmentId) {
  const r = replayOps(investmentId);
  if (r.error) return r;
  if (r.units <= 0) return { error: 'Las operaciones dejan la posición a cero — para cerrarla, borra la inversión' };
  db.prepare('UPDATE investments SET units = ?, cost_cents = ? WHERE id = ?').run(r.units, r.cost_cents, investmentId);
  return r;
}

router.get('/investments/:id/ops', (req, res) => {
  const id = Number(req.params.id);
  const inv = db.prepare('SELECT * FROM investments WHERE id = ?').get(id);
  if (!inv) return res.status(404).json({ error: 'Inversión no encontrada' });
  const r = replayOps(id);
  res.json({
    investment: { id: inv.id, name: inv.name, units: inv.units, cost_cents: inv.cost_cents },
    items: r.ops || [],
    units: r.units || 0,
    cost_cents: r.cost_cents || 0,
    realized_gain_cents: r.realized_gain_cents || 0,
  });
});

router.post('/investments/:id/ops', (req, res) => {
  const id = Number(req.params.id);
  const inv = db.prepare('SELECT * FROM investments WHERE id = ?').get(id);
  if (!inv) return res.status(404).json({ error: 'Inversión no encontrada' });
  const body = req.body || {};
  const type = body.type === 'venta' ? 'venta' : body.type === 'compra' ? 'compra' : null;
  if (!type) return badRequest(res, "El tipo debe ser 'compra' o 'venta'");
  const units = Number(String(body.units == null ? '' : body.units).replace(',', '.'));
  if (!isFinite(units) || units <= 0) return badRequest(res, 'Unidades no válidas');
  const amountCents = parseAmountCents(body.amount);
  if (!Number.isInteger(amountCents) || amountCents < 0) return badRequest(res, 'Importe no válido');
  const date = isValidDate(body.date) ? body.date : todayLocal();

  const info = db
    .prepare('INSERT INTO investment_ops (investment_id, type, units, amount_cents, date) VALUES (?, ?, ?, ?, ?)')
    .run(id, type, units, amountCents, date);
  const r = syncInvestmentFromOps(id);
  if (r.error) {
    db.prepare('DELETE FROM investment_ops WHERE id = ?').run(info.lastInsertRowid);
    return badRequest(res, r.error);
  }
  res.status(201).json({ ok: true, units: r.units, cost_cents: r.cost_cents, realized_gain_cents: r.realized_gain_cents });
});

router.delete('/investments/ops/:opId', (req, res) => {
  const opId = Number(req.params.opId);
  const op = db.prepare('SELECT * FROM investment_ops WHERE id = ?').get(opId);
  if (!op) return res.status(404).json({ error: 'Operación no encontrada' });
  db.prepare('DELETE FROM investment_ops WHERE id = ?').run(opId);
  const r = syncInvestmentFromOps(op.investment_id);
  if (r.error) {
    db.prepare(
      'INSERT INTO investment_ops (id, investment_id, type, units, amount_cents, date, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)'
    ).run(op.id, op.investment_id, op.type, op.units, op.amount_cents, op.date, op.created_at);
    return badRequest(res, r.error);
  }
  res.json({ deleted: true, units: r.units, cost_cents: r.cost_cents });
});

// ---------- objetivos de ahorro ----------

router.get('/goals', (req, res) => {
  res.json(db.prepare('SELECT * FROM goals WHERE archived = 0 ORDER BY id').all());
});

router.post('/goals', (req, res) => {
  const body = req.body || {};
  const name = String(body.name || '').trim();
  if (!name) return badRequest(res, 'Falta el nombre');
  const target = parseAmountCents(body.target);
  if (!Number.isInteger(target) || target <= 0) return badRequest(res, 'Objetivo no válido');
  const icon = sanitizeIcon(body.icon, '🎯');
  let deadline = null;
  if (body.deadline != null && body.deadline !== '') {
    if (!isValidDate(body.deadline)) return badRequest(res, 'Fecha límite no válida (YYYY-MM-DD)');
    deadline = body.deadline;
  }
  const info = db.prepare('INSERT INTO goals (name, icon, target_cents, deadline) VALUES (?, ?, ?, ?)').run(name, icon, target, deadline);
  res.status(201).json(db.prepare('SELECT * FROM goals WHERE id = ?').get(info.lastInsertRowid));
});

router.put('/goals/:id', (req, res) => {
  const id = Number(req.params.id);
  const existing = db.prepare('SELECT * FROM goals WHERE id = ?').get(id);
  if (!existing) return res.status(404).json({ error: 'Objetivo no encontrado' });
  const body = req.body || {};
  const name = body.name != null ? String(body.name).trim() : existing.name;
  if (!name) return badRequest(res, 'Falta el nombre');
  const icon = body.icon != null ? sanitizeIcon(body.icon, existing.icon) : existing.icon;
  let target = existing.target_cents;
  if (body.target != null) {
    target = parseAmountCents(body.target);
    if (!Number.isInteger(target) || target <= 0) return badRequest(res, 'Objetivo no válido');
  }
  let deadline = existing.deadline;
  if (body.deadline !== undefined) {
    if (body.deadline === null || body.deadline === '') deadline = null;
    else if (isValidDate(body.deadline)) deadline = body.deadline;
    else return badRequest(res, 'Fecha límite no válida (YYYY-MM-DD)');
  }
  db.prepare('UPDATE goals SET name = ?, icon = ?, target_cents = ?, deadline = ? WHERE id = ?').run(name, icon, target, deadline, id);
  res.json(db.prepare('SELECT * FROM goals WHERE id = ?').get(id));
});

// Aportar (o retirar, con importe negativo) dinero a un objetivo.
router.post('/goals/:id/add', (req, res) => {
  const id = Number(req.params.id);
  const existing = db.prepare('SELECT * FROM goals WHERE id = ?').get(id);
  if (!existing) return res.status(404).json({ error: 'Objetivo no encontrado' });
  const delta = parseAmountCents((req.body || {}).amount);
  if (!Number.isInteger(delta) || delta === 0) return badRequest(res, 'Importe no válido');
  const newSaved = existing.saved_cents + delta;
  if (newSaved < 0) return badRequest(res, 'No puedes retirar más de lo ahorrado');
  db.prepare('UPDATE goals SET saved_cents = ? WHERE id = ?').run(newSaved, id);
  res.json(db.prepare('SELECT * FROM goals WHERE id = ?').get(id));
});

router.delete('/goals/:id', (req, res) => {
  const info = db.prepare('DELETE FROM goals WHERE id = ?').run(Number(req.params.id));
  if (info.changes === 0) return res.status(404).json({ error: 'Objetivo no encontrado' });
  res.json({ deleted: true });
});

// ---------- vista anual ----------

router.get('/year', (req, res) => {
  applyRecurring();
  const year = /^\d{4}$/.test(req.query.year) ? req.query.year : todayLocal().slice(0, 4);

  const perMonth = (y) =>
    db
      .prepare(
        `SELECT strftime('%m', date) AS m,
                COALESCE(SUM(CASE WHEN type = 'ingreso' THEN amount_cents END), 0) AS ingresos_cents,
                COALESCE(SUM(CASE WHEN type = 'gasto' THEN amount_cents END), 0) AS gastos_cents
         FROM transactions WHERE strftime('%Y', date) = ? GROUP BY m`
      )
      .all(y);

  const rows = perMonth(year);
  const byM = new Map(rows.map((r) => [Number(r.m), r]));
  const months = [];
  for (let m = 1; m <= 12; m++) {
    const r = byM.get(m) || { ingresos_cents: 0, gastos_cents: 0 };
    months.push({ month: `${year}-${String(m).padStart(2, '0')}`, ingresos_cents: r.ingresos_cents, gastos_cents: r.gastos_cents });
  }
  const total = (arr, k) => arr.reduce((a, r) => a + r[k], 0);

  const prevRows = perMonth(String(Number(year) - 1));
  const byCategory = db
    .prepare(
      `SELECT c.id, c.name, c.icon, c.color, SUM(t.amount_cents) AS total_cents, COUNT(*) AS count
       FROM transactions t LEFT JOIN categories c ON c.id = t.category_id
       WHERE t.type = 'gasto' AND strftime('%Y', t.date) = ?
       GROUP BY t.category_id ORDER BY total_cents DESC`
    )
    .all(year);

  res.json({
    year: Number(year),
    months,
    ingresos_cents: total(months, 'ingresos_cents'),
    gastos_cents: total(months, 'gastos_cents'),
    prev_ingresos_cents: total(prevRows, 'ingresos_cents'),
    prev_gastos_cents: total(prevRows, 'gastos_cents'),
    by_category: byCategory,
  });
});

// ---------- patrimonio (evolución) ----------

router.get('/networth', (req, res) => {
  const openings = db.prepare('SELECT * FROM account_balances').all();
  const snapshots = db.prepare('SELECT * FROM networth_snapshots ORDER BY date').all();
  const today = todayLocal();

  let start = today;
  if (openings.length) start = openings.map((o) => o.opening_date).sort()[0];
  if (snapshots.length && snapshots[0].date < start) start = snapshots[0].date;

  // Serie diaria (semanal si el rango supera ~13 meses para no pasar de ~400 puntos).
  const startMs = new Date(start + 'T12:00:00').getTime();
  const todayMs = new Date(today + 'T12:00:00').getTime();
  const totalDays = Math.max(1, Math.round((todayMs - startMs) / 86400000));
  const stepDays = totalDays > 400 ? 7 : 1;

  const fmt = (ms) => {
    const d = new Date(ms);
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  };
  const dates = [];
  for (let ms = startMs; ms < todayMs; ms += stepDays * 86400000) dates.push(fmt(ms));
  dates.push(today);

  let snapIdx = 0;
  let lastSnap = 0;
  const series = dates.map((date) => {
    let accounts = 0;
    for (const o of openings) {
      if (o.opening_date <= date) {
        accounts += computeAccountBalance(o.name, o.opening_cents, o.opening_date, date, o.apy);
      }
    }
    while (snapIdx < snapshots.length && snapshots[snapIdx].date <= date) {
      lastSnap = snapshots[snapIdx].investments_cents;
      snapIdx += 1;
    }
    return { date, accounts_cents: accounts, investments_cents: lastSnap, total_cents: accounts + lastSnap };
  });
  res.json({ series });
});

// ---------- exportación ----------

router.get('/export.csv', (req, res) => {
  const rows = db.prepare(`${TX_SELECT} ORDER BY t.date ASC, t.id ASC`).all();
  const escape = (v) => {
    const s = String(v == null ? '' : v);
    return /[";\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
  };
  const lines = ['fecha;tipo;importe;categoria;nota;cuenta;origen'];
  for (const r of rows) {
    const amount = (r.amount_cents / 100).toFixed(2).replace('.', ',');
    lines.push(
      [r.date, r.type, amount, r.category_name || '', r.note, r.account, r.source].map(escape).join(';')
    );
  }
  const transferRows = db.prepare('SELECT * FROM transfers ORDER BY date ASC, id ASC').all();
  for (const t of transferRows) {
    const amount = (t.amount_cents / 100).toFixed(2).replace('.', ',');
    lines.push(
      [t.date, 'transferencia', amount, '', t.note, `${t.from_account} → ${t.to_account}`, 'web'].map(escape).join(';')
    );
  }
  res.setHeader('Content-Type', 'text/csv; charset=utf-8');
  res.setHeader('Content-Disposition', 'attachment; filename="finanzillo.csv"');
  res.send('\uFEFF' + lines.join('\n'));
});

module.exports = router;
