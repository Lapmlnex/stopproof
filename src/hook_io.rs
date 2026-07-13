//! Claude Code hook I/O: stdin payload, stdout decisions, attempt state.

use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct HookInput {
    pub session_id: String,
    pub transcript_path: String,
    pub cwd: String,
    pub hook_event_name: String,
    pub stop_hook_active: bool,
}

pub fn parse_input(raw: &str) -> Option<HookInput> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let get = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    Some(HookInput {
        session_id: get("session_id"),
        transcript_path: get("transcript_path"),
        cwd: get("cwd"),
        hook_event_name: get("hook_event_name"),
        stop_hook_active: v
            .get("stop_hook_active")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

pub fn read_stdin_input() -> Option<HookInput> {
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw).ok()?;
    if raw.trim().is_empty() {
        return None;
    }
    parse_input(&raw)
}

fn state_file(cwd: &Path, receipt_dir: &str, session_id: &str) -> PathBuf {
    let safe: String = session_id
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    let name = if safe.is_empty() { "unknown".to_string() } else { safe };
    cwd.join(receipt_dir).join("state").join(format!("{}.attempts", name))
}

/// Number of times we've already blocked this session's Stop.
pub fn attempts_get(cwd: &Path, receipt_dir: &str, session_id: &str) -> u32 {
    std::fs::read_to_string(state_file(cwd, receipt_dir, session_id))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

pub fn attempts_set(cwd: &Path, receipt_dir: &str, session_id: &str, n: u32) {
    let path = state_file(cwd, receipt_dir, session_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(path, n.to_string()).ok();
}

pub fn attempts_reset(cwd: &Path, receipt_dir: &str, session_id: &str) {
    std::fs::remove_file(state_file(cwd, receipt_dir, session_id)).ok();
}

/// Block the Stop: Claude Code keeps the session going and shows `reason`
/// to the model so it can fix the problems.
pub fn emit_block(reason: &str) {
    let payload = json!({ "decision": "block", "reason": reason });
    println!("{}", payload);
}

/// Allow the Stop, optionally surfacing a message to the user.
pub fn emit_allow(system_message: Option<&str>) {
    if let Some(msg) = system_message {
        let payload = json!({ "systemMessage": msg });
        println!("{}", payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stop_payload() {
        let raw = r#"{
            "session_id": "abc-123",
            "transcript_path": "/tmp/t.jsonl",
            "cwd": "/repo",
            "hook_event_name": "Stop",
            "stop_hook_active": true
        }"#;
        let input = parse_input(raw).unwrap();
        assert_eq!(input.session_id, "abc-123");
        assert_eq!(input.hook_event_name, "Stop");
        assert!(input.stop_hook_active);
    }

    #[test]
    fn missing_fields_default() {
        let input = parse_input(r#"{"hook_event_name":"Stop"}"#).unwrap();
        assert_eq!(input.session_id, "");
        assert!(!input.stop_hook_active);
    }

    #[test]
    fn attempts_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "stopproof-state-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(attempts_get(&dir, ".stopproof", "s1"), 0);
        attempts_set(&dir, ".stopproof", "s1", 2);
        assert_eq!(attempts_get(&dir, ".stopproof", "s1"), 2);
        attempts_reset(&dir, ".stopproof", "s1");
        assert_eq!(attempts_get(&dir, ".stopproof", "s1"), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn weird_session_ids_are_sanitised() {
        let dir = std::env::temp_dir().join(format!(
            "stopproof-state2-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        attempts_set(&dir, ".stopproof", "../../evil", 1);
        assert_eq!(attempts_get(&dir, ".stopproof", "../../evil"), 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
