//! The use cases the app and `wishctl` share, without GPUI: what to fetch, in
//! which order, when to stop, where a change can go, what an export holds.
//! The app runs them on its background executor; `wishctl` calls them as they
//! are; tests run them on fake transports. Each takes what it reaches as
//! parameters: the transport, the store, the game data.
//!
//! **The store is never locked during a request.** A service that fetches
//! takes `&Mutex<Store>` and locks it itself, before and after the requests
//! only (see [`lock`]).

use std::sync::{Mutex, MutexGuard};

use crate::store::Store;

pub mod export;
pub mod game;
pub mod hoyolab;

/// The store, for one read or write. A write that panicked left it as it
/// was (each write is one transaction, rolled back as the panic unwound), so
/// the lock it poisoned is taken back.
pub fn lock(store: &Mutex<Store>) -> MutexGuard<'_, Store> {
    store.lock().unwrap_or_else(|poisoned| {
        store.clear_poison();
        poisoned.into_inner()
    })
}
