//! Genuine quorum boundary checks and durable read-only retention restart controls.

use super::super::finality as deployment_finality;
use super::*;
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus_v2::{self as wire, *},
    },
    bridge::{BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeFinalityProof},
    isi::kagemusha_v1::{InstalledBeaconEpochBindingV1, KAGEMUSHA_CHAIN_VERSION_V1},
};
use iroha_model_base::peer::PeerId;
use std::{num::NonZeroU64, os::unix::fs::PermissionsExt as _, sync::OnceLock};

pub(super) struct Fixture {
    pub(super) trust: DeploymentTrustV1,
    pub(super) previous: BridgeFinalityProof,
    pub(super) next: BridgeFinalityProof,
    pub(super) receipt: RetentionCompletionV1,
    keys: Vec<KeyPair>,
}

fn sign_certificate(certificate: &mut QuorumCertificate, keys: &[KeyPair], signers: &[u32]) {
    certificate.signers = signers.to_vec();
    let preimage = Vote {
        round: certificate.round,
        proposal_round: certificate.proposal_round,
        phase: certificate.phase,
        subject: certificate.subject,
        execution_commitment: certificate.execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let signatures = signers
        .iter()
        .map(|index| {
            Signature::try_new(keys[*index as usize].private_key(), &preimage)
                .unwrap()
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .unwrap();
}

pub(super) fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let trust = deployment_finality::test_trust();
        let genesis = crate::taira_public_reset::deployment_validated_genesis_fixture();
        let network_id = NetworkId::from_genesis_hash(genesis.expected_hash());
        let mut keys = (110..114)
            .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
        let roster = keys
            .iter()
            .map(|key| ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let pops = keys
            .iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
            .collect::<Vec<_>>();
        let authority = genesis
            .consensus_metadata()
            .kagemusha_mint_finality
            .authority_generation
            .bind_network_id(network_id)
            .unwrap();
        let initial = KagemushaMintFinalityEpochAuthorizationV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            network_id,
            epoch: 0,
            first_height: 1,
            last_height: 20,
            authority_generation: authority.generation,
            authority_id: authority.authority_id().unwrap(),
            beacon: BeaconEpochBindingV1::Bootstrap,
            previous_authorization_id: [0; 32],
            transition_id: [0; 32],
            decision: KagemushaMintFinalityEpochDecisionV1::Genesis,
        };
        let successor = KagemushaMintFinalityEpochAuthorizationV1 {
            epoch: 1,
            first_height: 21,
            last_height: 40,
            beacon: BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
                session_id: [0x71; 32],
                transcript_hash: [0x72; 32],
            }),
            previous_authorization_id: initial.authorization_id().unwrap(),
            decision: KagemushaMintFinalityEpochDecisionV1::Retain,
            ..initial
        };
        successor.validate_successor(&initial).unwrap();
        let mut proofs: Vec<BridgeFinalityProof> = Vec::new();
        for height in 1..=21 {
            let previous = proofs.last();
            let header = if height == 1 {
                genesis.block().header().clone()
            } else {
                BlockHeader::new(
                    NonZeroU64::new(height).unwrap(),
                    previous.map(|proof| proof.block_header.hash()),
                    None,
                    u64::try_from(genesis.block().header().creation_time().as_millis()).unwrap()
                        + height,
                    0,
                )
            };
            let authorization = if height <= 20 { initial } else { successor };
            let context = HeightContext {
                network_id,
                protocol_version: PROTOCOL_VERSION,
                height,
                epoch: authorization.epoch,
                epoch_end_height: authorization.last_height,
                kagemusha_mint_finality_authorization: authorization,
                kagemusha_mint_finality_authority: authority.clone(),
                mode: ConsensusMode::Npos,
                next_epoch_snapshot: if height == 20 {
                    Some(wire::finality::FinalizedNextEpochSnapshot {
                        epoch: 1,
                        kagemusha_mint_finality_authorization: successor,
                        kagemusha_mint_finality_authority: authority.clone(),
                        epoch_end_height: 40,
                        mode: ConsensusMode::Npos,
                        quorum: DualQuorum::from_roster(&roster).unwrap(),
                        roster: roster.clone(),
                        validator_set_pops: pops.clone(),
                        leader_seed: [0x55; 32],
                    })
                } else {
                    None
                },
                parent_commit_qc: previous.map(|proof| proof.finality_artifact.commit_qc.clone()),
                snapshot_bootstrap: None,
                roster: roster.clone(),
                quorum: DualQuorum::from_roster(&roster).unwrap(),
                nexus_amx_context_hash: Hash::new(b"retention nexus"),
                execution_policy_hash: Hash::new(b"retention execution"),
                da_layout: DataAvailabilityLayout {
                    encoding: PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1024,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 4096,
                    max_chunk_count: 8,
                },
                leader_seed: [0x55; 32],
            };
            context.validate().unwrap();
            let subject = BlockSubject {
                parent_block_hash: header.prev_block_hash(),
                block_hash: header.hash(),
                payload_hash: Hash::new(b"retention fixture payload"),
            };
            let round = ConsensusRound {
                context_id: context.id(),
                height,
                view: 0,
            };
            let mut qc = QuorumCertificate {
                round,
                proposal_round: round,
                phase: GlobalPhase::Commit,
                subject,
                execution_commitment:
                    ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                        Hash::new(b"parent"),
                        Hash::new(b"post"),
                        Hash::new(b"writes"),
                        1,
                        Hash::new(b"executed"),
                    ),
                signers: Vec::new(),
                aggregate_signature: Vec::new(),
            };
            sign_certificate(&mut qc, &keys, &[0, 1, 2]);
            let finality_artifact =
                wire::finality::V2FinalityArtifact::new(context, subject, qc, pops.clone());
            finality_artifact.verify().unwrap();
            proofs.push(BridgeFinalityProof {
                version: BRIDGE_FINALITY_PROOF_VERSION_V2,
                block_header: header,
                finality_artifact,
            });
        }
        let next = proofs.pop().unwrap();
        let previous = proofs.pop().unwrap();
        let receipt =
            make_retention_receipt(&previous, &next, &[initial, successor], None).unwrap();
        Fixture {
            trust,
            previous,
            next,
            receipt,
            keys,
        }
    })
}

#[cfg(unix)]
fn supervisor_guard_fixture() -> (tempfile::TempDir, PathBuf, NetworkId, PathBuf) {
    use std::os::unix::fs::PermissionsExt as _;
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let parent = directory.path().canonicalize().unwrap();
    let network = deployment_finality::test_network_id();
    let child = parent.join(format!("epoch-worker-{network}"));
    (directory, parent, network, child)
}

#[cfg(unix)]
#[test]
fn epoch_supervisor_journal_guard_excludes_active_worker_until_drop() {
    let (_directory, parent, network, child) = supervisor_guard_fixture();
    let worker = Journal::open(&child, true).unwrap();
    assert!(supervisor_journal_guard(&parent, network).is_err());
    drop(worker);
    let guard = supervisor_journal_guard(&parent, network).unwrap();
    guard.revalidate().unwrap();
    assert!(Journal::open(&child, false).is_err());
    drop(guard);
    Journal::open(&child, false).unwrap();
}

#[cfg(unix)]
#[test]
fn epoch_supervisor_journal_guard_absence_is_read_only_and_revalidated() {
    let (_directory, parent, network, child) = supervisor_guard_fixture();
    let guard = supervisor_journal_guard(&parent, network).unwrap();
    guard.revalidate().unwrap();
    assert!(!child.exists());
    assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
    let worker = Journal::open(&child, true).unwrap();
    assert!(guard.revalidate().is_err());
    drop(worker);
}

#[cfg(unix)]
#[test]
fn epoch_supervisor_journal_guard_never_repairs_missing_lock() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_directory, parent, network, child) = supervisor_guard_fixture();
    fs::create_dir(&child).unwrap();
    fs::set_permissions(&child, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(supervisor_journal_guard(&parent, network).is_err());
    assert_eq!(fs::read_dir(&child).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn epoch_supervisor_journal_guard_rejects_symlink_parent_and_child() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let (_directory, parent, network, child) = supervisor_guard_fixture();
    let real = parent.join("real");
    fs::create_dir(&real).unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o700)).unwrap();
    let alias = parent.join("alias");
    symlink(&real, &alias).unwrap();
    assert!(supervisor_journal_guard(&alias, network).is_err());
    symlink(&real, &child).unwrap();
    assert!(supervisor_journal_guard(&parent, network).is_err());
    assert!(!real.join("lock").exists());
}

#[cfg(unix)]
#[test]
fn epoch_supervisor_journal_guard_rejects_parent_and_child_rebinding() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_directory, base, network, _) = supervisor_guard_fixture();
    let parent = base.join("journals");
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    let absent = supervisor_journal_guard(&parent, network).unwrap();
    fs::rename(&parent, base.join("original-parent")).unwrap();
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(absent.revalidate().is_err());
    drop(absent);
    let child = parent.join(format!("epoch-worker-{network}"));
    drop(Journal::open(&child, true).unwrap());
    let guard = supervisor_journal_guard(&parent, network).unwrap();
    fs::rename(&child, parent.join("original-child")).unwrap();
    let replacement = Journal::open(&child, true).unwrap();
    assert!(guard.revalidate().is_err());
    drop(replacement);
}

#[test]
fn automatic_retention_requires_actual_successor_and_complete_unchanged_authority() {
    let f = fixture();
    verify_retained_boundary(&f.previous, &f.next).unwrap();
    assert_eq!(f.receipt.completed_epoch, 1);
    assert_eq!(f.receipt.observed_height, 21);
    assert_eq!(f.receipt.authority_generation, 0);
    assert_eq!(f.receipt.authorization_chain.len(), 2);
    assert_eq!(
        f.receipt.authority,
        f.previous
            .finality_artifact
            .height_context
            .kagemusha_mint_finality_authority
    );
    assert!(
        verify_retained_boundary(&f.previous, &f.previous).is_err(),
        "boundary snapshot is not actual epoch advancement"
    );
    for mutation in 0..8 {
        let mut bad = f.next.clone();
        let context = &mut bad.finality_artifact.height_context;
        match mutation {
            0 => {
                context
                    .kagemusha_mint_finality_authorization
                    .previous_authorization_id = [9; 32]
            }
            1 => {
                context
                    .kagemusha_mint_finality_authorization
                    .authority_generation = 1
            }
            2 => context.kagemusha_mint_finality_authority.validators[0].ep_proof_public_key = [0; 32],
            3 => context.roster.swap(0, 1),
            4 => context.height += 1,
            5 => {
                context.kagemusha_mint_finality_authorization.decision =
                    KagemushaMintFinalityEpochDecisionV1::RetainAndCancel
            }
            6 => context.parent_commit_qc = None,
            _ => {
                context
                    .parent_commit_qc
                    .as_mut()
                    .unwrap()
                    .execution_commitment
                    .post_state_root = Hash::new(b"other state")
            }
        }
        assert!(
            verify_retained_boundary(&f.previous, &bad).is_err(),
            "mutation {mutation}"
        );
    }
    let mut boundary = f.previous.clone();
    boundary
        .finality_artifact
        .height_context
        .next_epoch_snapshot = None;
    assert!(verify_retained_boundary(&boundary, &f.next).is_err());
    assert!(
        make_retention_receipt(
            &f.previous,
            &f.next,
            &f.receipt.authorization_chain[1..],
            None
        )
        .is_err()
    );
}

#[test]
fn retention_cursor_is_independent_of_valid_quorum_witness_and_unchanged_reproposal() {
    let f = fixture();
    let mut next = f.next.clone();
    let parent = next
        .finality_artifact
        .height_context
        .parent_commit_qc
        .as_mut()
        .unwrap();
    parent.round.view = 3;
    parent.proposal_round = parent.round;
    sign_certificate(parent, &f.keys, &[0, 2, 3]);
    wire::finality::verify_quorum_certificate_with_validator_pops(
        &f.previous.finality_artifact.height_context,
        parent,
        &f.previous.finality_artifact.validator_set_pops,
    )
    .unwrap();
    let receipt =
        make_retention_receipt(&f.previous, &next, &f.receipt.authorization_chain, None).unwrap();
    assert_eq!(
        receipt, f.receipt,
        "equivalent genuine parent witnesses preserve durable restart identity"
    );
    let mut changed = f.receipt.clone();
    changed.authority_generation += 1;
    assert_ne!(changed.computed_id().unwrap(), f.receipt.cursor_id);
}

#[test]
fn retention_restart_reauthenticates_immutable_receipt_original_trust_and_cursor() {
    let f = fixture();
    let directory = crate::taira_public_reset::private_custody_test_dir("retention-journal-");
    let parent = directory.path().canonicalize().unwrap();
    let path = retention_path(&parent, f.receipt.network_id);
    let journal = open_initializing_journal(&path, true).unwrap();
    retain_receipts(
        &journal,
        std::slice::from_ref(&f.receipt),
        &f.trust,
        f.receipt.network_id,
    )
    .unwrap();
    let receipt_bytes = journal.read_optional("epoch-1.json").unwrap().unwrap();
    let cursor_bytes = journal.read_optional("cursor.json").unwrap().unwrap();
    assert!(
        Journal::open(&path, false).is_err(),
        "a live observer exclusively owns cursor writes"
    );
    drop(journal);
    let resumed = Journal::open(&path, false).unwrap();
    retain_receipts(
        &resumed,
        std::slice::from_ref(&f.receipt),
        &f.trust,
        f.receipt.network_id,
    )
    .unwrap();
    assert_eq!(
        resumed.read_optional("epoch-1.json").unwrap().unwrap(),
        receipt_bytes
    );
    assert_eq!(
        resumed.read_optional("cursor.json").unwrap().unwrap(),
        cursor_bytes
    );
    read_retained_completion(&parent, f.receipt.network_id, &f.receipt, &f.trust).unwrap();
    assert!(
        !path.join("submitted.json").exists(),
        "retention never dispatches a ledger transaction"
    );
    assert!(
        !path.join("prepared.json").exists(),
        "retention requires no synthetic preparation"
    );
    let mut cursor: RetentionCursorV1 = resumed.read_json("cursor.json").unwrap();
    cursor.cursor_id = "0".repeat(64);
    fs::write(path.join("cursor.json"), json::to_vec(&cursor).unwrap()).unwrap();
    assert!(
        retain_receipts(
            &resumed,
            std::slice::from_ref(&f.receipt),
            &f.trust,
            f.receipt.network_id
        )
        .is_err()
    );
    let mut changed = f.receipt.clone();
    changed.observed_height += 1;
    fs::write(path.join("epoch-1.json"), json::to_vec(&changed).unwrap()).unwrap();
    assert!(read_retained_completion(&parent, f.receipt.network_id, &f.receipt, &f.trust).is_err());
}

#[test]
fn retention_preserves_original_trust_across_explicit_release_and_rejects_identity_drift() {
    let f = fixture();
    let mut current = f.trust.clone();
    for peer in &mut current.peers {
        peer.build_fingerprint = Hash::new(b"selected release");
        peer.config_fingerprint = Hash::new(b"selected config");
    }
    validate_observation_trust(&f.trust, &current, f.receipt.network_id).unwrap();
    current.peers[0].torii_origin = "http://127.0.0.1:9999/".into();
    assert!(validate_observation_trust(&f.trust, &current, f.receipt.network_id).is_err());
    current = f.trust.clone();
    current.peers.swap(0, 1);
    assert!(validate_observation_trust(&f.trust, &current, f.receipt.network_id).is_err());
    current = f.trust.clone();
    current.genesis_signed_wire_hex.push_str("00");
    assert!(validate_observation_trust(&f.trust, &current, f.receipt.network_id).is_err());
}

#[test]
fn partial_retention_initialization_never_replaces_evidence_or_renews_deadlines() {
    let f = fixture();
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = directory.path().join("retention");
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(Journal::open(&path, false).is_err());
    let journal = open_initializing_journal(&path, false).unwrap();
    require_uninitialized_journal(&journal, &[]).unwrap();
    journal.install_json("epoch-1.json", &f.receipt).unwrap();
    assert!(require_uninitialized_journal(&journal, &[]).is_err());
    assert!(
        retain_receipts(
            &journal,
            std::slice::from_ref(&f.receipt),
            &f.trust,
            f.receipt.network_id
        )
        .is_err(),
        "receipt without original trust cannot authorize initialization"
    );
    let expired = Instant::now();
    let error = require_epoch_budget(expired, "retention fixture").unwrap_err();
    assert!(!crate::taira::observation_transport_unavailable(&error));
}

#[test]
fn retention_parser_exposes_no_seeds_provisioner_fee_or_mutating_commands() {
    use clap::Parser as _;
    #[derive(clap::Parser)]
    struct Cli {
        #[command(subcommand)]
        command: Command,
    }
    let args = [
        "epoch-maintenance",
        "maintain",
        "--trust",
        "trust.json",
        "--journal-dir",
        "journal",
        "--stop-after-epoch",
        "2",
    ];
    assert!(Cli::try_parse_from(args).is_ok());
    for flag in [
        "--schedule",
        "--custody",
        "--seed-fd",
        "--payment-asset",
        "--operation-timeout-ms",
    ] {
        let mut wrong = args.to_vec();
        wrong.extend([flag, "forbidden"]);
        assert!(Cli::try_parse_from(wrong).is_err());
    }
    for command in ["prepare", "apply", "preflight"] {
        let mut wrong = args;
        wrong[1] = command;
        assert!(Cli::try_parse_from(wrong).is_err());
    }
    let mut status = args;
    status[1] = "status";
    assert!(Cli::try_parse_from(status).is_ok());
}
