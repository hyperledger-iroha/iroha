//! BLS-based Verifiable Random Function (VRF)
//!
//! Design
//! - Proof: BLS signature bytes `sigma` over `msg = Hash(b"iroha:vrf:v1:input" || input)`.
//! - Output: `y = Hash(b"iroha:vrf:v1:output" || sigma)`.
//! - Hash is the canonical `iroha_crypto::Hash` (Blake2b-256). This separates
//!   the VRF transcript from regular signatures and is deterministic across
//!   hardware.
//! - Canonicalization: `sigma` MUST be the canonical compressed encoding for
//!   the chosen variant (G1: 48 bytes, G2: 96 bytes). Non-canonical encodings,
//!   infinity, or wrong-subgroup points must be rejected during verification.
//!
//! Prover uses VRF‑specific DSTs via blstrs for both variants, deriving the
//! scalar from the 32‑byte secret key bytes deterministically.
//!
//! Keys
//! - Uses the existing BLS key types and backends selected by the `bls` feature.
//! - Supports both Normal and Small variants transparently via the generic
//!   implementation in `signature::bls`.
//!
//! Serialization
//! - `VrfProof` and `VrfOutput` derive Norito codecs for on-wire stability.
//! - Consumers should always pass/return raw bytes (`Blob`) over the pointer‑ABI
//!   and Norito‑encode higher-level envelopes as needed.

#![allow(clippy::size_of_ref)]
use core::convert::TryInto;
use std::vec::Vec;

use group::{Curve, prime::PrimeCurveAffine};
#[allow(unused_imports)]
use w3f_bls::SerializableToBytes as _;

use crate::{
    hash::Hash,
    signature::bls::{
        BlsNormalPrivateKey, BlsNormalPublicKey, BlsSmallPrivateKey, BlsSmallPublicKey,
    },
};

// Domain separation tags (DST) for VRF hash_to_curve operations
const DST_G2: &[u8] = b"BLS12381G2_XMD:SHA-256_SSWU_RO_IROHA_VRF_V1";
const DST_G1: &[u8] = b"BLS12381G1_XMD:SHA-256_SSWU_RO_IROHA_VRF_V1";

/// VRF proof: variant-tagged, fixed-size BLS signature bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
pub enum VrfProof {
    /// Signature in G1 (48 bytes) — corresponds to `Small`/`sig_in_G1` variant.
    SigInG1([u8; 48]),
    /// Signature in G2 (96 bytes) — corresponds to `Normal`/`sig_in_G2` variant.
    SigInG2([u8; 96]),
}

/// VRF output: 32-byte Blake2b digest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
pub struct VrfOutput(pub [u8; 32]);

/// VRF algorithm variant over BLS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VrfBlsVariant {
    /// Public key in G1, signature in G2 (normal BLS).
    Normal,
    /// Public key in G2, signature in G1 (small signature BLS).
    Small,
}

fn prehash_input_with_chain(chain_id: &[u8], input: &[u8]) -> Vec<u8> {
    // DS: "iroha:vrf:v1:input|<chain_id>|<input>"
    let mut buf =
        Vec::with_capacity(b"iroha:vrf:v1:input|".len() + chain_id.len() + 1 + input.len());
    buf.extend_from_slice(b"iroha:vrf:v1:input|");
    buf.extend_from_slice(chain_id);
    buf.push(b'|');
    buf.extend_from_slice(input);
    let h: [u8; 32] = Hash::new(&buf).into();
    h.to_vec()
}

fn output_from_sigma(sigma: &[u8]) -> VrfOutput {
    let mut buf = Vec::with_capacity(b"iroha:vrf:v1:output".len() + sigma.len());
    buf.extend_from_slice(b"iroha:vrf:v1:output");
    buf.extend_from_slice(sigma);
    VrfOutput(*Hash::new(&buf).as_ref())
}

/// Produce a VRF proof and output using the BLS Normal variant.
pub fn prove_normal_with_chain(
    sk: &BlsNormalPrivateKey,
    chain_id: &[u8],
    input: &[u8],
) -> (VrfOutput, VrfProof) {
    use blstrs::{G2Projective, Scalar};
    let msg = prehash_input_with_chain(chain_id, input);
    let sk_bytes = sk.to_bytes();
    let sk_slice: &[u8; 32] = (&sk_bytes[..])
        .try_into()
        .expect("BLS secret keys must be 32 bytes");
    let x = Scalar::from_bytes_le(sk_slice).unwrap();
    let h = G2Projective::hash_to_curve(&msg, DST_G2, &[]);
    let sig = (h * x).to_affine().to_compressed();
    let y = output_from_sigma(&sig);
    let mut arr = [0u8; 96];
    arr.copy_from_slice(&sig);
    (y, VrfProof::SigInG2(arr))
}

/// Produce a VRF proof and output using the BLS Normal variant with an empty chain-id.
/// The proof is a signature in G2 over a pre-hashed input; the output is a 32-byte digest.
pub fn prove_normal(sk: &BlsNormalPrivateKey, input: &[u8]) -> (VrfOutput, VrfProof) {
    prove_normal_with_chain(sk, &[], input)
}

/// Produce a VRF proof and output using the BLS Small variant.
pub fn prove_small_with_chain(
    sk: &BlsSmallPrivateKey,
    chain_id: &[u8],
    input: &[u8],
) -> (VrfOutput, VrfProof) {
    use blstrs::{G1Projective, Scalar};
    let msg = prehash_input_with_chain(chain_id, input);
    let sk_bytes = sk.to_bytes();
    let sk_slice: &[u8; 32] = (&sk_bytes[..])
        .try_into()
        .expect("BLS secret keys must be 32 bytes");
    let x = Scalar::from_bytes_le(sk_slice).unwrap();
    let h = G1Projective::hash_to_curve(&msg, DST_G1, &[]);
    let sig = (h * x).to_affine().to_compressed();
    let y = output_from_sigma(&sig);
    let mut arr = [0u8; 48];
    arr.copy_from_slice(&sig);
    (y, VrfProof::SigInG1(arr))
}

/// Produce a VRF proof and output using the BLS Small variant with an empty chain-id.
/// The proof is a signature in G1 over a pre-hashed input; the output is a 32-byte digest.
pub fn prove_small(sk: &BlsSmallPrivateKey, input: &[u8]) -> (VrfOutput, VrfProof) {
    prove_small_with_chain(sk, &[], input)
}

/// Verify a VRF proof under the BLS Normal public key and recompute the output.
pub fn verify_normal_with_chain(
    pk: &BlsNormalPublicKey,
    chain_id: &[u8],
    input: &[u8],
    proof: &VrfProof,
) -> Option<VrfOutput> {
    let msg = prehash_input_with_chain(chain_id, input);
    match proof {
        VrfProof::SigInG2(sig) => {
            verify_vrf_normal_pairing(pk, &msg, sig).then(|| output_from_sigma(sig))
        }
        _ => None,
    }
}

/// Verify a VRF proof under the BLS Normal public key with an empty chain-id.
/// Returns the derived output if proof verification succeeds; otherwise `None`.
pub fn verify_normal(pk: &BlsNormalPublicKey, input: &[u8], proof: &VrfProof) -> Option<VrfOutput> {
    verify_normal_with_chain(pk, &[], input, proof)
}

/// Verify a VRF proof under the BLS Small public key and recompute the output.
pub fn verify_small_with_chain(
    pk: &BlsSmallPublicKey,
    chain_id: &[u8],
    input: &[u8],
    proof: &VrfProof,
) -> Option<VrfOutput> {
    let msg = prehash_input_with_chain(chain_id, input);
    match proof {
        VrfProof::SigInG1(sig) => {
            verify_vrf_small_pairing(pk, &msg, sig).then(|| output_from_sigma(sig))
        }
        _ => None,
    }
}

/// Verify a VRF proof under the BLS Small public key with an empty chain-id.
/// Returns the derived output if proof verification succeeds; otherwise `None`.
pub fn verify_small(pk: &BlsSmallPublicKey, input: &[u8], proof: &VrfProof) -> Option<VrfOutput> {
    verify_small_with_chain(pk, &[], input, proof)
}

/// Derive VRF output from a canonical proof encoding without re-verification.
pub fn output_from_proof(proof: &VrfProof) -> VrfOutput {
    match proof {
        VrfProof::SigInG1(sig) => output_from_sigma(sig),
        VrfProof::SigInG2(sig) => output_from_sigma(sig),
    }
}

fn verify_vrf_normal_pairing(pk: &BlsNormalPublicKey, msg: &[u8], sig: &[u8; 96]) -> bool {
    use blstrs::{G1Affine, G1Projective, G2Prepared};
    use group::{Curve, Group as _, prime::PrimeCurveAffine};
    use pairing::{MillerLoopResult as _, MultiMillerLoop as _};

    // pk in G1, signature in G2
    let pk_bytes = pk.to_bytes();
    let Some(pk) = to_g1(&pk_bytes) else {
        return false;
    };
    let Some(sig) = to_g2(sig) else {
        return false;
    };
    let h = hash_msg_to_g2(msg);
    let terms: [(&G1Affine, &G2Prepared); 2] = [
        (&G1Affine::generator(), &G2Prepared::from(sig)),
        (&(-G1Projective::from(pk)).to_affine(), &G2Prepared::from(h)),
    ];
    let gt = blstrs::Bls12::multi_miller_loop(&terms).final_exponentiation();
    gt.is_identity().into()
}

fn verify_vrf_small_pairing(pk: &BlsSmallPublicKey, msg: &[u8], sig: &[u8; 48]) -> bool {
    use blstrs::{G1Affine, G1Projective, G2Affine, G2Prepared};
    use group::{Curve, Group as _, prime::PrimeCurveAffine};
    use pairing::{MillerLoopResult as _, MultiMillerLoop as _};

    // pk in G2, signature in G1
    let pk_bytes = pk.to_bytes();
    let Some(pk) = to_g2(&pk_bytes) else {
        return false;
    };
    let Some(sig) = to_g1(sig) else {
        return false;
    };
    let h = hash_msg_to_g1(msg);
    let terms: [(&G1Affine, &G2Prepared); 2] = [
        (&sig, &G2Prepared::from(G2Affine::generator())),
        (&(-G1Projective::from(h)).to_affine(), &G2Prepared::from(pk)),
    ];
    let gt = blstrs::Bls12::multi_miller_loop(&terms).final_exponentiation();
    gt.is_identity().into()
}

fn to_g1(bytes: &[u8]) -> Option<blstrs::G1Affine> {
    if bytes.len() != 48 {
        return None;
    }
    let mut arr = [0u8; 48];
    arr.copy_from_slice(bytes);
    let ct = blstrs::G1Affine::from_compressed(&arr);
    if !bool::from(ct.is_some()) {
        return None;
    }
    let point = ct.unwrap();
    if point.is_identity().into() {
        return None;
    }
    if point.to_compressed() != arr {
        return None;
    }
    Some(point)
}

fn to_g2(bytes: &[u8]) -> Option<blstrs::G2Affine> {
    if bytes.len() != 96 {
        return None;
    }
    let mut arr = [0u8; 96];
    arr.copy_from_slice(bytes);
    let ct = blstrs::G2Affine::from_compressed(&arr);
    if !bool::from(ct.is_some()) {
        return None;
    }
    let point = ct.unwrap();
    if point.is_identity().into() {
        return None;
    }
    if point.to_compressed() != arr {
        return None;
    }
    Some(point)
}

fn hash_msg_to_g2(msg: &[u8]) -> blstrs::G2Affine {
    use blstrs::G2Projective;
    use group::Curve;
    G2Projective::hash_to_curve(msg, DST_G2, &[]).to_affine()
}

fn hash_msg_to_g1(msg: &[u8]) -> blstrs::G1Affine {
    use blstrs::G1Projective;
    use group::Curve;
    G1Projective::hash_to_curve(msg, DST_G1, &[]).to_affine()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signature::bls;
    use blstrs::{G1Affine, G2Affine};
    use group::prime::PrimeCurveAffine;

    #[test]
    fn vrf_normal_roundtrip() {
        let (_pk, sk) = bls::BlsNormal::keypair(crate::KeyGenOption::UseSeed(vec![1, 2, 3, 4]));
        let input = b"vrf:test:normal";
        let chain = b"test-chain";
        let (y1, pi) = prove_normal_with_chain(&sk, chain, input);
        // Re-derive pk from sk for verification
        let (pk2, _sk2) = bls::BlsNormal::keypair(crate::KeyGenOption::FromPrivateKey(sk.clone()));
        let y2 = verify_normal_with_chain(&pk2, chain, input, &pi).expect("valid proof");
        assert_eq!(y1, y2);
        // Output-only derivation is consistent
        assert_eq!(y1, output_from_proof(&pi));
    }

    #[test]
    fn vrf_small_roundtrip() {
        let (_pk, sk) = bls::BlsSmall::keypair(crate::KeyGenOption::UseSeed(vec![5, 6, 7, 8]));
        let input = b"vrf:test:small";
        let chain = b"test-chain";
        let (y1, pi) = prove_small_with_chain(&sk, chain, input);
        let (pk2, _sk2) = bls::BlsSmall::keypair(crate::KeyGenOption::FromPrivateKey(sk.clone()));
        let y2 = verify_small_with_chain(&pk2, chain, input, &pi).expect("valid proof");
        assert_eq!(y1, y2);
        assert_eq!(y1, output_from_proof(&pi));
    }

    #[test]
    fn cross_protocol_vrf_not_regular_bls() {
        // Normal variant
        let (_pk, sk) = bls::BlsNormal::keypair(crate::KeyGenOption::UseSeed(vec![9, 9, 9]));
        let chain = b"chain-A";
        let input = b"input";
        let (y, pi) = prove_normal_with_chain(&sk, chain, input);
        assert!(y.0 != [0u8; 32]);
        let (pk, _sk2) = bls::BlsNormal::keypair(crate::KeyGenOption::FromPrivateKey(sk.clone()));
        // Attempt to verify VRF proof as a regular BLS signature
        if let VrfProof::SigInG2(sig) = pi {
            // Use the VRF prehash as the "message"; w3f-bls still uses a different DST.
            let msg = prehash_input_with_chain(chain, input);
            let ok = bls::BlsNormal::verify(&msg, &sig, &pk).is_ok();
            assert!(
                !ok,
                "VRF proof must not verify under regular BLS signature verify"
            );
        }
    }

    #[test]
    fn cross_protocol_regular_bls_not_vrf() {
        // Normal variant
        let (_pk, sk) = bls::BlsNormal::keypair(crate::KeyGenOption::UseSeed(vec![7, 7, 7]));
        let msg = b"regular-sign";
        let sig = bls::BlsNormal::sign(msg, &sk);
        let mut arr = [0u8; 96];
        arr.copy_from_slice(&sig);
        let proof = VrfProof::SigInG2(arr);
        let (pk, _sk2) = bls::BlsNormal::keypair(crate::KeyGenOption::FromPrivateKey(sk));
        let chain = b"chain-A";
        let input = b"input";
        let out = verify_normal_with_chain(&pk, chain, input, &proof);
        assert!(
            out.is_none(),
            "regular BLS signature must not pass VRF verify"
        );
    }

    #[test]
    fn cross_protocol_vrf_not_regular_bls_small() {
        // Small variant (SigInG1)
        let (_pk, sk) = bls::BlsSmall::keypair(crate::KeyGenOption::UseSeed(vec![3, 3, 3]));
        let chain = b"chain-B";
        let input = b"inputB";
        let (y, pi) = prove_small_with_chain(&sk, chain, input);
        assert!(y.0 != [0u8; 32]);
        let (pk, _sk2) = bls::BlsSmall::keypair(crate::KeyGenOption::FromPrivateKey(sk.clone()));
        if let VrfProof::SigInG1(sig) = pi {
            let msg = prehash_input_with_chain(chain, input);
            let ok = bls::BlsSmall::verify(&msg, &sig, &pk).is_ok();
            assert!(
                !ok,
                "VRF proof (G1) must not verify under regular BLS Small verify"
            );
        }
    }

    #[test]
    fn cross_protocol_regular_bls_not_vrf_small() {
        let (_pk, sk) = bls::BlsSmall::keypair(crate::KeyGenOption::UseSeed(vec![4, 4, 4]));
        let msg = b"regular-sign-small";
        let sig = bls::BlsSmall::sign(msg, &sk);
        let mut arr = [0u8; 48];
        arr.copy_from_slice(&sig);
        let proof = VrfProof::SigInG1(arr);
        let (pk, _sk2) = bls::BlsSmall::keypair(crate::KeyGenOption::FromPrivateKey(sk));
        let chain = b"chain-B";
        let input = b"inputB";
        let out = verify_small_with_chain(&pk, chain, input, &proof);
        assert!(
            out.is_none(),
            "regular BLS Small signature must not pass VRF verify"
        );
    }

    #[test]
    fn vrf_rejects_identity_signature_normal() {
        let (pk, _sk) = bls::BlsNormal::keypair(crate::KeyGenOption::UseSeed(vec![1, 2, 3, 4]));
        let identity = G2Affine::identity().to_compressed();
        let mut sig = [0u8; 96];
        sig.copy_from_slice(&identity);
        let proof = VrfProof::SigInG2(sig);
        let out = verify_normal_with_chain(&pk, b"chain", b"input", &proof);
        assert!(out.is_none(), "identity signature must be rejected");
    }

    #[test]
    fn vrf_rejects_identity_signature_small() {
        let (pk, _sk) = bls::BlsSmall::keypair(crate::KeyGenOption::UseSeed(vec![5, 6, 7, 8]));
        let identity = G1Affine::identity().to_compressed();
        let mut sig = [0u8; 48];
        sig.copy_from_slice(&identity);
        let proof = VrfProof::SigInG1(sig);
        let out = verify_small_with_chain(&pk, b"chain", b"input", &proof);
        assert!(out.is_none(), "identity signature must be rejected");
    }

    #[test]
    fn vrf_to_g1_rejects_invalid_length() {
        assert!(to_g1(&[0u8; 47]).is_none());
        assert!(to_g1(&[0u8; 49]).is_none());
    }

    #[test]
    fn vrf_to_g2_rejects_invalid_length() {
        assert!(to_g2(&[0u8; 95]).is_none());
        assert!(to_g2(&[0u8; 97]).is_none());
    }
}
