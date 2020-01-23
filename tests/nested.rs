#[test]
fn mitosis() {
    mitosis::init_test();
}

#[test]
fn nested() {
    mitosis::init();
    let five = mitosis::spawn(5, |x| {
        println!("1");
        let x = mitosis::spawn(x, |y| {
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
