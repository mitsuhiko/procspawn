use ipc_channel::ipc::{self, IpcOneShotServer, IpcReceiver, IpcSender};
use serde::{Deserialize, Serialize};
use std::{env, process};

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
    s: String,
}

fn bootstrap_ipc(token: String) {
    let connection_bootstrap: IpcSender<IpcSender<BootstrapData>> =
        IpcSender::connect(token).unwrap();
    let (tx, rx) = ipc::channel().unwrap();
    connection_bootstrap.send(tx).unwrap();
    let bootstrap_data = rx.recv().unwrap();
    println!("{:?}", bootstrap_data);

    process::exit(0);
}

pub fn spawn() {
    let (server, token) = IpcOneShotServer::<IpcSender<BootstrapData>>::new().unwrap();
    // XXXManishearth use /proc/self/exe on linux
    let me = env::current_exe().unwrap();
    let mut child = process::Command::new(me);
    child.arg(format!("{}{}", ARGNAME, token));
    child.spawn().unwrap();

    let (rx, tx) = server.accept().unwrap();
    tx.send(BootstrapData { s: "aaa".into() }).unwrap();
}
