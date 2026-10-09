//! Retain only evidence reverified by the installed complete finality graph.

use ff::PrimeField;
use iroha_data_model::{
    isi::kagemusha_wallet::KagemushaWalletLoadReceiptV1,
    kagemusha::{KagemushaWalletLoadFinalityV1, KagemushaWalletValidationErrorV1},
};
use iroha_kagemusha_proof::finality::{
    continuity::{SourceNodeEvidence, producer::Error as SourceError},
    native::InstalledFinality,
};
use iroha_pasta::{Fp, msm::MemoryBudget};

/// Refusal to retain complete ordinary Load finality evidence.
#[derive(Debug, thiserror::Error)]
pub enum FinalityRetentionError {
    /// The original receipt or bounded evidence frame is malformed.
    #[error(transparent)]
    Model(#[from] KagemushaWalletValidationErrorV1),
    /// The installed terminal source refuses an endpoint, proof or carried claim.
    #[error(transparent)]
    Source(#[from] SourceError),
    /// A receipt digest is not a canonical scalar.
    #[error("ordinary Load receipt digest is not a canonical field")]
    Digest,
}

/// Verify the installed terminal source before forming its retained wire record.
///
/// The owner pins the original source catalog and signed-genesis anchor. It
/// reconstructs the exact terminal statement from this receipt, verifies the
/// wrapper under its installed key and decides both carried curves. No caller
/// endpoints or acceptance flags are serialized into the retained model.
///
/// # Errors
/// Refuses a malformed receipt, another terminal statement/key, wrong proof
/// length, failed proof/claim decision, exhausted proof budget or oversized record.
pub fn retain_load_finality(
    installed: &InstalledFinality,
    receipt: &KagemushaWalletLoadReceiptV1,
    evidence: SourceNodeEvidence,
    budget: MemoryBudget,
) -> Result<KagemushaWalletLoadFinalityV1, FinalityRetentionError> {
    let receipt_digest = receipt.receipt_digest()?;
    let digest =
        Option::<Fp>::from(Fp::from_repr(receipt_digest)).ok_or(FinalityRetentionError::Digest)?;
    installed.verify_receipt_evidence(digest, &evidence, budget)?;
    let retained = KagemushaWalletLoadFinalityV1 {
        version: 1,
        anchor_digest: installed.anchor().digest().to_repr(),
        receipt_digest,
        proof: evidence.proof,
        pallas_claim: evidence.pallas.to_bytes(),
        vesta_claim: evidence.vesta.to_bytes(),
    };
    retained.validate()?;
    Ok(retained)
}
