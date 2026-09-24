//! Real generated genesis and retained Core/file boundary controls for stopped heights.

use super::*;
use crate::kura::scaling_evidence::export::launcher::prepare::assemble::tests::Fixture;
use iroha_crypto::{HashOf, KeyPair};
use norito::codec::{DecodeAll as _, Encode as _};
use std::{
    fs,
    io::BufWriter,
    os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink},
    panic::{AssertUnwindSafe, catch_unwind},
};

fn limits(fixture: &Fixture) -> CanonicalKuraEvidenceLimits {
    let mut limits = fixture.reader_limits();
    limits.last_height = 1;
    limits
}
fn run(fixture: &Fixture, hook: impl FnMut(Boundary) -> Result<()>) -> Result<RetainedStoppedTip> {
    observe_with_hook(
        fixture.bindings().signed_genesis,
        fixture.genesis().network_id,
        fixture.block_store(),
        fixture.merge_log(),
        limits(fixture),
        hook,
    )
}
fn core_paths(fixture: &Fixture) -> [PathBuf; 5] {
    [
        fixture.block_store().join("blocks.data"),
        fixture.block_store().join("blocks.index"),
        fixture.block_store().join("blocks.hashes"),
        fixture.block_store().join("blocks.count.norito"),
        fixture.merge_log().to_owned(),
    ]
}
fn replace(path: &Path) -> PathBuf {
    let raw = fs::read(path).unwrap();
    let info = fs::metadata(path).unwrap();
    let saved = path.with_extension("stopped-original");
    assert!(!saved.exists());
    fs::rename(path, &saved).unwrap();
    fs::write(path, &raw).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(info.mode() & 0o7777)).unwrap();
    assert_ne!(fs::metadata(path).unwrap().ino(), info.ino());
    assert_eq!(fs::read(path).unwrap(), raw);
    saved
}
fn restore(path: &Path, saved: &Path) {
    fs::remove_file(path).unwrap();
    fs::rename(saved, path).unwrap();
}
fn reject<T>(result: Result<T>, expected: &str) {
    let error = result.err().expect("observation must fail");
    assert!(format!("{error:#}").contains(expected), "{error:#}");
}

#[test]
fn stopped_tip_real_one_and_four_observe_whole_marker_with_only_genesis_carrier() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes);
        let mut original = fixture.bindings().signed_genesis;
        let copied = fs::read(&original.path).unwrap();
        // Use the actual generator's public original, not the semantic fixture's 0600 copy.
        original.path = fixture
            .block_store()
            .parent()
            .unwrap()
            .join("generated/genesis.signed.nrt");
        let bytes = fs::read(&original.path).unwrap();
        assert_eq!(bytes, copied);
        fs::set_permissions(&original.path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(fs::metadata(&original.path).unwrap().mode() & 0o777, 0o644);
        let owner = observe_stopped_tip(
            original,
            fixture.genesis().network_id,
            fixture.block_store(),
            fixture.merge_log(),
            limits(&fixture),
        )
        .unwrap();
        assert_eq!(owner.complete.carrier_count(), 1);
        assert_eq!(owner.complete.merge_frames(), 1);
        assert_eq!(owner.complete.committed_height(), 2);
        let mut writer = BufWriter::new(Vec::new());
        let identity = owner
            .finish_reply(|identity| {
                assert_eq!(identity.committed_height, 2);
                assert_eq!(identity.network_id, fixture.genesis().network_id);
                assert_eq!(identity.genesis.raw_sha256, iroha_crypto::sha256(&bytes));
                assert_eq!(identity.genesis.byte_length, bytes.len() as u64);
                writeln!(writer, "{}", identity.committed_height)?;
                writer.flush()?;
                Ok(())
            })
            .unwrap();
        assert_eq!(identity.committed_height, 2);
        assert_eq!(writer.into_inner().unwrap(), b"2\n");
        assert!(!fixture.output_path().exists());
    }
}

#[test]
fn stopped_tip_actual_genesis_only_store_yields_one_without_discovery_mode() {
    let fixture = Fixture::new(1);
    let root = fixture.block_store().with_file_name("genesis-only-kura");
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let genesis = iroha_genesis::decode_signed_genesis(
        &fs::read(fixture.bindings().signed_genesis.path).unwrap(),
    )
    .unwrap();
    let mut store = iroha_core::kura::BlockStore::new(&root);
    store.create_files_if_they_do_not_exist().unwrap();
    store.append_block_to_chain(&genesis).unwrap();
    drop(store);
    let merge = root.join("merge.log");
    fs::write(&merge, []).unwrap();
    for name in [
        "blocks.data",
        "blocks.index",
        "blocks.hashes",
        "blocks.count.norito",
        "merge.log",
    ] {
        fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o600)).unwrap();
    }
    let owner = observe_stopped_tip(
        fixture.bindings().signed_genesis,
        fixture.genesis().network_id,
        &root,
        &merge,
        limits(&fixture),
    )
    .unwrap();
    assert_eq!(owner.complete.merge_frames(), 0);
    assert_eq!(owner.finish_reply(|_| Ok(())).unwrap().committed_height, 1);
}

#[test]
fn stopped_tip_invalid_independent_bounds_reject_before_any_file_hook() {
    let base = CanonicalKuraEvidenceLimits {
        first_height: 1,
        last_height: 1,
        max_committed_blocks: 10,
        max_store_data_bytes: 1024 * 1024,
        max_carrier_bytes: 1024 * 1024,
        max_merge_log_bytes: 1024 * 1024,
        max_merge_frames: 10,
        max_output_bytes: 1024 * 1024,
        max_decode_allocation_bytes: 16 * 1024 * 1024,
        owner_uid: rustix::process::geteuid().as_raw(),
    };
    for case in 0..22 {
        let mut reader = base;
        let mut genesis = ProofInputBinding {
            path: "/absent/genesis.nrt".into(),
            sha256: [0; 32],
            max_bytes: 1024,
        };
        let mut root = PathBuf::from("/absent/kura");
        match case {
            0 => reader.first_height = 0,
            1 => reader.first_height = 2,
            2 => reader.last_height = 2,
            3 => reader.max_committed_blocks = 0,
            4 => reader.max_committed_blocks = 1_000_001,
            5 => reader.owner_uid = reader.owner_uid.wrapping_add(1),
            6 => reader.max_store_data_bytes = 0,
            7 => reader.max_store_data_bytes = 2 * 1024 * 1024 * 1024 + 1,
            8 => reader.max_carrier_bytes = 0,
            9 => reader.max_carrier_bytes = 32 * 1024 * 1024 + 1,
            10 => reader.max_merge_log_bytes = MAX_INPUT_BYTES + 1,
            11 => reader.max_merge_frames = 11,
            12 => reader.max_output_bytes = 0,
            13 => reader.max_output_bytes = MAX_INPUT_BYTES + 1,
            14 => reader.max_decode_allocation_bytes = 0,
            15 => reader.max_decode_allocation_bytes = 512 * 1024 * 1024 + 1,
            16 => genesis.max_bytes = 0,
            17 => genesis.max_bytes = 32 * 1024 * 1024 + 1,
            18 => genesis.path = root.join("blocks.data"),
            19 => root = "relative".into(),
            20 => genesis.path = "/absent/../genesis.nrt".into(),
            21 => root = format!("/{}", "x".repeat(MAX_PATH_BYTES)).into(),
            _ => unreachable!(),
        }
        let mut hooks = 0;
        let result = observe_with_hook(
            genesis,
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"test network",
            ))),
            &root,
            Path::new("/absent/merge.log"),
            reader,
            |_| {
                hooks += 1;
                Ok(())
            },
        );
        assert!(result.is_err(), "case {case}");
        assert_eq!(hooks, 0, "case {case}");
    }
}

#[test]
fn stopped_tip_raw_pin_network_and_same_header_different_signed_wire_are_independent() {
    let fixture = Fixture::new(1);
    let mut binding = fixture.bindings().signed_genesis;
    binding.sha256[0] ^= 1;
    reject(
        observe_stopped_tip(
            binding,
            fixture.genesis().network_id,
            fixture.block_store(),
            fixture.merge_log(),
            limits(&fixture),
        ),
        "SHA-256 mismatch",
    );
    reject(
        observe_stopped_tip(
            fixture.bindings().signed_genesis,
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"wrong network",
            ))),
            fixture.block_store(),
            fixture.merge_log(),
            limits(&fixture),
        ),
        "expected unmerged genesis",
    );
    let mut binding = fixture.bindings().signed_genesis;
    let raw = fs::read(&binding.path).unwrap();
    let mut changed = iroha_genesis::decode_signed_genesis(&raw).unwrap();
    let original_hash = changed.hash();
    changed.sign(KeyPair::random().private_key(), 20);
    assert_eq!(changed.hash(), original_hash);
    let changed = changed.encode_wire().unwrap();
    assert_ne!(changed, raw);
    fs::write(&binding.path, &changed).unwrap();
    binding.sha256 = iroha_crypto::sha256(&changed);
    reject(
        observe_stopped_tip(
            binding,
            fixture.genesis().network_id,
            fixture.block_store(),
            fixture.merge_log(),
            limits(&fixture),
        ),
        "differs from original signed genesis",
    );
    fs::write(fixture.bindings().signed_genesis.path, raw).unwrap();
}

#[test]
fn stopped_tip_actual_whole_store_files_reject_missing_truncated_and_uncommitted_suffix() {
    let fixture = Fixture::new(1);
    for path in core_paths(&fixture) {
        let raw = fs::read(&path).unwrap();
        assert!(!raw.is_empty());
        fs::write(&path, &raw[..raw.len() - 1]).unwrap();
        assert!(run(&fixture, |_| Ok(())).is_err(), "{}", path.display());
        fs::write(&path, &raw).unwrap();
        let saved = path.with_extension("missing-original");
        fs::rename(&path, &saved).unwrap();
        assert!(run(&fixture, |_| Ok(())).is_err(), "{}", path.display());
        fs::rename(saved, &path).unwrap();
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&[0]).unwrap();
        drop(file);
        assert!(run(&fixture, |_| Ok(())).is_err(), "{}", path.display());
        fs::write(&path, raw).unwrap();
    }
    assert_eq!(
        run(&fixture, |_| Ok(()))
            .unwrap()
            .finish_reply(|_| Ok(()))
            .unwrap()
            .committed_height,
        2
    );
}

#[test]
fn stopped_tip_full_merge_scan_rejects_genesis_epoch_gaps_and_beyond_marker_carriers() {
    let fixture = Fixture::new(1);
    let original = fs::read(fixture.merge_log()).unwrap();
    let entry = iroha_data_model::merge::MergeLedgerEntry::decode_all(&mut &original[4..]).unwrap();
    for case in 0..3 {
        let mut changed =
            iroha_data_model::merge::MergeLedgerEntry::decode_all(&mut &original[4..]).unwrap();
        match case {
            0 => changed.merge_qc.carrier_height = 1,
            1 => changed.epoch_id = 2,
            2 => changed.merge_qc.carrier_height = 3,
            _ => unreachable!(),
        }
        let payload = changed.encode();
        let mut raw = u32::try_from(payload.len()).unwrap().to_le_bytes().to_vec();
        raw.extend(payload);
        fs::write(fixture.merge_log(), raw).unwrap();
        assert!(run(&fixture, |_| Ok(())).is_err(), "case {case}");
    }
    assert_eq!(entry.epoch_id, 1);
    assert_eq!(entry.merge_qc.carrier_height, 2);
    fs::write(fixture.merge_log(), original).unwrap();
}

#[test]
fn stopped_tip_original_namespace_mode_and_every_admission_read_boundary_are_retained() {
    let fixture = Fixture::new(1);
    let path = fixture.bindings().signed_genesis.path;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    reject(run(&fixture, |_| Ok(())), "admitted owned single-link");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let original = path.with_extension("original-for-link");
    fs::rename(&path, &original).unwrap();
    symlink(&original, &path).unwrap();
    assert!(run(&fixture, |_| Ok(())).is_err());
    restore(&path, &original);
    for phase in [
        Phase::InputAdmitted,
        Phase::InputRetained,
        Phase::BeforeRead,
        Phase::AfterRead,
        Phase::BeforeInputFinish,
    ] {
        let mut saved = None;
        let result = run(&fixture, |point| {
            if point == Boundary::Original(phase) && saved.is_none() {
                saved = Some(replace(&path));
            }
            Ok(())
        });
        assert!(result.is_err(), "{phase:?}");
        restore(&path, &saved.expect("hook was reached"));
    }
    let parent = path.parent().unwrap();
    let saved = parent.with_extension("renamed-originals");
    let owner = run(&fixture, |_| Ok(())).unwrap();
    fs::rename(parent, &saved).unwrap();
    fs::create_dir(parent).unwrap();
    assert!(
        owner
            .finish_reply(|_| panic!("reply must not execute"))
            .is_err()
    );
    fs::remove_dir(parent).unwrap();
    fs::rename(saved, parent).unwrap();
}

#[test]
fn stopped_tip_core_and_genesis_replacement_at_every_observation_boundary_prevents_success() {
    let fixture = Fixture::new(1);
    let mut paths = core_paths(&fixture).to_vec();
    paths.push(fixture.bindings().signed_genesis.path);
    for boundary in [
        Boundary::AfterCoreOpen,
        Boundary::BeforeCarrier,
        Boundary::AfterCarrier,
        Boundary::BeforeMergeScan,
        Boundary::AfterMergeScan,
        Boundary::BeforeCoreFinish,
        Boundary::AfterCoreFinish,
        Boundary::BeforeIdentity,
        Boundary::AfterIdentity,
    ] {
        for path in &paths {
            let mut saved = None;
            let result = run(&fixture, |point| {
                if point == boundary && saved.is_none() {
                    saved = Some(replace(path));
                }
                Ok(())
            });
            assert!(result.is_err(), "{boundary:?}: {}", path.display());
            restore(path, &saved.expect("hook was reached"));
        }
    }
}

#[test]
fn stopped_tip_failed_or_panicking_identity_check_is_permanent_after_original_restoration() {
    let fixture = Fixture::new(1);
    for panic in [false, true] {
        let owner = run(&fixture, |_| Ok(())).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            owner.identity_with_hook(|point| {
                if point == Boundary::BeforeIdentity {
                    if panic {
                        panic!("identity hook panic");
                    }
                    return Err(eyre!("identity hook failure"));
                }
                Ok(())
            })
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        reject(
            owner.finish_reply(|_| panic!("poisoned reply must not execute")),
            "permanently failed",
        );
    }
    let owner = run(&fixture, |_| Ok(())).unwrap();
    let path = fixture.bindings().signed_genesis.path;
    let saved = replace(&path);
    assert!(owner.identity_with_hook(|_| Ok(())).is_err());
    restore(&path, &saved);
    reject(
        owner.finish_reply(|_| panic!("repaired source cannot repair owner")),
        "permanently failed",
    );
}

#[test]
fn stopped_tip_actual_reply_write_flush_mutations_errors_and_panics_never_return_success() {
    struct Writer<'a> {
        path: &'a Path,
        on_flush: bool,
        saved: Option<PathBuf>,
        bytes: Vec<u8>,
    }
    impl std::io::Write for Writer<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if !self.on_flush && self.saved.is_none() {
                self.saved = Some(replace(self.path));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            if self.on_flush && self.saved.is_none() {
                self.saved = Some(replace(self.path));
            }
            Ok(())
        }
    }
    let fixture = Fixture::new(1);
    let mut paths = core_paths(&fixture).to_vec();
    paths.push(fixture.bindings().signed_genesis.path);
    for path in &paths {
        for on_flush in [false, true] {
            let owner = run(&fixture, |_| Ok(())).unwrap();
            let mut writer = Writer {
                path,
                on_flush,
                saved: None,
                bytes: Vec::new(),
            };
            let result = owner.finish_reply(|identity| {
                writeln!(writer, "{}", identity.committed_height)?;
                writer.flush()?;
                Ok(())
            });
            assert!(result.is_err());
            assert_eq!(writer.bytes, b"2\n");
            restore(path, &writer.saved.expect("actual writer boundary reached"));
        }
    }
    reject(
        run(&fixture, |_| Ok(()))
            .unwrap()
            .finish_reply(|_| Err(eyre!("reply failed"))),
        "reply failed",
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| run(&fixture, |_| Ok(()))
            .unwrap()
            .finish_reply(|_| panic!("reply panic"))))
        .is_err()
    );
    assert!(!fixture.output_path().exists());
}

#[test]
fn stopped_tip_original_under_core_namespace_and_core_parent_replacement_are_rejected() {
    let fixture = Fixture::new(1);
    let raw = fs::read(fixture.bindings().signed_genesis.path).unwrap();
    let path = fixture.block_store().join("original-genesis.nrt");
    fs::write(&path, &raw).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    reject(
        observe_stopped_tip(
            ProofInputBinding {
                path: path.clone(),
                sha256: iroha_crypto::sha256(&raw),
                max_bytes: raw.len() as u64,
            },
            fixture.genesis().network_id,
            fixture.block_store(),
            fixture.merge_log(),
            limits(&fixture),
        ),
        "protected Core source namespace",
    );
    fs::remove_file(path).unwrap();
    let owner = run(&fixture, |_| Ok(())).unwrap();
    let root = fixture.block_store();
    let saved = root.with_extension("retained-core");
    fs::rename(root, &saved).unwrap();
    fs::create_dir(root).unwrap();
    assert!(
        owner
            .finish_reply(|_| panic!("changed Core parent must prevent reply"))
            .is_err()
    );
    fs::remove_dir(root).unwrap();
    fs::rename(saved, root).unwrap();
}
