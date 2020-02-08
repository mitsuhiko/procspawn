use std::io::Read;
use std::process::Stdio;

fn main() {
    procspawn::init();

    let mut builder = procspawn::Builder::new();
    builder.stdout(Stdio::piped());

    let mut handle = builder.spawn((1, 2), |(a, b)| {
        println!("{:?} {:?}", a, b);
    });

    let mut s = String::new();
    handle.stdout().unwrap().read_to_string(&mut s).unwrap();
    assert_eq!(s, "1 2\n");
}
