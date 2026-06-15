use super::index_query::{describe_index_query, escape_like, IndexQuery};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParseConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ParsedAssistantQuery {
    pub query: IndexQuery,
    pub summary: String,
    pub confidence: ParseConfidence,
}

pub fn parse_natural_language(input: &str) -> ParsedAssistantQuery {
    let text = input.trim();
    if text.is_empty() {
        return ParsedAssistantQuery {
            query: IndexQuery::default(),
            summary: "Enter a search query".to_string(),
            confidence: ParseConfidence::Low,
        };
    }

    let lower = text.to_lowercase();
    let mut query = IndexQuery::default();
    let mut matched_rules = 0usize;

    if let Some(min) = parse_min_size(&lower) {
        query.min_size = Some(min);
        matched_rules += 1;
    }
    if let Some(max) = parse_max_size(&lower) {
        query.max_size = Some(max);
        matched_rules += 1;
    }
    if let Some(ext) = parse_extension(&lower, text) {
        query.key_pattern = Some(format!("%.{ext}"));
        matched_rules += 1;
    }
    if let Some(classes) = parse_storage_class(&lower) {
        query.storage_class = Some(classes);
        matched_rules += 1;
    }
    if let Some(after) = parse_modified_after(&lower) {
        query.modified_after = Some(after);
        matched_rules += 1;
    }
    if let Some(prefix) = parse_prefix_hint(text) {
        query.prefix = Some(prefix);
        matched_rules += 1;
    }
    if query.key_pattern.is_none() {
        if let Some(glob) = parse_glob_pattern(text) {
            query.key_pattern = Some(glob);
            matched_rules += 1;
        }
    }

    let confidence = if query.key_pattern.is_none() && query.prefix.is_none() && matched_rules == 0 {
        query = IndexQuery::with_key_substring(text);
        ParseConfidence::Low
    } else {
        if query.key_pattern.is_none() && matched_rules > 0 {
            let residual = strip_known_phrases(text);
            if !residual.is_empty() && residual.len() >= 2 {
                query.key_pattern = Some(format!("%{}%", escape_like(&residual)));
            }
        }
        match matched_rules {
            0 => ParseConfidence::Low,
            1 => ParseConfidence::Medium,
            _ => ParseConfidence::High,
        }
    };

    ParsedAssistantQuery {
        summary: describe_index_query(&query),
        query,
        confidence,
    }
}

fn parse_min_size(lower: &str) -> Option<u64> {
    for pattern in [
        r"(?:>|greater than|larger than|more than|at least|over|bigger than)\s*(\d+(?:\.\d+)?)\s*(b|kb|mb|gb|tb)?",
        r"(\d+(?:\.\d+)?)\s*(kb|mb|gb|tb)\s*(?:or\s+)?(?:larger|bigger|more|greater)",
    ] {
        if let Some(caps) = regex_simple(pattern, lower) {
            return parse_size_value(&caps.0, caps.1.as_deref());
        }
    }
    None
}

fn parse_max_size(lower: &str) -> Option<u64> {
    for pattern in [
        r"(?:<|less than|smaller than|under|at most|below)\s*(\d+(?:\.\d+)?)\s*(b|kb|mb|gb|tb)?",
        r"(\d+(?:\.\d+)?)\s*(kb|mb|gb|tb)\s*(?:or\s+)?(?:smaller|less)",
    ] {
        if let Some(caps) = regex_simple(pattern, lower) {
            return parse_size_value(&caps.0, caps.1.as_deref());
        }
    }
    None
}

fn parse_size_value(num: &str, unit: Option<&str>) -> Option<u64> {
    let value: f64 = num.parse().ok()?;
    let multiplier = match unit.unwrap_or("b").to_lowercase().as_str() {
        "kb" => 1024f64,
        "mb" => 1024f64 * 1024f64,
        "gb" => 1024f64 * 1024f64 * 1024f64,
        "tb" => 1024f64 * 1024f64 * 1024f64 * 1024f64,
        _ => 1f64,
    };
    Some((value * multiplier).round() as u64)
}

fn parse_extension(lower: &str, original: &str) -> Option<String> {
    if let Some(caps) = regex_simple(r"extension\s+([a-z0-9]+)", lower) {
        return Some(caps.0);
    }
    if let Some(caps) = regex_simple(r"\*\.([a-z0-9]+)", original) {
        return Some(caps.0.to_lowercase());
    }
    if let Some(caps) = regex_simple(r"\b([a-z0-9]{2,5})\s+files?\b", lower) {
        let ext = caps.0.to_lowercase();
        if matches!(
            ext.as_str(),
            "pdf" | "jpg" | "jpeg" | "png" | "gif" | "webp" | "txt" | "csv" | "json" | "xml"
                | "zip" | "gz" | "tar" | "mp4" | "mp3" | "log" | "html" | "md"
        ) {
            return Some(ext);
        }
    }
    None
}

fn parse_storage_class(lower: &str) -> Option<Vec<String>> {
    if lower.contains("glacier") || lower.contains("deep archive") {
        return Some(vec![
            "GLACIER".to_string(),
            "DEEP_ARCHIVE".to_string(),
            "GLACIER_IR".to_string(),
        ]);
    }
    if lower.contains("intelligent") && lower.contains("tier") {
        return Some(vec!["INTELLIGENT_TIERING".to_string()]);
    }
    if lower.contains("standard-ia") || lower.contains("infrequent") {
        return Some(vec!["STANDARD_IA".to_string(), "ONEZONE_IA".to_string()]);
    }
    if lower.contains("standard") && !lower.contains("non-standard") {
        return Some(vec!["STANDARD".to_string()]);
    }
    None
}

fn parse_modified_after(lower: &str) -> Option<String> {
    if let Some(caps) = regex_simple(r"last\s+(\d+)\s+days?", lower) {
        let days: i64 = caps.0.parse().ok()?;
        return Some((Utc::now() - Duration::days(days)).to_rfc3339());
    }
    if let Some(caps) = regex_simple(r"last\s+(\d+)\s+weeks?", lower) {
        let weeks: i64 = caps.0.parse().ok()?;
        return Some((Utc::now() - Duration::weeks(weeks)).to_rfc3339());
    }
    if lower.contains("last month") {
        return Some((Utc::now() - Duration::days(30)).to_rfc3339());
    }
    if lower.contains("last year") {
        return Some((Utc::now() - Duration::days(365)).to_rfc3339());
    }
    if let Some(caps) = regex_simple(r"since\s+(\d{4}-\d{2}-\d{2})", lower) {
        return Some(format!("{}T00:00:00Z", caps.0));
    }
    if let Some(caps) = regex_simple(r"after\s+(\d{4}-\d{2}-\d{2})", lower) {
        return Some(format!("{}T00:00:00Z", caps.0));
    }
    None
}

fn parse_prefix_hint(original: &str) -> Option<String> {
    if let Some(caps) = find_prefix_phrase(original) {
        let mut p = caps.0;
        if !p.ends_with('/') {
            p.push('/');
        }
        return Some(p);
    }
    let trimmed = original.trim();
    if trimmed.ends_with('/') && trimmed.contains('/') {
        return Some(trimmed.to_string());
    }
    None
}

fn parse_glob_pattern(original: &str) -> Option<String> {
    let trimmed = original.trim();
    if trimmed.contains('*') || trimmed.contains('?') {
        let like = trimmed
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
            .replace('*', "%")
            .replace('?', "_");
        return Some(like);
    }
    None
}

fn strip_known_phrases(text: &str) -> String {
    let mut s = text.to_string();
    for phrase in [
        r"(?i)greater than",
        r"(?i)larger than",
        r"(?i)more than",
        r"(?i)less than",
        r"(?i)last \d+ days?",
        r"(?i)last month",
        r"(?i)glacier",
        r"(?i)standard",
        r"(?i)files?",
        r"(?i)extension \w+",
        r"(?i)under [\w./-]+",
        r"(?i)in [\w./-]+",
    ] {
        s = regex_replace(phrase, &s, "");
    }
    s = regex_replace(r"\d+(?:\.\d+)?\s*(?:kb|mb|gb|tb|b)\b", &s, "");
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Lightweight pattern helper (no regex crate dependency).
fn regex_simple(pattern: &str, haystack: &str) -> Option<(String, Option<String>)> {
    match pattern {
        r"(?:>|greater than|larger than|more than|at least|over|bigger than)\s*(\d+(?:\.\d+)?)\s*(b|kb|mb|gb|tb)?" => {
            parse_size_phrase(haystack, false)
        }
        r"(\d+(?:\.\d+)?)\s*(kb|mb|gb|tb)\s*(?:or\s+)?(?:larger|bigger|more|greater)" => {
            parse_size_unit_first(haystack, false)
        }
        r"(?:<|less than|smaller than|under|at most|below)\s*(\d+(?:\.\d+)?)\s*(b|kb|mb|gb|tb)?" => {
            parse_size_phrase(haystack, true)
        }
        r"(\d+(?:\.\d+)?)\s*(kb|mb|gb|tb)\s*(?:or\s+)?(?:smaller|less)" => {
            parse_size_unit_first(haystack, true)
        }
        r"extension\s+([a-z0-9]+)" => find_after_word(haystack, "extension"),
        r"\*\.([a-z0-9]+)" => {
            if let Some(idx) = haystack.find("*.") {
                let rest = &haystack[idx + 2..];
                let ext: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
                if ext.is_empty() { None } else { Some((ext, None)) }
            } else {
                None
            }
        }
        r"\b([a-z0-9]{2,5})\s+files?\b" => find_word_before_files(haystack),
        r"last\s+(\d+)\s+days?" => find_last_n(haystack, "days"),
        r"last\s+(\d+)\s+weeks?" => find_last_n(haystack, "weeks"),
        r"since\s+(\d{4}-\d{2}-\d{2})" => find_iso_date_after(haystack, "since"),
        r"after\s+(\d{4}-\d{2}-\d{2})" => find_iso_date_after(haystack, "after"),
        _ => None,
    }
}

fn parse_size_phrase(haystack: &str, _less_than: bool) -> Option<(String, Option<String>)> {
    let lower = haystack.to_lowercase();
    for kw in [
        "greater than",
        "larger than",
        "more than",
        "at least",
        "over",
        "bigger than",
        "less than",
        "smaller than",
        "under",
        "at most",
        "below",
    ] {
        if let Some(idx) = lower.find(kw) {
            let rest = &haystack[idx + kw.len()..];
            return parse_number_unit(rest.trim_start());
        }
    }
    if let Some(idx) = lower.find('>') {
        return parse_number_unit(haystack[idx + 1..].trim_start());
    }
    if let Some(idx) = lower.find('<') {
        return parse_number_unit(haystack[idx + 1..].trim_start());
    }
    None
}

fn parse_size_unit_first(haystack: &str, _less: bool) -> Option<(String, Option<String>)> {
    parse_number_unit(haystack.trim())
}

fn parse_number_unit(s: &str) -> Option<(String, Option<String>)> {
    let s = s.trim();
    let mut num_end = 0;
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() || c == '.' {
            num_end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if num_end == 0 {
        return None;
    }
    let num = s[..num_end].to_string();
    let rest = s[num_end..].trim().to_lowercase();
    let unit = rest
        .split_whitespace()
        .next()
        .map(|u| u.trim_matches(|c: char| !c.is_ascii_alphabetic()).to_string());
    Some((num, unit.filter(|u| !u.is_empty())))
}

fn find_after_word(haystack: &str, word: &str) -> Option<(String, Option<String>)> {
    let lower = haystack.to_lowercase();
    let idx = lower.find(word)?;
    let rest = haystack[idx + word.len()..].trim();
    let ext: String = rest
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    if ext.is_empty() {
        None
    } else {
        Some((ext.to_lowercase(), None))
    }
}

fn find_word_before_files(haystack: &str) -> Option<(String, Option<String>)> {
    let lower = haystack.to_lowercase();
    let files_idx = lower.find(" file")?;
    let before = haystack[..files_idx].trim();
    let word = before.split_whitespace().last()?.to_lowercase();
    if word.len() < 2 || word.len() > 5 {
        return None;
    }
    Some((word, None))
}

fn find_last_n(haystack: &str, unit: &str) -> Option<(String, Option<String>)> {
    let lower = haystack.to_lowercase();
    let prefix = format!("last ");
    let idx = lower.find(&prefix)?;
    let rest = &lower[idx + prefix.len()..];
    let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if num.is_empty() || !rest[num.len()..].trim_start().starts_with(unit) {
        return None;
    }
    Some((num, None))
}

fn find_iso_date_after(haystack: &str, word: &str) -> Option<(String, Option<String>)> {
    let lower = haystack.to_lowercase();
    let idx = lower.find(word)?;
    let rest = haystack[idx + word.len()..].trim();
    if rest.len() >= 10 && rest.as_bytes()[4] == b'-' && rest.as_bytes()[7] == b'-' {
        Some((rest[..10].to_string(), None))
    } else {
        None
    }
}

fn find_prefix_phrase(haystack: &str) -> Option<(String, Option<String>)> {
    let lower = haystack.to_lowercase();
    for kw in ["under ", "in ", "prefix ", "folder "] {
        if let Some(idx) = lower.find(kw) {
            let rest = haystack[idx + kw.len()..]
                .trim()
                .trim_matches(['`', '"', '\'']);
            let path: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '/' || *c == '-' || *c == '_' || *c == '.')
                .collect();
            if !path.is_empty() {
                return Some((path, None));
            }
        }
    }
    None
}

fn regex_replace(_pattern: &str, text: &str, replacement: &str) -> String {
    // Minimal stub: strip common keywords manually
    let mut out = text.to_string();
    for kw in [
        "greater than",
        "larger than",
        "more than",
        "less than",
        "last month",
        "glacier",
        "standard",
        "files",
        "file",
    ] {
        if let Some(idx) = out.to_lowercase().find(kw) {
            out.replace_range(idx..idx + kw.len(), replacement);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_large_pdf_last_month() {
        let parsed = parse_natural_language("pdf files larger than 100mb last 30 days");
        assert!(parsed.query.min_size.unwrap() >= 100 * 1024 * 1024);
        assert_eq!(parsed.query.key_pattern.as_deref(), Some("%.pdf"));
        assert!(parsed.query.modified_after.is_some());
        assert_eq!(parsed.confidence, ParseConfidence::High);
    }

    #[test]
    fn parses_glacier_under_prefix() {
        let parsed = parse_natural_language("glacier under logs/");
        assert!(parsed.query.storage_class.is_some());
        assert_eq!(parsed.query.prefix.as_deref(), Some("logs/"));
    }

    #[test]
    fn fallback_substring_search() {
        let parsed = parse_natural_language("cat photos");
        assert_eq!(
            parsed.query.key_pattern.as_deref(),
            Some("%cat photos%")
        );
        assert_eq!(parsed.confidence, ParseConfidence::Low);
    }

    #[test]
    fn parses_glob_pattern() {
        let parsed = parse_natural_language("*.tmp");
        assert_eq!(parsed.query.key_pattern.as_deref(), Some("%.tmp"));
    }
}
