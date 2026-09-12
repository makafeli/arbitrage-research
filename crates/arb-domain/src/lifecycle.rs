use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Observe,
    Paper,
    Replay,
    /// Reserved for reading future contracts; research sessions reject it.
    Live,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Recovering,
    Stopped,
    Running,
    Pausing,
    Paused,
    Draining,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Start,
    Pause,
    Resume,
    Stop,
    Disarm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Progress {
    Pending,
    Applied,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlError {
    CapabilityUnavailable,
    InvalidTransition,
    RevisionConflict,
    CommandPending,
    NoMatchingCommand,
    FenceNotEstablished,
    NotReady,
    UnresolvedAttempts,
    NoOutstandingAttempt,
    CounterOverflow,
    RealizedRequiresLive,
}

impl fmt::Display for ControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ControlError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Pending {
    action: Action,
    revision: u64,
    fence_started: bool,
}

/// Pure reference reducer. Every operation returns a new state; an error
/// cannot partially mutate the previous state. It neither persists receipts
/// nor coordinates processes. Outstanding attempts can be synthetic paper
/// attempts; their presence never means this crate broadcast a transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    mode: Mode,
    state: State,
    desired_revision: u64,
    applied_revision: u64,
    outstanding: u32,
    local_fence: bool,
    generation: u64,
    pending: Option<Pending>,
    progress: Option<Progress>,
}

impl Session {
    pub fn new_research(mode: Mode) -> Result<Self, ControlError> {
        Self::recover_research(mode, 0)
    }

    /// Restore the unresolved count from a trusted journal. No automatic start.
    pub fn recover_research(mode: Mode, outstanding: u32) -> Result<Self, ControlError> {
        if mode == Mode::Live {
            return Err(ControlError::CapabilityUnavailable);
        }
        Ok(Self {
            mode,
            state: State::Recovering,
            desired_revision: 0,
            applied_revision: 0,
            outstanding,
            local_fence: true,
            generation: 0,
            pending: None,
            progress: None,
        })
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn desired_revision(&self) -> u64 {
        self.desired_revision
    }

    pub fn applied_revision(&self) -> u64 {
        self.applied_revision
    }

    pub fn outstanding(&self) -> u32 {
        self.outstanding
    }

    pub fn local_fence_engaged(&self) -> bool {
        self.local_fence
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn command_progress(&self) -> Option<Progress> {
        self.progress
    }

    pub fn allows_evaluation(&self) -> bool {
        self.state == State::Running && !self.local_fence
    }

    pub fn complete_recovery(&self) -> Result<Self, ControlError> {
        if self.state != State::Recovering {
            return Err(ControlError::InvalidTransition);
        }
        if self.outstanding != 0 {
            return Err(ControlError::UnresolvedAttempts);
        }
        let mut next = self.clone();
        next.state = State::Stopped;
        Ok(next)
    }

    /// Model acceptance only. It does not prove that a worker has fenced.
    /// Durable idempotency and actor authorization are future service concerns.
    pub fn request(&self, action: Action, expected_revision: u64) -> Result<Self, ControlError> {
        if action == Action::Disarm {
            return Err(ControlError::CapabilityUnavailable);
        }
        if expected_revision != self.desired_revision {
            return Err(ControlError::RevisionConflict);
        }
        // A later STOP may supersede START, RESUME or PAUSE. Receipt history is
        // owned by the future durable service, which must mark it SUPERSEDED.
        if self.pending.is_some() && action != Action::Stop {
            return Err(ControlError::CommandPending);
        }
        let compatible = match action {
            Action::Start => self.state == State::Stopped,
            Action::Resume => self.state == State::Paused,
            Action::Pause => self.state == State::Running,
            Action::Stop => true,
            Action::Disarm => false,
        };
        if !compatible {
            return Err(ControlError::InvalidTransition);
        }
        let revision = self
            .desired_revision
            .checked_add(1)
            .ok_or(ControlError::CounterOverflow)?;
        let mut next = self.clone();
        next.desired_revision = revision;
        next.pending = Some(Pending {
            action,
            revision,
            fence_started: false,
        });
        next.progress = Some(Progress::Pending);
        Ok(next)
    }

    /// Worker-local gate closes here; the future journal/transport integration
    /// must serialize this transition with admission to every dispatch call.
    pub fn begin_fence(&self, revision: u64) -> Result<Self, ControlError> {
        let mut pending = self.matching_pending(revision)?;
        if !matches!(pending.action, Action::Pause | Action::Stop) {
            return Err(ControlError::InvalidTransition);
        }
        if pending.fence_started {
            return Ok(self.clone());
        }
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(ControlError::CounterOverflow)?;
        pending.fence_started = true;
        let mut next = self.clone();
        next.local_fence = true;
        next.generation = generation;
        // Stopping must not erase a fault or claim recovery has completed.
        if !matches!(self.state, State::Recovering | State::Faulted) {
            next.state = State::Pausing;
        }
        next.pending = Some(pending);
        Ok(next)
    }

    /// `ready` must come from real health/freshness/limits checks before START
    /// or RESUME. It is deliberately not represented as a trading capability.
    /// PAUSE/STOP require a begun local fence, independent of readiness.
    pub fn acknowledge(&self, revision: u64, ready: bool) -> Result<Self, ControlError> {
        let pending = self.matching_pending(revision)?;
        let mut next = self.clone();
        match pending.action {
            Action::Start | Action::Resume => {
                if !ready {
                    return Err(ControlError::NotReady);
                }
                let expected = if pending.action == Action::Start {
                    State::Stopped
                } else {
                    State::Paused
                };
                if self.state != expected {
                    return Err(ControlError::InvalidTransition);
                }
                next.state = State::Running;
                next.local_fence = false;
            }
            Action::Pause | Action::Stop => {
                if !pending.fence_started || !self.local_fence {
                    return Err(ControlError::FenceNotEstablished);
                }
                next.state = if pending.action == Action::Pause {
                    State::Paused
                } else if matches!(self.state, State::Recovering | State::Faulted) {
                    self.state
                } else if self.outstanding == 0 {
                    State::Stopped
                } else {
                    State::Draining
                };
            }
            Action::Disarm => return Err(ControlError::CapabilityUnavailable),
        }
        next.applied_revision = revision;
        next.pending = None;
        next.progress = Some(Progress::Applied);
        Ok(next)
    }

    /// In-memory attempt accounting only; no transaction is constructed/sent.
    pub fn record_attempt(&self) -> Result<Self, ControlError> {
        if !self.allows_evaluation() {
            return Err(ControlError::InvalidTransition);
        }
        let mut next = self.clone();
        next.outstanding = self
            .outstanding
            .checked_add(1)
            .ok_or(ControlError::CounterOverflow)?;
        Ok(next)
    }

    /// Call only after positive resolution evidence; timeout is insufficient.
    pub fn resolve_attempt(&self) -> Result<Self, ControlError> {
        let mut next = self.clone();
        next.outstanding = self
            .outstanding
            .checked_sub(1)
            .ok_or(ControlError::NoOutstandingAttempt)?;
        if next.state == State::Draining && next.outstanding == 0 {
            next.state = State::Stopped;
        }
        Ok(next)
    }

    pub fn fault(&self) -> Result<Self, ControlError> {
        let mut next = self.clone();
        next.generation = self
            .generation
            .checked_add(1)
            .ok_or(ControlError::CounterOverflow)?;
        next.state = State::Faulted;
        next.local_fence = true;
        if next.pending.take().is_some() {
            next.progress = Some(Progress::Rejected);
        }
        Ok(next)
    }

    pub fn begin_recovery(&self) -> Result<Self, ControlError> {
        if self.state != State::Faulted {
            return Err(ControlError::InvalidTransition);
        }
        let mut next = self.clone();
        next.state = State::Recovering;
        Ok(next)
    }

    fn matching_pending(&self, revision: u64) -> Result<Pending, ControlError> {
        self.pending
            .filter(|pending| pending.revision == revision)
            .ok_or(ControlError::NoMatchingCommand)
    }
}
