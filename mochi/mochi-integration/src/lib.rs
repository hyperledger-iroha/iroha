//! Integration utilities for exercising the MOCHI supervisor against mock Torii services.
//!
//! The helpers exposed by this crate spawn a lightweight in-process Torii
//! endpoint so tests can validate supervisor behaviour without compiling or
//! launching real Iroha binaries.

mod mock_torii;

pub use mock_torii::{
    CANONICAL_BLOCK_WIRE, CANONICAL_EVENT_MESSAGE, MockTorii, MockToriiBuilder, MockToriiData,
    MockToriiFrame, kagami_default_manifest_json,
};
