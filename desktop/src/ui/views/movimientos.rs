//! Movimientos: filtros, totales y lista agrupada por día con columnas fijas.

use egui::{Align, Layout, RichText, Ui, vec2};
use egui_phosphor::light as ic;

use crate::ledger::{Transfer, Tx, TxFilter};
use crate::ui::forms::Modal;
use crate::ui::theme::{self, p};
use crate::ui::widgets::{self as w, Kind};
use crate::ui::workspace::Workspace;
use crate::util;

pub struct MovState {
    pub month: Option<String>,
    pub day: Option<String>,
    pub ty: usize,
    pub category: Option<i64>,
    pub account: String,
    pub q: String,
    pub min: String,
    pub max: String,
    pub focus_search: bool,
}

impl Default for MovState {
    fn default() -> Self {
        MovState {
            month: Some(util::current_month()),
            day: None,
            ty: 0,
            category: None,
            account: String::new(),
            q: String::new(),
            min: String::new(),
            max: String::new(),
            focus_search: false,
        }
    }
}

impl MovState {
    pub fn reset_to_month(&mut self, m: &str) {
        *self = MovState { month: Some(m.to_string()), ..Default::default() };
    }
    pub fn reset_to_day(&mut self, m: &str, day: &str) {
        *self = MovState { month: Some(m.to_string()), day: Some(day.to_string()), ..Default::default() };
    }
}

#[derive(PartialEq)]
pub enum RowAction {
    None,
    Edit,
    Delete,
    Repeat,
}

const W_DATE: f32 = 78.0;
const W_CAT: f32 = 176.0;
const W_ACC: f32 = 132.0;
const W_AMT: f32 = 116.0;
const W_ACT: f32 = 104.0;

/// Una fila de movimiento con columnas fijas. `with_date` muestra la fecha.
pub fn tx_row(ws: &Workspace, ui: &mut Ui, t: &Tx, with_date: bool) -> RowAction {
    let pal = p();
    let mut action = RowAction::None;
    let (resp, _) = w::row_frame(ui, false, |ui| {
        ui.horizontal(|ui| {
            ui.set_min_height(22.0);
            if with_date {
                ui.allocate_ui_with_layout(vec2(W_DATE, 22.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_width(ui.max_rect().width());
                    ui.label(RichText::new(util::date_label(&t.date)).font(theme::mono(11.5)).color(pal.muted));
                });
            }
            let fixed = W_CAT + W_ACC + W_AMT + W_ACT + if with_date { W_DATE } else { 0.0 } + 40.0;
            let note_w = (ui.available_width() - fixed + if with_date { W_DATE } else { 0.0 }).max(120.0);
            ui.allocate_ui_with_layout(vec2(note_w, 22.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_width(ui.max_rect().width());
                ui.set_max_width(note_w);
                let note = if t.note.is_empty() { t.category_name.clone() } else { t.note.clone() };
                ui.add(egui::Label::new(RichText::new(note).font(theme::ui_font(14.0)).color(pal.ink)).truncate());
                for tag in t.tags.split(',').filter(|s| !s.is_empty()).take(3) {
                    ui.label(RichText::new(format!("#{tag}")).font(theme::mono(10.5)).color(pal.muted));
                }
                if t.source == "recurrente" {
                    ui.label(RichText::new(ic::REPEAT).font(theme::icon(13.0)).color(pal.muted)).on_hover_text("Recurrente");
                } else if t.source == "intereses" {
                    ui.label(RichText::new(ic::PERCENT).font(theme::icon(13.0)).color(pal.positive)).on_hover_text("Intereses del mes, calculados con la TAE (edítalo si el banco pagó otra cifra)");
                }
            });
            ui.allocate_ui_with_layout(vec2(W_CAT, 22.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_width(ui.max_rect().width());
                w::color_strip(ui, theme::cat_color_for(t.category_id, &t.category_color), vec2(3.0, 16.0));
                ui.add(egui::Label::new(RichText::new(&t.category_name).font(theme::ui_font(13.0)).color(pal.ink2)).truncate());
            });
            ui.allocate_ui_with_layout(vec2(W_ACC, 22.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_width(ui.max_rect().width());
                ui.add(egui::Label::new(RichText::new(&t.account).font(theme::ui_font(13.0)).color(pal.muted)).truncate());
            });
            ui.allocate_ui_with_layout(vec2(W_AMT, 22.0), Layout::right_to_left(Align::Center), |ui| {
                ui.set_width(ui.max_rect().width());
                ui.label(w::amount_text(t.amount_cents, &t.ty, 14.5, ws.privacy));
            });
            ui.allocate_ui_with_layout(vec2(W_ACT, 22.0), Layout::right_to_left(Align::Center), |ui| {
                ui.set_width(ui.max_rect().width());
                ui.spacing_mut().item_spacing.x = 0.0;
                if w::icon_button_sm(ui, ic::TRASH, "Borrar").clicked() {
                    action = RowAction::Delete;
                }
                if w::icon_button_sm(ui, ic::COPY, "Repetir hoy").clicked() {
                    action = RowAction::Repeat;
                }
                if w::icon_button_sm(ui, ic::PENCIL_SIMPLE, "Editar").clicked() {
                    action = RowAction::Edit;
                }
            });
        });
    });
    if resp.clicked() && action == RowAction::None {
        action = RowAction::Edit;
    }
    action
}

fn transfer_row(ws: &mut Workspace, ui: &mut Ui, t: &Transfer) {
    let pal = p();
    let mut edit = false;
    let mut del = false;
    let (resp, _) = w::row_frame(ui, false, |ui| {
        ui.horizontal(|ui| {
            ui.set_min_height(22.0);
            let fixed = W_CAT + W_ACC + W_AMT + W_ACT + 40.0;
            let note_w = (ui.available_width() - fixed).max(120.0);
            ui.allocate_ui_with_layout(vec2(note_w, 22.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_width(ui.max_rect().width());
                ui.label(RichText::new(ic::ARROWS_LEFT_RIGHT).font(theme::icon(15.0)).color(pal.ink2));
                let txt = if t.note.is_empty() { "Transferencia".to_string() } else { t.note.clone() };
                ui.add(egui::Label::new(RichText::new(txt).color(pal.ink2)).truncate());
            });
            ui.allocate_ui_with_layout(vec2(W_CAT, 22.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_width(ui.max_rect().width());
                ui.add(egui::Label::new(RichText::new(format!("{}  →", t.from_account)).font(theme::ui_font(13.0)).color(pal.muted)).truncate());
            });
            ui.allocate_ui_with_layout(vec2(W_ACC, 22.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_width(ui.max_rect().width());
                ui.add(egui::Label::new(RichText::new(&t.to_account).font(theme::ui_font(13.0)).color(pal.muted)).truncate());
            });
            ui.allocate_ui_with_layout(vec2(W_AMT, 22.0), Layout::right_to_left(Align::Center), |ui| {
                ui.set_width(ui.max_rect().width());
                ui.label(w::amount_text(t.amount_cents, "transfer", 14.5, ws.privacy));
            });
            ui.allocate_ui_with_layout(vec2(W_ACT, 22.0), Layout::right_to_left(Align::Center), |ui| {
                ui.set_width(ui.max_rect().width());
                ui.spacing_mut().item_spacing.x = 0.0;
                if w::icon_button_sm(ui, ic::TRASH, "Borrar").clicked() {
                    del = true;
                }
                ui.add_space(26.0);
                if w::icon_button_sm(ui, ic::PENCIL_SIMPLE, "Editar").clicked() {
                    edit = true;
                }
            });
        });
    });
    if del {
        ws.modal = Modal::confirm_delete_transfer(t);
    } else if edit || resp.clicked() {
        ws.modal = Modal::edit_transfer(t);
    }
}

pub fn show(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Movimientos").font(theme::wide(22.0)));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if w::button(ui, Some(ic::ARROWS_LEFT_RIGHT), "Transferencia", Kind::Secondary).clicked() {
                ws.modal = Modal::new_transfer();
            }
            if w::button_ex(ui, Some(ic::PLUS), "Nuevo movimiento", Kind::Primary, true, true).clicked() {
                ws.modal = Modal::new_tx(&ws.core);
            }
        });
    });
    ui.add_space(10.0);

    // Filtros.
    let st = &mut ws.views.mov;
    ui.horizontal_wrapped(|ui| {
        let mut all = st.month.is_none();
        if let Some(m) = st.month.clone() {
            if w::icon_button(ui, ic::CARET_LEFT, "Mes anterior").clicked() {
                st.month = Some(util::add_months(&m, -1));
                st.day = None;
            }
            ui.label(RichText::new(util::cap(&util::month_label(&m))).font(theme::medium(14.0)));
            if w::icon_button(ui, ic::CARET_RIGHT, "Mes siguiente").clicked() {
                st.month = Some(util::add_months(&m, 1));
                st.day = None;
            }
        }
        if ui.checkbox(&mut all, "Todos los meses").changed() {
            st.month = if all { None } else { Some(util::current_month()) };
            st.day = None;
        }
        ui.add_space(8.0);
        w::segmented(ui, &mut st.ty, &["Todo", "Gastos", "Ingresos"]);
    });
    ui.add_space(6.0);
    let mut cat_opts: Vec<(Option<i64>, String)> = vec![(None, "Todas las categorías".into())];
    let ty_filter = match st.ty {
        1 => Some("gasto"),
        2 => Some("ingreso"),
        _ => None,
    };
    for c in ws.cache.categories.iter().filter(|c| ty_filter.is_none_or(|t| c.ty == t)) {
        cat_opts.push((Some(c.id), c.name.clone()));
    }
    let mut acc_opts: Vec<(String, String)> = vec![(String::new(), "Todas las cuentas".into())];
    acc_opts.extend(ws.cache.accounts.iter().map(|a| (a.name.clone(), a.name.clone())));
    let st = &mut ws.views.mov;
    ui.horizontal(|ui| {
        let r = w::text_field(ui, &mut st.q, "Buscar · #etiqueta · Ctrl+F", 300.0);
        if st.focus_search {
            r.request_focus();
            st.focus_search = false;
        }
        w::select(ui, "mov_cat", &mut st.category, &cat_opts, 200.0);
        w::select(ui, "mov_acc", &mut st.account, &acc_opts, 160.0);
        w::mono_field(ui, &mut st.min, "mín €", 80.0);
        w::mono_field(ui, &mut st.max, "máx €", 80.0);
        if let Some(d) = st.day.clone() {
            if w::button(ui, Some(ic::X), &format!("Solo el {}", util::date_label(&d)), Kind::Secondary).clicked() {
                st.day = None;
            }
        }
        let filtered = !st.q.is_empty() || st.category.is_some() || !st.account.is_empty() || !st.min.is_empty() || !st.max.is_empty() || st.ty != 0;
        if filtered && w::button(ui, None, "Limpiar", Kind::Ghost).clicked() {
            let m = st.month.clone();
            *st = MovState { month: m, ..Default::default() };
        }
    });
    ui.add_space(8.0);

    let st = &ws.views.mov;
    let f = TxFilter {
        month: if st.day.is_some() { None } else { st.month.clone() },
        from: st.day.clone(),
        to: st.day.clone(),
        ty: ty_filter.map(String::from),
        category_id: st.category,
        account: if st.account.is_empty() { None } else { Some(st.account.clone()) },
        q: if st.q.trim().is_empty() { None } else { Some(st.q.clone()) },
        amount_min: util::parse_amount_cents(&st.min),
        amount_max: util::parse_amount_cents(&st.max),
        limit: 2000,
    };
    let list = ws.core.transactions(&f);
    let only_tx = f.ty.is_some() || f.category_id.is_some() || f.q.is_some() || f.amount_min.is_some() || f.amount_max.is_some();
    let transfers: Vec<Transfer> = if only_tx {
        vec![]
    } else {
        ws.core
            .transfers(f.month.as_deref(), f.account.as_deref())
            .into_iter()
            .filter(|t| st.day.as_ref().is_none_or(|d| &t.date == d))
            .collect()
    };

    // Totales del filtro.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 22.0;
        let pr = ws.privacy;
        ui.label(RichText::new(format!("{} movimientos", list.total_count)).font(theme::mono(12.0)).color(pal.muted));
        ui.label(RichText::new(format!("ingresos {}", w::hide(util::eur(list.ingresos_cents), pr))).font(theme::mono(12.0)).color(pal.ingreso));
        ui.label(RichText::new(format!("gastos {}", w::hide(util::eur(list.gastos_cents), pr))).font(theme::mono(12.0)).color(pal.gasto));
        ui.label(RichText::new(format!("saldo {}", w::hide(util::eur_signed(list.ingresos_cents - list.gastos_cents), pr))).font(theme::mono(12.0)).color(pal.ink));
    });
    ui.add_space(4.0);
    w::hline(ui);

    // Lista agrupada por día.
    let mut days: Vec<String> = list.items.iter().map(|t| t.date.clone()).chain(transfers.iter().map(|t| t.date.clone())).collect();
    days.sort();
    days.dedup();
    days.reverse();
    let items = list.items.clone();
    super::scroll(ui, "mov_lista", |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        if days.is_empty() {
            w::empty_state(ui, ic::MAGNIFYING_GLASS, "No hay movimientos con estos filtros", "Prueba con otro mes o pulsa «Limpiar».");
        }
        for d in &days {
            let day_tx: Vec<&Tx> = items.iter().filter(|t| &t.date == d).collect();
            let net: i64 = day_tx.iter().map(|t| t.signed()).sum();
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(util::date_label(d).to_uppercase()).font(theme::mono(11.0)).color(pal.ink2).extra_letter_spacing(1.2));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Mismo borde derecho que la columna de importes de las filas.
                    ui.add_space(W_ACT + 10.0 + 2.0 * ui.spacing().item_spacing.x);
                    ui.label(RichText::new(w::hide(util::fmt_num(net), ws.privacy)).font(theme::mono(11.0)).color(pal.muted));
                });
            });
            for t in day_tx {
                match tx_row(ws, ui, t, false) {
                    RowAction::Edit => ws.modal = Modal::edit_tx(t),
                    RowAction::Delete => ws.modal = Modal::confirm_delete_tx(t),
                    RowAction::Repeat => {
                        let r = ws.core.create_tx(&crate::ledger::TxInput {
                            ty: t.ty.clone(),
                            amount_cents: t.amount_cents,
                            category_id: t.category_id,
                            note: t.note.clone(),
                            account: t.account.clone(),
                            date: util::today_str(),
                            tags: t.tags.clone(),
                            source: "escritorio".into(),
                        });
                        if let Some(id) = ws.report(r, None) {
                            ws.toast_undo("Repetido con fecha de hoy", id);
                        }
                    }
                    RowAction::None => {}
                }
            }
            for tr in transfers.iter().filter(|t| &t.date == d) {
                transfer_row(ws, ui, tr);
            }
        }
    });
}
