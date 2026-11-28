//! Diálogos de edición. Un único diálogo abierto a la vez (`Workspace::modal`).

use egui::{Align, Key, Layout, RichText, Ui, vec2};
use egui_phosphor::light as ic;

use super::theme::{self, p};
use super::widgets::{self as w, Kind};
use super::workspace::Workspace;
use crate::ledger::accounts::Account;
use crate::ledger::recurring::Recurring;
use crate::ledger::{Core, Transfer, Tx, TxInput, csv};
use crate::util;

pub struct TxForm {
    pub id: Option<i64>,
    /// 0 gasto · 1 ingreso · 2 transferencia
    pub kind: usize,
    pub amount: String,
    pub note: String,
    pub cat: Option<i64>,
    pub account: String,
    pub new_account: String,
    pub date: String,
    pub tags: String,
    pub from: String,
    pub to: String,
    pub transfer_id: Option<i64>,
    pub as_template: bool,
    pub error: String,
    pub focus: bool,
    /// La categoría la propuso el concepto (se puede seguir cambiando sola).
    pub cat_guessed: bool,
}

pub struct AccountForm {
    pub original: Option<String>,
    pub name: String,
    pub opening: String,
    pub date: String,
    pub apy: String,
    pub rename_to: String,
    pub reconcile: String,
    pub current_cents: i64,
    pub error: String,
}

pub struct CategoryForm {
    pub id: Option<i64>,
    pub name: String,
    pub ty: usize,
    pub icon: String,
    pub color: String,
    pub budget: String,
    pub fixed: bool,
    pub error: String,
}

pub struct RecurringForm {
    pub id: Option<i64>,
    pub active: bool,
    /// 0 gasto · 1 ingreso · 2 traspaso · 3 aportación
    pub kind: usize,
    pub amount: String,
    pub note: String,
    pub cat: Option<i64>,
    pub account: String,
    pub from: String,
    pub to: String,
    pub inv: Option<i64>,
    pub day: String,
    pub error: String,
}

pub struct InvestmentForm {
    pub id: Option<i64>,
    pub name: String,
    pub provider: usize,
    pub symbol: String,
    pub units: String,
    pub cost: String,
    pub manual: String,
    pub query: String,
    pub hits: Vec<crate::ledger::investments::SymbolHit>,
    pub searching: bool,
    pub error: String,
}

pub struct OpsForm {
    pub inv_id: i64,
    pub name: String,
    pub ty: usize,
    pub units: String,
    pub amount: String,
    pub date: String,
    pub error: String,
}

pub struct GoalForm {
    pub id: Option<i64>,
    pub name: String,
    pub icon: String,
    pub target: String,
    pub deadline: String,
    pub error: String,
}

pub struct GoalAddForm {
    pub id: i64,
    pub name: String,
    pub amount: String,
    pub withdraw: bool,
    pub error: String,
}

pub struct TemplateForm {
    pub name: String,
    pub amount: String,
    pub cat: Option<i64>,
    pub account: String,
    pub error: String,
}

pub struct PasswordForm {
    pub current: String,
    pub new1: String,
    pub new2: String,
    pub error: String,
}

pub struct CsvForm {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub map: csv::Mapping,
    pub error: String,
}

pub enum Confirm {
    DeleteTx(i64),
    DeleteTransfer(i64),
    DeleteAccount(String),
    DeleteCategory(i64),
    DeleteRecurring(i64),
    DeleteInvestment(i64),
    DeleteGoal(i64),
    ReimportWeb(std::path::PathBuf),
}

pub enum Modal {
    None,
    Tx(TxForm),
    Account(AccountForm),
    Category(CategoryForm),
    Recurring(RecurringForm),
    Investment(InvestmentForm),
    Ops(OpsForm),
    Goal(GoalForm),
    GoalAdd(GoalAddForm),
    Template(TemplateForm),
    Password(PasswordForm),
    Csv(CsvForm),
    /// Ficha de una categoría: serie de 12 meses y movimientos del mes.
    CategoryDetail { id: i64, month: String },
    Confirm { title: String, body: String, button: String, action: Confirm },
}

fn amount_str(c: i64) -> String {
    util::fmt_num(c).replace('.', "")
}

impl Modal {
    pub fn new_tx(core: &Core) -> Modal {
        Modal::Tx(TxForm {
            id: None,
            kind: 0,
            amount: String::new(),
            note: String::new(),
            cat: None,
            account: core.default_account(),
            new_account: String::new(),
            date: util::today_str(),
            tags: String::new(),
            from: core.default_account(),
            to: String::new(),
            transfer_id: None,
            as_template: false,
            error: String::new(),
            focus: true,
            cat_guessed: false,
        })
    }
    pub fn new_transfer() -> Modal {
        Modal::Tx(TxForm {
            id: None,
            kind: 2,
            amount: String::new(),
            note: String::new(),
            cat: None,
            account: String::new(),
            new_account: String::new(),
            date: util::today_str(),
            tags: String::new(),
            from: String::new(),
            to: String::new(),
            transfer_id: None,
            as_template: false,
            error: String::new(),
            focus: true,
            cat_guessed: false,
        })
    }
    pub fn edit_tx(t: &Tx) -> Modal {
        Modal::Tx(TxForm {
            id: Some(t.id),
            kind: if t.ty == "ingreso" { 1 } else { 0 },
            amount: amount_str(t.amount_cents),
            note: t.note.clone(),
            cat: t.category_id,
            account: t.account.clone(),
            new_account: String::new(),
            date: t.date.clone(),
            tags: t.tags.replace(',', ", "),
            from: t.account.clone(),
            to: String::new(),
            transfer_id: None,
            as_template: false,
            error: String::new(),
            focus: true,
            cat_guessed: false,
        })
    }
    pub fn edit_transfer(t: &Transfer) -> Modal {
        Modal::Tx(TxForm {
            id: None,
            kind: 2,
            amount: amount_str(t.amount_cents),
            note: t.note.clone(),
            cat: None,
            account: String::new(),
            new_account: String::new(),
            date: t.date.clone(),
            tags: String::new(),
            from: t.from_account.clone(),
            to: t.to_account.clone(),
            transfer_id: Some(t.id),
            as_template: false,
            error: String::new(),
            focus: true,
            cat_guessed: false,
        })
    }
    pub fn confirm_delete_tx(t: &Tx) -> Modal {
        Modal::Confirm {
            title: "¿Borrar este movimiento?".into(),
            body: format!(
                "{} · {} · {}",
                if t.note.is_empty() { &t.category_name } else { &t.note },
                util::eur(t.amount_cents),
                util::date_label(&t.date)
            ),
            button: "Borrar".into(),
            action: Confirm::DeleteTx(t.id),
        }
    }
    pub fn confirm_delete_transfer(t: &Transfer) -> Modal {
        Modal::Confirm {
            title: "¿Borrar esta transferencia?".into(),
            body: format!("{} → {} · {}", t.from_account, t.to_account, util::eur(t.amount_cents)),
            button: "Borrar".into(),
            action: Confirm::DeleteTransfer(t.id),
        }
    }
    pub fn new_account() -> Modal {
        Modal::Account(AccountForm {
            original: None,
            name: String::new(),
            opening: String::new(),
            date: util::today_str(),
            apy: String::new(),
            rename_to: String::new(),
            reconcile: String::new(),
            current_cents: 0,
            error: String::new(),
        })
    }
    pub fn edit_account(a: &Account) -> Modal {
        Modal::Account(AccountForm {
            original: Some(a.name.clone()),
            name: a.name.clone(),
            opening: amount_str(a.opening_cents),
            date: if a.opening_date.starts_with("1970") { util::today_str() } else { a.opening_date.clone() },
            apy: if a.apy > 0.0 { util::fmt_apy(a.apy) } else { String::new() },
            rename_to: a.name.clone(),
            reconcile: String::new(),
            current_cents: a.current_cents,
            error: String::new(),
        })
    }
    pub fn new_template(core: &Core) -> Modal {
        Modal::Template(TemplateForm { name: String::new(), amount: String::new(), cat: None, account: core.default_account(), error: String::new() })
    }
}

/// Fecha: acepta AAAA-MM-DD o DD/MM/AAAA, con atajos Hoy/Ayer.
fn date_field(ui: &mut Ui, v: &mut String) {
    ui.horizontal(|ui| {
        w::mono_field(ui, v, "AAAA-MM-DD", 120.0);
        if w::button(ui, None, "Hoy", Kind::Ghost).clicked() {
            *v = util::today_str();
        }
        if w::button(ui, None, "Ayer", Kind::Ghost).clicked() {
            *v = util::fmt_date(util::today() - chrono::Duration::days(1));
        }
    });
}

fn norm_date(v: &str) -> Option<String> {
    csv::parse_date(v)
}

fn field(ui: &mut Ui, label: &str) {
    ui.add_space(6.0);
    w::label_caps(ui, label);
}

fn account_picker(ui: &mut Ui, id: &str, current: &mut String, accounts: &[String], allow_new: bool, new_name: &mut String) {
    let mut opts: Vec<(String, String)> = accounts.iter().map(|a| (a.clone(), a.clone())).collect();
    if !current.is_empty() && !accounts.contains(current) {
        opts.push((current.clone(), current.clone()));
    }
    opts.insert(0, (String::new(), "(sin cuenta)".into()));
    if allow_new {
        opts.push(("\u{1}nueva".into(), "Otra cuenta…".into()));
    }
    ui.horizontal(|ui| {
        w::select(ui, id, current, &opts, 220.0);
        if current == "\u{1}nueva" {
            w::text_field(ui, new_name, "Nombre de la cuenta nueva", 200.0);
        }
    });
}

/// Rótulo de campo con icono: dice de un vistazo qué va en cada sitio.
fn form_label(ui: &mut Ui, icon: &str, label: &str) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.spacing_mut().interact_size.y = 14.0;
        ui.label(RichText::new(icon).font(theme::icon(14.0)).color(p().ink2));
        w::label_caps(ui, label);
    });
    ui.add_space(2.0);
}

fn account_picker_w(ui: &mut Ui, id: &str, current: &mut String, accounts: &[String], allow_new: bool, new_name: &mut String, width: f32) {
    let mut opts: Vec<(String, String)> = accounts.iter().map(|a| (a.clone(), a.clone())).collect();
    if !current.is_empty() && !accounts.contains(current) {
        opts.push((current.clone(), current.clone()));
    }
    opts.insert(0, (String::new(), "(sin cuenta)".into()));
    if allow_new {
        opts.push(("\u{1}nueva".into(), "Otra cuenta…".into()));
    }
    w::select(ui, id, current, &opts, width - 8.0);
    if current == "\u{1}nueva" {
        ui.add_space(4.0);
        w::text_field(ui, new_name, "Nombre de la cuenta nueva", width);
    }
}

/// Pastilla de categoría con su color; la elegida va en tinta.
fn cat_chip(ui: &mut Ui, name: &str, color: egui::Color32, selected: bool) -> egui::Response {
    let pal = p();
    let font = theme::ui_font(13.0);
    let g = ui.painter().layout_no_wrap(name.to_string(), font.clone(), pal.ink);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 30.0, 28.0), egui::Sense::click());
    let (fill, stroke, text) = if selected {
        (pal.ink, pal.ink, pal.bg)
    } else if resp.hovered() {
        (pal.surface2, pal.line_strong, pal.ink)
    } else {
        (egui::Color32::TRANSPARENT, pal.line_strong, pal.ink2)
    };
    ui.painter().rect(rect, w::R2, fill, egui::Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
    let sw = egui::Rect::from_center_size(egui::pos2(rect.left() + 11.0, rect.center().y), vec2(8.0, 8.0));
    ui.painter().rect_filled(sw, egui::CornerRadius::ZERO, color);
    ui.painter().text(egui::pos2(rect.left() + 21.0, rect.center().y), egui::Align2::LEFT_CENTER, name, font, text);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn resolve_account(sel: &str, new_name: &str) -> String {
    if sel == "\u{1}nueva" { new_name.trim().to_string() } else { sel.to_string() }
}

fn footer(ui: &mut Ui, error: &str, save_label: &str, danger: Option<&str>) -> (bool, bool, bool) {
    let pal = p();
    ui.add_space(12.0);
    if !error.is_empty() {
        ui.label(RichText::new(error).color(pal.critical));
        ui.add_space(6.0);
    }
    let mut save = false;
    let mut cancel = false;
    let mut del = false;
    ui.horizontal(|ui| {
        if let Some(d) = danger {
            if w::button(ui, Some(ic::TRASH), d, Kind::Danger).clicked() {
                del = true;
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if w::button_ex(ui, Some(ic::CHECK), save_label, Kind::Primary, true, true).on_hover_text("Ctrl+Enter").clicked() {
                save = true;
            }
            if w::button(ui, None, "Cancelar", Kind::Secondary).on_hover_text("Esc").clicked() {
                cancel = true;
            }
        });
    });
    if ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter)) {
        save = true;
    }
    (save, cancel, del)
}

/// Muestra el diálogo abierto y aplica su resultado.
pub fn show(ws: &mut Workspace, ctx: &egui::Context) {
    if matches!(ws.modal, Modal::None) {
        return;
    }
    let pal = p();
    let mut modal = std::mem::replace(&mut ws.modal, Modal::None);
    let mut keep = true;
    // Esc se lee antes de pintar: si no, el campo con el cursor se la queda
    // (para soltar el foco) y el diálogo no se cerraba.
    let esc = ctx.input(|i| i.key_pressed(Key::Escape));
    let width = match &modal {
        Modal::Csv(_) | Modal::CategoryDetail { .. } => 760.0,
        Modal::Confirm { .. } => 420.0,
        _ => 520.0,
    };
    let resp = egui::Modal::new(egui::Id::new("modal"))
        .backdrop_color(egui::Color32::from_black_alpha(if pal.dark { 170 } else { 90 }))
        .frame(egui::Frame::new().fill(pal.surface).stroke(egui::Stroke::new(1.0, pal.line_strong)).corner_radius(w::R2).inner_margin(egui::Margin::same(22)))
        .show(ctx, |ui| {
            ui.set_width(width);
            keep = body(ws, ui, &mut modal);
        });
    let _ = resp;
    if esc {
        keep = false;
    }
    // Un diálogo puede abrir otro (p. ej. la ficha abre «Editar categoría"):
    // si el cuerpo ya puso uno nuevo, no se pisa.
    if keep && matches!(ws.modal, Modal::None) {
        ws.modal = modal;
    }
}

fn title(ui: &mut Ui, t: &str) {
    ui.label(RichText::new(t).font(theme::wide(17.0)).color(p().ink));
    ui.add_space(6.0);
}

/// Devuelve false cuando el diálogo debe cerrarse.
fn body(ws: &mut Workspace, ui: &mut Ui, modal: &mut Modal) -> bool {
    let pal = p();
    let accounts = ws.account_names();
    match modal {
        Modal::None => false,
        Modal::Tx(f) => {
            let editing_transfer = f.transfer_id.is_some();
            let full = ui.available_width();
            ui.vertical_centered(|ui| {
                let t = match (f.id.is_some() || editing_transfer, f.kind) {
                    (true, 2) => "Editar transferencia",
                    (true, _) => "Editar movimiento",
                    (false, 2) => "Nueva transferencia",
                    _ => "Nuevo movimiento",
                };
                ui.label(RichText::new(t).font(theme::wide(18.0)).color(pal.ink));
            });
            ui.add_space(12.0);
            if f.id.is_none() && !editing_transfer {
                w::segmented_fill(ui, &mut f.kind, &["Gasto", "Ingreso", "Transferencia"], full);
            } else if f.id.is_some() {
                let mut k = f.kind.min(1);
                if w::segmented_fill(ui, &mut k, &["Gasto", "Ingreso"], full) {
                    f.kind = k;
                    f.cat = None;
                }
            }
            // Importe grande y centrado, del color del tipo.
            let tone = match f.kind {
                0 => pal.gasto,
                1 => pal.ingreso,
                _ => pal.ink,
            };
            ui.add_space(16.0);
            // Ancho fijo de la caja del importe para poder centrarla.
            const BOX: f32 = 24.0 + 220.0 + 24.0 + 12.0 + 28.0 + 2.0;
            let r = ui
                .horizontal(|ui| {
                    ui.add_space(((full - BOX) / 2.0).max(0.0));
                    egui::Frame::new()
                        .stroke(egui::Stroke::new(1.0, pal.line_strong))
                        .fill(pal.bg)
                        .corner_radius(w::R2)
                        .inner_margin(egui::Margin::symmetric(14, 6))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let sign = match f.kind {
                                0 => "−",
                                1 => "+",
                                _ => "",
                            };
                            ui.add_sized(vec2(24.0, 44.0), egui::Label::new(RichText::new(sign).font(theme::display(30.0)).color(tone)));
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut f.amount)
                                    .id(egui::Id::new("tx_amount"))
                                    .font(theme::display(30.0))
                                    .text_color(tone)
                                    .hint_text(RichText::new("0,00").font(theme::display(30.0)).color(pal.line_strong))
                                    .horizontal_align(Align::Center)
                                    .desired_width(220.0)
                                    .frame(egui::Frame::NONE),
                            );
                            ui.add_sized(vec2(24.0, 44.0), egui::Label::new(RichText::new("€").font(theme::display(22.0)).color(pal.muted)));
                            r
                        })
                        .inner
                })
                .inner;
            ui.vertical_centered(|ui| {
                if f.focus {
                    r.request_focus();
                    f.focus = false;
                }
                let calc = util::eval_amount_cents(&f.amount).filter(|_| f.amount.trim_start_matches(['+', '-']).contains(['+', '-', '*', '/', 'x']));
                let hint = match calc {
                    Some(v) => format!("= {}", util::eur(v)),
                    None => "IMPORTE · ADMITE CUENTAS: 12,50+3*2".to_string(),
                };
                ui.add_space(4.0);
                ui.label(RichText::new(hint).font(theme::mono(11.0)).color(if calc.is_some() { pal.ink2 } else { pal.muted }));
            });
            ui.add_space(8.0);
            let ty = if f.kind == 1 { "ingreso" } else { "gasto" };
            form_label(ui, ic::TEXT_ALIGN_LEFT, if f.kind == 2 { "Nota (opcional)" } else { "Concepto" });
            let note = w::text_field(ui, &mut f.note, if f.kind == 2 { "Ahorro mensual…" } else { "Supermercado, nómina…" }, full);
            if f.kind != 2 {
                if note.changed() && f.id.is_none() {
                    // Un concepto ya usado propone su categoría y su cuenta.
                    if let Some((cat, acc)) = ws.core.guess_from_note(&f.note, ty) {
                        if f.cat.is_none() || f.cat_guessed {
                            f.cat = cat;
                            f.cat_guessed = true;
                        }
                        if !acc.is_empty() {
                            f.account = acc;
                        }
                    }
                }
                let sugg = ws.core.note_suggestions(&f.note, ty, 4);
                if !f.note.trim().is_empty() && !sugg.is_empty() && !(sugg.len() == 1 && sugg[0].eq_ignore_ascii_case(f.note.trim())) {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("¿Quisiste decir?").font(theme::ui_font(12.0)).color(pal.muted));
                        for sgt in sugg {
                            if w::button(ui, None, &sgt, Kind::Ghost).clicked() {
                                if let Some((cat, acc)) = ws.core.guess_from_note(&sgt, ty) {
                                    f.cat = cat;
                                    f.cat_guessed = true;
                                    if !acc.is_empty() {
                                        f.account = acc;
                                    }
                                }
                                f.note = sgt;
                            }
                        }
                    });
                }
            }
            if f.kind == 2 {
                ui.columns(2, |c| {
                    form_label(&mut c[0], ic::ARROW_UP_RIGHT, "De la cuenta");
                    let mut dummy = String::new();
                    let w0 = c[0].available_width();
                    account_picker_w(&mut c[0], "tf_from", &mut f.from, &accounts, false, &mut dummy, w0);
                    form_label(&mut c[1], ic::ARROW_DOWN_RIGHT, "A la cuenta");
                    let w1 = c[1].available_width();
                    account_picker_w(&mut c[1], "tf_to", &mut f.to, &accounts, true, &mut f.new_account, w1);
                });
            } else {
                form_label(ui, ic::TAG_SIMPLE, "Categoría");
                // «Otros» es el valor por defecto (sin categoría): no se repite
                // aunque exista como categoría.
                let fallback = if ty == "gasto" { "Otros" } else { "Otros ingresos" };
                let mut opts = vec![(None, fallback.to_string())];
                for (id, name) in ws.active_categories(ty) {
                    if name.eq_ignore_ascii_case(fallback) {
                        if f.cat == id {
                            f.cat = None;
                        }
                    } else {
                        opts.push((id, name));
                    }
                }
                let before = f.cat;
                w::search_select(ui, "tx_cat", &mut f.cat, &opts, full, "Escribe para buscar o elige abajo…");
                ui.add_space(6.0);
                let cats = ws.cache.categories.clone();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    for (id, name) in &opts {
                        let color = match id {
                            Some(id) => cats.iter().find(|c| c.id == *id).map(|c| theme::cat_color_for(c.id, &c.color)).unwrap_or(pal.muted),
                            None => pal.muted,
                        };
                        if cat_chip(ui, name, color, f.cat == *id).clicked() {
                            f.cat = *id;
                        }
                    }
                });
                if f.cat != before {
                    f.cat_guessed = false;
                }
                ui.add_space(2.0);
            }
            ui.columns(2, |c| {
                if f.kind != 2 {
                    form_label(&mut c[0], ic::BANK, "Cuenta");
                    let w0 = c[0].available_width();
                    account_picker_w(&mut c[0], "tx_acc", &mut f.account, &accounts, true, &mut f.new_account, w0);
                    form_label(&mut c[1], ic::CALENDAR_BLANK, "Fecha");
                    date_field(&mut c[1], &mut f.date);
                } else {
                    form_label(&mut c[0], ic::CALENDAR_BLANK, "Fecha");
                    date_field(&mut c[0], &mut f.date);
                }
            });
            if f.kind != 2 {
                form_label(ui, ic::HASH, "Etiquetas (opcional, separadas por comas)");
                w::text_field(ui, &mut f.tags, "vacaciones, reembolsable", full);
            }
            if f.kind != 2 && f.id.is_none() {
                ui.add_space(8.0);
                ui.checkbox(&mut f.as_template, "Guardar también como gasto rápido");
            }
            ui.add_space(6.0);
            w::hline(ui);
            let del_label = if f.id.is_some() || editing_transfer { Some("Borrar") } else { None };
            let (save, cancel, del) = footer(ui, &f.error, "Guardar", del_label);
            if cancel {
                return false;
            }
            if del {
                let r = if let Some(id) = f.id { ws.core.delete_tx(id) } else { ws.core.delete_transfer(f.transfer_id.unwrap_or(0)) };
                return ws.report(r, Some("Borrado")).is_none();
            }
            if save {
                let Some(cents) = util::eval_amount_cents(&f.amount).filter(|c| *c > 0) else {
                    f.error = "Escribe un importe mayor que cero".into();
                    return true;
                };
                let Some(date) = norm_date(&f.date) else {
                    f.error = "La fecha no es válida (AAAA-MM-DD)".into();
                    return true;
                };
                if f.kind == 2 {
                    let t = Transfer {
                        id: 0,
                        amount_cents: cents,
                        from_account: f.from.clone(),
                        to_account: resolve_account(&f.to, &f.new_account),
                        note: f.note.clone(),
                        date,
                    };
                    match ws.core.save_transfer(f.transfer_id, &t) {
                        Ok(_) => {
                            ws.toast("Transferencia guardada", false);
                            return false;
                        }
                        Err(e) => f.error = e,
                    }
                } else {
                    let input = TxInput {
                        ty: if f.kind == 1 { "ingreso".into() } else { "gasto".into() },
                        amount_cents: cents,
                        category_id: f.cat,
                        note: f.note.clone(),
                        account: resolve_account(&f.account, &f.new_account),
                        date,
                        tags: f.tags.clone(),
                        source: "escritorio".into(),
                    };
                    let r = match f.id {
                        Some(id) => ws.core.update_tx(id, &input).map(|_| id),
                        None => ws.core.create_tx(&input),
                    };
                    match r {
                        Ok(id) => {
                            if f.as_template {
                                let r2 = ws.core.create_template("", &input);
                                ws.report(r2, None);
                            }
                            if f.id.is_none() {
                                ws.toast_undo(format!("Apuntado: {}", if input.note.is_empty() { "movimiento" } else { &input.note }), id);
                            } else {
                                ws.toast("Movimiento actualizado", false);
                            }
                            return false;
                        }
                        Err(e) => f.error = e,
                    }
                }
            }
            true
        }
        Modal::Account(f) => {
            title(ui, if f.original.is_some() { "Cuenta" } else { "Nueva cuenta" });
            if f.original.is_none() {
                field(ui, "Nombre");
                w::text_field(ui, &mut f.name, "Banco, tarjeta, efectivo…", f32::INFINITY);
            } else {
                ui.label(RichText::new(&f.name).font(theme::medium(16.0)));
                ui.label(RichText::new(format!("Saldo actual: {}", util::eur(f.current_cents))).font(theme::mono(13.0)).color(pal.ink2));
            }
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    field(ui, "Saldo inicial");
                    w::mono_field(ui, &mut f.opening, "0,00", 140.0);
                });
                ui.vertical(|ui| {
                    field(ui, "A fecha de");
                    date_field(ui, &mut f.date);
                });
            });
            field(ui, "TAE % (solo cuentas remuneradas)");
            w::mono_field(ui, &mut f.apy, "0", 100.0);
            ui.label(
                RichText::new("Con TAE, el interés se calcula a diario y el último día de cada mes se apunta como ingreso en «Intereses».")
                    .font(theme::ui_font(12.0))
                    .color(pal.muted),
            );
            let mut extra_close = false;
            if let Some(orig) = f.original.clone() {
                ui.add_space(10.0);
                w::hline(ui);
                field(ui, "Cuadrar con el banco: saldo real hoy");
                ui.horizontal(|ui| {
                    w::mono_field(ui, &mut f.reconcile, "0,00", 140.0);
                    if w::button(ui, Some(ic::SCALES), "Cuadrar", Kind::Secondary).clicked() {
                        match util::eval_amount_cents(&f.reconcile) {
                            Some(real) => {
                                let r = ws.core.reconcile(&orig, real);
                                if let Some(diff) = ws.report(r, None) {
                                    ws.toast(
                                        if diff == 0 { "Ya cuadraba".to_string() } else { format!("Ajuste apuntado: {}", util::eur_signed(diff)) },
                                        false,
                                    );
                                    extra_close = true;
                                }
                            }
                            None => f.error = "Escribe el saldo real".into(),
                        }
                    }
                });
                field(ui, "Renombrar (cambia todo el histórico)");
                ui.horizontal(|ui| {
                    w::text_field(ui, &mut f.rename_to, "Nuevo nombre", 240.0);
                    if f.rename_to.trim() != orig && w::button(ui, Some(ic::PENCIL_SIMPLE), "Renombrar", Kind::Secondary).clicked() {
                        match ws.core.rename_account(&orig, &f.rename_to) {
                            Ok(()) => {
                                ws.toast("Cuenta renombrada", false);
                                extra_close = true;
                            }
                            Err(e) => f.error = e,
                        }
                    }
                });
            }
            if extra_close {
                return false;
            }
            let (save, cancel, del) = footer(ui, &f.error, "Guardar", f.original.as_ref().map(|_| "Quitar saldo inicial"));
            if cancel {
                return false;
            }
            if del {
                let name = f.original.clone().unwrap_or_default();
                ws.modal = Modal::Confirm {
                    title: format!("¿Quitar el saldo inicial de «{name}»?"),
                    body: "La cuenta seguirá apareciendo si tiene movimientos, con el saldo calculado desde cero.".into(),
                    button: "Quitar".into(),
                    action: Confirm::DeleteAccount(name),
                };
                return false;
            }
            if save {
                let opening = if f.opening.trim().is_empty() { Some(0) } else { util::eval_amount_cents(&f.opening) };
                let Some(opening) = opening else {
                    f.error = "El saldo inicial no es válido".into();
                    return true;
                };
                let apy: f64 = if f.apy.trim().is_empty() { 0.0 } else { f.apy.trim().replace('%', "").replace(',', ".").parse().unwrap_or(-1.0) };
                let Some(date) = norm_date(&f.date) else {
                    f.error = "La fecha no es válida".into();
                    return true;
                };
                match ws.core.save_account(&f.name, opening, &date, apy) {
                    Ok(()) => {
                        ws.toast("Cuenta guardada", false);
                        return false;
                    }
                    Err(e) => f.error = e,
                }
            }
            true
        }
        Modal::Category(f) => {
            title(ui, if f.id.is_some() { "Editar categoría" } else { "Nueva categoría" });
            if f.id.is_none() {
                w::segmented(ui, &mut f.ty, &["Gasto", "Ingreso"]);
            }
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    field(ui, "Emoji");
                    w::text_field(ui, &mut f.icon, "📦", 60.0);
                });
                ui.vertical(|ui| {
                    field(ui, "Nombre");
                    w::text_field(ui, &mut f.name, "Nombre", 300.0);
                });
            });
            field(ui, "Color");
            ui.horizontal_wrapped(|ui| {
                for c in theme::CATEGORY_SWATCHES {
                    let col = theme::cat_color(c);
                    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), egui::Sense::click());
                    ui.painter().rect_filled(r, w::R2, col);
                    if f.color.eq_ignore_ascii_case(c) {
                        ui.painter().rect_stroke(r.expand(2.0), w::R2, egui::Stroke::new(2.0, pal.ink), egui::StrokeKind::Outside);
                    }
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        f.color = c.to_string();
                    }
                }
            });
            if f.ty == 0 {
                field(ui, "Presupuesto mensual (vacío = sin presupuesto)");
                w::mono_field(ui, &mut f.budget, "0,00", 140.0);
                ui.add_space(6.0);
                ui.checkbox(&mut f.fixed, "Gasto fijo (alquiler, suscripciones…): se separa en «fijos · variables»");
            }
            let protected = ["otros", "otros ingresos"].contains(&f.name.trim().to_lowercase().as_str()) && f.id.is_some();
            let (save, cancel, del) = footer(ui, &f.error, "Guardar", if f.id.is_some() && !protected { Some("Borrar") } else { None });
            if cancel {
                return false;
            }
            if del {
                let id = f.id.unwrap_or(0);
                ws.modal = Modal::Confirm {
                    title: format!("¿Borrar «{}»?", f.name),
                    body: "Si tiene movimientos se archiva (no se pierde el histórico); si no, se borra.".into(),
                    button: "Borrar".into(),
                    action: Confirm::DeleteCategory(id),
                };
                return false;
            }
            if save {
                let budget = if f.budget.trim().is_empty() { Some(0) } else { util::eval_amount_cents(&f.budget) };
                let Some(budget) = budget else {
                    f.error = "El presupuesto no es válido".into();
                    return true;
                };
                let ty = if f.ty == 1 { "ingreso" } else { "gasto" };
                match ws.core.save_category(f.id, &f.name, ty, &f.icon, &f.color, budget, f.fixed) {
                    Ok(_) => {
                        ws.toast("Categoría guardada", false);
                        return false;
                    }
                    Err(e) => f.error = e,
                }
            }
            true
        }
        Modal::Recurring(f) => {
            title(ui, if f.id.is_some() { "Editar recurrente" } else { "Nuevo recurrente" });
            w::segmented(ui, &mut f.kind, &["Gasto", "Ingreso", "Traspaso", "Aportación"]);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    field(ui, "Importe");
                    w::mono_field(ui, &mut f.amount, "0,00", 140.0);
                });
                ui.vertical(|ui| {
                    field(ui, "Día del mes (31 = último)");
                    w::mono_field(ui, &mut f.day, "1", 70.0);
                });
            });
            field(ui, "Concepto");
            w::text_field(ui, &mut f.note, "Alquiler, Spotify, nómina…", f32::INFINITY);
            let mut dummy = String::new();
            match f.kind {
                0 | 1 => {
                    let ty = if f.kind == 1 { "ingreso" } else { "gasto" };
                    field(ui, "Categoría");
                    let mut opts = vec![(None, "Otros".to_string())];
                    opts.extend(ws.active_categories(ty));
                    w::select(ui, "rec_cat", &mut f.cat, &opts, 300.0);
                    field(ui, "Cuenta");
                    account_picker(ui, "rec_acc", &mut f.account, &accounts, false, &mut dummy);
                }
                2 => {
                    field(ui, "De la cuenta");
                    account_picker(ui, "rec_from", &mut f.from, &accounts, false, &mut dummy);
                    field(ui, "A la cuenta");
                    account_picker(ui, "rec_to", &mut f.to, &accounts, false, &mut dummy);
                }
                _ => {
                    field(ui, "Inversión");
                    let invs: Vec<(Option<i64>, String)> = ws.cache.portfolio.items.iter().map(|i| (Some(i.id), i.name.clone())).collect();
                    let mut opts = vec![(None, "Elige una inversión…".to_string())];
                    opts.extend(invs);
                    w::select(ui, "rec_inv", &mut f.inv, &opts, 300.0);
                    field(ui, "Cuenta que paga (opcional: apunta el gasto en «Inversiones»)");
                    account_picker(ui, "rec_pay", &mut f.from, &accounts, false, &mut dummy);
                }
            }
            if f.id.is_some() {
                ui.add_space(6.0);
                ui.checkbox(&mut f.active, "Activo");
            }
            let (save, cancel, del) = footer(ui, &f.error, "Guardar", f.id.map(|_| "Borrar"));
            if cancel {
                return false;
            }
            if del {
                ws.modal = Modal::Confirm {
                    title: "¿Borrar este recurrente?".into(),
                    body: "Los movimientos que ya creó se conservan.".into(),
                    button: "Borrar".into(),
                    action: Confirm::DeleteRecurring(f.id.unwrap_or(0)),
                };
                return false;
            }
            if save {
                let Some(cents) = util::eval_amount_cents(&f.amount).filter(|c| *c > 0) else {
                    f.error = "Escribe un importe mayor que cero".into();
                    return true;
                };
                let r = Recurring {
                    kind: ["movimiento", "movimiento", "traspaso", "aportacion"][f.kind].into(),
                    ty: if f.kind == 1 { "ingreso".into() } else { "gasto".into() },
                    amount_cents: cents,
                    category_id: f.cat,
                    note: f.note.clone(),
                    account: f.account.clone(),
                    day_of_month: f.day.trim().parse().unwrap_or(0),
                    active: f.active,
                    from_account: f.from.clone(),
                    to_account: f.to.clone(),
                    investment_id: f.inv,
                    ..Default::default()
                };
                match ws.core.save_recurring(f.id, &r) {
                    Ok(_) => {
                        ws.toast("Recurrente guardado", false);
                        return false;
                    }
                    Err(e) => f.error = e,
                }
            }
            true
        }
        Modal::Investment(f) => {
            title(ui, if f.id.is_some() { "Editar inversión" } else { "Nueva inversión" });
            field(ui, "Fuente del precio");
            if w::segmented(ui, &mut f.provider, &["Bolsa (Yahoo)", "Cripto (CoinGecko)", "Manual"]) {
                f.hits.clear();
            }
            if f.provider < 2 {
                field(ui, "Buscar por nombre o ticker");
                ui.horizontal(|ui| {
                    let r = w::text_field(ui, &mut f.query, "ishares acwi, bitcoin…", 300.0);
                    let go = w::button(ui, Some(ic::MAGNIFYING_GLASS), "Buscar", Kind::Secondary).clicked()
                        || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
                    if go && f.query.trim().len() >= 2 && !f.searching {
                        let (tx, rx) = std::sync::mpsc::channel();
                        let q = f.query.clone();
                        let prov = if f.provider == 1 { "coingecko" } else { "yahoo" };
                        let ctx = ui.ctx().clone();
                        std::thread::spawn(move || {
                            let _ = tx.send(crate::ledger::investments::search_symbols(prov, &q));
                            ctx.request_repaint();
                        });
                        ws.search_rx = Some(rx);
                        f.searching = true;
                    }
                    if f.searching {
                        ui.spinner();
                    }
                });
                if let Some(rx) = &ws.search_rx {
                    if let Ok(hits) = rx.try_recv() {
                        f.hits = hits;
                        f.searching = false;
                        ws.search_rx = None;
                        if f.hits.is_empty() {
                            f.error = "Sin resultados (o sin conexión)".into();
                        } else {
                            f.error.clear();
                        }
                    }
                }
                for h in f.hits.clone() {
                    let (resp, _) = w::row_frame(ui, f.symbol == h.symbol, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&h.symbol).font(theme::mono(13.0)));
                            ui.label(RichText::new(&h.name).color(pal.ink2));
                            ui.label(RichText::new(&h.exchange).font(theme::ui_font(12.0)).color(pal.muted));
                        });
                    });
                    if resp.clicked() {
                        f.symbol = h.symbol.clone();
                        if f.name.trim().is_empty() {
                            f.name = h.name.clone();
                        }
                    }
                }
                field(ui, "Símbolo");
                w::text_field(ui, &mut f.symbol, "IUSQ.DE, bitcoin…", 220.0);
            }
            field(ui, "Nombre");
            w::text_field(ui, &mut f.name, "iShares MSCI ACWI", f32::INFINITY);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    field(ui, "Unidades");
                    w::mono_field(ui, &mut f.units, "0", 130.0);
                });
                ui.vertical(|ui| {
                    field(ui, "Coste total pagado (€)");
                    w::mono_field(ui, &mut f.cost, "0,00", 140.0);
                });
                if f.provider == 2 {
                    ui.vertical(|ui| {
                        field(ui, "Precio por unidad (€)");
                        w::mono_field(ui, &mut f.manual, "0,00", 130.0);
                    });
                }
            });
            if f.id.is_some() {
                ui.label(
                    RichText::new("Cambiar unidades o coste aquí reinicia el historial de operaciones a un único lote.").font(theme::ui_font(12.0)).color(pal.muted),
                );
            }
            let (save, cancel, del) = footer(ui, &f.error, "Guardar", f.id.map(|_| "Borrar"));
            if cancel {
                return false;
            }
            if del {
                ws.modal = Modal::Confirm {
                    title: format!("¿Borrar «{}»?", f.name),
                    body: "Se borran la posición y su historial de operaciones.".into(),
                    button: "Borrar".into(),
                    action: Confirm::DeleteInvestment(f.id.unwrap_or(0)),
                };
                return false;
            }
            if save {
                let units: f64 = f.units.trim().replace(',', ".").parse().unwrap_or(-1.0);
                let cost = if f.cost.trim().is_empty() { Some(0) } else { util::eval_amount_cents(&f.cost) };
                let manual = if f.provider == 2 { util::eval_amount_cents(&f.manual).map(|c| c as f64 / 100.0) } else { None };
                let Some(cost) = cost else {
                    f.error = "El coste no es válido".into();
                    return true;
                };
                let prov = ["yahoo", "coingecko", "manual"][f.provider];
                match ws.core.save_investment(f.id, &f.name, prov, &f.symbol, units, cost, manual) {
                    Ok(_) => {
                        ws.toast("Inversión guardada", false);
                        ws.refresh_prices(true);
                        return false;
                    }
                    Err(e) => f.error = e,
                }
            }
            true
        }
        Modal::Ops(f) => {
            title(ui, &format!("Operaciones · {}", f.name));
            let rep = ws.core.replay_ops(f.inv_id).unwrap_or_default();
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for op in rep.ops.iter().rev() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(util::date_label(&op.date)).font(theme::mono(12.0)).color(pal.muted));
                        let col = if op.ty == "compra" { pal.ink } else { pal.positive };
                        ui.label(RichText::new(util::cap(&op.ty)).color(col));
                        ui.label(RichText::new(format!("{} ud", op.units)).font(theme::mono(12.5)));
                        ui.label(RichText::new(util::eur(op.amount_cents)).font(theme::mono(12.5)));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if w::icon_button(ui, ic::TRASH, "Borrar operación").clicked() {
                                let r = ws.core.delete_op(op.id);
                                ws.report(r, Some("Operación borrada"));
                            }
                        });
                    });
                }
            });
            ui.label(
                RichText::new(format!(
                    "Posición: {} ud · coste {} · ganancia realizada {}",
                    (rep.units * 1e6).round() / 1e6,
                    util::eur(rep.cost_cents),
                    util::eur_signed(rep.realized_gain_cents)
                ))
                .font(theme::mono(12.0))
                .color(pal.ink2),
            );
            ui.add_space(8.0);
            w::hline(ui);
            field(ui, "Nueva operación");
            w::segmented(ui, &mut f.ty, &["Compra", "Venta"]);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    field(ui, "Unidades");
                    w::mono_field(ui, &mut f.units, "0", 120.0);
                });
                ui.vertical(|ui| {
                    field(ui, if f.ty == 0 { "Pagado (€)" } else { "Cobrado (€)" });
                    w::mono_field(ui, &mut f.amount, "0,00", 130.0);
                });
            });
            field(ui, "Fecha");
            date_field(ui, &mut f.date);
            let (save, cancel, _) = footer(ui, &f.error, "Añadir operación", None);
            if cancel {
                return false;
            }
            if save {
                let units: f64 = f.units.trim().replace(',', ".").parse().unwrap_or(-1.0);
                let Some(amount) = util::eval_amount_cents(&f.amount).filter(|c| *c >= 0) else {
                    f.error = "El importe no es válido".into();
                    return true;
                };
                let date = norm_date(&f.date).unwrap_or_else(util::today_str);
                match ws.core.add_op(f.inv_id, if f.ty == 0 { "compra" } else { "venta" }, units, amount, &date) {
                    Ok(()) => {
                        ws.toast("Operación añadida", false);
                        f.units.clear();
                        f.amount.clear();
                        f.error.clear();
                    }
                    Err(e) => f.error = e,
                }
            }
            true
        }
        Modal::Goal(f) => {
            title(ui, if f.id.is_some() { "Editar objetivo" } else { "Nuevo objetivo" });
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    field(ui, "Emoji");
                    w::text_field(ui, &mut f.icon, "🎯", 60.0);
                });
                ui.vertical(|ui| {
                    field(ui, "Nombre");
                    w::text_field(ui, &mut f.name, "Viaje a Japón, colchón…", 320.0);
                });
            });
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    field(ui, "Meta (€)");
                    w::mono_field(ui, &mut f.target, "0,00", 140.0);
                });
                ui.vertical(|ui| {
                    field(ui, "Fecha límite (opcional)");
                    w::mono_field(ui, &mut f.deadline, "AAAA-MM-DD", 130.0);
                });
            });
            let (save, cancel, del) = footer(ui, &f.error, "Guardar", f.id.map(|_| "Borrar"));
            if cancel {
                return false;
            }
            if del {
                ws.modal = Modal::Confirm {
                    title: format!("¿Borrar el objetivo «{}»?", f.name),
                    body: "Se borra el objetivo y lo ahorrado en él (no afecta a tus cuentas).".into(),
                    button: "Borrar".into(),
                    action: Confirm::DeleteGoal(f.id.unwrap_or(0)),
                };
                return false;
            }
            if save {
                let Some(target) = util::eval_amount_cents(&f.target).filter(|c| *c > 0) else {
                    f.error = "Escribe la meta en euros".into();
                    return true;
                };
                let deadline = if f.deadline.trim().is_empty() { None } else { norm_date(&f.deadline) };
                if !f.deadline.trim().is_empty() && deadline.is_none() {
                    f.error = "La fecha límite no es válida".into();
                    return true;
                }
                match ws.core.save_goal(f.id, &f.name, &f.icon, target, deadline.as_deref()) {
                    Ok(_) => {
                        ws.toast("Objetivo guardado", false);
                        return false;
                    }
                    Err(e) => f.error = e,
                }
            }
            true
        }
        Modal::GoalAdd(f) => {
            title(ui, &format!("{} · {}", if f.withdraw { "Retirar de" } else { "Aportar a" }, f.name));
            let mut k = f.withdraw as usize;
            if w::segmented(ui, &mut k, &["Aportar", "Retirar"]) {
                f.withdraw = k == 1;
            }
            field(ui, "Importe (€)");
            let r = w::mono_field(ui, &mut f.amount, "0,00", 140.0);
            r.request_focus();
            let (save, cancel, _) = footer(ui, &f.error, if f.withdraw { "Retirar" } else { "Aportar" }, None);
            if cancel {
                return false;
            }
            if save || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))) {
                let Some(c) = util::eval_amount_cents(&f.amount).filter(|c| *c > 0) else {
                    f.error = "Escribe un importe".into();
                    return true;
                };
                match ws.core.goal_add(f.id, if f.withdraw { -c } else { c }) {
                    Ok(()) => return false,
                    Err(e) => f.error = e,
                }
            }
            true
        }
        Modal::Template(f) => {
            title(ui, "Nuevo gasto rápido");
            ui.label(RichText::new("Aparece en el mes y se apunta hoy con un clic.").color(pal.muted));
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    field(ui, "Nombre");
                    w::text_field(ui, &mut f.name, "Café", 220.0);
                });
                ui.vertical(|ui| {
                    field(ui, "Importe");
                    w::mono_field(ui, &mut f.amount, "0,60", 110.0);
                });
            });
            field(ui, "Categoría");
            let mut opts = vec![(None, "Otros".to_string())];
            opts.extend(ws.active_categories("gasto"));
            w::select(ui, "tpl_cat", &mut f.cat, &opts, 300.0);
            field(ui, "Cuenta");
            let mut dummy = String::new();
            account_picker(ui, "tpl_acc", &mut f.account, &accounts, false, &mut dummy);
            let (save, cancel, _) = footer(ui, &f.error, "Crear", None);
            if cancel {
                return false;
            }
            if save {
                let Some(c) = util::eval_amount_cents(&f.amount).filter(|c| *c > 0) else {
                    f.error = "Escribe un importe".into();
                    return true;
                };
                let input = TxInput {
                    ty: "gasto".into(),
                    amount_cents: c,
                    category_id: f.cat,
                    note: f.name.clone(),
                    account: f.account.clone(),
                    ..Default::default()
                };
                match ws.core.create_template(&f.name, &input) {
                    Ok(_) => return false,
                    Err(e) => f.error = e,
                }
            }
            true
        }
        Modal::Password(f) => {
            title(ui, "Cambiar contraseña");
            ui.label(RichText::new("El archivo se vuelve a cifrar con la nueva. Las copias antiguas siguen con la anterior.").color(pal.muted));
            for (label, v) in [("Contraseña actual", &mut f.current), ("Nueva (mínimo 8)", &mut f.new1), ("Repite la nueva", &mut f.new2)] {
                field(ui, label);
                ui.add(egui::TextEdit::singleline(v).password(true).desired_width(f32::INFINITY).font(theme::mono(14.0)).margin(egui::Margin::symmetric(10, 7)));
            }
            let (save, cancel, _) = footer(ui, &f.error, "Cambiar", None);
            if cancel {
                return false;
            }
            if save {
                if !ws.verify_password(&f.current) {
                    f.error = "La contraseña actual no es correcta".into();
                } else if f.new1 != f.new2 {
                    f.error = "Las contraseñas nuevas no coinciden".into();
                } else {
                    let new = f.new1.clone();
                    match ws.change_password(&new) {
                        Ok(()) => {
                            ws.toast("Contraseña cambiada", false);
                            return false;
                        }
                        Err(e) => f.error = e,
                    }
                }
            }
            true
        }
        Modal::CategoryDetail { id, month } => {
            let id = *id;
            let Some(cat) = ws.cache.categories.iter().find(|c| c.id == id).cloned() else {
                return false;
            };
            let privacy = ws.privacy;
            let col = theme::cat_color_for(id, &cat.color);
            ui.horizontal(|ui| {
                w::color_strip(ui, col, vec2(6.0, 26.0));
                ui.label(RichText::new(&cat.name).font(theme::wide(19.0)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if w::icon_button(ui, ic::CARET_RIGHT, "Mes siguiente").clicked() {
                        *month = util::add_months(month, 1);
                    }
                    ui.label(RichText::new(util::cap(&util::month_label(month))).font(theme::medium(14.0)).color(pal.ink2));
                    if w::icon_button(ui, ic::CARET_LEFT, "Mes anterior").clicked() {
                        *month = util::add_months(month, -1);
                    }
                });
            });
            ui.add_space(8.0);
            let series = ws.core.category_monthly(id, month);
            let total_mes = series.last().map(|m| m.gastos_cents).unwrap_or(0);
            let s = ws.core.summary(month);
            let base = if cat.ty == "ingreso" { s.ingresos_cents } else { s.gastos_cents };
            let avg12 = series.iter().map(|m| m.gastos_cents).sum::<i64>() / 12;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 36.0;
                for (l, v) in [
                    ("Este mes", w::hide(util::eur(total_mes), privacy)),
                    (if cat.ty == "ingreso" { "De los ingresos" } else { "Del gasto del mes" }, format!("{}%", util::pct(total_mes, base))),
                    ("Media 12 meses", w::hide(util::eur(avg12), privacy)),
                ] {
                    ui.vertical(|ui| {
                        ui.label(w::caps(l));
                        ui.label(RichText::new(v).font(theme::mono_bold(18.0)));
                    });
                }
                let budget = ws.core.budget_for_month(id, month, cat.budget_cents);
                if budget > 0 {
                    ui.vertical(|ui| {
                        ui.label(w::caps("Presupuesto"));
                        let pc = util::pct(total_mes, budget);
                        ui.label(RichText::new(format!("{pc}% de {}", w::hide(util::eur(budget), privacy))).font(theme::mono_bold(18.0)).color(w::budget_color(pc, pal.ink)));
                    });
                }
            });
            ui.add_space(10.0);
            let labels: Vec<String> = series.iter().map(|m| util::MESES_CORTOS[(util::month_parts(&m.month).1 - 1) as usize].to_string()).collect();
            let vals: Vec<i64> = series.iter().map(|m| m.gastos_cents).collect();
            super::charts::line(ui, &labels, &vals, 150.0, col, privacy);
            ui.add_space(10.0);
            ui.label(w::caps("Movimientos del mes"));
            w::hline(ui);
            let txs = ws.core.transactions(&crate::ledger::TxFilter { month: Some(month.clone()), category_id: Some(id), limit: 500, ..Default::default() }).items;
            let mut edit_tx: Option<Tx> = None;
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if txs.is_empty() {
                    ui.label(RichText::new("Sin movimientos este mes.").color(pal.muted));
                }
                for t in &txs {
                    let (resp, _) = w::row_frame(ui, false, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(util::date_label(&t.date)).font(theme::mono(11.5)).color(pal.muted));
                            ui.label(if t.note.is_empty() { &t.category_name } else { &t.note });
                            ui.label(RichText::new(&t.account).font(theme::ui_font(12.5)).color(pal.muted));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(w::amount_text(t.amount_cents, &t.ty, 13.5, privacy));
                            });
                        });
                    });
                    if resp.clicked() {
                        edit_tx = Some(t.clone());
                    }
                }
            });
            if let Some(t) = edit_tx {
                ws.modal = Modal::edit_tx(&t);
                return false;
            }
            ui.add_space(12.0);
            let mut close = false;
            ui.horizontal(|ui| {
                if w::button(ui, Some(ic::LIST_BULLETS), "Ver en Movimientos", Kind::Secondary).clicked() {
                    ws.views.mov.reset_to_month(month);
                    ws.views.mov.category = Some(id);
                    ws.view = super::workspace::View::Movimientos;
                    close = true;
                }
                if w::button(ui, Some(ic::PENCIL_SIMPLE), "Editar categoría", Kind::Secondary).clicked() {
                    ws.modal = Modal::Category(CategoryForm {
                        id: Some(cat.id),
                        name: cat.name.clone(),
                        ty: if cat.ty == "ingreso" { 1 } else { 0 },
                        icon: cat.icon.clone(),
                        color: cat.color.clone(),
                        budget: if cat.budget_cents > 0 { util::fmt_num(cat.budget_cents).replace('.', "") } else { String::new() },
                        fixed: cat.fixed,
                        error: String::new(),
                    });
                    close = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if w::button(ui, None, "Cerrar", Kind::Secondary).on_hover_text("Esc").clicked() {
                        close = true;
                    }
                });
            });
            if close && !matches!(ws.modal, Modal::Category(_)) {
                return false;
            }
            !close
        }
        Modal::Csv(f) => {
            title(ui, "Importar CSV");
            let cols: Vec<(Option<usize>, String)> =
                std::iter::once((None, "(ninguna)".to_string())).chain(f.headers.iter().enumerate().map(|(i, h)| (Some(i), if h.is_empty() { format!("Columna {}", i + 1) } else { h.clone() }))).collect();
            egui::Grid::new("csv_map").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                for (label, slot, id) in [
                    ("Fecha", &mut f.map.fecha, "c_fecha"),
                    ("Importe", &mut f.map.importe, "c_imp"),
                    ("Concepto", &mut f.map.nota, "c_nota"),
                    ("Categoría", &mut f.map.categoria, "c_cat"),
                    ("Cuenta", &mut f.map.cuenta, "c_cta"),
                ] {
                    ui.label(label);
                    w::select(ui, id, slot, &cols, 240.0);
                    ui.end_row();
                }
                ui.label("Tipo");
                let modes = vec![
                    ("signo".to_string(), "Según el signo (negativo = gasto)".to_string()),
                    ("gasto".into(), "Todos gastos".into()),
                    ("ingreso".into(), "Todos ingresos".into()),
                ];
                w::select(ui, "c_modo", &mut f.map.mode, &modes, 300.0);
                ui.end_row();
            });
            let (items, skipped) = csv::map_rows(&f.rows, &f.map);
            ui.add_space(8.0);
            egui::Grid::new("csv_prev").striped(false).spacing([14.0, 4.0]).show(ui, |ui| {
                for h in ["Fecha", "Tipo", "Importe", "Categoría", "Concepto", "Cuenta"] {
                    ui.label(w::caps(h));
                }
                ui.end_row();
                for it in items.iter().take(5) {
                    ui.label(RichText::new(&it.date).font(theme::mono(12.0)));
                    ui.label(&it.ty);
                    ui.label(RichText::new(util::fmt_num(it.amount_cents)).font(theme::mono(12.0)));
                    ui.label(&it.category);
                    ui.label(&it.note);
                    ui.label(&it.account);
                    ui.end_row();
                }
            });
            ui.label(
                RichText::new(format!("{} movimientos listos{}", items.len(), if skipped > 0 { format!(" · {skipped} filas sin fecha o importe se ignoran") } else { String::new() }))
                    .color(pal.ink2),
            );
            let (save, cancel, _) = footer(ui, &f.error, "Importar", None);
            if cancel {
                return false;
            }
            if save {
                if items.is_empty() {
                    f.error = "No hay filas válidas: revisa las columnas de fecha e importe".into();
                    return true;
                }
                match ws.core.import_items(&items) {
                    Ok((c, d)) => {
                        ws.toast(format!("Importados {c} movimientos{}", if d > 0 { format!(" ({d} duplicados omitidos)") } else { String::new() }), false);
                        return false;
                    }
                    Err(e) => f.error = e,
                }
            }
            true
        }
        Modal::Confirm { title: t, body: b, button, action } => {
            title(ui, t);
            ui.label(RichText::new(b.as_str()).color(pal.ink2));
            ui.add_space(14.0);
            let mut close = false;
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if w::button(ui, Some(ic::TRASH), button, Kind::Danger).clicked() {
                        let r: Result<(), String> = match action {
                            Confirm::DeleteTx(id) => ws.core.delete_tx(*id),
                            Confirm::DeleteTransfer(id) => ws.core.delete_transfer(*id),
                            Confirm::DeleteAccount(n) => ws.core.delete_account(n),
                            Confirm::DeleteCategory(id) => ws.core.delete_category(*id).map(|_| ()),
                            Confirm::DeleteRecurring(id) => ws.core.delete_recurring(*id),
                            Confirm::DeleteInvestment(id) => ws.core.delete_investment(*id),
                            Confirm::DeleteGoal(id) => ws.core.delete_goal(*id),
                            Confirm::ReimportWeb(path) => match crate::db::import_sqlite_file(path) {
                                Ok(conn) => {
                                    ws.core.conn = conn;
                                    ws.core.touch();
                                    ws.core.apply_pending();
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            },
                        };
                        ws.report(r, Some("Hecho"));
                        close = true;
                    }
                    if w::button(ui, None, "Cancelar", Kind::Secondary).clicked() {
                        close = true;
                    }
                });
            });
            !close
        }
    }
}
