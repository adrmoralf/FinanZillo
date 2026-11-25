//! Informe mensual en la app, con exportación a PDF, HTML y Markdown.

use std::path::{Path, PathBuf};

use egui::{Align, Layout, RichText, Ui};
use egui_phosphor::light as ic;

use crate::ui::theme::{self, p};
use crate::ui::widgets::{self as w, Kind};
use crate::ui::workspace::Workspace;
use crate::util;

/// Edge (o Chrome) instalado: imprime el HTML a PDF sin abrir ventana.
fn browser() -> Option<PathBuf> {
    [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
}

pub(crate) fn html_to_pdf(html: &str, out: &Path) -> Result<(), String> {
    let exe = browser().ok_or("No encuentro Edge ni Chrome para generar el PDF; exporta en HTML e imprímelo")?;
    let tmp = std::env::temp_dir().join(format!("finanzillo-informe-{}.html", std::process::id()));
    std::fs::write(&tmp, html).map_err(|e| e.to_string())?;
    let profile = std::env::temp_dir().join("finanzillo-pdf-perfil");
    let url = format!("file:///{}", tmp.display().to_string().replace('\\', "/"));
    let mut cmd = std::process::Command::new(exe);
    cmd.args([
        "--headless=new",
        "--disable-gpu",
        "--no-pdf-header-footer",
        &format!("--user-data-dir={}", profile.display()),
        &format!("--print-to-pdf={}", out.display()),
        &url,
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let status = cmd.status().map_err(|e| e.to_string());
    // El informe en claro solo vive lo justo en la carpeta temporal.
    let _ = std::fs::remove_file(&tmp);
    let status = status?;
    if !status.success() || !out.is_file() {
        return Err("El navegador no pudo generar el PDF".into());
    }
    Ok(())
}

pub fn show(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    let privacy = ws.privacy;
    let month = ws.views.informe_month.clone().unwrap_or_else(|| ws.month.clone());
    let rep = ws.core.report(&month);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Informe").font(theme::wide(22.0)));
        ui.add_space(12.0);
        if w::icon_button(ui, ic::CARET_LEFT, "Mes anterior").clicked() {
            ws.views.informe_month = Some(util::add_months(&month, -1));
        }
        ui.label(RichText::new(util::cap(&util::month_label(&month))).font(theme::medium(15.0)).color(pal.ink2));
        if w::icon_button(ui, ic::CARET_RIGHT, "Mes siguiente").clicked() {
            ws.views.informe_month = Some(util::add_months(&month, 1));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let name = format!("informe-finanzillo-{month}");
            if w::button_ex(ui, Some(ic::FILE_PDF), "Exportar PDF", Kind::Primary, true, true).clicked() {
                if let Some(path) = rfd::FileDialog::new().set_file_name(format!("{name}.pdf")).add_filter("PDF", &["pdf"]).save_file() {
                    let r = html_to_pdf(&rep.html(), &path);
                    if ws.report(r, Some("PDF guardado")).is_some() {
                        let _ = std::process::Command::new("explorer").arg(&path).spawn();
                    }
                }
            }
            if w::button(ui, Some(ic::FILE_HTML), "HTML", Kind::Secondary).clicked() {
                if let Some(path) = rfd::FileDialog::new().set_file_name(format!("{name}.html")).add_filter("HTML", &["html"]).save_file() {
                    let r = std::fs::write(&path, rep.html()).map_err(|e| e.to_string());
                    ws.report(r, Some("HTML guardado"));
                }
            }
            if w::button(ui, Some(ic::COPY), "Copiar resumen", Kind::Secondary).clicked() {
                ui.ctx().copy_text(rep.markdown());
                ws.toast("Resumen copiado en Markdown", false);
            }
        });
    });
    ui.label(RichText::new("Los archivos exportados no van cifrados: guárdalos donde quieras compartirlos.").color(pal.muted));
    ui.add_space(10.0);
    let s = rep.summary.clone();
    super::scroll(ui, "informe", |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 40.0;
            for (l, v, c) in [
                ("Saldo del mes", util::eur_signed(s.balance()), if s.balance() < 0 { pal.critical } else { pal.ink }),
                ("Ingresos", util::eur(s.ingresos_cents), pal.ingreso),
                ("Gastos", util::eur(s.gastos_cents), pal.gasto),
                ("Tasa de ahorro", format!("{}%", rep.savings_rate), pal.ink),
            ] {
                ui.vertical(|ui| {
                    ui.label(w::caps(l));
                    ui.label(RichText::new(w::hide(v, privacy)).font(theme::mono_bold(22.0)).color(c));
                });
            }
        });
        ui.add_space(16.0);
        ui.label(w::caps("Gasto medio por día de la semana"));
        crate::ui::charts::weekday(ui, &s.weekday_avg(), privacy);
        ui.add_space(16.0);
        ui.label(w::caps("Conclusiones"));
        w::hline(ui);
        for h in &rep.highlights {
            ui.horizontal_wrapped(|ui| {
                let c = match h.tone {
                    "bien" => pal.positive,
                    "mal" => pal.critical,
                    "aviso" => pal.warning,
                    _ => pal.ingreso,
                };
                w::color_strip(ui, c, egui::vec2(4.0, 16.0));
                ui.label(RichText::new(&h.title).font(theme::medium(14.0)));
                ui.label(RichText::new(w::hide(h.desc.clone(), privacy)).color(pal.ink2));
            });
        }
        ui.add_space(16.0);
        ui.label(w::caps("Mayores gastos"));
        w::hline(ui);
        for t in &rep.top_expenses {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&t.date[8..10]).font(theme::mono(12.0)).color(pal.muted));
                ui.label(if t.note.is_empty() { &t.category_name } else { &t.note });
                ui.label(RichText::new(&t.category_name).color(pal.muted));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(w::hide(util::eur(t.amount_cents), privacy)).font(theme::mono(13.0)));
                });
            });
        }
        ui.add_space(16.0);
        ui.label(w::caps("Flujo por cuenta"));
        w::hline(ui);
        egui::Grid::new("flujo").num_columns(4).spacing([24.0, 6.0]).show(ui, |ui| {
            for h in ["Cuenta", "Entradas", "Salidas", "Neto"] {
                ui.label(w::caps(h));
            }
            ui.end_row();
            for a in &rep.accounts {
                ui.label(&a.account);
                ui.label(RichText::new(w::hide(util::fmt_num(a.ingresos_cents + a.transfers_in_cents), privacy)).font(theme::mono(13.0)));
                ui.label(RichText::new(w::hide(util::fmt_num(a.gastos_cents + a.transfers_out_cents), privacy)).font(theme::mono(13.0)));
                ui.label(RichText::new(w::hide(util::eur_signed(a.net()), privacy)).font(theme::mono(13.0)).color(if a.net() < 0 { pal.critical } else { pal.positive }));
                ui.end_row();
            }
        });
    });
}
