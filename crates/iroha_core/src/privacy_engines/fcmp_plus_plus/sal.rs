//! Spend-Authorization-and-Linkability proof for FCMP++.
//!
//! This is a concrete native port of the BP+/GSP conjunction used by Monero
//! FCMP++.  The transcript and proof order intentionally match the pinned
//! upstream construction.  The caller-supplied context hash must already bind
//! the complete transaction statement (including the typed root, namespace,
//! pseudo-outs, outputs, and network/domain); changing it invalidates the
//! proof and prevents cross-statement replay.
use super::{
    FCMP_POINT_BYTES_V1, FcmpNativeErrorV1,
    field::{decode_edwards_point, validate_edwards_scalar},
    wire::FcmpProofInputPublicV1,
};
use blake2::{Blake2b512, Digest as _};
use curve25519_dalek::{
    constants::ED25519_BASEPOINT_POINT,
    edwards::EdwardsPoint,
    scalar::Scalar,
    traits::{Identity as _, IsIdentity as _},
};
use p256::elliptic_curve::subtle::{Choice, ConstantTimeLess as _};
use rand_core_06::{CryptoRng, RngCore};
use std::sync::OnceLock;
use zeroize::Zeroize;
const SAL_POINT_COUNT_V1: usize = 6;
const SAL_SCALAR_COUNT_V1: usize = 6;
const MAX_SAL_SCALAR_ATTEMPTS_V1: usize = 128;
const MAX_SAL_PROVER_RESTARTS_V1: usize = 128;
static ED25519_SCALAR_MODULUS_LE_V1: [u8; 32] = [
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
];
/// Exact canonical SAL proof width.
pub const FCMP_SAL_PROOF_BYTES_V1: usize =
    (SAL_POINT_COUNT_V1 + SAL_SCALAR_COUNT_V1) * FCMP_POINT_BYTES_V1;
struct SalSecretCopyValueV1<T: Copy + Zeroize>(T);
struct BorrowedSalCopySlotV1<'a, T: Copy + Zeroize>(&'a mut T);
#[cfg(test)]
std::thread_local! {
    static SAL_SECRET_COPY_OWNER_DROPS_V1: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static FCMP_SAL_WITNESS_OWNER_DROPS_V1: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static SAL_SECRET_CANONICALITY_STATE_OWNER_DROPS_V1: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static SAL_SECRET_WIDE_INPUT_OWNER_DROPS_V1: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}
impl<T: Copy + Zeroize> BorrowedSalCopySlotV1<'_, T> {
    fn expose_copy(&self) -> T {
        *self.0
    }
}
impl<T: Copy + Zeroize> Drop for BorrowedSalCopySlotV1<'_, T> {
    fn drop(&mut self) {
        self.0.zeroize();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *self.0);
    }
}
impl<T: Copy + Zeroize> SalSecretCopyValueV1<T> {
    #[cfg(test)]
    fn new(mut value: T) -> Self {
        Self::take(&mut value)
    }
    fn take(value: &mut T) -> Self {
        let incoming = BorrowedSalCopySlotV1(value);
        let owned = Self(incoming.expose_copy());
        drop(incoming);
        owned
    }
    fn expose_ref(&self) -> &T {
        &self.0
    }
}
impl<T: Copy + Zeroize> Drop for SalSecretCopyValueV1<T> {
    fn drop(&mut self) {
        self.0.zeroize();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.0);
        #[cfg(test)]
        let _ = SAL_SECRET_COPY_OWNER_DROPS_V1.try_with(|drops| {
            drops.set(drops.get().saturating_add(1));
        });
    }
}
struct SalSecretScalarCanonicalityStateV1 {
    less: Choice,
    greater: Choice,
    byte_less: Choice,
    byte_greater: Choice,
    prefix_decided: Choice,
    prefix_equal: Choice,
    less_update: Choice,
    greater_update: Choice,
}
impl SalSecretScalarCanonicalityStateV1 {
    fn new_v1() -> Self {
        Self {
            less: Choice::from(0),
            greater: Choice::from(0),
            byte_less: Choice::from(0),
            byte_greater: Choice::from(0),
            prefix_decided: Choice::from(0),
            prefix_equal: Choice::from(0),
            less_update: Choice::from(0),
            greater_update: Choice::from(0),
        }
    }
    fn observe_byte_v1(&mut self, byte: &u8, modulus_byte: &u8) {
        self.prefix_decided = self.less | self.greater;
        self.prefix_equal = !self.prefix_decided;
        self.byte_less = byte.ct_lt(modulus_byte);
        self.byte_greater = modulus_byte.ct_lt(byte);
        self.less_update = self.prefix_equal & self.byte_less;
        self.greater_update = self.prefix_equal & self.byte_greater;
        self.less |= self.less_update;
        self.greater |= self.greater_update;
    }
}
impl Drop for SalSecretScalarCanonicalityStateV1 {
    fn drop(&mut self) {
        self.less = Choice::from(0);
        self.greater = Choice::from(0);
        self.byte_less = Choice::from(0);
        self.byte_greater = Choice::from(0);
        self.prefix_decided = Choice::from(0);
        self.prefix_equal = Choice::from(0);
        self.less_update = Choice::from(0);
        self.greater_update = Choice::from(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.less);
        let _ = core::hint::black_box(&mut self.greater);
        let _ = core::hint::black_box(&mut self.byte_less);
        let _ = core::hint::black_box(&mut self.byte_greater);
        let _ = core::hint::black_box(&mut self.prefix_decided);
        let _ = core::hint::black_box(&mut self.prefix_equal);
        let _ = core::hint::black_box(&mut self.less_update);
        let _ = core::hint::black_box(&mut self.greater_update);
        #[cfg(test)]
        let _ = SAL_SECRET_CANONICALITY_STATE_OWNER_DROPS_V1.try_with(|drops| {
            drops.set(drops.get().saturating_add(1));
        });
    }
}
struct SalSecretScalarWideInputV1([u8; 64]);
impl SalSecretScalarWideInputV1 {
    fn from_borrowed_v1(bytes: &[u8; 32]) -> Self {
        let mut wide = Self([0_u8; 64]);
        wide.0[..32].copy_from_slice(bytes);
        wide
    }
}
impl Drop for SalSecretScalarWideInputV1 {
    fn drop(&mut self) {
        self.0.zeroize();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.0);
        #[cfg(test)]
        let _ = SAL_SECRET_WIDE_INPUT_OWNER_DROPS_V1.try_with(|drops| {
            drops.set(drops.get().saturating_add(1));
        });
    }
}
pub(super) struct FcmpSalSecretScalarEncodingV1(SalSecretCopyValueV1<[u8; 32]>);
impl FcmpSalSecretScalarEncodingV1 {
    pub(super) fn from_scalar_ref_v1(scalar: &Scalar) -> Self {
        let mut encoded = scalar.to_bytes();
        Self::take(&mut encoded)
    }
    fn take(encoded: &mut [u8; 32]) -> Self {
        Self(SalSecretCopyValueV1::take(encoded))
    }
    #[cfg(test)]
    pub(super) fn from_test_bytes_v1(mut encoded: [u8; 32]) -> Self {
        Self::take(&mut encoded)
    }
}
#[cfg(test)]
pub(super) fn reset_sal_secret_copy_owner_drops_v1() {
    SAL_SECRET_COPY_OWNER_DROPS_V1.with(|drops| drops.set(0));
}
#[cfg(test)]
pub(super) fn sal_secret_copy_owner_drops_v1() -> usize {
    SAL_SECRET_COPY_OWNER_DROPS_V1.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(super) fn reset_fcmp_sal_witness_owner_drops_v1() {
    FCMP_SAL_WITNESS_OWNER_DROPS_V1.with(|drops| drops.set(0));
}
#[cfg(test)]
pub(super) fn fcmp_sal_witness_owner_drops_v1() -> usize {
    FCMP_SAL_WITNESS_OWNER_DROPS_V1.with(std::cell::Cell::get)
}
// Generated by the pinned Monero `hash_to_ec` construction. Keeping the
// canonical encodings here avoids silently substituting a generic
// hash-to-curve algorithm with different semantics.
const MONERO_T_BYTES: [u8; 32] = [
    0x96, 0x6f, 0xc6, 0x6b, 0x82, 0xcd, 0x56, 0xcf, 0x85, 0xea, 0xec, 0x80, 0x1c, 0x42, 0x84, 0x5f,
    0x5f, 0x40, 0x88, 0x78, 0xd1, 0x56, 0x1e, 0x00, 0xd3, 0xd7, 0xde, 0xd2, 0x79, 0x4d, 0x09, 0x4f,
];
const MONERO_FCMP_U_BYTES: [u8; 32] = [
    0x09, 0x75, 0x9c, 0x17, 0xc9, 0x07, 0xf7, 0x16, 0xa2, 0x0b, 0x1a, 0xec, 0x5c, 0xc3, 0xaf, 0xfd,
    0xe7, 0xf3, 0xa1, 0xb9, 0x14, 0x6b, 0x5a, 0xf2, 0x8c, 0xb7, 0xaf, 0x0a, 0xf4, 0x7a, 0x00, 0x66,
];
const MONERO_FCMP_V_BYTES: [u8; 32] = [
    0x32, 0xb4, 0xd2, 0x9f, 0x2a, 0x80, 0x55, 0x69, 0xd9, 0x59, 0xd2, 0x44, 0x96, 0xed, 0x41, 0x1e,
    0x87, 0x91, 0x26, 0xd8, 0xf5, 0x2c, 0x1e, 0xcd, 0x86, 0x4d, 0xb9, 0x02, 0xb5, 0x81, 0x33, 0xe0,
];
fn fixed_generator(cell: &OnceLock<EdwardsPoint>, bytes: [u8; 32]) -> EdwardsPoint {
    *cell.get_or_init(|| {
        decode_edwards_point(bytes, false).expect("pinned Monero generator is canonical")
    })
}
pub(super) fn generator_t() -> EdwardsPoint {
    static CELL: OnceLock<EdwardsPoint> = OnceLock::new();
    fixed_generator(&CELL, MONERO_T_BYTES)
}
pub(super) fn generator_u() -> EdwardsPoint {
    static CELL: OnceLock<EdwardsPoint> = OnceLock::new();
    fixed_generator(&CELL, MONERO_FCMP_U_BYTES)
}
pub(super) fn generator_v() -> EdwardsPoint {
    static CELL: OnceLock<EdwardsPoint> = OnceLock::new();
    fixed_generator(&CELL, MONERO_FCMP_V_BYTES)
}
fn scalar_from_bytes(bytes: [u8; 32]) -> Result<Scalar, FcmpNativeErrorV1> {
    validate_edwards_scalar(bytes)?;
    Option::<Scalar>::from(Scalar::from_canonical_bytes(bytes))
        .ok_or(FcmpNativeErrorV1::ScalarEncoding)
}
fn secret_scalar_from_bytes_v1(
    bytes: &[u8; 32],
) -> Result<SalSecretCopyValueV1<Scalar>, FcmpNativeErrorV1> {
    let mut canonicality = SalSecretScalarCanonicalityStateV1::new_v1();
    let mut index = ED25519_SCALAR_MODULUS_LE_V1.len();
    while index != 0 {
        index -= 1;
        let byte = &bytes[index];
        let modulus_byte = &ED25519_SCALAR_MODULUS_LE_V1[index];
        canonicality.observe_byte_v1(byte, modulus_byte);
    }
    let is_canonical = bool::from(canonicality.less);
    drop(canonicality);
    if !is_canonical {
        return Err(FcmpNativeErrorV1::ScalarEncoding);
    }
    let wide = SalSecretScalarWideInputV1::from_borrowed_v1(bytes);
    let mut scalar = Scalar::from_bytes_mod_order_wide(&wide.0);
    drop(wide);
    let scalar = SalSecretCopyValueV1::take(&mut scalar);
    Ok(scalar)
}
fn random_scalar(rng: &mut (impl RngCore + CryptoRng)) -> Result<Scalar, FcmpNativeErrorV1> {
    // A zero alpha or beta is algebraically valid but makes the corresponding
    // response `e*x` or `e*r_i`, directly revealing the witness. Require every
    // SAL nonce/blinder to be non-zero and bound retries so a faulty
    // caller-supplied RNG cannot hang the prover.
    for _ in 0..MAX_SAL_SCALAR_ATTEMPTS_V1 {
        let mut wide = [0_u8; 64];
        if rng.try_fill_bytes(&mut wide).is_err() {
            wide.zeroize();
            return Err(FcmpNativeErrorV1::RandomnessUnavailable);
        }
        let scalar = Scalar::from_bytes_mod_order_wide(&wide);
        wide.zeroize();
        if scalar != Scalar::ZERO {
            return Ok(scalar);
        }
    }
    Err(FcmpNativeErrorV1::ProverRandomnessExhausted)
}
fn sum_terms(terms: &[(Scalar, EdwardsPoint)]) -> EdwardsPoint {
    terms
        .iter()
        .fold(EdwardsPoint::identity(), |sum, (scalar, point)| {
            sum + (point * scalar)
        })
}
/// Canonical proof object for the FCMP++ SAL conjunction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FcmpSalProofV1 {
    points: [[u8; 32]; SAL_POINT_COUNT_V1],
    scalars: [[u8; 32]; SAL_SCALAR_COUNT_V1],
}
impl FcmpSalProofV1 {
    /// Construct from the exact upstream point/scalar order.
    pub fn new(
        points: [[u8; 32]; SAL_POINT_COUNT_V1],
        scalars: [[u8; 32]; SAL_SCALAR_COUNT_V1],
    ) -> Result<Self, FcmpNativeErrorV1> {
        for point in points {
            decode_edwards_point(point, false)?;
        }
        for scalar in scalars {
            validate_edwards_scalar(scalar)?;
        }
        Ok(Self { points, scalars })
    }
    /// Decode exactly 384 bytes in `P,A,B,R_O,R_P,R_L` then response order.
    pub fn decode(bytes: &[u8]) -> Result<Self, FcmpNativeErrorV1> {
        if bytes.len() != FCMP_SAL_PROOF_BYTES_V1 {
            return Err(FcmpNativeErrorV1::ProofLength {
                actual: bytes.len(),
                expected: FCMP_SAL_PROOF_BYTES_V1,
            });
        }
        let mut cursor = 0;
        let mut points = [[0_u8; 32]; SAL_POINT_COUNT_V1];
        for point in &mut points {
            point.copy_from_slice(&bytes[cursor..cursor + 32]);
            cursor += 32;
        }
        let mut scalars = [[0_u8; 32]; SAL_SCALAR_COUNT_V1];
        for scalar in &mut scalars {
            scalar.copy_from_slice(&bytes[cursor..cursor + 32]);
            cursor += 32;
        }
        Self::new(points, scalars)
    }
    /// Encode in the canonical upstream order.
    pub fn encode(self) -> [u8; FCMP_SAL_PROOF_BYTES_V1] {
        let mut encoded = [0_u8; FCMP_SAL_PROOF_BYTES_V1];
        let mut cursor = 0;
        for point in self.points {
            encoded[cursor..cursor + 32].copy_from_slice(&point);
            cursor += 32;
        }
        for scalar in self.scalars {
            encoded[cursor..cursor + 32].copy_from_slice(&scalar);
            cursor += 32;
        }
        encoded
    }
    /// Return the exact six point and six scalar wire components.
    pub const fn components(self) -> ([[u8; 32]; 6], [[u8; 32]; 6]) {
        (self.points, self.scalars)
    }
}
/// Secret opening used to produce a SAL proof.
///
/// The constructor accepts canonical Ed25519 scalar encodings for `x`, `y`,
/// `r_i`, and `r_{r_i}`.  The prover checks that these open O~, R, and L
/// before emitting a proof.
#[derive(Clone)]
pub struct FcmpSalWitnessV1 {
    x: Scalar,
    y: Scalar,
    r_i: Scalar,
    r_r_i: Scalar,
}
impl Zeroize for FcmpSalWitnessV1 {
    fn zeroize(&mut self) {
        self.x.zeroize();
        self.y.zeroize();
        self.r_i.zeroize();
        self.r_r_i.zeroize();
    }
}
impl Drop for FcmpSalWitnessV1 {
    fn drop(&mut self) {
        self.zeroize();
        #[cfg(test)]
        let _ = FCMP_SAL_WITNESS_OWNER_DROPS_V1.try_with(|drops| {
            drops.set(drops.get().saturating_add(1));
        });
    }
}
impl core::fmt::Debug for FcmpSalWitnessV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FcmpSalWitnessV1")
            .finish_non_exhaustive()
    }
}
impl FcmpSalWitnessV1 {
    /// Construct a witness from four canonical scalars.
    pub fn new(
        mut x: [u8; 32],
        mut y: [u8; 32],
        mut r_i: [u8; 32],
        mut r_r_i: [u8; 32],
    ) -> Result<Self, FcmpNativeErrorV1> {
        let x_bytes = FcmpSalSecretScalarEncodingV1::take(&mut x);
        let y_bytes = FcmpSalSecretScalarEncodingV1::take(&mut y);
        let r_i_bytes = FcmpSalSecretScalarEncodingV1::take(&mut r_i);
        let r_r_i_bytes = FcmpSalSecretScalarEncodingV1::take(&mut r_r_i);
        Self::from_secret_scalar_encoding_owners_v1(x_bytes, y_bytes, r_i_bytes, r_r_i_bytes)
    }
    pub(super) fn from_secret_scalar_encoding_owners_v1(
        x_bytes: FcmpSalSecretScalarEncodingV1,
        y_bytes: FcmpSalSecretScalarEncodingV1,
        r_i_bytes: FcmpSalSecretScalarEncodingV1,
        r_r_i_bytes: FcmpSalSecretScalarEncodingV1,
    ) -> Result<Self, FcmpNativeErrorV1> {
        let mut x = secret_scalar_from_bytes_v1(x_bytes.0.expose_ref())?;
        let mut y = secret_scalar_from_bytes_v1(y_bytes.0.expose_ref())?;
        let mut r_i = secret_scalar_from_bytes_v1(r_i_bytes.0.expose_ref())?;
        let mut r_r_i = secret_scalar_from_bytes_v1(r_r_i_bytes.0.expose_ref())?;
        let mut witness = Self {
            x: Scalar::ZERO,
            y: Scalar::ZERO,
            r_i: Scalar::ZERO,
            r_r_i: Scalar::ZERO,
        };
        core::mem::swap(&mut witness.x, &mut x.0);
        drop(x);
        core::mem::swap(&mut witness.y, &mut y.0);
        drop(y);
        core::mem::swap(&mut witness.r_i, &mut r_i.0);
        drop(r_i);
        core::mem::swap(&mut witness.r_r_i, &mut r_r_i.0);
        drop(r_r_i);
        Ok(witness)
    }
}
#[allow(clippy::too_many_arguments)]
fn challenge(
    context_hash: [u8; 32],
    public: &FcmpProofInputPublicV1,
    points: &[EdwardsPoint; SAL_POINT_COUNT_V1],
) -> Result<Scalar, FcmpNativeErrorV1> {
    let mut transcript = Blake2b512::new();
    transcript.update(context_hash);
    transcript.update(public.output_key_tilde);
    transcript.update(public.linking_tag_generator_tilde);
    transcript.update(public.pseudo_out);
    transcript.update(public.rerandomization_commitment);
    transcript.update(public.key_image);
    for point in points {
        transcript.update(point.compress().to_bytes());
    }
    let challenge = Scalar::from_bytes_mod_order_wide(&transcript.finalize().into());
    if challenge == Scalar::ZERO {
        return Err(FcmpNativeErrorV1::SalChallengeZero);
    }
    Ok(challenge)
}
fn public_points(public: &FcmpProofInputPublicV1) -> Result<[EdwardsPoint; 5], FcmpNativeErrorV1> {
    Ok([
        decode_edwards_point(public.output_key_tilde, false)?,
        decode_edwards_point(public.linking_tag_generator_tilde, false)?,
        decode_edwards_point(public.rerandomization_commitment, false)?,
        decode_edwards_point(public.pseudo_out, false)?,
        decode_edwards_point(public.key_image, false)?,
    ])
}
fn validate_sal_witness_relation_v1(
    public: &FcmpProofInputPublicV1,
    witness: &FcmpSalWitnessV1,
) -> Result<(), FcmpNativeErrorV1> {
    let [o_tilde, i_tilde, r, _c_tilde, key_image] = public_points(public)?;
    let g = ED25519_BASEPOINT_POINT;
    let t = generator_t();
    let u = generator_u();
    let v = generator_v();
    let expected_o = (g * witness.x) + (t * witness.y);
    let expected_r = (v * witness.r_i) + (t * witness.r_r_i);
    let expected_l = (i_tilde * witness.x) - (u * (witness.x * witness.r_i));
    if expected_o != o_tilde || expected_r != r || expected_l != key_image {
        return Err(FcmpNativeErrorV1::SalWitnessMismatch);
    }
    Ok(())
}
/// Produce the canonical FCMP++ SAL proof.
///
/// `context_hash` must be the protocol-domain-separated digest of the complete
/// authoritative statement. The function rejects a witness that does not open
/// O~, R, and L exactly.
pub fn prove_fcmp_sal_v1(
    rng: &mut (impl RngCore + CryptoRng),
    context_hash: [u8; 32],
    public: &FcmpProofInputPublicV1,
    witness: &FcmpSalWitnessV1,
) -> Result<FcmpSalProofV1, FcmpNativeErrorV1> {
    validate_sal_witness_relation_v1(public, witness)?;
    let mut checked_rng = super::health_checked_fcmp_rng_v1(rng)?;
    let proof =
        prove_fcmp_sal_with_checked_rng_v1(&mut checked_rng, context_hash, public, witness)?;
    verify_fcmp_sal_v1(context_hash, public, &proof)
        .map_err(|_| FcmpNativeErrorV1::ProverSelfCheckFailed)?;
    Ok(proof)
}
pub(super) fn prove_fcmp_sal_with_checked_rng_v1(
    rng: &mut (impl RngCore + CryptoRng),
    context_hash: [u8; 32],
    public: &FcmpProofInputPublicV1,
    witness: &FcmpSalWitnessV1,
) -> Result<FcmpSalProofV1, FcmpNativeErrorV1> {
    validate_sal_witness_relation_v1(public, witness)?;
    retry_sal_prover_v1(|| prove_fcmp_sal_once_v1(rng, context_hash, public, witness))
}
fn retry_sal_prover_v1<T>(
    mut prove_once: impl FnMut() -> Result<T, FcmpNativeErrorV1>,
) -> Result<T, FcmpNativeErrorV1> {
    for _ in 0..MAX_SAL_PROVER_RESTARTS_V1 {
        match prove_once() {
            Ok(proof) => return Ok(proof),
            Err(FcmpNativeErrorV1::SalProofPointIdentity | FcmpNativeErrorV1::SalChallengeZero) => {
                continue;
            }
            Err(error) => return Err(error),
        }
    }
    Err(FcmpNativeErrorV1::SalProverRestartExhausted)
}
fn prove_fcmp_sal_once_v1(
    rng: &mut (impl RngCore + CryptoRng),
    context_hash: [u8; 32],
    public: &FcmpProofInputPublicV1,
    witness: &FcmpSalWitnessV1,
) -> Result<FcmpSalProofV1, FcmpNativeErrorV1> {
    let [_o_tilde, i_tilde, _r, _c_tilde, _key_image] = public_points(public)?;
    let g = ED25519_BASEPOINT_POINT;
    let t = generator_t();
    let u = generator_u();
    let v = generator_v();
    let x_r_i = witness.x * witness.r_i;
    let alpha = random_scalar(rng)?;
    let beta = random_scalar(rng)?;
    let delta = random_scalar(rng)?;
    let mu = random_scalar(rng)?;
    let r_y = random_scalar(rng)?;
    let r_z = random_scalar(rng)?;
    let r_p = random_scalar(rng)?;
    let r_r_p = random_scalar(rng)?;
    let p = (g * witness.x) + (v * witness.r_i) + (u * x_r_i) + (t * r_p);
    let alpha_g = g * alpha;
    let a = alpha_g + (v * beta) + (u * ((alpha * witness.r_i) + (beta * witness.x))) + (t * delta);
    let b = (u * (alpha * beta)) + (t * mu);
    let r_o = alpha_g + (t * r_y);
    let r_p_point = (u * r_z) + (t * r_r_p);
    let r_l = (i_tilde * alpha) - (u * r_z);
    let proof_points = [p, a, b, r_o, r_p_point, r_l];
    if proof_points.iter().any(EdwardsPoint::is_identity) {
        return Err(FcmpNativeErrorV1::SalProofPointIdentity);
    }
    let e = challenge(context_hash, public, &proof_points)?;
    let s_alpha = alpha + (e * witness.x);
    let s_beta = beta + (e * witness.r_i);
    let s_delta = mu + (e * delta) + (r_p * e * e);
    let s_y = r_y + (e * witness.y);
    let s_z = r_z + (e * x_r_i);
    let r_p_double_quote = r_p - witness.y - witness.r_r_i;
    let s_r_p = r_r_p + (e * r_p_double_quote);
    FcmpSalProofV1::new(
        proof_points.map(|point| point.compress().to_bytes()),
        [s_alpha, s_beta, s_delta, s_y, s_z, s_r_p].map(|scalar| scalar.to_bytes()),
    )
}
/// Verify all four FCMP++ SAL equations directly.
///
/// Success means the proof authorizes the hidden output key and binds the
/// key image to the same secret used in O~, while proving the required
/// bilinear conjunction. Membership and tuple re-randomization remain the
/// responsibility of the FCMP circuit verifier.
pub fn verify_fcmp_sal_v1(
    context_hash: [u8; 32],
    public: &FcmpProofInputPublicV1,
    proof: &FcmpSalProofV1,
) -> Result<(), FcmpNativeErrorV1> {
    let [o_tilde, i_tilde, r, _c_tilde, key_image] = public_points(public)?;
    let points = proof
        .points
        .map(|point| decode_edwards_point(point, false))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let points: [EdwardsPoint; SAL_POINT_COUNT_V1] = points
        .try_into()
        .map_err(|_| FcmpNativeErrorV1::ArithmeticInvariant)?;
    let scalars = proof
        .scalars
        .map(scalar_from_bytes)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let [s_alpha, s_beta, s_delta, s_y, s_z, s_r_p]: [Scalar; SAL_SCALAR_COUNT_V1] = scalars
        .try_into()
        .map_err(|_| FcmpNativeErrorV1::ArithmeticInvariant)?;
    let [p, a, b, r_o, r_p, r_l] = points;
    let g = ED25519_BASEPOINT_POINT;
    let t = generator_t();
    let u = generator_u();
    let v = generator_v();
    let e = challenge(context_hash, public, &[p, a, b, r_o, r_p, r_l])?;
    let equations = [
        sum_terms(&[
            (e * e, p),
            (e, a),
            (Scalar::ONE, b),
            (-(s_alpha * e), g),
            (-(s_beta * e), v),
            (-(s_alpha * s_beta), u),
            (-s_delta, t),
        ]),
        sum_terms(&[(Scalar::ONE, r_o), (e, o_tilde), (-s_alpha, g), (-s_y, t)]),
        sum_terms(&[
            (Scalar::ONE, r_p),
            (e, p - o_tilde - r),
            (-s_z, u),
            (-s_r_p, t),
        ]),
        sum_terms(&[
            (Scalar::ONE, r_l),
            (e, key_image),
            (-s_alpha, i_tilde),
            (s_z, u),
        ]),
    ];
    if equations
        .iter()
        .any(|equation| equation != &EdwardsPoint::identity())
    {
        return Err(FcmpNativeErrorV1::SalEquation);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::fcmp_plus_plus::FailingRngV1;
    use core::cell::Cell;
    use curve25519_dalek::scalar::Scalar;
    use p256::elliptic_curve::bigint::{Encoding as _, U256};
    use rand_08::{SeedableRng as _, rngs::StdRng};
    thread_local! {
        static SAL_SECRET_CLEARS: Cell<usize> = const { Cell::new(0) };
    }
    #[derive(Clone, Copy)]
    struct TrackingCopy(u64);
    impl Zeroize for TrackingCopy {
        fn zeroize(&mut self) {
            self.0 = 0;
            SAL_SECRET_CLEARS.with(|calls| calls.set(calls.get() + 1));
        }
    }
    fn reset_secret_scalar_decoder_owner_drops_v1() {
        SAL_SECRET_CANONICALITY_STATE_OWNER_DROPS_V1.with(|drops| drops.set(0));
        SAL_SECRET_WIDE_INPUT_OWNER_DROPS_V1.with(|drops| drops.set(0));
        reset_sal_secret_copy_owner_drops_v1();
    }
    fn secret_canonicality_state_owner_drops_v1() -> usize {
        SAL_SECRET_CANONICALITY_STATE_OWNER_DROPS_V1.with(Cell::get)
    }
    fn secret_wide_input_owner_drops_v1() -> usize {
        SAL_SECRET_WIDE_INPUT_OWNER_DROPS_V1.with(Cell::get)
    }
    #[test]
    fn sal_secret_copy_owner_clears_transfer_success_and_unwind_slots() {
        SAL_SECRET_CLEARS.with(|calls| calls.set(0));
        let mut source = TrackingCopy(7);
        let owner = SalSecretCopyValueV1::take(&mut source);
        assert_eq!(source.0, 0);
        assert_eq!(owner.expose_ref().0, 7);
        assert_eq!(SAL_SECRET_CLEARS.with(Cell::get), 1);
        drop(owner);
        assert_eq!(SAL_SECRET_CLEARS.with(Cell::get), 2);
        SAL_SECRET_CLEARS.with(|calls| calls.set(0));
        assert!(
            std::panic::catch_unwind(|| {
                let _owner = SalSecretCopyValueV1::new(TrackingCopy(11));
                panic!("tracking unwind");
            })
            .is_err()
        );
        assert_eq!(SAL_SECRET_CLEARS.with(Cell::get), 2);
    }
    #[test]
    fn secret_scalar_decoder_owns_canonicality_and_wide_scratch_on_every_exit() {
        assert_eq!(
            ED25519_SCALAR_MODULUS_LE_V1,
            U256::from_be_hex("1000000000000000000000000000000014def9dea2f79cd65812631a5cf5d3ed")
                .to_le_bytes()
        );
        for (label, integer, expected) in [
            ("zero", U256::ZERO, Scalar::ZERO),
            ("one", U256::ONE, Scalar::ONE),
            (
                "l-1",
                U256::from_le_bytes(ED25519_SCALAR_MODULUS_LE_V1).wrapping_sub(&U256::ONE),
                -Scalar::ONE,
            ),
        ] {
            reset_secret_scalar_decoder_owner_drops_v1();
            let bytes = integer.to_le_bytes();
            let scalar = secret_scalar_from_bytes_v1(&bytes)
                .unwrap_or_else(|error| panic!("{label} rejected: {error:?}"));
            assert_eq!(scalar.expose_ref(), &expected, "{label}");
            assert_eq!(secret_canonicality_state_owner_drops_v1(), 1, "{label}");
            assert_eq!(secret_wide_input_owner_drops_v1(), 1, "{label}");
            assert_eq!(sal_secret_copy_owner_drops_v1(), 0, "{label}");
            drop(scalar);
            assert_eq!(sal_secret_copy_owner_drops_v1(), 1, "{label}");
        }

        let modulus = U256::from_le_bytes(ED25519_SCALAR_MODULUS_LE_V1);
        for (label, integer) in [
            ("l", modulus),
            ("l+1", modulus.wrapping_add(&U256::ONE)),
            ("max", U256::MAX),
        ] {
            reset_secret_scalar_decoder_owner_drops_v1();
            let bytes = integer.to_le_bytes();
            assert!(
                matches!(
                    secret_scalar_from_bytes_v1(&bytes),
                    Err(FcmpNativeErrorV1::ScalarEncoding)
                ),
                "{label} accepted"
            );
            assert_eq!(secret_canonicality_state_owner_drops_v1(), 1, "{label}");
            assert_eq!(secret_wide_input_owner_drops_v1(), 0, "{label}");
            assert_eq!(sal_secret_copy_owner_drops_v1(), 0, "{label}");
        }

        reset_secret_scalar_decoder_owner_drops_v1();
        let unwind = std::panic::catch_unwind(|| {
            let bytes = U256::from(7_u8).to_le_bytes();
            let scalar = secret_scalar_from_bytes_v1(&bytes).expect("canonical scalar");
            assert_eq!(secret_canonicality_state_owner_drops_v1(), 1);
            assert_eq!(secret_wide_input_owner_drops_v1(), 1);
            assert_eq!(sal_secret_copy_owner_drops_v1(), 0);
            let _ = core::hint::black_box(scalar.expose_ref());
            panic!("exercise decoded scalar owner unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(secret_canonicality_state_owner_drops_v1(), 1);
        assert_eq!(secret_wide_input_owner_drops_v1(), 1);
        assert_eq!(sal_secret_copy_owner_drops_v1(), 1);

        reset_secret_scalar_decoder_owner_drops_v1();
        let comparison_unwind = std::panic::catch_unwind(|| {
            let mut canonicality = SalSecretScalarCanonicalityStateV1::new_v1();
            canonicality.observe_byte_v1(&7_u8, &11_u8);
            assert_eq!(secret_canonicality_state_owner_drops_v1(), 0);
            assert_eq!(secret_wide_input_owner_drops_v1(), 0);
            let _ = core::hint::black_box(&canonicality.less);
            panic!("exercise active scalar canonicality state unwind");
        });
        assert!(comparison_unwind.is_err());
        assert_eq!(secret_canonicality_state_owner_drops_v1(), 1);
        assert_eq!(secret_wide_input_owner_drops_v1(), 0);
        assert_eq!(sal_secret_copy_owner_drops_v1(), 0);

        reset_secret_scalar_decoder_owner_drops_v1();
        let wide_unwind = std::panic::catch_unwind(|| {
            let bytes = U256::from(11_u8).to_le_bytes();
            let wide = SalSecretScalarWideInputV1::from_borrowed_v1(&bytes);
            assert_eq!(secret_canonicality_state_owner_drops_v1(), 0);
            assert_eq!(secret_wide_input_owner_drops_v1(), 0);
            let _ = core::hint::black_box(&wide.0);
            panic!("exercise active scalar wide-input owner unwind");
        });
        assert!(wide_unwind.is_err());
        assert_eq!(secret_canonicality_state_owner_drops_v1(), 0);
        assert_eq!(secret_wide_input_owner_drops_v1(), 1);
        assert_eq!(sal_secret_copy_owner_drops_v1(), 0);
    }
    #[test]
    fn sal_witness_takes_all_bytes_before_borrowed_decoding() {
        let source = include_str!("sal.rs");
        let production = source
            .split_once("#[cfg(test)]\nmod tests {")
            .expect("test module boundary")
            .0;
        for forbidden in ["U256", "from_le_slice", "Encoding as _"] {
            assert!(
                !production.contains(forbidden),
                "retained production {forbidden}"
            );
        }
        let modulus = source
            .split_once("static ED25519_SCALAR_MODULUS_LE_V1: [u8; 32] = [")
            .expect("little-endian scalar modulus")
            .1
            .split_once("];")
            .expect("scalar modulus boundary")
            .0;
        assert_eq!(
            modulus.split_whitespace().collect::<String>(),
            "0xed,0xd3,0xf5,0x5c,0x1a,0x63,0x12,0x58,0xd6,0x9c,0xf7,0xa2,0xde,0xf9,0xde,0x14,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x10,"
        );
        let public_decoder = source
            .split_once("fn scalar_from_bytes(bytes: [u8; 32])")
            .expect("public scalar decoder")
            .1
            .split_once("fn secret_scalar_from_bytes_v1(")
            .expect("public scalar decoder boundary")
            .0;
        assert_eq!(
            public_decoder
                .matches("validate_edwards_scalar(bytes)?")
                .count(),
            1
        );
        assert_eq!(
            public_decoder
                .matches("Scalar::from_canonical_bytes(bytes)")
                .count(),
            1
        );
        let decoder = source
            .split_once("fn secret_scalar_from_bytes_v1(")
            .expect("secret scalar decoder")
            .1
            .split_once("fn random_scalar")
            .expect("decoder boundary")
            .0;
        assert!(decoder.contains("bytes: &[u8; 32]"));
        let decoder_steps = [
            "let mut canonicality = SalSecretScalarCanonicalityStateV1::new_v1()",
            "let mut index = ED25519_SCALAR_MODULUS_LE_V1.len()",
            "while index != 0",
            "index -= 1",
            "let byte = &bytes[index]",
            "let modulus_byte = &ED25519_SCALAR_MODULUS_LE_V1[index]",
            "canonicality.observe_byte_v1(byte, modulus_byte)",
            "let is_canonical = bool::from(canonicality.less)",
            "drop(canonicality)",
            "if !is_canonical",
            "let wide = SalSecretScalarWideInputV1::from_borrowed_v1(bytes)",
            "let mut scalar = Scalar::from_bytes_mod_order_wide(&wide.0)",
            "drop(wide)",
            "let scalar = SalSecretCopyValueV1::take(&mut scalar)",
            "Ok(scalar)",
        ];
        let decoder_positions = decoder_steps
            .iter()
            .map(|needle| {
                decoder
                    .find(needle)
                    .unwrap_or_else(|| panic!("missing decoder step {needle}"))
            })
            .collect::<Vec<_>>();
        assert!(decoder_positions.windows(2).all(|pair| pair[0] < pair[1]));
        for (needle, expected) in [
            ("SalSecretScalarCanonicalityStateV1::new_v1()", 1),
            ("ED25519_SCALAR_MODULUS_LE_V1.len()", 1),
            ("canonicality.observe_byte_v1(byte, modulus_byte)", 1),
            ("bool::from(canonicality.less)", 1),
            ("SalSecretScalarWideInputV1::from_borrowed_v1(bytes)", 1),
            ("Scalar::from_bytes_mod_order_wide(&wide.0)", 1),
            ("SalSecretCopyValueV1::take(&mut scalar)", 1),
            ("drop(canonicality)", 1),
            ("drop(wide)", 1),
        ] {
            assert_eq!(decoder.matches(needle).count(), expected, "{needle}");
        }
        for forbidden in [
            "bytes: [u8; 32]",
            "Scalar::from_canonical_bytes",
            "validate_edwards_scalar",
            "U256",
            "from_le_slice",
            "U256::from_le_bytes",
            "let integer",
            "let mut res",
            "let mut buf",
            "Choice::",
            ".ct_lt(",
            "*bytes",
            ".expose_copy()",
            ".clone()",
            ".to_owned()",
            "Zeroizing::new(",
            "callback",
            "FnOnce",
            "Deref",
        ] {
            assert!(!decoder.contains(forbidden), "retained {forbidden}");
        }
        assert_eq!(decoder.matches("bool::from(").count(), 1);
        let comparison_owner = source
            .split_once("struct SalSecretScalarCanonicalityStateV1 {")
            .expect("secret scalar canonicality owner")
            .1
            .split_once("struct SalSecretScalarWideInputV1([u8; 64]);")
            .expect("secret scalar canonicality owner boundary")
            .0;
        assert!(
            comparison_owner
                .contains("fn observe_byte_v1(&mut self, byte: &u8, modulus_byte: &u8)")
        );
        let comparison_steps = [
            "self.prefix_decided = self.less | self.greater",
            "self.prefix_equal = !self.prefix_decided",
            "self.byte_less = byte.ct_lt(modulus_byte)",
            "self.byte_greater = modulus_byte.ct_lt(byte)",
            "self.less_update = self.prefix_equal & self.byte_less",
            "self.greater_update = self.prefix_equal & self.byte_greater",
            "self.less |= self.less_update",
            "self.greater |= self.greater_update",
        ];
        let comparison_positions = comparison_steps
            .iter()
            .map(|needle| {
                comparison_owner
                    .find(needle)
                    .unwrap_or_else(|| panic!("missing comparison step {needle}"))
            })
            .collect::<Vec<_>>();
        assert!(
            comparison_positions
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
        assert_eq!(comparison_owner.matches(".ct_lt(").count(), 2);
        assert!(!comparison_owner.contains("bool::from("));
        for field in [
            "less",
            "greater",
            "byte_less",
            "byte_greater",
            "prefix_decided",
            "prefix_equal",
            "less_update",
            "greater_update",
        ] {
            assert_eq!(
                comparison_owner
                    .matches(&format!("\n    {field}: Choice,"))
                    .count(),
                1,
                "missing comparison state {field}"
            );
            assert_eq!(
                comparison_owner
                    .matches(&format!("self.{field} = Choice::from(0)"))
                    .count(),
                1,
                "comparison state {field} is not cleared"
            );
            assert_eq!(
                comparison_owner
                    .matches(&format!("black_box(&mut self.{field})"))
                    .count(),
                1,
                "comparison state {field} is not pinned after clear"
            );
        }
        assert!(comparison_owner.contains("compiler_fence"));
        let wide_owner = source
            .split_once("struct SalSecretScalarWideInputV1([u8; 64]);")
            .expect("secret wide-input owner")
            .1
            .split_once("pub(super) struct FcmpSalSecretScalarEncodingV1(")
            .expect("secret wide-input owner boundary")
            .0;
        assert!(wide_owner.contains("fn from_borrowed_v1(bytes: &[u8; 32]) -> Self"));
        assert!(wide_owner.contains("let mut wide = Self([0_u8; 64])"));
        assert!(wide_owner.contains("wide.0[..32].copy_from_slice(bytes)"));
        assert!(wide_owner.contains("self.0.zeroize()"));
        assert!(wide_owner.contains("compiler_fence"));
        assert!(wide_owner.contains("black_box"));
        for owner in [comparison_owner, wide_owner] {
            for forbidden in [
                "#[derive(",
                "impl Clone",
                "impl Copy",
                "Deref",
                "fn expose_",
                "fn get",
                "fn as_",
                "fn with_",
                "callback",
                "FnOnce",
                "FnMut",
            ] {
                assert!(!owner.contains(forbidden), "retained owner {forbidden}");
            }
        }
        let constructor = source
            .split_once("impl FcmpSalWitnessV1 {")
            .expect("SAL witness impl")
            .1
            .split_once("#[allow(clippy::too_many_arguments)]")
            .expect("constructor boundary")
            .0;
        assert_eq!(
            constructor
                .matches("FcmpSalSecretScalarEncodingV1::take(&mut")
                .count(),
            4
        );
        let last_take = constructor
            .rfind("FcmpSalSecretScalarEncodingV1::take(&mut")
            .expect("last input take");
        let consume = constructor
            .find("Self::from_secret_scalar_encoding_owners_v1(")
            .expect("owner-consuming constructor");
        assert!(last_take < consume);
        let first_decode = constructor
            .find("secret_scalar_from_bytes_v1(")
            .expect("first borrowed decode");
        assert!(consume < first_decode);
        assert_eq!(
            constructor.matches("secret_scalar_from_bytes_v1(").count(),
            4
        );
        let last_decode = constructor
            .rfind("secret_scalar_from_bytes_v1(")
            .expect("last scalar owner");
        let destination = constructor
            .find("let mut witness = Self {")
            .expect("zero destination");
        assert!(first_decode < last_decode && last_decode < destination);
        let swaps = [
            "core::mem::swap(&mut witness.x, &mut x.0)",
            "drop(x)",
            "core::mem::swap(&mut witness.y, &mut y.0)",
            "drop(y)",
            "core::mem::swap(&mut witness.r_i, &mut r_i.0)",
            "drop(r_i)",
            "core::mem::swap(&mut witness.r_r_i, &mut r_r_i.0)",
            "drop(r_r_i)",
            "Ok(witness)",
        ];
        let positions = swaps
            .iter()
            .map(|needle| {
                constructor
                    .find(needle)
                    .unwrap_or_else(|| panic!("missing {needle}"))
            })
            .collect::<Vec<_>>();
        assert!(destination < positions[0]);
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(constructor.matches("Scalar::ZERO").count(), 4);
        assert!(!constructor.contains(".expose_copy()"));
        assert!(!constructor.contains("Zeroizing::new(x)"));
        assert!(!constructor.contains("scalar_from_bytes(*"));
        let encoding_owner = source
            .split_once("pub(super) struct FcmpSalSecretScalarEncodingV1(")
            .expect("opaque scalar-encoding owner")
            .1
            .split_once("// Generated by the pinned Monero")
            .expect("encoding owner boundary")
            .0;
        for forbidden in [
            "#[derive(",
            "impl Clone",
            "impl Copy",
            "Deref",
            "AsRef",
            "Borrow",
            "fn expose_",
            "fn get",
            "fn as_",
            "fn with_",
            "FnOnce",
            "FnMut",
            ") -> [u8; 32]",
            ") -> Scalar",
            "callback",
        ] {
            assert!(!encoding_owner.contains(forbidden), "retained {forbidden}");
        }
        assert_eq!(encoding_owner.matches("impl ").count(), 1);
        let owned_copy_impl = source
            .split_once("impl<T: Copy + Zeroize> SalSecretCopyValueV1<T> {")
            .expect("SAL copy owner impl")
            .1
            .split_once("impl<T: Copy + Zeroize> Drop for SalSecretCopyValueV1<T>")
            .expect("SAL copy owner boundary")
            .0;
        assert!(!owned_copy_impl.contains("fn expose_copy(&self)"));
    }
    #[test]
    fn sal_scalar_encoding_owners_cover_each_decode_position_zeroize_and_unwind() {
        let values = [
            Scalar::from(17_u64),
            Scalar::from(23_u64),
            Scalar::from(31_u64),
            Scalar::from(43_u64),
        ];
        reset_sal_secret_copy_owner_drops_v1();
        reset_fcmp_sal_witness_owner_drops_v1();
        {
            let witness = FcmpSalWitnessV1::from_secret_scalar_encoding_owners_v1(
                FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&values[0]),
                FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&values[1]),
                FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&values[2]),
                FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&values[3]),
            )
            .expect("owned SAL witness");
            assert_eq!(witness.x, values[0]);
            assert_eq!(witness.y, values[1]);
            assert_eq!(witness.r_i, values[2]);
            assert_eq!(witness.r_r_i, values[3]);
            assert_eq!(sal_secret_copy_owner_drops_v1(), 8);
            assert_eq!(fcmp_sal_witness_owner_drops_v1(), 0);
        }
        assert_eq!(sal_secret_copy_owner_drops_v1(), 8);
        assert_eq!(fcmp_sal_witness_owner_drops_v1(), 1);

        for invalid_position in 0..4 {
            reset_sal_secret_copy_owner_drops_v1();
            reset_fcmp_sal_witness_owner_drops_v1();
            let mut encodings = values.map(|value| value.to_bytes());
            encodings[invalid_position] = [u8::MAX; 32];
            let result = FcmpSalWitnessV1::from_secret_scalar_encoding_owners_v1(
                FcmpSalSecretScalarEncodingV1::from_test_bytes_v1(encodings[0]),
                FcmpSalSecretScalarEncodingV1::from_test_bytes_v1(encodings[1]),
                FcmpSalSecretScalarEncodingV1::from_test_bytes_v1(encodings[2]),
                FcmpSalSecretScalarEncodingV1::from_test_bytes_v1(encodings[3]),
            );
            assert!(matches!(result, Err(FcmpNativeErrorV1::ScalarEncoding)));
            assert_eq!(
                sal_secret_copy_owner_drops_v1(),
                4 + invalid_position,
                "decode position {invalid_position}"
            );
            assert_eq!(fcmp_sal_witness_owner_drops_v1(), 0);
        }

        reset_sal_secret_copy_owner_drops_v1();
        reset_fcmp_sal_witness_owner_drops_v1();
        let mut witness = FcmpSalWitnessV1::from_secret_scalar_encoding_owners_v1(
            FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&values[0]),
            FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&values[1]),
            FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&values[2]),
            FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&values[3]),
        )
        .expect("witness for explicit zeroize");
        assert_eq!(sal_secret_copy_owner_drops_v1(), 8);
        witness.zeroize();
        assert_eq!(witness.x, Scalar::ZERO);
        assert_eq!(witness.y, Scalar::ZERO);
        assert_eq!(witness.r_i, Scalar::ZERO);
        assert_eq!(witness.r_r_i, Scalar::ZERO);
        drop(witness);
        assert_eq!(fcmp_sal_witness_owner_drops_v1(), 1);

        reset_sal_secret_copy_owner_drops_v1();
        reset_fcmp_sal_witness_owner_drops_v1();
        let unwind = std::panic::catch_unwind(|| {
            let witness = FcmpSalWitnessV1::from_secret_scalar_encoding_owners_v1(
                FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&values[0]),
                FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&values[1]),
                FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&values[2]),
                FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&values[3]),
            )
            .expect("witness before unwind");
            assert_eq!(sal_secret_copy_owner_drops_v1(), 8);
            assert_eq!(fcmp_sal_witness_owner_drops_v1(), 0);
            let _ = core::hint::black_box(&witness);
            panic!("exercise SAL witness downstream unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(sal_secret_copy_owner_drops_v1(), 8);
        assert_eq!(fcmp_sal_witness_owner_drops_v1(), 1);
    }
    #[test]
    fn sal_witness_owner_drops_after_downstream_relation_error() {
        let (public, correct_witness) = public_and_witness();
        drop(correct_witness);
        reset_sal_secret_copy_owner_drops_v1();
        reset_fcmp_sal_witness_owner_drops_v1();
        let wrong = FcmpSalWitnessV1::from_secret_scalar_encoding_owners_v1(
            FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&Scalar::from(18_u64)),
            FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&Scalar::from(23_u64)),
            FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&Scalar::from(31_u64)),
            FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(&Scalar::from(43_u64)),
        )
        .expect("canonical mismatched witness");
        assert_eq!(sal_secret_copy_owner_drops_v1(), 8);
        assert_eq!(fcmp_sal_witness_owner_drops_v1(), 0);
        assert_eq!(
            validate_sal_witness_relation_v1(&public, &wrong),
            Err(FcmpNativeErrorV1::SalWitnessMismatch)
        );
        assert_eq!(fcmp_sal_witness_owner_drops_v1(), 0);
        drop(wrong);
        assert_eq!(fcmp_sal_witness_owner_drops_v1(), 1);
    }
    struct ZeroRng;
    impl RngCore for ZeroRng {
        fn next_u32(&mut self) -> u32 {
            0
        }
        fn next_u64(&mut self) -> u64 {
            0
        }
        fn fill_bytes(&mut self, destination: &mut [u8]) {
            destination.fill(0);
        }
        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
            self.fill_bytes(destination);
            Ok(())
        }
    }
    impl CryptoRng for ZeroRng {}
    struct PeriodicRng {
        period: usize,
        cursor: usize,
    }
    impl RngCore for PeriodicRng {
        fn next_u32(&mut self) -> u32 {
            panic!("FCMP++ SAL must reject the periodic prefix")
        }
        fn next_u64(&mut self) -> u64 {
            panic!("FCMP++ SAL must reject the periodic prefix")
        }
        fn fill_bytes(&mut self, _destination: &mut [u8]) {
            panic!("FCMP++ SAL must use fallible entropy")
        }
        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
            for byte in destination {
                *byte = ((self.cursor % self.period) as u8)
                    .wrapping_mul(43)
                    .wrapping_add(11);
                self.cursor += 1;
            }
            Ok(())
        }
    }
    impl CryptoRng for PeriodicRng {}
    fn public_and_witness() -> (FcmpProofInputPublicV1, FcmpSalWitnessV1) {
        let x = Scalar::from(17_u64);
        let y = Scalar::from(23_u64);
        let r_i = Scalar::from(31_u64);
        let r_r_i = Scalar::from(43_u64);
        let o_tilde = (ED25519_BASEPOINT_POINT * x) + (generator_t() * y);
        let i_tilde = ED25519_BASEPOINT_POINT * Scalar::from(59_u64);
        let r = (generator_v() * r_i) + (generator_t() * r_r_i);
        let c_tilde = ED25519_BASEPOINT_POINT * Scalar::from(61_u64);
        let key_image = (i_tilde * x) - (generator_u() * (r_i * x));
        (
            FcmpProofInputPublicV1::new(
                o_tilde.compress().to_bytes(),
                i_tilde.compress().to_bytes(),
                r.compress().to_bytes(),
                c_tilde.compress().to_bytes(),
                key_image.compress().to_bytes(),
            )
            .expect("valid public relation"),
            FcmpSalWitnessV1::new(x.to_bytes(), y.to_bytes(), r_i.to_bytes(), r_r_i.to_bytes())
                .expect("canonical witness"),
        )
    }
    #[test]
    fn pinned_monero_generators_are_strict_and_distinct() {
        assert_eq!(generator_t().compress().to_bytes(), MONERO_T_BYTES);
        assert_eq!(generator_u().compress().to_bytes(), MONERO_FCMP_U_BYTES);
        assert_eq!(generator_v().compress().to_bytes(), MONERO_FCMP_V_BYTES);
        assert_ne!(generator_t(), generator_u());
        assert_ne!(generator_t(), generator_v());
        assert_ne!(generator_u(), generator_v());
    }
    #[test]
    fn sal_prove_verify_and_codec_roundtrip() {
        let (public, witness) = public_and_witness();
        let context = [0x42; 32];
        let mut rng = StdRng::seed_from_u64(0x5a17);
        let proof =
            prove_fcmp_sal_v1(&mut rng, context, &public, &witness).expect("honest SAL proof");
        verify_fcmp_sal_v1(context, &public, &proof).expect("honest SAL verification");
        assert_eq!(
            FcmpSalProofV1::decode(&proof.encode()).expect("strict roundtrip"),
            proof
        );
    }
    #[test]
    fn sal_zero_rng_is_bounded_internally_and_rejected_at_the_public_boundary() {
        let mut scalar_rng = ZeroRng;
        assert_eq!(
            random_scalar(&mut scalar_rng),
            Err(FcmpNativeErrorV1::ProverRandomnessExhausted)
        );
        let (public, witness) = public_and_witness();
        let mut proof_rng = ZeroRng;
        assert_eq!(
            prove_fcmp_sal_v1(&mut proof_rng, [0x7a; 32], &public, &witness),
            Err(FcmpNativeErrorV1::RandomnessHealthCheckFailed)
        );
    }
    #[test]
    fn sal_rng_unavailability_fails_without_calling_infallible_rng_methods() {
        assert_eq!(
            random_scalar(&mut FailingRngV1),
            Err(FcmpNativeErrorV1::RandomnessUnavailable)
        );
        let (public, witness) = public_and_witness();
        assert_eq!(
            prove_fcmp_sal_v1(&mut FailingRngV1, [0x7b; 32], &public, &witness),
            Err(FcmpNativeErrorV1::RandomnessUnavailable)
        );
    }
    #[test]
    fn sal_public_prover_rejects_every_prohibited_short_period_prefix() {
        let (public, witness) = public_and_witness();
        for period in [1, 2, 4, 8, 16, 32] {
            assert_eq!(
                prove_fcmp_sal_v1(
                    &mut PeriodicRng { period, cursor: 0 },
                    [0x7c; 32],
                    &public,
                    &witness,
                ),
                Err(FcmpNativeErrorV1::RandomnessHealthCheckFailed),
                "period-{period} SAL entropy was not rejected"
            );
        }
    }
    #[test]
    fn sal_restarts_only_point_and_challenge_honest_aborts_at_a_fixed_bound() {
        let mut attempts = 0;
        let recovered = retry_sal_prover_v1(|| {
            attempts += 1;
            match attempts {
                1 => Err(FcmpNativeErrorV1::SalProofPointIdentity),
                2 => Err(FcmpNativeErrorV1::SalChallengeZero),
                _ => Ok(29_u8),
            }
        })
        .expect("third attempt succeeds");
        assert_eq!(recovered, 29);
        assert_eq!(attempts, 3);
        for retryable in [
            FcmpNativeErrorV1::SalProofPointIdentity,
            FcmpNativeErrorV1::SalChallengeZero,
        ] {
            attempts = 0;
            assert_eq!(
                retry_sal_prover_v1::<()>(|| {
                    attempts += 1;
                    Err(retryable)
                }),
                Err(FcmpNativeErrorV1::SalProverRestartExhausted)
            );
            assert_eq!(attempts, MAX_SAL_PROVER_RESTARTS_V1);
        }
        assert_eq!(MAX_SAL_PROVER_RESTARTS_V1, 128);
        attempts = 0;
        assert_eq!(
            retry_sal_prover_v1::<()>(|| {
                attempts += 1;
                Err(FcmpNativeErrorV1::SalWitnessMismatch)
            }),
            Err(FcmpNativeErrorV1::SalWitnessMismatch)
        );
        assert_eq!(attempts, 1);
    }
    #[test]
    fn sal_every_component_context_and_statement_are_bound() {
        let (public, witness) = public_and_witness();
        let context = [0x24; 32];
        let mut rng = StdRng::seed_from_u64(0x5a18);
        let proof =
            prove_fcmp_sal_v1(&mut rng, context, &public, &witness).expect("honest SAL proof");
        let encoded = proof.encode();
        for component in 0..(SAL_POINT_COUNT_V1 + SAL_SCALAR_COUNT_V1) {
            let mut mutation = encoded;
            mutation[component * 32] ^= 1;
            let result = FcmpSalProofV1::decode(&mutation)
                .and_then(|proof| verify_fcmp_sal_v1(context, &public, &proof));
            assert!(
                result.is_err(),
                "component {component} mutation was accepted"
            );
        }
        let mut replay_context = context;
        replay_context[0] ^= 1;
        assert_eq!(
            verify_fcmp_sal_v1(replay_context, &public, &proof),
            Err(FcmpNativeErrorV1::SalEquation)
        );
        let mut altered_public = public;
        altered_public.pseudo_out = (ED25519_BASEPOINT_POINT * Scalar::from(67_u64))
            .compress()
            .to_bytes();
        assert_eq!(
            verify_fcmp_sal_v1(context, &altered_public, &proof),
            Err(FcmpNativeErrorV1::SalEquation)
        );
    }
    #[test]
    fn sal_rejects_inconsistent_witness_and_noncanonical_encoding() {
        let (public, _witness) = public_and_witness();
        let wrong = FcmpSalWitnessV1::new(
            Scalar::from(18_u64).to_bytes(),
            Scalar::from(23_u64).to_bytes(),
            Scalar::from(31_u64).to_bytes(),
            Scalar::from(43_u64).to_bytes(),
        )
        .expect("canonical wrong witness");
        assert_eq!(
            prove_fcmp_sal_v1(&mut FailingRngV1, [0; 32], &public, &wrong),
            Err(FcmpNativeErrorV1::SalWitnessMismatch)
        );
        assert!(matches!(
            FcmpSalProofV1::decode(&[0; FCMP_SAL_PROOF_BYTES_V1 - 1]),
            Err(FcmpNativeErrorV1::ProofLength { .. })
        ));
        let mut invalid = [0_u8; FCMP_SAL_PROOF_BYTES_V1];
        invalid[..32].fill(u8::MAX);
        assert_eq!(
            FcmpSalProofV1::decode(&invalid),
            Err(FcmpNativeErrorV1::EdwardsPointEncoding)
        );
    }
}
