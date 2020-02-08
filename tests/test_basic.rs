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
