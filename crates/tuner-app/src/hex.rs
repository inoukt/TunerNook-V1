use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::TableColorSettings;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HexDisplayFormat {
    #[default]
    Bytes,
    Unsigned8,
    Signed8,
    Unsigned16,
    Signed16,
    Unsigned32,
    Signed32,
    Unsigned64,
    Signed64,
    Float32,
    Float64,
}

impl HexDisplayFormat {
    pub fn width(self) -> usize {
        match self {
            Self::Bytes | Self::Unsigned8 | Self::Signed8 => 1,
            Self::Unsigned16 | Self::Signed16 => 2,
            Self::Unsigned32 | Self::Signed32 | Self::Float32 => 4,
            Self::Unsigned64 | Self::Signed64 | Self::Float64 => 8,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Bytes => "Hex bytes",
            Self::Unsigned8 => "u8",
            Self::Signed8 => "i8",
            Self::Unsigned16 => "u16",
            Self::Signed16 => "i16",
            Self::Unsigned32 => "u32",
            Self::Signed32 => "i32",
            Self::Unsigned64 => "u64",
            Self::Signed64 => "i64",
            Self::Float32 => "float32",
            Self::Float64 => "float64",
        }
    }

    pub fn command_name(self) -> &'static str {
        match self {
            Self::Bytes => "bytes",
            Self::Unsigned8 => "u8",
            Self::Signed8 => "i8",
            Self::Unsigned16 => "u16",
            Self::Signed16 => "i16",
            Self::Unsigned32 => "u32",
            Self::Signed32 => "i32",
            Self::Unsigned64 => "u64",
            Self::Signed64 => "i64",
            Self::Float32 => "float32",
            Self::Float64 => "float64",
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, Self::Float32 | Self::Float64)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HexEndianness {
    #[default]
    Little,
    Big,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HexAutoRangeScope {
    #[default]
    VisiblePercentile,
    WholeBin,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HexValue {
    Unsigned(u64),
    Signed(i64),
    Float(f64),
}

impl HexValue {
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Unsigned(value) => value as f64,
            Self::Signed(value) => value as f64,
            Self::Float(value) => value,
        }
    }

    pub fn format(self, decimal_places: u8) -> String {
        match self {
            Self::Unsigned(value) => value.to_string(),
            Self::Signed(value) => value.to_string(),
            Self::Float(value) => format!("{:.*}", usize::from(decimal_places.min(8)), value),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HexSelection {
    pub anchor: usize,
    pub focus: usize,
}

impl HexSelection {
    pub fn new(anchor: usize, focus: usize) -> Self {
        Self { anchor, focus }
    }

    /// Return the selected bytes as a half-open range.
    pub fn bounds(self) -> (usize, usize) {
        (
            self.anchor.min(self.focus),
            self.anchor.max(self.focus).saturating_add(1),
        )
    }

    pub fn contains(self, offset: usize) -> bool {
        let (start, end) = self.bounds();
        (start..end).contains(&offset)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct HexWindowMemory {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default = "crate::window_geometry::legacy_position_saved")]
    pub position_saved: bool,
    pub display_format: HexDisplayFormat,
    pub endianness: HexEndianness,
    pub auto_range_scope: HexAutoRangeScope,
    pub decimal_places: u8,
    pub coloring: TableColorSettings,
    pub search_query: String,
}

impl Default for HexWindowMemory {
    fn default() -> Self {
        Self {
            x: 120,
            y: 90,
            width: 980,
            height: 620,
            position_saved: false,
            display_format: HexDisplayFormat::default(),
            endianness: HexEndianness::default(),
            auto_range_scope: HexAutoRangeScope::default(),
            decimal_places: 2,
            coloring: TableColorSettings::default(),
            search_query: String::new(),
        }
    }
}

impl HexWindowMemory {
    pub fn sanitize(&mut self) {
        self.width = self.width.clamp(520, 2_400);
        self.height = self.height.clamp(320, 1_800);
        self.decimal_places = self.decimal_places.min(8);
        self.search_query = self.search_query.chars().take(512).collect();
        self.coloring.sanitize();
    }
}

pub(super) fn hex_scroll_offset_for_address(
    byte_offset: usize,
    row_height_sans_spacing: f32,
    vertical_spacing: f32,
) -> f32 {
    (byte_offset / 16) as f32 * (row_height_sans_spacing + vertical_spacing)
}

pub fn decode_hex_value(
    bytes: &[u8],
    offset: usize,
    format: HexDisplayFormat,
    endianness: HexEndianness,
) -> Option<HexValue> {
    let width = format.width();
    let payload = bytes.get(offset..offset.checked_add(width)?)?;
    let mut raw = 0u64;
    for (index, byte) in payload.iter().copied().enumerate() {
        raw = match endianness {
            HexEndianness::Little => raw | (u64::from(byte) << (index * 8)),
            HexEndianness::Big => (raw << 8) | u64::from(byte),
        };
    }
    Some(match format {
        HexDisplayFormat::Bytes
        | HexDisplayFormat::Unsigned8
        | HexDisplayFormat::Unsigned16
        | HexDisplayFormat::Unsigned32
        | HexDisplayFormat::Unsigned64 => HexValue::Unsigned(raw),
        HexDisplayFormat::Signed8
        | HexDisplayFormat::Signed16
        | HexDisplayFormat::Signed32
        | HexDisplayFormat::Signed64 => {
            let bits = width * 8;
            let signed = if bits == 64 {
                raw as i64
            } else if raw & (1u64 << (bits - 1)) != 0 {
                (raw | (!0u64 << bits)) as i64
            } else {
                raw as i64
            };
            HexValue::Signed(signed)
        }
        HexDisplayFormat::Float32 => HexValue::Float(f32::from_bits(raw as u32) as f64),
        HexDisplayFormat::Float64 => HexValue::Float(f64::from_bits(raw)),
    })
}

pub fn hex_value_range(
    bytes: &[u8],
    format: HexDisplayFormat,
    endianness: HexEndianness,
) -> Option<(f64, f64)> {
    let width = format.width();
    let count = bytes.len() / width;
    let mut range: Option<(f64, f64)> = None;
    for index in 0..count {
        let Some(value) = decode_hex_value(bytes, index * width, format, endianness)
            .map(HexValue::as_f64)
            .filter(|value| value.is_finite())
        else {
            continue;
        };
        range = Some(match range {
            Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
            None => (value, value),
        });
    }
    range
}

pub fn hex_visible_value_range(
    bytes: &[u8],
    format: HexDisplayFormat,
    endianness: HexEndianness,
    visible_rows: Range<usize>,
) -> Option<(f64, f64)> {
    let width = format.width();
    let values = visible_rows
        .flat_map(|row| {
            let row_start = row.saturating_mul(16);
            (0..16 / width).filter_map(move |column| {
                decode_hex_value(bytes, row_start + column * width, format, endianness)
                    .map(HexValue::as_f64)
            })
        })
        .filter(|value| value.is_finite())
        .collect();
    robust_value_range(values)
}

fn robust_value_range(mut values: Vec<f64>) -> Option<(f64, f64)> {
    values.retain(|value| value.is_finite());
    values.sort_by(f64::total_cmp);
    let first = *values.first()?;
    let last = *values.last()?;
    if values.len() < 8 {
        return Some((first, last));
    }

    let percentile = |fraction: f64| {
        let rank = fraction * (values.len() - 1) as f64;
        let lower = rank.floor() as usize;
        let upper = rank.ceil() as usize;
        let weight = rank - lower as f64;
        values[lower] * (1.0 - weight) + values[upper] * weight
    };
    Some((percentile(0.02), percentile(0.98)))
}

pub fn find_hex_match(
    bytes: &[u8],
    query: &str,
    format: HexDisplayFormat,
    endianness: HexEndianness,
    from_offset: usize,
    forward: bool,
) -> Result<Option<usize>, String> {
    if format == HexDisplayFormat::Bytes {
        let needle = parse_hex_bytes(query)?;
        if needle.len() > bytes.len() {
            return Ok(None);
        }
        let last_start = bytes.len() - needle.len();
        let start_count = last_start + 1;
        let start = from_offset % start_count;
        if forward {
            return Ok((start..start_count)
                .chain(0..start)
                .find(|offset| bytes[*offset..*offset + needle.len()] == needle));
        }
        return Ok((0..=start)
            .rev()
            .chain((start.saturating_add(1)..start_count).rev())
            .find(|offset| bytes[*offset..*offset + needle.len()] == needle));
    }

    let needle = parse_typed_query(query, format)?;
    let width = format.width();
    let count = bytes.len() / width;
    if count == 0 {
        return Ok(None);
    }
    let start = (from_offset / width) % count;
    for distance in 0..count {
        let index = if forward {
            (start + distance) % count
        } else {
            (start + count - distance) % count
        };
        let offset = index * width;
        if decode_hex_value(bytes, offset, format, endianness) == Some(needle) {
            return Ok(Some(offset));
        }
    }
    Ok(None)
}

fn parse_typed_query(query: &str, format: HexDisplayFormat) -> Result<HexValue, String> {
    let text = query.trim();
    match format {
        HexDisplayFormat::Bytes => parse_hex_bytes(text)
            .map(|bytes| HexValue::Unsigned(bytes.first().copied().unwrap_or_default().into())),
        HexDisplayFormat::Unsigned8
        | HexDisplayFormat::Unsigned16
        | HexDisplayFormat::Unsigned32
        | HexDisplayFormat::Unsigned64 => {
            let value = parse_unsigned_query(text)?;
            let max = if format.width() == 8 {
                u64::MAX
            } else {
                (1u64 << (format.width() * 8)) - 1
            };
            if value > max {
                return Err(format!("Value does not fit in {}.", format.label()));
            }
            Ok(HexValue::Unsigned(value))
        }
        HexDisplayFormat::Signed8
        | HexDisplayFormat::Signed16
        | HexDisplayFormat::Signed32
        | HexDisplayFormat::Signed64 => {
            let value = text
                .parse::<i64>()
                .map_err(|_| format!("Enter a decimal {} value.", format.label()))?;
            let bits = format.width() * 8;
            if bits < 64 {
                let min = -(1i64 << (bits - 1));
                let max = (1i64 << (bits - 1)) - 1;
                if !(min..=max).contains(&value) {
                    return Err(format!("Value does not fit in {}.", format.label()));
                }
            }
            Ok(HexValue::Signed(value))
        }
        HexDisplayFormat::Float32 => {
            let value = text
                .parse::<f64>()
                .map_err(|_| "Enter a decimal float32 value.".to_string())?
                as f32;
            if value.is_finite() {
                Ok(HexValue::Float(value as f64))
            } else {
                Err("Float search values must be finite.".to_string())
            }
        }
        HexDisplayFormat::Float64 => {
            let value = text
                .parse::<f64>()
                .map_err(|_| "Enter a decimal float64 value.".to_string())?;
            if value.is_finite() {
                Ok(HexValue::Float(value))
            } else {
                Err("Float search values must be finite.".to_string())
            }
        }
    }
}

fn parse_unsigned_query(text: &str) -> Result<u64, String> {
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).map_err(|_| "Enter a decimal or hexadecimal integer.".into())
    } else {
        text.parse::<u64>()
            .map_err(|_| "Enter a decimal or hexadecimal integer.".into())
    }
}

pub fn parse_hex_bytes(text: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    for token in text
        .split(|character: char| character.is_ascii_whitespace() || matches!(character, ',' | ';'))
    {
        if token.is_empty() {
            continue;
        }
        let token = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
            .unwrap_or(token);
        if token.is_empty() || token.len() % 2 != 0 {
            return Err("Hex input must contain complete pairs of digits.".to_string());
        }
        for pair in token.as_bytes().as_chunks::<2>().0 {
            let pair = std::str::from_utf8(pair)
                .map_err(|_| "Hex input contains an invalid byte.".to_string())?;
            let value = u8::from_str_radix(pair, 16)
                .map_err(|_| format!("'{pair}' is not a hexadecimal byte."))?;
            bytes.push(value);
        }
    }
    if bytes.is_empty() {
        return Err("Enter at least one hexadecimal byte.".to_string());
    }
    Ok(bytes)
}

pub fn format_hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn ascii_byte(byte: u8) -> char {
    if byte.is_ascii_graphic() || byte == b' ' {
        char::from(byte)
    } else {
        '.'
    }
}

#[cfg(test)]
mod tests {
    use super::{
        hex_scroll_offset_for_address, hex_visible_value_range, HexAutoRangeScope,
        HexDisplayFormat, HexEndianness, HexWindowMemory,
    };

    #[test]
    fn hex_scroll_offset_uses_virtual_row_pitch_including_spacing() {
        assert_eq!(hex_scroll_offset_for_address(0x20, 22.0, 3.0), 50.0);
    }

    #[test]
    fn visible_hex_range_ignores_hidden_rows_and_trims_finite_float_outliers() {
        let values = std::iter::once(-1.0e30_f32)
            .chain((0..98).map(|value| value as f32))
            .chain(std::iter::once(1.0e30_f32));
        let mut bytes = Vec::new();
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }

        let visible = hex_visible_value_range(
            &bytes,
            HexDisplayFormat::Float32,
            HexEndianness::Little,
            0..25,
        )
        .unwrap();
        assert!((visible.0 - 0.98).abs() < 1e-5);
        assert!((visible.1 - 96.02).abs() < 1e-5);

        let middle_only = hex_visible_value_range(
            &bytes,
            HexDisplayFormat::Float32,
            HexEndianness::Little,
            1..24,
        )
        .unwrap();
        assert!((middle_only.0 - 4.82).abs() < 1e-5, "{middle_only:?}");
        assert!((middle_only.1 - 92.18).abs() < 1e-5, "{middle_only:?}");
    }

    #[test]
    fn legacy_hex_window_settings_default_to_visible_auto_range() {
        let memory: HexWindowMemory = serde_json::from_str("{}").unwrap();
        assert_eq!(
            memory.auto_range_scope,
            HexAutoRangeScope::VisiblePercentile
        );
    }

    #[test]
    fn hex_window_settings_persist_whole_bin_auto_range_scope() {
        let memory = HexWindowMemory {
            auto_range_scope: HexAutoRangeScope::WholeBin,
            ..HexWindowMemory::default()
        };
        let saved = serde_json::to_string(&memory).unwrap();
        let restored: HexWindowMemory = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.auto_range_scope, HexAutoRangeScope::WholeBin);
    }
}
