use procspawn::{self, spawn};

fn main() {
    procspawn::init();

    let handle = spawn((), |()| {
        panic!("Whatever!");
    });
    handle.join().unwrap();
}
