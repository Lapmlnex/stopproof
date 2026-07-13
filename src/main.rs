//! stopproof — deterministic completion receipts for coding agents.
//!
//! Runs as a Claude Code Stop hook. When the agent claims it's done,
//! stopproof reconciles the claim against the actual git diff, re-runs the
//! project's test command, and either blocks the stop with a surgical
//! fix-list or writes a verification receipt. No LLM calls. No API cost.

mod claims;
mod config;
mod gitstate;
mod hook_io;
mod init_cmd;
mod receipt;
mod testrun;
mod timefmt;
mod transcript;

use config::Config;
use receipt::{Check, Receipt};
use std::path::{Path, PathBuf};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_usage() {
    println!(
        "stopproof {} — deterministic completion receipts for coding agents

USAGE:
  stopproof            Run as a Claude Code Stop hook (reads JSON on stdin)
  stopproof hook       Same as above, explicit
  stopproof run        Verify the current directory manually (great for CI)
  stopproof init       Install the hook + default config into this project
  stopproof init --print   Print config snippets without writing files
  stopproof --version  Print version

DOCS: https://github.com/Lapmlnex/stopproof",
        VERSION
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(|s| s.as_str()) {
        None | Some("hook") => hook_main(),
        Some("run") => run_main(),
        Some("init") => init_cmd::run(&args[1..]),
        Some("--version") | Some("-V") | Some("version") => {
            println!("stopproof {}", VERSION);
            0
        }
        Some("--help") | Some("-h") | Some("help") => {
            print_usage();
            0
        }
        Some(other) => {
            eprintln!("stopproof: unknown command `{}`\n", other);
            print_usage();
            1
        }
    };
    std::process::exit(code);
}

struct Verification {
    checks: Vec<Check>,
    verdict: String, // "pass" | "fail"
    test: testrun::TestOutcome,
}

fn build_verification(
    cwd: &Path,
    cfg: &Config,
    facts: &transcript::TranscriptFacts,
    git: &gitstate::GitFacts,
    cl: &claims::Claims,
    has_transcript: bool,
) -> Verification {
    let mut checks: Vec<Check> = Vec::new();

    // Check 1: claim vs. reality reconciliation (needs transcript + git).
    if !has_transcript {
        checks.push(Check::new(
            "diff-reconciliation",
            "info",
            "manual run: no transcript to reconcile".to_string(),
        ));
    } else if !git.in_repo {
        checks.push(Check::new(
            "diff-reconciliation",
            "info",
            "not a git repository; claim reconciliation skipped".to_string(),
        ));
    } else {
        let mut phantom: Vec<String> = Vec::new();
        for f in &cl.claimed_paths {
            if !transcript::any_path_match(&facts.edited_files, f) {
                continue; // mentioned but never edited via tools — reference, not claim
            }
            let evidence = transcript::any_path_match(&git.changed, f)
                || transcript::any_path_match(&git.committed_since, f);
            if !evidence {
                phantom.push(f.clone());
            }
        }
        if phantom.is_empty() {
            checks.push(Check::new(
                "diff-reconciliation",
                "pass",
                "final-message file claims are backed by git evidence".to_string(),
            ));
        } else {
            checks.push(Check::new(
                "diff-reconciliation",
                "fail",
                format!(
                    "claimed as changed, but git shows no evidence (phantom edit?): {}",
                    phantom.join(", ")
                ),
            ));
        }

        let mut ghosts: Vec<String> = Vec::new();
        for f in &facts.edited_files {
            let evidence = transcript::any_path_match(&git.changed, f)
                || transcript::any_path_match(&git.committed_since, f);
            if !evidence && !transcript::any_path_match(&cl.claimed_paths, f) {
                ghosts.push(f.clone());
            }
        }
        if !ghosts.is_empty() {
            checks.push(Check::new(
                "session-edits",
                "warn",
                format!(
                    "edited during session but absent from git (reverted?): {}",
                    ghosts.join(", ")
                ),
            ));
        }
    }

    // Check 2: tests actually pass.
    let test = testrun::detect_and_run(cwd, cfg);
    match &test.command {
        Some(cmd) => {
            if test.timed_out {
                checks.push(Check::new(
                    "tests",
                    "fail",
                    format!("`{}` timed out after {}s", cmd, cfg.test_timeout_secs),
                ));
            } else {
                match test.exit_code {
                    Some(0) => checks.push(Check::new(
                        "tests",
                        "pass",
                        format!(
                            "`{}` passed in {}ms (from {})",
                            cmd, test.duration_ms, test.detected_from
                        ),
                    )),
                    Some(code) => checks.push(Check::new(
                        "tests",
                        "fail",
                        format!("`{}` exited {} (from {})", cmd, code, test.detected_from),
                    )),
                    None => checks.push(Check::new(
                        "tests",
                        "fail",
                        format!("`{}` was terminated before finishing", cmd),
                    )),
                }
            }
        }
        None => {
            if cfg.require_tests {
                checks.push(Check::new(
                    "tests",
                    "fail",
                    "no test command detected and require_tests=true; set test_command in .stopproof.json".to_string(),
                ));
            } else {
                checks.push(Check::new(
                    "tests",
                    "warn",
                    "no test command detected; set test_command in .stopproof.json for full verification".to_string(),
                ));
            }
        }
    }

    // Check 3: test-weakening heuristic (warn only in v1).
    if !git.deleted_tests.is_empty() || git.assertions_removed > git.assertions_added {
        checks.push(Check::new(
            "test-integrity",
            "warn",
            format!(
                "possible test weakening: {} test file(s) deleted; assertion lines -{} / +{}",
                git.deleted_tests.len(),
                git.assertions_removed,
                git.assertions_added
            ),
        ));
    }

    let verdict = if checks.iter().any(|c| c.status == "fail") {
        "fail".to_string()
    } else {
        "pass".to_string()
    };

    Verification {
        checks,
        verdict,
        test,
    }
}

fn make_receipt(
    session_id: &str,
    attempt: u32,
    v: &Verification,
    facts: &transcript::TranscriptFacts,
    git: &gitstate::GitFacts,
    cl: &claims::Claims,
) -> Receipt {
    Receipt {
        tool: "stopproof".to_string(),
        version: VERSION.to_string(),
        verdict: v.verdict.clone(),
        session_id: session_id.to_string(),
        generated_at: timefmt::iso_utc(timefmt::now_epoch_secs()),
        attempt,
        checks: v.checks.clone(),
        edited_files: facts.edited_files.clone(),
        claimed_paths: cl.claimed_paths.clone(),
        git_changed: git.changed.clone(),
        test_command: v.test.command.clone(),
        test_exit: v.test.exit_code,
        test_output_tail: v.test.output_tail.clone(),
    }
}

fn block_reason(cfg: &Config, v: &Verification, attempt: u32) -> String {
    let mut reason = format!(
        "stopproof verification FAILED (attempt {}/{}):\n",
        attempt, cfg.max_attempts
    );
    for c in v.checks.iter().filter(|c| c.status == "fail") {
        reason.push_str(&format!("  - {}: {}\n", c.name, c.detail));
    }
    let tests_failed = v
        .checks
        .iter()
        .any(|c| c.name == "tests" && c.status == "fail");
    if tests_failed && !v.test.output_tail.trim().is_empty() {
        reason.push_str(&format!(
            "\n--- test output (tail) ---\n{}\n",
            v.test.output_tail
        ));
    }
    reason.push_str(&format!(
        "\nDo not claim completion yet. Fix the issues above, re-run the checks, and finish only when they pass. Full receipt: {}/last-receipt.md",
        cfg.receipt_dir
    ));
    if reason.len() > 9000 {
        let mut cut = 9000;
        while cut < reason.len() && !reason.is_char_boundary(cut) {
            cut += 1;
        }
        reason.truncate(cut);
        reason.push_str("\n…(truncated)");
    }
    reason
}

fn hook_main() -> i32 {
    // Escape hatch: STOPPROOF_SKIP=1 disables verification entirely.
    if std::env::var("STOPPROOF_SKIP").ok().as_deref() == Some("1") {
        return 0;
    }
    let input = match hook_io::read_stdin_input() {
        Some(i) => i,
        None => return 0, // never brick a session on our own failure
    };
    // Only verify main-chain stops; other events pass through.
    if !input.hook_event_name.is_empty() && input.hook_event_name != "Stop" {
        return 0;
    }

    let cwd: PathBuf = if input.cwd.is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        PathBuf::from(&input.cwd)
    };
    let cfg = config::load(&cwd);
    if cfg.mode == "off" {
        return 0;
    }

    let facts = transcript::analyze(Path::new(&input.transcript_path), &cwd);
    let git = gitstate::collect(&cwd, facts.session_start_epoch, &cfg.ignore_paths);
    let cl = claims::extract(&facts.final_text, &cfg.claim_keywords);

    // When should we verify? "always", or: the session actually edited
    // files, or the agent claims completion over a dirty tree (it may have
    // changed files via Bash, invisible to the transcript's file tools).
    // A pure Q&A stop — even in a repo with the user's own WIP — passes
    // through untouched.
    let session_did_work = !facts.edited_files.is_empty();
    let should_verify = cfg.verify_when == "always"
        || session_did_work
        || (cl.completion_claimed && !git.changed.is_empty());
    if !should_verify {
        return 0;
    }

    let v = build_verification(&cwd, &cfg, &facts, &git, &cl, true);

    let mut prev_blocks = hook_io::attempts_get(&cwd, &cfg.receipt_dir, &input.session_id);
    // Claude Code says a stop hook already blocked this cycle but our
    // ledger shows nothing (lost state, or another hook): be conservative
    // and count one prior attempt rather than risk a longer loop.
    if input.stop_hook_active && prev_blocks == 0 {
        prev_blocks = 1;
    }
    let attempt = prev_blocks + 1;
    let rec = make_receipt(&input.session_id, attempt, &v, &facts, &git, &cl);
    receipt::write(&cwd, &cfg, &rec);

    if v.verdict == "pass" {
        hook_io::attempts_reset(&cwd, &cfg.receipt_dir, &input.session_id);
        let tests_note = match (&v.test.command, v.test.exit_code) {
            (Some(cmd), Some(0)) => format!("tests passed (`{}`)", cmd),
            _ => "no tests detected".to_string(),
        };
        let recon_note = if git.in_repo {
            " · diff consistent"
        } else {
            ""
        };
        hook_io::emit_allow(Some(&format!(
            "stopproof: verified — {}{} · receipt: {}/last-receipt.md",
            tests_note, recon_note, cfg.receipt_dir
        )));
        return 0;
    }

    // Verification failed.
    if cfg.mode == "warn" {
        let fails: Vec<String> = v
            .checks
            .iter()
            .filter(|c| c.status == "fail")
            .map(|c| c.name.clone())
            .collect();
        hook_io::emit_allow(Some(&format!(
            "stopproof (warn mode): verification FAILED [{}] — receipt: {}/last-receipt.md",
            fails.join(", "),
            cfg.receipt_dir
        )));
        return 0;
    }

    if prev_blocks >= cfg.max_attempts {
        hook_io::attempts_reset(&cwd, &cfg.receipt_dir, &input.session_id);
        hook_io::emit_allow(Some(&format!(
            "stopproof: still failing after {} blocked attempt(s) — allowing the session to end so you stay in control. FAILED receipt: {}/last-receipt.md",
            prev_blocks, cfg.receipt_dir
        )));
        return 0;
    }

    hook_io::attempts_set(&cwd, &cfg.receipt_dir, &input.session_id, prev_blocks + 1);
    // Fail open: if attempt state can't be persisted (read-only dir, locked
    // file), blocking would loop forever — report instead of blocking.
    if hook_io::attempts_get(&cwd, &cfg.receipt_dir, &input.session_id) != prev_blocks + 1 {
        hook_io::emit_allow(Some(&format!(
            "stopproof: verification FAILED but attempt state is not persistable — failing open (not blocking). Receipt: {}/last-receipt.md",
            cfg.receipt_dir
        )));
        return 0;
    }
    hook_io::emit_block(&block_reason(&cfg, &v, attempt));
    0
}

fn run_main() -> i32 {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let cfg = config::load(&cwd);
    let facts = transcript::TranscriptFacts::default();
    let git = gitstate::collect(&cwd, None, &cfg.ignore_paths);
    let cl = claims::Claims::default();
    let v = build_verification(&cwd, &cfg, &facts, &git, &cl, false);
    let rec = make_receipt("manual", 1, &v, &facts, &git, &cl);
    let receipt_path = receipt::write(&cwd, &cfg, &rec);

    println!("stopproof {} — manual verification\n", VERSION);
    for c in &v.checks {
        let icon = match c.status.as_str() {
            "pass" => "PASS",
            "fail" => "FAIL",
            "warn" => "WARN",
            _ => "INFO",
        };
        println!("  [{}] {}: {}", icon, c.name, c.detail);
    }
    if let Some(p) = receipt_path {
        println!("\nreceipt: {}", p.display());
    }
    println!("\nverdict: {}", v.verdict.to_uppercase());
    if v.verdict == "pass" {
        0
    } else {
        1
    }
}
