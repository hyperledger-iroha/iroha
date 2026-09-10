//! Canonical commitments and guard contexts for KAGEMUSHA state transitions.
//!
//! Each preimage owns its frame identity. Hashes bind the complete canonical frame, including
//! its header, and retain the protocol identity independently of this module location.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::MintFoldEffectV1")]
pub(super) struct MintFoldEffectV1 {
    pub(super) credit_id: CreditIdV1,
    pub(super) envelope_digest: DigestV1,
    pub(super) amount: u128,
    pub(super) issuance_digest: DigestV1,
    pub(super) mint_finality_semantic_digest: DigestV1,
    pub(super) mint_finality_proof_binding_digest: DigestV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::RotateEffectV1")]
pub(super) struct RotateEffectV1 {
    pub(super) predecessor_epoch: HardwareEpochV1,
    pub(super) successor_epoch: HardwareEpochV1,
    pub(super) predecessor_device_policy_binding: DevicePolicyBindingV1,
    pub(super) successor_device_policy_binding: DevicePolicyBindingV1,
    pub(super) predecessor_state_nonce_commitment: DigestV1,
    pub(super) successor_state_nonce_commitment: DigestV1,
    pub(super) carried_balance: u128,
    pub(super) carried_consumed_credit_root: KagemushaPastaStateCommitmentV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::TransitionIntentPreimageV1")]
struct TransitionIntentPreimageV1 {
    release_id: DigestV1,
    liability_pool_id: DigestV1,
    trusted_commit_time_ms: u64,
    statement: TransitionProofStatementV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::RecoveryRecordPreimageV1")]
struct RecoveryRecordPreimageV1 {
    transition_intent_digest: DigestV1,
    state_transition_digest: DigestV1,
    successor_state_commitment: DigestV1,
    journal_revision_after: u128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::DurableEffectPreimageV1")]
struct DurableEffectPreimageV1 {
    kind: KagemushaTransitionKindV1,
    transition_effect_digest: DigestV1,
    predecessor_state_commitment: DigestV1,
    successor_state_commitment: DigestV1,
    journal_revision_after: u128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::LocalTransitionTransportStatementV1")]
struct LocalTransitionTransportStatementV1 {
    version: u16,
    kind: KagemushaTransitionKindV1,
    release_id: DigestV1,
    liability_pool_id: DigestV1,
    transition_effect_digest: DigestV1,
    predecessor_state_commitment: DigestV1,
    successor_state_commitment: DigestV1,
    normalized_guard_statement_digest: DigestV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::BootstrapIntentPreimageV1")]
struct BootstrapIntentPreimageV1 {
    trusted_commit_time_ms: u64,
    statement: BootstrapStatementV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::BootstrapRecoveryPreimageV1")]
struct BootstrapRecoveryPreimageV1 {
    transition_intent_digest: DigestV1,
    bootstrap_statement_digest: DigestV1,
    successor_state_commitment: DigestV1,
}

pub(super) fn derive_liability_pool_id(
    lane: &KagemushaLaneIdV1,
    asset_incarnation: AxtAssetIncarnationV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    kagemusha_liability_pool_id_v1(&lane.network_id, &lane.asset, asset_incarnation)
        .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)
}

pub(super) fn local_transition_transport_digest(
    kind: KagemushaTransitionKindV1,
    release_id: DigestV1,
    liability_pool_id: DigestV1,
    transition_effect_digest: DigestV1,
    predecessor_state_commitment: DigestV1,
    successor_state_commitment: DigestV1,
    normalized_guard_statement_digest: DigestV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    canonical_sha256_digest(
        TRANSPORT_STATEMENT_DOMAIN,
        &LocalTransitionTransportStatementV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            kind,
            release_id,
            liability_pool_id,
            transition_effect_digest,
            predecessor_state_commitment,
            successor_state_commitment,
            normalized_guard_statement_digest,
        },
    )
}

pub(super) fn bootstrap_guard_context(
    artifacts: KagemushaRecursionArtifactsV1,
    statement: &BootstrapStatementV1,
    trusted_commit_time_ms: u64,
) -> Result<KagemushaGuardContextV1, KagemushaStateErrorV1> {
    let transition_effect_digest = canonical_sha256_digest(TRANSITION_EFFECT_DOMAIN, statement)?;
    let transition_intent_digest = canonical_sha256_digest(
        TRANSITION_INTENT_DOMAIN,
        &BootstrapIntentPreimageV1 {
            trusted_commit_time_ms,
            statement: statement.clone(),
        },
    )?;
    let recovery_record_digest = canonical_sha256_digest(
        RECOVERY_RECORD_DOMAIN,
        &BootstrapRecoveryPreimageV1 {
            transition_intent_digest,
            bootstrap_statement_digest: statement.proof_statement_digest()?,
            successor_state_commitment: statement.state_commitment,
        },
    )?;
    Ok(KagemushaGuardContextV1 {
        release_id: artifacts.release_id,
        liability_pool_id: statement.liability_pool_id,
        lifecycle_binding_digest: canonical_sha256_digest(TRANSITION_LIFECYCLE_DOMAIN, statement)?,
        prepared_transition_binding_digest: [0; 32],
        receive_credit_binding_digest: [0; 32],
        terminal_commit_binding_digest: [0; 32],
        sender_one_time_authorization_digest: [0; 32],
        transition_intent_digest,
        transition_effect_digest,
        recovery_record_digest,
        durable_inbox_effect_digest: artifacts.canonical_empty_effect_digest,
        durable_outbox_effect_digest: artifacts.canonical_empty_effect_digest,
        canonical_empty_effect_digest: artifacts.canonical_empty_effect_digest,
    })
}

pub(super) fn transition_guard_context(
    artifacts: KagemushaRecursionArtifactsV1,
    statement: &TransitionProofStatementV1,
    trusted_commit_time_ms: u64,
) -> Result<KagemushaGuardContextV1, KagemushaStateErrorV1> {
    let transition_intent_digest = canonical_sha256_digest(
        TRANSITION_INTENT_DOMAIN,
        &TransitionIntentPreimageV1 {
            release_id: artifacts.release_id,
            liability_pool_id: derive_liability_pool_id(
                &statement.lane,
                statement.asset_incarnation,
            )?,
            trusted_commit_time_ms,
            statement: statement.clone(),
        },
    )?;
    let state_transition_digest = transition_statement_digest(statement)?;
    let recovery_record_digest = canonical_sha256_digest(
        RECOVERY_RECORD_DOMAIN,
        &RecoveryRecordPreimageV1 {
            transition_intent_digest,
            state_transition_digest,
            successor_state_commitment: statement.successor_commitment,
            journal_revision_after: statement.journal_revision_after,
        },
    )?;
    let durable_effect = DurableEffectPreimageV1 {
        kind: statement.kind,
        transition_effect_digest: statement.effect_digest,
        predecessor_state_commitment: statement.predecessor_commitment,
        successor_state_commitment: statement.successor_commitment,
        journal_revision_after: statement.journal_revision_after,
    };
    let empty = artifacts.canonical_empty_effect_digest;
    let (durable_inbox_effect_digest, durable_outbox_effect_digest) = match statement.kind {
        KagemushaTransitionKindV1::MintFold | KagemushaTransitionKindV1::ReceiveFold => (
            canonical_sha256_digest(DURABLE_INBOX_EFFECT_DOMAIN, &durable_effect)?,
            empty,
        ),
        KagemushaTransitionKindV1::SendSplit | KagemushaTransitionKindV1::RedeemSplit => (
            empty,
            canonical_sha256_digest(DURABLE_OUTBOX_EFFECT_DOMAIN, &durable_effect)?,
        ),
        KagemushaTransitionKindV1::Rotate => (empty, empty),
    };
    Ok(KagemushaGuardContextV1 {
        release_id: artifacts.release_id,
        liability_pool_id: derive_liability_pool_id(&statement.lane, statement.asset_incarnation)?,
        lifecycle_binding_digest: statement.lifecycle_binding_digest,
        prepared_transition_binding_digest: statement.prepared_transition_binding_digest,
        receive_credit_binding_digest: statement.receive_credit_binding_digest,
        terminal_commit_binding_digest: [0; 32],
        sender_one_time_authorization_digest: [0; 32],
        transition_intent_digest,
        transition_effect_digest: statement.effect_digest,
        recovery_record_digest,
        durable_inbox_effect_digest,
        durable_outbox_effect_digest,
        canonical_empty_effect_digest: empty,
    })
}

pub(super) fn transition_statement_digest(
    statement: &TransitionProofStatementV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    canonical_sha256_digest(TRANSITION_STATEMENT_DOMAIN, statement)
}

pub(super) fn transport_semantic_digest(
    normalized_guard_statement_digest: DigestV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    canonical_sha256_digest(
        TRANSPORT_STATEMENT_DOMAIN,
        &normalized_guard_statement_digest,
    )
}

pub(super) fn canonical_sha256_digest<T: norito::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    let encoded =
        norito::encode_canonical(value).map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
    let mut hasher = Sha256::new();
    hasher.update(
        u64::try_from(domain.len())
            .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?
            .to_be_bytes(),
    );
    hasher.update(domain);
    hasher.update(
        u64::try_from(encoded.len())
            .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?
            .to_be_bytes(),
    );
    hasher.update(encoded);
    Ok(hasher.finalize().into())
}

pub(super) fn canonical_poseidon_digest<T: norito::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    let encoded =
        norito::encode_canonical(value).map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
    let mut framed = Vec::with_capacity(
        domain
            .len()
            .saturating_add(encoded.len())
            .saturating_add(16),
    );
    framed.extend_from_slice(
        &u64::try_from(domain.len())
            .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?
            .to_be_bytes(),
    );
    framed.extend_from_slice(domain);
    framed.extend_from_slice(
        &u64::try_from(encoded.len())
            .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?
            .to_be_bytes(),
    );
    framed.extend_from_slice(&encoded);
    Ok(poseidon::hash_bytes(&framed))
}

#[cfg(test)]
#[test]
fn captured_state_frame_owners() {
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<MintFoldEffectV1>(
        "iroha_core::zk::kagemusha_v1_state::MintFoldEffectV1",
    );
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<RotateEffectV1>(
        "iroha_core::zk::kagemusha_v1_state::RotateEffectV1",
    );
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<TransitionIntentPreimageV1>(
        "iroha_core::zk::kagemusha_v1_state::TransitionIntentPreimageV1",
    );
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<RecoveryRecordPreimageV1>(
        "iroha_core::zk::kagemusha_v1_state::RecoveryRecordPreimageV1",
    );
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<DurableEffectPreimageV1>(
        "iroha_core::zk::kagemusha_v1_state::DurableEffectPreimageV1",
    );
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<
        LocalTransitionTransportStatementV1,
    >("iroha_core::zk::kagemusha_v1_state::LocalTransitionTransportStatementV1");
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<BootstrapIntentPreimageV1>(
        "iroha_core::zk::kagemusha_v1_state::BootstrapIntentPreimageV1",
    );
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<
        BootstrapRecoveryPreimageV1,
    >("iroha_core::zk::kagemusha_v1_state::BootstrapRecoveryPreimageV1");
}
