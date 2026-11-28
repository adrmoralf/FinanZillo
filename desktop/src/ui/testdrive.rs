//! Solo en compilación de depuración: recorre vistas y guarda capturas.
//! FZ_TEST_PASSWORD  contraseña de prueba (abre o crea el archivo)
//! FZ_SHOTS          lista: inicio,mes,movimientos,…,mes:claro,mes:nuevo
//! FZ_SHOT_DIR       carpeta de salida de los PNG

use super::forms::Modal;
use super::workspace::{View, Workspace};

pub struct Shots {
    list: Vec<String>,
    idx: usize,
    wait: u32,
    requested: bool,
    script_frame: u32,
    step_start: Option<f64>,
    /// Eventos de teclado pendientes: se entregan en la entrada cruda del frame siguiente.
    pub pending: Vec<egui::Event>,
    /// Modificadores a mantener pulsados en el frame en que se entregan los eventos.
    pub pending_mods: egui::Modifiers,
    pub mods_down: bool,
    dlg: Vec<String>,
    dir: std::path::PathBuf,
    pub done: bool,
}

impl Shots {
    pub fn from_env() -> Option<Shots> {
        let list: Vec<String> = std::env::var("FZ_SHOTS").ok()?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let dir = std::path::PathBuf::from(std::env::var("FZ_SHOT_DIR").ok()?);
        let _ = std::fs::create_dir_all(&dir);
        Some(Shots { list, idx: 0, wait: 0, requested: false, script_frame: 0, step_start: None, pending: Vec::new(), pending_mods: egui::Modifiers::NONE, mods_down: false, dlg: Vec::new(), dir, done: false })
    }

    pub fn current(&self) -> Option<&str> {
        self.list.get(self.idx).map(String::as_str)
    }

    fn view_of(name: &str) -> Option<View> {
        Some(match name {
            "mes" => View::Mes,
            "movimientos" => View::Movimientos,
            "cuentas" => View::Cuentas,
            "presupuestos" => View::Presupuestos,
            "recurrentes" => View::Recurrentes,
            "inversiones" => View::Inversiones,
            "objetivos" => View::Objetivos,
            "anual" => View::Anual,
            "informe" => View::Informe,
            "ajustes" => View::Ajustes,
            _ => return None,
        })
    }

    /// Prepara el espacio de trabajo para la captura actual (una vez por paso).
    pub fn setup(&mut self, ws: Option<&mut Workspace>) {
        if self.wait != 0 {
            return;
        }
        let Some(name) = self.current().map(str::to_string) else { return };
        if let Some(ws) = ws {
            let (base, opt) = name.split_once(':').unwrap_or((&name, ""));
            if let Some(v) = Self::view_of(base) {
                ws.view = v;
            }
            ws.dark = opt != "claro";
            ws.modal = match opt {
                "nuevo" | "apunte" => Modal::new_tx(&ws.core),
                "transferencia" => Modal::new_transfer(),
                "editar" => ws.core.recent(1).first().map(Modal::edit_tx).unwrap_or(Modal::None),
                "ficha" => ws.core.resolve_category("Ocio", "gasto").map(|id| Modal::CategoryDetail { id, month: ws.month.clone() }).unwrap_or(Modal::None),
                "cuenta" => ws.cache.accounts.iter().find(|a| a.apy > 0.0).or(ws.cache.accounts.first()).map(Modal::edit_account).unwrap_or(Modal::None),
                _ => Modal::None,
            };
            ws.toasts.clear();
            if opt == "apunte" {
                // Diálogo nuevo con datos: importe con cuenta y concepto conocido.
                if let Modal::Tx(f) = &mut ws.modal {
                    f.amount = "2,50+1".into();
                    f.note = "Café".into();
                    f.focus = false;
                }
            }
            if opt == "pdf" {
                let out = self.dir.join("informe.pdf");
                let rep = ws.core.report(&ws.month);
                let _ = std::fs::write(self.dir.join("informe.html"), rep.html());
                match super::views::informe::html_to_pdf(&rep.html(), &out) {
                    Ok(()) => eprintln!("pdf ok: {} bytes", std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0)),
                    Err(e) => eprintln!("pdf fallo: {e}"),
                }
            }
        }
    }

    /// Guion de teclado («mes:teclado»): Ctrl+N, «3,20», Tab, concepto, Tab,
    /// «oci», Enter, Ctrl+Enter. Se inyecta al principio del frame, antes que los widgets.
    pub fn inject(&mut self, ctx: &egui::Context, ws: &mut Workspace) {
        if self.current() != Some("mes:teclado") {
            return;
        }
        let key = |k: egui::Key| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
        let f = self.script_frame;
        self.script_frame += 1;
        let ev: Option<egui::Event> = match f {
            2 => {
                self.pending_mods = egui::Modifiers::COMMAND;
                Some(egui::Event::Key { key: egui::Key::N, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND })
            }
            5 => Some(egui::Event::Text("3,20".into())),
            7 => Some(key(egui::Key::Tab)),
            9 => Some(egui::Event::Text("Prueba teclado".into())),
            11 => Some(key(egui::Key::Tab)),
            13 => Some(egui::Event::Text("oci".into())),
            15 => Some(key(egui::Key::Enter)),
            17 => {
                self.pending_mods = egui::Modifiers::COMMAND;
                Some(egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND })
            }
            20 => {
                let t = ws.core.recent(1);
                match t.first() {
                    Some(t) if t.note == "Prueba teclado" => eprintln!(
                        "teclado → apunte creado: «{}» {} € categoría «{}» cuenta «{}»",
                        t.note,
                        crate::util::fmt_num(t.amount_cents),
                        t.category_name,
                        t.account
                    ),
                    _ => eprintln!("teclado → NO se creó el apunte"),
                }
                None
            }
            _ => None,
        };
        if let Some(e) = ev {
            self.pending.push(e);
            ctx.request_repaint();
        }
        if f < 24 {
            self.wait = 1; // no capturar hasta terminar el guion
        }
    }


    /// Recorrido por los diálogos («dialogos»): cada paso abre un diálogo con
    /// datos, pulsa Ctrl+Enter (o Esc) por la entrada cruda y comprueba en la
    /// base que se aplicó y que el diálogo se cerró.
    pub fn dialogs(&mut self, ws: &mut Workspace) {
        use super::forms::*;
        if self.current() != Some("dialogos") {
            return;
        }
        const STEP: u32 = 10;
        let f = self.script_frame;
        self.script_frame += 1;
        let n = (f / STEP) as usize;
        let phase = f % STEP;
        let total = 12usize;
        if n >= total {
            if phase == 0 && n == total {
                let ok = self.dlg.iter().filter(|l| l.starts_with("OK")).count();
                eprintln!("diálogos: {ok}/{} pasos correctos", self.dlg.len());
                for l in &self.dlg {
                    eprintln!("  {l}");
                }
            }
            return;
        }
        self.wait = 1;
        let core = &ws.core;
        let ctrl_enter = egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND };
        let esc = egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
        let cuenta = core.default_account();
        let ocio = core.resolve_category("Ocio", "gasto");
        if phase == 0 {
            ws.modal = match n {
                0 => {
                    let t = core.recent(1).into_iter().next().expect("hay movimientos");
                    let mut m = Modal::edit_tx(&t);
                    if let Modal::Tx(f) = &mut m {
                        f.amount = "4,10".into();
                        f.note = "Editado por la prueba".into();
                        f.focus = false;
                    }
                    m
                }
                1 => {
                    let mut m = Modal::new_transfer();
                    if let Modal::Tx(f) = &mut m {
                        f.amount = "5".into();
                        f.from = cuenta.clone();
                        f.to = "Efectivo".into();
                        f.note = "Transferencia de prueba".into();
                        f.focus = false;
                    }
                    m
                }
                2 => {
                    let a = core.accounts().into_iter().find(|a| a.name == "Efectivo").expect("cuenta Efectivo");
                    let mut m = Modal::edit_account(&a);
                    if let Modal::Account(f) = &mut m {
                        f.apy = "2,5".into();
                    }
                    m
                }
                3 => {
                    let c = core.category(ocio.unwrap()).unwrap();
                    Modal::Category(CategoryForm { id: Some(c.id), name: c.name, ty: 0, icon: c.icon, color: c.color, budget: "150".into(), fixed: false, error: String::new() })
                }
                4 => Modal::Category(CategoryForm {
                    id: None,
                    name: "Categoría de prueba".into(),
                    ty: 0,
                    icon: "🧪".into(),
                    color: "#2a78d6".into(),
                    budget: String::new(),
                    fixed: true,
                    error: String::new(),
                }),
                5 => Modal::Recurring(RecurringForm {
                    id: None,
                    active: true,
                    kind: 0,
                    amount: "9,99".into(),
                    note: "Recurrente de prueba".into(),
                    cat: ocio,
                    account: cuenta.clone(),
                    from: String::new(),
                    to: String::new(),
                    inv: None,
                    day: "28".into(),
                    error: String::new(),
                }),
                6 => {
                    let inv = core.portfolio().items.into_iter().next().expect("hay una inversión");
                    Modal::Ops(OpsForm { inv_id: inv.id, name: inv.name, ty: 0, units: "0,5".into(), amount: "50".into(), date: crate::util::today_str(), error: String::new() })
                }
                7 => Modal::Goal(GoalForm { id: None, name: "Objetivo de prueba".into(), icon: "🎯".into(), target: "1000".into(), deadline: "2027-06-30".into(), error: String::new() }),
                8 => {
                    let g = core.goals().into_iter().find(|g| g.name == "Objetivo de prueba").expect("objetivo creado");
                    Modal::GoalAdd(GoalAddForm { id: g.id, name: g.name, amount: "120".into(), withdraw: false, error: String::new() })
                }
                9 => Modal::Template(TemplateForm { name: "Metro".into(), amount: "1,50".into(), cat: None, account: cuenta.clone(), error: String::new() }),
                10 => {
                    // Esc cierra sin guardar.
                    let mut m = Modal::new_tx(core);
                    if let Modal::Tx(f) = &mut m {
                        f.amount = "77".into();
                        f.note = "No debe guardarse".into();
                        f.focus = false;
                    }
                    m
                }
                _ => {
                    // Importe vacío: muestra el error y el diálogo sigue abierto.
                    let mut m = Modal::new_tx(core);
                    if let Modal::Tx(f) = &mut m {
                        f.note = "Sin importe".into();
                        f.focus = false;
                    }
                    m
                }
            };
        } else if phase == 3 {
            self.pending.push(if n == 10 { esc } else { ctrl_enter });
            self.pending_mods = if n == 10 { egui::Modifiers::NONE } else { egui::Modifiers::COMMAND };
        } else if phase == 7 {
            let closed = matches!(ws.modal, Modal::None);
            let (name, ok, detail): (&str, bool, String) = match n {
                0 => {
                    let t = core.recent(1).into_iter().next().unwrap();
                    ("editar movimiento", closed && t.amount_cents == 410 && t.note == "Editado por la prueba", format!("{} {}", t.note, t.amount_cents))
                }
                1 => {
                    let ok = core.transfers(None, None).iter().any(|t| t.note == "Transferencia de prueba" && t.amount_cents == 500);
                    ("nueva transferencia", closed && ok, String::new())
                }
                2 => {
                    let a = core.accounts().into_iter().find(|a| a.name == "Efectivo").unwrap();
                    ("editar cuenta (TAE)", closed && (a.apy - 2.5).abs() < 1e-9, format!("apy={}", a.apy))
                }
                3 => {
                    let c = core.category(ocio.unwrap()).unwrap();
                    ("editar categoría (presupuesto)", closed && c.budget_cents == 15000, format!("budget={}", c.budget_cents))
                }
                4 => {
                    let ok = core.categories(false).iter().any(|c| c.name == "Categoría de prueba" && c.fixed);
                    ("nueva categoría", closed && ok, String::new())
                }
                5 => {
                    let ok = core.recurring_list().iter().any(|r| r.note == "Recurrente de prueba" && r.amount_cents == 999 && r.day_of_month == 28);
                    ("nuevo recurrente", closed && ok, String::new())
                }
                6 => {
                    // El diálogo de operaciones se queda abierto a propósito para añadir más.
                    let inv = core.portfolio().items.into_iter().next().unwrap();
                    let r = core.replay_ops(inv.id).unwrap();
                    let ok = r.ops.iter().any(|o| o.ty == "compra" && (o.units - 0.5).abs() < 1e-9 && o.amount_cents == 5000);
                    ws.modal = Modal::None;
                    ("compra de inversión", ok, format!("{} operaciones", r.ops.len()))
                }
                7 => {
                    let ok = core.goals().iter().any(|g| g.name == "Objetivo de prueba" && g.target_cents == 100000 && g.deadline.as_deref() == Some("2027-06-30"));
                    ("nuevo objetivo", closed && ok, String::new())
                }
                8 => {
                    let ok = core.goals().iter().any(|g| g.name == "Objetivo de prueba" && g.saved_cents == 12000);
                    ("aportar a objetivo", closed && ok, String::new())
                }
                9 => {
                    let ok = core.templates().iter().any(|t| t.name == "Metro" && t.amount_cents == 150);
                    ("nuevo gasto rápido", closed && ok, String::new())
                }
                10 => {
                    let saved = core.recent(5).iter().any(|t| t.note == "No debe guardarse");
                    ("Esc cierra sin guardar", closed && !saved, String::new())
                }
                _ => {
                    let err = match &ws.modal {
                        Modal::Tx(f) => f.error.clone(),
                        _ => String::new(),
                    };
                    let ok = !closed && !err.is_empty();
                    ws.modal = Modal::None;
                    ("importe vacío → error sin cerrar", ok, err)
                }
            };
            self.dlg.push(format!("{} {name}{}", if ok { "OK  " } else { "FALLO" }, if detail.is_empty() { String::new() } else { format!(" ({detail})") }));
        }
    }

    /// Clic de ratón real en FZ_CLICK=x,y («mes:clic»): mueve, pulsa, suelta y
    /// dice qué diálogo se abrió.
    pub fn click(&mut self, ws: &Workspace, ctx: &egui::Context) {
        if self.current() != Some("mes:clic") {
            return;
        }
        // FZ_CLICK=x,y[;x,y…]: varios clics seguidos (p. ej. borrar y confirmar).
        let pts: Vec<egui::Pos2> = std::env::var("FZ_CLICK")
            .unwrap_or_default()
            .split(';')
            .filter_map(|v| {
                let (a, b) = v.split_once(',')?;
                Some(egui::pos2(a.trim().parse().ok()?, b.trim().parse().ok()?))
            })
            .collect();
        if pts.is_empty() {
            return;
        }
        let f = self.script_frame;
        self.script_frame += 1;
        let total = || ws.core.recent(100_000).len();
        if f == 19 {
            eprintln!("clic: {} puntos por píxel · movimientos: {}", ctx.pixels_per_point(), total());
        }
        // Cada clic ocupa 14 frames a partir del 20.
        let (i, k) = (f.saturating_sub(20) / 14, f.saturating_sub(20) % 14);
        if f >= 20 {
            if let Some(&pos) = pts.get(i as usize) {
                let btn = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
                match k {
                    0 => self.pending.push(egui::Event::PointerMoved(pos)),
                    // FZ_HOVER=1: solo pasar el ratón (capturar resaltados).
                    3 if std::env::var("FZ_HOVER").is_err() => self.pending.push(btn(true)),
                    5 if std::env::var("FZ_HOVER").is_err() => self.pending.push(btn(false)),
                    10 => {
                        let what = match &ws.modal {
                            super::forms::Modal::None => "ningún diálogo".to_string(),
                            super::forms::Modal::Tx(f) => format!("diálogo de movimiento (importe «{}», concepto «{}», fecha {})", f.amount, f.note, f.date),
                            super::forms::Modal::Confirm { title, .. } => format!("confirmación: «{title}»"),
                            _ => "otro diálogo".to_string(),
                        };
                        eprintln!("clic {} en ({},{}) → {what} · movimientos: {}", i + 1, pos.x, pos.y, total());
                    }
                    _ => {}
                }
            }
        }
        if f < 20 + 14 * pts.len() as u32 + 4 {
            self.wait = 1;
        }
    }

    /// Llamar cada frame. Devuelve true cuando hay que cerrar la app.
    pub fn tick(&mut self, ctx: &egui::Context) -> bool {
        if self.idx >= self.list.len() {
            self.done = true;
            return true;
        }
        // Captura recibida → guardar y pasar a la siguiente.
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
        });
        if let Some(img) = shot {
            let name = self.list[self.idx].replace(':', "-");
            let path = self.dir.join(format!("{name}.png"));
            if let Some(buf) = image::RgbaImage::from_raw(img.size[0] as u32, img.size[1] as u32, img.as_raw().to_vec()) {
                let _ = buf.save(&path);
            }
            self.idx += 1;
            self.wait = 0;
            self.script_frame = 0;
            self.step_start = None;
            self.requested = false;
            ctx.request_repaint();
            return false;
        }
        self.wait += 1;
        // Además de unos frames, medio segundo real: deja terminar los fundidos de entrada.
        let now = ctx.input(|i| i.time);
        let start = *self.step_start.get_or_insert(now);
        if self.wait > 12 && now - start > 0.5 && !self.requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.requested = true;
        }
        ctx.request_repaint();
        false
    }
}
