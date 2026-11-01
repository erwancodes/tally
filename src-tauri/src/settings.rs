use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct ProviderSettings {
    pub disabled: bool,
    /// Overrides the provider's default folder.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub providers: BTreeMap<String, ProviderSettings>,
    /// Cache tokens are always shown apart; this decides whether the headline totals count them.
    pub include_cache: bool,
    /// Notify past this many tokens today; `None` turns it off.
    pub alert_daily: Option<u64>,
    /// Notify past this many tokens this month; `None` turns it off.
    pub alert_monthly: Option<u64>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { providers: BTreeMap::new(), include_cache: true, alert_daily: None, alert_monthly: None }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Settings {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn provider(&self, id: &str) -> ProviderSettings {
        self.providers.get(id).cloned().unwrap_or_default()
    }

    pub fn roots(&self, id: &str, defaults: Vec<PathBuf>) -> Vec<PathBuf> {
        match self.provider(id).path.filter(|p| !p.trim().is_empty()) {
            Some(p) => vec![PathBuf::from(p.trim())],
            None => defaults,
        }
    }
}
