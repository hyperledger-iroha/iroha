//! Native compile-once source registration input.
//!
//! TODO: Register this canonical instruction and its native execution handler together after
//! the coordinated dependency freeze and complete compiler allocation admission. A submitted
//! value contains original inputs only; no caller-supplied receipt is accepted.

use super::*;
use crate::smart_contract::{ContractArtifactId, verified_source::ContractSourceInventory};

isi! {
    /// Compile exact original source once and retain its immutable native execution result.
    ///
    /// The signed registered account authority submits the source inventory for the exact
    /// dataspace/artifact. Native execution compares actual canonical compilation with the
    /// already registered bytecode, ABI and signed manifest before constructing any receipt.
    /// Identical retained source reuses the first immutable provenance before compiler work;
    /// another original inventory cannot replace that record. Ordinary transaction fees and
    /// the immutable root's dataspace admission apply.
    #[norito_schema(name = "iroha_data_model::isi::smart_contract_code::RegisterVerifiedContractSource")]
    pub struct RegisterVerifiedContractSource {
        /// Exact target dataspace and canonical complete artifact identity.
        pub artifact_id: ContractArtifactId,
        /// Complete canonical original inventory, including all locked package inputs.
        pub inventory: ContractSourceInventory,
    }
}
impl crate::seal::Instruction for RegisterVerifiedContractSource {}
