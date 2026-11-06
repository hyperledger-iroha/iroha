//! Sole typed carrier for the explicitly authorized all-four-validator SoraFS repair.

use super::*;

isi! {
    /// Atomically select the exact new repair definitions and bootstrap SoraFS once.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::sorafs_repair::ApplySorafsRepairV1")]
    pub struct ApplySorafsRepairV1 {
        /// Exact parent-bound successor plan and all four scheduled validator signatures.
        pub certificate: crate::runtime::sorafs_repair::SorafsRepairCertificateV1,
    }
}

impl crate::seal::Instruction for ApplySorafsRepairV1 {}

impl ApplySorafsRepairV1 {
    /// Stable wire identity, separate from the unchanged threshold-key lifecycle definition.
    pub const WIRE_ID: &'static str = "iroha.sorafs.bootstrap.repair.apply.v1";
}
