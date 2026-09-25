//! Signer and crypto traits (spec §12.1) and pure verification and formation functions for
//! votes, timeouts, proposals and certificates (§3.4, §6.2 step 1, §6.4, §6.7, §11).
//!
//! Certificates are verified only against the committee of their own height (`C_{cert.height}`);
//! passing that committee is the caller's obligation (SR15).

use std::collections::BTreeMap;

use crate::{
    message::{Proposal, Qc, TcEntry, TimeoutCert, TimeoutVote, Vote, VoteKind},
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

/// Why a vote, timeout, proposal or certificate failed verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertError {
    /// `instance ≠ I`.
    WrongInstance,
    /// A signer index `≥ n`.
    SignerOutOfRange,
    /// Bitmap length not `ceil(n/8)` or spare bits set.
    MalformedBitmap,
    /// Fewer than `q` distinct signers.
    TooFewSigners,
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
}

/// Verify a `PrepareQC` or `CommitQC` under `committee = C_{qc.height}` (§3.4 `verify_qc`):
/// instance, exact bitmap, `popcount ≥ q`, aggregate signature over the vote preimage.
/// Certificates with more than `q` signers are valid.
///
/// # Errors
/// The first failed check.
pub fn verify_qc(
    crypto: &dyn Crypto,
    instance: &Hash32,
    committee: &Committee,
    qc: &Qc,
) -> Result<(), CertError> {
    if &qc.instance != instance {
        return Err(CertError::WrongInstance);
    }
    let pks = committee
        .keys_of(&qc.signers)
        .ok_or(CertError::MalformedBitmap)?;
    #[cfg(not(sumeragi_mutation = "MS14"))]
    let needed = committee.q();
    #[cfg(sumeragi_mutation = "MS14")]
    let needed = committee.q().saturating_sub(1);
    if pks.len() < needed {
        return Err(CertError::TooFewSigners);
    }
    if crypto.verify_aggregate(&pks, &qc.preimage(), &qc.agg_sig) {
        Ok(())
    } else {
        Err(CertError::BadSignature)
    }
}

/// Light-client check of a foreign instance's `CommitQC` (§11): `kind == Commit` and
/// [`verify_qc`] under the committee of the certificate's height.
pub fn verify_commit_qc(
    crypto: &dyn Crypto,
    instance: &Hash32,
    committee: &Committee,
    qc: &Qc,
) -> bool {
    qc.kind == VoteKind::Commit && verify_qc(crypto, instance, committee, qc).is_ok()
}

/// Verify a TC under `committee = C_{tc.height}` (§3.4 `verify_tc`), including the recomputation
/// of `max(hq)` and the full verification of `high_pqc` (SR12).
///
/// # Errors
/// The first failed check.
pub fn verify_tc(
    crypto: &dyn Crypto,
    instance: &Hash32,
    committee: &Committee,
    tc: &TimeoutCert,
) -> Result<(), CertError> {
    verify_tc_inner(crypto, instance, committee, tc, true)
}

/// [`verify_tc`] for a caller that has already verified (or holds a cached verdict for) the exact
/// `tc.high_pqc`: every check except the signature check of `high_pqc` itself.
///
/// # Errors
/// The first failed check.
pub fn verify_tc_with_verified_high_qc(
    crypto: &dyn Crypto,
    instance: &Hash32,
    committee: &Committee,
    tc: &TimeoutCert,
) -> Result<(), CertError> {
    verify_tc_inner(crypto, instance, committee, tc, false)
}

fn verify_tc_inner(
    crypto: &dyn Crypto,
    instance: &Hash32,
    committee: &Committee,
    tc: &TimeoutCert,
    check_high_qc: bool,
) -> Result<(), CertError> {
    if &tc.instance != instance {
        return Err(CertError::WrongInstance);
    }
    if tc.entries.len() < committee.q() {
        return Err(CertError::TooFewSigners);
    }
    // Group signers by signed `hq` (BTreeMap: deterministic group order).
    let mut groups: BTreeMap<Option<u64>, Vec<&PublicKey>> = BTreeMap::new();
    let mut previous: Option<ValidatorIndex> = None;
    for entry in &tc.entries {
        if previous.is_some_and(|p| entry.signer <= p) {
            return Err(CertError::UnorderedEntries);
        }
        previous = Some(entry.signer);
        let pk = committee
            .get(entry.signer)
            .ok_or(CertError::SignerOutOfRange)?;
        if entry.hq.is_some_and(|hq| hq > tc.view) {
            return Err(CertError::HqAboveView);
        }
        groups.entry(entry.hq).or_default().push(pk);
    }
    // SR12: the attached PrepareQC must be the one of the maximal *signed* hq.
    match (tc.max_hq(), &tc.high_pqc) {
        (None, None) => {}
        (Some(max), Some(qc)) => {
            if qc.kind != VoteKind::Prepare
                || qc.instance != tc.instance
                || qc.height != tc.height
                || (qc.view != max && !cfg!(sumeragi_mutation = "MS12"))
            {
                return Err(CertError::HighQcMismatch);
            }
            if check_high_qc && verify_qc(crypto, instance, committee, qc).is_err() {
                return Err(CertError::HighQcInvalid);
            }
        }
        _ => return Err(CertError::HighQcPresence),
    }
    let groups: Vec<(Vec<&PublicKey>, Vec<u8>)> = groups
        .into_iter()
        .map(|(hq, pks)| {
            (
                pks,
                preimage::tmo_preimage(&tc.instance, tc.height, tc.view, hq),
            )
        })
        .collect();
    if crypto.verify_aggregate_multi(&groups, &tc.agg_sig) {
        Ok(())
    } else {
        Err(CertError::BadSignature)
    }
}

/// Verify an individual vote under `committee = C_{vote.height}` (§6.4 step 3): instance,
/// signer range and signature.
///
/// # Errors
/// The first failed check.
pub fn verify_vote(
    crypto: &dyn Crypto,
    instance: &Hash32,
    committee: &Committee,
    vote: &Vote,
) -> Result<(), CertError> {
    if &vote.instance != instance {
        return Err(CertError::WrongInstance);
    }
    let pk = committee
        .get(vote.signer)
        .ok_or(CertError::SignerOutOfRange)?;
    if crypto.verify(pk, &vote.preimage(), &vote.sig) {
        Ok(())
    } else {
        Err(CertError::BadSignature)
    }
}

/// Verify a timeout vote's own signature and the shape of its carried `PrepareQC` (§6.7 step 1),
/// without verifying the carried certificate's aggregate signature (see [`verify_timeout`]).
///
/// # Errors
/// The first failed check.
pub fn verify_timeout_signature(
    crypto: &dyn Crypto,
    instance: &Hash32,
    committee: &Committee,
    timeout: &TimeoutVote,
) -> Result<(), CertError> {
    if &timeout.instance != instance {
        return Err(CertError::WrongInstance);
    }
    let pk = committee
        .get(timeout.signer)
        .ok_or(CertError::SignerOutOfRange)?;
    if let Some(qc) = &timeout.high_pqc {
        if qc.kind != VoteKind::Prepare
            || qc.instance != timeout.instance
            || qc.height != timeout.height
        {
            return Err(CertError::HighQcMismatch);
        }
        if qc.view > timeout.view {
            return Err(CertError::HqAboveView);
        }
    }
    if crypto.verify(pk, &timeout.preimage(), &timeout.sig) {
        Ok(())
    } else {
        Err(CertError::BadSignature)
    }
}

/// Full timeout-vote verification (§6.7 step 1): [`verify_timeout_signature`] plus
/// [`verify_qc`] of the carried `PrepareQC`.
///
/// # Errors
/// The first failed check.
pub fn verify_timeout(
    crypto: &dyn Crypto,
    instance: &Hash32,
    committee: &Committee,
    timeout: &TimeoutVote,
) -> Result<(), CertError> {
    verify_timeout_signature(crypto, instance, committee, timeout)?;
    timeout.high_pqc.as_ref().map_or(Ok(()), |qc| {
        verify_qc(crypto, instance, committee, qc).map_err(|_| CertError::HighQcInvalid)
    })
}

/// Verify a proposal's signature by the round leader `leader` (`idx(L(h, v))`) of
/// `committee = C_{p.height}` over `prop_preimage(h, v, bh, ad)` (§6.2 step 1). Returns
/// `(bh, ad)` on success.
///
/// # Errors
/// Wrong instance, leader index out of range, or bad signature.
pub fn verify_proposal_signature(
    crypto: &dyn Crypto,
    instance: &Hash32,
    committee: &Committee,
    leader: ValidatorIndex,
    proposal: &Proposal,
) -> Result<(Hash32, Hash32), CertError> {
    if &proposal.instance != instance {
        return Err(CertError::WrongInstance);
    }
    let pk = committee.get(leader).ok_or(CertError::SignerOutOfRange)?;
    let bh = proposal.block_hash(crypto);
    let ad = proposal.att_digest(crypto);
    let msg = preimage::prop_preimage(&proposal.instance, proposal.height, proposal.view, &bh, &ad);
    if crypto.verify(pk, &msg, &proposal.sig) {
        Ok((bh, ad))
    } else {
        Err(CertError::BadSignature)
    }
}

/// Why a certificate could not be formed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormError {
    /// Fewer than `q` inputs.
    TooFew,
    /// Inputs disagree on kind, instance, height, view or value.
    Mismatch,
    /// A signer twice or out of range.
    BadSigner,
}

/// Form a QC from `votes` (§3.4 formation): every vote must have identical
/// `(kind, instance, height, view, block_hash, result)` and a distinct signer `< n`; at least
/// `q(n)` votes. All given votes are used (callers pass exactly the `q` they hold). The inputs
/// must already be verified (SR38): formation never verifies.
///
/// # Errors
/// [`FormError`] when the inputs cannot form a certificate.
pub fn form_qc(crypto: &dyn Crypto, n: usize, votes: &[&Vote]) -> Result<Qc, FormError> {
    let first = votes.first().ok_or(FormError::TooFew)?;
    if votes.len() < crate::types::quorum(n) {
        return Err(FormError::TooFew);
    }
    let mut sorted: Vec<&Vote> = votes.to_vec();
    sorted.sort_by_key(|vote| vote.signer);
    let mut signers = Bitmap::new(n);
    for vote in &sorted {
        if (
            vote.kind,
            vote.instance,
            vote.height,
            vote.view,
            vote.block_hash,
            vote.result,
        ) != (
            first.kind,
            first.instance,
            first.height,
            first.view,
            first.block_hash,
            first.result,
        ) {
            return Err(FormError::Mismatch);
        }
        if signers.get(vote.signer) || !signers.set(vote.signer) || !signers.is_well_formed(n) {
            return Err(FormError::BadSigner);
        }
    }
    let sigs: Vec<Signature> = sorted.iter().map(|vote| vote.sig).collect();
    Ok(Qc {
        kind: first.kind,
        instance: first.instance,
        height: first.height,
        view: first.view,
        block_hash: first.block_hash,
        result: first.result,
        signers,
        agg_sig: crypto.aggregate(&sigs),
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
    if timeouts.len() < q {
        return Err(FormError::TooFew);
    }
    let mut seen = Bitmap::new(n);
    for timeout in timeouts {
        if (timeout.instance, timeout.height, timeout.view)
            != (first.instance, first.height, first.view)
        {
            return Err(FormError::Mismatch);
        }
        if seen.get(timeout.signer) || !seen.set(timeout.signer) || !seen.is_well_formed(n) {
            return Err(FormError::BadSigner);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        testing::{FakeValidators, SignLog},
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
    fn qc_verifies_and_rejects_tampering() {
        for n in [1usize, 4, 5, 7, 22] {
            let v = validators(n);
            let q = v.committee.q();
            let signers: Vec<ValidatorIndex> = (0..crate::types::index_of(q)).collect();
            let qc = v.qc(VoteKind::Commit, &I, 5, 1, &h(2), &h(3), &signers);
            assert_eq!(verify_qc(&v.crypto, &I, &v.committee, &qc), Ok(()), "n={n}");
            assert!(verify_commit_qc(&v.crypto, &I, &v.committee, &qc));
            let prepare = v.qc(VoteKind::Prepare, &I, 5, 1, &h(2), &h(3), &signers);
            assert!(!verify_commit_qc(&v.crypto, &I, &v.committee, &prepare));
            assert!(!verify_commit_qc(&v.crypto, &J, &v.committee, &qc));
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
                    verify_qc(&v.crypto, &I, &v.committee, &bad),
                    Err(CertError::BadSignature),
                    "n={n}"
                );
            }
            // Foreign instance.
            assert_eq!(
                verify_qc(&v.crypto, &J, &v.committee, &qc),
                Err(CertError::WrongInstance)
            );
            // Malformed bitmaps.
            let mut long = qc.signers.as_bytes().to_vec();
            long.push(0);
            assert_eq!(
                verify_qc(
                    &v.crypto,
                    &I,
                    &v.committee,
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
                    verify_qc(
                        &v.crypto,
                        &I,
                        &v.committee,
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
                verify_qc(&v.crypto, &I, &v.committee, &qc),
                Err(CertError::TooFewSigners),
                "n={n}"
            );
            // More than q signers is fine.
            let all: Vec<ValidatorIndex> = (0..crate::types::index_of(n)).collect();
            let qc = v.qc(VoteKind::Prepare, &I, 5, 1, &h(2), &h(3), &all);
            assert_eq!(verify_qc(&v.crypto, &I, &v.committee, &qc), Ok(()));
        }
    }

    #[test]
    fn det_s13_quorum_n5_qc_with_three_signers_rejected() {
        let v = validators(5);
        let qc = v.qc(VoteKind::Commit, &I, 3, 0, &h(2), &h(3), &[0, 1, 2]);
        assert_eq!(
            verify_qc(&v.crypto, &I, &v.committee, &qc),
            Err(CertError::TooFewSigners)
        );
        let qc = v.qc(VoteKind::Commit, &I, 3, 0, &h(2), &h(3), &[0, 1, 2, 4]);
        assert_eq!(verify_qc(&v.crypto, &I, &v.committee, &qc), Ok(()));
    }

    #[test]
    fn det_s15_wrong_committee_rejected() {
        // A CommitQC signed by one committee never verifies under another (e.g. removed members
        // certifying a later height whose committee differs).
        let old = FakeValidators::new(4, 1, None);
        let new = FakeValidators::new(4, 2, None);
        let qc = old.qc(VoteKind::Commit, &I, 9, 0, &h(2), &h(3), &[0, 1, 2]);
        assert_eq!(verify_qc(&old.crypto, &I, &old.committee, &qc), Ok(()));
        assert_eq!(
            verify_qc(&new.crypto, &I, &new.committee, &qc),
            Err(CertError::BadSignature)
        );
        assert!(!verify_commit_qc(&new.crypto, &I, &new.committee, &qc));
        // Larger committee: the bitmap length no longer matches.
        let bigger = FakeValidators::new(9, 1, None);
        assert_eq!(
            verify_qc(&bigger.crypto, &I, &bigger.committee, &qc),
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
            verify_qc(&v.crypto, &J, &v.committee, &replay),
            Err(CertError::BadSignature)
        );
        let vote = v.vote(VoteKind::Prepare, 1, &I, 9, 0, &h(2), &h(3));
        let replay = Vote {
            instance: J,
            ..vote
        };
        assert_eq!(
            verify_vote(&v.crypto, &J, &v.committee, &replay),
            Err(CertError::BadSignature)
        );
        assert_eq!(
            verify_vote(&v.crypto, &J, &v.committee, &vote),
            Err(CertError::WrongInstance)
        );
        let timeout = v.timeout(1, &I, 9, 0, None);
        let replay = TimeoutVote {
            instance: J,
            ..timeout.clone()
        };
        assert_eq!(
            verify_timeout(&v.crypto, &J, &v.committee, &replay),
            Err(CertError::BadSignature)
        );
        let tc = v.tc(&I, 9, 0, &[(0, None), (1, None), (2, None)]);
        let replay = TimeoutCert {
            instance: J,
            ..tc.clone()
        };
        assert_eq!(
            verify_tc(&v.crypto, &J, &v.committee, &replay),
            Err(CertError::BadSignature)
        );
        assert_eq!(
            verify_tc(&v.crypto, &J, &v.committee, &tc),
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
            verify_qc(&v.crypto, &I, &v.committee, &rewritten),
            Err(CertError::BadSignature)
        );
        let vote = v.vote(VoteKind::Commit, 0, &I, 9, 0, &h(2), &h(3));
        let rewritten = Vote {
            result: h(4),
            ..vote
        };
        assert_eq!(
            verify_vote(&v.crypto, &I, &v.committee, &rewritten),
            Err(CertError::BadSignature)
        );
    }

    #[test]
    fn votes_verify() {
        let v = validators(4);
        let vote = v.vote(VoteKind::Prepare, 3, &I, 9, 1, &h(2), &h(3));
        assert_eq!(verify_vote(&v.crypto, &I, &v.committee, &vote), Ok(()));
        assert_eq!(
            verify_vote(&v.crypto, &I, &v.committee, &Vote { signer: 2, ..vote }),
            Err(CertError::BadSignature)
        );
        assert_eq!(
            verify_vote(&v.crypto, &I, &v.committee, &Vote { signer: 4, ..vote }),
            Err(CertError::SignerOutOfRange)
        );
        assert_eq!(
            verify_vote(
                &v.crypto,
                &I,
                &v.committee,
                &Vote {
                    sig: Signature([7; SIGNATURE_LEN]),
                    ..vote
                }
            ),
            Err(CertError::BadSignature)
        );
    }

    #[test]
    fn timeouts_verify() {
        let v = validators(4);
        let pqc = v.qc(VoteKind::Prepare, &I, 9, 1, &h(2), &h(3), &[0, 1, 2]);
        let t = v.timeout(3, &I, 9, 2, Some(pqc.clone()));
        assert_eq!(verify_timeout(&v.crypto, &I, &v.committee, &t), Ok(()));
        let none = v.timeout(3, &I, 9, 2, None);
        assert_eq!(verify_timeout(&v.crypto, &I, &v.committee, &none), Ok(()));
        // hq is signed: swapping the carried QC for none (or another view) breaks the signature.
        assert_eq!(
            verify_timeout(
                &v.crypto,
                &I,
                &v.committee,
                &TimeoutVote {
                    high_pqc: None,
                    ..t.clone()
                }
            ),
            Err(CertError::BadSignature)
        );
        // Carried QC of a higher view than the timeout.
        let future = v.qc(VoteKind::Prepare, &I, 9, 3, &h(2), &h(3), &[0, 1, 2]);
        let t_future = v.timeout(3, &I, 9, 2, Some(future));
        assert_eq!(
            verify_timeout(&v.crypto, &I, &v.committee, &t_future),
            Err(CertError::HqAboveView)
        );
        // Carried QC of another height, kind or instance.
        for wrong in [
            v.qc(VoteKind::Commit, &I, 9, 1, &h(2), &h(3), &[0, 1, 2]),
            v.qc(VoteKind::Prepare, &I, 8, 1, &h(2), &h(3), &[0, 1, 2]),
            v.qc(VoteKind::Prepare, &J, 9, 1, &h(2), &h(3), &[0, 1, 2]),
        ] {
            let t_wrong = v.timeout(3, &I, 9, 2, Some(wrong));
            assert_eq!(
                verify_timeout(&v.crypto, &I, &v.committee, &t_wrong),
                Err(CertError::HighQcMismatch)
            );
        }
        // Carried QC that does not verify: signature-only check passes, full check fails.
        let forged = Qc {
            agg_sig: AggregateSignature([1; SIGNATURE_LEN]),
            ..pqc
        };
        let t_forged = v.timeout(3, &I, 9, 2, Some(forged));
        assert_eq!(
            verify_timeout_signature(&v.crypto, &I, &v.committee, &t_forged),
            Ok(())
        );
        assert_eq!(
            verify_timeout(&v.crypto, &I, &v.committee, &t_forged),
            Err(CertError::HighQcInvalid)
        );
        assert_eq!(
            verify_timeout(
                &v.crypto,
                &I,
                &v.committee,
                &TimeoutVote { signer: 9, ..none }
            ),
            Err(CertError::SignerOutOfRange)
        );
    }

    #[test]
    fn proposal_signature() {
        let v = validators(4);
        let header = crate::message::BlockHeader {
            instance: I,
            height: 9,
            origin_view: 0,
            parent_hash: h(1),
            parent_result: h(2),
            payload_hash: preimage::payload_hash(&v.crypto, &[]),
            payload_len: 0,
            proposer: 2,
            skipped_leaders: vec![],
        };
        let parent = v.qc(VoteKind::Commit, &I, 8, 0, &h(1), &h(2), &[0, 1, 2]);
        let p = v.proposal(2, &I, 9, 0, header, None, Some(parent), Some(vec![]));
        let (bh, ad) = verify_proposal_signature(&v.crypto, &I, &v.committee, 2, &p).unwrap();
        assert_eq!(bh, p.block_hash(&v.crypto));
        assert_eq!(ad, p.att_digest(&v.crypto));
        // Not the leader's key.
        assert_eq!(
            verify_proposal_signature(&v.crypto, &I, &v.committee, 1, &p),
            Err(CertError::BadSignature)
        );
        assert_eq!(
            verify_proposal_signature(&v.crypto, &I, &v.committee, 7, &p),
            Err(CertError::SignerOutOfRange)
        );
        assert_eq!(
            verify_proposal_signature(&v.crypto, &J, &v.committee, 2, &p),
            Err(CertError::WrongInstance)
        );
        // The payload is unsigned: stripping it keeps the signature valid.
        let stripped = Proposal {
            payload: None,
            ..p.clone()
        };
        assert!(verify_proposal_signature(&v.crypto, &I, &v.committee, 2, &stripped).is_ok());
        // The attachments are signed: stripping the parent QC breaks it.
        let tampered = Proposal {
            parent_qc: None,
            ..p.clone()
        };
        assert_eq!(
            verify_proposal_signature(&v.crypto, &I, &v.committee, 2, &tampered),
            Err(CertError::BadSignature)
        );
        let tampered = Proposal { view: 1, ..p };
        assert_eq!(
            verify_proposal_signature(&v.crypto, &I, &v.committee, 2, &tampered),
            Err(CertError::BadSignature)
        );
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
        assert_eq!(verify_tc(&v.crypto, &I, &v.committee, &tc), Ok(()));
        assert_eq!(
            verify_tc_with_verified_high_qc(&v.crypto, &I, &v.committee, &tc),
            Ok(())
        );
        // All-None TC.
        let tc_none = v.tc(&I, 9, 2, &[(0, None), (1, None), (2, None), (3, None)]);
        assert_eq!(tc_none.high_pqc, None);
        assert_eq!(verify_tc(&v.crypto, &I, &v.committee, &tc_none), Ok(()));

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
                verify_tc(&v.crypto, &I, &v.committee, &bad),
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
            verify_tc_with_verified_high_qc(&v.crypto, &I, &v.committee, &trusted),
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
            verify_tc(&v.crypto, &I, &v.committee, &lowered),
            Err(CertError::HighQcMismatch)
        );
        assert_eq!(
            verify_tc_with_verified_high_qc(&v.crypto, &I, &v.committee, &lowered),
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
        assert_eq!(verify_qc(&v.crypto, &I, &v.committee, &qc), Ok(()));
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
            ..votes[0]
        };
        assert_eq!(
            form_qc(&v.crypto, 4, &[refs[0], refs[1], &out_of_range]),
            Err(FormError::BadSigner)
        );
        let spare = Vote {
            signer: 5,
            ..votes[0]
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
            assert_eq!(verify_tc(&v.crypto, &I, &v.committee, &tc), Ok(()));
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
        assert_eq!(verify_tc(&v.crypto, &I, &v.committee, &tc), Ok(()));

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
