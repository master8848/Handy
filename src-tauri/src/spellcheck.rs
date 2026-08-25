//! Offline grammar + spelling checking, powered by Harper
//! (Apache-2.0, https://github.com/Automattic/harper).
//!
//! The curated [`LintGroup`] (embedded dictionary + rule set) is built lazily on
//! first use — the dictionary parse is the expensive part — and cached behind a
//! mutex for the app's lifetime. Every check runs in a blocking worker via
//! [`commands::spellcheck::check_spelling`], never on the UI thread.

use harper_core::linting::{LintGroup, Linter};
use harper_core::parsers::PlainEnglish;
use harper_core::spell::FstDictionary;
use harper_core::{Dialect, Document};
use log::debug;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Mutex;
use tauri::AppHandle;

/// A single issue found by the checker.
///
/// Offsets are UTF-16 code units so the frontend can slice JavaScript strings
/// directly (`text.slice(start, end)`), matching how the streaming overlay
/// measures text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SpellingIssue {
    /// UTF-16 code-unit offset of the start of the flagged span.
    pub start: usize,
    /// UTF-16 code-unit offset of the end (exclusive) of the flagged span.
    pub end: usize,
    /// Category of the issue (`Spelling`, `Grammar`, `Punctuation`, ...).
    pub kind: String,
    /// Human-readable description of the issue.
    pub message: String,
    /// Replacement texts that would resolve the issue (empty when unknown).
    pub suggestions: Vec<String>,
    /// Importance of the issue; lower is more important.
    pub priority: u8,
}

/// Lazily-initialized Harper lint group, shared as managed Tauri state.
pub struct SpellChecker {
    linter: Mutex<Option<LintGroup>>,
    /// App handle for reading persisted settings at check time. Absent in unit
    /// tests, where checking runs ungated.
    app_handle: Option<AppHandle>,
}

impl SpellChecker {
    #[cfg(test)]
    pub fn new() -> Self {
        Self {
            linter: Mutex::new(None),
            app_handle: None,
        }
    }

    pub fn with_app_handle(app_handle: AppHandle) -> Self {
        Self {
            linter: Mutex::new(None),
            app_handle: Some(app_handle),
        }
    }

    /// Whether the lint group has been built (and is ready for checks).
    pub fn is_initialized(&self) -> bool {
        self.linter
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Build the lint group now if it doesn't exist yet. Errors are logged, not
    /// propagated — initialization stays best-effort for a lazy component.
    pub fn ensure_initialized(&self) {
        if self.is_initialized() {
            return;
        }
        let result = self.linter.lock().map(|mut guard| {
            guard.get_or_insert_with(|| {
                LintGroup::new_curated(FstDictionary::curated(), Dialect::American)
            });
        });
        if let Err(e) = result {
            debug!("Failed to initialize spell checker: {}", e);
        }
    }

    /// Lint `text`, returning all spelling and grammar issues found.
    pub fn check(&self, text: &str) -> Result<Vec<SpellingIssue>, String> {
        if text.is_empty() {
            return Ok(Vec::new());
        }

        // Globally gate spell checking on the persisted setting so a single
        // toggle controls every surface (overlay, editor, ...).
        if let Some(app_handle) = &self.app_handle {
            if !crate::settings::get_settings(app_handle).spell_check_enabled {
                return Ok(Vec::new());
            }
        }

        let mut guard = self.linter.lock().map_err(|e| e.to_string())?;
        let linter = guard.get_or_insert_with(|| {
            LintGroup::new_curated(FstDictionary::curated(), Dialect::American)
        });

        let document = Document::new_curated(text, &PlainEnglish);
        let source_chars: Vec<char> = text.chars().collect();

        // UTF-16 code-unit offset of every char boundary, so span lookup is O(1).
        let mut offsets = Vec::with_capacity(source_chars.len() + 1);
        offsets.push(0);
        for c in &source_chars {
            offsets.push(offsets.last().copied().unwrap_or(0) + c.len_utf16());
        }

        Ok(linter
            .lint(&document)
            .into_iter()
            .map(|lint| {
                let mut suggestions: Vec<String> = lint
                    .suggestions
                    .iter()
                    .filter_map(|suggestion| {
                        let mut chars = source_chars.clone();
                        suggestion.apply(lint.span, &mut chars);
                        // `apply` mutates the whole char vector in place; the
                        // replacement itself is whatever now occupies the span.
                        // Clamp: Harper may elide junk (e.g. emoji) from its
                        // internal source, stretching spans past the end.
                        let replacement: String = chars
                            .get(lint.span.start..lint.span.end.min(chars.len()))
                            .unwrap_or_default()
                            .iter()
                            .collect();
                        (!replacement.is_empty()).then_some(replacement)
                    })
                    .collect();
                suggestions.dedup();

                SpellingIssue {
                    start: offsets.get(lint.span.start).copied().unwrap_or(0),
                    end: offsets
                        .get(lint.span.end)
                        .copied()
                        .unwrap_or(offsets.len() - 1),
                    kind: format!("{:?}", lint.lint_kind),
                    message: lint.message,
                    suggestions,
                    priority: lint.priority,
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issues(text: &str) -> Vec<SpellingIssue> {
        SpellChecker::new().check(text).expect("check succeeds")
    }

    /// Slice `text` by UTF-16 code-unit offsets (same convention as JS strings).
    fn slice_utf16(text: &str, start: usize, end: usize) -> &str {
        let mut byte_start = 0usize;
        let mut byte_end = text.len();
        let mut offset = 0usize;
        for (byte_idx, c) in text.char_indices() {
            if offset == start {
                byte_start = byte_idx;
            }
            if offset == end {
                byte_end = byte_idx;
                break;
            }
            offset += c.len_utf16();
        }
        &text[byte_start..byte_end]
    }

    fn issue_on(text: &str, word: &str) -> Option<SpellingIssue> {
        issues(text)
            .into_iter()
            .find(|i| slice_utf16(text, i.start, i.end) == word)
    }

    #[test]
    fn clean_text_has_no_issues() {
        assert!(
            issues("The quick brown fox jumps over the lazy dog.").is_empty(),
            "clean text should not be flagged"
        );
    }

    #[test]
    fn empty_text_has_no_issues() {
        assert!(issues("").is_empty());
    }

    #[test]
    fn flags_misspellings_with_suggestions() {
        let issue = issue_on("This is a misspelled sentence with heloo in it.", "heloo")
            .expect("'heloo' should be flagged");
        assert_eq!(issue.kind, "Spelling");
        assert!(
            issue.suggestions.iter().any(|s| s == "hello"),
            "expected 'hello' among suggestions, got {:?}",
            issue.suggestions
        );
    }

    #[test]
    fn offsets_are_utf16_code_units() {
        // 😀 is 2 UTF-16 units, so 'heloo' starts at 3 (after "😀 ") and ends at 8.
        let issue = issue_on("😀 heloo", "heloo").expect("'heloo' should be flagged");
        assert_eq!((issue.start, issue.end), (3, 8));
    }

    #[test]
    fn non_latin_text_is_not_flagged_as_spelling_errors() {
        // Harper is English-only; CJK text must not produce a wall of errors.
        assert!(issues("这是一段测试文本。").is_empty());
    }

    #[test]
    fn emoji_only_text_is_safe() {
        assert!(issues("😀🎉🚀").is_empty());
    }
}
