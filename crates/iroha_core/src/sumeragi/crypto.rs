//! Production cryptography of the Sumeragi driver (`specs/sumeragi.md` §1, §3.4, §12.1): the
//! chain hash `H` is [`iroha_crypto::Hash`] (32 bytes, least significant bit set, DECISIONS #4),
//! signatures are BLS-normal (48-byte G1 keys, 96-byte G2 signatures), and the node signs with its
//! key pair.
//!
//! Aggregate verification is rogue-key safe only for keys whose proof of possession (`PoP`)
//! verified. [`BlsCrypto`] therefore verifies a committee key's `PoP` once, when the height
//! schedule admits the key ([`BlsCrypto::admit`], [`BlsCrypto::admit_committee`]), keeps the
//! parsed key, and refuses every aggregate that names a key it has not admitted (fail closed).
//! Individual signatures need no `PoP` and verify under any well-formed BLS-normal key.

use std::{collections::HashMap, sync::Arc};

use iroha_crypto::{
    Algorithm, BlsNormalPopVerifiedKey, Hash, KeyPair, PrivateKey, PublicKey as IrohaPublicKey,
    Signature as IrohaSignature, bls_normal_aggregate_signatures,
    bls_normal_verify_preaggregated_multi_message,
};
use iroha_sumeragi::{
    crypto::{Crypto, Signer},
    types::{AggregateSignature, Hash32, PublicKey, SIGNATURE_LEN, Signature},
};
use parking_lot::RwLock;

/// Why a key cannot be used as a Sumeragi consensus key.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    /// Consensus keys are BLS-normal.
    #[error("consensus keys must be BLS-normal, not {0}")]
    NotBlsNormal(Algorithm),
    /// The bytes are not a valid BLS-normal public key.
    #[error("malformed BLS-normal public key: {0}")]
    Malformed(String),
    /// The proof of possession does not verify for the key.
    #[error("the proof of possession does not verify: {0}")]
    BadPop(String),
}

/// The core's key (raw 48-byte compressed G1 point) of a BLS-normal Iroha key.
///
/// # Errors
/// [`KeyError::NotBlsNormal`] for another algorithm, [`KeyError::Malformed`] for a key whose
/// payload the core does not accept.
pub fn core_key(key: &IrohaPublicKey) -> Result<PublicKey, KeyError> {
    let (algorithm, payload) = key
        .try_to_bytes()
        .map_err(|e| KeyError::Malformed(e.to_string()))?;
    if algorithm != Algorithm::BlsNormal {
        return Err(KeyError::NotBlsNormal(algorithm));
    }
    PublicKey::new(payload.to_vec()).map_err(|e| KeyError::Malformed(e.to_string()))
}

/// The Iroha BLS-normal key of a core key (validated: canonical, in the subgroup, not the
/// identity).
///
/// # Errors
/// [`KeyError::Malformed`] for bytes that are not such a key (e.g. from a peer).
pub fn iroha_key(key: &PublicKey) -> Result<IrohaPublicKey, KeyError> {
    IrohaPublicKey::from_bytes(Algorithm::BlsNormal, key.as_bytes())
        .map_err(|e| KeyError::Malformed(e.to_string()))
}

/// The production [`Crypto`]: `H = iroha_crypto::Hash` and BLS-normal signatures, with the
/// `PoP`-verified keys admitted by the height schedule. Shared by every thread and instance of
/// the node (`Send + Sync`). The admitted set holds one parsed key per key ever admitted (a few
/// hundred bytes each); a key stays admitted, since certificates of old heights still name it.
#[derive(Debug, Default)]
pub struct BlsCrypto {
    admitted: RwLock<HashMap<PublicKey, Arc<BlsNormalPopVerifiedKey>>>,
}

impl BlsCrypto {
    /// A crypto with no admitted key.
    pub fn new() -> Self {
        Self::default()
    }

    /// Admit a committee key: verify its proof of possession `pop` once and keep the parsed
    /// key for aggregate verification. The height schedule calls this for every key it
    /// schedules into a committee (genesis committee, registrations, key rotations), before
    /// the configuration reaches the core. Admitting a key again is harmless.
    ///
    /// # Errors
    /// A key that is not BLS-normal, or a `PoP` that does not verify; nothing is admitted.
    pub fn admit(&self, key: &IrohaPublicKey, pop: &[u8]) -> Result<PublicKey, KeyError> {
        let core = core_key(key)?;
        let verified =
            BlsNormalPopVerifiedKey::new(key, pop).map_err(|e| KeyError::BadPop(e.to_string()))?;
        self.admitted
            .write()
            .insert(core.clone(), Arc::new(verified));
        Ok(core)
    }

    /// Admit every member of a committee (all or nothing): the keys in the given order.
    ///
    /// # Errors
    /// The first member whose key or `PoP` is invalid (with its position); nothing is admitted.
    pub fn admit_committee<'a>(
        &self,
        members: impl IntoIterator<Item = (&'a IrohaPublicKey, &'a [u8])>,
    ) -> Result<Vec<PublicKey>, (usize, KeyError)> {
        let mut verified = Vec::new();
        for (position, (key, pop)) in members.into_iter().enumerate() {
            let core = core_key(key).map_err(|e| (position, e))?;
            let key = BlsNormalPopVerifiedKey::new(key, pop)
                .map_err(|e| (position, KeyError::BadPop(e.to_string())))?;
            verified.push((core, Arc::new(key)));
        }
        let mut admitted = self.admitted.write();
        Ok(verified
            .into_iter()
            .map(|(core, key)| {
                admitted.insert(core.clone(), key);
                core
            })
            .collect())
    }

    /// Whether `key` was admitted.
    pub fn is_admitted(&self, key: &PublicKey) -> bool {
        self.admitted.read().contains_key(key)
    }

    /// Number of admitted keys.
    pub fn admitted_len(&self) -> usize {
        self.admitted.read().len()
    }

    /// The admitted keys of `groups`, or `None` if one is not admitted.
    fn admitted_groups(
        &self,
        groups: &[(Vec<&PublicKey>, Vec<u8>)],
    ) -> Option<Vec<Vec<Arc<BlsNormalPopVerifiedKey>>>> {
        let admitted = self.admitted.read();
        groups
            .iter()
            .map(|(keys, _)| {
                keys.iter()
                    .map(|key| admitted.get(*key).cloned())
                    .collect::<Option<Vec<_>>>()
            })
            .collect()
    }
}

impl Crypto for BlsCrypto {
    fn hash(&self, bytes: &[u8]) -> Hash32 {
        Hash32(Hash::new(bytes).into())
    }

    fn verify(&self, pk: &PublicKey, msg: &[u8], sig: &Signature) -> bool {
        iroha_key(pk).is_ok_and(|key| IrohaSignature::from_bytes(&sig.0).verify(&key, msg).is_ok())
    }

    fn aggregate(&self, sigs: &[Signature]) -> AggregateSignature {
        let payloads: Vec<&[u8]> = sigs.iter().map(|sig| &sig.0[..]).collect();
        // The core aggregates verified signatures only; a failure (none, or a malformed one)
        // gives the all-zero aggregate, which never verifies.
        bls_normal_aggregate_signatures(&payloads)
            .ok()
            .and_then(|bytes| <[u8; SIGNATURE_LEN]>::try_from(bytes).ok())
            .map_or_else(
                || {
                    iroha_logger::error!(count = sigs.len(), "BLS aggregation failed");
                    AggregateSignature([0; SIGNATURE_LEN])
                },
                AggregateSignature,
            )
    }

    fn verify_aggregate(&self, pks: &[&PublicKey], msg: &[u8], agg: &AggregateSignature) -> bool {
        self.verify_aggregate_multi(&[(pks.to_vec(), msg.to_vec())], agg)
    }

    fn verify_aggregate_multi(
        &self,
        groups: &[(Vec<&PublicKey>, Vec<u8>)],
        agg: &AggregateSignature,
    ) -> bool {
        let Some(keys) = self.admitted_groups(groups) else {
            return false;
        };
        let refs: Vec<Vec<&BlsNormalPopVerifiedKey>> = keys
            .iter()
            .map(|group| group.iter().map(AsRef::as_ref).collect())
            .collect();
        let groups: Vec<(&[&BlsNormalPopVerifiedKey], &[u8])> = refs
            .iter()
            .zip(groups)
            .map(|(keys, (_, msg))| (keys.as_slice(), msg.as_slice()))
            .collect();
        bls_normal_verify_preaggregated_multi_message(&groups, &agg.0).is_ok()
    }
}

/// The node's consensus [`Signer`] over its BLS-normal key pair: local, non-blocking and
/// deterministic (BLS signatures are unique for a key and message), as §12.1 requires.
pub struct KeyPairSigner {
    public: PublicKey,
    private: PrivateKey,
}

impl KeyPairSigner {
    /// A signer for `key_pair`.
    ///
    /// # Errors
    /// The key pair is not BLS-normal.
    pub fn new(key_pair: &KeyPair) -> Result<Self, KeyError> {
        Ok(Self {
            public: core_key(key_pair.public_key())?,
            private: key_pair.private_key().clone(),
        })
    }
}

impl core::fmt::Debug for KeyPairSigner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KeyPairSigner")
            .field("public", &self.public)
            .finish_non_exhaustive()
    }
}

impl Signer for KeyPairSigner {
    fn public_key(&self) -> &PublicKey {
        &self.public
    }

    fn sign(&self, preimage: &[u8]) -> Signature {
        // Signing with a valid BLS-normal key does not fail; if it ever does, the all-zero
        // signature is sent, which every verifier rejects (the message is lost, like a drop).
        IrohaSignature::try_new(&self.private, preimage)
            .ok()
            .and_then(|sig| <[u8; SIGNATURE_LEN]>::try_from(sig.payload()).ok())
            .map_or_else(
                || {
                    iroha_logger::error!("BLS signing failed");
                    Signature([0; SIGNATURE_LEN])
                },
                Signature,
            )
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::bls_normal_pop_prove;
    use iroha_sumeragi::{
        crypto::{NoAttestation, form_qc, form_tc, verify_qc, verify_tc},
        message::{Qc, TimeoutVote, Vote, VoteKind},
        types::{Committee, EpochId, ValidatorIndex},
    };

    use super::*;

    const I: Hash32 = Hash32([0x42; 32]);
    const EPOCH: EpochId = EpochId {
        epoch: 7,
        context: Hash32([0x51; 32]),
    };

    fn key_pair(seed: u8) -> KeyPair {
        KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal)
    }

    fn pop(key_pair: &KeyPair) -> Vec<u8> {
        bls_normal_pop_prove(key_pair.private_key()).unwrap()
    }

    /// `n` validators: their signers, a crypto that admitted every key, and the committee.
    fn validators(n: u8) -> (Vec<KeyPairSigner>, BlsCrypto, Committee) {
        let pairs: Vec<KeyPair> = (1..=n).map(key_pair).collect();
        let pops: Vec<Vec<u8>> = pairs.iter().map(pop).collect();
        let crypto = BlsCrypto::new();
        crypto
            .admit_committee(
                pairs
                    .iter()
                    .zip(&pops)
                    .map(|(kp, pop)| (kp.public_key(), pop.as_slice())),
            )
            .unwrap();
        let signers: Vec<KeyPairSigner> = pairs
            .iter()
            .map(|kp| KeyPairSigner::new(kp).unwrap())
            .collect();
        let committee =
            Committee::new(signers.iter().map(|s| s.public_key().clone()).collect()).unwrap();
        (signers, crypto, committee)
    }

    fn index(committee: &Committee, signer: &KeyPairSigner) -> ValidatorIndex {
        committee.index_of(signer.public_key()).unwrap()
    }

    fn vote(kind: VoteKind, view: u64, signer: &KeyPairSigner, committee: &Committee) -> Vote {
        let mut vote = Vote {
            kind,
            instance: I,
            epoch: EPOCH,
            height: 7,
            view,
            block_hash: Hash32([1; 32]),
            result: Hash32([2; 32]),
            attest: false,
            signer: index(committee, signer),
            sig: Signature([0; SIGNATURE_LEN]),
            attestation: None,
        };
        vote.sig = signer.sign(&vote.preimage());
        vote
    }

    fn qc(kind: VoteKind, view: u64, signers: &[&KeyPairSigner], committee: &Committee) -> Qc {
        let crypto = BlsCrypto::new();
        let votes: Vec<Vote> = signers
            .iter()
            .map(|s| vote(kind, view, s, committee))
            .collect();
        let refs: Vec<&Vote> = votes.iter().collect();
        form_qc(&crypto, committee.n(), &refs).unwrap()
    }

    /// `H` is `iroha_crypto::Hash`: Blake2b-256 with the least significant bit set.
    #[test]
    fn hash_is_the_iroha_hash() {
        let crypto = BlsCrypto::new();
        let h = crypto.hash(b"sumeragi");
        assert_eq!(h.0, <[u8; 32]>::from(Hash::new(b"sumeragi")));
        assert_eq!(h.0[31] & 1, 1);
        assert_ne!(crypto.hash(b"a"), crypto.hash(b"b"));
    }

    /// Core keys and Iroha keys convert both ways; other algorithms and malformed bytes are
    /// refused.
    #[test]
    fn key_conversions() {
        let kp = key_pair(9);
        let core = core_key(kp.public_key()).unwrap();
        assert_eq!(core.as_bytes().len(), 48);
        assert_eq!(&iroha_key(&core).unwrap(), kp.public_key());
        let ed = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
        assert_eq!(
            core_key(ed.public_key()),
            Err(KeyError::NotBlsNormal(Algorithm::Ed25519))
        );
        let junk = PublicKey::new(vec![0xAB; 48]).unwrap();
        assert!(matches!(iroha_key(&junk), Err(KeyError::Malformed(_))));
    }

    /// The signer is deterministic and its signatures verify for exactly its key and
    /// message; malformed keys and signatures are rejected without panicking.
    #[test]
    fn signer_signs_deterministically_and_verifies() {
        let signers = (1..=2)
            .map(|seed| KeyPairSigner::new(&key_pair(seed)).unwrap())
            .collect::<Vec<_>>();
        let crypto = BlsCrypto::new();
        let sig = signers[0].sign(b"preimage");
        assert_eq!(sig, signers[0].sign(b"preimage"), "deterministic");
        assert!(crypto.verify(signers[0].public_key(), b"preimage", &sig));
        assert!(!crypto.verify(signers[0].public_key(), b"other", &sig));
        assert!(!crypto.verify(signers[1].public_key(), b"preimage", &sig));
        let junk = PublicKey::new(vec![7; 48]).unwrap();
        assert!(!crypto.verify(&junk, b"preimage", &sig));
        assert!(!crypto.verify(
            signers[0].public_key(),
            b"preimage",
            &Signature([0xFF; SIGNATURE_LEN])
        ));
        assert!(format!("{:?}", signers[0]).contains("KeyPairSigner"));
        let ed = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
        assert!(KeyPairSigner::new(&ed).is_err());
    }

    /// Aggregates verify only under admitted keys (fail closed) and only for the signers and
    /// message they cover; a failed aggregation never verifies.
    #[test]
    fn aggregates_need_admitted_keys() {
        let signers = (1..=3)
            .map(|seed| KeyPairSigner::new(&key_pair(seed)).unwrap())
            .collect::<Vec<_>>();
        let sigs: Vec<Signature> = signers.iter().map(|s| s.sign(b"m")).collect();
        let fresh = BlsCrypto::new();
        let agg = fresh.aggregate(&sigs);
        let keys: Vec<&PublicKey> = signers.iter().map(Signer::public_key).collect();
        assert!(!fresh.verify_aggregate(&keys, b"m", &agg), "not admitted");
        let kps: Vec<KeyPair> = (1..=3).map(key_pair).collect();
        for kp in &kps[..2] {
            fresh.admit(kp.public_key(), &pop(kp)).unwrap();
        }
        assert!(fresh.is_admitted(&keys[0]) && !fresh.is_admitted(&keys[2]));
        assert!(
            !fresh.verify_aggregate(&keys, b"m", &agg),
            "one not admitted"
        );
        fresh.admit(kps[2].public_key(), &pop(&kps[2])).unwrap();
        assert_eq!(fresh.admitted_len(), 3);
        assert!(fresh.verify_aggregate(&keys, b"m", &agg));
        assert!(!fresh.verify_aggregate(&keys, b"n", &agg));
        assert!(!fresh.verify_aggregate(&keys[..2], b"m", &agg));
        assert_eq!(fresh.aggregate(&[]), AggregateSignature([0; SIGNATURE_LEN]));
        assert!(!fresh.verify_aggregate(&keys, b"m", &fresh.aggregate(&[])));
    }

    /// A proof of possession is verified at admission; a bad one admits nothing.
    #[test]
    fn admission_verifies_pops() {
        let crypto = BlsCrypto::new();
        let (a, b) = (key_pair(1), key_pair(2));
        assert!(matches!(
            crypto.admit(a.public_key(), &pop(&b)),
            Err(KeyError::BadPop(_))
        ));
        let ed = KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519);
        assert!(matches!(
            crypto.admit(ed.public_key(), &pop(&a)),
            Err(KeyError::NotBlsNormal(_))
        ));
        let (pa, pb) = (pop(&a), pop(&b));
        let bad = crypto.admit_committee([
            (a.public_key(), pa.as_slice()),
            (b.public_key(), pa.as_slice()),
        ]);
        assert!(matches!(bad, Err((1, KeyError::BadPop(_)))));
        assert_eq!(crypto.admitted_len(), 0, "all or nothing");
        let admitted = crypto
            .admit_committee([
                (a.public_key(), pa.as_slice()),
                (b.public_key(), pb.as_slice()),
            ])
            .unwrap();
        assert_eq!(admitted.len(), 2);
        assert_eq!(crypto.admit(a.public_key(), &pa).unwrap(), admitted[0]);
        assert_eq!(crypto.admitted_len(), 2);
    }

    /// Cross-check with the core: a QC formed by the core with this crypto verifies with
    /// `iroha_sumeragi::crypto::verify_qc`, and not after tampering.
    #[test]
    fn core_formed_qc_verifies() {
        for n in [4u8, 7, 10, 31] {
            let (signers, crypto, committee) = validators(n);
            let q = committee.q();
            let chosen: Vec<&KeyPairSigner> = signers.iter().take(q).collect();
            let qc = qc(VoteKind::Commit, 3, &chosen, &committee);
            assert_eq!(
                verify_qc(&crypto, &NoAttestation, &I, &EPOCH, &committee, &qc),
                Ok(())
            );
            let other_epoch = EpochId {
                epoch: EPOCH.epoch + 1,
                ..EPOCH
            };
            assert!(verify_qc(&crypto, &NoAttestation, &I, &other_epoch, &committee, &qc).is_err());
            let other_context = EpochId {
                context: Hash32([0x52; 32]),
                ..EPOCH
            };
            assert!(
                verify_qc(&crypto, &NoAttestation, &I, &other_context, &committee, &qc).is_err()
            );
            let mut rebound = qc.clone();
            rebound.epoch = other_context;
            assert!(
                verify_qc(
                    &crypto,
                    &NoAttestation,
                    &I,
                    &other_context,
                    &committee,
                    &rebound
                )
                .is_err()
            );
            let mut tampered = qc.clone();
            tampered.result = Hash32([9; 32]);
            assert!(verify_qc(&crypto, &NoAttestation, &I, &EPOCH, &committee, &tampered).is_err());
            let unadmitted = BlsCrypto::new();
            assert!(verify_qc(&unadmitted, &NoAttestation, &I, &EPOCH, &committee, &qc).is_err());
        }
    }

    /// A TC whose signers carried PrepareQCs of different views (distinct messages, one group
    /// per view) verifies through `verify_aggregate_multi`.
    #[test]
    fn core_formed_tc_verifies() {
        let (signers, crypto, committee) = validators(4);
        let all: Vec<&KeyPairSigner> = signers.iter().collect();
        let pqc1 = qc(VoteKind::Prepare, 1, &all[..3], &committee);
        let pqc2 = qc(VoteKind::Prepare, 2, &all[1..4], &committee);
        let timeout = |signer: &KeyPairSigner, high_pqc: Option<Qc>| {
            let mut t = TimeoutVote {
                instance: I,
                epoch: EPOCH,
                height: 7,
                view: 4,
                high_pqc,
                signer: index(&committee, signer),
                sig: Signature([0; SIGNATURE_LEN]),
            };
            t.sig = signer.sign(&t.preimage());
            t
        };
        let timeouts = [
            timeout(all[0], None),
            timeout(all[1], Some(pqc1)),
            timeout(all[2], Some(pqc2.clone())),
            timeout(all[3], Some(pqc2)),
        ];
        let refs: Vec<&TimeoutVote> = timeouts.iter().collect();
        let tc = form_tc(&crypto, committee.n(), &refs).unwrap();
        assert_eq!(verify_tc(&crypto, &I, &EPOCH, &committee, &tc), Ok(()));
        let other_context = EpochId {
            context: Hash32([0x52; 32]),
            ..EPOCH
        };
        assert!(verify_tc(&crypto, &I, &other_context, &committee, &tc).is_err());
        let mut rebound = tc.clone();
        rebound.epoch = other_context;
        assert!(verify_tc(&crypto, &I, &other_context, &committee, &rebound).is_err());
        let mut tampered = tc.clone();
        tampered.view = 5;
        assert!(verify_tc(&crypto, &I, &EPOCH, &committee, &tampered).is_err());
        let mut regrouped = tc.clone();
        for entry in &mut regrouped.entries {
            entry.hq = Some(2);
        }
        assert!(
            verify_tc(&crypto, &I, &EPOCH, &committee, &regrouped).is_err(),
            "the signers' groups are bound"
        );
    }
}
