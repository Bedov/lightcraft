# Bedov Auto Detail

An opt-in starting treatment for photographic noise and capture sharpening. This uses the existing
AI Denoise engine; it adds no renderer, decoder, dependency, or model weights to the application.
The RawNIND Bayer model must be installed separately after accepting its terms.

## Use

- Settings > Import > **Bedov Auto Detail**: enable automatic detail for newly imported photos
  in this library. It does not change any existing photograph.
- Detail > **Bedov Auto Detail**: apply to the active photo only, independently of Auto Sync.
- Photo > **Apply Bedov Auto Detail to Selected**, or the button in Settings > Import: apply
  to selected photos. Every changed photo receives a **Before Bedov Auto Detail** version.
  One Undo restores the entire batch, including versions/history. Reapplying unchanged values is a no-op.
- Settings remain ordinary, editable photo settings. Moving exposure does not silently recalculate them.

Only original, fully decoded CR3/NEF files from the listed cameras are eligible. Missing ISO,
unknown cameras, disabled Detail, JPEGs, preview-only files and DNGs are skipped. DNGs can already
be demosaiced, so they must not be assumed compatible with the Bayer model.

## Policy v1

| Camera class | Models | Radius | Sharpen, AI off / on |
|---|---|---|---|
| Full-frame 24 MP | Canon R6 II, Nikon Z6 III, Nikon D750 | 0.8 | 25 / 20 |
| Full-frame high resolution | Nikon Z7 II | 0.7 | 30 / 22 |

| Recorded ISO | RawNIND Amount | Manual luminance / colour NR | Sharpen masking |
|---|---|---|---|
| Below 1600 | 0 | 0 / 25 | 70 |
| 1600–3199 | 80 | 0 / 0 | 90 |
| 3200 and above | 100 | 0 / 0 | 90 |

Sharpen Detail is 15. These are conservative starting values, not measured equivalence between
cameras or a promise that every frame needs the same processing. Only the specified detail controls
and AI Denoise amount/on switch change. Light, colour, clarity, texture, crop and local masks stay intact.
Raw Details and Super Resolution are preserved. Camera presets, explicit import presets and XMP
take precedence over these defaults. Reset returns to the recorded import look.

Since the 0.19.0 update, new photos use upstream process V6 with working pixel-scale Radius
and Detail. Existing V1/V2 edits retain the older sharpening implementation until explicitly
updated; applying Auto Detail does not change their process. The policy still targets the
verified RawNIND model, not the new built-in BM3D denoiser. Compare that alternative separately
before choosing its defaults.

The preview and export use the same saved settings and denoised cache. Existing export output
sharpening remains a separate choice for final size and paper. No extra denoising is silently added
at export. The existing engine computes a missing denoised result when exporting.

## Small integration surface for upstream merges

- `crates/engine/src/cmd/bedov_detail.rs`: isolated camera/ISO policy, availability, preview and
  selected-photo command. `bedov_detail_tests.rs` covers preservation and reversible batch behavior.
- `cmd.rs`: module and command registration.
- `import/defaults.rs`: one opt-in preference and the starting-settings hook; the existing
  `import_look` records these defaults so Reset works without changing catalog schemas.
- `import/commit.rs`: refresh the selected model once per import batch.
- `cmd/prefs.rs`: validate/save the opt-in preference (`import.bedovAutoDetail`). Old preferences
  default to false. Without the matching model/runtime, import keeps the ordinary defaults.
- `ui-egui/src/panels/bedov_detail.rs`: isolated UI, with short hooks in Settings > Import and Detail.

Policy changes affect future imports and explicit applications only; they do not rerender saved
settings differently. Keep the `bedov.*` command namespace and separate fork commits when merging
an upstream release. Never replace upstream render files with old fork copies to resolve conflicts.

Validation includes targeted engine tests, full `cargo xtask ci`, a separate native UI/library check,
and private real-RAW checks. Private photographs, models and output files remain outside Git.
