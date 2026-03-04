//! Oneshot execution of `apply_blocks` benchmark.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Can be useful to profile using flamegraph.
//!
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph --root --release --example apply_blocks
//! ```

mod apply_blocks;

use apply_blocks::StateApplyBlocks;
use iroha_config::base::{env::std_env, read::ConfigReader};
use iroha_logger::Config;

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the Runtime");
    {
        let _guard = rt.enter();
        let mut config = ConfigReader::new()
            .with_env(std_env)
            .read_and_complete()
            .expect("Failed to load config");
        config.terminal_colors = true;
        let _ = iroha_logger::init_global(config).expect("Failed to initialize logger");
    }
    iroha_logger::info!("Starting...");
    let bench = StateApplyBlocks::setup(rt.handle());
    StateApplyBlocks::measure(&bench);
}
