//! Confidential-asset helpers shared across host and VM components.
//!
//! The routines in this module intentionally avoid depending on the wider
//! data-model crates so they remain usable from lightweight environments
//! (e.g., wallets or signing services) while still matching the canonical
//! on-chain derivations.

use crate::poseidon;

/// Domain-separation tag for confidential nullifier derivation.
const NULLIFIER_DST: &[u8] = b"iroha:conf:nullifier:v1";

/// Derive a canonical 32-byte nullifier from the nullifier key `nk`,
/// per-note randomness `rho`, the asset identifier, and the chain id.
///
/// Inputs follow the design captured in `docs/source/confidential_assets.md`.
/// All callers must supply the *canonical* UTF-8 representations for the asset
/// identifier and chain id (matching the values hashed into block headers).
///
/// The derivation is:
/// ```text
/// Poseidon2_256(
///     NULLIFIER_DST || 0x00 ||
///     nk[32] || rho[32] ||
///     len(asset_id)[u32 LE] || asset_id ||
///     len(chain_id)[u32 LE] || chain_id
/// )
/// ```
/// The nullifier is returned in canonical little-endian byte order matching the
/// BN254 scalar representation used by the proving system.
#[must_use]
pub fn derive_nullifier(
    nk: &[u8; 32],
    rho: &[u8; 32],
    asset_id: impl AsRef<[u8]>,
    chain_id: impl AsRef<[u8]>,
) -> [u8; 32] {
    let asset_id = asset_id.as_ref();
    let chain_id = chain_id.as_ref();

    let mut buf = Vec::with_capacity(
        NULLIFIER_DST.len()
            + 1
            + nk.len()
            + rho.len()
            + core::mem::size_of::<u32>()
            + asset_id.len()
            + core::mem::size_of::<u32>()
            + chain_id.len(),
    );
    buf.extend_from_slice(NULLIFIER_DST);
    buf.push(0x00); // explicit separator to guard against accidental DST tweaks
    buf.extend_from_slice(nk);
    buf.extend_from_slice(rho);
    buf.extend_from_slice(&(asset_id.len() as u32).to_le_bytes());
    buf.extend_from_slice(asset_id);
    buf.extend_from_slice(&(chain_id.len() as u32).to_le_bytes());
    buf.extend_from_slice(chain_id);
    poseidon::hash_bytes(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NK: [u8; 32] = [0xA5; 32];
    const RHO: [u8; 32] = [0x5A; 32];

    #[test]
    fn derive_nullifier_is_deterministic() {
        let asset_id = b"shielded#chain";
        let chain_id = b"alphanet";

        let first = derive_nullifier(&NK, &RHO, asset_id, chain_id);
        let second = derive_nullifier(&NK, &RHO, asset_id, chain_id);
        assert_eq!(
            first, second,
            "nullifier derivation must be deterministic for identical inputs"
        );
    }

    #[test]
    fn derive_nullifier_domain_separates_inputs() {
        let asset_id_a = b"asset#a";
        let asset_id_b = b"asset#b";
        let chain_id = b"iroha-main";

        let nullifier_a = derive_nullifier(&NK, &RHO, asset_id_a, chain_id);
        let nullifier_b = derive_nullifier(&NK, &RHO, asset_id_b, chain_id);
        assert_ne!(
            nullifier_a, nullifier_b,
            "distinct asset ids must yield distinct nullifiers"
        );

        let nullifier_chain = derive_nullifier(&NK, &RHO, asset_id_a, b"iroha-test");
        assert_ne!(
            nullifier_a, nullifier_chain,
            "chain id must participate in nullifier derivation"
        );
    }
}
