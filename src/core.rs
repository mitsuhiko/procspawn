use std::env;
use std::ffi::{OsStr, OsString};
use std::io;
use std::mem;
use std::panic;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "safe-shared-libraries")]
use findshlibs::{Avma, IterationControl, Segment, SharedLibrary};

use ipc_channel::ipc::{self, IpcReceiver, IpcSender, OpaqueIpcReceiver, OpaqueIpcSender};
use ipc_channel::ErrorKind as IpcErrorKind;
use serde::{Deserialize, Serialize};

use crate::error::PanicInfo;
use crate::panic::{init_panic_hook, reset_panic_info, take_panic, BacktraceCapture};

pub const ENV_NAME: &str = "__PROCSPAWN_CONTENT_PROCESS_ID";
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static PASS_ARGS: AtomicBool = AtomicBool::new(false);

#[cfg(not(feature = "safe-shared-libraries"))]
static ALLOW_UNSAFE_SPAWN: AtomicBool = AtomicBool::new(false);

/// Asserts no shared libraries are used for functions spawned.
///
/// If the `safe-shared-libraries` feature is disabled this function must be
/// called once to validate that the application does not spawn functions
/// from a shared library.
///
/// This must be called once before the first call to a spawn function as
/// otherwise they will panic.
///
/// # Safety
///
/// You must only call this function if you can guarantee that none of your
/// `spawn` calls cross a shared library boundary.
pub unsafe fn assert_spawn_is_safe() {
    #[cfg(not(feature = "safe-shared-libraries"))]
    {
        ALLOW_UNSAFE_SPAWN.store(true, Ordering::SeqCst);
    }
}

/// Can be used to configure the process.
pub struct ProcConfig {
    callback: Option<Box<dyn FnOnce()>>,
    panic_handling: bool,
    pass_args: bool,
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
            pass_args: true,
            #[cfg(feature = "backtrace")]
            capture_backtraces: true,
            #[cfg(feature = "backtrace")]
            resolve_backtraces: true,
        }
    }
}

pub fn mark_initialized() {
    INITIALIZED.store(true, Ordering::SeqCst);
}

pub fn should_pass_args() -> bool {
    PASS_ARGS.load(Ordering::SeqCst)
}

fn find_shared_library_offset_by_name(name: &OsStr) -> isize {
    #[cfg(feature = "safe-shared-libraries")]
    {
        let mut result = None;
        findshlibs::TargetSharedLibrary::each(|shlib| {
            if shlib.name() == name {
                result = Some(
                    shlib
                        .segments()
                        .next()
                        .map_or(0, |x| x.actual_virtual_memory_address(shlib).0 as isize),
                );
                return IterationControl::Break;
            }
            IterationControl::Continue
        });
        match result {
            Some(rv) => rv,
            None => panic!("Unable to locate shared library {:?} in subprocess", name),
        }
    }
    #[cfg(not(feature = "safe-shared-libraries"))]
    {
        let _ = name;
        init as *const () as isize
    }
}

fn find_library_name_and_offset(f: *const u8) -> (OsString, isize) {
    #[cfg(feature = "safe-shared-libraries")]
    {
        let mut result = None;
        findshlibs::TargetSharedLibrary::each(|shlib| {
            let start = shlib
                .segments()
                .next()
                .map_or(0, |x| x.actual_virtual_memory_address(shlib).0 as isize);
            for seg in shlib.segments() {
                if seg.contains_avma(shlib, Avma(f)) {
                    result = Some((shlib.name().to_owned(), start));
                    return IterationControl::Break;
                }
            }
            IterationControl::Continue
        });
        result.expect("Unable to locate function pointer in loaded image")
    }
    #[cfg(not(feature = "safe-shared-libraries"))]
    {
        let _ = f;
        (OsString::new(), init as *const () as isize)
    }
}

impl ProcConfig {
    /// Creates a default proc config.
    pub fn new() -> ProcConfig {
        ProcConfig::default()
    }

    /// Attaches a callback that is used to initializes all processes.
    pub fn config_callback<F: FnOnce() + 'static>(&mut self, f: F) -> &mut Self {
        self.callback = Some(Box::new(f));
        self
    }

    /// Enables or disables argument passing.
    ///
    /// By default all arguments are forwarded to the spawned process.
    pub fn pass_args(&mut self, enabled: bool) -> &mut Self {
        self.pass_args = enabled;
        self
    }

    /// Configure the automatic panic handling.
    ///
    /// The default behavior is that panics are caught and that a panic handler
    /// is installed.
    pub fn panic_handling(&mut self, enabled: bool) -> &mut Self {
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
    pub fn capture_backtraces(&mut self, enabled: bool) -> &mut Self {
        self.capture_backtraces = enabled;
        self
    }

    /// Controls whether backtraces should be resolved.
    #[cfg(feature = "backtrace")]
    pub fn resolve_backtraces(&mut self, enabled: bool) -> &mut Self {
        self.resolve_backtraces = enabled;
        self
    }

    /// Consumes the config and initializes the process.
    pub fn init(&mut self) {
        mark_initialized();
        PASS_ARGS.store(self.pass_args, Ordering::SeqCst);

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
/// For more complex initializations see [`ProcConfig`](struct.ProcConfig.html).
pub fn init() {
    ProcConfig::default().init()
}

#[inline]
pub fn assert_spawn_okay() {
    if !INITIALIZED.load(Ordering::SeqCst) {
        panic!("procspawn was not initialized");
    }
    #[cfg(not(feature = "safe-shared-libraries"))]
    {
        if !ALLOW_UNSAFE_SPAWN.load(Ordering::SeqCst) {
            panic!(
                "spawn() prevented because safe-shared-library feature was \
                 disabled and assert_no_shared_libraries was not invoked."
            );
        }
    }
}

fn is_benign_bootstrap_error(err: &io::Error) -> bool {
    // on macos we will observe an unknown mach error
    err.kind() == io::ErrorKind::Other && err.to_string() == "Unknown Mach error: 44e"
}

fn bootstrap_ipc(token: String, config: &ProcConfig) {
    if config.panic_handling {
        init_panic_hook(config.backtrace_capture());
    }

    let connection_bootstrap: IpcSender<IpcSender<MarshalledCall>> = match IpcSender::connect(token)
    {
        Ok(sender) => sender,
        Err(err) => {
            if !is_benign_bootstrap_error(&err) {
                Err::<(), _>(err).expect("could not bootstrap ipc connection");
            }
            process::exit(1);
        }
    };
    let (tx, rx) = ipc::channel().unwrap();
    connection_bootstrap.send(tx).unwrap();
    let marshalled_call = rx.recv().unwrap();
    marshalled_call.call(config.panic_handling);
    process::exit(0);
}

/// Marshals a call across process boundaries.
#[derive(Serialize, Deserialize, Debug)]
pub struct MarshalledCall {
    pub lib_name: OsString,
    pub fn_offset: isize,
    pub wrapper_offset: isize,
    pub args_receiver: OpaqueIpcReceiver,
    pub return_sender: OpaqueIpcSender,
}

impl MarshalledCall {
    /// Marshalls the call.
    pub fn marshal<A, R>(
        f: fn(A) -> R,
        args_receiver: IpcReceiver<A>,
        return_sender: IpcSender<Result<R, PanicInfo>>,
    ) -> MarshalledCall
    where
        A: Serialize + for<'de> Deserialize<'de>,
        R: Serialize + for<'de> Deserialize<'de>,
    {
        let (lib_name, offset) = find_library_name_and_offset(f as *const () as *const u8);
        let init_loc = init as *const () as isize;
        let fn_offset = f as *const () as isize - offset as isize;
        let wrapper_offset = run_func::<A, R> as *const () as isize - init_loc;
        MarshalledCall {
            lib_name,
            fn_offset,
            wrapper_offset,
            args_receiver: args_receiver.to_opaque(),
            return_sender: return_sender.to_opaque(),
        }
    }

    /// Unmarshals and performs the call.
    pub fn call(self, panic_handling: bool) {
        unsafe {
            let ptr = self.wrapper_offset + init as *const () as isize;
            let func: fn(&OsStr, isize, OpaqueIpcReceiver, OpaqueIpcSender, bool) =
                mem::transmute(ptr);
            func(
                &self.lib_name,
                self.fn_offset,
                self.args_receiver,
                self.return_sender,
                panic_handling,
            );
        }
    }
}

unsafe fn run_func<A, R>(
    lib_name: &OsStr,
    fn_offset: isize,
    recv: OpaqueIpcReceiver,
    sender: OpaqueIpcSender,
    panic_handling: bool,
) where
    A: Serialize + for<'de> Deserialize<'de>,
    R: Serialize + for<'de> Deserialize<'de>,
{
    let lib_offset = find_shared_library_offset_by_name(lib_name) as isize;
    let function: fn(A) -> R = mem::transmute(fn_offset + lib_offset as *const () as isize);
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

    // sending can fail easily because of bincode limitations.  If you see
    // this in your tracebacks consider using the `Json` wrapper.
    if let Err(err) = sender.to().send(rv) {
        if let IpcErrorKind::Io(ref io) = *err {
            if io.kind() == io::ErrorKind::NotFound || io.kind() == io::ErrorKind::ConnectionReset {
                // this error is okay.  this means nobody actually
                // waited for the call, so we just ignore it.
                return;
            }
        } else {
            Err::<(), _>(err).expect("could not send event over ipc channel");
        }
    }
}
