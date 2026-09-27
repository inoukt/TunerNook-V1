use serde::{Deserialize, Serialize};
use tuner_core::BinDocument;
use tuner_xdf::{RawValue, XdfDocument};

use eframe::egui;

use crate::auto_surface_range;
use crate::surface::{SurfaceData, SurfaceRange};
use crate::window_geometry::clamp_rect;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareValueMode {
    #[default]
    Destination,
    Source,
    AbsoluteDelta,
    PercentDelta,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CompareCacheKey {
    pub source_bin_sha256: String,
    pub destination_revision: u64,
    pub source_xdf_fingerprint: String,
    pub destination_xdf_fingerprint: String,
    pub semantic_id: String,
    pub mode: CompareValueMode,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct CompareWindowMemory {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default = "crate::window_geometry::legacy_position_saved")]
    pub position_saved: bool,
}

impl Default for CompareWindowMemory {
    fn default() -> Self {
        Self {
            x: 160,
            y: 100,
            width: 900,
            height: 640,
            position_saved: false,
        }
    }
}

impl CompareWindowMemory {
    pub fn sanitize(&mut self) {
        self.width = self.width.clamp(420, 2_400);
        self.height = self.height.clamp(300, 1_800);
    }

    pub fn constrain_to_bounds(&mut self, left: i32, top: i32, right: i32, bottom: i32) {
        let bounds = egui::Rect::from_min_size(
            egui::pos2(left as f32, top as f32),
            egui::vec2(
                right.saturating_sub(left).max(1) as f32,
                bottom.saturating_sub(top).max(1) as f32,
            ),
        );
        let rect = clamp_rect(
            egui::Rect::from_min_size(
                egui::pos2(self.x as f32, self.y as f32),
                egui::vec2(self.width as f32, self.height as f32),
            ),
            bounds,
            egui::vec2(420.0, 300.0),
        );
        self.x = rect.left().round() as i32;
        self.y = rect.top().round() as i32;
        self.width = rect.width().round().max(1.0) as u32;
        self.height = rect.height().round().max(1.0) as u32;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompareCell {
    pub destination: Option<f64>,
    pub source: Option<f64>,
}

impl CompareCell {
    pub fn from_values(destination: Option<f64>, source: Option<f64>) -> Self {
        Self {
            destination: destination.filter(|value| value.is_finite()),
            source: source.filter(|value| value.is_finite()),
        }
    }

    pub fn value(self, mode: CompareValueMode) -> Option<f64> {
        match mode {
            CompareValueMode::Destination => self.destination,
            CompareValueMode::Source => self.source,
            CompareValueMode::AbsoluteDelta => {
                Some(self.source? - self.destination?).filter(|value| value.is_finite())
            }
            CompareValueMode::PercentDelta => {
                let destination = self.destination?;
                let source = self.source?;
                if destination == 0.0 {
                    return None;
                }
                Some((source - destination) / destination.abs() * 100.0)
                    .filter(|value| value.is_finite())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompareMapData {
    pub semantic_id: String,
    pub rows: usize,
    pub columns: usize,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub cells: Vec<CompareCell>,
}

impl CompareMapData {
    pub fn new(
        semantic_id: String,
        rows: usize,
        columns: usize,
        x: Vec<f64>,
        y: Vec<f64>,
        cells: Vec<CompareCell>,
    ) -> Self {
        Self {
            semantic_id,
            rows,
            columns,
            x,
            y,
            cells,
        }
    }

    pub fn value(&self, row: usize, column: usize, mode: CompareValueMode) -> Option<f64> {
        if row >= self.rows || column >= self.columns {
            return None;
        }
        self.cells
            .get(row.saturating_mul(self.columns).saturating_add(column))
            .copied()
            .and_then(|cell| cell.value(mode))
    }

    pub fn values(&self, mode: CompareValueMode) -> impl Iterator<Item = f64> + '_ {
        self.cells.iter().filter_map(move |cell| cell.value(mode))
    }

    pub fn surface_data(&self, mode: CompareValueMode) -> SurfaceData {
        let values = (0..self.rows)
            .flat_map(|row| (0..self.columns).map(move |column| self.value(row, column, mode)))
            .collect();
        SurfaceData {
            rows: self.rows,
            columns: self.columns,
            x: self.x.clone(),
            y: self.y.clone(),
            values,
        }
    }

    pub fn range(&self, mode: CompareValueMode, _auto_range: bool) -> Option<SurfaceRange> {
        let (minimum, maximum) = match mode {
            CompareValueMode::AbsoluteDelta | CompareValueMode::PercentDelta => {
                symmetric_delta_range(self.values(mode))?
            }
            CompareValueMode::Destination | CompareValueMode::Source => {
                let range = auto_surface_range(self.values(mode))?;
                (range.minimum, range.maximum)
            }
        };
        Some(SurfaceRange { minimum, maximum })
    }
}

pub fn symmetric_delta_range(values: impl IntoIterator<Item = f64>) -> Option<(f64, f64)> {
    let maximum = values
        .into_iter()
        .filter(|value| value.is_finite())
        .map(f64::abs)
        .fold(None, |maximum: Option<f64>, value| {
            Some(maximum.map_or(value, |current| current.max(value)))
        })?;
    let extent = (maximum * 1.05).max(1.0);
    Some((-extent, extent))
}

pub fn build_compare_map_data(
    source_xdf: &XdfDocument,
    destination_xdf: &XdfDocument,
    source_bin: &BinDocument,
    destination_bin: &BinDocument,
    semantic_id: &str,
) -> Result<CompareMapData, String> {
    let source = source_xdf
        .parameter(semantic_id)
        .ok_or_else(|| format!("source XDF has no parameter '{semantic_id}'"))?;
    let destination = destination_xdf
        .parameter(semantic_id)
        .ok_or_else(|| format!("destination XDF has no parameter '{semantic_id}'"))?;
    let source_dimensions = source.dimensions();
    let destination_dimensions = destination.dimensions();
    if source_dimensions != destination_dimensions {
        return Err(format!(
            "parameter '{semantic_id}' dimensions differ: source={}x{}, destination={}x{}",
            source_dimensions.rows,
            source_dimensions.columns,
            destination_dimensions.rows,
            destination_dimensions.columns
        ));
    }

    let rows = destination_dimensions.rows;
    let columns = destination_dimensions.columns;
    let mut cells = Vec::with_capacity(rows.saturating_mul(columns));
    for row in 0..rows {
        for column in 0..columns {
            cells.push(CompareCell::from_values(
                read_value(destination, destination_bin, row, column),
                read_value(source, source_bin, row, column),
            ));
        }
    }

    Ok(CompareMapData::new(
        semantic_id.to_string(),
        rows,
        columns,
        axis_values(
            destination,
            destination_bin,
            axis_index_for_role(destination, &["x", "column", "columns"], columns, 0),
            columns,
        ),
        axis_values(
            destination,
            destination_bin,
            axis_index_for_role(destination, &["y", "row", "rows"], rows, 1),
            rows,
        ),
        cells,
    ))
}

fn read_value(
    parameter: &tuner_xdf::ParameterDefinition,
    bin: &BinDocument,
    row: usize,
    column: usize,
) -> Option<f64> {
    parameter
        .read_engineering_cell(bin, row, column)
        .ok()
        .filter(|value| value.is_finite())
        .or_else(|| {
            parameter
                .read_raw_cell(bin, row, column)
                .ok()
                .and_then(raw_value_as_f64)
        })
}

fn axis_values(
    parameter: &tuner_xdf::ParameterDefinition,
    bin: &BinDocument,
    axis_index: Option<usize>,
    count: usize,
) -> Vec<f64> {
    let Some(axis_index) = axis_index else {
        return index_values(count);
    };
    let Some(axis) = parameter.axes.get(axis_index) else {
        return index_values(count);
    };
    if axis.count < count {
        return index_values(count);
    }
    let values: Vec<_> = (0..count)
        .map(|index| {
            parameter
                .read_engineering_axis(bin, axis_index, index)
                .ok()
                .filter(|value| value.is_finite())
                .or_else(|| {
                    parameter
                        .read_raw_axis(bin, axis_index, index)
                        .ok()
                        .and_then(raw_value_as_f64)
                })
        })
        .collect();
    if values.iter().all(Option::is_some) {
        values.into_iter().map(Option::unwrap).collect::<Vec<_>>()
    } else {
        index_values(count)
    }
}

fn axis_index_for_role(
    parameter: &tuner_xdf::ParameterDefinition,
    explicit_ids: &[&str],
    expected_count: usize,
    positional_index: usize,
) -> Option<usize> {
    if parameter.axes.len() == 1 && parameter.axes[0].id.trim().eq_ignore_ascii_case("z") {
        return None;
    }
    let explicit_id_present = parameter.axes.iter().any(|axis| {
        explicit_ids
            .iter()
            .any(|id| axis.id.trim().eq_ignore_ascii_case(id))
    });
    for id in explicit_ids {
        if let Some((index, _)) = parameter.axes.iter().enumerate().find(|(_, axis)| {
            axis.count == expected_count && axis.id.trim().eq_ignore_ascii_case(id)
        }) {
            return Some(index);
        }
    }
    if explicit_id_present {
        return None;
    }
    parameter
        .axes
        .get(positional_index)
        .filter(|axis| axis.count == expected_count)
        .map(|_| positional_index)
}

fn index_values(count: usize) -> Vec<f64> {
    (0..count).map(|index| index as f64).collect()
}

fn raw_value_as_f64(raw: RawValue) -> Option<f64> {
    let value = match raw {
        RawValue::Unsigned(value) => value as f64,
        RawValue::Signed(value) => value as f64,
        RawValue::Float32Bits(bits) => f32::from_bits(bits) as f64,
    };
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SurfaceRange;

    #[test]
    fn compare_cell_delta_is_source_minus_destination() {
        let cell = CompareCell::from_values(Some(10.0), Some(15.0));

        assert_eq!(cell.value(CompareValueMode::Destination), Some(10.0));
        assert_eq!(cell.value(CompareValueMode::Source), Some(15.0));
        assert_eq!(cell.value(CompareValueMode::AbsoluteDelta), Some(5.0));
        assert_eq!(cell.value(CompareValueMode::PercentDelta), Some(50.0));
    }

    #[test]
    fn compare_cell_percentage_delta_is_unavailable_for_zero_destination() {
        let cell = CompareCell::from_values(Some(0.0), Some(12.0));

        assert_eq!(cell.value(CompareValueMode::AbsoluteDelta), Some(12.0));
        assert_eq!(cell.value(CompareValueMode::PercentDelta), None);
    }

    #[test]
    fn compare_map_preserves_missing_cells_in_every_mode() {
        let map = CompareMapData::new(
            "table:uid:map".to_string(),
            1,
            2,
            vec![0.0, 1.0],
            vec![0.0],
            vec![
                CompareCell::from_values(Some(1.0), None),
                CompareCell::from_values(None, Some(2.0)),
            ],
        );

        assert_eq!(map.value(0, 0, CompareValueMode::Source), None);
        assert_eq!(map.value(0, 1, CompareValueMode::Destination), None);
    }

    #[test]
    fn symmetric_delta_range_handles_flat_and_signed_values() {
        assert_eq!(symmetric_delta_range([0.0, 0.0]), Some((-1.0, 1.0)));
        let (minimum, maximum) = symmetric_delta_range([-10.0, 4.0]).unwrap();
        assert!(minimum < -10.0);
        assert!(maximum > 10.0);
        assert!((minimum + maximum).abs() < f64::EPSILON);
    }

    #[test]
    fn compare_surface_data_uses_delta_values_and_preserves_holes() {
        let map = CompareMapData::new(
            "table:map".to_string(),
            2,
            2,
            vec![0.0, 1.0],
            vec![0.0, 1.0],
            vec![
                CompareCell::from_values(Some(1.0), Some(3.0)),
                CompareCell::from_values(Some(2.0), None),
                CompareCell::from_values(Some(4.0), Some(1.0)),
                CompareCell::from_values(Some(5.0), Some(5.0)),
            ],
        );
        let surface = map.surface_data(CompareValueMode::AbsoluteDelta);

        assert_eq!(surface.value(0, 0), Some(2.0));
        assert_eq!(surface.value(0, 1), None);
        assert_eq!(surface.value(1, 0), Some(-3.0));
    }

    #[test]
    fn compare_delta_graph_range_is_symmetric_around_zero() {
        let map = CompareMapData::new(
            "table:map".to_string(),
            1,
            3,
            vec![0.0, 1.0, 2.0],
            vec![0.0],
            vec![
                CompareCell::from_values(Some(0.0), Some(-2.0)),
                CompareCell::from_values(Some(0.0), Some(4.0)),
                CompareCell::from_values(Some(0.0), Some(1.0)),
            ],
        );
        let range: SurfaceRange = map.range(CompareValueMode::AbsoluteDelta, true).unwrap();

        assert!((range.minimum + range.maximum).abs() < f64::EPSILON);
        assert!(range.minimum < -4.0);
        assert!(range.maximum > 4.0);
    }

    #[test]
    fn compare_map_range_preserves_sparse_and_non_finite_values() {
        let map = CompareMapData::new(
            "table:uid:map".to_string(),
            1,
            4,
            vec![0.0, 1.0, 2.0, 3.0],
            vec![0.0],
            vec![
                CompareCell::from_values(Some(10.0), Some(15.0)),
                CompareCell::from_values(Some(0.0), Some(12.0)),
                CompareCell::from_values(None, None),
                CompareCell::from_values(Some(f64::NAN), Some(18.0)),
            ],
        );

        let assert_range = |mode: CompareValueMode, minimum: f64, maximum: f64| {
            let range = map.range(mode, true).unwrap();
            assert!((range.minimum - minimum).abs() < 1e-12);
            assert!((range.maximum - maximum).abs() < 1e-12);
        };
        assert_range(CompareValueMode::Destination, -0.5, 10.5);
        assert_range(CompareValueMode::Source, 11.7, 18.3);
        assert_range(CompareValueMode::AbsoluteDelta, -12.6, 12.6);
        assert_range(CompareValueMode::PercentDelta, -52.5, 52.5);
        assert_eq!(map.value(0, 1, CompareValueMode::PercentDelta), None);

        let empty = CompareMapData::new(
            "table:uid:empty".to_string(),
            1,
            1,
            vec![0.0],
            vec![0.0],
            vec![CompareCell::from_values(None, None)],
        );
        assert_eq!(empty.range(CompareValueMode::Destination, true), None);
    }

    #[test]
    fn compare_window_memory_stays_inside_viewport() {
        let mut memory = CompareWindowMemory {
            x: -900,
            y: 700,
            width: 4_000,
            height: 2_000,
            position_saved: true,
        };

        memory.constrain_to_bounds(0, 0, 1_200, 800);

        assert!(memory.x >= 0);
        assert!(memory.y >= 0);
        assert!(memory.x + memory.width as i32 <= 1_200);
        assert!(memory.y + memory.height as i32 <= 800);
    }
}
