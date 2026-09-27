use std::cmp::Ordering;

use eframe::egui;
use serde::{Deserialize, Serialize};

const SURFACE_MIN_WINDOW_WIDTH: u32 = 420;
const SURFACE_MAX_WINDOW_WIDTH: u32 = 2_400;
const SURFACE_MIN_WINDOW_HEIGHT: u32 = 300;
const SURFACE_MAX_WINDOW_HEIGHT: u32 = 1_800;

#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceData {
    pub rows: usize,
    pub columns: usize,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub values: Vec<Option<f64>>,
}

impl SurfaceData {
    pub fn value(&self, row: usize, column: usize) -> Option<f64> {
        if row >= self.rows || column >= self.columns {
            return None;
        }
        self.values
            .get(row.saturating_mul(self.columns).saturating_add(column))
            .copied()
            .flatten()
    }

    pub fn finite_value_count(&self) -> usize {
        self.values.iter().flatten().count()
    }

    pub fn missing_value_count(&self) -> usize {
        self.rows
            .saturating_mul(self.columns)
            .saturating_sub(self.finite_value_count())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceRange {
    pub minimum: f64,
    pub maximum: f64,
}

impl SurfaceRange {
    pub fn span(self) -> f64 {
        (self.maximum - self.minimum).max(f64::EPSILON)
    }

    pub fn normalize(self, value: f64) -> f64 {
        ((value - self.minimum) / self.span()).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct SurfaceViewMemory {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default = "crate::window_geometry::legacy_position_saved")]
    pub position_saved: bool,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom_percent: u16,
    pub pan_x: f32,
    pub pan_y: f32,
    pub height_exaggeration: f32,
    pub auto_range: bool,
    pub fixed_min: Option<f64>,
    pub fixed_max: Option<f64>,
    pub wireframe: bool,
    pub show_axes: bool,
}

impl Default for SurfaceViewMemory {
    fn default() -> Self {
        Self {
            x: 140,
            y: 120,
            width: 900,
            height: 640,
            position_saved: false,
            yaw: -45.0,
            pitch: 34.0,
            zoom_percent: 100,
            pan_x: 0.0,
            pan_y: 0.0,
            height_exaggeration: 100.0,
            auto_range: true,
            fixed_min: None,
            fixed_max: None,
            wireframe: false,
            show_axes: true,
        }
    }
}

impl SurfaceViewMemory {
    pub fn sanitize(&mut self) {
        self.width = self
            .width
            .clamp(SURFACE_MIN_WINDOW_WIDTH, SURFACE_MAX_WINDOW_WIDTH);
        self.height = self
            .height
            .clamp(SURFACE_MIN_WINDOW_HEIGHT, SURFACE_MAX_WINDOW_HEIGHT);
        if !self.yaw.is_finite() {
            self.yaw = -45.0;
        }
        if !self.pitch.is_finite() {
            self.pitch = 34.0;
        }
        if !self.pan_x.is_finite() {
            self.pan_x = 0.0;
        }
        if !self.pan_y.is_finite() {
            self.pan_y = 0.0;
        }
        if !self.height_exaggeration.is_finite() {
            self.height_exaggeration = 100.0;
        }
        self.yaw = self.yaw.clamp(-360.0, 360.0);
        self.pitch = self.pitch.clamp(-80.0, 80.0);
        self.zoom_percent = self.zoom_percent.clamp(30, 300);
        self.pan_x = self.pan_x.clamp(-2.0, 2.0);
        self.pan_y = self.pan_y.clamp(-2.0, 2.0);
        self.height_exaggeration = self.height_exaggeration.clamp(10.0, 400.0);
        if self.fixed_min.is_some_and(|value| !value.is_finite()) {
            self.fixed_min = None;
        }
        if self.fixed_max.is_some_and(|value| !value.is_finite()) {
            self.fixed_max = None;
        }
        if let (Some(minimum), Some(maximum)) = (self.fixed_min, self.fixed_max) {
            if minimum >= maximum {
                self.fixed_max = None;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceTriangle {
    pub row: usize,
    pub column: usize,
    pub triangle_index: u8,
    pub points: [egui::Pos2; 3],
    pub outer_edges: [bool; 3],
    pub value: f64,
    pub depth: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfacePoint {
    pub row: usize,
    pub column: usize,
    pub position: egui::Pos2,
    pub value: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceAxisTick {
    pub value: f64,
    pub position: f32,
}

pub fn auto_surface_range(values: impl IntoIterator<Item = f64>) -> Option<SurfaceRange> {
    let mut finite = values.into_iter().filter(|value| value.is_finite());
    let first = finite.next()?;
    let mut minimum = first;
    let mut maximum = first;
    for value in finite {
        minimum = minimum.min(value);
        maximum = maximum.max(value);
    }
    let span = maximum - minimum;
    let padding = if span.is_finite() && span > 0.0 {
        span * 0.05
    } else {
        (minimum.abs() * 0.05).max(1.0)
    };
    let padded_minimum = minimum - padding;
    let padded_maximum = maximum + padding;
    Some(SurfaceRange {
        minimum: if padded_minimum.is_finite() {
            padded_minimum
        } else {
            minimum
        },
        maximum: if padded_maximum.is_finite() {
            padded_maximum
        } else {
            maximum
        },
    })
}

pub fn project_surface(
    data: &SurfaceData,
    view: &SurfaceViewMemory,
    rect: egui::Rect,
    range: SurfaceRange,
) -> Vec<SurfaceTriangle> {
    project_surface_geometry(data, view, rect, range).1
}

pub fn project_surface_points(
    data: &SurfaceData,
    view: &SurfaceViewMemory,
    rect: egui::Rect,
    range: SurfaceRange,
) -> Vec<SurfacePoint> {
    project_surface_geometry(data, view, rect, range).0
}

pub fn project_surface_geometry(
    data: &SurfaceData,
    view: &SurfaceViewMemory,
    rect: egui::Rect,
    range: SurfaceRange,
) -> (Vec<SurfacePoint>, Vec<SurfaceTriangle>) {
    if data.rows == 0
        || data.columns == 0
        || data.x.len() < data.columns
        || data.y.len() < data.rows
    {
        return (Vec::new(), Vec::new());
    }
    let x_positions = normalized_positions(&data.x[..data.columns]);
    let y_positions = normalized_positions(&data.y[..data.rows]);
    let scale = rect.width().min(rect.height()).mul_add(0.27, 0.0)
        * f32::from(view.zoom_percent.clamp(30, 300))
        / 100.0;
    let center = rect.center();
    let yaw = view.yaw.to_radians();
    let pitch = view.pitch.to_radians();
    let height = f64::from(view.height_exaggeration.clamp(10.0, 400.0)) / 100.0;
    let mut vertices = Vec::with_capacity(data.rows.saturating_mul(data.columns));
    let mut points = Vec::new();
    for (row, &y) in y_positions.iter().enumerate() {
        for (column, &x) in x_positions.iter().enumerate() {
            let value = data.value(row, column);
            let projected = value.map(|value| {
                project_point(
                    x,
                    y,
                    range.normalize(value),
                    center,
                    scale,
                    yaw,
                    pitch,
                    height,
                    view.pan_x,
                    view.pan_y,
                )
            });
            vertices.push(projected);
            if let (Some(value), Some((position, _))) =
                (value.filter(|value| value.is_finite()), projected)
            {
                points.push(SurfacePoint {
                    row,
                    column,
                    position,
                    value,
                });
            }
        }
    }
    if data.rows < 2 || data.columns < 2 {
        return (points, Vec::new());
    }

    let mut faces = Vec::new();
    for row in 0..data.rows - 1 {
        for column in 0..data.columns - 1 {
            let (Some(top_left), Some(top_right), Some(bottom_right), Some(bottom_left)) = (
                data.value(row, column),
                data.value(row, column + 1),
                data.value(row + 1, column + 1),
                data.value(row + 1, column),
            ) else {
                continue;
            };
            let index = row * data.columns + column;
            let (
                Some((top_left_point, top_left_depth)),
                Some((top_right_point, top_right_depth)),
                Some((bottom_right_point, bottom_right_depth)),
                Some((bottom_left_point, bottom_left_depth)),
            ) = (
                vertices[index],
                vertices[index + 1],
                vertices[index + data.columns + 1],
                vertices[index + data.columns],
            )
            else {
                continue;
            };
            let face_points = [
                top_left_point,
                top_right_point,
                bottom_right_point,
                bottom_left_point,
            ];
            let value = (top_left + top_right + bottom_right + bottom_left) / 4.0;
            let vertex_depths = [
                top_left_depth,
                top_right_depth,
                bottom_right_depth,
                bottom_left_depth,
            ];
            for (triangle_index, corners, outer_edges) in [
                (0, [0, 1, 2], [true, true, false]),
                (1, [0, 2, 3], [false, true, true]),
            ] {
                let depth = corners
                    .iter()
                    .map(|&corner| vertex_depths[corner])
                    .sum::<f32>()
                    / 3.0;
                faces.push(SurfaceTriangle {
                    row,
                    column,
                    triangle_index,
                    points: corners.map(|corner| face_points[corner]),
                    outer_edges,
                    value,
                    depth,
                });
            }
        }
    }
    faces.sort_by(|left, right| {
        left.depth
            .partial_cmp(&right.depth)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.row.cmp(&right.row))
            .then_with(|| left.column.cmp(&right.column))
            .then_with(|| left.triangle_index.cmp(&right.triangle_index))
    });
    (points, faces)
}

pub fn project_surface_location(
    view: &SurfaceViewMemory,
    rect: egui::Rect,
    range: SurfaceRange,
    x_position: f32,
    y_position: f32,
    value: f64,
) -> egui::Pos2 {
    let scale = rect.width().min(rect.height()).mul_add(0.27, 0.0)
        * f32::from(view.zoom_percent.clamp(30, 300))
        / 100.0;
    project_point(
        x_position.clamp(0.0, 1.0),
        y_position.clamp(0.0, 1.0),
        range.normalize(if value.is_finite() {
            value
        } else {
            range.minimum
        }),
        rect.center(),
        scale,
        view.yaw.to_radians(),
        view.pitch.to_radians(),
        f64::from(view.height_exaggeration.clamp(10.0, 400.0)) / 100.0,
        view.pan_x,
        view.pan_y,
    )
    .0
}

pub fn surface_engineering_delta(
    view: &SurfaceViewMemory,
    rect: egui::Rect,
    range: SurfaceRange,
    screen_delta: egui::Vec2,
) -> Option<f64> {
    let minimum = project_surface_location(view, rect, range, 0.0, 0.0, range.minimum);
    let maximum = project_surface_location(view, rect, range, 0.0, 0.0, range.maximum);
    let pixels_per_range = maximum.y - minimum.y;
    if !pixels_per_range.is_finite() || pixels_per_range.abs() <= f32::EPSILON {
        return None;
    }
    let delta = f64::from(screen_delta.y) / f64::from(pixels_per_range) * range.span();
    delta.is_finite().then_some(delta)
}

pub fn surface_soft_pull_weight(distance: f32, reference_distance: f32) -> f64 {
    if !distance.is_finite() || distance <= 0.0 {
        return 1.0;
    }
    if !reference_distance.is_finite() || reference_distance <= f32::EPSILON {
        return 0.0;
    }
    1.0 / (1.0 + f64::from(distance) / f64::from(reference_distance))
}

pub fn surface_axis_ticks(values: &[f64], max_ticks: usize) -> Vec<SurfaceAxisTick> {
    if values.is_empty() || max_ticks == 0 {
        return Vec::new();
    }
    let display_values: Vec<f64> = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if value.is_finite() {
                *value
            } else {
                index as f64
            }
        })
        .collect();
    let positions = normalized_positions(&display_values);
    let count = max_ticks.min(display_values.len());
    if count == 1 {
        return vec![SurfaceAxisTick {
            value: display_values[0],
            position: positions[0],
        }];
    }
    (0..count)
        .map(|tick_index| {
            let index = ((tick_index * (display_values.len() - 1)) as f64 / (count - 1) as f64)
                .round() as usize;
            SurfaceAxisTick {
                value: display_values[index],
                position: positions[index],
            }
        })
        .collect()
}

pub fn nearest_surface_point(
    points: &[SurfacePoint],
    position: egui::Pos2,
    radius: f32,
) -> Option<SurfacePoint> {
    let radius_squared = radius.max(0.0).powi(2);
    points
        .iter()
        .filter_map(|point| {
            let delta = point.position - position;
            let distance_squared = delta.x.mul_add(delta.x, delta.y * delta.y);
            (distance_squared <= radius_squared).then_some((*point, distance_squared))
        })
        .min_by(|(left, left_distance), (right, right_distance)| {
            left_distance
                .partial_cmp(right_distance)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.row.cmp(&right.row))
                .then_with(|| left.column.cmp(&right.column))
        })
        .map(|(point, _)| point)
}

pub fn surface_selection_for_rect(
    points: &[SurfacePoint],
    rect: egui::Rect,
) -> Option<(usize, usize, usize, usize)> {
    let mut bounds: Option<(usize, usize, usize, usize)> = None;
    for point in points.iter().filter(|point| rect.contains(point.position)) {
        bounds = Some(match bounds {
            Some((min_row, min_column, max_row, max_column)) => (
                min_row.min(point.row),
                min_column.min(point.column),
                max_row.max(point.row),
                max_column.max(point.column),
            ),
            None => (point.row, point.column, point.row, point.column),
        });
    }
    bounds
}

fn normalized_positions(values: &[f64]) -> Vec<f32> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for value in values.iter().copied().filter(|value| value.is_finite()) {
        minimum = minimum.min(value);
        maximum = maximum.max(value);
    }
    let span = maximum - minimum;
    if !minimum.is_finite() || !maximum.is_finite() || !span.is_finite() || span <= 0.0 {
        return (0..values.len())
            .map(|index| {
                if values.len() <= 1 {
                    0.0
                } else {
                    index as f32 / (values.len() - 1) as f32
                }
            })
            .collect();
    }
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if value.is_finite() {
                ((value - minimum) / span).clamp(0.0, 1.0) as f32
            } else if values.len() <= 1 {
                0.0
            } else {
                index as f32 / (values.len() - 1) as f32
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn project_point(
    x_position: f32,
    y_position: f32,
    z_position: f64,
    center: egui::Pos2,
    scale: f32,
    yaw: f32,
    pitch: f32,
    height: f64,
    pan_x: f32,
    pan_y: f32,
) -> (egui::Pos2, f32) {
    let x = f64::from(x_position.mul_add(2.0, -1.0));
    let y = f64::from(y_position.mul_add(2.0, -1.0));
    let z = z_position * height;
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    let rotated_x = cos_yaw as f64 * x - sin_yaw as f64 * y;
    let depth = sin_yaw as f64 * x + cos_yaw as f64 * y;
    let screen_y = cos_pitch as f64 * z - sin_pitch as f64 * depth;
    let camera_depth = sin_pitch as f64 * z + cos_pitch as f64 * depth;
    (
        egui::pos2(
            center.x + rotated_x as f32 * scale + pan_x * scale,
            center.y - screen_y as f32 * scale + pan_y * scale,
        ),
        camera_depth as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_auto_range_adds_padding_and_handles_flat_values() {
        let range = auto_surface_range([10.0, 20.0]).unwrap();
        assert_eq!(range.minimum, 9.5);
        assert_eq!(range.maximum, 20.5);

        let flat = auto_surface_range([42.0, 42.0]).unwrap();
        assert_eq!(flat.minimum, 39.9);
        assert_eq!(flat.maximum, 44.1);
    }

    #[test]
    fn surface_projection_is_deterministic_and_depth_sorted() {
        let data = SurfaceData {
            rows: 2,
            columns: 2,
            x: vec![0.0, 1.0],
            y: vec![0.0, 1.0],
            values: vec![Some(0.0), Some(1.0), Some(2.0), Some(3.0)],
        };
        let view = SurfaceViewMemory::default();
        let range = auto_surface_range(data.values.iter().flatten().copied()).unwrap();
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));

        let first = project_surface(&data, &view, rect, range);
        let second = project_surface(&data, &view, rect, range);

        assert_eq!(first, second);
        assert_eq!(first.len(), 2);
        assert!(first[0].points.iter().all(|point| rect.contains(*point)));
    }

    #[test]
    fn surface_faces_sort_by_camera_depth_after_rotation() {
        let data = SurfaceData {
            rows: 2,
            columns: 3,
            x: vec![0.0, 1.0, 2.0],
            y: vec![0.0, 1.0],
            values: vec![
                Some(0.0),
                Some(0.5),
                Some(1.0),
                Some(0.0),
                Some(0.5),
                Some(1.0),
            ],
        };
        let view = SurfaceViewMemory {
            yaw: 0.0,
            pitch: 34.0,
            ..SurfaceViewMemory::default()
        };
        let range = SurfaceRange {
            minimum: 0.0,
            maximum: 1.0,
        };
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let faces = project_surface(&data, &view, rect, range);

        assert_eq!(
            faces
                .iter()
                .map(|face| (face.row, face.column, face.triangle_index))
                .collect::<Vec<_>>(),
            [(0, 0, 0), (0, 1, 0), (0, 0, 1), (0, 1, 1)]
        );
        assert!(faces[0].depth < faces[1].depth);
    }

    #[test]
    fn surface_projection_splits_warped_cells_into_triangles() {
        let data = SurfaceData {
            rows: 2,
            columns: 2,
            x: vec![0.0, 1.0],
            y: vec![0.0, 1.0],
            values: vec![Some(0.0), Some(1.0), Some(0.0), Some(0.0)],
        };
        let view = SurfaceViewMemory {
            yaw: 0.0,
            pitch: -25.0,
            ..SurfaceViewMemory::default()
        };
        let range = SurfaceRange {
            minimum: 0.0,
            maximum: 1.0,
        };
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let faces = project_surface(&data, &view, rect, range);

        assert_eq!(faces.len(), 2);
        assert!(faces.iter().all(|face| face.points.len() == 3));
        let mut edge_flags: Vec<_> = faces
            .iter()
            .map(|face| (face.triangle_index, face.outer_edges))
            .collect();
        edge_flags.sort_unstable_by_key(|(triangle_index, _)| *triangle_index);
        assert_eq!(
            edge_flags,
            [(0, [true, true, false]), (1, [false, true, true])]
        );
    }

    #[test]
    fn shared_surface_projection_preserves_vertices_holes_and_order() {
        let data = SurfaceData {
            rows: 3,
            columns: 4,
            x: vec![10.0, 20.0, 30.0, 40.0],
            y: vec![100.0, 200.0, 300.0],
            values: vec![
                Some(0.0),
                Some(1.0),
                Some(2.0),
                Some(3.0),
                Some(4.0),
                Some(5.0),
                None,
                Some(7.0),
                Some(8.0),
                Some(9.0),
                Some(10.0),
                Some(11.0),
            ],
        };
        let range = auto_surface_range(data.values.iter().flatten().copied()).unwrap();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let (points, faces) =
            project_surface_geometry(&data, &SurfaceViewMemory::default(), rect, range);

        let point_cells: Vec<_> = points
            .iter()
            .map(|point| (point.row, point.column))
            .collect();
        assert_eq!(
            point_cells,
            [
                (0, 0),
                (0, 1),
                (0, 2),
                (0, 3),
                (1, 0),
                (1, 1),
                (1, 3),
                (2, 0),
                (2, 1),
                (2, 2),
                (2, 3),
            ]
        );
        let mut face_cells: Vec<_> = faces.iter().map(|face| (face.row, face.column)).collect();
        face_cells.sort_unstable();
        assert_eq!(face_cells, [(0, 0), (0, 0), (1, 0), (1, 0)]);
        for face in &faces {
            let cells = match face.triangle_index {
                0 => [
                    (face.row, face.column),
                    (face.row, face.column + 1),
                    (face.row + 1, face.column + 1),
                ],
                1 => [
                    (face.row, face.column),
                    (face.row + 1, face.column + 1),
                    (face.row + 1, face.column),
                ],
                _ => unreachable!("surface triangles have stable indices 0 and 1"),
            };
            for (corner, cell) in face.points.iter().zip(cells) {
                let point = points
                    .iter()
                    .find(|point| (point.row, point.column) == cell)
                    .unwrap();
                assert_eq!(*corner, point.position);
            }
        }
        assert!(faces.windows(2).all(|pair| {
            pair[0].depth < pair[1].depth
                || (pair[0].depth == pair[1].depth
                    && (pair[0].row, pair[0].column, pair[0].triangle_index)
                        <= (pair[1].row, pair[1].column, pair[1].triangle_index))
        }));
    }

    #[test]
    fn surface_data_preserves_missing_cells_as_holes() {
        let data = SurfaceData {
            rows: 2,
            columns: 2,
            x: vec![0.0, 1.0],
            y: vec![0.0, 1.0],
            values: vec![Some(1.0), None, Some(2.0), Some(3.0)],
        };
        let range = auto_surface_range(data.values.iter().flatten().copied()).unwrap();
        let faces = project_surface(
            &data,
            &SurfaceViewMemory::default(),
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(500.0, 400.0)),
            range,
        );

        assert!(faces.is_empty());
        assert_eq!(data.values[1], None);
    }

    #[test]
    fn surface_axis_ticks_use_actual_values_and_limit_count() {
        let ticks = surface_axis_ticks(&[10.0, 25.0, 40.0, 100.0, 160.0, 250.0], 4);

        assert_eq!(ticks.len(), 4);
        assert_eq!(
            ticks.iter().map(|tick| tick.value).collect::<Vec<_>>(),
            vec![10.0, 40.0, 100.0, 250.0]
        );
        assert_eq!(ticks.first().unwrap().position, 0.0);
        assert_eq!(ticks.last().unwrap().position, 1.0);
    }

    #[test]
    fn surface_points_are_cell_addressed_and_nearest_hit_is_deterministic() {
        let data = SurfaceData {
            rows: 2,
            columns: 2,
            x: vec![100.0, 200.0],
            y: vec![1.0, 2.0],
            values: vec![Some(10.0), Some(20.0), Some(30.0), Some(40.0)],
        };
        let range = auto_surface_range(data.values.iter().flatten().copied()).unwrap();
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let points = project_surface_points(&data, &SurfaceViewMemory::default(), rect, range);

        assert_eq!(points.len(), 4);
        assert_eq!(
            points
                .iter()
                .map(|point| (point.row, point.column, point.value))
                .collect::<Vec<_>>(),
            vec![(0, 0, 10.0), (0, 1, 20.0), (1, 0, 30.0), (1, 1, 40.0),]
        );

        let hit = nearest_surface_point(&points, points[2].position, 0.1).unwrap();
        assert_eq!((hit.row, hit.column), (1, 0));
    }

    #[test]
    fn surface_selection_for_rect_returns_logical_bounds() {
        let points = vec![
            SurfacePoint {
                row: 0,
                column: 0,
                position: egui::pos2(10.0, 10.0),
                value: 1.0,
            },
            SurfacePoint {
                row: 0,
                column: 1,
                position: egui::pos2(20.0, 10.0),
                value: 2.0,
            },
            SurfacePoint {
                row: 1,
                column: 0,
                position: egui::pos2(10.0, 20.0),
                value: 3.0,
            },
            SurfacePoint {
                row: 1,
                column: 1,
                position: egui::pos2(20.0, 20.0),
                value: 4.0,
            },
        ];

        assert_eq!(
            surface_selection_for_rect(
                &points,
                egui::Rect::from_min_max(egui::pos2(9.0, 9.0), egui::pos2(21.0, 21.0)),
            ),
            Some((0, 0, 1, 1))
        );
        assert_eq!(
            surface_selection_for_rect(
                &points,
                egui::Rect::from_min_max(egui::pos2(30.0, 30.0), egui::pos2(40.0, 40.0)),
            ),
            None
        );
    }

    #[test]
    fn surface_engineering_delta_follows_projected_z_direction() {
        let view = SurfaceViewMemory::default();
        let range = SurfaceRange {
            minimum: 0.0,
            maximum: 100.0,
        };
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));

        let upward = surface_engineering_delta(&view, rect, range, egui::vec2(0.0, -20.0))
            .expect("default view should expose a usable Z direction");
        let downward = surface_engineering_delta(&view, rect, range, egui::vec2(0.0, 20.0))
            .expect("default view should expose a usable Z direction");

        assert!(upward > 0.0);
        assert!(downward < 0.0);
        assert!((upward + downward).abs() < 1.0e-10);
    }

    #[test]
    fn surface_soft_pull_weight_is_full_at_anchor_and_falls_off_smoothly() {
        assert_eq!(surface_soft_pull_weight(0.0, 10.0), 1.0);
        assert_eq!(surface_soft_pull_weight(10.0, 10.0), 0.5);
        assert!(surface_soft_pull_weight(20.0, 10.0) < 0.5);
        assert!(surface_soft_pull_weight(20.0, 10.0) > 0.0);
    }
}
