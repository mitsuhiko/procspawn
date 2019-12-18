use ipc_channel::ipc::{self, IpcOneShotServer, IpcSender, OpaqueIpcReceiver};
use serde::{Deserialize, Serialize};
use std::{env, mem, process};

const ARGNAME: &'static str = "--mitosis-content-process-id=";

pub fn init() {
    let mut args = env::args();
    if args.len() != 2 {
        return;
    }
    if let Some(arg) = args.nth(1) {
        println!("found arg {}", arg);
        if arg.starts_with(ARGNAME) {
            bootstrap_ipc(arg[ARGNAME.len()..].into());
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct BootstrapData {
    offset: isize,
    args_receiver: OpaqueIpcReceiver,
}

fn bootstrap_ipc(token: String) {
    let connection_bootstrap: IpcSender<IpcSender<BootstrapData>> =
        IpcSender::connect(token).unwrap();
    let (tx, rx) = ipc::channel().unwrap();
    connection_bootstrap.send(tx).unwrap();
    let bootstrap_data = rx.recv().unwrap();
    unsafe {
        let ptr = bootstrap_data.offset + init as *const() as isize;
        let func: fn(OpaqueIpcReceiver) = mem::transmute(ptr);
        func(bootstrap_data.args_receiver);   
    }
    process::exit(0);
}

pub fn spawn<F: FnOnce(A), A: Serialize + for<'de> Deserialize<'de>>(args: A, f: F) {
    if mem::size_of::<F>() != 0 {
        panic!("mitosis::spawn cannot be called with closures that have captures");
    }
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
    let offset =  run_func::<F, A> as *const () as isize - init as *const() as isize;
    let bootstrap = BootstrapData {
        offset,
        args_receiver: args_rx.to_opaque(),
    };
    tx.send(bootstrap).unwrap();
}

unsafe fn run_func<F: FnOnce(A), A: Serialize + for<'de> Deserialize<'de>>(recv: OpaqueIpcReceiver) {
    println!("running");
    let function: F = mem::zeroed();

    let args = recv.to().recv().unwrap();
    function(args)
}
