# Changelog

## 1.0.1

* Removed winapi dependency
* Upgraded backtrace to newer minimal version
* Bump MSRV to 1.70
* Update ipc-channel dependency

## 1.0.0

* Changes the `join` and `join_timeout` API to no longer consume the handle.

## 0.10.3

* Update ipc-channel to 0.16.1.

## 0.10.2

* Update ipc-channel to 0.16.

## 0.10.1

* Fixed some clippy warnings.
* Fixed a test that timed out in CI.

## 0.10.0

* Upgraded ctor
* Name the pool monitoring threads

## 0.9.0

* Removed experimental async support.

## 0.8.4

* Added pool support for macros.

## 0.8.3

* Resolved a deadlock when large args are sent over the
  IPC boundary.
  ([#31](https://github.com/mitsuhiko/procspawn/pull/31))

## 0.8.2

* Detect path to test module in case enable test mode is in a
  submodule.
  ([#28](https://github.com/mitsuhiko/procspawn/pull/28))
* Fixed zombies being left behind.
  ([#27](https://github.com/mitsuhiko/procspawn/pull/27))

## 0.8.1

* Fixed test support not working correctly for other crates.
  ([#26](https://github.com/mitsuhiko/procspawn/pull/26))

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
