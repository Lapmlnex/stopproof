//! Detect the project's test command and run it with a timeout.

use crate::config::Config;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct TestOutcome {
    pub command: Option<String>,
    /// Where the command came from: "config", "package.json", "Cargo.toml",
    /// "go.mod", "pytest", "Makefile", or "none".
    pub detected_from: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub output_tail: String,
    pub duration_ms: u128,
}

fn package_json_test(cwd: &Path) -> Option<String> {
    let text = std::fs::read_to_string(cwd.join("package.json")).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let script = v.get("scripts")?.get("test")?.as_str()?;
    if script.trim().is_empty() || script.contains("no test specified") {
        return None;
    }
    let runner = if cwd.join("pnpm-lock.yaml").exists() {
        "pnpm test"
    } else if cwd.join("yarn.lock").exists() {
        "yarn test"
    } else if cwd.join("bun.lock").exists() || cwd.join("bun.lockb").exists() {
        "bun run test"
    } else {
        "npm test --silent"
    };
    Some(runner.to_string())
}

fn has_pytest_layout(cwd: &Path) -> bool {
    if cwd.join("pytest.ini").exists() || cwd.join("conftest.py").exists() {
        return true;
    }
    if let Ok(text) = std::fs::read_to_string(cwd.join("pyproject.toml")) {
        if text.contains("pytest") {
            return true;
        }
    }
    let tests = cwd.join("tests");
    if tests.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&tests) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if (name.starts_with("test_") || name.ends_with("_test.py"))
                    && name.ends_with(".py")
                {
                    return true;
                }
            }
        }
    }
    false
}

fn makefile_has_test(cwd: &Path) -> bool {
    for candidate in ["Makefile", "makefile"] {
        if let Ok(text) = std::fs::read_to_string(cwd.join(candidate)) {
            if text.lines().any(|l| l.starts_with("test:")) {
                return true;
            }
        }
    }
    false
}

/// Returns (command, source). Order: explicit config, then ecosystem markers.
pub fn detect(cwd: &Path, cfg: &Config) -> (Option<String>, String) {
    if !cfg.test_command.trim().is_empty() {
        return (Some(cfg.test_command.clone()), "config".to_string());
    }
    if let Some(cmd) = package_json_test(cwd) {
        return (Some(cmd), "package.json".to_string());
    }
    if cwd.join("Cargo.toml").exists() {
        return (Some("cargo test --quiet".to_string()), "Cargo.toml".to_string());
    }
    if cwd.join("go.mod").exists() {
        return (Some("go test ./...".to_string()), "go.mod".to_string());
    }
    if has_pytest_layout(cwd) {
        let py = if cfg!(windows) { "python" } else { "python3" };
        return (Some(format!("{} -m pytest -q", py)), "pytest".to_string());
    }
    if makefile_has_test(cwd) {
        return (Some("make test".to_string()), "Makefile".to_string());
    }
    (None, "none".to_string())
}

fn tail_of(bytes_out: &[u8], bytes_err: &[u8], max_lines: usize, max_chars: usize) -> String {
    let mut text = String::from_utf8_lossy(bytes_out).to_string();
    let err = String::from_utf8_lossy(bytes_err);
    if !err.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&err);
    }
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    let mut tail = lines[start..].join("\n");
    if tail.len() > max_chars {
        let mut cut = tail.len() - max_chars;
        while cut < tail.len() && !tail.is_char_boundary(cut) {
            cut += 1;
        }
        tail = format!("…{}", &tail[cut..]);
    }
    tail
}

fn run_with_timeout(cwd: &Path, command: &str, secs: u64) -> (Option<i32>, bool, String) {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", command]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
    };
    cmd.current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("STOPPROOF_ACTIVE", "1");

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return (None, false, format!("failed to spawn `{}`: {}", command, e)),
    };

    // Readers report over channels rather than being joined: if the test
    // command leaves grandchildren holding the pipes open (dev servers,
    // stubborn runners), killing the shell isn't enough and a join would
    // hang forever. recv_timeout below caps how long we wait for output.
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let (tx_out, rx_out) = std::sync::mpsc::channel::<Vec<u8>>();
    let (tx_err, rx_err) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = stdout_pipe.as_mut() {
            use std::io::Read;
            p.read_to_end(&mut buf).ok();
        }
        tx_out.send(buf).ok();
    });
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = stderr_pipe.as_mut() {
            use std::io::Read;
            p.read_to_end(&mut buf).ok();
        }
        tx_err.send(buf).ok();
    });

    let deadline = Instant::now() + Duration::from_secs(secs.max(1));
    let mut timed_out = false;
    let exit_code: Option<i32> = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {
                if Instant::now() >= deadline {
                    child.kill().ok();
                    child.wait().ok();
                    timed_out = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => break None,
        }
    };

    let grace = Duration::from_secs(10);
    let out = rx_out.recv_timeout(grace).unwrap_or_default();
    let err = rx_err.recv_timeout(grace).unwrap_or_default();
    (exit_code, timed_out, tail_of(&out, &err, 40, 3500))
}

pub fn detect_and_run(cwd: &Path, cfg: &Config) -> TestOutcome {
    let (command, detected_from) = detect(cwd, cfg);
    let mut outcome = TestOutcome {
        command: command.clone(),
        detected_from,
        ..Default::default()
    };
    if let Some(cmd) = command {
        let started = Instant::now();
        let (exit_code, timed_out, tail) = run_with_timeout(cwd, &cmd, cfg.test_timeout_secs);
        outcome.exit_code = exit_code;
        outcome.timed_out = timed_out;
        outcome.output_tail = tail;
        outcome.duration_ms = started.elapsed().as_millis();
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    fn tempdir(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "stopproof-testrun-{}-{}-{}",
            tag,
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn config_command_wins() {
        let dir = tempdir("cfg");
        let cfg = Config {
            test_command: "echo custom".to_string(),
            ..Config::default()
        };
        let (cmd, from) = detect(&dir, &cfg);
        assert_eq!(cmd.as_deref(), Some("echo custom"));
        assert_eq!(from, "config");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detects_package_json() {
        let dir = tempdir("npm");
        std::fs::write(
            dir.join("package.json"),
            r#"{"scripts":{"test":"vitest run"}}"#,
        )
        .unwrap();
        let (cmd, from) = detect(&dir, &Config::default());
        assert_eq!(cmd.as_deref(), Some("npm test --silent"));
        assert_eq!(from, "package.json");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_placeholder_npm_test() {
        let dir = tempdir("npm2");
        std::fs::write(
            dir.join("package.json"),
            r#"{"scripts":{"test":"echo \"Error: no test specified\" && exit 1"}}"#,
        )
        .unwrap();
        let (cmd, _) = detect(&dir, &Config::default());
        assert!(cmd.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn nothing_detected_in_empty_dir() {
        let dir = tempdir("empty");
        let (cmd, from) = detect(&dir, &Config::default());
        assert!(cmd.is_none());
        assert_eq!(from, "none");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn runs_passing_and_failing_commands() {
        let dir = tempdir("run");
        let cfg = Config {
            test_command: "exit 0".to_string(),
            ..Config::default()
        };
        let ok = detect_and_run(&dir, &cfg);
        assert_eq!(ok.exit_code, Some(0));

        let failing_cfg = Config {
            test_command: "exit 3".to_string(),
            ..Config::default()
        };
        let bad = detect_and_run(&dir, &failing_cfg);
        assert_eq!(bad.exit_code, Some(3));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn captures_output_tail() {
        let dir = tempdir("tail");
        let cfg = Config {
            test_command: "echo FAILURE_DETAIL".to_string(),
            ..Config::default()
        };
        let out = detect_and_run(&dir, &cfg);
        assert!(out.output_tail.contains("FAILURE_DETAIL"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
