use alloc::{format, string::String, vec::Vec};

use iroha_data_model::prelude::*;
use iroha_executor_data_model::parameter::Parameter as ExecutorParameter;
use iroha_schema::IntoSchema;
use serde::{Deserialize, Serialize};
// use cargo_metadata::MetadataCommand;

#[derive(PartialEq, Eq, ExecutorParameter, Deserialize, Serialize, IntoSchema, Debug)]
pub struct FeesOptions {
    pub asset: AssetId,
}

impl Default for FeesOptions {
    fn default() -> Self {
        // Value can be defined in .cargo/config.toml
        let asset = env!("DEFAULT_FEE_ASSET");

        Self {
            asset: asset.parse().unwrap(),
        }
    }
}
