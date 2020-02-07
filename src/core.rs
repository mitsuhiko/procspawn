use std::any::Any;
use std::cell::RefCell;
use std::env;
use std::mem;
use std::panic;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};

use ipc_channel::ipc::{self, IpcSender, OpaqueIpcReceiver, OpaqueIpcSender};
use serde::{Deserialize, Serialize};

use crate::error::Panic;

pub const ENV_NAME: &str = "__PROCSPAWN_CONTENT_PROCESS_ID";
static INITIALIZED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static PANIC_INFO: RefCell<Option<Panic>> = RefCell::new(None);
}

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

#[derive(Serialize, Deserialize, Debug)]
pub struct BootstrapData {
    pub wrapper_offset: isize,
    pub args_receiver: OpaqueIpcReceiver,
    pub return_sender: OpaqueIpcSender,
}

fn bootstrap_ipc(token: String, config: &ProcConfig) {
    if config.panic_handling {
        init_panic_hook(config);
    }

    let connection_bootstrap: IpcSender<IpcSender<BootstrapData>> =
        IpcSender::connect(token).unwrap();
    let (tx, rx) = ipc::channel().unwrap();
    connection_bootstrap.send(tx).unwrap();
    let bootstrap_data = rx.recv().unwrap();
    unsafe {
        let ptr = bootstrap_data.wrapper_offset + init as *const () as isize;
        let func: fn(OpaqueIpcReceiver, OpaqueIpcSender, &ProcConfig) = mem::transmute(ptr);
        func(
            bootstrap_data.args_receiver,
            bootstrap_data.return_sender,
            config,
        );
    }
    process::exit(0);
}

pub fn get_wrapper_offset<F, A, R>() -> isize
where
    F: FnOnce(A) -> R,
    A: Serialize + for<'de> Deserialize<'de>,
    R: Serialize + for<'de> Deserialize<'de>,
{
    let init_loc = init as *const () as isize;
    run_func::<F, A, R> as *const () as isize - init_loc
}

fn reset_panic_info() {
    PANIC_INFO.with(|pi| {
        *pi.borrow_mut() = None;
    });
}

fn take_panic(panic: &(dyn Any + Send + 'static)) -> Panic {
    PANIC_INFO
        .with(|pi| pi.borrow_mut().take())
        .unwrap_or_else(move || serialize_panic(panic))
}

#[derive(Copy, Clone)]
enum BacktraceCapture {
    No,
    #[cfg(feature = "backtrace")]
    Resolved,
    #[cfg(feature = "backtrace")]
    Unresolved,
}

fn panic_handler(info: &panic::PanicInfo<'_>, capture_backtraces: BacktraceCapture) {
    PANIC_INFO.with(|pi| {
        #[allow(unused_mut)]
        let mut panic = serialize_panic(info.payload());
        match capture_backtraces {
            BacktraceCapture::No => {}
            #[cfg(feature = "backtrace")]
            BacktraceCapture::Resolved => {
                panic.backtrace = Some(backtrace::Backtrace::new());
            }
            #[cfg(feature = "backtrace")]
            BacktraceCapture::Unresolved => {
                panic.backtrace = Some(backtrace::Backtrace::new_unresolved());
            }
        }
        *pi.borrow_mut() = Some(panic);
    });
}

fn init_panic_hook(config: &ProcConfig) {
    let next = panic::take_hook();
    let capture_backtraces = {
        #[cfg(feature = "backtrace")]
        {
            match (config.capture_backtraces, config.resolve_backtraces) {
                (false, _) => BacktraceCapture::No,
                (true, true) => BacktraceCapture::Resolved,
                (true, false) => BacktraceCapture::Unresolved,
            }
        }
        #[cfg(not(feature = "backtrace"))]
        {
            let _config = config;
            BacktraceCapture::No
        }
    };
    panic::set_hook(Box::new(move |info| {
        panic_handler(info, capture_backtraces);
        next(info);
    }));
}

fn serialize_panic(panic: &(dyn Any + Send + 'static)) -> Panic {
    Panic::new(match panic.downcast_ref::<&'static str>() {
        Some(s) => *s,
        None => match panic.downcast_ref::<String>() {
            Some(s) => &s[..],
            None => "Box<Any>",
        },
    })
}

unsafe fn run_func<F, A, R>(recv: OpaqueIpcReceiver, sender: OpaqueIpcSender, config: &ProcConfig)
where
    F: FnOnce(A) -> R,
    A: Serialize + for<'de> Deserialize<'de>,
    R: Serialize + for<'de> Deserialize<'de>,
{
    let function: F = mem::zeroed();
    let args = recv.to().recv().unwrap();
    let rv = if config.panic_handling {
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
