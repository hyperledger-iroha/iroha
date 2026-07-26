//! Lane privacy commitment registry and verification helpers (NX-10).
//!
//! This module defines the canonical commitment types used by Nexus private
//! lanes. Runtime code and SDKs rely on these helpers to validate Merkle roots
//! and zk-SNARK attestations before admitting cross-lane transfers.

use core::{convert::TryFrom, fmt};

use blake3::Hasher;
use iroha_schema::IntoSchema;
#[cfg(feature = "json")]
use norito::derive::{JsonDeserialize, JsonSerialize};
use thiserror::Error;

use crate::{Hash, HashOf, MerkleProof, MerkleTree};

/// Result type returned by the privacy commitment helpers.
pub type Result<T, E = PrivacyError> = core::result::Result<T, E>;

/// Identifier assigned to a registered commitment slot.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    norito::codec::Encode,
    norito::codec::Decode,
    IntoSchema,
)]
pub struct LaneCommitmentId(u16);

impl LaneCommitmentId {
    /// Create a new identifier.
    #[must_use]
    pub const fn new(id: u16) -> Self {
        Self(id)
    }

    /// Return the numeric representation.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl fmt::Display for LaneCommitmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Canonical commitment schemes supported by Nexus private lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitmentScheme {
    /// Merkle tree root enforcing membership proofs.
    Merkle(MerkleCommitment),
    /// zk-SNARK circuit attesting to a redacted state transition.
    Snark(SnarkCircuit),
}

impl CommitmentScheme {
    fn kind(&self) -> CommitmentSchemeKind {
        match self {
            CommitmentScheme::Merkle(_) => CommitmentSchemeKind::Merkle,
            CommitmentScheme::Snark(_) => CommitmentSchemeKind::Snark,
        }
    }
}

/// Kind of scheme embedded in a [`LanePrivacyCommitment`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CommitmentSchemeKind {
    /// Merkle tree root commitment.
    Merkle,
    /// zk-SNARK circuit commitment.
    Snark,
}

/// Runtime witness supplied when validating a commitment.
#[derive(Clone, Debug)]
pub enum PrivacyWitness<'a> {
    /// Membership proof for a Merkle commitment.
    Merkle(MerkleWitness),
    /// zk-SNARK proof bound to a registered circuit.
    Snark(SnarkWitness<'a>),
}

impl PrivacyWitness<'_> {
    fn kind(&self) -> PrivacyWitnessKind {
        match self {
            PrivacyWitness::Merkle(_) => PrivacyWitnessKind::Merkle,
            PrivacyWitness::Snark(_) => PrivacyWitnessKind::Snark,
        }
    }
}

/// Kind of witness supplied during verification.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PrivacyWitnessKind {
    /// Merkle membership proof.
    Merkle,
    /// zk-SNARK proof.
    Snark,
}

/// High-level commitment descriptor stored in the lane registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LanePrivacyCommitment {
    id: LaneCommitmentId,
    scheme: CommitmentScheme,
}

impl LanePrivacyCommitment {
    /// Construct a Merkle-root commitment.
    #[must_use]
    pub const fn merkle(id: LaneCommitmentId, commitment: MerkleCommitment) -> Self {
        Self {
            id,
            scheme: CommitmentScheme::Merkle(commitment),
        }
    }

    /// Construct a zk-SNARK circuit commitment.
    #[must_use]
    pub const fn snark(id: LaneCommitmentId, circuit: SnarkCircuit) -> Self {
        Self {
            id,
            scheme: CommitmentScheme::Snark(circuit),
        }
    }

    /// Return the identifier assigned to this commitment.
    #[must_use]
    pub const fn id(&self) -> LaneCommitmentId {
        self.id
    }

    /// Borrow the embedded commitment scheme.
    #[must_use]
    pub const fn scheme(&self) -> &CommitmentScheme {
        &self.scheme
    }

    /// Verify the provided witness against the stored scheme.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyError::SchemeMismatch`] when the witness kind does not
    /// match the registered commitment, or any verification error raised by the
    /// underlying proof system (`Merkle` or `Snark`).
    pub fn verify(&self, witness: PrivacyWitness<'_>) -> Result<()> {
        match (&self.scheme, witness) {
            (CommitmentScheme::Merkle(commitment), PrivacyWitness::Merkle(w)) => {
                commitment.verify(&w)
            }
            (CommitmentScheme::Snark(circuit), PrivacyWitness::Snark(w)) => circuit.verify(w),
            (scheme, witness) => Err(PrivacyError::SchemeMismatch {
                expected: scheme.kind(),
                actual: witness.kind(),
            }),
        }
    }
}

/// Errors raised when verifying lane commitments.
#[derive(Debug, Error, PartialEq, Eq, Copy, Clone)]
pub enum PrivacyError {
    /// Witness does not match the registered scheme.
    #[error("scheme mismatch (expected {expected:?}, got {actual:?})")]
    SchemeMismatch {
        /// Expected scheme.
        expected: CommitmentSchemeKind,
        /// Witness-kind supplied at runtime.
        actual: PrivacyWitnessKind,
    },
    /// Proof depth exceeds the declared Merkle-tree budget.
    #[error("merkle proof depth {actual} exceeds declared limit {declared}")]
    MerkleProofExceedsDepth {
        /// Depth declared when registering the root.
        declared: u8,
        /// Depth observed at runtime.
        actual: u8,
    },
    /// Merkle proof failed verification against the registered root.
    #[error("merkle proof failed to verify against the registered root")]
    InvalidMerkleProof,
    /// SNARK witness public inputs do not match the recorded trace hash.
    #[error("snark statement hash mismatch")]
    SnarkStatementMismatch,
    /// SNARK proof bytes do not match the expected digest for the circuit.
    #[error("snark proof digest mismatch")]
    SnarkProofDigestMismatch,
}

/// Merkle membership proof bound to a canonical leaf hash.
#[derive(Clone, Debug)]
pub struct MerkleWitness {
    leaf_hash: HashOf<[u8; 32]>,
    proof: MerkleProof<[u8; 32]>,
}

impl MerkleWitness {
    /// Construct a witness from a pre-hashed leaf and proof.
    #[must_use]
    pub fn new(leaf_hash: HashOf<[u8; 32]>, proof: MerkleProof<[u8; 32]>) -> Self {
        Self { leaf_hash, proof }
    }

    /// Hash raw leaf bytes using `Hash::prehashed` before storing them.
    #[must_use]
    pub fn from_leaf_bytes(leaf: [u8; 32], proof: MerkleProof<[u8; 32]>) -> Self {
        let hash = Hash::prehashed(leaf);
        let typed = HashOf::<[u8; 32]>::from_untyped_unchecked(hash);
        Self::new(typed, proof)
    }

    /// Borrow the leaf hash referenced by this witness.
    #[must_use]
    pub const fn leaf_hash(&self) -> &HashOf<[u8; 32]> {
        &self.leaf_hash
    }

    /// Borrow the Merkle proof.
    #[must_use]
    pub const fn proof(&self) -> &MerkleProof<[u8; 32]> {
        &self.proof
    }
}

/// Metadata recorded for registered Merkle roots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MerkleCommitment {
    root: HashOf<MerkleTree<[u8; 32]>>,
    max_depth: u8,
}

impl MerkleCommitment {
    /// Create a new commitment from a canonical root hash.
    #[must_use]
    pub const fn new(root: HashOf<MerkleTree<[u8; 32]>>, max_depth: u8) -> Self {
        Self { root, max_depth }
    }

    /// Convert a raw 32-byte root digest into the typed representation.
    #[must_use]
    pub fn from_root_bytes(root: [u8; 32], max_depth: u8) -> Self {
        let hash = Hash::prehashed(root);
        let typed = HashOf::<MerkleTree<[u8; 32]>>::from_untyped_unchecked(hash);
        Self::new(typed, max_depth)
    }

    /// Borrow the registered root hash.
    #[must_use]
    pub const fn root(&self) -> &HashOf<MerkleTree<[u8; 32]>> {
        &self.root
    }

    /// Maximum depth allowed for membership proofs.
    #[must_use]
    pub const fn max_depth(&self) -> u8 {
        self.max_depth
    }

    fn verify(&self, witness: &MerkleWitness) -> Result<()> {
        let depth = witness.proof.audit_path().len();
        if depth > usize::from(self.max_depth) {
            let actual = u8::try_from(depth).unwrap_or(u8::MAX);
            return Err(PrivacyError::MerkleProofExceedsDepth {
                declared: self.max_depth,
                actual,
            });
        }
        let proof = witness.proof.clone();
        if proof.verify_sha256(
            witness.leaf_hash(),
            self.root(),
            usize::from(self.max_depth),
        ) {
            Ok(())
        } else {
            Err(PrivacyError::InvalidMerkleProof)
        }
    }
}

/// Unique identifier assigned to zk-SNARK circuits.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SnarkCircuitId(u16);

impl SnarkCircuitId {
    /// Construct a new identifier.
    #[must_use]
    pub const fn new(id: u16) -> Self {
        Self(id)
    }

    /// Return the numeric ID.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Metadata recorded for registered zk-SNARK circuits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnarkCircuit {
    circuit_id: SnarkCircuitId,
    verifying_key_digest: [u8; 32],
    statement_hash: [u8; 32],
    proof_hash: [u8; 32],
}

impl SnarkCircuit {
    /// Create a new circuit descriptor.
    #[must_use]
    pub const fn new(
        circuit_id: SnarkCircuitId,
        verifying_key_digest: [u8; 32],
        statement_hash: [u8; 32],
        proof_hash: [u8; 32],
    ) -> Self {
        Self {
            circuit_id,
            verifying_key_digest,
            statement_hash,
            proof_hash,
        }
    }

    /// Return the circuit identifier.
    #[must_use]
    pub const fn circuit_id(&self) -> SnarkCircuitId {
        self.circuit_id
    }

    /// Hash of the verifying key held by the registry.
    #[must_use]
    pub const fn verifying_key_digest(&self) -> &[u8; 32] {
        &self.verifying_key_digest
    }

    /// Hash of the canonical public inputs statement.
    #[must_use]
    pub const fn statement_hash(&self) -> &[u8; 32] {
        &self.statement_hash
    }

    /// Proof digest recorded for the circuit.
    #[must_use]
    pub const fn proof_hash(&self) -> &[u8; 32] {
        &self.proof_hash
    }

    fn verify(&self, witness: SnarkWitness<'_>) -> Result<()> {
        let expected_statement = hash_public_inputs(witness.public_inputs);
        if expected_statement != self.statement_hash {
            return Err(PrivacyError::SnarkStatementMismatch);
        }
        let expected_proof = hash_proof(self.verifying_key_digest(), witness.proof);
        if expected_proof != self.proof_hash {
            return Err(PrivacyError::SnarkProofDigestMismatch);
        }
        Ok(())
    }
}

/// Runtime witness for zk-SNARK circuits.
#[derive(Clone, Copy, Debug)]
pub struct SnarkWitness<'a> {
    /// Canonical encoding of the public inputs.
    pub public_inputs: &'a [u8],
    /// Proof bytes emitted by the zk-SNARK prover.
    pub proof: &'a [u8],
}

/// Hash the canonical public inputs for zk-SNARK circuits.
#[must_use]
pub fn hash_public_inputs(public_inputs: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(public_inputs);
    *hasher.finalize().as_bytes()
}

/// Hash the proof bytes while binding them to the verifying key digest.
#[must_use]
pub fn hash_proof(verifying_key_digest: &[u8; 32], proof_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(verifying_key_digest);
    hasher.update(proof_bytes);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merkle_commitment_accepts_valid_proof() {
        let leaf_bytes = [0x11u8; 32];
        let leaf_hash = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf_bytes));
        let proof = MerkleProof::from_audit_path_bytes(1, vec![[0x22; 32], [0x33; 32]]);
        let root = proof
            .clone()
            .compute_root_sha256(&leaf_hash, 8)
            .expect("valid proof must produce root");
        let commitment =
            LanePrivacyCommitment::merkle(LaneCommitmentId::new(7), MerkleCommitment::new(root, 8));
        let witness = MerkleWitness::new(leaf_hash, proof);
        assert!(commitment.verify(PrivacyWitness::Merkle(witness)).is_ok());
    }

    #[test]
    fn merkle_commitment_rejects_excessive_depth() {
        let leaf_bytes = [0xFFu8; 32];
        let witness = MerkleWitness::from_leaf_bytes(
            leaf_bytes,
            MerkleProof::from_audit_path_bytes(0, vec![[0x44; 32], [0x55; 32], [0x66; 32]]),
        );
        let root = witness
            .proof()
            .clone()
            .compute_root_sha256(witness.leaf_hash(), 8)
            .unwrap();
        let commitment =
            LanePrivacyCommitment::merkle(LaneCommitmentId::new(9), MerkleCommitment::new(root, 2));
        let err = commitment
            .verify(PrivacyWitness::Merkle(witness))
            .expect_err("depth mismatch");
        assert!(matches!(
            err,
            PrivacyError::MerkleProofExceedsDepth {
                declared: 2,
                actual: 3
            }
        ));
    }

    #[test]
    fn snark_commitment_accepts_matching_hashes() {
        let vk_digest = [0xAAu8; 32];
        let public_inputs = b"lane=cbdc;amount=42";
        let proof_bytes = b"demo-proof";
        let circuit = SnarkCircuit::new(
            SnarkCircuitId::new(5),
            vk_digest,
            hash_public_inputs(public_inputs),
            hash_proof(&vk_digest, proof_bytes),
        );
        let commitment = LanePrivacyCommitment::snark(LaneCommitmentId::new(42), circuit);
        let witness = SnarkWitness {
            public_inputs,
            proof: proof_bytes,
        };
        assert!(commitment.verify(PrivacyWitness::Snark(witness)).is_ok());
    }

    #[test]
    fn snark_commitment_rejects_mismatched_statement() {
        let vk_digest = [0x01u8; 32];
        let good_inputs = b"lane=twas;amount=10";
        let proof_bytes = b"proof";
        let circuit = SnarkCircuit::new(
            SnarkCircuitId::new(99),
            vk_digest,
            hash_public_inputs(good_inputs),
            hash_proof(&vk_digest, proof_bytes),
        );
        let commitment = LanePrivacyCommitment::snark(LaneCommitmentId::new(1), circuit);
        let witness = SnarkWitness {
            public_inputs: b"lane=twas;amount=11",
            proof: proof_bytes,
        };
        let err = commitment
            .verify(PrivacyWitness::Snark(witness))
            .expect_err("mismatched inputs");
        assert!(matches!(err, PrivacyError::SnarkStatementMismatch));
    }
}
