use std::env;
use std::ffi::{OsStr, OsString};
use std::future::Future;
use std::io;
use std::mem;
use std::panic;
use std::pin::Pin;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "safe-shared-libraries")]
use findshlibs::{Avma, IterationControl, Segment, SharedLibrary};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio_unix_ipc::panic::{catch_panic, init_panic_hook};
use tokio_unix_ipc::{RawReceiver, RawSender, Receiver, Sender};

use crate::error::PanicInfo;

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
}

impl Default for ProcConfig {
    fn default() -> ProcConfig {
        ProcConfig {
            callback: None,
            panic_handling: true,
            pass_args: true,
            #[cfg(feature = "backtrace")]
            capture_backtraces: true,
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
                if seg.contains_avma(shlib, Avma(f as usize)) {
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

    /// Consumes the config and initializes the process.
    pub async fn init(&mut self) {
        mark_initialized();
        PASS_ARGS.store(self.pass_args, Ordering::SeqCst);

        if let Ok(token) = env::var(ENV_NAME) {
            // permit nested invocations
            std::env::remove_var(ENV_NAME);
            if let Some(callback) = self.callback.take() {
                callback();
            }
            bootstrap_ipc(token, &self).await;
        }
    }

    fn backtrace_capture(&self) -> bool {
        #[cfg(feature = "backtrace")]
        {
            self.capture_backtraces
        }
        #[cfg(not(feature = "backtrace"))]
        {
            false
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
pub async fn init() {
    ProcConfig::default().init().await
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
                 disabled and assert_spawn_is_safe was not invoked."
            );
        }
    }
}

fn is_benign_bootstrap_error(err: &io::Error) -> bool {
    // on macos we will observe an unknown mach error
    err.kind() == io::ErrorKind::Other && err.to_string() == "Unknown Mach error: 44e"
}

async fn bootstrap_ipc(token: String, config: &ProcConfig) {
    if config.panic_handling {
        init_panic_hook(config.backtrace_capture());
    }

    {
        let connection_bootstrap: Receiver<MarshalledCall> = match Receiver::connect(token).await {
            Ok(receiver) => receiver,
            Err(err) => {
                if !is_benign_bootstrap_error(&err) {
                    Err::<(), _>(err).expect("could not bootstrap ipc connection");
                }
                process::exit(1);
            }
        };
        let marshalled_call = connection_bootstrap.recv().await.unwrap();
        marshalled_call.call(config.panic_handling).await;
    }
    process::exit(0);
}

/// Marshals a call across process boundaries.
#[derive(Serialize, Deserialize, Debug)]
pub struct MarshalledCall {
    pub lib_name: OsString,
    pub fn_offset: isize,
    pub wrapper_offset: isize,
    pub args_receiver: RawReceiver,
    pub return_sender: RawSender,
}

impl MarshalledCall {
    /// Marshalls the call.
    pub fn marshal<A, R>(
        f: fn(A) -> R,
        args_receiver: Receiver<A>,
        return_sender: Sender<Result<R, PanicInfo>>,
    ) -> MarshalledCall
    where
        A: Serialize + DeserializeOwned,
        R: Serialize + DeserializeOwned,
    {
        let (lib_name, offset) = find_library_name_and_offset(f as *const () as *const u8);
        let init_loc = init as *const () as isize;
        let fn_offset = f as *const () as isize - offset as isize;
        let wrapper_offset = run_func::<A, R> as *const () as isize - init_loc;
        MarshalledCall {
            lib_name,
            fn_offset,
            wrapper_offset,
            args_receiver: args_receiver.into_raw_receiver(),
            return_sender: return_sender.into_raw_sender(),
        }
    }

    /// Unmarshals and performs the call.
    pub async fn call(self, panic_handling: bool) {
        unsafe {
            let init_loc = init as *const () as isize;
            let ptr = self.wrapper_offset + init_loc;
            let func: fn(
                &OsStr,
                isize,
                RawReceiver,
                RawSender,
                bool,
            ) -> Pin<Box<dyn Future<Output = ()>>> = mem::transmute(ptr);
            func(
                &self.lib_name,
                self.fn_offset,
                self.args_receiver,
                self.return_sender,
                panic_handling,
            )
            .await;
        }
    }
}

unsafe fn run_func<A, R>(
    lib_name: &OsStr,
    fn_offset: isize,
    args_recv: RawReceiver,
    sender: RawSender,
    panic_handling: bool,
) -> Pin<Box<dyn Future<Output = ()>>>
where
    A: Serialize + DeserializeOwned,
    R: Serialize + DeserializeOwned,
{
    let lib_name = lib_name.to_owned();
    Box::pin(async move {
        let lib_offset = find_shared_library_offset_by_name(&lib_name) as isize;
        let function: fn(A) -> R = mem::transmute(fn_offset + lib_offset as *const () as isize);
        let args = Receiver::<A>::from(args_recv).recv().await.unwrap();
        let rv = if panic_handling {
            match catch_panic(|| function(args)) {
                Ok(rv) => Ok(rv),
                Err(panic) => Err(panic),
            }
        } else {
            Ok(function(args))
        };

        // sending can fail easily because of bincode limitations.  If you see
        // this in your tracebacks consider using the `Structural` wrapper.
        if let Err(err) = Sender::<Result<R, PanicInfo>>::from(sender).send(rv).await {
            Err::<(), _>(err).expect("could not send event over ipc channel");
        }
    })
}
