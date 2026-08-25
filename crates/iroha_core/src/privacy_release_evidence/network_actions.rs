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
        IrohaZkAmsStatementV1, OrchardHalo2ActionsStatementV1, PqMaspStarkStatementV1,
        PrivacyConsensusLimitsV1, PrivacyIssuerIdV1, PrivacyJindoFieldElementV1,
        PrivacyNativeConsensusBindingV1, PrivacyOrchardActionV1, PrivacyOrchardPoolBootstrapV1,
        PrivacyP256PointV1, PrivacyPolicyDigestV1, PrivacyPolicyIdV1, PrivacyPoolIdV1,
        PrivacyPqMaspPoolBootstrapV1, PrivacyProofBytesV1, PrivacyProofEnvelopeV1,
        PrivacyProofManagedPoolBootstrapV1, PrivacyProofV1, PrivacyRootV1,
        PrivacyStatementContextV1, PrivacyStatementDigestV1, PrivacyStatementV1,
        PrivacyTransactionIntentDigestV1, PrivacyValueBalanceDirectionV1, PrivacyValueBalanceV1,
        PrivacyVegaIssuerRecordV1, PrivacyVegaMdlDateV1, PrivacyZkAmsActionV1,
        PrivacyZkAmsAdmissionAnchorV1, PrivacyZkAmsBatchAdmissionV1, PrivacyZkAmsCredentialNonceV1,
        PrivacyZkAmsKeyImageV1, PrivacyZkAmsPersonhoodCredentialV1, PrivacyZkAmsProvisionAccountV1,
        PrivacyZkAmsRegistryBootstrapV1, PrivacyZkAmsRegistryRecordDigestV1,
        PrivacyZkAmsSeedPublicKeyV1, PrivacyZkAmsSubjectCommitmentV1, ZK_AMS_PHC_VERSION_V1,
        ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1, zk_ams_registry_record_digest_v1,
    },
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionPayload},
};
use p256::ecdsa::{
    Signature as P256Signature, SigningKey as P256SigningKey, signature::hazmat::PrehashSigner as _,
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
    orchard::{OrchardBundleDraftV1, OrchardChangeProverInputV1, Scope},
    vega::{
        VegaPrivacyActionTransactionContextV1, VegaPrivacyActionWitnessMaterialV1,
        build_signed_vega_privacy_action_with_rng_v1,
    },
    zk_ams::{
        SignedZkAmsPrivacyActionV1, ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1, ZK_AMS_MIN_RING_SIZE_V1,
        ZkAmsBatchCredentialWitnessV1, ZkAmsMaskedProverConfigV1, ZkAmsPrivacyActionEffectV1,
        ZkAmsPrivacyActionGovernanceV1, ZkAmsPrivacyActionTransactionContextV1, ZkAmsSeedSecretV1,
        build_signed_zk_ams_batch_admission_privacy_action_with_rng_v1,
        build_signed_zk_ams_provision_privacy_action_with_rng_v1, zk_ams_key_image_v1,
        zk_ams_registry_transition_root_v1, zk_ams_seed_public_key_v1,
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
}
/// Exact two-batch admission and successor-state provisioning sequence for ZK-AMS.
///
/// Credential, issuer-signing, seed-secret, proof-randomness, and transaction-signing material
/// remains internal to the builder. The returned bootstrap and statements contain only the typed
/// public state that a sealed controller must register, submit, and verify in order.
#[derive(Clone, Debug)]
pub struct PrivacyReleaseZkAmsNetworkActionsV1 {
    /// First eight-anchor admission transaction, from bootstrap epoch one to epoch two.
    pub first_admission_transaction: SignedTransaction,
    /// Second eight-anchor admission transaction, from epoch two to epoch three.
    pub second_admission_transaction: SignedTransaction,
    /// Minimum-ring account-provisioning transaction against the epoch-three state.
    pub provision_transaction: SignedTransaction,
    /// Immutable governed registry bootstrap required before either admission.
    pub bootstrap: PrivacyZkAmsRegistryBootstrapV1,
    /// Exact public statement carried by `first_admission_transaction`.
    pub first_admission_statement: IrohaZkAmsStatementV1,
    /// Exact public statement carried by `second_admission_transaction`.
    pub second_admission_statement: IrohaZkAmsStatementV1,
    /// Exact public statement carried by `provision_transaction`.
    pub provision_statement: IrohaZkAmsStatementV1,
}

struct ZkAmsReleaseCredentialFixtureV1 {
    credential: PrivacyZkAmsPersonhoodCredentialV1,
    issuer_signature: [u8; 64],
    seed_secret: ZkAmsSeedSecretV1,
}

struct ZkAmsReleasePublicLineageV1 {
    bootstrap: PrivacyZkAmsRegistryBootstrapV1,
    first_admission: PrivacyZkAmsBatchAdmissionV1,
    second_admission: PrivacyZkAmsBatchAdmissionV1,
    provision: PrivacyZkAmsProvisionAccountV1,
    first_registry_record_digest: PrivacyZkAmsRegistryRecordDigestV1,
    second_registry_record_digest: PrivacyZkAmsRegistryRecordDigestV1,
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

fn zk_ams_issuer_public_key_v1() -> Result<PrivacyP256PointV1, PrivacyReleaseEvidenceErrorClassV1> {
    let signing_key = P256SigningKey::from_bytes((&[7_u8; 32]).into())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let encoded = signing_key.verifying_key().to_encoded_point(true);
    let bytes: [u8; 33] = encoded
        .as_bytes()
        .try_into()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    Ok(PrivacyP256PointV1::new(bytes))
}

fn zk_ams_release_fixture_v1(
    fixture_seed: [u8; 32],
) -> Result<
    (
        PrivacyZkAmsRegistryBootstrapV1,
        Vec<ZkAmsReleaseCredentialFixtureV1>,
    ),
    PrivacyReleaseEvidenceErrorClassV1,
> {
    if fixture_seed == [0; 32]
        || ZK_AMS_MIN_RING_SIZE_V1
            != ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1
                .checked_mul(2)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed);
    }
    let issuer_signing_key = P256SigningKey::from_bytes((&[7_u8; 32]).into())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let issuer_id = PrivacyIssuerIdV1::new([0x31; 32]);
    let registry_id = iroha_data_model::privacy::PrivacyZkAmsRegistryIdV1::new([0x33; 32]);
    let policy_id = PrivacyPolicyIdV1::new([0x35; 32]);
    let bootstrap = PrivacyZkAmsRegistryBootstrapV1 {
        issuer_id,
        registry_id,
        policy_id,
        issuer_public_key: zk_ams_issuer_public_key_v1()?,
        policy_digest: PrivacyPolicyDigestV1::new([0x36; 32]),
        initial_registry_root: PrivacyRootV1::new([0x37; 32]),
        initial_registry_epoch: ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1,
    };
    bootstrap
        .validate()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let mut seed_material = (0..ZK_AMS_MIN_RING_SIZE_V1)
        .map(|index| {
            let index = u8::try_from(index)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            let mut rng =
                EvidenceRng06::new(network_seed_v1(fixture_seed, b"zk-ams-seed-secret", index));
            let seed_secret = ZkAmsSeedSecretV1::generate(&mut rng)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            Ok((zk_ams_seed_public_key_v1(&seed_secret), seed_secret))
        })
        .collect::<Result<Vec<_>, PrivacyReleaseEvidenceErrorClassV1>>()?;
    seed_material.sort_by_key(|(public_key, _)| *public_key);
    if !seed_material.windows(2).all(|pair| pair[0].0 < pair[1].0) {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let credentials = seed_material
        .into_iter()
        .enumerate()
        .map(|(index, (seed_public_key, seed_secret))| {
            let index = u8::try_from(index)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            let subject_byte = 0x41_u8
                .checked_add(index)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            let nonce_byte = 0x51_u8
                .checked_add(index)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            let credential = PrivacyZkAmsPersonhoodCredentialV1 {
                version: ZK_AMS_PHC_VERSION_V1,
                issuer_id,
                policy_id,
                subject_commitment: PrivacyZkAmsSubjectCommitmentV1::new([subject_byte; 32]),
                seed_public_key: PrivacyZkAmsSeedPublicKeyV1::new(seed_public_key),
                credential_nonce: PrivacyZkAmsCredentialNonceV1::new([nonce_byte; 32]),
            };
            let signature: P256Signature = issuer_signing_key
                .sign_prehash(credential.digest().as_bytes())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            let signature = signature.normalize_s().unwrap_or(signature);
            Ok(ZkAmsReleaseCredentialFixtureV1 {
                credential,
                issuer_signature: signature.to_bytes().into(),
                seed_secret,
            })
        })
        .collect::<Result<Vec<_>, PrivacyReleaseEvidenceErrorClassV1>>()?;
    if credentials.len() != ZK_AMS_MIN_RING_SIZE_V1 {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok((bootstrap, credentials))
}

fn zk_ams_next_root_v1(
    registry_id: iroha_data_model::privacy::PrivacyZkAmsRegistryIdV1,
    current_root: PrivacyRootV1,
    current_epoch: u64,
    next_epoch: u64,
    anchors: &[PrivacyZkAmsAdmissionAnchorV1],
) -> Result<PrivacyRootV1, PrivacyReleaseEvidenceErrorClassV1> {
    let batch_size = u32::try_from(anchors.len())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    Ok(anchors
        .iter()
        .copied()
        .enumerate()
        .fold(current_root, |prior_root, (index, anchor)| {
            zk_ams_registry_transition_root_v1(
                registry_id,
                prior_root,
                current_epoch,
                next_epoch,
                batch_size,
                u32::try_from(index).expect("eight ZK-AMS admission anchors fit u32"),
                anchor,
            )
        }))
}

fn zk_ams_release_public_lineage_v1(
    bootstrap: PrivacyZkAmsRegistryBootstrapV1,
    credentials: &[ZkAmsReleaseCredentialFixtureV1],
    provision_account_id: AccountId,
) -> Result<ZkAmsReleasePublicLineageV1, PrivacyReleaseEvidenceErrorClassV1> {
    if credentials.len() != ZK_AMS_MIN_RING_SIZE_V1 {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let all_anchors = credentials
        .iter()
        .map(|fixture| PrivacyZkAmsAdmissionAnchorV1 {
            phc_hash: fixture.credential.digest(),
            seed_public_key: fixture.credential.seed_public_key,
        })
        .collect::<Vec<_>>();
    let (first_anchors, second_anchors) = all_anchors.split_at(ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1);
    if first_anchors.len() != ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1
        || second_anchors.len() != ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let first_epoch = bootstrap
        .initial_registry_epoch
        .checked_add(1)
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let first_root = zk_ams_next_root_v1(
        bootstrap.registry_id,
        bootstrap.initial_registry_root,
        bootstrap.initial_registry_epoch,
        first_epoch,
        first_anchors,
    )?;
    let first_admission = PrivacyZkAmsBatchAdmissionV1 {
        account_registry_root: bootstrap.initial_registry_root,
        account_registry_root_epoch: bootstrap.initial_registry_epoch,
        next_account_registry_root: first_root,
        next_account_registry_root_epoch: first_epoch,
        anchors: first_anchors.to_vec(),
    };
    let first_registry_record_digest = zk_ams_registry_record_digest_v1(
        bootstrap.issuer_id,
        bootstrap.registry_id,
        bootstrap.policy_id,
        bootstrap.issuer_policy_record_digest(),
        bootstrap.policy_digest,
        first_root,
        first_epoch,
    );
    let second_epoch = first_epoch
        .checked_add(1)
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let second_root = zk_ams_next_root_v1(
        bootstrap.registry_id,
        first_root,
        first_epoch,
        second_epoch,
        second_anchors,
    )?;
    let second_admission = PrivacyZkAmsBatchAdmissionV1 {
        account_registry_root: first_root,
        account_registry_root_epoch: first_epoch,
        next_account_registry_root: second_root,
        next_account_registry_root_epoch: second_epoch,
        anchors: second_anchors.to_vec(),
    };
    let second_registry_record_digest = zk_ams_registry_record_digest_v1(
        bootstrap.issuer_id,
        bootstrap.registry_id,
        bootstrap.policy_id,
        bootstrap.issuer_policy_record_digest(),
        bootstrap.policy_digest,
        second_root,
        second_epoch,
    );
    let signer_secret = credentials
        .get(5)
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let provision = PrivacyZkAmsProvisionAccountV1 {
        account_registry_root: second_root,
        account_registry_root_epoch: second_epoch,
        admitted_seed_key_ring: all_anchors
            .iter()
            .map(|anchor| anchor.seed_public_key)
            .collect(),
        account_id: provision_account_id,
        key_image: PrivacyZkAmsKeyImageV1::new(
            zk_ams_key_image_v1(&signer_secret.seed_secret)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?,
        ),
    };
    Ok(ZkAmsReleasePublicLineageV1 {
        bootstrap,
        first_admission,
        second_admission,
        provision,
        first_registry_record_digest,
        second_registry_record_digest,
    })
}

fn zk_ams_action_context_v1(
    context: &PrivacyReleaseTransactionContextV1,
) -> ZkAmsPrivacyActionTransactionContextV1 {
    ZkAmsPrivacyActionTransactionContextV1 {
        network_id: context.network_id,
        authority: context.authority.clone(),
        creation_time: context.creation_time,
        time_to_live: context.time_to_live,
        nonce: context.nonce,
        fee_payment: context.fee_payment.clone(),
        metadata: context.metadata.clone(),
    }
}

fn zk_ams_governance_v1(
    bootstrap: PrivacyZkAmsRegistryBootstrapV1,
    registry_record_digest: PrivacyZkAmsRegistryRecordDigestV1,
) -> ZkAmsPrivacyActionGovernanceV1 {
    ZkAmsPrivacyActionGovernanceV1 {
        issuer_id: bootstrap.issuer_id,
        issuer_public_key: bootstrap.issuer_public_key,
        issuer_policy_record_digest: bootstrap.issuer_policy_record_digest(),
        registry_id: bootstrap.registry_id,
        registry_record_digest,
        policy_id: bootstrap.policy_id,
        policy_digest: bootstrap.policy_digest,
    }
}

fn zk_ams_statement_from_signed_v1(
    signed: &SignedZkAmsPrivacyActionV1,
    expected_effect: ZkAmsPrivacyActionEffectV1,
) -> Result<IrohaZkAmsStatementV1, PrivacyReleaseEvidenceErrorClassV1> {
    signed
        .signed_transaction()
        .verify_signature()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let (intent, submission) = signed
        .signed_transaction()
        .payload()
        .privacy_transaction_intent_binding_if_present_v1()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let PrivacyStatementV1::IrohaZkAmsV1(statement) = &submission.envelope.statement else {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    };
    let observed_effect = match &statement.action {
        PrivacyZkAmsActionV1::BatchAdmission(_) => ZkAmsPrivacyActionEffectV1::BatchAdmission,
        PrivacyZkAmsActionV1::ProvisionAccount(_) => ZkAmsPrivacyActionEffectV1::ProvisionAccount,
    };
    if signed.effect() != expected_effect
        || observed_effect != expected_effect
        || signed.transaction_intent_digest() != *intent.as_bytes()
        || statement.context.transaction_intent_digest != intent
        || signed.transaction_hash() != *signed.signed_transaction().hash().as_ref()
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok(statement.clone())
}

fn zk_ams_contexts_form_sequence_v1(
    first: &PrivacyReleaseTransactionContextV1,
    second: &PrivacyReleaseTransactionContextV1,
    provision: &PrivacyReleaseTransactionContextV1,
) -> bool {
    let same_signed_fields = |other: &PrivacyReleaseTransactionContextV1| {
        first.network_id == other.network_id
            && first.authority == other.authority
            && first.time_to_live == other.time_to_live
            && first.fee_payment == other.fee_payment
            && first.metadata == other.metadata
            && first.genesis_hash == other.genesis_hash
    };
    let strictly_increasing_nonce = matches!(
        (first.nonce, second.nonce, provision.nonce),
        (Some(first), Some(second), Some(provision))
            if first.get() < second.get() && second.get() < provision.get()
    );
    same_signed_fields(second)
        && same_signed_fields(provision)
        && first.creation_time < second.creation_time
        && second.creation_time < provision.creation_time
        && strictly_increasing_nonce
        && first.genesis_hash != [0; 32]
        && first.network_id.as_bytes() == &first.genesis_hash
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

/// Build the canonical network-bound ZK-AMS minimum-ring lifecycle bundle.
///
/// The sequence contains exactly two eight-anchor admissions followed by one account provision
/// against their common successor state. It constructs transactions only: callers remain
/// responsible for registering the returned bootstrap, submitting each transaction in order, and
/// establishing authoritative network outcomes.
#[allow(clippy::too_many_lines)]
pub fn build_privacy_release_zk_ams_network_actions_v1(
    first_admission_context: PrivacyReleaseTransactionContextV1,
    second_admission_context: PrivacyReleaseTransactionContextV1,
    provision_context: PrivacyReleaseTransactionContextV1,
    provision_account_id: AccountId,
    fixture_seed: [u8; 32],
    private_key: &PrivateKey,
) -> Result<PrivacyReleaseZkAmsNetworkActionsV1, PrivacyReleaseEvidenceErrorClassV1> {
    if !zk_ams_contexts_form_sequence_v1(
        &first_admission_context,
        &second_admission_context,
        &provision_context,
    ) || provision_account_id == first_admission_context.authority
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed);
    }
    let (bootstrap, credentials) = zk_ams_release_fixture_v1(fixture_seed)?;
    let lineage = zk_ams_release_public_lineage_v1(bootstrap, &credentials, provision_account_id)?;
    let first_witnesses = credentials[..ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1]
        .iter()
        .map(|fixture| {
            ZkAmsBatchCredentialWitnessV1::new(
                &fixture.credential,
                &fixture.issuer_signature,
                &fixture.seed_secret,
            )
        })
        .collect::<Vec<_>>();
    let mut first_rng =
        EvidenceRng06::new(network_seed_v1(fixture_seed, b"zk-ams-admission-proof", 0));
    let first_signed = build_signed_zk_ams_batch_admission_privacy_action_with_rng_v1(
        zk_ams_action_context_v1(&first_admission_context),
        zk_ams_governance_v1(
            lineage.bootstrap,
            lineage.bootstrap.registry_record_digest(),
        ),
        lineage.first_admission.clone(),
        &first_witnesses,
        ZkAmsMaskedProverConfigV1::new(1)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?,
        first_admission_context.genesis_hash,
        private_key,
        &mut first_rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let first_admission_statement =
        zk_ams_statement_from_signed_v1(&first_signed, ZkAmsPrivacyActionEffectV1::BatchAdmission)?;

    let second_witnesses = credentials[ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1..]
        .iter()
        .map(|fixture| {
            ZkAmsBatchCredentialWitnessV1::new(
                &fixture.credential,
                &fixture.issuer_signature,
                &fixture.seed_secret,
            )
        })
        .collect::<Vec<_>>();
    let mut second_rng =
        EvidenceRng06::new(network_seed_v1(fixture_seed, b"zk-ams-admission-proof", 1));
    let second_signed = build_signed_zk_ams_batch_admission_privacy_action_with_rng_v1(
        zk_ams_action_context_v1(&second_admission_context),
        zk_ams_governance_v1(lineage.bootstrap, lineage.first_registry_record_digest),
        lineage.second_admission.clone(),
        &second_witnesses,
        ZkAmsMaskedProverConfigV1::new(1)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?,
        second_admission_context.genesis_hash,
        private_key,
        &mut second_rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let second_admission_statement = zk_ams_statement_from_signed_v1(
        &second_signed,
        ZkAmsPrivacyActionEffectV1::BatchAdmission,
    )?;

    let signer_index = 5_usize;
    let signer_secret = credentials
        .get(signer_index)
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let mut provision_rng =
        EvidenceRng06::new(network_seed_v1(fixture_seed, b"zk-ams-provision-proof", 0));
    let provision_signed = build_signed_zk_ams_provision_privacy_action_with_rng_v1(
        zk_ams_action_context_v1(&provision_context),
        zk_ams_governance_v1(lineage.bootstrap, lineage.second_registry_record_digest),
        lineage.provision.clone(),
        signer_index,
        &signer_secret.seed_secret,
        provision_context.genesis_hash,
        private_key,
        &mut provision_rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let provision_statement = zk_ams_statement_from_signed_v1(
        &provision_signed,
        ZkAmsPrivacyActionEffectV1::ProvisionAccount,
    )?;

    let transaction_hashes = [
        first_signed.transaction_hash(),
        second_signed.transaction_hash(),
        provision_signed.transaction_hash(),
    ];
    let transaction_intents = [
        first_signed.transaction_intent_digest(),
        second_signed.transaction_intent_digest(),
        provision_signed.transaction_intent_digest(),
    ];
    let first_governance = zk_ams_governance_v1(
        lineage.bootstrap,
        lineage.bootstrap.registry_record_digest(),
    );
    let second_governance =
        zk_ams_governance_v1(lineage.bootstrap, lineage.first_registry_record_digest);
    let provision_governance =
        zk_ams_governance_v1(lineage.bootstrap, lineage.second_registry_record_digest);
    let statement_matches_governance =
        |statement: &IrohaZkAmsStatementV1, governance: ZkAmsPrivacyActionGovernanceV1| {
            statement.issuer_id == governance.issuer_id
                && statement.issuer_public_key == governance.issuer_public_key
                && statement.issuer_policy_record_digest == governance.issuer_policy_record_digest
                && statement.registry_id == governance.registry_id
                && statement.registry_record_digest == governance.registry_record_digest
                && statement.policy_id == governance.policy_id
                && statement.policy_digest == governance.policy_digest
        };
    if transaction_hashes
        .iter()
        .enumerate()
        .any(|(index, hash)| transaction_hashes[..index].contains(hash))
        || transaction_intents
            .iter()
            .enumerate()
            .any(|(index, intent)| transaction_intents[..index].contains(intent))
        || first_admission_statement.action
            != PrivacyZkAmsActionV1::BatchAdmission(lineage.first_admission.clone())
        || second_admission_statement.action
            != PrivacyZkAmsActionV1::BatchAdmission(lineage.second_admission.clone())
        || provision_statement.action
            != PrivacyZkAmsActionV1::ProvisionAccount(lineage.provision.clone())
        || !statement_matches_governance(&first_admission_statement, first_governance)
        || !statement_matches_governance(&second_admission_statement, second_governance)
        || !statement_matches_governance(&provision_statement, provision_governance)
        || lineage.first_admission.anchors.len() != ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1
        || lineage.second_admission.anchors.len() != ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1
        || lineage.provision.admitted_seed_key_ring.len() != ZK_AMS_MIN_RING_SIZE_V1
        || lineage.second_admission.account_registry_root
            != lineage.first_admission.next_account_registry_root
        || lineage.second_admission.account_registry_root_epoch
            != lineage.first_admission.next_account_registry_root_epoch
        || lineage.provision.account_registry_root
            != lineage.second_admission.next_account_registry_root
        || lineage.provision.account_registry_root_epoch
            != lineage.second_admission.next_account_registry_root_epoch
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok(PrivacyReleaseZkAmsNetworkActionsV1 {
        first_admission_transaction: first_signed.into_signed_transaction(),
        second_admission_transaction: second_signed.into_signed_transaction(),
        provision_transaction: provision_signed.into_signed_transaction(),
        bootstrap: lineage.bootstrap,
        first_admission_statement,
        second_admission_statement,
        provision_statement,
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, PublicKey};

    #[test]
    fn zk_ams_public_lineage_is_exactly_two_batches_then_minimum_ring_provision() {
        let (bootstrap, credentials) =
            zk_ams_release_fixture_v1([0x29; 32]).expect("construct ZK-AMS fixture");
        let account_key = PrivateKey::from_bytes(Algorithm::Ed25519, &[0x2a; 32])
            .expect("construct provision account key");
        let lineage = zk_ams_release_public_lineage_v1(
            bootstrap,
            &credentials,
            AccountId::new(PublicKey::from(account_key)),
        )
        .expect("construct public ZK-AMS lineage");

        assert_eq!(lineage.first_admission.anchors.len(), 8);
        assert_eq!(lineage.second_admission.anchors.len(), 8);
        assert_eq!(lineage.first_admission.account_registry_root_epoch, 1);
        assert_eq!(lineage.first_admission.next_account_registry_root_epoch, 2);
        assert_eq!(lineage.second_admission.account_registry_root_epoch, 2);
        assert_eq!(lineage.second_admission.next_account_registry_root_epoch, 3);
        assert_eq!(
            lineage.second_admission.account_registry_root,
            lineage.first_admission.next_account_registry_root
        );
        assert_eq!(lineage.provision.account_registry_root_epoch, 3);
        assert_eq!(
            lineage.provision.account_registry_root,
            lineage.second_admission.next_account_registry_root
        );
        assert_eq!(lineage.provision.admitted_seed_key_ring.len(), 16);
        assert_eq!(
            lineage.provision.admitted_seed_key_ring,
            lineage
                .first_admission
                .anchors
                .iter()
                .chain(&lineage.second_admission.anchors)
                .map(|anchor| anchor.seed_public_key)
                .collect::<Vec<_>>()
        );
    }
}
