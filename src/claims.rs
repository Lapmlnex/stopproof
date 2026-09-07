//! Extract completion claims and file-path mentions from the agent's
//! final message. Pure string processing — no LLM, no regex crate.

const BASE_KEYWORDS: [&str; 16] = [
    "done",
    "complete",
    "completed",
    "implemented",
    "finished",
    "fixed",
    "added",
    "created",
    "updated",
    "ready",
    "passing",
    "all tests pass",
    "tests pass",
    "works as expected",
    "successfully",
    "should now work",
];

const PATH_EXTENSIONS: [&str; 30] = [
    ".rs", ".py", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".go", ".java", ".rb", ".php",
    ".c", ".h", ".cpp", ".hpp", ".cs", ".swift", ".kt", ".md", ".json", ".toml", ".yml", ".yaml",
    ".css", ".html", ".sh", ".sql", ".txt", ".vue",
];

#[derive(Debug, Default)]
pub struct Claims {
    pub completion_claimed: bool,
    pub matched_keywords: Vec<String>,
    pub claimed_paths: Vec<String>,
}

/// True if `word` occurs in `text` bounded by non-alphanumeric characters.
fn contains_word(text: &str, word: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = text[start..].find(word) {
        let abs = start + pos;
        let before_ok = abs == 0
            || !text[..abs]
                .chars()
                .next_back()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);
        let after = abs + word.len();
        let after_ok = after >= text.len()
            || !text[after..]
                .chars()
                .next()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);
        if before_ok && after_ok {
            return true;
        }
        start = abs + word.len().max(1);
        if start >= text.len() {
            break;
        }
    }
    false
}

fn looks_like_path(token: &str) -> bool {
    if token.len() < 3 || token.len() > 200 {
        return false;
    }
    if token.starts_with("http://") || token.starts_with("https://") {
        return false;
    }
    let has_ext = PATH_EXTENSIONS.iter().any(|e| token.ends_with(e));
    let has_slash = token.contains('/');
    // "src/main.rs" (slash + dot) or bare "main.rs" (known extension)
    (has_slash && token.contains('.')) || has_ext
}

pub fn extract(final_text: &str, extra_keywords: &[String]) -> Claims {
    let mut claims = Claims::default();
    let lower = final_text.to_lowercase();

    for kw in BASE_KEYWORDS
        .iter()
        .map(|s| s.to_string())
        .chain(extra_keywords.iter().map(|s| s.to_lowercase()))
    {
        if kw.is_empty() {
            continue;
        }
        let hit = if kw.contains(' ') {
            lower.contains(&kw)
        } else {
            contains_word(&lower, &kw)
        };
        if hit {
            claims.matched_keywords.push(kw);
        }
    }
    claims.completion_claimed = !claims.matched_keywords.is_empty();

    for raw in final_text.split_whitespace() {
        let token = raw
            .trim_matches(|c: char| {
                matches!(
                    c,
                    '`' | '('
                        | ')'
                        | '"'
                        | '\''
                        | '*'
                        | ','
                        | ';'
                        | ':'
                        | '['
                        | ']'
                        | '<'
                        | '>'
                        | '!'
                        | '?'
                )
            })
            .trim_end_matches('.');
        let token = token.strip_prefix("./").unwrap_or(token);
        if looks_like_path(token) {
            let t = token.to_string();
            if !claims.claimed_paths.contains(&t) {
                claims.claimed_paths.push(t);
            }
        }
    }

    claims
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_completion_and_paths() {
        let c = extract(
            "Done! I updated `src/auth.rs` and added tests in tests/auth_test.rs.",
            &[],
        );
        assert!(c.completion_claimed);
        assert!(c.claimed_paths.contains(&"src/auth.rs".to_string()));
        assert!(c.claimed_paths.contains(&"tests/auth_test.rs".to_string()));
    }

    #[test]
    fn word_boundaries_respected() {
        // "abandoned" must not match "done"
        let c = extract("The abandoned branch remains untouched.", &[]);
        assert!(!c.completion_claimed);
    }

    #[test]
    fn urls_are_not_paths() {
        let c = extract("See https://example.com/a.rs for done reference", &[]);
        assert!(!c.claimed_paths.iter().any(|p| p.contains("example.com")));
    }

    #[test]
    fn custom_keywords() {
        let c = extract("ship it", &["ship it".to_string()]);
        assert!(c.completion_claimed);
    }

    #[test]
    fn neutral_text_has_no_claims() {
        let c = extract("What would you like me to look at next?", &[]);
        assert!(!c.completion_claimed);
        assert!(c.claimed_paths.is_empty());
    }
}
