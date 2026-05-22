/// Trim a prefix from a string, returning the original if it doesn't match.
pub fn trim_prefix(s: &str, prefix: &str) -> String {
    s.strip_prefix(prefix)
        .map(String::from)
        .unwrap_or_else(|| s.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_prefix() {
        assert_eq!(trim_prefix("hello", "he"), "llo");
        assert_eq!(trim_prefix("hello", "xx"), "hello");
        assert_eq!(trim_prefix("", "xx"), "");
    }
}