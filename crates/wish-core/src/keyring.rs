//! The system keyring, where the player may ask the app to keep their HoYoLAB
//! login, so the chronicle syncs without pasting the cookie each time: the
//! Keychain on macOS, the Credential Manager on Windows, the Secret Service
//! (GNOME Keyring, KWallet, KeePassXC) elsewhere.
//!
//! What is kept is [`Cookie::login`]: `ltuid_v2`, `ltoken_v2` and
//! `ltmid_v2`, never the cookie token. Nowhere else: without a keyring the
//! app keeps nothing (no file instead), and nothing here prints it. The
//! keyring encrypts it at rest; it doesn't keep it from a program running as
//! the player.
//!
//! [`Keyring`] is what the app reaches it through, so tests give a fake
//! ([`MemoryKeyring`]) and never touch the player's keyring.

use std::sync::{Arc, Mutex, OnceLock};

use hoyolab::{Cookie, Secret};

use crate::error::{Error, Result};
use crate::service::lock;

/// The keyring entry's service and user, as the system's keyring tool shows
/// them.
const SERVICE: &str = "genshin-companion";
const USER: &str = "hoyolab";
/// The entry's name in the Secret Service's tools (Seahorse…).
const LABEL: &str = "Genshin Companion: HoYoLAB login";

/// Where the app keeps the player's HoYoLAB login, when they ask.
pub trait Keyring: Send + Sync {
    /// Whether this system has a keyring the app can use. It may ask the
    /// system (D-Bus on Linux): call it off the window's thread.
    fn available(&self) -> bool;
    /// The login kept, if there is one.
    fn load(&self) -> Result<Option<Cookie>>;
    /// Keeps `cookie`'s login fields ([`Cookie::login`]) in place of any kept
    /// before.
    fn save(&self, cookie: &Cookie) -> Result<()>;
    /// Forgets the login kept; nothing kept is no error.
    fn forget(&self) -> Result<()>;
}

/// The system's keyring. Reached the first time it is used, off the window's
/// thread.
#[derive(Default)]
pub struct SystemKeyring {
    store: OnceLock<Option<Arc<keyring_core::CredentialStore>>>,
}

impl SystemKeyring {
    /// This system's store, or `None` when it has none the app can reach.
    fn store(&self) -> Option<&Arc<keyring_core::CredentialStore>> {
        self.store.get_or_init(system_store).as_ref()
    }

    fn entry(&self) -> Result<keyring_core::Entry> {
        let store = self.store().ok_or(Error::NoKeyring)?;
        let label = std::collections::HashMap::from([("label", LABEL)]);
        store
            .build(SERVICE, USER, Some(&label))
            .map_err(keyring_error)
    }
}

#[cfg(target_os = "macos")]
fn system_store() -> Option<Arc<keyring_core::CredentialStore>> {
    let store = apple_native_keyring_store::keychain::Store::new().ok()?;
    Some(store as Arc<keyring_core::CredentialStore>)
}

#[cfg(target_os = "windows")]
fn system_store() -> Option<Arc<keyring_core::CredentialStore>> {
    let store = windows_native_keyring_store::Store::new().ok()?;
    Some(store as Arc<keyring_core::CredentialStore>)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn system_store() -> Option<Arc<keyring_core::CredentialStore>> {
    // Fails without a Secret Service on the session bus.
    let store = zbus_secret_service_keyring_store::Store::new().ok()?;
    Some(store as Arc<keyring_core::CredentialStore>)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn system_store() -> Option<Arc<keyring_core::CredentialStore>> {
    None
}

/// A keyring's failure, in the player's words, without what was stored.
fn keyring_error(error: keyring_core::Error) -> Error {
    match error {
        keyring_core::Error::NoStorageAccess(_) | keyring_core::Error::NoDefaultStore => {
            Error::NoKeyring
        }
        // Platform errors name the call that failed, never the secret.
        other => Error::Keyring(other.to_string()),
    }
}

impl Keyring for SystemKeyring {
    fn available(&self) -> bool {
        self.store().is_some()
    }

    fn load(&self) -> Result<Option<Cookie>> {
        match self.entry()?.get_password() {
            Ok(text) => {
                // Wiped once read.
                let text = Secret::new(text);
                Ok(Cookie::parse(text.expose()).ok())
            }
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(error) => Err(keyring_error(error)),
        }
    }

    fn save(&self, cookie: &Cookie) -> Result<()> {
        let header = cookie.login().header();
        (self.entry()?.set_password(header.expose())).map_err(keyring_error)
    }

    fn forget(&self) -> Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(error) => Err(keyring_error(error)),
        }
    }
}

/// No keyring: nothing is kept (tests, and the app where there is none).
pub struct NoKeyring;

impl Keyring for NoKeyring {
    fn available(&self) -> bool {
        false
    }

    fn load(&self) -> Result<Option<Cookie>> {
        Ok(None)
    }

    fn save(&self, _: &Cookie) -> Result<()> {
        Err(Error::NoKeyring)
    }

    fn forget(&self) -> Result<()> {
        Ok(())
    }
}

/// A keyring in memory, for tests and the screenshot tool: never the
/// player's keyring, and gone when dropped.
#[derive(Default)]
pub struct MemoryKeyring {
    kept: Mutex<Option<Secret>>,
}

impl MemoryKeyring {
    /// One that keeps `cookie`'s login already.
    pub fn with(cookie: &Cookie) -> Self {
        let keyring = Self::default();
        *lock(&keyring.kept) = Some(cookie.login().header());
        keyring
    }

    /// Whether it keeps a login.
    pub fn keeps(&self) -> bool {
        lock(&self.kept).is_some()
    }
}

impl Keyring for MemoryKeyring {
    fn available(&self) -> bool {
        true
    }

    fn load(&self) -> Result<Option<Cookie>> {
        let kept = lock(&self.kept);
        Ok(kept
            .as_ref()
            .and_then(|text| Cookie::parse(text.expose()).ok()))
    }

    fn save(&self, cookie: &Cookie) -> Result<()> {
        *lock(&self.kept) = Some(cookie.login().header());
        Ok(())
    }

    fn forget(&self) -> Result<()> {
        *lock(&self.kept) = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_login_is_kept() {
        let keyring = MemoryKeyring::default();
        assert!(keyring.load().unwrap().is_none());
        let cookie = Cookie::parse(
            "ltuid_v2=1; ltoken_v2=v2_t; ltmid_v2=m; cookie_token_v2=v2_ct; account_id_v2=1",
        )
        .unwrap();
        keyring.save(&cookie).unwrap();
        let kept = keyring.load().unwrap().unwrap();
        assert_eq!(kept.names(), ["ltuid_v2", "ltoken_v2", "ltmid_v2"]);
        keyring.forget().unwrap();
        assert!(!keyring.keeps());
        // Without a keyring, nothing is kept anywhere.
        assert!(matches!(NoKeyring.save(&cookie), Err(Error::NoKeyring)));
    }
}
