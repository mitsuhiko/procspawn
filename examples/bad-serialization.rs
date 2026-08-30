use procspawn::{self, serde::Json, spawn};
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
    println!("JSON lets you send a flattened object through:");
    let handle = spawn((), |()| {
        Json(BadStruct {
            inner: InnerStruct { value: 42 },
        })
    });
    println!("result with JSON: {:?}", handle.join());

    println!("the default binary serializer currently does not permit this:");
    // Binary serialization fails:
    let handle = spawn((), |()| BadStruct {
        inner: InnerStruct { value: 42 },
    });
    println!("result with the default serializer: {:?}", handle.join());
}
