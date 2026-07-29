#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Grouped integration harness for Torii queries and proof flows.

#[path = "iterable_queries_torii.rs"]
mod iterable_queries_torii;
#[path = "iterable_query_smoke.rs"]
mod iterable_query_smoke;
#[path = "proof_from_path.rs"]
mod proof_from_path;
#[path = "proofs.rs"]
mod proofs;
#[path = "queries/mod.rs"]
mod queries;
