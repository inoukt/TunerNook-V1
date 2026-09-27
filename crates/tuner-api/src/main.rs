use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::io::{self, BufRead, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process;

use tuner_core::{
    compare_bytes, json_escape, sha256_hex, BinDocument, ByteRange, CoreError, DiagnosticLogger,
    LogContext, LOG_SCHEMA,
};
use tuner_transfer::{
    ParameterTransfer, TransferError, TransferIssue, TransferOptions, TransferPlan,
};
use tuner_xdf::{ParameterDefinition, ValidationIssue, XdfDocument, XdfError};

const API_PROTOCOL: &str = "tuner-api/v1";
const API_RESULT_SCHEMA: &str = "tuner-api-result/v1";

#[derive(Debug)]
struct ApiError {
    message: String,
    context: LogContext,
}

impl ApiError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            context: LogContext::new(),
        }
    }

    fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key, value);
        self
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

struct ApiResult {
    command: String,
    message: String,
    context: LogContext,
    fields: Vec<(String, String)>,
}

fn main() {
    let (log_path, show_help) = match extract_global_options(env::args().skip(1).collect()) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("tuner-api: {error}");
            process::exit(2);
        }
    };
    if show_help {
        print_help();
        return;
    }

    let stdin = io::stdin();
    let mut stdout = BufWriter::new(io::stdout().lock());
    let mut fatal = false;
    for (index, line_result) in stdin.lock().lines().enumerate() {
        let line_number = index + 1;
        let line = match line_result {
            Ok(line) => line,
            Err(error) => {
                let response = error_response(
                    "request",
                    None,
                    &ApiError::new(format!("could not read JSONL request: {error}"))
                        .with("line_number", line_number.to_string()),
                    None,
                );
                if writeln!(stdout, "{response}").is_err() {
                    fatal = true;
                    break;
                }
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }

        let request = match parse_request(&line) {
            Ok(request) => request,
            Err(message) => {
                let response =
                    process_parse_error(&line, line_number, &message, log_path.as_deref());
                if writeln!(stdout, "{response}").is_err() {
                    fatal = true;
                    break;
                }
                continue;
            }
        };
        let response = process_request(request, &line, line_number, log_path.as_deref());
        if writeln!(stdout, "{response}").is_err() {
            fatal = true;
            break;
        }
        if stdout.flush().is_err() {
            fatal = true;
            break;
        }
    }
    if stdout.flush().is_err() {
        fatal = true;
    }
    if fatal {
        process::exit(1);
    }
}

fn extract_global_options(args: Vec<String>) -> Result<(Option<PathBuf>, bool), String> {
    let mut log_path = None;
    let mut show_help = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--help" | "-h" => {
                show_help = true;
                index += 1;
            }
            "--log-file" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--log-file requires a path".to_string())?;
                if value.starts_with('-') {
                    return Err("--log-file requires a path".to_string());
                }
                if log_path.replace(PathBuf::from(value)).is_some() {
                    return Err("--log-file may only be supplied once".to_string());
                }
                index += 2;
            }
            value => return Err(format!("unknown option '{value}'")),
        }
    }
    Ok((log_path, show_help))
}

fn print_help() {
    println!(
        "{}",
        "TunerNook local agent API ("
            .to_string()
            + API_PROTOCOL
            + ")\n\n"
            + "Run as a JSONL process: write one JSON object per stdin line and read one JSON result per stdout line.\n"
            + "Actions:\n"
            + "  {\"action\":\"capabilities\",\"request_id\":\"1\"}\n"
            + "  {\"action\":\"inspect\",\"input\":\"stock.bin\"}\n"
            + "  {\"action\":\"inspect_xdf\",\"request_id\":\"r-3\",\"xdf\":\"definition.xdf\"}\n"
            + "  {\"action\":\"transfer_plan\",\"request_id\":\"r-5\",\"source_bin\":\"source.bin\",\"destination_bin\":\"stock.bin\",\"source_xdf\":\"definition.xdf\",\"destination_xdf\":\"definition.xdf\",\"selected\":[\"constant:uid:rpm\"]}\n"
            + "  {\"action\":\"transfer_apply\",\"request_id\":\"r-6\",\"source_bin\":\"source.bin\",\"destination_bin\":\"stock.bin\",\"source_xdf\":\"definition.xdf\",\"destination_xdf\":\"definition.xdf\",\"plan_id\":\"...\",\"output\":\"candidate.bin\",\"selected\":[\"constant:uid:rpm\"]}\n"
            + "  {\"action\":\"plan_edit\",\"request_id\":\"r-7\",\"input\":\"stock.bin\",\"offset\":288,\"value\":127}\n"
            + "  {\"action\":\"apply_edit\",\"request_id\":\"r-8\",\"plan_id\":\"...\",\"input\":\"stock.bin\",\"expected_sha256\":\"...\",\"offset\":288,\"value\":127,\"output\":\"candidate.bin\"}\n\n"
            + "Every response is one JSON object. Diagnostics are self-contained JSON Lines on stderr; --log-file duplicates them to a file.\n"
    );
}

fn process_parse_error(
    line: &str,
    line_number: usize,
    message: &str,
    log_path: Option<&Path>,
) -> String {
    let operation = "api.parse";
    let mut context = LogContext::new();
    context.insert("line_number", line_number.to_string());
    context.insert("request_bytes", line.len().to_string());
    context.insert("request_sha256", sha256_hex(line.as_bytes()));
    context.insert("parse_error", message);

    let mut logger = match DiagnosticLogger::new("tuner-api", operation, log_path) {
        Ok(logger) => logger,
        Err(error) => {
            return error_response(
                "request",
                None,
                &ApiError::new(format!("invalid JSON request: {message}"))
                    .with("line_number", line_number.to_string())
                    .with("logger_error", error.to_string()),
                None,
            );
        }
    };
    logger.add_context("protocol", API_PROTOCOL);
    let started = logger.started("request JSON parsing failed", &context);
    if let Ok(started) = started {
        let _ = logger.aborted("invalid JSON request", &context, started.elapsed());
    }
    error_response(
        "request",
        None,
        &ApiError::new(format!("invalid JSON request: {message}"))
            .with("line_number", line_number.to_string())
            .with("request_sha256", sha256_hex(line.as_bytes())),
        Some(logger.operation_id()),
    )
}

fn process_request(
    request: JsonValue,
    line: &str,
    line_number: usize,
    log_path: Option<&Path>,
) -> String {
    let request_id = request_string(&request, "request_id").ok();
    let action = action_name(&request);
    let operation = format!("api.{action}");
    let mut logger = match DiagnosticLogger::new("tuner-api", &operation, log_path) {
        Ok(logger) => logger,
        Err(error) => {
            return error_response(
                &action,
                request_id.as_deref(),
                &ApiError::new("could not initialize diagnostics")
                    .with("logger_error", error.to_string())
                    .with("line_number", line_number.to_string()),
                None,
            );
        }
    };
    logger.add_context("protocol", API_PROTOCOL);
    logger.add_context("line_number", line_number.to_string());
    logger.add_context("request_bytes", line.len().to_string());
    logger.add_context("request_sha256", sha256_hex(line.as_bytes()));
    logger.add_context("action", action.clone());
    if let Some(request_id) = &request_id {
        logger.add_context("request_id", request_id.clone());
    }

    let mut start_context = LogContext::new();
    start_context.insert("request_received", "true");
    let started = match logger.started("API request started", &start_context) {
        Ok(started) => started,
        Err(error) => {
            return error_response(
                &action,
                request_id.as_deref(),
                &ApiError::new("could not write request-start diagnostic")
                    .with("logger_error", error.to_string()),
                Some(logger.operation_id()),
            );
        }
    };

    match execute_request(&request) {
        Ok(result) => {
            let mut context = result.context.clone();
            context.insert("status", "ok");
            let _ = logger.complete(&result.message, &context, started.elapsed());
            success_response(
                &result.command,
                &result.message,
                request_id.as_deref(),
                &result.fields,
                Some(logger.operation_id()),
            )
        }
        Err(error) => {
            let mut context = error.context.clone();
            context.insert("status", "error");
            let _ = logger.aborted(&error.message, &context, started.elapsed());
            error_response(
                &action,
                request_id.as_deref(),
                &error,
                Some(logger.operation_id()),
            )
        }
    }
}

fn execute_request(request: &JsonValue) -> Result<ApiResult, ApiError> {
    let action = request_string(request, "action")?;
    match action.as_str() {
        "capabilities" => capabilities_command(),
        "inspect" => inspect_command(request),
        "compare" => compare_command(request),
        "inspect_xdf" => inspect_xdf_command(request),
        "xdf_validate" => xdf_validate_command(request),
        "transfer_plan" => transfer_plan_command(request),
        "transfer_apply" => transfer_apply_command(request),
        "plan_edit" => plan_edit_command(request),
        "apply_edit" => apply_edit_command(request),
        _ => Err(ApiError::new(format!(
            "unsupported action '{action}'; request capabilities for the supported protocol"
        ))
        .with("action", action)),
    }
}

fn transfer_plan_command(request: &JsonValue) -> Result<ApiResult, ApiError> {
    let arguments = transfer_request_arguments(request)?;
    let source_xdf = XdfDocument::load(&arguments.source_xdf)
        .map_err(|error| xdf_load_error("transfer_plan", &arguments.source_xdf, error))?;
    let destination_xdf = XdfDocument::load(&arguments.destination_xdf)
        .map_err(|error| xdf_load_error("transfer_plan", &arguments.destination_xdf, error))?;
    let source_bin = load_document("transfer_plan", &arguments.source_bin).map_err(|error| {
        ApiError::new(error.to_string())
            .with("operation", "transfer_plan")
            .with("path", arguments.source_bin.display().to_string())
    })?;
    let destination_bin =
        load_document("transfer_plan", &arguments.destination_bin).map_err(|error| {
            ApiError::new(error.to_string())
                .with("operation", "transfer_plan")
                .with("path", arguments.destination_bin.display().to_string())
        })?;
    let options = TransferOptions {
        selected: arguments.selected,
        approved_address_reviews: arguments.approved_address_reviews.clone(),
    };
    let plan = TransferPlan::build(
        &source_xdf,
        &destination_xdf,
        &source_bin,
        &destination_bin,
        &options,
    );
    let mut context = LogContext::new();
    context.insert("source_bin", arguments.source_bin.display().to_string());
    context.insert(
        "destination_bin",
        arguments.destination_bin.display().to_string(),
    );
    context.insert("source_xdf", arguments.source_xdf.display().to_string());
    context.insert(
        "destination_xdf",
        arguments.destination_xdf.display().to_string(),
    );
    context.insert("plan_id", plan.plan_id.clone());
    context.insert("plan_status", plan.status().to_string());
    context.insert("selected_count", plan.selected_count.to_string());
    context.insert("changed_bytes", plan.changed_bytes.to_string());
    context.insert("issue_count", plan.issues.len().to_string());
    Ok(ApiResult {
        command: "transfer_plan".to_string(),
        message: transfer_plan_message(&plan),
        context,
        fields: transfer_plan_fields(&plan),
    })
}

fn transfer_apply_command(request: &JsonValue) -> Result<ApiResult, ApiError> {
    let arguments = transfer_request_arguments(request)?;
    let supplied_plan_id = arguments
        .plan_id
        .clone()
        .ok_or_else(|| ApiError::new("transfer_apply is missing plan_id".to_string()))?;
    let output = arguments
        .output
        .clone()
        .ok_or_else(|| ApiError::new("transfer_apply is missing output".to_string()))?;
    let source_xdf = XdfDocument::load(&arguments.source_xdf)
        .map_err(|error| xdf_load_error("transfer_apply", &arguments.source_xdf, error))?;
    let destination_xdf = XdfDocument::load(&arguments.destination_xdf)
        .map_err(|error| xdf_load_error("transfer_apply", &arguments.destination_xdf, error))?;
    let source_bin = load_document("transfer_apply", &arguments.source_bin).map_err(|error| {
        ApiError::new(error.to_string())
            .with("operation", "transfer_apply")
            .with("path", arguments.source_bin.display().to_string())
    })?;
    let mut destination_bin =
        load_document("transfer_apply", &arguments.destination_bin).map_err(|error| {
            ApiError::new(error.to_string())
                .with("operation", "transfer_apply")
                .with("path", arguments.destination_bin.display().to_string())
        })?;
    let options = TransferOptions {
        selected: arguments.selected,
        approved_address_reviews: arguments.approved_address_reviews.clone(),
    };
    let plan = TransferPlan::build(
        &source_xdf,
        &destination_xdf,
        &source_bin,
        &destination_bin,
        &options,
    );
    if plan.plan_id != supplied_plan_id {
        return Err(ApiError::new(
            "transfer plan ID does not match the current files and approvals; re-plan before applying",
        )
        .with("supplied_plan_id", supplied_plan_id)
        .with("current_plan_id", plan.plan_id));
    }
    let applied = plan
        .apply(&source_bin, &mut destination_bin)
        .map_err(|error| transfer_apply_error("transfer_apply", error))?;
    destination_bin.save_as(&output).map_err(|error| {
        ApiError::new(error.to_string())
            .with("operation", "transfer_apply")
            .with("output", output.display().to_string())
            .with("plan_id", plan.plan_id.clone())
    })?;
    let mut context = LogContext::new();
    context.insert("source_bin", arguments.source_bin.display().to_string());
    context.insert(
        "destination_bin",
        arguments.destination_bin.display().to_string(),
    );
    context.insert("output", output.display().to_string());
    context.insert("plan_id", plan.plan_id.clone());
    context.insert("changed_bytes", applied.changed_bytes.to_string());
    context.insert("result_sha256", applied.result_sha256.clone());
    Ok(ApiResult {
        command: "transfer_apply".to_string(),
        message: "validated transfer applied and exported".to_string(),
        context,
        fields: vec![
            ("plan_id".to_string(), string_json(&plan.plan_id)),
            (
                "output".to_string(),
                string_json(&output.display().to_string()),
            ),
            (
                "changed_bytes".to_string(),
                applied.changed_bytes.to_string(),
            ),
            (
                "result_sha256".to_string(),
                string_json(&applied.result_sha256),
            ),
            ("written".to_string(), "true".to_string()),
            ("overwrote_existing".to_string(), "false".to_string()),
        ],
    })
}

struct TransferRequestArguments {
    source_bin: PathBuf,
    destination_bin: PathBuf,
    source_xdf: PathBuf,
    destination_xdf: PathBuf,
    selected: Option<BTreeSet<String>>,
    approved_address_reviews: BTreeSet<String>,
    plan_id: Option<String>,
    output: Option<PathBuf>,
}

fn transfer_request_arguments(request: &JsonValue) -> Result<TransferRequestArguments, ApiError> {
    let source_bin = required_path(request, "source_bin")?;
    let destination_bin = required_path(request, "destination_bin")?;
    let source_xdf = required_path(request, "source_xdf")?;
    let destination_xdf = required_path(request, "destination_xdf")?;
    let selected = request.get("selected").and_then(|value| match value {
        JsonValue::Array(values) => {
            let mut out = BTreeSet::new();
            for value in values {
                match value {
                    JsonValue::String(text) if !text.trim().is_empty() => {
                        out.insert(text.clone());
                    }
                    _ => return None,
                }
            }
            Some(out)
        }
        _ => None,
    });
    let approved_address_reviews = request
        .get("approved_address_reviews")
        .and_then(|value| match value {
            JsonValue::Array(values) => {
                let mut out = BTreeSet::new();
                for value in values {
                    if let JsonValue::String(text) = value {
                        out.insert(text.clone());
                    }
                }
                Some(out)
            }
            _ => None,
        })
        .unwrap_or_default();
    let plan_id = request.get("plan_id").and_then(|value| match value {
        JsonValue::String(value) => Some(value.clone()),
        _ => None,
    });
    let output = request.get("output").and_then(|value| match value {
        JsonValue::String(value) => Some(PathBuf::from(value)),
        _ => None,
    });
    Ok(TransferRequestArguments {
        source_bin,
        destination_bin,
        source_xdf,
        destination_xdf,
        selected,
        approved_address_reviews,
        plan_id,
        output,
    })
}

fn transfer_plan_message(plan: &TransferPlan) -> String {
    let status = plan.status();
    match status {
        "ready" => format!(
            "transfer plan is ready for {0} bytes across {1} selected parameters",
            plan.changed_bytes, plan.selected_count,
        ),
        "review_required" => {
            format!(
                "transfer plan requires address review for {0} selected parameters",
                plan.selected_count,
            )
        }
        _ => format!(
            "transfer plan is blocked for {0} selected parameters",
            plan.selected_count,
        ),
    }
}

fn transfer_plan_fields(plan: &TransferPlan) -> Vec<(String, String)> {
    let entries = plan
        .entries
        .iter()
        .map(transfer_entry_json)
        .collect::<Vec<_>>()
        .join(",");
    let issues = plan
        .issues
        .iter()
        .map(transfer_issue_json)
        .collect::<Vec<_>>()
        .join(",");
    let changed_ranges = plan
        .changed_ranges
        .iter()
        .map(|range| range_json(*range))
        .collect::<Vec<_>>()
        .join(",");
    vec![
        ("plan_id".to_string(), string_json(&plan.plan_id)),
        (
            "source_xdf_sha256".to_string(),
            string_json(&plan.source_xdf_sha256),
        ),
        (
            "destination_xdf_sha256".to_string(),
            string_json(&plan.destination_xdf_sha256),
        ),
        (
            "source_xdf_fingerprint".to_string(),
            string_json(&plan.source_xdf_fingerprint),
        ),
        (
            "destination_xdf_fingerprint".to_string(),
            string_json(&plan.destination_xdf_fingerprint),
        ),
        (
            "source_bin_sha256".to_string(),
            string_json(&plan.source_bin_sha256),
        ),
        (
            "destination_bin_sha256".to_string(),
            string_json(&plan.destination_bin_sha256),
        ),
        (
            "xdf_hash_match".to_string(),
            plan.xdf_hash_match.to_string(),
        ),
        ("status".to_string(), string_json(plan.status())),
        (
            "selected_count".to_string(),
            plan.selected_count.to_string(),
        ),
        ("changed_bytes".to_string(), plan.changed_bytes.to_string()),
        ("issue_count".to_string(), plan.issues.len().to_string()),
        ("entries".to_string(), format!("[{entries}]")),
        ("issues".to_string(), format!("[{issues}]")),
        ("changed_ranges".to_string(), format!("[{changed_ranges}]")),
    ]
}

fn range_json(range: ByteRange) -> String {
    format!(
        "{{\"start\":{},\"end\":{},\"length\":{}}}",
        range.start,
        range.end,
        range.len()
    )
}

fn transfer_entry_json(entry: &ParameterTransfer) -> String {
    let source_range = entry
        .source_range
        .map(range_json)
        .unwrap_or_else(|| "null".to_string());
    let destination_range = entry
        .destination_range
        .map(range_json)
        .unwrap_or_else(|| "null".to_string());
    let changed_ranges = entry
        .changed_ranges
        .iter()
        .map(|range| range_json(*range))
        .collect::<Vec<_>>()
        .join(",");
    let issues = entry
        .issues
        .iter()
        .map(transfer_issue_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"semantic_id\":{},\"title\":{},\"kind\":{},\"source_range\":{},\"destination_range\":{},\"source_address\":{},\"destination_address\":{},\"source_size_bytes\":{},\"destination_size_bytes\":{},\"changed_bytes\":{},\"fast_path\":{},\"status\":{},\"changed_ranges\":[{}],\"issues\":[{}]}}",
        string_json(&entry.semantic_id),
        string_json(&entry.title),
        string_json(entry.kind.as_str()),
        source_range,
        destination_range,
        optional_number_json(entry.source_address),
        optional_number_json(entry.destination_address),
        entry.source_size_bytes,
        entry.destination_size_bytes,
        entry.changed_bytes,
        entry.fast_path,
        string_json(entry.status.as_str()),
        changed_ranges,
        issues,
    )
}

fn transfer_issue_json(issue: &TransferIssue) -> String {
    format!(
        "{{\"code\":{},\"semantic_id\":{},\"message\":{},\"blocking\":{}}}",
        string_json(issue.code.as_str()),
        optional_string_json(issue.semantic_id.as_deref()),
        string_json(&issue.message),
        issue.blocking,
    )
}

fn transfer_apply_error(_operation: &str, error: TransferError) -> ApiError {
    match error {
        TransferError::PlanBlocked { plan_id, message } => {
            ApiError::new(message).with("plan_id", plan_id)
        }
        TransferError::SourceChanged { expected, actual } => ApiError::new(format!(
            "source BIN changed since planning; expected {expected}, actual {actual}"
        ))
        .with("expected", expected)
        .with("actual", actual),
        TransferError::DestinationChanged { expected, actual } => ApiError::new(format!(
            "destination BIN changed since planning; expected {expected}, actual {actual}"
        ))
        .with("expected", expected)
        .with("actual", actual),
        TransferError::Core { operation, error } => {
            ApiError::new(format!("transfer core error during {operation}: {error}"))
                .with("operation", operation)
                .with("error", error.to_string())
        }
    }
}

fn capabilities_command() -> Result<ApiResult, ApiError> {
    let mut context = LogContext::new();
    context.insert("capability_count", "9");
    Ok(ApiResult {
        command: "capabilities".to_string(),
        message: "API capabilities returned".to_string(),
        context,
        fields: vec![
            ("protocol".to_string(), string_json(API_PROTOCOL)),
            ("log_schema".to_string(), string_json(LOG_SCHEMA)),
            (
                "actions".to_string(),
                "[\"capabilities\",\"inspect\",\"compare\",\"inspect_xdf\",\"xdf_validate\",\"transfer_plan\",\"transfer_apply\",\"plan_edit\",\"apply_edit\"]"
                    .to_string(),
            ),
            ("supports_dry_run".to_string(), "true".to_string()),
            ("requires_expected_sha256".to_string(), "true".to_string()),
            (
                "write_policy".to_string(),
                string_json("apply_edit requires an exported plan, a matching source hash, and a new output path"),
            ),
        ],
    })
}

fn inspect_command(request: &JsonValue) -> Result<ApiResult, ApiError> {
    let input = required_path(request, "input")?;
    let document = load_document("inspect", &input)?;
    let digest = sha256_hex(document.bytes());
    let mut context = LogContext::new();
    context.insert("input", input.display().to_string());
    context.insert("size_bytes", document.len().to_string());
    context.insert("sha256", digest.clone());
    context.insert("changed_bytes", "0");
    Ok(ApiResult {
        command: "inspect".to_string(),
        message: "BIN inspected".to_string(),
        context,
        fields: vec![
            (
                "input".to_string(),
                string_json(&input.display().to_string()),
            ),
            ("size_bytes".to_string(), document.len().to_string()),
            ("sha256".to_string(), string_json(&digest)),
            ("dirty".to_string(), "false".to_string()),
            (
                "head_hex".to_string(),
                string_json(&hex_preview(document.bytes(), 32)),
            ),
        ],
    })
}

fn compare_command(request: &JsonValue) -> Result<ApiResult, ApiError> {
    let left_path = required_path(request, "left")?;
    let right_path = required_path(request, "right")?;
    let left = load_document("compare", &left_path)?;
    let right = load_document("compare", &right_path)?;
    let comparison = compare_bytes(left.bytes(), right.bytes());
    let left_hash = sha256_hex(left.bytes());
    let right_hash = sha256_hex(right.bytes());
    let ranges = comparison
        .ranges
        .iter()
        .map(|range| {
            format!(
                "{{\"start\":{},\"end\":{},\"length\":{}}}",
                range.start,
                range.end,
                range.len()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut context = LogContext::new();
    context.insert("left", left_path.display().to_string());
    context.insert("right", right_path.display().to_string());
    context.insert("left_size_bytes", left.len().to_string());
    context.insert("right_size_bytes", right.len().to_string());
    context.insert("changed_bytes", comparison.changed_bytes.to_string());
    context.insert("changed_ranges", comparison.ranges.len().to_string());
    context.insert("identical", comparison.identical().to_string());
    Ok(ApiResult {
        command: "compare".to_string(),
        message: if comparison.identical() {
            "BINs are identical".to_string()
        } else {
            "BIN comparison completed".to_string()
        },
        context,
        fields: vec![
            (
                "left".to_string(),
                string_json(&left_path.display().to_string()),
            ),
            (
                "right".to_string(),
                string_json(&right_path.display().to_string()),
            ),
            ("left_size_bytes".to_string(), left.len().to_string()),
            ("right_size_bytes".to_string(), right.len().to_string()),
            ("left_sha256".to_string(), string_json(&left_hash)),
            ("right_sha256".to_string(), string_json(&right_hash)),
            ("identical".to_string(), comparison.identical().to_string()),
            (
                "changed_bytes".to_string(),
                comparison.changed_bytes.to_string(),
            ),
            ("changed_ranges".to_string(), format!("[{ranges}]")),
        ],
    })
}

fn inspect_xdf_command(request: &JsonValue) -> Result<ApiResult, ApiError> {
    let xdf_path = required_path(request, "xdf")?;
    let document = XdfDocument::load(&xdf_path)
        .map_err(|error| xdf_load_error("inspect_xdf", &xdf_path, error))?;
    let mut context = LogContext::new();
    context.insert("xdf", xdf_path.display().to_string());
    context.insert("exact_sha256", document.exact_sha256.clone());
    context.insert(
        "normalized_fingerprint",
        document.normalized_fingerprint.clone(),
    );
    context.insert("parameter_count", document.parameters.len().to_string());
    context.insert(
        "category_reference_mode",
        category_reference_mode_name(document.category_reference_mode).to_string(),
    );
    context.insert(
        "unknown_element_count",
        document.unknown_element_count.to_string(),
    );
    Ok(ApiResult {
        command: "inspect_xdf".to_string(),
        message: "XDF inspected".to_string(),
        context,
        fields: inspection_fields(&document, &xdf_path),
    })
}

fn inspection_fields(document: &XdfDocument, path: &Path) -> Vec<(String, String)> {
    let categories = document
        .categories
        .iter()
        .map(category_json)
        .collect::<Vec<_>>()
        .join(",");
    let diagnostics = document
        .diagnostics
        .iter()
        .map(diagnostic_json)
        .collect::<Vec<_>>()
        .join(",");
    let auxiliary_objects = document
        .auxiliary_objects
        .iter()
        .map(auxiliary_object_json)
        .collect::<Vec<_>>()
        .join(",");
    let parameters = document
        .parameters
        .iter()
        .map(parameter_json)
        .collect::<Vec<_>>()
        .join(",");
    vec![
        ("xdf".to_string(), string_json(&path.display().to_string())),
        (
            "exact_sha256".to_string(),
            string_json(&document.exact_sha256),
        ),
        (
            "normalized_fingerprint".to_string(),
            string_json(&document.normalized_fingerprint),
        ),
        (
            "parameter_count".to_string(),
            document.parameters.len().to_string(),
        ),
        (
            "unknown_element_count".to_string(),
            document.unknown_element_count.to_string(),
        ),
        (
            "category_count".to_string(),
            document.categories.len().to_string(),
        ),
        (
            "category_reference_mode".to_string(),
            string_json(category_reference_mode_name(
                document.category_reference_mode,
            )),
        ),
        (
            "diagnostic_count".to_string(),
            document.diagnostics.len().to_string(),
        ),
        (
            "auxiliary_object_count".to_string(),
            document.auxiliary_objects.len().to_string(),
        ),
        ("header".to_string(), header_json(&document.header)),
        ("categories".to_string(), format!("[{categories}]")),
        ("diagnostics".to_string(), format!("[{diagnostics}]")),
        (
            "auxiliary_objects".to_string(),
            format!("[{auxiliary_objects}]"),
        ),
        ("parameters".to_string(), format!("[{parameters}]")),
    ]
}

fn xdf_validate_command(request: &JsonValue) -> Result<ApiResult, ApiError> {
    let xdf_path = required_path(request, "xdf")?;
    let bin_path = required_path(request, "bin")?;
    let xdf = XdfDocument::load(&xdf_path)
        .map_err(|error| xdf_load_error("xdf_validate", &xdf_path, error))?;
    let bin = load_document("xdf_validate", &bin_path)?;
    let report = xdf.validate_against_document(&bin);
    let issues = report
        .issues
        .iter()
        .map(validation_issue_json)
        .collect::<Vec<_>>()
        .join(",");
    let bin_hash = sha256_hex(bin.bytes());
    let mut context = LogContext::new();
    context.insert("xdf", xdf_path.display().to_string());
    context.insert("bin", bin_path.display().to_string());
    context.insert("xdf_exact_sha256", xdf.exact_sha256.clone());
    context.insert(
        "xdf_normalized_fingerprint",
        xdf.normalized_fingerprint.clone(),
    );
    context.insert("bin_sha256", bin_hash.clone());
    context.insert("bin_size_bytes", bin.len().to_string());
    context.insert("parameter_count", xdf.parameters.len().to_string());
    context.insert("issue_count", report.issue_count().to_string());
    context.insert("valid", report.is_valid().to_string());
    Ok(ApiResult {
        command: "xdf_validate".to_string(),
        message: if report.is_valid() {
            "XDF mappings are valid against BIN".to_string()
        } else {
            "XDF validation found blocking mapping issues".to_string()
        },
        context,
        fields: vec![
            (
                "xdf".to_string(),
                string_json(&xdf_path.display().to_string()),
            ),
            (
                "bin".to_string(),
                string_json(&bin_path.display().to_string()),
            ),
            (
                "xdf_exact_sha256".to_string(),
                string_json(&xdf.exact_sha256),
            ),
            (
                "xdf_normalized_fingerprint".to_string(),
                string_json(&xdf.normalized_fingerprint),
            ),
            (
                "category_reference_mode".to_string(),
                string_json(category_reference_mode_name(xdf.category_reference_mode)),
            ),
            ("bin_sha256".to_string(), string_json(&bin_hash)),
            ("bin_size_bytes".to_string(), bin.len().to_string()),
            (
                "parameter_count".to_string(),
                xdf.parameters.len().to_string(),
            ),
            ("valid".to_string(), report.is_valid().to_string()),
            ("issue_count".to_string(), report.issue_count().to_string()),
            ("issues".to_string(), format!("[{issues}]")),
        ],
    })
}

fn parameter_json(parameter: &ParameterDefinition) -> String {
    let unique_id = optional_string_json(parameter.unique_id.as_deref());
    let category = optional_string_json(parameter.category.as_deref());
    let category_path = parameter
        .category_path()
        .iter()
        .map(|component| string_json(component))
        .collect::<Vec<_>>()
        .join(",");
    let raw_type = optional_string_json(parameter.raw_type.as_deref());
    let conversion = optional_string_json(parameter.conversion.as_deref());
    let category_memberships = parameter
        .category_memberships
        .iter()
        .map(|membership| {
            format!(
                "{{\"slot\":{},\"category_index\":{},\"resolved_category_index\":{},\"category_name\":{}}}",
                membership.slot,
                membership.category_index,
                optional_u64_json(membership.resolved_category_index),
                optional_string_json(membership.category_name.as_deref())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let axes = parameter
        .axes
        .iter()
        .map(axis_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"semantic_id\":{},\"unique_id\":{},\"kind\":{},\"title\":{},\"description\":{},\"category\":{},\"category_path\":[{}],\"category_memberships\":[{}],\"raw_type\":{},\"address\":{},\"range\":{{\"start\":{},\"end\":{},\"length\":{}}},\"element_width_bits\":{},\"rows\":{},\"columns\":{},\"row_stride_bits\":{},\"column_stride_bits\":{},\"signed\":{},\"endianness\":{},\"storage\":{},\"conversion\":{},\"bit_offset\":{},\"bit_width\":{},\"bit_mask\":{},\"raw_bit_mask\":{},\"axes\":[{}]}}",
        string_json(&parameter.semantic_id),
        unique_id,
        string_json(parameter.kind.as_str()),
        string_json(&parameter.title),
        string_json(&parameter.description),
        category,
        category_path,
        category_memberships,
        raw_type,
        parameter.layout.address,
        parameter.layout.range.start,
        parameter.layout.range.end,
        parameter.layout.range.len(),
        parameter.layout.element_width_bits,
        parameter.layout.dimensions.rows,
        parameter.layout.dimensions.columns,
        parameter.layout.row_stride_bits,
        parameter.layout.column_stride_bits,
        parameter.layout.signed,
        string_json(endianness_name(parameter.layout.endianness)),
        storage_json(&parameter.layout.storage),
        conversion,
        optional_number_json(parameter.bit_offset),
        optional_number_json(parameter.bit_width),
        optional_u64_json(parameter.bit_mask),
        optional_string_json(parameter.raw_bit_mask.as_deref()),
        axes
    )
}

fn category_reference_mode_name(mode: tuner_xdf::CategoryReferenceMode) -> &'static str {
    match mode {
        tuner_xdf::CategoryReferenceMode::DeclaredIndex => "declared-index",
        tuner_xdf::CategoryReferenceMode::OneBasedPosition => "one-based-position",
    }
}

fn axis_json(axis: &tuner_xdf::AxisDefinition) -> String {
    let labels = axis
        .labels
        .iter()
        .map(|label| {
            format!(
                "{{\"index\":{},\"value\":{}}}",
                label.index,
                string_json(&label.value)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let links = axis
        .links
        .iter()
        .map(|link| {
            format!(
                "{{\"index\":{},\"object_id_hash\":{}}}",
                optional_number_json(link.index),
                optional_string_json(link.object_id_hash.as_deref())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let embed_info = axis
        .embed_info
        .as_ref()
        .map(embed_info_json)
        .unwrap_or_else(|| "null".to_string());
    let range = axis
        .range
        .map(|value| {
            format!(
                "{{\"start\":{},\"end\":{},\"length\":{}}}",
                value.start,
                value.end,
                value.len()
            )
        })
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"id\":{},\"title\":{},\"count\":{},\"address\":{},\"element_width_bits\":{},\"stride_bits\":{},\"signed\":{},\"endianness\":{},\"storage\":{},\"metadata\":{},\"labels\":[{}],\"links\":[{}],\"embed_info\":{},\"conversion\":{},\"range\":{}}}",
        string_json(&axis.id),
        string_json(&axis.title),
        axis.count,
        optional_number_json(axis.address),
        axis.element_width_bits,
        axis.stride_bits,
        axis.signed,
        string_json(endianness_name(axis.endianness)),
        axis
            .storage
            .as_ref()
            .map(storage_json)
            .unwrap_or_else(|| "null".to_string()),
        axis_metadata_json(&axis.metadata),
        labels,
        links,
        embed_info,
        optional_string_json(axis.conversion.as_deref()),
        range
    )
}

fn storage_json(storage: &tuner_xdf::StorageSpec) -> String {
    format!(
        "{{\"element_size_bits\":{},\"signed\":{},\"byte_order\":{},\"numeric_kind\":{},\"column_major\":{},\"raw_type_flags\":{},\"unknown_type_flags\":{}}}",
        storage.element_size_bits,
        storage.signed,
        string_json(endianness_name(storage.byte_order)),
        string_json(storage.numeric_kind.as_str()),
        storage.column_major,
        storage.raw_type_flags,
        storage.unknown_type_flags
    )
}

fn axis_metadata_json(metadata: &tuner_xdf::AxisMetadata) -> String {
    format!(
        "{{\"units\":{},\"unit_type\":{},\"decimal_places\":{},\"min\":{},\"max\":{},\"output_type\":{}}}",
        optional_string_json(metadata.units.as_deref()),
        metadata
            .unit_type
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        metadata
            .decimal_places
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        metadata
            .min
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        metadata
            .max
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        metadata
            .output_type
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string())
    )
}

fn embed_info_json(embed_info: &tuner_xdf::EmbedInfo) -> String {
    let attributes = embed_info
        .attributes
        .iter()
        .map(|(key, value)| format!("{}:{}", string_json(key), string_json(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"type\":{},\"attributes\":{{{}}}}}",
        optional_u64_json(embed_info.type_code),
        attributes
    )
}

fn category_json(category: &tuner_xdf::XdfCategory) -> String {
    format!(
        "{{\"index\":{},\"name\":{}}}",
        category.index,
        string_json(&category.name)
    )
}

fn diagnostic_json(diagnostic: &tuner_xdf::XdfDiagnostic) -> String {
    let severity = match diagnostic.severity {
        tuner_xdf::DiagnosticSeverity::Warning => "warning",
        tuner_xdf::DiagnosticSeverity::Error => "error",
    };
    format!(
        "{{\"severity\":{},\"code\":{},\"path\":{},\"message\":{}}}",
        string_json(severity),
        string_json(&diagnostic.code),
        string_json(&diagnostic.path),
        string_json(&diagnostic.message)
    )
}

fn auxiliary_object_json(object: &tuner_xdf::XdfAuxiliaryObject) -> String {
    let kind = match object.kind {
        tuner_xdf::XdfAuxiliaryKind::Function => "function",
        tuner_xdf::XdfAuxiliaryKind::Patch => "patch",
        tuner_xdf::XdfAuxiliaryKind::Checksum => "checksum",
        tuner_xdf::XdfAuxiliaryKind::Unknown => "unknown",
    };
    let attributes = object
        .attributes
        .iter()
        .map(|(key, value)| format!("{}:{}", string_json(key), string_json(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"kind\":{},\"source_name\":{},\"unique_id\":{},\"title\":{},\"description\":{},\"path\":{},\"attributes\":{{{}}}}}",
        string_json(kind),
        string_json(&object.source_name),
        optional_string_json(object.unique_id.as_deref()),
        string_json(&object.title),
        string_json(&object.description),
        string_json(&object.path),
        attributes
    )
}

fn header_json(header: &tuner_xdf::XdfHeader) -> String {
    let regions = header
        .regions
        .iter()
        .map(|region| {
            format!(
                "{{\"type\":{},\"start_address\":{},\"size\":{},\"region_flags\":{},\"name\":{},\"description\":{}}}",
                optional_u64_json(region.type_code),
                optional_u64_json(region.start_address),
                optional_u64_json(region.size),
                optional_u64_json(region.region_flags),
                optional_string_json(region.name.as_deref()),
                optional_string_json(region.description.as_deref())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let raw_fields = header
        .raw_fields
        .iter()
        .map(|(key, value)| format!("{}:{}", string_json(key), string_json(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"version\":{},\"flags\":{},\"title\":{},\"description\":{},\"author\":{},\"defaults\":{{\"data_size_bits\":{},\"significant_digits\":{},\"output_type\":{},\"signed\":{},\"lsb_first\":{},\"float\":{}}},\"base_offset\":{{\"offset\":{},\"subtract\":{}}},\"regions\":[{}],\"raw_fields\":{{{}}}}}",
        optional_string_json(header.version.as_deref()),
        optional_u64_json(header.flags),
        optional_string_json(header.title.as_deref()),
        optional_string_json(header.description.as_deref()),
        optional_string_json(header.author.as_deref()),
        header
            .defaults
            .data_size_bits
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        header
            .defaults
            .significant_digits
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        header
            .defaults
            .output_type
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        header
            .defaults
            .signed
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        header
            .defaults
            .lsb_first
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        header
            .defaults
            .float
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        header.base_offset.offset,
        header.base_offset.subtract,
        regions,
        raw_fields
    )
}

fn validation_issue_json(issue: &ValidationIssue) -> String {
    let range = issue
        .range
        .map(|value| {
            format!(
                "{{\"start\":{},\"end\":{},\"length\":{}}}",
                value.start,
                value.end,
                value.len()
            )
        })
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"semantic_id\":{},\"title\":{},\"kind\":{},\"range\":{},\"message\":{}}}",
        string_json(&issue.semantic_id),
        string_json(&issue.title),
        string_json(issue.kind.as_str()),
        range,
        string_json(&issue.message)
    )
}

fn optional_string_json(value: Option<&str>) -> String {
    value.map(string_json).unwrap_or_else(|| "null".to_string())
}

fn optional_number_json(value: Option<usize>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn optional_u64_json(value: Option<u64>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn endianness_name(endianness: tuner_core::Endianness) -> &'static str {
    match endianness {
        tuner_core::Endianness::Little => "little",
        tuner_core::Endianness::Big => "big",
    }
}

fn xdf_load_error(operation: &str, path: &Path, error: XdfError) -> ApiError {
    ApiError::new(error.to_string())
        .with("operation", operation)
        .with("path", path.display().to_string())
}

fn plan_edit_command(request: &JsonValue) -> Result<ApiResult, ApiError> {
    let input = required_path(request, "input")?;
    let offset = required_usize(request, "offset")?;
    let value = required_byte(request, "value")?;
    let document = load_document("plan_edit", &input)?;
    let before_hash = sha256_hex(document.bytes());
    let before_value = document
        .read_u8(offset)
        .map_err(|error| core_error("plan_edit", &input, error))?;
    let mut candidate = document.clone();
    let summary = {
        let mut transaction = candidate.transaction("api.plan_edit");
        transaction
            .write_u8(offset, value)
            .map_err(|error| ApiError::new(error.to_string()).with("offset", offset.to_string()))?;
        transaction
            .commit()
            .map_err(|error| ApiError::new(error.to_string()).with("offset", offset.to_string()))?
    };
    let after_hash = sha256_hex(candidate.bytes());
    let plan_id = plan_id_for(&before_hash, offset, value);
    let mut context = LogContext::new();
    context.insert("input", input.display().to_string());
    context.insert("offset", offset.to_string());
    context.insert("before_value", before_value.to_string());
    context.insert("value", value.to_string());
    context.insert("changed_bytes", summary.changed_bytes.to_string());
    context.insert("source_sha256", before_hash.clone());
    context.insert("result_sha256", after_hash.clone());
    context.insert("plan_id", plan_id.clone());
    Ok(ApiResult {
        command: "plan_edit".to_string(),
        message: "byte edit validated as a dry-run plan; no file was written".to_string(),
        context,
        fields: vec![
            (
                "input".to_string(),
                string_json(&input.display().to_string()),
            ),
            ("plan_id".to_string(), string_json(&plan_id)),
            ("offset".to_string(), offset.to_string()),
            ("before_value".to_string(), before_value.to_string()),
            ("value".to_string(), value.to_string()),
            (
                "changed_bytes".to_string(),
                summary.changed_bytes.to_string(),
            ),
            ("source_sha256".to_string(), string_json(&before_hash)),
            ("result_sha256".to_string(), string_json(&after_hash)),
            ("dry_run".to_string(), "true".to_string()),
            (
                "next_action".to_string(),
                string_json(
                    "send apply_edit with this plan_id and source_sha256 as expected_sha256",
                ),
            ),
        ],
    })
}

fn apply_edit_command(request: &JsonValue) -> Result<ApiResult, ApiError> {
    let input = required_path(request, "input")?;
    let output = required_path(request, "output")?;
    let plan_id = required_sha256(request, "plan_id")?;
    let expected_hash = required_sha256(request, "expected_sha256")?;
    let offset = required_usize(request, "offset")?;
    let value = required_byte(request, "value")?;
    let expected_plan_id = plan_id_for(&expected_hash, offset, value);
    if plan_id != expected_plan_id {
        return Err(ApiError::new(
            "plan_id does not match expected_sha256, offset, and value; re-plan before applying",
        )
        .with("plan_id", plan_id)
        .with("expected_plan_id", expected_plan_id)
        .with("offset", offset.to_string())
        .with("value", value.to_string()));
    }

    let mut document = load_document("apply_edit", &input)?;
    let actual_hash = sha256_hex(document.bytes());
    if actual_hash != expected_hash {
        return Err(ApiError::new(
            "source BIN changed after planning; refusing to apply a stale edit",
        )
        .with("input", input.display().to_string())
        .with("expected_sha256", expected_hash)
        .with("actual_sha256", actual_hash));
    }
    let before_value = document
        .read_u8(offset)
        .map_err(|error| core_error("apply_edit", &input, error))?;
    let summary = {
        let mut transaction = document.transaction("api.apply_edit");
        transaction
            .write_u8(offset, value)
            .map_err(|error| ApiError::new(error.to_string()).with("offset", offset.to_string()))?;
        transaction
            .commit()
            .map_err(|error| ApiError::new(error.to_string()).with("offset", offset.to_string()))?
    };
    document.save_as(&output).map_err(|error| {
        ApiError::new(error.to_string())
            .with("input", input.display().to_string())
            .with("output", output.display().to_string())
            .with("changed_bytes", summary.changed_bytes.to_string())
    })?;
    let result_hash = sha256_hex(document.bytes());
    let mut context = LogContext::new();
    context.insert("input", input.display().to_string());
    context.insert("output", output.display().to_string());
    context.insert("plan_id", plan_id.clone());
    context.insert("offset", offset.to_string());
    context.insert("before_value", before_value.to_string());
    context.insert("value", value.to_string());
    context.insert("changed_bytes", summary.changed_bytes.to_string());
    context.insert("source_sha256", expected_hash.clone());
    context.insert("result_sha256", result_hash.clone());
    Ok(ApiResult {
        command: "apply_edit".to_string(),
        message: "exported planned edit to a new BIN".to_string(),
        context,
        fields: vec![
            (
                "input".to_string(),
                string_json(&input.display().to_string()),
            ),
            (
                "output".to_string(),
                string_json(&output.display().to_string()),
            ),
            ("plan_id".to_string(), string_json(&plan_id)),
            ("offset".to_string(), offset.to_string()),
            ("before_value".to_string(), before_value.to_string()),
            ("value".to_string(), value.to_string()),
            (
                "changed_bytes".to_string(),
                summary.changed_bytes.to_string(),
            ),
            ("source_sha256".to_string(), string_json(&expected_hash)),
            ("result_sha256".to_string(), string_json(&result_hash)),
            ("written".to_string(), "true".to_string()),
            ("overwrote_existing".to_string(), "false".to_string()),
        ],
    })
}

fn action_name(request: &JsonValue) -> String {
    match request.get("action") {
        Some(JsonValue::String(value)) if !value.trim().is_empty() => value.clone(),
        _ => "request".to_string(),
    }
}

fn required_path(request: &JsonValue, key: &str) -> Result<PathBuf, ApiError> {
    let value = request_string(request, key)?;
    if value.trim().is_empty() {
        return Err(ApiError::new(format!("{key} cannot be empty")).with(key, value));
    }
    Ok(PathBuf::from(value))
}

fn request_string(request: &JsonValue, key: &str) -> Result<String, ApiError> {
    match request.get(key) {
        Some(JsonValue::String(value)) => Ok(value.clone()),
        Some(value) => Err(ApiError::new(format!("{key} must be a JSON string"))
            .with("field", key)
            .with("received_type", value.type_name())),
        None => Err(ApiError::new(format!("missing required field '{key}'")).with("field", key)),
    }
}

fn required_usize(request: &JsonValue, key: &str) -> Result<usize, ApiError> {
    let value = request.get(key).ok_or_else(|| {
        ApiError::new(format!("missing required field '{key}'")).with("field", key)
    })?;
    match value {
        JsonValue::Number(number) if *number >= 0 => usize::try_from(*number).map_err(|_| {
            ApiError::new(format!("{key} is too large for this platform"))
                .with(key, number.to_string())
        }),
        JsonValue::Number(number) => {
            Err(ApiError::new(format!("{key} must be non-negative")).with(key, number.to_string()))
        }
        JsonValue::String(value) => parse_number(value, key),
        value => Err(
            ApiError::new(format!("{key} must be an integer or numeric string"))
                .with("field", key)
                .with("received_type", value.type_name()),
        ),
    }
}

fn required_byte(request: &JsonValue, key: &str) -> Result<u8, ApiError> {
    let number = required_usize(request, key)?;
    u8::try_from(number).map_err(|_| {
        ApiError::new(format!("{key} must be between 0 and 255")).with(key, number.to_string())
    })
}

fn required_sha256(request: &JsonValue, key: &str) -> Result<String, ApiError> {
    let value = request_string(request, key)?.to_ascii_lowercase();
    if value.len() != 64 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(ApiError::new(format!(
            "{key} must be a 64-character SHA-256 hexadecimal digest"
        ))
        .with(key, value));
    }
    Ok(value)
}

fn parse_number(value: &str, name: &str) -> Result<usize, ApiError> {
    let value = value.trim();
    let (digits, radix) = if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        (hex, 16)
    } else {
        (value, 10)
    };
    if digits.is_empty() {
        return Err(ApiError::new(format!("{name} cannot be empty")).with(name, value));
    }
    usize::from_str_radix(digits, radix).map_err(|_| {
        ApiError::new(format!(
            "{name} must be a non-negative decimal or hexadecimal integer"
        ))
        .with(name, value)
    })
}

fn load_document(operation: &str, path: &Path) -> Result<BinDocument, ApiError> {
    BinDocument::load(path).map_err(|error| core_error(operation, path, error))
}

fn core_error(operation: &str, path: &Path, error: CoreError) -> ApiError {
    ApiError::new(error.to_string())
        .with("operation", operation)
        .with("path", path.display().to_string())
}

fn plan_id_for(source_sha256: &str, offset: usize, value: u8) -> String {
    let canonical = format!("tuner-plan/v1\0{source_sha256}\0{offset}\0{value}");
    sha256_hex(canonical.as_bytes())
}

fn hex_preview(bytes: &[u8], limit: usize) -> String {
    bytes
        .iter()
        .take(limit)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

fn success_response(
    command: &str,
    message: &str,
    request_id: Option<&str>,
    fields: &[(String, String)],
    operation_id: Option<&str>,
) -> String {
    let mut values = vec![
        ("schema".to_string(), string_json(API_RESULT_SCHEMA)),
        ("command".to_string(), string_json(command)),
        ("status".to_string(), string_json("ok")),
        ("message".to_string(), string_json(message)),
    ];
    if let Some(request_id) = request_id {
        values.push(("request_id".to_string(), string_json(request_id)));
    }
    if let Some(operation_id) = operation_id {
        values.push(("operation_id".to_string(), string_json(operation_id)));
    }
    values.extend(fields.iter().cloned());
    json_object(values)
}

fn error_response(
    command: &str,
    request_id: Option<&str>,
    error: &ApiError,
    operation_id: Option<&str>,
) -> String {
    let mut values = vec![
        ("schema".to_string(), string_json(API_RESULT_SCHEMA)),
        ("command".to_string(), string_json(command)),
        ("status".to_string(), string_json("error")),
        (
            "error".to_string(),
            json_object(vec![
                ("message".to_string(), string_json(&error.message)),
                ("context".to_string(), error.context.to_json()),
            ]),
        ),
    ];
    if let Some(request_id) = request_id {
        values.insert(3, ("request_id".to_string(), string_json(request_id)));
    }
    if let Some(operation_id) = operation_id {
        values.push(("operation_id".to_string(), string_json(operation_id)));
    }
    json_object(values)
}

fn string_json(value: &str) -> String {
    format!("\"{}\"", json_escape(value))
}

fn json_object(fields: Vec<(String, String)>) -> String {
    let mut output = String::from("{");
    for (index, (key, value)) in fields.into_iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push('"');
        output.push_str(&json_escape(&key));
        output.push_str("\":");
        output.push_str(&value);
    }
    output.push('}');
    output
}

#[derive(Debug, Clone, PartialEq)]
enum JsonValue {
    Object(BTreeMap<String, JsonValue>),
    Array(Vec<JsonValue>),
    String(String),
    Number(i128),
    Bool(bool),
    Null,
}

impl JsonValue {
    fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            Self::Object(values) => values.get(key),
            _ => None,
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            Self::Object(_) => "object",
            Self::Array(_) => "array",
            Self::String(_) => "string",
            Self::Number(_) => "number",
            Self::Bool(_) => "boolean",
            Self::Null => "null",
        }
    }
}

fn parse_request(input: &str) -> Result<JsonValue, String> {
    let mut parser = JsonParser {
        input: input.chars().collect(),
        position: 0,
    };
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.position != parser.input.len() {
        return Err("unexpected characters after the JSON value".to_string());
    }
    if !matches!(value, JsonValue::Object(_)) {
        return Err("request root must be a JSON object".to_string());
    }
    Ok(value)
}

struct JsonParser {
    input: Vec<char>,
    position: usize,
}

impl JsonParser {
    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_whitespace();
        match self.current() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => self.parse_string().map(JsonValue::String),
            Some('t') => {
                self.parse_literal("true")?;
                Ok(JsonValue::Bool(true))
            }
            Some('f') => {
                self.parse_literal("false")?;
                Ok(JsonValue::Bool(false))
            }
            Some('n') => {
                self.parse_literal("null")?;
                Ok(JsonValue::Null)
            }
            Some('-') | Some('0'..='9') => self.parse_number().map(JsonValue::Number),
            Some(character) => Err(format!("unexpected character '{character}'")),
            None => Err("request is empty".to_string()),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.expect('{')?;
        let mut values = BTreeMap::new();
        self.skip_whitespace();
        if self.consume('}') {
            return Ok(JsonValue::Object(values));
        }
        loop {
            self.skip_whitespace();
            let key = match self.current() {
                Some('"') => self.parse_string()?,
                _ => return Err("object keys must be JSON strings".to_string()),
            };
            self.skip_whitespace();
            self.expect(':')?;
            let value = self.parse_value()?;
            if values.insert(key.clone(), value).is_some() {
                return Err(format!("duplicate object key '{key}'"));
            }
            self.skip_whitespace();
            if self.consume('}') {
                break;
            }
            self.expect(',')?;
        }
        Ok(JsonValue::Object(values))
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.expect('[')?;
        let mut values = Vec::new();
        self.skip_whitespace();
        if self.consume(']') {
            return Ok(JsonValue::Array(values));
        }
        loop {
            values.push(self.parse_value()?);
            self.skip_whitespace();
            if self.consume(']') {
                break;
            }
            self.expect(',')?;
        }
        Ok(JsonValue::Array(values))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut value = String::new();
        loop {
            match self.next() {
                Some('"') => return Ok(value),
                Some('\\') => match self.next() {
                    Some('"') => value.push('"'),
                    Some('\\') => value.push('\\'),
                    Some('/') => value.push('/'),
                    Some('b') => value.push('\u{0008}'),
                    Some('f') => value.push('\u{000c}'),
                    Some('n') => value.push('\n'),
                    Some('r') => value.push('\r'),
                    Some('t') => value.push('\t'),
                    Some('u') => value.push(self.parse_unicode_escape()?),
                    Some(character) => {
                        return Err(format!("invalid JSON string escape '\\{character}'"))
                    }
                    None => return Err("unterminated JSON string escape".to_string()),
                },
                Some(character) if character.is_control() => {
                    return Err("unescaped control character in JSON string".to_string())
                }
                Some(character) => value.push(character),
                None => return Err("unterminated JSON string".to_string()),
            }
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, String> {
        let mut code = 0u32;
        for _ in 0..4 {
            let character = self
                .next()
                .ok_or_else(|| "incomplete unicode escape in JSON string".to_string())?;
            let digit = character
                .to_digit(16)
                .ok_or_else(|| format!("invalid unicode escape digit '{character}'"))?;
            code = code * 16 + digit;
        }
        char::from_u32(code).ok_or_else(|| "invalid unicode code point in JSON string".to_string())
    }

    fn parse_number(&mut self) -> Result<i128, String> {
        let start = self.position;
        self.consume('-');
        match self.current() {
            Some('0') => {
                self.position += 1;
                if matches!(self.current(), Some('0'..='9')) {
                    return Err("leading zeros are not allowed in JSON numbers".to_string());
                }
            }
            Some('1'..='9') => {
                self.position += 1;
                while matches!(self.current(), Some('0'..='9')) {
                    self.position += 1;
                }
            }
            _ => return Err("invalid JSON number".to_string()),
        }
        if matches!(self.current(), Some('.') | Some('e') | Some('E')) {
            return Err("only integer JSON numbers are supported by this protocol".to_string());
        }
        let number: String = self.input[start..self.position].iter().collect();
        number
            .parse::<i128>()
            .map_err(|_| "JSON number is outside the supported range".to_string())
    }

    fn parse_literal(&mut self, literal: &str) -> Result<(), String> {
        for expected in literal.chars() {
            if self.next() != Some(expected) {
                return Err(format!("invalid JSON literal; expected {literal}"));
            }
        }
        Ok(())
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.current(), Some(' ' | '\n' | '\r' | '\t')) {
            self.position += 1;
        }
    }

    fn expect(&mut self, expected: char) -> Result<(), String> {
        if self.next() == Some(expected) {
            Ok(())
        } else {
            Err(format!("expected '{expected}' in JSON request"))
        }
    }

    fn consume(&mut self, expected: char) -> bool {
        if self.current() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_protocol_request_with_numeric_string_values() {
        let request = parse_request(
            "{\"action\":\"plan_edit\",\"input\":\"input.bin\",\"offset\":\"0x10\",\"value\":127}",
        )
        .unwrap();
        assert_eq!(request_string(&request, "action").unwrap(), "plan_edit");
        assert_eq!(required_usize(&request, "offset").unwrap(), 16);
        assert_eq!(required_byte(&request, "value").unwrap(), 127);
        assert_eq!(
            required_path(&request, "input").unwrap(),
            PathBuf::from("input.bin")
        );
    }

    #[test]
    fn rejects_duplicate_keys_and_non_object_roots() {
        assert!(parse_request("{\"action\":\"inspect\",\"action\":\"compare\"}").is_err());
        assert!(parse_request("[1,2,3]").is_err());
    }

    #[test]
    fn plan_ids_change_when_any_edit_input_changes() {
        let source = "a".repeat(64);
        assert_ne!(plan_id_for(&source, 1, 2), plan_id_for(&source, 1, 3));
        assert_ne!(plan_id_for(&source, 1, 2), plan_id_for(&source, 2, 2));
    }

    #[test]
    fn error_response_is_json_object_with_context() {
        let error = ApiError::new("bad request").with("field", "input");
        let response = error_response("inspect", Some("req-1"), &error, Some("op-1"));
        assert!(response.contains("\"status\":\"error\""));
        assert!(response.contains("\"request_id\":\"req-1\""));
        assert!(response.contains("\"context\":{\"field\":\"input\"}"));
    }

    #[test]
    fn transfer_request_normalizes_selected_ids_to_a_set() {
        let request = parse_request(
            "{\"source_bin\":\"source.bin\",\"destination_bin\":\"destination.bin\",\"source_xdf\":\"source.xdf\",\"destination_xdf\":\"destination.xdf\",\"selected\":[\"constant:uid:rpm\",\"constant:uid:rpm\",\"table:uid:load\"]}",
        )
        .unwrap();

        let arguments = transfer_request_arguments(&request).unwrap();
        let expected = ["constant:uid:rpm", "table:uid:load"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        assert_eq!(arguments.selected, Some(expected));
    }

    #[test]
    fn transfer_range_json_includes_half_open_length() {
        assert_eq!(
            range_json(ByteRange { start: 4, end: 9 }),
            "{\"start\":4,\"end\":9,\"length\":5}"
        );
    }

    #[test]
    fn inspect_serializers_include_normalized_metadata() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT version="1.70">
                  <XDFHEADER>
                    <DEFAULTS datasizeinbits="16" />
                    <CATEGORY index="2" name="Fuel" />
                  </XDFHEADER>
                  <XDFCONSTANT uniqueid="rpm">
                    <CATEGORYMEM index="0" category="2" />
                    <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="16" mmedtypeflags="0x06" />
                  </XDFCONSTANT>
                </XDFFORMAT>
            "#,
        )
        .unwrap();
        let parameter = parameter_json(&document.parameters[0]);
        assert!(parameter.contains("\"numeric_kind\":\"integer\""));
        assert!(parameter.contains("\"raw_type_flags\":6"));
        assert!(parameter.contains("\"column_major\":true"));
        assert!(parameter.contains("\"category_memberships\""));
        let document_json = json_object(inspection_fields(&document, Path::new("fixture.xdf")));
        assert!(document_json.contains("\"header\""));
        assert!(document_json.contains("\"category_count\":1"));
        assert!(document_json.contains("\"diagnostic_count\":0"));
    }

    #[test]
    fn axis_serializer_includes_signed_stride_bits() {
        let document = XdfDocument::parse(
            br#"
                <XDFFORMAT>
                  <XDFTABLE uniqueid="axis-map">
                    <title>Axis map</title>
                    <XDFAXIS id="x">
                      <indexcount>2</indexcount>
                      <EMBEDDEDDATA mmedaddress="0x10" mmedelementsizebits="8"
                        mmedmajorstridebits="8" mmedtypeflags="0x06" />
                    </XDFAXIS>
                    <XDFAXIS id="y">
                      <indexcount>2</indexcount>
                      <EMBEDDEDDATA mmedaddress="0x20" mmedelementsizebits="16"
                        mmedmajorstridebits="-16" mmedtypeflags="0x06" />
                    </XDFAXIS>
                    <XDFAXIS id="z">
                      <EMBEDDEDDATA mmedaddress="0x30" mmedelementsizebits="8"
                        mmedrowcount="1" mmedcolcount="1" mmedtypeflags="0x06" />
                    </XDFAXIS>
                  </XDFTABLE>
                </XDFFORMAT>
            "#,
        )
        .unwrap();

        let parameter = parameter_json(&document.parameters[0]);
        assert!(parameter.contains("\"stride_bits\":8"));
        assert!(parameter.contains("\"stride_bits\":-16"));
    }
}
