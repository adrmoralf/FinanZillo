//! Movimientos recurrentes y suscripciones detectadas.

use egui::{Align, Layout, RichText, Ui};
use egui_phosphor::light as ic;

use crate::ledger::recurring::Recurring;
use crate::ui::forms::{Modal, RecurringForm};
use crate::ui::theme::{self, p};
use crate::ui::widgets::{self as w, Kind};
use crate::ui::workspace::Workspace;
use crate::util;

fn form_from(r: &Recurring) -> RecurringForm {
    RecurringForm {
        id: Some(r.id),
        active: r.active,
        kind: match (r.kind.as_str(), r.ty.as_str()) {
            ("traspaso", _) => 2,
            ("aportacion", _) => 3,
            (_, "ingreso") => 1,
            _ => 0,
        },
        amount: util::fmt_num(r.amount_cents).replace('.', ""),
        note: r.note.clone(),
        cat: r.category_id,
        account: r.account.clone(),
        from: r.from_account.clone(),
        to: r.to_account.clone(),
        inv: r.investment_id,
        day: r.day_of_month.to_string(),
        error: String::new(),
    }
}

pub fn show(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    let privacy = ws.privacy;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Recurrentes").font(theme::wide(22.0)));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if w::button_ex(ui, Some(ic::PLUS), "Nuevo recurrente", Kind::Primary, true, true).clicked() {
                ws.modal = Modal::Recurring(RecurringForm {
                    id: None,
                    active: true,
                    kind: 0,
                    amount: String::new(),
                    note: String::new(),
                    cat: None,
                    account: ws.core.default_account(),
                    from: ws.core.default_account(),
                    to: String::new(),
                    inv: None,
                    day: "1".into(),
                    error: String::new(),
                });
            }
        });
    });
    ui.label(RichText::new("Se apuntan solos cada mes, el día indicado. Si la app estaba cerrada, se ponen al día al abrirla.").color(pal.muted));
    ui.add_space(10.0);
    let list = ws.core.recurring_list();
    let sugg = ws.core.recurring_suggestions();
    super::scroll(ui, "recurrentes", |ui| {
        let monthly: i64 = list.iter().filter(|r| r.active && r.kind != "traspaso" && r.ty == "gasto").map(|r| r.amount_cents).sum();
        ui.label(w::caps("Gasto fijo recurrente al mes"));
        ui.label(RichText::new(w::hide(util::eur(monthly), privacy)).font(theme::display(34.0)));
        ui.add_space(12.0);
        w::hline(ui);
        if list.is_empty() {
            w::empty_state(ui, ic::REPEAT, "Sin recurrentes", "Añade la nómina, el alquiler o tus suscripciones para que se apunten solos.");
        }
        for r in &list {
            let (resp, toggle) = w::row_frame(ui, false, |ui| {
                let mut toggle = false;
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(w::caps("día"));
                        ui.label(RichText::new(format!("{:>2}", r.day_of_month)).font(theme::mono_bold(18.0)).color(if r.active { pal.ink } else { pal.muted }));
                    });
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let title = if r.note.is_empty() { r.category_name.clone() } else { r.note.clone() };
                        ui.label(RichText::new(title).font(theme::medium(14.5)).color(if r.active { pal.ink } else { pal.muted }));
                        let sub = match r.kind.as_str() {
                            "traspaso" => format!("Traspaso {} → {}", r.from_account, r.to_account),
                            "aportacion" => format!("Aportación a {}{}", r.investment_name, if r.from_account.is_empty() { String::new() } else { format!(" desde {}", r.from_account) }),
                            _ => format!("{} · {}", r.category_name, r.account),
                        };
                        let last = r.last_applied.as_deref().map(|m| format!(" · último: {}", util::month_label(m))).unwrap_or_default();
                        ui.label(RichText::new(format!("{sub}{last}{}", if r.active { "" } else { " · en pausa" })).font(theme::ui_font(12.0)).color(pal.muted));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if w::icon_button(ui, if r.active { ic::PAUSE } else { ic::PLAY }, if r.active { "Pausar" } else { "Reanudar" }).clicked() {
                            toggle = true;
                        }
                        ui.add_space(10.0);
                        let ty = if r.kind == "traspaso" { "transfer" } else { r.ty.as_str() };
                        ui.label(w::amount_text(r.amount_cents, ty, 15.0, privacy));
                    });
                });
                toggle
            });
            if toggle {
                let res = ws.core.set_recurring_active(r.id, !r.active);
                ws.report(res, None);
            } else if resp.clicked() {
                ws.modal = Modal::Recurring(form_from(r));
            }
        }
        if !sugg.is_empty() {
            ui.add_space(22.0);
            ui.label(w::caps("Parecen suscripciones"));
            ui.label(RichText::new("Pagos iguales que se repiten en 3 o más de los últimos 5 meses.").color(pal.muted));
            w::hline(ui);
            for s in sugg {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&s.note).font(theme::medium(14.0)));
                    ui.label(RichText::new(format!("{} · {} meses · día {}", s.category_name, s.months_seen, s.typical_day)).color(pal.muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if w::button(ui, Some(ic::PLUS), "Hacer recurrente", Kind::Secondary).clicked() {
                            ws.modal = Modal::Recurring(RecurringForm {
                                id: None,
                                active: true,
                                kind: 0,
                                amount: util::fmt_num(s.amount_cents).replace('.', ""),
                                note: s.note.clone(),
                                cat: s.category_id,
                                account: ws.core.default_account(),
                                from: String::new(),
                                to: String::new(),
                                inv: None,
                                day: s.typical_day.clamp(1, 31).to_string(),
                                error: String::new(),
                            });
                        }
                        ui.label(RichText::new(w::hide(util::eur(s.amount_cents), privacy)).font(theme::mono(13.5)));
                    });
                });
            }
        }
    });
}
