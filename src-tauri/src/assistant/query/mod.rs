mod index_query;
mod rule_parser;

pub use index_query::{describe_index_query, IndexQuery};
pub use rule_parser::{parse_natural_language, ParseConfidence, ParsedAssistantQuery};
