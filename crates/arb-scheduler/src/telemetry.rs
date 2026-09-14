//! Bounded, best-effort operational snapshots. This is not an audit journal.
//! A slow consumer owns its frame without retaining any scheduler/control lock.
use crate::{Scheduler, SchedulerError, StageSnapshot};
use std::{
    sync::mpsc::{self, Receiver, SyncSender, TrySendError},
    time::Instant,
};

#[derive(Debug)]
pub struct Frame {
    pub sampled_at: Instant,
    pub stages: Vec<StageSnapshot>,
    /// Cumulative samples not published before this frame, not dropped work.
    pub missed_samples: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishOutcome {
    Published,
    SchedulerBusy,
    ConsumerFull,
    ConsumerDisconnected,
}

/// One producer per process; bounded channel slots each hold twelve stage rows.
/// No background thread, output sink, payload or unbounded trace buffer lives here.
pub struct Publisher {
    sender: SyncSender<Frame>,
    missed_samples: u64,
}

pub fn channel(capacity: usize) -> Result<(Publisher, Receiver<Frame>), SchedulerError> {
    if !(1..=16).contains(&capacity) {
        return Err(SchedulerError::InvalidLimits);
    }
    let (sender, receiver) = mpsc::sync_channel(capacity);
    Ok((
        Publisher {
            sender,
            missed_samples: 0,
        },
        receiver,
    ))
}

impl Publisher {
    pub fn missed_samples(&self) -> u64 {
        self.missed_samples
    }

    /// Never waits for channel capacity or consumer I/O. A contended scheduler
    /// snapshot is skipped. Scheduler integrity errors still fail closed.
    pub fn try_publish<T, G: Copy + Eq>(
        &mut self,
        scheduler: &Scheduler<T, G>,
        now: Instant,
    ) -> Result<PublishOutcome, SchedulerError> {
        let Some(stages) = scheduler.try_snapshot(now)? else {
            self.missed_samples = self.missed_samples.saturating_add(1);
            return Ok(PublishOutcome::SchedulerBusy);
        };
        let result = match self.sender.try_send(Frame {
            sampled_at: now,
            stages,
            missed_samples: self.missed_samples,
        }) {
            Ok(()) => return Ok(PublishOutcome::Published),
            Err(TrySendError::Full(_)) => PublishOutcome::ConsumerFull,
            Err(TrySendError::Disconnected(_)) => PublishOutcome::ConsumerDisconnected,
        };
        self.missed_samples = self.missed_samples.saturating_add(1);
        Ok(result)
    }
}
