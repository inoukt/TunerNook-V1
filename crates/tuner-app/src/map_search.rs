use std::cmp::Reverse;
use std::collections::BTreeMap;

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use tuner_core::ByteRange;

use crate::window_geometry::clamp_rect;
use crate::{decode_hex_value, HexDisplayFormat, HexEndianness};

const MAX_CANDIDATES: usize = 250;
const MAX_CANDIDATES_PER_SHAPE: usize = 8;
// ponytail: keep brute-force dimension ranges to 128 shapes; add staged scanning if wider ranges are needed.
pub const MAX_DIMENSION_SHAPES: usize = 128;
const MAX_NEARBY_AXIS_BYTES: usize = 128;
// ponytail: poll by batches to keep cancellation responsive without an atomic load per offset.
const CANCEL_CHECK_INTERVAL: usize = 64;

#[derive(Clone, Copy, Debug)]
pub struct MapSearchConfig {
    pub rows_min: usize,
    pub rows_max: usize,
    pub columns_min: usize,
    pub columns_max: usize,
    pub display_format: HexDisplayFormat,
    pub endianness: HexEndianness,
    pub minimum_score: u8,
    pub scan_every_byte: bool,
    pub start_offset: usize,
    pub end_offset: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct MapCandidate {
    pub offset: usize,
    pub byte_length: usize,
    pub rows: usize,
    pub columns: usize,
    pub display_format: HexDisplayFormat,
    pub endianness: HexEndianness,
    pub score: u8,
    pub value_range: (f64, f64),
    pub value_bands: u8,
    pub axis_suggestions: Vec<MapAxisSuggestion>,
    pub selected_x_axis: Option<usize>,
    pub selected_y_axis: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct MapSearchOutcome {
    pub candidates: Vec<MapCandidate>,
    pub cancelled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapAxisRole {
    X,
    Y,
}

impl MapCandidate {
    pub fn set_axis_assignment(
        &mut self,
        role: MapAxisRole,
        suggestion_index: Option<usize>,
    ) -> Result<(), &'static str> {
        let Some(suggestion_index) = suggestion_index else {
            match role {
                MapAxisRole::X => self.selected_x_axis = None,
                MapAxisRole::Y => self.selected_y_axis = None,
            }
            return Ok(());
        };

        let suggestion = self
            .axis_suggestions
            .get(suggestion_index)
            .ok_or("Axis suggestion index is outside this map's suggestions.")?;
        let matches_role = match role {
            MapAxisRole::X => suggestion.matches_columns,
            MapAxisRole::Y => suggestion.matches_rows,
        };
        if !matches_role {
            return Err(match role {
                MapAxisRole::X => "This axis suggestion does not match the map's column count.",
                MapAxisRole::Y => "This axis suggestion does not match the map's row count.",
            });
        }

        match role {
            MapAxisRole::X => self.selected_x_axis = Some(suggestion_index),
            MapAxisRole::Y => self.selected_y_axis = Some(suggestion_index),
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct MapAxisSuggestion {
    pub offset: usize,
    pub values: Vec<f64>,
    pub score: u8,
    pub increasing: bool,
    pub matches_columns: bool,
    pub matches_rows: bool,
    pub display_format: HexDisplayFormat,
    pub endianness: HexEndianness,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct MapFinderMemory {
    pub window_open: bool,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default = "crate::window_geometry::legacy_position_saved")]
    pub position_saved: bool,
    pub rows_min: u16,
    pub rows_max: u16,
    pub columns_min: u16,
    pub columns_max: u16,
    pub display_format: HexDisplayFormat,
    pub endianness: HexEndianness,
    pub minimum_score: u8,
    pub skip_mapped: bool,
    pub scan_every_byte: bool,
    pub start_address: String,
    pub end_address: String,
    pub decimal_places: u8,
    pub zoom_percent: u16,
    pub candidate_pane_width: u16,
    pub coloring: crate::TableColorSettings,
}

impl Default for MapFinderMemory {
    fn default() -> Self {
        Self {
            window_open: false,
            x: 140,
            y: 110,
            width: 1_100,
            height: 720,
            position_saved: false,
            rows_min: 8,
            rows_max: 8,
            columns_min: 16,
            columns_max: 16,
            display_format: HexDisplayFormat::Unsigned16,
            endianness: HexEndianness::Little,
            minimum_score: 55,
            skip_mapped: true,
            scan_every_byte: false,
            start_address: "0".to_string(),
            end_address: String::new(),
            decimal_places: 2,
            zoom_percent: 100,
            candidate_pane_width: 360,
            coloring: crate::TableColorSettings::default(),
        }
    }
}

impl MapFinderMemory {
    pub fn sanitize(&mut self) {
        self.width = self.width.clamp(480, 2_400);
        self.height = self.height.clamp(320, 1_800);
        self.rows_min = self.rows_min.clamp(2, 64);
        self.rows_max = self.rows_max.clamp(2, 64);
        self.columns_min = self.columns_min.clamp(2, 64);
        self.columns_max = self.columns_max.clamp(2, 64);
        if self.rows_min > self.rows_max {
            std::mem::swap(&mut self.rows_min, &mut self.rows_max);
        }
        if self.columns_min > self.columns_max {
            std::mem::swap(&mut self.columns_min, &mut self.columns_max);
        }
        self.minimum_score = self.minimum_score.clamp(0, 100);
        self.decimal_places = self.decimal_places.min(8);
        self.zoom_percent = self.zoom_percent.clamp(50, 200);
        self.candidate_pane_width = self.candidate_pane_width.clamp(220, 900);
        self.start_address = self.start_address.chars().take(18).collect();
        self.end_address = self.end_address.chars().take(18).collect();
        self.coloring.sanitize();
    }
}

pub fn clamp_map_finder_geometry(memory: &mut MapFinderMemory, viewport: egui::Rect) {
    let rect = clamp_rect(
        egui::Rect::from_min_size(
            egui::pos2(memory.x as f32, memory.y as f32),
            egui::vec2(memory.width as f32, memory.height as f32),
        ),
        viewport,
        egui::vec2(480.0, 320.0),
    );
    memory.x = rect.left().round() as i32;
    memory.y = rect.top().round() as i32;
    memory.width = rect.width().round().max(0.0) as u32;
    memory.height = rect.height().round().max(0.0) as u32;
}

#[derive(Default)]
pub struct MapFinderState {
    pub memory: MapFinderMemory,
    pub candidates: Vec<MapCandidate>,
    pub selected_candidate: Option<usize>,
    pub selected_cell: Option<(usize, usize)>,
    pub status: String,
    pub operation_id: Option<u64>,
    pub cancellation: Option<Arc<AtomicBool>>,
    pub progress_percent: Arc<AtomicU8>,
    pub focus_requested: bool,
}

pub fn search_map_candidates(
    bytes: &[u8],
    mapped_ranges: &[ByteRange],
    config: MapSearchConfig,
) -> Vec<MapCandidate> {
    search_map_candidates_with_progress(bytes, mapped_ranges, config, |_| {})
}

pub fn search_map_candidates_with_progress(
    bytes: &[u8],
    mapped_ranges: &[ByteRange],
    config: MapSearchConfig,
    report_progress: impl FnMut(u8),
) -> Vec<MapCandidate> {
    let never_cancel = AtomicBool::new(false);
    search_map_candidates_with_progress_and_cancel(
        bytes,
        mapped_ranges,
        config,
        &never_cancel,
        report_progress,
    )
    .candidates
}

pub fn search_map_candidates_with_progress_and_cancel(
    bytes: &[u8],
    mapped_ranges: &[ByteRange],
    config: MapSearchConfig,
    cancellation: &AtomicBool,
    mut report_progress: impl FnMut(u8),
) -> MapSearchOutcome {
    report_progress(0);
    if cancellation.load(Ordering::Acquire) {
        return MapSearchOutcome {
            candidates: Vec::new(),
            cancelled: true,
        };
    }
    let rows_min = config.rows_min.clamp(2, 64);
    let rows_max = config.rows_max.clamp(rows_min, 64);
    let columns_min = config.columns_min.clamp(2, 64);
    let columns_max = config.columns_max.clamp(columns_min, 64);
    let shape_count = (rows_max - rows_min + 1) * (columns_max - columns_min + 1);
    if shape_count > MAX_DIMENSION_SHAPES {
        report_progress(100);
        return MapSearchOutcome {
            candidates: Vec::new(),
            cancelled: false,
        };
    }
    let start = config.start_offset.min(bytes.len());
    let end = config.end_offset.unwrap_or(bytes.len()).min(bytes.len());
    if end <= start {
        report_progress(100);
        return MapSearchOutcome {
            candidates: Vec::new(),
            cancelled: false,
        };
    }

    let mut ranges = mapped_ranges.to_vec();
    ranges.sort_unstable_by_key(|range| (range.start, range.end));
    let mut merged_ranges: Vec<ByteRange> = Vec::with_capacity(ranges.len());
    for range in ranges {
        if let Some(previous) = merged_ranges.last_mut() {
            if range.start <= previous.end {
                previous.end = previous.end.max(range.end);
                continue;
            }
        }
        merged_ranges.push(range);
    }
    let step = if config.scan_every_byte {
        1
    } else {
        config.display_format.width()
    };
    let first_offset = if config.scan_every_byte {
        start
    } else {
        start.saturating_add((step - start % step) % step)
    };
    let total_work: u128 = (rows_min..=rows_max)
        .flat_map(|rows| (columns_min..=columns_max).map(move |columns| (rows, columns)))
        .filter_map(|(rows, columns)| {
            let byte_length = rows
                .checked_mul(columns)?
                .checked_mul(config.display_format.width())?;
            let last_offset = end.checked_sub(byte_length)?;
            (first_offset <= last_offset).then(|| {
                u128::from(((last_offset - first_offset) / step + 1) as u64)
                    * rows as u128
                    * columns as u128
            })
        })
        .sum();
    if total_work == 0 {
        report_progress(100);
        return MapSearchOutcome {
            candidates: Vec::new(),
            cancelled: false,
        };
    }
    let mut completed_work = 0_u128;
    let mut last_percent = 0_u8;
    let mut ranked = Vec::new();

    let mut cancelled = false;
    'scan: for rows in rows_min..=rows_max {
        for columns in columns_min..=columns_max {
            let Some(element_count) = rows.checked_mul(columns) else {
                continue;
            };
            let Some(byte_length) = element_count.checked_mul(config.display_format.width()) else {
                continue;
            };
            if end.saturating_sub(start) < byte_length {
                continue;
            }
            let last_offset = end - byte_length;
            let offset_count = (last_offset - first_offset) / step + 1;
            let mut range_index = 0;
            let mut values = Vec::with_capacity(element_count);
            let mut best = BTreeMap::<(u8, Reverse<usize>), MapCandidate>::new();

            for (offset_index, offset) in (first_offset..=last_offset).step_by(step).enumerate() {
                if (offset_index % CANCEL_CHECK_INTERVAL == 0 || offset_index + 1 == offset_count)
                    && cancellation.load(Ordering::Acquire)
                {
                    cancelled = true;
                    break 'scan;
                }
                while range_index < merged_ranges.len() && merged_ranges[range_index].end <= offset
                {
                    range_index += 1;
                }
                if merged_ranges
                    .get(range_index)
                    .is_some_and(|range| range.start < offset + byte_length)
                {
                    completed_work += element_count as u128;
                    let percent = (completed_work * 90 / total_work).min(90) as u8;
                    emit_progress(&mut report_progress, &mut last_percent, percent);
                    continue;
                }

                values.clear();
                let mut valid = true;
                for index in 0..element_count {
                    let Some(value) = decode_hex_value(
                        bytes,
                        offset + index * config.display_format.width(),
                        config.display_format,
                        config.endianness,
                    )
                    .map(|value| value.as_f64())
                    .filter(|value| value.is_finite()) else {
                        valid = false;
                        break;
                    };
                    values.push(value);
                }
                completed_work += element_count as u128;
                let percent = (completed_work * 90 / total_work).min(90) as u8;
                emit_progress(&mut report_progress, &mut last_percent, percent);
                if !valid {
                    continue;
                }
                let Some((score, value_range, value_bands)) = score_grid(&values, rows, columns)
                else {
                    continue;
                };
                if score < config.minimum_score {
                    continue;
                }

                let candidate = MapCandidate {
                    offset,
                    byte_length,
                    rows,
                    columns,
                    display_format: config.display_format,
                    endianness: config.endianness,
                    score,
                    value_range,
                    value_bands,
                    axis_suggestions: Vec::new(),
                    selected_x_axis: None,
                    selected_y_axis: None,
                };
                best.insert((score, Reverse(offset)), candidate);
                if best.len() > MAX_CANDIDATES_PER_SHAPE {
                    best.pop_first();
                }
            }
            ranked.extend(best.into_values());
        }
    }

    if cancelled || cancellation.load(Ordering::Acquire) {
        return MapSearchOutcome {
            candidates: Vec::new(),
            cancelled: true,
        };
    }

    emit_progress(&mut report_progress, &mut last_percent, 92);
    ranked.sort_unstable_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.offset.cmp(&right.offset))
            .then_with(|| (right.rows * right.columns).cmp(&(left.rows * left.columns)))
    });
    let mut distinct = Vec::with_capacity(ranked.len());
    for candidate in ranked {
        let candidate_end = candidate.offset.saturating_add(candidate.byte_length);
        let duplicate = distinct.iter().any(|other: &MapCandidate| {
            if candidate.rows != other.rows || candidate.columns != other.columns {
                return false;
            }
            let other_end = other.offset.saturating_add(other.byte_length);
            let overlap = candidate_end
                .min(other_end)
                .saturating_sub(candidate.offset.max(other.offset));
            overlap.saturating_mul(4)
                >= candidate
                    .byte_length
                    .min(other.byte_length)
                    .saturating_mul(3)
        });
        if duplicate {
            continue;
        }
        distinct.push(candidate);
        if distinct.len() == MAX_CANDIDATES {
            break;
        }
    }
    emit_progress(&mut report_progress, &mut last_percent, 95);
    let candidate_count = distinct.len().max(1);
    for (index, candidate) in distinct.iter_mut().enumerate() {
        if cancellation.load(Ordering::Acquire) {
            return MapSearchOutcome {
                candidates: Vec::new(),
                cancelled: true,
            };
        }
        candidate.axis_suggestions = find_nearby_axes(bytes, candidate, &merged_ranges);
        let percent = 95 + ((index + 1) * 4 / candidate_count) as u8;
        emit_progress(&mut report_progress, &mut last_percent, percent.min(99));
    }
    emit_progress(&mut report_progress, &mut last_percent, 100);
    MapSearchOutcome {
        candidates: distinct,
        cancelled: false,
    }
}

fn emit_progress(report: &mut impl FnMut(u8), last_percent: &mut u8, percent: u8) {
    if percent > *last_percent {
        *last_percent = percent;
        report(percent);
    }
}

fn monotonic_axis_score(values: &[f64]) -> Option<u8> {
    if values.len() < 3 || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let increasing = values.windows(2).all(|pair| pair[0] < pair[1]);
    let decreasing = values.windows(2).all(|pair| pair[0] > pair[1]);
    (increasing || decreasing).then_some(100)
}

fn find_nearby_axes(
    bytes: &[u8],
    candidate: &MapCandidate,
    mapped_ranges: &[ByteRange],
) -> Vec<MapAxisSuggestion> {
    let table_end = candidate.offset.saturating_add(candidate.byte_length);
    let before_start = candidate.offset.saturating_sub(MAX_NEARBY_AXIS_BYTES);
    let after_end = table_end
        .saturating_add(MAX_NEARBY_AXIS_BYTES)
        .min(bytes.len());
    let mut lengths = vec![candidate.columns];
    if candidate.rows != candidate.columns {
        lengths.push(candidate.rows);
    }
    let formats = [
        HexDisplayFormat::Unsigned8,
        HexDisplayFormat::Signed8,
        HexDisplayFormat::Unsigned16,
        HexDisplayFormat::Signed16,
        HexDisplayFormat::Unsigned32,
        HexDisplayFormat::Signed32,
        HexDisplayFormat::Float32,
        HexDisplayFormat::Float64,
    ];
    let mut suggestions = Vec::new();

    for length in lengths {
        let mut best: Option<(usize, MapAxisSuggestion)> = None;
        for format in formats {
            let width = format.width();
            let Some(axis_bytes) = length.checked_mul(width) else {
                continue;
            };
            for endianness in [HexEndianness::Little, HexEndianness::Big] {
                if width == 1 && endianness == HexEndianness::Big {
                    continue;
                }
                for (region_start, region_end) in
                    [(before_start, candidate.offset), (table_end, after_end)]
                {
                    if region_end.saturating_sub(region_start) < axis_bytes {
                        continue;
                    }
                    let last_offset = region_end - axis_bytes;
                    let offsets: Box<dyn Iterator<Item = usize>> = if region_end == candidate.offset
                    {
                        Box::new((region_start..=last_offset).rev())
                    } else {
                        Box::new(region_start..=last_offset)
                    };
                    for offset in offsets {
                        let axis_end = offset + axis_bytes;
                        let range_index =
                            mapped_ranges.partition_point(|range| range.end <= offset);
                        if mapped_ranges
                            .get(range_index)
                            .is_some_and(|range| range.start < axis_end)
                        {
                            continue;
                        }
                        let values: Option<Vec<_>> = (0..length)
                            .map(|index| {
                                decode_hex_value(bytes, offset + index * width, format, endianness)
                                    .map(|value| value.as_f64())
                                    .filter(|value| value.is_finite())
                            })
                            .collect();
                        let Some(values) = values else {
                            continue;
                        };
                        let Some(score) = monotonic_axis_score(&values) else {
                            continue;
                        };
                        let distance = if axis_end <= candidate.offset {
                            candidate.offset - axis_end
                        } else {
                            offset.saturating_sub(table_end)
                        };
                        let suggestion = MapAxisSuggestion {
                            offset,
                            increasing: values[0] < *values.last().unwrap_or(&values[0]),
                            values,
                            score,
                            matches_columns: length == candidate.columns,
                            matches_rows: length == candidate.rows,
                            display_format: format,
                            endianness,
                        };
                        if best
                            .as_ref()
                            .is_none_or(|(best_distance, _)| distance < *best_distance)
                        {
                            best = Some((distance, suggestion));
                        }
                    }
                }
            }
        }
        if let Some((_, suggestion)) = best {
            suggestions.push(suggestion);
        }
    }
    suggestions
}

fn score_grid(values: &[f64], rows: usize, columns: usize) -> Option<(u8, (f64, f64), u8)> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for value in values {
        minimum = minimum.min(*value);
        maximum = maximum.max(*value);
    }
    let span = maximum - minimum;
    if !span.is_finite() || span <= f64::EPSILON * maximum.abs().max(1.0) {
        return None;
    }

    let mut bands = [false; 16];
    for value in values {
        let band = (((*value - minimum) / span) * 15.0).round() as usize;
        bands[band.min(15)] = true;
    }
    let value_bands = bands.iter().filter(|band| **band).count() as u8;
    if value_bands < 4 {
        return None;
    }

    let mut delta_total = 0.0;
    let mut delta_count = 0usize;
    let mut horizontal_up = 0usize;
    let mut horizontal_down = 0usize;
    let mut vertical_up = 0usize;
    let mut vertical_down = 0usize;
    for row in 0..rows {
        for column in 0..columns {
            let index = row * columns + column;
            if column + 1 < columns {
                let delta = values[index + 1] - values[index];
                delta_total += delta.abs() / span;
                delta_count += 1;
                horizontal_up += usize::from(delta > 0.0);
                horizontal_down += usize::from(delta < 0.0);
            }
            if row + 1 < rows {
                let delta = values[index + columns] - values[index];
                delta_total += delta.abs() / span;
                delta_count += 1;
                vertical_up += usize::from(delta > 0.0);
                vertical_down += usize::from(delta < 0.0);
            }
        }
    }
    if delta_count == 0 {
        return None;
    }

    let average_step = delta_total / delta_count as f64;
    let smoothness = (1.0 - average_step * 2.0).clamp(0.0, 1.0);
    let horizontal_count = rows * columns.saturating_sub(1);
    let vertical_count = rows.saturating_sub(1) * columns;
    let horizontal_direction = if horizontal_count > 0 {
        horizontal_up.abs_diff(horizontal_down) as f64 / horizontal_count as f64
    } else {
        0.0
    };
    let vertical_direction = if vertical_count > 0 {
        vertical_up.abs_diff(vertical_down) as f64 / vertical_count as f64
    } else {
        0.0
    };
    let direction_consistency = (horizontal_direction + vertical_direction) * 0.5;
    let diversity = f64::from(value_bands) / 16.0;
    let score = (diversity * 20.0 + smoothness * 60.0 + direction_consistency * 20.0)
        .round()
        .clamp(0.0, 100.0) as u8;
    Some((score, (minimum, maximum), value_bands))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range_config(rows_max: usize, columns_max: usize, end_offset: usize) -> MapSearchConfig {
        MapSearchConfig {
            rows_min: 4,
            rows_max,
            columns_min: 4,
            columns_max,
            display_format: HexDisplayFormat::Unsigned8,
            endianness: HexEndianness::Little,
            minimum_score: 80,
            scan_every_byte: true,
            start_offset: 0,
            end_offset: Some(end_offset),
        }
    }

    #[test]
    fn dimension_range_finds_each_included_edge_shape_and_never_exceeds_bounds() {
        let mut four_by_four = vec![0_u8; 16];
        four_by_four.copy_from_slice(&(0_u8..16).collect::<Vec<_>>());
        let candidates =
            search_map_candidates(&four_by_four, &[], range_config(12, 12, four_by_four.len()));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.rows == 4 && candidate.columns == 4));
        assert!(candidates.iter().all(|candidate| {
            (4..=12).contains(&candidate.rows) && (4..=12).contains(&candidate.columns)
        }));

        let twelve_by_twelve = (0_u8..144).collect::<Vec<_>>();
        let candidates = search_map_candidates(
            &twelve_by_twelve,
            &[],
            range_config(12, 12, twelve_by_twelve.len()),
        );
        assert!(candidates
            .iter()
            .any(|candidate| candidate.rows == 12 && candidate.columns == 12));
    }

    #[test]
    fn configured_address_range_excludes_candidates_outside_it() {
        let mut bytes = vec![0_u8; 64];
        bytes[32..48].copy_from_slice(&(0_u8..16).collect::<Vec<_>>());
        let mut config = range_config(4, 4, 16);
        assert!(search_map_candidates(&bytes, &[], config).is_empty());

        config.end_offset = None;
        assert!(search_map_candidates(&bytes, &[], config)
            .iter()
            .any(|candidate| candidate.offset == 32));
    }

    #[test]
    fn scan_progress_is_monotonic_bounded_and_reaches_completion() {
        let bytes = (0_u8..64).collect::<Vec<_>>();
        let mut config = range_config(4, 4, bytes.len());
        config.scan_every_byte = true;
        let mut progress = Vec::new();

        search_map_candidates_with_progress(&bytes, &[], config, |percent| {
            progress.push(percent);
        });

        assert_eq!(progress.first(), Some(&0));
        assert_eq!(progress.last(), Some(&100));
        assert!(progress.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(progress.iter().any(|percent| (1..100).contains(percent)));
        assert!(progress.len() <= 102);
    }

    #[test]
    fn cancelled_map_scan_discards_partial_candidates() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let bytes = (0_u8..128).collect::<Vec<_>>();
        let config = MapSearchConfig {
            rows_min: 4,
            rows_max: 4,
            columns_min: 4,
            columns_max: 4,
            display_format: HexDisplayFormat::Unsigned8,
            endianness: HexEndianness::Little,
            minimum_score: 0,
            scan_every_byte: true,
            start_offset: 0,
            end_offset: Some(bytes.len()),
        };
        assert!(!search_map_candidates(&bytes, &[], config).is_empty());

        let cancellation = AtomicBool::new(false);
        let mut cancellation_requested = false;
        let outcome = search_map_candidates_with_progress_and_cancel(
            &bytes,
            &[],
            config,
            &cancellation,
            |percent| {
                if percent >= 5 {
                    cancellation_requested = true;
                    cancellation.store(true, Ordering::Release);
                }
            },
        );

        assert!(cancellation_requested);
        assert!(outcome.cancelled);
        assert!(outcome.candidates.is_empty());
    }

    #[test]
    fn extreme_mixed_prefix_does_not_score_like_a_coherent_map() {
        const ROWS: usize = 16;
        const COLUMNS: usize = 14;
        let extreme_prefix = [
            -2_410_057.0,
            -2_101_500.0,
            -1_449_067.0,
            -1_100_232.0,
            -696_826.69,
            -4_229_695.0,
            -4_229_695.0,
            -4_829_405.0,
            -4_989_237.0,
            -3_687_240.0,
            -3_050_283.0,
            -2_893_762.0,
            -2_981_151.0,
            -2_255_316.0,
            -1_961_827.0,
            -1_079_682.0,
            -613_123.69,
            357_428.0,
            997_279.19,
            -3_351_887.0,
            -3_351_887.0,
            -3_290_529.0,
            -2_752_419.0,
            -2_118_080.0,
            -1_652_414.0,
            -1_980_264.0,
            -2_935_692.0,
            -1_705_524.0,
            -1_802_490.0,
            -838_747.62,
            -482_208.5,
            1_150_068.0,
            1_727_189.0,
        ];
        let mut mixed = vec![0.0; ROWS * COLUMNS];
        mixed[..extreme_prefix.len()].copy_from_slice(&extreme_prefix);
        for row in 2..ROWS {
            for column in 0..COLUMNS {
                let index = row * COLUMNS + column;
                if index >= extreme_prefix.len() {
                    mixed[index] = if (row + column) % 2 == 0 { 5.0 } else { 8.0 };
                }
            }
        }

        let noisy_score = score_grid(&mixed, ROWS, COLUMNS).unwrap().0;
        let coherent = (0..100)
            .map(|index| 267.0 + index as f64 * 61.0)
            .collect::<Vec<_>>();
        let coherent_score = score_grid(&coherent, 10, 10).unwrap().0;

        assert!(noisy_score < 82, "mixed outliers scored {noisy_score}/100");
        assert!(
            coherent_score >= 82,
            "coherent map scored {coherent_score}/100"
        );
    }

    #[test]
    fn axis_suggestions_accept_strictly_monotonic_sequences_and_reject_jagged_data() {
        assert!(monotonic_axis_score(&[0.0, 10.0, 20.0, 30.0]).is_some());
        assert!(monotonic_axis_score(&[30.0, 20.0, 10.0, 0.0]).is_some());
        assert!(monotonic_axis_score(&[0.0, 10.0, 7.0, 30.0]).is_none());
        assert!(monotonic_axis_score(&[0.0, 10.0, 10.0, 30.0]).is_none());
    }

    #[test]
    fn axis_assignment_requires_compatible_suggestion_and_can_be_cleared() {
        let mut candidate = MapCandidate {
            offset: 16,
            byte_length: 32,
            rows: 4,
            columns: 4,
            display_format: HexDisplayFormat::Unsigned16,
            endianness: HexEndianness::Little,
            score: 90,
            value_range: (0.0, 1.0),
            value_bands: 2,
            axis_suggestions: vec![
                MapAxisSuggestion {
                    offset: 0,
                    values: vec![100.0, 200.0, 300.0, 400.0],
                    score: 100,
                    increasing: true,
                    matches_columns: true,
                    matches_rows: false,
                    display_format: HexDisplayFormat::Unsigned16,
                    endianness: HexEndianness::Little,
                },
                MapAxisSuggestion {
                    offset: 8,
                    values: vec![10.0, 20.0, 30.0, 40.0],
                    score: 100,
                    increasing: true,
                    matches_columns: false,
                    matches_rows: true,
                    display_format: HexDisplayFormat::Unsigned16,
                    endianness: HexEndianness::Little,
                },
            ],
            selected_x_axis: None,
            selected_y_axis: None,
        };

        assert!(candidate
            .set_axis_assignment(MapAxisRole::X, Some(0))
            .is_ok());
        assert_eq!(candidate.selected_x_axis, Some(0));
        assert!(candidate
            .set_axis_assignment(MapAxisRole::X, Some(1))
            .is_err());
        assert_eq!(candidate.selected_x_axis, Some(0));
        assert!(candidate
            .set_axis_assignment(MapAxisRole::Y, Some(1))
            .is_ok());
        assert_eq!(candidate.selected_y_axis, Some(1));
        assert!(candidate.set_axis_assignment(MapAxisRole::Y, None).is_ok());
        assert_eq!(candidate.selected_y_axis, None);
    }

    #[test]
    fn nearby_axis_suggestions_preserve_raw_values_and_ignore_xdf_mapped_bytes() {
        let mut bytes = vec![0_u8; 40];
        for (index, value) in [100_u16, 200, 300, 400].into_iter().enumerate() {
            bytes[index * 2..index * 2 + 2].copy_from_slice(&value.to_le_bytes());
        }
        for pair in bytes[8..40].as_chunks_mut::<2>().0 {
            pair.copy_from_slice(&1000_u16.to_le_bytes());
        }
        let candidate = MapCandidate {
            offset: 8,
            byte_length: 32,
            rows: 4,
            columns: 4,
            display_format: HexDisplayFormat::Unsigned16,
            endianness: HexEndianness::Little,
            score: 90,
            value_range: (1000.0, 1000.0),
            value_bands: 1,
            axis_suggestions: Vec::new(),
            selected_x_axis: None,
            selected_y_axis: None,
        };

        let suggestions = find_nearby_axes(&bytes, &candidate, &[]);
        assert!(suggestions.iter().any(|axis| {
            axis.offset == 0
                && axis.matches_columns
                && axis.matches_rows
                && axis.values == [100.0, 200.0, 300.0, 400.0]
        }));
        assert!(find_nearby_axes(&bytes, &candidate, &[ByteRange { start: 0, end: 8 }]).is_empty());
    }

    #[test]
    fn saved_window_geometry_is_resized_and_clamped_to_the_current_viewport() {
        let mut memory = MapFinderMemory {
            x: 980,
            y: 740,
            width: 1_100,
            height: 720,
            ..MapFinderMemory::default()
        };
        let viewport = eframe::egui::Rect::from_min_size(
            eframe::egui::pos2(100.0, 80.0),
            eframe::egui::vec2(900.0, 560.0),
        );

        clamp_map_finder_geometry(&mut memory, viewport);

        assert_eq!(memory.width, 900);
        assert_eq!(memory.height, 560);
        assert_eq!(memory.x, 100);
        assert_eq!(memory.y, 80);
    }
}
