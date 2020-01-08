//! This crate provides the ability to spawn processes with a function similar
//! to `thread::spawn`
//!
//! To use this crate, call `mitosis::init()` at the beginning of your `main()`,
//! and then anywhere in your program you may call `mitosis::spawn()`:
//!
//! ```rust,no_run
//! let data = vec![1, 2, 3, 4];
//! mitosis::spawn(data, |data| {
//!     // This will run in a separate process
//!     println!("Received data {:?}", data);
//! });
//!```
//!
//! `mitosis::spawn()` can pass arbitrary serializable data, including IPC senders
//! and receivers from the `ipc-channel` crate, down to the new process.
use ipc_channel::ipc::{
    self, IpcOneShotServer, IpcReceiver, IpcSender, OpaqueIpcReceiver, OpaqueIpcSender,
};
use ipc_channel::Error as IpcError;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::{ChildStderr, ChildStdin, ChildStdout};
use std::sync::{Mutex, RwLock};
use std::{env, mem, process};

mod builder;

pub use builder::*;

const ENV_NAME: &str = "MITOSIS_CONTENT_PROCESS_ID";

/// Initialize mitosis
///
/// This MUST be called near the top of your main(), before
/// you do any environment variable processing. Any code found before this will also
/// be executed for each spawned child process.
///
/// # Safety
/// It is not unsafe to omit this function, however `mitosis::spawn`
/// may lead to unexpected behavior.
pub fn init() {
    if let Ok(token) = env::var(ENV_NAME) {
        bootstrap_ipc(token);
    }
    // Clear environment variable so processes spawned from the `spawn` closure can
    // themselves be using `mitosis`
    std::env::remove_var(ENV_NAME);
}

lazy_static::lazy_static! {
    // This is necessary because it's not possible to downgrade a `RwLockWriteGuard` to
    // a `RwLockReadGuard` without unlocking. So if we write to make sure that tests using
    // `init_test` do not interfere with each other (e.g. by overwriting the `TEST_HOOK`),
    // we need to block for the entire execution of the test.
    static ref PROTECT_WRITING_TO_TEST_HOOK: Mutex<()> = Mutex::new(());
    static ref TEST_HOOK: RwLock<Option<&'static str>> = RwLock::new(None);
}

/// Initialize `mitosis` within a `#[test]`. You need to specify the name of the
/// test (including its entire module path), because this calls the test binary
/// with the test's name as the argument, just like `cargo test your::test::name`
/// does. Technically you can specify an test name that runs `init`, so for simplicity
/// you may want to include a `#[test] fn mitosis() { init() }` in your test suite and
/// just use `"mitosis"` as the `init_test` argument everywhere.
///
/// The closure argument is where you run code that may use `mitosis::spawn`. Any code
/// using `mitosis::spawn` outside the closure will cause unexpected behaviour.
///
/// Note that using `mitosis` within tests is slow. Whenever you call `spawn`
/// the entire test harness is executed before actually running your closure.
///
/// # Safety
/// It is not unsafe to omit this function, or specify a wrong test name,
/// however `mitosis::spawn` may lead to unexpected behavior.
pub fn init_test<R>(name: &'static str, f: impl FnOnce() -> R) -> R {
    init();
    // Read the documentation on these globals for an explanation of the
    // multi-level locking mechanism. Doing this wrong does not affect safety,
    // but will cause tests to interfere with each other if cargo is run with
    // --test-threads > 1 (the default is the number of cores).
    let _guard = PROTECT_WRITING_TO_TEST_HOOK.lock();
    *TEST_HOOK.write().unwrap() = Some(name);
    let ret = f();
    *TEST_HOOK.write().unwrap() = None;
    ret
}

#[derive(Serialize, Deserialize, Debug)]
struct BootstrapData {
    wrapper_offset: isize,
    args_receiver: OpaqueIpcReceiver,
    return_sender: OpaqueIpcSender,
}

fn bootstrap_ipc(token: String) {
    let connection_bootstrap: IpcSender<IpcSender<BootstrapData>> =
        IpcSender::connect(token).unwrap();
    let (tx, rx) = ipc::channel().unwrap();
    connection_bootstrap.send(tx).unwrap();
    let bootstrap_data = rx.recv().unwrap();
    unsafe {
        let ptr = bootstrap_data.wrapper_offset + init as *const () as isize;
        let func: fn(OpaqueIpcReceiver, OpaqueIpcSender) = mem::transmute(ptr);
        func(bootstrap_data.args_receiver, bootstrap_data.return_sender);
    }
    process::exit(0);
}

/// Spawn a new process to run a function with some payload
///
/// ```rust,no_run
/// let data = vec![1, 2, 3, 4];
/// mitosis::spawn(data, |data| {
///     // This will run in a separate process
///     println!("Received data {:?}", data);
/// });
/// ```
///
/// The function itself cannot capture anything from its environment, but you can
/// explicitly pass down data through the `args` parameter. This function will panic if
/// you pass a closure that captures anything from its environment.
///
/// The `JoinHandle` returned by this function can be used to wait for
/// the child process to finish, and obtain the return value of the function it executed.
///
/// ```rust,no_run
/// let data = vec![1, 1, 2, 3, 3, 5, 4, 1];
/// let handle = mitosis::spawn(data, |mut data| {
///     // This will run in a separate process
///     println!("Received data {:?}", data);
///     data.dedup();
/// });
/// // do some other work
/// println!("Deduplicated {:?}", handle.join());
/// ```
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

impl Builder {
    pub fn spawn<
        F: FnOnce(A) -> R + Copy,
        A: Serialize + for<'de> Deserialize<'de>,
        R: Serialize + for<'de> Deserialize<'de>,
    >(
        self,
        args: A,
        _: F,
    ) -> JoinHandle<R> {
        if mem::size_of::<F>() != 0 {
            panic!("mitosis::spawn cannot be called with closures that capture data!");
        }

        let (server, token) = IpcOneShotServer::<IpcSender<BootstrapData>>::new().unwrap();
        let me = if cfg!(target_os = "linux") {
            // will work even if exe is moved
            let path: PathBuf = "/proc/self/exe".into();
            if path.is_file() {
                path
            } else {
                // might not exist, e.g. on chroot
                env::current_exe().unwrap()
            }
        } else {
            env::current_exe().unwrap()
        };
        let mut child = process::Command::new(me);
        assert!(
            !self.envs.contains_key(OsStr::new(ENV_NAME)),
            "cannot spawn mitosis process with `{}` still set",
            ENV_NAME
        );
        child.envs(self.envs.into_iter());
        child.env(ENV_NAME, token);
        if let Some(test_name) = *TEST_HOOK.read().unwrap() {
            child.arg(test_name);
            child.arg("--exact");
        }
        if let Some(stdin) = self.stdin {
            child.stdin(stdin);
        }
        if let Some(stdout) = self.stdout {
            child.stdout(stdout);
        }
        if let Some(stderr) = self.stderr {
            child.stderr(stderr);
        }
        let process = child.spawn().unwrap();

        let (_rx, tx) = server.accept().unwrap();

        let (args_tx, args_rx) = ipc::channel().unwrap();
        let (return_tx, return_rx) = ipc::channel().unwrap();
        args_tx.send(args).unwrap();
        // ASLR mitigation
        let init_loc = init as *const () as isize;
        let wrapper_offset = run_func::<F, A, R> as *const () as isize - init_loc;
        let bootstrap = BootstrapData {
            wrapper_offset,
            args_receiver: args_rx.to_opaque(),
            return_sender: return_tx.to_opaque(),
        };
        tx.send(bootstrap).unwrap();
        JoinHandle {
            recv: return_rx,
            process,
        }
    }
}

unsafe fn run_func<
    F: FnOnce(A) -> R,
    A: Serialize + for<'de> Deserialize<'de>,
    R: Serialize + for<'de> Deserialize<'de>,
>(
    recv: OpaqueIpcReceiver,
    sender: OpaqueIpcSender,
) {
    let function: F = mem::zeroed();

    let args = recv.to().recv().unwrap();
    let ret = function(args);
    let _ = sender.to().send(ret);
}

/// This value is returned by `mitosis::spawn` and lets you
/// wait on the result of the child process' computation
pub struct JoinHandle<T> {
    recv: IpcReceiver<T>,
    process: process::Child,
}

impl<T: Serialize + for<'de> Deserialize<'de>> JoinHandle<T> {
    /// Wait for the child process to return a result
    pub fn join(self) -> Result<T, IpcError> {
        self.recv.recv()
    }

    /// Kill the child process.
    pub fn kill(mut self) -> std::io::Result<()> {
        self.process.kill()
    }

    /// Fetch the `stdin` handle if it has been captured
    pub fn stdin(&mut self) -> Option<&mut ChildStdin> {
        self.process.stdin.as_mut()
    }

    /// Fetch the `stdout` handle if it has been captured
    pub fn stdout(&mut self) -> Option<&mut ChildStdout> {
        self.process.stdout.as_mut()
    }

    /// Fetch the `stderr` handle if it has been captured
    pub fn stderr(&mut self) -> Option<&mut ChildStderr> {
        self.process.stderr.as_mut()
    }
}
