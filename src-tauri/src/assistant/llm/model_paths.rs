use std::path::{Path, PathBuf};

/// Primary NL → `IndexQuery` parser model (Gemma 3 270M or Llama 3.2 1B Q4 GGUF).
pub const PARSER_MODEL_PRIMARY: &str = "paker-parser.gguf";

/// Alternate filenames users may drop in the models directory.
pub const PARSER_MODEL_ALIASES: &[&str] = &[
    "paker-parser.gguf",
    "gemma-3-270m-q4.gguf",
    "llama-3.2-1b-q4.gguf",
];

/// Future semantic key search (EmbeddingGemma 300M Q8).
pub const EMBED_MODEL_PRIMARY: &str = "paker-embed.gguf";

pub const EMBED_MODEL_ALIASES: &[&str] = &["paker-embed.gguf", "embeddinggemma-300m-q8.gguf"];

pub fn models_dir_in(base: &Path) -> PathBuf {
    base.join("models")
}

pub fn resolve_parser_model_path(base: &Path) -> Option<PathBuf> {
    resolve_first_existing(&models_dir_in(base), PARSER_MODEL_ALIASES)
}

pub fn resolve_embed_model_path(base: &Path) -> Option<PathBuf> {
    resolve_first_existing(&models_dir_in(base), EMBED_MODEL_ALIASES)
}

fn resolve_first_existing(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    for name in names {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolve_parser_prefers_primary_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        let models = dir.path().join("models");
        fs::create_dir_all(&models).expect("mkdir");
        fs::write(models.join(PARSER_MODEL_PRIMARY), b"x").expect("write");
        let found = resolve_parser_model_path(dir.path()).expect("found");
        assert_eq!(found.file_name().unwrap(), PARSER_MODEL_PRIMARY);
    }
}
