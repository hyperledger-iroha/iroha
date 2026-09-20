//! Retained original-facts and two-output publication controls over signed fixtures.

use super::super::prepare_pair::{PrepareRole, prepare_with_hook};
use super::*;

fn write_prepare_facts(disk: &Disk) -> ProofInputBinding {
    let bytes = crate::kura::scaling_evidence::export::launcher::prepare::tests::encode_facts(
        disk.signed.plan(),
        disk.limits(),
        disk.supplied(),
    );
    fs::write(&disk.files.input, bytes).unwrap();
    fs::set_permissions(&disk.files.input, fs::Permissions::from_mode(0o600)).unwrap();
    binding(&disk.files.input)
}
fn prepare_paths(disk: &Disk) -> (PathBuf, PathBuf) {
    (
        disk.files.parent.join("launcher.norito"),
        disk.files.parent.join("supplied.norito"),
    )
}
fn pair_caps(facts: &ProofInputBinding) -> PrepareOutputCaps {
    PrepareOutputCaps {
        request_bytes: 1024 * 1024,
        bundle_bytes: 1024 * 1024,
        total_bytes: facts.max_bytes + 2 * 1024 * 1024,
    }
}
fn run_preparation(
    disk: &Disk,
    facts: ProofInputBinding,
    mut hook: impl FnMut(PrepareRole, Phase) -> Result<()>,
) -> Result<PreparedLaunch> {
    let (request, bundle) = prepare_paths(disk);
    let outputs =
        PreparedOutputPair::admit_with_hook(&request, &bundle, pair_caps(&facts), &mut hook)?;
    prepare_with_hook(facts, outputs, hook)
}
fn change_bytes(path: &Path) {
    let mut bytes = if path.exists() {
        fs::read(path).unwrap()
    } else {
        vec![]
    };
    if bytes.is_empty() {
        bytes.push(1);
    } else {
        bytes[0] ^= 1;
    }
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn transport_path(disk: &Disk, role: PrepareRole) -> PathBuf {
    let (request, bundle) = prepare_paths(disk);
    let path = match role {
        PrepareRole::Request => request,
        PrepareRole::Bundle => bundle,
        _ => panic!("expected a transport role"),
    };
    if path.exists() {
        path
    } else {
        path.with_file_name(format!(
            "{}.publishing",
            path.file_name().unwrap().to_str().unwrap()
        ))
    }
}
fn preparation_events() -> Vec<(PrepareRole, Phase)> {
    let disk = Disk::new(4);
    let facts = write_prepare_facts(&disk);
    let mut events = Vec::new();
    let owner = run_preparation(&disk, facts, |role, phase| {
        events.push((role, phase));
        Ok(())
    })
    .unwrap();
    owner.identity().unwrap();
    for phase in [
        Phase::ParentAdmitted,
        Phase::ParentRetained,
        Phase::InputAdmitted,
        Phase::InputRetained,
        Phase::BeforeRead,
        Phase::AfterRead,
        Phase::BeforeInputFinish,
    ] {
        assert!(
            events.contains(&(PrepareRole::Facts, phase)),
            "missing facts boundary {phase:?}"
        );
    }
    for role in [PrepareRole::Request, PrepareRole::Bundle] {
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
                events.contains(&(role, phase)),
                "missing output boundary {role:?}/{phase:?}"
            );
        }
    }
    for phase in [
        Phase::BeforeVerification,
        Phase::AfterVerification,
        Phase::BeforeIdentity,
        Phase::AfterIdentity,
    ] {
        assert!(
            events.contains(&(PrepareRole::Pair, phase)),
            "missing pair boundary {phase:?}"
        );
    }
    events
}

#[test]
fn prepared_pair_retains_exact_one_and_four_lane_facts_and_both_replayable_outputs() {
    for lanes in [1, 4] {
        let disk = Disk::new(lanes);
        let facts = write_prepare_facts(&disk);
        let original = fs::read(&facts.path).unwrap();
        let expected_facts = PreparedTransportIdentity {
            raw_sha256: facts.sha256,
            byte_length: original.len() as u64,
        };
        let (request, bundle) = prepare_paths(&disk);
        let caps = pair_caps(&facts);
        let pair = PreparedOutputPair::admit(&request, &bundle, caps).unwrap();
        let owner = prepare_bound(facts, pair).unwrap();
        let identity = owner.identity().unwrap();
        assert_eq!(identity.facts, expected_facts);
        for (path, value) in [(&request, identity.request), (&bundle, identity.bundle)] {
            let bytes = fs::read(path).unwrap();
            assert_eq!(value.raw_sha256, iroha_crypto::sha256(&bytes));
            assert_eq!(value.byte_length, bytes.len() as u64);
            assert_eq!(fs::metadata(path).unwrap().mode() & 0o7777, 0o600);
            assert_eq!(fs::metadata(path).unwrap().nlink(), 1);
            assert!(
                !path
                    .with_file_name(format!(
                        "{}.publishing",
                        path.file_name().unwrap().to_str().unwrap()
                    ))
                    .exists()
            );
        }
        assert_ne!(
            fs::metadata(&request).unwrap().ino(),
            fs::metadata(&bundle).unwrap().ino()
        );
        let proof = export_bound_request(
            open_launcher(binding(&request)).unwrap(),
            &disk.root,
            &disk.log,
            disk.reader_limits(),
            binding(&bundle),
        )
        .unwrap();
        let proof_identity = proof.identity().unwrap();
        let published = ProofOutput::admit(&disk.files.output(), 2 * 1024 * 1024)
            .unwrap()
            .publish(proof)
            .unwrap();
        assert_eq!(published.identity().unwrap(), proof_identity);
        let replayed = replay_bound_request(
            open_launcher(binding(&request)).unwrap(),
            proof_identity.iroha_hash,
            binding(&disk.files.output()),
        )
        .unwrap();
        assert_eq!(replayed.identity().unwrap(), proof_identity);
        assert_eq!(fs::read(&disk.files.input).unwrap(), original);
        assert_eq!(owner.identity().unwrap(), identity);
    }
}

#[test]
fn preparation_facts_mutation_at_every_observed_pair_boundary_rejects_success() {
    let events = preparation_events();
    for (target, expected) in events.iter().enumerate() {
        let disk = Disk::new(4);
        let facts = write_prepare_facts(&disk);
        let mut index = 0;
        let mut fired = false;
        let result = run_preparation(&disk, facts, |role, phase| {
            if index == target {
                assert_eq!((role, phase), *expected);
                change_bytes(&disk.files.input);
                fired = true;
            }
            index += 1;
            Ok(())
        });
        assert!(
            fired,
            "facts mutation boundary was not reached: {target}/{expected:?}"
        );
        assert!(
            result.is_err(),
            "changed original facts were accepted at {target}/{expected:?}"
        );
        assert!(
            disk.files.input.exists(),
            "rejected original facts must not be cleaned up"
        );
    }
}

#[test]
fn preparation_output_mutation_at_every_observed_output_boundary_rejects_success() {
    let events = preparation_events();
    for (target, expected) in events
        .iter()
        .enumerate()
        .filter(|(_, (role, _))| matches!(role, PrepareRole::Request | PrepareRole::Bundle))
    {
        let disk = Disk::new(4);
        let facts = write_prepare_facts(&disk);
        let mut index = 0;
        let mut changed = None;
        let result = run_preparation(&disk, facts, |role, phase| {
            if index == target {
                assert_eq!((role, phase), *expected);
                let path = transport_path(&disk, role);
                change_bytes(&path);
                changed = Some(path);
            }
            index += 1;
            Ok(())
        });
        let changed = changed.expect("output boundary reached");
        assert!(
            result.is_err(),
            "changed output accepted at {target}/{expected:?}"
        );
        assert!(
            changed.exists(),
            "rejected output evidence must be preserved"
        );
    }
}

#[test]
fn preparation_final_pair_boundaries_reject_either_changed_output() {
    for phase in [Phase::BeforeIdentity, Phase::AfterIdentity] {
        for role in [PrepareRole::Request, PrepareRole::Bundle] {
            let disk = Disk::new(4);
            let facts = write_prepare_facts(&disk);
            let mut fired = false;
            let result = run_preparation(&disk, facts, |seen_role, seen_phase| {
                if (seen_role, seen_phase) == (PrepareRole::Pair, phase) {
                    change_bytes(&transport_path(&disk, role));
                    fired = true;
                }
                Ok(())
            });
            assert!(fired);
            assert!(result.is_err(), "changed final {role:?} survived {phase:?}");
        }
    }
}

#[test]
fn preparation_ancestor_rename_at_admission_read_and_both_publications_rejects_success() {
    for target in [
        (PrepareRole::Facts, Phase::InputRetained),
        (PrepareRole::Facts, Phase::AfterRead),
        (PrepareRole::Pair, Phase::AfterVerification),
        (PrepareRole::Request, Phase::BeforeCreate),
        (PrepareRole::Request, Phase::AfterCreate),
        (PrepareRole::Request, Phase::BeforeRename),
        (PrepareRole::Request, Phase::AfterRename),
        (PrepareRole::Bundle, Phase::BeforeCreate),
        (PrepareRole::Bundle, Phase::AfterFileSync),
        (PrepareRole::Bundle, Phase::RenameReady),
        (PrepareRole::Bundle, Phase::AfterRename),
        (PrepareRole::Pair, Phase::BeforeIdentity),
        (PrepareRole::Pair, Phase::AfterIdentity),
    ] {
        let disk = Disk::new(4);
        let facts = write_prepare_facts(&disk);
        let mut fired = false;
        let result = run_preparation(&disk, facts, |role, phase| {
            if !fired && (role, phase) == target {
                disk.files.replace_parent(true);
                fired = true;
            }
            Ok(())
        });
        assert!(fired);
        assert!(
            result.is_err(),
            "renamed original namespace accepted at {target:?}"
        );
        assert!(disk.files.ancestor.with_extension("old").exists());
    }
}

#[test]
fn preparation_second_publication_failure_preserves_first_and_all_remaining_evidence() {
    for phase in [Phase::BeforeRename, Phase::AfterRename] {
        let disk = Disk::new(4);
        let facts = write_prepare_facts(&disk);
        let original = fs::read(&facts.path).unwrap();
        let mut fired = false;
        let result = run_preparation(&disk, facts, |role, seen| {
            if (role, seen) == (PrepareRole::Bundle, phase) {
                fired = true;
                return Err(eyre!("injected second publication durability failure"));
            }
            Ok(())
        });
        assert!(fired);
        assert!(result.is_err());
        let (request, bundle) = prepare_paths(&disk);
        assert!(
            request.exists(),
            "first published output must survive failure"
        );
        assert!(!fs::read(&request).unwrap().is_empty());
        let surviving_bundle = if phase == Phase::BeforeRename {
            bundle.with_file_name("supplied.norito.publishing")
        } else {
            bundle.clone()
        };
        assert!(surviving_bundle.exists());
        assert!(!fs::read(&surviving_bundle).unwrap().is_empty());
        assert_eq!(fs::read(&disk.files.input).unwrap(), original);
        let retry_facts = binding(&disk.files.input);
        assert!(PreparedOutputPair::admit(&request, &bundle, pair_caps(&retry_facts)).is_err());
    }
}

#[test]
fn prepared_pair_requires_distinct_absent_names_and_rejects_existing_inode_aliases() {
    for alias in 0..4 {
        let disk = Disk::new(4);
        let facts = write_prepare_facts(&disk);
        let (request, mut bundle) = prepare_paths(&disk);
        match alias {
            0 => bundle = request.clone(),
            1 => bundle = request.with_file_name("launcher.norito.publishing"),
            2 => fs::hard_link(&disk.files.input, &request).unwrap(),
            3 => symlink(&disk.files.input, &bundle).unwrap(),
            _ => unreachable!(),
        }
        assert!(PreparedOutputPair::admit(&request, &bundle, pair_caps(&facts)).is_err());
        assert!(disk.files.input.exists());
    }
    let disk = Disk::new(4);
    let facts = write_prepare_facts(&disk);
    let (request, bundle) = prepare_paths(&disk);
    let stage_binding = ProofInputBinding {
        path: request.with_file_name("launcher.norito.publishing"),
        sha256: facts.sha256,
        max_bytes: facts.max_bytes,
    };
    let pair = PreparedOutputPair::admit(&request, &bundle, pair_caps(&facts)).unwrap();
    let mut reads = 0;
    assert!(
        prepare_with_hook(stage_binding, pair, |_, phase| {
            if phase == Phase::BeforeRead {
                reads += 1;
            }
            Ok(())
        })
        .is_err()
    );
    assert_eq!(
        reads, 0,
        "a conflicting facts path must fail before reading"
    );
}

#[test]
fn preparation_total_reservation_overflow_rejects_before_facts_admission_or_read() {
    let disk = Disk::new(4);
    let facts = write_prepare_facts(&disk);
    let (request, bundle) = prepare_paths(&disk);
    let mut caps = pair_caps(&facts);
    caps.total_bytes -= 1;
    let pair = PreparedOutputPair::admit(&request, &bundle, caps).unwrap();
    let mut input_events = 0;
    assert!(
        prepare_with_hook(facts, pair, |role, _| {
            if role == PrepareRole::Facts {
                input_events += 1;
            }
            Ok(())
        })
        .is_err()
    );
    assert_eq!(input_events, 0);
    assert!(!request.exists() && !bundle.exists());
    assert!(
        !request
            .with_file_name("launcher.norito.publishing")
            .exists()
    );
    let invalid = PrepareOutputCaps {
        request_bytes: u64::MAX,
        bundle_bytes: 1,
        total_bytes: MAX_INPUT_BYTES,
    };
    assert!(PreparedOutputPair::admit(&request, &bundle, invalid).is_err());
}

#[test]
fn preparation_wrong_raw_digest_and_noncanonical_original_facts_cannot_publish() {
    for wrong_digest in [true, false] {
        let disk = Disk::new(4);
        let mut facts = write_prepare_facts(&disk);
        if wrong_digest {
            facts.sha256[0] ^= 1;
        } else {
            let mut bytes = fs::read(&facts.path).unwrap();
            bytes.push(0);
            fs::write(&facts.path, bytes).unwrap();
            facts = binding(&facts.path);
        }
        assert!(run_preparation(&disk, facts, |_, _| Ok(())).is_err());
        let (request, bundle) = prepare_paths(&disk);
        assert!(!request.exists() && !bundle.exists());
        assert!(
            !request
                .with_file_name("launcher.norito.publishing")
                .exists()
        );
        assert!(!bundle.with_file_name("supplied.norito.publishing").exists());
    }
}

#[test]
fn prepared_pair_late_mutation_of_any_retained_role_rejects_identity_and_poisoned_retry() {
    for role in [
        PrepareRole::Facts,
        PrepareRole::Request,
        PrepareRole::Bundle,
    ] {
        let disk = Disk::new(4);
        let facts = write_prepare_facts(&disk);
        let owner = run_preparation(&disk, facts, |_, _| Ok(())).unwrap();
        owner.identity().unwrap();
        let path = if role == PrepareRole::Facts {
            disk.files.input.clone()
        } else {
            transport_path(&disk, role)
        };
        let original = fs::read(&path).unwrap();
        change_bytes(&path);
        assert!(owner.identity().is_err());
        fs::write(&path, original).unwrap();
        assert!(
            owner
                .identity()
                .unwrap_err()
                .to_string()
                .contains("poisoned")
        );
    }
}

#[test]
fn prepared_pair_identity_failure_or_panic_permanently_poison_after_catch() {
    for panic in [false, true] {
        for phase in [Phase::BeforeIdentity, Phase::AfterIdentity] {
            let disk = Disk::new(4);
            let facts = write_prepare_facts(&disk);
            let owner = run_preparation(&disk, facts, |_, _| Ok(())).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                owner.identity_with_hook(|_, seen| {
                    if seen == phase {
                        if panic {
                            panic!("injected final reply census panic");
                        }
                        return Err(eyre!("injected final reply census failure"));
                    }
                    Ok(())
                })
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert!(
                owner
                    .identity()
                    .unwrap_err()
                    .to_string()
                    .contains("poisoned")
            );
        }
    }
}

#[test]
fn prepared_pair_identity_census_detects_mutation_during_final_reply_boundaries() {
    for phase in [Phase::BeforeIdentity, Phase::AfterIdentity] {
        for role in [
            PrepareRole::Facts,
            PrepareRole::Request,
            PrepareRole::Bundle,
        ] {
            let disk = Disk::new(4);
            let facts = write_prepare_facts(&disk);
            let owner = run_preparation(&disk, facts, |_, _| Ok(())).unwrap();
            let mut fired = false;
            let result = owner.identity_with_hook(|_, seen| {
                if phase == seen {
                    let path = if role == PrepareRole::Facts {
                        disk.files.input.clone()
                    } else {
                        transport_path(&disk, role)
                    };
                    change_bytes(&path);
                    fired = true;
                }
                Ok(())
            });
            assert!(fired && result.is_err());
            assert!(
                owner
                    .identity()
                    .unwrap_err()
                    .to_string()
                    .contains("poisoned")
            );
        }
    }
}
