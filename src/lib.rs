//! This crate provides the ability to spawn processes with a function similar
//! to `thread::spawn`
//!
//! To use this crate, call `mitosis::init()` at the beginning of your `main()`,
//! and then anywhere in your program you may call `mitosis::spawn()`:
//!
//! ```rust,norun
//! let data = vec![1, 2, 3, 4];
//! mitosis::spawn(data, |data| {
//!     // This will run in a separate process
//!     println!("Received data {}", data);
//! })
//!```
//!
//! `mitosis::spawn()` can pass arbitrary serializable data, including IPC senders
//! and receivers from the `ipc-channel` crate, down to the new process.
use ipc_channel::ipc::{self, IpcOneShotServer, IpcSender, OpaqueIpcReceiver};
use serde::{Deserialize, Serialize};
use std::{env, mem, process};

const ARGNAME: &'static str = "--mitosis-content-process-id=";

/// Initialize mitosis
///
/// This MUST be called near the top of your main(), before
/// you do any argument parsing. Any code found before this will also
/// be executed for each spawned child process.
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
    fn_offset: isize,
    args_receiver: OpaqueIpcReceiver,
}

fn bootstrap_ipc(token: String) {
    let connection_bootstrap: IpcSender<IpcSender<BootstrapData>> =
        IpcSender::connect(token).unwrap();
    let (tx, rx) = ipc::channel().unwrap();
    connection_bootstrap.send(tx).unwrap();
    let bootstrap_data = rx.recv().unwrap();
    unsafe {
        let ptr = bootstrap_data.wrapper_offset + init as *const () as isize;
        let func: fn(isize, OpaqueIpcReceiver) = mem::transmute(ptr);
        func(bootstrap_data.fn_offset, bootstrap_data.args_receiver);
    }
    process::exit(0);
}

/// Spawn a new process to run a function with some payload
///
/// ```rust,norun
/// let data = vec![1, 2, 3, 4];
/// mitosis::spawn(data, |data| {
///     // This will run in a separate process
///     println!("Received data {}", data);
/// })
/// ```
///
/// The function itself cannot capture anything from its environment, but you can
/// explicitly pass down data through the `args` parameter
pub fn spawn<A: Serialize + for<'de> Deserialize<'de>>(args: A, f: fn(A)) {
    let (server, token) = IpcOneShotServer::<IpcSender<BootstrapData>>::new().unwrap();
    // XXXManishearth use /proc/self/exe on linux
    let me = env::current_exe().unwrap();
    let mut child = process::Command::new(me);
    child.arg(format!("{}{}", ARGNAME, token));
    child.spawn().unwrap();

    let (_rx, tx) = server.accept().unwrap();

    let (args_tx, args_rx) = ipc::channel().unwrap();
    args_tx.send(args).unwrap();
    // ASLR mitigation
    let init_loc = init as *const () as isize;
    let fn_offset = f as *const () as isize - init_loc;
    let wrapper_offset = run_func::<A> as *const () as isize - init_loc;
    let bootstrap = BootstrapData {
        fn_offset,
        wrapper_offset,
        args_receiver: args_rx.to_opaque(),
    };
    tx.send(bootstrap).unwrap();
}

unsafe fn run_func<A: Serialize + for<'de> Deserialize<'de>>(
    offset: isize,
    recv: OpaqueIpcReceiver,
) {
    let function: fn(A) = mem::transmute(offset + init as *const () as isize);

    let args = recv.to().recv().unwrap();
    function(args)
}
