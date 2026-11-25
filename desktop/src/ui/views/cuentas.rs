//! Cuentas: saldos, TAE, orden, cuadre y evolución del patrimonio.

use egui::{Align, Layout, RichText, Ui};
use egui_phosphor::light as ic;

use crate::ui::charts;
use crate::ui::forms::Modal;
use crate::ui::theme::{self, p};
use crate::ui::widgets::{self as w, Kind};
use crate::ui::workspace::Workspace;
use crate::util;

pub fn show(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    let privacy = ws.privacy;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Cuentas").font(theme::wide(22.0)));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if w::button_ex(ui, Some(ic::PLUS), "Nueva cuenta", Kind::Primary, true, true).clicked() {
                ws.modal = Modal::new_account();
            }
            if w::button(ui, Some(ic::ARROWS_LEFT_RIGHT), "Transferencia", Kind::Secondary).clicked() {
                ws.modal = Modal::new_transfer();
            }
        });
    });
    ui.add_space(10.0);
    let accounts = ws.cache.accounts.clone();
    let inv = ws.cache.portfolio.value_cents;
    let total: i64 = accounts.iter().map(|a| a.current_cents).sum::<i64>() + inv;
    super::scroll(ui, "cuentas", |ui| {
        ui.label(w::caps("Patrimonio"));
        ui.label(RichText::new(w::hide(util::eur(total), privacy)).font(theme::display(54.0)));
        ui.label(
            RichText::new(format!("cuentas {} · inversiones {}", w::hide(util::eur(total - inv), privacy), w::hide(util::eur(inv), privacy)))
                .font(theme::mono(12.0))
                .color(pal.muted),
        );
        ui.add_space(14.0);
        let nw = ws.networth().to_vec();
        let labels: Vec<String> = nw.iter().map(|p| util::date_label(&p.date)).collect();
        let vals: Vec<i64> = nw.iter().map(|p| p.total()).collect();
        charts::line(ui, &labels, &vals, 200.0, pal.ink, privacy);
        ui.add_space(18.0);

        ui.label(w::caps("Saldos"));
        w::hline(ui);
        let n = accounts.len();
        let mut order: Option<Vec<String>> = None;
        for (i, a) in accounts.iter().enumerate() {
            let (resp, _) = w::row_frame(ui, false, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new(&a.name).font(theme::medium(15.0)));
                        let mut sub = if a.stored && !a.opening_date.starts_with("1970") {
                            format!("saldo inicial {} el {}", w::hide(util::eur(a.opening_cents), privacy), util::date_label(&a.opening_date))
                        } else {
                            "sin saldo inicial: se calcula desde cero".into()
                        };
                        if a.apy > 0.0 {
                            sub.push_str(&format!(" · {} % TAE · +{} este mes", util::fmt_apy(a.apy), w::hide(util::eur(a.interest_cents), privacy)));
                        }
                        ui.label(RichText::new(sub).font(theme::mono(11.0)).color(pal.muted));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        if i + 1 < n {
                            if w::icon_button_sm(ui, ic::ARROW_DOWN, "Bajar").clicked() {
                                let mut v: Vec<String> = accounts.iter().map(|x| x.name.clone()).collect();
                                v.swap(i, i + 1);
                                order = Some(v);
                            }
                        } else {
                            ui.add_space(26.0);
                        }
                        if i > 0 {
                            if w::icon_button_sm(ui, ic::ARROW_UP, "Subir").clicked() {
                                let mut v: Vec<String> = accounts.iter().map(|x| x.name.clone()).collect();
                                v.swap(i, i - 1);
                                order = Some(v);
                            }
                        } else {
                            ui.add_space(26.0);
                        }
                        ui.add_space(14.0);
                        let col = if a.current_cents < 0 { pal.critical } else { pal.ink };
                        ui.label(RichText::new(w::hide(util::eur(a.current_cents), privacy)).font(theme::mono_bold(17.0)).color(col));
                    });
                });
            });
            if resp.clicked() {
                ws.modal = Modal::edit_account(a);
            }
        }
        if let Some(v) = order {
            let r = ws.core.reorder_accounts(&v);
            ws.report(r, None);
        }
        if accounts.is_empty() {
            w::empty_state(ui, ic::BANK, "Aún no hay cuentas", "Crea una con su saldo inicial para que los saldos cuadren con el banco.");
        }
        ui.add_space(10.0);
        ui.label(RichText::new("Clic en una cuenta para editar su saldo inicial o TAE, cuadrarla con el banco o renombrarla.").color(pal.muted));
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.label(w::caps("Cuenta predeterminada al apuntar"));
            let mut def = ws.core.default_account();
            let opts: Vec<(String, String)> = accounts.iter().map(|a| (a.name.clone(), a.name.clone())).collect();
            if w::select(ui, "def_acc", &mut def, &opts, 200.0).changed() || def != ws.core.default_account() {
                let r = ws.core.set_setting("default_account", &def);
                ws.report(r, None);
                ws.quick.account = def;
            }
        });
    });
}
