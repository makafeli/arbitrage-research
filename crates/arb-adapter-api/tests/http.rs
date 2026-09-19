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
fn block_time_uses_allowlisted_wire_method_and_retains_exact_success_or_null_body() {
    for result in [json!(1_700_000_000), serde_json::Value::Null] {
        let body = format!("{{\n\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{result}\n}}");
        let (endpoint, server) = server("200 OK", body.clone(), "");
        let mut rpc = HttpReadRpc::new(&endpoint, Duration::from_secs(2), 1024, 1).unwrap();
        assert_eq!(
            rpc.call(ReadMethod::GetBlockTime, json!([987_654]))
                .unwrap(),
            result
        );
        let records = rpc.into_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].method, ReadMethod::GetBlockTime);
        assert_eq!(records[0].params, json!([987_654]));
        assert_eq!(records[0].response, body);
        let request = server.join().unwrap();
        let request: serde_json::Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(request["method"], "getBlockTime");
        assert_eq!(request["params"], json!([987_654]));
    }
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

#[test]
fn status_classification_is_fixed_and_never_retains_error_bodies() {
    for (status, expected) in [
        ("400 Bad Request", "bad request"),
        ("402 Payment Required", "payment required"),
        ("404 Not Found", "not found"),
        ("408 Request Timeout", "request timeout"),
        ("429 Too Many Requests", "rate limited"),
        ("403 Forbidden", "access refused"),
        ("503 Service Unavailable", "provider server failure"),
    ] {
        let (endpoint, server) = server(status, "PRIVATE_ERROR_BODY".into(), "");
        let mut rpc = HttpReadRpc::new(&endpoint, Duration::from_secs(2), 1024, 1).unwrap();
        let error = rpc.call(ReadMethod::EthChainId, json!([])).unwrap_err();
        assert!(error.0.contains(expected));
        assert!(!error.0.contains("PRIVATE_ERROR_BODY"));
        assert!(rpc.into_records().is_empty());
        server.join().unwrap();
    }
}

#[test]
fn block_hash_log_request_uses_the_read_only_transport_and_replays_exactly() {
    let body = "{\n\"jsonrpc\":\"2.0\",\"id\":0,\"result\":[]\n}".to_owned();
    let (endpoint, server) = server("200 OK", body.clone(), "");
    let params = json!([{"blockHash":format!("0x{}", "a".repeat(64)),
                        "address":["0x0303030303030303030303030303030303030303"]}]);
    let mut rpc = HttpReadRpc::new(&endpoint, Duration::from_secs(2), 1024, 1).unwrap();
    assert_eq!(
        rpc.call(ReadMethod::EthGetLogs, params.clone()).unwrap(),
        json!([])
    );
    let records = rpc.into_records();
    assert_eq!(records[0].response, body);
    let request = server.join().unwrap();
    let wire: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(wire["method"], "eth_getLogs");
    assert_eq!(wire["params"], params);
    assert!(wire["params"][0].get("fromBlock").is_none());
    assert!(wire["params"][0].get("toBlock").is_none());
    let serialized = serde_json::to_string(&records).unwrap();
    let restored = serde_json::from_str(&serialized).unwrap();
    let mut replay = arb_adapter_api::TranscriptRpc::new(restored);
    assert_eq!(
        replay.call(ReadMethod::EthGetLogs, params).unwrap(),
        json!([])
    );
    replay.finish().unwrap();
}

#[test]
fn node_filter_methods_use_existing_bounded_http_and_exact_transcripts() {
    for (method, params, result) in [
        (ReadMethod::EthNewBlockFilter, json!([]), json!("0x1")),
        (
            ReadMethod::EthNewFilter,
            json!([{"address":["0x0000000000000000000000000000000000000001"]}]),
            json!("0x2"),
        ),
        (ReadMethod::EthGetFilterChanges, json!(["0x2"]), json!([])),
        (ReadMethod::EthUninstallFilter, json!(["0x2"]), json!(true)),
    ] {
        let body = json!({"jsonrpc":"2.0","id":0,"result":result}).to_string();
        let (endpoint, server) = server("200 OK", body, "");
        let mut rpc = HttpReadRpc::new(&endpoint, Duration::from_secs(2), 4096, 1).unwrap();
        assert_eq!(rpc.call(method, params.clone()).unwrap(), result);
        let request = server.join().unwrap();
        assert!(request.contains(method.wire_name()));
        let mut replay = arb_adapter_api::TranscriptRpc::new(rpc.into_records());
        assert_eq!(replay.call(method, params).unwrap(), result);
        replay.finish().unwrap();
    }
}
