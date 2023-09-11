use std::fmt;
use std::io;

use ipc_channel::ipc::{IpcError, TryRecvError};
use ipc_channel::{Error as BincodeError, ErrorKind as BincodeErrorKind};
use serde::{Deserialize, Serialize};

/// Represents a panic caugh across processes.
///
/// This contains the marshalled panic information so that it can be used
/// for other purposes.
///
/// This is similar to `std::panic::PanicInfo` but can cross process boundaries.
#[derive(Serialize, Deserialize)]
pub struct PanicInfo {
    msg: String,
    pub(crate) location: Option<Location>,
    #[cfg(feature = "backtrace")]
    pub(crate) backtrace: Option<backtrace::Backtrace>,
}

/// Location of a panic.
///
/// This is similar to `std::panic::Location` but can cross process boundaries.
#[derive(Serialize, Deserialize, Debug)]
pub struct Location {
    file: String,
    line: u32,
    column: u32,
}

impl Location {
    pub(crate) fn from_std(loc: &std::panic::Location) -> Location {
        Location {
            file: loc.file().into(),
            line: loc.line(),
            column: loc.column(),
        }
    }

    /// Returns the name of the source file from which the panic originated.
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the line number from which the panic originated.
    pub fn line(&self) -> u32 {
        self.line
    }

    /// Returns the column from which the panic originated.
    pub fn column(&self) -> u32 {
        self.column
    }
}

impl PanicInfo {
    /// Creates a new panic object.
    pub(crate) fn new(s: &str) -> PanicInfo {
        PanicInfo {
            msg: s.into(),
            location: None,
            #[cfg(feature = "backtrace")]
            backtrace: None,
        }
    }

    /// Returns the message of the panic.
    pub fn message(&self) -> &str {
        self.msg.as_str()
    }

    /// Returns the panic location.
    pub fn location(&self) -> Option<&Location> {
        self.location.as_ref()
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

impl fmt::Debug for PanicInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PanicInfo")
            .field("message", &self.message())
            .field("location", &self.location())
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

impl fmt::Display for PanicInfo {
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
    Bincode(BincodeError),
    Io(io::Error),
    Panic(PanicInfo),
    IpcChannelClosed(io::Error),
    Cancelled,
    TimedOut,
    Consumed,
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

    /// True if this error indicates a cancellation.
    pub fn is_cancellation(&self) -> bool {
        matches!(self.kind, SpawnErrorKind::Cancelled)
    }

    /// True if this error indicates a timeout.
    pub fn is_timeout(&self) -> bool {
        matches!(self.kind, SpawnErrorKind::TimedOut)
    }

    /// True if this means the remote side closed.
    pub fn is_remote_close(&self) -> bool {
        matches!(self.kind, SpawnErrorKind::IpcChannelClosed(..))
    }

    pub(crate) fn new_remote_close() -> SpawnError {
        SpawnError {
            kind: SpawnErrorKind::IpcChannelClosed(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "remote closed",
            )),
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

    pub(crate) fn new_consumed() -> SpawnError {
        SpawnError {
            kind: SpawnErrorKind::Consumed,
        }
    }
}

impl std::error::Error for SpawnError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind {
            SpawnErrorKind::Bincode(ref err) => Some(err),
            SpawnErrorKind::Io(ref err) => Some(err),
            SpawnErrorKind::Panic(_) => None,
            SpawnErrorKind::Cancelled => None,
            SpawnErrorKind::TimedOut => None,
            SpawnErrorKind::Consumed => None,
            SpawnErrorKind::IpcChannelClosed(ref err) => Some(err),
        }
    }
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            SpawnErrorKind::Bincode(_) => write!(f, "process spawn error: bincode error"),
            SpawnErrorKind::Io(_) => write!(f, "process spawn error: i/o error"),
            SpawnErrorKind::Panic(ref p) => write!(f, "process spawn error: panic: {}", p),
            SpawnErrorKind::Cancelled => write!(f, "process spawn error: call cancelled"),
            SpawnErrorKind::TimedOut => write!(f, "process spawn error: timed out"),
            SpawnErrorKind::Consumed => write!(f, "process spawn error: result already consumed"),
            SpawnErrorKind::IpcChannelClosed(_) => write!(
                f,
                "process spawn error: remote side closed (might have panicked on serialization)"
            ),
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

impl From<TryRecvError> for SpawnError {
    fn from(err: TryRecvError) -> SpawnError {
        match err {
            TryRecvError::Empty => SpawnError::new_remote_close(),
            TryRecvError::IpcError(err) => SpawnError::from(err),
        }
    }
}

impl From<IpcError> for SpawnError {
    fn from(err: IpcError) -> SpawnError {
        // unwrap nested IO errors
        match err {
            IpcError::Io(err) => SpawnError::from(err),
            IpcError::Bincode(err) => SpawnError::from(err),
            IpcError::Disconnected => SpawnError::new_remote_close(),
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
