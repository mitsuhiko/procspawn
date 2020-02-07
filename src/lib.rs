//! This crate provides the ability to spawn processes with a function similar
//! to `thread::spawn`.
//!
//! ```rust,no_run
//! procspawn::init();
//! 
//! let data = vec![1, 2, 3, 4];
//! let handle = procspawn::spawn(data, |data| {
//!     println!("Received data {:?}", &data);
//!     data.into_iter().sum::<i64>()
//! });
//! let result = handle.join().unwrap();
//!```
//!
//! `spawn()` can pass arbitrary serializable data, including IPC senders
//! and receivers from the `ipc-channel` crate, down to the new process.
//!
//! This crate is a fork of `mitosis`.

use serde::{Deserialize, Serialize};

mod core;
mod error;
mod proc;

pub use self::core::{init, ProcConfig};
pub use self::error::{Panic, SpawnError};
pub use self::proc::{Builder, JoinHandle};

/// Spawn a new process to run a function with some payload.
pub fn spawn<
    F: FnOnce(A) -> R + Copy,
    A: Serialize + for<'de> Deserialize<'de>,
    R: Serialize + for<'de> Deserialize<'de>,
>(
    args: A,
    f: F,
) -> JoinHandle<R> {
    Builder::new().spawn(args, f)
}
