//! Canonical SHA-256 sparse-tree semantics for the zk-X509 relation.
//!
//! Both trees have fixed depth.  A proof never carries caller-selected
//! direction bits: the path is derived from the domain-separated key.  Nodes
//! include their height in the hash frame, preventing a value valid at one
//! level from being replayed at another.

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::profile::{
    ZK_X509_CA_EMPTY_LEAF_DOMAIN_V1, ZK_X509_CA_KEY_DOMAIN_V1, ZK_X509_CA_LEAF_DOMAIN_V1,
    ZK_X509_CA_NODE_DOMAIN_V1, ZK_X509_CA_TREE_DEPTH_V1, ZK_X509_CRL_EMPTY_LEAF_DOMAIN_V1,
    ZK_X509_CRL_KEY_DOMAIN_V1, ZK_X509_CRL_LEAF_DOMAIN_V1, ZK_X509_CRL_NODE_DOMAIN_V1,
    ZK_X509_CRL_TREE_DEPTH_V1, ZK_X509_HASH_FRAME_DOMAIN_V1, ZK_X509_MAX_SERIAL_BYTES_V1,
};

/// One SHA-256 accumulator digest.
pub(crate) type ZkX509MerkleDigestV1 = [u8; 32];

/// Fixed-depth path for a governed CA member.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CaMembershipPathV1 {
    /// Sibling at each level, ordered leaf-to-root.
    pub(crate) siblings: Vec<ZkX509MerkleDigestV1>,
}

/// Fixed-depth path proving an empty leaf in the CRL sparse tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CrlNonmembershipPathV1 {
    /// Sibling at each level, ordered leaf-to-root.
    pub(crate) siblings: Vec<ZkX509MerkleDigestV1>,
}

/// Accumulator-input or path failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509MerkleErrorV1 {
    /// A mandatory byte string was empty.
    #[error("zk-X509 accumulator field {field} must not be empty")]
    EmptyField {
        /// Stable field name.
        field: &'static str,
    },
    /// An unsigned serial is zero, negative-looking, non-minimal, or oversized.
    #[error("zk-X509 certificate serial is not canonical")]
    InvalidSerial,
    /// A complete sparse input contains the same derived key more than once.
    #[error("zk-X509 complete sparse accumulator contains duplicate keys")]
    DuplicateKey,
    /// A requested CA member does not occur in the complete trust-store input.
    #[error("zk-X509 requested CA member is absent from the complete trust store")]
    MissingMember,
    /// The supplied sibling count differs from the fixed tree depth.
    #[error("zk-X509 {tree} path has {actual} siblings; expected {expected}")]
    InvalidPathLength {
        /// Stable tree name.
        tree: &'static str,
        /// Supplied siblings.
        actual: usize,
        /// Fixed depth.
        expected: usize,
    },
    /// A derived root does not equal the governed root.
    #[error("zk-X509 {tree} path does not match the governed root")]
    RootMismatch {
        /// Stable tree name.
        tree: &'static str,
    },
    /// A frame label, field count, or field length cannot be represented.
    #[error("zk-X509 SHA-256 frame length overflow")]
    FrameLengthOverflow,
}

/// Hash one canonical length-delimited frame.
///
/// The frame is:
/// `frame-domain || u16(domain-len) || domain || u16(field-count) ||
///  repeated(u64(field-len) || field)`.
pub(crate) fn hash_frame_v1(
    domain: &[u8],
    fields: &[&[u8]],
) -> Result<ZkX509MerkleDigestV1, ZkX509MerkleErrorV1> {
    let domain_len =
        u16::try_from(domain.len()).map_err(|_| ZkX509MerkleErrorV1::FrameLengthOverflow)?;
    let field_count =
        u16::try_from(fields.len()).map_err(|_| ZkX509MerkleErrorV1::FrameLengthOverflow)?;
    let mut hash = Sha256::new();
    hash.update(ZK_X509_HASH_FRAME_DOMAIN_V1);
    hash.update(domain_len.to_be_bytes());
    hash.update(domain);
    hash.update(field_count.to_be_bytes());
    for field in fields {
        let field_len =
            u64::try_from(field.len()).map_err(|_| ZkX509MerkleErrorV1::FrameLengthOverflow)?;
        hash.update(field_len.to_be_bytes());
        hash.update(field);
    }
    Ok(hash.finalize().into())
}

/// Derive the fixed CA-tree key from exact root SPKI DER.
pub(crate) fn ca_key_v1(root_spki_der: &[u8]) -> Result<ZkX509MerkleDigestV1, ZkX509MerkleErrorV1> {
    require_nonempty("root_spki_der", root_spki_der)?;
    hash_frame_v1(ZK_X509_CA_KEY_DOMAIN_V1, &[root_spki_der])
}

/// Derive an occupied CA leaf from exact root SPKI DER.
pub(crate) fn ca_leaf_v1(
    root_spki_der: &[u8],
) -> Result<ZkX509MerkleDigestV1, ZkX509MerkleErrorV1> {
    let key = ca_key_v1(root_spki_der)?;
    hash_frame_v1(ZK_X509_CA_LEAF_DOMAIN_V1, &[&key, root_spki_der])
}

/// Derive the unique empty CA leaf.
pub(crate) fn ca_empty_leaf_v1() -> ZkX509MerkleDigestV1 {
    hash_frame_v1(ZK_X509_CA_EMPTY_LEAF_DOMAIN_V1, &[])
        .expect("fixed empty CA leaf frame is representable")
}

/// Derive the CRL-tree key from exact issuer SPKI DER and unsigned serial.
pub(crate) fn crl_key_v1(
    issuer_spki_der: &[u8],
    serial: &[u8],
) -> Result<ZkX509MerkleDigestV1, ZkX509MerkleErrorV1> {
    require_nonempty("issuer_spki_der", issuer_spki_der)?;
    validate_unsigned_serial_v1(serial)?;
    hash_frame_v1(ZK_X509_CRL_KEY_DOMAIN_V1, &[issuer_spki_der, serial])
}

/// Derive an occupied CRL leaf.
///
/// Publishers use this value when constructing the governed sparse tree.  A
/// showing proves the leaf at the same key is instead [`crl_empty_leaf_v1`].
pub(crate) fn crl_leaf_v1(
    issuer_spki_der: &[u8],
    serial: &[u8],
) -> Result<ZkX509MerkleDigestV1, ZkX509MerkleErrorV1> {
    let key = crl_key_v1(issuer_spki_der, serial)?;
    hash_frame_v1(ZK_X509_CRL_LEAF_DOMAIN_V1, &[&key])
}

/// Derive the unique empty CRL leaf.
pub(crate) fn crl_empty_leaf_v1() -> ZkX509MerkleDigestV1 {
    hash_frame_v1(ZK_X509_CRL_EMPTY_LEAF_DOMAIN_V1, &[])
        .expect("fixed empty CRL leaf frame is representable")
}

/// Reconstruct the complete issuer-scoped CRL sparse-tree root.
///
/// `serials` must contain every revoked serial in the signed base CRL.  The
/// function derives all 256-bit keys, rejects duplicates, and fills every
/// absent subtree with the unique recursively derived empty root.  Runtime is
/// `O(n log n + 256n)` for at most the profile's bounded CRL entry count; it
/// never allocates the exponentially large logical tree.
pub(crate) fn crl_root_from_complete_serials_v1(
    issuer_spki_der: &[u8],
    serials: &[&[u8]],
) -> Result<ZkX509MerkleDigestV1, ZkX509MerkleErrorV1> {
    require_nonempty("issuer_spki_der", issuer_spki_der)?;

    let mut entries = Vec::with_capacity(serials.len());
    for serial in serials {
        let key = crl_key_v1(issuer_spki_der, serial)?;
        let leaf = crl_leaf_v1(issuer_spki_der, serial)?;
        entries.push((key, leaf));
    }
    entries.sort_unstable_by_key(|entry| entry.0);
    if entries.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(ZkX509MerkleErrorV1::DuplicateKey);
    }

    let empty_roots = crl_empty_subtree_roots_v1()?;
    crl_sparse_subtree_root_v1(&entries, 0, &empty_roots)
}

/// Reconstruct a CA sparse-tree root from the complete governed SPKI set.
///
/// Ordering is irrelevant, while duplicate derived keys are rejected.  This
/// is the authoritative publisher-side counterpart to
/// [`verify_ca_membership_v1`].
pub(crate) fn ca_root_from_complete_spkis_v1(
    root_spkis_der: &[&[u8]],
) -> Result<ZkX509MerkleDigestV1, ZkX509MerkleErrorV1> {
    let entries = ca_entries_from_complete_spkis_v1(root_spkis_der)?;
    let empty_roots = ca_empty_subtree_roots_v1()?;
    ca_sparse_subtree_root_v1(&entries, 0, &empty_roots)
}

/// Construct the sole canonical membership path for one governed root SPKI.
pub(crate) fn ca_membership_path_from_complete_spkis_v1(
    root_spkis_der: &[&[u8]],
    member_spki_der: &[u8],
) -> Result<ZkX509CaMembershipPathV1, ZkX509MerkleErrorV1> {
    let entries = ca_entries_from_complete_spkis_v1(root_spkis_der)?;
    let member_key = ca_key_v1(member_spki_der)?;
    let member_leaf = ca_leaf_v1(member_spki_der)?;
    if entries
        .binary_search_by_key(&member_key, |entry| entry.0)
        .ok()
        .and_then(|index| entries.get(index))
        .is_none_or(|entry| entry.1 != member_leaf)
    {
        return Err(ZkX509MerkleErrorV1::MissingMember);
    }
    let empty_roots = ca_empty_subtree_roots_v1()?;
    let mut siblings_root_to_leaf = Vec::with_capacity(ZK_X509_CA_TREE_DEPTH_V1);
    collect_ca_siblings_v1(
        &entries,
        &member_key,
        0,
        &empty_roots,
        &mut siblings_root_to_leaf,
    )?;
    siblings_root_to_leaf.reverse();
    Ok(ZkX509CaMembershipPathV1 {
        siblings: siblings_root_to_leaf,
    })
}

/// Compute the root selected by a CA membership witness.
pub(crate) fn ca_root_from_path_v1(
    root_spki_der: &[u8],
    path: &ZkX509CaMembershipPathV1,
) -> Result<ZkX509MerkleDigestV1, ZkX509MerkleErrorV1> {
    validate_path_length("CA", path.siblings.len(), ZK_X509_CA_TREE_DEPTH_V1)?;
    let key = ca_key_v1(root_spki_der)?;
    fold_path_v1(
        ca_leaf_v1(root_spki_der)?,
        &key,
        &path.siblings,
        ZK_X509_CA_NODE_DOMAIN_V1,
    )
}

/// Verify a CA membership witness against the exact governed root.
pub(crate) fn verify_ca_membership_v1(
    governed_root: ZkX509MerkleDigestV1,
    root_spki_der: &[u8],
    path: &ZkX509CaMembershipPathV1,
) -> Result<(), ZkX509MerkleErrorV1> {
    if ca_root_from_path_v1(root_spki_der, path)? != governed_root {
        return Err(ZkX509MerkleErrorV1::RootMismatch { tree: "CA" });
    }
    Ok(())
}

/// Compute the root selected by a CRL non-membership witness.
pub(crate) fn crl_root_from_nonmembership_path_v1(
    issuer_spki_der: &[u8],
    serial: &[u8],
    path: &ZkX509CrlNonmembershipPathV1,
) -> Result<ZkX509MerkleDigestV1, ZkX509MerkleErrorV1> {
    validate_path_length("CRL", path.siblings.len(), ZK_X509_CRL_TREE_DEPTH_V1)?;
    let key = crl_key_v1(issuer_spki_der, serial)?;
    fold_path_v1(
        crl_empty_leaf_v1(),
        &key,
        &path.siblings,
        ZK_X509_CRL_NODE_DOMAIN_V1,
    )
}

/// Verify an empty CRL leaf against the exact governed sparse root.
pub(crate) fn verify_crl_nonmembership_v1(
    governed_root: ZkX509MerkleDigestV1,
    issuer_spki_der: &[u8],
    serial: &[u8],
    path: &ZkX509CrlNonmembershipPathV1,
) -> Result<(), ZkX509MerkleErrorV1> {
    if crl_root_from_nonmembership_path_v1(issuer_spki_der, serial, path)? != governed_root {
        return Err(ZkX509MerkleErrorV1::RootMismatch { tree: "CRL" });
    }
    Ok(())
}

fn require_nonempty(field: &'static str, value: &[u8]) -> Result<(), ZkX509MerkleErrorV1> {
    if value.is_empty() {
        return Err(ZkX509MerkleErrorV1::EmptyField { field });
    }
    Ok(())
}

fn validate_unsigned_serial_v1(serial: &[u8]) -> Result<(), ZkX509MerkleErrorV1> {
    if serial.is_empty()
        || serial.len() > ZK_X509_MAX_SERIAL_BYTES_V1
        || serial.iter().all(|byte| *byte == 0)
        || (serial.len() > 1 && serial[0] == 0)
    {
        return Err(ZkX509MerkleErrorV1::InvalidSerial);
    }
    Ok(())
}

fn validate_path_length(
    tree: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), ZkX509MerkleErrorV1> {
    if actual != expected {
        return Err(ZkX509MerkleErrorV1::InvalidPathLength {
            tree,
            actual,
            expected,
        });
    }
    Ok(())
}

fn fold_path_v1(
    mut current: ZkX509MerkleDigestV1,
    key: &ZkX509MerkleDigestV1,
    siblings: &[ZkX509MerkleDigestV1],
    node_domain: &[u8],
) -> Result<ZkX509MerkleDigestV1, ZkX509MerkleErrorV1> {
    for (level, sibling) in siblings.iter().enumerate() {
        let height = u16::try_from(level + 1)
            .map_err(|_| ZkX509MerkleErrorV1::FrameLengthOverflow)?
            .to_be_bytes();
        let bit_index = siblings.len() - 1 - level;
        let (left, right) = if key_bit_from_msb(key, bit_index) {
            (sibling, &current)
        } else {
            (&current, sibling)
        };
        current = hash_frame_v1(node_domain, &[&height, left, right])?;
    }
    Ok(current)
}

fn crl_empty_subtree_roots_v1()
-> Result<[ZkX509MerkleDigestV1; ZK_X509_CRL_TREE_DEPTH_V1 + 1], ZkX509MerkleErrorV1> {
    let mut roots = [[0_u8; 32]; ZK_X509_CRL_TREE_DEPTH_V1 + 1];
    roots[0] = crl_empty_leaf_v1();
    for height in 1..=ZK_X509_CRL_TREE_DEPTH_V1 {
        let encoded_height = u16::try_from(height)
            .map_err(|_| ZkX509MerkleErrorV1::FrameLengthOverflow)?
            .to_be_bytes();
        roots[height] = hash_frame_v1(
            ZK_X509_CRL_NODE_DOMAIN_V1,
            &[&encoded_height, &roots[height - 1], &roots[height - 1]],
        )?;
    }
    Ok(roots)
}

fn ca_entries_from_complete_spkis_v1(
    root_spkis_der: &[&[u8]],
) -> Result<Vec<(ZkX509MerkleDigestV1, ZkX509MerkleDigestV1)>, ZkX509MerkleErrorV1> {
    let mut entries = Vec::with_capacity(root_spkis_der.len());
    for spki in root_spkis_der {
        entries.push((ca_key_v1(spki)?, ca_leaf_v1(spki)?));
    }
    entries.sort_unstable_by_key(|entry| entry.0);
    if entries.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(ZkX509MerkleErrorV1::DuplicateKey);
    }
    Ok(entries)
}

fn ca_empty_subtree_roots_v1()
-> Result<[ZkX509MerkleDigestV1; ZK_X509_CA_TREE_DEPTH_V1 + 1], ZkX509MerkleErrorV1> {
    let mut roots = [[0_u8; 32]; ZK_X509_CA_TREE_DEPTH_V1 + 1];
    roots[0] = ca_empty_leaf_v1();
    for height in 1..=ZK_X509_CA_TREE_DEPTH_V1 {
        let encoded_height = u16::try_from(height)
            .map_err(|_| ZkX509MerkleErrorV1::FrameLengthOverflow)?
            .to_be_bytes();
        roots[height] = hash_frame_v1(
            ZK_X509_CA_NODE_DOMAIN_V1,
            &[&encoded_height, &roots[height - 1], &roots[height - 1]],
        )?;
    }
    Ok(roots)
}

fn ca_sparse_subtree_root_v1(
    entries: &[(ZkX509MerkleDigestV1, ZkX509MerkleDigestV1)],
    depth: usize,
    empty_roots: &[ZkX509MerkleDigestV1; ZK_X509_CA_TREE_DEPTH_V1 + 1],
) -> Result<ZkX509MerkleDigestV1, ZkX509MerkleErrorV1> {
    let remaining_height = ZK_X509_CA_TREE_DEPTH_V1
        .checked_sub(depth)
        .ok_or(ZkX509MerkleErrorV1::FrameLengthOverflow)?;
    if entries.is_empty() {
        return Ok(empty_roots[remaining_height]);
    }
    if depth == ZK_X509_CA_TREE_DEPTH_V1 {
        debug_assert_eq!(entries.len(), 1);
        return Ok(entries[0].1);
    }
    let first_right = entries.partition_point(|entry| !key_bit_from_msb(&entry.0, depth));
    let left = ca_sparse_subtree_root_v1(&entries[..first_right], depth + 1, empty_roots)?;
    let right = ca_sparse_subtree_root_v1(&entries[first_right..], depth + 1, empty_roots)?;
    let encoded_height = u16::try_from(remaining_height)
        .map_err(|_| ZkX509MerkleErrorV1::FrameLengthOverflow)?
        .to_be_bytes();
    hash_frame_v1(ZK_X509_CA_NODE_DOMAIN_V1, &[&encoded_height, &left, &right])
}

fn collect_ca_siblings_v1(
    entries: &[(ZkX509MerkleDigestV1, ZkX509MerkleDigestV1)],
    member_key: &ZkX509MerkleDigestV1,
    depth: usize,
    empty_roots: &[ZkX509MerkleDigestV1; ZK_X509_CA_TREE_DEPTH_V1 + 1],
    siblings_root_to_leaf: &mut Vec<ZkX509MerkleDigestV1>,
) -> Result<(), ZkX509MerkleErrorV1> {
    if depth == ZK_X509_CA_TREE_DEPTH_V1 {
        return if entries.len() == 1 && entries[0].0 == *member_key {
            Ok(())
        } else {
            Err(ZkX509MerkleErrorV1::MissingMember)
        };
    }
    let first_right = entries.partition_point(|entry| !key_bit_from_msb(&entry.0, depth));
    let (member_entries, sibling_entries) = if key_bit_from_msb(member_key, depth) {
        (&entries[first_right..], &entries[..first_right])
    } else {
        (&entries[..first_right], &entries[first_right..])
    };
    siblings_root_to_leaf.push(ca_sparse_subtree_root_v1(
        sibling_entries,
        depth + 1,
        empty_roots,
    )?);
    collect_ca_siblings_v1(
        member_entries,
        member_key,
        depth + 1,
        empty_roots,
        siblings_root_to_leaf,
    )
}

fn crl_sparse_subtree_root_v1(
    entries: &[(ZkX509MerkleDigestV1, ZkX509MerkleDigestV1)],
    depth: usize,
    empty_roots: &[ZkX509MerkleDigestV1; ZK_X509_CRL_TREE_DEPTH_V1 + 1],
) -> Result<ZkX509MerkleDigestV1, ZkX509MerkleErrorV1> {
    let remaining_height = ZK_X509_CRL_TREE_DEPTH_V1
        .checked_sub(depth)
        .ok_or(ZkX509MerkleErrorV1::FrameLengthOverflow)?;
    if entries.is_empty() {
        return Ok(empty_roots[remaining_height]);
    }
    if depth == ZK_X509_CRL_TREE_DEPTH_V1 {
        debug_assert_eq!(entries.len(), 1);
        return Ok(entries[0].1);
    }

    let first_right = entries.partition_point(|entry| !key_bit_from_msb(&entry.0, depth));
    let left = crl_sparse_subtree_root_v1(&entries[..first_right], depth + 1, empty_roots)?;
    let right = crl_sparse_subtree_root_v1(&entries[first_right..], depth + 1, empty_roots)?;
    let encoded_height = u16::try_from(remaining_height)
        .map_err(|_| ZkX509MerkleErrorV1::FrameLengthOverflow)?
        .to_be_bytes();
    hash_frame_v1(
        ZK_X509_CRL_NODE_DOMAIN_V1,
        &[&encoded_height, &left, &right],
    )
}

fn key_bit_from_msb(key: &ZkX509MerkleDigestV1, bit_index: usize) -> bool {
    let byte = key[bit_index / 8];
    let shift = 7 - (bit_index % 8);
    (byte >> shift) & 1 == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ca_path(seed: u8) -> ZkX509CaMembershipPathV1 {
        ZkX509CaMembershipPathV1 {
            siblings: (0..ZK_X509_CA_TREE_DEPTH_V1)
                .map(|index| [seed.wrapping_add(index as u8); 32])
                .collect(),
        }
    }

    fn crl_path(seed: u8) -> ZkX509CrlNonmembershipPathV1 {
        ZkX509CrlNonmembershipPathV1 {
            siblings: (0..ZK_X509_CRL_TREE_DEPTH_V1)
                .map(|index| [seed.wrapping_add(index as u8); 32])
                .collect(),
        }
    }

    #[test]
    fn ca_membership_binds_spki_path_order_direction_and_root() {
        let spki = b"canonical root SPKI DER";
        let path = ca_path(11);
        let root = ca_root_from_path_v1(spki, &path).expect("CA root");
        verify_ca_membership_v1(root, spki, &path).expect("valid CA path");

        let mut changed_spki = spki.to_vec();
        changed_spki[0] ^= 1;
        assert_eq!(
            verify_ca_membership_v1(root, &changed_spki, &path),
            Err(ZkX509MerkleErrorV1::RootMismatch { tree: "CA" })
        );

        let mut changed_path = path.clone();
        changed_path.siblings[7][3] ^= 1;
        assert_eq!(
            verify_ca_membership_v1(root, spki, &changed_path),
            Err(ZkX509MerkleErrorV1::RootMismatch { tree: "CA" })
        );

        let mut reordered = path.clone();
        reordered.siblings.swap(0, 1);
        assert_eq!(
            verify_ca_membership_v1(root, spki, &reordered),
            Err(ZkX509MerkleErrorV1::RootMismatch { tree: "CA" })
        );

        assert_eq!(
            verify_ca_membership_v1([0; 32], spki, &path),
            Err(ZkX509MerkleErrorV1::RootMismatch { tree: "CA" })
        );
    }

    #[test]
    fn ca_path_length_is_exact_and_empty_spki_is_rejected() {
        let mut short = ca_path(21);
        short.siblings.pop();
        assert_eq!(
            ca_root_from_path_v1(b"spki", &short),
            Err(ZkX509MerkleErrorV1::InvalidPathLength {
                tree: "CA",
                actual: ZK_X509_CA_TREE_DEPTH_V1 - 1,
                expected: ZK_X509_CA_TREE_DEPTH_V1,
            })
        );
        assert_eq!(
            ca_root_from_path_v1(&[], &ca_path(21)),
            Err(ZkX509MerkleErrorV1::EmptyField {
                field: "root_spki_der"
            })
        );
    }

    #[test]
    fn crl_nonmembership_binds_issuer_serial_path_and_empty_leaf() {
        let issuer_spki = b"canonical issuer SPKI DER";
        let serial = [0x01, 0x23, 0x45];
        let path = crl_path(31);
        let root =
            crl_root_from_nonmembership_path_v1(issuer_spki, &serial, &path).expect("CRL root");
        verify_crl_nonmembership_v1(root, issuer_spki, &serial, &path)
            .expect("valid nonmembership");

        assert_eq!(
            verify_crl_nonmembership_v1(root, b"other issuer", &serial, &path),
            Err(ZkX509MerkleErrorV1::RootMismatch { tree: "CRL" })
        );
        assert_eq!(
            verify_crl_nonmembership_v1(root, issuer_spki, &[0x01, 0x23, 0x46], &path),
            Err(ZkX509MerkleErrorV1::RootMismatch { tree: "CRL" })
        );

        let occupied = crl_leaf_v1(issuer_spki, &serial).expect("occupied CRL leaf");
        assert_ne!(occupied, crl_empty_leaf_v1());
        let key = crl_key_v1(issuer_spki, &serial).expect("CRL key");
        let occupied_root =
            fold_path_v1(occupied, &key, &path.siblings, ZK_X509_CRL_NODE_DOMAIN_V1)
                .expect("occupied root");
        assert_ne!(root, occupied_root);
    }

    #[test]
    fn crl_paths_and_serials_fail_closed() {
        let issuer = b"issuer";
        let mut long = crl_path(41);
        long.siblings.push([0x99; 32]);
        assert_eq!(
            crl_root_from_nonmembership_path_v1(issuer, &[1], &long),
            Err(ZkX509MerkleErrorV1::InvalidPathLength {
                tree: "CRL",
                actual: ZK_X509_CRL_TREE_DEPTH_V1 + 1,
                expected: ZK_X509_CRL_TREE_DEPTH_V1,
            })
        );

        for serial in [
            Vec::new(),
            vec![0],
            vec![0, 1],
            vec![1; ZK_X509_MAX_SERIAL_BYTES_V1 + 1],
        ] {
            assert_eq!(
                crl_key_v1(issuer, &serial),
                Err(ZkX509MerkleErrorV1::InvalidSerial)
            );
        }
        assert_eq!(
            crl_key_v1(&[], &[1]),
            Err(ZkX509MerkleErrorV1::EmptyField {
                field: "issuer_spki_der"
            })
        );
    }

    #[test]
    fn complete_crl_root_matches_empty_and_single_entry_paths() {
        let issuer = b"canonical issuer SPKI";
        let empty = crl_root_from_complete_serials_v1(issuer, &[]).expect("empty root");
        let empty_roots = crl_empty_subtree_roots_v1().expect("empty roots");
        assert_eq!(empty, empty_roots[ZK_X509_CRL_TREE_DEPTH_V1]);

        let serial = [0x12, 0x34];
        let serials: [&[u8]; 1] = [&serial];
        let complete =
            crl_root_from_complete_serials_v1(issuer, &serials).expect("single-entry root");
        let key = crl_key_v1(issuer, &serial).expect("key");
        let mut siblings = Vec::with_capacity(ZK_X509_CRL_TREE_DEPTH_V1);
        for level in 0..ZK_X509_CRL_TREE_DEPTH_V1 {
            siblings.push(empty_roots[level]);
        }
        let from_path = fold_path_v1(
            crl_leaf_v1(issuer, &serial).expect("leaf"),
            &key,
            &siblings,
            ZK_X509_CRL_NODE_DOMAIN_V1,
        )
        .expect("path root");
        assert_eq!(complete, from_path);
    }

    #[test]
    fn complete_crl_root_is_order_independent_and_rejects_duplicates() {
        let issuer = b"issuer";
        let a = [1_u8];
        let b = [2_u8];
        let forward: [&[u8]; 2] = [&a, &b];
        let reverse: [&[u8]; 2] = [&b, &a];
        assert_eq!(
            crl_root_from_complete_serials_v1(issuer, &forward),
            crl_root_from_complete_serials_v1(issuer, &reverse)
        );

        let duplicate: [&[u8]; 2] = [&a, &a];
        assert_eq!(
            crl_root_from_complete_serials_v1(issuer, &duplicate),
            Err(ZkX509MerkleErrorV1::DuplicateKey)
        );
    }

    #[test]
    fn complete_ca_tree_constructs_every_canonical_membership_path() {
        let first = b"first canonical root SPKI".as_slice();
        let second = b"second canonical root SPKI".as_slice();
        let third = b"third canonical root SPKI".as_slice();
        let complete = [first, second, third];
        let root = ca_root_from_complete_spkis_v1(&complete).expect("complete CA root");
        assert_eq!(
            ca_root_from_complete_spkis_v1(&[third, first, second]).expect("reordered CA root"),
            root
        );
        for member in complete {
            let path = ca_membership_path_from_complete_spkis_v1(&complete, member)
                .expect("canonical membership path");
            assert_eq!(path.siblings.len(), ZK_X509_CA_TREE_DEPTH_V1);
            verify_ca_membership_v1(root, member, &path).expect("member verifies");
        }
        assert_eq!(
            ca_membership_path_from_complete_spkis_v1(&complete, b"absent root SPKI"),
            Err(ZkX509MerkleErrorV1::MissingMember)
        );
        assert_eq!(
            ca_root_from_complete_spkis_v1(&[first, first]),
            Err(ZkX509MerkleErrorV1::DuplicateKey)
        );
    }

    #[test]
    fn every_hash_role_is_domain_separated() {
        let value = b"same bytes";
        let key = ca_key_v1(value).expect("CA key");
        let ca_leaf = ca_leaf_v1(value).expect("CA leaf");
        let crl_key = crl_key_v1(value, &[1]).expect("CRL key");
        let crl_leaf = crl_leaf_v1(value, &[1]).expect("CRL leaf");
        let values = [
            key,
            ca_leaf,
            ca_empty_leaf_v1(),
            crl_key,
            crl_leaf,
            crl_empty_leaf_v1(),
        ];
        for (index, left) in values.iter().enumerate() {
            for right in &values[index + 1..] {
                assert_ne!(left, right);
            }
        }
    }

    #[test]
    fn frame_is_length_delimited_and_field_ordered() {
        let left = hash_frame_v1(b"d", &[b"ab", b"c"]).expect("frame");
        let ambiguous = hash_frame_v1(b"d", &[b"a", b"bc"]).expect("frame");
        let reordered = hash_frame_v1(b"d", &[b"c", b"ab"]).expect("frame");
        let other_domain = hash_frame_v1(b"e", &[b"ab", b"c"]).expect("frame");
        assert_ne!(left, ambiguous);
        assert_ne!(left, reordered);
        assert_ne!(left, other_domain);
    }
}
