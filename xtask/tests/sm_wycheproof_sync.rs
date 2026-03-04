#![allow(clippy::unwrap_used)]

use std::{
    io::{ErrorKind, Read, Write},
    net::TcpListener,
    thread,
};

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use tempfile::TempDir;

fn sample_wycheproof() -> String {
    let key = format!("04{}{}", "11".repeat(32), "22".repeat(32));
    format!(
        r#"{{
            "algorithm": "SM2",
            "generatorVersion": "wycheproof-sm2-fixture-test",
            "numberOfTests": 999,
            "testGroups": [
                {{
                    "distid": "device:test",
                    "groupId": 7,
                    "key": {{
                        "uncompressed": "{key}"
                    }},
                    "comment": "group comment",
                    "tests": [
                        {{
                            "tcId": 1,
                            "comment": "ok",
                            "flags": ["RandomFlag"],
                            "msg": "",
                            "sig": "3006020100020100",
                            "result": "valid",
                            "unused": true
                        }}
                    ],
                    "unused_group_field": 42
                }}
            ]
        }}"#
    )
}

#[test]
fn sm_wycheproof_sync_from_file() {
    let dir = TempDir::new().expect("temp dir");
    let input_path = dir.path().join("source.json");
    std::fs::write(&input_path, sample_wycheproof()).expect("write sample");

    let output_path = dir.path().join("output.json");
    cargo_bin_cmd!("xtask")
        .args([
            "sm-wycheproof-sync",
            "--input",
            input_path.to_str().expect("utf8 path"),
            "--output",
            output_path.to_str().expect("utf8 path"),
            "--minify",
        ])
        .assert()
        .success();

    let sanitized = std::fs::read_to_string(&output_path).expect("read sanitized file");
    assert_fixture_shape(&sanitized);
}

#[test]
fn sm_wycheproof_sync_from_url() {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            eprintln!("sm_wycheproof_sync_from_url skipped: cannot bind loopback listener ({err})");
            return;
        }
        Err(err) => panic!("bind HTTP listener: {err}"),
    };
    let addr = listener.local_addr().expect("obtain addr");
    let body = sample_wycheproof();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write http response");
        }
    });

    let dir = TempDir::new().expect("temp dir");
    let output_path = dir.path().join("out.json");

    cargo_bin_cmd!("xtask")
        .args([
            "sm-wycheproof-sync",
            "--input-url",
            &format!("http://{addr}/"),
            "--output",
            output_path.to_str().expect("utf8 path"),
        ])
        .assert()
        .success();

    let sanitized = std::fs::read_to_string(&output_path).expect("read sanitized file");
    assert_fixture_shape(&sanitized);
}

fn assert_fixture_shape(raw: &str) {
    let value: Value = json::from_str(raw).expect("parse sanitized json");
    assert_eq!(
        value["algorithm"].as_str(),
        Some("SM2"),
        "algorithm should be preserved"
    );
    assert_eq!(
        value["numberOfTests"].as_u64(),
        Some(1),
        "expected sanitized test count"
    );
    let groups = value["testGroups"]
        .as_array()
        .expect("testGroups must be an array");
    assert_eq!(groups.len(), 1, "expected a single test group");
    let group = &groups[0];
    assert!(
        group.get("unused_group_field").is_none(),
        "sanitizer should drop extraneous fields"
    );
    let tests = group["tests"].as_array().expect("tests must be array");
    assert_eq!(tests.len(), 1, "expected a single test case");
    let test = &tests[0];
    assert!(
        test.get("unused").is_none(),
        "sanitizer should drop extraneous test fields"
    );
    assert_eq!(test["tcId"].as_u64(), Some(1));
}
