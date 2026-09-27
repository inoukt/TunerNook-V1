# Per-workspace canvas background image

## Goal and boundaries

Let each workspace number/ID display its own still image behind floating editor
windows, independent of which BIN project is open. The image belongs only to the
central editor canvas; it never covers the browser, toolbar, diagnostics, dock,
or floating tables. Background settings and image files are presentation-only
and do not modify BIN or XDF data.

Keep the existing right-click menus on tables and cells unchanged. A secondary
click on otherwise-empty canvas opens workspace background options. The first
version supports selecting/replacing an image, Fit or Fill placement, opacity,
and removal. It does not support panning, tiling, animation, or per-table
images.

## Persisted state and asset ownership

Add a serde-defaulted map to global `AppPreferences`, keyed only by numeric
workspace ID. Each entry contains an optional relative image reference,
placement (`Fit` or `Fill`), and opacity percentage. Do not put this setting in
BIN-scoped `ProjectPreferences`, project manifests, or `WorkspaceViewState`.
Therefore, workspace ID 2 intentionally uses the same background across BIN
projects, while ID 3 can use a different one. Missing fields in older main
settings default to no image, Fit, and 100% opacity. Sanitize opacity and
reject absolute or parent-traversing asset references.

Workspace IDs are currently allocated within each BIN project, so equal IDs in
different projects intentionally share one global background entry. Deleting a
workspace in one project must not clear that entry; it may still be in use by
another project or by a later workspace with the same number.

Use the directory containing the main settings file to derive one app-managed
asset directory, `workspace-backgrounds/`. Store a relative reference within
that folder in the global entry for the workspace ID. When selected, validate
the supported still-image format and copy its original bytes (without resizing
or re-encoding) to a content-addressed filename. PNG, JPEG, and BMP are
supported; animated formats are not. Content addressing avoids clobbering
existing assets and deduplicates identical files. Replacing/removing a
reference does not delete an asset because another workspace ID may use it. No
asset write touches the BIN or XDF path.

If the expected content-addressed name is occupied by unreadable or different
bytes, keep that file and choose a collision-safe suffix for the newly copied
original. This lets the user repair a corrupt managed asset through Replace
without overwriting the damaged file or another workspace's reference.

If the asset root cannot be created or written, retain the current setting and
show a clear error. A missing or unreadable saved asset must not prevent loading
or switching workspaces; the canvas indicates that the background is
unavailable, and the context menu continues to offer replacement and removal.

## Canvas rendering and cache lifecycle

Paint one textured image rectangle clipped to the central editor canvas before
floating table and surface windows are drawn. Fit keeps the full image visible
and centers it within the canvas; Fill covers the canvas and center-crops the
texture coordinates. Apply opacity through the image tint. The normal no-image
path performs no file access, decoding, or texture upload.

Hold at most the active workspace's decoded GPU texture. Its cache key includes
workspace ID and relative image reference, not BIN identity. Decode/upload only
when that key changes; changing opacity or placement reuses the texture.
Workspace switches and image removal discard the previous active texture.
Cache a failed load for the same key so a bad asset is not decoded on every
frame. Selection and table movement do not invalidate the image cache.

Respect the renderer's reported maximum texture side. If the original exceeds
that limit, downscale only the decoded, temporary GPU texture with aspect ratio
preserved; never resize or replace the managed source image. A 4K source can
therefore display on a renderer limited to 2048 pixels per side as a
2048×1152 texture, while a higher-capability renderer can use the full source.

The canvas context-menu hit region stays beneath floating-window layers. It
must not capture table/cell input or change the existing table/cell context
menus. Because settings and assets are global, background images can be assigned
or removed even when no BIN is loaded.

## Verification

- Round-trip global background settings keyed by workspace ID; switching
  between IDs preserves distinct backgrounds, and reopening the app restores
  them.
- Verify the same workspace ID uses the same background across BIN projects,
  while another ID remains independent.
- Deserialize legacy main settings with no background map as no-image defaults;
  project preferences/manifests remain independent of background settings.
- Verify placement and opacity sanitization and reject unsafe relative paths.
- Verify a selected original is copied unchanged to a relative managed asset
  reference, and the BIN/XDF files and data revision remain unchanged.
- Verify right-clicking empty canvas opens the options while table/cell input
  and their existing context menus remain unaffected.
- Verify missing, unreadable, and unsupported images do not prevent workspace
  use and can be replaced or removed.
- With test instrumentation, compare no image, 1080p, and 4K texture cases;
  assert that the no-image path does no image work, a changed image/workspace
  loads once, and unchanged frames, selection, and table movement do not repeat
  decoding or upload.
- Attempt a release-renderer timing check on the available GPU; report clearly
  if a modest/integrated GPU is not available in the verification environment.
