# Changelog

## 0.8.0

* Added support for `spawn!` and `spawn_async!` macros
  ([#25](https://github.com/mitsuhiko/procspawn/pull/25))

## 0.7.0

* Added basic support for using this crate with async/await
  ([#24](https://github.com/mitsuhiko/procspawn/pull/24))

## 0.6.0

* Calls from the test support not have stdout disabled by default
  ([#22](https://github.com/mitsuhiko/procspawn/pull/22))
* Spurious "Unknown Mach error: 44e" on the remote side is now
  silenced on macOS when the caller disconnects.
