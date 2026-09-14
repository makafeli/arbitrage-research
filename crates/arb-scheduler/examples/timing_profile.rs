//! Explicit synthetic scheduler microprofile, not a market or production benchmark.
use arb_domain::NetworkId;
use arb_scheduler::{CorrelationId, Limits, Scheduler, Stage, WorkItem};
use std::time::{Duration, Instant};

fn optional(value: Option<u128>) -> String {
    value.map_or_else(|| "null".into(), |v| format!("\"{v}\""))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scheduler = Scheduler::new(Limits {
        queue_per_network: 2,
        in_flight_per_network: 1,
        global_in_flight: 2,
        stage_deadlines: [Duration::from_secs(5); 6],
    })?;
    let networks = [NetworkId::BaseMainnet, NetworkId::SolanaMainnet];
    let stages = [
        Stage::Ingestion,
        Stage::Snapshot,
        Stage::Quote,
        Stage::Simulation,
        Stage::Persistence,
        Stage::Api,
    ];
    for network in networks {
        scheduler.set_gate(network, Some(1_u64))?;
    }
    let started = Instant::now();
    for _ in 0..200 {
        for network in networks {
            for stage in stages {
                let now = Instant::now();
                assert!(
                    scheduler
                        .try_enqueue(
                            WorkItem {
                                network,
                                stage,
                                generation: 1,
                                correlation_id: CorrelationId::new("fixture:microprofile")?,
                                observed_at: now,
                                payload: 17_u64,
                            },
                            now
                        )?
                        .is_ok()
                );
                let permit = scheduler
                    .dispatch(Instant::now())?
                    .expect("admitted single item");
                // A deterministic CPU placeholder exercises the permit boundaries;
                // none of the named pipeline stages execute a real market operation.
                let mut value = permit.item().payload;
                for i in 0..64 {
                    value = std::hint::black_box(value.wrapping_mul(7).wrapping_add(i));
                }
                std::hint::black_box(value);
                assert!(permit.finish(Instant::now())?.is_ok());
            }
        }
    }
    let mut rows = Vec::new();
    for row in scheduler.snapshot(Instant::now())? {
        for (population, summary) in [
            ("queue_wait_dispatched", row.timing.queue_wait),
            ("successful_execution", row.timing.successful_execution),
        ] {
            assert_eq!(summary.samples, 200);
            rows.push(format!(
                "{{\"network_id\":\"{}\",\"stage\":\"{:?}\",\"population\":\"{}\",\"samples\":\"{}\",\"min_ns\":{},\"max_ns\":{},\"p50_upper_ns\":{},\"p95_upper_ns\":{},\"p99_upper_ns\":{}}}",
                row.network.as_str(), row.stage, population, summary.samples,
                optional(summary.min_ns), optional(summary.max_ns),
                optional(summary.p50_upper_ns), optional(summary.p95_upper_ns), optional(summary.p99_upper_ns)
            ));
        }
    }
    println!(
        "{{\"kind\":\"SYNTHETIC_SCHEDULER_MICROPROFILE\",\"clock\":\"std::time::Instant\",\"method\":\"cumulative-log2-nanoseconds-upper-bounds\",\"elapsed_ns\":\"{}\",\"production_or_market_performance_verified\":false,\"rows\":[{}]}}",
        started.elapsed().as_nanos(),
        rows.join(",")
    );
    Ok(())
}
