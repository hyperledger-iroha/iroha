//! Genuine native BLS inclusion and registry coverage; receipt execution remains synthetic.

use super::*;
use iroha_crypto::{HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::consensus::{ExecKv, ExecWitness},
    private_dataspace::{
        PrivateDataspaceAdmissionPolicy, PrivateDataspaceAnchor, PrivateDataspaceRegistration,
    },
    runtime::{checkpoint::StartupFloorV1, retirement::RuntimeRetirementReceiptV1},
    sumeragi_amx::{AmxCertifiedBlockV1, AmxWriteProofV1},
    sumeragi_finality::{
        SUMERAGI_LANE_STATE_WITNESS_KEY, SumeragiLaneStateCommitment, authenticated_genesis,
        test_fixtures::NativeFinalityFixture,
    },
};
use iroha_model_base::topology::DataSpaceId;

fn large_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn parent() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"retirement parent",
    )))
}

fn child(registry: &mut PrivateDataspaceRegistry, id: u64) -> RuntimeRetirementProofV1 {
    let scope = SumeragiRootScope::Dataspace {
        parent_network_id: parent(),
        dataspace_id: DataSpaceId::new(id),
    };
    let mut fixture =
        NativeFinalityFixture::start_with_scope(&format!("retirement-child-{id}"), scope);
    let genesis = fixture
        .verifier()
        .verify_retained_decision(fixture.genesis_proof())
        .unwrap();
    let registration = PrivateDataspaceRegistration::new(
        scope,
        fixture.chain_id().parse().unwrap(),
        fixture.network_id(),
        genesis.result().0,
        authenticated_genesis(fixture.genesis())
            .unwrap()
            .into_parts()
            .0,
    )
    .unwrap();
    let owner = AccountId::new(
        KeyPair::from_seed(vec![31; 32], iroha_crypto::Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    registry
        .register_authorized(
            PrivateDataspaceAdmissionPolicy {
                max_registered_roots: 2,
                max_roots_per_owner: 2,
            },
            format!("child-{id}"),
            owner.clone(),
            1,
            registration.clone(),
        )
        .unwrap();
    for _ in 2..=4 {
        let block = fixture.block_with_submitted_work(fixture.next_header());
        let proof = fixture.certify(block);
        let verified = fixture.verifier().verify_retained_decision(&proof).unwrap();
        let anchor = PrivateDataspaceAnchor::from_certificate(
            &registration,
            verified.block().commit_certificate().unwrap(),
        )
        .unwrap();
        registry
            .apply_authorized(DataSpaceId::new(id), &owner, 1, &anchor)
            .unwrap();
    }
    let receipt = RuntimeRetirementReceiptV1 {
        root: RuntimeRetirementRootV1 {
            network: fixture.network_id(),
            scope,
            instance: registration.instance,
        },
        definition: DefinitionId([4; 32]),
        history_started: 2,
        last_required: Some(3),
        excluded_at: 4,
        startup_floor: StartupFloorV1 {
            network: fixture.network_id(),
            height: 4,
            block_hash: *fixture.latest().block_header.hash().as_ref(),
            committed_ms: fixture.latest().block_header.creation_time_ms,
        },
        receipt_height: 5,
    };
    let state =
        SumeragiLaneStateCommitment::from_state(fixture.network_id(), 5, &Default::default())
            .unwrap();
    let witness = ExecWitness {
        writes: vec![
            ExecKv {
                key: SUMERAGI_LANE_STATE_WITNESS_KEY.to_vec(),
                value: norito::encode_canonical(&state).unwrap(),
            },
            ExecKv {
                key: receipt.witness_key(),
                value: norito::encode_canonical(&receipt).unwrap(),
            },
        ],
        ..Default::default()
    };
    let (path, _) = AmxWriteProofV1::from_writes(
        witness
            .writes
            .iter()
            .map(|kv| (kv.key.as_slice(), kv.value.as_slice())),
        &receipt.witness_key(),
    )
    .unwrap();
    let block = fixture.block_with_submitted_work(fixture.next_header());
    let proof = fixture.certify_with_witness(block, &witness);
    let verified = fixture.verifier().verify_retained_decision(&proof).unwrap();
    let certificate = verified.block().commit_certificate().unwrap();
    let anchor = PrivateDataspaceAnchor::from_certificate(&registration, certificate).unwrap();
    registry
        .apply_authorized(DataSpaceId::new(id), &owner, 1, &anchor)
        .unwrap();
    RuntimeRetirementProofV1 {
        receipt,
        block: AmxCertifiedBlockV1::from_certificate(certificate),
        path,
    }
}

#[test]
fn retirement_registered_children_require_every_exact_original_certificate_and_write() {
    large_stack(|| {
        let mut registry = PrivateDataspaceRegistry::default();
        let first = child(&mut registry, 1);
        let second = child(&mut registry, u64::MAX);
        let initial = Hash::new(b"authenticated parent coverage");
        let proof_for = |root| {
            if root == first.receipt.root {
                first.clone()
            } else {
                second.clone()
            }
        };
        let (digest, count) = verify_children(
            parent(),
            &registry,
            first.receipt.definition,
            initial,
            &mut |root| Ok(Some(proof_for(root))),
        )
        .unwrap();
        assert_eq!(count, 2);
        assert_ne!(digest, initial);
        assert!(
            verify_children(
                parent(),
                &registry,
                first.receipt.definition,
                initial,
                &mut |root| { Ok((root == first.receipt.root).then(|| first.clone())) }
            )
            .is_err(),
            "unreachable registered root must block coverage"
        );
        assert!(
            verify_children(
                parent(),
                &registry,
                first.receipt.definition,
                initial,
                &mut |_| Ok(Some(first.clone()))
            )
            .is_err(),
            "one root cannot cover a different full-width identity"
        );
        let wrong_parent = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"foreign parent",
        )));
        assert!(
            verify_children(
                wrong_parent,
                &registry,
                first.receipt.definition,
                initial,
                &mut |root| Ok(Some(proof_for(root)))
            )
            .is_err()
        );
    });
}

#[test]
fn retirement_changed_quorum_result_scope_floor_and_cursor_are_rejected() {
    large_stack(|| {
        let mut registry = PrivateDataspaceRegistry::default();
        let original = child(&mut registry, 1);
        let record = registry.get(DataSpaceId::new(1)).unwrap();
        verify_child(record, original.receipt.definition, &original).unwrap();
        let mut changed = original.clone();
        changed.block.commit_qc[0] ^= 1;
        assert!(verify_child(record, original.receipt.definition, &changed).is_err());
        changed = original.clone();
        changed.block.result_preimage[0] ^= 1;
        assert!(verify_child(record, original.receipt.definition, &changed).is_err());
        changed = original.clone();
        changed.receipt.root.scope = SumeragiRootScope::Global;
        assert!(verify_child(record, original.receipt.definition, &changed).is_err());
        changed = original.clone();
        changed.receipt.startup_floor.height = 3;
        assert!(verify_child(record, original.receipt.definition, &changed).is_err());
        changed = original.clone();
        changed.receipt.last_required = None;
        assert!(verify_child(record, original.receipt.definition, &changed).is_err());
        let mut stale = record.clone();
        stale.anchor = iroha_data_model::private_dataspace::PrivateDataspaceAnchorState::from_authorized_registration(record.anchor.registration().clone()).unwrap();
        assert!(verify_child(&stale, original.receipt.definition, &original).is_err());
    });
}

#[test]
fn retirement_coverage_preserves_original_codec_refusal_without_forging_completion() {
    large_stack(|| {
        let mut registry = PrivateDataspaceRegistry::default();
        let proof = child(&mut registry, 1);
        let record = registry.get(DataSpaceId::new(1)).unwrap();
        let error = norito::core::with_decode_limits_scope(
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
            || verify_child(record, proof.receipt.definition, &proof),
        )
        .unwrap_err();
        assert!(matches!(error, Attempt::Deferred(_)));
        verify_child(record, proof.receipt.definition, &proof).unwrap();
    });
}
