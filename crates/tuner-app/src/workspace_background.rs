use std::fs::{self, OpenOptions};
use std::io::{Cursor, Write};
use std::path::{Component, Path, PathBuf};

use eframe::egui;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum WorkspaceBackgroundPlacement {
    #[default]
    Fit,
    Fill,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct WorkspaceBackgroundSettings {
    pub image: Option<String>,
    pub placement: WorkspaceBackgroundPlacement,
    pub opacity_percent: u8,
}

impl Default for WorkspaceBackgroundSettings {
    fn default() -> Self {
        Self {
            image: None,
            placement: WorkspaceBackgroundPlacement::Fit,
            opacity_percent: 100,
        }
    }
}

impl WorkspaceBackgroundSettings {
    pub(crate) fn sanitize(&mut self) {
        self.opacity_percent = self.opacity_percent.min(100);
        if self
            .image
            .as_deref()
            .is_some_and(|reference| validate_asset_reference(reference).is_err())
        {
            self.image = None;
        }
    }
}

pub(crate) fn asset_directory(settings_path: &Path) -> PathBuf {
    settings_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join("workspace-backgrounds")
}

pub(crate) fn validate_asset_reference(reference: &str) -> Result<(), String> {
    let path = Path::new(reference);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) if !reference.is_empty() => Ok(()),
        _ => Err("Background image reference must be a relative file name.".into()),
    }
}

pub(crate) fn resolve_asset_path(settings_path: &Path, reference: &str) -> Result<PathBuf, String> {
    validate_asset_reference(reference)?;
    Ok(asset_directory(settings_path).join(reference))
}

pub(crate) fn copy_background_asset(source: &Path, settings_path: &Path) -> Result<String, String> {
    let bytes = fs::read(source).map_err(|error| {
        format!(
            "Could not read background image '{}': {error}",
            source.display()
        )
    })?;
    let reader = image::ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|error| format!("Could not identify background image: {error}"))?;
    let extension = match reader.format() {
        Some(image::ImageFormat::Png) => "png",
        Some(image::ImageFormat::Jpeg) => "jpg",
        Some(image::ImageFormat::Bmp) => "bmp",
        _ => return Err("Choose a PNG, JPEG, or BMP image.".into()),
    };
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| format!("Could not read background image dimensions: {error}"))?;
    if width == 0 || height == 0 {
        return Err("Background image has zero width or height.".into());
    }

    let hash = tuner_core::sha256_hex(&bytes);
    let directory = asset_directory(settings_path);
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "Could not create managed background image folder '{}': {error}",
            directory.display()
        )
    })?;
    let mut suffix = 0_u32;
    loop {
        let reference = if suffix == 0 {
            format!("{hash}.{extension}")
        } else {
            format!("{hash}-{suffix}.{extension}")
        };
        validate_asset_reference(&reference)?;
        let destination = directory.join(&reference);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
        {
            Ok(mut file) => {
                if let Err(error) = file.write_all(&bytes) {
                    drop(file);
                    let _ = fs::remove_file(&destination);
                    return Err(format!(
                        "Could not save managed background image '{}': {error}",
                        destination.display()
                    ));
                }
                return Ok(reference);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if fs::read(&destination).is_ok_and(|existing| existing == bytes) {
                    return Ok(reference);
                }
                suffix = suffix.checked_add(1).ok_or_else(|| {
                    "No unique managed background image name is available.".to_string()
                })?;
            }
            Err(error) => {
                return Err(format!(
                    "Could not save managed background image '{}': {error}",
                    destination.display()
                ));
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct WorkspaceBackgroundTextureCache {
    key: Option<(u64, String)>,
    texture: Option<egui::TextureHandle>,
    error: Option<String>,
    #[cfg(test)]
    decode_count: usize,
    #[cfg(test)]
    upload_count: usize,
}

impl WorkspaceBackgroundTextureCache {
    pub(crate) fn paint(
        &mut self,
        ui: &egui::Ui,
        settings_path: &Path,
        workspace_id: u64,
        settings: Option<&WorkspaceBackgroundSettings>,
        canvas: egui::Rect,
    ) -> Option<String> {
        let Some(settings) = settings.filter(|settings| settings.image.is_some()) else {
            self.key = None;
            self.texture = None;
            self.error = None;
            return None;
        };
        let reference = settings.image.as_ref().expect("filtered above");
        let key = (workspace_id, reference.clone());
        if self.key.as_ref() != Some(&key) {
            self.key = Some(key.clone());
            self.texture = None;
            self.error = None;
            self.load(ui.ctx(), settings_path, &key);
        }
        if let Some(texture) = &self.texture {
            let (image_rect, uv) =
                background_image_geometry(texture.size_vec2(), canvas, settings.placement);
            let alpha = ((settings.opacity_percent.min(100) as f32 / 100.0) * 255.0).round() as u8;
            ui.painter().image(
                texture.id(),
                image_rect,
                uv,
                egui::Color32::from_white_alpha(alpha),
            );
        }
        self.error.clone()
    }

    fn load(&mut self, context: &egui::Context, settings_path: &Path, key: &(u64, String)) {
        #[cfg(test)]
        {
            self.decode_count += 1;
        }
        let result = (|| {
            let path = resolve_asset_path(settings_path, &key.1)?;
            let bytes = fs::read(&path).map_err(|error| {
                format!(
                    "Could not read background image '{}': {error}",
                    path.display()
                )
            })?;
            let reader = image::ImageReader::new(Cursor::new(&bytes))
                .with_guessed_format()
                .map_err(|error| format!("Could not identify background image: {error}"))?;
            if !matches!(
                reader.format(),
                Some(image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::Bmp)
            ) {
                return Err("Background image is not a supported PNG, JPEG, or BMP.".into());
            }
            let image = reader
                .decode()
                .map_err(|error| format!("Could not decode background image: {error}"))?;
            let maximum = context
                .input(|input| input.max_texture_side)
                .min(u32::MAX as usize) as u32;
            if maximum == 0 {
                return Err("The renderer reports no supported texture size.".into());
            }
            let texture_size = texture_size_within_limit(image.width(), image.height(), maximum);
            let image = if texture_size != (image.width(), image.height()) {
                image.resize_exact(
                    texture_size.0,
                    texture_size.1,
                    image::imageops::FilterType::Triangle,
                )
            } else {
                image
            }
            .to_rgba8();
            let size = [image.width() as usize, image.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
            Ok::<_, String>(context.load_texture(
                format!("workspace-background-{}", key.0),
                color_image,
                egui::TextureOptions::LINEAR,
            ))
        })();
        match result {
            Ok(texture) => {
                self.texture = Some(texture);
                #[cfg(test)]
                {
                    self.upload_count += 1;
                }
            }
            Err(error) => self.error = Some(error),
        }
    }
}

#[cfg(test)]
pub(crate) fn cache_work_counts(cache: &WorkspaceBackgroundTextureCache) -> (usize, usize) {
    (cache.decode_count, cache.upload_count)
}

fn background_image_geometry(
    image_size: egui::Vec2,
    canvas: egui::Rect,
    placement: WorkspaceBackgroundPlacement,
) -> (egui::Rect, egui::Rect) {
    let full_uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
    if image_size.x <= 0.0 || image_size.y <= 0.0 || canvas.width() <= 0.0 || canvas.height() <= 0.0
    {
        return (canvas, full_uv);
    }
    let image_aspect = image_size.x / image_size.y;
    let canvas_aspect = canvas.width() / canvas.height();
    match placement {
        WorkspaceBackgroundPlacement::Fit => {
            let scale = (canvas.width() / image_size.x).min(canvas.height() / image_size.y);
            let size = image_size * scale;
            (egui::Rect::from_center_size(canvas.center(), size), full_uv)
        }
        WorkspaceBackgroundPlacement::Fill => {
            let uv = if image_aspect > canvas_aspect {
                let visible_width = canvas_aspect / image_aspect;
                let margin = (1.0 - visible_width) / 2.0;
                egui::Rect::from_min_max(egui::pos2(margin, 0.0), egui::pos2(1.0 - margin, 1.0))
            } else {
                let visible_height = image_aspect / canvas_aspect;
                let margin = (1.0 - visible_height) / 2.0;
                egui::Rect::from_min_max(egui::pos2(0.0, margin), egui::pos2(1.0, 1.0 - margin))
            };
            (canvas, uv)
        }
    }
}

fn texture_size_within_limit(width: u32, height: u32, maximum: u32) -> (u32, u32) {
    if maximum == 0 || (width <= maximum && height <= maximum) {
        return (width, height);
    }
    let scale = (maximum as f64 / width as f64).min(maximum as f64 / height as f64);
    (
        ((width as f64 * scale).round() as u32).clamp(1, maximum),
        ((height as f64 * scale).round() as u32).clamp(1, maximum),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use tuner_core::sha256_hex;

    use super::*;

    static NEXT_TEMP_DIR: AtomicUsize = AtomicUsize::new(1);

    fn temporary_directory() -> PathBuf {
        let id = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tunernook-background-test-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn small_png() -> Vec<u8> {
        let image = RgbaImage::from_pixel(2, 2, Rgba([12, 34, 56, 255]));
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    #[test]
    fn background_asset_copy_preserves_original_bytes_and_deduplicates() {
        let directory = temporary_directory();
        let settings_path = directory.join("settings.json");
        let source = directory.join("source.png");
        let bytes = small_png();
        fs::write(&source, &bytes).unwrap();

        let first = copy_background_asset(&source, &settings_path).unwrap();
        let second = copy_background_asset(&source, &settings_path).unwrap();

        assert_eq!(first, format!("{}.png", sha256_hex(&bytes)));
        assert_eq!(first, second);
        assert_eq!(
            fs::read(resolve_asset_path(&settings_path, &first).unwrap()).unwrap(),
            bytes
        );
        assert_eq!(
            asset_directory(&settings_path),
            directory.join("workspace-backgrounds")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reselecting_original_repairs_a_corrupted_managed_asset_without_overwriting_it() {
        let directory = temporary_directory();
        let settings_path = directory.join("settings.json");
        let source = directory.join("source.png");
        let bytes = small_png();
        fs::write(&source, &bytes).unwrap();
        let original_reference = copy_background_asset(&source, &settings_path).unwrap();
        let original_path = resolve_asset_path(&settings_path, &original_reference).unwrap();
        fs::write(&original_path, b"corrupt managed image").unwrap();

        let repair_result = copy_background_asset(&source, &settings_path);
        let repair_reference = repair_result.as_ref().ok().cloned();
        let repaired_bytes = repair_reference
            .as_deref()
            .and_then(|reference| resolve_asset_path(&settings_path, reference).ok())
            .and_then(|path| fs::read(path).ok());
        let damaged_bytes = fs::read(&original_path).ok();
        fs::remove_dir_all(directory).unwrap();

        assert!(
            repair_result.is_ok(),
            "replacing a corrupt managed image should succeed: {repair_result:?}"
        );
        assert_ne!(
            repair_reference.as_deref(),
            Some(original_reference.as_str())
        );
        assert_eq!(repaired_bytes.as_deref(), Some(bytes.as_slice()));
        assert_eq!(
            damaged_bytes.as_deref(),
            Some(b"corrupt managed image".as_slice())
        );
    }

    #[test]
    fn invalid_or_unsupported_background_images_are_not_copied() {
        let directory = temporary_directory();
        let settings_path = directory.join("settings.json");
        let corrupt = directory.join("corrupt.png");
        let unsupported = directory.join("animated.gif");
        fs::write(&corrupt, b"not a PNG").unwrap();
        fs::write(&unsupported, b"GIF89a").unwrap();

        assert!(copy_background_asset(&corrupt, &settings_path).is_err());
        assert!(copy_background_asset(&unsupported, &settings_path).is_err());
        assert!(!asset_directory(&settings_path).exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn background_asset_resolution_rejects_path_escape() {
        let settings_path = Path::new("C:/Users/test/settings.json");

        assert!(resolve_asset_path(settings_path, "../outside.png").is_err());
        assert!(resolve_asset_path(settings_path, "/outside.png").is_err());
        assert_eq!(
            resolve_asset_path(settings_path, "inside.png").unwrap(),
            Path::new("C:/Users/test/workspace-backgrounds/inside.png")
        );
    }

    #[test]
    fn fit_contains_the_image_and_fill_center_crops_it() {
        let canvas = egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(110.0, 120.0));
        let (fit_rect, fit_uv) = background_image_geometry(
            egui::vec2(200.0, 100.0),
            canvas,
            WorkspaceBackgroundPlacement::Fit,
        );
        let (fill_rect, fill_uv) = background_image_geometry(
            egui::vec2(200.0, 100.0),
            canvas,
            WorkspaceBackgroundPlacement::Fill,
        );

        assert_eq!(
            fit_rect,
            egui::Rect::from_min_max(egui::pos2(10.0, 45.0), egui::pos2(110.0, 95.0))
        );
        assert_eq!(
            fit_uv,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0))
        );
        assert_eq!(fill_rect, canvas);
        assert!((fill_uv.left() - 0.25).abs() < f32::EPSILON);
        assert!((fill_uv.right() - 0.75).abs() < f32::EPSILON);
        assert_eq!(fill_uv.top(), 0.0);
        assert_eq!(fill_uv.bottom(), 1.0);
    }

    #[test]
    fn oversized_background_texture_respects_renderer_limit_without_distorting_aspect() {
        assert_eq!(texture_size_within_limit(3840, 2160, 2048), (2048, 1152));
        assert_eq!(texture_size_within_limit(1920, 1080, 2048), (1920, 1080));
        assert_eq!(texture_size_within_limit(3840, 2160, 4096), (3840, 2160));
    }

    fn paint_frame(
        context: &egui::Context,
        cache: &mut WorkspaceBackgroundTextureCache,
        settings_path: &Path,
        workspace_id: u64,
        settings: Option<&WorkspaceBackgroundSettings>,
    ) -> (egui::FullOutput, Option<String>) {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        let mut error = None;
        let output = context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let canvas = ui.available_rect_before_wrap();
                error = cache.paint(ui, settings_path, workspace_id, settings, canvas);
            });
        });
        (output, error)
    }

    fn image_mesh_count(output: &egui::FullOutput, texture_id: egui::TextureId) -> usize {
        fn count(shape: &egui::Shape, texture_id: egui::TextureId) -> usize {
            match shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == texture_id => 1,
                egui::Shape::Vec(shapes) => {
                    shapes.iter().map(|shape| count(shape, texture_id)).sum()
                }
                _ => 0,
            }
        }
        output
            .shapes
            .iter()
            .map(|shape| count(&shape.shape, texture_id))
            .sum()
    }

    #[test]
    fn no_background_does_no_image_work() {
        let context = egui::Context::default();
        let mut cache = WorkspaceBackgroundTextureCache::default();
        let settings_path = Path::new("missing-settings/settings.json");

        let (output, error) = paint_frame(&context, &mut cache, settings_path, 0, None);
        let texture = cache.texture.is_some();
        let decode_count = cache.decode_count;
        let upload_count = cache.upload_count;
        output.drop_without_applying_deltas();
        assert!(error.is_none());
        assert_eq!(decode_count, 0);
        assert_eq!(upload_count, 0);
        assert!(!texture);
    }

    #[test]
    fn background_load_failure_is_cached_until_workspace_changes() {
        let directory = temporary_directory();
        let settings_path = directory.join("settings.json");
        let reference = "repaired.png";
        let mut settings = WorkspaceBackgroundSettings::default();
        settings.image = Some(reference.into());
        let context = egui::Context::default();
        let mut cache = WorkspaceBackgroundTextureCache::default();

        let (first, first_error) =
            paint_frame(&context, &mut cache, &settings_path, 1, Some(&settings));
        let first_failed = first_error.is_some();
        let first_decode_count = cache.decode_count;
        first.drop_without_applying_deltas();
        assert!(first_failed);
        assert_eq!(first_decode_count, 1);

        fs::create_dir_all(asset_directory(&settings_path)).unwrap();
        fs::write(
            resolve_asset_path(&settings_path, reference).unwrap(),
            small_png(),
        )
        .unwrap();
        let (same_key, same_key_error) =
            paint_frame(&context, &mut cache, &settings_path, 1, Some(&settings));
        let same_key_failed = same_key_error.is_some();
        let same_key_decode_count = cache.decode_count;
        same_key.drop_without_applying_deltas();
        assert!(same_key_failed);
        assert_eq!(same_key_decode_count, 1);

        let (new_workspace, new_workspace_error) =
            paint_frame(&context, &mut cache, &settings_path, 2, Some(&settings));
        let new_workspace_loaded = new_workspace_error.is_none();
        let new_workspace_decode_count = cache.decode_count;
        let new_workspace_upload_count = cache.upload_count;
        let texture_id = cache.texture.as_ref().map(egui::TextureHandle::id);
        let image_count = texture_id
            .map(|texture_id| image_mesh_count(&new_workspace, texture_id))
            .unwrap_or_default();
        new_workspace.drop_without_applying_deltas();
        assert!(new_workspace_loaded);
        assert_eq!(new_workspace_decode_count, 2);
        assert_eq!(new_workspace_upload_count, 1);
        assert_eq!(image_count, 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn unchanged_background_frames_upload_once_and_draw_one_image_shape() {
        let directory = temporary_directory();
        let settings_path = directory.join("settings.json");
        let source = directory.join("source.png");
        fs::write(&source, small_png()).unwrap();
        let reference = copy_background_asset(&source, &settings_path).unwrap();
        let settings = WorkspaceBackgroundSettings {
            image: Some(reference),
            ..WorkspaceBackgroundSettings::default()
        };
        let context = egui::Context::default();
        let mut cache = WorkspaceBackgroundTextureCache::default();

        let (first, first_error) =
            paint_frame(&context, &mut cache, &settings_path, 1, Some(&settings));
        let first_loaded = first_error.is_none();
        let first_decode_count = cache.decode_count;
        let first_upload_count = cache.upload_count;
        let texture_id = cache.texture.as_ref().map(egui::TextureHandle::id);
        let first_image_count = texture_id
            .map(|texture_id| image_mesh_count(&first, texture_id))
            .unwrap_or_default();
        first.drop_without_applying_deltas();
        assert!(first_loaded);
        assert_eq!(first_decode_count, 1);
        assert_eq!(first_upload_count, 1);
        assert_eq!(first_image_count, 1);

        let (second, second_error) =
            paint_frame(&context, &mut cache, &settings_path, 1, Some(&settings));
        let second_loaded = second_error.is_none();
        let second_decode_count = cache.decode_count;
        let second_upload_count = cache.upload_count;
        let second_image_count = texture_id
            .map(|texture_id| image_mesh_count(&second, texture_id))
            .unwrap_or_default();
        second.drop_without_applying_deltas();
        assert!(second_loaded);
        assert_eq!(second_decode_count, 1);
        assert_eq!(second_upload_count, 1);
        assert_eq!(second_image_count, 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn cache_preparation_scales_from_no_image_to_1080p_and_4k_without_repeat_uploads() {
        let directory = temporary_directory();
        let settings_path = directory.join("settings.json");
        let context = egui::Context::default();
        let mut cache = WorkspaceBackgroundTextureCache::default();
        let no_image_started = std::time::Instant::now();
        let (no_image_output, no_image_error) =
            paint_frame(&context, &mut cache, &settings_path, 0, None);
        let no_image_elapsed = no_image_started.elapsed();
        no_image_output.drop_without_applying_deltas();
        assert!(no_image_error.is_none());
        assert_eq!(cache.decode_count, 0);
        assert_eq!(cache.upload_count, 0);
        eprintln!(
            "workspace background no-image frame: {no_image_elapsed:?}, 0 bytes decoded/uploaded"
        );

        for (index, (width, height)) in [(1920u32, 1080u32), (3840, 2160)].into_iter().enumerate() {
            let source = directory.join(format!("background-{width}x{height}.png"));
            let pixels = RgbaImage::from_pixel(width, height, Rgba([40, 80, 120, 255]));
            pixels.save(&source).unwrap();
            drop(pixels);
            let original_bytes = fs::read(&source).unwrap();
            let reference = copy_background_asset(&source, &settings_path).unwrap();
            let managed_path = resolve_asset_path(&settings_path, &reference).unwrap();
            assert_eq!(fs::read(managed_path).unwrap(), original_bytes);
            let settings = WorkspaceBackgroundSettings {
                image: Some(reference),
                ..WorkspaceBackgroundSettings::default()
            };
            let first_started = std::time::Instant::now();
            let (first, first_error) = paint_frame(
                &context,
                &mut cache,
                &settings_path,
                index as u64 + 1,
                Some(&settings),
            );
            let first_elapsed = first_started.elapsed();
            let first_loaded = first_error.is_none();
            let expected_count = index + 1;
            let first_decode_count = cache.decode_count;
            let first_upload_count = cache.upload_count;
            let image_size = cache.texture.as_ref().map(egui::TextureHandle::size_vec2);
            let expected_texture_size = texture_size_within_limit(width, height, 2048);
            first.drop_without_applying_deltas();
            assert!(first_loaded);
            assert_eq!(first_decode_count, expected_count);
            assert_eq!(first_upload_count, expected_count);
            assert_eq!(
                image_size,
                Some(egui::vec2(
                    expected_texture_size.0 as f32,
                    expected_texture_size.1 as f32
                ))
            );

            let repeated_started = std::time::Instant::now();
            let (repeated, repeated_error) = paint_frame(
                &context,
                &mut cache,
                &settings_path,
                index as u64 + 1,
                Some(&settings),
            );
            let repeated_elapsed = repeated_started.elapsed();
            let repeated_loaded = repeated_error.is_none();
            let repeated_decode_count = cache.decode_count;
            let repeated_upload_count = cache.upload_count;
            repeated.drop_without_applying_deltas();
            assert!(repeated_loaded);
            assert_eq!(repeated_decode_count, expected_count);
            assert_eq!(repeated_upload_count, expected_count);
            eprintln!(
                "workspace background original {width}x{height}, texture {}x{}: first decode/resize/upload preparation {first_elapsed:?}; unchanged frame {repeated_elapsed:?}",
                expected_texture_size.0,
                expected_texture_size.1
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
