//! Bedov's opt-in starting detail treatment. Keep policy here, outside upstream renderers.
//! These are photographic defaults, not a claim of measured camera/Lightroom parity.
use lightcraft_catalog::{Op, Photo, Version};
use lightcraft_develop::DevelopSettings;
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_selection, photo_ids};
use crate::{Result, Session};

const APPLY: &str = "bedov.detail.apply";
const LABEL: &str = "Bedov Auto Detail";
const MODEL: &str = "rawnind-bayer";
const MODEL_SHA: &str = "da27509dab6a2915da67e988acd86cf71f9d5bbc8d1aa0ed32933578a887b901";

#[derive(Clone, Copy, Debug, PartialEq)]
enum CameraClass {
    FullFrame24,
    FullFrameHighResolution,
}

impl CameraClass {
    fn name(self) -> &'static str {
        match self {
            Self::FullFrame24 => "Full-frame 24 MP",
            Self::FullFrameHighResolution => "Full-frame high resolution",
        }
    }
}

fn camera_class(camera: &str) -> Option<CameraClass> {
    let name: String = camera.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_uppercase()).collect();
    if ["CANONEOSR6M2", "CANONEOSR6MARKII", "NIKONZ63", "NIKONZ6III", "NIKOND750"].iter().any(|s| name.ends_with(s)) {
        Some(CameraClass::FullFrame24)
    } else if ["NIKONZ72", "NIKONZ7II"].iter().any(|s| name.ends_with(s)) {
        Some(CameraClass::FullFrameHighResolution)
    } else {
        // Never infer sensor size or a Bayer layout from file dimensions alone.
        None
    }
}

/// Cheap UI/import availability check. The existing denoise pump refreshes the selected model.
pub fn available(s: &Session) -> bool {
    cfg!(feature = "denoise")
        && s.denoise_products_dir().is_some()
        && s.denoise.active.as_ref().is_some_and(|a| a.id == MODEL && a.manifest.sha256.as_deref() == Some(MODEL_SHA))
}

fn recommendation(p: &Photo, before: &DevelopSettings) -> std::result::Result<(DevelopSettings, CameraClass), &'static str> {
    if !crate::denoise::eligible(p) || !matches!(p.format.to_ascii_uppercase().as_str(), "CR3" | "NEF") {
        return Err("Requires an original supported Canon CR3 or Nikon NEF");
    }
    let class = camera_class(&p.meta.camera).ok_or("No camera class configured for this model")?;
    let iso = p.meta.iso.filter(|iso| *iso > 0).ok_or("ISO metadata is missing")?;
    if !before.section_enabled("detail") {
        return Err("Detail is disabled for this photo");
    }
    let amount = if iso >= 3200 {
        100.0
    } else if iso >= 1600 {
        80.0
    } else {
        0.0
    };
    let mut d = before.clone();
    d.enhance.denoise = amount;
    d.enhance.denoise_on = Some(amount > 0.0);
    d.detail.nr_luminance = 0.0;
    d.detail.nr_color = if amount > 0.0 { 0.0 } else { 25.0 };
    d.detail.nr_detail = 50.0;
    d.detail.nr_contrast = 0.0;
    d.detail.nr_color_detail = 50.0;
    d.detail.nr_color_smoothness = 50.0;
    d.detail.sharpen_amount = match (class, amount > 0.0) {
        (CameraClass::FullFrame24, false) => 25.0,
        (CameraClass::FullFrame24, true) => 20.0,
        (CameraClass::FullFrameHighResolution, false) => 30.0,
        (CameraClass::FullFrameHighResolution, true) => 22.0,
    };
    d.detail.sharpen_radius = if class == CameraClass::FullFrame24 { 0.8 } else { 0.7 };
    d.detail.sharpen_detail = 15.0;
    d.detail.sharpen_masking = if amount > 0.0 { 90.0 } else { 70.0 };
    Ok((d, class))
}

/// Runs before camera presets and XMP: explicit user processing keeps precedence.
pub(crate) fn import_start(s: &Session, p: &Photo, base: DevelopSettings) -> DevelopSettings {
    if s.import_defaults.bedov_auto_detail && available(s) {
        match recommendation(p, &base) {
            Ok((d, _)) => d,
            Err(_) => base,
        }
    } else {
        base
    }
}

fn selected(s: &Session, p: &Value, command: &str) -> Result<Vec<lightcraft_catalog::PhotoId>> {
    let mut ids = photo_ids(p, command)?.unwrap_or_else(|| s.selected());
    ids.sort_unstable();
    ids.dedup();
    for id in &ids {
        if s.catalog.photo(*id).is_none() {
            return Err(bad(command, format!("photo {} is not in this library", id.0)));
        }
    }
    Ok(ids)
}

fn preview(s: &mut Session, p: &Value) -> Result<Value> {
    s.denoise_refresh_active(false);
    let ids = selected(s, p, "bedov.detail.preview")?;
    let photos: Vec<_> = ids
        .iter()
        .filter_map(|id| s.catalog.photo(*id))
        .map(|photo| match recommendation(photo, &photo.develop) {
            Ok((d, class)) => json!({"id": photo.id.0, "fileName": photo.file_name, "cameraClass": class.name(),
                "iso": photo.meta.iso, "denoise": d.enhance.denoise, "sharpen": d.detail.sharpen_amount}),
            Err(reason) => json!({"id": photo.id.0, "fileName": photo.file_name, "skip": reason}),
        })
        .collect();
    Ok(json!({"available": available(s), "onImport": s.import_defaults.bedov_auto_detail, "photos": photos}))
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    // Refuse a gesture before building any undo/version operations.
    if s.interaction.is_some() {
        return Err(bad(APPLY, "finish the current slider adjustment first"));
    }
    let ids = selected(s, p, APPLY)?;
    s.denoise_refresh_active(false);
    if !available(s) {
        return Err(bad(APPLY, "install and select RawNIND UtNet2 (Bayer) in Settings > AI Denoise, in a library on disk"));
    }
    apply_ready(s, ids)
}

fn apply_ready(s: &mut Session, ids: Vec<lightcraft_catalog::PhotoId>) -> Result<Value> {
    let mut ops = Vec::new();
    let mut changed = Vec::new();
    let mut skipped = Vec::new();
    for id in ids {
        let Some(p) = s.catalog.photo(id) else { continue };
        let (d, _) = match recommendation(p, &p.develop) {
            Ok(r) => r,
            Err(reason) => {
                skipped.push(json!({"id": id.0, "reason": reason}));
                continue;
            }
        };
        if *p.develop == d {
            continue;
        }
        let mut versions = p.versions.clone();
        versions.push(Version { name: "Before Bedov Auto Detail".into(), created: (s.clock)(), settings: p.develop.clone(), auto: false });
        ops.push(Op::SetVersions { id, versions });
        ops.extend(s.develop_op(id, d, LABEL));
        changed.push(id.0);
    }
    if !ops.is_empty() {
        // Explicit targets only, independent of Auto Sync; one undo restores edits AND versions.
        s.commit(LABEL, Op::Batch { ops })?;
    }
    Ok(json!({"changed": changed.len(), "ids": changed, "skipped": skipped}))
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            explicit_targets: true,
            ..cmd!(query "bedov.detail.preview", "Preview Bedov Auto Detail", [], None,
            "{ids?: [photoId]} → {available, onImport, photos}: camera/ISO recommendations; no edits or inference", has_selection, preview)
        },
        CommandSpec {
            explicit_targets: true,
            ..cmd!(
                "bedov.detail.apply",
                "Apply Bedov Auto Detail to Selected",
                ["Photo"],
                None,
                "{ids?: [photoId]} → {changed, ids, skipped}: set camera/ISO noise and sharpening defaults, save previous versions, one undo; ignores Auto Sync",
                has_selection,
                apply
            )
        },
    ]
}

#[cfg(test)]
#[path = "bedov_detail_tests.rs"]
mod tests;
