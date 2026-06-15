/// LLM-powered query parsing sub-module.
///
/// The `llm` Cargo feature gates the actual llama.cpp integration.
/// Without it, `try_load_model` returns `None` and `run_grammar_parse` returns
/// `Err`, so callers always fall back to the regex parser transparently.
pub mod gbnf_grammar;
pub mod model_paths;
pub mod model_runner;
pub mod model_status;
#[cfg(feature = "llm")]
mod model_runner_llm;
pub mod parse_result;

pub use model_paths::{models_dir_in, PARSER_MODEL_PRIMARY};
pub use model_runner::{run_grammar_parse, try_load_model, ModelHandle};
pub use model_status::{get_model_status, AssistantModelStatus};
pub use parse_result::{merge_with_regex, LlmParsedQuery, ParseSource};
