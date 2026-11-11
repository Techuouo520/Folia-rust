// One QuickJS runtime and context per worker; all calls have an interruptible wall-clock deadline.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant};

use rquickjs::{Context, Ctx, Function, Object, Persistent, Promise, Runtime};
use serde_json::{Value, json};

use super::{Limits, SourceError, SourceMetadata, SourcePlatform, UpdateAlert, bridge::Network, http};

struct Call {
    data: Value,
    reply: tokio::sync::oneshot::Sender<Result<String, SourceError>>,
    expires: Instant,
}

struct Worker {
    sender: mpsc::SyncSender<Call>,
    stopped: Arc<AtomicBool>,
    alerts: Arc<Mutex<Vec<UpdateAlert>>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

/// Cloneable channel to an isolated script. Dropping the last handle interrupts and releases its worker.
#[derive(Clone)]
pub struct Script {
    worker: Arc<Worker>,
    sources: BTreeMap<String, SourcePlatform>,
    timeout: Duration,
}

impl Script {
    /// Run on a background thread: this waits for the script's `inited` event.
    pub fn start(
        code: String,
        info: SourceMetadata,
        client: reqwest::Client,
        limits: Limits,
    ) -> Result<Self, SourceError> {
        if code.len() > super::SCRIPT_BYTES {
            return Err(SourceError::Limit);
        }
        let (sender, receiver) = mpsc::sync_channel(16);
        let (initialized, ready) = mpsc::sync_channel(1);
        let stopped = Arc::new(AtomicBool::new(false));
        let alerts = Arc::new(Mutex::new(Vec::new()));
        let worker = Arc::new(Worker { sender, stopped: stopped.clone(), alerts: alerts.clone() });
        std::thread::Builder::new().name("folia-lx".into()).spawn(move || {
            let mut engine = match Engine::new(&code, &info, client, limits, stopped, alerts) {
                Ok(engine) => engine,
                Err(err) => {
                    let _ = initialized.send(Err(err));
                    return;
                }
            };
            match engine.initialize(&code) {
                Ok(sources) => {
                    if initialized.send(Ok(sources)).is_err() {
                        return;
                    }
                }
                Err(err) => {
                    let _ = initialized.send(Err(err));
                    return;
                }
            }
            while !engine.stopped.load(Ordering::Relaxed) {
                match receiver.recv_timeout(Duration::from_millis(50)) {
                    Ok(Call { data, reply, expires }) => {
                        if reply.is_closed() {
                            continue;
                        }
                        engine.set_deadline(expires.saturating_duration_since(Instant::now()));
                        let result = engine.request(&data);
                        let fatal = matches!(result, Err(SourceError::Timeout | SourceError::Memory));
                        let _ = reply.send(result);
                        if fatal {
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Err(err) = engine.idle() {
                            tracing::warn!(
                                error = &err as &dyn std::error::Error,
                                "custom source background execution stopped"
                            );
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })?;
        let sources = ready.recv_timeout(limits.init_timeout + Duration::from_secs(1)).map_err(|err| match err {
            mpsc::RecvTimeoutError::Timeout => SourceError::Timeout,
            mpsc::RecvTimeoutError::Disconnected => SourceError::Stopped,
        })??;
        Ok(Self { worker, sources, timeout: limits.request_timeout })
    }

    pub fn sources(&self) -> &BTreeMap<String, SourcePlatform> {
        &self.sources
    }

    pub async fn music_url(&self, source: &str, quality: &str, music_info: Value) -> Result<String, SourceError> {
        let (reply, receiver) = tokio::sync::oneshot::channel();
        let data = json!({"source":source,"action":"musicUrl","info":{"type":quality,"musicInfo":music_info}});
        self.worker.sender.try_send(Call { data, reply, expires: Instant::now() + self.timeout }).map_err(|err| {
            match err {
                mpsc::TrySendError::Full(_) => SourceError::Limit,
                mpsc::TrySendError::Disconnected(_) => SourceError::Stopped,
            }
        })?;
        tokio::time::timeout(self.timeout + Duration::from_millis(250), receiver)
            .await
            .map_err(|_| SourceError::Timeout)?
            .map_err(|_| SourceError::Stopped)?
    }

    pub(crate) fn stop(&self) {
        self.worker.stopped.store(true, Ordering::Relaxed);
    }

    pub fn take_update_alerts(&self) -> Vec<UpdateAlert> {
        std::mem::take(&mut *self.worker.alerts.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

struct Engine {
    bridge: Persistent<Object<'static>>,
    context: Context,
    runtime: Runtime,
    network: Network,
    origin: Instant,
    deadline: Arc<AtomicU64>,
    stopped: Arc<AtomicBool>,
    rejected: Rc<Cell<usize>>,
    alerts: Arc<Mutex<Vec<UpdateAlert>>>,
}

impl Engine {
    fn idle(&mut self) -> Result<(), SourceError> {
        let deadline = self.deadline.load(Ordering::Relaxed);
        self.set_deadline(Duration::from_millis(25));
        let pending = self
            .context
            .with(|ctx| {
                let bridge = self.bridge.clone().restore(&ctx)?;
                bridge.get::<_, Function>("pending")?.call::<_, usize>(())
            })
            .map_err(|_| SourceError::Script);
        self.deadline.store(deadline, Ordering::Relaxed);
        let pending = pending?;
        if pending > 0 || self.runtime.is_job_pending() {
            self.pump()?;
        }
        Ok(())
    }
    fn new(
        code: &str,
        info: &SourceMetadata,
        client: reqwest::Client,
        limits: Limits,
        stopped: Arc<AtomicBool>,
        alerts: Arc<Mutex<Vec<UpdateAlert>>>,
    ) -> Result<Self, SourceError> {
        let runtime = Runtime::new().map_err(|_| SourceError::Memory)?;
        runtime.set_memory_limit(limits.memory_bytes);
        runtime.set_max_stack_size(512 * 1024);
        let origin = Instant::now();
        let deadline = Arc::new(AtomicU64::new(limits.init_timeout.as_millis() as u64));
        let (interrupt, stop) = (deadline.clone(), stopped.clone());
        runtime.set_interrupt_handler(Some(Box::new(move || {
            stop.load(Ordering::Relaxed) || origin.elapsed().as_millis() as u64 >= interrupt.load(Ordering::Relaxed)
        })));
        let rejected = Rc::new(Cell::new(0_usize));
        let unhandled = rejected.clone();
        runtime.set_host_promise_rejection_tracker(Some(Box::new(move |_, _, _, handled| {
            unhandled.set(if handled { unhandled.get().saturating_sub(1) } else { unhandled.get().saturating_add(1) });
        })));
        let context = Context::full(&runtime).map_err(|_| SourceError::Memory)?;
        let mut network = Network::new()?;
        let bridge = context.with(|ctx| {
            network
                .install(&ctx, client, info, code)
                .map(|bridge| Persistent::save(&ctx, bridge))
                .map_err(|err| js_error(&ctx, err))
        })?;
        Ok(Self { bridge, context, runtime, network, origin, deadline, stopped, rejected, alerts })
    }

    fn set_deadline(&self, duration: Duration) {
        self.deadline.store((self.origin.elapsed() + duration).as_millis() as u64, Ordering::Relaxed);
    }

    fn check(&self) -> Result<(), SourceError> {
        if self.stopped.load(Ordering::Relaxed) {
            return Err(SourceError::Stopped);
        }
        if self.origin.elapsed().as_millis() as u64 >= self.deadline.load(Ordering::Relaxed) {
            return Err(SourceError::Timeout);
        }
        Ok(())
    }

    fn initialize(&mut self, code: &str) -> Result<BTreeMap<String, SourcePlatform>, SourceError> {
        let result = self.context.with(|ctx| ctx.eval::<(), _>(code).map_err(|err| js_error(&ctx, err)));
        self.check()?;
        result?;
        loop {
            self.pump()?;
            if self.rejected.get() > 0 {
                return Err(SourceError::Script);
            }
            let result: Option<String> = self.call("initialized", ())?;
            if let Some(result) = result {
                return serde_json::from_str(&result).map_err(|_| SourceError::Protocol);
            }
        }
    }

    fn request(&mut self, data: &Value) -> Result<String, SourceError> {
        let promise = self.context.with(|ctx| {
            let result = (|| {
                let bridge = self.bridge.clone().restore(&ctx)?;
                let function: Function = bridge.get("request")?;
                let promise: Promise = function.call((data.to_string(),))?;
                Ok(Persistent::save(&ctx, promise))
            })();
            result.map_err(|err| js_error(&ctx, err))
        });
        self.check()?;
        let promise = promise?;
        loop {
            self.pump()?;
            let result = self.context.with(|ctx| {
                let promise = promise.clone().restore(&ctx).map_err(|err| js_error(&ctx, err))?;
                Ok::<_, SourceError>(promise.result::<String>().map(|result| result.map_err(|err| js_error(&ctx, err))))
            })?;
            if let Some(result) = result {
                let url = result?;
                if url.encode_utf16().count() > 2048 {
                    return Err(SourceError::Protocol);
                }
                http::validate_url(&url)?;
                return Ok(url);
            }
        }
    }

    fn call<T, A>(&self, name: &str, args: A) -> Result<T, SourceError>
    where
        T: for<'js> rquickjs::FromJs<'js>,
        A: for<'js> rquickjs::function::IntoArgs<'js>,
    {
        let result = self.context.with(|ctx| {
            let result = (|| {
                let bridge = self.bridge.clone().restore(&ctx)?;
                let function: Function = bridge.get(name)?;
                function.call(args)
            })();
            result.map_err(|err| js_error(&ctx, err))
        });
        self.check()?;
        result
    }

    fn pump(&mut self) -> Result<(), SourceError> {
        self.check()?;
        self.network.runtime.block_on(async { tokio::time::sleep(Duration::from_millis(2)).await });
        while let Ok(response) = self.network.responses.try_recv() {
            self.call::<(), _>("response", (response,))?;
        }
        self.call::<(), _>("tick", ())?;
        for _ in 0..256 {
            self.check()?;
            match self.runtime.execute_pending_job() {
                Ok(false) => break,
                Ok(true) => {}
                Err(err) => {
                    self.check()?;
                    return Err(err.0.with(|ctx| js_error(&ctx, rquickjs::Error::Exception)));
                }
            }
        }
        self.check()?;
        let update: String = self.call("update", ())?;
        if update != "null" {
            let update = serde_json::from_str(&update).map_err(|_| SourceError::Protocol)?;
            self.alerts.lock().unwrap_or_else(PoisonError::into_inner).push(update);
        }
        Ok(())
    }
}

fn js_error(ctx: &Ctx<'_>, error: rquickjs::Error) -> SourceError {
    if matches!(error, rquickjs::Error::Allocation) {
        return SourceError::Memory;
    }
    let value = ctx.catch();
    let message = value.as_exception().and_then(|e| e.message()).unwrap_or_default();
    if message.contains("out of memory") { SourceError::Memory } else { SourceError::Script }
}
