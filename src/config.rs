//! Project configuration: `.stopproof.json` in the project root.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CONFIG_FILE: &str = ".stopproof.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
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

/// Missing configuration uses defaults. Existing invalid or unreadable files
/// are errors; the caller chooses whether to fail open (hook) or fail (CLI).
pub fn load(cwd: &Path) -> Result<Config, String> {
    let path = cwd.join(CONFIG_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err)
            if err.kind() == std::io::ErrorKind::NotFound
                && matches!(std::fs::symlink_metadata(&path), Err(missing) if missing.kind() == std::io::ErrorKind::NotFound) =>
        {
            return Ok(Config::default());
        }
        Err(err) => return Err(format!("cannot read {}: {}", path.display(), err)),
    };
    let cfg: Config = serde_json::from_str(&text)
        .map_err(|err| format!("invalid {}: {}", path.display(), err))?;
    cfg.validate()
        .map_err(|err| format!("invalid {}: {}", path.display(), err))?;
    Ok(cfg)
}

impl Config {
    fn validate(&self) -> Result<(), String> {
        if !matches!(self.mode.as_str(), "enforce" | "warn" | "off") {
            return Err("mode must be enforce, warn, or off".into());
        }
        if !matches!(self.verify_when.as_str(), "auto" | "always") {
            return Err("verify_when must be auto or always".into());
        }
        if self.test_timeout_secs == 0 {
            return Err("test_timeout_secs must be a positive integer".into());
        }
        if self.receipt_dir.trim().is_empty() {
            return Err("receipt_dir must not be empty".into());
        }
        Ok(())
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
        let c = load(Path::new("/definitely/not/a/real/dir")).unwrap();
        assert_eq!(c.mode, "enforce");
    }
}
