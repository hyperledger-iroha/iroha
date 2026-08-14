//! Compact transparent accumulators for first-release private-note pools.
//!
//! Native IVM private notes and PQ-MASP both use an append-only depth-32
//! commitment tree.  The leaf preimage binds the exact typed namespace and
//! commitment; internal nodes bind their level.  Persisted state stores the
//! compact frontier, not a caller-supplied root and not the complete historical
//! leaf set.  Restore therefore reconstructs and rehashes the frontier before
//! any proof can consume it.
//!
//! FCMP++ deliberately does not use this module.  Its canonical accumulator is
//! the Selene/Helios curve tree from the FCMP++ construction.
use std::collections::BTreeSet;
use incrementalmerkletree::{Hashable, Level, Position, frontier::Frontier};
use iroha_data_model::privacy::{
    PrivacyCommitmentV1, PrivacyNamespaceV1, PrivacyProtocolIdV1, PrivacyRootV1,
};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
const TREE_DEPTH_V1: u8 = 32;
const EMPTY_LEAF_DOMAIN_V1: &[u8] = b"iroha.privacy.proof-managed-note-tree.empty-leaf.v1";
const LEAF_DOMAIN_V1: &[u8] = b"iroha.privacy.proof-managed-note-tree.leaf.v1";
const NODE_DOMAIN_V1: &[u8] = b"iroha.privacy.proof-managed-note-tree.node.v1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NoteTreeNodeV1([u8; 32]);
impl Hashable for NoteTreeNodeV1 {
    fn empty_leaf() -> Self {
        let mut hasher = Sha256::new();
        hasher.update(EMPTY_LEAF_DOMAIN_V1);
        Self(hasher.finalize().into())
    }
    fn combine(level: Level, left: &Self, right: &Self) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(NODE_DOMAIN_V1);
        hasher.update([u8::from(level)]);
        hasher.update(left.0);
        hasher.update(right.0);
        Self(hasher.finalize().into())
    }
}
/// Canonical compact representation of an IVM/PQ note frontier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProofManagedFrontierPartsV1 {
    /// Number of leaves already appended.
    pub(crate) tree_size: u64,
    /// Most recently appended leaf hash, absent only for the empty tree.
    pub(crate) leaf: Option<[u8; 32]>,
    /// Past subtree roots required to continue appending.
    pub(crate) ommers: Vec<[u8; 32]>,
    /// Root reconstructed from the complete compact frontier.
    pub(crate) root: PrivacyRootV1,
}
/// Failure while constructing, restoring, or advancing a note frontier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ProofManagedAccumulatorErrorV1 {
    /// The namespace is malformed or belongs to another protocol.
    #[error("proof-managed note accumulator namespace is invalid")]
    Namespace,
    /// Canonical namespace encoding failed.
    #[error("proof-managed note accumulator namespace encoding failed")]
    NamespaceEncoding,
    /// A commitment is the reserved all-zero value.
    #[error("proof-managed note accumulator commitment {index} is zero")]
    ZeroCommitment {
        /// Zero-based commitment index.
        index: usize,
    },
    /// A commitment occurs more than once in the same append batch.
    #[error("proof-managed note accumulator commitment {index} is duplicated")]
    DuplicateCommitment {
        /// Index of the repeated commitment.
        index: usize,
    },
    /// A bootstrap or transition omitted every commitment.
    #[error("proof-managed note accumulator requires at least one commitment")]
    EmptyCommitments,
    /// Empty frontier fields disagree.
    #[error("proof-managed note accumulator has an inconsistent empty frontier")]
    EmptyShape,
    /// The compact frontier parts are not a valid depth-32 shape.
    #[error("proof-managed note accumulator frontier shape is invalid")]
    FrontierShape,
    /// Reconstructed and persisted roots differ.
    #[error("proof-managed note accumulator reconstructed root differs from persisted root")]
    RootMismatch,
    /// Appending would exceed the depth-32 capacity.
    #[error("proof-managed note accumulator tree is full")]
    TreeFull,
}
fn validate_namespace_v1(
    namespace: PrivacyNamespaceV1,
) -> Result<(), ProofManagedAccumulatorErrorV1> {
    namespace
        .validate()
        .map_err(|_| ProofManagedAccumulatorErrorV1::Namespace)?;
    if !matches!(
        namespace.protocol_id(),
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 | PrivacyProtocolIdV1::PqMaspStarkV0
    ) {
        return Err(ProofManagedAccumulatorErrorV1::Namespace);
    }
    Ok(())
}
fn commitment_leaf_v1(
    namespace: PrivacyNamespaceV1,
    commitment: PrivacyCommitmentV1,
    index: usize,
) -> Result<NoteTreeNodeV1, ProofManagedAccumulatorErrorV1> {
    validate_namespace_v1(namespace)?;
    if commitment.is_zero() {
        return Err(ProofManagedAccumulatorErrorV1::ZeroCommitment { index });
    }
    let encoded_namespace = norito::to_bytes(&namespace)
        .map_err(|_| ProofManagedAccumulatorErrorV1::NamespaceEncoding)?;
    let mut hasher = Sha256::new();
    hasher.update(LEAF_DOMAIN_V1);
    hasher.update(
        u64::try_from(encoded_namespace.len())
            .expect("canonical namespace length fits u64")
            .to_be_bytes(),
    );
    hasher.update(encoded_namespace);
    hasher.update(commitment.as_bytes());
    Ok(NoteTreeNodeV1(hasher.finalize().into()))
}
fn restore_frontier_v1(
    tree_size: u64,
    leaf: Option<[u8; 32]>,
    ommers: &[[u8; 32]],
) -> Result<Frontier<NoteTreeNodeV1, TREE_DEPTH_V1>, ProofManagedAccumulatorErrorV1> {
    if tree_size == 0 {
        if leaf.is_some() || !ommers.is_empty() {
            return Err(ProofManagedAccumulatorErrorV1::EmptyShape);
        }
        return Ok(Frontier::empty());
    }
    let leaf = leaf.ok_or(ProofManagedAccumulatorErrorV1::EmptyShape)?;
    Frontier::from_parts(
        Position::from(tree_size - 1),
        NoteTreeNodeV1(leaf),
        ommers.iter().copied().map(NoteTreeNodeV1).collect(),
    )
    .map_err(|_| ProofManagedAccumulatorErrorV1::FrontierShape)
}
fn frontier_parts_v1(
    frontier: Frontier<NoteTreeNodeV1, TREE_DEPTH_V1>,
) -> ProofManagedFrontierPartsV1 {
    let root = PrivacyRootV1::new(frontier.root().0);
    let tree_size = frontier.tree_size();
    let (leaf, ommers) = frontier.take().map_or((None, Vec::new()), |frontier| {
        let (_, leaf, ommers) = frontier.into_parts();
        (
            Some(leaf.0),
            ommers.into_iter().map(|ommer| ommer.0).collect(),
        )
    });
    ProofManagedFrontierPartsV1 {
        tree_size,
        leaf,
        ommers,
        root,
    }
}
/// Return the unique depth-32 authentication path for the sole leaf in a
/// canonical proof-managed frontier.
///
/// The helper is crate-private and exists for non-shipping native release
/// fixtures. Keeping it beside `NoteTreeNodeV1::empty_leaf` prevents those
/// fixtures from carrying a second copy of the consensus empty-node schedule.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn canonical_single_leaf_authentication_path_v1() -> [[u8; 32]; TREE_DEPTH_V1 as usize] {
    let mut empty = NoteTreeNodeV1::empty_leaf();
    core::array::from_fn(|level| {
        let sibling = empty.0;
        empty = NoteTreeNodeV1::combine(
            Level::from(u8::try_from(level).expect("depth-32 level fits u8")),
            &empty,
            &empty,
        );
        sibling
    })
}
/// Construct the unique frontier for a complete ordered genesis set.
///
/// # Errors
///
/// Rejects an empty set, zero commitment, malformed/cross-protocol namespace,
/// or a set that exceeds the depth-32 tree.
pub(crate) fn build_proof_managed_frontier_v1(
    namespace: PrivacyNamespaceV1,
    commitments: &[PrivacyCommitmentV1],
) -> Result<ProofManagedFrontierPartsV1, ProofManagedAccumulatorErrorV1> {
    validate_namespace_v1(namespace)?;
    if commitments.is_empty() {
        return Err(ProofManagedAccumulatorErrorV1::EmptyCommitments);
    }
    let mut frontier = Frontier::empty();
    let mut seen = BTreeSet::new();
    for (index, commitment) in commitments.iter().copied().enumerate() {
        if !seen.insert(commitment) {
            return Err(ProofManagedAccumulatorErrorV1::DuplicateCommitment { index });
        }
        let leaf = commitment_leaf_v1(namespace, commitment, index)?;
        if !frontier.append(leaf) {
            return Err(ProofManagedAccumulatorErrorV1::TreeFull);
        }
    }
    Ok(frontier_parts_v1(frontier))
}
/// Reconstruct and authenticate a persisted compact frontier.
///
/// # Errors
///
/// Rejects a malformed namespace/frontier or a substituted root.
pub(crate) fn validate_proof_managed_frontier_v1(
    namespace: PrivacyNamespaceV1,
    tree_size: u64,
    leaf: Option<[u8; 32]>,
    ommers: &[[u8; 32]],
    expected_root: PrivacyRootV1,
) -> Result<(), ProofManagedAccumulatorErrorV1> {
    validate_namespace_v1(namespace)?;
    let frontier = restore_frontier_v1(tree_size, leaf, ommers)?;
    if PrivacyRootV1::new(frontier.root().0) != expected_root {
        return Err(ProofManagedAccumulatorErrorV1::RootMismatch);
    }
    Ok(())
}
/// Append public output commitments in their exact statement order.
///
/// # Errors
///
/// Rejects a malformed persisted frontier, empty/zero output set, substituted
/// root, or capacity exhaustion.
pub(crate) fn append_proof_managed_commitments_v1(
    namespace: PrivacyNamespaceV1,
    tree_size: u64,
    leaf: Option<[u8; 32]>,
    ommers: &[[u8; 32]],
    expected_root: PrivacyRootV1,
    output_commitments: &[PrivacyCommitmentV1],
) -> Result<ProofManagedFrontierPartsV1, ProofManagedAccumulatorErrorV1> {
    validate_namespace_v1(namespace)?;
    if output_commitments.is_empty() {
        return Err(ProofManagedAccumulatorErrorV1::EmptyCommitments);
    }
    let mut frontier = restore_frontier_v1(tree_size, leaf, ommers)?;
    if PrivacyRootV1::new(frontier.root().0) != expected_root {
        return Err(ProofManagedAccumulatorErrorV1::RootMismatch);
    }
    let mut seen = BTreeSet::new();
    for (index, commitment) in output_commitments.iter().copied().enumerate() {
        if !seen.insert(commitment) {
            return Err(ProofManagedAccumulatorErrorV1::DuplicateCommitment { index });
        }
        let leaf = commitment_leaf_v1(namespace, commitment, index)?;
        if !frontier.append(leaf) {
            return Err(ProofManagedAccumulatorErrorV1::TreeFull);
        }
    }
    Ok(frontier_parts_v1(frontier))
}
#[cfg(test)]
mod tests {
    use iroha_data_model::privacy::{
        PrivacyNamespaceScopeV1, PrivacyPoolIdV1, PrivacyPoolNamespaceV1,
        PrivacyPoolProgramNamespaceV1, PrivacyProgramIdV1,
    };
    use super::*;
    fn commitment(byte: u8) -> PrivacyCommitmentV1 {
        PrivacyCommitmentV1::new([byte; 32])
    }
    fn pq_namespace() -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::PqMaspStarkV0,
            PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                pool_id: PrivacyPoolIdV1::new([0x31; 32]),
            }),
        )
    }
    fn ivm_namespace() -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            PrivacyNamespaceScopeV1::PoolProgram(PrivacyPoolProgramNamespaceV1 {
                pool_id: PrivacyPoolIdV1::new([0x41; 32]),
                program_id: PrivacyProgramIdV1::new([0x42; 32]),
            }),
        )
    }
    #[test]
    fn compact_frontier_round_trips_and_advances_in_order() {
        for (namespace, expected_origin, expected_successor) in [
            (
                pq_namespace(),
                "2bd2034e6ff160bb58284113396dfe267150d8b3457106452b3bbe1183920193",
                "13156e8af8ecf533b342ff01b14f2e4b74d6a6694e874332947a3384adace9a8",
            ),
            (
                ivm_namespace(),
                "b8ae846db865b4f4bb7d03182c281ea9df51f0bcc32d8a493b60ae700bda98e6",
                "5b21783a03f4660f596ad3abaf34cca1601f33d4179af7985a12d9c66b09c000",
            ),
        ] {
            let origin =
                build_proof_managed_frontier_v1(namespace, &[commitment(1), commitment(2)])
                    .expect("canonical origin");
            assert_eq!(origin.tree_size, 2);
            validate_proof_managed_frontier_v1(
                namespace,
                origin.tree_size,
                origin.leaf,
                &origin.ommers,
                origin.root,
            )
            .expect("origin frontier authenticates");
            let successor = append_proof_managed_commitments_v1(
                namespace,
                origin.tree_size,
                origin.leaf,
                &origin.ommers,
                origin.root,
                &[commitment(3), commitment(4)],
            )
            .expect("canonical append");
            assert_eq!(hex::encode(origin.root.as_bytes()), expected_origin);
            assert_eq!(hex::encode(successor.root.as_bytes()), expected_successor);
            assert_eq!(successor.tree_size, 4);
            assert_ne!(successor.root, origin.root);
            validate_proof_managed_frontier_v1(
                namespace,
                successor.tree_size,
                successor.leaf,
                &successor.ommers,
                successor.root,
            )
            .expect("successor frontier authenticates");
            let reversed =
                build_proof_managed_frontier_v1(namespace, &[commitment(2), commitment(1)])
                    .expect("ordered alternate origin");
            assert_ne!(reversed.root, origin.root);
        }
    }
    #[test]
    fn protocol_and_namespace_are_cryptographically_separated() {
        let commitments = [commitment(7), commitment(8)];
        let pq =
            build_proof_managed_frontier_v1(pq_namespace(), &commitments).expect("PQ frontier");
        let ivm =
            build_proof_managed_frontier_v1(ivm_namespace(), &commitments).expect("IVM frontier");
        assert_ne!(pq.root, ivm.root);
        let other_pq = PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::PqMaspStarkV0,
            PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                pool_id: PrivacyPoolIdV1::new([0x32; 32]),
            }),
        );
        let other =
            build_proof_managed_frontier_v1(other_pq, &commitments).expect("other PQ frontier");
        assert_ne!(pq.root, other.root);
    }
    #[test]
    fn malformed_frontiers_and_commitments_fail_closed() {
        let namespace = pq_namespace();
        assert_eq!(
            build_proof_managed_frontier_v1(namespace, &[]),
            Err(ProofManagedAccumulatorErrorV1::EmptyCommitments)
        );
        assert_eq!(
            build_proof_managed_frontier_v1(namespace, &[PrivacyCommitmentV1::new([0; 32])]),
            Err(ProofManagedAccumulatorErrorV1::ZeroCommitment { index: 0 })
        );
        assert_eq!(
            build_proof_managed_frontier_v1(namespace, &[commitment(1), commitment(1)]),
            Err(ProofManagedAccumulatorErrorV1::DuplicateCommitment { index: 1 })
        );
        let origin = build_proof_managed_frontier_v1(namespace, &[commitment(1), commitment(2)])
            .expect("canonical origin");
        let mut wrong_root = origin.root;
        wrong_root.0[0] ^= 1;
        assert_eq!(
            validate_proof_managed_frontier_v1(
                namespace,
                origin.tree_size,
                origin.leaf,
                &origin.ommers,
                wrong_root,
            ),
            Err(ProofManagedAccumulatorErrorV1::RootMismatch)
        );
        assert_eq!(
            validate_proof_managed_frontier_v1(namespace, 0, Some([1; 32]), &[], origin.root),
            Err(ProofManagedAccumulatorErrorV1::EmptyShape)
        );
        assert_eq!(
            validate_proof_managed_frontier_v1(namespace, 1, None, &[], origin.root),
            Err(ProofManagedAccumulatorErrorV1::EmptyShape)
        );
        assert_eq!(
            append_proof_managed_commitments_v1(
                namespace,
                origin.tree_size,
                origin.leaf,
                &origin.ommers,
                origin.root,
                &[],
            ),
            Err(ProofManagedAccumulatorErrorV1::EmptyCommitments)
        );
        assert_eq!(
            append_proof_managed_commitments_v1(
                namespace,
                origin.tree_size,
                origin.leaf,
                &origin.ommers,
                origin.root,
                &[commitment(3), commitment(3)],
            ),
            Err(ProofManagedAccumulatorErrorV1::DuplicateCommitment { index: 1 })
        );
        let mut substituted_predecessor = origin.root;
        substituted_predecessor.0[31] ^= 1;
        assert_eq!(
            append_proof_managed_commitments_v1(
                namespace,
                origin.tree_size,
                origin.leaf,
                &origin.ommers,
                substituted_predecessor,
                &[commitment(3)],
            ),
            Err(ProofManagedAccumulatorErrorV1::RootMismatch)
        );
        let full_tree_size = 1_u64 << TREE_DEPTH_V1;
        let full_leaf = Some([0xA5; 32]);
        let full_ommers = vec![[0x5A; 32]; usize::from(TREE_DEPTH_V1)];
        let full_frontier =
            restore_frontier_v1(full_tree_size, full_leaf, &full_ommers).expect("full frontier");
        let full_root = PrivacyRootV1::new(full_frontier.root().0);
        assert_eq!(
            append_proof_managed_commitments_v1(
                namespace,
                full_tree_size,
                full_leaf,
                &full_ommers,
                full_root,
                &[commitment(9)],
            ),
            Err(ProofManagedAccumulatorErrorV1::TreeFull)
        );
        assert_eq!(
            validate_proof_managed_frontier_v1(
                namespace,
                full_tree_size + 1,
                full_leaf,
                &full_ommers,
                full_root,
            ),
            Err(ProofManagedAccumulatorErrorV1::FrontierShape)
        );
    }
}
