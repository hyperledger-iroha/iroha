//! Grouped Torii governance integration tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[path = "../gov_council_persist_integration.rs"]
mod gov_council_persist_integration;
#[path = "../gov_council_vrf.rs"]
mod gov_council_vrf;
#[path = "../gov_enact_handler.rs"]
mod gov_enact_handler;
#[path = "../gov_mode_mismatch_and_autoclose.rs"]
mod gov_mode_mismatch_and_autoclose;
#[path = "../gov_protected_endpoints.rs"]
mod gov_protected_endpoints;
#[path = "../gov_protected_endpoints_router.rs"]
mod gov_protected_endpoints_router;
#[path = "../gov_read_endpoints.rs"]
mod gov_read_endpoints;
#[path = "../gov_read_endpoints_router.rs"]
mod gov_read_endpoints_router;
