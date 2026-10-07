//! Contract tests against a local TCP server; no credentials or external network.
use lingo::{config::Config, provider};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

fn fixture_response(content: Value, finish_reason: &str) -> String {
    json!({"choices": [{"message": {"content": content}, "finish_reason": finish_reason}]})
        .to_string()
}

fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut bytes = Vec::new();
    loop {
        let mut buffer = [0u8; 4096];
        let read = stream.read(&mut buffer).unwrap();
        assert!(read > 0, "request ended before its body");
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(split) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..split]).to_lowercase();
            let length: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            if bytes.len() >= split + 4 + length {
                return String::from_utf8(bytes).unwrap();
            }
        }
    }
}

fn serve(
    status: u16,
    body: String,
    extra_headers: &str,
    delay: Duration,
) -> (Config, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let extra_headers = extra_headers.to_string();
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_request(&mut stream);
        if !delay.is_zero() {
            thread::sleep(delay);
        }
        let response = format!(
            "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra_headers}\r\n{body}",
            body.len()
        );
        // A timeout test can close the connection before the server writes.
        let _ = stream.write_all(response.as_bytes());
        request
    });
    let mut config = Config::default();
    config.provider.base_url = format!("http://{address}/v1");
    config.provider.model = "local-compatible-model".into();
    config.target_language = "Japanese".into();
    (config, task)
}

#[test]
fn sends_openai_contract_and_preserves_multiline_output() {
    let output = "こんにちは。\n\n- `customer_id`: 42\n";
    let (config, server) = serve(
        200,
        fixture_response(json!(output), "stop"),
        "",
        Duration::ZERO,
    );
    let source = "你好。\n\n- `customer_id`: 42\n";
    assert_eq!(
        provider::translate(&config, "fixture-api-key", source).unwrap(),
        output
    );
    let request = server.join().unwrap();
    assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
    assert!(
        request
            .to_lowercase()
            .contains("authorization: bearer fixture-api-key\r\n")
    );
    let (_, body) = request.split_once("\r\n\r\n").unwrap();
    let body: Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["model"], "local-compatible-model");
    assert_eq!(body["stream"], false);
    assert!(
        body["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains(&json!({"source_text": source}).to_string())
    );
    assert!(
        body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("Japanese")
    );
    assert!(body.get("thinking").is_none());
    assert!(!body.to_string().contains("fixture-api-key"));
}

#[test]
fn rejects_incomplete_empty_refused_and_malformed_responses() {
    let fixtures = [
        fixture_response(json!("partial translation"), "length"),
        fixture_response(json!("   \n"), "stop"),
        fixture_response(Value::Null, "stop"),
        fixture_response(json!("filtered"), "content_filter"),
        fixture_response(json!("function call"), "tool_calls"),
        json!({"choices": []}).to_string(),
        json!({"choices": [{"message": {"content": "text"}}]}).to_string(),
        json!({"choices": [{"message": {"content": "text", "refusal": "No."}, "finish_reason": "stop"}]}).to_string(),
        "this is invalid JSON with sensitive-source-text and fixture-api-key".into(),
    ];
    for body in fixtures {
        let (config, server) = serve(200, body, "", Duration::ZERO);
        let error = provider::translate(&config, "fixture-api-key", "sensitive-source-text")
            .unwrap_err()
            .to_string();
        assert!(!error.contains("sensitive-source-text"));
        assert!(!error.contains("fixture-api-key"));
        server.join().unwrap();
    }
}

#[test]
fn errors_never_echo_provider_bodies_source_or_key() {
    for status in [400, 401, 402, 403, 404, 413, 429, 500, 503] {
        let (config, server) = serve(
            status,
            "sensitive-source-text fixture-api-key".into(),
            "",
            Duration::ZERO,
        );
        let error = provider::translate(&config, "fixture-api-key", "sensitive-source-text")
            .unwrap_err()
            .to_string();
        assert!(!error.contains("sensitive-source-text"));
        assert!(!error.contains("fixture-api-key"));
        server.join().unwrap();
    }
}

#[test]
fn refuses_redirect_instead_of_forwarding_credentials() {
    let (config, server) = serve(
        307,
        String::new(),
        "Location: http://127.0.0.1:1/stolen\r\n",
        Duration::ZERO,
    );
    let error = provider::translate(&config, "fixture-api-key", "hello")
        .unwrap_err()
        .to_string();
    assert!(error.contains("redirected"));
    server.join().unwrap();
}

#[test]
fn rejects_oversized_response_and_input() {
    let body = fixture_response(json!("x".repeat(1_048_577)), "stop");
    let (config, server) = serve(200, body, "", Duration::ZERO);
    let error = provider::translate(&config, "fixture-api-key", "hello")
        .unwrap_err()
        .to_string();
    assert!(error.contains("size limit"));
    server.join().unwrap();
    // No listener: validation must happen before a request is made.
    let mut config = Config::default();
    config.provider.base_url = "http://127.0.0.1:1/v1".into();
    assert!(
        provider::translate(&config, "fixture-api-key", &"a".repeat(20_001))
            .unwrap_err()
            .to_string()
            .contains("shorter passage")
    );
    assert!(
        provider::translate(&config, "fixture-api-key", " \n")
            .unwrap_err()
            .to_string()
            .contains("Select some text")
    );
}

#[test]
fn times_out_without_leaking_diagnostics() {
    let (mut config, server) = serve(
        200,
        fixture_response(json!("hello"), "stop"),
        "",
        Duration::from_millis(5400),
    );
    config.provider.timeout_secs = 5;
    let start = Instant::now();
    let error = provider::translate(&config, "fixture-api-key", "sensitive-source-text")
        .unwrap_err()
        .to_string();
    assert!(error.contains("timed out"));
    assert!(start.elapsed() < Duration::from_secs(7));
    assert!(!error.contains("fixture-api-key"));
    assert!(!error.contains("sensitive-source-text"));
    server.join().unwrap();
}

/// Explicit opt-in only; reads an already exported key, never auto-loads `.env`.
#[test]
#[ignore = "makes a real, billable DeepSeek request; requires DEEPSEEK_API_KEY"]
fn live_deepseek_flash_translation() {
    let config = Config::default();
    let key = std::env::var("DEEPSEEK_API_KEY")
        .expect("Export DEEPSEEK_API_KEY explicitly for this test");
    let output = provider::translate(&config, &key, "你好，世界！").unwrap();
    assert!(output.to_lowercase().contains("hello"));
    assert!(output.to_lowercase().contains("world"));
}
