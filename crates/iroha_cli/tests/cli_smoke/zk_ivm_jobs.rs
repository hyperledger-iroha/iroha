//! Exercise proof-job polling through the actual CLI and a bounded local HTTP fixture.

use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

use norito::json::{self, Value};

use super::{
    command,
    torii_mock_support::{TempDir, write_client_config},
};

fn read_http_line(reader: &mut impl BufRead, remaining: &mut usize) -> String {
    let mut line = String::new();
    (&mut *reader)
        .take(*remaining as u64)
        .read_line(&mut line)
        .expect("bounded HTTP header line");
    assert!(line.ends_with("\r\n"), "truncated or oversized HTTP header");
    *remaining -= line.len();
    line
}

fn serve_job(
    listener: TcpListener,
    expected_request: Value,
    terminal_response: Value,
) -> thread::JoinHandle<()> {
    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        let responses = [
            norito::json!({"job_id": "job-7"}),
            norito::json!({"job_id": "job-7", "status": "pending"}),
            norito::json!({"job_id": "job-7", "status": "running"}),
            terminal_response,
        ];
        for (index, response) in responses.into_iter().enumerate() {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "missing HTTP request {index}");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accept HTTP request: {error}"),
                }
            };
            stream
                .set_nonblocking(false)
                .expect("blocking accepted connection");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout");
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .expect("write timeout");
            let mut reader = BufReader::new(&mut stream);
            let mut remaining_header_bytes = 65_536;
            let request_line = read_http_line(&mut reader, &mut remaining_header_bytes);
            let expected_line = if index == 0 {
                "POST /v1/zk/ivm/prove HTTP/1.1\r\n"
            } else {
                "GET /v1/zk/ivm/prove/job-7 HTTP/1.1\r\n"
            };
            assert_eq!(request_line, expected_line);
            let mut content_length = 0;
            loop {
                let line = read_http_line(&mut reader, &mut remaining_header_bytes);
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    content_length = value.trim().parse::<usize>().expect("content length");
                }
            }
            assert!(content_length <= 4_096, "oversized fixture request body");
            let mut body = vec![0; content_length];
            reader.read_exact(&mut body).expect("request body");
            if index == 0 {
                assert_eq!(
                    json::from_slice::<Value>(&body).expect("posted request JSON"),
                    expected_request
                );
            } else {
                assert!(body.is_empty(), "poll must not send a request body");
            }
            let body = json::to_vec(&response).expect("response JSON");
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .expect("response headers");
            stream.write_all(&body).expect("response body");
            stream.flush().expect("flush response");
        }
    })
}

#[test]
fn proof_job_polling_preserves_terminal_json_and_exit_status() {
    let cases = [
        (
            norito::json!({"job_id": "job-7", "status": "done", "proved": {"receipt": "public"}}),
            None,
            true,
        ),
        (
            norito::json!({"job_id": "job-7", "status": "error", "error": "invalid witness"}),
            Some("IVM prove job job-7 failed: invalid witness"),
            true,
        ),
        (
            norito::json!({"job_id": "another-job", "status": "done"}),
            Some("prove response does not identify requested job job-7"),
            false,
        ),
        (
            norito::json!({"job_id": "job-7", "status": "unknown"}),
            Some("unexpected job status `unknown` for job job-7"),
            false,
        ),
    ];
    for (terminal, expected_error, should_print) in cases {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("local HTTP fixture");
        let base_url = format!("http://{}", listener.local_addr().expect("fixture address"));
        let temp = TempDir::new("zk_ivm_job_http").expect("temporary directory");
        let config = temp.path().join("client.toml");
        write_client_config(&config, &base_url).expect("fixture client config");
        let request = norito::json!({
            "vk_ref": {"backend": "halo2/ipa", "name": "replay_binding"},
            "metadata": {}
        });
        let input = temp.path().join("request.json");
        fs::write(&input, json::to_vec(&request).expect("request JSON")).expect("request file");
        let server = serve_job(listener, request, terminal.clone());
        let output = command()
            .args(["--output-format", "json", "--machine"])
            .arg("--config")
            .arg(&config)
            .args(["app", "zk", "ivm", "prove", "--json"])
            .arg(&input)
            .args(["--wait", "--poll-interval-ms", "10", "--timeout-secs", "10"])
            .output();
        server.join().expect("HTTP fixture completed");
        let output = output.expect("CLI proof-job command");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(if expected_error.is_none() { 0 } else { 1 }),
            "{stderr}"
        );
        if let Some(expected) = expected_error {
            assert!(stderr.contains(expected), "unexpected CLI error: {stderr}");
        }
        if should_print {
            assert_eq!(
                json::from_slice::<Value>(&output.stdout).expect("one terminal JSON response"),
                terminal,
                "intermediate responses must not pollute terminal JSON"
            );
        } else {
            assert!(
                output.stdout.is_empty(),
                "untrusted terminal response printed"
            );
        }
    }
}
