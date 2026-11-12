// Serial GPU initialization runs on a worker while each widget owns its completed resources.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak, mpsc};

type Job = Box<dyn FnOnce() + Send>;

#[cfg(feature = "dev")]
static PENDING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(feature = "dev")]
static STARTED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Pending jobs and total submissions, for headless render scheduling and initialization measurements.
#[cfg(feature = "dev")]
pub fn gpu_builds() -> (usize, usize) {
    (PENDING.load(Ordering::Acquire), STARTED.load(Ordering::Acquire))
}

struct Running;

impl Drop for Running {
    fn drop(&mut self) {
        #[cfg(feature = "dev")]
        PENDING.fetch_sub(1, Ordering::Release);
    }
}

fn enqueue(job: Job) -> Result<(), String> {
    static WORKER: OnceLock<Result<mpsc::Sender<Job>, String>> = OnceLock::new();
    let sender = WORKER
        .get_or_init(|| {
            let (sender, receiver) = mpsc::channel::<Job>();
            std::thread::Builder::new()
                .name("folia-gpu-init".into())
                .spawn(move || {
                    for job in receiver {
                        job();
                    }
                })
                .map_err(|err| err.to_string())?;
            Ok(sender)
        })
        .as_ref()
        .map_err(Clone::clone)?;
    sender.send(job).map_err(|err| err.to_string())
}

/// Pollable initialization. A dropped widget cancels queued work and releases any completed resources.
pub struct Prepared<T> {
    value: Option<T>,
    result: Arc<Mutex<Option<T>>>,
    finished: Arc<AtomicBool>,
    owner: Weak<AtomicBool>,
}

impl<T: Send + 'static> Prepared<T> {
    pub fn new(owner: Weak<AtomicBool>, make: impl FnOnce() -> T + Send + 'static) -> Self {
        let result = Arc::new(Mutex::new(None));
        let finished = Arc::new(AtomicBool::new(false));
        let task_result = result.clone();
        let task_finished = finished.clone();
        let task_owner = owner.clone();
        #[cfg(feature = "dev")]
        {
            PENDING.fetch_add(1, Ordering::Release);
            STARTED.fetch_add(1, Ordering::Release);
        }
        let running = Running;
        let job = Box::new(move || {
            let _running = running;
            if task_owner.strong_count() > 0 {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(make)) {
                    Ok(value) if task_owner.strong_count() > 0 => {
                        *task_result.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(value)
                    }
                    Ok(_) => {}
                    Err(_) => tracing::error!("GPU resource initialization failed"),
                }
            }
            task_finished.store(true, Ordering::Release);
        });
        if let Err(err) = enqueue(job) {
            tracing::error!(error = %err, "GPU initialization worker unavailable");
            finished.store(true, Ordering::Release);
        }
        Self { value: None, result, finished, owner }
    }

    pub fn poll(&mut self) -> Option<&mut T> {
        if self.finished.load(Ordering::Acquire) {
            if self.value.is_none() {
                self.value = self.result.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
            }
            if let Some(owner) = self.owner.upgrade() {
                owner.store(true, Ordering::Release);
            }
        }
        self.value.as_mut()
    }

    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.value.as_mut()
    }
}
