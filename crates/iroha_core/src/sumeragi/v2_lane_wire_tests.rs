//! Native cryptographic and shared-reducer projection controls.
//!
//! These fixtures deliberately exercise the private cryptographic boundary.
//! They do not manufacture a production VerifiedLaneContext or claim finality.

use super::*;
use iroha_crypto::{Algorithm, HashOf, KeyPair};
use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};

struct Fixture {
    frozen: FrozenLaneConsensusContextV1,
    context: reducer::HeightContext,
    keys: Vec<KeyPair>,
}

fn fixture() -> Fixture {
    let mut validators = (1..=4)
        .map(|seed| {
            let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap();
            let pop = iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap();
            (PeerId::new(key.public_key().clone()), key, pop)
        })
        .collect::<Vec<_>>();
    validators.sort_by(|left, right| left.0.cmp(&right.0));
    let frozen = FrozenLaneConsensusContextV1 {
        network_id: iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
            Hash::new(b"native lane crypto fixture"),
        )),
        protocol_version: wire::PROTOCOL_VERSION,
        opening_global_height: 1,
        opening_global_context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(
            Hash::new(b"crypto fixture opening"),
        )),
        admitted_binding_hash: Hash::new(b"exact first admitted group"),
        admission_priority: crate::state::QueuePlanAdmissionPriorityV1::new(1, 0).unwrap(),
        epoch: 0,
        mode: wire::ConsensusMode::Permissioned,
        lane_id: LaneId::new(2),
        dataspace_id: DataSpaceId::new(3),
        lane_incarnation: Hash::new(b"fixture route incarnation"),
        next_lane_height: 1,
        predecessor_height: 0,
        predecessor_hash: None,
        predecessor_applied_global_height: 0,
        committee: validators.iter().map(|entry| entry.0.clone()).collect(),
        validator_set_pops: validators.iter().map(|entry| entry.2.clone()).collect(),
        nexus_amx_context_hash: Hash::new(b"fixture AMX"),
        execution_policy_hash: Hash::new(b"fixture execution"),
        da_layout: wire::recommended_data_availability_layout(),
        leader_seed: [7; Hash::LENGTH],
    };
    frozen.validate().unwrap(); // Actual complete BLS-normal/PoP validation.
    let roster = (0u32..4)
        .map(|index| {
            let mut bytes = [0; 32];
            bytes[28..].copy_from_slice(&index.to_be_bytes());
            reducer::Validator::new(
                reducer::ValidatorId::new(bytes),
                reducer::VotingPower::new(1),
            )
        })
        .collect();
    let id =
        reducer::ContextId::new(Hash::new(b"unit-test-only instance, no finality token").into());
    let seed = Hash::new(
        preimage(
            b"iroha:lane-reducer:leader:v1\0",
            &(
                frozen.leader_seed,
                frozen.lane_id,
                frozen.dataspace_id,
                frozen.lane_incarnation,
                frozen.next_lane_height,
            ),
        )
        .unwrap(),
    );
    let context = reducer::HeightContext::new_from_finalized_state(
        id,
        reducer::NetworkId::new(*frozen.network_id.as_bytes()),
        1,
        reducer::FinalizedStateAnchor {
            context_id: reducer::ContextId::new(Hash::new(b"unit test external context").into()),
            height: 1,
            subject: reducer::Subject::new(Hash::new(b"unit test external subject").into()),
            predecessor_height: 0,
            predecessor_subject: None,
        },
        0,
        roster,
        reducer::VotingMode::Permissioned,
        reducer::Digest::new(frozen.nexus_amx_context_hash.into()),
        reducer::Digest::new(frozen.execution_policy_hash.into()),
        reducer::Digest::new(Hash::new(frozen.da_layout.encode()).into()),
        reducer::Digest::new(seed.into()),
    )
    .unwrap();
    Fixture {
        frozen,
        context,
        keys: validators.into_iter().map(|entry| entry.1).collect(),
    }
}

impl Fixture {
    fn authenticator(&self) -> LaneAuthenticator<'_> {
        LaneAuthenticator {
            frozen: &self.frozen,
            context: &self.context,
        }
    }
    fn round(&self, view: u64) -> LaneRoundV1 {
        LaneRoundV1 {
            instance_id: Hash::prehashed(*self.context.id().as_bytes()),
            lane_height: 1,
            voting_view: view,
        }
    }
    fn value(&self, origin_view: u64) -> LaneValueRefV1 {
        let leader = self.context.leader(origin_view);
        LaneValueRefV1 {
            instance_id: self.round(0).instance_id,
            admitted_binding_hash: self.frozen.admitted_binding_hash,
            kind: LaneValueKindV1::Execution,
            origin_view,
            origin_producer: self
                .context
                .roster()
                .iter()
                .position(|entry| entry.id() == leader)
                .unwrap() as u32,
            descriptor_hash: Hash::new(b"native immutable descriptor"),
            payload_hash: Hash::new(b"canonical lane body"),
            availability_hash: iroha_data_model::block::lane_consensus::lane_availability_hash(
                self.frozen.da_layout,
                Hash::new(b"RS16 root"),
                1,
                wire::expected_encoded_chunk_count(1, self.frozen.da_layout).unwrap(),
            )
            .unwrap(),
        }
    }
    fn statement(&self, view: u64) -> LaneVoteStatementV1 {
        LaneVoteStatementV1 {
            round: self.round(view),
            phase: LanePhaseV1::Prepare,
            value: self.value(0),
        }
    }
    fn share(&self, index: u32, bytes: &[u8]) -> LaneSignatureShareV1 {
        LaneSignatureShareV1 {
            signer: index,
            signature: Signature::new(self.keys[index as usize].private_key(), bytes)
                .payload()
                .to_vec(),
        }
    }
    fn qc(&self, statement: LaneVoteStatementV1) -> LaneQcV1 {
        let bytes = statement.signature_preimage().unwrap();
        LaneQcV1 {
            statement,
            shares: (0..3).map(|index| self.share(index, &bytes)).collect(),
        }
    }
    fn tc(&self, view: u64, high: Option<LaneQcV1>) -> LaneTcV1 {
        let round = self.round(view);
        let votes = (0..3)
            .map(|signer| {
                let body = LaneTimeoutBodyV1 {
                    round,
                    highest_prepare: if signer == 0 { high.clone() } else { None },
                };
                let share = self.share(signer, &body.signature_preimage().unwrap());
                LaneTimeoutVoteV1 { body, share }
            })
            .collect();
        LaneTcV1 { round, votes }
    }
}

#[test]
fn native_qc_requires_exact_quorum_native_signatures_and_instance() {
    let fixture = fixture();
    let auth = fixture.authenticator();
    let certificate = fixture.qc(fixture.statement(0));
    let core = auth.qc(&certificate).unwrap();
    assert_eq!(core.signatures().len(), 3);
    for count in [0, 1, 2, 4] {
        let mut changed = certificate.clone();
        changed.shares = (0..count)
            .map(|index| fixture.share(index, &changed.statement.signature_preimage().unwrap()))
            .collect();
        assert!(
            auth.qc(&changed).is_err(),
            "exact 3 of 4, including rejecting padded quorum"
        );
    }
    let mut changed = certificate.clone();
    changed.shares[1] = changed.shares[0].clone();
    assert!(auth.qc(&changed).is_err());
    let mut changed = certificate.clone();
    changed.shares.swap(0, 1);
    assert!(auth.qc(&changed).is_err());
    let mut changed = certificate.clone();
    changed.shares[0].signature[0] ^= 1;
    assert!(auth.qc(&changed).is_err());
    let mut changed = certificate.clone();
    changed.statement.round.instance_id = Hash::new(b"foreign instance");
    assert!(auth.qc(&changed).is_err());
    let mut changed = certificate.clone();
    changed.statement.value.admitted_binding_hash = Hash::new(b"later work");
    assert!(auth.qc(&changed).is_err());
    let mut changed = certificate;
    changed.statement.phase = LanePhaseV1::Commit;
    assert!(
        auth.qc(&changed).is_err(),
        "Prepare bytes cannot authenticate Commit"
    );
}

#[test]
fn native_prepayload_timeout_advances_without_an_initial_payload() {
    let fixture = fixture();
    let auth = fixture.authenticator();
    let timeout = fixture.tc(0, None);
    let tc = auth.tc(&timeout).unwrap();
    assert!(tc.highest_prepare().is_none());
    assert_eq!(tc.round(), reducer::Round::new(1, 0));
    let value = fixture.value(1);
    let manifest = LaneManifestV1 {
        value,
        layout: fixture.frozen.da_layout,
        chunk_root: Hash::new(b"RS16 root"),
        byte_len: 1,
        chunk_count: wire::expected_encoded_chunk_count(1, fixture.frozen.da_layout).unwrap(),
    };
    let proposal = LaneProposalBodyV1 {
        round: fixture.round(1),
        proposer: manifest.value.origin_producer,
        manifest,
        justification: LaneJustificationV1::Timeout(timeout),
    };
    let core = auth.proposal_body(&proposal).unwrap();
    assert_eq!(core.round().view(), 1);
    assert_ne!(proposal.manifest.value.origin_view, 0);
    let mut changed = proposal;
    changed.manifest.layout.parity_shards = 0;
    assert!(
        auth.proposal_body(&changed).is_err(),
        "TC never relaxes signed RS16"
    );
}

#[test]
fn native_timeout_preserves_authenticated_highest_prepare_and_value_origin() {
    let fixture = fixture();
    let auth = fixture.authenticator();
    let high = fixture.qc(fixture.statement(0));
    let tc = fixture.tc(1, Some(high.clone()));
    let projected = auth.tc(&tc).unwrap();
    assert_eq!(
        projected.highest_prepare().unwrap().subject(),
        high.statement.value.subject().unwrap()
    );
    let reproposal_vote = fixture.statement(2);
    assert_eq!(
        reproposal_vote.value, high.statement.value,
        "origin survives voting-view changes"
    );
    assert_eq!(
        auth.qc(&fixture.qc(reproposal_vote)).unwrap().subject(),
        projected.highest_prepare().unwrap().subject()
    );
    let mut changed = tc.clone();
    changed.votes[0].body.highest_prepare = None;
    assert!(
        auth.tc(&changed).is_err(),
        "timeout signature binds reported high Prepare"
    );
    let mut changed = tc.clone();
    changed.votes[1].share.signer = changed.votes[0].share.signer;
    assert!(auth.tc(&changed).is_err());
    let future = fixture.qc(fixture.statement(3));
    assert!(auth.tc(&fixture.tc(1, Some(future))).is_err());
    let mut commit = fixture.statement(0);
    commit.phase = LanePhaseV1::Commit;
    assert!(auth.tc(&fixture.tc(1, Some(fixture.qc(commit)))).is_err());
}

#[test]
fn native_wal_reauthenticates_evidence_after_common_framing_and_rejects_substitution() {
    let fixture = fixture();
    let auth = fixture.authenticator();
    let timeout = fixture.tc(0, None);
    let envelope = LaneWalEnvelopeV1 {
        version: FORMAT,
        persistence_id: 1,
        record: LaneWalRecordV1::InstallTimeout(timeout),
    };
    let issued = auth.wal_entry(&envelope).unwrap();
    let payload = auth.encode_wal(&envelope, &issued).unwrap();
    let hasher = |bytes: &[u8]| Hash::new(bytes).into();
    let identity = auth.wal_identity(0).unwrap();
    let mut bytes = reducer::encode_wal_file_header(identity, &hasher).to_vec();
    let frame = reducer::encode_wal_frame(0, [0; 32], &payload, &hasher).unwrap();
    bytes.extend_from_slice(frame.bytes());
    let recovered = reducer::recover_wal_file(&bytes, identity, &hasher).unwrap();
    let entry = auth.decode_wal(&recovered.records()[0]).unwrap();
    let replayed = reducer::DurableState::replay(
        &fixture.context,
        Some(fixture.context.roster()[0].id()),
        [entry],
    );
    assert!(replayed.is_ok());

    let mut changed = envelope.clone();
    changed.persistence_id = 2;
    assert!(
        auth.encode_wal(&changed, &issued).is_err(),
        "even well-formed evidence must equal the exact reducer-issued persistence intent"
    );
    let substituted = norito::encode_canonical(&changed).unwrap();
    let frame = reducer::encode_wal_frame(0, [0; 32], &substituted, &hasher).unwrap();
    let mut bytes = reducer::encode_wal_file_header(identity, &hasher).to_vec();
    bytes.extend_from_slice(frame.bytes());
    let recovered = reducer::recover_wal_file(&bytes, identity, &hasher).unwrap();
    assert!(
        auth.decode_wal(&recovered.records()[0]).is_err(),
        "valid checksum is not an intent id"
    );
    let mut changed = envelope;
    let LaneWalRecordV1::InstallTimeout(tc) = &mut changed.record else {
        unreachable!()
    };
    tc.votes[0].share.signature[0] ^= 1;
    assert!(
        auth.wal_entry(&changed).is_err(),
        "durable framing never replaces remote BLS verification"
    );
    assert!(auth.encode_wal(&changed, &issued).is_err());
}

#[test]
fn native_envelope_rejects_noncanonical_or_unknown_layout() {
    let fixture = fixture();
    let envelope = LaneMessageEnvelopeV1 {
        version: FORMAT,
        message: LaneMessageV1::TimeoutCertificate(fixture.tc(0, None)),
    };
    let bytes = norito::encode_canonical(&envelope).unwrap();
    assert_eq!(decode_message(&bytes).unwrap(), envelope.message);
    let mut trailing = bytes;
    trailing.push(0);
    assert!(decode_message(&trailing).is_err());
    let mut unknown = envelope;
    unknown.version += 1;
    assert!(decode_message(&norito::encode_canonical(&unknown).unwrap()).is_err());
}

#[test]
fn native_decision_authentication_requires_exact_commit_subject_and_frozen_layout() {
    let fixture = fixture();
    let auth = fixture.authenticator();
    let mut statement = fixture.statement(2);
    statement.phase = LanePhaseV1::Commit;
    statement.value.availability_hash =
        iroha_data_model::block::lane_consensus::lane_availability_hash(
            fixture.frozen.da_layout,
            Hash::new(b"decision body chunk root"),
            8,
            wire::expected_encoded_chunk_count(8, fixture.frozen.da_layout).unwrap(),
        )
        .unwrap();
    let decision = LaneDecisionV1 {
        manifest: LaneManifestV1 {
            value: statement.value.clone(),
            layout: fixture.frozen.da_layout,
            chunk_root: Hash::new(b"decision body chunk root"),
            byte_len: 8,
            chunk_count: wire::expected_encoded_chunk_count(8, fixture.frozen.da_layout).unwrap(),
        },
        commit_qc: fixture.qc(statement),
    };
    let verified = auth.decision_certificate(&decision).unwrap();
    assert_eq!(verified.phase(), reducer::Phase::Commit);
    assert_eq!(
        verified.subject(),
        decision.manifest.value.subject().unwrap()
    );
    let mut changed = decision.clone();
    changed.commit_qc.shares[0].signature = changed.commit_qc.shares[1].signature.clone();
    changed.validate_shape(&fixture.frozen).unwrap();
    assert!(
        auth.decision_certificate(&changed).is_err(),
        "shape cannot authenticate a signature"
    );
    let mut changed = decision.clone();
    changed.commit_qc.statement.phase = LanePhaseV1::Prepare;
    changed.commit_qc = fixture.qc(changed.commit_qc.statement);
    assert!(auth.decision_certificate(&changed).is_err());
    let mut changed = decision.clone();
    changed.manifest.value.payload_hash = Hash::new(b"other payload");
    assert!(auth.decision_certificate(&changed).is_err());
    let mut changed = decision.clone();
    changed.manifest.layout.chunk_size_bytes /= 2;
    changed.manifest.chunk_count =
        wire::expected_encoded_chunk_count(8, changed.manifest.layout).unwrap();
    assert!(auth.decision_certificate(&changed).is_err());
    let mut changed = decision;
    changed.commit_qc.statement.value.origin_producer =
        (changed.commit_qc.statement.value.origin_producer + 1) % 4;
    changed.manifest.value = changed.commit_qc.statement.value.clone();
    changed.commit_qc = fixture.qc(changed.commit_qc.statement);
    assert!(
        auth.decision_certificate(&changed).is_err(),
        "even valid signatures cannot alter original leader"
    );
}

#[cfg(all(unix, not(target_os = "espidf")))]
#[test]
fn native_storage_wal_decoder_requires_recovered_exact_frame_and_reauthenticates_after_reopen() {
    let fixture = fixture();
    let auth = fixture.authenticator();
    let envelope = LaneWalEnvelopeV1 {
        version: FORMAT,
        persistence_id: 1,
        record: LaneWalRecordV1::InstallTimeout(fixture.tc(0, None)),
    };
    let issued = auth.wal_entry(&envelope).unwrap();
    let payload = auth.encode_wal(&envelope, &issued).unwrap();
    let kura = crate::kura::Kura::blank_kura_for_testing();
    let identity = auth.wal_identity(0).unwrap();
    let mut wal = super::super::safety_wal::SafetyWal::open_with_kura_authority(
        &kura,
        kura.mint_safety_wal_directory_authority().unwrap(),
        "native-model-custody.wal",
        identity,
    )
    .unwrap();
    let receipt = wal.append(&payload).unwrap();
    let recovered = wal.recovered_records().last().unwrap();
    assert!(recovered.exactly_matches_receipt(receipt));
    assert_eq!(auth.decode_storage_wal(recovered).unwrap(), issued);
    drop(wal);
    let mut reopened = super::super::safety_wal::SafetyWal::open_with_kura_authority(
        &kura,
        kura.mint_safety_wal_directory_authority().unwrap(),
        "native-model-custody.wal",
        identity,
    )
    .unwrap();
    assert_eq!(
        auth.decode_storage_wal(&reopened.recovered_records()[0])
            .unwrap(),
        issued
    );
    let mut substituted = envelope;
    substituted.persistence_id = 3; // Physical second frame must own id 2.
    reopened
        .append(&norito::encode_canonical(&substituted).unwrap())
        .unwrap();
    assert!(
        auth.decode_storage_wal(&reopened.recovered_records()[1])
            .is_err()
    );
}

#[test]
fn native_manifest_substitution_cannot_reuse_prepare_commit_or_timeout_authority() {
    use iroha_data_model::block::lane_consensus::lane_availability_hash;
    let fixture = fixture();
    let auth = fixture.authenticator();
    let mut statement = fixture.statement(0);
    statement.phase = LanePhaseV1::Commit;
    let decision = LaneDecisionV1 {
        manifest: LaneManifestV1 {
            value: statement.value,
            layout: fixture.frozen.da_layout,
            chunk_root: Hash::new(b"RS16 root"),
            byte_len: 1,
            chunk_count: wire::expected_encoded_chunk_count(1, fixture.frozen.da_layout).unwrap(),
        },
        commit_qc: fixture.qc(statement),
    };
    auth.decision_certificate(&decision).unwrap();
    for field in 0..3 {
        let mut changed = decision.clone();
        let mut altered_authority = fixture.frozen.clone();
        match field {
            0 => changed.manifest.chunk_root = Hash::new(b"another valid nonzero root"),
            1 => changed.manifest.byte_len += 1,
            2 => {
                changed.manifest.layout.chunk_size_bytes /= 2;
                altered_authority.da_layout = changed.manifest.layout;
            }
            _ => unreachable!(),
        }
        changed.manifest.chunk_count =
            wire::expected_encoded_chunk_count(changed.manifest.byte_len, changed.manifest.layout)
                .unwrap();
        // Matching altered policy isolates signed-manifest binding from the
        // separately enforced frozen-layout comparison (test-only authenticator).
        let altered_auth = LaneAuthenticator {
            frozen: &altered_authority,
            context: &fixture.context,
        };
        assert!(altered_auth.decision_certificate(&changed).is_err());
        changed.manifest.value.availability_hash = lane_availability_hash(
            changed.manifest.layout,
            changed.manifest.chunk_root,
            changed.manifest.byte_len,
            changed.manifest.chunk_count,
        )
        .unwrap();
        changed.commit_qc.statement.value = changed.manifest.value;
        changed.validate_shape(&altered_authority).unwrap();
        assert!(
            altered_auth.decision_certificate(&changed).is_err(),
            "recomputed hash cannot reuse original Commit signatures"
        );
    }
    let mut proposal = LaneProposalBodyV1 {
        round: fixture.round(0),
        proposer: decision.manifest.value.origin_producer,
        manifest: decision.manifest,
        justification: LaneJustificationV1::Opening,
    };
    auth.proposal_body(&proposal).unwrap();
    proposal.manifest.chunk_root = Hash::new(b"mutated ProposalIntent root");
    assert!(auth.proposal_body(&proposal).is_err());
    let mut tc = fixture.tc(1, Some(fixture.qc(fixture.statement(0))));
    let high = tc.votes[0].body.highest_prepare.as_mut().unwrap();
    high.statement.value.availability_hash = Hash::new(b"different availability commitment");
    assert!(
        auth.tc(&tc).is_err(),
        "highest Prepare cannot substitute an availability manifest"
    );
}

#[path = "v2_lane_wire_projection_tests.rs"]
mod native_projection_tests;
