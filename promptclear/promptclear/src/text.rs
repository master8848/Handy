// PromptClear — offline voice notes. MIT.
//!
//! Char-boundary-safe string helpers for the streaming transcription path.
//! Streaming events append `committed` tails and the finalize path replaces
//! them; raw byte slicing there would panic on multi-byte UTF-8 (emoji, CJK)
//! the moment a guard assumption breaks, so all slicing goes through `str`
//! methods that can never land mid-character.

/// The largest `index <= limit` that is a char boundary of `text`.
pub fn floor_char_boundary(text: &str, limit: usize) -> usize {
    let len = text.len();
    if limit >= len {
        return len;
    }
    if text.is_char_boundary(limit) {
        return limit;
    }
    let mut index = limit;
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// Truncate `target` to at most `limit` bytes, snapping to a char boundary.
pub fn truncate(target: &mut String, limit: usize) {
    let len = floor_char_boundary(target, limit);
    target.truncate(len);
}

/// Remove a trailing `suffix` from `target` (snapping to a char boundary).
///
/// Returns `true` if the suffix was removed. This is the safe equivalent of
/// `target.truncate(target.len() - suffix.len())` for the common
/// "committed stream text was appended verbatim" pattern.
pub fn truncate_suffix(target: &mut String, suffix: &str) -> bool {
    match target.strip_suffix(suffix) {
        Some(rest) => {
            target.truncate(rest.len());
            true
        }
        None => false,
    }
}

/// Append the part of `new_text` that follows the `committed` prefix.
///
/// The streaming engine hands out monotonically growing `committed` strings;
/// `target` already contains the previous `committed`, so the tail is what
/// should be appended. Returns `false` (and appends nothing) when `committed`
/// is not actually a prefix of `new_text`, or when the slice would be
/// ill-formed. The slice itself is taken with `str::get`, which can never
/// panic on an invalid char boundary.
pub fn append_continuation(target: &mut String, new_text: &str, committed: &str) -> bool {
    if !new_text.starts_with(committed) {
        return false;
    }
    if let Some(tail) = new_text.get(committed.len()..) {
        target.push_str(tail);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_boundary_with_emoji_and_cjk() {
        // "a" (1 byte) + "😀" (4 bytes) + "日" (3 bytes) + "b" (1 byte)
        let text = "a😀日b";
        assert_eq!(text.len(), 9);
        // Valid boundaries: 0, 1, 5, 8, 9.
        assert_eq!(floor_char_boundary(text, 0), 0);
        assert_eq!(floor_char_boundary(text, 1), 1);
        assert_eq!(floor_char_boundary(text, 2), 1);
        assert_eq!(floor_char_boundary(text, 3), 1);
        assert_eq!(floor_char_boundary(text, 4), 1);
        assert_eq!(floor_char_boundary(text, 5), 5);
        assert_eq!(floor_char_boundary(text, 6), 5);
        assert_eq!(floor_char_boundary(text, 7), 5);
        assert_eq!(floor_char_boundary(text, 8), 8);
        assert_eq!(floor_char_boundary(text, 9), 9);
        assert_eq!(floor_char_boundary(text, 100), 9);
    }

    #[test]
    fn truncate_at_char_boundary() {
        let mut text = "a😀日b".to_string();
        truncate(&mut text, 2); // mid-emoji
        assert_eq!(text, "a");
        truncate(&mut text, 0);
        assert_eq!(text, "");
    }

    #[test]
    fn truncate_suffix_removes_committed_verbatim() {
        let mut text = "hello world😀".to_string();
        assert!(truncate_suffix(&mut text, "world😀"));
        assert_eq!(text, "hello ");
        // Not a suffix: no-op.
        assert!(!truncate_suffix(&mut text, "😀"));
        assert_eq!(text, "hello ");
    }

    #[test]
    fn append_continuation_uses_char_safe_slice() {
        let mut target = "こんにち".to_string();
        // committed == "こんにち" (12 bytes); tail "は世界" appended.
        assert!(append_continuation(
            &mut target,
            "こんにちは世界",
            "こんにち"
        ));
        assert_eq!(target, "こんにちは世界");

        // committed not a prefix (engine rewrote the beginning): never panics,
        // appends nothing.
        let mut target2 = "abc".to_string();
        assert!(!append_continuation(&mut target2, "xyzabc", "abc"));
        assert_eq!(target2, "abc");
    }

    #[test]
    fn empty_and_ascii_edge_cases() {
        let mut text = String::new();
        // Empty suffix: vacuously true, and leaves the text untouched.
        assert!(truncate_suffix(&mut text, ""));
        truncate(&mut text, 0);
        assert!(text.is_empty());

        let mut target = "abc".to_string();
        assert!(append_continuation(&mut target, "abcd", "abc"));
        assert_eq!(target, "abcd");
        // Empty tail is a valid continuation.
        assert!(append_continuation(&mut target, "abcd", "abcd"));
        assert_eq!(target, "abcd");
    }
}
