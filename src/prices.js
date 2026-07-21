'use strict';

const db = require('./db');

// Precios de inversiones con caché en BD (tabla price_cache). Todo se guarda
// como EUR por unidad. Si no hay internet, se sirve el último precio cacheado
// (con su fecha) en vez de fallar.

const TTL_MINUTES = 15;
const FETCH_TIMEOUT_MS = 8000;
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) FinanZillo/1.0';

const getCache = db.prepare('SELECT price, fetched_at FROM price_cache WHERE key = ?');
const setCache = db.prepare(
  `INSERT INTO price_cache (key, price, fetched_at) VALUES (?, ?, datetime('now', 'localtime'))
   ON CONFLICT(key) DO UPDATE SET price = excluded.price, fetched_at = excluded.fetched_at`
);

function cacheAgeMinutes(fetchedAt) {
  return (Date.now() - new Date(fetchedAt.replace(' ', 'T')).getTime()) / 60000;
}

async function fetchJson(url) {
  const res = await fetch(url, {
    headers: { 'User-Agent': UA, Accept: 'application/json' },
    signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
}

// Yahoo: precio de mercado + divisa en la que cotiza.
async function fetchYahoo(symbol) {
  let data;
  try {
    data = await fetchJson(
      `https://query1.finance.yahoo.com/v8/finance/chart/${encodeURIComponent(symbol)}?range=1d&interval=1d`
    );
  } catch (e) {
    if (String(e.message).includes('404')) {
      throw new Error(`"${symbol}" no existe en Yahoo — usa el buscador del formulario (las bolsas europeas llevan sufijo: .DE Xetra, .MC Madrid, .AS Ámsterdam, .L Londres)`);
    }
    throw e;
  }
  const meta = data && data.chart && data.chart.result && data.chart.result[0] && data.chart.result[0].meta;
  if (!meta || typeof meta.regularMarketPrice !== 'number') {
    throw new Error('Símbolo no encontrado en Yahoo');
  }
  return { price: meta.regularMarketPrice, currency: (meta.currency || 'EUR').toUpperCase() };
}

// Búsqueda de símbolos para el formulario de inversiones.
async function searchSymbols(provider, query) {
  if (provider === 'coingecko') {
    const data = await fetchJson(
      `https://api.coingecko.com/api/v3/search?query=${encodeURIComponent(query)}`
    );
    return (data.coins || []).slice(0, 8).map((c) => ({
      symbol: c.id,
      name: `${c.name} (${String(c.symbol || '').toUpperCase()})`,
      exchange: 'CoinGecko',
    }));
  }
  const yahooSearch = async (q) => {
    const data = await fetchJson(
      `https://query1.finance.yahoo.com/v1/finance/search?q=${encodeURIComponent(q)}&quotesCount=8&newsCount=0`
    );
    return (data.quotes || [])
      .filter((x) => x.symbol && ['EQUITY', 'ETF', 'MUTUALFUND', 'INDEX', 'CRYPTOCURRENCY'].includes(x.quoteType))
      .map((x) => ({
        symbol: x.symbol,
        name: x.shortname || x.longname || x.symbol,
        exchange: x.exchDisp || x.exchange || '',
      }));
  };
  let results = await yahooSearch(query);
  // El buscador de Yahoo es quisquilloso con frases en minúsculas ("ishares acwi" → 0);
  // si no hay nada, reintentar con la última palabra (suele ser el nombre/ticker clave).
  if (results.length === 0) {
    const words = query.split(/\s+/).filter(Boolean);
    if (words.length > 1) results = await yahooSearch(words[words.length - 1]);
  }
  return results;
}

// Unidades de `currency` por 1 EUR (para convertir cotizaciones no-EUR).
async function fxPerEur(currency) {
  const key = `fx:${currency}`;
  const cached = getCache.get(key);
  if (cached && cacheAgeMinutes(cached.fetched_at) < 60) return cached.price;
  const { price } = await fetchYahoo(`EUR${currency}=X`);
  setCache.run(key, price);
  return price;
}

async function fetchPriceEur(provider, symbol) {
  if (provider === 'coingecko') {
    const id = symbol.toLowerCase();
    const data = await fetchJson(
      `https://api.coingecko.com/api/v3/simple/price?ids=${encodeURIComponent(id)}&vs_currencies=eur`
    );
    const price = data && data[id] && data[id].eur;
    if (typeof price !== 'number') throw new Error('Id no encontrado en CoinGecko');
    return price;
  }
  const { price, currency } = await fetchYahoo(symbol);
  if (currency === 'EUR') return price;
  // GBp = peniques (Bolsa de Londres): céntimos de libra.
  if (currency === 'GBP' || currency === 'GBp') {
    const rate = await fxPerEur('GBP');
    return (currency === 'GBp' ? price / 100 : price) / rate;
  }
  const rate = await fxPerEur(currency);
  return price / rate;
}

/**
 * Devuelve { price, fetched_at, error } para una inversión. Refresca si el
 * caché tiene más de TTL_MINUTES (o siempre con force); si el refresco falla,
 * devuelve el último precio conocido junto al error.
 */
async function getPriceEur(provider, symbol, force = false) {
  const key = `${provider}:${symbol.toLowerCase()}`;
  const cached = getCache.get(key);
  if (cached && !force && cacheAgeMinutes(cached.fetched_at) < TTL_MINUTES) {
    return { price: cached.price, fetched_at: cached.fetched_at, error: null };
  }
  try {
    const price = await fetchPriceEur(provider, symbol);
    setCache.run(key, price);
    const fresh = getCache.get(key);
    return { price, fetched_at: fresh.fetched_at, error: null };
  } catch (e) {
    return {
      price: cached ? cached.price : null,
      fetched_at: cached ? cached.fetched_at : null,
      error: e.message,
    };
  }
}

module.exports = { getPriceEur, searchSymbols };
