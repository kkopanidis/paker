/// Model runner for grammar-constrained LLM inference.
///
/// Without the `llm` Cargo feature this module is a zero-cost stub: callers fall
/// back to the regex parser. With `llm`, loads a user-dropped GGUF from
/// `{data_dir}/models/paker-parser.gguf` (see `model_paths`).

#[cfg(feature = "llm")]
pub use super::model_runner_llm::{run_grammar_parse, try_load_model, ModelHandle};

#[cfg(not(feature = "llm"))]
pub struct ModelHandle;

#[cfg(not(feature = "llm"))]
#[allow(unused_variables)]
pub fn try_load_model(_app_data_dir: &std::path::Path) -> Option<ModelHandle> {
    None
}

#[cfg(not(feature = "llm"))]
#[allow(unused_variables)]
pub fn run_grammar_parse(
    _handle: &ModelHandle,
    _text: &str,
    _grammar: &str,
) -> anyhow::Result<String> {
    Err(anyhow::anyhow!("llm feature not enabled"))
}
