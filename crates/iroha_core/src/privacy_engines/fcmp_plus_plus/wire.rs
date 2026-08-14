//! Canonical bounded wire framing for native FCMP++ proofs.
//!
//! The upstream Monero proof wire omits the input count, tree depth,
//! pseudo-outs, and key images.  Iroha's sole first-release `IFC1` envelope
//! commits to the three structural counts in a fixed header, while the
//! authoritative statement supplies canonical O~/I~/R/C~/L values and the
//! decoder rejects disagreement with the duplicated proof O~/I~/R.
//! The contained proof bytes retain the upstream order:
//! `O~ || I~ || R || SAL` per input, followed by the generalized
//! Bulletproofs FCMP and its root-blind proof of knowledge, then the sole
//! ordered aggregate output Bulletproofs+ range proof.
use super::{
    FCMP_MAX_OUTPUTS_NATIVE_V1, FCMP_POINT_BYTES_V1, FcmpNativeErrorV1, FcmpRangeProofV1,
    FcmpTreeCurveV1, FcmpTreeRootV1, fcmp_range_proof_size_v1,
    field::{
        HeliosPoint, SelenePoint, decode_field25519_scalar, decode_helioselene_scalar,
        validate_edwards_scalar,
    },
    validate_fcmp_edwards_point_v1, validate_layer_count,
};
use std::collections::BTreeSet;
/// Sole first-release FCMP++ proof-envelope magic and version.
pub const FCMP_PROOF_WIRE_MAGIC_V1: [u8; 4] = *b"IFC1";
/// Maximum FCMP++ input count accepted by the native first-release parser.
pub const FCMP_MAX_INPUTS_NATIVE_V1: usize = 2;
/// Exact `IFC1` header width.
pub const FCMP_PROOF_WIRE_HEADER_BYTES_V1: usize = 8;
const SAL_POINT_COUNT_V1: usize = 6;
const SAL_SCALAR_COUNT_V1: usize = 6;
const SAL_BYTES_V1: usize = (SAL_POINT_COUNT_V1 + SAL_SCALAR_COUNT_V1) * FCMP_POINT_BYTES_V1;
/// Exact upstream partial-input plus SAL width per input.
pub const FCMP_PROOF_INPUT_BYTES_V1: usize = (3 * FCMP_POINT_BYTES_V1) + SAL_BYTES_V1;
/// Smallest canonical first-release `IFC1` proof envelope.
pub const FCMP_MIN_PROOF_WIRE_BYTES_V1: usize = 4_008;
/// Largest canonical first-release `IFC1` proof envelope.
pub const FCMP_MAX_PROOF_WIRE_BYTES_V1: usize = 12_520;
const ROOT_BLIND_POK_BYTES_V1: usize = 64;
const COMMITMENT_WORD_LEN_V1: usize = 128;
const C1_LEAVES_ROWS_PER_INPUT_V1: usize = 97;
const C1_BRANCH_ROWS_PER_INPUT_V1: usize = 52;
const C2_ROWS_PER_INPUT_PER_LAYER_V1: usize = 32;
const C1_TARGET_ROWS_V1: usize = 256;
const C2_TARGET_ROWS_V1: usize = 128;
/// Authoritative public FCMP++ relation for one input.
///
/// Upstream serializes O~/I~/R in the proof and supplies C~/L externally.
/// Iroha commits all five values in the typed transaction statement; the
/// decoder requires the three duplicated proof values to match exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FcmpProofInputPublicV1 {
    /// Canonical non-identity re-randomized output key `O~`.
    pub output_key_tilde: [u8; 32],
    /// Canonical non-identity re-randomized linking-tag generator `I~`.
    pub linking_tag_generator_tilde: [u8; 32],
    /// Canonical non-identity re-randomization commitment `R`.
    pub rerandomization_commitment: [u8; 32],
    /// Canonical non-identity pseudo-out `C~`.
    pub pseudo_out: [u8; 32],
    /// Canonical non-identity key image/link tag `L`.
    pub key_image: [u8; 32],
}
impl FcmpProofInputPublicV1 {
    /// Validate one complete O~/I~/R/C~/L public relation.
    pub fn new(
        output_key_tilde: [u8; 32],
        linking_tag_generator_tilde: [u8; 32],
        rerandomization_commitment: [u8; 32],
        pseudo_out: [u8; 32],
        key_image: [u8; 32],
    ) -> Result<Self, FcmpNativeErrorV1> {
        validate_fcmp_edwards_point_v1(output_key_tilde)?;
        validate_fcmp_edwards_point_v1(linking_tag_generator_tilde)?;
        validate_fcmp_edwards_point_v1(rerandomization_commitment)?;
        validate_fcmp_edwards_point_v1(pseudo_out)?;
        validate_fcmp_edwards_point_v1(key_image)?;
        Ok(Self {
            output_key_tilde,
            linking_tag_generator_tilde,
            rerandomization_commitment,
            pseudo_out,
            key_image,
        })
    }
}
/// Structurally decoded proof components for one FCMP++ input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedFcmpProofInputV1 {
    /// Re-randomized output key `O~`.
    pub output_key_tilde: [u8; 32],
    /// Re-randomized linking-tag generator `I~`.
    pub linking_tag_generator_tilde: [u8; 32],
    /// Commitment `R` to the `I~` re-randomization.
    pub rerandomization_commitment: [u8; 32],
    /// Six canonical non-identity SAL commitment points.
    pub sal_points: [[u8; 32]; 6],
    /// Six canonical Ed25519 scalar responses.
    pub sal_scalars: [[u8; 32]; 6],
}
/// Strictly framed but not yet cryptographically verified FCMP++ proof.
///
/// Construction of this type proves only canonical structure and size.  It is
/// intentionally not a verification token and cannot be converted into a
/// ledger effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedFcmpPlusPlusWireV1 {
    /// Header input count.
    pub input_count: u8,
    /// Header tree layer count.
    pub layers: u8,
    /// Header newly-created output count.
    pub output_count: u8,
    /// Per-input re-randomized tuple and SAL encodings.
    pub inputs: Vec<ParsedFcmpProofInputV1>,
    /// Opaque generalized Bulletproof transcript bytes.
    ///
    /// These bytes require the full dual-curve arithmetic-circuit verifier
    /// before any proof can be accepted.
    pub circuit_proof: Vec<u8>,
    /// Root-blind proof commitment on the root curve.
    pub root_blind_commitment: [u8; 32],
    /// Root-blind proof response in the root curve's scalar field.
    pub root_blind_response: [u8; 32],
    /// Canonical aggregate strict-positive `u64` range proof for the ordered
    /// newly-created output commitments.
    pub range_proof: FcmpRangeProofV1,
}
#[derive(Clone, Copy, Debug)]
struct TapeSizer {
    words_per_commitment: usize,
    offset: usize,
    commitments: usize,
}
impl TapeSizer {
    fn new(commitment_len: usize) -> Result<Self, FcmpNativeErrorV1> {
        if commitment_len == 0 || commitment_len % COMMITMENT_WORD_LEN_V1 != 0 {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        Ok(Self {
            words_per_commitment: commitment_len / COMMITMENT_WORD_LEN_V1,
            offset: 0,
            commitments: 0,
        })
    }
    fn append_words(&mut self, words: usize) {
        for _ in 0..words {
            if self.offset == 0 {
                self.commitments += 1;
            }
            self.offset += 1;
            if self.offset == self.words_per_commitment {
                self.offset = 0;
            }
        }
    }
    fn append_branch(&mut self) -> Result<(), FcmpNativeErrorV1> {
        if self.offset != 0 {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        self.commitments += 1;
        Ok(())
    }
    fn append_claimed_point(&mut self) {
        self.append_words(4);
    }
    fn append_divisor(&mut self) {
        self.append_words(2);
    }
}
fn next_power_of_two_at_least(value: usize, minimum: usize) -> Result<usize, FcmpNativeErrorV1> {
    value
        .max(1)
        .checked_next_power_of_two()
        .map(|value| value.max(minimum))
        .ok_or(FcmpNativeErrorV1::TreeFull)
}
pub(super) fn ipa_rows(inputs: usize, layers: usize) -> Result<(usize, usize), FcmpNativeErrorV1> {
    let non_leaf_c1_branches = layers.saturating_sub(1) / 2;
    let c1_rows = inputs
        .checked_mul(
            C1_LEAVES_ROWS_PER_INPUT_V1
                .checked_add(
                    non_leaf_c1_branches
                        .checked_mul(C1_BRANCH_ROWS_PER_INPUT_V1)
                        .ok_or(FcmpNativeErrorV1::TreeFull)?,
                )
                .ok_or(FcmpNativeErrorV1::TreeFull)?,
        )
        .ok_or(FcmpNativeErrorV1::TreeFull)?;
    let c2_rows = inputs
        .checked_mul(
            (layers / 2)
                .checked_mul(C2_ROWS_PER_INPUT_PER_LAYER_V1)
                .ok_or(FcmpNativeErrorV1::TreeFull)?
                .max(1),
        )
        .ok_or(FcmpNativeErrorV1::TreeFull)?;
    Ok((
        next_power_of_two_at_least(c1_rows, C1_TARGET_ROWS_V1)?,
        next_power_of_two_at_least(c2_rows, C2_TARGET_ROWS_V1)?,
    ))
}
fn log2_power_of_two(value: usize) -> Result<usize, FcmpNativeErrorV1> {
    if !value.is_power_of_two() {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    Ok(value.trailing_zeros() as usize)
}
fn fcmp_membership_proof_size_v1(inputs: usize, layers: usize) -> Result<usize, FcmpNativeErrorV1> {
    let (c1_rows, c2_rows) = ipa_rows(inputs, layers)?;
    let c1_ipa_elements = 2_usize
        .checked_mul(log2_power_of_two(c1_rows)?)
        .ok_or(FcmpNativeErrorV1::TreeFull)?;
    let c2_ipa_elements = 2_usize
        .checked_mul(log2_power_of_two(c2_rows)?)
        .ok_or(FcmpNativeErrorV1::TreeFull)?;
    let mut proof_elements = 16_usize
        .checked_add(c1_ipa_elements)
        .and_then(|value| value.checked_add(c2_ipa_elements))
        .ok_or(FcmpNativeErrorV1::TreeFull)?;
    let mut c1 = TapeSizer::new(c1_rows)?;
    let mut c2 = TapeSizer::new(c2_rows)?;
    let mut c1_non_root_branches = 0_usize;
    let mut c2_non_root_branches = 0_usize;
    for _ in 0..inputs {
        for layer in 0..layers.saturating_sub(1) {
            if layer % 2 == 0 {
                c1.append_branch()?;
                c1_non_root_branches += 1;
            } else {
                c2.append_branch()?;
                c2_non_root_branches += 1;
            }
        }
    }
    if layers % 2 == 1 {
        c1.append_branch()?;
    } else {
        c2.append_branch()?;
    }
    for _ in 0..inputs {
        c1.append_claimed_point();
        c1.append_claimed_point();
        c1.append_divisor();
        c1.append_claimed_point();
        c1.append_claimed_point();
    }
    let additional_c1_points = if c1_non_root_branches == 0 {
        0
    } else {
        c1_non_root_branches
            .checked_sub(inputs)
            .and_then(|value| value.checked_add(inputs * (layers % 2)))
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
    };
    for _ in 0..additional_c1_points {
        c1.append_claimed_point();
    }
    let additional_c2_points = c2_non_root_branches
        .checked_add(inputs * usize::from(layers % 2 == 0))
        .ok_or(FcmpNativeErrorV1::TreeFull)?;
    for _ in 0..additional_c2_points {
        c2.append_claimed_point();
    }
    for commitments in [c1.commitments, c2.commitments] {
        let ni = 2_usize
            .checked_add(2 * (commitments / 2))
            .ok_or(FcmpNativeErrorV1::TreeFull)?;
        let l_r_poly_len = ni.checked_add(2).ok_or(FcmpNativeErrorV1::TreeFull)?;
        let t_commitments = 2_usize
            .checked_mul(l_r_poly_len)
            .and_then(|value| value.checked_sub(2))
            .ok_or(FcmpNativeErrorV1::TreeFull)?;
        proof_elements = proof_elements
            .checked_add(commitments)
            .and_then(|value| value.checked_add(t_commitments))
            .ok_or(FcmpNativeErrorV1::TreeFull)?;
    }
    proof_elements
        .checked_mul(32)
        .and_then(|value| value.checked_add(ROOT_BLIND_POK_BYTES_V1))
        .ok_or(FcmpNativeErrorV1::TreeFull)
}
/// Return the unique `IFC1` wire size for input, tree-depth, and output counts.
pub fn fcmp_plus_plus_wire_size_v1(
    inputs: usize,
    layers: u8,
    outputs: usize,
) -> Result<usize, FcmpNativeErrorV1> {
    let max_inputs = FCMP_MAX_INPUTS_NATIVE_V1;
    if inputs == 0 || inputs > max_inputs {
        return Err(FcmpNativeErrorV1::InputCount {
            actual: inputs,
            max: max_inputs,
        });
    }
    validate_layer_count(layers)?;
    if outputs == 0 || outputs > FCMP_MAX_OUTPUTS_NATIVE_V1 {
        return Err(FcmpNativeErrorV1::OutputCount {
            actual: outputs,
            max: FCMP_MAX_OUTPUTS_NATIVE_V1,
        });
    }
    let input_bytes = inputs
        .checked_mul(FCMP_PROOF_INPUT_BYTES_V1)
        .ok_or(FcmpNativeErrorV1::TreeFull)?;
    let membership_bytes = fcmp_membership_proof_size_v1(inputs, usize::from(layers))?;
    let range_bytes = fcmp_range_proof_size_v1(outputs)?;
    FCMP_PROOF_WIRE_HEADER_BYTES_V1
        .checked_add(input_bytes)
        .and_then(|value| value.checked_add(membership_bytes))
        .and_then(|value| value.checked_add(range_bytes))
        .ok_or(FcmpNativeErrorV1::TreeFull)
}
fn take_array<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], FcmpNativeErrorV1> {
    let end = cursor.checked_add(N).ok_or(FcmpNativeErrorV1::TreeFull)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(FcmpNativeErrorV1::ProofLength {
            actual: bytes.len(),
            expected: end,
        })?;
    let mut array = [0_u8; N];
    array.copy_from_slice(value);
    *cursor = end;
    Ok(array)
}
/// Strictly decode the self-describing FCMP++ proof envelope.
///
/// This function validates canonical points, canonical scalars, exact length,
/// public-input cardinality, tree parity, and root-blind encodings.  It does
/// not verify the SAL or generalized Bulletproof equations.
pub fn decode_fcmp_plus_plus_wire_v1(
    bytes: &[u8],
    public_inputs: &[FcmpProofInputPublicV1],
    root: FcmpTreeRootV1,
) -> Result<ParsedFcmpPlusPlusWireV1, FcmpNativeErrorV1> {
    if bytes.len() < FCMP_PROOF_WIRE_HEADER_BYTES_V1 {
        return Err(FcmpNativeErrorV1::ProofLength {
            actual: bytes.len(),
            expected: FCMP_PROOF_WIRE_HEADER_BYTES_V1,
        });
    }
    if bytes[..4] != FCMP_PROOF_WIRE_MAGIC_V1 {
        return Err(FcmpNativeErrorV1::ProofWireMagic);
    }
    let input_count_u8 = bytes[4];
    let input_count = usize::from(input_count_u8);
    let layers = bytes[5];
    let output_count_u8 = bytes[6];
    let output_count = usize::from(output_count_u8);
    if bytes[7] != 0 {
        return Err(FcmpNativeErrorV1::ProofWireReserved);
    }
    let expected_len = fcmp_plus_plus_wire_size_v1(input_count, layers, output_count)?;
    if bytes.len() != expected_len {
        return Err(FcmpNativeErrorV1::ProofLength {
            actual: bytes.len(),
            expected: expected_len,
        });
    }
    if input_count != public_inputs.len() || layers != root.layers() {
        return Err(FcmpNativeErrorV1::ProofHeaderMismatch);
    }
    let mut pseudo_outs = BTreeSet::new();
    let mut key_images = BTreeSet::new();
    for input in public_inputs {
        let validated = FcmpProofInputPublicV1::new(
            input.output_key_tilde,
            input.linking_tag_generator_tilde,
            input.rerandomization_commitment,
            input.pseudo_out,
            input.key_image,
        )?;
        if !pseudo_outs.insert(validated.pseudo_out) {
            return Err(FcmpNativeErrorV1::DuplicatePseudoOut);
        }
        if !key_images.insert(validated.key_image) {
            return Err(FcmpNativeErrorV1::DuplicateKeyImage);
        }
    }
    let mut cursor = FCMP_PROOF_WIRE_HEADER_BYTES_V1;
    let mut inputs = Vec::with_capacity(input_count);
    for (input_index, public_input) in public_inputs.iter().enumerate() {
        let output_key_tilde = take_array(bytes, &mut cursor)?;
        let linking_tag_generator_tilde = take_array(bytes, &mut cursor)?;
        let rerandomization_commitment = take_array(bytes, &mut cursor)?;
        for point in [
            output_key_tilde,
            linking_tag_generator_tilde,
            rerandomization_commitment,
        ] {
            validate_fcmp_edwards_point_v1(point)?;
        }
        if output_key_tilde != public_input.output_key_tilde
            || linking_tag_generator_tilde != public_input.linking_tag_generator_tilde
            || rerandomization_commitment != public_input.rerandomization_commitment
        {
            return Err(FcmpNativeErrorV1::ProofPublicInputMismatch { index: input_index });
        }
        let mut sal_points = [[0; 32]; SAL_POINT_COUNT_V1];
        for point in &mut sal_points {
            *point = take_array(bytes, &mut cursor)?;
            // Identity commitments have negligible honest probability and are
            // excluded from the sole canonical Iroha proof wire.
            validate_fcmp_edwards_point_v1(*point)?;
        }
        let mut sal_scalars = [[0; 32]; SAL_SCALAR_COUNT_V1];
        for scalar in &mut sal_scalars {
            *scalar = take_array(bytes, &mut cursor)?;
            validate_edwards_scalar(*scalar)?;
        }
        inputs.push(ParsedFcmpProofInputV1 {
            output_key_tilde,
            linking_tag_generator_tilde,
            rerandomization_commitment,
            sal_points,
            sal_scalars,
        });
    }
    let membership_size = fcmp_membership_proof_size_v1(input_count, usize::from(layers))?;
    let circuit_size = membership_size
        .checked_sub(ROOT_BLIND_POK_BYTES_V1)
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    let circuit_end = cursor
        .checked_add(circuit_size)
        .ok_or(FcmpNativeErrorV1::TreeFull)?;
    let circuit_proof = bytes
        .get(cursor..circuit_end)
        .ok_or(FcmpNativeErrorV1::ProofLength {
            actual: bytes.len(),
            expected: circuit_end,
        })?
        .to_vec();
    if circuit_proof.iter().all(|byte| *byte == 0) {
        return Err(FcmpNativeErrorV1::EmptyCircuitProof);
    }
    cursor = circuit_end;
    let root_blind_commitment = take_array(bytes, &mut cursor)?;
    let root_blind_response = take_array(bytes, &mut cursor)?;
    match root.curve() {
        FcmpTreeCurveV1::Selene => {
            SelenePoint::decode(root_blind_commitment, false)?;
            decode_field25519_scalar(root_blind_response)?;
        }
        FcmpTreeCurveV1::Helios => {
            HeliosPoint::decode(root_blind_commitment, false)?;
            decode_helioselene_scalar(root_blind_response)?;
        }
    }
    let range_size = fcmp_range_proof_size_v1(output_count)?;
    let range_end = cursor
        .checked_add(range_size)
        .ok_or(FcmpNativeErrorV1::TreeFull)?;
    let range_proof = FcmpRangeProofV1::decode(
        bytes
            .get(cursor..range_end)
            .ok_or(FcmpNativeErrorV1::ProofLength {
                actual: bytes.len(),
                expected: range_end,
            })?,
        output_count,
    )?;
    cursor = range_end;
    if cursor != bytes.len() {
        return Err(FcmpNativeErrorV1::ProofLength {
            actual: bytes.len(),
            expected: cursor,
        });
    }
    Ok(ParsedFcmpPlusPlusWireV1 {
        input_count: input_count_u8,
        layers,
        output_count: output_count_u8,
        inputs,
        circuit_proof,
        root_blind_commitment,
        root_blind_response,
        range_proof,
    })
}
/// Decode a pinned upstream membership-only IFC1 fixture for differential
/// tests. This path is absent from production builds: it supplies a
/// structurally canonical dummy range suffix solely so the production parser
/// can exercise the historical membership component against upstream bytes.
#[cfg(test)]
pub(super) fn decode_fcmp_membership_fixture_v1(
    bytes: &[u8],
    public_inputs: &[FcmpProofInputPublicV1],
    root: FcmpTreeRootV1,
) -> Result<ParsedFcmpPlusPlusWireV1, FcmpNativeErrorV1> {
    use curve25519_dalek::{constants::ED25519_BASEPOINT_POINT, scalar::Scalar};
    if bytes.len() < FCMP_PROOF_WIRE_HEADER_BYTES_V1
        || bytes.get(6..8) != Some([0_u8, 0_u8].as_slice())
    {
        return Err(FcmpNativeErrorV1::ProofWireReserved);
    }
    let inputs = usize::from(bytes[4]);
    let layers = bytes[5];
    let membership_only_len = FCMP_PROOF_WIRE_HEADER_BYTES_V1
        .checked_add(
            inputs
                .checked_mul(FCMP_PROOF_INPUT_BYTES_V1)
                .ok_or(FcmpNativeErrorV1::TreeFull)?,
        )
        .and_then(|value| {
            fcmp_membership_proof_size_v1(inputs, usize::from(layers))
                .ok()
                .and_then(|membership| value.checked_add(membership))
        })
        .ok_or(FcmpNativeErrorV1::TreeFull)?;
    if bytes.len() != membership_only_len {
        return Err(FcmpNativeErrorV1::ProofLength {
            actual: bytes.len(),
            expected: membership_only_len,
        });
    }
    let output_count = 1_usize;
    let mut framed = Vec::with_capacity(
        membership_only_len
            .checked_add(fcmp_range_proof_size_v1(output_count)?)
            .ok_or(FcmpNativeErrorV1::TreeFull)?,
    );
    framed.extend_from_slice(bytes);
    framed[6] = u8::try_from(output_count).map_err(|_| FcmpNativeErrorV1::ArithmeticInvariant)?;
    let words = fcmp_range_proof_size_v1(output_count)? / FCMP_POINT_BYTES_V1;
    for index in 0..words {
        if index < 3 || index >= 6 {
            framed.extend_from_slice(
                &(ED25519_BASEPOINT_POINT
                    * Scalar::from(
                        u64::try_from(index + 1)
                            .map_err(|_| FcmpNativeErrorV1::ArithmeticInvariant)?,
                    ))
                .compress()
                .to_bytes(),
            );
        } else {
            framed.extend_from_slice(
                &Scalar::from(
                    u64::try_from(index + 1).map_err(|_| FcmpNativeErrorV1::ArithmeticInvariant)?,
                )
                .to_bytes(),
            );
        }
    }
    decode_fcmp_plus_plus_wire_v1(&framed, public_inputs, root)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::fcmp_plus_plus::{build_fcmp_frontier_v1, output_from_multiples};
    use curve25519_dalek::{constants::ED25519_BASEPOINT_POINT, scalar::Scalar};
    use iroha_data_model::privacy::FCMP_MAX_INPUTS_V1;
    #[test]
    fn input_bound_matches_the_typed_consensus_model() {
        assert_eq!(
            FCMP_MAX_INPUTS_NATIVE_V1,
            usize::try_from(FCMP_MAX_INPUTS_V1).expect("model limit fits usize")
        );
    }
    fn point(multiple: u64) -> [u8; 32] {
        (ED25519_BASEPOINT_POINT * Scalar::from(multiple))
            .compress()
            .to_bytes()
    }
    fn one_layer_root() -> FcmpTreeRootV1 {
        build_fcmp_frontier_v1(&[output_from_multiples(1, 2, 3)])
            .expect("root")
            .root
    }
    fn structural_wire(
        inputs: usize,
        root: FcmpTreeRootV1,
    ) -> (Vec<u8>, Vec<FcmpProofInputPublicV1>) {
        structural_wire_for_outputs(inputs, root, 1)
    }
    fn structural_wire_for_outputs(
        inputs: usize,
        root: FcmpTreeRootV1,
        outputs: usize,
    ) -> (Vec<u8>, Vec<FcmpProofInputPublicV1>) {
        let len = fcmp_plus_plus_wire_size_v1(inputs, root.layers(), outputs).expect("wire size");
        let mut wire = vec![0_u8; len];
        wire[..4].copy_from_slice(&FCMP_PROOF_WIRE_MAGIC_V1);
        wire[4] = u8::try_from(inputs).expect("test input count fits u8");
        wire[5] = root.layers();
        wire[6] = u8::try_from(outputs).expect("test output count fits u8");
        let mut cursor = FCMP_PROOF_WIRE_HEADER_BYTES_V1;
        let mut multiple = 10_u64;
        let mut public_inputs = Vec::with_capacity(inputs);
        for input_index in 0..inputs {
            let output_key_tilde = point(multiple);
            let linking_tag_generator_tilde = point(multiple + 1);
            let rerandomization_commitment = point(multiple + 2);
            for relation_point in [
                output_key_tilde,
                linking_tag_generator_tilde,
                rerandomization_commitment,
            ] {
                wire[cursor..cursor + 32].copy_from_slice(&relation_point);
                cursor += 32;
            }
            multiple += 3;
            for _ in 0..SAL_POINT_COUNT_V1 {
                wire[cursor..cursor + 32].copy_from_slice(&point(multiple));
                cursor += 32;
                multiple += 1;
            }
            for scalar in 1..=SAL_SCALAR_COUNT_V1 {
                wire[cursor] = u8::try_from(scalar).expect("small scalar");
                cursor += 32;
            }
            let public_base = u64::try_from(input_index).expect("index fits u64") * 2 + 10_000;
            public_inputs.push(
                FcmpProofInputPublicV1::new(
                    output_key_tilde,
                    linking_tag_generator_tilde,
                    rerandomization_commitment,
                    point(public_base),
                    point(public_base + 1),
                )
                .expect("public input"),
            );
        }
        let membership_size =
            fcmp_membership_proof_size_v1(inputs, usize::from(root.layers())).expect("size");
        let circuit_size = membership_size - ROOT_BLIND_POK_BYTES_V1;
        wire[cursor] = 1;
        cursor += circuit_size;
        wire[cursor..cursor + 32].copy_from_slice(&root.point());
        cursor += 32;
        wire[cursor] = 1;
        cursor += 32;
        let range_words = fcmp_range_proof_size_v1(outputs).expect("range size") / 32;
        for index in 0..range_words {
            if index < 3 || index >= 6 {
                wire[cursor..cursor + 32].copy_from_slice(&point(multiple));
                multiple += 1;
            } else {
                wire[cursor] = u8::try_from(index + 1).expect("small scalar");
            }
            cursor += 32;
        }
        assert_eq!(cursor, wire.len());
        (wire, public_inputs)
    }
    #[test]
    fn proof_size_matches_pinned_upstream_layout_kats() {
        // Exhaustively generated by full-chain-membership-proofs 0.1.0 at
        // 15ef711 for all compiled input/depth pairs, plus the IFC1 header.
        const ONE_INPUT: [usize; 32] = [
            3_368, 3_784, 4_008, 4_552, 4_904, 5_320, 5_544, 6_088, 5_736, 5_256, 5_448, 5_800,
            5_992, 6_216, 6_408, 6_760, 6_440, 5_928, 5_960, 6_152, 6_344, 6_536, 6_696, 6_888,
            7_080, 7_272, 7_304, 7_496, 7_688, 7_880, 8_040, 8_232,
        ];
        const TWO_INPUTS: [usize; 32] = [
            4_648, 5_608, 5_256, 6_088, 6_600, 6_344, 6_856, 7_304, 7_112, 6_664, 7_016, 7_400,
            7_752, 8_008, 8_360, 8_744, 8_584, 8_104, 8_296, 8_520, 8_872, 9_224, 9_288, 9_640,
            9_992, 10_216, 10_408, 10_632, 10_984, 11_336, 11_400, 11_752,
        ];
        for (input_count, expected_sizes) in [(1, ONE_INPUT), (2, TWO_INPUTS)] {
            for (layer_index, expected) in expected_sizes.into_iter().enumerate() {
                let layers = u8::try_from(layer_index + 1).expect("compiled layer count fits u8");
                assert_eq!(
                    fcmp_plus_plus_wire_size_v1(input_count, layers, 1),
                    Ok(expected + fcmp_range_proof_size_v1(1).expect("range size")),
                    "input_count={input_count}, layers={layers}"
                );
            }
        }
        assert_eq!(
            fcmp_plus_plus_wire_size_v1(1, 1, 1),
            Ok(FCMP_MIN_PROOF_WIRE_BYTES_V1)
        );
        assert_eq!(
            fcmp_plus_plus_wire_size_v1(
                FCMP_MAX_INPUTS_NATIVE_V1,
                super::super::FCMP_MAX_TREE_LAYERS_V1,
                FCMP_MAX_OUTPUTS_NATIVE_V1,
            ),
            Ok(FCMP_MAX_PROOF_WIRE_BYTES_V1)
        );
        assert!(matches!(
            fcmp_plus_plus_wire_size_v1(0, 1, 1),
            Err(FcmpNativeErrorV1::InputCount { .. })
        ));
        assert!(matches!(
            fcmp_plus_plus_wire_size_v1(FCMP_MAX_INPUTS_NATIVE_V1 + 1, 1, 1),
            Err(FcmpNativeErrorV1::InputCount { .. })
        ));
        assert_eq!(
            fcmp_plus_plus_wire_size_v1(1, 0, 1),
            Err(FcmpNativeErrorV1::LayerCount)
        );
        assert_eq!(
            fcmp_plus_plus_wire_size_v1(1, super::super::FCMP_MAX_TREE_LAYERS_V1 + 1, 1,),
            Err(FcmpNativeErrorV1::LayerCount)
        );
        assert!(matches!(
            fcmp_plus_plus_wire_size_v1(1, 1, 0),
            Err(FcmpNativeErrorV1::OutputCount { .. })
        ));
        assert!(matches!(
            fcmp_plus_plus_wire_size_v1(1, 1, FCMP_MAX_OUTPUTS_NATIVE_V1 + 1),
            Err(FcmpNativeErrorV1::OutputCount { .. })
        ));
    }
    #[test]
    fn decoder_accepts_the_exact_compiled_maximum_and_rejects_plus_one_limits() {
        let root = FcmpTreeRootV1::new(
            super::super::FCMP_MAX_TREE_LAYERS_V1,
            crate::privacy_engines::fcmp_plus_plus::field::helios_hash_initializer().encode(),
        )
        .expect("maximum even-layer root");
        let (wire, public) = structural_wire_for_outputs(
            FCMP_MAX_INPUTS_NATIVE_V1,
            root,
            FCMP_MAX_OUTPUTS_NATIVE_V1,
        );
        assert_eq!(wire.len(), FCMP_MAX_PROOF_WIRE_BYTES_V1);
        assert!(
            decode_fcmp_plus_plus_wire_v1(&wire, &public, root).is_ok(),
            "the exact compiled maximum must remain decodable"
        );
        let mut trailing = wire.clone();
        trailing.push(0);
        assert_eq!(trailing.len(), FCMP_MAX_PROOF_WIRE_BYTES_V1 + 1);
        assert!(matches!(
            decode_fcmp_plus_plus_wire_v1(&trailing, &public, root),
            Err(FcmpNativeErrorV1::ProofLength {
                actual,
                expected: FCMP_MAX_PROOF_WIRE_BYTES_V1
            }) if actual == FCMP_MAX_PROOF_WIRE_BYTES_V1 + 1
        ));
        let mut too_many_inputs = wire.clone();
        too_many_inputs[4] =
            u8::try_from(FCMP_MAX_INPUTS_NATIVE_V1 + 1).expect("small compiled limit");
        assert!(matches!(
            decode_fcmp_plus_plus_wire_v1(&too_many_inputs, &public, root),
            Err(FcmpNativeErrorV1::InputCount { .. })
        ));
        let mut too_many_outputs = wire.clone();
        too_many_outputs[6] =
            u8::try_from(FCMP_MAX_OUTPUTS_NATIVE_V1 + 1).expect("small compiled limit");
        assert!(matches!(
            decode_fcmp_plus_plus_wire_v1(&too_many_outputs, &public, root),
            Err(FcmpNativeErrorV1::OutputCount { .. })
        ));
        let mut too_many_layers = wire;
        too_many_layers[5] = super::super::FCMP_MAX_TREE_LAYERS_V1 + 1;
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&too_many_layers, &public, root),
            Err(FcmpNativeErrorV1::LayerCount)
        );
    }
    #[test]
    fn strict_wire_roundtrip_shape_and_public_binding() {
        let root = one_layer_root();
        let (wire, public) = structural_wire(1, root);
        let parsed =
            decode_fcmp_plus_plus_wire_v1(&wire, &public, root).expect("structural decode");
        assert_eq!(parsed.input_count, 1);
        assert_eq!(parsed.layers, 1);
        assert_eq!(parsed.inputs.len(), 1);
        assert!(!parsed.circuit_proof.is_empty());
    }
    #[test]
    fn framing_and_noncanonical_components_fail_closed() {
        let root = one_layer_root();
        let (wire, public) = structural_wire(1, root);
        let mut mutation = wire.clone();
        mutation[0] ^= 1;
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&mutation, &public, root),
            Err(FcmpNativeErrorV1::ProofWireMagic)
        );
        let mut mutation = wire.clone();
        mutation[7] = 1;
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&mutation, &public, root),
            Err(FcmpNativeErrorV1::ProofWireReserved)
        );
        assert!(matches!(
            decode_fcmp_plus_plus_wire_v1(&wire[..wire.len() - 1], &public, root),
            Err(FcmpNativeErrorV1::ProofLength { .. })
        ));
        let mut trailing = wire.clone();
        trailing.push(0);
        assert!(matches!(
            decode_fcmp_plus_plus_wire_v1(&trailing, &public, root),
            Err(FcmpNativeErrorV1::ProofLength { .. })
        ));
        let mut identity = wire.clone();
        identity[FCMP_PROOF_WIRE_HEADER_BYTES_V1..FCMP_PROOF_WIRE_HEADER_BYTES_V1 + 32].fill(0);
        identity[FCMP_PROOF_WIRE_HEADER_BYTES_V1] = 1;
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&identity, &public, root),
            Err(FcmpNativeErrorV1::EdwardsPointIdentity)
        );
        let scalar_offset =
            FCMP_PROOF_WIRE_HEADER_BYTES_V1 + (3 + SAL_POINT_COUNT_V1) * FCMP_POINT_BYTES_V1;
        let mut noncanonical_scalar = wire.clone();
        noncanonical_scalar[scalar_offset..scalar_offset + 32].fill(u8::MAX);
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&noncanonical_scalar, &public, root),
            Err(FcmpNativeErrorV1::ScalarEncoding)
        );
    }
    #[test]
    fn every_structural_wire_field_is_canonical_or_explicitly_opaque() {
        let root = one_layer_root();
        let (wire, public) = structural_wire(2, root);
        for magic_index in 0..FCMP_PROOF_WIRE_MAGIC_V1.len() {
            let mut mutation = wire.clone();
            mutation[magic_index] ^= 1;
            assert_eq!(
                decode_fcmp_plus_plus_wire_v1(&mutation, &public, root),
                Err(FcmpNativeErrorV1::ProofWireMagic)
            );
        }
        for reserved_index in 7..FCMP_PROOF_WIRE_HEADER_BYTES_V1 {
            let mut mutation = wire.clone();
            mutation[reserved_index] = 1;
            assert_eq!(
                decode_fcmp_plus_plus_wire_v1(&mutation, &public, root),
                Err(FcmpNativeErrorV1::ProofWireReserved)
            );
        }
        let mut zero_inputs = wire.clone();
        zero_inputs[4] = 0;
        assert!(matches!(
            decode_fcmp_plus_plus_wire_v1(&zero_inputs, &public, root),
            Err(FcmpNativeErrorV1::InputCount { .. })
        ));
        let mut zero_layers = wire.clone();
        zero_layers[5] = 0;
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&zero_layers, &public, root),
            Err(FcmpNativeErrorV1::LayerCount)
        );
        let mut zero_outputs = wire.clone();
        zero_outputs[6] = 0;
        assert!(matches!(
            decode_fcmp_plus_plus_wire_v1(&zero_outputs, &public, root),
            Err(FcmpNativeErrorV1::OutputCount { .. })
        ));
        for relation_index in 0..3 {
            let mut mutation = wire.clone();
            let offset = FCMP_PROOF_WIRE_HEADER_BYTES_V1 + relation_index * FCMP_POINT_BYTES_V1;
            mutation[offset..offset + FCMP_POINT_BYTES_V1].copy_from_slice(&point(
                20_000 + u64::try_from(relation_index).expect("small relation index"),
            ));
            assert_eq!(
                decode_fcmp_plus_plus_wire_v1(&mutation, &public, root),
                Err(FcmpNativeErrorV1::ProofPublicInputMismatch { index: 0 })
            );
        }
        let mut cursor = FCMP_PROOF_WIRE_HEADER_BYTES_V1;
        let identity = {
            let mut encoding = [0; 32];
            encoding[0] = 1;
            encoding
        };
        for _ in 0..2 {
            for _ in 0..(3 + SAL_POINT_COUNT_V1) {
                let mut mutation = wire.clone();
                mutation[cursor..cursor + FCMP_POINT_BYTES_V1].copy_from_slice(&identity);
                assert_eq!(
                    decode_fcmp_plus_plus_wire_v1(&mutation, &public, root),
                    Err(FcmpNativeErrorV1::EdwardsPointIdentity)
                );
                cursor += FCMP_POINT_BYTES_V1;
            }
            for _ in 0..SAL_SCALAR_COUNT_V1 {
                let mut mutation = wire.clone();
                mutation[cursor..cursor + FCMP_POINT_BYTES_V1].fill(u8::MAX);
                assert_eq!(
                    decode_fcmp_plus_plus_wire_v1(&mutation, &public, root),
                    Err(FcmpNativeErrorV1::ScalarEncoding)
                );
                cursor += FCMP_POINT_BYTES_V1;
            }
        }
        let membership_size =
            fcmp_membership_proof_size_v1(2, usize::from(root.layers())).expect("size");
        let circuit_size = membership_size - ROOT_BLIND_POK_BYTES_V1;
        let circuit_start = cursor;
        let mut opaque_mutation = wire.clone();
        opaque_mutation[circuit_start + (circuit_size / 2)] ^= 0x5a;
        assert!(
            decode_fcmp_plus_plus_wire_v1(&opaque_mutation, &public, root).is_ok(),
            "the parser must not pretend an opaque circuit-byte mutation is cryptographic verification"
        );
        cursor += circuit_size;
        let mut identity_root_blind = wire.clone();
        identity_root_blind[cursor..cursor + FCMP_POINT_BYTES_V1].fill(0);
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&identity_root_blind, &public, root),
            Err(FcmpNativeErrorV1::CyclePointIdentity)
        );
        cursor += FCMP_POINT_BYTES_V1;
        let mut noncanonical_root_response = wire.clone();
        noncanonical_root_response[cursor..cursor + FCMP_POINT_BYTES_V1].fill(u8::MAX);
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&noncanonical_root_response, &public, root),
            Err(FcmpNativeErrorV1::ScalarEncoding)
        );
        cursor += FCMP_POINT_BYTES_V1;
        cursor += fcmp_range_proof_size_v1(1).expect("range size");
        assert_eq!(cursor, wire.len());
    }
    #[test]
    fn public_inputs_and_alternating_root_curves_fail_closed() {
        let selene_root = one_layer_root();
        let (wire, public) = structural_wire(1, selene_root);
        let mut mismatching_relation = public.clone();
        mismatching_relation[0].output_key_tilde = point(30_000);
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&wire, &mismatching_relation, selene_root),
            Err(FcmpNativeErrorV1::ProofPublicInputMismatch { index: 0 })
        );
        let mut identity = [0; 32];
        identity[0] = 1;
        for field_index in 0..5 {
            let mut identity_public = public.clone();
            match field_index {
                0 => identity_public[0].output_key_tilde = identity,
                1 => identity_public[0].linking_tag_generator_tilde = identity,
                2 => identity_public[0].rerandomization_commitment = identity,
                3 => identity_public[0].pseudo_out = identity,
                4 => identity_public[0].key_image = identity,
                _ => unreachable!("five public relation fields"),
            }
            assert_eq!(
                decode_fcmp_plus_plus_wire_v1(&wire, &identity_public, selene_root),
                Err(FcmpNativeErrorV1::EdwardsPointIdentity)
            );
            let mut noncanonical_public = public.clone();
            match field_index {
                0 => noncanonical_public[0].output_key_tilde = [u8::MAX; 32],
                1 => noncanonical_public[0].linking_tag_generator_tilde = [u8::MAX; 32],
                2 => noncanonical_public[0].rerandomization_commitment = [u8::MAX; 32],
                3 => noncanonical_public[0].pseudo_out = [u8::MAX; 32],
                4 => noncanonical_public[0].key_image = [u8::MAX; 32],
                _ => unreachable!("five public relation fields"),
            }
            assert_eq!(
                decode_fcmp_plus_plus_wire_v1(&wire, &noncanonical_public, selene_root),
                Err(FcmpNativeErrorV1::EdwardsPointEncoding)
            );
        }
        let helios_root = FcmpTreeRootV1::from_helios(
            2,
            crate::privacy_engines::fcmp_plus_plus::field::helios_hash_initializer(),
        )
        .expect("canonical even-layer root");
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&wire, &public, helios_root),
            Err(FcmpNativeErrorV1::ProofHeaderMismatch)
        );
        let (helios_wire, helios_public) = structural_wire(1, helios_root);
        assert!(decode_fcmp_plus_plus_wire_v1(&helios_wire, &helios_public, helios_root).is_ok());
        let root_blind_offset = helios_wire.len()
            - fcmp_range_proof_size_v1(1).expect("range size")
            - ROOT_BLIND_POK_BYTES_V1;
        let mut wrong_curve = helios_wire;
        wrong_curve[root_blind_offset..root_blind_offset + FCMP_POINT_BYTES_V1].copy_from_slice(
            &crate::privacy_engines::fcmp_plus_plus::field::selene_hash_initializer().encode(),
        );
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&wrong_curve, &helios_public, helios_root),
            Err(FcmpNativeErrorV1::CyclePointEncoding)
        );
    }
    #[test]
    fn duplicate_public_inputs_and_zero_circuit_proof_fail_closed() {
        let root = one_layer_root();
        let (wire, mut public) = structural_wire(2, root);
        public[1].key_image = public[0].key_image;
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&wire, &public, root),
            Err(FcmpNativeErrorV1::DuplicateKeyImage)
        );
        let (wire, mut public) = structural_wire(2, root);
        public[1].pseudo_out = public[0].pseudo_out;
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&wire, &public, root),
            Err(FcmpNativeErrorV1::DuplicatePseudoOut)
        );
        let (mut wire, public) = structural_wire(1, root);
        let circuit_start = FCMP_PROOF_WIRE_HEADER_BYTES_V1 + FCMP_PROOF_INPUT_BYTES_V1;
        let circuit_size =
            fcmp_membership_proof_size_v1(1, 1).expect("size") - ROOT_BLIND_POK_BYTES_V1;
        wire[circuit_start..circuit_start + circuit_size].fill(0);
        assert_eq!(
            decode_fcmp_plus_plus_wire_v1(&wire, &public, root),
            Err(FcmpNativeErrorV1::EmptyCircuitProof)
        );
    }
    #[test]
    fn every_truncation_and_hostile_header_fails_without_panicking() {
        let root = one_layer_root();
        let (wire, public) = structural_wire(2, root);
        for length in 0..wire.len() {
            let result = std::panic::catch_unwind(|| {
                decode_fcmp_plus_plus_wire_v1(&wire[..length], &public, root)
            });
            assert!(
                result.is_ok(),
                "decoder panicked for prefix length {length}"
            );
            assert!(
                result.expect("checked above").is_err(),
                "truncated prefix length {length} was accepted"
            );
        }
        for header_index in 0..FCMP_PROOF_WIRE_HEADER_BYTES_V1 {
            for replacement in [0_u8, 1, u8::MAX] {
                if wire[header_index] == replacement {
                    continue;
                }
                let mut mutation = wire.clone();
                mutation[header_index] = replacement;
                let result = std::panic::catch_unwind(|| {
                    decode_fcmp_plus_plus_wire_v1(&mutation, &public, root)
                });
                assert!(
                    result.is_ok(),
                    "decoder panicked for header byte {header_index}={replacement}"
                );
            }
        }
    }
}
