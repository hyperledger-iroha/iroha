//! Four-validator regression for consensus-pinned SoraFS moderation sortition.

use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    crypto::{KeyPair, Signature},
    data_model::{
        isi::sorafs::{
            CommitSorafsPopCredentialBatch, FinalizeSorafsModerationSortition,
            SetSorafsModerationPolicy, SetSorafsPopIssuerPolicy, SubmitSorafsModerationAppeal,
        },
        prelude::*,
        query::{block::prelude::FindBlocks, sorafs::prelude::FindSorafsModerationAppeal},
        sorafs::{
            moderation_ledger::{
                MODERATION_APPEAL_INTAKE_VERSION_V1, MODERATION_CHALLENGE_BOND_AMOUNT_V1,
                MODERATION_CHALLENGE_REJECTED_SLASH_BPS_V1,
                MODERATION_CHALLENGE_RESOLUTION_GRACE_MS_V1, MODERATION_LEDGER_POLICY_VERSION_V1,
                ModerationAppealIntakeV1, ModerationAppealRecordV1, ModerationAppealStatusV1,
                ModerationLedgerPolicyV1, ModerationSortitionAnchorV1,
            },
            pop_registry::{
                POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1, POP_ISSUER_POLICY_VERSION_V1,
                PopCredentialCommitmentBatchV1, PopCredentialCommitmentV1, PopIssuerPolicyV1,
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
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
use sorafs_manifest::pop_credentials::{
    POP_COMMITMENT_ROOT_VERSION_V1, POP_CREDENTIAL_TREE_DEPTH_V1, POP_REVOCATION_LIST_VERSION_V1,
    POP_REVOCATION_TREE_DEPTH_V1, PopCommitmentRootV1, PopRevocationListV1,
    PopSignatureAlgorithmV1, PopSignatureV1, pop_commitment_root_signature_digest_v1,
    pop_revocation_list_signature_digest_v1, pop_revocation_root_v1,
    verify_pop_commitment_root_signature_v1, verify_pop_revocation_list_signature_v1,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::{Instant, sleep};

const CASE_ID: &str = "four-peer-anchor";
const ROUND_ID: &str = "round-1";
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

fn pop_policy(issuer: &KeyPair) -> PopIssuerPolicyV1 {
    PopIssuerPolicyV1 {
        version: POP_ISSUER_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        issuer_id: "four-peer-pop-issuer".to_owned(),
        issuer_account: BOB_ID.clone(),
        issuer_public_key: public_key_bytes(issuer),
        max_credentials_per_batch: 4,
        max_revocations_per_publication: 4,
        max_credential_lifetime_secs: 7_200,
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
            issuer_id: "four-peer-pop-issuer".to_owned(),
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
            issuer_id: "four-peer-pop-issuer".to_owned(),
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
