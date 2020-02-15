use futures::executor::block_on;

fn main() {
    procspawn::init();

    let result = block_on(async {
        let handle = procspawn::Builder::new().spawn_async((1, 2), |(a, b)| {
            println!("in process: {:?} {:?}", a, b);
            a + b
        });
        handle.join_async().await
    });

    println!("result: {}", result.unwrap());
}
