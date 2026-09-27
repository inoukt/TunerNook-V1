use serde::{Deserialize, Serialize};
use tuner_xdf::{ParameterDefinition, ParameterKind};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, Default)]
pub enum CatalogSortKey {
    Category,
    #[default]
    Title,
    Type,
    Dimensions,
    Elements,
    Bytes,
    Address,
    Favorite,
    Recent,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn reversed(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterSummary {
    pub semantic_id: String,
    pub unique_id: Option<String>,
    pub title: String,
    pub category: String,
    pub category_path: Vec<String>,
    pub category_path_key: String,
    pub kind: ParameterKind,
    pub rows: usize,
    pub columns: usize,
    pub element_count: Option<usize>,
    pub byte_size: usize,
    pub address: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterGroup {
    pub category: String,
    pub parameters: Vec<ParameterSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryNode {
    pub key: String,
    pub name: String,
    pub path: Vec<String>,
    pub total_count: usize,
    pub parameters: Vec<ParameterSummary>,
    pub children: Vec<CategoryNode>,
}

pub fn category_path_key(path: &[String]) -> String {
    if path.is_empty() {
        return "Uncategorized".to_string();
    }
    if path.len() == 1 {
        return path[0].clone();
    }
    path.iter()
        .map(|component| format!("{}:{}", component.chars().count(), component))
        .collect::<Vec<_>>()
        .join("/")
}

pub fn summary_from_parameter(parameter: &ParameterDefinition) -> ParameterSummary {
    let mut category_path = parameter.category_path();
    if category_path.is_empty() {
        if let Some(category) = parameter.category.as_deref() {
            if !category.trim().is_empty() && !is_numeric_category_reference(category) {
                category_path.push(category.trim().to_string());
            }
        }
    }
    if category_path.is_empty() {
        category_path.push("Uncategorized".to_string());
    }
    let category = category_path[0].clone();
    ParameterSummary {
        semantic_id: parameter.semantic_id.clone(),
        unique_id: parameter.unique_id.clone(),
        title: parameter.title.clone(),
        category,
        category_path_key: category_path_key(&category_path),
        category_path,
        kind: parameter.kind,
        rows: parameter.layout.dimensions.rows,
        columns: parameter.layout.dimensions.columns,
        element_count: parameter.layout.element_count(),
        byte_size: parameter.layout.range.len(),
        address: parameter.layout.address,
    }
}

fn is_numeric_category_reference(value: &str) -> bool {
    let value = value.trim();
    value.parse::<i128>().is_ok()
        || value
            .strip_prefix("0x")
            .or_else(|| value.strip_prefix("0X"))
            .is_some_and(|digits| !digits.is_empty() && i128::from_str_radix(digits, 16).is_ok())
}

pub fn summary_label(summary: &ParameterSummary) -> String {
    let elements = summary
        .element_count
        .map(|count| format!(" · {count} el"))
        .unwrap_or_default();
    format!(
        "{} · {} · {}×{}{} · {} B",
        summary.title,
        summary.kind.as_str(),
        summary.rows,
        summary.columns,
        elements,
        summary.byte_size
    )
}

pub fn table_title_label(summary: &ParameterSummary) -> String {
    format!(
        "{} · {} · {}",
        summary.category_path.join(" › "),
        summary_label(summary),
        summary.semantic_id
    )
}

pub fn build_category_tree(mut summaries: Vec<ParameterSummary>) -> Vec<CategoryNode> {
    let mut roots = Vec::new();
    for summary in summaries.drain(..) {
        let mut path = summary.category_path.clone();
        if path.is_empty() {
            path.push("Uncategorized".to_string());
        }
        insert_summary(&mut roots, &path, 0, summary);
    }
    sort_category_nodes(&mut roots);
    roots
}

fn insert_summary(
    nodes: &mut Vec<CategoryNode>,
    path: &[String],
    depth: usize,
    summary: ParameterSummary,
) {
    let name = path[depth].clone();
    let prefix = path[..=depth].to_vec();
    let key = category_path_key(&prefix);
    let index = nodes.iter().position(|node| node.key == key);
    let index = match index {
        Some(index) => index,
        None => {
            nodes.push(CategoryNode {
                key,
                name,
                path: prefix,
                total_count: 0,
                parameters: Vec::new(),
                children: Vec::new(),
            });
            nodes.len() - 1
        }
    };
    nodes[index].total_count += 1;
    if depth + 1 == path.len() {
        nodes[index].parameters.push(summary);
    } else {
        insert_summary(&mut nodes[index].children, path, depth + 1, summary);
    }
}

fn sort_category_nodes(nodes: &mut [CategoryNode]) {
    nodes.sort_by(|left, right| left.name.cmp(&right.name).then(left.key.cmp(&right.key)));
    for node in nodes {
        node.parameters.sort_by(|left, right| {
            left.title
                .cmp(&right.title)
                .then(left.semantic_id.cmp(&right.semantic_id))
        });
        sort_category_nodes(&mut node.children);
    }
}

pub fn sort_summaries(
    summaries: &mut [ParameterSummary],
    key: CatalogSortKey,
    direction: SortDirection,
    favorite_keys: &[String],
    recent_keys: &[String],
) {
    summaries.sort_by(|left, right| {
        let ordering = match key {
            CatalogSortKey::Category => left
                .category_path
                .cmp(&right.category_path)
                .then(left.title.cmp(&right.title)),
            CatalogSortKey::Title => left.title.cmp(&right.title),
            CatalogSortKey::Type => left.kind.cmp(&right.kind),
            CatalogSortKey::Dimensions => {
                (left.rows, left.columns).cmp(&(right.rows, right.columns))
            }
            CatalogSortKey::Elements => left.element_count.cmp(&right.element_count),
            CatalogSortKey::Bytes => left.byte_size.cmp(&right.byte_size),
            CatalogSortKey::Address => left.address.cmp(&right.address),
            CatalogSortKey::Favorite => {
                favorite_rank(left, favorite_keys).cmp(&favorite_rank(right, favorite_keys))
            }
            CatalogSortKey::Recent => {
                recent_rank(left, recent_keys).cmp(&recent_rank(right, recent_keys))
            }
        };
        let ordering = match direction {
            SortDirection::Ascending => ordering,
            SortDirection::Descending => ordering.reverse(),
        };
        ordering.then(left.semantic_id.cmp(&right.semantic_id))
    });
}

fn favorite_rank(summary: &ParameterSummary, favorite_keys: &[String]) -> usize {
    if key_list_contains(summary, favorite_keys) {
        0
    } else {
        1
    }
}

fn recent_rank(summary: &ParameterSummary, recent_keys: &[String]) -> usize {
    recent_keys
        .iter()
        .position(|key| key_matches_summary(key, summary))
        .unwrap_or(recent_keys.len() + 1)
}

fn key_list_contains(summary: &ParameterSummary, keys: &[String]) -> bool {
    keys.iter().any(|key| key_matches_summary(key, summary))
}

fn key_matches_summary(key: &str, summary: &ParameterSummary) -> bool {
    key == summary.semantic_id || key.ends_with(&format!("|{}", summary.semantic_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuner_xdf::{ParameterKind, XdfDocument};

    const TABLE_XDF: &str = r#"
        <XDFFORMAT><XDFHEADER><CATEGORY index="0" name="Fuel" /></XDFHEADER>
        <XDFTABLE uniqueid="map">
          <title>Map</title>
          <CATEGORYMEM index="0" category="0" />
          <XDFDATA><EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8" mmedrowcount="2" mmedcolcount="3" /></XDFDATA>
        </XDFTABLE></XDFFORMAT>
    "#;

    const NUMERIC_CATEGORY_XDF: &str = r#"
        <XDFFORMAT><XDFTABLE uniqueid="orphan" category="57">
          <title>Orphan Map</title>
          <XDFDATA><EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="1" /></XDFDATA>
        </XDFTABLE></XDFFORMAT>
    "#;

    #[test]
    fn summary_exposes_shape_and_mapped_size() {
        let xdf = XdfDocument::parse(TABLE_XDF.as_bytes()).unwrap();
        let summary = summary_from_parameter(&xdf.parameters[0]);
        assert_eq!(summary.title, "Map");
        assert_eq!(summary.kind, ParameterKind::Table);
        assert_eq!((summary.rows, summary.columns), (2, 3));
        assert_eq!(summary.element_count, Some(6));
        assert_eq!(summary.byte_size, 6);
        assert_eq!(summary.address, 0);
        assert_eq!(summary.category_path, vec!["Fuel"]);
        assert!(summary_label(&summary).contains("2×3"));
        assert!(summary_label(&summary).contains("6 el"));
        assert!(summary_label(&summary).contains("6 B"));
        assert!(table_title_label(&summary).contains("Fuel"));
    }

    #[test]
    fn unresolved_numeric_category_references_are_uncategorized() {
        let xdf = XdfDocument::parse(NUMERIC_CATEGORY_XDF.as_bytes()).unwrap();
        let summary = summary_from_parameter(&xdf.parameters[0]);
        assert_eq!(summary.category_path, vec!["Uncategorized"]);
        assert_eq!(summary.category, "Uncategorized");
    }

    #[test]
    fn category_tree_preserves_nested_paths_and_subtree_counts() {
        let tree = build_category_tree(vec![
            summary("A", &["Limiter", "RPM"]),
            summary("B", &["Limiter", "RPM"]),
            summary("C", &["Limiter", "Speed"]),
        ]);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "Limiter");
        assert_eq!(tree[0].total_count, 3);
        assert_eq!(tree[0].children[0].name, "RPM");
        assert_eq!(tree[0].children[0].total_count, 2);
    }

    #[test]
    fn catalog_sort_is_stable_and_supports_size_and_direction() {
        let mut rows = vec![
            summary_with_size("same", 4, "b"),
            summary_with_size("same", 8, "a"),
        ];
        sort_summaries(
            &mut rows,
            CatalogSortKey::Bytes,
            SortDirection::Descending,
            &[],
            &[],
        );
        assert_eq!(
            rows.iter().map(|row| row.byte_size).collect::<Vec<_>>(),
            vec![8, 4]
        );
    }

    #[test]
    fn catalog_sort_controls_cover_metadata_favorites_and_recents() {
        let mut rows = vec![
            configured_summary("Zulu", "z", &["Fuel"], ParameterKind::Table, (2, 3), 6, 20),
            configured_summary(
                "Alpha",
                "a",
                &["Airflow"],
                ParameterKind::Constant,
                (1, 1),
                1,
                10,
            ),
            configured_summary(
                "Middle",
                "m",
                &["Fuel", "RPM"],
                ParameterKind::Flag,
                (1, 2),
                2,
                15,
            ),
        ];
        for key in [
            CatalogSortKey::Category,
            CatalogSortKey::Title,
            CatalogSortKey::Type,
            CatalogSortKey::Dimensions,
            CatalogSortKey::Elements,
            CatalogSortKey::Bytes,
            CatalogSortKey::Address,
        ] {
            let expected_ascending = match key {
                CatalogSortKey::Category | CatalogSortKey::Type => ["a", "z", "m"],
                _ => ["a", "m", "z"],
            };
            sort_summaries(&mut rows, key, SortDirection::Ascending, &[], &[]);
            assert_eq!(
                rows.iter()
                    .map(|summary| summary.semantic_id.as_str())
                    .collect::<Vec<_>>(),
                expected_ascending
            );
            sort_summaries(&mut rows, key, SortDirection::Descending, &[], &[]);
            let expected_descending = match key {
                CatalogSortKey::Category | CatalogSortKey::Type => ["m", "z", "a"],
                _ => ["z", "m", "a"],
            };
            assert_eq!(
                rows.iter()
                    .map(|summary| summary.semantic_id.as_str())
                    .collect::<Vec<_>>(),
                expected_descending
            );
        }

        sort_summaries(
            &mut rows,
            CatalogSortKey::Favorite,
            SortDirection::Ascending,
            &["z".to_string()],
            &[],
        );
        assert_eq!(
            rows.iter()
                .map(|summary| summary.semantic_id.as_str())
                .collect::<Vec<_>>(),
            ["z", "a", "m"]
        );
        sort_summaries(
            &mut rows,
            CatalogSortKey::Recent,
            SortDirection::Ascending,
            &[],
            &["m".to_string(), "z".to_string()],
        );
        assert_eq!(
            rows.iter()
                .map(|summary| summary.semantic_id.as_str())
                .collect::<Vec<_>>(),
            ["m", "z", "a"]
        );
    }

    fn summary(title: &str, path: &[&str]) -> ParameterSummary {
        ParameterSummary {
            semantic_id: title.to_ascii_lowercase(),
            unique_id: None,
            title: title.to_string(),
            category: path.first().copied().unwrap_or("Uncategorized").to_string(),
            category_path: path.iter().map(|part| (*part).to_string()).collect(),
            category_path_key: category_path_key(
                &path
                    .iter()
                    .map(|part| (*part).to_string())
                    .collect::<Vec<_>>(),
            ),
            kind: ParameterKind::Table,
            rows: 1,
            columns: 1,
            element_count: Some(1),
            byte_size: 1,
            address: 0,
        }
    }

    fn summary_with_size(title: &str, byte_size: usize, semantic_id: &str) -> ParameterSummary {
        let mut result = summary(title, &["Fuel"]);
        result.byte_size = byte_size;
        result.semantic_id = semantic_id.to_string();
        result
    }

    fn configured_summary(
        title: &str,
        semantic_id: &str,
        path: &[&str],
        kind: ParameterKind,
        dimensions: (usize, usize),
        element_count: usize,
        address: usize,
    ) -> ParameterSummary {
        let mut result = summary(title, path);
        result.semantic_id = semantic_id.to_string();
        result.kind = kind;
        result.rows = dimensions.0;
        result.columns = dimensions.1;
        result.element_count = Some(element_count);
        result.byte_size = element_count;
        result.address = address;
        result
    }
}
