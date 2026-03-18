//! Parameter and permission derives should compile with executor data model helpers.

use iroha_executor_data_model_derive::{Parameter, Permission};

/// Custom executor parameter used for UI tests.
#[derive(
    Default,
    Clone,
    iroha_executor_data_model::json_macros::JsonSerialize,
    iroha_executor_data_model::json_macros::JsonDeserialize,
    iroha_schema::IntoSchema,
    Parameter,
)]
struct CustomLimit {
    /// Arbitrary threshold to exercise serialization.
    threshold: u32,
}

/// Custom permission token used for UI tests.
#[derive(
    Clone,
    iroha_executor_data_model::json_macros::JsonSerialize,
    iroha_executor_data_model::json_macros::JsonDeserialize,
    iroha_schema::IntoSchema,
    Permission,
)]
struct CanEditDomainMetadata {
    /// Domain scoped by the permission.
    domain: iroha_data_model::domain::DomainId,
}

fn main() {
    let param = CustomLimit::default();
    let perm = CanEditDomainMetadata {
        domain: "wonderland".parse().unwrap(),
    };

    let _custom_param_id =
        <CustomLimit as iroha_executor_data_model::parameter::Parameter>::id();
    let _permission_name =
        <CanEditDomainMetadata as iroha_executor_data_model::permission::Permission>::name();

    let _custom_param: iroha_executor_data_model::parameter::CustomParameter = param.into();
    let _generic_perm: iroha_data_model::permission::Permission = perm.into();
}
