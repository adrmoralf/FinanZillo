//! Exportar e importar CSV (mismo formato que la app web).

use super::{Core, R, TxFilter, TxInput};
use crate::util;

fn escape(v: &str) -> String {
    if v.contains(['"', ';', '\n']) { format!("\"{}\"", v.replace('"', "\"\"")) } else { v.to_string() }
}

fn amount(c: i64) -> String {
    format!("{},{:02}", c / 100, (c % 100).abs())
}

/// Parser CSV mínimo con comillas; detecta ';' o ',' como separador.
pub fn parse(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().next().unwrap_or("");
    let sep = if first.matches(';').count() >= first.matches(',').count() { ';' } else { ',' };
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_q = false;
    let mut it = text.chars().peekable();
    while let Some(ch) = it.next() {
        if in_q {
            if ch == '"' && it.peek() == Some(&'"') {
                field.push('"');
                it.next();
            } else if ch == '"' {
                in_q = false;
            } else {
                field.push(ch);
            }
        } else if ch == '"' {
            in_q = true;
        } else if ch == sep {
            row.push(std::mem::take(&mut field));
        } else if ch == '\n' || ch == '\r' {
            if ch == '\r' && it.peek() == Some(&'\n') {
                it.next();
            }
            row.push(std::mem::take(&mut field));
            if row.iter().any(|c| !c.trim().is_empty()) {
                rows.push(std::mem::take(&mut row));
            }
            row.clear();
        } else {
            field.push(ch);
        }
    }
    row.push(field);
    if row.iter().any(|c| !c.trim().is_empty()) {
        rows.push(row);
    }
    rows
}

/// "2026-11-24", "24/11/2026", "24-11-2026", "24.11.2026" → "2026-11-24"
pub fn parse_date(s: &str) -> Option<String> {
    let s = s.trim().split(' ').next().unwrap_or("");
    if util::parse_date(s).is_some() {
        return Some(s.to_string());
    }
    let parts: Vec<&str> = s.split(['/', '-', '.']).collect();
    if parts.len() == 3 && parts[2].len() == 4 {
        let d = format!("{}-{:0>2}-{:0>2}", parts[2], parts[1], parts[0]);
        if util::parse_date(&d).is_some() {
            return Some(d);
        }
    }
    None
}

pub fn guess_column(headers: &[String], candidates: &[&str]) -> Option<usize> {
    let lower: Vec<String> = headers.iter().map(|h| h.to_lowercase()).collect();
    candidates.iter().find_map(|c| lower.iter().position(|h| h.contains(c)))
}

#[derive(Clone, Debug, Default)]
pub struct Mapping {
    pub fecha: Option<usize>,
    pub importe: Option<usize>,
    pub nota: Option<usize>,
    pub categoria: Option<usize>,
    pub cuenta: Option<usize>,
    /// "signo" | "gasto" | "ingreso"
    pub mode: String,
}

impl Mapping {
    pub fn guess(headers: &[String]) -> Self {
        Mapping {
            fecha: guess_column(headers, &["fecha", "date", "día", "dia"]),
            importe: guess_column(headers, &["importe", "amount", "cantidad", "monto", "valor"]),
            nota: guess_column(headers, &["nota", "descrip", "concepto", "note"]),
            categoria: guess_column(headers, &["categor", "category"]),
            cuenta: guess_column(headers, &["cuenta", "account", "banco"]),
            mode: "signo".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CsvItem {
    pub ty: String,
    pub amount_cents: i64,
    pub date: String,
    pub note: String,
    pub category: String,
    pub account: String,
}

/// Convierte filas en movimientos; devuelve (válidos, nº de filas descartadas).
pub fn map_rows(rows: &[Vec<String>], m: &Mapping) -> (Vec<CsvItem>, usize) {
    let get = |row: &Vec<String>, i: Option<usize>| -> String { i.and_then(|i| row.get(i)).map(|s| s.trim().to_string()).unwrap_or_default() };
    let mut items = Vec::new();
    let mut skipped = 0;
    for row in rows {
        let date = parse_date(&get(row, m.fecha));
        let raw = get(row, m.importe);
        let neg = raw.starts_with('-');
        let cents = util::parse_amount_cents(raw.trim_start_matches(['-', '+'])).filter(|c| *c > 0);
        match (date, cents) {
            (Some(date), Some(cents)) => items.push(CsvItem {
                ty: match m.mode.as_str() {
                    "gasto" => "gasto".into(),
                    "ingreso" => "ingreso".into(),
                    _ => if neg { "gasto".into() } else { "ingreso".into() },
                },
                amount_cents: cents,
                date,
                note: get(row, m.nota),
                category: get(row, m.categoria),
                account: get(row, m.cuenta),
            }),
            _ => skipped += 1,
        }
    }
    (items, skipped)
}

impl Core {
    pub fn export_csv(&self) -> String {
        let mut lines = vec!["fecha;tipo;importe;categoria;nota;cuenta;origen".to_string()];
        let mut txs = self.transactions(&TxFilter { limit: 5000, ..Default::default() }).items;
        txs.reverse();
        for t in txs {
            lines.push(
                [t.date, t.ty, amount(t.amount_cents), t.category_name, t.note, t.account, t.source]
                    .iter()
                    .map(|s| escape(s))
                    .collect::<Vec<_>>()
                    .join(";"),
            );
        }
        let mut trs = self.transfers(None, None);
        trs.reverse();
        for t in trs {
            lines.push(
                [
                    t.date,
                    "transferencia".into(),
                    amount(t.amount_cents),
                    String::new(),
                    t.note,
                    format!("{} → {}", t.from_account, t.to_account),
                    "escritorio".into(),
                ]
                .iter()
                .map(|s| escape(s))
                .collect::<Vec<_>>()
                .join(";"),
            );
        }
        format!("\u{feff}{}", lines.join("\n"))
    }

    /// Importa movimientos saltándose duplicados. Devuelve (creados, duplicados).
    pub fn import_items(&mut self, items: &[CsvItem]) -> R<(usize, usize)> {
        self.conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;
        let (mut created, mut dups) = (0, 0);
        let result: R<()> = (|| {
            for it in items {
            let input = TxInput {
                ty: it.ty.clone(),
                amount_cents: it.amount_cents,
                category_id: self.resolve_category(&it.category, &it.ty),
                note: it.note.clone(),
                account: it.account.clone(),
                date: it.date.clone(),
                tags: String::new(),
                source: "csv".into(),
            };
            if self.is_duplicate(&input) {
                dups += 1;
                continue;
            }
            self.create_tx(&input)?;
            created += 1;
            }
            Ok(())
        })();
        match result {
            Ok(()) => self.conn.execute_batch("COMMIT").map_err(|e| e.to_string())?,
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                return Err(e);
            }
        }
        self.touch();
        Ok((created, dups))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_y_mapea() {
        let rows = parse("\u{feff}Fecha;Concepto;Importe\n24/11/2026;\"Supermercado; frutas\";-42,18\n21/11/2026;Nómina;1603\nmal;x;y\n");
        assert_eq!(rows.len(), 4);
        let headers = rows[0].clone();
        let m = Mapping::guess(&headers);
        let (items, skipped) = map_rows(&rows[1..], &m);
        assert_eq!(skipped, 1);
        assert_eq!(items[0].ty, "gasto");
        assert_eq!(items[0].amount_cents, 4218);
        assert_eq!(items[0].note, "Supermercado; frutas");
        assert_eq!(items[1].ty, "ingreso");
        assert_eq!(items[1].date, "2026-11-21");
    }

    #[test]
    fn importa_sin_duplicar() {
        let mut c = super::super::test_core();
        let it = CsvItem {
            ty: "gasto".into(),
            amount_cents: 500,
            date: "2026-11-01".into(),
            note: "x".into(),
            category: "Ocio".into(),
            account: String::new(),
        };
        assert_eq!(c.import_items(&[it.clone(), it.clone()]).unwrap(), (1, 1));
        assert!(c.export_csv().contains("2026-11-01;gasto;5,00;Ocio;x;;csv"));
    }
}
