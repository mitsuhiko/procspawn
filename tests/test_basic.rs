use std::env;
use std::thread;
use std::time::Duration;

use procspawn::{self, spawn};

procspawn::enable_test_support!();

#[test]
fn test_basic() {
    let handle = spawn(true, |b| !b);
    let value = handle.join().unwrap();

    assert!(!value);
}

#[test]
fn test_panic() {
    let handle = spawn::<_, ()>((), |()| panic!("something went wrong"));
    let err = handle.join().unwrap_err();

    let panic_info = err.panic_info().unwrap();
    assert_eq!(panic_info.message(), "something went wrong");
    assert!(panic_info.backtrace().is_some());

    let loc = panic_info.location().unwrap();
    assert_eq!(loc.line(), 19);
    assert_eq!(loc.column(), 42);
    assert!(loc.file().contains("test_basic.rs"));
}

#[test]
fn test_kill() {
    let mut handle = spawn((), |()| {
        thread::sleep(Duration::from_secs(10));
    });
    handle.kill().unwrap();
    let err = handle.join().unwrap_err();
    dbg!(&err);
    assert!(err.is_remote_close());
}

#[test]
fn test_bad_roundtrip_does_not_hang() {
    let mut handle = spawn((), |()| BadRoundtrip);
    let err = handle.join_timeout(Duration::from_secs(2)).unwrap_err();
    assert!(!err.is_timeout());
}

#[test]
fn test_envvar() {
    let val = procspawn::Builder::new()
        .env("FOO", "42")
        .spawn(23, |val| {
            env::var("FOO").unwrap().parse::<i32>().unwrap() + val
        })
        .join()
        .unwrap();
    assert_eq!(val, 42 + 23);
}

#[test]
fn test_env_remove() {
    let removed = procspawn::Builder::new()
        .env_remove("PATH")
        .spawn((), |()| env::var_os("PATH").is_none())
        .join()
        .unwrap();
    assert!(removed);
}

#[test]
fn test_env_clear() {
    let env = procspawn::Builder::new()
        .env_clear()
        .env("PROCSPAWN_TEST_ENV", "present")
        .spawn((), |()| {
            (env::var_os("PATH"), env::var("PROCSPAWN_TEST_ENV").unwrap())
        })
        .join()
        .unwrap();
    assert_eq!(env, (None, "present".to_string()));
}

#[test]
fn test_nested() {
    let five = spawn(5, |x| {
        println!("1");
        let x = spawn(x, |y| {
            println!("2");
            y
        })
        .join()
        .unwrap();
        println!("3");
        x
    })
    .join()
    .unwrap();
    println!("4");
    assert_eq!(five, 5);
}

#[test]
fn test_timeout() {
    let mut handle = spawn((), |()| {
        thread::sleep(Duration::from_secs(10));
    });

    let err = handle.join_timeout(Duration::from_millis(100)).unwrap_err();
    assert!(err.is_timeout());

    let mut handle = spawn((), |()| {
        thread::sleep(Duration::from_millis(100));
        42
    });

    let val = handle.join_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(val, 42);
}

#[test]
fn test_timeout_can_be_retried() {
    let mut handle = spawn((), |()| {
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
