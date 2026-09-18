// Actual State/Kura finality, all-route input, RS16 and physical WAL sibling.

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_body_store_reopens_fsynced_body_and_preserves_every_origin
    use crate::sumeragi::{v2_lane_payload::encode_lane_input, v2_lane_wal::LaneSafetyWal};
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let current = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &current.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&current, lane).unwrap() else { panic!("exact first source"); };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&current, lane, &source).unwrap() else { panic!("all routes own the same group"); };
    let origin0 = *encode_lane_input(lane, &body, 0).unwrap().manifest();
    let origin1 = *encode_lane_input(lane, &body, 1).unwrap().manifest();
    assert_ne!(origin0.value.subject_hash().unwrap(), origin1.value.subject_hash().unwrap());
    let wal = LaneSafetyWal::open(&state.kura, lane, 0).unwrap();
    let mut store = wal.open_body_store().unwrap();
    assert!(wal.open_body_store().is_err(), "one actual WAL mints only one sibling owner");
    assert!(store.read_for_manifest(&origin0).unwrap().is_none());
    let read0 = store.persist(&body, &origin0).unwrap();
    assert_eq!(read0.canonical_bytes(), body.canonical_bytes());
    assert_eq!(read0.payload(), body.payload());
    assert_eq!(read0.receipt().manifest(), &origin0);
    store.validate_receipt(read0.receipt()).unwrap();
    let path = store.path_for_test().to_path_buf();
    let frame0 = std::fs::read(&path).unwrap();
    assert!(frame0.len() as u64 <= store.maximum_frame_bytes_for_test());
    let read1 = store.persist(&body, &origin1).unwrap();
    assert_eq!(read1.receipt().manifest(), &origin1);
    assert_eq!(std::fs::read(&path).unwrap(), frame0, "new origins never overwrite the immutable body/first origin");
    store.validate_receipt(read0.receipt()).unwrap();
    store.validate_receipt(read1.receipt()).unwrap();
    // Drop every physical owner without delivering any reducer completion. The
    // returned receipt is kept only to test that it cannot authorize a new owner.
    drop(read1); drop(store); drop(wal);
    let reopened_wal = LaneSafetyWal::open(&state.kura, lane, 0).unwrap();
    let reopened = reopened_wal.open_body_store().unwrap();
    let recovered0 = reopened.read_for_manifest(&origin0).unwrap().unwrap();
    let recovered1 = reopened.read_for_manifest(&origin1).unwrap().unwrap();
    assert_eq!(recovered0.canonical_bytes(), body.canonical_bytes());
    assert_eq!(recovered1.canonical_bytes(), body.canonical_bytes());
    reopened.validate_receipt(recovered0.receipt()).unwrap();
    reopened.validate_receipt(recovered1.receipt()).unwrap();
    assert!(reopened.validate_receipt(read0.receipt()).is_err(), "a lost-ack receipt cannot alias a reopened physical owner");
    assert_eq!(std::fs::read(&path).unwrap(), frame0);
    let mut changed = origin0;
    changed.value.origin_producer = (changed.value.origin_producer + 1) % 4;
    assert!(reopened.read_for_manifest(&changed).is_err());
    changed = origin0; changed.value.payload_hash = Hash::new(b"foreign immutable input");
    assert!(reopened.read_for_manifest(&changed).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), frame0, "adverse readback never mutates custody");
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_body_store_rejects_corruption_foreign_instance_key_and_oversize
    use crate::sumeragi::{v2_lane_payload::encode_lane_input, v2_lane_wal::LaneSafetyWal};
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let current = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane0 = &current.contexts()[0];
    let lane1 = &current.contexts()[1];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&current, lane0).unwrap() else { panic!("source"); };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&current, lane0, &source).unwrap() else { panic!("body"); };
    let manifest0 = *encode_lane_input(lane0, &body, 0).unwrap().manifest();
    let manifest1 = *encode_lane_input(lane1, &body, 0).unwrap().manifest();
    let wal0 = LaneSafetyWal::open(&state.kura, lane0, 0).unwrap();
    let mut store0 = wal0.open_body_store().unwrap();
    let read0 = store0.persist(&body, &manifest0).unwrap();
    let path0 = store0.path_for_test().to_path_buf();
    let exact = std::fs::read(&path0).unwrap();
    let maximum = store0.maximum_frame_bytes_for_test();
    for (foreign_lane, signer, manifest) in [(lane1, 0, &manifest1), (lane0, 1, &manifest0)] {
        let wal = LaneSafetyWal::open(&state.kura, foreign_lane, signer).unwrap();
        let mut store = wal.open_body_store().unwrap();
        store.persist(&body, manifest).unwrap();
        let foreign_path = store.path_for_test().to_path_buf();
        drop(store); drop(wal);
        std::fs::write(&foreign_path, &exact).unwrap();
        let foreign_wal = LaneSafetyWal::open(&state.kura, foreign_lane, signer).unwrap();
        assert!(foreign_wal.open_body_store().is_err(), "same canonical input cannot substitute another instance/key's physical frame");
        assert_eq!(std::fs::read(&foreign_path).unwrap(), exact, "reopen cannot repair foreign storage implicitly");
    }
    let mut corrupt = exact.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    std::fs::write(&path0, &corrupt).unwrap();
    assert!(store0.read_for_manifest(&manifest0).is_err());
    assert!(store0.validate_receipt(read0.receipt()).is_err());
    drop(store0); drop(wal0);
    let corrupt_wal = LaneSafetyWal::open(&state.kura, lane0, 0).unwrap();
    assert!(corrupt_wal.open_body_store().is_err());
    drop(corrupt_wal);
    assert_eq!(std::fs::read(&path0).unwrap(), corrupt);
    // Sparse physical oversize fails at the descriptor stat bound before decode.
    let file = std::fs::OpenOptions::new().write(true).open(&path0).unwrap();
    file.set_len(maximum + 1).unwrap(); file.sync_all().unwrap(); drop(file);
    let oversized_wal = LaneSafetyWal::open(&state.kura, lane0, 0).unwrap();
    assert!(oversized_wal.open_body_store().is_err());
    drop(oversized_wal);
    std::fs::write(&path0, &exact).unwrap();
    let restored_wal = LaneSafetyWal::open(&state.kura, lane0, 0).unwrap();
    let restored = restored_wal.open_body_store().unwrap();
    let receipt = restored.read_for_manifest(&manifest0).unwrap().unwrap();
    std::fs::remove_file(&path0).unwrap();
    assert!(restored.read_for_manifest(&manifest0).unwrap().is_none(), "missing storage is explicit, not corruption");
    assert!(restored.validate_receipt(receipt.receipt()).is_err(), "an acknowledged exact body cannot be reported present after removal");
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_body_store_refuses_rebound_directory_without_losing_source
    use crate::sumeragi::{v2_lane_payload::encode_lane_input, v2_lane_wal::LaneSafetyWal};
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let current = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &current.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&current, lane).unwrap() else { panic!("source"); };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&current, lane, &source).unwrap() else { panic!("body"); };
    let manifest = *encode_lane_input(lane, &body, 0).unwrap().manifest();
    let wal = LaneSafetyWal::open(&state.kura, lane, 0).unwrap();
    let mut store = wal.open_body_store().unwrap();
    let path = store.path_for_test().to_path_buf();
    let directory = path.parent().unwrap();
    let moved = directory.with_extension("retained-native-test");
    std::fs::rename(directory, &moved).unwrap();
    std::fs::create_dir(directory).unwrap();
    assert!(store.persist(&body, &manifest).is_err());
    assert!(!path.exists(), "a rebound pathname cannot receive the native body");
    assert_eq!(source.canonical_control_bytes(), body.source().canonical_control_bytes());
    std::fs::remove_dir(directory).unwrap();
    std::fs::rename(&moved, directory).unwrap();
    drop(store); drop(wal);
    let reopened_wal = LaneSafetyWal::open(&state.kura, lane, 0).unwrap();
    let mut reopened = reopened_wal.open_body_store().unwrap();
    let read = reopened.persist(&body, &manifest).unwrap();
    reopened.validate_receipt(read.receipt()).unwrap();
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_body_store_publication_error_requires_reopen_and_preserves_input
    use crate::sumeragi::{v2_lane_payload::encode_lane_input, v2_lane_wal::LaneSafetyWal};
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let current = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &current.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&current, lane).unwrap() else { panic!("source"); };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&current, lane, &source).unwrap() else { panic!("body"); };
    let manifest = *encode_lane_input(lane, &body, 0).unwrap().manifest();
    let wal = LaneSafetyWal::open(&state.kura, lane, 0).unwrap();
    let mut store = wal.open_body_store().unwrap();
    let path = store.path_for_test().to_path_buf();
    let mut name = path.file_name().unwrap().to_os_string(); name.push(".tmp");
    let temporary = path.with_file_name(name);
    // A non-file temporary is an actual guarded publication failure. It cannot
    // be silently removed or make the wrapper retry a possibly uncertain write.
    std::fs::create_dir(&temporary).unwrap();
    assert!(store.persist(&body, &manifest).is_err());
    assert!(temporary.is_dir());
    assert!(!path.exists());
    std::fs::remove_dir(&temporary).unwrap();
    assert!(store.persist(&body, &manifest).unwrap_err().to_string().contains("requires physical reopen"));
    assert!(store.read_for_manifest(&manifest).is_err());
    assert_eq!(source.canonical_control_bytes(), body.source().canonical_control_bytes());
    drop(store); drop(wal);
    let reopened_wal = LaneSafetyWal::open(&state.kura, lane, 0).unwrap();
    let mut reopened = reopened_wal.open_body_store().unwrap();
    let read = reopened.persist(&body, &manifest).unwrap();
    reopened.validate_receipt(read.receipt()).unwrap();
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_body_store_postpublication_readback_failure_stays_fenced
    use crate::sumeragi::{v2_lane_payload::encode_lane_input, v2_lane_wal::LaneSafetyWal};
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let current = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &current.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&current, lane).unwrap() else { panic!("source"); };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&current, lane, &source).unwrap() else { panic!("body"); };
    let manifest = *encode_lane_input(lane, &body, 0).unwrap().manifest();
    for signer in 0..2 {
        let wal = LaneSafetyWal::open(&state.kura, lane, signer).unwrap();
        let mut store = wal.open_body_store().unwrap();
        store.publish_before_readback_for_test(&body, &manifest).unwrap();
        let path = store.path_for_test().to_path_buf();
        let exact = std::fs::read(&path).unwrap();
        assert!(store.read_for_manifest(&manifest).is_err(), "publication remains fenced until exact readback");
        if signer == 0 {
            std::fs::remove_file(&path).unwrap();
        } else {
            let mut corrupt = exact.clone();
            *corrupt.last_mut().unwrap() ^= 1;
            std::fs::write(&path, corrupt).unwrap();
        }
        assert!(store.finish_readback_for_test(&body, &manifest).is_err(), "both absent and corrupt post-write readback fail");
        std::fs::write(&path, &exact).unwrap();
        assert!(store.persist(&body, &manifest).unwrap_err().to_string().contains("requires physical reopen"));
        assert!(store.read_for_manifest(&manifest).is_err());
        drop(store); drop(wal);
        let reopened_wal = LaneSafetyWal::open(&state.kura, lane, signer).unwrap();
        let mut reopened = reopened_wal.open_body_store().unwrap();
        let read = reopened.persist(&body, &manifest).unwrap();
        reopened.validate_receipt(read.receipt()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), exact);
        // A read-only wrong-origin request did not begin a publication and
        // must not poison the valid existing owner.
        let mut wrong = manifest; wrong.value.payload_hash = Hash::new(b"wrong read-only origin");
        assert!(reopened.read_for_manifest(&wrong).is_err());
        reopened.validate_receipt(read.receipt()).unwrap();
    }
}
