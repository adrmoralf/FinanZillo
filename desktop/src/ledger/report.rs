//! Informe mensual: datos, conclusiones y HTML autónomo (imprimible a PDF).

use std::collections::BTreeMap;

use super::analysis::{BudgetLine, CatTotal, Summary};
use super::{Core, Transfer, Tx, TxFilter};
use crate::util::{self, eur};

#[derive(Clone, Debug, Default)]
pub struct AccountFlow {
    pub account: String,
    pub ingresos_cents: i64,
    pub gastos_cents: i64,
    pub transfers_in_cents: i64,
    pub transfers_out_cents: i64,
}

impl AccountFlow {
    pub fn net(&self) -> i64 {
        self.ingresos_cents + self.transfers_in_cents - self.gastos_cents - self.transfers_out_cents
    }
}

#[derive(Clone, Debug)]
pub struct Highlight {
    /// "bien" | "mal" | "aviso" | "info"
    pub tone: &'static str,
    pub title: String,
    pub desc: String,
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct Report {
    pub summary: Summary,
    pub savings_rate: i64,
    pub top_expenses: Vec<Tx>,
    pub accounts: Vec<AccountFlow>,
    pub transfers: Vec<Transfer>,
    pub highlights: Vec<Highlight>,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn safe_color(c: &str) -> String {
    super::sanitize_color(c, "#898781")
}

impl Core {
    pub fn report(&self, month: &str) -> Report {
        let s = self.summary(month);
        let bal = s.balance();
        let savings_rate = if s.ingresos_cents > 0 {
            util::pct(bal, s.ingresos_cents)
        } else if s.gastos_cents == 0 {
            0
        } else {
            -100
        };
        let mut top = self.transactions(&TxFilter { month: Some(month.into()), ty: Some("gasto".into()), limit: 5000, ..Default::default() }).items;
        top.sort_by(|a, b| b.amount_cents.cmp(&a.amount_cents).then(b.id.cmp(&a.id)));
        top.truncate(10);

        let all = self.transactions(&TxFilter { month: Some(month.into()), limit: 5000, ..Default::default() }).items;
        let mut flows: BTreeMap<String, AccountFlow> = BTreeMap::new();
        for t in &all {
            let name = if t.account.trim().is_empty() { "Sin cuenta".to_string() } else { t.account.trim().to_string() };
            let f = flows.entry(name.clone()).or_insert_with(|| AccountFlow { account: name, ..Default::default() });
            if t.ty == "ingreso" { f.ingresos_cents += t.amount_cents } else { f.gastos_cents += t.amount_cents }
        }
        let transfers = self.transfers(Some(month), None);
        for t in &transfers {
            flows.entry(t.from_account.clone()).or_insert_with(|| AccountFlow { account: t.from_account.clone(), ..Default::default() }).transfers_out_cents +=
                t.amount_cents;
            flows.entry(t.to_account.clone()).or_insert_with(|| AccountFlow { account: t.to_account.clone(), ..Default::default() }).transfers_in_cents +=
                t.amount_cents;
        }
        let mut accounts: Vec<AccountFlow> = flows.into_values().collect();
        accounts.sort_by_key(|a| -(a.ingresos_cents + a.gastos_cents + a.transfers_in_cents + a.transfers_out_cents));

        let mut h = Vec::new();
        let prev = util::MESES[(util::month_parts(&s.prev_month).1 - 1) as usize];
        if s.ingresos_cents > 0 {
            if bal > 0 {
                h.push(Highlight { tone: "bien", title: "Superávit del mes".into(), desc: format!("Has ahorrado el {savings_rate}% de tus ingresos ({} a tu favor).", util::eur_signed(bal)) });
            } else if bal < 0 {
                h.push(Highlight { tone: "mal", title: "Déficit en el periodo".into(), desc: format!("El gasto superó a los ingresos en {}.", eur(-bal)) });
            } else {
                h.push(Highlight { tone: "info", title: "Mes equilibrado".into(), desc: "Gastos e ingresos han quedado exactamente iguales.".into() });
            }
        }
        if s.prev_gastos_cents > 0 && s.gastos_cents > 0 {
            let diff = s.gastos_cents - s.prev_gastos_cents;
            let p = util::pct(diff.abs(), s.prev_gastos_cents);
            if diff < 0 {
                h.push(Highlight { tone: "bien", title: "Gastas menos que el mes anterior".into(), desc: format!("Un {p}% menos ({} menos) que en {prev}.", eur(-diff)) });
            } else if p >= 5 {
                h.push(Highlight { tone: "aviso", title: "El gasto sube".into(), desc: format!("Un {p}% más ({}) que en {prev}.", util::eur_signed(diff)) });
            }
        }
        if let Some(c) = s.by_category.first() {
            h.push(Highlight {
                tone: "info",
                title: "Categoría principal".into(),
                desc: format!("{} supuso el {}% del gasto ({}).", c.name, util::pct(c.total_cents, s.gastos_cents), eur(c.total_cents)),
            });
        }
        if let Some(t) = top.first() {
            let name = if !t.note.is_empty() { t.note.clone() } else { t.category_name.clone() };
            h.push(Highlight { tone: "info", title: "Mayor gasto".into(), desc: format!("«{name}» por {} el día {}.", eur(t.amount_cents), &t.date[8..10]) });
        }
        if !s.budgets.is_empty() {
            let over: Vec<&BudgetLine> = s.budgets.iter().filter(|b| b.spent_cents > b.budget_cents).collect();
            if over.is_empty() {
                h.push(Highlight { tone: "bien", title: "Presupuestos cumplidos".into(), desc: format!("Has respetado los {} presupuestos.", s.budgets.len()) });
            } else {
                h.push(Highlight {
                    tone: "mal",
                    title: "Presupuestos superados".into(),
                    desc: format!("Te has pasado en: {}.", over.iter().map(|b| b.name.as_str()).collect::<Vec<_>>().join(", ")),
                });
            }
        }
        Report { summary: s, savings_rate, top_expenses: top, accounts, transfers, highlights: h }
    }
}

impl Report {
    /// Página HTML autónoma: sin archivos externos ni scripts. Tema claro para imprimir.
    pub fn html(&self) -> String {
        let s = &self.summary;
        let title = format!("Informe de {}", util::month_label(&s.month));
        let mut o = String::new();
        o.push_str(&format!(
            "<!doctype html><html lang=\"es\"><head><meta charset=\"utf-8\"><title>{}</title><style>{}</style></head><body><main>",
            esc(&title),
            CSS
        ));
        o.push_str(&format!(
            "<header><div class=\"cat\">FZ {} · INFORME MENSUAL</div><h1>{}</h1><p class=\"sub\">{} movimientos · generado el {}</p></header>",
            s.month.replace('-', "·"),
            esc(&util::cap(&util::month_label(&s.month))),
            s.count,
            util::date_label(&util::today_str())
        ));
        o.push_str("<section class=\"kpis\">");
        for (label, val, cls) in [
            ("Saldo del mes", util::eur_signed(s.balance()), if s.balance() >= 0 { "pos" } else { "neg" }),
            ("Ingresos", eur(s.ingresos_cents), "in"),
            ("Gastos", eur(s.gastos_cents), "out"),
            ("Tasa de ahorro", format!("{}%", self.savings_rate), ""),
        ] {
            o.push_str(&format!("<div class=\"kpi\"><span>{label}</span><b class=\"{cls}\">{}</b></div>", esc(&val)));
        }
        o.push_str("</section>");
        o.push_str(&format!(
            "<p class=\"facts\">Media diaria {} · {} días sin gastar de {} · fijos {} · variables {}</p>",
            eur(s.daily_avg()),
            s.days_without_spending(),
            s.elapsed_days,
            eur(s.fixed_cents),
            eur(s.gastos_cents - s.fixed_cents)
        ));
        if !self.highlights.is_empty() {
            o.push_str("<section><h2>Conclusiones</h2><ul class=\"hl\">");
            for h in &self.highlights {
                o.push_str(&format!("<li class=\"{}\"><b>{}</b> {}</li>", h.tone, esc(&h.title), esc(&h.desc)));
            }
            o.push_str("</ul></section>");
        }
        if !s.budgets.is_empty() {
            o.push_str("<section><h2>Presupuestos</h2><table><thead><tr><th>Categoría</th><th>Gastado</th><th>Presupuesto</th><th>%</th></tr></thead><tbody>");
            for b in &s.budgets {
                let cls = if b.pct() > 100 { "neg" } else if b.pct() >= 75 { "warn" } else { "" };
                o.push_str(&format!(
                    "<tr><td><i style=\"background:{}\"></i>{}</td><td>{}</td><td>{}</td><td class=\"{cls}\">{}%</td></tr>",
                    safe_color(&b.color),
                    esc(&b.name),
                    eur(b.spent_cents),
                    eur(b.budget_cents),
                    b.pct()
                ));
            }
            o.push_str("</tbody></table></section>");
        }
        o.push_str(&cat_table("Gasto por categoría", &s.by_category, s.gastos_cents));
        o.push_str(&cat_table("Ingresos por categoría", &s.by_category_ingreso, s.ingresos_cents));
        if !self.top_expenses.is_empty() {
            o.push_str("<section><h2>Mayores gastos</h2><table><thead><tr><th>Día</th><th>Concepto</th><th>Categoría</th><th>Cuenta</th><th>Importe</th></tr></thead><tbody>");
            for t in &self.top_expenses {
                o.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    &t.date[8..10],
                    esc(&t.note),
                    esc(&t.category_name),
                    esc(&t.account),
                    eur(t.amount_cents)
                ));
            }
            o.push_str("</tbody></table></section>");
        }
        if !self.accounts.is_empty() {
            o.push_str("<section><h2>Flujo por cuenta</h2><table><thead><tr><th>Cuenta</th><th>Entradas</th><th>Salidas</th><th>Neto</th></tr></thead><tbody>");
            for a in &self.accounts {
                o.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"{}\">{}</td></tr>",
                    esc(&a.account),
                    eur(a.ingresos_cents + a.transfers_in_cents),
                    eur(a.gastos_cents + a.transfers_out_cents),
                    if a.net() >= 0 { "pos" } else { "neg" },
                    util::eur_signed(a.net())
                ));
            }
            o.push_str("</tbody></table></section>");
        }
        o.push_str("<footer>FinanZillo · datos locales cifrados</footer></main></body></html>");
        o
    }

    /// Resumen en Markdown (para copiar al portapapeles).
    pub fn markdown(&self) -> String {
        let s = &self.summary;
        let mut o = format!("# Informe de {}\n\n", util::month_label(&s.month));
        o.push_str(&format!(
            "- Saldo del mes: **{}**\n- Ingresos: {}\n- Gastos: {}\n- Tasa de ahorro: {}%\n\n",
            util::eur_signed(s.balance()),
            eur(s.ingresos_cents),
            eur(s.gastos_cents),
            self.savings_rate
        ));
        if !self.highlights.is_empty() {
            o.push_str("## Conclusiones\n\n");
            for h in &self.highlights {
                o.push_str(&format!("- **{}**: {}\n", h.title, h.desc));
            }
            o.push('\n');
        }
        if !s.by_category.is_empty() {
            o.push_str("## Gasto por categoría\n\n| Categoría | Importe | % |\n|---|---:|---:|\n");
            for c in &s.by_category {
                o.push_str(&format!("| {} | {} | {}% |\n", c.name, eur(c.total_cents), util::pct(c.total_cents, s.gastos_cents)));
            }
        }
        o
    }
}

fn cat_table(title: &str, rows: &[CatTotal], total: i64) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut o = format!("<section><h2>{title}</h2><table><thead><tr><th>Categoría</th><th>Movs.</th><th>Importe</th><th>%</th><th></th></tr></thead><tbody>");
    for c in rows {
        let p = util::pct(c.total_cents, total);
        o.push_str(&format!(
            "<tr><td><i style=\"background:{col}\"></i>{}</td><td>{}</td><td>{}</td><td>{p}%</td><td class=\"bar\"><span style=\"width:{p}%;background:{col}\"></span></td></tr>",
            esc(&c.name),
            c.count,
            eur(c.total_cents),
            col = safe_color(&c.color)
        ));
    }
    o.push_str("</tbody></table></section>");
    o
}

const CSS: &str = r#"
@page{size:A4;margin:14mm}
*{box-sizing:border-box}
body{margin:0;background:#f4f4f2;color:#0b0b0c;font:14px/1.45 "Segoe UI",system-ui,sans-serif;-webkit-print-color-adjust:exact;print-color-adjust:exact}
main{max-width:860px;margin:0 auto;padding:40px 32px}
header{border-bottom:2px solid #0b0b0c;padding-bottom:18px;margin-bottom:22px}
.cat{font:600 11px/1 Consolas,monospace;letter-spacing:.18em}
h1{font-size:40px;letter-spacing:-.02em;margin:10px 0 4px}
.sub{color:#55555a;margin:0}
h2{font:600 11px/1 Consolas,monospace;letter-spacing:.18em;text-transform:uppercase;border-bottom:1px solid #0b0b0c;padding-bottom:6px;margin:28px 0 8px}
.kpis{display:grid;grid-template-columns:repeat(4,1fr);border:1px solid #0b0b0c}
.kpi{padding:14px;border-right:1px solid #0b0b0c}.kpi:last-child{border-right:0}
.kpi span{display:block;font:600 10px/1 Consolas,monospace;letter-spacing:.14em;text-transform:uppercase;color:#55555a}
.kpi b{display:block;font:700 22px/1.2 Consolas,monospace;margin-top:8px}
.pos{color:#0a7d4f}.neg{color:#c23a1c}.in{color:#2f5fd0}.out{color:#c23a1c}.warn{color:#9a6b00}
.facts{color:#55555a;font-size:13px}
table{width:100%;border-collapse:collapse;font-variant-numeric:tabular-nums}
th{text-align:left;font:600 10px/1 Consolas,monospace;letter-spacing:.12em;text-transform:uppercase;color:#55555a;padding:6px 4px;border-bottom:1px solid #0b0b0c}
td{padding:6px 4px;border-bottom:1px solid #d6d6d2}
td i{display:inline-block;width:10px;height:10px;margin-right:8px;vertical-align:-1px}
td.bar{width:28%}td.bar span{display:block;height:6px}
.hl{list-style:none;padding:0;margin:0}.hl li{padding:8px 0 8px 12px;border-bottom:1px solid #d6d6d2;border-left:4px solid #0b0b0c}
.hl li.bien{border-left-color:#0a7d4f}.hl li.mal{border-left-color:#c23a1c}.hl li.aviso{border-left-color:#d9a400}.hl li.info{border-left-color:#2f5fd0}
footer{margin-top:36px;font:11px Consolas,monospace;color:#55555a;letter-spacing:.1em}
section{break-inside:avoid}
"#;

#[cfg(test)]
mod tests {
    use super::super::{TxInput, test_core};

    #[test]
    fn html_escapa_datos() {
        let mut c = test_core();
        c.create_tx(&TxInput {
            ty: "gasto".into(),
            amount_cents: 1000,
            note: "<script>alert(1)</script>".into(),
            date: "2026-11-02".into(),
            ..Default::default()
        })
        .unwrap();
        let html = c.report("2026-11").html();
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
