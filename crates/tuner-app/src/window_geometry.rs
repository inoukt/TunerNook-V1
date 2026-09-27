use eframe::egui;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct WindowGeometryMemory {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowGeometryMemory {
    fn default() -> Self {
        Self {
            x: 80,
            y: 60,
            width: 720,
            height: 480,
        }
    }
}

impl WindowGeometryMemory {
    pub fn from_rect(rect: egui::Rect) -> Self {
        let mut memory = Self {
            x: rect.left().round() as i32,
            y: rect.top().round() as i32,
            width: rect.width().round().max(0.0) as u32,
            height: rect.height().round().max(0.0) as u32,
        };
        memory.sanitize();
        memory
    }

    pub fn rect(&self) -> egui::Rect {
        egui::Rect::from_min_size(
            egui::pos2(self.x as f32, self.y as f32),
            egui::vec2(self.width as f32, self.height as f32),
        )
    }

    pub fn sanitize(&mut self) {
        self.width = self.width.clamp(320, 10_000);
        self.height = self.height.clamp(180, 10_000);
    }
}

pub fn legacy_position_saved() -> bool {
    true
}

pub fn cascade_rect(bounds: egui::Rect, desired_size: egui::Vec2, index: usize) -> egui::Rect {
    let size = desired_size
        .max(egui::Vec2::ZERO)
        .min(bounds.size().max(egui::Vec2::ZERO));
    let margin = 8.0;
    let step = 32.0;
    let columns = (((bounds.width() - size.x - margin * 2.0).max(0.0) / step).floor() as usize)
        .saturating_add(1);
    let rows = (((bounds.height() - size.y - margin * 2.0).max(0.0) / step).floor() as usize)
        .saturating_add(1);
    let column = index % columns;
    let row = index % rows;
    let position =
        bounds.min + egui::vec2(margin + column as f32 * step, margin + row as f32 * step);
    clamp_rect(
        egui::Rect::from_min_size(position, size),
        bounds,
        egui::vec2(320.0, 180.0),
    )
}

pub fn is_utility_geometry_id(id: &str) -> bool {
    matches!(
        id,
        "xdf_editor" | "project_notepad" | "debug_report" | "settings" | "nooklink"
    ) || id
        .strip_prefix("history:")
        .is_some_and(|key| !key.is_empty())
}

pub fn clamp_rect(rect: egui::Rect, bounds: egui::Rect, minimum: egui::Vec2) -> egui::Rect {
    let max_size = bounds.size().max(egui::Vec2::ZERO);
    if max_size.x <= f32::EPSILON || max_size.y <= f32::EPSILON {
        return egui::Rect::from_min_size(bounds.min, egui::Vec2::ZERO);
    }

    let min_size = minimum
        .max(egui::Vec2::ZERO)
        .min(max_size)
        .min(max_size * 0.75);
    let size = egui::vec2(
        rect.width().max(min_size.x).min(max_size.x),
        rect.height().max(min_size.y).min(max_size.y),
    );
    let max_position = bounds.max - size;
    let min = egui::pos2(
        rect.left().clamp(bounds.left(), max_position.x),
        rect.top().clamp(bounds.top(), max_position.y),
    );
    egui::Rect::from_min_size(min, size)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RootWindowGeometry {
    pub outer_position: egui::Pos2,
    pub inner_size: egui::Vec2,
    pub minimum_inner_size: egui::Vec2,
}

pub fn fit_root_window(
    work_area: egui::Rect,
    desired: egui::Vec2,
    minimum: egui::Vec2,
) -> RootWindowGeometry {
    let chrome = egui::vec2(32.0, 56.0);
    let safe_inner = (work_area.size() - chrome).max(egui::Vec2::ZERO);
    let inner_size = desired.max(egui::Vec2::ZERO).min(safe_inner);
    let minimum_inner_size = minimum
        .max(egui::Vec2::ZERO)
        .min(inner_size)
        .min(inner_size * 0.75);
    let outer_offset = (work_area.size() - inner_size - chrome).max(egui::Vec2::ZERO) * 0.5;
    RootWindowGeometry {
        outer_position: work_area.min + outer_offset,
        inner_size,
        minimum_inner_size,
    }
}

pub fn shell_safe_rect(
    viewport: egui::Rect,
    toolbar: egui::Rect,
    dock: egui::Rect,
    diagnostics: Option<egui::Rect>,
) -> egui::Rect {
    let top = toolbar.bottom().clamp(viewport.top(), viewport.bottom());
    let bottom_panel_top = diagnostics.map_or(dock.top(), |rect| rect.top().min(dock.top()));
    let bottom = bottom_panel_top.clamp(top, viewport.bottom());
    egui::Rect::from_min_max(
        egui::pos2(viewport.left(), top),
        egui::pos2(viewport.right(), bottom),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::egui;

    #[test]
    fn clamp_rect_keeps_oversized_offscreen_window_inside_small_view() {
        let bounds = egui::Rect::from_min_size(egui::pos2(40.0, 30.0), egui::vec2(300.0, 180.0));
        let saved = egui::Rect::from_min_size(egui::pos2(-500.0, 600.0), egui::vec2(640.0, 480.0));
        assert_eq!(clamp_rect(saved, bounds, egui::vec2(160.0, 100.0)), bounds);
    }

    #[test]
    fn clamp_rect_preserves_in_bounds_geometry_and_is_idempotent() {
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let saved = egui::Rect::from_min_size(egui::pos2(120.0, 80.0), egui::vec2(400.0, 300.0));
        let fitted = clamp_rect(saved, bounds, egui::vec2(160.0, 100.0));
        assert_eq!(fitted, saved);
        assert_eq!(clamp_rect(fitted, bounds, egui::vec2(160.0, 100.0)), fitted);
    }

    #[test]
    fn fit_root_window_centers_inside_dpi_scaled_work_area() {
        let work_area =
            egui::Rect::from_min_size(egui::pos2(120.0, 40.0), egui::vec2(1280.0, 680.0));
        let fitted = fit_root_window(
            work_area,
            egui::vec2(1400.0, 900.0),
            egui::vec2(960.0, 640.0),
        );
        assert!(fitted.inner_size.x <= 1248.0);
        assert!(fitted.inner_size.y <= 624.0);
        assert!(fitted.minimum_inner_size.x <= fitted.inner_size.x);
        assert!(fitted.minimum_inner_size.y <= fitted.inner_size.y);
        assert!(fitted.minimum_inner_size.y <= 468.0);
        assert!(fitted.outer_position.x >= work_area.left());
        assert!(fitted.outer_position.y >= work_area.top());
        assert!(fitted.outer_position.x + fitted.inner_size.x + 32.0 <= work_area.right());
        assert!(fitted.outer_position.y + fitted.inner_size.y + 56.0 <= work_area.bottom());

        let small = fit_root_window(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 520.0)),
            egui::vec2(1400.0, 900.0),
            egui::vec2(960.0, 640.0),
        );
        assert!(small.inner_size.x <= 768.0 && small.inner_size.y <= 464.0);
        assert!(small.minimum_inner_size.y < small.inner_size.y);
    }

    #[test]
    fn clamp_rect_handles_empty_work_area() {
        let bounds = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(0.0, 100.0));
        assert_eq!(
            clamp_rect(
                egui::Rect::from_min_size(egui::pos2(-100.0, 500.0), egui::vec2(300.0, 200.0)),
                bounds,
                egui::vec2(160.0, 100.0),
            ),
            egui::Rect::from_min_size(bounds.min, egui::Vec2::ZERO)
        );
    }

    #[test]
    fn shell_safe_rect_excludes_toolbar_dock_and_diagnostics() {
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
        let toolbar = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 44.0));
        let dock = egui::Rect::from_min_size(egui::pos2(0.0, 720.0), egui::vec2(1200.0, 32.0));
        let diagnostics =
            egui::Rect::from_min_size(egui::pos2(0.0, 752.0), egui::vec2(1200.0, 48.0));
        assert_eq!(
            shell_safe_rect(viewport, toolbar, dock, Some(diagnostics)),
            egui::Rect::from_min_max(egui::pos2(0.0, 44.0), egui::pos2(1200.0, 720.0))
        );
    }

    #[test]
    fn cascade_rect_offsets_fresh_windows_and_stays_inside_bounds() {
        let bounds = egui::Rect::from_min_size(egui::pos2(320.0, 48.0), egui::vec2(880.0, 690.0));
        let size = egui::vec2(480.0, 360.0);
        let first = cascade_rect(bounds, size, 0);
        let second = cascade_rect(bounds, size, 1);
        let wrapped = cascade_rect(bounds, size, usize::MAX);
        assert_eq!(first.min, bounds.min + egui::vec2(8.0, 8.0));
        assert_eq!(second.min, first.min + egui::vec2(32.0, 32.0));
        assert_ne!(first.min, wrapped.min);
        assert!(bounds.contains_rect(first));
        assert!(bounds.contains_rect(second));
        assert!(bounds.contains_rect(wrapped));

        let oversized = cascade_rect(bounds, egui::vec2(2_000.0, 1_500.0), 0);
        assert!(bounds.contains_rect(oversized));
    }
}
