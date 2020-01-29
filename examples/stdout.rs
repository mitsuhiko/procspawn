use mitosis;

use std::io::Read;

fn main() {
    mitosis::init();

    let mut builder = mitosis::Builder::new();
    builder.stdout(std::process::Stdio::piped());
    let mut handle = builder.spawn((1, 2), |(a, b)| {
        println!("{:?} {:?}", a, b);
    });

    let mut s = String::new();
    handle.stdout().unwrap().read_to_string(&mut s).unwrap();
    assert_eq!(s, "1 2\n");
}
