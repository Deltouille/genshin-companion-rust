//! A small HoYoLAB client for one Genshin Impact account (overseas servers):
//! the game accounts behind a cookie, their characters as the Battle
//! Chronicle shows them (level, constellation, talents, the equipped
//! weapon), what HoYoLAB keeps for a while only (the Spiral Abyss, the
//! Imaginarium Theater, the Traveler's Diary: [`chronicle`]), and an
//! authkey for the wish history.
//!
//! HoYoLAB has no public API: the endpoints, the signing and the error codes
//! follow [genshin.py](https://github.com/seriaati/genshin.py) (MIT), whose
//! updates this crate follows when HoYoverse changes something.
//!
//! The cookie is the player's: this crate never stores it. [`Cookie`] keeps
//! only the fields HoYoLAB needs, each a [`Secret`] (its own copy wiped when
//! dropped, hidden from `Debug` and `Display`), and the client sends it to
//! HoYoverse's hosts only ([`transport::is_hoyoverse`]), over https. An app
//! that keeps a login for later keeps [`Cookie::login`] only, never the
//! cookie token.
//!
//! Nothing here knows the app: `wish-core` turns what this returns into an
//! import of the collection.

pub mod chronicle;
mod client;
mod cookie;
mod ds;
mod error;
mod secret;
pub mod transport;

pub use client::{
    CharacterDetail, Client, Constellation, EquippedWeapon, GameAccount, OwnedCharacter, Skill,
    genshin_server,
};
pub use cookie::Cookie;
pub use error::{Error, Result};
pub use secret::Secret;
pub use transport::{HttpTransport, Request, Transport};
