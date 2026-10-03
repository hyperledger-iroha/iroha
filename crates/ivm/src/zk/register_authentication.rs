//! Borrowed register-event membership checks for local trace diagnostics.

use std::num::NonZeroU64;

use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTreeCommitment};
use sha2::{Digest, Sha256};

use super::DiagnosticRegisterEvent;
use crate::{REGISTER_MERKLE_PATH_DEPTH, VMError};

pub(super) fn check(event: DiagnosticRegisterEvent<'_>) -> Result<(), VMError> {
    let leaf_index = u32::try_from(event.index)
        .ok()
        .filter(|index| *index < 256)
        .ok_or(VMError::AssertionFailed)?;
    let mut leaf = [0u8; 9];
    leaf[0] = u8::from(event.tag);
    leaf[1..].copy_from_slice(&event.value.to_le_bytes());
    let mut leaf_hash = [0u8; 32];
    leaf_hash.copy_from_slice(&Sha256::digest(leaf));
    iroha_crypto::zeroize_value_for_confidential_discard(&mut leaf);
    let leaf = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf_hash));
    // Detached diagnostic views remain untrusted even though live register events
    // own exactly eight siblings. Validate the view before copying fixed storage.
    let path: &[[u8; 32]; REGISTER_MERKLE_PATH_DEPTH] = event
        .path
        .try_into()
        .map_err(|_| VMError::AssertionFailed)?;
    let siblings = path.map(|sibling| {
        (sibling != [0; 32]).then(|| HashOf::from_untyped_unchecked(Hash::prehashed(sibling)))
    });
    let commitment = MerkleTreeCommitment::new(
        HashOf::from_untyped_unchecked(Hash::prehashed(*event.root)),
        NonZeroU64::new(256).expect("register tree leaf count is non-zero"),
    );
    if MerkleProof::verify_audit_path_sha256(leaf_index, &siblings, &leaf, &commitment) {
        Ok(())
    } else {
        Err(VMError::AssertionFailed)
    }
}
