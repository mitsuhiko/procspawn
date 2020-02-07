use std::collections::HashMap;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::process::{ChildStderr, ChildStdin, ChildStdout};
use std::{env, mem, process};

use ipc_channel::ipc::{self, IpcOneShotServer, IpcReceiver, IpcSender};
use serde::{Deserialize, Serialize};

use crate::core::{assert_initialized, get_wrapper_offset, BootstrapData, ENV_NAME};
use crate::error::{Panic, SpawnError};

/// Process factory, which can be used in order to configure the properties
/// of a process being created.
///
/// Methods can be chained on it in order to configure it.
#[derive(Debug, Default)]
pub struct Builder {
    pub(crate) stdin: Option<Stdio>,
    pub(crate) stdout: Option<Stdio>,
    pub(crate) stderr: Option<Stdio>,
    pub(crate) vars: HashMap<OsString, OsString>,
}

impl Builder {
    /// Generates the base configuration for spawning a thread, from which
    /// configuration methods can be chained.
    pub fn new() -> Self {
        Self {
            stdin: None,
            stdout: None,
            stderr: None,
            vars: std::env::vars_os().collect(),
        }
    }

    /// Set an environment variable in the spawned process.
    ///
    /// Equivalent to `Command::env`
    pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.vars
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
        self.vars.extend(
            vars.into_iter()
                .map(|(k, v)| (k.as_ref().to_owned(), v.as_ref().to_owned())),
        );
        self
    }

    /// Removes an environment variable in the spawned process.
    ///
    /// Equivalent to `Command::env_remove`
    pub fn env_remove<K: AsRef<OsStr>>(&mut self, key: K) -> &mut Self {
        self.vars.remove(key.as_ref());
        self
    }

    /// Clears all environment variables in the spawned process.
    ///
    /// Equivalent to `Command::env_clear`
    pub fn env_clear(&mut self) -> &mut Self {
        self.vars.clear();
        self
    }

    /// Captures the `stdin` of the spawned process, allowing you to manually
    /// send data via `JoinHandle::stdin`
    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.stdin = Some(cfg.into());
        self
    }

    /// Captures the `stdout` of the spawned process, allowing you to manually
    /// receive data via `JoinHandle::stdout`
    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.stdout = Some(cfg.into());
        self
    }

    /// Captures the `stderr` of the spawned process, allowing you to manually
    /// receive data via `JoinHandle::stderr`
    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.stderr = Some(cfg.into());
        self
    }

    /// Spawns the process.
    pub fn spawn<
        F: FnOnce(A) -> R + Copy,
        A: Serialize + for<'de> Deserialize<'de>,
        R: Serialize + for<'de> Deserialize<'de>,
    >(
        self,
        args: A,
        func: F,
    ) -> JoinHandle<R> {
        assert_initialized();
        JoinHandle(self.spawn_helper(args, func))
    }

    fn spawn_helper<
        F: FnOnce(A) -> R + Copy,
        A: Serialize + for<'de> Deserialize<'de>,
        R: Serialize + for<'de> Deserialize<'de>,
    >(
        self,
        args: A,
        _: F,
    ) -> Result<JoinHandleInner<R>, SpawnError> {
        if mem::size_of::<F>() != 0 {
            panic!("procspawn cannot be called with closures that capture data!");
        }

        let (server, token) = IpcOneShotServer::<IpcSender<BootstrapData>>::new()?;
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
        child.envs(self.vars.into_iter());
        child.env(ENV_NAME, token);
        if let Some(stdin) = self.stdin {
            child.stdin(stdin);
        }
        if let Some(stdout) = self.stdout {
            child.stdout(stdout);
        }
        if let Some(stderr) = self.stderr {
            child.stderr(stderr);
        }
        let process = child.spawn()?;

        let (_rx, tx) = server.accept()?;

        let (args_tx, args_rx) = ipc::channel()?;
        let (return_tx, return_rx) = ipc::channel()?;
        args_tx.send(args)?;

        let bootstrap = BootstrapData {
            wrapper_offset: get_wrapper_offset::<F, A, R>(),
            args_receiver: args_rx.to_opaque(),
            return_sender: return_tx.to_opaque(),
        };
        tx.send(bootstrap)?;
        Ok(JoinHandleInner {
            recv: return_rx,
            process,
        })
    }
}

/// An owned permission to join on a process (block on its termination).
///
/// The join handle can be used to join a process but also provides the
/// ability to kill it.
pub struct JoinHandle<T>(Result<JoinHandleInner<T>, SpawnError>);

pub struct JoinHandleInner<T> {
    recv: IpcReceiver<Result<T, Panic>>,
    process: process::Child,
}

impl<T: Serialize + for<'de> Deserialize<'de>> JoinHandle<T> {
    /// Wait for the child process to return a result.
    pub fn join(self) -> Result<T, SpawnError> {
        match self.0 {
            Ok(inner) => {
                let rv: Result<T, Panic> = inner.recv.recv()?;
                match rv {
                    Ok(rv) => Ok(rv),
                    Err(panic) => Err(panic.into()),
                }
            }
            Err(err) => Err(err),
        }
    }

    /// Kill the child process.
    pub fn kill(self) -> std::io::Result<()> {
        match self.0 {
            Ok(mut inner) => inner.process.kill().map_err(Into::into),
            Err(_) => Ok(()),
        }
    }

    /// Fetch the `stdin` handle if it has been captured
    pub fn stdin(&mut self) -> Option<&mut ChildStdin> {
        match self.0 {
            Ok(ref mut inner) => inner.process.stdin.as_mut(),
            Err(_) => None,
        }
    }

    /// Fetch the `stdout` handle if it has been captured
    pub fn stdout(&mut self) -> Option<&mut ChildStdout> {
        match self.0 {
            Ok(ref mut inner) => inner.process.stdout.as_mut(),
            Err(_) => None,
        }
    }

    /// Fetch the `stderr` handle if it has been captured
    pub fn stderr(&mut self) -> Option<&mut ChildStderr> {
        match self.0 {
            Ok(ref mut inner) => inner.process.stderr.as_mut(),
            Err(_) => None,
        }
    }
}
