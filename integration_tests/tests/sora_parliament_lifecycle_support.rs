//! Shared real Parliament corridor, rooted in independently validated native genesis.
//! The publication harness imports these helpers without collecting unrelated beacon scenarios.

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

pub(super) const VALIDATOR_COUNT: usize = 4;
pub(super) const CITIZEN_COUNT: usize = 24;
pub(super) const BODY_SEATS: u32 = 3;
// An isolated ordinary transaction executes in the next certified global carrier.
pub(super) const EXECUTION_CARRIER_BLOCKS: u64 = 1;
// Keep real FastPQ proving enabled, but prevent four local debug daemons from
// each provisioning a wide Rayon pool on the same host while consensus traffic is live.
pub(super) const PARLIAMENT_NETWORK_RAYON_THREADS_PER_PEER: i64 = 2;
// Six three-seat bodies can draw eighteen distinct invitees. Keep enough native
// execution heights for separately signed responses and the exact roster seal.
pub(super) const INVITATION_PHASE_BLOCKS: u64 = 56;
// The corridor executes three proof-valid registrations and one proof-invalid
// early close before the exact close, each in its own certified carrier.
pub(super) const REGISTRATION_PHASE_BLOCKS: u64 = 15;
pub(super) const SURVIVOR_PHASE_BLOCKS: u64 = 9;
// A replayed survivor freeze and an early corpus freeze each consume one full
// native transaction lifecycle before the exact corpus-freeze authority point.
pub(super) const COMMITMENT_PHASE_BLOCKS: u64 = 9;
// The replayed corpus freeze and wrong-pulse opening must terminate before the
// autonomous release pulse reaches its exact height.
pub(super) const RELEASE_DELAY_BLOCKS: u64 = 7;
pub(super) const OPENING_PHASE_BLOCKS: u64 = 9;
pub(super) const MIN_ENACTMENT_DELAY: u64 = 4;
pub(super) const MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS: u64 = 8;
// Exact-roster certificates bind their containing height. Leave time to sign
// and submit against the immediately preceding certified State.
pub(super) const EXACT_HEIGHT_SUBMISSION_CADENCE: Duration = Duration::from_secs(5);
pub(super) const PARLIAMENT_NETWORK_STACK_BYTES: usize = 32 * 1024 * 1024;
pub(super) const TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES: i64 = 1_073_741_824;
pub(super) const OPERATION_TIMEOUT: Duration = Duration::from_secs(300);
pub(super) const FAIL_CLOSED_BEACON_OBSERVATION_WINDOW: Duration = Duration::from_secs(8);
pub(super) const POSITIVE_BEACON_SIGNER_MODES: [ParliamentBeaconSignerMode; VALIDATOR_COUNT] = [
    ParliamentBeaconSignerMode::Valid,
    ParliamentBeaconSignerMode::Valid,
    ParliamentBeaconSignerMode::Absent,
    ParliamentBeaconSignerMode::Invalid,
];
pub(super) const FAIL_CLOSED_BEACON_SIGNER_MODES: [ParliamentBeaconSignerMode; VALIDATOR_COUNT] = [
    ParliamentBeaconSignerMode::Valid,
    ParliamentBeaconSignerMode::Absent,
    ParliamentBeaconSignerMode::Absent,
    ParliamentBeaconSignerMode::Invalid,
];
pub(super) const CONTRACT_ADDRESS: &str =
    "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw";
pub(super) const NO_RESULT_RETRY_CONTRACT_ADDRESS: &str =
    "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh";

pub(super) fn fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(Vec::new(), None)
}

// The blocking client is retained only as the test network's account/configuration
// holder. All writes use its current account-owned async signing and finality API.
pub(super) async fn prepare_parliament_transaction(
    client: &Client,
    instructions: impl IntoIterator<Item = impl Into<InstructionBox>>,
) -> Result<SignedTransaction> {
    let account = client.account_client();
    let mut payload = account.prepare_transaction(AccountTransactionDraft::new(
        instructions.into_iter().map(Into::into).collect::<Vec<_>>(),
        fee(),
        Metadata::default(),
    ))?;
    let quote = tokio::time::timeout(
        OPERATION_TIMEOUT,
        account.quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload }),
    )
    .await
    .map_err(|_| eyre!("Parliament fee quote exceeded {OPERATION_TIMEOUT:?}"))??;
    if !payload
        .fee_payment
        .has_same_payer_and_gas_bound(&quote.intent)
    {
        return Err(eyre!("Parliament fee quote changed payer or gas bound"));
    }
    payload.fee_payment = quote.intent;
    Ok(account.sign_transaction(payload)?)
}

pub(super) async fn submit_parliament_instructions(
    client: &Client,
    instructions: impl IntoIterator<Item = impl Into<InstructionBox>>,
) -> Result<()> {
    let transaction = prepare_parliament_transaction(client, instructions).await?;
    let applied_hash = tokio::time::timeout(
        OPERATION_TIMEOUT,
        client
            .account_client()
            .submit_transaction_and_wait(&transaction),
    )
    .await
    .map_err(|_| eyre!("Parliament Applied finality exceeded {OPERATION_TIMEOUT:?}"))??;
    if applied_hash != transaction.hash() {
        return Err(eyre!(
            "Parliament Applied response substituted the signed transaction hash"
        ));
    }
    Ok(())
}

// The caller observes the exact certified carrier height after native admission.
pub(super) async fn admit_parliament_height_carrier(
    client: &Client,
    instructions: [Log; 1],
) -> Result<()> {
    let transaction = prepare_parliament_transaction(client, instructions).await?;
    let admitted_hash = tokio::time::timeout(
        OPERATION_TIMEOUT,
        client.account_client().submit_transaction(&transaction),
    )
    .await
    .map_err(|_| eyre!("Parliament carrier admission exceeded {OPERATION_TIMEOUT:?}"))??;
    if admitted_hash != transaction.hash() {
        return Err(eyre!(
            "Parliament carrier admission substituted the signed hash"
        ));
    }
    Ok(())
}

pub(super) fn minimal_contract_artifact() -> Vec<u8> {
    minimal_contract_artifact_with_identity("ParliamentLifecycleSmoke", "integration-tests")
}

pub(super) fn minimal_contract_artifact_with_identity(
    seiyaku_name: &str,
    compiler_fingerprint: &str,
) -> Vec<u8> {
    let metadata = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 1_000,
        abi_version: 1,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: seiyaku_name.to_owned(),
        compiler_fingerprint: compiler_fingerprint.to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: "main".to_owned(),
            kind: iroha::data_model::smart_contract::manifest::EntryPointKind::View,
            params: Vec::new(),
            argument_schema: None,
            return_type: Some("()".to_owned()),
            return_schema: Some(iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1 {
                nodes: vec![iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeNodeV1::Unit],
            }),
            permission: None,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        }],
        error_types: Vec::new(),
        states: Vec::new(),
    };
    let mut artifact = metadata.encode();
    artifact.extend_from_slice(&interface.encode_section());
    artifact.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    artifact
}

pub(super) fn citizen_keys() -> Vec<KeyPair> {
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

pub(super) fn citizen_accounts(keys: &[KeyPair]) -> Vec<AccountId> {
    let mut accounts = keys
        .iter()
        .map(|key| AccountId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    accounts.sort_unstable();
    accounts
}

pub(super) fn client_for(base: &Client, account: &AccountId, keys: &[KeyPair]) -> Client {
    let key = keys
        .iter()
        .find(|key| {
            account
                .try_signatory()
                .is_some_and(|signatory| key.public_key() == signatory)
        })
        .expect("selected citizen owns one deterministic key")
        .clone();
    integration_tests::sync::rebind_blocking_client(base, |builder| {
        builder.account = account.clone();
        builder.key_pair = key;
    })
}

pub(super) async fn current_height(client: &Client) -> Result<u64> {
    Ok(client.client().status().get().await?.blocks)
}

pub(super) async fn tick(client: &Client, label: impl Into<String>) -> Result<u64> {
    submit_parliament_instructions(&client, [Log::new(Level::INFO, label.into())]).await?;
    current_height(client).await
}

pub(super) async fn next_execution_height(
    client: &Client,
    minimum_height: u64,
    label: &str,
) -> Result<u64> {
    loop {
        let authority_height = current_height(client).await?;
        let execution_height = authority_height
            .checked_add(EXECUTION_CARRIER_BLOCKS)
            .ok_or_else(|| eyre!("{label}: native transaction execution height overflow"))?;
        if execution_height >= minimum_height {
            return Ok(execution_height);
        }
        tick(
            client,
            format!("{label} authority-height tick {}", authority_height + 1),
        )
        .await?;
    }
}

pub(super) async fn advance_to_height_with_carriers(
    network: &iroha_test_network::Network,
    client: &Client,
    target_height: u64,
    label: &str,
) -> Result<()> {
    loop {
        let height = current_height(client).await?;
        if height == target_height {
            return Ok(());
        }
        if height > target_height {
            return Err(eyre!(
                "{label}: exact height {target_height} passed at finalized height {height}"
            ));
        }
        let carrier_height = height
            .checked_add(1)
            .ok_or_else(|| eyre!("{label}: native transaction carrier height overflow"))?;
        admit_parliament_height_carrier(
            &client,
            [Log::new(
                Level::INFO,
                format!("{label} admission carrier {carrier_height}"),
            )],
        )
        .await?;
        network.ensure_blocks(carrier_height).await?;
        let observed_height = current_height(client).await?;
        if observed_height != carrier_height {
            return Err(eyre!(
                "{label}: native transaction carrier expected exact height {carrier_height}, observed {observed_height}"
            ));
        }
    }
}

pub(super) async fn advance_to_execution_predecessor(
    network: &iroha_test_network::Network,
    client: &Client,
    execution_height: u64,
    label: &str,
) -> Result<()> {
    let authority_height = execution_height
        .checked_sub(EXECUTION_CARRIER_BLOCKS)
        .ok_or_else(|| {
            eyre!(
                "{label}: native transaction execution height {execution_height} has no H - {EXECUTION_CARRIER_BLOCKS} authority"
            )
        })?;
    advance_to_height_with_carriers(network, client, authority_height, label).await
}

pub(super) async fn advance_to_autonomous_predecessor(
    network: &iroha_test_network::Network,
    client: &Client,
    target_height: u64,
    label: &str,
) -> Result<()> {
    let predecessor_height = target_height.checked_sub(1).ok_or_else(|| {
        eyre!("{label}: autonomous target height {target_height} has no predecessor")
    })?;
    advance_to_height_with_carriers(network, client, predecessor_height, label).await
}

pub(super) async fn submit_transition(
    client: &Client,
    attempt_id: GovernanceAttemptId,
    transition: ParliamentLifecycleTransitionV1,
) -> Result<u64> {
    submit_parliament_instructions(
        &client,
        [SubmitParliamentLifecycleTransitionV1 {
            governance_attempt_id: attempt_id,
            transition,
        }],
    )
    .await?;
    current_height(client).await
}

pub(super) async fn submit_transitions(
    client: &Client,
    attempt_id: GovernanceAttemptId,
    transitions: impl IntoIterator<Item = ParliamentLifecycleTransitionV1>,
) -> Result<u64> {
    submit_parliament_instructions(
        &client,
        transitions.into_iter().map(|transition| {
            InstructionBox::from(SubmitParliamentLifecycleTransitionV1 {
                governance_attempt_id: attempt_id,
                transition,
            })
        }),
    )
    .await?;
    current_height(client).await
}

pub(super) async fn assert_transition_rejected_without_state_change(
    client: &Client,
    attempt_id: GovernanceAttemptId,
    transition: ParliamentLifecycleTransitionV1,
    label: &str,
) -> Result<()> {
    let before = read_on_dedicated_thread({
        let client = client.client().clone();
        let attempt_id = (attempt_id).clone();
        move || client.get_parliament_attempt(attempt_id)
    })
    .await?
    .state_payload_hex;
    if submit_parliament_instructions(
        &client,
        [SubmitParliamentLifecycleTransitionV1 {
            governance_attempt_id: attempt_id,
            transition,
        }],
    )
    .await
    .is_ok()
    {
        return Err(eyre!("{label}: invalid Parliament transition was accepted"));
    }
    let after = read_on_dedicated_thread({
        let client = client.client().clone();
        let attempt_id = (attempt_id).clone();
        move || client.get_parliament_attempt(attempt_id)
    })
    .await?
    .state_payload_hex;
    if after != before {
        return Err(eyre!(
            "{label}: rejected Parliament transition mutated reducer state"
        ));
    }
    Ok(())
}

pub(super) async fn assert_governed_contract_absent(
    client: &Client,
    contract_address: &ContractAddress,
    label: &str,
) -> Result<()> {
    let response = read_on_dedicated_thread({
        let client = client.client().clone();
        let contract_address = (contract_address).clone();
        move || client.get_gov_contract_response(&contract_address)
    })
    .await
    .wrap_err_with(|| format!("{label}: inactive governed-contract lookup failed"))?;
    if response.status() != iroha::http::StatusCode::OK {
        return Err(eyre!(
            "{label}: expected governed-contract HTTP 200, observed {}",
            response.status(),
        ));
    }
    let projection: norito::json::Value = norito::json::from_slice(response.body())
        .wrap_err_with(|| format!("{label}: inactive governed-contract response is not JSON"))?;
    let object = projection
        .as_object()
        .ok_or_else(|| eyre!("{label}: inactive governed-contract response is not an object"))?;
    if object.len() != 3
        || object.get("found").and_then(norito::json::Value::as_bool) != Some(false)
        || object
            .get("contract_address")
            .and_then(norito::json::Value::as_str)
            != Some(contract_address.as_ref())
        || object
            .get("dataspace")
            .and_then(norito::json::Value::as_str)
            != Some("universal")
    {
        return Err(eyre!(
            "{label}: expected the exact inactive governed-contract projection, got {object:?}"
        ));
    }
    Ok(())
}

pub(super) async fn assert_governed_contract_binding(
    client: &Client,
    contract_address: &ContractAddress,
    expected_code_hash: ContractCodeHash,
    expected_abi_hash: ContractAbiHash,
    label: &str,
) -> Result<()> {
    let response = read_on_dedicated_thread({
        let client = client.client().clone();
        let contract_address = (contract_address).clone();
        move || client.get_gov_contract_json(&contract_address)
    })
    .await
    .wrap_err_with(|| format!("{label}: active governed-contract lookup failed"))?;
    let object = response
        .as_object()
        .ok_or_else(|| eyre!("{label}: active governed-contract response is not an object"))?;
    let expected_subject = contract_address.subject_id().to_string();
    let expected_code_hash = expected_code_hash.to_hex();
    let expected_abi_hash = expected_abi_hash.to_hex();
    let has_exact_entrypoints = object
        .get("public_entrypoints")
        .and_then(norito::json::Value::as_array)
        .is_some_and(|entrypoints| {
            entrypoints.len() == 1 && entrypoints[0].as_str() == Some("main")
        });
    if object.len() != 7
        || object.get("found").and_then(norito::json::Value::as_bool) != Some(true)
        || object
            .get("contract_address")
            .and_then(norito::json::Value::as_str)
            != Some(contract_address.as_ref())
        || object
            .get("contract_subject_account")
            .and_then(norito::json::Value::as_str)
            != Some(expected_subject.as_str())
        || object
            .get("dataspace")
            .and_then(norito::json::Value::as_str)
            != Some("universal")
        || object
            .get("code_hash_hex")
            .and_then(norito::json::Value::as_str)
            != Some(expected_code_hash.as_str())
        || object
            .get("abi_hash_hex")
            .and_then(norito::json::Value::as_str)
            != Some(expected_abi_hash.as_str())
        || !has_exact_entrypoints
    {
        return Err(eyre!(
            "{label}: expected the exact active governed-contract projection, got {object:?}"
        ));
    }
    Ok(())
}

pub(super) async fn assert_asset_not_found(
    client: &Client,
    asset_id: &AssetId,
    label: &str,
) -> Result<()> {
    let query = FindAssetById::new(asset_id.clone());
    assert_eq!(
        query.asset_id(),
        asset_id,
        "{label}: bind the exact requested asset"
    );
    match read_on_dedicated_thread({
        let client = client.client().clone();
        move || Ok(client.query_single(query))
    })
    .await?
    {
        Err(QueryError::Validation(ValidationFail::QueryFailed(QueryExecutionFail::Find(
            FindError::Asset(missing),
        )))) if missing.as_ref() == asset_id => Ok(()),
        Ok(asset) => Err(eyre!(
            "{label}: expected asset `{asset_id}` to be absent, but the query returned `{}`",
            asset.id()
        )),
        Err(error) => Err(eyre!(
            "{label}: expected typed absence of exact asset `{asset_id}`, got {error:?}"
        )),
    }
}

pub(super) async fn assert_timed_ovn_casting_context_not_castable(
    client: &Client,
    ballot_attempt_id: BallotAttemptId,
    label: &str,
) -> Result<()> {
    let error = read_on_dedicated_thread({
        let client = client.client().clone();
        let ballot_attempt_id = (ballot_attempt_id).clone();
        move || client.get_parliament_timed_ovn_casting_context(ballot_attempt_id)
    })
    .await
    .expect_err("a sealed timed-OVN corpus must not return a casting context");
    let rendered = format!("{error:#}");
    const PHASE_NOT_CASTABLE: &str = "Parliament timed-OVN casting context is not authorized: \
        timed-OVN lifecycle is no longer in a casting phase";
    if !rendered.contains("400 Bad Request") || !rendered.contains(PHASE_NOT_CASTABLE) {
        return Err(eyre!(
            "{label}: expected the exact sealed-corpus casting-context rejection, got {rendered}"
        ));
    }
    Ok(())
}

pub(super) async fn read_attempt(
    client: &Client,
    attempt_id: GovernanceAttemptId,
) -> Result<ParliamentAttemptStateV1> {
    let response = read_on_dedicated_thread({
        let client = client.client().clone();
        let attempt_id = (attempt_id).clone();
        move || client.get_parliament_attempt(attempt_id)
    })
    .await?;
    let frame = hex::decode(response.state_payload_hex)?;
    norito::decode_canonical(&frame).wrap_err("decode canonical Parliament reducer projection")
}

pub(super) async fn ordered_validator_roster(
    network: &iroha_test_network::Network,
    client: &Client,
) -> Result<Vec<PeerId>> {
    let height = current_height(client).await?;
    let (proof, verified) = finality::certified_block(network, client, height).await?;
    assert_eq!(verified.height(), height);
    let roster = proof
        .committee
        .into_iter()
        .map(|validator| PeerId::new(validator.public_key))
        .collect::<Vec<_>>();
    eprintln!(
        "SORA_PARLIAMENT_LIFECYCLE frozen_roster authority_height={height} roster_hash={}",
        hex::encode(global_threshold_beacon_roster_hash_v1(&roster)),
    );
    Ok(roster)
}

pub(super) fn lifecycle_certificate(
    network: &iroha_test_network::Network,
    ordered_roster: &[PeerId],
    action: ThresholdKeyLifecycleActionV1,
    session_id: [u8; 32],
    transcript_hash: [u8; 32],
    public_state: Vec<u8>,
    effective_height: u64,
) -> Result<ApplyThresholdKeyLifecycleCertificateV1> {
    lifecycle_certificate_replacing(
        network,
        ordered_roster,
        action,
        None,
        session_id,
        transcript_hash,
        public_state,
        effective_height,
    )
}

pub(super) fn lifecycle_certificate_replacing(
    network: &iroha_test_network::Network,
    ordered_roster: &[PeerId],
    action: ThresholdKeyLifecycleActionV1,
    expected_active_session_id: Option<[u8; 32]>,
    session_id: [u8; 32],
    transcript_hash: [u8; 32],
    public_state: Vec<u8>,
    effective_height: u64,
) -> Result<ApplyThresholdKeyLifecycleCertificateV1> {
    let mut certificate = ThresholdKeyLifecycleCertificateV1 {
        version: THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
        action,
        expected_active_session_id,
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
    verify_threshold_key_lifecycle_certificate_v1(
        &certificate,
        &network.network_id(),
        effective_height,
        ordered_roster,
    )
    .wrap_err("independently verify the exact lifecycle certificate before submission")?;
    Ok(ApplyThresholdKeyLifecycleCertificateV1 { certificate })
}

pub(super) async fn pulse_at(
    client: &Client,
    height: u64,
) -> Result<iroha::data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1> {
    exact_block(client, height)
        .await?
        .npos_consensus_effects()
        .and_then(|effects| effects.finalized_global_beacon_pulse)
        .ok_or_else(|| eyre!("block {height} does not carry the demanded global beacon pulse"))
}

pub(super) fn signed_consensus_handshake(
    network: &iroha_test_network::Network,
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

pub(super) fn casting_archive(
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

pub(super) fn release_projection(
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

pub(super) fn release_partial(
    partial: ParliamentTlePartialReleaseShareV1,
) -> TlePartialReleaseShareV1 {
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

pub(super) async fn stage_contract_artifact(
    client: &Client,
    artifact: &[u8],
) -> Result<(ContractCodeHash, ContractAbiHash)> {
    let verified = ivm::verify_contract_artifact(artifact)
        .map_err(|error| eyre!("verify integration contract artifact: {error}"))?;
    let manifest = verified
        .manifest
        .try_signed(client.client().key_pair())
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
        submit_parliament_instructions(&client, instructions).await?;
    }
    submit_parliament_instructions(&client, [RegisterSmartContractCode { manifest }]).await?;
    let code_hash = *verified.code_hash.as_ref();
    let abi_hash = *verified.abi_hash.as_ref();
    Ok((
        ContractCodeHash::new(code_hash),
        ContractAbiHash::new(abi_hash),
    ))
}

pub(super) fn public_finding_root(
    attempt_id: GovernanceAttemptId,
    body: ParliamentBody,
) -> [u8; 32] {
    let body = body.encode();
    Hash::new_from_chunks(&[
        b"iroha.integration.parliament.public-finding.v1\0",
        attempt_id.as_bytes(),
        &body,
    ])
    .into()
}

pub(super) async fn exact_block(client: &Client, height: u64) -> Result<SignedBlock> {
    let requested_height =
        NonZeroU64::new(height).ok_or_else(|| eyre!("finalized block height must be nonzero"))?;
    let mut matching = read_on_dedicated_thread({
        let client = client.client().clone();
        move || {
            client
                .query(FindBlocks)
                .filter_with(|block| block.equals("height", height).into_predicate())
                .execute_all()
                .map_err(|error| eyre!("query finalized blocks for exact height {height}: {error}"))
        }
    })
    .await?
    .into_iter()
    .filter(|block| block.header().height() == requested_height);
    let block = matching
        .next()
        .ok_or_else(|| eyre!("finalized block height {height} is absent"))?;
    if matching.next().is_some() {
        return Err(eyre!(
            "finalized block stream contains duplicate height {height}"
        ));
    }
    if block.header().height() != requested_height {
        return Err(eyre!(
            "finalized block stream returned height {} for exact request {height}",
            block.header().height()
        ));
    }
    Ok(block)
}

pub(super) async fn assert_no_global_beacon_pulse_at(
    client: &Client,
    height: u64,
    label: &str,
) -> Result<()> {
    let block = exact_block(client, height)
        .await
        .wrap_err_with(|| format!("{label}: exact finalized block is unavailable"))?;
    if block
        .npos_consensus_effects()
        .and_then(|effects| effects.finalized_global_beacon_pulse)
        .is_some()
    {
        return Err(eyre!(
            "{label}: finalized block {height} unexpectedly carries a global beacon pulse"
        ));
    }
    Ok(())
}

#[path = "sora_parliament_finality.rs"]
pub(super) mod finality;

#[path = "sora_parliament_enactment.rs"]
pub(super) mod enactment;

/// Prepare the actual seven-body provider governance corridor with independent threshold signers.
/// The caller must provision four validators and provider genesis inputs before network startup.
pub(crate) fn publication_parliament_builder(builder: NetworkBuilder) -> NetworkBuilder {
    enactment::builder(builder).with_config_layer(|layer| {
        layer
            .write(["gov", "coordination_council_size"], BODY_SEATS as i64)
            .write(["gov", "parliament_invitation_phase_blocks"], 68_i64);
    })
}

/// Execute a provider proposal through genuine citizen sortition, findings, timed OVN and enactment.
/// The return coordinates are observed only after all four peers agree on the enacted attempt.
pub(crate) async fn enact_publication_proposal(
    network: &iroha_test_network::Network,
    proposal: ProposalKind,
) -> Result<(GovernanceAttemptId, u64)> {
    let ProposalKind::SorafsProviderGovernance(provider) = &proposal else {
        return Err(eyre!(
            "publication corridor requires a provider governance proposal"
        ));
    };
    let preliminary = iroha_data_model::isi::governance::ProposeSorafsProviderGovernance {
        action: (*provider.action).clone(),
    };
    let keys = citizen_keys();
    let account = citizen_accounts(&keys).remove(0);
    let proposer = client_for(&network.client(), &account, &keys);
    let enacted =
        enactment::enact(network, proposal, vec![preliminary.into()], Some(proposer)).await?;
    let first = exact_block(&network.client(), enacted.height).await?;
    for peer in network.peers() {
        let client = peer.client();
        let block = exact_block(&client, enacted.height).await?;
        assert_eq!(block.hash(), first.hash());
        let attempt = read_attempt(&client, enacted.attempt_id).await?;
        assert_eq!(attempt.attempt().status, GovernanceAttemptStatusV1::Enacted);
        assert_eq!(attempt.terminal_height(), Some(enacted.height));
        let (_, verified) = finality::certified_block(network, &client, enacted.height).await?;
        assert_eq!(verified.block().hash(), block.hash());
        assert_eq!(
            verified.canonical_executed_wire()?,
            block.clone().with_commit_certificate(None).encode_wire()?,
            "per-peer enacted execution differs from its authenticated native decision",
        );
    }
    Ok((enacted.attempt_id, enacted.height))
}
