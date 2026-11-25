//! Importes, fechas y textos en formato es-ES.

use chrono::{Datelike, Local, NaiveDate};

pub const MESES: [&str; 12] = [
    "enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre",
    "noviembre", "diciembre",
];
pub const MESES_CORTOS: [&str; 12] =
    ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"];
pub const DIAS_CORTOS: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn today_str() -> String {
    fmt_date(today())
}

pub fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    if s.len() != 10 {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

pub fn current_month() -> String {
    today().format("%Y-%m").to_string()
}

/// "2026-11" → (2026, 11)
pub fn month_parts(month: &str) -> (i32, u32) {
    let y = month.get(0..4).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let m = month.get(5..7).and_then(|s| s.parse().ok()).unwrap_or(1);
    (y, m.clamp(1, 12))
}

pub fn month_str(y: i32, m: u32) -> String {
    format!("{y:04}-{m:02}")
}

/// Suma `delta` meses a "YYYY-MM".
pub fn add_months(month: &str, delta: i32) -> String {
    let (y, m) = month_parts(month);
    let total = y * 12 + (m as i32 - 1) + delta;
    month_str(total.div_euclid(12), (total.rem_euclid(12) + 1) as u32)
}

pub fn days_in_month(month: &str) -> u32 {
    let (y, m) = month_parts(month);
    let next = if m == 12 { NaiveDate::from_ymd_opt(y + 1, 1, 1) } else { NaiveDate::from_ymd_opt(y, m + 1, 1) };
    next.and_then(|n| n.pred_opt()).map(|d| d.day()).unwrap_or(30)
}

pub fn last_day_of_month(month: &str) -> String {
    format!("{month}-{:02}", days_in_month(month))
}

pub fn first_day(month: &str) -> NaiveDate {
    let (y, m) = month_parts(month);
    NaiveDate::from_ymd_opt(y, m, 1).unwrap_or_else(today)
}

/// "noviembre de 2026"
pub fn month_label(month: &str) -> String {
    let (y, m) = month_parts(month);
    format!("{} de {y}", MESES[(m - 1) as usize])
}

pub fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Fecha legible: "lun 24 nov"
pub fn date_label(d: &str) -> String {
    match parse_date(d) {
        Some(nd) => format!(
            "{} {} {}",
            DIAS_CORTOS[nd.weekday().num_days_from_monday() as usize],
            nd.day(),
            MESES_CORTOS[(nd.month() - 1) as usize]
        ),
        None => d.to_string(),
    }
}

/// Agrupa miles con punto a partir de 5 cifras, como `Intl.NumberFormat('es-ES')`.
fn group(int_part: u64) -> String {
    let s = int_part.to_string();
    if s.len() < 5 {
        return s;
    }
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(ch);
    }
    out
}

/// 123456 → "1234,56" ; -5 → "-0,05"
pub fn fmt_num(cents: i64) -> String {
    let neg = cents < 0;
    let a = cents.unsigned_abs();
    format!("{}{},{:02}", if neg { "-" } else { "" }, group(a / 100), a % 100)
}

/// "1234,56 €"
pub fn eur(cents: i64) -> String {
    format!("{} €", fmt_num(cents))
}

/// Con signo explícito: "+122,15 €" / "−58,40 €" (signo menos tipográfico).
pub fn eur_signed(cents: i64) -> String {
    if cents > 0 {
        format!("+{}", eur(cents))
    } else if cents < 0 {
        format!("−{}", eur(-cents))
    } else {
        eur(0)
    }
}

/// Acepta 12.5, "12,50", "1.234,56", "1234.56", "12€"... → céntimos.
pub fn parse_amount_cents(value: &str) -> Option<i64> {
    let mut s: String = value.chars().filter(|c| !c.is_whitespace() && *c != '€').collect();
    if s.is_empty() {
        return None;
    }
    if s.contains(',') {
        s = s.replace('.', "").replacen(',', ".", 1);
    }
    let f: f64 = s.parse().ok()?;
    if !f.is_finite() {
        return None;
    }
    Some((f * 100.0).round() as i64)
}

/// Importe que admite operaciones: "12,50+3*2" → 1850 céntimos.
/// Sin operadores se comporta como `parse_amount_cents`.
pub fn eval_amount_cents(input: &str) -> Option<i64> {
    let s: String = input.chars().filter(|c| !c.is_whitespace() && *c != '€').collect();
    let body = s.trim_start_matches(['+', '-']);
    if !body.contains(['+', '-', '*', '/', 'x', '×', '(', ')']) {
        return parse_amount_cents(&s);
    }
    // Analizador recursivo sencillo; los números usan coma o punto decimal.
    struct P<'a> {
        b: &'a [u8],
        i: usize,
    }
    impl P<'_> {
        fn peek(&self) -> Option<u8> {
            self.b.get(self.i).copied()
        }
        fn expr(&mut self) -> Option<f64> {
            let mut v = self.term()?;
            while let Some(c) = self.peek() {
                match c {
                    b'+' => {
                        self.i += 1;
                        v += self.term()?
                    }
                    b'-' => {
                        self.i += 1;
                        v -= self.term()?
                    }
                    _ => break,
                }
            }
            Some(v)
        }
        fn term(&mut self) -> Option<f64> {
            let mut v = self.factor()?;
            while let Some(c) = self.peek() {
                match c {
                    b'*' | b'x' => {
                        self.i += 1;
                        v *= self.factor()?
                    }
                    b'/' => {
                        self.i += 1;
                        let d = self.factor()?;
                        if d == 0.0 {
                            return None;
                        }
                        v /= d
                    }
                    _ => break,
                }
            }
            Some(v)
        }
        fn factor(&mut self) -> Option<f64> {
            match self.peek()? {
                b'(' => {
                    self.i += 1;
                    let v = self.expr()?;
                    if self.peek()? != b')' {
                        return None;
                    }
                    self.i += 1;
                    Some(v)
                }
                b'-' => {
                    self.i += 1;
                    Some(-self.factor()?)
                }
                _ => {
                    let start = self.i;
                    while self.peek().is_some_and(|c| c.is_ascii_digit() || c == b',' || c == b'.') {
                        self.i += 1;
                    }
                    let num = std::str::from_utf8(&self.b[start..self.i]).ok()?;
                    let n = if num.contains(',') { num.replace('.', "").replace(',', ".") } else { num.to_string() };
                    n.parse().ok()
                }
            }
        }
    }
    let norm = s.replace('×', "*");
    let mut p = P { b: norm.as_bytes(), i: 0 };
    let v = p.expr()?;
    if p.i != p.b.len() || !v.is_finite() {
        return None;
    }
    Some((v * 100.0).round() as i64)
}

pub fn pct(part: i64, total: i64) -> i64 {
    if total <= 0 {
        0
    } else {
        ((part as f64 / total as f64) * 100.0).round() as i64
    }
}

pub fn fmt_apy(apy: f64) -> String {
    let s = format!("{apy}");
    s.replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formato_es() {
        assert_eq!(fmt_num(160300), "1603,00");
        assert_eq!(fmt_num(1234567), "12.345,67");
        assert_eq!(fmt_num(-5), "-0,05");
        assert_eq!(eur_signed(12215), "+122,15 €");
    }

    #[test]
    fn importes() {
        assert_eq!(parse_amount_cents("12,50"), Some(1250));
        assert_eq!(parse_amount_cents("1.234,56"), Some(123456));
        assert_eq!(parse_amount_cents("1234.56"), Some(123456));
        assert_eq!(parse_amount_cents("12€"), Some(1200));
        assert_eq!(parse_amount_cents(""), None);
        assert_eq!(parse_amount_cents("abc"), None);
    }

    #[test]
    fn calculadora() {
        assert_eq!(eval_amount_cents("12,50+3*2"), Some(1850));
        assert_eq!(eval_amount_cents("(10+5)/3"), Some(500));
        assert_eq!(eval_amount_cents("1.234,56"), Some(123456));
        assert_eq!(eval_amount_cents("10/0"), None);
        assert_eq!(eval_amount_cents("3x2,5"), Some(750));
    }

    #[test]
    fn meses() {
        assert_eq!(add_months("2026-01", -1), "2025-12");
        assert_eq!(add_months("2026-11", 2), "2027-01");
        assert_eq!(days_in_month("2028-02"), 29);
        assert_eq!(last_day_of_month("2026-11"), "2026-11-30");
    }
}
