//! Offline litesvm harness for research-only Solana plans (ARB-029, issue #43).
//!
//! This crate exists so that `arb_solana::plan::SolanaPlan` can be executed
//! atomically inside an in-process SVM and shown to fail as a whole when its
//! final-balance guard trips. The library surface is intentionally empty:
//! the harness lives in `tests/`, every keypair it uses is created in memory
//! for one test and dropped, and nothing here talks to an RPC endpoint or
//! produces a transaction that could be submitted anywhere.
