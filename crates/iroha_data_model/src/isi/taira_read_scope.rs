//! Typed carrier of one owner-authorized, all-four-validator read-scope activation on Taira.

use super::*;

isi! {
    /// Record the exact certified read-scope change set as scheduled, once.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::taira_read_scope::ScheduleTairaReadScopeV1")]
    pub struct ScheduleTairaReadScopeV1 {
        /// Exact plan and all four scheduled validator signatures.
        pub certificate: crate::runtime::taira_read_scope::TairaReadScopeCertificateV1,
    }
}

impl crate::seal::Instruction for ScheduleTairaReadScopeV1 {}

impl ScheduleTairaReadScopeV1 {
    /// Stable wire identity. No selection binds it: Core recognizes only the exact certified
    /// boundary, and every other envelope stays an ordinary unbound rejection.
    pub const WIRE_ID: &'static str = "iroha.runtime.taira_read_scope.schedule.v1";
}
