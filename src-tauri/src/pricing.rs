//! Rough API list prices, to estimate what the tokens would cost.
//! Subscriptions bill differently, so this is an order of magnitude, not a bill.

use crate::store::Totals;

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

/// Estimated cost in dollars, or None when the model has no known price.
pub fn cost(model: &str, t: &Totals) -> Option<f64> {
    // OpenCode and others may prefix the provider: "anthropic/claude-...".
    let name = model.rsplit('/').next().unwrap_or(model).to_ascii_lowercase();
    let price = PRICES.iter().find(|p| name.starts_with(p.prefix))?;
    let m = 1_000_000.0;
    Some(
        (t.input as f64 * price.input
            + t.output as f64 * price.output
            + t.cache_read as f64 * price.cache_read
            + t.cache_write as f64 * price.input * 1.25)
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
}
