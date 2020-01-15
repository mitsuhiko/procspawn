use mitosis;

fn main() {
    mitosis::init();

    let handle = mitosis::spawn((), |()| loop {});

    handle.kill().unwrap();
}
