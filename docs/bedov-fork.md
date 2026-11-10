# Bedov LightCraft fork

This fork keeps a small set of local changes on an upstream release. The current base is
`v0.19.0` (`16636c4`), integrated on 2026-11-09. The initial base was
`v0.15.0` (`e381c1a`), migrated from the local `0.9.1` snapshot
`d999e382bafbb5d5c54482fdb1d11f6a63bfdf69` on 2026-10-29.

- Repository: https://github.com/Bedov/lightcraft
- Upstream: https://github.com/storytold/lightcraft
- Working branch: `bedov/stable`
- Nikon Z6 III full-frame HE support: [scope and evidence](nikon-he.md).
- The MIT decoder dependency is pinned by commit, without machine-specific paths.
- **LightCraft Bedov** has its own bundle/update identity. Its updater only uses
  `Bedov/lightcraft`; automatic startup checks are disabled. Official binaries omit our HE extension.
- Left/Right change photos even after clicking or dragging a develop slider. Up/Down still
  adjust the slider; numeric/text entry, dialogs and photo-tool handles retain their keys.
- Migration checks also caught an unsupported glyph in the empty Reference view's English
  hint. The hint now uses an available arrow, retaining its translated messages, with a regression test.

- Bedov Auto Detail: opt-in import defaults by camera/ISO plus an explicit, undoable selected-photo command.
  [Policy, UI and update hooks](bedov-auto-detail.md) are isolated from upstream raw decoding and rendering.
- Sony full-frame ARWs whose crop tags (`0x74c7/0x74c8`) still hold the centred APS-C window open at full size.
  Some ILCE-7M4 files (firmware 6.01) store the whole sensor and a full-frame default crop, and report a full-frame
  35 mm focal length, yet Sony's tags give 4608 × 3072. Only that combination uses the default crop: genuine APS-C files,
  which store just the window, and in-camera aspect ratios are unchanged. The rule and its tests live in
  `crates/raw/src/vendor/arw_stale_crop.rs`, hooked from one line of `arw.rs`. After an upstream merge, check that hook.
- Lens-database (lensfun) vignetting no longer reaches the camera-look fit, only a file's own correction does
  (`correct_vignetting` in `crates/engine/src/camera_preview/look_fit.rs`, `LOOK_VERSION` 14). Upstream 0.19 fitted
  the look on the database-corrected sensor, although the camera never applied that profile to its JPEG: once the app
  had downloaded the database, photos whose lens has lensfun vignetting data (ILCE-7M4 with Tamron 35-150 mm A058,
  FE 85 mm F1.8 or Sigma 20 mm Art; Canon EF 24-70/70-200 and Nikon Z 24-70/70-200/35/50 mm on their bodies)
  rendered up to 12 L\* darker with Enable Profile Corrections off. After an upstream merge, check that filter and keep the fork's
  `LOOK_VERSION` / `RENDER_CACHE_VERSION` above upstream's.

## Build on macOS

Use Rust at least as new as `workspace.package.rust-version` in `Cargo.toml`. Run `cargo xtask ci`
before committing. Optional `CRAFT_FONTS_DIR` is described in the main README.

Run `python3 scripts/bedov-build.py --library /absolute/path/to/library --config /absolute/path/to/settings`.
The output defaults to the sibling `LightCraft-Fork-Local` folder, outside the source repository.
Pass `--output` for a different destination. The script never overwrites an existing app.
It builds desktop and CLI, includes licences, signs the local bundle ad hoc, and embeds the selected
library/config paths so the app opens normally from Finder.

Library paths, photographs, user settings, calibration samples, binaries and build outputs stay
outside Git. An ad-hoc signed local build is not a notarized distribution release.

## Adopt an upstream release

The 0.19.0 integration retains all four Bedov extensions and uses upstream's pixel-scale
sharpening, BM3D denoiser, colour NR and user-supplied DCP import. Existing photos retain
their saved rendering process. Test **Update to Current Process** on copies before applying
it to an edited series: V6 includes tone changes as well as detail improvements.
The selected RawNIND model remains selected; the built-in BM3D is an alternative, not an
automatic replacement. Bedov Auto Detail still requires the verified RawNIND model.
The updater identity is centralized in `ui-egui::UPDATE_APP`; both the dialog and restart
installer must continue to use the Bedov repository and separate bundle identity.
The macOS V1/V2 golden-render fixture was refreshed for upstream's intentional 8-bit dithering
(#3509), verified against the pre-V3 renderer with only that quantization change applied.
This corrects the test reference, without changing the renderer. Saved process versions are
preserved, but an update is not a promise of bit-identical JPEGs across all decoder/GPU changes.

1. `git fetch upstream --tags` and inspect the official release notes.
2. Start an update branch from `bedov/stable`, then merge the chosen release tag.
3. Resolve conflicts while preserving upstream decoder fixes and the HE extension.
4. Run `cargo xtask ci`, then the private reference tests described in `nikon-he.md`.
   Normal CI reports the private tests as skipped when those files are unavailable.
5. Run the `bedov_detail` tests; recheck the small integration hooks listed in `bedov-auto-detail.md`.
   Verify import defaults, XMP precedence, selected targets, saved versions/undo, and a real RawNIND export.
   Compare Canon and Nikon renders and verify saved edits on a copy of a current library.
6. Build a separate app, check opening and export, then advance `bedov/stable` and push it.

Keep the last working app and a stable catalog backup. Upstream updates are reviewed merges,
not a reset of this fork or an automatic replacement of its binary.
