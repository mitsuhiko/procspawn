use procspawn::{self, spawn};

#[test]
fn test_basic() {
    procspawn::init();

    let handle = spawn(true, |b| !b);
    let value = handle.join().unwrap();

    assert_eq!(value, false);
}
