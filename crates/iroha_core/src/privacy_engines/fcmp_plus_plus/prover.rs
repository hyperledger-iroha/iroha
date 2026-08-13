//! Canonical native FCMP++ prover.
//!
//! The public API accepts an output-opening and a complete alternating tree
//! path for each input.  Re-randomization, branch blinding, divisor
//! construction, both arithmetic-circuit proofs, SAL, root-blind proof, and
//! IFC1 framing are all produced here; callers cannot inject opaque
//! precomputed circuit witnesses.

use curve25519_dalek::{constants::ED25519_BASEPOINT_POINT, edwards::EdwardsPoint, scalar::Scalar};
use p256::elliptic_curve::subtle::{Choice, ConstantTimeEq as _};
use rand_core_06::{CryptoRng, RngCore};
use zeroize::{Zeroize, Zeroizing};

use super::{
    FCMP_LAYER_ONE_LEN_V1, FCMP_LAYER_TWO_LEN_V1, FCMP_MAX_TREE_LAYERS_V1,
    FCMP_PROOF_WIRE_HEADER_BYTES_V1, FcmpNativeErrorV1, FcmpOutputCommitmentOpeningV1,
    FcmpOutputTupleV1, FcmpProofInputPublicV1, FcmpSalWitnessV1, FcmpTreeCurveV1, FcmpTreeRootV1,
    bulletproof::Variable,
    circuit::{
        CYCLE_DLOG_PARAMETERS, Circuit, ED25519_DLOG_PARAMETERS, PointWithDlog,
        ProverVectorCommitmentTape,
    },
    divisor::{
        NormalizedDivisor, ed25519_scalar_decomposition, scalar_decomposition, scalar_mul_divisor,
    },
    field::{
        Field25519, HeliosPoint, HelioseleneField, SelenePoint, decode_edwards_point,
        decode_secret_field25519_scalar_v1, decode_secret_helioselene_scalar_v1,
        edwards_to_wei25519, hash_helios, hash_selene, secret_edwards_to_wei25519_v1,
        validate_edwards_scalar,
    },
    membership::{
        TranscriptedInput, constrain_input, ed25519_curve, helios_curve, membership_context,
        native_parameters, selene_curve,
    },
    proof_math::{
        FcmpProofRandomSource, HeliosSuite, ProofPoint, ProofScalar, ProofSuite, ProverTranscript,
        SecretMultiexpBuilder, SeleneSuite, helios_bp_generators, random_scalar_from_fcmp_rng,
        selene_bp_generators,
    },
    range::prove_fcmp_range_with_checked_rng_v1,
    sal::{generator_t, generator_u, generator_v, prove_fcmp_sal_with_checked_rng_v1},
    wire::{FCMP_PROOF_WIRE_MAGIC_V1, fcmp_plus_plus_wire_size_v1, ipa_rows},
};

const MAX_PROVER_SCALAR_ATTEMPTS_V1: usize = 128;
const MAX_MEMBERSHIP_PROVER_RESTARTS_V1: usize = 128;

struct ProverSecretCopyValueV1<T: Copy + Zeroize>(T);

struct BorrowedProverCopySlotV1<'a, T: Copy + Zeroize>(&'a mut T);

impl<T: Copy + Zeroize> BorrowedProverCopySlotV1<'_, T> {
    fn expose_copy(&self) -> T {
        *self.0
    }
}

impl<T: Copy + Zeroize> Drop for BorrowedProverCopySlotV1<'_, T> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl<T: Copy + Zeroize> ProverSecretCopyValueV1<T> {
    fn new(mut value: T) -> Self {
        Self::take(&mut value)
    }

    fn take(value: &mut T) -> Self {
        let incoming = BorrowedProverCopySlotV1(value);
        let owned = Self(incoming.expose_copy());
        drop(incoming);
        owned
    }

    fn expose_copy(&self) -> T {
        self.0
    }

    fn expose_ref(&self) -> &T {
        &self.0
    }
}

impl<T: Copy + Zeroize> Drop for ProverSecretCopyValueV1<T> {
    fn drop(&mut self) {
        self.0.zeroize();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.0);
    }
}

/// Clears one prover-owned scalar on success, error, and unwind.
struct ProverSecretScalarV1<F: ProofScalar>(F);

struct BorrowedProverScalarSlotV1<'a, F: ProofScalar>(&'a mut F);

impl<F: ProofScalar> BorrowedProverScalarSlotV1<'_, F> {
    fn expose_copy(&self) -> F {
        *self.0
    }
}

impl<F: ProofScalar> Drop for BorrowedProverScalarSlotV1<'_, F> {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}

impl<F: ProofScalar> ProverSecretScalarV1<F> {
    fn take(value: &mut F) -> Self {
        let incoming = BorrowedProverScalarSlotV1(value);
        let owned = Self(incoming.expose_copy());
        drop(incoming);
        owned
    }

    fn expose_copy(&self) -> F {
        self.0
    }

    fn expose_ref(&self) -> &F {
        &self.0
    }

    fn add_product_assign(&mut self, left: &F, right: &F) {
        self.0 += *left * *right;
    }
}

impl<F: ProofScalar> Drop for ProverSecretScalarV1<F> {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}

struct ProverSecretPointV1<P: ProofPoint>(P);

struct BorrowedProverPointSlotV1<'a, P: ProofPoint>(&'a mut P);

impl<P: ProofPoint> BorrowedProverPointSlotV1<'_, P> {
    fn expose_copy(&self) -> P {
        *self.0
    }
}

impl<P: ProofPoint> Drop for BorrowedProverPointSlotV1<'_, P> {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}

impl<P: ProofPoint> ProverSecretPointV1<P> {
    fn take(value: &mut P) -> Self {
        let incoming = BorrowedProverPointSlotV1(value);
        let owned = Self(incoming.expose_copy());
        drop(incoming);
        owned
    }

    fn expose_copy(&self) -> P {
        self.0
    }

    fn expose_ref(&self) -> &P {
        &self.0
    }
}

impl<P: ProofPoint> Drop for ProverSecretPointV1<P> {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}

fn zeroizing_digest_buffer(
    exact_capacity: usize,
) -> Result<Zeroizing<Vec<[u8; 32]>>, FcmpNativeErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(exact_capacity)
        .map_err(|_| FcmpNativeErrorV1::ArithmeticInvariant)?;
    if values.capacity() < exact_capacity {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    Ok(Zeroizing::new(values))
}

fn push_secret_scalar_v1<F: ProofScalar>(
    values: &mut Zeroizing<Vec<F>>,
    mut value: F,
) -> Result<(), FcmpNativeErrorV1> {
    let value = ProverSecretScalarV1::take(&mut value);
    require_preallocated_push(values.len(), values.capacity())?;
    values.push(value.expose_copy());
    drop(value);
    Ok(())
}

fn ct_slice_contains_by<T, U>(
    values: &[T],
    target: &U,
    mut equal: impl FnMut(&T, &U) -> Choice,
) -> Choice {
    let mut found = Choice::from(0);
    for value in values {
        found |= equal(value, target);
    }
    found
}

fn ct_has_duplicate_by<T>(values: &[T], mut equal: impl FnMut(&T, &T) -> Choice) -> Choice {
    let mut duplicate = Choice::from(0);
    for (index, left) in values.iter().enumerate() {
        for right in &values[index + 1..] {
            duplicate |= equal(left, right);
        }
    }
    duplicate
}

/// Compare every element when the public slice lengths agree.
fn ct_equal_slices_by<T>(
    left: &[T],
    right: &[T],
    mut equal: impl FnMut(&T, &T) -> Choice,
) -> Choice {
    if left.len() != right.len() {
        return Choice::from(0);
    }
    let mut all_equal = Choice::from(1);
    for (left, right) in left.iter().zip(right) {
        all_equal &= equal(left, right);
    }
    all_equal
}

fn ct_all_match_by<T, U>(
    values: &[T],
    expected: &U,
    mut matches: impl FnMut(&T, &U) -> Choice,
) -> Choice {
    let mut all_match = Choice::from(1);
    for value in values {
        all_match &= matches(value, expected);
    }
    all_match
}

/// Require an already allocated slot before a private value is constructed.
///
/// Calling this immediately before sampling/preparing and pushing a secret
/// prevents `Vec::push` from copying previously inserted secrets during a
/// growth operation. The caller's count is public, so capacity exhaustion is
/// an arithmetic invariant rather than a secret-dependent branch.
fn require_preallocated_push(
    current_len: usize,
    allocation_capacity: usize,
) -> Result<(), FcmpNativeErrorV1> {
    if current_len >= allocation_capacity {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    Ok(())
}

/// Compare every fixed-size digest pair without early exit.
///
/// The slice length is public transaction shape. Digest contents can identify
/// hidden spent leaves before proof publication, so neither equality nor the
/// duplicate position may influence the prover's control flow.
fn ct_has_duplicate_digests(values: &[[u8; 32]]) -> bool {
    bool::from(ct_has_duplicate_by(values, |left, right| {
        left.as_slice().ct_eq(right.as_slice())
    }))
}

fn ct_digest_slice_contains(values: &[[u8; 32]], target: &[u8; 32]) -> bool {
    bool::from(ct_slice_contains_by(values, target, |value, target| {
        value.as_slice().ct_eq(target.as_slice())
    }))
}

// `hash_{selene,helios}` and `decode(..., false)` both guarantee nonidentity,
// so the identity arm inside point encoding has a fixed outcome here. The
// encoded-byte comparison itself examines all 32 bytes.
fn ct_selene_point_eq(left: &SelenePoint, right: &SelenePoint) -> bool {
    let left = Zeroizing::new(left.encode());
    let right = Zeroizing::new(right.encode());
    bool::from(left.as_slice().ct_eq(right.as_slice()))
}

fn ct_helios_point_eq(left: &HeliosPoint, right: &HeliosPoint) -> bool {
    let left = Zeroizing::new(left.encode());
    let right = Zeroizing::new(right.encode());
    bool::from(left.as_slice().ct_eq(right.as_slice()))
}

fn ct_field25519_slice_contains(values: &[Field25519], target: Field25519) -> bool {
    bool::from(ct_slice_contains_by(values, &target, |value, target| {
        (*value - *target).ct_is_zero()
    }))
}

fn ct_helioselene_slice_contains(values: &[HelioseleneField], target: HelioseleneField) -> bool {
    bool::from(ct_slice_contains_by(values, &target, |value, target| {
        (*value - *target).ct_is_zero()
    }))
}

enum AdditionalBranch {
    /// A branch of Selene x-coordinates which hashes to Helios.
    ToHelios(Vec<HelioseleneField>),
    /// A branch of Helios x-coordinates which hashes to Selene.
    ToSelene(Vec<Field25519>),
}

impl Zeroize for AdditionalBranch {
    fn zeroize(&mut self) {
        match self {
            Self::ToHelios(branch) => branch.zeroize(),
            Self::ToSelene(branch) => branch.zeroize(),
        }
    }
}

impl Drop for AdditionalBranch {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
fn duplicate_zeroizing_slice<T>(values: &[T]) -> Zeroizing<Vec<T>>
where
    T: Copy + Zeroize,
{
    let mut duplicate = Zeroizing::new(Vec::with_capacity(values.len()));
    duplicate.extend_from_slice(values);
    duplicate
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
fn duplicate_zeroizing_nested_slices<T>(values: &[Vec<T>]) -> Zeroizing<Vec<Vec<T>>>
where
    T: Copy + Zeroize,
{
    let mut duplicate = Zeroizing::new(Vec::with_capacity(values.len()));
    for values in values {
        let mut inner = duplicate_zeroizing_slice(values);
        duplicate.push(core::mem::take(&mut *inner));
    }
    duplicate
}

#[cfg(test)]
impl AdditionalBranch {
    fn duplicate_for_test(&self) -> Self {
        match self {
            Self::ToHelios(values) => {
                let mut duplicate = duplicate_zeroizing_slice(values);
                Self::ToHelios(core::mem::take(&mut *duplicate))
            }
            Self::ToSelene(values) => {
                let mut duplicate = duplicate_zeroizing_slice(values);
                Self::ToSelene(core::mem::take(&mut *duplicate))
            }
        }
    }
}

/// Caller-selected rerandomization witness for one authoritative FCMP++
/// public input.
///
/// These values must be chosen with a cryptographically secure RNG before the
/// typed statement is hashed. Keeping them explicit makes O~/I~/R/C~/L
/// derivable in one pass and avoids any dependence on replaying an RNG stream.
pub struct FcmpInputRerandomizationV1 {
    output: Scalar,
    linking: Scalar,
    rerandomization_blind: Scalar,
    commitment: Scalar,
}

impl FcmpInputRerandomizationV1 {
    /// Decode four canonical non-zero Ed25519 scalars.
    pub fn new(
        mut output: [u8; 32],
        mut linking: [u8; 32],
        mut rerandomization_blind: [u8; 32],
        mut commitment: [u8; 32],
    ) -> Result<Self, FcmpNativeErrorV1> {
        let output = ProverSecretCopyValueV1::take(&mut output);
        let linking = ProverSecretCopyValueV1::take(&mut linking);
        let rerandomization_blind = ProverSecretCopyValueV1::take(&mut rerandomization_blind);
        let commitment = ProverSecretCopyValueV1::take(&mut commitment);
        let decode = |bytes: &[u8; 32]| {
            validate_edwards_scalar(*bytes)?;
            Option::<Scalar>::from(Scalar::from_canonical_bytes(*bytes))
                .filter(|scalar| *scalar != Scalar::ZERO)
                .ok_or(FcmpNativeErrorV1::ScalarEncoding)
        };
        let output_scalar = ProverSecretCopyValueV1::new(decode(output.expose_ref())?);
        let linking_scalar = ProverSecretCopyValueV1::new(decode(linking.expose_ref())?);
        let rerandomization_blind_scalar =
            ProverSecretCopyValueV1::new(decode(rerandomization_blind.expose_ref())?);
        let commitment_scalar = ProverSecretCopyValueV1::new(decode(commitment.expose_ref())?);
        Ok(Self {
            output: output_scalar.expose_copy(),
            linking: linking_scalar.expose_copy(),
            rerandomization_blind: rerandomization_blind_scalar.expose_copy(),
            commitment: commitment_scalar.expose_copy(),
        })
    }

    #[cfg(test)]
    fn duplicate_for_test(&self) -> Self {
        let output = Zeroizing::new(self.output);
        let linking = Zeroizing::new(self.linking);
        let rerandomization_blind = Zeroizing::new(self.rerandomization_blind);
        let commitment = Zeroizing::new(self.commitment);
        Self {
            output: *output,
            linking: *linking,
            rerandomization_blind: *rerandomization_blind,
            commitment: *commitment,
        }
    }
}

impl Zeroize for FcmpInputRerandomizationV1 {
    fn zeroize(&mut self) {
        self.output.zeroize();
        self.linking.zeroize();
        self.rerandomization_blind.zeroize();
        self.commitment.zeroize();
    }
}

impl Drop for FcmpInputRerandomizationV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl core::fmt::Debug for FcmpInputRerandomizationV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FcmpInputRerandomizationV1")
            .finish_non_exhaustive()
    }
}

/// Secret input and complete tree path for the native FCMP++ prover.
///
/// `additional_branches[0]` is the second-layer Helios branch (up to 18
/// canonical Helioselene-field elements), index 1 is the third-layer Selene
/// branch (up to 38 Field25519 elements), and the curves continue
/// alternating. The last branch is the shared root branch. Missing capacity
/// is canonically padded with zero by the prover.
pub struct FcmpProverInputV1 {
    output: FcmpOutputTupleV1,
    spend_x: Scalar,
    output_y: Scalar,
    rerandomization: FcmpInputRerandomizationV1,
    leaves: Vec<FcmpOutputTupleV1>,
    additional_branches: Vec<AdditionalBranch>,
}

impl Zeroize for FcmpProverInputV1 {
    fn zeroize(&mut self) {
        self.output.zeroize();
        self.spend_x.zeroize();
        self.output_y.zeroize();
        self.rerandomization.zeroize();
        self.leaves.zeroize();
        self.additional_branches.zeroize();
    }
}

impl Drop for FcmpProverInputV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl core::fmt::Debug for FcmpProverInputV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FcmpProverInputV1")
            .field("leaf_count", &self.leaves.len())
            .field("layers", &(self.additional_branches.len() + 1))
            .finish_non_exhaustive()
    }
}

impl FcmpProverInputV1 {
    /// Validate one output opening, caller-selected pseudo-out commitment
    /// rerandomization, and alternating tree path.
    ///
    /// Making the commitment rerandomization explicit lets the transaction
    /// builder choose output commitment masks that satisfy the mandatory
    /// aggregate balance equation before hashing the authoritative statement.
    pub fn new(
        output: FcmpOutputTupleV1,
        mut spend_x: [u8; 32],
        mut output_y: [u8; 32],
        rerandomization: FcmpInputRerandomizationV1,
        leaves: Vec<FcmpOutputTupleV1>,
        additional_branches: Vec<Vec<[u8; 32]>>,
    ) -> Result<Self, FcmpNativeErrorV1> {
        let spend_x_bytes = ProverSecretCopyValueV1::take(&mut spend_x);
        let output_y_bytes = ProverSecretCopyValueV1::take(&mut output_y);
        let mut leaves = Zeroizing::new(leaves);
        let additional_branches = Zeroizing::new(additional_branches);
        validate_edwards_scalar(*spend_x_bytes.expose_ref())?;
        validate_edwards_scalar(*output_y_bytes.expose_ref())?;
        let spend_x_scalar = ProverSecretCopyValueV1::new(
            Option::<Scalar>::from(Scalar::from_canonical_bytes(*spend_x_bytes.expose_ref()))
                .ok_or(FcmpNativeErrorV1::ScalarEncoding)?,
        );
        let output_y_scalar = ProverSecretCopyValueV1::new(
            Option::<Scalar>::from(Scalar::from_canonical_bytes(*output_y_bytes.expose_ref()))
                .ok_or(FcmpNativeErrorV1::ScalarEncoding)?,
        );
        if leaves.is_empty()
            || leaves.len() > FCMP_LAYER_ONE_LEN_V1
            || additional_branches.len() + 1 > usize::from(FCMP_MAX_TREE_LAYERS_V1)
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let mut leaf_ids = zeroizing_digest_buffer(leaves.len())?;
        for leaf in leaves.iter() {
            leaf_ids.push(leaf.output_id());
        }
        let output_id = Zeroizing::new(output.output_id());
        let output_present = ct_digest_slice_contains(&leaf_ids, &output_id);
        let duplicate_leaf = ct_has_duplicate_digests(&leaf_ids);
        let zero_spend = bool::from(spend_x_scalar.expose_ref().ct_eq(&Scalar::ZERO));
        if zero_spend || !output_present {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        if duplicate_leaf {
            return Err(FcmpNativeErrorV1::DuplicateOutput);
        }
        let mut decoded = Vec::with_capacity(additional_branches.len());
        for (index, branch) in additional_branches.iter().enumerate() {
            let width = if index % 2 == 0 {
                FCMP_LAYER_TWO_LEN_V1
            } else {
                FCMP_LAYER_ONE_LEN_V1
            };
            if branch.is_empty() || branch.len() > width {
                return Err(FcmpNativeErrorV1::BranchWidth);
            }
            if index % 2 == 0 {
                let mut decoded_branch = Zeroizing::new(Vec::with_capacity(branch.len()));
                for encoded in branch {
                    require_preallocated_push(decoded_branch.len(), decoded_branch.capacity())?;
                    push_secret_scalar_v1(
                        &mut decoded_branch,
                        decode_secret_helioselene_scalar_v1(encoded)?,
                    )?;
                }
                decoded.push(AdditionalBranch::ToHelios(core::mem::take(
                    &mut *decoded_branch,
                )));
            } else {
                let mut decoded_branch = Zeroizing::new(Vec::with_capacity(branch.len()));
                for encoded in branch {
                    require_preallocated_push(decoded_branch.len(), decoded_branch.capacity())?;
                    push_secret_scalar_v1(
                        &mut decoded_branch,
                        decode_secret_field25519_scalar_v1(encoded)?,
                    )?;
                }
                decoded.push(AdditionalBranch::ToSelene(core::mem::take(
                    &mut *decoded_branch,
                )));
            }
        }
        Ok(Self {
            output,
            spend_x: spend_x_scalar.expose_copy(),
            output_y: output_y_scalar.expose_copy(),
            rerandomization,
            leaves: core::mem::take(&mut *leaves),
            additional_branches: decoded,
        })
    }

    #[cfg(test)]
    fn duplicate_for_test(&self) -> Self {
        let output = Zeroizing::new(self.output);
        let spend_x = Zeroizing::new(self.spend_x);
        let output_y = Zeroizing::new(self.output_y);
        let rerandomization = self.rerandomization.duplicate_for_test();
        let mut leaves = duplicate_zeroizing_slice(&self.leaves);
        let mut additional_branches =
            Zeroizing::new(Vec::with_capacity(self.additional_branches.len()));
        for branch in &self.additional_branches {
            additional_branches.push(branch.duplicate_for_test());
        }
        Self {
            output: *output,
            spend_x: *spend_x,
            output_y: *output_y,
            rerandomization,
            leaves: core::mem::take(&mut *leaves),
            additional_branches: core::mem::take(&mut *additional_branches),
        }
    }

    /// Number of layers represented by this path.
    pub fn layers(&self) -> Result<u8, FcmpNativeErrorV1> {
        u8::try_from(self.additional_branches.len() + 1)
            .map_err(|_| FcmpNativeErrorV1::ArithmeticInvariant)
    }

    /// Derive the complete authoritative O~/I~/R/C~/L public relation before
    /// hashing the typed transaction statement.
    pub fn public_input(&self) -> Result<FcmpProofInputPublicV1, FcmpNativeErrorV1> {
        let (output_bytes, linking_bytes, commitment_bytes) = self.output.components();
        let output = decode_edwards_point(output_bytes, false)?;
        let linking = decode_edwards_point(linking_bytes, false)?;
        let amount_commitment = decode_edwards_point(commitment_bytes, false)?;
        if (ED25519_BASEPOINT_POINT * self.spend_x) + (generator_t() * self.output_y) != output {
            return Err(FcmpNativeErrorV1::SalWitnessMismatch);
        }
        let output_key_tilde = output + (generator_t() * self.rerandomization.output);
        let linking_tilde = linking + (generator_u() * self.rerandomization.linking);
        let rerandomization = (generator_v() * self.rerandomization.linking)
            + (generator_t() * self.rerandomization.rerandomization_blind);
        let pseudo_out =
            amount_commitment + (ED25519_BASEPOINT_POINT * self.rerandomization.commitment);
        let key_image = (linking_tilde * self.spend_x)
            - (generator_u() * (self.rerandomization.linking * self.spend_x));
        FcmpProofInputPublicV1::new(
            output_key_tilde.compress().to_bytes(),
            linking_tilde.compress().to_bytes(),
            rerandomization.compress().to_bytes(),
            pseudo_out.compress().to_bytes(),
            key_image.compress().to_bytes(),
        )
    }

    /// Borrow the complete canonical origin set used by a non-shipping release
    /// fixture.
    ///
    /// This is intentionally crate-private and feature-gated: production
    /// wallets retain their own output set, while the release network builder
    /// needs the public tuples to construct the exact authoritative bootstrap
    /// without exposing any spend witness.
    #[cfg(feature = "privacy-release-evidence")]
    pub(crate) fn release_origin_outputs_v1(&self) -> &[FcmpOutputTupleV1] {
        &self.leaves
    }
}

/// Complete result of native FCMP++ proving.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FcmpProvedBundleV1 {
    proof_wire: Vec<u8>,
    public_inputs: Vec<FcmpProofInputPublicV1>,
}

impl FcmpProvedBundleV1 {
    /// Canonical IFC1 proof bytes.
    pub fn proof_wire(&self) -> &[u8] {
        &self.proof_wire
    }

    /// Authoritative public input relations generated with the proof.
    pub fn public_inputs(&self) -> &[FcmpProofInputPublicV1] {
        &self.public_inputs
    }

    /// Consume the bundle.
    pub fn into_parts(self) -> (Vec<u8>, Vec<FcmpProofInputPublicV1>) {
        (self.proof_wire, self.public_inputs)
    }
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
fn fcmp_fixture_spendable_output_from_scalars_v1(
    spend_x: Scalar,
    output_y: Scalar,
    linking: Scalar,
    amount: u64,
    commitment_mask: Scalar,
) -> Result<(FcmpOutputTupleV1, [u8; 32], [u8; 32]), FcmpNativeErrorV1> {
    let output = FcmpOutputTupleV1::new(
        ((ED25519_BASEPOINT_POINT * spend_x) + (generator_t() * output_y))
            .compress()
            .to_bytes(),
        (ED25519_BASEPOINT_POINT * linking).compress().to_bytes(),
        (super::range::amount_generator()? * Scalar::from(amount)
            + ED25519_BASEPOINT_POINT * commitment_mask)
            .compress()
            .to_bytes(),
    )?;
    Ok((output, spend_x.to_bytes(), output_y.to_bytes()))
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
fn fcmp_fixture_spendable_output_v1(
    spend_x: u64,
    output_y: u64,
    linking: u64,
    amount: u64,
    commitment_mask: u64,
) -> Result<(FcmpOutputTupleV1, [u8; 32], [u8; 32]), FcmpNativeErrorV1> {
    fcmp_fixture_spendable_output_from_scalars_v1(
        Scalar::from(spend_x),
        Scalar::from(output_y),
        Scalar::from(linking),
        amount,
        Scalar::from(commitment_mask),
    )
}

#[cfg(test)]
pub(crate) fn fcmp_test_spendable_output_v1(
    spend_x: u64,
    output_y: u64,
    linking: u64,
    amount: u64,
    commitment_mask: u64,
) -> (FcmpOutputTupleV1, [u8; 32], [u8; 32]) {
    fcmp_fixture_spendable_output_v1(spend_x, output_y, linking, amount, commitment_mask)
        .expect("non-zero test scalars construct canonical FCMP++ points")
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
fn fcmp_fixture_output_opening_v1(
    output_key: u64,
    linking: u64,
    amount: u64,
    mask: u64,
) -> Result<FcmpOutputCommitmentOpeningV1, FcmpNativeErrorV1> {
    let mask = Scalar::from(mask);
    let output = FcmpOutputTupleV1::new(
        (ED25519_BASEPOINT_POINT * Scalar::from(output_key))
            .compress()
            .to_bytes(),
        (ED25519_BASEPOINT_POINT * Scalar::from(linking))
            .compress()
            .to_bytes(),
        (super::range::amount_generator()? * Scalar::from(amount) + ED25519_BASEPOINT_POINT * mask)
            .compress()
            .to_bytes(),
    )?;
    FcmpOutputCommitmentOpeningV1::new(output, amount, mask.to_bytes())
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
fn fcmp_fixture_rerandomization_v1(
    output: u64,
    linking: u64,
    blind: u64,
    commitment: u64,
) -> Result<FcmpInputRerandomizationV1, FcmpNativeErrorV1> {
    FcmpInputRerandomizationV1::new(
        Scalar::from(output).to_bytes(),
        Scalar::from(linking).to_bytes(),
        Scalar::from(blind).to_bytes(),
        Scalar::from(commitment).to_bytes(),
    )
}

/// Build the canonical deterministic FCMP++ release fixture.
///
/// The maximum fixture binds two inputs, four strictly-positive outputs, and
/// all 32 alternating curve-tree layers. The smaller fixture binds one input,
/// one output, and one layer. Secret-bearing typed witnesses are returned only
/// inside tests or the non-shipping release-evidence feature.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn fcmp_release_fixture_v1(
    maximum: bool,
) -> Result<
    (
        Vec<FcmpProverInputV1>,
        Vec<FcmpOutputCommitmentOpeningV1>,
        FcmpTreeRootV1,
    ),
    FcmpNativeErrorV1,
> {
    const INPUT_AMOUNT: u64 = 5;
    if !maximum {
        let (output, spend_x, output_y) =
            fcmp_fixture_spendable_output_v1(17, 23, 31, INPUT_AMOUNT, 37)?;
        let new_output = fcmp_fixture_output_opening_v1(43, 47, INPUT_AMOUNT, 37 + 41)?;
        let root = super::build_fcmp_frontier_v1(&[output])?.root;
        let input = FcmpProverInputV1::new(
            output,
            spend_x,
            output_y,
            fcmp_fixture_rerandomization_v1(61, 67, 71, 41)?,
            vec![output],
            Vec::new(),
        )?;
        return Ok((vec![input], vec![new_output], root));
    }

    let (output_1, spend_x_1, output_y_1) =
        fcmp_fixture_spendable_output_v1(401, 409, 431, INPUT_AMOUNT, 149)?;
    let (output_2, spend_x_2, output_y_2) =
        fcmp_fixture_spendable_output_v1(419, 421, 433, INPUT_AMOUNT, 157)?;
    let mut leaves = Zeroizing::new(vec![output_1, output_2]);

    let mut leaf_coordinates = Vec::with_capacity(6 * leaves.len());
    for leaf in leaves.iter() {
        let (output_key, linking_tag_generator, amount_commitment) = leaf.components();
        for point in [output_key, linking_tag_generator, amount_commitment] {
            let (x, y) = edwards_to_wei25519(point)?;
            leaf_coordinates.extend([x, y]);
        }
    }
    let mut current_selene = hash_selene(&leaf_coordinates)?;
    let mut current_helios = None;
    let mut branches = Zeroizing::new(Vec::with_capacity(usize::from(
        FCMP_MAX_TREE_LAYERS_V1.saturating_sub(1),
    )));
    for branch_index in 0..usize::from(FCMP_MAX_TREE_LAYERS_V1 - 1) {
        if branch_index % 2 == 0 {
            let child = current_selene
                .x()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
            branches.push(vec![super::field::encode_helioselene_scalar(child)]);
            current_helios = Some(hash_helios(&[child])?);
        } else {
            let child = current_helios
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
                .x()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
            branches.push(vec![super::field::encode_field25519_scalar(child)]);
            current_selene = hash_selene(&[child])?;
        }
    }
    let root = FcmpTreeRootV1::new(
        FCMP_MAX_TREE_LAYERS_V1,
        current_helios
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
            .encode(),
    )?;

    let mut first_leaves = duplicate_zeroizing_slice(&leaves);
    let mut first_branches = duplicate_zeroizing_nested_slices(&branches);
    let first_input = FcmpProverInputV1::new(
        output_1,
        spend_x_1,
        output_y_1,
        fcmp_fixture_rerandomization_v1(439, 443, 449, 163)?,
        core::mem::take(&mut *first_leaves),
        core::mem::take(&mut *first_branches),
    )?;
    let second_input = FcmpProverInputV1::new(
        output_2,
        spend_x_2,
        output_y_2,
        fcmp_fixture_rerandomization_v1(457, 461, 463, 167)?,
        core::mem::take(&mut *leaves),
        core::mem::take(&mut *branches),
    )?;
    let inputs = vec![first_input, second_input];

    // Input masks plus their pseudo-out rerandomizations total 636. These
    // four masks preserve the aggregate while positive amounts total 10.
    let outputs = vec![
        fcmp_fixture_output_opening_v1(467, 479, 1, 101)?,
        fcmp_fixture_output_opening_v1(487, 491, 2, 103)?,
        fcmp_fixture_output_opening_v1(499, 503, 3, 107)?,
        fcmp_fixture_output_opening_v1(509, 521, 4, 325)?,
    ];
    Ok((inputs, outputs, root))
}

/// Build a maximum-shape fixture whose first canonical branch does not resolve
/// to the supplied authoritative root.
///
/// This is feature-only negative-test material. The public prover must reject
/// it during deterministic path preflight before producing any proof.
#[cfg(feature = "privacy-release-evidence")]
pub(crate) fn fcmp_release_invalid_path_fixture_v1() -> Result<
    (
        Vec<FcmpProverInputV1>,
        Vec<FcmpOutputCommitmentOpeningV1>,
        FcmpTreeRootV1,
    ),
    FcmpNativeErrorV1,
> {
    let (mut inputs, outputs, root) = fcmp_release_fixture_v1(true)?;
    let first_input = inputs
        .first_mut()
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    let first_branch = first_input
        .additional_branches
        .first_mut()
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    match first_branch {
        AdditionalBranch::ToHelios(values) => {
            let value = values
                .first_mut()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
            *value = if *value == HelioseleneField::ONE {
                HelioseleneField::ONE + HelioseleneField::ONE
            } else {
                HelioseleneField::ONE
            };
        }
        AdditionalBranch::ToSelene(values) => {
            let value = values
                .first_mut()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
            *value = if *value == Field25519::ONE {
                Field25519::ONE + Field25519::ONE
            } else {
                Field25519::ONE
            };
        }
    }
    Ok((inputs, outputs, root))
}

enum RootValues {
    C1(Vec<Field25519>),
    C2(Vec<HelioseleneField>),
}

fn root_values_ct_eq(left: &RootValues, right: &RootValues) -> Choice {
    // Variant and length are fixed by the public layer count. Only coordinate
    // contents are private, and equal-shape roots always scan every one.
    match (left, right) {
        (RootValues::C1(left), RootValues::C1(right)) => {
            ct_equal_slices_by(left, right, |left, right| (*left - *right).ct_is_zero())
        }
        (RootValues::C2(left), RootValues::C2(right)) => {
            ct_equal_slices_by(left, right, |left, right| (*left - *right).ct_is_zero())
        }
        _ => Choice::from(0),
    }
}

fn all_paths_share_root(paths: &[PathValues], shared_root: &RootValues) -> bool {
    bool::from(ct_all_match_by(paths, shared_root, |path, shared_root| {
        root_values_ct_eq(&path.root, shared_root)
    }))
}

impl Zeroize for RootValues {
    fn zeroize(&mut self) {
        match self {
            Self::C1(values) => values.zeroize(),
            Self::C2(values) => values.zeroize(),
        }
    }
}

impl Drop for RootValues {
    fn drop(&mut self) {
        self.zeroize();
    }
}

struct PathValues {
    c1_non_root: Vec<Vec<Field25519>>,
    c2_non_root: Vec<Vec<HelioseleneField>>,
    root: RootValues,
}

impl Drop for PathValues {
    fn drop(&mut self) {
        self.c1_non_root.zeroize();
        self.c2_non_root.zeroize();
        self.root.zeroize();
    }
}

#[derive(Clone)]
struct TranscriptedPath {
    c1_non_root: Vec<Vec<Variable>>,
    c2_non_root: Vec<Vec<Variable>>,
}

fn parse_path(
    input: &FcmpProverInputV1,
    root: FcmpTreeRootV1,
) -> Result<PathValues, FcmpNativeErrorV1> {
    if input.layers()? != root.layers() {
        return Err(FcmpNativeErrorV1::ProofHeaderMismatch);
    }
    let mut leaves = Zeroizing::new(Vec::with_capacity(6 * FCMP_LAYER_ONE_LEN_V1));
    for leaf in &input.leaves {
        let (output, linking, commitment) = leaf.components();
        for point in [output, linking, commitment] {
            let (x, y) = edwards_to_wei25519(point)?;
            leaves.extend([x, y]);
        }
    }
    leaves.resize(6 * FCMP_LAYER_ONE_LEN_V1, Field25519::ZERO);
    let mut current_c1 = Zeroizing::new(Some(hash_selene(&leaves)?));
    let mut current_c2 = Zeroizing::new(None);
    let private_branch_capacity = input.additional_branches.len().saturating_add(1);
    let mut c1_non_root = Zeroizing::new(Vec::with_capacity(private_branch_capacity));
    let mut c2_non_root = Zeroizing::new(Vec::with_capacity(private_branch_capacity));

    if input.additional_branches.is_empty() {
        let expected = SelenePoint::decode(root.point(), false)?;
        let matches_expected = Option::as_ref(&current_c1)
            .map_or(false, |actual| ct_selene_point_eq(actual, &expected));
        if root.curve() != FcmpTreeCurveV1::Selene || !matches_expected {
            return Err(FcmpNativeErrorV1::RootMismatch);
        }
        return Ok(PathValues {
            c1_non_root: core::mem::take(&mut *c1_non_root),
            c2_non_root: core::mem::take(&mut *c2_non_root),
            root: RootValues::C1(core::mem::take(&mut *leaves)),
        });
    }
    c1_non_root.push(core::mem::take(&mut *leaves));

    let last = input.additional_branches.len() - 1;
    let mut root_values = None;
    for (index, branch) in input.additional_branches.iter().enumerate() {
        match branch {
            AdditionalBranch::ToHelios(branch) => {
                let prior_x = current_c1
                    .take()
                    .and_then(SelenePoint::x)
                    .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
                let mut padded = Zeroizing::new(Vec::with_capacity(FCMP_LAYER_TWO_LEN_V1));
                let allocation_capacity = padded.capacity();
                padded.extend_from_slice(branch);
                padded.resize(FCMP_LAYER_TWO_LEN_V1, HelioseleneField::ZERO);
                debug_assert!(allocation_capacity >= FCMP_LAYER_TWO_LEN_V1);
                debug_assert_eq!(padded.capacity(), allocation_capacity);
                if !ct_helioselene_slice_contains(&padded, prior_x) {
                    return Err(FcmpNativeErrorV1::ArithmeticInvariant);
                }
                *current_c2 = Some(hash_helios(&padded)?);
                if index == last {
                    root_values = Some(RootValues::C2(core::mem::take(&mut *padded)));
                } else {
                    c2_non_root.push(core::mem::take(&mut *padded));
                }
            }
            AdditionalBranch::ToSelene(branch) => {
                let prior_x = current_c2
                    .take()
                    .and_then(HeliosPoint::x)
                    .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
                let mut padded = Zeroizing::new(Vec::with_capacity(FCMP_LAYER_ONE_LEN_V1));
                let allocation_capacity = padded.capacity();
                padded.extend_from_slice(branch);
                padded.resize(FCMP_LAYER_ONE_LEN_V1, Field25519::ZERO);
                debug_assert!(allocation_capacity >= FCMP_LAYER_ONE_LEN_V1);
                debug_assert_eq!(padded.capacity(), allocation_capacity);
                if !ct_field25519_slice_contains(&padded, prior_x) {
                    return Err(FcmpNativeErrorV1::ArithmeticInvariant);
                }
                *current_c1 = Some(hash_selene(&padded)?);
                if index == last {
                    root_values = Some(RootValues::C1(core::mem::take(&mut *padded)));
                } else {
                    c1_non_root.push(core::mem::take(&mut *padded));
                }
            }
        }
    }

    let matches_root = match root.curve() {
        FcmpTreeCurveV1::Selene => {
            let expected = SelenePoint::decode(root.point(), false)?;
            Option::as_ref(&current_c1)
                .map_or(false, |actual| ct_selene_point_eq(actual, &expected))
        }
        FcmpTreeCurveV1::Helios => {
            let expected = HeliosPoint::decode(root.point(), false)?;
            Option::as_ref(&current_c2)
                .map_or(false, |actual| ct_helios_point_eq(actual, &expected))
        }
    };
    if !matches_root {
        return Err(FcmpNativeErrorV1::RootMismatch);
    }
    Ok(PathValues {
        c1_non_root: core::mem::take(&mut *c1_non_root),
        c2_non_root: core::mem::take(&mut *c2_non_root),
        root: root_values.ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
    })
}

fn random_proof_scalar<F: ProofScalar>(
    rng: &mut (impl RngCore + CryptoRng),
) -> Result<F, FcmpNativeErrorV1> {
    for _ in 0..MAX_PROVER_SCALAR_ATTEMPTS_V1 {
        if let Some(mut scalar) = random_scalar_from_fcmp_rng::<F, _>(rng)? {
            let scalar = ProverSecretScalarV1::take(&mut scalar);
            if !scalar.expose_copy().is_zero() {
                return Ok(scalar.expose_copy());
            }
        }
    }
    Err(FcmpNativeErrorV1::ProverRandomnessExhausted)
}

fn root_nonce_commitment_v1<S: ProofSuite>(
    nonce: &S::Scalar,
) -> Result<S::Point, FcmpNativeErrorV1> {
    let mut terms = SecretMultiexpBuilder::<S>::new(1)?;
    terms.push(nonce, &S::generators().h)?;
    terms.evaluate().map_err(Into::into)
}

fn prepared_secret_point_v1<S: ProofSuite>(
    scalar: &S::Scalar,
) -> Result<ProverSecretPointV1<S::Point>, FcmpNativeErrorV1> {
    let mut terms = SecretMultiexpBuilder::<S>::new(1)?;
    terms.push(scalar, &S::generators().h)?;
    let mut point = terms.evaluate()?;
    Ok(ProverSecretPointV1::take(&mut point))
}

struct PreparedEdBlind {
    decomposition: Vec<u64>,
    divisor: NormalizedDivisor<Field25519>,
    coordinates: (Field25519, Field25519),
}

impl Drop for PreparedEdBlind {
    fn drop(&mut self) {
        self.decomposition.zeroize();
        self.coordinates.0.zeroize();
        self.coordinates.1.zeroize();
    }
}

fn prepare_ed_blind(
    generator: EdwardsPoint,
    scalar: &Scalar,
    negate: bool,
) -> Result<PreparedEdBlind, FcmpNativeErrorV1> {
    let scalar = Zeroizing::new(if negate { -*scalar } else { *scalar });
    let mut decomposition = ed25519_scalar_decomposition(&scalar)?;
    let point = Zeroizing::new(&generator * &*scalar);
    let encoded_point = Zeroizing::new(point.compress().to_bytes());
    let coordinates = Zeroizing::new(secret_edwards_to_wei25519_v1(&encoded_point)?);
    let curve = ed25519_curve();
    let divisor = scalar_mul_divisor(curve.a, curve.b, generator, &decomposition, &point)?;
    Ok(PreparedEdBlind {
        decomposition: core::mem::take(&mut *decomposition),
        divisor,
        coordinates: *coordinates,
    })
}

struct PreparedSeleneBlind {
    scalar: Field25519,
    decomposition: Vec<u64>,
    divisor: NormalizedDivisor<HelioseleneField>,
    point: SelenePoint,
}

impl Drop for PreparedSeleneBlind {
    fn drop(&mut self) {
        self.scalar.zeroize();
        self.decomposition.zeroize();
        self.point.zeroize();
    }
}

fn prepare_selene_blind(mut scalar: Field25519) -> Result<PreparedSeleneBlind, FcmpNativeErrorV1> {
    let scalar = ProverSecretScalarV1::take(&mut scalar);
    let mut decomposition =
        scalar_decomposition(scalar.expose_ref(), CYCLE_DLOG_PARAMETERS.scalar_bits)?;
    let generator = selene_bp_generators().h;
    let point = prepared_secret_point_v1::<SeleneSuite>(scalar.expose_ref())?;
    let curve = selene_curve();
    let divisor = scalar_mul_divisor(
        curve.a,
        curve.b,
        generator,
        &decomposition,
        point.expose_ref(),
    )?;
    Ok(PreparedSeleneBlind {
        scalar: scalar.expose_copy(),
        decomposition: core::mem::take(&mut *decomposition),
        divisor,
        point: point.expose_copy(),
    })
}

struct PreparedHeliosBlind {
    scalar: HelioseleneField,
    decomposition: Vec<u64>,
    divisor: NormalizedDivisor<Field25519>,
    point: HeliosPoint,
}

impl Drop for PreparedHeliosBlind {
    fn drop(&mut self) {
        self.scalar.zeroize();
        self.decomposition.zeroize();
        self.point.zeroize();
    }
}

fn prepare_helios_blind(
    mut scalar: HelioseleneField,
) -> Result<PreparedHeliosBlind, FcmpNativeErrorV1> {
    let scalar = ProverSecretScalarV1::take(&mut scalar);
    let mut decomposition =
        scalar_decomposition(scalar.expose_ref(), CYCLE_DLOG_PARAMETERS.scalar_bits)?;
    let generator = helios_bp_generators().h;
    let point = prepared_secret_point_v1::<HeliosSuite>(scalar.expose_ref())?;
    let curve = helios_curve();
    let divisor = scalar_mul_divisor(
        curve.a,
        curve.b,
        generator,
        &decomposition,
        point.expose_ref(),
    )?;
    Ok(PreparedHeliosBlind {
        scalar: scalar.expose_copy(),
        decomposition: core::mem::take(&mut *decomposition),
        divisor,
        point: point.expose_copy(),
    })
}

fn commitment_index(variable: Variable) -> Result<usize, FcmpNativeErrorV1> {
    match variable {
        Variable::CG { commitment, .. } => Ok(commitment),
        _ => Err(FcmpNativeErrorV1::ArithmeticInvariant),
    }
}

struct PreparedInput {
    public: FcmpProofInputPublicV1,
    sal: super::FcmpSalProofV1,
    transcripted: TranscriptedInput,
}

fn prove_fcmp_plus_plus_once_v1(
    rng: &mut (impl RngCore + CryptoRng),
    context_hash: [u8; 32],
    inputs: &[FcmpProverInputV1],
    new_output_openings: &[FcmpOutputCommitmentOpeningV1],
    root: FcmpTreeRootV1,
) -> Result<FcmpProvedBundleV1, FcmpNativeErrorV1> {
    if inputs.is_empty() || inputs.len() > super::FCMP_MAX_INPUTS_NATIVE_V1 {
        return Err(FcmpNativeErrorV1::InputCount {
            actual: inputs.len(),
            max: super::FCMP_MAX_INPUTS_NATIVE_V1,
        });
    }
    let mut spent_outputs = zeroizing_digest_buffer(inputs.len())?;
    let mut derived_key_images = zeroizing_digest_buffer(inputs.len())?;
    for input in inputs {
        spent_outputs.push(input.output.output_id());
        let linking = decode_edwards_point(input.output.components().1, false)?;
        let key_image = (linking * input.spend_x).compress().to_bytes();
        derived_key_images.push(key_image);
    }
    if ct_has_duplicate_digests(&spent_outputs) {
        return Err(FcmpNativeErrorV1::DuplicateOutput);
    }
    if ct_has_duplicate_digests(&derived_key_images) {
        return Err(FcmpNativeErrorV1::DuplicateKeyImage);
    }
    if new_output_openings.is_empty()
        || new_output_openings.len() > super::FCMP_MAX_OUTPUTS_NATIVE_V1
    {
        return Err(FcmpNativeErrorV1::OutputCount {
            actual: new_output_openings.len(),
            max: super::FCMP_MAX_OUTPUTS_NATIVE_V1,
        });
    }
    let new_outputs = new_output_openings
        .iter()
        .map(FcmpOutputCommitmentOpeningV1::output)
        .collect::<Vec<_>>();
    let mut new_output_ids = zeroizing_digest_buffer(new_outputs.len())?;
    for output in &new_outputs {
        new_output_ids.push(output.output_id());
    }
    if ct_has_duplicate_digests(&new_output_ids) {
        return Err(FcmpNativeErrorV1::DuplicateOutput);
    }
    let layers = usize::from(root.layers());
    let mut paths = Vec::with_capacity(inputs.len());
    for input in inputs {
        paths.push(parse_path(input, root)?);
    }
    let shared_root = &paths
        .first()
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
        .root;
    if !all_paths_share_root(&paths, shared_root) {
        return Err(FcmpNativeErrorV1::RootMismatch);
    }

    let (c1_rows, c2_rows) = ipa_rows(inputs.len(), layers)?;
    let c1_generators = <SeleneSuite as ProofSuite>::generators().reduce(c1_rows)?;
    let c2_generators = <HeliosSuite as ProofSuite>::generators().reduce(c2_rows)?;
    let mut c1_tape = ProverVectorCommitmentTape::new(c1_rows)?;
    let mut c2_tape = ProverVectorCommitmentTape::new(c2_rows)?;
    let c1_non_root_count = paths.iter().try_fold(0_usize, |count, path| {
        count
            .checked_add(path.c1_non_root.len())
            .ok_or(FcmpNativeErrorV1::TreeFull)
    })?;
    let c2_non_root_count = paths.iter().try_fold(0_usize, |count, path| {
        count
            .checked_add(path.c2_non_root.len())
            .ok_or(FcmpNativeErrorV1::TreeFull)
    })?;
    let mut transcripted_paths = Vec::with_capacity(paths.len());
    // The row counts are public upper bounds for every subsequently appended
    // vector commitment. Allocate them before accepting any branch mask so no
    // private scalar can be copied by a Vec growth operation.
    let mut c1_branch_masks = Zeroizing::new(Vec::with_capacity(c1_rows));
    let mut c2_branch_masks = Zeroizing::new(Vec::with_capacity(c2_rows));
    let mut selene_blinds = Vec::with_capacity(c1_non_root_count);
    let mut helios_blinds = Vec::with_capacity(c2_non_root_count);

    for path in &paths {
        let mut c1_non_root = Vec::with_capacity(path.c1_non_root.len());
        for branch in &path.c1_non_root {
            c1_non_root.push(c1_tape.append_branch(branch)?);
            require_preallocated_push(c1_branch_masks.len(), c1_branch_masks.capacity())?;
            require_preallocated_push(selene_blinds.len(), selene_blinds.capacity())?;
            let blind = prepare_selene_blind(random_proof_scalar(rng)?)?;
            push_secret_scalar_v1(&mut c1_branch_masks, blind.scalar.neg_ref())?;
            selene_blinds.push(blind);
        }
        let mut c2_non_root = Vec::with_capacity(path.c2_non_root.len());
        for branch in &path.c2_non_root {
            c2_non_root.push(c2_tape.append_branch(branch)?);
            require_preallocated_push(c2_branch_masks.len(), c2_branch_masks.capacity())?;
            require_preallocated_push(helios_blinds.len(), helios_blinds.capacity())?;
            let blind = prepare_helios_blind(random_proof_scalar(rng)?)?;
            push_secret_scalar_v1(&mut c2_branch_masks, blind.scalar.neg_ref())?;
            helios_blinds.push(blind);
        }
        transcripted_paths.push(TranscriptedPath {
            c1_non_root,
            c2_non_root,
        });
    }
    let root_variables = match shared_root {
        RootValues::C1(values) => {
            let variables = c1_tape.append_branch(values)?;
            require_preallocated_push(c1_branch_masks.len(), c1_branch_masks.capacity())?;
            push_secret_scalar_v1(&mut c1_branch_masks, random_proof_scalar(rng)?)?;
            variables
        }
        RootValues::C2(values) => {
            let variables = c2_tape.append_branch(values)?;
            require_preallocated_push(c2_branch_masks.len(), c2_branch_masks.capacity())?;
            push_secret_scalar_v1(&mut c2_branch_masks, random_proof_scalar(rng)?)?;
            variables
        }
    };
    let root_commitment_index = commitment_index(
        *root_variables
            .first()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
    )?;

    let mut prepared_inputs = Vec::with_capacity(inputs.len());
    let mut generated_pseudo_outs = zeroizing_digest_buffer(inputs.len())?;
    for input in inputs {
        let (output_bytes, linking_bytes, commitment_bytes) = input.output.components();
        let public = input.public_input()?;
        generated_pseudo_outs.push(public.pseudo_out);
        let rerandomization = &input.rerandomization;
        let output_blind = prepare_ed_blind(generator_t(), &rerandomization.output, true)?;
        let input_blind_u = prepare_ed_blind(generator_u(), &rerandomization.linking, true)?;
        let input_blind_v = prepare_ed_blind(generator_v(), &rerandomization.linking, true)?;
        let input_blind_blind =
            prepare_ed_blind(generator_t(), &rerandomization.rerandomization_blind, false)?;
        let commitment_blind =
            prepare_ed_blind(ED25519_BASEPOINT_POINT, &rerandomization.commitment, true)?;
        let output_coordinates = Zeroizing::new(edwards_to_wei25519(output_bytes)?);
        let linking_coordinates = Zeroizing::new(edwards_to_wei25519(linking_bytes)?);
        let commitment_coordinates = Zeroizing::new(edwards_to_wei25519(commitment_bytes)?);
        let output_padding = Zeroizing::new([output_coordinates.0, output_coordinates.1]);
        let linking_padding = Zeroizing::new([linking_coordinates.0, linking_coordinates.1]);
        let input_blind_v_padding =
            Zeroizing::new([input_blind_v.coordinates.0, input_blind_v.coordinates.1]);
        let commitment_padding =
            Zeroizing::new([commitment_coordinates.0, commitment_coordinates.1]);

        let (output_blind_claim, output_variables) = c1_tape.append_claimed_point(
            ED25519_DLOG_PARAMETERS,
            &output_blind.decomposition,
            &output_blind.divisor,
            &output_blind.coordinates,
            &output_padding[..],
        )?;
        let (input_blind_u_claim, linking_variables) = c1_tape.append_claimed_point(
            ED25519_DLOG_PARAMETERS,
            &input_blind_u.decomposition,
            &input_blind_u.divisor,
            &input_blind_u.coordinates,
            &linking_padding[..],
        )?;
        let (input_blind_v_divisor, _) = c1_tape.append_divisor(
            ED25519_DLOG_PARAMETERS,
            &input_blind_v.divisor,
            &Field25519::ZERO,
        )?;
        let (input_blind_blind_claim, input_blind_v_variables) = c1_tape.append_claimed_point(
            ED25519_DLOG_PARAMETERS,
            &input_blind_blind.decomposition,
            &input_blind_blind.divisor,
            &input_blind_blind.coordinates,
            &input_blind_v_padding[..],
        )?;
        let (commitment_blind_claim, commitment_variables) = c1_tape.append_claimed_point(
            ED25519_DLOG_PARAMETERS,
            &commitment_blind.decomposition,
            &commitment_blind.divisor,
            &commitment_blind.coordinates,
            &commitment_padding[..],
        )?;
        if output_variables.len() != 2
            || linking_variables.len() != 2
            || input_blind_v_variables.len() != 2
            || commitment_variables.len() != 2
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let input_blind_v_claim = PointWithDlog {
            point: (input_blind_v_variables[0], input_blind_v_variables[1]),
            dlog: input_blind_u_claim.dlog.clone(),
            divisor: input_blind_v_divisor,
        };
        let sal_y = Zeroizing::new(input.output_y + rerandomization.output);
        let sal_witness = FcmpSalWitnessV1::new(
            input.spend_x.to_bytes(),
            sal_y.to_bytes(),
            rerandomization.linking.to_bytes(),
            rerandomization.rerandomization_blind.to_bytes(),
        )?;
        let sal = prove_fcmp_sal_with_checked_rng_v1(rng, context_hash, &public, &sal_witness)?;
        prepared_inputs.push(PreparedInput {
            public,
            sal,
            transcripted: TranscriptedInput {
                output_key: (output_variables[0], output_variables[1]),
                linking_generator: (linking_variables[0], linking_variables[1]),
                amount_commitment: (commitment_variables[0], commitment_variables[1]),
                output_blind: output_blind_claim,
                input_blind_u: input_blind_u_claim,
                input_blind_v: input_blind_v_claim,
                input_blind_blind: input_blind_blind_claim,
                commitment_blind: commitment_blind_claim,
            },
        });
    }
    if ct_has_duplicate_digests(&generated_pseudo_outs) {
        return Err(FcmpNativeErrorV1::DuplicatePseudoOut);
    }

    // The first proof field opens Helios branch blinds; the second opens
    // Selene branch blinds.
    let mut c1_blind_claims = Vec::with_capacity(helios_blinds.len());
    for blind in &helios_blinds {
        let coordinates = Zeroizing::new(
            blind
                .point
                .coordinates()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
        );
        c1_blind_claims.push(
            c1_tape
                .append_claimed_point(
                    CYCLE_DLOG_PARAMETERS,
                    &blind.decomposition,
                    &blind.divisor,
                    &coordinates,
                    &[],
                )?
                .0,
        );
    }
    let mut c2_blind_claims = Vec::with_capacity(selene_blinds.len());
    for blind in &selene_blinds {
        let coordinates = Zeroizing::new(
            blind
                .point
                .coordinates()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
        );
        c2_blind_claims.push(
            c2_tape
                .append_claimed_point(
                    CYCLE_DLOG_PARAMETERS,
                    &blind.decomposition,
                    &blind.divisor,
                    &coordinates,
                    &[],
                )?
                .0,
        );
    }

    if c1_tape.commitment_count() > c1_rows || c2_tape.commitment_count() > c2_rows {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    let mut c1_masks = c1_branch_masks;
    let c1_mask_allocation_capacity = c1_masks.capacity();
    while c1_masks.len() < c1_tape.commitment_count() {
        require_preallocated_push(c1_masks.len(), c1_masks.capacity())?;
        push_secret_scalar_v1(&mut c1_masks, random_proof_scalar(rng)?)?;
        debug_assert_eq!(c1_masks.capacity(), c1_mask_allocation_capacity);
    }
    let mut c2_masks = c2_branch_masks;
    let c2_mask_allocation_capacity = c2_masks.capacity();
    while c2_masks.len() < c2_tape.commitment_count() {
        require_preallocated_push(c2_masks.len(), c2_masks.capacity())?;
        push_secret_scalar_v1(&mut c2_masks, random_proof_scalar(rng)?)?;
        debug_assert_eq!(c2_masks.capacity(), c2_mask_allocation_capacity);
    }
    let root_mask_c1 =
        (root.curve() == FcmpTreeCurveV1::Selene).then(|| &c1_masks[root_commitment_index]);
    let root_mask_c2 =
        (root.curve() == FcmpTreeCurveV1::Helios).then(|| &c2_masks[root_commitment_index]);
    let (c1_commitments, c1_openings) =
        c1_tape.commitments_and_openings::<SeleneSuite>(c1_generators, c1_masks.as_slice())?;
    let (c2_commitments, c2_openings) =
        c2_tape.commitments_and_openings::<HeliosSuite>(c2_generators, c2_masks.as_slice())?;

    let (root_blind_commitment, mut root_nonce_c1, mut root_nonce_c2): (
        [u8; 32],
        Option<ProverSecretScalarV1<Field25519>>,
        Option<ProverSecretScalarV1<HelioseleneField>>,
    ) = match root.curve() {
        FcmpTreeCurveV1::Selene => {
            let mut nonce = random_proof_scalar::<Field25519>(rng)?;
            let nonce = ProverSecretScalarV1::take(&mut nonce);
            (
                root_nonce_commitment_v1::<SeleneSuite>(nonce.expose_ref())?.encode(),
                Some(nonce),
                None,
            )
        }
        FcmpTreeCurveV1::Helios => {
            let mut nonce = random_proof_scalar::<HelioseleneField>(rng)?;
            let nonce = ProverSecretScalarV1::take(&mut nonce);
            (
                root_nonce_commitment_v1::<HeliosSuite>(nonce.expose_ref())?.encode(),
                None,
                Some(nonce),
            )
        }
    };
    let public_inputs = prepared_inputs
        .iter()
        .map(|input| input.public)
        .collect::<Vec<_>>();
    let mut pseudo_outs = zeroizing_digest_buffer(public_inputs.len())?;
    let mut key_images = zeroizing_digest_buffer(public_inputs.len())?;
    for public in &public_inputs {
        pseudo_outs.push(public.pseudo_out);
        key_images.push(public.key_image);
    }
    if ct_has_duplicate_digests(&pseudo_outs) {
        return Err(FcmpNativeErrorV1::DuplicatePseudoOut);
    }
    if ct_has_duplicate_digests(&key_images) {
        return Err(FcmpNativeErrorV1::DuplicateKeyImage);
    }
    super::verify_fcmp_commitment_balance_v1(&public_inputs, &new_outputs)?;

    let context = membership_context(root, &public_inputs, root_blind_commitment)?;
    let mut transcript = ProverTranscript::new(context);
    transcript.write_commitments::<SeleneSuite>(c1_commitments.clone(), Vec::new());
    transcript.write_commitments::<HeliosSuite>(c2_commitments.clone(), Vec::new());
    let root_blind_response = match root.curve() {
        FcmpTreeCurveV1::Selene => {
            let challenge = transcript.challenge::<SeleneSuite>()?;
            let nonce = root_nonce_c1
                .as_mut()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
            let mask = root_mask_c1.ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
            nonce.add_product_assign(&challenge, mask);
            nonce.expose_copy().encode()
        }
        FcmpTreeCurveV1::Helios => {
            let challenge = transcript.challenge::<HeliosSuite>()?;
            let nonce = root_nonce_c2
                .as_mut()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
            let mask = root_mask_c2.ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
            nonce.add_product_assign(&challenge, mask);
            nonce.expose_copy().encode()
        }
    };

    let mut c1_circuit = Circuit::<SeleneSuite>::prove(c1_openings, c1_rows)?;
    let mut c2_circuit = Circuit::<HeliosSuite>::prove(c2_openings, c2_rows)?;
    let mut c1_dlog_challenge = None;
    let mut c2_dlog_challenge = None;
    let mut c1_commitment_openings = c1_commitments
        .iter()
        .copied()
        .zip(c1_masks.iter())
        .zip(c2_blind_claims)
        .map(|((commitment, mask), blind)| (commitment, Some(mask), blind));
    let mut c2_commitment_openings = c2_commitments
        .iter()
        .copied()
        .zip(c2_masks.iter())
        .zip(c1_blind_claims)
        .map(|((commitment, mask), blind)| (commitment, Some(mask), blind));
    for ((path, prepared), public) in transcripted_paths
        .into_iter()
        .zip(prepared_inputs.iter())
        .zip(&public_inputs)
    {
        constrain_input(
            native_parameters(),
            layers,
            &mut transcript,
            &mut c1_circuit,
            &mut c1_dlog_challenge,
            &mut c2_circuit,
            &mut c2_dlog_challenge,
            &root_variables,
            &mut path.c1_non_root.into_iter(),
            &mut path.c2_non_root.into_iter(),
            &mut c1_commitment_openings,
            &mut c2_commitment_openings,
            public,
            prepared.transcripted.clone(),
        )?;
    }
    if c1_commitment_openings.next().is_some() || c2_commitment_openings.next().is_some() {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }

    let expected_c1_muls = inputs
        .len()
        .checked_mul(
            97_usize
                .checked_add(
                    layers
                        .saturating_sub(1)
                        .checked_div(2)
                        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
                        .checked_mul(52)
                        .ok_or(FcmpNativeErrorV1::TreeFull)?,
                )
                .ok_or(FcmpNativeErrorV1::TreeFull)?,
        )
        .ok_or(FcmpNativeErrorV1::TreeFull)?;
    let expected_c2_muls = inputs
        .len()
        .checked_mul(
            (layers / 2)
                .checked_mul(32)
                .ok_or(FcmpNativeErrorV1::TreeFull)?,
        )
        .ok_or(FcmpNativeErrorV1::TreeFull)?;
    if c1_circuit.muls() != expected_c1_muls || c2_circuit.muls() != expected_c2_muls {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    let (c1_statement, c1_witness) = c1_circuit.proving_statement(c1_generators, c1_commitments)?;
    c1_statement.prove(
        &mut FcmpProofRandomSource::new(rng),
        &mut transcript,
        c1_witness,
    )?;
    let (c2_statement, c2_witness) = c2_circuit.proving_statement(c2_generators, c2_commitments)?;
    c2_statement.prove(
        &mut FcmpProofRandomSource::new(rng),
        &mut transcript,
        c2_witness,
    )?;
    let circuit_proof = transcript.complete();
    let range_proof = prove_fcmp_range_with_checked_rng_v1(rng, context_hash, new_output_openings)?;

    let expected_wire_len =
        fcmp_plus_plus_wire_size_v1(inputs.len(), root.layers(), new_outputs.len())?;
    let mut proof_wire = Vec::with_capacity(expected_wire_len);
    proof_wire.extend_from_slice(&FCMP_PROOF_WIRE_MAGIC_V1);
    proof_wire.push(u8::try_from(inputs.len()).map_err(|_| FcmpNativeErrorV1::TreeFull)?);
    proof_wire.push(root.layers());
    proof_wire
        .push(u8::try_from(new_outputs.len()).map_err(|_| FcmpNativeErrorV1::ArithmeticInvariant)?);
    proof_wire.push(0);
    if proof_wire.len() != FCMP_PROOF_WIRE_HEADER_BYTES_V1 {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    for prepared in &prepared_inputs {
        proof_wire.extend_from_slice(&prepared.public.output_key_tilde);
        proof_wire.extend_from_slice(&prepared.public.linking_tag_generator_tilde);
        proof_wire.extend_from_slice(&prepared.public.rerandomization_commitment);
        proof_wire.extend_from_slice(&prepared.sal.encode());
    }
    proof_wire.extend(circuit_proof);
    proof_wire.extend_from_slice(&root_blind_commitment);
    proof_wire.extend_from_slice(&root_blind_response);
    proof_wire.extend(range_proof.encode(new_outputs.len())?);
    if proof_wire.len() != expected_wire_len {
        return Err(FcmpNativeErrorV1::ProofLength {
            actual: proof_wire.len(),
            expected: expected_wire_len,
        });
    }
    Ok(FcmpProvedBundleV1 {
        proof_wire,
        public_inputs,
    })
}

fn retry_membership_prover<T>(
    mut prove_once: impl FnMut() -> Result<T, FcmpNativeErrorV1>,
) -> Result<T, FcmpNativeErrorV1> {
    for _ in 0..MAX_MEMBERSHIP_PROVER_RESTARTS_V1 {
        match prove_once() {
            Ok(proof) => return Ok(proof),
            Err(
                FcmpNativeErrorV1::TranscriptChallengeExhausted
                | FcmpNativeErrorV1::DlogChallengeExhausted
                | FcmpNativeErrorV1::DlogWitnessPole
                | FcmpNativeErrorV1::CircuitProverCommitmentIdentity
                | FcmpNativeErrorV1::InnerProductRoundIdentity,
            ) => {
                continue;
            }
            Err(error) => return Err(error),
        }
    }
    Err(FcmpNativeErrorV1::MembershipProverRestartExhausted)
}

struct FcmpProverPreflightV1 {
    new_outputs: Vec<FcmpOutputTupleV1>,
    public_inputs: Vec<FcmpProofInputPublicV1>,
}

fn preflight_fcmp_plus_plus_v1(
    inputs: &[FcmpProverInputV1],
    new_output_openings: &[FcmpOutputCommitmentOpeningV1],
    root: FcmpTreeRootV1,
) -> Result<FcmpProverPreflightV1, FcmpNativeErrorV1> {
    if inputs.is_empty() || inputs.len() > super::FCMP_MAX_INPUTS_NATIVE_V1 {
        return Err(FcmpNativeErrorV1::InputCount {
            actual: inputs.len(),
            max: super::FCMP_MAX_INPUTS_NATIVE_V1,
        });
    }
    if new_output_openings.is_empty()
        || new_output_openings.len() > super::FCMP_MAX_OUTPUTS_NATIVE_V1
    {
        return Err(FcmpNativeErrorV1::OutputCount {
            actual: new_output_openings.len(),
            max: super::FCMP_MAX_OUTPUTS_NATIVE_V1,
        });
    }

    let mut spent_outputs = zeroizing_digest_buffer(inputs.len())?;
    let mut key_images = zeroizing_digest_buffer(inputs.len())?;
    let mut public_inputs = Vec::with_capacity(inputs.len());
    let mut paths = Vec::with_capacity(inputs.len());
    for input in inputs {
        spent_outputs.push(input.output.output_id());
        let public = input.public_input()?;
        key_images.push(public.key_image);
        public_inputs.push(public);
        paths.push(parse_path(input, root)?);
    }
    if ct_has_duplicate_digests(&spent_outputs) {
        return Err(FcmpNativeErrorV1::DuplicateOutput);
    }
    if ct_has_duplicate_digests(&key_images) {
        return Err(FcmpNativeErrorV1::DuplicateKeyImage);
    }
    let shared_root = &paths
        .first()
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
        .root;
    if !all_paths_share_root(&paths, shared_root) {
        return Err(FcmpNativeErrorV1::RootMismatch);
    }

    let mut pseudo_outs = zeroizing_digest_buffer(public_inputs.len())?;
    for public in &public_inputs {
        pseudo_outs.push(public.pseudo_out);
    }
    if ct_has_duplicate_digests(&pseudo_outs) {
        return Err(FcmpNativeErrorV1::DuplicatePseudoOut);
    }
    let new_outputs = new_output_openings
        .iter()
        .map(FcmpOutputCommitmentOpeningV1::output)
        .collect::<Vec<_>>();
    let mut new_output_ids = zeroizing_digest_buffer(new_outputs.len())?;
    for output in &new_outputs {
        new_output_ids.push(output.output_id());
    }
    if ct_has_duplicate_digests(&new_output_ids) {
        return Err(FcmpNativeErrorV1::DuplicateOutput);
    }
    super::verify_fcmp_commitment_balance_v1(&public_inputs, &new_outputs)?;

    Ok(FcmpProverPreflightV1 {
        new_outputs,
        public_inputs,
    })
}

/// Prove a complete first-release FCMP++ statement in native Rust.
///
/// Each input supplies its statement-visible rerandomization witness; all
/// zero-knowledge proof nonces and vector-commitment masks are sampled
/// internally. The supplied paths must resolve exactly to `root`, and all
/// input paths must use the same canonical root branch. A transcript challenge
/// can exhaust the public exceptional-point sampler or hit a hidden dlog
/// denominator pole, exhaust its non-zero transcript challenge sampler, or
/// produce an identity inner-product round point with negligible probability;
/// the prover rebuilds the commitments and transcript with fresh randomness at
/// a fixed bound instead of exposing an honest-abort failure.
pub fn prove_fcmp_plus_plus_v1(
    rng: &mut (impl RngCore + CryptoRng),
    context_hash: [u8; 32],
    inputs: &[FcmpProverInputV1],
    new_output_openings: &[FcmpOutputCommitmentOpeningV1],
    root: FcmpTreeRootV1,
) -> Result<FcmpProvedBundleV1, FcmpNativeErrorV1> {
    let preflight = preflight_fcmp_plus_plus_v1(inputs, new_output_openings, root)?;
    let mut checked_rng = super::health_checked_fcmp_rng_v1(rng)?;
    let bundle = retry_membership_prover(|| {
        prove_fcmp_plus_plus_once_v1(
            &mut checked_rng,
            context_hash,
            inputs,
            new_output_openings,
            root,
        )
    })?;
    if bundle.public_inputs != preflight.public_inputs {
        return Err(FcmpNativeErrorV1::ProverSelfCheckFailed);
    }
    super::verify_fcmp_transaction_v1(
        context_hash,
        bundle.proof_wire(),
        bundle.public_inputs(),
        &preflight.new_outputs,
        root,
    )
    .map_err(|_| FcmpNativeErrorV1::ProverSelfCheckFailed)?;
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use core::cell::Cell;

    use rand_08::{SeedableRng as _, rngs::StdRng};
    use sha2::{Digest as _, Sha256};

    use super::*;
    use crate::privacy_engines::fcmp_plus_plus::{
        FCMP_MAX_INPUTS_NATIVE_V1, FCMP_MAX_OUTPUTS_NATIVE_V1, FCMP_MAX_PROOF_WIRE_BYTES_V1,
        FCMP_NATIVE_KAT_PUBLIC_SHA256_V1, FCMP_NATIVE_KAT_WIRE_SHA256_V1,
        FCMP_OUTPUT_TUPLE_BYTES_V1, FailingRngV1, build_fcmp_frontier_v1,
        field::{encode_field25519_scalar, encode_helioselene_scalar, hash_helios, hash_selene},
        output_from_multiples, verify_fcmp_plus_plus_v1, verify_fcmp_transaction_v1,
    };

    const TEST_AMOUNT: u64 = 5;

    thread_local! {
        static PROVER_COPY_CLEARS: Cell<usize> = const { Cell::new(0) };
    }

    #[derive(Clone, Copy)]
    struct TrackingCopy(u64);

    impl Zeroize for TrackingCopy {
        fn zeroize(&mut self) {
            self.0 = 0;
            PROVER_COPY_CLEARS.with(|calls| calls.set(calls.get() + 1));
        }
    }

    #[test]
    fn prover_copy_owner_clears_transfer_success_and_unwind_slots() {
        PROVER_COPY_CLEARS.with(|calls| calls.set(0));
        let mut source = TrackingCopy(7);
        let owner = ProverSecretCopyValueV1::take(&mut source);
        assert_eq!(source.0, 0);
        assert_eq!(owner.expose_ref().0, 7);
        assert_eq!(PROVER_COPY_CLEARS.with(Cell::get), 1);
        drop(owner);
        assert_eq!(PROVER_COPY_CLEARS.with(Cell::get), 2);

        PROVER_COPY_CLEARS.with(|calls| calls.set(0));
        assert!(
            std::panic::catch_unwind(|| {
                let _owner = ProverSecretCopyValueV1::new(TrackingCopy(11));
                panic!("tracking unwind");
            })
            .is_err()
        );
        assert_eq!(PROVER_COPY_CLEARS.with(Cell::get), 2);
    }

    #[test]
    fn rerandomization_constructor_takes_all_bytes_before_decoding() {
        let source = include_str!("prover.rs");
        let constructor = source
            .split_once("impl FcmpInputRerandomizationV1 {")
            .expect("rerandomization impl")
            .1
            .split_once("#[cfg(test)]\n    fn duplicate_for_test")
            .expect("constructor boundary")
            .0;
        assert_eq!(
            constructor
                .matches("ProverSecretCopyValueV1::take(&mut")
                .count(),
            4
        );
        let last_take = constructor
            .rfind("ProverSecretCopyValueV1::take(&mut")
            .expect("last input take");
        let decode = constructor.find("let decode =").expect("decoder");
        assert!(last_take < decode);
        assert_eq!(
            constructor
                .matches("ProverSecretCopyValueV1::new(decode(")
                .count(),
            4
        );
        let last_scalar = constructor
            .rfind("ProverSecretCopyValueV1::new(decode(")
            .expect("last decoded owner");
        let publish = constructor.find("Ok(Self {").expect("final publication");
        assert!(decode < last_scalar && last_scalar < publish);
        assert!(!constructor.contains("Zeroizing::new(output)"));
        assert!(!constructor.contains("output: decode("));
    }

    #[test]
    fn prover_input_constructor_takes_secret_bytes_before_validation() {
        let source = include_str!("prover.rs");
        let constructor = source
            .split_once("impl FcmpProverInputV1 {")
            .expect("prover input impl")
            .1
            .split_once("#[cfg(test)]\n    fn duplicate_for_test")
            .expect("constructor boundary")
            .0;
        assert_eq!(
            constructor
                .matches("ProverSecretCopyValueV1::take(&mut")
                .count(),
            2
        );
        let last_take = constructor
            .rfind("ProverSecretCopyValueV1::take(&mut")
            .expect("last input take");
        let first_validation = constructor
            .find("validate_edwards_scalar(")
            .expect("first scalar validation");
        assert!(last_take < first_validation);
        assert_eq!(
            constructor.matches("ProverSecretCopyValueV1::new(").count(),
            2
        );
        let last_scalar = constructor
            .rfind("ProverSecretCopyValueV1::new(")
            .expect("last decoded owner");
        let publish = constructor.find("Ok(Self {").expect("final publication");
        assert!(first_validation < last_scalar && last_scalar < publish);
        assert!(!constructor.contains("Zeroizing::new(spend_x)"));
        assert!(!constructor.contains("Zeroizing::new(output_y)"));
        assert!(constructor.contains("spend_x: spend_x_scalar.expose_copy()"));
        assert!(constructor.contains("output_y: output_y_scalar.expose_copy()"));
        assert_eq!(
            constructor
                .matches("decode_secret_helioselene_scalar_v1(encoded)?")
                .count(),
            1
        );
        assert_eq!(
            constructor
                .matches("decode_secret_field25519_scalar_v1(encoded)?")
                .count(),
            1
        );
        assert_eq!(
            constructor
                .matches(
                    "require_preallocated_push(decoded_branch.len(), decoded_branch.capacity())?"
                )
                .count(),
            2
        );
        assert_eq!(
            constructor
                .matches("push_secret_scalar_v1(\n                        &mut decoded_branch,")
                .count(),
            2
        );
        assert!(!constructor.contains("decoded_branch.push(decode_"));
        assert!(!constructor.contains("decode_helioselene_scalar(*encoded)"));
        assert!(!constructor.contains("decode_field25519_scalar(*encoded)"));
    }

    #[test]
    fn commitment_mask_openings_remain_borrowed_until_the_membership_boundary() {
        fn between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
            let start = source.find(start).expect("source start");
            let tail = &source[start..];
            let end = tail.find(end).expect("source end");
            &tail[..end]
        }

        let prover = include_str!("prover.rs");
        assert!(!prover.contains("c1_masks.iter().copied()"));
        assert!(!prover.contains("c2_masks.iter().copied()"));
        assert!(prover.contains(".zip(c1_masks.iter())"));
        assert!(prover.contains(".zip(c2_masks.iter())"));
        assert!(!prover.contains("then(|| c1_masks[root_commitment_index])"));
        assert!(!prover.contains("then(|| c2_masks[root_commitment_index])"));
        assert!(prover.contains("then(|| &c1_masks[root_commitment_index])"));
        assert!(prover.contains("then(|| &c2_masks[root_commitment_index])"));
        let secret_push = between(
            prover,
            "fn push_secret_scalar_v1<F: ProofScalar>",
            "fn ct_slice_contains_by",
        );
        let take = secret_push
            .find("ProverSecretScalarV1::take(&mut value)")
            .expect("incoming scalar take");
        let capacity = secret_push
            .find("require_preallocated_push")
            .expect("capacity check");
        let push = secret_push
            .find("values.push(value.expose_copy())")
            .expect("retained copy");
        let drop = secret_push.find("drop(value)").expect("owner drop");
        assert!(take < capacity && capacity < push && push < drop);
        assert!(!prover.contains("c1_branch_masks.push("));
        assert!(!prover.contains("c2_branch_masks.push("));
        assert!(!prover.contains("c1_masks.push("));
        assert!(!prover.contains("c2_masks.push("));
        assert_eq!(prover.matches("push_secret_scalar_v1(&mut").count(), 6);
        assert!(!prover.contains("-blind.scalar"));
        assert_eq!(prover.matches("blind.scalar.neg_ref()").count(), 2);
        let root_nonce = between(
            prover,
            "let (root_blind_commitment, root_nonce_c1, root_nonce_c2)",
            "let public_inputs =",
        );
        assert_eq!(
            root_nonce
                .matches("let mut nonce = random_proof_scalar")
                .count(),
            2
        );
        assert_eq!(
            root_nonce
                .matches("ProverSecretScalarV1::take(&mut nonce)")
                .count(),
            2
        );
        assert!(root_nonce.contains("root_nonce_commitment_v1::<SeleneSuite>(nonce.expose_ref())"));
        assert!(root_nonce.contains("root_nonce_commitment_v1::<HeliosSuite>(nonce.expose_ref())"));
        assert_eq!(root_nonce.matches("Some(nonce)").count(), 2);
        assert!(!root_nonce.contains("let nonce = random_proof_scalar"));
        assert!(!root_nonce.contains(".h.scale(nonce)"));
        let root_commitment = between(
            prover,
            "fn root_nonce_commitment_v1<S: ProofSuite>",
            "struct PreparedEdBlind",
        );
        assert!(root_commitment.contains("SecretMultiexpBuilder::<S>::new(1)"));
        assert!(root_commitment.contains("terms.push(nonce, &S::generators().h)"));
        assert!(root_commitment.contains("terms.evaluate().map_err(Into::into)"));
        let prepared_point = between(
            prover,
            "fn prepared_secret_point_v1<S: ProofSuite>",
            "struct PreparedEdBlind",
        );
        assert!(prepared_point.contains("SecretMultiexpBuilder::<S>::new(1)"));
        assert!(prepared_point.contains("terms.push(scalar, &S::generators().h)"));
        assert!(prepared_point.contains("let mut point = terms.evaluate()?"));
        assert!(prepared_point.contains("ProverSecretPointV1::take(&mut point)"));
        for (start, end) in [
            ("fn prepare_selene_blind(", "struct PreparedHeliosBlind"),
            ("fn prepare_helios_blind(", "fn commitment_index"),
        ] {
            let blind = between(prover, start, end);
            let transfer = blind
                .find("ProverSecretScalarV1::take(&mut scalar)")
                .expect("scalar transfer");
            let decomposition = blind
                .find("scalar_decomposition(scalar.expose_ref()")
                .expect("borrowed decomposition");
            let point = blind
                .find("prepared_secret_point_v1::<")
                .expect("owned point");
            let divisor = blind
                .find("point.expose_ref()")
                .expect("borrowed divisor point");
            assert!(transfer < decomposition && decomposition < point && point < divisor);
            assert!(!blind.contains(".scale(scalar)"));
            assert!(!blind.contains("let point = generator.scale"));
            assert!(blind.contains("scalar: scalar.expose_copy()"));
            assert!(blind.contains("point: point.expose_copy()"));
            assert!(blind.contains("decomposition: core::mem::take(&mut *decomposition)"));
        }
        let divisor_source = include_str!("divisor.rs");
        let cycle_decomposition = between(
            divisor_source,
            "pub(super) fn scalar_decomposition<F: ProofScalar>",
            "pub(super) fn ed25519_scalar_decomposition",
        );
        assert!(cycle_decomposition.contains("Result<Zeroizing<Vec<u64>>"));
        assert!(cycle_decomposition.contains("let scalar_bytes = Zeroizing::new"));
        assert!(cycle_decomposition.contains("scalar_decomposition_encoded(&scalar_bytes"));
        assert!(cycle_decomposition.contains("SecretDecompositionScalarV1(F::ZERO)"));
        assert!(cycle_decomposition.contains("for coefficient in decomposition.iter()"));
        let ed_decomposition = between(
            divisor_source,
            "pub(super) fn ed25519_scalar_decomposition",
            "fn scalar_decomposition_encoded(",
        );
        assert!(ed_decomposition.contains("for coefficient in decomposition.iter()"));
        let encoded_decomposition = between(
            divisor_source,
            "fn scalar_decomposition_encoded(",
            "pub(super) trait DivisorPoint",
        );
        assert!(encoded_decomposition.contains("scalar: &[u8; 32]"));
        assert!(encoded_decomposition.contains("let mut decomposition = Zeroizing::new("));
        assert!(encoded_decomposition.contains("let mut low_bytes = Zeroizing::new([0_u8; 8])"));
        assert!(encoded_decomposition.contains("let mut sum = Zeroizing::new("));
        let ed_blind = between(prover, "fn prepare_ed_blind(", "struct PreparedSeleneBlind");
        let scalar_owner = ed_blind
            .find("let scalar = Zeroizing::new(if negate")
            .expect("signed scalar owner");
        let decomposition = ed_blind
            .find("ed25519_scalar_decomposition(&scalar)")
            .expect("borrowed Ed decomposition");
        let point_owner = ed_blind
            .find("let point = Zeroizing::new(&generator * &*scalar)")
            .expect("borrowed Ed multiplication");
        let encoded_owner = ed_blind
            .find("let encoded_point = Zeroizing::new")
            .expect("encoded point owner");
        let coordinate_owner = ed_blind
            .find("let coordinates = Zeroizing::new")
            .expect("coordinate owner");
        let divisor = ed_blind
            .find("scalar_mul_divisor")
            .expect("borrowed divisor");
        assert!(
            scalar_owner < decomposition
                && decomposition < point_owner
                && point_owner < encoded_owner
                && encoded_owner < coordinate_owner
                && coordinate_owner < divisor
        );
        assert!(ed_blind.contains("scalar: &Scalar"));
        assert!(ed_blind.contains("decomposition: core::mem::take(&mut *decomposition)"));
        assert!(ed_blind.contains("coordinates: *coordinates"));
        assert!(!ed_blind.contains("generator * scalar"));
        assert!(ed_blind.contains("secret_edwards_to_wei25519_v1(&encoded_point)"));
        assert!(!ed_blind.contains("edwards_to_wei25519(*encoded_point)"));
        let secret_coordinates = between(
            field,
            "pub(super) fn secret_edwards_to_wei25519_v1",
            "pub(super) fn monero_varint",
        );
        assert!(secret_coordinates.contains("bytes: &[u8; 32]"));
        assert!(secret_coordinates.contains("SecretCopyValueV1::new(CompressedEdwardsY(*bytes))"));
        assert!(secret_coordinates.contains("let point = SecretCopyValueV1::new("));
        assert!(secret_coordinates.contains("let mut y_bytes = SecretCopyValueV1::new(*bytes)"));
        assert!(secret_coordinates.contains("secret_decode_field25519_v1(y_bytes.as_ref())"));
        assert!(secret_coordinates.contains("secret_invert_field25519_v1"));
        assert!(secret_coordinates.contains("secret_sqrt_field25519_v1"));
        assert!(secret_coordinates.contains("Ok((wei_x.expose_copy(), wei_y.expose_copy()))"));
        assert_eq!(secret_coordinates.matches("expose_copy()").count(), 2);
        assert!(!secret_coordinates.contains("field25519_is_odd(x.expose_copy())"));
        assert!(!secret_coordinates.contains("y_squared.expose_copy()"));
        assert!(!secret_coordinates.contains("y_plus_one.expose_copy()"));
        assert!(!secret_coordinates.contains("one_minus_y.expose_copy()"));
        let secret_sqrt = between(
            field,
            "fn secret_sqrt_field25519_v1",
            "pub(super) fn secret_edwards_to_wei25519_v1",
        );
        assert!(!secret_sqrt.contains("expose_copy()"));
        assert!(secret_sqrt.contains("first.as_ref().square().eq_ref(value)"));
        assert!(secret_sqrt.contains("first.as_ref()"));
        assert!(secret_sqrt.contains(".mul_ref(&Field25519::new"));
        let secret_invert = between(
            field,
            "fn secret_invert_field25519_v1",
            "fn secret_sqrt_field25519_v1",
        );
        let invert = secret_invert
            .find("value.invert()")
            .expect("field inversion");
        let take = secret_invert
            .find("SecretCopyValueV1::take(&mut inverse)")
            .expect("inverse take");
        let branch = secret_invert
            .find("then_some(inverse)")
            .expect("option branch");
        assert!(invert < take && take < branch);
        let input_blinds = between(
            prover,
            "let mut prepared_inputs = Vec::with_capacity(inputs.len())",
            "let sal = prove_fcmp_sal_with_checked_rng_v1",
        );
        assert!(input_blinds.contains("let rerandomization = &input.rerandomization"));
        for raw in ["let r_o =", "let r_i =", "let r_r_i =", "let r_c ="] {
            assert!(!input_blinds.contains(raw));
        }
        assert_eq!(input_blinds.matches("prepare_ed_blind(").count(), 5);
        assert!(input_blinds.contains("let sal_y = Zeroizing::new("));
        assert!(input_blinds.contains("sal_y.to_bytes()"));
        assert!(input_blinds.contains("rerandomization.linking.to_bytes()"));
        assert!(input_blinds.contains("rerandomization.rerandomization_blind.to_bytes()"));
        let owner = between(
            prover,
            "impl<F: ProofScalar> ProverSecretScalarV1<F>",
            "impl<F: ProofScalar> Drop for ProverSecretScalarV1<F>",
        );
        assert!(owner.contains("fn add_product_assign(&mut self, left: &F, right: &F)"));
        assert!(owner.contains("self.0 += *left * *right;"));
        let response = between(
            prover,
            "let root_blind_response = match root.curve()",
            "let mut c1_circuit =",
        );
        assert_eq!(response.matches(".as_mut()").count(), 2);
        assert_eq!(
            response
                .matches("nonce.add_product_assign(&challenge, mask)")
                .count(),
            2
        );
        assert_eq!(response.matches("nonce.expose_copy().encode()").count(), 2);
        assert!(!response.contains(".as_ref()"));
        assert!(!response.contains("challenge * *root_mask"));

        let field = include_str!("field.rs");
        let mul_ref = between(
            field,
            "pub(super) fn mul_ref(&self, rhs: &Self)",
            "pub(super) const fn pow",
        );
        assert!(mul_ref.contains("Self(self.0 * rhs.0)"));
        assert!(field.contains("pub(super) fn add_ref(&self, rhs: &Self)"));
        assert!(field.contains("pub(super) fn sub_ref(&self, rhs: &Self)"));
        assert!(field.contains("pub(super) fn neg_ref(&self)"));
        assert!(field.contains("pub(super) fn is_odd_ref(&self)"));
        assert!(field.contains("pub(super) fn eq_ref(&self, rhs: &Self)"));
        let coordinates = between(
            field,
            "pub(super) fn secret_coordinates_v1(mut self)",
            "pub(super) fn x(self)",
        );
        let point_guard = coordinates
            .find("BorrowedZeroizingCopySlot(&mut self)")
            .unwrap();
        let invert = coordinates.find("point.as_ref().z.invert()").unwrap();
        let inverse_guard = coordinates
            .find("BorrowedZeroizingCopySlot(&mut inverse)")
            .unwrap();
        let branch = coordinates.find("if !bool::from(is_some)").unwrap();
        assert!(point_guard < invert && invert < inverse_guard && inverse_guard < branch);
        assert!(coordinates.contains("point.as_ref().x.mul_ref(inverse.as_ref())"));
        assert!(coordinates.contains("point.as_ref().y.mul_ref(inverse.as_ref())"));
        assert!(coordinates.contains("drop(inverse);\n                drop(point);"));

        let membership = include_str!("membership.rs");
        assert!(membership.contains("Option<&'c1 Field25519>"));
        assert!(membership.contains("Option<&'c2 HelioseleneField>"));
        assert!(membership.contains("None::<&Field25519>"));
        assert!(membership.contains("None::<&HelioseleneField>"));
        assert!(!membership.contains(".h.scale(*mask)"));
        assert!(!membership.contains("prior_commitment - borrowed_secret_scale_v1"));
        assert!(membership.contains("secret_unblind_helios_coordinates_v1"));
        assert!(membership.contains("secret_unblind_selene_coordinates_v1"));
        assert!(membership.contains(".secret_coordinates_v1()"));
        assert!(!membership.contains("let hash_witness ="));
        assert!(membership.contains("let (hash_x, hash_y, _) = match prior_mask"));

        let helios = between(
            membership,
            "fn secret_unblind_helios_coordinates_v1",
            "fn secret_unblind_selene_coordinates_v1",
        );
        assert!(helios.contains("SecretMultiexpBuilder::<HeliosSuite>::new(2)"));
        assert!(helios.contains("terms.push(&HelioseleneField::ONE, prior_commitment)?"));
        assert!(helios.contains("terms.push(mask, &negative_h)?"));
        assert!(helios.contains(".evaluate()?\n        .secret_coordinates_v1()"));
        let selene = between(
            membership,
            "fn secret_unblind_selene_coordinates_v1",
            "const ED25519_WEI_A",
        );
        assert!(selene.contains("SecretMultiexpBuilder::<SeleneSuite>::new(2)"));
        assert!(selene.contains("terms.push(&Field25519::ONE, prior_commitment)?"));
        assert!(selene.contains("terms.push(mask, &negative_h)?"));
        assert!(selene.contains(".evaluate()?\n        .secret_coordinates_v1()"));

        let c1_branch = between(
            membership,
            "for branch in these_c1_branches",
            "for branch in these_c2_branches",
        );
        assert!(c1_branch.contains("Some(secret_unblind_helios_coordinates_v1("));
        assert!(c1_branch.contains(")?),\n            )?"));
        let c2_branch = between(
            membership,
            "for branch in these_c2_branches",
            "fn verify_membership",
        );
        assert!(c2_branch.contains("Some(secret_unblind_selene_coordinates_v1("));
        assert!(c2_branch.contains(")?),\n            )?"));
    }

    #[derive(Default)]
    struct ZeroRng {
        calls: usize,
    }

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
            self.calls += 1;
            destination.fill(0);
            Ok(())
        }
    }

    impl CryptoRng for ZeroRng {}

    #[derive(Default)]
    struct ZeroThenOneRng {
        calls: usize,
    }

    impl RngCore for ZeroThenOneRng {
        fn next_u32(&mut self) -> u32 {
            0
        }

        fn next_u64(&mut self) -> u64 {
            0
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            self.try_fill_bytes(destination)
                .expect("infallible fixture");
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
            self.calls += 1;
            destination.fill(0);
            if self.calls == 2 {
                destination[0] = 1;
            }
            Ok(())
        }
    }

    impl CryptoRng for ZeroThenOneRng {}

    struct PeriodicRng {
        period: usize,
        cursor: usize,
    }

    impl RngCore for PeriodicRng {
        fn next_u32(&mut self) -> u32 {
            panic!("FCMP++ public prover must reject the periodic prefix")
        }

        fn next_u64(&mut self) -> u64 {
            panic!("FCMP++ public prover must reject the periodic prefix")
        }

        fn fill_bytes(&mut self, _destination: &mut [u8]) {
            panic!("FCMP++ public prover must use fallible entropy")
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
            for byte in destination {
                *byte = ((self.cursor % self.period) as u8)
                    .wrapping_mul(73)
                    .wrapping_add(19);
                self.cursor += 1;
            }
            Ok(())
        }
    }

    impl CryptoRng for PeriodicRng {}

    fn spendable_output(
        x: Scalar,
        y: Scalar,
        linking: Scalar,
        commitment: Scalar,
    ) -> FcmpOutputTupleV1 {
        fcmp_fixture_spendable_output_from_scalars_v1(x, y, linking, TEST_AMOUNT, commitment)
            .expect("valid output")
            .0
    }

    fn output_opening(
        output_key: u64,
        linking: u64,
        amount: u64,
        mask: u64,
    ) -> FcmpOutputCommitmentOpeningV1 {
        fcmp_fixture_output_opening_v1(output_key, linking, amount, mask)
            .expect("valid output opening")
    }

    fn rerandomization(
        output: u64,
        linking: u64,
        blind: u64,
        commitment: u64,
    ) -> FcmpInputRerandomizationV1 {
        fcmp_fixture_rerandomization_v1(output, linking, blind, commitment)
            .expect("canonical test rerandomization")
    }

    fn one_layer_fixture() -> (
        FcmpProverInputV1,
        FcmpOutputCommitmentOpeningV1,
        FcmpTreeRootV1,
    ) {
        let (mut inputs, mut outputs, root) =
            fcmp_release_fixture_v1(false).expect("one-layer release fixture");
        assert_eq!(inputs.len(), 1);
        assert_eq!(outputs.len(), 1);
        (inputs.remove(0), outputs.remove(0), root)
    }

    fn maximum_bound_fixture() -> (
        Vec<FcmpProverInputV1>,
        Vec<FcmpOutputCommitmentOpeningV1>,
        FcmpTreeRootV1,
    ) {
        fcmp_release_fixture_v1(true).expect("maximum-bound release fixture")
    }

    #[test]
    fn prover_witness_debug_is_redacted_and_explicit_zeroize_covers_the_full_path() {
        let (mut input, _new_output, _root) = one_layer_fixture();
        let output_debug = format!("{:?}", input.output);
        let witness_debug = format!("{input:?}");
        assert!(!witness_debug.contains(&output_debug));
        for secret_field in [
            "spend_x",
            "output_y",
            "rerandomization",
            "leaves",
            "additional_branches",
        ] {
            assert!(
                !witness_debug.contains(secret_field),
                "witness debug exposed {secret_field}"
            );
        }

        input.additional_branches = vec![
            AdditionalBranch::ToHelios(vec![HelioseleneField::ONE]),
            AdditionalBranch::ToSelene(vec![Field25519::ONE]),
        ];
        input.zeroize();
        assert_eq!(input.output.encode(), [0; FCMP_OUTPUT_TUPLE_BYTES_V1]);
        assert_eq!(input.spend_x, Scalar::ZERO);
        assert_eq!(input.output_y, Scalar::ZERO);
        assert_eq!(input.rerandomization.output, Scalar::ZERO);
        assert_eq!(input.rerandomization.linking, Scalar::ZERO);
        assert_eq!(input.rerandomization.rerandomization_blind, Scalar::ZERO);
        assert_eq!(input.rerandomization.commitment, Scalar::ZERO);
        assert!(input.leaves.is_empty());
        assert!(input.additional_branches.is_empty());
    }

    #[test]
    fn constant_work_scan_primitives_visit_every_element_and_pair() {
        let values = [11_u8, 22, 33, 44, 55];
        for (target, expected) in [(11, true), (33, true), (55, true), (99, false)] {
            let comparisons = std::cell::Cell::new(0_usize);
            let found = ct_slice_contains_by(&values, &target, |left, right| {
                comparisons.set(comparisons.get() + 1);
                Choice::from(u8::from(left == right))
            });
            assert_eq!(bool::from(found), expected);
            assert_eq!(comparisons.get(), values.len());
        }

        let duplicate_cases = [
            ([7_u8, 7, 2, 3, 4], true),
            ([0_u8, 7, 7, 3, 4], true),
            ([0_u8, 1, 2, 7, 7], true),
            ([0_u8, 1, 2, 3, 4], false),
        ];
        let expected_pairs = values.len() * (values.len() - 1) / 2;
        for (values, expected) in duplicate_cases {
            let comparisons = std::cell::Cell::new(0_usize);
            let duplicate = ct_has_duplicate_by(&values, |left, right| {
                comparisons.set(comparisons.get() + 1);
                Choice::from(u8::from(left == right))
            });
            assert_eq!(bool::from(duplicate), expected);
            assert_eq!(comparisons.get(), expected_pairs);
        }

        for mismatch in [Some(0_usize), Some(2), Some(4), None] {
            let mut candidates = [9_u8; 5];
            if let Some(index) = mismatch {
                candidates[index] = 8;
            }
            let comparisons = std::cell::Cell::new(0_usize);
            let all_match = ct_all_match_by(&candidates, &9, |left, right| {
                comparisons.set(comparisons.get() + 1);
                Choice::from(u8::from(left == right))
            });
            assert_eq!(bool::from(all_match), mismatch.is_none());
            assert_eq!(comparisons.get(), candidates.len());
        }

        let left = [5_u8; 5];
        for mismatch in [Some(0_usize), Some(2), Some(4), None] {
            let mut right = left;
            if let Some(index) = mismatch {
                right[index] = 6;
            }
            let comparisons = std::cell::Cell::new(0_usize);
            let equal = ct_equal_slices_by(&left, &right, |left, right| {
                comparisons.set(comparisons.get() + 1);
                Choice::from(u8::from(left == right))
            });
            assert_eq!(bool::from(equal), mismatch.is_none());
            assert_eq!(comparisons.get(), left.len());
        }
    }

    #[test]
    fn typed_membership_and_duplicate_scans_cover_every_position() {
        let digests = [[1_u8; 32], [2_u8; 32], [3_u8; 32], [4_u8; 32], [5_u8; 32]];
        for (target, expected) in [
            (digests[0], true),
            (digests[2], true),
            (digests[4], true),
            ([9_u8; 32], false),
        ] {
            assert_eq!(ct_digest_slice_contains(&digests, &target), expected);
        }

        for (duplicate_pair, expected) in [
            (Some((0_usize, 1_usize)), true),
            (Some((1, 2)), true),
            (Some((3, 4)), true),
            (None, false),
        ] {
            let mut candidates = digests;
            if let Some((source, destination)) = duplicate_pair {
                candidates[destination] = candidates[source];
            }
            assert_eq!(ct_has_duplicate_digests(&candidates), expected);
        }

        let field_target = Field25519::ONE + Field25519::ONE;
        for target_index in [0, FCMP_LAYER_ONE_LEN_V1 / 2, FCMP_LAYER_ONE_LEN_V1 - 1] {
            let mut padded = vec![Field25519::ONE; FCMP_LAYER_ONE_LEN_V1];
            padded[target_index] = field_target;
            assert!(ct_field25519_slice_contains(&padded, field_target));
        }
        assert!(!ct_field25519_slice_contains(
            &vec![Field25519::ONE; FCMP_LAYER_ONE_LEN_V1],
            field_target,
        ));

        let helioselene_target = HelioseleneField::ONE + HelioseleneField::ONE;
        for target_index in [0, FCMP_LAYER_TWO_LEN_V1 / 2, FCMP_LAYER_TWO_LEN_V1 - 1] {
            let mut padded = vec![HelioseleneField::ONE; FCMP_LAYER_TWO_LEN_V1];
            padded[target_index] = helioselene_target;
            assert!(ct_helioselene_slice_contains(&padded, helioselene_target));
        }
        assert!(!ct_helioselene_slice_contains(
            &vec![HelioseleneField::ONE; FCMP_LAYER_TWO_LEN_V1],
            helioselene_target,
        ));
    }

    #[test]
    fn hidden_leaf_membership_and_duplicates_cover_first_middle_last_and_absent() {
        let xs = [101_u64, 103, 107, 109, 113];
        let ys = [127_u64, 131, 137, 139, 149];
        let leaves: [FcmpOutputTupleV1; 5] = core::array::from_fn(|index| {
            spendable_output(
                Scalar::from(xs[index]),
                Scalar::from(ys[index]),
                Scalar::from(151_u64 + u64::try_from(index).expect("index")),
                Scalar::from(163_u64 + u64::try_from(index).expect("index")),
            )
        });

        for target_index in [0_usize, 2, 4] {
            FcmpProverInputV1::new(
                leaves[target_index],
                Scalar::from(xs[target_index]).to_bytes(),
                Scalar::from(ys[target_index]).to_bytes(),
                rerandomization(173, 179, 181, 191),
                leaves.to_vec(),
                Vec::new(),
            )
            .expect("hidden output at any position is accepted");
        }

        let absent_x = Scalar::from(193_u64);
        let absent_y = Scalar::from(197_u64);
        let absent = spendable_output(
            absent_x,
            absent_y,
            Scalar::from(199_u64),
            Scalar::from(211_u64),
        );
        assert!(matches!(
            FcmpProverInputV1::new(
                absent,
                absent_x.to_bytes(),
                absent_y.to_bytes(),
                rerandomization(223, 227, 229, 233),
                leaves.to_vec(),
                Vec::new(),
            ),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        ));

        for duplicate_pair in [(0_usize, 1_usize), (1, 2), (3, 4)] {
            let mut candidates = leaves;
            candidates[duplicate_pair.1] = candidates[duplicate_pair.0];
            assert!(matches!(
                FcmpProverInputV1::new(
                    leaves[0],
                    Scalar::from(xs[0]).to_bytes(),
                    Scalar::from(ys[0]).to_bytes(),
                    rerandomization(239, 241, 251, 257),
                    candidates.to_vec(),
                    Vec::new(),
                ),
                Err(FcmpNativeErrorV1::DuplicateOutput)
            ));
        }
    }

    #[test]
    fn shared_root_scan_covers_first_middle_last_and_absent_mismatches() {
        let root_coordinates = [Field25519::ONE; 5];
        let shared_root = RootValues::C1(root_coordinates.to_vec());
        for mismatch in [Some(0_usize), Some(2), Some(4), None] {
            let mut paths = Vec::with_capacity(5);
            for path_index in 0..5 {
                let mut coordinates = root_coordinates;
                if mismatch == Some(path_index) {
                    coordinates[2] += Field25519::ONE;
                }
                paths.push(PathValues {
                    c1_non_root: Vec::new(),
                    c2_non_root: Vec::new(),
                    root: RootValues::C1(coordinates.to_vec()),
                });
            }
            assert_eq!(
                all_paths_share_root(&paths, &shared_root),
                mismatch.is_none()
            );
        }

        for mismatch in [Some(0_usize), Some(2), Some(4), None] {
            let mut coordinates = root_coordinates;
            if let Some(index) = mismatch {
                coordinates[index] += Field25519::ONE;
            }
            let candidate = RootValues::C1(coordinates.to_vec());
            assert_eq!(
                bool::from(root_values_ct_eq(&candidate, &shared_root)),
                mismatch.is_none()
            );
        }

        let c2_coordinates = [HelioseleneField::ONE; 5];
        let c2_shared_root = RootValues::C2(c2_coordinates.to_vec());
        for mismatch in [Some(0_usize), Some(2), Some(4), None] {
            let mut coordinates = c2_coordinates;
            if let Some(index) = mismatch {
                coordinates[index] += HelioseleneField::ONE;
            }
            let candidate = RootValues::C2(coordinates.to_vec());
            assert_eq!(
                bool::from(root_values_ct_eq(&candidate, &c2_shared_root)),
                mismatch.is_none()
            );
        }
    }

    #[test]
    fn private_push_guard_forbids_vector_growth() {
        let mut values = Vec::with_capacity(3);
        let allocation_capacity = values.capacity();
        for _ in 0..allocation_capacity {
            require_preallocated_push(values.len(), values.capacity()).expect("preallocated slot");
            values.push(Field25519::ONE);
            assert_eq!(values.capacity(), allocation_capacity);
        }
        assert_eq!(
            require_preallocated_push(values.len(), values.capacity()),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
    }

    #[test]
    fn maximum_compiled_shape_has_canonical_paths_and_exact_resource_bound() {
        let (inputs, outputs, root) = maximum_bound_fixture();
        assert_eq!(inputs.len(), FCMP_MAX_INPUTS_NATIVE_V1);
        assert_eq!(outputs.len(), FCMP_MAX_OUTPUTS_NATIVE_V1);
        assert_eq!(root.layers(), FCMP_MAX_TREE_LAYERS_V1);
        let paths = inputs
            .iter()
            .map(|input| parse_path(input, root))
            .collect::<Result<Vec<_>, _>>()
            .expect("maximum-depth paths resolve");
        let shared_root = &paths.first().expect("at least one path").root;
        assert!(all_paths_share_root(&paths, shared_root));
        assert_eq!(
            ipa_rows(inputs.len(), usize::from(root.layers())).expect("maximum IPA rows"),
            (2_048, 1_024)
        );
        assert_eq!(
            fcmp_plus_plus_wire_size_v1(inputs.len(), root.layers(), outputs.len())
                .expect("maximum wire size"),
            FCMP_MAX_PROOF_WIRE_BYTES_V1
        );
    }

    #[test]
    fn malicious_zero_rng_exhausts_a_fixed_bound_instead_of_hanging() {
        let mut rng = ZeroRng::default();
        assert!(matches!(
            random_proof_scalar::<Field25519>(&mut rng),
            Err(FcmpNativeErrorV1::ProverRandomnessExhausted)
        ));
        assert_eq!(rng.calls, MAX_PROVER_SCALAR_ATTEMPTS_V1);
        assert_eq!(MAX_PROVER_SCALAR_ATTEMPTS_V1, 128);
    }

    #[test]
    fn sampled_scalar_slots_are_owned_before_rejection_or_return() {
        let mut rng = ZeroThenOneRng::default();
        assert_eq!(
            random_proof_scalar::<Field25519>(&mut rng).expect("second candidate is one"),
            Field25519::ONE
        );
        assert_eq!(rng.calls, 2);

        let source = include_str!("prover.rs");
        let random = source
            .split_once("fn random_proof_scalar<F: ProofScalar>")
            .expect("random scalar function")
            .1
            .split_once("struct PreparedEdBlind")
            .expect("random scalar boundary")
            .0;
        let candidate = random
            .find("if let Some(mut scalar)")
            .expect("mutable candidate");
        let transfer = random
            .find("ProverSecretScalarV1::take(&mut scalar)")
            .expect("candidate transfer");
        let zero_check = random
            .find("if !scalar.expose_copy().is_zero()")
            .expect("owned zero check");
        let returned = random
            .find("return Ok(scalar.expose_copy())")
            .expect("intentional output copy");
        assert!(candidate < transfer && transfer < zero_check && zero_check < returned);
        assert!(!random.contains("return Ok(scalar);"));
    }

    #[test]
    fn membership_prover_retries_only_prover_honest_aborts_at_a_fixed_bound() {
        let mut attempts = 0;
        let recovered = retry_membership_prover(|| {
            attempts += 1;
            match attempts {
                1 => Err(FcmpNativeErrorV1::TranscriptChallengeExhausted),
                2 => Err(FcmpNativeErrorV1::DlogChallengeExhausted),
                3 => Err(FcmpNativeErrorV1::DlogWitnessPole),
                4 => Err(FcmpNativeErrorV1::CircuitProverCommitmentIdentity),
                5 => Err(FcmpNativeErrorV1::InnerProductRoundIdentity),
                _ => Ok(17_u8),
            }
        })
        .expect("sixth attempt succeeds");
        assert_eq!(recovered, 17);
        assert_eq!(attempts, 6);

        for retryable in [
            FcmpNativeErrorV1::TranscriptChallengeExhausted,
            FcmpNativeErrorV1::DlogChallengeExhausted,
            FcmpNativeErrorV1::DlogWitnessPole,
            FcmpNativeErrorV1::CircuitProverCommitmentIdentity,
            FcmpNativeErrorV1::InnerProductRoundIdentity,
        ] {
            attempts = 0;
            assert_eq!(
                retry_membership_prover::<()>(|| {
                    attempts += 1;
                    Err(retryable)
                }),
                Err(FcmpNativeErrorV1::MembershipProverRestartExhausted)
            );
            assert_eq!(attempts, MAX_MEMBERSHIP_PROVER_RESTARTS_V1);
        }

        for non_retryable in [
            FcmpNativeErrorV1::ArithmeticInvariant,
            FcmpNativeErrorV1::CircuitEquation,
        ] {
            attempts = 0;
            assert_eq!(
                retry_membership_prover::<()>(|| {
                    attempts += 1;
                    Err(non_retryable)
                }),
                Err(non_retryable)
            );
            assert_eq!(attempts, 1);
        }
    }

    #[test]
    #[ignore = "manual release resource audit; run under `/usr/bin/time -l` for peak RSS"]
    fn maximum_compiled_shape_release_resource_audit() {
        // Reproduce on macOS with:
        // /usr/bin/time -l cargo test -p iroha_core --release --lib
        // privacy_engines::fcmp_plus_plus::prover::tests::maximum_compiled_shape_release_resource_audit
        // -- --ignored --exact --nocapture --test-threads=1
        let setup_started = std::time::Instant::now();
        let (inputs, output_openings, root) = maximum_bound_fixture();
        let setup_ms = setup_started.elapsed().as_millis();
        let context = [0xa5_u8; 32];
        let mut rng = StdRng::seed_from_u64(0xfcff_ff01);

        let prove_started = std::time::Instant::now();
        let bundle = prove_fcmp_plus_plus_v1(&mut rng, context, &inputs, &output_openings, root)
            .expect("maximum-bound native proof");
        let prove_ms = prove_started.elapsed().as_millis();
        assert_eq!(bundle.proof_wire().len(), FCMP_MAX_PROOF_WIRE_BYTES_V1);

        let outputs = output_openings
            .iter()
            .map(FcmpOutputCommitmentOpeningV1::output)
            .collect::<Vec<_>>();
        let verify_started = std::time::Instant::now();
        verify_fcmp_transaction_v1(
            context,
            bundle.proof_wire(),
            bundle.public_inputs(),
            &outputs,
            root,
        )
        .expect("maximum-bound transaction verifies");
        let verify_ms = verify_started.elapsed().as_millis();
        let wire_bytes = bundle.proof_wire().len();
        eprintln!(
            "FCMP_RESOURCE_V1 inputs={} layers={} outputs={} wire_bytes={wire_bytes} \
             setup_ms={setup_ms} prove_ms={prove_ms} verify_ms={verify_ms}",
            inputs.len(),
            root.layers(),
            outputs.len(),
        );
    }

    #[test]
    fn membership_rng_unavailability_fails_without_calling_infallible_rng_methods() {
        assert_eq!(
            random_proof_scalar::<Field25519>(&mut FailingRngV1),
            Err(FcmpNativeErrorV1::RandomnessUnavailable)
        );
    }

    #[test]
    fn public_prover_rejects_unavailable_and_short_period_entropy_before_proving() {
        let context = [0x90_u8; 32];
        let (input, output, root) = one_layer_fixture();
        assert_eq!(
            prove_fcmp_plus_plus_v1(
                &mut FailingRngV1,
                context,
                std::slice::from_ref(&input),
                std::slice::from_ref(&output),
                root,
            ),
            Err(FcmpNativeErrorV1::RandomnessUnavailable)
        );

        for period in [1, 2, 4, 8, 16, 32] {
            let mut rng = PeriodicRng { period, cursor: 0 };
            assert_eq!(
                prove_fcmp_plus_plus_v1(
                    &mut rng,
                    context,
                    std::slice::from_ref(&input),
                    std::slice::from_ref(&output),
                    root,
                ),
                Err(FcmpNativeErrorV1::RandomnessHealthCheckFailed),
                "period-{period} source was not rejected"
            );
        }
    }

    #[test]
    fn deterministic_preflight_errors_take_precedence_over_entropy_failure() {
        let context = [0x90_u8; 32];
        let (input, _, root) = one_layer_fixture();
        assert_eq!(
            prove_fcmp_plus_plus_v1(&mut FailingRngV1, context, &[], &[], root),
            Err(FcmpNativeErrorV1::InputCount {
                actual: 0,
                max: FCMP_MAX_INPUTS_NATIVE_V1,
            })
        );

        let unbalanced_output = output_opening(43, 47, TEST_AMOUNT, 999);
        assert_eq!(
            prove_fcmp_plus_plus_v1(
                &mut FailingRngV1,
                context,
                std::slice::from_ref(&input),
                std::slice::from_ref(&unbalanced_output),
                root,
            ),
            Err(FcmpNativeErrorV1::CommitmentBalanceEquation)
        );
    }

    #[test]
    fn native_one_layer_prover_round_trips_end_to_end() {
        let context = [0x91_u8; 32];
        let (input, new_output, root) = one_layer_fixture();
        let mut rng = StdRng::seed_from_u64(0xfc_0001);
        let bundle = prove_fcmp_plus_plus_v1(
            &mut rng,
            context,
            &[input],
            std::slice::from_ref(&new_output),
            root,
        )
        .expect("native proof");
        let wire_digest: [u8; 32] = Sha256::digest(bundle.proof_wire()).into();
        let mut public_digest = Sha256::new();
        for public in bundle.public_inputs() {
            for field in [
                public.output_key_tilde,
                public.linking_tag_generator_tilde,
                public.rerandomization_commitment,
                public.pseudo_out,
                public.key_image,
            ] {
                public_digest.update(field);
            }
        }
        let public_digest: [u8; 32] = public_digest.finalize().into();
        // Pin the complete Iroha transfer wire and public relation. The
        // membership-only differential fixtures separately exercise the exact
        // upstream Ed25519, Selene, and Helios equations.
        assert_eq!(
            wire_digest, FCMP_NATIVE_KAT_WIRE_SHA256_V1,
            "deterministic IFC1 bytes drifted"
        );
        assert_eq!(
            public_digest, FCMP_NATIVE_KAT_PUBLIC_SHA256_V1,
            "deterministic public relation drifted"
        );
        assert_eq!(
            bundle.proof_wire().len(),
            fcmp_plus_plus_wire_size_v1(1, 1, 1).expect("wire size")
        );
        verify_fcmp_plus_plus_v1(context, bundle.proof_wire(), bundle.public_inputs(), root)
            .expect("native proof verifies");
        verify_fcmp_transaction_v1(
            context,
            bundle.proof_wire(),
            bundle.public_inputs(),
            &[new_output.output()],
            root,
        )
        .expect("complete native transaction verifies");

        let range_size = super::super::fcmp_range_proof_size_v1(1).expect("range proof size");
        let range_start = bundle.proof_wire().len() - range_size;
        for offset in [
            range_start,
            range_start + (range_size / 2),
            bundle.proof_wire().len() - 1,
        ] {
            let mut mutation = bundle.proof_wire().to_vec();
            mutation[offset] ^= 1;
            assert!(
                verify_fcmp_transaction_v1(
                    context,
                    &mutation,
                    bundle.public_inputs(),
                    &[new_output.output()],
                    root,
                )
                .is_err(),
                "complete verifier accepted range-proof mutation at {offset}"
            );
        }
        let mut mismatching_output_count = bundle.proof_wire().to_vec();
        mismatching_output_count[6] = 2;
        assert!(
            verify_fcmp_transaction_v1(
                context,
                &mismatching_output_count,
                bundle.public_inputs(),
                &[new_output.output()],
                root,
            )
            .is_err()
        );

        let mut mutation = bundle.proof_wire().to_vec();
        let middle = mutation.len() / 2;
        mutation[middle] ^= 1;
        assert!(
            verify_fcmp_plus_plus_v1(context, &mutation, bundle.public_inputs(), root).is_err()
        );
        let wrong_root = build_fcmp_frontier_v1(&[spendable_output(
            Scalar::from(41_u64),
            Scalar::from(43_u64),
            Scalar::from(47_u64),
            Scalar::from(53_u64),
        )])
        .expect("other tree")
        .root;
        assert!(
            verify_fcmp_plus_plus_v1(
                context,
                bundle.proof_wire(),
                bundle.public_inputs(),
                wrong_root,
            )
            .is_err()
        );
    }

    #[test]
    fn native_two_layer_prover_exercises_alternating_curve_path() {
        let context = [0x92_u8; 32];
        let x = Scalar::from(101_u64);
        let y = Scalar::from(103_u64);
        let output = spendable_output(x, y, Scalar::from(107_u64), Scalar::from(109_u64));
        let mut outputs = (0..FCMP_LAYER_ONE_LEN_V1)
            .map(|index| {
                let base = 1_000 + (u64::try_from(index).expect("index") * 3);
                output_from_multiples(base, base + 1, base + 2)
            })
            .collect::<Vec<_>>();
        outputs.push(output);
        let frontier = build_fcmp_frontier_v1(&outputs).expect("two-layer tree");
        assert_eq!(frontier.root.layers(), 2);
        assert_eq!(frontier.active_outputs, vec![output]);
        assert_eq!(frontier.levels.len(), 1);

        let mut coordinates = Vec::new();
        let (output_key, linking_tag_generator, commitment) = output.components();
        for point in [output_key, linking_tag_generator, commitment] {
            let (x, y) = edwards_to_wei25519(point).expect("coordinates");
            coordinates.extend([x, y]);
        }
        let active_leaf = hash_selene(&coordinates).expect("active leaf");
        let mut root_branch = duplicate_zeroizing_slice(&frontier.levels[0]);
        root_branch.push(encode_helioselene_scalar(
            active_leaf.x().expect("nonidentity leaf"),
        ));
        let input = FcmpProverInputV1::new(
            output,
            x.to_bytes(),
            y.to_bytes(),
            rerandomization(137, 139, 149, 113),
            vec![output],
            vec![core::mem::take(&mut *root_branch)],
        )
        .expect("two-layer witness");
        let new_output = output_opening(127, 131, TEST_AMOUNT, 109 + 113);
        let mut rng = StdRng::seed_from_u64(0xfc_0002);
        let bundle = prove_fcmp_plus_plus_v1(
            &mut rng,
            context,
            &[input],
            std::slice::from_ref(&new_output),
            frontier.root,
        )
        .expect("native two-layer proof");
        assert_eq!(
            bundle.proof_wire().len(),
            fcmp_plus_plus_wire_size_v1(1, 2, 1).expect("wire size")
        );
        verify_fcmp_plus_plus_v1(
            context,
            bundle.proof_wire(),
            bundle.public_inputs(),
            frontier.root,
        )
        .expect("two-layer native proof verifies");
    }

    #[test]
    fn native_two_input_prover_round_trips_at_the_compiled_bound() {
        let context = [0x93_u8; 32];
        let x_1 = Scalar::from(113_u64);
        let y_1 = Scalar::from(127_u64);
        let x_2 = Scalar::from(131_u64);
        let y_2 = Scalar::from(137_u64);
        let output_1 = spendable_output(x_1, y_1, Scalar::from(139_u64), Scalar::from(149_u64));
        let output_2 = spendable_output(x_2, y_2, Scalar::from(151_u64), Scalar::from(157_u64));
        let mut leaves = Zeroizing::new(vec![output_1, output_2]);
        let root = build_fcmp_frontier_v1(&leaves).expect("tree").root;
        let mut first_leaves = duplicate_zeroizing_slice(&leaves);
        let inputs = [
            FcmpProverInputV1::new(
                output_1,
                x_1.to_bytes(),
                y_1.to_bytes(),
                rerandomization(181, 191, 193, 163),
                core::mem::take(&mut *first_leaves),
                Vec::new(),
            )
            .expect("first witness"),
            FcmpProverInputV1::new(
                output_2,
                x_2.to_bytes(),
                y_2.to_bytes(),
                rerandomization(197, 199, 211, 167),
                core::mem::take(&mut *leaves),
                Vec::new(),
            )
            .expect("second witness"),
        ];
        let new_output = output_opening(173, 179, TEST_AMOUNT * 2, 149 + 163 + 157 + 167);
        let mut rng = StdRng::seed_from_u64(0xfc_0003);
        let bundle = prove_fcmp_plus_plus_v1(
            &mut rng,
            context,
            &inputs,
            std::slice::from_ref(&new_output),
            root,
        )
        .expect("two-input proof");
        assert_eq!(
            bundle.proof_wire().len(),
            fcmp_plus_plus_wire_size_v1(FCMP_MAX_INPUTS_NATIVE_V1, 1, 1).expect("wire size")
        );
        verify_fcmp_plus_plus_v1(context, bundle.proof_wire(), bundle.public_inputs(), root)
            .expect("two-input proof verifies");

        let mut duplicate_key_image = bundle.public_inputs().to_vec();
        duplicate_key_image[1].key_image = duplicate_key_image[0].key_image;
        assert_eq!(
            verify_fcmp_plus_plus_v1(context, bundle.proof_wire(), &duplicate_key_image, root,),
            Err(FcmpNativeErrorV1::DuplicateKeyImage)
        );
        let mut duplicate_pseudo_out = bundle.public_inputs().to_vec();
        duplicate_pseudo_out[1].pseudo_out = duplicate_pseudo_out[0].pseudo_out;
        assert_eq!(
            verify_fcmp_plus_plus_v1(context, bundle.proof_wire(), &duplicate_pseudo_out, root,),
            Err(FcmpNativeErrorV1::DuplicatePseudoOut)
        );
    }

    #[test]
    fn prover_rejects_duplicate_outputs_key_images_and_input_overflow_preflight() {
        let x = Scalar::from(163_u64);
        let first = spendable_output(
            x,
            Scalar::from(167_u64),
            Scalar::from(173_u64),
            Scalar::from(179_u64),
        );
        assert!(matches!(
            FcmpProverInputV1::new(
                first,
                x.to_bytes(),
                Scalar::from(167_u64).to_bytes(),
                rerandomization(211, 223, 227, 181),
                vec![first, first],
                Vec::new(),
            ),
            Err(FcmpNativeErrorV1::DuplicateOutput)
        ));

        let second = spendable_output(
            x,
            Scalar::from(181_u64),
            Scalar::from(173_u64),
            Scalar::from(191_u64),
        );
        let mut leaves = Zeroizing::new(vec![first, second]);
        let root = build_fcmp_frontier_v1(&leaves).expect("tree").root;
        let mut first_leaves = duplicate_zeroizing_slice(&leaves);
        let first_input = FcmpProverInputV1::new(
            first,
            x.to_bytes(),
            Scalar::from(167_u64).to_bytes(),
            rerandomization(229, 233, 239, 193),
            core::mem::take(&mut *first_leaves),
            Vec::new(),
        )
        .expect("first input");
        let second_input = FcmpProverInputV1::new(
            second,
            x.to_bytes(),
            Scalar::from(181_u64).to_bytes(),
            rerandomization(241, 251, 257, 197),
            core::mem::take(&mut *leaves),
            Vec::new(),
        )
        .expect("second input");
        let new_output = output_opening(199, 211, TEST_AMOUNT, 179 + 193);
        let mut rng = StdRng::seed_from_u64(0xfc_0004);
        let duplicate_output_a = first_input.duplicate_for_test();
        let duplicate_output_b = first_input.duplicate_for_test();
        assert_eq!(
            prove_fcmp_plus_plus_v1(
                &mut rng,
                [0x94; 32],
                &[duplicate_output_a, duplicate_output_b],
                std::slice::from_ref(&new_output),
                root,
            ),
            Err(FcmpNativeErrorV1::DuplicateOutput)
        );
        let duplicate_key_image = first_input.duplicate_for_test();
        assert_eq!(
            prove_fcmp_plus_plus_v1(
                &mut rng,
                [0x94; 32],
                &[duplicate_key_image, second_input],
                std::slice::from_ref(&new_output),
                root,
            ),
            Err(FcmpNativeErrorV1::DuplicateKeyImage)
        );
        let overflow_a = first_input.duplicate_for_test();
        let overflow_b = first_input.duplicate_for_test();
        assert!(matches!(
            prove_fcmp_plus_plus_v1(
                &mut rng,
                [0x94; 32],
                &[overflow_a, overflow_b, first_input],
                std::slice::from_ref(&new_output),
                root,
            ),
            Err(FcmpNativeErrorV1::InputCount {
                actual: 3,
                max: FCMP_MAX_INPUTS_NATIVE_V1
            })
        ));
    }

    #[test]
    fn prover_paths_reject_reordered_omitted_and_duplicated_layers() {
        let x = Scalar::from(193_u64);
        let y = Scalar::from(197_u64);
        let output = spendable_output(x, y, Scalar::from(199_u64), Scalar::from(211_u64));
        let completed_capacity = FCMP_LAYER_ONE_LEN_V1 * FCMP_LAYER_TWO_LEN_V1;
        let mut outputs = (0..completed_capacity)
            .map(|index| {
                let base = 20_000 + (u64::try_from(index).expect("index") * 3);
                output_from_multiples(base, base + 1, base + 2)
            })
            .collect::<Vec<_>>();
        outputs.push(output);
        let frontier = build_fcmp_frontier_v1(&outputs).expect("three-layer tree");
        assert_eq!(frontier.root.layers(), 3);
        assert_eq!(frontier.active_outputs, vec![output]);
        assert_eq!(frontier.levels.len(), 2);
        assert!(frontier.levels[0].is_empty());

        let mut coordinates = Vec::new();
        let (output_key, linking_tag_generator, commitment) = output.components();
        for point in [output_key, linking_tag_generator, commitment] {
            let (x, y) = edwards_to_wei25519(point).expect("coordinates");
            coordinates.extend([x, y]);
        }
        let leaf = hash_selene(&coordinates).expect("leaf");
        let leaf_x = leaf.x().expect("nonidentity leaf");
        let first_branch = vec![encode_helioselene_scalar(leaf_x)];
        let active_helios = hash_helios(&[leaf_x]).expect("second layer");
        let mut second_branch = duplicate_zeroizing_slice(&frontier.levels[1]);
        second_branch.push(encode_field25519_scalar(
            active_helios.x().expect("nonidentity second layer"),
        ));

        let valid = FcmpProverInputV1::new(
            output,
            x.to_bytes(),
            y.to_bytes(),
            rerandomization(227, 229, 233, 223),
            vec![output],
            vec![first_branch, core::mem::take(&mut *second_branch)],
        )
        .expect("canonical path");
        parse_path(&valid, frontier.root).expect("canonical path resolves");

        let mut reordered = valid.duplicate_for_test();
        reordered.additional_branches.swap(0, 1);
        assert!(matches!(
            parse_path(&reordered, frontier.root),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        ));

        let mut omitted = valid.duplicate_for_test();
        omitted.additional_branches.remove(0);
        assert!(matches!(
            parse_path(&omitted, frontier.root),
            Err(FcmpNativeErrorV1::ProofHeaderMismatch)
        ));

        let mut duplicated = valid.duplicate_for_test();
        duplicated
            .additional_branches
            .push(valid.additional_branches[0].duplicate_for_test());
        assert!(matches!(
            parse_path(&duplicated, frontier.root),
            Err(FcmpNativeErrorV1::ProofHeaderMismatch)
        ));
    }
}
