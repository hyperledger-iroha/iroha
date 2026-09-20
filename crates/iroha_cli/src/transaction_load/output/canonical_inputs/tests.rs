// Real descriptors and public Core reader completion; no synthetic completion capability.
use super::*;
use iroha_core::{
    block::BlockBuilder,
    kura::{BlockStore, CanonicalKuraEvidenceLimits, CanonicalKuraEvidenceReader},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::block::SignedBlock;
use std::{
    fs,
    io::Write as _,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _, symlink},
    panic::{AssertUnwindSafe, catch_unwind},
};

const CONTEXT: &[u8] = b"independently retained context bytes";
struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    context: PathBuf,
    finality: PathBuf,
    queries: PathBuf,
    store: PathBuf,
    merge: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::Builder::new()
            .prefix("canonical-fs-")
            .tempdir_in("/tmp")
            .unwrap();
        let root = temp.path().canonicalize().unwrap().join("a");
        fs::create_dir(&root).unwrap();
        let inputs = root.join("inputs");
        let outputs = root.join("outputs");
        let store = root.join("kura");
        for path in [&inputs, &outputs, &store] {
            fs::create_dir(path).unwrap();
        }
        let context = inputs.join("context.norito");
        fs::write(&context, CONTEXT).unwrap();
        fs::set_permissions(&context, fs::Permissions::from_mode(0o600)).unwrap();
        // This is a structural canonical store fixture, not a signed finality proof. Public
        // Core builder/store APIs create its real canonical wire and published marker; the
        // ordinary read-only reader must consume everything to produce its opaque completion.
        let key = KeyPair::try_from_seed(vec![91; 32], Algorithm::Ed25519).unwrap();
        let block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
            .chain(0, None)
            .sign(key.private_key())
            .unpack(|_| {})
            .into();
        let mut writer = BlockStore::new(&store);
        writer.create_files_if_they_do_not_exist().unwrap();
        writer.append_block_to_chain(&block).unwrap();
        drop(writer);
        let merge = store.join("merge.log");
        fs::write(&merge, []).unwrap();
        Self {
            _temp: temp,
            root,
            context,
            finality: outputs.join("finality.norito"),
            queries: outputs.join("queries.norito"),
            store,
            merge,
        }
    }
    fn binding(&self) -> OriginalInputBinding {
        OriginalInputBinding {
            path: self.context.clone(),
            raw_sha256: iroha_crypto::sha256(CONTEXT),
            max_bytes: 1024,
        }
    }
    fn context(&self) -> RetainedOriginalInput {
        RetainedOriginalInput::open(self.binding()).unwrap()
    }
    fn complete(&self) -> CanonicalKuraEvidenceComplete {
        let limits = CanonicalKuraEvidenceLimits {
            first_height: 1,
            last_height: 1,
            max_committed_blocks: 4,
            max_store_data_bytes: 1024 * 1024,
            max_carrier_bytes: 1024 * 1024,
            max_merge_log_bytes: 1024 * 1024,
            max_merge_frames: 4,
            max_output_bytes: 2 * 1024 * 1024,
            max_decode_allocation_bytes: 8 * 1024 * 1024,
            owner_uid: fs::metadata(&self.store).unwrap().uid(),
        };
        let mut reader =
            CanonicalKuraEvidenceReader::open(&self.store, &self.merge, limits).unwrap();
        assert!(!reader.read_carrier(1).unwrap().is_empty());
        reader.scan_merge_entries(&[], |_, _, _| Ok(())).unwrap();
        let complete = reader.finish().unwrap();
        assert_eq!(complete.carrier_count(), 1);
        complete
    }
    fn pair(&self) -> CanonicalInputPair {
        CanonicalInputPair::admit(&self.finality, &self.queries, caps()).unwrap()
    }
    fn published(&self) -> PublishedCanonicalInputs {
        self.pair()
            .publish(
                self.context(),
                self.complete(),
                &vec![],
                &vec![],
                &|| Ok(()),
            )
            .unwrap()
    }
    fn stage(path: &Path) -> PathBuf {
        path.with_file_name(format!(
            "{}.collecting",
            path.file_name().unwrap().to_str().unwrap()
        ))
    }
}
fn caps() -> CanonicalInputCaps {
    CanonicalInputCaps {
        finality_bytes: 4096,
        query_bytes: 4096,
        total_bytes: 1024 + 8192,
    }
}
fn rewrite(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    if bytes.is_empty() {
        bytes.push(1);
    } else {
        bytes[0] ^= 1;
    }
    fs::write(path, bytes).unwrap();
}
#[test]
fn context_retains_exact_readonly_private_descriptor_and_independent_identity() {
    let f = Fixture::new();
    let owner = f.context();
    assert_eq!(
        rustix::fs::fcntl_getfl(&owner.file).unwrap() & OFlags::ACCMODE,
        OFlags::RDONLY
    );
    assert!(
        rustix::fs::fcntl_getfl(&owner.file)
            .unwrap()
            .contains(OFlags::NONBLOCK)
    );
    assert!(
        rustix::io::fcntl_getfd(&owner.file)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    assert_eq!(owner.with_bytes(|b| Ok(b.to_vec())).unwrap(), CONTEXT);
    assert_eq!(
        owner.identity().unwrap(),
        RawFileIdentity {
            raw_sha256: iroha_crypto::sha256(CONTEXT),
            byte_length: CONTEXT.len() as u64
        }
    );
    assert_eq!(fs::read(&f.context).unwrap(), CONTEXT);
}
#[test]
fn context_rejects_unbounded_wrong_digest_mode_links_and_path_admission() {
    for kind in 0..10 {
        let f = Fixture::new();
        let mut binding = f.binding();
        match kind {
            0 => binding.max_bytes = 0,
            1 => binding.max_bytes = MAX_CONTEXT_BYTES + 1,
            2 => binding.max_bytes = CONTEXT.len() as u64 - 1,
            3 => binding.raw_sha256 = [0; 32],
            4 => fs::set_permissions(&f.context, fs::Permissions::from_mode(0o644)).unwrap(),
            5 => fs::hard_link(&f.context, f.root.join("hardlink")).unwrap(),
            6 => binding.path = PathBuf::from("relative"),
            7 => binding.path = f.context.parent().unwrap().join(".").join("context.norito"),
            8 => {
                let alias = f.root.join("alias");
                symlink(f.context.parent().unwrap(), &alias).unwrap();
                binding.path = alias.join("context.norito");
            }
            _ => {
                fs::remove_file(&f.context).unwrap();
                symlink(f.root.join("missing"), &f.context).unwrap();
            }
        }
        assert!(
            RetainedOriginalInput::open(binding).is_err(),
            "admission kind {kind}"
        );
    }
}
#[test]
fn context_rejects_replacement_or_mutation_at_every_actual_admission_boundary() {
    let f = Fixture::new();
    let mut events = Vec::new();
    RetainedOriginalInput::open_with_hook(f.binding(), |e| {
        events.push(e);
        Ok(())
    })
    .unwrap();
    for target in 0..events.len() {
        let f = Fixture::new();
        let mut calls = 0;
        let mut fired = false;
        let result = RetainedOriginalInput::open_with_hook(f.binding(), |_| {
            if calls == target {
                rewrite(&f.context);
                fired = true;
            }
            calls += 1;
            Ok(())
        });
        assert!(
            fired && result.is_err(),
            "admission boundary {target} {:?}",
            events[target]
        );
    }
}
#[test]
fn context_callbacks_and_final_checks_permanently_poison_caught_failures() {
    for kind in 0..4 {
        let f = Fixture::new();
        let owner = f.context();
        match kind {
            0 => assert!(
                owner
                    .with_bytes::<()>(|_| Err(eyre!("caller failure")))
                    .is_err()
            ),
            1 => assert!(
                catch_unwind(AssertUnwindSafe(
                    || owner.with_bytes::<()>(|_| panic!("caller panic"))
                ))
                .is_err()
            ),
            2 => assert!(
                owner
                    .with_bytes(|_| {
                        rewrite(&f.context);
                        Ok(())
                    })
                    .is_err()
            ),
            _ => assert!(
                owner
                    .with_bytes_hook(
                        |_| Ok(()),
                        |e| {
                            if e == Event::AfterContextCheck {
                                return Err(eyre!("final hook failure"));
                            }
                            Ok(())
                        }
                    )
                    .is_err()
            ),
        }
        assert!(
            owner
                .identity()
                .unwrap_err()
                .to_string()
                .contains("poisoned")
        );
        if kind != 2 {
            assert_eq!(fs::read(&f.context).unwrap(), CONTEXT);
        }
    }
}
#[test]
fn context_full_digest_check_is_independent_of_metadata_comparison() {
    let f = Fixture::new();
    let mut owner = f.context();
    rewrite(&f.context);
    // Test-only rebind of the metadata oracle isolates the mandatory full-content comparison.
    owner.state = held(&owner.file).unwrap();
    assert!(
        owner
            .identity()
            .unwrap_err()
            .to_string()
            .contains("raw digest")
    );
}
#[test]
fn pair_admission_rejects_destination_stage_aliases_and_invalid_reservations_without_creating_files()
 {
    for kind in 0..7 {
        let f = Fixture::new();
        let mut values = caps();
        let mut second = f.queries.clone();
        match kind {
            0 => second = f.finality.clone(),
            1 => second = Fixture::stage(&f.finality),
            2 => values.finality_bytes = 0,
            3 => values.query_bytes = MAX_TRANSPORT_BYTES + 1,
            4 => values.total_bytes = MAX_TRANSPORT_BYTES + 1,
            5 => values.total_bytes = values.finality_bytes + values.query_bytes - 1,
            _ => fs::write(&second, b"racer").unwrap(),
        }
        assert!(CanonicalInputPair::admit(&f.finality, &second, values).is_err());
        assert!(!f.finality.exists() && !Fixture::stage(&f.finality).exists());
        assert!(!Fixture::stage(&f.queries).exists());
    }
}
#[test]
fn pair_counts_both_exact_vec_frames_and_total_reservations_before_any_stage() {
    for kind in 0..3 {
        let f = Fixture::new();
        let mut values = caps();
        if kind == 0 {
            values.total_bytes = values.finality_bytes + values.query_bytes;
        }
        if kind == 1 {
            values.finality_bytes =
                norito::canonical_frame_len(&Vec::<BridgeFinalityProof>::new()).unwrap() as u64 - 1;
        }
        if kind == 2 {
            values.query_bytes = norito::canonical_frame_len(&Vec::<CommittedTransaction>::new())
                .unwrap() as u64
                - 1;
        }
        let pair = CanonicalInputPair::admit(&f.finality, &f.queries, values).unwrap();
        let mut created = false;
        let result = pair.publish_with_hook(f.context(), f.complete(), &vec![], &vec![], |e| {
            if matches!(e, Event::Output(_, Phase::BeforeCreate)) {
                created = true;
            }
            Ok(())
        });
        assert!(result.is_err() && !created);
        assert!(!Fixture::stage(&f.finality).exists() && !Fixture::stage(&f.queries).exists());
    }
}
#[test]
fn both_core_ancestries_and_original_context_alias_are_checked_before_any_stage() {
    for slot in 0..2 {
        let f = Fixture::new();
        let protected = f.store.join("forbidden.norito");
        let (first, second) = if slot == 0 {
            (&protected, &f.queries)
        } else {
            (&f.finality, &protected)
        };
        let pair = CanonicalInputPair::admit(first, second, caps()).unwrap();
        let error = pair
            .publish(f.context(), f.complete(), &vec![], &vec![], &|| Ok(()))
            .err()
            .unwrap();
        assert!(error.to_string().contains("protected Core"));
        assert!(!Fixture::stage(first).exists() && !Fixture::stage(second).exists());
    }
    let f = Fixture::new();
    assert!(CanonicalInputPair::admit(&f.context, &f.queries, caps()).is_err());
    let f = Fixture::new();
    let pair = f.pair();
    // A retained file can appear at a destination after pair admission; never overwrite it.
    let bytes = fs::read(&f.context).unwrap();
    fs::rename(&f.context, &f.finality).unwrap();
    let context = RetainedOriginalInput::open(OriginalInputBinding {
        path: f.finality.clone(),
        raw_sha256: iroha_crypto::sha256(&bytes),
        max_bytes: 1024,
    })
    .unwrap();
    assert!(
        pair.publish(context, f.complete(), &vec![], &vec![], &|| Ok(()))
            .err()
            .unwrap()
            .to_string()
            .contains("alias")
    );
    assert_eq!(fs::read(&f.finality).unwrap(), bytes);
    assert!(!Fixture::stage(&f.finality).exists());
}
#[test]
fn complete_pair_publishes_exact_typed_frames_after_both_fsync_readbacks_and_retains_descriptors() {
    let f = Fixture::new();
    let finality = Vec::<BridgeFinalityProof>::new();
    let queries = Vec::<CommittedTransaction>::new();
    let mut events = Vec::new();
    let _ambient_flags = norito::core::DecodeFlagsGuard::enter(0);
    let published = f
        .pair()
        .publish_with_hook(f.context(), f.complete(), &finality, &queries, |e| {
            events.push(e);
            Ok(())
        })
        .unwrap();
    let identity = published.identity().unwrap();
    for (path, expected, actual, file) in [
        (
            &f.finality,
            norito::encode_canonical(&finality).unwrap(),
            identity.finality,
            &published.finality.file,
        ),
        (
            &f.queries,
            norito::encode_canonical(&queries).unwrap(),
            identity.queries,
            &published.queries.file,
        ),
    ] {
        assert_eq!(fs::read(path).unwrap(), expected);
        assert_eq!(actual.raw_sha256, iroha_crypto::sha256(&expected));
        assert_eq!(actual.byte_length, expected.len() as u64);
        assert_eq!(
            file.metadata().unwrap().ino(),
            fs::metadata(path).unwrap().ino()
        );
        assert_eq!(file.metadata().unwrap().mode() & 0o7777, 0o600);
        assert_eq!(file.metadata().unwrap().nlink(), 1);
        assert_eq!(
            rustix::fs::fcntl_getfl(file).unwrap() & OFlags::ACCMODE,
            OFlags::RDWR
        );
        assert!(!Fixture::stage(path).exists());
    }
    assert!(
        norito::decode_canonical::<Vec<BridgeFinalityProof>>(&fs::read(&f.finality).unwrap())
            .unwrap()
            .is_empty()
    );
    assert!(
        norito::decode_canonical::<Vec<CommittedTransaction>>(&fs::read(&f.queries).unwrap())
            .unwrap()
            .is_empty()
    );
    let rename = events
        .iter()
        .position(|e| *e == Event::Output(0, Phase::BeforeRename))
        .unwrap();
    for event in [
        Event::Readback(0),
        Event::Readback(1),
        Event::Output(0, Phase::AfterFileSync),
        Event::Output(1, Phase::AfterFileSync),
        Event::BothStaged,
    ] {
        assert!(events.iter().position(|e| *e == event).unwrap() < rename);
    }
    let mut reply = Vec::new();
    write!(
        &mut reply,
        "{} {}",
        identity.finality.byte_length, identity.queries.byte_length
    )
    .unwrap();
    reply.flush().unwrap();
    assert_eq!(published.identity().unwrap(), identity);
    assert!(!reply.is_empty());
}
#[test]
fn every_actual_pair_boundary_rechecks_context_and_core_and_never_returns_success_on_mutation() {
    let f = Fixture::new();
    let mut events = Vec::new();
    f.pair()
        .publish_with_hook(f.context(), f.complete(), &vec![], &vec![], |e| {
            events.push(e);
            Ok(())
        })
        .unwrap();
    for core in [false, true] {
        for target in 0..events.len() {
            let f = Fixture::new();
            let pair = f.pair();
            let context = f.context();
            let complete = f.complete();
            let mut calls = 0;
            let mut fired = false;
            let result = pair.publish_with_hook(context, complete, &vec![], &vec![], |_| {
                if calls == target {
                    rewrite(if core { &f.merge } else { &f.context });
                    fired = true;
                }
                calls += 1;
                Ok(())
            });
            assert!(
                fired && result.is_err(),
                "core={core} boundary={target} {:?}",
                events[target]
            );
        }
    }
}
#[test]
fn first_stage_and_published_output_remain_checked_while_second_owner_advances() {
    for event in [
        Event::Output(1, Phase::BeforeCreate),
        Event::Output(1, Phase::AfterFileSync),
        Event::BothStaged,
        Event::Output(1, Phase::BeforeRename),
        Event::Output(1, Phase::AfterRename),
    ] {
        let f = Fixture::new();
        let mut fired = false;
        let result = f
            .pair()
            .publish_with_hook(f.context(), f.complete(), &vec![], &vec![], |e| {
                if e == event && !fired {
                    let stage = Fixture::stage(&f.finality);
                    rewrite(if f.finality.exists() {
                        &f.finality
                    } else {
                        &stage
                    });
                    fired = true;
                }
                Ok(())
            });
        assert!(fired && result.is_err(), "cross-output boundary {event:?}");
    }
}
#[test]
fn second_noreplace_racer_preserves_first_publication_both_original_stages_and_racer() {
    let f = Fixture::new();
    let mut fired = false;
    let result = f
        .pair()
        .publish_with_hook(f.context(), f.complete(), &vec![], &vec![], |e| {
            if e == Event::Output(1, Phase::RenameReady) {
                fs::write(&f.queries, b"racer").unwrap();
                fired = true;
            }
            Ok(())
        });
    assert!(fired && result.is_err());
    assert!(f.finality.exists());
    assert!(!Fixture::stage(&f.finality).exists());
    assert!(Fixture::stage(&f.queries).exists());
    assert_eq!(fs::read(&f.queries).unwrap(), b"racer");
    assert_eq!(fs::read(&f.context).unwrap(), CONTEXT);
}
#[test]
fn last_check_source_substitution_is_detected_after_noreplace_without_cleanup() {
    for slot in 0..2 {
        let f = Fixture::new();
        let path = if slot == 0 { &f.finality } else { &f.queries };
        let stage = Fixture::stage(path);
        let retained = stage.with_extension("retained");
        let mut fired = false;
        let result = f
            .pair()
            .publish_with_hook(f.context(), f.complete(), &vec![], &vec![], |e| {
                if e == Event::Output(slot, Phase::RenameReady) {
                    fs::rename(&stage, &retained).unwrap();
                    fs::write(&stage, b"substitute").unwrap();
                    fs::set_permissions(&stage, fs::Permissions::from_mode(0o600)).unwrap();
                    fired = true;
                }
                Ok(())
            });
        assert!(fired && result.is_err());
        assert!(retained.exists());
        assert_eq!(fs::read(path).unwrap(), b"substitute");
    }
}
#[test]
fn output_parent_or_ancestor_replacement_at_identity_or_publication_cannot_succeed() {
    for event in [
        Event::AfterEncode,
        Event::BothStaged,
        Event::Output(0, Phase::AfterRename),
        Event::AfterIdentity,
    ] {
        let f = Fixture::new();
        let old = f.root.join("outputs-old");
        let mut fired = false;
        let result = f
            .pair()
            .publish_with_hook(f.context(), f.complete(), &vec![], &vec![], |e| {
                if e == event && !fired {
                    fs::rename(f.finality.parent().unwrap(), &old).unwrap();
                    fs::create_dir(f.finality.parent().unwrap()).unwrap();
                    fired = true;
                }
                Ok(())
            });
        assert!(fired && result.is_err());
        assert!(old.exists());
    }
}
#[test]
fn late_identity_checks_retain_all_owners_and_permanently_poison_errors_and_panics() {
    for kind in 0..6 {
        let f = Fixture::new();
        let owner = f.published();
        let identity = owner.identity().unwrap();
        match kind {
            0 => rewrite(&f.context),
            1 => rewrite(&f.merge),
            2 => rewrite(&f.finality),
            3 => rewrite(&f.queries),
            4 => assert!(
                owner
                    .identity_with_hook(|e| {
                        if e == Event::AfterIdentity {
                            Err(eyre!("late failure"))
                        } else {
                            Ok(())
                        }
                    })
                    .is_err()
            ),
            _ => assert!(
                catch_unwind(AssertUnwindSafe(|| owner.identity_with_hook(|e| {
                    if e == Event::AfterIdentity {
                        panic!("late panic")
                    }
                    Ok(())
                })))
                .is_err()
            ),
        }
        if kind < 4 {
            assert!(owner.identity().is_err());
        }
        assert!(
            owner
                .identity()
                .unwrap_err()
                .to_string()
                .contains("poisoned")
        );
        assert!(f.finality.exists() && f.queries.exists());
        if kind >= 4 {
            assert_eq!(
                iroha_crypto::sha256(&fs::read(&f.finality).unwrap()),
                identity.finality.raw_sha256
            );
        }
    }
}
#[test]
fn output_full_digest_rechecks_remain_required_even_if_metadata_oracle_is_rebound_in_test() {
    let f = Fixture::new();
    let mut owner = f.published();
    rewrite(&f.finality);
    owner.finality.state = held(&owner.finality.file).unwrap();
    assert!(
        owner
            .identity()
            .unwrap_err()
            .to_string()
            .contains("raw digest")
    );
}

#[test]
fn context_namespace_moves_are_rejected_with_original_descriptors_still_retained() {
    for ancestor in [false, true] {
        let f = Fixture::new();
        let owner = f.context();
        let path = if ancestor {
            &f.root
        } else {
            f.context.parent().unwrap()
        };
        let old = path.with_extension("retained");
        fs::rename(path, &old).unwrap();
        fs::create_dir(path).unwrap();
        assert!(owner.identity().is_err());
        assert!(
            owner
                .identity()
                .unwrap_err()
                .to_string()
                .contains("poisoned")
        );
        assert!(old.exists());
    }
}
#[test]
fn every_output_stage_operation_detects_content_or_name_changes_and_retains_evidence() {
    let f = Fixture::new();
    let mut events = Vec::new();
    f.pair()
        .publish_with_hook(f.context(), f.complete(), &vec![], &vec![], |e| {
            events.push(e);
            Ok(())
        })
        .unwrap();
    for (target, event) in events.iter().copied().enumerate() {
        let slot = match event {
            Event::Output(i, _)
            | Event::BeforeWrite(i)
            | Event::AfterWrite(i)
            | Event::BeforeReadback(i)
            | Event::Readback(i) => i,
            _ => continue,
        };
        let f = Fixture::new();
        let mut calls = 0;
        let mut fired = false;
        let result =
            f.pair()
                .publish_with_hook(f.context(), f.complete(), &vec![], &vec![], |_| {
                    if calls == target {
                        let output = if slot == 0 { &f.finality } else { &f.queries };
                        let stage = Fixture::stage(output);
                        if output.exists() {
                            rewrite(output);
                        } else if stage.exists() {
                            rewrite(&stage);
                        } else {
                            fs::write(&stage, b"racer").unwrap();
                        }
                        fired = true;
                    }
                    calls += 1;
                    Ok(())
                });
        assert!(
            fired && result.is_err(),
            "stage boundary {target} {event:?}"
        );
        let output = if slot == 0 { &f.finality } else { &f.queries };
        assert!(output.exists() || Fixture::stage(output).exists());
    }
}

#[test]
fn complete_pair_check_rejects_earlier_output_swap_during_later_scan() {
    let f = Fixture::new();
    let owner = f.published();
    let bytes = fs::read(&f.finality).unwrap();
    let replacement = f.finality.with_extension("replacement");
    let error = owner
        .check_with_midpoint(|| {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&replacement)?
                .write_all(&bytes)?;
            fs::rename(&replacement, &f.finality)?;
            Ok(())
        })
        .unwrap_err();
    assert!(error.to_string().contains("namespace changed"));
    assert!(owner.identity().is_err());
}

#[test]
fn retained_original_requires_exact_readonly_inherited_descriptor_and_original_bytes() {
    use std::os::fd::AsRawFd as _;
    let f = Fixture::new();
    let owner = f.context();
    let original = File::open(&f.context).unwrap();
    owner
        .require_descriptor(original.as_raw_fd() as u32)
        .unwrap();
    let foreign = f.context.with_extension("same-bytes");
    let bytes = fs::read(&f.context).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&foreign)
        .unwrap()
        .write_all(&bytes)
        .unwrap();
    let foreign = File::open(foreign).unwrap();
    assert!(
        owner
            .require_descriptor(foreign.as_raw_fd() as u32)
            .is_err()
    );
    assert!(owner.identity().is_err());
    let owner = f.context();
    let writable = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&f.context)
        .unwrap();
    assert!(
        owner
            .require_descriptor(writable.as_raw_fd() as u32)
            .is_err()
    );
}

#[test]
fn external_publication_guard_failure_never_returns_a_pair() {
    let f = Fixture::new();
    assert!(
        f.pair()
            .publish(f.context(), f.complete(), &vec![], &vec![], &|| Err(eyre!(
                "original client/deadline failed"
            )))
            .is_err()
    );
    assert!(!f.finality.exists() && !f.queries.exists());
}
