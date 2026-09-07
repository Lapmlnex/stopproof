//! Best-effort parsing of the Claude Code session transcript (JSONL).
//!
//! Transcript formats have drifted across Claude Code versions, so this
//! module is deliberately defensive: it recognises multiple known shapes,
//! skips lines it can't parse, and never fails hard.

use serde_json::Value;
use std::path::Path;

const EDIT_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];

#[derive(Debug, Default)]
pub struct TranscriptFacts {
    /// Files touched via file-editing tools this session (normalised,
    /// deduplicated, in first-seen order). Includes subagent edits.
    pub edited_files: Vec<String>,
    /// Text of the last main-chain assistant message.
    pub final_text: String,
    /// Timestamp of the first transcript entry, if parseable.
    pub session_start_epoch: Option<u64>,
    /// Number of lines successfully parsed.
    pub entries: usize,
}

/// Compare two normalised paths, tolerating absolute-vs-relative mismatches.
pub fn paths_match(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    a.ends_with(&format!("/{}", b)) || b.ends_with(&format!("/{}", a))
}

pub fn any_path_match(list: &[String], p: &str) -> bool {
    list.iter().any(|x| paths_match(x, p))
}

fn normalise_path(raw: &str, cwd: &Path) -> String {
    let p = Path::new(raw);
    let stripped = p.strip_prefix(cwd).unwrap_or(p);
    let mut s = stripped.to_string_lossy().replace('\\', "/");
    if let Some(rest) = s.strip_prefix("./") {
        s = rest.to_string();
    }
    s
}

fn record_edit(name: &str, input: &Value, cwd: &Path, out: &mut Vec<String>) {
    if !EDIT_TOOLS.contains(&name) {
        return;
    }
    let raw = input
        .get("file_path")
        .and_then(Value::as_str)
        .or_else(|| input.get("notebook_path").and_then(Value::as_str));
    if let Some(raw) = raw {
        let norm = normalise_path(raw, cwd);
        if !norm.is_empty() && !out.iter().any(|e| e == &norm) {
            out.push(norm);
        }
    }
}

/// Extract text and tool_use blocks from an assistant `content` value,
/// which may be a plain string or an array of typed blocks.
fn scan_content(content: &Value, cwd: &Path, edits: &mut Vec<String>) -> String {
    let mut text = String::new();
    match content {
        Value::String(s) => text.push_str(s),
        Value::Array(blocks) => {
            for b in blocks {
                match b.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(t) = b.get("text").and_then(Value::as_str) {
                            if !text.is_empty() {
                                text.push('\n');
                            }
                            text.push_str(t);
                        }
                    }
                    Some("tool_use") => {
                        let name = b.get("name").and_then(Value::as_str).unwrap_or("");
                        let default_input = Value::Null;
                        let input = b.get("input").unwrap_or(&default_input);
                        record_edit(name, input, cwd, edits);
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    text
}

pub fn analyze(transcript_path: &Path, cwd: &Path) -> TranscriptFacts {
    let mut facts = TranscriptFacts::default();
    let raw = match std::fs::read_to_string(transcript_path) {
        Ok(r) => r,
        Err(_) => return facts,
    };

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        facts.entries += 1;

        if facts.session_start_epoch.is_none() {
            if let Some(ts) = v.get("timestamp").and_then(Value::as_str) {
                facts.session_start_epoch = crate::timefmt::parse_iso_to_epoch(ts);
            }
        }

        let is_sidechain = v.get("isSidechain").and_then(Value::as_bool) == Some(true);
        let entry_type = v.get("type").and_then(Value::as_str).unwrap_or("");

        // Shape A: {"type":"assistant","message":{"role":"assistant","content":[...]}}
        // Shape B: {"type":"message","role":"assistant","content":[...]}
        //          or {"role":"assistant","content":[...]}
        let (role, content) = if let Some(msg) = v.get("message") {
            (
                msg.get("role")
                    .and_then(Value::as_str)
                    .unwrap_or(entry_type),
                msg.get("content"),
            )
        } else {
            (
                v.get("role").and_then(Value::as_str).unwrap_or(entry_type),
                v.get("content"),
            )
        };

        if role == "assistant" || entry_type == "assistant" {
            if let Some(content) = content {
                let text = scan_content(content, cwd, &mut facts.edited_files);
                if !text.trim().is_empty() && !is_sidechain {
                    facts.final_text = text;
                }
            }
        }

        // Shape C: standalone tool_use lines:
        // {"type":"tool_use","tool_name":"Edit","tool_input":{...}}
        if entry_type == "tool_use" {
            let name = v
                .get("tool_name")
                .and_then(Value::as_str)
                .or_else(|| v.get("name").and_then(Value::as_str))
                .unwrap_or("");
            let default_input = Value::Null;
            let input = v
                .get("tool_input")
                .or_else(|| v.get("input"))
                .unwrap_or(&default_input);
            record_edit(name, input, cwd, &mut facts.edited_files);
        }
    }

    facts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    fn write_lines(lines: &[&str]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "stopproof-transcript-{}-{}.jsonl",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let mut f = std::fs::File::create(&p).unwrap();
        for l in lines {
            writeln!(f, "{}", l).unwrap();
        }
        p
    }

    #[test]
    fn parses_shape_a_nested_message() {
        let p = write_lines(&[
            r#"{"type":"user","timestamp":"2026-07-12T09:00:00Z","message":{"role":"user","content":"please fix"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Edit","input":{"file_path":"src/lib.rs"}}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Done, fixed src/lib.rs"}]}}"#,
        ]);
        let facts = analyze(&p, Path::new("/repo"));
        std::fs::remove_file(&p).ok();
        assert_eq!(facts.edited_files, vec!["src/lib.rs".to_string()]);
        assert_eq!(facts.final_text, "Done, fixed src/lib.rs");
        assert!(facts.session_start_epoch.is_some());
        assert_eq!(facts.entries, 3);
    }

    #[test]
    fn parses_shape_b_and_standalone_tool_use() {
        let p = write_lines(&[
            r#"{"type":"message","role":"assistant","content":[{"type":"text","text":"working on it"}]}"#,
            r#"{"type":"tool_use","tool_name":"Write","tool_input":{"file_path":"/repo/a.py"}}"#,
            r#"{"type":"message","role":"assistant","content":[{"type":"text","text":"All done."}]}"#,
        ]);
        let facts = analyze(&p, Path::new("/repo"));
        std::fs::remove_file(&p).ok();
        assert_eq!(facts.edited_files, vec!["a.py".to_string()]);
        assert_eq!(facts.final_text, "All done.");
    }

    #[test]
    fn sidechain_text_does_not_become_final_text() {
        let p = write_lines(&[
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"main answer"}]}}"#,
            r#"{"type":"assistant","isSidechain":true,"message":{"role":"assistant","content":[{"type":"text","text":"subagent chatter"}]}}"#,
        ]);
        let facts = analyze(&p, Path::new("/repo"));
        std::fs::remove_file(&p).ok();
        assert_eq!(facts.final_text, "main answer");
    }

    #[test]
    fn garbage_lines_are_skipped() {
        let p = write_lines(&["not json at all", "{\"type\":\"summary\"}"]);
        let facts = analyze(&p, Path::new("/repo"));
        std::fs::remove_file(&p).ok();
        assert_eq!(facts.entries, 1);
        assert!(facts.edited_files.is_empty());
    }

    #[test]
    fn missing_file_is_empty_facts() {
        let facts = analyze(Path::new("/no/such/transcript.jsonl"), Path::new("/repo"));
        assert_eq!(facts.entries, 0);
    }

    #[test]
    fn path_matching() {
        assert!(paths_match("src/a.rs", "src/a.rs"));
        assert!(paths_match("/repo/src/a.rs", "src/a.rs"));
        assert!(paths_match("src/a.rs", "/repo/src/a.rs"));
        assert!(!paths_match("src/a.rs", "src/b.rs"));
        assert!(!paths_match("a.rs", "aa.rs"));
    }
}
