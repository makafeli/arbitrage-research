//! Optional lossy operational telemetry, independent of durable research records.
use arb_scheduler::{
    DropReason, Stage,
    telemetry::{self, Frame, Publisher},
};
use serde_json::json;
use std::{
    io::{self, Write},
    sync::mpsc::Receiver,
    time::Instant,
};

pub fn enabled() -> io::Result<bool> {
    match std::env::var("ARB_STAGE_METRICS_STDERR") {
        Err(std::env::VarError::NotPresent) => Ok(false),
        Ok(value) if value == "0" => Ok(false),
        Ok(value) if value == "1" => Ok(true),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ARB_STAGE_METRICS_STDERR must be 0 or 1",
        )),
    }
}

pub fn start(enabled: bool) -> io::Result<Option<Publisher>> {
    if !enabled {
        return Ok(None);
    }
    // One queued twelve-row frame plus the frame held by one dedicated writer.
    // Never consume Tokio's sole blocking slot with a slow operational sink.
    let (publisher, receiver) = telemetry::channel(1).map_err(io::Error::other)?;
    let thread = std::thread::Builder::new()
        .name("arb-stage-metrics".into())
        .spawn(move || {
            let _ = write_frames(receiver, io::stderr());
        })?;
    // A stopped producer closes the channel. A blocked OS write cannot be
    // cancelled; do not join it on the control/shutdown path. Process exit ends
    // any remaining writer. This never detaches evaluation or durable writes.
    drop(thread);
    Ok(Some(publisher))
}

fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Ingestion => "ingestion",
        Stage::Snapshot => "snapshot",
        Stage::Quote => "quote",
        Stage::Simulation => "simulation",
        Stage::Persistence => "persistence",
        Stage::Api => "api",
    }
}

/// Convert bounded operational timing to explicit string-valued nanoseconds.
fn latency_json(summary: &arb_scheduler::timing::LatencySummary) -> serde_json::Value {
    json!({
        "samples": summary.samples.to_string(), "saturated": summary.saturated,
        "min_ns": summary.min_ns.map(|v| v.to_string()),
        "max_ns": summary.max_ns.map(|v| v.to_string()),
        "p50_upper_ns": summary.p50_upper_ns.map(|v| v.to_string()),
        "p95_upper_ns": summary.p95_upper_ns.map(|v| v.to_string()),
        "p99_upper_ns": summary.p99_upper_ns.map(|v| v.to_string())
    })
}

fn write_frames(receiver: Receiver<Frame>, mut writer: impl Write) -> io::Result<()> {
    for frame in receiver {
        // Frame ownership contains no scheduler/control lock. Encoding and
        // potentially blocking I/O happen only on this dedicated writer.
        let stages: Vec<_> = frame
            .stages
            .iter()
            .map(|row| {
                let dropped: Vec<_> = DropReason::ALL
                    .into_iter()
                    .map(|reason| {
                        json!({"reason":format!("{reason:?}"),"count":row.counters.dropped_for(reason).to_string()})
                    })
                    .collect();
                json!({
                    "network_id":row.network.as_str(), "stage":stage_name(row.stage),
                    "queued":row.queued, "in_flight":row.in_flight,
                    "oldest_queue_age_ms":row.oldest_queue_age.map(|v|v.as_millis().to_string()),
                    "accepted":row.counters.accepted.to_string(),
                    "dispatched":row.counters.dispatched.to_string(),
                    "completed":row.counters.completed.to_string(), "dropped":dropped,
                    "timing": {
                        "method": "cumulative-log2-nanoseconds-upper-bounds",
                        "queue_wait_dispatched": latency_json(&row.timing.queue_wait),
                        "successful_execution": latency_json(&row.timing.successful_execution),
                        "rejected_execution": latency_json(&row.timing.rejected_execution),
                        "unmeasurable_completions": row.timing.unmeasurable_completions.to_string()
                    }
                })
            })
            .collect();
        let message = json!({
            "event":"scheduler-stage-metrics", "scope":"PROCESS_LOCAL_OPERATIONAL",
            "process_id":std::process::id(),
            "sample_age_ms":Instant::now().saturating_duration_since(frame.sampled_at).as_millis().to_string(),
            "missed_metric_samples":frame.missed_samples.to_string(), "stages":stages,
            "market_coverage_verified":false
        });
        writeln!(writer, "{message}")?;
        writer.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arb_domain::NetworkId::{BaseMainnet as Base, SolanaMainnet as Solana};
    use arb_scheduler::{CorrelationId, Limits, Scheduler, WorkItem, telemetry::PublishOutcome};
    use std::{sync::mpsc, thread, time::Duration};

    fn scheduler() -> Scheduler<u64, u64> {
        Scheduler::new(Limits {
            queue_per_network: 2,
            in_flight_per_network: 1,
            global_in_flight: 2,
            stage_deadlines: [Duration::from_secs(5); 6],
        })
        .unwrap()
    }

    #[test]
    fn disabled_output_creates_no_channel_or_thread() {
        assert!(start(false).unwrap().is_none());
    }

    #[test]
    fn output_preserves_absent_age_exact_counters_and_operational_provenance() {
        let scheduler = scheduler();
        let (mut publisher, receiver) = telemetry::channel(1).unwrap();
        publisher.try_publish(&scheduler, Instant::now()).unwrap();
        drop(publisher);
        let mut bytes = Vec::new();
        write_frames(receiver, &mut bytes).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["scope"], "PROCESS_LOCAL_OPERATIONAL");
        assert_eq!(value["market_coverage_verified"], false);
        assert_eq!(value["stages"].as_array().unwrap().len(), 12);
        assert!(value["stages"][0]["oldest_queue_age_ms"].is_null());
        assert_eq!(value["stages"][0]["accepted"], "0");
        assert_eq!(value["stages"][0]["dropped"].as_array().unwrap().len(), 7);
        let timing = &value["stages"][0]["timing"];
        assert_eq!(timing["queue_wait_dispatched"]["samples"], "0");
        assert!(timing["successful_execution"]["p50_upper_ns"].is_null());
        assert_eq!(timing["unmeasurable_completions"], "0");
    }

    #[test]
    fn nanosecond_summaries_preserve_values_beyond_javascript_integer_precision() {
        let summary = arb_scheduler::timing::LatencySummary {
            samples: u64::MAX,
            saturated: true,
            min_ns: Some(0),
            max_ns: Some(u128::from(u64::MAX)),
            p50_upper_ns: Some(9_007_199_254_740_993),
            p95_upper_ns: Some(u128::from(u64::MAX)),
            p99_upper_ns: Some(u128::from(u64::MAX)),
        };
        let value = latency_json(&summary);
        assert_eq!(value["samples"], u64::MAX.to_string());
        assert_eq!(value["p50_upper_ns"], "9007199254740993");
        assert_eq!(value["max_ns"], u64::MAX.to_string());
        assert_eq!(value["saturated"], true);
    }

    #[test]
    fn output_failure_disconnects_without_fencing_scheduler() {
        struct FailedWriter;
        impl Write for FailedWriter {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "test sink closed",
                ))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let scheduler = scheduler();
        let (mut publisher, receiver) = telemetry::channel(1).unwrap();
        publisher.try_publish(&scheduler, Instant::now()).unwrap();
        assert!(write_frames(receiver, FailedWriter).is_err());
        assert_eq!(
            publisher.try_publish(&scheduler, Instant::now()).unwrap(),
            PublishOutcome::ConsumerDisconnected
        );
        assert_eq!(scheduler.set_gate(Base, Some(7)).unwrap(), 0);
    }

    #[test]
    fn blocked_writer_does_not_block_scheduling_or_control_fences() {
        struct SlowWriter {
            entered: Option<mpsc::Sender<()>>,
            release: mpsc::Receiver<()>,
        }
        impl Write for SlowWriter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if let Some(entered) = self.entered.take() {
                    entered.send(()).unwrap();
                    self.release.recv().unwrap();
                }
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let scheduler = scheduler();
        let (mut publisher, receiver) = telemetry::channel(1).unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let writer = thread::spawn(move || {
            write_frames(
                receiver,
                SlowWriter {
                    entered: Some(entered_tx),
                    release: release_rx,
                },
            )
        });
        publisher.try_publish(&scheduler, Instant::now()).unwrap();
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        // The writer is blocked inside a real Write call throughout these steps.
        let exercise = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scheduler.set_gate(Base, Some(7)).unwrap();
            let now = Instant::now();
            assert!(
                scheduler
                    .try_enqueue(
                        WorkItem {
                            network: Base,
                            stage: Stage::Quote,
                            generation: 7,
                            correlation_id: CorrelationId::new("fixture:slow-sink").unwrap(),
                            observed_at: now,
                            payload: 1,
                        },
                        now
                    )
                    .unwrap()
                    .is_ok()
            );
            assert!(
                scheduler
                    .dispatch(now)
                    .unwrap()
                    .unwrap()
                    .finish(now)
                    .unwrap()
                    .is_ok()
            );
            assert_eq!(scheduler.fence(Solana).unwrap(), 0);
            assert_eq!(
                publisher.try_publish(&scheduler, now).unwrap(),
                PublishOutcome::Published
            );
            assert_eq!(
                publisher.try_publish(&scheduler, now).unwrap(),
                PublishOutcome::ConsumerFull
            );
        }));
        release_tx.send(()).unwrap();
        drop(publisher);
        writer.join().unwrap().unwrap();
        exercise.unwrap();
    }
}
