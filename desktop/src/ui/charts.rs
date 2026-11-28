//! Gráficas dibujadas a mano con el pintor de egui.

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

use super::theme::{self, p};
use crate::ledger::analysis::{MonthTotals, PulseLine};
use crate::util;

/// Suavizado gaussiano ligero para que el gasto diario se lea como un pulso.
fn smooth(v: &[f32], sigma: f32) -> Vec<f32> {
    if v.is_empty() {
        return vec![];
    }
    let r = (sigma * 2.5).ceil() as isize;
    let w: Vec<f32> = (-r..=r).map(|i| (-(i as f32).powi(2) / (2.0 * sigma * sigma)).exp()).collect();
    (0..v.len() as isize)
        .map(|i| {
            let (mut s, mut ws) = (0.0, 0.0);
            for (k, wk) in (-r..=r).zip(&w) {
                let j = i + k;
                if j >= 0 && (j as usize) < v.len() {
                    s += v[j as usize] * wk;
                    ws += wk;
                }
            }
            s / ws
        })
        .collect()
}

/// El gráfico de pulsos: un mes por línea (el más antiguo arriba). Devuelve el
/// mes clicado, si lo hay.
pub fn pulse(ui: &mut Ui, lines: &[PulseLine], selected: &str, height: f32, privacy: bool) -> Option<String> {
    let pal = p();
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    if lines.is_empty() || !ui.is_rect_visible(rect) {
        return None;
    }
    let painter = ui.painter_at(rect.expand(2.0));
    let label_w = 64.0;
    let total_w = 104.0;
    // Abajo se reservan 16 px para el eje de días, dentro del propio rectángulo.
    let plot = Rect::from_min_max(pos2(rect.left() + label_w, rect.top() + 2.0), pos2(rect.right() - total_w, rect.bottom() - 18.0));
    let n = lines.len();
    let max = lines.iter().flat_map(|l| l.daily.iter()).copied().max().unwrap_or(1).max(1) as f32;
    let norm_of = |v: f32| (v / max).powf(0.7);
    let smoothed: Vec<Vec<f32>> =
        lines.iter().map(|l| smooth(&(0..31).map(|d| norm_of(l.daily.get(d).copied().unwrap_or(0) as f32)).collect::<Vec<_>>(), 0.9)).collect();
    let peaks: Vec<f32> = smoothed.iter().map(|v| v.iter().cloned().fold(0.0, f32::max)).collect();
    // Las crestas invaden varias líneas de detrás, como en la funda.
    let amp = plot.height() * 0.45;
    // Reserva superior: solo lo que necesita el pico más alto (sin banda muerta).
    let mut reserve = amp;
    let mut step = (plot.height() - reserve) / (n.max(2) - 1) as f32;
    for _ in 0..4 {
        reserve = peaks.iter().enumerate().map(|(i, pk)| pk * amp - i as f32 * step).fold(16.0, f32::max);
        step = (plot.height() - reserve) / (n.max(2) - 1) as f32;
    }
    let amp_top = reserve;
    let today = util::today();
    let cur_month = util::current_month();

    let hover_i = resp.hover_pos().map(|hp| {
        let rel = ((hp.y - plot.top() - amp_top) / step).round();
        rel.clamp(0.0, n as f32 - 1.0) as usize
    });

    for (i, line) in lines.iter().enumerate() {
        let base = plot.top() + amp_top + step * i as f32;
        let days = 31usize;
        let sm = &smoothed[i];
        // Más puntos que días para que la curva se vea continua.
        let len = line.daily.len().max(1);
        let pts_per_day = 4;
        let mut pts: Vec<Pos2> = Vec::new();
        let last_day = if line.month == cur_month { today.format("%d").to_string().parse::<usize>().unwrap_or(len) } else { len };
        for k in 0..=(len - 1) * pts_per_day {
            let t = k as f32 / pts_per_day as f32;
            if t > (last_day.max(1) - 1) as f32 + 0.001 {
                break;
            }
            let i0 = t.floor() as usize;
            let i1 = (i0 + 1).min(len - 1);
            let f = t - i0 as f32;
            let v = sm[i0] * (1.0 - f) + sm[i1] * f;
            let x = plot.left() + plot.width() * (t / (days - 1) as f32);
            pts.push(pos2(x, base - v * amp));
        }
        if pts.len() < 2 {
            continue;
        }
        let is_sel = line.month == selected;
        let is_hover = hover_i == Some(i);
        // Relleno del color de fondo bajo la curva: tapa las líneas de detrás.
        // La zona no es convexa, así que se teje con un trapecio por tramo.
        let mut mesh = egui::Mesh::default();
        for (k, pt) in pts.iter().enumerate() {
            mesh.colored_vertex(*pt, pal.bg);
            mesh.colored_vertex(pos2(pt.x, base + 1.5), pal.bg);
            if k > 0 {
                let b = (k as u32) * 2;
                mesh.add_triangle(b - 2, b - 1, b);
                mesh.add_triangle(b - 1, b + 1, b);
            }
        }
        painter.add(Shape::mesh(mesh));
        let (w, col) = if is_sel {
            (2.6, pal.ink)
        } else if is_hover {
            (1.6, pal.ink)
        } else {
            (1.1, pal.ink.gamma_multiply(if pal.dark { 0.62 } else { 0.55 }))
        };
        painter.add(Shape::line(pts.clone(), Stroke::new(w, col)));

        // Hoy: punto rojo al final de la línea del mes en curso.
        if line.month == cur_month {
            if let Some(last) = pts.last() {
                painter.circle_filled(*last, 4.0, pal.signal);
            }
        }
        let (y, m) = util::month_parts(&line.month);
        let lbl = if m == 1 || i == 0 { format!("{} {}", util::MESES_CORTOS[(m - 1) as usize], y % 100) } else { util::MESES_CORTOS[(m - 1) as usize].to_string() };
        let lcol = if is_sel || is_hover { pal.ink } else { pal.muted };
        painter.text(pos2(rect.left(), base - 2.0), Align2::LEFT_BOTTOM, lbl.to_uppercase(), theme::mono(11.0), lcol);
        let tot = if privacy { "•••••".into() } else { util::fmt_num(line.total) };
        painter.text(pos2(rect.right(), base - 2.0), Align2::RIGHT_BOTTOM, tot, theme::mono(11.5), lcol);
    }
    // Eje de días.
    for d in [1usize, 8, 15, 22, 29] {
        let x = plot.left() + plot.width() * ((d - 1) as f32 / 30.0);
        painter.text(pos2(x, plot.bottom() + 5.0), Align2::CENTER_TOP, d.to_string(), theme::mono(10.0), pal.muted);
    }
    let mut clicked = None;
    if let Some(hi) = hover_i {
        if let Some(l) = lines.get(hi) {
            let txt = if privacy {
                util::cap(&util::month_label(&l.month))
            } else {
                format!("{} · {} de gasto · clic para abrirlo", util::cap(&util::month_label(&l.month)), util::eur(l.total))
            };
            resp.clone().on_hover_text_at_pointer(txt);
            if resp.clicked() {
                clicked = Some(l.month.clone());
            }
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    clicked
}

/// Barras de 12 meses: ingresos y gastos lado a lado, con leyenda en texto.
pub fn bars12(ui: &mut Ui, data: &[MonthTotals], selected: &str, height: f32, privacy: bool) -> Option<String> {
    let pal = p();
    // Leyenda: el color nunca va solo.
    ui.horizontal(|ui| {
        for (c, t) in [(pal.ingreso, "ingresos"), (pal.gasto, "gastos")] {
            let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().rect_filled(r, CornerRadius::ZERO, c);
            ui.label(egui::RichText::new(t).font(theme::mono(11.0)).color(pal.ink2));
            ui.add_space(10.0);
        }
        ui.label(egui::RichText::new("pasa por un mes para ver las cifras · clic para abrirlo").font(theme::ui_font(11.5)).color(pal.muted));
    });
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    if data.is_empty() {
        return None;
    }
    let painter = ui.painter_at(rect);
    let plot = Rect::from_min_max(rect.min + vec2(0.0, 4.0), rect.max - vec2(0.0, 18.0));
    let max = data.iter().map(|m| m.ingresos_cents.max(m.gastos_cents)).max().unwrap_or(1).max(1) as f32;
    let slot = plot.width() / data.len() as f32;
    let bw = 5.0;
    painter.line_segment([plot.left_bottom(), plot.right_bottom()], Stroke::new(1.0, pal.line_strong));
    let mut clicked = None;
    let hover = resp.hover_pos();
    for (i, m) in data.iter().enumerate() {
        let cx = plot.left() + slot * (i as f32 + 0.5);
        let col_rect = Rect::from_min_max(pos2(cx - slot / 2.0, plot.top()), pos2(cx + slot / 2.0, rect.bottom()));
        let hov = hover.is_some_and(|h| col_rect.contains(h));
        let sel = m.month == selected;
        let hi = m.ingresos_cents as f32 / max * plot.height();
        let hg = m.gastos_cents as f32 / max * plot.height();
        let r_in = Rect::from_min_max(pos2(cx - bw - 1.5, plot.bottom() - hi), pos2(cx - 1.5, plot.bottom()));
        let r_out = Rect::from_min_max(pos2(cx + 1.5, plot.bottom() - hg), pos2(cx + bw + 1.5, plot.bottom()));
        painter.rect_filled(r_in, CornerRadius::ZERO, pal.ingreso);
        painter.rect_filled(r_out, CornerRadius::ZERO, pal.gasto);
        let (_, mm) = util::month_parts(&m.month);
        let lc = if sel || hov { pal.ink } else { pal.muted };
        painter.text(pos2(cx, rect.bottom()), Align2::CENTER_BOTTOM, util::MESES_CORTOS[(mm - 1) as usize].to_uppercase(), theme::mono(9.5), lc);
        if hov {
            painter.line_segment([pos2(cx, plot.top()), pos2(cx, plot.bottom())], Stroke::new(1.0, pal.line_strong));
            let t = if privacy {
                util::cap(&util::month_label(&m.month))
            } else {
                format!(
                    "{}\ningresos {}\ngastos {}\nsaldo {}",
                    util::cap(&util::month_label(&m.month)),
                    util::eur(m.ingresos_cents),
                    util::eur(m.gastos_cents),
                    util::eur_signed(m.ingresos_cents - m.gastos_cents)
                )
            };
            resp.clone().on_hover_text_at_pointer(t);
            if resp.clicked() {
                clicked = Some(m.month.clone());
            }
        }
    }
    clicked
}

/// Calendario de gasto del mes: celdas por día con intensidad según el gasto.
/// Devuelve el día clicado (1..=31).
pub fn heatmap(ui: &mut Ui, month: &str, daily: &[i64], privacy: bool) -> Option<u32> {
    let pal = p();
    let width = ui.available_width();
    let cols = 7.0;
    let gap = 3.0;
    let cell = ((width - gap * 6.0) / cols).min(34.0);
    let first = util::first_day(month);
    use chrono::Datelike;
    let offset = first.weekday().num_days_from_monday() as usize;
    let days = daily.len();
    let rows = (offset + days).div_ceil(7);
    let head = 16.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(cell * cols + gap * 6.0, head + rows as f32 * (cell + gap)), Sense::click());
    let painter = ui.painter_at(rect.expand(2.0));
    for (i, d) in ["L", "M", "X", "J", "V", "S", "D"].iter().enumerate() {
        let x = rect.left() + i as f32 * (cell + gap) + cell / 2.0;
        painter.text(pos2(x, rect.top()), Align2::CENTER_TOP, *d, theme::mono(10.0), pal.muted);
    }
    let max = daily.iter().copied().max().unwrap_or(1).max(1) as f32;
    let today = util::today();
    let is_cur = month == util::current_month();
    let mut clicked = None;
    for d in 0..days {
        let idx = offset + d;
        let (r, c) = (idx / 7, idx % 7);
        let cr = Rect::from_min_size(pos2(rect.left() + c as f32 * (cell + gap), rect.top() + head + r as f32 * (cell + gap)), vec2(cell, cell));
        let v = daily[d];
        let future = is_cur && (d as u32 + 1) > today.day();
        let fill = if v > 0 { pal.gasto.gamma_multiply(0.18 + 0.82 * (v as f32 / max).sqrt()) } else { pal.surface2 };
        painter.rect_filled(cr, CornerRadius::same(1), if future { Color32::TRANSPARENT } else { fill });
        if future {
            painter.rect_stroke(cr, CornerRadius::same(1), Stroke::new(1.0, pal.line), StrokeKind::Inside);
        }
        if is_cur && d as u32 + 1 == today.day() {
            painter.rect_stroke(cr, CornerRadius::same(1), Stroke::new(1.5, pal.ink), StrokeKind::Inside);
        }
        let txt_col = if v as f32 > max * 0.45 { Color32::WHITE } else { pal.ink2 };
        painter.text(cr.left_top() + vec2(4.0, 3.0), Align2::LEFT_TOP, (d + 1).to_string(), theme::mono(9.5), if future { pal.muted } else { txt_col });
        if let Some(hp) = resp.hover_pos() {
            if cr.contains(hp) {
                let t = if privacy { format!("Día {}", d + 1) } else { format!("Día {}: {}", d + 1, util::eur(v)) };
                resp.clone().on_hover_text_at_pointer(t);
                if resp.clicked() {
                    clicked = Some(d as u32 + 1);
                }
            }
        }
    }
    clicked
}

/// Barras pequeñas por día de la semana.
pub fn weekday(ui: &mut Ui, avg: &[i64; 7], privacy: bool) {
    let pal = p();
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 70.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let max = avg.iter().copied().max().unwrap_or(1).max(1) as f32;
    let slot = rect.width() / 7.0;
    for (i, v) in avg.iter().enumerate() {
        let h = (*v as f32 / max) * (rect.height() - 16.0);
        let cx = rect.left() + slot * (i as f32 + 0.5);
        let r = Rect::from_min_max(pos2(cx - slot * 0.22, rect.bottom() - 14.0 - h), pos2(cx + slot * 0.22, rect.bottom() - 14.0));
        painter.rect_filled(r, CornerRadius::ZERO, pal.ink.gamma_multiply(0.8));
        painter.text(pos2(cx, rect.bottom()), Align2::CENTER_BOTTOM, util::DIAS_CORTOS[i].to_uppercase(), theme::mono(9.0), pal.muted);
        if let Some(hp) = resp.hover_pos() {
            if hp.x > cx - slot / 2.0 && hp.x < cx + slot / 2.0 && !privacy {
                resp.clone().on_hover_text_at_pointer(format!("Media del {}: {}", util::DIAS_CORTOS[i], util::eur(*v)));
            }
        }
    }
}

/// Valor «redondo» para las marcas del eje (1, 2, 5 × 10^n).
fn nice_step(range: f32, ticks: f32) -> f32 {
    let raw = (range / ticks).max(1.0);
    let mag = 10f32.powf(raw.log10().floor());
    let n = raw / mag;
    let m = if n <= 1.0 { 1.0 } else if n <= 2.0 { 2.0 } else if n <= 5.0 { 5.0 } else { 10.0 };
    m * mag
}

/// Línea temporal (patrimonio, serie de una categoría…) en trazo de tinta.
/// `values` en céntimos; las marcas del eje son cifras redondas en euros.
pub fn line(ui: &mut Ui, labels: &[String], values: &[i64], height: f32, color: Color32, privacy: bool) -> Response {
    let pal = p();
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    if values.len() < 2 {
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, "Aún no hay datos suficientes", theme::ui_font(13.0), pal.muted);
        return resp;
    }
    let painter = ui.painter_at(rect.expand(3.0));
    let left_axis = if privacy { 8.0 } else { 64.0 };
    let plot = Rect::from_min_max(pos2(rect.left() + left_axis, rect.top() + 8.0), pos2(rect.right() - 6.0, rect.bottom() - 18.0));
    let lo_e = *values.iter().min().unwrap() as f32 / 100.0;
    let hi_e = *values.iter().max().unwrap() as f32 / 100.0;
    let step = nice_step((hi_e - lo_e).max(1.0), 3.0);
    let lo = (lo_e / step).floor() * step;
    let hi = ((hi_e / step).ceil() * step).max(lo + step);
    let y_of = |e: f32| plot.bottom() - (e - lo) / (hi - lo) * plot.height();
    // Marcas cortas en el eje, sin rejilla de fondo.
    let mut t = lo;
    while t <= hi + 0.01 {
        let y = y_of(t);
        painter.line_segment([pos2(plot.left() - 4.0, y), pos2(plot.left(), y)], Stroke::new(1.0, pal.line_strong));
        if !privacy {
            painter.text(pos2(plot.left() - 8.0, y), Align2::RIGHT_CENTER, util::fmt_num((t * 100.0) as i64).replace(",00", ""), theme::mono(10.0), pal.muted);
        }
        t += step;
    }
    painter.line_segment([plot.left_top(), plot.left_bottom()], Stroke::new(1.0, pal.line));
    painter.line_segment([plot.left_bottom(), plot.right_bottom()], Stroke::new(1.0, pal.line));
    let n = values.len();
    let pts: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| pos2(plot.left() + plot.width() * i as f32 / (n - 1) as f32, y_of(*v as f32 / 100.0)))
        .collect();
    painter.add(Shape::line(pts.clone(), Stroke::new(1.4, color)));
    for i in [0, n / 2, n - 1] {
        if let Some(l) = labels.get(i) {
            let al = if i == 0 { Align2::LEFT_TOP } else if i == n - 1 { Align2::RIGHT_TOP } else { Align2::CENTER_TOP };
            painter.text(pos2(pts[i].x, plot.bottom() + 4.0), al, l, theme::mono(10.0), pal.muted);
        }
    }
    if let Some(hp) = resp.hover_pos() {
        let i = (((hp.x - plot.left()) / plot.width()) * (n - 1) as f32).round().clamp(0.0, (n - 1) as f32) as usize;
        painter.line_segment([pos2(pts[i].x, plot.top()), pos2(pts[i].x, plot.bottom())], Stroke::new(1.0, pal.line_strong));
        painter.circle_filled(pts[i], 3.5, pal.signal);
        if !privacy {
            resp.clone().on_hover_text_at_pointer(format!("{}: {}", labels.get(i).cloned().unwrap_or_default(), util::eur(values[i])));
        }
    }
    resp
}

/// Ritmo de gasto: gasto acumulado día a día del mes frente al mes anterior,
/// con el presupuesto total como referencia. `elapsed` = días ya transcurridos.
pub fn pace(ui: &mut Ui, cur: &[i64], prev: &[i64], budget: i64, elapsed: usize, prev_label: &str, privacy: bool) {
    let pal = p();
    // Leyenda: el color nunca va solo.
    ui.horizontal(|ui| {
        let item = |ui: &mut Ui, c: Color32, w_: f32, t: &str| {
            let (r, _) = ui.allocate_exact_size(vec2(18.0, 10.0), Sense::hover());
            ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(w_, c));
            ui.label(egui::RichText::new(t).font(theme::mono(11.0)).color(pal.ink2));
            ui.add_space(10.0);
        };
        item(ui, pal.ink, 2.2, "este mes");
        item(ui, pal.muted, 1.2, prev_label);
        if budget > 0 {
            item(ui, pal.signal, 1.0, "presupuesto");
        }
    });
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 150.0), Sense::hover());
    let painter = ui.painter_at(rect.expand(3.0));
    let acc = |v: &[i64]| -> Vec<i64> {
        let mut s = 0;
        v.iter().map(|x| {
            s += x;
            s
        }).collect()
    };
    let c = acc(cur);
    let pv = acc(prev);
    let days = cur.len().max(prev.len()).max(28);
    let max = c.iter().chain(pv.iter()).copied().max().unwrap_or(0).max(budget).max(1) as f32 * 1.08;
    let left_axis = if privacy { 6.0 } else { 56.0 };
    let plot = Rect::from_min_max(pos2(rect.left() + left_axis, rect.top() + 4.0), pos2(rect.right() - 4.0, rect.bottom() - 16.0));
    let x_of = |d: usize| plot.left() + plot.width() * d as f32 / (days - 1) as f32;
    let y_of = |v: i64| plot.bottom() - v as f32 / max * plot.height();
    painter.line_segment([plot.left_bottom(), plot.right_bottom()], Stroke::new(1.0, pal.line));
    painter.line_segment([plot.left_top(), plot.left_bottom()], Stroke::new(1.0, pal.line));
    if !privacy {
        // Marcas en cifras redondas (1-2-5 × 10^n euros).
        let step = nice_step(max / 100.0, 3.0);
        let mut t = 0.0f32;
        while t * 100.0 <= max {
            let v = (t * 100.0) as i64;
            painter.line_segment([pos2(plot.left() - 4.0, y_of(v)), pos2(plot.left(), y_of(v))], Stroke::new(1.0, pal.line_strong));
            painter.text(pos2(plot.left() - 8.0, y_of(v)), Align2::RIGHT_CENTER, util::fmt_num(v).replace(",00", ""), theme::mono(10.0), pal.muted);
            t += step;
        }
    }
    for d in [1usize, 8, 15, 22, 29] {
        if d <= days {
            painter.text(pos2(x_of(d - 1), plot.bottom() + 3.0), Align2::CENTER_TOP, d.to_string(), theme::mono(10.0), pal.muted);
        }
    }
    if budget > 0 {
        let y = y_of(budget);
        painter.add(Shape::dashed_line(&[pos2(plot.left(), y), pos2(plot.right(), y)], Stroke::new(1.0, pal.signal), 5.0, 4.0));
    }
    let pts_prev: Vec<Pos2> = pv.iter().enumerate().map(|(i, v)| pos2(x_of(i), y_of(*v))).collect();
    if pts_prev.len() > 1 {
        painter.add(Shape::line(pts_prev, Stroke::new(1.2, pal.muted)));
    }
    let n = elapsed.min(c.len());
    let pts_cur: Vec<Pos2> = c.iter().take(n).enumerate().map(|(i, v)| pos2(x_of(i), y_of(*v))).collect();
    if pts_cur.len() > 1 {
        painter.add(Shape::line(pts_cur.clone(), Stroke::new(2.2, pal.ink)));
    }
    if let Some(last) = pts_cur.last() {
        painter.circle_filled(*last, 3.5, pal.signal);
    }
    if let Some(hp) = resp.hover_pos() {
        let d = (((hp.x - plot.left()) / plot.width()) * (days - 1) as f32).round().clamp(0.0, (days - 1) as f32) as usize;
        painter.line_segment([pos2(x_of(d), plot.top()), pos2(x_of(d), plot.bottom())], Stroke::new(1.0, pal.line_strong));
        if !privacy {
            let mut t = format!("Día {}", d + 1);
            if d < n {
                t.push_str(&format!("\neste mes: {}", util::eur(c[d])));
            }
            if let Some(v) = pv.get(d) {
                t.push_str(&format!("\n{prev_label}: {}", util::eur(*v)));
            }
            resp.clone().on_hover_text_at_pointer(t);
        }
    }
}

/// Mini línea de precio (30 días) para una posición.
pub fn spark(ui: &mut Ui, values: &[f64], w: f32, h: f32) {
    let pal = p();
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    if values.len() < 2 {
        return;
    }
    let lo = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = (hi - lo).max(1e-9);
    let n = values.len();
    let pts: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| pos2(rect.left() + rect.width() * i as f32 / (n - 1) as f32, rect.bottom() - ((v - lo) / span) as f32 * rect.height()))
        .collect();
    let col = if values[n - 1] >= values[0] { pal.positive } else { pal.gasto };
    ui.painter().add(Shape::line(pts, Stroke::new(1.3, col)));
}

/// Anillo de reparto (gasto o ingreso por categoría). `focus` resalta una
/// porción desde fuera (la leyenda). Devuelve la porción bajo el ratón y la
/// pulsada. En el hueco: la porción resaltada o, si no hay, el total.
pub struct Slice {
    pub color: Color32,
    pub value: i64,
    /// Texto del hueco cuando la porción está resaltada (importe y nombre).
    pub big: String,
    pub small: String,
}

/// Sector de corona como malla: bordes interior y exterior suavizados con una
/// franja de `f` px que se desvanece. Los cortes radiales son exactos (ángulos
/// compartidos con la porción vecina), así que las porciones encajan sin hueco.
fn ring_sector(mesh: &mut egui::Mesh, c: Pos2, r0: f32, r1: f32, a0: f32, a1: f32, col: Color32, f: f32) {
    let radii = [r0 - f / 2.0, r0 + f / 2.0, r1 - f / 2.0, r1 + f / 2.0];
    let alpha = [0.0, 1.0, 1.0, 0.0];
    // Segmentos de ~2 px en el borde exterior: curva lisa a cualquier tamaño.
    let steps = (((a1 - a0) * r1) / 2.0).ceil().max(2.0) as u32;
    let base = mesh.vertices.len() as u32;
    for k in 0..=steps {
        let a = a0 + (a1 - a0) * k as f32 / steps as f32;
        let dir = vec2(a.cos(), a.sin());
        for j in 0..4 {
            mesh.colored_vertex(c + dir * radii[j], col.gamma_multiply(alpha[j]));
        }
    }
    for k in 0..steps {
        for j in 0..3u32 {
            let i0 = base + k * 4 + j;
            let i1 = i0 + 4;
            mesh.add_triangle(i0, i0 + 1, i1);
            mesh.add_triangle(i0 + 1, i1 + 1, i1);
        }
    }
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

pub fn donut(ui: &mut Ui, slices: &[Slice], size: f32, center: (&str, &str), focus: Option<usize>) -> (Option<usize>, Option<usize>) {
    use egui::emath::easing;
    let pal = p();
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let ctx = ui.ctx().clone();
    let id = resp.id;
    let c = rect.center();
    const GROW: f32 = 6.0;
    let r_out = size / 2.0 - GROW - 1.0;
    let r_in = r_out * 0.66;
    let total: i64 = slices.iter().map(|s| s.value.max(0)).sum();
    let tau = std::f32::consts::TAU;
    let start = -tau / 4.0;

    // Entrada: al cambiar los datos (otro mes) el anillo se dibuja en barrido.
    let key = slices.iter().fold(total as u64, |h, s| h.wrapping_mul(31).wrapping_add(s.value as u64));
    let now = ctx.input(|i| i.time);
    let (last_key, t0) = ctx.data(|d| d.get_temp::<(u64, f64)>(id.with("sweep"))).unwrap_or((0, now));
    let t0 = if last_key != key { now } else { t0 };
    ctx.data_mut(|d| d.insert_temp(id.with("sweep"), (key, t0)));
    let sweep_t = (((now - t0) / 0.55) as f32).clamp(0.0, 1.0);
    if sweep_t < 1.0 {
        ctx.request_repaint();
    }
    let sweep = easing::cubic_out(sweep_t);

    // Porción bajo el ratón: por ángulo y radio.
    let mut hovered = None;
    if let Some(pos) = resp.hover_pos() {
        let d = pos - c;
        let dist = d.length();
        if total > 0 && dist >= r_in - 2.0 && dist <= r_out + GROW + 2.0 {
            let ang = (d.y.atan2(d.x) - start).rem_euclid(tau);
            let mut acc = 0.0;
            for (i, s) in slices.iter().enumerate() {
                let frac = s.value.max(0) as f32 / total as f32;
                if ang >= acc * tau && ang < (acc + frac) * tau {
                    hovered = Some(i);
                    break;
                }
                acc += frac;
            }
        }
    }
    let lit = hovered.or(focus);
    let painter = ui.painter_at(rect.expand(2.0));
    let f = 1.0 / ctx.pixels_per_point();
    if total <= 0 {
        let mut mesh = egui::Mesh::default();
        ring_sector(&mut mesh, c, r_in, r_out, 0.0, tau, pal.line, f);
        painter.add(Shape::mesh(mesh));
    } else {
        let mut mesh = egui::Mesh::default();
        let mut cuts = Vec::new();
        let mut a0 = start;
        let mut grows = Vec::new();
        for (i, sl) in slices.iter().enumerate() {
            let a1 = a0 + sl.value.max(0) as f32 / total as f32 * tau * sweep;
            // Resaltado animado: la porción crece y las demás se apagan.
            let on = ctx.animate_bool_with_time_and_easing(id.with(("on", i)), lit == Some(i), 0.18, easing::cubic_out);
            let off = ctx.animate_bool_with_time_and_easing(id.with(("off", i)), lit.is_some() && lit != Some(i), 0.18, easing::cubic_out);
            let col = mix(sl.color, pal.bg, 0.62 * off);
            if a1 > a0 {
                ring_sector(&mut mesh, c, r_in - 2.0 * on, r_out + GROW * on, a0, a1, col, f);
            }
            grows.push(on);
            cuts.push(a0);
            a0 = a1;
        }
        painter.add(Shape::mesh(mesh));
        // Separadores de grosor constante (lados paralelos), del color del
        // fondo, por encima de los cortes: todas las juntas iguales.
        if slices.iter().filter(|s| s.value > 0).count() > 1 {
            if sweep >= 1.0 {
                cuts.push(start + tau);
            }
            for a in cuts.iter().skip(if sweep >= 1.0 { 1 } else { 0 }) {
                let dir = vec2(a.cos(), a.sin());
                painter.line_segment([c + dir * (r_in - 3.0), c + dir * (r_out + GROW + 1.5)], Stroke::new(2.0, pal.bg));
            }
        }
        if grows.iter().any(|g| *g > 0.0 && *g < 1.0) {
            ctx.request_repaint();
        }
    }
    let (big, small) = match lit.and_then(|i| slices.get(i)) {
        Some(sl) => (sl.big.clone(), sl.small.clone()),
        None => (center.0.to_string(), center.1.to_string()),
    };
    painter.text(c - vec2(0.0, 7.0), Align2::CENTER_CENTER, big, theme::mono_bold((size / 11.0).clamp(13.0, 19.0)), pal.ink);
    painter.text(c + vec2(0.0, 13.0), Align2::CENTER_CENTER, small, theme::ui_font(11.5), pal.muted);
    if hovered.is_some() {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let clicked = if resp.clicked() { hovered } else { None };
    (hovered, clicked)
}
