use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!(
        "EVM worker is not implemented. Base adapters, persistent control and paper execution are planned."
    );
    eprintln!("No RPC connection, wallet access, signing or broadcast was attempted.");
    ExitCode::from(2)
}
