//! Public-boundary tests of the SCCP v1 core skeleton (`specs/sccp.md` §4): every v1
//! instruction dispatches and fails closed naming its owning workstream, leaf allocation is dense
//! with the 512-leaf cap, every SCCP instruction routes to the universal dataspace, and a network
//! without SCCP behaves exactly as before.

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::evaluate_policy_plan_with_nexus_and_world_at,
    smartcontracts::{
        Execute,
        isi::sccp::{self, admission, fees, hook, leaves, params, store},
    },
    state::{State, World},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::BlockHeader,
    bridge::SccpNetworkV1,
    isi::{
        InstructionBox,
        error::InstructionExecutionError,
        sccp::{
            AdvanceSccpLightClientV1, InitializeSccpV1, RecordSccpMessage,
            ReportSccpLightClientEquivocationV1, SetSccpBridgeKeyV1, SettleSccpV1,
            SubmitSccpAttestationFaultV1, SubmitSccpAttestationsV1, SubmitSccpInboundMessageV1,
            SubmitSccpOutboundVoidV1,
        },
    },
    nexus::{DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneConfig},
    sccp::{
        attestation::{SccpAttestationSignatureV1, SccpAttestationStatementV1},
        control::SccpLeafRefV1,
        inbound::SccpSourceProofBytesV1,
        keys::SccpBridgeKeyBindingV1,
        light_client::{SccpLcAdvanceBytesV1, SccpLcEvidenceBytesV1},
        params::SccpParametersV1,
    },
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder},
};
use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use iroha_primitives::numeric::Numeric;
use std::num::{NonZeroU32, NonZeroU64};

const PRIVATE_DATASPACE: DataSpaceId = DataSpaceId::new(10);
const PRIVATE_LANE: LaneId = LaneId::new(1);

fn blank_state() -> State {
    State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn header(height: u64) -> BlockHeader {
    BlockHeader::new(
        NonZeroU64::new(height).expect("nonzero height"),
        None,
        None,
        height * 4_000,
        0,
    )
}

fn key_pair(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("deterministic seed")
}

fn account(seed: u8) -> AccountId {
    AccountId::new(key_pair(seed).public_key().clone())
}

fn network_id() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        [0x7a; 32],
    )))
}

fn set_bridge_key() -> SetSccpBridgeKeyV1 {
    let peer_keys = key_pair(0x31);
    let peer = PeerId::new(peer_keys.public_key().clone());
    let binding = SccpBridgeKeyBindingV1::new(network_id(), peer.clone(), Some([2; 33]), 3, 0);
    SetSccpBridgeKeyV1 {
        peer,
        public_key: Some([2; 33]),
        activation_epoch: 3,
        binding_nonce: 0,
        peer_signature: SignatureOf::new(peer_keys.private_key(), &binding),
        key_pop: Some([0x1b; 65]),
    }
}

fn statement() -> SccpAttestationStatementV1 {
    SccpAttestationStatementV1 {
        height: 9,
        epoch: 1,
        timestamp_ms: 36_000,
        block_hash: [1; 32],
        sccp_root: [2; 32],
        message_count: 1,
        history_root: [3; 32],
        history_size: 1,
        roster_digest: [4; 32],
        next_roster_digest: [0; 32],
    }
}

fn proof(tag: u8) -> SccpSourceProofBytesV1 {
    SccpSourceProofBytesV1::new(vec![0x4e, 0x52, 0x54, 0x30, tag]).expect("bounded proof")
}

/// Execute every v1 instruction in a fresh transaction and return `(name, error)`.
fn execute_every_instruction(state: &State) -> Vec<(&'static str, InstructionExecutionError)> {
    let mut block = state.try_block(header(2)).expect("open block");
    let authority = account(1);
    let mut outcomes = Vec::new();
    macro_rules! run {
        ($name:literal, $instruction:expr) => {{
            let mut transaction = block.try_transaction().expect("open transaction");
            let error = Execute::execute($instruction, &authority, &mut transaction)
                .expect_err(concat!($name, " must fail closed in the skeleton"));
            outcomes.push(($name, error));
        }};
    }
    run!(
        "InitializeSccpV1",
        InitializeSccpV1 {
            parameters: SccpParametersV1::taira_default(),
            reset_nonce: [0x5a; 32],
        }
    );
    run!("SetSccpBridgeKeyV1", set_bridge_key());
    run!(
        "SubmitSccpAttestationsV1",
        SubmitSccpAttestationsV1 {
            entries: vec![SccpAttestationSignatureV1 {
                height: 1,
                signer_index: 0,
                signature: [0x1c; 65],
            }],
        }
    );
    run!(
        "SubmitSccpAttestationFaultV1",
        SubmitSccpAttestationFaultV1 {
            statement: statement(),
            signature: [0x1b; 65],
        }
    );
    run!(
        "RecordSccpMessage",
        RecordSccpMessage {
            network: SccpNetworkV1::EthereumMainnet,
            expected_revision: 1,
            amount: Numeric::new(1_000_000_000_u64, 9),
            recipient: vec![0x22; 20],
        }
    );
    run!(
        "SubmitSccpInboundMessageV1",
        SubmitSccpInboundMessageV1 {
            network: SccpNetworkV1::TonMainnet,
            revision: 1,
            payload: vec![2, 1, 0],
            proof: proof(1),
        }
    );
    run!("SettleSccpV1", SettleSccpV1::inbound([6; 32]));
    run!(
        "SubmitSccpOutboundVoidV1",
        SubmitSccpOutboundVoidV1 {
            network: SccpNetworkV1::TronMainnet,
            revision: 1,
            proof: proof(2),
        }
    );
    run!(
        "AdvanceSccpLightClientV1",
        AdvanceSccpLightClientV1 {
            network: SccpNetworkV1::EthereumMainnet,
            expected_state_hash: None,
            advance: SccpLcAdvanceBytesV1::new(vec![1; 8]).expect("bounded advance"),
        }
    );
    run!(
        "ReportSccpLightClientEquivocationV1",
        ReportSccpLightClientEquivocationV1 {
            network: SccpNetworkV1::BscMainnet,
            a: SccpLcEvidenceBytesV1::new(vec![1]).expect("bounded evidence"),
            b: SccpLcEvidenceBytesV1::new(vec![2]).expect("bounded evidence"),
        }
    );
    outcomes
}

#[test]
fn every_stub_instruction_fails_with_its_todo_error() {
    let state = blank_state();
    let owners = [
        "ws31", "ws31", "ws31", "ws31", "ws32", "ws41", "ws41", "ws41", "ws41", "ws41",
    ];
    let outcomes = execute_every_instruction(&state);
    assert_eq!(outcomes.len(), owners.len());
    for ((name, error), owner) in outcomes.into_iter().zip(owners) {
        let InstructionExecutionError::InvariantViolation(message) = &error else {
            panic!("{name}: unexpected error kind {error:?}");
        };
        assert_eq!(
            message.as_ref(),
            format!("SCCP: {name} execution not implemented yet (TODO({owner}))"),
            "{name}"
        );
    }
}

#[test]
fn leaves_are_dense_per_block_and_capped_at_512() {
    let state = blank_state();
    let mut block = state.try_block(header(5)).expect("open block");
    let mut transaction = block.try_transaction().expect("open transaction");
    let control = SccpLeafRefV1::control(SccpNetworkV1::TonMainnet, 1, 1);
    assert_eq!(leaves::allocate_leaf(&mut transaction, control), Ok(0));
    for index in 1..leaves::MAX_LEAVES_PER_BLOCK {
        let mut id = [0_u8; 32];
        id[..4].copy_from_slice(&index.to_be_bytes());
        assert_eq!(
            leaves::allocate_leaf(&mut transaction, SccpLeafRefV1::transfer(id)),
            Ok(index)
        );
    }
    assert!(
        leaves::allocate_leaf(&mut transaction, SccpLeafRefV1::transfer([0xff; 32])).is_err(),
        "the 513th leaf of a block is refused"
    );
    let recorded = leaves::leaves_at(&*transaction.world, 5);
    assert_eq!(recorded.len(), 512);
    assert_eq!(
        recorded[0], control,
        "index 0 holds the first allocated leaf"
    );
    assert!(leaves::leaves_at(&*transaction.world, 4).is_empty());
    assert_eq!(leaves::leaf_count_at(&*transaction.world, 5), 512);
}

fn signed(instructions: Vec<InstructionBox>) -> SignedTransaction {
    let signer = key_pair(0x41);
    TransactionBuilder::new(
        network_id(),
        AccountId::new(signer.public_key().clone()),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions)
    .sign(signer.private_key())
}

#[test]
fn every_sccp_instruction_routes_to_the_universal_dataspace() {
    let state = blank_state();
    // The two-lane Nexus configuration is evaluated directly: the blank test Kura pins the
    // default lane catalog as the immutable configured baseline, so State keeps its own.
    let mut nexus = state.nexus_snapshot();
    let lanes = LaneCatalog::new(
        NonZeroU32::new(2).expect("nonzero lane bound"),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: PRIVATE_LANE,
                dataspace_id: PRIVATE_DATASPACE,
                alias: "private".to_owned(),
                ..LaneConfig::default()
            },
        ],
    )
    .expect("valid lane catalog");
    nexus.lane_catalog = lanes.clone();
    nexus.lane_config = iroha_config::parameters::actual::LaneConfig::from_catalog(&lanes);
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: PRIVATE_DATASPACE,
            alias: "private".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("valid dataspace catalog");
    nexus.routing_policy.default_lane = PRIVATE_LANE;
    nexus.routing_policy.default_dataspace = PRIVATE_DATASPACE;
    let view = state.view();
    let route = |instructions: Vec<InstructionBox>| {
        evaluate_policy_plan_with_nexus_and_world_at(
            &nexus,
            signed(instructions).payload(),
            view.world(),
            0,
        )
        .expect("routing resolves")
        .coordinator_route()
    };
    let ordinary = route(vec![
        iroha_data_model::isi::Log::new(iroha_data_model::Level::INFO, "ordinary".to_owned())
            .into(),
    ]);
    assert_eq!(
        ordinary.dataspace_id, PRIVATE_DATASPACE,
        "the fixture routes ordinary work away from the universal dataspace"
    );
    let samples: Vec<InstructionBox> = vec![
        SettleSccpV1::inbound([1; 32]).into(),
        RecordSccpMessage {
            network: SccpNetworkV1::EthereumMainnet,
            expected_revision: 1,
            amount: Numeric::new(1_000_000_000_u64, 9),
            recipient: vec![0x22; 20],
        }
        .into(),
        SubmitSccpAttestationsV1 { entries: vec![] }.into(),
        set_bridge_key().into(),
    ];
    for instruction in samples {
        assert!(sccp::is_sccp_instruction(&*instruction));
        assert_eq!(
            route(vec![instruction]).dataspace_id,
            DataSpaceId::UNIVERSAL
        );
    }
}

#[test]
fn a_network_without_sccp_is_unchanged() {
    let state = blank_state();
    let view = state.view();
    let world = view.world();
    assert_eq!(params::parameters(world), None);
    assert!(!params::exists(world));
    assert!(!params::enabled(world));
    assert!(store::routes::is_empty(world));
    assert!(store::rosters::is_empty(world));
    assert_eq!(*store::roster_current::get(world), 0);
    assert!(!hook::heartbeat_start_work_pending(world, &header(2)));
    let transaction = signed(vec![
        iroha_data_model::isi::Log::new(iroha_data_model::Level::INFO, "ordinary".to_owned())
            .into(),
    ]);
    assert_eq!(admission::classify(world, &view, 2, &transaction), Ok(None));
    assert!(!admission::allows_unregistered_authority(
        transaction.instructions(),
        transaction.authority()
    ));
    assert!(admission::block_exempt_cap_ok(world, &[]));
    assert!(!fees::exempt_on_success(world, transaction.payload()));
    let log = InstructionBox::from(iroha_data_model::isi::Log::new(
        iroha_data_model::Level::INFO,
        "ordinary".to_owned(),
    ));
    assert!(!sccp::is_sccp_instruction(&*log));
    drop(view);
    let mut block = state.try_block(header(2)).expect("open block");
    hook::finalize_block(&mut block, &header(2), None).expect("no-op without SCCP");
    assert!(store::block_commitments::is_empty(&block.world));
}
