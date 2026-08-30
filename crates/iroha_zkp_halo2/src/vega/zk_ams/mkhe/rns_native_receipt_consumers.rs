//! Move-only consumers for verified RNS-native algebraic receipts.
//!
//! The composite verifier remains fail-closed while any production proof
//! stage is unavailable. If that verifier later succeeds, its single opaque
//! receipt can be consumed in exactly one of two ways:
//!
//! - one terminal-materialization use bound to the exact materialized state;
//! - eight party-indexed split-decryption uses bound to one exact statement.
//!
//! Neither use is a release/readiness certificate. The types have private
//! seals, no codec, no default, and no clone/copy surface. Their constructors
//! accept only the move-only algebraic receipt, and every consumer revalidates
//! all public axes before burning the use.

use super::{
    ZkAmsMkheErrorV1,
    decryption::ZkAmsMkheStreamingDecryptionStatementV1,
    manifest::ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
    phase23_encrypted::{
        ZkAmsPhase23MaterializedAccumulatorsV1, validate_materialized_accumulators_v1,
    },
    rns_native_composite_verifier::ZkAmsMkheRnsNativeAlgebraicReceiptV1,
    rns_native_profile::{
        zk_ams_mkhe_rns_native_profile_manifest_v1, zk_ams_mkhe_rns_native_profile_v1,
    },
    terminal::{ZkAmsPhase3TerminalContextV1, validate_terminal_context},
};
use crate::vega::sponge::Keccak256;

const RECEIPT_CONSUMER_VERSION_V1: u8 = 1;
const SPLIT_DECRYPTION_USE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native.split-decryption-use";
const TERMINAL_MATERIALIZATION_USE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native.terminal-materialization-use";

struct SplitDecryptionUseSealV1;

/// One exact party's non-reusable RNS-Link capability for staged decryption.
///
/// Construction consumes no caller-supplied success flag or raw digest shell:
/// the only public constructor consumes a verified algebraic receipt and
/// returns the complete ordered eight-use set at once.
#[must_use = "dropping this value burns one verified split-decryption use"]
pub struct ZkAmsMkheRnsNativeSplitDecryptionUseV1 {
    _seal: SplitDecryptionUseSealV1,
    version: u8,
    profile_manifest_digest: [u8; 32],
    profile_digest: [u8; 32],
    release_candidate_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    algebraic_receipt_digest: [u8; 32],
    verifier_context_digest: [u8; 32],
    opening_commitment_root: [u8; 32],
    verifier_transport_digest: [u8; 32],
    roster_digest: [u8; 32],
    ciphertext_digest: [u8; 32],
    statement_digest: [u8; 32],
    operational_context_digest: [u8; 32],
    party_index: u8,
    use_digest: [u8; 32],
}

impl core::fmt::Debug for ZkAmsMkheRnsNativeSplitDecryptionUseV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheRnsNativeSplitDecryptionUseV1")
            .field("party_index", &self.party_index)
            .field("use_digest", &hex::encode(self.use_digest))
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheRnsNativeSplitDecryptionUseV1 {
    fn validate_for_statement_v1(
        &self,
        statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
        party_index: usize,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let profile = zk_ams_mkhe_rns_native_profile_v1()?;
        let manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()?;
        let expected_party =
            u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidShareSet)?;
        if self.version != RECEIPT_CONSUMER_VERSION_V1
            || party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || self.party_index != expected_party
            || self.profile_manifest_digest != manifest.manifest_digest
            || self.profile_digest != profile.profile_digest
            || statement.roster().profile_digest() != self.profile_digest
            || statement.roster().roster_digest() != self.roster_digest
            || statement.ciphertext_digest() != self.ciphertext_digest
            || statement.rns_link_statement_digest_v1() != self.statement_digest
            || statement.rns_link_operational_context_digest_v1() != self.operational_context_digest
            || self.use_digest == [0; 32]
            || self.use_digest != split_decryption_use_digest_v1(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidShareSet);
        }
        Ok(())
    }

    /// Consume this exact party use at the staged prover boundary.
    pub(super) fn consume_for_split_decryption_v1(
        self,
        statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
        party_index: usize,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.validate_for_statement_v1(statement, party_index)
    }
}

/// Consume one verified RNS algebraic receipt into exactly eight party uses.
///
/// The receipt statement and operational-context digests must equal the
/// canonical values derived by the live decryption statement. The corrected
/// 40-limb profile is required explicitly, so today's legacy 38-limb runtime
/// cannot obtain these capabilities by relabelling its result.
///
/// # Errors
///
/// Returns [`ZkAmsMkheErrorV1`] if the opaque receipt fails revalidation or
/// any corrected profile, roster, ciphertext, statement, or replay-context
/// identity differs from the live decryption statement.
pub fn bind_zk_ams_mkhe_rns_native_split_decryption_uses_v1(
    receipt: ZkAmsMkheRnsNativeAlgebraicReceiptV1,
    statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
) -> Result<
    [ZkAmsMkheRnsNativeSplitDecryptionUseV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ZkAmsMkheErrorV1,
> {
    receipt
        .validate_v1()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidShareSet)?;
    let profile = zk_ams_mkhe_rns_native_profile_v1()?;
    let manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()?;
    let statement_digest = statement.rns_link_statement_digest_v1();
    let operational_context_digest = statement.rns_link_operational_context_digest_v1();
    if statement.roster().profile_digest() != profile.profile_digest
        || receipt.profile_manifest_digest() != manifest.manifest_digest
        || receipt.governed_roster_digest() != statement.roster().roster_digest()
        || receipt.public_ciphertext_digest() != statement.ciphertext_digest()
        || receipt.statement_digest() != statement_digest
        || receipt.operational_context_digest() != operational_context_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidShareSet);
    }
    let common = ReceiptConsumerAxesV1::from_receipt_v1(
        &receipt,
        profile.profile_digest,
        statement.roster().roster_digest(),
        statement.ciphertext_digest(),
        statement_digest,
        operational_context_digest,
    )?;
    Ok(core::array::from_fn(|party_index| {
        let mut use_value = ZkAmsMkheRnsNativeSplitDecryptionUseV1 {
            _seal: SplitDecryptionUseSealV1,
            version: RECEIPT_CONSUMER_VERSION_V1,
            profile_manifest_digest: common.profile_manifest_digest,
            profile_digest: common.profile_digest,
            release_candidate_digest: common.release_candidate_digest,
            source_binding_digest: common.source_binding_digest,
            algebraic_receipt_digest: common.algebraic_receipt_digest,
            verifier_context_digest: common.verifier_context_digest,
            opening_commitment_root: common.opening_commitment_root,
            verifier_transport_digest: common.verifier_transport_digest,
            roster_digest: common.roster_digest,
            ciphertext_digest: common.ciphertext_digest,
            statement_digest: common.statement_digest,
            operational_context_digest: common.operational_context_digest,
            party_index: u8::try_from(party_index).expect("the governed roster has eight parties"),
            use_digest: [0; 32],
        };
        use_value.use_digest = split_decryption_use_digest_v1(&use_value);
        use_value
    }))
}

fn split_decryption_use_digest_v1(use_value: &ZkAmsMkheRnsNativeSplitDecryptionUseV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(SPLIT_DECRYPTION_USE_DOMAIN_V1);
    hash.update(&[use_value.version]);
    hash.update(&use_value.profile_manifest_digest);
    hash.update(&use_value.profile_digest);
    hash.update(&use_value.release_candidate_digest);
    hash.update(&use_value.source_binding_digest);
    hash.update(&use_value.algebraic_receipt_digest);
    hash.update(&use_value.verifier_context_digest);
    hash.update(&use_value.opening_commitment_root);
    hash.update(&use_value.verifier_transport_digest);
    hash.update(&use_value.roster_digest);
    hash.update(&use_value.ciphertext_digest);
    hash.update(&use_value.statement_digest);
    hash.update(&use_value.operational_context_digest);
    hash.update(&[use_value.party_index]);
    hash.finalize()
}

struct TerminalMaterializationUseSealV1;

/// Single-use RNS-Link capability for one exact terminal materialization.
#[must_use = "dropping this value burns the verified terminal-materialization use"]
pub struct ZkAmsMkheRnsNativeTerminalMaterializationUseV1 {
    _seal: TerminalMaterializationUseSealV1,
    version: u8,
    profile_manifest_digest: [u8; 32],
    profile_digest: [u8; 32],
    release_candidate_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    algebraic_receipt_digest: [u8; 32],
    verifier_context_digest: [u8; 32],
    opening_commitment_root: [u8; 32],
    verifier_transport_digest: [u8; 32],
    roster_digest: [u8; 32],
    public_ciphertext_digest: [u8; 32],
    statement_digest: [u8; 32],
    operational_context_digest: [u8; 32],
    use_digest: [u8; 32],
}

impl core::fmt::Debug for ZkAmsMkheRnsNativeTerminalMaterializationUseV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheRnsNativeTerminalMaterializationUseV1")
            .field("use_digest", &hex::encode(self.use_digest))
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheRnsNativeTerminalMaterializationUseV1 {
    /// Consume this exact use before the terminal prover opens materialized rows.
    pub(super) fn consume_for_terminal_materialization_v1(
        self,
        context: ZkAmsPhase3TerminalContextV1,
        materialized: &ZkAmsPhase23MaterializedAccumulatorsV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let (statement_digest, operational_context_digest) =
            zk_ams_mkhe_rns_native_terminal_materialization_binding_v1(context, materialized)?;
        let manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()?;
        if self.version != RECEIPT_CONSUMER_VERSION_V1
            || self.profile_manifest_digest != manifest.manifest_digest
            || self.profile_digest != materialized.profile_digest
            || self.roster_digest != materialized.roster_digest
            || self.statement_digest != statement_digest
            || self.operational_context_digest != operational_context_digest
            || self.use_digest == [0; 32]
            || self.use_digest != terminal_materialization_use_digest_v1(&self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}

/// Return the exact source-layout bindings for one terminal materialization.
///
/// The materialized-state digest is the RNS statement and the complete
/// terminal-context digest is the operational/replay context. This function
/// rejects the legacy 38-limb profile rather than allowing it to be relabelled
/// as the corrected 40-limb RNS profile.
///
/// # Errors
///
/// Returns [`ZkAmsMkheErrorV1`] if either owner is invalid, uses the legacy
/// profile, or disagrees on batch, roster, transcript, or ordered input axes.
pub fn zk_ams_mkhe_rns_native_terminal_materialization_binding_v1(
    context: ZkAmsPhase3TerminalContextV1,
    materialized: &ZkAmsPhase23MaterializedAccumulatorsV1,
) -> Result<([u8; 32], [u8; 32]), ZkAmsMkheErrorV1> {
    validate_terminal_context(context)?;
    validate_materialized_accumulators_v1(materialized)?;
    let profile = zk_ams_mkhe_rns_native_profile_v1()?;
    if context.profile_digest != profile.profile_digest
        || materialized.profile_digest != profile.profile_digest
        || context.roster_digest != materialized.roster_digest
        || context.transcript_digest != materialized.transcript_digest
        || context.batch_id != materialized.batch_id
        || context.ordered_batch_input_digest != materialized.ordered_batch_input_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok((materialized.digest, context.digest))
}

/// Consume one verified RNS receipt into one terminal-materialization use.
///
/// # Errors
///
/// Returns [`ZkAmsMkheErrorV1`] if the receipt is invalid or its exact
/// corrected-profile statement/replay axes differ from the materialization.
pub fn bind_zk_ams_mkhe_rns_native_terminal_materialization_use_v1(
    receipt: ZkAmsMkheRnsNativeAlgebraicReceiptV1,
    context: ZkAmsPhase3TerminalContextV1,
    materialized: &ZkAmsPhase23MaterializedAccumulatorsV1,
) -> Result<ZkAmsMkheRnsNativeTerminalMaterializationUseV1, ZkAmsMkheErrorV1> {
    receipt
        .validate_v1()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let profile = zk_ams_mkhe_rns_native_profile_v1()?;
    let manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()?;
    let (statement_digest, operational_context_digest) =
        zk_ams_mkhe_rns_native_terminal_materialization_binding_v1(context, materialized)?;
    if receipt.profile_manifest_digest() != manifest.manifest_digest
        || receipt.governed_roster_digest() != context.roster_digest
        || receipt.statement_digest() != statement_digest
        || receipt.operational_context_digest() != operational_context_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let common = ReceiptConsumerAxesV1::from_receipt_v1(
        &receipt,
        profile.profile_digest,
        context.roster_digest,
        receipt.public_ciphertext_digest(),
        statement_digest,
        operational_context_digest,
    )?;
    let mut use_value = ZkAmsMkheRnsNativeTerminalMaterializationUseV1 {
        _seal: TerminalMaterializationUseSealV1,
        version: RECEIPT_CONSUMER_VERSION_V1,
        profile_manifest_digest: common.profile_manifest_digest,
        profile_digest: common.profile_digest,
        release_candidate_digest: common.release_candidate_digest,
        source_binding_digest: common.source_binding_digest,
        algebraic_receipt_digest: common.algebraic_receipt_digest,
        verifier_context_digest: common.verifier_context_digest,
        opening_commitment_root: common.opening_commitment_root,
        verifier_transport_digest: common.verifier_transport_digest,
        roster_digest: common.roster_digest,
        public_ciphertext_digest: common.ciphertext_digest,
        statement_digest: common.statement_digest,
        operational_context_digest: common.operational_context_digest,
        use_digest: [0; 32],
    };
    use_value.use_digest = terminal_materialization_use_digest_v1(&use_value);
    Ok(use_value)
}

fn terminal_materialization_use_digest_v1(
    use_value: &ZkAmsMkheRnsNativeTerminalMaterializationUseV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(TERMINAL_MATERIALIZATION_USE_DOMAIN_V1);
    hash.update(&[use_value.version]);
    hash.update(&use_value.profile_manifest_digest);
    hash.update(&use_value.profile_digest);
    hash.update(&use_value.release_candidate_digest);
    hash.update(&use_value.source_binding_digest);
    hash.update(&use_value.algebraic_receipt_digest);
    hash.update(&use_value.verifier_context_digest);
    hash.update(&use_value.opening_commitment_root);
    hash.update(&use_value.verifier_transport_digest);
    hash.update(&use_value.roster_digest);
    hash.update(&use_value.public_ciphertext_digest);
    hash.update(&use_value.statement_digest);
    hash.update(&use_value.operational_context_digest);
    hash.finalize()
}

#[derive(Clone, Copy)]
struct ReceiptConsumerAxesV1 {
    profile_manifest_digest: [u8; 32],
    profile_digest: [u8; 32],
    release_candidate_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    algebraic_receipt_digest: [u8; 32],
    verifier_context_digest: [u8; 32],
    opening_commitment_root: [u8; 32],
    verifier_transport_digest: [u8; 32],
    roster_digest: [u8; 32],
    ciphertext_digest: [u8; 32],
    statement_digest: [u8; 32],
    operational_context_digest: [u8; 32],
}

impl ReceiptConsumerAxesV1 {
    #[allow(clippy::too_many_arguments)]
    fn from_receipt_v1(
        receipt: &ZkAmsMkheRnsNativeAlgebraicReceiptV1,
        profile_digest: [u8; 32],
        roster_digest: [u8; 32],
        ciphertext_digest: [u8; 32],
        statement_digest: [u8; 32],
        operational_context_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let axes = Self {
            profile_manifest_digest: receipt.profile_manifest_digest(),
            profile_digest,
            release_candidate_digest: receipt.release_candidate_digest(),
            source_binding_digest: receipt.source_binding_digest(),
            algebraic_receipt_digest: receipt.receipt_digest(),
            verifier_context_digest: receipt.verifier_context_digest(),
            opening_commitment_root: receipt.opening_commitment_root(),
            verifier_transport_digest: receipt.verifier_transport_digest(),
            roster_digest,
            ciphertext_digest,
            statement_digest,
            operational_context_digest,
        };
        let digests = [
            axes.profile_manifest_digest,
            axes.profile_digest,
            axes.release_candidate_digest,
            axes.source_binding_digest,
            axes.algebraic_receipt_digest,
            axes.verifier_context_digest,
            axes.opening_commitment_root,
            axes.verifier_transport_digest,
            axes.roster_digest,
            axes.ciphertext_digest,
            axes.statement_digest,
            axes.operational_context_digest,
        ];
        if digests.contains(&[0; 32])
            || digests
                .iter()
                .enumerate()
                .any(|(index, digest)| digests[index + 1..].contains(digest))
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(axes)
    }
}
