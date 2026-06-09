//! Grouped Torii core route integration tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[path = "../account_address_vectors.rs"]
mod account_address_vectors;
#[path = "../account_query_subrouter_smoke.rs"]
mod account_query_subrouter_smoke;
#[path = "../accounts_endpoints.rs"]
mod accounts_endpoints;
#[path = "../accounts_faucet.rs"]
mod accounts_faucet;
#[path = "../accounts_onboard.rs"]
mod accounts_onboard;
#[path = "../accounts_portfolio.rs"]
mod accounts_portfolio;
#[path = "../address_parsing.rs"]
mod address_parsing;
#[path = "../api_versioning.rs"]
mod api_versioning;
#[path = "../app_api_router_smoke.rs"]
mod app_api_router_smoke;
#[path = "../asset_definitions_endpoints.rs"]
mod asset_definitions_endpoints;
#[path = "../configuration_endpoint.rs"]
mod configuration_endpoint;
#[path = "../domains_endpoints.rs"]
mod domains_endpoints;
#[path = "../nfts_endpoints.rs"]
mod nfts_endpoints;
#[path = "../router_feature_matrix.rs"]
mod router_feature_matrix;
#[path = "../runtime_endpoints.rs"]
mod runtime_endpoints;
#[path = "../rwas_endpoints.rs"]
mod rwas_endpoints;
#[path = "../webhooks_subrouter_smoke.rs"]
mod webhooks_subrouter_smoke;
