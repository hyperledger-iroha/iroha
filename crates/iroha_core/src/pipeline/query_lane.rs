//! Query lane compatibility wrapper.
//!
//! The snapshot lane logic now lives in [`crate::query::snapshot`]. This module re-exports
//! the public surface so existing call sites in the pipeline continue to compile.

pub use crate::query::snapshot::{
    CursorMode, SnapshotQueryError, run_on_snapshot, run_on_snapshot_with_mode,
};
