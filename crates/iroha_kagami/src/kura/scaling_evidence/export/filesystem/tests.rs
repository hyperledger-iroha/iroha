//! Actual retained descriptor and signed canonical adapter controls.

// Actual descriptor and signed-adapter controls; no global cwd/hooks or child processes.
use super::*;
use crate::kura::scaling_evidence::fixture;
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
    request_sequence: Cell<u64>,
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
            request_sequence: Cell::new(0),
        }
    }
    fn launcher_file_with(
        &self,
        plan: TrustedRunPlan,
        limits: VerificationLimits,
        bindings: &[HeightInputBinding],
    ) -> ProofInputBinding {
        let bytes = crate::kura::scaling_evidence::export::launcher::encode(
            plan,
            limits,
            bindings
                .iter()
                .map(|b| HeightInputBinding {
                    height: b.height,
                    finality_hash: b.finality_hash,
                    query_hashes: b.query_hashes.clone(),
                })
                .collect(),
            MAX_INPUT_BYTES,
        )
        .unwrap();
        let sequence = self.request_sequence.get();
        self.request_sequence.set(sequence + 1);
        // Separate from the evidence directory census and from every earlier
        // still-retained request. No proof's authority file is rewritten here.
        let path = self
            .files
            .ancestor
            .join(format!("launcher-{sequence}.norito"));
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        binding(&path)
    }
    fn launcher_file(&self) -> ProofInputBinding {
        self.launcher_file_with(self.signed.plan(), self.limits(), &self.bindings())
    }
    fn request_with(
        &self,
        plan: TrustedRunPlan,
        limits: VerificationLimits,
        bindings: &[HeightInputBinding],
    ) -> RetainedLauncherRequest {
        open_launcher(self.launcher_file_with(plan, limits, bindings)).unwrap()
    }
    fn request(&self) -> RetainedLauncherRequest {
        open_launcher(self.launcher_file()).unwrap()
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
        replay_bound_request(
            self.request_with(self.signed.plan(), self.limits(), &self.bindings()),
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
        export_bound_request(
            self.request_with(self.signed.plan(), self.limits(), &self.bindings()),
            &self.root,
            &self.log,
            self.reader_limits(),
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
        let retained = export_bound_request(
            disk.request_with(disk.signed.plan(), fixture::limits(), &disk.bindings()),
            &disk.root,
            &disk.log,
            disk.reader_limits(),
            disk.bundle_file(),
        )
        .unwrap();
        assert_eq!(retained.proof.canonical_bytes(), expected.canonical_bytes());
        assert_eq!(
            retained.json_projection(MAX_INPUT_BYTES).unwrap(),
            expected.json_projection(MAX_INPUT_BYTES).unwrap()
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
    assert!(!old.json_projection(MAX_INPUT_BYTES).unwrap().is_empty());
    assert!(!current.json_projection(MAX_INPUT_BYTES).unwrap().is_empty());
    assert!(
        replay_bound_request(
            four.request_with(four.signed.plan(), fixture::limits(), &four.bindings()),
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
    assert!(
        !completed
            .json_projection(MAX_INPUT_BYTES)
            .unwrap()
            .is_empty()
    );
    let mut bad = disk.files.binding();
    bad.sha256[0] ^= 1;
    assert!(
        replay_bound_request(
            disk.request_with(disk.signed.plan(), fixture::limits(), &disk.bindings()),
            Hash::new(completed.proof.canonical_bytes()),
            bad
        )
        .is_err()
    );
    assert!(
        replay_bound_request(
            disk.request_with(disk.signed.plan(), fixture::limits(), &disk.bindings()),
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
            disk.request_with(disk.signed.plan(), fixture::limits(), &disk.bindings()),
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
            disk.request_with(disk.signed.plan(), fixture::limits(), &disk.bindings()),
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
    let positive = export_bound_request(
        disk.request_with(disk.signed.plan(), fixture::limits(), &disk.bindings()),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
        disk.bundle_file(),
    )
    .unwrap();
    assert_eq!(positive.proof.canonical_bytes(), expected.canonical_bytes());
    let mut bundle = disk.bundle();
    bundle.heights[0].height = 99;
    let path = disk.bundle_file().path;
    fs::write(&path, norito::encode_canonical(&bundle).unwrap()).unwrap();
    let result = export_bound_request(
        disk.request_with(disk.signed.plan(), fixture::limits(), &disk.bindings()),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
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
    assert!(
        !published
            .json_projection(MAX_INPUT_BYTES)
            .unwrap()
            .is_empty()
    );
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
    let retained = export_bound_request(
        disk.request_with(disk.signed.plan(), disk.limits(), &disk.bindings()),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
        input,
    )
    .unwrap();
    assert_eq!(retained.proof.rows().len(), 128);
    assert_eq!(retained.proof.canonical_bytes(), expected.canonical_bytes());
    assert_eq!(
        retained.json_projection(MAX_INPUT_BYTES).unwrap(),
        expected.json_projection(MAX_INPUT_BYTES).unwrap()
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
    let positive = export_bound_request(
        disk.request_with(disk.signed.plan(), disk.limits(), &disk.bindings()),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
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
            export_bound_request(
                disk.request_with(disk.signed.plan(), disk.limits(), &disk.bindings()),
                &disk.root,
                &disk.log,
                disk.reader_limits(),
                binding(&path)
            )
            .is_err()
        );
    }
}

#[test]
fn supplied_bundle_rehash_cannot_change_complete_height_or_leaf_roles() {
    let disk = Disk::new(1);
    let positive = export_bound_request(
        disk.request_with(disk.signed.plan(), disk.limits(), &disk.bindings()),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
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
            export_bound_request(
                disk.request_with(disk.signed.plan(), disk.limits(), &disk.bindings()),
                &disk.root,
                &disk.log,
                disk.reader_limits(),
                binding(&path)
            )
            .is_err()
        );
    }
}

#[test]
fn supplied_bundle_one_byte_under_admitted_size_rejects_after_actual_positive() {
    let disk = Disk::new(1);
    let positive = export_bound_request(
        disk.request_with(disk.signed.plan(), disk.limits(), &disk.bindings()),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
        disk.bundle_file(),
    )
    .unwrap();
    assert_eq!(positive.proof.rows().len(), 8);
    let mut input = disk.bundle_file();
    let exact = input.max_bytes;
    input.max_bytes -= 1;
    let failure = export_bound_request(
        disk.request_with(disk.signed.plan(), disk.limits(), &disk.bindings()),
        &disk.root,
        &disk.log,
        disk.reader_limits(),
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
    assert!(
        !published
            .json_projection(MAX_INPUT_BYTES)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn output_equal_to_or_below_actual_core_store_is_rejected_before_stage_creation() {
    let disk = Disk::new(1);
    let nested = disk.root.join("nested");
    fs::create_dir(&nested).unwrap();
    for parent in [&disk.root, &nested] {
        let proof = disk.bound_export();
        assert!(!proof.json_projection(MAX_INPUT_BYTES).unwrap().is_empty());
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
    assert!(!proof.json_projection(MAX_INPUT_BYTES).unwrap().is_empty());
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
    assert!(
        !baseline
            .json_projection(MAX_INPUT_BYTES)
            .unwrap()
            .is_empty()
    );
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
    assert!(!proof.json_projection(MAX_INPUT_BYTES).unwrap().is_empty());
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
    assert!(!proof.json_projection(MAX_INPUT_BYTES).unwrap().is_empty());
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
    assert!(!proof.json_projection(MAX_INPUT_BYTES).unwrap().is_empty());
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

// Retained launcher authority controls. These use the exact signed fixtures and
// consuming filesystem owners; no test callback can manufacture a completion.
fn change_request_bytes(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[0] ^= 1;
    fs::write(path, bytes).unwrap();
}
fn request_path(proof: &RetainedProof) -> PathBuf {
    assert_eq!(proof.input_lease.files.len(), 2);
    proof
        .input_lease
        .files
        .iter()
        .find(|file| {
            file.path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("launcher-")
        })
        .unwrap()
        .path
        .clone()
}

#[test]
fn retained_launcher_has_real_descriptors_and_no_shared_read_offset() {
    for lanes in [1, 4] {
        let disk = Disk::new(lanes);
        let bound = disk.launcher_file();
        let expected = fs::read(&bound.path).unwrap();
        let request = open_launcher(bound).unwrap();
        assert_eq!(request.input_lease.files.len(), 1);
        let input = &request.input_lease.files[0];
        assert_eq!(input.digest, iroha_crypto::sha256(&expected));
        assert!(
            rustix::fs::fcntl_getfl(&input.file)
                .unwrap()
                .contains(OFlags::NONBLOCK)
        );
        assert!(
            rustix::io::fcntl_getfd(&input.file)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        let mut file = &input.file;
        file.seek(SeekFrom::Start(3)).unwrap();
        request.input_lease.check().unwrap();
        assert_eq!(file.stream_position().unwrap(), 3);
        let mut first = ContentReader {
            file: &input.file,
            offset: 0,
        };
        let mut second = ContentReader {
            file: &input.file,
            offset: 0,
        };
        let mut a = [0; 7];
        let mut b = [0; 7];
        first.read_exact(&mut a).unwrap();
        second.read_exact(&mut b).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.as_slice(), &expected[..7]);
        assert_eq!(file.stream_position().unwrap(), 3);
        let proof = export_bound_request(
            request,
            &disk.root,
            &disk.log,
            disk.reader_limits(),
            disk.bundle_file(),
        )
        .unwrap();
        assert_eq!(proof.input_lease.files.len(), 2);
        assert_eq!(
            proof.proof.canonical_bytes(),
            disk.export().canonical_bytes()
        );
        assert!(!proof.json_projection(MAX_INPUT_BYTES).unwrap().is_empty());
    }
}

#[test]
fn retained_lease_rechecks_digest_even_when_held_metadata_matches() {
    let disk = Disk::new(1);
    let mut request = disk.request();
    request.input_lease.check().unwrap();
    let input = &mut request.input_lease.files[0];
    // A wrong independent digest with completely unchanged held/named metadata
    // must fail the reusable check, not merely admission's original read.
    input.digest[0] ^= 1;
    input.check().unwrap();
    assert!(
        request
            .input_lease
            .check()
            .err()
            .unwrap()
            .to_string()
            .contains("digest mismatch")
    );
}

#[test]
fn launcher_raw_pin_cap_and_canonical_frame_are_mandatory() {
    let disk = Disk::new(1);
    let mut wrong_pin = disk.launcher_file();
    wrong_pin.sha256[0] ^= 1;
    assert!(open_launcher(wrong_pin).is_err());
    let mut too_small = disk.launcher_file();
    too_small.max_bytes -= 1;
    assert!(open_launcher(too_small).is_err());
    let canonical = fs::read(disk.launcher_file().path).unwrap();
    let mut trailing = canonical.clone();
    trailing.push(0);
    for bytes in [
        trailing,
        canonical[..canonical.len() - 1].to_vec(),
        norito::encode_canonical(&disk.bundle()).unwrap(),
    ] {
        let path = disk.launcher_file().path;
        fs::write(&path, bytes).unwrap();
        // Rehash the complete actual mutant: the raw pin cannot make an invalid
        // canonical request become an admitted plan.
        assert!(open_launcher(binding(&path)).is_err());
    }
}

#[test]
fn request_mutation_at_every_admission_and_decode_boundary_fails() {
    for target in [
        Phase::ParentAdmitted,
        Phase::ParentRetained,
        Phase::InputAdmitted,
        Phase::InputRetained,
        Phase::BeforeRead,
        Phase::AfterRead,
        Phase::BeforeRequestDecode,
        Phase::AfterRequestDecode,
        Phase::BeforeInputFinish,
    ] {
        let disk = Disk::new(1);
        let bound = disk.launcher_file();
        let path = bound.path.clone();
        let reached = Cell::new(false);
        let result = open_launcher_with_hook(bound, |phase| {
            if phase == target && !reached.replace(true) {
                change_request_bytes(&path);
            }
            Ok(())
        });
        assert!(reached.get(), "unreached request phase {target:?}");
        assert!(result.is_err(), "accepted request mutation at {target:?}");
    }
}

#[test]
fn retained_request_replacement_symlink_and_ancestor_change_fail() {
    for kind in 0..3 {
        let disk = Disk::new(1);
        let bound = disk.launcher_file();
        let path = bound.path.clone();
        let bytes = fs::read(&path).unwrap();
        let reached = Cell::new(false);
        let result = open_launcher_with_hook(bound, |phase| {
            if phase == Phase::InputRetained {
                reached.set(true);
                if kind == 2 {
                    disk.files.replace_parent(true);
                } else {
                    let old = path.with_extension("old");
                    fs::rename(&path, &old)?;
                    if kind == 0 {
                        fs::write(&path, &bytes)?;
                    } else {
                        symlink(&old, &path)?;
                    }
                }
            }
            Ok(())
        });
        assert!(reached.get());
        assert!(result.is_err());
    }
}

#[test]
fn export_and_replay_recheck_request_through_input_and_verification() {
    for replay in [false, true] {
        for target in [
            Phase::ParentAdmitted,
            Phase::ParentRetained,
            Phase::InputAdmitted,
            Phase::InputRetained,
            Phase::BeforeRead,
            Phase::AfterRead,
            Phase::BeforeVerification,
            Phase::AfterVerification,
            Phase::BeforeInputFinish,
        ] {
            let disk = Disk::new(1);
            let bound = disk.launcher_file();
            let path = bound.path.clone();
            let request = open_launcher(bound).unwrap();
            let expected = disk.export();
            fs::write(&disk.files.input, expected.canonical_bytes()).unwrap();
            let reached = Cell::new(false);
            let hook = |phase| {
                if phase == target && !reached.replace(true) {
                    change_request_bytes(&path);
                }
                Ok(())
            };
            let result = if replay {
                replay_with_hook(
                    request,
                    Hash::new(expected.canonical_bytes()),
                    disk.files.binding(),
                    hook,
                )
            } else {
                export_with_hook(
                    request,
                    &disk.root,
                    &disk.log,
                    disk.reader_limits(),
                    disk.bundle_file(),
                    hook,
                )
            };
            assert!(
                reached.get(),
                "unreached request phase {target:?}, replay={replay}"
            );
            assert!(
                result.is_err(),
                "accepted request mutation at {target:?}, replay={replay}"
            );
            assert!(!disk.files.output().exists());
        }
    }
}

#[test]
fn aggregate_request_reservation_exact_boundary_and_overflow_precede_second_open() {
    for replay in [false, true] {
        for excess in [0, 1] {
            let disk = Disk::new(1);
            let expected = disk.export();
            fs::write(&disk.files.input, expected.canonical_bytes()).unwrap();
            let input = if replay {
                disk.files.binding()
            } else {
                disk.bundle_file()
            };
            let mut bound = disk.launcher_file();
            let core = if replay {
                0
            } else {
                disk.reader_limits().max_output_bytes
            };
            let reservation = remaining_input(&disk.signed.plan(), disk.limits()).unwrap()
                - core
                - input.max_bytes;
            assert!(reservation >= bound.max_bytes);
            bound.max_bytes = reservation + excess;
            let request = open_launcher(bound).unwrap();
            let reached = Cell::new(false);
            let hook = |_| {
                reached.set(true);
                Ok(())
            };
            let result = if replay {
                replay_with_hook(request, Hash::new(expected.canonical_bytes()), input, hook)
            } else {
                export_with_hook(
                    request,
                    &disk.root,
                    &disk.log,
                    disk.reader_limits(),
                    input,
                    hook,
                )
            };
            if excess == 0 {
                assert!(reached.get());
                assert_eq!(
                    result.unwrap().proof.canonical_bytes(),
                    expected.canonical_bytes()
                );
            } else {
                assert!(
                    !reached.get(),
                    "reservation rejected after opening second input"
                );
                assert!(result.is_err());
            }
        }
    }
}

#[test]
fn duplicate_request_path_and_inode_cannot_be_read_as_evidence() {
    for hardlink in [false, true] {
        for replay in [false, true] {
            let disk = Disk::new(1);
            let bound = disk.launcher_file();
            let path = bound.path.clone();
            let request = open_launcher(bound).unwrap();
            let other = if hardlink {
                let alias = disk.files.parent.join("request-alias");
                fs::hard_link(&path, &alias).unwrap();
                alias
            } else {
                path
            };
            let read = Cell::new(false);
            let hook = |phase| {
                if phase == Phase::BeforeRead {
                    read.set(true);
                }
                Ok(())
            };
            let result = if replay {
                replay_with_hook(request, Hash::new(b"unused proof"), binding(&other), hook)
            } else {
                export_with_hook(
                    request,
                    &disk.root,
                    &disk.log,
                    disk.reader_limits(),
                    binding(&other),
                    hook,
                )
            };
            assert!(result.is_err());
            assert!(!read.get());
            assert!(!disk.files.output().exists());
        }
    }
}

#[test]
fn request_mutation_at_every_publication_and_sync_boundary_has_no_receipt() {
    for target in [
        Phase::BeforeCreate,
        Phase::AfterCreate,
        Phase::BeforeDirectorySync,
        Phase::AfterDirectorySync,
        Phase::BeforeFileSync,
        Phase::AfterFileSync,
        Phase::BeforeRename,
        Phase::RenameReady,
        Phase::AfterRename,
    ] {
        // Directory sync occurs both for stage admission and after final rename.
        let occurrences = if matches!(
            target,
            Phase::BeforeDirectorySync | Phase::AfterDirectorySync
        ) {
            2
        } else {
            1
        };
        for occurrence in 1..=occurrences {
            let disk = Disk::new(1);
            let proof = disk.bound_export();
            let path = request_path(&proof);
            let owner = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES).unwrap();
            let count = Cell::new(0);
            let reached = Cell::new(false);
            let result = owner.publish_with_hook(proof, |phase| {
                if phase == target {
                    count.set(count.get() + 1);
                    if count.get() == occurrence {
                        reached.set(true);
                        change_request_bytes(&path);
                    }
                }
                Ok(())
            });
            assert!(
                reached.get(),
                "unreached publication phase {target:?}/{occurrence}"
            );
            assert!(
                result.is_err(),
                "accepted publication mutation {target:?}/{occurrence}"
            );
        }
    }
}

#[test]
fn request_output_alias_and_request_ancestor_drift_cannot_publish() {
    let disk = Disk::new(1);
    let proof = disk.bound_export();
    let path = request_path(&proof);
    let original = fs::read(&path).unwrap();
    assert!(ProofOutput::admit(&path, MAX_INPUT_BYTES).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    let owner = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES).unwrap();
    let reached = Cell::new(false);
    assert!(
        owner
            .publish_with_hook(proof, |phase| {
                if phase == Phase::BeforeFileSync {
                    reached.set(true);
                    disk.files.replace_parent(true);
                }
                Ok(())
            })
            .is_err()
    );
    assert!(reached.get());
}

#[test]
fn retained_request_remains_authoritative_through_final_projection() {
    for target in [Phase::BeforeProjection, Phase::AfterProjection] {
        let disk = Disk::new(1);
        let proof = disk.bound_export();
        let path = request_path(&proof);
        let published = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES)
            .unwrap()
            .publish(proof)
            .unwrap();
        assert!(
            !published
                .json_projection(MAX_INPUT_BYTES)
                .unwrap()
                .is_empty()
        );
        assert_eq!(published.proof.input_lease.files.len(), 2);
        let reached = Cell::new(false);
        assert!(
            published
                .proof
                .json_projection_with_hook(MAX_INPUT_BYTES, |phase| {
                    if phase == target {
                        reached.set(true);
                        change_request_bytes(&path);
                    }
                    Ok(())
                })
                .is_err()
        );
        assert!(reached.get());
        assert!(published.json_projection(MAX_INPUT_BYTES).is_err());
        assert!(disk.files.output().is_file());
    }
}

#[test]
fn caught_request_verification_and_publication_panics_return_no_success() {
    for publishing in [false, true] {
        let disk = Disk::new(1);
        let reached = Cell::new(false);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            if publishing {
                let proof = disk.bound_export();
                let path = request_path(&proof);
                ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES)
                    .unwrap()
                    .publish_with_hook(proof, |phase| {
                        if phase == Phase::AfterRename {
                            reached.set(true);
                            change_request_bytes(&path);
                            panic!("request changed after rename");
                        }
                        Ok(())
                    })
                    .map(|_| ())
            } else {
                let bound = disk.launcher_file();
                let path = bound.path.clone();
                let request = open_launcher(bound).unwrap();
                export_with_hook(
                    request,
                    &disk.root,
                    &disk.log,
                    disk.reader_limits(),
                    disk.bundle_file(),
                    |phase| {
                        if phase == Phase::AfterVerification {
                            reached.set(true);
                            change_request_bytes(&path);
                            panic!("request changed after verification");
                        }
                        Ok(())
                    },
                )
                .map(|_| ())
            }
        }));
        assert!(reached.get());
        assert!(outcome.is_err());
    }
}

#[test]
fn retained_and_published_identity_hash_actual_canonical_bytes_independently() {
    for lanes in [1, 4] {
        let disk = Disk::new(lanes);
        let proof = disk.bound_export();
        let expected = CanonicalProofIdentity {
            raw_sha256: iroha_crypto::sha256(proof.proof.canonical_bytes()),
            iroha_hash: Hash::new(proof.proof.canonical_bytes()),
            byte_length: proof.proof.canonical_bytes().len() as u64,
        };
        assert_eq!(proof.identity().unwrap(), expected);
        let published = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES)
            .unwrap()
            .publish(proof)
            .unwrap();
        assert_eq!(published.identity().unwrap(), expected);
        assert_eq!(published.sha256(), expected.raw_sha256);
        assert_eq!(published.byte_length(), expected.byte_length);
        let replay = disk.retain();
        assert_eq!(replay.identity().unwrap(), expected);
        let mut changed_receipt = published;
        changed_receipt.sha256[0] ^= 1;
        assert!(changed_receipt.identity().is_err());
        changed_receipt.sha256[0] ^= 1;
        changed_receipt.byte_length += 1;
        assert!(changed_receipt.identity().is_err());
    }
}

#[test]
fn canonical_identity_rejects_late_request_evidence_and_core_changes() {
    for source in 0..3 {
        for target in [Phase::BeforeIdentity, Phase::AfterIdentity] {
            let disk = Disk::new(1);
            let proof = disk.bound_export();
            let path = match source {
                0 => request_path(&proof),
                1 => disk.files.parent.join("supplied.norito"),
                _ => disk.root.join("blocks.data"),
            };
            let published = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES)
                .unwrap()
                .publish(proof)
                .unwrap();
            assert!(published.identity().is_ok());
            let reached = Cell::new(false);
            assert!(
                published
                    .proof
                    .identity_with_hook(|phase| {
                        if phase == target {
                            reached.set(true);
                            change_request_bytes(&path);
                        }
                        Ok(())
                    })
                    .is_err()
            );
            assert!(reached.get());
            assert!(published.identity().is_err());
            assert!(published.json_projection(MAX_INPUT_BYTES).is_err());
        }
    }
}

// Mandatory independent projection caps retain the same original proof authority.
#[test]
fn retained_and_published_projection_caps_preserve_complete_proof_identity() {
    for lanes in [1, 4] {
        let disk = Disk::new(lanes);
        let proof = disk.bound_export();
        let identity = proof.identity().unwrap();
        let complete = proof.json_projection(MAX_INPUT_BYTES).unwrap();
        let exact = complete.len() as u64;
        assert_eq!(proof.json_projection(exact).unwrap(), complete);
        for maximum in [0, 1, exact - 1, MAX_INPUT_BYTES + 1, u64::MAX] {
            assert!(proof.json_projection(maximum).is_err());
            assert_eq!(proof.identity().unwrap(), identity);
        }
        let published = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES)
            .unwrap()
            .publish(proof)
            .unwrap();
        assert_eq!(published.identity().unwrap(), identity);
        assert_eq!(published.json_projection(exact).unwrap(), complete);
        let original = fs::read(disk.files.output()).unwrap();
        for maximum in [0, 1, exact - 1, MAX_INPUT_BYTES + 1, u64::MAX] {
            assert!(published.json_projection(maximum).is_err());
            assert_eq!(published.identity().unwrap(), identity);
            assert_eq!(fs::read(disk.files.output()).unwrap(), original);
        }
        assert_eq!(published.json_projection(exact).unwrap(), complete);
        let decoded: norito::json::Value = norito::json::from_slice(&complete).unwrap();
        assert_eq!(decoded.as_array().unwrap().len(), 8);
    }
}

#[test]
fn bounded_projection_checks_request_evidence_and_core_before_and_after_derivation() {
    for source in 0..3 {
        for target in [Phase::BeforeProjection, Phase::AfterProjection] {
            let disk = Disk::new(1);
            let proof = disk.bound_export();
            let path = match source {
                0 => request_path(&proof),
                1 => disk.files.parent.join("supplied.norito"),
                _ => disk.root.join("blocks.data"),
            };
            let published = ProofOutput::admit(&disk.files.output(), MAX_INPUT_BYTES)
                .unwrap()
                .publish(proof)
                .unwrap();
            let complete = published.json_projection(MAX_INPUT_BYTES).unwrap();
            let exact = complete.len() as u64;
            assert_eq!(published.json_projection(exact).unwrap(), complete);
            let original_output = fs::read(disk.files.output()).unwrap();
            let reached = Cell::new(false);
            let result = published.proof.json_projection_with_hook(exact, |phase| {
                if phase == target {
                    reached.set(true);
                    change_request_bytes(&path);
                }
                Ok(())
            });
            assert!(reached.get(), "unreached source {source} phase {target:?}");
            assert!(result.is_err(), "accepted source {source} phase {target:?}");
            assert!(published.identity().is_err());
            assert!(published.json_projection(exact).is_err());
            assert_eq!(fs::read(disk.files.output()).unwrap(), original_output);
        }
    }
}

#[test]
fn bounded_replay_projection_keeps_retained_identity_after_rejected_output_size() {
    for lanes in [1, 4] {
        let disk = Disk::new(lanes);
        let proof = disk.retain();
        let identity = proof.identity().unwrap();
        let complete = proof.json_projection(MAX_INPUT_BYTES).unwrap();
        let exact = complete.len() as u64;
        assert!(proof.json_projection(exact - 1).is_err());
        assert_eq!(proof.identity().unwrap(), identity);
        assert_eq!(proof.json_projection(exact).unwrap(), complete);
        let path = request_path(&proof);
        change_request_bytes(&path);
        assert!(proof.identity().is_err());
        assert!(proof.json_projection(exact).is_err());
    }
}

// Actual command dispatch controls use the same independently signed four-validator
// transcript and retained files as the filesystem owner. No fake proof receipt or
// command-level callback substitutes for canonical authentication.
fn scaling_command_arguments(
    disk: &Disk,
    operation: &str,
    request: &ProofInputBinding,
    input: &ProofInputBinding,
) -> Vec<String> {
    let mut args: Vec<String> = ["kagami", "advanced", "kura", "scaling-evidence", operation]
        .into_iter()
        .map(str::to_owned)
        .collect();
    for (flag, value) in [
        ("--invocation-id", "01".repeat(32)),
        ("--request", request.path.to_str().unwrap().to_owned()),
        ("--request-sha256", hex::encode(request.sha256)),
        ("--request-max-bytes", request.max_bytes.to_string()),
        ("--input", input.path.to_str().unwrap().to_owned()),
        ("--input-sha256", hex::encode(input.sha256)),
        ("--input-max-bytes", input.max_bytes.to_string()),
        ("--reply-max-bytes", (2 * 1024 * 1024).to_string()),
    ] {
        args.extend([flag.to_owned(), value]);
    }
    if operation == "export" {
        let limits = disk.reader_limits();
        for (flag, value) in [
            ("--block-store", disk.root.to_str().unwrap().to_owned()),
            ("--merge-log", disk.log.to_str().unwrap().to_owned()),
            ("--output", disk.files.output().to_str().unwrap().to_owned()),
            ("--output-max-bytes", (2 * 1024 * 1024).to_string()),
            ("--first-height", limits.first_height.to_string()),
            ("--last-height", limits.last_height.to_string()),
            (
                "--max-committed-blocks",
                limits.max_committed_blocks.to_string(),
            ),
            (
                "--max-store-data-bytes",
                limits.max_store_data_bytes.to_string(),
            ),
            ("--max-carrier-bytes", limits.max_carrier_bytes.to_string()),
            (
                "--max-merge-log-bytes",
                limits.max_merge_log_bytes.to_string(),
            ),
            ("--max-merge-frames", limits.max_merge_frames.to_string()),
            (
                "--reader-max-output-bytes",
                limits.max_output_bytes.to_string(),
            ),
            (
                "--max-decode-allocation-bytes",
                limits.max_decode_allocation_bytes.to_string(),
            ),
            ("--owner-uid", limits.owner_uid.to_string()),
        ] {
            args.extend([flag.to_owned(), value]);
        }
    } else {
        args.extend([
            "--proof-iroha-hash".to_owned(),
            Hash::new(fs::read(&input.path).unwrap()).to_string(),
        ]);
    }
    args
}

#[test]
fn scaling_command_exports_and_replays_complete_one_and_four_lane_proofs() {
    use crate::RunArgs as _;
    use clap::Parser as _;
    for lanes in [1, 4] {
        let disk = Disk::new(lanes);
        let request = disk.launcher_file();
        let supplied = disk.bundle_file();
        let export_args = scaling_command_arguments(&disk, "export", &request, &supplied);
        let cli = crate::Cli::try_parse_from(export_args.clone()).unwrap();
        let mut writer = std::io::BufWriter::new(Vec::new());
        cli.command.run(&mut writer).unwrap();
        let exported: norito::json::Value =
            norito::json::from_slice(&writer.into_inner().unwrap()).unwrap();
        let raw = fs::read(disk.files.output()).unwrap();
        assert_eq!(exported.as_object().unwrap().len(), 8);
        assert_eq!(exported["operation"].as_str(), Some("export"));
        assert_eq!(
            exported["request_sha256"].as_str(),
            Some(hex::encode(request.sha256).as_str())
        );
        assert_eq!(
            exported["input_sha256"].as_str(),
            Some(hex::encode(supplied.sha256).as_str())
        );
        assert_eq!(
            exported["proof_sha256"].as_str(),
            Some(hex::encode(iroha_crypto::sha256(&raw)).as_str())
        );
        assert_eq!(
            exported["proof_iroha_hash"].as_str(),
            Some(Hash::new(&raw).to_string().as_str())
        );
        assert_eq!(exported["proof_bytes"].as_u64(), Some(raw.len() as u64));
        assert_eq!(
            fs::metadata(disk.files.output()).unwrap().mode() & 0o7777,
            0o600
        );
        assert!(!disk.files.stage().exists());
        // An existing proof destination is not overwritten by another export.
        let cli = crate::Cli::try_parse_from(export_args).unwrap();
        let mut writer = std::io::BufWriter::new(Vec::new());
        assert!(cli.command.run(&mut writer).is_err());
        assert!(writer.into_inner().unwrap().is_empty());
        assert_eq!(fs::read(disk.files.output()).unwrap(), raw);
        let input = binding(&disk.files.output());
        let cli = crate::Cli::try_parse_from(scaling_command_arguments(
            &disk, "replay", &request, &input,
        ))
        .unwrap();
        let mut writer = std::io::BufWriter::new(Vec::new());
        cli.command.run(&mut writer).unwrap();
        let replayed: norito::json::Value =
            norito::json::from_slice(&writer.into_inner().unwrap()).unwrap();
        assert_eq!(replayed.as_object().unwrap().len(), 9);
        assert_eq!(replayed["operation"].as_str(), Some("replay"));
        assert_eq!(replayed["proof_sha256"], exported["proof_sha256"]);
        assert_eq!(replayed["proof_iroha_hash"], exported["proof_iroha_hash"]);
        assert_eq!(replayed["proof_bytes"], exported["proof_bytes"]);
        let independent: norito::json::Value =
            norito::json::from_slice(&disk.export().json_projection(MAX_INPUT_BYTES).unwrap())
                .unwrap();
        assert_eq!(replayed["rows"], independent);
        assert_eq!(
            replayed["rows"].as_array().unwrap().len(),
            disk.signed.requests.len()
        );
    }
}

#[test]
fn scaling_command_requires_each_pin_and_bound_and_rejects_legacy_shapes() {
    use clap::Parser as _;
    let disk = Disk::new(1);
    let request = disk.launcher_file();
    let supplied = disk.bundle_file();
    let args = scaling_command_arguments(&disk, "export", &request, &supplied);
    assert!(crate::Cli::try_parse_from(args.clone()).is_ok());
    for index in (5..args.len()).step_by(2) {
        let mut missing = args.clone();
        missing.drain(index..index + 2);
        assert!(
            crate::Cli::try_parse_from(missing).is_err(),
            "missing {} accepted",
            args[index]
        );
    }
    for flag in [
        "--request-max-bytes",
        "--input-max-bytes",
        "--reply-max-bytes",
        "--output-max-bytes",
    ] {
        let position = args.iter().position(|value| value == flag).unwrap();
        for invalid in ["0", "268435457", "18446744073709551615"] {
            let mut changed = args.clone();
            changed[position + 1] = invalid.to_owned();
            assert!(crate::Cli::try_parse_from(changed).is_err());
        }
    }
    for old in [
        vec!["kagami", "advanced", "kura", "./store", "print"],
        vec![
            "kagami", "advanced", "kura", "./store", "sidecar", "-H", "1",
        ],
        vec!["kagami", "kura", "scaling-evidence", "replay"],
    ] {
        assert!(crate::Cli::try_parse_from(old).is_err());
    }
    assert!(
        crate::Cli::try_parse_from(["kagami", "advanced", "kura", "print", "./store", "-f", "1"])
            .is_ok()
    );
    assert!(
        crate::Cli::try_parse_from([
            "kagami", "advanced", "kura", "sidecar", "./store", "-H", "1"
        ])
        .is_ok()
    );
}

#[test]
fn scaling_command_replay_rejects_independent_digest_drift_without_reply() {
    use crate::RunArgs as _;
    use clap::Parser as _;
    let disk = Disk::new(1);
    let exported = disk.export();
    fs::write(&disk.files.input, exported.canonical_bytes()).unwrap();
    let request = disk.launcher_file();
    let input = disk.files.binding();
    let args = scaling_command_arguments(&disk, "replay", &request, &input);
    for flag in ["--request-sha256", "--input-sha256", "--proof-iroha-hash"] {
        let mut changed = args.clone();
        let position = changed.iter().position(|value| value == flag).unwrap();
        changed[position + 1] = "01".repeat(32);
        let cli = crate::Cli::try_parse_from(changed).unwrap();
        let mut writer = std::io::BufWriter::new(Vec::new());
        assert!(
            cli.command.run(&mut writer).is_err(),
            "accepted wrong {flag}"
        );
        assert!(writer.into_inner().unwrap().is_empty());
    }
    let mut short = args;
    let position = short
        .iter()
        .position(|value| value == "--reply-max-bytes")
        .unwrap();
    short[position + 1] = "1".to_owned();
    let cli = crate::Cli::try_parse_from(short).unwrap();
    let mut writer = std::io::BufWriter::new(Vec::new());
    assert!(cli.command.run(&mut writer).is_err());
    assert!(writer.into_inner().unwrap().is_empty());
}

#[test]
fn scaling_command_keeps_original_inputs_through_actual_reply_flush() {
    use crate::RunArgs as _;
    use clap::Parser as _;
    struct MutateOnFlush {
        path: PathBuf,
        bytes: Vec<u8>,
    }
    impl std::io::Write for MutateOnFlush {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            let mut bytes = fs::read(&self.path)?;
            bytes[0] ^= 1;
            fs::write(&self.path, bytes)
        }
    }
    for operation in ["export", "replay"] {
        for change_request in [true, false] {
            let disk = Disk::new(1);
            let request = disk.launcher_file();
            let input = if operation == "export" {
                disk.bundle_file()
            } else {
                fs::write(&disk.files.input, disk.export().canonical_bytes()).unwrap();
                disk.files.binding()
            };
            let changed_path = if change_request {
                request.path.clone()
            } else {
                input.path.clone()
            };
            let cli = crate::Cli::try_parse_from(scaling_command_arguments(
                &disk, operation, &request, &input,
            ))
            .unwrap();
            let mut writer = std::io::BufWriter::new(MutateOnFlush {
                path: changed_path,
                bytes: Vec::new(),
            });
            assert!(cli.command.run(&mut writer).is_err());
            // Even a syntactically complete reply cannot turn this late failure
            // into success: the parent must require the terminal exit status.
            let reply: norito::json::Value =
                norito::json::from_slice(&writer.get_ref().bytes).unwrap();
            assert_eq!(reply["operation"].as_str(), Some(operation));
        }
    }
}

#[path = "prepare_pair_tests.rs"]
mod prepare_pair_tests;
