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
