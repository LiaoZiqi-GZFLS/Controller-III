//! Pattern filtering

use regex::{Regex, RegexBuilder};
use crate::search::entry::FileEntry;

/// Check if file entry matches the search pattern
pub fn matches_pattern(entry: &FileEntry, pattern: &Regex) -> bool {
    pattern.is_match(&entry.file_name)
}

/// Convert user input search query to regex
/// Supports simple glob-like patterns: * matches any characters
pub fn query_to_regex(query: &str, case_sensitive: bool) -> Regex {
    let mut regex_pattern = String::new();

    // If query contains no glob characters, match anywhere
    if !query.contains('*') && !query.contains('?') {
        regex_pattern.push_str(".*");
        regex_pattern.push_str(&escape_regex(query));
        regex_pattern.push_str(".*");
    } else {
        // Convert glob patterns to regex
        let mut chars = query.chars().peekable();
        let first_char = query.chars().next().unwrap();
        // Anchor at start if it doesn't start with *
        if first_char != '*' {
            regex_pattern.push('^');
        }
        while let Some(c) = chars.next() {
            match c {
                '*' => regex_pattern.push_str(".*"),
                '?' => regex_pattern.push_str("."),
                '.' => regex_pattern.push_str("\\."),
                '\\' => regex_pattern.push_str("\\\\"),
                c if regex_special_char(c) => {
                    regex_pattern.push('\\');
                    regex_pattern.push(c);
                }
                c => regex_pattern.push(c),
            }
        }
        let last_char = query.chars().last().unwrap();
        // Anchor at end if it doesn't end with *
        if last_char != '*' {
            regex_pattern.push('$');
        }
    }

    let mut builder = regex::RegexBuilder::new(&regex_pattern);
    builder.case_insensitive(!case_sensitive);

    builder.build().unwrap_or_else(|_| {
        // Fallback: match the raw query as literal
        let fallback = format!(".{}.*", escape_regex(query));
        RegexBuilder::new(&fallback)
            .case_insensitive(!case_sensitive)
            .build()
            .unwrap()
    })
}

fn regex_special_char(c: char) -> bool {
    matches!(c, '^' | '$' | '.' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '+' | '-')
}

fn escape_regex(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len() + 10);
    for c in s.chars() {
        if regex_special_char(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_to_regex_no_globs() {
        let re = query_to_regex("test", false);
        assert!(re.is_match("mytestfile.txt"));
        assert!(re.is_match("testfile.txt"));
        assert!(re.is_match("my_test.txt"));
    }

    #[test]
    fn test_query_to_regex_prefix() {
        let re = query_to_regex("test*", false);
        assert!(re.is_match("testfile.txt"));
        assert!(!re.is_match("mytest.txt"));
    }

    #[test]
    fn test_query_to_regex_suffix() {
        let re = query_to_regex("*.txt", false);
        assert!(re.is_match("file.txt"));
        assert!(re.is_match("test.txt"));
        assert!(!re.is_match("file.pdf"));
    }
}
