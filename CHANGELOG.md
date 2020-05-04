# Changelog

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
