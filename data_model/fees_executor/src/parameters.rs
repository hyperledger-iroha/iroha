use alloc::{format, string::String, vec::Vec};

use iroha_data_model::prelude::*;
use iroha_executor_data_model::parameter::Parameter;
use iroha_schema::IntoSchema;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Parameter, Deserialize, Serialize, IntoSchema, Debug, Clone)]
pub struct FeesOptions {
    pub asset: AssetId,
    pub amounts: FeesAmountsOptions,
}

impl Default for FeesOptions {
    fn default() -> Self {
        // Path to the config can be defined in .cargo/config.toml
        let config = include_str!(env!("DEFAULT_FEES_CONFIG_PATH"));

        serde_json::from_str(config).expect(config)
    }
}

#[derive(PartialEq, Eq, Parameter, Deserialize, Serialize, IntoSchema, Debug, Clone)]
pub struct FeesAmountsOptions {
    pub fixed: Numeric,
    pub dynamic: Numeric,
    pub precision: u32,
}

impl Default for FeesAmountsOptions {
    fn default() -> Self {
        FeesOptions::default().amounts
    }
}
