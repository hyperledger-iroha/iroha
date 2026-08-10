// Remote SoraFS hydration security regressions included by the parent test module.

use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::{Router, routing::get};
use tokio::net::TcpListener;

use super::*;

#[test]
fn provider_base_url_requires_clean_https_origin() {
    assert_eq!(
        normalize_provider_torii_base_url("provider.example:8443")
            .expect("bare HTTPS origin")
            .as_str(),
        "https://provider.example:8443/"
    );
    assert!(normalize_provider_torii_base_url("https://provider.example/").is_ok());

    for unsafe_url in [
        "http://provider.example/",
        "https://user@provider.example/",
        "https://user:password@provider.example/",
        "https://provider.example/?token=secret",
        "https://provider.example/#fragment",
        "https://provider.example/api",
        "https://provider.example/a/..",
        "https://provider.example/%2e%2e/",
        "https://provider.example\\@127.0.0.1/",
        "https://provider.example@127.0.0.1/",
        "https://[fe80::1%25en0]/",
        "https://provider.example:0/",
    ] {
        assert!(
            normalize_provider_torii_base_url(unsafe_url).is_err(),
            "unsafe endpoint should be rejected: {unsafe_url}"
        );
    }
}

#[test]
fn public_ip_policy_rejects_special_ipv4_and_ipv6_ranges() {
    for ip in [
        "0.0.0.0",
        "10.0.0.1",
        "100.64.0.1",
        "127.0.0.1",
        "169.254.169.254",
        "172.16.0.1",
        "192.0.0.1",
        "192.0.2.1",
        "192.168.1.1",
        "198.18.0.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "255.255.255.255",
        "::",
        "::1",
        "::ffff:127.0.0.1",
        "::ffff:10.0.0.1",
        "::ffff:169.254.169.254",
        "::8.8.8.8",
        "::192.168.1.1",
        "fc00::1",
        "fe80::1",
        "ff02::1",
        "2001:db8::1",
        "2001:2::1",
        "2002::1",
        "3fff::1",
    ] {
        let parsed = ip.parse::<IpAddr>().expect("test IP literal");
        assert!(!ip_is_public(parsed), "special address allowed: {ip}");
    }

    for ip in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"] {
        let parsed = ip.parse::<IpAddr>().expect("test public IP literal");
        assert!(ip_is_public(parsed), "public address rejected: {ip}");
    }
}

#[tokio::test]
async fn endpoint_resolution_rejects_private_literals_before_connect() {
    for endpoint in [
        "https://127.0.0.1/",
        "https://127.1/",
        "https://2130706433/",
        "https://0177.0.0.1/",
        "https://0x7f000001/",
        "https://[::ffff:127.0.0.1]/",
        "https://[fe80::1]/",
    ] {
        let private = normalize_provider_torii_base_url(endpoint)
            .expect("syntactically valid private endpoint");
        assert!(
            resolve_public_endpoint(&private).await.is_err(),
            "private endpoint should be rejected: {endpoint}"
        );
    }

    let public =
        normalize_provider_torii_base_url("https://8.8.8.8:444/").expect("public endpoint");
    assert_eq!(
        resolve_public_endpoint(&public)
            .await
            .expect("resolve public literal"),
        vec!["8.8.8.8:444".parse().expect("socket literal")]
    );
    let public_v6 = normalize_provider_torii_base_url("https://[2606:4700:4700::1111]:444/")
        .expect("public IPv6 endpoint");
    assert_eq!(
        resolve_public_endpoint(&public_v6)
            .await
            .expect("resolve public IPv6 literal"),
        vec![
            "[2606:4700:4700::1111]:444"
                .parse()
                .expect("IPv6 socket literal")
        ]
    );
}

#[test]
fn pinned_client_revalidates_addresses_to_prevent_rebinding() {
    let mut source = RemoteCidSource {
        manifest_digest_hex: hex::encode([0x11; 32]),
        provider_id_hex: hex::encode([0x22; 32]),
        torii_base_url: normalize_provider_torii_base_url("https://provider.example/")
            .expect("provider URL"),
        pinned_addrs: vec!["8.8.8.8:443".parse().expect("public address")],
    };
    assert!(build_pinned_remote_client(&source).is_ok());

    source.pinned_addrs = vec!["127.0.0.1:443".parse().expect("loopback address")];
    assert!(build_pinned_remote_client(&source).is_err());
}

#[test]
fn remote_file_layout_rejects_traversal_gaps_and_overflow() {
    let file = |path: &[&str], offset, size| StorageStoredFileDto {
        path: path
            .iter()
            .map(|component| (*component).to_owned())
            .collect(),
        offset,
        size,
        first_chunk: 0,
        chunk_count: 1,
    };
    assert!(
        storage_file_entries_from_manifest_response(&[file(&["..", "secret"], 0, 1)], 1).is_err()
    );
    assert!(
        storage_file_entries_from_manifest_response(&[file(&["index.html"], 1, 1)], 1).is_err()
    );
    assert!(
        storage_file_entries_from_manifest_response(
            &[
                file(&["first"], 0, u64::MAX),
                file(&["second"], u64::MAX, 1),
            ],
            u64::MAX,
        )
        .is_err()
    );
}

#[test]
fn site_ranges_are_bounded_and_single_only() {
    let empty = HeaderMap::new();
    assert_eq!(
        parse_site_response_range(&empty, 7).expect("full range"),
        SiteResponseRange {
            offset: 0,
            length: 7,
            partial: false,
        }
    );
    assert_eq!(
        parse_site_response_range(&empty, MAX_SITE_RESPONSE_BYTES + 1)
            .expect_err("oversized full response")
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );

    let mut headers = HeaderMap::new();
    headers.insert(header::RANGE, HeaderValue::from_static("bytes=2-5"));
    assert_eq!(
        parse_site_response_range(&headers, 10).expect("bounded range"),
        SiteResponseRange {
            offset: 2,
            length: 4,
            partial: true,
        }
    );
    headers.insert(header::RANGE, HeaderValue::from_static("bytes=-3"));
    assert_eq!(
        parse_site_response_range(&headers, 10).expect("suffix range"),
        SiteResponseRange {
            offset: 7,
            length: 3,
            partial: true,
        }
    );
    headers.insert(header::RANGE, HeaderValue::from_static("bytes=0-1,4-5"));
    assert_eq!(
        parse_site_response_range(&headers, 10)
            .expect_err("multiple ranges")
            .status(),
        StatusCode::BAD_REQUEST
    );
}

#[test]
fn active_content_types_require_isolated_origin() {
    for media_type in [
        "text/html; charset=utf-8",
        "text/css; charset=utf-8",
        "text/javascript; charset=utf-8",
        "image/svg+xml",
        "application/xml; charset=utf-8",
        "application/pdf",
        "application/wasm",
    ] {
        assert!(content_type_is_active(media_type), "missed {media_type}");
    }
    assert!(!content_type_is_active("image/png"));
    assert!(!content_type_is_active("application/octet-stream"));
}

#[tokio::test]
async fn same_cid_hydrations_share_one_exclusion_lock() {
    let cid = b"single-flight-security-test-cid";
    let first = cid_hydration_flight(cid).expect("first hydration flight");
    let second = cid_hydration_flight(cid).expect("second hydration flight");
    assert!(Arc::ptr_eq(&first, &second));

    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut tasks = Vec::new();
    for _ in 0..24 {
        let active = Arc::clone(&active);
        let maximum = Arc::clone(&maximum);
        let lock = Arc::clone(&first);
        tasks.push(tokio::spawn(async move {
            let _guard = lock.gate.lock().await;
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            maximum.fetch_max(current, Ordering::SeqCst);
            tokio::task::yield_now().await;
            active.fetch_sub(1, Ordering::SeqCst);
        }));
    }
    for task in tasks {
        task.await.expect("hydration task");
    }
    assert_eq!(maximum.load(Ordering::SeqCst), 1);

    first.record_failure();
    assert!(second.failure_backoff_active());
    first.clear_failure();
    assert!(!second.failure_backoff_active());
}

#[tokio::test]
async fn remote_response_reader_rejects_redirects_and_declared_or_streamed_oversize_bodies() {
    let router = Router::new()
        .route(
            "/ok",
            get(|| async {
                let mut response = Response::new(Body::from("{}"));
                response.headers_mut().insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
                response
            }),
        )
        .route(
            "/redirect",
            get(|| async {
                let mut response = Response::new(Body::from("{}"));
                *response.status_mut() = StatusCode::FOUND;
                response
                    .headers_mut()
                    .insert(header::LOCATION, HeaderValue::from_static("/target"));
                response.headers_mut().insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
                response
            }),
        )
        .route(
            "/declared-oversize",
            get(|| async {
                let mut response = Response::new(Body::from("12345"));
                response.headers_mut().insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
                response
                    .headers_mut()
                    .insert(header::CONTENT_LENGTH, HeaderValue::from_static("5"));
                response
            }),
        )
        .route(
            "/streamed-oversize",
            get(|| async {
                let chunks = futures::stream::iter([
                    Ok::<_, Infallible>(Bytes::from_static(b"123")),
                    Ok::<_, Infallible>(Bytes::from_static(b"456")),
                ]);
                let mut response = Response::new(Body::from_stream(chunks));
                response.headers_mut().insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
                response
            }),
        )
        .route(
            "/oversize-headers",
            get(|| async {
                let mut response = Response::new(Body::from("{}"));
                response.headers_mut().insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
                response.headers_mut().insert(
                    HeaderName::from_static("x-adversarial-padding"),
                    HeaderValue::from_str(&"a".repeat(MAX_REMOTE_RESPONSE_HEADER_BYTES + 1))
                        .expect("oversized test header"),
                );
                response
            }),
        );
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind adversarial response server");
    let addr = listener.local_addr().expect("response server address");
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("serve adversarial responses");
    });
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("test client");

    for path in [
        "redirect",
        "declared-oversize",
        "streamed-oversize",
        "oversize-headers",
    ] {
        let response = client
            .get(format!("http://{addr}/{path}"))
            .send()
            .await
            .expect("fetch adversarial response");
        assert!(
            bounded_remote_response_bytes(response, 4, "adversarial response")
                .await
                .is_err(),
            "response should be rejected: {path}"
        );
    }
    let response = client
        .get(format!("http://{addr}/ok"))
        .send()
        .await
        .expect("fetch bounded response");
    let (status, body) = bounded_remote_response_bytes(response, 4, "bounded response")
        .await
        .expect("accept bounded response");
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"{}");

    server.abort();
}
