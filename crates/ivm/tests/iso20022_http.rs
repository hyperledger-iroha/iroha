//! Integration tests for ISO 20022 HTTP dispatch.

use std::{
    io::{BufRead, BufReader, ErrorKind, Read, Write},
    net::TcpListener,
    thread,
};

use ivm::iso20022::{msg_create, msg_send, msg_set};

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
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut headers = String::new();
        let mut line = String::new();
        let mut content_len = 0usize;
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
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
