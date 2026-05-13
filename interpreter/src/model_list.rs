//! `--model-list`: print every known model from each supported provider in the
//! exact `provider/model-name` form expected by the config file, with live
//! pricing.
//!
//! Pricing comes from the LiteLLM model catalog
//! (<https://github.com/BerriAI/litellm>), which is the de-facto community
//! source of truth and is kept up to date as providers release new models.
//! OpenAI and Anthropic are always queried. Ollama is included only when a
//! local server is reachable.
//!
//! Synchronous on purpose — this is a one-shot CLI command, not a hot path,
//! and the rest of the binary's async surface (the `llm` crate) is irrelevant
//! here.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

const PRICING_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";
const OLLAMA_URL: &str = "http://localhost:11434/api/tags";
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const OLLAMA_TIMEOUT: Duration = Duration::from_millis(500);

/// Minimal projection of the LiteLLM pricing JSON. Every entry is keyed by
/// model id; only the fields we care about are deserialised, and unknowns are
/// ignored so the schema can grow without breaking us.
#[derive(Debug, Deserialize)]
struct LitellmEntry {
    #[serde(default)]
    litellm_provider: Option<String>,
    #[serde(default)]
    input_cost_per_token: Option<f64>,
    #[serde(default)]
    output_cost_per_token: Option<f64>,
    #[serde(default)]
    mode: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaTags {
    models: Vec<OllamaModel>,
}
#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub qualified: String,
    pub input_per_mtok: Option<f64>,
    pub output_per_mtok: Option<f64>,
    pub note: Option<&'static str>,
}

impl Row {
    /// Human-readable price column, matching the `--model-list` output.
    pub fn price_label(&self) -> String {
        match (self.note, self.input_per_mtok, self.output_per_mtok) {
            (Some(note), _, _) => note.to_string(),
            (None, Some(i), Some(o)) => format!("${i:>7.2} in / ${o:>7.2} out per 1M tok"),
            _ => "—".to_string(),
        }
    }
}

pub fn run() -> Result<()> {
    let rows = collect_rows()?;
    print_rows(&rows);
    Ok(())
}

/// Fetch model rows from the network. Always includes OpenAI + Anthropic;
/// includes Ollama only when a local server is reachable.
pub fn collect_rows() -> Result<Vec<Row>> {
    let pricing = fetch_litellm_pricing().context("fetching pricing catalog")?;
    let mut rows: Vec<Row> = Vec::new();
    rows.extend(collect_provider(&pricing, "openai"));
    rows.extend(collect_provider(&pricing, "anthropic"));
    rows.extend(collect_ollama());
    rows.sort_by(|a, b| a.qualified.cmp(&b.qualified));
    Ok(rows)
}

fn fetch_litellm_pricing() -> Result<BTreeMap<String, LitellmEntry>> {
    let agent = ureq::AgentBuilder::new().timeout(HTTP_TIMEOUT).build();
    let resp = agent
        .get(PRICING_URL)
        .call()
        .with_context(|| format!("GET {PRICING_URL}"))?;
    // Read as raw JSON first because the document contains a `sample_spec`
    // entry whose values are strings, which would fail typed deserialisation.
    let raw: serde_json::Value = resp.into_json().context("parsing pricing JSON")?;
    let obj = raw
        .as_object()
        .context("pricing payload was not a JSON object")?;
    let mut out = BTreeMap::new();
    for (key, value) in obj {
        if key == "sample_spec" {
            continue;
        }
        if let Ok(entry) = serde_json::from_value::<LitellmEntry>(value.clone()) {
            out.insert(key.clone(), entry);
        }
    }
    Ok(out)
}

fn collect_provider(pricing: &BTreeMap<String, LitellmEntry>, provider: &str) -> Vec<Row> {
    pricing
        .iter()
        .filter(|(_, e)| e.litellm_provider.as_deref() == Some(provider))
        // Only chat-style models can drive a shell-command rewriter.
        .filter(|(_, e)| {
            e.mode.as_deref().map(|m| m == "chat" || m == "responses").unwrap_or(true)
        })
        .map(|(id, e)| {
            // LiteLLM keys are usually bare ids (`gpt-4o-mini`), but a few
            // arrive prefixed (`anthropic/claude-3-5-sonnet`). Normalise so we
            // never emit `anthropic/anthropic/...`.
            let bare = id.strip_prefix(&format!("{provider}/")).unwrap_or(id);
            Row {
                qualified: format!("{provider}/{bare}"),
                input_per_mtok: e.input_cost_per_token.map(per_mtok),
                output_per_mtok: e.output_cost_per_token.map(per_mtok),
                note: None,
            }
        })
        .collect()
}

fn collect_ollama() -> Vec<Row> {
    let agent = ureq::AgentBuilder::new().timeout(OLLAMA_TIMEOUT).build();
    let resp = match agent.get(OLLAMA_URL).call() {
        Ok(r) => r,
        Err(_) => return Vec::new(), // no local server — silently skip
    };
    let parsed: OllamaTags = match resp.into_json() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    parsed
        .models
        .into_iter()
        .map(|m| Row {
            qualified: format!("ollama/{}", m.name),
            input_per_mtok: None,
            output_per_mtok: None,
            note: Some("(local)"),
        })
        .collect()
}

fn per_mtok(per_token: f64) -> f64 {
    per_token * 1_000_000.0
}

fn print_rows(rows: &[Row]) {
    if rows.is_empty() {
        eprintln!("no models found");
        return;
    }
    let col1 = rows.iter().map(|r| r.qualified.len()).max().unwrap_or(0);
    for row in rows {
        println!(
            "{q:<col1$}  {price}",
            q = row.qualified,
            col1 = col1,
            price = row.price_label()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(provider: &str, input: f64, output: f64, mode: &str) -> LitellmEntry {
        LitellmEntry {
            litellm_provider: Some(provider.to_string()),
            input_cost_per_token: Some(input),
            output_cost_per_token: Some(output),
            mode: Some(mode.to_string()),
        }
    }

    fn fixture() -> BTreeMap<String, LitellmEntry> {
        let mut m = BTreeMap::new();
        m.insert("gpt-4o-mini".into(), entry("openai", 1.5e-7, 6e-7, "chat"));
        m.insert(
            "claude-opus-4-7".into(),
            entry("anthropic", 5e-6, 25e-6, "chat"),
        );
        // Already-prefixed key — make sure we don't double-prefix.
        m.insert(
            "anthropic/claude-3-5-sonnet".into(),
            entry("anthropic", 3e-6, 15e-6, "chat"),
        );
        // Embedding model — should be filtered out by mode check.
        m.insert(
            "text-embedding-3-small".into(),
            entry("openai", 2e-8, 0.0, "embedding"),
        );
        m
    }

    #[test]
    fn collects_and_normalises_openai() {
        let rows = collect_provider(&fixture(), "openai");
        let ids: Vec<&str> = rows.iter().map(|r| r.qualified.as_str()).collect();
        assert_eq!(ids, vec!["openai/gpt-4o-mini"]);
        assert!((rows[0].input_per_mtok.unwrap() - 0.15).abs() < 1e-9);
        assert!((rows[0].output_per_mtok.unwrap() - 0.60).abs() < 1e-9);
    }

    #[test]
    fn collects_anthropic_and_strips_double_prefix() {
        let rows = collect_provider(&fixture(), "anthropic");
        let mut ids: Vec<&str> = rows.iter().map(|r| r.qualified.as_str()).collect();
        ids.sort();
        assert_eq!(
            ids,
            vec!["anthropic/claude-3-5-sonnet", "anthropic/claude-opus-4-7"]
        );
    }

    #[test]
    fn per_mtok_converts() {
        assert!((per_mtok(1.5e-7) - 0.15).abs() < 1e-9);
        assert!((per_mtok(25e-6) - 25.0).abs() < 1e-9);
    }
}
