//! Time-trigger transcript identities must survive block execution and FASTPQ context capture.

use super::*;
use crate::{block::BlockBuilder, kura::Kura, query::store::LiveQueryStore};
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
    let new_block = BlockBuilder::new(Vec::new())
        .chain(0, None)
        .sign(keypair.private_key())
        .unpack(|_| {});
    let header = new_block.header();
    let expected_calls;
    let expected_time_entrypoints;
    {
        let mut trial = state.block(header);
        let event = trial.create_time_event(&header);
        expected_calls = trigger_ids
            .iter()
            .enumerate()
            .map(|(index, id)| {
                trial
                    .transaction()
                    .seed_time_trigger_invocation_call_hash(id, &authority, &event, index)
            })
            .collect::<Vec<_>>();
        let (entrypoints, display_hashes, results, calls) = trial.execute_time_triggers(&header);
        assert_eq!(entrypoints.len(), 2);
        expected_time_entrypoints = entrypoints
            .iter()
            .cloned()
            .map(TransactionEntrypoint::Time)
            .collect::<Vec<_>>();
        assert_eq!(calls, expected_calls);
        assert!(results[0].is_ok(), "{:?}", results[0]);
        assert!(results[1].is_err(), "overdrawn transfer must fail");
        for (entrypoint, display_hash) in entrypoints.iter().zip(&display_hashes) {
            assert_eq!(entrypoint.hash_as_entrypoint(), *display_hash);
            assert!(!calls.contains(&Hash::from(*display_hash)));
        }
        let transcripts = trial.drain_transfer_transcripts();
        assert_eq!(
            transcripts.keys().copied().collect::<Vec<_>>(),
            vec![calls[0]]
        );
        assert_eq!(transcripts[&calls[0]][0].batch_hash, calls[0]);
        let sources = trial.captured_fastpq_transcript_sources().unwrap();
        assert_eq!(sources.keys().copied().collect::<Vec<_>>(), vec![calls[0]]);
        assert!(!sources[&calls[0]].is_protocol_purpose());
        assert_eq!(
            sources[&calls[0]].route(),
            crate::fastpq::FastpqCapturedSourceRoute::Unrouted
        );
        assert_eq!(sources[&calls[0]].dataspace_id(), DataSpaceId::UNIVERSAL);
    }
    // Execute the full result-recording path again from the same unchanged state.
    let mut block = state.block(header);
    let _valid = new_block
        .validate_and_record_transactions(&mut block)
        .unpack(|_| {});
    assert_eq!(block.fastpq_entry_dataspaces.len(), 2);
    let inventory = block.fastpq_source_inventory().unwrap().unwrap();
    assert_eq!(
        inventory
            .entries()
            .iter()
            .map(|entry| entry.entry_hash)
            .collect::<Vec<_>>(),
        expected_calls,
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
    assert_eq!(
        block
            .captured_fastpq_transcript_sources()
            .unwrap()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![expected_calls[0]],
        "failed invocation must leave neither transfers nor source provenance"
    );
    for call in &expected_calls {
        assert_eq!(
            block.fastpq_entry_dataspaces.get(call),
            Some(&DataSpaceId::UNIVERSAL)
        );
    }
    // Invocation call hashes identify source entries. The wire commitment also
    // binds both complete Time entrypoints, including the rejected invocation.
    let canonical_wire_hash: [u8; 32] =
        iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(
            expected_time_entrypoints.iter(),
        )
        .unwrap()
        .into();
    let omitted_failed_wire: [u8; 32] =
        iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(
            expected_time_entrypoints[..1].iter(),
        )
        .unwrap()
        .into();
    let reversed_wires: [u8; 32] = iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(
        expected_time_entrypoints.iter().rev(),
    )
    .unwrap()
    .into();
    assert_ne!(canonical_wire_hash, omitted_failed_wire);
    assert_ne!(canonical_wire_hash, reversed_wires);
    assert_eq!(block.fastpq_tx_set_hash, Some(canonical_wire_hash));
    assert_eq!(inventory.tx_set_hash(), canonical_wire_hash);
}
