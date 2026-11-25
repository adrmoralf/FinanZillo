//! Mes: saldo enorme, apunte rápido, pulso de 12 meses, calendario, últimos
//! movimientos; a la derecha, cuentas, patrimonio y presupuestos.

use egui::{Align, Key, Layout, RichText, Ui, vec2};
use egui_phosphor::light as ic;

use super::movimientos::{RowAction, tx_row};
use crate::ui::charts;
use crate::ui::forms::Modal;
use crate::ui::theme::{self, p};
use crate::ui::widgets::{self as w, Kind};
use crate::ui::workspace::{View, Workspace};
use crate::util;

pub fn show(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    // Dos columnas propias (centro y lateral), cada una con su scroll.
    let full = ui.available_rect_before_wrap();
    let side_w = 336.0;
    let gap = 28.0;
    let left = egui::Rect::from_min_max(full.min, egui::pos2(full.right() - side_w - gap, full.bottom()));
    let right = egui::Rect::from_min_max(egui::pos2(full.right() - side_w, full.top()), full.max);
    ui.painter().vline(full.right() - side_w - gap / 2.0, full.y_range(), egui::Stroke::new(1.0, pal.line));
    ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| super::scroll(ui, "mes_centro", |ui| center(ws, ui)));
    ui.scope_builder(egui::UiBuilder::new().max_rect(right), |ui| super::scroll(ui, "mes_lado", |ui| side(ws, ui)));
}

fn delta_text(cur: i64, prev: i64, prev_month: &str, up_is_good: bool) -> Option<RichText> {
    if prev <= 0 || cur == 0 {
        return None;
    }
    let pct = util::pct(cur - prev, prev);
    if pct == 0 {
        return None;
    }
    let pal = p();
    let good = (pct > 0) == up_is_good;
    let mname = util::MESES_CORTOS[(util::month_parts(prev_month).1 - 1) as usize];
    Some(
        RichText::new(format!("{}{}% vs {mname}", if pct > 0 { "+" } else { "−" }, pct.abs()))
            .font(theme::mono(11.0))
            .color(if good { pal.positive } else { pal.critical }),
    )
}

fn center(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    let privacy = ws.privacy;
    // Cabecera: mes y acciones secundarias.
    ui.horizontal(|ui| {
        let mut m = ws.month.clone();
        if crate::ui::workspace::header_inline(ui, &mut m) {
            ws.month = m;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if w::button(ui, Some(ic::ARROWS_LEFT_RIGHT), "Transferencia", Kind::Secondary).on_hover_text("Ctrl+T").clicked() {
                ws.modal = Modal::new_transfer();
            }
            if w::button(ui, Some(ic::PLUS), "Movimiento", Kind::Secondary).on_hover_text("Todos los campos · Ctrl+Mayús+N").clicked() {
                ws.modal = Modal::new_tx(&ws.core);
            }
        });
    });
    ui.add_space(10.0);

    // Saldo del mes, enorme; a su lado, los cuatro datos que lo explican.
    let s = ws.cache.summary.clone();
    let bal = s.balance();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label(w::caps("Saldo del mes"));
            let sign = if bal > 0 { "+" } else if bal < 0 { "−" } else { "" };
            let whole = util::fmt_num(bal.abs());
            let (int, dec) = whole.split_once(',').unwrap_or((&whole, "00"));
            let col = if bal < 0 { pal.critical } else { pal.ink };
            let mut job = egui::text::LayoutJob::default();
            let fmt = |size: f32, c: egui::Color32| egui::TextFormat { font_id: theme::display(size), color: c, valign: Align::BOTTOM, ..Default::default() };
            if privacy {
                job.append("•••", 0.0, fmt(theme::SIZE_DISPLAY, col));
            } else {
                job.append(&format!("{sign}{int}"), 0.0, fmt(theme::SIZE_DISPLAY, col));
                job.append(&format!(",{dec} €"), 0.0, fmt(32.0, pal.muted));
            }
            ui.label(job);
        });
        ui.add_space(40.0);
        ui.vertical(|ui| {
            ui.add_space(4.0);
            egui::Grid::new("mes_kv").num_columns(2).spacing([40.0, 14.0]).show(ui, |ui| {
                kv(ui, "Ingresos", w::hide(util::eur(s.ingresos_cents), privacy), pal.ingreso, delta_text(s.ingresos_cents, s.prev_ingresos_cents, &s.prev_month, true));
                kv(ui, "Gastos", w::hide(util::eur(s.gastos_cents), privacy), pal.gasto, delta_text(s.gastos_cents, s.prev_gastos_cents, &s.prev_month, false));
                ui.end_row();
                match s.projected_balance() {
                    Some(pb) => {
                        let budget_bad = s.global_budget_cents > 0 && s.projected_gastos().unwrap_or(0) > s.global_budget_cents;
                        let col = if pb < 0 || budget_bad { pal.critical } else { pal.positive };
                        let hint = RichText::new(format!("gasto previsto {}", w::hide(util::eur(s.projected_gastos().unwrap_or(0)), privacy)))
                            .font(theme::mono(11.0))
                            .color(pal.muted);
                        kv(ui, "Al cerrar el mes", w::hide(util::eur_signed(pb), privacy), col, Some(hint));
                    }
                    None => kv(ui, "Movimientos", s.count.to_string(), pal.ink, None),
                }
                let sin = format!("{} de {} días sin gastar", s.days_without_spending(), s.elapsed_days);
                kv(ui, "Media diaria", w::hide(util::eur(s.daily_avg()), privacy), pal.ink, Some(RichText::new(sin).font(theme::mono(11.0)).color(pal.muted)));
                ui.end_row();
            });
        });
    });
    ui.add_space(14.0);

    quick_entry(ws, ui);
    ui.add_space(8.0);
    templates_row(ws, ui);
    ui.add_space(16.0);

    // El pulso de 12 meses es la firma de la pantalla: todo el ancho de la columna.
    ui.horizontal(|ui| {
        ui.label(w::caps("Gasto diario · 12 meses"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new("pasa por una línea para ver su total · clic para abrir ese mes").font(theme::ui_font(11.5)).color(pal.muted));
        });
    });
    ui.add_space(2.0);
    // Alto según la ventana: deja ver los últimos movimientos sin hacer scroll.
    let pulse_h = (ui.ctx().content_rect().height() - 640.0).clamp(220.0, 360.0);
    let pulse = ws.cache.pulse.clone();
    if let Some(m) = charts::pulse(ui, &pulse, &ws.month, pulse_h, privacy) {
        ws.month = m;
    }
    ui.add_space(14.0);

    // Últimos movimientos.
    ui.horizontal(|ui| {
        ui.label(w::caps("Últimos movimientos"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if w::button(ui, None, "Ver todos", Kind::Ghost).clicked() {
                ws.views.mov.reset_to_month(&ws.month);
                ws.view = View::Movimientos;
            }
        });
    });
    w::hline(ui);
    let recent = ws.core.recent(8);
    ui.spacing_mut().item_spacing.y = 2.0;
    if recent.is_empty() {
        w::empty_state(ui, ic::RECEIPT, "Aún no hay movimientos", "Escribe un importe arriba y pulsa Enter para apuntar el primero.");
    }
    for t in &recent {
        match tx_row(ws, ui, t, true) {
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
                    ws.toast_undo(format!("Repetido hoy: {}", if t.note.is_empty() { &t.category_name } else { &t.note }), id);
                }
            }
            RowAction::None => {}
        }
    }
}

fn kv(ui: &mut Ui, label: &str, value: String, color: egui::Color32, sub: Option<RichText>) {
    ui.vertical(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.label(w::caps(label));
        ui.label(RichText::new(value).font(theme::mono_bold(19.0)).color(color));
        if let Some(s) = sub {
            ui.label(s);
        } else {
            ui.label(RichText::new(" ").font(theme::mono(11.0)));
        }
    });
}

/// Línea de apunte rápido: importe, concepto, categoría, cuenta y Enter.
fn quick_entry(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    let ty = if ws.quick.ingreso { "ingreso" } else { "gasto" };
    let cats = ws.active_categories(ty);
    let accounts = ws.account_names();
    let mut submit = false;
    let frame = egui::Frame::new()
        .fill(pal.surface)
        .stroke(egui::Stroke::new(1.0, pal.line_strong))
        .corner_radius(w::R2)
        .inner_margin(egui::Margin::symmetric(10, 8));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(w::caps("Apuntar hoy · Ctrl+N · Enter guarda"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut idx = ws.quick.ingreso as usize;
                // El orden se invierte al pintar de derecha a izquierda.
                let mut rev = 1 - idx;
                if w::segmented(ui, &mut rev, &["Ingreso", "Gasto"]) {
                    idx = 1 - rev;
                    ws.quick.ingreso = idx == 1;
                    ws.quick.cat = None;
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let amt = w::mono_field(ui, &mut ws.quick.amount, "0,00 €", 104.0);
            if ws.quick.focus {
                amt.request_focus();
                ws.quick.focus = false;
            }
            // Concepto: el ancho que sobra tras los campos fijos.
            let fixed = 170.0 + 124.0 + 112.0 + 4.0 * ui.spacing().item_spacing.x + 4.0;
            let note_w = (ui.available_width() - fixed).max(120.0);
            let note = w::text_field(ui, &mut ws.quick.note, "Concepto (supermercado, nómina…)", note_w);
            if note.changed() {
                // Al escribir un concepto conocido se propone su categoría y su cuenta.
                if let Some((cat, acc)) = ws.core.guess_from_note(&ws.quick.note, ty) {
                    if ws.quick.cat.is_none() || ws.quick.cat_from_guess {
                        ws.quick.cat = cat;
                        ws.quick.cat_from_guess = true;
                    }
                    if !acc.is_empty() {
                        ws.quick.account = acc;
                    }
                }
            }
            let mut cat = ws.quick.cat;
            let cat_pick = w::search_select(ui, "quick_cat", &mut cat, &cats, 150.0, "Categoría…");
            if cat != ws.quick.cat {
                ws.quick.cat = cat;
                ws.quick.cat_from_guess = false;
            }
            let mut acc_opts: Vec<(String, String)> = accounts.iter().map(|a| (a.clone(), a.clone())).collect();
            if acc_opts.is_empty() {
                acc_opts.push((String::new(), "Sin cuenta".into()));
            }
            let mut acc = ws.quick.account.clone();
            let acc_pick = w::search_select(ui, "quick_acc", &mut acc, &acc_opts, 104.0, "Cuenta");
            ws.quick.account = acc;
            if cat_pick.enter || acc_pick.enter {
                submit = true;
            }
            if w::button_ex(ui, Some(ic::KEY_RETURN), "Apuntar", Kind::Primary, true, true).on_hover_text("Enter").clicked() {
                submit = true;
            }
            let enter = ui.input(|i| i.key_pressed(Key::Enter));
            if enter && (amt.lost_focus() || note.lost_focus()) {
                submit = true;
            }
        });
        // Sugerencias de concepto.
        let sugg = ws.core.note_suggestions(&ws.quick.note, ty, 5);
        let show_sugg = !sugg.is_empty() && !(sugg.len() == 1 && sugg[0].eq_ignore_ascii_case(ws.quick.note.trim()));
        if show_sugg || !ws.quick.error.is_empty() {
            ui.horizontal(|ui| {
                if !ws.quick.error.is_empty() {
                    ui.label(RichText::new(&ws.quick.error).color(pal.critical));
                }
                if show_sugg {
                    ui.label(RichText::new("¿Quisiste decir?").font(theme::ui_font(12.0)).color(pal.muted));
                    for sgt in sugg {
                        if w::button(ui, None, &sgt, Kind::Ghost).clicked() {
                            ws.quick.note = sgt.clone();
                            if let Some((cat, acc)) = ws.core.guess_from_note(&sgt, ty) {
                                ws.quick.cat = cat;
                                ws.quick.cat_from_guess = true;
                                if !acc.is_empty() {
                                    ws.quick.account = acc;
                                }
                            }
                            ws.quick.focus = true;
                        }
                    }
                }
            });
        }
    });
    if submit {
        ws.quick.error.clear();
        ws.quick_submit();
    }
}

/// Gastos rápidos: un clic apunta el gasto hoy.
fn templates_row(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    let templates = ws.cache.templates.clone();
    ui.horizontal_wrapped(|ui| {
        ui.label(w::caps("Rápidos"));
        ui.add_space(4.0);
        for t in &templates {
            let label = format!("{}  {}", t.name, if ws.privacy { "•••".into() } else { util::fmt_num(t.amount_cents) });
            let r = w::button(ui, None, &label, Kind::Secondary).on_hover_text("Clic: apuntar hoy · clic derecho: quitar");
            if r.clicked() {
                let res = ws.core.use_template(t.id);
                if let Some(id) = ws.report(res, None) {
                    ws.toast_undo(format!("Apuntado: {} −{}", t.name, util::eur(t.amount_cents)), id);
                }
            }
            r.context_menu(|ui| {
                if ui.button("Quitar este gasto rápido").clicked() {
                    let res = ws.core.delete_template(t.id);
                    ws.report(res, Some("Gasto rápido quitado"));
                    ui.close();
                }
            });
        }
        if w::button(ui, Some(ic::PLUS), "Nuevo rápido", Kind::Ghost).clicked() {
            ws.modal = Modal::new_template(&ws.core);
        }
        if templates.is_empty() {
            ui.label(RichText::new("Guarda tus gastos habituales (café, metro…) para apuntarlos con un clic.").color(pal.muted));
        }
    });
}

fn side(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    let privacy = ws.privacy;
    ui.horizontal(|ui| {
        ui.label(w::caps("Cuentas"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if w::button(ui, None, "Gestionar", Kind::Ghost).clicked() {
                ws.view = View::Cuentas;
            }
        });
    });
    w::hline(ui);
    let accounts = ws.cache.accounts.clone();
    let mut total = 0;
    for a in &accounts {
        total += a.current_cents;
        let (resp, _) = w::row_frame(ui, false, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new(&a.name).font(theme::medium(14.0)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let col = if a.current_cents < 0 { pal.critical } else { pal.ink };
                    ui.label(RichText::new(w::hide(util::fmt_num(a.current_cents), privacy)).font(theme::mono(15.0)).color(col));
                });
            });
            if a.apy > 0.0 {
                ui.label(
                    RichText::new(format!("{} % TAE · +{} este mes", util::fmt_apy(a.apy), w::hide(util::eur(a.interest_cents), privacy)))
                        .font(theme::mono(10.5))
                        .color(pal.positive),
                )
                .on_hover_text("Interés estimado del mes en curso. El último día del mes se apunta como ingreso en «Intereses»; puedes editar ese movimiento para cuadrarlo con el banco.");
            }
        });
        if resp.clicked() {
            ws.modal = Modal::edit_account(a);
        }
        w::hline(ui);
    }
    let inv = ws.cache.portfolio.value_cents;
    if inv > 0 {
        let (resp, _) = w::row_frame(ui, false, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Inversiones").font(theme::medium(14.0)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(w::hide(util::fmt_num(inv), privacy)).font(theme::mono(15.0)));
                });
            });
        });
        if resp.clicked() {
            ws.view = View::Inversiones;
        }
    }
    ui.add_space(6.0);
    ui.label(w::caps("Patrimonio"));
    ui.label(RichText::new(w::hide(util::eur(total + inv), privacy)).font(theme::display(30.0)).color(pal.ink));
    ui.add_space(18.0);

    // Presupuestos del mes.
    let s = ws.cache.summary.clone();
    ui.horizontal(|ui| {
        ui.label(w::caps("Presupuestos"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if w::button(ui, None, "Ajustar", Kind::Ghost).clicked() {
                ws.view = View::Presupuestos;
            }
        });
    });
    w::hline(ui);
    ui.add_space(4.0);
    if s.global_budget_cents > 0 {
        budget_line(ui, "Total del mes", pal.ink, s.gastos_cents, s.global_budget_cents, privacy);
    }
    for b in s.budgets.iter().take(6) {
        budget_line(ui, &b.name, theme::cat_color_for(b.id, &b.color), b.spent_cents, b.budget_cents, privacy);
    }
    if s.budgets.is_empty() && s.global_budget_cents == 0 {
        ui.label(RichText::new("Sin presupuestos. Ponlos en Presupuestos para ver aquí cómo vas.").color(pal.muted));
    }
    ui.add_space(18.0);

    // Tira de códigos: reparto del gasto del mes por categoría.
    if !s.by_category.is_empty() {
        ui.label(w::caps("En qué se va"));
        ui.add_space(4.0);
        let parts: Vec<(egui::Color32, i64)> = s.by_category.iter().map(|c| (theme::cat_color_for(c.id, &c.color), c.total_cents)).collect();
        charts::category_strip(ui, &parts, 12.0);
        ui.add_space(6.0);
        ui.spacing_mut().item_spacing.y = 0.0;
        for c in s.by_category.iter().take(5) {
            let (resp, _) = w::row_frame(ui, false, |ui| {
                ui.horizontal(|ui| {
                    w::color_strip(ui, theme::cat_color_for(c.id, &c.color), vec2(10.0, 10.0));
                    ui.label(RichText::new(&c.name).font(theme::ui_font(13.0)));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format!("{}%", util::pct(c.total_cents, s.gastos_cents))).font(theme::mono(11.5)).color(pal.muted));
                        ui.label(RichText::new(w::hide(util::fmt_num(c.total_cents), privacy)).font(theme::mono(12.5)));
                    });
                });
            });
            if let Some(id) = c.id {
                if resp.on_hover_text("Ver la ficha de la categoría").clicked() {
                    ws.modal = Modal::CategoryDetail { id, month: ws.month.clone() };
                }
            }
        }
    }
    ui.add_space(18.0);
    // Calendario del mes e ideas automáticas.
    ui.label(w::caps("Calendario"));
    ui.add_space(4.0);
    if let Some(d) = charts::heatmap(ui, &ws.month, &s.daily, privacy) {
        let date = format!("{}-{d:02}", ws.month);
        ws.views.mov.reset_to_day(&ws.month, &date);
        ws.view = View::Movimientos;
    }
    ui.add_space(8.0);
    for line in s.insights().into_iter().take(2) {
        ui.horizontal_wrapped(|ui| {
            w::color_strip(ui, pal.muted, vec2(6.0, 6.0));
            ui.label(RichText::new(w::hide(line, privacy)).font(theme::ui_font(12.5)).color(pal.ink2));
        });
    }
    ui.add_space(18.0);

}

fn budget_line(ui: &mut Ui, name: &str, color: egui::Color32, spent: i64, budget: i64, privacy: bool) {
    let pal = p();
    let pct = util::pct(spent, budget);
    ui.horizontal(|ui| {
        w::color_strip(ui, color, vec2(4.0, 16.0));
        ui.label(RichText::new(name).font(theme::ui_font(13.5)));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let pc = w::budget_color(pct, pal.ink2);
            ui.label(RichText::new(format!("{pct}%")).font(theme::mono_bold(12.5)).color(pc));
            ui.label(
                RichText::new(format!("{} / {}", w::hide(util::fmt_num(spent), privacy), w::hide(util::fmt_num(budget), privacy)))
                    .font(theme::mono(11.0))
                    .color(pal.muted),
            );
        });
    });
    w::progress(ui, spent as f32 / budget.max(1) as f32, w::budget_color(pct, color), 4.0);
    ui.add_space(6.0);
}
