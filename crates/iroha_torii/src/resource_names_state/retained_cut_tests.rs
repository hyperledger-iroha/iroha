//! A real successor publication cannot split the names proof from its retained originals.

use super::*;
use iroha_core::{
    state::World,
    sumeragi::{
        node::NodeIdentity,
        test_chain::{CertifiedTestChain, TestChainConfig},
    },
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{Registrable, account::Account};
use std::{collections::BTreeSet, sync::Arc};

#[test]
fn names_proof_and_snapshot_keep_one_original_cut_while_chain_advances() {
    let reader_key = KeyPair::from_seed(vec![0x53; 32], Algorithm::Ed25519);
    let reader = iroha_data_model::account::AccountId::new(reader_key.public_key().clone());
    let mut world = World::with([], [Account::new(reader.clone()).build(&reader)], []);
    world.account_permissions_mut_for_testing().insert(
        reader.clone(),
        BTreeSet::from([
            iroha_executor_data_model::permission::query::CanReadCertifiedOriginals.into(),
        ]),
    );
    world
        .smart_contract_state_mut_for_testing()
        .insert("customer/retained-name".parse().unwrap(), vec![1, 2, 3]);
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(world, 1_000))
        .map_err(|error| error.error)
        .unwrap();
    chain.commit_at(2_000, Vec::new());
    let state = Arc::clone(chain.state());
    let budget = AllocationBudget::new(48 * 1024 * 1024);
    let captured = state
        .capture_native_resource_names_snapshot_v1(&reader, &budget)
        .unwrap();
    let height = u64::try_from(captured.view().height()).unwrap();
    assert_eq!(height, 2);
    let signer = KeyPair::from_seed(vec![0x54; 32], Algorithm::BlsNormal);
    let identity = NodeIdentity {
        node_id: iroha_model_base::peer::PeerId::new(signer.public_key().clone()),
        config_fingerprint: iroha_crypto::Hash::new(b"retained names configuration"),
    };
    let status = iroha_data_model::sumeragi::SumeragiStatus {
        protocol_version: iroha_data_model::sumeragi::PROTOCOL_VERSION,
        config_fingerprint: identity.config_fingerprint,
        beacon_horizon: None,
        instance: chain.instance().0,
        height: height + 1,
        view: 0,
        stage: 0,
        leader: None,
        proxy_tail: None,
        high_qc_view: None,
        level: 0,
        start_level: 0,
        t_retx_ms: 100,
        committed_height: height,
        applied_height: height,
        awaiting: false,
        signer: Some(signer.public_key().clone()),
        unanchored: false,
        abstaining: false,
        halted: None,
        footprint: iroha_data_model::sumeragi::SumeragiFootprint::default(),
    };
    // Publish actual certified successors after the original acquisition and
    // before either portable proof is constructed. No writer remains held.
    chain.commit_at(3_000, Vec::new());
    chain.commit_at(4_000, Vec::new());
    assert_eq!(state.view().height(), 4);
    let limits = NativeFinalityLimits {
        block_bytes: NATIVE_FINALITY_MAX_BLOCK_BYTES,
        journal_bytes: NATIVE_FINALITY_MAX_JOURNAL_BYTES,
        block_count: 3,
        allocated_bytes: 24 * 1024 * 1024,
    };
    let tip =
        current_names_tip(captured.view(), height, SumeragiRootScope::Global, limits).unwrap();
    require_status_height(&status, height).unwrap();
    let attestation = iroha_core::sumeragi::finality::build_attestation(
        captured.view(),
        status,
        &identity,
        iroha_crypto::Hash::new(b"retained names executable"),
        height,
        [0x53; 32],
        &signer,
    )
    .unwrap();
    let encoded = captured
        .with_native_resource_names_snapshot_v1(&tip, |snapshot, aliases, keys, names| {
            assert!(aliases.is_empty());
            assert!(
                keys.iter()
                    .any(|key| key.as_ref() == "customer/retained-name")
            );
            assert_eq!(
                snapshot.root().unwrap(),
                tip.commitment().execution.world_state_root
            );
            let names = names
                .iter()
                .map(|(key, value)| NativeDataspaceSnsOriginalRefV1::new(key, value.as_slice()))
                .collect::<Vec<_>>();
            let payload =
                NativeResourceNamesStateRefV1::new(&attestation, snapshot, &[], keys, &names);
            encode(
                &payload,
                ResponseFormat::Norito,
                NATIVE_RESOURCE_NAMES_STATE_MAX_BYTES_V1,
                &budget,
            )
            .map_err(|error| error.to_string())
        })
        .unwrap();
    let decoded =
        iroha_torii_shared::resource_names_state::decode_unverified_native_resource_names_state_v1(
            encoded.as_ref(),
        )
        .unwrap();
    assert_eq!(decoded.attestation.body.challenge, [0x53; 32]);
    assert_eq!(decoded.attestation.body.status.applied_height, 2);
    assert_eq!(
        decoded.attestation.body.finality_proof.block_header.hash(),
        tip.block_hash()
    );
    assert_eq!(
        decoded.world_snapshot.root().unwrap(),
        tip.commitment().execution.world_state_root
    );
    decoded
        .attestation
        .verify(iroha_data_model::sumeragi_finality::CertifiedResultScope::TairaGlobal)
        .unwrap();
    assert!(
        captured
            .with_native_resource_names_snapshot_v1(&chain.committed(4), |_, _, _, _| Ok(()),)
            .is_err(),
        "a later certified tip cannot relabel the retained originals"
    );
    drop(decoded);
    drop(encoded);
    drop(captured);
    assert_eq!(budget.reserved_bytes(), 0);
}
