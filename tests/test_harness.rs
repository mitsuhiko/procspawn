use mitosis::{init, init_test, spawn};

#[test]
fn normal_test() {}

#[test]
fn mitosis() {
    init();
}

#[test]
fn using_init_test_but_not_spawn() {
    init_test();
}

#[test]
fn using_init_test_and_spawn() {
    init_test();
    let val = spawn(42, |x| x / 2).join().unwrap();
    assert_eq!(val, 21);
}
