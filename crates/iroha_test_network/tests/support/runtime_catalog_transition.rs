//! A committed catalog expansion must survive full Kura replay before signed-snapshot recovery.
//! All four peers retain their startup lane configuration, keys, genesis and block history.
//! Snapshot writes begin only after the retained replay, before geometry compaction is permitted.
use super::*;

#[path = "catalog_recovery.rs"]
pub(super) mod real_custody;
use iroha_core::{
    lane_consensus::{validate_lane_block_proposal, validate_lane_block_qc_aggregate},
    merge::{merge_application_header_from_carrier, merge_execution_batch_commitments_match},
    merge_sidecar::decode_certified_merge_sidecar,
    queue::{RoutingDecision, RoutingPlan},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockHeader,
        consensus::CertPhase,
        consensus_v2::{ConsensusMode, ExecutionCommitment, ValidatorPower},
        decode_framed_signed_block,
        execution_context::CertifiedMergeLedgerReference,
    },
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    isi::{Grant, Register, Revoke, SetParameter},
    merge::{MAX_MERGE_LEDGER_ENTRY_BYTES, MergeLedgerEntry},
    nexus::{
        DataSpaceCatalog, DataSpaceMetadata, LaneConfig, LaneLifecycleStatusV1, LaneVisibility,
        NexusCatalogTransitionV1, NexusRuntimeCatalogV1, RuntimeDataSpaceAdditionV1,
        RuntimeLaneManifestV1, dataspace_catalog_hash,
    },
    parameter::Parameter,
    permission::Permission,
    role::Role,
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};
use iroha_executor_data_model::permission::account::{
    AccountAliasPermissionScope, CanDelegateAccountAliasResolution, CanResolveAccountAlias,
};
use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use iroha_primitives::json::Json;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use norito::codec::{DecodeAll, Encode};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    num::NonZeroU64,
    sync::{Arc, Mutex},
};

const ADDED_LANE: LaneId = LaneId::new(3);
const ADDED_DATASPACE: DataSpaceId = DataSpaceId::new(3);

#[derive(Clone)]
struct AppliedEvidence {
    transaction: SignedTransaction,
    height: u64,
    lane: LaneId,
    dataspace: DataSpaceId,
    canonical_block: Vec<u8>,
}

#[derive(Clone)]
struct FixtureFinality {
    network_id: NetworkId,
    genesis_hash: HashOf<BlockHeader>,
    roster: Vec<ValidatorPower>,
    validator_pops: Vec<Vec<u8>>,
    peers: BTreeMap<PeerId, Arc<Mutex<VerifiedPeerFinality>>>,
}

struct VerifiedPeerFinality {
    network_id: NetworkId,
    genesis_hash: HashOf<BlockHeader>,
    peer: PeerId,
    verifier: Option<BridgeFinalityVerifier>,
    proofs: BTreeMap<u64, BridgeFinalityProof>,
}

impl FixtureFinality {
    fn from_network(network: &Network) -> Result<Self> {
        ensure!(
            network.peers().len() == 4,
            "finality trust requires all four fixture validators"
        );
        let network_id = network.network_id();
        let genesis_hash = network.genesis().0.hash();
        let mut validators = BTreeMap::new();
        let mut peers = BTreeMap::new();
        for peer in network.peers() {
            let key = peer
                .bls_public_key()
                .ok_or_else(|| eyre!("fixture validator has no BLS identity"))?;
            let pop = peer
                .bls_pop()
                .ok_or_else(|| eyre!("fixture validator has no public BLS proof of possession"))?;
            ensure!(
                !pop.is_empty()
                    && validators
                        .insert(PeerId::new(key.clone()), pop.to_vec())
                        .is_none(),
                "fixture validator identities must be distinct and carry public PoPs"
            );
            let identity = peer.network_peer_id();
            ensure!(
                peers
                    .insert(
                        identity.clone(),
                        Arc::new(Mutex::new(VerifiedPeerFinality {
                            network_id,
                            genesis_hash,
                            peer: identity,
                            verifier: None,
                            proofs: BTreeMap::new(),
                        }))
                    )
                    .is_none(),
                "fixture transport identities must be distinct"
            );
        }
        // Native protocol v4 gives every validator one vote and sorts by PeerId. These keys
        // and PoPs come from the generated fixture, never from a fetched finality document.
        let (roster, validator_pops) = validators
            .into_iter()
            .map(|(validator, pop)| {
                (
                    ValidatorPower {
                        validator,
                        power: 1,
                    },
                    pop,
                )
            })
            .unzip();
        Ok(Self {
            network_id,
            genesis_hash,
            roster,
            validator_pops,
            peers,
        })
    }

    fn validate_fixture_roster(&self, proof: &BridgeFinalityProof) -> Result<()> {
        let artifact = &proof.finality_artifact;
        ensure!(
            artifact.height_context.network_id == self.network_id
                && artifact.height_context.mode == ConsensusMode::Npos
                && artifact.height_context.roster == self.roster
                && artifact.validator_set_pops == self.validator_pops
                && artifact.height_context.snapshot_bootstrap.is_none(),
            "finality proof differs from the exact generated fixture validator authority"
        );
        Ok(())
    }

    fn execution_commitment(
        &self,
        peer: &PeerId,
        client: &iroha::client::Client,
        height: u64,
        expected_block_hash: HashOf<BlockHeader>,
    ) -> Result<ExecutionCommitment> {
        ensure!(
            (1..=128).contains(&height),
            "catalog fixture exceeded its bounded finality history"
        );
        let cache = self
            .peers
            .get(peer)
            .ok_or_else(|| eyre!("finality reader is not a fixture peer"))?;
        // This synchronous lock is held only on the peer's dedicated blocking reader. Other
        // peers have independent caches, and no asynchronous network work holds this lock.
        let mut cache = cache
            .lock()
            .map_err(|_| eyre!("fixture finality cache was poisoned"))?;
        ensure!(
            cache.network_id == self.network_id
                && cache.genesis_hash == self.genesis_hash
                && cache.peer == *peer,
            "finality cache belongs to another network, genesis or peer"
        );
        if cache.verifier.is_none() {
            ensure!(
                cache.proofs.is_empty(),
                "unanchored cache contains finality evidence"
            );
            let (proof, hash) = client.get_bridge_finality_anchor(
                NonZeroU64::new(1).expect("genesis height"),
                self.network_id,
            )?;
            ensure!(
                hash == self.genesis_hash && proof.block_header.hash() == self.genesis_hash,
                "finality anchor is not the fixture's exact signed genesis"
            );
            self.validate_fixture_roster(&proof)?;
            // Only the externally known genesis hash and complete fixture roster/PoPs can
            // authorize this context. A self-consistent proof-controlled roster is insufficient.
            let mut verifier = BridgeFinalityVerifier::with_context(
                self.network_id,
                proof.finality_artifact.context_id(),
            );
            verifier.verify(&proof)?;
            cache.proofs.insert(1, proof);
            cache.verifier = Some(verifier);
        }
        while cache
            .proofs
            .last_key_value()
            .map_or(0, |(height, _)| *height)
            < height
        {
            let next = cache
                .proofs
                .last_key_value()
                .expect("anchored proof cache")
                .0
                + 1;
            let mut verifier = cache.verifier.clone().expect("anchored native verifier");
            let proof = client.get_next_bridge_finality_proof(
                NonZeroU64::new(next).expect("successor height"),
                &mut verifier,
            )?;
            self.validate_fixture_roster(&proof)?;
            cache.proofs.insert(next, proof);
            cache.verifier = Some(verifier);
        }
        let proof = cache
            .proofs
            .get(&height)
            .ok_or_else(|| eyre!("verified finality history has a gap"))?;
        ensure!(
            proof.block_header.hash() == expected_block_hash
                && proof.finality_artifact.block_hash == expected_block_hash,
            "transaction carrier differs from the independently verified finality chain"
        );
        Ok(proof.finality_artifact.commit_qc.execution_commitment)
    }
}

fn resolution_permission() -> Permission {
    CanResolveAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(ADDED_DATASPACE),
    }
    .into()
}

fn resolution_delegation_permission() -> Permission {
    CanDelegateAccountAliasResolution {
        scope: AccountAliasPermissionScope::Dataspace(ADDED_DATASPACE),
    }
    .into()
}

fn resolution_delegation_role() -> InstructionBox {
    Register::role(
        Role::new(
            "runtime_catalog_resolution_delegate"
                .parse()
                .expect("role id"),
            ALICE_ID.clone(),
        )
        .add_permission(resolution_delegation_permission()),
    )
    .into()
}

fn baseline_dataspaces() -> Result<DataSpaceCatalog> {
    // Exact descriptors from the shared multiroute fixture, including their descriptions.
    Ok(DataSpaceCatalog::new(vec![
        DataSpaceMetadata {
            id: DataSpaceId::UNIVERSAL,
            alias: "universal".into(),
            description: Some("default dataspace".into()),
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: DataSpaceId::new(1),
            alias: "ds1".into(),
            description: Some("alice route dataspace".into()),
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: DataSpaceId::new(2),
            alias: "ds2".into(),
            description: Some("bob route dataspace".into()),
            fault_tolerance: 1,
        },
    ])?)
}

async fn lifecycle_and_runtime(
    peer: &NetworkPeer,
) -> Result<(LaneLifecycleStatusV1, Option<NexusRuntimeCatalogV1>)> {
    let client = peer.client().client().clone();
    read_on_dedicated_thread(move || {
        let status = client.get_lane_lifecycle_status()?;
        status.validate()?;
        let parameters = client.get_parameters()?;
        let runtime = parameters
            .custom()
            .get(&NexusRuntimeCatalogV1::parameter_id())
            .map(NexusRuntimeCatalogV1::from_custom_parameter)
            .transpose()?
            .flatten();
        ensure!(
            status.runtime_catalog_hash
                == runtime
                    .as_ref()
                    .map(NexusRuntimeCatalogV1::canonical_hash)
                    .transpose()?,
            "lifecycle runtime catalog hash differs from authenticated overlay"
        );
        Ok((status, runtime))
    })
    .await
}

async fn current_manifest(network: &Network) -> Result<(RuntimeLaneManifestV1, Vec<PeerId>)> {
    let client = network.peers()[0].client().client().clone();
    let roster =
        read_on_dedicated_thread(move || client.get_public_lane_validators(LaneId::new(0))).await?;
    let items = roster
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("live lane-0 roster omitted items"))?;
    ensure!(
        items.len() == 4,
        "manifest must bind exactly the four live validators"
    );
    let mut bindings = Vec::new();
    let mut validators = BTreeSet::new();
    let mut peers = BTreeSet::new();
    for item in items {
        ensure!(
            item.get("status")
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str)
                == Some("Active"),
            "manifest source contains an inactive validator"
        );
        let validator = item
            .get("validator")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("live roster omitted validator account"))?;
        let peer = item
            .get("peer_id")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("live roster omitted consensus peer"))?;
        let parsed_peer: PeerId = peer.parse()?;
        ensure!(
            validators.insert(validator.to_owned()) && peers.insert(parsed_peer),
            "live roster repeats an identity"
        );
        bindings.push(norito::json!({"validator": validator, "peer_id": peer}));
    }
    let expected_peers: BTreeSet<_> = network
        .peers()
        .iter()
        .map(|peer| peer.id().clone())
        .collect();
    ensure!(
        peers == expected_peers,
        "manifest peers differ from the actual running cohort"
    );
    Ok((
        RuntimeLaneManifestV1 {
            lane_id: ADDED_LANE,
            manifest: Json::from_norito_value_ref(&norito::json!({
                "lane": "runtime-committee", "version": 1, "validators": bindings, "quorum": 3,
            }))?,
        },
        peers.into_iter().collect(),
    ))
}

async fn exact_applied_height(network: &Network, transaction: &SignedTransaction) -> Result<u64> {
    let hash = transaction.hash();
    let expected_hex = hash
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let deadline = Instant::now() + FUNCTIONAL_FINALITY_TIMEOUT;
    timeout_at(deadline, async {
        loop {
            let observations = try_join_all(network.peers().iter().map(|peer| async move {
                let client = peer.client().client().clone();
                let status = validator_status_until(&client, deadline).await?;
                let remaining = deadline.saturating_duration_since(Instant::now());
                ensure!(
                    !remaining.is_zero(),
                    "catalog transaction observation exceeded its deadline"
                );
                let mut builder = client.to_builder();
                builder.torii_request_timeout =
                    iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT.min(remaining);
                let client = builder.build()?;
                let local = read_on_dedicated_thread(move || {
                    client.get_transaction_status_response_local(hash)
                })
                .await?;
                Ok::<_, eyre::Report>((status.blocks, local))
            }))
            .await?;
            let heights = observations
                .iter()
                .map(|(tip, response)| {
                    response
                        .as_ref()
                        .filter(|response| {
                            response.hash == expected_hex
                                && response.scope == "local"
                                && response.resolved_from == "state"
                                && response.status.kind == "Applied"
                        })
                        .and_then(|response| response.status.block_height)
                        .filter(|height| *height > 1 && *tip >= *height)
                })
                .collect::<Option<Vec<_>>>();
            if let Some(heights) = heights {
                ensure!(
                    heights.iter().all(|height| *height == heights[0]),
                    "four peers disagree on the applied height"
                );
                return Ok(heights[0]);
            }
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .wrap_err("four-peer catalog transaction did not reach exact local Applied")?
}

// Read only the durable prefix ending at the authenticated carrier's epoch. The daemon
// owns Kura; opening another Kura would run recovery and mutate its live store. The
// outer u32 framing is the maintained MergeLedgerLog format; all entry decoding and
// reference verification below use native codecs. Pending sidecars are unsuitable
// because successful commit/restart may remove them.
fn committed_merge_entry(
    path: &Path,
    reference: &CertifiedMergeLedgerReference,
) -> Result<MergeLedgerEntry> {
    ensure!(
        (1..=128).contains(&reference.epoch_id),
        "catalog fixture exceeded its bounded merge history"
    );
    let mut file = fs::File::open(path).wrap_err("open committed merge log read-only")?;
    ensure!(
        file.metadata()?.is_file(),
        "merge log must be a regular file"
    );
    let mut hashes = BTreeSet::new();
    for epoch in 1..=reference.epoch_id {
        let mut length = [0_u8; 4];
        file.read_exact(&mut length)?;
        let length = usize::try_from(u32::from_le_bytes(length))?;
        ensure!(
            (1..=MAX_MERGE_LEDGER_ENTRY_BYTES).contains(&length),
            "committed merge log frame exceeds native size bounds"
        );
        let mut bytes = vec![0_u8; length];
        file.read_exact(&mut bytes)?;
        let entry = MergeLedgerEntry::decode_all(&mut bytes.as_slice())?;
        ensure!(
            entry.has_current_version()
                && entry.encode() == bytes
                && entry.canonical_size_within_limit()
                && entry.epoch_id == epoch
                && hashes.insert(entry.canonical_hash()),
            "committed merge log prefix is not canonical and contiguous"
        );
        if epoch == reference.epoch_id {
            return Ok(decode_certified_merge_sidecar(
                reference,
                &entry.canonical_bytes(),
            )?);
        }
    }
    Err(eyre!(
        "authenticated merge epoch is absent from the durable log"
    ))
}

async fn canonical_execution(
    finality: &FixtureFinality,
    peer: &NetworkPeer,
    transaction: &SignedTransaction,
    height: u64,
    lane: LaneId,
    dataspace: DataSpaceId,
    expected_committee: Option<&[PeerId]>,
) -> Result<Vec<u8>> {
    let client = peer.client().client().clone();
    let finality = finality.clone();
    let peer_identity = peer.network_peer_id();
    let store = peer.kura_store_dir();
    let transaction = transaction.clone();
    let expected_committee = expected_committee.map(<[PeerId]>::to_vec);
    read_on_dedicated_thread(move || {
        let details =
            client.get_successful_transaction_details(transaction.hash_as_entrypoint())?;
        let committed = &details.transaction;
        ensure!(
            committed.entrypoint() == &TransactionEntrypoint::External(transaction.clone())
                && committed.result().0.is_ok(),
            "committed transaction bytes or result changed"
        );
        let execution_commitment = finality.execution_commitment(
            &peer_identity, &client, height, *committed.block_hash(),
        )?;
        let wire = client.get_canonical_executed_block_wire(
            NonZeroU64::new(height).ok_or_else(|| eyre!("zero Applied height"))?,
            committed,
            &execution_commitment,
        )?;
        let block = decode_framed_signed_block(&wire)?;
        let context = block
            .execution_context()
            .ok_or_else(|| eyre!("committed block omitted execution context"))?;
        let carrier_routes = context
            .external
            .iter()
            .filter(|entry| entry.entrypoint_hash == transaction.hash_as_entrypoint())
            .count();
        ensure!(
            carrier_routes == 0,
            "QueuePlanSynced transaction has {carrier_routes} duplicate carrier routes"
        );
        let reference = context
            .merge_entry
            .as_ref()
            .ok_or_else(|| eyre!("QueuePlanSynced carrier omitted certified merge reference"))?;
        ensure!(
            committed.verify_certified_merge_inclusion(reference),
            "exact transaction/result proofs are not bound to the certified merge reference"
        );
        let catalog = client.get_lane_lifecycle_status()?.validate()?;
        let log = iroha_config::parameters::actual::LaneConfig::from_catalog(&catalog)
            .primary()
            .merge_log_path(&store);
        let entry = committed_merge_entry(&log, reference)?;
        let batch = entry
            .execution_batch
            .as_ref()
            .ok_or_else(|| eyre!("certified merge entry omitted execution batch"))?;
        ensure!(
            merge_execution_batch_commitments_match(batch)
                && batch.application_block_header
                    == merge_application_header_from_carrier(&block.header()),
            "certified merge batch commitments or exact carrier context changed"
        );
        let mut routed = Vec::new();
        for execution in &batch.lanes {
            for (index, candidate) in execution.entrypoints.iter().enumerate() {
                if candidate.hash() == transaction.hash_as_entrypoint() {
                    routed.push((execution, index));
                }
            }
        }
        ensure!(
            routed.len() == 1,
            "exact transaction must have one certified merge route, got {}",
            routed.len()
        );
        let (execution, index) = routed[0];
        let descriptor = &execution.proposal.descriptor;
        ensure!(
            execution.entrypoint_hashes.len() == execution.entrypoints.len()
                && execution.routing_plans.len() == execution.entrypoints.len()
                && execution.results.len() == execution.entrypoints.len()
                && execution.result_hashes.len() == execution.entrypoints.len()
                && descriptor.accepted_transaction_hashes == execution.entrypoint_hashes,
            "certified merge route/result vectors are not aligned with the lane descriptor"
        );
        let plan: RoutingPlan = norito::decode_canonical(&execution.routing_plans[index])?;
        ensure!(
            execution.entrypoints[index] == *committed.entrypoint()
                && execution.entrypoint_hashes[index]
                    == Hash::from(transaction.hash_as_entrypoint())
                && execution.results[index] == *committed.result()
                && execution.result_hashes[index] == Hash::from(*committed.result_hash())
                && descriptor.lane_id == lane
                && descriptor.dataspace_id == dataspace
                && plan == RoutingPlan::single(RoutingDecision::new(lane, dataspace)),
            "the exact successful signed transaction executed on a different certified route: {plan:?}"
        );
        validate_lane_block_proposal(&execution.proposal)?;
        let mut pops = BTreeMap::new();
        for proof in &execution.signer_proofs {
            ensure!(
                pops.insert(proof.public_key.clone(), proof.proof_of_possession.clone())
                    .is_none(),
                "certified lane repeats a signer proof"
            );
        }
        let mut expected_signers = BTreeSet::new();
        for (phase, qc) in [
            (CertPhase::Prepare, &execution.prepare_qc),
            (CertPhase::Commit, &execution.commit_qc),
        ] {
            ensure!(
                qc.body == execution.proposal.vote_body(phase)
                    && qc.validator_set == descriptor.validator_set,
                "lane QC body or committee differs from its certified proposal"
            );
            validate_lane_block_qc_aggregate(qc, &pops)?;
            for (index, peer) in qc.validator_set.iter().enumerate() {
                if qc.signers_bitmap[index / 8] & (1 << (index % 8)) != 0 {
                    expected_signers.insert(peer.public_key().clone());
                }
            }
        }
        ensure!(
            pops.keys().cloned().collect::<BTreeSet<_>>() == expected_signers,
            "lane signer proofs differ from the exact Prepare/Commit signer union"
        );
        if let Some(expected_committee) = expected_committee {
            ensure!(
                descriptor.validator_set == expected_committee
                    && descriptor.validator_count == 4
                    && descriptor.min_quorum == 3,
                "new lane did not execute under its exact four-member/q3 committee"
            );
        }
        Ok(wire)
    })
    .await
}

async fn submit_on_route(
    network: &Network,
    finality: &FixtureFinality,
    instructions: Vec<InstructionBox>,
    lane: LaneId,
    dataspace: DataSpaceId,
    committee: Option<&[PeerId]>,
) -> Result<AppliedEvidence> {
    let mut builder = network.client().client().to_builder();
    builder.transaction_status_timeout = FUNCTIONAL_FINALITY_TIMEOUT;
    let client = builder.build()?;
    let account = client.account_client()?;
    let mut payload = account.prepare_transaction(
        AccountTransactionDraft::new(
            instructions,
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        )
        .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced),
    )?;
    let quote = account
        .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
    ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "fee quote changed payer or gas bound"
    );
    payload.fee_payment = quote.intent;
    let transaction = account.sign_transaction(payload)?;
    ensure!(
        account.submit_transaction_and_wait(&transaction).await? == transaction.hash(),
        "submission changed transaction identity"
    );
    let height = exact_applied_height(network, &transaction).await?;
    let wires = try_join_all(network.peers().iter().map(|peer| {
        canonical_execution(
            finality,
            peer,
            &transaction,
            height,
            lane,
            dataspace,
            committee,
        )
    }))
    .await?;
    ensure!(
        wires.iter().all(|wire| *wire == wires[0]),
        "peers disagree on the exact executed block wire"
    );
    Ok(AppliedEvidence {
        transaction,
        height,
        lane,
        dataspace,
        canonical_block: wires[0].clone(),
    })
}

const PERMISSION_FIXTURE_LIMIT: u64 = 500;

fn complete_permission_page(
    response: &iroha::http::Response<Vec<u8>>,
) -> Result<BTreeSet<Permission>> {
    #[derive(norito::derive::JsonDeserialize)]
    #[norito(deny_unknown_fields)]
    struct Page {
        items: Vec<Permission>,
        total: u64,
    }

    ensure!(
        response.status().as_u16() == 200,
        "permission read failed at offset 0, limit {PERMISSION_FIXTURE_LIMIT}: {}; body: {}",
        response.status(),
        String::from_utf8_lossy(&response.body()[..response.body().len().min(2048)])
    );
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
    };
    ensure!(
        header("content-type").is_some_and(|value| {
            value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .eq_ignore_ascii_case("application/json")
        }) && header("x-iroha-account-permission-semantics") == Some("effective-v1"),
        "permission response omitted its canonical media type or effective semantics"
    );
    let counter = |name: &str| -> Result<u64> {
        header(name)
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| eyre!("permission response omitted {name}"))
    };
    let attempted = counter("x-iroha-fanout-routes-attempted")?;
    ensure!(
        attempted > 0
            && counter("x-iroha-fanout-routes-succeeded")? == attempted
            && counter("x-iroha-fanout-routes-failed")? == 0
            && counter("x-iroha-fanout-routes-denied")? == 0
            && counter("x-iroha-fanout-routes-unavailable")? == 0
            && counter("x-iroha-fanout-routes-not-found")? == 0,
        "permission read returned incomplete fanout"
    );
    let page: Page = json::from_slice(response.body())?;
    ensure!(
        page.total == u64::try_from(page.items.len())?,
        "permission page count mismatch"
    );
    // Each route returns unique permissions, and fanout merges their union.
    // A union smaller than the requested limit proves every route was short.
    // `total` counts only this merged page; it is not a global row count.
    // At the fetch-budget boundary there is no safe next-page exhaustion probe.
    ensure!(
        page.total < PERMISSION_FIXTURE_LIMIT,
        "permission fixture cannot prove exhaustion within its {PERMISSION_FIXTURE_LIMIT}-row fetch budget"
    );
    let permissions: BTreeSet<_> = page.items.into_iter().collect();
    ensure!(
        u64::try_from(permissions.len())? == page.total,
        "permission fanout returned duplicate items"
    );
    Ok(permissions)
}

fn effective_permissions(
    client: &iroha::client::Client,
    account_id: &AccountId,
) -> Result<BTreeSet<Permission>> {
    let response =
        client.get_account_permissions_page_response(account_id, PERMISSION_FIXTURE_LIMIT, 0)?;
    complete_permission_page(&response)
}

#[cfg(test)]
mod permission_page_tests {
    use super::*;
    use iroha::http::Response;

    fn response(items: Vec<Permission>) -> Response<Vec<u8>> {
        let total = items.len();
        let body = json::to_vec(&norito::json!({"total": total, "items": items})).unwrap();
        Response::builder()
            .status(200)
            .header("content-type", "application/json; charset=utf-8")
            .header("x-iroha-account-permission-semantics", "effective-v1")
            .header("x-iroha-fanout-routes-attempted", "4")
            .header("x-iroha-fanout-routes-succeeded", "4")
            .header("x-iroha-fanout-routes-failed", "0")
            .header("x-iroha-fanout-routes-denied", "0")
            .header("x-iroha-fanout-routes-unavailable", "0")
            .header("x-iroha-fanout-routes-not-found", "0")
            .body(body)
            .unwrap()
    }

    #[test]
    fn permission_page_requires_complete_short_fanout() {
        let items = vec![resolution_permission(), resolution_delegation_permission()];
        assert_eq!(
            complete_permission_page(&response(items.clone())).unwrap(),
            items.into_iter().collect()
        );
        assert!(
            complete_permission_page(&response(Vec::new()))
                .unwrap()
                .is_empty()
        );
        for name in [
            "x-iroha-fanout-routes-attempted",
            "x-iroha-fanout-routes-succeeded",
            "x-iroha-fanout-routes-failed",
            "x-iroha-fanout-routes-denied",
            "x-iroha-fanout-routes-unavailable",
            "x-iroha-fanout-routes-not-found",
        ] {
            let mut page = response(vec![resolution_permission()]);
            page.headers_mut().insert(name, "1".parse().unwrap());
            assert!(
                complete_permission_page(&page).is_err(),
                "accepted changed {name}"
            );
            page.headers_mut().remove(name);
            assert!(
                complete_permission_page(&page).is_err(),
                "accepted absent {name}"
            );
        }
    }

    #[test]
    fn permission_page_rejects_saturation_and_duplicate_items() {
        for size in [500, 501] {
            let items = (0..size)
                .map(|value| Permission::new(format!("fixture{value}"), Json::default()))
                .collect();
            assert!(
                complete_permission_page(&response(items))
                    .unwrap_err()
                    .to_string()
                    .contains("cannot prove exhaustion")
            );
        }
        let duplicate = response(vec![resolution_permission(), resolution_permission()]);
        assert!(
            complete_permission_page(&duplicate)
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
    }

    #[test]
    fn permission_page_preserves_failure_context_and_rejects_invalid_metadata() {
        let failed = Response::builder()
            .status(400)
            .body(b"invalid_pagination: fetch budget exceeded".to_vec())
            .unwrap();
        let error = complete_permission_page(&failed).unwrap_err().to_string();
        assert!(error.contains("offset 0, limit 500"));
        assert!(error.contains("400 Bad Request"));
        assert!(error.contains("invalid_pagination"));
        for name in ["content-type", "x-iroha-account-permission-semantics"] {
            let mut page = response(Vec::new());
            page.headers_mut().remove(name);
            assert!(complete_permission_page(&page).is_err());
        }
        let mut mismatch = response(vec![resolution_permission()]);
        *mismatch.body_mut() = br#"{"items":[],"total":1}"#.to_vec();
        assert!(
            complete_permission_page(&mismatch)
                .unwrap_err()
                .to_string()
                .contains("count mismatch")
        );
    }
}

async fn assert_resolution_delegation(network: &Network) -> Result<()> {
    try_join_all(network.peers().iter().map(|peer| async move {
        let client = peer.client().client().clone();
        let permissions =
            read_on_dedicated_thread(move || effective_permissions(&client, &ALICE_ID)).await?;
        ensure!(
            permissions.contains(&resolution_delegation_permission()),
            "peer lost ALICE's exact dataspace alias-resolution delegation"
        );
        Ok::<(), eyre::Report>(())
    }))
    .await?;
    Ok(())
}

async fn assert_catalog_and_history(
    network: &Network,
    finality: &FixtureFinality,
    before: &LaneLifecycleStatusV1,
    after: &LaneLifecycleStatusV1,
    expected_runtime: &NexusRuntimeCatalogV1,
    history: &[AppliedEvidence],
    committee: &[PeerId],
    permission_present: bool,
) -> Result<()> {
    assert_resolution_delegation(network).await?;
    try_join_all(network.peers().iter().map(|peer| async move {
        let (status, runtime) = lifecycle_and_runtime(peer).await?;
        ensure!(
            status == *after && runtime.as_ref() == Some(expected_runtime),
            "peer lost committed topology or inline manifests"
        );
        ensure!(
            before.lanes.iter().all(|lane| status.lanes.contains(lane))
                && before
                    .incarnations
                    .iter()
                    .all(|incarnation| status.incarnations.contains(incarnation)),
            "catalog expansion changed an old lane or incarnation"
        );
        let client = peer.client().client().clone();
        let permissions =
            read_on_dedicated_thread(move || effective_permissions(&client, &BOB_ID)).await?;
        ensure!(
            permissions.contains(&resolution_permission()) == permission_present,
            "replayed permission state differs from committed history"
        );
        for applied in history {
            let current = canonical_execution(
                finality,
                peer,
                &applied.transaction,
                applied.height,
                applied.lane,
                applied.dataspace,
                (applied.lane == ADDED_LANE).then_some(committee),
            )
            .await?;
            ensure!(
                current == applied.canonical_block,
                "recovery changed a historical executed block"
            );
        }
        Ok::<(), eyre::Report>(())
    }))
    .await?;
    Ok(())
}

fn replayed_complete_history(peer: &NetworkPeer, minimum_height: u64) -> Result<bool> {
    for path in [peer.latest_stdout_log_path(), peer.latest_stderr_log_path()]
        .into_iter()
        .flatten()
    {
        for line in BufReader::new(fs::File::open(path)?).lines() {
            let line = line?;
            if !line.contains("Replaying authenticated complete Kura prefix") {
                continue;
            }
            let record: Value = json::from_str(&line)?;
            let fields = record.get("fields").unwrap_or(&record);
            if fields.get("start_height").and_then(Value::as_u64) == Some(1)
                && fields
                    .get("generic_replay_height")
                    .and_then(Value::as_u64)
                    .is_some_and(|height| height >= minimum_height)
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
