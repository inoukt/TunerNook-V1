use serde::{Deserialize, Serialize};
use tuner_core::{BinDocument, ByteRange};
use tuner_xdf::{ParameterDefinition, RawValue, XdfDocument};

use crate::{summary_from_parameter, ParameterSummary, SortDirection};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum SearchMatchMode {
    #[default]
    Contains,
    Exact,
    Wildcard,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum SearchFieldScope {
    #[default]
    All,
    Metadata,
    Title,
    Category,
    Ids,
    Type,
    AddressSize,
    RawValues,
    EngineeringValues,
}

impl SearchFieldScope {
    pub fn includes_metadata(self) -> bool {
        matches!(
            self,
            Self::All
                | Self::Metadata
                | Self::Title
                | Self::Category
                | Self::Ids
                | Self::Type
                | Self::AddressSize
        )
    }

    pub fn includes_raw_values(self) -> bool {
        matches!(self, Self::All | Self::RawValues)
    }

    pub fn includes_engineering_values(self) -> bool {
        matches!(self, Self::All | Self::EngineeringValues)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum SearchSortKey {
    #[default]
    Relevance,
    Category,
    Title,
    Type,
    Dimensions,
    Elements,
    Bytes,
    Address,
    MatchCount,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct SearchWindowMemory {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default = "crate::window_geometry::legacy_position_saved")]
    pub position_saved: bool,
    pub zoom_percent: u16,
}

impl Default for SearchWindowMemory {
    fn default() -> Self {
        Self {
            x: 140,
            y: 100,
            width: 780,
            height: 640,
            position_saved: false,
            zoom_percent: 100,
        }
    }
}

impl SearchWindowMemory {
    pub fn sanitize(&mut self) {
        self.width = self.width.clamp(420, 2_000);
        self.height = self.height.clamp(280, 1_600);
        self.zoom_percent = self.zoom_percent.clamp(75, 175);
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct SearchState {
    pub open: bool,
    pub query: String,
    pub match_mode: SearchMatchMode,
    pub field_scope: SearchFieldScope,
    pub sort_key: SearchSortKey,
    pub sort_direction: SortDirection,
    pub selected_semantic_id: Option<String>,
    pub result_limit: usize,
    pub window: SearchWindowMemory,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            open: false,
            query: String::new(),
            match_mode: SearchMatchMode::Contains,
            field_scope: SearchFieldScope::All,
            sort_key: SearchSortKey::Relevance,
            sort_direction: SortDirection::Ascending,
            selected_semantic_id: None,
            result_limit: 2_000,
            window: SearchWindowMemory::default(),
        }
    }
}

impl SearchState {
    pub fn for_query(query: impl Into<String>, field_scope: SearchFieldScope) -> Self {
        Self {
            open: true,
            query: query.into(),
            field_scope,
            ..Self::default()
        }
    }

    pub fn sanitize(&mut self) {
        self.query = self.query.chars().take(512).collect();
        self.result_limit = self.result_limit.clamp(1, 20_000);
        self.selected_semantic_id = self
            .selected_semantic_id
            .take()
            .filter(|value| !value.is_empty());
        self.window.sanitize();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchMatch {
    pub row: usize,
    pub column: usize,
    pub range: ByteRange,
    pub raw: RawValue,
    pub engineering: Option<f64>,
    pub display: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchDiagnostic {
    pub row: usize,
    pub column: usize,
    pub range: Option<ByteRange>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub summary: ParameterSummary,
    pub matches: Vec<SearchMatch>,
    pub diagnostics: Vec<SearchDiagnostic>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchMatchEvaluation {
    matched: bool,
}

impl SearchMatchEvaluation {
    pub fn is_match(self) -> bool {
        self.matched
    }
}

pub fn search_summary(
    summary: &ParameterSummary,
    query: &str,
    mode: SearchMatchMode,
    scope: SearchFieldScope,
) -> SearchMatchEvaluation {
    if query.trim().is_empty() || !scope.includes_metadata() {
        return SearchMatchEvaluation { matched: false };
    }
    let fields = metadata_fields(summary, scope);
    SearchMatchEvaluation {
        matched: fields.iter().any(|field| text_matches(field, query, mode)),
    }
}

pub fn compile_query(state: &SearchState) -> Result<(), String> {
    if state.match_mode != SearchMatchMode::Exact || state.query.trim().is_empty() {
        return Ok(());
    }
    let numeric_scope = match state.field_scope {
        SearchFieldScope::RawValues => "raw",
        SearchFieldScope::EngineeringValues => "engineering",
        SearchFieldScope::All if looks_like_numeric_query(&state.query) => "numeric",
        _ => return Ok(()),
    };
    if parse_numeric_query(&state.query).is_none() {
        Err(format!(
            "Exact {numeric_scope} value searches require a finite decimal or hexadecimal number."
        ))
    } else {
        Ok(())
    }
}

fn looks_like_numeric_query(query: &str) -> bool {
    let query = query.trim();
    query.starts_with("0x")
        || query.starts_with("0X")
        || query.starts_with('+')
        || query.starts_with('-')
        || query.starts_with('.')
        || query
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
}

pub fn search_query_error(state: &SearchState) -> Option<String> {
    compile_query(state).err()
}

pub fn search_snapshot(
    xdf: &XdfDocument,
    bin_bytes: Option<&[u8]>,
    state: &SearchState,
) -> Vec<SearchResult> {
    let query = state.query.trim();
    let bin = bin_bytes.map(|bytes| BinDocument::from_bytes(bytes.to_vec()));
    let mut results = Vec::new();
    for parameter in &xdf.parameters {
        let summary = summary_from_parameter(parameter);
        let metadata_match = if query.is_empty() {
            state.field_scope.includes_metadata()
        } else {
            search_summary(&summary, query, state.match_mode, state.field_scope).is_match()
        };
        let (value_matches, value_diagnostics) = if query.is_empty() {
            (Vec::new(), Vec::new())
        } else if let Some(bin) = bin.as_ref() {
            search_parameter_values(parameter, bin, query, state)
        } else {
            (Vec::new(), Vec::new())
        };
        if metadata_match || !value_matches.is_empty() || !value_diagnostics.is_empty() {
            results.push(SearchResult {
                summary,
                matches: value_matches,
                diagnostics: value_diagnostics,
            });
        }
    }
    sort_results(&mut results, state.sort_key, state.sort_direction);
    results.truncate(state.result_limit.clamp(1, 20_000));
    results
}

fn metadata_fields(summary: &ParameterSummary, scope: SearchFieldScope) -> Vec<String> {
    match scope {
        SearchFieldScope::All | SearchFieldScope::Metadata => vec![
            summary.title.clone(),
            summary.category_path.join(" › "),
            summary.unique_id.clone().unwrap_or_default(),
            summary.semantic_id.clone(),
            summary.kind.as_str().to_string(),
            format!(
                "0x{:X} {} {}×{} {}",
                summary.address,
                summary.byte_size,
                summary.rows,
                summary.columns,
                summary.element_count.unwrap_or_default()
            ),
        ],
        SearchFieldScope::Title => vec![summary.title.clone()],
        SearchFieldScope::Category => vec![summary.category_path.join(" › ")],
        SearchFieldScope::Ids => vec![
            summary.unique_id.clone().unwrap_or_default(),
            summary.semantic_id.clone(),
        ],
        SearchFieldScope::Type => vec![summary.kind.as_str().to_string()],
        SearchFieldScope::AddressSize => vec![format!(
            "0x{:X} {} {}×{} {}",
            summary.address,
            summary.byte_size,
            summary.rows,
            summary.columns,
            summary.element_count.unwrap_or_default()
        )],
        SearchFieldScope::RawValues | SearchFieldScope::EngineeringValues => Vec::new(),
    }
}

fn search_parameter_values(
    parameter: &ParameterDefinition,
    bin: &BinDocument,
    query: &str,
    state: &SearchState,
) -> (Vec<SearchMatch>, Vec<SearchDiagnostic>) {
    let query_numeric = parse_numeric_query(query);
    let mut matches = Vec::new();
    let mut diagnostics = Vec::new();
    let dimensions = parameter.layout.dimensions;
    for row in 0..dimensions.rows {
        for column in 0..dimensions.columns {
            let range = parameter.cell_range(row, column).ok();
            let raw = match parameter.read_raw_cell(bin, row, column) {
                Ok(raw) => raw,
                Err(error) => {
                    if state.field_scope.includes_raw_values()
                        || state.field_scope.includes_engineering_values()
                    {
                        diagnostics.push(SearchDiagnostic {
                            row,
                            column,
                            range,
                            message: format!("Could not read the raw value for this cell: {error}"),
                        });
                    }
                    continue;
                }
            };
            let engineering_result = parameter.read_engineering_cell(bin, row, column);
            let engineering = engineering_result.as_ref().ok().copied();
            let engineering_error = engineering_result.err().map(|error| error.to_string());
            if let Some(error) = engineering_error.as_deref() {
                if state.field_scope.includes_engineering_values() {
                    diagnostics.push(SearchDiagnostic {
                        row,
                        column,
                        range,
                        message: format!("Could not calculate the engineering value: {error}"),
                    });
                }
            }
            let raw_value_match =
                value_matches_raw(raw, query, state.match_mode, query_numeric.as_ref());
            let raw_match = state.field_scope.includes_raw_values() && raw_value_match;
            let engineering_match = state.field_scope.includes_engineering_values()
                && value_matches_engineering(
                    engineering,
                    query,
                    state.match_mode,
                    query_numeric.as_ref(),
                );
            let engineering_error_match = state.field_scope.includes_engineering_values()
                && engineering_error.is_some()
                && raw_value_match;
            if raw_match || engineering_match || engineering_error_match {
                matches.push(SearchMatch {
                    row,
                    column,
                    range: range.unwrap_or(parameter.layout.range),
                    raw,
                    engineering,
                    display: format_value_display(raw, engineering),
                    error: engineering_error,
                });
            }
        }
    }
    (matches, diagnostics)
}

fn value_matches_raw(
    raw: RawValue,
    query: &str,
    mode: SearchMatchMode,
    numeric: Option<&NumericQuery>,
) -> bool {
    match mode {
        SearchMatchMode::Exact => numeric.is_some_and(|query| raw_matches_numeric(raw, query)),
        SearchMatchMode::Contains | SearchMatchMode::Wildcard => {
            text_matches(&format_raw_value(raw), query, mode)
        }
    }
}

fn value_matches_engineering(
    engineering: Option<f64>,
    query: &str,
    mode: SearchMatchMode,
    numeric: Option<&NumericQuery>,
) -> bool {
    let Some(engineering) = engineering else {
        return false;
    };
    match mode {
        SearchMatchMode::Exact => numeric.is_some_and(|query| {
            let target = query.as_f64();
            target.is_finite() && (engineering - target).abs() <= 1e-9 * target.abs().max(1.0)
        }),
        SearchMatchMode::Contains | SearchMatchMode::Wildcard => {
            text_matches(&format!("{engineering:.12}"), query, mode)
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum NumericQuery {
    Integer(i128),
    Float(f64),
}

impl NumericQuery {
    fn as_f64(self) -> f64 {
        match self {
            Self::Integer(value) => value as f64,
            Self::Float(value) => value,
        }
    }
}

fn parse_numeric_query(query: &str) -> Option<NumericQuery> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (sign, digits) = match trimmed.as_bytes().first().copied() {
        Some(b'-') => (-1i128, &trimmed[1..]),
        Some(b'+') => (1i128, &trimmed[1..]),
        _ => (1i128, trimmed),
    };
    if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        let value = i128::from_str_radix(hex, 16).ok()?;
        return Some(NumericQuery::Integer(value * sign));
    }
    if digits.contains('.') || digits.contains('e') || digits.contains('E') {
        let value = trimmed.parse::<f64>().ok()?;
        value.is_finite().then_some(NumericQuery::Float(value))
    } else {
        trimmed.parse::<i128>().ok().map(NumericQuery::Integer)
    }
}

fn raw_matches_numeric(raw: RawValue, query: &NumericQuery) -> bool {
    match (raw, query) {
        (RawValue::Unsigned(value), NumericQuery::Integer(query)) => {
            u64::try_from(*query).is_ok_and(|query| value == query)
        }
        (RawValue::Signed(value), NumericQuery::Integer(query)) => {
            i64::try_from(*query).is_ok_and(|query| value == query)
        }
        (RawValue::Float32Bits(bits), NumericQuery::Integer(query)) => bits as i128 == *query,
        (RawValue::Unsigned(value), NumericQuery::Float(query)) => {
            (value as f64 - *query).abs() <= 1e-9 * query.abs().max(1.0)
        }
        (RawValue::Signed(value), NumericQuery::Float(query)) => {
            (value as f64 - *query).abs() <= 1e-9 * query.abs().max(1.0)
        }
        (RawValue::Float32Bits(bits), NumericQuery::Float(query)) => {
            let value = f32::from_bits(bits) as f64;
            value.is_finite() && (value - *query).abs() <= 1e-9 * query.abs().max(1.0)
        }
    }
}

fn text_matches(value: &str, query: &str, mode: SearchMatchMode) -> bool {
    let value = value.trim().to_lowercase();
    let query = query.trim().to_lowercase();
    match mode {
        SearchMatchMode::Contains => value.contains(&query),
        SearchMatchMode::Exact => value == query,
        SearchMatchMode::Wildcard => wildcard_matches(&value, &query),
    }
}

fn wildcard_matches(value: &str, pattern: &str) -> bool {
    let value: Vec<char> = value.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    let mut value_index = 0;
    let mut pattern_index = 0;
    let mut star_index = None;
    let mut star_value_index = 0;
    while value_index < value.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == '?' || pattern[pattern_index] == value[value_index])
        {
            value_index += 1;
            pattern_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == '*' {
            star_index = Some(pattern_index);
            star_value_index = value_index;
            pattern_index += 1;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            star_value_index += 1;
            value_index = star_value_index;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == '*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

fn format_raw_value(raw: RawValue) -> String {
    match raw {
        RawValue::Unsigned(value) => format!("{value} 0x{value:X}"),
        RawValue::Signed(value) => format!("{value} 0x{value:X}"),
        RawValue::Float32Bits(bits) => {
            format!("{} 0x{bits:X}", f32::from_bits(bits))
        }
    }
}

fn format_value_display(raw: RawValue, engineering: Option<f64>) -> String {
    match engineering {
        Some(engineering) => format!(
            "raw={} engineering={engineering:.12}",
            format_raw_value(raw)
        ),
        None => format!("raw={}", format_raw_value(raw)),
    }
}

fn sort_results(results: &mut [SearchResult], key: SearchSortKey, direction: SortDirection) {
    results.sort_by(|left, right| {
        let ordering = match key {
            SearchSortKey::Relevance => right
                .matches
                .len()
                .cmp(&left.matches.len())
                .then(left.summary.title.cmp(&right.summary.title)),
            SearchSortKey::Category => left.summary.category_path.cmp(&right.summary.category_path),
            SearchSortKey::Title => left.summary.title.cmp(&right.summary.title),
            SearchSortKey::Type => left.summary.kind.cmp(&right.summary.kind),
            SearchSortKey::Dimensions => (left.summary.rows, left.summary.columns)
                .cmp(&(right.summary.rows, right.summary.columns)),
            SearchSortKey::Elements => left.summary.element_count.cmp(&right.summary.element_count),
            SearchSortKey::Bytes => left.summary.byte_size.cmp(&right.summary.byte_size),
            SearchSortKey::Address => left.summary.address.cmp(&right.summary.address),
            SearchSortKey::MatchCount => left.matches.len().cmp(&right.matches.len()),
        };
        let ordering = match direction {
            SortDirection::Ascending => ordering,
            SortDirection::Descending => ordering.reverse(),
        };
        ordering.then(left.summary.semantic_id.cmp(&right.summary.semantic_id))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary_from_parameter;
    use tuner_core::BinDocument;
    use tuner_xdf::XdfDocument;

    const VALUE_XDF: &str = r#"
        <XDFFORMAT><XDFHEADER><CATEGORY index="0" name="Fuel" /></XDFHEADER>
        <XDFTABLE uniqueid="map">
          <title>Driver Pedal Map</title>
          <CATEGORYMEM index="0" category="0" />
          <XDFDATA><EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="2" /></XDFDATA>
        </XDFTABLE></XDFFORMAT>
    "#;

    const ERROR_VALUE_XDF: &str = r#"
        <XDFFORMAT><XDFHEADER><CATEGORY index="0" name="Fuel" /></XDFHEADER>
        <XDFTABLE uniqueid="map">
          <title>Broken Conversion Map</title>
          <CATEGORYMEM index="0" category="0" />
          <XDFCONVERT><MATH equation="X / (X - 10)" /></XDFCONVERT>
          <XDFDATA><EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="1" /></XDFDATA>
        </XDFTABLE></XDFFORMAT>
    "#;

    #[test]
    fn search_matches_contains_exact_and_wildcard_modes() {
        let xdf = XdfDocument::parse(VALUE_XDF.as_bytes()).unwrap();
        let row = summary_from_parameter(&xdf.parameters[0]);
        assert!(search_summary(
            &row,
            "pedal",
            SearchMatchMode::Contains,
            SearchFieldScope::Title
        )
        .is_match());
        assert!(search_summary(
            &row,
            "driver pedal map",
            SearchMatchMode::Exact,
            SearchFieldScope::Title
        )
        .is_match());
        assert!(search_summary(
            &row,
            "Driver*Map",
            SearchMatchMode::Wildcard,
            SearchFieldScope::Title
        )
        .is_match());
        assert!(!search_summary(
            &row,
            "Driver?Pedal",
            SearchMatchMode::Wildcard,
            SearchFieldScope::Title
        )
        .is_match());
    }

    #[test]
    fn search_reads_raw_and_engineering_values_without_matching_errors() {
        let xdf = XdfDocument::parse(VALUE_XDF.as_bytes()).unwrap();
        let raw_state = SearchState::for_query("0x0A", SearchFieldScope::RawValues);
        let mut raw_state = raw_state;
        raw_state.match_mode = SearchMatchMode::Exact;
        let results = search_snapshot(&xdf, Some(&[10, 20]), &raw_state);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matches[0].row, 0);
        assert_eq!(results[0].matches[0].column, 0);

        let mut engineering_state =
            SearchState::for_query("10", SearchFieldScope::EngineeringValues);
        engineering_state.match_mode = SearchMatchMode::Exact;
        let engineering_results = search_snapshot(&xdf, Some(&[10, 20]), &engineering_state);
        assert_eq!(engineering_results.len(), 1);
        assert_eq!(engineering_results[0].matches[0].column, 0);

        let mut wildcard_state = SearchState::for_query("*0x14", SearchFieldScope::RawValues);
        wildcard_state.match_mode = SearchMatchMode::Wildcard;
        let wildcard_results = search_snapshot(&xdf, Some(&[10, 20]), &wildcard_state);
        assert_eq!(wildcard_results.len(), 1);
        assert_eq!(wildcard_results[0].matches[0].column, 1);

        let bad = SearchState::for_query("not-a-number", SearchFieldScope::RawValues);
        assert!(search_snapshot(&xdf, Some(&[10, 20]), &bad).is_empty());

        let _ = BinDocument::from_bytes(vec![10, 20]);
    }

    #[test]
    fn search_state_round_trips_and_sanitizes_saved_values() {
        let mut state = SearchState::default();
        state.query = "x".repeat(600);
        state.result_limit = 0;
        state.window.width = 100;
        state.window.height = 100;
        state.window.zoom_percent = 220;
        state.selected_semantic_id = Some(String::new());
        state.sanitize();

        assert_eq!(state.query.chars().count(), 512);
        assert_eq!(state.result_limit, 1);
        assert_eq!(state.window.width, 420);
        assert_eq!(state.window.height, 280);
        assert_eq!(state.window.zoom_percent, 175);
        assert_eq!(state.selected_semantic_id, None);
        let encoded = serde_json::to_string(&state).unwrap();
        assert_eq!(
            serde_json::from_str::<SearchState>(&encoded).unwrap(),
            state
        );
    }

    #[test]
    fn exact_value_search_reports_invalid_numeric_queries() {
        let mut raw = SearchState::for_query("not-a-number", SearchFieldScope::RawValues);
        raw.match_mode = SearchMatchMode::Exact;
        assert!(search_query_error(&raw).is_some());
        assert!(compile_query(&raw).is_err());

        let mut metadata = raw.clone();
        metadata.field_scope = SearchFieldScope::Title;
        assert!(search_query_error(&metadata).is_none());

        raw.query = "0xFF".to_string();
        assert!(search_query_error(&raw).is_none());

        let mut all = SearchState::for_query("0xnot-a-number", SearchFieldScope::All);
        all.match_mode = SearchMatchMode::Exact;
        assert!(search_query_error(&all).is_some());

        let mut all_text = SearchState::for_query("driver pedal map", SearchFieldScope::All);
        all_text.match_mode = SearchMatchMode::Exact;
        assert!(search_query_error(&all_text).is_none());

        let mut all_metadata = SearchState::for_query("0x0 2 1×2 2", SearchFieldScope::All);
        all_metadata.match_mode = SearchMatchMode::Exact;
        assert!(search_query_error(&all_metadata).is_some());
        let xdf = XdfDocument::parse(VALUE_XDF.as_bytes()).unwrap();
        assert_eq!(search_snapshot(&xdf, None, &all_metadata).len(), 1);
    }

    #[test]
    fn empty_query_lists_metadata_without_scanning_values() {
        let xdf = XdfDocument::parse(VALUE_XDF.as_bytes()).unwrap();
        let results = search_snapshot(&xdf, None, &SearchState::default());
        assert_eq!(results.len(), 1);
        assert!(results[0].matches.is_empty());
    }

    #[test]
    fn value_matches_keep_conversion_errors_without_matching_the_error_text() {
        let xdf = XdfDocument::parse(ERROR_VALUE_XDF.as_bytes()).unwrap();
        let mut state = SearchState::for_query("10", SearchFieldScope::RawValues);
        state.match_mode = SearchMatchMode::Exact;
        let results = search_snapshot(&xdf, Some(&[10]), &state);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matches.len(), 1);
        assert!(results[0].matches[0]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("division by zero")));

        let mut engineering = SearchState::for_query("10", SearchFieldScope::EngineeringValues);
        engineering.match_mode = SearchMatchMode::Exact;
        let engineering_results = search_snapshot(&xdf, Some(&[10]), &engineering);
        assert_eq!(engineering_results.len(), 1);
        assert_eq!(engineering_results[0].matches.len(), 1);
        assert_eq!(engineering_results[0].diagnostics.len(), 1);
        assert!(engineering_results[0].matches[0]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("division by zero")));

        let mut unreadable = SearchState::for_query("10", SearchFieldScope::RawValues);
        unreadable.match_mode = SearchMatchMode::Exact;
        let unreadable_results = search_snapshot(&xdf, Some(&[]), &unreadable);
        assert_eq!(unreadable_results.len(), 1);
        assert_eq!(unreadable_results[0].matches.len(), 0);
        assert_eq!(unreadable_results[0].diagnostics.len(), 1);
    }
}
