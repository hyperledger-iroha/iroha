//! Non-shipping canonical action builders for four-peer release gates.
//!
//! The parent module is compiled only with `privacy-release-evidence`. These helpers retain PQ
//! witness material inside `iroha_core` and return ordinary production `SignedTransaction` and
//! bootstrap data-model values. Network execution therefore traverses the same Torii, DA/RBC,
//! verifier, and ledger paths as any externally constructed action.
use iroha_crypto::PrivateKey;
use iroha_data_model::{
    metadata::Metadata,
    prelude::{AccountId, AssetDefinitionId, NetworkId},
    privacy::{
        OrchardHalo2ActionsStatementV1, PqMaspStarkStatementV1, PrivacyConsensusLimitsV1,
        PrivacyJindoFieldElementV1, PrivacyNativeConsensusBindingV1, PrivacyOrchardActionV1,
        PrivacyOrchardNullifierProvenanceV1, PrivacyOrchardPoolBootstrapV1,
        PrivacyOrchardPoolStateViewV1, PrivacyPoolIdV1, PrivacyPqMaspPoolBootstrapV1,
        PrivacyProofBytesV1, PrivacyProofEnvelopeV1, PrivacyProofManagedPoolBootstrapV1,
        PrivacyProofV1, PrivacyStatementContextV1, PrivacyStatementDigestV1, PrivacyStatementV1,
        PrivacyTransactionIntentDigestV1, PrivacyValueBalanceDirectionV1, PrivacyValueBalanceV1,
        PrivacyVegaIssuerRecordV1, PrivacyVegaMdlDateV1,
    },
    query::privacy::prelude::{
        FindPrivacyOrchardNullifierV1, FindPrivacyOrchardPoolStateV1,
        FindPrivacyProofManagedPoolStateV1,
    },
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionPayload},
};
use sha2::{Digest as _, Sha256};
use std::{num::NonZeroU32, time::Duration};
mod retained;
use super::{
    EvidenceRng06, EvidenceRng09, PrivacyReleaseEvidenceErrorClassV1, authorize_orchard_bundle_v1,
    compiled_privacy_profile_v1, orchard_empty_root_v1, orchard_spending_key_v1,
    pq_masp_release_fixture_v1, pq_masp_release_successor_replay_fixture_v1,
    prepare_orchard_bundle_v1_with_rng, prove_pq_masp_v1_with_rng,
};
use crate::privacy_engines::{
    jindo::{
        JindoPrivacyActionTransactionContextV1, JindoPrivacyActionWitnessV1,
        build_signed_privacy_action_with_rng_v1,
    },
    orchard::{
        OrchardBundleDraftV1, OrchardChangeProverInputV1, OrchardPreparedBundleV1,
        OrchardProvedBundleV1, Scope, append_orchard_commitments_v1,
        orchard_singleton_output_witness_v1, recover_orchard_spend_prover_input_v1,
    },
    vega::{
        VegaPrivacyActionTransactionContextV1, VegaPrivacyActionWitnessMaterialV1,
        build_signed_vega_privacy_action_with_rng_v1,
    },
};
pub use retained::{
    PrivacyReleaseAnonymousPgcNetworkActionV1, PrivacyReleaseBootleLanternNetworkActionV1,
    PrivacyReleaseFcmpNetworkActionV1, PrivacyReleaseIvmPrivateNoteNetworkActionV1,
    PrivacyReleaseVeRangeNetworkActionV1, PrivacyReleaseZkAceNetworkActionV1,
    build_privacy_release_anonymous_pgc_network_action_v1,
    build_privacy_release_bootle_lantern_network_action_v1,
    build_privacy_release_fcmp_network_action_v1,
    build_privacy_release_ivm_private_note_network_action_v1,
    build_privacy_release_verange_network_action_v1,
    build_privacy_release_zk_ace_network_action_v1,
};
/// Exact transaction and consensus context used by one non-shipping network action builder.
#[derive(Clone, Debug)]
pub struct PrivacyReleaseTransactionContextV1 {
    /// Exact genesis-header-derived transaction security domain.
    pub network_id: NetworkId,
    /// Actual submitting account.
    pub authority: AccountId,
    /// Canonical transaction creation time.
    pub creation_time: Duration,
    /// Optional canonical transaction TTL.
    pub time_to_live: Option<Duration>,
    /// Optional nonzero transaction nonce.
    pub nonce: Option<NonZeroU32>,
    /// Exact fee-payment intent.
    pub fee_payment: FeePaymentIntent,
    /// Exact transaction metadata.
    pub metadata: Metadata,
    /// Actual canonical genesis block hash queried from the network.
    pub genesis_hash: [u8; 32],
}
/// One canonical output-only Orchard deposit and its governed pool bootstrap.
#[derive(Clone, Debug)]
pub struct PrivacyReleaseOrchardNetworkActionV1 {
    /// Ordinary production transaction carrying exactly one Orchard submit.
    pub transaction: SignedTransaction,
    /// Immutable bootstrap required by the action's pool.
    pub bootstrap: PrivacyOrchardPoolBootstrapV1,
    /// Exact statement carried by `transaction`.
    pub statement: OrchardHalo2ActionsStatementV1,
}
/// Ordered funding and real-spend Orchard transitions for semantic network qualification.
///
/// The first transaction shields 23 public units into the governed reserve and
/// creates a wallet note. The second consumes that exact note, returns six units as a
/// shielded wallet-owned change output, and bridges 17 public units to a
/// distinct receiver authority. Post-NU6.3 Orchard deliberately forbids a
/// shielded cross-address receiver output, so the receiver leg uses the genuine
/// governed public reserve bridge rather than relabeling sender change.
#[derive(Clone, Debug)]
pub struct PrivacyReleaseOrchardNetworkActionsV1 {
    /// Must commit first and reach terminal applied state.
    pub funding_transaction: SignedTransaction,
    /// Must commit only after the funding transaction is finalized.
    pub spend_transaction: SignedTransaction,
    /// Immutable bootstrap required by both transitions.
    pub bootstrap: PrivacyOrchardPoolBootstrapV1,
    /// Exact statement carried by `funding_transaction`.
    pub funding_statement: OrchardHalo2ActionsStatementV1,
    /// Exact statement carried by `spend_transaction`.
    pub spend_statement: OrchardHalo2ActionsStatementV1,
    /// Exact real-note nullifier that the spend must consume.
    pub spent_note_nullifier: [u8; 32],
    /// Public account that receives the 17-unit reserve-bridge output.
    pub receiver_account: AccountId,
    /// Exact funded note value.
    pub funded_value: u64,
    /// Exact public receiver value.
    pub receiver_value: u64,
    /// Exact shielded sender-change value.
    pub change_value: u64,
    /// Exact root after the funding commitment and two spend commitments.
    pub expected_final_root: [u8; 32],
    /// Typed finalized pool-state query expected to report epoch three/tree size three.
    pub state_query: FindPrivacyOrchardPoolStateV1,
    /// Typed finalized replay-marker query for the consumed real-note nullifier.
    pub nullifier_query: FindPrivacyOrchardNullifierV1,
}
impl PrivacyReleaseOrchardNetworkActionsV1 {
    /// Validate the two typed finalized query results against this exact
    /// funding/spend artifact pair.
    ///
    /// This binds the durable epoch-three, three-commitment pool head and the
    /// consumed real-note nullifier to the same bootstrap, spend statement,
    /// admission action, finalized height, and finalized block. Public asset
    /// balance queries remain a controller responsibility; the signed spend
    /// transaction itself fixes their expected receiver and 17-unit delta.
    ///
    /// # Errors
    ///
    /// Rejects malformed query views or any artifact, pool-head, nullifier,
    /// admission, or finality binding that differs from this exact fixture.
    pub fn validate_finalized_post_state_v1(
        &self,
        pool_state: &PrivacyOrchardPoolStateViewV1,
        nullifier: &PrivacyOrchardNullifierProvenanceV1,
    ) -> Result<(), &'static str> {
        pool_state.validate()?;
        nullifier.validate()?;
        let bootstrap_digest = self
            .bootstrap
            .digest()
            .map_err(|_| "Orchard semantic bootstrap digest encoding failed")?;
        let spend_statement_digest =
            PrivacyStatementV1::OrchardHalo2ActionsV1(self.spend_statement.clone())
                .digest()
                .map_err(|_| "Orchard semantic spend statement digest encoding failed")?;
        let expected_tree_size = u64::try_from(
            self.funding_statement
                .actions
                .len()
                .checked_add(self.spend_statement.actions.len())
                .ok_or("Orchard semantic action count overflow")?,
        )
        .map_err(|_| "Orchard semantic action count does not fit u64")?;
        let latest = pool_state
            .latest_transition
            .ok_or("Orchard semantic post-state has no latest transition")?;
        let spend_nullifier_count = self
            .spend_statement
            .actions
            .iter()
            .filter(|action| action.nullifier == self.spent_note_nullifier)
            .count();

        if self.funded_value.checked_sub(self.receiver_value) != Some(self.change_value)
            || self.funding_transaction.authority() == self.spend_transaction.authority()
            || self.spend_transaction.authority() != &self.receiver_account
            || self.funding_statement.context.network_id != self.spend_statement.context.network_id
            || self.funding_statement.pool_id != self.bootstrap.pool_id
            || self.spend_statement.pool_id != self.bootstrap.pool_id
            || self.funding_statement.asset_definition_id != self.bootstrap.asset_definition_id
            || self.spend_statement.asset_definition_id != self.bootstrap.asset_definition_id
            || self.funding_statement.anchor_epoch != 1
            || self.spend_statement.anchor_epoch != 2
            || self.funding_statement.value_balance.direction
                != PrivacyValueBalanceDirectionV1::IntoPool
            || self.funding_statement.value_balance.amount != u128::from(self.funded_value)
            || self.spend_statement.value_balance.direction
                != PrivacyValueBalanceDirectionV1::OutOfPool
            || self.spend_statement.value_balance.amount != u128::from(self.receiver_value)
            || self.funding_statement.actions.len() != 1
            || self.spend_statement.actions.len() != 2
            || spend_nullifier_count != 1
        {
            return Err("Orchard semantic artifact pair violates its fixed value or action shape");
        }
        if pool_state.network_id != self.spend_statement.context.network_id
            || pool_state.pool_id != self.bootstrap.pool_id
            || pool_state.asset_definition_id != self.bootstrap.asset_definition_id
            || pool_state.public_balance_scope != self.bootstrap.public_balance_scope
            || pool_state.reserve_account != self.bootstrap.reserve_account
            || pool_state.bootstrap_digest != bootstrap_digest
            || pool_state.current_epoch != 3
            || pool_state.current_root.as_bytes() != &self.expected_final_root
            || pool_state.tree_size != expected_tree_size
            || latest.successor_epoch != 3
            || latest.parent_epoch != self.spend_statement.anchor_epoch
            || latest.parent_root != self.spend_statement.anchor
            || latest.statement_digest != spend_statement_digest
        {
            return Err("Orchard finalized pool state does not match the semantic artifact pair");
        }
        if nullifier.network_id != pool_state.network_id
            || nullifier.pool_id != self.bootstrap.pool_id
            || nullifier.nullifier != self.spent_note_nullifier
            || nullifier.bootstrap_digest != bootstrap_digest
            || nullifier.statement_digest != spend_statement_digest
            || nullifier.admitted_at_height != latest.admitted_at_height
            || nullifier.action_index != latest.action_index
            || nullifier.finalized_height != pool_state.finalized_height
            || nullifier.finalized_block_hash != pool_state.finalized_block_hash
        {
            return Err(
                "Orchard finalized nullifier provenance does not match the semantic transition",
            );
        }
        Ok(())
    }
}
/// One canonical verification-only revised-Jindo action.
#[derive(Debug)]
pub struct PrivacyReleaseJindoNetworkActionV1 {
    /// Ordinary production transaction carrying exactly one revised-Jindo proof.
    pub transaction: SignedTransaction,
}
/// One canonical verification-only Vega presentation and its issuer revision.
#[derive(Debug)]
pub struct PrivacyReleaseVegaNetworkActionV1 {
    /// Ordinary production transaction carrying exactly one Vega proof.
    pub transaction: SignedTransaction,
    /// Exact active issuer record validators must register before submission.
    pub issuer_record: PrivacyVegaIssuerRecordV1,
}
/// Four independently proved PQ-MASP actions consuming the same genesis note.
///
/// Each transaction has a distinct canonical intent and proof. The first is a valid pre-activation
/// probe; the second is expected to apply after activation; the remaining two carry the same stable
/// nullifier as protocol replay probes rather than duplicate transaction-hash probes. Keeping the
/// fourth transaction fresh lets a restart gate prove that both the successor frontier and
/// consumed-nullifier set were recovered by the restarted peer.
#[derive(Clone, Debug)]
pub struct PrivacyReleasePqMaspNetworkActionsV1 {
    /// Valid ordinary production transaction submitted before activation.
    pub preactivation_transaction: SignedTransaction,
    /// Ordinary production transaction expected to apply after activation.
    pub canonical_transaction: SignedTransaction,
    /// Independently proved and signed stable-nullifier replay transaction.
    pub replay_transaction: SignedTransaction,
    /// Fresh stable-nullifier replay submitted through a restarted peer.
    pub post_restart_replay_transaction: SignedTransaction,
    /// Immutable proof-managed bootstrap authenticating the consumed note.
    pub bootstrap: PrivacyProofManagedPoolBootstrapV1,
    /// Exact statement carried by the pre-activation transaction.
    pub preactivation_statement: PqMaspStarkStatementV1,
    /// Exact canonical statement carried by the applying transaction.
    pub canonical_statement: PqMaspStarkStatementV1,
    /// Exact replay statement carrying the same stable nullifier.
    pub replay_statement: PqMaspStarkStatementV1,
    /// Exact fresh replay statement used after peer restart.
    pub post_restart_replay_statement: PqMaspStarkStatementV1,
    /// Typed finalized pool-state query used to bind the canonical transition.
    pub state_query: FindPrivacyProofManagedPoolStateV1,
}
fn network_seed_v1(master: [u8; 32], purpose: &[u8], index: u8) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"iroha.privacy.release.network-action.seed.v1");
    hash.update(master);
    hash.update(
        u64::try_from(purpose.len())
            .expect("in-memory purpose length fits u64")
            .to_be_bytes(),
    );
    hash.update(purpose);
    hash.update([index]);
    let mut seed: [u8; 32] = hash.finalize().into();
    if seed.iter().all(|byte| *byte == 0) {
        seed[0] = 1;
    }
    seed
}
fn statement_context_v1(
    transaction: &PrivacyReleaseTransactionContextV1,
    profile: crate::privacy_profiles::CompiledPrivacyProfileV1,
) -> PrivacyStatementContextV1 {
    PrivacyStatementContextV1 {
        network_id: transaction.network_id,
        action_index: 0,
        transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0; 32]),
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
    }
}
fn transaction_payload_v1(
    context: &PrivacyReleaseTransactionContextV1,
    envelope: PrivacyProofEnvelopeV1,
) -> Result<TransactionPayload, PrivacyReleaseEvidenceErrorClassV1> {
    if context.network_id.as_bytes() != &context.genesis_hash {
        return Err(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed);
    }
    let mut builder = TransactionBuilder::new(
        context.network_id,
        context.authority.clone(),
        context.fee_payment.clone(),
    )
    .with_instructions([iroha_data_model::isi::privacy::SubmitPrivacyProofV1::new(
        envelope,
    )])
    .with_metadata(context.metadata.clone());
    builder.set_creation_time(context.creation_time);
    if let Some(ttl) = context.time_to_live {
        builder.set_ttl(ttl);
    }
    if let Some(nonce) = context.nonce {
        builder.set_nonce(nonce);
    }
    builder
        .into_payload()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
}
fn signed_payload_v1(
    payload: TransactionPayload,
    expected_intent: PrivacyTransactionIntentDigestV1,
    private_key: &PrivateKey,
) -> Result<SignedTransaction, PrivacyReleaseEvidenceErrorClassV1> {
    let transaction = TransactionBuilder::from_payload(payload)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?
        .try_sign(private_key)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    transaction
        .verify_signature()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let (observed_intent, _) = transaction
        .privacy_transaction_intent_binding_if_present_v1()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    if observed_intent != expected_intent {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok(transaction)
}
fn orchard_statement_v1(
    context: PrivacyStatementContextV1,
    draft: &OrchardBundleDraftV1,
    asset_definition_id: AssetDefinitionId,
    pool_id: PrivacyPoolIdV1,
    anchor_epoch: u64,
    expiry_height: u64,
) -> Result<OrchardHalo2ActionsStatementV1, PrivacyReleaseEvidenceErrorClassV1> {
    if anchor_epoch == 0 {
        return Err(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed);
    }
    let value_balance = match draft.value_balance.cmp(&0) {
        core::cmp::Ordering::Equal => PrivacyValueBalanceV1::balanced(),
        core::cmp::Ordering::Less => PrivacyValueBalanceV1 {
            direction: PrivacyValueBalanceDirectionV1::IntoPool,
            amount: u128::from(draft.value_balance.unsigned_abs()),
        },
        core::cmp::Ordering::Greater => PrivacyValueBalanceV1 {
            direction: PrivacyValueBalanceDirectionV1::OutOfPool,
            amount: u128::from(draft.value_balance.unsigned_abs()),
        },
    };
    Ok(OrchardHalo2ActionsStatementV1 {
        context,
        asset_definition_id,
        public_balance_scope: iroha_data_model::asset::AssetBalanceScope::Global,
        pool_id,
        anchor: iroha_data_model::privacy::PrivacyRootV1::new(draft.anchor),
        anchor_epoch,
        actions: draft
            .actions
            .iter()
            .map(|action| PrivacyOrchardActionV1 {
                nullifier: action.nullifier,
                randomized_key: action.randomized_key,
                note_commitment: action.note_commitment,
                ephemeral_key: action.ephemeral_key,
                encrypted_note: action.encrypted_note.to_vec(),
                outgoing_ciphertext: action.outgoing_ciphertext.to_vec(),
                value_commitment: action.value_commitment,
            })
            .collect(),
        value_balance,
        expiry_height,
    })
}
fn jindo_field_v1(value: u64) -> PrivacyJindoFieldElementV1 {
    let mut encoding = [0_u8; 32];
    encoding[..8].copy_from_slice(&value.to_le_bytes());
    PrivacyJindoFieldElementV1::new(encoding)
}
fn vega_utc_date_from_timestamp_ms_v1(
    timestamp_ms: u64,
) -> Result<PrivacyVegaMdlDateV1, PrivacyReleaseEvidenceErrorClassV1> {
    let days = i64::try_from(timestamp_ms / 86_400_000)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let shifted = days
        .checked_add(719_468)
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    Ok(PrivacyVegaMdlDateV1 {
        year: u16::try_from(year)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?,
        month: u8::try_from(month)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?,
        day: u8::try_from(day)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?,
    })
}
/// Build one canonical network-bound revised-Jindo action.
///
/// The polynomial coefficients, evaluation point, proof randomness, and transaction signing key
/// remain native Rust values for their complete lifetime. This builder makes no claim about the
/// deliberately exposed distribution-wide knowledge-soundness limitation.
pub fn build_privacy_release_jindo_network_action_v1(
    transaction_context: PrivacyReleaseTransactionContextV1,
    fixture_seed: [u8; 32],
    private_key: &PrivateKey,
) -> Result<PrivacyReleaseJindoNetworkActionV1, PrivacyReleaseEvidenceErrorClassV1> {
    let context = JindoPrivacyActionTransactionContextV1 {
        network_id: transaction_context.network_id,
        authority: transaction_context.authority,
        creation_time: transaction_context.creation_time,
        time_to_live: transaction_context.time_to_live,
        nonce: transaction_context.nonce,
        fee_payment: transaction_context.fee_payment,
        metadata: transaction_context.metadata,
    };
    let witness = JindoPrivacyActionWitnessV1::try_new(
        vec![
            vec![
                jindo_field_v1(3),
                jindo_field_v1(5),
                jindo_field_v1(7),
                jindo_field_v1(11),
            ],
            vec![jindo_field_v1(13), jindo_field_v1(17)],
            vec![jindo_field_v1(19), jindo_field_v1(23)],
            vec![jindo_field_v1(29), jindo_field_v1(31)],
        ],
        jindo_field_v1(37),
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let mut rng = EvidenceRng06::new(network_seed_v1(fixture_seed, b"jindo-proof", 0));
    let signed = build_signed_privacy_action_with_rng_v1(
        context,
        witness,
        transaction_context.genesis_hash,
        private_key,
        &mut rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    Ok(PrivacyReleaseJindoNetworkActionV1 {
        transaction: signed.into_signed_transaction(),
    })
}
/// Build one canonical network-bound Vega presentation.
///
/// The mDL document fragments, issuer signature, holder device key, proof
/// randomness, and transaction signing key never cross this native boundary.
pub fn build_privacy_release_vega_network_action_v1(
    transaction_context: PrivacyReleaseTransactionContextV1,
    fixture_seed: [u8; 32],
    private_key: &PrivateKey,
) -> Result<PrivacyReleaseVegaNetworkActionV1, PrivacyReleaseEvidenceErrorClassV1> {
    let mut fixture = super::vega::vega_release_fixture_v1()?;
    let trusted_timestamp_ms = u64::try_from(transaction_context.creation_time.as_millis())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    fixture.public_input.presentation_date =
        vega_utc_date_from_timestamp_ms_v1(trusted_timestamp_ms)?;
    let witness = VegaPrivacyActionWitnessMaterialV1::new(
        fixture.issuer_authentication_sig_structure,
        fixture.mobile_security_object_payload,
        fixture.birth_date_issuer_signed_item,
        &fixture.issuer_signature.to_bytes(),
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let context = VegaPrivacyActionTransactionContextV1 {
        network_id: transaction_context.network_id,
        authority: transaction_context.authority,
        creation_time: transaction_context.creation_time,
        time_to_live: transaction_context.time_to_live,
        nonce: transaction_context.nonce,
        fee_payment: transaction_context.fee_payment,
        metadata: transaction_context.metadata,
    };
    let mut rng = EvidenceRng06::new(network_seed_v1(fixture_seed, b"vega-proof", 0));
    let signed = build_signed_vega_privacy_action_with_rng_v1(
        context,
        fixture.public_input,
        witness,
        &fixture.device_signing_key,
        transaction_context.genesis_hash,
        trusted_timestamp_ms,
        private_key,
        &mut rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    Ok(PrivacyReleaseVegaNetworkActionV1 {
        transaction: signed.into_signed_transaction(),
        issuer_record: fixture.issuer_record,
    })
}
/// Build one canonical network-bound Orchard action through the production
/// two-phase prover and consuming authorization API.
///
/// This helper exists only in the non-default release-evidence feature.
fn finalize_orchard_network_transaction_v1(
    transaction_context: &PrivacyReleaseTransactionContextV1,
    pool_id: PrivacyPoolIdV1,
    asset_definition_id: AssetDefinitionId,
    anchor_epoch: u64,
    expiry_height: u64,
    prepared: OrchardPreparedBundleV1,
    private_key: &PrivateKey,
) -> Result<
    (
        SignedTransaction,
        OrchardHalo2ActionsStatementV1,
        OrchardProvedBundleV1,
    ),
    PrivacyReleaseEvidenceErrorClassV1,
> {
    let profile = compiled_privacy_profile_v1(
        iroha_data_model::privacy::PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let mut statement = orchard_statement_v1(
        statement_context_v1(transaction_context, profile),
        prepared.public_draft(),
        asset_definition_id,
        pool_id,
        anchor_epoch,
        expiry_height,
    )?;
    let draft_envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest: PrivacyStatementDigestV1::new([0; 32]),
        statement: PrivacyStatementV1::OrchardHalo2ActionsV1(statement.clone()),
        proof: PrivacyProofV1::OrchardHalo2ActionsV1(PrivacyProofBytesV1::new(Vec::new())),
    };
    let intent = transaction_payload_v1(transaction_context, draft_envelope)?
        .privacy_transaction_intent_digest_v1()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    if intent.is_zero() {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    statement.context.transaction_intent_digest = intent;
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let consensus_binding = PrivacyNativeConsensusBindingV1::new(
        &statement.context,
        transaction_context.genesis_hash,
        &limits,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let proved = authorize_orchard_bundle_v1(prepared, consensus_binding, &limits)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    if proved.public.anchor != *statement.anchor.as_bytes()
        || proved.public.actions.len() != statement.actions.len()
        || proved
            .public
            .actions
            .iter()
            .zip(&statement.actions)
            .any(|(native, typed)| {
                native.nullifier != typed.nullifier
                    || native.randomized_key != typed.randomized_key
                    || native.note_commitment != typed.note_commitment
                    || native.ephemeral_key != typed.ephemeral_key
                    || native.encrypted_note.as_slice() != typed.encrypted_note.as_slice()
                    || native.outgoing_ciphertext.as_slice() != typed.outgoing_ciphertext.as_slice()
                    || native.value_commitment != typed.value_commitment
            })
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let typed_statement = PrivacyStatementV1::OrchardHalo2ActionsV1(statement.clone());
    let envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest: typed_statement
            .digest()
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
        statement: typed_statement,
        proof: PrivacyProofV1::OrchardHalo2ActionsV1(PrivacyProofBytesV1::new(
            proved.authorization.clone(),
        )),
    };
    let payload = transaction_payload_v1(transaction_context, envelope)?;
    if payload
        .validate_privacy_transaction_intent_binding_v1()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
        != intent
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let transaction = signed_payload_v1(payload, intent, private_key)?;
    Ok((transaction, statement, proved))
}

/// Build the deterministic single-transaction Orchard release fixture.
pub fn build_privacy_release_orchard_network_action_v1(
    transaction_context: PrivacyReleaseTransactionContextV1,
    pool_id: PrivacyPoolIdV1,
    asset_definition_id: AssetDefinitionId,
    reserve_account: AccountId,
    expiry_height: u64,
    fixture_seed: [u8; 32],
    private_key: &PrivateKey,
) -> Result<PrivacyReleaseOrchardNetworkActionV1, PrivacyReleaseEvidenceErrorClassV1> {
    let profile = compiled_privacy_profile_v1(
        iroha_data_model::privacy::PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let wallet_seed = fixture_seed[0].max(1);
    let change_value = 17_u64;
    let change = OrchardChangeProverInputV1::new(
        orchard_spending_key_v1(wallet_seed),
        Scope::External,
        u32::from(wallet_seed),
        change_value,
        [wallet_seed; 512],
    );
    let mut prover_rng = EvidenceRng09::new(network_seed_v1(fixture_seed, b"orchard-prepare", 0));
    let prepared = prepare_orchard_bundle_v1_with_rng(
        orchard_empty_root_v1(),
        Vec::new(),
        vec![change],
        1,
        &mut prover_rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let mut statement = orchard_statement_v1(
        statement_context_v1(&transaction_context, profile),
        prepared.public_draft(),
        asset_definition_id.clone(),
        pool_id,
        1,
        expiry_height,
    )?;
    let draft_envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest: PrivacyStatementDigestV1::new([0; 32]),
        statement: PrivacyStatementV1::OrchardHalo2ActionsV1(statement.clone()),
        proof: PrivacyProofV1::OrchardHalo2ActionsV1(PrivacyProofBytesV1::new(Vec::new())),
    };
    let intent = transaction_payload_v1(&transaction_context, draft_envelope)?
        .privacy_transaction_intent_digest_v1()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    if intent.is_zero() {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    statement.context.transaction_intent_digest = intent;
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let consensus_binding = PrivacyNativeConsensusBindingV1::new(
        &statement.context,
        transaction_context.genesis_hash,
        &limits,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let proved = authorize_orchard_bundle_v1(prepared, consensus_binding, &limits)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    if proved.public.anchor != *statement.anchor.as_bytes()
        || proved.public.actions.len() != statement.actions.len()
        || proved.public.value_balance
            != -i64::try_from(change_value).expect("fixed value fits i64")
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let typed_statement = PrivacyStatementV1::OrchardHalo2ActionsV1(statement.clone());
    let envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest: typed_statement
            .digest()
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
        statement: typed_statement,
        proof: PrivacyProofV1::OrchardHalo2ActionsV1(PrivacyProofBytesV1::new(
            proved.authorization,
        )),
    };
    let payload = transaction_payload_v1(&transaction_context, envelope)?;
    if payload
        .validate_privacy_transaction_intent_binding_v1()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
        != intent
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let transaction = signed_payload_v1(payload, intent, private_key)?;
    let bootstrap = PrivacyOrchardPoolBootstrapV1::new(
        pool_id,
        asset_definition_id,
        iroha_data_model::asset::AssetBalanceScope::Global,
        reserve_account,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    Ok(PrivacyReleaseOrchardNetworkActionV1 {
        transaction,
        bootstrap,
        statement,
    })
}

/// Build the ordered Orchard funding and real-note-spend semantic fixture.
///
/// `funding_context.authority` supplies 23 public units to the governed reserve
/// and receives one wallet-owned Orchard note. After that transaction is
/// finalized, `spend_context.authority` submits the successor action, receives
/// 17 public units from the reserve, and leaves six units in a wallet-owned
/// internal change note. The real funded-note nullifier is returned together
/// with typed finalized queries for both the pool transition and replay marker.
/// The bounded canary must use a freshly bootstrapped dedicated pool and admit
/// no intervening Orchard transition between these ordered submissions; the
/// typed post-state validator rejects any head other than epoch three.
///
/// A shielded cross-address recipient output is intentionally not offered by
/// this API: the pinned Post-NU6.3 profile uses
/// `Flags::CROSS_ADDRESS_DISABLED`, and its two-action cap is fully occupied by
/// the real spend plus wallet-owned change. Callers that require a shielded
/// recipient must fail closed until a separately governed profile supports it.
///
/// # Errors
///
/// Rejects mismatched network/authority contexts, invalid pool/bootstrap
/// fields, proof or authorization failure, intent/signature mismatch, output
/// recovery failure, or any fixed value/action/nullifier invariant violation.
pub fn build_privacy_release_orchard_network_actions_v1(
    funding_context: PrivacyReleaseTransactionContextV1,
    spend_context: PrivacyReleaseTransactionContextV1,
    pool_id: PrivacyPoolIdV1,
    asset_definition_id: AssetDefinitionId,
    reserve_account: AccountId,
    expiry_height: u64,
    fixture_seed: [u8; 32],
    funding_private_key: &PrivateKey,
    spend_private_key: &PrivateKey,
) -> Result<PrivacyReleaseOrchardNetworkActionsV1, PrivacyReleaseEvidenceErrorClassV1> {
    if funding_context.network_id != spend_context.network_id
        || funding_context.genesis_hash != spend_context.genesis_hash
        || funding_context.authority == spend_context.authority
        || funding_context.authority == reserve_account
        || spend_context.authority == reserve_account
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed);
    }

    const FUNDED_VALUE: u64 = 23;
    const RECEIVER_VALUE: u64 = 17;
    const CHANGE_VALUE: u64 = 6;
    if RECEIVER_VALUE.checked_add(CHANGE_VALUE) != Some(FUNDED_VALUE) {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }

    let wallet_seed = fixture_seed[0].max(1);
    let funding_output = OrchardChangeProverInputV1::new(
        orchard_spending_key_v1(wallet_seed),
        Scope::External,
        u32::from(wallet_seed),
        FUNDED_VALUE,
        [wallet_seed; 512],
    );
    let mut funding_rng = EvidenceRng09::new(network_seed_v1(
        fixture_seed,
        b"orchard-semantic-funding-prepare",
        0,
    ));
    let funding_prepared = prepare_orchard_bundle_v1_with_rng(
        orchard_empty_root_v1(),
        Vec::new(),
        vec![funding_output],
        1,
        &mut funding_rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let (funding_transaction, funding_statement, funding_bundle) =
        finalize_orchard_network_transaction_v1(
            &funding_context,
            pool_id,
            asset_definition_id.clone(),
            1,
            expiry_height,
            funding_prepared,
            funding_private_key,
        )?;
    if funding_bundle.public.anchor != orchard_empty_root_v1()
        || funding_bundle.public.value_balance
            != -i64::try_from(FUNDED_VALUE).expect("fixed Orchard value fits i64")
        || funding_bundle.public.actions.len() != 1
        || funding_statement.value_balance.direction != PrivacyValueBalanceDirectionV1::IntoPool
        || funding_statement.value_balance.amount != u128::from(FUNDED_VALUE)
        || funding_statement.anchor_epoch != 1
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }

    let funded_commitment = funding_bundle.public.actions[0].note_commitment;
    let funded_frontier =
        append_orchard_commitments_v1(0, None, &[], orchard_empty_root_v1(), &[funded_commitment])
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let (funded_anchor, funded_authentication_path) =
        orchard_singleton_output_witness_v1(funded_commitment)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    if funded_frontier.root != funded_anchor || funded_frontier.tree_size != 1 {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let funded_spend = recover_orchard_spend_prover_input_v1(
        &funding_bundle,
        &limits,
        orchard_spending_key_v1(wallet_seed),
        0,
        0,
        funded_authentication_path,
        funded_anchor,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let spent_note_nullifier = funded_spend.nullifier_v1();
    let change = OrchardChangeProverInputV1::new(
        orchard_spending_key_v1(wallet_seed),
        Scope::Internal,
        u32::from(wallet_seed) + 1,
        CHANGE_VALUE,
        [wallet_seed.wrapping_add(1); 512],
    );
    let mut spend_rng = EvidenceRng09::new(network_seed_v1(
        fixture_seed,
        b"orchard-semantic-spend-prepare",
        0,
    ));
    let spend_prepared = prepare_orchard_bundle_v1_with_rng(
        funded_anchor,
        vec![funded_spend],
        vec![change],
        2,
        &mut spend_rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let (spend_transaction, spend_statement, spend_bundle) =
        finalize_orchard_network_transaction_v1(
            &spend_context,
            pool_id,
            asset_definition_id.clone(),
            2,
            expiry_height,
            spend_prepared,
            spend_private_key,
        )?;
    let spent_nullifier_count = spend_bundle
        .public
        .actions
        .iter()
        .filter(|action| action.nullifier == spent_note_nullifier)
        .count();
    if spend_bundle.public.anchor != funded_anchor
        || spend_bundle.public.value_balance
            != i64::try_from(RECEIVER_VALUE).expect("fixed Orchard value fits i64")
        || spend_bundle.public.actions.len() != 2
        || spent_nullifier_count != 1
        || spend_statement.value_balance.direction != PrivacyValueBalanceDirectionV1::OutOfPool
        || spend_statement.value_balance.amount != u128::from(RECEIVER_VALUE)
        || spend_statement.anchor_epoch != 2
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let spend_commitments = spend_bundle
        .public
        .actions
        .iter()
        .map(|action| action.note_commitment)
        .collect::<Vec<_>>();
    let final_frontier = append_orchard_commitments_v1(
        funded_frontier.tree_size,
        funded_frontier.leaf,
        &funded_frontier.ommers,
        funded_frontier.root,
        &spend_commitments,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    if final_frontier.tree_size != 3 || final_frontier.root == funded_frontier.root {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }

    let bootstrap = PrivacyOrchardPoolBootstrapV1::new(
        pool_id,
        asset_definition_id,
        iroha_data_model::asset::AssetBalanceScope::Global,
        reserve_account,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    Ok(PrivacyReleaseOrchardNetworkActionsV1 {
        funding_transaction,
        spend_transaction,
        bootstrap,
        funding_statement,
        spend_statement,
        spent_note_nullifier,
        receiver_account: spend_context.authority,
        funded_value: FUNDED_VALUE,
        receiver_value: RECEIVER_VALUE,
        change_value: CHANGE_VALUE,
        expected_final_root: final_frontier.root,
        state_query: FindPrivacyOrchardPoolStateV1::new(pool_id),
        nullifier_query: FindPrivacyOrchardNullifierV1::new(pool_id, spent_note_nullifier),
    })
}
fn build_pq_masp_transaction_v1(
    transaction_context: &PrivacyReleaseTransactionContextV1,
    mut statement: PqMaspStarkStatementV1,
    witness: &crate::privacy_engines::pq_masp::PqMaspWitnessV1,
    authorization_secret_key: &[u8],
    proof_seed: [u8; 32],
    private_key: &PrivateKey,
) -> Result<(SignedTransaction, PqMaspStarkStatementV1), PrivacyReleaseEvidenceErrorClassV1> {
    let profile =
        compiled_privacy_profile_v1(iroha_data_model::privacy::PrivacyProtocolIdV1::PqMaspStarkV0)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    statement.context = statement_context_v1(transaction_context, profile);
    let draft_envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest: PrivacyStatementDigestV1::new([0; 32]),
        statement: PrivacyStatementV1::PqMaspStarkV0(statement.clone()),
        proof: PrivacyProofV1::PqMaspStarkV0(PrivacyProofBytesV1::new(Vec::new())),
    };
    let intent = transaction_payload_v1(transaction_context, draft_envelope)?
        .privacy_transaction_intent_digest_v1()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    if intent.is_zero() {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    statement.context.transaction_intent_digest = intent;
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let consensus_binding = PrivacyNativeConsensusBindingV1::new(
        &statement.context,
        transaction_context.genesis_hash,
        &limits,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let mut proof_rng = EvidenceRng09::new(proof_seed);
    let proof = prove_pq_masp_v1_with_rng(
        &statement,
        &consensus_binding,
        &limits,
        witness,
        authorization_secret_key,
        &mut proof_rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let typed_statement = PrivacyStatementV1::PqMaspStarkV0(statement.clone());
    let envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest: typed_statement
            .digest()
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
        statement: typed_statement,
        proof: PrivacyProofV1::PqMaspStarkV0(PrivacyProofBytesV1::new(proof)),
    };
    let payload = transaction_payload_v1(transaction_context, envelope)?;
    if payload
        .validate_privacy_transaction_intent_binding_v1()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
        != intent
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok((signed_payload_v1(payload, intent, private_key)?, statement))
}
/// Build four network-bound PQ-MASP actions with distinct intents and proofs
/// but the same stable nullifier.
///
/// This helper exists only in the non-default release-evidence feature. The
/// complex witness and ML-DSA secret never cross its API boundary.
pub fn build_privacy_release_pq_masp_network_actions_v1(
    preactivation_context: PrivacyReleaseTransactionContextV1,
    canonical_context: PrivacyReleaseTransactionContextV1,
    replay_context: PrivacyReleaseTransactionContextV1,
    post_restart_replay_context: PrivacyReleaseTransactionContextV1,
    fixture_seed: [u8; 32],
    private_key: &PrivateKey,
) -> Result<PrivacyReleasePqMaspNetworkActionsV1, PrivacyReleaseEvidenceErrorClassV1> {
    if preactivation_context.network_id != canonical_context.network_id
        || preactivation_context.authority != canonical_context.authority
        || preactivation_context.genesis_hash != canonical_context.genesis_hash
        || canonical_context.network_id != replay_context.network_id
        || canonical_context.authority != replay_context.authority
        || canonical_context.genesis_hash != replay_context.genesis_hash
        || canonical_context.network_id != post_restart_replay_context.network_id
        || canonical_context.authority != post_restart_replay_context.authority
        || canonical_context.genesis_hash != post_restart_replay_context.genesis_hash
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed);
    }
    let keygen_seed = network_seed_v1(fixture_seed, b"pq-masp-keygen", 0);
    let mut fixture_rng =
        EvidenceRng09::new(network_seed_v1(fixture_seed, b"pq-masp-encryption", 0));
    let fixture = pq_masp_release_fixture_v1(false, keygen_seed, &mut fixture_rng)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let (successor_replay_statement, successor_replay_witness) =
        pq_masp_release_successor_replay_fixture_v1(&fixture)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let initial_note_commitments = fixture
        .witness
        .inputs()
        .iter()
        .map(|input| input.commitment_v1(&fixture.statement))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let bootstrap =
        PrivacyProofManagedPoolBootstrapV1::PqMaspStarkV0(PrivacyPqMaspPoolBootstrapV1 {
            pool_id: fixture.statement.pool_id,
            asset_definition_id: fixture.statement.asset_definition_id.clone(),
            initial_note_commitments,
        });
    bootstrap
        .validate()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let (preactivation_transaction, preactivation_statement) = build_pq_masp_transaction_v1(
        &preactivation_context,
        fixture.statement.clone(),
        &fixture.witness,
        fixture.authorization_secret_key.as_slice(),
        network_seed_v1(fixture_seed, b"pq-masp-proof", 0),
        private_key,
    )?;
    let (canonical_transaction, canonical_statement) = build_pq_masp_transaction_v1(
        &canonical_context,
        fixture.statement.clone(),
        &fixture.witness,
        fixture.authorization_secret_key.as_slice(),
        network_seed_v1(fixture_seed, b"pq-masp-proof", 1),
        private_key,
    )?;
    let (replay_transaction, replay_statement) = build_pq_masp_transaction_v1(
        &replay_context,
        successor_replay_statement.clone(),
        &successor_replay_witness,
        fixture.authorization_secret_key.as_slice(),
        network_seed_v1(fixture_seed, b"pq-masp-proof", 2),
        private_key,
    )?;
    let (post_restart_replay_transaction, post_restart_replay_statement) =
        build_pq_masp_transaction_v1(
            &post_restart_replay_context,
            successor_replay_statement,
            &successor_replay_witness,
            fixture.authorization_secret_key.as_slice(),
            network_seed_v1(fixture_seed, b"pq-masp-proof", 3),
            private_key,
        )?;
    let expected_replay_epoch = canonical_statement
        .anchor_epoch
        .checked_add(1)
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let transaction_hashes = [
        preactivation_transaction.hash(),
        canonical_transaction.hash(),
        replay_transaction.hash(),
        post_restart_replay_transaction.hash(),
    ];
    let transaction_intents = [
        preactivation_statement.context.transaction_intent_digest,
        canonical_statement.context.transaction_intent_digest,
        replay_statement.context.transaction_intent_digest,
        post_restart_replay_statement
            .context
            .transaction_intent_digest,
    ];
    if transaction_hashes
        .iter()
        .enumerate()
        .any(|(index, hash)| transaction_hashes[..index].contains(hash))
        || transaction_intents
            .iter()
            .enumerate()
            .any(|(index, intent)| transaction_intents[..index].contains(intent))
        || preactivation_statement.nullifiers != canonical_statement.nullifiers
        || canonical_statement.nullifiers != replay_statement.nullifiers
        || replay_statement.nullifiers != post_restart_replay_statement.nullifiers
        || replay_statement.nullifiers.is_empty()
        || replay_statement.output_commitments != post_restart_replay_statement.output_commitments
        || canonical_statement.anchor == replay_statement.anchor
        || replay_statement.anchor != post_restart_replay_statement.anchor
        || replay_statement.anchor_epoch != expected_replay_epoch
        || post_restart_replay_statement.anchor_epoch != expected_replay_epoch
        || replay_statement.authorization_epoch != expected_replay_epoch
        || post_restart_replay_statement.authorization_epoch != expected_replay_epoch
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok(PrivacyReleasePqMaspNetworkActionsV1 {
        preactivation_transaction,
        canonical_transaction,
        replay_transaction,
        post_restart_replay_transaction,
        bootstrap,
        preactivation_statement,
        canonical_statement,
        replay_statement,
        post_restart_replay_statement,
        state_query: FindPrivacyProofManagedPoolStateV1::new(
            iroha_data_model::privacy::PrivacyProtocolIdV1::PqMaspStarkV0,
            fixture.statement.pool_id,
        ),
    })
}
