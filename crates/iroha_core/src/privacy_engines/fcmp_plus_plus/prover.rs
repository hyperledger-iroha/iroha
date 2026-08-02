//! Canonical native FCMP++ prover.
//!
//! The public API accepts an output-opening and a complete alternating tree
//! path for each input.  Re-randomization, branch blinding, divisor
//! construction, both arithmetic-circuit proofs, SAL, root-blind proof, and
//! IFC1 framing are all produced here; callers cannot inject opaque
//! precomputed circuit witnesses.

use std::collections::BTreeSet;

use curve25519_dalek::{constants::ED25519_BASEPOINT_POINT, edwards::EdwardsPoint, scalar::Scalar};
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
        decode_field25519_scalar, decode_helioselene_scalar, edwards_to_wei25519, hash_helios,
        hash_selene, validate_edwards_scalar,
    },
    membership::{
        TranscriptedInput, constrain_input, ed25519_curve, helios_curve, membership_context,
        native_parameters, selene_curve,
    },
    proof_math::{
        FcmpProofRandomSource, HeliosSuite, ProofPoint, ProofScalar, ProofSuite, ProverTranscript,
        SeleneSuite, helios_bp_generators, random_scalar_from_fcmp_rng, selene_bp_generators,
    },
    range::prove_fcmp_range_with_checked_rng_v1,
    sal::{generator_t, generator_u, generator_v, prove_fcmp_sal_with_checked_rng_v1},
    wire::{FCMP_PROOF_WIRE_MAGIC_V1, fcmp_plus_plus_wire_size_v1, ipa_rows},
};

const MAX_PROVER_SCALAR_ATTEMPTS_V1: usize = 128;
const MAX_MEMBERSHIP_PROVER_RESTARTS_V1: usize = 128;

#[derive(Clone)]
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

/// Caller-selected rerandomization witness for one authoritative FCMP++
/// public input.
///
/// These values must be chosen with a cryptographically secure RNG before the
/// typed statement is hashed. Keeping them explicit makes O~/I~/R/C~/L
/// derivable in one pass and avoids any dependence on replaying an RNG stream.
#[derive(Clone)]
pub struct FcmpInputRerandomizationV1 {
    output: Scalar,
    linking: Scalar,
    rerandomization_blind: Scalar,
    commitment: Scalar,
}

impl FcmpInputRerandomizationV1 {
    /// Decode four canonical non-zero Ed25519 scalars.
    pub fn new(
        output: [u8; 32],
        linking: [u8; 32],
        rerandomization_blind: [u8; 32],
        commitment: [u8; 32],
    ) -> Result<Self, FcmpNativeErrorV1> {
        let output = Zeroizing::new(output);
        let linking = Zeroizing::new(linking);
        let rerandomization_blind = Zeroizing::new(rerandomization_blind);
        let commitment = Zeroizing::new(commitment);
        let decode = |bytes: &[u8; 32]| {
            validate_edwards_scalar(*bytes)?;
            Option::<Scalar>::from(Scalar::from_canonical_bytes(*bytes))
                .filter(|scalar| *scalar != Scalar::ZERO)
                .ok_or(FcmpNativeErrorV1::ScalarEncoding)
        };
        Ok(Self {
            output: decode(&output)?,
            linking: decode(&linking)?,
            rerandomization_blind: decode(&rerandomization_blind)?,
            commitment: decode(&commitment)?,
        })
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
#[derive(Clone)]
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
        spend_x: [u8; 32],
        output_y: [u8; 32],
        rerandomization: FcmpInputRerandomizationV1,
        leaves: Vec<FcmpOutputTupleV1>,
        additional_branches: Vec<Vec<[u8; 32]>>,
    ) -> Result<Self, FcmpNativeErrorV1> {
        let spend_x_bytes = Zeroizing::new(spend_x);
        let output_y_bytes = Zeroizing::new(output_y);
        let mut leaves = Zeroizing::new(leaves);
        let additional_branches = Zeroizing::new(additional_branches);
        validate_edwards_scalar(*spend_x_bytes)?;
        validate_edwards_scalar(*output_y_bytes)?;
        let spend_x = Zeroizing::new(
            Option::<Scalar>::from(Scalar::from_canonical_bytes(*spend_x_bytes))
                .ok_or(FcmpNativeErrorV1::ScalarEncoding)?,
        );
        let output_y = Zeroizing::new(
            Option::<Scalar>::from(Scalar::from_canonical_bytes(*output_y_bytes))
                .ok_or(FcmpNativeErrorV1::ScalarEncoding)?,
        );
        if *spend_x == Scalar::ZERO
            || leaves.is_empty()
            || leaves.len() > FCMP_LAYER_ONE_LEN_V1
            || !leaves.contains(&output)
            || additional_branches.len() + 1 > usize::from(FCMP_MAX_TREE_LAYERS_V1)
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let mut leaf_ids = BTreeSet::new();
        if leaves.iter().any(|leaf| !leaf_ids.insert(leaf.output_id())) {
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
                    decoded_branch.push(decode_helioselene_scalar(*encoded)?);
                }
                decoded.push(AdditionalBranch::ToHelios(core::mem::take(
                    &mut *decoded_branch,
                )));
            } else {
                let mut decoded_branch = Zeroizing::new(Vec::with_capacity(branch.len()));
                for encoded in branch {
                    decoded_branch.push(decode_field25519_scalar(*encoded)?);
                }
                decoded.push(AdditionalBranch::ToSelene(core::mem::take(
                    &mut *decoded_branch,
                )));
            }
        }
        Ok(Self {
            output,
            spend_x: *spend_x,
            output_y: *output_y,
            rerandomization,
            leaves: core::mem::take(&mut *leaves),
            additional_branches: decoded,
        })
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
    let leaves = vec![output_1, output_2];

    let mut leaf_coordinates = Vec::with_capacity(6 * leaves.len());
    for leaf in &leaves {
        let (output_key, linking_tag_generator, amount_commitment) = leaf.components();
        for point in [output_key, linking_tag_generator, amount_commitment] {
            let (x, y) = edwards_to_wei25519(point)?;
            leaf_coordinates.extend([x, y]);
        }
    }
    let mut current_selene = hash_selene(&leaf_coordinates)?;
    let mut current_helios = None;
    let mut branches = Vec::with_capacity(usize::from(FCMP_MAX_TREE_LAYERS_V1.saturating_sub(1)));
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

    let inputs = vec![
        FcmpProverInputV1::new(
            output_1,
            spend_x_1,
            output_y_1,
            fcmp_fixture_rerandomization_v1(439, 443, 449, 163)?,
            leaves.clone(),
            branches.clone(),
        )?,
        FcmpProverInputV1::new(
            output_2,
            spend_x_2,
            output_y_2,
            fcmp_fixture_rerandomization_v1(457, 461, 463, 167)?,
            leaves,
            branches,
        )?,
    ];

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

#[derive(Clone, PartialEq, Eq)]
enum RootValues {
    C1(Vec<Field25519>),
    C2(Vec<HelioseleneField>),
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

#[derive(Clone)]
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
    let mut c1_non_root = Zeroizing::new(Vec::new());
    let mut c2_non_root = Zeroizing::new(Vec::new());

    if input.additional_branches.is_empty() {
        let expected = SelenePoint::decode(root.point(), false)?;
        if root.curve() != FcmpTreeCurveV1::Selene || *current_c1 != Some(expected) {
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
                let mut branch = Zeroizing::new(branch.clone());
                branch.resize(FCMP_LAYER_TWO_LEN_V1, HelioseleneField::ZERO);
                if !branch.contains(&prior_x) {
                    return Err(FcmpNativeErrorV1::ArithmeticInvariant);
                }
                *current_c2 = Some(hash_helios(&branch)?);
                if index == last {
                    root_values = Some(RootValues::C2(core::mem::take(&mut *branch)));
                } else {
                    c2_non_root.push(core::mem::take(&mut *branch));
                }
            }
            AdditionalBranch::ToSelene(branch) => {
                let prior_x = current_c2
                    .take()
                    .and_then(HeliosPoint::x)
                    .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
                let mut branch = Zeroizing::new(branch.clone());
                branch.resize(FCMP_LAYER_ONE_LEN_V1, Field25519::ZERO);
                if !branch.contains(&prior_x) {
                    return Err(FcmpNativeErrorV1::ArithmeticInvariant);
                }
                *current_c1 = Some(hash_selene(&branch)?);
                if index == last {
                    root_values = Some(RootValues::C1(core::mem::take(&mut *branch)));
                } else {
                    c1_non_root.push(core::mem::take(&mut *branch));
                }
            }
        }
    }

    let matches_root = match root.curve() {
        FcmpTreeCurveV1::Selene => *current_c1 == Some(SelenePoint::decode(root.point(), false)?),
        FcmpTreeCurveV1::Helios => *current_c2 == Some(HeliosPoint::decode(root.point(), false)?),
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
        if let Some(scalar) = random_scalar_from_fcmp_rng::<F, _>(rng)?
            && !scalar.is_zero()
        {
            return Ok(scalar);
        }
    }
    Err(FcmpNativeErrorV1::ProverRandomnessExhausted)
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
    scalar: Scalar,
) -> Result<PreparedEdBlind, FcmpNativeErrorV1> {
    let decomposition = ed25519_scalar_decomposition(scalar)?;
    let point = generator * scalar;
    let coordinates = edwards_to_wei25519(point.compress().to_bytes())?;
    let curve = ed25519_curve();
    let divisor = scalar_mul_divisor(curve.a, curve.b, generator, &decomposition, point)?;
    Ok(PreparedEdBlind {
        decomposition,
        divisor,
        coordinates,
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

fn prepare_selene_blind(scalar: Field25519) -> Result<PreparedSeleneBlind, FcmpNativeErrorV1> {
    let decomposition = scalar_decomposition(scalar, CYCLE_DLOG_PARAMETERS.scalar_bits)?;
    let generator = selene_bp_generators().h;
    let point = generator.scale(scalar);
    let curve = selene_curve();
    let divisor = scalar_mul_divisor(curve.a, curve.b, generator, &decomposition, point)?;
    Ok(PreparedSeleneBlind {
        scalar,
        decomposition,
        divisor,
        point,
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
    scalar: HelioseleneField,
) -> Result<PreparedHeliosBlind, FcmpNativeErrorV1> {
    let decomposition = scalar_decomposition(scalar, CYCLE_DLOG_PARAMETERS.scalar_bits)?;
    let generator = helios_bp_generators().h;
    let point = generator.scale(scalar);
    let curve = helios_curve();
    let divisor = scalar_mul_divisor(curve.a, curve.b, generator, &decomposition, point)?;
    Ok(PreparedHeliosBlind {
        scalar,
        decomposition,
        divisor,
        point,
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
    let mut spent_outputs = BTreeSet::new();
    let mut derived_key_images = BTreeSet::new();
    for input in inputs {
        if !spent_outputs.insert(input.output.output_id()) {
            return Err(FcmpNativeErrorV1::DuplicateOutput);
        }
        let linking = decode_edwards_point(input.output.components().1, false)?;
        let key_image = (linking * input.spend_x).compress().to_bytes();
        if !derived_key_images.insert(key_image) {
            return Err(FcmpNativeErrorV1::DuplicateKeyImage);
        }
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
    let mut new_output_ids = BTreeSet::new();
    if new_outputs
        .iter()
        .any(|output| !new_output_ids.insert(output.output_id()))
    {
        return Err(FcmpNativeErrorV1::DuplicateOutput);
    }
    let layers = usize::from(root.layers());
    let paths = inputs
        .iter()
        .map(|input| parse_path(input, root))
        .collect::<Result<Vec<_>, _>>()?;
    let mut shared_root = paths
        .first()
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
        .root
        .clone();
    if paths.iter().any(|path| path.root != shared_root) {
        return Err(FcmpNativeErrorV1::RootMismatch);
    }

    let (c1_rows, c2_rows) = ipa_rows(inputs.len(), layers)?;
    let c1_generators = <SeleneSuite as ProofSuite>::generators().reduce(c1_rows)?;
    let c2_generators = <HeliosSuite as ProofSuite>::generators().reduce(c2_rows)?;
    let mut c1_tape = ProverVectorCommitmentTape::new(c1_rows)?;
    let mut c2_tape = ProverVectorCommitmentTape::new(c2_rows)?;
    let mut transcripted_paths = Vec::with_capacity(paths.len());
    let mut c1_branch_masks = Zeroizing::new(Vec::new());
    let mut c2_branch_masks = Zeroizing::new(Vec::new());
    let mut selene_blinds = Vec::new();
    let mut helios_blinds = Vec::new();

    for path in &paths {
        let mut c1_non_root = Vec::with_capacity(path.c1_non_root.len());
        for branch in &path.c1_non_root {
            c1_non_root.push(c1_tape.append_branch(branch.clone())?);
            let blind = prepare_selene_blind(random_proof_scalar(rng)?)?;
            c1_branch_masks.push(-blind.scalar);
            selene_blinds.push(blind);
        }
        let mut c2_non_root = Vec::with_capacity(path.c2_non_root.len());
        for branch in &path.c2_non_root {
            c2_non_root.push(c2_tape.append_branch(branch.clone())?);
            let blind = prepare_helios_blind(random_proof_scalar(rng)?)?;
            c2_branch_masks.push(-blind.scalar);
            helios_blinds.push(blind);
        }
        transcripted_paths.push(TranscriptedPath {
            c1_non_root,
            c2_non_root,
        });
    }
    let root_variables = match &mut shared_root {
        RootValues::C1(values) => {
            let variables = c1_tape.append_branch(core::mem::take(values))?;
            c1_branch_masks.push(random_proof_scalar(rng)?);
            variables
        }
        RootValues::C2(values) => {
            let variables = c2_tape.append_branch(core::mem::take(values))?;
            c2_branch_masks.push(random_proof_scalar(rng)?);
            variables
        }
    };
    let root_commitment_index = commitment_index(
        *root_variables
            .first()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
    )?;

    let mut prepared_inputs = Vec::with_capacity(inputs.len());
    let mut generated_pseudo_outs = BTreeSet::new();
    for input in inputs {
        let (output_bytes, linking_bytes, commitment_bytes) = input.output.components();
        let public = input.public_input()?;
        if !generated_pseudo_outs.insert(public.pseudo_out) {
            return Err(FcmpNativeErrorV1::DuplicatePseudoOut);
        }
        let r_o = input.rerandomization.output;
        let r_i = input.rerandomization.linking;
        let r_r_i = input.rerandomization.rerandomization_blind;
        let r_c = input.rerandomization.commitment;

        let output_blind = prepare_ed_blind(generator_t(), -r_o)?;
        let input_blind_u = prepare_ed_blind(generator_u(), -r_i)?;
        let input_blind_v = prepare_ed_blind(generator_v(), -r_i)?;
        let input_blind_blind = prepare_ed_blind(generator_t(), r_r_i)?;
        let commitment_blind = prepare_ed_blind(ED25519_BASEPOINT_POINT, -r_c)?;
        let output_coordinates = edwards_to_wei25519(output_bytes)?;
        let linking_coordinates = edwards_to_wei25519(linking_bytes)?;
        let commitment_coordinates = edwards_to_wei25519(commitment_bytes)?;

        let (output_blind_claim, output_variables) = c1_tape.append_claimed_point(
            ED25519_DLOG_PARAMETERS,
            &output_blind.decomposition,
            &output_blind.divisor,
            output_blind.coordinates,
            &[output_coordinates.0, output_coordinates.1],
        )?;
        let (input_blind_u_claim, linking_variables) = c1_tape.append_claimed_point(
            ED25519_DLOG_PARAMETERS,
            &input_blind_u.decomposition,
            &input_blind_u.divisor,
            input_blind_u.coordinates,
            &[linking_coordinates.0, linking_coordinates.1],
        )?;
        let (input_blind_v_divisor, _) = c1_tape.append_divisor(
            ED25519_DLOG_PARAMETERS,
            &input_blind_v.divisor,
            Field25519::ZERO,
        )?;
        let (input_blind_blind_claim, input_blind_v_variables) = c1_tape.append_claimed_point(
            ED25519_DLOG_PARAMETERS,
            &input_blind_blind.decomposition,
            &input_blind_blind.divisor,
            input_blind_blind.coordinates,
            &[input_blind_v.coordinates.0, input_blind_v.coordinates.1],
        )?;
        let (commitment_blind_claim, commitment_variables) = c1_tape.append_claimed_point(
            ED25519_DLOG_PARAMETERS,
            &commitment_blind.decomposition,
            &commitment_blind.divisor,
            commitment_blind.coordinates,
            &[commitment_coordinates.0, commitment_coordinates.1],
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
        let sal_witness = FcmpSalWitnessV1::new(
            input.spend_x.to_bytes(),
            (input.output_y + r_o).to_bytes(),
            r_i.to_bytes(),
            r_r_i.to_bytes(),
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

    // The first proof field opens Helios branch blinds; the second opens
    // Selene branch blinds.
    let mut c1_blind_claims = Vec::with_capacity(helios_blinds.len());
    for blind in &helios_blinds {
        c1_blind_claims.push(
            c1_tape
                .append_claimed_point(
                    CYCLE_DLOG_PARAMETERS,
                    &blind.decomposition,
                    &blind.divisor,
                    blind
                        .point
                        .coordinates()
                        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
                    &[],
                )?
                .0,
        );
    }
    let mut c2_blind_claims = Vec::with_capacity(selene_blinds.len());
    for blind in &selene_blinds {
        c2_blind_claims.push(
            c2_tape
                .append_claimed_point(
                    CYCLE_DLOG_PARAMETERS,
                    &blind.decomposition,
                    &blind.divisor,
                    blind
                        .point
                        .coordinates()
                        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
                    &[],
                )?
                .0,
        );
    }

    let mut c1_masks = c1_branch_masks;
    while c1_masks.len() < c1_tape.commitment_count() {
        c1_masks.push(random_proof_scalar(rng)?);
    }
    let mut c2_masks = c2_branch_masks;
    while c2_masks.len() < c2_tape.commitment_count() {
        c2_masks.push(random_proof_scalar(rng)?);
    }
    let root_mask_c1 =
        (root.curve() == FcmpTreeCurveV1::Selene).then(|| c1_masks[root_commitment_index]);
    let root_mask_c2 =
        (root.curve() == FcmpTreeCurveV1::Helios).then(|| c2_masks[root_commitment_index]);
    let (c1_commitments, c1_openings) = c1_tape
        .commitments_and_openings::<SeleneSuite>(c1_generators, c1_masks.as_slice().to_vec())?;
    let (c2_commitments, c2_openings) = c2_tape
        .commitments_and_openings::<HeliosSuite>(c2_generators, c2_masks.as_slice().to_vec())?;

    let (root_blind_commitment, root_nonce_c1, root_nonce_c2) = match root.curve() {
        FcmpTreeCurveV1::Selene => {
            let nonce = random_proof_scalar::<Field25519>(rng)?;
            (
                selene_bp_generators().h.scale(nonce).encode(),
                Some(nonce),
                None,
            )
        }
        FcmpTreeCurveV1::Helios => {
            let nonce = random_proof_scalar::<HelioseleneField>(rng)?;
            (
                helios_bp_generators().h.scale(nonce).encode(),
                None,
                Some(nonce),
            )
        }
    };
    let public_inputs = prepared_inputs
        .iter()
        .map(|input| input.public)
        .collect::<Vec<_>>();
    let mut pseudo_outs = BTreeSet::new();
    let mut key_images = BTreeSet::new();
    for public in &public_inputs {
        if !pseudo_outs.insert(public.pseudo_out) {
            return Err(FcmpNativeErrorV1::DuplicatePseudoOut);
        }
        if !key_images.insert(public.key_image) {
            return Err(FcmpNativeErrorV1::DuplicateKeyImage);
        }
    }
    super::verify_fcmp_commitment_balance_v1(&public_inputs, &new_outputs)?;

    let context = membership_context(root, &public_inputs, root_blind_commitment)?;
    let mut transcript = ProverTranscript::new(context);
    transcript.write_commitments::<SeleneSuite>(c1_commitments.clone(), Vec::new());
    transcript.write_commitments::<HeliosSuite>(c2_commitments.clone(), Vec::new());
    let root_blind_response = match root.curve() {
        FcmpTreeCurveV1::Selene => {
            let challenge = transcript.challenge::<SeleneSuite>()?;
            (root_nonce_c1.ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
                + (challenge * root_mask_c1.ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?))
            .encode()
        }
        FcmpTreeCurveV1::Helios => {
            let challenge = transcript.challenge::<HeliosSuite>()?;
            (root_nonce_c2.ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
                + (challenge * root_mask_c2.ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?))
            .encode()
        }
    };

    let mut c1_circuit = Circuit::<SeleneSuite>::prove(c1_openings);
    let mut c2_circuit = Circuit::<HeliosSuite>::prove(c2_openings);
    let mut c1_dlog_challenge = None;
    let mut c2_dlog_challenge = None;
    let mut c1_commitment_openings = c1_commitments
        .iter()
        .copied()
        .zip(c1_masks.iter().copied())
        .zip(c2_blind_claims)
        .map(|((commitment, mask), blind)| (commitment, Some(mask), blind));
    let mut c2_commitment_openings = c2_commitments
        .iter()
        .copied()
        .zip(c2_masks.iter().copied())
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

    let mut spent_outputs = BTreeSet::new();
    let mut key_images = BTreeSet::new();
    let mut public_inputs = Vec::with_capacity(inputs.len());
    let paths = inputs
        .iter()
        .map(|input| {
            if !spent_outputs.insert(input.output.output_id()) {
                return Err(FcmpNativeErrorV1::DuplicateOutput);
            }
            let public = input.public_input()?;
            if !key_images.insert(public.key_image) {
                return Err(FcmpNativeErrorV1::DuplicateKeyImage);
            }
            public_inputs.push(public);
            parse_path(input, root)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shared_root = paths
        .first()
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
        .root
        .clone();
    if paths.iter().any(|path| path.root != shared_root) {
        return Err(FcmpNativeErrorV1::RootMismatch);
    }

    let mut pseudo_outs = BTreeSet::new();
    if public_inputs
        .iter()
        .any(|public| !pseudo_outs.insert(public.pseudo_out))
    {
        return Err(FcmpNativeErrorV1::DuplicatePseudoOut);
    }
    let new_outputs = new_output_openings
        .iter()
        .map(FcmpOutputCommitmentOpeningV1::output)
        .collect::<Vec<_>>();
    let mut new_output_ids = BTreeSet::new();
    if new_outputs
        .iter()
        .any(|output| !new_output_ids.insert(output.output_id()))
    {
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
        assert!(paths.windows(2).all(|pair| pair[0].root == pair[1].root));
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
        let mut root_branch = frontier.levels[0].clone();
        root_branch.push(encode_helioselene_scalar(
            active_leaf.x().expect("nonidentity leaf"),
        ));
        let input = FcmpProverInputV1::new(
            output,
            x.to_bytes(),
            y.to_bytes(),
            rerandomization(137, 139, 149, 113),
            vec![output],
            vec![root_branch],
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
        let leaves = vec![output_1, output_2];
        let root = build_fcmp_frontier_v1(&leaves).expect("tree").root;
        let inputs = [
            FcmpProverInputV1::new(
                output_1,
                x_1.to_bytes(),
                y_1.to_bytes(),
                rerandomization(181, 191, 193, 163),
                leaves.clone(),
                Vec::new(),
            )
            .expect("first witness"),
            FcmpProverInputV1::new(
                output_2,
                x_2.to_bytes(),
                y_2.to_bytes(),
                rerandomization(197, 199, 211, 167),
                leaves,
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
        let leaves = vec![first, second];
        let root = build_fcmp_frontier_v1(&leaves).expect("tree").root;
        let first_input = FcmpProverInputV1::new(
            first,
            x.to_bytes(),
            Scalar::from(167_u64).to_bytes(),
            rerandomization(229, 233, 239, 193),
            leaves.clone(),
            Vec::new(),
        )
        .expect("first input");
        let second_input = FcmpProverInputV1::new(
            second,
            x.to_bytes(),
            Scalar::from(181_u64).to_bytes(),
            rerandomization(241, 251, 257, 197),
            leaves,
            Vec::new(),
        )
        .expect("second input");
        let new_output = output_opening(199, 211, TEST_AMOUNT, 179 + 193);
        let mut rng = StdRng::seed_from_u64(0xfc_0004);
        assert_eq!(
            prove_fcmp_plus_plus_v1(
                &mut rng,
                [0x94; 32],
                &[first_input.clone(), first_input.clone()],
                std::slice::from_ref(&new_output),
                root,
            ),
            Err(FcmpNativeErrorV1::DuplicateOutput)
        );
        assert_eq!(
            prove_fcmp_plus_plus_v1(
                &mut rng,
                [0x94; 32],
                &[first_input.clone(), second_input],
                std::slice::from_ref(&new_output),
                root,
            ),
            Err(FcmpNativeErrorV1::DuplicateKeyImage)
        );
        assert!(matches!(
            prove_fcmp_plus_plus_v1(
                &mut rng,
                [0x94; 32],
                &[first_input.clone(), first_input.clone(), first_input],
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
        let mut second_branch = frontier.levels[1].clone();
        second_branch.push(encode_field25519_scalar(
            active_helios.x().expect("nonidentity second layer"),
        ));

        let valid = FcmpProverInputV1::new(
            output,
            x.to_bytes(),
            y.to_bytes(),
            rerandomization(227, 229, 233, 223),
            vec![output],
            vec![first_branch, second_branch],
        )
        .expect("canonical path");
        parse_path(&valid, frontier.root).expect("canonical path resolves");

        let mut reordered = valid.clone();
        reordered.additional_branches.swap(0, 1);
        assert!(matches!(
            parse_path(&reordered, frontier.root),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        ));

        let mut omitted = valid.clone();
        omitted.additional_branches.remove(0);
        assert!(matches!(
            parse_path(&omitted, frontier.root),
            Err(FcmpNativeErrorV1::ProofHeaderMismatch)
        ));

        let mut duplicated = valid.clone();
        duplicated
            .additional_branches
            .push(valid.additional_branches[0].clone());
        assert!(matches!(
            parse_path(&duplicated, frontier.root),
            Err(FcmpNativeErrorV1::ProofHeaderMismatch)
        ));
    }
}
