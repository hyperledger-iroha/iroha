//! Canonical native frame admission before opening a signing instance.
//!
//! Every dynamic field is bounded by the exact committee geometry: a TC has
//! 2f+1 votes, each carrying at most one QC with 2f+1 96-byte shares. Compute the
//! canonical codec's upper envelope, including all nested framing, once per
//! supported committee. Size templates are deliberately not crypto authority.

use std::sync::OnceLock;

use iroha_crypto::Hash;
use iroha_data_model::block::{
    consensus_v2::{self as global, DataAvailabilityLayout, PayloadEncoding},
    lane_consensus::*,
};

/// Largest canonical native envelope or manifest-bearing Decision for this
/// exact 3f+1 committee. No payload bytes are transmitted by these controls.
pub(crate) fn maximum_message_bytes(committee_len: usize) -> Result<usize, String> {
    if !global::is_valid_committee_size(committee_len) {
        return Err("native frame admission requires an exact bounded 3f+1 committee".into());
    }
    static BOUNDS: OnceLock<Result<Vec<usize>, String>> = OnceLock::new();
    let bounds = BOUNDS.get_or_init(|| {
        let mut bounds = vec![0; global::MAX_FAULTS_PER_HEIGHT + 1];
        for (faults, bound) in bounds.iter_mut().enumerate().skip(1) {
            *bound = compute_bound(3 * faults + 1)?;
        }
        Ok(bounds)
    });
    bounds
        .as_ref()
        .map(|bounds| bounds[(committee_len - 1) / 3])
        .map_err(Clone::clone)
}

fn compute_bound(committee_len: usize) -> Result<usize, String> {
    let (messages, decision) = maximum_shapes(committee_len);
    messages
        .into_iter()
        .map(|message| {
            norito::encode_canonical(&LaneMessageEnvelopeV1 {
                version: LANE_MESSAGE_VERSION_V1,
                message,
            })
            .map(|bytes| bytes.len())
            .map_err(|error| error.to_string())
        })
        .chain(std::iter::once(
            norito::encode_canonical(&decision)
                .map(|bytes| bytes.len())
                .map_err(|error| error.to_string()),
        ))
        .try_fold(0, |maximum, length| {
            length.map(|length| maximum.max(length))
        })
}

fn maximum_shapes(committee_len: usize) -> (Vec<LaneMessageV1>, LaneDecisionV1) {
    let quorum = 2 * ((committee_len - 1) / 3) + 1;
    let hash = Hash::prehashed([0xFF; Hash::LENGTH]);
    let round = LaneRoundV1 {
        instance_id: hash,
        lane_height: u64::MAX,
        voting_view: u64::MAX - 1,
    };
    let value = LaneValueRefV1 {
        instance_id: hash,
        admitted_binding_hash: hash,
        kind: LaneValueKindV1::AtomicGroup,
        origin_view: u64::MAX - 1,
        origin_producer: u32::try_from(committee_len - 1).expect("bounded committee"),
        descriptor_hash: hash,
        payload_hash: hash,
        availability_hash: hash,
    };
    let shares = (committee_len - quorum..committee_len)
        .map(|signer| LaneSignatureShareV1 {
            signer: u32::try_from(signer).expect("bounded committee"),
            signature: vec![0xFF; 96],
        })
        .collect::<Vec<_>>();
    let qc = LaneQcV1 {
        statement: LaneVoteStatementV1 {
            round,
            phase: LanePhaseV1::Prepare,
            value,
        },
        shares: shares.clone(),
    };
    let votes = shares
        .iter()
        .map(|share| LaneTimeoutVoteV1 {
            body: LaneTimeoutBodyV1 {
                round,
                highest_prepare: Some(qc.clone()),
            },
            share: share.clone(),
        })
        .collect::<Vec<_>>();
    let tc = LaneTcV1 { round, votes };
    // Fixed-size layout fields use their representable maxima. These are size
    // templates, not valid DA geometry or admissible evidence. This also covers
    // variable-width integer encodings without a byte-offset/layout assumption.
    let manifest = LaneManifestV1 {
        value,
        layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: u32::MAX,
            data_shards: u16::MAX,
            parity_shards: u16::MAX,
            max_payload_size_bytes: u64::MAX,
            max_chunk_count: u32::MAX,
        },
        chunk_root: hash,
        byte_len: u64::MAX,
        chunk_count: u32::MAX,
    };
    let proposal = LaneProposalV1 {
        body: LaneProposalBodyV1 {
            round: LaneRoundV1 {
                voting_view: u64::MAX,
                ..round
            },
            proposer: value.origin_producer,
            manifest,
            justification: LaneJustificationV1::Timeout(tc.clone()),
        },
        signature: vec![0xFF; 96],
    };
    let decision = LaneDecisionV1 {
        manifest,
        commit_qc: LaneQcV1 {
            statement: LaneVoteStatementV1 {
                phase: LanePhaseV1::Commit,
                ..qc.statement
            },
            shares: shares.clone(),
        },
    };
    (
        vec![
            LaneMessageV1::Proposal(proposal),
            LaneMessageV1::Vote(LaneVoteV1 {
                statement: qc.statement,
                share: shares[0].clone(),
            }),
            LaneMessageV1::QuorumCertificate(qc),
            LaneMessageV1::TimeoutVote(tc.votes[0].clone()),
            LaneMessageV1::TimeoutCertificate(tc),
        ],
        decision,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_native_committee_fits_existing_lane_control_reservation() {
        let mut previous = 0;
        for faults in 1..=global::MAX_FAULTS_PER_HEIGHT {
            let committee = 3 * faults + 1;
            let maximum = maximum_message_bytes(committee).unwrap();
            assert!(maximum > previous);
            previous = maximum;
            // Keep the existing resource owner: no new local cap or alternate
            // protocol committee size is used to make the new path fit.
            assert!(
                maximum < crate::sumeragi::MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES,
                "native committee {committee} requires {maximum} bytes before its outer frame"
            );
            let (messages, decision) = maximum_shapes(committee);
            assert_eq!(decision.commit_qc.shares.len(), 2 * faults + 1);
            let mut measured = norito::encode_canonical(&decision).unwrap().len();
            for message in messages {
                if let LaneMessageV1::TimeoutCertificate(tc) = &message {
                    assert_eq!(tc.votes.len(), 2 * faults + 1);
                    assert!(tc.votes.iter().all(|vote| {
                        vote.body.highest_prepare.as_ref().unwrap().shares.len() == 2 * faults + 1
                    }));
                }
                measured = measured.max(
                    norito::encode_canonical(&LaneMessageEnvelopeV1 {
                        version: LANE_MESSAGE_VERSION_V1,
                        message,
                    })
                    .unwrap()
                    .len(),
                );
            }
            assert_eq!(
                measured, maximum,
                "canonical full encoding includes nested Norito framing"
            );
        }
    }

    #[test]
    fn native_frame_admission_rejects_unbounded_or_noncommittee_geometry() {
        for count in [
            0,
            1,
            2,
            3,
            5,
            6,
            global::MAX_VALIDATORS_PER_HEIGHT + 1,
            usize::MAX,
        ] {
            assert!(maximum_message_bytes(count).is_err(), "{count}");
        }
    }
}
