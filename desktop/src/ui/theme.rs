//! Sistema visual «Catálogo Factory»: negro mate, tinta blanca, filetes de un
//! píxel, controles de esquina casi recta y color solo con función (gasto,
//! ingreso, estado y la tira de cada categoría).

use std::sync::Arc;

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Visuals};

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub surface2: Color32,
    pub line: Color32,
    pub line_strong: Color32,
    pub ink: Color32,
    pub ink2: Color32,
    pub muted: Color32,
    pub gasto: Color32,
    pub ingreso: Color32,
    pub positive: Color32,
    pub warning: Color32,
    pub critical: Color32,
    /// Bloque de estado activo / foco (el rojo de la funda).
    pub signal: Color32,
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub const DARK: Palette = Palette {
    dark: true,
    bg: hex(0x0b0b0c),
    surface: hex(0x121214),
    surface2: hex(0x19191c),
    line: hex(0x26262a),
    line_strong: hex(0x3a3a3f),
    ink: hex(0xf4f4f2),
    ink2: hex(0xb4b4b9),
    muted: hex(0x8d8d93),
    gasto: hex(0xff6a45),
    ingreso: hex(0x6e9bff),
    positive: hex(0x37d6a4),
    warning: hex(0xffd23f),
    critical: hex(0xff5a36),
    signal: hex(0xff3b2f),
};

pub const LIGHT: Palette = Palette {
    dark: false,
    bg: hex(0xf2f2ef),
    surface: hex(0xfbfbf9),
    surface2: hex(0xe8e8e4),
    line: hex(0xd4d4cf),
    line_strong: hex(0xa9a9a4),
    ink: hex(0x0b0b0c),
    ink2: hex(0x35353a),
    muted: hex(0x5f5f65),
    gasto: hex(0xc23a1c),
    ingreso: hex(0x2c58c9),
    positive: hex(0x0a7d4f),
    warning: hex(0x8f6400),
    critical: hex(0xc23a1c),
    signal: hex(0xe0281d),
};

pub fn palette(dark: bool) -> Palette {
    if dark { DARK } else { LIGHT }
}

thread_local! {
    static CURRENT: std::cell::Cell<Palette> = const { std::cell::Cell::new(DARK) };
}

/// Paleta activa (se fija al aplicar el tema).
pub fn p() -> Palette {
    CURRENT.with(|c| c.get())
}

// Tamaños de la escala tipográfica (ratio ≈1,2).
pub const SIZE_SMALL: f32 = 11.5;
pub const SIZE_BODY: f32 = 14.0;
pub const SIZE_TITLE: f32 = 20.0;
pub const SIZE_DISPLAY: f32 = 84.0;

pub fn ui_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}
pub fn display(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("display".into()))
}
pub fn wide(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("wide".into()))
}
pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}
pub fn mono_bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("mono_bold".into()))
}
pub fn icon(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("icons".into()))
}

pub fn fonts() -> FontDefinitions {
    let mut f = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        f.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add("archivo", include_bytes!("../../assets/fonts/Archivo-Regular.ttf"));
    add("archivo_medium", include_bytes!("../../assets/fonts/Archivo-Medium.ttf"));
    add("archivo_semibold", include_bytes!("../../assets/fonts/Archivo-SemiBold.ttf"));
    add("archivo_exp_xb", include_bytes!("../../assets/fonts/ArchivoExpanded-ExtraBold.ttf"));
    add("archivo_exp_sb", include_bytes!("../../assets/fonts/ArchivoExpanded-SemiBold.ttf"));
    add("jbm", include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf"));
    add("jbm_bold", include_bytes!("../../assets/fonts/JetBrainsMono-Bold.ttf"));
    add("phosphor", egui_phosphor::Variant::Light.font_bytes());

    // Los emojis de las categorías (datos del usuario) salen de la fuente de
    // emoji que trae egui; los iconos de la interfaz, de Phosphor.
    let emoji: Vec<String> =
        f.families.get(&FontFamily::Proportional).cloned().unwrap_or_default().into_iter().filter(|k| k.contains("emoji") || k.contains("Emoji")).collect();
    let fam = |main: &str| -> Vec<String> {
        let mut v = vec![main.to_string(), "phosphor".to_string()];
        v.extend(emoji.iter().cloned());
        v
    };
    f.families.insert(FontFamily::Proportional, fam("archivo"));
    f.families.insert(FontFamily::Monospace, fam("jbm"));
    f.families.insert(FontFamily::Name("medium".into()), fam("archivo_medium"));
    f.families.insert(FontFamily::Name("semibold".into()), fam("archivo_semibold"));
    f.families.insert(FontFamily::Name("display".into()), fam("archivo_exp_xb"));
    f.families.insert(FontFamily::Name("wide".into()), fam("archivo_exp_sb"));
    f.families.insert(FontFamily::Name("mono_bold".into()), fam("jbm_bold"));
    f.families.insert(FontFamily::Name("icons".into()), vec!["phosphor".into()]);
    f
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    let p = palette(dark);
    CURRENT.with(|c| c.set(p));
    ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
    let mut style = (*ctx.style_of(if dark { egui::Theme::Dark } else { egui::Theme::Light })).clone();

    style.text_styles = [
        (TextStyle::Small, ui_font(SIZE_SMALL)),
        (TextStyle::Body, ui_font(SIZE_BODY)),
        (TextStyle::Button, medium(SIZE_BODY)),
        (TextStyle::Heading, wide(SIZE_TITLE)),
        (TextStyle::Monospace, mono(13.5)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.interact_size = egui::vec2(36.0, 32.0);
    style.spacing.combo_height = 320.0;
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.floating = true;
    style.interaction.selectable_labels = false;

    let r2 = CornerRadius::same(2);
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    v.override_text_color = Some(p.ink);
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.line_strong);
    v.window_corner_radius = r2;
    v.window_shadow = egui::epaint::Shadow { offset: [0, 10], blur: 28, spread: 0, color: Color32::from_black_alpha(if dark { 160 } else { 50 }) };
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(if dark { 140 } else { 40 }) };
    v.menu_corner_radius = r2;
    v.extreme_bg_color = p.surface2;
    v.faint_bg_color = p.surface;
    v.code_bg_color = p.surface2;
    v.hyperlink_color = p.ingreso;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.critical;
    v.selection.bg_fill = p.signal.gamma_multiply(if dark { 0.45 } else { 0.25 });
    v.selection.stroke = Stroke::new(1.0, p.ink);
    v.text_cursor.stroke = Stroke::new(2.0, p.signal);
    v.striped = false;
    v.indent_has_left_vline = false;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.ink2);
    w.noninteractive.corner_radius = r2;
    for (st, bg, stroke) in [
        (&mut w.inactive, p.surface2, p.line_strong),
        (&mut w.hovered, p.surface2, p.ink2),
        (&mut w.active, p.surface2, p.ink),
        (&mut w.open, p.surface2, p.ink2),
    ] {
        st.bg_fill = bg;
        st.weak_bg_fill = bg;
        st.bg_stroke = Stroke::new(1.0, stroke);
        st.fg_stroke = Stroke::new(1.0, p.ink);
        st.corner_radius = r2;
        st.expansion = 0.0;
    }
    style.visuals = v;
    ctx.set_style_of(if dark { egui::Theme::Dark } else { egui::Theme::Light }, style);
}

/// Paleta industrial de la casa: los bloques de la funda y tonos derivados.
/// Cada color guardado (los de la web) se pinta con su equivalente.
const INDUSTRIAL: [(u32, u32); 17] = [
    (0x2a78d6, 0x5b8cff), // azul
    (0x1baf7a, 0x2fd1a0), // verde agua
    (0xeda100, 0xffd23f), // amarillo
    (0x008300, 0x9be15d), // verde lima (derivado)
    (0x4a3aa7, 0xb98cff), // violeta
    (0xe34948, 0xff5a36), // rojo señal
    (0xe87ba4, 0xff8fb1), // rosa (derivado)
    (0xeb6834, 0xff9a3d), // naranja (derivado)
    (0x1c5cab, 0x9ec0ff), // azul claro (derivado)
    (0xc98500, 0xffe68a), // amarillo claro (derivado)
    (0x898781, 0x8b8b90), // gris industrial
    // Tonos extra (derivados de los bloques) para cuando hay más categorías que
    // huecos: sin color guardado propio, solo se asignan si los anteriores se agotan.
    (0xf4f4f2, 0xe6e6e1), // blanco de la funda
    (0x9a7cff, 0xdcc8ff), // violeta claro
    (0x5fd6b0, 0x9af0d4), // verde agua claro
    (0x2f5cff, 0x3d6bff), // azul intenso
    (0xc8743a, 0xd08a4e), // cobre
    (0xb8c25a, 0xc9d16a), // oliva
];

fn rgb_of(v: u32) -> (f32, f32, f32) {
    (((v >> 16) & 255) as f32, ((v >> 8) & 255) as f32, (v & 255) as f32)
}

fn dist(a: u32, b: u32) -> f32 {
    let ((r1, g1, b1), (r2, g2, b2)) = (rgb_of(a), rgb_of(b));
    (r1 - r2).powi(2) + (g1 - g2).powi(2) + (b1 - b2).powi(2)
}

thread_local! {
    /// id de categoría → hueco de la paleta industrial asignado.
    static SLOTS: std::cell::RefCell<std::collections::HashMap<i64, u32>> = std::cell::RefCell::new(Default::default());
}

/// Reparte la paleta: cada categoría (la más usada primero) recibe el tono más
/// cercano a su color guardado que siga libre, para que dos categorías no
/// compartan color aunque en los datos tengan el mismo. Gastos e ingresos se
/// reparten por separado (nunca se ven juntos en la misma gráfica).
pub fn assign_category_slots(cats: &[(i64, String, String, i64)]) {
    let mut map = std::collections::HashMap::new();
    for ty in ["gasto", "ingreso"] {
        // Las más usadas eligen primero; las archivadas (uso negativo) van al final.
        let mut list: Vec<&(i64, String, String, i64)> = cats.iter().filter(|c| c.2 == ty).collect();
        list.sort_by(|a, b| b.3.cmp(&a.3).then(a.0.cmp(&b.0)));
        let mut used: Vec<u32> = Vec::new();
        for (id, hexstr, _, _) in list {
            let v = u32::from_str_radix(hexstr.trim().trim_start_matches('#'), 16).unwrap_or(0x898781);
            let mut order: Vec<&(u32, u32)> = INDUSTRIAL.iter().collect();
            order.sort_by(|a, b| dist(v, a.0).total_cmp(&dist(v, b.0)));
            let pick = order.iter().find(|x| !used.contains(&x.1)).map(|x| x.1).unwrap_or(order[0].1);
            used.push(pick);
            map.insert(*id, pick);
        }
    }
    SLOTS.with(|s| *s.borrow_mut() = map);
}

/// Color de una categoría concreta (su hueco asignado; si no lo tiene, por su color).
pub fn cat_color_for(id: impl Into<Option<i64>>, hexstr: &str) -> Color32 {
    if let Some(slot) = id.into().and_then(|i| SLOTS.with(|s| s.borrow().get(&i).copied())) {
        let c = hex(slot);
        return if p().dark { c } else { lerp(c, Color32::BLACK, 0.28) };
    }
    cat_color(hexstr)
}

/// Color de una categoría en la paleta industrial (el más parecido si es uno propio).
pub fn cat_color(hexstr: &str) -> Color32 {
    let s = hexstr.trim().trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).unwrap_or(0x898781);
    let (r, g, b) = rgb_of(v);
    let ind = INDUSTRIAL
        .iter()
        .min_by(|a, b2| {
            let d = |x: u32| {
                let (r2, g2, b2_) = rgb_of(x);
                (r - r2).powi(2) + (g - g2).powi(2) + (b - b2_).powi(2)
            };
            d(a.0).total_cmp(&d(b2.0))
        })
        .map(|x| x.1)
        .unwrap_or(0x8b8b90);
    let c = hex(ind);
    // Sobre papel claro se oscurecen para mantener el contraste.
    if p().dark { c } else { lerp(c, Color32::BLACK, 0.28) }
}

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// Paleta para elegir color de categoría (los mismos tonos validados de la web).
pub const CATEGORY_SWATCHES: [&str; 11] =
    ["#2a78d6", "#1baf7a", "#eda100", "#008300", "#4a3aa7", "#e34948", "#e87ba4", "#eb6834", "#1c5cab", "#c98500", "#898781"];
