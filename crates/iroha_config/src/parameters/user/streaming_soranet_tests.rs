//! Streaming SoraNet configuration parsing tests.

use super::*;

#[test]
fn streaming_soranet_rejects_zero_window_segments() {
    let mut emitter = Emitter::<ParseError>::new();
    let config = StreamingSoranet {
        enabled: true,
        exit_multiaddr: WithOrigin::inline(
            defaults::streaming::soranet::EXIT_MULTIADDR.to_string(),
        ),
        padding_budget_ms: WithOrigin::inline(defaults::streaming::soranet::padding_budget_ms()),
        access_kind: WithOrigin::inline(defaults::streaming::soranet::ACCESS_KIND.to_string()),
        channel_salt: None,
        provision_spool_dir: WithOrigin::inline(PathBuf::from(
            defaults::streaming::soranet::PROVISION_SPOOL_DIR,
        )),
        provision_spool_max_bytes: WithOrigin::inline(
            defaults::streaming::soranet::PROVISION_SPOOL_MAX_BYTES,
        ),
        provision_window_segments: WithOrigin::inline(0),
        provision_queue_capacity: WithOrigin::inline(
            defaults::streaming::soranet::PROVISION_QUEUE_CAPACITY,
        ),
    };
    assert!(config.parse(&mut emitter).is_none());
    let err = emitter
        .into_result()
        .expect_err("zero window segments must be rejected");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("streaming.soranet.provision_window_segments"),
        "unexpected error payload: {debug}"
    );
}

#[test]
fn streaming_soranet_rejects_zero_queue_capacity() {
    let mut emitter = Emitter::<ParseError>::new();
    let config = StreamingSoranet {
        enabled: true,
        exit_multiaddr: WithOrigin::inline(
            defaults::streaming::soranet::EXIT_MULTIADDR.to_string(),
        ),
        padding_budget_ms: WithOrigin::inline(defaults::streaming::soranet::padding_budget_ms()),
        access_kind: WithOrigin::inline(defaults::streaming::soranet::ACCESS_KIND.to_string()),
        channel_salt: None,
        provision_spool_dir: WithOrigin::inline(PathBuf::from(
            defaults::streaming::soranet::PROVISION_SPOOL_DIR,
        )),
        provision_spool_max_bytes: WithOrigin::inline(
            defaults::streaming::soranet::PROVISION_SPOOL_MAX_BYTES,
        ),
        provision_window_segments: WithOrigin::inline(
            defaults::streaming::soranet::PROVISION_WINDOW_SEGMENTS,
        ),
        provision_queue_capacity: WithOrigin::inline(0),
    };
    assert!(config.parse(&mut emitter).is_none());
    let err = emitter
        .into_result()
        .expect_err("zero queue capacity must be rejected");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("streaming.soranet.provision_queue_capacity"),
        "unexpected error payload: {debug}"
    );
}
