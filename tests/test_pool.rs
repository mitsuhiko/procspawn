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

#[test]
fn test_timeout() {
    let pool = Pool::new(4).unwrap();

    let handle = pool.spawn((), |()| {
        thread::sleep(Duration::from_secs(10));
    });
    match handle.join_timeout(Duration::from_secs(1)) {
        Ok(()) => panic!("should not happen"),
        Err(err) => assert!(err.is_timeout()),
    }

    let handle = pool.spawn((), |()| {
        thread::sleep(Duration::from_secs(1));
        42
    });

    let val = handle.join_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(val, 42);
}

#[test]
fn test_overload() {
    let pool = Pool::new(2).unwrap();
    let mut handles = vec![];

    for _ in 0..10 {
        handles.push(pool.spawn((), |()| {
            thread::sleep(Duration::from_secs(10));
        }));
    }

    thread::sleep(Duration::from_millis(100));
    for (idx, handle) in handles.iter().enumerate() {
        assert_eq!(handle.pid().is_some(), idx < 2);
    }

    // kill the pool
    pool.kill();
}
