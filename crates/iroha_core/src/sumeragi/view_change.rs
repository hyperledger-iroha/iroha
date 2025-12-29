//! Structures related to proofs and reasons of view changes.
//! Where view change is a process of changing topology due to some faulty network behavior.

use std::collections::{BTreeMap, btree_map::Entry};

use derive_more::Constructor;
use eyre::Result;
use iroha_crypto::{HashOf, PublicKey, SignatureOf};
use iroha_data_model::block::BlockHeader;
use norito::codec::{Decode, Encode};
use norito::core::{self as ncore, DecodeFromSlice};
use thiserror::Error;

use super::network_topology::Topology;

/// The node's public key and its corresponding signature on the view-change proof.
#[derive(Debug, Clone, Decode, Encode, PartialEq, Eq, Hash, Constructor)]
struct ViewChangeProofSignature {
    public_key: PublicKey,
    signature: SignatureOf<ViewChangeProofPayload>,
}

/// Error emerge during insertion of `Proof` into `ProofChain`
#[derive(Error, displaydoc::Display, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Block hash of proof doesn't match hash of proof chain
    BlockHashMismatch,
    /// Peer already have verified view change proof with index larger than received
    ViewChangeOutdated,
    /// Proof failed validation: {0:?}
    InvalidProof(candidate::SignedProofCandidateError),
}

#[derive(Debug, Clone, Decode, Encode, PartialEq, Eq)]
struct ViewChangeProofPayload {
    /// Hash of the latest committed block.
    latest_block: HashOf<BlockHeader>,
    /// Within a round, what is the index of the view change this proof is trying to prove.
    view_change_index: u32,
}

// Provide slice-based decode so Norito can treat this payload as self-delimiting
impl<'a> DecodeFromSlice<'a> for ViewChangeProofPayload {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        // Read a dynamic length header at the start of `bytes`
        let (len, hdr) = ncore::read_len_dyn_slice(bytes)?;
        let start = hdr;
        let end = hdr + len;
        if end > bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let value = ncore::decode_from_bytes::<Self>(&bytes[start..end])?;
        Ok((value, end))
    }
}

/// The proof of a view change. It needs to be signed by f+1 peers for proof to be valid and view change to happen.
#[derive(Debug, Clone, Decode, Encode, PartialEq, Eq)]
pub struct SignedViewChangeProof {
    signatures: Vec<ViewChangeProofSignature>,
    payload: ViewChangeProofPayload,
}

impl<'a> DecodeFromSlice<'a> for SignedViewChangeProof {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let (candidate, used) =
            <candidate::SignedProofCandidate as DecodeFromSlice>::decode_from_slice(bytes)?;
        let proof = candidate
            .validate()
            .map_err(|err| ncore::Error::from(err.to_string()))?;
        Ok((proof, used))
    }
}

// Derive Encode/Decode above for view change types

/// Builder for proofs
#[repr(transparent)]
pub struct ProofBuilder(SignedViewChangeProof);

impl ProofBuilder {
    /// Constructor from index.
    pub fn new(latest_block: HashOf<BlockHeader>, view_change_index: usize) -> Self {
        let view_change_index = view_change_index
            .try_into()
            .expect("INTERNAL BUG: Blockchain height should fit into usize");

        let proof = SignedViewChangeProof {
            payload: ViewChangeProofPayload {
                latest_block,
                view_change_index,
            },
            signatures: Vec::new(),
        };

        Self(proof)
    }

    /// Sign this message with the peer's private key.
    pub fn sign(mut self, key_pair: &iroha_crypto::KeyPair) -> SignedViewChangeProof {
        let signature = SignatureOf::new(key_pair.private_key(), &self.0.payload);
        self.0.signatures = vec![ViewChangeProofSignature::new(
            key_pair.public_key().clone(),
            signature,
        )];
        self.0
    }
}

// Derive Encode/Decode above for view change types

impl SignedViewChangeProof {
    /// Verify the signatures of `other` and add them to this proof.
    /// Return number of new signatures added.
    fn merge_signatures(
        &mut self,
        other: Vec<ViewChangeProofSignature>,
        topology: &Topology,
    ) -> usize {
        use std::collections::BTreeMap;
        // Canonicalize existing signatures into a map keyed by public key, keeping only
        // cryptographically valid signatures from peers present in the topology.
        let mut by_pk: BTreeMap<PublicKey, SignatureOf<ViewChangeProofPayload>> = BTreeMap::new();
        for s in core::mem::take(&mut self.signatures) {
            if topology.position(&s.public_key).is_some()
                && s.signature.verify(&s.public_key, &self.payload).is_ok()
            {
                by_pk.entry(s.public_key).or_insert(s.signature);
            }
        }
        let len_before = by_pk.len();

        // Merge new signatures with the same canonicalization rules; do not overwrite
        // an existing signature for the same public key.
        for s in other {
            if topology.position(&s.public_key).is_some()
                && s.signature.verify(&s.public_key, &self.payload).is_ok()
            {
                by_pk.entry(s.public_key).or_insert(s.signature);
            }
        }

        // Materialize back into the vector form
        self.signatures = by_pk
            .into_iter()
            .map(|(public_key, signature)| ViewChangeProofSignature {
                public_key,
                signature,
            })
            .collect();

        self.signatures.len().saturating_sub(len_before)
    }

    /// Verify if the proof is valid, given the peers in `topology`.
    fn verify(&self, topology: &Topology) -> bool {
        use std::collections::BTreeSet;
        let mut seen: BTreeSet<&PublicKey> = BTreeSet::new();
        let mut valid_unique = 0usize;
        for s in &self.signatures {
            if topology.position(&s.public_key).is_none() {
                continue;
            }
            if !seen.insert(&s.public_key) {
                continue; // duplicate signer does not increase weight
            }
            if s.signature.verify(&s.public_key, &self.payload).is_ok() {
                valid_unique += 1;
            }
        }
        // NOTE: See Whitepaper for the information on this limit.
        valid_unique > topology.max_faults()
    }

    /// Return `true` if the proof already contains a signature from `public_key`.
    pub fn has_signer(&self, public_key: &PublicKey) -> bool {
        self.signatures.iter().any(|s| &s.public_key == public_key)
    }

    /// Latest committed block hash advertised by this proof.
    pub fn latest_block(&self) -> HashOf<BlockHeader> {
        self.payload.latest_block
    }

    /// View-change index proven by this payload.
    #[allow(clippy::cast_possible_truncation)]
    pub fn view_change_index(&self) -> usize {
        self.payload.view_change_index as usize
    }

    fn validate_signatures(&self) -> Result<(), candidate::SignedProofCandidateError> {
        candidate::SignedProofCandidate::from_proof(self)
            .validate()
            .map(|_| ())
    }
}

/// Maximum number of view-change proofs retained for the latest block.
const MAX_VIEW_CHANGE_PROOFS_PER_BLOCK: usize = 128;

/// Structure representing view change proofs collected by the peer.
/// All proofs are attributed to the same block.
#[derive(Debug, Clone, Default)]
pub struct ProofChain(BTreeMap<u32, SignedViewChangeProof>);

impl ProofChain {
    /// Find next index to last verified view change proof.
    /// Proof is verified if it has more or qual ot f + 1 valid signatures.
    pub fn verify_with_state(
        &self,
        topology: &Topology,
        latest_block: HashOf<BlockHeader>,
    ) -> usize {
        self.0
            .iter()
            .rev()
            .filter(|(_, proof)| proof.payload.latest_block == latest_block)
            .find(|(_, proof)| proof.verify(topology))
            .map_or(0, |(view_change_index, _)| {
                (*view_change_index as usize) + 1
            })
    }

    /// Prune proofs leave only proofs for specified latest block
    pub fn prune(&mut self, latest_block: HashOf<BlockHeader>) {
        self.0
            .retain(|_, proof| proof.payload.latest_block == latest_block)
    }

    /// Attempt to insert a view chain proof into this `ProofChain`.
    ///
    /// # Errors
    /// - If proof latest block hash doesn't match peer latest block hash
    /// - If proof view change number lower than current verified view change
    pub fn insert_proof(
        &mut self,
        new_proof: SignedViewChangeProof,
        topology: &Topology,
        latest_block: HashOf<BlockHeader>,
    ) -> Result<(), Error> {
        if new_proof.payload.latest_block != latest_block {
            return Err(Error::BlockHashMismatch);
        }
        new_proof
            .validate_signatures()
            .map_err(Error::InvalidProof)?;
        let next_unfinished_view_change = self.verify_with_state(topology, latest_block);
        let new_proof_view_change_index = new_proof.payload.view_change_index as usize;
        if new_proof_view_change_index + 1 < next_unfinished_view_change {
            return Err(Error::ViewChangeOutdated); // We only care about current proof and proof which might happen in the future
        }
        if new_proof_view_change_index + 1 == next_unfinished_view_change {
            return Ok(()); // Received a proof for already verified latest proof, not an error just nothing to do about
        }

        let mut sanitized = new_proof;
        // Retain only valid unique signatures from peers in the active topology.
        sanitized.merge_signatures(Vec::new(), topology);
        if sanitized.signatures.is_empty() {
            // No signatures from the active topology; treat as invalid to avoid storing junk.
            return Err(Error::InvalidProof(
                candidate::SignedProofCandidateError::ProofMissingSignatures,
            ));
        }

        match self.0.entry(sanitized.payload.view_change_index) {
            Entry::Occupied(mut occupied) => {
                occupied
                    .get_mut()
                    .merge_signatures(sanitized.signatures, topology);
            }
            Entry::Vacant(vacant) => {
                vacant.insert(sanitized);
            }
        }

        // Drop any proofs that are strictly older than the latest verified proof.
        let verified_next = self.verify_with_state(topology, latest_block);
        if verified_next > 0 {
            let min_index = verified_next.saturating_sub(1);
            if let Ok(min_index_u32) = u32::try_from(min_index) {
                self.0.retain(|idx, _| *idx >= min_index_u32);
            }
        }

        if self.0.len() > MAX_VIEW_CHANGE_PROOFS_PER_BLOCK {
            let keep_index = verified_next
                .checked_sub(1)
                .and_then(|idx| u32::try_from(idx).ok());
            let keys: Vec<u32> = self.0.keys().copied().collect();
            for key in keys.iter().copied() {
                if self.0.len() <= MAX_VIEW_CHANGE_PROOFS_PER_BLOCK {
                    break;
                }
                if Some(key) == keep_index {
                    continue;
                }
                self.0.remove(&key);
            }
            if self.0.len() > MAX_VIEW_CHANGE_PROOFS_PER_BLOCK {
                for key in keys.iter().rev().copied() {
                    if self.0.len() <= MAX_VIEW_CHANGE_PROOFS_PER_BLOCK {
                        break;
                    }
                    if Some(key) == keep_index {
                        continue;
                    }
                    self.0.remove(&key);
                }
            }
        }

        Ok(())
    }

    /// Get proof for requested view change index
    pub fn get_proof_for_view_change(
        &self,
        view_change_index: usize,
    ) -> Option<SignedViewChangeProof> {
        #[allow(clippy::cast_possible_truncation)]
        // Was created from u32 so should be able to cast back
        self.0.get(&(view_change_index as u32)).cloned()
    }
}

mod candidate {
    use core::fmt;
    use indexmap::IndexSet;

    use super::*;

    #[derive(Decode)]
    #[norito(decode_from_slice)]
    pub(super) struct SignedProofCandidate {
        signatures: Vec<ViewChangeProofSignature>,
        payload: ViewChangeProofPayload,
    }

    /// Errors that may occur while validating [`SignedProofCandidate`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SignedProofCandidateError {
        /// Proof has no signatures.
        ProofMissingSignatures,
        /// Proof contains duplicate signatures.
        DuplicateSignature,
        /// Proof contains an invalid signature.
        InvalidSignature,
    }

    impl fmt::Display for SignedProofCandidateError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(match self {
                SignedProofCandidateError::ProofMissingSignatures => "Proof missing signatures",
                SignedProofCandidateError::DuplicateSignature => "Duplicate signature",
                SignedProofCandidateError::InvalidSignature => "Invalid signature",
            })
        }
    }

    impl From<SignedProofCandidateError> for norito::codec::Error {
        fn from(err: SignedProofCandidateError) -> Self {
            norito::codec::Error::from(err.to_string())
        }
    }

    impl SignedProofCandidate {
        pub(super) fn from_proof(proof: &SignedViewChangeProof) -> Self {
            Self {
                signatures: proof.signatures.clone(),
                payload: proof.payload.clone(),
            }
        }

        pub(super) fn validate(self) -> Result<SignedViewChangeProof, SignedProofCandidateError> {
            self.validate_signatures()?;

            Ok(SignedViewChangeProof {
                signatures: self.signatures,
                payload: self.payload,
            })
        }

        fn validate_signatures(&self) -> Result<(), SignedProofCandidateError> {
            if self.signatures.is_empty() {
                return Err(SignedProofCandidateError::ProofMissingSignatures);
            }

            self.signatures
                .iter()
                .map(|signature| &signature.public_key)
                .try_fold(IndexSet::new(), |mut acc, elem| {
                    if !acc.insert(elem) {
                        return Err(SignedProofCandidateError::DuplicateSignature);
                    }

                    Ok(acc)
                })?;

            self.signatures.iter().try_for_each(|signature| {
                signature
                    .signature
                    .verify(&signature.public_key, &self.payload)
                    .map_err(|_| SignedProofCandidateError::InvalidSignature)
            })?;

            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use iroha_crypto::{Hash, HashOf, KeyPair, SignatureOf};

        #[test]
        fn missing_signatures_is_error() {
            let key_pair = KeyPair::random();
            let payload = ViewChangeProofPayload {
                latest_block: HashOf::from_untyped_unchecked(Hash::prehashed([0; 32])),
                view_change_index: 0,
            };

            let candidate = SignedProofCandidate {
                signatures: Vec::new(),
                payload,
            };

            assert_eq!(
                candidate.validate(),
                Err(SignedProofCandidateError::ProofMissingSignatures)
            );

            // also ensure invalid signature is detected
            let payload = ViewChangeProofPayload {
                latest_block: HashOf::from_untyped_unchecked(Hash::prehashed([0; 32])),
                view_change_index: 1,
            };
            let signatures = vec![ViewChangeProofSignature::new(
                key_pair.public_key().clone(),
                SignatureOf::new(key_pair.private_key(), &payload),
            )];

            let candidate = SignedProofCandidate {
                signatures,
                payload,
            };

            assert!(candidate.validate().is_ok());
        }
    }
}

// NoritoDeserialize is derived via `Decode` above

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, HashOf, KeyPair, SignatureOf};
    use norito::{
        codec::Encode,
        core::{DecodeFromSlice, NoritoDeserialize, from_bytes, to_bytes},
    };

    use super::*;
    use crate::sumeragi::network_topology::test_topology_with_keys;

    fn key_pairs<const N: usize>() -> [KeyPair; N] {
        [(); N].map(|()| KeyPair::random())
    }

    fn prepare_data<const N: usize>() -> ([KeyPair; N], Topology, HashOf<BlockHeader>) {
        let key_pairs = key_pairs::<N>();
        let topology = test_topology_with_keys(&key_pairs);
        let latest_block = HashOf::from_untyped_unchecked(Hash::prehashed([0; 32]));

        (key_pairs, topology, latest_block)
    }

    fn create_signed_payload(
        payload: ViewChangeProofPayload,
        signatories: &[KeyPair],
    ) -> SignedViewChangeProof {
        let signatures = signatories
            .iter()
            .map(|key_pair| {
                ViewChangeProofSignature::new(
                    key_pair.public_key().clone(),
                    SignatureOf::new(key_pair.private_key(), &payload),
                )
            })
            .collect();
        SignedViewChangeProof {
            signatures,
            payload,
        }
    }

    #[test]
    fn insert_proof_rejects_missing_signatures() {
        let (key_pairs, topology, latest_block) = prepare_data::<3>();
        let mut chain = ProofChain::default();
        let proof = SignedViewChangeProof {
            signatures: Vec::new(),
            payload: ViewChangeProofPayload {
                latest_block,
                view_change_index: 0,
            },
        };
        let err = chain
            .insert_proof(proof, &topology, latest_block)
            .expect_err("proof without signatures must be rejected");
        assert!(matches!(
            err,
            Error::InvalidProof(candidate::SignedProofCandidateError::ProofMissingSignatures)
        ));

        // Valid proof should still succeed after rejection.
        let payload = ViewChangeProofPayload {
            latest_block,
            view_change_index: 0,
        };
        let signed = create_signed_payload(payload, &key_pairs[..2]);
        assert!(chain.insert_proof(signed, &topology, latest_block).is_ok());
    }

    #[test]
    fn insert_proof_rejects_non_topology_signers() {
        let (_key_pairs, topology, latest_block) = prepare_data::<3>();
        let outsider = vec![KeyPair::random()];
        let payload = ViewChangeProofPayload {
            latest_block,
            view_change_index: 0,
        };
        let proof = create_signed_payload(payload, &outsider);

        let mut chain = ProofChain::default();
        let err = chain
            .insert_proof(proof, &topology, latest_block)
            .expect_err("proof without topology signers must be rejected");
        assert!(matches!(
            err,
            Error::InvalidProof(candidate::SignedProofCandidateError::ProofMissingSignatures)
        ));
        assert_eq!(chain.verify_with_state(&topology, latest_block), 0);
    }

    #[test]
    fn signed_view_change_proof_norito_roundtrip() {
        let (key_pairs, _topology, latest_block) = prepare_data::<4>();
        let proof = ProofBuilder::new(latest_block, 0).sign(&key_pairs[0]);
        let bytes = to_bytes(&proof).expect("encode with header");
        let archived = from_bytes::<SignedViewChangeProof>(&bytes).expect("from_bytes");
        let decoded = <SignedViewChangeProof as NoritoDeserialize>::deserialize(archived);
        assert_eq!(decoded, proof);
    }

    #[test]
    fn decode_from_slice_roundtrip_valid_proof() {
        let key_pair = KeyPair::random();
        let payload = ViewChangeProofPayload {
            latest_block: HashOf::from_untyped_unchecked(Hash::prehashed([1; 32])),
            view_change_index: 2,
        };
        let proof = SignedViewChangeProof {
            signatures: vec![ViewChangeProofSignature::new(
                key_pair.public_key().clone(),
                SignatureOf::new(key_pair.private_key(), &payload),
            )],
            payload,
        };

        let bytes = proof.encode();
        let (decoded, used) =
            SignedViewChangeProof::decode_from_slice(&bytes).expect("valid proof must decode");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, proof);
    }

    #[test]
    fn decode_from_slice_rejects_missing_signatures() {
        let payload = ViewChangeProofPayload {
            latest_block: HashOf::from_untyped_unchecked(Hash::prehashed([2; 32])),
            view_change_index: 3,
        };
        let proof = SignedViewChangeProof {
            signatures: Vec::new(),
            payload,
        };

        let bytes = proof.encode();
        let err = SignedViewChangeProof::decode_from_slice(&bytes)
            .expect_err("missing signatures must be rejected");
        assert!(
            matches!(err, ncore::Error::Message(msg) if msg.contains("Proof missing signatures"))
        );
    }

    #[test]
    fn decode_from_slice_rejects_duplicate_signatures() {
        let key_pair = KeyPair::random();
        let payload = ViewChangeProofPayload {
            latest_block: HashOf::from_untyped_unchecked(Hash::prehashed([3; 32])),
            view_change_index: 4,
        };
        let signature = SignatureOf::new(key_pair.private_key(), &payload);
        let proof = SignedViewChangeProof {
            signatures: vec![
                ViewChangeProofSignature::new(key_pair.public_key().clone(), signature.clone()),
                ViewChangeProofSignature::new(key_pair.public_key().clone(), signature),
            ],
            payload,
        };

        let bytes = proof.encode();
        let err = SignedViewChangeProof::decode_from_slice(&bytes)
            .expect_err("duplicate signer must be rejected");
        assert!(matches!(err, ncore::Error::Message(msg) if msg.contains("Duplicate signature")));
    }

    #[test]
    fn decode_from_slice_rejects_invalid_signature() {
        let key_pair = KeyPair::random();
        let payload = ViewChangeProofPayload {
            latest_block: HashOf::from_untyped_unchecked(Hash::prehashed([4; 32])),
            view_change_index: 5,
        };
        let mismatched_payload = ViewChangeProofPayload {
            latest_block: HashOf::from_untyped_unchecked(Hash::prehashed([5; 32])),
            view_change_index: 6,
        };
        let proof = SignedViewChangeProof {
            signatures: vec![ViewChangeProofSignature::new(
                key_pair.public_key().clone(),
                SignatureOf::new(key_pair.private_key(), &mismatched_payload),
            )],
            payload,
        };

        let bytes = proof.encode();
        let err = SignedViewChangeProof::decode_from_slice(&bytes)
            .expect_err("invalid signature must be rejected");
        assert!(matches!(err, ncore::Error::Message(msg) if msg.contains("Invalid signature")));
    }

    #[test]
    fn verify_with_state_on_empty() {
        let (_key_pairs, topology, latest_block) = prepare_data::<10>();
        let chain = ProofChain::default();

        assert_eq!(chain.verify_with_state(&topology, latest_block), 0);
    }

    #[test]
    fn verify_with_state() {
        let (key_pairs, topology, latest_block) = prepare_data::<10>();

        let len = 10;

        let mut view_change_payloads = (0..).map(|view_change_index| ViewChangeProofPayload {
            latest_block,
            view_change_index,
        });

        let complete_proofs = (&mut view_change_payloads)
            .take(len)
            .map(|payload| create_signed_payload(payload, &key_pairs[..=topology.max_faults()]))
            .collect::<Vec<_>>();

        let incomplete_proofs = (&mut view_change_payloads)
            .take(len)
            .map(|payload| create_signed_payload(payload, &key_pairs[..1]))
            .collect::<Vec<_>>();

        let proofs = {
            let mut proofs = complete_proofs;
            proofs.extend(incomplete_proofs);
            proofs
        };
        let chain = ProofChain(
            proofs
                .clone()
                .into_iter()
                .map(|proof| (proof.payload.view_change_index, proof))
                .collect(),
        );

        // verify_with_state equal to view_change_index of last verified proof plus 1
        assert_eq!(chain.verify_with_state(&topology, latest_block), len);

        // Add complete proofs on top to check that verified view change is updated as well
        let complete_proofs = (&mut view_change_payloads)
            .take(len)
            .map(|payload| create_signed_payload(payload, &key_pairs[..=topology.max_faults()]))
            .collect::<Vec<_>>();

        let proofs = {
            let mut proofs = proofs;
            proofs.extend(complete_proofs);
            proofs
        };
        let chain = ProofChain(
            proofs
                .clone()
                .into_iter()
                .map(|proof| (proof.payload.view_change_index, proof))
                .collect(),
        );
        assert_eq!(chain.verify_with_state(&topology, latest_block), 3 * len);
    }

    #[test]
    fn proof_for_invalid_block_is_rejected() {
        let (key_pairs, topology, latest_block) = prepare_data::<10>();

        let wrong_latest_block = HashOf::from_untyped_unchecked(Hash::prehashed([1; 32]));

        let mut me = ProofChain::default();
        let other = ProofBuilder::new(wrong_latest_block, 0).sign(&key_pairs[1]);

        assert_eq!(
            me.insert_proof(other, &topology, latest_block),
            Err(Error::BlockHashMismatch)
        );
    }

    #[test]
    fn proof_from_the_past_is_rejected() {
        let (key_pairs, topology, latest_block) = prepare_data::<10>();

        let mut chain = ProofChain::default();

        let proof_future = create_signed_payload(
            ViewChangeProofPayload {
                latest_block,
                view_change_index: 10,
            },
            &key_pairs,
        );

        assert_eq!(
            Ok(()),
            chain.insert_proof(proof_future, &topology, latest_block)
        );
        assert_eq!(chain.verify_with_state(&topology, latest_block), 11);

        let proof = create_signed_payload(
            ViewChangeProofPayload {
                latest_block,
                view_change_index: 1,
            },
            &key_pairs,
        );

        assert_eq!(
            Err(Error::ViewChangeOutdated),
            chain.insert_proof(proof, &topology, latest_block)
        );
        assert_eq!(chain.verify_with_state(&topology, latest_block), 11);
    }

    #[test]
    fn proofs_are_merged() {
        let (key_pairs, topology, latest_block) = prepare_data::<10>();

        let mut chain = ProofChain::default();

        let (from, to) = (topology.max_faults() / 2, topology.max_faults() + 1);
        let payload = ViewChangeProofPayload {
            latest_block,
            view_change_index: 0,
        };

        let proof_0_part_1 = create_signed_payload(payload.clone(), &key_pairs[..from]);

        assert_eq!(
            Ok(()),
            chain.insert_proof(proof_0_part_1, &topology, latest_block)
        );
        assert_eq!(chain.verify_with_state(&topology, latest_block), 0);

        let proof_0_part_2 = create_signed_payload(payload, &key_pairs[from..to]);

        assert_eq!(
            Ok(()),
            chain.insert_proof(proof_0_part_2, &topology, latest_block)
        );
        assert_eq!(chain.verify_with_state(&topology, latest_block), 1);
    }

    #[test]
    fn proofs_are_appended() {
        let (key_pairs, topology, latest_block) = prepare_data::<10>();

        let mut chain = ProofChain::default();

        let proof_0 = create_signed_payload(
            ViewChangeProofPayload {
                latest_block,
                view_change_index: 0,
            },
            &key_pairs,
        );

        assert_eq!(Ok(()), chain.insert_proof(proof_0, &topology, latest_block));
        assert_eq!(chain.verify_with_state(&topology, latest_block), 1);

        let proof_1 = create_signed_payload(
            ViewChangeProofPayload {
                latest_block,
                view_change_index: 1,
            },
            &key_pairs,
        );

        assert_eq!(Ok(()), chain.insert_proof(proof_1, &topology, latest_block));
        assert_eq!(chain.verify_with_state(&topology, latest_block), 2);
    }

    #[test]
    fn proof_chain_caps_entries() {
        let (key_pairs, topology, latest_block) = prepare_data::<10>();
        let mut chain = ProofChain::default();
        let signers = &key_pairs[..=topology.max_faults()];
        let total = MAX_VIEW_CHANGE_PROOFS_PER_BLOCK + 8;

        for idx in 0..total {
            let payload = ViewChangeProofPayload {
                latest_block,
                view_change_index: u32::try_from(idx).expect("test index fits u32"),
            };
            let proof = create_signed_payload(payload, signers);
            assert_eq!(Ok(()), chain.insert_proof(proof, &topology, latest_block));
        }

        assert!(
            chain.0.len() <= MAX_VIEW_CHANGE_PROOFS_PER_BLOCK,
            "proof chain must stay within cap"
        );
        assert_eq!(chain.verify_with_state(&topology, latest_block), total);
    }

    #[test]
    fn duplicate_signers_rejected() {
        // Topology with N=7 => f = 2; need > f => 3 unique valid signatures
        let (key_pairs, topology, latest_block) = prepare_data::<7>();

        let payload = ViewChangeProofPayload {
            latest_block,
            view_change_index: 0,
        };
        // Two unique valid signatures (<= f)
        let sig0 = ViewChangeProofSignature::new(
            key_pairs[0].public_key().clone(),
            SignatureOf::new(key_pairs[0].private_key(), &payload),
        );
        let sig1 = ViewChangeProofSignature::new(
            key_pairs[1].public_key().clone(),
            SignatureOf::new(key_pairs[1].private_key(), &payload),
        );
        // Craft proof with duplicates of the same signers
        let proof = SignedViewChangeProof {
            signatures: vec![sig0.clone(), sig1.clone(), sig0.clone(), sig1.clone()],
            payload: payload.clone(),
        };

        let mut chain = ProofChain::default();
        let err = chain
            .insert_proof(proof, &topology, latest_block)
            .expect_err("duplicate signatures must be rejected");
        assert!(matches!(
            err,
            Error::InvalidProof(candidate::SignedProofCandidateError::DuplicateSignature)
        ));
        assert_eq!(chain.verify_with_state(&topology, latest_block), 0);

        // Unique proof with f+1 distinct signatures succeeds.
        let sig2 = ViewChangeProofSignature::new(
            key_pairs[2].public_key().clone(),
            SignatureOf::new(key_pairs[2].private_key(), &payload),
        );
        let unique_proof = SignedViewChangeProof {
            signatures: vec![sig0, sig1, sig2],
            payload,
        };
        assert_eq!(
            Ok(()),
            chain.insert_proof(unique_proof, &topology, latest_block)
        );
        assert_eq!(chain.verify_with_state(&topology, latest_block), 1);
    }

    #[test]
    fn invalid_signatures_rejected() {
        // Topology with N=7 => f = 2
        let (key_pairs, topology, latest_block) = prepare_data::<7>();

        // Correct payload for view 0
        let payload_ok = ViewChangeProofPayload {
            latest_block,
            view_change_index: 0,
        };
        // Wrong payload to produce a non-matching signature
        let payload_wrong = ViewChangeProofPayload {
            latest_block,
            view_change_index: 1,
        };
        let bad_sig0 = ViewChangeProofSignature::new(
            key_pairs[0].public_key().clone(),
            SignatureOf::new(key_pairs[0].private_key(), &payload_wrong),
        );
        let bad_proof = SignedViewChangeProof {
            signatures: vec![bad_sig0],
            payload: payload_ok.clone(),
        };
        let mut chain = ProofChain::default();
        let err = chain
            .insert_proof(bad_proof, &topology, latest_block)
            .expect_err("invalid signatures must be rejected");
        assert!(matches!(
            err,
            Error::InvalidProof(candidate::SignedProofCandidateError::InvalidSignature)
        ));
        // No progress after rejection
        assert_eq!(chain.verify_with_state(&topology, latest_block), 0);

        // Now add f+1 valid unique signatures for the same payload
        let good_sig0 = ViewChangeProofSignature::new(
            key_pairs[0].public_key().clone(),
            SignatureOf::new(key_pairs[0].private_key(), &payload_ok),
        );
        let good_sig1 = ViewChangeProofSignature::new(
            key_pairs[1].public_key().clone(),
            SignatureOf::new(key_pairs[1].private_key(), &payload_ok),
        );
        let good_sig2 = ViewChangeProofSignature::new(
            key_pairs[2].public_key().clone(),
            SignatureOf::new(key_pairs[2].private_key(), &payload_ok),
        );
        let proof = SignedViewChangeProof {
            signatures: vec![good_sig0, good_sig1, good_sig2],
            payload: payload_ok,
        };
        assert_eq!(Ok(()), chain.insert_proof(proof, &topology, latest_block));
        assert_eq!(chain.verify_with_state(&topology, latest_block), 1);
    }
}
