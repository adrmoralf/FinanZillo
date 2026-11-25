//! Vista anual: totales con comparativa, tabla mes a mes y ranking.

use egui::{Align, Layout, RichText, Ui, vec2};
use egui_phosphor::light as ic;

use crate::ui::charts;
use crate::ui::theme::{self, p};
use crate::ui::widgets::{self as w};
use crate::ui::workspace::{View, Workspace};
use crate::util;

pub fn show(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    let privacy = ws.privacy;
    let year = *ws.views.anual_year.get_or_insert(util::month_parts(&ws.month).0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Año").font(theme::wide(22.0)));
        ui.add_space(12.0);
        if w::icon_button(ui, ic::CARET_LEFT, "Año anterior").clicked() {
            ws.views.anual_year = Some(year - 1);
        }
        ui.label(RichText::new(year.to_string()).font(theme::medium(16.0)));
        if w::icon_button(ui, ic::CARET_RIGHT, "Año siguiente").clicked() {
            ws.views.anual_year = Some(year + 1);
        }
    });
    ui.add_space(10.0);
    let y = ws.core.year_view(year);
    super::scroll(ui, "anual", |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 46.0;
            for (label, cur, prev, col, up_good) in [
                ("Ingresos", y.ingresos_cents, y.prev_ingresos_cents, pal.ingreso, true),
                ("Gastos", y.gastos_cents, y.prev_gastos_cents, pal.gasto, false),
                ("Ahorro", y.ingresos_cents - y.gastos_cents, y.prev_ingresos_cents - y.prev_gastos_cents, pal.ink, true),
            ] {
                ui.vertical(|ui| {
                    ui.label(w::caps(label));
                    ui.label(RichText::new(w::hide(util::eur(cur), privacy)).font(theme::display(34.0)).color(col));
                    if prev != 0 {
                        let pct = util::pct(cur - prev, prev.abs());
                        let good = (pct > 0) == up_good;
                        ui.label(
                            RichText::new(format!("{}{}% vs {}", if pct >= 0 { "+" } else { "−" }, pct.abs(), year - 1))
                                .font(theme::mono(11.5))
                                .color(if good { pal.positive } else { pal.critical }),
                        );
                    }
                });
            }
        });
        ui.add_space(14.0);
        let dec = util::month_str(year, 12);
        if let Some(m) = charts::bars12(ui, &y.months, &dec, 160.0, privacy) {
            ws.month = m;
            ws.view = View::Mes;
        }
        ui.add_space(16.0);
        ui.horizontal_top(|ui| {
            let half = (ui.available_width() - 32.0) / 2.0;
            ui.allocate_ui_with_layout(vec2(half, 10.0), Layout::top_down(Align::LEFT), |ui| {
                ui.label(w::caps("Mes a mes"));
                w::hline(ui);
                egui::Grid::new("anual_tabla").num_columns(4).spacing([18.0, 6.0]).min_col_width(70.0).show(ui, |ui| {
                    for h in ["Mes", "Ingresos", "Gastos", "Saldo"] {
                        ui.label(w::caps(h));
                    }
                    ui.end_row();
                    for m in &y.months {
                        let empty = m.ingresos_cents == 0 && m.gastos_cents == 0;
                        let c = if empty { pal.muted } else { pal.ink };
                        let (_, mm) = util::month_parts(&m.month);
                        if ui.link(RichText::new(util::cap(util::MESES[(mm - 1) as usize])).color(c)).clicked() {
                            ws.month = m.month.clone();
                            ws.view = View::Mes;
                        }
                        ui.label(RichText::new(w::hide(util::fmt_num(m.ingresos_cents), privacy)).font(theme::mono(13.0)).color(c));
                        ui.label(RichText::new(w::hide(util::fmt_num(m.gastos_cents), privacy)).font(theme::mono(13.0)).color(c));
                        let b = m.ingresos_cents - m.gastos_cents;
                        ui.label(RichText::new(w::hide(util::fmt_num(b), privacy)).font(theme::mono(13.0)).color(if empty { pal.muted } else if b < 0 { pal.critical } else { pal.ink }));
                        ui.end_row();
                    }
                    ui.label(RichText::new("Total").font(theme::medium(14.0)));
                    ui.label(RichText::new(w::hide(util::fmt_num(y.ingresos_cents), privacy)).font(theme::mono_bold(13.0)));
                    ui.label(RichText::new(w::hide(util::fmt_num(y.gastos_cents), privacy)).font(theme::mono_bold(13.0)));
                    ui.label(RichText::new(w::hide(util::fmt_num(y.ingresos_cents - y.gastos_cents), privacy)).font(theme::mono_bold(13.0)));
                    ui.end_row();
                });
            });
            ui.add_space(32.0);
            ui.allocate_ui_with_layout(vec2(half, 10.0), Layout::top_down(Align::LEFT), |ui| {
                ui.label(w::caps("En qué se fue el año"));
                w::hline(ui);
                let max = y.by_category.first().map(|c| c.total_cents).unwrap_or(1).max(1);
                for c in &y.by_category {
                    let r = ui.horizontal(|ui| {
                        ui.label(RichText::new(&c.name).font(theme::ui_font(13.5)));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(format!("{}%", util::pct(c.total_cents, y.gastos_cents))).font(theme::mono(11.5)).color(pal.muted));
                            ui.label(RichText::new(w::hide(util::fmt_num(c.total_cents), privacy)).font(theme::mono(13.0)));
                        });
                    });
                    w::progress(ui, c.total_cents as f32 / max as f32, theme::cat_color_for(c.id, &c.color), 4.0);
                    if let Some(id) = c.id {
                        let click = ui.interact(r.response.rect, ui.id().with(("cat_anual", id)), egui::Sense::click());
                        if click.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Ver la ficha de la categoría").clicked() {
                            let m = if year == util::month_parts(&util::current_month()).0 { util::current_month() } else { util::month_str(year, 12) };
                            ws.modal = crate::ui::forms::Modal::CategoryDetail { id, month: m };
                        }
                    }
                    ui.add_space(4.0);
                }
                if y.by_category.is_empty() {
                    ui.label(RichText::new("Sin gastos este año.").color(pal.muted));
                }
            });
        });
    });
}
