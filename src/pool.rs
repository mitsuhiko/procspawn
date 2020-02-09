use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::process;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use ipc_channel::ipc;
use serde::{Deserialize, Serialize};

use crate::core::MarshalledCall;
use crate::error::SpawnError;
use crate::proc::{Builder, JoinHandle, JoinHandleInner, ProcCommon, ProcessHandleState};

type WaitFunc = Box<dyn FnOnce() -> bool + Send>;
type NotifyErrorFunc = Box<dyn FnMut(SpawnError) + Send>;

#[derive(Debug)]
pub struct PooledHandleState {
    pub cancelled: AtomicBool,
    pub process_handle_state: Mutex<Option<Arc<ProcessHandleState>>>,
}

impl PooledHandleState {
    pub fn kill(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Some(ref process_handle_state) = *self.process_handle_state.lock().unwrap() {
            process_handle_state.kill();
        }
    }
}

pub struct PooledHandle<T> {
    waiter_rx: mpsc::Receiver<Result<T, SpawnError>>,
    shared: Arc<PooledHandleState>,
}

impl<T> PooledHandle<T> {
    pub fn process_handle_state(&self) -> Option<Arc<ProcessHandleState>> {
        self.shared.process_handle_state.lock().unwrap().clone()
    }

    pub fn kill(&mut self) -> Result<(), SpawnError> {
        self.shared.kill();
        Ok(())
    }
}

impl<T: Serialize + for<'de> Deserialize<'de>> PooledHandle<T> {
    pub fn join(&mut self) -> Result<T, SpawnError> {
        match self.waiter_rx.recv() {
            Ok(Ok(rv)) => Ok(rv),
            Ok(Err(err)) => Err(err),
            Err(..) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "process went away").into()),
        }
    }

    pub fn join_timeout(&mut self, timeout: Duration) -> Result<T, SpawnError> {
        match self.waiter_rx.recv_timeout(timeout) {
            Ok(Ok(rv)) => Ok(rv),
            Ok(Err(err)) => Err(err),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.kill().ok();
                Err(SpawnError::new_timeout())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "process went away").into())
            }
        }
    }
}

/// A process pool.
///
/// This works similar to `spawn` but lets you retain a pool of processes. Since
/// procspawn is intended to isolate potentially crashing code the pool will
/// automatically restart broken processes.
///
/// Note that it's not possible to intercept streams of processes spawned
/// through the pool.
///
/// When the process pool is dropped all processes are killed.
///
/// This requires the `pool` feature.
pub struct Pool {
    sender: mpsc::Sender<(
        MarshalledCall,
        Arc<PooledHandleState>,
        WaitFunc,
        NotifyErrorFunc,
    )>,
    shared: Arc<PoolShared>,
}

impl fmt::Debug for Pool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Pool")
            .field("size", &self.size())
            .field("active_count", &self.active_count())
            .field("queued_count", &self.queued_count())
            .finish()
    }
}

impl Pool {
    /// Creates the default pool.
    pub fn new(size: usize) -> Result<Pool, SpawnError> {
        Pool::builder(size).build()
    }

    /// Creates a builder to customize pool creation.
    pub fn builder(size: usize) -> PoolBuilder {
        PoolBuilder::new(size)
    }

    /// Returns the size of the pool.
    pub fn size(&self) -> usize {
        self.shared.monitors.lock().unwrap().len()
    }

    /// Returns the number of jobs waiting to executed in the pool.
    pub fn queued_count(&self) -> usize {
        self.shared.queued_count.load(Ordering::Relaxed)
    }

    /// Returns the number of currently active threads.
    pub fn active_count(&self) -> usize {
        self.shared.active_count.load(Ordering::SeqCst)
    }

    /// Spawns a closure into a process of the pool.
    pub fn spawn<
        F: FnOnce(A) -> R + Copy,
        A: Serialize + for<'de> Deserialize<'de>,
        R: Serialize + for<'de> Deserialize<'de> + Send + 'static,
    >(
        &self,
        args: A,
        func: F,
    ) -> JoinHandle<R> {
        self.assert_alive();
        let _func = func;
        let (args_tx, args_rx) = ipc::channel().unwrap();
        let (return_tx, return_rx) = ipc::channel().unwrap();
        let call = MarshalledCall::marshal::<F, A, R>(args_rx, return_tx);
        args_tx.send(args).unwrap();
        let (waiter_tx, waiter_rx) = mpsc::sync_channel(0);
        let error_waiter_tx = waiter_tx.clone();
        self.shared.queued_count.fetch_add(1, Ordering::SeqCst);

        let shared = Arc::new(PooledHandleState {
            cancelled: AtomicBool::new(false),
            process_handle_state: Mutex::new(None),
        });

        self.sender
            .send((
                call,
                shared.clone(),
                Box::new(move || {
                    if let Ok(rv) = return_rx.recv() {
                        waiter_tx.send(rv.map_err(Into::into)).is_ok()
                    } else {
                        false
                    }
                }),
                Box::new(move |error| {
                    error_waiter_tx.send(Err(error)).ok();
                }),
            ))
            .ok();

        JoinHandle {
            inner: Ok(JoinHandleInner::Pooled(PooledHandle { waiter_rx, shared })),
        }
    }

    /// Joins the process pool.
    pub fn join(&self) {
        self.assert_alive();

        // fast path requires no mutex
        if !self.shared.has_work() {
            return;
        }

        let generation = self.shared.join_generation.load(Ordering::SeqCst);
        let mut lock = self.shared.empty_trigger.lock().unwrap();

        while generation == self.shared.join_generation.load(Ordering::Relaxed)
            && self.shared.has_work()
        {
            lock = self.shared.empty_condvar.wait(lock).unwrap();
        }

        // increase generation if we are the first thread to come out of the loop
        self.shared.join_generation.compare_and_swap(
            generation,
            generation.wrapping_add(1),
            Ordering::SeqCst,
        );
    }

    /// Joins and shuts down.
    pub fn shutdown(&self) {
        if !self.shared.dead.load(Ordering::SeqCst) {
            self.join();
            self.kill();
        }
    }

    /// Hard kills all processes in the pool.
    ///
    /// After calling this the pool cannot be used any more.
    pub fn kill(&self) {
        if self.shared.dead.load(Ordering::SeqCst) {
            return;
        }
        self.shared.dead.store(true, Ordering::SeqCst);
        for monitor in self.shared.monitors.lock().unwrap().iter_mut() {
            if let Some(mut join_handle) = monitor.join_handle.lock().unwrap().take() {
                join_handle.kill().ok();
            }
        }
    }

    fn assert_alive(&self) {
        if self.shared.dead.load(Ordering::SeqCst) {
            panic!("The process pool is dead");
        }
    }
}

/// Utility to configure a pool.
///
/// This requires the `pool` feature.
#[derive(Debug)]
pub struct PoolBuilder {
    size: usize,
    disable_stdin: bool,
    disable_stdout: bool,
    disable_stderr: bool,
    common: ProcCommon,
}

impl PoolBuilder {
    /// Create a new pool builder.
    ///
    /// The size of the process pool is mandatory.  If you want to
    /// make it depending on the processor count you can use the
    /// `num_cpus` crate.
    fn new(size: usize) -> PoolBuilder {
        PoolBuilder {
            size,
            disable_stdin: false,
            disable_stdout: false,
            disable_stderr: false,
            common: ProcCommon::default(),
        }
    }

    define_common_methods!();

    /// Redirects stdin to `/dev/null`.
    pub fn disable_stdin(&mut self) -> &mut Self {
        self.disable_stdin = true;
        self
    }

    /// Redirects stdout to `/dev/null`.
    pub fn disable_stdout(&mut self) -> &mut Self {
        self.disable_stdout = true;
        self
    }

    /// Redirects stderr to `/dev/null`.
    pub fn disable_stderr(&mut self) -> &mut Self {
        self.disable_stderr = true;
        self
    }

    /// Creates the pool.
    pub fn build(&mut self) -> Result<Pool, SpawnError> {
        let (tx, rx) = mpsc::channel();

        let shared = Arc::new(PoolShared {
            call_receiver: Mutex::new(rx),
            empty_trigger: Mutex::new(()),
            empty_condvar: Condvar::new(),
            join_generation: AtomicUsize::new(0),
            monitors: Mutex::new(Vec::with_capacity(self.size)),
            queued_count: AtomicUsize::new(0),
            active_count: AtomicUsize::new(0),
            dead: AtomicBool::new(false),
        });

        {
            let mut monitors = shared.monitors.lock().unwrap();
            for _ in 0..self.size {
                monitors.push(spawn_worker(shared.clone(), &self)?);
            }
        }

        Ok(Pool { sender: tx, shared })
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.kill();
    }
}

struct PoolShared {
    #[allow(clippy::type_complexity)]
    call_receiver: Mutex<
        mpsc::Receiver<(
            MarshalledCall,
            Arc<PooledHandleState>,
            WaitFunc,
            NotifyErrorFunc,
        )>,
    >,
    empty_trigger: Mutex<()>,
    empty_condvar: Condvar,
    join_generation: AtomicUsize,
    monitors: Mutex<Vec<WorkerMonitor>>,
    queued_count: AtomicUsize,
    active_count: AtomicUsize,
    dead: AtomicBool,
}

impl PoolShared {
    fn has_work(&self) -> bool {
        self.queued_count.load(Ordering::SeqCst) > 0 || self.active_count.load(Ordering::SeqCst) > 0
    }

    fn no_work_notify_all(&self) {
        if !self.has_work() {
            drop(
                self.empty_trigger
                    .lock()
                    .expect("Unable to notify all joining threads"),
            );
            self.empty_condvar.notify_all();
        }
    }
}

struct WorkerMonitor {
    join_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

fn spawn_worker(
    shared: Arc<PoolShared>,
    builder: &PoolBuilder,
) -> Result<WorkerMonitor, SpawnError> {
    let join_handle = Arc::new(Mutex::new(None::<JoinHandle<()>>));
    let current_call_tx = Arc::new(Mutex::new(None::<ipc::IpcSender<MarshalledCall>>));

    let spawn = Arc::new(Mutex::new({
        let disable_stdin = builder.disable_stdin;
        let disable_stdout = builder.disable_stdout;
        let disable_stderr = builder.disable_stderr;
        let common = builder.common.clone();
        let join_handle = join_handle.clone();
        let current_call_tx = current_call_tx.clone();
        move || {
            let (call_tx, call_rx) = ipc::channel::<MarshalledCall>().unwrap();
            let mut builder = Builder::new();
            builder.common(common.clone());
            if disable_stdin {
                builder.stdin(process::Stdio::null());
            }
            if disable_stdout {
                builder.stdout(process::Stdio::null());
            }
            if disable_stderr {
                builder.stderr(process::Stdio::null());
            }
            *join_handle.lock().unwrap() = Some(builder.spawn(call_rx, |rx| {
                while let Ok(call) = rx.recv() {
                    // we never want panic handling here as we're going to
                    // defer this to the process'.
                    call.call(false);
                }
            }));
            *current_call_tx.lock().unwrap() = Some(call_tx);
        }
    }));

    let check_for_restart = {
        let spawn = spawn.clone();
        let join_handle = join_handle.clone();
        let shared = shared.clone();
        move |f: &mut NotifyErrorFunc| {
            // something went wrong so we're expecting the join handle to
            // indicate an error.
            if let Some(join_handle) = join_handle.lock().unwrap().take() {
                match join_handle.join() {
                    Ok(()) => f(SpawnError::from(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "client process died",
                    ))),
                    Err(err) => f(err),
                }
            }

            // next step is respawning the client.
            if !shared.dead.load(Ordering::SeqCst) {
                (*spawn.lock().unwrap())();
            }
        }
    };

    // for each worker we spawn a monitoring thread
    {
        let join_handle = join_handle.clone();
        thread::spawn(move || {
            loop {
                if shared.dead.load(Ordering::SeqCst) {
                    break;
                }

                let (call, state, wait_func, mut err_func) = {
                    // Only lock jobs for the time it takes
                    // to get a job, not run it.
                    let lock = shared
                        .call_receiver
                        .lock()
                        .expect("Monitor thread unable to lock call receiver");
                    match lock.recv() {
                        Ok(rv) => rv,
                        Err(_) => break,
                    }
                };

                shared.active_count.fetch_add(1, Ordering::SeqCst);
                shared.queued_count.fetch_sub(1, Ordering::SeqCst);

                // this task was already cancelled, no need to execute it
                if state.cancelled.load(Ordering::SeqCst) {
                    err_func(SpawnError::new_cancelled());
                } else {
                    if let Some(ref mut handle) = *join_handle.lock().unwrap() {
                        *state.process_handle_state.lock().unwrap() = handle.process_handle_state();
                    }

                    let mut restart = false;
                    {
                        let mut call_tx = current_call_tx.lock().unwrap();
                        if let Some(ref mut call_tx) = *call_tx {
                            match call_tx.send(call) {
                                Ok(()) => {}
                                Err(..) => {
                                    restart = true;
                                }
                            }
                        } else {
                            restart = true;
                        }
                    }

                    if !restart && !wait_func() {
                        restart = true;
                    }

                    *state.process_handle_state.lock().unwrap() = None;

                    if restart {
                        check_for_restart(&mut err_func);
                    }
                }

                shared.active_count.fetch_sub(1, Ordering::SeqCst);
                shared.no_work_notify_all();
            }
        });
    }

    (*spawn.lock().unwrap())();

    Ok(WorkerMonitor { join_handle })
}
