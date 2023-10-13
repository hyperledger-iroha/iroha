//! Module for kura-related configuration and structs
use std::{num::NonZeroU64, path::Path};

use eyre::{eyre, Result};
use iroha_config_base::derive::{Documented, Proxy};
use serde::{Deserialize, Serialize};

const DEFAULT_BLOCKS_PER_STORAGE_FILE: u64 = 1000_u64;
const DEFAULT_BLOCK_STORE_PATH: &str = "./storage";
const DEFAULT_ACTOR_CHANNEL_CAPACITY: u32 = 100;

/// `Kura` configuration.
#[derive(Clone, Deserialize, Serialize, Debug, Documented, Proxy, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
#[config(env_prefix = "KURA_")]
pub struct Configuration {
    /// Initialization mode: `strict` or `fast`.
    pub init_mode: Mode,
    /// Path to the existing block store folder or path to create new folder.
    pub block_store_path: String,
    /// Maximum number of blocks to write into a single storage file.
    pub blocks_per_storage_file: NonZeroU64,
    /// Default buffer capacity of actor's MPSC channel.
    pub actor_channel_capacity: u32,
    /// Whether or not new blocks be outputted to a file called blocks.json.
    pub debug_output_new_blocks: bool,
}

impl Default for ConfigurationProxy {
    fn default() -> Self {
        Self {
            init_mode: Some(Mode::default()),
            block_store_path: Some(DEFAULT_BLOCK_STORE_PATH.to_owned()),
            blocks_per_storage_file: Some(
                NonZeroU64::new(DEFAULT_BLOCKS_PER_STORAGE_FILE)
                    .expect("BLOCKS_PER_STORAGE cannot be set to a non-positive value."),
            ),
            actor_channel_capacity: Some(DEFAULT_ACTOR_CHANNEL_CAPACITY),
            debug_output_new_blocks: Some(false),
        }
    }
}

impl Configuration {
    /// Set `block_store_path` configuration parameter. Will overwrite the existing one.
    ///
    /// # Errors
    /// Fails if the path is not valid
    pub fn block_store_path(&mut self, path: &Path) -> Result<()> {
        self.block_store_path = path
            .to_str()
            .ok_or_else(|| eyre!("Failed to yield slice from path"))?
            .to_owned();
        Ok(())
    }
}

/// Kura initialization mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Strict validation of all blocks.
    #[default]
    Strict,
    /// Fast initialization with basic checks.
    Fast,
}

#[cfg(test)]
pub mod tests {
    use proptest::prelude::*;

    use super::*;

    prop_compose! {
        pub fn arb_proxy()
            (
                init_mode in prop::option::of(Just(Mode::default())),
                block_store_path in prop::option::of(Just(DEFAULT_BLOCK_STORE_PATH.into())),
                blocks_per_storage_file in prop::option::of(Just(NonZeroU64::new(DEFAULT_BLOCKS_PER_STORAGE_FILE).expect("Cannot be set to a negative value"))),
                actor_channel_capacity in prop::option::of(Just(DEFAULT_ACTOR_CHANNEL_CAPACITY)),
                debug_output_new_blocks in prop::option::of(Just(false))
            )
            -> ConfigurationProxy {
            ConfigurationProxy { init_mode, block_store_path, blocks_per_storage_file, actor_channel_capacity, debug_output_new_blocks }
        }
    }
}
