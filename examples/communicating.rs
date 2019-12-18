use ipc_channel::ipc;
use mitosis;
fn main() {
    mitosis::init();
    let five = fibonacci_par(5);
    let ten = fibonacci_par(10);
    let thirty = fibonacci_par(30);
    assert_eq!(five.recv().unwrap(), 5);
    assert_eq!(ten.recv().unwrap(), 55);
    assert_eq!(thirty.recv().unwrap(), 832040);
    println!("Successfully calculated fibonacci values!");
}

fn fibonacci_par(n: u32) -> ipc::IpcReceiver<u32> {
    let (tx, rx) = ipc::channel().unwrap();

    mitosis::spawn((n, tx), |(n, tx)| {
        tx.send(fibonacci(n)).unwrap();
    });
    rx
}

fn fibonacci(n: u32) -> u32 {
    if n <= 2 {
        return 1;
    }
    fibonacci(n - 1) + fibonacci(n - 2)
}
