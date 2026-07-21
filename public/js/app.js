'use strict';

/* ===== Utilidades ===== */

const $ = (sel) => document.querySelector(sel);

const fmtEUR = new Intl.NumberFormat('es-ES', { style: 'currency', currency: 'EUR' });
const eur = (cents) => fmtEUR.format(cents / 100);

const fmtMonthLong = new Intl.DateTimeFormat('es-ES', { month: 'long', year: 'numeric' });
const fmtMonthName = new Intl.DateTimeFormat('es-ES', { month: 'long' });
const fmtDateLong = new Intl.DateTimeFormat('es-ES', { weekday: 'long', day: 'numeric', month: 'long' });
const fmtMonthShort = new Intl.DateTimeFormat('es-ES', { month: 'short' });

const cap = (s) => s.charAt(0).toUpperCase() + s.slice(1);

function todayISO() {
  const d = new Date();
  const p = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

// Variante para tema oscuro de cada color de la paleta categórica.
const DARK_MAP = {
  '#2a78d6': '#3987e5', '#1baf7a': '#199e70', '#eda100': '#c98500',
  '#008300': '#008300', '#4a3aa7': '#9085e9', '#e34948': '#e66767',
  '#e87ba4': '#d55181', '#eb6834': '#d95926', '#1c5cab': '#3987e5',
  '#c98500': '#eda100',
};
const PALETTE = ['#2a78d6', '#1baf7a', '#eda100', '#008300', '#4a3aa7', '#e34948', '#e87ba4', '#eb6834', '#1c5cab', '#c98500', '#898781'];

function isDark() {
  return document.documentElement.dataset.theme === 'dark';
}
function catColor(hex) {
  const h = (hex || '#898781').toLowerCase();
  return isDark() ? (DARK_MAP[h] || h) : h;
}
function cssVar(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/* ===== Estado ===== */

const state = {
  month: todayISO().slice(0, 7),
  categories: [],
  summary: null,
  editingTx: null,
  editingCat: null,
  txType: 'gasto',
  catType: 'gasto',
  catColor: PALETTE[0],
  accounts: new Set(),
  accountBalances: [],
  editingAccount: null,
  transfers: [],
  investments: null,
  editingInv: null,
  goals: [],
  editingGoal: null,
  networth: null,
  globalBudgetCents: 0,
  defaultAccount: '',
  year: new Date().getFullYear(),
  yearData: null,
  templates: [],
  privacy: false,
};

function noteAccounts(items) {
  for (const tx of items) if (tx.account) state.accounts.add(tx.account);
}

/* Cuentas como desplegable: opciones = cuentas conocidas + "Otra cuenta…".
   Elegir "Otra cuenta…" revela el input de texto asociado para teclear una nueva. */
const OTHER_ACCOUNT = '__other__';

function knownAccountNames() {
  const names = new Map(); // minúsculas -> nombre a mostrar
  for (const a of state.accountBalances) names.set(a.name.toLowerCase(), a.name);
  for (const a of state.accounts) if (!names.has(a.toLowerCase())) names.set(a.toLowerCase(), a);
  return [...names.values()].sort((a, b) => a.localeCompare(b, 'es'));
}

function fillAccountSelect(sel, newInput, selected = '', { includeBlank = false } = {}) {
  sel.innerHTML = '';
  if (includeBlank) {
    const o = document.createElement('option');
    o.value = '';
    o.textContent = '(ninguna)';
    sel.appendChild(o);
  }
  const names = knownAccountNames();
  if (selected && !names.some((n) => n.toLowerCase() === selected.toLowerCase())) names.unshift(selected);
  for (const name of names) {
    const o = document.createElement('option');
    o.value = name;
    o.textContent = name;
    sel.appendChild(o);
  }
  const other = document.createElement('option');
  other.value = OTHER_ACCOUNT;
  other.textContent = 'Otra cuenta…';
  sel.appendChild(other);
  if (selected) sel.value = names.find((n) => n.toLowerCase() === selected.toLowerCase()) || selected;
  else sel.value = includeBlank ? '' : names[0] || OTHER_ACCOUNT;
  syncAccountNewInput(sel, newInput);
}

function syncAccountNewInput(sel, newInput) {
  const isOther = sel.value === OTHER_ACCOUNT;
  newInput.classList.toggle('hidden', !isOther);
  if (!isOther) newInput.value = '';
}

function accountValue(sel, newInput) {
  return sel.value === OTHER_ACCOUNT ? newInput.value.trim() : sel.value;
}

// Cablea un par select+input para que "Otra cuenta…" muestre el input.
function wireAccountSelect(selId, newId) {
  const sel = $(selId);
  const inp = $(newId);
  sel.addEventListener('change', () => {
    syncAccountNewInput(sel, inp);
    if (sel.value === OTHER_ACCOUNT) inp.focus();
  });
}

let chartMonthly = null;
let chartDonut = null;
let chartNetworth = null;
let chartPace = null;
let chartWeekday = null;
let chartCatDetail = null;
const sparkCharts = new Map();

/* ===== API ===== */

async function api(path, options = {}) {
  // Si ya sabemos que el servidor no está, no se pide a la red: cada petición
  // tardaba ~2,2 s en rendirse y el arranque sin conexión se iba a ~7 s con la
  // app a medio pintar. Vamos directos a la copia del service worker.
  const metodo = (options.method || 'GET').toUpperCase();
  if (!serverUp && metodo === 'GET' && 'caches' in window) {
    const guardada = await caches.match(new URL('api' + path, location.href).href);
    if (guardada) return guardada.json();
    throw new Error('Sin conexión');
  }
  const res = await fetch('api' + path, {
    headers: { 'Content-Type': 'application/json' },
    ...options,
  });
  if (res.status === 401) {
    showLogin();
    throw new Error('No autorizado');
  }
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || `Error ${res.status}`);
  return data;
}

/* ===== Modo offline =====
   El service worker (sw.js) sirve la app y los últimos datos vistos sin red.
   Aquí va la otra mitad: los movimientos creados sin servidor se guardan en
   localStorage y se mandan en lote cuando vuelve la conexión. */

const QUEUE_KEY = 'mg-queue';
const WAS_AUTH_KEY = 'mg-authed';

// fetch() lanza TypeError cuando no hay servidor; el resto son errores de la API.
function isNetworkError(err) {
  return err instanceof TypeError || err.message === 'Sin conexión';
}

function readQueue() {
  try {
    const q = JSON.parse(localStorage.getItem(QUEUE_KEY));
    return Array.isArray(q) ? q : [];
  } catch (_) {
    return [];
  }
}

function writeQueue(items) {
  localStorage.setItem(QUEUE_KEY, JSON.stringify(items));
  renderOfflineBar();
}

/* Copia local de los datos de referencia (categorías, cuentas, ajustes).
   La caché del service worker solo guarda respuestas que ya se pidieron alguna
   vez; si está fría, el modal de "nuevo gasto" se quedaba sin categorías ni
   cuentas y no se podía apuntar nada. Con esto, en cuanto la app carga bien una
   vez, esos datos quedan disponibles para siempre sin servidor. */
const LOCAL_KEYS = {
  categories: 'mg-cache-categories',
  accounts: 'mg-cache-accounts',
  settings: 'mg-cache-settings',
};

function saveLocal(key, data) {
  try {
    localStorage.setItem(key, JSON.stringify(data));
  } catch (_) {
    /* almacenamiento lleno o bloqueado: no es crítico */
  }
}

function readLocal(key, fallback) {
  try {
    const v = JSON.parse(localStorage.getItem(key));
    return v === null || v === undefined ? fallback : v;
  } catch (_) {
    return fallback;
  }
}

/* Estado del servidor (LED de la topbar).
   `navigator.onLine` solo mira si hay red, no si el servidor responde: con el
   wifi puesto y el servidor apagado diría "conectado". Por eso se sondea
   /api/ping, que el service worker no cachea a propósito. */
let serverUp = true;
let pinging = false;

function renderLed() {
  const led = $('#server-led');
  if (!led) return;
  led.classList.toggle('up', serverUp);
  led.classList.toggle('down', !serverUp);
  const txt = serverUp ? 'Servidor conectado' : 'Servidor apagado — los gastos se guardan y se envían al volver';
  led.title = txt;
  led.setAttribute('aria-label', txt);
}

async function pingServer() {
  if (pinging) return;
  pinging = true;
  const antes = serverUp;
  try {
    // Se sondea /api/me, no /api/ping: `me` no exige sesión y siempre responde
    // 200, así que sirve de latido sin dejar un 401 en la consola cada 15 s.
    // Basta con que el servidor conteste; lo que conteste da igual.
    await fetch('api/me', { cache: 'no-store' });
    serverUp = true;
  } catch (_) {
    serverUp = false;
  } finally {
    pinging = false;
  }
  if (serverUp !== antes) {
    renderLed();
    renderOfflineBar();
    if (serverUp) flushQueue(); // ha vuelto: manda lo pendiente sin esperar a recargar
  }
}

function renderOfflineBar() {
  const bar = $('#offline-bar');
  if (!bar) return;
  const pend = readQueue().length;
  const offline = !navigator.onLine || !serverUp;
  bar.classList.toggle('hidden', !offline && pend === 0);
  if (offline) {
    bar.textContent = pend
      ? `Sin conexión · ${pend} ${pend === 1 ? 'movimiento pendiente' : 'movimientos pendientes'} de enviar`
      : 'Sin conexión · viendo los últimos datos guardados';
  } else {
    bar.textContent = `Enviando ${pend} ${pend === 1 ? 'movimiento' : 'movimientos'}…`;
  }
}

// Crear un movimiento nuevo: si no hay servidor, a la cola local.
async function createTxMaybeOffline(body) {
  // Si ya sabemos que no hay servidor, a la cola directamente: intentar el POST
  // solo para que agote el tiempo de espera dejaría el modal 2 s congelado.
  // Si el servidor hubiera vuelto sin que lo sepamos, el siguiente sondeo lo
  // manda igual (y el batch es idempotente, así que no hay riesgo de duplicar).
  if (!serverUp) {
    writeQueue([...readQueue(), body]);
    return;
  }
  try {
    await api('/transactions', { method: 'POST', body: JSON.stringify(body) });
  } catch (err) {
    if (!isNetworkError(err)) throw err;
    // Acabamos de comprobar en directo que no hay servidor: el LED no espera al sondeo.
    serverUp = false;
    renderLed();
    writeQueue([...readQueue(), body]);
  }
}

let flushing = false;

/* Vuelca la cola con /transactions/batch y skip_duplicates: reenviar lo mismo
   dos veces (misma fecha+importe+tipo+nota) nunca duplica, así que un fallo a
   medias se puede reintentar sin miedo. */
async function flushQueue() {
  const items = readQueue();
  if (flushing || !items.length || !navigator.onLine) return;
  flushing = true;
  renderOfflineBar();
  try {
    const r = await api('/transactions/batch', {
      method: 'POST',
      body: JSON.stringify({ items, skip_duplicates: true }),
    });
    const errs = r.errors || [];
    // Lo que dio error de validación no se arreglará reintentando: se descarta
    // avisando, para no dejar la cola atascada para siempre.
    writeQueue([]);
    if (errs.length) {
      alert(`Se enviaron ${r.created} movimientos. ${errs.length} no se pudieron guardar: ${errs[0].error}`);
    }
    await refreshAll();
  } catch (err) {
    if (!isNetworkError(err)) {
      // Error real del servidor (p. ej. sesión caducada): se conserva la cola.
      console.warn('No se pudo vaciar la cola:', err.message);
    }
  } finally {
    flushing = false;
    renderOfflineBar();
  }
}

window.addEventListener('online', () => {
  renderOfflineBar();
  pingServer();
});
window.addEventListener('offline', () => {
  serverUp = false;
  renderLed();
  renderOfflineBar();
});

// Sondeo periódico + al volver a la pestaña (caso típico: reiniciar el servidor
// y volver al navegador, que debe reflejarlo sin recargar).
setInterval(pingServer, 15000);
document.addEventListener('visibilitychange', () => {
  if (!document.hidden) pingServer();
});

if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('sw.js').catch((e) => console.warn('SW no registrado:', e.message));
  });
}

/* ===== Login ===== */

function showLogin() {
  $('#app').classList.add('hidden');
  $('#view-login').classList.remove('hidden');
  $('#login-password').focus();
}

/* Sin servidor, cada carga que falle debe quedarse en su sitio y no arrastrar a
   las demás: antes un fallo de /categories abortaba showApp() entero y la app
   se quedaba sin cuentas, sin categorías y sin movimientos. */
const sinRomper = (p) => Promise.resolve(p).catch(() => {});

async function showApp() {
  $('#view-login').classList.add('hidden');
  $('#app').classList.remove('hidden');
  // En paralelo: son independientes y en serie sumaban ~4,7 s sin conexión.
  await Promise.all([loadCategories(), loadSettingsApp(), loadAccounts()].map(sinRomper));
  await Promise.all(
    [refreshResumen(), refreshMovimientos(), loadRecurring(), loadGoals(), loadTemplates()].map(sinRomper)
  );
  renderCategorias();
  loadAjustes();
  loadNetworth();
  // Las inversiones pueden tardar (consultan precios): sin bloquear el arranque.
  loadInvestments();
}

async function loadSettingsApp() {
  let s = null;
  try {
    s = await api('/settings');
    saveLocal(LOCAL_KEYS.settings, s);
  } catch (_) {
    // Sin sesión aún, o sin servidor: se tira de la última copia conocida para
    // que la cuenta predeterminada siga funcionando offline.
    s = readLocal(LOCAL_KEYS.settings, null);
  }
  if (!s) return;
  state.globalBudgetCents = s.global_budget_cents || 0;
  state.defaultAccount = s.default_account || '';
  state.apiTokenValue = s.api_token;
}

$('#login-form').addEventListener('submit', async (e) => {
  e.preventDefault();
  const errEl = $('#login-error');
  errEl.classList.add('hidden');
  try {
    await api('/login', {
      method: 'POST',
      body: JSON.stringify({ password: $('#login-password').value }),
    });
    $('#login-password').value = '';
    localStorage.setItem(WAS_AUTH_KEY, '1');
    await showApp();
    flushQueue();
  } catch (err) {
    errEl.textContent = err.message;
    errEl.classList.remove('hidden');
  }
});

/* ===== Tema ===== */

function applyTheme(theme) {
  document.documentElement.dataset.theme = theme;
  localStorage.setItem('mg-theme', theme);
  if (state.summary) {
    renderCharts();
    renderPaceChart();
    renderHeatmap();
    renderWeekdayChart();
  }
  renderCatLegend();
  if (state.investments) renderInvestments();
}
$('#theme-toggle').addEventListener('click', () => {
  applyTheme(isDark() ? 'light' : 'dark');
});
{
  const saved = localStorage.getItem('mg-theme');
  const preferred = saved || (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
  document.documentElement.dataset.theme = preferred;
}

/* ===== Modo privacidad (difumina los importes) ===== */

function applyPrivacy(on) {
  state.privacy = on;
  document.body.classList.toggle('privacy', on);
  localStorage.setItem('mg-privacy', on ? '1' : '0');
  $('#privacy-toggle').textContent = on ? '🙈' : '👁️';
  $('#privacy-toggle').title = on ? 'Mostrar importes' : 'Ocultar importes';
}
$('#privacy-toggle').addEventListener('click', () => applyPrivacy(!state.privacy));
applyPrivacy(localStorage.getItem('mg-privacy') === '1');

/* ===== Pestañas ===== */

$('#tabs').addEventListener('click', (e) => {
  const btn = e.target.closest('.tab');
  if (!btn) return;
  document.querySelectorAll('.tab').forEach((t) => t.classList.toggle('active', t === btn));
  document.querySelectorAll('.view').forEach((v) => v.classList.add('hidden'));
  $('#view-' + btn.dataset.view).classList.remove('hidden');
  // En móvil la barra se desliza: deja la pestaña elegida centrada. Se mueve
  // SOLO el contenedor — scrollIntoView() arrastraría también al documento y
  // desplazaba la página entera en horizontal.
  const tabs = $('#tabs');
  tabs.scrollTo({ left: btn.offsetLeft - (tabs.clientWidth - btn.offsetWidth) / 2, behavior: 'smooth' });
  window.scrollTo({ top: 0, behavior: 'smooth' });
  if (btn.dataset.view === 'anual' && !state.yearData) refreshYear();
  if (btn.dataset.view === 'inversiones' && !state.investments) loadInvestments();
  if (btn.dataset.view === 'recurrentes') loadSuggestions();
});

/* ===== Categorías (datos) ===== */

async function loadCategories() {
  try {
    state.categories = await api('/categories?all=1');
    saveLocal(LOCAL_KEYS.categories, state.categories);
  } catch (err) {
    if (!isNetworkError(err)) throw err;
    state.categories = readLocal(LOCAL_KEYS.categories, []);
  }
}
const activeCategories = (type) =>
  state.categories.filter((c) => c.type === type && !c.archived);

/* ===== Resumen ===== */

$('#month-prev').addEventListener('click', () => shiftMonth(-1));
$('#month-next').addEventListener('click', () => shiftMonth(1));

function shiftMonth(delta) {
  const [y, m] = state.month.split('-').map(Number);
  const d = new Date(y, m - 1 + delta, 1);
  state.month = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`;
  refreshResumen();
}

async function refreshResumen() {
  const s = await api('/summary?month=' + state.month);
  state.summary = s;

  $('#month-label').textContent = cap(fmtMonthLong.format(new Date(state.month + '-01T12:00:00')));
  $('#stat-ingresos').textContent = eur(s.ingresos_cents);
  $('#stat-gastos').textContent = eur(s.gastos_cents);
  const balance = $('#stat-balance');
  balance.textContent = (s.balance_cents > 0 ? '+' : '') + eur(s.balance_cents);
  balance.classList.toggle('positive', s.balance_cents > 0);
  balance.classList.toggle('negative', s.balance_cents < 0);

  renderDelta($('#delta-ingresos'), s.ingresos_cents, s.prev_ingresos_cents, s.prev_month, true);
  renderDelta($('#delta-gastos'), s.gastos_cents, s.prev_gastos_cents, s.prev_month, false);

  renderStatsStrip(s);
  renderInsights(s);
  renderCharts();
  renderPaceChart();
  renderHeatmap();
  renderWeekdayChart();
  renderCatLegend();
  renderBudgets();
  renderTxList($('#recent-list'), s.recent, { grouped: false });
}

/* Insights automáticos comparando con el mes anterior (solo si hay con qué comparar). */
function renderInsights(s) {
  const card = $('#insights-card');
  const list = $('#insights-list');
  const msgs = [];
  const prevName = cap(fmtMonthName.format(new Date(s.prev_month + '-01T12:00:00')));

  // Cada mensaje es una lista de partes: texto plano (string) o negrita ({ b: '…' }).
  if (s.prev_gastos_cents > 0 && s.gastos_cents > 0) {
    const diff = s.gastos_cents - s.prev_gastos_cents;
    const pct = Math.round((diff / s.prev_gastos_cents) * 100);
    if (Math.abs(pct) >= 1) {
      msgs.push({ icon: diff < 0 ? '📉' : '📈', parts: ['Gastas un ', { b: `${Math.abs(pct)}%` }, ` ${diff < 0 ? 'menos' : 'más'} que en ${prevName}`] });
    }
  }

  // Categoría que más sube respecto al mes anterior.
  const prevBy = new Map((s.prev_by_category || []).map((c) => [c.id, c.total_cents]));
  let biggest = null;
  for (const c of s.by_category || []) {
    const before = prevBy.get(c.id) || 0;
    const delta = c.total_cents - before;
    if (before > 0 && delta > 0 && (!biggest || delta > biggest.delta)) {
      biggest = { name: c.name, icon: c.icon, pct: Math.round((delta / before) * 100), delta };
    }
  }
  if (biggest && biggest.pct >= 15) {
    msgs.push({ icon: biggest.icon || '🔺', parts: [{ b: biggest.name }, ' sube un ', { b: `${biggest.pct}%` }, ` vs ${prevName}`] });
  }

  // Mayor categoría del mes.
  if ((s.by_category || []).length > 0 && s.gastos_cents > 0) {
    const top = s.by_category[0];
    const pct = Math.round((top.total_cents / s.gastos_cents) * 100);
    msgs.push({ icon: top.icon || '🥇', parts: ['Tu mayor gasto es ', { b: top.name }, ` (${pct}% del total)`] });
  }

  card.classList.toggle('hidden', msgs.length === 0);
  list.innerHTML = '';
  for (const m of msgs) {
    const li = document.createElement('li');
    const ico = document.createElement('span');
    ico.textContent = m.icon + ' ';
    li.appendChild(ico);
    for (const part of m.parts) {
      if (typeof part === 'string') {
        li.appendChild(document.createTextNode(part));
      } else {
        const b = document.createElement('b');
        b.textContent = part.b; // datos de usuario (nombre de categoría) vía textContent
        li.appendChild(b);
      }
    }
    list.appendChild(li);
  }
}

/* Gasto acumulado día a día del mes vs el mismo tramo del mes anterior. */
function renderPaceChart() {
  const s = state.summary;
  const grid = cssVar('--grid');
  const muted = cssVar('--muted');
  const accent = cssVar('--accent');
  const [y, m] = s.month.split('-').map(Number);
  const days = new Date(y, m, 0).getDate();
  const [py, pm] = s.prev_month.split('-').map(Number);
  const prevDays = new Date(py, pm, 0).getDate();

  const cumulative = (daily, n) => {
    const perDay = new Map(daily.map((d) => [Number(d.date.slice(8, 10)), d.cents]));
    const out = [];
    let acc = 0;
    for (let d = 1; d <= n; d++) { acc += perDay.get(d) || 0; out.push(acc / 100); }
    return out;
  };
  const isCurrent = s.month === todayISO().slice(0, 7);
  const todayDay = new Date().getDate();
  const cur = cumulative(s.daily, days).map((v, i) => (isCurrent && i + 1 > todayDay ? null : v));
  const prev = cumulative(s.prev_daily, prevDays);

  const labels = Array.from({ length: days }, (_, i) => i + 1);
  if (chartPace) chartPace.destroy();
  chartPace = new Chart($('#chart-pace'), {
    type: 'line',
    data: {
      labels,
      datasets: [
        { label: 'Este mes', data: cur, borderColor: accent, backgroundColor: accent, borderWidth: 2, pointRadius: 0, pointHoverRadius: 3, tension: 0.2, spanGaps: false },
        { label: cap(fmtMonthName.format(new Date(s.prev_month + '-01T12:00:00'))), data: prev, borderColor: muted, backgroundColor: muted, borderWidth: 1.5, borderDash: [4, 4], pointRadius: 0, pointHoverRadius: 3, tension: 0.2 },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      interaction: { mode: 'index', intersect: false },
      plugins: {
        legend: { position: 'top', align: 'end', labels: { usePointStyle: true, pointStyle: 'circle', boxWidth: 7, boxHeight: 7, color: muted } },
        tooltip: { callbacks: { title: (items) => `Día ${items[0].label}`, label: (ctx) => ` ${ctx.dataset.label}: ${fmtEUR.format(ctx.parsed.y)}` } },
      },
      scales: {
        x: { grid: { display: false }, ticks: { maxTicksLimit: 8, color: muted } },
        y: { beginAtZero: true, grid: { color: grid, drawTicks: false }, border: { display: false }, ticks: { maxTicksLimit: 4, color: muted, callback: (v) => (v >= 1000 ? `${(v / 1000).toFixed(1).replace('.0', '')}k` : v) } },
      },
    },
  });
}

/* Calendario de gasto: una celda por día del mes, intensidad según lo gastado. */
function renderHeatmap() {
  const s = state.summary;
  const el = $('#heatmap');
  el.innerHTML = '';
  const [y, m] = s.month.split('-').map(Number);
  const days = new Date(y, m, 0).getDate();
  const firstDow = (new Date(y, m - 1, 1).getDay() + 6) % 7; // lunes = 0
  const perDay = new Map(s.daily.map((d) => [Number(d.date.slice(8, 10)), d.cents]));
  const max = Math.max(1, ...s.daily.map((d) => d.cents));
  const isCurrent = s.month === todayISO().slice(0, 7);
  const todayDay = new Date().getDate();
  const accent = cssVar('--accent');

  for (const name of ['L', 'M', 'X', 'J', 'V', 'S', 'D']) {
    const h = document.createElement('div');
    h.className = 'hm-head';
    h.textContent = name;
    el.appendChild(h);
  }
  for (let i = 0; i < firstDow; i++) {
    const c = document.createElement('div');
    c.className = 'hm-cell empty';
    el.appendChild(c);
  }
  for (let d = 1; d <= days; d++) {
    const cents = perDay.get(d) || 0;
    const c = document.createElement('div');
    c.className = 'hm-cell';
    if (isCurrent && d > todayDay) c.classList.add('future');
    if (cents > 0) {
      c.classList.add('spent');
      const t = 0.2 + 0.8 * (cents / max);
      c.style.background = `color-mix(in srgb, ${accent} ${Math.round(t * 100)}%, transparent)`;
      c.title = `${d}: ${eur(cents)}`;
    }
    c.textContent = String(d);
    el.appendChild(c);
  }
}

/* Gasto medio por día de la semana (lunes→domingo). */
function renderWeekdayChart() {
  const s = state.summary;
  const muted = cssVar('--muted');
  const grid = cssVar('--grid');
  const accent = cssVar('--accent');
  const sums = new Array(7).fill(0);
  const counts = new Array(7).fill(0);
  for (const d of s.daily) {
    const dow = (new Date(d.date + 'T12:00:00').getDay() + 6) % 7;
    sums[dow] += d.cents;
    counts[dow] += 1;
  }
  const avg = sums.map((v, i) => (counts[i] ? v / counts[i] / 100 : 0));
  if (chartWeekday) chartWeekday.destroy();
  chartWeekday = new Chart($('#chart-weekday'), {
    type: 'bar',
    data: {
      labels: ['Lun', 'Mar', 'Mié', 'Jue', 'Vie', 'Sáb', 'Dom'],
      datasets: [{ data: avg, backgroundColor: accent, borderRadius: 5, borderSkipped: 'bottom' }],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: { display: false },
        tooltip: { callbacks: { label: (ctx) => ` Media: ${fmtEUR.format(ctx.parsed.y)}` } },
      },
      scales: {
        x: { grid: { display: false }, ticks: { color: muted } },
        y: { beginAtZero: true, grid: { color: grid, drawTicks: false }, border: { display: false }, ticks: { maxTicksLimit: 3, color: muted, callback: (v) => (v >= 1000 ? `${(v / 1000).toFixed(1).replace('.0', '')}k` : v) } },
      },
    },
  });
}

/* Mini-estadísticas del mes: proyección, media diaria, días sin gastar, fijos/variables. */
function renderStatsStrip(s) {
  const [y, m] = s.month.split('-').map(Number);
  const daysInMonth = new Date(y, m, 0).getDate();
  const isCurrent = s.month === todayISO().slice(0, 7);
  const elapsed = isCurrent ? new Date().getDate() : daysInMonth;

  const media = elapsed > 0 ? s.gastos_cents / elapsed : 0;
  $('#ms-media').textContent = eur(Math.round(media));

  const proy = $('#ms-proyeccion');
  proy.classList.remove('bad', 'good');
  if (isCurrent && s.gastos_cents > 0) {
    // Extrapola solo el gasto variable (el ya materializado por recurrentes no se
    // repite) y suma los recurrentes que aún no han caído este mes.
    const variable = Math.max(0, s.gastos_cents - (s.gastos_recurrentes_cents || 0));
    const variableDaily = elapsed > 0 ? variable / elapsed : 0;
    const remaining = Math.max(0, daysInMonth - elapsed);
    const projected = Math.round(s.gastos_cents + variableDaily * remaining + (s.pending_recurring_cents || 0));
    proy.textContent = eur(projected);
    const budgetMonth = s.global_budget_month_cents || state.globalBudgetCents;
    if (budgetMonth > 0) {
      proy.classList.add(projected > budgetMonth ? 'bad' : 'good');
    }
  } else {
    proy.textContent = '—';
  }

  $('#ms-singastar').textContent = `${Math.max(0, elapsed - s.spend_days)} de ${elapsed}`;

  const fixedIds = new Set(state.categories.filter((c) => c.fixed).map((c) => c.id));
  const fijos = (s.by_category || []).reduce((a, c) => a + (fixedIds.has(c.id) ? c.total_cents : 0), 0);
  $('#ms-fijos').textContent = s.gastos_cents > 0 ? `${eur(fijos)} · ${eur(s.gastos_cents - fijos)}` : '—';
}

/* Comparativa con el mes anterior: para ingresos subir es bueno; para gastos, malo. */
function renderDelta(el, current, previous, prevMonth, upIsGood) {
  el.textContent = '';
  el.classList.remove('good', 'bad');
  if (!previous || (!current && !previous)) return;
  const pct = Math.round(((current - previous) / previous) * 100);
  if (pct === 0) return;
  const monthName = fmtMonthName.format(new Date(prevMonth + '-01T12:00:00'));
  el.textContent = `${pct > 0 ? '▲' : '▼'} ${Math.abs(pct)}% vs ${monthName}`;
  el.classList.add(pct > 0 === upIsGood ? 'good' : 'bad');
}

/* Presupuestos: barra del presupuesto global + una por categoría con presupuesto. */
function budgetRow(icon, name, spent, budget) {
  const pct = Math.round((spent / budget) * 100);
  const level = pct > 100 ? 'critical' : pct >= 75 ? 'warning' : '';
  const li = document.createElement('li');
  li.className = 'budget-item';
  li.innerHTML = `
    <div class="row">
      <span class="tx-icon"></span>
      <span class="name"></span>
      <span class="nums"></span>
      <span class="pct ${level}">${pct}%</span>
    </div>
    <div class="budget-track"><div class="budget-fill ${level}"></div></div>`;
  li.querySelector('.budget-fill').style.width = `${Math.min(pct, 100)}%`;
  li.querySelector('.tx-icon').textContent = icon;
  li.querySelector('.name').textContent = name;
  li.querySelector('.nums').textContent = `${eur(spent)} de ${eur(budget)}`;
  return li;
}

function renderBudgets() {
  const card = $('#budget-card');
  const list = $('#budget-list');
  // Presupuestos efectivos del mes consultado (con su historial de cambios),
  // que da el servidor en el summary; el mes actual coincide con los actuales.
  const budgeted = state.summary.budgets || [];
  const globalMonth = state.summary.global_budget_month_cents || 0;
  const hasGlobal = globalMonth > 0;
  card.classList.toggle('hidden', budgeted.length === 0 && !hasGlobal);
  if (budgeted.length === 0 && !hasGlobal) return;
  const spentBy = new Map((state.summary.by_category || []).map((c) => [c.id, c.total_cents]));
  list.innerHTML = '';
  if (hasGlobal) {
    list.appendChild(budgetRow('🌍', 'Total del mes', state.summary.gastos_cents, globalMonth));
  }
  for (const c of budgeted) {
    list.appendChild(budgetRow(c.icon, c.name, spentBy.get(c.id) || 0, c.budget_cents));
  }
}

/* Donut: como máximo 8 porciones — top 7 categorías + "Otras". */
function donutData() {
  const cats = state.summary.by_category;
  const top = cats.slice(0, 7);
  const rest = cats.slice(7);
  const slices = top.map((c) => ({
    id: c.id,
    label: `${c.icon || ''} ${c.name || 'Sin categoría'}`.trim(),
    value: c.total_cents,
    color: catColor(c.color),
  }));
  if (rest.length) {
    slices.push({
      id: null,
      label: 'Otras',
      value: rest.reduce((a, c) => a + c.total_cents, 0),
      color: '#898781',
    });
  }
  return slices;
}

function renderCharts() {
  const ink2 = cssVar('--text-2');
  const muted = cssVar('--muted');
  const grid = cssVar('--grid');
  const surface = cssVar('--surface');
  const cIngreso = cssVar('--ingreso');
  const cGasto = cssVar('--gasto');
  const s = state.summary;

  Chart.defaults.font.family = 'system-ui, -apple-system, "Segoe UI", sans-serif';
  Chart.defaults.color = muted;

  const tooltipStyle = {
    backgroundColor: isDark() ? '#2c2c2a' : '#0b0b0b',
    titleColor: '#ffffff',
    bodyColor: '#e1e0d9',
    padding: 10,
    cornerRadius: 8,
    boxPadding: 4,
  };

  // --- Barras: últimos 12 meses ---
  if (chartMonthly) chartMonthly.destroy();
  const labels = s.monthly.map((m) => {
    const d = new Date(m.month + '-01T12:00:00');
    return fmtMonthShort.format(d).replace('.', '') + (d.getMonth() === 0 ? ` ${String(d.getFullYear()).slice(2)}` : '');
  });
  chartMonthly = new Chart($('#chart-monthly'), {
    type: 'bar',
    data: {
      labels,
      datasets: [
        {
          label: 'Ingresos',
          data: s.monthly.map((m) => m.ingresos_cents / 100),
          backgroundColor: cIngreso,
          borderRadius: 4,
          borderSkipped: 'bottom',
        },
        {
          label: 'Gastos',
          data: s.monthly.map((m) => m.gastos_cents / 100),
          backgroundColor: cGasto,
          borderRadius: 4,
          borderSkipped: 'bottom',
        },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      datasets: { bar: { categoryPercentage: 0.72, barPercentage: 0.85 } },
      plugins: {
        legend: {
          position: 'top',
          align: 'end',
          labels: { usePointStyle: true, pointStyle: 'circle', boxWidth: 7, boxHeight: 7, color: ink2 },
        },
        tooltip: {
          ...tooltipStyle,
          callbacks: { label: (ctx) => ` ${ctx.dataset.label}: ${fmtEUR.format(ctx.parsed.y)}` },
        },
      },
      scales: {
        x: { grid: { display: false }, border: { color: grid } },
        y: {
          beginAtZero: true,
          grid: { color: grid, drawTicks: false },
          border: { display: false },
          ticks: {
            maxTicksLimit: 5,
            callback: (v) => (v >= 1000 ? `${v / 1000}k €` : `${v} €`),
          },
        },
      },
    },
  });

  // --- Donut: gasto por categoría ---
  if (chartDonut) chartDonut.destroy();
  const slices = donutData();
  const hasData = slices.length > 0;
  $('#donut-empty').classList.toggle('hidden', hasData);
  $('#chart-categories').parentElement.classList.toggle('hidden', !hasData);
  if (hasData) {
    chartDonut = new Chart($('#chart-categories'), {
      type: 'doughnut',
      data: {
        labels: slices.map((x) => x.label),
        datasets: [
          {
            data: slices.map((x) => x.value / 100),
            backgroundColor: slices.map((x) => x.color),
            borderColor: surface,
            borderWidth: 2,
            hoverOffset: 6,
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        cutout: '68%',
        onClick: (_e, els) => {
          if (!els.length) return;
          const slice = slices[els[0].index];
          if (slice && slice.id != null) openCatDetail(slice.id);
        },
        onHover: (e, els) => { e.native.target.style.cursor = els.length && slices[els[0].index].id != null ? 'pointer' : 'default'; },
        plugins: {
          legend: { display: false },
          tooltip: {
            ...tooltipStyle,
            callbacks: { label: (ctx) => ` ${fmtEUR.format(ctx.parsed)}` },
          },
        },
      },
      plugins: [
        {
          id: 'centerText',
          afterDraw(chart) {
            const { ctx, chartArea } = chart;
            const cx = (chartArea.left + chartArea.right) / 2;
            const cy = (chartArea.top + chartArea.bottom) / 2;
            ctx.save();
            ctx.textAlign = 'center';
            ctx.font = '600 15px system-ui, sans-serif';
            ctx.fillStyle = cssVar('--text');
            ctx.fillText(eur(s.gastos_cents), cx, cy - 2);
            ctx.font = '11px system-ui, sans-serif';
            ctx.fillStyle = cssVar('--muted');
            ctx.fillText('gastado', cx, cy + 14);
            ctx.restore();
          },
        },
      ],
    });
  }

  renderNetworthChart();
}

/* Línea de patrimonio total (cuentas + inversiones) a lo largo del tiempo. */
function renderNetworthChart() {
  if (!state.networth) return;
  const series = state.networth;
  const grid = cssVar('--grid');
  const muted = cssVar('--muted');
  const blue = cssVar('--ingreso');
  const fmtDay = new Intl.DateTimeFormat('es-ES', { day: 'numeric', month: 'short' });
  if (chartNetworth) chartNetworth.destroy();
  chartNetworth = new Chart($('#chart-networth'), {
    type: 'line',
    data: {
      labels: series.map((p) => fmtDay.format(new Date(p.date + 'T12:00:00')).replace('.', '')),
      datasets: [
        {
          label: 'Patrimonio',
          data: series.map((p) => p.total_cents / 100),
          borderColor: blue,
          backgroundColor: blue,
          borderWidth: 2,
          pointRadius: 0,
          pointHoverRadius: 4,
          tension: 0.25,
        },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      interaction: { mode: 'index', intersect: false },
      plugins: {
        legend: { display: false },
        tooltip: {
          backgroundColor: isDark() ? '#2c2c2a' : '#0b0b0b',
          titleColor: '#ffffff',
          bodyColor: '#e1e0d9',
          padding: 10,
          cornerRadius: 8,
          callbacks: {
            label: (ctx) => {
              const p = series[ctx.dataIndex];
              const parts = [` Total: ${fmtEUR.format(ctx.parsed.y)}`];
              if (p.investments_cents > 0) {
                parts.push(` Cuentas: ${eur(p.accounts_cents)} · Inversiones: ${eur(p.investments_cents)}`);
              }
              return parts;
            },
          },
        },
      },
      scales: {
        x: { grid: { display: false }, ticks: { maxTicksLimit: 8, color: muted } },
        y: {
          grid: { color: grid, drawTicks: false },
          border: { display: false },
          ticks: { maxTicksLimit: 5, color: muted, callback: (v) => (Math.abs(v) >= 1000 ? `${(v / 1000).toFixed(1).replace('.0', '')}k €` : `${v} €`) },
        },
      },
    },
  });
}

async function loadNetworth() {
  try {
    const data = await api('/networth');
    state.networth = data.series;
    renderNetworthChart();
  } catch (_) { /* se reintenta en el siguiente refresh */ }
}

function renderCatLegend() {
  const el = $('#cat-legend');
  if (!state.summary) return;
  const total = state.summary.gastos_cents || 1;
  el.innerHTML = '';
  for (const slice of donutData()) {
    const li = document.createElement('li');
    const pct = Math.round((slice.value / total) * 100);
    li.innerHTML = `<span class="swatch"></span>
      <span class="name"></span><span class="val"></span><span class="pct">${pct}%</span>`;
    li.querySelector('.swatch').style.background = slice.color;
    li.querySelector('.name').textContent = slice.label;
    li.querySelector('.val').textContent = eur(slice.value);
    if (slice.id != null) {
      li.style.cursor = 'pointer';
      li.addEventListener('click', () => openCatDetail(slice.id));
    }
    el.appendChild(li);
  }
}

/* ===== Detalle de categoría (al tocar el donut o la leyenda) ===== */

const catDetailModal = $('#catdetail-modal');

async function openCatDetail(categoryId) {
  try {
    const [detail, txs] = await Promise.all([
      api(`/categories/${categoryId}/monthly?month=${state.month}`),
      api(`/transactions?month=${state.month}&category_id=${categoryId}`),
    ]);
    const monthCents = (detail.months.find((m) => m.month === state.month) || {}).total_cents || 0;
    $('#catdetail-title').textContent = `${detail.category.icon || ''} ${detail.category.name}`.trim();
    $('#catdetail-total').textContent = `${eur(monthCents)} en ${cap(fmtMonthLong.format(new Date(state.month + '-01T12:00:00')))}`;
    $('#catdetail-month-label').textContent = 'Movimientos del mes';

    const muted = cssVar('--muted');
    const grid = cssVar('--grid');
    const color = catColor(detail.category.color);
    const labels = detail.months.map((m) => {
      const d = new Date(m.month + '-01T12:00:00');
      return fmtMonthShort.format(d).replace('.', '');
    });
    if (chartCatDetail) chartCatDetail.destroy();
    chartCatDetail = new Chart($('#catdetail-chart'), {
      type: 'bar',
      data: { labels, datasets: [{ data: detail.months.map((m) => m.total_cents / 100), backgroundColor: color, borderRadius: 5, borderSkipped: 'bottom' }] },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: { legend: { display: false }, tooltip: { callbacks: { label: (ctx) => ` ${fmtEUR.format(ctx.parsed.y)}` } } },
        scales: {
          x: { grid: { display: false }, ticks: { color: muted } },
          y: { beginAtZero: true, grid: { color: grid, drawTicks: false }, border: { display: false }, ticks: { maxTicksLimit: 4, color: muted, callback: (v) => (v >= 1000 ? `${(v / 1000).toFixed(1).replace('.0', '')}k` : v) } },
        },
      },
    });

    renderTxList($('#catdetail-list'), txs.items, { grouped: false });
    $('#catdetail-empty').classList.toggle('hidden', txs.items.length > 0);
    catDetailModal.showModal();
  } catch (err) {
    alert(err.message);
  }
}

$('#catdetail-close').addEventListener('click', () => catDetailModal.close());

/* ===== Lista de movimientos (compartida) ===== */

function renderTxList(el, items, { grouped = true } = {}) {
  noteAccounts(items);
  el.innerHTML = '';
  let lastDate = null;
  for (const tx of items) {
    if (grouped && tx.date !== lastDate) {
      lastDate = tx.date;
      const h = document.createElement('li');
      h.className = 'tx-date-header';
      h.textContent = cap(fmtDateLong.format(new Date(tx.date + 'T12:00:00')));
      el.appendChild(h);
    }
    const li = document.createElement('li');
    li.className = 'tx-item';
    const isTransfer = Boolean(tx._transfer);
    const sign = isTransfer ? '' : tx.type === 'ingreso' ? '+' : '−';
    const amountClass = isTransfer ? 'transfer' : tx.type;
    li.innerHTML = `
      <span class="tx-icon"></span>
      <div class="tx-info">
        <div class="tx-cat"></div>
        <div class="tx-note"></div>
      </div>
      <span class="tx-amount ${amountClass}">${sign}${eur(tx.amount_cents)}</span>
      <span class="tx-actions">
        ${isTransfer ? '' : '<button class="icon-btn" data-action="repeat" title="Repetir hoy">🔁</button>'}
        <button class="icon-btn" data-action="edit" title="Editar">✏️</button>
        <button class="icon-btn" data-action="delete" title="Borrar">🗑️</button>
      </span>`;
    const note = li.querySelector('.tx-note');
    const dateLabel = grouped ? '' : cap(fmtDateLong.format(new Date(tx.date + 'T12:00:00')));
    if (isTransfer) {
      const icon = li.querySelector('.tx-icon');
      icon.textContent = '🔁';
      icon.style.background = `color-mix(in srgb, ${cssVar('--accent')} 18%, transparent)`;
      li.querySelector('.tx-cat').textContent = 'Transferencia';
      const parts = [`${tx.from_account} → ${tx.to_account}`, tx.note].filter(Boolean);
      note.textContent = parts.join(' · ');
      li.querySelector('[data-action="edit"]').addEventListener('click', () => openTxModal(tx));
      li.querySelector('[data-action="delete"]').addEventListener('click', async () => {
        if (!confirm(`¿Borrar la transferencia de ${eur(tx.amount_cents)}?`)) return;
        await api('/transfers/' + tx.id, { method: 'DELETE' });
        refreshAll();
      });
    } else {
      const icon = li.querySelector('.tx-icon');
      icon.textContent = tx.category_icon || '📦';
      // Burbuja tintada con el color de la categoría (CSSOM: la CSP prohíbe estilos inline).
      icon.style.background = `color-mix(in srgb, ${catColor(tx.category_color)} 20%, transparent)`;
      const catEl = li.querySelector('.tx-cat');
      catEl.textContent = tx.category_name || 'Sin categoría';
      if (tx.tags) {
        for (const tag of tx.tags.split(',').filter(Boolean)) {
          const chip = document.createElement('span');
          chip.className = 'tag-chip';
          chip.textContent = '#' + tag;
          catEl.appendChild(chip);
        }
      }
      const parts = [tx.note, tx.account].filter(Boolean);
      note.textContent = parts.length ? parts.join(' · ') : dateLabel;
      li.querySelector('[data-action="repeat"]').addEventListener('click', async () => {
        if (!confirm(`¿Apuntar de nuevo hoy "${tx.note || tx.category_name}" por ${eur(tx.amount_cents)}?`)) return;
        await api('/transactions', {
          method: 'POST',
          body: JSON.stringify({
            type: tx.type,
            amount: tx.amount_cents / 100,
            category_id: tx.category_id,
            note: tx.note,
            account: tx.account,
            date: todayISO(),
          }),
        });
        refreshAll();
      });
      li.querySelector('[data-action="edit"]').addEventListener('click', () => openTxModal(tx));
      li.querySelector('[data-action="delete"]').addEventListener('click', async () => {
        if (!confirm(`¿Borrar este movimiento de ${eur(tx.amount_cents)}?`)) return;
        await api('/transactions/' + tx.id, { method: 'DELETE' });
        refreshAll();
      });
    }
    if (!note.textContent) note.remove();
    addSwipe(li);
    el.appendChild(li);
  }
}

// Swipe en móvil: deslizar un movimiento a la izquierda revela sus acciones.
function addSwipe(li) {
  let startX = null;
  li.addEventListener('touchstart', (e) => { startX = e.touches[0].clientX; }, { passive: true });
  li.addEventListener('touchend', (e) => {
    if (startX == null) return;
    const dx = e.changedTouches[0].clientX - startX;
    if (dx < -40) {
      document.querySelectorAll('.tx-item.swiped').forEach((o) => o !== li && o.classList.remove('swiped'));
      li.classList.add('swiped');
    } else if (dx > 40) {
      li.classList.remove('swiped');
    }
    startX = null;
  }, { passive: true });
}

/* ===== Vista Movimientos ===== */

const fMonth = $('#f-month');
const fType = $('#f-type');
const fCategory = $('#f-category');
const fAccount = $('#f-account');
const fMin = $('#f-min');
const fMax = $('#f-max');
const fSearch = $('#f-search');
fMonth.value = state.month;

let searchTimer = null;
for (const el of [fMonth, fType, fCategory, fAccount]) el.addEventListener('change', refreshMovimientos);
for (const el of [fSearch, fMin, fMax]) {
  el.addEventListener('input', () => {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(refreshMovimientos, 250);
  });
}

function renderFilterAccountOptions() {
  const current = fAccount.value;
  fAccount.innerHTML = '<option value="">Todas las cuentas</option>';
  for (const a of state.accountBalances) {
    const opt = document.createElement('option');
    opt.value = a.name;
    opt.textContent = a.name;
    fAccount.appendChild(opt);
  }
  fAccount.value = current;
}

function renderFilterCategoryOptions() {
  const current = fCategory.value;
  fCategory.innerHTML = '<option value="">Todas las categorías</option>';
  const type = fType.value;
  const cats = state.categories.filter((c) => !c.archived && (!type || c.type === type));
  for (const c of cats) {
    const opt = document.createElement('option');
    opt.value = c.id;
    opt.textContent = `${c.icon} ${c.name}`;
    fCategory.appendChild(opt);
  }
  fCategory.value = current;
}

async function refreshMovimientos() {
  renderFilterCategoryOptions();
  renderFilterAccountOptions();
  const params = new URLSearchParams();
  if (fMonth.value) params.set('month', fMonth.value);
  if (fType.value) params.set('type', fType.value);
  if (fCategory.value) params.set('category_id', fCategory.value);
  if (fAccount.value) params.set('account', fAccount.value);
  if (fSearch.value.trim()) params.set('q', fSearch.value.trim());
  if (fMin.value.trim()) params.set('amount_min', fMin.value.trim());
  if (fMax.value.trim()) params.set('amount_max', fMax.value.trim());
  const data = await api('/transactions?' + params.toString());

  // Las transferencias se mezclan en la lista (salvo con filtros que no les aplican).
  let items = data.items;
  const hasAmountFilter = Boolean(fMin.value.trim() || fMax.value.trim());
  if (!fType.value && !fCategory.value && !fSearch.value.trim() && !hasAmountFilter) {
    const tParams = new URLSearchParams();
    if (fMonth.value) tParams.set('month', fMonth.value);
    if (fAccount.value) tParams.set('account', fAccount.value);
    const transfers = await api('/transfers?' + tParams.toString());
    items = [...items, ...transfers.map((t) => ({ ...t, type: 'transfer', _transfer: true }))]
      .sort((a, b) => (a.date === b.date ? b.id - a.id : a.date < b.date ? 1 : -1));
  }

  renderTxList($('#tx-list'), items);
  $('#tx-empty').classList.toggle('hidden', items.length > 0);
  $('#filter-totals').innerHTML =
    `<span>${data.total_count} movimientos</span>` +
    `<span>Ingresos: <b>${eur(data.ingresos_cents)}</b></span>` +
    `<span>Gastos: <b>${eur(data.gastos_cents)}</b></span>` +
    `<span>Balance: <b>${eur(data.ingresos_cents - data.gastos_cents)}</b></span>`;
}

async function refreshAll() {
  await Promise.all([refreshResumen(), refreshMovimientos(), loadAccounts()]);
  loadNetworth();
  state.yearData = null; // se recarga al entrar en la pestaña Anual
}

/* ===== Modal de movimiento ===== */

const txModal = $('#tx-modal');

function setTxType(type) {
  state.txType = type;
  document.querySelectorAll('#tx-type-toggle .type-btn').forEach((b) =>
    b.classList.toggle('active', b.dataset.type === type)
  );
  const isTransfer = type === 'transfer';
  $('#tx-category-label').classList.toggle('hidden', isTransfer);
  $('#tx-account-label').classList.toggle('hidden', isTransfer);
  $('#tx-tags-label').classList.toggle('hidden', isTransfer);
  $('#tx-from-label').classList.toggle('hidden', !isTransfer);
  $('#tx-to-label').classList.toggle('hidden', !isTransfer);
  $('#tx-save-template').classList.toggle('hidden', isTransfer || Boolean(state.editingTx));
  if (isTransfer) { $('#tx-templates').classList.add('hidden'); return; }
  const sel = $('#tx-category');
  sel.innerHTML = '';
  for (const c of activeCategories(type)) {
    const opt = document.createElement('option');
    opt.value = c.id;
    opt.textContent = `${c.icon} ${c.name}`;
    sel.appendChild(opt);
  }
}

$('#tx-type-toggle').addEventListener('click', (e) => {
  const btn = e.target.closest('.type-btn');
  if (btn) setTxType(btn.dataset.type);
});

function openTxModal(tx = null) {
  state.editingTx = tx;
  const isTransfer = Boolean(tx && tx._transfer);
  $('#tx-modal-title').textContent = tx ? (isTransfer ? 'Editar transferencia' : 'Editar movimiento') : 'Nuevo movimiento';
  $('#tx-error').classList.add('hidden');
  setTxType(tx ? (isTransfer ? 'transfer' : tx.type) : 'gasto');
  $('#tx-amount').value = tx ? String(tx.amount_cents / 100).replace('.', ',') : '';
  $('#tx-date').value = tx ? tx.date : todayISO();
  $('#tx-note').value = tx ? tx.note : '';
  $('#tx-tags').value = tx && !isTransfer ? (tx.tags || '').split(',').filter(Boolean).map((t) => '#' + t).join(' ') : '';
  // Al editar manda la cuenta del movimiento; al crear, la preferida de Ajustes.
  const accountSel = tx ? (isTransfer ? '' : tx.account || '') : state.defaultAccount;
  fillAccountSelect($('#tx-account'), $('#tx-account-new'), accountSel, { includeBlank: true });
  fillAccountSelect($('#tx-from'), $('#tx-from-new'), isTransfer ? tx.from_account : '');
  fillAccountSelect($('#tx-to'), $('#tx-to-new'), isTransfer ? tx.to_account : '');
  if (tx && !isTransfer && tx.category_id) $('#tx-category').value = tx.category_id;
  // Plantillas y "guardar plantilla" solo al crear un movimiento nuevo normal.
  renderTemplateChips(!tx && !isTransfer);
  $('#tx-save-template').classList.toggle('hidden', Boolean(tx));
  txModal.showModal();
  $('#tx-amount').focus();
}

/* Plantillas rápidas: chips que rellenan el modal de un toque. */
function renderTemplateChips(show) {
  const wrap = $('#tx-templates');
  wrap.innerHTML = '';
  const items = show ? state.templates : [];
  wrap.classList.toggle('hidden', items.length === 0);
  for (const tp of items) {
    const chip = document.createElement('span');
    chip.className = 'template-chip';
    const apply = document.createElement('button');
    apply.type = 'button';
    apply.className = 'apply';
    apply.textContent = `${tp.category_icon || (tp.type === 'ingreso' ? '➕' : '💸')} ${tp.name} · ${eur(tp.amount_cents)}`;
    apply.addEventListener('click', () => applyTemplate(tp));
    const del = document.createElement('button');
    del.type = 'button';
    del.className = 'del';
    del.textContent = '✕';
    del.title = 'Borrar plantilla';
    del.addEventListener('click', async (e) => {
      e.stopPropagation();
      await api('/templates/' + tp.id, { method: 'DELETE' });
      await loadTemplates();
      renderTemplateChips(true);
    });
    chip.appendChild(apply);
    chip.appendChild(del);
    wrap.appendChild(chip);
  }
}

function applyTemplate(tp) {
  setTxType(tp.type);
  $('#tx-amount').value = String(tp.amount_cents / 100).replace('.', ',');
  $('#tx-note').value = tp.note || '';
  fillAccountSelect($('#tx-account'), $('#tx-account-new'), tp.account || state.defaultAccount, { includeBlank: true });
  if (tp.category_id) $('#tx-category').value = tp.category_id;
  $('#tx-amount').focus();
}

async function loadTemplates() {
  try {
    state.templates = await api('/templates');
  } catch (_) { state.templates = []; }
}

$('#tx-save-template').addEventListener('click', async () => {
  if (state.txType === 'transfer') return;
  const amount = $('#tx-amount').value.trim();
  if (!amount) { $('#tx-amount').focus(); return; }
  const catOpt = $('#tx-category').selectedOptions[0];
  const body = {
    name: ($('#tx-note').value.trim() || (catOpt ? catOpt.textContent.replace(/^\S+\s/, '') : 'Plantilla')).slice(0, 40),
    type: state.txType,
    amount,
    category_id: Number($('#tx-category').value) || null,
    note: $('#tx-note').value.trim(),
    account: accountValue($('#tx-account'), $('#tx-account-new')),
  };
  const btn = $('#tx-save-template');
  try {
    await api('/templates', { method: 'POST', body: JSON.stringify(body) });
    await loadTemplates();
    btn.textContent = '★ Guardada';
    setTimeout(() => (btn.textContent = '☆ Plantilla'), 1200);
  } catch (err) {
    const el = $('#tx-error');
    el.textContent = err.message;
    el.classList.remove('hidden');
  }
});

$('#fab-add').addEventListener('click', () => openTxModal());
$('#tx-cancel').addEventListener('click', () => txModal.close());

// Cablea todos los pares select+input "Otra cuenta…" de los modales.
for (const [sel, inp] of [
  ['#tx-account', '#tx-account-new'], ['#tx-from', '#tx-from-new'], ['#tx-to', '#tx-to-new'],
  ['#rec-from', '#rec-from-new'], ['#rec-to', '#rec-to-new'], ['#rec-source', '#rec-source-new'],
]) wireAccountSelect(sel, inp);

// Aviso de posible duplicado al crear (mismo día, importe, tipo y nota).
async function looksDuplicate(body) {
  try {
    const cents = Math.round(Number(String(body.amount).replace(/[€\s]/g, '').replace(/\./g, '').replace(',', '.')) * 100);
    if (!Number.isInteger(cents)) return false;
    const params = new URLSearchParams({ month: body.date.slice(0, 7), type: body.type, amount_min: body.amount, amount_max: body.amount });
    const data = await api('/transactions?' + params.toString());
    const note = (body.note || '').trim().toLowerCase();
    const dup = data.items.find((t) => t.date === body.date && (t.note || '').trim().toLowerCase() === note);
    if (!dup) return false;
    return !confirm(`Ya tienes un movimiento igual el ${body.date} (${eur(dup.amount_cents)}${dup.note ? ' · ' + dup.note : ''}). ¿Guardar de todos modos?`);
  } catch (_) {
    return false; // ante la duda, no bloquear
  }
}

$('#tx-form').addEventListener('submit', async (e) => {
  e.preventDefault();
  const isTransfer = state.txType === 'transfer';
  const editing = state.editingTx;
  const wasTransfer = Boolean(editing && editing._transfer);
  try {
    if (isTransfer) {
      const body = {
        amount: $('#tx-amount').value,
        from_account: accountValue($('#tx-from'), $('#tx-from-new')),
        to_account: accountValue($('#tx-to'), $('#tx-to-new')),
        date: $('#tx-date').value,
        note: $('#tx-note').value,
      };
      if (editing && wasTransfer) {
        await api('/transfers/' + editing.id, { method: 'PUT', body: JSON.stringify(body) });
      } else {
        // Nuevo, o un movimiento normal convertido en transferencia.
        if (editing && !wasTransfer) await api('/transactions/' + editing.id, { method: 'DELETE' });
        await api('/transfers', { method: 'POST', body: JSON.stringify(body) });
      }
    } else {
      const body = {
        type: state.txType,
        amount: $('#tx-amount').value,
        category_id: Number($('#tx-category').value) || null,
        date: $('#tx-date').value,
        note: $('#tx-note').value,
        account: accountValue($('#tx-account'), $('#tx-account-new')),
        tags: $('#tx-tags').value,
      };
      // Sin servidor no se puede comprobar el duplicado (y esperar al fallo
      // congelaría el modal): se guarda y ya avisará el batch al sincronizar.
      if (!editing && serverUp && (await looksDuplicate(body))) return;
      if (editing && wasTransfer) {
        // Una transferencia convertida en movimiento normal.
        await api('/transfers/' + editing.id, { method: 'DELETE' });
        await api('/transactions', { method: 'POST', body: JSON.stringify(body) });
      } else if (editing) {
        await api('/transactions/' + editing.id, { method: 'PUT', body: JSON.stringify(body) });
      } else {
        await createTxMaybeOffline(body);
      }
    }
    txModal.close();
    // Sin red, refrescar fallará: la píldora de offline ya informa del pendiente.
    refreshAll().catch(() => {});
  } catch (err) {
    const el = $('#tx-error');
    el.textContent = err.message;
    el.classList.remove('hidden');
  }
});

/* ===== Vista Categorías ===== */

function renderCategorias() {
  for (const type of ['gasto', 'ingreso']) {
    const ul = $('#cat-list-' + type);
    ul.innerHTML = '';
    for (const c of state.categories.filter((x) => x.type === type)) {
      const li = document.createElement('li');
      li.className = 'cat-item' + (c.archived ? ' archived' : '');
      li.innerHTML = `
        <span class="tx-icon"></span>
        <span class="swatch"></span>
        <span class="name"></span>
        <span class="count">${c.usage_count} mov.</span>
        <span class="tx-actions">
          <button class="icon-btn" data-action="edit" title="Editar">✏️</button>
          <button class="icon-btn" data-action="delete" title="${c.archived ? 'Archivada' : 'Borrar'}">🗑️</button>
        </span>`;
      li.querySelector('.tx-icon').textContent = c.icon;
      li.querySelector('.swatch').style.background = catColor(c.color);
      li.querySelector('.name').textContent = c.name + (c.archived ? ' (archivada)' : '');
      li.querySelector('[data-action="edit"]').addEventListener('click', () => openCatModal(c));
      li.querySelector('[data-action="delete"]').addEventListener('click', async () => {
        const msg = c.usage_count > 0
          ? `"${c.name}" tiene movimientos: se archivará (deja de aparecer al añadir). ¿Continuar?`
          : `¿Borrar la categoría "${c.name}"?`;
        if (!confirm(msg)) return;
        await api('/categories/' + c.id, { method: 'DELETE' });
        await loadCategories();
        renderCategorias();
      });
      ul.appendChild(li);
    }
  }
}

/* ===== Modal de categoría ===== */

const catModal = $('#cat-modal');

function setCatType(type) {
  state.catType = type;
  document.querySelectorAll('#cat-type-toggle .type-btn').forEach((b) =>
    b.classList.toggle('active', b.dataset.type === type)
  );
  $('#cat-budget-label').classList.toggle('hidden', type !== 'gasto');
  $('#cat-fixed-label').classList.toggle('hidden', type !== 'gasto');
}
$('#cat-type-toggle').addEventListener('click', (e) => {
  const btn = e.target.closest('.type-btn');
  if (btn && !state.editingCat) setCatType(btn.dataset.type);
});

function renderSwatches() {
  const wrap = $('#cat-swatches');
  wrap.innerHTML = '';
  for (const hex of PALETTE) {
    const b = document.createElement('button');
    b.type = 'button';
    b.style.background = catColor(hex);
    b.classList.toggle('active', hex === state.catColor);
    b.addEventListener('click', () => {
      state.catColor = hex;
      renderSwatches();
    });
    wrap.appendChild(b);
  }
}

function openCatModal(cat = null) {
  state.editingCat = cat;
  $('#cat-modal-title').textContent = cat ? 'Editar categoría' : 'Nueva categoría';
  $('#cat-error').classList.add('hidden');
  setCatType(cat ? cat.type : 'gasto');
  $('#cat-name').value = cat ? cat.name : '';
  $('#cat-icon').value = cat ? cat.icon : '';
  $('#cat-budget').value =
    cat && cat.budget_cents > 0 ? String(cat.budget_cents / 100).replace('.', ',') : '';
  $('#cat-fixed').checked = Boolean(cat && cat.fixed);
  state.catColor = cat ? cat.color : PALETTE[0];
  renderSwatches();
  catModal.showModal();
  $('#cat-name').focus();
}

$('#btn-new-category').addEventListener('click', () => openCatModal());
$('#cat-cancel').addEventListener('click', () => catModal.close());

$('#cat-form').addEventListener('submit', async (e) => {
  e.preventDefault();
  const body = {
    name: $('#cat-name').value,
    type: state.catType,
    icon: $('#cat-icon').value || '📦',
    color: state.catColor,
    budget: $('#cat-budget').value.trim() || 0,
    fixed: $('#cat-fixed').checked,
  };
  try {
    if (state.editingCat) {
      await api('/categories/' + state.editingCat.id, { method: 'PUT', body: JSON.stringify(body) });
    } else {
      await api('/categories', { method: 'POST', body: JSON.stringify(body) });
    }
    catModal.close();
    await loadCategories();
    renderCategorias();
    refreshAll();
  } catch (err) {
    const el = $('#cat-error');
    el.textContent = err.message;
    el.classList.remove('hidden');
  }
});

/* ===== Recurrentes ===== */

const recModal = $('#rec-modal');

// Modos del modal: 'gasto' | 'ingreso' (kind movimiento), 'traspaso', 'aportacion'.
function setRecKind(kindBtn) {
  state.recKind = kindBtn;
  document.querySelectorAll('#rec-type-toggle .type-btn').forEach((b) =>
    b.classList.toggle('active', b.dataset.kind === kindBtn)
  );
  const isMov = kindBtn === 'gasto' || kindBtn === 'ingreso';
  $('#rec-category-label').classList.toggle('hidden', !isMov);
  $('#rec-from-label').classList.toggle('hidden', kindBtn !== 'traspaso');
  $('#rec-to-label').classList.toggle('hidden', kindBtn !== 'traspaso');
  $('#rec-investment-label').classList.toggle('hidden', kindBtn !== 'aportacion');
  $('#rec-source-label').classList.toggle('hidden', kindBtn !== 'aportacion');

  if (isMov) {
    const sel = $('#rec-category');
    sel.innerHTML = '';
    for (const c of activeCategories(kindBtn)) {
      const opt = document.createElement('option');
      opt.value = c.id;
      opt.textContent = `${c.icon} ${c.name}`;
      sel.appendChild(opt);
    }
  }
  if (kindBtn === 'traspaso') {
    fillAccountSelect($('#rec-from'), $('#rec-from-new'));
    fillAccountSelect($('#rec-to'), $('#rec-to-new'));
  }
  if (kindBtn === 'aportacion') {
    fillRecInvestmentOptions();
    fillAccountSelect($('#rec-source'), $('#rec-source-new'), '', { includeBlank: true });
  }
}

async function fillRecInvestmentOptions() {
  if (!state.investments) await loadInvestments();
  const sel = $('#rec-investment');
  sel.innerHTML = '';
  const items = state.investments ? state.investments.items : [];
  for (const inv of items) {
    const opt = document.createElement('option');
    opt.value = inv.id;
    opt.textContent = inv.name;
    sel.appendChild(opt);
  }
  if (items.length === 0) {
    const opt = document.createElement('option');
    opt.value = '';
    opt.textContent = '(no tienes inversiones aún — créala en la pestaña Inversiones)';
    sel.appendChild(opt);
  }
}

$('#rec-type-toggle').addEventListener('click', (e) => {
  const btn = e.target.closest('.type-btn');
  if (btn) setRecKind(btn.dataset.kind);
});

async function loadRecurring() {
  state.recurring = await api('/recurring');
  renderRecurring();
}

function renderRecurring() {
  const ul = $('#rec-list');
  ul.innerHTML = '';
  const items = state.recurring || [];
  $('#rec-empty').classList.toggle('hidden', items.length > 0);
  for (const r of items) {
    const li = document.createElement('li');
    li.className = 'rec-item' + (r.active ? '' : ' inactive');
    let sign = r.type === 'ingreso' ? '+' : '−';
    let amountClass = r.type;
    if (r.kind === 'traspaso') { sign = ''; amountClass = 'transfer'; }
    li.innerHTML = `
      <span class="tx-icon"></span>
      <div class="info">
        <div class="what"></div>
        <div class="when">Cada mes, el día ${r.day_of_month}${r.active ? '' : ' · pausado'}</div>
      </div>
      <span class="tx-amount ${amountClass}">${sign}${eur(r.amount_cents)}</span>
      <span class="tx-actions">
        <button class="icon-btn" data-action="toggle" title="${r.active ? 'Pausar' : 'Reanudar'}">${r.active ? '⏸️' : '▶️'}</button>
        <button class="icon-btn" data-action="edit" title="Editar">✏️</button>
        <button class="icon-btn" data-action="delete" title="Borrar">🗑️</button>
      </span>`;
    const what = li.querySelector('.what');
    if (r.kind === 'traspaso') {
      li.querySelector('.tx-icon').textContent = '🔁';
      what.textContent = [`${r.from_account} → ${r.to_account}`, r.note].filter(Boolean).join(' · ');
    } else if (r.kind === 'aportacion') {
      li.querySelector('.tx-icon').textContent = '📈';
      const parts = [`Aportación a ${r.investment_name || '(inversión borrada)'}`];
      if (r.from_account) parts.push(`desde ${r.from_account}`);
      if (r.note) parts.push(r.note);
      what.textContent = parts.join(' · ');
    } else {
      li.querySelector('.tx-icon').textContent = r.category_icon || '🔁';
      what.textContent = r.note || r.category_name || 'Sin nota';
    }
    li.querySelector('[data-action="toggle"]').addEventListener('click', async () => {
      await api('/recurring/' + r.id, { method: 'PUT', body: JSON.stringify({ active: !r.active }) });
      await loadRecurring();
      refreshAll();
    });
    li.querySelector('[data-action="edit"]').addEventListener('click', () => openRecModal(r));
    li.querySelector('[data-action="delete"]').addEventListener('click', async () => {
      if (!confirm(`¿Borrar el recurrente "${r.note || r.category_name}"? Los movimientos ya creados se conservan.`)) return;
      await api('/recurring/' + r.id, { method: 'DELETE' });
      loadRecurring();
    });
    ul.appendChild(li);
  }
}

/* Suscripciones detectadas: pagos repetidos que aún no son recurrentes. */
async function loadSuggestions() {
  let items = [];
  try {
    items = await api('/suggestions/recurring');
  } catch (_) { /* ignora */ }
  const card = $('#suggestions-card');
  const ul = $('#suggestions-list');
  card.classList.toggle('hidden', items.length === 0);
  ul.innerHTML = '';
  for (const s of items) {
    const li = document.createElement('li');
    li.className = 'rec-item';
    li.innerHTML = `
      <span class="tx-icon"></span>
      <div class="info">
        <div class="what"></div>
        <div class="when"></div>
      </div>
      <span class="tx-amount gasto"></span>
      <span class="tx-actions">
        <button class="icon-btn" data-action="convert" title="Convertir en recurrente">➕</button>
      </span>`;
    li.querySelector('.tx-icon').textContent = s.category_icon || '🔁';
    li.querySelector('.what').textContent = s.note || s.category_name || 'Pago repetido';
    li.querySelector('.when').textContent = `Visto ${s.months_seen} meses · sobre el día ${s.typical_day}`;
    li.querySelector('.tx-amount').textContent = '−' + eur(s.amount_cents);
    li.querySelector('[data-action="convert"]').addEventListener('click', () => {
      openRecModal();
      setRecKind('gasto');
      $('#rec-amount').value = String(s.amount_cents / 100).replace('.', ',');
      $('#rec-note').value = s.note || '';
      $('#rec-day').value = s.typical_day || 1;
      if (s.category_id) $('#rec-category').value = s.category_id;
    });
    ul.appendChild(li);
  }
}

function openRecModal(rec = null) {
  state.editingRec = rec;
  $('#rec-modal-title').textContent = rec ? 'Editar recurrente' : 'Nuevo recurrente';
  $('#rec-error').classList.add('hidden');
  const kindBtn = rec
    ? rec.kind === 'movimiento' ? rec.type : rec.kind
    : 'gasto';
  setRecKind(kindBtn);
  $('#rec-amount').value = rec ? String(rec.amount_cents / 100).replace('.', ',') : '';
  $('#rec-day').value = rec ? rec.day_of_month : 1;
  $('#rec-note').value = rec ? rec.note : '';
  if (rec && rec.kind === 'traspaso') {
    fillAccountSelect($('#rec-from'), $('#rec-from-new'), rec.from_account);
    fillAccountSelect($('#rec-to'), $('#rec-to-new'), rec.to_account);
  }
  if (rec && rec.kind === 'aportacion') {
    fillAccountSelect($('#rec-source'), $('#rec-source-new'), rec.from_account, { includeBlank: true });
  }
  if (rec && rec.category_id) $('#rec-category').value = rec.category_id;
  if (rec && rec.kind === 'aportacion' && rec.investment_id) {
    fillRecInvestmentOptions().then(() => { $('#rec-investment').value = rec.investment_id; });
  }
  recModal.showModal();
  $('#rec-amount').focus();
}

$('#btn-new-recurring').addEventListener('click', () => openRecModal());
$('#rec-cancel').addEventListener('click', () => recModal.close());

$('#rec-form').addEventListener('submit', async (e) => {
  e.preventDefault();
  const k = state.recKind;
  const body = {
    kind: k === 'traspaso' ? 'traspaso' : k === 'aportacion' ? 'aportacion' : 'movimiento',
    amount: $('#rec-amount').value,
    day_of_month: Number($('#rec-day').value),
    note: $('#rec-note').value,
  };
  if (body.kind === 'movimiento') {
    body.type = k;
    body.category_id = Number($('#rec-category').value) || null;
  } else if (body.kind === 'traspaso') {
    body.from_account = accountValue($('#rec-from'), $('#rec-from-new'));
    body.to_account = accountValue($('#rec-to'), $('#rec-to-new'));
  } else {
    body.investment_id = Number($('#rec-investment').value) || null;
    body.from_account = accountValue($('#rec-source'), $('#rec-source-new'));
  }
  try {
    if (state.editingRec) {
      await api('/recurring/' + state.editingRec.id, { method: 'PUT', body: JSON.stringify(body) });
    } else {
      await api('/recurring', { method: 'POST', body: JSON.stringify(body) });
    }
    recModal.close();
    await loadRecurring();
    loadSuggestions();
    refreshAll();
  } catch (err) {
    const el = $('#rec-error');
    el.textContent = err.message;
    el.classList.remove('hidden');
  }
});

/* ===== Cuentas ===== */

const accountModal = $('#account-modal');

function accountIcon(name) {
  const n = name.toLowerCase();
  if (n.includes('efectivo') || n.includes('cash')) return '💵';
  if (n.includes('revolut')) return '💳';
  return '🏦';
}

async function loadAccounts() {
  try {
    state.accountBalances = await api('/accounts');
    saveLocal(LOCAL_KEYS.accounts, state.accountBalances);
  } catch (err) {
    if (!isNetworkError(err)) throw err;
    state.accountBalances = readLocal(LOCAL_KEYS.accounts, []);
  }
  renderAccounts();
}

function renderAccounts() {
  const ul = $('#account-list');
  ul.innerHTML = '';
  const accountsTotal = state.accountBalances.reduce((a, x) => a + x.current_cents, 0);
  for (const acc of state.accountBalances) {
    const li = document.createElement('li');
    li.className = 'account-item';
    li.innerHTML = `
      <span class="tx-icon"></span>
      <div class="info">
        <div class="name"></div>
        <div class="balance"></div>
        <div class="apy-note hidden"></div>
      </div>
      <button class="icon-btn" title="Editar saldo">✏️</button>`;
    li.querySelector('.tx-icon').textContent = accountIcon(acc.name);
    li.querySelector('.name').textContent = acc.name;
    li.querySelector('.balance').textContent = eur(acc.current_cents);
    if (acc.apy > 0) {
      const note = li.querySelector('.apy-note');
      note.classList.remove('hidden');
      note.textContent = `${String(acc.apy).replace('.', ',')}% TAE · +${eur(acc.interest_cents)} interés`;
    }
    li.querySelector('.icon-btn').addEventListener('click', () => openAccountModal(acc));
    ul.appendChild(li);
  }

  const invTotal = state.investments && state.investments.totals ? state.investments.totals.value_cents : 0;
  if (invTotal > 0) {
    const li = document.createElement('li');
    li.className = 'account-item';
    li.innerHTML = `
      <span class="tx-icon">📈</span>
      <div class="info"><div class="name">Inversiones</div><div class="balance"></div></div>`;
    li.querySelector('.balance').textContent = eur(invTotal);
    ul.appendChild(li);
  }
  if (state.accountBalances.length > 1 || invTotal > 0) {
    const li = document.createElement('li');
    li.className = 'account-item total';
    li.innerHTML = `
      <span class="tx-icon">Σ</span>
      <div class="info"><div class="name">Total patrimonio</div><div class="balance"></div></div>`;
    li.querySelector('.balance').textContent = eur(accountsTotal + invTotal);
    ul.appendChild(li);
  }
}

function openAccountModal(acc = null) {
  state.editingAccount = acc ? acc.name : null;
  state.editingAccountObj = acc;
  $('#account-modal-title').textContent = acc ? 'Editar ' + acc.name : 'Nueva cuenta';
  $('#account-error').classList.add('hidden');
  const nameInput = $('#account-name');
  nameInput.value = acc ? acc.name : '';
  // Renombrar movería el histórico de movimientos: el nombre solo se elige al crear.
  nameInput.disabled = Boolean(acc);
  $('#account-opening').value = acc ? String(acc.opening_cents / 100).replace('.', ',') : '';
  $('#account-opening-date').value = acc ? acc.opening_date : todayISO();
  $('#account-apy').value = acc && acc.apy > 0 ? String(acc.apy).replace('.', ',') : '';
  $('#account-delete').classList.toggle('hidden', !acc);
  // Cuadrar el saldo solo tiene sentido en una cuenta existente.
  $('#account-reconcile-block').classList.toggle('hidden', !acc);
  $('#account-real').value = '';
  if (acc) $('#account-reconcile-hint').textContent = `Saldo calculado ahora: ${eur(acc.current_cents)}. Se creará un ajuste por la diferencia.`;
  accountModal.showModal();
  (acc ? $('#account-opening') : nameInput).focus();
}

$('#account-reconcile').addEventListener('click', async () => {
  const acc = state.editingAccountObj;
  if (!acc) return;
  const realCents = parseInputCents($('#account-real').value);
  if (realCents == null) { $('#account-real').focus(); return; }
  const diff = realCents - acc.current_cents;
  if (diff === 0) {
    $('#account-reconcile-hint').textContent = 'El saldo ya cuadra: no hace falta ajuste.';
    return;
  }
  const type = diff > 0 ? 'ingreso' : 'gasto';
  if (!confirm(`Se creará un ${type} de ${eur(Math.abs(diff))} en "${acc.name}" para cuadrar el saldo a ${eur(realCents)}. ¿Continuar?`)) return;
  try {
    await api('/transactions', {
      method: 'POST',
      body: JSON.stringify({ type, amount: Math.abs(diff) / 100, note: 'Ajuste de saldo', account: acc.name, date: todayISO() }),
    });
    accountModal.close();
    refreshAll();
  } catch (err) {
    const el = $('#account-error');
    el.textContent = err.message;
    el.classList.remove('hidden');
  }
});

// Convierte lo tecleado en un campo de importe español a céntimos (o null).
function parseInputCents(value) {
  let s = String(value || '').replace(/[€\s]/g, '').trim();
  if (!s) return null;
  if (s.includes(',')) s = s.replace(/\./g, '').replace(',', '.');
  const f = Number(s);
  return isFinite(f) ? Math.round(f * 100) : null;
}

$('#btn-new-account').addEventListener('click', () => openAccountModal());
$('#account-cancel').addEventListener('click', () => accountModal.close());

$('#account-form').addEventListener('submit', async (e) => {
  e.preventDefault();
  const name = state.editingAccount || $('#account-name').value.trim();
  if (!name) return;
  try {
    await api('/accounts/' + encodeURIComponent(name), {
      method: 'PUT',
      body: JSON.stringify({
        opening: $('#account-opening').value,
        opening_date: $('#account-opening-date').value,
        apy: $('#account-apy').value.trim(),
      }),
    });
    accountModal.close();
    loadAccounts();
    loadNetworth();
  } catch (err) {
    const el = $('#account-error');
    el.textContent = err.message;
    el.classList.remove('hidden');
  }
});

$('#account-delete').addEventListener('click', async () => {
  const name = state.editingAccount;
  if (!name) return;
  const msg = `¿Eliminar la cuenta "${name}"?\n\nSe borra su saldo guardado y su TAE. Si tiene movimientos, seguirá apareciendo (con saldo contado desde 0); si no los tiene, desaparecerá de la lista.`;
  if (!confirm(msg)) return;
  try {
    await api('/accounts/' + encodeURIComponent(name), { method: 'DELETE' });
    accountModal.close();
    loadAccounts();
    loadNetworth();
  } catch (err) {
    const el = $('#account-error');
    el.textContent = err.message;
    el.classList.remove('hidden');
  }
});

/* ===== Cartera de inversiones ===== */

const invModal = $('#inv-modal');

function renderInvestments() {
  const data = state.investments;
  if (!data) return;
  const { items, totals } = data;
  $('#inv-empty').classList.toggle('hidden', items.length > 0);
  $('#inv-total').textContent = eur(totals.value_cents);

  const gainEl = $('#inv-gain');
  gainEl.textContent = (totals.gain_cents > 0 ? '+' : '') + eur(totals.gain_cents);
  gainEl.classList.toggle('positive', totals.gain_cents > 0);
  gainEl.classList.toggle('negative', totals.gain_cents < 0);
  $('#inv-gain-pct').textContent =
    totals.cost_cents > 0 ? `${((totals.gain_cents / totals.cost_cents) * 100).toFixed(1).replace('.', ',')} % sobre lo invertido` : '';

  const stale = items.filter((i) => i.price_error).length;
  const newest = items.map((i) => i.price_fetched_at).filter(Boolean).sort().pop();
  $('#inv-updated').textContent = stale
    ? `⚠ ${stale} sin precio fresco (¿sin internet?)`
    : newest
      ? `precios de ${newest.slice(0, 16).replace('T', ' ')}`
      : '';

  const ul = $('#inv-list');
  for (const c of sparkCharts.values()) c.destroy();
  sparkCharts.clear();
  ul.innerHTML = '';
  for (const inv of items) {
    const li = document.createElement('li');
    li.className = 'inv-item';
    const gainCls = inv.gain_cents > 0 ? 'positive' : inv.gain_cents < 0 ? 'negative' : '';
    li.innerHTML = `
      <div class="info">
        <div class="name"></div>
        <div class="detail"></div>
      </div>
      <span class="inv-spark"><canvas></canvas></span>
      <div class="nums">
        <div class="value"></div>
        <div class="gain ${gainCls}"></div>
      </div>
      <span class="tx-actions">
        <button class="icon-btn" data-action="ops" title="Compras y ventas">📝</button>
        <button class="icon-btn" data-action="edit" title="Editar">✏️</button>
        <button class="icon-btn" data-action="delete" title="Borrar">🗑️</button>
      </span>`;
    li.querySelector('.name').textContent = inv.name;
    const unitPrice = inv.price_eur != null ? `${fmtEUR.format(inv.price_eur)}/ud` : 'sin precio';
    const src = inv.provider === 'manual' ? 'manual' : inv.symbol;
    li.querySelector('.detail').textContent = `${String(inv.units).replace('.', ',')} × ${unitPrice} · ${src}${inv.price_error ? ' · ⚠ ' + inv.price_error : ''}`;
    li.querySelector('.value').textContent = inv.value_cents != null ? eur(inv.value_cents) : '—';
    li.querySelector('.gain').textContent =
      inv.gain_cents != null ? `${inv.gain_cents > 0 ? '+' : ''}${eur(inv.gain_cents)}` : '';
    li.querySelector('[data-action="ops"]').addEventListener('click', () => openOpsModal(inv));
    li.querySelector('[data-action="edit"]').addEventListener('click', () => openInvModal(inv));
    li.querySelector('[data-action="delete"]').addEventListener('click', async () => {
      if (!confirm(`¿Borrar la inversión "${inv.name}"?`)) return;
      await api('/investments/' + inv.id, { method: 'DELETE' });
      loadInvestments(true);
    });
    ul.appendChild(li);
    renderSparkline(li.querySelector('.inv-spark canvas'), inv);
  }
}

/* Mini-gráfica del precio de los últimos 30 días de una posición. */
function renderSparkline(canvas, inv) {
  const hist = inv.price_history || [];
  if (hist.length < 2) { canvas.parentElement.classList.add('hidden'); return; }
  const prices = hist.map((h) => h.price);
  const up = prices[prices.length - 1] >= prices[0];
  const color = up ? cssVar('--positive') : cssVar('--negative');
  const chart = new Chart(canvas, {
    type: 'line',
    data: {
      labels: hist.map((h) => h.date),
      datasets: [{ data: prices, borderColor: color, borderWidth: 1.5, pointRadius: 0, tension: 0.3, fill: false }],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: { legend: { display: false }, tooltip: { enabled: false } },
      scales: { x: { display: false }, y: { display: false } },
      elements: { line: { capBezierPoints: true } },
    },
  });
  sparkCharts.set(inv.id, chart);
}

async function loadInvestments(force = false) {
  try {
    state.investments = await api(force ? '/investments/refresh' : '/investments', force ? { method: 'POST' } : {});
    renderInvestments();
    renderAccounts();
    loadNetworth();
  } catch (_) { /* sin conexión: se mantiene lo último */ }
}

$('#btn-refresh-prices').addEventListener('click', () => loadInvestments(true));
$('#btn-new-investment').addEventListener('click', () => openInvModal());
$('#inv-cancel').addEventListener('click', () => invModal.close());

$('#inv-provider').addEventListener('change', () => {
  const manual = $('#inv-provider').value === 'manual';
  $('#inv-symbol-label').classList.toggle('hidden', manual);
  $('#inv-price-label').classList.toggle('hidden', !manual);
});

// Buscador de símbolos: sugiere tickers válidos (Yahoo) o ids (CoinGecko)
// mientras escribes, para no tener que saber los sufijos de bolsa (.DE, .MC…).
let symbolSearchTimer = null;
const symbolNames = new Map(); // símbolo elegido -> nombre, para autorrellenar

$('#inv-symbol').addEventListener('input', () => {
  const q = $('#inv-symbol').value.trim();
  const hint = $('#inv-symbol-hint');

  // Si lo escrito es una sugerencia elegida, autorrellenar el nombre si está vacío.
  if (symbolNames.has(q)) {
    if (!$('#inv-name').value.trim()) $('#inv-name').value = symbolNames.get(q);
    hint.textContent = symbolNames.get(q);
    return;
  }

  clearTimeout(symbolSearchTimer);
  if (q.length < 2) { hint.textContent = ''; return; }
  symbolSearchTimer = setTimeout(async () => {
    try {
      const provider = $('#inv-provider').value;
      const results = await api(`/investments/search?provider=${provider}&q=${encodeURIComponent(q)}`);
      const dl = $('#inv-symbol-options');
      dl.innerHTML = '';
      symbolNames.clear();
      for (const r of results) {
        const opt = document.createElement('option');
        opt.value = r.symbol;
        opt.label = `${r.name}${r.exchange ? ' · ' + r.exchange : ''}`;
        symbolNames.set(r.symbol, r.name);
        dl.appendChild(opt);
      }
      hint.textContent = results.length ? `${results.length} resultados — elige uno de la lista` : 'Sin resultados';
    } catch (_) { /* sin red: no molestar */ }
  }, 350);
});

function openInvModal(inv = null) {
  state.editingInv = inv;
  $('#inv-modal-title').textContent = inv ? 'Editar inversión' : 'Nueva inversión';
  $('#inv-error').classList.add('hidden');
  $('#inv-name').value = inv ? inv.name : '';
  $('#inv-provider').value = inv ? inv.provider : 'yahoo';
  $('#inv-symbol').value = inv ? inv.symbol : '';
  $('#inv-units').value = inv ? String(inv.units).replace('.', ',') : '';
  $('#inv-cost').value = inv && inv.cost_cents ? String(inv.cost_cents / 100).replace('.', ',') : '';
  $('#inv-price').value = inv && inv.manual_price != null ? String(inv.manual_price).replace('.', ',') : '';
  $('#inv-provider').dispatchEvent(new Event('change'));
  invModal.showModal();
  $('#inv-name').focus();
}

/* ===== Operaciones de una inversión (compras/ventas parciales) ===== */

const opsModal = $('#ops-modal');

async function openOpsModal(inv) {
  state.opsInvId = inv.id;
  $('#ops-title').textContent = `Operaciones · ${inv.name}`;
  $('#ops-error').classList.add('hidden');
  $('#op-type').value = 'compra';
  $('#op-units').value = '';
  $('#op-amount').value = '';
  $('#op-date').value = todayISO();
  await renderOps();
  opsModal.showModal();
}

async function renderOps() {
  const data = await api(`/investments/${state.opsInvId}/ops`);
  const parts = [`${String(data.units).replace('.', ',')} uds · coste ${eur(data.cost_cents)}`];
  if (data.realized_gain_cents) parts.push(`ganancia realizada ${data.realized_gain_cents > 0 ? '+' : ''}${eur(data.realized_gain_cents)}`);
  $('#ops-summary').textContent = parts.join(' · ');
  const ul = $('#ops-list');
  ul.innerHTML = '';
  for (const op of [...data.items].reverse()) {
    const li = document.createElement('li');
    li.className = 'rec-item';
    li.innerHTML = `
      <span class="tx-icon">${op.type === 'compra' ? '🟢' : '🔴'}</span>
      <div class="info">
        <div class="what"></div>
        <div class="when"></div>
      </div>
      <span class="tx-amount"></span>
      <span class="tx-actions">
        <button class="icon-btn" data-action="del" title="Borrar operación">🗑️</button>
      </span>`;
    li.querySelector('.what').textContent = `${cap(op.type)} · ${String(op.units).replace('.', ',')} uds`;
    li.querySelector('.when').textContent = cap(fmtDateLong.format(new Date(op.date + 'T12:00:00')));
    li.querySelector('.tx-amount').textContent = eur(op.amount_cents);
    li.querySelector('[data-action="del"]').addEventListener('click', async () => {
      if (!confirm('¿Borrar esta operación? Se recalcula la posición.')) return;
      try {
        await api('/investments/ops/' + op.id, { method: 'DELETE' });
        await renderOps();
        loadInvestments(true);
      } catch (err) {
        const el = $('#ops-error');
        el.textContent = err.message;
        el.classList.remove('hidden');
      }
    });
    ul.appendChild(li);
  }
}

$('#ops-close').addEventListener('click', () => opsModal.close());

$('#ops-form').addEventListener('submit', async (e) => {
  e.preventDefault();
  $('#ops-error').classList.add('hidden');
  const body = {
    type: $('#op-type').value,
    units: $('#op-units').value,
    amount: $('#op-amount').value,
    date: $('#op-date').value,
  };
  try {
    await api(`/investments/${state.opsInvId}/ops`, { method: 'POST', body: JSON.stringify(body) });
    $('#op-units').value = '';
    $('#op-amount').value = '';
    await renderOps();
    loadInvestments(true);
  } catch (err) {
    const el = $('#ops-error');
    el.textContent = err.message;
    el.classList.remove('hidden');
  }
});

$('#inv-form').addEventListener('submit', async (e) => {
  e.preventDefault();
  const body = {
    name: $('#inv-name').value,
    provider: $('#inv-provider').value,
    symbol: $('#inv-symbol').value,
    units: $('#inv-units').value,
    cost: $('#inv-cost').value,
    manual_price: $('#inv-price').value,
  };
  try {
    if (state.editingInv) {
      await api('/investments/' + state.editingInv.id, { method: 'PUT', body: JSON.stringify(body) });
    } else {
      await api('/investments', { method: 'POST', body: JSON.stringify(body) });
    }
    invModal.close();
    loadInvestments(true);
  } catch (err) {
    const el = $('#inv-error');
    el.textContent = err.message;
    el.classList.remove('hidden');
  }
});

/* ===== Objetivos de ahorro ===== */

const goalModal = $('#goal-modal');
const amountModal = $('#amount-modal');

async function loadGoals() {
  state.goals = await api('/goals');
  renderGoals();
}

function renderGoals() {
  const ul = $('#goal-list');
  ul.innerHTML = '';
  $('#goal-empty').classList.toggle('hidden', state.goals.length > 0);
  for (const g of state.goals) {
    const pct = Math.min(100, Math.round((g.saved_cents / g.target_cents) * 100));
    const done = g.saved_cents >= g.target_cents;
    const li = document.createElement('li');
    li.className = 'budget-item';
    li.innerHTML = `
      <div class="row">
        <span class="tx-icon"></span>
        <span class="name"></span>
        <span class="nums"></span>
        <span class="pct">${pct}%</span>
        <span class="tx-actions">
          <button class="icon-btn" data-action="add" title="Aportar">💰</button>
          <button class="icon-btn" data-action="edit" title="Editar">✏️</button>
          <button class="icon-btn" data-action="delete" title="Borrar">🗑️</button>
        </span>
      </div>
      <div class="budget-track"><div class="budget-fill goal-fill"></div></div>`;
    li.querySelector('.budget-fill').style.width = `${pct}%`;
    li.querySelector('.tx-icon').textContent = done ? '🏁' : g.icon;
    li.querySelector('.name').textContent = g.name + (done ? ' — ¡conseguido!' : '');
    let nums = `${eur(g.saved_cents)} de ${eur(g.target_cents)}`;
    if (!done && g.deadline) {
      const months = monthsUntil(g.deadline);
      const remaining = g.target_cents - g.saved_cents;
      if (months >= 1 && remaining > 0) nums += ` · ${eur(Math.ceil(remaining / months))}/mes hasta ${shortDate(g.deadline)}`;
      else if (months < 1 && remaining > 0) nums += ` · ¡se acaba el plazo (${shortDate(g.deadline)})!`;
    }
    li.querySelector('.nums').textContent = nums;
    li.querySelector('[data-action="add"]').addEventListener('click', () => openAmountModal(g));
    li.querySelector('[data-action="edit"]').addEventListener('click', () => openGoalModal(g));
    li.querySelector('[data-action="delete"]').addEventListener('click', async () => {
      if (!confirm(`¿Borrar el objetivo "${g.name}"?`)) return;
      await api('/goals/' + g.id, { method: 'DELETE' });
      loadGoals();
    });
    ul.appendChild(li);
  }
}

// Meses que quedan hasta una fecha YYYY-MM-DD (<= 0 si ya pasó).
function monthsUntil(dateStr) {
  const now = new Date();
  const to = new Date(dateStr + 'T12:00:00');
  let months = (to.getFullYear() - now.getFullYear()) * 12 + (to.getMonth() - now.getMonth());
  if (to.getDate() >= now.getDate()) months += 1; // el resto del mes en curso cuenta como un mes
  return months;
}
const fmtShortDate = new Intl.DateTimeFormat('es-ES', { month: 'short', year: 'numeric' });
function shortDate(dateStr) {
  return fmtShortDate.format(new Date(dateStr + 'T12:00:00')).replace('.', '');
}

function openGoalModal(goal = null) {
  state.editingGoal = goal;
  $('#goal-modal-title').textContent = goal ? 'Editar objetivo' : 'Nuevo objetivo';
  $('#goal-error').classList.add('hidden');
  $('#goal-name').value = goal ? goal.name : '';
  $('#goal-icon').value = goal ? goal.icon : '';
  $('#goal-target').value = goal ? String(goal.target_cents / 100).replace('.', ',') : '';
  $('#goal-deadline').value = goal && goal.deadline ? goal.deadline : '';
  goalModal.showModal();
  $('#goal-name').focus();
}

$('#btn-new-goal').addEventListener('click', () => openGoalModal());
$('#goal-cancel').addEventListener('click', () => goalModal.close());

$('#goal-form').addEventListener('submit', async (e) => {
  e.preventDefault();
  const body = {
    name: $('#goal-name').value,
    icon: $('#goal-icon').value || '🎯',
    target: $('#goal-target').value,
    deadline: $('#goal-deadline').value || '',
  };
  try {
    if (state.editingGoal) {
      await api('/goals/' + state.editingGoal.id, { method: 'PUT', body: JSON.stringify(body) });
    } else {
      await api('/goals', { method: 'POST', body: JSON.stringify(body) });
    }
    goalModal.close();
    loadGoals();
  } catch (err) {
    const el = $('#goal-error');
    el.textContent = err.message;
    el.classList.remove('hidden');
  }
});

function openAmountModal(goal) {
  state.editingGoal = goal;
  $('#amount-modal-title').textContent = `Aportar a ${goal.name}`;
  $('#amount-error').classList.add('hidden');
  $('#amount-input').value = '';
  amountModal.showModal();
  $('#amount-input').focus();
}

$('#amount-cancel').addEventListener('click', () => amountModal.close());

$('#amount-form').addEventListener('submit', async (e) => {
  e.preventDefault();
  try {
    await api('/goals/' + state.editingGoal.id + '/add', {
      method: 'POST',
      body: JSON.stringify({ amount: $('#amount-input').value }),
    });
    amountModal.close();
    loadGoals();
  } catch (err) {
    const el = $('#amount-error');
    el.textContent = err.message;
    el.classList.remove('hidden');
  }
});

/* ===== Vista Anual ===== */

const MESES_CORTOS = ['Ene', 'Feb', 'Mar', 'Abr', 'May', 'Jun', 'Jul', 'Ago', 'Sep', 'Oct', 'Nov', 'Dic'];

$('#year-prev').addEventListener('click', () => { state.year -= 1; refreshYear(); });
$('#year-next').addEventListener('click', () => { state.year += 1; refreshYear(); });

async function refreshYear() {
  const d = await api('/year?year=' + state.year);
  state.yearData = d;
  $('#year-label').textContent = String(d.year);
  $('#year-ingresos').textContent = eur(d.ingresos_cents);
  $('#year-gastos').textContent = eur(d.gastos_cents);
  const bal = $('#year-balance');
  const balance = d.ingresos_cents - d.gastos_cents;
  bal.textContent = (balance > 0 ? '+' : '') + eur(balance);
  bal.classList.toggle('positive', balance > 0);
  bal.classList.toggle('negative', balance < 0);
  renderDelta($('#year-delta-ingresos'), d.ingresos_cents, d.prev_ingresos_cents, `${d.year - 1}-01`, true);
  renderDelta($('#year-delta-gastos'), d.gastos_cents, d.prev_gastos_cents, `${d.year - 1}-01`, false);
  $('#year-delta-ingresos').textContent = $('#year-delta-ingresos').textContent.replace(/vs .+$/, `vs ${d.year - 1}`);
  $('#year-delta-gastos').textContent = $('#year-delta-gastos').textContent.replace(/vs .+$/, `vs ${d.year - 1}`);

  const table = $('#year-table');
  table.innerHTML = '<thead><tr><th>Mes</th><th>Ingresos</th><th>Gastos</th><th>Balance</th></tr></thead>';
  const tbody = document.createElement('tbody');
  for (let i = 0; i < 12; i++) {
    const m = d.months[i];
    const b = m.ingresos_cents - m.gastos_cents;
    const empty = m.ingresos_cents === 0 && m.gastos_cents === 0;
    const tr = document.createElement('tr');
    if (empty) tr.className = 'dim';
    tr.innerHTML = `<td>${MESES_CORTOS[i]}</td><td>${empty ? '—' : eur(m.ingresos_cents)}</td><td>${empty ? '—' : eur(m.gastos_cents)}</td><td class="${b > 0 ? 'positive' : b < 0 ? 'negative' : ''}">${empty ? '—' : eur(b)}</td>`;
    tbody.appendChild(tr);
  }
  const totalTr = document.createElement('tr');
  totalTr.className = 'total';
  totalTr.innerHTML = `<td>Total</td><td>${eur(d.ingresos_cents)}</td><td>${eur(d.gastos_cents)}</td><td class="${balance > 0 ? 'positive' : balance < 0 ? 'negative' : ''}">${eur(balance)}</td>`;
  tbody.appendChild(totalTr);
  table.appendChild(tbody);

  const ul = $('#year-cats');
  ul.innerHTML = '';
  $('#year-cats-empty').classList.toggle('hidden', d.by_category.length > 0);
  const totalGastos = d.gastos_cents || 1;
  for (const c of d.by_category) {
    const li = document.createElement('li');
    const pct = Math.round((c.total_cents / totalGastos) * 100);
    li.innerHTML = `<span class="swatch"></span><span class="name"></span><span class="val"></span><span class="pct">${pct}%</span>`;
    li.querySelector('.swatch').style.background = catColor(c.color || '#898781');
    li.querySelector('.name').textContent = `${c.icon || ''} ${c.name || 'Sin categoría'}`.trim();
    li.querySelector('.val').textContent = eur(c.total_cents);
    ul.appendChild(li);
  }
}

/* ===== Ajustes ===== */

let apiToken = '';

async function loadAjustes() {
  $('#api-url').textContent = location.origin + '/api/transactions';
  apiToken = state.apiTokenValue || '';
  $('#global-budget').value =
    state.globalBudgetCents > 0 ? String(state.globalBudgetCents / 100).replace('.', ',') : '';
  fillAccountSelect($('#default-account'), $('#default-account-new'), state.defaultAccount);
}

wireAccountSelect('#default-account', '#default-account-new');

$('#btn-save-default-account').addEventListener('click', async () => {
  const btn = $('#btn-save-default-account');
  try {
    const r = await api('/settings', {
      method: 'PUT',
      body: JSON.stringify({ default_account: accountValue($('#default-account'), $('#default-account-new')) }),
    });
    state.defaultAccount = r.default_account;
    btn.textContent = '✓ Guardado';
    setTimeout(() => (btn.textContent = 'Guardar'), 1200);
  } catch (err) {
    alert(err.message);
  }
});

$('#btn-save-budget').addEventListener('click', async () => {
  const btn = $('#btn-save-budget');
  try {
    const r = await api('/settings', {
      method: 'PUT',
      body: JSON.stringify({ global_budget: $('#global-budget').value.trim() || 0 }),
    });
    state.globalBudgetCents = r.global_budget_cents;
    renderBudgets();
    renderStatsStrip(state.summary);
    btn.textContent = '✓ Guardado';
    setTimeout(() => (btn.textContent = 'Guardar'), 1200);
  } catch (err) {
    alert(err.message);
  }
});

/* ===== Importador CSV ===== */

const csvState = { headers: [], rows: [] };

// Parser CSV mínimo con soporte de comillas; detecta ; o , como separador.
function parseCSV(text) {
  const firstLine = text.slice(0, text.indexOf('\n') + 1 || text.length);
  const sep = (firstLine.match(/;/g) || []).length >= (firstLine.match(/,/g) || []).length ? ';' : ',';
  const rows = [];
  let row = [];
  let field = '';
  let inQuotes = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (inQuotes) {
      if (ch === '"' && text[i + 1] === '"') { field += '"'; i++; }
      else if (ch === '"') inQuotes = false;
      else field += ch;
    } else if (ch === '"') inQuotes = true;
    else if (ch === sep) { row.push(field); field = ''; }
    else if (ch === '\n' || ch === '\r') {
      if (ch === '\r' && text[i + 1] === '\n') i++;
      row.push(field); field = '';
      if (row.some((c) => c.trim() !== '')) rows.push(row);
      row = [];
    } else field += ch;
  }
  row.push(field);
  if (row.some((c) => c.trim() !== '')) rows.push(row);
  return rows;
}

function parseCsvDate(s) {
  s = String(s || '').trim().split(' ')[0];
  let m = s.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (m) return s;
  m = s.match(/^(\d{1,2})[/\-.](\d{1,2})[/\-.](\d{4})$/);
  if (m) return `${m[3]}-${m[2].padStart(2, '0')}-${m[1].padStart(2, '0')}`;
  return null;
}

function guessColumn(headers, candidates) {
  const lower = headers.map((h) => h.toLowerCase());
  for (const c of candidates) {
    const i = lower.findIndex((h) => h.includes(c));
    if (i !== -1) return i;
  }
  return -1;
}

$('#csv-file').addEventListener('change', () => {
  const file = $('#csv-file').files[0];
  $('#csv-result').textContent = '';
  if (!file) return;
  const reader = new FileReader();
  reader.onload = () => {
    const rows = parseCSV(String(reader.result));
    if (rows.length < 2) {
      $('#csv-result').textContent = 'El archivo no tiene datos (hace falta cabecera + filas).';
      return;
    }
    csvState.headers = rows[0].map((h) => h.trim());
    csvState.rows = rows.slice(1);
    renderCsvMapper();
  };
  reader.readAsText(file);
});

function renderCsvMapper() {
  const wrap = $('#csv-selects');
  wrap.innerHTML = '';
  const fields = [
    ['fecha', 'Columna de fecha', ['fecha', 'date', 'día', 'dia']],
    ['importe', 'Columna de importe', ['importe', 'amount', 'cantidad', 'monto', 'valor']],
    ['nota', 'Columna de nota (opcional)', ['nota', 'descrip', 'concepto', 'note']],
    ['categoria', 'Columna de categoría (opcional)', ['categor', 'category']],
    ['cuenta', 'Columna de cuenta (opcional)', ['cuenta', 'account', 'banco']],
  ];
  for (const [key, label, candidates] of fields) {
    const lab = document.createElement('label');
    lab.textContent = label;
    const sel = document.createElement('select');
    sel.dataset.field = key;
    const none = document.createElement('option');
    none.value = '-1';
    none.textContent = '(ninguna)';
    sel.appendChild(none);
    csvState.headers.forEach((h, i) => {
      const opt = document.createElement('option');
      opt.value = String(i);
      opt.textContent = h || `Columna ${i + 1}`;
      sel.appendChild(opt);
    });
    sel.value = String(guessColumn(csvState.headers, candidates));
    sel.addEventListener('change', renderCsvPreview);
    lab.appendChild(sel);
    wrap.appendChild(lab);
  }
  const typeLab = document.createElement('label');
  typeLab.textContent = 'Tipo de los movimientos';
  const typeSel = document.createElement('select');
  typeSel.id = 'csv-type-mode';
  for (const [v, t] of [['signo', 'Según el signo del importe (negativo = gasto)'], ['gasto', 'Todos gastos'], ['ingreso', 'Todos ingresos']]) {
    const opt = document.createElement('option');
    opt.value = v;
    opt.textContent = t;
    typeSel.appendChild(opt);
  }
  typeSel.addEventListener('change', renderCsvPreview);
  typeLab.appendChild(typeSel);
  wrap.appendChild(typeLab);

  $('#csv-mapper').classList.remove('hidden');
  renderCsvPreview();
}

function mapCsvRows() {
  const col = {};
  document.querySelectorAll('#csv-selects select[data-field]').forEach((s) => {
    col[s.dataset.field] = Number(s.value);
  });
  const mode = $('#csv-type-mode') ? $('#csv-type-mode').value : 'signo';
  const items = [];
  const errors = [];
  csvState.rows.forEach((row, i) => {
    const date = col.fecha >= 0 ? parseCsvDate(row[col.fecha]) : null;
    const rawAmount = col.importe >= 0 ? String(row[col.importe] || '').trim() : '';
    if (!date || !rawAmount) {
      errors.push(i);
      return;
    }
    const negative = rawAmount.startsWith('-');
    const amount = rawAmount.replace(/^[-+]/, '');
    const type = mode === 'signo' ? (negative ? 'gasto' : 'ingreso') : mode;
    items.push({
      type,
      amount,
      date,
      note: col.nota >= 0 ? String(row[col.nota] || '').trim() : '',
      category: col.categoria >= 0 ? String(row[col.categoria] || '').trim() : '',
      account: col.cuenta >= 0 ? String(row[col.cuenta] || '').trim() : '',
      source: 'csv',
    });
  });
  return { items, skipped: errors.length };
}

function renderCsvPreview() {
  const { items, skipped } = mapCsvRows();
  const table = $('#csv-preview');
  table.innerHTML = '<thead><tr><th>Fecha</th><th>Tipo</th><th>Importe</th><th>Categoría</th><th>Nota</th><th>Cuenta</th></tr></thead>';
  const tbody = document.createElement('tbody');
  for (const it of items.slice(0, 5)) {
    const tr = document.createElement('tr');
    for (const v of [it.date, it.type, it.amount, it.category, it.note, it.account]) {
      const td = document.createElement('td');
      td.textContent = String(v || '');
      tr.appendChild(td);
    }
    tbody.appendChild(tr);
  }
  table.appendChild(tbody);
  $('#csv-count').textContent =
    `${items.length} movimientos listos para importar` +
    (skipped ? ` (${skipped} filas sin fecha o importe válidos se ignorarán)` : '');
}

$('#btn-csv-import').addEventListener('click', async () => {
  const { items } = mapCsvRows();
  if (items.length === 0) {
    $('#csv-result').textContent = 'Nada que importar: revisa las columnas elegidas.';
    return;
  }
  if (!confirm(`¿Importar ${items.length} movimientos? Los duplicados exactos se saltarán.`)) return;
  try {
    const r = await api('/transactions/batch', {
      method: 'POST',
      body: JSON.stringify({ items, skip_duplicates: true }),
    });
    $('#csv-result').textContent =
      `Importados ${r.created} · duplicados saltados ${r.skipped_duplicates}` +
      (r.errors.length ? ` · con error ${r.errors.length}` : '');
    $('#csv-mapper').classList.add('hidden');
    $('#csv-file').value = '';
    refreshAll();
  } catch (err) {
    $('#csv-result').textContent = 'Error: ' + err.message;
  }
});

$('#btn-show-token').addEventListener('click', (e) => {
  const el = $('#api-token');
  const shown = el.dataset.shown === '1';
  el.textContent = shown ? '••••••••' : apiToken || '(no disponible)';
  el.dataset.shown = shown ? '0' : '1';
  e.target.textContent = shown ? 'Mostrar' : 'Ocultar';
});

document.querySelectorAll('[data-copy]').forEach((btn) => {
  btn.addEventListener('click', async () => {
    const id = btn.dataset.copy;
    const text = id === 'api-token' ? apiToken : $('#' + id).textContent;
    try {
      await navigator.clipboard.writeText(text);
      const prev = btn.textContent;
      btn.textContent = '✓ Copiado';
      setTimeout(() => (btn.textContent = prev), 1200);
    } catch (_) {
      prompt('Copia manualmente:', text);
    }
  });
});

$('#btn-logout').addEventListener('click', async () => {
  await api('/logout', { method: 'POST' });
  localStorage.removeItem(WAS_AUTH_KEY);
  showLogin();
});

/* ===== Arranque ===== */

(async function init() {
  try {
    const me = await fetch('api/me').then((r) => r.json());
    if (me.authenticated) {
      localStorage.setItem(WAS_AUTH_KEY, '1');
      await showApp();
      flushQueue();
    } else {
      localStorage.removeItem(WAS_AUTH_KEY);
      showLogin();
    }
  } catch (_) {
    serverUp = false; // api/me ha fallado: el servidor no está
    // Sin servidor: si la última sesión estaba abierta, entramos igual y
    // trabajamos con lo que tenga la caché. La cookie sigue mandando en cuanto
    // vuelva la red — esto solo decide qué pantalla enseñar.
    if (localStorage.getItem(WAS_AUTH_KEY)) {
      try {
        await showApp();
      } catch (_) {
        /* caché parcial: la app ya está visible, cada vista pinta lo que tenga */
      }
    } else {
      showLogin();
    }
  }
  renderLed();
  renderOfflineBar();
  pingServer();
})();
