//! Signer, crypto and commit-attestation traits (spec §12.1, §3.7) and pure verification and
//! formation functions for votes, timeouts, proposals and certificates (§3.4, §3.7, §6.2 step 1,
//! §6.4, §6.7, §11).
//!
//! Certificates are verified only against the committee of their own height (`C_{cert.height}`);
//! passing that committee is the caller's obligation (SR15).

use std::collections::BTreeMap;

use crate::{
    message::{
        BlockHeader, CommitAttestation, Proposal, Qc, ResultWitness, TcEntry, TimeoutCert,
        TimeoutVote, Vote, VoteKind,
    },
    preimage,
    types::{AggregateSignature, Bitmap, Committee, Hash32, PublicKey, Signature, ValidatorIndex},
};

/// A local, non-blocking signing key (§12.1). `sign` runs inside `handle`. It MUST be
/// deterministic (the same key and preimage always give the same bytes, as BLS does): after a
/// restart the core re-creates recorded messages by signing their recorded preimages again
/// (§6.10 rule 0, §7.4).
pub trait Signer {
    /// The consensus public key.
    fn public_key(&self) -> &PublicKey;
    /// Sign `preimage`.
    fn sign(&self, preimage: &[u8]) -> Signature;
}

/// Pure cryptographic primitives (§12.1): BLS in production, a fake scheme in the simulator.
pub trait Crypto {
    /// The chain hash `H`.
    fn hash(&self, bytes: &[u8]) -> Hash32;
    /// Verify an individual signature.
    fn verify(&self, pk: &PublicKey, msg: &[u8], sig: &Signature) -> bool;
    /// Aggregate signatures (possibly over different messages).
    fn aggregate(&self, sigs: &[Signature]) -> AggregateSignature;
    /// Verify an aggregate of signatures by `pks` over one message.
    fn verify_aggregate(&self, pks: &[&PublicKey], msg: &[u8], agg: &AggregateSignature) -> bool;
    /// Verify an aggregate over several `(keys, message)` groups.
    fn verify_aggregate_multi(
        &self,
        groups: &[(Vec<&PublicKey>, Vec<u8>)],
        agg: &AggregateSignature,
    ) -> bool;
}

/// An [`Attestor`]'s answer for one statement (§3.7 A2).
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant, reason = "transient `Attestor` result")]
pub enum AttestOutcome {
    /// The attestation bytes: the same for the same inputs.
    Attested(CommitAttestation),
    /// Not yet: the authority lacks application data the attestation binds that only this
    /// node's own execution of the block provides (in KAGEMUSHA, `R`'s preimage). No Commit
    /// vote and no fault; the core asks again after each `Valid` execution of the current
    /// proposal's block and at each stage raise.
    Pending,
    /// This node holds no authority for member `key` at `height`: it does not Commit-vote on
    /// flagged blocks there (`LocalFault(AttestationUnavailable)` once per view).
    NoAuthority,
}

/// A node's application authority for commit attestations (§3.7), e.g. its KAGEMUSHA
/// mint-finality authority. Like [`Signer::sign`], `attest` runs inside `handle` and MUST be
/// local, non-blocking and deterministic (the same inputs give the same bytes).
pub trait Attestor {
    /// Attest `statement` (`att_preimage(height, bh, R)`, §3.3) as the authority of member `key`
    /// at `height` (§3.7 A2): [`AttestOutcome::NoAuthority`] iff this node holds no authority for
    /// `(height, key)`, [`AttestOutcome::Pending`] while it still needs its own execution of `bh`.
    fn attest(&self, height: u64, key: &PublicKey, statement: &[u8]) -> AttestOutcome;
}

/// Verifies commit attestations (§3.7). `verify` MUST be a pure function of its arguments and of
/// committed application state known wherever `C_height` is known, never of local execution
/// (sync, `Status` and `parent_qc` verify certificates of blocks this node has not executed). It
/// MUST accept only an attestation that the authority of member `key` (canonical index `signer`
/// of `C_height`) produced over exactly `statement`, and whatever else the attestation binds (its
/// application message) MUST be determined by `statement` and that state: all attestations it
/// accepts for one statement at one height bind one message. An attestation therefore carries
/// the preimage of every statement digest it binds (e.g. `R`'s), which `verify` re-hashes.
pub trait AttestationVerifier {
    /// Whether `attestation` is member `key`'s valid attestation of `statement` at `height`.
    fn verify(
        &self,
        height: u64,
        signer: ValidatorIndex,
        key: &PublicKey,
        statement: &[u8],
        witness: &ResultWitness,
        attestation: &[u8],
    ) -> bool;
}

/// The attestor of a node without attestation authority, and the verifier that accepts no
/// attestation: through it no flagged block ever commits (fail closed, §3.7 A5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoAttestation;

impl Attestor for NoAttestation {
    fn attest(&self, _height: u64, _key: &PublicKey, _statement: &[u8]) -> AttestOutcome {
        AttestOutcome::NoAuthority
    }
}

impl AttestationVerifier for NoAttestation {
    fn verify(
        &self,
        _height: u64,
        _signer: ValidatorIndex,
        _key: &PublicKey,
        _statement: &[u8],
        _witness: &ResultWitness,
        _attestation: &[u8],
    ) -> bool {
        false
    }
}

/// A node's commit-attestation extension (§3.7), passed to `Core::new`: its attestor and the
/// verifier of every attestation it receives.
pub struct Attestation {
    /// The node's authority (may attest nothing).
    pub attestor: Box<dyn Attestor>,
    /// The verifier of every node's attestations.
    pub verifier: Box<dyn AttestationVerifier>,
}

impl Attestation {
    /// An extension from its two parts.
    pub fn new(attestor: Box<dyn Attestor>, verifier: Box<dyn AttestationVerifier>) -> Self {
        Self { attestor, verifier }
    }

    /// No attestation support ([`NoAttestation`]): for an application that never flags a block.
    pub fn none() -> Self {
        Self::new(Box::new(NoAttestation), Box::new(NoAttestation))
    }
}

impl core::fmt::Debug for Attestation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Attestation")
    }
}

/// Why a vote, timeout, proposal or certificate failed verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertError {
    /// `instance ≠ I`.
    WrongInstance,
    /// Scheduling epoch or complete context differs from the authenticated height.
    WrongEpoch,
    /// A signer index `≥ n`.
    SignerOutOfRange,
    /// Bitmap length not `ceil(n/8)` or spare bits set.
    MalformedBitmap,
    /// Fewer than `q` distinct signers.
    TooFewSigners,
    /// More than `q` signers: certificates have exactly one equal-vote quorum.
    TooManySigners,
    /// TC entry indices not strictly increasing.
    UnorderedEntries,
    /// A TC entry or timeout declares `hq > view`.
    HqAboveView,
    /// `high_pqc` absent although some `hq` is set, or present although none is.
    HighQcPresence,
    /// `high_pqc` is not a Prepare QC of this height with view `max(hq)` (or `hq`).
    HighQcMismatch,
    /// The carried `high_pqc` does not verify.
    HighQcInvalid,
    /// Signature or aggregate signature does not verify.
    BadSignature,
    /// Attestations present where none belong, not exactly one per signer where they are
    /// required, or a flagged `CommitQC` without exactly `q` signers (§3.7 A3, A4).
    AttestationShape,
    /// An attestation does not verify (§3.7).
    BadAttestation,
}

/// Consensus signature verification bound to one independently authenticated authority.
///
/// Construct this from the original chain configuration, never from the artifact being checked.
/// Reusing the context for related artifacts prevents mixing instance, epoch or committee inputs.
#[derive(Clone, Copy)]
pub struct Verifier<'a> {
    crypto: &'a dyn Crypto,
    instance: &'a Hash32,
    epoch: &'a crate::types::EpochId,
    committee: &'a Committee,
}
impl<'a> Verifier<'a> {
    /// Bind a crypto implementation to one authenticated instance and scheduled committee.
    pub fn new(
        crypto: &'a dyn Crypto,
        instance: &'a Hash32,
        epoch: &'a crate::types::EpochId,
        committee: &'a Committee,
    ) -> Self {
        Self {
            crypto,
            instance,
            epoch,
            committee,
        }
    }

    fn key(
        &self,
        instance: &Hash32,
        epoch: &crate::types::EpochId,
        signer: ValidatorIndex,
    ) -> Result<&PublicKey, CertError> {
        reject_if(instance != self.instance, CertError::WrongInstance)?;
        reject_if(epoch != self.epoch, CertError::WrongEpoch)?;
        self.committee
            .get(signer)
            .ok_or(CertError::SignerOutOfRange)
    }

    /// Verify a `PrepareQC` or `CommitQC` under `committee = C_{qc.height}` (§3.4 `verify_qc`):
    /// instance, exact bitmap, `popcount == q`, aggregate signature over the vote preimage, and the
    /// attestation rule A4 of §3.7 with `verifier`. Every certificate has exactly `q` signers,
    /// including unflagged Prepare and Commit certificates.
    ///
    /// # Errors
    /// The first failed check.
    pub fn verify_qc(&self, verifier: &dyn AttestationVerifier, qc: &Qc) -> Result<(), CertError> {
        self.verify_qc_signatures(qc)?;
        // MA2: the attestation check of a flagged CommitQC is skipped.
        #[cfg(not(sumeragi_mutation = "MA2"))]
        verify_attestations(verifier, self.committee, qc)?;
        #[cfg(sumeragi_mutation = "MA2")]
        let _ = verifier;
        Ok(())
    }

    /// [`Verifier::verify_qc`] without the attestation rule: instance, exact bitmap, `popcount == q` and the
    /// aggregate signature. Only the safety monitor (§7.6) uses it alone: `q` signatures on another
    /// value already prove an agreement violation.
    ///
    /// # Errors
    /// The first failed check.
    pub fn verify_qc_signatures(&self, qc: &Qc) -> Result<(), CertError> {
        reject_if(&qc.instance != self.instance, CertError::WrongInstance)?;
        reject_if(&qc.epoch != self.epoch, CertError::WrongEpoch)?;
        let pks = self
            .committee
            .keys_of(&qc.signers)
            .ok_or(CertError::MalformedBitmap)?;
        #[cfg(not(sumeragi_mutation = "MS14"))]
        let needed = self.committee.q();
        #[cfg(sumeragi_mutation = "MS14")]
        let needed = self.committee.q().saturating_sub(1);
        reject_if(pks.len() < needed, CertError::TooFewSigners)?;
        // MS39: otherwise genuine over-aggregated Prepare/Commit certificates are accepted.
        reject_if(
            pks.len() > self.committee.q() && !cfg!(sumeragi_mutation = "MS39"),
            CertError::TooManySigners,
        )?;
        (self
            .crypto
            .verify_aggregate(&pks, &qc.preimage(), &qc.agg_sig))
        .then_some(())
        .ok_or(CertError::BadSignature)
    }

    /// Light-client check of a `CommitQC` (§11, §3.7): `kind == Commit`, [`Verifier::verify_qc`] under the
    /// committee of the certificate's height and, when the caller holds the certified header, that
    /// the header is the certified block and carries the certificate's flag.
    pub fn verify_commit_qc(
        &self,
        verifier: &dyn AttestationVerifier,
        qc: &Qc,
        header: Option<&BlockHeader>,
    ) -> bool {
        let header_ok = header.is_none_or(|header| {
            header.attest == qc.attest
                && header.epoch == *self.epoch
                && header.height == qc.height
                && header.hash(self.crypto) == qc.block_hash
        });
        qc.kind == VoteKind::Commit && header_ok && self.verify_qc(verifier, qc).is_ok()
    }

    /// Verify a TC under `committee = C_{tc.height}` (§3.4 `verify_tc`), including the recomputation
    /// of `max(hq)` and the full verification of `high_pqc` (SR12).
    ///
    /// # Errors
    /// The first failed check.
    pub fn verify_tc(&self, tc: &TimeoutCert) -> Result<(), CertError> {
        self.verify_tc_inner(tc, true)
    }

    /// [`Verifier::verify_tc`] for a caller that has already verified (or holds a cached verdict for) the exact
    /// `tc.high_pqc`: every check except the signature check of `high_pqc` itself.
    ///
    /// # Errors
    /// The first failed check.
    pub fn verify_tc_with_verified_high_qc(&self, tc: &TimeoutCert) -> Result<(), CertError> {
        self.verify_tc_inner(tc, false)
    }

    fn verify_tc_inner(&self, tc: &TimeoutCert, check_high_qc: bool) -> Result<(), CertError> {
        reject_if(&tc.instance != self.instance, CertError::WrongInstance)?;
        reject_if(&tc.epoch != self.epoch, CertError::WrongEpoch)?;
        reject_if(
            tc.entries.len() < self.committee.q(),
            CertError::TooFewSigners,
        )?;
        // MS40: an over-aggregated Timeout certificate is accepted on either verification path.
        reject_if(
            tc.entries.len() > self.committee.q() && !cfg!(sumeragi_mutation = "MS40"),
            CertError::TooManySigners,
        )?;
        // Group signers by signed `hq` (BTreeMap: deterministic group order).
        let mut groups: BTreeMap<Option<u64>, Vec<&PublicKey>> = BTreeMap::new();
        let mut previous: Option<ValidatorIndex> = None;
        for entry in &tc.entries {
            reject_if(
                previous.is_some_and(|p| entry.signer <= p),
                CertError::UnorderedEntries,
            )?;
            previous = Some(entry.signer);
            let pk = self
                .committee
                .get(entry.signer)
                .ok_or(CertError::SignerOutOfRange)?;
            reject_if(
                entry.hq.is_some_and(|hq| hq > tc.view),
                CertError::HqAboveView,
            )?;
            groups.entry(entry.hq).or_default().push(pk);
        }
        // SR12: the attached PrepareQC must be the one of the maximal *signed* hq.
        match (tc.max_hq(), &tc.high_pqc) {
            (None, None) => {}
            (Some(max), Some(qc)) => {
                reject_if(
                    qc.kind != VoteKind::Prepare
                        || qc.instance != tc.instance
                        || qc.epoch != tc.epoch
                        || qc.height != tc.height
                        || (qc.view != max && !cfg!(sumeragi_mutation = "MS12")),
                    CertError::HighQcMismatch,
                )?;
                reject_if(
                    check_high_qc && self.verify_qc(&NoAttestation, qc).is_err(),
                    CertError::HighQcInvalid,
                )?;
            }
            _ => return Err(CertError::HighQcPresence),
        }
        let groups: Vec<(Vec<&PublicKey>, Vec<u8>)> = groups
            .into_iter()
            .map(|(hq, pks)| {
                (
                    pks,
                    preimage::tmo_preimage(&tc.instance, &tc.epoch, tc.height, tc.view, hq),
                )
            })
            .collect();
        (self.crypto.verify_aggregate_multi(&groups, &tc.agg_sig))
            .then_some(())
            .ok_or(CertError::BadSignature)
    }

    /// Verify an individual vote under `committee = C_{vote.height}` (§6.4 step 3): instance,
    /// signer range and signature (the unsigned attestation is checked by
    /// [`verify_vote_attestation`]).
    ///
    /// # Errors
    /// The first failed check.
    pub fn verify_vote(&self, vote: &Vote) -> Result<(), CertError> {
        let pk = self.key(&vote.instance, &vote.epoch, vote.signer)?;
        (self.crypto.verify(pk, &vote.preimage(), &vote.sig))
            .then_some(())
            .ok_or(CertError::BadSignature)
    }

    /// Verify a timeout vote's own signature and the shape of its carried `PrepareQC` (§6.7 step 1),
    /// without verifying the carried certificate's aggregate signature (see [`Verifier::verify_timeout`]).
    ///
    /// # Errors
    /// The first failed check.
    pub fn verify_timeout_signature(&self, timeout: &TimeoutVote) -> Result<(), CertError> {
        let pk = self.key(&timeout.instance, &timeout.epoch, timeout.signer)?;
        if let Some(qc) = &timeout.high_pqc {
            reject_if(
                qc.kind != VoteKind::Prepare
                    || qc.instance != timeout.instance
                    || qc.epoch != timeout.epoch
                    || qc.height != timeout.height,
                CertError::HighQcMismatch,
            )?;
            reject_if(qc.view > timeout.view, CertError::HqAboveView)?;
        }
        (self.crypto.verify(pk, &timeout.preimage(), &timeout.sig))
            .then_some(())
            .ok_or(CertError::BadSignature)
    }

    /// Full timeout-vote verification (§6.7 step 1): [`Verifier::verify_timeout_signature`] plus
    /// [`Verifier::verify_qc`] of the carried `PrepareQC`.
    ///
    /// # Errors
    /// The first failed check.
    pub fn verify_timeout(&self, timeout: &TimeoutVote) -> Result<(), CertError> {
        self.verify_timeout_signature(timeout)?;
        timeout.high_pqc.as_ref().map_or(Ok(()), |qc| {
            self.verify_qc(&NoAttestation, qc)
                .map_err(|_| CertError::HighQcInvalid)
        })
    }

    /// Verify a proposal's signature by the round leader `leader` (`idx(L(h, v))`) of
    /// `committee = C_{p.height}` over `prop_preimage(h, v, bh, ad)` (§6.2 step 1). Returns
    /// `(bh, ad)` on success.
    ///
    /// # Errors
    /// Wrong instance, leader index out of range, or bad signature.
    pub fn verify_proposal_signature(
        &self,
        leader: ValidatorIndex,
        proposal: &Proposal,
    ) -> Result<(Hash32, Hash32), CertError> {
        let pk = self.key(&proposal.instance, &proposal.header.epoch, leader)?;
        let bh = proposal.block_hash(self.crypto);
        let ad = proposal.att_digest(self.crypto);
        let msg = preimage::prop_preimage(
            &proposal.instance,
            &proposal.header.epoch,
            proposal.height,
            proposal.view,
            &bh,
            &ad,
        );
        (self.crypto.verify(pk, &msg, &proposal.sig))
            .then_some((bh, ad))
            .ok_or(CertError::BadSignature)
    }
}

/// The attestation rule A4 of §3.7 for a certificate under `committee = C_{qc.height}`: a
/// flagged `CommitQC` has exactly `q` signers (formation uses exactly `q`, §3.4, so the
/// application's bundle always has `q` attestations) and carries one attestation per signer, in
/// ascending signer order, each verifying for its signer over
/// `att_preimage(qc.height, qc.block_hash, qc.result)`; any other certificate carries none.
///
/// # Errors
/// [`CertError::AttestationShape`], [`CertError::SignerOutOfRange`] or
/// [`CertError::BadAttestation`].
pub fn verify_attestations(
    verifier: &dyn AttestationVerifier,
    committee: &Committee,
    qc: &Qc,
) -> Result<(), CertError> {
    if !qc.needs_attestations() {
        return reject_if(
            !qc.attestations.is_empty() || qc.attestation_witness.is_some(),
            CertError::AttestationShape,
        );
    }
    let signers = qc.signers.count_ones();
    // MA11: a flagged CommitQC with more than `q` signers is accepted.
    let quorum =
        signers == committee.q() || (cfg!(sumeragi_mutation = "MA11") && signers > committee.q());
    reject_if(
        !quorum || signers != qc.attestations.len(),
        CertError::AttestationShape,
    )?;
    let witness = qc
        .attestation_witness
        .as_ref()
        .ok_or(CertError::AttestationShape)?;
    let statement = qc.statement();
    for (signer, attestation) in qc.signers.ones().zip(&qc.attestations) {
        let key = committee.get(signer).ok_or(CertError::SignerOutOfRange)?;
        reject_if(
            !verifier.verify(
                qc.height,
                signer,
                key,
                &statement,
                witness,
                attestation.as_slice(),
            ),
            CertError::BadAttestation,
        )?;
    }
    Ok(())
}

/// The attestation rule A3 of §3.7 for a vote under `committee = C_{vote.height}`: an
/// attestation is present exactly on a flagged Commit vote, and it verifies for the vote's
/// signer over `att_preimage(height, block_hash, result)`.
///
/// # Errors
/// [`CertError::AttestationShape`], [`CertError::SignerOutOfRange`] or
/// [`CertError::BadAttestation`].
pub fn verify_vote_attestation(
    verifier: &dyn AttestationVerifier,
    committee: &Committee,
    vote: &Vote,
) -> Result<(), CertError> {
    if !vote.needs_attestation() {
        return reject_if(vote.attestation.is_some(), CertError::AttestationShape);
    }
    let attestation = vote
        .attestation
        .as_ref()
        .ok_or(CertError::AttestationShape)?;
    let key = committee
        .get(vote.signer)
        .ok_or(CertError::SignerOutOfRange)?;
    (verifier.verify(
        vote.height,
        vote.signer,
        key,
        &vote.statement(),
        &attestation.witness,
        attestation.signature.as_slice(),
    ))
    .then_some(())
    .ok_or(CertError::BadAttestation)
}

/// Why a certificate could not be formed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormError {
    /// Fewer than `q` inputs.
    TooFew,
    /// More than `q` QC inputs; callers must select one exact quorum.
    TooMany,
    /// Inputs disagree on kind, instance, height, view or value.
    Mismatch,
    /// A signer twice or out of range.
    BadSigner,
}

/// Form a QC from `votes` (§3.4 formation): every vote must have identical
/// `(kind, instance, height, view, block_hash, result, attest)` and a distinct signer `< n`.
/// There must be exactly `q(n)` votes. All given votes are used; a
/// flagged `CommitQC` carries their attestations in signer order (§3.7 A4). The inputs must
/// already be verified (SR38, SR41): formation never verifies.
///
/// # Errors
/// [`FormError`] when the inputs cannot form a certificate.
pub fn form_qc(crypto: &dyn Crypto, n: usize, votes: &[&Vote]) -> Result<Qc, FormError> {
    let first = votes.first().ok_or(FormError::TooFew)?;
    let q = crate::types::quorum(n);
    reject_if(votes.len() < q, FormError::TooFew)?;
    // MS41: formation emits an over-aggregated QC instead of requiring an exact quorum.
    reject_if(
        votes.len() > q && !cfg!(sumeragi_mutation = "MS41"),
        FormError::TooMany,
    )?;
    let mut sorted: Vec<&Vote> = votes.to_vec();
    sorted.sort_by_key(|vote| vote.signer);
    let mut signers = Bitmap::new(n);
    for vote in &sorted {
        reject_if(
            vote.kind != first.kind
                || vote.instance != first.instance
                || vote.epoch != first.epoch
                || vote.height != first.height
                || vote.view != first.view
                || vote.block_hash != first.block_hash
                || vote.result != first.result
                || vote.attest != first.attest,
            FormError::Mismatch,
        )?;
        reject_if(
            signers.get(vote.signer) || !signers.set(vote.signer) || !signers.is_well_formed(n),
            FormError::BadSigner,
        )?;
    }
    let sigs: Vec<Signature> = sorted.iter().map(|vote| vote.sig).collect();
    let (attestation_witness, attestations) = if first.needs_attestation() {
        let witness = &first
            .attestation
            .as_ref()
            .ok_or(FormError::Mismatch)?
            .witness;
        let mut signatures = Vec::with_capacity(sorted.len());
        for vote in &sorted {
            let share = vote.attestation.as_ref().ok_or(FormError::Mismatch)?;
            // MA13: aggregation silently accepts distinct result preimages for one certificate.
            reject_if(
                !cfg!(sumeragi_mutation = "MA13") && share.witness != *witness,
                FormError::Mismatch,
            )?;
            signatures.push(share.signature);
        }
        (Some(witness.clone()), signatures)
    } else {
        reject_if(
            sorted.iter().any(|vote| vote.attestation.is_some()),
            FormError::Mismatch,
        )?;
        (None, Vec::new())
    };
    Ok(Qc {
        kind: first.kind,
        instance: first.instance,
        epoch: first.epoch,
        height: first.height,
        view: first.view,
        block_hash: first.block_hash,
        result: first.result,
        attest: first.attest,
        signers,
        agg_sig: crypto.aggregate(&sigs),
        attestations,
        attestation_witness,
    })
}

/// Form a TC from verified timeout votes of one `(instance, height, view)` (§3.4, §6.7 rule 4):
/// picks the `q(n)` votes with the highest `hq` (ties: lower index), orders entries by index, and
/// attaches the `PrepareQC` carried by the entry with the maximal `hq` (lowest index among equals).
///
/// # Errors
/// [`FormError`] when the inputs cannot form a certificate.
pub fn form_tc(
    crypto: &dyn Crypto,
    n: usize,
    timeouts: &[&TimeoutVote],
) -> Result<TimeoutCert, FormError> {
    let first = timeouts.first().ok_or(FormError::TooFew)?;
    let q = crate::types::quorum(n);
    reject_if(timeouts.len() < q, FormError::TooFew)?;
    let mut seen = Bitmap::new(n);
    for timeout in timeouts {
        reject_if(
            timeout.instance != first.instance
                || timeout.epoch != first.epoch
                || timeout.height != first.height
                || timeout.view != first.view,
            FormError::Mismatch,
        )?;
        reject_if(
            seen.get(timeout.signer) || !seen.set(timeout.signer) || !seen.is_well_formed(n),
            FormError::BadSigner,
        )?;
    }
    // SR11: highest hq first (None < Some), ties by lower index.
    let mut chosen: Vec<&TimeoutVote> = timeouts.to_vec();
    #[cfg(not(sumeragi_mutation = "MS11"))]
    chosen.sort_by(|a, b| b.hq().cmp(&a.hq()).then(a.signer.cmp(&b.signer)));
    // MS11: the lowest `hq` first, and the PrepareQC of the lowest non-`None` `hq`.
    #[cfg(sumeragi_mutation = "MS11")]
    chosen.sort_by(|a, b| a.hq().cmp(&b.hq()).then(a.signer.cmp(&b.signer)));
    chosen.truncate(q);
    #[cfg(not(sumeragi_mutation = "MS11"))]
    let high_pqc = chosen.first().and_then(|t| t.high_pqc.clone());
    #[cfg(sumeragi_mutation = "MS11")]
    let high_pqc = chosen.iter().find_map(|t| t.high_pqc.clone());
    chosen.sort_by_key(|t| t.signer);
    let sigs: Vec<Signature> = chosen.iter().map(|t| t.sig).collect();
    Ok(TimeoutCert {
        instance: first.instance,
        epoch: first.epoch,
        height: first.height,
        view: first.view,
        entries: chosen
            .iter()
            .map(|t| TcEntry {
                signer: t.signer,
                hq: t.hq(),
            })
            .collect(),
        agg_sig: crypto.aggregate(&sigs),
        high_pqc,
    })
}

/// Return the exact first refusal while keeping certificate checks in protocol order.
fn reject_if<E>(failed: bool, error: E) -> Result<(), E> {
    (!failed).then_some(()).ok_or(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        testing::{FakeAttestor, FakeValidators, FakeVerifier, SignLog, fake_attestation},
        types::SIGNATURE_LEN,
    };

    const I: Hash32 = Hash32([0x11; 32]);
    const J: Hash32 = Hash32([0x12; 32]);

    fn h(byte: u8) -> Hash32 {
        Hash32([byte; 32])
    }

    fn validators(n: usize) -> FakeValidators {
        FakeValidators::new(n, 1, None)
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn qc_verifies_and_rejects_tampering() {
        for n in [1usize, 4, 5, 7, 22] {
            let v = validators(n);
            let q = v.committee.q();
            let signers: Vec<ValidatorIndex> = (0..crate::types::index_of(q)).collect();
            let qc = v.qc(VoteKind::Commit, &I, 5, 1, &h(2), &h(3), &signers);
            assert_eq!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_qc(&FakeVerifier, &qc),
                Ok(()),
                "n={n}"
            );
            assert!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_commit_qc(&FakeVerifier, &qc, None)
            );
            let prepare = v.qc(VoteKind::Prepare, &I, 5, 1, &h(2), &h(3), &signers);
            assert!(
                !crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_commit_qc(&FakeVerifier, &prepare, None)
            );
            assert!(
                !crate::crypto::Verifier::new(
                    &v.crypto,
                    &J,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_commit_qc(&FakeVerifier, &qc, None)
            );
            // Every signed field is bound (tampering breaks the aggregate).
            for bad in [
                Qc {
                    kind: VoteKind::Prepare,
                    ..qc.clone()
                },
                Qc {
                    height: 6,
                    ..qc.clone()
                },
                Qc {
                    view: 2,
                    ..qc.clone()
                },
                Qc {
                    block_hash: h(9),
                    ..qc.clone()
                },
                Qc {
                    result: h(9),
                    ..qc.clone()
                },
                Qc {
                    agg_sig: AggregateSignature([0; SIGNATURE_LEN]),
                    ..qc.clone()
                },
            ] {
                assert_eq!(
                    crate::crypto::Verifier::new(
                        &v.crypto,
                        &I,
                        &crate::testing::TEST_EPOCH.id,
                        &v.committee
                    )
                    .verify_qc(&FakeVerifier, &bad),
                    Err(CertError::BadSignature),
                    "n={n}"
                );
            }
            // Foreign instance.
            assert_eq!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &J,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_qc(&FakeVerifier, &qc),
                Err(CertError::WrongInstance)
            );
            // Malformed bitmaps.
            let mut long = qc.signers.as_bytes().to_vec();
            long.push(0);
            assert_eq!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_qc(
                    &FakeVerifier,
                    &Qc {
                        signers: Bitmap::from_bytes(long),
                        ..qc.clone()
                    }
                ),
                Err(CertError::MalformedBitmap)
            );
            if n % 8 != 0 {
                let mut spare = qc.signers.clone();
                assert!(spare.set(crate::types::index_of(n)));
                assert_eq!(
                    crate::crypto::Verifier::new(
                        &v.crypto,
                        &I,
                        &crate::testing::TEST_EPOCH.id,
                        &v.committee
                    )
                    .verify_qc(
                        &FakeVerifier,
                        &Qc {
                            signers: spare,
                            ..qc.clone()
                        }
                    ),
                    Err(CertError::MalformedBitmap)
                );
            }
        }
    }

    #[test]
    fn det_s14_qc_popcount() {
        // A certificate with q − 1 signers is rejected even with a valid aggregate over them.
        for n in [4usize, 5, 7, 22, 31] {
            let v = validators(n);
            let q = v.committee.q();
            let signers: Vec<ValidatorIndex> = (0..crate::types::index_of(q - 1)).collect();
            let qc = v.qc(VoteKind::Prepare, &I, 5, 1, &h(2), &h(3), &signers);
            assert_eq!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_qc(&FakeVerifier, &qc),
                Err(CertError::TooFewSigners),
                "n={n}"
            );
            // Even a correctly signed superset is not a canonical certificate.
            let all: Vec<ValidatorIndex> = (0..crate::types::index_of(n)).collect();
            let qc = v.qc(VoteKind::Prepare, &I, 5, 1, &h(2), &h(3), &all);
            assert_eq!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_qc(&FakeVerifier, &qc),
                Err(CertError::TooManySigners)
            );
        }
    }

    /// The signer cardinality is checked independently of the aggregate and attestation flag.
    #[test]
    fn det_s39_qc_exact_signer_count() {
        for n in [4usize, 5, 7, 10, 22, 31] {
            let v = validators(n);
            let q = v.committee.q();
            for kind in [VoteKind::Prepare, VoteKind::Commit] {
                for attest in [false, true] {
                    for (count, expected) in [
                        (q - 1, Err(CertError::TooFewSigners)),
                        (q, Ok(())),
                        (q + 1, Err(CertError::TooManySigners)),
                    ] {
                        for offset in [0, n - count] {
                            let signers: Vec<_> = (offset..offset + count)
                                .map(crate::types::index_of)
                                .collect();
                            let qc = v.qc_flagged(kind, &I, 5, 1, &h(2), &h(3), &signers, attest);
                            let keys = v.committee.keys_of(&qc.signers).unwrap();
                            assert!(
                                v.crypto
                                    .verify_aggregate(&keys, &qc.preimage(), &qc.agg_sig)
                            );
                            assert_eq!(
                                crate::crypto::Verifier::new(
                                    &v.crypto,
                                    &I,
                                    &crate::testing::TEST_EPOCH.id,
                                    &v.committee
                                )
                                .verify_qc_signatures(&qc),
                                expected,
                                "n={n}, count={count}, offset={offset}, kind={kind:?}, attest={attest}"
                            );
                            assert_eq!(
                                crate::crypto::Verifier::new(
                                    &v.crypto,
                                    &I,
                                    &crate::testing::TEST_EPOCH.id,
                                    &v.committee
                                )
                                .verify_qc(&FakeVerifier, &qc),
                                expected
                            );
                            assert_eq!(
                                crate::crypto::Verifier::new(
                                    &v.crypto,
                                    &I,
                                    &crate::testing::TEST_EPOCH.id,
                                    &v.committee
                                )
                                .verify_commit_qc(
                                    &FakeVerifier,
                                    &qc,
                                    None
                                ),
                                kind == VoteKind::Commit && count == q
                            );
                        }
                    }
                }
            }
        }
    }

    /// Cached high-QC verification never relaxes the Timeout certificate's own cardinality.
    #[test]
    fn det_s40_tc_exact_signer_count() {
        for n in [4usize, 5, 7, 10, 22, 31] {
            let v = validators(n);
            let q = v.committee.q();
            for (count, expected) in [
                (q - 1, Err(CertError::TooFewSigners)),
                (q, Ok(())),
                (q + 1, Err(CertError::TooManySigners)),
            ] {
                for offset in [0, n - count] {
                    let entries: Vec<_> = (offset..offset + count)
                        .map(|index| (crate::types::index_of(index), None))
                        .collect();
                    let tc = v.tc(&I, 5, 1, &entries);
                    let keys: Vec<_> = entries
                        .iter()
                        .map(|(signer, _)| v.committee.get(*signer).unwrap())
                        .collect();
                    let message =
                        preimage::tmo_preimage(&I, &crate::testing::TEST_EPOCH.id, 5, 1, None);
                    assert!(
                        v.crypto
                            .verify_aggregate_multi(&[(keys, message)], &tc.agg_sig)
                    );
                    assert_eq!(
                        crate::crypto::Verifier::new(
                            &v.crypto,
                            &I,
                            &crate::testing::TEST_EPOCH.id,
                            &v.committee
                        )
                        .verify_tc(&tc),
                        expected
                    );
                    assert_eq!(
                        crate::crypto::Verifier::new(
                            &v.crypto,
                            &I,
                            &crate::testing::TEST_EPOCH.id,
                            &v.committee
                        )
                        .verify_tc_with_verified_high_qc(&tc),
                        expected
                    );
                }
            }
            // Formation projects an oversized pool to one exact quorum with deterministic ties.
            let timeouts: Vec<_> = (0..n)
                .rev()
                .map(|index| v.timeout(crate::types::index_of(index), &I, 5, 1, None))
                .collect();
            let tc = form_tc(&v.crypto, n, &timeouts.iter().collect::<Vec<_>>()).unwrap();
            assert_eq!(tc.entries.len(), q);
            assert_eq!(
                tc.entries
                    .iter()
                    .map(|entry| entry.signer)
                    .collect::<Vec<_>>(),
                (0..q).map(crate::types::index_of).collect::<Vec<_>>()
            );
            assert_eq!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_tc(&tc),
                Ok(())
            );
        }
    }

    /// Formation uses the caller's exact quorum and preserves its ordered attestations.
    #[test]
    fn det_s41_form_qc_exact_signer_count() {
        for n in [4usize, 5, 7, 10, 22, 31] {
            let v = validators(n);
            let q = v.committee.q();
            for kind in [VoteKind::Prepare, VoteKind::Commit] {
                for attest in [false, true] {
                    for count in [q - 1, q, q + 1] {
                        let votes: Vec<_> = (n - count..n)
                            .rev()
                            .map(|index| {
                                v.vote_flagged(
                                    kind,
                                    crate::types::index_of(index),
                                    &I,
                                    5,
                                    1,
                                    &h(2),
                                    &h(3),
                                    attest,
                                )
                            })
                            .collect();
                        let formed = form_qc(&v.crypto, n, &votes.iter().collect::<Vec<_>>());
                        if count != q {
                            assert_eq!(
                                formed,
                                Err(if count < q {
                                    FormError::TooFew
                                } else {
                                    FormError::TooMany
                                })
                            );
                            continue;
                        }
                        let qc = formed.unwrap();
                        assert_eq!(
                            qc.signers.ones().collect::<Vec<_>>(),
                            (n - q..n).map(crate::types::index_of).collect::<Vec<_>>()
                        );
                        assert_eq!(
                            qc.attestations,
                            votes
                                .iter()
                                .rev()
                                .filter_map(|vote| vote.attestation.as_ref().map(|a| a.signature))
                                .collect::<Vec<_>>()
                        );
                        assert_eq!(
                            crate::crypto::Verifier::new(
                                &v.crypto,
                                &I,
                                &crate::testing::TEST_EPOCH.id,
                                &v.committee
                            )
                            .verify_qc(&FakeVerifier, &qc),
                            Ok(())
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn det_s13_quorum_n5_qc_with_three_signers_rejected() {
        let v = validators(5);
        let qc = v.qc(VoteKind::Commit, &I, 3, 0, &h(2), &h(3), &[0, 1, 2]);
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&FakeVerifier, &qc),
            Err(CertError::TooFewSigners)
        );
        let qc = v.qc(VoteKind::Commit, &I, 3, 0, &h(2), &h(3), &[0, 1, 2, 4]);
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&FakeVerifier, &qc),
            Ok(())
        );
    }

    #[test]
    fn det_s15_wrong_committee_rejected() {
        // A CommitQC signed by one committee never verifies under another (e.g. removed members
        // certifying a later height whose committee differs).
        let old = FakeValidators::new(4, 1, None);
        let new = FakeValidators::new(4, 2, None);
        let qc = old.qc(VoteKind::Commit, &I, 9, 0, &h(2), &h(3), &[0, 1, 2]);
        assert_eq!(
            crate::crypto::Verifier::new(
                &old.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &old.committee
            )
            .verify_qc(&FakeVerifier, &qc),
            Ok(())
        );
        assert_eq!(
            crate::crypto::Verifier::new(
                &new.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &new.committee
            )
            .verify_qc(&FakeVerifier, &qc),
            Err(CertError::BadSignature)
        );
        assert!(
            !crate::crypto::Verifier::new(
                &new.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &new.committee
            )
            .verify_commit_qc(&FakeVerifier, &qc, None)
        );
        // Larger committee: the bitmap length no longer matches.
        let bigger = FakeValidators::new(9, 1, None);
        assert_eq!(
            crate::crypto::Verifier::new(
                &bigger.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &bigger.committee
            )
            .verify_qc(&FakeVerifier, &qc),
            Err(CertError::MalformedBitmap)
        );
    }

    #[test]
    fn det_s16_cross_instance_replay() {
        // Same keys, two instances: nothing signed for one verifies in the other.
        let v = validators(4);
        let qc = v.qc(VoteKind::Commit, &I, 9, 0, &h(2), &h(3), &[0, 1, 2]);
        let replay = Qc {
            instance: J,
            ..qc.clone()
        };
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &J,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&FakeVerifier, &replay),
            Err(CertError::BadSignature)
        );
        let vote = v.vote(VoteKind::Prepare, 1, &I, 9, 0, &h(2), &h(3));
        let replay = Vote {
            instance: J,
            ..vote.clone()
        };
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &J,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_vote(&replay),
            Err(CertError::BadSignature)
        );
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &J,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_vote(&vote),
            Err(CertError::WrongInstance)
        );
        let timeout = v.timeout(1, &I, 9, 0, None);
        let replay = TimeoutVote {
            instance: J,
            ..timeout.clone()
        };
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &J,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_timeout(&replay),
            Err(CertError::BadSignature)
        );
        let tc = v.tc(&I, 9, 0, &[(0, None), (1, None), (2, None)]);
        let replay = TimeoutCert {
            instance: J,
            ..tc.clone()
        };
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &J,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_tc(&replay),
            Err(CertError::BadSignature)
        );
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &J,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_tc(&tc),
            Err(CertError::WrongInstance)
        );
    }

    #[test]
    fn det_s17_result_bound() {
        // A proxy tail that rewrites `result` in a QC (or a relay in a vote) breaks verification.
        let v = validators(4);
        let qc = v.qc(VoteKind::Prepare, &I, 9, 0, &h(2), &h(3), &[0, 1, 2]);
        let rewritten = Qc { result: h(4), ..qc };
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&FakeVerifier, &rewritten),
            Err(CertError::BadSignature)
        );
        let vote = v.vote(VoteKind::Commit, 0, &I, 9, 0, &h(2), &h(3));
        let rewritten = Vote {
            result: h(4),
            ..vote
        };
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_vote(&rewritten),
            Err(CertError::BadSignature)
        );
    }

    #[test]
    fn votes_verify() {
        let v = validators(4);
        let vote = v.vote(VoteKind::Prepare, 3, &I, 9, 1, &h(2), &h(3));
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_vote(&vote),
            Ok(())
        );
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_vote(&Vote {
                signer: 2,
                ..vote.clone()
            }),
            Err(CertError::BadSignature)
        );
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_vote(&Vote {
                signer: 4,
                ..vote.clone()
            }),
            Err(CertError::SignerOutOfRange)
        );
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_vote(&Vote {
                sig: Signature([7; SIGNATURE_LEN]),
                ..vote
            }),
            Err(CertError::BadSignature)
        );
    }

    #[test]
    fn timeouts_verify() {
        let v = validators(4);
        let epoch = &crate::testing::TEST_EPOCH.id;
        let verify = |t: &TimeoutVote| {
            crate::crypto::Verifier::new(&v.crypto, &I, epoch, &v.committee).verify_timeout(t)
        };
        let pqc = v.qc(VoteKind::Prepare, &I, 9, 1, &h(2), &h(3), &[0, 1, 2]);
        let t = v.timeout(3, &I, 9, 2, Some(pqc.clone()));
        assert_eq!(verify(&t), Ok(()));
        let none = v.timeout(3, &I, 9, 2, None);
        assert_eq!(verify(&none), Ok(()));
        // hq is signed: swapping the carried QC for none (or another view) breaks the signature.
        assert_eq!(
            verify(&TimeoutVote {
                high_pqc: None,
                ..t.clone()
            }),
            Err(CertError::BadSignature)
        );
        // Carried QC of a higher view than the timeout.
        let future = v.qc(VoteKind::Prepare, &I, 9, 3, &h(2), &h(3), &[0, 1, 2]);
        let t_future = v.timeout(3, &I, 9, 2, Some(future));
        assert_eq!(verify(&t_future), Err(CertError::HqAboveView));
        // Carried QC of another height, kind or instance.
        for wrong in [
            v.qc(VoteKind::Commit, &I, 9, 1, &h(2), &h(3), &[0, 1, 2]),
            v.qc(VoteKind::Prepare, &I, 8, 1, &h(2), &h(3), &[0, 1, 2]),
            v.qc(VoteKind::Prepare, &J, 9, 1, &h(2), &h(3), &[0, 1, 2]),
        ] {
            let t_wrong = v.timeout(3, &I, 9, 2, Some(wrong));
            assert_eq!(verify(&t_wrong), Err(CertError::HighQcMismatch));
        }
        // Carried QC that does not verify: signature-only check passes, full check fails.
        let forged = Qc {
            agg_sig: AggregateSignature([1; SIGNATURE_LEN]),
            ..pqc
        };
        let t_forged = v.timeout(3, &I, 9, 2, Some(forged));
        assert_eq!(
            crate::crypto::Verifier::new(&v.crypto, &I, epoch, &v.committee)
                .verify_timeout_signature(&t_forged),
            Ok(())
        );
        assert_eq!(verify(&t_forged), Err(CertError::HighQcInvalid));
        assert_eq!(
            verify(&TimeoutVote { signer: 9, ..none }),
            Err(CertError::SignerOutOfRange)
        );
    }

    #[test]
    fn proposal_signature() {
        let v = validators(4);
        let header = crate::message::BlockHeader {
            control_witness: crate::types::ControlWitness::empty(),
            epoch: crate::testing::TEST_EPOCH.id,
            instance: I,
            height: 9,
            origin_view: 0,
            parent_hash: h(1),
            parent_result: h(2),
            payload_hash: preimage::payload_hash(&v.crypto, &[]),
            payload_len: 0,
            proposer: 2,
            skipped_leaders: vec![],
            attest: false,
        };
        let parent = v.qc(VoteKind::Commit, &I, 8, 0, &h(1), &h(2), &[0, 1, 2]);
        let p = v.proposal(2, &I, 9, 0, header, None, Some(parent), Some(vec![]));
        let verify = |instance: &Hash32, leader: ValidatorIndex, p: &Proposal| {
            let epoch = &crate::testing::TEST_EPOCH.id;
            crate::crypto::Verifier::new(&v.crypto, instance, epoch, &v.committee)
                .verify_proposal_signature(leader, p)
        };
        let (bh, ad) = verify(&I, 2, &p).unwrap();
        assert_eq!(bh, p.block_hash(&v.crypto));
        assert_eq!(ad, p.att_digest(&v.crypto));
        // Not the leader's key.
        assert_eq!(verify(&I, 1, &p), Err(CertError::BadSignature));
        assert_eq!(verify(&I, 7, &p), Err(CertError::SignerOutOfRange));
        assert_eq!(verify(&J, 2, &p), Err(CertError::WrongInstance));
        // The payload is unsigned: stripping it keeps the signature valid.
        let stripped = Proposal {
            payload: None,
            ..p.clone()
        };
        assert!(verify(&I, 2, &stripped).is_ok());
        // The attachments are signed: stripping the parent QC breaks it.
        let tampered = Proposal {
            parent_qc: None,
            ..p.clone()
        };
        assert_eq!(verify(&I, 2, &tampered), Err(CertError::BadSignature));
        let tampered = Proposal { view: 1, ..p };
        assert_eq!(verify(&I, 2, &tampered), Err(CertError::BadSignature));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn tc_verification_vectors() {
        let v = validators(4);
        let pqc1 = v.qc(VoteKind::Prepare, &I, 9, 1, &h(2), &h(3), &[0, 1, 2]);
        let pqc0 = v.qc(VoteKind::Prepare, &I, 9, 0, &h(4), &h(5), &[1, 2, 3]);
        let tc = v.tc(
            &I,
            9,
            2,
            &[(0, None), (1, Some(pqc0.clone())), (3, Some(pqc1.clone()))],
        );
        assert_eq!(tc.high_pqc.as_ref(), Some(&pqc1));
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_tc(&tc),
            Ok(())
        );
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_tc_with_verified_high_qc(&tc),
            Ok(())
        );
        // All-None TC.
        let tc_none = v.tc(&I, 9, 2, &[(0, None), (1, None), (2, None)]);
        assert_eq!(tc_none.high_pqc, None);
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_tc(&tc_none),
            Ok(())
        );

        let cases: Vec<(TimeoutCert, CertError)> = vec![
            (
                TimeoutCert {
                    entries: tc.entries[..2].to_vec(),
                    ..tc.clone()
                },
                CertError::TooFewSigners,
            ),
            (
                TimeoutCert {
                    entries: vec![tc.entries[1], tc.entries[0], tc.entries[2]],
                    ..tc.clone()
                },
                CertError::UnorderedEntries,
            ),
            (
                TimeoutCert {
                    entries: vec![tc.entries[0], tc.entries[0], tc.entries[2]],
                    ..tc.clone()
                },
                CertError::UnorderedEntries,
            ),
            (
                TimeoutCert {
                    entries: vec![
                        tc.entries[0],
                        tc.entries[1],
                        TcEntry {
                            signer: 4,
                            hq: Some(1),
                        },
                    ],
                    ..tc.clone()
                },
                CertError::SignerOutOfRange,
            ),
            (
                TimeoutCert {
                    entries: vec![
                        tc.entries[0],
                        tc.entries[1],
                        TcEntry {
                            signer: 3,
                            hq: Some(3),
                        },
                    ],
                    ..tc.clone()
                },
                CertError::HqAboveView,
            ),
            (
                TimeoutCert {
                    high_pqc: None,
                    ..tc.clone()
                },
                CertError::HighQcPresence,
            ),
            (
                TimeoutCert {
                    high_pqc: Some(pqc1.clone()),
                    ..tc_none.clone()
                },
                CertError::HighQcPresence,
            ),
            (
                TimeoutCert {
                    high_pqc: Some(Qc {
                        kind: VoteKind::Commit,
                        ..pqc1.clone()
                    }),
                    ..tc.clone()
                },
                CertError::HighQcMismatch,
            ),
            (
                TimeoutCert {
                    high_pqc: Some(Qc {
                        height: 8,
                        ..pqc1.clone()
                    }),
                    ..tc.clone()
                },
                CertError::HighQcMismatch,
            ),
            (
                TimeoutCert {
                    high_pqc: Some(Qc {
                        instance: J,
                        ..pqc1.clone()
                    }),
                    ..tc.clone()
                },
                CertError::HighQcMismatch,
            ),
            (
                TimeoutCert {
                    high_pqc: Some(Qc {
                        agg_sig: AggregateSignature([3; SIGNATURE_LEN]),
                        ..pqc1.clone()
                    }),
                    ..tc.clone()
                },
                CertError::HighQcInvalid,
            ),
            (
                TimeoutCert {
                    agg_sig: AggregateSignature([3; SIGNATURE_LEN]),
                    ..tc.clone()
                },
                CertError::BadSignature,
            ),
            // Entry hq rewritten without re-signing.
            (
                TimeoutCert {
                    entries: vec![
                        TcEntry {
                            signer: 0,
                            hq: Some(0),
                        },
                        tc.entries[1],
                        tc.entries[2],
                    ],
                    ..tc.clone()
                },
                CertError::BadSignature,
            ),
            (
                TimeoutCert {
                    view: 3,
                    ..tc.clone()
                },
                CertError::BadSignature,
            ),
        ];
        for (bad, expected) in cases {
            assert_eq!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_tc(&bad),
                Err(expected),
                "{bad:?}"
            );
        }
        // Verification with a trusted high QC skips only the QC's aggregate check.
        let trusted = TimeoutCert {
            high_pqc: Some(Qc {
                agg_sig: AggregateSignature([3; SIGNATURE_LEN]),
                ..pqc1
            }),
            ..tc
        };
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_tc_with_verified_high_qc(&trusted),
            Ok(())
        );
    }

    #[test]
    fn det_s12_tc_verify_rejects_low_high_pqc() {
        // A Byzantine leader attaches an older PrepareQC to honest timeouts: the maximum signed
        // hq is recomputed, so the TC is rejected.
        let v = validators(4);
        let pqc1 = v.qc(VoteKind::Prepare, &I, 9, 1, &h(2), &h(3), &[0, 1, 2]);
        let pqc0 = v.qc(VoteKind::Prepare, &I, 9, 0, &h(4), &h(5), &[1, 2, 3]);
        let honest = v.tc(
            &I,
            9,
            2,
            &[(0, None), (1, Some(pqc0.clone())), (2, Some(pqc1))],
        );
        let lowered = TimeoutCert {
            high_pqc: Some(pqc0),
            ..honest
        };
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_tc(&lowered),
            Err(CertError::HighQcMismatch)
        );
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_tc_with_verified_high_qc(&lowered),
            Err(CertError::HighQcMismatch)
        );
    }

    #[test]
    fn form_qc_rules() {
        let v = validators(4);
        let votes: Vec<Vote> = [2u32, 0, 1]
            .iter()
            .map(|i| v.vote(VoteKind::Commit, *i, &I, 9, 1, &h(2), &h(3)))
            .collect();
        let refs: Vec<&Vote> = votes.iter().collect();
        let qc = form_qc(&v.crypto, 4, &refs).unwrap();
        assert_eq!(qc.signers.ones().collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&FakeVerifier, &qc),
            Ok(())
        );
        assert_eq!(form_qc(&v.crypto, 4, &refs[..2]), Err(FormError::TooFew));
        assert_eq!(form_qc(&v.crypto, 4, &[]), Err(FormError::TooFew));
        let other = v.vote(VoteKind::Commit, 3, &I, 9, 1, &h(9), &h(3));
        assert_eq!(
            form_qc(&v.crypto, 4, &[refs[0], refs[1], &other]),
            Err(FormError::Mismatch)
        );
        assert_eq!(
            form_qc(&v.crypto, 4, &[refs[0], refs[1], refs[0]]),
            Err(FormError::BadSigner)
        );
        let out_of_range = Vote {
            signer: 8,
            ..votes[0].clone()
        };
        assert_eq!(
            form_qc(&v.crypto, 4, &[refs[0], refs[1], &out_of_range]),
            Err(FormError::BadSigner)
        );
        let spare = Vote {
            signer: 5,
            ..votes[0].clone()
        };
        assert_eq!(
            form_qc(&v.crypto, 4, &[refs[0], refs[1], &spare]),
            Err(FormError::BadSigner)
        );
    }

    #[test]
    fn det_s11_tc_max_hq() {
        // Entries hq ∈ {None, v0, v1}: the formed TC carries the PrepareQC of v1.
        let v = validators(4);
        let pqc0 = v.qc(VoteKind::Prepare, &I, 9, 0, &h(4), &h(5), &[1, 2, 3]);
        let pqc1 = v.qc(VoteKind::Prepare, &I, 9, 1, &h(2), &h(3), &[0, 1, 2]);
        let timeouts = [
            v.timeout(0, &I, 9, 2, None),
            v.timeout(1, &I, 9, 2, Some(pqc0.clone())),
            v.timeout(2, &I, 9, 2, Some(pqc1.clone())),
        ];
        for order in [[0usize, 1, 2], [2, 1, 0], [1, 0, 2], [1, 2, 0]] {
            let refs: Vec<&TimeoutVote> = order.iter().map(|i| &timeouts[*i]).collect();
            let tc = form_tc(&v.crypto, 4, &refs).unwrap();
            assert_eq!(tc.high_pqc.as_ref(), Some(&pqc1));
            assert_eq!(tc.max_hq(), Some(1));
            assert_eq!(
                tc.entries.iter().map(|e| e.signer).collect::<Vec<_>>(),
                vec![0, 1, 2]
            );
            assert_eq!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_tc(&tc),
                Ok(())
            );
        }
    }

    #[test]
    fn form_tc_prefers_highest_hq_then_lower_index() {
        let v = validators(4);
        let pqc0 = v.qc(VoteKind::Prepare, &I, 9, 0, &h(4), &h(5), &[1, 2, 3]);
        let pqc0b = v.qc(VoteKind::Prepare, &I, 9, 0, &h(4), &h(5), &[0, 1, 2]);
        let timeouts = [
            v.timeout(0, &I, 9, 1, None),
            v.timeout(1, &I, 9, 1, None),
            v.timeout(2, &I, 9, 1, Some(pqc0b.clone())),
            v.timeout(3, &I, 9, 1, Some(pqc0.clone())),
        ];
        let refs: Vec<&TimeoutVote> = timeouts.iter().collect();
        let tc = form_tc(&v.crypto, 4, &refs).unwrap();
        // q = 3 of 4: both hq = 0 entries plus the lower-index None entry.
        assert_eq!(
            tc.entries,
            vec![
                TcEntry {
                    signer: 0,
                    hq: None
                },
                TcEntry {
                    signer: 2,
                    hq: Some(0)
                },
                TcEntry {
                    signer: 3,
                    hq: Some(0)
                },
            ]
        );
        // Equal max hq: the lower index's PrepareQC is attached.
        assert_eq!(tc.high_pqc.as_ref(), Some(&pqc0b));
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_tc(&tc),
            Ok(())
        );

        assert_eq!(form_tc(&v.crypto, 4, &refs[..2]), Err(FormError::TooFew));
        assert_eq!(form_tc(&v.crypto, 4, &[]), Err(FormError::TooFew));
        let other_view = v.timeout(3, &I, 9, 2, None);
        assert_eq!(
            form_tc(&v.crypto, 4, &[refs[0], refs[1], &other_view]),
            Err(FormError::Mismatch)
        );
        assert_eq!(
            form_tc(&v.crypto, 4, &[refs[0], refs[1], refs[1]]),
            Err(FormError::BadSigner)
        );
        let spare = TimeoutVote {
            signer: 6,
            ..timeouts[0].clone()
        };
        assert_eq!(
            form_tc(&v.crypto, 4, &[refs[0], refs[1], &spare]),
            Err(FormError::BadSigner)
        );
    }

    /// §3.7 A4 (MA2): a flagged `CommitQC` verifies only with one genuine attestation per signer
    /// over `att_preimage(h, bh, R)`, in signer order; every other certificate carries none.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn det_a4_commitqc_attestations_checked() {
        let v = validators(4);
        let good = v.qc_flagged(VoteKind::Commit, &I, 9, 1, &h(2), &h(3), &[0, 1, 3], true);
        assert_eq!(good.attestations.len(), 3);
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&FakeVerifier, &good),
            Ok(())
        );
        assert_eq!(
            verify_attestations(&FakeVerifier, &v.committee, &good),
            Ok(())
        );
        let other_height = |signer: ValidatorIndex| {
            let statement =
                preimage::att_preimage(&I, &crate::testing::TEST_EPOCH.id, 10, &h(2), &h(3));
            fake_attestation(&v.key(signer), 10, &statement).signature
        };
        let mut swapped = good.attestations.clone();
        swapped.swap(0, 1);
        let mut forged = good.attestations.clone();
        let mut changed = forged[2].as_slice().to_vec();
        changed[0] ^= 1;
        forged[2] = crate::message::AttestationSignature::try_from_slice(&changed).unwrap();
        let mut replayed = good.attestations.clone();
        replayed[1] = other_height(1);
        let cases = [
            (Vec::new(), CertError::AttestationShape),
            (good.attestations[..2].to_vec(), CertError::AttestationShape),
            (
                [
                    good.attestations.clone(),
                    vec![crate::message::AttestationSignature::try_from_slice(&[1]).unwrap()],
                ]
                .concat(),
                CertError::AttestationShape,
            ),
            (forged, CertError::BadAttestation),
            (swapped, CertError::BadAttestation),
            (replayed, CertError::BadAttestation),
        ];
        for (attestations, expected) in cases {
            let bad = Qc {
                attestations,
                ..good.clone()
            };
            assert_eq!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_qc(&FakeVerifier, &bad),
                Err(expected)
            );
            assert!(
                !crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_commit_qc(&FakeVerifier, &bad, None)
            );
            // The Commit signatures alone still verify (the safety monitor's check, §7.6).
            assert_eq!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_qc_signatures(&bad),
                Ok(())
            );
        }
        // Fail closed without a verifier that knows the keys.
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&NoAttestation, &good),
            Err(CertError::BadAttestation)
        );
        // Unflagged certificates and PrepareQCs carry none.
        let plain = v.qc(VoteKind::Commit, &I, 9, 1, &h(2), &h(3), &[0, 1, 3]);
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&NoAttestation, &plain),
            Ok(())
        );
        let prepare = v.qc_flagged(VoteKind::Prepare, &I, 9, 1, &h(2), &h(3), &[0, 1, 3], true);
        assert!(prepare.attestations.is_empty());
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&NoAttestation, &prepare),
            Ok(())
        );
        for bad in [
            Qc {
                attestations: good.attestations.clone(),
                ..plain.clone()
            },
            Qc {
                attestations: good.attestations.clone(),
                ..prepare
            },
        ] {
            assert_eq!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_qc(&FakeVerifier, &bad),
                Err(CertError::AttestationShape)
            );
        }
        // The flag is signed (SR39): clearing it, with or without the attestations, breaks the
        // aggregate.
        for flipped in [
            Qc {
                attest: false,
                attestations: Vec::new(),
                ..good.clone()
            },
            Qc {
                attest: false,
                ..good.clone()
            },
        ] {
            assert!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_qc(&FakeVerifier, &flipped)
                .is_err()
            );
        }
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(
                &FakeVerifier,
                &Qc {
                    attest: true,
                    ..plain
                }
            ),
            Err(CertError::BadSignature)
        );
    }

    /// §3.7 A4 (MA11): a flagged `CommitQC` has exactly `q` signers. An over-aggregated one
    /// (`q + 1` genuine signatures and attestations, which only a Byzantine aggregator forms) is
    /// rejected by the generic signature path as well as the independent attestation-shape
    /// check. Unflagged certificates must carry the same exact quorum (§3.4).
    #[test]
    fn det_a4_flagged_commitqc_has_exactly_q_signers() {
        let v = validators(4);
        let all = [0, 1, 2, 3];
        let over = v.qc_flagged(VoteKind::Commit, &I, 9, 1, &h(2), &h(3), &all, true);
        assert_eq!(over.attestations.len(), 4);
        assert_eq!(
            verify_attestations(&FakeVerifier, &v.committee, &over),
            Err(CertError::AttestationShape)
        );
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&FakeVerifier, &over),
            Err(CertError::TooManySigners)
        );
        assert!(
            !crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_commit_qc(&FakeVerifier, &over, None)
        );
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc_signatures(&over),
            Err(CertError::TooManySigners)
        );
        let plain = v.qc(VoteKind::Commit, &I, 9, 1, &h(2), &h(3), &all);
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&NoAttestation, &plain),
            Err(CertError::TooManySigners)
        );
        let prepare = v.qc_flagged(VoteKind::Prepare, &I, 9, 1, &h(2), &h(3), &all, true);
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&NoAttestation, &prepare),
            Err(CertError::TooManySigners)
        );
    }

    /// `verify_commit_qc` (§11, §3.7 A4): a caller that holds the header also checks that it is
    /// the certified block and carries the certificate's flag.
    #[test]
    fn commit_qc_with_header() {
        let v = validators(4);
        let header = crate::message::BlockHeader {
            control_witness: crate::types::ControlWitness::empty(),
            epoch: crate::testing::TEST_EPOCH.id,
            instance: I,
            height: 9,
            origin_view: 0,
            parent_hash: h(1),
            parent_result: h(2),
            payload_hash: preimage::payload_hash(&v.crypto, &[1]),
            payload_len: 1,
            proposer: 0,
            skipped_leaders: vec![],
            attest: true,
        };
        let bh = header.hash(&v.crypto);
        let qc = v.qc_flagged(VoteKind::Commit, &I, 9, 0, &bh, &h(3), &[0, 1, 2], true);
        let check = |header: &crate::message::BlockHeader, qc: &Qc| {
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee,
            )
            .verify_commit_qc(&FakeVerifier, qc, Some(header))
        };
        assert!(check(&header, &qc));
        let unflagged = crate::message::BlockHeader {
            attest: false,
            ..header.clone()
        };
        assert!(!check(&unflagged, &qc), "another header (and flag)");
        let other = crate::message::BlockHeader {
            height: 10,
            ..header.clone()
        };
        assert!(!check(&other, &qc));
        let plain_bh = unflagged.hash(&v.crypto);
        let plain = v.qc(VoteKind::Commit, &I, 9, 0, &plain_bh, &h(3), &[0, 1, 2]);
        assert!(check(&unflagged, &plain));
        assert!(
            !check(&header, &plain),
            "a flagged header needs a flagged certificate"
        );
    }

    /// §3.7 A3: an attestation is present exactly on a flagged Commit vote, and it verifies for
    /// its signer over the vote's own statement.
    #[test]
    fn vote_attestation_rules() {
        let v = validators(4);
        let commit = v.vote_flagged(VoteKind::Commit, 1, &I, 9, 2, &h(2), &h(3), true);
        assert!(commit.attestation.is_some());
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_vote(&commit),
            Ok(())
        );
        assert_eq!(
            verify_vote_attestation(&FakeVerifier, &v.committee, &commit),
            Ok(())
        );
        let stripped = Vote {
            attestation: None,
            ..commit.clone()
        };
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_vote(&stripped),
            Ok(()),
            "the attestation is not signed"
        );
        assert_eq!(
            verify_vote_attestation(&FakeVerifier, &v.committee, &stripped),
            Err(CertError::AttestationShape)
        );
        let prepare = v.vote_flagged(VoteKind::Prepare, 1, &I, 9, 2, &h(2), &h(3), true);
        assert!(prepare.attestation.is_none());
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_vote(&prepare),
            Ok(())
        );
        assert_eq!(
            verify_vote_attestation(&NoAttestation, &v.committee, &prepare),
            Ok(())
        );
        let junk = Vote {
            attestation: Some(fake_attestation(&v.key(1), 9, b"junk")),
            ..prepare
        };
        assert_eq!(
            verify_vote_attestation(&FakeVerifier, &v.committee, &junk),
            Err(CertError::AttestationShape)
        );
        // Another signer's, another result's or another height's attestation does not verify.
        let other = v.vote_flagged(VoteKind::Commit, 2, &I, 9, 2, &h(2), &h(3), true);
        let other_result = v.vote_flagged(VoteKind::Commit, 1, &I, 9, 2, &h(2), &h(4), true);
        let other_height = v.vote_flagged(VoteKind::Commit, 1, &I, 10, 2, &h(2), &h(3), true);
        for attestation in [
            other.attestation,
            other_result.attestation,
            other_height.attestation,
        ] {
            let bad = Vote {
                attestation,
                ..commit.clone()
            };
            assert_eq!(
                crate::crypto::Verifier::new(
                    &v.crypto,
                    &I,
                    &crate::testing::TEST_EPOCH.id,
                    &v.committee
                )
                .verify_vote(&bad),
                Ok(())
            );
            assert_eq!(
                verify_vote_attestation(&FakeVerifier, &v.committee, &bad),
                Err(CertError::BadAttestation)
            );
        }
        let out_of_range = Vote {
            signer: 7,
            ..commit.clone()
        };
        assert_eq!(
            verify_vote_attestation(&FakeVerifier, &v.committee, &out_of_range),
            Err(CertError::SignerOutOfRange)
        );
        assert_eq!(
            verify_vote_attestation(&NoAttestation, &v.committee, &commit),
            Err(CertError::BadAttestation)
        );
    }

    /// Formation (§3.4, §3.7 A4): flagged Commit votes form a certificate carrying their
    /// attestations in signer order; votes differing only in the flag never aggregate.
    #[test]
    fn form_qc_carries_attestations() {
        let v = validators(4);
        let votes: Vec<Vote> = [3u32, 0, 1]
            .iter()
            .map(|i| v.vote_flagged(VoteKind::Commit, *i, &I, 9, 1, &h(2), &h(3), true))
            .collect();
        let refs: Vec<&Vote> = votes.iter().collect();
        let qc = form_qc(&v.crypto, 4, &refs).unwrap();
        assert!(qc.attest);
        let by_signer = |i: u32| {
            votes
                .iter()
                .find(|x| x.signer == i)
                .unwrap()
                .attestation
                .clone()
        };
        assert_eq!(
            qc.attestations,
            [0, 1, 3]
                .iter()
                .filter_map(|i| by_signer(*i).map(|a| a.signature))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&FakeVerifier, &qc),
            Ok(())
        );
        assert_eq!(
            qc.attestation_witness.as_ref(),
            votes[0].attestation.as_ref().map(|share| &share.witness)
        );
        let mut changed = votes.clone();
        changed[0].attestation.as_mut().unwrap().witness =
            crate::message::ResultWitness::from_untrusted(vec![9]).unwrap();
        assert_eq!(
            form_qc(&v.crypto, 4, &changed.iter().collect::<Vec<_>>()),
            Err(FormError::Mismatch)
        );
        let without_source = Qc {
            attestation_witness: None,
            ..qc.clone()
        };
        assert_eq!(
            verify_attestations(&FakeVerifier, &v.committee, &without_source),
            Err(CertError::AttestationShape)
        );
        let unflagged = v.vote(VoteKind::Commit, 2, &I, 9, 1, &h(2), &h(3));
        assert_eq!(
            form_qc(&v.crypto, 4, &[refs[0], refs[1], &unflagged]),
            Err(FormError::Mismatch)
        );
        let prepares: Vec<Vote> = [0u32, 1, 2]
            .iter()
            .map(|i| v.vote_flagged(VoteKind::Prepare, *i, &I, 9, 1, &h(2), &h(3), true))
            .collect();
        let prepare_qc = form_qc(&v.crypto, 4, &prepares.iter().collect::<Vec<_>>()).unwrap();
        assert!(prepare_qc.attest && prepare_qc.attestations.is_empty());
        assert_eq!(
            crate::crypto::Verifier::new(
                &v.crypto,
                &I,
                &crate::testing::TEST_EPOCH.id,
                &v.committee
            )
            .verify_qc(&NoAttestation, &prepare_qc),
            Ok(())
        );
    }

    /// The fail-closed extension and the fake authority (§3.7).
    #[test]
    fn attestation_extension_parts() {
        let v = validators(4);
        let key = v.key(0);
        assert_eq!(
            NoAttestation.attest(1, &key, b"s"),
            AttestOutcome::NoAuthority
        );
        assert!(!NoAttestation.verify(
            1,
            0,
            &key,
            b"s",
            &fake_attestation(&key, 1, b"s").witness,
            b"a"
        ));
        let none = Attestation::none();
        assert_eq!(
            none.attestor.attest(1, &key, b"s"),
            AttestOutcome::NoAuthority
        );
        assert_eq!(format!("{none:?}"), "Attestation");
        let fake = Attestation::new(Box::new(FakeAttestor::new()), Box::new(FakeVerifier));
        let AttestOutcome::Attested(attestation) = fake.attestor.attest(1, &key, b"s") else {
            panic!("the fake authority attests every key");
        };
        assert!(fake.verifier.verify(
            1,
            0,
            &key,
            b"s",
            &attestation.witness,
            attestation.signature.as_slice()
        ));
        assert!(
            !fake.verifier.verify(
                2,
                0,
                &key,
                b"s",
                &attestation.witness,
                attestation.signature.as_slice()
            ),
            "height bound"
        );
        assert!(
            !fake.verifier.verify(
                1,
                0,
                &v.key(1),
                b"s",
                &attestation.witness,
                attestation.signature.as_slice()
            ),
            "key bound"
        );
        assert!(
            !fake.verifier.verify(
                1,
                0,
                &key,
                b"t",
                &attestation.witness,
                attestation.signature.as_slice()
            ),
            "statement bound"
        );
        assert_eq!(
            FakeAttestor::without_authority([key.clone()]).attest(1, &key, b"s"),
            AttestOutcome::NoAuthority
        );
        let AttestOutcome::Attested(forged) = FakeAttestor::forging().attest(1, &key, b"s") else {
            panic!("a forging authority attests");
        };
        assert!(!FakeVerifier.verify(
            1,
            0,
            &key,
            b"s",
            &forged.witness,
            forged.signature.as_slice()
        ));
    }

    /// An execution-gated fake authority (§3.7 A2): `Pending` until its node executed the
    /// block to the statement's `R`, then the genuine attestation; `Executed` records,
    /// prunes applied heights and clears on a crash.
    #[test]
    fn execution_gated_attestor_waits_for_execution() {
        let v = validators(4);
        let key = v.key(0);
        let executed = crate::testing::Executed::new();
        assert!(executed.is_empty());
        let attestor = FakeAttestor::new().after_execution(executed.clone());
        let statement = preimage::att_preimage(&I, &crate::testing::TEST_EPOCH.id, 5, &h(2), &h(3));
        assert_eq!(attestor.attest(5, &key, &statement), AttestOutcome::Pending);
        executed.record(&I, &crate::testing::TEST_EPOCH.id, 5, &h(2), &h(4));
        assert_eq!(
            attestor.attest(5, &key, &statement),
            AttestOutcome::Pending,
            "another result"
        );
        let successor = crate::testing::scheduled_epoch(1, 5, 9).id;
        executed.record(&I, &successor, 5, &h(2), &h(3));
        assert_eq!(
            attestor.attest(5, &key, &statement),
            AttestOutcome::Pending,
            "another authenticated epoch"
        );
        let successor_statement = preimage::att_preimage(&I, &successor, 5, &h(2), &h(3));
        assert_eq!(
            attestor.attest(5, &key, &successor_statement),
            AttestOutcome::Attested(fake_attestation(&key, 5, &successor_statement))
        );
        executed.record(&I, &crate::testing::TEST_EPOCH.id, 5, &h(2), &h(3));
        assert!(executed.contains(5, &statement) && !executed.contains(6, &statement));
        assert_eq!(
            attestor.attest(5, &key, &statement),
            AttestOutcome::Attested(fake_attestation(&key, 5, &statement))
        );
        executed.record(&I, &crate::testing::TEST_EPOCH.id, 7, &h(2), &h(3));
        assert_eq!(executed.len(), 4);
        executed.prune_through(5);
        assert_eq!(executed.len(), 1);
        assert_eq!(attestor.attest(5, &key, &statement), AttestOutcome::Pending);
        executed.clear();
        assert!(executed.is_empty());
        // Authority still decides first.
        let gated = FakeAttestor::without_authority([key.clone()]).after_execution(executed);
        assert_eq!(
            gated.attest(5, &key, &statement),
            AttestOutcome::NoAuthority
        );
    }

    #[test]
    fn fake_crypto_provenance_available() {
        let log = SignLog::new();
        let v = FakeValidators::new(4, 3, Some(log.clone()));
        let qc = v.qc(VoteKind::Commit, &I, 9, 0, &h(2), &h(3), &[0, 1, 3]);
        assert!(log.qc_provenance_ok(&v.committee, &qc));
        let tc = v.tc(&I, 9, 0, &[(0, None), (1, None), (2, None)]);
        assert!(log.tc_provenance_ok(&v.committee, &tc));
    }
}
