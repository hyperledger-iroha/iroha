//! Stateless translation of shared-reducer effects into exact native evidence.
//!
//! The process driver owns bounded witness custody. This module neither retains
//! evidence nor authorizes signing, fsync acknowledgement, or body readiness.

use super::*;
use iroha_data_model::block::lane_consensus::{LaneManifestV1, LaneProposalV1};

/// Read-only native witnesses retained by the driver for reducer-owned subjects.
/// Absence is an explicit error; projection must never invent a value or manifest.
pub(crate) trait LaneNativeWitnesses {
    /// Exact immutable value whose canonical subject equals the requested hash.
    fn value(&self, subject: reducer::Subject) -> Option<&LaneValueRefV1>;
    /// Full RS16 manifest for the requested immutable value.
    fn manifest(&self, subject: reducer::Subject) -> Option<&LaneManifestV1>;
}

impl LaneAuthenticator<'_> {
    fn native_signer(&self, signer: reducer::ValidatorId) -> Result<u32> {
        self.context
            .roster()
            .iter()
            .position(|entry| entry.id() == signer)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(|| bad("effect signer is outside the frozen roster"))
    }

    fn native_round(
        &self,
        context: reducer::ContextId,
        round: reducer::Round,
    ) -> Result<LaneRoundV1> {
        if context != self.context.id() || round.height() != self.context.height() {
            return Err(bad("effect belongs to another native instance"));
        }
        Ok(LaneRoundV1 {
            instance_id: Hash::prehashed(*context.as_bytes()),
            lane_height: round.height(),
            voting_view: round.view(),
        })
    }

    fn native_statement(
        &self,
        vote: reducer::Vote,
        witnesses: &impl LaneNativeWitnesses,
    ) -> Result<LaneVoteStatementV1> {
        let round = self.native_round(vote.context_id(), vote.round())?;
        let value = *witnesses
            .value(vote.subject())
            .ok_or_else(|| bad("missing exact native value witness"))?;
        let statement = LaneVoteStatementV1 {
            round,
            phase: match vote.phase() {
                reducer::Phase::Prepare => LanePhaseV1::Prepare,
                reducer::Phase::Commit => LanePhaseV1::Commit,
            },
            value,
        };
        if self.statement(&statement, self.native_signer(vote.signer())?)? != vote {
            return Err(bad("native value witness differs from the issued vote"));
        }
        Ok(statement)
    }

    fn native_qc(
        &self,
        qc: &reducer::QuorumCertificate,
        witnesses: &impl LaneNativeWitnesses,
    ) -> Result<LaneQcV1> {
        qc.validate(self.context).map_err(bad)?;
        let first = qc
            .signatures()
            .first()
            .ok_or_else(|| bad("empty shared quorum"))?;
        let reference = qc.reference();
        let vote = reducer::Vote::new_with_proposal_round(
            reference.context_id(),
            reference.round(),
            reference.proposal_round(),
            reference.phase(),
            reference.subject(),
            first.signer(),
        );
        let statement = self.native_statement(vote, witnesses)?;
        let shares = qc
            .signatures()
            .iter()
            .map(|share| {
                Ok(LaneSignatureShareV1 {
                    signer: self.native_signer(share.signer())?,
                    signature: share.signature().as_bytes().to_vec(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let native = LaneQcV1 { statement, shares };
        if self.qc(&native)? != *qc {
            return Err(bad("native quorum projection changed the issued evidence"));
        }
        Ok(native)
    }

    fn native_timeout_body(
        &self,
        vote: &reducer::TimeoutVote,
        witnesses: &impl LaneNativeWitnesses,
    ) -> Result<LaneTimeoutBodyV1> {
        let body = LaneTimeoutBodyV1 {
            round: self.native_round(vote.context_id(), vote.round())?,
            highest_prepare: vote
                .highest_prepare()
                .map(|qc| self.native_qc(qc, witnesses))
                .transpose()?,
        };
        if self.timeout_body(&body, self.native_signer(vote.signer())?)? != *vote {
            return Err(bad("native timeout projection changed its issued evidence"));
        }
        Ok(body)
    }

    fn native_tc(
        &self,
        tc: &reducer::TimeoutCertificate,
        witnesses: &impl LaneNativeWitnesses,
    ) -> Result<LaneTcV1> {
        tc.validate(self.context).map_err(bad)?;
        let round = self.native_round(tc.context_id(), tc.round())?;
        let mut votes = Vec::new();
        for group in tc.groups() {
            let highest_prepare = group
                .highest_prepare()
                .map(|qc| self.native_qc(qc, witnesses))
                .transpose()?;
            for share in group.signatures() {
                votes.push(LaneTimeoutVoteV1 {
                    body: LaneTimeoutBodyV1 {
                        round,
                        highest_prepare: highest_prepare.clone(),
                    },
                    share: LaneSignatureShareV1 {
                        signer: self.native_signer(share.signer())?,
                        signature: share.signature().as_bytes().to_vec(),
                    },
                });
            }
        }
        votes.sort_by_key(|vote| vote.share.signer);
        let native = LaneTcV1 { round, votes };
        if self.tc(&native)? != *tc {
            return Err(bad("native TC projection changed the issued evidence"));
        }
        Ok(native)
    }

    pub(super) fn native_local_proposal(
        &self,
        proposal: &reducer::Proposal,
        witnesses: &impl LaneNativeWitnesses,
    ) -> Result<LaneProposalBodyV1> {
        let manifest = *witnesses
            .manifest(proposal.manifest().subject())
            .ok_or_else(|| bad("missing exact native manifest witness"))?;
        let justification = match proposal.justification() {
            reducer::ProposalJustification::ParentCommit(None) => LaneJustificationV1::Opening,
            reducer::ProposalJustification::Timeout(tc) => {
                LaneJustificationV1::Timeout(self.native_tc(tc, witnesses)?)
            }
            reducer::ProposalJustification::ParentCommit(Some(_)) => {
                return Err(bad("native opening cannot carry a global parent QC"));
            }
        };
        let body = LaneProposalBodyV1 {
            round: self.native_round(proposal.context_id(), proposal.round())?,
            proposer: self.native_signer(proposal.proposer())?,
            manifest,
            justification,
        };
        if self.proposal_body(&body)? != *proposal {
            return Err(bad("native manifest differs from the issued proposal"));
        }
        Ok(body)
    }

    /// Materialize one issued WAL entry with complete native witnesses.
    /// The caller retains this exact envelope through append, ack, sign and replay.
    /// Never regenerate an already persisted or recovered intent from its abstract
    /// projection: its retained native envelope owns the original signing bytes.
    /// Missing witnesses do not release the caller's outstanding effect custody.
    pub(crate) fn native_wal(
        &self,
        entry: &reducer::WalEntry,
        witnesses: &impl LaneNativeWitnesses,
    ) -> Result<LaneWalEnvelopeV1> {
        let record = match entry.record() {
            reducer::WalRecord::ProposalIntent(proposal) => {
                LaneWalRecordV1::ProposalIntent(self.native_local_proposal(proposal, witnesses)?)
            }
            reducer::WalRecord::PrepareIntent(vote) => LaneWalRecordV1::PrepareIntent {
                statement: self.native_statement(*vote, witnesses)?,
                signer: self.native_signer(vote.signer())?,
            },
            reducer::WalRecord::ObservePrepare(qc) => {
                LaneWalRecordV1::ObservePrepare(self.native_qc(qc, witnesses)?)
            }
            reducer::WalRecord::LockAndCommit { prepare, vote } => LaneWalRecordV1::LockAndCommit {
                prepare: self.native_qc(prepare, witnesses)?,
                statement: self.native_statement(*vote, witnesses)?,
                signer: self.native_signer(vote.signer())?,
            },
            reducer::WalRecord::TimeoutIntent(vote) => LaneWalRecordV1::TimeoutIntent {
                body: self.native_timeout_body(vote, witnesses)?,
                signer: self.native_signer(vote.signer())?,
            },
            reducer::WalRecord::InstallTimeout(tc) => {
                LaneWalRecordV1::InstallTimeout(self.native_tc(tc, witnesses)?)
            }
            reducer::WalRecord::Decision(qc) => {
                LaneWalRecordV1::Decision(self.native_qc(qc, witnesses)?)
            }
        };
        let native = LaneWalEnvelopeV1 {
            version: FORMAT,
            persistence_id: entry.id().get(),
            record,
        };
        if self.wal_entry(&native)? != *entry {
            return Err(bad("native WAL differs from the issued entry"));
        }
        Ok(native)
    }

    /// Exact native signing bytes from the retained intent, never a new proposal encoding.
    /// This verifies identity only: the caller still needs the actual fsync receipt,
    /// outstanding reducer Sign effect and fresh State publication lease.
    pub(crate) fn native_signing_preimage(
        &self,
        message: &reducer::SignableMessage,
        retained: &LaneWalEnvelopeV1,
    ) -> Result<Vec<u8>> {
        let entry = self.wal_entry(retained)?;
        match (message, entry.record(), &retained.record) {
            (
                reducer::SignableMessage::Proposal(issued),
                reducer::WalRecord::ProposalIntent(stored),
                LaneWalRecordV1::ProposalIntent(body),
            ) if issued == stored => body.signature_preimage().map_err(bad),
            (
                reducer::SignableMessage::Vote(issued),
                reducer::WalRecord::PrepareIntent(stored),
                LaneWalRecordV1::PrepareIntent { statement, .. },
            ) if issued == stored => statement.signature_preimage().map_err(bad),
            (
                reducer::SignableMessage::Vote(issued),
                reducer::WalRecord::LockAndCommit { vote: stored, .. },
                LaneWalRecordV1::LockAndCommit { statement, .. },
            ) if issued == stored => statement.signature_preimage().map_err(bad),
            (
                reducer::SignableMessage::TimeoutVote(issued),
                reducer::WalRecord::TimeoutIntent(stored),
                LaneWalRecordV1::TimeoutIntent { body, .. },
            ) if issued == stored => body.signature_preimage().map_err(bad),
            _ => Err(bad(
                "native signing bytes do not match the retained durable intent",
            )),
        }
    }

    /// Translate a broadcast, verifying native signatures after projection.
    /// Proposals require the exact retained signed body: grouped TC projection can
    /// discard incidental QC signer subsets, so reconstructing it would alter the
    /// proposal's signed preimage. Votes and timeouts sign stable statements.
    pub(crate) fn native_broadcast(
        &self,
        message: &reducer::ConsensusMessageV2,
        witnesses: &impl LaneNativeWitnesses,
        exact_proposal: Option<&LaneProposalBodyV1>,
    ) -> Result<LaneMessageV1> {
        let native = match message {
            reducer::ConsensusMessageV2::Proposal(signed) => {
                let body = exact_proposal.ok_or_else(|| {
                    bad("signed proposal requires its exact retained native body")
                })?;
                if self.proposal_body(body)? != *signed.proposal() {
                    return Err(bad("retained native proposal differs from broadcast"));
                }
                LaneMessageV1::Proposal(LaneProposalV1 {
                    body: body.clone(),
                    signature: signed.signature().as_bytes().to_vec(),
                })
            }
            reducer::ConsensusMessageV2::Vote(signed) => LaneMessageV1::Vote(LaneVoteV1 {
                statement: self.native_statement(signed.vote(), witnesses)?,
                share: LaneSignatureShareV1 {
                    signer: self.native_signer(signed.vote().signer())?,
                    signature: signed.signature().as_bytes().to_vec(),
                },
            }),
            reducer::ConsensusMessageV2::QuorumCertificate(qc) => {
                LaneMessageV1::QuorumCertificate(self.native_qc(qc, witnesses)?)
            }
            reducer::ConsensusMessageV2::TimeoutVote(signed) => {
                LaneMessageV1::TimeoutVote(LaneTimeoutVoteV1 {
                    body: self.native_timeout_body(&signed.vote(), witnesses)?,
                    share: LaneSignatureShareV1 {
                        signer: self.native_signer(signed.vote().signer())?,
                        signature: signed.signature().as_bytes().to_vec(),
                    },
                })
            }
            reducer::ConsensusMessageV2::TimeoutCertificate(tc) => {
                LaneMessageV1::TimeoutCertificate(self.native_tc(tc, witnesses)?)
            }
        };
        match &native {
            LaneMessageV1::Proposal(proposal) => {
                self.verify_signature(
                    &LaneSignatureShareV1 {
                        signer: proposal.body.proposer,
                        signature: proposal.signature.clone(),
                    },
                    &proposal.body.signature_preimage().map_err(bad)?,
                )?;
            }
            LaneMessageV1::Vote(vote) => {
                self.vote(vote)?;
            }
            LaneMessageV1::TimeoutVote(vote) => {
                self.timeout_vote(vote)?;
            }
            LaneMessageV1::QuorumCertificate(_) | LaneMessageV1::TimeoutCertificate(_) => {}
        }
        Ok(native)
    }
}
