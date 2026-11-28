//! Componentes propios del sistema Factory.

use egui::{Align2, Color32, CornerRadius, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use super::theme::{self, p};
use crate::util;

pub const R2: CornerRadius = CornerRadius::same(2);

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    /// Relleno de tinta (acción principal de la zona).
    Primary,
    /// Contorno de 1 px.
    Secondary,
    /// Solo texto (acciones de fila).
    Ghost,
    /// Destructiva: contorno y texto en rojo.
    Danger,
}

/// Etiqueta de sección: mayúsculas pequeñas con mucho espaciado.
pub fn label_caps(ui: &mut Ui, text: &str) -> Response {
    ui.label(caps(text))
}

pub fn caps(text: &str) -> RichText {
    RichText::new(text.to_uppercase()).font(theme::mono(10.5)).color(p().muted).extra_letter_spacing(1.4)
}

/// Botón con icono opcional. `active` dibuja el bloque de estado a la derecha.
pub fn button(ui: &mut Ui, icon: Option<&str>, text: &str, kind: Kind) -> Response {
    button_ex(ui, icon, text, kind, false, true)
}

pub fn button_ex(ui: &mut Ui, icon: Option<&str>, text: &str, kind: Kind, active: bool, enabled: bool) -> Response {
    let pal = p();
    let font = theme::medium(13.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font.clone(), pal.ink);
    let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
    let pad = vec2(14.0, 0.0);
    let block = if active { 8.0 } else { 0.0 };
    let size = vec2(galley.size().x + icon_w + pad.x * 2.0 + block, 34.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered() && enabled;
        let pressed = resp.is_pointer_button_down_on();
        let (fill, stroke, fg) = match kind {
            Kind::Primary => {
                let f = if pressed { theme::lerp(pal.ink, pal.bg, 0.25) } else if hovered { theme::lerp(pal.ink, pal.bg, 0.12) } else { pal.ink };
                (f, Stroke::new(1.0, f), pal.bg)
            }
            Kind::Secondary => (
                if hovered { pal.surface2 } else { Color32::TRANSPARENT },
                Stroke::new(1.0, if hovered { pal.ink2 } else { pal.line_strong }),
                pal.ink,
            ),
            Kind::Ghost => (if hovered { pal.surface2 } else { Color32::TRANSPARENT }, Stroke::NONE, if hovered { pal.ink } else { pal.ink2 }),
            Kind::Danger => (
                if hovered { pal.critical.gamma_multiply(0.15) } else { Color32::TRANSPARENT },
                Stroke::new(1.0, pal.critical),
                pal.critical,
            ),
        };
        let fg = if enabled { fg } else { pal.muted };
        let painter = ui.painter();
        painter.rect(rect, R2, fill, stroke, StrokeKind::Inside);
        if active {
            let b = Rect::from_min_max(pos2(rect.right() - block, rect.top()), rect.right_bottom());
            painter.rect_filled(b, CornerRadius { ne: 2, se: 2, nw: 0, sw: 0 }, pal.signal);
        }
        let mut x = rect.left() + pad.x;
        if let Some(ic) = icon {
            painter.text(pos2(x + 8.0, rect.center().y), Align2::CENTER_CENTER, ic, theme::icon(17.0), fg);
            x += icon_w;
        }
        painter.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, text, font, fg);
        if resp.has_focus() {
            painter.rect_stroke(rect.expand(2.0), R2, Stroke::new(1.5, pal.signal), StrokeKind::Outside);
        }
    }
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Botón cuadrado solo de icono.
pub fn icon_button(ui: &mut Ui, icon: &str, tooltip: &str) -> Response {
    icon_button_sized(ui, icon, tooltip, 32.0)
}

/// Variante compacta para acciones dentro de filas de lista.
pub fn icon_button_sm(ui: &mut Ui, icon: &str, tooltip: &str) -> Response {
    icon_button_sized(ui, icon, tooltip, 26.0)
}

fn icon_button_sized(ui: &mut Ui, icon: &str, tooltip: &str, side: f32) -> Response {
    let pal = p();
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered();
        if hovered {
            ui.painter().rect_filled(rect, R2, pal.surface2);
        }
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, theme::icon(if side < 30.0 { 16.0 } else { 18.0 }), if hovered { pal.ink } else { pal.ink2 });
        if resp.has_focus() {
            ui.painter().rect_stroke(rect, R2, Stroke::new(1.5, pal.signal), StrokeKind::Inside);
        }
    }
    resp.on_hover_text(tooltip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Importe con signo y color semántico en la monoespaciada tabular.
pub fn amount_text(cents: i64, ty: &str, size: f32, privacy: bool) -> RichText {
    let pal = p();
    let (txt, col) = match ty {
        "ingreso" => (format!("+{}", util::fmt_num(cents)), pal.ingreso),
        "gasto" => (format!("−{}", util::fmt_num(cents)), pal.ink),
        _ => (util::fmt_num(cents), pal.ink2),
    };
    RichText::new(if privacy { "•••••".to_string() } else { txt }).font(theme::mono(size)).color(col)
}

pub fn hide(s: String, privacy: bool) -> String {
    if privacy { "•••••".into() } else { s }
}

/// Tira de color de categoría (el «código de color» de la funda).
pub fn color_strip(ui: &mut Ui, color: Color32, size: Vec2) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(1), color);
    resp
}

/// Barra de progreso fina: relleno con el color dado, fondo de filete.
pub fn progress(ui: &mut Ui, frac: f32, color: Color32, height: f32) -> Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
    let pal = p();
    ui.painter().rect_filled(rect, CornerRadius::same(1), pal.line);
    let f = frac.clamp(0.0, 1.0);
    if f > 0.0 {
        let r = Rect::from_min_size(rect.min, vec2(rect.width() * f, rect.height()));
        ui.painter().rect_filled(r, CornerRadius::same(1), color);
    }
    resp
}

/// Color de estado de un presupuesto: <75 % normal, 75–100 % aviso, >100 % crítico.
pub fn budget_color(pct: i64, base: Color32) -> Color32 {
    let pal = p();
    if pct > 100 {
        pal.critical
    } else if pct >= 75 {
        pal.warning
    } else {
        base
    }
}

/// Fila de lista que reacciona al ratón; devuelve la respuesta de toda la fila.
pub fn row_frame<R>(ui: &mut Ui, selected: bool, add: impl FnOnce(&mut Ui) -> R) -> (Response, R) {
    let pal = p();
    // Se reserva la capa del fondo antes del contenido para no taparlo.
    let bg = ui.painter().add(egui::Shape::Noop);
    // El clic de la fila se registra POR DEBAJO de su contenido: así los botones
    // de la fila (editar, repetir, borrar, flechas…) reciben su propio clic.
    // Antes se registraba después y la fila se los quedaba todos.
    let out = ui.scope_builder(egui::UiBuilder::new().sense(Sense::click()), |ui| {
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(10, 4))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                add(ui)
            })
            .inner
    });
    let resp = out.response;
    let rect = resp.rect;
    if selected || resp.contains_pointer() {
        let fill = if selected { pal.surface2 } else { pal.surface };
        ui.painter().set(bg, egui::Shape::rect_filled(rect, R2, fill));
    }
    if selected {
        let block = Rect::from_min_max(pos2(rect.right() - 4.0, rect.top()), rect.right_bottom());
        ui.painter().rect_filled(block, CornerRadius::ZERO, pal.signal);
    }
    (resp, out.inner)
}

/// Filete horizontal de 1 px a todo el ancho.
pub fn hline(ui: &mut Ui) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, p().line);
}

/// Panel con borde de 1 px (contenedor de zona, no tarjeta decorativa).
pub fn panel<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let pal = p();
    egui::Frame::new()
        .fill(pal.surface)
        .stroke(Stroke::new(1.0, pal.line))
        .corner_radius(R2)
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Mensaje de estado vacío que enseña qué hacer.
pub fn empty_state(ui: &mut Ui, icon: &str, title: &str, hint: &str) {
    let pal = p();
    ui.vertical_centered(|ui| {
        ui.add_space(18.0);
        ui.label(RichText::new(icon).font(theme::icon(30.0)).color(pal.muted));
        ui.add_space(6.0);
        ui.label(RichText::new(title).font(theme::medium(15.0)).color(pal.ink));
        ui.label(RichText::new(hint).color(pal.muted));
        ui.add_space(18.0);
    });
}

/// Campo de texto con el estilo de la casa.
pub fn text_field(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> Response {
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(RichText::new(hint).color(p().muted))
            .desired_width(width)
            .margin(egui::Margin::symmetric(10, 7)),
    )
}

pub fn mono_field(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> Response {
    ui.add(
        egui::TextEdit::singleline(value)
            .font(theme::mono(14.0))
            .hint_text(RichText::new(hint).color(p().muted).font(theme::mono(14.0)))
            .desired_width(width)
            .horizontal_align(egui::Align::RIGHT)
            .margin(egui::Margin::symmetric(10, 7)),
    )
}

/// Selector de opciones de texto (cuentas, categorías…).
pub fn select<T: PartialEq + Clone>(ui: &mut Ui, id: &str, current: &mut T, options: &[(T, String)], width: f32) -> Response {
    let label = options.iter().find(|(v, _)| v == current).map(|(_, l)| l.clone()).unwrap_or_else(|| "—".into());
    egui::ComboBox::from_id_salt(id)
        .selected_text(label)
        .width(width)
        .height(360.0)
        .show_ui(ui, |ui| {
            for (v, l) in options {
                ui.selectable_value(current, v.clone(), l);
            }
        })
        .response
}

/// Minúsculas y sin tildes, para buscar "ocio" en "Ócio" o "alimentacion" en "Alimentación".
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}

#[allow(dead_code)]
pub struct Pick {
    pub resp: Response,
    /// Se eligió una opción en este frame (Tab, Enter o clic).
    pub picked: bool,
    /// Se pulsó Enter en el campo (para apuntar directamente).
    pub enter: bool,
}

#[derive(Clone, Default)]
struct PickState {
    query: String,
    hl: usize,
    open: bool,
    popup: Option<Rect>,
}

/// Selector con búsqueda: se escribe para filtrar (sin tildes ni mayúsculas),
/// ↑/↓ recorre, Tab elige y pasa al siguiente campo, Enter elige, clic también.
pub fn search_select<T: PartialEq + Clone>(ui: &mut Ui, id_src: &str, current: &mut T, options: &[(T, String)], width: f32, hint: &str) -> Pick {
    let pal = p();
    let id = egui::Id::new(("fz_pick", id_src));
    let edit_id = id.with("edit");
    let mut st: PickState = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let focused = ui.memory(|m| m.has_focus(edit_id));
    let filter = |q: &str| -> Vec<usize> {
        let q = fold(q.trim());
        let mut v: Vec<usize> = (0..options.len()).filter(|i| q.is_empty() || fold(&options[*i].1).contains(&q)).collect();
        // Primero las que empiezan por lo escrito.
        v.sort_by_key(|i| !fold(&options[*i].1).starts_with(&q));
        v
    };
    let mut picked = false;
    if focused {
        let list = filter(&st.query);
        let (down, up, tab) =
            ui.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown), i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp), i.key_pressed(egui::Key::Tab)));
        if down {
            st.open = true;
            st.hl = (st.hl + 1).min(list.len().saturating_sub(1));
        }
        if up {
            st.hl = st.hl.saturating_sub(1);
        }
        if tab && st.open && !st.query.is_empty() {
            if let Some(i) = list.get(st.hl) {
                *current = options[*i].0.clone();
                picked = true;
                st.open = false;
            }
        }
    }
    let label_now = options.iter().find(|(v, _)| v == current).map(|(_, l)| l.clone()).unwrap_or_default();
    let mut text = if focused { st.query.clone() } else { label_now.clone() };
    let resp = ui.add(
        egui::TextEdit::singleline(&mut text)
            .id(edit_id)
            .hint_text(RichText::new(if focused { label_now.as_str() } else { hint }).color(pal.muted))
            .desired_width(width)
            .margin(egui::Margin::symmetric(10, 7)),
    );
    if resp.gained_focus() {
        st = PickState { open: true, ..Default::default() };
    } else if resp.changed() && focused {
        st.query = text.clone();
        st.hl = 0;
        st.open = true;
    }
    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let list = filter(&st.query);
    if enter && !st.query.is_empty() {
        if let Some(i) = list.get(st.hl) {
            *current = options[*i].0.clone();
            picked = true;
        }
    }
    // Al perder el foco se cierra, salvo que el ratón esté sobre la lista (clic en curso).
    let over_popup = st.popup.zip(ui.input(|i| i.pointer.hover_pos())).is_some_and(|(r, h)| r.contains(h));
    if resp.lost_focus() && !over_popup {
        st.open = false;
    }
    if st.open && (resp.has_focus() || over_popup) && !list.is_empty() {
        let area = egui::Area::new(id.with("pop")).order(egui::Order::Foreground).fixed_pos(resp.rect.left_bottom() + vec2(0.0, 2.0)).show(ui.ctx(), |ui| {
            egui::Frame::new().fill(pal.surface).stroke(Stroke::new(1.0, pal.line_strong)).corner_radius(R2).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
                ui.set_width(width.max(180.0));
                for (k, i) in list.iter().take(12).enumerate() {
                    let sel = k == st.hl;
                    let (r, rr) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
                    if sel || rr.hovered() {
                        ui.painter().rect_filled(r, R2, pal.surface2);
                    }
                    if sel {
                        let b = Rect::from_min_max(pos2(r.right() - 4.0, r.top()), r.right_bottom());
                        ui.painter().rect_filled(b, CornerRadius::ZERO, pal.signal);
                    }
                    ui.painter().text(pos2(r.left() + 8.0, r.center().y), Align2::LEFT_CENTER, &options[*i].1, theme::ui_font(13.5), pal.ink);
                    if rr.clicked() {
                        *current = options[*i].0.clone();
                        picked = true;
                        st.open = false;
                    }
                }
                if list.len() > 12 {
                    ui.label(RichText::new(format!("… {} más: sigue escribiendo", list.len() - 12)).font(theme::ui_font(12.0)).color(pal.muted));
                }
            });
        });
        st.popup = Some(area.response.rect);
    } else {
        st.popup = None;
    }
    if picked {
        st.query.clear();
    }
    ui.data_mut(|d| d.insert_temp(id, st));
    Pick { resp, picked, enter }
}

/// Grupo segmentado (Gasto | Ingreso | Transferencia…).
pub fn segmented(ui: &mut Ui, current: &mut usize, labels: &[&str]) -> bool {
    let pal = p();
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, l) in labels.iter().enumerate() {
            let sel = *current == i;
            let font = theme::medium(13.0);
            let g = ui.painter().layout_no_wrap(l.to_string(), font.clone(), pal.ink);
            let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 26.0, 32.0), Sense::click());
            let fill = if sel { pal.ink } else if resp.hovered() { pal.surface2 } else { Color32::TRANSPARENT };
            ui.painter().rect(rect, CornerRadius::ZERO, fill, Stroke::new(1.0, if sel { pal.ink } else { pal.line_strong }), StrokeKind::Inside);
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, *l, font, if sel { pal.bg } else { pal.ink2 });
            if resp.clicked() && !sel {
                *current = i;
                changed = true;
            }
            resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
    });
    changed
}

/// Grupo segmentado que ocupa `width` con segmentos iguales (diálogos).
pub fn segmented_fill(ui: &mut Ui, current: &mut usize, labels: &[&str], width: f32) -> bool {
    let pal = p();
    let mut changed = false;
    let seg = (width / labels.len() as f32).floor();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, l) in labels.iter().enumerate() {
            let sel = *current == i;
            let font = theme::medium(13.5);
            let (rect, resp) = ui.allocate_exact_size(vec2(seg, 36.0), Sense::click());
            let fill = if sel { pal.ink } else if resp.hovered() { pal.surface2 } else { Color32::TRANSPARENT };
            ui.painter().rect(rect, CornerRadius::ZERO, fill, Stroke::new(1.0, if sel { pal.ink } else { pal.line_strong }), StrokeKind::Inside);
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, *l, font, if sel { pal.bg } else { pal.ink2 });
            if resp.clicked() && !sel {
                *current = i;
                changed = true;
            }
            resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
    });
    changed
}

/// Aviso efímero en la esquina inferior.
#[derive(Clone)]
pub struct Toast {
    pub text: String,
    pub error: bool,
    pub until: f64,
    /// Acción «Deshacer»: id del movimiento creado.
    pub undo_tx: Option<i64>,
}

pub fn show_toasts(ctx: &egui::Context, toasts: &mut Vec<Toast>, now: f64) -> Option<i64> {
    toasts.retain(|t| t.until > now);
    let mut undo = None;
    if toasts.is_empty() {
        return None;
    }
    let pal = p();
    egui::Area::new(egui::Id::new("toasts")).anchor(Align2::RIGHT_BOTTOM, vec2(-20.0, -20.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        for (i, t) in toasts.iter().enumerate() {
            egui::Frame::new()
                .fill(pal.surface2)
                .stroke(Stroke::new(1.0, if t.error { pal.critical } else { pal.line_strong }))
                .corner_radius(R2)
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let col = if t.error { pal.critical } else { pal.positive };
                        let ic = if t.error { egui_phosphor::light::WARNING } else { egui_phosphor::light::CHECK };
                        ui.label(RichText::new(ic).font(theme::icon(16.0)).color(col));
                        ui.label(RichText::new(&t.text).color(pal.ink));
                        if let Some(id) = t.undo_tx {
                            if button(ui, None, "Deshacer", Kind::Ghost).clicked() {
                                undo = Some(id);
                            }
                        }
                    });
                });
            if i + 1 < toasts.len() {
                ui.add_space(6.0);
            }
        }
    });
    if undo.is_some() {
        toasts.retain(|t| t.undo_tx != undo);
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
    undo
}
