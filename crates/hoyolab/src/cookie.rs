//! The player's HoYoLAB cookie, as pasted: a request's `Cookie` header from
//! the browser's network tool (`ltoken_v2` is HttpOnly, so the console's
//! `document.cookie` lacks it). Only the fields HoYoLAB needs are kept.

use std::fmt;

use crate::{Error, Result, Secret};

/// The cookie fields kept, in the order they are sent. The `_v2` ones are
/// today's; the older names still work for some accounts.
const KEPT: [&str; 10] = [
    "ltuid_v2",
    "ltoken_v2",
    "ltmid_v2",
    "cookie_token_v2",
    "account_id_v2",
    "account_mid_v2",
    "ltuid",
    "ltoken",
    "cookie_token",
    "account_id",
];

/// What HoYoLAB needs of the player's cookie: each field a [`Secret`].
#[derive(Clone)]
pub struct Cookie {
    fields: Vec<(&'static str, Secret)>,
}

impl Cookie {
    /// Reads a pasted cookie: `name=value` pairs separated by `;`, with or
    /// without a leading `Cookie:`. Fields other than HoYoLAB's are left
    /// out. Fails when the pair that logs in (`ltuid_v2` and `ltoken_v2`,
    /// or the older `ltuid` and `ltoken`) is missing.
    pub fn parse(pasted: &str) -> Result<Self> {
        let text = pasted.trim();
        let text = text
            .strip_prefix("Cookie:")
            .or_else(|| text.strip_prefix("cookie:"))
            .unwrap_or(text);
        let mut fields = Vec::new();
        for pair in text.split(';') {
            let Some((name, value)) = pair.split_once('=') else {
                continue;
            };
            let (name, value) = (name.trim(), value.trim());
            if value.is_empty() {
                continue;
            }
            if let Some(kept) = KEPT.iter().find(|kept| **kept == name)
                && !fields.iter().any(|(n, _)| n == kept)
            {
                fields.push((*kept, Secret::new(value)));
            }
        }
        fields.sort_by_key(|(name, _)| KEPT.iter().position(|kept| kept == name));
        let cookie = Self { fields };
        let logs_in = |uid, token| cookie.has(uid) && cookie.has(token);
        if !(logs_in("ltuid_v2", "ltoken_v2") || logs_in("ltuid", "ltoken")) {
            return Err(Error::MissingCookie("ltuid_v2 and ltoken_v2"));
        }
        Ok(cookie)
    }

    /// Whether the cookie has field `name`.
    pub fn has(&self, name: &str) -> bool {
        self.fields.iter().any(|(n, _)| *n == name)
    }

    /// The names of the fields kept, without their values.
    pub fn names(&self) -> Vec<&'static str> {
        self.fields.iter().map(|(name, _)| *name).collect()
    }

    /// Whether it can ask for an authkey: `cookie_token_v2` with
    /// `account_id_v2` (or the older `cookie_token` with `account_id`).
    pub fn can_make_authkeys(&self) -> bool {
        (self.has("cookie_token_v2") && self.has("account_id_v2"))
            || (self.has("cookie_token") && self.has("account_id"))
    }

    /// The `Cookie` header to send.
    pub fn header(&self) -> Secret {
        let pairs: Vec<String> = self
            .fields
            .iter()
            .map(|(name, value)| format!("{name}={}", value.expose()))
            .collect();
        Secret::new(pairs.join("; "))
    }
}

impl fmt::Debug for Cookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Cookie").field(&self.names()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_hoyolabs_fields_are_kept() {
        let cookie = Cookie::parse(
            "Cookie: _ga=GA1.1; mi18nLang=fr-fr; ltuid_v2=123; ltoken_v2=v2_abc; \
             ltmid_v2=mid; DEVICEFP=xyz; cookie_token_v2=v2_ct; account_id_v2=123",
        )
        .unwrap();
        assert_eq!(
            cookie.names(),
            [
                "ltuid_v2",
                "ltoken_v2",
                "ltmid_v2",
                "cookie_token_v2",
                "account_id_v2"
            ]
        );
        assert!(cookie.can_make_authkeys());
        assert_eq!(
            cookie.header().expose(),
            "ltuid_v2=123; ltoken_v2=v2_abc; ltmid_v2=mid; cookie_token_v2=v2_ct; account_id_v2=123"
        );
        // The values never show.
        assert!(!format!("{cookie:?}").contains("v2_abc"));
    }

    #[test]
    fn a_cookie_that_cannot_log_in_is_refused() {
        let error = Cookie::parse("_ga=GA1.1; ltuid_v2=123").unwrap_err();
        assert!(matches!(error, Error::MissingCookie(_)));
        assert!(Cookie::parse("").is_err());
        // The older names log in too, but make no authkey alone.
        let older = Cookie::parse("ltuid=1; ltoken=abc").unwrap();
        assert!(!older.can_make_authkeys());
    }
}
