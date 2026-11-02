use super::*;
use lightcraft_catalog::{MediaKind, PhotoId, Source};
use std::sync::Arc;

fn photo(id: u64, camera: &str, iso: Option<u32>) -> Photo {
    let mut p = Photo::new(PhotoId(id), Source::File { path: format!("/test/{id}.NEF") }, "test.NEF", "NEF", 6000, 4000, "2026-11-02");
    p.kind = MediaKind::Raw;
    p.meta.camera = camera.into();
    p.meta.iso = iso;
    p
}

fn session() -> Session {
    let mut s = Session::new();
    for id in 1..=3 {
        let mut p = photo(id, "NIKON CORPORATION NIKON Z6_3", Some(6400));
        let mut d = (*p.develop).clone();
        d.light.exposure = id as f64 / 10.0;
        d.effects.clarity = 18.0;
        d.enhance.raw_details = true;
        p.develop = Arc::new(d);
        s.commit("Fixture", Op::AddPhoto { photo: Box::new(p) }).unwrap();
    }
    s.execute("library.select", &json!({"ids": [1,2,3], "active": 1})).unwrap();
    s.auto_sync = true;
    s
}

#[test]
fn identifies_actual_camera_names_without_confusing_d750_with_d7500() {
    for name in ["Canon Canon EOS R6m2", "Canon EOS R6 Mark II", "NIKON CORPORATION NIKON Z6_3", "Nikon Z6 III", "NIKON D750"] {
        assert_eq!(camera_class(name), Some(CameraClass::FullFrame24), "{name}");
    }
    assert_eq!(camera_class("NIKON CORPORATION NIKON Z 7_2"), Some(CameraClass::FullFrameHighResolution));
    assert_eq!(camera_class("NIKON D7500"), None);
    assert_eq!(camera_class(""), None);
}

#[test]
fn iso_boundaries_avoid_stacked_denoise_and_keep_missing_iso_untouched() {
    for (iso, expected) in [(100, 0.0), (1599, 0.0), (1600, 80.0), (3199, 80.0), (3200, 100.0), (16000, 100.0)] {
        let p = photo(1, "NIKON D750", Some(iso));
        let (d, _) = recommendation(&p, &p.develop).unwrap();
        assert_eq!(d.enhance.denoise, expected);
        assert_eq!(d.detail.nr_luminance, 0.0);
        assert_eq!(d.detail.nr_color, if expected > 0.0 { 0.0 } else { 25.0 });
        assert_eq!(d.enhance.denoise_enabled(), expected > 0.0);
    }
    for iso in [None, Some(0)] {
        let p = photo(1, "NIKON D750", iso);
        assert!(recommendation(&p, &p.develop).is_err());
    }
}

#[test]
fn detail_defaults_preserve_every_other_setting_and_are_idempotent() {
    let s = session();
    let p = s.catalog.photo(PhotoId(1)).unwrap();
    let (mut d, _) = recommendation(p, &p.develop).unwrap();
    assert_eq!(recommendation(p, &d).unwrap().0, d);
    d.detail = p.develop.detail;
    d.enhance.denoise = p.develop.enhance.denoise;
    d.enhance.denoise_on = p.develop.enhance.denoise_on;
    assert_eq!(d, *p.develop);
}

#[test]
fn skips_nonraw_linear_dng_unknown_cameras_and_disabled_detail() {
    let mut p = photo(1, "NIKON D750", Some(6400));
    p.format = "DNG".into();
    assert!(recommendation(&p, &p.develop).is_err(), "DNG may already be demosaiced");
    p.format = "NEF".into();
    p.kind = MediaKind::Image;
    assert!(recommendation(&p, &p.develop).is_err());
    p.kind = MediaKind::Raw;
    p.meta.camera = "Unknown".into();
    assert!(recommendation(&p, &p.develop).is_err());
    p.meta.camera = "NIKON D750".into();
    let mut d = (*p.develop).clone();
    d.disabled_sections.push("detail".into());
    assert!(recommendation(&p, &d).is_err());
    p.deleted = true;
    assert!(recommendation(&p, &p.develop).is_err());
}

#[test]
fn explicit_targets_ignore_auto_sync_save_versions_and_undo_together() {
    let mut s = session();
    let originals: Vec<_> = (1..=3).map(|id| s.catalog.photo(PhotoId(id)).unwrap().clone()).collect();
    let ids = selected(&s, &json!({"ids": [2,2]}), APPLY).unwrap();
    let result = apply_ready(&mut s, ids).unwrap();
    assert_eq!(result["ids"], json!([2]));
    assert_eq!(s.catalog.photo(PhotoId(1)).unwrap().develop, originals[0].develop);
    assert_eq!(s.catalog.photo(PhotoId(3)).unwrap().develop, originals[2].develop);
    let edited = s.catalog.photo(PhotoId(2)).unwrap();
    assert_eq!(edited.versions.last().unwrap().settings, originals[1].develop);
    assert_eq!(edited.develop.light.exposure, originals[1].develop.light.exposure);
    let count = edited.versions.len();
    assert_eq!(apply_ready(&mut s, vec![PhotoId(2)]).unwrap()["changed"], 0);
    assert_eq!(s.catalog.photo(PhotoId(2)).unwrap().versions.len(), count);
    s.execute("edit.undo", &json!({})).unwrap();
    for p in originals {
        let now = s.catalog.photo(p.id).unwrap();
        assert_eq!(now.develop, p.develop);
        // Normal selection changes may add the engine's automatic snapshots.
        let manual = |versions: &[Version]| versions.iter().filter(|v| !v.auto).cloned().collect::<Vec<_>>();
        assert_eq!(manual(&now.versions), manual(&p.versions));
        assert_eq!(now.history, p.history);
    }
}

#[test]
fn invalid_target_lists_and_missing_models_do_not_change_photos() {
    let mut s = session();
    let before = s.catalog.photo(PhotoId(1)).unwrap().develop.clone();
    for params in [json!({"ids": "1"}), json!({"ids": [1,999]}), json!({"ids": [1]})] {
        assert!(s.execute(APPLY, &params).is_err());
        assert_eq!(s.catalog.photo(PhotoId(1)).unwrap().develop, before);
        assert!(s.catalog.photo(PhotoId(1)).unwrap().versions.is_empty());
    }
    assert_eq!(s.execute("bedov.detail.preview", &json!({"ids": [1]})).unwrap()["photos"][0]["denoise"], 100.0);
}

#[test]
fn old_preferences_default_off_and_enabling_requires_a_ready_model() {
    let defaults: crate::import::ImportDefaults = serde_json::from_value(json!({"rawPreset": null})).unwrap();
    assert!(!defaults.bedov_auto_detail);
    let mut s = session();
    assert!(s.execute("library.preferences", &json!({"import": {"bedovAutoDetail": true}})).is_err());
    assert!(!s.import_defaults.bedov_auto_detail);
    assert!(s.execute("library.preferences", &json!({"import": {"bedovAutoDetail": "yes"}})).is_err());
    let p = s.catalog.photo(PhotoId(1)).unwrap();
    assert_eq!(import_start(&s, p, (*p.develop).clone()), *p.develop);
}
