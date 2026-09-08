//! Full-width SMT child selection and path/root linkage numerators.
//!
//! Each active level selects the same 256-bit sibling for the old and new
//! balance paths. Direction zero puts the changing child on the left; direction
//! one puts it on the right, matching `TransferMerkleProof::compute_root`.
//! Hash input/output ports contain every bit, including Iroha's marker bit 248.
//! No complete word or hash is reduced to a single Goldilocks element.
//!
//! # Required surrounding proof contract
//!
//! * Commit all columns before sampling independent Fp4 composition challenges,
//!   prove their degree bounds, and apply the all-row zerofier to
//!   [`level_residues`]. Each digest bit satisfies `b * (b - active) = 0`, and
//!   all 11 digest marker bits equal `active`. Active hashes are canonically
//!   marked, while inactive levels and their hash ports are zero. Direction is activated
//!   Boolean and equals the corresponding externally allocated path bit.
//! * The fixed/authenticated schedule contains exactly 32 levels per active
//!   update, in least-significant-path-bit order. It determines all active,
//!   interior-link, leaf-boundary, root-boundary and update-chain selectors.
//!   Booleanity alone does not authenticate that schedule or its cardinality.
//!   [`level_transition_residues`] applies to adjacent levels within an update
//!   with the final trace row excluded by its transition zerofier. It must not
//!   cyclically link the final row to the first. The boundary/link helpers apply
//!   on their declared fixed domains, using the corresponding boundary zerofier
//!   (or a fixed-domain selector with the all-row zerofier). Their selectors are
//!   not free witness choices, and their input bits require the local/upstream
//!   Boolean constraints. Empty batches require their separate unchanged-root
//!   acceptance relation.
//! * Both [`SmtNodeHashIo`] ports must be equality-linked to separately proved
//!   single-block BLAKE2b-256 invocations with the exact message
//!   `b"fastpq:v1:smt:node|" || left[32] || right[32]`, digest-length IV,
//!   exact counter/finalization, zero block padding and Iroha output conversion.
//!   Selected children must be those hash message bytes; a correct hash of an
//!   unrelated message is insufficient. Leaf hashes need their own full key,
//!   value and leaf-domain hash relations before the initial-leaf boundary.
//! * The allocated path bit must come from the complete canonical allocation in
//!   `transfer.rs`: full unique balance keys in lexicographic order, base paths
//!   from `path_index`, and bounded wrapping linear probing for collisions over
//!   the same complete key set. Merely taking 32 key-hash bits is insufficient.
//!   Sibling ports and witness roots must already have their marker set; never
//!   normalize an unmarked witness through `Hash::prehashed`. The native witness
//!   decoder rejects that alias. Exact key encodings, update order and repeated-key
//!   sequencing also require independent statement/trace bindings.
//! * First/last update roots must bind to independently authenticated public
//!   roots. The touched-balance tree is not automatically a consensus state root.
//!
//! TODO: Integrate the complete hash, allocation, schedule and source-anchoring
//! proofs before replacing replay. This module has no production admission or
//! schema call site. Its bounded wide relation includes 2,819 field inputs per
//! level; it is not a proposal to raise the existing 512-column production limit.

use super::{
    iroha_hash_output_air::{DIGEST_BITS, IrohaHashOutput, MARKER_BIT},
    transfer::TRANSFER_MERKLE_HEIGHT,
    transfer_integer_air::IntegerAirField,
};

/// Exact level count required by the current transfer SMT schedule.
pub const PATH_LEVELS: usize = TRANSFER_MERKLE_HEIGHT;
/// Maximum degree of every local, transition and boundary numerator.
pub const MAX_CONSTRAINT_DEGREE: usize = 2;
/// Direction plus five complete digest witnesses.
pub const LEVEL_WITNESS_FIELD_COUNT: usize = 1 + 5 * DIGEST_BITS;
/// Two full children and a full output for one separately proved node hash.
pub const NODE_HASH_IO_FIELD_COUNT: usize = 3 * DIGEST_BITS;
/// Complete field inputs: level, both hash ports, active and allocated path bit.
pub const LEVEL_INPUT_FIELD_COUNT: usize =
    LEVEL_WITNESS_FIELD_COUNT + 2 * NODE_HASH_IO_FIELD_COUNT + 2;
/// Selector/direction, 11 marker and 17 per-bit Boolean/selection/link relations.
pub const LEVEL_CONSTRAINT_COUNT: usize = 3 + 11 + 17 * DIGEST_BITS;
/// Selector Booleanity and full old/new digest links at a paired boundary.
pub const PAIRED_LINK_CONSTRAINT_COUNT: usize = 1 + 2 * DIGEST_BITS;
/// Selector Booleanity and one complete consecutive-update root link.
pub const UPDATE_CHAIN_CONSTRAINT_COUNT: usize = 1 + DIGEST_BITS;

/// Opened ports of one separately proved, correctly framed SMT node hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SmtNodeHashIo<F = u64> {
    /// Complete first child in the node-domain message.
    pub left: IrohaHashOutput<F>,
    /// Complete second child in the same node-domain message.
    pub right: IrohaHashOutput<F>,
    /// Complete Iroha-encoded BLAKE2b-256 output for that message.
    pub output: IrohaHashOutput<F>,
}

impl<F: IntegerAirField> SmtNodeHashIo<F> {
    /// Canonical zero ports for an inactive invocation.
    #[must_use]
    pub fn inactive() -> Self {
        Self {
            left: IrohaHashOutput::inactive(),
            right: IrohaHashOutput::inactive(),
            output: IrohaHashOutput::inactive(),
        }
    }
}

/// One level of the old/new paths for the same exact balance update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SmtPathLevelWitness<F = u64> {
    /// Zero for child-left, one for child-right; zero on inactive rows.
    pub direction: F,
    /// Changing subtree immediately before the balance update.
    pub old_child: IrohaHashOutput<F>,
    /// Changing subtree immediately after the balance update.
    pub new_child: IrohaHashOutput<F>,
    /// One complete sibling shared by both old/new paths at this level.
    pub sibling: IrohaHashOutput<F>,
    /// Old-path parent, linked to the old node-hash output.
    pub old_parent: IrohaHashOutput<F>,
    /// New-path parent, linked to the new node-hash output.
    pub new_parent: IrohaHashOutput<F>,
}

impl<F: IntegerAirField> SmtPathLevelWitness<F> {
    /// Canonical zero level for inactive schedule rows.
    #[must_use]
    pub fn inactive() -> Self {
        Self {
            direction: F::ZERO,
            old_child: IrohaHashOutput::inactive(),
            new_child: IrohaHashOutput::inactive(),
            sibling: IrohaHashOutput::inactive(),
            old_parent: IrohaHashOutput::inactive(),
            new_parent: IrohaHashOutput::inactive(),
        }
    }
}

/// Complete before/after root ports for one canonically ordered update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SmtUpdateRoots<F = u64> {
    /// Root immediately before the update.
    pub before: IrohaHashOutput<F>,
    /// Root immediately after the update.
    pub after: IrohaHashOutput<F>,
}

fn digest_bit<F: Copy>(digest: &IrohaHashOutput<F>, bit: usize) -> F {
    digest.words[bit / 64].bits[bit % 64]
}

fn level_bit_inputs<F: Copy>(
    level: &SmtPathLevelWitness<F>,
    old_hash: &SmtNodeHashIo<F>,
    new_hash: &SmtNodeHashIo<F>,
    bit: usize,
) -> [F; 11] {
    [
        &level.old_child,
        &level.new_child,
        &level.sibling,
        &level.old_parent,
        &level.new_parent,
        &old_hash.left,
        &old_hash.right,
        &old_hash.output,
        &new_hash.left,
        &new_hash.right,
        &new_hash.output,
    ]
    .map(|digest| digest_bit(digest, bit))
}

fn selector_residues<F: IntegerAirField>(active: F, direction: F, allocated_bit: F) -> [F; 3] {
    [
        active.mul(active.sub(F::ONE)),
        direction.mul(direction.sub(active)),
        direction.sub(allocated_bit),
    ]
}

fn marker_residues<F: IntegerAirField>(active: F, marker_bits: [F; 11]) -> [F; 11] {
    marker_bits.map(|bit| bit.sub(active))
}

fn level_bit_residues<F: IntegerAirField>(active: F, direction: F, bits: [F; 11]) -> [F; 17] {
    let [
        old_child,
        new_child,
        sibling,
        old_parent,
        new_parent,
        old_left,
        old_right,
        old_output,
        new_left,
        new_right,
        new_output,
    ] = bits;
    let mut residues = [F::ZERO; 17];
    for (residue, bit) in residues.iter_mut().zip(bits) {
        *residue = bit.mul(bit.sub(active));
    }
    let old_swap = direction.mul(sibling.sub(old_child));
    let new_swap = direction.mul(sibling.sub(new_child));
    residues[11] = old_left.sub(old_child.add(old_swap));
    residues[12] = old_right.sub(sibling.sub(old_swap));
    residues[13] = new_left.sub(new_child.add(new_swap));
    residues[14] = new_right.sub(sibling.sub(new_swap));
    residues[15] = old_parent.sub(old_output);
    residues[16] = new_parent.sub(new_output);
    residues
}

/// Evaluate all active-bit, direction, child-selection and full parent links.
///
/// Residue order is three selector/direction equations, 11 marker equalities,
/// then 17 equations for
/// each bit in little-endian word order: 11 activated Boolean bits, old/new
/// left/right selection, and old/new parent/output equality. No additional
/// selector gate multiplies the quadratic selection equations.
#[must_use]
pub fn level_residues<F: IntegerAirField>(
    active: F,
    allocated_path_bit: F,
    level: &SmtPathLevelWitness<F>,
    old_hash: &SmtNodeHashIo<F>,
    new_hash: &SmtNodeHashIo<F>,
) -> Vec<F> {
    let mut residues = Vec::with_capacity(LEVEL_CONSTRAINT_COUNT);
    residues.extend(selector_residues(
        active,
        level.direction,
        allocated_path_bit,
    ));
    residues.extend(marker_residues(
        active,
        level_bit_inputs(level, old_hash, new_hash, MARKER_BIT),
    ));
    for bit in 0..DIGEST_BITS {
        residues.extend(level_bit_residues(
            active,
            level.direction,
            level_bit_inputs(level, old_hash, new_hash, bit),
        ));
    }
    residues
}

fn linked_bit<F: IntegerAirField>(selector: F, left: F, right: F) -> F {
    selector.mul(left.sub(right))
}

fn paired_links<F: IntegerAirField>(
    selector: F,
    left: [&IrohaHashOutput<F>; 2],
    right: [&IrohaHashOutput<F>; 2],
) -> Vec<F> {
    let mut residues = Vec::with_capacity(PAIRED_LINK_CONSTRAINT_COUNT);
    residues.push(selector.mul(selector.sub(F::ONE)));
    for (left, right) in left.into_iter().zip(right) {
        for bit in 0..DIGEST_BITS {
            residues.push(linked_bit(
                selector,
                digest_bit(left, bit),
                digest_bit(right, bit),
            ));
        }
    }
    residues
}

/// Link old/new parents to the next level's old/new children within one update.
///
/// The fixed interior-level selector and transition zerofier must exclude the
/// final trace row and every update boundary; see the module schedule contract.
#[must_use]
pub fn level_transition_residues<F: IntegerAirField>(
    selector: F,
    current: &SmtPathLevelWitness<F>,
    next: &SmtPathLevelWitness<F>,
) -> Vec<F> {
    paired_links(
        selector,
        [&current.old_parent, &current.new_parent],
        [&next.old_child, &next.new_child],
    )
}

/// Bind an update's first children to the separately proved old/new leaf hashes.
#[must_use]
pub fn initial_leaf_residues<F: IntegerAirField>(
    selector: F,
    first: &SmtPathLevelWitness<F>,
    old_leaf: &IrohaHashOutput<F>,
    new_leaf: &IrohaHashOutput<F>,
) -> Vec<F> {
    paired_links(
        selector,
        [&first.old_child, &first.new_child],
        [old_leaf, new_leaf],
    )
}

/// Bind an update's last parents to its complete before/after root ports.
#[must_use]
pub fn final_root_residues<F: IntegerAirField>(
    selector: F,
    last: &SmtPathLevelWitness<F>,
    roots: &SmtUpdateRoots<F>,
) -> Vec<F> {
    paired_links(
        selector,
        [&last.old_parent, &last.new_parent],
        [&roots.before, &roots.after],
    )
}

/// Bind the previous update's after-root to the next update's before-root.
///
/// The surrounding authenticated schedule supplies canonical update ordering,
/// including sender-before-receiver execution and repeated-key sequencing.
#[must_use]
pub fn update_chain_residues<F: IntegerAirField>(
    selector: F,
    previous: &SmtUpdateRoots<F>,
    next: &SmtUpdateRoots<F>,
) -> Vec<F> {
    let mut residues = Vec::with_capacity(UPDATE_CHAIN_CONSTRAINT_COUNT);
    residues.push(selector.mul(selector.sub(F::ONE)));
    for bit in 0..DIGEST_BITS {
        residues.push(linked_bit(
            selector,
            digest_bit(&previous.after, bit),
            digest_bit(&next.before, bit),
        ));
    }
    residues
}

/// Bind the first before-root and last after-root to full authenticated public roots.
#[must_use]
pub fn public_root_residues<F: IntegerAirField>(
    selector: F,
    first: &SmtUpdateRoots<F>,
    last: &SmtUpdateRoots<F>,
    public_old: &IrohaHashOutput<F>,
    public_new: &IrohaHashOutput<F>,
) -> Vec<F> {
    paired_links(
        selector,
        [&first.before, &last.after],
        [public_old, public_new],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GoldilocksFp4V1,
        gadgets::{arx64_air::BitWord64, transfer::internal_hash},
    };
    use iroha_crypto::Hash;

    fn output(hash: Hash) -> IrohaHashOutput {
        let bytes: [u8; 32] = hash.into();
        IrohaHashOutput {
            words: core::array::from_fn(|word| {
                BitWord64::from_integer(u64::from_le_bytes(
                    bytes[word * 8..(word + 1) * 8].try_into().unwrap(),
                ))
            }),
        }
    }

    fn fixture_from_hashes(
        direction: bool,
        old_child: Hash,
        new_child: Hash,
        sibling: Hash,
    ) -> (
        SmtPathLevelWitness,
        SmtNodeHashIo,
        SmtNodeHashIo,
        Hash,
        Hash,
    ) {
        let (old_left, old_right, new_left, new_right) = if direction {
            (sibling, old_child, sibling, new_child)
        } else {
            (old_child, sibling, new_child, sibling)
        };
        let old_parent = internal_hash(&old_left, &old_right);
        let new_parent = internal_hash(&new_left, &new_right);
        (
            SmtPathLevelWitness {
                direction: u64::from(direction),
                old_child: output(old_child),
                new_child: output(new_child),
                sibling: output(sibling),
                old_parent: output(old_parent),
                new_parent: output(new_parent),
            },
            SmtNodeHashIo {
                left: output(old_left),
                right: output(old_right),
                output: output(old_parent),
            },
            SmtNodeHashIo {
                left: output(new_left),
                right: output(new_right),
                output: output(new_parent),
            },
            old_parent,
            new_parent,
        )
    }

    fn fixture(direction: bool) -> (SmtPathLevelWitness, SmtNodeHashIo, SmtNodeHashIo) {
        // Complete digest words above Goldilocks remain bit-decomposed integers.
        let words = [
            crate::GOLDILOCKS_MODULUS_V1,
            u64::MAX,
            0,
            0x1234_5678_90ab_cdef,
        ];
        let bytes = core::array::from_fn(|byte| words[byte / 8].to_le_bytes()[byte % 8]);
        let (level, old, new, _, _) = fixture_from_hashes(
            direction,
            Hash::prehashed(bytes),
            Hash::new(b"new child"),
            Hash::new(b"shared sibling"),
        );
        (level, old, new)
    }

    fn all_zero(residues: &[u64]) -> bool {
        residues.iter().all(|&residue| residue == 0)
    }

    fn complement(mut digest: IrohaHashOutput) -> IrohaHashOutput {
        for word in &mut digest.words {
            for bit in &mut word.bits {
                *bit ^= 1;
            }
        }
        digest
    }

    fn assert_changed_pair(residues: Vec<u64>, pair: usize) {
        assert_eq!(residues.len(), PAIRED_LINK_CONSTRAINT_COUNT);
        assert_eq!(residues[0], 0);
        for (index, residue) in residues[1..].iter().enumerate() {
            assert_eq!(
                *residue != 0,
                index / DIGEST_BITS == pair,
                "linked bit {index}"
            );
        }
    }

    #[test]
    fn both_directions_match_existing_full_width_node_hash_inputs() {
        for direction in [false, true] {
            let (level, old_hash, new_hash) = fixture(direction);
            let residues = level_residues(1, u64::from(direction), &level, &old_hash, &new_hash);
            assert_eq!(residues.len(), LEVEL_CONSTRAINT_COUNT);
            assert!(all_zero(&residues));
            let (old_selected_sibling, new_selected_sibling) = if direction {
                (old_hash.left, new_hash.left)
            } else {
                (old_hash.right, new_hash.right)
            };
            assert_eq!(old_selected_sibling, level.sibling);
            assert_eq!(new_selected_sibling, level.sibling);
            assert_eq!(
                level.old_child.words[0],
                BitWord64::from_integer(crate::GOLDILOCKS_MODULUS_V1)
            );
            assert_eq!(level.old_child.words[1], BitWord64::from_integer(u64::MAX));
        }
    }

    #[test]
    fn every_bit_of_each_child_sibling_parent_and_hash_port_is_bound() {
        for direction in [false, true] {
            let (level, old_hash, new_hash) = fixture(direction);
            for bit in 0..DIGEST_BITS {
                let bits = level_bit_inputs(&level, &old_hash, &new_hash, bit);
                assert!(all_zero(&level_bit_residues(1, level.direction, bits)));
                for field in 0..bits.len() {
                    let mut changed = bits;
                    changed[field] ^= 1;
                    // Exercise the exact production per-bit kernel, avoiding
                    // a full 4,366-residue re-evaluation for each local mutation.
                    assert!(
                        !all_zero(&level_bit_residues(1, level.direction, changed)),
                        "direction {direction}, field {field}, bit {bit}",
                    );
                }
            }
        }
    }

    #[test]
    fn coherent_marker_clearing_cannot_alias_canonical_hash_ports() {
        let (mut level, mut old_hash, mut new_hash) = fixture(false);
        for digest in [
            &mut level.old_child,
            &mut level.new_child,
            &mut level.sibling,
            &mut level.old_parent,
            &mut level.new_parent,
            &mut old_hash.left,
            &mut old_hash.right,
            &mut old_hash.output,
            &mut new_hash.left,
            &mut new_hash.right,
            &mut new_hash.output,
        ] {
            digest.words[MARKER_BIT / 64].bits[MARKER_BIT % 64] = 0;
        }
        let residues = level_residues(1, 0, &level, &old_hash, &new_hash);
        assert!(all_zero(&residues[..3]));
        assert!(residues[3..14].iter().all(|&residue| residue != 0));
        assert!(
            all_zero(&residues[14..]),
            "selection alone misses coherent marker clearing"
        );
        assert_eq!(marker_residues(0_u64, [0; 11]), [0; 11]);
        for index in 0..11 {
            let mut markers = [1_u64; 11];
            markers[index] = 0;
            let residues = marker_residues(1, markers);
            assert_eq!(residues.iter().filter(|&&value| value != 0).count(), 1);
            assert_ne!(residues[index], 0);
        }
    }

    #[test]
    fn forged_direction_allocated_bit_and_nonboolean_inputs_are_rejected() {
        let (mut level, old_hash, new_hash) = fixture(false);
        assert!(!all_zero(&level_residues(
            1, 1, &level, &old_hash, &new_hash
        )));
        level.direction = 1;
        assert!(!all_zero(&level_residues(
            1, 1, &level, &old_hash, &new_hash
        )));
        level.direction = 2;
        assert!(!all_zero(&level_residues(
            1, 2, &level, &old_hash, &new_hash
        )));
        let (level, old_hash, new_hash) = fixture(false);
        let bits = level_bit_inputs(&level, &old_hash, &new_hash, 0);
        for field in 0..bits.len() {
            let mut changed = bits;
            changed[field] = 2;
            assert!(!all_zero(&level_bit_residues(1, 0, changed)));
        }
    }

    #[test]
    fn inactive_levels_require_zero_ports_and_fixed_link_selectors_can_disable_boundaries() {
        let level = SmtPathLevelWitness::inactive();
        let hash = SmtNodeHashIo::inactive();
        assert!(all_zero(&level_residues(0, 0, &level, &hash, &hash)));
        for bit in 0..DIGEST_BITS {
            let bits = level_bit_inputs(&level, &hash, &hash, bit);
            for field in 0..bits.len() {
                let mut changed = bits;
                changed[field] = 1;
                assert!(!all_zero(&level_bit_residues(0, 0, changed)));
            }
        }
        assert!(!all_zero(&selector_residues(0_u64, 1, 1)));
        assert!(!all_zero(&selector_residues(0_u64, 0, 1)));
        assert!(!all_zero(&selector_residues(2_u64, 0, 0)));
        let public = output(Hash::new(b"independently supplied public root"));
        assert!(all_zero(&initial_leaf_residues(
            0, &level, &public, &public
        )));
        assert!(!all_zero(&initial_leaf_residues(
            2, &level, &public, &public
        )));
    }

    #[test]
    fn thirty_two_levels_and_consecutive_updates_chain_between_exact_boundaries() {
        let leaves = [
            Hash::new(b"balance before"),
            Hash::new(b"balance middle"),
            Hash::new(b"balance after"),
        ];
        let allocated_path = 0xa10f_0295_u32;
        let mut paths = Vec::new();
        for update in 0..2 {
            let (mut old, mut new) = (leaves[update], leaves[update + 1]);
            let mut path = Vec::new();
            for level_index in 0..PATH_LEVELS {
                let direction = (allocated_path >> level_index) & 1 != 0;
                let sibling = Hash::new(format!("unchanged sibling {level_index}"));
                let (level, old_hash, new_hash, old_parent, new_parent) =
                    fixture_from_hashes(direction, old, new, sibling);
                assert!(all_zero(&level_residues(
                    1,
                    u64::from(direction),
                    &level,
                    &old_hash,
                    &new_hash
                )));
                path.push(level);
                old = old_parent;
                new = new_parent;
            }
            paths.push(path);
        }
        let roots: Vec<_> = paths
            .iter()
            .map(|path| {
                let last = path.last().unwrap();
                SmtUpdateRoots {
                    before: last.old_parent,
                    after: last.new_parent,
                }
            })
            .collect();
        for (update, path) in paths.iter().enumerate() {
            assert_eq!(path.len(), 32);
            assert!(all_zero(&initial_leaf_residues(
                1,
                &path[0],
                &output(leaves[update]),
                &output(leaves[update + 1])
            )));
            for levels in path.windows(2) {
                assert!(all_zero(&level_transition_residues(
                    1, &levels[0], &levels[1]
                )));
            }
            assert!(all_zero(&final_root_residues(
                1,
                path.last().unwrap(),
                &roots[update]
            )));
        }
        let chained = update_chain_residues(1, &roots[0], &roots[1]);
        assert_eq!(chained.len(), UPDATE_CHAIN_CONSTRAINT_COUNT);
        assert!(all_zero(&chained));
        assert!(all_zero(&public_root_residues(
            1,
            &roots[0],
            &roots[1],
            &roots[0].before,
            &roots[1].after
        )));
        assert!(!all_zero(&update_chain_residues(1, &roots[1], &roots[0])));
    }

    #[test]
    fn every_leaf_transition_root_and_public_boundary_bit_has_its_own_equality() {
        let (first, _, _) = fixture(false);
        let mut next = first;
        next.old_child = first.old_parent;
        next.new_child = first.new_parent;
        let roots = SmtUpdateRoots {
            before: first.old_parent,
            after: first.new_parent,
        };
        let following = SmtUpdateRoots {
            before: roots.after,
            after: output(Hash::new(b"last public root")),
        };
        // Complementing each bit at once exposes every independent equality in
        // one evaluation. The per-bit kernel checks below additionally exercise
        // isolated mutations without quadratic whole-gadget recomputation.
        assert_changed_pair(
            initial_leaf_residues(1, &first, &complement(first.old_child), &first.new_child),
            0,
        );
        assert_changed_pair(
            initial_leaf_residues(1, &first, &first.old_child, &complement(first.new_child)),
            1,
        );
        let mut changed = next;
        changed.old_child = complement(changed.old_child);
        assert_changed_pair(level_transition_residues(1, &first, &changed), 0);
        changed = next;
        changed.new_child = complement(changed.new_child);
        assert_changed_pair(level_transition_residues(1, &first, &changed), 1);
        let mut changed_roots = roots;
        changed_roots.before = complement(changed_roots.before);
        assert_changed_pair(final_root_residues(1, &first, &changed_roots), 0);
        changed_roots = roots;
        changed_roots.after = complement(changed_roots.after);
        assert_changed_pair(final_root_residues(1, &first, &changed_roots), 1);
        assert_changed_pair(
            public_root_residues(
                1,
                &roots,
                &following,
                &complement(roots.before),
                &following.after,
            ),
            0,
        );
        assert_changed_pair(
            public_root_residues(
                1,
                &roots,
                &following,
                &roots.before,
                &complement(following.after),
            ),
            1,
        );
        let mut changed_following = following;
        changed_following.before = complement(changed_following.before);
        let residues = update_chain_residues(1, &roots, &changed_following);
        assert_eq!(residues[0], 0);
        assert!(residues[1..].iter().all(|&residue| residue != 0));
        for bit in 0..DIGEST_BITS {
            let value = digest_bit(&roots.after, bit);
            assert_eq!(linked_bit(1_u64, value, value), 0);
            assert_ne!(linked_bit(1_u64, value, value ^ 1), 0);
            assert_eq!(linked_bit(0_u64, value, value ^ 1), 0);
        }
    }

    fn map_output<F: Copy>(
        output: IrohaHashOutput,
        map: &mut impl FnMut(u64) -> F,
    ) -> IrohaHashOutput<F> {
        IrohaHashOutput {
            words: output.words.map(|word| BitWord64 {
                bits: word.bits.map(&mut *map),
            }),
        }
    }

    fn map_level<F: Copy>(
        level: SmtPathLevelWitness,
        map: &mut impl FnMut(u64) -> F,
    ) -> SmtPathLevelWitness<F> {
        SmtPathLevelWitness {
            direction: map(level.direction),
            old_child: map_output(level.old_child, map),
            new_child: map_output(level.new_child, map),
            sibling: map_output(level.sibling, map),
            old_parent: map_output(level.old_parent, map),
            new_parent: map_output(level.new_parent, map),
        }
    }

    fn map_hash<F: Copy>(hash: SmtNodeHashIo, map: &mut impl FnMut(u64) -> F) -> SmtNodeHashIo<F> {
        SmtNodeHashIo {
            left: map_output(hash.left, map),
            right: map_output(hash.right, map),
            output: map_output(hash.output, map),
        }
    }

    #[test]
    fn extension_field_evaluation_agrees_on_invalid_full_width_openings() {
        let (mut level, old, new) = fixture(true);
        level.new_child.words[3].bits[63] = 2;
        let base = level_residues(1, 1, &level, &old, &new);
        assert!(!all_zero(&base));
        let mut lift = |value| GoldilocksFp4V1::from_base(value).unwrap();
        let extension = level_residues(
            GoldilocksFp4V1::ONE,
            GoldilocksFp4V1::ONE,
            &map_level(level, &mut lift),
            &map_hash(old, &mut lift),
            &map_hash(new, &mut lift),
        );
        assert_eq!(extension, base.into_iter().map(lift).collect::<Vec<_>>());
    }

    #[derive(Clone, Copy)]
    struct Degree(usize);

    impl IntegerAirField for Degree {
        const ZERO: Self = Self(0);
        const ONE: Self = Self(0);
        fn from_u32(_: u32) -> Self {
            Self(0)
        }
        fn add(self, other: Self) -> Self {
            Self(self.0.max(other.0))
        }
        fn sub(self, other: Self) -> Self {
            Self(self.0.max(other.0))
        }
        fn mul(self, other: Self) -> Self {
            Self(self.0 + other.0)
        }
    }

    #[test]
    fn every_relation_has_quadratic_degree_and_fixed_resource_bounds() {
        let (level, old, new) = fixture(false);
        let mut variable = |_| Degree(1);
        let level = map_level(level, &mut variable);
        let old = map_hash(old, &mut variable);
        let new = map_hash(new, &mut variable);
        let roots = SmtUpdateRoots {
            before: level.old_child,
            after: level.new_child,
        };
        let local = level_residues(Degree(1), Degree(1), &level, &old, &new);
        assert_eq!(local.len(), 4_366);
        assert_eq!(local.len(), LEVEL_CONSTRAINT_COUNT);
        for residues in [
            local,
            level_transition_residues(Degree(1), &level, &level),
            initial_leaf_residues(Degree(1), &level, &roots.before, &roots.after),
            final_root_residues(Degree(1), &level, &roots),
            update_chain_residues(Degree(1), &roots, &roots),
            public_root_residues(Degree(1), &roots, &roots, &roots.before, &roots.after),
        ] {
            assert!(
                residues
                    .iter()
                    .all(|degree| degree.0 <= MAX_CONSTRAINT_DEGREE)
            );
            assert_eq!(residues.iter().map(|degree| degree.0).max(), Some(2));
        }
        assert_eq!(PATH_LEVELS, 32);
        assert_eq!(LEVEL_INPUT_FIELD_COUNT, 2_819);
        assert_eq!(
            core::mem::size_of::<SmtPathLevelWitness>(),
            LEVEL_WITNESS_FIELD_COUNT * 8
        );
        assert_eq!(
            core::mem::size_of::<SmtNodeHashIo>(),
            NODE_HASH_IO_FIELD_COUNT * 8
        );
        assert_eq!(PAIRED_LINK_CONSTRAINT_COUNT, 513);
        assert_eq!(UPDATE_CHAIN_CONSTRAINT_COUNT, 257);
    }
}
