//! Ground truth from git: what actually changed on disk.

use std::path::Path;
use std::process::Command;

#[derive(Debug, Default)]
pub struct GitFacts {
    pub in_repo: bool,
    /// Uncommitted paths (staged, unstaged, untracked), repo-relative.
    pub changed: Vec<String>,
    /// Paths touched by commits made since the session started.
    pub committed_since: Vec<String>,
    /// Test files deleted in the working tree.
    pub deleted_tests: Vec<String>,
    /// Assertion-ish lines removed minus added across changed test files.
    pub assertions_removed: i64,
    pub assertions_added: i64,
}

fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        None
    }
}

pub fn is_test_path(p: &str) -> bool {
    let l = p.to_lowercase();
    l.contains("test") || l.contains("spec") || l.contains("__tests__")
}

fn parse_porcelain(out: &str, ignore: &[String]) -> (Vec<String>, Vec<String>) {
    let mut changed = Vec::new();
    let mut deleted_tests = Vec::new();
    for line in out.lines() {
        if line.len() < 4 {
            continue;
        }
        let status = &line[..2];
        let mut path = line[3..].trim().to_string();
        if let Some(idx) = path.find(" -> ") {
            path = path[idx + 4..].to_string();
        }
        let path = path.trim_matches('"').to_string();
        if path.is_empty() || ignore.iter().any(|ig| !ig.is_empty() && path.contains(ig)) {
            continue;
        }
        if status.contains('D') && is_test_path(&path) {
            deleted_tests.push(path.clone());
        }
        if !changed.contains(&path) {
            changed.push(path);
        }
    }
    (changed, deleted_tests)
}

const ASSERT_MARKERS: [&str; 8] = [
    "assert", "expect(", "#[test]", "def test_", "it(", ".test(", "test(", "should",
];

fn count_assertion_lines(diff: &str) -> (i64, i64) {
    let mut removed = 0i64;
    let mut added = 0i64;
    for line in diff.lines() {
        if line.starts_with("---") || line.starts_with("+++") {
            continue;
        }
        let (sign, body) = match line.chars().next() {
            Some('-') => (-1, &line[1..]),
            Some('+') => (1, &line[1..]),
            _ => continue,
        };
        let l = body.to_lowercase();
        if ASSERT_MARKERS.iter().any(|m| l.contains(m)) {
            if sign < 0 {
                removed += 1;
            } else {
                added += 1;
            }
        }
    }
    (removed, added)
}

pub fn collect(cwd: &Path, since_epoch: Option<u64>, ignore: &[String]) -> GitFacts {
    let mut facts = GitFacts::default();

    match git(cwd, &["rev-parse", "--is-inside-work-tree"]) {
        Some(out) if out.trim() == "true" => facts.in_repo = true,
        _ => return facts,
    }

    if let Some(out) = git(cwd, &["status", "--porcelain"]) {
        let (changed, deleted_tests) = parse_porcelain(&out, ignore);
        facts.changed = changed;
        facts.deleted_tests = deleted_tests;
    }

    if let Some(epoch) = since_epoch {
        let since = crate::timefmt::iso_utc(epoch);
        if let Some(out) = git(
            cwd,
            &[
                "log",
                "--name-only",
                "--pretty=format:",
                &format!("--since={}", since),
            ],
        ) {
            for line in out.lines() {
                let p = line.trim().to_string();
                if !p.is_empty()
                    && !ignore.iter().any(|ig| !ig.is_empty() && p.contains(ig))
                    && !facts.committed_since.contains(&p)
                {
                    facts.committed_since.push(p);
                }
            }
        }
    }

    // Assertion-weakening heuristic over changed test files (vs HEAD).
    let test_files: Vec<String> = facts
        .changed
        .iter()
        .filter(|p| is_test_path(p))
        .cloned()
        .collect();
    if !test_files.is_empty() && git(cwd, &["rev-parse", "--verify", "HEAD"]).is_some() {
        let mut args: Vec<&str> = vec!["diff", "HEAD", "-U0", "--"];
        let refs: Vec<&str> = test_files.iter().map(|s| s.as_str()).collect();
        args.extend(refs);
        if let Some(diff) = git(cwd, &args) {
            let (removed, added) = count_assertion_lines(&diff);
            facts.assertions_removed = removed;
            facts.assertions_added = added;
        }
    }

    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_parsing() {
        let out = " M src/a.rs\n?? new.txt\n D tests/old_test.py\nR  old.rs -> new.rs\n";
        let (changed, deleted_tests) = parse_porcelain(out, &[]);
        assert!(changed.contains(&"src/a.rs".to_string()));
        assert!(changed.contains(&"new.txt".to_string()));
        assert!(changed.contains(&"tests/old_test.py".to_string()));
        assert!(changed.contains(&"new.rs".to_string()));
        assert!(!changed.contains(&"old.rs".to_string()));
        assert_eq!(deleted_tests, vec!["tests/old_test.py".to_string()]);
    }

    #[test]
    fn porcelain_respects_ignores() {
        let out = " M .stopproof/receipt.json\n M src/a.rs\n";
        let (changed, _) = parse_porcelain(out, &[".stopproof/".to_string()]);
        assert_eq!(changed, vec!["src/a.rs".to_string()]);
    }

    #[test]
    fn assertion_counting() {
        let diff = "--- a/tests/t.py\n+++ b/tests/t.py\n-    assert x == 1\n-    assert y == 2\n+    assert x == 1\n context line\n";
        let (removed, added) = count_assertion_lines(diff);
        assert_eq!(removed, 2);
        assert_eq!(added, 1);
    }

    #[test]
    fn test_path_detection() {
        assert!(is_test_path("tests/auth_test.rs"));
        assert!(is_test_path("src/__tests__/x.js"));
        assert!(is_test_path("spec/user_spec.rb"));
        assert!(!is_test_path("src/main.rs"));
    }

    #[test]
    fn non_repo_dir() {
        let root = if cfg!(windows) { "C:\\" } else { "/" };
        let facts = collect(Path::new(root), None, &[]);
        // "/" may or may not be a repo on exotic setups; just ensure no panic
        // and coherent defaults when it isn't.
        if !facts.in_repo {
            assert!(facts.changed.is_empty());
        }
    }
}
