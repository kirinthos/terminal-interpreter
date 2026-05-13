use anyhow::{anyhow, bail, Context, Result};
use llm::builder::{LLMBackend, LLMBuilder};
use llm::chat::ChatMessage;

use crate::config::{Config, ProviderEnv};
use crate::shell::{self, ShellContext};

const DEFAULT_SYSTEM_PROMPT: &str = include_str!("prompts/default_system.md");

/// Build the prompt and route the request to the configured provider.
///
/// This is the only place that touches the `llm` crate; swap the routing
/// implementation here if the provider surface changes.
pub async fn generate_command(
    config: &Config,
    shell: &ShellContext,
    input: &str,
) -> Result<String> {
    let (provider, model) = config.provider_and_model()?;
    let system_prompt = config
        .system_prompt
        .as_deref()
        .unwrap_or(DEFAULT_SYSTEM_PROMPT);
    let user_prompt = build_user_prompt(shell, input, config.history_limit);

    let response = call_llm(provider, model, system_prompt, &user_prompt, config).await?;
    Ok(sanitize(response))
}

fn build_user_prompt(shell: &ShellContext, input: &str, history_limit: usize) -> String {
    let mut buf = String::new();
    buf.push_str(&format!("shell: {}\n", shell.kind.as_str()));
    buf.push_str(&format!("os: {}\n", shell.os));
    if let Some(cwd) = &shell.cwd {
        buf.push_str(&format!("cwd: {}\n", cwd.display()));
    }
    let history = shell::read_history(shell.kind, history_limit);
    if !history.is_empty() {
        buf.push_str("recent history:\n");
        for line in &history {
            buf.push_str("  ");
            buf.push_str(line);
            buf.push('\n');
        }
    }
    buf.push_str("\ncommand line so far:\n");
    buf.push_str(input);
    buf
}

/// Route the request through the `llm` crate. Provider, model, API key and
/// (optional) temperature come from the resolved `Config`; the system prompt
/// and rendered user prompt come from the caller.
async fn call_llm(
    provider: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    config: &Config,
) -> Result<String> {
    let provider_key = provider.to_ascii_lowercase();
    let backend = match provider_key.as_str() {
        "openai" => LLMBackend::OpenAI,
        "anthropic" => LLMBackend::Anthropic,
        "ollama" => LLMBackend::Ollama,
        other => bail!(
            "unsupported provider `{other}` — known: openai, anthropic, ollama"
        ),
    };

    let provider_env: Option<&ProviderEnv> = match provider_key.as_str() {
        "openai" => config.providers.openai.as_ref(),
        "anthropic" => config.providers.anthropic.as_ref(),
        "ollama" => config.providers.ollama.as_ref(),
        _ => None,
    };

    let mut builder = LLMBuilder::new()
        .backend(backend)
        .model(model)
        .system(system_prompt);

    if let Some(env) = provider_env {
        if let Some(key) = env.api_key.as_deref() {
            builder = builder.api_key(key);
        }
    }
    if let Some(t) = config.temperature {
        builder = builder.temperature(t);
    }

    let llm = builder
        .build()
        .map_err(|e| anyhow!("building {provider_key} client for {model}: {e}"))?;

    let messages = vec![ChatMessage::user().content(user_prompt).build()];
    let response = llm
        .chat(&messages)
        .await
        .map_err(|e| anyhow!("chat request to {provider_key}/{model}: {e}"))?;

    response
        .text()
        .context("LLM response had no text payload")
}

/// Strip code fences and leading/trailing whitespace; LLMs frequently wrap
/// single-line answers in ``` even when told not to.
fn sanitize(raw: String) -> String {
    let trimmed = raw.trim();
    let stripped = trimmed
        .strip_prefix("```")
        .map(|s| {
            s.trim_start_matches(|c: char| c.is_alphanumeric())
                .trim_start()
        })
        .and_then(|s| s.strip_suffix("```"))
        .unwrap_or(trimmed);
    stripped.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_fences() {
        assert_eq!(sanitize("```bash\nls -la\n```".into()), "ls -la");
        assert_eq!(sanitize("  ls\n".into()), "ls");
        assert_eq!(sanitize("```\necho hi\n```".into()), "echo hi");
    }

}
