//! The single sagex username rule.
//!
//! User-chosen names, also used as X.509 `CN=` values, JOSE `sub` claims,
//! and ledger `user_id`s. Rules: trimmed, 3–32 chars, ASCII alphanumerics
//! plus `_`, `-`, `.`.
//!
//! Deliberately ASCII-only (unlike chatsrv's Unicode-alphanumeric check):
//! the value is embedded in X.509 distinguished names and JOSE claims, where
//! non-ASCII buys nothing and risks encoding mismatches. Anything passing
//! this check also passes chatsrv's `valid_username`.

/// Returns true if `s` is an acceptable sagex username.
pub fn valid_username(s: &str) -> bool {
    let u = s.trim();
    if u.is_empty() {
        return false;
    }
    let len = u.chars().count();
    (3..=32).contains(&len)
        && u.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
}

#[cfg(test)]
mod tests {
    use super::valid_username;

    #[test]
    fn accepts_usernames() {
        for good in [
            "alice",
            "bob",
            "sagex-ca",
            "user_7",
            "a.b-c_d",
            "abc",
            "a-really-long-name-1234567890",
        ] {
            assert!(valid_username(good), "{good:?} should pass");
        }
    }

    #[test]
    fn rejects_non_usernames() {
        for bad in [
            "",
            "  ",
            "ab",
            "evil;cn=x",
            "alice bob",
            "alice@example.com",
            "CN=alice",
            "éclair",
            "a",
        ] {
            assert!(!valid_username(bad), "{bad:?} should fail");
        }
        assert!(!valid_username(&"x".repeat(33)));
        // Leading/trailing whitespace is trimmed, so padded names pass
        // only when the trimmed core is valid.
        assert!(valid_username("  alice  "));
        assert!(!valid_username("  ab  "));
    }
}
