//! Actual ten-original custody controls using the semantic assembler's real fixture.

use super::*;
use crate::kura::scaling_evidence::export::launcher::prepare::assemble::tests::{
    Fixture, with_facts_assembly_stack,
};
use std::{
    fs,
    os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink},
    panic::{AssertUnwindSafe, catch_unwind},
};

fn run(
    fixture: &Fixture,
    hook: impl FnMut(FactsRole, Phase) -> Result<()>,
) -> Result<PublishedFacts> {
    produce_with_hook(
        fixture.bindings(),
        &fixture.output_path(),
        fixture.genesis(),
        fixture.journal(),
        fixture.verification_limits(),
        fixture.block_store(),
        fixture.merge_log(),
        fixture.reader_limits(),
        fixture.caps(),
        hook,
    )
}
fn stage(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(".publishing");
    value.into()
}
fn change_content(path: &Path) {
    let original = fs::read(path).unwrap();
    let mut changed = original.clone();
    assert!(!changed.is_empty());
    changed[0] ^= 1;
    fs::write(path, &changed).unwrap();
    assert_ne!(fs::read(path).unwrap(), original);
}
fn replace_inode(path: &Path) {
    let original = fs::read(path).unwrap();
    let prior = fs::metadata(path).unwrap();
    let saved = path.with_extension("retained-original");
    fs::rename(path, &saved).unwrap();
    fs::write(path, &original).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(prior.mode() & 0o7777)).unwrap();
    assert_ne!(fs::metadata(path).unwrap().ino(), prior.ino());
    assert_eq!(fs::read(path).unwrap(), original);
    assert_eq!(fs::read(saved).unwrap(), original);
}
fn output_target(fixture: &Fixture) -> PathBuf {
    let output = fixture.output_path();
    if output.exists() {
        output
    } else {
        stage(&output)
    }
}
fn assert_no_output(fixture: &Fixture) {
    assert!(!fixture.output_path().exists());
    assert!(!stage(&fixture.output_path()).exists());
}

fn sensitive_inputs() -> (tempfile::TempDir, Inputs) {
    let directory = tempfile::Builder::new()
        .prefix("facts-read-")
        .tempdir_in("/tmp")
        .unwrap();
    let root = directory.path().canonicalize().unwrap();
    let mut bindings = Vec::new();
    for index in 0..10 {
        let path = root.join(format!("original-{index}"));
        let bytes =
            Zeroizing::new(format!("private_key = \"fixture-secret-{index}\"\n").into_bytes());
        fs::write(&path, &bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        bindings.push(ProofInputBinding {
            path,
            sha256: iroha_crypto::sha256(&bytes),
            max_bytes: bytes.len() as u64,
        });
    }
    let inputs = Inputs::open(bindings, 4096, &mut |_| Ok(())).unwrap();
    (directory, inputs)
}

#[test]
fn facts_sensitive_reads_return_guarded_buffers_and_preserve_single_consumption() {
    let (_directory, mut inputs) = sensitive_inputs();
    let bytes: Vec<Zeroizing<Vec<u8>>> = inputs.read_sensitive_all(&mut |_| Ok(())).unwrap();
    assert_eq!(bytes.len(), 10);
    for (index, bytes) in bytes.iter().enumerate() {
        assert_eq!(iroha_crypto::sha256(bytes), inputs.files[index].digest);
        assert_eq!(bytes.len() as u64, inputs.files[index].state.size);
    }
    assert!(inputs.read);
    assert!(!inputs.poisoned);
    assert!(inputs.read_sensitive_all(&mut |_| Ok(())).is_err());
    inputs
        .finish_lease(&mut |_| Ok(()))
        .unwrap()
        .check()
        .unwrap();
}

#[test]
fn facts_sensitive_read_errors_and_unwinds_after_prior_secret_buffers_poison_the_owner() {
    for panic in [false, true] {
        let (_directory, mut inputs) = sensitive_inputs();
        let mut completed = 0;
        let result = catch_unwind(AssertUnwindSafe(|| {
            inputs.read_sensitive_all(&mut |phase| {
                if phase == Phase::AfterRead {
                    completed += 1;
                    if completed == 7 {
                        if panic {
                            panic!("sensitive read hook unwind");
                        }
                        return Err(eyre!("sensitive read hook error"));
                    }
                }
                Ok(())
            })
        }));
        if panic {
            assert!(result.is_err());
        } else {
            let error = result.unwrap().err().unwrap();
            assert!(!format!("{error}\n{error:?}").contains("fixture-secret"));
        }
        assert_eq!(completed, 7);
        assert!(inputs.poisoned);
        assert!(!inputs.read);
        assert!(inputs.read_sensitive_all(&mut |_| Ok(())).is_err());
        assert!(inputs.finish_lease(&mut |_| Ok(())).is_err());
    }
}

#[test]
fn facts_sensitive_late_digest_failure_exposes_no_plaintext_and_cannot_finish() {
    let (_directory, mut inputs) = sensitive_inputs();
    inputs.files[9].digest[0] ^= 1;
    let mut completed = 0;
    let result = inputs.read_sensitive_all(&mut |phase| {
        if phase == Phase::AfterRead {
            completed += 1;
        }
        Ok(())
    });
    let error = result.err().unwrap();
    assert_eq!(completed, 10);
    assert!(!format!("{error}\n{error:?}").contains("fixture-secret"));
    assert!(inputs.poisoned);
    assert!(!inputs.read);
    assert!(inputs.finish_lease(&mut |_| Ok(())).is_err());
}

#[test]
fn facts_require_all_four_secret_peer_configs_owner_only_before_any_content_read() {
    with_facts_assembly_stack(|| {
        for index in 2..6 {
            for mode in [0o640, 0o644] {
                let fixture = Fixture::new(1);
                let path = fixture.original_paths().remove(index);
                fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
                let mut admitted = 0;
                let mut reads = 0;
                let result = run(&fixture, |_, phase| {
                    if phase == Phase::InputRetained {
                        admitted += 1;
                    }
                    if phase == Phase::BeforeRead {
                        reads += 1;
                    }
                    Ok(())
                });
                assert!(result.is_err(), "peer role {index} mode {mode:o}");
                assert_eq!(admitted, 10);
                assert_eq!(reads, 0);
                assert_no_output(&fixture);
            }
        }
        let fixture = Fixture::new(1);
        for path in &fixture.original_paths()[2..6] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        run(&fixture, |_, _| Ok(())).unwrap().identity().unwrap();
    });
}

#[test]
fn facts_publication_preserves_real_one_and_four_lane_originals_and_completed_core_through_reply() {
    with_facts_assembly_stack(|| {
        for lanes in [1, 4] {
            let fixture = Fixture::new(lanes);
            let paths = fixture.original_paths();
            assert_eq!(paths.len(), 10);
            let original: Vec<_> = paths
                .iter()
                .map(|p| (fs::read(p).unwrap(), fs::metadata(p).unwrap().ino()))
                .collect();
            let owner = run(&fixture, |_, _| Ok(())).unwrap();
            let identity = owner.identity().unwrap();
            let bytes = fs::read(fixture.output_path()).unwrap();
            assert_eq!(identity.raw_sha256, iroha_crypto::sha256(&bytes));
            assert_eq!(identity.byte_length, bytes.len() as u64);
            assert_eq!(owner.facts.canonical_bytes(), bytes);
            assert_eq!(owner.inputs.files.len(), 10);
            owner.facts.recheck_sources().unwrap();
            assert!(!stage(&fixture.output_path()).exists());
            let metadata = fs::metadata(fixture.output_path()).unwrap();
            assert_eq!(metadata.mode() & 0o7777, 0o600);
            assert_eq!(metadata.nlink(), 1);
            let mut reply = Vec::new();
            let returned = owner
                .finish_reply(|got| {
                    assert_eq!(got, identity);
                    reply.write_all(b"{\"operation\":\"facts\"}\n")?;
                    reply.flush()?;
                    Ok(())
                })
                .unwrap();
            assert_eq!(returned, identity);
            assert!(!reply.is_empty());
            for (path, (bytes, inode)) in paths.iter().zip(original) {
                assert_eq!(fs::read(path).unwrap(), bytes);
                assert_eq!(fs::metadata(path).unwrap().ino(), inode);
            }
        }
    });
}

#[test]
fn facts_boundary_census_includes_all_ten_roles_and_full_publication() {
    with_facts_assembly_stack(|| {
        let fixture = Fixture::new(4);
        let mut seen = Vec::new();
        let owner = run(&fixture, |role, phase| {
            seen.push((role, phase));
            Ok(())
        })
        .unwrap();
        owner
            .identity_with_hook(|role, phase| {
                seen.push((role, phase));
                Ok(())
            })
            .unwrap();
        for index in 0..10 {
            for phase in [
                Phase::ParentAdmitted,
                Phase::ParentRetained,
                Phase::InputAdmitted,
                Phase::InputRetained,
                Phase::BeforeRead,
                Phase::AfterRead,
            ] {
                assert!(
                    seen.contains(&(FactsRole::Original(index), phase)),
                    "missing original {index}/{phase:?}"
                );
            }
        }
        for phase in [
            Phase::BeforeInputFinish,
            Phase::BeforeVerification,
            Phase::AfterVerification,
            Phase::BeforeIdentity,
            Phase::AfterIdentity,
        ] {
            assert!(
                seen.contains(&(FactsRole::Assembly, phase)),
                "missing assembly {phase:?}"
            );
        }
        for phase in [
            Phase::ParentAdmitted,
            Phase::ParentRetained,
            Phase::BeforeCreate,
            Phase::AfterCreate,
            Phase::BeforeWrite,
            Phase::AfterWrite,
            Phase::BeforeFileSync,
            Phase::AfterFileSync,
            Phase::BeforeReadback,
            Phase::AfterReadback,
            Phase::BeforeRename,
            Phase::RenameReady,
            Phase::AfterRename,
            Phase::BeforeDirectorySync,
            Phase::AfterDirectorySync,
        ] {
            assert!(
                seen.contains(&(FactsRole::Output, phase)),
                "missing output {phase:?}"
            );
        }
    });
}

#[test]
fn facts_source_and_output_reservations_fail_before_any_file_hook() {
    with_facts_assembly_stack(|| {
        let fixture = Fixture::new(1);
        for choice in 0..11 {
            let mut caps = fixture.caps();
            let mut bindings = fixture.bindings();
            match choice {
                0 => caps.input_bytes = 0,
                1 => caps.facts_bytes = 0,
                2 => caps.total_bytes = MAX_INPUT_BYTES + 1,
                3 => caps.decode_bytes = 0,
                4 => caps.decode_bytes = 512 * 1024 * 1024 + 1,
                5 => caps.input_bytes = u64::MAX,
                6 => caps.facts_bytes = u64::MAX,
                7 => caps.total_bytes = caps.input_bytes,
                8 => bindings.manifest.max_bytes = 0,
                9 => bindings.queries.max_bytes = caps.input_bytes,
                10 => bindings.context.max_bytes = 8 * 1024 * 1024 + 1,
                _ => unreachable!(),
            }
            let mut observed = 0;
            let result = produce_with_hook(
                bindings,
                &fixture.output_path(),
                fixture.genesis(),
                fixture.journal(),
                fixture.verification_limits(),
                fixture.block_store(),
                fixture.merge_log(),
                fixture.reader_limits(),
                caps,
                |_, _| {
                    observed += 1;
                    Ok(())
                },
            );
            assert!(result.is_err(), "case {choice}");
            assert_eq!(
                observed, 0,
                "reservation validation occurred after a file hook"
            );
            assert_no_output(&fixture);
        }
    });
}

#[test]
fn facts_admit_all_proof_and_core_work_limits_before_any_file_hook() {
    with_facts_assembly_stack(|| {
        let fixture = Fixture::new(1);
        for choice in 0..22 {
            let mut verification = fixture.verification_limits();
            let mut reader = fixture.reader_limits();
            match choice {
                0 => verification.admitted_proof_bytes = 0,
                1 => verification.admitted_proof_bytes = MAX_INPUT_BYTES + 1,
                2 => verification.input_bytes = 0,
                3 => verification.output_bytes = 0,
                4 => verification.input_bytes = u64::MAX,
                5 => verification.heights = 0,
                6 => verification.heights = 65_537,
                7 => verification.requests = 0,
                8 => verification.requests = 1_000_001,
                9 => verification.leaves_per_carrier = 0,
                10 => verification.leaves_per_carrier = 1_000_001,
                11 => reader.first_height = 2,
                12 => reader.last_height = 1,
                13 => reader.last_height = reader.max_committed_blocks + 1,
                14 => reader.max_committed_blocks = 1_000_001,
                15 => reader.max_store_data_bytes = 0,
                16 => reader.max_store_data_bytes = verification.input_bytes + 1,
                17 => reader.max_carrier_bytes = 32 * 1024 * 1024 + 1,
                18 => reader.max_merge_log_bytes = verification.input_bytes + 1,
                19 => reader.max_merge_frames = reader.max_committed_blocks + 1,
                20 => reader.max_output_bytes = 0,
                21 => reader.max_decode_allocation_bytes = 0,
                _ => unreachable!(),
            }
            let mut observed = 0;
            assert!(
                produce_with_hook(
                    fixture.bindings(),
                    &fixture.output_path(),
                    fixture.genesis(),
                    fixture.journal(),
                    verification,
                    fixture.block_store(),
                    fixture.merge_log(),
                    reader,
                    fixture.caps(),
                    |_, _| {
                        observed += 1;
                        Ok(())
                    }
                )
                .is_err(),
                "work bound {choice}"
            );
            assert_eq!(observed, 0, "work admission occurred after a file hook");
            assert_no_output(&fixture);
        }
    });
}

#[test]
fn facts_reject_wrong_core_uid_before_any_file_hook() {
    with_facts_assembly_stack(|| {
        let fixture = Fixture::new(1);
        let mut limits = fixture.reader_limits();
        limits.owner_uid ^= 1;
        let mut observed = 0;
        assert!(
            produce_with_hook(
                fixture.bindings(),
                &fixture.output_path(),
                fixture.genesis(),
                fixture.journal(),
                fixture.verification_limits(),
                fixture.block_store(),
                fixture.merge_log(),
                limits,
                fixture.caps(),
                |_, _| {
                    observed += 1;
                    Ok(())
                }
            )
            .is_err()
        );
        assert_eq!(observed, 0);
        assert_no_output(&fixture);
    });
}

#[test]
fn facts_reject_duplicate_paths_and_original_output_stage_or_core_roles_before_read() {
    with_facts_assembly_stack(|| {
        for choice in 0..4 {
            let fixture = Fixture::new(1);
            let mut bindings = fixture.bindings();
            bindings.queries.path = match choice {
                0 => bindings.manifest.path.clone(),
                1 => fixture.output_path(),
                2 => stage(&fixture.output_path()),
                3 => fixture.block_store().join("blocks.data"),
                _ => unreachable!(),
            };
            let mut read = false;
            assert!(
                produce_with_hook(
                    bindings,
                    &fixture.output_path(),
                    fixture.genesis(),
                    fixture.journal(),
                    fixture.verification_limits(),
                    fixture.block_store(),
                    fixture.merge_log(),
                    fixture.reader_limits(),
                    fixture.caps(),
                    |_, phase| {
                        read |= phase == Phase::BeforeRead;
                        Ok(())
                    }
                )
                .is_err()
            );
            assert!(!read);
            assert_no_output(&fixture);
        }
    });
}

#[test]
fn facts_reject_wrong_raw_pin_for_every_original_before_assembly_or_stage() {
    with_facts_assembly_stack(|| {
        for index in 0..10 {
            let fixture = Fixture::new(1);
            let mut bindings = fixture.bindings();
            let selected = match index {
                0 => &mut bindings.manifest,
                1 => &mut bindings.signed_genesis,
                2..=5 => &mut bindings.peer_configs[index - 2],
                6 => &mut bindings.context,
                7 => &mut bindings.journal,
                8 => &mut bindings.finality,
                9 => &mut bindings.queries,
                _ => unreachable!(),
            };
            selected.sha256[0] ^= 1;
            let mut assembly = false;
            assert!(
                produce_with_hook(
                    bindings,
                    &fixture.output_path(),
                    fixture.genesis(),
                    fixture.journal(),
                    fixture.verification_limits(),
                    fixture.block_store(),
                    fixture.merge_log(),
                    fixture.reader_limits(),
                    fixture.caps(),
                    |_, phase| {
                        assembly |= phase == Phase::BeforeVerification;
                        Ok(())
                    }
                )
                .is_err()
            );
            assert!(!assembly);
            assert_no_output(&fixture);
        }
    });
}

#[test]
fn facts_reject_symlink_hardlink_and_unowned_mode_originals_before_read() {
    with_facts_assembly_stack(|| {
        for choice in 0..3 {
            let fixture = Fixture::new(1);
            let path = fixture.original_paths().remove(0);
            match choice {
                0 => {
                    let saved = path.with_extension("actual-original");
                    fs::rename(&path, &saved).unwrap();
                    symlink(saved, &path).unwrap();
                }
                1 => fs::hard_link(&path, path.with_extension("hardlink")).unwrap(),
                2 => fs::set_permissions(&path, fs::Permissions::from_mode(0o622)).unwrap(),
                _ => unreachable!(),
            }
            let mut read = false;
            assert!(
                run(&fixture, |_, phase| {
                    read |= phase == Phase::BeforeRead;
                    Ok(())
                })
                .is_err()
            );
            assert!(!read);
            assert_no_output(&fixture);
        }
    });
}

#[test]
fn facts_reject_each_original_content_and_inode_mutation_during_its_actual_read() {
    with_facts_assembly_stack(|| {
        for index in 0..10 {
            for fresh_inode in [false, true] {
                let fixture = Fixture::new(1);
                let path = fixture.original_paths().remove(index);
                let mut fired = false;
                assert!(
                    run(&fixture, |role, phase| {
                        if !fired
                            && role == FactsRole::Original(index)
                            && phase == Phase::BeforeRead
                        {
                            fired = true;
                            if fresh_inode {
                                replace_inode(&path);
                            } else {
                                change_content(&path);
                            }
                        }
                        Ok(())
                    })
                    .is_err()
                );
                assert!(fired);
                assert_no_output(&fixture);
            }
        }
    });
}

#[test]
fn facts_retain_every_original_across_assembly_stage_sync_rename_and_identity() {
    with_facts_assembly_stack(|| {
        for index in 0..10 {
            for (role, phase) in [
                (FactsRole::Assembly, Phase::AfterVerification),
                (FactsRole::Output, Phase::BeforeWrite),
                (FactsRole::Output, Phase::AfterFileSync),
                (FactsRole::Output, Phase::RenameReady),
                (FactsRole::Output, Phase::AfterRename),
                (FactsRole::Assembly, Phase::AfterIdentity),
            ] {
                let fixture = Fixture::new(1);
                let path = fixture.original_paths().remove(index);
                let mut fired = false;
                assert!(
                    run(&fixture, |at_role, at_phase| {
                        if !fired && (at_role, at_phase) == (role, phase) {
                            fired = true;
                            change_content(&path);
                        }
                        Ok(())
                    })
                    .is_err(),
                    "original {index}/{role:?}/{phase:?}"
                );
                assert!(fired);
            }
        }
    });
}

#[test]
fn facts_reject_actual_core_content_and_fresh_inode_changes_during_publication() {
    with_facts_assembly_stack(|| {
        for core_file in [
            "blocks.data",
            "blocks.index",
            "blocks.hashes",
            "blocks.count.norito",
            "merge.log",
        ] {
            for fresh_inode in [false, true] {
                let fixture = Fixture::new(4);
                let path = if core_file == "merge.log" {
                    fixture.merge_log().to_owned()
                } else {
                    fixture.block_store().join(core_file)
                };
                let mut fired = false;
                assert!(
                    run(&fixture, |role, phase| {
                        if !fired && role == FactsRole::Output && phase == Phase::AfterFileSync {
                            fired = true;
                            if fresh_inode {
                                replace_inode(&path);
                            } else {
                                change_content(&path);
                            }
                        }
                        Ok(())
                    })
                    .is_err(),
                    "Core {core_file}, fresh inode {fresh_inode}"
                );
                assert!(fired);
                assert!(stage(&fixture.output_path()).exists());
            }
        }
    });
}

#[test]
fn facts_reject_publication_inside_real_core_namespace_before_creating_a_stage() {
    with_facts_assembly_stack(|| {
        let fixture = Fixture::new(4);
        let output = fixture.block_store().join("facts.norito");
        let mut created = false;
        assert!(
            produce_with_hook(
                fixture.bindings(),
                &output,
                fixture.genesis(),
                fixture.journal(),
                fixture.verification_limits(),
                fixture.block_store(),
                fixture.merge_log(),
                fixture.reader_limits(),
                fixture.caps(),
                |_, phase| {
                    created |= phase == Phase::BeforeCreate;
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!created);
        assert!(!output.exists());
        assert!(!stage(&output).exists());
    });
}

#[test]
fn facts_reject_original_output_and_core_ancestor_replacement() {
    with_facts_assembly_stack(|| {
        for role in 0..3 {
            let fixture = Fixture::new(1);
            let parent = match role {
                0 => fixture.original_paths()[0].parent().unwrap().to_owned(),
                1 => fixture.output_path().parent().unwrap().to_owned(),
                2 => fixture.block_store().to_owned(),
                _ => unreachable!(),
            };
            let mut fired = false;
            assert!(
                run(&fixture, |at_role, phase| {
                    if !fired && at_role == FactsRole::Output && phase == Phase::BeforeWrite {
                        fired = true;
                        let before = fs::metadata(&parent).unwrap().ino();
                        fs::rename(&parent, parent.with_extension("retained-parent")).unwrap();
                        fs::create_dir(&parent).unwrap();
                        assert_ne!(fs::metadata(&parent).unwrap().ino(), before);
                    }
                    Ok(())
                })
                .is_err()
            );
            assert!(fired);
        }
    });
}

#[test]
fn facts_reject_output_content_and_inode_mutations_at_every_complete_output_boundary() {
    with_facts_assembly_stack(|| {
        for phase in [
            Phase::AfterWrite,
            Phase::BeforeFileSync,
            Phase::AfterFileSync,
            Phase::BeforeReadback,
            Phase::AfterReadback,
            Phase::BeforeRename,
            Phase::RenameReady,
            Phase::AfterRename,
        ] {
            for fresh_inode in [false, true] {
                let fixture = Fixture::new(1);
                let mut fired = false;
                assert!(
                    run(&fixture, |role, at| {
                        if !fired && role == FactsRole::Output && at == phase {
                            fired = true;
                            let path = output_target(&fixture);
                            if fresh_inode {
                                replace_inode(&path);
                            } else {
                                change_content(&path);
                            }
                        }
                        Ok(())
                    })
                    .is_err(),
                    "output {phase:?}, fresh inode {fresh_inode}"
                );
                assert!(fired);
                assert!(output_target(&fixture).exists());
            }
        }
    });
}

#[test]
fn facts_noreplace_preserves_existing_output_stage_and_publication_racer() {
    with_facts_assembly_stack(|| {
        for choice in 0..3 {
            let fixture = Fixture::new(1);
            let output = fixture.output_path();
            let selected = if choice == 1 {
                stage(&output)
            } else {
                output.clone()
            };
            if choice != 2 {
                fs::write(&selected, b"retained-other-owner").unwrap();
            }
            let mut fired = false;
            assert!(
                run(&fixture, |role, phase| {
                    if choice == 2
                        && !fired
                        && role == FactsRole::Output
                        && phase == Phase::RenameReady
                    {
                        fired = true;
                        fs::write(&output, b"retained-other-owner").unwrap();
                    }
                    Ok(())
                })
                .is_err()
            );
            assert_eq!(fs::read(&selected).unwrap(), b"retained-other-owner");
            if choice == 2 {
                assert!(fired);
                assert!(stage(&output).exists());
            }
        }
    });
}

#[test]
fn facts_identity_errors_and_unwinds_permanently_poison_the_real_owner() {
    with_facts_assembly_stack(|| {
        for panic in [false, true] {
            let fixture = Fixture::new(1);
            let owner = run(&fixture, |_, _| Ok(())).unwrap();
            let prior = owner.identity().unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                owner.identity_with_hook(|_, phase| {
                    if phase == Phase::AfterIdentity {
                        if panic {
                            panic!("injected identity unwind");
                        }
                        return Err(eyre!("injected identity error"));
                    }
                    Ok(())
                })
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert!(owner.identity().is_err());
            assert_eq!(
                iroha_crypto::sha256(&fs::read(fixture.output_path()).unwrap()),
                prior.raw_sha256
            );
        }
    });
}

#[test]
fn facts_final_reply_rechecks_original_core_and_output_after_actual_write_and_flush() {
    with_facts_assembly_stack(|| {
        for role in 0..3 {
            for after_flush in [false, true] {
                let fixture = Fixture::new(4);
                let owner = run(&fixture, |_, _| Ok(())).unwrap();
                let path = match role {
                    0 => fixture.original_paths().remove(7),
                    1 => fixture.block_store().join("blocks.hashes"),
                    2 => fixture.output_path(),
                    _ => unreachable!(),
                };
                let mut reply = Vec::new();
                assert!(
                    owner
                        .finish_reply(|_| {
                            reply.write_all(b"{\"operation\":\"facts\"}\n")?;
                            if !after_flush {
                                change_content(&path);
                            }
                            reply.flush()?;
                            if after_flush {
                                replace_inode(&path);
                            }
                            Ok(())
                        })
                        .is_err()
                );
                assert!(!reply.is_empty());
                assert!(fixture.output_path().exists());
            }
        }
    });
}

#[test]
fn facts_reply_error_and_unwind_preserve_the_published_artifact_without_success() {
    with_facts_assembly_stack(|| {
        for panic in [false, true] {
            let fixture = Fixture::new(1);
            let owner = run(&fixture, |_, _| Ok(())).unwrap();
            let before = fs::read(fixture.output_path()).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                owner.finish_reply(|_| {
                    if panic {
                        panic!("injected reply unwind");
                    }
                    Err(eyre!("injected reply writer failure"))
                })
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(fs::read(fixture.output_path()).unwrap(), before);
            assert!(!stage(&fixture.output_path()).exists());
        }
    });
}

#[test]
fn facts_exact_serialized_cap_succeeds_and_one_byte_small_creates_no_truncated_stage() {
    with_facts_assembly_stack(|| {
        let fixture = Fixture::new(4);
        let first = run(&fixture, |_, _| Ok(())).unwrap();
        let exact = first.identity().unwrap();
        let original = fs::read(fixture.output_path()).unwrap();
        for too_small in [false, true] {
            let mut caps = fixture.caps();
            caps.facts_bytes = exact.byte_length - u64::from(too_small);
            let path =
                fixture
                    .output_path()
                    .with_extension(if too_small { "small" } else { "exact" });
            let result = produce_facts(
                fixture.bindings(),
                &path,
                fixture.genesis(),
                fixture.journal(),
                fixture.verification_limits(),
                fixture.block_store(),
                fixture.merge_log(),
                fixture.reader_limits(),
                caps,
            );
            if too_small {
                assert!(result.is_err());
                assert!(!path.exists());
                assert!(!stage(&path).exists());
            } else {
                let owner = result.unwrap();
                assert_eq!(owner.identity().unwrap(), exact);
                assert_eq!(fs::read(path).unwrap(), original);
            }
        }
        assert_eq!(first.identity().unwrap(), exact);
    });
}

#[test]
fn facts_malformed_peer_config_errors_do_not_render_secret_source() {
    with_facts_assembly_stack(|| {
        let fixture = Fixture::new(1);
        let mut bindings = fixture.bindings();
        let secret = b"private_key = \"fixture-private-secret-without-terminator\n";
        let path = &bindings.peer_configs[0].path;
        fs::write(path, secret).unwrap();
        bindings.peer_configs[0].sha256 = iroha_crypto::sha256(secret);
        bindings.peer_configs[0].max_bytes = secret.len() as u64;
        let mut entered = false;
        let result = produce_with_hook(
            bindings,
            &fixture.output_path(),
            fixture.genesis(),
            fixture.journal(),
            fixture.verification_limits(),
            fixture.block_store(),
            fixture.merge_log(),
            fixture.reader_limits(),
            fixture.caps(),
            |role, phase| {
                entered |= role == FactsRole::Assembly && phase == Phase::BeforeVerification;
                Ok(())
            },
        );
        assert!(entered);
        let error = result.err().unwrap();
        assert!(!format!("{error}\n{error:?}").contains("fixture-private-secret"));
        assert_no_output(&fixture);
    });
}

#[test]
fn facts_actual_reply_write_and_flush_failures_propagate_without_removing_output() {
    with_facts_assembly_stack(|| {
        struct FailWriter {
            fail_flush: bool,
            written: usize,
        }
        impl std::io::Write for FailWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if !self.fail_flush {
                    return Err(std::io::Error::other("injected reply write"));
                }
                self.written += bytes.len();
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::other("injected reply flush"))
            }
        }
        for fail_flush in [false, true] {
            let fixture = Fixture::new(1);
            let owner = run(&fixture, |_, _| Ok(())).unwrap();
            let identity = owner.identity().unwrap();
            let mut writer = FailWriter {
                fail_flush,
                written: 0,
            };
            assert!(
                owner
                    .finish_reply(|_| {
                        writer.write_all(b"{\"operation\":\"facts\"}\n")?;
                        writer.flush()?;
                        Ok(())
                    })
                    .is_err()
            );
            assert_eq!(writer.written > 0, fail_flush);
            assert_eq!(
                iroha_crypto::sha256(&fs::read(fixture.output_path()).unwrap()),
                identity.raw_sha256
            );
        }
    });
}
