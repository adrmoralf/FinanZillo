//! Objetivos de ahorro con cuota mensual sugerida.

use egui::{Align, Layout, RichText, Ui};
use egui_phosphor::light as ic;

use crate::ui::forms::{GoalAddForm, GoalForm, Modal};
use crate::ui::theme::{self, p};
use crate::ui::widgets::{self as w, Kind};
use crate::ui::workspace::Workspace;
use crate::util;

pub fn show(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    let privacy = ws.privacy;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Objetivos").font(theme::wide(22.0)));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if w::button_ex(ui, Some(ic::PLUS), "Nuevo objetivo", Kind::Primary, true, true).clicked() {
                ws.modal = Modal::Goal(GoalForm { id: None, name: String::new(), icon: "🎯".into(), target: String::new(), deadline: String::new(), error: String::new() });
            }
        });
    });
    ui.label(RichText::new("Huchas para metas concretas. Lo que aportas aquí es un apartado: no mueve dinero de tus cuentas.").color(pal.muted));
    ui.add_space(12.0);
    let goals = ws.core.goals();
    super::scroll(ui, "objetivos", |ui| {
        if goals.is_empty() {
            w::empty_state(ui, ic::TARGET, "Sin objetivos", "Crea uno (un viaje, un colchón de seguridad…) y ve aportando.");
        }
        for g in goals {
            w::panel(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&g.icon).font(theme::ui_font(22.0)));
                    ui.label(RichText::new(&g.name).font(theme::medium(16.0)));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if w::icon_button(ui, ic::PENCIL_SIMPLE, "Editar").clicked() {
                            ws.modal = Modal::Goal(GoalForm {
                                id: Some(g.id),
                                name: g.name.clone(),
                                icon: g.icon.clone(),
                                target: util::fmt_num(g.target_cents).replace('.', ""),
                                deadline: g.deadline.clone().unwrap_or_default(),
                                error: String::new(),
                            });
                        }
                        if w::button(ui, None, "Retirar", Kind::Ghost).clicked() {
                            ws.modal = Modal::GoalAdd(GoalAddForm { id: g.id, name: g.name.clone(), amount: String::new(), withdraw: true, error: String::new() });
                        }
                        if w::button(ui, Some(ic::PLUS), "Aportar", Kind::Secondary).clicked() {
                            ws.modal = Modal::GoalAdd(GoalAddForm { id: g.id, name: g.name.clone(), amount: String::new(), withdraw: false, error: String::new() });
                        }
                    });
                });
                let pct = util::pct(g.saved_cents, g.target_cents);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(w::hide(util::eur(g.saved_cents), privacy)).font(theme::mono_bold(18.0)));
                    ui.label(RichText::new(format!("de {} · {pct}%", w::hide(util::eur(g.target_cents), privacy))).font(theme::mono(12.5)).color(pal.muted));
                });
                w::progress(ui, g.saved_cents as f32 / g.target_cents as f32, if pct >= 100 { pal.positive } else { pal.ink }, 6.0);
                if let Some(d) = g.deadline.as_deref().and_then(util::parse_date) {
                    let today = util::today();
                    let left = g.target_cents - g.saved_cents;
                    let months = ((d.signed_duration_since(today).num_days() as f64) / 30.44).ceil().max(1.0) as i64;
                    let txt = if left <= 0 {
                        "¡Conseguido!".to_string()
                    } else if d < today {
                        format!("La fecha límite ({}) ya pasó; faltan {}", util::date_label(&util::fmt_date(d)), w::hide(util::eur(left), privacy))
                    } else {
                        format!("Para llegar el {}: {} al mes durante {months} meses", util::date_label(&util::fmt_date(d)), w::hide(util::eur(left / months), privacy))
                    };
                    ui.label(RichText::new(txt).font(theme::ui_font(12.5)).color(pal.ink2));
                }
            });
            ui.add_space(10.0);
        }
    });
}
