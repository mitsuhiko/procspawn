use procspawn::{self, spawn};

fn main() {
    procspawn::init();

    let a = 42u32;
    let b = 23u32;
    let c = 1;
    let handle = spawn!((a => new_name1, b, mut c) || -> Result<_, ()> {
        c += 1;
        Ok(new_name1 + b + c)
    });
    let value = handle.join().unwrap();

    println!("{:?}", value);
}
