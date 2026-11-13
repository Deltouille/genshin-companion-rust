//! A piece of text that must not leak: a cookie's value, an authkey.

use std::fmt;

use zeroize::Zeroize;

/// Text that is wiped from memory when dropped and never shown: `Debug` and
/// `Display` print a placeholder, so a secret in an error, a log or a panic
/// message stays hidden. Read it with [`Secret::expose`], only to send it.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The text itself, to put in a request.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(…)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_never_shows() {
        let secret = Secret::new("v2_hunter2");
        assert_eq!(format!("{secret}"), "…");
        assert!(!format!("{secret:?}").contains("hunter2"));
        assert_eq!(secret.expose(), "v2_hunter2");
    }
}
