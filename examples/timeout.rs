use procspawn::{self, spawn};
use std::thread;
use std::time::Duration;

fn main() {
    procspawn::init();

    let handle = spawn((), |()| {
        thread::sleep(Duration::from_secs(10));
    });

    #[cfg(unix)]
    {
        println!("result: {:?}", handle.join_timeout(Duration::from_secs(1)));
    }
    #[cfg(windows)]
    {
        eprintln!("Warning: windows does not yet support timeouts");
        println!("result: {:?}", handle.join());
    }
}
