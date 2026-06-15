use super::model_paths::{
    models_dir_in, resolve_embed_model_path, resolve_parser_model_path, EMBED_MODEL_PRIMARY,
    PARSER_MODEL_PRIMARY,
};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantModelStatus {
    pub llm_feature_enabled: bool,
    pub parser_loaded: bool,
    pub models_dir: String,
    pub parser_path: Option<String>,
    pub parser_present: bool,
    pub parser_recommended_filename: String,
    pub embed_path: Option<String>,
    pub embed_present: bool,
    pub embed_recommended_filename: String,
    pub hint: String,
}

pub fn get_model_status(data_dir: &Path, parser_loaded: bool) -> AssistantModelStatus {
    let models_dir = models_dir_in(data_dir);
    let parser_path = resolve_parser_model_path(data_dir);
    let embed_path = resolve_embed_model_path(data_dir);

    let llm_feature_enabled = cfg!(feature = "llm");
    let parser_present = parser_path.is_some();
    let embed_present = embed_path.is_some();

    let hint = if !llm_feature_enabled {
        "Rebuild Paker with --features llm to enable on-device parsing fallback.".to_string()
    } else if !parser_present {
        format!(
            "Drop a GGUF parser model as {PARSER_MODEL_PRIMARY} in the models folder (e.g. Gemma 3 270M or Llama 3.2 1B Q4)."
        )
    } else if parser_loaded {
        "Parser model loaded — LLM fallback is active for low-confidence queries.".to_string()
    } else {
        "Parser model file found but not loaded — restart Paker after adding the model.".to_string()
    };

    AssistantModelStatus {
        llm_feature_enabled,
        parser_loaded,
        models_dir: models_dir.to_string_lossy().into_owned(),
        parser_path: parser_path.map(|p| p.to_string_lossy().into_owned()),
        parser_present,
        parser_recommended_filename: PARSER_MODEL_PRIMARY.to_string(),
        embed_path: embed_path.map(|p| p.to_string_lossy().into_owned()),
        embed_present,
        embed_recommended_filename: EMBED_MODEL_PRIMARY.to_string(),
        hint,
    }
}
