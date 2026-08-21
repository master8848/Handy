//! Built-in vocabulary datasets: curated term lists compiled into the binary
//! as text files (one term per line, `#` for comment lines). They are injected
//! into settings on first load — see `settings::ensure_builtin_datasets` — and
//! behave like read-only user datasets (toggleable, not editable).
//!
//! Term files live in `src-tauri/src/vocab_data/` and are curated by
//! contributors (see `.tmp/domain_plan.md` for the curation rules).

use crate::settings::CustomWordDataset;

/// Parses a vocabulary file: one term per line. Blank lines and `#`-prefixed
/// comment lines are ignored; lines are trimmed, deduplicated
/// case-insensitively, and lines longer than 100 chars are dropped.
pub fn parse_terms(source: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut terms = Vec::new();
    for line in source.lines() {
        let term = line.trim();
        if term.is_empty() || term.starts_with('#') || term.chars().count() > 100 {
            continue;
        }
        if seen.insert(term.to_lowercase()) {
            terms.push(term.to_string());
        }
    }
    terms
}

macro_rules! builtin_dataset {
    ($id:literal, $name:literal, $file:literal) => {
        CustomWordDataset {
            id: $id.to_string(),
            name: $name.to_string(),
            words: parse_terms(include_str!(concat!("../vocab_data/", $file))),
            builtin: true,
            enabled: false,
        }
    };
}

/// Returns all datasets bundled with the app. Built-ins default to disabled —
/// users opt in per dataset, keeping the Whisper prompt and fuzzy matcher
/// focused on the domains they actually speak about.
pub fn builtin_datasets() -> Vec<CustomWordDataset> {
    vec![
        builtin_dataset!("frontend_dev", "Frontend Development", "frontend_dev.txt"),
        builtin_dataset!("ai_dev", "AI & Machine Learning", "ai_dev.txt"),
        builtin_dataset!("backend_dev", "Backend Development", "backend_dev.txt"),
        builtin_dataset!("cloud_devops", "Cloud & DevOps", "cloud_devops.txt"),
        builtin_dataset!("databases", "Databases & Data", "databases.txt"),
        builtin_dataset!("cybersecurity", "Cybersecurity", "cybersecurity.txt"),
        builtin_dataset!("mobile_native", "Mobile & Native", "mobile_native.txt"),
        builtin_dataset!("git_tools", "Git & Tools", "git_tools.txt"),
        builtin_dataset!("game_dev", "Game Development", "game_dev.txt"),
        builtin_dataset!(
            "academic_science",
            "Academic & Science",
            "academic_science.txt"
        ),
        builtin_dataset!("finance", "Finance & Accounting", "finance.txt"),
        builtin_dataset!("legal", "Legal", "legal.txt"),
        builtin_dataset!("medical", "Medical", "medical.txt"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_terms_skips_comments_blanks_and_duplicates() {
        let terms = parse_terms(
            "# source: example\n\nReact\nTailwind CSS\nreact\n\n# another comment\nLLM\n",
        );
        assert_eq!(terms, vec!["React", "Tailwind CSS", "LLM"]);
    }

    #[test]
    fn parse_terms_drops_overlong_lines() {
        let terms = parse_terms(&format!("{}\nshort", "x".repeat(101)));
        assert_eq!(terms, vec!["short"]);
    }

    #[test]
    fn builtin_datasets_are_non_empty_and_prompt_safe() {
        for dataset in builtin_datasets() {
            assert!(
                !dataset.words.is_empty(),
                "builtin dataset '{}' is empty",
                dataset.id
            );
            assert!(dataset.builtin);
            assert!(!dataset.enabled);
            for word in &dataset.words {
                assert!(
                    word.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-'),
                    "term '{word}' in '{}' contains disallowed characters",
                    dataset.id
                );
            }
        }
    }
}
