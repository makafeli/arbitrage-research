use arb_domain::NetworkId::{self, BaseMainnet as Base, SolanaMainnet as Solana};
use arb_scheduler::{
    CorrelationId, DropReason, Limits, Scheduler, SchedulerError, Stage, WorkItem,
};
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

fn limits() -> Limits {
    Limits {
        queue_per_network: 3,
        in_flight_per_network: 1,
        global_in_flight: 2,
        stage_deadlines: [Duration::from_secs(1); 6],
    }
}
fn item(network: NetworkId, observed_at: Instant, payload: u64) -> WorkItem<u64, u64> {
    WorkItem {
        network,
        stage: Stage::Quote,
        generation: 7,
        correlation_id: CorrelationId::new(&format!("fixture:{payload}")).unwrap(),
        observed_at,
        payload,
    }
}
fn scheduler() -> Scheduler<u64, u64> {
    let scheduler = Scheduler::new(limits()).unwrap();
    scheduler.set_gate(Base, Some(7)).unwrap();
    scheduler.set_gate(Solana, Some(7)).unwrap();
    scheduler
}
fn accepted(scheduler: &Scheduler<u64, u64>, network: NetworkId, now: Instant, payload: u64) {
    assert!(
        scheduler
            .try_enqueue(item(network, now, payload), now)
            .unwrap()
            .is_ok()
    );
}

#[test]
fn limits_preserve_an_execution_slot_for_the_other_network() {
    let mut config = limits();
    config.global_in_flight = 1;
    assert!(matches!(
        Scheduler::<u64, u64>::new(config),
        Err(SchedulerError::InvalidLimits)
    ));
    config = limits();
    config.in_flight_per_network = 2;
    assert!(config.validate().is_err());
    config = limits();
    config.stage_deadlines[Stage::Quote as usize] = Duration::ZERO;
    assert!(config.validate().is_err());
    for key in ["", "a secret value", "bearer/token", &"x".repeat(129)] {
        assert!(CorrelationId::new(key).is_err());
    }
}

#[test]
fn overload_is_bounded_and_reports_chain_stage_and_queue_age() {
    let scheduler = scheduler();
    let now = Instant::now();
    for n in 0..100 {
        let result = scheduler.try_enqueue(item(Solana, now, n), now).unwrap();
        if n < 3 {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.unwrap_err().reason, DropReason::QueueFull);
        }
    }
    let snapshot = scheduler.snapshot(now + Duration::from_millis(25)).unwrap();
    assert_eq!(snapshot.len(), 12);
    let sol = snapshot
        .iter()
        .find(|value| value.network == Solana && value.stage == Stage::Quote)
        .unwrap();
    assert_eq!(sol.queued, 3);
    assert_eq!(sol.oldest_queue_age, Some(Duration::from_millis(25)));
    assert_eq!(sol.counters.accepted, 3);
    assert_eq!(sol.counters.dropped_for(DropReason::QueueFull), 97);
    let base = snapshot
        .iter()
        .find(|value| value.network == Base && value.stage == Stage::Quote)
        .unwrap();
    assert_eq!(base.queued, 0);
}

#[test]
fn round_robin_fairness_preserves_fifo_within_each_network() {
    let scheduler = scheduler();
    let now = Instant::now();
    for n in 0..3 {
        accepted(&scheduler, Solana, now, 10 + n);
        accepted(&scheduler, Base, now, n);
    }
    let mut results = Vec::new();
    for _ in 0..6 {
        let permit = scheduler.dispatch(now).unwrap().unwrap();
        results.push((permit.item().network, permit.item().payload));
        assert!(permit.finish(now).unwrap().is_ok());
    }
    assert_eq!(
        results,
        vec![
            (Base, 0),
            (Solana, 10),
            (Base, 1),
            (Solana, 11),
            (Base, 2),
            (Solana, 12)
        ]
    );
    assert!(scheduler.dispatch(now).unwrap().is_none());
}

#[test]
fn stalled_solana_permit_does_not_starve_base() {
    let scheduler = scheduler();
    let now = Instant::now();
    accepted(&scheduler, Solana, now, 9);
    let slow = scheduler.dispatch(now).unwrap().unwrap();
    assert_eq!(slow.item().network, Solana);
    for n in 0..20 {
        accepted(&scheduler, Base, now, n);
        let permit = scheduler.dispatch(now).unwrap().unwrap();
        assert_eq!(permit.item().network, Base);
        assert!(permit.finish(now).unwrap().is_ok());
    }
    assert!(slow.finish(now).unwrap().is_ok());
}

#[test]
fn global_cap_and_raii_release_survive_abandoned_work() {
    let scheduler = scheduler();
    let now = Instant::now();
    accepted(&scheduler, Base, now, 1);
    accepted(&scheduler, Solana, now, 2);
    accepted(&scheduler, Base, now, 3);
    let first = scheduler.dispatch(now).unwrap().unwrap();
    let second = scheduler.dispatch(now).unwrap().unwrap();
    assert!(scheduler.dispatch(now).unwrap().is_none());
    drop(first);
    let third = scheduler.dispatch(now).unwrap().unwrap();
    assert_eq!(third.item().payload, 3);
    assert!(third.finish(now).unwrap().is_ok());
    drop(second);
    let snapshot = scheduler.snapshot(now).unwrap();
    assert_eq!(
        snapshot
            .iter()
            .map(|value| value.counters.dropped_for(DropReason::Abandoned))
            .sum::<u64>(),
        2
    );
    assert!(snapshot.iter().all(|value| value.in_flight == 0));
}

#[test]
fn stale_work_is_rejected_at_admission_dispatch_and_completion() {
    let scheduler = scheduler();
    let now = Instant::now();
    assert_eq!(
        scheduler
            .try_enqueue(item(Base, now, 1), now + Duration::from_secs(1))
            .unwrap()
            .unwrap_err()
            .reason,
        DropReason::DeadlineExpired
    );
    assert_eq!(
        scheduler
            .try_enqueue(item(Base, now + Duration::from_secs(1), 2), now)
            .unwrap()
            .unwrap_err()
            .reason,
        DropReason::FutureTimestamp
    );
    accepted(&scheduler, Base, now, 3);
    assert!(
        scheduler
            .dispatch(now + Duration::from_secs(1))
            .unwrap()
            .is_none()
    );
    accepted(&scheduler, Base, now, 4);
    let permit = scheduler
        .dispatch(now + Duration::from_millis(100))
        .unwrap()
        .unwrap();
    assert_eq!(permit.queue_age(), Duration::from_millis(100));
    assert_eq!(
        permit.cancellation().check(now + Duration::from_secs(1)),
        Err(DropReason::DeadlineExpired)
    );
    assert_eq!(
        permit
            .finish(now + Duration::from_secs(1))
            .unwrap()
            .unwrap_err()
            .reason,
        DropReason::DeadlineExpired
    );
    let snapshot = scheduler.snapshot(now + Duration::from_secs(1)).unwrap();
    let base = snapshot
        .iter()
        .find(|value| value.network == Base && value.stage == Stage::Quote)
        .unwrap();
    assert_eq!(base.counters.dropped_for(DropReason::DeadlineExpired), 3);
}

#[test]
fn stop_fences_queued_and_running_work_without_waiting_for_cpu() {
    let scheduler = scheduler();
    let now = Instant::now();
    accepted(&scheduler, Solana, now, 1);
    accepted(&scheduler, Solana, now, 2);
    let permit = scheduler.dispatch(now).unwrap().unwrap();
    let token = permit.cancellation();
    let (resume_tx, resume_rx) = mpsc::sync_channel(0);
    let cpu = thread::spawn(move || {
        resume_rx.recv().unwrap();
        permit.finish(now).unwrap()
    });
    // The worker is unable to finish before resume_tx sends. Fence must return
    // first; this proves no wait on the CPU job, without a wall-clock benchmark.
    assert_eq!(scheduler.fence(Solana).unwrap(), 1);
    assert_eq!(token.check(now), Err(DropReason::Cancelled));
    assert_eq!(
        scheduler
            .try_enqueue(item(Solana, now, 3), now)
            .unwrap()
            .unwrap_err()
            .reason,
        DropReason::GateClosed
    );
    accepted(&scheduler, Base, now, 4);
    assert!(
        scheduler
            .dispatch(now)
            .unwrap()
            .unwrap()
            .finish(now)
            .unwrap()
            .is_ok()
    );
    resume_tx.send(()).unwrap();
    assert_eq!(
        cpu.join().unwrap().unwrap_err().reason,
        DropReason::GateClosed
    );
}

#[test]
fn generation_rotation_prevents_old_work_and_resume_aba() {
    let scheduler = scheduler();
    let now = Instant::now();
    accepted(&scheduler, Base, now, 1);
    let old = scheduler.dispatch(now).unwrap().unwrap();
    scheduler.fence(Base).unwrap();
    scheduler.set_gate(Base, Some(7)).unwrap();
    // Same numeric generation cannot resurrect an old cancellation token.
    assert_eq!(
        old.finish(now).unwrap().unwrap_err().reason,
        DropReason::Cancelled
    );
    accepted(&scheduler, Base, now, 2);
    let old = scheduler.dispatch(now).unwrap().unwrap();
    scheduler.set_gate(Base, Some(8)).unwrap();
    assert_eq!(
        old.finish(now).unwrap().unwrap_err().reason,
        DropReason::GenerationChanged
    );
    assert_eq!(
        scheduler
            .try_enqueue(item(Base, now, 3), now)
            .unwrap()
            .unwrap_err()
            .reason,
        DropReason::GenerationChanged
    );
    let mut fresh = item(Base, now, 4);
    fresh.generation = 8;
    assert!(scheduler.try_enqueue(fresh, now).unwrap().is_ok());
    assert!(
        scheduler
            .dispatch(now)
            .unwrap()
            .unwrap()
            .finish(now)
            .unwrap()
            .is_ok()
    );
}

#[test]
fn new_scheduler_starts_with_both_gates_closed() {
    let scheduler = Scheduler::new(limits()).unwrap();
    let now = Instant::now();
    for network in [Base, Solana] {
        assert_eq!(
            scheduler
                .try_enqueue(item(network, now, 0), now)
                .unwrap()
                .unwrap_err()
                .reason,
            DropReason::GateClosed
        );
    }
}
