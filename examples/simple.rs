use mitosis;

use std::thread::sleep_ms;

fn main() {
    mitosis::init();

    mitosis::spawn();

    sleep_ms(2000);
}
