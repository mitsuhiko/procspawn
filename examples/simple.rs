use mitosis;

use std::thread::sleep;
use std::time::Duration;

fn main() {
    mitosis::init();

    mitosis::spawn((1, 2), |(a, b)| {
        println!("{:?} {:?}", a, b);
    });

    sleep(Duration::from_secs(2));
}
