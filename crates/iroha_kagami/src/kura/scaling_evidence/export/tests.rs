//! Signed cross-owner disk export and independent canonical replay controls.

use super::*;
use crate::kura::scaling_evidence::fixture;
use iroha_core::kura::BlockStore;
use norito::codec::Encode as _;

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
        signed: fixture::Fixture,
    }
    impl Disk {
        fn new(lanes: usize) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().canonicalize().unwrap();
            let signed = fixture::Fixture::new(lanes);
            let mut store = BlockStore::new(&root);
            store.create_files_if_they_do_not_exist().unwrap();
            for height in &signed.heights {
                store.append_block_to_chain(&height.block).unwrap();
            }
            drop(store);

            Self {
                _directory: directory,
                root,

                signed,
            }
        }
        fn supplied(&self) -> Vec<SuppliedHeightEvidence> {
            self.signed
                .heights
                .iter()
                .map(|height| SuppliedHeightEvidence {
                    height: height.block.header().height().get(),
                    carrier: height.block.encode_wire().unwrap(),
                    lane_evidence: height.evidence.clone(),
                    queries: height.queries(),
                })
                .collect()
        }
        fn bindings(&self) -> Vec<HeightInputBinding> {
            bindings(&self.supplied())
        }
        fn reader_limits(&self) -> CanonicalKuraEvidenceLimits {
            CanonicalKuraEvidenceLimits {
                first_height: 1,
                last_height: self.signed.heights.len() as u64,
                max_committed_blocks: 1025,
                max_store_data_bytes: 2 * 1024 * 1024,
                max_carrier_bytes: 1024 * 1024,
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
                carrier_hash: Hash::new(&s.carrier),
                lane_evidence_hash: Hash::new(&s.lane_evidence),
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
    fn sdk_fixture_retains_the_entire_verified_native_artifact_and_all_request_rows() {
        for lanes in [1, 4] {
            let disk = Disk::new(lanes);
            let original = disk.snapshot();
            let export = disk.export().unwrap();
            let replayed = replay(&disk, &export).unwrap();
            let encoded = export.sdk_fixture_json(16 * 1024 * 1024).unwrap();
            assert_eq!(
                encoded,
                replayed.sdk_fixture_json(16 * 1024 * 1024).unwrap()
            );
            let document: norito::json::Value = norito::json::from_slice(&encoded).unwrap();
            assert_eq!(
                document
                    .get("version")
                    .and_then(norito::json::Value::as_u64),
                Some(1)
            );
            assert_eq!(
                document
                    .get("artifact_schema")
                    .and_then(norito::json::Value::as_str),
                Some("iroha_kagami::scaling_evidence::ExportEnvelopeV1")
            );
            let proof = hex::decode(
                document
                    .get("canonical_artifact_hex")
                    .and_then(norito::json::Value::as_str)
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(proof, export.canonical_bytes());
            let digest: Hash =
                norito::json::from_value(document.get("artifact_hash").unwrap().clone()).unwrap();
            assert_eq!(digest, Hash::new(&proof));
            let rows: norito::json::Value =
                norito::json::from_slice(&export.json_projection(1024 * 1024).unwrap()).unwrap();
            assert_eq!(document.get("requests"), Some(&rows));
            assert_eq!(
                rows.as_array().unwrap().len(),
                8,
                "all warmup and measurement requests remain"
            );
            assert_eq!(
                export.sdk_fixture_json(encoded.len() as u64).unwrap(),
                encoded
            );
            for cap in [0, 1, encoded.len() as u64 - 1, MAX_PROOF_BYTES + 1] {
                assert!(export.sdk_fixture_json(cap).is_err(), "reject bound {cap}");
            }
            assert_eq!(disk.snapshot(), original, "capture is read-only");
        }
    }

    #[test]
    fn native_execution_sdk_captures_match_the_complete_replayed_artifact() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sumeragi");
        for lanes in [1, 4] {
            let disk = Disk::new(lanes);
            let exported = disk.export().unwrap();
            let replayed = replay(&disk, &exported).unwrap();
            let bytes = replayed.sdk_fixture_json(16 * 1024 * 1024).unwrap();
            let path = root.join(format!("native_execution_evidence_{lanes}_lanes_v1.json"));
            if std::env::var_os("IROHA_UPDATE_NATIVE_EXECUTION_FIXTURES").as_deref()
                == Some(std::ffi::OsStr::new("1"))
            {
                fs::create_dir_all(&root).unwrap();
                fs::write(&path, &bytes).unwrap();
            }
            assert_eq!(
                fs::read(&path)
                    .expect("generate native captures with the explicit fixture update test"),
                bytes
            );
        }
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
            assert_eq!(envelope.heights.len(), disk.signed.heights.len());
            for (retained, actual) in envelope.heights.iter().zip(&disk.signed.heights) {
                assert_eq!(retained.lane_evidence, actual.evidence);
                assert_eq!(retained.carrier, actual.block.encode_wire().unwrap());
            }
            assert_eq!(
                envelope
                    .heights
                    .iter()
                    .map(|height| height.queries.len())
                    .sum::<usize>(),
                8
            );
        }
    }

    #[test]
    fn strict_projection_has_exact_types_order_and_signed_hash_identity() {
        let disk = Disk::new(4);
        let export = disk.export().unwrap();
        let json = export.json_projection(MAX_PROOF_BYTES).unwrap();
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
            "lane_source",
            "leaf_index",
            "lane_id",
            "dataspace_id",
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
            assert_eq!(
                row["carrier_height"].as_u64(),
                Some(export.rows()[index].request.carrier_height)
            );
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
                    bindings[1].carrier_hash = Hash::new(b"different");
                    "carrier/context input digest mismatch"
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
    fn canonical_hash_journal_rejects_a_late_tail_without_mutation() {
        let disk = Disk::new(4);
        assert!(disk.export().is_ok());
        let mut bytes = fs::read(disk.root.join("blocks.hashes")).unwrap();
        bytes.push(0x5a);
        fs::write(disk.root.join("blocks.hashes"), bytes).unwrap();
        let before = disk.snapshot();
        assert!(disk.export().is_err());
        assert_eq!(disk.snapshot(), before);
    }

    #[test]
    fn late_authentication_failure_has_no_artifact_or_disk_mutation() {
        let disk = Disk::new(4);
        assert!(disk.export().is_ok());
        let export = disk.export().unwrap();
        let mut envelope: ExportEnvelopeV1 = canonical(export.canonical_bytes()).unwrap();
        let mut second = disk.signed.heights[1].block.clone();
        let certificate = second.commit_certificate().unwrap();
        let mut qc = certificate.commit_qc().to_vec();
        qc.pop();
        let changed = iroha_data_model::block::CommitCertificate::from_untrusted_parts(
            certificate.consensus_header().to_vec(),
            qc,
            certificate.result_preimage().to_vec(),
        );
        second.set_commit_certificate(Some(changed));
        let mut supplied = disk.supplied();
        supplied[1].carrier = second.encode_wire().unwrap();
        envelope.heights[1].carrier = supplied[1].carrier.clone();
        // Coherently update every transport digest so rejection reaches native
        // certificate verification, not an earlier hash or disk-identity gate.
        let exact = bindings(&supplied);
        let bytes = norito::encode_canonical(&envelope).unwrap();
        let before = disk.snapshot();
        let result = replay_export(
            disk.signed.plan(),
            fixture::limits(),
            &exact,
            Hash::new(&bytes),
            &bytes,
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
            disk.reader_limits(),
            &disk.bindings(),
            disk.supplied(),
            || {
                called.set(true);
                let bytes = fs::read(disk.root.join("blocks.hashes")).unwrap();
                let replacement = disk.root.join("replacement.hashes");
                fs::write(&replacement, bytes).unwrap();
                fs::rename(replacement, disk.root.join("blocks.hashes")).unwrap();
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
        for mode in 0..15 {
            let mut e: ExportEnvelopeV1 = canonical(export.canonical_bytes()).unwrap();
            let row = &mut e.rows[1];
            match mode {
                0 => row.sequence += 1,
                1 => row.request.logical_id = format!("{:064x}", 99),
                2 => row.request.phase = WorkloadPhase::Measurement,
                3 => row.request.authority = disk.signed.requests[0].1.authority().clone(),
                4 => row.request.entrypoint_hash = disk.signed.requests[0].1.hash_as_entrypoint(),
                5 => row.request.carrier_height += 1,
                6 => row.request.carrier_hash = disk.signed.heights[0].block.hash(),
                7 => {
                    row.request.lane_source.as_mut().unwrap().anchor_hash =
                        HashOf::from_untyped_unchecked(Hash::new(b"changed entry"))
                }
                8 => {
                    row.request.lane_source.as_mut().unwrap().block_hash =
                        Hash::new(b"different source block").into()
                }
                13 => {
                    row.request.lane_source.as_mut().unwrap().instance =
                        Hash::new(b"different instance").into()
                }
                9 => row.request.leaf_index += 1,
                10 => row.request.lane_id = LaneId::new(99),
                11 => row.request.dataspace_id = DataSpaceId::new(99),
                12 => {
                    row.request.lane_source.as_mut().unwrap().incarnation =
                        Hash::new(b"different incarnation").into()
                }
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
                2 => e.heights[1].lane_evidence.clear(),
                3 => e.heights[3].queries.swap(0, 1),
                _ => e.heights[1].carrier = disk.signed.heights[0].block.encode_wire().unwrap(),
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
                0 => plan.genesis_epoch_context_id[0] ^= 1,
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
            disk.reader_limits(),
            &disk.bindings(),
            disk.supplied(),
        )
        .unwrap();
        assert_eq!(exact.canonical_bytes(), initial.canonical_bytes());
        assert!(exact.json_projection(MAX_PROOF_BYTES).is_ok());
        limits.output_bytes -= 1;
        let error = export_from_kura(
            disk.signed.plan(),
            limits,
            &disk.root,
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
    // Independent projection response budget controls.
    #[test]
    fn projection_independent_cap_accepts_exact_complete_array_and_rejects_every_prefix() {
        for lanes in [1, 4] {
            let disk = Disk::new(lanes);
            let export = disk.export().unwrap();
            let canonical = export.canonical_bytes().to_vec();
            let complete = export.json_projection(MAX_PROOF_BYTES).unwrap();
            let exact = u64::try_from(complete.len()).unwrap();
            assert!(exact < export.rows().len() as u64 * (ROW_RESERVATION + 1) + 2);
            let bounded = export.json_projection(exact).unwrap();
            assert_eq!(bounded, complete);
            assert!(bounded.capacity() as u64 <= exact);
            let decoded: norito::json::Value = norito::json::from_slice(&bounded).unwrap();
            assert_eq!(decoded.as_array().unwrap().len(), export.rows().len());
            let mut prefix = 1_u64;
            let mut insufficient = vec![1, 2, exact - 1];
            for (index, row) in export.rows().iter().enumerate() {
                prefix += u64::from(index > 0) + projection_row(row).unwrap().len() as u64;
                if index + 1 < export.rows().len() {
                    insufficient.push(prefix + 1);
                }
            }
            for maximum in insufficient {
                assert!(maximum < exact);
                assert_eq!(
                    export.json_projection(maximum).unwrap_err().to_string(),
                    "projection exceeds independent maximum",
                    "a complete prefix must not escape at cap {maximum}",
                );
            }
            assert_eq!(export.canonical_bytes(), canonical);
            assert_eq!(export.json_projection(exact).unwrap(), complete);
        }
    }

    #[test]
    fn projection_independent_cap_rejects_zero_and_overflow() {
        let disk = Disk::new(1);
        let export = disk.export().unwrap();
        let canonical = export.canonical_bytes().to_vec();
        for maximum in [0, MAX_PROOF_BYTES + 1, u64::MAX] {
            assert_eq!(
                export.json_projection(maximum).unwrap_err().to_string(),
                "projection maximum must be between 1 byte and 256 MiB",
            );
        }
        assert_eq!(export.canonical_bytes(), canonical);
        assert!(!export.json_projection(MAX_PROOF_BYTES).unwrap().is_empty());
    }

    #[test]
    fn projection_independent_cap_preserves_original_plan_output_reservation() {
        let disk = Disk::new(1);
        let mut export = disk.export().unwrap();
        let complete = export.json_projection(MAX_PROOF_BYTES).unwrap();
        let reservation = export.rows().len() as u64 * (ROW_RESERVATION + 1) + 2;
        let required = export.canonical_bytes().len() as u64 + reservation;
        export.output_limit = required;
        assert_eq!(
            export.json_projection(complete.len() as u64).unwrap(),
            complete
        );
        export.output_limit = required - 1;
        assert_eq!(
            export
                .json_projection(complete.len() as u64)
                .unwrap_err()
                .to_string(),
            "projection output reservation exceeded",
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
        heights: fixture
            .heights
            .iter()
            .map(|height| HeightProofV1 {
                height: height.block.header().height().get(),
                carrier: height.block.encode_wire().unwrap(),
                carrier: height.block.encode_wire().unwrap(),
                lane_evidence: height.evidence.clone(),
                queries: height.queries(),
            })
            .collect(),
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
    assert_eq!(decoded.heights.len(), fixture.heights.len());
    assert!(decoded.heights[0].queries.is_empty());
    assert_eq!(
        decoded
            .heights
            .iter()
            .map(|height| height.queries.len())
            .sum::<usize>(),
        8
    );
    assert!(same_rows(&envelope.rows, &decoded.rows));

    // Bind the replay to independent fixture inputs, not the decoded envelope.
    let bindings: Vec<_> = fixture
        .heights
        .iter()
        .map(|height| HeightInputBinding {
            height: height.block.header().height().get(),
            carrier_hash: Hash::new(height.block.encode_wire().unwrap()),
            lane_evidence_hash: Hash::new(&height.evidence),
            query_hashes: height.queries().iter().map(Hash::new).collect(),
        })
        .collect();
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

    // A valid outer frame cannot authorize an unframed nested context proof.
    let mut bare_contexts: ExportEnvelopeV1 = norito::decode_canonical(&bytes).unwrap();
    bare_contexts.heights[1].lane_evidence =
        fixture.heights[1].evidence[norito::core::Header::SIZE..].to_vec();
    let bare_bytes = norito::encode_canonical(&bare_contexts).unwrap();
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
