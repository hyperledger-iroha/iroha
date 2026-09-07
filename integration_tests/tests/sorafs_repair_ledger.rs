//! Four-validator repair authority, competing claims, revocation and restart checks.
//!
//! These exercise native ledger transitions. Provider storage execution and
//! production evidence collection remain separate qualification requirements.

use std::time::Duration;

use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    blocking::Client,
    crypto::{HashOf, KeyPair},
    data_model::{
        events::data::sorafs::SorafsRepairLedgerEventKind,
        isi::{
            error::{InstructionExecutionError, InvalidParameterError},
            sorafs::{
                ApplySorafsRepairTaskAction, SorafsRepairClaimV1, SorafsRepairCompleteV1,
                SorafsRepairFailV1, SorafsRepairTaskActionV1, SubmitSorafsRepairTask,
            },
        },
        prelude::*,
        query::sorafs::prelude::{
            FindSorafsRepairEvents, FindSorafsRepairStatus, FindSorafsRepairTask,
        },
        sorafs::{
            capacity::ProviderId,
            moderation_ledger::{
                REPAIR_LEDGER_MAX_LEASE_MS_V1, RepairFinalizedEventPageV1, RepairFinalizedStatusV1,
                RepairFinalizedTaskV1, RepairLedgerTerminalKindV1,
            },
        },
        transaction::{FeePaymentIntent, error::TransactionRejectionReason},
    },
};
use iroha_executor_data_model::permission::{
    query::CanReadAllLedgerData, sorafs::CanOperateSorafsRepair,
};
use iroha_test_network::read_on_dedicated_thread;
use iroha_test_network::{Network, NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
use sorafs_manifest::repair::{
    REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, RepairCauseV1, RepairEvidenceV1,
    RepairManualCauseV1, RepairReportV1, RepairTicketId,
};
use tokio::time::{Instant, sleep, timeout};

const TICKET: &str = "REP-FOUR-PEER-1";
const PROVIDER: [u8; 32] = [0xD1; 32];
const SOURCE: [u8; 32] = [0xD2; 32];
const EVIDENCE: [u8; 32] = [0xD3; 32];
const DEADLINE: Duration = Duration::from_secs(180);

fn no_fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(Vec::new(), None)
}

async fn submit_instructions(
    client: &Client,
    instructions: impl IntoIterator<Item = impl Into<InstructionBox>>,
) -> Result<HashOf<SignedTransaction>> {
    let account = client.account_client();
    let mut payload = account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
        instructions,
        no_fee(),
        Metadata::default(),
    ))?;
    let quote = account
        .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
    ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "fee quote changed the selected payer, sponsor revision, or gas bound"
    );
    payload.fee_payment = quote.intent;
    let transaction = account.sign_transaction(payload)?;
    account.submit_transaction_and_wait(&transaction).await
}

fn client(network: &Network, peer: usize, account: &AccountId, keys: &KeyPair) -> Client {
    let client = network.peers()[peer].client_for(account, keys.private_key().clone());
    integration_tests::sync::rebind_blocking_client(&client, |client| {
        client.transaction_status_timeout = DEADLINE;
        client.torii_request_timeout = Duration::from_secs(10);
        client.transaction_ttl = Some(Duration::from_secs(300));
        client.add_transaction_nonce = false;
    })
}

fn report(ticket: &str) -> Result<Vec<u8>> {
    Ok(norito::to_bytes(&RepairReportV1 {
        version: REPAIR_REPORT_VERSION_V1,
        ticket_id: RepairTicketId(ticket.to_owned()),
        auditor_account: ALICE_ID.to_string(),
        // A fixed past timestamp removes clock-dependent report eligibility.
        submitted_at_unix: 1,
        evidence: RepairEvidenceV1 {
            version: REPAIR_EVIDENCE_VERSION_V1,
            manifest_digest: [0xD4; 32],
            provider_id: PROVIDER,
            por_history_id: None,
            cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                reason: "four-validator native repair authority".to_owned(),
            }),
            evidence_json: None,
            notes: None,
        },
        notes: None,
    })?)
}

fn claim(revision: u64, key: &str) -> InstructionBox {
    ApplySorafsRepairTaskAction::new(
        TICKET.to_owned(),
        revision,
        SorafsRepairTaskActionV1::Claim(SorafsRepairClaimV1 {
            // Reclaim below is caused by revocation, never a timing race.
            lease_duration_ms: REPAIR_LEDGER_MAX_LEASE_MS_V1,
            idempotency_key: key.to_owned(),
        }),
    )
    .into()
}

fn complete(revision: u64, generation: u64, key: &str) -> InstructionBox {
    ApplySorafsRepairTaskAction::new(
        TICKET.to_owned(),
        revision,
        SorafsRepairTaskActionV1::Complete(SorafsRepairCompleteV1 {
            lease_generation: generation,
            evidence_digest: EVIDENCE,
            idempotency_key: key.to_owned(),
        }),
    )
    .into()
}

async fn race(
    left: Client,
    right: Client,
    instructions: [InstructionBox; 2],
) -> Result<[Result<HashOf<SignedTransaction>>; 2]> {
    let [left_instruction, right_instruction] = instructions;
    let mut left_metadata = Metadata::default();
    left_metadata.insert("repair_race_route".parse()?, 0u32);
    let mut right_metadata = Metadata::default();
    right_metadata.insert("repair_race_route".parse()?, 1u32);
    let left_transaction = {
        let account = left.account_client();
        account
            .prepare_transaction(iroha::client::AccountTransactionDraft::new(
                [left_instruction],
                no_fee(),
                left_metadata,
            ))
            .and_then(|payload| account.sign_transaction(payload))
    }?;
    let right_transaction = {
        let account = right.account_client();
        account
            .prepare_transaction(iroha::client::AccountTransactionDraft::new(
                [right_instruction],
                no_fee(),
                right_metadata,
            ))
            .and_then(|payload| account.sign_transaction(payload))
    }?;
    ensure!(
        left_transaction.hash() != right_transaction.hash(),
        "distinct transactions must exercise instruction replay, not transaction deduplication"
    );
    let barrier = tokio::sync::Barrier::new(2);
    let (left, right) = tokio::join!(
        async {
            barrier.wait().await;
            left.account_client()
                .submit_transaction_and_wait(&left_transaction)
                .await
        },
        async {
            barrier.wait().await;
            right
                .account_client()
                .submit_transaction_and_wait(&right_transaction)
                .await
        },
    );
    Ok([left, right])
}

fn require_validation_rejection(
    result: Result<HashOf<SignedTransaction>>,
    marker: &str,
) -> Result<()> {
    let error = result.expect_err("invalid repair transition must be rejected");
    let reason = error.downcast_ref::<TransactionRejectionReason>();
    let Some(TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message)),
    ))) = reason
    else {
        return Err(eyre!(
            "transport, timeout, or unrelated validation failure cannot establish repair rejection: {error:?}"
        ));
    };
    ensure!(
        message.to_ascii_lowercase().contains(marker),
        "unrelated validation failure cannot establish rejection of {marker}: {error:?}"
    );
    Ok(())
}

#[test]
fn repair_rejection_requires_the_exact_native_error() {
    let rejection = TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            "repair task revision mismatch: expected 1, found 2".to_owned(),
        )),
    ));
    require_validation_rejection(Err(eyre!(rejection.clone())), "revision mismatch")
        .expect("native rejection detail must be read below the outer validation error");
    assert!(require_validation_rejection(Err(eyre!(rejection)), "permission").is_err());
    assert!(
        require_validation_rejection(
            Err(eyre!("transport revision mismatch")),
            "revision mismatch"
        )
        .is_err()
    );
    assert!(
        require_validation_rejection(
            Err(eyre!(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted("revision mismatch".to_owned())
            ))),
            "revision mismatch",
        )
        .is_err()
    );
}

type Observation = (
    RepairFinalizedTaskV1,
    RepairFinalizedStatusV1,
    RepairFinalizedEventPageV1,
);

async fn converged(network: &Network, revision: u64, event_count: usize) -> Result<Observation> {
    let deadline = Instant::now() + DEADLINE;
    loop {
        let mut observations = Vec::new();
        for peer in 0..4 {
            let reader = client(network, peer, &BOB_ID, &BOB_KEYPAIR);
            let observation = read_on_dedicated_thread(move || -> Result<Observation> {
                let task = reader
                    .client()
                    .query_single(FindSorafsRepairTask::new(TICKET.to_owned(), None))?;
                let anchor = Some(task.finalized_cursor);
                Ok((
                    task,
                    reader
                        .client()
                        .query_single(FindSorafsRepairStatus::new(anchor))?,
                    reader
                        .client()
                        .query_single(FindSorafsRepairEvents::new(anchor, None, 16))?,
                ))
            })
            .await;
            if let Ok(observation) = observation {
                observations.push(observation);
            }
        }
        if observations.len() == 4
            && observations.iter().all(|(task, status, events)| {
                task.task.revision == revision
                    && status.status.tasks == 1
                    && events.events.len() == event_count
                    && !events.has_more
            })
        {
            let bytes = observations
                .iter()
                .map(norito::to_bytes)
                .collect::<Result<Vec<_>, _>>()?;
            if bytes.windows(2).all(|pair| pair[0] == pair[1]) {
                return Ok(observations.remove(0));
            }
        }
        ensure!(
            Instant::now() < deadline,
            "repair projections did not converge at revision {revision}"
        );
        sleep(Duration::from_millis(250)).await;
    }
}

#[test]
fn four_peer_repair_claim_revocation_terminal_and_restart_are_authoritative() -> Result<()> {
    super::sorafs_network::run(
        stringify!(four_peer_repair_claim_revocation_terminal_and_restart_are_authoritative),
        four_peer_repair_claim_revocation_terminal_and_restart_are_authoritative_impl,
    )
}

async fn four_peer_repair_claim_revocation_terminal_and_restart_are_authoritative_impl()
-> Result<()> {
    init_instruction_registry();
    let permission = Permission::from(CanOperateSorafsRepair {
        provider_id: ProviderId::new(PROVIDER),
    });
    let mut builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_secs(1))
        .with_npos_consensus();
    for account in [ALICE_ID.clone(), BOB_ID.clone()] {
        builder = builder
            .with_genesis_instruction(Grant::account_permission(permission.clone(), account));
    }
    // The canonical genesis already grants Alice read-all authority.
    builder = builder.with_genesis_instruction(Grant::account_permission(
        Permission::from(CanReadAllLedgerData),
        BOB_ID.clone(),
    ));
    let context =
        stringify!(four_peer_repair_claim_revocation_terminal_and_restart_are_authoritative);
    let builder = super::sorafs_network::bounded_storage(builder);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    ensure!(
        network.peers().len() == 4,
        "repair qualification requires four voters"
    );

    let submission: InstructionBox = SubmitSorafsRepairTask::new(SOURCE, report(TICKET)?).into();
    for result in race(
        client(&network, 0, &ALICE_ID, &ALICE_KEYPAIR),
        client(&network, 1, &ALICE_ID, &ALICE_KEYPAIR),
        [submission.clone(), submission],
    )
    .await?
    {
        result.wrap_err("same-source concurrent submission must be idempotent")?;
    }
    converged(&network, 1, 1).await?;
    require_validation_rejection(
        submit_instructions(
            &client(&network, 2, &ALICE_ID, &ALICE_KEYPAIR),
            [SubmitSorafsRepairTask::new(SOURCE, report("REP-CONFLICT")?)],
        )
        .await,
        "source identity",
    )?;

    let outcomes = race(
        client(&network, 0, &ALICE_ID, &ALICE_KEYPAIR),
        client(&network, 1, &BOB_ID, &BOB_KEYPAIR),
        [claim(1, "claim-alice"), claim(1, "claim-bob")],
    )
    .await?;
    ensure!(
        outcomes.iter().filter(|result| result.is_ok()).count() == 1,
        "exactly one simultaneous claim must commit"
    );
    for outcome in outcomes {
        if outcome.is_err() {
            require_validation_rejection(outcome, "revision mismatch")?;
        }
    }
    let (claimed, _, _) = converged(&network, 2, 2).await?;
    let lease = claimed
        .task
        .lease
        .as_ref()
        .ok_or_else(|| eyre!("claim omitted lease"))?;
    let ((winner, winner_keys), (successor, successor_keys)) = if lease.owner == *ALICE_ID {
        ((&*ALICE_ID, &*ALICE_KEYPAIR), (&*BOB_ID, &*BOB_KEYPAIR))
    } else {
        ensure!(lease.owner == *BOB_ID, "unexpected repair owner");
        ((&*BOB_ID, &*BOB_KEYPAIR), (&*ALICE_ID, &*ALICE_KEYPAIR))
    };
    require_validation_rejection(
        submit_instructions(
            &client(&network, 2, successor, successor_keys),
            [claim(2, "unexpired-claim")],
        )
        .await,
        "lease is held",
    )?;

    submit_instructions(
        &client(&network, 0, winner, winner_keys),
        [Revoke::account_permission(permission, winner.clone())],
    )
    .await?;
    submit_instructions(
        &client(&network, 1, successor, successor_keys),
        [claim(2, "revoked-owner-reclaim")],
    )
    .await?;
    let (reclaimed, _, _) = converged(&network, 3, 3).await?;
    let next_lease = reclaimed
        .task
        .lease
        .as_ref()
        .ok_or_else(|| eyre!("reclaim omitted lease"))?;
    ensure!(
        next_lease.owner == *successor && next_lease.generation == 2,
        "revocation must fence the old owner and advance generation"
    );
    ensure!(
        next_lease.acquired_at_unix_ms < lease.expires_at_unix_ms,
        "this test must exercise revocation before lease expiry"
    );
    require_validation_rejection(
        submit_instructions(
            &client(&network, 2, winner, winner_keys),
            [complete(3, 1, "revoked-owner-complete")],
        )
        .await,
        "permission",
    )?;

    let terminal = complete(3, 2, "successor-complete");
    for result in race(
        client(&network, 0, successor, successor_keys),
        client(&network, 3, successor, successor_keys),
        [terminal.clone(), terminal],
    )
    .await?
    {
        result.wrap_err("exact completion replay must reconcile without another outcome")?;
    }
    let (finished, status, events) = converged(&network, 4, 4).await?;
    ensure!(
        finished.task.source_identity == SOURCE
            && finished.task.canonical_report == report(TICKET)?
            && finished.task.submitted_by == *ALICE_ID,
        "repair source binding or canonical report changed during claims"
    );
    let terminal = finished
        .task
        .terminal_outcome
        .as_ref()
        .ok_or_else(|| eyre!("missing terminal"))?;
    ensure!(
        terminal.finalized_by == *successor && terminal.lease_generation == 2,
        "terminal authority differs from the finalized successor lease"
    );
    ensure!(
        matches!(&terminal.kind, RepairLedgerTerminalKindV1::Completed(result) if result.evidence_digest == EVIDENCE),
        "completion evidence was substituted"
    );
    ensure!(
        status.status.terminal_outcomes == 1
            && status.status.completed == 1
            && status.status.leased_tasks == 0,
        "replays changed authoritative repair counters"
    );
    ensure!(
        events
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>()
            == vec![1, 2, 3, 4],
        "repair journal contains duplicate or missing transitions"
    );
    ensure!(
        events
            .events
            .iter()
            .map(|event| event.event.kind)
            .collect::<Vec<_>>()
            == vec![
                SorafsRepairLedgerEventKind::TaskSubmitted,
                SorafsRepairLedgerEventKind::LeaseClaimed,
                SorafsRepairLedgerEventKind::LeaseClaimed,
                SorafsRepairLedgerEventKind::Completed,
            ],
        "repair journal emitted the wrong committed transition kinds"
    );
    require_validation_rejection(
        submit_instructions(
            &client(&network, 2, successor, successor_keys),
            [ApplySorafsRepairTaskAction::new(
                TICKET.to_owned(),
                4,
                SorafsRepairTaskActionV1::Fail(SorafsRepairFailV1 {
                    lease_generation: 2,
                    failure_digest: [0xE1; 32],
                    idempotency_key: "second-terminal".to_owned(),
                }),
            )],
        )
        .await,
        "terminal outcome",
    )?;
    let before_restart = converged(&network, 4, 4).await?;
    let peer = network.peers()[3].clone();
    let config = network.config_layers().collect::<Vec<_>>();
    ensure!(
        peer.shutdown_if_started().await,
        "restart peer was not running"
    );
    timeout(
        network.peer_startup_timeout(),
        peer.start_checked(config.iter(), None),
    )
    .await??;
    timeout(
        network.sync_timeout(),
        peer.once_block(before_restart.0.finalized_cursor.height),
    )
    .await?;
    let after_restart = converged(&network, 4, 4).await?;
    ensure!(
        norito::to_bytes(&before_restart)? == norito::to_bytes(&after_restart)?,
        "cold restart changed canonical repair task, counters or event bytes"
    );
    Ok(())
}
