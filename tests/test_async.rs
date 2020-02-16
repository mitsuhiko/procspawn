use std::thread::sleep;
use std::time::Duration;

use futures::executor::block_on;

procspawn::enable_test_support!();

#[test]
fn test_async_basic() {
    let result = block_on(async {
        let handle = procspawn::Builder::new().spawn_async((1u32, 2u32), |(a, b)| a + b);
        handle.join_async().await
    });

    assert_eq!(result.unwrap(), 3);
}

#[test]
fn test_async_kill() {
    let result = block_on(async {
        let mut handle = procspawn::Builder::new().spawn_async((1u32, 2u32), |(a, b)| {
            sleep(Duration::from_secs(10));
            a + b
        });
        assert!(handle.pid().is_some());
        handle.kill().unwrap();
        handle.join_async().await
    });

    assert!(result.unwrap_err().is_cancellation());
}
