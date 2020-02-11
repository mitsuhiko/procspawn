use procspawn::{self, spawn, Json};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct InnerStruct {
    value: u64,
}

#[derive(Serialize, Deserialize, Debug)]
struct BadStruct {
    #[serde(flatten)]
    inner: InnerStruct,
}

fn main() {
    procspawn::init();

    // json works:
    let handle = spawn((), |()| {
        Json(BadStruct {
            inner: InnerStruct { value: 42 },
        })
    });
    println!("result with JSON: {:?}", handle.join());

    // bincode fails:
    let handle = spawn((), |()| BadStruct {
        inner: InnerStruct { value: 42 },
    });
    println!("result with bincode: {:?}", handle.join());
}
