//! Signed cross-owner disk export and independent canonical replay controls.

use super::*;
use iroha_core::kura::BlockStore;
use norito::codec::Encode as _;
#[path = "../fixture.rs"]
mod fixture;

#[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
mod unix {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{MetadataExt as _, PermissionsExt as _},
        path::PathBuf,
    };

    struct Disk {
        _directory: tempfile::TempDir,
        root: PathBuf,
        log: PathBuf,
        signed: fixture::Fixture,
    }
    impl Disk {
        fn new(lanes: usize) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().canonicalize().unwrap();
            let signed = fixture::Fixture::new(lanes);
            let mut store = BlockStore::new(&root);
            store.create_files_if_they_do_not_exist().unwrap();
            store.append_block_to_chain(&signed.genesis).unwrap();
            store.append_block_to_chain(&signed.carrier).unwrap();
            drop(store);
            let log = root.join("merge.log");
            let entry = signed.entry.encode();
            let mut bytes = (entry.len() as u32).to_le_bytes().to_vec();
            bytes.extend_from_slice(&entry);
            fs::write(&log, bytes).unwrap();
            Self {
                _directory: directory,
                root,
                log,
                signed,
            }
        }
        fn supplied(&self) -> Vec<SuppliedHeightEvidence> {
            vec![
                SuppliedHeightEvidence {
                    height: 1,
                    finality: norito::encode_canonical(&self.signed.first).unwrap(),
                    queries: vec![],
                },
                SuppliedHeightEvidence {
                    height: 2,
                    finality: norito::encode_canonical(&self.signed.second).unwrap(),
                    queries: self.signed.queries(),
                },
            ]
        }
        fn bindings(&self) -> Vec<HeightInputBinding> {
            bindings(&self.supplied())
        }
        fn reader_limits(&self) -> CanonicalKuraEvidenceLimits {
            CanonicalKuraEvidenceLimits {
                first_height: 1,
                last_height: 2,
                max_committed_blocks: 8,
                max_store_data_bytes: 2 * 1024 * 1024,
                max_carrier_bytes: 1024 * 1024,
                max_merge_log_bytes: 2 * 1024 * 1024,
                max_merge_frames: 8,
                max_output_bytes: 2 * 1024 * 1024,
                max_decode_allocation_bytes: 8 * 1024 * 1024,
                owner_uid: fs::metadata(&self.root).unwrap().uid(),
            }
        }
        fn export(&self) -> Result<VerifiedExport> {
            export_from_kura(
                self.signed.plan(),
                fixture::limits(),
                &self.root,
                &self.log,
                self.reader_limits(),
                &self.bindings(),
                self.supplied(),
            )
        }
        fn snapshot(&self) -> Vec<(String, Vec<u8>, u32, u64, i64, i64)> {
            let mut files: Vec<_> = fs::read_dir(&self.root)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            files.sort();
            files
                .into_iter()
                .filter(|p| p.is_file())
                .map(|p| {
                    let m = fs::metadata(&p).unwrap();
                    (
                        p.file_name().unwrap().to_string_lossy().into_owned(),
                        fs::read(&p).unwrap(),
                        m.mode(),
                        m.ino(),
                        m.mtime(),
                        m.mtime_nsec(),
                    )
                })
                .collect()
        }
    }
    fn bindings(supplied: &[SuppliedHeightEvidence]) -> Vec<HeightInputBinding> {
        supplied
            .iter()
            .map(|s| HeightInputBinding {
                height: s.height,
                finality_hash: Hash::new(&s.finality),
                query_hashes: s.queries.iter().map(Hash::new).collect(),
            })
            .collect()
    }
    fn replay(disk: &Disk, export: &VerifiedExport) -> Result<VerifiedExport> {
        replay_export(
            disk.signed.plan(),
            fixture::limits(),
            &disk.bindings(),
            Hash::new(export.canonical_bytes()),
            export.canonical_bytes(),
        )
    }

    #[test]
    fn real_offline_store_one_and_four_lane_exports_replay_every_signed_effect() {
        for lanes in [1, 4] {
            let disk = Disk::new(lanes);
            for name in [
                "blocks.data",
                "blocks.index",
                "blocks.hashes",
                "blocks.count.norito",
                "merge.log",
            ] {
                fs::set_permissions(disk.root.join(name), fs::Permissions::from_mode(0o400))
                    .unwrap();
            }
            let before = disk.snapshot();
            let export = disk.export().unwrap();
            assert_eq!(export.rows().len(), 8);
            assert_eq!(
                export.rows().iter().map(|r| r.sequence).collect::<Vec<_>>(),
                vec![1, 2, 3, 4, 1, 2, 3, 4]
            );
            let replayed = replay(&disk, &export).unwrap();
            assert_eq!(replayed.canonical_bytes(), export.canonical_bytes());
            assert!(same_rows(replayed.rows(), export.rows()));
            assert_eq!(
                export
                    .rows()
                    .iter()
                    .map(|r| r.request.lane_id)
                    .collect::<BTreeSet<_>>()
                    .len(),
                lanes
            );
            assert_eq!(disk.snapshot(), before);
            let envelope: ExportEnvelopeV1 = canonical(export.canonical_bytes()).unwrap();
            assert_eq!(envelope.heights.len(), 2);
            assert_eq!(
                envelope
                    .heights
                    .iter()
                    .filter(|h| h.merge_entry.is_some())
                    .count(),
                1
            );
            assert_eq!(
                envelope.heights[1].merge_entry.as_ref().unwrap(),
                &disk.signed.entry.canonical_bytes()
            );
        }
    }

    #[test]
    fn strict_projection_has_exact_types_order_and_signed_hash_identity() {
        let disk = Disk::new(4);
        let export = disk.export().unwrap();
        let json = export.json_projection().unwrap();
        let rows: norito::json::Value = norito::json::from_slice(&json).unwrap();
        let rows = rows.as_array().unwrap();
        let names: BTreeSet<_> = [
            "logical_id",
            "phase",
            "sequence",
            "authority",
            "entrypoint_hash",
            "carrier_height",
            "carrier_hash",
            "merge_entry_hash",
            "merge_epoch",
            "leaf_index",
            "lane_id",
            "dataspace_id",
            "incarnation",
        ]
        .into_iter()
        .collect();
        assert_eq!(rows.len(), 8);
        for (index, (row, (_, tx, route, phase))) in
            rows.iter().zip(&disk.signed.requests).enumerate()
        {
            let object = row.as_object().unwrap();
            assert_eq!(
                object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
                names
            );
            assert_eq!(row["sequence"].as_u64(), Some((index % 4 + 1) as u64));
            assert_eq!(
                row["phase"].as_str(),
                Some(match phase {
                    WorkloadPhase::Warmup => "warmup",
                    WorkloadPhase::Measurement => "measurement",
                })
            );
            assert_eq!(
                row["authority"].as_str(),
                Some(tx.authority().canonical_i105().unwrap().as_str())
            );
            assert_eq!(
                row["entrypoint_hash"].as_str(),
                Some(tx.hash().to_string().as_str())
            );
            assert_eq!(row["carrier_height"].as_u64(), Some(2));
            assert_eq!(row["lane_id"].as_u64(), Some(route.lane_id.as_u32() as u64));
            assert_eq!(
                row["dataspace_id"].as_u64(),
                Some(route.dataspace_id.as_u64())
            );
        }
        assert!(
            json.len() + export.canonical_bytes().len() <= fixture::limits().output_bytes as usize
        );
    }

    #[test]
    fn independent_bindings_and_phase_admission_precede_any_path_open() {
        let disk = Disk::new(4);
        assert!(disk.export().is_ok());
        for mode in 0..8 {
            let mut supplied = disk.supplied();
            let mut bindings = disk.bindings();
            let mut plan = disk.signed.plan();
            let mut reader = disk.reader_limits();
            let expected = match mode {
                0 => {
                    bindings.pop();
                    "input binding interval mismatch"
                }
                1 => {
                    bindings[1].height = 1;
                    "input binding height order"
                }
                2 => {
                    bindings[1].finality_hash = Hash::new(b"different");
                    "finality input digest mismatch"
                }
                3 => {
                    bindings[1].query_hashes[0] = Hash::new(b"different");
                    "query input digest mismatch"
                }
                4 => {
                    supplied.swap(0, 1);
                    "supplied input shape differs from binding"
                }
                5 => {
                    plan.scheduled[7].phase = WorkloadPhase::Warmup;
                    "warmup follows measurement"
                }
                6 => {
                    for s in &mut plan.scheduled {
                        s.phase = WorkloadPhase::Warmup;
                    }
                    "missing measurement schedule"
                }
                _ => {
                    reader.last_height = 1;
                    "reader and launch intervals differ"
                }
            };
            let error = export_from_kura(
                plan,
                fixture::limits(),
                Path::new("/absent-canonical-fixture"),
                &disk.log,
                reader,
                &bindings,
                supplied,
            )
            .err()
            .unwrap();
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn full_scan_rejects_a_late_tail_after_the_requested_real_merge_entry() {
        let disk = Disk::new(4);
        assert!(disk.export().is_ok());
        let mut bytes = fs::read(&disk.log).unwrap();
        bytes.push(0x5a);
        fs::write(&disk.log, bytes).unwrap();
        let before = disk.snapshot();
        assert!(disk.export().is_err());
        assert_eq!(disk.snapshot(), before);
    }

    #[test]
    fn late_authentication_failure_has_no_artifact_or_disk_mutation() {
        let disk = Disk::new(4);
        assert!(disk.export().is_ok());
        let mut supplied = disk.supplied();
        let mut second = disk.signed.second.clone();
        second.finality_artifact.commit_qc.aggregate_signature.pop();
        supplied[1].finality = norito::encode_canonical(&second).unwrap();
        // Independently supplied digest is deliberately updated: this control
        // reaches real authentication rather than the earlier digest gate.
        let exact = bindings(&supplied);
        let before = disk.snapshot();
        let result = export_from_kura(
            disk.signed.plan(),
            fixture::limits(),
            &disk.root,
            &disk.log,
            disk.reader_limits(),
            &exact,
            supplied,
        );
        assert!(result.is_err());
        assert_eq!(disk.snapshot(), before);
    }

    #[test]
    fn final_disk_identity_change_after_authentication_prevents_artifact() {
        let disk = Disk::new(4);
        assert!(disk.export().is_ok());
        let called = std::cell::Cell::new(false);
        let result = export_with_finish_hook(
            disk.signed.plan(),
            fixture::limits(),
            &disk.root,
            &disk.log,
            disk.reader_limits(),
            &disk.bindings(),
            disk.supplied(),
            || {
                called.set(true);
                let bytes = fs::read(&disk.log).unwrap();
                let replacement = disk.root.join("replacement.log");
                fs::write(&replacement, bytes).unwrap();
                fs::rename(replacement, &disk.log).unwrap();
            },
        );
        assert!(called.get());
        assert!(result.is_err());
    }

    #[test]
    fn framing_digest_and_version_have_no_legacy_or_unsigned_fallback() {
        let disk = Disk::new(4);
        let export = disk.export().unwrap();
        assert!(replay(&disk, &export).is_ok());
        let envelope: ExportEnvelopeV1 = canonical(export.canonical_bytes()).unwrap();
        let mut wrong_version: ExportEnvelopeV1 = canonical(export.canonical_bytes()).unwrap();
        wrong_version.version = 2;
        let mut trailing = export.canonical_bytes().to_vec();
        trailing.push(0);
        for bytes in [
            envelope.encode(),
            norito::encode_canonical(&wrong_version).unwrap(),
            trailing,
            b"{\"verified\":true}".to_vec(),
        ] {
            assert!(
                replay_export(
                    disk.signed.plan(),
                    fixture::limits(),
                    &disk.bindings(),
                    Hash::new(&bytes),
                    &bytes
                )
                .is_err()
            );
        }
        assert!(
            replay_export(
                disk.signed.plan(),
                fixture::limits(),
                &disk.bindings(),
                Hash::new(b"stale digest"),
                export.canonical_bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn every_projected_identity_is_recomputed_even_when_outer_digest_is_updated() {
        let disk = Disk::new(4);
        let export = disk.export().unwrap();
        assert!(replay(&disk, &export).is_ok());
        for mode in 0..14 {
            let mut e: ExportEnvelopeV1 = canonical(export.canonical_bytes()).unwrap();
            let row = &mut e.rows[0];
            match mode {
                0 => row.sequence += 1,
                1 => row.request.logical_id = format!("{:064x}", 99),
                2 => row.request.phase = WorkloadPhase::Measurement,
                3 => row.request.authority = disk.signed.requests[1].1.authority().clone(),
                4 => row.request.entrypoint_hash = disk.signed.requests[1].1.hash_as_entrypoint(),
                5 => row.request.carrier_height += 1,
                6 => row.request.carrier_hash = disk.signed.genesis.hash(),
                7 => {
                    row.request.merge_entry_hash =
                        HashOf::from_untyped_unchecked(Hash::new(b"changed entry"))
                }
                8 => row.request.merge_epoch += 1,
                9 => row.request.leaf_index += 1,
                10 => row.request.lane_id = LaneId::new(99),
                11 => row.request.dataspace_id = DataSpaceId::new(99),
                12 => row.request.incarnation = Hash::new(b"different incarnation"),
                _ => e.rows.swap(0, 1),
            }
            let bytes = norito::encode_canonical(&e).unwrap();
            let error = replay_export(
                disk.signed.plan(),
                fixture::limits(),
                &disk.bindings(),
                Hash::new(&bytes),
                &bytes,
            )
            .err()
            .unwrap();
            assert_eq!(
                error.to_string(),
                "export rows differ from independent authentication"
            );
        }
    }

    #[test]
    fn retained_proof_changes_cannot_be_disguised_by_rehashed_rows_or_artifact() {
        let disk = Disk::new(4);
        let export = disk.export().unwrap();
        assert!(replay(&disk, &export).is_ok());
        for mode in 0..5 {
            let mut e: ExportEnvelopeV1 = canonical(export.canonical_bytes()).unwrap();
            match mode {
                0 => e.heights.swap(0, 1),
                1 => {
                    e.heights.pop();
                }
                2 => e.heights[1].merge_entry = None,
                3 => e.heights[1].queries.swap(0, 1),
                _ => e.heights[1].carrier = disk.signed.genesis.encode_wire().unwrap(),
            }
            let bytes = norito::encode_canonical(&e).unwrap();
            assert!(
                replay_export(
                    disk.signed.plan(),
                    fixture::limits(),
                    &disk.bindings(),
                    Hash::new(&bytes),
                    &bytes
                )
                .is_err()
            );
        }
    }

    #[test]
    fn independent_plan_cannot_be_replaced_by_proof_declared_authority_or_route() {
        let disk = Disk::new(4);
        let export = disk.export().unwrap();
        assert!(replay(&disk, &export).is_ok());
        for mode in 0..4 {
            let mut plan = disk.signed.plan();
            match mode {
                0 => plan.first_context = disk.signed.second.finality_artifact.context_id(),
                1 => plan.scheduled[0].route.lane_id = LaneId::new(1),
                2 => {
                    plan.scheduled[0].signed_transaction =
                        plan.scheduled[1].signed_transaction.clone()
                }
                _ => {
                    plan.scheduled.pop();
                }
            }
            assert!(
                replay_export(
                    plan,
                    fixture::limits(),
                    &disk.bindings(),
                    Hash::new(export.canonical_bytes()),
                    export.canonical_bytes()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn exact_serialized_output_plus_projection_boundary_and_one_byte_short() {
        let disk = Disk::new(4);
        let initial = disk.export().unwrap();
        let projection = initial.rows().len() as u64 * (ROW_RESERVATION + 1) + 2;
        let mut limits = fixture::limits();
        limits.output_bytes = initial.canonical_bytes().len() as u64 + projection;
        let exact = export_from_kura(
            disk.signed.plan(),
            limits,
            &disk.root,
            &disk.log,
            disk.reader_limits(),
            &disk.bindings(),
            disk.supplied(),
        )
        .unwrap();
        assert_eq!(exact.canonical_bytes(), initial.canonical_bytes());
        assert!(exact.json_projection().is_ok());
        limits.output_bytes -= 1;
        let error = export_from_kura(
            disk.signed.plan(),
            limits,
            &disk.root,
            &disk.log,
            disk.reader_limits(),
            &disk.bindings(),
            disk.supplied(),
        )
        .err()
        .unwrap();
        assert_eq!(
            error.to_string(),
            "replayable export and projection exceed output reservation"
        );
        assert!(
            replay_export(
                disk.signed.plan(),
                limits,
                &disk.bindings(),
                Hash::new(initial.canonical_bytes()),
                initial.canonical_bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn scope_and_byte_reservations_reject_before_decoder_or_reader_allocation() {
        let disk = Disk::new(4);
        assert!(disk.export().is_ok());
        for mode in 0..4 {
            let mut limits = fixture::limits();
            let mut reader = disk.reader_limits();
            match mode {
                0 => reader.max_output_bytes = limits.input_bytes,
                1 => reader.max_store_data_bytes = limits.input_bytes + 1,
                2 => limits.admitted_proof_bytes = MAX_PROOF_BYTES + 1,
                _ => limits.input_bytes = u64::MAX,
            }
            let error = export_from_kura(
                disk.signed.plan(),
                limits,
                Path::new("/absent-canonical-fixture"),
                &disk.log,
                reader,
                &disk.bindings(),
                disk.supplied(),
            )
            .err()
            .unwrap();
            assert!(!error.to_string().contains("I/O"));
        }
        let exported = disk.export().unwrap();
        let plan_bytes: usize = disk
            .signed
            .plan()
            .scheduled
            .iter()
            .map(|s| s.signed_transaction.len())
            .sum();
        let mut exact = fixture::limits();
        exact.input_bytes = (plan_bytes + exported.canonical_bytes().len()) as u64;
        assert!(
            replay_export(
                disk.signed.plan(),
                exact,
                &disk.bindings(),
                Hash::new(exported.canonical_bytes()),
                exported.canonical_bytes()
            )
            .is_ok()
        );
        exact.input_bytes -= 1;
        assert_eq!(
            replay_export(
                disk.signed.plan(),
                exact,
                &disk.bindings(),
                Hash::new(exported.canonical_bytes()),
                exported.canonical_bytes()
            )
            .err()
            .unwrap()
            .to_string(),
            "proof input allocation exceeded"
        );
        let mut limits = fixture::limits();
        limits.output_bytes = 1;
        assert_eq!(
            replay_export(
                disk.signed.plan(),
                limits,
                &disk.bindings(),
                Hash::new(b"xx"),
                b"xx"
            )
            .err()
            .unwrap()
            .to_string(),
            "export frame exceeds admitted allocation"
        );
    }
}

#[test]
fn export_row_declares_its_own_v1_canonical_frame() {
    let fixture = fixture::Fixture::new(1);
    let mut verifier = fixture.start(fixture.plan(), fixture::limits());
    fixture.push(&mut verifier).unwrap();
    let complete = verifier.finish().unwrap();
    let row = ExportRowV1 {
        sequence: 1,
        request: complete.rows()[0].clone(),
    };
    let decoded =
        super::super::tests::assert_declared_scaling_frame::<ExportRowV1, AuthenticatedRequest>(
            &row,
            "iroha_kagami::scaling_evidence::ExportRowV1",
            [
                119, 237, 171, 184, 92, 224, 31, 16, 240, 225, 81, 255, 174, 155, 183, 92,
            ],
        );
    assert!(same_rows(&[row], &[decoded]));
}

#[test]
fn export_envelope_declares_v1_identity_for_complete_nested_proofs() {
    let fixture = fixture::Fixture::new(1);
    let mut verifier = fixture.start(fixture.plan(), fixture::limits());
    fixture.push(&mut verifier).unwrap();
    let complete = verifier.finish().unwrap();
    let envelope = ExportEnvelopeV1 {
        version: 1,
        heights: vec![
            HeightProofV1 {
                height: 1,
                finality: norito::encode_canonical(&fixture.first).unwrap(),
                carrier: fixture.genesis.encode_wire().unwrap(),
                merge_entry: None,
                queries: vec![],
            },
            HeightProofV1 {
                height: 2,
                finality: norito::encode_canonical(&fixture.second).unwrap(),
                carrier: fixture.carrier.encode_wire().unwrap(),
                merge_entry: Some(fixture.entry.canonical_bytes()),
                queries: fixture.queries(),
            },
        ],
        rows: schedule_rows(complete.rows().to_vec()),
    };
    let decoded = super::super::tests::assert_declared_scaling_frame::<
        ExportEnvelopeV1,
        AuthenticatedRequest,
    >(
        &envelope,
        "iroha_kagami::scaling_evidence::ExportEnvelopeV1",
        [
            200, 163, 7, 133, 220, 31, 46, 92, 23, 246, 247, 217, 101, 154, 62, 182,
        ],
    );
    assert_eq!(decoded.version, 1);
    assert_eq!(decoded.heights.len(), 2);
    assert!(decoded.heights[0].queries.is_empty());
    assert_eq!(decoded.heights[1].queries.len(), 8);
    assert!(same_rows(&envelope.rows, &decoded.rows));

    // Bind the replay to independent fixture inputs, not the decoded envelope.
    let bindings = [
        HeightInputBinding {
            height: 1,
            finality_hash: Hash::new(norito::encode_canonical(&fixture.first).unwrap()),
            query_hashes: vec![],
        },
        HeightInputBinding {
            height: 2,
            finality_hash: Hash::new(norito::encode_canonical(&fixture.second).unwrap()),
            query_hashes: fixture.queries().iter().map(Hash::new).collect(),
        },
    ];
    let bytes = norito::encode_canonical(&decoded).unwrap();
    let replayed = replay_export(
        fixture.plan(),
        fixture::limits(),
        &bindings,
        Hash::new(&bytes),
        &bytes,
    )
    .unwrap();
    assert!(same_rows(&decoded.rows, replayed.rows()));
    assert_eq!(replayed.canonical_bytes(), bytes);

    // A valid outer frame cannot authorize an unframed nested merge entry.
    let mut bare_merge: ExportEnvelopeV1 = norito::decode_canonical(&bytes).unwrap();
    bare_merge.heights[1].merge_entry = Some(fixture.entry.encode());
    let bare_bytes = norito::encode_canonical(&bare_merge).unwrap();
    assert_ne!(bare_bytes, bytes);
    assert!(
        replay_export(
            fixture.plan(),
            fixture::limits(),
            &bindings,
            Hash::new(&bare_bytes),
            &bare_bytes,
        )
        .is_err()
    );
}
