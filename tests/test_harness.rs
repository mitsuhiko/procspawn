use mitosis::{init, init_test, spawn};

#[test]
fn normal_test() {}

#[test]
fn using_init_but_not_spawn() {
    init();
}

#[test]
fn using_init_test_but_not_spawn() {
    println!("start");
    init_test("using_init_test_but_not_spawn", || {
        println!("middle");
    });
    println!("end");
}

#[test]
fn wrong_test_name() {}

#[test]
fn using_wrong_init_test_but_not_spawn() {
    println!("start");
    init_test("wrong_test_name", || {
        println!("middle");
        // if we called `spawn` here we would block because `wrong_test_name` does not call `init`
    });
    println!("end");
}

#[test]
fn wrong_test_name_but_init() {
    init();
    println!("foomp");
}

#[test]
fn using_wrong_init_test_but_not_spawn2() {
    println!("start");
    init_test("wrong_test_name_but_init", || {
        println!("middle");
        let val = spawn(42, |_| "hello".to_owned()).join().unwrap();
        assert_eq!(val, "hello");
    });
    println!("end");
}

#[test]
fn using_init_test_and_spawn() {
    let val = init_test("using_init_test_and_spawn", || {
        spawn(42, |x| x / 2).join().unwrap()
    });
    assert_eq!(val, 21);
}
