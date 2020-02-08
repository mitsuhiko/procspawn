use std::env;
use std::mem;
use std::panic;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};

use ipc_channel::ipc::{self, IpcReceiver, IpcSender, OpaqueIpcReceiver, OpaqueIpcSender};
use serde::{Deserialize, Serialize};

use crate::error::Panic;
use crate::panic::{init_panic_hook, reset_panic_info, take_panic, BacktraceCapture};

pub const ENV_NAME: &str = "__PROCSPAWN_CONTENT_PROCESS_ID";
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Can be used to configure the process.
pub struct ProcConfig {
    callback: Option<Box<dyn FnOnce()>>,
    panic_handling: bool,
    #[cfg(feature = "backtrace")]
    capture_backtraces: bool,
    #[cfg(feature = "backtrace")]
    resolve_backtraces: bool,
}

impl Default for ProcConfig {
    fn default() -> ProcConfig {
        ProcConfig {
            callback: None,
            panic_handling: true,
            #[cfg(feature = "backtrace")]
            capture_backtraces: true,
            #[cfg(feature = "backtrace")]
            resolve_backtraces: true,
        }
    }
}

impl ProcConfig {
    /// Creates a default proc config.
    pub fn new() -> ProcConfig {
        ProcConfig::default()
    }

    /// Attaches a callback that is used to initializes all processes.
    pub fn config_callback<F: FnOnce() + 'static>(mut self, f: F) -> ProcConfig {
        self.callback = Some(Box::new(f));
        self
    }

    /// Configure the automatic panic handling.
    ///
    /// The default behavior is that panics are caught and that a panic handler
    /// is installed.
    pub fn panic_handling(mut self, enabled: bool) -> ProcConfig {
        self.panic_handling = enabled;
        self
    }

    /// Configures if backtraces should be captured.
    ///
    /// The default behavior is that if panic handling is enabled backtraces
    /// will be captured.
    ///
    /// This requires the `backtrace` feature.
    #[cfg(feature = "backtrace")]
    pub fn capture_backtraces(mut self, enabled: bool) -> ProcConfig {
        self.capture_backtraces = enabled;
        self
    }

    /// Controls whether backtraces should be resolved.
    #[cfg(feature = "backtrace")]
    pub fn resolve_backtraces(mut self, enabled: bool) -> ProcConfig {
        self.resolve_backtraces = enabled;
        self
    }

    /// Consumes the config and initializes the process.
    pub fn init(mut self) {
        INITIALIZED.store(true, Ordering::SeqCst);

        if let Ok(token) = env::var(ENV_NAME) {
            // permit nested invocations
            std::env::remove_var(ENV_NAME);
            if let Some(callback) = self.callback.take() {
                callback();
            }
            bootstrap_ipc(token, &self);
        }
    }

    fn backtrace_capture(&self) -> BacktraceCapture {
        #[cfg(feature = "backtrace")]
        {
            match (self.capture_backtraces, self.resolve_backtraces) {
                (false, _) => BacktraceCapture::No,
                (true, true) => BacktraceCapture::Resolved,
                (true, false) => BacktraceCapture::Unresolved,
            }
        }
        #[cfg(not(feature = "backtrace"))]
        {
            BacktraceCapture::No
        }
    }
}

/// Initializes procspawn.
///
/// This function must be called at the beginning of `main`.  Whatever comes
/// before it is also executed for all processes spawned through the `spawn`
/// function.
///
/// For more complex initializations see `ProcConfig`.
pub fn init() {
    ProcConfig::default().init()
}

#[inline]
pub fn assert_initialized() {
    if !INITIALIZED.load(Ordering::SeqCst) {
        panic!("procspawn was not initialized");
    }
}

fn bootstrap_ipc(token: String, config: &ProcConfig) {
    if config.panic_handling {
        init_panic_hook(config.backtrace_capture());
    }

    let connection_bootstrap: IpcSender<IpcSender<MarshalledCall>> =
        IpcSender::connect(token).unwrap();
    let (tx, rx) = ipc::channel().unwrap();
    connection_bootstrap.send(tx).unwrap();
    let marshalled_call = rx.recv().unwrap();
    marshalled_call.call(config.panic_handling);
    process::exit(0);
}

/// Marshals a call across process boundaries.
#[derive(Serialize, Deserialize, Debug)]
pub struct MarshalledCall {
    pub wrapper_offset: isize,
    pub args_receiver: OpaqueIpcReceiver,
    pub return_sender: OpaqueIpcSender,
}

impl MarshalledCall {
    /// Marshalls the call.
    pub fn marshal<F, A, R>(
        args_receiver: IpcReceiver<A>,
        return_sender: IpcSender<Result<R, Panic>>,
    ) -> MarshalledCall
    where
        F: FnOnce(A) -> R,
        A: Serialize + for<'de> Deserialize<'de>,
        R: Serialize + for<'de> Deserialize<'de>,
    {
        if mem::size_of::<F>() != 0 {
            panic!("marshallig of closures that capture data is not supported!");
        }
        MarshalledCall {
            wrapper_offset: get_wrapper_offset::<F, A, R>(),
            args_receiver: args_receiver.to_opaque(),
            return_sender: return_sender.to_opaque(),
        }
    }

    /// Unmarshals and performs the call.
    pub fn call(self, panic_handling: bool) {
        unsafe {
            let ptr = self.wrapper_offset + init as *const () as isize;
            let func: fn(OpaqueIpcReceiver, OpaqueIpcSender, bool) = mem::transmute(ptr);
            func(self.args_receiver, self.return_sender, panic_handling);
        }
    }
}

fn get_wrapper_offset<F, A, R>() -> isize
where
    F: FnOnce(A) -> R,
    A: Serialize + for<'de> Deserialize<'de>,
    R: Serialize + for<'de> Deserialize<'de>,
{
    let init_loc = init as *const () as isize;
    run_func::<F, A, R> as *const () as isize - init_loc
}

unsafe fn run_func<F, A, R>(recv: OpaqueIpcReceiver, sender: OpaqueIpcSender, panic_handling: bool)
where
    F: FnOnce(A) -> R,
    A: Serialize + for<'de> Deserialize<'de>,
    R: Serialize + for<'de> Deserialize<'de>,
{
    let function: F = mem::zeroed();
    let args = recv.to().recv().unwrap();
    let rv = if panic_handling {
        reset_panic_info();
        match panic::catch_unwind(panic::AssertUnwindSafe(|| function(args))) {
            Ok(rv) => Ok(rv),
            Err(panic) => Err(take_panic(&*panic)),
        }
    } else {
        Ok(function(args))
    };
    let _ = sender.to().send(rv);
}
