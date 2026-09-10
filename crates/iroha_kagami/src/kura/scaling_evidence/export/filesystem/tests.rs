//! Actual retained descriptor and signed canonical adapter controls.

// Actual descriptor and signed-adapter controls; no global cwd/hooks or child processes.
use super::*;
use crate::kura::scaling_evidence::*;
use iroha_core::kura::BlockStore;
use norito::codec::Encode as _;
use std::{
    cell::Cell,
    fs,
    os::unix::{
        fs::{MetadataExt as _, PermissionsExt as _, symlink},
        net::UnixListener,
    },
    panic::{AssertUnwindSafe, catch_unwind},
};
#[path = "../../fixture.rs"]
mod fixture;

struct Files {
    _directory: tempfile::TempDir,
    ancestor: PathBuf,
    parent: PathBuf,
    input: PathBuf,
}
impl Files {
    fn new() -> Self {
        let directory = tempfile::Builder::new()
            .prefix("proof-")
            .tempdir_in("/tmp")
            .unwrap();
        let ancestor = directory.path().canonicalize().unwrap().join("a");
        let parent = ancestor.join("p");
        fs::create_dir_all(&parent).unwrap();
        let input = parent.join("input");
        fs::write(&input, b"exact admitted input").unwrap();
        fs::set_permissions(&input, fs::Permissions::from_mode(0o600)).unwrap();
        Self {
            _directory: directory,
            ancestor,
            parent,
            input,
        }
    }
    fn binding(&self) -> ProofInputBinding {
        binding(&self.input)
    }
    fn output(&self) -> PathBuf {
        self.parent.join("canonical.norito")
    }
    fn stage(&self) -> PathBuf {
        self.parent.join("canonical.norito.publishing")
    }
    fn replace_parent(&self, ancestor: bool) {
        let path = if ancestor {
            &self.ancestor
        } else {
            &self.parent
        };
        fs::rename(path, path.with_extension("old")).unwrap();
        fs::create_dir(path).unwrap();
    }
}
fn binding(path: &Path) -> ProofInputBinding {
    let bytes = fs::read(path).unwrap();
    ProofInputBinding {
        path: path.to_owned(),
        sha256: iroha_crypto::sha256(&bytes),
        max_bytes: bytes.len() as u64,
    }
}
fn fifo(path: &Path) -> File {
    crate::secure_fs::create_fifo_for_test(path, 0o600).unwrap();
    // An independent RDWR|NONBLOCK peer prevents a missing-NONBLOCK regression
    // from hanging the test process. Admission must still reject the FIFO.
    File::from(
        rustix::fs::open(
            path,
            OFlags::RDWR | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap(),
    )
}
struct Disk {
    files: Files,
    root: PathBuf,
    log: PathBuf,
    signed: fixture::Fixture,
}
impl Disk {
    fn new(lanes: usize) -> Self {
        Self::with_requests(lanes, 8)
    }
    fn with_requests(lanes: usize, requests: usize) -> Self {
        let files = Files::new();
        let root = files.ancestor.join("kura");
        fs::create_dir(&root).unwrap();
        let signed = fixture::Fixture::with_request_count(lanes, requests);
        let mut store = BlockStore::new(&root);
        store.create_files_if_they_do_not_exist().unwrap();
        store.append_block_to_chain(&signed.genesis).unwrap();
        store.append_block_to_chain(&signed.carrier).unwrap();
        drop(store);
        let log = root.join("merge.log");
        let encoded = signed.entry.encode();
        let mut entry = (encoded.len() as u32).to_le_bytes().to_vec();
        entry.extend_from_slice(&encoded);
        fs::write(&log, entry).unwrap();
        Self {
            files,
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
        self.supplied()
            .iter()
            .map(|row| HeightInputBinding {
                height: row.height,
                finality_hash: Hash::new(&row.finality),
                query_hashes: row.queries.iter().map(Hash::new).collect(),
            })
            .collect()
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
    fn limits(&self) -> VerificationLimits {
        let mut limits = fixture::limits();
        limits.requests = limits.requests.max(self.signed.requests.len());
        limits.leaves_per_carrier = limits.leaves_per_carrier.max(self.signed.requests.len());
        limits
    }
    fn export(&self) -> VerifiedExport {
        crate::kura::scaling_evidence::export::export_from_kura(
            self.signed.plan(),
            self.limits(),
            &self.root,
            &self.log,
            self.reader_limits(),
            &self.bindings(),
            self.supplied(),
        )
        .unwrap()
    }
    fn retain(&self) -> RetainedProof {
        let exported = self.export();
        fs::write(&self.files.input, exported.canonical_bytes()).unwrap();
        replay_bound_export(
            self.signed.plan(),
            self.limits(),
            &self.bindings(),
            Hash::new(exported.canonical_bytes()),
            self.files.binding(),
        )
        .unwrap()
    }
    fn bundle(&self) -> SuppliedEvidenceBundleV1 {
        SuppliedEvidenceBundleV1 {
            version: 1,
            heights: self
                .supplied()
                .into_iter()
                .map(|h| SuppliedEvidenceHeightV1 {
                    height: h.height,
                    finality: h.finality,
                    queries: h.queries,
                })
                .collect(),
        }
    }
    fn bundle_file(&self) -> ProofInputBinding {
        let path = self.files.parent.join("supplied.norito");
        fs::write(&path, norito::encode_canonical(&self.bundle()).unwrap()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        binding(&path)
    }
    fn bound_export(&self) -> RetainedProof {
        export_bound_kura(
            self.signed.plan(),
            self.limits(),
            &self.root,
            &self.log,
            self.reader_limits(),
            &self.bindings(),
            self.bundle_file(),
        )
        .unwrap()
    }
}

#[test]
fn retained_input_uses_real_nonblocking_descriptor_and_exact_bytes() {
    let files = Files::new();
    let expected = fs::read(&files.input).unwrap();
    let mut owner = Inputs::open(vec![files.binding()], 1024, &mut |_| Ok(())).unwrap();
    let flags = rustix::fs::fcntl_getfl(&owner.files[0].file).unwrap();
    assert!(flags.contains(OFlags::NONBLOCK));
    assert!(
        rustix::io::fcntl_getfd(&owner.files[0].file)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    assert_eq!(owner.read_all(&mut |_| Ok(())).unwrap(), vec![expected]);
    assert!(owner.read_all(&mut |_| Ok(())).is_err());
}
#[test]
fn count_and_aggregate_caps_reject_before_any_open() {
    let files = Files::new();
    let touched = Cell::new(false);
    let mut hook = |_| {
        touched.set(true);
        Ok(())
    };
    let mut too_small = files.binding();
    too_small.max_bytes = 1025;
    assert!(Inputs::open(vec![too_small], 1024, &mut hook).is_err());
    assert!(!touched.get());
    let many = (0..=MAX_INPUT_FILES).map(|_| files.binding()).collect();
    assert!(Inputs::open(many, MAX_INPUT_BYTES, &mut hook).is_err());
    assert!(Inputs::open(vec![], 1024, &mut hook).is_err());
    assert!(Inputs::open(vec![files.binding()], MAX_INPUT_BYTES + 1, &mut hook).is_err());
    assert!(!touched.get());
}
#[test]
fn actual_oversize_and_zero_file_fail_before_read_allocation() {
    let files = Files::new();
    let mut bound = files.binding();
    bound.max_bytes -= 1;
    let read = Cell::new(false);
    assert!(
        Inputs::open(vec![bound], 1024, &mut |p| {
            read.set(p == Phase::BeforeRead);
            Ok(())
        })
        .is_err()
    );
    assert!(!read.get());
    fs::write(&files.input, b"").unwrap();
    let mut empty = files.binding();
    empty.max_bytes = 1;
    assert!(Inputs::open(vec![empty], 1024, &mut |_| Ok(())).is_err());
}
#[test]
fn duplicate_paths_and_actual_hardlinks_are_rejected() {
    let files = Files::new();
    assert!(
        Inputs::open(
            vec![files.binding(), files.binding()],
            1024,
            &mut |_| Ok(())
        )
        .is_err()
    );
    fs::hard_link(&files.input, files.parent.join("alias")).unwrap();
    assert!(Inputs::open(vec![files.binding()], 1024, &mut |_| Ok(())).is_err());
    assert_eq!(fs::metadata(&files.input).unwrap().nlink(), 2);
}
#[test]
fn symlink_and_special_inputs_are_rejected_without_blocking() {
    for kind in 0..4 {
        let files = Files::new();
        let bound = files.binding();
        fs::rename(&files.input, files.parent.join("original")).unwrap();
        let mut peer = None;
        let mut socket = None;
        match kind {
            0 => symlink(files.parent.join("original"), &files.input).unwrap(),
            1 => peer = Some(fifo(&files.input)),
            2 => socket = Some(UnixListener::bind(&files.input).unwrap()),
            _ => fs::create_dir(&files.input).unwrap(),
        }
        assert!(Inputs::open(vec![bound], 1024, &mut |_| Ok(())).is_err());
        assert!(files.parent.join("original").is_file());
        drop((peer, socket));
    }
}
#[test]
fn post_admission_fifo_and_symlink_races_reject_actual_safe_open() {
    for kind in 0..2 {
        let files = Files::new();
        let bound = files.binding();
        let mut peer = None;
        let touched = Cell::new(false);
        let result = Inputs::open(vec![bound], 1024, &mut |phase| {
            if phase == Phase::InputAdmitted {
                touched.set(true);
                fs::rename(&files.input, files.parent.join("original"))?;
                if kind == 0 {
                    peer = Some(fifo(&files.input));
                } else {
                    symlink(files.parent.join("original"), &files.input)?;
                }
            }
            Ok(())
        });
        assert!(touched.get());
        assert!(result.is_err());
        drop(peer);
    }
}
#[test]
fn all_retained_parent_and_ancestor_replacements_fail() {
    for ancestor in [false, true] {
        for phase in [
            Phase::ParentAdmitted,
            Phase::ParentRetained,
            Phase::InputRetained,
            Phase::BeforeRead,
            Phase::AfterRead,
        ] {
            let files = Files::new();
            let touched = Cell::new(false);
            let mut hook = |observed| {
                if observed == phase && !touched.replace(true) {
                    files.replace_parent(ancestor);
                }
                Ok(())
            };
            let result = Inputs::open(vec![files.binding()], 1024, &mut hook)
                .and_then(|mut owner| owner.read_all(&mut hook));
            assert!(touched.get());
            assert!(result.is_err());
        }
    }
}
#[test]
fn read_digest_failure_and_caught_panic_poison_actual_owner() {
    let files = Files::new();
    let mut bad = files.binding();
    bad.sha256[0] ^= 1;
    let mut owner = Inputs::open(vec![bad], 1024, &mut |_| Ok(())).unwrap();
    assert!(owner.read_all(&mut |_| Ok(())).is_err());
    assert!(owner.poisoned);
    assert!(owner.read_all(&mut |_| Ok(())).is_err());
    let mut owner = Inputs::open(vec![files.binding()], 1024, &mut |_| Ok(())).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| owner.read_all(&mut |phase| {
            if phase == Phase::AfterRead {
                panic!("caught actual read boundary");
            }
            Ok(())
        })))
        .is_err()
    );
    assert!(owner.poisoned);
    assert!(owner.read_all(&mut |_| Ok(())).is_err());
    let disk = Disk::new(1);
    assert!(owner.finish(disk.export(), &mut |_| Ok(())).is_err());
}
#[test]
fn same_length_content_replacement_growth_and_mode_changes_fail() {
    for change in 0..4 {
        let files = Files::new();
        let bound = files.binding();
        let mut owner = Inputs::open(vec![bound], 1024, &mut |_| Ok(())).unwrap();
        assert!(
            owner
                .read_all(&mut |phase| {
                    if phase == Phase::AfterRead {
                        match change {
                            0 => fs::write(&files.input, vec![b'x'; 20])?,
                            1 => {
                                let mut file =
                                    fs::OpenOptions::new().append(true).open(&files.input)?;
                                file.write_all(b"+")?;
                            }
                            2 => fs::set_permissions(
                                &files.input,
                                fs::Permissions::from_mode(0o644),
                            )?,
                            _ => {
                                fs::rename(&files.input, files.parent.join("old"))?;
                                fs::write(&files.input, b"exact admitted input")?;
                            }
                        }
                    }
                    Ok(())
                })
                .is_err()
        );
        assert!(owner.poisoned);
    }
}
#[test]
fn lexical_path_limits_and_unsafe_file_modes_fail() {
    let files = Files::new();
    for path in [
        PathBuf::from("relative"),
        files.parent.join("../p/input"),
        PathBuf::from(format!("{}/./input", files.parent.display())),
        PathBuf::from(format!("/{}", "a/".repeat(65))),
        PathBuf::from(format!("/{}", "a".repeat(4096))),
    ] {
        let mut bound = files.binding();
        bound.path = path;
        assert!(Inputs::open(vec![bound], 1024, &mut |_| Ok(())).is_err());
    }
    fs::set_permissions(&files.input, fs::Permissions::from_mode(0o666)).unwrap();
    assert!(Inputs::open(vec![files.binding()], 1024, &mut |_| Ok(())).is_err());
}
#[test]
fn actual_signed_export_files_and_replay_finish_before_projection() {
    for lanes in [1, 4] {
        let disk = Disk::new(lanes);
        let expected = disk.export();
        let retained = export_bound_kura(
            disk.signed.plan(),
            fixture::limits(),
            &disk.root,
            &disk.log,
            disk.reader_limits(),
            &disk.bindings(),
            disk.bundle_file(),
        )
        .unwrap();
        assert_eq!(retained.proof.canonical_bytes(), expected.canonical_bytes());
        assert_eq!(
            retained.json_projection().unwrap(),
            expected.json_projection().unwrap()
        );
        let replay = disk.retain();
        assert_eq!(replay.proof.canonical_bytes(), expected.canonical_bytes());
    }
}
#[test]
fn separately_valid_old_artifact_cannot_certify_a_different_plan() {
    let one = Disk::new(1);
    let four = Disk::new(4);
    let old = one.retain();
    let current = four.retain();
    assert!(!old.json_projection().unwrap().is_empty());
    assert!(!current.json_projection().unwrap().is_empty());
    assert!(
        replay_bound_export(
            four.signed.plan(),
            fixture::limits(),
            &four.bindings(),
            Hash::new(old.proof.canonical_bytes()),
            one.files.binding()
        )
        .is_err()
    );
}
#[test]
fn exact_sha_and_iroha_hash_are_independent_input_authorities() {
    let disk = Disk::new(1);
    let completed = disk.retain();
    assert!(!completed.json_projection().unwrap().is_empty());
    let mut bad = disk.files.binding();
    bad.sha256[0] ^= 1;
    assert!(
        replay_bound_export(
            disk.signed.plan(),
            fixture::limits(),
            &disk.bindings(),
            Hash::new(completed.proof.canonical_bytes()),
            bad
        )
        .is_err()
    );
    assert!(
        replay_bound_export(
            disk.signed.plan(),
            fixture::limits(),
            &disk.bindings(),
            Hash::new(b"wrong marked hash"),
            disk.files.binding()
        )
        .is_err()
    );
}
#[test]
fn late_input_mutation_and_finish_fault_expose_no_completed_proof() {
    let disk = Disk::new(1);
    let completed = disk.retain();
    let hash = Hash::new(completed.proof.canonical_bytes());
    for phase in [Phase::AfterVerification, Phase::BeforeInputFinish] {
        fs::write(&disk.files.input, completed.proof.canonical_bytes()).unwrap();
        let reached = Cell::new(false);
        let result = replay_with_hook(
            disk.signed.plan(),
            fixture::limits(),
            &disk.bindings(),
            hash,
            disk.files.binding(),
            |current| {
                if current == phase {
                    reached.set(true);
                    fs::rename(&disk.files.input, disk.files.parent.join("old"))?;
                    fs::write(&disk.files.input, completed.proof.canonical_bytes())?;
                }
                Ok(())
            },
        );
        assert!(reached.get());
        assert!(result.is_err());
    }
}
#[test]
fn consuming_verification_panic_never_returns_completion() {
    let disk = Disk::new(1);
    let completed = disk.retain();
    let reached = Cell::new(false);
    assert!(
        catch_unwind(AssertUnwindSafe(|| replay_with_hook(
            disk.signed.plan(),
            fixture::limits(),
            &disk.bindings(),
            Hash::new(completed.proof.canonical_bytes()),
            disk.files.binding(),
            |phase| {
                if phase == Phase::AfterVerification {
                    reached.set(true);
                    panic!("after actual verification");
                }
                Ok(())
            }
        )))
        .is_err()
    );
    assert!(reached.get());
    assert!(!disk.files.output().exists());
}
#[test]
fn canonical_bundle_role_mismatch_rejects_after_bounded_read_before_core() {
    let disk = Disk::new(1);
    let expected = disk.export();
    let positive = export_bound_kura(
        disk.signed.plan(),
        fixture::limits(),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
        &disk.bindings(),
        disk.bundle_file(),
    )
    .unwrap();
    assert_eq!(positive.proof.canonical_bytes(), expected.canonical_bytes());
    let mut bundle = disk.bundle();
    bundle.heights[0].height = 99;
    let path = disk.bundle_file().path;
    fs::write(&path, norito::encode_canonical(&bundle).unwrap()).unwrap();
    let result = export_bound_kura(
        disk.signed.plan(),
        fixture::limits(),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
        &disk.bindings(),
        binding(&path),
    );
    assert!(result.err().unwrap().to_string().contains("bundle roles"));
}

#[test]
fn canonical_output_exact_cap_preserves_bytes_and_requires_all_syncs() {
    let disk = Disk::new(1);
    let proof = disk.retain();
    let bytes = proof.proof.canonical_bytes().to_vec();
    let owner = ProofOutput::admit(&disk.files.output(), bytes.len() as u64).unwrap();
    let mut phases = Vec::new();
    let published = owner
        .publish_with_hook(proof, |p| {
            phases.push(p);
            Ok(())
        })
        .unwrap();
    assert_eq!(fs::read(disk.files.output()).unwrap(), bytes);
    assert_eq!(published.sha256(), iroha_crypto::sha256(&bytes));
    assert_eq!(published.byte_length(), bytes.len() as u64);
    assert_eq!(
        fs::metadata(disk.files.output()).unwrap().mode() & 0o7777,
        0o600
    );
    assert!(!disk.files.stage().exists());
    assert!(phases.contains(&Phase::BeforeFileSync));
    assert!(
        phases
            .iter()
            .rposition(|p| *p == Phase::AfterDirectorySync)
            .unwrap()
            > phases
                .iter()
                .position(|p| *p == Phase::AfterRename)
                .unwrap()
    );
    assert!(!published.json_projection().unwrap().is_empty());
}
#[test]
fn output_one_byte_under_cap_has_no_stage_or_destination() {
    let disk = Disk::new(1);
    let proof = disk.retain();
    let owner = ProofOutput::admit(
        &disk.files.output(),
        proof.proof.canonical_bytes().len() as u64 - 1,
    )
    .unwrap();
    assert!(owner.publish(proof).is_err());
    assert!(!disk.files.output().exists());
    assert!(!disk.files.stage().exists());
}
#[test]
fn existing_destination_and_stage_are_preserved_exactly() {
    let files = Files::new();
    fs::write(files.output(), b"destination owner").unwrap();
    assert!(ProofOutput::admit(&files.output(), 1024).is_err());
    assert_eq!(fs::read(files.output()).unwrap(), b"destination owner");
    fs::remove_file(files.output()).unwrap();
    fs::write(files.stage(), b"stage owner").unwrap();
    assert!(ProofOutput::admit(&files.output(), 1024).is_err());
    assert_eq!(fs::read(files.stage()).unwrap(), b"stage owner");
}
#[test]
fn destination_racer_is_never_replaced_by_publication() {
    let disk = Disk::new(1);
    let proof = disk.retain();
    let owner = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES).unwrap();
    let reached = Cell::new(false);
    assert!(
        owner
            .publish_with_hook(proof, |phase| {
                if phase == Phase::RenameReady {
                    reached.set(true);
                    fs::write(disk.files.output(), b"independent racer")?;
                }
                Ok(())
            })
            .is_err()
    );
    assert!(reached.get());
    assert_eq!(fs::read(disk.files.output()).unwrap(), b"independent racer");
    assert!(disk.files.stage().exists());
}
#[test]
fn output_parent_and_ancestor_replacement_fail_before_publication() {
    for ancestor in [false, true] {
        let disk = Disk::new(1);
        let proof = disk.retain();
        let owner = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES).unwrap();
        disk.files.replace_parent(ancestor);
        assert!(owner.publish(proof).is_err());
        assert!(!disk.files.output().exists());
    }
}
#[test]
fn post_create_stage_special_or_hardlink_substitution_is_rejected() {
    for kind in 0..4 {
        let disk = Disk::new(1);
        let proof = disk.retain();
        let owner = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES).unwrap();
        let mut peer = None;
        let mut socket = None;
        assert!(
            owner
                .publish_with_hook(proof, |phase| {
                    if phase == Phase::AfterCreate {
                        if kind == 3 {
                            fs::hard_link(disk.files.stage(), disk.files.parent.join("alias"))?;
                        } else {
                            fs::rename(
                                disk.files.stage(),
                                disk.files.parent.join("original-stage"),
                            )?;
                            match kind {
                                0 => symlink(
                                    disk.files.parent.join("original-stage"),
                                    disk.files.stage(),
                                )?,
                                1 => peer = Some(fifo(&disk.files.stage())),
                                _ => socket = Some(UnixListener::bind(disk.files.stage())?),
                            }
                        }
                    }
                    Ok(())
                })
                .is_err()
        );
        assert!(!disk.files.output().exists());
        assert!(fs::symlink_metadata(disk.files.stage()).is_ok());
        drop((peer, socket));
    }
}
#[test]
fn final_source_name_race_is_detected_and_never_cleaned_as_owned() {
    let disk = Disk::new(1);
    let proof = disk.retain();
    let owner = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES).unwrap();
    assert!(
        owner
            .publish_with_hook(proof, |phase| {
                if phase == Phase::RenameReady {
                    fs::rename(disk.files.stage(), disk.files.parent.join("retained-stage"))?;
                    fs::write(disk.files.stage(), b"different source inode")?;
                }
                Ok(())
            })
            .is_err()
    );
    assert_eq!(
        fs::read(disk.files.output()).unwrap(),
        b"different source inode"
    );
    assert!(disk.files.parent.join("retained-stage").is_file());
}
#[test]
fn late_parent_sync_failure_keeps_artifact_but_returns_no_receipt() {
    let disk = Disk::new(1);
    let proof = disk.retain();
    let bytes = proof.proof.canonical_bytes().to_vec();
    let owner = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES).unwrap();
    let renamed = Cell::new(false);
    let failed = Cell::new(false);
    assert!(
        owner
            .publish_with_hook(proof, |phase| {
                if phase == Phase::AfterRename {
                    renamed.set(true);
                }
                if renamed.get() && phase == Phase::BeforeDirectorySync {
                    failed.set(true);
                    return Err(eyre!("injected retained parent sync failure"));
                }
                Ok(())
            })
            .is_err()
    );
    assert!(renamed.get());
    assert!(failed.get());
    assert_eq!(fs::read(disk.files.output()).unwrap(), bytes);
}

#[test]
fn one_canonical_bundle_authenticates_128_requests_without_per_leaf_files() {
    let disk = Disk::with_requests(4, 128);
    let expected = disk.export();
    assert_eq!(expected.rows().len(), 128);
    assert_eq!(disk.bindings()[1].query_hashes.len(), 128);
    let input = disk.bundle_file();
    assert_eq!(input.path.file_name().unwrap(), "supplied.norito");
    let retained = export_bound_kura(
        disk.signed.plan(),
        disk.limits(),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
        &disk.bindings(),
        input,
    )
    .unwrap();
    assert_eq!(retained.proof.rows().len(), 128);
    assert_eq!(retained.proof.canonical_bytes(), expected.canonical_bytes());
    assert_eq!(
        retained.json_projection().unwrap(),
        expected.json_projection().unwrap()
    );
    let names: Vec<_> = fs::read_dir(&disk.files.parent)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 2); // Original unrelated input plus one bundle; no query files.
    assert!(names.contains(&OsString::from("supplied.norito")));
}

#[test]
fn supplied_bundle_requires_exact_canonical_v1_framing() {
    let disk = Disk::new(1);
    let positive = export_bound_kura(
        disk.signed.plan(),
        disk.limits(),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
        &disk.bindings(),
        disk.bundle_file(),
    )
    .unwrap();
    assert_eq!(positive.proof.rows().len(), 8);
    let canonical = norito::encode_canonical(&disk.bundle()).unwrap();
    let mut trailing = canonical.clone();
    trailing.push(0);
    let bare = disk.bundle().encode();
    let mut version = disk.bundle();
    version.version = 2;
    for bad in [
        bare,
        trailing,
        canonical[..canonical.len() - 1].to_vec(),
        norito::encode_canonical(&version).unwrap(),
    ] {
        let path = disk.bundle_file().path;
        fs::write(&path, bad).unwrap();
        assert!(
            export_bound_kura(
                disk.signed.plan(),
                disk.limits(),
                &disk.root,
                &disk.log,
                disk.reader_limits(),
                &disk.bindings(),
                binding(&path)
            )
            .is_err()
        );
    }
}

#[test]
fn supplied_bundle_rehash_cannot_change_complete_height_or_leaf_roles() {
    let disk = Disk::new(1);
    let positive = export_bound_kura(
        disk.signed.plan(),
        disk.limits(),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
        &disk.bindings(),
        disk.bundle_file(),
    )
    .unwrap();
    assert_eq!(positive.proof.rows().len(), 8);
    for change in 0..5 {
        let mut bundle = disk.bundle();
        match change {
            0 => bundle.heights.swap(0, 1),
            1 => {
                bundle.heights.pop();
            }
            2 => {
                bundle.heights[1].queries.pop();
            }
            3 => bundle.heights[1].queries.swap(0, 1),
            _ => bundle.heights[1].finality[0] ^= 1,
        }
        let path = disk.bundle_file().path;
        fs::write(&path, norito::encode_canonical(&bundle).unwrap()).unwrap();
        assert!(
            export_bound_kura(
                disk.signed.plan(),
                disk.limits(),
                &disk.root,
                &disk.log,
                disk.reader_limits(),
                &disk.bindings(),
                binding(&path)
            )
            .is_err()
        );
    }
}

#[test]
fn supplied_bundle_one_byte_under_admitted_size_rejects_after_actual_positive() {
    let disk = Disk::new(1);
    let positive = export_bound_kura(
        disk.signed.plan(),
        disk.limits(),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
        &disk.bindings(),
        disk.bundle_file(),
    )
    .unwrap();
    assert_eq!(positive.proof.rows().len(), 8);
    let mut input = disk.bundle_file();
    let exact = input.max_bytes;
    input.max_bytes -= 1;
    let failure = export_bound_kura(
        disk.signed.plan(),
        disk.limits(),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
        &disk.bindings(),
        input,
    );
    assert!(
        failure
            .err()
            .unwrap()
            .to_string()
            .contains("bounded regular file")
    );
    assert_eq!(
        fs::metadata(disk.files.parent.join("supplied.norito"))
            .unwrap()
            .len(),
        exact
    );
}

#[test]
fn completed_core_lease_allows_distinct_output_in_generic_bundle_parent() {
    let disk = Disk::new(1);
    let proof = disk.bound_export();
    assert!(proof.proof.disk_completion.is_some());
    let bytes = proof.proof.canonical_bytes().to_vec();
    let published = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES)
        .unwrap()
        .publish(proof)
        .unwrap();
    assert_eq!(published.byte_length(), bytes.len() as u64);
    assert_eq!(fs::read(disk.files.output()).unwrap(), bytes);
    assert!(!published.json_projection().unwrap().is_empty());
}

#[test]
fn output_equal_to_or_below_actual_core_store_is_rejected_before_stage_creation() {
    let disk = Disk::new(1);
    let nested = disk.root.join("nested");
    fs::create_dir(&nested).unwrap();
    for parent in [&disk.root, &nested] {
        let proof = disk.bound_export();
        assert!(!proof.json_projection().unwrap().is_empty());
        let output = parent.join("proof.norito");
        let owner = ProofOutput::admit(&output, MAX_INPUT_BYTES).unwrap();
        let created = Cell::new(false);
        let result = owner.publish_with_hook(proof, |phase| {
            if phase == Phase::BeforeCreate {
                created.set(true);
            }
            Ok(())
        });
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("protected Core source namespace")
        );
        assert!(!created.get());
        assert!(!output.exists());
        assert!(!parent.join("proof.norito.publishing").exists());
    }
}

#[test]
fn separately_located_merge_log_namespace_is_also_protected() {
    let mut disk = Disk::new(1);
    let log_root = disk.files.ancestor.join("merge-scope");
    fs::create_dir(&log_root).unwrap();
    let moved = log_root.join("merge.log");
    fs::rename(&disk.log, &moved).unwrap();
    disk.log = moved;
    let proof = disk.bound_export();
    assert!(!proof.json_projection().unwrap().is_empty());
    let output = log_root.join("proof.norito");
    assert!(
        ProofOutput::admit(&output, MAX_INPUT_BYTES)
            .unwrap()
            .publish(proof)
            .is_err()
    );
    assert!(!output.exists());
    assert!(!log_root.join("proof.norito.publishing").exists());
}

#[test]
fn actual_core_read_root_survives_swap_read_restore_and_rejects_publication() {
    let disk = Disk::new(1);
    let baseline = disk.bound_export();
    assert!(!baseline.json_projection().unwrap().is_empty());
    let replacement = disk.files.ancestor.join("replacement-store");
    fs::create_dir(&replacement).unwrap();
    for entry in fs::read_dir(&disk.root).unwrap() {
        let entry = entry.unwrap();
        assert!(entry.file_type().unwrap().is_file());
        fs::copy(entry.path(), replacement.join(entry.file_name())).unwrap();
    }
    let original_inode = fs::metadata(&disk.root).unwrap().ino();
    let replacement_inode = fs::metadata(&replacement).unwrap().ino();
    assert_ne!(original_inode, replacement_inode);
    let original = disk.files.ancestor.join("original-store");
    fs::rename(&disk.root, &original).unwrap();
    fs::rename(&replacement, &disk.root).unwrap();
    // Core now reads the different, but fully valid, copied namespace. A parallel
    // guard captured before this swap could incorrectly accept the restored root.
    let proof = disk.bound_export();
    assert!(!proof.json_projection().unwrap().is_empty());
    fs::rename(&disk.root, &replacement).unwrap();
    fs::rename(&original, &disk.root).unwrap();
    assert_eq!(fs::metadata(&disk.root).unwrap().ino(), original_inode);
    let owner = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES).unwrap();
    let created = Cell::new(false);
    assert!(
        owner
            .publish_with_hook(proof, |phase| {
                if phase == Phase::BeforeCreate {
                    created.set(true);
                }
                Ok(())
            })
            .is_err()
    );
    assert!(!created.get());
    assert!(!disk.files.output().exists());
    assert!(!disk.files.stage().exists());
}

#[test]
fn verified_input_unlink_cannot_turn_its_name_into_a_new_output() {
    let disk = Disk::new(1);
    let proof = disk.retain();
    assert!(!proof.json_projection().unwrap().is_empty());
    let original = disk.files.parent.join("original-input");
    fs::rename(&disk.files.input, &original).unwrap();
    let owner = ProofOutput::admit(&disk.files.input, MAX_INPUT_BYTES).unwrap();
    assert!(owner.publish(proof).is_err());
    assert!(!disk.files.input.exists());
    assert!(!disk.files.parent.join("input.publishing").exists());
    assert!(original.is_file());
}

#[test]
fn input_namespace_rebind_after_verification_rejects_a_separate_output_parent() {
    let disk = Disk::new(1);
    let output_parent = disk.files.ancestor.join("publication");
    fs::create_dir(&output_parent).unwrap();
    let output = output_parent.join("proof.norito");
    let proof = disk.bound_export();
    assert!(!proof.json_projection().unwrap().is_empty());
    let owner = ProofOutput::admit(&output, MAX_INPUT_BYTES).unwrap();
    disk.files.replace_parent(false);
    assert!(owner.publish(proof).is_err());
    assert!(!output.exists());
    assert!(!output_parent.join("proof.norito.publishing").exists());
}

#[test]
fn input_content_changed_during_publication_fails_before_destination() {
    let disk = Disk::new(1);
    let proof = disk.bound_export();
    let input = disk.files.parent.join("supplied.norito");
    let owner = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES).unwrap();
    let reached = Cell::new(false);
    assert!(
        owner
            .publish_with_hook(proof, |phase| {
                if phase == Phase::BeforeFileSync {
                    reached.set(true);
                    fs::write(&input, b"changed after verification")?;
                }
                Ok(())
            })
            .is_err()
    );
    assert!(reached.get());
    assert!(!disk.files.output().exists());
    assert!(disk.files.stage().exists());
}

#[test]
fn actual_core_source_change_after_rename_returns_no_publication_receipt() {
    let disk = Disk::new(1);
    let proof = disk.bound_export();
    let bytes = proof.proof.canonical_bytes().to_vec();
    let owner = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES).unwrap();
    let reached = Cell::new(false);
    assert!(
        owner
            .publish_with_hook(proof, |phase| {
                if phase == Phase::AfterRename {
                    reached.set(true);
                    let mut file = fs::OpenOptions::new()
                        .append(true)
                        .open(disk.root.join("blocks.data"))?;
                    file.write_all(b"changed")?;
                }
                Ok(())
            })
            .is_err()
    );
    assert!(reached.get());
    assert_eq!(fs::read(disk.files.output()).unwrap(), bytes);
}

#[test]
fn core_completion_rejects_unbounded_publication_identity_lists() {
    let disk = Disk::new(1);
    let proof = disk.bound_export();
    let complete = proof.proof.disk_completion.as_ref().unwrap();
    complete.recheck_sources().unwrap();
    assert!(complete.ensure_publication_ancestry(&[]).is_err());
    assert!(
        complete
            .ensure_publication_ancestry(&vec![(0, 0); 65])
            .is_err()
    );
    assert!(
        complete
            .ensure_publication_ancestry(&[(
                fs::metadata(&disk.root).unwrap().dev(),
                fs::metadata(&disk.root).unwrap().ino()
            )])
            .is_err()
    );
}

#[test]
fn supplied_evidence_bundle_declares_v1_identity_for_exact_finality_and_queries() {
    let fixture = fixture::Fixture::new(1);
    let bundle = SuppliedEvidenceBundleV1 {
        version: 1,
        heights: vec![
            SuppliedEvidenceHeightV1 {
                height: 1,
                finality: norito::encode_canonical(&fixture.first).unwrap(),
                queries: vec![],
            },
            SuppliedEvidenceHeightV1 {
                height: 2,
                finality: norito::encode_canonical(&fixture.second).unwrap(),
                queries: fixture.queries(),
            },
        ],
    };
    let decoded = crate::kura::scaling_evidence::tests::assert_declared_scaling_frame::<
        SuppliedEvidenceBundleV1,
        AuthenticatedRequest,
    >(
        &bundle,
        "iroha_kagami::scaling_evidence::SuppliedEvidenceBundleV1",
        [
            124, 223, 61, 193, 181, 188, 0, 216, 188, 208, 198, 107, 159, 182, 52, 38,
        ],
    );
    assert_eq!(decoded.version, 1);
    assert_eq!(decoded.heights.len(), 2);
    assert_eq!(decoded.heights[0].height, 1);
    assert_eq!(decoded.heights[1].height, 2);
    assert_eq!(decoded.heights[0].finality, bundle.heights[0].finality);
    assert_eq!(decoded.heights[1].queries, bundle.heights[1].queries);
}
