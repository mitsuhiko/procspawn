use std::fmt;
use std::io;

use bincode::{Error as BincodeError, ErrorKind as BincodeErrorKind};

pub use tokio_unix_ipc::panic::{Location, PanicInfo};

/// Encapsulates errors of the procspawn crate.
///
/// In particular it gives access to remotely captured panics.
#[derive(Debug)]
pub struct SpawnError {
    kind: SpawnErrorKind,
}

#[derive(Debug)]
enum SpawnErrorKind {
    Bincode(BincodeError),
    Io(io::Error),
    Panic(PanicInfo),
    IpcChannelClosed(io::Error),
}

impl SpawnError {
    /// If a panic ocurred this returns the captured panic info.
    pub fn panic_info(&self) -> Option<&PanicInfo> {
        if let SpawnErrorKind::Panic(ref info) = self.kind {
            Some(info)
        } else {
            None
        }
    }

    /// True if this error comes from a panic.
    pub fn is_panic(&self) -> bool {
        self.panic_info().is_some()
    }
}

impl std::error::Error for SpawnError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind {
            SpawnErrorKind::Bincode(ref err) => Some(&*err),
            SpawnErrorKind::Io(ref err) => Some(&*err),
            SpawnErrorKind::Panic(_) => None,
            SpawnErrorKind::IpcChannelClosed(_) => None,
        }
    }
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            SpawnErrorKind::Bincode(_) => write!(f, "process spawn error: bincode error"),
            SpawnErrorKind::Io(_) => write!(f, "process spawn error: i/o error"),
            SpawnErrorKind::Panic(ref p) => write!(f, "process spawn error: panic: {}", p),
            SpawnErrorKind::IpcChannelClosed(_) => write!(f, "the ipc channel is closed: "),
        }
    }
}

impl From<BincodeError> for SpawnError {
    fn from(err: BincodeError) -> SpawnError {
        // unwrap nested IO errors
        if let BincodeErrorKind::Io(io_err) = *err {
            return SpawnError::from(io_err);
        }
        SpawnError {
            kind: SpawnErrorKind::Bincode(err),
        }
    }
}

impl From<io::Error> for SpawnError {
    fn from(err: io::Error) -> SpawnError {
        if let io::ErrorKind::ConnectionReset = err.kind() {
            return SpawnError {
                kind: SpawnErrorKind::IpcChannelClosed(err),
            };
        }
        SpawnError {
            kind: SpawnErrorKind::Io(err),
        }
    }
}

impl From<PanicInfo> for SpawnError {
    fn from(panic: PanicInfo) -> SpawnError {
        SpawnError {
            kind: SpawnErrorKind::Panic(panic),
        }
    }
}
