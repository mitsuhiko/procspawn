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
    for mut handle in handles {
        if handle.join_timeout(Duration::from_secs(5)).is_ok() {
            ok += 1;
        } else {
            failed += 1;
        }
    }

    assert_eq!(ok, 12);
    assert_eq!(failed, 4);
}

#[test]
fn test_bad_roundtrip_does_not_hang() {
    let pool = Pool::new(1).unwrap();
    let mut handle = pool.spawn((), |()| BadRoundtrip);
    let err = handle.join_timeout(Duration::from_secs(2)).unwrap_err();
    assert!(!err.is_timeout());

    let handle = pool.spawn(41, |value| value + 1);
    assert_eq!(handle.join().unwrap(), 42);
}

#[test]
fn test_overload() {
    let pool = Pool::new(2).unwrap();
    let mut handles = vec![];
    let mut with_pid = 0;

    for _ in 0..10 {
        handles.push(pool.spawn((), |()| {
            thread::sleep(Duration::from_secs(10));
        }));
    }

    thread::sleep(Duration::from_millis(100));
    for handle in handles.iter() {
        if handle.pid().is_some() {
            with_pid += 1;
        }
    }

    assert_eq!(with_pid, 2);

    // kill the pool
    pool.kill();
}

#[test]
fn test_timeout() {
    let pool = Pool::new(2).unwrap();

    let mut handle = pool.spawn((), |()| {
        thread::sleep(Duration::from_secs(10));
    });

    let err = handle.join_timeout(Duration::from_millis(100)).unwrap_err();
    assert!(err.is_timeout());

    let mut handle = pool.spawn((), |()| {
        thread::sleep(Duration::from_millis(100));
        42
    });

    let val = handle.join_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(val, 42);
}

#[test]
fn test_timeout_can_be_retried() {
    let pool = Pool::new(2).unwrap();
    let mut handle = pool.spawn((), |()| {
        thread::sleep(Duration::from_millis(200));
        42
    });

    for _ in 0..2 {
        let err = handle.join_timeout(Duration::from_millis(10)).unwrap_err();
        assert!(err.is_timeout());
    }

    assert_eq!(handle.join_timeout(Duration::from_secs(2)).unwrap(), 42);
}

#[derive(Debug)]
struct BadRoundtrip;

impl serde::Serialize for BadRoundtrip {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(42)
    }
}

impl<'de> serde::Deserialize<'de> for BadRoundtrip {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(BadRoundtrip)
    }
}
