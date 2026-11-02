//! Small Bedov UI hooks; all decisions and edits live in the engine command.
use lightcraft_catalog::PhotoId;
use lightcraft_engine::cmd::bedov_detail::available;
use serde_json::json;

use super::settings::{check, heading, hint};
use crate::LightcraftApp;
use crate::theme::Tokens;
use crate::widgets::register;

pub(crate) fn import_options(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    heading(ui, t, "Bedov Auto Detail");
    let mut on = app.session.import_defaults.bedov_auto_detail;
    let ready = available(&app.session);
    ui.add_enabled_ui(ready || on, |ui| {
        if check(ui, "settings.bedovAutoDetail", &mut on, "Automatic noise reduction and sharpening on import") {
            let _ = app.run("library.preferences", json!({"import": {"bedovAutoDetail": on}}));
        }
    });
    hint(ui, t, "Camera and ISO defaults for new RAW photos. Existing edits stay unchanged; all controls remain editable.");
    hint(ui, t, "Supported: Canon R6 II, Nikon Z6 III, D750 and Z7 II. Other cameras keep their existing defaults.");
    if !ready {
        hint(ui, t, "Install and select RawNIND UtNet2 (Bayer) in Settings > AI Denoise to enable this option.");
    }
    let button = ui.add_enabled(ready && !app.session.selected().is_empty(), egui::Button::new(crate::i18n::tr("Apply to Selected Photos")));
    register(ui, "button:bedov.detail.selected", button.rect);
    if button.clicked() {
        apply(app, json!({}));
    }
    ui.add_space(12.0);
}

pub(crate) fn detail_button(app: &mut LightcraftApp, ui: &mut egui::Ui, id: PhotoId) {
    // Leave the upstream model-installation offer in view when no usable model is ready.
    if !available(&app.session) {
        return;
    }
    let r = ui.button(crate::i18n::tr("Bedov Auto Detail"));
    register(ui, "button:bedov.detail.active", r.rect);
    let r = r.on_hover_text(crate::i18n::tr(
        "Apply camera/ISO noise reduction and sharpening to this photo. Saves a previous version. Photo menu: apply to all selected.",
    ));
    if r.clicked() {
        apply(app, json!({"ids": [id.0]}));
    }
    ui.add_space(6.0);
}

fn apply(app: &mut LightcraftApp, params: serde_json::Value) {
    if let Ok(result) = app.run("bedov.detail.apply", params) {
        let changed = result["changed"].as_u64().unwrap_or(0);
        let skipped = result["skipped"].as_array().map_or(0, Vec::len);
        app.ui.status = format!("Bedov Auto Detail: {changed} changed, {skipped} skipped. Previous settings are in Versions.");
    }
}
