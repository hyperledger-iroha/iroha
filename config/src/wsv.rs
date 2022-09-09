//! Module for `WorldStateView`-related configuration and structs.
#![allow(clippy::std_instead_of_core)]

use iroha_config_base::derive::{Documented, LoadFromEnv, Proxy};
use iroha_data_model::{metadata::Limits as MetadataLimits, LengthLimits};
use serde::{Deserialize, Serialize};

use crate::wasm;

const DEFAULT_METADATA_LIMITS: MetadataLimits = MetadataLimits::new(2_u32.pow(20), 2_u32.pow(12));
const DEFAULT_IDENT_LENGTH_LIMITS: LengthLimits = LengthLimits::new(1, 2_u32.pow(7));

/// `WorldStateView` configuration.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Proxy, LoadFromEnv, Documented,
)]
#[config(env_prefix = "WSV_")]
#[serde(rename_all = "UPPERCASE", default)]
pub struct Configuration {
    /// [`MetadataLimits`] for every asset with store.
    pub asset_metadata_limits: MetadataLimits,
    /// [`MetadataLimits`] of any asset definition metadata.
    pub asset_definition_metadata_limits: MetadataLimits,
    /// [`MetadataLimits`] of any account metadata.
    pub account_metadata_limits: MetadataLimits,
    /// [`MetadataLimits`] of any domain metadata.
    pub domain_metadata_limits: MetadataLimits,
    /// [`LengthLimits`] for the number of chars in identifiers that can be stored in the WSV.
    pub ident_length_limits: LengthLimits,
    /// WASM runtime configuration
    pub wasm_runtime_config: wasm::Configuration,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            asset_metadata_limits: DEFAULT_METADATA_LIMITS,
            asset_definition_metadata_limits: DEFAULT_METADATA_LIMITS,
            account_metadata_limits: DEFAULT_METADATA_LIMITS,
            domain_metadata_limits: DEFAULT_METADATA_LIMITS,
            ident_length_limits: DEFAULT_IDENT_LENGTH_LIMITS,
            wasm_runtime_config: wasm::Configuration::default(),
        }
    }
}
