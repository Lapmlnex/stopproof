//! Verification receipts: auditable evidence of what was checked.

use crate::config::Config;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Clone)]
pub struct Check {
    pub name: String,
    /// "pass" | "fail" | "warn" | "info"
    pub status: String,
    pub detail: String,
}

impl Check {
    pub fn new(name: &str, status: &str, detail: String) -> Self {
        Check {
            name: name.to_string(),
            status: status.to_string(),
            detail,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Receipt {
    pub tool: String,
    pub version: String,
    pub verdict: String,
    pub session_id: String,
    pub generated_at: String,
    pub attempt: u32,
    pub checks: Vec<Check>,
    pub edited_files: Vec<String>,
    pub claimed_paths: Vec<String>,
    pub git_changed: Vec<String>,
    pub test_command: Option<String>,
    pub test_exit: Option<i32>,
    pub test_output_tail: String,
}

fn status_icon(status: &str) -> &'static str {
    match status {
        "pass" => "[PASS]",
        "fail" => "[FAIL]",
        "warn" => "[WARN]",
        _ => "[INFO]",
    }
}

pub fn to_markdown(r: &Receipt) -> String {
    let mut md = String::new();
    md.push_str(&format!(
        "# stopproof receipt — {}\n\n",
        r.verdict.to_uppercase()
    ));
    md.push_str(&format!(
        "- generated: {}\n- session: `{}`\n- attempt: {}\n- stopproof: v{}\n\n",
        r.generated_at, r.session_id, r.attempt, r.version
    ));
    md.push_str("## Checks\n\n");
    for c in &r.checks {
        md.push_str(&format!("- {} **{}** — {}\n", status_icon(&c.status), c.name, c.detail));
    }
    md.push_str("\n## Evidence\n\n");
    if !r.edited_files.is_empty() {
        md.push_str(&format!(
            "- files edited in session: {}\n",
            r.edited_files.join(", ")
        ));
    }
    if !r.claimed_paths.is_empty() {
        md.push_str(&format!(
            "- files referenced in final message: {}\n",
            r.claimed_paths.join(", ")
        ));
    }
    if !r.git_changed.is_empty() {
        md.push_str(&format!("- git changes: {}\n", r.git_changed.join(", ")));
    }
    match (&r.test_command, r.test_exit) {
        (Some(cmd), Some(code)) => {
            md.push_str(&format!("- test command: `{}` (exit {})\n", cmd, code))
        }
        (Some(cmd), None) => md.push_str(&format!("- test command: `{}` (no exit code)\n", cmd)),
        (None, _) => md.push_str("- test command: none detected\n"),
    }
    if !r.test_output_tail.trim().is_empty() {
        md.push_str(&format!(
            "\n<details><summary>test output (tail)</summary>\n\n```\n{}\n```\n\n</details>\n",
            r.test_output_tail
        ));
    }
    md
}

/// Write JSON + Markdown receipts. Returns the JSON path on success.
pub fn write(cwd: &Path, cfg: &Config, receipt: &Receipt) -> Option<PathBuf> {
    let dir = cwd.join(&cfg.receipt_dir);
    let receipts_dir = dir.join("receipts");
    std::fs::create_dir_all(&receipts_dir).ok()?;

    let stamp = crate::timefmt::compact_ts(crate::timefmt::now_epoch_secs());
    let json_path = receipts_dir.join(format!(
        "{}-a{}-p{}-{}.json",
        stamp,
        receipt.attempt,
        std::process::id(),
        receipt.verdict
    ));
    let json = serde_json::to_string_pretty(receipt).ok()?;
    std::fs::write(&json_path, json).ok()?;
    std::fs::write(dir.join("last-receipt.md"), to_markdown(receipt)).ok()?;
    Some(json_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Receipt {
        Receipt {
            tool: "stopproof".into(),
            version: "0.1.0".into(),
            verdict: "fail".into(),
            session_id: "abc".into(),
            generated_at: "2026-07-12T00:00:00Z".into(),
            attempt: 1,
            checks: vec![
                Check::new("tests", "fail", "`cargo test` exited 101".into()),
                Check::new("diff-reconciliation", "pass", "all claims backed by git".into()),
            ],
            edited_files: vec!["src/a.rs".into()],
            claimed_paths: vec!["src/a.rs".into()],
            git_changed: vec!["src/a.rs".into()],
            test_command: Some("cargo test".into()),
            test_exit: Some(101),
            test_output_tail: "1 test failed".into(),
        }
    }

    #[test]
    fn markdown_contains_key_facts() {
        let md = to_markdown(&sample());
        assert!(md.contains("FAIL"));
        assert!(md.contains("cargo test"));
        assert!(md.contains("[FAIL] **tests**"));
        assert!(md.contains("1 test failed"));
    }

    #[test]
    fn writes_both_files() {
        let dir = std::env::temp_dir().join(format!(
            "stopproof-receipt-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
        .to_string_lossy()
        .replace(['(', ')', ' '], "-");
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = Config::default();
        let path = write(&dir, &cfg, &sample()).expect("receipt written");
        assert!(path.exists());
        assert!(dir.join(".stopproof/last-receipt.md").exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
