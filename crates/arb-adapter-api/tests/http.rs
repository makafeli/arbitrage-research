use arb_adapter_api::{HttpReadRpc, ReadMethod, ReadRpc};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};
fn server(status: &str, body: String, extra_headers: &str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let status = status.to_string();
    let extra = extra_headers.to_string();
    let handle = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        let mut bytes = [0; 1024];
        loop {
            let n = socket.read(&mut bytes).unwrap();
            if n == 0 {
                break;
            }
            request.extend_from_slice(&bytes[..n]);
            if let Some(boundary) = request.windows(4).position(|b| b == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&request[..boundary]).to_ascii_lowercase();
                let length: usize = header
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .unwrap()
                    .parse()
                    .unwrap();
                if request.len() >= boundary + 4 + length {
                    break;
                }
            }
        }
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).unwrap();
        String::from_utf8(request).unwrap()
    });
    (endpoint, handle)
}
#[test]
fn request_is_read_only_and_exact_success_body_is_retained() {
    let body = "{\n\"jsonrpc\":\"2.0\",\"id\":0,\"result\":\"0x2105\"\n}".to_owned();
    let (endpoint, server) = server("200 OK", body.clone(), "");
    let mut rpc = HttpReadRpc::new(&endpoint, Duration::from_secs(2), 1024, 1).unwrap();
    assert_eq!(
        rpc.call(ReadMethod::EthChainId, json!([])).unwrap(),
        "0x2105"
    );
    let records = rpc.into_records();
    assert_eq!(records[0].response, body);
    let request = server.join().unwrap();
    assert!(request.contains("eth_chainId"));
    assert!(!request.contains("sendTransaction"));
}
#[test]
fn rpc_errors_do_not_leak_provider_body_and_consume_quota() {
    let body=json!({"jsonrpc":"2.0","id":0,"error":{"message":"https://provider.invalid/DO_NOT_LOG_API_KEY"}}).to_string();
    let (endpoint, server) = server("200 OK", body, "");
    let mut rpc = HttpReadRpc::new(&endpoint, Duration::from_secs(2), 1024, 1).unwrap();
    let error = rpc.call(ReadMethod::EthChainId, json!([])).unwrap_err();
    assert!(!error.to_string().contains("DO_NOT_LOG"));
    assert!(
        rpc.call(ReadMethod::EthChainId, json!([]))
            .unwrap_err()
            .0
            .contains("quota")
    );
    assert!(rpc.into_records().is_empty());
    server.join().unwrap();
}
#[test]
fn oversized_response_rejected_before_retention() {
    let (endpoint, server) = server("200 OK", "x".repeat(4096), "");
    let mut rpc = HttpReadRpc::new(&endpoint, Duration::from_secs(2), 1024, 1).unwrap();
    assert!(
        rpc.call(ReadMethod::EthChainId, json!([]))
            .unwrap_err()
            .0
            .contains("byte quota")
    );
    assert!(rpc.into_records().is_empty());
    server.join().unwrap();
}
#[test]
fn redirects_and_wrong_ids_fail_closed() {
    let (endpoint, server) = server(
        "302 Found",
        String::new(),
        "Location: http://127.0.0.1:1/secret\r\n",
    );
    let mut rpc = HttpReadRpc::new(&endpoint, Duration::from_secs(2), 1024, 1).unwrap();
    assert!(
        rpc.call(ReadMethod::EthChainId, json!([]))
            .unwrap_err()
            .0
            .contains("HTTP error")
    );
    server.join().unwrap();
    let (endpoint, server) = self::server(
        "200 OK",
        json!({"jsonrpc":"2.0","id":99,"result":"0x2105"}).to_string(),
        "",
    );
    let mut rpc = HttpReadRpc::new(&endpoint, Duration::from_secs(2), 1024, 1).unwrap();
    assert!(rpc.call(ReadMethod::EthChainId, json!([])).is_err());
    server.join().unwrap();
}
