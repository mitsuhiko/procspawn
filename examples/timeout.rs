use procspawn::{self, spawn};
use std::thread;
use std::time::Duration;

fn main() {
    procspawn::init();

    let mut handle = spawn((), |()| {
        thread::sleep(Duration::from_secs(10));
    });

    println!("result: {:?}", handle.join_timeout(Duration::from_secs(1)));
}
