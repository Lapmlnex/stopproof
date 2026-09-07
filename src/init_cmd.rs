//! `stopproof init` — install the Stop hook and default config.

use crate::config::{Config, CONFIG_FILE};
use serde_json::{json, Value};
use std::path::Path;

fn hook_entry() -> Value {
    json!({
        "hooks": [
            {
                "type": "command",
                "command": "stopproof",
                "timeout": 600
            }
        ]
    })
}

fn settings_snippet() -> String {
    serde_json::to_string_pretty(&json!({ "hooks": { "Stop": [hook_entry()] } }))
        .unwrap_or_default()
}

/// Check the structures we preserve while merging. This is not a complete
/// Claude settings schema: unrelated settings and optional hook fields remain
/// untouched, but malformed hook containers and required payloads are errors.
fn validate_settings(root: &Value) -> Result<(), String> {
    if !root.is_object() {
        return Err("expected a settings object".into());
    }
    let Some(hooks) = root.get("hooks") else {
        return Ok(());
    };
    let hooks = hooks.as_object().ok_or("hooks must be an object")?;
    for (event, entries) in hooks {
        let entries = entries
            .as_array()
            .ok_or_else(|| format!("hooks.{} must be an array", event))?;
        for (entry_index, entry) in entries.iter().enumerate() {
            let entry_path = format!("hooks.{}[{}]", event, entry_index);
            if !entry.is_object() {
                return Err(format!("{} must be an object", entry_path));
            }
            let handlers = entry
                .get("hooks")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("{}.hooks must be an array", entry_path))?;
            for (handler_index, handler) in handlers.iter().enumerate() {
                let handler_path = format!("{}.hooks[{}]", entry_path, handler_index);
                if !handler.is_object() {
                    return Err(format!("{} must be an object", handler_path));
                }
                let kind = handler
                    .get("type")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("{}.type must be a string", handler_path))?;
                let required: &[&str] = match kind {
                    "command" => &["command"],
                    "prompt" | "agent" => &["prompt"],
                    "http" => &["url"],
                    "mcp_tool" => &["server", "tool"],
                    _ => return Err(format!("{}.type is unsupported: {}", handler_path, kind)),
                };
                for field in required {
                    if handler.get(*field).and_then(Value::as_str).is_none() {
                        return Err(format!("{}.{} must be a string", handler_path, field));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Merge our Stop hook into an existing settings JSON value.
/// Returns true if the value was modified.
fn merge_into_settings(root: &mut Value) -> bool {
    if !root.is_object() {
        *root = json!({});
    }
    let obj = root.as_object_mut().expect("checked above");
    let hooks = obj.entry("hooks".to_string()).or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let stop = hooks
        .as_object_mut()
        .expect("checked above")
        .entry("Stop".to_string())
        .or_insert_with(|| json!([]));
    if !stop.is_array() {
        *stop = json!([]);
    }
    let arr = stop.as_array_mut().expect("checked above");

    let already = arr.iter().any(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .map(|hs| {
                hs.iter().any(|h| {
                    h.get("command")
                        .and_then(Value::as_str)
                        .map(|c| c.contains("stopproof"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    });
    if already {
        return false;
    }
    arr.push(hook_entry());
    true
}

fn ensure_gitignore(cwd: &Path, receipt_dir: &str) -> std::io::Result<()> {
    let path = cwd.join(".gitignore");
    let line = format!("{}/", receipt_dir.trim_end_matches('/'));
    let current = match std::fs::read_to_string(&path) {
        Ok(current) => current,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };
    if current.lines().any(|l| l.trim() == line) {
        return Ok(());
    }
    let mut next = current;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(&line);
    next.push('\n');
    std::fs::write(&path, next)
}

pub fn run(args: &[String]) -> i32 {
    let print_only = args.len() == 1 && args[0] == "--print";
    if !args.is_empty() && !print_only {
        return crate::cli_error(false, "init accepts only --print");
    }
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("stopproof: cannot resolve current directory: {}", e);
            return 1;
        }
    };

    let default_cfg =
        serde_json::to_string_pretty(&Config::default()).unwrap_or_else(|_| "{}".to_string());

    if print_only {
        println!("# {} (project root)\n{}\n", CONFIG_FILE, default_cfg);
        println!(
            "# .claude/settings.json (merge into \"hooks\")\n{}",
            settings_snippet()
        );
        return 0;
    }

    // Validate existing input before making any changes. An unreadable file
    // must never be treated as a missing file and overwritten.
    let cfg = match crate::config::load(&cwd) {
        Ok(cfg) => cfg,
        Err(err) => return crate::cli_error(false, &err),
    };
    let claude_dir = cwd.join(".claude");
    let settings_path = claude_dir.join("settings.json");
    let existing = match std::fs::read_to_string(&settings_path) {
        Ok(text) => Some(text),
        Err(err)
            if err.kind() == std::io::ErrorKind::NotFound
                && matches!(std::fs::symlink_metadata(&settings_path), Err(missing) if missing.kind() == std::io::ErrorKind::NotFound) =>
        {
            None
        }
        Err(err) => {
            return crate::cli_error(
                false,
                &format!("cannot read .claude/settings.json: {}", err),
            )
        }
    };
    let mut root = match existing.as_deref().map(serde_json::from_str::<Value>) {
        Some(Ok(value)) => value,
        Some(Err(err)) => {
            return crate::cli_error(
                false,
                &format!("invalid .claude/settings.json: {}; file preserved", err),
            )
        }
        None => json!({}),
    };
    if let Err(err) = validate_settings(&root) {
        return crate::cli_error(
            false,
            &format!("invalid .claude/settings.json: {}; file preserved", err),
        );
    }

    // 1. Default config (never overwrite an existing one).
    let cfg_path = cwd.join(CONFIG_FILE);
    if cfg_path.exists() {
        println!("  = {} already exists, leaving it alone", CONFIG_FILE);
    } else if std::fs::write(&cfg_path, format!("{}\n", default_cfg)).is_ok() {
        println!("  + wrote {}", CONFIG_FILE);
    } else {
        eprintln!("stopproof: failed to write {}", CONFIG_FILE);
        return 1;
    }

    // 2. Hook registration in .claude/settings.json.
    if std::fs::create_dir_all(&claude_dir).is_err() {
        eprintln!("stopproof: failed to create .claude/");
        return 1;
    }
    let modified = merge_into_settings(&mut root);
    if modified {
        if let Some(orig) = &existing {
            let backup = claude_dir.join(format!(
                "settings.json.bak.{}",
                crate::timefmt::compact_ts(crate::timefmt::now_epoch_secs())
            ));
            if let Err(err) = std::fs::write(backup, orig) {
                eprintln!(
                    "stopproof: cannot back up settings: {}; original preserved",
                    err
                );
                return 1;
            }
        }
        let pretty = serde_json::to_string_pretty(&root).unwrap_or_default();
        if std::fs::write(&settings_path, format!("{}\n", pretty)).is_ok() {
            println!("  + registered Stop hook in .claude/settings.json");
        } else {
            eprintln!("stopproof: failed to write .claude/settings.json");
            return 1;
        }
    } else {
        println!("  = Stop hook already registered in .claude/settings.json");
    }

    // 3. Keep receipts out of version control by default.
    if let Err(err) = ensure_gitignore(&cwd, &cfg.receipt_dir) {
        eprintln!("stopproof: failed to update .gitignore: {}", err);
        return 1;
    }
    println!(
        "  + ensured {}/ is gitignored",
        cfg.receipt_dir.trim_end_matches('/')
    );

    println!(
        "\nstopproof is installed for this project.\n
  - make sure `stopproof` is on PATH for Claude Code (or edit the hook\n    command in .claude/settings.json to an absolute path)\n  - hook timeout is 600s; raise it in .claude/settings.json if your test\n    suite runs longer\n  - try it: end a Claude Code session after an edit, or run `stopproof run`"
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_into_empty_settings() {
        let mut root = json!({});
        assert!(merge_into_settings(&mut root));
        let arr = root["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["hooks"][0]["command"], "stopproof");
    }

    #[test]
    fn is_idempotent() {
        let mut root = json!({});
        assert!(merge_into_settings(&mut root));
        assert!(!merge_into_settings(&mut root));
        assert_eq!(root["hooks"]["Stop"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn preserves_existing_hooks() {
        let mut root = json!({
            "permissions": { "allow": ["Bash(ls:*)"] },
            "hooks": {
                "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "dcg" }] }],
                "Stop": [{ "hooks": [{ "type": "command", "command": "other-tool" }] }]
            }
        });
        assert!(merge_into_settings(&mut root));
        assert_eq!(root["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(root["hooks"]["PreToolUse"][0]["hooks"][0]["command"], "dcg");
        assert_eq!(root["permissions"]["allow"][0], "Bash(ls:*)");
    }

    #[test]
    fn snippet_is_valid_json() {
        let v: Value = serde_json::from_str(&settings_snippet()).unwrap();
        assert!(v["hooks"]["Stop"].is_array());
    }
}
