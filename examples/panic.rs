use procspawn::{self, spawn};

fn main() {
    procspawn::init();

    let handle = spawn((), |()| {
        panic!("Whatever!");
    });

    match handle.join() {
        Ok(()) => unreachable!(),
        Err(err) => {
            let panic = err.panic_info().expect("got a non panic error");
            println!("process panicked with {}", panic.message());
            println!("{:#?}", panic);
        }
    }
}
