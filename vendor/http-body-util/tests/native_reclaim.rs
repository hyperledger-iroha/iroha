//! Verify post-deallocation reclamation for native unsynchronized body boxes.

#[path = "support/reclaim.rs"]
mod reclaim;

use bytes::Bytes;
use http_body::Body as _;
use http_body_util::combinators::UnsyncBoxBody;
use std::convert::Infallible;

#[test]
fn body_storage_is_freed_before_reclamation_on_every_exit() {
    type Body = UnsyncBoxBody<Bytes, Infallible>;
    reclaim::verify(
        Body::try_new_with_reclaim,
        Body::body_layout::<reclaim::ProbeBody<Infallible>>(),
        Body::reclaim_layout::<reclaim::ProbeToken>(),
    );
    reclaim::verify_plain(
        Body::new,
        Body::body_layout::<reclaim::ProbeBody<Infallible>>(),
    );
    assert!(Body::default().is_end_stream());
}
