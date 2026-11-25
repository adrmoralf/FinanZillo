//! Inversiones: posiciones, operaciones (coste medio) y precios.
//!
//! La red nunca se toca desde el hilo de la interfaz: `fetch_prices` y
//! `search_symbols` son funciones puras de red que la UI lanza en un hilo, y
//! luego guarda los resultados con `store_prices`.

use std::collections::HashMap;
use std::time::Duration;

use rusqlite::{OptionalExtension, params};

use super::{Core, R, SqlErr, err};
use crate::util;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) FinanZillo/2.0";

#[derive(Clone, Debug, Default)]
pub struct Investment {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub symbol: String,
    pub units: f64,
    pub cost_cents: i64,
    pub manual_price: Option<f64>,
    pub price_eur: Option<f64>,
    pub price_fetched_at: Option<String>,
    pub value_cents: Option<i64>,
    pub history: Vec<(String, f64)>,
}

impl Investment {
    pub fn gain_cents(&self) -> Option<i64> {
        self.value_cents.map(|v| v - self.cost_cents)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Portfolio {
    pub items: Vec<Investment>,
    pub value_cents: i64,
    pub cost_cents: i64,
}

impl Portfolio {
    pub fn gain_cents(&self) -> i64 {
        self.value_cents - self.cost_cents
    }
}

#[derive(Clone, Debug)]
pub struct Op {
    pub id: i64,
    pub ty: String,
    pub units: f64,
    pub amount_cents: i64,
    pub date: String,
}

#[derive(Clone, Debug, Default)]
pub struct OpsReplay {
    pub ops: Vec<Op>,
    pub units: f64,
    pub cost_cents: i64,
    pub realized_gain_cents: i64,
}

#[derive(Clone, Debug)]
pub struct SymbolHit {
    pub symbol: String,
    pub name: String,
    pub exchange: String,
}

pub struct PriceResult {
    pub key: String,
    pub price: Result<f64, String>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(8)))
        .user_agent(UA)
        .build()
        .into()
}

fn get_json(agent: &ureq::Agent, url: &str) -> Result<serde_json::Value, String> {
    let mut res = agent.get(url).header("Accept", "application/json").call().map_err(|e| match e {
        ureq::Error::StatusCode(404) => "404".to_string(),
        ureq::Error::StatusCode(c) => format!("HTTP {c}"),
        _ => "Sin conexión con el servidor de precios".to_string(),
    })?;
    res.body_mut().read_json::<serde_json::Value>().map_err(|e| e.to_string())
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn yahoo_quote(agent: &ureq::Agent, symbol: &str) -> Result<(f64, String), String> {
    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?range=1d&interval=1d", enc(symbol));
    let data = get_json(agent, &url).map_err(|e| {
        if e == "404" {
            format!("«{symbol}» no existe en Yahoo: usa el buscador (las bolsas europeas llevan sufijo: .DE, .MC, .AS, .L)")
        } else {
            e
        }
    })?;
    let meta = &data["chart"]["result"][0]["meta"];
    let price = meta["regularMarketPrice"].as_f64().ok_or("Símbolo no encontrado en Yahoo")?;
    let cur = meta["currency"].as_str().unwrap_or("EUR").to_string();
    Ok((price, cur))
}

/// Precio en EUR por unidad (red). `fx` cachea los tipos de cambio de esta tanda.
fn fetch_one(agent: &ureq::Agent, provider: &str, symbol: &str, fx: &mut HashMap<String, f64>) -> Result<f64, String> {
    if provider == "coingecko" {
        let id = symbol.to_lowercase();
        let data = get_json(agent, &format!("https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=eur", enc(&id)))?;
        return data[&id]["eur"].as_f64().ok_or_else(|| "Id no encontrado en CoinGecko".to_string());
    }
    let (price, cur) = yahoo_quote(agent, symbol)?;
    if cur.eq_ignore_ascii_case("EUR") {
        return Ok(price);
    }
    let base = if cur == "GBp" { "GBP".to_string() } else { cur.to_uppercase() };
    let rate = match fx.get(&base) {
        Some(r) => *r,
        None => {
            let (r, _) = yahoo_quote(agent, &format!("EUR{base}=X"))?;
            fx.insert(base.clone(), r);
            r
        }
    };
    let p = if cur == "GBp" { price / 100.0 } else { price };
    Ok(p / rate)
}

/// Descarga precios (en un hilo de fondo). Entrada: (proveedor, símbolo).
pub fn fetch_prices(items: Vec<(String, String)>) -> Vec<PriceResult> {
    let agent = agent();
    let mut fx = HashMap::new();
    items
        .into_iter()
        .map(|(p, s)| PriceResult { key: format!("{p}:{}", s.to_lowercase()), price: fetch_one(&agent, &p, &s, &mut fx) })
        .collect()
}

/// Buscador de símbolos (en un hilo de fondo).
pub fn search_symbols(provider: &str, query: &str) -> Vec<SymbolHit> {
    let agent = agent();
    if provider == "coingecko" {
        let Ok(d) = get_json(&agent, &format!("https://api.coingecko.com/api/v3/search?query={}", enc(query))) else {
            return vec![];
        };
        return d["coins"]
            .as_array()
            .map(|a| {
                a.iter()
                    .take(8)
                    .map(|c| SymbolHit {
                        symbol: c["id"].as_str().unwrap_or("").into(),
                        name: format!("{} ({})", c["name"].as_str().unwrap_or(""), c["symbol"].as_str().unwrap_or("").to_uppercase()),
                        exchange: "CoinGecko".into(),
                    })
                    .collect()
            })
            .unwrap_or_default();
    }
    let yahoo = |q: &str| -> Vec<SymbolHit> {
        let Ok(d) = get_json(&agent, &format!("https://query1.finance.yahoo.com/v1/finance/search?q={}&quotesCount=8&newsCount=0", enc(q)))
        else {
            return vec![];
        };
        d["quotes"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|x| {
                        x["symbol"].is_string()
                            && ["EQUITY", "ETF", "MUTUALFUND", "INDEX", "CRYPTOCURRENCY"].contains(&x["quoteType"].as_str().unwrap_or(""))
                    })
                    .map(|x| SymbolHit {
                        symbol: x["symbol"].as_str().unwrap_or("").into(),
                        name: x["shortname"].as_str().or(x["longname"].as_str()).unwrap_or("").into(),
                        exchange: x["exchDisp"].as_str().or(x["exchange"].as_str()).unwrap_or("").into(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut r = yahoo(query);
    if r.is_empty() {
        if let Some(last) = query.split_whitespace().last().filter(|_| query.split_whitespace().count() > 1) {
            r = yahoo(last);
        }
    }
    r
}

impl Core {
    pub fn portfolio(&self) -> Portfolio {
        let rows: Vec<Investment> = self
            .conn
            .prepare("SELECT id, name, provider, symbol, units, cost_cents, manual_price FROM investments WHERE archived = 0 ORDER BY name")
            .and_then(|mut st| {
                st.query_map([], |r| {
                    Ok(Investment {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        provider: r.get(2)?,
                        symbol: r.get(3)?,
                        units: r.get(4)?,
                        cost_cents: r.get(5)?,
                        manual_price: r.get(6)?,
                        ..Default::default()
                    })
                })
                .map(|it| it.flatten().collect())
            })
            .unwrap_or_default();
        let from = util::fmt_date(util::today() - chrono::Duration::days(31));
        let mut items = Vec::new();
        let (mut value, mut cost) = (0, 0);
        for mut inv in rows {
            if inv.provider == "manual" {
                inv.price_eur = inv.manual_price;
            } else {
                let key = format!("{}:{}", inv.provider, inv.symbol.to_lowercase());
                if let Some((p, at)) = self
                    .conn
                    .query_row("SELECT price, fetched_at FROM price_cache WHERE key = ?", [key], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()
                    .ok()
                    .flatten()
                {
                    inv.price_eur = Some(p);
                    inv.price_fetched_at = Some(at);
                }
            }
            inv.value_cents = inv.price_eur.map(|p| (inv.units * p * 100.0).round() as i64);
            inv.history = self
                .conn
                .prepare("SELECT date, price FROM investment_prices WHERE investment_id = ? AND date >= ? ORDER BY date")
                .and_then(|mut st| st.query_map(params![inv.id, from], |r| Ok((r.get(0)?, r.get(1)?))).map(|it| it.flatten().collect()))
                .unwrap_or_default();
            value += inv.value_cents.unwrap_or(0);
            cost += inv.cost_cents;
            items.push(inv);
        }
        Portfolio { items, value_cents: value, cost_cents: cost }
    }

    /// Lo que hay que pedir a la red (posiciones no manuales).
    pub fn price_requests(&self) -> Vec<(String, String)> {
        self.portfolio()
            .items
            .into_iter()
            .filter(|i| i.provider != "manual" && !i.symbol.is_empty())
            .map(|i| (i.provider, i.symbol))
            .collect()
    }

    /// ¿Hace falta refrescar? (algún precio con más de 15 minutos o sin precio)
    pub fn prices_stale(&self) -> bool {
        self.portfolio().items.iter().any(|i| {
            i.provider != "manual"
                && i.price_fetched_at.as_deref().is_none_or(|at| {
                    chrono::NaiveDateTime::parse_from_str(at, "%Y-%m-%d %H:%M:%S")
                        .map(|t| (chrono::Local::now().naive_local() - t).num_minutes() >= 15)
                        .unwrap_or(true)
                })
        })
    }

    /// Guarda precios descargados, la foto diaria de cada posición y la del patrimonio.
    pub fn store_prices(&mut self, results: &[PriceResult]) -> Vec<String> {
        let mut errors = Vec::new();
        for r in results {
            match &r.price {
                Ok(p) => {
                    let _ = self.conn.execute(
                        "INSERT INTO price_cache (key, price, fetched_at) VALUES (?, ?, datetime('now', 'localtime'))
                         ON CONFLICT(key) DO UPDATE SET price = excluded.price, fetched_at = excluded.fetched_at",
                        params![r.key, p],
                    );
                }
                Err(e) => errors.push(format!("{}: {e}", r.key.split_once(':').map(|x| x.1).unwrap_or(&r.key).to_uppercase())),
            }
        }
        let pf = self.portfolio();
        let today = util::today_str();
        for i in &pf.items {
            if let Some(p) = i.price_eur {
                let _ = self.conn.execute(
                    "INSERT INTO investment_prices (investment_id, date, price) VALUES (?, ?, ?)
                     ON CONFLICT(investment_id, date) DO UPDATE SET price = excluded.price",
                    params![i.id, today, p],
                );
            }
        }
        if !pf.items.is_empty() && pf.items.iter().all(|i| i.value_cents.is_some()) {
            let _ = self.conn.execute(
                "INSERT INTO networth_snapshots (date, investments_cents) VALUES (?, ?)
                 ON CONFLICT(date) DO UPDATE SET investments_cents = excluded.investments_cents",
                params![today, pf.value_cents],
            );
        }
        self.touch();
        errors
    }

    pub fn save_investment(
        &mut self,
        id: Option<i64>,
        name: &str,
        provider: &str,
        symbol: &str,
        units: f64,
        cost_cents: i64,
        manual_price: Option<f64>,
    ) -> R<i64> {
        let name = name.trim();
        if name.is_empty() {
            return err("Falta el nombre");
        }
        if !["yahoo", "coingecko", "manual"].contains(&provider) {
            return err("Fuente de precio no válida");
        }
        let symbol: String = symbol.trim().chars().take(40).collect();
        if provider != "manual" && symbol.is_empty() {
            return err("Falta el símbolo (ticker)");
        }
        if !(units.is_finite() && units > 0.0) {
            return err("Unidades no válidas");
        }
        if cost_cents < 0 {
            return err("Coste no válido");
        }
        if provider == "manual" && manual_price.is_none_or(|p| p < 0.0) {
            return err("Pon el precio por unidad");
        }
        let today = util::today_str();
        let rid = match id {
            None => {
                self.conn
                    .execute(
                        "INSERT INTO investments (name, provider, symbol, units, cost_cents, manual_price) VALUES (?, ?, ?, ?, ?, ?)",
                        params![name, provider, symbol, units, cost_cents, manual_price],
                    )
                    .e()?;
                let nid = self.conn.last_insert_rowid();
                self.conn
                    .execute(
                        "INSERT INTO investment_ops (investment_id, type, units, amount_cents, date) VALUES (?, 'compra', ?, ?, ?)",
                        params![nid, units, cost_cents, today],
                    )
                    .e()?;
                nid
            }
            Some(id) => {
                let (old_units, old_cost): (f64, i64) = self
                    .conn
                    .query_row("SELECT units, cost_cents FROM investments WHERE id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?)))
                    .map_err(|_| "Inversión no encontrada".to_string())?;
                self.conn
                    .execute(
                        "UPDATE investments SET name = ?, provider = ?, symbol = ?, units = ?, cost_cents = ?, manual_price = ? WHERE id = ?",
                        params![name, provider, symbol, units, cost_cents, manual_price, id],
                    )
                    .e()?;
                // Editar la posición a mano reinicia el historial a un único lote.
                if (units - old_units).abs() > 1e-12 || cost_cents != old_cost {
                    self.conn.execute("DELETE FROM investment_ops WHERE investment_id = ?", [id]).e()?;
                    self.conn
                        .execute(
                            "INSERT INTO investment_ops (investment_id, type, units, amount_cents, date) VALUES (?, 'compra', ?, ?, ?)",
                            params![id, units, cost_cents, today],
                        )
                        .e()?;
                }
                id
            }
        };
        self.touch();
        Ok(rid)
    }

    pub fn delete_investment(&mut self, id: i64) -> R<()> {
        self.conn.execute("DELETE FROM investment_ops WHERE investment_id = ?", [id]).e()?;
        self.conn.execute("DELETE FROM investment_prices WHERE investment_id = ?", [id]).e()?;
        self.conn.execute("DELETE FROM investments WHERE id = ?", [id]).e()?;
        self.touch();
        Ok(())
    }

    /// Recorre las operaciones con coste medio.
    pub fn replay_ops(&self, inv_id: i64) -> R<OpsReplay> {
        let ops: Vec<Op> = self
            .conn
            .prepare("SELECT id, type, units, amount_cents, date FROM investment_ops WHERE investment_id = ? ORDER BY date, id")
            .and_then(|mut st| {
                st.query_map([inv_id], |r| Ok(Op { id: r.get(0)?, ty: r.get(1)?, units: r.get(2)?, amount_cents: r.get(3)?, date: r.get(4)? }))
                    .map(|it| it.flatten().collect())
            })
            .e()?;
        let (mut units, mut cost, mut realized) = (0.0f64, 0i64, 0i64);
        for op in &ops {
            if op.ty == "compra" {
                units += op.units;
                cost += op.amount_cents;
            } else {
                if op.units > units + 1e-9 {
                    return err(format!("La venta del {} deja las unidades en negativo", op.date));
                }
                let avg = if units > 0.0 { cost as f64 / units } else { 0.0 };
                let out = (avg * op.units).round() as i64;
                realized += op.amount_cents - out;
                units -= op.units;
                cost -= out;
            }
        }
        units = (units * 1e9).round() / 1e9;
        Ok(OpsReplay { ops, units, cost_cents: cost.max(0), realized_gain_cents: realized })
    }

    fn sync_from_ops(&mut self, inv_id: i64) -> R<OpsReplay> {
        let r = self.replay_ops(inv_id)?;
        if r.units <= 0.0 {
            return err("Las operaciones dejan la posición a cero: para cerrarla, borra la inversión");
        }
        self.conn.execute("UPDATE investments SET units = ?, cost_cents = ? WHERE id = ?", params![r.units, r.cost_cents, inv_id]).e()?;
        Ok(r)
    }

    pub fn add_op(&mut self, inv_id: i64, ty: &str, units: f64, amount_cents: i64, date: &str) -> R<()> {
        if ty != "compra" && ty != "venta" {
            return err("El tipo debe ser compra o venta");
        }
        if !(units.is_finite() && units > 0.0) {
            return err("Unidades no válidas");
        }
        if amount_cents < 0 {
            return err("Importe no válido");
        }
        let date = if util::parse_date(date).is_some() { date.to_string() } else { util::today_str() };
        self.conn
            .execute(
                "INSERT INTO investment_ops (investment_id, type, units, amount_cents, date) VALUES (?, ?, ?, ?, ?)",
                params![inv_id, ty, units, amount_cents, date],
            )
            .e()?;
        let op_id = self.conn.last_insert_rowid();
        if let Err(e) = self.sync_from_ops(inv_id) {
            let _ = self.conn.execute("DELETE FROM investment_ops WHERE id = ?", [op_id]);
            return Err(e);
        }
        self.touch();
        Ok(())
    }

    pub fn delete_op(&mut self, op_id: i64) -> R<()> {
        let op: (i64, String, f64, i64, String, String) = self
            .conn
            .query_row(
                "SELECT investment_id, type, units, amount_cents, date, created_at FROM investment_ops WHERE id = ?",
                [op_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .map_err(|_| "Operación no encontrada".to_string())?;
        self.conn.execute("DELETE FROM investment_ops WHERE id = ?", [op_id]).e()?;
        if let Err(e) = self.sync_from_ops(op.0) {
            let _ = self.conn.execute(
                "INSERT INTO investment_ops (id, investment_id, type, units, amount_cents, date, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![op_id, op.0, op.1, op.2, op.3, op.4, op.5],
            );
            return Err(e);
        }
        self.touch();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_core;

    #[test]
    fn coste_medio() {
        let mut c = test_core();
        let id = c.save_investment(None, "ETF", "manual", "", 10.0, 100000, Some(110.0)).unwrap();
        c.add_op(id, "compra", 10.0, 120000, "2026-02-01").unwrap();
        c.add_op(id, "venta", 5.0, 70000, "2026-03-01").unwrap();
        let r = c.replay_ops(id).unwrap();
        assert!((r.units - 15.0).abs() < 1e-9);
        // Orden cronológico: compra de febrero (120 €/ud), venta de marzo (salen
        // 600 € a coste medio, ganancia realizada 100 €) y el lote inicial de hoy.
        assert_eq!(r.realized_gain_cents, 10000);
        assert_eq!(r.cost_cents, 160000);
        assert!(c.add_op(id, "venta", 100.0, 0, "2026-04-01").is_err());
        assert_eq!(c.portfolio().value_cents, 165000);
    }
}
