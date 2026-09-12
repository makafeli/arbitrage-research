//! Bounded, deterministic research-work admission and fair dispatch.
//!
//! This crate starts no tasks or threads. A fixed-size CPU worker pool polls
//! `dispatch`; Tokio ingestion uses capacity-nonblocking `try_enqueue` admission.
//! The durable control worker must still validate its generation token before
//! accepting a result. Cancellation here is cooperative, never thread abortion.
use arb_domain::NetworkId;
use std::{
    collections::{HashMap, VecDeque},
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const NETWORKS: [NetworkId; 2] = [NetworkId::BaseMainnet, NetworkId::SolanaMainnet];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Ingestion,
    Snapshot,
    Quote,
    Simulation,
    Persistence,
    Api,
}
impl Stage {
    const ALL: [Self; 6] = [
        Self::Ingestion,
        Self::Snapshot,
        Self::Quote,
        Self::Simulation,
        Self::Persistence,
        Self::Api,
    ];
    const fn index(self) -> usize {
        match self {
            Self::Ingestion => 0,
            Self::Snapshot => 1,
            Self::Quote => 2,
            Self::Simulation => 3,
            Self::Persistence => 4,
            Self::Api => 5,
        }
    }
}

/// Bounded, non-secret trace key. Never use correlation IDs as metric labels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorrelationId(String);
impl CorrelationId {
    pub fn new(value: &str) -> Result<Self, SchedulerError> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_:".contains(&b))
        {
            return Err(SchedulerError::InvalidCorrelation);
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropReason {
    GateClosed,
    GenerationChanged,
    QueueFull,
    DeadlineExpired,
    FutureTimestamp,
    Cancelled,
    Abandoned,
}
impl DropReason {
    pub const ALL: [Self; 7] = [
        Self::GateClosed,
        Self::GenerationChanged,
        Self::QueueFull,
        Self::DeadlineExpired,
        Self::FutureTimestamp,
        Self::Cancelled,
        Self::Abandoned,
    ];
    const fn index(self) -> usize {
        match self {
            Self::GateClosed => 0,
            Self::GenerationChanged => 1,
            Self::QueueFull => 2,
            Self::DeadlineExpired => 3,
            Self::FutureTimestamp => 4,
            Self::Cancelled => 5,
            Self::Abandoned => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    InvalidLimits,
    InvalidCorrelation,
    Poisoned,
    PermitSequenceExhausted,
}
impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLimits => "invalid scheduler limits: reserve capacity for both networks",
            Self::InvalidCorrelation => {
                "correlation ID must be 1..128 safe non-secret ASCII characters"
            }
            Self::Poisoned => "scheduler lock poisoned; fail closed and restart through recovery",
            Self::PermitSequenceExhausted => {
                "scheduler permit sequence exhausted; recover before more work"
            }
        })
    }
}
impl std::error::Error for SchedulerError {}

/// Network capacity is a partition, not a burst hint. A stalled network cannot
/// consume all global slots. Payload size must separately be bounded at decoding.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub queue_per_network: usize,
    pub in_flight_per_network: usize,
    pub global_in_flight: usize,
    pub stage_deadlines: [Duration; 6],
}
impl Limits {
    pub fn validate(self) -> Result<Self, SchedulerError> {
        if self.queue_per_network == 0
            || self.queue_per_network > 65_536
            || self.in_flight_per_network == 0
            || self.global_in_flight < 2
            || self.global_in_flight > 256
            || self.in_flight_per_network >= self.global_in_flight
            || self.stage_deadlines.contains(&Duration::ZERO)
        {
            return Err(SchedulerError::InvalidLimits);
        }
        Ok(self)
    }
}

#[derive(Debug)]
pub struct WorkItem<T, G> {
    pub network: NetworkId,
    pub stage: Stage,
    pub generation: G,
    pub correlation_id: CorrelationId,
    /// Capture/admission clock domain: monotonic, never UTC wall time.
    pub observed_at: Instant,
    pub payload: T,
}

#[derive(Debug)]
pub struct Rejected<T, G> {
    pub reason: DropReason,
    pub item: WorkItem<T, G>,
}

/// Successful or rejected computation, separate from scheduler integrity errors.
pub type WorkOutcome<T, G> = Result<WorkItem<T, G>, Rejected<T, G>>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Counters {
    pub accepted: u64,
    pub dispatched: u64,
    pub completed: u64,
    pub dropped: [u64; 7],
}
impl Counters {
    pub fn dropped_for(self, reason: DropReason) -> u64 {
        self.dropped[reason.index()]
    }
    fn record_drop(&mut self, reason: DropReason) {
        self.dropped[reason.index()] = self.dropped[reason.index()].saturating_add(1);
    }
}

#[derive(Clone, Debug)]
pub struct StageSnapshot {
    pub network: NetworkId,
    pub stage: Stage,
    pub queued: usize,
    pub in_flight: usize,
    pub oldest_queue_age: Option<Duration>,
    pub counters: Counters,
}

struct Queued<T, G> {
    item: WorkItem<T, G>,
    enqueued_at: Instant,
}
struct Running {
    network: usize,
    stage: Stage,
    cancelled: Arc<AtomicBool>,
}
struct State<T, G> {
    queues: [VecDeque<Queued<T, G>>; 2],
    gates: [Option<G>; 2],
    running: HashMap<u64, Running>,
    counts: [usize; 2],
    counters: [[Counters; 6]; 2],
    next_network: usize,
    next_permit: u64,
}

pub struct Scheduler<T, G> {
    state: Arc<Mutex<State<T, G>>>,
    limits: Limits,
}
impl<T, G> Clone for Scheduler<T, G> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            limits: self.limits,
        }
    }
}

fn network_index(network: NetworkId) -> usize {
    match network {
        NetworkId::BaseMainnet => 0,
        NetworkId::SolanaMainnet => 1,
    }
}

impl<T, G: Copy + Eq> Scheduler<T, G> {
    pub fn new(limits: Limits) -> Result<Self, SchedulerError> {
        let limits = limits.validate()?;
        Ok(Self {
            limits,
            state: Arc::new(Mutex::new(State {
                queues: std::array::from_fn(|_| VecDeque::with_capacity(limits.queue_per_network)),
                gates: [None, None],
                running: HashMap::with_capacity(limits.global_in_flight),
                counts: [0, 0],
                counters: [[Counters::default(); 6]; 2],
                next_network: 0,
                next_permit: 0,
            })),
        })
    }

    /// Install a confirmed control generation or close the local scheduling gate.
    /// Queued payloads are released AFTER unlocking; a slow destructor cannot
    /// hold the other chain's queue/control lock. Running jobs are only signalled.
    /// This returns no durable command acknowledgement and does not open LIVE.
    pub fn set_gate(
        &self,
        network: NetworkId,
        generation: Option<G>,
    ) -> Result<usize, SchedulerError> {
        let index = network_index(network);
        let discarded;
        {
            let mut state = self.state.lock().map_err(|_| SchedulerError::Poisoned)?;
            if state.gates[index] == generation {
                return Ok(0);
            }
            let reason = if generation.is_none() {
                DropReason::GateClosed
            } else {
                DropReason::GenerationChanged
            };
            state.gates[index] = generation;
            discarded = state.queues[index].drain(..).collect::<Vec<_>>();
            for queued in &discarded {
                state.counters[index][queued.item.stage.index()].record_drop(reason);
            }
            for running in state
                .running
                .values()
                .filter(|running| running.network == index)
            {
                running.cancelled.store(true, Ordering::Release);
            }
        }
        let count = discarded.len();
        drop(discarded);
        Ok(count)
    }

    pub fn fence(&self, network: NetworkId) -> Result<usize, SchedulerError> {
        self.set_gate(network, None)
    }

    /// Does not wait for queue capacity or execute the payload. Caller receives
    /// rejected ownership and must resync/retry according to the stage policy.
    pub fn try_enqueue(
        &self,
        item: WorkItem<T, G>,
        now: Instant,
    ) -> Result<Result<(), Rejected<T, G>>, SchedulerError> {
        let index = network_index(item.network);
        let stage = item.stage.index();
        let mut state = self.state.lock().map_err(|_| SchedulerError::Poisoned)?;
        let reason = match state.gates[index] {
            None => Some(DropReason::GateClosed),
            Some(generation) if generation != item.generation => {
                Some(DropReason::GenerationChanged)
            }
            _ if item.observed_at > now => Some(DropReason::FutureTimestamp),
            _ if now.duration_since(item.observed_at) >= self.limits.stage_deadlines[stage] => {
                Some(DropReason::DeadlineExpired)
            }
            _ if state.queues[index].len() >= self.limits.queue_per_network => {
                Some(DropReason::QueueFull)
            }
            _ => None,
        };
        if let Some(reason) = reason {
            state.counters[index][stage].record_drop(reason);
            return Ok(Err(Rejected { reason, item }));
        }
        state.counters[index][stage].accepted =
            state.counters[index][stage].accepted.saturating_add(1);
        state.queues[index].push_back(Queued {
            item,
            enqueued_at: now,
        });
        Ok(Ok(()))
    }

    /// Round-robin across networks; each call returns at most one CPU work permit.
    /// Stale entries are removed in bounded O(total queue capacity) work.
    pub fn dispatch(&self, now: Instant) -> Result<Option<WorkPermit<T, G>>, SchedulerError> {
        let mut discarded = Vec::new();
        let mut selected = None;
        {
            let mut state = self.state.lock().map_err(|_| SchedulerError::Poisoned)?;
            if state.running.len() >= self.limits.global_in_flight {
                return Ok(None);
            }
            if state.next_permit == u64::MAX {
                return Err(SchedulerError::PermitSequenceExhausted);
            }
            for offset in 0..2 {
                let index = (state.next_network + offset) % 2;
                if state.counts[index] >= self.limits.in_flight_per_network {
                    continue;
                }
                while let Some(queued) = state.queues[index].pop_front() {
                    let item = &queued.item;
                    let reason = match state.gates[index] {
                        None => Some(DropReason::GateClosed),
                        Some(generation) if generation != item.generation => {
                            Some(DropReason::GenerationChanged)
                        }
                        _ if item.observed_at > now => Some(DropReason::FutureTimestamp),
                        _ if now.duration_since(item.observed_at)
                            >= self.limits.stage_deadlines[item.stage.index()] =>
                        {
                            Some(DropReason::DeadlineExpired)
                        }
                        _ => None,
                    };
                    if let Some(reason) = reason {
                        state.counters[index][item.stage.index()].record_drop(reason);
                        discarded.push(queued);
                        continue;
                    }
                    let id = state.next_permit;
                    state.next_permit += 1;
                    let cancelled = Arc::new(AtomicBool::new(false));
                    state.running.insert(
                        id,
                        Running {
                            network: index,
                            stage: item.stage,
                            cancelled: Arc::clone(&cancelled),
                        },
                    );
                    state.counts[index] += 1;
                    state.counters[index][item.stage.index()].dispatched = state.counters[index]
                        [item.stage.index()]
                    .dispatched
                    .saturating_add(1);
                    state.next_network = (index + 1) % 2;
                    selected = Some(WorkPermit {
                        scheduler: self.clone(),
                        id,
                        item: Some(queued.item),
                        queue_age: now.saturating_duration_since(queued.enqueued_at),
                        cancellation: Cancellation {
                            cancelled,
                            deadline: queued.enqueued_at,
                            observed_at: queued.enqueued_at,
                        },
                    });
                    // Expiry is based on observation time, not dispatch time.
                    if let Some(permit) = &mut selected {
                        let item = permit.item.as_ref().expect("new permit has work");
                        permit.cancellation.observed_at = item.observed_at;
                        permit.cancellation.deadline = item
                            .observed_at
                            .checked_add(self.limits.stage_deadlines[item.stage.index()])
                            .unwrap_or(item.observed_at);
                    }
                    break;
                }
                if selected.is_some() {
                    break;
                }
            }
        }
        drop(discarded);
        Ok(selected)
    }

    /// Fixed cardinality: always exactly two networks × six stages. Correlation
    /// IDs stay with individual work/results, never become time-series labels.
    pub fn snapshot(&self, now: Instant) -> Result<Vec<StageSnapshot>, SchedulerError> {
        let state = self.state.lock().map_err(|_| SchedulerError::Poisoned)?;
        Ok(NETWORKS
            .into_iter()
            .enumerate()
            .flat_map(|(index, network)| {
                Stage::ALL
                    .into_iter()
                    .map(move |stage| (index, network, stage))
            })
            .map(|(index, network, stage)| {
                let queued = state.queues[index]
                    .iter()
                    .filter(|queued| queued.item.stage == stage)
                    .collect::<Vec<_>>();
                StageSnapshot {
                    network,
                    stage,
                    queued: queued.len(),
                    in_flight: state
                        .running
                        .values()
                        .filter(|running| running.network == index && running.stage == stage)
                        .count(),
                    oldest_queue_age: queued
                        .iter()
                        .map(|queued| now.saturating_duration_since(queued.enqueued_at))
                        .max(),
                    counters: state.counters[index][stage.index()],
                }
            })
            .collect())
    }

    fn complete(
        &self,
        id: u64,
        generation: G,
        now: Instant,
        cancellation: &Cancellation,
    ) -> Result<Result<(), DropReason>, SchedulerError> {
        let mut state = self.state.lock().map_err(|_| SchedulerError::Poisoned)?;
        let running = state
            .running
            .remove(&id)
            .expect("owned permit is completed exactly once");
        state.counts[running.network] -= 1;
        let rejected = match state.gates[running.network] {
            None => Some(DropReason::GateClosed),
            Some(current) if current != generation => Some(DropReason::GenerationChanged),
            _ => cancellation.check(now).err(),
        };
        let counters = &mut state.counters[running.network][running.stage.index()];
        match rejected {
            Some(reason) => {
                counters.record_drop(reason);
                Ok(Err(reason))
            }
            None => {
                counters.completed = counters.completed.saturating_add(1);
                Ok(Ok(()))
            }
        }
    }
}

/// Cooperative signal: CPU algorithms must check between bounded work chunks.
/// A cancelled or expired computation may finish running; its result is rejected.
#[derive(Clone)]
pub struct Cancellation {
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
    observed_at: Instant,
}
impl Cancellation {
    pub fn check(&self, now: Instant) -> Result<(), DropReason> {
        if self.cancelled.load(Ordering::Acquire) {
            Err(DropReason::Cancelled)
        } else if now < self.observed_at {
            Err(DropReason::FutureTimestamp)
        } else if now >= self.deadline {
            Err(DropReason::DeadlineExpired)
        } else {
            Ok(())
        }
    }
}

/// In-flight quota remains reserved until this permit completes or drops. It
/// cannot be cloned. A panic/drop releases quota and records abandonment.
pub struct WorkPermit<T, G: Copy + Eq> {
    scheduler: Scheduler<T, G>,
    id: u64,
    item: Option<WorkItem<T, G>>,
    queue_age: Duration,
    cancellation: Cancellation,
}
impl<T, G: Copy + Eq> WorkPermit<T, G> {
    pub fn item(&self) -> &WorkItem<T, G> {
        self.item.as_ref().expect("live permit owns work")
    }
    pub fn queue_age(&self) -> Duration {
        self.queue_age
    }
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }
    /// After CPU work, recheck fence/generation/deadline before exposing results.
    /// A successful return still requires durable ControlWorker admission.
    pub fn finish(mut self, now: Instant) -> Result<WorkOutcome<T, G>, SchedulerError> {
        let generation = self.item().generation;
        let outcome = self
            .scheduler
            .complete(self.id, generation, now, &self.cancellation)?;
        let item = self.item.take().expect("live permit owns work");
        Ok(match outcome {
            Ok(()) => Ok(item),
            Err(reason) => Err(Rejected { reason, item }),
        })
    }
}
impl<T, G: Copy + Eq> Drop for WorkPermit<T, G> {
    fn drop(&mut self) {
        if self.item.is_none() {
            return;
        }
        // Poisoning is already fail-closed at every admission boundary. Release
        // accounting even during unwinding without introducing a second panic.
        let mut state = self
            .scheduler
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(running) = state.running.remove(&self.id) {
            state.counts[running.network] -= 1;
            state.counters[running.network][running.stage.index()]
                .record_drop(DropReason::Abandoned);
        }
    }
}
