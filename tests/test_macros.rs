use procspawn::{self, spawn};

procspawn::enable_test_support!();

#[test]
fn test_macro_no_args() {
    let handle = spawn!(() || false);
    let value = handle.join().unwrap();
    assert_eq!(value, false);
}

#[test]
fn test_macro_single_arg() {
    let value = 42u32;

    let handle = spawn!((value) || value);
    assert_eq!(handle.join().unwrap(), 42);

    let handle = spawn!((value => new_name) || new_name);
    assert_eq!(handle.join().unwrap(), 42);

    let ref_value = &value;

    let handle = spawn!((*ref_value) || ref_value);
    assert_eq!(handle.join().unwrap(), 42);

    let handle = spawn!((*ref_value => new_name) || new_name);
    assert_eq!(handle.join().unwrap(), 42);

    let handle = spawn!((mut value) || {
        value += 1;
        value
    });
    assert_eq!(handle.join().unwrap(), 43);

    let handle = spawn!((value => mut new_name) || {
        new_name += 1;
        new_name
    });
    assert_eq!(handle.join().unwrap(), 43);
}

#[test]
fn test_macro_two_args() {
    let value1 = 42u32;
    let value2 = 23u32;

    let handle = spawn!((value1, value2) || value1 + value2);
    assert_eq!(handle.join().unwrap(), 42 + 23);

    let handle = spawn!((value1 => new_name1, value2) || new_name1 + value2);
    assert_eq!(handle.join().unwrap(), 42 + 23);

    let ref_value = &value1;

    let handle = spawn!((*ref_value, value2) || ref_value + value2);
    assert_eq!(handle.join().unwrap(), 42 + 23);

    let handle = spawn!((*ref_value => new_name, value2) || new_name + value2);
    assert_eq!(handle.join().unwrap(), 42 + 23);

    let handle = spawn!((mut value1, value2) || {
        value1 += 1;
        value1 + value2
    });
    assert_eq!(handle.join().unwrap(), 43 + 23);

    let handle = spawn!((value1 => mut new_name, value2) || {
        new_name += 1;
        new_name + value2
    });
    assert_eq!(handle.join().unwrap(), 43 + 23);
}

#[test]
fn test_macro_three_args() {
    let value1 = 42u32;
    let value2 = 23u32;
    let value3 = 99u32;

    let handle = spawn!((value1, value2, value3) || value1 + value2 + value3);
    assert_eq!(handle.join().unwrap(), 42 + 23 + 99);

    let handle = spawn!((value1 => new_name1, value2, value3) || new_name1 + value2 + value3);
    assert_eq!(handle.join().unwrap(), 42 + 23 + 99);

    let ref_value = &value1;

    let handle = spawn!((*ref_value, value2, value3) || ref_value + value2 + value3);
    assert_eq!(handle.join().unwrap(), 42 + 23 + 99);

    let handle = spawn!((*ref_value => new_name, value2, value3) || new_name + value2 + value3);
    assert_eq!(handle.join().unwrap(), 42 + 23 + 99);

    let handle = spawn!((mut value1, value2, value3) || {
        value1 += 1;
        value1 + value2 + value3
    });
    assert_eq!(handle.join().unwrap(), 43 + 23 + 99);

    let handle = spawn!((value1 => mut new_name, value2, value3) || {
        new_name += 1;
        new_name + value2 + value3
    });
    assert_eq!(handle.join().unwrap(), 43 + 23 + 99);
}

#[test]
fn test_macro_three_args_rv() {
    let value1 = 42u32;
    let value2 = 23u32;
    let value3 = 99u32;

    let handle =
        spawn!((value1, value2, value3) || -> Option<_> { Some(value1 + value2 + value3) });
    assert_eq!(handle.join().unwrap(), Some(42 + 23 + 99));

    let handle = spawn!((value1 => new_name1, value2, value3) || -> Option<_> { Some(new_name1 + value2 + value3) });
    assert_eq!(handle.join().unwrap(), Some(42 + 23 + 99));
}
