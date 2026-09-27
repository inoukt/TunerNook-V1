use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const MAX_RECENT_TASKS: usize = 100;
const MAX_PROPOSAL_OPERATIONS: usize = 128;
const MAX_XDF_PROPOSAL_OPERATIONS: usize = 64;
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCapability {
    Diagnostics,
    TuningHelp,
    AutomationEditing,
    ReverseEngineeringSearch,
    XdfAuthoring,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentTaskCommand {
    Start {
        capability: AgentCapability,
        goal: String,
    },
    Progress {
        task_id: String,
        phase: String,
        progress_percent: Option<u8>,
        message: String,
    },
    Submit {
        task_id: String,
        report: AgentReport,
    },
    Poll {
        task_id: String,
    },
    Fail {
        task_id: String,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskStatus {
    AwaitingUserApproval,
    Running,
    AwaitingReview,
    Accepted,
    Rejected,
    Cancelled,
    Failed,
    Stale,
    Applied,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskLevel {
    Quick,
    LongComplex,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct DocumentIdentity {
    pub bin_sha256: Option<String>,
    pub xdf_sha256: Option<String>,
    pub workspace_data_revision: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentReport {
    pub summary: String,
    pub findings: Vec<String>,
    pub confidence: Option<f32>,
    pub evidence: Vec<AgentEvidence>,
    pub operations: Vec<ProposedOperation>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentEvidence {
    pub summary: String,
    pub semantic_id: Option<String>,
    pub byte_range: Option<[usize; 2]>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum ProposedOperation {
    SetEngineeringCell {
        semantic_id: String,
        row: usize,
        column: usize,
        engineering_value: f64,
    },
    XdfAuthoring {
        operations: Vec<ProposedXdfOperation>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposedXdfParameterKind {
    Constant,
    Table,
    BitField,
    Flag,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposedNumericKind {
    Integer,
    Ieee754Binary32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposedEndianness {
    Little,
    Big,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ProposedXdfOperation {
    AddParameter {
        definition: ProposedXdfParameter,
    },
    ReplaceParameter {
        semantic_id: String,
        definition: ProposedXdfParameter,
    },
    DeleteParameter {
        semantic_id: String,
    },
    ReorderParameter {
        semantic_id: String,
        index: usize,
    },
    SetHeader {
        title: Option<String>,
        description: Option<String>,
        author: Option<String>,
        version: Option<String>,
        base_offset: u64,
        subtract_base_offset: bool,
    },
    SetCategories {
        categories: Vec<ProposedXdfCategory>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProposedXdfParameter {
    pub unique_id: Option<String>,
    pub kind: ProposedXdfParameterKind,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub xdf_address: u64,
    pub element_width_bits: u32,
    pub rows: usize,
    pub columns: usize,
    pub signed: bool,
    pub endianness: ProposedEndianness,
    pub numeric_kind: ProposedNumericKind,
    pub column_major: bool,
    pub row_stride_bits: i64,
    pub column_stride_bits: i64,
    pub conversion: Option<String>,
    pub bit_offset: Option<usize>,
    pub bit_width: Option<usize>,
    pub bit_mask: Option<u64>,
    pub axes: Vec<ProposedXdfAxis>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProposedXdfAxis {
    pub id: String,
    pub title: String,
    pub count: usize,
    pub xdf_address: Option<u64>,
    pub element_width_bits: u32,
    pub stride_bits: i64,
    pub signed: bool,
    pub endianness: ProposedEndianness,
    pub numeric_kind: ProposedNumericKind,
    pub conversion: Option<String>,
    pub units: Option<String>,
    pub decimal_places: Option<u32>,
    pub labels: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProposedXdfCategory {
    pub index: u64,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentTaskRecord {
    pub task_id: String,
    pub capability: AgentCapability,
    pub goal: String,
    pub document_identity: DocumentIdentity,
    pub status: AgentTaskStatus,
    pub level: Option<AgentTaskLevel>,
    pub raw_read_authorization_pending: bool,
    pub raw_read_authorized: bool,
    pub phase: String,
    pub progress_percent: Option<u8>,
    pub message: String,
    pub report: Option<AgentReport>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentTaskHistoryItem {
    pub task_id: String,
    pub capability: AgentCapability,
    pub document_identity: DocumentIdentity,
    pub status: AgentTaskStatus,
    pub summary: String,
    pub evidence: Vec<AgentEvidence>,
    pub decision: Option<String>,
    pub applied_operations: Vec<String>,
}

impl AgentTaskHistoryItem {
    fn from_record(record: &AgentTaskRecord) -> Self {
        let decision = match record.status {
            AgentTaskStatus::Accepted | AgentTaskStatus::Applied => "accepted",
            AgentTaskStatus::Rejected => "rejected",
            AgentTaskStatus::Cancelled => "cancelled",
            AgentTaskStatus::Failed => "failed",
            AgentTaskStatus::Stale => "stale",
            _ => "pending",
        };
        let mut item = Self {
            task_id: record.task_id.clone(),
            capability: record.capability,
            document_identity: record.document_identity.clone(),
            status: record.status,
            summary: format!("{:?} task", record.capability),
            evidence: record.report.as_ref().map_or_else(Vec::new, |report| {
                report
                    .evidence
                    .iter()
                    .filter_map(|evidence| {
                        evidence
                            .byte_range
                            .filter(|[start, end]| start <= end)
                            .map(|byte_range| AgentEvidence {
                                summary: String::new(),
                                semantic_id: None,
                                byte_range: Some(byte_range),
                            })
                    })
                    .collect()
            }),
            decision: Some(decision.into()),
            applied_operations: if record.status == AgentTaskStatus::Applied {
                record.report.as_ref().map_or_else(Vec::new, |report| {
                    report
                        .operations
                        .iter()
                        .map(|operation| match operation {
                            ProposedOperation::SetEngineeringCell {
                                semantic_id,
                                row,
                                column,
                                ..
                            } => format!("set_engineering_cell:{semantic_id}:{row}:{column}"),
                            ProposedOperation::XdfAuthoring { operations } => {
                                format!("xdf_authoring:{}_change(s)", operations.len())
                            }
                        })
                        .collect()
                })
            } else {
                Vec::new()
            },
        };
        item.sanitize();
        item
    }

    pub fn sanitize(&mut self) {
        self.task_id = self.task_id.chars().take(128).collect();
        self.summary = self.summary.chars().take(128).collect();
        self.decision = self
            .decision
            .take()
            .map(|value| value.chars().take(32).collect());
        self.document_identity.bin_sha256 = self
            .document_identity
            .bin_sha256
            .take()
            .map(|value| value.chars().take(64).collect());
        self.document_identity.xdf_sha256 = self
            .document_identity
            .xdf_sha256
            .take()
            .map(|value| value.chars().take(64).collect());
        self.evidence.truncate(16);
        for evidence in &mut self.evidence {
            evidence.summary.clear();
            evidence.semantic_id = None;
        }
        self.evidence
            .retain(|evidence| evidence.byte_range.is_some_and(|[start, end]| start <= end));
        self.applied_operations.truncate(16);
        for operation in &mut self.applied_operations {
            *operation = operation.chars().take(160).collect();
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentTaskError {
    pub message: String,
}

impl fmt::Display for AgentTaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AgentTaskError {}

#[derive(Debug, Default)]
pub struct AgentTaskStore {
    records: BTreeMap<String, AgentTaskRecord>,
    history_events: Vec<AgentTaskHistoryItem>,
}

impl AgentTaskStore {
    pub fn start(
        &mut self,
        capability: AgentCapability,
        goal: String,
        document_identity: DocumentIdentity,
    ) -> Result<String, AgentTaskError> {
        let evict = if self.records.len() >= MAX_RECENT_TASKS {
            Some(
                self.records
                    .iter()
                    .find(|(_, record)| is_terminal(record.status))
                    .map(|(task_id, _)| task_id.clone())
                    .ok_or_else(|| task_error("task history is full of active tasks"))?,
            )
        } else {
            None
        };
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let counter = NEXT_TASK_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| task_error("task ID counter exhausted"))?;
        let task_id = format!("{}-{timestamp}-{counter:020}", std::process::id());
        if let Some(task_id) = evict {
            self.records.remove(&task_id);
        }
        self.records.insert(
            task_id.clone(),
            AgentTaskRecord {
                task_id: task_id.clone(),
                capability,
                goal,
                document_identity,
                status: AgentTaskStatus::AwaitingUserApproval,
                level: None,
                raw_read_authorization_pending: false,
                raw_read_authorized: false,
                phase: String::new(),
                progress_percent: None,
                message: String::new(),
                report: None,
            },
        );
        Ok(task_id)
    }

    pub fn update_progress(
        &mut self,
        task_id: &str,
        phase: String,
        progress_percent: Option<u8>,
        message: String,
    ) -> Result<(), AgentTaskError> {
        if progress_percent.is_some_and(|value| value > 100) {
            return Err(task_error("progress_percent must be between 0 and 100"));
        }
        let record = self.running_record_mut(task_id)?;
        record.phase = phase;
        record.progress_percent = progress_percent;
        record.message = message;
        Ok(())
    }

    pub fn approve_start(
        &mut self,
        task_id: &str,
        level: AgentTaskLevel,
    ) -> Result<(), AgentTaskError> {
        let record = self
            .records
            .get_mut(task_id)
            .ok_or_else(|| task_error("unknown task"))?;
        if record.status != AgentTaskStatus::AwaitingUserApproval {
            return Err(task_error("task is not awaiting user approval"));
        }
        record.level = Some(level);
        record.status = AgentTaskStatus::Running;
        Ok(())
    }

    pub fn request_raw_read_authorization(&mut self, task_id: &str) -> Result<(), AgentTaskError> {
        let record = self.running_record_mut(task_id)?;
        record.raw_read_authorization_pending = true;
        record.raw_read_authorized = false;
        Ok(())
    }

    pub fn allow_raw_reads(
        &mut self,
        task_id: &str,
        current_bin_sha256: &str,
    ) -> Result<(), AgentTaskError> {
        let record = self.running_record_mut(task_id)?;
        if record.document_identity.bin_sha256.as_deref() != Some(current_bin_sha256) {
            return Err(task_error(
                "active BIN identity does not match the task source",
            ));
        }
        record.raw_read_authorization_pending = false;
        record.raw_read_authorized = true;
        Ok(())
    }

    pub fn submit(&mut self, task_id: &str, report: AgentReport) -> Result<(), AgentTaskError> {
        if report
            .confidence
            .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
        {
            return Err(task_error("confidence must be finite and between 0 and 1"));
        }
        if report.operations.len() > MAX_PROPOSAL_OPERATIONS {
            return Err(task_error("proposal contains too many operations"));
        }
        let record = self.running_record_mut(task_id)?;
        validate_report_capability(record.capability, &report)?;
        record.report = Some(report);
        record.status = AgentTaskStatus::AwaitingReview;
        clear_raw_read_authorization(record);
        Ok(())
    }

    pub fn poll(&self, task_id: &str) -> Result<&AgentTaskRecord, AgentTaskError> {
        self.records
            .get(task_id)
            .ok_or_else(|| task_error("unknown task"))
    }

    pub fn records(&self) -> impl Iterator<Item = &AgentTaskRecord> {
        self.records.values()
    }

    pub fn take_history(&mut self) -> Vec<AgentTaskHistoryItem> {
        std::mem::take(&mut self.history_events)
    }

    pub(crate) fn requeue_history(&mut self, mut events: Vec<AgentTaskHistoryItem>) {
        events.append(&mut self.history_events);
        self.history_events = events;
    }

    pub fn fail(&mut self, task_id: &str, message: String) -> Result<(), AgentTaskError> {
        let record = self.running_record_mut(task_id)?;
        record.message = message;
        record.status = AgentTaskStatus::Failed;
        clear_raw_read_authorization(record);
        let event = AgentTaskHistoryItem::from_record(record);
        self.history_events.push(event);
        Ok(())
    }

    pub fn cancel(&mut self, task_id: &str) -> Result<(), AgentTaskError> {
        let record = self
            .records
            .get_mut(task_id)
            .ok_or_else(|| task_error("unknown task"))?;
        if !matches!(
            record.status,
            AgentTaskStatus::AwaitingUserApproval | AgentTaskStatus::Running
        ) {
            return Err(task_error("task is not active"));
        }
        record.status = AgentTaskStatus::Cancelled;
        clear_raw_read_authorization(record);
        self.history_events
            .push(AgentTaskHistoryItem::from_record(record));
        Ok(())
    }

    pub fn decide(&mut self, task_id: &str, accepted: bool) -> Result<(), AgentTaskError> {
        let record = self
            .records
            .get_mut(task_id)
            .ok_or_else(|| task_error("unknown task"))?;
        if record.status != AgentTaskStatus::AwaitingReview {
            return Err(task_error("task is not awaiting review"));
        }
        record.status = if accepted {
            AgentTaskStatus::Accepted
        } else {
            AgentTaskStatus::Rejected
        };
        clear_raw_read_authorization(record);
        self.history_events
            .push(AgentTaskHistoryItem::from_record(record));
        Ok(())
    }

    pub fn mark_stale(&mut self, task_id: &str) -> Result<(), AgentTaskError> {
        let record = self
            .records
            .get_mut(task_id)
            .ok_or_else(|| task_error("unknown task"))?;
        if !matches!(
            record.status,
            AgentTaskStatus::AwaitingReview | AgentTaskStatus::Accepted
        ) {
            return Err(task_error("task is not reviewable"));
        }
        record.status = AgentTaskStatus::Stale;
        clear_raw_read_authorization(record);
        self.history_events
            .push(AgentTaskHistoryItem::from_record(record));
        Ok(())
    }

    pub fn mark_applied(&mut self, task_id: &str) -> Result<(), AgentTaskError> {
        let record = self
            .records
            .get_mut(task_id)
            .ok_or_else(|| task_error("unknown task"))?;
        if record.status != AgentTaskStatus::Accepted
            || !record
                .report
                .as_ref()
                .is_some_and(|report| !report.operations.is_empty())
        {
            return Err(task_error("task has no accepted operations to apply"));
        }
        record.status = AgentTaskStatus::Applied;
        clear_raw_read_authorization(record);
        self.history_events
            .push(AgentTaskHistoryItem::from_record(record));
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    fn running_record_mut(
        &mut self,
        task_id: &str,
    ) -> Result<&mut AgentTaskRecord, AgentTaskError> {
        let record = self
            .records
            .get_mut(task_id)
            .ok_or_else(|| task_error("unknown task"))?;
        if record.status != AgentTaskStatus::Running {
            return Err(task_error("task is not running"));
        }
        Ok(record)
    }
}

fn validate_report_capability(
    capability: AgentCapability,
    report: &AgentReport,
) -> Result<(), AgentTaskError> {
    let xdf_operation = report
        .operations
        .iter()
        .any(|operation| matches!(operation, ProposedOperation::XdfAuthoring { .. }));
    let cell_operation = report
        .operations
        .iter()
        .any(|operation| matches!(operation, ProposedOperation::SetEngineeringCell { .. }));
    if xdf_operation && capability != AgentCapability::XdfAuthoring {
        return Err(task_error(
            "XDF definition operations require the xdf_authoring capability",
        ));
    }
    if capability == AgentCapability::XdfAuthoring && cell_operation {
        return Err(task_error(
            "xdf_authoring tasks cannot propose BIN cell writes",
        ));
    }
    if xdf_operation && report.operations.len() != 1 {
        return Err(task_error(
            "XDF authoring proposals cannot be mixed with other operation families",
        ));
    }
    for operation in &report.operations {
        if let ProposedOperation::XdfAuthoring { operations } = operation {
            if operations.is_empty() || operations.len() > MAX_XDF_PROPOSAL_OPERATIONS {
                return Err(task_error(
                    "XDF proposal must contain 1 to 64 typed changes",
                ));
            }
            for change in operations {
                validate_xdf_change(change)?;
            }
        }
    }
    Ok(())
}

fn validate_xdf_change(change: &ProposedXdfOperation) -> Result<(), AgentTaskError> {
    let text_ok = |text: &str, max: usize| text.chars().count() <= max;
    match change {
        ProposedXdfOperation::AddParameter { definition }
        | ProposedXdfOperation::ReplaceParameter { definition, .. } => {
            let scalar = definition.kind != ProposedXdfParameterKind::Table;
            let cells = definition.rows.checked_mul(definition.columns);
            if definition.title.trim().is_empty()
                || definition.rows == 0
                || definition.columns == 0
                || definition.rows > 512
                || definition.columns > 512
                || cells.is_none_or(|cells| cells > 262_144)
                || (scalar && (definition.rows != 1 || definition.columns != 1))
                || definition.element_width_bits == 0
                || definition.element_width_bits > 64
                || (definition.numeric_kind == ProposedNumericKind::Ieee754Binary32
                    && definition.element_width_bits != 32)
                || definition.axes.len() > 3
                || !text_ok(&definition.title, 512)
                || !text_ok(&definition.description, 4096)
                || definition
                    .unique_id
                    .as_ref()
                    .is_some_and(|value| !text_ok(value, 256))
                || definition
                    .category
                    .as_ref()
                    .is_some_and(|value| !text_ok(value, 512))
                || definition
                    .conversion
                    .as_ref()
                    .is_some_and(|value| !text_ok(value, 4096))
            {
                return Err(task_error(
                    "XDF definition contains invalid dimensions, storage, or oversized text",
                ));
            }
            if definition
                .bit_offset
                .zip(definition.bit_width)
                .is_some_and(|(offset, width)| {
                    width == 0
                        || offset
                            .checked_add(width)
                            .is_none_or(|end| end > definition.element_width_bits as usize)
                })
            {
                return Err(task_error("XDF bit range is outside the element width"));
            }
            for axis in &definition.axes {
                if axis.id.trim().is_empty()
                    || axis.count == 0
                    || axis.count > 65_536
                    || axis.element_width_bits == 0
                    || axis.element_width_bits > 64
                    || (axis.numeric_kind == ProposedNumericKind::Ieee754Binary32
                        && axis.element_width_bits != 32)
                    || !text_ok(&axis.id, 64)
                    || !text_ok(&axis.title, 512)
                    || axis
                        .conversion
                        .as_ref()
                        .is_some_and(|value| !text_ok(value, 4096))
                    || axis
                        .units
                        .as_ref()
                        .is_some_and(|value| !text_ok(value, 128))
                    || axis.labels.len() > 1024
                    || axis.labels.iter().any(|label| !text_ok(label, 512))
                {
                    return Err(task_error(
                        "XDF axis contains invalid storage, count, or oversized text",
                    ));
                }
            }
        }
        ProposedXdfOperation::DeleteParameter { semantic_id }
        | ProposedXdfOperation::ReorderParameter { semantic_id, .. } => {
            if semantic_id.is_empty() || !text_ok(semantic_id, 1024) {
                return Err(task_error("XDF parameter identity is missing or oversized"));
            }
        }
        ProposedXdfOperation::SetHeader {
            title,
            description,
            author,
            version,
            ..
        } => {
            if title.as_ref().is_some_and(|value| !text_ok(value, 512))
                || description
                    .as_ref()
                    .is_some_and(|value| !text_ok(value, 4096))
                || author.as_ref().is_some_and(|value| !text_ok(value, 256))
                || version.as_ref().is_some_and(|value| !text_ok(value, 64))
            {
                return Err(task_error("XDF header contains oversized text"));
            }
        }
        ProposedXdfOperation::SetCategories { categories } => {
            if categories.len() > 256
                || categories.iter().any(|category| {
                    category.name.trim().is_empty() || !text_ok(&category.name, 512)
                })
            {
                return Err(task_error(
                    "XDF categories contain invalid or oversized names",
                ));
            }
        }
    }
    Ok(())
}

fn task_error(message: &str) -> AgentTaskError {
    AgentTaskError {
        message: message.to_owned(),
    }
}

fn clear_raw_read_authorization(record: &mut AgentTaskRecord) {
    record.raw_read_authorization_pending = false;
    record.raw_read_authorized = false;
}

fn is_terminal(status: AgentTaskStatus) -> bool {
    matches!(
        status,
        AgentTaskStatus::Rejected
            | AgentTaskStatus::Cancelled
            | AgentTaskStatus::Failed
            | AgentTaskStatus::Stale
            | AgentTaskStatus::Applied
    )
}

#[cfg(test)]
mod tests {
    use super::{
        AgentCapability, AgentReport, AgentTaskCommand, AgentTaskLevel, AgentTaskRecord,
        AgentTaskStatus, AgentTaskStore, DocumentIdentity, ProposedEndianness, ProposedNumericKind,
        ProposedOperation, ProposedXdfOperation, ProposedXdfParameter, ProposedXdfParameterKind,
        MAX_RECENT_TASKS,
    };

    fn empty_report() -> AgentReport {
        AgentReport {
            summary: String::new(),
            findings: Vec::new(),
            confidence: None,
            evidence: Vec::new(),
            operations: Vec::new(),
        }
    }

    fn start(tasks: &mut AgentTaskStore) -> String {
        let id = tasks
            .start(
                AgentCapability::Diagnostics,
                "inspect mappings".into(),
                DocumentIdentity::default(),
            )
            .unwrap();
        tasks.approve_start(&id, AgentTaskLevel::Quick).unwrap();
        id
    }

    fn record(task_id: String, status: AgentTaskStatus) -> AgentTaskRecord {
        AgentTaskRecord {
            task_id,
            capability: AgentCapability::Diagnostics,
            goal: "inspect mappings".into(),
            document_identity: DocumentIdentity::default(),
            status,
            level: Some(AgentTaskLevel::Quick),
            raw_read_authorization_pending: false,
            raw_read_authorized: false,
            phase: String::new(),
            progress_percent: None,
            message: String::new(),
            report: (status == AgentTaskStatus::AwaitingReview).then(empty_report),
        }
    }

    #[test]
    fn cancelled_task_rejects_late_progress_and_proposals() {
        let mut tasks = AgentTaskStore::default();
        let id = start(&mut tasks);
        tasks.cancel(&id).unwrap();

        assert!(tasks
            .update_progress(&id, "late result".into(), Some(50), String::new())
            .is_err());
        assert!(tasks.submit(&id, empty_report()).is_err());
        assert_eq!(tasks.poll(&id).unwrap().status, AgentTaskStatus::Cancelled);
    }

    #[test]
    fn progress_above_100_is_rejected() {
        let mut tasks = AgentTaskStore::default();
        let id = start(&mut tasks);
        assert!(tasks
            .update_progress(&id, "working".into(), Some(101), String::new())
            .is_err());
    }

    #[test]
    fn confidence_outside_unit_interval_is_rejected() {
        let mut tasks = AgentTaskStore::default();
        let id = start(&mut tasks);
        let mut report = empty_report();
        report.confidence = Some(1.01);
        assert!(tasks.submit(&id, report).is_err());

        for confidence in [0.0, 1.0] {
            let mut report = empty_report();
            report.confidence = Some(confidence);
            let id = start(&mut tasks);
            assert!(tasks.submit(&id, report).is_ok());
        }

        let mut report = empty_report();
        report.confidence = Some(f32::NAN);
        let id = start(&mut tasks);
        assert!(tasks.submit(&id, report).is_err());
    }

    #[test]
    fn accepted_report_without_edits_cannot_be_marked_applied() {
        let mut tasks = AgentTaskStore::default();
        let id = start(&mut tasks);
        tasks.submit(&id, empty_report()).unwrap();
        tasks.decide(&id, true).unwrap();
        assert_eq!(tasks.poll(&id).unwrap().status, AgentTaskStatus::Accepted);
        assert!(tasks.mark_applied(&id).is_err());
    }

    #[test]
    fn accepted_edit_proposal_can_be_marked_applied() {
        let mut tasks = AgentTaskStore::default();
        let id = start(&mut tasks);
        let mut report = empty_report();
        report
            .operations
            .push(ProposedOperation::SetEngineeringCell {
                semantic_id: "fuel-map".into(),
                row: 1,
                column: 2,
                engineering_value: 12.5,
            });
        tasks.submit(&id, report).unwrap();
        tasks.decide(&id, true).unwrap();
        tasks.mark_applied(&id).unwrap();
        assert_eq!(tasks.poll(&id).unwrap().status, AgentTaskStatus::Applied);
    }

    #[test]
    fn rejected_report_is_terminal() {
        let mut tasks = AgentTaskStore::default();
        let id = start(&mut tasks);
        tasks.submit(&id, empty_report()).unwrap();
        tasks.decide(&id, false).unwrap();
        assert!(tasks.decide(&id, true).is_err());
        assert_eq!(tasks.poll(&id).unwrap().status, AgentTaskStatus::Rejected);
    }

    #[test]
    fn failed_task_is_terminal_and_rejects_late_updates() {
        let mut tasks = AgentTaskStore::default();
        let id = start(&mut tasks);
        tasks.fail(&id, "agent aborted".into()).unwrap();

        assert!(tasks
            .update_progress(&id, "late".into(), Some(10), String::new())
            .is_err());
        assert!(tasks.submit(&id, empty_report()).is_err());
        assert!(tasks.cancel(&id).is_err());
        assert!(tasks.fail(&id, "late failure".into()).is_err());
        assert_eq!(tasks.poll(&id).unwrap().status, AgentTaskStatus::Failed);
    }

    #[test]
    fn accepted_proposal_rejects_late_agent_updates_before_and_after_apply() {
        let mut tasks = AgentTaskStore::default();
        let id = start(&mut tasks);
        let mut report = empty_report();
        report
            .operations
            .push(ProposedOperation::SetEngineeringCell {
                semantic_id: "fuel-map".into(),
                row: 0,
                column: 0,
                engineering_value: 1.0,
            });
        tasks.submit(&id, report).unwrap();
        tasks.decide(&id, true).unwrap();

        assert!(tasks
            .update_progress(&id, "late".into(), Some(10), String::new())
            .is_err());
        assert!(tasks.submit(&id, empty_report()).is_err());
        assert!(tasks.fail(&id, "late failure".into()).is_err());
        assert!(tasks.cancel(&id).is_err());
        assert_eq!(tasks.poll(&id).unwrap().status, AgentTaskStatus::Accepted);

        tasks.mark_applied(&id).unwrap();
        assert!(tasks
            .update_progress(&id, "late".into(), Some(10), String::new())
            .is_err());
        assert!(tasks.submit(&id, empty_report()).is_err());
        assert!(tasks.cancel(&id).is_err());
        assert_eq!(tasks.poll(&id).unwrap().status, AgentTaskStatus::Applied);
    }

    #[test]
    fn agent_proposal_mark_stale_clears_authorization_and_rejects_other_states() {
        let mut tasks = AgentTaskStore::default();
        let id = start(&mut tasks);
        tasks.request_raw_read_authorization(&id).unwrap();
        tasks.submit(&id, empty_report()).unwrap();
        tasks.mark_stale(&id).unwrap();
        let record = tasks.poll(&id).unwrap();
        assert_eq!(record.status, AgentTaskStatus::Stale);
        assert!(!record.raw_read_authorization_pending);
        assert!(!record.raw_read_authorized);
        assert!(tasks.mark_stale(&id).is_err());

        let accepted = start(&mut tasks);
        tasks.submit(&accepted, empty_report()).unwrap();
        tasks.decide(&accepted, true).unwrap();
        tasks.mark_stale(&accepted).unwrap();
        assert_eq!(
            tasks.poll(&accepted).unwrap().status,
            AgentTaskStatus::Stale
        );
        let running = start(&mut tasks);
        assert!(tasks.mark_stale(&running).is_err());
    }

    #[test]
    fn task_ids_are_unique_across_store_instances() {
        let mut first_store = AgentTaskStore::default();
        let mut second_store = AgentTaskStore::default();
        let first_id = start(&mut first_store);
        let second_id = start(&mut second_store);

        assert_ne!(first_id, second_id);
        assert_ne!(
            first_id.rsplit('-').next(),
            second_id.rsplit('-').next(),
            "IDs must use a process-wide counter even when timestamps match"
        );
    }

    #[test]
    fn start_waits_for_user_approval_before_running() {
        let mut tasks = AgentTaskStore::default();
        let id = tasks
            .start(
                AgentCapability::Diagnostics,
                "inspect mappings".into(),
                DocumentIdentity::default(),
            )
            .unwrap();
        assert_eq!(
            format!("{:?}", tasks.poll(&id).unwrap().status),
            "AwaitingUserApproval"
        );
        assert!(tasks.cancel(&id).is_ok());
        assert_eq!(tasks.poll(&id).unwrap().status, AgentTaskStatus::Cancelled);
    }

    #[test]
    fn quick_start_approval_runs_task_without_authorizing_raw_reads() {
        let mut tasks = AgentTaskStore::default();
        let id = tasks
            .start(
                AgentCapability::Diagnostics,
                "inspect mappings".into(),
                DocumentIdentity::default(),
            )
            .unwrap();
        tasks.approve_start(&id, AgentTaskLevel::Quick).unwrap();
        let record = tasks.poll(&id).unwrap();
        assert_eq!(record.status, AgentTaskStatus::Running);
        assert_eq!(record.level, Some(AgentTaskLevel::Quick));
        assert!(!record.raw_read_authorized);
    }

    #[test]
    fn raw_read_authorization_requires_running_task_and_captured_bin_identity() {
        let mut tasks = AgentTaskStore::default();
        let id = tasks
            .start(
                AgentCapability::Diagnostics,
                "inspect bytes".into(),
                DocumentIdentity {
                    bin_sha256: Some("captured-hash".into()),
                    xdf_sha256: None,
                    workspace_data_revision: 0,
                },
            )
            .unwrap();
        assert!(tasks.request_raw_read_authorization(&id).is_err());
        tasks.approve_start(&id, AgentTaskLevel::Quick).unwrap();
        tasks.request_raw_read_authorization(&id).unwrap();
        assert!(tasks.allow_raw_reads(&id, "different-hash").is_err());
        let pending = tasks.poll(&id).unwrap();
        assert!(pending.raw_read_authorization_pending);
        assert!(!pending.raw_read_authorized);
        tasks.allow_raw_reads(&id, "captured-hash").unwrap();
        let authorized = tasks.poll(&id).unwrap();
        assert!(!authorized.raw_read_authorization_pending);
        assert!(authorized.raw_read_authorized);
    }

    #[test]
    fn cancellation_invalidates_raw_read_authorization() {
        let mut tasks = AgentTaskStore::default();
        let id = tasks
            .start(
                AgentCapability::Diagnostics,
                "inspect bytes".into(),
                DocumentIdentity {
                    bin_sha256: Some("captured-hash".into()),
                    xdf_sha256: None,
                    workspace_data_revision: 0,
                },
            )
            .unwrap();
        tasks.approve_start(&id, AgentTaskLevel::Quick).unwrap();
        tasks.allow_raw_reads(&id, "captured-hash").unwrap();
        tasks.cancel(&id).unwrap();
        let record = tasks.poll(&id).unwrap();
        assert!(!record.raw_read_authorization_pending);
        assert!(!record.raw_read_authorized);
    }

    #[test]
    fn non_running_transitions_clear_raw_read_authorization() {
        let mut tasks = AgentTaskStore::default();
        let failed_id = tasks
            .start(
                AgentCapability::Diagnostics,
                "inspect bytes".into(),
                DocumentIdentity {
                    bin_sha256: Some("captured-hash".into()),
                    xdf_sha256: None,
                    workspace_data_revision: 0,
                },
            )
            .unwrap();
        tasks
            .approve_start(&failed_id, AgentTaskLevel::Quick)
            .unwrap();
        tasks.allow_raw_reads(&failed_id, "captured-hash").unwrap();
        tasks.fail(&failed_id, "failed".into()).unwrap();
        let failed = tasks.poll(&failed_id).unwrap();
        assert!(!failed.raw_read_authorization_pending);
        assert!(!failed.raw_read_authorized);

        let submitted_id = tasks
            .start(
                AgentCapability::Diagnostics,
                "inspect bytes".into(),
                DocumentIdentity {
                    bin_sha256: Some("captured-hash".into()),
                    xdf_sha256: None,
                    workspace_data_revision: 0,
                },
            )
            .unwrap();
        tasks
            .approve_start(&submitted_id, AgentTaskLevel::Quick)
            .unwrap();
        tasks.request_raw_read_authorization(&submitted_id).unwrap();
        tasks.submit(&submitted_id, empty_report()).unwrap();
        let submitted = tasks.poll(&submitted_id).unwrap();
        assert_eq!(submitted.status, AgentTaskStatus::AwaitingReview);
        assert!(!submitted.raw_read_authorization_pending);
        assert!(!submitted.raw_read_authorized);
    }

    #[test]
    fn retained_records_exposes_current_task_state() {
        let mut tasks = AgentTaskStore::default();
        let id = start(&mut tasks);
        tasks.submit(&id, empty_report()).unwrap();
        let records = tasks.records().collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].task_id, id);
        assert_eq!(records[0].status, AgentTaskStatus::AwaitingReview);
    }

    #[test]
    fn retention_evicts_terminal_records_before_live_records() {
        let mut tasks = AgentTaskStore::default();
        let running_id = start(&mut tasks);
        let review_id = start(&mut tasks);
        tasks.submit(&review_id, empty_report()).unwrap();

        for index in 0..(MAX_RECENT_TASKS - 2) {
            let id = format!("terminal-{index:03}");
            tasks
                .records
                .insert(id.clone(), record(id, AgentTaskStatus::Failed));
        }

        let newest_id = start(&mut tasks);
        assert_eq!(tasks.records.len(), MAX_RECENT_TASKS);
        assert_eq!(
            tasks.poll(&running_id).unwrap().status,
            AgentTaskStatus::Running
        );
        assert_eq!(
            tasks.poll(&review_id).unwrap().status,
            AgentTaskStatus::AwaitingReview
        );
        assert!(tasks.poll("terminal-000").is_err());
        assert_eq!(
            tasks.poll("terminal-001").unwrap().status,
            AgentTaskStatus::Failed
        );
        assert_eq!(
            tasks.poll(&newest_id).unwrap().status,
            AgentTaskStatus::Running
        );
    }

    #[test]
    fn start_rejects_when_all_retained_records_are_active() {
        let mut tasks = AgentTaskStore::default();
        let running_id = start(&mut tasks);
        let review_id = start(&mut tasks);
        tasks.submit(&review_id, empty_report()).unwrap();
        for index in 0..(MAX_RECENT_TASKS - 2) {
            let id = format!("active-{index:03}");
            tasks
                .records
                .insert(id.clone(), record(id, AgentTaskStatus::Running));
        }

        assert_eq!(tasks.records.len(), MAX_RECENT_TASKS);

        assert!(tasks
            .start(
                AgentCapability::Diagnostics,
                "one more".into(),
                DocumentIdentity::default(),
            )
            .is_err());
        assert_eq!(
            tasks.poll(&running_id).unwrap().status,
            AgentTaskStatus::Running
        );
        assert_eq!(
            tasks.poll(&review_id).unwrap().status,
            AgentTaskStatus::AwaitingReview
        );
    }

    #[test]
    fn agent_task_wire_names_are_snake_case() {
        let command = AgentTaskCommand::Start {
            capability: AgentCapability::Diagnostics,
            goal: "inspect mappings".into(),
        };
        let encoded = serde_json::to_value(&command).unwrap();
        assert_eq!(
            encoded,
            serde_json::json!({
                "kind": "start",
                "capability": "diagnostics",
                "goal": "inspect mappings",
            })
        );

        let decoded: AgentTaskCommand = serde_json::from_value(serde_json::json!({
            "kind": "start",
            "capability": "diagnostics",
            "goal": "inspect mappings",
        }))
        .unwrap();
        assert!(matches!(
            decoded,
            AgentTaskCommand::Start {
                capability: AgentCapability::Diagnostics,
                goal,
            } if goal == "inspect mappings"
        ));
        assert_eq!(
            serde_json::to_value(AgentTaskStatus::AwaitingReview).unwrap(),
            serde_json::json!("awaiting_review")
        );
    }

    #[test]
    fn legacy_document_identity_defaults_workspace_revision_to_zero() {
        let identity: super::DocumentIdentity = serde_json::from_value(serde_json::json!({
            "bin_sha256": "bin-hash",
            "xdf_sha256": "xdf-hash"
        }))
        .unwrap();
        assert_eq!(identity.workspace_data_revision, 0);
    }

    #[test]
    fn agent_task_history_emits_each_decision_with_revision_and_bounded_provenance() {
        let mut tasks = AgentTaskStore::default();
        let id = tasks
            .start(
                AgentCapability::Diagnostics,
                "secret goal token".into(),
                DocumentIdentity {
                    bin_sha256: Some("bin-hash".into()),
                    xdf_sha256: Some("xdf-hash".into()),
                    workspace_data_revision: 12,
                },
            )
            .unwrap();
        tasks.approve_start(&id, AgentTaskLevel::Quick).unwrap();
        tasks
            .submit(
                &id,
                AgentReport {
                    summary: "secret report token".into(),
                    findings: vec!["secret finding token".into()],
                    confidence: None,
                    evidence: vec![super::AgentEvidence {
                        summary: "secret evidence token".into(),
                        semantic_id: Some("fuel-map".into()),
                        byte_range: Some([16, 18]),
                    }],
                    operations: vec![ProposedOperation::SetEngineeringCell {
                        semantic_id: "fuel-map".into(),
                        row: 1,
                        column: 2,
                        engineering_value: 10.0,
                    }],
                },
            )
            .unwrap();
        tasks.decide(&id, true).unwrap();
        tasks.mark_applied(&id).unwrap();
        let events = tasks.take_history();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].status, AgentTaskStatus::Accepted);
        assert_eq!(events[1].status, AgentTaskStatus::Applied);
        assert_eq!(events[1].document_identity.workspace_data_revision, 12);
        assert_eq!(events[1].evidence[0].byte_range, Some([16, 18]));
        assert_eq!(
            events[1].applied_operations,
            vec!["set_engineering_cell:fuel-map:1:2"]
        );
        assert!(!serde_json::to_string(&events).unwrap().contains("secret"));
        assert!(tasks.take_history().is_empty());
    }

    #[test]
    fn agent_task_history_emits_reject_cancel_stale_and_failure() {
        let mut tasks = AgentTaskStore::default();
        let rejected = start(&mut tasks);
        tasks.submit(&rejected, empty_report()).unwrap();
        tasks.decide(&rejected, false).unwrap();
        let cancelled = start(&mut tasks);
        tasks.cancel(&cancelled).unwrap();
        let stale = start(&mut tasks);
        tasks.submit(&stale, empty_report()).unwrap();
        tasks.mark_stale(&stale).unwrap();
        let failed = start(&mut tasks);
        tasks.fail(&failed, "private failure token".into()).unwrap();
        let events = tasks.take_history();
        assert_eq!(
            events.iter().map(|item| item.status).collect::<Vec<_>>(),
            vec![
                AgentTaskStatus::Rejected,
                AgentTaskStatus::Cancelled,
                AgentTaskStatus::Stale,
                AgentTaskStatus::Failed,
            ]
        );
        assert!(!serde_json::to_string(&events)
            .unwrap()
            .contains("private failure token"));
    }

    #[test]
    fn xdf_authoring_proposals_require_the_dedicated_capability() {
        let mut tasks = AgentTaskStore::default();
        let diagnostic = tasks
            .start(
                AgentCapability::Diagnostics,
                "inspect XDF".into(),
                DocumentIdentity::default(),
            )
            .unwrap();
        tasks
            .approve_start(&diagnostic, AgentTaskLevel::Quick)
            .unwrap();
        let report = xdf_report();
        assert!(tasks.submit(&diagnostic, report.clone()).is_err());

        let authoring = tasks
            .start(
                AgentCapability::XdfAuthoring,
                "add a table definition".into(),
                DocumentIdentity::default(),
            )
            .unwrap();
        tasks
            .approve_start(&authoring, AgentTaskLevel::Quick)
            .unwrap();
        assert!(tasks.submit(&authoring, report).is_ok());
    }

    #[test]
    fn xdf_authoring_tasks_cannot_propose_bin_cell_writes() {
        let mut tasks = AgentTaskStore::default();
        let id = tasks
            .start(
                AgentCapability::XdfAuthoring,
                "author XDF definitions".into(),
                DocumentIdentity::default(),
            )
            .unwrap();
        tasks.approve_start(&id, AgentTaskLevel::Quick).unwrap();
        let mut report = empty_report();
        report
            .operations
            .push(ProposedOperation::SetEngineeringCell {
                semantic_id: "table:uid:map".into(),
                row: 0,
                column: 0,
                engineering_value: 1.0,
            });
        assert!(tasks.submit(&id, report).is_err());
    }

    #[test]
    fn xdf_authoring_payload_round_trips_as_typed_snake_case_json() {
        let report = xdf_report();
        let serialized = serde_json::to_string(&report).unwrap();
        assert!(serialized.contains("\"XdfAuthoring\""));
        assert!(serialized.contains("\"add_parameter\""));
        assert!(serialized.contains("\"xdf_address\":32"));
        let decoded: AgentReport = serde_json::from_str(&serialized).unwrap();
        assert_eq!(decoded, report);
        assert_eq!(
            serde_json::to_string(&AgentCapability::XdfAuthoring).unwrap(),
            "\"xdf_authoring\""
        );
    }

    fn xdf_report() -> AgentReport {
        let mut report = empty_report();
        report.operations.push(ProposedOperation::XdfAuthoring {
            operations: vec![ProposedXdfOperation::AddParameter {
                definition: ProposedXdfParameter {
                    unique_id: Some("agent-map".into()),
                    kind: ProposedXdfParameterKind::Table,
                    title: "Agent map".into(),
                    description: String::new(),
                    category: None,
                    xdf_address: 0x20,
                    element_width_bits: 8,
                    rows: 1,
                    columns: 1,
                    signed: false,
                    endianness: ProposedEndianness::Little,
                    numeric_kind: ProposedNumericKind::Integer,
                    column_major: false,
                    row_stride_bits: 8,
                    column_stride_bits: 8,
                    conversion: None,
                    bit_offset: None,
                    bit_width: None,
                    bit_mask: None,
                    axes: Vec::new(),
                },
            }],
        });
        report
    }
}
