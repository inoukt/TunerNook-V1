//! Safe, dependency-free semantic transfer planning for TunerNook.
//!
//! A transfer plan is deliberately separate from application. Planning performs all
//! definition, range, layout, hash, and overlap checks without mutating either BIN.
//! Application re-checks the source and destination hashes and commits every byte
//! through one [`tuner_core::BinDocument`] transaction.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use tuner_core::{sha256_hex, BinDocument, ByteRange, CoreError, EditSummary};
use tuner_xdf::{ParameterDefinition, ParameterKind, ValidationIssue, XdfDocument};

/// Stable version marker for serialized transfer plans.
pub const TRANSFER_PLAN_VERSION: &str = "tuner-transfer/v1";

/// Selection and explicit-review choices supplied to the planner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransferOptions {
    /// `None` selects every semantic parameter present in either definition.
    pub selected: Option<BTreeSet<String>>,
    /// Parameter identities whose source/destination backing addresses were reviewed.
    pub approved_address_reviews: BTreeSet<String>,
}

impl TransferOptions {
    pub fn all() -> Self {
        Self::default()
    }

    pub fn selected<I, S>(identities: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            selected: Some(identities.into_iter().map(Into::into).collect()),
            approved_address_reviews: BTreeSet::new(),
        }
    }

    pub fn approve_address_review(mut self, semantic_id: impl Into<String>) -> Self {
        self.approved_address_reviews.insert(semantic_id.into());
        self
    }
}

/// Machine-readable reason attached to a plan or parameter entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransferIssueCode {
    XdfHashMismatch,
    SourceBinOutOfBounds,
    DestinationBinOutOfBounds,
    MissingSourceParameter,
    MissingDestinationParameter,
    ParameterKindMismatch,
    DimensionsMismatch,
    ElementWidthMismatch,
    StorageKindMismatch,
    SignednessMismatch,
    EndiannessMismatch,
    StrideMismatch,
    ConversionMismatch,
    BitLayoutMismatch,
    RangeSizeMismatch,
    AddressReviewRequired,
    DestinationOverlapConflict,
    SourceReadFailed,
    DestinationReadFailed,
    SourceChangedSincePlan,
    DestinationChangedSincePlan,
    PlanBlocked,
}

impl TransferIssueCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::XdfHashMismatch => "xdf_hash_mismatch",
            Self::SourceBinOutOfBounds => "source_bin_out_of_bounds",
            Self::DestinationBinOutOfBounds => "destination_bin_out_of_bounds",
            Self::MissingSourceParameter => "missing_source_parameter",
            Self::MissingDestinationParameter => "missing_destination_parameter",
            Self::ParameterKindMismatch => "parameter_kind_mismatch",
            Self::DimensionsMismatch => "dimensions_mismatch",
            Self::ElementWidthMismatch => "element_width_mismatch",
            Self::StorageKindMismatch => "storage_kind_mismatch",
            Self::SignednessMismatch => "signedness_mismatch",
            Self::EndiannessMismatch => "endianness_mismatch",
            Self::StrideMismatch => "stride_mismatch",
            Self::ConversionMismatch => "conversion_mismatch",
            Self::BitLayoutMismatch => "bit_layout_mismatch",
            Self::RangeSizeMismatch => "range_size_mismatch",
            Self::AddressReviewRequired => "address_review_required",
            Self::DestinationOverlapConflict => "destination_overlap_conflict",
            Self::SourceReadFailed => "source_read_failed",
            Self::DestinationReadFailed => "destination_read_failed",
            Self::SourceChangedSincePlan => "source_changed_since_plan",
            Self::DestinationChangedSincePlan => "destination_changed_since_plan",
            Self::PlanBlocked => "plan_blocked",
        }
    }
}

/// A self-contained compatibility or safety explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferIssue {
    pub code: TransferIssueCode,
    pub semantic_id: Option<String>,
    pub message: String,
    pub blocking: bool,
}

impl TransferIssue {
    fn global(code: TransferIssueCode, message: impl Into<String>) -> Self {
        Self {
            code,
            semantic_id: None,
            message: message.into(),
            blocking: true,
        }
    }

    fn parameter(
        code: TransferIssueCode,
        semantic_id: &str,
        message: impl Into<String>,
        blocking: bool,
    ) -> Self {
        Self {
            code,
            semantic_id: Some(semantic_id.to_string()),
            message: message.into(),
            blocking,
        }
    }
}

/// State of one selected semantic parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferEntryStatus {
    Ready,
    Unchanged,
    ReviewRequired,
    Blocked,
}

impl TransferEntryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Unchanged => "unchanged",
            Self::ReviewRequired => "review_required",
            Self::Blocked => "blocked",
        }
    }
}

/// Dry-run information for one semantic parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterTransfer {
    pub semantic_id: String,
    pub title: String,
    pub kind: ParameterKind,
    pub source_range: Option<ByteRange>,
    pub destination_range: Option<ByteRange>,
    pub source_address: Option<usize>,
    pub destination_address: Option<usize>,
    pub source_size_bytes: usize,
    pub destination_size_bytes: usize,
    pub changed_bytes: usize,
    pub changed_ranges: Vec<ByteRange>,
    pub fast_path: bool,
    pub status: TransferEntryStatus,
    pub issues: Vec<TransferIssue>,
}

impl ParameterTransfer {
    fn new(semantic_id: String, title: String, kind: ParameterKind) -> Self {
        Self {
            semantic_id,
            title,
            kind,
            source_range: None,
            destination_range: None,
            source_address: None,
            destination_address: None,
            source_size_bytes: 0,
            destination_size_bytes: 0,
            changed_bytes: 0,
            changed_ranges: Vec::new(),
            fast_path: false,
            status: TransferEntryStatus::Blocked,
            issues: Vec::new(),
        }
    }

    fn add_issue(&mut self, issue: TransferIssue) {
        self.issues.push(issue);
    }

    fn has_blocking_issue(&self) -> bool {
        self.issues.iter().any(|issue| issue.blocking)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlannedWrite {
    offset: usize,
    before: u8,
    after: u8,
}

#[derive(Debug, Clone, Default)]
struct ProposedByte {
    proposals: BTreeMap<String, u8>,
}

/// An immutable dry-run transfer plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferPlan {
    pub plan_id: String,
    pub source_xdf_sha256: String,
    pub destination_xdf_sha256: String,
    pub source_xdf_fingerprint: String,
    pub destination_xdf_fingerprint: String,
    pub source_bin_sha256: String,
    pub destination_bin_sha256: String,
    pub xdf_hash_match: bool,
    pub selected_count: usize,
    pub entries: Vec<ParameterTransfer>,
    pub issues: Vec<TransferIssue>,
    pub changed_bytes: usize,
    pub changed_ranges: Vec<ByteRange>,
    writes: Vec<PlannedWrite>,
}

impl TransferPlan {
    /// Build a non-mutating plan from source/destination XDF and BIN documents.
    pub fn build(
        source_xdf: &XdfDocument,
        destination_xdf: &XdfDocument,
        source_bin: &BinDocument,
        destination_bin: &BinDocument,
        options: &TransferOptions,
    ) -> Self {
        let source_xdf_sha256 = source_xdf.exact_sha256.clone();
        let destination_xdf_sha256 = destination_xdf.exact_sha256.clone();
        let source_bin_sha256 = sha256_hex(source_bin.bytes());
        let destination_bin_sha256 = sha256_hex(destination_bin.bytes());
        let xdf_hash_match = source_xdf_sha256 == destination_xdf_sha256;
        let mut issues = Vec::new();
        if !xdf_hash_match {
            issues.push(TransferIssue::global(
                TransferIssueCode::XdfHashMismatch,
                format!(
                    "bulk transfer requires identical XDF file hashes; source={} destination={}",
                    source_xdf_sha256, destination_xdf_sha256
                ),
            ));
        }

        add_validation_issues(
            &mut issues,
            source_xdf.validate_against_document(source_bin).issues,
            true,
        );
        add_validation_issues(
            &mut issues,
            destination_xdf
                .validate_against_document(destination_bin)
                .issues,
            false,
        );

        let identities = selected_identities(source_xdf, destination_xdf, options);
        let selected_count = identities.len();
        let mut entries = Vec::with_capacity(selected_count);
        let mut proposed = BTreeMap::<usize, ProposedByte>::new();

        for semantic_id in identities {
            let source = source_xdf.parameter(&semantic_id);
            let destination = destination_xdf.parameter(&semantic_id);
            let title = source
                .map(|parameter| parameter.title.clone())
                .or_else(|| destination.map(|parameter| parameter.title.clone()))
                .unwrap_or_else(|| semantic_id.clone());
            let kind = source
                .map(|parameter| parameter.kind)
                .or_else(|| destination.map(|parameter| parameter.kind))
                .unwrap_or(ParameterKind::Constant);
            let mut entry = ParameterTransfer::new(semantic_id.clone(), title, kind);

            match (source, destination) {
                (None, Some(_)) => entry.add_issue(TransferIssue::parameter(
                    TransferIssueCode::MissingSourceParameter,
                    &semantic_id,
                    "semantic parameter is absent from the source XDF",
                    true,
                )),
                (Some(_), None) => entry.add_issue(TransferIssue::parameter(
                    TransferIssueCode::MissingDestinationParameter,
                    &semantic_id,
                    "semantic parameter is absent from the destination XDF",
                    true,
                )),
                (Some(source), Some(destination)) => {
                    plan_parameter(
                        &mut entry,
                        source,
                        destination,
                        source_bin,
                        destination_bin,
                        options,
                        &mut proposed,
                    );
                }
                (None, None) => entry.add_issue(TransferIssue::parameter(
                    TransferIssueCode::MissingDestinationParameter,
                    &semantic_id,
                    "semantic parameter is absent from both XDF definitions",
                    true,
                )),
            }

            if !xdf_hash_match && !entry.has_blocking_issue() {
                entry.add_issue(TransferIssue::parameter(
                    TransferIssueCode::XdfHashMismatch,
                    &semantic_id,
                    "bulk transfer is disabled until the source and destination XDF file hashes match",
                    true,
                ));
            }
            entries.push(entry);
        }

        add_overlap_issues(&mut entries, &mut issues, &proposed);
        let has_global_blocker = issues.iter().any(|issue| issue.blocking);
        let mut writes = Vec::new();
        if !has_global_blocker {
            for (offset, proposed_byte) in proposed {
                let Some(before) = destination_bin.bytes().get(offset).copied() else {
                    issues.push(TransferIssue::global(
                        TransferIssueCode::DestinationBinOutOfBounds,
                        format!(
                            "planned destination byte offset {} could not be read during plan assembly",
                            offset
                        ),
                    ));
                    continue;
                };
                let mut values = proposed_byte.proposals.values().copied();
                let Some(after) = values.next() else {
                    continue;
                };
                if values.any(|value| value != after) {
                    continue;
                }
                writes.push(PlannedWrite {
                    offset,
                    before,
                    after,
                });
            }
        }

        let has_global_blocker = issues.iter().any(|issue| issue.blocking);

        for entry in &mut entries {
            if entry.has_blocking_issue() || has_global_blocker {
                entry.status = TransferEntryStatus::Blocked;
            } else if entry
                .issues
                .iter()
                .any(|issue| issue.code == TransferIssueCode::AddressReviewRequired)
            {
                entry.status = TransferEntryStatus::ReviewRequired;
            } else if entry.changed_bytes == 0 {
                entry.status = TransferEntryStatus::Unchanged;
            } else {
                entry.status = TransferEntryStatus::Ready;
            }
        }

        let changed_ranges = ranges_from_writes(&writes);
        let changed_bytes = writes
            .iter()
            .filter(|write| write.before != write.after)
            .count();
        let plan_id = plan_id_for(
            &source_xdf_sha256,
            &destination_xdf_sha256,
            &source_bin_sha256,
            &destination_bin_sha256,
            options,
        );
        Self {
            plan_id,
            source_xdf_sha256,
            destination_xdf_sha256,
            source_xdf_fingerprint: source_xdf.normalized_fingerprint.clone(),
            destination_xdf_fingerprint: destination_xdf.normalized_fingerprint.clone(),
            source_bin_sha256,
            destination_bin_sha256,
            xdf_hash_match,
            selected_count,
            entries,
            issues,
            changed_bytes,
            changed_ranges,
            writes,
        }
    }

    pub fn is_blocked(&self) -> bool {
        self.issues.iter().any(|issue| issue.blocking)
            || self.entries.iter().any(|entry| {
                matches!(
                    entry.status,
                    TransferEntryStatus::Blocked | TransferEntryStatus::ReviewRequired
                )
            })
    }

    pub fn requires_review(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.status == TransferEntryStatus::ReviewRequired)
    }

    pub fn is_ready(&self) -> bool {
        !self.is_blocked()
    }

    pub fn status(&self) -> &'static str {
        if self.issues.iter().any(|issue| issue.blocking)
            || self
                .entries
                .iter()
                .any(|entry| entry.status == TransferEntryStatus::Blocked)
        {
            "blocked"
        } else if self.requires_review() {
            "review_required"
        } else {
            "ready"
        }
    }

    /// Apply the already-approved plan as one undoable destination transaction.
    pub fn apply(
        &self,
        source_bin: &BinDocument,
        destination_bin: &mut BinDocument,
    ) -> Result<TransferApplySummary, TransferError> {
        if self.is_blocked() {
            return Err(TransferError::PlanBlocked {
                plan_id: self.plan_id.clone(),
                message: format!(
                    "transfer plan is {} and requires all blocking issues/reviews to be resolved",
                    self.status()
                ),
            });
        }
        let actual_source_hash = sha256_hex(source_bin.bytes());
        if actual_source_hash != self.source_bin_sha256 {
            return Err(TransferError::SourceChanged {
                expected: self.source_bin_sha256.clone(),
                actual: actual_source_hash,
            });
        }
        let actual_destination_hash = sha256_hex(destination_bin.bytes());
        if actual_destination_hash != self.destination_bin_sha256 {
            return Err(TransferError::DestinationChanged {
                expected: self.destination_bin_sha256.clone(),
                actual: actual_destination_hash,
            });
        }

        let mut transaction = destination_bin.transaction("transfer.apply");
        for write in &self.writes {
            transaction
                .write_u8(write.offset, write.after)
                .map_err(|error| TransferError::Core {
                    operation: "apply transfer",
                    error,
                })?;
        }
        let summary = transaction.commit().map_err(|error| TransferError::Core {
            operation: "commit transfer",
            error,
        })?;
        Ok(TransferApplySummary {
            plan_id: self.plan_id.clone(),
            changed_bytes: summary.changed_bytes,
            result_sha256: sha256_hex(destination_bin.bytes()),
            edit_summary: summary,
        })
    }
}

/// Result of committing a transfer plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferApplySummary {
    pub plan_id: String,
    pub changed_bytes: usize,
    pub result_sha256: String,
    pub edit_summary: EditSummary,
}

/// Errors that can occur while applying an otherwise valid plan.
#[derive(Debug)]
pub enum TransferError {
    PlanBlocked {
        plan_id: String,
        message: String,
    },
    SourceChanged {
        expected: String,
        actual: String,
    },
    DestinationChanged {
        expected: String,
        actual: String,
    },
    Core {
        operation: &'static str,
        error: CoreError,
    },
}

impl fmt::Display for TransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlanBlocked { plan_id, message } => {
                write!(f, "transfer plan {plan_id} is blocked: {message}")
            }
            Self::SourceChanged { expected, actual } => write!(
                f,
                "source BIN changed after planning: expected SHA-256 {expected}, found {actual}"
            ),
            Self::DestinationChanged { expected, actual } => write!(
                f,
                "destination BIN changed after planning: expected SHA-256 {expected}, found {actual}"
            ),
            Self::Core { operation, error } => write!(f, "{operation} failed: {error}"),
        }
    }
}

impl std::error::Error for TransferError {}

fn selected_identities(
    source_xdf: &XdfDocument,
    destination_xdf: &XdfDocument,
    options: &TransferOptions,
) -> Vec<String> {
    let mut identities = BTreeSet::new();
    match &options.selected {
        Some(selected) => identities.extend(selected.iter().cloned()),
        None => {
            identities.extend(
                source_xdf
                    .parameters
                    .iter()
                    .map(|parameter| parameter.semantic_id.clone()),
            );
            identities.extend(
                destination_xdf
                    .parameters
                    .iter()
                    .map(|parameter| parameter.semantic_id.clone()),
            );
        }
    }
    identities.into_iter().collect()
}

fn add_validation_issues(
    output: &mut Vec<TransferIssue>,
    validation_issues: Vec<ValidationIssue>,
    source: bool,
) {
    for issue in validation_issues {
        let code = if source {
            TransferIssueCode::SourceBinOutOfBounds
        } else {
            TransferIssueCode::DestinationBinOutOfBounds
        };
        output.push(TransferIssue::global(
            code,
            format!(
                "{} mapping '{}' is invalid: {}",
                if source { "source" } else { "destination" },
                issue.semantic_id,
                issue.message
            ),
        ));
    }
}

fn plan_parameter(
    entry: &mut ParameterTransfer,
    source: &ParameterDefinition,
    destination: &ParameterDefinition,
    source_bin: &BinDocument,
    destination_bin: &BinDocument,
    options: &TransferOptions,
    proposed: &mut BTreeMap<usize, ProposedByte>,
) {
    entry.source_range = Some(source.byte_range());
    entry.destination_range = Some(destination.byte_range());
    entry.source_address = Some(source.layout.address);
    entry.destination_address = Some(destination.layout.address);
    entry.source_size_bytes = source.byte_range().len();
    entry.destination_size_bytes = destination.byte_range().len();

    if source.kind != destination.kind {
        entry.add_issue(TransferIssue::parameter(
            TransferIssueCode::ParameterKindMismatch,
            &entry.semantic_id,
            format!(
                "source kind '{}' differs from destination kind '{}'",
                source.kind.as_str(),
                destination.kind.as_str()
            ),
            true,
        ));
    }
    if source.dimensions() != destination.dimensions() {
        entry.add_issue(TransferIssue::parameter(
            TransferIssueCode::DimensionsMismatch,
            &entry.semantic_id,
            "source and destination dimensions differ",
            true,
        ));
    }
    if source.layout.element_width_bits != destination.layout.element_width_bits {
        entry.add_issue(TransferIssue::parameter(
            TransferIssueCode::ElementWidthMismatch,
            &entry.semantic_id,
            "source and destination element widths differ",
            true,
        ));
    }
    if source.layout.storage.numeric_kind != destination.layout.storage.numeric_kind
        || source.layout.storage.raw_type_flags != destination.layout.storage.raw_type_flags
    {
        entry.add_issue(TransferIssue::parameter(
            TransferIssueCode::StorageKindMismatch,
            &entry.semantic_id,
            format!(
                "source storage kind={} flags={:#x} differs from destination kind={} flags={:#x}",
                source.layout.storage.numeric_kind.as_str(),
                source.layout.storage.raw_type_flags,
                destination.layout.storage.numeric_kind.as_str(),
                destination.layout.storage.raw_type_flags,
            ),
            true,
        ));
    }
    if source.layout.signed != destination.layout.signed {
        entry.add_issue(TransferIssue::parameter(
            TransferIssueCode::SignednessMismatch,
            &entry.semantic_id,
            "source and destination signedness differ",
            true,
        ));
    }
    if source.layout.endianness != destination.layout.endianness {
        entry.add_issue(TransferIssue::parameter(
            TransferIssueCode::EndiannessMismatch,
            &entry.semantic_id,
            "source and destination endianness differ and no conversion policy is configured",
            true,
        ));
    }
    if source.layout.row_stride_bits != destination.layout.row_stride_bits
        || source.layout.column_stride_bits != destination.layout.column_stride_bits
    {
        entry.add_issue(TransferIssue::parameter(
            TransferIssueCode::StrideMismatch,
            &entry.semantic_id,
            "source and destination storage strides differ",
            true,
        ));
    }
    if source.conversion != destination.conversion {
        entry.add_issue(TransferIssue::parameter(
            TransferIssueCode::ConversionMismatch,
            &entry.semantic_id,
            "source and destination conversion metadata differ",
            true,
        ));
    }
    if source.bit_offset != destination.bit_offset || source.bit_width != destination.bit_width {
        entry.add_issue(TransferIssue::parameter(
            TransferIssueCode::BitLayoutMismatch,
            &entry.semantic_id,
            "source and destination bit-field layouts differ",
            true,
        ));
    }
    if source.byte_range().len() != destination.byte_range().len() {
        entry.add_issue(TransferIssue::parameter(
            TransferIssueCode::RangeSizeMismatch,
            &entry.semantic_id,
            "source and destination mapped byte ranges have different lengths",
            true,
        ));
    }

    let address_differs = source.layout.address != destination.layout.address;
    if address_differs
        && !options
            .approved_address_reviews
            .contains(&entry.semantic_id)
    {
        entry.add_issue(TransferIssue::parameter(
            TransferIssueCode::AddressReviewRequired,
            &entry.semantic_id,
            format!(
                "backing address differs: source={} destination={}; explicit review is required",
                source.layout.address, destination.layout.address
            ),
            false,
        ));
    }
    entry.fast_path = !address_differs && !entry.has_blocking_issue();

    if entry.has_blocking_issue() {
        return;
    }
    let source_range = source.byte_range();
    let destination_range = destination.byte_range();
    let source_bytes = match source_bin.read_bytes(source_range.start, source_range.len()) {
        Ok(bytes) => bytes,
        Err(error) => {
            entry.add_issue(TransferIssue::parameter(
                TransferIssueCode::SourceReadFailed,
                &entry.semantic_id,
                error.to_string(),
                true,
            ));
            return;
        }
    };
    let destination_bytes =
        match destination_bin.read_bytes(destination_range.start, destination_range.len()) {
            Ok(bytes) => bytes,
            Err(error) => {
                entry.add_issue(TransferIssue::parameter(
                    TransferIssueCode::DestinationReadFailed,
                    &entry.semantic_id,
                    error.to_string(),
                    true,
                ));
                return;
            }
        };
    for (index, after) in source_bytes.iter().copied().enumerate() {
        let offset = destination_range.start + index;
        let before = destination_bytes[index];
        let proposed_byte = proposed.entry(offset).or_default();
        proposed_byte
            .proposals
            .insert(entry.semantic_id.clone(), after);
        if before != after {
            entry.changed_bytes += 1;
        }
    }
    entry.changed_ranges =
        ranges_for_entry(source_bytes, destination_bytes, destination_range.start);
}

fn ranges_for_entry(source: &[u8], destination: &[u8], start: usize) -> Vec<ByteRange> {
    let mut ranges = Vec::new();
    let mut range_start = None;
    for (index, (before, after)) in destination.iter().zip(source.iter()).enumerate() {
        if before != after {
            if range_start.is_none() {
                range_start = Some(start + index);
            }
        } else if let Some(begin) = range_start.take() {
            ranges.push(ByteRange {
                start: begin,
                end: start + index,
            });
        }
    }
    if let Some(begin) = range_start {
        ranges.push(ByteRange {
            start: begin,
            end: start + source.len(),
        });
    }
    ranges
}

fn add_overlap_issues(
    entries: &mut [ParameterTransfer],
    issues: &mut Vec<TransferIssue>,
    proposed: &BTreeMap<usize, ProposedByte>,
) {
    for (offset, proposed_byte) in proposed {
        if proposed_byte.proposals.len() < 2 {
            continue;
        }
        let ids = proposed_byte.proposals.keys().cloned().collect::<Vec<_>>();
        let values = proposed_byte
            .proposals
            .values()
            .copied()
            .collect::<BTreeSet<_>>();
        if values.len() < 2 {
            continue;
        }
        let message = format!(
            "destination byte offset {} receives conflicting proposed values from parameters {}",
            offset,
            ids.join(", ")
        );
        issues.push(TransferIssue::global(
            TransferIssueCode::DestinationOverlapConflict,
            message.clone(),
        ));
        for entry in entries.iter_mut() {
            if proposed_byte.proposals.contains_key(&entry.semantic_id) {
                entry.add_issue(TransferIssue::parameter(
                    TransferIssueCode::DestinationOverlapConflict,
                    &entry.semantic_id,
                    message.clone(),
                    true,
                ));
            }
        }
    }
}

fn ranges_from_writes(writes: &[PlannedWrite]) -> Vec<ByteRange> {
    let mut ranges = Vec::new();
    let mut start = None;
    let mut previous = None;
    for write in writes.iter().filter(|write| write.before != write.after) {
        match (start, previous) {
            (Some(begin), Some(last)) if write.offset == last + 1 => {
                previous = Some(write.offset);
                if begin > write.offset {
                    start = Some(write.offset);
                }
            }
            (Some(begin), Some(last)) => {
                ranges.push(ByteRange {
                    start: begin,
                    end: last + 1,
                });
                start = Some(write.offset);
                previous = Some(write.offset);
            }
            _ => {
                start = Some(write.offset);
                previous = Some(write.offset);
            }
        }
    }
    if let (Some(begin), Some(last)) = (start, previous) {
        ranges.push(ByteRange {
            start: begin,
            end: last + 1,
        });
    }
    ranges
}

fn plan_id_for(
    source_xdf_sha256: &str,
    destination_xdf_sha256: &str,
    source_bin_sha256: &str,
    destination_bin_sha256: &str,
    options: &TransferOptions,
) -> String {
    let selected = options
        .selected
        .as_ref()
        .map(|values| values.iter().cloned().collect::<Vec<_>>().join("\0"))
        .unwrap_or_default();
    let approved = options
        .approved_address_reviews
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join("\0");
    let canonical = format!(
        "{TRANSFER_PLAN_VERSION}\0{source_xdf_sha256}\0{destination_xdf_sha256}\0{source_bin_sha256}\0{destination_bin_sha256}\0{selected}\0{approved}"
    );
    sha256_hex(canonical.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xdf(address: &str, title: &str) -> XdfDocument {
        let source = format!(
            "<XDFFORMAT><XDFCONSTANT uniqueid=\"rpm\" address=\"{address}\" elementsizebits=\"16\"><title>{title}</title></XDFCONSTANT></XDFFORMAT>"
        );
        XdfDocument::parse(source.as_bytes()).unwrap()
    }

    #[test]
    fn matching_xdf_builds_and_applies_one_atomic_transfer() {
        let definition = xdf("0x01", "RPM");
        let source = BinDocument::from_bytes(vec![0, 0x34, 0x12, 0]);
        let mut destination = BinDocument::from_bytes(vec![0, 0, 0, 0]);
        let plan = TransferPlan::build(
            &definition,
            &definition,
            &source,
            &destination,
            &TransferOptions::all(),
        );
        assert!(plan.is_ready());
        assert_eq!(plan.changed_bytes, 2);
        assert_eq!(plan.entries[0].status, TransferEntryStatus::Ready);
        let result = plan.apply(&source, &mut destination).unwrap();
        assert_eq!(result.changed_bytes, 2);
        assert_eq!(destination.bytes(), &[0, 0x34, 0x12, 0]);
        assert_eq!(destination.undo_depth(), 1);
    }

    #[test]
    fn xdf_hash_mismatch_blocks_bulk_transfer_even_with_matching_semantics() {
        let source_xdf = xdf("0x01", "RPM");
        let destination_xdf = xdf("0x01", "RPM changed");
        let source = BinDocument::from_bytes(vec![0, 1, 0]);
        let destination = BinDocument::from_bytes(vec![0, 0, 0]);
        let plan = TransferPlan::build(
            &source_xdf,
            &destination_xdf,
            &source,
            &destination,
            &TransferOptions::all(),
        );
        assert!(!plan.xdf_hash_match);
        assert!(plan.is_blocked());
        assert!(plan
            .issues
            .iter()
            .any(|issue| issue.code == TransferIssueCode::XdfHashMismatch));
        assert!(matches!(
            plan.apply(&source, &mut destination.clone()),
            Err(TransferError::PlanBlocked { .. })
        ));
    }

    #[test]
    fn address_difference_requires_explicit_review() {
        let source_xdf = xdf("0x01", "RPM");
        let mut destination_xdf = xdf("0x02", "RPM");
        // Simulate two loaded views of one verified definition whose resolved BIN
        // locations differ; exact hash equality remains the bulk-transfer gate.
        destination_xdf.exact_sha256 = source_xdf.exact_sha256.clone();
        let source = BinDocument::from_bytes(vec![0, 0x22, 0, 0]);
        let destination = BinDocument::from_bytes(vec![0, 0, 0, 0]);
        let pending = TransferPlan::build(
            &source_xdf,
            &destination_xdf,
            &source,
            &destination,
            &TransferOptions::all(),
        );
        assert!(pending.requires_review());
        assert_eq!(pending.status(), "review_required");
        let approved = TransferPlan::build(
            &source_xdf,
            &destination_xdf,
            &source,
            &destination,
            &TransferOptions::all().approve_address_review("constant:uid:rpm"),
        );
        assert!(approved.is_ready());
    }

    #[test]
    fn stale_destination_is_rejected_before_mutation() {
        let definition = xdf("0x00", "RPM");
        let source = BinDocument::from_bytes(vec![1, 0]);
        let destination = BinDocument::from_bytes(vec![0, 0]);
        let plan = TransferPlan::build(
            &definition,
            &definition,
            &source,
            &destination,
            &TransferOptions::all(),
        );
        let mut changed = BinDocument::from_bytes(vec![9, 0]);
        let before = changed.bytes().to_vec();
        let error = plan.apply(&source, &mut changed).unwrap_err();
        assert!(matches!(error, TransferError::DestinationChanged { .. }));
        assert_eq!(changed.bytes(), before.as_slice());
    }

    #[test]
    fn storage_kind_mismatch_blocks_byte_copy() {
        let source_xdf = XdfDocument::parse(
            br#"<XDFFORMAT><XDFCONSTANT uniqueid="rpm">
                <title>RPM</title>
                <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="32" mmedtypeflags="0x02" />
            </XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();
        let destination = XdfDocument::parse(
            br#"<XDFFORMAT><XDFCONSTANT uniqueid="rpm">
                <title>RPM</title>
                <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="32" mmedtypeflags="0x10006" />
            </XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();
        let mut destination_xdf = destination;
        destination_xdf.exact_sha256 = source_xdf.exact_sha256.clone();
        let source = BinDocument::from_bytes(vec![0, 1, 0, 0]);
        let destination_bin = BinDocument::from_bytes(vec![0, 0, 0, 0]);
        let plan = TransferPlan::build(
            &source_xdf,
            &destination_xdf,
            &source,
            &destination_bin,
            &TransferOptions::all(),
        );
        assert!(plan
            .entries
            .iter()
            .flat_map(|entry| &entry.issues)
            .any(|issue| issue.code == TransferIssueCode::StorageKindMismatch));
        assert!(plan.is_blocked());
    }

    #[test]
    fn identical_column_major_integer_storage_does_not_self_block() {
        let source_xdf = XdfDocument::parse(
            br#"<XDFFORMAT><XDFCONSTANT uniqueid="rpm">
                <title>RPM</title>
                <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="32" mmedtypeflags="0x06" />
            </XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();
        let destination_xdf = XdfDocument::parse(
            br#"<XDFFORMAT><XDFCONSTANT uniqueid="rpm">
                <title>RPM</title>
                <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="32" mmedtypeflags="0x06" />
            </XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();
        let source = BinDocument::from_bytes(vec![0, 1, 0, 0]);
        let destination = BinDocument::from_bytes(vec![0, 0, 0, 0]);
        let plan = TransferPlan::build(
            &source_xdf,
            &destination_xdf,
            &source,
            &destination,
            &TransferOptions::all(),
        );
        assert!(!plan
            .entries
            .iter()
            .flat_map(|entry| &entry.issues)
            .any(|issue| issue.code == TransferIssueCode::StorageKindMismatch));
        assert!(plan.is_ready());
    }
}
