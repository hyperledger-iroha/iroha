//! Module with custom parameters
use std::{format, string::String, vec::Vec};

use iroha_executor_data_model::parameter::{CustomParameter, Parameter};
use iroha_schema::IntoSchema;
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};

/// Parameter that controls domain limits
#[derive(PartialEq, Eq, JsonDeserialize, JsonSerialize, IntoSchema)]
pub struct DomainLimits {
    /// Length of domain id in bytes
    pub id_len: u32,
}

impl Parameter for DomainLimits {}

impl From<DomainLimits> for CustomParameter {
    fn from(value: DomainLimits) -> Self {
        CustomParameter::new(
            <DomainLimits as Parameter>::id(),
            json::to_value(&value)
                .expect("INTERNAL BUG: Failed to serialize Executor data model entity"),
        )
    }
}

impl From<DomainLimits> for iroha_data_model::parameter::Parameter {
    fn from(value: DomainLimits) -> Self {
        Self::Custom(value.into())
    }
}

impl Default for DomainLimits {
    fn default() -> Self {
        Self {
            id_len: 2_u32.pow(4),
        }
    }
}
