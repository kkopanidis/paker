use super::model_paths::resolve_parser_model_path;
use anyhow::Context;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::AddBos;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::Special;
use llama_cpp_2::sampling::LlamaSampler;
use parking_lot::Mutex;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;

const MAX_PARSE_TOKENS: u32 = 256;

pub struct LoadedModel {
    backend: LlamaBackend,
    model: LlamaModel,
}

impl LoadedModel {
    pub fn load(app_data_dir: &Path) -> anyhow::Result<Self> {
        let model_path = resolve_parser_model_path(app_data_dir)
            .context("parser GGUF not found in models directory")?;

        let backend = LlamaBackend::init().context("failed to init llama backend")?;
        let model_params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, &model_path, &model_params)
            .with_context(|| format!("failed to load model {}", model_path.display()))?;

        Ok(Self { backend, model })
    }

    pub fn run_grammar_parse(&self, text: &str, grammar: &str) -> anyhow::Result<String> {
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(2048))
            .with_n_batch(512);
        let mut ctx = self
            .model
            .new_context(&self.backend, ctx_params)
            .context("failed to create llama context")?;

        let prompt = format!(
            "Convert the following natural-language S3 bucket search request into IndexQuery JSON only.\n\
             Request: {text}\n\
             JSON:"
        );

        let tokens = self
            .model
            .str_to_token(&prompt, AddBos::Always)
            .context("failed to tokenize prompt")?;

        let mut batch = llama_cpp_2::llama_batch::LlamaBatch::new(512, 1);
        for (i, token) in tokens.iter().enumerate() {
            batch.add(*token, i as i32, &[0], false)?;
        }
        batch.add(self.model.token_eos(), tokens.len() as i32, &[0], true)?;
        ctx.decode(&mut batch).context("failed to decode prompt")?;

        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::grammar(&self.model, grammar, "root").context("grammar init failed")?,
            LlamaSampler::dist(0),
            LlamaSampler::greedy(),
        ]);

        let mut out = String::new();
        for _ in 0..MAX_PARSE_TOKENS {
            let token = sampler.sample(&ctx, 0);
            if self.model.is_eog_token(token) {
                break;
            }
            let piece = self
                .model
                .token_to_str(token, Special::Tokenize)
                .context("failed to decode token")?;
            out.push_str(piece);
            sampler.accept(token);
            let mut next = llama_cpp_2::llama_batch::LlamaBatch::new(1, 1);
            next.add(token, 0, &[0], true)?;
            ctx.decode(&mut next).context("failed to decode token")?;
        }

        let trimmed = out.trim();
        if trimmed.is_empty() {
            anyhow::bail!("model produced empty output");
        }
        Ok(trimmed.to_string())
    }
}

pub struct ModelHandle {
    inner: Arc<Mutex<LoadedModel>>,
}

impl ModelHandle {
    pub fn from_loaded(loaded: LoadedModel) -> Self {
        Self {
            inner: Arc::new(Mutex::new(loaded)),
        }
    }

    pub fn run_grammar_parse(&self, text: &str, grammar: &str) -> anyhow::Result<String> {
        self.inner.lock().run_grammar_parse(text, grammar)
    }
}

pub fn try_load_model(app_data_dir: &Path) -> Option<ModelHandle> {
    match LoadedModel::load(app_data_dir) {
        Ok(loaded) => {
            tracing::info!("assistant parser model loaded");
            Some(ModelHandle::from_loaded(loaded))
        }
        Err(e) => {
            tracing::debug!(error = %e, "parser model not loaded");
            None
        }
    }
}

pub fn run_grammar_parse(handle: &ModelHandle, text: &str, grammar: &str) -> anyhow::Result<String> {
    handle.run_grammar_parse(text, grammar)
}
