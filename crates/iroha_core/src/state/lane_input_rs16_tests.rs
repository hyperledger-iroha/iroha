// Native materialization through real first-carrier/frozen-context boundary fixtures.

state_test! { sync native_lane_input_rs16_materializes_identical_codeword_for_every_route_and_preserves_origin
    use super::{FirstLaneAdmittedInputReadV1, LaneInputBodyPreparationV1};
    use crate::sumeragi::v2_lane_payload::{encode_lane_input, verify_lane_input_manifest};
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let mut previous = None;
    for lane in observed.contexts() {
        let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&observed, lane).unwrap() else { panic!("actual source"); };
        let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&observed, lane, &source).unwrap() else { panic!("same head everywhere"); };
        let (manifest, chunks) = encode_lane_input(lane, &body, 0).unwrap().into_parts();
        manifest.validate_availability().unwrap();
        assert_eq!(manifest.layout, lane.frozen().da_layout);
        assert_eq!(manifest.value.instance_id, Hash::from(lane.instance_id().0));
        assert_eq!(manifest.value.payload_hash, Hash::new(body.canonical_bytes()));
        assert_eq!(manifest.value.descriptor_hash, body.payload().descriptor.canonical_hash().unwrap());
        assert_eq!(manifest.byte_len as usize, body.canonical_bytes().len());
        assert_eq!(manifest.chunk_count as usize, chunks.len());
        assert_eq!(manifest.value.origin_view, 0);
        assert_eq!(lane.reducer_context().roster()[manifest.value.origin_producer as usize].id(), lane.reducer_context().leader(0));
        let data = usize::from(manifest.layout.data_shards);
        let width = data + usize::from(manifest.layout.parity_shards);
        assert!(chunks.len().is_multiple_of(width));
        assert!(chunks.iter().all(|chunk| chunk.len() == manifest.layout.chunk_size_bytes as usize));
        let recovered = chunks.chunks_exact(width).flat_map(|stripe| stripe[..data].iter())
            .flatten().copied().take(manifest.byte_len as usize).collect::<Vec<_>>();
        assert_eq!(recovered, body.canonical_bytes(), "the real encoded data shards contain the canonical body");
        let hashes = chunks.iter().map(Hash::new).collect::<Vec<_>>();
        assert_eq!(iroha_data_model::block::consensus_v2::payload_chunk_root(&hashes), Some(manifest.chunk_root));
        let mut corrupt = chunks.clone();
        corrupt[data][0] ^= 1;
        assert_ne!(iroha_data_model::block::consensus_v2::payload_chunk_root(&corrupt.iter().map(Hash::new).collect::<Vec<_>>()), Some(manifest.chunk_root),
            "parity chunks participate in the same availability commitment");
        let later_origin = encode_lane_input(lane, &body, 1).unwrap();
        assert_eq!(later_origin.manifest().value.payload_hash, manifest.value.payload_hash);
        assert_eq!(later_origin.manifest().chunk_root, manifest.chunk_root);
        assert_ne!(later_origin.manifest().value.subject_hash().unwrap(), manifest.value.subject_hash().unwrap());
        assert_eq!(verify_lane_input_manifest(lane, &body, &manifest, body.canonical_bytes()).unwrap().manifest(), &manifest,
            "immutable-origin verification preserves the original value; reducer lock/TC integration is separate");
        if let Some((first, first_chunks)) = &previous {
            let first: &iroha_data_model::block::lane_consensus::LaneManifestV1 = first;
            assert_ne!(first.value.instance_id, manifest.value.instance_id);
            assert_eq!(first.value.payload_hash, manifest.value.payload_hash);
            assert_eq!(first.value.descriptor_hash, manifest.value.descriptor_hash);
            assert_eq!(first.chunk_root, manifest.chunk_root);
            assert_eq!(first_chunks, &chunks);
        } else { previous = Some((manifest, chunks)); }
    }
}

state_test! { sync native_lane_input_rs16_refuses_recommitted_manifest_and_body_substitution
    use super::{FirstLaneAdmittedInputReadV1, LaneInputBodyPreparationV1};
    use crate::sumeragi::v2_lane_payload::{encode_lane_input, verify_lane_input_manifest};
    use iroha_data_model::block::lane_consensus::lane_availability_hash;
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&observed, lane).unwrap() else { panic!("source"); };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&observed, lane, &source).unwrap() else { panic!("body"); };
    let manifest = *encode_lane_input(lane, &body, 0).unwrap().manifest();
    for mutation in 0..9 {
        let mut changed = manifest;
        match mutation {
            0 => { changed.chunk_root = Hash::new(b"different nonzero codeword root"); },
            1 => { changed.byte_len -= 1; },
            2 => { changed.value.origin_producer = (changed.value.origin_producer + 1) % 4; },
            3 => { changed.value.descriptor_hash = Hash::new(b"different route slots"); },
            4 => { changed.value.kind = iroha_data_model::block::lane_consensus::LaneValueKindV1::Execution; },
            5 => { changed.value.instance_id = Hash::new(b"other instance"); },
            6 => { changed.value.payload_hash = Hash::new(b"another canonical payload"); },
            7 => { changed.value.admitted_binding_hash = Hash::new(b"another admitted group"); },
            8 => { changed.layout.max_payload_size_bytes -= 1; },
            _ => unreachable!(),
        }
        changed.chunk_count = iroha_data_model::block::consensus_v2::expected_encoded_chunk_count(changed.byte_len, changed.layout).unwrap();
        changed.value.availability_hash = lane_availability_hash(changed.layout, changed.chunk_root, changed.byte_len, changed.chunk_count).unwrap();
        changed.validate_availability().unwrap();
        assert!(verify_lane_input_manifest(lane, &body, &changed, body.canonical_bytes()).is_err(),
            "structurally valid recommitted manifest mutation {mutation} is not the actual native body");
    }
    let mut different_body = body.canonical_bytes().to_vec();
    different_body[0] ^= 1;
    assert!(verify_lane_input_manifest(lane, &body, &manifest, &different_body).is_err());
    let (foreign, _) = first_lane_input_fixture(0x89);
    let foreign_observed = foreign.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(encode_lane_input(&foreign_observed.contexts()[0], &body, 0).is_err(),
        "authentic but different frozen context cannot borrow this body");
}

state_test! { sync native_lane_input_rs16_is_unchanged_by_unrelated_global_height
    use super::{FirstLaneAdmittedInputReadV1, LaneInputBodyPreparationV1};
    use crate::sumeragi::v2_lane_payload::encode_lane_input;
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&observed, lane).unwrap() else { panic!("source"); };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&observed, lane, &source).unwrap() else { panic!("body"); };
    let original = encode_lane_input(lane, &body, 0).unwrap().into_parts();
    let parent = state.kura.v2_finality_artifact(fixture.block.header().height().get()).unwrap().unwrap();
    let opening = crate::sumeragi::v2_context::build_successor_height_context(&parent, parent.height_context.nexus_amx_context_hash, None).unwrap();
    let later = empty_global_block_after(Some(&fixture.block));
    let mut overlay = state.block(later.header());
    overlay.finalize_lane_consensus_contexts(&later, Some(&opening)).unwrap();
    let mut witness = ExecWitness::default();
    overlay.capture_lane_consensus_contexts(&mut witness).unwrap();
    overlay.block_hashes.push(later.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, &later);
    overlay.commit().unwrap();
    state.kura.store_block(Arc::new(later.clone())).unwrap();
    let (artifact, receipt) = stage_lane_context_fixture_finality(state, &later, opening, witness);
    state.kura.promote_kagemusha_finality_sidecar(&artifact, &receipt).unwrap();
    let current = state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(!observed.is_current(state));
    assert_eq!(lane.instance_id(), current.contexts()[0].instance_id());
    let FirstLaneAdmittedInputReadV1::Ready(current_source) = state.first_lane_admitted_input(&current, &current.contexts()[0]).unwrap() else { panic!("retained first source"); };
    let LaneInputBodyPreparationV1::Ready(current_body) = state.prepare_lane_input_body(&current, &current.contexts()[0], &current_source).unwrap() else { panic!("retained route heads"); };
    assert_eq!(current_body.canonical_bytes(), body.canonical_bytes());
    assert_eq!(encode_lane_input(&current.contexts()[0], &current_body, 0).unwrap().into_parts(), original);
    assert_eq!(current_source.carrier_hash(), fixture.block.hash());
}
