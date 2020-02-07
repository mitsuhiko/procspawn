use procspawn::{self, spawn};

fn main() {
    procspawn::init();

    let five = spawn(5, fibonacci);
    let ten = spawn(10, fibonacci);
    let thirty = spawn(30, fibonacci);
    assert_eq!(five.join().unwrap(), 5);
    assert_eq!(ten.join().unwrap(), 55);
    assert_eq!(thirty.join().unwrap(), 832_040);
    println!("Successfully calculated fibonacci values!");
}

fn fibonacci(n: u32) -> u32 {
    if n <= 2 {
        return 1;
    }
    fibonacci(n - 1) + fibonacci(n - 2)
}
