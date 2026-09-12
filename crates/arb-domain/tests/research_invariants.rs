use arb_domain::{Action, ControlError, Evidence, Mode, Progress, Session, State};

fn stopped() -> Session {
    Session::new_research(Mode::Paper)
        .unwrap()
        .complete_recovery()
        .unwrap()
}

fn running() -> Session {
    stopped()
        .request(Action::Start, 0)
        .unwrap()
        .acknowledge(1, true)
        .unwrap()
}

#[test]
fn boot_and_recovery_cannot_resume_automatically() {
    let recovering = Session::new_research(Mode::Paper).unwrap();
    assert_eq!(recovering.state(), State::Recovering);
    assert!(recovering.local_fence_engaged());
    assert_eq!(
        recovering.request(Action::Start, 0),
        Err(ControlError::InvalidTransition)
    );
    let ready = recovering.complete_recovery().unwrap();
    assert_eq!(ready.state(), State::Stopped);
    assert!(!ready.allows_evaluation());
}

#[test]
fn recovery_keeps_unknown_attempts_until_positive_resolution() {
    let recovering = Session::recover_research(Mode::Paper, 2).unwrap();
    assert_eq!(
        recovering.complete_recovery(),
        Err(ControlError::UnresolvedAttempts)
    );
    let one_left = recovering.resolve_attempt().unwrap();
    assert_eq!(one_left.outstanding(), 1);
    assert_eq!(one_left.state(), State::Recovering);
    let ready = one_left.resolve_attempt().unwrap().complete_recovery().unwrap();
    assert_eq!(ready.state(), State::Stopped);
}

#[test]
fn acceptance_is_not_a_worker_acknowledgement() {
    let accepted = running().request(Action::Stop, 1).unwrap();
    assert_eq!(accepted.command_progress(), Some(Progress::Pending));
    assert_eq!(accepted.state(), State::Running);
    assert!(!accepted.local_fence_engaged());
    assert_eq!(accepted.applied_revision(), 1);
    assert_eq!(
        accepted.acknowledge(2, false),
        Err(ControlError::FenceNotEstablished)
    );
}

#[test]
fn applied_stop_with_outstanding_work_is_draining() {
    let running = running().record_attempt().unwrap();
    let accepted = running.request(Action::Stop, 1).unwrap();
    let fenced = accepted.begin_fence(2).unwrap();
    assert_eq!(fenced.state(), State::Pausing);
    assert!(fenced.local_fence_engaged());
    assert_eq!(fenced.command_progress(), Some(Progress::Pending));
    assert_eq!(fenced.generation(), running.generation() + 1);
    let applied = fenced.acknowledge(2, false).unwrap();
    assert_eq!(applied.state(), State::Draining);
    assert_eq!(applied.command_progress(), Some(Progress::Applied));
    assert_eq!(applied.outstanding(), 1);
    assert_eq!(
        applied.record_attempt(),
        Err(ControlError::InvalidTransition)
    );
    assert_eq!(applied.resolve_attempt().unwrap().state(), State::Stopped);
}

#[test]
fn applied_stop_without_outstanding_work_is_stopped() {
    let applied = running()
        .request(Action::Stop, 1)
        .unwrap()
        .begin_fence(2)
        .unwrap()
        .acknowledge(2, false)
        .unwrap();
    assert_eq!(applied.state(), State::Stopped);
    assert!(applied.local_fence_engaged());
    assert_eq!(applied.applied_revision(), 2);
}

#[test]
fn pause_allows_reconciliation_and_resume_requires_readiness() {
    let paused = running()
        .record_attempt()
        .unwrap()
        .request(Action::Pause, 1)
        .unwrap()
        .begin_fence(2)
        .unwrap()
        .acknowledge(2, false)
        .unwrap();
    assert_eq!(paused.state(), State::Paused);
    assert_eq!(paused.outstanding(), 1);
    let reconciled = paused.resolve_attempt().unwrap();
    assert_eq!(reconciled.state(), State::Paused);
    let resume = reconciled.request(Action::Resume, 2).unwrap();
    assert_eq!(resume.acknowledge(3, false), Err(ControlError::NotReady));
    assert!(resume.local_fence_engaged());
    assert!(resume.acknowledge(3, true).unwrap().allows_evaluation());
}

#[test]
fn stale_ack_cannot_override_a_superseding_stop() {
    let start = stopped().request(Action::Start, 0).unwrap();
    let stop = start.request(Action::Stop, 1).unwrap();
    assert_eq!(
        stop.acknowledge(1, true),
        Err(ControlError::NoMatchingCommand)
    );
    let applied = stop.begin_fence(2).unwrap().acknowledge(2, false).unwrap();
    assert_eq!(applied.state(), State::Stopped);
    assert!(applied.local_fence_engaged());
}

#[test]
fn a_stop_superseding_a_started_pause_keeps_the_fence_closed() {
    let pause = running()
        .request(Action::Pause, 1)
        .unwrap()
        .begin_fence(2)
        .unwrap();
    let stop = pause.request(Action::Stop, 2).unwrap();
    assert!(stop.local_fence_engaged());
    assert_eq!(stop.command_progress(), Some(Progress::Pending));
    assert_eq!(
        stop.acknowledge(2, false),
        Err(ControlError::NoMatchingCommand)
    );
    assert_eq!(
        stop.begin_fence(3).unwrap().acknowledge(3, false).unwrap().state(),
        State::Stopped
    );
}

#[test]
fn revision_conflict_does_not_mutate_the_original() {
    let before = running();
    let retained = before.clone();
    assert_eq!(
        before.request(Action::Stop, 0),
        Err(ControlError::RevisionConflict)
    );
    assert_eq!(before, retained);
}

#[test]
fn fault_closes_gate_and_recovery_requires_explicit_new_start() {
    let faulted = running().record_attempt().unwrap().fault().unwrap();
    assert!(faulted.local_fence_engaged());
    assert_eq!(faulted.state(), State::Faulted);
    assert_eq!(faulted.outstanding(), 1);
    let recovering = faulted.begin_recovery().unwrap();
    let ready = recovering.resolve_attempt().unwrap().complete_recovery().unwrap();
    assert_eq!(ready.state(), State::Stopped);
    assert!(!ready.allows_evaluation());
}

#[test]
fn live_and_disarm_are_unavailable_in_the_research_foundation() {
    assert_eq!(
        Session::new_research(Mode::Live),
        Err(ControlError::CapabilityUnavailable)
    );
    assert_eq!(
        stopped().request(Action::Disarm, 0),
        Err(ControlError::CapabilityUnavailable)
    );
}

#[test]
fn research_results_cannot_be_realized() {
    for mode in [Mode::Observe, Mode::Paper, Mode::Replay] {
        assert_eq!(
            arb_domain::validate_evidence_mode(mode, Evidence::Realized),
            Err(ControlError::RealizedRequiresLive)
        );
        assert_eq!(
            arb_domain::validate_evidence_mode(mode, Evidence::Candidate),
            Ok(())
        );
    }
}

#[test]
fn repeated_fence_begin_does_not_increment_generation_twice() {
    let fenced = running().request(Action::Stop, 1).unwrap().begin_fence(2).unwrap();
    assert_eq!(fenced.begin_fence(2).unwrap(), fenced);
}

#[test]
fn resolving_nonexistent_work_is_an_error() {
    assert_eq!(
        stopped().resolve_attempt(),
        Err(ControlError::NoOutstandingAttempt)
    );
}

#[test]
fn stop_during_recovery_does_not_bypass_reconciliation() {
    let applied = Session::recover_research(Mode::Paper, 1)
        .unwrap()
        .request(Action::Stop, 0)
        .unwrap()
        .begin_fence(1)
        .unwrap()
        .acknowledge(1, false)
        .unwrap();
    assert_eq!(applied.command_progress(), Some(Progress::Applied));
    assert_eq!(applied.state(), State::Recovering);
    assert_eq!(applied.outstanding(), 1);
    assert!(applied.local_fence_engaged());
    assert_eq!(
        applied.complete_recovery(),
        Err(ControlError::UnresolvedAttempts)
    );
}

#[test]
fn stop_during_fault_does_not_clear_the_fault() {
    let applied = running()
        .record_attempt()
        .unwrap()
        .fault()
        .unwrap()
        .request(Action::Stop, 1)
        .unwrap()
        .begin_fence(2)
        .unwrap()
        .acknowledge(2, false)
        .unwrap();
    assert_eq!(applied.command_progress(), Some(Progress::Applied));
    assert_eq!(applied.state(), State::Faulted);
    assert!(applied.local_fence_engaged());
    assert_eq!(applied.resolve_attempt().unwrap().state(), State::Faulted);
}
