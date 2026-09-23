//! Native lane evidence codec for the existing Sumeragi reducer.
//!
//! The process-owned lane driver joins this boundary to exact body/RS16 custody,
//! authenticated P2P ingress and the descriptor-relative durable WAL owner.
//! This module authenticates bytes and projects them into the shared reducer.
//! It never selects work, authorizes a second vote, acknowledges fsync or marks
//! a body Ready. Current-instance admission and effect custody remain required.

use std::collections::BTreeMap;

use iroha_crypto::{Hash, Signature};
#[cfg(test)]
use iroha_data_model::block::consensus_v2 as wire;
use iroha_data_model::block::lane_consensus::{
    LANE_MESSAGE_VERSION_V1, LaneDecisionV1, LaneJustificationV1, LaneMessageEnvelopeV1,
    LaneMessageV1, LanePhaseV1, LaneProposalBodyV1, LaneQcV1, LaneRoundV1, LaneSignatureShareV1,
    LaneTcV1, LaneTimeoutBodyV1, LaneTimeoutVoteV1, LaneValueRefV1, LaneVoteStatementV1,
    LaneVoteV1,
};
#[cfg(test)]
use iroha_data_model::block::lane_consensus::{LaneManifestV1, LaneValueKindV1};
use norito::codec::Encode as _;
use norito::{Decode, Encode};

use super::v2_core as reducer;
use crate::state::{FrozenLaneConsensusContextV1, VerifiedLaneContext};

#[path = "v2_lane_wire_projection.rs"]
mod native_projection;
pub(crate) use native_projection::LaneNativeWitnesses;

const FORMAT: u16 = LANE_MESSAGE_VERSION_V1;

/// Failure before any reducer event or recovered entry is admitted.
#[derive(Debug, thiserror::Error)]
#[error("invalid native lane evidence: {0}")]
pub(crate) struct LaneEvidenceError(String);
type Result<T> = std::result::Result<T, LaneEvidenceError>;

fn bad(reason: impl ToString) -> LaneEvidenceError {
    LaneEvidenceError(reason.to_string())
}

#[cfg(test)]
fn preimage<T: norito::NoritoSerialize>(domain: &[u8], value: &T) -> Result<Vec<u8>> {
    let frame = norito::encode_canonical(value).map_err(bad)?;
    let mut bytes = Vec::with_capacity(domain.len() + frame.len());
    bytes.extend_from_slice(domain);
    bytes.extend(frame);
    Ok(bytes)
}

trait NativeValueProjection {
    fn subject(&self) -> Result<reducer::Subject>;
}
impl NativeValueProjection for LaneValueRefV1 {
    fn subject(&self) -> Result<reducer::Subject> {
        Ok(reducer::Subject::new(
            self.subject_hash().map_err(bad)?.into(),
        ))
    }
}
trait NativePhaseProjection {
    fn core(self) -> reducer::Phase;
}
impl NativePhaseProjection for LanePhaseV1 {
    fn core(self) -> reducer::Phase {
        match self {
            Self::Prepare => reducer::Phase::Prepare,
            Self::Commit => reducer::Phase::Commit,
        }
    }
}

/// Native unsigned durable intents cannot contain ignored signature bytes.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::sumeragi::v2_lane_wire::LaneWalRecordV1")]
pub(crate) enum LaneWalRecordV1 {
    ProposalIntent(LaneProposalBodyV1),
    PrepareIntent {
        statement: LaneVoteStatementV1,
        signer: u32,
    },
    ObservePrepare(LaneQcV1),
    LockAndCommit {
        prepare: LaneQcV1,
        statement: LaneVoteStatementV1,
        signer: u32,
    },
    TimeoutIntent {
        body: LaneTimeoutBodyV1,
        signer: u32,
    },
    InstallTimeout(LaneTcV1),
    Decision(LaneQcV1),
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::sumeragi::v2_lane_wire::LaneWalEnvelopeV1")]
pub(crate) struct LaneWalEnvelopeV1 {
    pub(crate) version: u16,
    pub(crate) persistence_id: u64,
    pub(crate) record: LaneWalRecordV1,
}

/// Borrow authenticated immutable authority; retain no scheduling or vote state.
pub(crate) struct LaneAuthenticator<'a> {
    frozen: &'a FrozenLaneConsensusContextV1,
    context: &'a reducer::HeightContext,
}

impl<'a> LaneAuthenticator<'a> {
    /// Sole production constructor. State privately constructs this token only
    /// after current-set finality and exact historical opening authentication.
    pub(crate) fn new(verified: &'a VerifiedLaneContext) -> Self {
        Self {
            frozen: verified.frozen(),
            context: verified.reducer_context(),
        }
    }

    fn round(&self, round: LaneRoundV1) -> Result<reducer::Round> {
        if round.instance_id.as_ref() != self.context.id().as_bytes()
            || round.lane_height != self.context.height()
        {
            return Err(bad("foreign instance or lane height"));
        }
        Ok(reducer::Round::new(round.lane_height, round.voting_view))
    }

    fn signer(&self, index: u32) -> Result<reducer::ValidatorId> {
        self.context
            .roster()
            .get(index as usize)
            .map(|entry| entry.id())
            .ok_or_else(|| bad("signer outside exact frozen committee"))
    }

    fn verify_signature(
        &self,
        share: &LaneSignatureShareV1,
        bytes: &[u8],
    ) -> Result<reducer::SignatureShare> {
        let signer = self.signer(share.signer)?;
        if share.signature.len() != crate::lane_consensus::LANE_BLS_PROOF_BYTES {
            return Err(bad("noncanonical BLS signature length"));
        }
        // VerifiedLaneContext has already checked the complete native BLS/PoP
        // roster. Never replace its immutable keys with a current global roster.
        Signature::try_from_bytes(&share.signature)
            .map_err(bad)?
            .verify(
                self.frozen.committee[share.signer as usize].public_key(),
                bytes,
            )
            .map_err(bad)?;
        Ok(reducer::SignatureShare::new(
            signer,
            reducer::OpaqueSignature::new(share.signature.clone()),
        ))
    }

    fn value(&self, value: &LaneValueRefV1, voting_view: u64) -> Result<reducer::Subject> {
        if value.instance_id.as_ref() != self.context.id().as_bytes()
            || value.admitted_binding_hash != self.frozen.admitted_binding_hash
            || value.origin_view > voting_view
            || self.signer(value.origin_producer)? != self.context.leader(value.origin_view)
            || [
                value.descriptor_hash,
                value.payload_hash,
                value.availability_hash,
            ]
            .contains(&Hash::prehashed([0; Hash::LENGTH]))
        {
            return Err(bad(
                "foreign pinned group or invalid immutable value origin",
            ));
        }
        value.subject()
    }

    fn statement(&self, statement: &LaneVoteStatementV1, signer: u32) -> Result<reducer::Vote> {
        Ok(reducer::Vote::new(
            self.context.id(),
            self.round(statement.round)?,
            statement.phase.core(),
            self.value(&statement.value, statement.round.voting_view)?,
            self.signer(signer)?,
        ))
    }

    fn vote(&self, vote: &LaneVoteV1) -> Result<reducer::SignedVote> {
        let body = self.statement(&vote.statement, vote.share.signer)?;
        self.verify_signature(
            &vote.share,
            &vote.statement.signature_preimage().map_err(bad)?,
        )?;
        Ok(reducer::SignedVote::new(
            body,
            reducer::OpaqueSignature::new(vote.share.signature.clone()),
        ))
    }

    fn qc(&self, certificate: &LaneQcV1) -> Result<reducer::QuorumCertificate> {
        let required = self.context.roster().len() - (self.context.roster().len() - 1) / 3;
        if certificate.shares.len() != required
            || certificate
                .shares
                .windows(2)
                .any(|pair| pair[0].signer >= pair[1].signer)
        {
            return Err(bad("QC must have exactly 2f+1 ordered distinct signers"));
        }
        let vote = self.statement(&certificate.statement, certificate.shares[0].signer)?;
        let bytes = certificate.statement.signature_preimage().map_err(bad)?;
        let shares = certificate
            .shares
            .iter()
            .map(|share| self.verify_signature(share, &bytes))
            .collect::<Result<Vec<_>>>()?;
        let reference = reducer::CertificateRef::new(
            self.context.id(),
            vote.round(),
            vote.phase(),
            vote.subject(),
        );
        let qc = reducer::QuorumCertificate::new(reference, shares);
        qc.validate(self.context).map_err(bad)?;
        Ok(qc)
    }

    fn timeout_body(&self, body: &LaneTimeoutBodyV1, signer: u32) -> Result<reducer::TimeoutVote> {
        let round = self.round(body.round)?;
        let highest = body
            .highest_prepare
            .as_ref()
            .map(|qc| self.qc(qc))
            .transpose()?;
        if highest.as_ref().is_some_and(|qc| {
            qc.phase() != reducer::Phase::Prepare || qc.round().view() > round.view()
        }) {
            return Err(bad("timeout carries non-Prepare or future evidence"));
        }
        Ok(reducer::TimeoutVote::new(
            self.context.id(),
            round,
            self.signer(signer)?,
            highest,
        ))
    }

    fn timeout_vote(&self, vote: &LaneTimeoutVoteV1) -> Result<reducer::SignedTimeoutVote> {
        let body = self.timeout_body(&vote.body, vote.share.signer)?;
        self.verify_signature(&vote.share, &vote.body.signature_preimage().map_err(bad)?)?;
        Ok(reducer::SignedTimeoutVote::new(
            body,
            reducer::OpaqueSignature::new(vote.share.signature.clone()),
        ))
    }

    fn tc(&self, certificate: &LaneTcV1) -> Result<reducer::TimeoutCertificate> {
        let round = self.round(certificate.round)?;
        let required = self.context.roster().len() - (self.context.roster().len() - 1) / 3;
        if certificate.votes.len() != required
            || certificate
                .votes
                .windows(2)
                .any(|pair| pair[0].share.signer >= pair[1].share.signer)
        {
            return Err(bad("TC must have exactly 2f+1 ordered distinct signers"));
        }
        let mut groups = BTreeMap::<
            Option<reducer::CertificateRef>,
            (
                Option<reducer::QuorumCertificate>,
                Vec<reducer::SignatureShare>,
            ),
        >::new();
        for vote in &certificate.votes {
            if vote.body.round != certificate.round {
                return Err(bad("mixed timeout rounds"));
            }
            let authenticated = self.timeout_vote(vote)?;
            let body = authenticated.vote();
            let group = groups
                .entry(body.highest_prepare_ref())
                .or_insert_with(|| (body.highest_prepare().cloned(), Vec::new()));
            group.1.push(reducer::SignatureShare::new(
                body.signer(),
                authenticated.signature().clone(),
            ));
        }
        let groups = groups
            .into_values()
            .map(|(high, shares)| reducer::TimeoutSignatureGroup::new(high, shares))
            .collect();
        let tc = reducer::TimeoutCertificate::new(self.context.id(), round, groups);
        // Shared validation rejects incompatible highest Prepare maxima as well
        // as duplicate/overlapping signers. There is no lane-specific lock rule.
        tc.validate(self.context).map_err(bad)?;
        Ok(tc)
    }

    fn proposal_body(&self, body: &LaneProposalBodyV1) -> Result<reducer::Proposal> {
        let round = self.round(body.round)?;
        let proposer = self.signer(body.proposer)?;
        if proposer != self.context.leader(round.view()) {
            return Err(bad("wrong voting-round proposer"));
        }
        let manifest = &body.manifest;
        if manifest.layout != self.frozen.da_layout {
            return Err(bad("unsigned or different RS16 layout"));
        }
        // The same subject signed by Prepare/Commit and retained in timeout
        // evidence commits every availability field, independently of proposal custody.
        manifest.validate_availability().map_err(bad)?;
        let subject = self.value(&manifest.value, round.view())?;
        let justification = match &body.justification {
            LaneJustificationV1::Opening if round.view() == 0 => {
                reducer::ProposalJustification::ParentCommit(None)
            }
            LaneJustificationV1::Timeout(tc)
                if tc.round.voting_view.checked_add(1) == Some(round.view()) =>
            {
                reducer::ProposalJustification::Timeout(self.tc(tc)?)
            }
            _ => {
                return Err(bad(
                    "proposal lacks exact opening/previous-view timeout evidence",
                ));
            }
        };
        Ok(reducer::Proposal::new(
            self.context.id(),
            round,
            proposer,
            reducer::PayloadManifest::new(
                subject,
                reducer::Digest::new(manifest.value.payload_hash.into()),
                reducer::Digest::new(manifest.chunk_root.into()),
                manifest.byte_len,
                manifest.chunk_count,
            ),
            justification,
        ))
    }

    /// Authentication is not body validation or current-instance authorization.
    /// Caller supplies a fresh reducer tag only after the current-set gate.
    pub(crate) fn event(
        &self,
        message: &LaneMessageV1,
        tag: reducer::EventTag,
    ) -> Result<reducer::Event> {
        Ok(match message {
            LaneMessageV1::Proposal(proposal) => {
                let body = self.proposal_body(&proposal.body)?;
                self.verify_signature(
                    &LaneSignatureShareV1 {
                        signer: proposal.body.proposer,
                        signature: proposal.signature.clone(),
                    },
                    &proposal.body.signature_preimage().map_err(bad)?,
                )?;
                reducer::Event::ProposalReceived {
                    tag,
                    proposal: reducer::SignedProposal::new(
                        body,
                        reducer::OpaqueSignature::new(proposal.signature.clone()),
                    ),
                }
            }
            LaneMessageV1::Vote(vote) => reducer::Event::VoteReceived {
                tag,
                vote: self.vote(vote)?,
            },
            LaneMessageV1::QuorumCertificate(qc) => reducer::Event::QuorumCertificateReceived {
                tag,
                certificate: self.qc(qc)?,
            },
            LaneMessageV1::TimeoutVote(vote) => reducer::Event::TimeoutVoteReceived {
                tag,
                vote: self.timeout_vote(vote)?,
            },
            LaneMessageV1::TimeoutCertificate(tc) => reducer::Event::TimeoutCertificateReceived {
                tag,
                certificate: self.tc(tc)?,
            },
        })
    }

    /// Authenticate native decision evidence against the privately verified
    /// context. This proves neither body readiness nor ongoing membership:
    /// callers must retain the current-set gate and exact input/body custody.
    pub(crate) fn decision_certificate(
        &self,
        decision: &LaneDecisionV1,
    ) -> Result<reducer::QuorumCertificate> {
        decision.validate_shape(self.frozen).map_err(bad)?;
        self.qc(&decision.commit_qc)
    }

    fn wal_entry(&self, envelope: &LaneWalEnvelopeV1) -> Result<reducer::WalEntry> {
        if envelope.version != FORMAT || envelope.persistence_id == 0 {
            return Err(bad("invalid WAL revision/id"));
        }
        let record = match &envelope.record {
            LaneWalRecordV1::ProposalIntent(body) => {
                reducer::WalRecord::ProposalIntent(self.proposal_body(body)?)
            }
            LaneWalRecordV1::PrepareIntent { statement, signer } => {
                reducer::WalRecord::PrepareIntent(self.statement(statement, *signer)?)
            }
            LaneWalRecordV1::ObservePrepare(qc) => reducer::WalRecord::ObservePrepare(self.qc(qc)?),
            LaneWalRecordV1::LockAndCommit {
                prepare,
                statement,
                signer,
            } => reducer::WalRecord::LockAndCommit {
                prepare: self.qc(prepare)?,
                vote: self.statement(statement, *signer)?,
            },
            LaneWalRecordV1::TimeoutIntent { body, signer } => {
                reducer::WalRecord::TimeoutIntent(self.timeout_body(body, *signer)?)
            }
            LaneWalRecordV1::InstallTimeout(tc) => reducer::WalRecord::InstallTimeout(self.tc(tc)?),
            LaneWalRecordV1::Decision(qc) => reducer::WalRecord::Decision(self.qc(qc)?),
        };
        Ok(reducer::WalEntry::new(
            reducer::PersistenceId::new(envelope.persistence_id),
            record,
        ))
    }

    /// Encode only evidence that projects exactly to the reducer-issued entry.
    /// The caller still owns descriptor-relative append/fsync and its ack token.
    pub(crate) fn encode_wal(
        &self,
        envelope: &LaneWalEnvelopeV1,
        issued: &reducer::WalEntry,
    ) -> Result<Vec<u8>> {
        if self.wal_entry(envelope)? != *issued {
            return Err(bad("native evidence differs from reducer-issued intent"));
        }
        let bytes = norito::encode_canonical(envelope).map_err(bad)?;
        if bytes.len() > reducer::SAFETY_WAL_MAX_RECORD_BYTES {
            return Err(bad("native WAL payload exceeds common bound"));
        }
        Ok(bytes)
    }

    /// Called only on a frame already accepted by common header/hash-chain
    /// recovery. Reverify every native QC/TC before shared DurableState::replay.
    pub(crate) fn decode_wal(
        &self,
        frame: &reducer::RecoveredWalRecord,
    ) -> Result<reducer::WalEntry> {
        self.decode_recovered_payload(frame.sequence(), frame.payload())
            .map(|(_, entry)| entry)
    }

    /// Decode only a frame already authenticated by the production physical
    /// SafetyWal owner. No raw sequence/payload constructor is exposed.
    pub(crate) fn decode_storage_wal(
        &self,
        frame: &super::safety_wal::RecoveredRecord,
    ) -> Result<reducer::WalEntry> {
        self.decode_storage_wal_with_envelope(frame)
            .map(|(_, entry)| entry)
    }

    /// Retain the exact native witness alongside its authenticated shared projection.
    /// This accepts only an opaque physically recovered frame, never raw framing.
    pub(crate) fn decode_storage_wal_with_envelope(
        &self,
        frame: &super::safety_wal::RecoveredRecord,
    ) -> Result<(LaneWalEnvelopeV1, reducer::WalEntry)> {
        self.decode_recovered_payload(frame.sequence(), frame.payload())
    }

    fn decode_recovered_payload(
        &self,
        sequence: u64,
        payload: &[u8],
    ) -> Result<(LaneWalEnvelopeV1, reducer::WalEntry)> {
        if payload.len() > reducer::SAFETY_WAL_MAX_RECORD_BYTES {
            return Err(bad("native WAL payload exceeds common bound"));
        }
        let envelope: LaneWalEnvelopeV1 = norito::decode_canonical(payload).map_err(bad)?;
        if sequence.checked_add(1) != Some(envelope.persistence_id) {
            return Err(bad("WAL frame/persistence identity mismatch"));
        }
        let entry = self.wal_entry(&envelope)?;
        Ok((envelope, entry))
    }

    /// Reuse shared physical WAL framing under immutable instance/lane/signer identity.
    pub(crate) fn wal_identity(&self, signer: u32) -> Result<reducer::WalFileIdentity> {
        self.signer(signer)?;
        let key = self.frozen.committee[signer as usize].public_key();
        Ok(reducer::WalFileIdentity::new(
            self.frozen.protocol_version,
            *self.frozen.network_id.as_bytes(),
            self.context.id(),
            self.context.height(),
            Hash::new(key.encode()).into(),
        ))
    }
}

/// Decode exact canonical advertised Norito framing before expensive BLS work.
pub(crate) fn decode_message(bytes: &[u8]) -> Result<LaneMessageV1> {
    if bytes.len() > reducer::SAFETY_WAL_MAX_RECORD_BYTES {
        return Err(bad("native consensus evidence exceeds bound"));
    }
    let envelope: LaneMessageEnvelopeV1 = norito::decode_canonical(bytes).map_err(bad)?;
    if envelope.version != FORMAT {
        return Err(bad("unsupported native lane envelope revision"));
    }
    Ok(envelope.message)
}

#[cfg(test)]
#[path = "v2_lane_wire_tests.rs"]
mod tests;
