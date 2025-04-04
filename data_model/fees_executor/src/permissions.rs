use alloc::{format, string::String, vec::Vec};

use iroha_executor_data_model::permission::Permission;
use iroha_schema::IntoSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Permission, Deserialize, Serialize, IntoSchema)]
pub struct CanModifyFeesOptions;
