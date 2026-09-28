//! Disposable NPoS qualification of an overfull candidate pool and certified retention.
//!
//! Four genesis voters admit separately running candidates into pools of five, eight or
//! eleven. The public rank freezes exactly four or seven seats two epochs ahead, bounded
//! by the signed seven-seat ceiling. A selected seat withholds fresh Pasta keys in the
//! retention cases; the eight-candidate case specifically withholds an incumbent. The
//! current exact quorum must cancel that immutable attempt without losing finality.

use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::{sandbox, sync::rebind_blocking_client};
use iroha::{
    blocking::Client,
    crypto::{Hash, KeyPair, SignatureOf},
    data_model::{
        NetworkId,
        isi::{
            consensus_keys::ApplyThresholdKeyLifecycleCertificateV1,
            kagemusha_v1::KagemushaMintFinalityEpochDecisionV1,
            staking::{
                ExitPublicLaneValidator, PublicLaneCandidateAuthorization,
                RegisterPublicLaneCandidate, RegisterPublicLaneValidator,
            },
        },
        nexus::{
            AdmitValidatorCommitteeSeatV1, PrepareValidatorCommitteeCredentialsV1,
            PublicLaneMonetaryPlanV1, PublicLaneMonetaryPreconditionV1,
            PublicLaneMonetaryRegistrationV1, PublicLaneMonetaryScopeV1,
            ValidatorCandidateKeyAuthorizationV1, ValidatorCandidateKeysV1,
            ValidatorCommitteeCredentialsV1, ValidatorCommitteeOperationV1,
            ValidatorCommitteePreparationV1, ValidatorCommitteeSeatReadinessV1,
        },
        parameter::system::SumeragiNposParameters,
        prelude::*,
        sumeragi::finality::{NativeFinalityArtifact, NativeFinalityJournal, NativeFinalityLimits},
        transaction::FeePaymentIntent,
        validation_fee::ValidationFeePolicyRegistryV1,
    },
};
use iroha_config::parameters::defaults;
use iroha_core::{
    beacon::{
        GlobalThresholdBeaconSessionBindingV1, RuntimeGlobalThresholdBeaconShareCustodyV1,
        credential::global_beacon_partial_signer_public_inventory_digest_v1,
        global_threshold_beacon_roster_hash_v1, prove_global_threshold_beacon_seat_readiness_v1,
        validate_global_threshold_beacon_session_v1,
    },
    sumeragi::{
        certified_chain::CertifiedBlock,
        native_journal::{NativeJournalCursor, with_verified_native_journal},
    },
    validator_committee_evidence::{
        ValidatorCommitteeProvisioningEvidenceV1, ValidatorCommitteeSelectionEvidenceV1,
        verify_validator_committee_provisioning_evidence_v1,
    },
    zk::kagemusha_v1_recursion::verify_kagemusha_mint_finality_candidate_possession_v1,
};
use iroha_genesis::GenesisBlock;
use iroha_model_base::{metadata::Metadata, peer::PeerId, topology::LaneId};
use iroha_test_network::{
    CommitteeValidatorP2pBootstrap, DisposableBeaconProviderBinding, DisposableGenesisDkgOutput,
    DisposablePendingCustodyInput, DisposablePreparedBeaconCustody,
    DisposableRetainedBeaconCredential, DisposableRotationDkgOutput, DisposableRotationProofInput,
    NetworkBuilder, NetworkPeer, init_instruction_registry, prepare_disposable_pending_custody,
    run_disposable_genesis_dkg, run_disposable_rotation_dkg,
};
use iroha_test_samples::ALICE_ID;
use irohad::{
    IrohaRuntimeProviderBindingsV1, IrohaRuntimeProviderSlotV1,
    external_software_signer::encode_consensus_threshold_credential_bundle_v1,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    num::NonZeroU64,
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::{task::spawn_blocking, time::sleep};
use zeroize::Zeroizing;

#[derive(norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct NativeProviderManifest {
    signer_index: u16,
    validator: PeerId,
    handle: String,
    revision: u64,
    policy_digest: [u8; 32],
}

const EPOCH: u64 = 24;
const SELECTION: u64 = EPOCH;
const CUTOFF: u64 = EPOCH * 2;
const TARGET_FIRST: u64 = EPOCH * 2 + 1;
const TARGET_LAST: u64 = EPOCH * 3;
const WAIT: Duration = Duration::from_secs(600);
const POLL: Duration = Duration::from_millis(150);
const TAIRA_XOR: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";

#[derive(Clone)]
struct Operator {
    account: AccountId,
    peer: PeerId,
    consensus_key: KeyPair,
    pop: Vec<u8>,
    client: Client,
}

fn exact_quorum(seats: usize) -> Result<u32> {
    ensure!(
        seats >= 4 && (seats - 1) % 3 == 0,
        "not an exact 3f+1 roster"
    );
    Ok(u32::try_from(2 * ((seats - 1) / 3) + 1)?)
}

fn finality_limits() -> NativeFinalityLimits {
    NativeFinalityLimits {
        block_bytes: 32 * 1024 * 1024,
        journal_bytes: 64 * 1024 * 1024,
        block_count: 256,
        allocated_bytes: 512 * 1024 * 1024,
    }
}

fn verify_equal_vote_context(proof: &CertifiedBlock, expected: &BTreeSet<PeerId>) -> Result<()> {
    let context = &proof.commitment().schedule.current;
    let actual = context
        .committee
        .iter()
        .map(|seat| seat.validator.clone())
        .collect::<BTreeSet<_>>();
    let quorum = usize::try_from(exact_quorum(expected.len())?)?;
    ensure!(
        context.committee.len() == expected.len()
            && actual == *expected
            && proof
                .commit_qc()
                .is_some_and(|qc| qc.signers.count_ones() == quorum),
        "native finality is not an exact equal-vote certificate for the expected roster"
    );
    Ok(())
}

fn ranked_target(
    preparation: &ValidatorCommitteePreparationV1,
    pool: &BTreeSet<PeerId>,
    seats: usize,
) -> BTreeSet<PeerId> {
    let mut ranked = pool
        .iter()
        .map(|peer| {
            // Independent reproduction of the public, network/epoch-bound election rank.
            let identity = norito::encode_canonical(peer).expect("canonical candidate");
            let rank: [u8; 32] = Hash::new_from_chunks(&[
                b"iroha:validator-seat:v1",
                &[0],
                preparation.network_id.as_bytes(),
                &preparation.selection_epoch.to_le_bytes(),
                &preparation.target_epoch.to_le_bytes(),
                &preparation.election_seed,
                &identity,
            ])
            .into();
            (rank, peer.clone())
        })
        .collect::<Vec<_>>();
    ranked.sort();
    ranked
        .into_iter()
        .take(seats)
        .map(|(_, peer)| peer)
        .collect()
}

fn validator_xor_escrow(
    genesis: &iroha_genesis::GenesisBlock,
    xor: &AssetDefinitionId,
) -> Result<AssetId> {
    let fee_asset: AssetDefinitionId = defaults::nexus::fees::fee_asset_id().parse()?;
    ensure!(
        fee_asset == *xor && SumeragiNposParameters::default().xor_asset_definition_id == *xor,
        "fee, staking and NPoS defaults must select the same canonical XOR"
    );
    let mut definitions = 0;
    let mut signed_npos = 0;
    let mut registrations = 0;
    let mut alice_funded = false;
    let mut escrow = None;
    for transaction in genesis.0.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            continue;
        };
        for instruction in instructions {
            if let Some(iroha::data_model::isi::RegisterBox::AssetDefinition(register)) =
                instruction
                    .as_any()
                    .downcast_ref::<iroha::data_model::isi::RegisterBox>()
                && register.object.id == *xor
            {
                definitions += 1;
                ensure!(
                    register.object.spec == iroha_primitives::numeric::NumericSpec::fractional(9),
                    "signed XOR definition must have its canonical fractional quantity"
                );
            }
            if let Some(set) = instruction.as_any().downcast_ref::<SetParameter>()
                && let Parameter::Custom(custom) = set.inner()
                && custom.id() == &SumeragiNposParameters::parameter_id()
            {
                signed_npos += 1;
                let parameters = SumeragiNposParameters::from_custom_parameter(custom)
                    .ok_or_else(|| eyre!("signed genesis NPoS parameters are invalid"))?;
                ensure!(
                    parameters.xor_asset_definition_id == *xor,
                    "signed NPoS parameters bind a foreign staking asset"
                );
            }
            if let Some(registration) = instruction
                .as_any()
                .downcast_ref::<RegisterPublicLaneValidator>()
            {
                registrations += 1;
                ensure!(
                    registration.monetary_plan.source_asset.definition() == xor
                        && registration.monetary_plan.destination_asset.definition() == xor
                        && registration.monetary_plan.amount == registration.initial_stake,
                    "signed validator bond must transfer exact canonical XOR principal"
                );
                if let Some(existing) = &escrow {
                    ensure!(
                        existing == &registration.monetary_plan.destination_asset,
                        "genesis validator bonds must share the exact XOR escrow"
                    );
                } else {
                    escrow = Some(registration.monetary_plan.destination_asset.clone());
                }
            }
            if let Some(iroha::data_model::isi::MintBox::Asset(mint)) =
                instruction
                    .as_any()
                    .downcast_ref::<iroha::data_model::isi::MintBox>()
                && mint.destination.definition() == xor
                && mint.destination.account() == &*ALICE_ID
            {
                alice_funded = true;
            }
        }
    }
    ensure!(
        definitions == 1 && signed_npos == 1 && registrations == 4 && alice_funded,
        "signed genesis must define one canonical XOR, bind NPoS, fund XOR and bond all four voters"
    );
    escrow.ok_or_else(|| eyre!("signed genesis has no exact validator XOR escrow"))
}

fn admit_candidates(
    admin: &Client,
    candidates: &[Operator],
    network_id: NetworkId,
    xor: &AssetDefinitionId,
    escrow: &AssetId,
) -> Result<()> {
    let parameters = admin.client().query_single(FindParameters)?;
    ensure!(
        !parameters
            .custom()
            .contains_key(&ValidationFeePolicyRegistryV1::parameter_id()),
        "this custody scenario requires no enacted validation-fee policy"
    );
    let funding = candidates
        .iter()
        .flat_map(|operator| {
            [
                Register::account(Account::new(operator.account.clone())).into(),
                Transfer::asset_quantity(
                    AssetId::new(xor.clone(), ALICE_ID.clone()),
                    10_000_u64,
                    operator.account.clone(),
                )
                .into(),
            ]
        })
        .collect::<Vec<InstructionBox>>();
    admin.submit_all(funding, FeePaymentIntent::authority(Vec::new(), None))?;
    for operator in candidates {
        let inclusion_height = admin.status().get()?.blocks + 1;
        ensure!(
            inclusion_height < SELECTION,
            "candidate missed the unfrozen selecting prestate"
        );
        let registration = RegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: operator.account.clone(),
            peer_id: operator.peer.clone(),
            stake_account: operator.account.clone(),
            initial_stake: 2_000_u64.into(),
            metadata: Metadata::default(),
            monetary_plan: PublicLaneMonetaryPlanV1 {
                network_scope: PublicLaneMonetaryScopeV1::Network(network_id),
                valid_until_height: SELECTION - 1,
                source_asset: AssetId::new(xor.clone(), operator.account.clone()),
                destination_asset: escrow.clone(),
                amount: 2_000_u64.into(),
                precondition: PublicLaneMonetaryPreconditionV1::Registration(
                    PublicLaneMonetaryRegistrationV1 {
                        activation_height: TARGET_FIRST,
                    },
                ),
            },
        };
        let authorization =
            PublicLaneCandidateAuthorization::new(network_id, registration.clone(), TARGET_FIRST);
        let candidate = RegisterPublicLaneCandidate {
            registration,
            activation_height: TARGET_FIRST,
            proof_of_possession: operator.pop.clone(),
            peer_signature: SignatureOf::try_new(
                operator.consensus_key.private_key(),
                &authorization,
            )?,
        };
        operator
            .client
            .submit(candidate, FeePaymentIntent::authority(Vec::new(), None))
            .wrap_err_with(|| format!("candidate {} admission failed", operator.peer))?;
    }
    Ok(())
}

fn publish_selected(
    operators: &BTreeMap<PeerId, Operator>,
    processes: &BTreeMap<PeerId, NetworkPeer>,
    selected: &BTreeSet<PeerId>,
    withheld: Option<&PeerId>,
    network_id: NetworkId,
    generation: u64,
) -> Result<()> {
    for peer in selected {
        if withheld == Some(peer) {
            continue;
        }
        let operator = operators
            .get(peer)
            .ok_or_else(|| eyre!("selected peer lacks a real operator"))?;
        let process = processes
            .get(peer)
            .ok_or_else(|| eyre!("selected peer lacks held Pasta custody"))?;
        let (keys, possession) =
            process.disposable_mint_finality_candidate(network_id, generation)?;
        verify_kagemusha_mint_finality_candidate_possession_v1(
            network_id,
            generation,
            &keys,
            &possession,
        )?;
        let authorization = ValidatorCandidateKeyAuthorizationV1::new(
            network_id,
            generation,
            keys.clone(),
            possession.clone(),
        );
        let publication = ValidatorCandidateKeysV1 {
            network_id,
            generation,
            keys,
            possession,
            peer_signature: SignatureOf::try_new(
                operator.consensus_key.private_key(),
                &authorization,
            )?,
        };
        operator.client.submit(
            SetParameter::new(Parameter::Custom(
                ValidatorCommitteeOperationV1::PublishCandidate(publication)
                    .into_custom_parameter(),
            )),
            FeePaymentIntent::authority(Vec::new(), None),
        )?;
    }
    Ok(())
}

async fn advance_to_height(
    network: &sandbox::SerializedNetwork,
    voters: &[PeerId],
    target: u64,
) -> Result<()> {
    let peers = exact_process_roster(network, voters)?;
    let deadline = Instant::now() + WAIT;
    let mut tick = 0_u64;
    loop {
        let mut heights = Vec::new();
        for peer in &peers {
            heights.push(peer.status().await?.blocks);
        }
        let reached = heights.iter().filter(|height| **height >= target).count();
        if reached == peers.len() {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "current validator quorum stalled before height {target}; heights={heights:?}"
        );
        let client = peers[usize::try_from(tick)? % peers.len()].client();
        let message = format!("committee transition progress {target}:{tick}");
        spawn_blocking(move || {
            client.submit(
                Log::new(Level::INFO, message),
                FeePaymentIntent::authority(Vec::new(), None),
            )
        })
        .await
        .wrap_err("progress submit worker panicked")??;
        tick += 1;
        sleep(POLL).await;
    }
}

fn exact_process_roster<'a>(
    network: &'a sandbox::SerializedNetwork,
    roster: &[PeerId],
) -> Result<Vec<&'a NetworkPeer>> {
    let peers = network
        .validators()
        .iter()
        .chain(network.committee_validators());
    let mut ordered = Vec::with_capacity(roster.len());
    for validator in roster {
        let peer = peers
            .clone()
            .find(|peer| peer.id() == *validator)
            .ok_or_else(|| eyre!("authenticated committee lacks a disposable process"))?;
        ordered.push(peer);
    }
    ensure!(
        ordered.len() == roster.len()
            && roster.iter().collect::<BTreeSet<_>>().len() == roster.len(),
        "committee process roster is not an exact one-to-one mapping"
    );
    Ok(ordered)
}

async fn advance_exact_rotation_phase(
    network: &sandbox::SerializedNetwork,
    voters: &[PeerId],
    height: u64,
) -> Result<()> {
    ensure!(
        exact_quorum(voters.len()).is_ok(),
        "rotation phase requires an exact current quorum geometry"
    );
    let peers = exact_process_roster(network, voters)?;
    let mut before = Vec::with_capacity(peers.len());
    for peer in &peers {
        before.push(peer.status().await?.blocks);
    }
    ensure!(
        before.iter().all(|observed| *observed == height - 1),
        "rotation DKG phase h{height} started after a voter passed its predecessor: {before:?}"
    );
    let client = peers[0].client();
    spawn_blocking(move || {
        client.submit(
            Log::new(Level::INFO, format!("rotation DKG exact phase h{height}")),
            FeePaymentIntent::authority(Vec::new(), None),
        )
    })
    .await
    .wrap_err("rotation DKG phase submit worker panicked")??;
    let deadline = Instant::now() + WAIT;
    loop {
        let mut observed = Vec::with_capacity(peers.len());
        for peer in &peers {
            observed.push(peer.status().await?.blocks);
        }
        ensure!(
            observed.iter().all(|current| *current <= height),
            "rotation DKG phase h{height} was overtaken: {observed:?}"
        );
        if observed.iter().all(|current| *current == height) {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "rotation DKG phase h{height} did not finalize on all current voters: {observed:?}"
        );
        sleep(POLL).await;
    }
}

async fn advance_exact_genesis_phase(
    network: &sandbox::SerializedNetwork,
    height: u64,
) -> Result<()> {
    ensure!(
        (2..=4).contains(&height),
        "invalid genesis DKG phase height"
    );
    let mut before = Vec::new();
    for peer in network.validators() {
        before.push(peer.status().await?.blocks);
    }
    ensure!(
        before.iter().all(|observed| *observed == height - 1),
        "genesis DKG phase h{height} started after a peer passed its predecessor: {before:?}"
    );
    let client = network.validators()[0].client();
    spawn_blocking(move || {
        client.submit(
            Log::new(Level::INFO, format!("genesis DKG exact phase h{height}")),
            FeePaymentIntent::authority(Vec::new(), None),
        )
    })
    .await
    .wrap_err("genesis DKG phase submit worker panicked")??;
    let deadline = Instant::now() + WAIT;
    loop {
        let mut observed = Vec::new();
        for peer in network.validators() {
            observed.push(peer.status().await?.blocks);
        }
        ensure!(
            observed.iter().all(|current| *current <= height),
            "genesis DKG phase h{height} was overtaken: {observed:?}"
        );
        if observed.iter().all(|current| *current == height) {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "genesis DKG phase h{height} did not finalize on all four seats: {observed:?}"
        );
        sleep(POLL).await;
    }
}

fn read_finality_chain(
    client: &Client,
    network_id: NetworkId,
    genesis_voters: &BTreeSet<PeerId>,
    end: u64,
) -> Result<(CertifiedBlock, CertifiedBlock)> {
    let (_, blocks) =
        read_contiguous_finality_chain(client, network_id, network_id.into_genesis_hash(), end)?;
    let genesis = blocks
        .first()
        .ok_or_else(|| eyre!("missing signed genesis"))?;
    let epoch = &genesis.commitment().schedule.current;
    ensure!(
        epoch.authority.generation == 0
            && epoch
                .committee
                .iter()
                .map(|seat| seat.validator.clone())
                .collect::<BTreeSet<_>>()
                == *genesis_voters,
        "actual signed genesis differs from independently built initial authority"
    );
    let selection = blocks
        .iter()
        .find(|block| block.height() == SELECTION)
        .ok_or_else(|| eyre!("missing native selecting boundary"))?
        .clone();
    let cutoff = blocks
        .iter()
        .find(|block| block.height() == CUTOFF)
        .ok_or_else(|| eyre!("missing native cutoff"))?
        .clone();
    Ok((selection, cutoff))
}

fn read_genesis_dkg_finality_chain(
    client: &Client,
    genesis: &GenesisBlock,
    chain_id: &str,
    end: u64,
) -> Result<NativeFinalityJournal> {
    ensure!(
        (2..=4).contains(&end),
        "genesis DKG phase must have actual H2–H4 finality"
    );
    Ok(read_contiguous_finality_chain(client, network_id, signed_genesis_hash, end)?.0)
}

fn read_contiguous_finality_chain(
    client: &Client,
    network_id: NetworkId,
    signed_genesis_hash: iroha::crypto::HashOf<iroha::data_model::block::BlockHeader>,
    end: u64,
) -> Result<(NativeFinalityJournal, Vec<CertifiedBlock>)> {
    ensure!(
        (2..=256).contains(&end),
        "committee proof cut exceeds its explicit disposable bound"
    );
    ensure!(
        network_id.into_genesis_hash() == signed_genesis_hash,
        "network differs from independent signed genesis"
    );
    // The iterable read has an explicit count bound; no discarded suffix is treated as a trust root.
    let mut source = client
        .client()
        .query(FindBlocks)
        .with_pagination(iroha::data_model::query::parameters::Pagination::new(
            NonZeroU64::new(257),
            0,
        ))
        .execute_all()?;
    ensure!(
        source.len() <= 256,
        "disposable source outgrew configured qualification prefix"
    );
    source.retain(|block| block.header().height().get() <= end);
    source.sort_by_key(|block| block.header().height());
    ensure!(
        source.len() == usize::try_from(end)?,
        "native source lacks complete signed-genesis prefix"
    );
    let limits = finality_limits();
    let journal = NativeFinalityJournal {
        blocks: source
            .iter()
            .map(|block| {
                NativeFinalityArtifact::from_block(block, limits).map_err(|error| eyre!(error))
            })
            .collect::<Result<Vec<_>>>()?,
    };
    let cursor = NativeJournalCursor::new(client.client().chain().clone(), network_id, limits)
        .map_err(|error| eyre!(error))?;
    let blocks = with_verified_native_journal(
        &journal,
        client.client().chain(),
        &network_id,
        limits,
        cursor.attestations(),
        |reader| {
            reader
                .walk(1, end)
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())
        },
    )
    .map_err(|error| eyre!(error))?;
    Ok((journal, blocks))
}

async fn stage_genesis_brokers(
    network: &sandbox::SerializedNetwork,
    dkg: &DisposableGenesisDkgOutput,
) -> Result<BTreeMap<PeerId, Vec<u8>>> {
    let network_id = network.network_id();
    let chain_id = network.chain_id().to_string();
    ensure!(
        dkg.seats.len() == 4,
        "genesis DKG must yield four private seats"
    );
    let mut catalogs = BTreeMap::new();
    for output in &dkg.seats {
        let peer = network
            .validators()
            .iter()
            .find(|peer| peer.id() == output.validator)
            .ok_or_else(|| eyre!("DKG output has no real genesis voter"))?;
        let provider: NativeProviderManifest =
            norito::json::from_slice(&fs::read(&output.provider_path)?)?;
        let expected_digest = global_beacon_partial_signer_public_inventory_digest_v1(
            network_id,
            &[(dkg.public_session.clone(), output.signer_index)],
        )?;
        ensure!(
            provider.signer_index == output.signer_index
                && provider.validator == output.validator
                && provider.handle == output.provider_handle
                && provider.revision == output.provider_revision
                && provider.policy_digest == expected_digest,
            "genesis provider manifest is not this exact real seat's public inventory"
        );
        let catalog = IrohaRuntimeProviderBindingsV1::with_prepared_beacon_inventory_v1(
            None,
            &chain_id,
            network_id,
            &provider.handle,
            provider.revision,
            provider.policy_digest,
        )?
        .export_canonical_v1()?;
        let credential = Zeroizing::new(fs::read(&output.credential_path)?);
        let bundle = encode_consensus_threshold_credential_bundle_v1(Some(&credential), None)?;
        peer.provision_disposable_runtime_provider_broker(
            &catalog,
            bundle,
            DisposableBeaconProviderBinding {
                handle: provider.handle,
                revision: provider.revision,
                policy_digest: provider.policy_digest,
            },
        )
        .await?;
        catalogs.insert(output.validator.clone(), catalog);
    }
    Ok(catalogs)
}

#[derive(Clone)]
struct CurrentCustodyRecord {
    credential_path: PathBuf,
    catalog: Vec<u8>,
}

struct PreparedRotation {
    target: Vec<PeerId>,
    prepared: Vec<DisposablePreparedBeaconCustody>,
    _dkg: DisposableRotationDkgOutput,
}

fn genesis_current_custody(
    dkg: &DisposableGenesisDkgOutput,
    catalogs: &BTreeMap<PeerId, Vec<u8>>,
) -> Result<BTreeMap<PeerId, CurrentCustodyRecord>> {
    dkg.seats
        .iter()
        .map(|seat| {
            Ok((
                seat.validator.clone(),
                CurrentCustodyRecord {
                    credential_path: seat.credential_path.clone(),
                    catalog: catalogs
                        .get(&seat.validator)
                        .ok_or_else(|| eyre!("genesis seat lacks its exact public catalog"))?
                        .clone(),
                },
            ))
        })
        .collect()
}

fn prepared_current_custody(
    prepared: &[DisposablePreparedBeaconCustody],
) -> Result<BTreeMap<PeerId, CurrentCustodyRecord>> {
    prepared
        .iter()
        .map(|seat| {
            Ok((
                seat.validator.clone(),
                CurrentCustodyRecord {
                    credential_path: seat.credential_path.clone(),
                    catalog: fs::read(&seat.catalog_path)?,
                },
            ))
        })
        .collect()
}

async fn stage_prepared_brokers(
    network: &sandbox::SerializedNetwork,
    prepared: &[DisposablePreparedBeaconCustody],
    expected_revision: u64,
) -> Result<()> {
    for output in prepared {
        let catalog = fs::read(&output.catalog_path)?;
        let validated = IrohaRuntimeProviderBindingsV1::load_canonical_v1(&catalog)?;
        ensure!(
            validated.network_id() == &network.network_id()
                && validated.chain_id() == network.chain_id().to_string(),
            "prepared custody catalog differs from the signed network"
        );
        let binding = validated
            .iter()
            .find(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner)
            .ok_or_else(|| eyre!("native prepared catalog has no beacon binding"))?;
        ensure!(
            binding.revision() == Some(expected_revision)
                && binding
                    .policy_digest()
                    .is_some_and(|digest| digest != [0; 32]),
            "native prepared catalog lacks the exact new revision and public digest"
        );
        let peer = network
            .validators()
            .iter()
            .chain(network.committee_validators())
            .find(|peer| peer.id() == output.validator)
            .ok_or_else(|| eyre!("prepared custody has no real target process"))?;
        let credential = Zeroizing::new(fs::read(&output.credential_path)?);
        let bundle = encode_consensus_threshold_credential_bundle_v1(Some(&credential), None)?;
        peer.provision_disposable_runtime_provider_broker(
            &catalog,
            bundle,
            DisposableBeaconProviderBinding {
                handle: binding.handle().to_owned(),
                revision: expected_revision,
                policy_digest: binding.policy_digest().expect("checked public digest"),
            },
        )
        .await?;
    }
    Ok(())
}

fn prove_exact_target_readiness(
    transition: &iroha::data_model::nexus::ValidatorCommitteeTransitionV1,
    session: &iroha::data_model::consensus::GlobalThresholdBeaconKeySessionV1,
    output: &iroha_test_network::DisposableRotationSeatOutput,
    process: &NetworkPeer,
) -> Result<AdmitValidatorCommitteeSeatV1> {
    let index = u32::from(output.signer_index - 1);
    let context = transition
        .readiness_context(index)
        .map_err(|error| eyre!(error))?;
    let credentials = transition
        .credentials
        .as_ref()
        .ok_or_else(|| eyre!("readiness requires complete prepared credentials"))?;
    let roster = transition
        .preparation
        .committee
        .iter()
        .map(|seat| seat.validator.clone())
        .collect::<Vec<_>>();
    ensure!(
        roster.get(usize::try_from(index)?) == Some(&output.validator),
        "private share differs from the exact target roster seat"
    );
    let roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
    ensure!(
        session.roster_hash == roster_hash && usize::from(session.committee_size) == roster.len(),
        "pending beacon is not bound to the exact ordered target committee"
    );
    let binding = GlobalThresholdBeaconSessionBindingV1 {
        network_id: session.network_id,
        session_id: session.session_id,
        roster_hash,
        transcript_hash: session.transcript_hash,
    };
    let validated = validate_global_threshold_beacon_session_v1(session.clone(), &binding)?;
    let share = Zeroizing::new(fs::read(&output.pending_share_path)?);
    ensure!(
        share.len() == 96,
        "native private share is not three exact components"
    );
    let mut components = Zeroizing::new([[0_u8; 32]; 3]);
    for (component, bytes) in components.iter_mut().zip(share.chunks_exact(32)) {
        component.copy_from_slice(bytes);
    }
    let custody = RuntimeGlobalThresholdBeaconShareCustodyV1::new();
    custody.import_components(session.clone(), &binding, output.signer_index, components)?;
    ensure!(
        process.id() == output.validator,
        "Pasta seed belongs to another process"
    );
    let pasta = process.disposable_mint_finality_readiness(&credentials.authority, &context)?;
    let beacon = prove_global_threshold_beacon_seat_readiness_v1(
        &custody,
        &validated,
        &credentials.authority,
        &context,
    )?;
    Ok(AdmitValidatorCommitteeSeatV1 {
        transition_id: transition
            .preparation
            .transition_id()
            .map_err(|error| eyre!(error))?,
        target_epoch: transition.preparation.target_epoch,
        readiness: ValidatorCommitteeSeatReadinessV1 {
            validator_index: index,
            pasta,
            beacon,
        },
    })
}

async fn execute_rotation_preparation(
    network: &sandbox::SerializedNetwork,
    admin: &Client,
    operators: &BTreeMap<PeerId, Operator>,
    preparation: &ValidatorCommitteePreparationV1,
    current_custody: &BTreeMap<PeerId, CurrentCustodyRecord>,
    signed_genesis_hash: iroha::crypto::HashOf<iroha::data_model::block::BlockHeader>,
    provider_revision: u64,
    missing_custody: Option<&PeerId>,
) -> Result<PreparedRotation> {
    let network_id = network.network_id();
    let target_epoch = preparation.target_epoch;
    let transition_id = Hash::prehashed(preparation.transition_id().map_err(|error| eyre!(error))?);
    let status = spawn_blocking({
        let admin = admin.clone();
        move || {
            admin
                .client()
                .get_validator_committee_status(Some(target_epoch))
        }
    })
    .await
    .wrap_err("rotation selection status worker panicked")??;
    let observed = status
        .latest_finality
        .decode_block(finality_limits())
        .map_err(|error| eyre!(error))?
        .header()
        .height()
        .get();
    ensure!(
        status
            .selected
            .as_ref()
            .is_some_and(|selected| selected.transition.preparation == *preparation)
            && observed > preparation.selection_height
            && observed + 4 < preparation.first_height - 1,
        "rotation status missed the immutable preparation window"
    );
    let target = preparation
        .committee
        .iter()
        .map(|seat| seat.validator.clone())
        .collect::<Vec<_>>();
    let target_seats = exact_process_roster(network, &target)?;
    let (finality_journal, certified_chain) = spawn_blocking({
        let admin = admin.clone();
        move || read_contiguous_finality_chain(&admin, network_id, signed_genesis_hash, observed)
    })
    .await
    .wrap_err("selection finality worker panicked")??;
    let latest = certified_chain
        .last()
        .ok_or_else(|| eyre!("missing observed native tip"))?;
    ensure!(
        NativeFinalityArtifact::from_block(latest.block(), finality_limits())
            .map_err(|error| eyre!(error))?
            == status.latest_finality,
        "status source differs from authenticated native tip"
    );
    let current_roster = latest
        .commitment()
        .schedule
        .current
        .committee
        .iter()
        .map(|seat| seat.validator.clone())
        .collect::<Vec<_>>();
    let authorizing_seats = exact_process_roster(network, &current_roster)?;
    let selection_evidence = ValidatorCommitteeSelectionEvidenceV1 {
        status,
        finality_journal,
    };
    let input = DisposableRotationProofInput {
        network_id,
        chain_id: network.chain_id(),
        finality_limits: finality_limits(),
        target_epoch,
        transition_id,
    };
    let certificate_height = observed + 4;
    let dkg = run_disposable_rotation_dkg(
        &target_seats,
        &authorizing_seats,
        &selection_evidence,
        input,
        provider_revision,
        certificate_height,
        |height| {
            let admin = admin.clone();
            let voters = current_roster.clone();
            let genesis = network.genesis();
            let chain_id = network.chain_id().to_string();
            async move {
                advance_exact_rotation_phase(network, &voters, height).await?;
                let observed = spawn_blocking({
                    let admin = admin.clone();
                    move || -> Result<u64> { Ok(admin.status().get()?.blocks) }
                })
                .await
                .wrap_err("rotation phase status worker panicked")??;
                ensure!(
                    observed == height,
                    "rotation DKG public phase missed exact h{height} observation"
                );
                spawn_blocking(move || {
                    Ok(read_contiguous_finality_chain(
                        &admin,
                        network_id,
                        signed_genesis_hash,
                        height,
                    )?
                    .0)
                })
                .await
                .wrap_err("rotation phase finality worker panicked")?
            }
        },
    )
    .await?;
    let finalize: Vec<InstructionBox> =
        norito::json::from_slice(&fs::read(&dkg.finalization_instruction_path)?)?;
    ensure!(
        finalize.len() == 1,
        "native rotation must emit one lifecycle instruction"
    );
    let certificate = finalize[0]
        .as_any()
        .downcast_ref::<ApplyThresholdKeyLifecycleCertificateV1>()
        .ok_or_else(|| eyre!("native rotation instruction is not beacon finalization"))?
        .certificate
        .clone();
    let at_height = spawn_blocking({
        let admin = admin.clone();
        move || -> Result<u64> { Ok(admin.status().get()?.blocks + 1) }
    })
    .await
    .wrap_err("rotation certificate height worker panicked")??;
    ensure!(
        at_height == certificate_height && certificate.effective_height == certificate_height,
        "native finalization draft is not the exact next execution height"
    );
    spawn_blocking({
        let admin = admin.clone();
        move || admin.submit_all(finalize, FeePaymentIntent::authority(Vec::new(), None))
    })
    .await
    .wrap_err("rotation finalization worker panicked")??;
    let finalized_status = spawn_blocking({
        let admin = admin.clone();
        move || {
            admin
                .client()
                .get_validator_committee_status(Some(target_epoch))
        }
    })
    .await
    .wrap_err("rotation finalized status worker panicked")??;
    let session = finalized_status
        .pending_beacon_session
        .as_ref()
        .ok_or_else(|| eyre!("incumbent-certified target DKG is not pending on chain"))?;
    ensure!(
        session == &dkg.public_session,
        "on-chain pending beacon differs from the all-seat native transcript"
    );
    let authority =
        iroha::data_model::isi::kagemusha_v1::KagemushaMintFinalityAuthorityGenerationV1 {
            version: 1,
            network_id,
            generation: preparation.authority_generation,
            validators: target
                .iter()
                .map(|peer| {
                    finalized_status
                        .candidate_keys
                        .iter()
                        .find(|row| row.keys.validator == *peer)
                        .map(|row| row.keys.clone())
                        .ok_or_else(|| eyre!("target seat has no exact published generation keys"))
                })
                .collect::<Result<Vec<_>>>()?,
        };
    let credentials = ValidatorCommitteeCredentialsV1 {
        authority,
        beacon: iroha::data_model::isi::kagemusha_v1::InstalledBeaconEpochBindingV1 {
            session_id: session.session_id,
            transcript_hash: session.transcript_hash,
        },
    };
    let prepare =
        ValidatorCommitteeOperationV1::PrepareCredentials(PrepareValidatorCommitteeCredentialsV1 {
            transition_id: preparation.transition_id().map_err(|error| eyre!(error))?,
            target_epoch,
            credentials,
        });
    let owner = operators
        .get(&target[0])
        .ok_or_else(|| eyre!("first target seat lacks an owning operator"))?
        .client
        .clone();
    spawn_blocking(move || {
        owner.submit(
            SetParameter::new(Parameter::Custom(prepare.into_custom_parameter())),
            FeePaymentIntent::authority(Vec::new(), None),
        )
    })
    .await
    .wrap_err("rotation credential preparation worker panicked")??;
    let prepared_status = spawn_blocking({
        let admin = admin.clone();
        move || {
            admin
                .client()
                .get_validator_committee_status(Some(target_epoch))
        }
    })
    .await
    .wrap_err("rotation prepared status worker panicked")??;
    let prepared_transition = prepared_status
        .selected
        .as_ref()
        .ok_or_else(|| eyre!("prepared status lost its immutable selection"))?
        .transition
        .clone();
    ensure!(
        prepared_transition.credentials.is_some()
            && prepared_transition.readiness.is_empty()
            && prepared_status.pending_beacon_session.as_ref() == Some(&dkg.public_session),
        "prepared status lacks exact public target credentials"
    );
    let proof_end = prepared_status
        .latest_finality
        .decode_block(finality_limits())
        .map_err(|error| eyre!(error))?
        .header()
        .height()
        .get();
    let (custody_journal, _) = spawn_blocking({
        let admin = admin.clone();
        move || read_contiguous_finality_chain(&admin, network_id, signed_genesis_hash, proof_end)
    })
    .await
    .wrap_err("custody finality worker panicked")??;
    let custody_evidence = ValidatorCommitteeProvisioningEvidenceV1 {
        status: prepared_status,
        finality_journal: custody_journal,
        beacon_finalization: certificate,
    };
    let proof_cursor = NativeJournalCursor::new(network.chain_id(), network_id, finality_limits())
        .map_err(|error| eyre!(error))?;
    verify_validator_committee_provisioning_evidence_v1(
        &custody_evidence,
        &network.chain_id(),
        network_id,
        target_epoch,
        preparation.transition_id().map_err(|error| eyre!(error))?,
        finality_limits(),
        proof_cursor.attestations(),
    )
    .map_err(|error| eyre!("native custody evidence was not independently authorized: {error}"))?;
    let mut prepared = Vec::with_capacity(target.len());
    for seat in &dkg.seats {
        if missing_custody == Some(&seat.validator) {
            fs::remove_file(&seat.pending_share_path)?;
            fs::remove_file(&seat.credential_path)?;
            continue;
        }
        let retained = current_custody.get(&seat.validator).map(|current| {
            DisposableRetainedBeaconCredential {
                credential_path: &current.credential_path,
                catalog: &current.catalog,
            }
        });
        prepared.push(
            prepare_disposable_pending_custody(
                &custody_evidence,
                &seat.pending_share_path,
                retained,
                DisposablePendingCustodyInput {
                    network_id,
                    finality_limits: finality_limits(),
                    target_epoch,
                    transition_id,
                    local_validator: seat.validator.clone(),
                    chain_id: network.chain_id().to_string(),
                    handle: seat.provider_handle.clone(),
                    revision: provider_revision,
                },
            )
            .await?,
        );
    }
    network.shutdown().await;
    stage_prepared_brokers(network, &prepared, provider_revision).await?;
    network.start_all().await?;
    network.ensure_blocks(proof_end).await?;
    for seat in &dkg.seats {
        if missing_custody == Some(&seat.validator) {
            continue;
        }
        let process = network
            .validators()
            .iter()
            .chain(network.committee_validators())
            .find(|peer| peer.id() == seat.validator)
            .ok_or_else(|| eyre!("prepared seat has no owner-private Pasta seed process"))?;
        let admission =
            prove_exact_target_readiness(&prepared_transition, &dkg.public_session, seat, process)?;
        let owner = operators
            .get(&seat.validator)
            .ok_or_else(|| eyre!("prepared seat lacks a real owning operator"))?
            .client
            .clone();
        spawn_blocking(move || {
            owner.submit(
                SetParameter::new(Parameter::Custom(
                    ValidatorCommitteeOperationV1::AdmitSeat(admission).into_custom_parameter(),
                )),
                FeePaymentIntent::authority(Vec::new(), None),
            )
        })
        .await
        .wrap_err("target seat readiness admission worker panicked")??;
    }
    let readiness = spawn_blocking({
        let admin = admin.clone();
        move || {
            admin
                .client()
                .get_validator_committee_status(Some(target_epoch))
        }
    })
    .await
    .wrap_err("target readiness status worker panicked")??;
    ensure!(
        readiness.selected.as_ref().is_some_and(|selected| {
            selected.transition.readiness.len()
                == target.len() - usize::from(missing_custody.is_some())
        }),
        "on-chain seat readiness does not match actual provisioned target custody"
    );
    Ok(PreparedRotation {
        target,
        prepared,
        _dkg: dkg,
    })
}

async fn run_custody_or_activation_scenario(
    scenario: QualificationScenario,
    network: &sandbox::SerializedNetwork,
    admin: &Client,
    genesis_dkg: &DisposableGenesisDkgOutput,
    genesis_catalogs: &BTreeMap<PeerId, Vec<u8>>,
    operators: &BTreeMap<PeerId, Operator>,
    genesis_voters: &BTreeSet<PeerId>,
    pool: &BTreeSet<PeerId>,
    first_preparation: &ValidatorCommitteePreparationV1,
    signed_genesis_hash: iroha::crypto::HashOf<iroha::data_model::block::BlockHeader>,
) -> Result<()> {
    ensure!(
        !scenario.withholds_keys(),
        "custody scenario cannot withhold fresh generation keys"
    );
    let missing = if scenario == QualificationScenario::MissingTargetCustody {
        first_preparation
            .committee
            .iter()
            .map(|seat| &seat.validator)
            .find(|peer| !genesis_voters.contains(*peer))
            .cloned()
    } else {
        None
    };
    ensure!(
        scenario != QualificationScenario::MissingTargetCustody || missing.is_some(),
        "missing-custody scenario requires a genuine new target process"
    );
    let first_current = genesis_current_custody(genesis_dkg, genesis_catalogs)?;
    let first = execute_rotation_preparation(
        network,
        admin,
        operators,
        first_preparation,
        &first_current,
        signed_genesis_hash,
        2,
        missing.as_ref(),
    )
    .await?;
    ensure!(
        first.target.len() == 7 && first.prepared.len() == 7 - usize::from(missing.is_some()),
        "first target custody preparation did not preserve exact seven-seat membership"
    );
    let initial_roster = network
        .validators()
        .iter()
        .map(|peer| peer.id())
        .collect::<Vec<_>>();
    let survivors = if scenario == QualificationScenario::ActivateSevenThenReturnFour {
        let survivors = first
            .target
            .iter()
            .take(4)
            .cloned()
            .collect::<BTreeSet<_>>();
        ensure!(
            survivors.len() == 4,
            "return election must retain four distinct seats"
        );
        for peer in pool.difference(&survivors) {
            let owner = operators
                .get(peer)
                .ok_or_else(|| eyre!("exiting candidate lacks a real owner"))?
                .clone();
            spawn_blocking(move || {
                owner.client.submit(
                    ExitPublicLaneValidator {
                        lane_id: LaneId::SINGLE,
                        validator: owner.account,
                        release_at_ms: u64::MAX,
                    },
                    FeePaymentIntent::authority(Vec::new(), None),
                )
            })
            .await
            .wrap_err("future candidate exit worker panicked")??;
        }
        let before_cutoff = spawn_blocking({
            let admin = admin.clone();
            move || -> Result<u64> { Ok(admin.status().get()?.blocks) }
        })
        .await
        .wrap_err("pre-cutoff height worker panicked")??;
        ensure!(
            before_cutoff < CUTOFF,
            "future exit requests must precede the E+3 selecting boundary"
        );
        Some(survivors)
    } else {
        None
    };
    advance_to_height(network, &initial_roster, CUTOFF).await?;
    let (_, cutoff_chain) = spawn_blocking({
        let admin = admin.clone();
        let network_id = network.network_id();
        move || read_contiguous_finality_chain(&admin, network_id, signed_genesis_hash, CUTOFF)
    })
    .await
    .wrap_err("first cutoff finality worker panicked")??;
    let cutoff = cutoff_chain
        .last()
        .ok_or_else(|| eyre!("first cutoff lacks authenticated finality"))?;
    verify_equal_vote_context(cutoff, genesis_voters)?;
    let snapshot = cutoff
        .commitment()
        .schedule
        .boundary
        .as_ref()
        .ok_or_else(|| eyre!("first cutoff lacks certified epoch effect"))?;
    let decision = &snapshot.next.authorization;
    ensure!(
        decision.epoch == 2
            && decision.transition_id
                == first_preparation
                    .transition_id()
                    .map_err(|error| eyre!(error))?,
        "first cutoff does not decide the exact frozen seven-seat attempt"
    );
    if let Some(withheld) = missing {
        ensure!(
            decision.decision == KagemushaMintFinalityEpochDecisionV1::RetainAndCancel
                && decision.authority_generation == 0
                && snapshot
                    .next
                    .committee
                    .iter()
                    .map(|seat| &seat.validator)
                    .collect::<BTreeSet<_>>()
                    == genesis_voters.iter().collect::<BTreeSet<_>>(),
            "one genuinely absent target custodian must force certified four-seat retention"
        );
        let progress = spawn_blocking({
            let admin = admin.clone();
            move || admin.client().get_validator_committee_status(Some(2))
        })
        .await
        .wrap_err("missing-custody terminal status worker panicked")??;
        let transition = &progress
            .selected
            .as_ref()
            .ok_or_else(|| eyre!("terminal status lost the frozen attempt"))?
            .transition;
        ensure!(
            transition.readiness.len() == 6
                && !transition.readiness.iter().any(|row| {
                    usize::try_from(row.validator_index)
                        .ok()
                        .and_then(|index| first_preparation.committee.get(index))
                        .is_some_and(|seat| seat.validator == withheld)
                })
                && transition.outcome.as_ref() == Some(decision),
            "certified retention must name exactly the missing target custody seat"
        );
        return Ok(());
    }
    ensure!(
        decision.decision == KagemushaMintFinalityEpochDecisionV1::Activate
            && decision.authority_generation == 1
            && snapshot
                .next
                .committee
                .iter()
                .map(|seat| seat.validator.clone())
                .collect::<Vec<_>>()
                == first.target,
        "complete seven-seat custody must activate as one certified boundary effect"
    );
    advance_to_height(network, &first.target, TARGET_FIRST).await?;
    let (_, activated_chain) = spawn_blocking({
        let admin = admin.clone();
        let network_id = network.network_id();
        move || {
            read_contiguous_finality_chain(&admin, network_id, signed_genesis_hash, TARGET_FIRST)
        }
    })
    .await
    .wrap_err("seven-seat activation finality worker panicked")??;
    let seven = activated_chain
        .last()
        .ok_or_else(|| eyre!("seven-seat activation lacks finality"))?;
    let seven_set = first.target.iter().cloned().collect::<BTreeSet<_>>();
    verify_equal_vote_context(seven, &seven_set)?;
    ensure!(
        seven.commitment().schedule.current.authority.generation == 1,
        "seven-seat activation did not publish the new Pasta generation"
    );
    let status = spawn_blocking({
        let admin = admin.clone();
        move || admin.client().get_validator_committee_status(Some(3))
    })
    .await
    .wrap_err("return selection status worker panicked")??;
    let return_preparation = status
        .selected
        .as_ref()
        .ok_or_else(|| eyre!("E+3 return election did not freeze"))?
        .transition
        .preparation
        .clone();
    let return_target = return_preparation
        .committee
        .iter()
        .map(|seat| seat.validator.clone())
        .collect::<BTreeSet<_>>();
    ensure!(
        return_preparation.selection_height == CUTOFF
            && return_preparation.target_epoch == 3
            && return_preparation.first_height == TARGET_LAST + 1
            && return_preparation.authority_generation == 2
            && return_target
                == survivors.ok_or_else(|| eyre!("missing requested return roster"))?
            && return_target.len() == 4,
        "genuine eight-candidate election must freeze the four non-exiting seats for E+3"
    );
    spawn_blocking({
        let operators = operators.clone();
        let processes = network
            .validators()
            .iter()
            .chain(network.committee_validators())
            .map(|peer| (peer.id(), peer.clone()))
            .collect::<BTreeMap<_, _>>();
        let return_target = return_target.clone();
        let network_id = network.network_id();
        move || publish_selected(&operators, &processes, &return_target, None, network_id, 2)
    })
    .await
    .wrap_err("return generation key worker panicked")??;
    let seven_current = prepared_current_custody(&first.prepared)?;
    let second = execute_rotation_preparation(
        network,
        admin,
        operators,
        &return_preparation,
        &seven_current,
        signed_genesis_hash,
        3,
        None,
    )
    .await?;
    ensure!(
        second.target.len() == 4 && second.prepared.len() == 4,
        "return attempt must prove all four target custodians"
    );
    let second_cutoff = TARGET_LAST;
    advance_to_height(network, &first.target, second_cutoff).await?;
    let (_, return_chain) = spawn_blocking({
        let admin = admin.clone();
        let network_id = network.network_id();
        move || {
            read_contiguous_finality_chain(&admin, network_id, signed_genesis_hash, second_cutoff)
        }
    })
    .await
    .wrap_err("return cutoff finality worker panicked")??;
    let return_boundary = return_chain
        .last()
        .ok_or_else(|| eyre!("return boundary lacks finality"))?;
    verify_equal_vote_context(return_boundary, &seven_set)?;
    let return_snapshot = return_boundary
        .commitment()
        .schedule
        .boundary
        .as_ref()
        .ok_or_else(|| eyre!("return boundary lacks certified epoch effect"))?;
    let return_decision = &return_snapshot.next.authorization;
    ensure!(
        return_decision.decision == KagemushaMintFinalityEpochDecisionV1::Activate
            && return_decision.epoch == 3
            && return_decision.authority_generation == 2
            && return_decision.transition_id
                == return_preparation
                    .transition_id()
                    .map_err(|error| eyre!(error))?
            && return_snapshot
                .next
                .committee
                .iter()
                .map(|seat| seat.validator.clone())
                .collect::<Vec<_>>()
                == second.target,
        "seven-seat exact quorum did not certify the independently prepared four-seat return"
    );
    let four_first = second_cutoff + 1;
    advance_to_height(network, &second.target, four_first).await?;
    let (_, four_chain) = spawn_blocking({
        let admin = admin.clone();
        let network_id = network.network_id();
        move || read_contiguous_finality_chain(&admin, network_id, signed_genesis_hash, four_first)
    })
    .await
    .wrap_err("four-seat return finality worker panicked")??;
    let four = four_chain
        .last()
        .ok_or_else(|| eyre!("four-seat return lacks finality"))?;
    verify_equal_vote_context(four, &return_target)?;
    ensure!(
        four.commitment().schedule.current.authority.generation == 2,
        "4→7→4 did not complete the second authenticated signing generation"
    );
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QualificationScenario {
    WithheldIncumbentKeys,
    FiveCandidateRetention,
    ElevenCandidateRetention,
    MissingTargetCustody,
    ActivateSevenThenReturnFour,
}

impl QualificationScenario {
    fn pool_size(self) -> usize {
        match self {
            Self::FiveCandidateRetention => 5,
            Self::ElevenCandidateRetention => 11,
            _ => 8,
        }
    }

    fn withholds_keys(self) -> bool {
        matches!(
            self,
            Self::WithheldIncumbentKeys
                | Self::FiveCandidateRetention
                | Self::ElevenCandidateRetention
        )
    }

    fn selected_seats(self) -> usize {
        let available = self.pool_size().min(7);
        3 * ((available - 1) / 3) + 1
    }

    fn seed(self) -> &'static str {
        match self {
            Self::WithheldIncumbentKeys => "npos-eight-candidates-withheld-keys",
            Self::FiveCandidateRetention => "npos-five-candidates-retention",
            Self::ElevenCandidateRetention => "npos-eleven-candidates-retention",
            Self::MissingTargetCustody => "npos-eight-candidates-missing-custody",
            Self::ActivateSevenThenReturnFour => "npos-eight-candidates-seven-then-four",
        }
    }
}

async fn run_overfull_qualification(scenario: QualificationScenario) -> Result<()> {
    init_instruction_registry();
    let pool_size = scenario.pool_size();
    let seats = scenario.selected_seats();
    let supplementary = pool_size - 4;
    let registry_capacity = i64::try_from(pool_size.max(7))?;
    let xor: AssetDefinitionId = defaults::nexus::staking::stake_asset_id().parse()?;
    ensure!(
        xor.to_string() == TAIRA_XOR,
        "staking must use real genesis-pinned XOR"
    );
    let mut npos = SumeragiNposParameters::default();
    npos.epoch_length_blocks = NonZeroU64::new(EPOCH).expect("nonzero epoch");
    npos.max_validators = 7;
    npos.min_self_bond = 1_000_u64.into();
    npos.evidence_horizon_blocks = EPOCH * 2;
    npos.slashing_delay_blocks = 1;
    npos.validate().map_err(|error| eyre!(error))?;
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_max_validator_capacity(7)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus()
        .with_disposable_mint_finality_custody()
        .with_npos_genesis_bootstrap(1_000_u64.into())
        .with_committee_validator_p2p_bootstrap(CommitteeValidatorP2pBootstrap::new(
            supplementary,
        )?)?
        .with_config_layer(|layer| {
            // Admission may retain a larger candidate pool than the seven-seat
            // consensus ceiling; the authenticated election selects exactly 3f+1.
            layer.write(["nexus", "staking", "max_validators"], registry_capacity);
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos.into_custom_parameter(),
        )));
    let network = sandbox::build_network_or_skip(builder, scenario.seed()).ok_or_else(|| {
        eyre!("committee qualification requires an actual {pool_size}-process disposable network")
    })?;
    let result = async {
        let genesis_bundle = network.native_genesis_provisioning_bundle()?;
        ensure!(
            genesis_bundle.block_hash == network.genesis().0.hash(),
            "genesis provisioners must consume the retained signed manifest and block"
        );
        network.start_all().await?;
        network.ensure_blocks(1).await?;
        let admin = rebind_blocking_client(&network.client(), |builder| {
            builder.transaction_status_timeout = WAIT;
        });
        let network_id = network.network_id();
        let block_hash = genesis_bundle.block_hash;
        let network_ref = &network;
        let genesis_dkg = run_disposable_genesis_dkg(
            &network,
            finality_limits(),
            5,
            |height| {
                let admin = admin.clone();
                let genesis = genesis.clone();
                let chain_id = chain_id.clone();
                async move {
                    advance_exact_genesis_phase(network_ref, height).await?;
                    let observed = spawn_blocking({
                        let admin = admin.clone();
                        move || -> Result<u64> { Ok(admin.status().get()?.blocks) }
                    })
                    .await
                    .wrap_err("genesis phase status worker panicked")??;
                    ensure!(
                        observed == height,
                        "genesis DKG public phase missed exact h{height} observation"
                    );
                    spawn_blocking(move || {
                        read_genesis_dkg_finality_chain(&admin, network_id, block_hash, height)
                    })
                    .await
                    .wrap_err("genesis phase finality worker panicked")?
                }
            },
        )
        .await?;
        let install: Vec<InstructionBox> =
            norito::json::from_slice(&fs::read(&genesis_dkg.install_instruction_path)?)?;
        ensure!(install.len() == 1, "native genesis install must emit one instruction");
        let next_height = spawn_blocking({
            let admin = admin.clone();
            move || -> Result<u64> { Ok(admin.status().get()?.blocks + 1) }
        })
        .await
        .wrap_err("genesis install height worker panicked")??;
        ensure!(
            next_height == 5,
            "genesis installation must execute immediately after the exact h4 DKG cutoff"
        );
        spawn_blocking({
            let admin = admin.clone();
            move || admin.submit_all(install, FeePaymentIntent::authority(Vec::new(), None))
        })
        .await
        .wrap_err("genesis beacon install worker panicked")??;
        network.shutdown().await;
        let genesis_catalogs = stage_genesis_brokers(&network, &genesis_dkg).await?;
        network.start_all().await?;
        network.ensure_blocks(5).await?;
        ensure!(
            network.validators().len() == 4 && network.committee_validators().len() == supplementary,
            "fixture must start four genesis voters and the exact separate candidate pool"
        );
        let network_id = network.network_id();
        let genesis_voters = network
            .validators()
            .iter()
            .map(|peer| peer.id())
            .collect::<BTreeSet<_>>();
        let pool = network
            .validators()
            .iter()
            .chain(network.committee_validators())
            .map(|peer| peer.id())
            .collect::<BTreeSet<_>>();
        ensure!(pool.len() == pool_size, "all candidate BLS keys must be distinct");
        let pasta = network
            .validators()
            .iter()
            .chain(network.committee_validators())
            .map(|peer| peer.disposable_mint_finality_keys(0))
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            pasta.iter().map(|keys| keys.eq_proof_public_key).collect::<BTreeSet<_>>().len() == pool_size
                && pasta
                    .iter()
                    .map(|keys| keys.ep_proof_public_key)
                    .collect::<BTreeSet<_>>()
                    .len()
                    == pool_size,
            "all processes must hold independent real Pasta signing custody"
        );
        let escrow = validator_xor_escrow(&network.genesis(), &xor)?;
        ensure!(escrow.definition == xor, "genesis escrow must hold actual XOR");
        let operators = network
            .validators()
            .iter()
            .chain(network.committee_validators())
            .map(|peer| {
                let account = peer.account_id();
                let client = rebind_blocking_client(
                    &network.validators()[0].client_for(
                        &account,
                        peer.streaming_key_pair().private_key().clone(),
                    ),
                    |builder| builder.transaction_status_timeout = WAIT,
                );
                Ok((
                    peer.id(),
                    Operator {
                        account,
                        peer: peer.id(),
                        consensus_key: peer.bls_key_pair().ok_or_else(|| eyre!("missing real BLS candidate key"))?.clone(),
                        pop: peer.bls_pop().ok_or_else(|| eyre!("missing real BLS possession proof"))?.to_vec(),
                        client,
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let fresh_peers = network
            .committee_validators()
            .iter()
            .map(|peer| peer.id())
            .collect::<Vec<_>>();
        let candidates = fresh_peers
            .iter()
            .map(|peer| operators.get(peer).ok_or_else(|| eyre!("missing candidate operator")))
            .collect::<Result<Vec<_>>>()?;
        let initial_height = spawn_blocking({
            let admin = admin.clone();
            let xor = xor.clone();
            let escrow = escrow.clone();
            let candidates = candidates
                .iter()
                .map(|operator| Operator::clone(operator))
                .collect::<Vec<_>>();
            move || -> Result<u64> {
                admit_candidates(&admin, &candidates, network_id, &xor, &escrow)?;
                Ok(admin.status().get()?.blocks)
            }
        })
        .await
        .wrap_err("candidate admission worker panicked")??;
        ensure!(initial_height < SELECTION, "candidate pool did not enter the selecting prestate");
        let initial_roster = network.validators().iter().map(|peer| peer.id()).collect::<Vec<_>>();
        advance_to_height(&network, &initial_roster, SELECTION).await?;
        let before = spawn_blocking({
            let admin = admin.clone();
            move || admin.client().get_validator_committee_status(Some(2))
        })
        .await
        .wrap_err("selection status worker panicked")??;
        let selected = before.selected.as_ref().ok_or_else(|| eyre!("boundary did not freeze E+2"))?;
        let preparation = selected.transition.preparation.clone();
        preparation.validate().map_err(|error| eyre!(error))?;
        let target = preparation
            .committee
            .iter()
            .map(|seat| seat.validator.clone())
            .collect::<BTreeSet<_>>();
        ensure!(
            preparation.network_id == network_id
                && preparation.selection_epoch == 0
                && preparation.selection_height == SELECTION
                && preparation.target_epoch == 2
                && preparation.first_height == TARGET_FIRST
                && preparation.last_height == TARGET_LAST
                && preparation.authority_generation == 1
                && target.len() == seats
                && target == ranked_target(&preparation, &pool, seats)
                && target.is_subset(&pool),
            "selection must freeze the exact public rank and 3f+1 size for the full candidate pool"
        );
        ensure!(
            target.intersection(&genesis_voters).count() >= seats.saturating_sub(supplementary),
            "selected incumbent overlap must satisfy the actual candidate pool geometry"
        );
        for seat in &preparation.committee {
            ensure!(
                operators.get(&seat.validator).is_some_and(|operator| operator.pop == seat.proof_of_possession),
                "frozen seat must carry its real BLS possession proof"
            );
        }
        let withheld = if scenario.withholds_keys() {
            Some(
                target
                    .intersection(&genesis_voters)
                    .next()
                    .or_else(|| {
                        (scenario != QualificationScenario::WithheldIncumbentKeys)
                            .then(|| target.iter().next())
                            .flatten()
                    })
                    .cloned()
                    .ok_or_else(|| eyre!("selected target lacks the required withholding seat"))?,
            )
        } else {
            None
        };
        let selected_id = preparation.transition_id().map_err(|error| eyre!(error))?;
        spawn_blocking({
            let target = target.clone();
            let withheld = withheld.clone();
            let operators = operators.clone();
            let processes = network
                .validators()
                .iter()
                .chain(network.committee_validators())
                .map(|peer| (peer.id(), peer.clone()))
                .collect::<BTreeMap<_, _>>();
            move || publish_selected(
                &operators,
                &processes,
                &target,
                withheld.as_ref(),
                network_id,
                1,
            )
        })
        .await
        .wrap_err("candidate key publication worker panicked")??;
        let progress = spawn_blocking({
            let admin = admin.clone();
            move || admin.client().get_validator_committee_status(Some(2))
        })
        .await
        .wrap_err("preparation status worker panicked")??;
        ensure!(
            progress.candidate_keys.len() == seats - usize::from(withheld.is_some())
                && progress.selected.as_ref().is_some_and(|row| row.transition.preparation == preparation),
            "selected key publications differ from the exact frozen committee"
        );
        if let Some(withheld) = &withheld {
            ensure!(
                progress.candidate_keys.iter().all(|row| row.keys.validator != *withheld),
                "the selected withholding seat must genuinely lack its fresh generation keys"
            );
        } else {
            return run_custody_or_activation_scenario(
                scenario,
                &network,
                &admin,
                &genesis_dkg,
                &genesis_catalogs,
                &operators,
                &genesis_voters,
                &pool,
                &preparation,
                genesis_bundle.block_hash,
            )
            .await;
        }
        advance_to_height(&network, &initial_roster, CUTOFF).await?;
        let (selection_proof, cutoff_proof) = spawn_blocking({
            let admin = admin.clone();
            let voters = genesis_voters.clone();
            move || read_finality_chain(&admin, network_id, &voters, CUTOFF)
        })
        .await
        .wrap_err("boundary finality worker panicked")??;
        verify_equal_vote_context(&selection_proof, &genesis_voters)?;
        verify_equal_vote_context(&cutoff_proof, &genesis_voters)?;
        ensure!(
            NativeFinalityArtifact::from_block(selection_proof.block(), finality_limits()).map_err(|error| eyre!(error))? == selected.selecting_finality,
            "committee status must attach the exact authenticated selecting certificate"
        );
        let selected_snapshot = selection_proof
            .commitment().schedule.boundary
            .as_ref()
            .ok_or_else(|| eyre!("selection boundary lacks a finalized next-epoch snapshot"))?;
        ensure!(
            selected_snapshot.preparation.as_ref() == Some(&preparation),
            "the complete selected preparation must be frozen in the incumbent boundary QC"
        );
        let cutoff_snapshot = cutoff_proof
            .commitment().schedule.boundary
            .as_ref()
            .ok_or_else(|| eyre!("cutoff lacks an incumbent-certified next epoch"))?;
        let authorization = &cutoff_snapshot.next.authorization;
        ensure!(
            authorization.decision == KagemushaMintFinalityEpochDecisionV1::RetainAndCancel
                && authorization.epoch == 2
                && authorization.authority_generation == 0
                && authorization.transition_id == selected_id
                && cutoff_snapshot.next.authority
                    == selection_proof.commitment().schedule.current.authority
                && cutoff_snapshot.next.committee == selection_proof.commitment().schedule.current.committee,
            "a missing target key must cancel this exact attempt while retaining all four incumbent seats"
        );
        let replacement = cutoff_snapshot
            .preparation
            .as_ref()
            .ok_or_else(|| eyre!("next selection must create a new E+3 attempt"))?;
        ensure!(
            replacement.target_epoch == 3
                && replacement.selection_height == CUTOFF
                && replacement.transition_id().map_err(|error| eyre!(error))? != selected_id
                && replacement.beacon_session_id().map_err(|error| eyre!(error))?
                    != preparation.beacon_session_id().map_err(|error| eyre!(error))?,
            "cancelled preparation cannot be shrunk or reused as the next attempt"
        );
        let terminal = spawn_blocking({
            let admin = admin.clone();
            move || admin.client().get_validator_committee_status(Some(2))
        })
        .await
        .wrap_err("terminal committee status worker panicked")??;
        ensure!(
            terminal.selected.as_ref().is_some_and(|row| {
                row.transition.preparation == preparation
                    && row.transition.outcome.as_ref() == Some(authorization)
            }),
            "retained transition outcome must match its finality-certified body"
        );
        Ok(())
    }
    .await;
    network.shutdown_and_release().await;
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn overfull_pool_freezes_seven_then_missing_incumbent_keys_certify_four_retained()
-> Result<()> {
    run_overfull_qualification(QualificationScenario::WithheldIncumbentKeys).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn five_candidates_freeze_exact_four_and_certify_retention_without_reroll() -> Result<()> {
    run_overfull_qualification(QualificationScenario::FiveCandidateRetention).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn eleven_candidates_freeze_exact_seven_and_certify_retention_without_reroll() -> Result<()> {
    run_overfull_qualification(QualificationScenario::ElevenCandidateRetention).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn complete_keys_and_dkg_but_missing_target_custody_certifies_four_retained() -> Result<()> {
    run_overfull_qualification(QualificationScenario::MissingTargetCustody).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn complete_real_xor_committee_rotates_four_to_seven_to_four() -> Result<()> {
    run_overfull_qualification(QualificationScenario::ActivateSevenThenReturnFour).await
}

#[test]
fn exact_quorum_uses_only_three_f_plus_one_equal_vote_geometry() {
    assert_eq!(exact_quorum(4).unwrap(), 3);
    assert_eq!(exact_quorum(7).unwrap(), 5);
    assert!(exact_quorum(6).is_err());
    assert!(exact_quorum(8).is_err());
}

#[test]
fn native_finality_rejects_wrong_independent_genesis_before_query() {
    let config = iroha::config::Config {
        chain: "committee-transition-unit".into(),
        network_id: NetworkId::from_genesis_hash(iroha::crypto::HashOf::from_untyped_unchecked(
            Hash::prehashed([0x5A; Hash::LENGTH]),
        )),
        key_pair: iroha_test_samples::ALICE_KEYPAIR.clone(),
        account: ALICE_ID.clone(),
        account_chain_discriminant: iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT,
        torii_api_url: "http://committee-transition.invalid/".parse().unwrap(),
        torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
        basic_auth: None,
        transaction_add_nonce: false,
        transaction_ttl: Duration::from_secs(5),
        transaction_status_timeout: Duration::from_secs(10),
        sorafs_alias_cache: iroha::config::AliasCache::default().into_policy(),
        sorafs_anonymity_policy: iroha_service_model::soranet::AnonymityPolicy::default(),
        sorafs_rollout_phase: iroha_service_model::soranet::RolloutPhase::default(),
    };
    let network_id = config.network_id;
    let client = Client::new(config).unwrap();
    let error = read_contiguous_finality_chain(
        &client,
        network_id,
        iroha::crypto::HashOf::from_untyped_unchecked(Hash::new(b"another signed genesis")),
        2,
    )
    .expect_err("foreign independent source is rejected before any query");
    assert!(
        error
            .to_string()
            .contains("network differs from independent signed genesis")
    );
}
