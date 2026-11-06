//! Closed ordinary instruction requests, using native model types and constructors.

use eyre::{Result, ensure};
use iroha_data_model::{
    account::AccountId,
    isi::{
        InstructionBox,
        sorafs::{
            DecideSorafsReserveMovement, RegisterCapacityDeclaration, RegisterSorafsReserveAccount,
            RequestSorafsReserveMovement, SetProviderIngestCompletionAuthority,
            SetSorafsProofOutcomeSignerPolicy, SetSorafsReservePolicy, UpsertProviderCredit,
        },
    },
    sorafs::{
        capacity::ProviderId,
        pin_registry::ProviderIngestCompletionAuthorityV1,
        proof_ledger::ProofOutcomeSignerPolicyV1,
        reserve::{ReserveAuthorityPolicyV1, ReserveMovementKindV1, ReserveProviderTermsV1},
    },
};
use sorafs_manifest::{
    capacity::{CapacityDeclarationV1, CapacityMetadataEntry, ChunkerCommitmentV1},
    deal::XorQuantity,
    provider_advert::StakePointer,
};

#[derive(Debug, norito::derive::JsonDeserialize)]
#[norito(tag = "kind", content = "payload", rename_all = "snake_case")]
pub(super) enum Request {
    ReservePolicy(ReserveAuthorityPolicyV1),
    CompletionAuthority(Completion),
    ReserveAccount(ReserveAccount),
    ReserveMovement(ReserveMovement),
    ReserveDecision(ReserveDecision),
    ProviderCredit(UpsertProviderCredit),
    ProofOutcomePolicy(ProofOutcomeSignerPolicyV1),
    Capacity(Capacity),
}

#[derive(Debug, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct Completion {
    provider_id: ProviderId,
    #[norito(required)]
    expected_current: Option<ProviderIngestCompletionAuthorityV1>,
    next: ProviderIngestCompletionAuthorityV1,
}

#[derive(Debug, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct ReserveAccount {
    terms: ReserveProviderTermsV1,
    policy_digest: [u8; 32],
}

#[derive(Debug, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct ReserveMovement {
    movement_id: [u8; 32],
    provider_id: ProviderId,
    kind: ReserveMovementKindV1,
    amount: XorQuantity,
    expected_provider_revision: u64,
    policy_digest: [u8; 32],
}

#[derive(Debug, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct ReserveDecision {
    movement_id: [u8; 32],
    expected_provider_revision: u64,
    policy_digest: [u8; 32],
    approve: bool,
    rationale: String,
}

#[derive(Debug, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct Capacity {
    provider_id: [u8; 32],
    owner: AccountId,
    stake_pool_id: [u8; 32],
    stake_xor: String,
    committed_capacity_gib: u64,
    profile_id: String,
    storage_class: String,
    valid_from: u64,
    valid_until: u64,
}

pub(super) fn build(request: Request, now: u64) -> Result<InstructionBox> {
    Ok(match request {
        Request::ReservePolicy(policy) => {
            policy.validate()?;
            println!(
                "reserve_policy_digest_hex={}",
                hex::encode(policy.digest()?)
            );
            SetSorafsReservePolicy::new(policy).into()
        }
        Request::CompletionAuthority(request) => {
            nonzero(request.provider_id.as_bytes(), "provider")?;
            ensure!(
                request.next.is_valid()
                    && request.next.provider_owner != request.next.completion_signer,
                "completion authority requires a valid policy and separate owner/signer"
            );
            ensure!(
                request
                    .expected_current
                    .as_ref()
                    .is_none_or(|old| old.is_valid()),
                "invalid expected completion authority"
            );
            SetProviderIngestCompletionAuthority::new(
                request.provider_id,
                request.expected_current,
                request.next,
            )
            .into()
        }
        Request::ReserveAccount(request) => {
            nonzero(request.terms.provider_id.as_bytes(), "provider")?;
            nonzero(&request.policy_digest, "reserve policy")?;
            ensure!(
                request.terms.capacity_gib > 0,
                "reserve capacity must be positive"
            );
            RegisterSorafsReserveAccount::new(request.terms, request.policy_digest).into()
        }
        Request::ReserveMovement(request) => {
            nonzero(&request.movement_id, "movement")?;
            nonzero(request.provider_id.as_bytes(), "provider")?;
            nonzero(&request.policy_digest, "reserve policy")?;
            ensure!(
                request.expected_provider_revision > 0 && !request.amount.is_zero(),
                "movement revision and amount must be positive"
            );
            RequestSorafsReserveMovement::new(
                request.movement_id,
                request.provider_id,
                request.kind,
                request.amount,
                request.expected_provider_revision,
                request.policy_digest,
            )
            .into()
        }
        Request::ReserveDecision(request) => {
            nonzero(&request.movement_id, "movement")?;
            nonzero(&request.policy_digest, "reserve policy")?;
            ensure!(
                request.expected_provider_revision > 0
                    && !request.rationale.trim().is_empty()
                    && request.rationale.len() <= 1024,
                "decision revision or bounded rationale is invalid"
            );
            DecideSorafsReserveMovement::new(
                request.movement_id,
                request.expected_provider_revision,
                request.policy_digest,
                request.approve,
                request.rationale,
            )
            .into()
        }
        Request::ProviderCredit(request) => {
            nonzero(request.record.provider_id.as_bytes(), "provider")?;
            ensure!(
                request.record.onboarding_epoch <= now
                    && request.record.last_settlement_epoch <= now,
                "credit epochs cannot be in the future"
            );
            request.into()
        }
        Request::ProofOutcomePolicy(policy) => {
            nonzero(policy.provider_id.as_bytes(), "provider")?;
            nonzero(&policy.admission_envelope_digest, "admission")?;
            ensure!(
                policy.version == 1
                    && policy.revision > 0
                    && policy.valid_from_unix > 0
                    && policy.valid_from_unix <= now
                    && now <= policy.valid_until_unix,
                "invalid proof policy version, revision or lifetime"
            );
            ensure!(
                matches!((policy.revision, policy.predecessor_digest), (1, None))
                    || (policy.revision > 1
                        && policy
                            .predecessor_digest
                            .is_some_and(|digest| digest != [0; 32])),
                "invalid proof policy predecessor"
            );
            iroha_crypto::PublicKey::from_bytes(
                iroha_crypto::Algorithm::Ed25519,
                &policy.pdp_public_key,
            )?;
            iroha_crypto::PublicKey::from_bytes(
                iroha_crypto::Algorithm::Ed25519,
                &policy.gateway_public_key,
            )?;
            iroha_crypto::PublicKey::from_bytes(
                iroha_crypto::Algorithm::MlDsa,
                &policy.potr_mldsa_public_key,
            )?;
            sorafs_manifest::provider_advert::validate_potr_mldsa_capability(
                &policy.potr_mldsa_public_key,
            )?;
            SetSorafsProofOutcomeSignerPolicy::new(policy).into()
        }
        Request::Capacity(request) => {
            ensure!(
                request.valid_from <= now && now < request.valid_until,
                "capacity lifetime must contain current time"
            );
            ensure!(
                matches!(request.storage_class.as_str(), "hot" | "warm" | "cold"),
                "storage class must be hot, warm or cold"
            );
            let declaration = CapacityDeclarationV1 {
                version: 1,
                provider_id: request.provider_id,
                stake: StakePointer {
                    pool_id: request.stake_pool_id,
                    stake_amount: super::super::super::parse_xor_quantity_labeled(
                        &request.stake_xor,
                        "capacity stake",
                    )?,
                },
                committed_capacity_gib: request.committed_capacity_gib,
                chunker_commitments: vec![ChunkerCommitmentV1 {
                    profile_id: request.profile_id,
                    profile_aliases: None,
                    committed_gib: request.committed_capacity_gib,
                    capability_refs: Vec::new(),
                }],
                lane_commitments: Vec::new(),
                pricing: None,
                valid_from: request.valid_from,
                valid_until: request.valid_until,
                metadata: vec![
                    CapacityMetadataEntry {
                        key: "sorafs.owner_account_id".into(),
                        value: request.owner.to_string(),
                    },
                    CapacityMetadataEntry {
                        key: "sorafs.storage_class".into(),
                        value: request.storage_class,
                    },
                ],
            };
            declaration.validate()?;
            RegisterCapacityDeclaration::new(norito::encode_canonical(&declaration)?).into()
        }
    })
}

fn nonzero(value: &[u8; 32], label: &str) -> Result<()> {
    ensure!(*value != [0; 32], "{label} identity must be nonzero");
    Ok(())
}
