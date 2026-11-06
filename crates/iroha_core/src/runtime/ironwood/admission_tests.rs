// Native proposal carriers retain their existing opaque-row behavior under the predecessor.
use super::*;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    privacy::{
        OrchardLimitsV1, PrivacyProtocolIdV1, PrivacyRuntimeProtocolIdV1,
    },
};
use iroha_executor_data_model::isi::multisig::{
    MultisigProposalId, MultisigProposalNonce, MultisigProposalState,
    MultisigProposalTerminalExecutionStateV1, MultisigProposalTerminalState,
    MultisigProposalTerminalStatus, MultisigProposalValue,
};
use iroha_model_base::state_path::StatePath;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn proposal_rows(protocol: PrivacyRuntimeProtocolIdV1) -> [(StatePath, Vec<u8>); 3] {
    let account = iroha_test_samples::ALICE_ID.clone();
    let network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x91; 32])));
    let nonce = MultisigProposalNonce::new([0x92; 32]).unwrap();
    let instruction: InstructionBox =
        if protocol == PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1 {
            SchedulePrivacyIronwoodLimitsUpdateV1::new(
                100,
                OrchardLimitsV1 {
                    max_action_count: 2,
                },
            )
            .into()
        } else {
            let protocol: PrivacyProtocolIdV1 = protocol.try_into().unwrap();
            SchedulePrivacyProtocolLimitsUpdateV1::new(
                protocol,
                100,
                iroha_data_model::privacy::PrivacyRuntimeProtocolLimitsV1::canonical_default_v1(protocol.into()).try_into().unwrap(),
            )
            .into()
        };
    let values = vec![instruction];
    let proposal = MultisigProposalState {
        network_id: network,
        multisig_account_id: account.clone(),
        proposal_nonce: nonce,
        proposal_id: MultisigProposalId::derive(&network, &account, &nonce),
        instructions_hash: HashOf::new(&values),
        transaction_ttl_ms: None,
        instructions: values,
        proposed_at_ms: 0,
        expires_at_ms: 100,
        approvals: BTreeSet::new(),
    };
    let terminal = MultisigProposalTerminalState::new(
        account.clone(),
        proposal.proposal_id,
        proposal.instructions_hash,
        MultisigProposalValue {
            network_id: network,
            proposal_nonce: nonce,
            proposal_id: proposal.proposal_id,
            transaction_ttl_ms: None,
            instructions: proposal.instructions.clone(),
            proposed_at_ms: 0,
            expires_at_ms: 100,
            approvals: BTreeSet::new(),
        },
        MultisigProposalTerminalStatus::Canceled,
        1,
    );
    let execution =
        MultisigProposalTerminalExecutionStateV1::new(terminal.clone(), account, 1, [0x93; 32]);
    [
        (
            "multisig/proposal/fixture".parse().unwrap(),
            norito::encode_canonical(&proposal).unwrap(),
        ),
        (
            "multisig/proposal-terminal/fixture".parse().unwrap(),
            norito::encode_canonical(&terminal).unwrap(),
        ),
        (
            "multisig/proposal-terminal-execution/fixture"
                .parse()
                .unwrap(),
            norito::encode_canonical(&execution).unwrap(),
        ),
    ]
}

#[test]
fn ironwood_proposal_row_gate_preserves_opaque_bytes_and_old_typed_carriers() {
    let catalog = crate::runtime::release_catalog().unwrap();
    let baseline = &catalog.baseline;
    let successor = &catalog.ironwood.as_ref().unwrap().selection;
    for (key, old) in proposal_rows(PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1) {
        proposal_state_row(&key, &old, baseline).unwrap();
        proposal_state_row(&key, &old, successor).unwrap();
        for malformed in [
            Vec::new(),
            vec![0xff],
            norito::encode_canonical(&42_u64).unwrap(),
        ] {
            proposal_state_row(&key, &malformed, baseline).unwrap();
        }
    }
    for (key, new) in proposal_rows(PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1) {
        assert!(
            matches!(proposal_state_row(&key, &new, baseline), Err(Attempt::Rejected(message)) if message.contains("inactive Ironwood"))
        );
        proposal_state_row(&key, &new, successor).unwrap();
        let ordinary: StatePath = format!("sc/contract/{}", key.as_ref()).parse().unwrap();
        proposal_state_row(&ordinary, &new, baseline).unwrap();
        durable_state_overlay(&BTreeMap::from([(key, None)]), baseline).unwrap();
    }
}

#[test]
fn ironwood_all_proposal_carriers_in_current_and_undo_keep_original_bytes() {
    for (key, bytes) in proposal_rows(PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1) {
        for undo in [false, true] {
            let mut value = crate::state::World::default();
            value.smart_contract_state = if undo {
                mv::storage::Storage::from_snapshot_parts(
                    BTreeMap::new(),
                    BTreeMap::from([(key.clone(), Some(bytes.clone()))]),
                )
            } else {
                BTreeMap::from([(key.clone(), bytes.clone())])
                    .into_iter()
                    .collect()
            };
            let before = norito::json::to_json(&value.smart_contract_state).unwrap();
            assert!(
                matches!(world(&mut value), Err(Attempt::Rejected(message)) if message.contains("inactive Ironwood"))
            );
            assert_eq!(
                norito::json::to_json(&value.smart_contract_state).unwrap(),
                before
            );
            super::super::install_for_test(&mut value);
            world(&mut value).unwrap();
            assert_eq!(
                norito::json::to_json(&value.smart_contract_state).unwrap(),
                before
            );
        }
    }
}

pub(crate) fn opaque_proposal(protocol: PrivacyRuntimeProtocolIdV1) -> InstructionBox {
    let (_, bytes) = proposal_rows(protocol).into_iter().next().unwrap();
    let proposal: MultisigProposalState = norito::decode_from_bytes(&bytes).unwrap();
    iroha_executor_data_model::isi::multisig::MultisigPropose::new(
        proposal.multisig_account_id,
        proposal.proposal_nonce,
        proposal.instructions,
        None,
    )
    .into()
}

#[test]
fn ironwood_opaque_custom_is_wire_admissible_but_not_a_predecessor_native_proposal() {
    use iroha_executor_data_model::isi::multisig::MultisigPropose;
    let catalog = crate::runtime::release_catalog().unwrap();
    let baseline = &catalog.baseline;
    let successor = &catalog.ironwood.as_ref().unwrap().selection;
    let old = opaque_proposal(PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1);
    let new = opaque_proposal(PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1);
    for value in [&old, &new] {
        instruction(value, baseline).unwrap();
        multisig_instruction(value, successor).unwrap();
    }
    multisig_instruction(&old, baseline).unwrap();
    assert!(multisig_instruction(&new, baseline).is_err());
    // A nested Custom object was opaque even inside a successfully decoded old proposal.
    let nested: InstructionBox = MultisigPropose::new(
        iroha_test_samples::ALICE_ID.clone(),
        MultisigProposalNonce::new([0x94; 32]).unwrap(),
        vec![new],
        None,
    )
    .into();
    instruction(&nested, baseline).unwrap();
    let MultisigInstructionBox::Propose(parsed) = multisig_instruction(&nested, baseline).unwrap()
    else {
        panic!("proposal");
    };
    assert!(multisig_instruction(&parsed.instructions[0], baseline).is_err());
}
