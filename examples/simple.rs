use mitosis;

use std::thread::sleep_ms;

fn main() {
    mitosis::init();

    mitosis::spawn((1, 2), |(a, b)| {println!("{:?} {:?}", a, b);});

    sleep_ms(2000);
}
