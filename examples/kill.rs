use mitosis;

#[allow(clippy::empty_loop)]
fn main() {
    mitosis::init();

    let handle = mitosis::spawn((), |()| loop {});

    handle.kill().unwrap();
}
