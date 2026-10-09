//! Direct ordinary Load finality bound to the independently selected signed genesis.

use super::*;
use iroha_data_model::{
    block::consensus::SumeragiRootScope,
    kagemusha::{KagemushaWalletLoadFinalityV1, KagemushaWalletLoadReceiptV1},
    sumeragi_finality::{FinalityError, SumeragiFinalityVerifier},
};

/// Refusal to bind a wallet installation to its native signed-genesis verifier.
#[derive(Debug, thiserror::Error)]
pub enum FinalityQualificationErrorV1 {
    /// The selected native root does not authenticate this wallet network.
    #[error("wallet finality requires the installed network's signed global genesis")]
    AnchorMismatch,
}

/// Native certificate verifier bound to one authenticated wallet installation.
pub struct QualifiedReceiptSourceV1 {
    verifier: SumeragiFinalityVerifier,
    scheme_id: [u8; 32],
    manifest_digest: [u8; 32],
}

impl QualifiedReceiptSourceV1 {
    /// Exact authenticated wallet installation using this native trust root.
    pub const fn installation(&self) -> ([u8; 32], [u8; 32]) {
        (self.scheme_id, self.manifest_digest)
    }

    /// Independently selected signed-genesis verifier.
    pub const fn verifier(&self) -> &SumeragiFinalityVerifier {
        &self.verifier
    }

    /// Authenticate exact receipt inclusion using native BLS commit certificates.
    ///
    /// # Errors
    /// Invalid certificate, epoch authorization, receipt binding or event inclusion.
    pub fn verify_receipt_evidence(
        &self,
        receipt: &KagemushaWalletLoadReceiptV1,
        evidence: &KagemushaWalletLoadFinalityV1,
    ) -> Result<(), FinalityError> {
        evidence.verify(&self.verifier, receipt)
    }
}

impl AuthenticatedProducerInventoryV1 {
    /// Bind the independently authenticated native root to this wallet installation.
    ///
    /// # Errors
    /// A private root or another network cannot authorize ordinary wallet Loads.
    pub fn qualify_finality(
        &self,
        installed: &InstalledVerifierPackV1,
        verifier: &SumeragiFinalityVerifier,
    ) -> Result<QualifiedReceiptSourceV1, FinalityQualificationErrorV1> {
        if verifier.root_scope().ok() != Some(SumeragiRootScope::Global)
            || verifier.initial_epoch().network_id.as_bytes()
                != &installed.verifier().scheme().network_id
            || self.installation()
                != (installed.verifier().scheme().scheme_id(), installed.verifier().manifest_digest())
        {
            return Err(FinalityQualificationErrorV1::AnchorMismatch);
        }
        Ok(QualifiedReceiptSourceV1 {
            verifier: verifier.clone(),
            scheme_id: self.scheme_id,
            manifest_digest: self.manifest_digest,
        })
    }
}
