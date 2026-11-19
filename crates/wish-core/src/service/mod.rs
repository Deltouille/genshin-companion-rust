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

pub mod export;
pub mod game;
pub mod good;
pub mod hoyolab;

/// What `mutex` guards, for one read or write, even after a panic poisoned
/// it: the lock is taken back. The one way the app and `wishctl` lock a
/// mutex. It suits what they guard: the store, where a write that panicked
/// left everything as it was (each write is one transaction, rolled back as
/// the panic unwound); and sets, counters and progress, which a panic leaves
/// whole. Treating a poisoned lock as "nothing" instead would hide, say,
/// every image until the app restarts.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| {
        mutex.clear_poison();
        poisoned.into_inner()
    })
}
