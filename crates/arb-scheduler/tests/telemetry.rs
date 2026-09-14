use arb_domain::NetworkId::{self, BaseMainnet as Base, SolanaMainnet as Solana};
use arb_scheduler::{
    CorrelationId, DropReason, Limits, Scheduler, Stage, WorkItem,
    telemetry::{self, PublishOutcome},
};
use std::time::{Duration, Instant};

fn scheduler() -> Scheduler<u64, u64> {
    let scheduler = Scheduler::new(Limits {
        queue_per_network: 2,
        in_flight_per_network: 1,
        global_in_flight: 2,
        stage_deadlines: [Duration::from_secs(5); 6],
    })
    .unwrap();
    scheduler.set_gate(Base, Some(7)).unwrap();
    scheduler.set_gate(Solana, Some(7)).unwrap();
    scheduler
}
fn enqueue(scheduler: &Scheduler<u64, u64>, network: NetworkId, now: Instant) {
    assert!(
        scheduler
            .try_enqueue(
                WorkItem {
                    network,
                    stage: Stage::Quote,
                    generation: 7,
                    correlation_id: CorrelationId::new("fixture:telemetry").unwrap(),
                    observed_at: now,
                    payload: 1,
                },
                now,
            )
            .unwrap()
            .is_ok()
    );
}

#[test]
fn capacity_is_explicit_and_bounded() {
    for capacity in [0, 17, usize::MAX] {
        assert!(telemetry::channel(capacity).is_err());
    }
    assert!(telemetry::channel(1).is_ok());
    assert!(telemetry::channel(16).is_ok());
}

#[test]
fn full_consumer_drops_samples_not_work_and_preserves_fixed_cardinality() {
    let scheduler = scheduler();
    let (mut publisher, receiver) = telemetry::channel(1).unwrap();
    let now = Instant::now();
    assert_eq!(
        publisher.try_publish(&scheduler, now).unwrap(),
        PublishOutcome::Published
    );
    for _ in 0..1000 {
        assert_eq!(
            publisher.try_publish(&scheduler, now).unwrap(),
            PublishOutcome::ConsumerFull
        );
    }
    enqueue(&scheduler, Base, now);
    assert!(
        scheduler
            .dispatch(now)
            .unwrap()
            .unwrap()
            .finish(now)
            .unwrap()
            .is_ok()
    );
    let old_frame = receiver.try_recv().unwrap();
    assert_eq!(old_frame.stages.len(), 12);
    assert_eq!(old_frame.missed_samples, 0);
    assert!(receiver.try_recv().is_err());
    assert_eq!(
        publisher.try_publish(&scheduler, now).unwrap(),
        PublishOutcome::Published
    );
    let frame = receiver.try_recv().unwrap();
    assert_eq!(frame.missed_samples, 1000);
    assert_eq!(
        frame
            .stages
            .iter()
            .map(|s| s.counters.completed)
            .sum::<u64>(),
        1
    );
}

#[test]
fn dropped_receiver_does_not_fence_or_admit_any_work() {
    let scheduler = scheduler();
    let (mut publisher, receiver) = telemetry::channel(1).unwrap();
    drop(receiver);
    let now = Instant::now();
    assert_eq!(
        publisher.try_publish(&scheduler, now).unwrap(),
        PublishOutcome::ConsumerDisconnected
    );
    assert_eq!(publisher.missed_samples(), 1);
    enqueue(&scheduler, Solana, now);
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
fn consumer_owns_only_a_snapshot_and_cannot_hold_the_gate_lock() {
    let scheduler = scheduler();
    let now = Instant::now();
    enqueue(&scheduler, Solana, now);
    let (mut publisher, receiver) = telemetry::channel(1).unwrap();
    publisher
        .try_publish(&scheduler, now + Duration::from_millis(25))
        .unwrap();
    let retained = receiver.recv().unwrap();
    assert_eq!(scheduler.fence(Solana).unwrap(), 1);
    enqueue(&scheduler, Base, now);
    assert!(
        scheduler
            .dispatch(now)
            .unwrap()
            .unwrap()
            .finish(now)
            .unwrap()
            .is_ok()
    );
    let old = retained
        .stages
        .iter()
        .find(|s| s.network == Solana && s.stage == Stage::Quote)
        .unwrap();
    assert_eq!(old.queued, 1);
    assert_eq!(old.oldest_queue_age, Some(Duration::from_millis(25)));
    let fresh = scheduler.try_snapshot(now).unwrap().unwrap();
    let solana = fresh
        .iter()
        .find(|s| s.network == Solana && s.stage == Stage::Quote)
        .unwrap();
    assert_eq!(solana.queued, 0);
    assert_eq!(solana.oldest_queue_age, None);
    assert_eq!(solana.counters.dropped_for(DropReason::GateClosed), 1);
}

#[test]
fn blocked_solana_and_slow_telemetry_leave_base_capacity_available() {
    let scheduler = scheduler();
    let now = Instant::now();
    enqueue(&scheduler, Solana, now);
    let stalled = scheduler.dispatch(now).unwrap().unwrap();
    let (mut publisher, _receiver) = telemetry::channel(1).unwrap();
    publisher.try_publish(&scheduler, now).unwrap();
    for _ in 0..100 {
        assert_eq!(
            publisher.try_publish(&scheduler, now).unwrap(),
            PublishOutcome::ConsumerFull
        );
        enqueue(&scheduler, Base, now);
        let work = scheduler.dispatch(now).unwrap().unwrap();
        assert_eq!(work.item().network, Base);
        assert!(work.finish(now).unwrap().is_ok());
    }
    drop(stalled);
    assert_eq!(
        scheduler
            .snapshot(now)
            .unwrap()
            .iter()
            .map(|s| s.counters.completed)
            .sum::<u64>(),
        100
    );
}

#[test]
fn producer_drop_closes_consumer_after_bounded_pending_frames() {
    let scheduler = scheduler();
    let (mut publisher, receiver) = telemetry::channel(2).unwrap();
    publisher.try_publish(&scheduler, Instant::now()).unwrap();
    drop(publisher);
    assert_eq!(receiver.recv().unwrap().stages.len(), 12);
    assert!(receiver.recv().is_err());
}
