//! Bounded loopback provider with an explicit durable RESEARCH-attempt handshake.
//! This file is test support; it changes no production HTTP timeout or protocol.
use arb_adapter_api::RpcRecord;
use arb_domain::NetworkId;
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    process::Child,
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}},
    time::{Duration, Instant},
};

pub const HOLD_LIMIT: Duration = Duration::from_secs(4);
pub const ASSERTION_WINDOW: Duration = Duration::from_millis(3500);

#[derive(Default)]
pub struct Handshake {
    pub entered: AtomicBool,
    pub release: AtomicBool,
    pub held: Mutex<Option<(String, Instant)>>,
    pub active: Mutex<Option<String>>,
}

#[derive(Default)]
pub struct Shutdown(AtomicBool);

pub struct ChildGuard {
    pub process: Child,
    pub shutdown: Arc<Shutdown>,
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        // Signal the server before closing a socket mid-request, including on
        // unwinding. Unexpected EOF during the live experiment still fails.
        self.shutdown.0.store(true, Ordering::SeqCst);
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

fn read_line(reader: &mut impl BufRead, line: &mut String, shutdown: &Shutdown) -> bool {
    match reader.read_line(line) {
        Ok(n) if n > 0 => true,
        other => {
            assert!(shutdown.0.load(Ordering::SeqCst), "unexpected live request EOF/read error: {other:?}");
            false
        }
    }
}

pub struct IsolatedRpc {
    pub endpoint: String,
    pub shutdown: Arc<Shutdown>,
    server: Option<std::thread::JoinHandle<()>>,
}
impl IsolatedRpc {
    pub fn start(network: NetworkId, blocked: bool, handshake: Arc<Handshake>,
        database: sqlx::PgPool, session: String) -> Self {
        let records: Vec<RpcRecord> = serde_json::from_str(match network {
            NetworkId::BaseMainnet => include_str!("../../../../crates/arb-evm/tests/fixtures/batch-rpc.json"),
            NetworkId::SolanaMainnet => include_str!("../../../../crates/arb-solana/tests/fixtures/batch-rpc.json"),
        }).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let shutdown = Arc::new(Shutdown::default());
        let finished = Arc::clone(&shutdown);
        let runtime = tokio::runtime::Handle::current();
        let server = std::thread::spawn(move || {
            let mut cursor = 0;
            let mut research_handled = false;
            while !finished.0.load(Ordering::SeqCst) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                };
                stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if !read_line(&mut reader, &mut line, &finished) { return; }
                assert!(line.starts_with("POST "));
                let mut length = None;
                let mut header_bytes = line.len();
                loop {
                    line.clear();
                    if !read_line(&mut reader, &mut line, &finished) { return; }
                    header_bytes += line.len();
                    assert!(header_bytes <= 16_384);
                    if line == "\r\n" { break; }
                    if let Some((key, value)) = line.split_once(':')
                        && key.eq_ignore_ascii_case("content-length") {
                        assert!(length.is_none());
                        length = Some(value.trim().parse::<usize>().unwrap());
                    }
                }
                let length = length.unwrap();
                assert!(length <= 65_536);
                let mut body = vec![0; length];
                if let Err(error) = reader.read_exact(&mut body) {
                    assert!(finished.0.load(Ordering::SeqCst), "unexpected live body read error: {error}");
                    return;
                }
                let request: Value = serde_json::from_slice(&body).unwrap();
                let record = &records[cursor];
                assert_eq!(request["method"], record.method.wire_name());
                assert_eq!(request["params"], record.params);
                assert_eq!(request["id"], record.sequence);
                if cursor == 0 && !research_handled {
                    // The worker durably creates one attempt before serial RPC.
                    // At its first request, bind this socket to that exact attempt,
                    // not to a fixture position, batch count or later DB query.
                    let pending: Vec<(String, String)> = runtime.block_on(async {
                        tokio::time::timeout(Duration::from_secs(1),
                            sqlx::query_as("SELECT attempt_id,purpose FROM collection_attempts WHERE session_id=$1 AND outcome='IN_PROGRESS'")
                                .bind(&session).fetch_all(&database)
                        ).await.unwrap().unwrap()
                    });
                    assert_eq!(pending.len(), 1, "request must have exactly one durable pending attempt");
                    let (attempt, purpose) = &pending[0];
                    if purpose == "RESEARCH" {
                        research_handled = true;
                        if blocked {
                            let started = Instant::now();
                            *handshake.held.lock().unwrap() = Some((attempt.clone(), started));
                            handshake.entered.store(true, Ordering::SeqCst);
                            while !handshake.release.load(Ordering::SeqCst) && !finished.0.load(Ordering::SeqCst) {
                                assert!(started.elapsed() < HOLD_LIMIT, "held experiment exceeded four seconds, below the real five-second RPC timeout");
                                std::thread::sleep(Duration::from_millis(2));
                            }
                        } else {
                            // Make the active decision causally later than the held
                            // RESEARCH request. Never count an earlier readiness result.
                            let deadline = Instant::now() + Duration::from_secs(3);
                            while !handshake.entered.load(Ordering::SeqCst) && !finished.0.load(Ordering::SeqCst) {
                                assert!(Instant::now() < deadline, "other research request did not reach the handshake");
                                std::thread::sleep(Duration::from_millis(2));
                            }
                            *handshake.active.lock().unwrap() = Some(attempt.clone());
                        }
                    } else {
                        assert_eq!(purpose, "READINESS");
                    }
                }
                if finished.0.load(Ordering::SeqCst) { break; }
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", record.response.len(), record.response);
                if let Err(error) = stream.write_all(response.as_bytes()) {
                    assert!(finished.0.load(Ordering::SeqCst), "unexpected live provider disconnect: {error}");
                    break;
                }
                cursor = (cursor + 1) % records.len();
            }
        });
        Self { endpoint, shutdown, server: Some(server) }
    }
}
impl Drop for IsolatedRpc {
    fn drop(&mut self) {
        self.shutdown.0.store(true, Ordering::SeqCst);
        if let Some(server) = self.server.take() {
            if std::thread::panicking() { let _ = server.join(); }
            else { server.join().unwrap(); }
        }
    }
}

#[test]
fn eof_is_expected_only_after_explicit_shutdown() {
    let stopped = Shutdown(AtomicBool::new(true));
    assert!(!read_line(&mut std::io::Cursor::new(b""), &mut String::new(), &stopped));
    let live = Shutdown::default();
    assert!(std::panic::catch_unwind(|| {
        read_line(&mut std::io::Cursor::new(b""), &mut String::new(), &live);
    }).is_err());
    let mut request = std::io::Cursor::new(b"POST / HTTP/1.1\r\n");
    let mut line = String::new();
    assert!(read_line(&mut request, &mut line, &live));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        read_line(&mut request, &mut String::new(), &live);
    })).is_err());
    assert!(!read_line(&mut request, &mut String::new(), &stopped));
}
