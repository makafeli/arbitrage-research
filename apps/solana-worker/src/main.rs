use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!(
        "Solana worker is not implemented. Account ingestion, venue adapters and paper execution are planned."
    );
    eprintln!("No RPC connection, wallet access, signing or broadcast was attempted.");
    ExitCode::from(2)
}
