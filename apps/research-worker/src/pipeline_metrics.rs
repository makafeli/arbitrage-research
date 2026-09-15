//! Fixed-memory observations at actual worker call boundaries, never an audit log.
//! Only a bounded last collection UUID is retained; it is not a metric label.
use arb_domain::NetworkId;
use arb_scheduler::timing::{Histogram, LatencySummary};
use std::{
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub enum Component {
    Rpc,
    SnapshotDecode,
    Evaluation,
    Persistence,
}
impl Component {
    const ALL: [Self; 4] = [
        Self::Rpc,
        Self::SnapshotDecode,
        Self::Evaluation,
        Self::Persistence,
    ];
    fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Rpc => "ingestion_rpc",
            Self::SnapshotDecode => "snapshot_decode_excluding_rpc",
            Self::Evaluation => "route_evaluation",
            Self::Persistence => "capture_and_journal_persistence",
        }
    }
}

#[derive(Default)]
struct Population {
    completed: Histogram,
    failed: Histogram,
    unfinished: u64,
    last_collection: Option<Uuid>,
}

pub struct Row {
    pub component: Component,
    pub completed: LatencySummary,
    pub failed: LatencySummary,
    pub unfinished: u64,
    pub last_collection: Option<Uuid>,
}

pub struct Snapshot {
    pub network: NetworkId,
    pub rows: Vec<Row>,
    pub missed_samples: u64,
}

pub struct PipelineMetrics {
    network: NetworkId,
    populations: Mutex<[Population; 4]>,
    missed: AtomicU64,
}
impl PipelineMetrics {
    pub fn new(network: NetworkId) -> Arc<Self> {
        Arc::new(Self {
            network,
            populations: Mutex::new(std::array::from_fn(|_| Population::default())),
            missed: AtomicU64::new(0),
        })
    }

    fn miss(&self) {
        let _ = self
            .missed
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some(value.saturating_add(1))
            });
    }

    /// Nonblocking and fixed-memory, including when operational output is disabled.
    /// None means interrupted or unmeasurable, never a zero-duration completion.
    pub fn record(
        &self,
        component: Component,
        collection: Uuid,
        duration: Option<Duration>,
        ok: bool,
    ) {
        let Ok(mut populations) = self.populations.try_lock() else {
            self.miss();
            return;
        };
        let population = &mut populations[component.index()];
        population.last_collection = Some(collection);
        match duration {
            Some(duration) if ok => population.completed.record(duration),
            Some(duration) => population.failed.record(duration),
            None => population.unfinished = population.unfinished.saturating_add(1),
        }
    }

    pub fn span(self: &Arc<Self>, component: Component, collection: Uuid) -> Span {
        Span {
            metrics: Arc::clone(self),
            component,
            collection,
            started: Instant::now(),
            finished: false,
        }
    }

    /// The output consumer copies a bounded snapshot before any encoding or I/O.
    pub fn snapshot(&self) -> Option<Snapshot> {
        let Ok(populations) = self.populations.try_lock() else {
            self.miss();
            return None;
        };
        Some(Snapshot {
            network: self.network,
            rows: Component::ALL
                .into_iter()
                .map(|component| {
                    let population = &populations[component.index()];
                    Row {
                        component,
                        completed: population.completed.snapshot(),
                        failed: population.failed.snapshot(),
                        unfinished: population.unfinished,
                        last_collection: population.last_collection,
                    }
                })
                .collect(),
            missed_samples: self.missed.load(Ordering::Relaxed),
        })
    }
}

pub struct Span {
    metrics: Arc<PipelineMetrics>,
    component: Component,
    collection: Uuid,
    started: Instant,
    finished: bool,
}
impl Span {
    /// A successful stage is not a successful trade, an admission, or settlement.
    pub fn finish(mut self, ok: bool) {
        self.metrics.record(
            self.component,
            self.collection,
            Instant::now().checked_duration_since(self.started),
            ok,
        );
        self.finished = true;
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if !self.finished {
            self.metrics
                .record(self.component, self.collection, None, false);
        }
    }
}

pub async fn persistence<T, E>(
    metrics: &Arc<PipelineMetrics>,
    collection: Uuid,
    operation: impl Future<Output = Result<T, E>>,
) -> Result<T, E> {
    let span = metrics.span(Component::Persistence, collection);
    let result = operation.await;
    span.finish(result.is_ok());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurements_keep_origins_outcomes_and_unfinished_work_separate() {
        let base = PipelineMetrics::new(NetworkId::BaseMainnet);
        let solana = PipelineMetrics::new(NetworkId::SolanaMainnet);
        let id = Uuid::new_v4();
        base.record(Component::Rpc, id, Some(Duration::from_nanos(3)), true);
        base.record(Component::Rpc, id, Some(Duration::from_nanos(9)), false);
        drop(base.span(Component::Rpc, id));
        let snapshot = base.snapshot().unwrap();
        assert_eq!(snapshot.network, NetworkId::BaseMainnet);
        assert_eq!(snapshot.rows.len(), 4);
        assert_eq!(snapshot.rows[0].completed.samples, 1);
        assert_eq!(snapshot.rows[0].completed.max_ns, Some(3));
        assert_eq!(snapshot.rows[0].failed.max_ns, Some(9));
        assert_eq!(snapshot.rows[0].unfinished, 1);
        assert_eq!(snapshot.rows[0].last_collection, Some(id));
        assert_eq!(snapshot.rows[1].completed.p99_upper_ns, None);
        assert!(
            solana
                .snapshot()
                .unwrap()
                .rows
                .iter()
                .all(|r| r.completed.samples == 0)
        );
    }

    #[test]
    fn contention_skips_metrics_instead_of_waiting_or_holding_control() {
        let metrics = PipelineMetrics::new(NetworkId::BaseMainnet);
        let held = metrics.populations.lock().unwrap();
        metrics.record(
            Component::Evaluation,
            Uuid::new_v4(),
            Some(Duration::ZERO),
            true,
        );
        assert!(metrics.snapshot().is_none());
        drop(held);
        let snapshot = metrics.snapshot().unwrap();
        assert_eq!(snapshot.missed_samples, 2);
        assert_eq!(snapshot.rows[2].completed.samples, 0);
    }

    #[test]
    fn arbitrarily_many_collections_do_not_grow_storage_or_metric_labels() {
        let metrics = PipelineMetrics::new(NetworkId::BaseMainnet);
        let bytes = std::mem::size_of::<PipelineMetrics>();
        let mut last = Uuid::nil();
        for i in 1..=10000_u128 {
            last = Uuid::from_u128(i);
            metrics.record(Component::Rpc, last, Some(Duration::from_nanos(5)), true);
        }
        let snapshot = metrics.snapshot().unwrap();
        assert_eq!(snapshot.rows.len(), 4);
        assert_eq!(snapshot.rows[0].completed.samples, 10000);
        assert_eq!(snapshot.rows[0].last_collection, Some(last));
        assert!(bytes < 16384);
    }

    #[tokio::test]
    async fn dropped_polled_future_records_one_unfinished_persistence() {
        let metrics = PipelineMetrics::new(NetworkId::SolanaMainnet);
        {
            let future = persistence(
                &metrics,
                Uuid::new_v4(),
                std::future::pending::<Result<(), ()>>(),
            );
            tokio::pin!(future);
            tokio::select! {
                biased;
                _ = &mut future => panic!("pending operation completed"),
                _ = tokio::task::yield_now() => {},
            }
        }
        let snapshot = metrics.snapshot().unwrap();
        assert_eq!(snapshot.rows[3].unfinished, 1);
        assert_eq!(snapshot.rows[3].failed.samples, 0);
        assert_eq!(snapshot.rows[3].completed.samples, 0);
        assert!(snapshot.rows[3].completed.min_ns.is_none());
    }

    #[tokio::test]
    async fn persistence_result_and_error_are_returned_unchanged() {
        let metrics = PipelineMetrics::new(NetworkId::BaseMainnet);
        assert_eq!(
            persistence(&metrics, Uuid::new_v4(), async { Ok::<_, &str>(17) }).await,
            Ok(17)
        );
        assert_eq!(
            persistence(&metrics, Uuid::new_v4(), async { Err::<u8, _>("original") }).await,
            Err("original")
        );
        let snapshot = metrics.snapshot().unwrap();
        assert_eq!(snapshot.rows[3].completed.samples, 1);
        assert_eq!(snapshot.rows[3].failed.samples, 1);
        assert_eq!(snapshot.rows[3].unfinished, 0);
    }
}
