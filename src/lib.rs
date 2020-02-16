//! This crate provides the ability to spawn processes with a function similar
//! to `thread::spawn`.
//!
//! Unlike `thread::spawn` data cannot be passed by the use of closures.  Instead
//! if must be explicitly passed as serializable object (specifically it must be
//! [`serde`](https://serde.rs/) serializable).  The return value from the
//! spawned closure also must be serializable and can then be retrieved from
//! the returned join handle.
//!
//! If the spawned functiom causes a panic it will also be serialized across
//! the process boundaries.
//!
//! # Example
//!
//! First for all of this to work you need to invoke `procspawn::init` at a
//! point early in your program (somewhere at the beginning of the main function).
//! Whatever happens before that point also happens in your spawned functions.
//!
//! Subprocesses are by default invoked with the same arguments and environment
//! variables as the parent process.
//!
//! ```rust,no_run
//! procspawn::init();
//! ```
//!
//! Now you can start spawning functions:
//!
//! ```rust,no_run
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
//! [`procspawn::init`](fn.init.html) explicitly an early time in the program.
//!
//! Alternatively you can use the [`ProcConfig`](struct.ProcConfig.html)
//! builder object to initialize the process which gives you some extra
//! abilities to customize the processes spawned.  This for instance lets you
//! disable the default panic handling.
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
//! needed.  Pools are currently not supported for async usage.
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
//! * `safe-shared-libraries`: this feature is enabled by default.  When this
//!   feature is disable then no validation about shared library load status
//!   is performed around IPC calls.  This is highly unsafe if shared libraries
//!   are being used and a function from a shared library is spawned.
//! * `backtrace`: this feature is enabled by default.  When in use then
//!   backtraces are captured with the `backtrace-rs` crate and serialized
//!   across process boundaries.
//! * `test-support`: when this feature is enabled procspawn can be used
//!   with rusttest.  See [`testing`](#testing) for more information.
//! * `json`: enables optional JSON serialization.  For more information see
//!   [Bincode Limitations](#bincode-limitations).
//! * `async`: enables support for the async methods.
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
//! # Testing
//!
//! Due to limitations of the rusttest testing system there are some
//! restrictions to how this crate operates.  First of all you need to enable
//! the `test-support` feature for `procspawn` to work with rusttest at all.
//! Secondly your tests need to invoke the
//! [`enable_test_support!`](macro.enable_test_support.html) macro once
//! top-level.
//!
//! With this done the following behavior applies:
//!
//! * Tests behave as if `procspawn::init` was called (that means with the
//!   default arguments).  Other configuration is not supported.
//! * procspawn will register a dummy test (named `procspawn_test_helper`)
//!   which doesn't do anything when called directly, but acts as the spawning
//!   helper for all `spawn` calls.
//! * stdout is silenced by default unless `--show-output` or `--nocapture`
//!   is passed to tests.
//! * when trying to spawn with intercepted `stdout` be aware that there is
//!   extra noise that will be emitted by rusttest.
//!
//! Example:
//!
//! ```rust,no_run
//! procspawn::enable_test_support!();
//!
//! #[test]
//! fn test_basic() {
//!     let handle = procspawn::spawn((1, 2), |(a, b)| a + b);
//!     let value = handle.join().unwrap();
//!     assert_eq!(value, 3);
//! }
//! ```
//!
//! # Shared Libraries
//!
//! `procspawn` uses the [`findshlibs`](https://github.com/gimli-rs/findshlibs)
//! crate to determine where a function is located in memory in both processes.
//! If a shared library is not loaded in the subprocess (because for instance it
//! is loaded at runtime) then the call will fail.  Because this adds quite
//! some overhead over every call you can also disable the `safe-shared-libraries`
//! feature (which is on by default) in which case you are not allowed to
//! invoke functions from shared libraries and no validation is performed.
//!
//! This in normal circumstances should be okay but you need to validate this.
//! Spawning processes will be disabled if the feature is not enabled until
//! you call the [`assert_spawn_is_safe`](fn.assert_spawn_is_safe.html) function.
//!
//! # Async Support
//!
//! When the `async` feature is enabled a `spawn_async` function becomes
//! available which gives you an async version of a join handle.  There are
//! however a few limitations / differences with async support currently:
//!
//! * pools are not supported. Right now you can only spawn one-off processes.
//! * replacing stdin/stdout/stderr with a pipe is not supported.  The async
//!   join handle does not give you access to these streams.
//! * when you drop a join handle the process is being terminated.
//! * there is no native join with timeout support.  You can use your executors
//!   timeout functionality to achieve the same.
//!
//! # Platform Support
//!
//! Currently this crate only supports macOS and Linux because ipc-channel
//! itself does not support Windows yet.  Additionally the findshlibs which is
//! used for the `safe-shared-libraries` feature also does not yet support
//! Windows.
//!
//! # More Examples
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
//! * [async.rs](https://github.com/mitsuhiko/procspawn/blob/master/examples/async.rs):
//!   demonstrates async usage.
//!
//! More examples can be found in the example folder: [examples](https://github.com/mitsuhiko/procspawn/tree/master/examples)

#[macro_use]
mod proc;

mod core;
mod error;
mod panic;
mod pool;

#[cfg(feature = "json")]
mod json;

#[cfg(feature = "async")]
mod asyncsupport;

#[doc(hidden)]
pub mod testsupport;

pub use self::core::{assert_spawn_is_safe, init, ProcConfig};
pub use self::error::{Location, PanicInfo, SpawnError};
pub use self::pool::{Pool, PoolBuilder};
pub use self::proc::{spawn, Builder, JoinHandle};

#[cfg(feature = "json")]
pub use self::json::Json;

#[cfg(feature = "async")]
pub use self::asyncsupport::{spawn_async, AsyncJoinHandle};
