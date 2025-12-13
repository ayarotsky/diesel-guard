use regex::Regex;
use std::sync::LazyLock;

/// Regex pattern to detect ALTER TABLE ... ADD CONSTRAINT ... PRIMARY KEY USING INDEX
static PRIMARY_KEY_USING_INDEX_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)ALTER\s+TABLE\s+\S+\s+ADD\s+CONSTRAINT\s+\S+\s+PRIMARY\s+KEY\s+USING\s+INDEX\s+\S+",
    )
    .unwrap()
});

/// Check if SQL contains PRIMARY KEY USING INDEX syntax that sqlparser can't parse
pub fn contains_primary_key_using_index(sql: &str) -> bool {
    PRIMARY_KEY_USING_INDEX_PATTERN.is_match(sql)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_primary_key_using_index() {
        assert!(contains_primary_key_using_index(
            "ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_pkey;"
        ));
    }

    #[test]
    fn test_detects_case_insensitive() {
        assert!(contains_primary_key_using_index(
            "alter table users add constraint pk PRIMARY KEY using index idx;"
        ));
    }

    #[test]
    fn test_ignores_regular_primary_key() {
        assert!(!contains_primary_key_using_index(
            "ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY (id);"
        ));
    }

    #[test]
    fn test_ignores_create_unique_index() {
        assert!(!contains_primary_key_using_index(
            "CREATE UNIQUE INDEX CONCURRENTLY idx ON users(id);"
        ));
    }
}
