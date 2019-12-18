## mitosis

<!-- [![Build Status](https://travis-ci.org/Manishearth/elsa.svg?branch=master)](https://travis-ci.org/Manishearth/elsa) -->
[![Current Version](https://meritbadge.herokuapp.com/mitosis)](https://crates.io/crates/mitosis)
[![License: MIT/Apache-2.0](https://img.shields.io/crates/l/mitosis.svg)](#license)


WIP

This crate provides `mitosis::spawn()`, which is similar to `thread::spawn()` but will spawn a new process instead.


```rust

fn main() {
    // Needs to be near the beginning of your program
    mitosis::init();

    // some code
    let some_data = 5
    mitosis::spawn(some_data, |data| {
        println!("hello from another process, your data is {}", 5);
    });
}
```
