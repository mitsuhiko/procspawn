use procspawn::{self, spawn};

fn main() {
    procspawn::init();

    let handle = spawn((1, 2), |(a, b)| {
        println!("in process: {:?} {:?}", a, b);
        a + b
    });

    println!("result: {}", handle.join().unwrap());
}
