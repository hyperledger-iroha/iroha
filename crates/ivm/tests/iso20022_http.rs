//! Integration tests for ISO 20022 HTTP dispatch.

use std::{
    io::{self, BufRead, BufReader, ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::{Duration, Instant},
};

use ivm::iso20022::{msg_create, msg_send, msg_set};

const HTTP_TEST_TIMEOUT: Duration = Duration::from_secs(5);
const HTTP_TEST_POLL_INTERVAL: Duration = Duration::from_millis(10);

fn accept_http_test_stream(listener: &TcpListener) -> io::Result<TcpStream> {
    let started = Instant::now();
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                if started.elapsed() >= HTTP_TEST_TIMEOUT {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "timed out waiting for ISO 20022 HTTP client",
                    ));
                }
                thread::sleep(HTTP_TEST_POLL_INTERVAL);
            }
            Err(err) => return Err(err),
        }
    }
}

fn populate_pacs008_minimal() {
    msg_set("MsgId", b"1");
    msg_set("IntrBkSttlmCcy", b"USD");
    msg_set("IntrBkSttlmAmt", b"100");
    msg_set("IntrBkSttlmDt", b"2024-01-01");
    msg_set("DbtrAcct", b"GB82WEST12345698765432");
    msg_set("CdtrAcct", b"GB33BUKB20201555555555");
    msg_set("DbtrAgt", b"DEUTDEFF");
    msg_set("CdtrAgt", b"DEUTDEFF");
}

#[test]
fn msg_send_http_posts_payload_integration() {
    msg_create("pacs.008");
    populate_pacs008_minimal();

    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            // Some CI sandboxes disallow networking; skip rather than fail so
            // the suite still passes under those restrictions.
            eprintln!("skipping msg_send_http_posts_payload_integration: {err}");
            return;
        }
        Err(err) => panic!("listener: {err}"),
    };
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let mut stream = accept_http_test_stream(&listener).unwrap();
        stream.set_read_timeout(Some(HTTP_TEST_TIMEOUT)).unwrap();
        stream.set_write_timeout(Some(HTTP_TEST_TIMEOUT)).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut headers = String::new();
        let mut line = String::new();
        let mut content_len = 0usize;
        loop {
            line.clear();
            let read = reader.read_line(&mut line).unwrap();
            assert_ne!(read, 0, "HTTP client closed before header terminator");
            if line == "\r\n" {
                break;
            }
            if let Some((_, value)) = line
                .split_once(':')
                .filter(|_| line.to_ascii_lowercase().starts_with("content-length"))
            {
                content_len = value.trim().parse().unwrap_or(0);
            }
            headers.push_str(&line);
        }
        let mut body = vec![0u8; content_len];
        reader.read_exact(&mut body).unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
        (headers, body)
    });

    let url = format!("http://{addr}/submit");
    msg_send(&url).unwrap();
    let (headers, body) = handle.join().unwrap();
    assert!(headers.contains("POST /submit HTTP/1.1"));
    assert!(String::from_utf8_lossy(&body).contains("ISO20022"));
}
