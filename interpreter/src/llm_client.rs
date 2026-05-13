use anyhow::Result;

use crate::config::Config;
use crate::shell::ShellContext;

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
    let history_start = shell.history.len().saturating_sub(history_limit);
    let history = &shell.history[history_start..];
    if !history.is_empty() {
        buf.push_str("recent history:\n");
        for line in history {
            buf.push_str("  ");
            buf.push_str(line);
            buf.push('\n');
        }
    }
    buf.push_str("\ncommand line so far:\n");
    buf.push_str(input);
    buf
}

/// Thin shim around the `llm` crate. Filled in once the provider routing API
/// is locked down; for now this is a structural placeholder that compiles and
/// fails loudly at runtime so the rest of the binary can be exercised.
async fn call_llm(
    provider: &str,
    model: &str,
    _system_prompt: &str,
    _user_prompt: &str,
    _config: &Config,
) -> Result<String> {
    // TODO: wire this to `llm::builder()` (or whichever entrypoint the chosen
    //       version of the crate exposes) using `_config.providers.{provider}`
    //       for API keys / base URLs and `_config.temperature` for sampling.
    anyhow::bail!(
        "LLM routing not yet implemented (requested {provider}/{model}). \
         Wire `llm_client::call_llm` to the `llm` crate."
    );
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

    // Silence unused-warning for `_` params on `call_llm` until it's wired up.
    #[allow(dead_code)]
    fn _ensure_call_llm_is_referenced() {
        let _ = call_llm;
    }
}
