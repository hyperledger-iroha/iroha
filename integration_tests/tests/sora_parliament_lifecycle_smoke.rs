#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Four-validator modern SORA Parliament and mandatory timed-OVN lifecycle corridor.

use std::{collections::BTreeMap, num::NonZeroU64, str::FromStr as _, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use eyre::{Result, WrapErr as _, eyre};
use integration_tests::sandbox;
use iroha::{
    client::{
        Client, ParliamentTimedOvnCastingContextResponseV1, ParliamentTlePartialReleaseShareV1,
        ParliamentTleReleaseContextResponseV1,
    },
    crypto::{Algorithm, Hash, KeyPair, Signature},
    data_model::{
        account::AccountId,
        block::{
            SignedBlock,
            consensus_v2::{PROTOCOL_VERSION, SumeragiV2GenesisContextParameters},
        },
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
                ParliamentEndorsePublicFindingV1, ParliamentFinalizeOpenedBallotV1,
                ParliamentFreezeBallotSurvivorsV1, ParliamentFreezeTimedOvnCorpusV1,
                ParliamentInvitationDecisionV1, ParliamentLifecycleTransitionV1,
                ParliamentRecordAttemptAbsenceV1, ParliamentRecordInvitationResponseV1,
                ParliamentRegisterBallotAttemptV1, ParliamentRegisterBallotParticipantV1,
                ParliamentRegisterSortitionRequestV1, ParliamentSealBodyRosterV1,
                ParliamentSortitionRequestRegistrationV1, ParliamentTleFinalReleaseSignatureV1,
                ProposeDeployContract, RegisterCitizen, SubmitParliamentLifecycleTransitionV1,
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
        peer::PeerId,
        permission::Permission,
        prelude::{
            Account, FeePaymentIntent, FindBlocks, Grant, Level, QueryBuilderExt as _, Register,
            SetParameter,
        },
        smart_contract::ContractAddress,
    },
};
use iroha_core::{
    beacon::{
        GlobalThresholdBeaconSessionBindingV1, global_threshold_beacon_npos_successor_seed_v1,
        global_threshold_beacon_roster_hash_v1,
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
    governance::CanProposeContractDeployment, smart_contract::CanRegisterSmartContractCode,
};
use iroha_test_network::{NetworkBuilder, ParliamentBeaconSignerMode};
use iroha_test_samples::ALICE_ID;
use norito::codec::Encode as _;
use rand::{SeedableRng as _, rngs::StdRng};

const VALIDATOR_COUNT: usize = 4;
const CITIZEN_COUNT: usize = 24;
const BODY_SEATS: u32 = 3;
const INVITATION_PHASE_BLOCKS: u64 = 30;
const REGISTRATION_PHASE_BLOCKS: u64 = 9;
const SURVIVOR_PHASE_BLOCKS: u64 = 8;
const COMMITMENT_PHASE_BLOCKS: u64 = 4;
const RELEASE_DELAY_BLOCKS: u64 = 3;
const OPENING_PHASE_BLOCKS: u64 = 8;
const MIN_ENACTMENT_DELAY: u64 = 3;
const MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS: u64 = 8;
const PARLIAMENT_NETWORK_STACK_BYTES: usize = 32 * 1024 * 1024;
const TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES: i64 = 1_073_741_824;
const OPERATION_TIMEOUT: Duration = Duration::from_secs(300);
const FAIL_CLOSED_BEACON_OBSERVATION_WINDOW: Duration = Duration::from_secs(8);
const POSITIVE_BEACON_SIGNER_MODES: [ParliamentBeaconSignerMode; VALIDATOR_COUNT] = [
    ParliamentBeaconSignerMode::Valid,
    ParliamentBeaconSignerMode::Valid,
    ParliamentBeaconSignerMode::Absent,
    ParliamentBeaconSignerMode::Invalid,
];
const FAIL_CLOSED_BEACON_SIGNER_MODES: [ParliamentBeaconSignerMode; VALIDATOR_COUNT] = [
    ParliamentBeaconSignerMode::Valid,
    ParliamentBeaconSignerMode::Absent,
    ParliamentBeaconSignerMode::Absent,
    ParliamentBeaconSignerMode::Invalid,
];
const CONTRACT_ADDRESS: &str = "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw";
const NO_RESULT_RETRY_CONTRACT_ADDRESS: &str =
    "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh";

fn fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(Vec::new(), None)
}

fn minimal_contract_artifact() -> Vec<u8> {
    let metadata = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 1_000,
        abi_version: 1,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "ParliamentLifecycleSmoke".to_owned(),
        compiler_fingerprint: "integration-tests".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: "main".to_owned(),
            kind: iroha::data_model::smart_contract::manifest::EntryPointKind::View,
            params: Vec::new(),
            argument_schema: None,
            return_type: None,
            return_schema: None,
            permission: None,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        }],
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    let mut artifact = metadata.encode();
    artifact.extend_from_slice(&interface.encode_section());
    artifact.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    artifact
}

fn citizen_keys() -> Vec<KeyPair> {
    (0..CITIZEN_COUNT)
        .map(|index| {
            KeyPair::try_from_seed(
                format!("sora-parliament-modern-citizen-{index:02}").into_bytes(),
                Algorithm::Ed25519,
            )
            .expect("derive deterministic citizen key")
        })
        .collect()
}

fn citizen_accounts(keys: &[KeyPair]) -> Vec<AccountId> {
    let mut accounts = keys
        .iter()
        .map(|key| AccountId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    accounts.sort_unstable();
    accounts
}

fn client_for(base: &Client, account: &AccountId, keys: &[KeyPair]) -> Client {
    let key = keys
        .iter()
        .find(|key| {
            account
                .try_signatory()
                .is_some_and(|signatory| key.public_key() == signatory)
        })
        .expect("selected citizen owns one deterministic key")
        .clone();
    let mut client = base.clone();
    client.account = account.clone();
    client.key_pair = key;
    client
}

fn current_height(client: &Client) -> Result<u64> {
    Ok(client.get_status()?.blocks)
}

fn tick(client: &Client, label: impl Into<String>) -> Result<u64> {
    client.submit_blocking(Log::new(Level::INFO, label.into()), fee())?;
    current_height(client)
}

fn advance_to_predecessor(client: &Client, target_height: u64, label: &str) -> Result<()> {
    loop {
        let height = current_height(client)?;
        if height + 1 == target_height {
            return Ok(());
        }
        if height + 1 > target_height {
            return Err(eyre!(
                "{label}: exact height {target_height} passed at finalized height {height}"
            ));
        }
        tick(client, format!("{label} height tick {}", height + 1))?;
    }
}

fn submit_transition(
    client: &Client,
    attempt_id: GovernanceAttemptId,
    transition: ParliamentLifecycleTransitionV1,
) -> Result<u64> {
    client.submit_blocking(
        SubmitParliamentLifecycleTransitionV1 {
            governance_attempt_id: attempt_id,
            transition,
        },
        fee(),
    )?;
    current_height(client)
}

fn submit_transitions(
    client: &Client,
    attempt_id: GovernanceAttemptId,
    transitions: impl IntoIterator<Item = ParliamentLifecycleTransitionV1>,
) -> Result<u64> {
    client.submit_all_blocking(
        transitions.into_iter().map(|transition| {
            InstructionBox::from(SubmitParliamentLifecycleTransitionV1 {
                governance_attempt_id: attempt_id,
                transition,
            })
        }),
        fee(),
    )?;
    current_height(client)
}

fn assert_transition_rejected_without_state_change(
    client: &Client,
    attempt_id: GovernanceAttemptId,
    transition: ParliamentLifecycleTransitionV1,
    label: &str,
) -> Result<()> {
    let before = client.get_parliament_attempt(attempt_id)?.state_payload_hex;
    if client
        .submit_blocking(
            SubmitParliamentLifecycleTransitionV1 {
                governance_attempt_id: attempt_id,
                transition,
            },
            fee(),
        )
        .is_ok()
    {
        return Err(eyre!("{label}: invalid Parliament transition was accepted"));
    }
    let after = client.get_parliament_attempt(attempt_id)?.state_payload_hex;
    if after != before {
        return Err(eyre!(
            "{label}: rejected Parliament transition mutated reducer state"
        ));
    }
    Ok(())
}

fn read_attempt(
    client: &Client,
    attempt_id: GovernanceAttemptId,
) -> Result<ParliamentAttemptStateV1> {
    let response = client.get_parliament_attempt(attempt_id)?;
    let frame = hex::decode(response.state_payload_hex)?;
    norito::decode_canonical(&frame).wrap_err("decode canonical Parliament reducer projection")
}

fn ordered_validator_roster(network: &sandbox::SerializedNetwork) -> Result<Vec<PeerId>> {
    let roster = iroha_core::sumeragi::signed_genesis_voting_peers(&network.genesis())
        .wrap_err("read exact signed genesis voting roster")?;
    if roster.len() != VALIDATOR_COUNT {
        return Err(eyre!("expected exactly four signed validators"));
    }
    Ok(roster)
}

fn lifecycle_certificate(
    network: &sandbox::SerializedNetwork,
    ordered_roster: &[PeerId],
    action: ThresholdKeyLifecycleActionV1,
    session_id: [u8; 32],
    transcript_hash: [u8; 32],
    public_state: Vec<u8>,
    effective_height: u64,
) -> Result<ApplyThresholdKeyLifecycleCertificateV1> {
    let mut certificate = ThresholdKeyLifecycleCertificateV1 {
        version: THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
        action,
        expected_active_session_id: None,
        effective_height,
        network_id: network.network_id(),
        roster_hash: global_threshold_beacon_roster_hash_v1(ordered_roster),
        committee_size: VALIDATOR_COUNT as u16,
        quorum: 3,
        session_id,
        transcript_hash,
        public_state,
        signatures: Vec::new(),
    };
    let preimage = threshold_key_lifecycle_certificate_preimage_v1(&certificate)
        .wrap_err("encode lifecycle QC preimage")?;
    certificate.signatures = ordered_roster
        .iter()
        .take(3)
        .enumerate()
        .map(|(index, peer_id)| {
            let peer = network
                .peers()
                .iter()
                .find(|peer| peer.id() == *peer_id)
                .ok_or_else(|| eyre!("signed roster peer is absent from the network"))?;
            let key = peer
                .bls_key_pair()
                .ok_or_else(|| eyre!("validator lacks its normal BLS keypair"))?;
            Ok(ThresholdKeyLifecycleSignatureV1 {
                signer_index: u16::try_from(index)?,
                signature: Signature::try_new(key.private_key(), &preimage)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ApplyThresholdKeyLifecycleCertificateV1 { certificate })
}

fn pulse_at(
    client: &Client,
    height: u64,
) -> Result<iroha::data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1> {
    client
        .query(FindBlocks)
        .execute_all()?
        .into_iter()
        .find(|block| block.header().height().get() == height)
        .and_then(|block| {
            block
                .npos_consensus_effects()
                .and_then(|effects| effects.finalized_global_beacon_pulse)
        })
        .ok_or_else(|| eyre!("block {height} does not carry the demanded global beacon pulse"))
}

fn signed_consensus_handshake(
    network: &sandbox::SerializedNetwork,
) -> Result<ConsensusHandshakeMetadata> {
    let mut handshakes = network
        .genesis_isi()
        .iter()
        .flatten()
        .filter_map(|instruction| instruction.as_any().downcast_ref::<SetParameter>())
        .filter_map(|set_parameter| match set_parameter.inner() {
            Parameter::Custom(custom)
                if custom.id() == &consensus_metadata::handshake_meta_id() =>
            {
                Some(custom)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if handshakes.len() != 1 {
        return Err(eyre!("expected one signed consensus handshake"));
    }
    norito::json::from_str(
        handshakes
            .pop()
            .expect("handshake count checked")
            .payload()
            .get(),
    )
    .wrap_err("decode signed consensus handshake")
}

fn casting_archive(
    response: &ParliamentTimedOvnCastingContextResponseV1,
    ballot_attempt_id: BallotAttemptId,
) -> Result<ParliamentTimedOvnCastingContextArchiveV1> {
    response
        .validate_for_ballot(ballot_attempt_id)
        .map_err(|error| eyre!(error))?;
    let bytes = BASE64_STANDARD
        .decode(response.archive_norito_base64.as_bytes())
        .wrap_err("decode padded canonical casting archive")?;
    if bytes.len() > PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_ARCHIVE_MAX_BYTES_V1 {
        return Err(eyre!("casting archive exceeds its fixed V1 bound"));
    }
    let archive = norito::decode_canonical::<ParliamentTimedOvnCastingContextArchiveV1>(&bytes)
        .wrap_err("decode canonical casting-context Norito frame")?;
    archive
        .validate_v1()
        .wrap_err("replay-validate public casting archive")?;
    Ok(archive)
}

fn release_projection(
    context: &ParliamentTleReleaseContextResponseV1,
) -> Result<AuthorizedTleReleaseProjectionV1> {
    context
        .validate_for_ballot(context.ballot_attempt_id)
        .map_err(|error| eyre!(error))?;
    let identity_payload: [u8; TLE_AUTHORIZED_RELEASE_IDENTITY_PAYLOAD_BYTES_V1] =
        hex::decode(&context.identity_payload_hex)
            .wrap_err("decode exact release identity payload")?
            .try_into()
            .map_err(|_| eyre!("release identity payload has the wrong width"))?;
    let key_session = &context.tle_key_session;
    Ok(AuthorizedTleReleaseProjectionV1 {
        version: TLE_AUTHORIZED_RELEASE_PROJECTION_VERSION_V1,
        ballot_attempt_id: context.ballot_attempt_id,
        opening_deadline_height: context.opening_deadline_height,
        finalized_height: context.current_height,
        key_session: TleKeySessionPublicStateV1 {
            version: key_session.version,
            key_session_id: key_session.key_session_id,
            network_id: key_session.network_id,
            roster_hash: key_session.roster_hash,
            committee_size: key_session.committee_size,
            threshold: key_session.threshold,
            generator_h: key_session.generator_h,
            generator_v: key_session.generator_v,
            qualified_dealers: key_session.qualified_dealers.clone(),
            qualified_dealer_commitments: key_session
                .qualified_dealer_commitments
                .iter()
                .map(|dealer| TleAdaptiveDealerCommitmentV1 {
                    dealer_index: dealer.dealer_index,
                    coefficient_commitments: dealer.coefficient_commitments.clone(),
                    constant_pok_commitment: dealer.constant_pok_commitment,
                    constant_pok_response: dealer.constant_pok_response,
                })
                .collect(),
            dkg_event_hash: key_session.dkg_event_hash,
            group_public_key: key_session.group_public_key,
            public_shares: key_session
                .public_shares
                .iter()
                .map(|share| TleAdaptivePublicShareV1 {
                    index: share.index,
                    participant_hash: share.participant_hash,
                    public_key_share: share.public_key_share,
                })
                .collect(),
            transcript_hash: key_session.transcript_hash,
        },
        public_release_identity: TimedOvnReleaseIdentityPublicV1 {
            tle_key_session_id: context.release_identity.tle_key_session_id,
            governance_attempt_id: *context.release_identity.governance_attempt_id.as_bytes(),
            body_instance_id: *context.release_identity.body_instance_id.as_bytes(),
            ballot_attempt_id: *context.release_identity.ballot_attempt_id.as_bytes(),
            survivor_corpus_root: context.release_identity.survivor_corpus_root,
            no_recovery_root: context.release_identity.no_recovery_root,
            target_finalized_height: context.release_identity.target_finalized_height,
            parameter_hash: context.release_identity.parameter_hash,
        },
        identity_payload,
        identity_digest: context.identity_digest,
    })
}

fn release_partial(partial: ParliamentTlePartialReleaseShareV1) -> TlePartialReleaseShareV1 {
    TlePartialReleaseShareV1 {
        key_session_id: partial.key_session_id,
        identity_digest: partial.identity_digest,
        participant_index: partial.participant_index,
        sigma: partial.sigma,
        proof_x: partial.proof_x,
        proof_y: partial.proof_y,
        z_s: partial.z_s,
        z_r: partial.z_r,
        z_u: partial.z_u,
    }
}

fn stage_contract_artifact(
    client: &Client,
    artifact: &[u8],
) -> Result<(ContractCodeHash, ContractAbiHash)> {
    let verified = ivm::verify_contract_artifact(artifact)
        .map_err(|error| eyre!("verify integration contract artifact: {error}"))?;
    let manifest = verified
        .manifest
        .try_signed(&client.key_pair)
        .map_err(|error| eyre!("sign integration contract manifest: {error}"))?;
    let total_size = u64::try_from(artifact.len())?;
    let chunk_count = u32::try_from(artifact.len().div_ceil(SMART_CONTRACT_CODE_CHUNK_BYTES))?;
    for (index, chunk) in artifact.chunks(SMART_CONTRACT_CODE_CHUNK_BYTES).enumerate() {
        let chunk_index = u32::try_from(index)?;
        let mut instructions = vec![InstructionBox::from(UploadSmartContractCodeChunk {
            code_hash: verified.code_hash,
            total_size,
            chunk_index,
            chunk_count,
            chunk: chunk.to_vec(),
        })];
        if chunk_index + 1 == chunk_count {
            instructions.push(InstructionBox::from(FinalizeSmartContractCodeUpload {
                code_hash: verified.code_hash,
                total_size,
                chunk_count,
            }));
        }
        client.submit_all_blocking(instructions, fee())?;
    }
    client.submit_blocking(RegisterSmartContractCode { manifest }, fee())?;
    let code_hash = *verified.code_hash.as_ref();
    let abi_hash = *verified.abi_hash.as_ref();
    Ok((
        ContractCodeHash::new(code_hash),
        ContractAbiHash::new(abi_hash),
    ))
}

fn public_finding_root(attempt_id: GovernanceAttemptId, body: ParliamentBody) -> [u8; 32] {
    let body = body.encode();
    Hash::new_from_chunks(&[
        b"iroha.integration.parliament.public-finding.v1\0",
        attempt_id.as_bytes(),
        &body,
    ])
    .into()
}

fn exact_block(client: &Client, height: u64) -> Result<SignedBlock> {
    client
        .query(FindBlocks)
        .execute_all()?
        .into_iter()
        .find(|block| block.header().height().get() == height)
        .ok_or_else(|| eyre!("peer does not retain finalized block {height}"))
}

async fn exercise_public_finding_impossible_quorum_retry(
    network: &sandbox::SerializedNetwork,
    client: &Client,
    citizens: &[AccountId],
    citizen_keys: &[KeyPair],
    contract_address: &ContractAddress,
    code_hash: ContractCodeHash,
    abi_hash: ContractAbiHash,
    logical_beacon: BeaconSessionId,
) -> Result<()> {
    let proposal = ProposalKind::DeployContract(DeployContractProposal {
        contract_address: contract_address.clone(),
        code_hash,
        abi_hash,
        abi_version: AbiVersion::new(1),
        manifest_provenance: None,
    });
    let create = CreateParliamentGovernanceAttemptV1 {
        proposal: proposal.clone(),
        attempt_sequence: 0,
    };
    let attempt_id = create.governance_attempt_id();
    client.submit_all_blocking(
        [
            InstructionBox::from(ProposeDeployContract {
                contract_address: contract_address.clone(),
                code_hash,
                abi_hash,
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            }),
            InstructionBox::from(create),
        ],
        fee(),
    )?;
    submit_transition(
        client,
        attempt_id,
        ParliamentLifecycleTransitionV1::CompleteQualification,
    )?;

    let expected_bodies = [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::OversightCommittee,
        ParliamentBody::PolicyJury,
    ];
    let request_height = current_height(client)? + 1;
    let sortition_pulse_height = request_height + 4;
    let mut election_ids = BTreeMap::new();
    let mut request_ids = Vec::new();
    let mut registrations = Vec::new();
    for body in expected_bodies {
        let election_id = BodyElectionAttemptId::derive_v1(attempt_id, body, 0);
        let request = SortitionRequestV1::try_new_canonical(
            attempt_id,
            election_id,
            body,
            parliament_candidate_root_v1(attempt_id, body, citizens),
            u32::try_from(citizens.len())?,
            BODY_SEATS,
            request_height,
            sortition_pulse_height,
            logical_beacon,
            None,
        )
        .map_err(|error| eyre!("construct no-result sortition request: {error}"))?;
        election_ids.insert(body, election_id);
        request_ids.push(request.id);
        registrations.push(ParliamentSortitionRequestRegistrationV1 {
            sequence: 0,
            request,
        });
    }
    request_ids.sort_unstable();
    submit_transition(
        client,
        attempt_id,
        ParliamentLifecycleTransitionV1::RegisterSortitionRequest(
            ParliamentRegisterSortitionRequestV1 {
                requests: registrations,
            },
        ),
    )?;
    assert_eq!(current_height(client)?, request_height);
    advance_to_predecessor(
        client,
        sortition_pulse_height,
        "no-result retry sortition pulse",
    )?;
    network.ensure_blocks(sortition_pulse_height).await?;
    let pulses = network
        .peers()
        .iter()
        .map(|peer| pulse_at(&peer.client(), sortition_pulse_height))
        .collect::<Result<Vec<_>>>()?;
    assert!(pulses.windows(2).all(|pair| pair[0] == pair[1]));
    let pulse = &pulses[0];
    submit_transition(
        client,
        attempt_id,
        ParliamentLifecycleTransitionV1::ConsumeSortitionPulseBatch(
            ParliamentConsumeSortitionPulseBatchV1 {
                request_ids,
                beacon_session_id: logical_beacon,
                pulse_height: sortition_pulse_height,
                pulse_id: BeaconPulseId::new(pulse.pulse_id),
            },
        ),
    )?;

    submit_transitions(
        client,
        attempt_id,
        expected_bodies.into_iter().map(|body| {
            ParliamentLifecycleTransitionV1::BeginInvitationAcceptance(
                ParliamentBeginInvitationAcceptanceV1 {
                    election_attempt_id: election_ids[&body],
                },
            )
        }),
    )?;
    let invitation_state = read_attempt(client, attempt_id)?;
    let invitation_close_height = expected_bodies
        .into_iter()
        .map(|body| {
            invitation_state
                .election(&election_ids[&body])
                .and_then(|election| election.invitation_close_height())
                .expect("no-result invitation deadline is frozen")
        })
        .reduce(|left, right| {
            assert_eq!(left, right);
            left
        })
        .expect("the no-result attempt requires Parliament bodies");
    let mut invitations_by_member =
        BTreeMap::<AccountId, Vec<(ParliamentBody, BodyElectionAttemptId)>>::new();
    for body in expected_bodies {
        let election = invitation_state
            .election(&election_ids[&body])
            .expect("no-result body election");
        for assignment in election.primary_assignments() {
            invitations_by_member
                .entry(assignment.member.clone())
                .or_default()
                .push((body, election_ids[&body]));
        }
    }
    for (member, invitations) in invitations_by_member {
        submit_transitions(
            &client_for(client, &member, citizen_keys),
            attempt_id,
            invitations.into_iter().map(|(body, election_attempt_id)| {
                ParliamentLifecycleTransitionV1::RecordInvitationResponse(
                    ParliamentRecordInvitationResponseV1 {
                        election_attempt_id,
                        body,
                        decision: ParliamentInvitationDecisionV1::Accept,
                    },
                )
            }),
        )?;
    }
    assert!(current_height(client)? <= invitation_close_height);
    let roster_seal_height = invitation_close_height
        .checked_add(1)
        .ok_or_else(|| eyre!("no-result roster-seal height overflow"))?;
    advance_to_predecessor(client, roster_seal_height, "no-result retry roster sealing")?;
    submit_transitions(
        client,
        attempt_id,
        expected_bodies.into_iter().map(|body| {
            ParliamentLifecycleTransitionV1::SealBodyRoster(ParliamentSealBodyRosterV1 {
                election_attempt_id: election_ids[&body],
            })
        }),
    )?;

    let rules_body_id = read_attempt(client, attempt_id)?
        .sealed_body_for_role(ParliamentBody::RulesCommittee)
        .expect("no-result Rules Committee is sealed")
        .instance()
        .id;
    submit_transitions(
        client,
        attempt_id,
        [
            DeliberationPhaseV1::Orientation,
            DeliberationPhaseV1::Evidence,
            DeliberationPhaseV1::Questions,
            DeliberationPhaseV1::Responses,
            DeliberationPhaseV1::Deliberation,
            DeliberationPhaseV1::Reflection,
        ]
        .into_iter()
        .map(|target| {
            ParliamentLifecycleTransitionV1::AdvanceBodyPhase(ParliamentAdvanceBodyPhaseV1 {
                body_instance_id: rules_body_id,
                target,
            })
        }),
    )?;
    let reflecting = read_attempt(client, attempt_id)?;
    let rules = reflecting
        .body(&rules_body_id)
        .expect("reflecting no-result Rules Committee");
    let public_finding_deadline = rules
        .public_finding_deadline_height()
        .expect("no-result public-finding deadline is frozen");
    let absent_assignments = rules.assignments()[..2].to_vec();
    for (index, assignment) in absent_assignments.iter().enumerate() {
        submit_transition(
            &client_for(client, &assignment.member, citizen_keys),
            attempt_id,
            ParliamentLifecycleTransitionV1::RecordAttemptAbsence(
                ParliamentRecordAttemptAbsenceV1 {
                    body_instance_id: rules_body_id,
                    assignment_id: assignment.assignment_id,
                },
            ),
        )?;
        let observed = read_attempt(client, attempt_id)?;
        let observed_rules = observed
            .body(&rules_body_id)
            .expect("no-result Rules Committee survives projection");
        assert_eq!(observed_rules.excluded_assignments().len(), index + 1);
        if index == 0 {
            assert_eq!(observed.attempt().status, GovernanceAttemptStatusV1::Active);
            assert_eq!(
                observed_rules.instance().status,
                BodyInstanceStatusV1::Deliberating(DeliberationPhaseV1::Reflection),
            );
        }
    }
    let failed_height = current_height(client)?;
    assert!(
        failed_height < public_finding_deadline,
        "objective quorum impossibility must terminate before the frozen deadline",
    );
    let rejected = read_attempt(client, attempt_id)?;
    let rejected_rules = rejected
        .body(&rules_body_id)
        .expect("rejected Rules Committee remains auditable");
    assert_eq!(
        rejected.attempt().status,
        GovernanceAttemptStatusV1::Rejected,
    );
    assert_eq!(
        rejected_rules.instance().status,
        BodyInstanceStatusV1::NoResult
    );
    assert_eq!(
        rejected_rules.public_finding_no_result_kind(),
        Some(ParliamentNoResultKindV1::PublicFindingQuorumUnreachable),
    );
    assert_eq!(
        rejected_rules.public_finding_no_result_height(),
        Some(failed_height),
    );
    assert!(
        client.get_gov_contract_json(contract_address).is_err(),
        "a no-result Parliament attempt must not apply its governed effect",
    );

    let retry = CreateParliamentGovernanceAttemptV1 {
        proposal,
        attempt_sequence: 1,
    };
    let retry_id = retry.governance_attempt_id();
    client.submit_blocking(retry, fee())?;
    let retry_height = current_height(client)?;
    network.ensure_blocks(retry_height).await?;
    let retry_state = read_attempt(client, retry_id)?;
    assert_eq!(retry_state.attempt().sequence, 1);
    assert_eq!(
        retry_state.attempt().status,
        GovernanceAttemptStatusV1::Active
    );
    assert_eq!(
        retry_state.attempt().stage,
        GovernanceStageV1::Qualification
    );

    let rejected_response = client.get_parliament_attempt(attempt_id)?;
    let retry_response = client.get_parliament_attempt(retry_id)?;
    for peer in network.peers() {
        let peer_client = peer.client();
        let peer_rejected = peer_client.get_parliament_attempt(attempt_id)?;
        let peer_retry = peer_client.get_parliament_attempt(retry_id)?;
        assert_eq!(
            peer_rejected.state_payload_hex,
            rejected_response.state_payload_hex
        );
        assert_eq!(
            peer_retry.state_payload_hex,
            retry_response.state_payload_hex
        );
        assert!(
            peer_client.get_gov_contract_json(contract_address).is_err(),
            "every validator must preserve no-result effect isolation",
        );
    }
    Ok(())
}

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
    let mut builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus()
        .with_parliament_beacon_signer_modes(POSITIVE_BEACON_SIGNER_MODES)
        .with_block_cadence(Duration::from_secs(1))
        .with_config_layer(|layer| {
            layer
                // Keep mandatory SoraNet admission enabled while bounding the
                // localhost-only puzzle cost so this corridor measures
                // Parliament/consensus liveness rather than Argon2 contention.
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
                .write(["nexus", "lane_count"], 1_i64)
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
                )
                .write(["gov", "citizenship_bond_amount"], "0")
                .write(["gov", "min_enactment_delay"], MIN_ENACTMENT_DELAY as i64)
                .write(["gov", "parliament_alternate_size"], 0_i64)
                .write(
                    ["gov", "parliament_invitation_phase_blocks"],
                    INVITATION_PHASE_BLOCKS as i64,
                )
                .write(["gov", "parliament_public_finding_phase_blocks"], 20_i64)
                .write(["gov", "rules_committee_size"], BODY_SEATS as i64)
                .write(["gov", "agenda_council_size"], BODY_SEATS as i64)
                .write(["gov", "interest_panel_size"], BODY_SEATS as i64)
                .write(["gov", "review_panel_size"], BODY_SEATS as i64)
                .write(["gov", "oversight_committee_size"], BODY_SEATS as i64)
                .write(["gov", "policy_jury_size"], BODY_SEATS as i64)
                .write(["gov", "confirmation_jury_size"], BODY_SEATS as i64)
                .write(
                    ["gov", "parliament_timed_ovn", "registration_phase_blocks"],
                    REGISTRATION_PHASE_BLOCKS as i64,
                )
                .write(
                    [
                        "gov",
                        "parliament_timed_ovn",
                        "survivor_freeze_phase_blocks",
                    ],
                    SURVIVOR_PHASE_BLOCKS as i64,
                )
                .write(
                    ["gov", "parliament_timed_ovn", "commitment_phase_blocks"],
                    COMMITMENT_PHASE_BLOCKS as i64,
                )
                .write(
                    ["gov", "parliament_timed_ovn", "release_delay_blocks"],
                    RELEASE_DELAY_BLOCKS as i64,
                )
                .write(
                    ["gov", "parliament_timed_ovn", "opening_phase_blocks"],
                    OPENING_PHASE_BLOCKS as i64,
                )
                .write(["gov", "parliament_timed_ovn", "max_ballot_retries"], 0_i64)
                .write(["gov", "parliament_timed_ovn", "max_corpus_entries"], 8_i64);
        })
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanRegisterSmartContractCode),
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
    for citizen in &citizens {
        builder = builder
            .with_genesis_instruction(Register::account(Account::new(citizen.clone())))
            .with_genesis_instruction(RegisterCitizen {
                owner: citizen.clone(),
                amount: 0_u64.into(),
            });
    }

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
        handshake.sumeragi_v2.da_layout,
        SumeragiV2GenesisContextParameters::recommended().da_layout,
        "the corridor must retain the signed revision-4 RS16 DA layout",
    );
    network.ensure_blocks(1).await?;
    let client = network.client();
    let ordered_roster = ordered_validator_roster(&network)?;
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
    let install_height =
        (current_height(&client)? + 1).max(beacon_record.session.adaptive_dkg.finalized_at_height);
    advance_to_predecessor(&client, install_height, "threshold-key installation")?;
    client.submit_all_blocking(
        [
            InstructionBox::from(lifecycle_certificate(
                &network,
                &ordered_roster,
                ThresholdKeyLifecycleActionV1::InstallGlobalBeaconKey,
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
        ],
        fee(),
    )?;
    assert_eq!(current_height(&client)?, install_height);
    tick(&client, "activate installed global beacon session")?;

    let (code_hash, abi_hash) = stage_contract_artifact(&client, &minimal_contract_artifact())?;
    let proposal = ProposalKind::DeployContract(DeployContractProposal {
        contract_address: contract_address.clone(),
        code_hash,
        abi_hash,
        abi_version: AbiVersion::new(1),
        manifest_provenance: None,
    });
    let create = CreateParliamentGovernanceAttemptV1 {
        proposal,
        attempt_sequence: 0,
    };
    let attempt_id = create.governance_attempt_id();
    client.submit_all_blocking(
        [
            InstructionBox::from(ProposeDeployContract {
                contract_address: contract_address.clone(),
                code_hash,
                abi_hash,
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            }),
            InstructionBox::from(create),
        ],
        fee(),
    )?;
    submit_transition(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::CompleteQualification,
    )?;

    let expected_bodies = [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::OversightCommittee,
        ParliamentBody::PolicyJury,
    ];
    let initial = read_attempt(&client, attempt_id)?;
    assert_eq!(initial.required_bodies().len(), expected_bodies.len());
    assert_eq!(initial.attempt().stage, GovernanceStageV1::Rules);
    let request_height = current_height(&client)? + 1;
    let sortition_pulse_height = request_height + 4;
    let logical_beacon = BeaconSessionId::for_network_v1(&network.network_id());
    let mut election_ids = BTreeMap::new();
    let mut request_ids = Vec::new();
    let mut request_registrations = Vec::new();
    for body in expected_bodies {
        let election_id = BodyElectionAttemptId::derive_v1(attempt_id, body, 0);
        let request = SortitionRequestV1::try_new_canonical(
            attempt_id,
            election_id,
            body,
            parliament_candidate_root_v1(attempt_id, body, &citizens),
            u32::try_from(citizens.len())?,
            BODY_SEATS,
            request_height,
            sortition_pulse_height,
            logical_beacon,
            None,
        )
        .map_err(|error| eyre!("construct canonical sortition request: {error}"))?;
        election_ids.insert(body, election_id);
        request_ids.push(request.id);
        request_registrations.push(ParliamentSortitionRequestRegistrationV1 {
            sequence: 0,
            request,
        });
    }
    request_ids.sort_unstable();
    submit_transition(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::RegisterSortitionRequest(
            ParliamentRegisterSortitionRequestV1 {
                requests: request_registrations,
            },
        ),
    )?;
    assert_eq!(current_height(&client)?, request_height);
    advance_to_predecessor(&client, sortition_pulse_height, "sortition pulse")?;
    network.ensure_blocks(sortition_pulse_height).await?;
    assert_eq!(
        current_height(&client)?,
        sortition_pulse_height,
        "the demanded sortition threshold-beacon effect must autonomously finalize its exact height",
    );
    let sortition_pulses = network
        .peers()
        .iter()
        .map(|peer| pulse_at(&peer.client(), sortition_pulse_height))
        .collect::<Result<Vec<_>>>()?;
    assert!(sortition_pulses.windows(2).all(|pair| pair[0] == pair[1]));
    let sortition_pulse = sortition_pulses[0].clone();
    assert_eq!(sortition_pulse.height, sortition_pulse_height);
    assert_eq!(sortition_pulse.network_id, network.network_id());
    assert_eq!(sortition_pulse.session_id, beacon_record.session.session_id);
    assert_eq!(
        sortition_pulse.roster_hash,
        beacon_record.session.roster_hash
    );
    assert_eq!(
        sortition_pulse.transcript_hash,
        beacon_record.session.transcript_hash,
    );
    assert_eq!(sortition_pulse.round, 0, "V1 pulses are view-independent");
    assert_eq!(
        sortition_pulse.finalized_chain_anchor.height.checked_add(1),
        Some(sortition_pulse_height),
    );
    assert_eq!(
        exact_block(&client, sortition_pulse.finalized_chain_anchor.height,)?
            .header()
            .hash(),
        sortition_pulse.finalized_chain_anchor.block_hash,
    );
    verify_finalized_global_threshold_beacon_pulse_v1(
        &validated_beacon_session,
        &sortition_pulse,
        sortition_pulse.finalized_chain_anchor,
    )
    .wrap_err("independently verify the sortition pulse threshold signature")?;
    submit_transition(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::ConsumeSortitionPulseBatch(
            ParliamentConsumeSortitionPulseBatchV1 {
                request_ids: request_ids.clone(),
                beacon_session_id: logical_beacon,
                pulse_height: sortition_pulse_height,
                pulse_id: BeaconPulseId::new(sortition_pulse.pulse_id),
            },
        ),
    )?;
    let drawn = read_attempt(&client, attempt_id)?;
    for body in expected_bodies {
        let election = drawn
            .election(election_ids.get(&body).expect("body election id"))
            .expect("simultaneous body election exists");
        assert_eq!(
            election.pulse_id(),
            Some(BeaconPulseId::new(sortition_pulse.pulse_id))
        );
        assert_eq!(election.pulse_output(), Some(sortition_pulse.seed));
        assert_eq!(election.primary_assignments().len(), BODY_SEATS as usize);
        assert!(election.alternate_assignments().is_empty());
    }

    submit_transitions(
        &client,
        attempt_id,
        expected_bodies.into_iter().map(|body| {
            ParliamentLifecycleTransitionV1::BeginInvitationAcceptance(
                ParliamentBeginInvitationAcceptanceV1 {
                    election_attempt_id: election_ids[&body],
                },
            )
        }),
    )?;
    let invitation_state = read_attempt(&client, attempt_id)?;
    let common_invitation_close = expected_bodies
        .into_iter()
        .map(|body| {
            invitation_state
                .election(&election_ids[&body])
                .and_then(|election| election.invitation_close_height())
                .expect("invitation deadline is frozen")
        })
        .collect::<Vec<_>>();
    assert!(
        common_invitation_close
            .windows(2)
            .all(|pair| pair[0] == pair[1])
    );
    let mut invitations_by_member =
        BTreeMap::<AccountId, Vec<(ParliamentBody, BodyElectionAttemptId)>>::new();
    for body in expected_bodies {
        let election = invitation_state
            .election(&election_ids[&body])
            .expect("drawn election");
        for assignment in election.primary_assignments() {
            invitations_by_member
                .entry(assignment.member.clone())
                .or_default()
                .push((body, election_ids[&body]));
        }
    }
    for (member, invitations) in invitations_by_member {
        let member_client = client_for(&client, &member, &citizen_keys);
        submit_transitions(
            &member_client,
            attempt_id,
            invitations.into_iter().map(|(body, election_attempt_id)| {
                ParliamentLifecycleTransitionV1::RecordInvitationResponse(
                    ParliamentRecordInvitationResponseV1 {
                        election_attempt_id,
                        body,
                        decision: ParliamentInvitationDecisionV1::Accept,
                    },
                )
            }),
        )?;
    }
    assert!(current_height(&client)? <= common_invitation_close[0]);
    let roster_seal_height = common_invitation_close[0]
        .checked_add(1)
        .ok_or_else(|| eyre!("invitation close height overflow"))?;
    advance_to_predecessor(
        &client,
        roster_seal_height,
        "canonical Parliament roster sealing",
    )?;
    submit_transitions(
        &client,
        attempt_id,
        expected_bodies.into_iter().map(|body| {
            ParliamentLifecycleTransitionV1::SealBodyRoster(ParliamentSealBodyRosterV1 {
                election_attempt_id: election_ids[&body],
            })
        }),
    )?;
    assert_eq!(current_height(&client)?, roster_seal_height);
    let sealed = read_attempt(&client, attempt_id)?;
    let mut body_ids = BTreeMap::<ParliamentBody, BodyInstanceId>::new();
    for body in expected_bodies {
        let instance = sealed
            .sealed_body_for_role(body)
            .expect("every simultaneous draw seals one body");
        assert_eq!(
            instance.instance().status,
            BodyInstanceStatusV1::RosterSealed
        );
        assert_eq!(instance.instance().original_seats, BODY_SEATS);
        assert_eq!(instance.assignments().len(), BODY_SEATS as usize);
        assert_eq!(
            sealed
                .election(&election_ids[&body])
                .expect("election")
                .accepted_assignments()
                .len(),
            BODY_SEATS as usize,
        );
        body_ids.insert(body, instance.instance().id);
    }

    let public_bodies = [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::OversightCommittee,
    ];
    let deliberation_phases = [
        DeliberationPhaseV1::Orientation,
        DeliberationPhaseV1::Evidence,
        DeliberationPhaseV1::Questions,
        DeliberationPhaseV1::Responses,
        DeliberationPhaseV1::Deliberation,
        DeliberationPhaseV1::Reflection,
    ];
    for body in public_bodies {
        let body_id = body_ids[&body];
        submit_transitions(
            &client,
            attempt_id,
            deliberation_phases.into_iter().map(|target| {
                ParliamentLifecycleTransitionV1::AdvanceBodyPhase(ParliamentAdvanceBodyPhaseV1 {
                    body_instance_id: body_id,
                    target,
                })
            }),
        )?;
        let reflecting = read_attempt(&client, attempt_id)?;
        let body_state = reflecting.body(&body_id).expect("reflecting public body");
        assert_eq!(
            body_state.instance().status,
            BodyInstanceStatusV1::Deliberating(DeliberationPhaseV1::Reflection),
        );
        assert_eq!(
            body_state.public_finding_deadline_height(),
            Some(
                body_state
                    .public_finding_opened_at_height()
                    .expect("reflection height")
                    + 20,
            ),
        );
        let members = body_state
            .assignments()
            .iter()
            .map(|assignment| assignment.member.clone())
            .collect::<Vec<_>>();
        let result_root = public_finding_root(attempt_id, body);
        for member in &members[..2] {
            submit_transition(
                &client_for(&client, member, &citizen_keys),
                attempt_id,
                ParliamentLifecycleTransitionV1::EndorsePublicFinding(
                    ParliamentEndorsePublicFindingV1 {
                        body_instance_id: body_id,
                        result_root,
                    },
                ),
            )?;
        }
        let completed = read_attempt(&client, attempt_id)?;
        let body_state = completed.body(&body_id).expect("completed public body");
        assert_eq!(body_state.result_root(), Some(result_root));
        assert_eq!(body_state.instance().status, BodyInstanceStatusV1::Approved);
        assert_eq!(body_state.public_finding_endorsements().len(), 2);
    }

    let policy_body_id = body_ids[&ParliamentBody::PolicyJury];
    submit_transitions(
        &client,
        attempt_id,
        [
            DeliberationPhaseV1::Orientation,
            DeliberationPhaseV1::Evidence,
            DeliberationPhaseV1::Questions,
            DeliberationPhaseV1::Responses,
            DeliberationPhaseV1::Deliberation,
            DeliberationPhaseV1::Reflection,
            DeliberationPhaseV1::Vote,
        ]
        .into_iter()
        .map(|target| {
            ParliamentLifecycleTransitionV1::AdvanceBodyPhase(ParliamentAdvanceBodyPhaseV1 {
                body_instance_id: policy_body_id,
                target,
            })
        }),
    )?;
    let ballot_attempt_id = BallotAttemptId::derive_v1(policy_body_id, 0);
    let registered_at_height = current_height(&client)? + 1;
    let registration_close_height = registered_at_height + REGISTRATION_PHASE_BLOCKS;
    let survivor_freeze_height = registration_close_height + SURVIVOR_PHASE_BLOCKS;
    let commitment_close_height = survivor_freeze_height + COMMITMENT_PHASE_BLOCKS;
    let release_height = commitment_close_height + RELEASE_DELAY_BLOCKS;
    let opening_deadline_height = release_height + OPENING_PHASE_BLOCKS;
    let tle_session_id = TleSessionId::derive_v1(
        ballot_attempt_id,
        tle_public_state.key_session_id,
        logical_beacon,
        release_height,
    );
    submit_transition(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::RegisterBallotAttempt(ParliamentRegisterBallotAttemptV1 {
            body_instance_id: policy_body_id,
            ballot_attempt_id,
            sequence: 0,
            tle_session_id,
            tle_key_session_id: tle_public_state.key_session_id,
            release_beacon_session_id: logical_beacon,
            release_height,
        }),
    )?;
    assert_eq!(current_height(&client)?, registered_at_height);
    let registered_response = client.get_parliament_timed_ovn_casting_context(ballot_attempt_id)?;
    let registered_archive = casting_archive(&registered_response, ballot_attempt_id)?;
    assert_eq!(
        registered_archive.phase(),
        ParliamentTimedOvnCastingPhaseV1::Registered
    );
    assert!(registered_archive.registration_records().is_empty());
    assert_eq!(registered_archive.target_finalized_height(), release_height);
    let registered_validated = registered_archive
        .validate_v1()
        .wrap_err("validate initial registration archive")?;
    let policy_members = read_attempt(&client, attempt_id)?
        .body(&policy_body_id)
        .expect("Policy Jury body")
        .assignments()
        .iter()
        .map(|assignment| assignment.member.clone())
        .collect::<Vec<_>>();
    assert_eq!(policy_members.len(), BODY_SEATS as usize);
    let mut registration_secrets = BTreeMap::<[u8; 32], TimedOvnRegistrationSecretV1>::new();
    for (index, member) in policy_members.iter().enumerate() {
        let participant_hash = parliament_ballot_participant_hash_v1(ballot_attempt_id, member);
        let mut rng = StdRng::from_seed(
            Hash::new_from_chunks(&[
                b"iroha.integration.parliament.registration-rng.v1\0",
                attempt_id.as_bytes(),
                &[u8::try_from(index)?],
            ])
            .into(),
        );
        let (secret, registration) = TimedOvnRegistrationSecretV1::generate_with_rng(
            registered_validated.timed_ovn_session(),
            participant_hash,
            &mut rng,
        )
        .wrap_err("generate proof-valid timed-OVN registration")?;
        submit_transition(
            &client_for(&client, member, &citizen_keys),
            attempt_id,
            ParliamentLifecycleTransitionV1::RegisterBallotParticipant(
                ParliamentRegisterBallotParticipantV1 {
                    ballot_attempt_id,
                    registration_record: registration.to_bytes(),
                },
            ),
        )?;
        assert!(
            registration_secrets
                .insert(participant_hash, secret)
                .is_none()
        );
    }
    assert!(current_height(&client)? < registration_close_height);
    assert_transition_rejected_without_state_change(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::CloseBallotRegistration(
            ParliamentCloseBallotRegistrationV1 { ballot_attempt_id },
        ),
        "registration close before the frozen exact height",
    )?;
    advance_to_predecessor(
        &client,
        registration_close_height,
        "timed-OVN registration close",
    )?;
    assert_eq!(
        submit_transition(
            &client,
            attempt_id,
            ParliamentLifecycleTransitionV1::CloseBallotRegistration(
                ParliamentCloseBallotRegistrationV1 { ballot_attempt_id },
            ),
        )?,
        registration_close_height,
    );
    let registration_closed = client.get_parliament_timed_ovn_casting_context(ballot_attempt_id)?;
    let registration_closed_archive = casting_archive(&registration_closed, ballot_attempt_id)?;
    assert_eq!(
        registration_closed_archive.phase(),
        ParliamentTimedOvnCastingPhaseV1::RegistrationClosed,
    );
    assert_eq!(
        registration_closed_archive.registration_records().len(),
        BODY_SEATS as usize,
    );
    assert_eq!(
        read_attempt(&client, attempt_id)?
            .ballot(&ballot_attempt_id)
            .expect("registered ballot")
            .registered_voters(),
        Some(BODY_SEATS),
    );
    assert_transition_rejected_without_state_change(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::CloseBallotRegistration(
            ParliamentCloseBallotRegistrationV1 { ballot_attempt_id },
        ),
        "replayed registration close",
    )?;
    assert!(current_height(&client)? < survivor_freeze_height);
    assert_transition_rejected_without_state_change(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::FreezeBallotSurvivors(ParliamentFreezeBallotSurvivorsV1 {
            ballot_attempt_id,
        }),
        "survivor freeze before the frozen exact height",
    )?;
    advance_to_predecessor(&client, survivor_freeze_height, "timed-OVN survivor freeze")?;
    assert_eq!(
        submit_transition(
            &client,
            attempt_id,
            ParliamentLifecycleTransitionV1::FreezeBallotSurvivors(
                ParliamentFreezeBallotSurvivorsV1 { ballot_attempt_id },
            ),
        )?,
        survivor_freeze_height,
    );
    assert_transition_rejected_without_state_change(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::FreezeBallotSurvivors(ParliamentFreezeBallotSurvivorsV1 {
            ballot_attempt_id,
        }),
        "replayed survivor freeze",
    )?;
    let survivors_response = client.get_parliament_timed_ovn_casting_context(ballot_attempt_id)?;
    let survivors_archive = casting_archive(&survivors_response, ballot_attempt_id)?;
    assert_eq!(
        survivors_archive.phase(),
        ParliamentTimedOvnCastingPhaseV1::SurvivorsFrozen,
    );
    let survivor_ids = survivors_archive
        .survivor_participant_hashes()
        .expect("survivor freeze exposes exact public identifiers");
    assert_eq!(survivor_ids.len(), BODY_SEATS as usize);
    let survivors_validated = survivors_archive
        .validate_v1()
        .wrap_err("replay survivor-frozen casting archive")?;
    let prepared = survivors_validated
        .prepared_attempt()
        .expect("survivor-frozen archive prepares the exact ballot roster");
    let mut ballot_records = Vec::with_capacity(survivor_ids.len());
    for (index, participant_hash) in survivor_ids.iter().enumerate() {
        let choice = if index < 2 {
            TimedOvnChoiceV1::Aye
        } else {
            TimedOvnChoiceV1::Nay
        };
        let mut rng = StdRng::from_seed(
            Hash::new_from_chunks(&[
                b"iroha.integration.parliament.ballot-rng.v1\0",
                attempt_id.as_bytes(),
                &[u8::try_from(index)?],
            ])
            .into(),
        );
        let record = registration_secrets
            .get(participant_hash)
            .ok_or_else(|| eyre!("frozen survivor has no locally retained secret"))?
            .cast_ballot_with_rng(prepared.survivor_roster(), choice, &mut rng)
            .wrap_err("generate proof-valid masked timed-OVN ballot")?
            .to_bytes();
        assert_eq!(record.len(), TIMED_OVN_BALLOT_RECORD_BYTES_V1);
        ballot_records.push(record);
    }
    assert!(current_height(&client)? < commitment_close_height);
    assert_transition_rejected_without_state_change(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::FreezeTimedOvnCorpus(ParliamentFreezeTimedOvnCorpusV1 {
            ballot_attempt_id,
            ballot_records: ballot_records.clone(),
        }),
        "timed-OVN corpus freeze before the frozen exact height",
    )?;
    advance_to_predecessor(
        &client,
        commitment_close_height,
        "timed-OVN commitment close",
    )?;
    assert_eq!(
        submit_transition(
            &client,
            attempt_id,
            ParliamentLifecycleTransitionV1::FreezeTimedOvnCorpus(
                ParliamentFreezeTimedOvnCorpusV1 {
                    ballot_attempt_id,
                    ballot_records: ballot_records.clone(),
                },
            ),
        )?,
        commitment_close_height,
    );
    let committed_attempt = read_attempt(&client, attempt_id)?;
    let committed_ballot = committed_attempt
        .ballot(&ballot_attempt_id)
        .expect("committed hidden ballot");
    assert_eq!(committed_ballot.accepted_ballots(), Some(BODY_SEATS));
    assert!(committed_ballot.corpus_root().is_some());
    assert_eq!(
        committed_ballot.attempt().status,
        BallotAttemptStatusV1::AwaitingRelease,
    );
    assert!(
        client
            .get_parliament_timed_ovn_casting_context(ballot_attempt_id)
            .is_err(),
        "a sealed corpus is no longer a cast-capable context",
    );
    assert_transition_rejected_without_state_change(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::FreezeTimedOvnCorpus(ParliamentFreezeTimedOvnCorpusV1 {
            ballot_attempt_id,
            ballot_records,
        }),
        "replayed timed-OVN corpus freeze",
    )?;
    assert_transition_rejected_without_state_change(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::BeginBallotOpeningBatch(
            ParliamentBeginBallotOpeningBatchV1 {
                ballot_attempt_ids: vec![ballot_attempt_id],
                release_beacon_session_id: logical_beacon,
                release_height,
                pulse_id: BeaconPulseId::new([0xE1; 32]),
            },
        ),
        "ballot opening before the frozen release height and authoritative pulse",
    )?;

    advance_to_predecessor(&client, release_height, "timed-OVN release pulse")?;
    network.ensure_blocks(release_height).await?;
    assert_eq!(
        current_height(&client)?,
        release_height,
        "the demanded ballot-release threshold-beacon effect must autonomously finalize its exact height",
    );
    let release_pulses = network
        .peers()
        .iter()
        .map(|peer| pulse_at(&peer.client(), release_height))
        .collect::<Result<Vec<_>>>()?;
    assert!(release_pulses.windows(2).all(|pair| pair[0] == pair[1]));
    let release_pulse = release_pulses[0].clone();
    assert_eq!(release_pulse.height, release_height);
    assert_eq!(release_pulse.session_id, beacon_record.session.session_id);
    assert_eq!(release_pulse.roster_hash, beacon_record.session.roster_hash);
    assert_eq!(
        release_pulse.transcript_hash,
        beacon_record.session.transcript_hash,
    );
    assert_eq!(release_pulse.round, 0, "V1 pulses are view-independent");
    assert_eq!(
        release_pulse.finalized_chain_anchor.height.checked_add(1),
        Some(release_height),
    );
    assert_eq!(
        exact_block(&client, release_pulse.finalized_chain_anchor.height)?
            .header()
            .hash(),
        release_pulse.finalized_chain_anchor.block_hash,
    );
    verify_finalized_global_threshold_beacon_pulse_v1(
        &validated_beacon_session,
        &release_pulse,
        release_pulse.finalized_chain_anchor,
    )
    .wrap_err("independently verify the ballot-release pulse threshold signature")?;
    assert_ne!(release_pulse.pulse_id, sortition_pulse.pulse_id);
    submit_transition(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::BeginBallotOpeningBatch(
            ParliamentBeginBallotOpeningBatchV1 {
                ballot_attempt_ids: vec![ballot_attempt_id],
                release_beacon_session_id: logical_beacon,
                release_height,
                pulse_id: BeaconPulseId::new(release_pulse.pulse_id),
            },
        ),
    )?;
    let opening_height = current_height(&client)?;
    assert!(opening_height <= opening_deadline_height);
    assert_transition_rejected_without_state_change(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::BeginBallotOpeningBatch(
            ParliamentBeginBallotOpeningBatchV1 {
                ballot_attempt_ids: vec![ballot_attempt_id],
                release_beacon_session_id: logical_beacon,
                release_height,
                pulse_id: BeaconPulseId::new(release_pulse.pulse_id),
            },
        ),
        "replayed ballot opening",
    )?;
    network.ensure_blocks(opening_height).await?;
    let release_context = client.get_parliament_tle_release_context(ballot_attempt_id)?;
    let validated_release = release_projection(&release_context)?
        .validate()
        .wrap_err("replay full public TLE transcript and release identity")?;
    assert_eq!(release_context.release_height, release_height);
    assert_eq!(
        release_context.opening_deadline_height,
        opening_deadline_height
    );
    assert_eq!(release_context.status, BallotAttemptStatusV1::Opening);
    assert_eq!(
        release_context.tle_key_session.threshold, 2,
        "the four-validator test fixture uses an independently verified 2-of-4 release threshold",
    );
    let mut verified_partials = BTreeMap::<u16, TlePartialReleaseShareV1>::new();
    for peer in network.peers() {
        let peer_client = peer.client();
        let peer_context = peer_client.get_parliament_tle_release_context(ballot_attempt_id)?;
        assert_eq!(peer_context, release_context);
        let partial =
            release_partial(peer_client.post_parliament_tle_partial_release(&peer_context)?);
        validated_release
            .session()
            .verify_partial_release(
                validated_release.identity(),
                validated_release.finalized_height(),
                &partial,
            )
            .wrap_err("independently verify one proof-carrying validator release share")?;
        let participant_index = partial.participant_index;
        if let Some(previous) = verified_partials.insert(participant_index, partial.clone()) {
            assert_eq!(previous, partial, "a validator seat may not equivocate");
        }
    }
    assert_eq!(verified_partials.len(), VALIDATOR_COUNT);
    let canonical_threshold = verified_partials
        .into_values()
        .take(usize::from(release_context.tle_key_session.threshold))
        .collect::<Vec<_>>();
    let final_release = validated_release
        .session()
        .combine_partial_releases(
            validated_release.identity(),
            validated_release.finalized_height(),
            &canonical_threshold,
        )
        .wrap_err("combine canonical threshold of verified release shares")?;
    validated_release
        .session()
        .verify_final_release(
            validated_release.identity(),
            validated_release.finalized_height(),
            &final_release,
        )
        .wrap_err("independently verify combined final release")?;
    let parliament_final_release = ParliamentTleFinalReleaseSignatureV1 {
        key_session_id: final_release.key_session_id,
        identity_digest: final_release.identity_digest,
        signature: final_release.signature,
    };
    submit_transition(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(ParliamentFinalizeOpenedBallotV1 {
            ballot_attempt_id,
            final_release: parliament_final_release,
        }),
    )?;
    let certified = read_attempt(&client, attempt_id)?;
    let certificate = certified
        .certificate()
        .cloned()
        .expect("Core constructs a certificate atomically with the approved final aggregate");
    certificate
        .validate()
        .wrap_err("revalidate the complete Core-constructed Parliament certificate")?;
    assert_eq!(
        certified.attempt().status,
        GovernanceAttemptStatusV1::Certified
    );
    assert_eq!(certified.attempt().stage, GovernanceStageV1::Enactment);
    assert_eq!(certificate.certified_at_height, current_height(&client)?);
    assert_eq!(
        certificate.enact_at_height,
        certificate.certified_at_height + MIN_ENACTMENT_DELAY,
    );
    assert_eq!(certificate.body_bindings.len(), expected_bodies.len());
    let policy_binding = certificate
        .body_bindings
        .iter()
        .find(|binding| binding.body == ParliamentBody::PolicyJury)
        .expect("certificate carries exactly one Policy Jury binding");
    let ballot_binding = policy_binding
        .ballot
        .expect("Policy Jury certificate binding is mandatory and private");
    assert_eq!(ballot_binding.ballot_attempt_id, ballot_attempt_id);
    assert_eq!(ballot_binding.registered_at_height, registered_at_height);
    assert_eq!(
        ballot_binding.registration_close_height,
        registration_close_height
    );
    assert_eq!(
        ballot_binding.survivor_freeze_height,
        survivor_freeze_height
    );
    assert_eq!(
        ballot_binding.commitment_close_height,
        commitment_close_height
    );
    assert_eq!(
        ballot_binding.registration_closed_at_height,
        registration_close_height,
    );
    assert_eq!(
        ballot_binding.survivors_frozen_at_height,
        survivor_freeze_height,
    );
    assert_eq!(
        ballot_binding.commitment_closed_at_height,
        commitment_close_height,
    );
    assert_eq!(ballot_binding.release_height, release_height);
    assert_eq!(
        ballot_binding.opening_deadline_height,
        opening_deadline_height,
    );
    assert_eq!(ballot_binding.max_ballot_retries, 0);
    assert_eq!(ballot_binding.max_corpus_entries, 8);
    assert_eq!(ballot_binding.tally.accepted_ballots, BODY_SEATS);
    assert_eq!(ballot_binding.tally.aye, 2);
    assert_eq!(ballot_binding.tally.nay, 1);
    assert_eq!(ballot_binding.tally.abstain, 0);
    assert_eq!(
        ballot_binding.outcome,
        ParliamentAggregateOutcomeV1::Approved
    );
    assert_eq!(ballot_binding.opening_height, opening_height);
    assert_eq!(
        ballot_binding.release_pulse_id,
        BeaconPulseId::new(release_pulse.pulse_id),
    );
    assert!(
        certificate
            .body_bindings
            .iter()
            .filter(|binding| binding.body != ParliamentBody::PolicyJury)
            .all(|binding| {
                binding.public_finding.as_ref().is_some_and(|finding| {
                    finding.endorsements == 2
                        && finding.quorum == 2
                        && finding.endorsing_assignments.len() == 2
                }) && binding.ballot.is_none()
            }),
    );
    assert_transition_rejected_without_state_change(
        &client,
        attempt_id,
        ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(ParliamentFinalizeOpenedBallotV1 {
            ballot_attempt_id,
            final_release: parliament_final_release,
        }),
        "replayed aggregate ballot finalization",
    )?;

    advance_to_predecessor(
        &client,
        certificate.enact_at_height,
        "automatic exact-height Parliament enactment",
    )?;
    assert_eq!(
        tick(&client, "drive automatic due-certificate execution")?,
        certificate.enact_at_height,
    );
    let enacted_height = certificate.enact_at_height;
    network.ensure_blocks(enacted_height).await?;
    let enacted_response = client.get_parliament_attempt(attempt_id)?;
    let enacted = read_attempt(&client, attempt_id)?;
    assert_eq!(enacted.attempt().status, GovernanceAttemptStatusV1::Enacted);
    assert_eq!(enacted.attempt().stage, GovernanceStageV1::Enactment);
    assert_eq!(enacted.terminal_height(), Some(enacted_height));
    assert_eq!(enacted.certificate(), Some(&certificate));
    client
        .get_gov_contract_json(&contract_address)
        .wrap_err("consensus-owned certificate enactment must bind the staged contract")?;

    let peer_blocks = network
        .peers()
        .iter()
        .map(|peer| exact_block(&peer.client(), enacted_height))
        .collect::<Result<Vec<_>>>()?;
    assert!(
        peer_blocks
            .windows(2)
            .all(|pair| pair[0].hash() == pair[1].hash()),
        "all four validators must finalize the same exact enactment block",
    );
    let enacted_height_nonzero =
        NonZeroU64::new(enacted_height).expect("a Parliament enactment cannot be genesis");
    for (peer, block) in network.peers().iter().zip(&peer_blocks) {
        let (proof, verified_hash) = peer
            .client()
            .get_bridge_finality_anchor(enacted_height_nonzero, network.network_id())
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
            SumeragiV2GenesisContextParameters::recommended().da_layout,
            "every enactment proof must retain the signed revision-4 RS16 DA layout",
        );
    }
    for peer in network.peers() {
        let peer_client = peer.client();
        let response = peer_client.get_parliament_attempt(attempt_id)?;
        assert_eq!(response.current_height, enacted_height);
        assert_eq!(response.attempt.status, GovernanceAttemptStatusV1::Enacted);
        assert_eq!(
            response.state_payload_hex,
            enacted_response.state_payload_hex
        );
        peer_client
            .get_gov_contract_json(&contract_address)
            .wrap_err("every validator must expose the consensus-enacted contract")?;
        let status = peer_client.get_sumeragi_status()?;
        status
            .validate()
            .map_err(|error| eyre!("invalid enacted Parliament peer status: {error}"))?;
        assert!(
            !status.restart_required,
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
    let restarted_response = restart_peer.client().get_parliament_attempt(attempt_id)?;
    assert_eq!(
        restarted_response.attempt.status,
        GovernanceAttemptStatusV1::Enacted
    );
    assert_eq!(
        restarted_response.state_payload_hex, enacted_response.state_payload_hex,
        "normal restart must restore the complete reducer/certificate state",
    );
    let restarted_block = exact_block(&restart_peer.client(), enacted_height)?;
    assert_eq!(restarted_block.hash(), peer_blocks[0].hash());
    restart_peer
        .client()
        .get_gov_contract_json(&contract_address)
        .wrap_err("normal restart must restore the consensus-enacted contract")?;
    let restarted_status = restart_peer.client().get_sumeragi_status()?;
    restarted_status
        .validate()
        .map_err(|error| eyre!("invalid restarted Parliament peer status: {error}"))?;
    assert!(
        !restarted_status.restart_required,
        "normal restart must restore a live non-fail-stopped consensus reducer",
    );
    exercise_public_finding_impossible_quorum_retry(
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
    npos.vrf_commit_window_blocks = 2;
    npos.vrf_reveal_window_blocks = 2;
    npos.validate()
        .map_err(|error| eyre!("invalid mandatory NPoS fixture: {error}"))?;

    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus()
        .with_parliament_beacon_signer_modes(POSITIVE_BEACON_SIGNER_MODES)
        .with_block_cadence(Duration::from_secs(1))
        .with_config_layer(|layer| {
            layer
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
    let ordered_roster = ordered_validator_roster(&network)?;
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

    let install_height =
        (current_height(&client)? + 1).max(beacon_record.session.adaptive_dkg.finalized_at_height);
    advance_to_predecessor(&client, install_height, "mandatory beacon-key installation")?;
    client.submit_blocking(
        lifecycle_certificate(
            &network,
            &ordered_roster,
            ThresholdKeyLifecycleActionV1::InstallGlobalBeaconKey,
            beacon_record.session.session_id,
            beacon_record.session.transcript_hash,
            norito::encode_canonical(&beacon_record)?,
            install_height,
        )?,
        fee(),
    )?;
    assert_eq!(current_height(&client)?, install_height);
    tick(&client, "activate mandatory NPoS beacon session")?;

    let boundary_height = MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS;
    let pulse_height = boundary_height - 1;
    advance_to_predecessor(&client, pulse_height, "mandatory pre-boundary pulse")?;
    assert!(
        pulse_at(&client, pulse_height - 1).is_err(),
        "an unrequested non-boundary height must not emit a global pulse"
    );
    network.ensure_blocks(pulse_height).await?;
    assert_eq!(
        current_height(&client)?,
        pulse_height,
        "the mandatory threshold-beacon effect must autonomously finalize its exact pre-boundary height",
    );

    let pulses = network
        .peers()
        .iter()
        .map(|peer| pulse_at(&peer.client(), pulse_height))
        .collect::<Result<Vec<_>>>()?;
    assert!(pulses.windows(2).all(|pair| pair[0] == pair[1]));
    let pulse = &pulses[0];
    assert_eq!(pulse.height, pulse_height);
    assert_eq!(pulse.session_id, beacon_record.session.session_id);
    assert_eq!(pulse.roster_hash, beacon_record.session.roster_hash);
    assert_eq!(pulse.transcript_hash, beacon_record.session.transcript_hash);
    assert_eq!(pulse.round, 0, "the mandatory pulse is view-independent");
    assert_eq!(pulse.finalized_chain_anchor.height + 1, pulse_height);
    assert_eq!(
        exact_block(&client, pulse.finalized_chain_anchor.height)?
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
    assert_eq!(
        tick(&client, "commit mandatory NPoS boundary")?,
        boundary_height
    );
    assert_eq!(
        tick(&client, "prove successor epoch can finalize")?,
        boundary_height + 1
    );
    network.ensure_blocks(boundary_height + 1).await?;
    for peer in network.peers() {
        let status = peer.client().get_sumeragi_status()?;
        status
            .validate()
            .map_err(|error| eyre!("invalid successor NPoS status: {error}"))?;
        assert!(
            !status.restart_required,
            "a successful mandatory beacon transition must not fail-stop a validator",
        );
        assert!(status.last_committed_height >= boundary_height + 1);
        assert_eq!(status.height_context.epoch, successor_epoch);
        assert_eq!(status.height_context.epoch_seed, successor_seed);
        assert_eq!(
            status.height_context.epoch_end_height,
            boundary_height + MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS,
        );
    }

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
    npos.vrf_commit_window_blocks = 2;
    npos.vrf_reveal_window_blocks = 2;
    npos.validate()
        .map_err(|error| eyre!("invalid fail-closed NPoS fixture: {error}"))?;

    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus()
        .with_parliament_beacon_signer_modes(FAIL_CLOSED_BEACON_SIGNER_MODES)
        .with_block_cadence(Duration::from_secs(1))
        .with_config_layer(|layer| {
            layer
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
    let ordered_roster = ordered_validator_roster(&network)?;
    let beacon_record =
        deterministic_parliament_beacon_key_record_v1(network.network_id(), &ordered_roster)
            .wrap_err("derive fail-closed NPoS beacon fixture")?;
    assert_eq!(beacon_record.session.committee_size, 4);
    assert_eq!(beacon_record.session.threshold, 2);
    let install_height =
        (current_height(&client)? + 1).max(beacon_record.session.adaptive_dkg.finalized_at_height);
    advance_to_predecessor(
        &client,
        install_height,
        "fail-closed beacon-key installation",
    )?;
    client.submit_blocking(
        lifecycle_certificate(
            &network,
            &ordered_roster,
            ThresholdKeyLifecycleActionV1::InstallGlobalBeaconKey,
            beacon_record.session.session_id,
            beacon_record.session.transcript_hash,
            norito::encode_canonical(&beacon_record)?,
            install_height,
        )?,
        fee(),
    )?;
    assert_eq!(current_height(&client)?, install_height);
    tick(&client, "activate fail-closed NPoS beacon session")?;

    let pulse_height = MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS - 1;
    advance_to_predecessor(&client, pulse_height, "fail-closed pre-boundary pulse")?;
    let predecessor_height = pulse_height - 1;
    assert_eq!(current_height(&client)?, predecessor_height);
    let unexpected_pulse_height = tokio::time::timeout(
        FAIL_CLOSED_BEACON_OBSERVATION_WINDOW,
        network.peers()[0].once_block(pulse_height),
    )
    .await;
    assert!(
        unexpected_pulse_height.is_err(),
        "one valid share plus one proof-invalid share must not satisfy the exact threshold of two",
    );

    for peer in network.peers() {
        assert!(
            peer.is_running(),
            "the beacon-share fault must not stop a consensus validator",
        );
        let peer_client = peer.client();
        assert_eq!(
            current_height(&peer_client)?,
            predecessor_height,
            "the mandatory pre-boundary height must remain uncommitted below threshold",
        );
        let status = peer_client.get_sumeragi_status()?;
        status
            .validate()
            .map_err(|error| eyre!("invalid fail-closed NPoS status: {error}"))?;
        assert!(
            !status.restart_required,
            "below-threshold beacon liveness must stall without fail-stopping consensus",
        );
        assert_eq!(status.last_committed_height, predecessor_height);
        assert_eq!(status.height, pulse_height);
    }

    network.shutdown().await;
    Ok(())
}

#[test]
fn parliament_network_corridor_has_no_legacy_or_consensus_bypass_surface() {
    let source = include_str!("sora_parliament_lifecycle_smoke.rs");
    let forbidden = [
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
        "SumeragiV2GenesisContextParameters::recommended()",
        ".da_layout"
    )));
    let boundary_helper = concat!("assert_transition_rejected_without_state_", "change(");
    assert_eq!(
        source.matches(boundary_helper).count(),
        10,
        "the helper definition plus nine exact checkpoint/replay calls must remain",
    );
    for required in [
        concat!("ConsumeSortition", "PulseBatch"),
        concat!("RegisterBallot", "Participant"),
        concat!("CloseBallot", "Registration"),
        concat!("FreezeBallot", "Survivors"),
        concat!("FreezeTimedOvn", "Corpus"),
        concat!("combine_partial_", "releases"),
        concat!("FinalizeOpened", "Ballot"),
        concat!("GovernanceAttemptStatusV1::", "Enacted"),
        concat!("exercise_public_finding_", "impossible_quorum_retry"),
        concat!(
            "ParliamentNoResultKindV1::",
            "PublicFindingQuorumUnreachable"
        ),
        concat!("attempt_sequence:", " 1"),
        concat!("shutdown_if_", "started"),
        concat!("start_", "checked"),
    ] {
        assert!(
            source.contains(required),
            "modern Parliament corridor lost required operation `{required}`",
        );
    }
}
