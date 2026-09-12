//! Canonical frames and checked reconstruction for public manual model owners.

#[path = "manual_frame_identity/block_signature.rs"]
mod block_signature;
#[cfg(feature = "http")]
#[path = "manual_frame_identity/event_slice.rs"]
mod event_slice;
#[path = "manual_frame_identity/support.rs"]
mod frame_identity_test_support;
#[path = "manual_frame_identity/identifier.rs"]
mod identifier;
#[path = "manual_frame_identity/identifier_rejection.rs"]
mod identifier_rejection;
#[path = "manual_frame_identity/proof.rs"]
mod proof;
#[path = "manual_frame_identity/protocol.rs"]
mod protocol;
#[path = "manual_frame_identity/query_derived.rs"]
mod query_derived;
#[path = "manual_frame_identity/query_parameters.rs"]
mod query_parameters;
#[path = "manual_frame_identity/query_time.rs"]
mod query_time;
#[path = "manual_frame_identity/scalar.rs"]
mod scalar;
#[path = "manual_frame_identity/time_events.rs"]
mod time_events;

#[cfg(feature = "http")]
#[path = "manual_frame_identity/fresh_events.rs"]
mod fresh_events;
