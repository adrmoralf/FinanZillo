#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod db;
mod demo;
mod ledger;
mod store;
mod ui;
mod util;

/// `finanzillo --dump <base.db> <mes>…`: vuelca cálculos en JSON para
/// compararlos con la app web (verificación de paridad; no abre ventana).
fn dump(args: &[String]) -> Result<(), String> {
    let path = std::path::Path::new(args.first().ok_or("falta la ruta de la base")?);
    let conn = db::import_sqlite_file(path)?;
    let mut core = ledger::Core::new(conn);
    core.apply_pending();
    let accounts: Vec<_> = core
        .accounts()
        .into_iter()
        .map(|a| serde_json::json!({"name": a.name, "current_cents": a.current_cents, "interest_cents": a.interest_cents}))
        .collect();
    let mut months = serde_json::Map::new();
    for m in &args[1..] {
        let s = core.summary(m);
        months.insert(
            m.clone(),
            serde_json::json!({
                "ingresos_cents": s.ingresos_cents, "gastos_cents": s.gastos_cents, "count": s.count,
                "spend_days": s.spend_days, "gastos_recurrentes_cents": s.gastos_recurrentes_cents,
                "pending_recurring_cents": s.pending_recurring_cents,
                "by_category": s.by_category.iter().map(|c| (c.name.clone(), c.total_cents)).collect::<Vec<_>>(),
                "budgets": s.budgets.iter().map(|b| (b.name.clone(), b.budget_cents)).collect::<Vec<_>>(),
                "global_budget_month_cents": s.global_budget_cents,
            }),
        );
    }
    let year = core.year_view(util::month_parts(&util::current_month()).0);
    let nw = core.networth();
    let out = serde_json::json!({
        "accounts": accounts,
        "months": months,
        "year": {"ingresos_cents": year.ingresos_cents, "gastos_cents": year.gastos_cents},
        "networth": {"points": nw.len(), "last_total": nw.last().map(|p| p.total()), "first_date": nw.first().map(|p| p.date.clone())},
        "suggestions": core.recurring_suggestions().len(),
        "transactions": core.transactions(&ledger::TxFilter { limit: 5000, ..Default::default() }).total_count,
    });
    println!("{}", serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?);
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--demo") {
        // Base de demostración con datos ficticios, importable al crear el archivo.
        let path = std::path::PathBuf::from(args.get(1).cloned().unwrap_or_else(|| "finanzillo-demo.db".into()));
        match demo::write(&path) {
            Ok(()) => println!("Base de demostración creada: {}", path.display()),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.first().map(String::as_str) == Some("--dump") {
        if let Err(e) = dump(&args[1..]) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png")).ok();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("FinanZillo")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([1120.0, 720.0])
        .with_app_id("finanzillo");
    if let Some(i) = icon {
        viewport = viewport.with_icon(std::sync::Arc::new(i));
    }
    // Solo en depuración: tamaño de ventana fijo para las capturas (FZ_SIZE=1920x1080).
    #[cfg(debug_assertions)]
    if let Some((w, h)) = std::env::var("FZ_SIZE").ok().and_then(|s| {
        let (a, b) = s.split_once('x')?;
        Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?))
    }) {
        viewport = viewport.with_inner_size([w, h]);
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, persist_window: true, ..Default::default() };
    // En las capturas con tamaño fijo no se restaura la ventana guardada.
    #[cfg(debug_assertions)]
    if std::env::var("FZ_SIZE").is_ok() {
        options.persist_window = false;
    }
    if let Err(e) = eframe::run_native("FinanZillo", options, Box::new(|cc| Ok(Box::new(app::App::new(cc))))) {
        let _ = rfd::MessageDialog::new()
            .set_title("FinanZillo")
            .set_description(format!("No se pudo abrir la ventana: {e}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
