use std::fmt;
use std::io;

use ipc_channel::Error as IpcError;
use serde::{Deserialize, Serialize};

/// Represents a panic caugh across processes.
///
/// This contains the marshalled panic information so that it can be used
/// for other purposes.
#[derive(Serialize, Deserialize)]
pub struct Panic {
    msg: String,
    #[cfg(feature = "backtrace")]
    pub(crate) backtrace: Option<backtrace::Backtrace>,
}

impl Panic {
    /// Creates a new panic object.
    pub(crate) fn new(s: &str) -> Panic {
        Panic {
            msg: s.into(),
            #[cfg(feature = "backtrace")]
            backtrace: None,
        }
    }

    /// Returns the message of the panic.
    pub fn message(&self) -> &str {
        self.msg.as_str()
    }

    /// Returns a reference to the backtrace.
    ///
    /// Typically this backtrace is already resolved because it's currently
    /// not possible to cross the process boundaries with unresolved backtraces.
    #[cfg(feature = "backtrace")]
    pub fn backtrace(&self) -> Option<&backtrace::Backtrace> {
        self.backtrace.as_ref()
    }
}

impl fmt::Debug for Panic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Panic")
            .field("message", &self.message())
            .field("backtrace", &{
                #[cfg(feature = "backtrace")]
                {
                    self.backtrace()
                }
                #[cfg(not(feature = "backtrace"))]
                {
                    None::<()>
                }
            })
            .finish()
    }
}

impl fmt::Display for Panic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

/// Encapsulates errors of the procspawn crate.
///
/// In particular it gives access to remotely captured panics.
#[derive(Debug)]
pub struct SpawnError {
    kind: SpawnErrorKind,
}

#[derive(Debug)]
enum SpawnErrorKind {
    Ipc(IpcError),
    Io(io::Error),
    Panic(Panic),
    Cancelled,
    TimedOut,
}

impl SpawnError {
    /// If a panic ocurred this returns the captured panic info.
    pub fn panic_info(&self) -> Option<&Panic> {
        if let SpawnErrorKind::Panic(ref info) = self.kind {
            Some(info)
        } else {
            None
        }
    }

    /// True if this error indicates a cancellation.
    pub fn is_cancellation(&self) -> bool {
        if let SpawnErrorKind::Cancelled = self.kind {
            true
        } else {
            false
        }
    }

    /// True if this error indicates a timeout
    pub fn is_timeout(&self) -> bool {
        if let SpawnErrorKind::TimedOut = self.kind {
            true
        } else {
            false
        }
    }

    pub(crate) fn new_cancelled() -> SpawnError {
        SpawnError {
            kind: SpawnErrorKind::Cancelled,
        }
    }

    pub(crate) fn new_timeout() -> SpawnError {
        SpawnError {
            kind: SpawnErrorKind::TimedOut,
        }
    }
}

impl std::error::Error for SpawnError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind {
            SpawnErrorKind::Ipc(ref err) => Some(&*err),
            SpawnErrorKind::Io(ref err) => Some(&*err),
            SpawnErrorKind::Panic(_) => None,
            SpawnErrorKind::Cancelled => None,
            SpawnErrorKind::TimedOut => None,
        }
    }
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            SpawnErrorKind::Ipc(_) => write!(f, "process spawn error: ipc error"),
            SpawnErrorKind::Io(_) => write!(f, "process spawn error: i/o error"),
            SpawnErrorKind::Panic(ref p) => write!(f, "process spawn error: panic: {}", p),
            SpawnErrorKind::Cancelled => write!(f, "process spawn error: call cancelled"),
            SpawnErrorKind::TimedOut => write!(f, "process spawn error: timed out"),
        }
    }
}

impl From<IpcError> for SpawnError {
    fn from(err: IpcError) -> SpawnError {
        SpawnError {
            kind: SpawnErrorKind::Ipc(err),
        }
    }
}

impl From<io::Error> for SpawnError {
    fn from(err: io::Error) -> SpawnError {
        SpawnError {
            kind: SpawnErrorKind::Io(err),
        }
    }
}

impl From<Panic> for SpawnError {
    fn from(panic: Panic) -> SpawnError {
        SpawnError {
            kind: SpawnErrorKind::Panic(panic),
        }
    }
}
