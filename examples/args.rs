use procspawn::{self, spawn};

fn main() {
    procspawn::init();

    let handle = spawn((), |()| std::env::args().collect::<Vec<_>>());

    let args = handle.join().unwrap();

    println!("args in subprocess: {:?}", args);
}
