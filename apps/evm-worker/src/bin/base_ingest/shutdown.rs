//! Process stop signals close the same ingestion admission/result fence.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::task::JoinHandle;

/// Install both Unix listeners synchronously before any provider work begins.
/// An unavailable signal handler fails startup rather than silently running
/// without the requested cancellation boundary. A closed signal stream also
/// cancels; it is never interpreted as permission to continue collection.
#[cfg(unix)]
pub(super) fn install(cancelled: Arc<AtomicBool>) -> Result<JoinHandle<()>, &'static str> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut interrupt = signal(SignalKind::interrupt()).map_err(|_| "STOP_SIGNAL_UNAVAILABLE")?;
    let mut terminate = signal(SignalKind::terminate()).map_err(|_| "STOP_SIGNAL_UNAVAILABLE")?;
    Ok(tokio::spawn(async move {
        tokio::select! {
            _ = interrupt.recv() => {},
            _ = terminate.recv() => {},
        }
        cancelled.store(true, Ordering::SeqCst);
    }))
}

/// Non-Unix builds retain Ctrl+C. A listener error cancels instead of leaving
/// an ingestion process running after its stop-listener task has exited.
#[cfg(not(unix))]
pub(super) fn install(cancelled: Arc<AtomicBool>) -> Result<JoinHandle<()>, &'static str> {
    Ok(tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        cancelled.store(true, Ordering::SeqCst);
    }))
}
