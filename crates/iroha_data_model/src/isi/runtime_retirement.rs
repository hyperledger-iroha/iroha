//! Selected native permanent code exclusion; governance must first disable the definition.

use super::*;

isi! {
    /// Finalize or refresh certified permanent exclusion after governance and the startup floor.
    /// An ordinary submitter only relays the deterministic native check and gains no authority.
    #[derive(Copy, crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::runtime_retirement::RetireRuntimeDefinitionV1")]
    pub struct RetireRuntimeDefinitionV1 {
        /// Exact immutable native implementation whose code is permanently excluded.
        pub definition: crate::runtime::definition::DefinitionId,
    }
}

impl crate::seal::Instruction for RetireRuntimeDefinitionV1 {}

impl RetireRuntimeDefinitionV1 {
    /// Stable selected retirement-producer wire identity.
    pub const WIRE_ID: &'static str = "iroha.runtime.retirement.finalize.v1";
}
