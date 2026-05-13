use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli::Cli;

const DEFAULT_HISTORY_LIMIT: usize = 50;

/// On-disk configuration. Loaded from JSON, then mutated by CLI overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Default model in `provider/model-name` form.
    #[serde(default = "default_model")]
    pub model: String,

    /// How many lines of shell history to read off the tail of the history
    /// file and include in the prompt. Read fresh on every invocation.
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,

    /// Sampling temperature, if the provider supports it.
    #[serde(default)]
    pub temperature: Option<f32>,

    /// Optional system prompt override.
    #[serde(default)]
    pub system_prompt: Option<String>,

    /// Per-provider environment settings (API keys, base URLs, etc.).
    #[serde(default)]
    pub providers: ProviderSettings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderSettings {
    #[serde(default)]
    pub openai: Option<ProviderEnv>,
    #[serde(default)]
    pub anthropic: Option<ProviderEnv>,
    #[serde(default)]
    pub ollama: Option<ProviderEnv>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderEnv {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: default_model(),
            history_limit: default_history_limit(),
            temperature: None,
            system_prompt: None,
            providers: ProviderSettings::default(),
        }
    }
}

impl Config {
    /// Load from an explicit path, or the platform default config location.
    /// A missing file falls back to defaults.
    pub fn load(explicit: Option<&Path>) -> Result<Self> {
        let path = match explicit {
            Some(p) => Some(p.to_path_buf()),
            None => default_config_path(),
        };

        let Some(path) = path else {
            return Ok(Self::default());
        };
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config {}", path.display()))?;
        serde_json::from_str(&raw).with_context(|| format!("parsing config {}", path.display()))
    }

    pub fn with_overrides(mut self, cli: &Cli) -> Self {
        if let Some(model) = cli.model.as_deref() {
            self.model = model.to_string();
        }
        self
    }

    /// Split `provider/model-name` into its two halves.
    pub fn provider_and_model(&self) -> Result<(&str, &str)> {
        self.model
            .split_once('/')
            .context("`model` must be in `provider/model-name` form")
    }
}

fn default_model() -> String {
    "openai/gpt-4o-mini".to_string()
}

fn default_history_limit() -> usize {
    DEFAULT_HISTORY_LIMIT
}

pub fn default_config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "interpreter")
        .map(|dirs| dirs.config_dir().join("config.json"))
}

impl Config {
    /// Write the config as pretty-printed JSON, creating parent dirs as needed.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
        }
        let body = serde_json::to_string_pretty(self).context("serializing config")?;
        std::fs::write(path, body).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_json_is_empty_object() {
        let cfg: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg.model, default_model());
        assert_eq!(cfg.history_limit, DEFAULT_HISTORY_LIMIT);
        assert!(cfg.temperature.is_none());
    }

    #[test]
    fn parses_full_config() {
        let raw = r#"{
            "model": "anthropic/claude-opus-4-7",
            "history_limit": 25,
            "temperature": 0.2,
            "system_prompt": "be terse",
            "providers": {
                "anthropic": { "api_key": "sk-..." }
            }
        }"#;
        let cfg: Config = serde_json::from_str(raw).unwrap();
        assert_eq!(cfg.model, "anthropic/claude-opus-4-7");
        assert_eq!(cfg.history_limit, 25);
        assert_eq!(cfg.temperature, Some(0.2));
        assert_eq!(cfg.system_prompt.as_deref(), Some("be terse"));
        assert_eq!(
            cfg.providers.anthropic.as_ref().and_then(|p| p.api_key.as_deref()),
            Some("sk-...")
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        let err = serde_json::from_str::<Config>(r#"{"nope": 1}"#).unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }
}
