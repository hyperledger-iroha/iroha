#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Four-validator modern SORA Parliament and mandatory timed-OVN lifecycle corridor.

use iroha::query::QueryError;
use iroha_data_model::sumeragi::PROTOCOL_VERSION;
use iroha_data_model::{
    ValidationFail,
    query::error::{FindError, QueryExecutionFail},
};
use iroha_sumeragi::availability::recommended_data_availability_layout;
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    str::FromStr as _,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use eyre::{Result, WrapErr as _, eyre};
use integration_tests::sandbox;
use iroha::{
    blocking::Client,
    client::{
        AccountTransactionDraft, FeeQuoteRequest, ParliamentTimedOvnCastingContextResponseV1,
        ParliamentTlePartialReleaseShareV1, ParliamentTleReleaseContextResponseV1,
    },
    crypto::{Algorithm, Hash, KeyPair, Signature},
    data_model::{
        account::AccountId,
        block::SignedBlock,
        governance::types::{
            AbiVersion, BallotAttemptId, BallotAttemptStatusV1, BeaconPulseId, BeaconSessionId,
            BodyElectionAttemptId, BodyInstanceId, BodyInstanceStatusV1, ContractAbiHash,
            ContractCodeHash, DeliberationPhaseV1, DeployContractProposal, GovernanceAttemptId,
            GovernanceAttemptStatusV1, GovernanceStageV1, ParliamentAggregateOutcomeV1,
            ParliamentBody, ParliamentNoResultKindV1, ProposalKind, SortitionRequestV1,
            TleSessionId, parliament_ballot_participant_hash_v1, parliament_candidate_root_v1,
        },
        isi::{
            InstructionBox, Log,
            consensus_keys::{
                ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleActionV1,
                ThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleSignatureV1,
            },
            governance::{
                CreateParliamentGovernanceAttemptV1, ParliamentAdvanceBodyPhaseV1,
                ParliamentBeginBallotOpeningBatchV1, ParliamentBeginInvitationAcceptanceV1,
                ParliamentCloseBallotRegistrationV1, ParliamentConsumeSortitionPulseBatchV1,
                ParliamentEndorsePublicFindingV1, ParliamentFailPublicFindingNoResultV1,
                ParliamentFinalizeOpenedBallotV1, ParliamentFreezeBallotSurvivorsV1,
                ParliamentFreezeTimedOvnCorpusV1, ParliamentInvitationDecisionV1,
                ParliamentLifecycleTransitionV1, ParliamentRecordAttemptAbsenceV1,
                ParliamentRecordInvitationResponseV1, ParliamentRegisterBallotAttemptV1,
                ParliamentRegisterBallotParticipantV1, ParliamentRegisterSortitionRequestV1,
                ParliamentSealBodyRosterV1, ParliamentSortitionRequestRegistrationV1,
                ParliamentTleFinalReleaseSignatureV1, ProposeDeployContract, RegisterCitizen,
                SubmitParliamentLifecycleTransitionV1,
            },
            smart_contract_code::{
                FinalizeSmartContractCodeUpload, RegisterSmartContractCode,
                SMART_CONTRACT_CODE_CHUNK_BYTES, UploadSmartContractCodeChunk,
            },
        },
        parameter::{
            Parameter,
            system::{
                ConsensusHandshakeMetadata, SumeragiConsensusMode, SumeragiNposParameters,
                consensus_metadata,
            },
        },
        permission::Permission,
        prelude::{
            Account, AssetId, FeePaymentIntent, FindAssetById, FindBlocks, Grant,
            Identifiable as _, Level, QueryBuilderExt as _, Register, SetParameter,
            SignedTransaction,
        },
        query::dsl::IntoPredicate as _,
        smart_contract::ContractAddress,
        sumeragi::SumeragiStatus,
    },
};
use iroha_core::{
    beacon::{
        GlobalThresholdBeaconSessionBindingV1, global_threshold_beacon_governance_seed_v1,
        global_threshold_beacon_npos_successor_seed_v1, global_threshold_beacon_roster_hash_v1,
        parliament_test_network_signer::deterministic_parliament_beacon_key_record_v1,
        validate_global_threshold_beacon_session_v1,
        verify_finalized_global_threshold_beacon_pulse_v1,
    },
    governance::{
        parliament::ParliamentAttemptStateV1,
        timed_ovn::{TIMED_OVN_BALLOT_RECORD_BYTES_V1, TimedOvnReleaseIdentityPublicV1},
    },
    state::{
        THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
        threshold_key_lifecycle_certificate_preimage_v1,
        verify_threshold_key_lifecycle_certificate_v1,
    },
    tle_release::{
        AuthorizedTleReleaseProjectionV1,
        PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_ARCHIVE_MAX_BYTES_V1,
        ParliamentTimedOvnCastingContextArchiveV1, ParliamentTimedOvnCastingPhaseV1,
        TLE_AUTHORIZED_RELEASE_IDENTITY_PAYLOAD_BYTES_V1,
        TLE_AUTHORIZED_RELEASE_PROJECTION_VERSION_V1, TleAdaptiveDealerCommitmentV1,
        TleAdaptivePublicShareV1, TleKeySessionPublicStateV1, TlePartialReleaseShareV1,
        parliament_test_network_signer::deterministic_parliament_tle_key_public_state_v1,
    },
};
use iroha_crypto::timed_ovn::{TimedOvnChoiceV1, TimedOvnRegistrationSecretV1};
use iroha_executor_data_model::permission::{
    governance::CanProposeContractDeployment, smart_contract::CanManageSmartContractCode,
};
use iroha_model_base::{metadata::Metadata, peer::PeerId};
use iroha_test_network::{NetworkBuilder, ParliamentBeaconSignerMode, read_on_dedicated_thread};
use iroha_test_samples::ALICE_ID;
use norito::codec::Encode as _;
use rand::{SeedableRng as _, rngs::StdRng};

#[path = "sora_parliament_lifecycle_support.rs"]
mod support;
use support::*;

#[test]
fn four_validator_policy_jury_uses_future_pulses_and_mandatory_timed_ovn() -> Result<()> {
    let name = stringify!(four_validator_policy_jury_uses_future_pulses_and_mandatory_timed_ovn);
    let handle = std::thread::Builder::new()
        .name(name.to_owned())
        .stack_size(PARLIAMENT_NETWORK_STACK_BYTES)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .thread_stack_size(PARLIAMENT_NETWORK_STACK_BYTES)
                .enable_all()
                .build()
                .expect("build four-validator Parliament test runtime")
                .block_on(
                    four_validator_policy_jury_uses_future_pulses_and_mandatory_timed_ovn_impl(),
                )
        })
        .expect("spawn four-validator Parliament test thread");
    match handle.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

async fn four_validator_policy_jury_uses_future_pulses_and_mandatory_timed_ovn_impl() -> Result<()>
{
    let citizen_keys = citizen_keys();
    let citizens = citizen_accounts(&citizen_keys);
    let contract_address = ContractAddress::from_str(CONTRACT_ADDRESS)?;
    let no_result_retry_contract_address =
        ContractAddress::from_str(NO_RESULT_RETRY_CONTRACT_ADDRESS)?;
    let builder = enactment::builder(
        NetworkBuilder::new()
            .with_peers(VALIDATOR_COUNT)
            .with_auto_populated_trusted_peers(),
    )
    .with_genesis_instruction(Grant::account_permission(
        Permission::from(CanManageSmartContractCode),
        ALICE_ID.clone(),
    ))
    .with_genesis_instruction(Grant::account_permission(
        Permission::from(CanProposeContractDeployment {
            contract_address: contract_address.clone(),
        }),
        ALICE_ID.clone(),
    ))
    .with_genesis_instruction(Grant::account_permission(
        Permission::from(CanProposeContractDeployment {
            contract_address: no_result_retry_contract_address.clone(),
        }),
        ALICE_ID.clone(),
    ));

    let context = stringify!(four_validator_policy_jury_uses_future_pulses_and_mandatory_timed_ovn);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    assert_eq!(network.peers().len(), VALIDATOR_COUNT);
    let handshake = signed_consensus_handshake(&network)?;
    handshake
        .validate()
        .map_err(|error| eyre!("signed consensus handshake is invalid: {error}"))?;
    assert_eq!(handshake.mode, SumeragiConsensusMode::Npos);
    assert_eq!(
        handshake.wire_protocol_version,
        u32::from(PROTOCOL_VERSION),
        "the signed genesis handshake must select consensus revision 4",
    );
    assert_eq!(
        handshake.sumeragi_context.da_layout,
        recommended_data_availability_layout(),
        "the corridor must retain the signed revision-4 RS16 DA layout",
    );
    network.ensure_blocks(1).await?;
    let client = network.client();
    let ordered_roster = ordered_validator_roster(&network, &client).await?;
    let beacon_record =
        deterministic_parliament_beacon_key_record_v1(network.network_id(), &ordered_roster)
            .wrap_err("derive exact public beacon fixture")?;
    let beacon_binding = GlobalThresholdBeaconSessionBindingV1 {
        network_id: beacon_record.session.network_id,
        session_id: beacon_record.session.session_id,
        roster_hash: beacon_record.session.roster_hash,
        transcript_hash: beacon_record.session.transcript_hash,
    };
    let validated_beacon_session =
        validate_global_threshold_beacon_session_v1(beacon_record.session.clone(), &beacon_binding)
            .wrap_err("replay the exact public beacon transcript")?;
    let tle_public_state =
        deterministic_parliament_tle_key_public_state_v1(network.network_id(), &ordered_roster)
            .wrap_err("derive exact public TLE fixture")?;
    let install_height = next_execution_height(
        &client,
        beacon_record.session.adaptive_dkg.finalized_at_height,
        "threshold-key installation",
    )
    .await?;
    let lifecycle_certificates = [
        InstructionBox::from(lifecycle_certificate(
            &network,
            &ordered_roster,
            ThresholdKeyLifecycleActionV1::FinalizeGlobalBeaconKey,
            beacon_record.session.session_id,
            beacon_record.session.transcript_hash,
            norito::encode_canonical(&beacon_record)?,
            install_height,
        )?),
        InstructionBox::from(lifecycle_certificate(
            &network,
            &ordered_roster,
            ThresholdKeyLifecycleActionV1::InstallParliamentTleKey,
            *tle_public_state.key_session_id.as_bytes(),
            tle_public_state.transcript_hash,
            norito::encode_canonical(&tle_public_state)?,
            install_height,
        )?),
    ];
    let submission_authority_height = current_height(&client).await?;
    let submission_roster = ordered_validator_roster(&network, &client).await?;
    if submission_roster != ordered_roster {
        return Err(eyre!(
            "threshold-key installation roster changed between fixture derivation and submission"
        ));
    }
    eprintln!(
        "SORA_PARLIAMENT_LIFECYCLE submit_lifecycle authority_height={submission_authority_height} install_height={install_height} roster_hash={}",
        hex::encode(global_threshold_beacon_roster_hash_v1(&ordered_roster)),
    );
    submit_parliament_instructions(&client, lifecycle_certificates).await?;
    assert_eq!(current_height(&client).await?, install_height);
    let activation_height = install_height
        .checked_add(1)
        .ok_or_else(|| eyre!("threshold-key activation height overflow"))?;
    admit_parliament_height_carrier(
        &client,
        [Log::new(
            Level::INFO,
            "carry Parliament threshold-key activation".to_owned(),
        )],
    )
    .await?;
    network.ensure_blocks(activation_height).await?;
    assert_eq!(current_height(&client).await?, activation_height);

    let (code_hash, abi_hash) =
        stage_contract_artifact(&client, &minimal_contract_artifact()).await?;
    let proposal = ProposalKind::DeployContract(DeployContractProposal {
        proposal_operator: client.client().account().clone(),
        contract_address: contract_address.clone(),
        code_hash,
        abi_hash,
        abi_version: AbiVersion::new(1),
        manifest_provenance: None,
    });
    let enacted_fixture = enactment::enact(
        &network,
        proposal,
        vec![
            ProposeDeployContract {
                contract_address: contract_address.clone(),
                code_hash,
                abi_hash,
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            }
            .into(),
        ],
        None,
    )
    .await?;
    let attempt_id = enacted_fixture.attempt_id;
    let enacted_height = enacted_fixture.height;
    let logical_beacon = enacted_fixture.logical_beacon;
    let enacted_response = read_on_dedicated_thread({
        let client = client.client().clone();
        move || client.get_parliament_attempt(attempt_id)
    })
    .await?;
    assert_governed_contract_binding(
        &client,
        &contract_address,
        code_hash,
        abi_hash,
        "consensus-owned certificate enactment must bind the staged contract",
    )
    .await?;

    let mut peer_blocks = Vec::with_capacity(network.peers().len());
    for peer in network.peers() {
        peer_blocks.push(exact_block(&peer.client(), enacted_height).await?);
    }
    assert!(
        peer_blocks
            .windows(2)
            .all(|pair| pair[0].hash() == pair[1].hash()),
        "all four validators must finalize the same exact enactment block",
    );
    let enacted_height_nonzero =
        NonZeroU64::new(enacted_height).expect("a Parliament enactment cannot be genesis");
    for (peer, block) in network.peers().iter().zip(&peer_blocks) {
        let (proof, verified_hash) = read_on_dedicated_thread({
            let client = peer.client().client().clone();
            let height = (enacted_height_nonzero).clone();
            let network_id = (network.network_id()).clone();
            move || client.get_bridge_finality_anchor(height, network_id)
        })
        .await
        .wrap_err("independently verify the enactment block's revision-4 finality")?;
        let artifact = &proof.finality_artifact;
        assert_eq!(verified_hash, block.hash());
        assert_eq!(artifact.height, enacted_height);
        assert_eq!(artifact.height_context.roster.len(), VALIDATOR_COUNT);
        assert_eq!(artifact.height_context.quorum.min_signers, 3);
        assert_eq!(artifact.height_context.quorum.total_power, 4);
        assert_eq!(artifact.commit_qc.signers.len(), 3);
        assert!(
            artifact
                .height_context
                .roster
                .iter()
                .all(|entry| entry.power == 1),
            "each signed-genesis validator must retain exactly one vote",
        );
        let mut proof_roster = artifact
            .height_context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        proof_roster.sort_unstable();
        let mut signed_genesis_roster = ordered_roster.clone();
        signed_genesis_roster.sort_unstable();
        assert_eq!(
            proof_roster, signed_genesis_roster,
            "the revision-4 proof roster must equal the signed-genesis voting authority",
        );
        assert_eq!(
            artifact.height_context.da_layout,
            recommended_data_availability_layout(),
            "every enactment proof must retain the signed revision-4 RS16 DA layout",
        );
    }
    for peer in network.peers() {
        let peer_client = peer.client();
        let response = read_on_dedicated_thread({
            let client = peer_client.client().clone();
            let attempt_id = (attempt_id).clone();
            move || client.get_parliament_attempt(attempt_id)
        })
        .await?;
        assert_eq!(response.current_height, enacted_height);
        assert_eq!(response.attempt.status, GovernanceAttemptStatusV1::Enacted);
        assert_eq!(
            response.state_payload_hex,
            enacted_response.state_payload_hex
        );
        assert_governed_contract_binding(
            &peer_client,
            &contract_address,
            code_hash,
            abi_hash,
            "every validator must expose the consensus-enacted contract",
        )
        .await?;
        let status = read_on_dedicated_thread({
            let client = peer_client.client().clone();
            move || client.get_sumeragi_status()
        })
        .await?;
        assert!(
            !status.is_halted(),
            "an enacted Parliament validator must not be live-but-fail-stopped",
        );
    }

    let restart_index = network.peers().len() - 1;
    let restart_peer = network.peers()[restart_index].clone();
    let config_layers = network.config_layers().collect::<Vec<_>>();
    assert!(
        restart_peer.shutdown_if_started().await,
        "selected validator must be running before the persistence restart",
    );
    tokio::time::timeout(
        network.peer_startup_timeout(),
        restart_peer.start_checked(config_layers.iter(), None),
    )
    .await
    .map_err(|_| eyre!("Parliament validator restart exceeded {OPERATION_TIMEOUT:?}"))??;
    tokio::time::timeout(
        network.sync_timeout(),
        restart_peer.once_block(enacted_height),
    )
    .await
    .map_err(|_| eyre!("restarted Parliament validator did not recover finalized state"))?;
    let restarted_response = read_on_dedicated_thread({
        let client = restart_peer.client().client().clone();
        let attempt_id = (attempt_id).clone();
        move || client.get_parliament_attempt(attempt_id)
    })
    .await?;
    assert_eq!(
        restarted_response.attempt.status,
        GovernanceAttemptStatusV1::Enacted
    );
    assert_eq!(
        restarted_response.state_payload_hex, enacted_response.state_payload_hex,
        "normal restart must restore the complete reducer/certificate state",
    );
    let restarted_block = exact_block(&restart_peer.client(), enacted_height).await?;
    assert_eq!(restarted_block.hash(), peer_blocks[0].hash());
    assert_governed_contract_binding(
        &restart_peer.client(),
        &contract_address,
        code_hash,
        abi_hash,
        "normal restart must restore the consensus-enacted contract",
    )
    .await?;
    let restarted_status = read_on_dedicated_thread({
        let client = restart_peer.client().client().clone();
        move || client.get_sumeragi_status()
    })
    .await?;
    assert!(
        !restarted_status.is_halted(),
        "normal restart must restore a live non-fail-stopped consensus reducer",
    );
    no_result_paths::exercise_public_finding_no_result_retries_and_restore(
        &network,
        &client,
        &citizens,
        &citizen_keys,
        &no_result_retry_contract_address,
        code_hash,
        abi_hash,
        logical_beacon,
    )
    .await?;
    Ok(())
}

#[test]
fn four_validator_mandatory_npos_epoch_boundary_threshold_beacon_release_gate() -> Result<()> {
    let name =
        stringify!(four_validator_mandatory_npos_epoch_boundary_threshold_beacon_release_gate);
    let handle = std::thread::Builder::new()
        .name(name.to_owned())
        .stack_size(PARLIAMENT_NETWORK_STACK_BYTES)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .thread_stack_size(PARLIAMENT_NETWORK_STACK_BYTES)
                .enable_all()
                .build()
                .expect("build four-validator mandatory-beacon test runtime")
                .block_on(
                    four_validator_mandatory_npos_epoch_boundary_threshold_beacon_release_gate_impl(
                    ),
                )
        })
        .expect("spawn four-validator mandatory-beacon test thread");
    match handle.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

async fn four_validator_mandatory_npos_epoch_boundary_threshold_beacon_release_gate_impl()
-> Result<()> {
    let mut npos = SumeragiNposParameters::default();
    npos.epoch_length_blocks = NonZeroU64::new(MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS)
        .expect("mandatory NPoS epoch length is non-zero");
    npos.evidence_horizon_blocks = MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS * 2;
    npos.slashing_delay_blocks = MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS;
    npos.validate()
        .map_err(|error| eyre!("invalid mandatory NPoS fixture: {error}"))?;

    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus()
        .with_parliament_beacon_signer_modes(POSITIVE_BEACON_SIGNER_MODES)
        .with_block_cadence(EXACT_HEIGHT_SUBMISSION_CADENCE)
        .with_config_layer(|layer| {
            layer
                .write(
                    ["concurrency", "rayon_global_threads"],
                    PARLIAMENT_NETWORK_RAYON_THREADS_PER_PEER,
                )
                .write(
                    [
                        "network",
                        "soranet_handshake",
                        "pow",
                        "puzzle",
                        "memory_kib",
                    ],
                    i64::from(iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB),
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "time_cost"],
                    1_i64,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "lanes"],
                    1_i64,
                )
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos.into_custom_parameter(),
        )));
    let context =
        stringify!(four_validator_mandatory_npos_epoch_boundary_threshold_beacon_release_gate);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    assert_eq!(network.peers().len(), VALIDATOR_COUNT);
    network.ensure_blocks(1).await?;

    let client = network.client();
    let ordered_roster = ordered_validator_roster(&network, &client).await?;
    let beacon_record =
        deterministic_parliament_beacon_key_record_v1(network.network_id(), &ordered_roster)
            .wrap_err("derive mandatory NPoS beacon fixture")?;
    assert_eq!(beacon_record.session.committee_size, 4);
    assert_eq!(beacon_record.session.threshold, 2);
    let beacon_binding = GlobalThresholdBeaconSessionBindingV1 {
        network_id: beacon_record.session.network_id,
        session_id: beacon_record.session.session_id,
        roster_hash: beacon_record.session.roster_hash,
        transcript_hash: beacon_record.session.transcript_hash,
    };
    let validated_beacon_session =
        validate_global_threshold_beacon_session_v1(beacon_record.session.clone(), &beacon_binding)
            .wrap_err("replay mandatory NPoS beacon transcript")?;
    let install_height = next_execution_height(
        &client,
        beacon_record.session.adaptive_dkg.finalized_at_height,
        "mandatory beacon-key installation",
    )
    .await?;
    let lifecycle_certificate = lifecycle_certificate(
        &network,
        &ordered_roster,
        ThresholdKeyLifecycleActionV1::FinalizeGlobalBeaconKey,
        beacon_record.session.session_id,
        beacon_record.session.transcript_hash,
        norito::encode_canonical(&beacon_record)?,
        install_height,
    )?;
    submit_parliament_instructions(&client, [lifecycle_certificate]).await?;
    assert_eq!(current_height(&client).await?, install_height);
    let activation_height = install_height
        .checked_add(1)
        .ok_or_else(|| eyre!("mandatory beacon-key activation height overflow"))?;
    admit_parliament_height_carrier(
        &client,
        [Log::new(
            Level::INFO,
            "carry mandatory beacon-key activation".to_owned(),
        )],
    )
    .await?;
    network.ensure_blocks(activation_height).await?;
    assert_eq!(current_height(&client).await?, activation_height);

    let boundary_height = MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS;
    let pulse_height = boundary_height - 1;
    advance_to_autonomous_predecessor(
        &network,
        &client,
        pulse_height,
        "initial mandatory pre-boundary pulse",
    )
    .await?;
    assert_no_global_beacon_pulse_at(
        &client,
        pulse_height - 1,
        "an unrequested non-boundary height must not emit a global pulse",
    )
    .await?;
    network.ensure_blocks(pulse_height).await?;
    assert_eq!(
        current_height(&client).await?,
        pulse_height,
        "the mandatory threshold-beacon effect must autonomously finalize its exact pre-boundary height",
    );

    let mut pulses = Vec::with_capacity(network.peers().len());
    for peer in network.peers() {
        pulses.push(pulse_at(&peer.client(), pulse_height).await?);
    }
    assert!(pulses.windows(2).all(|pair| pair[0] == pair[1]));
    let pulse = &pulses[0];
    assert_eq!(pulse.height, pulse_height);
    assert_eq!(pulse.session_id, beacon_record.session.session_id);
    assert_eq!(pulse.roster_hash, beacon_record.session.roster_hash);
    assert_eq!(pulse.transcript_hash, beacon_record.session.transcript_hash);
    assert_eq!(pulse.round, 0, "the mandatory pulse is view-independent");
    assert_eq!(pulse.finalized_chain_anchor.height + 1, pulse_height);
    assert_eq!(
        exact_block(&client, pulse.finalized_chain_anchor.height)
            .await?
            .header()
            .hash(),
        pulse.finalized_chain_anchor.block_hash,
    );
    verify_finalized_global_threshold_beacon_pulse_v1(
        &validated_beacon_session,
        pulse,
        pulse.finalized_chain_anchor,
    )
    .wrap_err("independently verify the mandatory pre-boundary pulse")?;

    let successor_epoch = 1;
    let successor_seed =
        global_threshold_beacon_npos_successor_seed_v1(pulse, boundary_height, successor_epoch);
    admit_parliament_height_carrier(
        &client,
        [Log::new(
            Level::INFO,
            "carry retained-authority successor progression".to_owned(),
        )],
    )
    .await?;
    network.ensure_blocks(boundary_height).await?;
    assert_eq!(current_height(&client).await?, boundary_height);
    network.ensure_blocks(boundary_height + 1).await?;
    assert_eq!(current_height(&client).await?, boundary_height + 1);
    for peer in network.peers() {
        let status = read_on_dedicated_thread({
            let client = peer.client().client().clone();
            move || client.get_sumeragi_status()
        })
        .await?;
        assert!(
            !status.is_halted(),
            "a successful mandatory beacon transition must not fail-stop a validator",
        );
        assert!(status.committed_height >= boundary_height + 1);
        assert!(status.applied_height <= status.committed_height);
        // Epoch state is certified finality evidence, not a local status field.
        let (proof, certified_hash) = read_on_dedicated_thread({
            let client = peer.client().client().clone();
            let network_id = network.network_id();
            move || {
                client.get_bridge_finality_anchor(
                    NonZeroU64::new(boundary_height + 1).unwrap(),
                    network_id,
                )
            }
        })
        .await?;
        assert_eq!(
            exact_block(&peer.client(), boundary_height + 1)
                .await?
                .header()
                .hash(),
            certified_hash,
        );
        let context = &proof.finality_artifact.height_context;
        assert_eq!(context.epoch, successor_epoch);
        assert_eq!(context.leader_seed, successor_seed);
        assert_eq!(
            context.epoch_end_height,
            boundary_height + MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS,
        );
    }

    let successor_pulse_height = boundary_height
        .checked_add(MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS)
        .and_then(|height| height.checked_sub(1))
        .ok_or_else(|| eyre!("successor mandatory pulse height overflow"))?;
    advance_to_autonomous_predecessor(
        &network,
        &client,
        successor_pulse_height,
        "retained-session mandatory pre-boundary pulse",
    )
    .await?;
    network.ensure_blocks(successor_pulse_height).await?;
    let mut successor_pulses = Vec::with_capacity(network.peers().len());
    for peer in network.peers() {
        successor_pulses.push(pulse_at(&peer.client(), successor_pulse_height).await?);
    }
    assert!(successor_pulses.windows(2).all(|pair| pair[0] == pair[1]));
    let successor_pulse = &successor_pulses[0];
    assert_eq!(successor_pulse.height, successor_pulse_height);
    assert_eq!(successor_pulse.session_id, beacon_record.session.session_id);
    assert_eq!(
        successor_pulse.roster_hash,
        beacon_record.session.roster_hash
    );
    assert_eq!(
        successor_pulse.transcript_hash,
        beacon_record.session.transcript_hash
    );
    assert_eq!(successor_pulse.session_id, pulse.session_id);
    verify_finalized_global_threshold_beacon_pulse_v1(
        &validated_beacon_session,
        successor_pulse,
        successor_pulse.finalized_chain_anchor,
    )
    .wrap_err("independently verify the retained-session mandatory pulse")?;

    let second_boundary_height = boundary_height
        .checked_add(MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS)
        .ok_or_else(|| eyre!("second mandatory NPoS boundary overflow"))?;
    let second_successor_epoch = 2;
    let second_successor_seed = global_threshold_beacon_npos_successor_seed_v1(
        successor_pulse,
        second_boundary_height,
        second_successor_epoch,
    );
    admit_parliament_height_carrier(
        &client,
        [Log::new(
            Level::INFO,
            "carry the retained-session NPoS boundary".to_owned(),
        )],
    )
    .await?;
    network.ensure_blocks(second_boundary_height).await?;
    network.ensure_blocks(second_boundary_height + 1).await?;
    for peer in network.peers() {
        let status = read_on_dedicated_thread({
            let client = peer.client().client().clone();
            move || client.get_sumeragi_status()
        })
        .await?;
        assert!(!status.is_halted());
        assert!(status.committed_height >= second_boundary_height + 1);
        assert!(status.applied_height <= status.committed_height);
        // Epoch state is certified finality evidence, not a local status field.
        let (proof, certified_hash) = read_on_dedicated_thread({
            let client = peer.client().client().clone();
            let network_id = network.network_id();
            move || {
                client.get_bridge_finality_anchor(
                    NonZeroU64::new(second_boundary_height + 1).unwrap(),
                    network_id,
                )
            }
        })
        .await?;
        assert_eq!(
            exact_block(&peer.client(), second_boundary_height + 1)
                .await?
                .header()
                .hash(),
            certified_hash,
        );
        let context = &proof.finality_artifact.height_context;
        assert_eq!(context.epoch, second_successor_epoch);
        assert_eq!(context.leader_seed, second_successor_seed);
        assert_eq!(
            context.epoch_end_height,
            second_boundary_height + MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS,
        );
    }

    // NetworkId pins the signed genesis. Verify every contiguous successor from that
    // anchor so a self-consistent server snapshot cannot establish a new authority.
    let trusted_network = network.network_id();
    let expected_roster = ordered_roster.clone();
    let expected_session = beacon_record.session.session_id;
    let expected_transcript = beacon_record.session.transcript_hash;
    read_on_dedicated_thread({
        let client = client.client().clone();
        move || -> Result<()> {
            use iroha_data_model::{
                bridge::BridgeFinalityVerifier,
                isi::kagemusha_v1::{
                    BeaconEpochBindingV1, InstalledBeaconEpochBindingV1,
                    KagemushaMintFinalityEpochDecisionV1,
                },
            };
            let (genesis, genesis_hash) =
                client.get_bridge_finality_anchor(NonZeroU64::new(1).unwrap(), trusted_network)?;
            assert_eq!(genesis_hash, trusted_network.into_genesis_hash());
            assert_eq!(
                genesis
                    .finality_artifact
                    .height_context
                    .roster
                    .iter()
                    .map(|seat| seat.validator.clone())
                    .collect::<Vec<_>>(),
                expected_roster
            );
            let authority = genesis
                .finality_artifact
                .height_context
                .kagemusha_mint_finality_authority
                .clone();
            let mut verifier = BridgeFinalityVerifier::with_context(
                trusted_network,
                genesis.finality_artifact.context_id(),
            );
            verifier.verify(&genesis)?;
            let mut frozen_attempt = None;
            for height in 2..=second_boundary_height + 1 {
                let proof = client.get_next_bridge_finality_proof(
                    NonZeroU64::new(height).unwrap(),
                    &mut verifier,
                )?;
                let context = &proof.finality_artifact.height_context;
                assert_eq!(context.kagemusha_mint_finality_authority, authority);
                assert_eq!(context.da_layout, recommended_data_availability_layout());
                assert_eq!(context.quorum.min_signers, 3);
                assert_eq!(proof.finality_artifact.commit_qc.signers.len(), 3);
                if height == boundary_height {
                    frozen_attempt = context
                        .next_epoch_snapshot
                        .as_ref()
                        .unwrap()
                        .committee_preparation
                        .clone();
                }
                if [boundary_height + 1, second_boundary_height + 1].contains(&height) {
                    let (expected_epoch, expected_seed, expected_end) =
                        if height == boundary_height + 1 {
                            (
                                successor_epoch,
                                successor_seed,
                                boundary_height + MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS,
                            )
                        } else {
                            (
                                second_successor_epoch,
                                second_successor_seed,
                                second_boundary_height + MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS,
                            )
                        };
                    assert_eq!(context.epoch, expected_epoch);
                    assert_eq!(context.leader_seed, expected_seed);
                    assert_eq!(context.epoch_end_height, expected_end);
                    let authorization = &context.kagemusha_mint_finality_authorization;
                    assert_eq!(
                        authorization.epoch,
                        (height - 1) / MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS
                    );
                    if height == second_boundary_height + 1 && frozen_attempt.is_some() {
                        assert_eq!(
                            authorization.decision,
                            KagemushaMintFinalityEpochDecisionV1::RetainAndCancel
                        );
                        assert_eq!(
                            authorization.transition_id,
                            frozen_attempt.as_ref().unwrap().transition_id().unwrap()
                        );
                    } else {
                        assert_eq!(
                            authorization.decision,
                            KagemushaMintFinalityEpochDecisionV1::Retain
                        );
                        assert_eq!(authorization.transition_id, [0; 32]);
                    }
                    assert_eq!(
                        authorization.beacon,
                        BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
                            session_id: expected_session,
                            transcript_hash: expected_transcript,
                        })
                    );
                }
            }
            Ok(())
        }
    })
    .await
    .wrap_err("authenticate retained authority across both real scheduling boundaries")?;

    network.shutdown().await;
    Ok(())
}

#[test]
fn four_validator_mandatory_npos_beacon_fails_closed_below_threshold() -> Result<()> {
    let name = stringify!(four_validator_mandatory_npos_beacon_fails_closed_below_threshold);
    let handle = std::thread::Builder::new()
        .name(name.to_owned())
        .stack_size(PARLIAMENT_NETWORK_STACK_BYTES)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .thread_stack_size(PARLIAMENT_NETWORK_STACK_BYTES)
                .enable_all()
                .build()
                .expect("build four-validator fail-closed beacon test runtime")
                .block_on(four_validator_mandatory_npos_beacon_fails_closed_below_threshold_impl())
        })
        .expect("spawn four-validator fail-closed beacon test thread");
    match handle.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

async fn four_validator_mandatory_npos_beacon_fails_closed_below_threshold_impl() -> Result<()> {
    assert_eq!(
        FAIL_CLOSED_BEACON_SIGNER_MODES
            .iter()
            .filter(|mode| **mode == ParliamentBeaconSignerMode::Valid)
            .count(),
        1,
        "the negative corridor must retain exactly one proof-valid beacon share",
    );
    let mut npos = SumeragiNposParameters::default();
    npos.epoch_length_blocks = NonZeroU64::new(MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS)
        .expect("mandatory NPoS epoch length is non-zero");
    npos.evidence_horizon_blocks = MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS * 2;
    npos.slashing_delay_blocks = MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS;
    npos.validate()
        .map_err(|error| eyre!("invalid fail-closed NPoS fixture: {error}"))?;

    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus()
        .with_parliament_beacon_signer_modes(FAIL_CLOSED_BEACON_SIGNER_MODES)
        .with_block_cadence(EXACT_HEIGHT_SUBMISSION_CADENCE)
        .with_config_layer(|layer| {
            layer
                .write(
                    ["concurrency", "rayon_global_threads"],
                    PARLIAMENT_NETWORK_RAYON_THREADS_PER_PEER,
                )
                .write(
                    [
                        "network",
                        "soranet_handshake",
                        "pow",
                        "puzzle",
                        "memory_kib",
                    ],
                    i64::from(iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB),
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "time_cost"],
                    1_i64,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "lanes"],
                    1_i64,
                )
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos.into_custom_parameter(),
        )));
    let context = stringify!(four_validator_mandatory_npos_beacon_fails_closed_below_threshold);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    assert_eq!(network.peers().len(), VALIDATOR_COUNT);
    network.ensure_blocks(1).await?;

    let client = network.client();
    let ordered_roster = ordered_validator_roster(&network, &client).await?;
    let beacon_record =
        deterministic_parliament_beacon_key_record_v1(network.network_id(), &ordered_roster)
            .wrap_err("derive fail-closed NPoS beacon fixture")?;
    assert_eq!(beacon_record.session.committee_size, 4);
    assert_eq!(beacon_record.session.threshold, 2);
    let install_height = next_execution_height(
        &client,
        beacon_record.session.adaptive_dkg.finalized_at_height,
        "fail-closed beacon-key installation",
    )
    .await?;
    let lifecycle_certificate = lifecycle_certificate(
        &network,
        &ordered_roster,
        ThresholdKeyLifecycleActionV1::FinalizeGlobalBeaconKey,
        beacon_record.session.session_id,
        beacon_record.session.transcript_hash,
        norito::encode_canonical(&beacon_record)?,
        install_height,
    )?;
    submit_parliament_instructions(&client, [lifecycle_certificate]).await?;
    assert_eq!(current_height(&client).await?, install_height);
    let activation_height = install_height
        .checked_add(1)
        .ok_or_else(|| eyre!("fail-closed beacon-key activation height overflow"))?;
    admit_parliament_height_carrier(
        &client,
        [Log::new(
            Level::INFO,
            "carry fail-closed beacon-key activation".to_owned(),
        )],
    )
    .await?;
    network.ensure_blocks(activation_height).await?;
    assert_eq!(current_height(&client).await?, activation_height);

    let pulse_height = MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS - 1;
    let predecessor_height = pulse_height - 1;
    assert_eq!(
        activation_height.checked_add(1),
        Some(predecessor_height),
        "the retained deterministic fixture must expose one committed predecessor after activation",
    );
    network.ensure_blocks(predecessor_height).await?;
    assert_eq!(current_height(&client).await?, predecessor_height);
    let pulse_status_is_active = |status: &SumeragiStatus| -> bool {
        assert!(
            !status.is_halted(),
            "below-threshold beacon liveness must stall without fail-stopping consensus",
        );
        assert_eq!(status.committed_height, predecessor_height);
        assert!(status.applied_height <= status.committed_height);
        if status.height == predecessor_height {
            // A committed round remains current only until its successor
            // configuration is available. Keep polling through that handoff.
            assert!(status.awaiting);
            return false;
        }
        assert_eq!(status.height, pulse_height);
        // Round entry and durable application are reported separately. Begin
        // the observation only once the predecessor is applied and the pulse
        // round has its configuration, so configuration/application lag cannot
        // masquerade as a below-threshold beacon stall.
        !status.awaiting && status.applied_height == predecessor_height
    };
    // Keep each synchronous request short and check one monotonic deadline
    // before and after it. The complete wait can therefore exceed its nominal
    // window by at most one request bound, without leaving detached blocking
    // tasks behind.
    let status_poll_window = network.sync_timeout();
    if status_poll_window.is_zero() {
        return Err(eyre!(
            "the fail-closed status polling window must be non-zero"
        ));
    }
    let requests_per_sweep = u32::try_from(network.peers().len())
        .ok()
        .and_then(|peers| peers.checked_mul(2))
        .filter(|requests| *requests != 0)
        .ok_or_else(|| eyre!("the fail-closed status sweep width must fit in u32"))?;
    let status_poll_request_timeout = status_poll_window
        .checked_div(requests_per_sweep)
        .unwrap_or(Duration::ZERO)
        .max(Duration::from_millis(1))
        .min(Duration::from_secs(5));
    let status_poll_clients = network
        .peers()
        .iter()
        .map(|peer| {
            integration_tests::sync::rebind_blocking_client(&peer.client(), |builder| {
                builder.torii_request_timeout = status_poll_request_timeout;
            })
        })
        .collect::<Vec<_>>();

    let activation_deadline = Instant::now()
        .checked_add(status_poll_window)
        .ok_or_else(|| eyre!("fail-closed activation deadline overflow"))?;
    let mut last_activation_status_error = None;
    loop {
        let mut all_pulse_heights_active = true;
        for (peer_index, peer_client) in status_poll_clients.iter().enumerate() {
            if Instant::now() >= activation_deadline {
                return Err(eyre!(
                    "validators did not publish the mandatory pulse-height context within {status_poll_window:?} plus the {status_poll_request_timeout:?} in-flight request bound; last status fetch error: {}",
                    last_activation_status_error.as_deref().unwrap_or("none"),
                ));
            }
            let observed_height = match current_height(peer_client).await {
                Ok(height) => height,
                Err(error) => {
                    last_activation_status_error =
                        Some(format!("peer {peer_index} height: {error}"));
                    all_pulse_heights_active = false;
                    continue;
                }
            };
            if Instant::now() >= activation_deadline {
                return Err(eyre!(
                    "validators did not publish the mandatory pulse-height context within {status_poll_window:?} plus the {status_poll_request_timeout:?} in-flight request bound; last status fetch error: {}",
                    last_activation_status_error.as_deref().unwrap_or("none"),
                ));
            }
            assert_eq!(
                observed_height, predecessor_height,
                "the pulse height must remain uncommitted during successor activation",
            );
            let status = match read_on_dedicated_thread({
                let client = peer_client.client().clone();
                move || client.get_sumeragi_status()
            })
            .await
            {
                Ok(status) => status,
                Err(error) => {
                    last_activation_status_error =
                        Some(format!("peer {peer_index} sumeragi status: {error}"));
                    all_pulse_heights_active = false;
                    continue;
                }
            };
            if Instant::now() >= activation_deadline {
                return Err(eyre!(
                    "validators did not publish the mandatory pulse-height context within {status_poll_window:?} plus the {status_poll_request_timeout:?} in-flight request bound; last status fetch error: {}",
                    last_activation_status_error.as_deref().unwrap_or("none"),
                ));
            }
            all_pulse_heights_active &= pulse_status_is_active(&status);
        }
        if all_pulse_heights_active {
            break;
        }
        let remaining = activation_deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(eyre!(
                "validators did not publish the mandatory pulse-height context within {status_poll_window:?} plus the {status_poll_request_timeout:?} in-flight request bound; last status fetch error: {}",
                last_activation_status_error.as_deref().unwrap_or("none"),
            ));
        }
        tokio::time::sleep(Duration::from_millis(100).min(remaining)).await;
    }

    let unexpected_pulse_height = tokio::time::timeout(
        FAIL_CLOSED_BEACON_OBSERVATION_WINDOW,
        network.peers()[0].once_block(pulse_height),
    )
    .await;
    assert!(
        unexpected_pulse_height.is_err(),
        "one valid share plus one proof-invalid share must not satisfy the exact threshold of two",
    );

    let post_observation_deadline = Instant::now()
        .checked_add(status_poll_window)
        .ok_or_else(|| eyre!("fail-closed post-observation deadline overflow"))?;
    let mut last_post_observation_status_error = None;
    loop {
        let mut all_post_observation_statuses_verified = true;
        for (peer_index, (peer, peer_client)) in
            network.peers().iter().zip(&status_poll_clients).enumerate()
        {
            assert!(
                peer.is_running(),
                "the beacon-share fault must not stop a consensus validator",
            );
            if Instant::now() >= post_observation_deadline {
                return Err(eyre!(
                    "validators did not retain the mandatory pulse-height context within {status_poll_window:?} plus the {status_poll_request_timeout:?} in-flight request bound after the below-threshold observation; last status fetch error: {}",
                    last_post_observation_status_error
                        .as_deref()
                        .unwrap_or("none"),
                ));
            }
            let observed_height = match current_height(peer_client).await {
                Ok(height) => height,
                Err(error) => {
                    last_post_observation_status_error =
                        Some(format!("peer {peer_index} height: {error}"));
                    all_post_observation_statuses_verified = false;
                    continue;
                }
            };
            if Instant::now() >= post_observation_deadline {
                return Err(eyre!(
                    "validators did not retain the mandatory pulse-height context within {status_poll_window:?} plus the {status_poll_request_timeout:?} in-flight request bound after the below-threshold observation; last status fetch error: {}",
                    last_post_observation_status_error
                        .as_deref()
                        .unwrap_or("none"),
                ));
            }
            assert_eq!(
                observed_height, predecessor_height,
                "the mandatory pre-boundary height must remain uncommitted below threshold",
            );
            let status = match read_on_dedicated_thread({
                let client = peer_client.client().clone();
                move || client.get_sumeragi_status()
            })
            .await
            {
                Ok(status) => status,
                Err(error) => {
                    last_post_observation_status_error =
                        Some(format!("peer {peer_index} sumeragi status: {error}"));
                    all_post_observation_statuses_verified = false;
                    continue;
                }
            };
            if Instant::now() >= post_observation_deadline {
                return Err(eyre!(
                    "validators did not retain the mandatory pulse-height context within {status_poll_window:?} plus the {status_poll_request_timeout:?} in-flight request bound after the below-threshold observation; last status fetch error: {}",
                    last_post_observation_status_error
                        .as_deref()
                        .unwrap_or("none"),
                ));
            }
            assert!(
                pulse_status_is_active(&status),
                "the bounded below-threshold observation must begin and end in the active pulse context",
            );
        }
        if all_post_observation_statuses_verified {
            break;
        }
        let remaining = post_observation_deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(eyre!(
                "validators did not retain the mandatory pulse-height context within {status_poll_window:?} plus the {status_poll_request_timeout:?} in-flight request bound after the below-threshold observation; last status fetch error: {}",
                last_post_observation_status_error
                    .as_deref()
                    .unwrap_or("none"),
            ));
        }
        tokio::time::sleep(Duration::from_millis(100).min(remaining)).await;
    }

    network.shutdown().await;
    Ok(())
}

#[test]
fn parliament_network_corridor_has_no_legacy_or_consensus_bypass_surface() {
    let source = [
        include_str!("sora_parliament_lifecycle_smoke.rs"),
        include_str!("sora_parliament_lifecycle_support.rs"),
        include_str!("sora_parliament_enactment.rs"),
        include_str!("sora_parliament_no_result_paths.rs"),
        include_str!("sora_parliament_failure_paths.rs"),
        include_str!("sora_parliament_private_ballot_retry.rs"),
    ]
    .concat();
    let rayon_threads_key = concat!("rayon_global_", "threads");
    assert_eq!(
        source.matches(rayon_threads_key).count(),
        6,
        "every four-validator builder must bound per-peer proving concurrency without disabling FastPQ",
    );
    let forbidden = [
        concat!(".", "submit("),
        concat!(".", "submit_all("),
        concat!("client.", "status()"),
        concat!("tokio::task::", "block_in_place"),
        concat!("tokio::task::", "spawn_blocking"),
        concat!("Cast", "PlainBallot"),
        concat!("Cast", "ParliamentBallot"),
        concat!("Finalize", "Referendum"),
        concat!("Enact", "Referendum"),
        concat!("Construct", "Certificate"),
        concat!("Parliament", "CertificateV1"),
        concat!("ParliamentAutomatic", "ExecutionOutcomeV1"),
        concat!("Mark", "Enacted"),
        concat!("Mark", "Superseded"),
        concat!("Mark", "ExecutionFailed"),
        concat!("Commit", "ContractDeployment"),
        concat!("without_npos_", "genesis_bootstrap"),
        concat!("with_consensus_", "message_control"),
        concat!("sumeragi.debug", ".rbc"),
        concat!("legacy", "_rbc"),
        concat!("rbc_", "bypass"),
    ];
    for name in forbidden {
        assert!(
            !source.contains(name),
            "modern Parliament corridor must not contain retired or bypass operation `{name}`",
        );
    }
    assert!(source.contains(concat!("with_peers(", "VALIDATOR_COUNT)")));
    assert!(source.contains(concat!("with_npos_", "consensus()")));
    assert!(source.contains(concat!(
        "with_parliament_beacon_",
        "signer_modes(POSITIVE_BEACON_SIGNER_MODES)"
    )));
    assert!(source.contains(concat!(
        "with_parliament_beacon_",
        "signer_modes(FAIL_CLOSED_BEACON_SIGNER_MODES)"
    )));
    assert!(source.contains(concat!(
        "SumeragiGenesisContextParameters::recommended()",
        ".da_layout"
    )));
    let boundary_helper = concat!("assert_transition_rejected_without_state_", "change(");
    assert_eq!(
        source.matches(boundary_helper).count(),
        12,
        "the helper definition plus eleven exact checkpoint/replay calls must remain",
    );
    assert_eq!(
        source
            .matches(concat!("assert_private_transition_", "rejected("))
            .count(),
        5,
        "the typed private-retry helper and all four boundary cases must remain",
    );
    for required in [
        concat!("ConsumeSortition", "PulseBatch"),
        concat!("RegisterBallot", "Participant"),
        concat!("CloseBallot", "Registration"),
        concat!("FreezeBallot", "Survivors"),
        concat!("FreezeTimedOvn", "Corpus"),
        concat!("FailBallot", "NoResult"),
        concat!("private_ballot_deadline_", "retry_impl"),
        concat!("combine_partial_", "releases"),
        concat!("FinalizeOpened", "Ballot"),
        concat!("GovernanceAttemptStatusV1::", "Enacted"),
        concat!("exercise_public_finding_", "no_result_retries_and_restore"),
        concat!(
            "ParliamentNoResultKindV1::",
            "PublicFindingQuorumUnreachable"
        ),
        concat!("ParliamentNoResultKindV1::", "PublicFindingDeadlineExpired"),
        concat!("attempt_sequence:", " 2"),
        concat!("attempt_sequence:", " 1"),
        concat!(
            "deterministic_parliament_beacon_",
            "successor_key_record_v1"
        ),
        concat!("lifecycle_certificate_", "replacing"),
        concat!("shutdown_if_", "started"),
        concat!("start_", "checked"),
    ] {
        assert!(
            source.contains(required),
            "modern Parliament corridor lost required operation `{required}`",
        );
    }
}

#[path = "sora_parliament_failure_paths.rs"]
mod failure_paths;

#[path = "sora_parliament_no_result_paths.rs"]
mod no_result_paths;
