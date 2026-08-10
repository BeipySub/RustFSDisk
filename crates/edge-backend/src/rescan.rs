use crate::disk_detection::{
    BoxFuture, DiskDetectionError, DiskProbe, DiskRuntimeLedger, EdgeDiskDetector,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

pub trait DiskRescanRunner: Send + Sync + 'static {
    fn run_disk_rescan<'a>(
        &'a self,
        trigger: DiskRescanTrigger,
    ) -> BoxFuture<'a, Result<usize, DiskDetectionError>>;
}

impl<P, V, L> DiskRescanRunner for EdgeDiskDetector<P, V, L>
where
    P: DiskProbe + 'static,
    V: crate::disk_detection::CenterDiskVerifier + 'static,
    L: DiskRuntimeLedger + 'static,
{
    fn run_disk_rescan<'a>(
        &'a self,
        trigger: DiskRescanTrigger,
    ) -> BoxFuture<'a, Result<usize, DiskDetectionError>> {
        Box::pin(async move {
            let records = match trigger.source {
                DiskRescanSource::Startup => self.scan_existing_transport_disks().await?,
                DiskRescanSource::Udev | DiskRescanSource::Manual | DiskRescanSource::Queued => {
                    self.handle_udev_disk_change().await?
                }
            };
            Ok(records.len())
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskRescanSource {
    Startup,
    Udev,
    Manual,
    Queued,
}

#[derive(Debug, Clone)]
pub struct DiskRescanTrigger {
    pub source: DiskRescanSource,
    pub device: Option<String>,
}

impl DiskRescanTrigger {
    pub fn startup() -> Self {
        Self {
            source: DiskRescanSource::Startup,
            device: None,
        }
    }

    pub fn udev(device: Option<String>) -> Self {
        Self {
            source: DiskRescanSource::Udev,
            device,
        }
    }

    pub fn manual(device: Option<String>) -> Self {
        Self {
            source: DiskRescanSource::Manual,
            device,
        }
    }

    fn queued() -> Self {
        Self {
            source: DiskRescanSource::Queued,
            device: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskRescanAccepted {
    pub accepted: bool,
    pub queued: bool,
    pub message: String,
}

#[derive(Clone)]
pub struct DiskRescanCoordinator {
    inner: Arc<DiskRescanCoordinatorInner>,
}

struct DiskRescanCoordinatorInner {
    runner: Arc<dyn DiskRescanRunner>,
    state: Mutex<DiskRescanState>,
}

#[derive(Default)]
struct DiskRescanState {
    running: bool,
    pending: bool,
}

impl DiskRescanCoordinator {
    pub fn new(runner: Arc<dyn DiskRescanRunner>) -> Self {
        Self {
            inner: Arc::new(DiskRescanCoordinatorInner {
                runner,
                state: Mutex::new(DiskRescanState::default()),
            }),
        }
    }

    pub async fn request_rescan(&self, trigger: DiskRescanTrigger) -> DiskRescanAccepted {
        let mut state = self.inner.state.lock().await;
        if state.running {
            state.pending = true;
            tracing::info!(
                source = ?trigger.source,
                device = trigger.device.as_deref(),
                "edge disk rescan already running; queued one follow-up scan"
            );
            return DiskRescanAccepted {
                accepted: true,
                queued: true,
                message: "rescan already running; queued one follow-up scan".to_owned(),
            };
        }

        state.running = true;
        drop(state);

        let coordinator = self.clone();
        tokio::spawn(async move {
            coordinator.run_until_idle(trigger).await;
        });

        DiskRescanAccepted {
            accepted: true,
            queued: false,
            message: "rescan started".to_owned(),
        }
    }

    async fn run_until_idle(&self, mut trigger: DiskRescanTrigger) {
        loop {
            tracing::info!(
                source = ?trigger.source,
                device = trigger.device.as_deref(),
                "starting edge disk rescan"
            );
            match self.inner.runner.run_disk_rescan(trigger.clone()).await {
                Ok(record_count) => tracing::info!(
                    source = ?trigger.source,
                    record_count,
                    "edge disk rescan completed"
                ),
                Err(error) => tracing::error!(
                    source = ?trigger.source,
                    error = %error,
                    "edge disk rescan failed"
                ),
            }

            let mut state = self.inner.state.lock().await;
            if state.pending {
                state.pending = false;
                drop(state);
                trigger = DiskRescanTrigger::queued();
                continue;
            }

            state.running = false;
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };

    struct SlowRunner {
        calls: Arc<AtomicUsize>,
    }

    impl DiskRescanRunner for SlowRunner {
        fn run_disk_rescan<'a>(
            &'a self,
            _trigger: DiskRescanTrigger,
        ) -> Pin<Box<dyn Future<Output = Result<usize, DiskDetectionError>> + Send + 'a>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(20)).await;
                Ok(0)
            })
        }
    }

    #[tokio::test]
    async fn coalesces_concurrent_rescan_requests() {
        let calls = Arc::new(AtomicUsize::new(0));
        let coordinator = DiskRescanCoordinator::new(Arc::new(SlowRunner {
            calls: calls.clone(),
        }));

        let first = coordinator
            .request_rescan(DiskRescanTrigger::udev(Some("/dev/sdb".to_owned())))
            .await;
        let second = coordinator
            .request_rescan(DiskRescanTrigger::udev(Some("/dev/sdc".to_owned())))
            .await;

        assert!(!first.queued);
        assert!(second.queued);
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
