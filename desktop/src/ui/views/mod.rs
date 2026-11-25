//! Una función por sección. Todas reciben el espacio de trabajo completo.

mod ajustes;
mod anual;
mod cuentas;
pub mod informe;
mod inversiones;
mod mes;
pub mod movimientos;
mod objetivos;
mod presupuestos;
mod recurrentes;

use egui::Ui;

use super::workspace::{View, Workspace};

#[derive(Default)]
pub struct ViewStates {
    pub mov: movimientos::MovState,
    pub anual_year: Option<i32>,
    pub informe_month: Option<String>,
    pub ajustes: ajustes::AjustesState,
}

pub fn show(ws: &mut Workspace, ui: &mut Ui) {
    match ws.view {
        View::Mes => mes::show(ws, ui),
        View::Movimientos => movimientos::show(ws, ui),
        View::Cuentas => cuentas::show(ws, ui),
        View::Presupuestos => presupuestos::show(ws, ui),
        View::Recurrentes => recurrentes::show(ws, ui),
        View::Inversiones => inversiones::show(ws, ui),
        View::Objetivos => objetivos::show(ws, ui),
        View::Anual => anual::show(ws, ui),
        View::Informe => informe::show(ws, ui),
        View::Ajustes => ajustes::show(ws, ui),
    }
}

/// Área con scroll vertical que ocupa el resto de la sección.
pub fn scroll(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin { left: 0, right: 16, top: 2, bottom: 28 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    });
}
