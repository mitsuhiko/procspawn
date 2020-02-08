# procspawn

This crate provides the ability to spawn processes with a function similar
to `thread::spawn`.

Unlike `thread::spawn` data cannot be passed in closures but must be
explicitly passed as single argument which must be [`serde`](https://serde.rs/)
serializable.  The return value from the spawned closure also must be
serializable and can then be unwrapped from the returned join handle.

```rust
procspawn::init();

let data = vec![1, 2, 3, 4];
let handle = procspawn::spawn(data, |data| {
    println!("Received data {:?}", &data);
    data.into_iter().sum::<i64>()
});
let result = handle.join().unwrap();
```

Because `procspawn` will invoke a subprocess and there is currently no
reliable way to intercept `main` in Rust it's necessary for you to call
[`procspawn::init`](fn.init.html) at an early time in the program. The
place where this will be called is the entrypoint for the subprocesses
spawned.

[`spawn`](fn.spawn.html) can pass arbitrary serializable data, including
IPC senders and receivers from the [`ipc-channel`](https://crates.io/crates/ipc-channel)
crate, down to the new process.

### Differences to Mitosis

This crate is a fork of the `mitosis` crate with various differences in
how they operate.  The most obvious one is that `procspawn` is very
opinionated about error handling and will automatically capture and
send backtraces across the process boundaries.  Additionally `procspawn`
provides a process pool.

### Feature Flags

The following feature flags exist:

* `backtrace`: this feature is enabled by default.  When in use then
  backtraces are captured with the `backtrace-rs` crate and serialized
  across process boundaries.
* `test-support`: when this feature is enabled procspawn can be used
  with rusttest.  See [`enable_test_support!`](macro.enable_test_support.html)
  for more information.

License: MIT/Apache-2.0
