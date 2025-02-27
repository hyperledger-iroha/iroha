use alloc::{format, string::String, vec::Vec};

use iroha_executor_data_model::parameter::Parameter as ExecutorParameter;
use iroha_schema::IntoSchema;
use iroha_data_model::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, ExecutorParameter, Deserialize, Serialize, IntoSchema)]
pub struct FeesOptions {
    pub receiver: AccountId,
    pub asset: AssetDefinitionId,
}

impl Default for FeesOptions {
    fn default() -> Self {
        Self {
            receiver: "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland".parse().unwrap(),
            asset: "xor#wonderland".parse().unwrap(),
        }
    }
}
