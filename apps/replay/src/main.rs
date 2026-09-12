use arb_domain::{Action, Mode, Session};
use std::{error::Error, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 4 && args[0] == "--verify-capture" && args[2] == "--manifest-digest" {
        let now = match std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        {
            Some(now) => now,
            None => {
                eprintln!("System clock unavailable");
                return ExitCode::FAILURE;
            }
        };
        return match replay::verify_capture(std::path::Path::new(&args[1]), &args[3], now) {
            Ok(mut report) => {
                match std::env::current_exe()
                    .ok()
                    .and_then(|path| arb_capture::file_digest(&path).ok())
                {
                    Some(digest) => report["replay_build_digest"] = serde_json::json!(digest),
                    None => {
                        eprintln!("Replay build identity unavailable");
                        return ExitCode::FAILURE;
                    }
                }
                println!("{report}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("Capture verification failed: {error}");
                ExitCode::FAILURE
            }
        };
    }
    if args.len() == 2 && args[0] == "--evaluate-captures" {
        return match evaluate_file(&args[1]) {
            Ok(report) => {
                println!("{report}");
                ExitCode::SUCCESS
            }
            Err(_) => {
                eprintln!(
                    "Replay evaluation failed: invalid request, unavailable retained inputs or incompatible evidence"
                );
                ExitCode::FAILURE
            }
        };
    }
    if args.len() != 1 || args[0] != "--lifecycle-demo" {
        eprintln!("Usage: replay --verify-capture DIRECTORY --manifest-digest sha256:HASH");
        eprintln!("       replay --evaluate-captures REQUEST.json");
        eprintln!(
            "Research arithmetic replay produces CANDIDATE evidence only; paper-outcome replay is unavailable."
        );
        eprintln!("Use --lifecycle-demo for an offline synthetic control-state example.");
        return ExitCode::from(2);
    }
    match lifecycle_demo() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Lifecycle example failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn lifecycle_demo() -> Result<(), Box<dyn Error>> {
    println!("SYNTHETIC LIFECYCLE DEMO — no market dataset, PnL or trading");
    let session = Session::new_research(Mode::Replay)?.complete_recovery()?;
    let session = session.request(Action::Start, 0)?.acknowledge(1, true)?;
    let session = session.record_attempt()?;
    let session = session.request(Action::Stop, 1)?;
    println!(
        "Accepted STOP: {:?}, worker {:?}",
        session.command_progress(),
        session.state()
    );
    let session = session.begin_fence(2)?.acknowledge(2, false)?;
    println!(
        "Applied STOP: {:?}, unresolved {}",
        session.state(),
        session.outstanding()
    );
    let session = session.resolve_attempt()?;
    println!("After synthetic resolution: {:?}", session.state());
    Ok(())
}

fn evaluate_file(path: &str) -> Result<serde_json::Value, Box<dyn Error>> {
    use std::io::Read;
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() || meta.len() > 1024 * 1024 {
        return Err("request must be a regular file at most 1 MiB".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("request exceeds byte bound".into());
    }
    let request: replay::ReplayEvaluationRequest = serde_json::from_slice(&bytes)?;
    let now = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
    )?;
    let mut report = replay::evaluate_captures(&request, now)?;
    report["replay_build_digest"] =
        serde_json::json!(arb_capture::file_digest(&std::env::current_exe()?)?);
    Ok(report)
}
