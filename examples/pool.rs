use procspawn::{self, Pool};
use std::thread;
use std::time::Duration;

fn main() {
    procspawn::init();

    let pool = Pool::new(4).unwrap();
    let mut handles = vec![];

    for counter in 0..8 {
        handles.push(pool.spawn(counter, |counter| {
            thread::sleep(Duration::from_millis(500));
            counter
        }));
    }

    for handle in handles {
        match handle.join() {
            Ok(val) => println!("got result: {}", val),
            Err(err) => {
                let panic = err.panic_info().expect("got a non panic error");
                println!("process panicked with {}", panic.message());
                println!("{:#?}", panic);
            }
        }
    }

    pool.shutdown();
}
