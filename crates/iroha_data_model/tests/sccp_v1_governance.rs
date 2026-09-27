//! Public-boundary tests for the SCCP v1 Parliament governance payload and its value types.
//!
//! Covers `specs/sccp.md` (revision 3) §3.4 destination words, §4.1 parameters, §4.13 light-client
//! values and the §4.14.3 `SccpGovernanceProposalV1` payload: stable schema names, binary and JSON
//! roundtrips, subject scoping of the expected head, every state-independent Propose check, and
//! the exact JSON integer invariant at `2^53 − 1`.

use hex_literal::hex;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::BlockHeader,
    bridge::SccpNetworkV1,
    parliament_types::FIRST_RELEASE_MAX_EXACT_JSON_U64,
    sccp::{
        deployment::{
            SCCP_TRON_ADDRESS_PREFIX_V1, SccpDeploymentV1, SccpEvmDeploymentV1, SccpTonCodeRefV1,
            SccpTonDeploymentV1, SccpTronDeploymentV1,
        },
        governance::{
            SCCP_GOVERNANCE_MAX_ACTIONS_V1, SCCP_GOVERNANCE_MEMO_MAX_BYTES_V1,
            SCCP_JSON_SAFE_U64_MAX_V1, SCCP_TON_MAX_WRAPPED_SUPPLY_EXCLUSIVE_V1,
            SccpClearBridgeKeyFaultActionV1, SccpFreezeLightClientActionV1, SccpGovernanceActionV1,
            SccpGovernanceBaseRevisionV1, SccpGovernanceProposalV1, SccpGovernanceStaticError,
            SccpGovernanceSubjectV1, SccpInitializeLightClientActionV1,
            SccpInstallTrustedCheckpointActionV1, SccpRegisterRouteActionV1,
            SccpReleaseStrandedActionV1, SccpRouteRevisionActionV1,
            SccpSetDestinationPausedActionV1, SccpSetParametersActionV1,
            SccpSetTairaPausedActionV1, SccpSwitchRevisionActionV1,
        },
        keys::SccpFaultRefV1,
        light_client::{
            SCCP_LC_BOOTSTRAP_MAX_BYTES_V1, SccpLcBootstrapV1, SccpLcCheckpointDataV1,
            SccpLcCheckpointOriginV1, SccpLcCheckpointV1, SccpLcInitExpectationV1,
            SccpLightClientParamsError, SccpLightClientParamsV1,
        },
        params::{
            SCCP_MAX_CLOCK_SKEW_MS_V1, SCCP_MAX_ROSTER_VALIDITY_MS_V1,
            SCCP_MESSAGES_MAX_PER_BLOCK_V1, SCCP_PREVIOUS_ROSTER_GRACE_MS_V1, SccpParametersError,
            SccpParametersV1,
        },
    },
};
use iroha_model_base::peer::PeerId;
use norito::{
    NoritoSchema,
    codec::{DecodeAll, Encode},
    json::Value,
};

const JSON_MAX: u64 = SCCP_JSON_SAFE_U64_MAX_V1;
const DAY_MS: u64 = 86_400_000;
const ETH: SccpNetworkV1 = SccpNetworkV1::EthereumMainnet;
const BSC: SccpNetworkV1 = SccpNetworkV1::BscMainnet;
const TRON: SccpNetworkV1 = SccpNetworkV1::TronMainnet;
const TON: SccpNetworkV1 = SccpNetworkV1::TonMainnet;
const TAIRA: SccpNetworkV1 = SccpNetworkV1::SoraTaira;
const EXTERNAL: [SccpNetworkV1; 4] = [ETH, BSC, TRON, TON];
const NETWORKS: [SccpNetworkV1; 5] = [TAIRA, ETH, BSC, TRON, TON];

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn network_id(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        [seed; Hash::LENGTH],
    )))
}

fn peer(seed: u8) -> PeerId {
    let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("deterministic Ed25519 seed");
    PeerId::new(key_pair.public_key().clone())
}

fn account(seed: u8) -> AccountId {
    let key_pair = KeyPair::try_from_seed(vec![seed.wrapping_add(0x80); 32], Algorithm::Ed25519)
        .expect("deterministic Ed25519 seed");
    AccountId::new(key_pair.public_key().clone())
}

fn code(seed: u8) -> SccpTonCodeRefV1 {
    SccpTonCodeRefV1 {
        hash: [seed; 32],
        depth: u16::from(seed) * 3,
    }
}

fn evm(address: [u8; 20]) -> SccpDeploymentV1 {
    SccpDeploymentV1::Evm(SccpEvmDeploymentV1 {
        address,
        runtime_code_hash: [0xc0; 32],
    })
}

fn tron_address(body: [u8; 20]) -> [u8; 21] {
    let mut address = [0_u8; 21];
    address[0] = SCCP_TRON_ADDRESS_PREFIX_V1;
    address[1..].copy_from_slice(&body);
    address
}

fn tron(address: [u8; 21]) -> SccpDeploymentV1 {
    SccpDeploymentV1::Tron(SccpTronDeploymentV1 {
        address,
        runtime_code_hash: [0xc1; 32],
    })
}

fn ton(master_account: [u8; 32]) -> SccpDeploymentV1 {
    SccpDeploymentV1::Ton(SccpTonDeploymentV1 {
        master_account,
        minter_code: code(1),
        wallet_code: code(2),
        bucket_code: code(3),
    })
}

fn register_with(
    network: SccpNetworkV1,
    deployment: SccpDeploymentV1,
    max_wrapped_supply: u128,
) -> SccpGovernanceActionV1 {
    SccpGovernanceActionV1::RegisterRoute(SccpRegisterRouteActionV1 {
        network,
        revision: 1,
        deployment,
        max_wrapped_supply,
        initial_roster_generation: 3,
    })
}

fn register(network: SccpNetworkV1, deployment: SccpDeploymentV1) -> SccpGovernanceActionV1 {
    register_with(network, deployment, 21_000_000_000_000_000)
}

fn revision(network: SccpNetworkV1, revision: u32) -> SccpRouteRevisionActionV1 {
    SccpRouteRevisionActionV1 { network, revision }
}

fn switch(network: SccpNetworkV1, from: u32, to: u32) -> SccpGovernanceActionV1 {
    SccpGovernanceActionV1::SwitchRevision(SccpSwitchRevisionActionV1 { network, from, to })
}

fn release(network: SccpNetworkV1, amount: u128, memo: &str) -> SccpGovernanceActionV1 {
    SccpGovernanceActionV1::ReleaseStranded(SccpReleaseStrandedActionV1 {
        network,
        amount,
        recipient: account(9),
        memo: memo.to_owned(),
    })
}

fn taira_paused(network: SccpNetworkV1, paused: bool) -> SccpGovernanceActionV1 {
    SccpGovernanceActionV1::SetTairaPaused(SccpSetTairaPausedActionV1 { network, paused })
}

fn destination_paused(
    network: SccpNetworkV1,
    revision: u32,
    paused: bool,
) -> SccpGovernanceActionV1 {
    SccpGovernanceActionV1::SetDestinationPaused(SccpSetDestinationPausedActionV1 {
        network,
        revision,
        paused,
    })
}

fn lc_params(network: SccpNetworkV1) -> SccpLightClientParamsV1 {
    SccpLightClientParamsV1::defaults_for(network).expect("external network has defaults")
}

fn initialize(network: SccpNetworkV1) -> SccpInitializeLightClientActionV1 {
    SccpInitializeLightClientActionV1 {
        network,
        expected: SccpLcInitExpectationV1::Absent,
        params: lc_params(network),
        bootstrap: SccpLcBootstrapV1 {
            network,
            bytes: b"NRT0 bootstrap frame".to_vec(),
        },
    }
}

fn checkpoint() -> SccpLcCheckpointDataV1 {
    SccpLcCheckpointDataV1 {
        source_height: 21_000_000,
        block_hash: [0x44; 32],
        state_root: Some([0x45; 32]),
        receipts_or_tx_root: [0x46; 32],
        source_time_ms: 1_758_000_000_000,
    }
}

fn install_checkpoint(
    network: SccpNetworkV1,
    checkpoint: SccpLcCheckpointDataV1,
) -> SccpGovernanceActionV1 {
    SccpGovernanceActionV1::InstallTrustedCheckpoint(SccpInstallTrustedCheckpointActionV1 {
        network,
        checkpoint,
    })
}

fn freeze(network: SccpNetworkV1) -> SccpGovernanceActionV1 {
    SccpGovernanceActionV1::FreezeLightClient(SccpFreezeLightClientActionV1 { network })
}

fn set_parameters(next: SccpParametersV1) -> SccpGovernanceActionV1 {
    SccpGovernanceActionV1::SetParameters(SccpSetParametersActionV1 { next })
}

fn clear_fault(seed: u8, height: u64) -> SccpGovernanceActionV1 {
    SccpGovernanceActionV1::ClearBridgeKeyFault(SccpClearBridgeKeyFaultActionV1 {
        peer: peer(seed),
        fault: SccpFaultRefV1 {
            address: [0x5a; 20],
            height,
        },
    })
}

/// One valid action of every kind, in declaration order, with distinct destination words.
fn every_action() -> Vec<SccpGovernanceActionV1> {
    vec![
        register(ETH, evm([0x22; 20])),
        SccpGovernanceActionV1::ActivateRevision(revision(ETH, 1)),
        switch(BSC, 1, 2),
        SccpGovernanceActionV1::DeactivateOutbound(revision(TRON, 2)),
        SccpGovernanceActionV1::RetireRevision(revision(TRON, 1)),
        SccpGovernanceActionV1::RemoveStaged(revision(TON, 3)),
        release(
            ETH,
            5_000_000_000,
            "stranded release reviewed by the FMA Committee",
        ),
        taira_paused(BSC, true),
        destination_paused(BSC, 2, true),
        SccpGovernanceActionV1::InitializeLightClient(initialize(TON)),
        install_checkpoint(ETH, checkpoint()),
        freeze(TRON),
        set_parameters(SccpParametersV1::taira_default()),
        clear_fault(3, 1_000),
    ]
}

/// A proposal bound to `network_id(1)` whose base revisions are exactly `S(P)` at revision 0.
fn proposal(actions: Vec<SccpGovernanceActionV1>) -> SccpGovernanceProposalV1 {
    let mut proposal = SccpGovernanceProposalV1 {
        network_id: network_id(1),
        base_revisions: Vec::new(),
        actions,
    };
    proposal.base_revisions = proposal
        .subjects()
        .into_iter()
        .map(|subject| SccpGovernanceBaseRevisionV1::from((subject, 0)))
        .collect();
    proposal
}

fn validate(actions: Vec<SccpGovernanceActionV1>) -> Result<(), SccpGovernanceStaticError> {
    proposal(actions).validate_static(&network_id(1))
}

/// Rewrites one parameter field.
type ParamsUpdate = fn(&mut SccpParametersV1);

fn parameters_with(update: impl FnOnce(&mut SccpParametersV1)) -> SccpParametersV1 {
    let mut parameters = SccpParametersV1::taira_default();
    update(&mut parameters);
    parameters
}

// ---------------------------------------------------------------------------------------------
// Codec helpers
// ---------------------------------------------------------------------------------------------

fn roundtrip<T>(value: &T)
where
    T: Encode
        + DecodeAll
        + norito::NoritoSerialize
        + for<'a> norito::NoritoDeserialize<'a>
        + for<'a> norito::core::DecodeFromSlice<'a>
        + norito::json::JsonSerialize
        + norito::json::JsonDeserialize
        + PartialEq
        + core::fmt::Debug,
{
    let encoded = value.encode();
    assert_eq!(
        &T::decode_all(&mut encoded.as_slice()).expect("bare codec decodes"),
        value
    );
    let framed = norito::to_bytes(value).expect("headered frame encodes");
    assert_eq!(
        &norito::decode_from_bytes::<T>(&framed).expect("headered frame decodes"),
        value
    );
    let json = norito::json::to_json(value).expect("JSON serializes");
    assert_eq!(
        &norito::json::from_json::<T>(&json).expect("JSON deserializes"),
        value,
        "{json}"
    );
}

fn json_value<T: norito::json::JsonSerialize>(value: &T) -> Value {
    norito::json::to_value(value).expect("JSON value")
}

fn json_tag<T: norito::json::JsonSerialize>(value: &T, tag: &str) -> String {
    json_value(value)
        .get(tag)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("JSON tag `{tag}` is absent"))
        .to_owned()
}

/// Insert an unknown field into the JSON object reached by `path` and assert rejection.
fn assert_rejects_unknown_field<T>(value: &T, path: &[&str])
where
    T: norito::json::JsonSerialize + norito::json::JsonDeserialize + core::fmt::Debug,
{
    let mut json = json_value(value);
    let mut current = &mut json;
    for field in path {
        let Value::Object(object) = current else {
            panic!("JSON path component `{field}` is not an object");
        };
        current = object
            .get_mut(*field)
            .unwrap_or_else(|| panic!("JSON path component `{field}` is absent"));
    }
    let Value::Object(object) = current else {
        panic!("JSON target at {path:?} is not an object");
    };
    object.insert("adversarial_extension".to_owned(), Value::Null);
    let hostile = norito::json::to_json(&json).expect("JSON serializes");
    assert!(
        norito::json::from_json::<T>(&hostile).is_err(),
        "unknown JSON field at {path:?} must be rejected: {hostile}"
    );
}

// ---------------------------------------------------------------------------------------------
// Constants and schema identities
// ---------------------------------------------------------------------------------------------

#[test]
fn constants_match_the_spec() {
    assert_eq!(SCCP_MAX_ROSTER_VALIDITY_MS_V1, 2_592_000_000);
    assert_eq!(SCCP_PREVIOUS_ROSTER_GRACE_MS_V1, 86_400_000);
    assert_eq!(SCCP_MAX_CLOCK_SKEW_MS_V1, 3_600_000);
    assert_eq!(SCCP_MESSAGES_MAX_PER_BLOCK_V1, 512);
    assert_eq!(SCCP_GOVERNANCE_MAX_ACTIONS_V1, 16);
    assert_eq!(SCCP_GOVERNANCE_MEMO_MAX_BYTES_V1, 256);
    assert_eq!(SCCP_JSON_SAFE_U64_MAX_V1, (1_u64 << 53) - 1);
    assert_eq!(SCCP_JSON_SAFE_U64_MAX_V1, FIRST_RELEASE_MAX_EXACT_JSON_U64);
    assert_eq!(SCCP_TON_MAX_WRAPPED_SUPPLY_EXCLUSIVE_V1, 1_u128 << 96);
    assert_eq!(SCCP_LC_BOOTSTRAP_MAX_BYTES_V1, 1_048_576);
    assert_eq!(SCCP_TRON_ADDRESS_PREFIX_V1, 0x41);
}

#[test]
fn value_type_schema_names_are_stable() {
    let cases = [
        (
            SccpParametersV1::nominal_name(),
            "iroha_data_model::sccp::params::SccpParametersV1",
        ),
        (
            SccpTonCodeRefV1::nominal_name(),
            "iroha_data_model::sccp::deployment::SccpTonCodeRefV1",
        ),
        (
            SccpEvmDeploymentV1::nominal_name(),
            "iroha_data_model::sccp::deployment::SccpEvmDeploymentV1",
        ),
        (
            SccpTronDeploymentV1::nominal_name(),
            "iroha_data_model::sccp::deployment::SccpTronDeploymentV1",
        ),
        (
            SccpTonDeploymentV1::nominal_name(),
            "iroha_data_model::sccp::deployment::SccpTonDeploymentV1",
        ),
        (
            SccpDeploymentV1::nominal_name(),
            "iroha_data_model::sccp::deployment::SccpDeploymentV1",
        ),
        (
            SccpLightClientParamsV1::nominal_name(),
            "iroha_data_model::sccp::light_client::SccpLightClientParamsV1",
        ),
        (
            SccpLcBootstrapV1::nominal_name(),
            "iroha_data_model::sccp::light_client::SccpLcBootstrapV1",
        ),
        (
            SccpLcCheckpointDataV1::nominal_name(),
            "iroha_data_model::sccp::light_client::SccpLcCheckpointDataV1",
        ),
        (
            SccpLcCheckpointOriginV1::nominal_name(),
            "iroha_data_model::sccp::light_client::SccpLcCheckpointOriginV1",
        ),
        (
            SccpLcCheckpointV1::nominal_name(),
            "iroha_data_model::sccp::light_client::SccpLcCheckpointV1",
        ),
        (
            SccpLcInitExpectationV1::nominal_name(),
            "iroha_data_model::sccp::light_client::SccpLcInitExpectationV1",
        ),
        (
            SccpFaultRefV1::nominal_name(),
            "iroha_data_model::sccp::keys::SccpFaultRefV1",
        ),
    ];
    for (actual, expected) in cases {
        assert_eq!(actual, expected);
    }
}

#[test]
fn governance_schema_names_are_stable() {
    let cases = [
        (
            SccpGovernanceSubjectV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpGovernanceSubjectV1",
        ),
        (
            SccpRegisterRouteActionV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpRegisterRouteActionV1",
        ),
        (
            SccpRouteRevisionActionV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpRouteRevisionActionV1",
        ),
        (
            SccpSwitchRevisionActionV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpSwitchRevisionActionV1",
        ),
        (
            SccpReleaseStrandedActionV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpReleaseStrandedActionV1",
        ),
        (
            SccpSetTairaPausedActionV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpSetTairaPausedActionV1",
        ),
        (
            SccpSetDestinationPausedActionV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpSetDestinationPausedActionV1",
        ),
        (
            SccpInitializeLightClientActionV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpInitializeLightClientActionV1",
        ),
        (
            SccpInstallTrustedCheckpointActionV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpInstallTrustedCheckpointActionV1",
        ),
        (
            SccpFreezeLightClientActionV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpFreezeLightClientActionV1",
        ),
        (
            SccpSetParametersActionV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpSetParametersActionV1",
        ),
        (
            SccpClearBridgeKeyFaultActionV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpClearBridgeKeyFaultActionV1",
        ),
        (
            SccpGovernanceActionV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpGovernanceActionV1",
        ),
        (
            SccpGovernanceBaseRevisionV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpGovernanceBaseRevisionV1",
        ),
        (
            SccpGovernanceProposalV1::nominal_name(),
            "iroha_data_model::sccp::governance::SccpGovernanceProposalV1",
        ),
    ];
    for (actual, expected) in cases {
        assert_eq!(actual, expected);
    }
}

// ---------------------------------------------------------------------------------------------
// Roundtrips and JSON shape
// ---------------------------------------------------------------------------------------------

#[test]
fn every_type_roundtrips_through_binary_frame_and_json() {
    roundtrip(&SccpParametersV1::taira_default());
    roundtrip(&parameters_with(|p| {
        p.enabled = false;
        p.min_outbound_amount = u128::MAX;
        p.inbound_self_claim_fee = 0;
        p.roster_validity_ms = u64::MAX;
        p.max_exempt_transactions_per_block = u32::MAX;
    }));

    roundtrip(&code(7));
    for deployment in [
        evm([0x22; 20]),
        tron(tron_address([0x23; 20])),
        ton([0x24; 32]),
    ] {
        roundtrip(&deployment);
        match deployment {
            SccpDeploymentV1::Evm(inner) => roundtrip(&inner),
            SccpDeploymentV1::Tron(inner) => roundtrip(&inner),
            SccpDeploymentV1::Ton(inner) => roundtrip(&inner),
        }
    }

    for network in EXTERNAL {
        roundtrip(&lc_params(network));
        roundtrip(&initialize(network).bootstrap);
    }
    roundtrip(&SccpLcBootstrapV1 {
        network: TON,
        bytes: Vec::new(),
    });
    let mut data = checkpoint();
    roundtrip(&data);
    data.state_root = None;
    roundtrip(&data);
    for origin in [
        SccpLcCheckpointOriginV1::Advance,
        SccpLcCheckpointOriginV1::Proof,
        SccpLcCheckpointOriginV1::Backfill,
        SccpLcCheckpointOriginV1::Parliament,
    ] {
        roundtrip(&origin);
        roundtrip(&SccpLcCheckpointV1 {
            data,
            recorded_at_taira_ms: 1_758_000_000_321,
            origin,
        });
    }
    roundtrip(&SccpLcInitExpectationV1::Absent);
    roundtrip(&SccpLcInitExpectationV1::Unusable);

    roundtrip(&SccpFaultRefV1 {
        address: [0x5a; 20],
        height: u64::MAX,
    });

    for subject in [
        SccpGovernanceSubjectV1::Route(ETH),
        SccpGovernanceSubjectV1::RouteControl(TRON),
        SccpGovernanceSubjectV1::LightClient(TON),
        SccpGovernanceSubjectV1::Parameters,
        SccpGovernanceSubjectV1::BridgeKeyFault(peer(5)),
    ] {
        roundtrip(&subject);
        roundtrip(&SccpGovernanceBaseRevisionV1 {
            subject,
            revision: JSON_MAX,
        });
    }

    for action in every_action() {
        roundtrip(&action);
        match &action {
            SccpGovernanceActionV1::RegisterRoute(inner) => roundtrip(inner),
            SccpGovernanceActionV1::ActivateRevision(inner)
            | SccpGovernanceActionV1::DeactivateOutbound(inner)
            | SccpGovernanceActionV1::RetireRevision(inner)
            | SccpGovernanceActionV1::RemoveStaged(inner) => roundtrip(inner),
            SccpGovernanceActionV1::SwitchRevision(inner) => roundtrip(inner),
            SccpGovernanceActionV1::ReleaseStranded(inner) => roundtrip(inner),
            SccpGovernanceActionV1::SetTairaPaused(inner) => roundtrip(inner),
            SccpGovernanceActionV1::SetDestinationPaused(inner) => roundtrip(inner),
            SccpGovernanceActionV1::InitializeLightClient(inner) => roundtrip(inner),
            SccpGovernanceActionV1::InstallTrustedCheckpoint(inner) => roundtrip(inner),
            SccpGovernanceActionV1::FreezeLightClient(inner) => roundtrip(inner),
            SccpGovernanceActionV1::SetParameters(inner) => roundtrip(inner),
            SccpGovernanceActionV1::ClearBridgeKeyFault(inner) => roundtrip(inner),
        }
    }
    roundtrip(&register(TRON, tron(tron_address([0x34; 20]))));
    roundtrip(&register_with(TON, ton([0x35; 32]), (1 << 96) - 1));
    roundtrip(&release(BSC, u128::MAX, "\u{3042}\u{3044} memo"));

    let mut full = proposal(every_action());
    for (index, entry) in full.base_revisions.iter_mut().enumerate() {
        entry.revision = u64::try_from(index).expect("small index") * 7;
    }
    roundtrip(&full);
    roundtrip(&SccpGovernanceProposalV1 {
        network_id: network_id(9),
        base_revisions: Vec::new(),
        actions: Vec::new(),
    });
}

#[test]
fn json_tags_are_snake_case() {
    let actions = [
        "register_route",
        "activate_revision",
        "switch_revision",
        "deactivate_outbound",
        "retire_revision",
        "remove_staged",
        "release_stranded",
        "set_taira_paused",
        "set_destination_paused",
        "initialize_light_client",
        "install_trusted_checkpoint",
        "freeze_light_client",
        "set_parameters",
        "clear_bridge_key_fault",
    ];
    let every = every_action();
    assert_eq!(every.len(), actions.len());
    for (action, tag) in every.iter().zip(actions) {
        assert_eq!(json_tag(action, "action"), tag);
        assert!(json_value(action).get("payload").is_some(), "{tag}");
    }

    for (subject, tag) in [
        (SccpGovernanceSubjectV1::Route(ETH), "route"),
        (SccpGovernanceSubjectV1::RouteControl(ETH), "route_control"),
        (SccpGovernanceSubjectV1::LightClient(ETH), "light_client"),
        (SccpGovernanceSubjectV1::Parameters, "parameters"),
        (
            SccpGovernanceSubjectV1::BridgeKeyFault(peer(1)),
            "bridge_key_fault",
        ),
    ] {
        assert_eq!(json_tag(&subject, "subject"), tag);
    }

    for (deployment, tag) in [
        (evm([1; 20]), "evm"),
        (tron(tron_address([1; 20])), "tron"),
        (ton([1; 32]), "ton"),
    ] {
        assert_eq!(json_tag(&deployment, "family"), tag);
    }

    for (origin, tag) in [
        (SccpLcCheckpointOriginV1::Advance, "advance"),
        (SccpLcCheckpointOriginV1::Proof, "proof"),
        (SccpLcCheckpointOriginV1::Backfill, "backfill"),
        (SccpLcCheckpointOriginV1::Parliament, "parliament"),
    ] {
        assert_eq!(json_tag(&origin, "origin"), tag);
    }

    for (expected, tag) in [
        (SccpLcInitExpectationV1::Absent, "absent"),
        (SccpLcInitExpectationV1::Unusable, "unusable"),
    ] {
        assert_eq!(json_tag(&expected, "expected"), tag);
    }
}

#[test]
fn proposal_json_is_closed_at_every_level() {
    let proposal = proposal(vec![
        register(ETH, evm([0x22; 20])),
        release(ETH, 1, "memo"),
        SccpGovernanceActionV1::InitializeLightClient(initialize(ETH)),
        install_checkpoint(ETH, checkpoint()),
        set_parameters(SccpParametersV1::taira_default()),
        clear_fault(4, 9),
    ]);
    assert_rejects_unknown_field(&proposal, &[]);
    assert_rejects_unknown_field(&proposal.base_revisions[0], &[]);
    for action in &proposal.actions {
        assert_rejects_unknown_field(action, &[]);
        assert_rejects_unknown_field(action, &["payload"]);
    }
    let SccpGovernanceActionV1::RegisterRoute(register) = &proposal.actions[0] else {
        panic!("first action registers a route");
    };
    assert_rejects_unknown_field(&register.deployment, &[]);
    assert_rejects_unknown_field(&register.deployment, &["deployment"]);
    let SccpGovernanceActionV1::InitializeLightClient(initialize) = &proposal.actions[2] else {
        panic!("third action initializes a light client");
    };
    assert_rejects_unknown_field(initialize, &["params"]);
    assert_rejects_unknown_field(initialize, &["bootstrap"]);
    let SccpGovernanceActionV1::InstallTrustedCheckpoint(install) = &proposal.actions[3] else {
        panic!("fourth action installs a checkpoint");
    };
    assert_rejects_unknown_field(install, &["checkpoint"]);
    let SccpGovernanceActionV1::SetParameters(set) = &proposal.actions[4] else {
        panic!("fifth action sets parameters");
    };
    assert_rejects_unknown_field(set, &["next"]);
    let SccpGovernanceActionV1::ClearBridgeKeyFault(clear) = &proposal.actions[5] else {
        panic!("sixth action clears a fault");
    };
    assert_rejects_unknown_field(clear, &["fault"]);
    assert_rejects_unknown_field(
        &SccpLcCheckpointV1 {
            data: checkpoint(),
            recorded_at_taira_ms: 1,
            origin: SccpLcCheckpointOriginV1::Parliament,
        },
        &["data"],
    );
    assert_rejects_unknown_field(&code(1), &[]);
}

#[test]
fn proposal_json_carries_exact_integers_and_decimal_amounts() {
    let mut proposal = proposal(vec![
        register_with(ETH, evm([0x22; 20]), u128::MAX),
        release(ETH, 1_000_000_000_000_000_000_000, "memo"),
    ]);
    proposal.base_revisions[0].revision = JSON_MAX;
    let json = norito::json::to_json(&proposal).expect("serialize");
    assert!(json.contains(r#""revision":9007199254740991"#), "{json}");
    assert!(
        json.contains(r#""max_wrapped_supply":"340282366920938463463374607431768211455""#),
        "{json}"
    );
    assert!(
        json.contains(r#""amount":"1000000000000000000000""#),
        "{json}"
    );
    assert!(json.contains(r#""initial_roster_generation":3"#), "{json}");
    let numeric_amount = json.replace(
        r#""amount":"1000000000000000000000""#,
        r#""amount":1000000000000000000000"#,
    );
    assert!(norito::json::from_json::<SccpGovernanceProposalV1>(&numeric_amount).is_err());
    let padded_amount = json.replace(
        r#""amount":"1000000000000000000000""#,
        r#""amount":"01000000000000000000000""#,
    );
    assert!(norito::json::from_json::<SccpGovernanceProposalV1>(&padded_amount).is_err());
}

#[test]
fn unknown_binary_tags_are_rejected() {
    for tag in [14_u32, 15, 255, u32::MAX] {
        let mut encoded = tag.encode();
        encoded.extend_from_slice(&[0x11; 256]);
        assert!(
            SccpGovernanceActionV1::decode_all(&mut encoded.as_slice()).is_err(),
            "action tag {tag}"
        );
    }
    for tag in [5_u32, u32::MAX] {
        let mut encoded = tag.encode();
        encoded.extend_from_slice(&[0x11; 64]);
        assert!(
            SccpGovernanceSubjectV1::decode_all(&mut encoded.as_slice()).is_err(),
            "subject tag {tag}"
        );
    }
    for tag in [3_u32, u32::MAX] {
        let mut encoded = tag.encode();
        encoded.extend_from_slice(&[0x11; 160]);
        assert!(
            SccpDeploymentV1::decode_all(&mut encoded.as_slice()).is_err(),
            "deployment tag {tag}"
        );
    }
    for tag in [4_u32, u32::MAX] {
        assert!(
            SccpLcCheckpointOriginV1::decode_all(&mut tag.encode().as_slice()).is_err(),
            "origin tag {tag}"
        );
    }
    for tag in [2_u32, u32::MAX] {
        assert!(
            SccpLcInitExpectationV1::decode_all(&mut tag.encode().as_slice()).is_err(),
            "expectation tag {tag}"
        );
    }
}

#[test]
fn trailing_bytes_are_rejected() {
    let mut encoded = proposal(every_action()).encode();
    encoded.push(0);
    assert!(SccpGovernanceProposalV1::decode_all(&mut encoded.as_slice()).is_err());
}

// ---------------------------------------------------------------------------------------------
// Destination words (§3.4) and deployment shape
// ---------------------------------------------------------------------------------------------

#[test]
fn destination_word_vectors() {
    // §3.4 control-leaf example: destination_word = 12 zero bytes ‖ 20 bytes of 0x22.
    assert_eq!(
        evm([0x22; 20]).destination_word(),
        hex!("0000000000000000000000002222222222222222222222222222222222222222")
    );
    // EVM: word(address20).
    assert_eq!(
        evm(hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")).destination_word(),
        hex!("000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
    );
    // TRON: word(address20) of the 0x41-prefixed address with the prefix dropped.
    assert_eq!(
        tron(hex!("41a614f803b6fd780986a42c78ec9c7f77e6ded13c")).destination_word(),
        hex!("000000000000000000000000a614f803b6fd780986a42c78ec9c7f77e6ded13c")
    );
    // TON: the Jetton master's basechain account id, verbatim.
    let master = hex!("b113a994b5024a16719f69139328eb759596c38a25f59028b146fecdc3621dfe");
    assert_eq!(ton(master).destination_word(), master);
    // The runtime code hash and TON code refs never influence the word.
    let SccpDeploymentV1::Ton(mut other) = ton(master) else {
        panic!("TON deployment");
    };
    other.minter_code = code(9);
    other.bucket_code = code(8);
    assert_eq!(SccpDeploymentV1::Ton(other).destination_word(), master);
    assert_eq!(
        SccpDeploymentV1::Evm(SccpEvmDeploymentV1 {
            address: [0x22; 20],
            runtime_code_hash: [0xee; 32],
        })
        .destination_word(),
        evm([0x22; 20]).destination_word()
    );
    // An EVM contract and a TRON contract with equal 20-byte bodies share one word, which is why
    // the destination-word index is global across routes.
    assert_eq!(
        tron(tron_address([0x22; 20])).destination_word(),
        evm([0x22; 20]).destination_word()
    );
}

#[test]
fn deployments_fit_exactly_their_networks() {
    let cases = [
        (evm([0x22; 20]), [false, true, true, false, false]),
        (
            tron(tron_address([0x22; 20])),
            [false, false, false, true, false],
        ),
        (ton([0x33; 32]), [false, false, false, false, true]),
        (evm([0; 20]), [false; 5]),
        (tron(tron_address([0; 20])), [false; 5]),
        (tron([0x22; 21]), [false; 5]),
        (tron([0; 21]), [false; 5]),
        (ton([0; 32]), [false; 5]),
    ];
    for (deployment, fits) in cases {
        for (network, expected) in NETWORKS.into_iter().zip(fits) {
            assert_eq!(
                deployment.fits_network(network),
                expected,
                "{deployment:?} on {network:?}"
            );
        }
    }
    let mut last_byte = [0_u8; 20];
    last_byte[19] = 1;
    assert!(evm(last_byte).fits_network(ETH));
    assert!(tron(tron_address(last_byte)).fits_network(TRON));
    let mut last_account = [0_u8; 32];
    last_account[31] = 1;
    assert!(ton(last_account).fits_network(TON));
}

// ---------------------------------------------------------------------------------------------
// Parameters (§4.1)
// ---------------------------------------------------------------------------------------------

#[test]
fn taira_default_is_the_genesis_table_and_passes() {
    let defaults = SccpParametersV1::taira_default();
    assert_eq!(
        defaults,
        SccpParametersV1 {
            enabled: true,
            roster_max_age_ms: 86_400_000,
            roster_validity_ms: 1_209_600_000,
            outbound_ttl_ms: 604_800_000,
            min_outbound_amount: 1_000_000_000,
            inbound_self_claim_fee: 10_000_000,
            attestation_retention_ms: 2_592_000_000,
            attestation_stall_ms: 600_000,
            max_attestation_entries_per_instruction: 256,
            max_exempt_transactions_per_block: 128,
        }
    );
    assert_eq!(defaults.validate(), Ok(()));
    assert_eq!(parameters_with(|p| p.enabled = false).validate(), Ok(()));
}

/// Assert `validate()` at a boundary value and just beyond it.
fn assert_boundary(
    name: &str,
    at: impl FnOnce(&mut SccpParametersV1),
    beyond: impl FnOnce(&mut SccpParametersV1),
    error: SccpParametersError,
) {
    assert_eq!(parameters_with(at).validate(), Ok(()), "{name} at bound");
    assert_eq!(
        parameters_with(beyond).validate(),
        Err(error),
        "{name} beyond bound"
    );
}

#[test]
fn roster_ttl_and_amount_rules_hold_at_and_beyond_their_bounds() {
    use SccpParametersError as E;
    assert_boundary(
        "roster_max_age_ms floor",
        |p| p.roster_max_age_ms = 3_600_000,
        |p| p.roster_max_age_ms = 3_599_999,
        E::RosterMaxAgeTooShort,
    );
    assert_boundary(
        "roster_validity_ms ceiling",
        |p| p.roster_validity_ms = 30 * DAY_MS,
        |p| p.roster_validity_ms = 30 * DAY_MS + 1,
        E::RosterValidityTooLong,
    );
    assert_boundary(
        "outbound_ttl_ms floor",
        |p| p.outbound_ttl_ms = DAY_MS,
        |p| p.outbound_ttl_ms = DAY_MS - 1,
        E::OutboundTtlOutOfRange,
    );
    assert_boundary(
        "outbound_ttl_ms ceiling",
        |p| {
            p.outbound_ttl_ms = 90 * DAY_MS;
            p.attestation_retention_ms = 91 * DAY_MS;
        },
        |p| {
            p.outbound_ttl_ms = 90 * DAY_MS + 1;
            p.attestation_retention_ms = 92 * DAY_MS;
        },
        E::OutboundTtlOutOfRange,
    );
    assert_boundary(
        "min_outbound_amount floor",
        |p| p.min_outbound_amount = 1,
        |p| p.min_outbound_amount = 0,
        E::MinOutboundAmountOutOfRange,
    );
    assert_boundary(
        "min_outbound_amount ceiling",
        |p| p.min_outbound_amount = 1_000_000_000_000_000,
        |p| p.min_outbound_amount = 1_000_000_000_000_001,
        E::MinOutboundAmountOutOfRange,
    );
    assert_boundary(
        "inbound_self_claim_fee ceiling",
        |p| p.inbound_self_claim_fee = 10_000_000_000,
        |p| p.inbound_self_claim_fee = 10_000_000_001,
        E::InboundSelfClaimFeeTooLarge,
    );
    assert_eq!(
        parameters_with(|p| p.inbound_self_claim_fee = 0).validate(),
        Ok(())
    );
}

#[test]
fn retention_stall_and_batch_rules_hold_at_and_beyond_their_bounds() {
    use SccpParametersError as E;
    assert_boundary(
        "attestation_retention_ms ceiling",
        |p| p.attestation_retention_ms = 365 * DAY_MS,
        |p| p.attestation_retention_ms = 365 * DAY_MS + 1,
        E::AttestationRetentionTooLong,
    );
    assert_boundary(
        "attestation_stall_ms floor",
        |p| p.attestation_stall_ms = 60_000,
        |p| p.attestation_stall_ms = 59_999,
        E::AttestationStallOutOfRange,
    );
    assert_boundary(
        "attestation_stall_ms ceiling",
        |p| {
            p.attestation_stall_ms = DAY_MS;
            p.roster_validity_ms = 5 * DAY_MS;
        },
        |p| {
            p.attestation_stall_ms = DAY_MS + 1;
            p.roster_validity_ms = 5 * DAY_MS;
        },
        E::AttestationStallOutOfRange,
    );
    assert_boundary(
        "max_attestation_entries_per_instruction floor",
        |p| p.max_attestation_entries_per_instruction = 64,
        |p| p.max_attestation_entries_per_instruction = 63,
        E::AttestationEntriesOutOfRange,
    );
    assert_boundary(
        "max_attestation_entries_per_instruction ceiling",
        |p| p.max_attestation_entries_per_instruction = 1_024,
        |p| p.max_attestation_entries_per_instruction = 1_025,
        E::AttestationEntriesOutOfRange,
    );
    assert_boundary(
        "max_exempt_transactions_per_block floor",
        |p| p.max_exempt_transactions_per_block = 94,
        |p| p.max_exempt_transactions_per_block = 93,
        E::ExemptTransactionsOutOfRange,
    );
    assert_boundary(
        "max_exempt_transactions_per_block ceiling",
        |p| p.max_exempt_transactions_per_block = 1_024,
        |p| p.max_exempt_transactions_per_block = 1_025,
        E::ExemptTransactionsOutOfRange,
    );
    for zero in [
        parameters_with(|p| p.roster_max_age_ms = 0),
        parameters_with(|p| p.outbound_ttl_ms = 0),
        parameters_with(|p| p.max_attestation_entries_per_instruction = 0),
        parameters_with(|p| p.max_exempt_transactions_per_block = 0),
    ] {
        assert!(zero.validate().is_err(), "{zero:?}");
    }
}

#[test]
fn joint_roster_validity_rule_holds_at_and_beyond_its_bound() {
    use SccpParametersError as E;
    // 2 × roster_max_age + attestation_stall + 1 d ≤ roster_validity.
    let floor = 2 * DAY_MS + 600_000 + DAY_MS;
    assert_boundary(
        "validity floor (defaults)",
        |p| p.roster_validity_ms = floor,
        |p| p.roster_validity_ms = floor - 1,
        E::RosterValidityTooShort,
    );
    assert_boundary(
        "validity floor moves with the stall window",
        |p| {
            p.roster_validity_ms = floor + 1;
            p.attestation_stall_ms = 600_001;
        },
        |p| {
            p.roster_validity_ms = floor;
            p.attestation_stall_ms = 600_001;
        },
        E::RosterValidityTooShort,
    );
    assert_boundary(
        "validity floor moves twice as fast with the heartbeat",
        |p| {
            p.roster_validity_ms = floor + 2;
            p.roster_max_age_ms = DAY_MS + 1;
        },
        |p| {
            p.roster_validity_ms = floor + 1;
            p.roster_max_age_ms = DAY_MS + 1;
        },
        E::RosterValidityTooShort,
    );
    // The smallest admissible configuration: 1 h heartbeat, 1 min stall.
    let smallest = 2 * 3_600_000 + 60_000 + DAY_MS;
    assert_boundary(
        "validity floor at the smallest heartbeat and stall",
        |p| {
            p.roster_max_age_ms = 3_600_000;
            p.attestation_stall_ms = 60_000;
            p.roster_validity_ms = smallest;
        },
        |p| {
            p.roster_max_age_ms = 3_600_000;
            p.attestation_stall_ms = 60_000;
            p.roster_validity_ms = smallest - 1;
        },
        E::RosterValidityTooShort,
    );
    // The largest heartbeat under the 30 d ceiling at the default stall.
    let largest_age = (30 * DAY_MS - 600_000 - DAY_MS) / 2;
    assert_boundary(
        "largest heartbeat under the ceiling",
        |p| {
            p.roster_validity_ms = 30 * DAY_MS;
            p.roster_max_age_ms = largest_age;
        },
        |p| {
            p.roster_validity_ms = 30 * DAY_MS;
            p.roster_max_age_ms = largest_age + 1;
        },
        E::RosterValidityTooShort,
    );
    // Checked arithmetic: an overflowing floor is a violation, never a wrap-around pass.
    for overflow in [
        parameters_with(|p| {
            p.roster_max_age_ms = u64::MAX / 2 + 1;
            p.roster_validity_ms = u64::MAX;
        }),
        parameters_with(|p| {
            p.roster_max_age_ms = u64::MAX / 2;
            p.attestation_stall_ms = u64::MAX;
            p.roster_validity_ms = u64::MAX;
        }),
        parameters_with(|p| {
            p.roster_max_age_ms = (u64::MAX - DAY_MS) / 2;
            p.attestation_stall_ms = 2;
            p.roster_validity_ms = u64::MAX;
        }),
    ] {
        assert_eq!(overflow.validate(), Err(E::RosterValidityTooShort));
    }
}

#[test]
fn joint_attestation_retention_rule_holds_at_and_beyond_its_bound() {
    use SccpParametersError as E;
    // outbound_ttl + 1 d ≤ attestation_retention.
    let floor = 604_800_000 + DAY_MS;
    assert_boundary(
        "retention floor (defaults)",
        |p| p.attestation_retention_ms = floor,
        |p| p.attestation_retention_ms = floor - 1,
        E::AttestationRetentionTooShort,
    );
    assert_boundary(
        "retention floor moves with the TTL",
        |p| {
            p.outbound_ttl_ms = 604_800_001;
            p.attestation_retention_ms = floor + 1;
        },
        |p| {
            p.outbound_ttl_ms = 604_800_001;
            p.attestation_retention_ms = floor;
        },
        E::AttestationRetentionTooShort,
    );
    assert_boundary(
        "retention floor at the longest TTL",
        |p| {
            p.outbound_ttl_ms = 90 * DAY_MS;
            p.attestation_retention_ms = 91 * DAY_MS;
        },
        |p| {
            p.outbound_ttl_ms = 90 * DAY_MS;
            p.attestation_retention_ms = 91 * DAY_MS - 1;
        },
        E::AttestationRetentionTooShort,
    );
}

#[test]
fn parameter_errors_render_their_rule() {
    for (error, needle) in [
        (
            SccpParametersError::RosterMaxAgeTooShort,
            "roster_max_age_ms",
        ),
        (
            SccpParametersError::RosterValidityTooShort,
            "roster_validity_ms",
        ),
        (
            SccpParametersError::RosterValidityTooLong,
            "roster_validity_ms",
        ),
        (
            SccpParametersError::OutboundTtlOutOfRange,
            "outbound_ttl_ms",
        ),
        (
            SccpParametersError::MinOutboundAmountOutOfRange,
            "min_outbound_amount",
        ),
        (
            SccpParametersError::InboundSelfClaimFeeTooLarge,
            "inbound_self_claim_fee",
        ),
        (
            SccpParametersError::AttestationRetentionTooShort,
            "attestation_retention_ms",
        ),
        (
            SccpParametersError::AttestationRetentionTooLong,
            "attestation_retention_ms",
        ),
        (
            SccpParametersError::AttestationStallOutOfRange,
            "attestation_stall_ms",
        ),
        (
            SccpParametersError::AttestationEntriesOutOfRange,
            "max_attestation_entries_per_instruction",
        ),
        (
            SccpParametersError::ExemptTransactionsOutOfRange,
            "max_exempt_transactions_per_block",
        ),
    ] {
        assert!(error.to_string().contains(needle), "{error}");
    }
}

// ---------------------------------------------------------------------------------------------
// Light-client values (§4.13)
// ---------------------------------------------------------------------------------------------

#[test]
fn light_client_defaults_follow_the_per_chain_table() {
    assert_eq!(SccpLightClientParamsV1::defaults_for(TAIRA), None);
    let expected = [
        (ETH, 14 * DAY_MS, 8_192),
        (BSC, 5 * DAY_MS, 8_192),
        (TRON, 7 * DAY_MS, 1_200),
        (TON, 0, 0),
    ];
    for (network, ws_bound_ms, stride) in expected {
        let params = lc_params(network);
        assert_eq!(params.network, network);
        assert_eq!(params.ws_bound_ms, ws_bound_ms, "{network:?}");
        assert_eq!(params.checkpoint_stride, stride, "{network:?}");
        assert_eq!(params.set_retention_ms, 180 * DAY_MS, "{network:?}");
        assert_eq!(params.checkpoint_prune_after_ms, 30 * DAY_MS, "{network:?}");
        assert_eq!(params.max_backfill_headers, 256, "{network:?}");
        assert!(params.max_advance_bytes >= 262_144, "{network:?}");
        assert!(params.max_proof_bytes > 0, "{network:?}");
        assert_eq!(params.validate(), Ok(()), "{network:?}");
    }
    assert_eq!(lc_params(ETH).max_updates_per_advance, 16);
    assert_eq!(lc_params(TRON).max_segment_headers, 128);
    assert_eq!(lc_params(TRON).max_ancestry_headers, 1_200);
}

#[test]
fn light_client_params_validation() {
    let mut taira = lc_params(ETH);
    taira.network = TAIRA;
    assert_eq!(
        taira.validate(),
        Err(SccpLightClientParamsError::NotExternal)
    );
    let mut zero = lc_params(BSC);
    zero.max_segment_headers = 0;
    assert_eq!(
        zero.validate(),
        Err(SccpLightClientParamsError::ZeroBound {
            field: "max_segment_headers"
        })
    );
    let mut zero_ws = lc_params(TRON);
    zero_ws.ws_bound_ms = 0;
    assert_eq!(
        zero_ws.validate(),
        Err(SccpLightClientParamsError::ZeroBound {
            field: "ws_bound_ms"
        })
    );
    let mut ton_ws = lc_params(TON);
    ton_ws.ws_bound_ms = DAY_MS;
    assert_eq!(
        ton_ws.validate(),
        Err(SccpLightClientParamsError::TonDerivedFieldNonZero {
            field: "ws_bound_ms"
        })
    );
    let mut at = lc_params(ETH);
    at.set_retention_ms = JSON_MAX;
    assert_eq!(at.validate(), Ok(()));
    let mut over = at;
    over.set_retention_ms = JSON_MAX + 1;
    assert_eq!(
        over.validate(),
        Err(SccpLightClientParamsError::ExceedsJsonSafeInteger {
            field: "set_retention_ms"
        })
    );
}

#[test]
fn bootstrap_json_is_base64_and_checkpoint_state_root_is_explicit() {
    let bootstrap = SccpLcBootstrapV1 {
        network: ETH,
        bytes: vec![0xde, 0xad, 0xbe, 0xef],
    };
    let json = norito::json::to_json(&bootstrap).expect("serialize");
    assert!(json.contains("3q2+7w=="), "{json}");

    let mut data = checkpoint();
    data.state_root = None;
    let mut value = json_value(&data);
    let Value::Object(object) = &mut value else {
        panic!("checkpoint JSON is an object");
    };
    assert!(object.contains_key("state_root"));
    object.remove("state_root");
    let missing = norito::json::to_json(&value).expect("serialize");
    assert!(norito::json::from_json::<SccpLcCheckpointDataV1>(&missing).is_err());
}

// ---------------------------------------------------------------------------------------------
// Subjects and S(P)
// ---------------------------------------------------------------------------------------------

#[test]
fn every_action_has_its_spec_subject() {
    let expected = [
        SccpGovernanceSubjectV1::Route(ETH),
        SccpGovernanceSubjectV1::Route(ETH),
        SccpGovernanceSubjectV1::Route(BSC),
        SccpGovernanceSubjectV1::Route(TRON),
        SccpGovernanceSubjectV1::Route(TRON),
        SccpGovernanceSubjectV1::Route(TON),
        SccpGovernanceSubjectV1::Route(ETH),
        SccpGovernanceSubjectV1::Route(BSC),
        SccpGovernanceSubjectV1::RouteControl(BSC),
        SccpGovernanceSubjectV1::LightClient(TON),
        SccpGovernanceSubjectV1::LightClient(ETH),
        SccpGovernanceSubjectV1::LightClient(TRON),
        SccpGovernanceSubjectV1::Parameters,
        SccpGovernanceSubjectV1::BridgeKeyFault(peer(3)),
    ];
    let actions = every_action();
    assert_eq!(actions.len(), 14, "§4.14.3 defines fourteen actions");
    for (action, subject) in actions.iter().zip(expected) {
        assert_eq!(action.subject(), subject, "{action:?}");
    }
    assert_eq!(
        actions
            .iter()
            .map(SccpGovernanceActionV1::network)
            .collect::<Vec<_>>(),
        vec![
            Some(ETH),
            Some(ETH),
            Some(BSC),
            Some(TRON),
            Some(TRON),
            Some(TON),
            Some(ETH),
            Some(BSC),
            Some(BSC),
            Some(TON),
            Some(ETH),
            Some(TRON),
            None,
            None,
        ]
    );
}

#[test]
fn subjects_order_by_variant_then_value() {
    let mut subjects = vec![
        SccpGovernanceSubjectV1::BridgeKeyFault(peer(2)),
        SccpGovernanceSubjectV1::BridgeKeyFault(peer(1)),
        SccpGovernanceSubjectV1::Parameters,
        SccpGovernanceSubjectV1::LightClient(TON),
        SccpGovernanceSubjectV1::LightClient(ETH),
        SccpGovernanceSubjectV1::RouteControl(TRON),
        SccpGovernanceSubjectV1::RouteControl(BSC),
        SccpGovernanceSubjectV1::Route(TON),
        SccpGovernanceSubjectV1::Route(TRON),
        SccpGovernanceSubjectV1::Route(BSC),
        SccpGovernanceSubjectV1::Route(ETH),
    ];
    subjects.sort();
    let (low, high) = if peer(1) < peer(2) {
        (peer(1), peer(2))
    } else {
        (peer(2), peer(1))
    };
    assert_eq!(
        subjects,
        vec![
            SccpGovernanceSubjectV1::Route(ETH),
            SccpGovernanceSubjectV1::Route(BSC),
            SccpGovernanceSubjectV1::Route(TRON),
            SccpGovernanceSubjectV1::Route(TON),
            SccpGovernanceSubjectV1::RouteControl(BSC),
            SccpGovernanceSubjectV1::RouteControl(TRON),
            SccpGovernanceSubjectV1::LightClient(ETH),
            SccpGovernanceSubjectV1::LightClient(TON),
            SccpGovernanceSubjectV1::Parameters,
            SccpGovernanceSubjectV1::BridgeKeyFault(low),
            SccpGovernanceSubjectV1::BridgeKeyFault(high),
        ]
    );
}

#[test]
fn subject_list_is_sorted_and_deduplicated() {
    assert_eq!(
        proposal(every_action()).subjects(),
        vec![
            SccpGovernanceSubjectV1::Route(ETH),
            SccpGovernanceSubjectV1::Route(BSC),
            SccpGovernanceSubjectV1::Route(TRON),
            SccpGovernanceSubjectV1::Route(TON),
            SccpGovernanceSubjectV1::RouteControl(BSC),
            SccpGovernanceSubjectV1::LightClient(ETH),
            SccpGovernanceSubjectV1::LightClient(TRON),
            SccpGovernanceSubjectV1::LightClient(TON),
            SccpGovernanceSubjectV1::Parameters,
            SccpGovernanceSubjectV1::BridgeKeyFault(peer(3)),
        ]
    );
    // A pause spans Route{n} and RouteControl{n}, whatever the action order and repetition.
    let pause = proposal(vec![
        destination_paused(ETH, 1, true),
        taira_paused(ETH, true),
        destination_paused(ETH, 2, true),
        taira_paused(ETH, true),
    ]);
    assert_eq!(
        pause.subjects(),
        vec![
            SccpGovernanceSubjectV1::Route(ETH),
            SccpGovernanceSubjectV1::RouteControl(ETH),
        ]
    );
    // Registration, light-client initialization and activation of one network.
    let onboarding = proposal(vec![
        register(TON, ton([0x33; 32])),
        SccpGovernanceActionV1::InitializeLightClient(initialize(TON)),
        SccpGovernanceActionV1::ActivateRevision(revision(TON, 1)),
    ]);
    assert_eq!(
        onboarding.subjects(),
        vec![
            SccpGovernanceSubjectV1::Route(TON),
            SccpGovernanceSubjectV1::LightClient(TON),
        ]
    );
    // Distinct peers are distinct subjects; the same peer twice is one subject.
    let faults = proposal(vec![
        clear_fault(1, 5),
        clear_fault(2, 6),
        clear_fault(1, 7),
    ]);
    assert_eq!(faults.subjects().len(), 2);
    assert!(proposal(Vec::new()).subjects().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Static validation (§4.14.3 Propose)
// ---------------------------------------------------------------------------------------------

#[test]
fn valid_proposals_pass() {
    assert_eq!(validate(every_action()), Ok(()));
    assert_eq!(
        validate(
            (0..SCCP_GOVERNANCE_MAX_ACTIONS_V1)
                .map(|index| taira_paused(ETH, index % 2 == 0))
                .collect()
        ),
        Ok(())
    );
    assert_eq!(
        validate(vec![
            register(ETH, evm([0x22; 20])),
            register(BSC, evm([0x23; 20])),
            register(TRON, tron(tron_address([0x24; 20]))),
            register(TON, ton([0x25; 32])),
        ]),
        Ok(())
    );
    let mut with_revisions = proposal(every_action());
    for (index, entry) in with_revisions.base_revisions.iter_mut().enumerate() {
        entry.revision = u64::try_from(index).expect("small index") + 1;
    }
    assert_eq!(with_revisions.validate_static(&network_id(1)), Ok(()));
}

#[test]
fn network_id_must_equal_the_live_network() {
    let proposal = proposal(every_action());
    assert_eq!(proposal.validate_static(&network_id(1)), Ok(()));
    assert_eq!(
        proposal.validate_static(&network_id(2)),
        Err(SccpGovernanceStaticError::NetworkIdMismatch)
    );
}

#[test]
fn action_count_is_one_to_sixteen() {
    assert_eq!(
        validate(Vec::new()),
        Err(SccpGovernanceStaticError::NoActions)
    );
    assert_eq!(validate(vec![freeze(ETH)]), Ok(()));
    assert_eq!(validate(vec![freeze(ETH); 16]), Ok(()));
    assert_eq!(
        validate(vec![freeze(ETH); 17]),
        Err(SccpGovernanceStaticError::TooManyActions { count: 17 })
    );
}

#[test]
fn every_network_bearing_action_rejects_taira() {
    let taira_actions = [
        register(TAIRA, evm([0x22; 20])),
        SccpGovernanceActionV1::ActivateRevision(revision(TAIRA, 1)),
        switch(TAIRA, 1, 2),
        SccpGovernanceActionV1::DeactivateOutbound(revision(TAIRA, 1)),
        SccpGovernanceActionV1::RetireRevision(revision(TAIRA, 1)),
        SccpGovernanceActionV1::RemoveStaged(revision(TAIRA, 1)),
        release(TAIRA, 1, ""),
        taira_paused(TAIRA, true),
        destination_paused(TAIRA, 1, true),
        SccpGovernanceActionV1::InitializeLightClient(SccpInitializeLightClientActionV1 {
            network: TAIRA,
            ..initialize(ETH)
        }),
        install_checkpoint(TAIRA, checkpoint()),
        freeze(TAIRA),
    ];
    assert_eq!(taira_actions.len(), 12);
    for action in taira_actions {
        assert_eq!(
            validate(vec![freeze(ETH), action.clone()]),
            Err(SccpGovernanceStaticError::TairaNetwork { action: 1 }),
            "{action:?}"
        );
    }
}

#[test]
fn every_revision_bearing_action_rejects_revision_zero() {
    let SccpGovernanceActionV1::RegisterRoute(mut zero) = register(ETH, evm([0x22; 20])) else {
        panic!("register action");
    };
    zero.revision = 0;
    for action in [
        SccpGovernanceActionV1::RegisterRoute(zero),
        SccpGovernanceActionV1::ActivateRevision(revision(ETH, 0)),
        switch(ETH, 0, 1),
        switch(ETH, 1, 0),
        SccpGovernanceActionV1::DeactivateOutbound(revision(ETH, 0)),
        SccpGovernanceActionV1::RetireRevision(revision(ETH, 0)),
        SccpGovernanceActionV1::RemoveStaged(revision(ETH, 0)),
        destination_paused(ETH, 0, false),
    ] {
        assert_eq!(
            validate(vec![action.clone()]),
            Err(SccpGovernanceStaticError::ZeroRevision { action: 0 }),
            "{action:?}"
        );
    }
    assert_eq!(
        validate(vec![switch(ETH, 3, 3)]),
        Err(SccpGovernanceStaticError::SwitchRevisionToSelf { action: 0 })
    );
    assert_eq!(validate(vec![switch(ETH, 3, 4)]), Ok(()));
    assert_eq!(validate(vec![switch(ETH, 4, 3)]), Ok(()));
}

#[test]
fn register_route_cap_bounds() {
    assert_eq!(
        validate(vec![register_with(ETH, evm([0x22; 20]), 0)]),
        Err(SccpGovernanceStaticError::ZeroMaxWrappedSupply { action: 0 })
    );
    for network in [ETH, BSC] {
        assert_eq!(
            validate(vec![register_with(network, evm([0x22; 20]), 1)]),
            Ok(())
        );
        assert_eq!(
            validate(vec![register_with(network, evm([0x22; 20]), u128::MAX)]),
            Ok(())
        );
    }
    assert_eq!(
        validate(vec![register_with(
            TRON,
            tron(tron_address([0x22; 20])),
            u128::MAX
        )]),
        Ok(())
    );
    assert_eq!(
        validate(vec![register_with(TON, ton([0x33; 32]), (1 << 96) - 1)]),
        Ok(())
    );
    for cap in [1_u128 << 96, u128::MAX] {
        assert_eq!(
            validate(vec![register_with(TON, ton([0x33; 32]), cap)]),
            Err(SccpGovernanceStaticError::TonMaxWrappedSupplyTooLarge { action: 0 })
        );
    }
    assert_eq!(
        validate(vec![register_with(TON, ton([0x33; 32]), 0)]),
        Err(SccpGovernanceStaticError::ZeroMaxWrappedSupply { action: 0 })
    );
}

#[test]
fn register_route_deployment_must_fit_its_network() {
    for (network, deployment) in [
        (TRON, evm([0x22; 20])),
        (TON, evm([0x22; 20])),
        (ETH, tron(tron_address([0x22; 20]))),
        (BSC, tron(tron_address([0x22; 20]))),
        (ETH, ton([0x33; 32])),
        (TRON, ton([0x33; 32])),
        (ETH, evm([0; 20])),
        (BSC, evm([0; 20])),
        (TRON, tron(tron_address([0; 20]))),
        (TRON, tron([0x42; 21])),
        (TON, ton([0; 32])),
    ] {
        assert_eq!(
            validate(vec![register(network, deployment)]),
            Err(SccpGovernanceStaticError::DeploymentNetworkMismatch { action: 0 }),
            "{deployment:?} on {network:?}"
        );
    }
}

#[test]
fn destination_words_are_unique_within_one_proposal() {
    for (actions, action, first) in [
        // The same EVM address on Ethereum and BSC.
        (
            vec![
                register(ETH, evm([0x22; 20])),
                freeze(ETH),
                register(BSC, evm([0x22; 20])),
            ],
            2,
            0,
        ),
        // A TRON body equal to an EVM address.
        (
            vec![
                register(TRON, tron(tron_address([0x22; 20]))),
                register(ETH, evm([0x22; 20])),
            ],
            1,
            0,
        ),
        // A TON account id equal to an EVM word.
        (
            vec![
                register(ETH, evm([0x22; 20])),
                register(TON, ton(evm([0x22; 20]).destination_word())),
            ],
            1,
            0,
        ),
        // The same route registered twice with the same deployment.
        (
            vec![
                register(BSC, evm([0x23; 20])),
                register(BSC, evm([0x24; 20])),
                register(BSC, evm([0x23; 20])),
            ],
            2,
            0,
        ),
    ] {
        assert_eq!(
            validate(actions),
            Err(SccpGovernanceStaticError::DuplicateDestinationWord { action, first })
        );
    }
}

#[test]
fn release_stranded_amount_and_memo() {
    assert_eq!(
        validate(vec![release(ETH, 0, "")]),
        Err(SccpGovernanceStaticError::ZeroAmount { action: 0 })
    );
    assert_eq!(validate(vec![release(ETH, 1, "")]), Ok(()));
    assert_eq!(validate(vec![release(ETH, u128::MAX, "")]), Ok(()));
    assert_eq!(
        validate(vec![release(
            ETH,
            1,
            &"m".repeat(SCCP_GOVERNANCE_MEMO_MAX_BYTES_V1)
        )]),
        Ok(())
    );
    assert_eq!(
        validate(vec![release(
            ETH,
            1,
            &"m".repeat(SCCP_GOVERNANCE_MEMO_MAX_BYTES_V1 + 1)
        )]),
        Err(SccpGovernanceStaticError::MemoTooLong {
            action: 0,
            len: 257
        })
    );
    // The bound is in UTF-8 bytes: 85 three-byte characters fit, 86 do not.
    assert_eq!(
        validate(vec![release(ETH, 1, &"\u{3042}".repeat(85))]),
        Ok(())
    );
    assert_eq!(
        validate(vec![release(ETH, 1, &"\u{3042}".repeat(86))]),
        Err(SccpGovernanceStaticError::MemoTooLong {
            action: 0,
            len: 258
        })
    );
}

#[test]
fn initialize_light_client_checks() {
    let mut params_network = initialize(ETH);
    params_network.params = lc_params(BSC);
    assert_eq!(
        validate(vec![SccpGovernanceActionV1::InitializeLightClient(
            params_network
        )]),
        Err(SccpGovernanceStaticError::LightClientParamsNetworkMismatch { action: 0 })
    );

    let mut invalid_params = initialize(ETH);
    invalid_params.params.max_advance_bytes = 0;
    assert_eq!(
        validate(vec![SccpGovernanceActionV1::InitializeLightClient(
            invalid_params
        )]),
        Err(SccpGovernanceStaticError::InvalidLightClientParams {
            action: 0,
            error: SccpLightClientParamsError::ZeroBound {
                field: "max_advance_bytes"
            },
        })
    );

    let mut bootstrap_network = initialize(ETH);
    bootstrap_network.bootstrap.network = TRON;
    assert_eq!(
        validate(vec![SccpGovernanceActionV1::InitializeLightClient(
            bootstrap_network
        )]),
        Err(SccpGovernanceStaticError::BootstrapNetworkMismatch { action: 0 })
    );

    let mut empty = initialize(BSC);
    empty.bootstrap.bytes.clear();
    assert_eq!(
        validate(vec![SccpGovernanceActionV1::InitializeLightClient(empty)]),
        Err(SccpGovernanceStaticError::EmptyBootstrap { action: 0 })
    );

    let mut at_bound = initialize(TON);
    at_bound.bootstrap.bytes = vec![0xb5; SCCP_LC_BOOTSTRAP_MAX_BYTES_V1];
    assert_eq!(
        validate(vec![SccpGovernanceActionV1::InitializeLightClient(
            at_bound.clone()
        )]),
        Ok(())
    );
    let mut beyond = at_bound;
    beyond.bootstrap.bytes.push(0xb5);
    assert_eq!(
        validate(vec![SccpGovernanceActionV1::InitializeLightClient(beyond)]),
        Err(SccpGovernanceStaticError::BootstrapTooLarge {
            action: 0,
            len: SCCP_LC_BOOTSTRAP_MAX_BYTES_V1 + 1,
        })
    );

    for network in EXTERNAL {
        let mut unusable = initialize(network);
        unusable.expected = SccpLcInitExpectationV1::Unusable;
        assert_eq!(
            validate(vec![SccpGovernanceActionV1::InitializeLightClient(
                unusable
            )]),
            Ok(()),
            "{network:?}"
        );
    }
}

#[test]
fn set_parameters_applies_every_parameter_rule() {
    use SccpParametersError as E;
    let cases: [(ParamsUpdate, SccpParametersError); 11] = [
        (|p| p.roster_max_age_ms = 0, E::RosterMaxAgeTooShort),
        (|p| p.roster_validity_ms = 1, E::RosterValidityTooShort),
        (
            |p| p.roster_validity_ms = 30 * DAY_MS + 1,
            E::RosterValidityTooLong,
        ),
        (|p| p.outbound_ttl_ms = 1, E::OutboundTtlOutOfRange),
        (
            |p| p.min_outbound_amount = 0,
            E::MinOutboundAmountOutOfRange,
        ),
        (
            |p| p.inbound_self_claim_fee = u128::MAX,
            E::InboundSelfClaimFeeTooLarge,
        ),
        (
            |p| p.attestation_retention_ms = 1,
            E::AttestationRetentionTooShort,
        ),
        (
            |p| p.attestation_retention_ms = u64::MAX,
            E::AttestationRetentionTooLong,
        ),
        (
            |p| p.attestation_stall_ms = 1,
            E::AttestationStallOutOfRange,
        ),
        (
            |p| p.max_attestation_entries_per_instruction = 1,
            E::AttestationEntriesOutOfRange,
        ),
        (
            |p| p.max_exempt_transactions_per_block = 1,
            E::ExemptTransactionsOutOfRange,
        ),
    ];
    for (update, error) in cases {
        assert_eq!(
            validate(vec![freeze(ETH), set_parameters(parameters_with(update))]),
            Err(SccpGovernanceStaticError::InvalidParameters { action: 1, error }),
            "{error:?}"
        );
    }
    assert_eq!(
        validate(vec![set_parameters(parameters_with(|p| p.enabled = false))]),
        Ok(())
    );
}

#[test]
fn base_revisions_must_be_exactly_the_sorted_subject_list() {
    let live = network_id(1);
    let valid = proposal(every_action());
    assert_eq!(valid.validate_static(&live), Ok(()));

    let mut missing = valid.clone();
    missing.base_revisions.remove(4);
    let mut extra = valid.clone();
    extra
        .base_revisions
        .push(SccpGovernanceBaseRevisionV1::from((
            SccpGovernanceSubjectV1::BridgeKeyFault(peer(8)),
            0,
        )));
    let mut unsorted = valid.clone();
    unsorted.base_revisions.swap(1, 2);
    let mut reversed = valid.clone();
    reversed.base_revisions.reverse();
    let mut duplicated = valid.clone();
    let repeated = duplicated.base_revisions[3].clone();
    duplicated.base_revisions.insert(3, repeated);
    let mut wrong_subject = valid.clone();
    wrong_subject.base_revisions[0].subject = SccpGovernanceSubjectV1::LightClient(BSC);
    let mut wrong_peer = valid.clone();
    wrong_peer
        .base_revisions
        .last_mut()
        .expect("fault subject")
        .subject = SccpGovernanceSubjectV1::BridgeKeyFault(peer(4));
    let mut empty = valid;
    empty.base_revisions.clear();
    for (name, candidate) in [
        ("missing", missing),
        ("extra", extra),
        ("unsorted", unsorted),
        ("reversed", reversed),
        ("duplicated", duplicated),
        ("wrong subject", wrong_subject),
        ("wrong peer", wrong_peer),
        ("empty", empty),
    ] {
        assert_eq!(
            candidate.validate_static(&live),
            Err(SccpGovernanceStaticError::BaseRevisionsMismatch),
            "{name}"
        );
    }
}

#[test]
fn errors_name_the_offending_action() {
    assert_eq!(
        validate(vec![
            freeze(ETH),
            taira_paused(BSC, true),
            release(TRON, 0, "")
        ]),
        Err(SccpGovernanceStaticError::ZeroAmount { action: 2 })
    );
    let rendered = SccpGovernanceStaticError::MemoTooLong {
        action: 5,
        len: 300,
    }
    .to_string();
    assert!(
        rendered.contains('5') && rendered.contains("300"),
        "{rendered}"
    );
    let nested = SccpGovernanceStaticError::InvalidParameters {
        action: 0,
        error: SccpParametersError::AttestationStallOutOfRange,
    }
    .to_string();
    assert!(nested.contains("attestation_stall_ms"), "{nested}");
}

// ---------------------------------------------------------------------------------------------
// Exact JSON integer invariant (2^53 − 1)
// ---------------------------------------------------------------------------------------------

/// Builds an action carrying one `u64` value at a fixed payload location.
type U64Location = Box<dyn Fn(u64) -> SccpGovernanceActionV1>;

fn location(build: impl Fn(u64) -> SccpGovernanceActionV1 + 'static) -> U64Location {
    Box::new(build)
}

fn lc_location(update: fn(&mut SccpLightClientParamsV1, u64)) -> U64Location {
    location(move |value| {
        let mut action = initialize(ETH);
        update(&mut action.params, value);
        SccpGovernanceActionV1::InitializeLightClient(action)
    })
}

fn params_location(update: fn(&mut SccpParametersV1, u64)) -> U64Location {
    location(move |value| {
        let mut next = SccpParametersV1::taira_default();
        update(&mut next, value);
        set_parameters(next)
    })
}

/// Every location at which a proposal can carry a `u64`, besides `base_revisions`.
fn u64_locations() -> Vec<(&'static str, U64Location)> {
    vec![
        (
            "initial_roster_generation",
            location(|value| {
                SccpGovernanceActionV1::RegisterRoute(SccpRegisterRouteActionV1 {
                    network: ETH,
                    revision: 1,
                    deployment: evm([0x22; 20]),
                    max_wrapped_supply: 1,
                    initial_roster_generation: value,
                })
            }),
        ),
        ("ws_bound_ms", lc_location(|p, v| p.ws_bound_ms = v)),
        (
            "set_retention_ms",
            lc_location(|p, v| p.set_retention_ms = v),
        ),
        (
            "checkpoint_stride",
            lc_location(|p, v| p.checkpoint_stride = v),
        ),
        (
            "checkpoint_prune_after_ms",
            lc_location(|p, v| p.checkpoint_prune_after_ms = v),
        ),
        (
            "source_height",
            location(|value| {
                let mut data = checkpoint();
                data.source_height = value;
                install_checkpoint(ETH, data)
            }),
        ),
        (
            "source_time_ms",
            location(|value| {
                let mut data = checkpoint();
                data.source_time_ms = value;
                install_checkpoint(ETH, data)
            }),
        ),
        (
            "roster_max_age_ms",
            params_location(|p, v| p.roster_max_age_ms = v),
        ),
        (
            "roster_validity_ms",
            params_location(|p, v| p.roster_validity_ms = v),
        ),
        (
            "outbound_ttl_ms",
            params_location(|p, v| p.outbound_ttl_ms = v),
        ),
        (
            "attestation_retention_ms",
            params_location(|p, v| p.attestation_retention_ms = v),
        ),
        (
            "attestation_stall_ms",
            params_location(|p, v| p.attestation_stall_ms = v),
        ),
        ("fault height", location(|value| clear_fault(3, value))),
    ]
}

#[test]
fn every_payload_u64_is_bounded_by_two_to_the_fifty_three() {
    let locations = u64_locations();
    assert_eq!(locations.len(), 13);
    for (field, build) in &locations {
        let at = proposal(vec![build(JSON_MAX)]);
        assert_eq!(at.first_json_u64_violation(), None, "{field} at 2^53 - 1");
        assert_eq!(at.actions[0].first_json_u64_violation(), None, "{field}");

        let beyond = proposal(vec![build(JSON_MAX + 1)]);
        let detail = beyond
            .first_json_u64_violation()
            .unwrap_or_else(|| panic!("{field} at 2^53 must be reported"));
        assert!(detail.contains(field), "{detail} should name {field}");
        assert_eq!(
            beyond.actions[0].first_json_u64_violation(),
            Some(detail),
            "{field}"
        );
        let maximum = proposal(vec![build(u64::MAX)]);
        assert!(maximum.first_json_u64_violation().is_some(), "{field}");
    }
    // Actions without u64 fields never report a violation.
    for action in every_action() {
        assert_eq!(action.first_json_u64_violation(), None, "{action:?}");
    }
}

#[test]
fn base_revisions_are_bounded_by_two_to_the_fifty_three() {
    let live = network_id(1);
    let mut at = proposal(every_action());
    for entry in &mut at.base_revisions {
        entry.revision = JSON_MAX;
    }
    assert_eq!(at.first_json_u64_violation(), None);
    assert_eq!(at.validate_static(&live), Ok(()));
    for index in 0..at.base_revisions.len() {
        let mut beyond = at.clone();
        beyond.base_revisions[index].revision = JSON_MAX + 1;
        let detail = beyond
            .first_json_u64_violation()
            .expect("base revision at 2^53 is reported");
        assert!(detail.contains("base revision"), "{detail}");
        assert_eq!(
            beyond.validate_static(&live),
            Err(SccpGovernanceStaticError::ExceedsJsonSafeInteger { detail })
        );
    }
}

#[test]
fn static_validation_reports_the_json_bound() {
    let live = network_id(1);
    let generation = proposal(vec![SccpGovernanceActionV1::RegisterRoute(
        SccpRegisterRouteActionV1 {
            network: ETH,
            revision: 1,
            deployment: evm([0x22; 20]),
            max_wrapped_supply: 1,
            initial_roster_generation: JSON_MAX + 1,
        },
    )]);
    assert!(matches!(
        generation.validate_static(&live),
        Err(SccpGovernanceStaticError::ExceedsJsonSafeInteger { detail })
            if detail.contains("initial_roster_generation")
    ));
    let mut data = checkpoint();
    data.source_time_ms = JSON_MAX + 1;
    assert!(matches!(
        proposal(vec![install_checkpoint(ETH, data)]).validate_static(&live),
        Err(SccpGovernanceStaticError::ExceedsJsonSafeInteger { detail })
            if detail.contains("source_time_ms")
    ));
    assert!(matches!(
        proposal(vec![clear_fault(3, JSON_MAX + 1)]).validate_static(&live),
        Err(SccpGovernanceStaticError::ExceedsJsonSafeInteger { detail })
            if detail.contains("fault height")
    ));
    // Light-client params above the bound fail their own validation first.
    let mut lc_over = initialize(ETH);
    lc_over.params.checkpoint_prune_after_ms = JSON_MAX + 1;
    assert_eq!(
        proposal(vec![SccpGovernanceActionV1::InitializeLightClient(lc_over)])
            .validate_static(&live),
        Err(SccpGovernanceStaticError::InvalidLightClientParams {
            action: 0,
            error: SccpLightClientParamsError::ExceedsJsonSafeInteger {
                field: "checkpoint_prune_after_ms"
            },
        })
    );
}
