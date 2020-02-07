use procspawn::{self, spawn};

#[allow(clippy::empty_loop)]
fn main() {
    procspawn::init();
    let handle = spawn((), |()| loop {});
    handle.kill().unwrap();
}
