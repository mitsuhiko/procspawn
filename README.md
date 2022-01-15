# procspawn

[![Crates.io](https://img.shields.io/crates/d/procspawn.svg)](https://crates.io/crates/procspawn)
[![Documentation](https://docs.rs/procspawn/badge.svg)](https://docs.rs/procspawn)
[![rustc 1.42.0](https://img.shields.io/badge/rust-1.42%2B-orange.svg)](https://img.shields.io/badge/rust-1.42%2B-orange.svg)

This crate provides the ability to spawn processes with a function similar
to `thread::spawn`.

Unlike `thread::spawn` data cannot be passed by the use of closures.  Instead
if must be explicitly passed as serializable object (specifically it must be
[`serde`](https://serde.rs/) serializable).  The return value from the
spawned closure also must be serializable and can then be retrieved from
the returned join handle.

If the spawned functiom causes a panic it will also be serialized across
the process boundaries.

## Example

First for all of this to work you need to invoke `procspawn::init` at a
point early in your program (somewhere at the beginning of the main function).
Whatever happens before that point also happens in your spawned functions.

Subprocesses are by default invoked with the same arguments and environment
variables as the parent process.

```rust
procspawn::init();
```

Now you can start spawning functions:

```rust
let data = vec![1, 2, 3, 4];
let handle = procspawn::spawn(data, |data| {
    println!("Received data {:?}", &data);
    data.into_iter().sum::<i64>()
});
let result = handle.join().unwrap();
```

## License and Links

- [Documentation](https://docs.rs/procspawn/)
- [Issue Tracker](https://github.com/mitsuhiko/procspawn/issues)
- [Examples](https://github.com/mitsuhiko/procspawn/tree/master/examples)
- License: [Apache-2.0](https://github.com/mitsuhiko/procspawn/blob/master/LICENSE-APACHE)
