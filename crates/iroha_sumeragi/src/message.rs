//! Blocks, signed consensus messages, certificates, unsigned service messages, the wire envelope
//! and evidence (spec §3.2–§3.6), with Norito encodings and decode-time size limits.

use core::fmt;

use crate::{
    crypto::Crypto,
    preimage,
    types::{
        AggregateSignature, Bitmap, EpochId, Hash32, MAX_COMMITTEE_SIZE, PublicKey, Signature,
        ValidatorIndex,
    },
};

/// Largest number of `(block, CommitQC)` entries in one [`SyncResponse`].
// SPEC: §3.5 bounds a response only by `max_bytes`; an entry-count bound keeps decoding bounded
// independently of the frame size (local `sync_batch` is validated against it) (Appendix E, E10).
pub const MAX_SYNC_ENTRIES: usize = 1024;

/// Largest signer bitmap in bytes.
pub const MAX_BITMAP_BYTES: usize = MAX_COMMITTEE_SIZE.div_ceil(8);

mod attestation;
pub use attestation::{
    AttestationSignature, CommitAttestation, MAX_ATTESTATION_SIGNATURE_BYTES,
    MAX_RESULT_WITNESS_BYTES, ResultWitness, WitnessAdmissionError,
};

/// The wire-format version of [`WireMessage`] for the P2P handshake (§3.5). Every incompatible
/// change replaces this layout directly; no alternate decoder is accepted.
pub const PROTOCOL_VERSION: u16 = 8;

/// Vote / certificate kind (§3.3).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, norito::Encode, norito::Decode,
)]
pub enum VoteKind {
    /// First phase; its certificate is the `PrepareQC` (lock).
    Prepare,
    /// Second phase; its certificate is the `CommitQC` (finality).
    Commit,
}

impl VoteKind {
    /// The preimage kind byte: `KIND_PREPARE = 0x02`, `KIND_COMMIT = 0x03`.
    pub const fn byte(self) -> u8 {
        match self {
            Self::Prepare => preimage::KIND_PREPARE,
            Self::Commit => preimage::KIND_COMMIT,
        }
    }
}

/// Block header (§3.2). Its hash is [`preimage::block_hash`].
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sumeragi::BlockHeader")]
pub struct BlockHeader {
    /// Instance id `I`.
    pub instance: Hash32,
    /// Scheduling epoch and complete authenticated context bound by every signature.
    pub epoch: EpochId,
    /// Height `h`.
    pub height: u64,
    /// View in which this block was first (freshly) proposed.
    pub origin_view: u64,
    /// `block_hash` of the committed block at `h − 1`.
    pub parent_hash: Hash32,
    /// `R` certified by the `CommitQC` of `h − 1`.
    pub parent_result: Hash32,
    /// `H(TAG_PAY ‖ payload)`.
    pub payload_hash: Hash32,
    /// `len(payload)`.
    pub payload_len: u32,
    /// Canonical index of `L(h, origin_view)`.
    pub proposer: ValidatorIndex,
    /// `[L(h, x) for x in 0..min(origin_view, a_h)]`.
    pub skipped_leaders: Vec<PublicKey>,
    /// Bounded canonical application control, signed independently of transaction payload.
    /// Preserved unchanged on EMPTY, locked reproposals, sync and restart.
    pub control_witness: crate::types::ControlWitness,
    /// Application flag (§3.7): Commit votes for this block carry attestations. Set from the
    /// independent transaction/control builders and checked by execution; proposals always carry
    /// nonempty work. The final height of an authenticated epoch requires this flag so the
    /// application certifies the boundary transition.
    pub attest: bool,
}

impl BlockHeader {
    /// `block_hash` of this header (§3.2).
    pub fn hash(&self, crypto: &dyn Crypto) -> Hash32 {
        preimage::block_hash(crypto, self)
    }
}

/// A block: header plus opaque payload (§3.2).
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sumeragi::Block")]
pub struct Block {
    /// The header.
    pub header: BlockHeader,
    /// Payload bytes (opaque to the core); `EMPTY = []` is the canonical empty block.
    pub payload: Vec<u8>,
}

impl Block {
    /// `block_hash(self.header)`.
    pub fn hash(&self, crypto: &dyn Crypto) -> Hash32 {
        self.header.hash(crypto)
    }

    /// `body_ok` (§3.2): payload length and hash match the header.
    pub fn body_ok(&self, crypto: &dyn Crypto) -> bool {
        preimage::body_ok(crypto, self)
    }
}

/// A leader's proposal for round `(height, view)` (§3.3).
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sumeragi::Proposal")]
pub struct Proposal {
    /// Instance id.
    pub instance: Hash32,
    /// Round height.
    pub height: u64,
    /// Round view (`≥ header.origin_view`).
    pub view: u64,
    /// The proposed block's header.
    pub header: BlockHeader,
    /// `Some` iff `view > 0`: a TC for `(height, view − 1)`.
    pub justify: Option<TimeoutCert>,
    /// A `CommitQC` of `height − 1`; `None` iff `height == g + 1`.
    pub parent_qc: Option<Qc>,
    /// Payload bytes; unsigned (bound through the header); relays may strip it.
    pub payload: Option<Vec<u8>>,
    /// Signature of `L(height, view)` over `prop_preimage(height, view, bh, ad)`.
    pub sig: Signature,
}

impl Proposal {
    /// `block_hash(self.header)`.
    pub fn block_hash(&self, crypto: &dyn Crypto) -> Hash32 {
        self.header.hash(crypto)
    }

    /// `ad = att_digest(justify, parent_qc)`.
    pub fn att_digest(&self, crypto: &dyn Crypto) -> Hash32 {
        preimage::att_digest(crypto, self.justify.as_ref(), self.parent_qc.as_ref())
    }

    /// The signing preimage `prop_preimage(height, view, bh, ad)` of this proposal.
    pub fn signing_preimage(&self, crypto: &dyn Crypto) -> Vec<u8> {
        preimage::prop_preimage(
            &self.instance,
            &self.header.epoch,
            self.height,
            self.view,
            &self.block_hash(crypto),
            &self.att_digest(crypto),
        )
    }

    /// The block, if the payload is present (not checked; see [`Block::body_ok`]).
    pub fn block(&self) -> Option<Block> {
        self.payload.as_ref().map(|payload| Block {
            header: self.header.clone(),
            payload: payload.clone(),
        })
    }
}

/// A Prepare or Commit vote (§3.3).
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sumeragi::Vote")]
pub struct Vote {
    /// Prepare or Commit.
    pub kind: VoteKind,
    /// Instance id.
    pub instance: Hash32,
    /// Scheduling epoch and complete authenticated context bound by every signature.
    pub epoch: EpochId,
    /// Height.
    pub height: u64,
    /// View.
    pub view: u64,
    /// Voted block hash.
    pub block_hash: Hash32,
    /// Execution result `R` bound by the vote.
    pub result: Hash32,
    /// The block's attestation flag (§3.7): the proposal header's for a Prepare, the lock's for
    /// a Commit. Signed.
    pub attest: bool,
    /// Canonical index of the signer in `C_height`.
    pub signer: ValidatorIndex,
    /// Signature over `vote_preimage(kind, height, view, block_hash, result, attest)`.
    pub sig: Signature,
    /// The signer's attestation of `att_preimage(height, block_hash, result)`: present iff
    /// `kind == Commit` and `attest` (§3.7 A3). Not covered by `sig`.
    pub attestation: Option<CommitAttestation>,
}

impl Vote {
    /// The signing preimage of this vote.
    pub fn preimage(&self) -> Vec<u8> {
        preimage::vote_preimage(
            self.kind,
            &self.instance,
            &self.epoch,
            self.height,
            self.view,
            &self.block_hash,
            &self.result,
            self.attest,
        )
    }

    /// The voted value `(block_hash, result)`.
    pub fn value(&self) -> (Hash32, Hash32) {
        (self.block_hash, self.result)
    }

    /// Whether this vote must carry an attestation: a Commit vote of a flagged block (§3.7).
    pub fn needs_attestation(&self) -> bool {
        self.kind == VoteKind::Commit && self.attest
    }

    /// The commit statement `att_preimage(height, block_hash, result)` this vote's attestation
    /// covers (§3.7).
    pub fn statement(&self) -> Vec<u8> {
        preimage::att_preimage(
            &self.instance,
            &self.epoch,
            self.height,
            &self.block_hash,
            &self.result,
        )
    }
}

/// A timeout vote (§3.3).
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sumeragi::TimeoutVote")]
pub struct TimeoutVote {
    /// Instance id.
    pub instance: Hash32,
    /// Scheduling epoch and complete authenticated context bound by every signature.
    pub epoch: EpochId,
    /// Height.
    pub height: u64,
    /// The view being timed out.
    pub view: u64,
    /// The signer's lock at signing time (a `PrepareQC` of `height` with view `≤ self.view`).
    pub high_pqc: Option<Qc>,
    /// Canonical index of the signer.
    pub signer: ValidatorIndex,
    /// Signature over `tmo_preimage(height, view, high_pqc.map(|q| q.view))`.
    pub sig: Signature,
}

impl TimeoutVote {
    /// `hq`: the view of the carried `PrepareQC`.
    pub fn hq(&self) -> Option<u64> {
        self.high_pqc.as_ref().map(|qc| qc.view)
    }

    /// The signing preimage of this timeout.
    pub fn preimage(&self) -> Vec<u8> {
        preimage::tmo_preimage(
            &self.instance,
            &self.epoch,
            self.height,
            self.view,
            self.hq(),
        )
    }
}

/// A quorum certificate: `PrepareQC` if `kind == Prepare`, `CommitQC` if `kind == Commit` (§3.4).
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sumeragi::Qc")]
pub struct Qc {
    /// Prepare or Commit.
    pub kind: VoteKind,
    /// Instance id.
    pub instance: Hash32,
    /// Scheduling epoch and complete authenticated context bound by every signature.
    pub epoch: EpochId,
    /// Height.
    pub height: u64,
    /// View.
    pub view: u64,
    /// Certified block hash.
    pub block_hash: Hash32,
    /// Certified execution result.
    pub result: Hash32,
    /// The certified block's attestation flag (§3.7), signed by every signer.
    pub attest: bool,
    /// Signers (canonical indices of `C_height`).
    pub signers: Bitmap,
    /// Aggregate of the signers' vote signatures.
    pub agg_sig: AggregateSignature,
    /// A `CommitQC` with `attest`: one attestation per signer, in ascending signer order
    /// (§3.7 A4); empty otherwise.
    pub attestations: Vec<AttestationSignature>,
    /// One canonical result preimage shared by all required attestations; absent otherwise.
    pub attestation_witness: Option<ResultWitness>,
}

impl Qc {
    /// Admit the shared result witness before retaining this decoded certificate in production.
    /// The canonical certificate is unchanged; admitted same-pool clones share the real owner.
    ///
    /// # Errors
    /// Returns a foreign-source or actual original-pool allocation refusal without replacing
    /// the original witness. The caller must preserve the source and retry local refusals.
    pub fn admit_attestation_witness(
        &mut self,
        budget: &mv::allocation::AllocationBudget,
    ) -> Result<(), WitnessAdmissionError> {
        if let Some(witness) = &mut self.attestation_witness {
            witness.admit(budget)?;
        }
        Ok(())
    }

    /// The vote preimage every signer signed.
    pub fn preimage(&self) -> Vec<u8> {
        preimage::vote_preimage(
            self.kind,
            &self.instance,
            &self.epoch,
            self.height,
            self.view,
            &self.block_hash,
            &self.result,
            self.attest,
        )
    }

    /// Whether this certificate must carry attestations: a `CommitQC` of a flagged block (§3.7).
    pub fn needs_attestations(&self) -> bool {
        self.kind == VoteKind::Commit && self.attest
    }

    /// The commit statement `att_preimage(height, block_hash, result)` its attestations cover.
    pub fn statement(&self) -> Vec<u8> {
        preimage::att_preimage(
            &self.instance,
            &self.epoch,
            self.height,
            &self.block_hash,
            &self.result,
        )
    }

    /// `qc_digest(self)` (§3.3).
    pub fn digest(&self, crypto: &dyn Crypto) -> Hash32 {
        preimage::qc_digest(crypto, self)
    }

    /// The certified value `(block_hash, result)`.
    pub fn value(&self) -> (Hash32, Hash32) {
        (self.block_hash, self.result)
    }
}

/// One signer of a [`TimeoutCert`] with the view of the `PrepareQC` it carried (`hq`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, norito::Encode, norito::Decode)]
pub struct TcEntry {
    /// Canonical index of the signer.
    pub signer: ValidatorIndex,
    /// `hq` signed by that signer.
    pub hq: Option<u64>,
}

/// A timeout certificate for `(height, view)` (§3.4).
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sumeragi::TimeoutCert")]
pub struct TimeoutCert {
    /// Instance id.
    pub instance: Hash32,
    /// Scheduling epoch and complete authenticated context bound by every signature.
    pub epoch: EpochId,
    /// Height.
    pub height: u64,
    /// The timed-out view.
    pub view: u64,
    /// Signers in strictly increasing index order with their signed `hq`.
    pub entries: Vec<TcEntry>,
    /// Aggregate of the individual timeout signatures.
    pub agg_sig: AggregateSignature,
    /// The `PrepareQC` whose view is `max(entries.hq)`; `None` iff every `hq` is `None`.
    pub high_pqc: Option<Qc>,
}

impl TimeoutCert {
    /// `max(entries.hq)` (`None` if every entry has `None`).
    pub fn max_hq(&self) -> Option<u64> {
        self.entries.iter().filter_map(|entry| entry.hq).max()
    }

    /// `tc_digest(self)` (§3.3).
    pub fn digest(&self, crypto: &dyn Crypto) -> Hash32 {
        preimage::tc_digest(crypto, self)
    }
}

/// Periodic "state, not custody" summary (§3.5, §6.11). While awaiting (§6.8) a node reports
/// `height = tip.height + 1`, view 0, `committed_qc = tip.commit_qc`, no lock, TC or proposal hash
/// and `want_proposal = false`.
#[derive(
    Clone, PartialEq, Eq, Debug, Default, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sumeragi::Status")]
pub struct Status {
    /// Instance id.
    pub instance: Hash32,
    /// Sender's current round height.
    pub height: u64,
    /// Sender's current round view.
    pub view: u64,
    /// `CommitQC` of `height − 1` (`None` at the first height).
    pub committed_qc: Option<Qc>,
    /// Sender's lock at `height`.
    pub high_pqc: Option<Qc>,
    /// Highest TC the sender holds at `height`.
    pub high_tc: Option<TimeoutCert>,
    /// Block hash of the `(height, view)` proposal the sender holds.
    pub proposal_hash: Option<Hash32>,
    /// The sender holds no proposal of `(height, view)` and asks the leader for it (§6.11).
    pub want_proposal: bool,
    /// Probe nonce: "answer me with a signed echo" (§7.4 R2). Set in every `Status` while one
    /// of the sender's keys is unanchored.
    pub probe: Option<u64>,
    /// Signed answer to a probe (§7.4 R2).
    pub echo: Option<Echo>,
}

/// A signed answer to a probe (§3.5, §7.4 R2): `sig` by `key` over
/// `echo_preimage(nonce, Status.height)`.
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sumeragi::Echo")]
pub struct Echo {
    /// Authenticated epoch authorizing the replier's reported height.
    pub epoch: EpochId,
    /// The probe nonce this `Status` answers.
    pub nonce: u64,
    /// The replier's signing key at its height, else its first configured key.
    pub key: PublicKey,
    /// Signature by `key` over `echo_preimage(nonce, height)`.
    pub sig: Signature,
}

/// Request for committed blocks starting at `from_height` (§6.9).
#[derive(Clone, Copy, PartialEq, Eq, Debug, norito::Encode, norito::Decode)]
pub struct SyncRequest {
    /// Instance id.
    pub instance: Hash32,
    /// First requested height.
    pub from_height: u64,
    /// Largest number of entries wanted.
    pub max_count: u16,
    /// Largest total size wanted (exceeded only by a single-entry response).
    pub max_bytes: u32,
}

/// One committed block with its `CommitQC`.
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode)]
pub struct SyncEntry {
    /// The committed block.
    pub block: Block,
    /// Its `CommitQC`.
    pub commit_qc: Qc,
}

/// Committed blocks at consecutive heights starting at the requested height (§3.5). An empty
/// response means "I hold nothing at `from_height`".
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode)]
pub struct SyncResponse {
    /// Instance id.
    pub instance: Hash32,
    /// Consecutive entries (possibly none).
    pub blocks: Vec<SyncEntry>,
}

/// Request for one block body (§6.9).
#[derive(Clone, Copy, PartialEq, Eq, Debug, norito::Encode, norito::Decode)]
pub struct BlockRequest {
    /// Instance id.
    pub instance: Hash32,
    /// Height of the wanted block.
    pub height: u64,
    /// Hash of the wanted block.
    pub block_hash: Hash32,
}

/// One block body, answering a [`BlockRequest`] (§6.9).
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode)]
pub struct BlockResponse {
    /// Instance id.
    pub instance: Hash32,
    /// The block.
    pub block: Block,
}

/// One bounded application partial tied to an exact applied parent and scheduling context.
/// The application authenticates its contents and the P2P sender before changing producer state.
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode)]
pub struct ApplicationControl {
    /// Complete view-independent source of this partial.
    pub context: crate::api::ApplicationControlContext,
    /// Sole canonical application partial frame; empty sideframes are rejected.
    pub bytes: crate::types::ControlWitness,
}

/// Everything that travels between nodes (§3.5).
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sumeragi::WireMessage")]
pub enum WireMessage {
    /// Round message: a proposal.
    Proposal(Box<Proposal>),
    /// Round message: a Prepare or Commit vote.
    Vote(Vote),
    /// Round message: a `PrepareQC` or `CommitQC`.
    Qc(Qc),
    /// Round message: a timeout vote.
    Timeout(Box<TimeoutVote>),
    /// Round message: a timeout certificate.
    Tc(Box<TimeoutCert>),
    /// Service message: status summary.
    Status(Box<Status>),
    /// Service message: sync request.
    SyncRequest(SyncRequest),
    /// Service message: sync response.
    SyncResponse(SyncResponse),
    /// Service message: body request.
    BlockRequest(BlockRequest),
    /// Service message: body response.
    BlockResponse(BlockResponse),
    /// Bounded source-bound application partial, independently verified by the application.
    ApplicationControl(ApplicationControl),
}

impl WireMessage {
    /// Whether every present result witness already belongs to this exact original pool.
    ///
    /// This performs no allocation and does not validate signatures or require a witness
    /// where the protocol demands one; those remain independent cryptographic checks.
    #[must_use]
    pub fn attestation_witnesses_admitted_to(
        &self,
        budget: &mv::allocation::AllocationBudget,
    ) -> bool {
        let qc = |value: &Qc| {
            value
                .attestation_witness
                .as_ref()
                .is_none_or(|w| w.admitted_to(budget))
        };
        let optional = |value: &Option<Qc>| value.as_ref().is_none_or(qc);
        let tc = |value: &TimeoutCert| optional(&value.high_pqc);
        match self {
            Self::Vote(value) => value
                .attestation
                .as_ref()
                .is_none_or(|a| a.witness.admitted_to(budget)),
            Self::Qc(value) => qc(value),
            Self::Proposal(value) => {
                optional(&value.parent_qc) && value.justify.as_ref().is_none_or(tc)
            }
            Self::Timeout(value) => optional(&value.high_pqc),
            Self::Tc(value) => tc(value),
            Self::Status(value) => {
                optional(&value.committed_qc)
                    && optional(&value.high_pqc)
                    && value.high_tc.as_ref().is_none_or(tc)
            }
            Self::SyncResponse(value) => value.blocks.iter().all(|entry| qc(&entry.commit_qc)),
            Self::SyncRequest(_)
            | Self::BlockRequest(_)
            | Self::BlockResponse(_)
            | Self::ApplicationControl(_) => true,
        }
    }

    /// Admit every result witness before a production message is retained by consensus.
    /// This changes ownership only, never canonical bytes. Completed admissions remain attached
    /// on refusal, so retrying this same message shares those exact owners.
    ///
    /// # Errors
    /// Rejects foreign admitted storage or an original-pool allocation refusal. The caller must
    /// not retain this message in the production ingress until every witness is admitted.
    pub fn admit_attestation_witnesses(
        &mut self,
        budget: &mv::allocation::AllocationBudget,
    ) -> Result<(), WitnessAdmissionError> {
        fn optional_qc(
            value: &mut Option<Qc>,
            budget: &mv::allocation::AllocationBudget,
        ) -> Result<(), WitnessAdmissionError> {
            if let Some(value) = value {
                value.admit_attestation_witness(budget)?;
            }
            Ok(())
        }
        fn tc(
            value: &mut TimeoutCert,
            budget: &mv::allocation::AllocationBudget,
        ) -> Result<(), WitnessAdmissionError> {
            optional_qc(&mut value.high_pqc, budget)
        }
        match self {
            Self::Vote(vote) => {
                if let Some(share) = &mut vote.attestation {
                    share.witness.admit(budget)?;
                }
            }
            Self::Qc(value) => value.admit_attestation_witness(budget)?,
            Self::Proposal(value) => {
                optional_qc(&mut value.parent_qc, budget)?;
                if let Some(value) = &mut value.justify {
                    tc(value, budget)?;
                }
            }
            Self::Timeout(value) => optional_qc(&mut value.high_pqc, budget)?,
            Self::Tc(value) => tc(value, budget)?,
            Self::Status(value) => {
                optional_qc(&mut value.committed_qc, budget)?;
                optional_qc(&mut value.high_pqc, budget)?;
                if let Some(value) = &mut value.high_tc {
                    tc(value, budget)?;
                }
            }
            Self::SyncResponse(value) => {
                for entry in &mut value.blocks {
                    entry.commit_qc.admit_attestation_witness(budget)?;
                }
            }
            Self::SyncRequest(_)
            | Self::BlockRequest(_)
            | Self::BlockResponse(_)
            | Self::ApplicationControl(_) => {}
        }
        Ok(())
    }

    /// The instance id the message claims.
    pub fn instance(&self) -> &Hash32 {
        match self {
            Self::Proposal(p) => &p.instance,
            Self::Vote(x) => &x.instance,
            Self::Qc(c) => &c.instance,
            Self::Timeout(t) => &t.instance,
            Self::Tc(t) => &t.instance,
            Self::Status(s) => &s.instance,
            Self::SyncRequest(r) => &r.instance,
            Self::SyncResponse(r) => &r.instance,
            Self::BlockRequest(r) => &r.instance,
            Self::BlockResponse(r) => &r.instance,
            Self::ApplicationControl(m) => &m.context.instance,
        }
    }

    /// The height of a round message (`None` for service messages, which are never
    /// height-filtered, §6.1).
    pub fn round_height(&self) -> Option<u64> {
        match self {
            Self::Proposal(p) => Some(p.height),
            Self::Vote(x) => Some(x.height),
            Self::Qc(c) => Some(c.height),
            Self::Timeout(t) => Some(t.height),
            Self::Tc(t) => Some(t.height),
            Self::Status(_)
            | Self::SyncRequest(_)
            | Self::SyncResponse(_)
            | Self::BlockRequest(_)
            | Self::BlockResponse(_)
            | Self::ApplicationControl(_) => None,
        }
    }

    /// The Norito enum tag of this message: its position in §3.5, 0 to 10.
    pub fn wire_tag(&self) -> u32 {
        match self {
            Self::Proposal(_) => 0,
            Self::Vote(_) => 1,
            Self::Qc(_) => 2,
            Self::Timeout(_) => 3,
            Self::Tc(_) => 4,
            Self::Status(_) => 5,
            Self::SyncRequest(_) => 6,
            Self::SyncResponse(_) => 7,
            Self::BlockRequest(_) => 8,
            Self::BlockResponse(_) => 9,
            Self::ApplicationControl(_) => 10,
        }
    }

    /// The §12.3 O8 traffic class (the table of §3.5).
    pub fn traffic_class(&self) -> TrafficClass {
        let payload = matches!(self, Self::Proposal(p) if p.payload.is_some());
        class_of_tag(self.wire_tag(), payload).unwrap_or(TrafficClass::Control)
    }

    /// Canonical Norito encoding (one exact V1 frame).
    ///
    /// # Errors
    /// Propagates a Norito serialization failure.
    pub fn encode(&self) -> Result<Vec<u8>, CodecError> {
        norito::encode_canonical(self).map_err(|e| CodecError::Norito(e.to_string()))
    }

    /// Decode one canonical frame of at most `max_frame` bytes and check the size limits.
    ///
    /// Never panics; non-canonical encodings are rejected, so the unsigned framing is not
    /// malleable.
    ///
    /// # Errors
    /// Oversized frame, malformed or non-canonical encoding, or a violated limit.
    pub fn decode(bytes: &[u8], max_frame: usize) -> Result<Self, CodecError> {
        if bytes.len() > max_frame {
            return Err(CodecError::TooLarge {
                len: bytes.len(),
                max: max_frame,
            });
        }
        let message: Self =
            norito::decode_canonical(bytes).map_err(|e| CodecError::Norito(e.to_string()))?;
        message.check_limits()?;
        Ok(message)
    }

    /// Structural size limits that do not depend on a committee (keys, bitmaps, TC entries,
    /// skipped leaders, sync entries).
    ///
    /// # Errors
    /// The first violated limit.
    pub fn check_limits(&self) -> Result<(), CodecError> {
        match self {
            Self::Proposal(p) => check_proposal(p),
            Self::Vote(x) => {
                check_attestation(x.attestation.as_ref().map(|a| a.signature.as_slice()))
            }
            Self::SyncRequest(_) | Self::BlockRequest(_) => Ok(()),
            Self::Qc(c) => check_qc(c),
            Self::Timeout(t) => check_opt_qc(t.high_pqc.as_ref()),
            Self::Tc(t) => check_tc(t),
            Self::Status(s) => {
                check_opt_qc(s.committed_qc.as_ref())?;
                check_opt_qc(s.high_pqc.as_ref())?;
                if s.echo.as_ref().is_some_and(|e| !e.key.is_well_formed()) {
                    return Err(CodecError::Limit("public key"));
                }
                s.high_tc.as_ref().map_or(Ok(()), check_tc)
            }
            Self::SyncResponse(r) => {
                if r.blocks.len() > MAX_SYNC_ENTRIES {
                    return Err(CodecError::Limit("sync entries"));
                }
                r.blocks.iter().try_for_each(|entry| {
                    check_header(&entry.block.header)?;
                    check_qc(&entry.commit_qc)
                })
            }
            Self::BlockResponse(r) => check_header(&r.block.header),
            Self::ApplicationControl(m) => {
                if m.bytes.is_empty() {
                    Err(CodecError::Limit("empty application control"))
                } else {
                    Ok(())
                }
            }
        }
    }
}

pub(crate) fn check_qc(qc: &Qc) -> Result<(), CodecError> {
    if qc.signers.as_bytes().len() > MAX_BITMAP_BYTES {
        return Err(CodecError::Limit("bitmap"));
    }
    if qc.attestations.len() > MAX_COMMITTEE_SIZE {
        return Err(CodecError::Limit("attestations"));
    }
    qc.attestations
        .iter()
        .try_for_each(|a| check_attestation(Some(a.as_slice())))
}

fn check_attestation(attestation: Option<&[u8]>) -> Result<(), CodecError> {
    if attestation.is_some_and(|a| a.len() > MAX_ATTESTATION_SIGNATURE_BYTES) {
        return Err(CodecError::Limit("attestation"));
    }
    Ok(())
}

pub(crate) fn check_opt_qc(qc: Option<&Qc>) -> Result<(), CodecError> {
    qc.map_or(Ok(()), check_qc)
}

pub(crate) fn check_tc(tc: &TimeoutCert) -> Result<(), CodecError> {
    if tc.entries.len() > MAX_COMMITTEE_SIZE {
        return Err(CodecError::Limit("tc entries"));
    }
    check_opt_qc(tc.high_pqc.as_ref())
}

pub(crate) fn check_header(header: &BlockHeader) -> Result<(), CodecError> {
    if header.skipped_leaders.len() > MAX_COMMITTEE_SIZE {
        return Err(CodecError::Limit("skipped leaders"));
    }
    if !header.skipped_leaders.iter().all(PublicKey::is_well_formed) {
        return Err(CodecError::Limit("public key"));
    }
    Ok(())
}

pub(crate) fn check_proposal(p: &Proposal) -> Result<(), CodecError> {
    check_header(&p.header)?;
    check_opt_qc(p.parent_qc.as_ref())?;
    p.justify.as_ref().map_or(Ok(()), check_tc)
}

/// Traffic class of a wire message (§12.3 O8, the table of §3.5): the transport and the
/// driver's ingress serve control before proposal before bulk traffic, with a minimum share for
/// bulk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrafficClass {
    /// Votes, certificates, timeouts, `Status`, requests, proposals without payload.
    Control,
    /// Proposals with payload and every `BlockResponse`.
    Proposal,
    /// `SyncResponse`.
    Bulk,
}

/// The class of wire tag `tag` (`payload`: a proposal carries its payload); `None` for an
/// unknown tag.
// SPEC: §12.3 O8 put a `BlockResponse` for the current height in the proposal class and other
// ones in bulk; the transport classifies a frame before it is decoded, so every `BlockResponse`
// is proposal class (Appendix E, E44).
fn class_of_tag(tag: u32, payload: bool) -> Option<TrafficClass> {
    match tag {
        0 if payload => Some(TrafficClass::Proposal),
        0..=6 | 8 | 10 => Some(TrafficClass::Control),
        7 => Some(TrafficClass::Bulk),
        9 => Some(TrafficClass::Proposal),
        _ => None,
    }
}

/// The Norito frame header: magic, version, schema hash, compression, length, checksum, flags.
const FRAME_HEADER: usize = 40;
/// The header flag of compact per-value lengths, the only flag of a canonical frame.
const COMPACT_LEN: u8 = 0x02;

/// The schema hash of a `WireMessage` frame (header bytes 6..22).
fn wire_schema_hash() -> [u8; 16] {
    static HASH: std::sync::OnceLock<[u8; 16]> = std::sync::OnceLock::new();
    *HASH.get_or_init(norito::schema::identity::frame_hash::<WireMessage>)
}

/// The traffic class of an encoded [`WireMessage`] frame without decoding it (§3.5): the
/// canonical frame header (magic, version, schema hash, no compression, compact lengths, exact
/// length), the enum tag and, for a proposal, the `Option` tag of its payload. `None` for bytes
/// that are not such a frame. Never panics; for every frame that
/// [`WireMessage::decode`] accepts it equals [`WireMessage::traffic_class`].
pub fn traffic_class_of_frame(frame: &[u8]) -> Option<TrafficClass> {
    let header = frame.get(..FRAME_HEADER)?;
    let length = u64::from_le_bytes(header.get(23..31)?.try_into().ok()?);
    if header.get(..6)? != b"NRT0\0\0"
        || header.get(6..22)? != wire_schema_hash()
        || header.get(22) != Some(&0)
        || header.get(39) != Some(&COMPACT_LEN)
        || usize::try_from(length).ok()? != frame.len() - FRAME_HEADER
    {
        return None;
    }
    let body = &frame[FRAME_HEADER..];
    let tag = u32::from_le_bytes(body.get(..4)?.try_into().ok()?);
    if tag != 0 {
        return class_of_tag(tag, false);
    }
    // `Proposal(Box<Proposal>)`: the variant's length, the box's length, then the fields
    // `instance, height, view, header, justify, parent_qc` and the `Option` of `payload`, each
    // prefixed with its compact length.
    let mut rest = body.get(4..)?;
    for _ in 0..2 {
        rest = take_len(rest)?.1;
    }
    for _ in 0..6 {
        let (field, tail) = take_len(rest)?;
        rest = tail.get(field..)?;
    }
    let (len, tail) = take_len(rest)?;
    match (len, tail.first()) {
        (1.., Some(0)) => class_of_tag(0, false),
        (1.., Some(1)) => class_of_tag(0, true),
        _ => None,
    }
}

/// Split a canonical compact length (LEB128, at most ten bytes, shortest form) off `bytes`.
fn take_len(bytes: &[u8]) -> Option<(usize, &[u8])> {
    let mut value: u64 = 0;
    for (i, byte) in bytes.iter().take(10).enumerate() {
        let bits = u64::from(byte & 0x7f);
        if i == 9 && bits > 1 {
            return None; // beyond 64 bits
        }
        value |= bits.checked_shl(u32::try_from(7 * i).ok()?)?;
        if byte & 0x80 == 0 {
            if i > 0 && bits == 0 {
                return None; // not the shortest form
            }
            return Some((usize::try_from(value).ok()?, bytes.get(i + 1..)?));
        }
    }
    None
}

/// Encoding or decoding failure.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CodecError {
    /// The frame exceeds the transport limit.
    TooLarge {
        /// Frame length.
        len: usize,
        /// Limit.
        max: usize,
    },
    /// Norito rejected the bytes (malformed, non-canonical, checksum, resource limits).
    Norito(String),
    /// A structural size limit was violated.
    Limit(&'static str),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { len, max } => write!(f, "frame of {len} bytes exceeds {max}"),
            Self::Norito(e) => write!(f, "norito: {e}"),
            Self::Limit(what) => write!(f, "limit exceeded: {what}"),
        }
    }
}

impl std::error::Error for CodecError {}

/// What was wrong with a signed proposal (§6.2 steps 3, 5, 6). Only signed-content defects
/// produce evidence; an `Invalid` execution never does (§3.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug, norito::Encode, norito::Decode)]
pub enum Defect {
    /// `view == 0` but `justify` is present.
    UnexpectedJustify,
    /// `view > 0` but `justify` is absent.
    MissingJustify,
    /// `justify` is not a valid TC for `(height, view − 1)`.
    InvalidJustify,
    /// `parent_qc` absent although `height > g + 1`.
    MissingParentQc,
    /// `parent_qc` present although `height == g + 1`.
    UnexpectedParentQc,
    /// `parent_qc` is not a valid `CommitQC` of `height − 1` for the committed tip.
    InvalidParentQc,
    /// `header.instance ≠ I`.
    HeaderInstance,
    /// `header.height ≠ height`.
    HeaderHeight,
    /// Header names a different scheduling epoch or complete context.
    EpochContext,
    /// An epoch boundary omits mandatory current-authority attestation.
    BoundaryAttestation,
    /// `header.parent_hash ≠ tip.block_hash`.
    ParentHash,
    /// `header.parent_result ≠ tip.result`.
    ParentResult,
    /// `payload_len > max_block_bytes(h)`.
    PayloadTooLarge,
    /// TC rule: the block is not `justify.high_pqc`'s block.
    TcRule,
    /// Fresh block with `origin_view ≠ view`.
    OriginView,
    /// Fresh block whose `proposer ≠ idx(L(h, view))`.
    Proposer,
    /// Fresh block with wrong `skipped_leaders`.
    SkippedLeaders,
    /// A block without work, forbidden in every view including re-proposals.
    EmptyPayload,
}

/// Evidence of signed misbehaviour (§3.6). Self-verifying from its content.
#[derive(Clone, PartialEq, Eq, Debug, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sumeragi::Evidence")]
pub enum Evidence {
    /// Same `(I, h, v)`, both signed by `L(h, v)`, different `(bh, ad)`.
    ProposalEquivocation(Box<Proposal>, Box<Proposal>),
    /// Same `(kind, I, h, v, signer)`, different `(bh, R)`.
    VoteEquivocation(Vote, Vote),
    /// Same `(I, h, v, signer)`, different `hq`.
    TimeoutEquivocation(Box<TimeoutVote>, Box<TimeoutVote>),
    /// A signed-content defect (§6.2 step 7).
    InvalidProposal {
        /// The proposal (payload stripped).
        proposal: Box<Proposal>,
        /// What was wrong.
        defect: Defect,
    },
    /// `CommitQCs` of the same committed height with different values (§7.6).
    ConflictingCertificates(Qc, Qc),
}

/// Maximum canonical native evidence frame. Admission also applies a per-block aggregate
/// bound; this limit is enforced before decoding any nested proof or attachment.
pub const MAX_EVIDENCE_FRAME_BYTES: usize = 4 * 1024 * 1024;

impl Evidence {
    /// Encode this native signed report as one exact canonical Norito V1 frame.
    /// This checks bounded structural shape, not authority or signatures.
    ///
    /// # Errors
    /// Rejects oversized graphs, proposal payloads, or canonical serialization failure.
    pub fn encode(&self) -> Result<Vec<u8>, CodecError> {
        self.check_limits()?;
        let len = norito::canonical_frame_len(self)
            .map_err(|error| CodecError::Norito(error.to_string()))?;
        if len > MAX_EVIDENCE_FRAME_BYTES {
            return Err(CodecError::TooLarge {
                len,
                max: MAX_EVIDENCE_FRAME_BYTES,
            });
        }
        norito::encode_canonical(self).map_err(|error| CodecError::Norito(error.to_string()))
    }

    /// Decode exactly one bounded canonical native evidence frame. The frame's declared
    /// schema and fixed V1 flags are mandatory. Decoding grants no signing or stake authority.
    ///
    /// # Errors
    /// Rejects oversized, malformed, noncanonical, truncated or suffixed frames and invalid
    /// bounded proof structure before any application consults a claimed signer.
    pub fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() > MAX_EVIDENCE_FRAME_BYTES {
            return Err(CodecError::TooLarge {
                len: bytes.len(),
                max: MAX_EVIDENCE_FRAME_BYTES,
            });
        }
        let value: Self = norito::decode_canonical(bytes)
            .map_err(|error| CodecError::Norito(error.to_string()))?;
        value.check_limits()?;
        Ok(value)
    }

    /// Check native artifact bounds without accepting their claimed authority context.
    /// Evidence never transports a proposal body: only the original signed header and
    /// justification can establish the native signed-content offences.
    ///
    /// # Errors
    /// A nested native proof exceeds its protocol bound or includes an unsigned payload.
    pub fn check_limits(&self) -> Result<(), CodecError> {
        let proposal = |value: &Proposal| {
            if value.payload.is_some() {
                return Err(CodecError::Limit("evidence proposal payload"));
            }
            check_proposal(value)
        };
        match self {
            Self::ProposalEquivocation(first, second) => {
                proposal(first)?;
                proposal(second)
            }
            Self::VoteEquivocation(first, second) => {
                check_attestation(
                    first
                        .attestation
                        .as_ref()
                        .map(|value| value.signature.as_slice()),
                )?;
                check_attestation(
                    second
                        .attestation
                        .as_ref()
                        .map(|value| value.signature.as_slice()),
                )
            }
            Self::TimeoutEquivocation(first, second) => {
                check_opt_qc(first.high_pqc.as_ref())?;
                check_opt_qc(second.high_pqc.as_ref())
            }
            Self::InvalidProposal {
                proposal: value, ..
            } => proposal(value),
            Self::ConflictingCertificates(first, second) => {
                check_qc(first)?;
                check_qc(second)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{testing::FakeCrypto, types::SIGNATURE_LEN};

    fn h(byte: u8) -> Hash32 {
        Hash32([byte; 32])
    }

    fn key(byte: u8) -> PublicKey {
        PublicKey::new(vec![byte; 32]).unwrap()
    }

    fn sample_header() -> BlockHeader {
        BlockHeader {
            control_witness: crate::types::ControlWitness::empty(),
            epoch: crate::testing::TEST_EPOCH.id,
            instance: h(1),
            height: 9,
            origin_view: 2,
            parent_hash: h(2),
            parent_result: h(3),
            payload_hash: h(4),
            payload_len: 3,
            proposer: 1,
            skipped_leaders: vec![key(5), key(6)],
            attest: false,
        }
    }

    fn sample_qc(kind: VoteKind, view: u64) -> Qc {
        Qc {
            attestation_witness: None,
            epoch: crate::testing::TEST_EPOCH.id,
            kind,
            instance: h(1),
            height: 9,
            view,
            block_hash: h(7),
            result: h(8),
            signers: Bitmap::from_indices(4, [0, 1, 3]).unwrap(),
            agg_sig: AggregateSignature([9; SIGNATURE_LEN]),
            attest: false,
            attestations: Vec::new(),
        }
    }

    fn sample_tc() -> TimeoutCert {
        TimeoutCert {
            epoch: crate::testing::TEST_EPOCH.id,
            instance: h(1),
            height: 9,
            view: 3,
            entries: vec![
                TcEntry {
                    signer: 0,
                    hq: None,
                },
                TcEntry {
                    signer: 2,
                    hq: Some(1),
                },
                TcEntry {
                    signer: 3,
                    hq: Some(2),
                },
            ],
            agg_sig: AggregateSignature([10; SIGNATURE_LEN]),
            high_pqc: Some(sample_qc(VoteKind::Prepare, 2)),
        }
    }

    fn sample_vote(kind: VoteKind) -> Vote {
        Vote {
            epoch: crate::testing::TEST_EPOCH.id,
            kind,
            instance: h(1),
            height: 9,
            view: 3,
            block_hash: h(7),
            result: h(8),
            attest: false,
            signer: 2,
            sig: Signature([11; SIGNATURE_LEN]),
            attestation: None,
        }
    }

    fn sample_timeout() -> TimeoutVote {
        TimeoutVote {
            epoch: crate::testing::TEST_EPOCH.id,
            instance: h(1),
            height: 9,
            view: 3,
            high_pqc: Some(sample_qc(VoteKind::Prepare, 2)),
            signer: 1,
            sig: Signature([12; SIGNATURE_LEN]),
        }
    }

    fn sample_proposal() -> Proposal {
        Proposal {
            instance: h(1),
            height: 9,
            view: 4,
            header: sample_header(),
            justify: Some(sample_tc()),
            parent_qc: Some(sample_qc(VoteKind::Commit, 0)),
            payload: Some(vec![1, 2, 3]),
            sig: Signature([13; SIGNATURE_LEN]),
        }
    }

    fn sample_status() -> Status {
        Status {
            instance: h(1),
            height: 9,
            view: 4,
            committed_qc: Some(sample_qc(VoteKind::Commit, 0)),
            high_pqc: Some(sample_qc(VoteKind::Prepare, 2)),
            high_tc: Some(sample_tc()),
            proposal_hash: Some(h(14)),
            want_proposal: true,
            probe: Some(0xdead_beef),
            echo: Some(Echo {
                epoch: crate::testing::TEST_EPOCH.id,
                nonce: 0x0102_0304,
                key: key(3),
                sig: Signature([15; SIGNATURE_LEN]),
            }),
        }
    }

    fn all_wire_messages() -> Vec<WireMessage> {
        let block = Block {
            header: sample_header(),
            payload: vec![1, 2, 3],
        };
        vec![
            WireMessage::Proposal(Box::new(sample_proposal())),
            WireMessage::Proposal(Box::new(Proposal {
                justify: None,
                parent_qc: None,
                payload: None,
                ..sample_proposal()
            })),
            WireMessage::Vote(sample_vote(VoteKind::Prepare)),
            WireMessage::Vote(sample_vote(VoteKind::Commit)),
            WireMessage::Qc(sample_qc(VoteKind::Prepare, 2)),
            WireMessage::Qc(sample_qc(VoteKind::Commit, 5)),
            WireMessage::Timeout(Box::new(sample_timeout())),
            WireMessage::Timeout(Box::new(TimeoutVote {
                high_pqc: None,
                ..sample_timeout()
            })),
            WireMessage::Tc(Box::new(sample_tc())),
            WireMessage::Tc(Box::new(TimeoutCert {
                high_pqc: None,
                entries: vec![],
                ..sample_tc()
            })),
            WireMessage::Status(Box::new(sample_status())),
            WireMessage::Status(Box::new(Status {
                committed_qc: None,
                high_pqc: None,
                high_tc: None,
                proposal_hash: None,
                want_proposal: false,
                probe: None,
                echo: None,
                ..sample_status()
            })),
            WireMessage::SyncRequest(SyncRequest {
                instance: h(1),
                from_height: 4,
                max_count: 64,
                max_bytes: 1 << 24,
            }),
            WireMessage::SyncResponse(SyncResponse {
                instance: h(1),
                blocks: Vec::new(),
            }),
            WireMessage::SyncResponse(SyncResponse {
                instance: h(1),
                blocks: vec![
                    SyncEntry {
                        block: block.clone(),
                        commit_qc: sample_qc(VoteKind::Commit, 0),
                    },
                    SyncEntry {
                        block: Block {
                            payload: vec![],
                            ..block.clone()
                        },
                        commit_qc: sample_qc(VoteKind::Commit, 1),
                    },
                ],
            }),
            WireMessage::BlockRequest(BlockRequest {
                instance: h(1),
                height: 9,
                block_hash: h(7),
            }),
            WireMessage::BlockResponse(BlockResponse {
                instance: h(1),
                block,
            }),
            WireMessage::ApplicationControl(ApplicationControl {
                context: crate::api::ApplicationControlContext {
                    instance: h(1),
                    epoch: crate::testing::TEST_EPOCH.id,
                    height: 9,
                    parent_hash: h(2),
                    parent_result: h(3),
                },
                bytes: crate::types::ControlWitness::try_from_slice(b"canonical partial fixture")
                    .unwrap(),
            }),
        ]
    }

    #[test]
    fn vote_kind_bytes() {
        assert_eq!(VoteKind::Prepare.byte(), 0x02);
        assert_eq!(VoteKind::Commit.byte(), 0x03);
    }

    #[test]
    fn wire_round_trip_every_variant() {
        for message in all_wire_messages() {
            let bytes = message.encode().unwrap();
            let back = WireMessage::decode(&bytes, bytes.len()).unwrap();
            assert_eq!(back, message);
            assert_eq!(back.encode().unwrap(), bytes, "canonical re-encoding");
        }
    }

    fn round_trip<T>(value: &T)
    where
        T: norito::NoritoSerialize + PartialEq + fmt::Debug,
        for<'de> T: norito::NoritoDeserialize<'de>,
    {
        let bytes = norito::encode_canonical(value).unwrap();
        let back: T = norito::decode_canonical(&bytes).unwrap();
        assert_eq!(&back, value);
    }

    #[test]
    fn standalone_round_trips() {
        round_trip(&sample_header());
        round_trip(&Block {
            header: sample_header(),
            payload: vec![],
        });
        round_trip(&sample_proposal());
        round_trip(&sample_vote(VoteKind::Commit));
        round_trip(&sample_timeout());
        round_trip(&sample_qc(VoteKind::Prepare, 0));
        round_trip(&sample_tc());
        round_trip(&sample_status());
        round_trip(&Status::default());
        round_trip(&sample_status().echo.unwrap());
        let evidence = [
            Evidence::ProposalEquivocation(
                Box::new(sample_proposal()),
                Box::new(Proposal {
                    view: 5,
                    ..sample_proposal()
                }),
            ),
            Evidence::VoteEquivocation(
                sample_vote(VoteKind::Prepare),
                Vote {
                    result: h(99),
                    ..sample_vote(VoteKind::Prepare)
                },
            ),
            Evidence::TimeoutEquivocation(
                Box::new(sample_timeout()),
                Box::new(TimeoutVote {
                    high_pqc: None,
                    ..sample_timeout()
                }),
            ),
            Evidence::InvalidProposal {
                proposal: Box::new(sample_proposal()),
                defect: Defect::TcRule,
            },
            Evidence::ConflictingCertificates(
                sample_qc(VoteKind::Commit, 0),
                sample_qc(VoteKind::Commit, 1),
            ),
        ];
        for item in &evidence {
            round_trip(item);
        }
        let defects = [
            Defect::UnexpectedJustify,
            Defect::MissingJustify,
            Defect::InvalidJustify,
            Defect::MissingParentQc,
            Defect::UnexpectedParentQc,
            Defect::InvalidParentQc,
            Defect::HeaderInstance,
            Defect::HeaderHeight,
            Defect::ParentHash,
            Defect::ParentResult,
            Defect::PayloadTooLarge,
            Defect::TcRule,
            Defect::OriginView,
            Defect::Proposer,
            Defect::SkippedLeaders,
            Defect::EmptyPayload,
        ];
        for defect in defects {
            round_trip(&Evidence::InvalidProposal {
                proposal: Box::new(sample_proposal()),
                defect,
            });
        }
    }

    #[test]
    fn accessors() {
        let messages = all_wire_messages();
        for message in &messages {
            assert_eq!(message.instance(), &h(1));
        }
        let heights: Vec<_> = messages.iter().map(WireMessage::round_height).collect();
        assert_eq!(&heights[..10], &[Some(9); 10]);
        assert!(heights[10..].iter().all(Option::is_none));
        assert_eq!(heights.len(), 18);

        let tc = sample_tc();
        assert_eq!(tc.max_hq(), Some(2));
        assert_eq!(
            TimeoutCert {
                entries: vec![],
                ..tc
            }
            .max_hq(),
            None
        );
        assert_eq!(sample_timeout().hq(), Some(2));
        let qc = sample_qc(VoteKind::Prepare, 3);
        assert_eq!(qc.value(), (h(7), h(8)));
        assert_eq!(sample_vote(VoteKind::Prepare).value(), (h(7), h(8)));
        assert_eq!(qc.preimage(), sample_vote(VoteKind::Prepare).preimage());

        let proposal = sample_proposal();
        let block = proposal.block().unwrap();
        assert_eq!(block.header, proposal.header);
        assert!(
            Proposal {
                payload: None,
                ..proposal.clone()
            }
            .block()
            .is_none()
        );
        let crypto = FakeCrypto::new();
        assert_eq!(proposal.block_hash(&crypto), block.hash(&crypto));
        assert_eq!(
            proposal.signing_preimage(&crypto),
            preimage::prop_preimage(
                &proposal.instance,
                &crate::testing::TEST_EPOCH.id,
                9,
                4,
                &block.hash(&crypto),
                &proposal.att_digest(&crypto)
            )
        );
        assert_eq!(qc.digest(&crypto), preimage::qc_digest(&crypto, &qc));
        assert_eq!(
            sample_tc().digest(&crypto),
            preimage::tc_digest(&crypto, &sample_tc())
        );
        // body_ok with a fake hash: the sample header's payload_hash is arbitrary.
        assert!(!block.body_ok(&crypto));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn decode_rejects_oversize_and_limits() {
        let message = WireMessage::Vote(sample_vote(VoteKind::Prepare));
        let bytes = message.encode().unwrap();
        assert_eq!(
            WireMessage::decode(&bytes, bytes.len() - 1),
            Err(CodecError::TooLarge {
                len: bytes.len(),
                max: bytes.len() - 1
            })
        );

        let big_bitmap = Qc {
            signers: Bitmap::from_bytes(vec![0; MAX_BITMAP_BYTES + 1]),
            ..sample_qc(VoteKind::Commit, 0)
        };
        let cases = [
            WireMessage::Qc(big_bitmap.clone()),
            WireMessage::Timeout(Box::new(TimeoutVote {
                high_pqc: Some(big_bitmap.clone()),
                ..sample_timeout()
            })),
            WireMessage::Tc(Box::new(TimeoutCert {
                entries: vec![
                    TcEntry {
                        signer: 0,
                        hq: None
                    };
                    MAX_COMMITTEE_SIZE + 1
                ],
                ..sample_tc()
            })),
            WireMessage::Tc(Box::new(TimeoutCert {
                high_pqc: Some(big_bitmap.clone()),
                ..sample_tc()
            })),
            WireMessage::Status(Box::new(Status {
                committed_qc: Some(big_bitmap.clone()),
                ..sample_status()
            })),
            WireMessage::Status(Box::new(Status {
                high_pqc: Some(big_bitmap.clone()),
                ..sample_status()
            })),
            WireMessage::Status(Box::new(Status {
                high_tc: Some(TimeoutCert {
                    high_pqc: Some(big_bitmap.clone()),
                    ..sample_tc()
                }),
                ..sample_status()
            })),
            WireMessage::Status(Box::new(Status {
                echo: Some(Echo {
                    epoch: crate::testing::TEST_EPOCH.id,
                    nonce: 1,
                    key: PublicKey::unchecked(vec![]),
                    sig: Signature([0; SIGNATURE_LEN]),
                }),
                ..sample_status()
            })),
            WireMessage::Proposal(Box::new(Proposal {
                parent_qc: Some(big_bitmap.clone()),
                ..sample_proposal()
            })),
            WireMessage::Proposal(Box::new(Proposal {
                justify: Some(TimeoutCert {
                    high_pqc: Some(big_bitmap.clone()),
                    ..sample_tc()
                }),
                ..sample_proposal()
            })),
            WireMessage::Proposal(Box::new(Proposal {
                header: BlockHeader {
                    skipped_leaders: vec![key(1); MAX_COMMITTEE_SIZE + 1],
                    ..sample_header()
                },
                ..sample_proposal()
            })),
            WireMessage::BlockResponse(BlockResponse {
                instance: h(1),
                block: Block {
                    header: BlockHeader {
                        skipped_leaders: vec![PublicKey::unchecked(vec![])],
                        ..sample_header()
                    },
                    payload: vec![],
                },
            }),
            WireMessage::SyncResponse(SyncResponse {
                instance: h(1),
                blocks: vec![SyncEntry {
                    block: Block {
                        header: sample_header(),
                        payload: vec![],
                    },
                    commit_qc: big_bitmap,
                }],
            }),
        ];
        for case in cases {
            assert!(
                matches!(case.check_limits(), Err(CodecError::Limit(_))),
                "{case:?}"
            );
            let bytes = case.encode().unwrap();
            assert!(matches!(
                WireMessage::decode(&bytes, usize::MAX),
                Err(CodecError::Limit(_))
            ));
        }
        let too_many = WireMessage::SyncResponse(SyncResponse {
            instance: h(1),
            blocks: vec![
                SyncEntry {
                    block: Block {
                        header: BlockHeader {
                            skipped_leaders: vec![],
                            ..sample_header()
                        },
                        payload: vec![],
                    },
                    commit_qc: sample_qc(VoteKind::Commit, 0),
                };
                MAX_SYNC_ENTRIES + 1
            ],
        });
        assert_eq!(
            too_many.check_limits(),
            Err(CodecError::Limit("sync entries"))
        );
        assert_eq!(
            CodecError::Limit("x").to_string(),
            "limit exceeded: x".to_string()
        );
        assert_eq!(
            CodecError::TooLarge { len: 2, max: 1 }.to_string(),
            "frame of 2 bytes exceeds 1"
        );
        assert!(CodecError::Norito("e".into()).to_string().contains('e'));
    }

    #[test]
    fn decode_garbage_never_panics() {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let encoded: Vec<Vec<u8>> = all_wire_messages()
            .iter()
            .map(|m| m.encode().unwrap())
            .collect();
        let mut accepted = 0usize;
        for round in 0..6_000u32 {
            let base = &encoded[usize::try_from(next() % encoded.len() as u64).unwrap()];
            let mut bytes = base.clone();
            match round % 4 {
                0 => {
                    let flips = 1 + next() % 4;
                    for _ in 0..flips {
                        let pos = usize::try_from(next() % bytes.len() as u64).unwrap();
                        bytes[pos] ^= u8::try_from(next() % 255 + 1).unwrap();
                    }
                }
                1 => {
                    let cut = usize::try_from(next() % bytes.len() as u64).unwrap();
                    bytes.truncate(cut);
                }
                2 => {
                    let len = usize::try_from(next() % 512).unwrap();
                    bytes = (0..len)
                        .map(|_| u8::try_from(next() & 0xff).unwrap())
                        .collect();
                }
                _ => {
                    // Corrupt the payload but keep the header intact.
                    let pos = 40 + usize::try_from(next() % (bytes.len() as u64 - 40)).unwrap();
                    bytes[pos] = u8::try_from(next() & 0xff).unwrap();
                }
            }
            if let Ok(message) = WireMessage::decode(&bytes, 1 << 20) {
                accepted += 1;
                // Anything accepted re-encodes to exactly the same bytes.
                assert_eq!(message.encode().unwrap(), bytes);
            }
        }
        // Checksums reject nearly every mutation.
        assert!(accepted < 200, "accepted {accepted}");
    }

    /// §3.5: the wire tag of every variant is its position, pinned against the encoded frame
    /// (bytes 40..44 after the Norito header), with its traffic class (§12.3 O8, E44).
    #[test]
    fn wire_tags_and_classes_pinned() {
        use TrafficClass::{Bulk, Control, Proposal as Prop};
        let expected: [(&str, u32, TrafficClass); 18] = [
            ("proposal with payload", 0, Prop),
            ("proposal without payload", 0, Control),
            ("prepare vote", 1, Control),
            ("commit vote", 1, Control),
            ("prepare qc", 2, Control),
            ("commit qc", 2, Control),
            ("timeout", 3, Control),
            ("timeout without lock", 3, Control),
            ("tc", 4, Control),
            ("empty tc", 4, Control),
            ("status", 5, Control),
            ("bare status", 5, Control),
            ("sync request", 6, Control),
            ("empty sync response", 7, Bulk),
            ("sync response", 7, Bulk),
            ("block request", 8, Control),
            ("block response", 9, Prop),
            ("application control", 10, Control),
        ];
        let messages = all_wire_messages();
        assert_eq!(messages.len(), expected.len());
        for (message, (name, tag, class)) in messages.iter().zip(expected) {
            let frame = message.encode().unwrap();
            assert_eq!(message.wire_tag(), tag, "{name}");
            assert_eq!(
                u32::from_le_bytes(frame[40..44].try_into().unwrap()),
                tag,
                "{name}: the Norito enum tag"
            );
            assert_eq!(message.traffic_class(), class, "{name}");
            assert_eq!(
                traffic_class_of_frame(&frame),
                Some(class),
                "{name}: raw frame"
            );
        }
        assert_eq!(PROTOCOL_VERSION, 8);
        assert!(TrafficClass::Control < TrafficClass::Proposal);
        assert!(TrafficClass::Proposal < TrafficClass::Bulk);
        assert_eq!(class_of_tag(11, false), None);
        assert_eq!(class_of_tag(u32::MAX, true), None);
    }

    #[test]
    fn every_nested_witness_is_admitted_before_retention_and_retry_preserves_wire() {
        use mv::allocation::{AllocationBudget, ChargedBuffer};
        let witness = ResultWitness::from_untrusted(vec![7; 200]).unwrap();
        let qc = Qc {
            attestation_witness: Some(witness.clone()),
            ..sample_qc(VoteKind::Commit, 0)
        };
        let tc = TimeoutCert {
            high_pqc: Some(qc.clone()),
            ..sample_tc()
        };
        let messages = vec![
            WireMessage::Vote(Vote {
                attestation: Some(CommitAttestation {
                    witness: witness.clone(),
                    signature: AttestationSignature::empty(),
                }),
                ..sample_vote(VoteKind::Commit)
            }),
            WireMessage::Qc(qc.clone()),
            WireMessage::Proposal(Box::new(Proposal {
                parent_qc: Some(qc.clone()),
                justify: Some(tc.clone()),
                ..sample_proposal()
            })),
            WireMessage::Timeout(Box::new(TimeoutVote {
                high_pqc: Some(qc.clone()),
                ..sample_timeout()
            })),
            WireMessage::Tc(Box::new(tc.clone())),
            WireMessage::Status(Box::new(Status {
                committed_qc: Some(qc.clone()),
                high_pqc: Some(qc.clone()),
                high_tc: Some(tc),
                ..sample_status()
            })),
            WireMessage::SyncResponse(SyncResponse {
                instance: qc.instance,
                blocks: vec![SyncEntry {
                    block: Block {
                        header: sample_header(),
                        payload: Vec::new(),
                    },
                    commit_qc: qc,
                }],
            }),
        ];
        for mut message in messages {
            let before = message.encode().unwrap();
            let budget = AllocationBudget::new(4096);
            assert!(!message.attestation_witnesses_admitted_to(&budget));
            let occupied = ChargedBuffer::<u8>::new(4096, &budget).unwrap();
            assert!(
                message
                    .admit_attestation_witnesses(&budget)
                    .unwrap_err()
                    .is_local_refusal()
            );
            assert_eq!(message.encode().unwrap(), before);
            drop(occupied);
            message.admit_attestation_witnesses(&budget).unwrap();
            assert!(message.attestation_witnesses_admitted_to(&budget));
            let reserved = budget.reserved_bytes();
            assert!(reserved >= 200);
            let mut clone = message.clone();
            clone.admit_attestation_witnesses(&budget).unwrap();
            assert_eq!(
                budget.reserved_bytes(),
                reserved,
                "same pool clones share actual owners"
            );
            assert_eq!(clone.encode().unwrap(), before);
            let foreign = AllocationBudget::new(4096);
            assert!(!message.attestation_witnesses_admitted_to(&foreign));
            assert!(matches!(
                clone.admit_attestation_witnesses(&foreign),
                Err(WitnessAdmissionError::ForeignBudget)
            ));
            drop(message);
            assert_eq!(budget.reserved_bytes(), reserved);
            drop(clone);
            assert_eq!(budget.reserved_bytes(), 0);
        }
    }

    #[test]
    fn source_complete_maximum_certificate_and_header_fit_declared_frame_allowance() {
        let count = MAX_COMMITTEE_SIZE;
        let witness = ResultWitness::from_untrusted(vec![7; MAX_RESULT_WITNESS_BYTES]).unwrap();
        let mut header = sample_header();
        header.skipped_leaders = (0..count)
            .map(|i| {
                let mut bytes = vec![5; crate::types::MAX_PUBLIC_KEY_LEN];
                bytes[..8].copy_from_slice(&(i as u64).to_be_bytes());
                PublicKey::new(bytes).unwrap()
            })
            .collect();
        header.control_witness = crate::types::ControlWitness::try_from_slice(
            &[3; crate::types::MAX_CONTROL_WITNESS_BYTES],
        )
        .unwrap();
        let parent = Qc {
            attest: true,
            signers: Bitmap::from_indices(count, (0..count).map(crate::types::index_of)).unwrap(),
            attestations: vec![
                AttestationSignature::try_from_slice(
                    &[9; MAX_ATTESTATION_SIGNATURE_BYTES]
                )
                .unwrap();
                count
            ],
            attestation_witness: Some(witness),
            ..sample_qc(VoteKind::Commit, 0)
        };
        let mut tc = sample_tc();
        tc.entries = (0..count)
            .map(|i| TcEntry {
                signer: crate::types::index_of(i),
                hq: Some(2),
            })
            .collect();
        let proposal = WireMessage::Proposal(Box::new(Proposal {
            header,
            parent_qc: Some(parent),
            justify: Some(tc),
            payload: Some(Vec::new()),
            ..sample_proposal()
        }));
        let frame = proposal.encode().unwrap();
        assert!(
            frame.len() <= crate::pacemaker::FRAME_OVERHEAD as usize,
            "{} bytes exceed allowance",
            frame.len()
        );
        assert!(WireMessage::decode(&frame, crate::pacemaker::FRAME_OVERHEAD as usize).is_ok());
    }

    /// The raw classifier refuses what is not a canonical `WireMessage` frame.
    #[test]
    fn raw_classification_rejects_other_frames() {
        let frame = WireMessage::Proposal(Box::new(sample_proposal()))
            .encode()
            .unwrap();
        assert_eq!(traffic_class_of_frame(&frame), Some(TrafficClass::Proposal));
        let mut cases: Vec<Vec<u8>> = vec![
            Vec::new(),
            frame[..39].to_vec(),
            frame[..frame.len() - 1].to_vec(),
            [frame.clone(), vec![0]].concat(),
        ];
        for (at, value) in [(0, b'X'), (4, 1), (10, 0xff), (22, 1), (39, 0)] {
            let mut bad = frame.clone();
            bad[at] = value;
            cases.push(bad);
        }
        let mut unknown = WireMessage::Vote(sample_vote(VoteKind::Prepare))
            .encode()
            .unwrap();
        unknown[40] = u8::MAX; // Every defined first-release message tag is distinct from this.
        cases.push(unknown);
        // Another Norito type (a `Status` struct frame, not a `WireMessage`).
        cases.push(norito::encode_canonical(&sample_status()).unwrap());
        for case in &cases {
            assert_eq!(traffic_class_of_frame(case), None, "{case:?}");
        }
        assert_eq!(take_len(&[0x80, 0x00]), None, "overlong length");
        assert_eq!(take_len(&[0xff; 11]), None, "unterminated length");
        let mut max = vec![0xff; 9];
        max.push(0x01);
        assert_eq!(
            take_len(&max).map(|(v, _)| u64::try_from(v).unwrap()),
            Some(u64::MAX)
        );
        max[9] = 0x02;
        assert_eq!(take_len(&max), None, "beyond 64 bits");
        assert_eq!(take_len(&[0x85, 0x01, 7]), Some((133, &[7u8][..])));
    }

    /// Property (§3.5): for random frames — valid frames of every variant with random bytes
    /// flipped, truncated, extended or re-encoded — the raw classification of every frame that
    /// decodes equals the decoded message's class, and the raw classifier never panics.
    #[test]
    fn raw_and_decoded_classification_agree() {
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut messages = all_wire_messages();
        // Larger payloads and attestations exercise multi-byte lengths.
        messages.push(WireMessage::Proposal(Box::new(Proposal {
            payload: Some(vec![0x5a; 300]),
            ..sample_proposal()
        })));
        messages.push(WireMessage::Vote(Vote {
            attest: true,
            attestation: Some(CommitAttestation {
                witness: ResultWitness::from_untrusted(vec![1; 200]).unwrap(),
                signature: AttestationSignature::try_from_slice(&[1; 200]).unwrap(),
            }),
            ..sample_vote(VoteKind::Commit)
        }));
        let frames: Vec<Vec<u8>> = messages.iter().map(|m| m.encode().unwrap()).collect();
        let (mut decoded, mut raw_only) = (0usize, 0usize);
        for round in 0..20_000u32 {
            let base = &frames[usize::try_from(next() % frames.len() as u64).unwrap()];
            let mut bytes = base.clone();
            match round % 5 {
                0 => {}
                1 => {
                    let pos = usize::try_from(next() % bytes.len() as u64).unwrap();
                    bytes[pos] ^= u8::try_from(next() % 255 + 1).unwrap();
                }
                2 => {
                    let cut = usize::try_from(next() % bytes.len() as u64).unwrap();
                    bytes.truncate(cut);
                }
                3 => bytes.push(u8::try_from(next() & 0xff).unwrap()),
                _ => {
                    let pos = 40 + usize::try_from(next() % (bytes.len() as u64 - 40)).unwrap();
                    bytes[pos] = u8::try_from(next() & 0xff).unwrap();
                }
            }
            let raw = traffic_class_of_frame(&bytes);
            match WireMessage::decode(&bytes, 1 << 20) {
                Ok(message) => {
                    decoded += 1;
                    assert_eq!(raw, Some(message.traffic_class()), "{message:?}");
                }
                Err(_) => raw_only += usize::from(raw.is_some()),
            }
        }
        assert!(decoded >= 4_000, "decoded {decoded}");
        // Frames the raw classifier accepts without decoding are only queued, never trusted.
        assert!(raw_only > 0, "some undecodable frames still classify");
    }

    /// Decode limits of the attestation fields (§3.7).
    #[test]
    fn attestation_limits() {
        let witness = ResultWitness::from_untrusted(vec![7; MAX_RESULT_WITNESS_BYTES]).unwrap();
        let signature =
            AttestationSignature::try_from_slice(&[0; MAX_ATTESTATION_SIGNATURE_BYTES]).unwrap();
        let vote = Vote {
            attest: true,
            attestation: Some(CommitAttestation {
                witness: witness.clone(),
                signature,
            }),
            ..sample_vote(VoteKind::Commit)
        };
        assert_eq!(WireMessage::Vote(vote.clone()).check_limits(), Ok(()));
        assert!(
            AttestationSignature::try_from_slice(&[0; MAX_ATTESTATION_SIGNATURE_BYTES + 1])
                .is_err()
        );
        assert!(ResultWitness::from_untrusted(vec![0; MAX_RESULT_WITNESS_BYTES + 1]).is_err());
        let vote_bytes = WireMessage::Vote(vote.clone()).encode().unwrap();
        assert_eq!(
            WireMessage::decode(&vote_bytes, vote_bytes.len()).unwrap(),
            WireMessage::Vote(vote)
        );
        let qc = Qc {
            attest: true,
            attestation_witness: Some(witness),
            attestations: vec![signature; 3],
            ..sample_qc(VoteKind::Commit, 0)
        };
        assert_eq!(WireMessage::Qc(qc.clone()).check_limits(), Ok(()));
        let bad = Qc {
            attestations: vec![AttestationSignature::empty(); MAX_COMMITTEE_SIZE + 1],
            ..qc.clone()
        };
        assert!(matches!(
            WireMessage::Qc(bad.clone()).check_limits(),
            Err(CodecError::Limit(_))
        ));
        let bytes = WireMessage::Qc(bad).encode().unwrap();
        assert!(matches!(
            WireMessage::decode(&bytes, usize::MAX),
            Err(CodecError::Limit(_))
        ));
        // Round trip with attestations, and the accessors.
        let bytes = WireMessage::Qc(qc.clone()).encode().unwrap();
        assert_eq!(
            WireMessage::decode(&bytes, bytes.len()).unwrap(),
            WireMessage::Qc(qc.clone())
        );
        assert!(qc.needs_attestations());
        assert!(!sample_qc(VoteKind::Commit, 0).needs_attestations());
        assert!(
            !Qc {
                attest: true,
                ..sample_qc(VoteKind::Prepare, 0)
            }
            .needs_attestations()
        );
        assert_eq!(
            qc.statement(),
            preimage::att_preimage(
                &qc.instance,
                &crate::testing::TEST_EPOCH.id,
                qc.height,
                &qc.block_hash,
                &qc.result
            )
        );
        let commit = sample_vote(VoteKind::Commit);
        assert!(!commit.needs_attestation());
        assert!(
            Vote {
                attest: true,
                ..commit.clone()
            }
            .needs_attestation()
        );
        assert_eq!(commit.statement(), qc.statement());
    }

    #[test]
    fn typical_encoded_sizes() {
        let sizes: Vec<(&str, usize)> = vec![
            (
                "vote",
                WireMessage::Vote(sample_vote(VoteKind::Prepare))
                    .encode()
                    .unwrap()
                    .len(),
            ),
            (
                "qc",
                WireMessage::Qc(sample_qc(VoteKind::Commit, 0))
                    .encode()
                    .unwrap()
                    .len(),
            ),
            (
                "timeout",
                WireMessage::Timeout(Box::new(sample_timeout()))
                    .encode()
                    .unwrap()
                    .len(),
            ),
        ];
        for (name, size) in sizes {
            // Spec typical sizes (§3.6) are ≈ 215 B / 215 B / ≤ 440 B raw; Norito adds a 40-byte
            // header and per-field length prefixes.
            assert!(size < 1_000, "{name}: {size}");
        }
    }
    #[test]
    fn application_control_frame_is_bounded_classified_and_not_an_empty_default() {
        let mut message = all_wire_messages().pop().unwrap();
        assert!(matches!(message, WireMessage::ApplicationControl(_)));
        let WireMessage::ApplicationControl(ref mut partial) = message else {
            unreachable!()
        };
        partial.bytes = crate::types::ControlWitness::try_from_slice(
            &[0xAB; crate::types::MAX_CONTROL_WITNESS_BYTES],
        )
        .unwrap();
        let bytes = message.encode().unwrap();
        assert_eq!(traffic_class_of_frame(&bytes), Some(TrafficClass::Control));
        assert_eq!(WireMessage::decode(&bytes, bytes.len()).unwrap(), message);
        assert!(matches!(
            WireMessage::decode(&bytes, bytes.len() - 1),
            Err(CodecError::TooLarge { .. })
        ));
        let WireMessage::ApplicationControl(ref mut partial) = message else {
            unreachable!()
        };
        partial.bytes = crate::types::ControlWitness::empty();
        assert_eq!(
            message.check_limits(),
            Err(CodecError::Limit("empty application control"))
        );
    }
    #[test]
    fn native_evidence_frame_is_exact_and_bounded() {
        let report = Evidence::ConflictingCertificates(
            sample_qc(VoteKind::Commit, 0),
            sample_qc(VoteKind::Commit, 1),
        );
        let frame = report.encode().unwrap();
        assert_eq!(Evidence::decode(&frame).unwrap(), report);
        let mut suffixed = frame.clone();
        suffixed.push(0);
        assert!(Evidence::decode(&suffixed).is_err());
        assert!(Evidence::decode(&frame[..frame.len() - 1]).is_err());
        assert!(Evidence::decode(&vec![0; MAX_EVIDENCE_FRAME_BYTES + 1]).is_err());
        let wire = WireMessage::Qc(sample_qc(VoteKind::Commit, 0))
            .encode()
            .unwrap();
        assert!(
            Evidence::decode(&wire).is_err(),
            "message schema is not evidence schema"
        );
    }

    #[test]
    fn native_evidence_rejects_oversized_certificate_shape_before_encoding() {
        let mut certificate = sample_qc(VoteKind::Commit, 0);
        certificate.signers = Bitmap::new((MAX_BITMAP_BYTES + 1) * 8);
        let report = Evidence::ConflictingCertificates(certificate, sample_qc(VoteKind::Commit, 1));
        assert!(report.check_limits().is_err());
        assert!(report.encode().is_err());
        let bytes = norito::encode_canonical(&report).unwrap();
        assert!(Evidence::decode(&bytes).is_err());
    }
}
