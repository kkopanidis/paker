use crate::assistant::query::{IndexQuery, ParseConfidence, ParsedAssistantQuery};

#[derive(Debug, Clone)]
pub enum ParseSource {
    Llm,
    Regex,
}

#[derive(Debug, Clone)]
pub struct LlmParsedQuery {
    pub index_query: IndexQuery,
    pub source: ParseSource,
}

/// Merge an optional LLM result with the regex result.
///
/// Strategy: if an LLM result is available AND regex confidence is not `High`,
/// replace the query fields with LLM values (keeping the regex summary).
/// If the LLM result is `None`, return the regex result unchanged.
pub fn merge_with_regex(
    llm: Option<LlmParsedQuery>,
    regex: ParsedAssistantQuery,
) -> ParsedAssistantQuery {
    let Some(llm_result) = llm else {
        return regex;
    };

    if regex.confidence == ParseConfidence::High {
        return regex;
    }

    ParsedAssistantQuery {
        query: llm_result.index_query,
        summary: regex.summary,
        confidence: regex.confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::query::IndexQuery;

    fn make_regex(confidence: ParseConfidence, pattern: Option<&str>) -> ParsedAssistantQuery {
        ParsedAssistantQuery {
            query: IndexQuery {
                key_pattern: pattern.map(str::to_string),
                ..Default::default()
            },
            summary: "regex summary".to_string(),
            confidence,
        }
    }

    fn make_llm(pattern: &str) -> LlmParsedQuery {
        LlmParsedQuery {
            index_query: IndexQuery {
                key_pattern: Some(pattern.to_string()),
                ..Default::default()
            },
            source: ParseSource::Llm,
        }
    }

    #[test]
    fn merge_prefers_llm_when_regex_confidence_low() {
        let regex = make_regex(ParseConfidence::Low, Some("%old%"));
        let llm = make_llm("%llm%");
        let merged = merge_with_regex(Some(llm), regex);
        assert_eq!(merged.query.key_pattern.as_deref(), Some("%llm%"));
        assert_eq!(merged.summary, "regex summary");
    }

    #[test]
    fn merge_prefers_llm_when_regex_confidence_medium() {
        let regex = make_regex(ParseConfidence::Medium, Some("%old%"));
        let llm = make_llm("%llm%");
        let merged = merge_with_regex(Some(llm), regex);
        assert_eq!(merged.query.key_pattern.as_deref(), Some("%llm%"));
    }

    #[test]
    fn merge_keeps_regex_on_high_confidence() {
        let regex = make_regex(ParseConfidence::High, Some("%regex%"));
        let llm = make_llm("%llm%");
        let merged = merge_with_regex(Some(llm), regex);
        assert_eq!(merged.query.key_pattern.as_deref(), Some("%regex%"));
    }

    #[test]
    fn merge_returns_regex_when_no_llm() {
        let regex = make_regex(ParseConfidence::Low, Some("%regex%"));
        let merged = merge_with_regex(None, regex);
        assert_eq!(merged.query.key_pattern.as_deref(), Some("%regex%"));
    }
}
