//! Shared BN254 Poseidon width-3 parameter staging for GPU hosts.

use std::sync::OnceLock;

/// Number of 64-bit limbs in a canonical BN254 field element.
pub(crate) const BN254_LIMBS: usize = 4;
/// Poseidon width used for transcript digest hashing.
pub(crate) const BN254_POSEIDON_WIDTH: usize = 3;

/// BN254 Poseidon width-3 constants flattened as canonical little-endian limbs.
pub(crate) struct Bn254PoseidonWidth3Params {
    /// Round constants, flattened as `[round][word][limb]`.
    pub(crate) round_constants: Box<[u64]>,
    /// MDS matrix, flattened as `[row][column][limb]`.
    pub(crate) mds: Box<[u64]>,
    /// Total round count.
    pub(crate) round_count: u32,
}

/// Return the shared BN254 Poseidon width-3 parameters in canonical limb form.
pub(crate) fn bn254_poseidon_width3_params() -> &'static Bn254PoseidonWidth3Params {
    static PARAMS: OnceLock<Bn254PoseidonWidth3Params> = OnceLock::new();
    PARAMS.get_or_init(|| {
        let params = iroha_zkp_halo2::poseidon::poseidon2_params_width3();
        let mut round_constants = Vec::with_capacity(
            params
                .round_constants
                .len()
                .saturating_mul(BN254_POSEIDON_WIDTH)
                .saturating_mul(BN254_LIMBS),
        );
        for round in &params.round_constants {
            for word in round {
                round_constants.extend_from_slice(&bn254_bytes_to_limbs(word));
            }
        }
        let mut mds = Vec::with_capacity(BN254_POSEIDON_WIDTH * BN254_POSEIDON_WIDTH * BN254_LIMBS);
        for row in &params.mds {
            for coeff in row {
                mds.extend_from_slice(&bn254_bytes_to_limbs(coeff));
            }
        }
        Bn254PoseidonWidth3Params {
            round_constants: round_constants.into_boxed_slice(),
            mds: mds.into_boxed_slice(),
            round_count: u32::try_from(params.round_constants.len())
                .expect("Poseidon round count must fit into u32"),
        }
    })
}

/// Convert canonical BN254 bytes into little-endian 64-bit limbs.
pub(crate) fn bn254_bytes_to_limbs(bytes: &[u8; 32]) -> [u64; BN254_LIMBS] {
    let mut limbs = [0u64; BN254_LIMBS];
    for (index, limb) in limbs.iter_mut().enumerate() {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes[index * 8..(index + 1) * 8]);
        *limb = u64::from_le_bytes(buf);
    }
    limbs
}

/// Convert little-endian BN254 limbs into canonical bytes.
pub(crate) fn bn254_limbs_to_bytes(limbs: &[u64]) -> [u8; 32] {
    debug_assert_eq!(limbs.len(), BN254_LIMBS);
    let mut out = [0u8; 32];
    for (index, limb) in limbs.iter().enumerate() {
        out[index * 8..(index + 1) * 8].copy_from_slice(&limb.to_le_bytes());
    }
    out
}
