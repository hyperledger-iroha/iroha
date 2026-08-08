//! Non-shipping canonical action builders for four-peer release gates.
//!
//! The parent module is compiled only with `privacy-release-evidence`. These
//! helpers retain PQ witness material inside `iroha_core` and return ordinary
//! production `SignedTransaction` and bootstrap data-model values. Network
//! execution therefore traverses the same Torii, DA/RBC, verifier, and ledger
//! paths as any externally constructed action.

use std::{num::NonZeroU32, time::Duration};

use iroha_crypto::PrivateKey;
use iroha_data_model::{
    metadata::Metadata,
    prelude::{AccountId, AssetDefinitionId, ChainId},
    privacy::{
        OrchardHalo2ActionsStatementV1, PqMaspStarkStatementV1, PrivacyConsensusLimitsV1,
        PrivacyNativeConsensusBindingV1, PrivacyOrchardActionV1, PrivacyOrchardPoolBootstrapV1,
        PrivacyPoolIdV1, PrivacyPqMaspPoolBootstrapV1, PrivacyProofBytesV1, PrivacyProofEnvelopeV1,
        PrivacyProofManagedPoolBootstrapV1, PrivacyProofV1, PrivacyStatementContextV1,
        PrivacyStatementDigestV1, PrivacyStatementV1, PrivacyTransactionIntentDigestV1,
        PrivacyValueBalanceDirectionV1, PrivacyValueBalanceV1,
    },
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionPayload},
};
use sha2::{Digest as _, Sha256};

mod retained;

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

use super::{
    EvidenceRng09, PrivacyReleaseEvidenceErrorClassV1, authorize_orchard_bundle_v1,
    compiled_privacy_profile_v1, orchard_empty_root_v1, orchard_spending_key_v1,
    pq_masp_release_fixture_v1, pq_masp_release_successor_replay_fixture_v1,
    prepare_orchard_bundle_v1_with_rng, prove_pq_masp_v1_with_rng,
};
use crate::privacy_engines::orchard::{OrchardBundleDraftV1, OrchardChangeProverInputV1, Scope};

/// Exact transaction and consensus context used by one non-shipping network
/// action builder.
#[derive(Clone, Debug)]
pub struct PrivacyReleaseTransactionContextV1 {
    /// Actual network chain identifier.
    pub chain_id: ChainId,
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

/// Four independently proved PQ-MASP actions consuming the same genesis note.
///
/// Each transaction has a distinct canonical intent and proof. The first is a
/// valid pre-activation probe; the second is expected to apply after
/// activation; the remaining two carry the same stable nullifier as protocol
/// replay probes rather than duplicate transaction-hash probes. Keeping the
/// fourth transaction fresh lets a restart gate prove that both the successor
/// frontier and consumed-nullifier set were recovered by the restarted peer.
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
        chain_id: transaction.chain_id.clone(),
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
    let mut builder = TransactionBuilder::new(
        context.chain_id.clone(),
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
    expiry_height: u64,
) -> Result<OrchardHalo2ActionsStatementV1, PrivacyReleaseEvidenceErrorClassV1> {
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
        anchor_epoch: 1,
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

/// Build one canonical network-bound Orchard action through the production
/// two-phase prover and consuming authorization API.
///
/// This helper exists only in the non-default release-evidence feature.
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
    if preactivation_context.chain_id != canonical_context.chain_id
        || preactivation_context.authority != canonical_context.authority
        || preactivation_context.genesis_hash != canonical_context.genesis_hash
        || canonical_context.chain_id != replay_context.chain_id
        || canonical_context.authority != replay_context.authority
        || canonical_context.genesis_hash != replay_context.genesis_hash
        || canonical_context.chain_id != post_restart_replay_context.chain_id
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
    })
}
