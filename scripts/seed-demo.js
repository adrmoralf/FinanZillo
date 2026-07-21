'use strict';
/* Rellena la base con datos de DEMOSTRACIÓN (inventados) para poder probar la
 * app, hacer capturas o enseñarla sin usar datos reales.
 *
 *   npm run seed:demo
 *
 * Trabaja siempre sobre `data/demo.db`, NUNCA sobre `data/finanzillo.db`: así
 * es imposible que pise datos de verdad por accidente. Para arrancar la app
 * contra la demo:
 *
 *   DB_FILE=demo.db npm start          (bash)
 *   $env:DB_FILE="demo.db"; npm start  (PowerShell)
 */

process.env.DB_FILE = process.env.DB_FILE || 'demo.db';

const path = require('path');
const fs = require('fs');
const config = require('../src/config');

const destino = path.join(config.DATA_DIR, process.env.DB_FILE);
if (path.basename(destino) === 'finanzillo.db') {
  console.error('Abortado: el seed de demo no debe escribir sobre finanzillo.db.');
  process.exit(1);
}
// Empezamos de cero cada vez para que la demo sea reproducible.
for (const suf of ['', '-wal', '-shm']) {
  try { fs.unlinkSync(destino + suf); } catch (_) { /* no existía */ }
}

const db = require('../src/db');

const hoy = new Date();
const iso = (d) => d.toISOString().slice(0, 10);
const diasAtras = (n) => { const d = new Date(hoy); d.setDate(d.getDate() - n); return iso(d); };

const CUENTAS = [
  { name: 'Banco', opening: 250000, apy: 0 },
  { name: 'Cuenta remunerada', opening: 400000, apy: 2.5 },
  { name: 'Efectivo', opening: 4000, apy: 0 },
];

// [categoría, nota, céntimos, cuenta, hace N días]
const GASTOS = [
  ['Alimentación', 'Compra semanal', 5240, 'Banco', 1],
  ['Restaurante', 'Comida con amigos', 2350, 'Banco', 2],
  ['Transporte', 'Abono mensual', 3500, 'Banco', 4],
  ['Ocio', 'Cine', 1100, 'Efectivo', 5],
  ['Alimentación', 'Frutería', 1870, 'Efectivo', 6],
  ['Suscripciones', 'Música', 1099, 'Banco', 8],
  ['Vivienda', 'Alquiler', 65000, 'Banco', 10],
  ['Salud', 'Farmacia', 1420, 'Banco', 12],
  ['Alimentación', 'Compra semanal', 4890, 'Banco', 13],
  ['Ropa', 'Camiseta', 1999, 'Banco', 16],
  ['Restaurante', 'Cena', 3120, 'Banco', 18],
  ['Transporte', 'Gasolina', 5500, 'Banco', 21],
  ['Ocio', 'Concierto', 4500, 'Banco', 24],
  ['Alimentación', 'Compra semanal', 5110, 'Banco', 27],
  ['Suscripciones', 'Vídeo', 1299, 'Banco', 30],
  ['Viajes', 'Tren fin de semana', 7800, 'Banco', 34],
  ['Alimentación', 'Compra semanal', 4750, 'Banco', 38],
  ['Vivienda', 'Alquiler', 65000, 'Banco', 40],
  ['Regalos', 'Cumpleaños', 3000, 'Efectivo', 44],
  ['Restaurante', 'Menú del día', 1250, 'Efectivo', 47],
];

const INGRESOS = [
  ['Nómina', 'Nómina', 185000, 'Banco', 3],
  ['Nómina', 'Nómina', 185000, 'Banco', 33],
  ['Extra', 'Venta de segunda mano', 4500, 'Efectivo', 20],
  ['Intereses', 'Intereses del mes', 830, 'Cuenta remunerada', 5],
];

const idCategoria = (nombre, tipo) => {
  const row = db.prepare('SELECT id FROM categories WHERE name = ? AND type = ?').get(nombre, tipo);
  return row ? row.id : null;
};

const insertTx = db.prepare(
  `INSERT INTO transactions (type, amount_cents, category_id, note, account, date, source)
   VALUES (?, ?, ?, ?, ?, ?, 'demo')`
);
const insertCuenta = db.prepare(
  'INSERT OR REPLACE INTO account_balances (name, opening_cents, opening_date, apy) VALUES (?, ?, ?, ?)'
);

for (const c of CUENTAS) insertCuenta.run(c.name, c.opening, diasAtras(60), c.apy);
for (const [cat, nota, cents, cuenta, dias] of GASTOS) {
  insertTx.run('gasto', cents, idCategoria(cat, 'gasto'), nota, cuenta, diasAtras(dias));
}
for (const [cat, nota, cents, cuenta, dias] of INGRESOS) {
  insertTx.run('ingreso', cents, idCategoria(cat, 'ingreso'), nota, cuenta, diasAtras(dias));
}

db.prepare('INSERT OR REPLACE INTO app_settings (key, value) VALUES (?, ?)').run('global_budget_cents', '120000');
db.prepare('INSERT OR REPLACE INTO app_settings (key, value) VALUES (?, ?)').run('default_account', 'Banco');
db.prepare('UPDATE categories SET budget_cents = 30000 WHERE name = ? AND type = ?').run('Alimentación', 'gasto');
db.prepare(
  'INSERT INTO goals (name, icon, target_cents, saved_cents, deadline) VALUES (?, ?, ?, ?, ?)'
).run('Viaje', '✈️', 150000, 45000, diasAtras(-120));

const n = db.prepare('SELECT COUNT(*) AS n FROM transactions').get().n;
console.log(`Datos de demostración creados en ${destino}`);
console.log(`  ${n} movimientos, ${CUENTAS.length} cuentas, 1 objetivo, presupuesto global 1.200 €`);
console.log('');
console.log('Arranca la app contra esta base:');
console.log('  DB_FILE=demo.db npm start            (bash)');
console.log('  $env:DB_FILE="demo.db"; npm start    (PowerShell)');
