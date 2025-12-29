//! Passing coverage for executor data model derives.
#![allow(dead_code)]

use iroha_data_model::asset::AssetDefinitionId;
use iroha_executor_data_model::json_macros::{JsonDeserialize, JsonSerialize};
use iroha_executor_data_model_derive::{Parameter, Permission};
use iroha_schema::IntoSchema;

#[derive(
    Default,
    Clone,
    JsonSerialize,
    JsonDeserialize,
    IntoSchema,
    Parameter,
)]
struct CustomParameter {
    value: String,
}

#[derive(Clone, JsonSerialize, JsonDeserialize, IntoSchema, Permission)]
struct CustomPermission {
    asset: AssetDefinitionId,
}

fn conversions(parameter: CustomParameter, permission: CustomPermission) {
    let _generic_param: iroha_data_model::parameter::Parameter = parameter.clone().into();
    let custom_param: iroha_executor_data_model::parameter::CustomParameter = parameter.into();
    let _roundtrip = CustomParameter::try_from(&custom_param).unwrap();

    let _generic_perm: iroha_data_model::permission::Permission = permission.clone().into();
    let _roundtrip_perm = CustomPermission::try_from(&_generic_perm).unwrap();
}

fn main() {
    let param = CustomParameter {
        value: "demo".to_owned(),
    };
    let perm = CustomPermission {
        asset: "xor#wonderland".parse().unwrap(),
    };
    conversions(param, perm);
}
