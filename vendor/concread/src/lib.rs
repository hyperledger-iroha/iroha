//! Concread - Concurrently Readable Datastructures
//!
//! Concurrently readable is often referred to as [Copy-On-Write](https://en.wikipedia.org/wiki/Copy-on-write), [Multi-Version-Concurrency-Control](https://en.wikipedia.org/wiki/Multiversion_concurrency_control)
//! or [Software Transactional Memory](https://en.wikipedia.org/wiki/Software_transactional_memory).
//!
//! These structures allow multiple readers with transactions
//! to proceed while single writers can operate. A reader is guaranteed the content
//! of their transaction will remain the same for the duration of the read, and readers do not block
//! writers from proceeding.
//! Writers are serialised, just like a mutex.
//!
//! You can use these in place of a RwLock, and will likely see improvements in
//! parallel throughput of your application.
//!
//! The best use is in place of mutex/rwlock, where the reader exists for a
//! non-trivial amount of time.
//!
//! For example, if you have a RwLock where the lock is taken, data changed or read, and dropped
//! immediately, this probably won't help you.
//!
//! However, if you have a RwLock where you hold the read lock for any amount of time,
//! writers will begin to stall - or inversely, the writer will cause readers to block
//! and wait as the writer proceeds.
//!
//! # Features
//! This library provides multiple structures for you to use. You may enable or disable these based
//! on our features.
//!
//! * `ebr` - epoch based reclaim cell
//! * `maps` - concurrently readable b+tree and hashmaps
//! * `arcache` - concurrently readable ARC cache
//!
//! By default all of these features are enabled. If you are planning to use this crate in a wasm
//! context we recommend you use only `maps` as a feature.

#![deny(warnings)]
#![warn(unused_extern_crates)]
#![warn(missing_docs)]
#![allow(clippy::needless_lifetimes)]
#![cfg_attr(feature = "simd_support", feature(portable_simd))]

#[cfg(all(test, feature = "dhat-heap"))]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[cfg(feature = "maps")]
#[macro_use]
extern crate smallvec;

pub mod cowcell;
pub use cowcell::CowCell;

#[cfg(feature = "ebr")]
pub mod ebrcell;
#[cfg(feature = "ebr")]
pub use ebrcell::EbrCell;

#[cfg(feature = "arcache")]
pub mod arcache;
#[cfg(feature = "tcache")]
pub mod threadcache;

// This is where the scary rust lives.
#[cfg(feature = "maps")]
pub mod internals;
// This is where the good rust lives.
#[cfg(feature = "maps")]
mod utils;

#[cfg(feature = "maps")]
pub mod bptree;
#[cfg(feature = "maps")]
pub mod hashmap;
#[cfg(feature = "maps")]
pub mod hashtrie;

#[cfg(test)]
mod lc_tests;
