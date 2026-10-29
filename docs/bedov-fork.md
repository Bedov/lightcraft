# Bedov LightCraft fork

This fork keeps a small set of local changes on a tested upstream release. The initial base is
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

1. `git fetch upstream --tags` and inspect the official release notes.
2. Start an update branch from `bedov/stable`, then merge the chosen release tag.
3. Resolve conflicts while preserving upstream decoder fixes and the HE extension.
4. Run `cargo xtask ci`, then the private reference tests described in `nikon-he.md`.
   Normal CI reports the private tests as skipped when those files are unavailable.
5. Compare Canon and Nikon renders and verify saved edits on a copy of a current library.
6. Build a separate app, check opening and export, then advance `bedov/stable` and push it.

Keep the last working app and a stable catalog backup. Upstream updates are reviewed merges,
not a reset of this fork or an automatic replacement of its binary.
