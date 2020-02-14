# Changelog

## 0.6.0

* Calls from the test support not have stdout disabled by default
  ([#22])(https://github.com/mitsuhiko/procspawn/pull/22)
* Spurious "Unknown Mach error: 44e" on the remote side is now
  silenced on macOS when the caller disconnects.
