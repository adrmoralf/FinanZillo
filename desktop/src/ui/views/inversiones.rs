//! Inversiones: valor, ganancia, posiciones con su línea de 30 días.

use egui::{Align, Layout, RichText, Ui};
use egui_phosphor::light as ic;

use crate::ui::charts;
use crate::ui::forms::{InvestmentForm, Modal, OpsForm};
use crate::ui::theme::{self, p};
use crate::ui::widgets::{self as w, Kind};
use crate::ui::workspace::Workspace;
use crate::util;

pub fn show(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    let privacy = ws.privacy;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Inversiones").font(theme::wide(22.0)));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if w::button_ex(ui, Some(ic::PLUS), "Nueva inversión", Kind::Primary, true, true).clicked() {
                ws.modal = Modal::Investment(InvestmentForm {
                    id: None,
                    name: String::new(),
                    provider: 0,
                    symbol: String::new(),
                    units: String::new(),
                    cost: String::new(),
                    manual: String::new(),
                    query: String::new(),
                    hits: vec![],
                    searching: false,
                    error: String::new(),
                });
            }
            let label = if ws.prices_busy { "Actualizando…" } else { "Actualizar precios" };
            if w::button_ex(ui, Some(ic::ARROW_CLOCKWISE), label, Kind::Secondary, false, !ws.prices_busy).clicked() {
                ws.refresh_prices(true);
            }
        });
    });
    ui.add_space(10.0);
    let pf = ws.cache.portfolio.clone();
    super::scroll(ui, "inversiones", |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 40.0;
            ui.vertical(|ui| {
                ui.label(w::caps("Valor actual"));
                ui.label(RichText::new(w::hide(util::eur(pf.value_cents), privacy)).font(theme::display(44.0)));
            });
            ui.vertical(|ui| {
                ui.label(w::caps("Invertido"));
                ui.label(RichText::new(w::hide(util::eur(pf.cost_cents), privacy)).font(theme::mono_bold(20.0)));
            });
            ui.vertical(|ui| {
                ui.label(w::caps("Ganancia"));
                let g = pf.gain_cents();
                let pct = if pf.cost_cents > 0 { format!(" ({:+.1} %)", g as f64 / pf.cost_cents as f64 * 100.0).replace('.', ",") } else { String::new() };
                ui.label(RichText::new(w::hide(format!("{}{pct}", util::eur_signed(g)), privacy)).font(theme::mono_bold(20.0)).color(if g >= 0 { pal.positive } else { pal.critical }));
            });
        });
        ui.add_space(16.0);
        w::hline(ui);
        if pf.items.is_empty() {
            w::empty_state(ui, ic::CHART_LINE_UP, "Sin inversiones", "Añade un fondo, ETF o cripto: el precio se busca solo en Yahoo o CoinGecko.");
        }
        for inv in &pf.items {
            let mut ops = false;
            let (resp, _) = w::row_frame(ui, false, |ui| {
                ui.horizontal(|ui| {
                    // Columna del nombre acotada: la línea de precio va aparte.
                    let name_w = (ui.available_width() - 360.0).max(200.0);
                    ui.allocate_ui_with_layout(egui::vec2(name_w, 36.0), Layout::top_down(Align::LEFT), |ui| {
                        ui.set_width(name_w);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.add(egui::Label::new(RichText::new(&inv.name).font(theme::medium(15.0))).truncate());
                        let price = inv.price_eur.map(|p| format!("{:.2} €/ud", p).replace('.', ",")).unwrap_or_else(|| "sin precio".into());
                        let when = inv.price_fetched_at.as_deref().map(|t| format!(" · {}", &t[..16.min(t.len())])).unwrap_or_default();
                        let src = if inv.provider == "manual" { "manual".to_string() } else { inv.symbol.clone() };
                        ui.add(egui::Label::new(RichText::new(format!("{src} · {} ud · {price}{when}", (inv.units * 1e4).round() / 1e4)).font(theme::mono(11.0)).color(pal.muted)).truncate());
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if w::icon_button(ui, ic::NOTEPAD, "Compras y ventas").clicked() {
                            ops = true;
                        }
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.with_layout(Layout::top_down(Align::RIGHT), |ui| {
                                ui.label(RichText::new(w::hide(inv.value_cents.map(util::eur).unwrap_or("—".into()), privacy)).font(theme::mono_bold(15.0)));
                                if let Some(g) = inv.gain_cents() {
                                    ui.label(RichText::new(w::hide(util::eur_signed(g), privacy)).font(theme::mono(11.5)).color(if g >= 0 { pal.positive } else { pal.critical }));
                                }
                            });
                        });
                        let hist: Vec<f64> = inv.history.iter().map(|h| h.1).collect();
                        charts::spark(ui, &hist, 120.0, 28.0);
                    });
                });
            });
            if ops {
                ws.modal = Modal::Ops(OpsForm { inv_id: inv.id, name: inv.name.clone(), ty: 0, units: String::new(), amount: String::new(), date: util::today_str(), error: String::new() });
            } else if resp.clicked() {
                ws.modal = Modal::Investment(InvestmentForm {
                    id: Some(inv.id),
                    name: inv.name.clone(),
                    provider: match inv.provider.as_str() {
                        "coingecko" => 1,
                        "manual" => 2,
                        _ => 0,
                    },
                    symbol: inv.symbol.clone(),
                    units: inv.units.to_string().replace('.', ","),
                    cost: util::fmt_num(inv.cost_cents).replace('.', ""),
                    manual: inv.manual_price.map(|p| format!("{p:.2}").replace('.', ",")).unwrap_or_default(),
                    query: String::new(),
                    hits: vec![],
                    searching: false,
                    error: String::new(),
                });
            }
        }
        ui.add_space(10.0);
        ui.label(RichText::new("Los precios se actualizan al abrir y cada 15 minutos como mucho. Sin conexión se usa el último precio guardado.").color(pal.muted));
    });
}
