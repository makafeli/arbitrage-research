//! Injected-clock contract tests, not measurements of real market latency.
use arb_domain::NetworkId::{self, BaseMainnet as Base, SolanaMainnet as Solana};
use arb_scheduler::{CorrelationId, DropReason, Limits, Scheduler, Stage, WorkItem};
use std::time::{Duration, Instant};

fn scheduler() -> Scheduler<u64, u64> {
    let scheduler = Scheduler::new(Limits {
        queue_per_network: 2,
        in_flight_per_network: 1,
        global_in_flight: 2,
        stage_deadlines: [Duration::from_secs(1); 6],
    })
    .unwrap();
    scheduler.set_gate(Base, Some(7)).unwrap();
    scheduler.set_gate(Solana, Some(7)).unwrap();
    scheduler
}
fn enqueue(
    s: &Scheduler<u64, u64>,
    network: NetworkId,
    stage: Stage,
    observed: Instant,
    admitted: Instant,
) {
    assert!(
        s.try_enqueue(
            WorkItem {
                network,
                stage,
                generation: 7,
                correlation_id: CorrelationId::new("fixture:timing").unwrap(),
                observed_at: observed,
                payload: 1,
            },
            admitted
        )
        .unwrap()
        .is_ok()
    );
}

#[test]
fn queue_and_execution_have_separate_real_clock_boundaries() {
    let s = scheduler();
    let now = Instant::now();
    enqueue(&s, Base, Stage::Quote, now, now + Duration::from_nanos(100));
    let permit = s
        .dispatch(now + Duration::from_nanos(110))
        .unwrap()
        .unwrap();
    assert!(
        permit
            .finish(now + Duration::from_nanos(310))
            .unwrap()
            .is_ok()
    );
    let rows = s.snapshot(now + Duration::from_nanos(310)).unwrap();
    let row = rows
        .iter()
        .find(|r| r.network == Base && r.stage == Stage::Quote)
        .unwrap();
    assert_eq!(row.timing.queue_wait.samples, 1);
    assert_eq!(row.timing.queue_wait.p50_upper_ns, Some(10));
    assert_eq!(row.timing.successful_execution.p99_upper_ns, Some(200));
    assert_eq!(row.timing.rejected_execution.samples, 0);
    assert_eq!(
        rows.iter()
            .map(|r| r.timing.successful_execution.samples)
            .sum::<u64>(),
        1
    );
}

#[test]
fn reversed_dispatch_clock_is_rejected_instead_of_becoming_zero_wait() {
    let s = scheduler();
    let now = Instant::now();
    enqueue(&s, Base, Stage::Quote, now, now + Duration::from_millis(10));
    assert!(
        s.dispatch(now + Duration::from_millis(5))
            .unwrap()
            .is_none()
    );
    let rows = s.snapshot(now).unwrap();
    let row = rows
        .iter()
        .find(|r| r.network == Base && r.stage == Stage::Quote)
        .unwrap();
    assert_eq!(row.counters.dropped_for(DropReason::FutureTimestamp), 1);
    assert_eq!(row.timing.queue_wait.samples, 0);
    assert_eq!(row.timing.successful_execution.p50_upper_ns, None);
}

#[test]
fn reversed_finish_clock_is_rejected_and_keeps_duration_unknown() {
    let s = scheduler();
    let now = Instant::now();
    enqueue(&s, Base, Stage::Quote, now, now);
    let permit = s
        .dispatch(now + Duration::from_millis(10))
        .unwrap()
        .unwrap();
    assert_eq!(
        permit
            .finish(now + Duration::from_millis(5))
            .unwrap()
            .unwrap_err()
            .reason,
        DropReason::FutureTimestamp
    );
    let rows = s.snapshot(now).unwrap();
    let row = rows
        .iter()
        .find(|r| r.network == Base && r.stage == Stage::Quote)
        .unwrap();
    assert_eq!(row.timing.unmeasurable_completions, 1);
    assert_eq!(row.counters.completed, 0);
    assert_eq!(row.timing.successful_execution.samples, 0);
    assert_eq!(row.timing.rejected_execution.samples, 0);
}

#[test]
fn expired_and_cancelled_finishes_do_not_enter_success_population() {
    let s = scheduler();
    let now = Instant::now();
    enqueue(&s, Base, Stage::Quote, now, now);
    let expired = s.dispatch(now).unwrap().unwrap();
    assert_eq!(
        expired
            .finish(now + Duration::from_secs(2))
            .unwrap()
            .unwrap_err()
            .reason,
        DropReason::DeadlineExpired
    );
    enqueue(&s, Base, Stage::Quote, now, now);
    let cancelled = s.dispatch(now).unwrap().unwrap();
    s.fence(Base).unwrap();
    assert_eq!(
        cancelled
            .finish(now + Duration::from_millis(3))
            .unwrap()
            .unwrap_err()
            .reason,
        DropReason::GateClosed
    );
    let rows = s.snapshot(now).unwrap();
    let row = rows
        .iter()
        .find(|r| r.network == Base && r.stage == Stage::Quote)
        .unwrap();
    assert_eq!(row.timing.rejected_execution.samples, 2);
    assert_eq!(row.timing.rejected_execution.min_ns, Some(3_000_000));
    assert_eq!(row.timing.rejected_execution.max_ns, Some(2_000_000_000));
    assert_eq!(row.timing.successful_execution.samples, 0);
}

#[test]
fn abandonment_does_not_fabricate_completed_execution_time() {
    let s = scheduler();
    let now = Instant::now();
    enqueue(&s, Solana, Stage::Quote, now, now);
    let unfinished = s.dispatch(now).unwrap().unwrap();
    drop(unfinished);
    let rows = s.snapshot(now).unwrap();
    let row = rows
        .iter()
        .find(|r| r.network == Solana && r.stage == Stage::Quote)
        .unwrap();
    assert_eq!(row.timing.queue_wait.samples, 1);
    assert_eq!(row.counters.dropped_for(DropReason::Abandoned), 1);
    assert_eq!(row.timing.successful_execution.samples, 0);
    assert_eq!(row.timing.rejected_execution.samples, 0);
}

#[test]
fn all_six_stage_names_and_both_networks_keep_independent_populations() {
    let s = scheduler();
    let now = Instant::now();
    let stages = [
        Stage::Ingestion,
        Stage::Snapshot,
        Stage::Quote,
        Stage::Simulation,
        Stage::Persistence,
        Stage::Api,
    ];
    for (n, network) in [Base, Solana].into_iter().enumerate() {
        for (i, stage) in stages.into_iter().enumerate() {
            enqueue(&s, network, stage, now, now);
            let permit = s.dispatch(now).unwrap().unwrap();
            assert!(
                permit
                    .finish(now + Duration::from_nanos((n * 6 + i + 1) as u64))
                    .unwrap()
                    .is_ok()
            );
        }
    }
    let rows = s.try_snapshot(now).unwrap().unwrap();
    assert_eq!(rows.len(), 12);
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.timing.successful_execution.samples, 1);
        assert_eq!(row.timing.successful_execution.max_ns, Some(i as u128 + 1));
        assert_eq!(row.timing.rejected_execution.samples, 0);
    }
}
