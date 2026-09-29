//! Consensus wire records and deterministic validation fixtures.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use norito::codec::DecodeAll as _;
fn network_id(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([seed; Hash::LENGTH]),
    ))
}
#[test]
fn consensus_modes_project_canonical_protocol_identities() {
    assert_eq!(ConsensusMode::Permissioned.tag(), PERMISSIONED_TAG);
    assert_eq!(ConsensusMode::Npos.tag(), NPOS_TAG);
    assert_eq!(
        ConsensusMode::Permissioned.bls_domain(),
        PERMISSIONED_BLS_DOMAIN
    );
    assert_eq!(ConsensusMode::Npos.bls_domain(), NPOS_BLS_DOMAIN);
    assert!(ConsensusMode::Permissioned.is_permissioned());
    assert!(!ConsensusMode::Npos.is_permissioned());
    for mode in [ConsensusMode::Permissioned, ConsensusMode::Npos] {
        let parameter_mode = crate::parameter::system::SumeragiConsensusMode::from(mode);
        assert_eq!(ConsensusMode::from(parameter_mode), mode);
    }
}
#[test]
fn global_phase_wire_tags_are_explicit_and_schema_aligned() {
    let prepare = GlobalPhase::Prepare.encode();
    let commit = GlobalPhase::Commit.encode();
    assert_eq!(prepare, u32::from(GlobalPhase::Prepare as u8).to_le_bytes());
    assert_eq!(commit, u32::from(GlobalPhase::Commit as u8).to_le_bytes());
    assert_eq!(prepare, 1_u32.to_le_bytes());
    assert_eq!(commit, 2_u32.to_le_bytes());
    let mut prepare_cursor = prepare.as_slice();
    let mut commit_cursor = commit.as_slice();
    assert_eq!(
        GlobalPhase::decode_all(&mut prepare_cursor).expect("decode Prepare"),
        GlobalPhase::Prepare
    );
    assert_eq!(
        GlobalPhase::decode_all(&mut commit_cursor).expect("decode Commit"),
        GlobalPhase::Commit
    );
    let legacy_implicit_zero_bytes = 0_u32.to_le_bytes();
    let mut legacy_implicit_zero = legacy_implicit_zero_bytes.as_slice();
    assert!(GlobalPhase::decode_all(&mut legacy_implicit_zero).is_err());
}
#[test]
fn payload_encoding_uses_natural_zero_tag_and_rejects_retired_tag_one() {
    let canonical = PayloadEncoding::ReedSolomon16.encode();
    assert_eq!(canonical, 0_u32.to_le_bytes());
    assert_eq!(
        PayloadEncoding::decode_all(&mut canonical.as_slice())
            .expect("decode canonical RS16 payload encoding"),
        PayloadEncoding::ReedSolomon16
    );
    let retired_tag = 1_u32.to_le_bytes();
    assert!(
        PayloadEncoding::decode_all(&mut retired_tag.as_slice()).is_err(),
        "retired payload-encoding tag 1 must fail closed"
    );
}

#[test]
fn payload_encoding_json_rejects_retired_plain_variant() {
    let canonical = norito::json::to_value(&PayloadEncoding::ReedSolomon16)
        .expect("serialize canonical RS16 payload encoding");
    assert_eq!(
        norito::json::from_value::<PayloadEncoding>(canonical.clone())
            .expect("decode canonical RS16 payload encoding"),
        PayloadEncoding::ReedSolomon16
    );
    let mut retired = canonical;
    let encoding = retired
        .as_object_mut()
        .expect("adjacently tagged payload encoding")
        .get_mut("encoding")
        .expect("payload encoding tag");
    assert_eq!(encoding.as_str(), Some("reed_solomon16"));
    *encoding = norito::json::Value::String("plain".to_owned());
    assert!(
        norito::json::from_value::<PayloadEncoding>(retired).is_err(),
        "retired Plain payload encoding must fail closed"
    );
}
#[test]
fn execution_commitment_enforces_topup_shape_count_and_combined_root() {
    let parent = Hash::new(b"parent");
    let ordinary = Hash::new(b"ordinary writes");
    let topup = Hash::new(b"topup tree");
    let executed_block_wire = b"executed block wire";
    let executed_block_wire_len =
        u64::try_from(executed_block_wire.len()).expect("fixture wire length fits u64");
    let executed = Hash::new(executed_block_wire);
    let post = ExecutionCommitment::kagemusha_post_state_root_v1(2, ordinary, topup);
    let canonical = ExecutionCommitment::new_without_merge_carrier(
        parent,
        post,
        ordinary,
        Some(topup),
        2,
        executed_block_wire_len,
        executed,
    )
    .expect("canonical top-up commitment");
    assert_eq!(canonical.validate(), Ok(()));
    assert_eq!(canonical.executed_block_wire_hash, executed);
    let encoded = canonical.encode();
    let mut cursor = encoded.as_slice();
    assert_eq!(
        ExecutionCommitment::decode_all(&mut cursor).expect("decode execution commitment"),
        canonical
    );
    assert_eq!(
        ExecutionCommitment::new_without_merge_carrier(
            parent,
            Hash::new(b"wrong"),
            ordinary,
            Some(topup),
            2,
            executed_block_wire_len,
            executed,
        ),
        Err(ValidationError::ExecutionCommitmentPostRootMismatch)
    );
    assert_eq!(
        ExecutionCommitment::new_without_merge_carrier(
            parent,
            post,
            ordinary,
            Some(topup),
            0,
            executed_block_wire_len,
            executed,
        ),
        Err(ValidationError::InvalidExecutionCommitment)
    );
    let wider_count = 17;
    let wider_post =
        ExecutionCommitment::kagemusha_post_state_root_v1(wider_count, ordinary, topup);
    assert!(
        ExecutionCommitment::new_without_merge_carrier(
            parent,
            wider_post,
            ordinary,
            Some(topup),
            wider_count,
            executed_block_wire_len,
            executed,
        )
        .is_ok(),
        "top-up count is bounded by physical block bytes, not an arbitrary protocol cap"
    );
}

fn peer(seed: u8) -> PeerId {
    let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("derive deterministic Sumeragi v2 fixture keypair");
    PeerId::new(key_pair.public_key().clone())
}
fn roster(powers: &[u64]) -> Vec<ValidatorPower> {
    let mut validators = (0..powers.len())
        .map(|index| peer(u8::try_from(index + 1).expect("small fixture roster")))
        .collect::<Vec<_>>();
    validators.sort();
    validators
        .into_iter()
        .zip(powers.iter().copied())
        .map(|(validator, power)| ValidatorPower { validator, power })
        .collect()
}
fn context(powers: &[u64]) -> HeightContext {
    let roster = roster(powers);
    let network_id = network_id(0xA1);
    let authority = test_kagemusha_mint_finality_authority(network_id, 0, &roster);
    let authorization =
        crate::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1::genesis(
            &authority, 100,
        )
        .expect("valid fixture genesis authorization");
    HeightContext {
        network_id,
        protocol_version: PROTOCOL_VERSION,
        height: 1,
        epoch: 0,
        kagemusha_mint_finality_authorization: authorization,
        kagemusha_mint_finality_authority: authority,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Npos,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("valid fixture quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"nexus amx context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 4,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 1024,
            max_chunk_count: 512,
        },
        leader_seed: [0xA5; 32],
    }
}
fn round(context: &HeightContext, view: View) -> ConsensusRound {
    ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view,
    }
}
fn subject(seed: u8) -> BlockSubject {
    BlockSubject {
        parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new([seed, 0]))),
        block_hash: HashOf::from_untyped_unchecked(Hash::new([seed, 1])),
        payload_hash: Hash::new([seed, 2]),
    }
}
fn execution_commitment(seed: u8) -> ExecutionCommitment {
    let executed_block_wire = [seed, 6];
    ExecutionCommitment::new_without_merge_carrier(
        Hash::new([seed, 3]),
        Hash::new([seed, 4]),
        Hash::new([seed, 5]),
        None,
        0,
        u64::try_from(executed_block_wire.len()).expect("fixture wire length fits u64"),
        Hash::new(executed_block_wire),
    )
    .expect("canonical fixture execution commitment")
}
fn qc(
    context: &HeightContext,
    view: View,
    phase: GlobalPhase,
    signers: Vec<ValidatorIndex>,
) -> QuorumCertificate {
    let round = round(context, view);
    QuorumCertificate {
        round,
        proposal_round: round,
        phase,
        subject: subject(u8::try_from(view + 1).expect("small fixture view")),
        execution_commitment: execution_commitment(
            u8::try_from(view + 1).expect("small fixture view"),
        ),
        signers,
        aggregate_signature: vec![0x5A; 48],
    }
}
#[test]
fn equal_vote_quorum_requires_two_f_plus_one_distinct_signers() {
    let context = context(&[1, 1, 1, 1]);
    assert_eq!(context.quorum.min_signers, 3);
    assert_eq!(context.validate_signers(&[0, 1, 2]), Ok(()));
    assert_eq!(context.validate_signers(&[1, 2, 3]), Ok(()));
    assert_eq!(context.validate_signers(&[0, 1, 2, 3]), Ok(()));
    assert_eq!(context.validate_certificate_signers(&[0, 1, 2]), Ok(()));
    assert_eq!(
        ValidationError::TooManySigners.to_string(),
        "signer count exceeds the wire range"
    );
    assert_eq!(
        ValidationError::SignerCountMismatch {
            expected: 3,
            actual: 4,
        }
        .to_string(),
        "certificate signer count mismatch: expected exactly 3, got 4"
    );
    assert_eq!(
        context.validate_certificate_signers(&[0, 1, 2, 3]),
        Err(ValidationError::SignerCountMismatch {
            expected: 3,
            actual: 4,
        })
    );
    assert_eq!(
        context.validate_signers(&[0, 1]),
        Err(ValidationError::InsufficientSignerCount)
    );
    assert_eq!(
        context.validate_signers(&[0, 1, 1]),
        Err(ValidationError::SignersNotStrictlySorted)
    );
    assert_eq!(
        qc(&context, 0, GlobalPhase::Commit, vec![0, 1, 2, 3]).validate(&context),
        Err(ValidationError::SignerCountMismatch {
            expected: 3,
            actual: 4,
        })
    );
}
#[test]
fn height_context_rejects_weighted_consensus_votes_in_all_modes() {
    for mode in [ConsensusMode::Permissioned, ConsensusMode::Npos] {
        let mut invalid = context(&[1, 1, 1, 1]);
        invalid.mode = mode;
        invalid.roster[0].power = 2;
        invalid.quorum =
            DualQuorum::from_roster(&invalid.roster).expect("structural weighted quorum");
        assert_eq!(invalid.validate(), Err(ValidationError::VotingPowerNotOne));
    }
}
#[test]
fn height_context_rejects_zero_execution_policy_hash() {
    let mut invalid = context(&[1, 1, 1, 1]);
    invalid.execution_policy_hash = Hash::prehashed([0; Hash::LENGTH]);
    assert_eq!(
        invalid.validate(),
        Err(ValidationError::InvalidExecutionPolicyHash)
    );
}
#[test]
fn data_availability_layout_enforces_protocol_resource_caps() {
    let maximum = DataAvailabilityLayout {
        encoding: PayloadEncoding::ReedSolomon16,
        chunk_size_bytes: MAX_DA_CHUNK_SIZE_BYTES,
        data_shards: MAX_DA_DATA_SHARDS,
        parity_shards: MAX_DA_PARITY_SHARDS,
        max_payload_size_bytes: MAX_DA_PAYLOAD_SIZE_BYTES,
        max_chunk_count: MAX_DA_CHUNK_COUNT,
    };
    assert_eq!(validate_data_availability_layout(maximum), Ok(()));
    let mut invalid_layouts = Vec::new();
    invalid_layouts.push(DataAvailabilityLayout {
        chunk_size_bytes: MAX_DA_CHUNK_SIZE_BYTES + 2,
        ..maximum
    });
    invalid_layouts.push(DataAvailabilityLayout {
        data_shards: MAX_DA_DATA_SHARDS + 1,
        ..maximum
    });
    invalid_layouts.push(DataAvailabilityLayout {
        parity_shards: MAX_DA_PARITY_SHARDS + 1,
        ..maximum
    });
    invalid_layouts.push(DataAvailabilityLayout {
        max_payload_size_bytes: MAX_DA_PAYLOAD_SIZE_BYTES + 1,
        ..maximum
    });
    invalid_layouts.push(DataAvailabilityLayout {
        max_chunk_count: MAX_DA_CHUNK_COUNT + 1,
        ..maximum
    });
    invalid_layouts.push(DataAvailabilityLayout {
        data_shards: 1,
        parity_shards: 15,
        ..maximum
    });
    invalid_layouts.push(DataAvailabilityLayout {
        data_shards: 1_024,
        parity_shards: 1_024,
        max_chunk_count: u32::MAX,
        ..maximum
    });
    for invalid in invalid_layouts {
        assert_eq!(
            validate_data_availability_layout(invalid),
            Err(ValidationError::InvalidDataAvailabilityLayout)
        );
    }
}
#[path = "consensus_v2_context_tests.rs"]
mod context_validation;
#[test]
fn snapshot_bootstrap_is_an_explicit_mutually_exclusive_parent_authority() {
    let mut anchored = context(&[1, 1, 1, 1]);
    anchored.height = 11;
    anchored.snapshot_bootstrap = Some(SnapshotBootstrapAnchor {
        snapshot_height: 10,
        snapshot_block_hash: HashOf::from_untyped_unchecked(Hash::new(b"audited snapshot tip")),
        snapshot_block_creation_time_ms: 1_000,
        snapshot_state_hash: Hash::new(b"audited snapshot WSV"),
    });
    anchored
        .validate()
        .expect("exact post-snapshot context is structurally valid");
    let record = SnapshotV2BootstrapRecord {
        version: SnapshotV2BootstrapRecord::VERSION,
        context: anchored.clone(),
        validator_set_pops: vec![vec![0xA5]; anchored.roster.len()],
    };
    record.validate().expect("complete bootstrap record");
    let mut wrong_height = record.clone();
    wrong_height.context.height = 12;
    assert_eq!(
        wrong_height.validate(),
        Err(ValidationError::InvalidParentCommit)
    );
    let mut ambiguous = anchored;
    ambiguous.parent_commit_qc = Some(qc(
        &context(&[1, 1, 1, 1]),
        0,
        GlobalPhase::Commit,
        vec![0, 1, 2],
    ));
    assert_eq!(
        ambiguous.validate(),
        Err(ValidationError::InvalidParentCommit)
    );
    let mut unsupported = record;
    unsupported.version = SnapshotV2BootstrapRecord::VERSION + 1;
    assert_eq!(
        unsupported.validate(),
        Err(ValidationError::InvalidSnapshotBootstrap)
    );
}
#[test]
fn non_boundary_height_context_id_is_pinned() {
    let context = context(&[1, 1, 1, 1]);
    context.validate().expect("valid non-boundary context");
    assert_eq!(
        *context.id().0.as_ref(),
        [
            0xc5, 0x2b, 0x81, 0xde, 0xc6, 0xb2, 0xca, 0xd3, 0x11, 0x46, 0xc6, 0x2f, 0x54, 0xf8,
            0x04, 0xbb, 0xd3, 0x53, 0x0e, 0x86, 0xbb, 0x30, 0x94, 0x86, 0x3c, 0xad, 0x56, 0x1a,
            0x7d, 0xf2, 0xd8, 0x11,
        ],
        "intentional identity-projection changes require updating this golden"
    );
}
#[test]
fn boundary_height_context_id_pins_the_complete_transition() {
    let mut context = context(&[1, 1, 1, 1]);
    use crate::isi::kagemusha_v1::{
        BeaconEpochBindingV1, InstalledBeaconEpochBindingV1,
        KagemushaMintFinalityEpochAuthorizationV1, KagemushaMintFinalityEpochDecisionV1,
    };
    context.epoch_end_height = context.height;
    context.kagemusha_mint_finality_authorization =
        KagemushaMintFinalityEpochAuthorizationV1::genesis(
            &context.kagemusha_mint_finality_authority,
            context.epoch_end_height,
        )
        .expect("genesis authorization ends at the boundary");
    let next_roster = roster(&[1, 1, 1, 1]);
    // Scheduling advances while this exact validator/key generation is retained.
    let next_authority = context.kagemusha_mint_finality_authority.clone();
    let next_authorization = KagemushaMintFinalityEpochAuthorizationV1 {
        epoch: 1,
        first_height: 2,
        last_height: 41,
        beacon: BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
            session_id: [7; 32],
            transcript_hash: [8; 32],
        }),
        previous_authorization_id: context
            .kagemusha_mint_finality_authorization
            .authorization_id()
            .unwrap(),
        decision: KagemushaMintFinalityEpochDecisionV1::Retain,
        ..context.kagemusha_mint_finality_authorization
    };
    next_authorization
        .validate_successor(&context.kagemusha_mint_finality_authorization)
        .unwrap();
    assert_eq!(next_authority, context.kagemusha_mint_finality_authority);
    assert_eq!(next_authority.generation, 0);
    assert_eq!(next_authorization.epoch, 1);
    assert_ne!(
        next_authorization.authorization_id().unwrap(),
        context
            .kagemusha_mint_finality_authorization
            .authorization_id()
            .unwrap(),
        "retention preserves key generation while advancing the certified schedule"
    );
    context.next_epoch_snapshot = Some(finality::FinalizedNextEpochSnapshot {
        committee_preparation: None,
        epoch: context.epoch + 1,
        kagemusha_mint_finality_authorization: next_authorization,
        kagemusha_mint_finality_authority: next_authority,
        epoch_end_height: 41,
        mode: context.mode,
        quorum: DualQuorum::from_roster(&next_roster).expect("valid next-epoch quorum"),
        roster: next_roster,
        validator_set_pops: vec![vec![0x81], vec![0x82, 0x83], vec![0x84], vec![0x85, 0x86]],
        leader_seed: [0x87; 32],
    });
    context.validate().expect("valid boundary context");
    assert_eq!(
        *context.id().0.as_ref(),
        [
            0xc5, 0x2c, 0x01, 0xb4, 0x53, 0xcd, 0xf0, 0x1d, 0x0c, 0x78, 0xa1, 0xb7, 0x5f, 0xb7,
            0xf8, 0x91, 0x98, 0x76, 0x68, 0x3b, 0x84, 0x44, 0x5c, 0x6f, 0xfa, 0xbd, 0xa8, 0x79,
            0x49, 0x5d, 0x14, 0x27,
        ],
        "intentional transition-identity changes require updating this golden"
    );
}
#[test]
fn height_context_id_ignores_equivalent_parent_qc_round_and_signer_evidence() {
    let mut left = context(&[1, 1, 1, 1]);
    left.height = 2;
    let parent_round = ConsensusRound {
        context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(b"parent context"))),
        height: left.height - 1,
        view: 3,
    };
    let parent_subject = subject(0x44);
    left.parent_commit_qc = Some(QuorumCertificate {
        round: parent_round,
        proposal_round: parent_round,
        phase: GlobalPhase::Commit,
        subject: parent_subject,
        execution_commitment: execution_commitment(0x44),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x11; 48],
    });
    let mut right = left.clone();
    let redecided_round = ConsensusRound {
        view: parent_round.view + 1,
        ..parent_round
    };
    right.parent_commit_qc = Some(QuorumCertificate {
        round: redecided_round,
        proposal_round: redecided_round,
        phase: GlobalPhase::Commit,
        subject: parent_subject,
        execution_commitment: execution_commitment(0x44),
        signers: vec![0, 1, 3],
        aggregate_signature: vec![0x22; 48],
    });
    assert_ne!(left.parent_commit_qc, right.parent_commit_qc);
    assert_eq!(left.id(), right.id());
    let mut different_execution = right.clone();
    different_execution
        .parent_commit_qc
        .as_mut()
        .expect("parent certificate")
        .execution_commitment = execution_commitment(0x45);
    assert_ne!(left.id(), different_execution.id());
    let mut different_subject = right.clone();
    different_subject
        .parent_commit_qc
        .as_mut()
        .expect("parent certificate")
        .subject = subject(0x45);
    assert_ne!(left.id(), different_subject.id());
    right
        .parent_commit_qc
        .as_mut()
        .expect("parent certificate")
        .round
        .context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"different parent context",
    )));
    assert_ne!(left.id(), right.id());
    let mut oversized_parent = left;
    oversized_parent
        .parent_commit_qc
        .as_mut()
        .expect("parent certificate")
        .aggregate_signature = vec![0x33; MAX_CONSENSUS_SIGNATURE_BYTES + 1];
    assert_eq!(
        oversized_parent.validate(),
        Err(ValidationError::SignatureTooLarge)
    );
}
#[test]
fn height_context_identity_ignores_reproposal_round_and_rejects_split_rounds() {
    let mut original = context(&[1, 1, 1, 1]);
    original.height = 2;
    let parent_round = ConsensusRound {
        context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"parent proposal-origin context",
        ))),
        height: 1,
        view: 5,
    };
    original.parent_commit_qc = Some(QuorumCertificate {
        round: parent_round,
        proposal_round: parent_round,
        phase: GlobalPhase::Commit,
        subject: subject(0x47),
        execution_commitment: execution_commitment(0x47),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x47; 48],
    });
    original.validate().expect("valid parent decision");
    let mut redecided = original.clone();
    let certificate = redecided
        .parent_commit_qc
        .as_mut()
        .expect("parent certificate");
    certificate.round.view += 1;
    certificate.proposal_round = certificate.round;
    redecided
        .validate()
        .expect("unchanged re-proposal may decide in another round");
    assert_eq!(original.id(), redecided.id());
    let mut cross_context_origin = original.clone();
    let certificate = cross_context_origin
        .parent_commit_qc
        .as_mut()
        .expect("parent certificate");
    certificate.round.context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"foreign proposal-origin context",
    )));
    certificate.proposal_round = certificate.round;
    cross_context_origin
        .validate()
        .expect("a structurally valid parent can belong to another prior context");
    assert_ne!(original.id(), cross_context_origin.id());
    let mut wrong_height_origin = original.clone();
    let certificate = wrong_height_origin
        .parent_commit_qc
        .as_mut()
        .expect("parent certificate");
    certificate.round.height = 2;
    certificate.proposal_round = certificate.round;
    assert_eq!(
        wrong_height_origin.validate(),
        Err(ValidationError::InvalidParentCommit)
    );
    let mut split_round = original;
    let parent = split_round
        .parent_commit_qc
        .as_mut()
        .expect("parent certificate");
    parent.proposal_round.view = parent.round.view + 1;
    assert_eq!(
        split_round.validate(),
        Err(ValidationError::InvalidParentCommit)
    );
}

#[test]
fn voting_power_sum_fails_closed_on_u64_overflow() {
    let mut roster = vec![
        ValidatorPower {
            validator: peer(1),
            power: u64::MAX,
        },
        ValidatorPower {
            validator: peer(2),
            power: 1,
        },
        ValidatorPower {
            validator: peer(3),
            power: 1,
        },
        ValidatorPower {
            validator: peer(4),
            power: 1,
        },
    ];
    roster.sort();
    assert_eq!(
        DualQuorum::from_roster(&roster),
        Err(ValidationError::VotingPowerOverflow)
    );
}
#[test]
fn leader_rotation_is_cyclic_and_wraps_roster() {
    let context = context(&[1, 1, 1, 1]);
    let start = context.leader(0);
    assert_eq!(context.leader(4), start);
    assert_eq!(
        (0..4)
            .map(|view| context.leader(view))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 1, 2, 3])
    );
}
#[test]
fn leader_rotation_reduces_the_maximum_view_without_truncation() {
    let context = context(&[1, 1, 1, 1]);
    let roster_len = u64::try_from(context.roster.len()).expect("fixture roster fits u64");
    assert_eq!(
        context.leader(u64::MAX),
        context.leader(u64::MAX % roster_len),
        "view rotation must reduce at the roster boundary before selecting an index"
    );
}
// Keep the JSON wire-contract matrix isolated from the core consensus tests.
include!("consensus_v2_json_tests.rs");
#[test]
fn leader_rotation_is_power_independent_and_wraps_roster() {
    let equal = context(&[1, 1, 1, 1]);
    let weighted = context(&[70, 10, 10, 10]);
    let start = equal.leader(0);
    assert_eq!(weighted.leader(0), start);
    assert_eq!(equal.leader(4), start);
    assert_eq!(weighted.leader(17), equal.leader(17));
    assert_eq!(
        (0..4)
            .map(|view| equal.leader(view))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 1, 2, 3])
    );
}

#[test]
fn kagemusha_consensus_signature_envelope_roundtrips_and_rejects_drift() {
    let bls = [0xA5; 96];
    let auxiliary = [0x5A; 384];
    let encoded = encode_kagemusha_consensus_signature_envelope_v1(
        KAGEMUSHA_COMMIT_VOTE_SIGNATURE_ENVELOPE_KIND_V1,
        &bls,
        &auxiliary,
    )
    .expect("bounded envelope");
    let decoded = decode_kagemusha_consensus_signature_envelope_v1(&encoded)
        .expect("canonical envelope")
        .expect("reserved envelope");
    assert_eq!(
        decoded.kind,
        KAGEMUSHA_COMMIT_VOTE_SIGNATURE_ENVELOPE_KIND_V1
    );
    assert_eq!(decoded.bls_signature, bls);
    assert_eq!(decoded.auxiliary_payload, auxiliary);

    let mut wrong_kind = encoded.clone();
    wrong_kind[16] = 99;
    assert_eq!(
        decode_kagemusha_consensus_signature_envelope_v1(&wrong_kind),
        Err(ValidationError::InvalidKagemushaSignatureEnvelope)
    );
    let mut wrong_length = encoded;
    wrong_length[19..23].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        decode_kagemusha_consensus_signature_envelope_v1(&wrong_length),
        Err(ValidationError::InvalidKagemushaSignatureEnvelope)
    );
    assert_eq!(
        decode_kagemusha_consensus_signature_envelope_v1(&bls),
        Ok(None)
    );
}
