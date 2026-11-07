//! Commit-hash validation.
//!
//! Every hash is validated before it is interpolated into a git command line, so
//! a pasted `--help` or `; rm -rf /` can never be interpreted as an argument.

/// Accepts 4-64 hex characters: git's abbreviated hashes through SHA-256.
pub fn is_valid_hash(hash: &str) -> bool {
    let len = hash.len();
    (4..=64).contains(&len) && hash.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::is_valid_hash;

    #[test]
    fn accepts_abbreviated_and_full_hashes() {
        assert!(is_valid_hash("a43c821"));
        assert!(is_valid_hash("a43c821aa11bb22cc33dd44ee55ff66aa77bb88"));
        // 64-char SHA-256 repositories
        assert!(is_valid_hash(&"a".repeat(64)));
    }

    #[test]
    fn rejects_anything_that_could_be_an_option_or_injection() {
        for bad in ["", "a3c", "--help", "-x", "HEAD~1", "main", "a43c821; rm -rf /", "a43c 821", "$(whoami)"] {
            assert!(!is_valid_hash(bad), "accepted {bad:?}");
        }
    }

    #[test]
    fn rejects_overlong_input() {
        assert!(!is_valid_hash(&"a".repeat(65)));
    }
}
