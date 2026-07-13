//! End-to-end tests: spawn the real binary and speak the hook protocol.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static N: AtomicUsize = AtomicUsize::new(0);

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_stopproof")
}

fn tempdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "stopproof-e2e-{}-{}-{}",
        tag,
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn write_file(dir: &Path, rel: &str, content: &str) {
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(p, content).unwrap();
}

fn assistant_text_line(text: &str) -> String {
    serde_json::json!({
        "type": "assistant",
        "timestamp": "2026-07-12T00:00:00Z",
        "message": {
            "role": "assistant",
            "content": [{"type": "text", "text": text}]
        }
    })
    .to_string()
}

fn edit_tool_line(path: &str) -> String {
    serde_json::json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [{"type": "tool_use", "name": "Edit", "input": {"file_path": path}}]
        }
    })
    .to_string()
}

fn write_transcript(dir: &Path, lines: &[String]) -> PathBuf {
    let p = dir.join("transcript.jsonl");
    std::fs::write(&p, lines.join("\n")).unwrap();
    p
}

fn hook_stdin(session: &str, transcript: &Path, cwd: &Path) -> String {
    serde_json::json!({
        "session_id": session,
        "transcript_path": transcript.to_string_lossy(),
        "cwd": cwd.to_string_lossy(),
        "hook_event_name": "Stop",
        "stop_hook_active": false
    })
    .to_string()
}

fn run_with_stdin(cwd: &Path, args: &[&str], stdin: &str) -> (Option<i32>, String, String) {
    let mut child = Command::new(bin())
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary should spawn");
    // Ignore write errors: a binary that exits before reading stdin (e.g.
    // STOPPROOF_SKIP) closes the pipe and the write may report EPIPE.
    let _ = child.stdin.take().unwrap().write_all(stdin.as_bytes());
    let out = child.wait_with_output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn run_hook(cwd: &Path, stdin: &str) -> (Option<i32>, String) {
    let (code, stdout, _stderr) = run_with_stdin(cwd, &[], stdin);
    (code, stdout)
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn init_repo(dir: &Path) {
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "t@example.com"],
        vec!["config", "user.name", "stopproof-test"],
    ] {
        let ok = Command::new("git")
            .args(&args)
            .current_dir(dir)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        assert!(ok, "git {:?} failed", args);
    }
}

// ---------------------------------------------------------------------------

#[test]
fn no_work_session_allows_silently() {
    let dir = tempdir("nowork");
    let t = write_transcript(
        &dir,
        &[assistant_text_line(
            "Here is the explanation you asked about.",
        )],
    );
    let (code, stdout) = run_hook(&dir, &hook_stdin("s-nowork", &t, &dir));
    assert_eq!(code, Some(0));
    assert_eq!(stdout.trim(), "");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn invalid_stdin_never_bricks_the_session() {
    let dir = tempdir("badstdin");
    let (code, stdout) = run_hook(&dir, "this is not json");
    assert_eq!(code, Some(0));
    assert_eq!(stdout.trim(), "");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn failing_tests_block_then_attempts_exhaust() {
    let dir = tempdir("failtests");
    write_file(
        &dir,
        ".stopproof.json",
        r#"{"test_command": "exit 1", "max_attempts": 1}"#,
    );
    write_file(&dir, "app.py", "print('hi')\n");
    let t = write_transcript(
        &dir,
        &[
            edit_tool_line("app.py"),
            assistant_text_line("Done, I updated app.py."),
        ],
    );
    let stdin = hook_stdin("s-fail", &t, &dir);

    // First stop: blocked with a reason.
    let (code, stdout) = run_hook(&dir, &stdin);
    assert_eq!(code, Some(0));
    assert!(
        stdout.contains("\"decision\":\"block\""),
        "expected block, got: {}",
        stdout
    );
    assert!(stdout.contains("attempt 1/1"), "got: {}", stdout);

    // Second stop: attempts exhausted, allowed with a failure notice.
    let (code2, stdout2) = run_hook(&dir, &stdin);
    assert_eq!(code2, Some(0));
    assert!(!stdout2.contains("\"decision\""), "got: {}", stdout2);
    assert!(stdout2.contains("still failing"), "got: {}", stdout2);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn passing_tests_allow_and_write_receipt() {
    let dir = tempdir("pass");
    write_file(&dir, ".stopproof.json", r#"{"test_command": "exit 0"}"#);
    write_file(&dir, "app.py", "print('hi')\n");
    let t = write_transcript(
        &dir,
        &[
            edit_tool_line("app.py"),
            assistant_text_line("Done, I updated app.py."),
        ],
    );
    let (code, stdout) = run_hook(&dir, &hook_stdin("s-pass", &t, &dir));
    assert_eq!(code, Some(0));
    assert!(stdout.contains("stopproof: verified"), "got: {}", stdout);
    assert!(!stdout.contains("\"decision\""), "got: {}", stdout);
    assert!(dir.join(".stopproof/last-receipt.md").exists());
    let receipts: Vec<_> = std::fs::read_dir(dir.join(".stopproof/receipts"))
        .unwrap()
        .collect();
    assert!(!receipts.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn phantom_edit_is_caught() {
    if !git_available() {
        eprintln!("git not available; skipping");
        return;
    }
    let dir = tempdir("phantom");
    init_repo(&dir);
    write_file(&dir, ".stopproof.json", r#"{"test_command": "exit 0"}"#);
    // The transcript says src/ghost.py was edited and the final message
    // claims it — but the file never landed on disk and git has no trace.
    let t = write_transcript(
        &dir,
        &[
            edit_tool_line("src/ghost.py"),
            assistant_text_line("Done — I updated src/ghost.py and everything works."),
        ],
    );
    let (code, stdout) = run_hook(&dir, &hook_stdin("s-phantom", &t, &dir));
    assert_eq!(code, Some(0));
    assert!(
        stdout.contains("\"decision\":\"block\""),
        "expected block, got: {}",
        stdout
    );
    assert!(stdout.contains("phantom"), "got: {}", stdout);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn warn_mode_reports_but_never_blocks() {
    let dir = tempdir("warn");
    write_file(
        &dir,
        ".stopproof.json",
        r#"{"mode": "warn", "test_command": "exit 1"}"#,
    );
    write_file(&dir, "app.py", "print('hi')\n");
    let t = write_transcript(
        &dir,
        &[
            edit_tool_line("app.py"),
            assistant_text_line("Done, I updated app.py."),
        ],
    );
    let (code, stdout) = run_hook(&dir, &hook_stdin("s-warn", &t, &dir));
    assert_eq!(code, Some(0));
    assert!(!stdout.contains("\"decision\""), "got: {}", stdout);
    assert!(stdout.contains("warn mode"), "got: {}", stdout);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn off_mode_is_a_noop() {
    let dir = tempdir("off");
    write_file(
        &dir,
        ".stopproof.json",
        r#"{"mode": "off", "test_command": "exit 1"}"#,
    );
    write_file(&dir, "app.py", "print('hi')\n");
    let t = write_transcript(
        &dir,
        &[
            edit_tool_line("app.py"),
            assistant_text_line("Done."),
        ],
    );
    let (code, stdout) = run_hook(&dir, &hook_stdin("s-off", &t, &dir));
    assert_eq!(code, Some(0));
    assert_eq!(stdout.trim(), "");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn run_mode_reflects_test_outcome_in_exit_code() {
    let dir = tempdir("runmode");
    write_file(&dir, ".stopproof.json", r#"{"test_command": "exit 0"}"#);
    let (code, stdout, _) = run_with_stdin(&dir, &["run"], "");
    assert_eq!(code, Some(0), "stdout: {}", stdout);
    assert!(stdout.contains("verdict: PASS"), "got: {}", stdout);

    write_file(&dir, ".stopproof.json", r#"{"test_command": "exit 1"}"#);
    let (code, stdout, _) = run_with_stdin(&dir, &["run"], "");
    assert_eq!(code, Some(1), "stdout: {}", stdout);
    assert!(stdout.contains("verdict: FAIL"), "got: {}", stdout);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn init_installs_hook_and_is_idempotent() {
    let dir = tempdir("init");
    let (code, stdout, stderr) = run_with_stdin(&dir, &["init"], "");
    assert_eq!(code, Some(0), "stdout: {} stderr: {}", stdout, stderr);
    assert!(dir.join(".stopproof.json").exists());
    let settings = std::fs::read_to_string(dir.join(".claude/settings.json")).unwrap();
    assert!(settings.contains("stopproof"));
    assert!(settings.contains("\"Stop\""));

    // Second init must not duplicate the hook entry.
    let (code2, _, _) = run_with_stdin(&dir, &["init"], "");
    assert_eq!(code2, Some(0));
    let settings2 = std::fs::read_to_string(dir.join(".claude/settings.json")).unwrap();
    assert_eq!(
        settings2.matches("stopproof").count(),
        1,
        "hook duplicated: {}",
        settings2
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn init_preserves_existing_settings() {
    let dir = tempdir("initmerge");
    write_file(
        &dir,
        ".claude/settings.json",
        r#"{"permissions": {"allow": ["Bash(ls:*)"]}, "hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "dcg"}]}]}}"#,
    );
    let (code, _, _) = run_with_stdin(&dir, &["init"], "");
    assert_eq!(code, Some(0));
    let settings = std::fs::read_to_string(dir.join(".claude/settings.json")).unwrap();
    assert!(settings.contains("dcg"), "existing hook lost: {}", settings);
    assert!(settings.contains("Bash(ls:*)"), "permissions lost: {}", settings);
    assert!(settings.contains("stopproof"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn version_prints() {
    let (code, stdout, _) = run_with_stdin(Path::new("."), &["--version"], "");
    assert_eq!(code, Some(0));
    assert!(stdout.contains("stopproof"));
}

#[test]
fn skip_env_var_bypasses_everything() {
    let dir = tempdir("skip");
    write_file(&dir, ".stopproof.json", r#"{"test_command": "exit 1"}"#);
    write_file(&dir, "app.py", "print('hi')\n");
    let t = write_transcript(
        &dir,
        &[
            edit_tool_line("app.py"),
            assistant_text_line("Done."),
        ],
    );
    let mut child = Command::new(bin())
        .current_dir(&dir)
        .env("STOPPROOF_SKIP", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child
        .stdin
        .take()
        .unwrap()
        .write_all(hook_stdin("s-skip", &t, &dir).as_bytes());
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "");
    std::fs::remove_dir_all(&dir).ok();
}
