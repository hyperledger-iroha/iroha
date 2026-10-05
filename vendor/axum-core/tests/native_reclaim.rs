//! Verify that axum forwards the exact native post-deallocation body seam.

// The isolated allocator fixture needs unsafe GlobalAlloc operations. Axum's
// production forwarding API and all other sources keep unsafe_code denied.
#[allow(unsafe_code)]
#[path = "../../http-body-util/tests/support/reclaim.rs"]
mod reclaim;

use axum_core::{body::Body, Error};
use http_body::Body as _;

#[test]
fn axum_body_storage_is_freed_before_reclamation_on_every_exit() {
    reclaim::verify(
        Body::try_new_with_reclaim,
        Body::body_layout::<reclaim::ProbeBody<Error>>(),
        Body::reclaim_layout::<reclaim::ProbeToken>(),
    );
    reclaim::verify_plain(Body::new, Body::body_layout::<reclaim::ProbeBody<Error>>());
    assert!(Body::default().is_end_stream());
}
