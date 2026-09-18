//! Time-trigger transcript identities must survive block execution and FASTPQ context capture.

use super::*;
use crate::{kura::Kura, query::store::LiveQueryStore};
use iroha_data_model::{
    account::Account,
    asset::{Asset, AssetBalancePolicy, AssetDefinition, AssetId},
    domain::Domain,
    events::time::{ExecutionTime, TimeEventFilter},
    isi::Transfer,
    prelude::{Action, Repeats},
    trigger::Trigger,
};
use iroha_test_samples::{BOB_ID, gen_account_in};

#[test]
fn time_trigger_call_hashes_bind_transcripts_and_include_failed_invocations() {
    let (authority, keypair) = gen_account_in("wonderland");
    let domain_id = DomainId::try_new("wonderland", "universal").unwrap();
    let asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id.clone(), "rose".parse().unwrap());
    let asset_id = AssetId::new(asset_definition_id.clone(), authority.clone());
    let world = World::with_assets(
        [Domain::new(domain_id).build(&authority)],
        [
            Account::new(authority.clone()).build(&authority),
            Account::new((*BOB_ID).clone()).build(&authority),
        ],
        [AssetDefinition::numeric(
            asset_definition_id,
            "rose",
            AssetBalancePolicy::Global,
            None,
        )
        .build(&authority)],
        [Asset::new(asset_id.clone(), Quantity::from(100u32))],
        [],
    );
    let state = State::new(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let trigger_ids = [
        "a_fastpq_success".parse::<TriggerId>().unwrap(),
        "z_fastpq_failure".parse().unwrap(),
    ];
    {
        let mut trigger_block = state.world.triggers.block();
        let mut transaction = trigger_block.transaction();
        for (id, amount) in trigger_ids.iter().zip([10u32, 1000]) {
            let mut metadata = iroha_model_base::metadata::Metadata::default();
            metadata.insert(
                "__registered_block_height".parse::<Name>().unwrap(),
                Json::new(0u64),
            );
            metadata.insert(
                "__registered_at_ms".parse::<Name>().unwrap(),
                Json::new(0u64),
            );
            let trigger = Trigger::new(
                id.clone(),
                Action::new(
                    [InstructionBox::from(Transfer::asset_quantity(
                        asset_id.clone(),
                        amount,
                        (*BOB_ID).clone(),
                    ))],
                    Repeats::Exactly(1),
                    authority.clone(),
                    TimeEventFilter::new(ExecutionTime::PreCommit),
                )
                .unwrap()
                .with_metadata(metadata),
            );
            transaction
                .add_time_trigger(trigger.try_into().unwrap())
                .unwrap();
        }
        transaction.apply();
        trigger_block.commit();
    }
    // This fixture starts from a prebuilt World. Publish its explicit empty
    // predecessor to seed asset incarnations and the Time interval; the tested
    // transfers below still execute through the complete ordinary block owner.
    // This setup does not claim an executed or finality-certified genesis.
    let predecessor = iroha_data_model::block::builder::BlockBuilder::new(
        iroha_data_model::block::BlockHeader::new(std::num::NonZeroU64::MIN, None, None, 0, 0),
    )
    .build_with_signature(0, keypair.private_key());
    state
        .block(predecessor.header())
        .commit_empty_block_for_testing()
        .unwrap();
    let predecessor_hash = predecessor.hash();
    state.kura_handle().store_block(predecessor).unwrap();
    let header = iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(2).unwrap(),
        Some(predecessor_hash),
        None,
        2,
        0,
    );
    let source = iroha_data_model::block::builder::BlockBuilder::new(header)
        .build_with_signature(0, keypair.private_key());
    let expected_calls = {
        let trial = state.block(header);
        let event = trial.create_time_event(&header);
        trigger_ids.iter().enumerate().map(|(index, id)| {
            iroha_data_model::block::execution_output::TimeInvocationV1 {
                schedule_index: u32::try_from(index).unwrap(),
                event: event.clone(),
                trigger: crate::smartcontracts::isi::triggers::set::invocation_identity::time_trigger_use_v1(
                    &trial.world.triggers, id, header.height().get(),
                ).unwrap(),
            }.execution_call_hash(source.hash()).unwrap()
        }).collect::<Vec<_>>()
    };
    let mut prior_wire = None;
    for _ in 0..2 {
        let mut carrier = source.clone();
        let mut block = state.block(header);
        crate::block::ValidBlock::execute_block_outputs_for_test(&mut carrier, &mut block, None)
            .expect("actual complete Time execution seals successfully");
        assert_eq!(carrier.network_entrypoint_count(), 0);
        let rows = carrier.execution_outputs();
        assert_eq!(rows.len(), 2);
        for (index, row) in rows.iter().enumerate() {
            let iroha_data_model::block::execution_output::ExecutionOutputV1::Time(time) = row
            else {
                panic!("Time invocations are typed outputs, never Network inputs");
            };
            assert_eq!(time.invocation.trigger.trigger_id, trigger_ids[index]);
            assert_eq!(
                time.invocation.execution_call_hash(source.hash()).unwrap(),
                expected_calls[index]
            );
            assert_eq!(time.result.is_ok(), index == 0);
        }
        assert!(rows[1].result().is_err(), "overdrawn transfer must fail");
        let transcripts = carrier.fastpq_transcripts();
        assert_eq!(
            transcripts.keys().copied().collect::<Vec<_>>(),
            vec![expected_calls[0]]
        );
        assert_eq!(
            transcripts[&expected_calls[0]][0].batch_hash,
            expected_calls[0]
        );
        let sources = block.captured_fastpq_transcript_sources().unwrap();
        assert_eq!(
            sources.keys().copied().collect::<Vec<_>>(),
            vec![expected_calls[0]],
            "failed invocation leaves neither transfers nor source provenance"
        );
        assert!(!sources[&expected_calls[0]].is_protocol_purpose());
        assert_eq!(
            sources[&expected_calls[0]].route(),
            crate::fastpq::FastpqCapturedSourceRoute::Unrouted
        );
        assert_eq!(
            sources[&expected_calls[0]].dataspace_id(),
            DataSpaceId::UNIVERSAL
        );
        assert_eq!(block.fastpq_entry_dataspaces.len(), 2);
        let inventory = block.fastpq_source_inventory().unwrap().unwrap();
        assert_eq!(
            inventory
                .entries()
                .iter()
                .map(|entry| entry.entry_hash)
                .collect::<Vec<_>>(),
            expected_calls
        );
        assert_eq!(inventory.transcript_entry_hashes().len(), 1);
        assert!(
            inventory
                .transcript_entry_hashes()
                .contains(&expected_calls[0])
        );
        assert!(
            !inventory
                .transcript_entry_hashes()
                .contains(&expected_calls[1])
        );
        for call in &expected_calls {
            assert_eq!(
                block.fastpq_entry_dataspaces.get(call),
                Some(&DataSpaceId::UNIVERSAL)
            );
        }
        // FASTPQ binds the physical Network input set separately from its complete
        // typed source inventory. Both Time outcomes also enter the output root.
        let network_digest: [u8; 32] =
            iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(
                carrier.external_entrypoints_slice().iter(),
            )
            .unwrap()
            .into();
        assert_eq!(block.fastpq_tx_set_hash, Some(network_digest));
        assert_eq!(inventory.tx_set_hash(), network_digest);
        let root =
            iroha_crypto::MerkleTree::root_from_typed_leaves(rows.iter().map(HashOf::new)).unwrap();
        let omitted =
            iroha_crypto::MerkleTree::root_from_typed_leaves(rows[..1].iter().map(HashOf::new))
                .unwrap();
        let reversed =
            iroha_crypto::MerkleTree::root_from_typed_leaves(rows.iter().rev().map(HashOf::new))
                .unwrap();
        assert_ne!(root, omitted);
        assert_ne!(root, reversed);
        carrier.validate_output_merkle_cache().unwrap();
        let wire = carrier.encode_wire().unwrap();
        if let Some(previous) = &prior_wire {
            assert_eq!(&wire, previous);
        }
        prior_wire = Some(wire);
        // Dropping this overlay retains the exact predecessor for the next run.
    }
}
