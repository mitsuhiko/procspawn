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
use std::{env, mem, process};

const ARGNAME: &'static str = "--mitosis-content-process-id=";

/// Initialize mitosis
///
/// This MUST be called near the top of your main(), before
/// you do any argument parsing. Any code found before this will also
/// be executed for each spawned child process.
///
/// # Safety
/// It is not unsafe to omit this function, however `mitosis::spawn`
/// may lead to unexpected behavior.
pub fn init() {
    let mut args = env::args();
    if args.len() != 2 {
        return;
    }
    if let Some(arg) = args.nth(1) {
        if arg.starts_with(ARGNAME) {
            bootstrap_ipc(arg[ARGNAME.len()..].into());
        }
    }
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
    _: F,
) -> JoinHandle<R> {
    if mem::size_of::<F>() != 0 {
        panic!("mitosis::spawn cannot be called with closures that capture data!");
    }

    let (server, token) = IpcOneShotServer::<IpcSender<BootstrapData>>::new().unwrap();
    let me = if cfg!(target_os = "linux") {
        // will work even if exe is moved
        "/proc/self/exe".into()
    } else {
        env::current_exe().unwrap()
    };
    let mut child = process::Command::new(me);
    child.arg(format!("{}{}", ARGNAME, token));
    child.spawn().unwrap();

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
    JoinHandle { recv: return_rx }
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
}

impl<T: Serialize + for<'de> Deserialize<'de>> JoinHandle<T> {
    /// Wait for the child process to return a result
    pub fn join(self) -> Result<T, IpcError> {
        self.recv.recv()
    }
}
