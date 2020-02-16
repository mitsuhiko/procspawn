use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;
use std::process::Stdio;
use std::process::{ChildStderr, ChildStdin, ChildStdout};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{env, mem, process};
use std::{io, thread};

use ipc_channel::ipc::{self, IpcOneShotServer, IpcReceiver, IpcSender};
use serde::{de::DeserializeOwned, Serialize};

use crate::core::{assert_spawn_okay, should_pass_args, MarshalledCall, ENV_NAME};
use crate::error::{PanicInfo, SpawnError};
use crate::pool::PooledHandle;

#[cfg(feature = "async")]
use crate::asyncsupport::AsyncJoinHandle;

type PreExecFunc = dyn FnMut() -> io::Result<()> + Send + Sync + 'static;

#[derive(Clone)]
pub struct ProcCommon {
    pub vars: HashMap<OsString, OsString>,
    #[cfg(unix)]
    pub uid: Option<u32>,
    #[cfg(unix)]
    pub gid: Option<u32>,
    #[cfg(unix)]
    pub pre_exec: Option<Arc<Mutex<Box<PreExecFunc>>>>,
}

impl fmt::Debug for ProcCommon {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ProcCommon")
            .field("vars", &self.vars)
            .finish()
    }
}

impl Default for ProcCommon {
    fn default() -> ProcCommon {
        ProcCommon {
            vars: std::env::vars_os().collect(),
            #[cfg(unix)]
            uid: None,
            #[cfg(unix)]
            gid: None,
            #[cfg(unix)]
            pre_exec: None,
        }
    }
}

/// Process factory, which can be used in order to configure the properties
/// of a process being created.
///
/// Methods can be chained on it in order to configure it.
#[derive(Debug, Default)]
pub struct Builder {
    stdin: Option<Stdio>,
    stdout: Option<Stdio>,
    stderr: Option<Stdio>,
    common: ProcCommon,
}

macro_rules! define_common_methods {
    () => {
        /// Set an environment variable in the spawned process.
        ///
        /// Equivalent to `Command::env`
        pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Self
        where
            K: AsRef<OsStr>,
            V: AsRef<OsStr>,
        {
            self.common.vars
                .insert(key.as_ref().to_owned(), val.as_ref().to_owned());
            self
        }

        /// Set environment variables in the spawned process.
        ///
        /// Equivalent to `Command::envs`
        pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
        where
            I: IntoIterator<Item = (K, V)>,
            K: AsRef<OsStr>,
            V: AsRef<OsStr>,
        {
            self.common.vars.extend(
                vars.into_iter()
                    .map(|(k, v)| (k.as_ref().to_owned(), v.as_ref().to_owned())),
            );
            self
        }

        /// Removes an environment variable in the spawned process.
        ///
        /// Equivalent to `Command::env_remove`
        pub fn env_remove<K: AsRef<OsStr>>(&mut self, key: K) -> &mut Self {
            self.common.vars.remove(key.as_ref());
            self
        }

        /// Clears all environment variables in the spawned process.
        ///
        /// Equivalent to `Command::env_clear`
        pub fn env_clear(&mut self) -> &mut Self {
            self.common.vars.clear();
            self
        }

        /// Sets the child process's user ID. This translates to a
        /// `setuid` call in the child process. Failure in the `setuid`
        /// call will cause the spawn to fail.
        ///
        /// Unix-specific extension only available on unix.
        ///
        /// Equivalent to `std::os::unix::process::CommandExt::uid`
        #[cfg(unix)]
        pub fn uid(&mut self, id: u32) -> &mut Self {
            self.common.uid = Some(id);
            self
        }

        /// Similar to `uid`, but sets the group ID of the child process. This has
        /// the same semantics as the `uid` field.
        ///
        /// Unix-specific extension only available on unix.
        ///
        /// Equivalent to `std::os::unix::process::CommandExt::gid`
        #[cfg(unix)]
        pub fn gid(&mut self, id: u32) -> &mut Self {
            self.common.gid = Some(id);
            self
        }

        /// Schedules a closure to be run just before the `exec` function is
        /// invoked.
        ///
        /// # Safety
        ///
        /// This method is inherently unsafe.  See the notes of the unix command
        /// ext for more information.
        ///
        /// Equivalent to `std::os::unix::process::CommandExt::pre_exec`
        #[cfg(unix)]
        pub unsafe fn pre_exec<F>(&mut self, f: F) -> &mut Self
        where
            F: FnMut() -> io::Result<()> + Send + Sync + 'static
        {
            self.common.pre_exec = Some(Arc::new(Mutex::new(Box::new(f))));
            self
        }
    }
}

impl Builder {
    /// Generates the base configuration for spawning a thread, from which
    /// configuration methods can be chained.
    pub fn new() -> Self {
        Self {
            stdin: None,
            stdout: None,
            stderr: None,
            common: ProcCommon::default(),
        }
    }

    pub(crate) fn common(&mut self, common: ProcCommon) -> &mut Self {
        self.common = common;
        self
    }

    define_common_methods!();

    /// Captures the `stdin` of the spawned process, allowing you to manually
    /// send data via `JoinHandle::stdin`
    ///
    /// For async spawns sending data is currently not supported.
    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.stdin = Some(cfg.into());
        self
    }

    /// Captures the `stdout` of the spawned process, allowing you to manually
    /// receive data via `JoinHandle::stdout`
    ///
    /// For async spawns retrieving the data is currently not supported.
    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.stdout = Some(cfg.into());
        self
    }

    /// Captures the `stderr` of the spawned process, allowing you to manually
    /// receive data via `JoinHandle::stderr`
    ///
    /// For async spawns retrieving the data is currently not supported.
    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.stderr = Some(cfg.into());
        self
    }

    /// Spawns the process.
    pub fn spawn<A: Serialize + DeserializeOwned, R: Serialize + DeserializeOwned>(
        &mut self,
        args: A,
        func: fn(A) -> R,
    ) -> JoinHandle<R> {
        assert_spawn_okay();
        JoinHandle {
            inner: mem::replace(self, Default::default())
                .spawn_helper(args, func)
                .map(JoinHandleInner::Process),
        }
    }

    /// Spawns the process (async).
    ///
    /// This is the async equivalent of [`spawn`](#method.spawn).
    #[cfg(feature = "async")]
    pub fn spawn_async<A: Serialize + DeserializeOwned, R: Serialize + DeserializeOwned>(
        &mut self,
        args: A,
        func: fn(A) -> R,
    ) -> AsyncJoinHandle<R> {
        assert_spawn_okay();
        match mem::replace(self, Default::default()).spawn_helper(args, func) {
            Ok(handle) => AsyncJoinHandle::from_process_handle(handle),
            Err(err) => AsyncJoinHandle::<R>::from_error(err),
        }
    }

    fn spawn_helper<A: Serialize + DeserializeOwned, R: Serialize + DeserializeOwned>(
        self,
        args: A,
        func: fn(A) -> R,
    ) -> Result<ProcessHandle<R>, SpawnError> {
        let (server, token) = IpcOneShotServer::<IpcSender<MarshalledCall>>::new()?;
        let me = if cfg!(target_os = "linux") {
            // will work even if exe is moved
            let path: PathBuf = "/proc/self/exe".into();
            if path.is_file() {
                path
            } else {
                // might not exist, e.g. on chroot
                env::current_exe()?
            }
        } else {
            env::current_exe()?
        };
        let mut child = process::Command::new(me);
        child.envs(self.common.vars.into_iter());
        child.env(ENV_NAME, token);

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            if let Some(id) = self.common.uid {
                child.uid(id);
            }
            if let Some(id) = self.common.gid {
                child.gid(id);
            }
            if let Some(ref func) = self.common.pre_exec {
                let func = func.clone();
                unsafe {
                    child.pre_exec(move || (&mut *func.lock().unwrap())());
                }
            }
        }

        let (can_pass_args, should_silence_stdout) = {
            #[cfg(feature = "test-support")]
            {
                match crate::testsupport::update_command_for_tests(&mut child) {
                    None => (true, false),
                    Some(crate::testsupport::TestMode {
                        can_pass_args,
                        should_silence_stdout,
                    }) => (can_pass_args, should_silence_stdout),
                }
            }
            #[cfg(not(feature = "test-support"))]
            {
                (true, false)
            }
        };

        if can_pass_args && should_pass_args() {
            child.args(env::args_os().skip(1));
        }

        if let Some(stdin) = self.stdin {
            child.stdin(stdin);
        }
        if let Some(stdout) = self.stdout {
            child.stdout(stdout);
        } else if should_silence_stdout {
            child.stdout(Stdio::null());
        }
        if let Some(stderr) = self.stderr {
            child.stderr(stderr);
        }
        let process = child.spawn()?;

        let (_rx, tx) = server.accept()?;

        let (args_tx, args_rx) = ipc::channel()?;
        let (return_tx, return_rx) = ipc::channel::<Result<R, PanicInfo>>()?;
        args_tx.send(args)?;

        tx.send(MarshalledCall::marshal::<A, R>(func, args_rx, return_tx))?;
        Ok(ProcessHandle {
            recv: return_rx,
            state: Arc::new(ProcessHandleState::new(Some(process.id()))),
            process,
        })
    }
}

#[derive(Debug)]
pub struct ProcessHandleState {
    pub exited: AtomicBool,
    pub pid: AtomicUsize,
}

impl ProcessHandleState {
    pub fn new(pid: Option<u32>) -> ProcessHandleState {
        ProcessHandleState {
            exited: AtomicBool::new(false),
            pid: AtomicUsize::new(pid.unwrap_or(0) as usize),
        }
    }

    pub fn pid(&self) -> Option<u32> {
        match self.pid.load(Ordering::SeqCst) {
            0 => None,
            x => Some(x as u32),
        }
    }

    pub fn kill(&self) {
        if !self.exited.load(Ordering::SeqCst) {
            self.exited.store(true, Ordering::SeqCst);
            if let Some(pid) = self.pid() {
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            }
        }
    }
}

pub struct ProcessHandle<T> {
    pub(crate) recv: IpcReceiver<Result<T, PanicInfo>>,
    pub(crate) process: process::Child,
    pub(crate) state: Arc<ProcessHandleState>,
}

fn is_ipc_timeout(err: &ipc_channel::Error) -> bool {
    if let ipc_channel::ErrorKind::Io(ref io) = &**err {
        match io.kind() {
            io::ErrorKind::TimedOut => true,
            io::ErrorKind::WouldBlock => true,
            _ => false,
        }
    } else {
        false
    }
}

impl<T> ProcessHandle<T> {
    pub fn state(&self) -> Arc<ProcessHandleState> {
        self.state.clone()
    }

    pub fn kill(&mut self) -> Result<(), SpawnError> {
        if self.state.exited.load(Ordering::SeqCst) {
            return Ok(());
        }
        let rv = self.process.kill().map_err(Into::into);
        self.process.wait().ok();
        self.state.exited.store(true, Ordering::SeqCst);
        rv
    }

    pub fn stdin(&mut self) -> Option<&mut ChildStdin> {
        self.process.stdin.as_mut()
    }

    pub fn stdout(&mut self) -> Option<&mut ChildStdout> {
        self.process.stdout.as_mut()
    }

    pub fn stderr(&mut self) -> Option<&mut ChildStderr> {
        self.process.stderr.as_mut()
    }
}

impl<T: Serialize + DeserializeOwned> ProcessHandle<T> {
    pub fn join(&mut self) -> Result<T, SpawnError> {
        let rv = self.recv.recv()?.map_err(Into::into);
        self.state.exited.store(true, Ordering::SeqCst);
        rv
    }

    pub fn join_timeout(&mut self, timeout: Duration) -> Result<T, SpawnError> {
        let deadline = match Instant::now().checked_add(timeout) {
            Some(deadline) => deadline,
            None => {
                return Err(io::Error::new(io::ErrorKind::Other, "timeout out of bounds").into())
            }
        };
        let mut to_sleep = Duration::from_millis(1);
        let rv = loop {
            match self.recv.try_recv() {
                Ok(rv) => break rv.map_err(Into::into),
                Err(err) if is_ipc_timeout(&err) => {
                    if let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
                        thread::sleep(remaining.min(to_sleep));
                        to_sleep *= 2;
                    } else {
                        return Err(SpawnError::new_timeout());
                    }
                }
                Err(err) => return Err(err.into()),
            }
        };
        self.state.exited.store(true, Ordering::SeqCst);
        rv
    }
}

pub enum JoinHandleInner<T> {
    Process(ProcessHandle<T>),
    Pooled(PooledHandle<T>),
}

/// An owned permission to join on a process (block on its termination).
///
/// The join handle can be used to join a process but also provides the
/// ability to kill it.
pub struct JoinHandle<T> {
    pub(crate) inner: Result<JoinHandleInner<T>, SpawnError>,
}

impl<T> fmt::Debug for JoinHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("JoinHandle")
            .field("pid", &self.pid())
            .finish()
    }
}

impl<T> JoinHandle<T> {
    pub(crate) fn process_handle_state(&self) -> Option<Arc<ProcessHandleState>> {
        match self.inner {
            Ok(JoinHandleInner::Process(ref handle)) => Some(handle.state()),
            Ok(JoinHandleInner::Pooled(ref handle)) => handle.process_handle_state(),
            Err(..) => None,
        }
    }

    /// Returns the process ID if available.
    ///
    /// The process ID is unavailable when pooled calls are not scheduled to
    /// processes.
    pub fn pid(&self) -> Option<u32> {
        self.process_handle_state().and_then(|x| x.pid())
    }

    /// Kill the child process.
    ///
    /// If the join handle was created from a pool this call will do one of
    /// two things depending on the situation:
    ///
    /// * if the call was already picked up by the process, the process will
    ///   be killed.
    /// * if the call was not yet scheduled to a process it will be cancelled.
    pub fn kill(&mut self) -> Result<(), SpawnError> {
        match self.inner {
            Ok(JoinHandleInner::Process(ref mut handle)) => handle.kill(),
            Ok(JoinHandleInner::Pooled(ref mut handle)) => handle.kill(),
            Err(_) => Ok(()),
        }
    }

    /// Fetch the `stdin` handle if it has been captured
    pub fn stdin(&mut self) -> Option<&mut ChildStdin> {
        match self.inner {
            Ok(JoinHandleInner::Process(ref mut process)) => process.stdin(),
            Ok(JoinHandleInner::Pooled(..)) => None,
            Err(_) => None,
        }
    }

    /// Fetch the `stdout` handle if it has been captured
    pub fn stdout(&mut self) -> Option<&mut ChildStdout> {
        match self.inner {
            Ok(JoinHandleInner::Process(ref mut process)) => process.stdout(),
            Ok(JoinHandleInner::Pooled(..)) => None,
            Err(_) => None,
        }
    }

    /// Fetch the `stderr` handle if it has been captured
    pub fn stderr(&mut self) -> Option<&mut ChildStderr> {
        match self.inner {
            Ok(JoinHandleInner::Process(ref mut process)) => process.stderr(),
            Ok(JoinHandleInner::Pooled(..)) => None,
            Err(_) => None,
        }
    }
}

impl<T: Serialize + DeserializeOwned> JoinHandle<T> {
    /// Wait for the child process to return a result.
    ///
    /// If the join handle was created from a pool the join is virtualized.
    pub fn join(self) -> Result<T, SpawnError> {
        match self.inner {
            Ok(JoinHandleInner::Process(mut handle)) => handle.join(),
            Ok(JoinHandleInner::Pooled(mut handle)) => handle.join(),
            Err(err) => Err(err),
        }
    }

    /// Like `join` but with a timeout.
    pub fn join_timeout(self, timeout: Duration) -> Result<T, SpawnError> {
        match self.inner {
            Ok(JoinHandleInner::Process(mut handle)) => handle.join_timeout(timeout),
            Ok(JoinHandleInner::Pooled(mut handle)) => handle.join_timeout(timeout),
            Err(err) => Err(err),
        }
    }
}

/// Spawn a new process to run a function with some payload.
///
/// ```rust,no_run
/// // call this early in your main() function.  This is where all spawned
/// // functions will be invoked.
/// procspawn::init();
///
/// let data = vec![1, 2, 3, 4];
/// let handle = procspawn::spawn(data, |data| {
///     println!("Received data {:?}", &data);
///     data.into_iter().sum::<i64>()
/// });
/// let result = handle.join().unwrap();
/// ```
pub fn spawn<A: Serialize + DeserializeOwned, R: Serialize + DeserializeOwned>(
    args: A,
    f: fn(A) -> R,
) -> JoinHandle<R> {
    Builder::new().spawn(args, f)
}
