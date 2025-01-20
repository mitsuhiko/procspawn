//! Utilities for working with serde.
//!
//! Because serde is the underlying library used for data passing between
//! processes procspawn provides various utilities that help with common
//! operations.
use ipc_channel::ipc::IpcSharedMemory;
use serde::{de::Deserializer, de::Error, de::Visitor, ser::Serializer};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

thread_local! {
    static IN_PROCSPAWN: AtomicBool = const { AtomicBool::new(false) };
}

struct ResetProcspawn(bool);

impl Drop for ResetProcspawn {
    fn drop(&mut self) {
        IN_PROCSPAWN.with(|in_procspawn| in_procspawn.swap(self.0, Ordering::Relaxed));
    }
}

/// Internal helper to mark all serde calls that go across processes
/// so that serializers can respond to it.
pub fn with_ipc_mode<F: FnOnce() -> R, R>(f: F) -> R {
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
pub fn in_ipc_mode() -> bool {
    IN_PROCSPAWN.with(|in_procspawn| in_procspawn.load(Ordering::Relaxed))
}

/// A read-only byte buffer for sending between processes.
///
/// The buffer behind the scenes uses shared memory which is faster send
/// between processes than to serialize the raw bytes directly.  It is however
/// read-only.
pub struct Shmem {
    shmem: IpcSharedMemory,
}

impl fmt::Debug for Shmem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct ByteRepr<'a>(&'a Shmem);

        impl fmt::Debug for ByteRepr<'_> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "b\"")?;
                for &byte in self.0.as_bytes() {
                    if (32..127).contains(&byte) {
                        write!(f, "{}", byte)?;
                    } else {
                        write!(f, "\\x{:02x}", byte)?;
                    }
                }
                write!(f, "b\"")?;
                Ok(())
            }
        }

        f.debug_tuple("Shmem").field(&ByteRepr(self)).finish()
    }
}

impl Shmem {
    /// Creates a buffer from some bytes.
    pub fn from_bytes(bytes: &[u8]) -> Shmem {
        Shmem {
            shmem: IpcSharedMemory::from_bytes(bytes),
        }
    }

    /// Returns the bytes inside.
    pub fn as_bytes(&self) -> &[u8] {
        &self.shmem
    }
}

impl std::ops::Deref for Shmem {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.shmem
    }
}

impl Serialize for Shmem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if in_ipc_mode() {
            self.shmem.serialize(serializer)
        } else {
            serializer.serialize_bytes(self.as_bytes())
        }
    }
}

struct ShmemVisitor;

impl<'de> Visitor<'de> for ShmemVisitor {
    type Value = Shmem;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a borrowed byte array")
    }

    fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(Shmem {
            shmem: IpcSharedMemory::from_bytes(v),
        })
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(Shmem {
            shmem: IpcSharedMemory::from_bytes(v.as_bytes()),
        })
    }
}

impl<'de> Deserialize<'de> for Shmem {
    fn deserialize<D>(deserializer: D) -> Result<Shmem, D::Error>
    where
        D: Deserializer<'de>,
    {
        if in_ipc_mode() {
            Ok(Shmem {
                shmem: IpcSharedMemory::deserialize(deserializer)?,
            })
        } else {
            deserializer.deserialize_bytes(ShmemVisitor)
        }
    }
}

#[cfg(feature = "json")]
pub use crate::json::Json;
