'use strict';

const db = require('./db');

const pad = (n) => String(n).padStart(2, '0');

function currentMonth() {
  const d = new Date();
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}`;
}

function nextMonth(month) {
  const [y, m] = month.split('-').map(Number);
  const d = new Date(y, m, 1);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}`;
}

function daysInMonth(month) {
  const [y, m] = month.split('-').map(Number);
  return new Date(y, m, 0).getDate();
}

// Precio EUR/ud de una inversión SIN salir a internet (esto es código síncrono):
// caché de precios → precio manual → precio medio de compra → null.
function cachedPriceEur(inv) {
  if (inv.provider === 'manual') return inv.manual_price;
  const row = db
    .prepare('SELECT price FROM price_cache WHERE key = ?')
    .get(`${inv.provider}:${inv.symbol.toLowerCase()}`);
  if (row) return row.price;
  if (inv.units > 0 && inv.cost_cents > 0) return inv.cost_cents / 100 / inv.units;
  return null;
}

function investCategoryId() {
  const cat = db
    .prepare("SELECT id FROM categories WHERE name = 'Ahorro o Inversiones' COLLATE NOCASE AND type = 'gasto'")
    .get();
  if (cat) return cat.id;
  const fallback = db
    .prepare("SELECT id FROM categories WHERE name = 'Otros' COLLATE NOCASE AND type = 'gasto'")
    .get();
  return fallback ? fallback.id : null;
}

/**
 * Materializa los recurrentes pendientes: para cada regla activa, ejecuta una
 * vez por cada mes desde su último aplicado hasta hoy, siempre que el día
 * configurado ya haya llegado (día 31 → último día en meses cortos).
 * Idempotente: se puede llamar en cada request.
 *
 * Tipos:
 * - movimiento: inserta un gasto/ingreso (source 'recurrente').
 * - traspaso:   inserta una transferencia from_account → to_account.
 * - aportacion: compra participaciones de la inversión al precio cacheado
 *               (units += importe/precio, cost += importe) y, si hay cuenta
 *               de origen, apunta el gasto en "Ahorro o Inversiones".
 */
function applyRecurring() {
  const rules = db.prepare('SELECT * FROM recurring WHERE active = 1').all();
  if (rules.length === 0) return 0;

  const today = new Date();
  const nowMonth = currentMonth();
  const insertTx = db.prepare(
    "INSERT INTO transactions (type, amount_cents, category_id, note, account, date, source) VALUES (?, ?, ?, ?, ?, ?, 'recurrente')"
  );
  const insertTransfer = db.prepare(
    'INSERT INTO transfers (amount_cents, from_account, to_account, note, date) VALUES (?, ?, ?, ?, ?)'
  );
  const updateRule = db.prepare('UPDATE recurring SET last_applied = ? WHERE id = ?');
  const updateInv = db.prepare('UPDATE investments SET units = units + ?, cost_cents = cost_cents + ? WHERE id = ?');
  const insertOp = db.prepare(
    "INSERT INTO investment_ops (investment_id, type, units, amount_cents, date) VALUES (?, 'compra', ?, ?, ?)"
  );
  const deactivate = db.prepare('UPDATE recurring SET active = 0 WHERE id = ?');

  let created = 0;
  for (const rule of rules) {
    let month = rule.last_applied ? nextMonth(rule.last_applied) : rule.start_month;
    while (month <= nowMonth) {
      const day = Math.min(rule.day_of_month, daysInMonth(month));
      if (month === nowMonth && today.getDate() < day) break;
      const date = `${month}-${pad(day)}`;

      if (rule.kind === 'traspaso') {
        insertTransfer.run(rule.amount_cents, rule.from_account, rule.to_account, rule.note, date);
      } else if (rule.kind === 'aportacion') {
        const inv = db
          .prepare('SELECT * FROM investments WHERE id = ? AND archived = 0')
          .get(rule.investment_id);
        if (!inv) {
          // La inversión ya no existe: se pausa la regla en vez de fallar para siempre.
          deactivate.run(rule.id);
          break;
        }
        const price = cachedPriceEur(inv);
        const unitsToAdd = price > 0 ? rule.amount_cents / 100 / price : 0;
        updateInv.run(unitsToAdd, rule.amount_cents, inv.id);
        // Queda registrada en el historial de operaciones de la inversión.
        if (unitsToAdd > 0) insertOp.run(inv.id, unitsToAdd, rule.amount_cents, date);
        if (rule.from_account) {
          const note = rule.note || `Aportación ${inv.name}`;
          insertTx.run('gasto', rule.amount_cents, investCategoryId(), note, rule.from_account, date);
        }
      } else {
        insertTx.run(rule.type, rule.amount_cents, rule.category_id, rule.note, '', date);
      }

      updateRule.run(month, rule.id);
      created += 1;
      month = nextMonth(month);
    }
  }
  return created;
}

module.exports = { applyRecurring, currentMonth };
