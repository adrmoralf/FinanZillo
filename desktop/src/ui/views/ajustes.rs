//! Ajustes: seguridad, apariencia, copias, importar/exportar y atajos.

use egui::{RichText, Ui};
use egui_phosphor::light as ic;

use crate::ledger::csv;
use crate::store;
use crate::ui::forms::{Confirm, CsvForm, Modal, PasswordForm};
use crate::ui::theme::{self, p};
use crate::ui::widgets::{self as w, Kind};
use crate::ui::workspace::Workspace;

#[derive(Default)]
pub struct AjustesState {
    pub global_budget: Option<String>,
    pub autolock: Option<String>,
}

fn section(ui: &mut Ui, title: &str, hint: &str) {
    ui.add_space(20.0);
    ui.label(RichText::new(title).font(theme::wide(15.0)));
    if !hint.is_empty() {
        ui.label(RichText::new(hint).color(p().muted));
    }
    ui.add_space(4.0);
    w::hline(ui);
    ui.add_space(6.0);
}

pub fn show(ws: &mut Workspace, ui: &mut Ui) {
    let pal = p();
    ui.label(RichText::new("Ajustes").font(theme::wide(22.0)));
    super::scroll(ui, "ajustes", |ui| {
        section(ui, "Seguridad", "Tus datos viven en un único archivo cifrado con tu contraseña (XChaCha20-Poly1305 + Argon2id).");
        ui.label(RichText::new(store::data_file().display().to_string()).font(theme::mono(12.0)).color(pal.ink2));
        ui.horizontal(|ui| {
            if w::button(ui, Some(ic::KEY), "Cambiar contraseña", Kind::Secondary).clicked() {
                ws.modal = Modal::Password(PasswordForm { current: String::new(), new1: String::new(), new2: String::new(), error: String::new() });
            }
            if w::button(ui, Some(ic::FOLDER_OPEN), "Abrir carpeta de datos", Kind::Secondary).clicked() {
                let _ = std::process::Command::new("explorer").arg(store::data_dir()).spawn();
            }
        });
        ui.horizontal(|ui| {
            ui.label("Bloquear tras");
            let st = &mut ws.views.ajustes;
            let cur = ws.core.setting("autolock_min").unwrap_or_else(|| "15".into());
            let v = st.autolock.get_or_insert(cur.clone());
            w::mono_field(ui, v, "15", 60.0);
            ui.label("minutos sin usar la app (0 = nunca)");
            if v.trim() != cur && w::button(ui, None, "Guardar", Kind::Secondary).clicked() {
                match v.trim().parse::<u32>() {
                    Ok(n) => {
                        let r = ws.core.set_setting("autolock_min", &n.to_string());
                        ws.report(r, Some("Guardado"));
                    }
                    Err(_) => ws.toast("Escribe un número de minutos", true),
                }
            }
        });

        section(ui, "Apariencia", "");
        ui.horizontal(|ui| {
            let mut idx = if ws.dark { 0 } else { 1 };
            if w::segmented(ui, &mut idx, &["Oscuro", "Claro"]) {
                ws.dark = idx == 0;
                let _ = ws.core.set_setting("tema", if ws.dark { "oscuro" } else { "claro" });
            }
            ui.checkbox(&mut ws.privacy, "Ocultar importes (Ctrl+H)");
        });

        section(ui, "Copias de seguridad", "Cada día se guarda una copia cifrada (se conservan las 30 últimas). Puedes añadir una carpeta externa, por ejemplo de OneDrive.");
        ui.label(RichText::new(store::backups_dir().display().to_string()).font(theme::mono(12.0)).color(pal.ink2));
        let ext = ws.core.setting("backup_dir").unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Carpeta externa:");
            ui.label(RichText::new(if ext.is_empty() { "ninguna".into() } else { ext.clone() }).font(theme::mono(12.0)).color(pal.ink2));
        });
        ui.horizontal(|ui| {
            if w::button(ui, Some(ic::FOLDER_OPEN), "Elegir carpeta externa…", Kind::Secondary).clicked() {
                if let Some(d) = rfd::FileDialog::new().pick_folder() {
                    let r = ws.core.set_setting("backup_dir", &d.display().to_string());
                    ws.report(r, Some("Carpeta externa guardada"));
                }
            }
            if !ext.is_empty() && w::button(ui, None, "Quitar", Kind::Secondary).clicked() {
                let r = ws.core.set_setting("backup_dir", "");
                ws.report(r, None);
            }
            if w::button(ui, Some(ic::FLOPPY_DISK), "Guardar una copia ahora…", Kind::Secondary).clicked() {
                if let Some(path) = rfd::FileDialog::new().set_file_name(format!("finanzillo-{}.fzl", crate::util::today_str())).save_file() {
                    ws.save_now();
                    let r = std::fs::copy(store::data_file(), &path).map(|_| ()).map_err(|e| e.to_string());
                    ws.report(r, Some("Copia guardada (cifrada con tu contraseña)"));
                }
            }
            if w::button(ui, None, "Abrir carpeta de copias", Kind::Secondary).clicked() {
                let _ = std::fs::create_dir_all(store::backups_dir());
                let _ = std::process::Command::new("explorer").arg(store::backups_dir()).spawn();
            }
        });
        ui.label(
            RichText::new("Para restaurar una copia: cierra la app, renombra el .fzl elegido a finanzillo.fzl y ponlo en la carpeta de datos.")
                .font(theme::ui_font(12.0))
                .color(pal.muted),
        );

        section(ui, "Importar y exportar", "");
        ui.horizontal(|ui| {
            if w::button(ui, Some(ic::DOWNLOAD_SIMPLE), "Importar CSV del banco…", Kind::Secondary).clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv", "txt"]).pick_file() {
                    match std::fs::read(&path) {
                        Ok(bytes) => {
                            let text = String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|b| *b as char).collect());
                            let rows = csv::parse(&text);
                            if rows.len() < 2 {
                                ws.toast("El archivo no tiene datos (hace falta cabecera y filas)", true);
                            } else {
                                let headers: Vec<String> = rows[0].iter().map(|h| h.trim().to_string()).collect();
                                let map = csv::Mapping::guess(&headers);
                                ws.modal = Modal::Csv(CsvForm { headers, rows: rows[1..].to_vec(), map, error: String::new() });
                            }
                        }
                        Err(e) => ws.toast(format!("No se pudo leer: {e}"), true),
                    }
                }
            }
            if w::button(ui, Some(ic::UPLOAD_SIMPLE), "Exportar todo a CSV…", Kind::Secondary).clicked() {
                if let Some(path) = rfd::FileDialog::new().set_file_name("finanzillo.csv").add_filter("CSV", &["csv"]).save_file() {
                    let r = std::fs::write(&path, ws.core.export_csv()).map_err(|e| e.to_string());
                    ws.report(r, Some("CSV exportado (sin cifrar)"));
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if w::button(ui, Some(ic::DATABASE), "Volver a importar la base de la app web…", Kind::Secondary).clicked() {
                let start = crate::db::guess_web_db();
                let mut dlg = rfd::FileDialog::new().add_filter("Base de datos SQLite", &["db", "sqlite"]);
                if let Some(dir) = start.as_ref().and_then(|p| p.parent()) {
                    dlg = dlg.set_directory(dir);
                }
                if let Some(path) = dlg.pick_file() {
                    ws.modal = Modal::Confirm {
                        title: "¿Sustituir todos los datos?".into(),
                        body: format!("Se reemplazará todo lo que hay en la app por el contenido de {}. Lo actual se pierde (salvo en las copias diarias).", path.display()),
                        button: "Sustituir".into(),
                        action: Confirm::ReimportWeb(path),
                    };
                }
            }
        });

        section(ui, "Atajos de teclado", "Pulsa F1 en cualquier momento para verlos.");
        for (k, d) in [
            ("Ctrl+N", "Apuntar (línea rápida)"),
            ("Ctrl+Mayús+N", "Movimiento con todos los campos"),
            ("Ctrl+T", "Transferencia"),
            ("Ctrl+F", "Buscar movimientos"),
            ("Alt+← / Alt+→", "Cambiar de mes"),
            ("Ctrl+1 … Ctrl+0", "Secciones"),
            ("Ctrl+H", "Ocultar importes"),
            ("Ctrl+L", "Bloquear"),
        ] {
            ui.horizontal(|ui| {
                ui.add_sized([150.0, 18.0], egui::Label::new(RichText::new(k).font(theme::mono(12.5))));
                ui.label(RichText::new(d).color(pal.ink2));
            });
        }
        ui.add_space(20.0);
        ui.label(RichText::new(format!("FinanZillo {} · escritorio", env!("CARGO_PKG_VERSION"))).font(theme::mono(11.0)).color(pal.muted));
    });
}
