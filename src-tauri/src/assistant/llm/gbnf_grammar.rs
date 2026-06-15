/// GBNF grammar that constrains LLM output to a strict JSON schema mapping
/// 1-to-1 with `IndexQuery`.  This prevents the model from producing free-form
/// text; every token must come from a valid-continuation set.
///
/// The grammar is used only when the `llm` feature is enabled.  Without the
/// feature, this constant is still compiled so that the grammar itself can be
/// unit-tested independently of the model.
pub const INDEX_QUERY_GBNF: &str = r#"
root    ::= obj | "{" ws "}"

obj     ::= "{" ws field-list ws "}"
field-list ::= field (ws "," ws field)*
             | ""

field ::= key-pattern-field
        | prefix-field
        | min-size-field
        | max-size-field
        | modified-after-field
        | modified-before-field
        | storage-class-field

key-pattern-field    ::= "\"keyPattern\""    ws ":" ws (string | null)
prefix-field         ::= "\"prefix\""        ws ":" ws (string | null)
min-size-field       ::= "\"minSize\""       ws ":" ws (uint64 | null)
max-size-field       ::= "\"maxSize\""       ws ":" ws (uint64 | null)
modified-after-field ::= "\"modifiedAfter\"" ws ":" ws (iso8601-string | null)
modified-before-field ::= "\"modifiedBefore\"" ws ":" ws (iso8601-string | null)
storage-class-field  ::= "\"storageClass\""  ws ":" ws (sc-array | null)

sc-array  ::= "[" ws "]"
            | "[" ws sc-value (ws "," ws sc-value)* ws "]"
sc-value  ::= "\"STANDARD\""
            | "\"STANDARD_IA\""
            | "\"ONEZONE_IA\""
            | "\"INTELLIGENT_TIERING\""
            | "\"GLACIER\""
            | "\"GLACIER_IR\""
            | "\"DEEP_ARCHIVE\""

string       ::= "\"" char* "\""
char         ::= [^"\\] | "\\" escape
escape       ::= ["\\bfnrt] | "u" [0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F]
uint64       ::= [1-9][0-9]* | "0"
iso8601-string ::= "\"" [0-9][0-9][0-9][0-9] "-" [0-9][0-9] "-" [0-9][0-9]
                   ("T" [0-9][0-9] ":" [0-9][0-9] ":" [0-9][0-9] "Z")? "\""
null         ::= "null"
ws           ::= [ \t\n\r]*
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_constant_is_non_empty() {
        assert!(!INDEX_QUERY_GBNF.trim().is_empty());
        assert!(INDEX_QUERY_GBNF.contains("root"));
        assert!(INDEX_QUERY_GBNF.contains("GLACIER"));
        assert!(INDEX_QUERY_GBNF.contains("keyPattern"));
    }
}
