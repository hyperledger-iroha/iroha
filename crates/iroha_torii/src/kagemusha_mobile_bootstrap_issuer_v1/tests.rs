//! Original signed bootstrap selection, explicit clock pins and durable issuer refusal tests.

use std::{fs, os::unix::fs::PermissionsExt as _, path::PathBuf, sync::atomic::Ordering};

use iroha_crypto::Algorithm;
use iroha_data_model::{
    kagemusha::{KagemushaMobileBootstrapApprovalV1, KagemushaMobileBootstrapCheckpointV1},
    sumeragi_finality::SumeragiFinalityCheckpoint,
};

use super::*;

struct Fixture {
    _root: tempfile::TempDir,
    directory: PathBuf,
    keys: Vec<KeyPair>,
    authority: KagemushaBootstrapIssuerAuthorityV1,
    package: KagemushaMobileBootstrapPackageV1,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().canonicalize().unwrap().join("issuer");
        let mut keys: Vec<_> = [41, 42, 43]
            .into_iter()
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
            .collect();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let policy = KagemushaReleaseAuthorityPolicyV1 {
            version: 1,
            authority_set_id: [40; 32],
            threshold: 2,
            authorized_signers: keys.iter().map(|key| key.public_key().clone()).collect(),
        };
        // The actual twice-captured native checkpoint is selected by fresh threshold signatures.
        // This release protocol grants no World execution or monetary-authority claim.
        let finality_checkpoint = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/sumeragi/native-finality/genesis-checkpoint.nrt"
        ))
        .to_vec();
        let authority = KagemushaBootstrapIssuerAuthorityV1 {
            network_id: SumeragiFinalityCheckpoint::decode_canonical(&finality_checkpoint)
                .unwrap()
                .network_id(),
            policy,
            scope: KagemushaMobileBootstrapScopeV1 {
                asset_identity_digest: [4; 32],
                asset_incarnation: [5; 32],
                asset_scale: 2,
                liability_pool_id: [6; 32],
            },
        };
        let checkpoint = KagemushaMobileBootstrapCheckpointV1 {
            version: 1,
            authority_policy_digest: authority.policy.canonical_digest().unwrap(),
            network_id: authority.network_id,
            scope: authority.scope,
            release_id: [7; 32],
            release_attestation_digest: [8; 32],
            finality_checkpoint,
            sequence: 10,
            issued_at_ms: 1_000,
            expires_at_ms: 300_000,
        };
        Self {
            _root: root,
            directory,
            package: signed(checkpoint, &keys[..2]),
            keys,
            authority,
        }
    }

    fn create(&self) -> KagemushaMobileBootstrapIssuerV1 {
        KagemushaMobileBootstrapIssuerV1::create(
            &self.directory,
            self.authority.clone(),
            self.keys[..2].to_vec(),
            &archive(&self.package),
            release(&self.package),
            time(),
        )
        .unwrap()
    }

    fn recover(&self) -> Result<KagemushaMobileBootstrapIssuerV1> {
        KagemushaMobileBootstrapIssuerV1::recover(
            &self.directory,
            self.authority.clone(),
            self.keys[..2].to_vec(),
        )
    }

    fn next(&self) -> KagemushaMobileBootstrapPackageV1 {
        let mut checkpoint = self.package.checkpoint.clone();
        checkpoint.sequence += 1;
        signed(checkpoint, &self.keys[..2])
    }

    fn head(&self) -> Vec<u8> {
        fs::read(self.directory.join(store::HEAD)).unwrap()
    }
}

fn signed(
    checkpoint: KagemushaMobileBootstrapCheckpointV1,
    keys: &[KeyPair],
) -> KagemushaMobileBootstrapPackageV1 {
    let approvals = keys
        .iter()
        .map(|key| KagemushaMobileBootstrapApprovalV1 {
            public_key: key.public_key().clone(),
            signature: SignatureOf::try_new(key.private_key(), &checkpoint.approval_payload())
                .unwrap(),
        })
        .collect();
    KagemushaMobileBootstrapPackageV1 {
        checkpoint,
        approvals,
    }
}

fn archive(package: &KagemushaMobileBootstrapPackageV1) -> Vec<u8> {
    norito::encode_canonical(package).unwrap()
}

fn release(package: &KagemushaMobileBootstrapPackageV1) -> KagemushaBootstrapIssuerReleaseV1 {
    KagemushaBootstrapIssuerReleaseV1 {
        release_id: package.checkpoint.release_id,
        release_attestation_digest: package.checkpoint.release_attestation_digest,
    }
}

fn time() -> KagemushaBootstrapIssuerTimeIntervalV1 {
    // Independent test authority bounds, never inferred from the package or host wall clock.
    KagemushaBootstrapIssuerTimeIntervalV1 {
        lower_ms: 1_500,
        upper_ms: 1_600,
    }
}

fn issue(
    issuer: &KagemushaMobileBootstrapIssuerV1,
    package: &KagemushaMobileBootstrapPackageV1,
) -> Result<KagemushaMobileBootstrapFreshnessPackageV1> {
    issuer.issue(&archive(package), [11; 32], release(package), time())
}

#[test]
fn original_signed_bootstrap_issues_exact_threshold_nonce_bound_freshness() {
    let fixture = Fixture::new();
    let issuer = fixture.create();
    let reply = issue(&issuer, &fixture.package).unwrap();
    assert_eq!(reply.approvals.len(), 2);
    assert_eq!(issuer.signatures_created.load(Ordering::SeqCst), 2);
    let pins = KagemushaMobileBootstrapFreshnessPinsV1 {
        authority_policy: &fixture.authority.policy,
        network_id: fixture.authority.network_id,
        scope: fixture.authority.scope,
        release_id: [7; 32],
        release_attestation_digest: [8; 32],
        minimum_sequence: 10,
        previous: None,
        checkpoint: &fixture.package.checkpoint,
        request_nonce: [11; 32],
        native_elapsed_ms: 50,
    };
    let observation = reply.authenticate(&pins).unwrap();
    assert_eq!(observation.trusted_time_lower_ms, 1_500);
    assert_eq!(observation.trusted_time_upper_ms, 1_650);
    let mut substituted = pins;
    substituted.request_nonce = [12; 32];
    assert!(reply.authenticate(&substituted).is_err());
    let retry = issuer
        .issue(
            &archive(&fixture.package),
            [12; 32],
            release(&fixture.package),
            time(),
        )
        .unwrap();
    retry.authenticate(&substituted).unwrap();
    assert_ne!(reply.statement, retry.statement);
    assert_eq!(fixture.head(), archive(&fixture.package));
    assert_eq!(
        fs::metadata(&fixture.directory)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(fixture.directory.join(store::HEAD))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn sequence_floor_survives_recovery_and_release_changes() {
    let fixture = Fixture::new();
    let issuer = fixture.create();
    let mut next = fixture.next().checkpoint;
    next.release_id = [17; 32];
    next.release_attestation_digest = [18; 32];
    let next = signed(next, &fixture.keys[..2]);
    issue(&issuer, &next).unwrap();
    assert_eq!(fixture.head(), archive(&next));
    drop(issuer);
    let recovered = fixture.recover().unwrap();
    assert!(issue(&recovered, &fixture.package).is_err());
    let mut equivocation = next.checkpoint.clone();
    equivocation.expires_at_ms -= 1;
    assert!(issue(&recovered, &signed(equivocation, &fixture.keys[..2])).is_err());
    assert!(
        recovered
            .issue(&archive(&next), [11; 32], release(&fixture.package), time())
            .is_err()
    );
    assert_eq!(recovered.signatures_created.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.head(), archive(&next));
    assert_eq!(
        issue(&recovered, &next)
            .unwrap()
            .statement
            .retained_sequence,
        11
    );
}

#[test]
fn invalid_authority_or_initial_approval_never_creates_storage() {
    let fixture = Fixture::new();
    let foreign = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519);
    for keys in [
        vec![fixture.keys[0].clone()],
        vec![fixture.keys[0].clone(); 2],
        vec![fixture.keys[0].clone(), foreign],
    ] {
        assert!(
            KagemushaMobileBootstrapIssuerV1::create(
                &fixture.directory,
                fixture.authority.clone(),
                keys,
                &archive(&fixture.package),
                release(&fixture.package),
                time()
            )
            .is_err()
        );
        assert!(!fixture.directory.exists());
    }
    let mut partial = fixture.package.clone();
    partial.approvals.pop();
    assert!(
        KagemushaMobileBootstrapIssuerV1::create(
            &fixture.directory,
            fixture.authority.clone(),
            fixture.keys[..2].to_vec(),
            &archive(&partial),
            release(&partial),
            time()
        )
        .is_err()
    );
    assert!(!fixture.directory.exists());
    assert!(fixture.recover().is_err());
    assert!(!fixture.directory.exists());
}

#[test]
fn invalid_inputs_and_unqualified_time_never_sign_or_advance() {
    let fixture = Fixture::new();
    let issuer = fixture.create();
    let next = fixture.next();
    for bounds in [
        KagemushaBootstrapIssuerTimeIntervalV1 {
            lower_ms: 0,
            upper_ms: 1_600,
        },
        KagemushaBootstrapIssuerTimeIntervalV1 {
            lower_ms: 1_600,
            upper_ms: 1_500,
        },
        KagemushaBootstrapIssuerTimeIntervalV1 {
            lower_ms: 999,
            upper_ms: 1_500,
        },
        KagemushaBootstrapIssuerTimeIntervalV1 {
            lower_ms: 1_500,
            upper_ms: 300_000,
        },
    ] {
        assert!(
            issuer
                .issue(&archive(&next), [11; 32], release(&next), bounds)
                .is_err()
        );
    }
    assert!(
        issuer
            .issue(&archive(&next), [0; 32], release(&next), time())
            .is_err()
    );
    assert!(
        issuer
            .issue(&[1, 2, 3], [11; 32], release(&next), time())
            .is_err()
    );
    let mut substituted = next.clone();
    substituted.checkpoint.scope.asset_incarnation = [19; 32];
    let substituted = signed(substituted.checkpoint, &fixture.keys[..2]);
    assert!(issue(&issuer, &substituted).is_err());
    let mut unsigned_change = next.clone();
    unsigned_change.checkpoint.expires_at_ms -= 1;
    assert!(issue(&issuer, &unsigned_change).is_err());
    assert_eq!(issuer.signatures_created.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.head(), archive(&fixture.package));
    assert_eq!(
        issue(&issuer, &next).unwrap().statement.retained_sequence,
        11
    );
}

#[test]
fn interrupted_durability_poisoning_requires_authenticated_recovery_before_signing() {
    for stage in 1..=3 {
        let fixture = Fixture::new();
        let issuer = fixture.create();
        let next = fixture.next();
        issuer.state.lock().unwrap().store.fail_next_write_stage = stage;
        assert!(issue(&issuer, &next).is_err());
        assert!(issue(&issuer, &next).is_err());
        assert_eq!(issuer.signatures_created.load(Ordering::SeqCst), 0);
        let expected = if stage == 1 { &fixture.package } else { &next };
        assert_eq!(fixture.head(), archive(expected));
        if stage == 1 {
            assert_eq!(
                fs::read(fixture.directory.join(store::TEMP)).unwrap(),
                archive(&next)
            );
        }
        drop(issuer);
        let recovered = fixture.recover().unwrap();
        assert_eq!(
            recovered.state.lock().unwrap().retained.sequence,
            expected.checkpoint.sequence
        );
        assert!(!fixture.directory.join(store::TEMP).exists());
        assert_eq!(
            issue(&recovered, &next)
                .unwrap()
                .statement
                .retained_sequence,
            11
        );
        assert_eq!(fixture.head(), archive(&next));
    }
}

#[test]
fn corrupt_head_refuses_signing_and_recovery_preserves_unpublished_bytes() {
    let fixture = Fixture::new();
    let issuer = fixture.create();
    issuer.state.lock().unwrap().store.fail_next_write_stage = 1;
    let next = fixture.next();
    assert!(issue(&issuer, &next).is_err());
    fs::write(
        fixture.directory.join(store::HEAD),
        b"invalid original head",
    )
    .unwrap();
    assert!(issue(&issuer, &fixture.package).is_err());
    assert_eq!(issuer.signatures_created.load(Ordering::SeqCst), 0);
    drop(issuer);
    assert!(fixture.recover().is_err());
    assert_eq!(fixture.head(), b"invalid original head");
    assert_eq!(
        fs::read(fixture.directory.join(store::TEMP)).unwrap(),
        archive(&next)
    );
}

#[test]
fn live_head_substitution_poisoning_and_exclusive_ownership_fail_closed() {
    let fixture = Fixture::new();
    let issuer = fixture.create();
    assert!(fixture.recover().is_err());
    assert!(
        KagemushaMobileBootstrapIssuerV1::create(
            &fixture.directory,
            fixture.authority.clone(),
            fixture.keys[..2].to_vec(),
            &archive(&fixture.package),
            release(&fixture.package),
            time()
        )
        .is_err()
    );
    let next = archive(&fixture.next());
    fs::write(fixture.directory.join(store::HEAD), &next).unwrap();
    assert!(issue(&issuer, &fixture.package).is_err());
    fs::write(
        fixture.directory.join(store::HEAD),
        archive(&fixture.package),
    )
    .unwrap();
    assert!(issue(&issuer, &fixture.package).is_err());
    assert_eq!(issuer.signatures_created.load(Ordering::SeqCst), 0);
    drop(issuer);
    issue(&fixture.recover().unwrap(), &fixture.package).unwrap();
}

#[test]
fn recovery_rejects_missing_unsafe_and_aliased_files_without_repair() {
    for variant in 0..5 {
        let fixture = Fixture::new();
        drop(fixture.create());
        let head = fixture.directory.join(store::HEAD);
        let original = fixture.head();
        let backup = fixture.directory.parent().unwrap().join("original.norito");
        match variant {
            0 => {
                fs::rename(&head, &backup).unwrap();
            }
            1 => {
                fs::rename(&head, &backup).unwrap();
                std::os::unix::fs::symlink(&backup, &head).unwrap();
            }
            2 => {
                fs::hard_link(&head, &backup).unwrap();
            }
            3 => {
                fs::set_permissions(&head, fs::Permissions::from_mode(0o644)).unwrap();
            }
            _ => {
                fs::write(fixture.directory.join("unexpected"), b"preserve").unwrap();
            }
        }
        assert!(fixture.recover().is_err(), "variant {variant}");
        assert_eq!(
            fs::read(if variant <= 2 { &backup } else { &head }).unwrap(),
            original
        );
        if variant == 0 {
            assert!(!head.exists());
        }
        if variant == 1 {
            assert!(
                fs::symlink_metadata(&head)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
        }
        if variant == 4 {
            assert_eq!(
                fs::read(fixture.directory.join("unexpected")).unwrap(),
                b"preserve"
            );
        }
    }
}

#[test]
fn foreign_process_is_refused_before_touching_an_inherited_mutex() {
    let fixture = Fixture::new();
    let mut issuer = fixture.create();
    issuer.process_id = issuer.process_id.wrapping_add(1);
    let locked = issuer.state.lock().unwrap();
    std::thread::scope(|scope| {
        let (send, receive) = std::sync::mpsc::channel();
        let issuer = &issuer;
        let package = &fixture.package;
        scope.spawn(move || send.send(issue(issuer, package)).unwrap());
        let result = receive.recv_timeout(std::time::Duration::from_secs(2));
        // Release before asserting so a regression cannot leave the scoped worker blocked.
        drop(locked);
        assert!(
            result
                .expect("refuse without acquiring the inherited lock")
                .unwrap_err()
                .contains("another process")
        );
    });
    assert_eq!(issuer.signatures_created.load(Ordering::SeqCst), 0);
}
