//! Four-validator regressions for SoraFS moderation sortition and bond settlement.

use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    crypto::{KeyPair, Signature},
    data_model::{
        isi::sorafs::{
            AcceptSorafsModerationJurorAssignment, ActivateSorafsModerationCase,
            CommitSorafsPopCredentialBatch, FinalizeSorafsModerationSortition,
            RaiseSorafsModerationChallenge, RegisterSorafsModerationJurorEligibility,
            ResolveSorafsModerationChallenge, SetSorafsModerationPolicy, SetSorafsPopIssuerPolicy,
            SubmitSorafsModerationAppeal,
        },
        prelude::*,
        query::{
            asset::prelude::{FindAssetDefinitionById, FindAssetsByAccountId},
            block::prelude::FindBlocks,
            sorafs::prelude::{
                FindSorafsModerationAppeal, FindSorafsModerationCase, FindSorafsModerationChallenge,
            },
        },
        sorafs::{
            moderation_ledger::{
                MODERATION_APPEAL_INTAKE_VERSION_V1, MODERATION_CHALLENGE_BOND_AMOUNT_V1,
                MODERATION_CHALLENGE_REJECTED_SLASH_BPS_V1,
                MODERATION_CHALLENGE_RESOLUTION_GRACE_MS_V1, MODERATION_LEDGER_POLICY_VERSION_V1,
                ModerationAppealIntakeV1, ModerationAppealRecordV1, ModerationAppealStatusV1,
                ModerationCaseRecordV1, ModerationCaseStatusV1, ModerationChallengeDecisionV1,
                ModerationChallengeKindV1, ModerationChallengeRecordV1, ModerationLedgerPolicyV1,
                ModerationSortitionAnchorV1, sorafs_moderation_pop_challenge_v1,
                sorafs_moderation_pop_verifier_context_v1,
            },
            pop_registry::{
                POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1, POP_ISSUER_POLICY_VERSION_V1,
                PopCredentialCommitmentBatchV1, PopCredentialCommitmentV1, PopIssuerPolicyV1,
                pop_credential_payload_commitment_v1, pop_revocation_nonce_commitment_v1,
            },
        },
        transaction::FeePaymentIntent,
    },
};
use iroha_executor_data_model::permission::{
    query::CanReadAllLedgerData,
    sorafs::{CanManageSorafsModeration, CanManageSorafsPopRegistry, CanOperateSorafsPopIssuer},
};
use iroha_primitives::numeric::Quantity;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{
    ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR, CARPENTER_ID, CARPENTER_KEYPAIR,
    SAMPLE_GENESIS_ACCOUNT_ID,
};
use sorafs_manifest::pop_credentials::{
    POP_COMMITMENT_ROOT_VERSION_V1, POP_CREDENTIAL_TREE_DEPTH_V1, POP_CREDENTIAL_VERSION_V1,
    POP_REVOCATION_LIST_VERSION_V1, POP_REVOCATION_TREE_DEPTH_V1, PopCommitmentRootV1,
    PopCredentialAttributeV1, PopCredentialMerklePathV1, PopCredentialV1, PopEligibilityClassV1,
    PopMembershipProofV1, PopMembershipWitnessV1, PopRevocationListV1,
    PopRevocationNonMembershipPathV1, PopSignatureAlgorithmV1, PopSignatureV1,
    build_pop_revocation_non_membership_path_v1, derive_pop_holder_commitment_v1,
    pop_commitment_root_signature_digest_v1, pop_credential_leaf_v1,
    pop_credential_root_from_path_v1, pop_credential_signature_digest_v1,
    pop_revocation_list_signature_digest_v1, pop_revocation_root_v1, prove_pop_membership_v1,
    verify_pop_commitment_root_signature_v1, verify_pop_credential_signature_v1,
    verify_pop_revocation_list_signature_v1,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::{Instant, sleep};

const CASE_ID: &str = "four-peer-anchor";
const ROUND_ID: &str = "round-1";
const ALICE_CHALLENGE_ID: &str = "alice-rejected";
const CARPENTER_CHALLENGE_ID: &str = "carpenter-accepted";
const POP_CREDENTIAL_LIFETIME_SECS: u64 = 2 * 24 * 60 * 60;
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const CONVERGENCE_TIMEOUT: Duration = Duration::from_secs(180);
const TRANSACTION_TIMEOUT: Duration = Duration::from_secs(180);

fn no_fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(Vec::new(), None)
}

fn unix_time_ms() -> Result<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .wrap_err("system clock predates the Unix epoch")?
            .as_millis(),
    )
    .wrap_err("current Unix time does not fit u64 milliseconds")
}

fn bounded_client(mut client: Client) -> Client {
    client.transaction_status_timeout = TRANSACTION_TIMEOUT;
    client.transaction_ttl = Some(Duration::from_secs(300));
    client
}

fn bob_client(peer: &iroha_test_network::NetworkPeer) -> Client {
    bounded_client(peer.client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone()))
}

fn alice_client(peer: &iroha_test_network::NetworkPeer) -> Client {
    bounded_client(peer.client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone()))
}

fn carpenter_client(peer: &iroha_test_network::NetworkPeer) -> Client {
    bounded_client(peer.client_for(&CARPENTER_ID, CARPENTER_KEYPAIR.private_key().clone()))
}

fn public_key_bytes(keypair: &KeyPair) -> [u8; 32] {
    let (_, bytes) = keypair
        .public_key()
        .try_to_bytes()
        .expect("fixture Ed25519 public key must encode");
    bytes
        .try_into()
        .expect("fixture Ed25519 public key must contain 32 bytes")
}

fn empty_pop_signature(keypair: &KeyPair) -> PopSignatureV1 {
    PopSignatureV1 {
        algorithm: PopSignatureAlgorithmV1::Ed25519,
        public_key: public_key_bytes(keypair).to_vec(),
        signature: Vec::new(),
    }
}

fn sign_pop_digest(keypair: &KeyPair, digest: [u8; 32]) -> Vec<u8> {
    Signature::try_new(keypair.private_key(), &digest)
        .expect("fixture PoP digest must sign")
        .payload()
        .to_vec()
}

fn sign_pop_credential(mut credential: PopCredentialV1, keypair: &KeyPair) -> PopCredentialV1 {
    credential.issuer_signature = empty_pop_signature(keypair);
    let digest = pop_credential_signature_digest_v1(&credential)
        .expect("fixture PoP credential signature digest");
    credential.issuer_signature.signature = sign_pop_digest(keypair, digest);
    verify_pop_credential_signature_v1(&credential)
        .expect("fixture PoP credential signature verifies");
    credential
}

fn sign_pop_root(mut root: PopCommitmentRootV1, keypair: &KeyPair) -> PopCommitmentRootV1 {
    root.publisher_signature = empty_pop_signature(keypair);
    let digest =
        pop_commitment_root_signature_digest_v1(&root).expect("fixture PoP root signature digest");
    root.publisher_signature.signature = sign_pop_digest(keypair, digest);
    verify_pop_commitment_root_signature_v1(&root).expect("fixture PoP root signature verifies");
    root
}

fn sign_pop_revocations(
    mut publication: PopRevocationListV1,
    keypair: &KeyPair,
) -> PopRevocationListV1 {
    publication.publisher_signature = empty_pop_signature(keypair);
    let digest = pop_revocation_list_signature_digest_v1(&publication)
        .expect("fixture PoP revocation signature digest");
    publication.publisher_signature.signature = sign_pop_digest(keypair, digest);
    verify_pop_revocation_list_signature_v1(&publication)
        .expect("fixture PoP revocation signature verifies");
    publication
}

fn scalar(value: u64) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes[..8].copy_from_slice(&value.to_le_bytes());
    bytes
}

fn pop_nonce(value: u128) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes[..16].copy_from_slice(&value.to_le_bytes());
    bytes
}

struct PopMaterial {
    credential: PopCredentialV1,
    root: PopCommitmentRootV1,
    revocations: PopRevocationListV1,
    holder_secret: [u8; 32],
    credential_path: PopCredentialMerklePathV1,
    revocation_path: PopRevocationNonMembershipPathV1,
}

impl PopMaterial {
    fn proof(
        &self,
        challenge: [u8; 32],
        verifier_context: &str,
        now_epoch: u64,
    ) -> PopMembershipProofV1 {
        prove_pop_membership_v1(
            &self.credential,
            &self.root,
            &self.revocations,
            &PopMembershipWitnessV1 {
                holder_secret: self.holder_secret,
                credential_path: self.credential_path.clone(),
                revocation_path: self.revocation_path.clone(),
            },
            challenge,
            verifier_context,
            now_epoch,
        )
        .expect("create challenge-bound moderation PoP proof")
    }
}

fn pop_material(issuer: &KeyPair, published_at_epoch: u64) -> PopMaterial {
    let holder_secret = scalar(0x1234_5678);
    let credential_id = scalar(0x8765_4321);
    let holder_commitment = derive_pop_holder_commitment_v1(holder_secret, credential_id)
        .expect("derive fixture PoP holder commitment");
    let revocation_nonce = pop_nonce(0xfeed_beef_dead_cafe_1234_5678_9abc_def0);
    let issued_at_epoch = published_at_epoch.saturating_sub(60).max(1);
    let expires_at_epoch = published_at_epoch.saturating_add(POP_CREDENTIAL_LIFETIME_SECS);
    let mut credential = PopCredentialV1 {
        version: POP_CREDENTIAL_VERSION_V1,
        credential_id,
        holder_commitment,
        eligibility_class: PopEligibilityClassV1::General,
        attributes: vec![PopCredentialAttributeV1 {
            key: "residency".to_owned(),
            value_commitment: [0x13; 32],
        }],
        issuer_id: "pop-issuer-sora-foundation".to_owned(),
        issued_at_epoch,
        expires_at_epoch,
        renewal_at_epoch: published_at_epoch.saturating_add(24 * 60 * 60),
        revocation_nonce,
        commitment_root: scalar(1),
        commitment_tree_version: 1,
        revocation_list_version: 1,
        issuer_signature: empty_pop_signature(issuer),
    };
    credential = sign_pop_credential(credential, issuer);
    let credential_path = PopCredentialMerklePathV1 {
        siblings: vec![scalar(0); usize::from(POP_CREDENTIAL_TREE_DEPTH_V1)],
        directions: (0..usize::from(POP_CREDENTIAL_TREE_DEPTH_V1))
            .map(|level| level % 3 == 1)
            .collect(),
    };
    let leaf = pop_credential_leaf_v1(&credential).expect("fixture PoP credential leaf");
    let root_digest = pop_credential_root_from_path_v1(leaf, &credential_path)
        .expect("fixture PoP credential root");
    credential.commitment_root = root_digest;
    credential = sign_pop_credential(credential, issuer);
    let root = sign_pop_root(
        PopCommitmentRootV1 {
            version: POP_COMMITMENT_ROOT_VERSION_V1,
            root_digest,
            tree_size: 1,
            tree_depth: POP_CREDENTIAL_TREE_DEPTH_V1,
            tree_version: 1,
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            published_at_epoch,
            previous_root_digest: None,
            governance_event_digest: [0x42; 32],
            publisher_signature: empty_pop_signature(issuer),
        },
        issuer,
    );
    let revocations = sign_pop_revocations(
        PopRevocationListV1 {
            version: POP_REVOCATION_LIST_VERSION_V1,
            list_version: 1,
            commitment_root: root_digest,
            revocation_root: pop_revocation_root_v1(&[])
                .expect("empty PoP revocation tree has a canonical root"),
            revocation_tree_depth: POP_REVOCATION_TREE_DEPTH_V1,
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            published_at_epoch,
            entries: Vec::new(),
            publisher_signature: empty_pop_signature(issuer),
        },
        issuer,
    );
    let revocation_path = build_pop_revocation_non_membership_path_v1(
        &revocations.entries,
        credential.revocation_nonce,
    )
    .expect("fixture PoP revocation non-membership path");
    PopMaterial {
        credential,
        root,
        revocations,
        holder_secret,
        credential_path,
        revocation_path,
    }
}

fn pop_policy(issuer: &KeyPair) -> PopIssuerPolicyV1 {
    PopIssuerPolicyV1 {
        version: POP_ISSUER_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        issuer_id: "pop-issuer-sora-foundation".to_owned(),
        issuer_account: BOB_ID.clone(),
        issuer_public_key: public_key_bytes(issuer),
        max_credentials_per_batch: 4,
        max_revocations_per_publication: 4,
        max_credential_lifetime_secs: POP_CREDENTIAL_LIFETIME_SECS + 120,
        max_future_clock_skew_secs: 5,
        paused: false,
    }
}

fn pop_batch(issuer: &KeyPair, published_at_epoch: u64) -> PopCredentialCommitmentBatchV1 {
    let root_digest = [0x41; 32];
    let root = sign_pop_root(
        PopCommitmentRootV1 {
            version: POP_COMMITMENT_ROOT_VERSION_V1,
            root_digest,
            tree_size: 1,
            tree_depth: POP_CREDENTIAL_TREE_DEPTH_V1,
            tree_version: 1,
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            published_at_epoch,
            previous_root_digest: None,
            governance_event_digest: [0x42; 32],
            publisher_signature: empty_pop_signature(issuer),
        },
        issuer,
    );
    let revocations = sign_pop_revocations(
        PopRevocationListV1 {
            version: POP_REVOCATION_LIST_VERSION_V1,
            list_version: 1,
            commitment_root: root_digest,
            revocation_root: pop_revocation_root_v1(&[])
                .expect("empty PoP revocation tree has a canonical root"),
            revocation_tree_depth: POP_REVOCATION_TREE_DEPTH_V1,
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            published_at_epoch,
            entries: Vec::new(),
            publisher_signature: empty_pop_signature(issuer),
        },
        issuer,
    );
    PopCredentialCommitmentBatchV1 {
        version: POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1,
        issuer_policy_digest: pop_policy(issuer)
            .digest()
            .expect("fixture PoP policy digest"),
        commitment_root_payload: norito::encode_canonical(&root)
            .expect("encode canonical signed PoP root"),
        revocation_list_payload: norito::encode_canonical(&revocations)
            .expect("encode canonical signed PoP revocations"),
        commitments: vec![PopCredentialCommitmentV1 {
            credential_commitment: [0x51; 32],
            revocation_nonce_commitment: [0x52; 32],
            commitment_root: root_digest,
            commitment_tree_version: 1,
            revocation_list_version: 1,
            issued_at_epoch: published_at_epoch.saturating_sub(10).max(1),
            expires_at_epoch: published_at_epoch.saturating_add(3_600),
        }],
    }
}

fn valid_pop_batch(issuer: &KeyPair, material: &PopMaterial) -> PopCredentialCommitmentBatchV1 {
    let canonical_credential =
        norito::encode_canonical(&material.credential).expect("encode canonical PoP credential");
    PopCredentialCommitmentBatchV1 {
        version: POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1,
        issuer_policy_digest: pop_policy(issuer)
            .digest()
            .expect("fixture PoP policy digest"),
        commitment_root_payload: norito::encode_canonical(&material.root)
            .expect("encode canonical signed PoP root"),
        revocation_list_payload: norito::encode_canonical(&material.revocations)
            .expect("encode canonical signed PoP revocations"),
        commitments: vec![PopCredentialCommitmentV1 {
            credential_commitment: pop_credential_payload_commitment_v1(&canonical_credential),
            revocation_nonce_commitment: pop_revocation_nonce_commitment_v1(
                material.credential.revocation_nonce,
            ),
            commitment_root: material.root.root_digest,
            commitment_tree_version: material.root.tree_version,
            revocation_list_version: material.revocations.list_version,
            issued_at_epoch: material.credential.issued_at_epoch,
            expires_at_epoch: material.credential.expires_at_epoch,
        }],
    }
}

fn moderation_policy() -> ModerationLedgerPolicyV1 {
    ModerationLedgerPolicyV1 {
        version: MODERATION_LEDGER_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        challenge_voting_asset_id: iroha_config::parameters::defaults::governance::voting_asset_id(
        )
        .parse()
        .expect("default governance voting asset must be canonical"),
        challenge_bond_amount: Quantity::from(MODERATION_CHALLENGE_BOND_AMOUNT_V1),
        challenge_escrow_account:
            iroha_config::parameters::defaults::governance::bond_escrow_account_id(),
        challenge_slash_receiver_account:
            iroha_config::parameters::defaults::governance::slash_receiver_account_id(),
        challenge_rejected_slash_bps: MODERATION_CHALLENGE_REJECTED_SLASH_BPS_V1,
        challenge_resolution_grace_ms: MODERATION_CHALLENGE_RESOLUTION_GRACE_MS_V1,
        max_panel_size: 4,
        max_candidate_pool_size: 16,
        max_waitlist_size: 4,
        max_exclusions_per_case: 8,
        max_total_window_ms: 90_000_000,
        max_challenges_per_case: 4,
        missing_commit_penalty_points: 11,
        unrevealed_commit_penalty_points: 23,
    }
}

fn moderation_policy_with_custody(
    escrow_account: AccountId,
    slash_receiver_account: AccountId,
) -> ModerationLedgerPolicyV1 {
    let mut policy = moderation_policy();
    policy.challenge_escrow_account = escrow_account;
    policy.challenge_slash_receiver_account = slash_receiver_account;
    policy
}

fn appeal_intake(registration_deadline_unix_ms: u64) -> ModerationAppealIntakeV1 {
    let acceptance_deadline_unix_ms = registration_deadline_unix_ms + 60_000;
    let commit_deadline_unix_ms = acceptance_deadline_unix_ms + 60_000;
    let challenge_submission_deadline_unix_ms = commit_deadline_unix_ms + 60_000;
    let challenge_resolution_deadline_unix_ms =
        challenge_submission_deadline_unix_ms + MODERATION_CHALLENGE_RESOLUTION_GRACE_MS_V1;
    ModerationAppealIntakeV1 {
        version: MODERATION_APPEAL_INTAKE_VERSION_V1,
        case_id: CASE_ID.to_owned(),
        round_id: ROUND_ID.to_owned(),
        appellant: ALICE_ID.clone(),
        appealed_decision_digest: [0x61; 32],
        proof_token_digest: [0x62; 32],
        evidence_bundle_digest: [0x63; 32],
        appeal_deposit_lock_digest: [0x64; 32],
        appeal_finance_config_version: "finance-v1".to_owned(),
        policy_reference: "policy-v1".to_owned(),
        evidence_uri: Some("ipfs://four-peer-anchor-evidence".to_owned()),
        panel_size: 1,
        waitlist_size: 0,
        quorum: 1,
        exclusions: vec![ALICE_ID.clone()],
        registration_deadline_unix_ms,
        acceptance_deadline_unix_ms,
        commit_deadline_unix_ms,
        challenge_submission_deadline_unix_ms,
        challenge_resolution_deadline_unix_ms,
        reveal_deadline_unix_ms: challenge_resolution_deadline_unix_ms + 60_000,
        policy_digest: moderation_policy()
            .digest()
            .expect("fixture moderation policy digest"),
    }
}

fn settlement_appeal_intake(
    registration_deadline_unix_ms: u64,
    policy_digest: [u8; 32],
) -> ModerationAppealIntakeV1 {
    let acceptance_deadline_unix_ms = registration_deadline_unix_ms + 30_000;
    let commit_deadline_unix_ms = acceptance_deadline_unix_ms + 20_000;
    let challenge_submission_deadline_unix_ms = commit_deadline_unix_ms + 120_000;
    let challenge_resolution_deadline_unix_ms =
        challenge_submission_deadline_unix_ms + MODERATION_CHALLENGE_RESOLUTION_GRACE_MS_V1;
    ModerationAppealIntakeV1 {
        version: MODERATION_APPEAL_INTAKE_VERSION_V1,
        case_id: CASE_ID.to_owned(),
        round_id: ROUND_ID.to_owned(),
        appellant: ALICE_ID.clone(),
        appealed_decision_digest: [0x71; 32],
        proof_token_digest: [0x72; 32],
        evidence_bundle_digest: [0x73; 32],
        appeal_deposit_lock_digest: [0x74; 32],
        appeal_finance_config_version: "finance-v1".to_owned(),
        policy_reference: "policy-v1".to_owned(),
        evidence_uri: Some("ipfs://four-peer-settlement-evidence".to_owned()),
        panel_size: 1,
        waitlist_size: 0,
        quorum: 1,
        exclusions: vec![ALICE_ID.clone()],
        registration_deadline_unix_ms,
        acceptance_deadline_unix_ms,
        commit_deadline_unix_ms,
        challenge_submission_deadline_unix_ms,
        challenge_resolution_deadline_unix_ms,
        reveal_deadline_unix_ms: challenge_resolution_deadline_unix_ms + 60_000,
        policy_digest,
    }
}

async fn wait_for_appeals(
    network: &sandbox::SerializedNetwork,
    label: &str,
    predicate: impl Fn(&ModerationAppealRecordV1) -> bool,
) -> Result<Vec<ModerationAppealRecordV1>> {
    let deadline = Instant::now() + CONVERGENCE_TIMEOUT;
    loop {
        let mut appeals = Vec::with_capacity(network.peers().len());
        let mut failures = Vec::new();
        for (index, peer) in network.peers().iter().enumerate() {
            match bob_client(peer)
                .query(FindSorafsModerationAppeal::new(
                    CASE_ID.to_owned(),
                    ROUND_ID.to_owned(),
                ))
                .execute_single()
            {
                Ok(appeal) => appeals.push(appeal),
                Err(error) => failures.push(format!("peer {index}: {error}")),
            }
        }
        if failures.is_empty()
            && appeals.len() == network.peers().len()
            && appeals.iter().all(&predicate)
        {
            return Ok(appeals);
        }
        let last_observation = if failures.is_empty() {
            appeals
                .iter()
                .map(|appeal| {
                    format!(
                        "{:?}/anchor={:?}",
                        appeal.status,
                        appeal
                            .sortition_anchor
                            .as_ref()
                            .map(|anchor| anchor.block_height)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            failures.join("; ")
        };
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{label}: four peers did not converge before timeout; last observation: {last_observation}"
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}

async fn sleep_past(deadline_unix_ms: u64) -> Result<()> {
    let wait_ms = deadline_unix_ms
        .saturating_sub(unix_time_ms()?)
        .saturating_add(1_000);
    sleep(Duration::from_millis(wait_ms)).await;
    Ok(())
}

#[derive(Debug)]
struct SettlementObservation {
    case: ModerationCaseRecordV1,
    alice_challenge: ModerationChallengeRecordV1,
    carpenter_challenge: Option<ModerationChallengeRecordV1>,
    balances: [Quantity; 4],
    total_quantity: Quantity,
}

#[derive(Debug, PartialEq, Eq)]
struct CanonicalSettlementSnapshot {
    case: Vec<u8>,
    alice_challenge: Vec<u8>,
    carpenter_challenge: Option<Vec<u8>>,
    balances: Vec<Vec<u8>>,
}

impl SettlementObservation {
    fn canonical_snapshot(&self) -> Result<CanonicalSettlementSnapshot> {
        let case = norito::encode_canonical(&self.case)
            .wrap_err("encode canonical moderation case snapshot")?;
        let alice_challenge = norito::encode_canonical(&self.alice_challenge)
            .wrap_err("encode canonical Alice challenge snapshot")?;
        let carpenter_challenge = self
            .carpenter_challenge
            .as_ref()
            .map(|challenge| {
                norito::encode_canonical(challenge)
                    .wrap_err("encode canonical Carpenter challenge snapshot")
            })
            .transpose()?;
        let balances = self
            .balances
            .iter()
            .chain(std::iter::once(&self.total_quantity))
            .map(|balance| {
                norito::encode_canonical(balance).wrap_err("encode canonical balance snapshot")
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(CanonicalSettlementSnapshot {
            case,
            alice_challenge,
            carpenter_challenge,
            balances,
        })
    }
}

fn voting_asset_balance(
    client: &Client,
    voting_asset_id: &AssetDefinitionId,
    account: &AccountId,
) -> Result<Quantity> {
    Ok(client
        .query(FindAssetsByAccountId::new(account.clone()))
        .execute_all()?
        .into_iter()
        .find(|asset| asset.id().definition() == voting_asset_id)
        .map_or_else(Quantity::zero, |asset| asset.value().clone()))
}

fn settlement_observation(
    client: &Client,
    voting_asset_id: &AssetDefinitionId,
    include_carpenter_challenge: bool,
) -> Result<SettlementObservation> {
    let case = client
        .query(FindSorafsModerationCase::new(
            CASE_ID.to_owned(),
            ROUND_ID.to_owned(),
        ))
        .execute_single()?;
    let alice_challenge = client
        .query(FindSorafsModerationChallenge::new(
            CASE_ID.to_owned(),
            ROUND_ID.to_owned(),
            ALICE_CHALLENGE_ID.to_owned(),
        ))
        .execute_single()?;
    let carpenter_challenge = include_carpenter_challenge
        .then(|| {
            client
                .query(FindSorafsModerationChallenge::new(
                    CASE_ID.to_owned(),
                    ROUND_ID.to_owned(),
                    CARPENTER_CHALLENGE_ID.to_owned(),
                ))
                .execute_single()
        })
        .transpose()?;
    let balances = [
        voting_asset_balance(client, voting_asset_id, &ALICE_ID)?,
        voting_asset_balance(client, voting_asset_id, &CARPENTER_ID)?,
        voting_asset_balance(client, voting_asset_id, &BOB_ID)?,
        voting_asset_balance(client, voting_asset_id, &SAMPLE_GENESIS_ACCOUNT_ID)?,
    ];
    let total_quantity = client
        .query(FindAssetDefinitionById::new(voting_asset_id.clone()))
        .execute_single()?
        .total_quantity()
        .clone();
    Ok(SettlementObservation {
        case,
        alice_challenge,
        carpenter_challenge,
        balances,
        total_quantity,
    })
}

fn exactly_conserves_voting_asset(observation: &SettlementObservation) -> bool {
    let expected = Quantity::from(2_000_u32);
    observation
        .balances
        .iter()
        .try_fold(Quantity::zero(), |total, balance| {
            total.checked_add(balance)
        })
        .is_ok_and(|total| total == expected)
        && observation.total_quantity == expected
}

async fn wait_for_settlement_convergence(
    network: &sandbox::SerializedNetwork,
    voting_asset_id: &AssetDefinitionId,
    label: &str,
    include_carpenter_challenge: bool,
    predicate: impl Fn(&SettlementObservation) -> bool,
) -> Result<()> {
    let deadline = Instant::now() + CONVERGENCE_TIMEOUT;
    loop {
        let mut observations = Vec::with_capacity(network.peers().len());
        let mut failures = Vec::new();
        for (index, peer) in network.peers().iter().enumerate() {
            match settlement_observation(
                &bob_client(peer),
                voting_asset_id,
                include_carpenter_challenge,
            ) {
                Ok(observation) => observations.push(observation),
                Err(error) => failures.push(format!("peer {index}: {error}")),
            }
        }
        if failures.is_empty()
            && observations.len() == network.peers().len()
            && observations.iter().all(&predicate)
        {
            let snapshots = observations
                .iter()
                .map(SettlementObservation::canonical_snapshot)
                .collect::<Result<Vec<_>>>()?;
            let first = snapshots
                .first()
                .ok_or_else(|| eyre!("{label}: no peer settlement snapshots"))?;
            ensure!(
                snapshots.iter().all(|snapshot| snapshot == first),
                "{label}: case, challenge, or balance bytes diverged across four peers"
            );
            return Ok(());
        }
        let last_observation = if failures.is_empty() {
            observations
                .iter()
                .map(|observation| {
                    format!(
                        "case={:?}/pending={}/accepted={}; alice={:?}/{}/{}; carpenter={:?}; balances={:?}/total={}",
                        observation.case.status,
                        observation.case.pending_challenge_count,
                        observation.case.accepted_challenge_count,
                        observation.alice_challenge.decision,
                        observation.alice_challenge.bond.refunded_amount,
                        observation.alice_challenge.bond.slashed_amount,
                        observation
                            .carpenter_challenge
                            .as_ref()
                            .and_then(|challenge| challenge.decision),
                        observation.balances,
                        observation.total_quantity,
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            failures.join("; ")
        };
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{label}: four peers did not converge before timeout; last observation: {last_observation}"
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}

fn exact_block(client: &Client, height: u64) -> Result<SignedBlock> {
    let block = client
        .query(FindBlocks)
        .filter_with(|block| block.equals("height", height).into_predicate())
        .execute_single()
        .map_err(|error| eyre!("query exact finalized block {height}: {error}"))?;
    ensure!(
        block.header().height().get() == height,
        "exact finalized-block query returned height {} for requested height {height}",
        block.header().height()
    );
    Ok(block)
}

fn assert_exact_first_post_deadline_anchor(
    network: &sandbox::SerializedNetwork,
    anchor: ModerationSortitionAnchorV1,
    registration_deadline_unix_ms: u64,
) -> Result<()> {
    ensure!(
        anchor.block_height > 1,
        "four-peer moderation anchor unexpectedly resolved at genesis"
    );
    ensure!(
        anchor.block_timestamp_unix_ms > registration_deadline_unix_ms,
        "anchor timestamp must be strictly after registration"
    );
    for (index, peer) in network.peers().iter().enumerate() {
        let client = bob_client(peer);
        let block = exact_block(&client, anchor.block_height)
            .wrap_err_with(|| format!("peer {index} anchor block"))?;
        let previous = exact_block(&client, anchor.block_height - 1)
            .wrap_err_with(|| format!("peer {index} pre-anchor block"))?;
        ensure!(
            *block.hash().as_ref() == anchor.block_hash,
            "peer {index} anchor hash differs from committed history"
        );
        ensure!(
            u64::try_from(block.header().creation_time().as_millis()).ok()
                == Some(anchor.block_timestamp_unix_ms),
            "peer {index} anchor timestamp differs from its committed header"
        );
        ensure!(
            previous.header().creation_time().as_millis()
                <= u128::from(registration_deadline_unix_ms),
            "peer {index} retained an earlier committed post-deadline block before the anchor"
        );
    }
    Ok(())
}

#[tokio::test]
async fn four_peer_moderation_sortition_anchor_is_post_deadline_and_queue_plan_stable() -> Result<()>
{
    init_instruction_registry();
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_secs(1))
        .with_npos_consensus()
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanManageSorafsPopRegistry),
            BOB_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanOperateSorafsPopIssuer),
            BOB_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanManageSorafsModeration),
            BOB_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanReadAllLedgerData),
            BOB_ID.clone(),
        ));
    let context =
        stringify!(four_peer_moderation_sortition_anchor_is_post_deadline_and_queue_plan_stable);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    ensure!(
        network.peers().len() == 4,
        "test requires exactly four voting peers"
    );

    let bob = bob_client(&network.peers()[0]);
    let now_epoch = unix_time_ms()? / 1_000;
    let pop_policy = pop_policy(&BOB_KEYPAIR);
    let moderation_policy = moderation_policy();
    bob.submit_all_blocking(
        [
            InstructionBox::from(SetSorafsPopIssuerPolicy::new(pop_policy)),
            InstructionBox::from(CommitSorafsPopCredentialBatch::new(
                norito::encode_canonical(&pop_batch(&BOB_KEYPAIR, now_epoch.saturating_sub(1)))
                    .expect("encode canonical PoP registry batch"),
            )),
            InstructionBox::from(SetSorafsModerationPolicy::new(moderation_policy)),
        ],
        no_fee(),
    )?;

    let registration_deadline_unix_ms = unix_time_ms()?.saturating_add(30_000);
    alice_client(&network.peers()[0]).submit_blocking(
        SubmitSorafsModerationAppeal::new(appeal_intake(registration_deadline_unix_ms)),
        no_fee(),
    )?;

    let wait_ms = registration_deadline_unix_ms
        .saturating_sub(unix_time_ms()?)
        .saturating_add(1_000);
    sleep(Duration::from_millis(wait_ms)).await;
    // An idle test network need not create empty blocks. This transaction starts the first
    // post-deadline QueuePlan sequence whose earliest committed carrier must pin the anchor.
    bob.submit_blocking(
        Log::new(Level::INFO, "pin due moderation anchor".to_owned()),
        no_fee(),
    )?;

    let anchored = wait_for_appeals(&network, "wait for pinned sortition anchor", |appeal| {
        appeal.status == ModerationAppealStatusV1::RegisteringJurors
            && appeal.sortition_anchor.is_some()
    })
    .await?;
    let anchor = anchored[0]
        .sortition_anchor
        .ok_or_else(|| eyre!("first peer lost the converged sortition anchor"))?;
    ensure!(
        anchored
            .iter()
            .all(|appeal| appeal.sortition_anchor == Some(anchor)),
        "four peers did not retain one byte-identical sortition anchor"
    );
    assert_exact_first_post_deadline_anchor(&network, anchor, registration_deadline_unix_ms)?;

    // Carry an unrelated transaction after the anchor before preparing sortition. The final
    // QueuePlan carrier must still consume the pinned draw rather than whichever parent is latest.
    bob.submit_blocking(
        Log::new(
            Level::INFO,
            "delayed moderation sortition carrier".to_owned(),
        ),
        no_fee(),
    )?;
    let delayed = wait_for_appeals(&network, "retain anchor after delayed carrier", |appeal| {
        appeal.status == ModerationAppealStatusV1::RegisteringJurors
            && appeal.sortition_anchor == Some(anchor)
    })
    .await?;
    ensure!(
        delayed
            .iter()
            .all(|appeal| appeal.sortition_anchor == Some(anchor)),
        "the delayed carrier changed the pinned anchor"
    );

    let appeal = &delayed[0];
    bob.submit_blocking(
        FinalizeSorafsModerationSortition::new(
            CASE_ID.to_owned(),
            ROUND_ID.to_owned(),
            appeal.pop_snapshot_digest,
            anchor.block_hash,
            Vec::new(),
            Vec::new(),
        ),
        no_fee(),
    )?;
    let finalized = wait_for_appeals(&network, "finalize insufficient panel", |appeal| {
        appeal.status == ModerationAppealStatusV1::InsufficientEligiblePool
    })
    .await?;
    ensure!(
        finalized.iter().all(|appeal| {
            appeal.sortition_anchor == Some(anchor) && appeal.selection.is_none()
        }),
        "finalization changed the pinned draw or invented an insufficient-pool selection"
    );
    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn four_peer_moderation_challenge_settlements_converge_and_conserve_bonds() -> Result<()> {
    init_instruction_registry();
    let voting_asset_id: AssetDefinitionId =
        iroha_config::parameters::defaults::governance::voting_asset_id()
            .parse()
            .wrap_err("parse default governance voting asset")?;
    let voting_domain = DomainId::try_new("sora", "universal")?;
    let escrow_literal = BOB_ID.to_string();
    let slash_receiver_literal = SAMPLE_GENESIS_ACCOUNT_ID.to_string();
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_secs(1))
        .with_npos_consensus()
        .with_config_layer(move |writer| {
            writer
                .write(["governance", "bond_escrow_account"], escrow_literal)
                .write(
                    ["governance", "slash_receiver_account"],
                    slash_receiver_literal,
                );
        })
        .with_genesis_instruction(Register::domain(Domain::new(voting_domain)))
        .with_genesis_instruction(Register::asset_definition(AssetDefinition::numeric(
            voting_asset_id.clone(),
            "moderation challenge XOR".to_owned(),
            AssetBalancePolicy::Global,
            None,
        )))
        .with_genesis_instruction(Mint::asset_quantity(
            1_000_u32,
            AssetId::new(voting_asset_id.clone(), ALICE_ID.clone()),
        ))
        .with_genesis_instruction(Mint::asset_quantity(
            1_000_u32,
            AssetId::new(voting_asset_id.clone(), CARPENTER_ID.clone()),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanManageSorafsPopRegistry),
            BOB_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanOperateSorafsPopIssuer),
            BOB_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanManageSorafsModeration),
            BOB_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanReadAllLedgerData),
            BOB_ID.clone(),
        ));
    let context =
        stringify!(four_peer_moderation_challenge_settlements_converge_and_conserve_bonds);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    ensure!(
        network.peers().len() == 4,
        "test requires exactly four voting peers"
    );

    let bob = bob_client(&network.peers()[0]);
    let published_at_epoch = (unix_time_ms()? / 1_000).saturating_sub(1).max(1);
    let material = pop_material(&BOB_KEYPAIR, published_at_epoch);
    let policy = moderation_policy_with_custody(BOB_ID.clone(), SAMPLE_GENESIS_ACCOUNT_ID.clone());
    let policy_digest = policy.digest().expect("fixture moderation policy digest");
    bob.submit_all_blocking(
        [
            InstructionBox::from(SetSorafsPopIssuerPolicy::new(pop_policy(&BOB_KEYPAIR))),
            InstructionBox::from(CommitSorafsPopCredentialBatch::new(
                norito::encode_canonical(&valid_pop_batch(&BOB_KEYPAIR, &material))
                    .expect("encode canonical valid PoP registry batch"),
            )),
            InstructionBox::from(SetSorafsModerationPolicy::new(policy)),
        ],
        no_fee(),
    )?;

    // Leave enough real time for the deterministic membership prover on debug/CI hardware.
    let registration_deadline_unix_ms = unix_time_ms()?.saturating_add(60_000);
    let intake = settlement_appeal_intake(registration_deadline_unix_ms, policy_digest);
    let acceptance_deadline_unix_ms = intake.acceptance_deadline_unix_ms;
    let commit_deadline_unix_ms = intake.commit_deadline_unix_ms;
    let reveal_deadline_epoch = intake.reveal_deadline_unix_ms.div_ceil(1_000);
    ensure!(
        material.credential.expires_at_epoch > reveal_deadline_epoch,
        "fixture credential must remain valid beyond the mandatory 24-hour reveal schedule"
    );
    alice_client(&network.peers()[0])
        .submit_blocking(SubmitSorafsModerationAppeal::new(intake), no_fee())?;

    let submitted = wait_for_appeals(&network, "wait for settlement appeal intake", |appeal| {
        appeal.status == ModerationAppealStatusV1::RegisteringJurors
    })
    .await?;
    let appeal = &submitted[0];
    let challenge =
        sorafs_moderation_pop_challenge_v1(appeal.intake_digest, appeal.pop_snapshot_digest);
    let verifier_context = sorafs_moderation_pop_verifier_context_v1(appeal.intake_digest);
    let proof = material.proof(
        challenge,
        &verifier_context,
        appeal.submitted_at_unix_ms / 1_000,
    );
    ensure!(
        proof.challenge_digest == challenge && proof.verifier_context == verifier_context,
        "fixture PoP proof must bind the admitted appeal challenge and verifier context"
    );
    carpenter_client(&network.peers()[0]).submit_blocking(
        RegisterSorafsModerationJurorEligibility::new(
            CASE_ID.to_owned(),
            ROUND_ID.to_owned(),
            norito::encode_canonical(&proof).expect("encode canonical moderation PoP proof"),
        ),
        no_fee(),
    )?;
    wait_for_appeals(&network, "wait for Carpenter eligibility", |appeal| {
        appeal.status == ModerationAppealStatusV1::RegisteringJurors
            && appeal.eligible_jurors == vec![CARPENTER_ID.clone()]
    })
    .await?;

    sleep_past(registration_deadline_unix_ms).await?;
    bob.submit_blocking(
        Log::new(Level::INFO, "pin settlement moderation anchor".to_owned()),
        no_fee(),
    )?;
    let anchored = wait_for_appeals(&network, "wait for settlement sortition anchor", |appeal| {
        appeal.status == ModerationAppealStatusV1::RegisteringJurors
            && appeal.sortition_anchor.is_some()
    })
    .await?;
    let anchor = anchored[0]
        .sortition_anchor
        .ok_or_else(|| eyre!("settlement appeal lost its converged sortition anchor"))?;
    ensure!(
        anchored
            .iter()
            .all(|appeal| appeal.sortition_anchor == Some(anchor)),
        "four peers did not retain one byte-identical settlement sortition anchor"
    );
    assert_exact_first_post_deadline_anchor(&network, anchor, registration_deadline_unix_ms)?;
    bob.submit_blocking(
        FinalizeSorafsModerationSortition::new(
            CASE_ID.to_owned(),
            ROUND_ID.to_owned(),
            anchored[0].pop_snapshot_digest,
            anchor.block_hash,
            vec![CARPENTER_ID.clone()],
            Vec::new(),
        ),
        no_fee(),
    )?;
    let selected = wait_for_appeals(&network, "wait for Carpenter sortition", |appeal| {
        appeal.status == ModerationAppealStatusV1::AwaitingAcceptance
            && appeal
                .selection
                .as_ref()
                .is_some_and(|selection| selection.jurors == vec![CARPENTER_ID.clone()])
    })
    .await?;
    let sortition_digest = selected[0]
        .selection
        .as_ref()
        .ok_or_else(|| eyre!("settlement appeal lost its panel selection"))?
        .sortition_digest;
    carpenter_client(&network.peers()[0]).submit_blocking(
        AcceptSorafsModerationJurorAssignment::new(
            CASE_ID.to_owned(),
            ROUND_ID.to_owned(),
            sortition_digest,
        ),
        no_fee(),
    )?;
    wait_for_appeals(
        &network,
        "wait for Carpenter assignment acceptance",
        |appeal| {
            appeal.status == ModerationAppealStatusV1::AwaitingAcceptance
                && appeal.accepted_jurors == vec![CARPENTER_ID.clone()]
        },
    )
    .await?;
    sleep_past(acceptance_deadline_unix_ms).await?;
    bob.submit_blocking(
        ActivateSorafsModerationCase::new(
            CASE_ID.to_owned(),
            ROUND_ID.to_owned(),
            sortition_digest,
        ),
        no_fee(),
    )?;
    wait_for_appeals(&network, "wait for activated settlement ballot", |appeal| {
        appeal.status == ModerationAppealStatusV1::BallotOpen
            && appeal.activated_at_unix_ms.is_some()
    })
    .await?;
    sleep_past(commit_deadline_unix_ms).await?;

    alice_client(&network.peers()[0]).submit_blocking(
        RaiseSorafsModerationChallenge::new(
            CASE_ID.to_owned(),
            ROUND_ID.to_owned(),
            ALICE_CHALLENGE_ID.to_owned(),
            ModerationChallengeKindV1::EvidenceMismatch,
            None,
            [0x81; 32],
            "Alice disputes the evidence binding".to_owned(),
        ),
        no_fee(),
    )?;
    wait_for_settlement_convergence(
        &network,
        &voting_asset_id,
        "Alice pending bond",
        false,
        |observation| {
            observation.case.status == ModerationCaseStatusV1::Open
                && observation.case.challenge_count == 1
                && observation.case.pending_challenge_count == 1
                && observation.case.accepted_challenge_count == 0
                && observation.alice_challenge.decision.is_none()
                && observation.alice_challenge.bond.amount == Quantity::from(150_u32)
                && observation.alice_challenge.bond.refunded_amount == Quantity::zero()
                && observation.alice_challenge.bond.slashed_amount == Quantity::zero()
                && observation.alice_challenge.bond.escrow_account == BOB_ID.clone()
                && observation.alice_challenge.bond.slash_receiver_account
                    == SAMPLE_GENESIS_ACCOUNT_ID.clone()
                && observation.balances
                    == [
                        Quantity::from(850_u32),
                        Quantity::from(1_000_u32),
                        Quantity::from(150_u32),
                        Quantity::zero(),
                    ]
                && exactly_conserves_voting_asset(observation)
        },
    )
    .await?;
    bob.submit_blocking(
        ResolveSorafsModerationChallenge::new(
            CASE_ID.to_owned(),
            ROUND_ID.to_owned(),
            ALICE_CHALLENGE_ID.to_owned(),
            ModerationChallengeDecisionV1::Rejected,
        ),
        no_fee(),
    )?;
    wait_for_settlement_convergence(
        &network,
        &voting_asset_id,
        "Alice rejected settlement",
        false,
        |observation| {
            observation.case.status == ModerationCaseStatusV1::Open
                && observation.case.challenge_count == 1
                && observation.case.pending_challenge_count == 0
                && observation.case.accepted_challenge_count == 0
                && observation.alice_challenge.decision
                    == Some(ModerationChallengeDecisionV1::Rejected)
                && observation.alice_challenge.bond.refunded_amount == Quantity::from(113_u32)
                && observation.alice_challenge.bond.slashed_amount == Quantity::from(37_u32)
                && observation
                    .alice_challenge
                    .bond
                    .settled_at_unix_ms
                    .is_some()
                && observation.balances
                    == [
                        Quantity::from(963_u32),
                        Quantity::from(1_000_u32),
                        Quantity::zero(),
                        Quantity::from(37_u32),
                    ]
                && exactly_conserves_voting_asset(observation)
        },
    )
    .await?;

    carpenter_client(&network.peers()[0]).submit_blocking(
        RaiseSorafsModerationChallenge::new(
            CASE_ID.to_owned(),
            ROUND_ID.to_owned(),
            CARPENTER_CHALLENGE_ID.to_owned(),
            ModerationChallengeKindV1::Other,
            None,
            [0x82; 32],
            "Carpenter requests governance review".to_owned(),
        ),
        no_fee(),
    )?;
    wait_for_settlement_convergence(
        &network,
        &voting_asset_id,
        "Carpenter pending bond",
        true,
        |observation| {
            observation.case.status == ModerationCaseStatusV1::Open
                && observation.case.challenge_count == 2
                && observation.case.pending_challenge_count == 1
                && observation.case.accepted_challenge_count == 0
                && observation.alice_challenge.decision
                    == Some(ModerationChallengeDecisionV1::Rejected)
                && observation.alice_challenge.bond.refunded_amount == Quantity::from(113_u32)
                && observation.alice_challenge.bond.slashed_amount == Quantity::from(37_u32)
                && observation
                    .carpenter_challenge
                    .as_ref()
                    .is_some_and(|challenge| {
                        challenge.decision.is_none()
                            && challenge.bond.amount == Quantity::from(150_u32)
                            && challenge.bond.refunded_amount == Quantity::zero()
                            && challenge.bond.slashed_amount == Quantity::zero()
                    })
                && observation.balances
                    == [
                        Quantity::from(963_u32),
                        Quantity::from(850_u32),
                        Quantity::from(150_u32),
                        Quantity::from(37_u32),
                    ]
                && exactly_conserves_voting_asset(observation)
        },
    )
    .await?;
    // Expiry still belongs to the core synthetic-time regression: this real network cannot wait
    // out the mandatory 24-hour resolution grace during an integration run.
    bob.submit_blocking(
        ResolveSorafsModerationChallenge::new(
            CASE_ID.to_owned(),
            ROUND_ID.to_owned(),
            CARPENTER_CHALLENGE_ID.to_owned(),
            ModerationChallengeDecisionV1::Accepted,
        ),
        no_fee(),
    )?;
    wait_for_settlement_convergence(
        &network,
        &voting_asset_id,
        "Carpenter accepted settlement",
        true,
        |observation| {
            observation.case.status == ModerationCaseStatusV1::Challenged
                && observation.case.challenge_count == 2
                && observation.case.pending_challenge_count == 0
                && observation.case.accepted_challenge_count == 1
                && observation.alice_challenge.decision
                    == Some(ModerationChallengeDecisionV1::Rejected)
                && observation.alice_challenge.bond.refunded_amount == Quantity::from(113_u32)
                && observation.alice_challenge.bond.slashed_amount == Quantity::from(37_u32)
                && observation
                    .carpenter_challenge
                    .as_ref()
                    .is_some_and(|challenge| {
                        challenge.decision == Some(ModerationChallengeDecisionV1::Accepted)
                            && challenge.bond.refunded_amount == Quantity::from(150_u32)
                            && challenge.bond.slashed_amount == Quantity::zero()
                            && challenge.bond.settled_at_unix_ms.is_some()
                    })
                && observation.balances
                    == [
                        Quantity::from(963_u32),
                        Quantity::from(1_000_u32),
                        Quantity::zero(),
                        Quantity::from(37_u32),
                    ]
                && exactly_conserves_voting_asset(observation)
        },
    )
    .await?;
    Ok(())
}
