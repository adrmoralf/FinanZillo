//! Arranque: crear el archivo cifrado (importando los datos de la web) o
//! desbloquearlo. Luego cede el control al espacio de trabajo.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use egui::{Align2, RichText, Ui, vec2};

use crate::ledger::Core;
use crate::store::{self, Sealer};
use crate::ui::theme::{self, p};
use crate::ui::widgets::{self as w, Kind};
use crate::ui::workspace::Workspace;
use crate::{db, util};

type Opened = Result<(Core, Sealer, bool), String>;

enum Phase {
    Setup { pass: String, pass2: String, import: bool, import_path: Option<PathBuf>, error: String },
    /// `hello_tried`: ya se pidió el PIN de Windows en esta pantalla (no repetir solo).
    Locked { pass: String, error: String, attempts: u32, hello_tried: bool },
    Busy { rx: Receiver<Opened>, label: String },
    Open(Box<Workspace>),
}

pub struct App {
    phase: Phase,
    dark: bool,
    applied: Option<bool>,
    title: String,
    #[cfg(debug_assertions)]
    shots: Option<crate::ui::testdrive::Shots>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_fonts(theme::fonts());
        let dark = cc.storage.and_then(|s| s.get_string("tema")).map(|t| t != "claro").unwrap_or(true);
        let phase = if store::data_file().exists() {
            Phase::Locked { pass: String::new(), error: String::new(), attempts: 0, hello_tried: false }
        } else {
            let found = db::guess_web_db();
            Phase::Setup { pass: String::new(), pass2: String::new(), import: found.is_some(), import_path: found, error: String::new() }
        };
        App {
            phase,
            dark,
            applied: None,
            title: String::new(),
            #[cfg(debug_assertions)]
            shots: crate::ui::testdrive::Shots::from_env(),
        }
    }
}

fn open_existing(password: String) -> Opened {
    let bytes = std::fs::read(store::data_file()).map_err(|e| format!("No se pudo leer el archivo de datos: {e}"))?;
    let (sealer, plain) = Sealer::open(&bytes, &password).map_err(|e| e.to_string())?;
    let conn = db::from_bytes(&plain).map_err(|e| format!("El archivo se descifró pero la base está dañada: {e}"))?;
    Ok((Core::new(conn), sealer, false))
}

fn open_with_hello() -> Opened {
    let bytes = std::fs::read(store::data_file()).map_err(|e| format!("No se pudo leer el archivo de datos: {e}"))?;
    let (sealer, plain) = crate::hello::unlock(&bytes)?;
    let conn = db::from_bytes(&plain).map_err(|e| format!("El archivo se descifró pero la base está dañada: {e}"))?;
    Ok((Core::new(conn), sealer, false))
}

fn create_new(password: String, import: Option<PathBuf>) -> Opened {
    let conn = match import {
        Some(path) => db::import_sqlite_file(&path)?,
        None => db::new_memory().map_err(|e| e.to_string())?,
    };
    let sealer = Sealer::new(&password).map_err(|e| e.to_string())?;
    let core = Core::new(conn);
    // Primer guardado inmediato: a partir de aquí ya existe el archivo cifrado.
    let plain = db::to_bytes(&core.conn).map_err(|e| e.to_string())?;
    let sealed = sealer.seal(&plain).map_err(|e| e.to_string())?;
    store::wipe(plain);
    store::write_atomic(&store::data_file(), &sealed).map_err(|e| e.to_string())?;
    Ok((core, sealer, true))
}

/// Pantalla de bienvenida/desbloqueo: columna centrada sobre negro.
fn shell(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let pal = p();
    egui::CentralPanel::default().frame(egui::Frame::new().fill(pal.bg)).show(ui, |ui| {
        let avail = ui.available_rect_before_wrap();
        let col_w = 420.0_f32.min(avail.width() - 40.0);
        let rect = egui::Rect::from_center_size(avail.center() - vec2(0.0, 30.0), vec2(col_w, 520.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.label(RichText::new("FinanZillo").font(theme::display(46.0)).color(pal.ink));
            ui.add_space(4.0);
            // Tira de códigos de color, como en las fundas.
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for c in ["#898781", "#2a78d6", "#eda100", "#e34948"] {
                    w::color_strip(ui, theme::cat_color(c), vec2(34.0, 10.0));
                }
                w::color_strip(ui, pal.ink, vec2(34.0, 10.0));
            });
            ui.add_space(28.0);
            add(ui);
        });
        let painter = ui.painter();
        painter.text(
            avail.center_bottom() - vec2(0.0, 18.0),
            Align2::CENTER_BOTTOM,
            store::data_file().display().to_string(),
            theme::mono(10.5),
            pal.muted,
        );
    });
}

fn pass_field(ui: &mut Ui, v: &mut String, hint: &str, focus: bool) -> egui::Response {
    let r = ui.add(
        egui::TextEdit::singleline(v)
            .password(true)
            .hint_text(RichText::new(hint).color(p().muted))
            .desired_width(f32::INFINITY)
            .font(theme::mono(15.0))
            .margin(egui::Margin::symmetric(12, 10)),
    );
    // Al abrir la pantalla el campo toma el foco durante los primeros frames,
    // aunque otro control lo haya cogido antes: así se puede escribir sin ratón.
    if focus {
        let key = egui::Id::new("fz_focus_frames");
        let n: u32 = ui.ctx().data(|d| d.get_temp(key)).unwrap_or(0);
        if n < 6 {
            r.request_focus();
            ui.ctx().data_mut(|d| d.insert_temp(key, n + 1));
        }
    }
    r
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // El espacio de trabajo puede cambiar el tema desde Ajustes.
        if let Phase::Open(ws) = &self.phase {
            self.dark = ws.dark;
        }
        if self.applied != Some(self.dark) {
            theme::apply(&ctx, self.dark);
            self.applied = Some(self.dark);
        }
        let pal = p();
        let mut next: Option<Phase> = None;
        #[cfg(debug_assertions)]
        if let Some(sh) = &mut self.shots {
            let at_start = sh.current() == Some("inicio");
            match &mut self.phase {
                Phase::Open(ws) => {
                    sh.setup(Some(ws));
                    sh.inject(&ctx, ws);
                    sh.dialogs(ws);
                    sh.click(ws, &ctx);
                    self.dark = ws.dark;
                    if sh.tick(&ctx) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                Phase::Setup { .. } | Phase::Locked { .. } if at_start => {
                    // Prueba de teclado: escribe sin ratón y comprueba que llega al campo.
                    if std::env::var("FZ_TYPE").is_ok() {
                        let n = ctx.cumulative_frame_nr();
                        if n == 8 {
                            ctx.input_mut(|i| i.events.push(egui::Event::Text("tecla".into())));
                        }
                        if n == 10 {
                            if let Phase::Locked { pass, .. } = &self.phase {
                                eprintln!("teclado → campo de contraseña: {} caracteres", pass.chars().count());
                            }
                        }
                    }
                    sh.tick(&ctx);
                }
                Phase::Setup { import, import_path, .. } => {
                    if let Ok(pw) = std::env::var("FZ_TEST_PASSWORD") {
                        let (tx, rx) = channel();
                        let imp = if *import { import_path.clone() } else { None };
                        std::thread::spawn(move || {
                            let _ = tx.send(create_new(pw, imp));
                        });
                        next = Some(Phase::Busy { rx, label: "Cifrando tus datos…".into() });
                    }
                }
                Phase::Locked { .. } => {
                    if let Ok(pw) = std::env::var("FZ_TEST_PASSWORD") {
                        let (tx, rx) = channel();
                        std::thread::spawn(move || {
                            let _ = tx.send(open_existing(pw));
                        });
                        next = Some(Phase::Busy { rx, label: "Descifrando…".into() });
                    }
                }
                Phase::Busy { .. } => ctx.request_repaint(),
            }
        }
        if let Some(n) = next.take() {
            self.phase = n;
        }
        match &mut self.phase {
            Phase::Setup { pass, pass2, import, import_path, error } => {
                shell(ui, |ui| {
                    ui.label(RichText::new("Crea la contraseña de tus datos").font(theme::wide(18.0)));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Todo se guarda en un único archivo cifrado. Sin esta contraseña nadie puede leerlo… ni siquiera tú: si la olvidas, los datos no se pueden recuperar.")
                            .color(pal.ink2),
                    );
                    ui.add_space(16.0);
                    w::label_caps(ui, "Contraseña (mínimo 8 caracteres)");
                    pass_field(ui, pass, "Contraseña", true);
                    ui.add_space(6.0);
                    w::label_caps(ui, "Repítela");
                    let r2 = pass_field(ui, pass2, "Repetir contraseña", false);
                    ui.add_space(12.0);
                    match import_path {
                        Some(path) => {
                            ui.checkbox(import, RichText::new("Importar esta base de datos de FinanZillo").color(pal.ink));
                            ui.label(RichText::new(path.display().to_string()).font(theme::mono(11.0)).color(pal.muted));
                        }
                        None => {
                            ui.label(RichText::new("Empezarás con una base vacía. Si ya usabas FinanZillo, puedes importar tu base de datos (.db).").color(pal.muted));
                        }
                    }
                    if w::button(ui, Some(egui_phosphor::light::FOLDER_OPEN), "Importar una base de datos…", Kind::Ghost).clicked() {
                        if let Some(f) = rfd::FileDialog::new().add_filter("Base de datos SQLite", &["db", "sqlite"]).pick_file() {
                            *import_path = Some(f);
                            *import = true;
                        }
                    }
                    ui.add_space(14.0);
                    if !error.is_empty() {
                        ui.label(RichText::new(error.as_str()).color(pal.critical));
                        ui.add_space(6.0);
                    }
                    let go = w::button_ex(ui, Some(egui_phosphor::light::LOCK_KEY), "Crear archivo cifrado", Kind::Primary, true, true).clicked()
                        || (r2.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                    if go {
                        if pass.chars().count() < 8 {
                            *error = "La contraseña debe tener al menos 8 caracteres".into();
                        } else if pass != pass2 {
                            *error = "Las contraseñas no coinciden".into();
                        } else {
                            let (tx, rx) = channel();
                            let pw = std::mem::take(pass);
                            pass2.clear();
                            let imp = if *import { import_path.clone() } else { None };
                            let ctx2 = ctx.clone();
                            std::thread::spawn(move || {
                                let _ = tx.send(create_new(pw, imp));
                                ctx2.request_repaint();
                            });
                            next = Some(Phase::Busy { rx, label: "Cifrando tus datos…".into() });
                        }
                    }
                });
            }
            Phase::Locked { pass, error, attempts, hello_tried } => {
                let hello_on = crate::hello::enabled();
                // Con el PIN activado se pide nada más bloquear o abrir (una vez).
                let test_mode = std::env::var("FZ_TEST_PASSWORD").is_ok();
                let mut use_hello = hello_on && !*hello_tried && !test_mode;
                shell(ui, |ui| {
                    ui.label(RichText::new(if hello_on { "Desbloquea FinanZillo" } else { "Introduce tu contraseña" }).font(theme::wide(18.0)));
                    ui.add_space(14.0);
                    if hello_on {
                        if w::button_ex(ui, Some(egui_phosphor::light::FINGERPRINT), "Usar el PIN de Windows", Kind::Primary, true, true).clicked() {
                            use_hello = true;
                        }
                        ui.add_space(10.0);
                        ui.label(RichText::new("o escribe la contraseña").color(pal.muted));
                        ui.add_space(4.0);
                    }
                    let r = pass_field(ui, pass, "Contraseña", true);
                    ui.add_space(10.0);
                    if !error.is_empty() {
                        ui.label(RichText::new(error.as_str()).color(pal.critical));
                        ui.add_space(6.0);
                    }
                    let kind = if hello_on { Kind::Secondary } else { Kind::Primary };
                    let go = w::button_ex(ui, Some(egui_phosphor::light::LOCK_KEY_OPEN), "Abrir", kind, true, !pass.is_empty()).clicked()
                        || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !pass.is_empty());
                    if go {
                        let (tx, rx) = channel();
                        let pw = std::mem::take(pass);
                        let ctx2 = ctx.clone();
                        // Pequeña espera creciente tras fallos repetidos.
                        let delay = std::time::Duration::from_millis(250 * (*attempts as u64).min(8));
                        std::thread::spawn(move || {
                            std::thread::sleep(delay);
                            let _ = tx.send(open_existing(pw));
                            ctx2.request_repaint();
                        });
                        *attempts += 1;
                        next = Some(Phase::Busy { rx, label: "Descifrando…".into() });
                    }
                });
                if use_hello && next.is_none() {
                    *hello_tried = true;
                    let (tx, rx) = channel();
                    let ctx2 = ctx.clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(open_with_hello());
                        ctx2.request_repaint();
                    });
                    next = Some(Phase::Busy { rx, label: "Confirma con tu PIN de Windows…".into() });
                }
            }
            Phase::Busy { rx, label } => {
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("fz_focus_frames"), 0u32));
                let label = label.clone();
                shell(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new(label).color(pal.ink2));
                    });
                });
                match rx.try_recv() {
                    Ok(Ok((core, sealer, fresh))) => {
                        let mut ws = Workspace::new(core, sealer, self.dark);
                        if fresh {
                            ws.toast("Archivo cifrado creado. A partir de ahora todo se guarda solo.", false);
                        }
                        next = Some(Phase::Open(Box::new(ws)));
                    }
                    Ok(Err(e)) => {
                        #[cfg(debug_assertions)]
                        eprintln!("apertura fallida: {e}");
                        let attempts = 1;
                        next = Some(if store::data_file().exists() {
                            Phase::Locked { pass: String::new(), error: e, attempts, hello_tried: true }
                        } else {
                            let found = db::guess_web_db();
                            Phase::Setup { pass: String::new(), pass2: String::new(), import: found.is_some(), import_path: found, error: e }
                        });
                    }
                    Err(_) => ctx.request_repaint_after(std::time::Duration::from_millis(60)),
                }
            }
            Phase::Open(ws) => {
                if ws.ui(ui) {
                    // Bloquear: guardar y volver a pedir la contraseña.
                    ws.save_now();
                    // Al bloquear no salta el PIN solo (podrías no estar delante): botón.
                    next = Some(Phase::Locked { pass: String::new(), error: String::new(), attempts: 0, hello_tried: true });
                }
            }
        }
        if let Some(n) = next {
            self.phase = n;
        }
        let title = match &self.phase {
            Phase::Open(ws) => format!("FinanZillo — {}", util::month_label(&ws.month)),
            _ => "FinanZillo".into(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("tema", if self.dark { "oscuro".into() } else { "claro".into() });
        if let Phase::Open(ws) = &mut self.phase {
            ws.save_now();
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Phase::Open(ws) = &mut self.phase {
            ws.save_now();
        }
    }

    /// Solo se guarda la ventana y el tema; nada del estado de la interfaz
    /// (que podría reflejar datos) sale del archivo cifrado.
    fn persist_egui_memory(&self) -> bool {
        false
    }

    /// Solo en depuración: el guion de teclado de las pruebas entra como
    /// entrada cruda, igual que un teclado real (así el Tab mueve el foco).
    #[cfg(debug_assertions)]
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(sh) = &mut self.shots {
            // En las capturas se ignora el ratón real (sin tooltips ni filas
            // resaltadas por dónde estuviera el cursor), salvo en «mes:clic».
            if sh.current() != Some("mes:clic") {
                raw.events.retain(|e| !matches!(e, egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. }));
                raw.events.push(egui::Event::PointerGone);
            }
            // Los modificadores llegan como evento: se pulsan antes de la tecla
            // y se sueltan en el frame siguiente.
            if sh.mods_down {
                raw.events.push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
                sh.mods_down = false;
            }
            if !sh.pending.is_empty() && sh.pending_mods != egui::Modifiers::NONE {
                raw.events.push(egui::Event::ModifiersChanged(sh.pending_mods));
                sh.pending_mods = egui::Modifiers::NONE;
                sh.mods_down = true;
            }
            raw.events.append(&mut sh.pending);
        }
    }

    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        let c = theme::palette(self.dark).bg;
        [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, 1.0]
    }
}
