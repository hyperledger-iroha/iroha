//! Typed carrier of the one owner-authorized, all-four-validator Taira activation.

use super::*;

isi! {
    /// Record the exact certified Time and Ironwood change set as scheduled, once.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::taira_activation::ScheduleTairaTimeIronwoodV1")]
    pub struct ScheduleTairaTimeIronwoodV1 {
        /// Exact plan and all four scheduled validator signatures.
        pub certificate: crate::runtime::taira_activation::TairaActivationCertificateV1,
    }
}

impl crate::seal::Instruction for ScheduleTairaTimeIronwoodV1 {}

impl ScheduleTairaTimeIronwoodV1 {
    /// Stable wire identity. No selection binds it: Core recognizes only the exact certified
    /// boundary, and every other envelope stays an ordinary unbound rejection.
    pub const WIRE_ID: &'static str = "iroha.runtime.taira_time_ironwood.schedule.v1";
}
