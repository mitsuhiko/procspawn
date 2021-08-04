use procspawn::{self, spawn};

#[tokio::main]
async fn main() {
    procspawn::init().await;

    let handle = spawn((1, vec![1, 2, 3]), |(a, b)| {
        println!("in process: {:?} {:?}", a, b);
        a + b.into_iter().sum::<i32>()
    })
    .await;

    println!("result: {}", handle.join().await.unwrap());
}
