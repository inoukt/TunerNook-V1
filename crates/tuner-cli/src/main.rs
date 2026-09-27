use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::process;

use tuner_core::{
    compare_bytes, json_escape, sha256_hex, BinDocument, CoreError, DiagnosticLogger, LogContext,
};
use tuner_transfer::{
    ParameterTransfer, TransferError, TransferIssue, TransferOptions, TransferPlan,
};
use tuner_xdf::{ParameterDefinition, ValidationIssue, XdfDocument, XdfError};

const RESULT_SCHEMA: &str = "tuner-result/v1";

#[derive(Debug)]
struct CommandError {
    message: String,
    context: LogContext,
}

impl CommandError {
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

struct CommandResult {
    json: String,
    message: String,
    context: LogContext,
}

fn main() {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    let (args, log_path) = match extract_global_options(raw_args) {
        Ok(parsed) => parsed,
        Err(error) => {
            println!("{}", failure_json("cli.parse", &error.message));
            process::exit(2);
        }
    };
    let operation = args
        .first()
        .map(|command| format!("cli.{command}"))
        .unwrap_or_else(|| "cli.help".to_string());
    let mut logger = match DiagnosticLogger::new("tuner-cli", &operation, log_path.as_deref()) {
        Ok(logger) => logger,
        Err(error) => {
            println!("{}", failure_json(&operation, &error.to_string()));
            process::exit(2);
        }
    };
    let invocation = args.join(" ");
    let mut start_context = LogContext::new();
    start_context.insert("invocation", invocation);
    start_context.insert("pid", process::id().to_string());
    if let Ok(cwd) = env::current_dir() {
        start_context.insert("cwd", cwd.display().to_string());
    }
    if let Some(path) = &log_path {
        start_context.insert("log_file", path.display().to_string());
    }
    let started = match logger.started("command started", &start_context) {
        Ok(started) => started,
        Err(error) => {
            println!("{}", failure_json(&operation, &error.to_string()));
            process::exit(2);
        }
    };

    match execute(&args) {
        Ok(result) => {
            let mut complete_context = result.context.clone();
            complete_context.insert("status", "ok");
            let _ = logger.complete(&result.message, &complete_context, started.elapsed());
            println!(
                "{}",
                append_json_string_field(&result.json, "operation_id", logger.operation_id())
            );
        }
        Err(error) => {
            let mut aborted_context = error.context.clone();
            aborted_context.insert("status", "error");
            let _ = logger.aborted(&error.message, &aborted_context, started.elapsed());
            println!(
                "{}",
                append_json_string_field(
                    &failure_json_with_context(&operation, &error),
                    "operation_id",
                    logger.operation_id()
                )
            );
            process::exit(1);
        }
    }
}

fn extract_global_options(
    raw_args: Vec<String>,
) -> Result<(Vec<String>, Option<PathBuf>), CommandError> {
    let mut args = Vec::with_capacity(raw_args.len());
    let mut log_path = None;
    let mut index = 0;
    while index < raw_args.len() {
        match raw_args[index].as_str() {
            "--log-file" => {
                let value = raw_args.get(index + 1).ok_or_else(|| {
                    CommandError::new("--log-file requires a path").with("argument", "--log-file")
                })?;
                if value.starts_with('-') {
                    return Err(CommandError::new("--log-file requires a path")
                        .with("argument", "--log-file"));
                }
                if log_path.replace(PathBuf::from(value)).is_some() {
                    return Err(CommandError::new("--log-file may only be supplied once")
                        .with("argument", "--log-file"));
                }
                index += 2;
            }
            value => {
                args.push(value.to_string());
                index += 1;
            }
        }
    }
    Ok((args, log_path))
}

fn execute(args: &[String]) -> Result<CommandResult, CommandError> {
    match args.first().map(String::as_str) {
        None | Some("help") | Some("--help") | Some("-h") => Ok(help_result()),
        Some("inspect") => inspect_command(&args[1..]),
        Some("compare") => compare_command(&args[1..]),
        Some("inspect-xdf") => inspect_xdf_command(&args[1..]),
        Some("xdf-validate") => xdf_validate_command(&args[1..]),
        Some("transfer-plan") => transfer_plan_command(&args[1..]),
        Some("transfer-apply") => transfer_apply_command(&args[1..]),
        Some("plan-byte") => plan_byte_command(&args[1..]),
        Some("edit-byte") => edit_byte_command(&args[1..]),
        Some(command) => Err(CommandError::new(format!(
            "unknown command '{command}'; run 'tuner-cli help' for usage"
        ))
        .with("command", command)),
    }
}

fn help_result() -> CommandResult {
    let help = "TunerNook agent CLI\n\nCommands:\n  inspect <input.bin>\n  compare <left.bin> <right.bin>\n  inspect-xdf <definition.xdf>\n  xdf-validate <definition.xdf> <input.bin>\n  transfer-plan <source.bin> <destination.bin> --source-xdf <source.xdf> --destination-xdf <destination.xdf> [--select <semantic-id>] [--approve-address <semantic-id>]\n  transfer-apply <source.bin> <destination.bin> --source-xdf <source.xdf> --destination-xdf <destination.xdf> --plan-id <id> --output <new.bin> [--select <semantic-id>] [--approve-address <semantic-id>]\n  plan-byte <input.bin> --offset <n> --value <0..255>\n  edit-byte <input.bin> --offset <n> --value <0..255> --output <new.bin>\n\nGlobal options:\n  --log-file <run.jsonl>    append self-contained JSONL diagnostics\n\nResults are JSON on stdout. Diagnostics are JSON Lines on stderr and, when requested, the log file.\n";
    let mut context = LogContext::new();
    context.insert("command", "help");
    CommandResult {
        json: format!(
            "{{\"schema\":\"{RESULT_SCHEMA}\",\"command\":\"help\",\"status\":\"ok\",\"usage\":\"{}\"}}",
            json_escape(help)
        ),
        message: "help displayed".to_string(),
        context,
    }
}

fn inspect_command(args: &[String]) -> Result<CommandResult, CommandError> {
    if args.len() != 1 {
        return Err(
            CommandError::new("inspect requires exactly one input BIN path")
                .with("argument_count", args.len().to_string()),
        );
    }
    let input = Path::new(&args[0]);
    let document = BinDocument::load(input).map_err(|error| load_error("inspect", input, error))?;
    let digest = sha256_hex(document.bytes());
    let head_hex = hex_preview(document.bytes(), 32);
    let mut context = LogContext::new();
    context.insert("input", input.display().to_string());
    context.insert("size_bytes", document.len().to_string());
    context.insert("sha256", digest.clone());
    context.insert("changed_bytes", "0");
    let json = format!(
        "{{\"schema\":\"{RESULT_SCHEMA}\",\"command\":\"inspect\",\"status\":\"ok\",\"input\":\"{}\",\"size_bytes\":{},\"sha256\":\"{}\",\"dirty\":false,\"head_hex\":\"{}\"}}",
        json_escape(&input.display().to_string()),
        document.len(),
        digest,
        head_hex
    );
    Ok(CommandResult {
        json,
        message: "BIN inspected".to_string(),
        context,
    })
}

fn compare_command(args: &[String]) -> Result<CommandResult, CommandError> {
    if args.len() != 2 {
        return Err(
            CommandError::new("compare requires exactly two BIN paths: left then right")
                .with("argument_count", args.len().to_string()),
        );
    }
    let left_path = Path::new(&args[0]);
    let right_path = Path::new(&args[1]);
    let left =
        BinDocument::load(left_path).map_err(|error| load_error("compare", left_path, error))?;
    let right =
        BinDocument::load(right_path).map_err(|error| load_error("compare", right_path, error))?;
    let comparison = compare_bytes(left.bytes(), right.bytes());
    let ranges = comparison
        .ranges
        .iter()
        .map(|range| format!("{{\"start\":{},\"end\":{}}}", range.start, range.end))
        .collect::<Vec<_>>()
        .join(",");
    let left_hash = sha256_hex(left.bytes());
    let right_hash = sha256_hex(right.bytes());
    let mut context = LogContext::new();
    context.insert("left", left_path.display().to_string());
    context.insert("right", right_path.display().to_string());
    context.insert("left_size_bytes", left.len().to_string());
    context.insert("right_size_bytes", right.len().to_string());
    context.insert("changed_bytes", comparison.changed_bytes.to_string());
    context.insert("changed_ranges", comparison.ranges.len().to_string());
    context.insert("identical", comparison.identical().to_string());
    let json = format!(
        "{{\"schema\":\"{RESULT_SCHEMA}\",\"command\":\"compare\",\"status\":\"ok\",\"left\":\"{}\",\"right\":\"{}\",\"left_size_bytes\":{},\"right_size_bytes\":{},\"left_sha256\":\"{}\",\"right_sha256\":\"{}\",\"identical\":{},\"changed_bytes\":{},\"changed_ranges\":[{}]}}",
        json_escape(&left_path.display().to_string()),
        json_escape(&right_path.display().to_string()),
        left.len(),
        right.len(),
        left_hash,
        right_hash,
        comparison.identical(),
        comparison.changed_bytes,
        ranges
    );
    Ok(CommandResult {
        json,
        message: if comparison.identical() {
            "BINs are identical"
        } else {
            "BIN comparison completed"
        }
        .to_string(),
        context,
    })
}

fn inspect_xdf_command(args: &[String]) -> Result<CommandResult, CommandError> {
    if args.len() != 1 {
        return Err(
            CommandError::new("inspect-xdf requires exactly one XDF path")
                .with("argument_count", args.len().to_string()),
        );
    }
    let path = Path::new(&args[0]);
    let document =
        XdfDocument::load(path).map_err(|error| xdf_load_error("inspect-xdf", path, error))?;
    let mut context = LogContext::new();
    context.insert("xdf", path.display().to_string());
    context.insert("exact_sha256", document.exact_sha256.clone());
    context.insert(
        "normalized_fingerprint",
        document.normalized_fingerprint.clone(),
    );
    context.insert("parameter_count", document.parameters.len().to_string());
    context.insert(
        "unknown_element_count",
        document.unknown_element_count.to_string(),
    );
    let json = inspection_json(&document, path);
    Ok(CommandResult {
        json,
        message: "XDF inspected".to_string(),
        context,
    })
}

fn inspection_json(document: &XdfDocument, path: &Path) -> String {
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
    format!(
        "{{\"schema\":\"{RESULT_SCHEMA}\",\"command\":\"inspect-xdf\",\"status\":\"ok\",\"xdf\":{},\"exact_sha256\":\"{}\",\"normalized_fingerprint\":\"{}\",\"parameter_count\":{},\"unknown_element_count\":{},\"category_count\":{},\"category_reference_mode\":{},\"diagnostic_count\":{},\"auxiliary_object_count\":{},\"header\":{},\"categories\":[{}],\"diagnostics\":[{}],\"auxiliary_objects\":[{}],\"parameters\":[{}]}}",
        string_json(&path.display().to_string()),
        document.exact_sha256,
        document.normalized_fingerprint,
        document.parameters.len(),
        document.unknown_element_count,
        document.categories.len(),
        string_json(category_reference_mode_name(document.category_reference_mode)),
        document.diagnostics.len(),
        document.auxiliary_objects.len(),
        header_json(&document.header),
        categories,
        diagnostics,
        auxiliary_objects,
        parameters
    )
}

fn xdf_validate_command(args: &[String]) -> Result<CommandResult, CommandError> {
    if args.len() != 2 {
        return Err(
            CommandError::new("xdf-validate requires an XDF path and a BIN path")
                .with("argument_count", args.len().to_string()),
        );
    }
    let xdf_path = Path::new(&args[0]);
    let bin_path = Path::new(&args[1]);
    let xdf = XdfDocument::load(xdf_path)
        .map_err(|error| xdf_load_error("xdf-validate", xdf_path, error))?;
    let bin =
        BinDocument::load(bin_path).map_err(|error| load_error("xdf-validate", bin_path, error))?;
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
    context.insert(
        "category_reference_mode",
        category_reference_mode_name(xdf.category_reference_mode).to_string(),
    );
    context.insert("issue_count", report.issue_count().to_string());
    context.insert("valid", report.is_valid().to_string());
    let json = format!(
        "{{\"schema\":\"{RESULT_SCHEMA}\",\"command\":\"xdf-validate\",\"status\":\"ok\",\"xdf\":\"{}\",\"bin\":\"{}\",\"xdf_exact_sha256\":\"{}\",\"xdf_normalized_fingerprint\":\"{}\",\"category_reference_mode\":{},\"bin_sha256\":\"{}\",\"bin_size_bytes\":{},\"parameter_count\":{},\"valid\":{},\"issue_count\":{},\"issues\":[{}]}}",
        json_escape(&xdf_path.display().to_string()),
        json_escape(&bin_path.display().to_string()),
        xdf.exact_sha256,
        xdf.normalized_fingerprint,
        string_json(category_reference_mode_name(xdf.category_reference_mode)),
        bin_hash,
        bin.len(),
        xdf.parameters.len(),
        report.is_valid(),
        report.issue_count(),
        issues
    );
    Ok(CommandResult {
        json,
        message: if report.is_valid() {
            "XDF mappings are valid against BIN".to_string()
        } else {
            "XDF validation found blocking mapping issues".to_string()
        },
        context,
    })
}

fn parameter_json(parameter: &ParameterDefinition) -> String {
    let unique_id = optional_string(parameter.unique_id.as_deref());
    let category = optional_string(parameter.category.as_deref());
    let category_path = parameter
        .category_path()
        .iter()
        .map(|component| string_json(component))
        .collect::<Vec<_>>()
        .join(",");
    let raw_type = optional_string(parameter.raw_type.as_deref());
    let conversion = optional_string(parameter.conversion.as_deref());
    let category_memberships = parameter
        .category_memberships
        .iter()
        .map(|membership| {
            format!(
                "{{\"slot\":{},\"category_index\":{},\"resolved_category_index\":{},\"category_name\":{}}}",
                membership.slot,
                membership.category_index,
                optional_u64(membership.resolved_category_index),
                optional_string(membership.category_name.as_deref())
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
        optional_number(parameter.bit_offset),
        optional_number(parameter.bit_width),
        optional_u64(parameter.bit_mask),
        optional_string(parameter.raw_bit_mask.as_deref()),
        axes
    )
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
                optional_number(link.index),
                optional_string(link.object_id_hash.as_deref())
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
        optional_number(axis.address),
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
        optional_string(axis.conversion.as_deref()),
        range
    )
}

fn optional_string(value: Option<&str>) -> String {
    value.map(string_json).unwrap_or_else(|| "null".to_string())
}

fn optional_number(value: Option<usize>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn optional_u64(value: Option<u64>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn optional_f64(value: Option<f64>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "null".to_string())
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
        optional_string(metadata.units.as_deref()),
        metadata
            .unit_type
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        metadata
            .decimal_places
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        optional_f64(metadata.min),
        optional_f64(metadata.max),
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
        optional_u64(embed_info.type_code),
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

fn category_reference_mode_name(mode: tuner_xdf::CategoryReferenceMode) -> &'static str {
    match mode {
        tuner_xdf::CategoryReferenceMode::DeclaredIndex => "declared-index",
        tuner_xdf::CategoryReferenceMode::OneBasedPosition => "one-based-position",
    }
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
        optional_string(object.unique_id.as_deref()),
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
                optional_u64(region.type_code),
                optional_u64(region.start_address),
                optional_u64(region.size),
                optional_u64(region.region_flags),
                optional_string(region.name.as_deref()),
                optional_string(region.description.as_deref())
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
        optional_string(header.version.as_deref()),
        optional_u64(header.flags),
        optional_string(header.title.as_deref()),
        optional_string(header.description.as_deref()),
        optional_string(header.author.as_deref()),
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

fn xdf_load_error(operation: &str, path: &Path, error: XdfError) -> CommandError {
    CommandError::new(error.to_string())
        .with("operation", operation)
        .with("path", path.display().to_string())
}
fn transfer_plan_command(args: &[String]) -> Result<CommandResult, CommandError> {
    let arguments = parse_transfer_arguments(args, false)?;
    let source_xdf = XdfDocument::load(&arguments.source_xdf)
        .map_err(|error| xdf_load_error("transfer-plan", &arguments.source_xdf, error))?;
    let destination_xdf = XdfDocument::load(&arguments.destination_xdf)
        .map_err(|error| xdf_load_error("transfer-plan", &arguments.destination_xdf, error))?;
    let source_bin = BinDocument::load(&arguments.source_bin)
        .map_err(|error| load_error("transfer-plan", &arguments.source_bin, error))?;
    let destination_bin = BinDocument::load(&arguments.destination_bin)
        .map_err(|error| load_error("transfer-plan", &arguments.destination_bin, error))?;
    let options = TransferOptions {
        selected: arguments.selected,
        approved_address_reviews: arguments.approved_address_reviews,
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
    context.insert("plan_status", plan.status());
    context.insert("selected_count", plan.selected_count.to_string());
    context.insert("changed_bytes", plan.changed_bytes.to_string());
    context.insert("issue_count", plan.issues.len().to_string());
    Ok(CommandResult {
        json: transfer_plan_json(&plan),
        message: transfer_plan_message(&plan),
        context,
    })
}

fn transfer_apply_command(args: &[String]) -> Result<CommandResult, CommandError> {
    let arguments = parse_transfer_arguments(args, true)?;
    let supplied_plan_id = arguments
        .plan_id
        .clone()
        .ok_or_else(|| CommandError::new("transfer-apply is missing --plan-id"))?;
    let output = arguments
        .output
        .clone()
        .ok_or_else(|| CommandError::new("transfer-apply is missing --output"))?;
    let source_xdf = XdfDocument::load(&arguments.source_xdf)
        .map_err(|error| xdf_load_error("transfer-apply", &arguments.source_xdf, error))?;
    let destination_xdf = XdfDocument::load(&arguments.destination_xdf)
        .map_err(|error| xdf_load_error("transfer-apply", &arguments.destination_xdf, error))?;
    let source_bin = BinDocument::load(&arguments.source_bin)
        .map_err(|error| load_error("transfer-apply", &arguments.source_bin, error))?;
    let mut destination_bin = BinDocument::load(&arguments.destination_bin)
        .map_err(|error| load_error("transfer-apply", &arguments.destination_bin, error))?;
    let options = TransferOptions {
        selected: arguments.selected,
        approved_address_reviews: arguments.approved_address_reviews,
    };
    let plan = TransferPlan::build(
        &source_xdf,
        &destination_xdf,
        &source_bin,
        &destination_bin,
        &options,
    );
    if plan.plan_id != supplied_plan_id {
        return Err(CommandError::new(
            "transfer plan ID does not match the current files and approvals; re-run transfer-plan",
        )
        .with("supplied_plan_id", supplied_plan_id)
        .with("current_plan_id", plan.plan_id));
    }
    let applied = plan
        .apply(&source_bin, &mut destination_bin)
        .map_err(|error| transfer_apply_error("transfer-apply", error))?;
    destination_bin.save_as(&output).map_err(|error| {
        CommandError::new(error.to_string())
            .with("operation", "transfer-apply")
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
    Ok(CommandResult {
        json: format!(
            "{{\"schema\":\"{RESULT_SCHEMA}\",\"command\":\"transfer-apply\",\"status\":\"ok\",\"plan_id\":\"{}\",\"output\":\"{}\",\"changed_bytes\":{},\"result_sha256\":\"{}\",\"written\":true,\"overwrote_existing\":false}}",
            json_escape(&plan.plan_id),
            json_escape(&output.display().to_string()),
            applied.changed_bytes,
            applied.result_sha256
        ),
        message: "validated transfer applied and exported".to_string(),
        context,
    })
}

struct TransferCommandArguments {
    source_bin: PathBuf,
    destination_bin: PathBuf,
    source_xdf: PathBuf,
    destination_xdf: PathBuf,
    selected: Option<BTreeSet<String>>,
    approved_address_reviews: BTreeSet<String>,
    plan_id: Option<String>,
    output: Option<PathBuf>,
}

fn parse_transfer_arguments(
    args: &[String],
    applying: bool,
) -> Result<TransferCommandArguments, CommandError> {
    if args.len() < 2 {
        return Err(CommandError::new(
            "transfer commands require source and destination BIN paths",
        ));
    }
    let source_bin = PathBuf::from(&args[0]);
    let destination_bin = PathBuf::from(&args[1]);
    let mut source_xdf = None;
    let mut destination_xdf = None;
    let mut selected = None;
    let mut approved_address_reviews = BTreeSet::new();
    let mut plan_id = None;
    let mut output = None;
    let mut index = 2;
    while index < args.len() {
        let option = args[index].as_str();
        let value = args.get(index + 1).ok_or_else(|| {
            CommandError::new(format!("{option} requires a value")).with("option", option)
        })?;
        if value.starts_with('-') {
            return Err(
                CommandError::new(format!("{option} requires a value")).with("option", option)
            );
        }
        match option {
            "--source-xdf" => {
                if source_xdf.replace(PathBuf::from(value)).is_some() {
                    return Err(CommandError::new("--source-xdf may only be supplied once"));
                }
            }
            "--destination-xdf" => {
                if destination_xdf.replace(PathBuf::from(value)).is_some() {
                    return Err(CommandError::new(
                        "--destination-xdf may only be supplied once",
                    ));
                }
            }
            "--select" => {
                selected
                    .get_or_insert_with(BTreeSet::new)
                    .insert(value.to_string());
            }
            "--approve-address" => {
                approved_address_reviews.insert(value.to_string());
            }
            "--plan-id" if applying => {
                if plan_id.replace(parse_digest(value, "plan-id")?).is_some() {
                    return Err(CommandError::new("--plan-id may only be supplied once"));
                }
            }
            "--output" if applying => {
                if output.replace(PathBuf::from(value)).is_some() {
                    return Err(CommandError::new("--output may only be supplied once"));
                }
            }
            _ => {
                return Err(
                    CommandError::new(format!("unknown transfer option '{option}'"))
                        .with("option", option),
                );
            }
        }
        index += 2;
    }
    let source_xdf =
        source_xdf.ok_or_else(|| CommandError::new("transfer command is missing --source-xdf"))?;
    let destination_xdf = destination_xdf
        .ok_or_else(|| CommandError::new("transfer command is missing --destination-xdf"))?;
    if applying && plan_id.is_none() {
        return Err(CommandError::new("transfer-apply is missing --plan-id"));
    }
    if applying && output.is_none() {
        return Err(CommandError::new("transfer-apply is missing --output"));
    }
    Ok(TransferCommandArguments {
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

fn parse_digest(value: &str, name: &str) -> Result<String, CommandError> {
    if value.len() != 64 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(CommandError::new(format!(
            "{name} must be a 64-character SHA-256 hexadecimal digest"
        ))
        .with(name, value));
    }
    Ok(value.to_ascii_lowercase())
}

fn transfer_apply_error(operation: &str, error: TransferError) -> CommandError {
    let mut command_error = CommandError::new(error.to_string()).with("operation", operation);
    match &error {
        TransferError::PlanBlocked { plan_id, .. } => {
            command_error = command_error.with("plan_id", plan_id.clone());
        }
        TransferError::SourceChanged { expected, actual } => {
            command_error = command_error
                .with("expected_source_sha256", expected.clone())
                .with("actual_source_sha256", actual.clone());
        }
        TransferError::DestinationChanged { expected, actual } => {
            command_error = command_error
                .with("expected_destination_sha256", expected.clone())
                .with("actual_destination_sha256", actual.clone());
        }
        TransferError::Core { .. } => {}
    }
    command_error
}

fn transfer_plan_message(plan: &TransferPlan) -> String {
    match plan.status() {
        "blocked" => "transfer dry-run is blocked".to_string(),
        "review_required" => "transfer dry-run requires explicit address review".to_string(),
        _ => "transfer dry-run is ready".to_string(),
    }
}

fn transfer_plan_json(plan: &TransferPlan) -> String {
    let issues = plan
        .issues
        .iter()
        .map(transfer_issue_json)
        .collect::<Vec<_>>()
        .join(",");
    let entries = plan
        .entries
        .iter()
        .map(parameter_transfer_json)
        .collect::<Vec<_>>()
        .join(",");
    let ranges = plan
        .changed_ranges
        .iter()
        .map(|range| range_json(*range))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":\"{RESULT_SCHEMA}\",\"command\":\"transfer-plan\",\"status\":\"ok\",\"plan_status\":\"{}\",\"plan_id\":\"{}\",\"source_xdf_sha256\":\"{}\",\"destination_xdf_sha256\":\"{}\",\"source_xdf_fingerprint\":\"{}\",\"destination_xdf_fingerprint\":\"{}\",\"source_bin_sha256\":\"{}\",\"destination_bin_sha256\":\"{}\",\"xdf_hash_match\":{},\"selected_count\":{},\"changed_bytes\":{},\"requires_review\":{},\"ready\":{},\"issues\":[{}],\"changed_ranges\":[{}],\"entries\":[{}],\"dry_run\":true}}",
        plan.status(),
        json_escape(&plan.plan_id),
        json_escape(&plan.source_xdf_sha256),
        json_escape(&plan.destination_xdf_sha256),
        json_escape(&plan.source_xdf_fingerprint),
        json_escape(&plan.destination_xdf_fingerprint),
        json_escape(&plan.source_bin_sha256),
        json_escape(&plan.destination_bin_sha256),
        plan.xdf_hash_match,
        plan.selected_count,
        plan.changed_bytes,
        plan.requires_review(),
        plan.is_ready(),
        issues,
        ranges,
        entries
    )
}

fn parameter_transfer_json(entry: &ParameterTransfer) -> String {
    let source_range = entry
        .source_range
        .map(range_json)
        .unwrap_or_else(|| "null".to_string());
    let destination_range = entry
        .destination_range
        .map(range_json)
        .unwrap_or_else(|| "null".to_string());
    let source_address = optional_number(entry.source_address);
    let destination_address = optional_number(entry.destination_address);
    let issues = entry
        .issues
        .iter()
        .map(transfer_issue_json)
        .collect::<Vec<_>>()
        .join(",");
    let ranges = entry
        .changed_ranges
        .iter()
        .map(|range| range_json(*range))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"semantic_id\":\"{}\",\"title\":\"{}\",\"kind\":\"{}\",\"source_range\":{},\"destination_range\":{},\"source_address\":{},\"destination_address\":{},\"source_size_bytes\":{},\"destination_size_bytes\":{},\"changed_bytes\":{},\"changed_ranges\":[{}],\"fast_path\":{},\"status\":\"{}\",\"issues\":[{}]}}",
        json_escape(&entry.semantic_id),
        json_escape(&entry.title),
        entry.kind.as_str(),
        source_range,
        destination_range,
        source_address,
        destination_address,
        entry.source_size_bytes,
        entry.destination_size_bytes,
        entry.changed_bytes,
        ranges,
        entry.fast_path,
        entry.status.as_str(),
        issues
    )
}

fn transfer_issue_json(issue: &TransferIssue) -> String {
    let semantic_id = issue
        .semantic_id
        .as_deref()
        .map(string_json)
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"code\":\"{}\",\"semantic_id\":{},\"message\":\"{}\",\"blocking\":{}}}",
        issue.code.as_str(),
        semantic_id,
        json_escape(&issue.message),
        issue.blocking
    )
}

fn range_json(range: tuner_core::ByteRange) -> String {
    format!(
        "{{\"start\":{},\"end\":{},\"length\":{}}}",
        range.start,
        range.end,
        range.len()
    )
}

fn endianness_name(endianness: tuner_core::Endianness) -> &'static str {
    match endianness {
        tuner_core::Endianness::Little => "little",
        tuner_core::Endianness::Big => "big",
    }
}

fn string_json(value: &str) -> String {
    format!("\"{}\"", json_escape(value))
}
fn plan_byte_command(args: &[String]) -> Result<CommandResult, CommandError> {
    if args.is_empty() {
        return Err(CommandError::new(
            "plan-byte requires an input path and --offset and --value",
        ));
    }
    let input = PathBuf::from(&args[0]);
    let mut offset = None;
    let mut value = None;
    let mut index = 1;
    while index < args.len() {
        let option = args[index].as_str();
        let argument = args.get(index + 1).ok_or_else(|| {
            CommandError::new(format!("{option} requires a value")).with("option", option)
        })?;
        if argument.starts_with('-') {
            return Err(
                CommandError::new(format!("{option} requires a value")).with("option", option)
            );
        }
        match option {
            "--offset" => {
                if offset.is_some() {
                    return Err(CommandError::new("--offset may only be supplied once"));
                }
                offset = Some(parse_number(argument, "offset")?);
            }
            "--value" => {
                if value.is_some() {
                    return Err(CommandError::new("--value may only be supplied once"));
                }
                let parsed = parse_number(argument, "value")?;
                if parsed > usize::from(u8::MAX) {
                    return Err(CommandError::new("value must be between 0 and 255")
                        .with("value", argument));
                }
                value = Some(parsed as u8);
            }
            _ => {
                return Err(
                    CommandError::new(format!("unknown plan-byte option '{option}'"))
                        .with("option", option),
                );
            }
        }
        index += 2;
    }
    let offset = offset.ok_or_else(|| CommandError::new("plan-byte is missing --offset"))?;
    let value = value.ok_or_else(|| CommandError::new("plan-byte is missing --value"))?;
    let document =
        BinDocument::load(&input).map_err(|error| load_error("plan-byte", &input, error))?;
    let before_hash = sha256_hex(document.bytes());
    let before_value = document
        .read_u8(offset)
        .map_err(|error| CommandError::new(error.to_string()).with("offset", offset.to_string()))?;
    let mut candidate = document.clone();
    let summary = {
        let mut transaction = candidate.transaction("cli.plan-byte");
        transaction.write_u8(offset, value).map_err(|error| {
            CommandError::new(error.to_string()).with("offset", offset.to_string())
        })?;
        transaction.commit().map_err(|error| {
            CommandError::new(error.to_string()).with("offset", offset.to_string())
        })?
    };
    let after_hash = sha256_hex(candidate.bytes());
    let mut context = LogContext::new();
    context.insert("input", input.display().to_string());
    context.insert("offset", offset.to_string());
    context.insert("before_value", before_value.to_string());
    context.insert("value", value.to_string());
    context.insert("changed_bytes", summary.changed_bytes.to_string());
    context.insert("before_sha256", before_hash.clone());
    context.insert("after_sha256", after_hash.clone());
    let json = format!(
        "{{\"schema\":\"{RESULT_SCHEMA}\",\"command\":\"plan-byte\",\"status\":\"ok\",\"input\":\"{}\",\"offset\":{},\"before_value\":{},\"value\":{},\"changed_bytes\":{},\"before_sha256\":\"{}\",\"after_sha256\":\"{}\",\"dry_run\":true}}",
        json_escape(&input.display().to_string()),
        offset,
        before_value,
        value,
        summary.changed_bytes,
        before_hash,
        after_hash
    );
    Ok(CommandResult {
        json,
        message: "byte edit validated as a dry-run; no file was written".to_string(),
        context,
    })
}

fn edit_byte_command(args: &[String]) -> Result<CommandResult, CommandError> {
    if args.is_empty() {
        return Err(CommandError::new(
            "edit-byte requires an input path and --offset, --value, and --output",
        ));
    }
    let input = PathBuf::from(&args[0]);
    let mut offset = None;
    let mut value = None;
    let mut output = None;
    let mut index = 1;
    while index < args.len() {
        let option = args[index].as_str();
        let argument = args.get(index + 1).ok_or_else(|| {
            CommandError::new(format!("{option} requires a value")).with("option", option)
        })?;
        if argument.starts_with('-') && option != "--value" {
            return Err(
                CommandError::new(format!("{option} requires a value")).with("option", option)
            );
        }
        match option {
            "--offset" => {
                if offset.is_some() {
                    return Err(CommandError::new("--offset may only be supplied once"));
                }
                offset = Some(parse_number(argument, "offset")?);
            }
            "--value" => {
                if value.is_some() {
                    return Err(CommandError::new("--value may only be supplied once"));
                }
                let parsed = parse_number(argument, "value")?;
                if parsed > usize::from(u8::MAX) {
                    return Err(CommandError::new("value must be between 0 and 255")
                        .with("value", argument));
                }
                value = Some(parsed as u8);
            }
            "--output" => {
                if output.is_some() {
                    return Err(CommandError::new("--output may only be supplied once"));
                }
                if argument.is_empty() {
                    return Err(CommandError::new("--output path cannot be empty"));
                }
                output = Some(PathBuf::from(argument));
            }
            _ => {
                return Err(
                    CommandError::new(format!("unknown edit-byte option '{option}'"))
                        .with("option", option),
                );
            }
        }
        index += 2;
    }
    let offset = offset.ok_or_else(|| CommandError::new("edit-byte is missing --offset"))?;
    let value = value.ok_or_else(|| CommandError::new("edit-byte is missing --value"))?;
    let output = output.ok_or_else(|| CommandError::new("edit-byte is missing --output"))?;
    let mut document =
        BinDocument::load(&input).map_err(|error| load_error("edit-byte", &input, error))?;
    let before_hash = sha256_hex(document.bytes());
    let summary = {
        let mut transaction = document.transaction("cli.edit-byte");
        transaction.write_u8(offset, value).map_err(|error| {
            CommandError::new(error.to_string()).with("offset", offset.to_string())
        })?;
        transaction.commit().map_err(|error| {
            CommandError::new(error.to_string()).with("offset", offset.to_string())
        })?
    };
    document.save_as(&output).map_err(|error| {
        CommandError::new(error.to_string())
            .with("input", input.display().to_string())
            .with("output", output.display().to_string())
            .with("changed_bytes", summary.changed_bytes.to_string())
    })?;
    let after_hash = sha256_hex(document.bytes());
    let mut context = LogContext::new();
    context.insert("input", input.display().to_string());
    context.insert("output", output.display().to_string());
    context.insert("offset", offset.to_string());
    context.insert("value", value.to_string());
    context.insert("changed_bytes", summary.changed_bytes.to_string());
    context.insert("before_sha256", before_hash.clone());
    context.insert("after_sha256", after_hash.clone());
    let json = format!(
        "{{\"schema\":\"{RESULT_SCHEMA}\",\"command\":\"edit-byte\",\"status\":\"ok\",\"input\":\"{}\",\"output\":\"{}\",\"offset\":{},\"value\":{},\"changed_bytes\":{},\"before_sha256\":\"{}\",\"after_sha256\":\"{}\",\"undoable\":true}}",
        json_escape(&input.display().to_string()),
        json_escape(&output.display().to_string()),
        offset,
        value,
        summary.changed_bytes,
        before_hash,
        after_hash
    );
    Ok(CommandResult {
        json,
        message: "byte edit committed and exported".to_string(),
        context,
    })
}

fn parse_number(value: &str, name: &str) -> Result<usize, CommandError> {
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
        return Err(
            CommandError::new(format!("{name} cannot be empty")).with(name, value.to_string())
        );
    }
    usize::from_str_radix(digits, radix).map_err(|_| {
        CommandError::new(format!(
            "{name} must be a non-negative decimal or hexadecimal integer"
        ))
        .with(name, value.to_string())
    })
}

fn load_error(operation: &str, path: &Path, error: CoreError) -> CommandError {
    CommandError::new(error.to_string())
        .with("operation", operation)
        .with("path", path.display().to_string())
}

fn hex_preview(bytes: &[u8], limit: usize) -> String {
    bytes
        .iter()
        .take(limit)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

fn failure_json(operation: &str, message: &str) -> String {
    format!(
        "{{\"schema\":\"{RESULT_SCHEMA}\",\"command\":\"{}\",\"status\":\"error\",\"error\":\"{}\"}}",
        json_escape(operation),
        json_escape(message)
    )
}

fn failure_json_with_context(operation: &str, error: &CommandError) -> String {
    let mut output = String::from("{\"schema\":\"");
    output.push_str(RESULT_SCHEMA);
    output.push_str("\",\"command\":\"");
    output.push_str(&json_escape(operation));
    output.push_str("\",\"status\":\"error\",\"error\":{\"operation\":\"");
    output.push_str(&json_escape(operation));
    output.push_str("\",\"message\":\"");
    output.push_str(&json_escape(&error.message));
    output.push_str("\",\"context\":");
    output.push_str(&error.context.to_json());
    output.push_str("}}");
    output
}

fn append_json_string_field(json: &str, key: &str, value: &str) -> String {
    let trimmed = json.trim_end();
    let Some(prefix) = trimmed.strip_suffix('}') else {
        return trimmed.to_string();
    };
    let mut output = String::with_capacity(trimmed.len() + key.len() + value.len() + 8);
    output.push_str(prefix);
    output.push_str(",\"");
    output.push_str(&json_escape(key));
    output.push_str("\":\"");
    output.push_str(&json_escape(value));
    output.push_str("\"}");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_decimal_and_hex_numbers() {
        assert_eq!(parse_number("42", "offset").unwrap(), 42);
        assert_eq!(parse_number("0x2a", "offset").unwrap(), 42);
    }

    #[test]
    fn extracts_log_file_without_changing_command_arguments() {
        let (args, path) = extract_global_options(vec![
            "inspect".to_string(),
            "file.bin".to_string(),
            "--log-file".to_string(),
            "run.jsonl".to_string(),
        ])
        .unwrap();
        assert_eq!(args, vec!["inspect", "file.bin"]);
        assert_eq!(path, Some(PathBuf::from("run.jsonl")));
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
        let document_json = inspection_json(&document, Path::new("fixture.xdf"));
        assert!(document_json.contains("\"header\""));
        assert!(document_json.contains("\"category_count\":1"));
        assert!(document_json.contains("\"category_reference_mode\":\"declared-index\""));
        assert!(document_json.contains("\"category_path\":[\"Fuel\"]"));
        assert!(document_json.contains("\"resolved_category_index\":2"));
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
