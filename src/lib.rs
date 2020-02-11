//! This crate provides the ability to spawn processes with a function similar
//! to `thread::spawn`.
//!
//! Unlike `thread::spawn` data cannot be passed in closures but must be
//! explicitly passed as single argument which must be [`serde`](https://serde.rs/)
//! serializable.  The return value from the spawned closure also must be
//! serializable and can then be unwrapped from the returned join handle.
//! If your function has data enclosed it will panic at runtime.
//!
//! ```rust,no_run
//! // call this early in your main() function.  This is where all spawned
//! // functions will be invoked.
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
//! Because `procspawn` will invoke a subprocess and there is currently no
//! reliable way to intercept `main` in Rust it's necessary for you to call
//! [`procspawn::init`](fn.init.html) at an early time in the program. The
//! place where this will be called is the entrypoint for the subprocesses
//! spawned.  The subprocess is invoked with the same arguments and environment
//! variables by default.
//!
//! [`spawn`](fn.spawn.html) can pass arbitrary serializable data, including
//! IPC senders and receivers from the [`ipc-channel`](https://crates.io/crates/ipc-channel)
//! crate, down to the new process.
//!
//! # Pools
//!
//! The default way to spawn processes will start and stop processes constantly.
//! For more uses it's a better idea to spawn a [`Pool`](struct.Pool.html)
//! which will keep processes around for reuse.  Between calls the processes
//! will stay around which also means the can keep state between calls if
//! needed.
//!
//! # Panics
//!
//! By default panics are captured and serialized across process boundaries.
//! This requires that the `backtrace` crate is used with serialization support.
//! If you do not need this feature you can disable the `backtrace` crate and
//! disable panic handling through the [`ProcConfig`](struct.ProcConfig.html)
//! object.
//!
//! # Feature Flags
//!
//! The following feature flags exist:
//!
//! * `backtrace`: this feature is enabled by default.  When in use then
//!   backtraces are captured with the `backtrace-rs` crate and serialized
//!   across process boundaries.
//! * `test-support`: when this feature is enabled procspawn can be used
//!   with rusttest.  See [`enable_test_support!`](macro.enable_test_support.html)
//!   for more information.
//! * `json`: enables optional JSON serialization.  For more information see
//!   [Bincode Limitations](#bincode-limitations).
//!
//! # Bincode Limitations
//!
//! This crate uses [`bincode`](https://github.com/servo/bincode) internally
//! for inter process communication.  Bincode currently has some limitations
//! which make some serde features incompatible with it.  Most notably if you
//! use `#[serde(flatten)]` data cannot be sent across the processes.  To
//! work around this you can enable the `json` feature and wrap affected objects
//! in the [`Json`](struct.Json.html) wrapper to force JSON serialization.
//!
//! # Platform Support
//!
//! Currently this crate only supports macOS and Linux because ipc-channel
//! itself does not support Windows yet.
//!
//! # Examples
//!
//! Here are some examples of `procspawn` in action:
//!
//! * [simple.rs](https://github.com/mitsuhiko/procspawn/blob/master/examples/simple.rs):
//!   a very simple example showing the basics.
//! * [args.rs](https://github.com/mitsuhiko/procspawn/blob/master/examples/args.rs):
//!   shows how arguments are available to the subprocess as well.
//! * [timeout.rs](https://github.com/mitsuhiko/procspawn/blob/master/examples/timeout.rs):
//!   shows how you can wait on a process with timeouts.
//! * [bad-serialization.rs](https://github.com/mitsuhiko/procspawn/blob/master/examples/bad-serialization.rs):
//!   shows JSON based workarounds for bincode limitations.
//!
//! More examples can be found in the example folder: [examples](https://github.com/mitsuhiko/procspawn/tree/master/examples)

use serde::{Deserialize, Serialize};

#[macro_use]
mod proc;

mod core;
mod error;
mod panic;
mod pool;

#[cfg(feature = "json")]
mod json;

#[doc(hidden)]
pub mod testsupport;

pub use self::core::{init, ProcConfig};
pub use self::error::{Location, PanicInfo, SpawnError};
pub use self::pool::{Pool, PoolBuilder};
pub use self::proc::{Builder, JoinHandle};

#[cfg(feature = "json")]
pub use self::json::Json;

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
