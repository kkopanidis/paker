use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct IndexQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<Vec<String>>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    500
}

impl IndexQuery {
    pub fn with_key_substring(text: &str) -> Self {
        let trimmed = text.trim();
        Self {
            key_pattern: if trimmed.is_empty() {
                None
            } else {
                Some(format!(
                    "%{}%",
                    escape_like(trimmed)
                ))
            },
            limit: default_limit(),
            ..Default::default()
        }
    }
}

pub fn escape_like(value: &str) -> String {
    value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

pub fn describe_index_query(query: &IndexQuery) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(prefix) = &query.prefix {
        parts.push(format!("prefix starts with `{prefix}`"));
    }
    if let Some(pattern) = &query.key_pattern {
        let readable = pattern
            .trim_matches('%')
            .replace("\\%", "%")
            .replace("\\_", "_")
            .replace("\\\\", "\\");
        if readable.contains('%') || readable.contains('_') {
            parts.push(format!("key matches `{readable}`"));
        } else {
            parts.push(format!("key contains `{readable}`"));
        }
    }
    if let Some(min) = query.min_size {
        parts.push(format!("size ≥ {}", format_bytes(min)));
    }
    if let Some(max) = query.max_size {
        parts.push(format!("size ≤ {}", format_bytes(max)));
    }
    if let Some(after) = &query.modified_after {
        parts.push(format!("modified after {after}"));
    }
    if let Some(before) = &query.modified_before {
        parts.push(format!("modified before {before}"));
    }
    if let Some(classes) = &query.storage_class {
        if !classes.is_empty() {
            parts.push(format!("storage class: {}", classes.join(", ")));
        }
    }

    if parts.is_empty() {
        "No filters (matches nothing)".to_string()
    } else {
        parts.join("; ")
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_combines_clauses() {
        let q = IndexQuery {
            min_size: Some(100 * 1024 * 1024),
            key_pattern: Some("%.pdf".to_string()),
            ..Default::default()
        };
        let desc = describe_index_query(&q);
        assert!(desc.contains("size ≥"));
        assert!(desc.contains(".pdf"));
    }
}
