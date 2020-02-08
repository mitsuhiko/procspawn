use std::env;
use std::thread;
use std::time::Duration;

use procspawn::{self, spawn};

procspawn::enable_test_support!();

#[test]
fn test_basic() {
    let handle = spawn(true, |b| !b);
    let value = handle.join().unwrap();

    assert_eq!(value, false);
}

#[test]
fn test_panic() {
    let handle = spawn((), |()| panic!("something went wrong"));
    let err = handle.join().unwrap_err();

    let panic_info = err.panic_info().unwrap();
    assert_eq!(panic_info.message(), "something went wrong");
    assert!(panic_info.backtrace().is_some());
}

#[test]
fn test_kill() {
    let handle = spawn((), |()| {
        thread::sleep(Duration::from_secs(10));
    });
    handle.kill().unwrap();
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
