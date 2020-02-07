# procspawn

This crate provides the ability to spawn processes with a function similar
to `thread::spawn`.

```rust
procspawn::init();

let data = vec![1, 2, 3, 4];
let handle = procspawn::spawn(data, |data| {
    println!("Received data {:?}", &data);
    data.into_iter().sum::<i64>()
});
let result = handle.join().unwrap();
```

`spawn()` can pass arbitrary serializable data, including IPC senders
and receivers from the `ipc-channel` crate, down to the new process.

This crate is a fork of `mitosis`.

License: MIT/Apache-2.0
