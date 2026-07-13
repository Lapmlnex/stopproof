//! Project configuration: `.stopproof.json` in the project root.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CONFIG_FILE: &str = ".stopproof.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// "enforce" (block stop on failure) | "warn" (report, never block) | "off"
    pub mode: String,
    /// Explicit test command. Empty string = auto-detect.
    pub test_command: String,
    /// Kill the test run after this many seconds.
    pub test_timeout_secs: u64,
    /// How many times stopproof may block a Stop before letting the
    /// session end anyway (prevents infinite loops).
    pub max_attempts: u32,
    /// "auto" = verify when the session edited files, or when the agent
    /// claims completion over a dirty tree. "always" = verify every stop.
    pub verify_when: String,
    /// Fail verification when no test command can be detected.
    pub require_tests: bool,
    /// Extra substrings that count as a completion claim in the final message.
    pub claim_keywords: Vec<String>,
    /// Path substrings excluded from diff/claim reconciliation.
    pub ignore_paths: Vec<String>,
    /// Directory for receipts and per-session state.
    pub receipt_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            mode: "enforce".to_string(),
            test_command: String::new(),
            test_timeout_secs: 300,
            max_attempts: 2,
            verify_when: "auto".to_string(),
            require_tests: false,
            claim_keywords: Vec::new(),
            ignore_paths: vec![".stopproof/".to_string()],
            receipt_dir: ".stopproof".to_string(),
        }
    }
}

/// Load config from `<cwd>/.stopproof.json`. Missing file or parse errors
/// fall back to defaults (a broken config must never brick a session).
pub fn load(cwd: &Path) -> Config {
    let path = cwd.join(CONFIG_FILE);
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Config>(&text) {
            Ok(cfg) => cfg,
            Err(err) => {
                eprintln!("stopproof: ignoring invalid {}: {}", CONFIG_FILE, err);
                Config::default()
            }
        },
        Err(_) => Config::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let c = Config::default();
        assert_eq!(c.mode, "enforce");
        assert_eq!(c.max_attempts, 2);
        assert_eq!(c.verify_when, "auto");
        assert!(!c.require_tests);
    }

    #[test]
    fn partial_config_merges_with_defaults() {
        let c: Config = serde_json::from_str(r#"{"mode":"warn"}"#).unwrap();
        assert_eq!(c.mode, "warn");
        assert_eq!(c.test_timeout_secs, 300);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let c = load(Path::new("/definitely/not/a/real/dir"));
        assert_eq!(c.mode, "enforce");
    }
}
