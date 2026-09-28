# README Storage and Recent Files Note

## Intent

Explain where TunerNook stores automatic preferences and how those differ from
user-owned BIN/XDF files and optional `.tnproj` manifests.

## Requirements

- Document the Windows app settings path and the XDG/fallback paths.
- Explain Recent BINs/XDFs: path-only history, up to 16, newest-first, successful
  loads only, and clearing the list does not delete files.
- Explain that per-BIN automatic workspace JSON files are keyed by a hash of
  the normalized BIN path and do not contain BIN/XDF bytes.
- Explain `.tnproj` manifests are saved at a chosen location and reference
  existing source file paths rather than bundling data.
- Mention the separate app-managed workspace-background asset folder.
- Warn that settings, per-BIN JSON, and manifests may reveal local paths and
  should be reviewed before sharing.
- Keep all descriptions aligned with implementation and existing safety rules.
