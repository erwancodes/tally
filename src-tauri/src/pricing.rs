//! Rough API list prices, to estimate what the tokens would cost.
//! Subscriptions bill differently, so this is an order of magnitude, not a bill.
//! Built-in prices can be overridden by a local table, downloaded only on request.

use crate::store::Totals;
use anyhow::{bail, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

const PRICES_URL: &str = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";
/// Providers billing directly; resellers like `bedrock/` have their own prices.
const DIRECT_PROVIDERS: &[&str] = &["anthropic", "openai", "moonshot", "gemini", "deepseek", "xai", "mistral"];
/// Fewer models than this means the file is not what we expect.
const MIN_MODELS: usize = 50;

/// $ per million tokens: input, output, cache read. Cache write is 1.25x input.
struct Price {
    prefix: &'static str,
    input: f64,
    output: f64,
    cache_read: f64,
}

const fn p(prefix: &'static str, input: f64, output: f64, cache_read: f64) -> Price {
    Price { prefix, input, output, cache_read }
}

// Most specific prefix first.
const PRICES: &[Price] = &[
    p("claude-fable-5-1", 10.0, 50.0, 0.25),
    p("claude-fable-5", 10.0, 50.0, 1.0),
    p("claude-opus-5-5", 4.0, 20.0, 0.2),
    p("claude-opus-5", 5.0, 25.0, 0.5),
    p("claude-opus-4-8", 5.0, 25.0, 0.5),
    p("claude-opus-4-7", 5.0, 25.0, 0.5),
    p("claude-opus-4-6", 5.0, 25.0, 0.5),
    p("claude-sonnet-5", 2.0, 10.0, 0.2),
    p("claude-sonnet-4-6", 3.0, 15.0, 0.3),
    p("claude-haiku-5-5", 0.1, 0.5, 0.01),
    p("claude-haiku-4-5", 1.0, 5.0, 0.1),
];

/// $ per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Rates {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Table {
    updated_ms: Option<i64>,
    rates: BTreeMap<String, Rates>,
}

static TABLE: RwLock<Table> = RwLock::new(Table { updated_ms: None, rates: BTreeMap::new() });

/// Loads the table saved by the last update, if any.
pub fn load(path: &Path) {
    let saved = std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok());
    if let Some(table) = saved {
        *TABLE.write() = table;
    }
}

/// When the prices were last downloaded.
pub fn updated_ms() -> Option<i64> {
    TABLE.read().updated_ms
}

/// Downloads the prices and saves them to `path`. On failure the current table stays.
pub async fn update(path: &Path) -> Result<i64> {
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build()?;
    let body = client.get(PRICES_URL).send().await?.error_for_status()?.bytes().await?;
    let rates = parse(&serde_json::from_slice(&body)?);
    if rates.len() < MIN_MODELS {
        bail!("unexpected price file");
    }
    let table = Table { updated_ms: Some(chrono::Utc::now().timestamp_millis()), rates };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string(&table)?)?;
    let updated = table.updated_ms.unwrap_or(0);
    *TABLE.write() = table;
    Ok(updated)
}

/// Keeps models priced by their own provider; the bare name wins over a prefixed one.
fn parse(json: &Value) -> BTreeMap<String, Rates> {
    let m = 1_000_000.0;
    let mut rates = BTreeMap::new();
    let Some(models) = json.as_object() else { return rates };
    for (key, v) in models {
        let key = key.to_ascii_lowercase();
        let (name, prefixed) = match key.split_once('/') {
            Some((provider, name)) if DIRECT_PROVIDERS.contains(&provider) => (name, true),
            Some(_) => continue,
            None => (key.as_str(), false),
        };
        let per_token = |field: &str| v.get(field).and_then(Value::as_f64);
        let (Some(input), Some(output)) = (per_token("input_cost_per_token"), per_token("output_cost_per_token")) else {
            continue;
        };
        if input <= 0.0 && output <= 0.0 {
            continue;
        }
        let r = Rates {
            input: input * m,
            output: output * m,
            cache_read: per_token("cache_read_input_token_cost").unwrap_or(input) * m,
            cache_write: per_token("cache_creation_input_token_cost").unwrap_or(input * 1.25) * m,
        };
        if prefixed {
            rates.entry(name.to_string()).or_insert(r);
        } else {
            rates.insert(name.to_string(), r);
        }
    }
    rates
}

/// Exact name, else the longest known name followed by a suffix like a date.
fn lookup(rates: &BTreeMap<String, Rates>, name: &str) -> Option<Rates> {
    if let Some(r) = rates.get(name) {
        return Some(*r);
    }
    rates
        .iter()
        .filter(|(k, _)| name.strip_prefix(k.as_str()).is_some_and(|rest| rest.starts_with('-')))
        .max_by_key(|(k, _)| k.len())
        .map(|(_, r)| *r)
}

/// Estimated cost in dollars, or None when the model has no known price.
pub fn cost(model: &str, t: &Totals) -> Option<f64> {
    // OpenCode and others may prefix the provider: "anthropic/claude-...".
    let name = model.rsplit('/').next().unwrap_or(model).to_ascii_lowercase();
    let rates = lookup(&TABLE.read().rates, &name).or_else(|| {
        PRICES.iter().find(|p| name.starts_with(p.prefix)).map(|p| Rates {
            input: p.input,
            output: p.output,
            cache_read: p.cache_read,
            cache_write: p.input * 1.25,
        })
    })?;
    let m = 1_000_000.0;
    Some(
        (t.input as f64 * rates.input
            + t.output as f64 * rates.output
            + t.cache_read as f64 * rates.cache_read
            + t.cache_write as f64 * rates.cache_write)
            / m,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices_known_models_by_prefix() {
        let t = Totals { input: 1_000_000, output: 1_000_000, cache_read: 1_000_000, cache_write: 1_000_000 };
        let near = |m: &str, want: f64| (cost(m, &t).unwrap() - want).abs() < 1e-9;
        assert!(near("claude-opus-5-5", 4.0 + 20.0 + 0.2 + 5.0));
        assert!(near("claude-fable-5-1-20261001", 10.0 + 50.0 + 0.25 + 12.5));
        assert!(near("anthropic/claude-sonnet-5-5", 2.0 + 10.0 + 0.2 + 2.5));
        assert_eq!(cost("gpt-6-sol", &t), None);
    }

    #[test]
    fn parses_direct_providers_only() {
        let rates = parse(&serde_json::json!({
            "sample_spec": { "input_cost_per_token": "x" },
            "bedrock/skipped": { "input_cost_per_token": 1e-6, "output_cost_per_token": 1e-6 },
            "free-model": { "input_cost_per_token": 0.0, "output_cost_per_token": 0.0 },
            "moonshot/kimi-k3": { "input_cost_per_token": 1e-6, "output_cost_per_token": 2e-6 },
            "openai/gpt-x": { "input_cost_per_token": 9e-6, "output_cost_per_token": 9e-6 },
            "gpt-x": {
                "input_cost_per_token": 1e-6,
                "output_cost_per_token": 2e-6,
                "cache_read_input_token_cost": 1e-7,
                "cache_creation_input_token_cost": 3e-6
            }
        }));
        assert_eq!(rates.len(), 2);
        let kimi = rates["kimi-k3"];
        assert!((kimi.cache_read - 1.0).abs() < 1e-9 && (kimi.cache_write - 1.25).abs() < 1e-9);
        let gpt = rates["gpt-x"];
        assert!((gpt.input - 1.0).abs() < 1e-9 && (gpt.cache_read - 0.1).abs() < 1e-9 && (gpt.cache_write - 3.0).abs() < 1e-9);
    }

    #[test]
    fn lookup_prefers_exact_then_longest_on_a_boundary() {
        let r = |input| Rates { input, output: 0.0, cache_read: 0.0, cache_write: 0.0 };
        let rates = BTreeMap::from([("gpt-6".to_string(), r(1.0)), ("gpt-6-sol".to_string(), r(2.0))]);
        assert_eq!(lookup(&rates, "gpt-6").unwrap().input, 1.0);
        assert_eq!(lookup(&rates, "gpt-6-sol-2026").unwrap().input, 2.0);
        assert_eq!(lookup(&rates, "gpt-6.1-terra"), None);
    }
}
