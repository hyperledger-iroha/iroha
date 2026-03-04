//! Module with custom tokens
use std::{format, string::String, vec::Vec};

use iroha_executor_data_model::permission::Permission;
use iroha_schema::IntoSchema;
use norito::derive::{JsonDeserialize, JsonSerialize};

/// Token to identify if user can (un-)register domains.
#[derive(Clone, Copy, PartialEq, Eq, Permission, JsonDeserialize, JsonSerialize, IntoSchema)]
pub struct CanControlDomainLives;
