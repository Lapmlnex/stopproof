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
        &[edit_tool_line("app.py"), assistant_text_line("Done.")],
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
    assert!(
        settings.contains("Bash(ls:*)"),
        "permissions lost: {}",
        settings
    );
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
        &[edit_tool_line("app.py"), assistant_text_line("Done.")],
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

#[test]
fn run_json_is_one_receipt_and_honors_command_override() {
    let dir = tempdir("json-override");
    write_file(&dir, ".stopproof.json", r#"{"test_command":"exit 9"}"#);
    let (code, stdout, stderr) = run_with_stdin(
        &dir,
        &[
            "run",
            "--command",
            "echo RUN_OUTPUT",
            "--timeout",
            "5",
            "--strict",
            "--json",
        ],
        "",
    );
    assert_eq!(code, Some(0), "{stdout}\n{stderr}");
    let receipt: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is only JSON");
    assert_eq!(receipt["verdict"], "pass");
    assert_eq!(receipt["test_command"], "echo RUN_OUTPUT");
    assert_eq!(receipt["test_exit"], 0);
    assert!(receipt["checks"].as_array().unwrap().iter().any(|c| {
        c["name"] == "tests" && c["detail"].as_str().unwrap().contains("from --command")
    }));
    assert!(receipt["test_output_tail"]
        .as_str()
        .unwrap()
        .contains("RUN_OUTPUT"));
    assert!(dir.join(".stopproof/last-receipt.md").is_file());
    assert!(stderr.trim().is_empty(), "{stderr}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn run_json_failure_has_verdict_exit_one() {
    let dir = tempdir("json-failure");
    let (code, stdout, _) = run_with_stdin(&dir, &["run", "--command", "exit 7", "--json"], "");
    assert_eq!(code, Some(1));
    let receipt: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(receipt["verdict"], "fail");
    assert_eq!(receipt["test_exit"], 7);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn strict_empty_project_fails_with_a_receipt() {
    let dir = tempdir("strict-empty");
    let (code, stdout, _) = run_with_stdin(&dir, &["run", "--strict", "--json"], "");
    assert_eq!(code, Some(1));
    let receipt: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(receipt["verdict"], "fail");
    assert!(receipt["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == "tests" && c["status"] == "fail"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn manual_and_doctor_reject_invalid_config_before_execution() {
    let dir = tempdir("badconfig");
    for config in [
        "{broken",
        r#"{"mode":"enforc"}"#,
        r#"{"verify_when":"sometimes"}"#,
        r#"{"test_timeout_secs":0}"#,
        r#"{"test_comand":"exit 0"}"#,
        r#"{"receipt_dir":" "}"#,
    ] {
        write_file(&dir, ".stopproof.json", config);
        for args in [
            vec!["run", "--command", "echo bad > executed", "--json"],
            vec!["doctor", "--json"],
        ] {
            let (code, stdout, stderr) = run_with_stdin(&dir, &args, "");
            assert_eq!(code, Some(2), "{config}: {stdout}\n{stderr}");
            let error: serde_json::Value = serde_json::from_str(&stdout).unwrap();
            assert!(error["error"].as_str().unwrap().contains(".stopproof.json"));
        }
        assert!(!dir.join("executed").exists());
        assert!(!dir.join(".stopproof").exists());
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn unreadable_config_is_not_treated_as_missing() {
    let dir = tempdir("config-directory");
    std::fs::create_dir(dir.join(".stopproof.json")).unwrap();
    let (code, _, stderr) = run_with_stdin(&dir, &["run"], "");
    assert_eq!(code, Some(2));
    assert!(stderr.contains(".stopproof.json"));
    std::fs::remove_dir_all(&dir).ok();
}

#[cfg(unix)]
#[test]
fn dangling_config_symlink_is_an_error() {
    let dir = tempdir("config-symlink");
    std::os::unix::fs::symlink("missing-config.json", dir.join(".stopproof.json")).unwrap();
    let (code, stdout, _) = run_with_stdin(&dir, &["doctor", "--json"], "");
    assert_eq!(code, Some(2), "{stdout}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn unknown_arguments_and_invalid_option_values_are_usage_errors() {
    let dir = tempdir("badargs");
    for args in [
        vec!["unknown"],
        vec!["run", "--typo"],
        vec!["run", "unexpected"],
        vec!["run", "--command"],
        vec!["run", "--command", " "],
        vec!["run", "--timeout"],
        vec!["run", "--timeout", "0"],
        vec!["run", "--timeout", "-1"],
        vec!["run", "--timeout", "1.5"],
        vec!["run", "--timeout", "nope"],
        vec!["doctor", "--strict"],
        vec!["init", "--unknown"],
        vec!["hook", "--unknown"],
        vec!["--version", "--unknown"],
    ] {
        let (code, stdout, stderr) = run_with_stdin(&dir, &args, "");
        assert_eq!(code, Some(2), "{args:?}: {stdout}\n{stderr}");
        assert!(stdout.is_empty(), "error prose belongs on stderr: {stdout}");
        assert!(!stderr.is_empty());
    }
    let (code, stdout, _) = run_with_stdin(&dir, &["run", "--typo", "--json"], "");
    assert_eq!(code, Some(2));
    assert!(serde_json::from_str::<serde_json::Value>(&stdout).unwrap()["error"].is_string());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn doctor_reports_config_without_running_the_command() {
    let dir = tempdir("doctor");
    write_file(
        &dir,
        ".stopproof.json",
        r#"{"test_command":"echo ran > executed","test_timeout_secs":42}"#,
    );
    let (code, stdout, stderr) = run_with_stdin(&dir, &["doctor", "--json"], "");
    assert_eq!(code, Some(0), "{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["config"]["test_timeout_secs"], 42);
    assert_eq!(report["config_source"], ".stopproof.json");
    assert_eq!(report["test_command"], "echo ran > executed");
    assert_eq!(report["detected_from"], "config");
    assert_eq!(report["command_executed"], false);
    assert!(!dir.join("executed").exists());
    assert!(!dir.join(".stopproof").exists());
    let (code, human, _) = run_with_stdin(&dir, &["doctor"], "");
    assert_eq!(code, Some(0));
    assert!(human.contains("echo ran > executed"));
    assert!(human.contains("42"));
    assert!(human.contains("not executed"));
    assert!(!dir.join("executed").exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn doctor_warns_when_no_tests_are_detected() {
    let dir = tempdir("doctor-empty");
    let (code, stdout, _) = run_with_stdin(&dir, &["doctor", "--json"], "");
    assert_eq!(code, Some(0));
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(report["test_command"].is_null());
    assert_eq!(report["config_source"], "defaults");
    assert!(!report["warnings"].as_array().unwrap().is_empty());
    let (_, human, _) = run_with_stdin(&dir, &["doctor"], "");
    assert!(human.contains("WARN") && human.contains("no test command"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn failed_receipt_storage_makes_manual_verification_fail() {
    let dir = tempdir("receipt-failure");
    write_file(&dir, ".stopproof", "this path is a file");
    let (code, stdout, _) = run_with_stdin(&dir, &["run", "--command", "exit 0", "--json"], "");
    assert_eq!(code, Some(1));
    let receipt: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(receipt["verdict"], "fail");
    assert!(receipt["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == "receipt-storage" && c["status"] == "fail"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn failed_receipt_storage_hook_allows_without_claiming_verified() {
    let dir = tempdir("hook-receipt-failure");
    write_file(&dir, ".stopproof", "this path is a file");
    write_file(
        &dir,
        ".stopproof.json",
        r#"{"test_command":"exit 0","verify_when":"always"}"#,
    );
    let t = write_transcript(&dir, &[assistant_text_line("Done.")]);
    let (code, stdout) = run_hook(&dir, &hook_stdin("s-storage", &t, &dir));
    assert_eq!(code, Some(0));
    assert!(!stdout.contains("\"decision\""));
    assert!(
        stdout.contains("receipt") && stdout.contains("not saved"),
        "{stdout}"
    );
    assert!(!stdout.contains("verified"), "{stdout}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn invalid_config_hook_still_fails_open() {
    let dir = tempdir("hook-badconfig");
    write_file(&dir, ".stopproof.json", "{invalid");
    let t = write_transcript(
        &dir,
        &[edit_tool_line("app.py"), assistant_text_line("Done.")],
    );
    let (code, stdout) = run_hook(&dir, &hook_stdin("s-bad-config", &t, &dir));
    assert_eq!(code, Some(0));
    assert!(!stdout.contains("\"decision\""));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn init_invalid_settings_reports_failure_and_preserves_original() {
    for invalid in [
        "{broken",
        "[]",
        r#"{"hooks":42}"#,
        r#"{"hooks":{"Stop":{}}}"#,
    ] {
        let dir = tempdir("init-invalid");
        write_file(&dir, ".claude/settings.json", invalid);
        let (code, stdout, stderr) = run_with_stdin(&dir, &["init"], "");
        assert_eq!(code, Some(2), "{stdout}\n{stderr}");
        assert!(!stdout.contains("installed"));
        assert_eq!(
            std::fs::read_to_string(dir.join(".claude/settings.json")).unwrap(),
            invalid
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(unix)]
#[test]
fn timeout_override_fails_the_verdict() {
    let dir = tempdir("timeout-override");
    let (code, stdout, _) = run_with_stdin(
        &dir,
        &[
            "run",
            "--command",
            "exec sleep 3",
            "--timeout",
            "1",
            "--json",
        ],
        "",
    );
    assert_eq!(code, Some(1));
    let receipt: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(receipt["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["detail"].as_str().unwrap().contains("timed out after 1s")));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn largest_timeout_does_not_overflow_the_clock() {
    let dir = tempdir("timeout-max");
    let (code, stdout, stderr) = run_with_stdin(
        &dir,
        &[
            "run",
            "--command",
            "exit 0",
            "--timeout",
            "18446744073709551615",
            "--json",
        ],
        "",
    );
    assert_eq!(code, Some(0), "{stdout}\n{stderr}");
    std::fs::remove_dir_all(&dir).ok();
}

#[cfg(unix)]
#[test]
fn large_subprocess_output_is_drained_and_a_small_tail_is_reported() {
    let dir = tempdir("large-output");
    let command = "i=0; while [ \"$i\" -lt 25000 ]; do echo 1234567890123456789012345678901234567890; echo 1234567890123456789012345678901234567890 >&2; i=$((i+1)); done; echo FINAL_OUTPUT >&2";
    let (code, stdout, stderr) = run_with_stdin(
        &dir,
        &["run", "--command", command, "--timeout", "30", "--json"],
        "",
    );
    assert_eq!(code, Some(0), "{stderr}");
    let receipt: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let tail = receipt["test_output_tail"].as_str().unwrap();
    assert!(tail.len() <= 3503);
    assert!(tail.contains("FINAL_OUTPUT"));
    std::fs::remove_dir_all(&dir).ok();
}
