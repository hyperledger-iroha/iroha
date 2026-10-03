//! Signed ordinary Mint clock originals authenticated against actual committed execution.
//!
//! This gate reads consensus-visible native execution receipts, never local CommitQC bytes or
//! local wall time. The exact four uploaded signed observations undergo independent BLS,
//! node/build/configuration, nonce, schedule and finality checks. Original samples establish
//! historical signature/finality mathematics, not the sender's elapsed-time custody or a debit.

use super::ordinary_mint_permission::KagemushaWorldOrdinaryMintIssuerPurposeV1;
use crate::{
    state::{StateReadOnly, StateTransaction},
    sumeragi::certified_chain::committed_block,
};
use iroha_core_zk::kagemusha_v1_state::{
    KagemushaOrdinaryNativeClockSelectionOriginalV1, KagemushaOrdinaryNativeSignedClockOriginalV1,
    KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
    verify_ordinary_native_signed_clock_original_v1,
};
use iroha_data_model::{
    kagemusha::KagemushaOrdinaryCashClockContextV1,
    sumeragi_finality::{
        SumeragiFinalityAttestation, SumeragiFinalityProof, SumeragiFinalityVerifier,
    },
};
use sha2::{Digest as _, Sha256};

// Clock samples already permit at most four MiB per node. Parent witnesses have the same
// purpose-specific finite limit, charged before decoding; the complete ISI has its own cap.
const MAX_PARENT_PROOF_BYTES: usize = 4 * 1024 * 1024;

/// Closed admission of actual signed clock DATA under current World and native execution.
/// No public constructor, decoder, Clone, elapsed-clock or monetary-effect conversion exists.
pub struct KagemushaWorldOrdinaryMintSignedClockV1 {
    verified: KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
    height: u64,
    context_id: [u8; 32],
}
impl KagemushaWorldOrdinaryMintSignedClockV1 {
    /// Borrow the exact signature/finality capability for the genuine Mint proof consumer.
    #[must_use]
    pub const fn verified_original(&self) -> &KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1 {
        &self.verified
    }
    /// Historical certified height, separate from a live issuer decision or logical sequence.
    #[must_use]
    pub const fn height(&self) -> u64 {
        self.height
    }
    /// Full certified decision identity; no DATA membership is asserted.
    #[must_use]
    pub const fn context_id(&self) -> [u8; 32] {
        self.context_id
    }
    /// Pin a separately effect-authorized decision to this transaction's actual predecessor.
    /// This establishes a current execution cut, without sampling local time or proving DATA.
    /// # Errors
    /// Refuses a stale/foreign cut or an original outside its finite mathematical bounds.
    pub fn require_current_execution_cut(
        &self,
        transaction: &StateTransaction<'_, '_>,
        context: &KagemushaOrdinaryCashClockContextV1,
        height: u64,
        context_id: [u8; 32],
        world_root: [u8; 32],
    ) -> Result<(), String> {
        self.verified
            .recheck_cash_context(context)
            .map_err(|e| e.to_string())?;
        let committed =
            u64::try_from(transaction.block_hashes().len()).map_err(|e| e.to_string())?;
        if self.height != committed || height != self.height || context_id != self.context_id {
            return Err(
                "ordinary Mint decision is not the exact current execution predecessor".into(),
            );
        }
        let actual = committed_block(transaction, height).map_err(|e| e.to_string())?;
        if actual.commitment().execution.world_state_root.as_ref() != &world_root {
            return Err("ordinary Mint decision certified World root differs".into());
        }
        Ok(())
    }
}

/// Authenticate complete signed observations after exact current World purpose admission.
///
/// All parent witnesses are caller DATA. Every decoded decision is compared with actual
/// consensus-visible committed execution before the bounded checkpoint data is imported.
/// The selected clock original must be the exact full SHA granted by the current asset owner;
/// an offered node pin or checkpoint cannot select its own authority.
/// # Errors
/// Refuses revoked purpose, foreign selection, absent/excessive parents, changed committed
/// execution, bad signature/finality/nonce/runtime or noncanonical complete originals.
pub fn admit_ordinary_mint_signed_clock_v1(
    transaction: &StateTransaction<'_, '_>,
    purpose: &KagemushaWorldOrdinaryMintIssuerPurposeV1,
    clock_selection_original: &[u8],
    clock_original: &[u8],
    parent_proof_originals: &[Vec<u8>],
    context: &KagemushaOrdinaryCashClockContextV1,
) -> Result<KagemushaWorldOrdinaryMintSignedClockV1, String> {
    purpose.recheck(transaction)?;
    let admitted = admit_retained_ordinary_mint_signed_clock_v1(
        transaction,
        purpose,
        clock_selection_original,
        clock_original,
        parent_proof_originals,
        context,
    )?;
    purpose.recheck(transaction)?;
    Ok(admitted)
}

/// Retained historical source admission under an independently held actual World purpose.
/// All authentic native execution and four-original signature checks are identical to the
/// live wrapper. This gate cannot create a current execution/debit or elapsed-time loan.
pub(super) fn admit_retained_ordinary_mint_signed_clock_v1(
    view: &impl StateReadOnly,
    purpose: &KagemushaWorldOrdinaryMintIssuerPurposeV1,
    clock_selection_original: &[u8],
    clock_original: &[u8],
    parent_proof_originals: &[Vec<u8>],
    context: &KagemushaOrdinaryCashClockContextV1,
) -> Result<KagemushaWorldOrdinaryMintSignedClockV1, String> {
    purpose.recheck_retained_scope(view)?;
    if <[u8; 32]>::from(Sha256::digest(clock_selection_original))
        != purpose.clock_selection_original_sha256()
    {
        return Err("ordinary Mint clock selection is not exactly World granted".into());
    }
    let selection =
        KagemushaOrdinaryNativeClockSelectionOriginalV1::decode_original(clock_selection_original)
            .map_err(|e| e.to_string())?;
    if selection.network() != *view.network_id()
        || selection.checkpoint().chain_id() != view.chain_id().to_string()
    {
        return Err("ordinary Mint clock root names another actual chain".into());
    }
    let original = KagemushaOrdinaryNativeSignedClockOriginalV1::decode_original(clock_original)
        .map_err(|e| e.to_string())?;
    let first = &original.signed_observations()[0];
    let reply: SumeragiFinalityAttestation = decode_exact(first, MAX_PARENT_PROOF_BYTES)?;
    let tip = reply.body.finality_proof;
    let expected_parents =
        usize::try_from(tip.height().saturating_sub(1).min(2)).map_err(|e| e.to_string())?;
    if tip.height() == 0 || parent_proof_originals.len() != expected_parents {
        return Err("ordinary Mint signed clock omits exact consecutive parent witnesses".into());
    }
    let mut proofs = Vec::with_capacity(3);
    for raw in parent_proof_originals {
        proofs.push(decode_exact::<SumeragiFinalityProof>(
            raw,
            MAX_PARENT_PROOF_BYTES,
        )?);
    }
    proofs.push(tip);
    for proof in &proofs {
        let decoded = proof.decode_checked().map_err(|e| e.to_string())?;
        let actual = committed_block(view, proof.height()).map_err(|e| e.to_string())?;
        if !decoded.matches_native_execution_decision(
            &actual.block_hash(),
            actual.core_hash().0,
            actual.result().0,
            actual.commitment(),
        ) {
            return Err(
                "ordinary Mint uploaded clock decision differs from authentic committed execution"
                    .into(),
            );
        }
    }
    let checkpoint = selection
        .checkpoint()
        .with_independently_authenticated_decision_data(&proofs)
        .map_err(|e| e.to_string())?;
    let verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
        &checkpoint,
        view.network_id(),
        selection.checkpoint().chain_id(),
    )
    .map_err(|e| e.to_string())?;
    let selected = selection.selected_originals().map_err(|e| e.to_string())?;
    let verified =
        verify_ordinary_native_signed_clock_original_v1(&selected, &verifier, clock_original)
            .map_err(|e| e.to_string())?;
    verified
        .recheck_cash_context(context)
        .map_err(|e| e.to_string())?;
    let admitted = KagemushaWorldOrdinaryMintSignedClockV1 {
        verified,
        height: proofs
            .last()
            .ok_or("ordinary Mint clock tip absent")?
            .height(),
        context_id: *original.certified_context_id().as_ref(),
    };
    purpose.recheck_retained_scope(view)?;
    Ok(admitted)
}

fn decode_exact<T: norito::NoritoSerialize>(raw: &[u8], maximum: usize) -> Result<T, String>
where
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if raw.is_empty() || raw.len() > maximum {
        return Err("ordinary Mint clock witness exceeds its finite bound".into());
    }
    let value: T =
        norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
            .map_err(|e| e.to_string())?;
    if norito::encode_canonical(&value).map_err(|e| e.to_string())? != raw {
        return Err("ordinary Mint clock witness is not the sole canonical original".into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clock_parent_original_decoder_rejects_trailing_and_oversized_data() {
        use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;
        let f = NativeFinalityFixture::new();
        let raw = norito::encode_canonical(f.latest()).unwrap();
        let proof = decode_exact::<SumeragiFinalityProof>(&raw, MAX_PARENT_PROOF_BYTES).unwrap();
        assert_eq!(proof, *f.latest());
        let mut trailing = raw.clone();
        trailing.push(0);
        assert!(decode_exact::<SumeragiFinalityProof>(&trailing, MAX_PARENT_PROOF_BYTES).is_err());
        assert!(decode_exact::<SumeragiFinalityProof>(&raw, raw.len() - 1).is_err());
        assert!(decode_exact::<SumeragiFinalityProof>(&[], MAX_PARENT_PROOF_BYTES).is_err());
    }
}
