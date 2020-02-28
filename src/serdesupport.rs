use std::sync::atomic::{AtomicBool, Ordering};

thread_local! {
    static IN_PROCSPAWN: AtomicBool = AtomicBool::new(false);
}

struct ResetProcspawn(bool);

impl Drop for ResetProcspawn {
    fn drop(&mut self) {
        IN_PROCSPAWN.with(|in_procspawn| in_procspawn.swap(self.0, Ordering::Relaxed));
    }
}

/// Internal helper to mark all serde calls that go across processes
/// so that serializers can respond to it.
pub fn mark_procspawn_serde<F: FnOnce() -> R, R>(f: F) -> R {
    let old = IN_PROCSPAWN.with(|in_procspawn| in_procspawn.swap(true, Ordering::Relaxed));
    let _dropper = ResetProcspawn(old);
    f()
}

/// Checks if serde is in IPC mode.
///
/// This can be used to customize the serialization behavior of custom
/// types for IPC specific purposes.  This is useful when a type has a regular
/// serialization/deserialization behavior but you want to use a cheaper one
/// when procspawn is used.
///
/// An example of this can be a type that abstracts over an mmap.  It might want
/// to serialize the raw bytes under normal circumstances but for IPC purposes
/// might want to instead serialize the underlying file path and reopen the
/// mmap on the other side.
///
/// This function returns `true` whenever procspawn is attempting to serialize
/// and deserialize but never anytime else.  Internally this is implemented as a
/// thread local.
pub fn serde_in_ipc_mode() -> bool {
    IN_PROCSPAWN.with(|in_procspawn| in_procspawn.load(Ordering::Relaxed))
}
