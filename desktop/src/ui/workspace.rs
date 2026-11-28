//! Espacio de trabajo abierto: estado, caché de cálculos, guardado
//! automático, barra lateral, atajos y diálogos.

use std::sync::mpsc::Receiver;

use egui::{Align2, CornerRadius, Key, Modifiers, RichText, Sense, Stroke, Ui, pos2, vec2};
use egui_phosphor::light as ic;

use super::forms::Modal;
use super::theme::{self, p};
use super::views;
use super::widgets::{self as w, Kind, Toast};
use crate::ledger::accounts::{Account, NetPoint};
use crate::ledger::analysis::{MonthTotals, PulseLine, Summary};
use crate::ledger::investments::{Portfolio, PriceResult, SymbolHit};
use crate::ledger::{Category, Core, Template};
use crate::store::{self, Sealer};
use crate::{db, util};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Mes,
    Movimientos,
    Cuentas,
    Presupuestos,
    Recurrentes,
    Inversiones,
    Objetivos,
    Anual,
    Informe,
    Ajustes,
}

pub const NAV: [(View, &str, &str); 10] = [
    (View::Mes, "Mes", ic::CALENDAR_BLANK),
    (View::Movimientos, "Movimientos", ic::LIST_BULLETS),
    (View::Cuentas, "Cuentas", ic::BANK),
    (View::Presupuestos, "Presupuestos", ic::CHART_PIE_SLICE),
    (View::Recurrentes, "Recurrentes", ic::REPEAT),
    (View::Inversiones, "Inversiones", ic::CHART_LINE_UP),
    (View::Objetivos, "Objetivos", ic::TARGET),
    (View::Anual, "Año", ic::CALENDAR),
    (View::Informe, "Informe", ic::FILE_TEXT),
    (View::Ajustes, "Ajustes", ic::GEAR_SIX),
];

/// Cálculos que solo se rehacen cuando cambian los datos o el mes.
#[derive(Default)]
pub struct Cache {
    pub key: (u64, String),
    pub summary: Summary,
    pub pulse: Vec<PulseLine>,
    pub series: Vec<MonthTotals>,
    pub accounts: Vec<Account>,
    pub portfolio: Portfolio,
    pub categories: Vec<Category>,
    pub templates: Vec<Template>,
    pub networth: Option<(u64, Vec<NetPoint>)>,
}

pub struct Workspace {
    pub core: Core,
    sealer: Sealer,
    saved_version: u64,
    seen_version: u64,
    last_change: f64,
    pub last_saved: Option<String>,
    pub save_error: Option<String>,
    pub view: View,
    pub month: String,
    pub privacy: bool,
    pub dark: bool,
    pub cache: Cache,
    pub modal: Modal,
    pub toasts: Vec<Toast>,
    pub now: f64,
    pub views: views::ViewStates,
    pub prices_rx: Option<Receiver<Vec<PriceResult>>>,
    pub prices_busy: bool,
    pub search_rx: Option<Receiver<Vec<SymbolHit>>>,
    /// Activación del PIN de Windows en curso (pide el PIN en otro hilo).
    pub hello_rx: Option<Receiver<Result<(), String>>>,
    lock_requested: bool,
    last_input: f64,
    last_pending: f64,
    show_help: bool,
}

impl Workspace {
    pub fn new(mut core: Core, sealer: Sealer, dark: bool) -> Self {
        core.apply_pending();
        let v = core.version;
        let mut ws = Workspace {
            core,
            sealer,
            saved_version: 0,
            seen_version: v,
            last_change: 0.0,
            last_saved: None,
            save_error: None,
            view: View::Mes,
            month: util::current_month(),
            privacy: false,
            dark,
            cache: Cache::default(),
            modal: Modal::None,
            toasts: Vec::new(),
            now: 0.0,
            views: views::ViewStates::default(),
            prices_rx: None,
            prices_busy: false,
            search_rx: None,
            hello_rx: None,
            lock_requested: false,
            last_input: 0.0,
            last_pending: 0.0,
            show_help: false,
        };
        ws.dark = ws.core.setting("tema").map(|t| t != "claro").unwrap_or(dark);
        // Guardado inicial (puede haber recurrentes o intereses recién apuntados).
        ws.save_now();
        ws.refresh_prices(false);
        ws
    }

    pub fn toast(&mut self, text: impl Into<String>, error: bool) {
        self.toasts.push(Toast { text: text.into(), error, until: self.now + if error { 6.0 } else { 3.5 }, undo_tx: None });
    }

    pub fn toast_undo(&mut self, text: impl Into<String>, tx_id: i64) {
        self.toasts.push(Toast { text: text.into(), error: false, until: self.now + 6.0, undo_tx: Some(tx_id) });
    }

    /// Resultado de una operación: muestra el error si lo hay.
    pub fn report<T>(&mut self, r: Result<T, String>, ok_msg: Option<&str>) -> Option<T> {
        match r {
            Ok(v) => {
                if let Some(m) = ok_msg {
                    self.toast(m, false);
                }
                Some(v)
            }
            Err(e) => {
                self.toast(e, true);
                None
            }
        }
    }

    // ---------- guardado ----------

    pub fn save_now(&mut self) {
        if self.core.version == self.saved_version {
            return;
        }
        let result = (|| -> Result<(), String> {
            let plain = db::to_bytes(&self.core.conn).map_err(|e| e.to_string())?;
            let sealed = self.sealer.seal(&plain).map_err(|e| e.to_string())?;
            store::wipe(plain);
            store::write_atomic(&store::data_file(), &sealed).map_err(|e| e.to_string())?;
            let ext = self.core.setting("backup_dir").filter(|s| !s.trim().is_empty()).map(std::path::PathBuf::from);
            let _ = store::daily_backup(&sealed, ext.as_deref(), 30);
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.saved_version = self.core.version;
                self.save_error = None;
                self.last_saved = Some(chrono::Local::now().format("%H:%M").to_string());
            }
            Err(e) => {
                if self.save_error.as_deref() != Some(e.as_str()) {
                    self.toast(format!("No se pudo guardar: {e}"), true);
                }
                self.save_error = Some(e);
            }
        }
    }

    pub fn change_password(&mut self, new: &str) -> Result<(), String> {
        if new.chars().count() < 8 {
            return Err("La contraseña debe tener al menos 8 caracteres".into());
        }
        self.sealer = Sealer::new(new).map_err(|e| e.to_string())?;
        self.saved_version = 0;
        self.save_now();
        // La clave cambió: el PIN guardaba la anterior y ya no sirve.
        if crate::hello::enabled() {
            crate::hello::disable();
            self.toast("El desbloqueo con PIN se ha desactivado al cambiar la contraseña: vuelve a activarlo en Ajustes", false);
        }
        if self.save_error.is_some() { Err("No se pudo guardar con la contraseña nueva".into()) } else { Ok(()) }
    }

    pub fn verify_password(&self, pw: &str) -> bool {
        std::fs::read(store::data_file()).map(|b| Sealer::open(&b, pw).is_ok()).unwrap_or(false)
    }

    // ---------- caché ----------

    pub fn refresh_cache(&mut self) {
        let key = (self.core.version, self.month.clone());
        if self.cache.key == key {
            return;
        }
        let c = &self.core;
        self.cache.summary = c.summary(&self.month);
        self.cache.pulse = c.pulse(&self.month);
        self.cache.series = c.monthly_series(&self.month);
        self.cache.accounts = c.accounts();
        self.cache.portfolio = c.portfolio();
        self.cache.categories = c.categories(true);
        let slots: Vec<(i64, String, String, i64)> =
            self.cache.categories.iter().map(|k| (k.id, k.color.clone(), k.ty.clone(), if k.archived { -1 } else { k.usage_count })).collect();
        theme::assign_category_slots(&slots);
        self.cache.templates = c.templates();
        self.cache.key = key;
    }

    pub fn networth(&mut self) -> &[NetPoint] {
        let v = self.core.version;
        if self.cache.networth.as_ref().is_none_or(|(ver, _)| *ver != v) {
            self.cache.networth = Some((v, self.core.networth()));
        }
        &self.cache.networth.as_ref().unwrap().1
    }

    pub fn account_names(&self) -> Vec<String> {
        self.cache.accounts.iter().map(|a| a.name.clone()).collect()
    }

    pub fn active_categories(&self, ty: &str) -> Vec<(Option<i64>, String)> {
        self.cache.categories.iter().filter(|c| c.ty == ty && !c.archived).map(|c| (Some(c.id), c.name.clone())).collect()
    }

    // ---------- precios en segundo plano ----------

    pub fn refresh_prices(&mut self, force: bool) {
        if self.prices_busy || (!force && !self.core.prices_stale()) {
            return;
        }
        let req = self.core.price_requests();
        if req.is_empty() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::ledger::investments::fetch_prices(req));
        });
        self.prices_rx = Some(rx);
        self.prices_busy = true;
    }

    /// Activa el desbloqueo con el PIN de Windows (pide el PIN).
    pub fn enable_hello(&mut self) {
        if self.hello_rx.is_some() {
            return;
        }
        let material = self.sealer.key_material();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::hello::enable(material));
        });
        self.hello_rx = Some(rx);
    }

    fn poll_background(&mut self) {
        if let Some(rx) = &self.hello_rx {
            if let Ok(r) = rx.try_recv() {
                self.hello_rx = None;
                match r {
                    Ok(()) => self.toast("Listo: la próxima vez podrás entrar con el PIN de Windows", false),
                    Err(e) => self.toast(e, true),
                }
            }
        }
        if let Some(rx) = &self.prices_rx {
            if let Ok(res) = rx.try_recv() {
                let errors = self.core.store_prices(&res);
                self.prices_rx = None;
                self.prices_busy = false;
                if !errors.is_empty() {
                    self.toast(format!("Precios sin actualizar: {}", errors.join(" · ")), true);
                }
            }
        }
    }

    // ---------- interfaz ----------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let typing = ctx.memory(|m| m.focused().is_some());
        let modal_open = !matches!(self.modal, Modal::None);
        let mut go: Option<View> = None;
        ctx.input_mut(|i| {
            if (i.consume_key(Modifiers::COMMAND, Key::N) || i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::N)) && !modal_open {
                self.modal = Modal::new_tx(&self.core);
            }
            if i.consume_key(Modifiers::COMMAND, Key::T) {
                self.modal = Modal::new_transfer();
            }
            if i.consume_key(Modifiers::COMMAND, Key::S) {
                self.saved_version = 0;
                self.save_now();
                self.toast("Guardado", false);
            }
            if i.consume_key(Modifiers::COMMAND, Key::H) {
                self.privacy = !self.privacy;
            }
            if i.consume_key(Modifiers::COMMAND, Key::L) {
                self.lock_requested = true;
            }
            if i.consume_key(Modifiers::COMMAND, Key::F) {
                go = Some(View::Movimientos);
                self.views.mov.focus_search = true;
            }
            if i.consume_key(Modifiers::NONE, Key::F1) {
                self.show_help = !self.show_help;
            }
            let nums = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9, Key::Num0];
            for (k, (v, _, _)) in nums.iter().zip(NAV.iter()) {
                if i.consume_key(Modifiers::COMMAND, *k) {
                    go = Some(*v);
                }
            }
            if !typing && !modal_open {
                if i.consume_key(Modifiers::ALT, Key::ArrowLeft) || i.consume_key(Modifiers::COMMAND, Key::PageUp) {
                    self.month = util::add_months(&self.month, -1);
                }
                if i.consume_key(Modifiers::ALT, Key::ArrowRight) || i.consume_key(Modifiers::COMMAND, Key::PageDown) {
                    self.month = util::add_months(&self.month, 1);
                }
            }
        });
        if let Some(v) = go {
            self.view = v;
        }
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        let pal = p();
        egui::Panel::left("nav")
            .exact_size(212.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(pal.bg).inner_margin(egui::Margin { left: 14, right: 12, top: 20, bottom: 14 }).stroke(Stroke::NONE))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                ui.painter().vline(rect.right() + 12.0, rect.y_range().expand(40.0), Stroke::new(1.0, pal.line));
                ui.label(RichText::new("FinanZillo").font(theme::display(19.0)).color(pal.ink));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    for c in ["#898781", "#2a78d6", "#eda100", "#e34948"] {
                        w::color_strip(ui, theme::cat_color(c), vec2(22.0, 6.0));
                    }
                    w::color_strip(ui, pal.ink, vec2(22.0, 6.0));
                });
                ui.add_space(6.0);
                let (y, m) = util::month_parts(&self.month);
                ui.label(RichText::new(format!("FZ {y}·{m:02}")).font(theme::mono(11.0)).color(pal.muted).extra_letter_spacing(1.5));
                ui.add_space(22.0);
                for (i, (v, label, icon)) in NAV.iter().enumerate() {
                    let sel = self.view == *v;
                    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
                    let hov = resp.hovered();
                    if sel || hov {
                        ui.painter().rect_filled(rect, w::R2, if sel { pal.surface2 } else { pal.surface });
                    }
                    if sel {
                        let b = egui::Rect::from_min_max(pos2(rect.right() - 5.0, rect.top()), rect.right_bottom());
                        ui.painter().rect_filled(b, CornerRadius::ZERO, pal.signal);
                    }
                    let col = if sel { pal.ink } else if hov { pal.ink } else { pal.ink2 };
                    ui.painter().text(pos2(rect.left() + 18.0, rect.center().y), Align2::CENTER_CENTER, *icon, theme::icon(18.0), col);
                    ui.painter().text(pos2(rect.left() + 36.0, rect.center().y), Align2::LEFT_CENTER, *label, theme::medium(14.0), col);
                    let num = if i == 9 { "0".to_string() } else { (i + 1).to_string() };
                    if hov {
                        ui.painter().text(pos2(rect.right() - 14.0, rect.center().y), Align2::RIGHT_CENTER, format!("Ctrl+{num}"), theme::mono(10.0), pal.muted);
                    }
                    if resp.clicked() {
                        self.view = *v;
                    }
                    resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    let state = match (&self.save_error, &self.last_saved) {
                        (Some(_), _) => RichText::new("Sin guardar · reintentando").color(pal.critical),
                        (None, Some(t)) if self.core.version == self.saved_version => RichText::new(format!("Cifrado · guardado {t}")).color(pal.muted),
                        _ => RichText::new("Cifrado · guardando…").color(pal.muted),
                    };
                    ui.label(state.font(theme::mono(10.5)));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let eye = if self.privacy { ic::EYE_SLASH } else { ic::EYE };
                        if w::icon_button(ui, eye, "Ocultar importes (Ctrl+H)").clicked() {
                            self.privacy = !self.privacy;
                        }
                        let th = if self.dark { ic::SUN } else { ic::MOON };
                        if w::icon_button(ui, th, "Cambiar tema claro/oscuro").clicked() {
                            self.dark = !self.dark;
                            let v = if self.dark { "oscuro" } else { "claro" };
                            let _ = self.core.set_setting("tema", v);
                        }
                        if w::icon_button(ui, ic::LOCK_SIMPLE, "Bloquear (Ctrl+L)").clicked() {
                            self.lock_requested = true;
                        }
                        if w::icon_button(ui, ic::KEYBOARD, "Atajos de teclado (F1)").clicked() {
                            self.show_help = !self.show_help;
                        }
                    });
                });
            });
    }

    fn help_window(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }
        let pal = p();
        let mut open = true;
        egui::Window::new(RichText::new("Atajos de teclado").font(theme::wide(15.0)))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                egui::Grid::new("atajos").num_columns(2).spacing([28.0, 8.0]).show(ui, |ui| {
                    for (k, d) in [
                        ("Ctrl+N", "Nuevo movimiento (Ctrl+Enter guarda)"),
                        ("Ctrl+T", "Transferencia entre cuentas"),
                        ("Ctrl+F", "Buscar movimientos"),
                        ("Alt+← / Alt+→", "Mes anterior / siguiente"),
                        ("Ctrl+1 … Ctrl+0", "Ir a cada sección"),
                        ("Ctrl+H", "Ocultar o mostrar importes"),
                        ("Ctrl+S", "Guardar ahora (también se guarda solo)"),
                        ("Ctrl+L", "Bloquear con contraseña"),
                        ("Esc", "Cerrar el diálogo"),
                        ("F1", "Esta ayuda"),
                    ] {
                        ui.label(RichText::new(k).font(theme::mono(13.0)).color(pal.ink));
                        ui.label(RichText::new(d).color(pal.ink2));
                        ui.end_row();
                    }
                });
            });
        if !open || ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.show_help = false;
        }
    }

    /// Dibuja todo. Devuelve true si hay que bloquear.
    pub fn ui(&mut self, ui: &mut Ui) -> bool {
        let ctx = ui.ctx().clone();
        self.now = ctx.input(|i| i.time);
        if ctx.input(|i| !i.events.is_empty() || i.pointer.is_moving()) {
            self.last_input = self.now;
        }
        self.poll_background();
        if self.hello_rx.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
        self.shortcuts(&ctx);

        // Cambios de datos → programar guardado (pequeña espera para agrupar).
        if self.core.version != self.seen_version {
            self.seen_version = self.core.version;
            self.last_change = self.now;
        }
        if self.core.version != self.saved_version && self.now - self.last_change > 0.8 {
            self.save_now();
        }
        if self.core.version != self.saved_version {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        }
        // Al cambiar de día (app abierta de un día para otro) entran recurrentes e intereses.
        if self.now - self.last_pending > 60.0 {
            self.last_pending = self.now;
            if self.core.apply_pending() > 0 {
                self.toast("Movimientos recurrentes e intereses al día", false);
            }
        }
        // Autobloqueo por inactividad.
        let mins: f64 = self.core.setting("autolock_min").and_then(|v| v.parse().ok()).unwrap_or(15.0);
        if mins > 0.0 && self.last_input > 0.0 && self.now - self.last_input > mins * 60.0 {
            self.lock_requested = true;
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(30));

        self.refresh_cache();
        self.sidebar(ui);
        let pal = p();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal.bg).inner_margin(egui::Margin { left: 30, right: 28, top: 20, bottom: 0 })).show(ui, |ui| {
            views::show(self, ui);
        });

        super::forms::show(self, &ctx);
        self.help_window(&ctx);
        if let Some(id) = w::show_toasts(&ctx, &mut self.toasts, self.now) {
            if self.core.delete_tx(id).is_ok() {
                self.toast("Apunte deshecho", false);
            }
        }
        let lock = self.lock_requested;
        self.lock_requested = false;
        lock
    }
}

/// Encabezado de sección: título + navegación de mes opcional.
pub fn header(ui: &mut Ui, title: &str, month: Option<&mut String>) -> bool {
    let pal = p();
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(theme::wide(22.0)).color(pal.ink));
        if let Some(m) = month {
            ui.add_space(12.0);
            if w::icon_button(ui, ic::CARET_LEFT, "Mes anterior (Alt+←)").clicked() {
                *m = util::add_months(m, -1);
                changed = true;
            }
            ui.label(RichText::new(util::cap(&util::month_label(m))).font(theme::medium(15.0)).color(pal.ink2));
            if w::icon_button(ui, ic::CARET_RIGHT, "Mes siguiente (Alt+→)").clicked() {
                *m = util::add_months(m, 1);
                changed = true;
            }
            if *m != util::current_month() && w::button(ui, None, "Hoy", Kind::Ghost).clicked() {
                *m = util::current_month();
                changed = true;
            }
        }
    });
    ui.add_space(6.0);
    changed
}

/// Título del mes con flechas (cabecera de la vista Mes).
pub fn header_inline(ui: &mut Ui, m: &mut String) -> bool {
    let pal = p();
    let mut changed = false;
    ui.label(RichText::new(util::cap(&util::month_label(m))).font(theme::wide(24.0)).color(pal.ink));
    ui.add_space(6.0);
    if w::icon_button(ui, ic::CARET_LEFT, "Mes anterior (Alt+←)").clicked() {
        *m = util::add_months(m, -1);
        changed = true;
    }
    if w::icon_button(ui, ic::CARET_RIGHT, "Mes siguiente (Alt+→)").clicked() {
        *m = util::add_months(m, 1);
        changed = true;
    }
    if *m != util::current_month() && w::button(ui, None, "Volver a hoy", Kind::Ghost).clicked() {
        *m = util::current_month();
        changed = true;
    }
    changed
}
