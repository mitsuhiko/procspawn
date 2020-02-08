use std::thread;
use std::time::Duration;

use procspawn::{self, Pool};

procspawn::enable_test_support!();

#[test]
fn test_basic() {
    let pool = Pool::new(4).unwrap();
    let mut handles = vec![];

    for x in 0..16 {
        handles.push(pool.spawn(x, |x| {
            if x % 4 == 0 {
                panic!("completely broken");
            }
            thread::sleep(Duration::from_millis(200));
        }));
    }

    let mut ok = 0;
    let mut failed = 0;
    for handle in handles {
        if handle.join().is_ok() {
            ok += 1;
        } else {
            failed += 1;
        }
    }

    assert_eq!(ok, 12);
    assert_eq!(failed, 4);
}
