//! Bind last-wins permission tests to actual canonical execution on all four voters.
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

use eyre::{Result, ensure, eyre};
use integration_tests::metrics::MetricsReader;
use iroha_data_model::{
    prelude::*,
    query::CommittedTransaction,
    sumeragi_finality::{FinalityValidator, SumeragiFinalityVerifier},
};
use iroha_test_network::Network;
use tokio::runtime::Runtime;

const DETACHED_METRICS: [&str; 3] = [
    "pipeline_detached_prepared",
    "pipeline_detached_merged",
    "pipeline_detached_fallback",
];

fn require_no_detached_activity(snapshot: &str) -> Result<()> {
    let metrics = MetricsReader::new(snapshot);
    for key in DETACHED_METRICS {
        let value = metrics
            .get_optional(key)
            .ok_or_else(|| eyre!("missing metric {key}"))?;
        ensure!(
            value == 0.0,
            "retired detached path reported activity: {key}={value}"
        );
    }
    Ok(())
}

/// The retired detached DAG must allocate no journals, before or after the target
/// transaction. These are gauges, not cumulative counters or execution proof.
pub(crate) fn assert_no_detached_activity(network: &Network, rt: &Runtime) -> Result<()> {
    ensure!(
        network.validators().len() == 4,
        "four real voters are required"
    );
    let http = integration_tests::http::client();
    // Preserve the previous five-second metric observation budget across all
    // peers. Concurrent reads do not give each peer a fresh budget.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let snapshots = rt.block_on(async {
        tokio::time::timeout_at(deadline, async {
            futures_util::future::try_join_all(network.validators().iter().map(|peer| {
                let http = &http;
                async move {
                    let url = peer.client().client().endpoint().join("/metrics")?;
                    let snapshot = http
                        .get(url)
                        .send()
                        .await?
                        .error_for_status()?
                        .text()
                        .await?;
                    Ok::<_, eyre::Report>((peer.id(), snapshot))
                }
            }))
            .await
        })
        .await
    })??;
    ensure!(
        tokio::time::Instant::now() < deadline,
        "metric observation deadline elapsed"
    );
    for (peer, snapshot) in snapshots {
        require_no_detached_activity(&snapshot)
            .map_err(|error| error.wrap_err(format!("metrics from voter {peer}")))?;
    }
    Ok(())
}

fn require_selected_output(
    selected: &CommittedTransaction,
    expected: HashOf<TransactionEntrypoint>,
    authority: &AccountId,
    instructions: &[InstructionBox],
) -> Result<()> {
    let TransactionEntrypoint::External(transaction) = &selected.entrypoint else {
        return Err(eyre!("expected the exact external permission transaction"));
    };
    ensure!(
        selected.entrypoint_hash == expected && transaction.hash_as_entrypoint() == expected,
        "permission evidence substituted the submitted transaction"
    );
    ensure!(
        transaction.authority() == authority,
        "permission evidence changed the signer"
    );
    let Executable::Instructions(actual) = transaction.instructions() else {
        return Err(eyre!("permission transaction is not an instruction batch"));
    };
    ensure!(
        actual.iter().eq(instructions.iter()),
        "permission instruction order or payload changed"
    );
    let output = selected
        .output
        .network()
        .ok_or_else(|| eyre!("permission output is not Network"))?;
    ensure!(
        output.input_index == selected.entrypoint_proof.leaf_index()
            && output.input_index == selected.output_proof.leaf_index()
            && output.result.0.is_ok(),
        "permission output is rejected or belongs to another input"
    );
    Ok(())
}

/// Verify a complete bounded native prefix from independently held signed
/// genesis, then join the exact submitted input/output to that certified block.
/// The Explorer Merkle paths alone are never treated as finality authority.
pub(crate) fn assert_canonical_output(
    network: &Network,
    submitted: HashOf<SignedTransaction>,
    authority: &AccountId,
    instructions: &[InstructionBox],
) -> Result<()> {
    ensure!(
        network.validators().len() == 4,
        "four real voters are required"
    );
    let genesis = network.genesis().0;
    let authenticated = iroha_data_model::sumeragi_finality::authenticated_genesis(&genesis)?;
    let (epoch, scope) = authenticated.into_parts();
    ensure!(
        epoch.committee.len() == 4
            && epoch.committee.iter().all(|member| network
                .validators()
                .iter()
                .any(|peer| peer.id() == member.validator)),
        "signed genesis must bind the exact four fixture voters"
    );
    let validators = epoch
        .committee
        .iter()
        .map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession.clone(),
        })
        .collect::<Vec<_>>();
    // SignedTransaction::hash and hash_as_entrypoint share the canonical intent
    // identity; only their type markers differ (transaction/signed.rs).
    let expected = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(submitted.into());
    let deadline = Instant::now() + network.sync_timeout();
    let mut decision = None;
    let mut target_height = None;
    for peer in network.validators() {
        ensure!(
            Instant::now() < deadline,
            "permission proof deadline elapsed"
        );
        let client = peer.client().with_request_deadline(deadline)?;
        if let Some(height) = target_height {
            wait_for_applied_height(&client, height, deadline)?;
        }
        let selected = client.client().get_explorer_transaction_proof(expected)?;
        if let Some(height) = target_height {
            ensure!(
                selected.block_height == height,
                "voters located the transaction in different blocks"
            );
        } else {
            target_height = Some(selected.block_height);
        }
        require_selected_output(&selected.transaction, expected, authority, instructions)?;
        ensure!(
            (2..=16).contains(&selected.block_height),
            "permission fixture exceeded its prefix bound"
        );
        let mut verifier = SumeragiFinalityVerifier::new(
            &genesis,
            network.chain_id().as_ref(),
            validators.clone(),
        )?;
        for height in 1..=selected.block_height {
            ensure!(
                Instant::now() < deadline,
                "permission proof deadline elapsed"
            );
            let proof = client
                .client()
                .get_sumeragi_finality_proof(NonZeroU64::new(height).unwrap(), scope)?;
            let verified = verifier.verify(&proof)?;
            if height == selected.block_height {
                verified.verify_global_scope(network.network_id(), network.chain_id().as_ref())?;
                verified
                    .verify_committed_transaction(&network.network_id(), &selected.transaction)?;
                ensure!(
                    verified.header() == selected.block_header,
                    "selective evidence changed the canonical header"
                );
                let block = verified.block();
                ensure!(
                    block
                        .network_entrypoints()
                        .filter(|entrypoint| entrypoint.hash() == expected)
                        .count()
                        == 1,
                    "submitted permission input must occur exactly once in its certified block"
                );
                let input = selected.transaction.output.network().unwrap().input_index;
                ensure!(
                    block
                        .execution_outputs()
                        .iter()
                        .filter(|output| output
                            .network()
                            .is_some_and(|output| output.input_index == input))
                        .count()
                        == 1,
                    "permission input must own exactly one canonical Network output"
                );
                let current = (verified.header().hash(), verified.context_id());
                if let Some(previous) = decision {
                    ensure!(
                        previous == current,
                        "voters certified different permission results"
                    );
                }
                decision = Some(current);
            }
        }
        ensure!(
            Instant::now() < deadline,
            "permission proof deadline elapsed"
        );
    }
    Ok(())
}

fn wait_for_applied_height(
    client: &iroha::blocking::Client,
    target: u64,
    deadline: Instant,
) -> Result<()> {
    let mut last_busy = None;
    loop {
        ensure!(
            Instant::now() < deadline,
            "permission State publication deadline: {last_busy:?}"
        );
        let mut delay = Duration::from_millis(100);
        let reached = match client.status().get() {
            Ok(status) => status.blocks >= target,
            Err(error) => {
                let iroha::Error::StatusUnavailable {
                    reason: Some(iroha::StatusFailureReason::StateBusy),
                    retry_after,
                } = &error
                else {
                    return Err(error.into());
                };
                delay = delay.max(retry_after.unwrap_or_default());
                last_busy = Some(error);
                false
            }
        };
        ensure!(
            Instant::now() < deadline,
            "permission State publication deadline: {last_busy:?}"
        );
        if reached {
            return Ok(());
        }
        std::thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(delay),
        );
    }
}

#[test]
fn missing_or_nonzero_detached_metrics_are_not_zero_activity_evidence() -> Result<()> {
    require_no_detached_activity(
        "pipeline_detached_prepared 0\npipeline_detached_merged 0\npipeline_detached_fallback 0\n",
    )?;
    for snapshot in [
        "",
        "pipeline_detached_prepared 0\n",
        "pipeline_detached_prepared 1\npipeline_detached_merged 0\npipeline_detached_fallback 0\n",
        "pipeline_detached_prepared 0\npipeline_detached_merged 0\npipeline_detached_fallback NaN\n",
    ] {
        assert!(require_no_detached_activity(snapshot).is_err());
    }
    Ok(())
}

#[test]
fn selected_permission_output_requires_exact_hash_signer_order_and_success() -> Result<()> {
    use iroha_crypto::{Hash, MerkleProof};
    use iroha_data_model::{
        block::execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1},
        transaction::TransactionResult,
    };
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"pure permission binding",
    )));
    let permission = Permission::new(
        "CanModifyAccountMetadata".to_owned(),
        iroha_primitives::json::Json::new(norito::json!({"account": (ALICE_ID.clone())})),
    );
    let instructions: Vec<InstructionBox> = vec![
        Revoke::account_permission(permission.clone(), BOB_ID.clone()).into(),
        Grant::account_permission(permission, BOB_ID.clone()).into(),
    ];
    let transaction = TransactionBuilder::new(
        network,
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions.clone())
    .sign(ALICE_KEYPAIR.private_key());
    let expected = transaction.hash_as_entrypoint();
    let output = ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
        input_index: 0,
        result: TransactionResult::new(Ok(vec![])),
        completions: vec![],
    });
    let mut selected = CommittedTransaction {
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"binding-only header")),
        entrypoint_hash: expected,
        entrypoint: TransactionEntrypoint::External(transaction),
        entrypoint_proof: MerkleProof::from_audit_path(0, vec![]),
        output_hash: HashOf::new(&output),
        output,
        output_proof: MerkleProof::from_audit_path(0, vec![]),
    };
    require_selected_output(&selected, expected, &ALICE_ID, &instructions)?;
    assert!(
        require_selected_output(
            &selected,
            HashOf::from_untyped_unchecked(Hash::new(b"wrong intent")),
            &ALICE_ID,
            &instructions
        )
        .is_err()
    );
    assert!(require_selected_output(&selected, expected, &BOB_ID, &instructions).is_err());
    let mut reversed = instructions.clone();
    reversed.reverse();
    assert!(require_selected_output(&selected, expected, &ALICE_ID, &reversed).is_err());
    selected.output = ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
        input_index: 1,
        result: TransactionResult::new(Ok(vec![])),
        completions: vec![],
    });
    assert!(require_selected_output(&selected, expected, &ALICE_ID, &instructions).is_err());
    selected.output = ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
        input_index: 0,
        result: TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted("denied".to_owned()),
        ))),
        completions: vec![],
    });
    assert!(require_selected_output(&selected, expected, &ALICE_ID, &instructions).is_err());
    Ok(())
}
