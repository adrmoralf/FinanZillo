//! Presupuestos del mes y gestión de categorías.

use egui::{Align, Layout, RichText, Ui, vec2};
use egui_phosphor::light as ic;

use crate::ui::charts;
use crate::ui::forms::{CategoryForm, Modal};
use crate::ui::theme::{self, p};
use crate::ui::widgets::{self as w, Kind};
use crate::ui::workspace::{Workspace, header};
use crate::util;

pub fn show(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    let privacy = ws.privacy;
    let mut m = ws.month.clone();
    if header(ui, "Presupuestos", Some(&mut m)) {
        ws.month = m;
    }
    let s = ws.cache.summary.clone();
    super::scroll(ui, "presupuestos", |ui| {
        // Presupuesto global.
        ui.label(w::caps("Total del mes"));
        ui.horizontal(|ui| {
            let st = &mut ws.views.ajustes;
            if st.global_budget.is_none() {
                st.global_budget = Some(if ws.core.global_budget_cents() > 0 { util::fmt_num(ws.core.global_budget_cents()).replace('.', "") } else { String::new() });
            }
            let v = st.global_budget.as_mut().unwrap();
            w::mono_field(ui, v, "sin límite", 130.0);
            if w::button(ui, None, "Guardar", Kind::Secondary).clicked() {
                let cents = if v.trim().is_empty() { Some(0) } else { util::eval_amount_cents(v) };
                match cents {
                    Some(c) => {
                        let r = ws.core.set_global_budget(c);
                        ws.report(r, Some("Presupuesto total guardado"));
                    }
                    None => ws.toast("El presupuesto no es válido", true),
                }
            }
            if s.global_budget_cents > 0 {
                let pct = util::pct(s.gastos_cents, s.global_budget_cents);
                ui.label(
                    RichText::new(format!("{} de {} · {pct}%", w::hide(util::eur(s.gastos_cents), privacy), w::hide(util::eur(s.global_budget_cents), privacy)))
                        .font(theme::mono(13.0))
                        .color(w::budget_color(pct, pal.ink2)),
                );
            }
        });
        if s.global_budget_cents > 0 {
            w::progress(ui, s.gastos_cents as f32 / s.global_budget_cents as f32, w::budget_color(util::pct(s.gastos_cents, s.global_budget_cents), pal.ink), 6.0);
        }
        ui.add_space(18.0);

        // Ritmo de gasto: ¿voy por encima o por debajo del mes pasado y del presupuesto?
        ui.add_space(18.0);
        ui.label(w::caps("Ritmo de gasto"));
        ui.add_space(2.0);
        let prev_name = util::MESES[(util::month_parts(&s.prev_month).1 - 1) as usize];
        charts::pace(ui, &s.daily, &s.prev_daily, s.global_budget_cents, s.elapsed_days as usize, prev_name, privacy);
        if s.is_current && s.prev_gastos_cents > 0 {
            let hasta: i64 = s.prev_daily.iter().take(s.elapsed_days as usize).sum();
            let diff = s.gastos_cents - hasta;
            let txt = if diff > 0 {
                format!("Vas {} por encima de {prev_name} a estas alturas del mes.", w::hide(util::eur(diff), privacy))
            } else {
                format!("Vas {} por debajo de {prev_name} a estas alturas del mes.", w::hide(util::eur(-diff), privacy))
            };
            ui.label(RichText::new(txt).color(if diff > 0 { pal.critical } else { pal.positive }));
        }
        ui.add_space(18.0);

        // Categorías de gasto con su presupuesto y lo gastado este mes.
        ui.horizontal(|ui| {
            ui.label(w::caps("Categorías de gasto"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if w::button(ui, Some(ic::PLUS), "Nueva categoría", Kind::Secondary).clicked() {
                    ws.modal = Modal::Category(CategoryForm {
                        id: None,
                        name: String::new(),
                        ty: 0,
                        icon: "📦".into(),
                        color: theme::CATEGORY_SWATCHES[0].into(),
                        budget: String::new(),
                        fixed: false,
                        error: String::new(),
                    });
                }
            });
        });
        w::hline(ui);
        let spent: std::collections::HashMap<Option<i64>, i64> = s.by_category.iter().map(|c| (c.id, c.total_cents)).collect();
        let cats = ws.cache.categories.clone();
        for ty in ["gasto", "ingreso"] {
            if ty == "ingreso" {
                ui.add_space(18.0);
                ui.label(w::caps("Categorías de ingreso"));
                w::hline(ui);
            }
            for c in cats.iter().filter(|c| c.ty == ty && !c.archived) {
                let budget = s.budgets.iter().find(|b| b.id == c.id).map(|b| b.budget_cents).unwrap_or(0);
                let sp = *spent.get(&Some(c.id)).unwrap_or(&0);
                let (resp, _) = w::row_frame(ui, false, |ui| {
                    ui.horizontal(|ui| {
                        w::color_strip(ui, theme::cat_color_for(c.id, &c.color), vec2(4.0, 30.0));
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&c.name).font(theme::medium(14.5)));
                                if c.fixed {
                                    ui.label(RichText::new("FIJO").font(theme::mono(10.0)).color(pal.muted));
                                }
                            });
                            ui.label(RichText::new(format!("{} movimientos en total", c.usage_count)).font(theme::mono(10.5)).color(pal.muted));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if budget > 0 {
                                let pct = util::pct(sp, budget);
                                ui.label(RichText::new(format!("{pct}%")).font(theme::mono_bold(13.0)).color(w::budget_color(pct, pal.ink2)));
                                ui.allocate_ui(vec2(160.0, 10.0), |ui| {
                                    w::progress(ui, sp as f32 / budget as f32, w::budget_color(pct, theme::cat_color_for(c.id, &c.color)), 5.0);
                                });
                                ui.label(
                                    RichText::new(format!("{} / {}", w::hide(util::fmt_num(sp), privacy), w::hide(util::fmt_num(budget), privacy)))
                                        .font(theme::mono(12.0))
                                        .color(pal.ink2),
                                );
                            } else if c.ty == "gasto" {
                                ui.label(RichText::new("sin límite").font(theme::mono(11.0)).color(pal.muted));
                                ui.add_space(118.0);
                                ui.label(RichText::new(w::hide(util::fmt_num(sp), privacy)).font(theme::mono(12.0)).color(pal.ink2));
                            } else if sp > 0 {
                                ui.label(RichText::new(w::hide(util::fmt_num(sp), privacy)).font(theme::mono(12.0)).color(pal.ink2));
                            }
                        });
                    });
                });
                if resp.clicked() {
                    ws.modal = Modal::CategoryDetail { id: c.id, month: ws.month.clone() };
                } else if resp.secondary_clicked() {
                    ws.modal = Modal::Category(CategoryForm {
                        id: Some(c.id),
                        name: c.name.clone(),
                        ty: if c.ty == "ingreso" { 1 } else { 0 },
                        icon: c.icon.clone(),
                        color: c.color.clone(),
                        budget: if c.budget_cents > 0 { util::fmt_num(c.budget_cents).replace('.', "") } else { String::new() },
                        fixed: c.fixed,
                        error: String::new(),
                    });
                }
            }
        }
        let archived: Vec<_> = cats.iter().filter(|c| c.archived).cloned().collect();
        if !archived.is_empty() {
            ui.add_space(18.0);
            ui.label(w::caps("Archivadas"));
            w::hline(ui);
            for c in archived {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} {}", c.icon, c.name)).color(pal.muted));
                    if w::button(ui, None, "Recuperar", Kind::Ghost).clicked() {
                        let r = ws.core.unarchive_category(c.id);
                        ws.report(r, None);
                    }
                });
            }
        }

        // Historial de 12 meses de la categoría con más gasto, como contexto.
        if let Some(top) = s.by_category.first().and_then(|c| c.id) {
            ui.add_space(18.0);
            let name = s.by_category[0].name.clone();
            ui.label(w::caps(&format!("{name} · últimos 12 meses")));
            let series = ws.core.category_monthly(top, &ws.month);
            let labels: Vec<String> = series.iter().map(|m| util::MESES_CORTOS[(util::month_parts(&m.month).1 - 1) as usize].to_string()).collect();
            let vals: Vec<i64> = series.iter().map(|m| m.gastos_cents).collect();
            charts::line(ui, &labels, &vals, 140.0, theme::cat_color_for(s.by_category[0].id, &s.by_category[0].color), privacy);
        }
    });
}
