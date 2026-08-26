#[test]
fn load_guard_directory_rejects_oversized_file_before_decode() {
    let file = NamedTempFile::new().expect("temp file");
    file.as_file()
        .set_len(
            u64::try_from(iroha_crypto::soranet::directory::GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1)
                .expect("fixed snapshot limit fits u64")
                + 1,
        )
        .expect("extend oversized sparse snapshot");
    let error = load_guard_directory(file.path(), &"00".repeat(32), 1_734_000_000)
        .expect_err("oversized snapshot must fail in the file loader");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("first-release limit"), "{diagnostic}");
}
#[test]
fn guard_directory_http_body_accepts_chunked_response_without_length() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind chunked-response listener");
    let address = listener.local_addr().expect("chunked-response address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept guard-directory client");
        let mut request = [0_u8; 1024];
        let request_bytes = stream
            .read(&mut request)
            .expect("read guard-directory request");
        assert!(
            request_bytes > 0,
            "guard-directory request must not be empty"
        );
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nguard\r\nA\r\n directory\r\n0\r\n\r\n",
            )
            .expect("write chunked guard-directory response");
    });
    let mut response = BlockingHttpClient::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("build HTTP client")
        .get(format!("http://{address}/guard-directory"))
        .send()
        .expect("fetch chunked guard-directory response")
        .error_for_status()
        .expect("chunked guard-directory response status");
    let content_length = response.content_length();
    assert_eq!(content_length, None);
    let body = read_guard_directory_http_body_bounded(&mut response, content_length)
        .expect("bounded reader accepts an in-limit chunked response");
    assert_eq!(body, b"guard directory");
    server.join().expect("chunked-response server finished");
}
#[test]
fn guard_directory_http_body_rejects_oversized_content_length_before_read() {
    struct UnexpectedReader;
    impl Read for UnexpectedReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            panic!("oversized Content-Length must fail before reading the response body");
        }
    }
    let declared_length =
        u64::try_from(iroha_crypto::soranet::directory::GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1)
            .expect("fixed snapshot limit fits u64")
            + 1;
    let error = read_guard_directory_http_body_bounded(UnexpectedReader, Some(declared_length))
        .expect_err("oversized declared body must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("first-release limit"));
}
#[test]
fn guard_directory_http_body_accepts_limit_and_rejects_no_length_max_plus_one() {
    assert_eq!(
        read_guard_directory_http_body_with_limit(io::Cursor::new([0_u8; 8]), None, 8)
            .expect("exact-limit no-length body is valid"),
        vec![0_u8; 8]
    );
    let error = read_guard_directory_http_body_with_limit(io::Cursor::new([0_u8; 9]), None, 8)
        .expect_err("oversized no-length body must fail at max plus one byte");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("8-byte first-release limit"));
}
#[test]
fn guard_directory_summary_reports_expected_counts() {
    let bytes = sample_guard_directory_snapshot_bytes();
    let summary = inspect_guard_directory_bytes(&bytes).expect("inspect directory");
    assert_eq!(summary.version, 2);
    assert_eq!(summary.authentication, "structural_inspection_only");
    assert_eq!(
        summary.snapshot_digest_hex,
        hex::encode(compute_snapshot_digest(&bytes))
    );
    assert_eq!(summary.issuer_count, 1);
    assert_eq!(summary.relay_count, 1);
    assert_eq!(summary.entry_guards, 1);
    assert_eq!(summary.entry_guards_pq, 1);
    assert_eq!(summary.exit_relays, 0);
    let expected_hash = "ab".repeat(32);
    assert_eq!(
        summary.directory_hash_hex.as_deref(),
        Some(expected_hash.as_str())
    );
    assert!(summary.entry_guard_pq_ratio > 0.99);
}
