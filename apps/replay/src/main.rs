use arb_domain::{Action, Mode, Session};
use std::{error::Error, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 1 || args[0] != "--lifecycle-demo" {
        eprintln!("Recorded-market replay is not implemented.");
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
