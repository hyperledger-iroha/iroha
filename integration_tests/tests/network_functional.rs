#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Grouped integration harness for network churn and functional network scenarios.

#[path = "concurrency.rs"]
mod concurrency;
#[path = "extra_functional/mod.rs"]
mod extra_functional;
#[path = "observer_sync.rs"]
mod observer_sync;
