//! Shared genuine compact payer/catalog construction; no test registrations.

#[path = "load_omega.rs"]
mod load_outer;

include!("compact_catalog_body.rs");
