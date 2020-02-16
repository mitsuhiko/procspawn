use std::fmt;
use std::process;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use futures::channel::oneshot;
use ipc_channel::ipc;
use ipc_channel::router::ROUTER;
use serde::{de::DeserializeOwned, Serialize};

use crate::error::{PanicInfo, SpawnError};
use crate::proc::{Builder, ProcessHandle, ProcessHandleState};

pub struct AsyncProcessHandle<T> {
    recv: oneshot::Receiver<ipc::OpaqueIpcMessage>,
    process: process::Child,
    state: Arc<ProcessHandleState>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> AsyncProcessHandle<T> {
    pub fn state(&self) -> Arc<ProcessHandleState> {
        self.state.clone()
    }

    pub fn kill(&mut self) -> Result<(), SpawnError> {
        if self.state.exited.load(Ordering::SeqCst) {
            return Ok(());
        }
        let rv = self.process.kill().map_err(Into::into);
        // this should be instant since we just killed the process.
        self.process.wait().ok();
        self.state.exited.store(true, Ordering::SeqCst);
        rv
    }
}

impl<T: Serialize + DeserializeOwned> AsyncProcessHandle<T> {
    pub async fn join(&mut self) -> Result<T, SpawnError> {
        let rv = match (&mut self.recv).await {
            Ok(msg) => msg
                .to::<Result<T, PanicInfo>>()
                .expect("return channel format mismatch")
                .map_err(|x| x.into()),
            Err(_) => Err(SpawnError::new_cancelled()),
        };
        self.state.exited.store(true, Ordering::SeqCst);
        rv
    }
}

impl<T> Drop for AsyncProcessHandle<T> {
    fn drop(&mut self) {
        self.kill().ok();
    }
}

pub enum AsyncJoinHandleInner<T> {
    Process(AsyncProcessHandle<T>),
}

/// An owned permission to join on a process (block on its termination).
///
/// The join handle can be used to join a process but also provides the
/// ability to kill it.
///
/// Unlike a normal [`JoinHandle`](struct.JoinHandle.html) dropping an async
/// handle will kill the process.  It *must* be awaited.
///
/// This requires the `async` feature.
pub struct AsyncJoinHandle<T> {
    pub(crate) inner: Result<AsyncJoinHandleInner<T>, SpawnError>,
}

impl<T> fmt::Debug for AsyncJoinHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AsyncJoinHandle")
            .field("pid", &self.pid())
            .finish()
    }
}

impl<T> AsyncJoinHandle<T> {
    pub(crate) fn from_error(err: SpawnError) -> AsyncJoinHandle<T> {
        AsyncJoinHandle { inner: Err(err) }
    }

    pub(crate) fn process_handle_state(&self) -> Option<Arc<ProcessHandleState>> {
        match self.inner {
            Ok(AsyncJoinHandleInner::Process(ref handle)) => Some(handle.state()),
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
            Ok(AsyncJoinHandleInner::Process(ref mut handle)) => handle.kill(),
            Err(_) => Ok(()),
        }
    }
}

impl<T: Serialize + DeserializeOwned> AsyncJoinHandle<T> {
    pub(crate) fn from_process_handle(handle: ProcessHandle<T>) -> AsyncJoinHandle<T> {
        let ProcessHandle {
            recv,
            state,
            process,
        } = handle;
        let (tx, rx) = oneshot::channel();
        let mut tx = Some(tx);
        ROUTER.add_route(
            recv.to_opaque(),
            Box::new(move |msg| {
                if let Some(tx) = tx.take() {
                    tx.send(msg).ok();
                }
            }),
        );
        AsyncJoinHandle {
            inner: Ok(AsyncJoinHandleInner::Process(AsyncProcessHandle {
                recv: rx,
                state,
                process,
                _marker: Default::default(),
            })),
        }
    }

    /// Joins the handle and returns the result.
    ///
    /// Note that unlike with a sync spawn there is no separate API to join
    /// with a timeout.  Use your executor's timeout functionality for this.
    /// Since dropping the join handle will in any case terminate the process
    /// this will have the same effect.
    pub async fn join_async(mut self) -> Result<T, SpawnError> {
        match self.inner {
            Ok(AsyncJoinHandleInner::Process(ref mut handle)) => handle.join().await,
            Err(err) => Err(err),
        }
    }
}

/// Spawn a new process to run a function with some payload (async).
///
/// This is the async equivalent of [`spawn`](fn.spawn.html).
///
/// This requires the `async` feature.
pub fn spawn_async<A: Serialize + DeserializeOwned, R: Serialize + DeserializeOwned>(
    args: A,
    f: fn(A) -> R,
) -> AsyncJoinHandle<R> {
    Builder::new().spawn_async(args, f)
}
