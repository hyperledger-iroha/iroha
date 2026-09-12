//! Context-free stream-token policy boundaries shared by issuers and hardware signers.

use super::*;

fn sample_body() -> StreamTokenBodyV1 {
    StreamTokenBodyV1 {
        token_id: "0123456789abcdef0123456789abcdef".to_string(),
        manifest_cid: vec![0x01, 0x55, 0x01],
        provider_id: [0xAA; 32],
        profile_handle: "sorafs.sf1@1.0.0".to_string(),
        max_streams: 4,
        ttl_epoch: 1_731_234_567,
        rate_limit_bytes: 10 * 1024 * 1024,
        issued_at: 1_731_234_000,
        requests_per_minute: 120,
        token_pk_version: 3,
    }
}
#[test]
fn canonical_body_validation_rejects_each_unsafe_dimension() {
    let mut cases = Vec::new();
    let mut body = sample_body();
    body.token_id = "ABC".to_string();
    cases.push((body, StreamTokenBodyError::TokenId));
    let mut body = sample_body();
    body.manifest_cid.clear();
    cases.push((body, StreamTokenBodyError::ManifestCid));
    let mut body = sample_body();
    body.provider_id = [0; 32];
    cases.push((body, StreamTokenBodyError::ProviderId));
    let mut body = sample_body();
    body.profile_handle = "sorafs profile".to_string();
    cases.push((body, StreamTokenBodyError::ProfileHandle));
    let mut body = sample_body();
    body.max_streams = 0;
    cases.push((body, StreamTokenBodyError::MaxStreams));
    let mut body = sample_body();
    body.ttl_epoch = body.issued_at;
    cases.push((body, StreamTokenBodyError::Lifetime));
    let mut body = sample_body();
    body.rate_limit_bytes = 0;
    cases.push((body, StreamTokenBodyError::RateLimit));
    let mut body = sample_body();
    body.requests_per_minute = 0;
    cases.push((body, StreamTokenBodyError::RequestsPerMinute));
    let mut body = sample_body();
    body.token_pk_version = 0;
    cases.push((body, StreamTokenBodyError::KeyVersion));
    for (body, expected) in cases {
        assert_eq!(validate_token_body(&body), Err(expected));
    }
}
#[test]
fn canonical_body_accepts_exact_maximum_lifetime_and_rejects_max_plus_one() {
    let mut maximum = sample_body();
    maximum.issued_at = 1_700_000_000;
    maximum.ttl_epoch = maximum.issued_at + STREAM_TOKEN_MAX_TTL_SECS_V1;
    validate_token_body(&maximum).expect("exact maximum lifetime");
    maximum.ttl_epoch += 1;
    assert_eq!(
        validate_token_body(&maximum),
        Err(StreamTokenBodyError::Lifetime)
    );
}
