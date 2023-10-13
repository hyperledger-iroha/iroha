//! Oneshot execution of `validate_blocks` benchmark.
//! Can be useful to profile using flamegraph.
//!
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph --root --release --example validate_blocks
//! ```

mod validate_blocks;

use iroha_config::base::proxy::Builder;
use iroha_data_model::Level;
use iroha_logger::{Configuration, ConfigurationProxy};
use validate_blocks::WsvValidateBlocks;

fn main() {
    let log_config = Configuration {
        max_log_level: Level::INFO.into(),
        compact_mode: false,
        ..ConfigurationProxy::default()
            .build()
            .expect("Default logger config should always build")
    };
    // Can't use logger because it's failed to initialize.
    if let Err(err) = iroha_logger::init(&log_config) {
        eprintln!("Failed to initialize logger: {err}");
    }
    iroha_logger::info!("Starting...");
    let bench = WsvValidateBlocks::setup().expect("Failed to setup benchmark");
    WsvValidateBlocks::measure(bench).expect("Failed to execute bnechmark");
}
