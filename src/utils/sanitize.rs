//! Tool result sanitization.
//!
//! Strips base64 data URIs, long hex blobs, and truncates oversized
//! results before feeding them back to the LLM. This saves tokens
//! without losing meaningful information.

use regex::Regex;
use once_cell::sync::Lazy;

/// Default maximum result size in bytes (50 KB).
pub const DEFAULT_MAX_RESULT_BYTES: usize = 51_200;

/// Minimum length of a contiguous hex string to be stripped.
const MIN_HEX_BLOB_LEN: usize = 200;

static BASE64_URI_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"data:[a-zA-Z0-9/+\-\.]+;base64,[A-Za-z0-9+/=]+").unwrap()
});

static HEX_BLOB_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(r"[0-9a-fA-F]{{{},}}", MIN_HEX_BLOB_LEN)).unwrap()
});

/// Sanitize a tool result string.
///
/// 1. Replace `data:...;base64,...` URIs with a placeholder.
/// 2. Replace hex blobs (>= 200 hex chars) with a placeholder.
/// 3. Truncate to `max_bytes` if still too large.
pub fn sanitize_tool_result(result: &str, max_bytes: usize) -> String {
    let mut out = BASE64_URI_RE
        .replace_all(result, |caps: &regex::Captures| {
            let len = caps[0].len();
            format!("[base64 data removed, {} bytes]", len)
        })
        .into_owned();

    out = HEX_BLOB_RE
        .replace_all(&out, |caps: &regex::Captures| {
            let len = caps[0].len();
            format!("[hex data removed, {} chars]", len)
        })
        .into_owned();

    if out.len() > max_bytes {
        let total = out.len();
        out.truncate(max_bytes);
        // Ensure we don't split a multi-byte char
        while !out.is_char_boundary(out.len()) {
            out.pop();
        }
        out.push_str(&format!("\n...[truncated, {} total bytes]", total));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_change_for_normal_text() {
        let input = "Hello, world! This is a normal tool result.";
        assert_eq!(sanitize_tool_result(input, DEFAULT_MAX_RESULT_BYTES), input);
    }

    #[test]
    fn test_strips_base64_data_uri() {
        let b64 = "A".repeat(500);
        let input = format!("before data:image/png;base64,{} after", b64);
        let result = sanitize_tool_result(&input, DEFAULT_MAX_RESULT_BYTES);
        assert!(!result.contains(&b64));
        assert!(result.contains("[base64 data removed,"));
        assert!(result.contains("before"));
        assert!(result.contains("after"));
    }

    #[test]
    fn test_strips_hex_blob() {
        let hex = "a1b2c3d4e5f6".repeat(40); // 480 hex chars
        let input = format!("prefix {} suffix", hex);
        let result = sanitize_tool_result(&input, DEFAULT_MAX_RESULT_BYTES);
        assert!(!result.contains(&hex));
        assert!(result.contains("[hex data removed,"));
        assert!(result.contains("prefix"));
        assert!(result.contains("suffix"));
    }

    #[test]
    fn test_short_hex_not_stripped() {
        let hex = "abcdef1234"; // 10 chars, below threshold
        let input = format!("hash: {}", hex);
        let result = sanitize_tool_result(&input, DEFAULT_MAX_RESULT_BYTES);
        assert!(result.contains(hex));
    }

    #[test]
    fn test_truncation() {
        let input = "x".repeat(1000);
        let result = sanitize_tool_result(&input, 100);
        assert!(result.len() < 200); // 100 + truncation message
        assert!(result.contains("[truncated, 1000 total bytes]"));
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(sanitize_tool_result("", DEFAULT_MAX_RESULT_BYTES), "");
    }

    #[test]
    fn test_multiple_base64_uris() {
        let b64 = "Q".repeat(100);
        let input = format!(
            "img1: data:image/png;base64,{} and img2: data:application/pdf;base64,{}",
            b64, b64
        );
        let result = sanitize_tool_result(&input, DEFAULT_MAX_RESULT_BYTES);
        assert!(!result.contains(&b64));
        // Should have two replacement markers
        assert_eq!(result.matches("[base64 data removed,").count(), 2);
    }
}
