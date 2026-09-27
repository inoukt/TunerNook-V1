//! Tolerant, dependency-free XDF definitions for TunerNook.
//!
//! This crate intentionally separates three concerns:
//!
//! * exact file identity (`exact_sha256`), used by the V1 transfer gate;
//! * normalized definition identity (`normalized_fingerprint`), useful for diagnostics;
//! * safe resolution of every mapped range against a BIN before editing.
//!
//! Unknown XML elements are retained for round-trip export, while known object definitions
//! are normalized into a small model shared by the desktop app, CLI, API, and transfer planner.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use tuner_cal::{Conversion, ConversionError};
use tuner_core::{sha256_hex, BinDocument, ByteRange, CoreError, Endianness, Transaction};

/// Stable version marker for the normalized XDF model.
pub const XDF_MODEL_VERSION: &str = "tuner-xdf/v1";

/// Errors produced while loading or normalizing an XDF file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XdfError {
    Io {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
    Xml {
        position: usize,
        message: String,
    },
    InvalidField {
        object: String,
        field: String,
        value: String,
        message: String,
    },
    Overflow {
        object: String,
        message: String,
    },
    DuplicateIdentity {
        identity: String,
    },
    InvalidCell {
        semantic_id: String,
        row: usize,
        column: usize,
        rows: usize,
        columns: usize,
    },
    InvalidAxis {
        semantic_id: String,
        axis_index: usize,
        index: Option<usize>,
        count: usize,
    },
    Export {
        message: String,
    },
}

impl fmt::Display for XdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                message,
            } => write!(
                f,
                "I/O operation '{operation}' failed for '{}': {message}",
                path.display()
            ),
            Self::Xml { position, message } => {
                write!(f, "XDF XML error at character {position}: {message}")
            }
            Self::InvalidField {
                object,
                field,
                value,
                message,
            } => write!(f, "invalid XDF field {object}.{field}={value:?}: {message}"),
            Self::Overflow { object, message } => {
                write!(f, "XDF range overflow for {object}: {message}")
            }
            Self::DuplicateIdentity { identity } => {
                write!(f, "duplicate XDF semantic parameter identity '{identity}'")
            }
            Self::InvalidCell {
                semantic_id,
                row,
                column,
                rows,
                columns,
            } => write!(
                f,
                "XDF cell {semantic_id} at row {row}, column {column} is outside {rows}x{columns} dimensions"
            ),
            Self::InvalidAxis {
                semantic_id,
                axis_index,
                index,
                count,
            } => match index {
                Some(index) => write!(
                    f,
                    "XDF axis {semantic_id} at index {axis_index}, element {index} is outside valid count {count}"
                ),
                None => write!(
                    f,
                    "XDF axis index {axis_index} for {semantic_id} is outside valid count {count}"
                ),
            },
            Self::Export { message } => write!(f, "could not export XDF: {message}"),
        }
    }
}

impl std::error::Error for XdfError {}

fn io_error(operation: &'static str, path: &Path, error: std::io::Error) -> XdfError {
    XdfError::Io {
        operation,
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

/// Header-level defaults that apply when an object does not provide a value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XdfDefaults {
    pub data_size_bits: Option<u32>,
    pub significant_digits: Option<u32>,
    pub output_type: Option<u32>,
    pub signed: Option<bool>,
    pub lsb_first: Option<bool>,
    pub float: Option<bool>,
}

/// Address translation from an XDF address to a BIN offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct XdfBaseOffset {
    pub offset: u64,
    pub subtract: bool,
}

impl XdfBaseOffset {
    pub fn translate(&self, xdf_address: u64) -> Result<usize, XdfError> {
        let signed_offset = if self.subtract {
            -(self.offset as i128)
        } else {
            self.offset as i128
        };
        let translated = xdf_address as i128 + signed_offset;
        if translated < 0 || translated > usize::MAX as i128 {
            return Err(XdfError::Overflow {
                object: "XDFHEADER.BASEOFFSET".to_string(),
                message: format!(
                    "translated address {xdf_address:#x} with offset {:#x} is outside BIN address space",
                    self.offset
                ),
            });
        }
        Ok(translated as usize)
    }
}

/// A declared address region from the XDF header.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XdfRegion {
    pub type_code: Option<u64>,
    pub start_address: Option<u64>,
    pub size: Option<u64>,
    pub region_flags: Option<u64>,
    pub name: Option<String>,
    pub description: Option<String>,
}

/// A category declared in the XDF header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdfCategory {
    pub index: u64,
    pub name: String,
}

/// An ordered category membership on an XDF object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryMembership {
    pub slot: usize,
    pub category_index: u64,
    pub resolved_category_index: Option<u64>,
    pub category_name: Option<String>,
}

/// The reference convention used by `CATEGORYMEM` values in one XDF document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryReferenceMode {
    /// Membership values are the numeric `CATEGORY index` values.
    DeclaredIndex,
    /// Membership values are one-based positions in the header category list.
    OneBasedPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Warning,
    Error,
}

/// A recoverable issue found while normalizing an XDF document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdfDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub path: String,
    pub message: String,
}

/// Typed XDF header metadata and address context.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XdfHeader {
    pub version: Option<String>,
    pub flags: Option<u64>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub defaults: XdfDefaults,
    pub base_offset: XdfBaseOffset,
    pub regions: Vec<XdfRegion>,
    pub raw_fields: BTreeMap<String, String>,
}

/// The normalized type of an XDF object that can be edited or transferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParameterKind {
    Constant,
    Table,
    BitField,
    Flag,
}

impl ParameterKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Constant => "constant",
            Self::Table => "table",
            Self::BitField => "bitfield",
            Self::Flag => "flag",
        }
    }
}

/// Logical dimensions of a parameter. Scalars and flags are `1 x 1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimensions {
    pub rows: usize,
    pub columns: usize,
}

impl Dimensions {
    pub fn elements(self) -> Option<usize> {
        self.rows.checked_mul(self.columns)
    }
}

/// Numeric interpretation of a stored element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NumericKind {
    Integer,
    Ieee754Binary32,
    Unsupported,
}

impl NumericKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Integer => "integer",
            Self::Ieee754Binary32 => "ieee754-binary32",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Effective physical storage semantics for an XDF value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageSpec {
    pub element_size_bits: u32,
    pub signed: bool,
    pub byte_order: Endianness,
    pub numeric_kind: NumericKind,
    pub column_major: bool,
    pub raw_type_flags: u32,
    pub unknown_type_flags: u32,
}

/// Physical storage layout for a normalized parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataLayout {
    /// Address is byte-addressed, matching the BIN model.
    pub address: usize,
    pub element_width_bits: usize,
    pub dimensions: Dimensions,
    pub row_stride_bits: i64,
    pub column_stride_bits: i64,
    pub signed: bool,
    pub endianness: Endianness,
    pub storage: StorageSpec,
    /// The complete byte footprint, including configured strides.
    pub range: ByteRange,
}

impl DataLayout {
    pub fn element_width_bytes(&self) -> usize {
        self.element_width_bits.saturating_add(7) / 8
    }

    pub fn element_count(&self) -> Option<usize> {
        self.dimensions.elements()
    }
}

/// A source label attached to an axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxisLabel {
    pub index: usize,
    pub value: String,
}

/// Display and engineering metadata attached to an axis.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisMetadata {
    pub units: Option<String>,
    pub unit_type: Option<u32>,
    pub decimal_places: Option<u32>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub output_type: Option<u32>,
}

/// A link from an axis to a separately stored data object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxisLink {
    pub index: Option<usize>,
    pub object_id_hash: Option<String>,
}

/// Raw EMBEDINFO metadata retained for callers that understand its extension fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedInfo {
    pub type_code: Option<u64>,
    pub attributes: BTreeMap<String, String>,
}

/// An optional axis attached to a table definition.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisDefinition {
    pub id: String,
    pub title: String,
    pub count: usize,
    pub address: Option<usize>,
    pub element_width_bits: usize,
    pub stride_bits: i64,
    pub signed: bool,
    pub endianness: Endianness,
    pub storage: Option<StorageSpec>,
    pub metadata: AxisMetadata,
    pub labels: Vec<AxisLabel>,
    pub links: Vec<AxisLink>,
    pub embed_info: Option<EmbedInfo>,
    pub conversion: Option<String>,
    pub range: Option<ByteRange>,
}

/// A normalized scalar, table, or bit-field definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterDefinition {
    /// Stable identity used for semantic matching. It never includes the address.
    pub semantic_id: String,
    pub unique_id: Option<String>,
    pub kind: ParameterKind,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub category_memberships: Vec<CategoryMembership>,
    pub raw_type: Option<String>,
    pub layout: DataLayout,
    pub conversion: Option<String>,
    pub bit_offset: Option<usize>,
    pub bit_width: Option<usize>,
    pub axes: Vec<AxisDefinition>,
    pub bit_mask: Option<u64>,
    pub raw_bit_mask: Option<String>,
}

/// Editable, normalized axis data. `address` is an XDF address, not a BIN offset.
#[derive(Debug, Clone, PartialEq)]
pub struct XdfAxisDraft {
    pub id: String,
    pub title: String,
    pub count: usize,
    pub address: Option<u64>,
    pub element_width_bits: u32,
    pub stride_bits: i64,
    pub signed: bool,
    pub endianness: Endianness,
    pub numeric_kind: NumericKind,
    pub unknown_type_flags: u32,
    pub conversion: Option<String>,
    pub metadata: AxisMetadata,
    pub labels: Vec<AxisLabel>,
}

/// Authorable fields for a scalar or map. Address and strides are explicit so the
/// editor can distinguish XDF addressing from resolved BIN addressing.
#[derive(Debug, Clone, PartialEq)]
pub struct XdfParameterDraft {
    pub unique_id: Option<String>,
    pub kind: ParameterKind,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    /// All source category memberships. The primary `category` field edits slot zero;
    /// other memberships are retained unless explicitly removed by a future editor.
    pub category_memberships: Vec<CategoryMembership>,
    pub xdf_address: u64,
    pub element_width_bits: u32,
    pub dimensions: Dimensions,
    pub signed: bool,
    pub endianness: Endianness,
    pub numeric_kind: NumericKind,
    pub column_major: bool,
    pub row_stride_bits: i64,
    pub column_stride_bits: i64,
    pub conversion: Option<String>,
    pub bit_offset: Option<usize>,
    pub bit_width: Option<usize>,
    pub bit_mask: Option<u64>,
    /// Unknown source flags are retained through ordinary definition edits.
    pub unknown_type_flags: u32,
    pub axes: Vec<XdfAxisDraft>,
}

/// Typed operations supported by the XDF maker and NookLink authoring proposals.
#[derive(Debug, Clone, PartialEq)]
pub enum XdfAuthoringOperation {
    AddParameter {
        definition: XdfParameterDraft,
    },
    ReplaceParameter {
        semantic_id: String,
        definition: XdfParameterDraft,
    },
    DeleteParameter {
        semantic_id: String,
    },
    ReorderParameter {
        semantic_id: String,
        index: usize,
    },
    SetHeader {
        header: XdfHeader,
    },
    SetCategories {
        categories: Vec<XdfCategory>,
    },
}

/// A normalized object that is retained for inspection but is not an editable cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdfAuxiliaryObject {
    pub kind: XdfAuxiliaryKind,
    pub source_name: String,
    pub unique_id: Option<String>,
    pub title: String,
    pub description: String,
    pub path: String,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum XdfAuxiliaryKind {
    Function,
    Patch,
    Checksum,
    Unknown,
}

/// A raw value read from or written to a mapped cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawValue {
    Unsigned(u64),
    Signed(i64),
    Float32Bits(u32),
}

/// Result of an engineering-unit write, including the value after storage quantization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngineeringWriteResult {
    pub requested_engineering: f64,
    pub raw_value: RawValue,
    pub stored_engineering: f64,
}

/// Failure while resolving or editing a mapped cell.
#[derive(Debug)]
pub enum CellAccessError {
    Mapping(XdfError),
    Core(CoreError),
    UnsupportedLayout {
        semantic_id: String,
        message: String,
    },
    InvalidValue {
        semantic_id: String,
        value: RawValue,
        message: String,
    },
    Conversion(ConversionError),
}

impl fmt::Display for CellAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mapping(error) => write!(f, "cell mapping failed: {error}"),
            Self::Core(error) => write!(f, "BIN access failed: {error}"),
            Self::UnsupportedLayout {
                semantic_id,
                message,
            } => write!(f, "unsupported cell layout for {semantic_id}: {message}"),
            Self::InvalidValue {
                semantic_id,
                value,
                message,
            } => write!(f, "invalid value {value:?} for {semantic_id}: {message}"),
            Self::Conversion(error) => write!(f, "engineering conversion failed: {error}"),
        }
    }
}

impl std::error::Error for CellAccessError {}

impl From<XdfError> for CellAccessError {
    fn from(error: XdfError) -> Self {
        Self::Mapping(error)
    }
}

impl From<CoreError> for CellAccessError {
    fn from(error: CoreError) -> Self {
        Self::Core(error)
    }
}

impl From<ConversionError> for CellAccessError {
    fn from(error: ConversionError) -> Self {
        Self::Conversion(error)
    }
}

impl ParameterDefinition {
    pub fn byte_range(&self) -> ByteRange {
        self.layout.range
    }

    pub fn dimensions(&self) -> Dimensions {
        self.layout.dimensions
    }

    pub fn is_scalar(&self) -> bool {
        self.kind == ParameterKind::Constant
    }

    pub fn axis_definition(&self, axis_index: usize) -> Result<&AxisDefinition, XdfError> {
        self.axes
            .get(axis_index)
            .ok_or_else(|| XdfError::InvalidAxis {
                semantic_id: self.semantic_id.clone(),
                axis_index,
                index: None,
                count: self.axes.len(),
            })
    }

    pub fn axis_range(&self, axis_index: usize, index: usize) -> Result<ByteRange, XdfError> {
        let axis = self.axis_definition(axis_index)?;
        let (start_bit, end_bit) = self.axis_bit_bounds(axis_index, index)?;
        let start = usize::try_from(start_bit / 8).map_err(|_| XdfError::Overflow {
            object: self.semantic_id.clone(),
            message: "axis start address overflows usize".to_string(),
        })?;
        let end = usize::try_from((end_bit + 7) / 8).map_err(|_| XdfError::Overflow {
            object: self.semantic_id.clone(),
            message: "axis end address overflows usize".to_string(),
        })?;
        if axis.address.is_none() || axis.storage.is_none() {
            return Err(XdfError::InvalidAxis {
                semantic_id: self.semantic_id.clone(),
                axis_index,
                index: Some(index),
                count: axis.count,
            });
        }
        Ok(ByteRange { start, end })
    }

    pub fn read_raw_axis(
        &self,
        bin: &BinDocument,
        axis_index: usize,
        index: usize,
    ) -> Result<RawValue, CellAccessError> {
        let axis = self.axis_definition(axis_index)?;
        let range = self.axis_access_range(axis_index, index)?;
        let storage = self.axis_storage(axis)?;
        let width_bytes = axis.element_width_bits.div_ceil(8);
        if storage.numeric_kind == NumericKind::Ieee754Binary32 {
            let bytes: [u8; 4] = bin.read_bytes(range.start, 4)?.try_into().map_err(|_| {
                CellAccessError::UnsupportedLayout {
                    semantic_id: self.semantic_id.clone(),
                    message: "binary32 axis does not resolve to four bytes".to_string(),
                }
            })?;
            let bits = match axis.endianness {
                Endianness::Little => u32::from_le_bytes(bytes),
                Endianness::Big => u32::from_be_bytes(bytes),
            };
            Ok(RawValue::Float32Bits(bits))
        } else if storage.signed {
            Ok(RawValue::Signed(bin.read_int(
                range.start,
                width_bytes,
                axis.endianness,
            )?))
        } else {
            Ok(RawValue::Unsigned(bin.read_uint(
                range.start,
                width_bytes,
                axis.endianness,
            )?))
        }
    }

    pub fn write_raw_axis(
        &self,
        transaction: &mut Transaction<'_>,
        axis_index: usize,
        index: usize,
        value: RawValue,
    ) -> Result<(), CellAccessError> {
        let axis = self.axis_definition(axis_index)?;
        let range = self.axis_access_range(axis_index, index)?;
        let storage = self.axis_storage(axis)?;
        let width_bytes = axis.element_width_bits.div_ceil(8);
        if storage.numeric_kind == NumericKind::Ieee754Binary32 {
            let RawValue::Float32Bits(bits) = value else {
                return Err(CellAccessError::InvalidValue {
                    semantic_id: self.semantic_id.clone(),
                    value,
                    message: "binary32 storage requires Float32Bits raw values".to_string(),
                });
            };
            let bytes = match axis.endianness {
                Endianness::Little => bits.to_le_bytes(),
                Endianness::Big => bits.to_be_bytes(),
            };
            transaction.write_bytes(range.start, &bytes)?;
        } else {
            match value {
                RawValue::Unsigned(value) => {
                    transaction.write_uint(range.start, width_bytes, axis.endianness, value)?
                }
                RawValue::Signed(value) => {
                    transaction.write_int(range.start, width_bytes, axis.endianness, value)?
                }
                RawValue::Float32Bits(_) => {
                    return Err(CellAccessError::InvalidValue {
                        semantic_id: self.semantic_id.clone(),
                        value,
                        message: "integer storage cannot accept Float32Bits raw values".to_string(),
                    })
                }
            }
        }
        Ok(())
    }

    pub fn read_engineering_axis(
        &self,
        bin: &BinDocument,
        axis_index: usize,
        index: usize,
    ) -> Result<f64, CellAccessError> {
        let axis = self.axis_definition(axis_index)?;
        let conversion = Conversion::parse(axis.conversion.as_deref().unwrap_or("X"))?;
        conversion
            .evaluate(raw_as_f64(self.read_raw_axis(bin, axis_index, index)?)?)
            .map_err(Into::into)
    }

    pub fn write_engineering_axis(
        &self,
        transaction: &mut Transaction<'_>,
        axis_index: usize,
        index: usize,
        engineering: f64,
    ) -> Result<EngineeringWriteResult, CellAccessError> {
        let axis = self.axis_definition(axis_index)?;
        let conversion = Conversion::parse(axis.conversion.as_deref().unwrap_or("X"))?;
        let raw = conversion.invert(engineering)?;
        let raw_value = match axis
            .storage
            .ok_or_else(|| CellAccessError::UnsupportedLayout {
                semantic_id: self.semantic_id.clone(),
                message: "axis has no writable storage".to_string(),
            })?
            .numeric_kind
        {
            NumericKind::Integer => integer_raw_value(&self.semantic_id, raw, axis.signed)?,
            NumericKind::Ieee754Binary32 => {
                let narrowed = raw as f32;
                if !narrowed.is_finite() {
                    return Err(CellAccessError::InvalidValue {
                        semantic_id: self.semantic_id.clone(),
                        value: RawValue::Float32Bits(narrowed.to_bits()),
                        message: "engineering value does not fit in finite binary32 storage"
                            .to_string(),
                    });
                }
                RawValue::Float32Bits(narrowed.to_bits())
            }
            NumericKind::Unsupported => {
                return Err(CellAccessError::UnsupportedLayout {
                    semantic_id: self.semantic_id.clone(),
                    message: "engineering access is unavailable for unsupported storage"
                        .to_string(),
                })
            }
        };
        let stored_engineering = conversion.evaluate(raw_as_f64(raw_value)?)?;
        self.write_raw_axis(transaction, axis_index, index, raw_value)?;
        Ok(EngineeringWriteResult {
            requested_engineering: engineering,
            raw_value,
            stored_engineering,
        })
    }

    fn axis_bit_bounds(&self, axis_index: usize, index: usize) -> Result<(i128, i128), XdfError> {
        let axis = self.axis_definition(axis_index)?;
        if index >= axis.count {
            return Err(XdfError::InvalidAxis {
                semantic_id: self.semantic_id.clone(),
                axis_index,
                index: Some(index),
                count: axis.count,
            });
        }
        let address = axis.address.ok_or_else(|| XdfError::InvalidAxis {
            semantic_id: self.semantic_id.clone(),
            axis_index,
            index: Some(index),
            count: axis.count,
        })?;
        let start = (address as i128)
            .checked_mul(8)
            .and_then(|base| {
                (index as i128)
                    .checked_mul(axis.stride_bits as i128)
                    .and_then(|offset| base.checked_add(offset))
            })
            .ok_or_else(|| XdfError::Overflow {
                object: self.semantic_id.clone(),
                message: "axis bit offset overflows signed arithmetic".to_string(),
            })?;
        let end = start
            .checked_add(axis.element_width_bits as i128)
            .ok_or_else(|| XdfError::Overflow {
                object: self.semantic_id.clone(),
                message: "axis bit end overflows signed arithmetic".to_string(),
            })?;
        checked_bit_bounds(&self.semantic_id, start, end)
    }

    fn axis_access_range(
        &self,
        axis_index: usize,
        index: usize,
    ) -> Result<ByteRange, CellAccessError> {
        let axis = self.axis_definition(axis_index)?;
        if axis.address.is_none() || axis.storage.is_none() {
            return Err(CellAccessError::UnsupportedLayout {
                semantic_id: self.semantic_id.clone(),
                message: "axis has no address-backed storage".to_string(),
            });
        }
        self.axis_range(axis_index, index)
            .map_err(CellAccessError::from)
            .and_then(|range| {
                if axis.element_width_bits % 8 != 0
                    || (range.start as i128) * 8 != self.axis_bit_bounds(axis_index, index)?.0
                {
                    return Err(CellAccessError::UnsupportedLayout {
                        semantic_id: self.semantic_id.clone(),
                        message: "raw axis access requires byte-aligned cells".to_string(),
                    });
                }
                Ok(range)
            })
    }

    fn axis_storage(&self, axis: &AxisDefinition) -> Result<StorageSpec, CellAccessError> {
        let storage = axis
            .storage
            .ok_or_else(|| CellAccessError::UnsupportedLayout {
                semantic_id: self.semantic_id.clone(),
                message: "axis has no address-backed storage".to_string(),
            })?;
        if storage.numeric_kind == NumericKind::Unsupported
            || (storage.numeric_kind == NumericKind::Ieee754Binary32
                && axis.element_width_bits != 32)
        {
            return Err(CellAccessError::UnsupportedLayout {
                semantic_id: self.semantic_id.clone(),
                message: "axis storage kind or width is unsupported".to_string(),
            });
        }
        Ok(storage)
    }

    /// Return resolved category names ordered by membership level.
    pub fn category_path(&self) -> Vec<String> {
        let mut memberships: Vec<_> = self.category_memberships.iter().enumerate().collect();
        memberships.sort_by_key(|(ordinal, membership)| (membership.slot, *ordinal));
        memberships
            .into_iter()
            .filter_map(|(_, membership)| membership.category_name.clone())
            .collect()
    }

    /// Resolve one logical cell to the byte range containing its mapped bits.
    pub fn cell_range(&self, row: usize, column: usize) -> Result<ByteRange, XdfError> {
        let (start_bit, end_bit) = self.cell_bit_bounds(row, column)?;
        let start_byte = usize::try_from(start_bit / 8).map_err(|_| XdfError::Overflow {
            object: self.semantic_id.clone(),
            message: "cell start address overflows usize".to_string(),
        })?;
        let end_byte = usize::try_from((end_bit + 7) / 8).map_err(|_| XdfError::Overflow {
            object: self.semantic_id.clone(),
            message: "cell end address overflows usize".to_string(),
        })?;
        Ok(ByteRange {
            start: start_byte,
            end: end_byte,
        })
    }

    /// Compile this object's conversion formula, treating an absent formula as identity.
    pub fn compile_conversion(&self) -> Result<Conversion, CellAccessError> {
        Conversion::parse(self.conversion.as_deref().unwrap_or("X"))
            .map_err(CellAccessError::Conversion)
    }

    /// Read one cell and convert its raw storage value to engineering units.
    pub fn read_engineering_cell(
        &self,
        bin: &BinDocument,
        row: usize,
        column: usize,
    ) -> Result<f64, CellAccessError> {
        let conversion = self.compile_conversion()?;
        let raw = self.read_raw_cell(bin, row, column)?;
        conversion
            .evaluate(raw_as_f64(raw)?)
            .map_err(CellAccessError::Conversion)
    }

    /// Invert an engineering value, validate storage representability, and stage the raw write.
    pub fn write_engineering_cell(
        &self,
        transaction: &mut Transaction<'_>,
        row: usize,
        column: usize,
        engineering: f64,
    ) -> Result<EngineeringWriteResult, CellAccessError> {
        let conversion = self.compile_conversion()?;
        let raw = conversion
            .invert(engineering)
            .map_err(CellAccessError::Conversion)?;
        let result = match self.layout.storage.numeric_kind {
            NumericKind::Integer => {
                let raw_value =
                    integer_raw_value(&self.semantic_id, raw, self.layout.storage.signed)?;
                let stored_engineering = conversion
                    .evaluate(raw_as_f64(raw_value)?)
                    .map_err(CellAccessError::Conversion)?;
                self.write_raw_cell(transaction, row, column, raw_value)?;
                EngineeringWriteResult {
                    requested_engineering: engineering,
                    raw_value,
                    stored_engineering,
                }
            }
            NumericKind::Ieee754Binary32 => {
                if self.layout.element_width_bits != 32 {
                    return Err(CellAccessError::UnsupportedLayout {
                        semantic_id: self.semantic_id.clone(),
                        message: "binary32 storage must have a 32-bit element".to_string(),
                    });
                }
                let narrowed = raw as f32;
                if !narrowed.is_finite() {
                    return Err(CellAccessError::InvalidValue {
                        semantic_id: self.semantic_id.clone(),
                        value: RawValue::Float32Bits(narrowed.to_bits()),
                        message: "engineering value does not fit in finite binary32 storage"
                            .to_string(),
                    });
                }
                let raw_value = RawValue::Float32Bits(narrowed.to_bits());
                let stored_engineering = conversion
                    .evaluate(narrowed as f64)
                    .map_err(CellAccessError::Conversion)?;
                self.write_raw_cell(transaction, row, column, raw_value)?;
                EngineeringWriteResult {
                    requested_engineering: engineering,
                    raw_value,
                    stored_engineering,
                }
            }
            NumericKind::Unsupported => {
                return Err(CellAccessError::UnsupportedLayout {
                    semantic_id: self.semantic_id.clone(),
                    message: "engineering access is unavailable for unsupported storage"
                        .to_string(),
                })
            }
        };
        Ok(result)
    }

    /// Read one raw scalar or table cell from a BIN document.
    pub fn read_raw_cell(
        &self,
        bin: &BinDocument,
        row: usize,
        column: usize,
    ) -> Result<RawValue, CellAccessError> {
        let (start_bit, _) = self.cell_bit_bounds(row, column)?;
        self.ensure_byte_aligned(start_bit)?;
        let range = self.cell_range(row, column)?;
        self.ensure_supported_storage()?;
        let width_bytes = self.layout.element_width_bytes();
        if self.layout.storage.numeric_kind == NumericKind::Ieee754Binary32 {
            let bytes = bin.read_bytes(range.start, 4)?;
            let bytes: [u8; 4] =
                bytes
                    .try_into()
                    .map_err(|_| CellAccessError::UnsupportedLayout {
                        semantic_id: self.semantic_id.clone(),
                        message: "binary32 cell does not resolve to exactly four bytes".to_string(),
                    })?;
            let bits = match self.layout.endianness {
                Endianness::Little => u32::from_le_bytes(bytes),
                Endianness::Big => u32::from_be_bytes(bytes),
            };
            return Ok(RawValue::Float32Bits(bits));
        }
        if matches!(self.kind, ParameterKind::BitField | ParameterKind::Flag) {
            let (bit_offset, bit_width) = self.bitfield_parts()?;
            let raw = bin.read_uint(self.layout.address, width_bytes, self.layout.endianness)?;
            let mask = value_mask(bit_width);
            return Ok(RawValue::Unsigned((raw >> bit_offset) & mask));
        }
        if self.layout.signed {
            Ok(RawValue::Signed(bin.read_int(
                range.start,
                width_bytes,
                self.layout.endianness,
            )?))
        } else {
            Ok(RawValue::Unsigned(bin.read_uint(
                range.start,
                width_bytes,
                self.layout.endianness,
            )?))
        }
    }

    /// Write one raw scalar or table cell into an existing atomic BIN transaction.
    pub fn write_raw_cell(
        &self,
        transaction: &mut Transaction<'_>,
        row: usize,
        column: usize,
        value: RawValue,
    ) -> Result<(), CellAccessError> {
        let (start_bit, _) = self.cell_bit_bounds(row, column)?;
        self.ensure_byte_aligned(start_bit)?;
        let range = self.cell_range(row, column)?;
        self.ensure_supported_storage()?;
        let width_bytes = self.layout.element_width_bytes();
        if self.layout.storage.numeric_kind == NumericKind::Ieee754Binary32 {
            let RawValue::Float32Bits(bits) = value else {
                return Err(CellAccessError::InvalidValue {
                    semantic_id: self.semantic_id.clone(),
                    value,
                    message: "binary32 storage requires Float32Bits raw values".to_string(),
                });
            };
            let bytes = match self.layout.endianness {
                Endianness::Little => bits.to_le_bytes(),
                Endianness::Big => bits.to_be_bytes(),
            };
            transaction.write_bytes(range.start, &bytes)?;
            return Ok(());
        }
        if matches!(value, RawValue::Float32Bits(_)) {
            return Err(CellAccessError::InvalidValue {
                semantic_id: self.semantic_id.clone(),
                value,
                message: "integer storage cannot accept Float32Bits raw values".to_string(),
            });
        }
        if matches!(self.kind, ParameterKind::BitField | ParameterKind::Flag) {
            let (bit_offset, bit_width) = self.bitfield_parts()?;
            let unsigned = match value {
                RawValue::Unsigned(value) => value,
                RawValue::Signed(value) if value >= 0 => value as u64,
                RawValue::Signed(_) => {
                    return Err(CellAccessError::InvalidValue {
                        semantic_id: self.semantic_id.clone(),
                        value,
                        message: "bitfields require a non-negative value".to_string(),
                    })
                }
                RawValue::Float32Bits(_) => {
                    return Err(CellAccessError::InvalidValue {
                        semantic_id: self.semantic_id.clone(),
                        value,
                        message: "bitfields require an integer raw value".to_string(),
                    })
                }
            };
            let mask = value_mask(bit_width);
            if unsigned > mask {
                return Err(CellAccessError::InvalidValue {
                    semantic_id: self.semantic_id.clone(),
                    value,
                    message: format!("value exceeds the {bit_width}-bit field"),
                });
            }
            let raw =
                transaction.read_uint(self.layout.address, width_bytes, self.layout.endianness)?;
            let shifted_mask = mask << bit_offset;
            let updated = (raw & !shifted_mask) | (unsigned << bit_offset);
            transaction.write_uint(
                self.layout.address,
                width_bytes,
                self.layout.endianness,
                updated,
            )?;
            return Ok(());
        }
        match value {
            RawValue::Unsigned(value) => {
                transaction.write_uint(range.start, width_bytes, self.layout.endianness, value)?
            }
            RawValue::Signed(value) => {
                transaction.write_int(range.start, width_bytes, self.layout.endianness, value)?
            }
            RawValue::Float32Bits(_) => {
                return Err(CellAccessError::InvalidValue {
                    semantic_id: self.semantic_id.clone(),
                    value,
                    message: "integer storage cannot accept Float32Bits raw values".to_string(),
                })
            }
        }
        Ok(())
    }

    fn ensure_supported_storage(&self) -> Result<(), CellAccessError> {
        if self.layout.storage.numeric_kind == NumericKind::Unsupported {
            return Err(CellAccessError::UnsupportedLayout {
                semantic_id: self.semantic_id.clone(),
                message: "raw access is unavailable for unsupported storage".to_string(),
            });
        }
        if self.layout.storage.numeric_kind == NumericKind::Ieee754Binary32
            && self.layout.element_width_bits != 32
        {
            return Err(CellAccessError::UnsupportedLayout {
                semantic_id: self.semantic_id.clone(),
                message: "binary32 storage must have a 32-bit element".to_string(),
            });
        }
        Ok(())
    }

    fn cell_bit_bounds(&self, row: usize, column: usize) -> Result<(i128, i128), XdfError> {
        if row >= self.layout.dimensions.rows || column >= self.layout.dimensions.columns {
            return Err(XdfError::InvalidCell {
                semantic_id: self.semantic_id.clone(),
                row,
                column,
                rows: self.layout.dimensions.rows,
                columns: self.layout.dimensions.columns,
            });
        }
        let base_bit = (self.layout.address as i128) * 8;
        let row_offset = (row as i128)
            .checked_mul(self.layout.row_stride_bits as i128)
            .ok_or_else(|| XdfError::Overflow {
                object: self.semantic_id.clone(),
                message: "cell row stride overflows signed arithmetic".to_string(),
            })?;
        let column_offset = (column as i128)
            .checked_mul(self.layout.column_stride_bits as i128)
            .ok_or_else(|| XdfError::Overflow {
                object: self.semantic_id.clone(),
                message: "cell column stride overflows signed arithmetic".to_string(),
            })?;
        let start_bit = base_bit
            .checked_add(row_offset)
            .and_then(|value| value.checked_add(column_offset))
            .ok_or_else(|| XdfError::Overflow {
                object: self.semantic_id.clone(),
                message: "cell bit offset overflows signed arithmetic".to_string(),
            })?;
        let end_bit = start_bit
            .checked_add(self.layout.element_width_bits as i128)
            .ok_or_else(|| XdfError::Overflow {
                object: self.semantic_id.clone(),
                message: "cell bit end overflows signed arithmetic".to_string(),
            })?;
        checked_bit_bounds(&self.semantic_id, start_bit, end_bit)
    }

    fn ensure_byte_aligned(&self, start_bit: i128) -> Result<(), CellAccessError> {
        if start_bit % 8 != 0 || !self.layout.element_width_bits.is_multiple_of(8) {
            return Err(CellAccessError::UnsupportedLayout {
                semantic_id: self.semantic_id.clone(),
                message: "raw integer access requires byte-aligned cells".to_string(),
            });
        }
        Ok(())
    }

    fn bitfield_parts(&self) -> Result<(usize, usize), CellAccessError> {
        let bit_offset = self
            .bit_offset
            .ok_or_else(|| CellAccessError::UnsupportedLayout {
                semantic_id: self.semantic_id.clone(),
                message: "bitfield has no bit offset".to_string(),
            })?;
        let bit_width = self
            .bit_width
            .ok_or_else(|| CellAccessError::UnsupportedLayout {
                semantic_id: self.semantic_id.clone(),
                message: "bitfield has no bit width".to_string(),
            })?;
        let end = bit_offset.checked_add(bit_width).ok_or_else(|| {
            CellAccessError::UnsupportedLayout {
                semantic_id: self.semantic_id.clone(),
                message: "bitfield range overflows usize".to_string(),
            }
        })?;
        if bit_width == 0 || end > self.layout.element_width_bits || end > 64 {
            return Err(CellAccessError::UnsupportedLayout {
                semantic_id: self.semantic_id.clone(),
                message: "bitfield must fit inside a byte-aligned value no wider than 64 bits"
                    .to_string(),
            });
        }
        Ok((bit_offset, bit_width))
    }
}

fn value_mask(width: usize) -> u64 {
    if width == 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

fn contiguous_mask_parts(mask: u64) -> Option<(usize, usize)> {
    if mask == 0 {
        return None;
    }
    let offset = mask.trailing_zeros() as usize;
    let compact = mask >> offset;
    if compact != u64::MAX && compact & (compact + 1) != 0 {
        return None;
    }
    Some((offset, compact.count_ones() as usize))
}

/// A single out-of-bounds mapping found while validating an XDF against a BIN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub semantic_id: String,
    pub title: String,
    pub kind: ParameterKind,
    pub range: Option<ByteRange>,
    pub message: String,
}

/// Result of resolving all XDF mappings against a BIN byte slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn issue_count(&self) -> usize {
        self.issues.len()
    }
}

fn raw_as_f64(value: RawValue) -> Result<f64, CellAccessError> {
    let result = match value {
        RawValue::Unsigned(value) => value as f64,
        RawValue::Signed(value) => value as f64,
        RawValue::Float32Bits(bits) => f32::from_bits(bits) as f64,
    };
    if result.is_finite() {
        Ok(result)
    } else {
        Err(CellAccessError::Conversion(ConversionError::NonFiniteInput))
    }
}

fn integer_raw_value(
    semantic_id: &str,
    raw: f64,
    signed: bool,
) -> Result<RawValue, CellAccessError> {
    if !raw.is_finite() {
        return Err(CellAccessError::Conversion(
            ConversionError::NonFiniteResult,
        ));
    }
    // Integer storage quantizes engineering values to the nearest raw step.
    let rounded = raw.round();
    if signed {
        let signed_limit = (1u64 << 63) as f64;
        if rounded < -signed_limit || rounded >= signed_limit {
            return Err(CellAccessError::InvalidValue {
                semantic_id: semantic_id.to_string(),
                value: RawValue::Unsigned(0),
                message: format!("raw value {rounded} is outside signed 64-bit storage"),
            });
        }
        Ok(RawValue::Signed(rounded as i64))
    } else {
        let unsigned_limit = (2u64.pow(63) as f64) * 2.0;
        if rounded < 0.0 || rounded >= unsigned_limit {
            return Err(CellAccessError::InvalidValue {
                semantic_id: semantic_id.to_string(),
                value: RawValue::Unsigned(0),
                message: format!("raw value {rounded} is outside unsigned 64-bit storage"),
            });
        }
        Ok(RawValue::Unsigned(rounded as u64))
    }
}

fn checked_bit_bounds(
    object: &str,
    start_bit: i128,
    end_bit: i128,
) -> Result<(i128, i128), XdfError> {
    let max_bit = (usize::MAX as i128) * 8;
    if start_bit < 0 || end_bit < start_bit || end_bit > max_bit {
        return Err(XdfError::Overflow {
            object: object.to_string(),
            message: format!(
                "mapped bit range [{start_bit}, {end_bit}) is outside the BIN address space"
            ),
        });
    }
    Ok((start_bit, end_bit))
}

/// A parsed XDF document with exact and normalized identities.
#[derive(Debug, Clone, PartialEq)]
pub struct XdfDocument {
    pub source_path: Option<PathBuf>,
    pub exact_sha256: String,
    pub normalized_fingerprint: String,
    pub header: XdfHeader,
    pub categories: Vec<XdfCategory>,
    pub category_reference_mode: CategoryReferenceMode,
    pub parameters: Vec<ParameterDefinition>,
    pub auxiliary_objects: Vec<XdfAuxiliaryObject>,
    pub diagnostics: Vec<XdfDiagnostic>,
    pub unknown_element_count: usize,
    source_root: XmlNode,
}

impl XdfDocument {
    /// Create an empty XDF draft with a title and no BIN-specific assumptions.
    pub fn new(title: impl Into<String>) -> Result<Self, XdfError> {
        let root = XmlNode {
            name: "XDFFORMAT".to_string(),
            attrs: BTreeMap::from([("version".to_string(), "1.70".to_string())]),
            children: vec![XmlNode {
                name: "XDFHEADER".to_string(),
                attrs: BTreeMap::new(),
                children: vec![text_node("DEFTITLE", &title.into())],
                text: String::new(),
            }],
            text: String::new(),
        };
        let text = serialize_xdf_root(&root);
        Self::parse(text.as_bytes())
    }

    /// Load and normalize an XDF file from disk.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, XdfError> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|error| io_error("read XDF", path, error))?;
        Self::parse_with_source(&bytes, Some(path.to_path_buf()))
    }

    /// Parse XDF bytes without assigning a source path.
    pub fn parse(bytes: &[u8]) -> Result<Self, XdfError> {
        Self::parse_with_source(bytes, None)
    }

    fn parse_with_source(bytes: &[u8], source_path: Option<PathBuf>) -> Result<Self, XdfError> {
        let input = String::from_utf8_lossy(bytes);
        let root = XmlParser::new(&input).parse_document()?;
        if root.name != "XDF" && root.name != "XDFFORMAT" {
            return Err(XdfError::Xml {
                position: 0,
                message: format!(
                    "expected XDF or XDFFORMAT root element, found <{}>",
                    root.name
                ),
            });
        }

        let (header, categories, mut diagnostics) = parse_header(&root)?;
        let auxiliary_objects = collect_auxiliary_objects(&root, &mut diagnostics);
        let mut nodes = Vec::new();
        collect_parameter_nodes(&root, &mut nodes);
        let category_reference_mode = detect_category_reference_mode(&categories, &nodes)?;
        if category_reference_mode == CategoryReferenceMode::OneBasedPosition {
            diagnostics.push(XdfDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "category-reference-one-based".to_string(),
                path: "/XDFHEADER/CATEGORY".to_string(),
                message: "CATEGORYMEM values are interpreted as one-based positions in the XDF header category list".to_string(),
            });
        }
        let mut parameters = Vec::with_capacity(nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            parameters.push(parse_parameter(
                node,
                index,
                &header,
                &categories,
                category_reference_mode,
                &mut diagnostics,
            )?);
        }
        normalize_parameter_identities(&mut parameters)?;

        let normalized_fingerprint = normalized_fingerprint(&parameters);
        Ok(Self {
            source_path,
            exact_sha256: sha256_hex(bytes),
            normalized_fingerprint,
            header,
            categories,
            category_reference_mode,
            parameters,
            auxiliary_objects,
            diagnostics,
            unknown_element_count: count_unknown_elements(&root),
            source_root: root,
        })
    }

    /// Serialize the retained XDF structure with the document's current conversions.
    ///
    /// The source tree is cloned before conversion nodes are updated, so exporting never
    /// changes the loaded document or its source identity. Comments and processing
    /// instructions are intentionally normalized by the tolerant parser.
    pub fn to_xdf_text(&self) -> Result<String, XdfError> {
        let mut root = self.source_root.clone();
        let mut parameter_index = 0;
        update_parameter_nodes(&mut root, &self.parameters, &mut parameter_index)?;
        if parameter_index != self.parameters.len() {
            return Err(XdfError::Export {
                message: format!(
                    "source tree contains {parameter_index} editable object(s), but the model has {}",
                    self.parameters.len()
                ),
            });
        }
        Ok(serialize_xdf_root(&root))
    }

    /// Convert a parsed definition into editable XDF-addressed fields.
    pub fn authoring_draft(&self, semantic_id: &str) -> Result<XdfParameterDraft, XdfError> {
        let parameter = self
            .parameter(semantic_id)
            .ok_or_else(|| XdfError::Export {
                message: format!("parameter {semantic_id} is not present in this XDF"),
            })?;
        Ok(XdfParameterDraft {
            unique_id: parameter.unique_id.clone(),
            kind: parameter.kind,
            title: parameter.title.clone(),
            description: parameter.description.clone(),
            category: parameter.category.clone(),
            category_memberships: parameter.category_memberships.clone(),
            xdf_address: reverse_translate_address(
                self.header.base_offset,
                parameter.layout.address,
            )?,
            element_width_bits: parameter.layout.storage.element_size_bits,
            dimensions: parameter.layout.dimensions,
            signed: parameter.layout.storage.signed,
            endianness: parameter.layout.storage.byte_order,
            numeric_kind: parameter.layout.storage.numeric_kind,
            column_major: parameter.layout.storage.column_major,
            row_stride_bits: parameter.layout.row_stride_bits,
            column_stride_bits: parameter.layout.column_stride_bits,
            conversion: parameter.conversion.clone(),
            bit_offset: parameter.bit_offset,
            bit_width: parameter.bit_width,
            bit_mask: parameter.bit_mask,
            unknown_type_flags: parameter.layout.storage.unknown_type_flags,
            axes: parameter
                .axes
                .iter()
                .map(|axis| {
                    Ok(XdfAxisDraft {
                        id: axis.id.clone(),
                        title: axis.title.clone(),
                        count: axis.count,
                        address: axis
                            .address
                            .map(|address| {
                                reverse_translate_address(self.header.base_offset, address)
                            })
                            .transpose()?,
                        element_width_bits: u32::try_from(axis.element_width_bits).map_err(
                            |_| XdfError::Overflow {
                                object: format!("{} axis {}", parameter.semantic_id, axis.id),
                                message: "axis element width exceeds u32".to_string(),
                            },
                        )?,
                        stride_bits: axis.stride_bits,
                        signed: axis.signed,
                        endianness: axis.endianness,
                        numeric_kind: axis
                            .storage
                            .map_or(parameter.layout.storage.numeric_kind, |storage| {
                                storage.numeric_kind
                            }),
                        unknown_type_flags: axis
                            .storage
                            .map_or(parameter.layout.storage.unknown_type_flags, |storage| {
                                storage.unknown_type_flags
                            }),
                        conversion: axis.conversion.clone(),
                        metadata: axis.metadata.clone(),
                        labels: axis.labels.clone(),
                    })
                })
                .collect::<Result<Vec<_>, XdfError>>()?,
        })
    }

    /// Convert a BIN byte offset to its XDF address using this document's base offset.
    pub fn xdf_address_for_bin_offset(&self, bin_offset: usize) -> Result<u64, XdfError> {
        reverse_translate_address(self.header.base_offset, bin_offset)
    }

    /// Apply a typed XDF edit batch atomically. Each operation is reparsed before the
    /// next, so callers only ever retain a structurally valid document.
    pub fn apply_authoring_operations(
        &mut self,
        operations: &[XdfAuthoringOperation],
    ) -> Result<(), XdfError> {
        if operations.is_empty() {
            return Ok(());
        }
        let mut candidate = self.clone();
        for operation in operations {
            candidate.apply_authoring_operation(operation)?;
        }
        *self = candidate;
        Ok(())
    }

    fn apply_authoring_operation(
        &mut self,
        operation: &XdfAuthoringOperation,
    ) -> Result<(), XdfError> {
        let mut root = self.source_root.clone();
        match operation {
            XdfAuthoringOperation::AddParameter { definition } => {
                let mut categories = self.categories.clone();
                let category_reference = ensure_category(
                    &mut root,
                    &mut categories,
                    definition.category.as_deref(),
                    self.category_reference_mode,
                )?;
                let memberships = category_memberships_for_definition(
                    definition,
                    category_reference,
                    &categories,
                    self.category_reference_mode,
                );
                root.children
                    .push(build_parameter_node(definition, &memberships)?);
            }
            XdfAuthoringOperation::ReplaceParameter {
                semantic_id,
                definition,
            } => {
                let index = self
                    .parameters
                    .iter()
                    .position(|parameter| parameter.semantic_id == *semantic_id)
                    .ok_or_else(|| XdfError::Export {
                        message: format!("parameter {semantic_id} is not present in this XDF"),
                    })?;
                let mut categories = self.categories.clone();
                let category_reference = ensure_category(
                    &mut root,
                    &mut categories,
                    definition.category.as_deref(),
                    self.category_reference_mode,
                )?;
                let memberships = category_memberships_for_definition(
                    definition,
                    category_reference,
                    &categories,
                    self.category_reference_mode,
                );
                update_nth_parameter_node(&mut root, index, definition, &memberships)?;
            }
            XdfAuthoringOperation::DeleteParameter { semantic_id } => {
                let index = self
                    .parameters
                    .iter()
                    .position(|parameter| parameter.semantic_id == *semantic_id)
                    .ok_or_else(|| XdfError::Export {
                        message: format!("parameter {semantic_id} is not present in this XDF"),
                    })?;
                if !remove_nth_parameter_node(&mut root, index) {
                    return Err(XdfError::Export {
                        message: format!("source XML for parameter {semantic_id} is unavailable"),
                    });
                }
            }
            XdfAuthoringOperation::ReorderParameter { semantic_id, index } => {
                let from = self
                    .parameters
                    .iter()
                    .position(|parameter| parameter.semantic_id == *semantic_id)
                    .ok_or_else(|| XdfError::Export {
                        message: format!("parameter {semantic_id} is not present in this XDF"),
                    })?;
                reorder_root_parameters(&mut root, from, *index, self.parameters.len())?;
            }
            XdfAuthoringOperation::SetHeader { header } => {
                sync_header_node(&mut root, header);
            }
            XdfAuthoringOperation::SetCategories { categories } => {
                validate_categories(categories)?;
                remap_category_references(
                    &mut root,
                    &self.categories,
                    categories,
                    self.category_reference_mode,
                )?;
                remap_parameter_category_paths(&mut root, &self.categories, categories);
                sync_category_nodes(&mut root, categories);
            }
        }

        let text = serialize_xdf_root(&root);
        *self = Self::parse_with_source(text.as_bytes(), self.source_path.clone())?;
        Ok(())
    }

    pub fn parameter(&self, semantic_id: &str) -> Option<&ParameterDefinition> {
        self.parameters
            .iter()
            .find(|parameter| parameter.semantic_id == semantic_id)
    }

    /// Validate every parameter and attached axis range against a BIN byte slice.
    pub fn validate_against(&self, bin: &[u8]) -> ValidationReport {
        let mut issues = Vec::new();
        for parameter in &self.parameters {
            add_range_issue(
                &mut issues,
                parameter.semantic_id.clone(),
                parameter.title.clone(),
                parameter.kind,
                Some(parameter.layout.range),
                bin.len(),
                "parameter range",
            );
            for axis in &parameter.axes {
                if let Some(range) = axis.range {
                    add_range_issue(
                        &mut issues,
                        parameter.semantic_id.clone(),
                        format!("{} axis {}", parameter.title, axis.id),
                        parameter.kind,
                        Some(range),
                        bin.len(),
                        "axis range",
                    );
                }
            }
        }
        ValidationReport {
            valid: issues.is_empty(),
            issues,
        }
    }

    /// Convenience form for callers that already own a [`BinDocument`].
    pub fn validate_against_document(&self, document: &BinDocument) -> ValidationReport {
        self.validate_against(document.bytes())
    }
}

fn add_range_issue(
    issues: &mut Vec<ValidationIssue>,
    semantic_id: String,
    title: String,
    kind: ParameterKind,
    range: Option<ByteRange>,
    bin_size: usize,
    label: &str,
) {
    let Some(range_value) = range else {
        issues.push(ValidationIssue {
            semantic_id,
            title,
            kind,
            range: None,
            message: format!("{label} could not be resolved"),
        });
        return;
    };
    if range_value.start > range_value.end || range_value.end > bin_size {
        issues.push(ValidationIssue {
            semantic_id,
            title,
            kind,
            range: Some(range_value),
            message: format!(
                "{label} [{}..{}) is outside BIN size {}",
                range_value.start, range_value.end, bin_size
            ),
        });
    }
}

fn parse_header(
    root: &XmlNode,
) -> Result<(XdfHeader, Vec<XdfCategory>, Vec<XdfDiagnostic>), XdfError> {
    let header_node = root.children.iter().find(|child| child.name == "XDFHEADER");
    let mut raw_fields = BTreeMap::new();
    raw_fields.insert("format.root".to_string(), root.name.clone());
    if let Some(version) = root.attr_any(&["version", "fileversion"]) {
        raw_fields.insert("format.version".to_string(), version);
    }
    if let Some(header) = header_node {
        collect_raw_fields(header, "header", &mut raw_fields);
    }

    let version = root.attr_any(&["version", "fileversion"]);
    let flags = header_node
        .and_then(|header| lookup_string(header, None, &["flags"], &["FLAGS"]))
        .map(|value| parse_u64("XDFHEADER", "flags", &value))
        .transpose()?;
    let title = header_node.and_then(|header| {
        lookup_string(header, None, &["deftitle", "title"], &["DEFTITLE", "TITLE"])
    });
    let description = header_node
        .and_then(|header| lookup_string(header, None, &["description"], &["DESCRIPTION"]));
    let author =
        header_node.and_then(|header| lookup_string(header, None, &["author"], &["AUTHOR"]));

    let defaults_node = header_node.and_then(|header| {
        header
            .children
            .iter()
            .find(|child| child.name == "DEFAULTS")
    });
    let defaults = if let Some(defaults_node) = defaults_node {
        XdfDefaults {
            data_size_bits: parse_optional_u32_attr(
                defaults_node,
                "datasizeinbits",
                "XDFHEADER.DEFAULTS",
                "datasizeinbits",
            )?,
            significant_digits: parse_optional_u32_attr(
                defaults_node,
                "sigdigits",
                "XDFHEADER.DEFAULTS",
                "sigdigits",
            )?,
            output_type: parse_optional_u32_attr(
                defaults_node,
                "outputtype",
                "XDFHEADER.DEFAULTS",
                "outputtype",
            )?,
            signed: parse_optional_bool_attr(
                defaults_node,
                "signed",
                "XDFHEADER.DEFAULTS",
                "signed",
            )?,
            lsb_first: parse_optional_bool_attr(
                defaults_node,
                "lsbfirst",
                "XDFHEADER.DEFAULTS",
                "lsbfirst",
            )?,
            float: parse_optional_bool_attr(defaults_node, "float", "XDFHEADER.DEFAULTS", "float")?,
        }
    } else {
        XdfDefaults::default()
    };

    let base_offset_node = header_node.and_then(|header| {
        header
            .children
            .iter()
            .find(|child| child.name == "BASEOFFSET")
    });
    let base_offset = if let Some(base_offset_node) = base_offset_node {
        let offset = base_offset_node
            .attr_any(&["offset"])
            .map(|value| parse_u64("XDFHEADER.BASEOFFSET", "offset", &value))
            .transpose()?
            .unwrap_or(0);
        let subtract = base_offset_node
            .attr_any(&["subtract"])
            .map(|value| parse_bool_strict("XDFHEADER.BASEOFFSET", "subtract", &value))
            .transpose()?
            .unwrap_or(false);
        XdfBaseOffset { offset, subtract }
    } else {
        XdfBaseOffset::default()
    };

    let mut diagnostics = Vec::new();
    let mut regions = Vec::new();
    let mut categories = Vec::new();
    if let Some(header) = header_node {
        for child in &header.children {
            match child.name.as_str() {
                "REGION" => regions.push(parse_region(child)?),
                "CATEGORY" => {
                    let index_value = child.attr_any(&["index"]).ok_or_else(|| {
                        invalid_field(
                            "XDFHEADER",
                            "category.index",
                            "<missing>".to_string(),
                            "category index is required",
                        )
                    })?;
                    let index = parse_u64("XDFHEADER", "category.index", &index_value)?;
                    let name = child
                        .attr_any(&["name"])
                        .or_else(|| (!child.text_value().is_empty()).then(|| child.text_value()))
                        .unwrap_or_else(|| format!("Category {index}"));
                    if categories
                        .iter()
                        .any(|category: &XdfCategory| category.index == index)
                    {
                        diagnostics.push(XdfDiagnostic {
                            severity: DiagnosticSeverity::Warning,
                            code: "duplicate-category".to_string(),
                            path: format!("XDFHEADER/CATEGORY[{index:#x}]"),
                            message: format!(
                                "category index {index:#x} is declared more than once"
                            ),
                        });
                    }
                    categories.push(XdfCategory { index, name });
                }
                _ => {}
            }
        }
    }

    let header = XdfHeader {
        version,
        flags,
        title,
        description,
        author,
        defaults,
        base_offset,
        regions,
        raw_fields,
    };
    Ok((header, categories, diagnostics))
}

fn collect_raw_fields(node: &XmlNode, prefix: &str, fields: &mut BTreeMap<String, String>) {
    for (name, value) in &node.attrs {
        fields.insert(format!("{prefix}.@{name}"), value.clone());
    }
    let text = node.text_value();
    if !text.is_empty() {
        fields.insert(format!("{prefix}.#text"), text);
    }
    for (index, child) in node.children.iter().enumerate() {
        collect_raw_fields(
            child,
            &format!("{prefix}/{name}[{index}]", name = child.name),
            fields,
        );
    }
}

fn parse_region(node: &XmlNode) -> Result<XdfRegion, XdfError> {
    Ok(XdfRegion {
        type_code: parse_optional_u64_attr(node, "type", "XDFHEADER.REGION", "type")?,
        start_address: parse_optional_u64_attr(
            node,
            "startaddress",
            "XDFHEADER.REGION",
            "startaddress",
        )?,
        size: parse_optional_u64_attr(node, "size", "XDFHEADER.REGION", "size")?,
        region_flags: parse_optional_u64_attr(
            node,
            "regionflags",
            "XDFHEADER.REGION",
            "regionflags",
        )?,
        name: node.attr_any(&["name"]),
        description: node.attr_any(&["desc", "description"]),
    })
}

fn parse_optional_u64_attr(
    node: &XmlNode,
    name: &str,
    object: &str,
    field: &str,
) -> Result<Option<u64>, XdfError> {
    node.attr_any(&[name])
        .map(|value| parse_u64(object, field, &value))
        .transpose()
}

fn parse_optional_u32_attr(
    node: &XmlNode,
    name: &str,
    object: &str,
    field: &str,
) -> Result<Option<u32>, XdfError> {
    parse_optional_u64_attr(node, name, object, field)?.map_or(Ok(None), |value| {
        u32::try_from(value)
            .map(Some)
            .map_err(|_| invalid_field(object, field, value.to_string(), "value exceeds u32"))
    })
}

fn parse_optional_bool_attr(
    node: &XmlNode,
    name: &str,
    object: &str,
    field: &str,
) -> Result<Option<bool>, XdfError> {
    node.attr_any(&[name])
        .map(|value| parse_bool_strict(object, field, &value))
        .transpose()
}

fn parse_bool_strict(object: &str, field: &str, value: &str) -> Result<bool, XdfError> {
    match normalize_text(value).as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(invalid_field(
            object,
            field,
            value.to_string(),
            "expected true, false, 1, or 0",
        )),
    }
}

fn parse_category_memberships(
    node: &XmlNode,
    categories: &[XdfCategory],
    object: &str,
    reference_mode: CategoryReferenceMode,
    diagnostics: &mut Vec<XdfDiagnostic>,
) -> Result<Vec<CategoryMembership>, XdfError> {
    let mut category_nodes = Vec::new();
    node.descendants_named("CATEGORYMEM", &mut category_nodes);
    let mut memberships = Vec::with_capacity(category_nodes.len());
    let mut slots = BTreeSet::new();
    for (ordinal, category_node) in category_nodes.into_iter().enumerate() {
        let slot = category_node
            .attr_any(&["index"])
            .map(|value| parse_number(object, "category.index", &value))
            .transpose()?
            .unwrap_or(ordinal);
        if !slots.insert(slot) {
            diagnostics.push(XdfDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "duplicate-category-slot".to_string(),
                path: format!("{object}/CATEGORYMEM[{slot}]"),
                message: format!("category membership slot {slot} is repeated"),
            });
        }
        let category_value = category_node.attr_any(&["category"]).ok_or_else(|| {
            invalid_field(
                object,
                "categorymem.category",
                "<missing>".to_string(),
                "category membership needs a category index",
            )
        })?;
        let category_index = parse_u64(object, "categorymem.category", &category_value)?;
        let resolved_category = match reference_mode {
            CategoryReferenceMode::DeclaredIndex => categories
                .iter()
                .find(|category| category.index == category_index),
            CategoryReferenceMode::OneBasedPosition => category_index
                .checked_sub(1)
                .and_then(|position| usize::try_from(position).ok())
                .and_then(|position| categories.get(position)),
        };
        let resolved_category_index = resolved_category.map(|category| category.index);
        let category_name = resolved_category.map(|category| category.name.clone());
        if category_name.is_none() {
            diagnostics.push(XdfDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "missing-category".to_string(),
                path: format!("{object}/CATEGORYMEM[{slot}]"),
                message: format!("category index {category_index:#x} is not declared in XDFHEADER"),
            });
        }
        memberships.push(CategoryMembership {
            slot,
            category_index,
            resolved_category_index,
            category_name,
        });
    }
    Ok(memberships)
}

fn detect_category_reference_mode(
    categories: &[XdfCategory],
    parameter_nodes: &[&XmlNode],
) -> Result<CategoryReferenceMode, XdfError> {
    if categories.is_empty() {
        return Ok(CategoryReferenceMode::DeclaredIndex);
    }
    let contiguous_zero_based = categories.len() <= (u64::MAX as usize)
        && categories
            .iter()
            .enumerate()
            .all(|(position, category)| category.index == position as u64);
    if !contiguous_zero_based {
        return Ok(CategoryReferenceMode::DeclaredIndex);
    }

    let maximum_index = categories.len() as u64 - 1;
    let mut values = Vec::new();
    for node in parameter_nodes {
        let mut category_nodes = Vec::new();
        node.descendants_named("CATEGORYMEM", &mut category_nodes);
        for category_node in category_nodes {
            if let Some(value) = category_node.attr_any(&["category"]) {
                values.push(parse_u64("CATEGORYMEM", "category", &value)?);
            }
        }
    }

    let decisive_one_based_reference = values.iter().any(|value| {
        *value > maximum_index
            && value
                .checked_sub(1)
                .is_some_and(|position| position <= maximum_index)
    });
    if decisive_one_based_reference {
        Ok(CategoryReferenceMode::OneBasedPosition)
    } else {
        Ok(CategoryReferenceMode::DeclaredIndex)
    }
}

fn collect_auxiliary_objects(
    root: &XmlNode,
    diagnostics: &mut Vec<XdfDiagnostic>,
) -> Vec<XdfAuxiliaryObject> {
    let mut auxiliary_objects = Vec::new();
    for (ordinal, child) in root.children.iter().enumerate() {
        let kind = match child.name.as_str() {
            "XDFFUNCTION" => Some(XdfAuxiliaryKind::Function),
            "XDFPATCH" => Some(XdfAuxiliaryKind::Patch),
            "XDFCHECKSUM" => Some(XdfAuxiliaryKind::Checksum),
            "XDFHEADER" | "XDFLOCK" | "XDFPAGE" | "XDFCATEGORY" | "XDFCONSTANT" | "XDFTABLE"
            | "XDFBITFIELD" | "XDFFLAG" => None,
            _ if child.name.starts_with("XDF") => Some(XdfAuxiliaryKind::Unknown),
            _ => None,
        };
        let Some(kind) = kind else {
            continue;
        };
        let path = format!("/{}[{}]", child.name, ordinal);
        let title = lookup_string(child, None, &[], &["TITLE", "NAME"]).unwrap_or_default();
        let description = lookup_string(child, None, &[], &["DESCRIPTION"]).unwrap_or_default();
        let unique_id = lookup_string(
            child,
            None,
            &["uniqueid", "unique_id", "uid"],
            &["UNIQUEID"],
        );
        if kind == XdfAuxiliaryKind::Unknown {
            diagnostics.push(XdfDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "unknown-object".to_string(),
                path: path.clone(),
                message: format!(
                    "unsupported top-level XDF object <{}> was retained as metadata",
                    child.name
                ),
            });
        }
        auxiliary_objects.push(XdfAuxiliaryObject {
            kind,
            source_name: child.name.clone(),
            unique_id,
            title,
            description,
            path,
            attributes: child.attrs.clone(),
        });
    }
    auxiliary_objects
}

fn normalize_parameter_identities(parameters: &mut [ParameterDefinition]) -> Result<(), XdfError> {
    let mut unique_id_counts = BTreeMap::new();
    for parameter in parameters.iter() {
        if let Some(unique_id) = parameter.unique_id.as_deref() {
            *unique_id_counts
                .entry(normalize_text(unique_id))
                .or_insert(0usize) += 1;
        }
    }

    let mut identities = BTreeSet::new();
    let mut semantic_occurrences = BTreeMap::new();
    for parameter in parameters.iter_mut() {
        let repeated_unique_id = parameter
            .unique_id
            .as_deref()
            .and_then(|unique_id| unique_id_counts.get(&normalize_text(unique_id)))
            .copied()
            .unwrap_or(0)
            > 1;
        let address_like_unique_id = parameter
            .unique_id
            .as_deref()
            .and_then(|unique_id| parse_number("uniqueid", "uniqueid", unique_id).ok())
            == Some(parameter.layout.address);
        let uses_semantic_identity = repeated_unique_id || address_like_unique_id;
        let base_identity = if uses_semantic_identity {
            semantic_identity(
                parameter.kind,
                None,
                &parameter.title,
                parameter.category.as_deref(),
                &parameter.layout,
                parameter.conversion.as_deref(),
                parameter.bit_offset,
                parameter.bit_width,
                parameter.bit_mask,
                &parameter.axes,
            )
        } else {
            parameter.semantic_id.clone()
        };

        let occurrence = semantic_occurrences
            .entry(base_identity.clone())
            .or_insert(0usize);
        *occurrence += 1;
        let mut candidate = if *occurrence == 1 {
            base_identity.clone()
        } else if uses_semantic_identity {
            format!("{base_identity}:alias:{occurrence}")
        } else {
            return Err(XdfError::DuplicateIdentity {
                identity: base_identity,
            });
        };
        while !identities.insert(candidate.clone()) {
            if !uses_semantic_identity {
                return Err(XdfError::DuplicateIdentity {
                    identity: candidate,
                });
            }
            *occurrence += 1;
            candidate = format!("{base_identity}:alias:{occurrence}");
        }
        parameter.semantic_id = candidate;
    }
    Ok(())
}

fn collect_parameter_nodes<'a>(node: &'a XmlNode, output: &mut Vec<&'a XmlNode>) {
    for child in &node.children {
        match child.name.as_str() {
            "XDFCONSTANT" | "XDFTABLE" | "XDFBITFIELD" | "XDFFLAG" => output.push(child),
            _ => collect_parameter_nodes(child, output),
        }
    }
}

fn parameter_embedded_data(node: &XmlNode) -> Option<&XmlNode> {
    if let Some(data) = node.children.iter().find(|child| child.name == "XDFDATA") {
        if let Some(embedded) = data.descendant("EMBEDDEDDATA") {
            return Some(embedded);
        }
    }
    if let Some(embedded) = node
        .children
        .iter()
        .find(|child| child.name == "EMBEDDEDDATA")
    {
        return Some(embedded);
    }
    if node.name == "XDFTABLE" {
        if let Some(axis) = node.children.iter().find(|child| {
            child.name == "XDFAXIS"
                && child
                    .attr_any(&["id", "axisid", "name"])
                    .map(|value| normalize_text(&value))
                    .as_deref()
                    == Some("z")
        }) {
            if let Some(embedded) = axis.descendant("EMBEDDEDDATA") {
                return Some(embedded);
            }
        }
    }
    node.descendant("EMBEDDEDDATA")
}

fn count_unknown_elements(node: &XmlNode) -> usize {
    let known = known_element_names();
    node.children
        .iter()
        .map(|child| {
            usize::from(!known.contains(child.name.as_str())) + count_unknown_elements(child)
        })
        .sum()
}

fn known_element_names() -> BTreeSet<&'static str> {
    [
        "XDFHEADER",
        "XDFLOCK",
        "XDFPAGE",
        "XDFCATEGORY",
        "CATEGORY",
        "XDFCONSTANT",
        "XDFTABLE",
        "XDFBITFIELD",
        "XDFFLAG",
        "XDFFUNCTION",
        "XDFPATCH",
        "XDFCHECKSUM",
        "XDFDATA",
        "EMBEDDEDDATA",
        "XDFAXIS",
        "EMBEDINFO",
        "DEFAULTS",
        "BASEOFFSET",
        "REGION",
        "AUTHOR",
        "FLAGS",
        "DEFTITLE",
        "XDFCONVERT",
        "MATH",
        "TITLE",
        "NAME",
        "DESCRIPTION",
        "ADDRESS",
        "DATATYPE",
        "ELEMENTSIZEBITS",
        "MMEDELEMENTSIZEBITS",
        "MMEDADDRESS",
        "MMEDROWCOUNT",
        "MMEDCOLCOUNT",
        "ROWCOUNT",
        "COLCOUNT",
        "ROWS",
        "COLUMNS",
        "INDEXCOUNT",
        "COUNT",
        "MMEDMAJORSTRIDEBITS",
        "MMEDMINORSTRIDEBITS",
        "MAJORSTRIDEBITS",
        "MINORSTRIDEBITS",
        "ROWSTRIDEBITS",
        "COLUMNSTRIDEBITS",
        "SIGNED",
        "ENDIANNESS",
        "BYTEORDER",
        "BITOFFSET",
        "BITWIDTH",
        "BITLENGTH",
        "MASK",
        "BITMASK",
        "TYPEFLAGS",
        "MMEDTYPEFLAGS",
        "LABEL",
        "DALINK",
        "UNITS",
        "UNIT",
        "UNITTYPE",
        "DECIMALPL",
        "DECIMALPLACES",
        "MIN",
        "MINIMUM",
        "MAX",
        "MAXIMUM",
        "OUTPUTTYPE",
    ]
    .into_iter()
    .collect()
}

const SIGNED_STORAGE_FLAG: u32 = 0x01;
const LSB_FIRST_STORAGE_FLAG: u32 = 0x02;
const COLUMN_MAJOR_STORAGE_FLAG: u32 = 0x04;
const BINARY32_STORAGE_FLAG: u32 = 0x10000;
const KNOWN_STORAGE_FLAGS: u32 = SIGNED_STORAGE_FLAG
    | LSB_FIRST_STORAGE_FLAG
    | COLUMN_MAJOR_STORAGE_FLAG
    | BINARY32_STORAGE_FLAG;

fn resolve_storage(
    node: &XmlNode,
    preferred: Option<&XmlNode>,
    width_bits: usize,
    fallback_signed: bool,
    fallback_endianness: Endianness,
    fallback_float: bool,
    _raw_type: Option<&str>,
    object: &str,
    diagnostics: &mut Vec<XdfDiagnostic>,
) -> Result<StorageSpec, XdfError> {
    let element_size_bits = u32::try_from(width_bits).map_err(|_| {
        invalid_field(
            object,
            "element_width_bits",
            width_bits.to_string(),
            "width exceeds u32",
        )
    })?;
    let flags = lookup_string(
        node,
        preferred,
        &["mmedtypeflags", "typeflags"],
        &["MMEDTYPEFLAGS", "TYPEFLAGS"],
    )
    .map(|value| {
        let parsed = parse_u64(object, "type_flags", &value)?;
        u32::try_from(parsed)
            .map_err(|_| invalid_field(object, "type_flags", value, "storage flags exceed u32"))
    })
    .transpose()?;

    let (signed, byte_order, numeric_kind, column_major, raw_type_flags, unknown_type_flags) =
        if let Some(flags) = flags {
            let unknown_type_flags = flags & !KNOWN_STORAGE_FLAGS;
            if unknown_type_flags != 0 {
                diagnostics.push(XdfDiagnostic {
                    severity: DiagnosticSeverity::Warning,
                    code: "unknown-storage-flags".to_string(),
                    path: format!("{object}/EMBEDDEDDATA.@mmedtypeflags"),
                    message: format!(
                        "storage flags contain unknown bits {unknown_type_flags:#x}; known bits are preserved"
                    ),
                });
            }
            let signed = flags & SIGNED_STORAGE_FLAG != 0;
            let byte_order = if flags & LSB_FIRST_STORAGE_FLAG != 0 {
                Endianness::Little
            } else {
                Endianness::Big
            };
            let column_major = flags & COLUMN_MAJOR_STORAGE_FLAG != 0;
            let numeric_kind = if flags & BINARY32_STORAGE_FLAG != 0 {
                if width_bits == 32 {
                    NumericKind::Ieee754Binary32
                } else {
                    diagnostics.push(XdfDiagnostic {
                        severity: DiagnosticSeverity::Warning,
                        code: "unsupported-storage".to_string(),
                        path: format!("{object}/EMBEDDEDDATA.@mmedtypeflags"),
                        message: format!(
                            "IEEE-754 binary32 storage marker requires a 32-bit element, got {width_bits} bits"
                        ),
                    });
                    NumericKind::Unsupported
                }
            } else {
                NumericKind::Integer
            };
            (
                signed,
                byte_order,
                numeric_kind,
                column_major,
                flags,
                unknown_type_flags,
            )
        } else {
            let signed = lookup_bool(node, preferred, &["signed", "issigned"], &["SIGNED"])
                .unwrap_or(fallback_signed);
            let byte_order = lookup_endianness(
                node,
                preferred,
                &["endianness", "byteorder", "mmedbyteorder"],
                &["ENDIANNESS", "BYTEORDER"],
            )
            .unwrap_or(fallback_endianness);
            let explicit_float = lookup_bool(
                node,
                preferred,
                &["float", "isfloat", "floating"],
                &["FLOAT"],
            );
            let numeric_kind = if explicit_float.unwrap_or(fallback_float) {
                if width_bits == 32 {
                    NumericKind::Ieee754Binary32
                } else {
                    diagnostics.push(XdfDiagnostic {
                        severity: DiagnosticSeverity::Warning,
                        code: "unsupported-storage".to_string(),
                        path: format!("{object}/storage"),
                        message: format!(
                            "floating-point storage requires a supported 32-bit element, got {width_bits} bits"
                        ),
                    });
                    NumericKind::Unsupported
                }
            } else {
                NumericKind::Integer
            };
            (signed, byte_order, numeric_kind, false, 0, 0)
        };

    Ok(StorageSpec {
        element_size_bits,
        signed,
        byte_order,
        numeric_kind,
        column_major,
        raw_type_flags,
        unknown_type_flags,
    })
}

fn diagnose_region_placement(
    header: &XdfHeader,
    object: &str,
    xdf_address: u64,
    width_bits: usize,
    diagnostics: &mut Vec<XdfDiagnostic>,
) {
    let Some(object_size) = width_bits.checked_add(7).map(|value| value / 8) else {
        return;
    };
    let Some(object_end) = xdf_address.checked_add(object_size as u64) else {
        diagnostics.push(XdfDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code: "outside-region".to_string(),
            path: object.to_string(),
            message: "object address plus element width overflows XDF address space".to_string(),
        });
        return;
    };
    let declared_regions = header
        .regions
        .iter()
        .filter_map(|region| Some((region.start_address?, region.size?)))
        .collect::<Vec<_>>();
    if declared_regions.is_empty() {
        return;
    }
    let inside_region = declared_regions.iter().any(|(start, size)| {
        start
            .checked_add(*size)
            .map(|end| xdf_address >= *start && object_end <= end)
            .unwrap_or(false)
    });
    if !inside_region {
        diagnostics.push(XdfDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code: "outside-region".to_string(),
            path: object.to_string(),
            message: format!(
                "object range [{xdf_address:#x}, {object_end:#x}) is outside all declared XDF regions"
            ),
        });
    }
}

fn parse_parameter(
    node: &XmlNode,
    ordinal: usize,
    header: &XdfHeader,
    categories: &[XdfCategory],
    category_reference_mode: CategoryReferenceMode,
    diagnostics: &mut Vec<XdfDiagnostic>,
) -> Result<ParameterDefinition, XdfError> {
    let kind = match node.name.as_str() {
        "XDFCONSTANT" => ParameterKind::Constant,
        "XDFTABLE" => ParameterKind::Table,
        "XDFBITFIELD" => ParameterKind::BitField,
        "XDFFLAG" => ParameterKind::Flag,
        _ => {
            return Err(XdfError::Xml {
                position: 0,
                message: format!("unsupported parameter element <{}>", node.name),
            })
        }
    };
    let unique_id = lookup_string(node, None, &["uniqueid", "unique_id", "uid"], &["UNIQUEID"]);
    let object = unique_id
        .clone()
        .unwrap_or_else(|| format!("{}#{}", kind.as_str(), ordinal));
    let title = lookup_string(node, None, &[], &["TITLE", "NAME"])
        .unwrap_or_else(|| format!("{} {}", kind.as_str(), ordinal + 1));
    let description = lookup_string(node, None, &[], &["DESCRIPTION"]).unwrap_or_default();
    let mut category = lookup_string(node, None, &["category", "categorypath"], &["CATEGORY"]);
    let category_memberships = parse_category_memberships(
        node,
        categories,
        &object,
        category_reference_mode,
        diagnostics,
    )?;
    if category.is_none() {
        category = category_memberships
            .first()
            .and_then(|membership| membership.category_name.clone())
            .or_else(|| {
                category_memberships
                    .first()
                    .map(|membership| membership.category_index.to_string())
            });
    }
    let embedded = parameter_embedded_data(node);
    let address_value = lookup_string(
        node,
        embedded,
        &["mmedaddress", "address", "addr"],
        &["MMEDADDRESS", "ADDRESS"],
    )
    .ok_or_else(|| XdfError::InvalidField {
        object: object.clone(),
        field: "address".to_string(),
        value: "<missing>".to_string(),
        message: "every editable XDF object needs a byte address".to_string(),
    })?;
    let xdf_address = parse_u64(&object, "address", &address_value)?;
    let address = header.base_offset.translate(xdf_address)?;
    let width_value = lookup_string(
        node,
        embedded,
        &["mmedelementsizebits", "elementsizebits", "widthbits"],
        &["MMEDELEMENTSIZEBITS", "ELEMENTSIZEBITS"],
    )
    .unwrap_or_else(|| header.defaults.data_size_bits.unwrap_or(8).to_string());
    let width_bits = parse_number(&object, "element_width_bits", &width_value)?;
    if width_bits == 0 {
        return Err(invalid_field(
            &object,
            "element_width_bits",
            width_value,
            "width must be greater than zero",
        ));
    }
    diagnose_region_placement(header, &object, xdf_address, width_bits, diagnostics);

    let raw_type = lookup_string(
        node,
        embedded,
        &["datatype", "mmedtype", "type"],
        &["DATATYPE"],
    );
    let fallback_signed = header
        .defaults
        .signed
        .unwrap_or_else(|| raw_type.as_deref().map(is_signed_type).unwrap_or(false));
    let fallback_endianness = header
        .defaults
        .lsb_first
        .map(|value| {
            if value {
                Endianness::Little
            } else {
                Endianness::Big
            }
        })
        .unwrap_or(Endianness::Little);
    let fallback_float = header
        .defaults
        .float
        .unwrap_or_else(|| raw_type.as_deref().map(is_float_type).unwrap_or(false));
    let storage = resolve_storage(
        node,
        embedded,
        width_bits,
        fallback_signed,
        fallback_endianness,
        fallback_float,
        raw_type.as_deref(),
        &object,
        diagnostics,
    )?;
    let signed = storage.signed;
    let endianness = storage.byte_order;

    let axes = parse_axes(node, &storage, &object, &header.base_offset, diagnostics)?;
    let explicit_rows = lookup_number(
        node,
        embedded,
        &["mmedrowcount", "rowcount", "rows"],
        &["MMEDROWCOUNT", "ROWCOUNT", "ROWS"],
        &object,
        "rows",
    )?;
    let explicit_columns = lookup_number(
        node,
        embedded,
        &["mmedcolcount", "colcount", "columns"],
        &["MMEDCOLCOUNT", "COLCOUNT", "COLUMNS"],
        &object,
        "columns",
    )?;
    let dimensions = resolve_dimensions(kind, explicit_rows, explicit_columns, &axes)?;
    if dimensions.rows == 0 || dimensions.columns == 0 {
        return Err(invalid_field(
            &object,
            "dimensions",
            format!("{}x{}", dimensions.rows, dimensions.columns),
            "dimensions must be greater than zero",
        ));
    }

    let explicit_row_stride_bits = lookup_signed_number(
        node,
        embedded,
        &[
            "mmedmajorstridebits",
            "majorstridebits",
            "rowstridebits",
            "row_stride_bits",
        ],
        &["MMEDMAJORSTRIDEBITS", "MAJORSTRIDEBITS", "ROWSTRIDEBITS"],
        &object,
        "row_stride_bits",
    )?;
    let explicit_column_stride_bits = lookup_signed_number(
        node,
        embedded,
        &[
            "mmedminorstridebits",
            "minorstridebits",
            "columnstridebits",
            "column_stride_bits",
        ],
        &["MMEDMINORSTRIDEBITS", "MINORSTRIDEBITS", "COLUMNSTRIDEBITS"],
        &object,
        "column_stride_bits",
    )?;
    let packed_row_stride_bits = if storage.column_major {
        checked_stride_from_usize(&object, width_bits)?
    } else {
        checked_stride_from_usize(
            &object,
            width_bits
                .checked_mul(dimensions.columns)
                .ok_or_else(|| XdfError::Overflow {
                    object: object.clone(),
                    message: "packed row stride overflows usize".to_string(),
                })?,
        )?
    };
    let packed_column_stride_bits = if storage.column_major {
        checked_stride_from_usize(
            &object,
            width_bits
                .checked_mul(dimensions.rows)
                .ok_or_else(|| XdfError::Overflow {
                    object: object.clone(),
                    message: "packed column stride overflows usize".to_string(),
                })?,
        )?
    } else {
        checked_stride_from_usize(&object, width_bits)?
    };
    let (row_stride_bits, column_stride_bits) =
        if explicit_row_stride_bits == Some(0) && explicit_column_stride_bits == Some(0) {
            (packed_row_stride_bits, packed_column_stride_bits)
        } else {
            (
                explicit_row_stride_bits.unwrap_or(packed_row_stride_bits),
                explicit_column_stride_bits.unwrap_or(packed_column_stride_bits),
            )
        };
    let range = calculate_range(
        &object,
        address,
        width_bits,
        dimensions,
        row_stride_bits,
        column_stride_bits,
    )?;
    let conversion = parse_conversion(node).or_else(|| {
        axes.iter()
            .find(|axis| normalize_text(&axis.id) == "z")
            .and_then(|axis| axis.conversion.clone())
    });
    let raw_bit_mask = lookup_string(
        node,
        embedded,
        &["mask", "bitmask", "bit_mask"],
        &["MASK", "BITMASK"],
    );
    let bit_mask = raw_bit_mask
        .as_deref()
        .map(|value| parse_u64(&object, "bit_mask", value))
        .transpose()?;
    let mut bit_offset = lookup_number(
        node,
        embedded,
        &["bitoffset", "bit_offset"],
        &["BITOFFSET"],
        &object,
        "bit_offset",
    )?;
    let mut bit_width = lookup_number(
        node,
        embedded,
        &["bitwidth", "bit_width", "bitlength", "bit_length"],
        &["BITWIDTH", "BITLENGTH"],
        &object,
        "bit_width",
    )?;
    if let Some(mask) = bit_mask {
        if let Some((mask_offset, mask_width)) = contiguous_mask_parts(mask) {
            if bit_offset.is_none() {
                bit_offset = Some(mask_offset);
            }
            if bit_width.is_none() {
                bit_width = Some(mask_width);
            }
        } else if kind == ParameterKind::Flag {
            diagnostics.push(XdfDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "non-contiguous-mask".to_string(),
                path: format!("{object}/MASK"),
                message: format!("flag mask {mask:#x} is not a contiguous bit range"),
            });
        }
    }
    let layout = DataLayout {
        address,
        element_width_bits: width_bits,
        dimensions,
        row_stride_bits,
        column_stride_bits,
        signed,
        endianness,
        storage,
        range,
    };
    let semantic_id = semantic_identity(
        kind,
        unique_id.as_deref(),
        &title,
        category.as_deref(),
        &layout,
        conversion.as_deref(),
        bit_offset,
        bit_width,
        bit_mask,
        &axes,
    );
    Ok(ParameterDefinition {
        semantic_id,
        unique_id,
        kind,
        title,
        description,
        category,
        category_memberships,
        raw_type,
        layout,
        conversion,
        bit_offset,
        bit_width,
        axes,
        bit_mask,
        raw_bit_mask,
    })
}

fn parse_axes(
    node: &XmlNode,
    default_storage: &StorageSpec,
    object: &str,
    base_offset: &XdfBaseOffset,
    diagnostics: &mut Vec<XdfDiagnostic>,
) -> Result<Vec<AxisDefinition>, XdfError> {
    let mut axis_nodes = Vec::new();
    node.descendants_named("XDFAXIS", &mut axis_nodes);
    let mut axes = Vec::with_capacity(axis_nodes.len());
    for (ordinal, axis) in axis_nodes.into_iter().enumerate() {
        let embedded = axis.descendant("EMBEDDEDDATA");
        let id = axis
            .attr_any(&["id", "axisid", "name"])
            .or_else(|| Some(format!("axis{}", ordinal + 1)))
            .unwrap_or_default();
        let title =
            lookup_string(axis, None, &[], &["TITLE", "NAME"]).unwrap_or_else(|| id.clone());
        let count = lookup_number(
            axis,
            embedded,
            &["indexcount", "count", "length"],
            &["INDEXCOUNT", "COUNT"],
            object,
            "axis_count",
        )?
        .unwrap_or(1);
        if count == 0 {
            return Err(invalid_field(
                object,
                "axis_count",
                "0".to_string(),
                "axis count must be greater than zero",
            ));
        }
        let address = match lookup_string(
            axis,
            embedded,
            &["mmedaddress", "address", "addr"],
            &["MMEDADDRESS", "ADDRESS"],
        ) {
            Some(value) => {
                Some(base_offset.translate(parse_u64(object, "axis_address", &value)?)?)
            }
            None => None,
        };
        let width_bits = match lookup_string(
            axis,
            embedded,
            &["mmedelementsizebits", "elementsizebits", "widthbits"],
            &["MMEDELEMENTSIZEBITS", "ELEMENTSIZEBITS"],
        ) {
            Some(value) => parse_number(object, "axis_width_bits", &value)?,
            None => default_storage.element_size_bits as usize,
        };
        if width_bits == 0 {
            return Err(invalid_field(
                object,
                "axis_width_bits",
                "0".to_string(),
                "axis width must be greater than zero",
            ));
        }
        let storage = if address.is_some() {
            Some(resolve_storage(
                axis,
                embedded,
                width_bits,
                default_storage.signed,
                default_storage.byte_order,
                default_storage.numeric_kind == NumericKind::Ieee754Binary32,
                None,
                object,
                diagnostics,
            )?)
        } else {
            None
        };
        let signed = storage
            .map(|storage| storage.signed)
            .unwrap_or(default_storage.signed);
        let endianness = storage
            .map(|storage| storage.byte_order)
            .unwrap_or(default_storage.byte_order);
        let metadata = parse_axis_metadata(axis, object)?;
        let labels = parse_axis_labels(axis, object, diagnostics)?;
        let links = parse_axis_links(axis, object, diagnostics)?;
        let embed_info = parse_embed_info(axis, object)?;
        let conversion = parse_conversion(axis);
        let major_stride_bits = lookup_signed_number(
            axis,
            embedded,
            &["mmedmajorstridebits", "majorstridebits", "rowstridebits"],
            &["MMEDMAJORSTRIDEBITS", "MAJORSTRIDEBITS", "ROWSTRIDEBITS"],
            object,
            "axis_major_stride_bits",
        )?;
        let minor_stride_bits = lookup_signed_number(
            axis,
            embedded,
            &["mmedminorstridebits", "minorstridebits", "columnstridebits"],
            &["MMEDMINORSTRIDEBITS", "MINORSTRIDEBITS", "COLUMNSTRIDEBITS"],
            object,
            "axis_minor_stride_bits",
        )?;
        let stride_bits = match (major_stride_bits, minor_stride_bits) {
            (Some(value), _) if value != 0 => value,
            (_, Some(value)) if value != 0 => value,
            _ => checked_stride_from_usize(object, width_bits)?,
        };
        let range = address
            .map(|start| {
                calculate_range(
                    object,
                    start,
                    width_bits,
                    Dimensions {
                        rows: 1,
                        columns: count,
                    },
                    0,
                    stride_bits,
                )
            })
            .transpose()?;
        axes.push(AxisDefinition {
            id,
            title,
            count,
            address,
            element_width_bits: width_bits,
            stride_bits,
            signed,
            endianness,
            storage,
            metadata,
            labels,
            links,
            embed_info,
            conversion,
            range,
        });
    }
    Ok(axes)
}

fn parse_axis_metadata(axis: &XmlNode, object: &str) -> Result<AxisMetadata, XdfError> {
    Ok(AxisMetadata {
        units: lookup_string(axis, None, &["units", "unit"], &["UNITS", "UNIT"]),
        unit_type: lookup_u32(
            axis,
            None,
            &["unittype", "unit_type"],
            &["UNITTYPE"],
            object,
            "axis_unit_type",
        )?,
        decimal_places: lookup_u32(
            axis,
            None,
            &["decimalpl", "decimalplaces", "decimal_places"],
            &["DECIMALPL", "DECIMALPLACES"],
            object,
            "axis_decimal_places",
        )?,
        min: lookup_f64(
            axis,
            None,
            &["min", "minimum"],
            &["MIN", "MINIMUM"],
            object,
            "axis_min",
        )?,
        max: lookup_f64(
            axis,
            None,
            &["max", "maximum"],
            &["MAX", "MAXIMUM"],
            object,
            "axis_max",
        )?,
        output_type: lookup_u32(
            axis,
            None,
            &["outputtype", "output_type"],
            &["OUTPUTTYPE"],
            object,
            "axis_output_type",
        )?,
    })
}

fn parse_axis_labels(
    axis: &XmlNode,
    object: &str,
    diagnostics: &mut Vec<XdfDiagnostic>,
) -> Result<Vec<AxisLabel>, XdfError> {
    let mut label_nodes = Vec::new();
    axis.descendants_named("LABEL", &mut label_nodes);
    let mut labels = Vec::with_capacity(label_nodes.len());
    let mut indices = BTreeSet::new();
    for (ordinal, label) in label_nodes.into_iter().enumerate() {
        let index = label
            .attr_any(&["index"])
            .map(|value| parse_number(object, "axis_label.index", &value))
            .transpose()?
            .unwrap_or(ordinal);
        if !indices.insert(index) {
            diagnostics.push(XdfDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "duplicate-label-index".to_string(),
                path: format!("{object}/XDFAXIS/LABEL[{index}]"),
                message: format!("axis label index {index} is repeated"),
            });
        }
        let value = if !label.text_value().is_empty() {
            label.text_value()
        } else {
            label
                .attr_any(&["value", "text", "name"])
                .unwrap_or_default()
        };
        labels.push(AxisLabel { index, value });
    }
    let mut sorted_indices = indices.into_iter().collect::<Vec<_>>();
    sorted_indices.sort_unstable();
    if sorted_indices
        .windows(2)
        .any(|window| window[1] > window[0].saturating_add(1))
    {
        diagnostics.push(XdfDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code: "label-gap".to_string(),
            path: format!("{object}/XDFAXIS/LABEL"),
            message: "axis label indices contain one or more gaps".to_string(),
        });
    }
    Ok(labels)
}

fn parse_axis_links(
    axis: &XmlNode,
    object: &str,
    diagnostics: &mut Vec<XdfDiagnostic>,
) -> Result<Vec<AxisLink>, XdfError> {
    let mut link_nodes = Vec::new();
    axis.descendants_named("DALINK", &mut link_nodes);
    let mut links = Vec::with_capacity(link_nodes.len());
    for (ordinal, link) in link_nodes.into_iter().enumerate() {
        let index = link
            .attr_any(&["index"])
            .map(|value| parse_number(object, "axis_link.index", &value))
            .transpose()?;
        let object_id_hash =
            link.attr_any(&["objectid", "objectidhash", "object_id_hash", "hash", "id"]);
        if object_id_hash.is_some() {
            diagnostics.push(XdfDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "unresolved-axis-link".to_string(),
                path: format!("{object}/XDFAXIS/DALINK[{ordinal}]"),
                message: "DALINK object reference was retained but could not be resolved by the normalized model".to_string(),
            });
        }
        links.push(AxisLink {
            index,
            object_id_hash,
        });
    }
    Ok(links)
}

fn parse_embed_info(axis: &XmlNode, object: &str) -> Result<Option<EmbedInfo>, XdfError> {
    let Some(embed_info) = axis.descendant("EMBEDINFO") else {
        return Ok(None);
    };
    let type_code = embed_info
        .attr_any(&["type", "typecode"])
        .map(|value| parse_u64(object, "embedinfo.type", &value))
        .transpose()?;
    Ok(Some(EmbedInfo {
        type_code,
        attributes: embed_info.attrs.clone(),
    }))
}

fn resolve_dimensions(
    kind: ParameterKind,
    rows: Option<usize>,
    columns: Option<usize>,
    axes: &[AxisDefinition],
) -> Result<Dimensions, XdfError> {
    if kind != ParameterKind::Table {
        return Ok(Dimensions {
            rows: rows.unwrap_or(1),
            columns: columns.unwrap_or(1),
        });
    }
    let mut resolved_rows = rows.unwrap_or(1);
    let mut resolved_columns = columns.unwrap_or(1);
    if rows.is_none() {
        if let Some(axis) = axes.iter().find(|axis| {
            let id = axis.id.to_ascii_lowercase();
            id == "y" || id == "row" || id == "rows"
        }) {
            resolved_rows = axis.count;
        } else if axes.len() > 1 {
            resolved_rows = axes[1].count;
        }
    }
    if columns.is_none() {
        if let Some(axis) = axes.iter().find(|axis| {
            let id = axis.id.to_ascii_lowercase();
            id == "x" || id == "column" || id == "columns"
        }) {
            resolved_columns = axis.count;
        } else if let Some(axis) = axes.first() {
            resolved_columns = axis.count;
        }
    }
    Ok(Dimensions {
        rows: resolved_rows,
        columns: resolved_columns,
    })
}

fn checked_stride_from_usize(object: &str, value: usize) -> Result<i64, XdfError> {
    i64::try_from(value).map_err(|_| XdfError::Overflow {
        object: object.to_string(),
        message: "packed stride exceeds signed 64-bit range".to_string(),
    })
}

fn calculate_range(
    object: &str,
    address: usize,
    width_bits: usize,
    dimensions: Dimensions,
    row_stride_bits: i64,
    column_stride_bits: i64,
) -> Result<ByteRange, XdfError> {
    let last_row = dimensions.rows.saturating_sub(1) as i128;
    let last_column = dimensions.columns.saturating_sub(1) as i128;
    let base_bit = (address as i128) * 8;
    let row_stride_bits = row_stride_bits as i128;
    let column_stride_bits = column_stride_bits as i128;
    let row_offset = last_row
        .checked_mul(row_stride_bits)
        .ok_or_else(|| XdfError::Overflow {
            object: object.to_string(),
            message: "row stride overflows signed arithmetic".to_string(),
        })?;
    let column_offset =
        last_column
            .checked_mul(column_stride_bits)
            .ok_or_else(|| XdfError::Overflow {
                object: object.to_string(),
                message: "column stride overflows signed arithmetic".to_string(),
            })?;
    let combined_offset =
        row_offset
            .checked_add(column_offset)
            .ok_or_else(|| XdfError::Overflow {
                object: object.to_string(),
                message: "combined strides overflow signed arithmetic".to_string(),
            })?;
    let offsets = [0, row_offset, column_offset, combined_offset];
    let min_offset = offsets.iter().copied().min().unwrap_or(0);
    let max_offset = offsets.iter().copied().max().unwrap_or(0);
    let start_bit = base_bit
        .checked_add(min_offset)
        .ok_or_else(|| XdfError::Overflow {
            object: object.to_string(),
            message: "mapped range start overflows signed arithmetic".to_string(),
        })?;
    let end_bit = base_bit
        .checked_add(max_offset)
        .and_then(|value| value.checked_add(width_bits as i128))
        .ok_or_else(|| XdfError::Overflow {
            object: object.to_string(),
            message: "mapped range end overflows signed arithmetic".to_string(),
        })?;
    checked_bit_bounds(object, start_bit, end_bit)?;
    let start = usize::try_from(start_bit / 8).map_err(|_| XdfError::Overflow {
        object: object.to_string(),
        message: "mapped range start exceeds usize".to_string(),
    })?;
    let end = usize::try_from((end_bit + 7) / 8).map_err(|_| XdfError::Overflow {
        object: object.to_string(),
        message: "mapped range end exceeds usize".to_string(),
    })?;
    Ok(ByteRange { start, end })
}

fn semantic_identity(
    kind: ParameterKind,
    unique_id: Option<&str>,
    title: &str,
    category: Option<&str>,
    layout: &DataLayout,
    conversion: Option<&str>,
    bit_offset: Option<usize>,
    bit_width: Option<usize>,
    bit_mask: Option<u64>,
    axes: &[AxisDefinition],
) -> String {
    if let Some(unique_id) = unique_id {
        return format!("{}:uid:{}", kind.as_str(), normalize_text(unique_id));
    }
    let canonical = canonical_semantic_definition(
        kind, title, category, layout, conversion, bit_offset, bit_width, bit_mask, axes,
    );
    format!(
        "{}:semantic:{}",
        kind.as_str(),
        sha256_hex(canonical.as_bytes())
    )
}

fn canonical_semantic_definition(
    kind: ParameterKind,
    title: &str,
    category: Option<&str>,
    layout: &DataLayout,
    conversion: Option<&str>,
    bit_offset: Option<usize>,
    bit_width: Option<usize>,
    bit_mask: Option<u64>,
    axes: &[AxisDefinition],
) -> String {
    let axes = axes
        .iter()
        .map(|axis| {
            let storage = axis
                .storage
                .map(|storage| canonical_storage(&storage))
                .unwrap_or_else(|| "none".to_string());
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{}",
                normalize_text(&axis.id),
                normalize_text(&axis.title),
                axis.count,
                axis.element_width_bits,
                axis.stride_bits,
                axis.signed,
                endianness_name(axis.endianness),
                normalize_optional(axis.conversion.as_deref()),
                storage
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "kind={}|title={}|category={}|width_bits={}|rows={}|columns={}|row_stride_bits={}|column_stride_bits={}|signed={}|endianness={}|storage={}|conversion={}|bit_offset={:?}|bit_width={:?}|bit_mask={:?}|axes=[{}]",
        kind.as_str(),
        normalize_text(title),
        normalize_optional(category),
        layout.element_width_bits,
        layout.dimensions.rows,
        layout.dimensions.columns,
        layout.row_stride_bits,
        layout.column_stride_bits,
        layout.signed,
        endianness_name(layout.endianness),
        canonical_storage(&layout.storage),
        normalize_optional(conversion),
        bit_offset,
        bit_width,
        bit_mask,
        axes
    )
}

fn canonical_storage(storage: &StorageSpec) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}:{}",
        storage.element_size_bits,
        storage.signed,
        endianness_name(storage.byte_order),
        storage.numeric_kind.as_str(),
        storage.column_major,
        storage.raw_type_flags,
        storage.unknown_type_flags
    )
}

fn normalized_fingerprint(parameters: &[ParameterDefinition]) -> String {
    let mut definitions = parameters
        .iter()
        .map(|parameter| {
            canonical_semantic_definition(
                parameter.kind,
                &parameter.title,
                parameter.category.as_deref(),
                &parameter.layout,
                parameter.conversion.as_deref(),
                parameter.bit_offset,
                parameter.bit_width,
                parameter.bit_mask,
                &parameter.axes,
            )
        })
        .collect::<Vec<_>>();
    definitions.sort();
    let canonical = format!("{XDF_MODEL_VERSION}\n{}", definitions.join("\n"));
    sha256_hex(canonical.as_bytes())
}

fn endianness_name(endianness: Endianness) -> &'static str {
    match endianness {
        Endianness::Little => "little",
        Endianness::Big => "big",
    }
}

fn normalize_optional(value: Option<&str>) -> String {
    value.map(normalize_text).unwrap_or_default()
}

fn normalize_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn is_signed_type(value: &str) -> bool {
    let normalized = normalize_text(value);
    normalized.contains("signed")
        || normalized.contains("sint")
        || normalized == "int"
        || normalized.starts_with("int")
}

fn is_float_type(value: &str) -> bool {
    let normalized = normalize_text(value);
    normalized.contains("float")
        || normalized.contains("ieee")
        || normalized == "single"
        || normalized == "double"
}

fn parse_conversion(node: &XmlNode) -> Option<String> {
    let convert = node
        .children
        .iter()
        .find(|child| child.name == "XDFCONVERT");
    let math = convert
        .and_then(|convert| convert.descendant("MATH"))
        .or_else(|| node.children.iter().find(|child| child.name == "MATH"));
    if let Some(math) = math {
        if let Some(equation) = math.attr_any(&["equation", "expr", "expression"]) {
            return Some(equation);
        }
        if !math.text_value().is_empty() {
            return Some(math.text_value());
        }
    }
    if let Some(convert) = convert {
        if !convert.text_value().is_empty() {
            return Some(convert.text_value());
        }
    }
    None
}

fn update_parameter_nodes(
    node: &mut XmlNode,
    parameters: &[ParameterDefinition],
    parameter_index: &mut usize,
) -> Result<(), XdfError> {
    for child in &mut node.children {
        if matches!(
            child.name.as_str(),
            "XDFCONSTANT" | "XDFTABLE" | "XDFBITFIELD" | "XDFFLAG"
        ) {
            let parameter = parameters
                .get(*parameter_index)
                .ok_or_else(|| XdfError::Export {
                    message: format!(
                    "source tree has more editable objects than the normalized model at index {}",
                    *parameter_index
                ),
                })?;
            set_conversion_node(child, parameter.conversion.as_deref());
            let mut axis_index = 0;
            update_axis_nodes(child, &parameter.axes, &mut axis_index)?;
            if axis_index != parameter.axes.len() {
                return Err(XdfError::Export {
                    message: format!(
                        "source tree contains {axis_index} axis node(s) for '{}', but the model has {}",
                        parameter.semantic_id,
                        parameter.axes.len()
                    ),
                });
            }
            *parameter_index += 1;
        } else {
            update_parameter_nodes(child, parameters, parameter_index)?;
        }
    }
    Ok(())
}

fn update_axis_nodes(
    node: &mut XmlNode,
    axes: &[AxisDefinition],
    axis_index: &mut usize,
) -> Result<(), XdfError> {
    for child in &mut node.children {
        if child.name == "XDFAXIS" {
            let axis = axes.get(*axis_index).ok_or_else(|| XdfError::Export {
                message: format!(
                    "source tree has more axis objects than the normalized model at index {}",
                    *axis_index
                ),
            })?;
            set_conversion_node(child, axis.conversion.as_deref());
            *axis_index += 1;
            update_axis_nodes(child, axes, axis_index)?;
        } else {
            update_axis_nodes(child, axes, axis_index)?;
        }
    }
    Ok(())
}

fn set_conversion_node(node: &mut XmlNode, source: Option<&str>) {
    let conversion_index = node
        .children
        .iter()
        .position(|child| child.name == "XDFCONVERT");
    let Some(source) = source else {
        if let Some(index) = conversion_index {
            node.children.remove(index);
        }
        return;
    };

    let Some(index) = conversion_index else {
        node.children.push(XmlNode {
            name: "XDFCONVERT".to_string(),
            attrs: BTreeMap::new(),
            children: vec![XmlNode {
                name: "MATH".to_string(),
                attrs: BTreeMap::from([("equation".to_string(), source.to_string())]),
                children: Vec::new(),
                text: String::new(),
            }],
            text: String::new(),
        });
        return;
    };

    let conversion = &mut node.children[index];
    let math_index = conversion
        .children
        .iter()
        .position(|child| child.name == "MATH");
    let math = if let Some(math_index) = math_index {
        &mut conversion.children[math_index]
    } else {
        conversion.children.push(XmlNode {
            name: "MATH".to_string(),
            attrs: BTreeMap::new(),
            children: Vec::new(),
            text: String::new(),
        });
        conversion.children.last_mut().expect("pushed MATH node")
    };
    math.attrs
        .insert("equation".to_string(), source.to_string());
    math.attrs.remove("expr");
    math.attrs.remove("expression");
    math.text.clear();
}

fn escape_xml(value: &str, attribute: bool) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' if attribute => escaped.push_str("&quot;"),
            '\'' if attribute => escaped.push_str("&apos;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn write_xml_node(node: &XmlNode, output: &mut String, depth: usize) {
    let indent = "  ".repeat(depth);
    output.push_str(&indent);
    output.push('<');
    output.push_str(&node.name);
    for (name, value) in &node.attrs {
        output.push(' ');
        output.push_str(name);
        output.push_str("=\"");
        output.push_str(&escape_xml(value, true));
        output.push('"');
    }

    let text = node.text.trim();
    if node.children.is_empty() && text.is_empty() {
        output.push_str("/>\n");
        return;
    }
    output.push('>');
    if !text.is_empty() {
        output.push_str(&escape_xml(text, false));
    }
    if !node.children.is_empty() {
        output.push('\n');
        for child in &node.children {
            write_xml_node(child, output, depth + 1);
        }
        output.push_str(&indent);
    }
    output.push_str("</");
    output.push_str(&node.name);
    output.push_str(">\n");
}

fn lookup_string(
    node: &XmlNode,
    preferred: Option<&XmlNode>,
    attr_names: &[&str],
    child_names: &[&str],
) -> Option<String> {
    if let Some(preferred) = preferred {
        if let Some(value) = preferred.attr_any(attr_names) {
            return Some(value);
        }
    }
    if let Some(value) = node.attr_any(attr_names) {
        return Some(value);
    }
    for child_name in child_names {
        if let Some(child) = node.descendant(child_name) {
            let value = child.text_value();
            if !value.is_empty() {
                return Some(value);
            }
            if let Some(value) = child.attr_any(&["value", "text", "name", "index"]) {
                return Some(value);
            }
        }
    }
    None
}

fn lookup_number(
    node: &XmlNode,
    preferred: Option<&XmlNode>,
    attr_names: &[&str],
    child_names: &[&str],
    object: &str,
    field: &str,
) -> Result<Option<usize>, XdfError> {
    lookup_string(node, preferred, attr_names, child_names)
        .map(|value| parse_number(object, field, &value))
        .transpose()
}

fn lookup_u32(
    node: &XmlNode,
    preferred: Option<&XmlNode>,
    attr_names: &[&str],
    child_names: &[&str],
    object: &str,
    field: &str,
) -> Result<Option<u32>, XdfError> {
    lookup_string(node, preferred, attr_names, child_names)
        .map(|value| {
            let parsed = parse_u64(object, field, &value)?;
            u32::try_from(parsed)
                .map_err(|_| invalid_field(object, field, value, "number exceeds u32"))
        })
        .transpose()
}

fn lookup_f64(
    node: &XmlNode,
    preferred: Option<&XmlNode>,
    attr_names: &[&str],
    child_names: &[&str],
    object: &str,
    field: &str,
) -> Result<Option<f64>, XdfError> {
    lookup_string(node, preferred, attr_names, child_names)
        .map(|value| parse_f64(object, field, &value))
        .transpose()
}

fn lookup_signed_number(
    node: &XmlNode,
    preferred: Option<&XmlNode>,
    attr_names: &[&str],
    child_names: &[&str],
    object: &str,
    field: &str,
) -> Result<Option<i64>, XdfError> {
    lookup_string(node, preferred, attr_names, child_names)
        .map(|value| parse_i64(object, field, &value))
        .transpose()
}

fn lookup_bool(
    node: &XmlNode,
    preferred: Option<&XmlNode>,
    attr_names: &[&str],
    child_names: &[&str],
) -> Option<bool> {
    let value = lookup_string(node, preferred, attr_names, child_names)?;
    match normalize_text(&value).as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

fn lookup_endianness(
    node: &XmlNode,
    preferred: Option<&XmlNode>,
    attr_names: &[&str],
    child_names: &[&str],
) -> Option<Endianness> {
    let value = lookup_string(node, preferred, attr_names, child_names)?;
    match normalize_text(&value).as_str() {
        "big" | "be" | "bigendian" | "big-endian" => Some(Endianness::Big),
        "little" | "le" | "littleendian" | "little-endian" => Some(Endianness::Little),
        _ => None,
    }
}

fn parse_number(object: &str, field: &str, value: &str) -> Result<usize, XdfError> {
    let parsed = parse_u64(object, field, value)?;
    usize::try_from(parsed).map_err(|_| {
        invalid_field(
            object,
            field,
            value.to_string(),
            "number exceeds the platform address size",
        )
    })
}

fn parse_i64(object: &str, field: &str, value: &str) -> Result<i64, XdfError> {
    let trimmed = value.trim();
    if let Some(magnitude) = trimmed.strip_prefix('-') {
        let magnitude = parse_u64(object, field, magnitude)?;
        if magnitude > (i64::MAX as u64) + 1 {
            return Err(invalid_field(
                object,
                field,
                value.to_string(),
                "number is outside the signed 64-bit range",
            ));
        }
        if magnitude == (i64::MAX as u64) + 1 {
            Ok(i64::MIN)
        } else {
            Ok(-(magnitude as i64))
        }
    } else {
        let parsed = parse_u64(object, field, trimmed)?;
        i64::try_from(parsed).map_err(|_| {
            invalid_field(
                object,
                field,
                value.to_string(),
                "number is outside the signed 64-bit range",
            )
        })
    }
}

fn parse_f64(object: &str, field: &str, value: &str) -> Result<f64, XdfError> {
    let parsed = value.trim().parse::<f64>().map_err(|_| {
        invalid_field(
            object,
            field,
            value.to_string(),
            "expected a finite decimal number",
        )
    })?;
    if !parsed.is_finite() {
        return Err(invalid_field(
            object,
            field,
            value.to_string(),
            "expected a finite decimal number",
        ));
    }
    Ok(parsed)
}

fn parse_u64(object: &str, field: &str, value: &str) -> Result<u64, XdfError> {
    let trimmed = value.trim();
    let unsigned = trimmed.strip_prefix('+').unwrap_or(trimmed);
    let (digits, radix) = if let Some(value) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
    {
        (value, 16)
    } else if let Some(value) = unsigned.strip_prefix('$') {
        (value, 16)
    } else {
        (unsigned, 10)
    };
    if digits.is_empty() {
        return Err(invalid_field(
            object,
            field,
            value.to_string(),
            "number cannot be empty",
        ));
    }
    u64::from_str_radix(digits, radix).map_err(|_| {
        invalid_field(
            object,
            field,
            value.to_string(),
            "expected a non-negative decimal or hexadecimal integer",
        )
    })
}

fn invalid_field(object: &str, field: &str, value: String, message: &str) -> XdfError {
    XdfError::InvalidField {
        object: object.to_string(),
        field: field.to_string(),
        value,
        message: message.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq)]
struct XmlNode {
    name: String,
    attrs: BTreeMap<String, String>,
    children: Vec<XmlNode>,
    text: String,
}

impl XmlNode {
    fn attr_any(&self, names: &[&str]) -> Option<String> {
        names
            .iter()
            .find_map(|name| self.attrs.get(&name.to_ascii_lowercase()).cloned())
    }

    fn text_value(&self) -> String {
        self.text.trim().to_string()
    }

    fn descendant(&self, name: &str) -> Option<&XmlNode> {
        self.children.iter().find_map(|child| {
            if child.name == name {
                Some(child)
            } else {
                child.descendant(name)
            }
        })
    }

    fn descendants_named<'a>(&'a self, name: &str, output: &mut Vec<&'a XmlNode>) {
        for child in &self.children {
            if child.name == name {
                output.push(child);
            }
            child.descendants_named(name, output);
        }
    }
}

fn text_node(name: &str, text: &str) -> XmlNode {
    XmlNode {
        name: name.to_string(),
        attrs: BTreeMap::new(),
        children: Vec::new(),
        text: text.to_string(),
    }
}

fn serialize_xdf_root(root: &XmlNode) -> String {
    let mut output = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    write_xml_node(root, &mut output, 0);
    output
}

fn reverse_translate_address(
    base_offset: XdfBaseOffset,
    bin_address: usize,
) -> Result<u64, XdfError> {
    let address = if base_offset.subtract {
        (bin_address as u128).checked_add(base_offset.offset as u128)
    } else {
        (bin_address as u128).checked_sub(base_offset.offset as u128)
    }
    .filter(|address| *address <= u64::MAX as u128)
    .ok_or_else(|| XdfError::Overflow {
        object: "XDFHEADER.BASEOFFSET".to_string(),
        message: format!("BIN offset {bin_address:#x} cannot be represented as an XDF address"),
    })?;
    Ok(address as u64)
}

fn parameter_element_name(kind: ParameterKind) -> &'static str {
    match kind {
        ParameterKind::Constant => "XDFCONSTANT",
        ParameterKind::Table => "XDFTABLE",
        ParameterKind::BitField => "XDFBITFIELD",
        ParameterKind::Flag => "XDFFLAG",
    }
}

fn is_parameter_node(node: &XmlNode) -> bool {
    matches!(
        node.name.as_str(),
        "XDFCONSTANT" | "XDFTABLE" | "XDFBITFIELD" | "XDFFLAG"
    )
}

fn set_optional_attr(node: &mut XmlNode, name: &str, value: Option<String>) {
    if let Some(value) = value {
        node.attrs.insert(name.to_string(), value);
    } else {
        node.attrs.remove(name);
    }
}

fn set_child_text(node: &mut XmlNode, name: &str, value: &str) {
    if let Some(child) = node.children.iter_mut().find(|child| child.name == name) {
        child.text = value.to_string();
        child.children.clear();
    } else {
        node.children.push(text_node(name, value));
    }
}

fn set_optional_child_text(node: &mut XmlNode, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        set_child_text(node, name, value);
    } else {
        node.children.retain(|child| child.name != name);
    }
}

fn header_node_mut(root: &mut XmlNode) -> &mut XmlNode {
    let index = root
        .children
        .iter()
        .position(|child| child.name == "XDFHEADER")
        .unwrap_or_else(|| {
            root.children.push(XmlNode {
                name: "XDFHEADER".to_string(),
                attrs: BTreeMap::new(),
                children: Vec::new(),
                text: String::new(),
            });
            root.children.len() - 1
        });
    &mut root.children[index]
}

fn sync_header_node(root: &mut XmlNode, header: &XdfHeader) {
    set_optional_attr(&mut *root, "version", header.version.clone());
    let node = header_node_mut(root);
    let flags = header.flags.map(|value| format!("{value:#X}"));
    set_optional_child_text(node, "FLAGS", flags.as_deref());
    set_optional_child_text(node, "DEFTITLE", header.title.as_deref());
    set_optional_child_text(node, "DESCRIPTION", header.description.as_deref());
    set_optional_child_text(node, "AUTHOR", header.author.as_deref());

    let base_offset_index = node
        .children
        .iter()
        .position(|child| child.name == "BASEOFFSET")
        .unwrap_or_else(|| {
            node.children.push(XmlNode {
                name: "BASEOFFSET".to_string(),
                attrs: BTreeMap::new(),
                children: Vec::new(),
                text: String::new(),
            });
            node.children.len() - 1
        });
    let base_offset = &mut node.children[base_offset_index];
    base_offset.attrs.insert(
        "offset".to_string(),
        format!("{:#X}", header.base_offset.offset),
    );
    base_offset.attrs.insert(
        "subtract".to_string(),
        if header.base_offset.subtract {
            "1"
        } else {
            "0"
        }
        .to_string(),
    );

    let defaults_index = node
        .children
        .iter()
        .position(|child| child.name == "DEFAULTS")
        .unwrap_or_else(|| {
            node.children.push(XmlNode {
                name: "DEFAULTS".to_string(),
                attrs: BTreeMap::new(),
                children: Vec::new(),
                text: String::new(),
            });
            node.children.len() - 1
        });
    let defaults = &mut node.children[defaults_index];
    set_optional_attr(
        defaults,
        "datasizeinbits",
        header.defaults.data_size_bits.map(|v| v.to_string()),
    );
    set_optional_attr(
        defaults,
        "sigdigits",
        header.defaults.significant_digits.map(|v| v.to_string()),
    );
    set_optional_attr(
        defaults,
        "outputtype",
        header.defaults.output_type.map(|v| v.to_string()),
    );
    set_optional_attr(
        defaults,
        "signed",
        header
            .defaults
            .signed
            .map(|v| if v { "1" } else { "0" }.into()),
    );
    set_optional_attr(
        defaults,
        "lsbfirst",
        header
            .defaults
            .lsb_first
            .map(|v| if v { "1" } else { "0" }.into()),
    );
    set_optional_attr(
        defaults,
        "float",
        header
            .defaults
            .float
            .map(|v| if v { "1" } else { "0" }.into()),
    );
}

fn validate_categories(categories: &[XdfCategory]) -> Result<(), XdfError> {
    let mut indices = BTreeSet::new();
    let mut names = BTreeSet::new();
    for category in categories {
        if category.name.trim().is_empty() {
            return Err(invalid_field(
                "XDFHEADER.CATEGORY",
                "name",
                String::new(),
                "category name cannot be empty",
            ));
        }
        if !indices.insert(category.index) {
            return Err(XdfError::DuplicateIdentity {
                identity: format!("category index {}", category.index),
            });
        }
        if !names.insert(normalize_text(&category.name)) {
            return Err(XdfError::DuplicateIdentity {
                identity: format!("category name {}", category.name),
            });
        }
    }
    Ok(())
}

fn sync_category_nodes(root: &mut XmlNode, categories: &[XdfCategory]) {
    let header = header_node_mut(root);
    let old = std::mem::take(&mut header.children);
    let first_category = old.iter().position(|child| child.name == "CATEGORY");
    let mut old_categories = Vec::new();
    let mut other_children = Vec::new();
    let mut insertion = 0;
    for (position, child) in old.into_iter().enumerate() {
        if child.name == "CATEGORY" {
            old_categories.push(Some(child));
        } else {
            if first_category.is_some_and(|first| position < first) {
                insertion += 1;
            }
            other_children.push(child);
        }
    }
    let mut category_nodes = Vec::with_capacity(categories.len());
    for (ordinal, category) in categories.iter().enumerate() {
        let matching = old_categories.iter().position(|candidate| {
            candidate.as_ref().is_some_and(|node| {
                node.attrs
                    .get("index")
                    .and_then(|value| parse_u64("CATEGORY", "index", value).ok())
                    == Some(category.index)
                    || node
                        .attrs
                        .get("name")
                        .is_some_and(|name| normalize_text(name) == normalize_text(&category.name))
            })
        });
        let matching = matching.or_else(|| {
            old_categories
                .get(ordinal)
                .is_some_and(Option::is_some)
                .then_some(ordinal)
        });
        let mut node = matching
            .and_then(|index| old_categories.get_mut(index).and_then(Option::take))
            .unwrap_or(XmlNode {
                name: "CATEGORY".to_string(),
                attrs: BTreeMap::new(),
                children: Vec::new(),
                text: String::new(),
            });
        node.attrs
            .insert("index".to_string(), format!("{:#X}", category.index));
        node.attrs.insert("name".to_string(), category.name.clone());
        category_nodes.push(node);
    }
    insertion = insertion.min(other_children.len());
    other_children.splice(insertion..insertion, category_nodes);
    header.children = other_children;
}

fn category_reference_value(
    category_index: u64,
    categories: &[XdfCategory],
    reference_mode: CategoryReferenceMode,
) -> Option<u64> {
    match reference_mode {
        CategoryReferenceMode::DeclaredIndex => Some(category_index),
        CategoryReferenceMode::OneBasedPosition => categories
            .iter()
            .position(|category| category.index == category_index)
            .map(|position| position as u64 + 1),
    }
}

fn remap_category_references(
    root: &mut XmlNode,
    old_categories: &[XdfCategory],
    new_categories: &[XdfCategory],
    reference_mode: CategoryReferenceMode,
) -> Result<(), XdfError> {
    fn visit(
        node: &mut XmlNode,
        old_categories: &[XdfCategory],
        new_categories: &[XdfCategory],
        reference_mode: CategoryReferenceMode,
    ) -> Result<(), XdfError> {
        for child in &mut node.children {
            if child.name == "CATEGORYMEM" {
                let Some(raw) = child.attrs.get("category").cloned() else {
                    continue;
                };
                let raw_index = parse_u64("CATEGORYMEM", "category", &raw)?;
                let old_position = match reference_mode {
                    CategoryReferenceMode::DeclaredIndex => old_categories
                        .iter()
                        .position(|category| category.index == raw_index),
                    CategoryReferenceMode::OneBasedPosition => raw_index
                        .checked_sub(1)
                        .and_then(|position| usize::try_from(position).ok())
                        .filter(|position| *position < old_categories.len()),
                };
                let Some(old_position) = old_position else {
                    continue;
                };
                let old_category = &old_categories[old_position];
                let new_category = new_categories
                    .iter()
                    .find(|category| {
                        normalize_text(&category.name) == normalize_text(&old_category.name)
                    })
                    .or_else(|| {
                        new_categories
                            .iter()
                            .find(|category| category.index == old_category.index)
                    })
                    .ok_or_else(|| XdfError::Export {
                        message: format!(
                            "category '{}' is still referenced and cannot be removed; reassign its memberships first",
                            old_category.name
                        ),
                    })?;
                if let Some(reference) =
                    category_reference_value(new_category.index, new_categories, reference_mode)
                {
                    child
                        .attrs
                        .insert("category".to_string(), format!("{reference:#X}"));
                }
            } else {
                visit(child, old_categories, new_categories, reference_mode)?;
            }
        }
        Ok(())
    }
    visit(root, old_categories, new_categories, reference_mode)
}

fn remap_parameter_category_paths(
    root: &mut XmlNode,
    old_categories: &[XdfCategory],
    new_categories: &[XdfCategory],
) {
    fn visit(node: &mut XmlNode, old_categories: &[XdfCategory], new_categories: &[XdfCategory]) {
        if is_parameter_node(node) {
            for key in ["category", "categorypath"] {
                let Some(old_name) = node.attrs.get(key).cloned() else {
                    continue;
                };
                let Some(old_category) = old_categories
                    .iter()
                    .find(|category| normalize_text(&category.name) == normalize_text(&old_name))
                else {
                    continue;
                };
                if let Some(new_category) = new_categories.iter().find(|category| {
                    category.index == old_category.index
                        || normalize_text(&category.name) == normalize_text(&old_category.name)
                }) {
                    node.attrs
                        .insert(key.to_string(), new_category.name.clone());
                }
            }
        }
        for child in &mut node.children {
            visit(child, old_categories, new_categories);
        }
    }
    visit(root, old_categories, new_categories);
}

fn ensure_category(
    root: &mut XmlNode,
    categories: &mut Vec<XdfCategory>,
    name: Option<&str>,
    reference_mode: CategoryReferenceMode,
) -> Result<Option<u64>, XdfError> {
    let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) else {
        return Ok(None);
    };
    let index = if let Some(category) = categories
        .iter()
        .find(|category| normalize_text(&category.name) == normalize_text(name))
    {
        category.index
    } else {
        let index = categories
            .iter()
            .map(|category| category.index)
            .max()
            .map_or(0, |value| value.saturating_add(1));
        categories.push(XdfCategory {
            index,
            name: name.to_string(),
        });
        validate_categories(categories)?;
        sync_category_nodes(root, categories);
        index
    };
    Ok(Some(match reference_mode {
        CategoryReferenceMode::DeclaredIndex => index,
        CategoryReferenceMode::OneBasedPosition => categories
            .iter()
            .position(|category| category.index == index)
            .map(|position| position as u64 + 1)
            .ok_or_else(|| XdfError::Export {
                message: format!("category {name:?} could not be assigned a reference"),
            })?,
    }))
}

fn category_memberships_for_definition(
    definition: &XdfParameterDraft,
    primary_reference: Option<u64>,
    categories: &[XdfCategory],
    reference_mode: CategoryReferenceMode,
) -> Vec<(usize, u64)> {
    if definition.category_memberships.is_empty() {
        return primary_reference
            .map(|category| vec![(0, category)])
            .unwrap_or_default();
    }

    let mut memberships = definition
        .category_memberships
        .iter()
        .filter_map(|membership| {
            if membership.slot == 0 {
                return primary_reference.map(|category| (0, category));
            }
            let reference = membership
                .category_name
                .as_ref()
                .and_then(|name| {
                    categories
                        .iter()
                        .find(|category| normalize_text(&category.name) == normalize_text(name))
                })
                .and_then(|category| {
                    category_reference_value(category.index, categories, reference_mode)
                })
                .unwrap_or(membership.category_index);
            Some((membership.slot, reference))
        })
        .collect::<Vec<_>>();
    if primary_reference.is_some() && !memberships.iter().any(|(slot, _)| *slot == 0) {
        memberships.push((0, primary_reference.unwrap()));
    }
    memberships.sort_by_key(|(slot, _)| *slot);
    memberships
}

fn set_category_memberships(node: &mut XmlNode, memberships: &[(usize, u64)]) {
    fn collect_memberships(node: &XmlNode, output: &mut Vec<XmlNode>) {
        for child in &node.children {
            if child.name == "CATEGORYMEM" {
                output.push(child.clone());
            } else {
                collect_memberships(child, output);
            }
        }
    }
    fn remove_memberships(node: &mut XmlNode) {
        node.children.retain(|child| child.name != "CATEGORYMEM");
        for child in &mut node.children {
            remove_memberships(child);
        }
    }
    let mut old_memberships = Vec::new();
    collect_memberships(node, &mut old_memberships);
    let old_count = old_memberships.len();
    let mut output = Vec::with_capacity(memberships.len());
    for (ordinal, (slot, category)) in memberships.iter().enumerate() {
        let existing = old_memberships
            .iter()
            .position(|candidate| {
                candidate
                    .attrs
                    .get("index")
                    .and_then(|value| parse_u64("CATEGORYMEM", "index", value).ok())
                    .and_then(|value| usize::try_from(value).ok())
                    == Some(*slot)
            })
            .or_else(|| (ordinal < old_count).then_some(ordinal));
        let mut member = existing
            .map(|index| old_memberships.remove(index))
            .unwrap_or(XmlNode {
                name: "CATEGORYMEM".to_string(),
                attrs: BTreeMap::new(),
                children: Vec::new(),
                text: String::new(),
            });
        member.name = "CATEGORYMEM".to_string();
        member.attrs.insert("index".to_string(), slot.to_string());
        member
            .attrs
            .insert("category".to_string(), format!("{category:#X}"));
        output.push(member);
    }
    remove_memberships(node);
    node.children.extend(output);
    if memberships.is_empty() {
        node.attrs.remove("categorypath");
    }
}

fn storage_flags(
    width_bits: u32,
    signed: bool,
    endianness: Endianness,
    numeric_kind: NumericKind,
    column_major: bool,
    unknown_type_flags: u32,
    object: &str,
) -> Result<u32, XdfError> {
    if width_bits == 0 || width_bits > 64 {
        return Err(invalid_field(
            object,
            "element_width_bits",
            width_bits.to_string(),
            "supported numeric widths are 1 through 64 bits",
        ));
    }
    if numeric_kind == NumericKind::Unsupported {
        return Err(invalid_field(
            object,
            "numeric_kind",
            numeric_kind.as_str().to_string(),
            "unsupported storage encodings cannot be authored",
        ));
    }
    if numeric_kind == NumericKind::Ieee754Binary32 && width_bits != 32 {
        return Err(invalid_field(
            object,
            "numeric_kind",
            numeric_kind.as_str().to_string(),
            "IEEE-754 binary32 requires a 32-bit element",
        ));
    }
    let mut flags = unknown_type_flags & !KNOWN_STORAGE_FLAGS;
    if signed {
        flags |= SIGNED_STORAGE_FLAG;
    }
    if endianness == Endianness::Little {
        flags |= LSB_FIRST_STORAGE_FLAG;
    }
    if column_major {
        flags |= COLUMN_MAJOR_STORAGE_FLAG;
    }
    if numeric_kind == NumericKind::Ieee754Binary32 {
        flags |= BINARY32_STORAGE_FLAG;
    }
    Ok(flags)
}

fn validate_parameter_draft(definition: &XdfParameterDraft) -> Result<(), XdfError> {
    let object = definition
        .unique_id
        .as_deref()
        .unwrap_or(definition.title.as_str());
    if definition.title.trim().is_empty() {
        return Err(invalid_field(
            object,
            "title",
            definition.title.clone(),
            "title cannot be empty",
        ));
    }
    if definition.dimensions.rows == 0 || definition.dimensions.columns == 0 {
        return Err(invalid_field(
            object,
            "dimensions",
            format!(
                "{}x{}",
                definition.dimensions.rows, definition.dimensions.columns
            ),
            "dimensions must be greater than zero",
        ));
    }
    if definition.dimensions.elements().is_none() {
        return Err(XdfError::Overflow {
            object: object.to_string(),
            message: "table dimensions overflow".to_string(),
        });
    }
    if definition.kind != ParameterKind::Table
        && (definition.dimensions.rows != 1 || definition.dimensions.columns != 1)
    {
        return Err(invalid_field(
            object,
            "dimensions",
            format!(
                "{}x{}",
                definition.dimensions.rows, definition.dimensions.columns
            ),
            "constants, flags, and bitfields must use 1x1 dimensions",
        ));
    }
    if definition
        .unique_id
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(invalid_field(
            object,
            "unique_id",
            String::new(),
            "unique ID cannot be empty",
        ));
    }
    storage_flags(
        definition.element_width_bits,
        definition.signed,
        definition.endianness,
        definition.numeric_kind,
        definition.column_major,
        definition.unknown_type_flags,
        object,
    )?;
    let storage_width = definition.element_width_bits as usize;
    let bit_offset = definition.bit_offset.unwrap_or(0);
    if definition
        .bit_offset
        .is_some_and(|offset| offset >= storage_width)
        || definition.bit_width.is_some_and(|width| {
            width == 0
                || bit_offset
                    .checked_add(width)
                    .is_none_or(|end| end > storage_width)
        })
        || definition
            .bit_mask
            .is_some_and(|mask| storage_width < 64 && mask >> storage_width != 0)
    {
        return Err(invalid_field(
            object,
            "bit_range",
            format!(
                "offset={:?}, width={:?}, mask={:?}",
                definition.bit_offset, definition.bit_width, definition.bit_mask
            ),
            "bit offsets, widths, and masks must fit the storage element",
        ));
    }
    if let Some(conversion) = &definition.conversion {
        Conversion::parse(conversion).map_err(|error| {
            invalid_field(object, "conversion", conversion.clone(), &error.to_string())
        })?;
    }
    let mut axis_ids = BTreeSet::new();
    for axis in &definition.axes {
        if axis.id.trim().is_empty() || axis.count == 0 {
            return Err(invalid_field(
                object,
                "axis",
                axis.id.clone(),
                "axis ID and count must be non-empty and non-zero",
            ));
        }
        if !axis_ids.insert(normalize_text(&axis.id)) {
            return Err(XdfError::DuplicateIdentity {
                identity: format!("{} axis {}", object, axis.id),
            });
        }
        if axis.address.is_some() {
            storage_flags(
                axis.element_width_bits,
                axis.signed,
                axis.endianness,
                axis.numeric_kind,
                false,
                axis.unknown_type_flags,
                &format!("{object}/axis:{}", axis.id),
            )?;
        }
        if axis.metadata.min.is_some_and(|value| !value.is_finite())
            || axis.metadata.max.is_some_and(|value| !value.is_finite())
        {
            return Err(invalid_field(
                object,
                "axis.range",
                axis.id.clone(),
                "axis bounds must be finite",
            ));
        }
        if let Some(conversion) = &axis.conversion {
            Conversion::parse(conversion).map_err(|error| {
                invalid_field(
                    object,
                    "axis.conversion",
                    conversion.clone(),
                    &error.to_string(),
                )
            })?;
        }
    }
    Ok(())
}

fn build_embedded_data(
    address: u64,
    width_bits: u32,
    signed: bool,
    endianness: Endianness,
    numeric_kind: NumericKind,
    column_major: bool,
    unknown_type_flags: u32,
    object: &str,
) -> Result<XmlNode, XdfError> {
    let flags = storage_flags(
        width_bits,
        signed,
        endianness,
        numeric_kind,
        column_major,
        unknown_type_flags,
        object,
    )?;
    Ok(XmlNode {
        name: "EMBEDDEDDATA".to_string(),
        attrs: BTreeMap::from([
            ("mmedaddress".to_string(), format!("{address:#X}")),
            ("mmedelementsizebits".to_string(), width_bits.to_string()),
            ("mmedtypeflags".to_string(), format!("{flags:#X}")),
        ]),
        children: Vec::new(),
        text: String::new(),
    })
}

fn build_parameter_node(
    definition: &XdfParameterDraft,
    category_memberships: &[(usize, u64)],
) -> Result<XmlNode, XdfError> {
    validate_parameter_draft(definition)?;
    let mut node = XmlNode {
        name: parameter_element_name(definition.kind).to_string(),
        attrs: BTreeMap::new(),
        children: Vec::new(),
        text: String::new(),
    };
    if let Some(unique_id) = &definition.unique_id {
        node.attrs.insert("uniqueid".to_string(), unique_id.clone());
    }
    set_child_text(&mut node, "TITLE", &definition.title);
    set_optional_child_text(
        &mut node,
        "DESCRIPTION",
        (!definition.description.is_empty()).then_some(definition.description.as_str()),
    );
    if let Some(category) = &definition.category {
        node.attrs
            .insert("categorypath".to_string(), category.clone());
    }
    set_category_memberships(&mut node, category_memberships);
    set_conversion_node(&mut node, definition.conversion.as_deref());
    for axis in &definition.axes {
        node.children.push(build_axis_node(axis)?);
    }
    let mut embedded = build_embedded_data(
        definition.xdf_address,
        definition.element_width_bits,
        definition.signed,
        definition.endianness,
        definition.numeric_kind,
        definition.column_major,
        definition.unknown_type_flags,
        &definition.title,
    )?;
    embedded.attrs.insert(
        "mmedrowcount".to_string(),
        definition.dimensions.rows.to_string(),
    );
    embedded.attrs.insert(
        "mmedcolcount".to_string(),
        definition.dimensions.columns.to_string(),
    );
    embedded.attrs.insert(
        "mmedmajorstridebits".to_string(),
        definition.row_stride_bits.to_string(),
    );
    embedded.attrs.insert(
        "mmedminorstridebits".to_string(),
        definition.column_stride_bits.to_string(),
    );
    node.children.push(embedded);
    set_bit_fields(
        &mut node,
        definition.bit_offset,
        definition.bit_width,
        definition.bit_mask,
    );
    Ok(node)
}

fn build_axis_node(axis: &XdfAxisDraft) -> Result<XmlNode, XdfError> {
    let mut node = XmlNode {
        name: "XDFAXIS".to_string(),
        attrs: BTreeMap::from([
            ("id".to_string(), axis.id.clone()),
            ("indexcount".to_string(), axis.count.to_string()),
        ]),
        children: Vec::new(),
        text: String::new(),
    };
    set_child_text(&mut node, "TITLE", &axis.title);
    sync_axis_metadata(&mut node, axis);
    set_conversion_node(&mut node, axis.conversion.as_deref());
    for label in &axis.labels {
        node.children.push(XmlNode {
            name: "LABEL".to_string(),
            attrs: BTreeMap::from([("index".to_string(), label.index.to_string())]),
            children: Vec::new(),
            text: label.value.clone(),
        });
    }
    if let Some(address) = axis.address {
        let mut embedded = build_embedded_data(
            address,
            axis.element_width_bits,
            axis.signed,
            axis.endianness,
            axis.numeric_kind,
            false,
            axis.unknown_type_flags,
            &format!("axis:{}", axis.id),
        )?;
        embedded.attrs.insert(
            "mmedmajorstridebits".to_string(),
            axis.stride_bits.to_string(),
        );
        node.children.push(embedded);
    }
    Ok(node)
}

fn set_bit_fields(
    node: &mut XmlNode,
    offset: Option<usize>,
    width: Option<usize>,
    mask: Option<u64>,
) {
    set_optional_attr(node, "bitoffset", offset.map(|value| value.to_string()));
    set_optional_attr(node, "bitwidth", width.map(|value| value.to_string()));
    set_optional_attr(node, "mask", mask.map(|value| format!("{value:#X}")));
}

fn sync_axis_metadata(node: &mut XmlNode, axis: &XdfAxisDraft) {
    set_optional_attr(node, "units", axis.metadata.units.clone());
    set_optional_attr(
        node,
        "unittype",
        axis.metadata.unit_type.map(|value| value.to_string()),
    );
    set_optional_attr(
        node,
        "decimalpl",
        axis.metadata.decimal_places.map(|value| value.to_string()),
    );
    set_optional_attr(
        node,
        "min",
        axis.metadata.min.map(|value| value.to_string()),
    );
    set_optional_attr(
        node,
        "max",
        axis.metadata.max.map(|value| value.to_string()),
    );
    set_optional_attr(
        node,
        "outputtype",
        axis.metadata.output_type.map(|value| value.to_string()),
    );
}

fn sync_axis_node(node: &mut XmlNode, axis: &XdfAxisDraft) -> Result<(), XdfError> {
    node.attrs.insert("id".to_string(), axis.id.clone());
    node.attrs
        .insert("indexcount".to_string(), axis.count.to_string());
    set_child_text(node, "TITLE", &axis.title);
    sync_axis_metadata(node, axis);
    set_conversion_node(node, axis.conversion.as_deref());
    node.children.retain(|child| child.name != "LABEL");
    node.children
        .extend(axis.labels.iter().map(|label| XmlNode {
            name: "LABEL".to_string(),
            attrs: BTreeMap::from([("index".to_string(), label.index.to_string())]),
            children: Vec::new(),
            text: label.value.clone(),
        }));
    if let Some(address) = axis.address {
        let flags = storage_flags(
            axis.element_width_bits,
            axis.signed,
            axis.endianness,
            axis.numeric_kind,
            false,
            axis.unknown_type_flags,
            &format!("axis:{}", axis.id),
        )?;
        let embedded_index = node
            .children
            .iter()
            .position(|child| child.name == "EMBEDDEDDATA")
            .unwrap_or_else(|| {
                node.children.push(XmlNode {
                    name: "EMBEDDEDDATA".to_string(),
                    attrs: BTreeMap::new(),
                    children: Vec::new(),
                    text: String::new(),
                });
                node.children.len() - 1
            });
        let embedded = &mut node.children[embedded_index];
        embedded
            .attrs
            .insert("mmedaddress".to_string(), format!("{address:#X}"));
        embedded.attrs.insert(
            "mmedelementsizebits".to_string(),
            axis.element_width_bits.to_string(),
        );
        embedded.attrs.insert(
            "mmedmajorstridebits".to_string(),
            axis.stride_bits.to_string(),
        );
        embedded.attrs.remove("mmedminorstridebits");
        embedded
            .attrs
            .insert("mmedtypeflags".to_string(), format!("{flags:#X}"));
    } else {
        node.children.retain(|child| child.name != "EMBEDDEDDATA");
    }
    Ok(())
}

fn sync_axis_nodes(node: &mut XmlNode, axes: &[XdfAxisDraft]) -> Result<(), XdfError> {
    for axis in axes {
        if let Some(existing) = find_axis_node_mut(node, &axis.id) {
            sync_axis_node(existing, axis)?;
        } else {
            node.children.push(build_axis_node(axis)?);
        }
    }
    let retained_ids = axes
        .iter()
        .map(|axis| axis.id.as_str())
        .collect::<BTreeSet<_>>();
    remove_unlisted_axis_nodes(node, &retained_ids);
    Ok(())
}

fn sync_embedded_data(node: &mut XmlNode, definition: &XdfParameterDraft) -> Result<(), XdfError> {
    let existing = find_parameter_embedded_mut(node);
    let embedded = if let Some(existing) = existing {
        existing
    } else {
        node.children.push(XmlNode {
            name: "EMBEDDEDDATA".to_string(),
            attrs: BTreeMap::new(),
            children: Vec::new(),
            text: String::new(),
        });
        node.children.last_mut().expect("EMBEDDEDDATA was inserted")
    };
    let flags = storage_flags(
        definition.element_width_bits,
        definition.signed,
        definition.endianness,
        definition.numeric_kind,
        definition.column_major,
        definition.unknown_type_flags,
        &definition.title,
    )?;
    embedded.attrs.insert(
        "mmedaddress".to_string(),
        format!("{:#X}", definition.xdf_address),
    );
    embedded.attrs.insert(
        "mmedelementsizebits".to_string(),
        definition.element_width_bits.to_string(),
    );
    embedded.attrs.insert(
        "mmedrowcount".to_string(),
        definition.dimensions.rows.to_string(),
    );
    embedded.attrs.insert(
        "mmedcolcount".to_string(),
        definition.dimensions.columns.to_string(),
    );
    embedded.attrs.insert(
        "mmedmajorstridebits".to_string(),
        definition.row_stride_bits.to_string(),
    );
    embedded.attrs.insert(
        "mmedminorstridebits".to_string(),
        definition.column_stride_bits.to_string(),
    );
    embedded
        .attrs
        .insert("mmedtypeflags".to_string(), format!("{flags:#X}"));
    Ok(())
}

fn update_parameter_node(
    node: &mut XmlNode,
    definition: &XdfParameterDraft,
    category_memberships: &[(usize, u64)],
) -> Result<(), XdfError> {
    validate_parameter_draft(definition)?;
    node.name = parameter_element_name(definition.kind).to_string();
    set_optional_attr(node, "uniqueid", definition.unique_id.clone());
    if let Some(category) = &definition.category {
        node.attrs
            .insert("categorypath".to_string(), category.clone());
    } else {
        node.attrs.remove("categorypath");
    }
    set_child_text(node, "TITLE", &definition.title);
    set_optional_child_text(
        node,
        "DESCRIPTION",
        (!definition.description.is_empty()).then_some(definition.description.as_str()),
    );
    set_category_memberships(node, category_memberships);
    set_conversion_node(node, definition.conversion.as_deref());
    set_bit_fields(
        node,
        definition.bit_offset,
        definition.bit_width,
        definition.bit_mask,
    );
    sync_axis_nodes(node, &definition.axes)?;
    sync_embedded_data(node, definition)
}

fn update_nth_parameter_node(
    node: &mut XmlNode,
    target: usize,
    definition: &XdfParameterDraft,
    category_memberships: &[(usize, u64)],
) -> Result<bool, XdfError> {
    let mut index = 0;
    for child in &mut node.children {
        if is_parameter_node(child) {
            if index == target {
                update_parameter_node(child, definition, category_memberships)?;
                return Ok(true);
            }
            index += 1;
        } else if update_nth_parameter_node(
            child,
            target - index,
            definition,
            category_memberships,
        )? {
            return Ok(true);
        } else {
            index += count_parameter_nodes(child);
        }
    }
    Ok(false)
}

fn count_parameter_nodes(node: &XmlNode) -> usize {
    node.children
        .iter()
        .map(|child| usize::from(is_parameter_node(child)) + count_parameter_nodes(child))
        .sum()
}

fn remove_nth_parameter_node(node: &mut XmlNode, target: usize) -> bool {
    let mut index = 0;
    let mut child_index = 0;
    while child_index < node.children.len() {
        if is_parameter_node(&node.children[child_index]) {
            if index == target {
                node.children.remove(child_index);
                return true;
            }
            index += 1;
        } else {
            let count = count_parameter_nodes(&node.children[child_index]);
            if target < index + count {
                return remove_nth_parameter_node(&mut node.children[child_index], target - index);
            }
            index += count;
        }
        child_index += 1;
    }
    false
}

fn reorder_root_parameters(
    root: &mut XmlNode,
    from: usize,
    to: usize,
    count: usize,
) -> Result<(), XdfError> {
    if from >= count || to >= count {
        return Err(invalid_field(
            "XDF",
            "parameter_order",
            to.to_string(),
            "target index is outside the parameter list",
        ));
    }
    if from == to {
        return Ok(());
    }
    let mut paths = Vec::new();
    collect_parameter_paths(root, &mut Vec::new(), &mut paths);
    if paths.len() != count {
        return Err(XdfError::Export {
            message: "source XML and normalized definitions disagree about parameter count"
                .to_string(),
        });
    }
    let from_path = &paths[from];
    let to_path = &paths[to];
    if from_path.len() != to_path.len()
        || from_path[..from_path.len() - 1] != to_path[..to_path.len() - 1]
    {
        return Err(XdfError::Export {
            message: "definitions can only be reordered within the same XDF container".to_string(),
        });
    }
    let parent_path = &from_path[..from_path.len() - 1];
    let parent = node_at_path_mut(root, parent_path).ok_or_else(|| XdfError::Export {
        message: "source XML container for reordered definition no longer exists".to_string(),
    })?;
    reorder_sibling_parameters(
        parent,
        from_path[from_path.len() - 1],
        to_path[to_path.len() - 1],
    )
}

fn collect_parameter_paths(node: &XmlNode, prefix: &mut Vec<usize>, output: &mut Vec<Vec<usize>>) {
    for (index, child) in node.children.iter().enumerate() {
        prefix.push(index);
        if is_parameter_node(child) {
            output.push(prefix.clone());
        }
        collect_parameter_paths(child, prefix, output);
        prefix.pop();
    }
}

fn node_at_path_mut<'a>(mut node: &'a mut XmlNode, path: &[usize]) -> Option<&'a mut XmlNode> {
    for index in path {
        node = node.children.get_mut(*index)?;
    }
    Some(node)
}

fn reorder_sibling_parameters(
    parent: &mut XmlNode,
    from_child_index: usize,
    to_child_index: usize,
) -> Result<(), XdfError> {
    let positions = parent
        .children
        .iter()
        .enumerate()
        .filter_map(|(index, child)| is_parameter_node(child).then_some(index))
        .collect::<Vec<_>>();
    let from = positions
        .iter()
        .position(|position| *position == from_child_index)
        .ok_or_else(|| XdfError::Export {
            message: "source XML parameter to reorder is missing".to_string(),
        })?;
    let to = positions
        .iter()
        .position(|position| *position == to_child_index)
        .ok_or_else(|| XdfError::Export {
            message: "target XML parameter position is missing".to_string(),
        })?;
    let mut parameters = positions
        .iter()
        .map(|position| parent.children[*position].clone())
        .collect::<Vec<_>>();
    let parameter = parameters.remove(from);
    parameters.insert(to, parameter);
    for (position, parameter) in positions.into_iter().zip(parameters) {
        parent.children[position] = parameter;
    }
    Ok(())
}

fn find_parameter_embedded_mut(node: &mut XmlNode) -> Option<&mut XmlNode> {
    for child in &mut node.children {
        if child.name == "XDFAXIS" {
            continue;
        }
        if child.name == "EMBEDDEDDATA" {
            return Some(child);
        }
        if let Some(descendant) = find_parameter_embedded_mut(child) {
            return Some(descendant);
        }
    }
    None
}

fn find_axis_node_mut<'a>(node: &'a mut XmlNode, id: &str) -> Option<&'a mut XmlNode> {
    for child in &mut node.children {
        if child.name == "XDFAXIS"
            && child
                .attrs
                .get("id")
                .is_some_and(|candidate| candidate == id)
        {
            return Some(child);
        }
        if let Some(axis) = find_axis_node_mut(child, id) {
            return Some(axis);
        }
    }
    None
}

fn remove_unlisted_axis_nodes(node: &mut XmlNode, retained_ids: &BTreeSet<&str>) {
    let mut index = 0;
    while index < node.children.len() {
        let remove = node.children[index].name == "XDFAXIS"
            && node.children[index]
                .attrs
                .get("id")
                .is_none_or(|id| !retained_ids.contains(id.as_str()));
        if remove {
            node.children.remove(index);
        } else {
            remove_unlisted_axis_nodes(&mut node.children[index], retained_ids);
            index += 1;
        }
    }
}

struct XmlParser {
    input: Vec<char>,
    position: usize,
}

impl XmlParser {
    fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            position: 0,
        }
    }

    fn parse_document(mut self) -> Result<XmlNode, XdfError> {
        self.skip_whitespace();
        while self.starts_with("<?") || self.starts_with("<!--") || self.starts_with("<!DOCTYPE") {
            if self.starts_with("<?") {
                self.skip_processing_instruction()?;
            } else if self.starts_with("<!--") {
                self.skip_comment()?;
            } else {
                self.skip_declaration()?;
            }
            self.skip_whitespace();
        }
        let root = self.parse_element()?;
        self.skip_whitespace();
        while self.position < self.input.len() {
            if self.starts_with("<!--") {
                self.skip_comment()?;
            } else if self.starts_with("<?") {
                self.skip_processing_instruction()?;
            } else if self.current() == Some('<') {
                return self.error("unexpected element after the XML root");
            } else if self.current().is_some() {
                return self.error("unexpected text after the XML root");
            }
            self.skip_whitespace();
        }
        Ok(root)
    }

    fn parse_element(&mut self) -> Result<XmlNode, XdfError> {
        self.expect_char('<')?;
        if self.current() == Some('/') {
            return self.error("unexpected closing element");
        }
        let name = self.parse_name()?.to_ascii_uppercase();
        let mut attrs = BTreeMap::new();
        loop {
            self.skip_whitespace();
            if self.consume_str("/>") {
                return Ok(XmlNode {
                    name,
                    attrs,
                    children: Vec::new(),
                    text: String::new(),
                });
            }
            if self.consume_char('>') {
                break;
            }
            let attr_name = self.parse_name()?.to_ascii_lowercase();
            self.skip_whitespace();
            self.expect_char('=')?;
            self.skip_whitespace();
            let quote = self
                .next()
                .ok_or_else(|| self.xml_error("missing attribute quote"))?;
            if quote != '"' && quote != '\'' {
                return self.error("attribute values must use single or double quotes");
            }
            let mut value = String::new();
            loop {
                let character = self
                    .next()
                    .ok_or_else(|| self.xml_error("unterminated attribute value"))?;
                if character == quote {
                    break;
                }
                value.push(character);
            }
            attrs.insert(attr_name, decode_entities(&value));
        }

        let mut children = Vec::new();
        let mut text = String::new();
        loop {
            if self.starts_with("</") {
                self.consume_str("</");
                let closing = self.parse_name()?.to_ascii_uppercase();
                self.skip_whitespace();
                self.expect_char('>')?;
                if closing != name {
                    return self.error(&format!(
                        "closing element </{closing}> does not match <{name}>"
                    ));
                }
                break;
            }
            if self.starts_with("<!--") {
                self.skip_comment()?;
                continue;
            }
            if self.starts_with("<?") {
                self.skip_processing_instruction()?;
                continue;
            }
            if self.starts_with("<![CDATA[") {
                self.consume_str("<![CDATA[");
                text.push_str(&self.read_until("]]>")?);
                continue;
            }
            if self.current() == Some('<') {
                children.push(self.parse_element()?);
                continue;
            }
            if self.position >= self.input.len() {
                return self.error("unterminated element");
            }
            let mut segment = String::new();
            while let Some(character) = self.current() {
                if character == '<' {
                    break;
                }
                segment.push(character);
                self.position += 1;
            }
            text.push_str(&decode_entities(&segment));
        }
        Ok(XmlNode {
            name,
            attrs,
            children,
            text,
        })
    }

    fn parse_name(&mut self) -> Result<String, XdfError> {
        let start = self.position;
        while let Some(character) = self.current() {
            if is_name_character(character) {
                self.position += 1;
            } else {
                break;
            }
        }
        if self.position == start {
            return self.error("expected an XML name");
        }
        Ok(self.input[start..self.position].iter().collect())
    }

    fn skip_comment(&mut self) -> Result<(), XdfError> {
        self.consume_str("<!--");
        self.read_until("-->").map(|_| ())
    }

    fn skip_processing_instruction(&mut self) -> Result<(), XdfError> {
        self.consume_str("<?");
        self.read_until("?>").map(|_| ())
    }

    fn skip_declaration(&mut self) -> Result<(), XdfError> {
        self.consume_str("<!");
        let mut bracket_depth = 0usize;
        let mut quote = None;
        while let Some(character) = self.next() {
            if let Some(expected) = quote {
                if character == expected {
                    quote = None;
                }
                continue;
            }
            if character == '\'' || character == '"' {
                quote = Some(character);
            } else if character == '[' {
                bracket_depth = bracket_depth.saturating_add(1);
            } else if character == ']' {
                bracket_depth = bracket_depth.saturating_sub(1);
            } else if character == '>' && bracket_depth == 0 {
                return Ok(());
            }
        }
        self.error("unterminated XML declaration")
    }

    fn read_until(&mut self, marker: &str) -> Result<String, XdfError> {
        let start = self.position;
        while self.position < self.input.len() {
            if self.starts_with(marker) {
                let value: String = self.input[start..self.position].iter().collect();
                self.position += marker.chars().count();
                return Ok(value);
            }
            self.position += 1;
        }
        Err(self.xml_error(&format!("missing XML terminator {marker:?}")))
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.current(), Some(' ' | '\n' | '\r' | '\t')) {
            self.position += 1;
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), XdfError> {
        if self.next() == Some(expected) {
            Ok(())
        } else {
            self.error(&format!("expected '{expected}'"))
        }
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.current() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn consume_str(&mut self, expected: &str) -> bool {
        if self.starts_with(expected) {
            self.position += expected.chars().count();
            true
        } else {
            false
        }
    }

    fn starts_with(&self, expected: &str) -> bool {
        let length = expected.chars().count();
        self.input
            .get(self.position..self.position.saturating_add(length))
            .map(|slice| slice.iter().copied().eq(expected.chars()))
            .unwrap_or(false)
    }

    fn current(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }

    fn next(&mut self) -> Option<char> {
        let value = self.current();
        if value.is_some() {
            self.position += 1;
        }
        value
    }

    fn xml_error(&self, message: &str) -> XdfError {
        XdfError::Xml {
            position: self.position,
            message: message.to_string(),
        }
    }

    fn error<T>(&self, message: &str) -> Result<T, XdfError> {
        Err(self.xml_error(message))
    }
}

fn is_name_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | ':' | '-' | '.')
}

fn decode_entities(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(start) = remaining.find('&') {
        output.push_str(&remaining[..start]);
        let after = &remaining[start + 1..];
        let Some(end) = after.find(';') else {
            output.push('&');
            remaining = after;
            continue;
        };
        let entity = &after[..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => {
                entity[1..].parse::<u32>().ok().and_then(char::from_u32)
            }
            _ => None,
        };
        if let Some(character) = decoded {
            output.push(character);
        } else {
            output.push('&');
            output.push_str(entity);
            output.push(';');
        }
        remaining = &after[end + 1..];
    }
    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
        <?xml version="1.0"?>
        <XDFFORMAT version="1.60">
          <XDFHEADER><flags>0x1</flags></XDFHEADER>
          <unknown future="safe"><nested>ignored</nested></unknown>
          <XDFCONSTANT uniqueid="0x10">
            <title>RPM &amp; Limit</title>
            <description>Scalar value</description>
            <CATEGORY index="2">Engine</CATEGORY>
            <EMBEDDEDDATA mmedaddress="0x10" mmedelementsizebits="16" />
            <XDFCONVERT><MATH equation="X * 0.25" /></XDFCONVERT>
          </XDFCONSTANT>
          <XDFTABLE>
            <title>Fuel Table</title>
            <XDFDATA><EMBEDDEDDATA mmedaddress="0x20" mmedelementsizebits="16" mmedrowcount="2" mmedcolcount="3" /></XDFDATA>
            <XDFAXIS id="x"><indexcount>3</indexcount><EMBEDDEDDATA mmedaddress="0x08" mmedelementsizebits="16" /></XDFAXIS>
            <XDFAXIS id="y"><indexcount>2</indexcount><EMBEDDEDDATA mmedaddress="0x0c" mmedelementsizebits="16" /></XDFAXIS>
          </XDFTABLE>
          <XDFBITFIELD uniqueid="flag-1" address="0x40" elementsizebits="8" bitoffset="2" bitwidth="1" />
        </XDFFORMAT>
    "#;

    #[test]
    fn export_materializes_effective_parameter_and_axis_formulas_without_touching_source() {
        let source = br#"<XDFFORMAT><XDFHEADER/><XDFVENDOR value="keep"/><XDFTABLE uniqueid="map" title="Map"><XDFCONVERT><MATH equation="X"/></XDFCONVERT><XDFAXIS id="x" indexcount="2"><XDFCONVERT><MATH equation="X + 1"/></XDFCONVERT><EMBEDDEDDATA mmedaddress="0" mmedelementsizebits="8" mmedtypeflags="0x06"/></XDFAXIS><EMBEDDEDDATA mmedaddress="0" mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="2" mmedmajorstridebits="16" mmedminorstridebits="8" mmedtypeflags="0x06"/></XDFTABLE></XDFFORMAT>"#;
        let mut document = XdfDocument::parse(source).unwrap();
        document.parameters[0].conversion = Some("X * 2".to_string());
        document.parameters[0].axes[0].conversion = Some("X + 3".to_string());

        let exported = document.to_xdf_text().unwrap();
        let reparsed = XdfDocument::parse(exported.as_bytes()).unwrap();
        assert_eq!(reparsed.parameters[0].conversion.as_deref(), Some("X * 2"));
        assert_eq!(
            reparsed.parameters[0].axes[0].conversion.as_deref(),
            Some("X + 3")
        );
        assert!(exported.contains("XDFVENDOR"));
        assert_eq!(document.exact_sha256, sha256_hex(source));
    }

    #[test]
    fn parses_known_objects_and_ignores_unknown_elements() {
        let document = XdfDocument::parse(FIXTURE.as_bytes()).unwrap();
        assert_eq!(document.parameters.len(), 3);
        assert!(document.unknown_element_count >= 1);
        assert_eq!(document.parameters[0].title, "RPM & Limit");
        assert_eq!(
            document.parameters[1].dimensions(),
            Dimensions {
                rows: 2,
                columns: 3
            }
        );
        assert_eq!(document.parameters[1].axes.len(), 2);
        assert_eq!(document.parameters[2].kind, ParameterKind::BitField);
        assert_eq!(document.parameters[2].bit_offset, Some(2));
        assert_eq!(document.parameters[2].bit_width, Some(1));
    }

    #[test]
    fn axis_stride_prefers_major_then_minor_then_element_width() {
        let document = XdfDocument::parse(
            br#"<XDFFORMAT>
                <XDFTABLE uniqueid="axis-table"><title>Axis table</title>
                  <EMBEDDEDDATA mmedaddress="0x100" mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="1" />
                  <XDFAXIS id="x"><indexcount>3</indexcount><EMBEDDEDDATA mmedaddress="0x20" mmedelementsizebits="8" mmedmajorstridebits="24" mmedminorstridebits="-8" /></XDFAXIS>
                  <XDFAXIS id="y"><indexcount>2</indexcount><EMBEDDEDDATA mmedaddress="0x30" mmedelementsizebits="16" mmedmajorstridebits="0" mmedminorstridebits="-16" /></XDFAXIS>
                  <XDFAXIS id="z"><indexcount>2</indexcount><EMBEDDEDDATA mmedaddress="0x40" mmedelementsizebits="8" mmedmajorstridebits="0" mmedminorstridebits="0" /></XDFAXIS>
                </XDFTABLE>
            </XDFFORMAT>"#,
        )
        .unwrap();
        let parameter = &document.parameters[0];
        assert_eq!(parameter.axes[0].stride_bits, 24);
        assert_eq!(
            parameter.axes[0].range,
            Some(ByteRange {
                start: 0x20,
                end: 0x27
            })
        );
        assert_eq!(
            parameter.axis_range(0, 2).unwrap(),
            ByteRange {
                start: 0x26,
                end: 0x27
            }
        );
        assert_eq!(parameter.axes[1].stride_bits, -16);
        assert_eq!(
            parameter.axis_range(1, 1).unwrap(),
            ByteRange {
                start: 0x2e,
                end: 0x30
            }
        );
        assert_eq!(parameter.axes[2].stride_bits, 8);
    }

    #[test]
    fn axis_read_conversion_and_transactional_write_use_declared_storage() {
        let document = XdfDocument::parse(
            br#"<XDFFORMAT><XDFTABLE uniqueid="axis-access"><title>Axis access</title>
              <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="1" />
              <XDFCONVERT><MATH equation="X" /></XDFCONVERT>
              <XDFAXIS id="x"><indexcount>2</indexcount><EMBEDDEDDATA mmedaddress="0x10" mmedelementsizebits="8" mmedmajorstridebits="8" /><XDFCONVERT><MATH equation="X * 0.5" /></XDFCONVERT></XDFAXIS>
              <XDFAXIS id="y"><indexcount>2</indexcount><EMBEDDEDDATA mmedaddress="0x20" mmedelementsizebits="16" mmedmajorstridebits="16" /></XDFAXIS>
            </XDFTABLE></XDFFORMAT>"#,
        )
        .unwrap();
        let parameter = &document.parameters[0];
        let mut bin = BinDocument::from_bytes(vec![0; 0x40]);
        {
            let mut transaction = bin.transaction("seed axes");
            transaction
                .write_uint(0x10, 1, Endianness::Little, 10)
                .unwrap();
            transaction
                .write_uint(0x11, 1, Endianness::Little, 20)
                .unwrap();
            transaction
                .write_uint(0x20, 2, Endianness::Little, 100)
                .unwrap();
            transaction
                .write_uint(0x22, 2, Endianness::Little, 200)
                .unwrap();
            transaction.commit().unwrap();
        }
        assert_eq!(
            parameter.read_raw_axis(&bin, 0, 1).unwrap(),
            RawValue::Unsigned(20)
        );
        assert_eq!(parameter.read_engineering_axis(&bin, 0, 1).unwrap(), 10.0);
        {
            let mut transaction = bin.transaction("write axis");
            let result = parameter
                .write_engineering_axis(&mut transaction, 0, 1, 12.5)
                .unwrap();
            assert_eq!(result.raw_value, RawValue::Unsigned(25));
            transaction.commit().unwrap();
        }
        assert_eq!(bin.read_uint(0x11, 1, Endianness::Little).unwrap(), 25);
        {
            let mut transaction = bin.transaction("quantized axis write");
            let result = parameter
                .write_engineering_axis(&mut transaction, 0, 1, 12.25)
                .unwrap();
            assert_eq!(result.requested_engineering, 12.25);
            assert_eq!(result.raw_value, RawValue::Unsigned(25));
            assert_eq!(result.stored_engineering, 12.5);
            transaction.commit().unwrap();
        }
        assert!(matches!(
            parameter.read_raw_axis(&bin, 2, 0),
            Err(CellAccessError::Mapping(XdfError::InvalidAxis { .. }))
        ));
        let error = parameter.read_raw_axis(&bin, 2, 0).unwrap_err();
        assert!(error.to_string().contains("outside valid count 2"));
    }

    #[test]
    fn axis_conversion_is_not_promoted_to_the_table_data_conversion() {
        let document = XdfDocument::parse(
            br#"<XDFFORMAT><XDFTABLE uniqueid="axis-conversion-scope"><title>Axis conversion scope</title>
              <XDFAXIS id="x"><indexcount>2</indexcount><XDFCONVERT><MATH equation="X * 0.5" /></XDFCONVERT>
                <EMBEDDEDDATA mmedaddress="0x10" mmedelementsizebits="8" mmedmajorstridebits="8" /></XDFAXIS>
              <XDFAXIS id="z"><EMBEDDEDDATA mmedaddress="0x20" mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="2" mmedmajorstridebits="8" mmedminorstridebits="8" /></XDFAXIS>
            </XDFTABLE></XDFFORMAT>"#,
        )
        .unwrap();

        let parameter = &document.parameters[0];
        assert_eq!(parameter.conversion, None);
        assert_eq!(parameter.axes[0].conversion.as_deref(), Some("X * 0.5"));
    }

    #[test]
    fn failed_quantized_axis_write_does_not_mutate_bin_or_undo_history() {
        let document = XdfDocument::parse(
            br#"<XDFFORMAT><XDFTABLE uniqueid="singular-axis"><title>Singular axis</title>
              <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="1" />
              <XDFAXIS id="x"><indexcount>1</indexcount><EMBEDDEDDATA mmedaddress="0x10" mmedelementsizebits="32" mmedtypeflags="0x10006" /><XDFCONVERT><MATH equation="(X + 1) / (X - 1)" /></XDFCONVERT></XDFAXIS>
            </XDFTABLE></XDFFORMAT>"#,
        )
        .unwrap();
        let parameter = &document.parameters[0];
        let mut bin = BinDocument::from_bytes(vec![0; 0x20]);
        let before = bin.bytes().to_vec();
        let undo_before = bin.undo_depth();
        let error = {
            let mut transaction = bin.transaction("singular axis");
            let error = parameter.write_engineering_axis(&mut transaction, 0, 0, 20_000_000_000.0);
            assert!(error.is_err());
            transaction.commit().unwrap();
            error
        };
        assert!(matches!(error, Err(CellAccessError::Conversion(_))));
        assert_eq!(bin.bytes(), before.as_slice());
        assert_eq!(bin.undo_depth(), undo_before);
    }

    #[test]
    fn descriptive_axis_is_not_writable() {
        let document = XdfDocument::parse(
            br#"<XDFFORMAT><XDFTABLE uniqueid="descriptive"><title>Descriptive</title>
              <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="1" />
              <XDFAXIS id="x"><indexcount>2</indexcount><LABEL index="0">A</LABEL><LABEL index="1">B</LABEL></XDFAXIS>
            </XDFTABLE></XDFFORMAT>"#,
        )
        .unwrap();
        let parameter = &document.parameters[0];
        let mut bin = BinDocument::from_bytes(vec![0; 4]);
        let before = bin.bytes().to_vec();
        let error = {
            let mut transaction = bin.transaction("descriptive axis");
            let error = parameter.write_raw_axis(&mut transaction, 0, 0, RawValue::Unsigned(1));
            assert!(error.is_err());
            error
        };
        assert_eq!(bin.bytes(), before.as_slice());
        assert_eq!(bin.undo_depth(), 0);
        assert!(matches!(
            error,
            Err(CellAccessError::UnsupportedLayout { .. })
        ));
    }

    #[test]
    fn exact_hash_and_normalized_fingerprint_have_different_purposes() {
        let first = XdfDocument::parse(FIXTURE.as_bytes()).unwrap();
        let reformatted = FIXTURE.replace("  <XDFHEADER>", "<XDFHEADER>\n          ");
        let second = XdfDocument::parse(reformatted.as_bytes()).unwrap();
        assert_eq!(first.exact_sha256, sha256_hex(FIXTURE.as_bytes()));
        assert_ne!(first.exact_sha256, second.exact_sha256);
        assert_eq!(first.normalized_fingerprint, second.normalized_fingerprint);
    }

    #[test]
    fn semantic_identity_does_not_include_backing_address() {
        let first = XdfDocument::parse(
            b"<XDFFORMAT><XDFCONSTANT><title>Same Name</title><address>0x10</address></XDFCONSTANT></XDFFORMAT>",
        )
        .unwrap();
        let second = XdfDocument::parse(
            b"<XDFFORMAT><XDFCONSTANT><title>Same Name</title><address>0x30</address></XDFCONSTANT></XDFFORMAT>",
        )
        .unwrap();
        assert_eq!(
            first.parameters[0].semantic_id,
            second.parameters[0].semantic_id
        );
        assert_eq!(first.normalized_fingerprint, second.normalized_fingerprint);
        assert_ne!(
            first.parameters[0].layout.address,
            second.parameters[0].layout.address
        );
    }

    #[test]
    fn validation_reports_only_ranges_outside_the_bin() {
        let document = XdfDocument::parse(FIXTURE.as_bytes()).unwrap();
        let report = document.validate_against(&[0; 0x30]);
        assert!(!report.is_valid());
        assert_eq!(report.issue_count(), 1);
        assert_eq!(report.issues[0].kind, ParameterKind::BitField);
        assert_eq!(report.issues[0].semantic_id, "bitfield:uid:flag-1");
        let valid = document.validate_against(&[0; 0x50]);
        assert!(valid.is_valid());
    }

    #[test]
    fn malformed_xml_fails_without_panicking() {
        let error = XdfDocument::parse(b"<XDFFORMAT><XDFCONSTANT></XDFFORMAT>").unwrap_err();
        assert!(matches!(error, XdfError::Xml { .. }));
    }

    #[test]
    fn numeric_entities_and_hex_numbers_are_supported() {
        let document = XdfDocument::parse(
            b"<XDF><XDFCONSTANT uniqueid=\"id\" address=\"$10\" elementsizebits=\"0x10\"><title>A&#x20;B</title></XDFCONSTANT></XDF>",
        )
        .unwrap();
        assert_eq!(document.parameters[0].title, "A B");
        assert_eq!(document.parameters[0].layout.address, 0x10);
        assert_eq!(document.parameters[0].layout.element_width_bits, 16);
    }

    #[test]
    fn table_payload_uses_addressed_z_axis_when_x_and_y_are_descriptive() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFTABLE uniqueid="0x20">
                    <title>Axis-backed table</title>
                    <XDFAXIS id="x">
                      <EMBEDDEDDATA mmedelementsizebits="8" mmedmajorstridebits="-16" />
                      <indexcount>2</indexcount>
                    </XDFAXIS>
                    <XDFAXIS id="y">
                      <EMBEDDEDDATA mmedelementsizebits="8" mmedmajorstridebits="-16" />
                      <indexcount>1</indexcount>
                    </XDFAXIS>
                    <XDFAXIS id="z">
                      <EMBEDDEDDATA mmedaddress="0x20" mmedelementsizebits="16" mmedrowcount="1" mmedcolcount="2" mmedmajorstridebits="0" mmedminorstridebits="16" />
                      <MATH equation="X * 0.5" />
                    </XDFAXIS>
                  </XDFTABLE>
                </XDFFORMAT>
            "#,
        )
        .unwrap();

        let parameter = &document.parameters[0];
        assert_eq!(parameter.layout.address, 0x20);
        assert_eq!(parameter.layout.element_width_bits, 16);
        assert_eq!(
            parameter.dimensions(),
            Dimensions {
                rows: 1,
                columns: 2
            }
        );
        assert_eq!(
            parameter.layout.range,
            ByteRange {
                start: 0x20,
                end: 0x24
            }
        );
        assert_eq!(parameter.conversion.as_deref(), Some("X * 0.5"));
        assert!(parameter.semantic_id.starts_with("table:semantic:"));
    }

    #[test]
    fn zero_table_strides_describe_packed_storage() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFTABLE uniqueid="0x40">
                    <title>Packed map</title>
                    <XDFAXIS id="z">
                      <EMBEDDEDDATA mmedaddress="0x40" mmedelementsizebits="16" mmedrowcount="4" mmedcolcount="6" mmedmajorstridebits="0" mmedminorstridebits="0" />
                    </XDFAXIS>
                  </XDFTABLE>
                </XDFFORMAT>
            "#,
        )
        .unwrap();

        let parameter = &document.parameters[0];
        assert_eq!(parameter.layout.row_stride_bits, 96);
        assert_eq!(parameter.layout.column_stride_bits, 16);
        assert_eq!(
            parameter.layout.range,
            ByteRange {
                start: 0x40,
                end: 0x70
            }
        );
    }

    #[test]
    fn column_major_packed_cells_follow_logical_rows_and_columns() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFTABLE uniqueid="column-major">
                    <XDFAXIS id="z">
                      <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8"
                          mmedrowcount="2" mmedcolcount="3"
                          mmedmajorstridebits="0" mmedminorstridebits="0"
                          mmedtypeflags="0x06" />
                    </XDFAXIS>
                  </XDFTABLE>
                </XDFFORMAT>
            "#,
        )
        .unwrap();
        let parameter = &document.parameters[0];
        let bin = BinDocument::from_bytes(vec![10, 20, 30, 40, 50, 60]);

        assert_eq!(parameter.layout.row_stride_bits, 8);
        assert_eq!(parameter.layout.column_stride_bits, 16);
        assert_eq!(
            parameter.cell_range(1, 0).unwrap(),
            ByteRange { start: 1, end: 2 }
        );
        assert_eq!(
            parameter.cell_range(0, 1).unwrap(),
            ByteRange { start: 2, end: 3 }
        );
        assert_eq!(
            parameter.read_raw_cell(&bin, 1, 0).unwrap(),
            RawValue::Unsigned(20)
        );
        assert_eq!(
            parameter.read_raw_cell(&bin, 0, 1).unwrap(),
            RawValue::Unsigned(30)
        );

        let mut bin = bin;
        {
            let mut transaction = bin.transaction("column-major cell edit");
            parameter
                .write_raw_cell(&mut transaction, 1, 2, RawValue::Unsigned(99))
                .unwrap();
            transaction.commit().unwrap();
        }
        assert_eq!(bin.bytes(), &[10, 20, 30, 40, 50, 99]);
    }

    #[test]
    fn repeated_address_like_unique_ids_get_deterministic_aliases() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFTABLE uniqueid="0x20">
                    <title>Repeated map</title>
                    <XDFAXIS id="z">
                      <EMBEDDEDDATA mmedaddress="0x20" mmedelementsizebits="16" mmedrowcount="1" mmedcolcount="1" />
                    </XDFAXIS>
                  </XDFTABLE>
                  <XDFTABLE uniqueid="0x20">
                    <title>Repeated map</title>
                    <XDFAXIS id="z">
                      <EMBEDDEDDATA mmedaddress="0x30" mmedelementsizebits="16" mmedrowcount="1" mmedcolcount="1" />
                    </XDFAXIS>
                  </XDFTABLE>
                </XDFFORMAT>
            "#,
        )
        .unwrap();

        let first = &document.parameters[0].semantic_id;
        let second = &document.parameters[1].semantic_id;
        assert!(first.starts_with("table:semantic:"));
        assert_eq!(second, &format!("{first}:alias:2"));
        assert_ne!(first, second);
    }

    #[test]
    fn cell_ranges_and_typed_reads_follow_strides() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFTABLE uniqueid="strided-map">
                    <title>Strided map</title>
                    <EMBEDDEDDATA mmedaddress="0x04" mmedelementsizebits="16" mmedrowcount="2" mmedcolcount="2" mmedmajorstridebits="48" mmedminorstridebits="16" signed="true" />
                  </XDFTABLE>
                </XDFFORMAT>
            "#,
        )
        .unwrap();
        let parameter = &document.parameters[0];
        let mut bytes = vec![0; 14];
        bytes[12] = 0x00;
        bytes[13] = 0x80;
        let bin = BinDocument::from_bytes(bytes);

        assert_eq!(
            parameter.cell_range(1, 1).unwrap(),
            ByteRange { start: 12, end: 14 }
        );
        assert_eq!(
            parameter.read_raw_cell(&bin, 1, 1).unwrap(),
            RawValue::Signed(-32768)
        );
    }

    #[test]
    fn cell_mapping_rejects_indexes_outside_dimensions() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFTABLE uniqueid="bounded-map">
                    <title>Bounded map</title>
                    <EMBEDDEDDATA mmedaddress="0x04" mmedelementsizebits="8" mmedrowcount="2" mmedcolcount="2" />
                  </XDFTABLE>
                </XDFFORMAT>
            "#,
        )
        .unwrap();

        assert!(matches!(
            document.parameters[0].cell_range(2, 0),
            Err(XdfError::InvalidCell { row: 2, .. })
        ));
        assert!(matches!(
            document.parameters[0].cell_range(0, 2),
            Err(XdfError::InvalidCell { column: 2, .. })
        ));
    }

    #[test]
    fn typed_cell_writes_commit_and_undo_as_one_operation() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFCONSTANT uniqueid="rpm" address="0x00" elementsizebits="16" />
                </XDFFORMAT>
            "#,
        )
        .unwrap();
        let parameter = &document.parameters[0];
        let mut bin = BinDocument::from_bytes(vec![0, 0]);

        let summary = {
            let mut transaction = bin.transaction("edit rpm");
            parameter
                .write_raw_cell(&mut transaction, 0, 0, RawValue::Unsigned(0x1234))
                .unwrap();
            transaction.commit().unwrap()
        };
        assert_eq!(summary.changed_bytes, 2);
        assert_eq!(bin.bytes(), &[0x34, 0x12]);
        assert_eq!(
            parameter.read_raw_cell(&bin, 0, 0).unwrap(),
            RawValue::Unsigned(0x1234)
        );

        bin.undo().unwrap();
        assert_eq!(bin.bytes(), &[0, 0]);
        bin.redo().unwrap();
        assert_eq!(bin.bytes(), &[0x34, 0x12]);
    }

    #[test]
    fn bitfield_writes_preserve_unselected_bits() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFBITFIELD uniqueid="flag" address="0x00" elementsizebits="8" bitoffset="2" bitwidth="3" />
                </XDFFORMAT>
            "#,
        )
        .unwrap();
        let parameter = &document.parameters[0];
        let mut bin = BinDocument::from_bytes(vec![0xA7]);

        assert_eq!(
            parameter.read_raw_cell(&bin, 0, 0).unwrap(),
            RawValue::Unsigned(1)
        );
        {
            let mut transaction = bin.transaction("edit flag");
            parameter
                .write_raw_cell(&mut transaction, 0, 0, RawValue::Unsigned(5))
                .unwrap();
            transaction.commit().unwrap();
        }
        assert_eq!(bin.bytes(), &[0xB7]);
        assert_eq!(
            parameter.read_raw_cell(&bin, 0, 0).unwrap(),
            RawValue::Unsigned(5)
        );
    }

    #[test]
    fn normalizes_header_and_category_metadata() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT version="1.70">
                  <XDFHEADER>
                    <DEFAULTS datasizeinbits="16" sigdigits="4" outputtype="1" signed="0" lsbfirst="1" float="0" />
                    <BASEOFFSET offset="0x2000" subtract="0" />
                    <CATEGORY index="0x2" name="Fuel" />
                  </XDFHEADER>
                  <XDFCONSTANT uniqueid="value">
                    <title>Value</title>
                    <CATEGORYMEM index="0" category="2" />
                    <EMBEDDEDDATA mmedaddress="0x10" />
                  </XDFCONSTANT>
                </XDFFORMAT>
            "#,
        )
        .unwrap();

        assert_eq!(document.header.defaults.data_size_bits, Some(16));
        assert_eq!(document.header.base_offset.translate(0x10).unwrap(), 0x2010);
        assert_eq!(document.categories[0].name, "Fuel");
        assert_eq!(
            document.parameters[0].category_memberships[0].category_index,
            2
        );
        assert_eq!(
            document.parameters[0].category_memberships[0]
                .category_name
                .as_deref(),
            Some("Fuel")
        );
    }

    #[test]
    #[ignore = "requires the local-only SCGa05 XDF fixture"]
    fn detects_one_based_category_positions_in_the_supplied_fixture() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("Test bin and xdf")
            .join("SCGa05_cal.xdf");
        let document = XdfDocument::load(path).unwrap();

        assert_eq!(
            document.category_reference_mode,
            CategoryReferenceMode::OneBasedPosition
        );
        assert!(document
            .parameters
            .iter()
            .flat_map(|parameter| parameter.category_memberships.iter())
            .any(|membership| {
                membership.category_index == 57
                    && membership.resolved_category_index == Some(56)
                    && membership.category_name.is_some()
            }));
        assert!(!document
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing-category"));
    }

    #[test]
    fn keeps_declared_indices_when_one_based_mode_has_no_decisive_evidence() {
        let document = XdfDocument::parse(
            br#"<XDFFORMAT><XDFHEADER>
              <CATEGORY index="0" name="Axis" />
              <CATEGORY index="1" name="Airflow" />
              <CATEGORY index="2" name="Fuel" />
            </XDFHEADER><XDFCONSTANT>
              <CATEGORYMEM index="0" category="2" />
              <EMBEDDEDDATA mmedaddress="0x00" />
            </XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();

        assert_eq!(
            document.category_reference_mode,
            CategoryReferenceMode::DeclaredIndex
        );
        assert_eq!(document.parameters[0].category.as_deref(), Some("Fuel"));
        assert_eq!(
            document.parameters[0].category_memberships[0].resolved_category_index,
            Some(2)
        );
    }

    #[test]
    fn category_path_orders_memberships_by_slot_and_omits_unresolved_names() {
        let document = XdfDocument::parse(
            br#"<XDFFORMAT><XDFHEADER>
              <CATEGORY index="0" name="Axis" />
              <CATEGORY index="1" name="Airflow" />
            </XDFHEADER><XDFCONSTANT>
              <CATEGORYMEM index="2" category="99" />
              <CATEGORYMEM index="0" category="0" />
              <CATEGORYMEM index="1" category="1" />
              <EMBEDDEDDATA mmedaddress="0x00" />
            </XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();

        assert_eq!(
            document.parameters[0].category_path(),
            vec!["Axis".to_string(), "Airflow".to_string()]
        );
    }

    #[test]
    fn base_offset_subtraction_is_checked() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFHEADER><BASEOFFSET offset="0x20" subtract="1" /></XDFHEADER>
                  <XDFCONSTANT address="0x20" />
                </XDFFORMAT>
            "#,
        )
        .unwrap();

        assert_eq!(document.header.base_offset.translate(0x20).unwrap(), 0);
        assert!(document.header.base_offset.translate(0x1).is_err());
    }

    #[test]
    fn invalid_header_fields_are_fatal_and_missing_categories_are_diagnostic() {
        let invalid = XdfDocument::parse(
            br#"<XDFFORMAT><XDFHEADER><DEFAULTS signed="sometimes" /></XDFHEADER></XDFFORMAT>"#,
        );
        assert!(matches!(invalid, Err(XdfError::InvalidField { .. })));

        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFCONSTANT>
                    <CATEGORYMEM index="0" category="99" />
                    <EMBEDDEDDATA mmedaddress="0x00" />
                  </XDFCONSTANT>
                </XDFFORMAT>
            "#,
        )
        .unwrap();
        assert!(document
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing-category"));

        let outside_region = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFHEADER>
                    <REGION startaddress="0x10" size="0x10" />
                  </XDFHEADER>
                  <XDFCONSTANT>
                    <EMBEDDEDDATA mmedaddress="0x30" mmedelementsizebits="8" />
                  </XDFCONSTANT>
                </XDFFORMAT>
            "#,
        )
        .unwrap();
        assert!(outside_region
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "outside-region"));
    }

    #[test]
    fn decodes_storage_flags_and_preserves_signed_strides() {
        let integer = XdfDocument::parse(
            br#"<XDFFORMAT><XDFCONSTANT>
                <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="16" mmedtypeflags="0x03" />
            </XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();
        assert!(integer.parameters[0].layout.signed);

        let float = XdfDocument::parse(
            br#"<XDFFORMAT><XDFCONSTANT>
                <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="32" mmedtypeflags="0x10006" />
            </XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();
        assert_eq!(
            float.parameters[0].layout.storage.numeric_kind,
            NumericKind::Ieee754Binary32
        );
        assert!(float.parameters[0].layout.storage.column_major);

        let column_major = XdfDocument::parse(
            br#"<XDFFORMAT><XDFCONSTANT>
                <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8" mmedtypeflags="0x06" />
            </XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();
        assert_eq!(
            column_major.parameters[0].layout.storage.numeric_kind,
            NumericKind::Integer
        );
        assert!(column_major.parameters[0].layout.storage.column_major);
        assert!(!column_major.parameters[0].layout.storage.signed);
        assert!(float
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "unsupported-storage"));

        let strided = XdfDocument::parse(
            br#"<XDFFORMAT><XDFTABLE>
                <XDFAXIS id="z"><EMBEDDEDDATA
                    mmedaddress="0x10" mmedelementsizebits="8"
                    mmedrowcount="2" mmedcolcount="2"
                    mmedmajorstridebits="-8" mmedminorstridebits="8"
                /></XDFAXIS>
            </XDFTABLE></XDFFORMAT>"#,
        )
        .unwrap();
        let parameter = &strided.parameters[0];
        assert_eq!(parameter.layout.row_stride_bits, -8);
        assert_eq!(parameter.layout.column_stride_bits, 8);
        assert_eq!(
            parameter.cell_range(1, 0).unwrap(),
            ByteRange {
                start: 0x0f,
                end: 0x10
            }
        );
    }

    #[test]
    fn flags_that_do_not_describe_supported_float_storage_are_diagnostic() {
        let wrong_width = XdfDocument::parse(
            br#"<XDFFORMAT><XDFCONSTANT>
                <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="16" mmedtypeflags="0x10006" />
            </XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();
        assert!(wrong_width
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "unsupported-storage" }));

        let unknown = XdfDocument::parse(
            br#"<XDFFORMAT><XDFCONSTANT>
                <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8" mmedtypeflags="0x20002" />
            </XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();
        assert!(unknown
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "unknown-storage-flags" }));
    }

    #[test]
    fn preserves_axis_metadata_labels_links_and_embed_info() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFTABLE uniqueid="table">
                    <XDFAXIS id="x">
                      <units>rpm</units>
                      <unittype>2</unittype>
                      <decimalpl>1</decimalpl>
                      <min>0.0</min>
                      <max>8000.0</max>
                      <outputtype>1</outputtype>
                      <LABEL index="0">zero</LABEL>
                      <LABEL index="2">two</LABEL>
                      <DALINK index="7" objectid="missing-axis" />
                      <embedinfo type="1" region="calibration" />
                    </XDFAXIS>
                    <XDFAXIS id="y">
                      <EMBEDDEDDATA mmedaddress="0x20" mmedelementsizebits="8" />
                    </XDFAXIS>
                    <XDFAXIS id="z">
                      <EMBEDDEDDATA mmedaddress="0x30" mmedelementsizebits="8"
                          mmedrowcount="2" mmedcolcount="3"
                          mmedmajorstridebits="24" mmedminorstridebits="8" />
                    </XDFAXIS>
                  </XDFTABLE>
                </XDFFORMAT>
            "#,
        )
        .unwrap();
        let axes = &document.parameters[0].axes;
        let x_axis = axes.iter().find(|axis| axis.id == "x").unwrap();
        assert_eq!(x_axis.address, None);
        assert_eq!(x_axis.metadata.units.as_deref(), Some("rpm"));
        assert_eq!(x_axis.metadata.unit_type, Some(2));
        assert_eq!(x_axis.metadata.decimal_places, Some(1));
        assert_eq!(x_axis.metadata.min, Some(0.0));
        assert_eq!(x_axis.metadata.max, Some(8000.0));
        assert_eq!(x_axis.labels[0].index, 0);
        assert_eq!(x_axis.labels[1].index, 2);
        assert_eq!(x_axis.links[0].index, Some(7));
        assert_eq!(
            x_axis.links[0].object_id_hash.as_deref(),
            Some("missing-axis")
        );
        assert_eq!(x_axis.embed_info.as_ref().unwrap().type_code, Some(1));
        assert_eq!(
            x_axis
                .embed_info
                .as_ref()
                .unwrap()
                .attributes
                .get("region")
                .map(String::as_str),
            Some("calibration")
        );
        assert!(document
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "label-gap"));
        assert!(document
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unresolved-axis-link"));
    }

    #[test]
    fn classifies_flags_and_retains_auxiliary_objects() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFFLAG uniqueid="enabled">
                    <title>Enabled</title>
                    <EMBEDDEDDATA mmedaddress="0x10" mmedelementsizebits="8" />
                    <MASK>0x04</MASK>
                  </XDFFLAG>
                  <XDFFUNCTION uniqueid="fn">
                    <title>Fuel function</title>
                    <description>not a cell</description>
                  </XDFFUNCTION>
                  <XDFPATCH uniqueid="patch"><title>Patch</title></XDFPATCH>
                  <XDFCHECKSUM uniqueid="checksum"><title>Checksum</title></XDFCHECKSUM>
                </XDFFORMAT>
            "#,
        )
        .unwrap();
        assert_eq!(document.parameters.len(), 1);
        assert_eq!(document.parameters[0].kind, ParameterKind::Flag);
        assert_eq!(document.parameters[0].bit_mask, Some(0x04));
        assert_eq!(document.parameters[0].bit_offset, Some(2));
        assert_eq!(document.parameters[0].bit_width, Some(1));
        assert_eq!(document.auxiliary_objects.len(), 3);
        assert_eq!(
            document.auxiliary_objects[0].kind,
            XdfAuxiliaryKind::Function
        );
        assert_eq!(document.auxiliary_objects[0].source_name, "XDFFUNCTION");
        assert_eq!(document.auxiliary_objects[1].kind, XdfAuxiliaryKind::Patch);
        assert_eq!(document.auxiliary_objects[2].title, "Checksum");
    }

    fn parse_constant(
        address: &str,
        width_bits: &str,
        type_flags: &str,
        equation: &str,
    ) -> XdfDocument {
        let source = format!(
            r#"<XDFFORMAT><XDFCONSTANT>
                <XDFCONVERT><MATH equation="{equation}" /></XDFCONVERT>
                <EMBEDDEDDATA mmedaddress="{address}" mmedelementsizebits="{width_bits}" mmedtypeflags="{type_flags}" />
            </XDFCONSTANT></XDFFORMAT>"#
        );
        XdfDocument::parse(source.as_bytes()).unwrap()
    }

    #[test]
    fn engineering_integer_write_is_integral_and_atomic() {
        let document = parse_constant("0x00", "16", "0x02", "2 * X + 1");
        let parameter = &document.parameters[0];
        let mut bin = BinDocument::from_bytes(vec![0, 0]);
        let before = bin.bytes().to_vec();
        let mut transaction = bin.transaction("engineering edit");
        let result = parameter
            .write_engineering_cell(&mut transaction, 0, 0, 5.0)
            .unwrap();
        assert_eq!(result.raw_value, RawValue::Unsigned(2));
        transaction.commit().unwrap();
        assert_ne!(bin.bytes(), before.as_slice());
    }

    #[test]
    fn binary32_read_and_write_use_ieee_bits() {
        let document = parse_constant("0x00", "32", "0x10006", "X / 2");
        let parameter = &document.parameters[0];
        let mut bin = BinDocument::from_bytes(1.5f32.to_le_bytes().to_vec());
        assert_eq!(parameter.read_engineering_cell(&bin, 0, 0).unwrap(), 0.75);
        let mut transaction = bin.transaction("float engineering edit");
        let result = parameter
            .write_engineering_cell(&mut transaction, 0, 0, 1.25)
            .unwrap();
        assert_eq!(result.raw_value, RawValue::Float32Bits(2.5f32.to_bits()));
        transaction.commit().unwrap();
    }

    #[test]
    fn fractional_integer_inverse_rounds_to_nearest_raw_and_reports_stored_value() {
        let document = parse_constant("0x00", "16", "0x02", "2 * X + 1");
        let parameter = &document.parameters[0];
        let mut bin = BinDocument::from_bytes(vec![0, 0]);
        let mut transaction = bin.transaction("fractional engineering edit");
        let result = parameter
            .write_engineering_cell(&mut transaction, 0, 0, 5.5)
            .unwrap();
        assert_eq!(result.requested_engineering, 5.5);
        assert_eq!(result.raw_value, RawValue::Unsigned(2));
        assert_eq!(result.stored_engineering, 5.0);
        transaction.commit().unwrap();
        assert_eq!(bin.bytes(), &[2, 0]);
    }

    #[test]
    fn rounded_integer_write_still_rejects_values_outside_element_width_atomically() {
        let document = parse_constant("0x00", "16", "0x02", "2 * X + 1");
        let parameter = &document.parameters[0];
        let mut bin = BinDocument::from_bytes(vec![0x34, 0x12]);
        let before = bin.bytes().to_vec();
        let error = {
            let mut transaction = bin.transaction("out-of-width engineering edit");
            let error = parameter.write_engineering_cell(&mut transaction, 0, 0, 131_073.0);
            drop(transaction);
            error
        };
        assert!(error.is_err());
        assert_eq!(bin.bytes(), before.as_slice());
        assert_eq!(bin.undo_depth(), 0);
    }

    #[test]
    fn non_finite_float_values_are_raw_visible_but_not_engineering_editable() {
        let document = parse_constant("0x00", "32", "0x10006", "X");
        let parameter = &document.parameters[0];
        let nan = f32::NAN.to_bits();
        let bin = BinDocument::from_bytes(nan.to_le_bytes().to_vec());
        assert_eq!(
            parameter.read_raw_cell(&bin, 0, 0).unwrap(),
            RawValue::Float32Bits(nan)
        );
        assert!(parameter.read_engineering_cell(&bin, 0, 0).is_err());
        let mut bin = BinDocument::from_bytes(vec![0; 4]);
        let before = bin.bytes().to_vec();
        {
            let mut transaction = bin.transaction("non-finite engineering edit");
            assert!(parameter
                .write_engineering_cell(&mut transaction, 0, 0, f64::INFINITY)
                .is_err());
        }
        assert_eq!(bin.bytes(), before.as_slice());
    }

    #[test]
    fn authoring_adds_strided_table_with_axis_and_round_trips_header() {
        let mut document = XdfDocument::new("Draft XDF").unwrap();
        let definition = XdfParameterDraft {
            unique_id: Some("load-map".into()),
            kind: ParameterKind::Table,
            title: "Load Map".into(),
            description: "Created from a map candidate".into(),
            category: Some("Fuel".into()),
            category_memberships: Vec::new(),
            xdf_address: 0x20,
            element_width_bits: 16,
            dimensions: Dimensions {
                rows: 2,
                columns: 3,
            },
            signed: true,
            endianness: Endianness::Little,
            numeric_kind: NumericKind::Integer,
            column_major: false,
            row_stride_bits: 64,
            column_stride_bits: 16,
            conversion: Some("X / 10".into()),
            bit_offset: None,
            bit_width: None,
            bit_mask: None,
            unknown_type_flags: 0,
            axes: vec![XdfAxisDraft {
                id: "x".into(),
                title: "RPM".into(),
                count: 3,
                address: Some(0x08),
                element_width_bits: 16,
                stride_bits: 16,
                signed: false,
                endianness: Endianness::Little,
                numeric_kind: NumericKind::Integer,
                unknown_type_flags: 0,
                conversion: None,
                metadata: AxisMetadata {
                    units: Some("rpm".into()),
                    unit_type: None,
                    decimal_places: Some(0),
                    min: None,
                    max: None,
                    output_type: None,
                },
                labels: Vec::new(),
            }],
        };
        document
            .apply_authoring_operations(&[
                XdfAuthoringOperation::SetHeader {
                    header: XdfHeader {
                        base_offset: XdfBaseOffset {
                            offset: 0x10,
                            subtract: false,
                        },
                        ..XdfHeader::default()
                    },
                },
                XdfAuthoringOperation::SetCategories {
                    categories: vec![XdfCategory {
                        index: 0,
                        name: "Fuel".into(),
                    }],
                },
                XdfAuthoringOperation::AddParameter {
                    definition: definition.clone(),
                },
            ])
            .unwrap();

        let parameter = &document.parameters[0];
        assert_eq!(parameter.title, "Load Map");
        assert_eq!(parameter.layout.address, 0x30);
        assert_eq!(document.xdf_address_for_bin_offset(0x30).unwrap(), 0x20);
        assert_eq!(parameter.layout.row_stride_bits, 64);
        assert_eq!(parameter.layout.column_stride_bits, 16);
        assert_eq!(parameter.axes[0].address, Some(0x18));
        assert_eq!(parameter.axes[0].metadata.units.as_deref(), Some("rpm"));
        assert_eq!(parameter.category.as_deref(), Some("Fuel"));

        let load_id = parameter.semantic_id.clone();
        let mut edited = document.authoring_draft(&load_id).unwrap();
        edited.element_width_bits = 8;
        edited.row_stride_bits = 32;
        edited.column_stride_bits = 8;
        document
            .apply_authoring_operations(&[XdfAuthoringOperation::ReplaceParameter {
                semantic_id: load_id,
                definition: edited,
            }])
            .unwrap();
        assert_eq!(document.parameters[0].layout.element_width_bits, 8);
        assert_eq!(document.parameters[0].axes[0].element_width_bits, 16);

        let reparsed = XdfDocument::parse(document.to_xdf_text().unwrap().as_bytes()).unwrap();
        assert_eq!(reparsed.parameters, document.parameters);
        assert_eq!(reparsed.header.base_offset.offset, 0x10);
        assert_eq!(reparsed.categories[0].name, "Fuel");

        let mut second = definition;
        second.unique_id = Some("second-map".into());
        second.title = "Second Map".into();
        second.xdf_address = 0x40;
        document
            .apply_authoring_operations(&[XdfAuthoringOperation::AddParameter {
                definition: second,
            }])
            .unwrap();
        let second_id = document.parameters[1].semantic_id.clone();
        document
            .apply_authoring_operations(&[XdfAuthoringOperation::ReorderParameter {
                semantic_id: second_id,
                index: 0,
            }])
            .unwrap();
        assert_eq!(document.parameters[0].title, "Second Map");
        assert_eq!(document.parameters[1].title, "Load Map");
    }

    #[test]
    fn authoring_updates_and_deletes_by_identity_without_dropping_vendor_xml() {
        let source = XdfDocument::parse(
            br#"<XDFFORMAT><XDFHEADER><deftitle>Source</deftitle><VENDORHEADER keep="yes" /></XDFHEADER><XDFTABLE uniqueid="map"><TITLE>Old</TITLE><DESCRIPTION>old</DESCRIPTION><XDFCONVERT><MATH equation="X" /></XDFCONVERT><VENDORAXES><XDFAXIS id="x" indexcount="2"><TITLE>RPM</TITLE><EMBEDDEDDATA mmedaddress="0x08" mmedelementsizebits="8" mmedtypeflags="0x02" mmedmajorstridebits="8" vendoraxis="keep" /></XDFAXIS></VENDORAXES><EMBEDDEDDATA mmedaddress="0x10" mmedelementsizebits="16" mmedrowcount="1" mmedcolcount="2" mmedmajorstridebits="32" mmedminorstridebits="16" mmedtypeflags="0x02" /><VENDORTABLE keep="yes" /></XDFTABLE><XDFCONSTANT uniqueid="other"><TITLE>Keep</TITLE><EMBEDDEDDATA mmedaddress="0x30" mmedelementsizebits="8" mmedtypeflags="0x02" /></XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();
        let semantic_id = source.parameters[0].semantic_id.clone();
        let mut definition = source.authoring_draft(&semantic_id).unwrap();
        definition.title = "Renamed".into();
        definition.xdf_address = 0x20;
        definition.element_width_bits = 8;
        definition.endianness = Endianness::Big;
        definition.signed = false;
        definition.row_stride_bits = 48;
        definition.conversion = Some("X * 2".into());
        let mut document = source;
        document
            .apply_authoring_operations(&[XdfAuthoringOperation::ReplaceParameter {
                semantic_id: semantic_id.clone(),
                definition,
            }])
            .unwrap();
        let exported = document.to_xdf_text().unwrap();
        assert!(exported.contains("<VENDORHEADER") && exported.contains("keep=\"yes\""));
        assert!(exported.contains("<VENDORTABLE") && exported.contains("keep=\"yes\""));
        assert!(exported.contains("vendoraxis=\"keep\""));
        assert_eq!(document.parameters[0].title, "Renamed");
        assert_eq!(document.parameters[0].layout.address, 0x20);
        assert_eq!(document.parameters[0].layout.element_width_bits, 8);
        assert_eq!(document.parameters[0].axes.len(), 1);
        assert_eq!(document.parameters[0].layout.endianness, Endianness::Big);
        assert!(!document.parameters[0].layout.signed);
        assert_eq!(document.parameters[0].layout.row_stride_bits, 48);
        assert_eq!(document.parameters[0].conversion.as_deref(), Some("X * 2"));

        document
            .apply_authoring_operations(&[XdfAuthoringOperation::DeleteParameter { semantic_id }])
            .unwrap();
        assert_eq!(document.parameters.len(), 1);
        assert_eq!(document.parameters[0].title, "Keep");
        let exported = document.to_xdf_text().unwrap();
        assert!(exported.contains("<VENDORHEADER") && exported.contains("keep=\"yes\""));
    }

    #[test]
    fn authoring_rejects_unsupported_numeric_storage_without_partial_changes() {
        let mut document = XdfDocument::new("Draft").unwrap();
        let before = document.to_xdf_text().unwrap();
        let result = document.apply_authoring_operations(&[XdfAuthoringOperation::AddParameter {
            definition: XdfParameterDraft {
                unique_id: Some("unsupported-float".into()),
                kind: ParameterKind::Table,
                title: "Unknown float".into(),
                description: String::new(),
                category: None,
                category_memberships: Vec::new(),
                xdf_address: 0,
                element_width_bits: 64,
                dimensions: Dimensions {
                    rows: 1,
                    columns: 1,
                },
                signed: false,
                endianness: Endianness::Little,
                numeric_kind: NumericKind::Unsupported,
                column_major: false,
                row_stride_bits: 64,
                column_stride_bits: 64,
                conversion: None,
                bit_offset: None,
                bit_width: None,
                bit_mask: None,
                unknown_type_flags: 0,
                axes: Vec::new(),
            },
        }]);
        assert!(result.is_err());
        assert!(document.parameters.is_empty());
        assert_eq!(document.to_xdf_text().unwrap(), before);
    }

    #[test]
    fn authoring_rejects_multi_cell_dimensions_for_scalar_definition_kinds() {
        let mut document = XdfDocument::new("Draft").unwrap();
        let result = document.apply_authoring_operations(&[XdfAuthoringOperation::AddParameter {
            definition: XdfParameterDraft {
                unique_id: Some("scalar-array".into()),
                kind: ParameterKind::Constant,
                title: "Invalid scalar array".into(),
                description: String::new(),
                category: None,
                category_memberships: Vec::new(),
                xdf_address: 0,
                element_width_bits: 16,
                dimensions: Dimensions {
                    rows: 2,
                    columns: 1,
                },
                signed: false,
                endianness: Endianness::Little,
                numeric_kind: NumericKind::Integer,
                column_major: false,
                row_stride_bits: 16,
                column_stride_bits: 16,
                conversion: None,
                bit_offset: None,
                bit_width: None,
                bit_mask: None,
                unknown_type_flags: 0,
                axes: Vec::new(),
            },
        }]);
        assert!(result.is_err());
        assert!(document.parameters.is_empty());
    }

    #[test]
    fn authoring_rejects_bit_offset_outside_storage_even_without_explicit_bit_width() {
        let mut document = XdfDocument::new("Draft").unwrap();
        let result = document.apply_authoring_operations(&[XdfAuthoringOperation::AddParameter {
            definition: XdfParameterDraft {
                unique_id: Some("invalid-bitfield".into()),
                kind: ParameterKind::BitField,
                title: "Invalid bitfield".into(),
                description: String::new(),
                category: None,
                category_memberships: Vec::new(),
                xdf_address: 0,
                element_width_bits: 8,
                dimensions: Dimensions {
                    rows: 1,
                    columns: 1,
                },
                signed: false,
                endianness: Endianness::Little,
                numeric_kind: NumericKind::Integer,
                column_major: false,
                row_stride_bits: 8,
                column_stride_bits: 8,
                conversion: None,
                bit_offset: Some(8),
                bit_width: None,
                bit_mask: None,
                unknown_type_flags: 0,
                axes: Vec::new(),
            },
        }]);
        assert!(result.is_err());
        assert!(document.parameters.is_empty());
    }

    #[test]
    fn authoring_preserves_multiple_category_memberships_and_category_extensions() {
        let mut document = XdfDocument::parse(
            br#"<XDFFORMAT><XDFHEADER><CATEGORY index="0" name="Fuel" vendor-category="keep"><VENDORCATEGORY note="keep" /></CATEGORY><CATEGORY index="1" name="Spark" /></XDFHEADER><XDFTABLE uniqueid="map"><TITLE>Map</TITLE><CATEGORYMEM index="0" category="0" /><CATEGORYMEM index="1" category="1" vendor-membership="keep"><VENDORMEMBERSHIP note="keep" /></CATEGORYMEM><EMBEDDEDDATA mmedaddress="0x10" mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="2" mmedmajorstridebits="16" mmedminorstridebits="8" mmedtypeflags="0x02" /></XDFTABLE></XDFFORMAT>"#,
        )
        .unwrap();
        let semantic_id = document.parameters[0].semantic_id.clone();
        let mut definition = document.authoring_draft(&semantic_id).unwrap();
        assert_eq!(definition.category_memberships.len(), 2);
        definition.title = "Edited map".into();
        document
            .apply_authoring_operations(&[XdfAuthoringOperation::ReplaceParameter {
                semantic_id,
                definition,
            }])
            .unwrap();
        assert_eq!(document.parameters[0].category_memberships.len(), 2);
        assert_eq!(
            document.parameters[0].category_memberships[1]
                .category_name
                .as_deref(),
            Some("Spark")
        );

        let mut categories = document.categories.clone();
        categories[0].name = "Fuel Revised".into();
        document
            .apply_authoring_operations(&[XdfAuthoringOperation::SetCategories { categories }])
            .unwrap();
        let exported = document.to_xdf_text().unwrap();
        assert!(exported.contains("vendor-category=\"keep\""));
        assert!(exported.contains("<VENDORCATEGORY"));
        assert!(exported.contains("vendor-membership=\"keep\""));
        assert!(exported.contains("<VENDORMEMBERSHIP"));
        assert_eq!(document.parameters[0].category_memberships.len(), 2);
        assert_eq!(
            document.parameters[0].category.as_deref(),
            Some("Fuel Revised")
        );
    }

    #[test]
    fn authoring_reorders_sibling_definitions_inside_a_preserved_container() {
        let mut document = XdfDocument::parse(
            br#"<XDFFORMAT><XDFHEADER/><VENDORGROUP keep="yes"><XDFCONSTANT uniqueid="a"><TITLE>A</TITLE><EMBEDDEDDATA mmedaddress="0" mmedelementsizebits="8" mmedtypeflags="0x02" /></XDFCONSTANT><VENDORSEPARATOR keep="position" /><XDFCONSTANT uniqueid="b"><TITLE>B</TITLE><EMBEDDEDDATA mmedaddress="1" mmedelementsizebits="8" mmedtypeflags="0x02" /></XDFCONSTANT></VENDORGROUP></XDFFORMAT>"#,
        )
        .unwrap();
        let second = document.parameters[1].semantic_id.clone();
        document
            .apply_authoring_operations(&[XdfAuthoringOperation::ReorderParameter {
                semantic_id: second,
                index: 0,
            }])
            .unwrap();
        assert_eq!(document.parameters[0].title, "B");
        assert_eq!(document.parameters[1].title, "A");
        let exported = document.to_xdf_text().unwrap();
        assert!(exported.contains("<VENDORGROUP") && exported.contains("keep=\"yes\""));
        let second_map = exported.find("uniqueid=\"b\"").unwrap();
        let separator = exported.find("VENDORSEPARATOR").unwrap();
        let first_map = exported.find("uniqueid=\"a\"").unwrap();
        assert!(second_map < separator && separator < first_map);
    }
}
