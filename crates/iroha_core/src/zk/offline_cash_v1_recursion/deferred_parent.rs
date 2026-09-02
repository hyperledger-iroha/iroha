//! In-circuit Pasta parent-proof and BGH19 history-fold primitives.
//!
//! These primitives are deliberately scalar-half only.  Every emitted symbolic curve equation
//! must be consumed by the reciprocal parity's point-audit relation before a proof can carry
//! monetary authority.  The production wrapper keeps this module private so an unconstrained
//! scalar half can never be installed as an [`super::OfflineCashRecursiveVerifierV1`].

use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    io::{self, Read},
    rc::Rc,
};

use ff::{Field as _, PrimeField as _};
use halo2_base::{
    AssignedValue,
    QuantumCell::{Constant, Existing},
    gates::circuit::builder::BaseCircuitBuilder,
    gates::{GateInstructions as _, RangeInstructions as _},
    utils::{BigPrimeField, CurveAffineExt},
};
use halo2_ecc::fields::fp::FpChip;
use halo2_proofs::halo2curves::CurveAffine;
use sha2::{Digest as _, Sha256};
use snark_verifier::{
    Error,
    loader::{Loader, halo2::Halo2Loader, native::NativeLoader},
    pcs::{
        AccumulationScheme,
        ipa::{Bgh19, IpaAccumulator, IpaAs, IpaSuccinctVerifyingKey},
    },
    system::halo2::transcript::halo2::PoseidonTranscript,
    util::hash::Poseidon,
    verifier::{
        SnarkVerifier as _,
        plonk::{PlonkProtocol, PlonkSuccinctVerifier},
    },
};

use super::{
    OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1, OFFLINE_CASH_IPA_POSEIDON_FULL_ROUNDS_V1,
    OFFLINE_CASH_IPA_POSEIDON_PARTIAL_ROUNDS_V1, OFFLINE_CASH_IPA_POSEIDON_RATE_V1,
    OFFLINE_CASH_IPA_POSEIDON_SECURE_MDS_V1, OFFLINE_CASH_IPA_POSEIDON_WIDTH_V1,
    OFFLINE_CASH_RECURSION_IPA_K_V1, OfflineCashPastaParityV1, state_relation::public_instance,
};
use crate::zk::pasta_cycle_loader::{
    DeferredEquationWitness, DeferredScalarEccChip, LIMB_BITS, LIMBS, PastaCycleEccChip,
    constrain_reciprocal_poseidon_v1, pasta_poseidon_domain_elements_v1,
};
use crate::zk::pasta_dense_msm::PastaDenseMsmJobsV1;

// These scalar and reciprocal helpers are shared by the production state, GuardBundle, and
// mint-authority composites. Each accepting circuit carries every resulting opening claim into
// its authenticated history and enforces the reciprocal curve equations in the opposite parity.

pub(super) type DeferredLoader<'chip, C> = Rc<Halo2Loader<C, DeferredScalarEccChip<'chip, C>>>;
pub(super) type DeferredScalar<'chip, C> =
    snark_verifier::loader::halo2::Scalar<C, DeferredScalarEccChip<'chip, C>>;
pub(super) type DeferredAccumulator<'chip, C> = IpaAccumulator<C, DeferredLoader<'chip, C>>;
type DeferredTranscript<'chip, C, R> = PoseidonTranscript<
    C,
    DeferredLoader<'chip, C>,
    R,
    OFFLINE_CASH_IPA_POSEIDON_WIDTH_V1,
    OFFLINE_CASH_IPA_POSEIDON_RATE_V1,
    OFFLINE_CASH_IPA_POSEIDON_FULL_ROUNDS_V1,
    OFFLINE_CASH_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
>;

const OFFLINE_CASH_PROTOCOL_STRUCTURE_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:compiled-protocol-structure";
const OFFLINE_CASH_PROTOCOL_IDENTITY_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:compiled-protocol-identity";
const OFFLINE_CASH_PROTOCOL_STRUCTURE_VERSION_V1: u32 = 1;
const OFFLINE_CASH_PROTOCOL_IDENTITY_VERSION_V1: u32 = 1;
const SNARK_VERIFIER_PROTOCOL_REVISION_V1: &str = "bbfcc721d714bea0d44a27c8fc6c4736e73ca853";

/// Exact transcript inventory of one ordinary Pasta/IPA proof.
///
/// Halo2's Poseidon transcript writes one 32-byte encoding per commitment,
/// evaluation, BGH19 opening item, and IPA item.  Keeping this derivation next
/// to the parser prevents internal helper proofs from inheriting a wire-slot
/// limit while still rejecting truncation, padding, and trailing bytes before
/// any verifier work begins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashOrdinaryProofProfileV1 {
    /// Witness commitments across all challenge phases.
    pub(super) witness_commitments: usize,
    /// Quotient-polynomial commitments.
    pub(super) quotient_commitments: usize,
    /// Scalar evaluations read from the transcript.
    pub(super) evaluations: usize,
    /// BGH19 evaluations, one per distinct polynomial rotation set.
    pub(super) bgh19_rotation_sets: usize,
    /// Fixed BGH19 and IPA transcript items for the authenticated `k`.
    pub(super) opening_items: usize,
    /// Exact canonical proof length.
    pub(super) byte_len: usize,
}

/// Derive the exact canonical byte length accepted by the ordinary proof parser.
pub(super) fn ordinary_ipa_proof_profile_v1<C, L>(
    protocol: &PlonkProtocol<C, L>,
) -> Result<OfflineCashOrdinaryProofProfileV1, String>
where
    C: CurveAffine,
    L: Loader<C>,
{
    validate_ordinary_ipa_protocol_shape_v1(
        protocol.domain.k,
        protocol.num_witness.len(),
        protocol.num_challenge.len(),
        protocol.quotient.chunk_degree,
        protocol.queries.len(),
    )?;
    let witness_commitments = protocol
        .num_witness
        .iter()
        .try_fold(0_usize, |total, count| total.checked_add(*count))
        .ok_or_else(|| "Offline Cash witness-commitment count overflowed".to_owned())?;
    let quotient_commitments = protocol.quotient.num_chunk();
    let evaluations = protocol.evaluations.len();
    let mut rotations_by_polynomial = BTreeMap::<usize, BTreeSet<i32>>::new();
    for query in &protocol.queries {
        rotations_by_polynomial
            .entry(query.poly)
            .or_default()
            .insert(query.rotation.0);
    }
    let bgh19_rotation_sets = rotations_by_polynomial
        .into_values()
        .map(|rotations| rotations.into_iter().collect::<Vec<_>>())
        .collect::<BTreeSet<_>>()
        .len();
    ordinary_ipa_proof_profile_from_counts_v1(
        witness_commitments,
        quotient_commitments,
        evaluations,
        bgh19_rotation_sets,
    )
}

fn validate_ordinary_ipa_protocol_shape_v1(
    domain_k: usize,
    witness_phases: usize,
    challenge_phases: usize,
    quotient_chunk_degree: usize,
    query_count: usize,
) -> Result<(), String> {
    if domain_k != OFFLINE_CASH_RECURSION_IPA_K_V1 as usize {
        return Err(format!(
            "Offline Cash ordinary proof protocol uses k={}, expected k={OFFLINE_CASH_RECURSION_IPA_K_V1}",
            domain_k
        ));
    }
    if witness_phases != challenge_phases {
        return Err(
            "Offline Cash ordinary proof protocol has mismatched witness/challenge phases"
                .to_owned(),
        );
    }
    if quotient_chunk_degree == 0 {
        return Err(
            "Offline Cash ordinary proof protocol has zero quotient chunk degree".to_owned(),
        );
    }
    if query_count == 0 {
        return Err("Offline Cash ordinary proof protocol has no PCS queries".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OfflineCashIpaProofKindV1 {
    Ordinary,
    Fold,
}

fn validate_zk_ipa_succinct_key_v1<C>(
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    proof_kind: OfflineCashIpaProofKindV1,
) -> Result<(), Error>
where
    C: CurveAffine,
{
    if !succinct_vk.zk() {
        let label = match proof_kind {
            OfflineCashIpaProofKindV1::Ordinary => "ordinary proof",
            OfflineCashIpaProofKindV1::Fold => "fold proof",
        };
        return Err(transcript_error(format!(
            "Offline Cash {label} requires a zero-knowledge IPA key"
        )));
    }
    Ok(())
}

fn ordinary_ipa_proof_profile_from_counts_v1(
    witness_commitments: usize,
    quotient_commitments: usize,
    evaluations: usize,
    bgh19_rotation_sets: usize,
) -> Result<OfflineCashOrdinaryProofProfileV1, String> {
    // BGH19 contributes F, one scalar per rotation set, and S. IPA then
    // contributes two points per round, c, blind, and the final basis point.
    let opening_items = usize::try_from(OFFLINE_CASH_RECURSION_IPA_K_V1)
        .ok()
        .and_then(|rounds| rounds.checked_mul(2))
        .and_then(|items| items.checked_add(5))
        .ok_or_else(|| "Offline Cash IPA opening-item count overflowed".to_owned())?;
    let transcript_items = witness_commitments
        .checked_add(quotient_commitments)
        .and_then(|items| items.checked_add(evaluations))
        .and_then(|items| items.checked_add(bgh19_rotation_sets))
        .and_then(|items| items.checked_add(opening_items))
        .ok_or_else(|| "Offline Cash proof transcript-item count overflowed".to_owned())?;
    let byte_len = transcript_items
        .checked_mul(32)
        .ok_or_else(|| "Offline Cash proof byte length overflowed".to_owned())?;
    Ok(OfflineCashOrdinaryProofProfileV1 {
        witness_commitments,
        quotient_commitments,
        evaluations,
        bgh19_rotation_sets,
        opening_items,
        byte_len,
    })
}

/// One witness-loaded parent protocol whose self-referential commitments and transcript state
/// have been bound to the shared field-native protocol identity.
pub(super) struct OfflineCashLoadedParentProtocolV1<'chip, C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    pub(super) protocol: PlonkProtocol<C, DeferredLoader<'chip, C>>,
}

/// Witness-load the self-referential parts of a compiled parent protocol and constrain their
/// native Poseidon identity to the two public `u128` limbs.
///
/// The value-free structure digest is a circuit constant and is checked against the supplied
/// compiled protocol before synthesis. The final authority-bearing identity is Poseidon in the
/// proof curve scalar field; SHA-256 is used only for the fixed structure descriptor whose bytes
/// are themselves absorbed as constants by that Poseidon relation.
pub(super) fn load_and_constrain_parent_protocol_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    protocol: &PlonkProtocol<C>,
    parity: OfflineCashPastaParityV1,
    fixed_structure_digest: [u8; 32],
    expected_limbs: &[AssignedValue<C::ScalarExt>; 2],
) -> Result<OfflineCashLoadedParentProtocolV1<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    load_and_constrain_parent_protocol_if_v1(
        loader,
        protocol,
        parity,
        fixed_structure_digest,
        expected_limbs,
        None,
    )
}

/// Witness-load a parent protocol and optionally gate its public identity equality.
///
/// Bootstrap uses the selector-zero branch because it has no monetary predecessor. The complete
/// parser and verifier shape is still instantiated, while every resulting point equation is also
/// disabled by the same selector in the caller.
pub(super) fn load_and_constrain_parent_protocol_if_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    protocol: &PlonkProtocol<C>,
    parity: OfflineCashPastaParityV1,
    fixed_structure_digest: [u8; 32],
    expected_limbs: &[AssignedValue<C::ScalarExt>; 2],
    enabled: Option<AssignedValue<C::ScalarExt>>,
) -> Result<OfflineCashLoadedParentProtocolV1<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    if protocol.preprocessed.is_empty()
        || protocol
            .preprocessed
            .iter()
            .any(|point| bool::from(point.is_identity()))
        || offline_cash_protocol_structure_digest_v1(protocol, parity).map_err(transcript_error)?
            != fixed_structure_digest
    {
        return Err(Error::InvalidInstances);
    }
    let loaded = protocol.loaded_preprocessed_as_witness(loader, false);
    let mut elements = pasta_poseidon_domain_elements_v1::<C::ScalarExt>(
        OFFLINE_CASH_PROTOCOL_IDENTITY_DOMAIN_V1,
        OFFLINE_CASH_PROTOCOL_IDENTITY_VERSION_V1,
    );
    elements.push(C::ScalarExt::from(u64::from(parity_tag(parity))));
    elements.extend(fixed_structure_digest.chunks_exact(8).map(|chunk| {
        C::ScalarExt::from(u64::from_le_bytes(
            chunk
                .try_into()
                .expect("structure digest chunk has eight bytes"),
        ))
    }));
    elements.push(C::ScalarExt::from(
        u64::try_from(loaded.preprocessed.len()).expect("fixed preprocessed count fits u64"),
    ));
    let chip = loader.ecc_chip();
    let mut ctx = loader.ctx_mut();
    let mut assigned = elements
        .into_iter()
        .map(|value| ctx.main().load_constant(value))
        .collect::<Vec<_>>();
    for point in &loaded.preprocessed {
        assigned.extend(chip.assigned_point_poseidon_elements_v1(&mut ctx, &point.assigned())?);
    }
    let transcript_state = loaded
        .transcript_initial_state
        .as_ref()
        .ok_or_else(|| transcript_error("Offline Cash parent protocol has no transcript state"))?;
    assigned.push(*transcript_state.assigned());
    drop(ctx);
    drop(chip);
    let digest = poseidon_digest_assigned(loader, assigned);
    let limbs = assigned_scalar_u128_limbs(loader, digest);
    for (actual, expected) in limbs.iter().zip(expected_limbs) {
        if let Some(enabled) = enabled {
            let chip = loader.ecc_chip();
            let mut ctx = loader.ctx_mut();
            let difference = chip.range().gate().sub(ctx.main(), *actual, *expected);
            let selected = chip.range().gate().mul(ctx.main(), difference, enabled);
            chip.range()
                .gate()
                .assert_is_const(ctx.main(), &selected, &C::ScalarExt::ZERO);
        } else {
            loader.ctx_mut().main().constrain_equal(actual, expected);
        }
    }
    Ok(OfflineCashLoadedParentProtocolV1 { protocol: loaded })
}

/// Derive the canonical field-native identity used by the release manifest and recursive circuit.
pub(super) fn native_parent_protocol_digest_v1<C>(
    protocol: &PlonkProtocol<C>,
    parity: OfflineCashPastaParityV1,
) -> Result<[u8; 32], String>
where
    C: CurveAffine,
    C::ScalarExt: BigPrimeField,
{
    use snark_verifier::{loader::native::NativeLoader, util::hash::Poseidon};

    let structure = offline_cash_protocol_structure_digest_v1(protocol, parity)?;
    let transcript_state = protocol
        .transcript_initial_state
        .ok_or_else(|| "Offline Cash compiled protocol has no transcript state".to_owned())?;
    let mut elements = pasta_poseidon_domain_elements_v1::<C::ScalarExt>(
        OFFLINE_CASH_PROTOCOL_IDENTITY_DOMAIN_V1,
        OFFLINE_CASH_PROTOCOL_IDENTITY_VERSION_V1,
    );
    elements.push(C::ScalarExt::from(u64::from(parity_tag(parity))));
    elements.extend(structure.chunks_exact(8).map(|chunk| {
        C::ScalarExt::from(u64::from_le_bytes(
            chunk
                .try_into()
                .expect("structure digest chunk has eight bytes"),
        ))
    }));
    elements.push(C::ScalarExt::from(
        u64::try_from(protocol.preprocessed.len())
            .map_err(|_| "Offline Cash preprocessed point count exceeds u64".to_owned())?,
    ));
    for point in &protocol.preprocessed {
        elements.extend(compressed_point_poseidon_elements(*point)?);
    }
    elements.push(transcript_state);
    let mut poseidon = Poseidon::<
        C::ScalarExt,
        C::ScalarExt,
        OFFLINE_CASH_IPA_POSEIDON_WIDTH_V1,
        OFFLINE_CASH_IPA_POSEIDON_RATE_V1,
    >::new::<
        OFFLINE_CASH_IPA_POSEIDON_FULL_ROUNDS_V1,
        OFFLINE_CASH_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
        OFFLINE_CASH_IPA_POSEIDON_SECURE_MDS_V1,
    >(&NativeLoader);
    poseidon.update(&elements);
    let digest = poseidon.squeeze().to_repr();
    digest
        .as_ref()
        .try_into()
        .map_err(|_| "Offline Cash protocol digest is not 32 bytes".to_owned())
}

fn parity_tag(parity: OfflineCashPastaParityV1) -> u32 {
    match parity {
        OfflineCashPastaParityV1::Eq => 1,
        OfflineCashPastaParityV1::Ep => 2,
    }
}

fn append_len(output: &mut Vec<u8>, len: usize, label: &str) -> Result<(), String> {
    output.extend_from_slice(
        &u32::try_from(len)
            .map_err(|_| format!("Offline Cash {label} length exceeds u32"))?
            .to_le_bytes(),
    );
    Ok(())
}

fn append_index(output: &mut Vec<u8>, value: usize, label: &str) -> Result<(), String> {
    output.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| format!("Offline Cash {label} exceeds u32"))?
            .to_le_bytes(),
    );
    Ok(())
}

fn append_scalar<F: ff::PrimeField>(output: &mut Vec<u8>, value: F) -> Result<(), String> {
    let encoding = value.to_repr();
    if encoding.as_ref().len() != 32 {
        return Err("Offline Cash protocol scalar is not 32 bytes".to_owned());
    }
    output.extend_from_slice(encoding.as_ref());
    Ok(())
}

fn unary_expression(tag: u8, child: Result<Vec<u8>, String>) -> Result<Vec<u8>, String> {
    let child = child?;
    let mut encoded = vec![tag];
    append_len(&mut encoded, child.len(), "expression child")?;
    encoded.extend_from_slice(&child);
    Ok(encoded)
}

fn binary_expression(
    tag: u8,
    left: Result<Vec<u8>, String>,
    right: Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    let left = left?;
    let right = right?;
    let mut encoded = vec![tag];
    append_len(&mut encoded, left.len(), "left expression child")?;
    encoded.extend_from_slice(&left);
    append_len(&mut encoded, right.len(), "right expression child")?;
    encoded.extend_from_slice(&right);
    Ok(encoded)
}

fn encode_common_polynomial(value: ciborium::value::Value) -> Result<Vec<u8>, String> {
    match value {
        ciborium::value::Value::Text(variant) if variant == "Identity" => Ok(vec![1, 0]),
        ciborium::value::Value::Map(mut fields) if fields.len() == 1 => {
            let (variant, rotation) = fields.pop().expect("one checked enum field");
            let ciborium::value::Value::Text(variant) = variant else {
                return Err("Offline Cash common-polynomial variant is not text".to_owned());
            };
            if variant != "Lagrange" {
                return Err(format!(
                    "unsupported Offline Cash common-polynomial variant `{variant}`"
                ));
            }
            let ciborium::value::Value::Integer(rotation) = rotation else {
                return Err("Offline Cash Lagrange rotation is not an integer".to_owned());
            };
            let rotation = i32::try_from(rotation)
                .map_err(|_| "Offline Cash Lagrange rotation exceeds i32".to_owned())?;
            let mut encoded = vec![1, 1];
            encoded.extend_from_slice(&rotation.to_le_bytes());
            Ok(encoded)
        }
        _ => Err("unsupported Offline Cash common-polynomial encoding".to_owned()),
    }
}

fn encode_linearization(value: ciborium::value::Value) -> Result<u8, String> {
    match value {
        ciborium::value::Value::Null => Ok(0),
        ciborium::value::Value::Text(variant) if variant == "WithoutConstant" => Ok(1),
        ciborium::value::Value::Text(variant) if variant == "MinusVanishingTimesQuotient" => Ok(2),
        _ => Err("unsupported Offline Cash linearization encoding".to_owned()),
    }
}

fn append_point<C: CurveAffine>(output: &mut Vec<u8>, point: C) -> Result<(), String> {
    let encoding = point.to_bytes();
    if encoding.as_ref().len() != 32 {
        return Err("Offline Cash compiled protocol point is not 32 bytes".to_owned());
    }
    output.extend_from_slice(encoding.as_ref());
    Ok(())
}

/// Return the complete value-free descriptor of a compiled protocol.
///
/// Self-referential verifying-key commitments and transcript state are deliberately excluded here
/// and are instead witness-loaded into the native Poseidon identity above.
pub(super) fn offline_cash_protocol_structure_digest_v1<C>(
    protocol: &PlonkProtocol<C>,
    parity: OfflineCashPastaParityV1,
) -> Result<[u8; 32], String>
where
    C: CurveAffine,
    C::ScalarExt: ff::PrimeField,
{
    if protocol.domain_as_witness.is_some() {
        return Err("native Offline Cash protocol unexpectedly has a witness domain".to_owned());
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(OFFLINE_CASH_PROTOCOL_STRUCTURE_DOMAIN_V1);
    bytes.push(0);
    bytes.extend_from_slice(&OFFLINE_CASH_PROTOCOL_STRUCTURE_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(SNARK_VERIFIER_PROTOCOL_REVISION_V1.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&parity_tag(parity).to_le_bytes());
    append_index(&mut bytes, protocol.domain.k, "domain k")?;
    append_index(&mut bytes, protocol.domain.n, "domain n")?;
    append_scalar(&mut bytes, protocol.domain.n_inv)?;
    append_scalar(&mut bytes, protocol.domain.r#gen)?;
    append_scalar(&mut bytes, protocol.domain.gen_inv)?;
    append_len(
        &mut bytes,
        protocol.preprocessed.len(),
        "preprocessed point count",
    )?;
    for (label, values) in [
        ("instance column count", &protocol.num_instance),
        ("witness phase count", &protocol.num_witness),
        ("challenge phase count", &protocol.num_challenge),
    ] {
        append_len(&mut bytes, values.len(), label)?;
        for value in values {
            append_index(&mut bytes, *value, label)?;
        }
    }
    for (label, queries) in [
        ("evaluation query count", &protocol.evaluations),
        ("PCS query count", &protocol.queries),
    ] {
        append_len(&mut bytes, queries.len(), label)?;
        for query in queries {
            append_index(&mut bytes, query.poly, "query polynomial index")?;
            bytes.extend_from_slice(&query.rotation.0.to_le_bytes());
        }
    }
    append_index(
        &mut bytes,
        protocol.quotient.chunk_degree,
        "quotient chunk degree",
    )?;
    let numerator = protocol.quotient.numerator.evaluate(
        &|scalar| {
            let mut encoded = vec![0];
            append_scalar(&mut encoded, scalar)?;
            Ok(encoded)
        },
        &|common| {
            let value = ciborium::value::Value::serialized(&common).map_err(|error| {
                format!("failed to inspect Offline Cash common polynomial: {error}")
            })?;
            encode_common_polynomial(value)
        },
        &|query| {
            let mut encoded = vec![2];
            append_index(&mut encoded, query.poly, "expression polynomial index")?;
            encoded.extend_from_slice(&query.rotation.0.to_le_bytes());
            Ok(encoded)
        },
        &|challenge| {
            let mut encoded = vec![3];
            append_index(&mut encoded, challenge, "expression challenge index")?;
            Ok(encoded)
        },
        &|child| unary_expression(4, child),
        &|left, right| binary_expression(5, left, right),
        &|left, right| binary_expression(6, left, right),
        &|child, scalar| {
            let mut encoded = unary_expression(7, child)?;
            append_scalar(&mut encoded, scalar)?;
            Ok(encoded)
        },
    )?;
    append_len(&mut bytes, numerator.len(), "quotient numerator")?;
    bytes.extend_from_slice(&numerator);
    bytes.push(u8::from(protocol.transcript_initial_state.is_some()));
    match &protocol.instance_committing_key {
        Some(key) => {
            bytes.push(1);
            append_len(
                &mut bytes,
                key.bases.len(),
                "instance committing-key base count",
            )?;
            for base in &key.bases {
                append_point(&mut bytes, *base)?;
            }
            match key.constant {
                Some(constant) => {
                    bytes.push(1);
                    append_point(&mut bytes, constant)?;
                }
                None => bytes.push(0),
            }
        }
        None => bytes.push(0),
    }
    let linearization = ciborium::value::Value::serialized(&protocol.linearization)
        .map_err(|error| format!("failed to inspect Offline Cash linearization: {error}"))?;
    bytes.push(encode_linearization(linearization)?);
    append_len(
        &mut bytes,
        protocol.accumulator_indices.len(),
        "accumulator column count",
    )?;
    for column in &protocol.accumulator_indices {
        append_len(&mut bytes, column.len(), "accumulator index count")?;
        for (column, row) in column {
            append_index(&mut bytes, *column, "accumulator column index")?;
            append_index(&mut bytes, *row, "accumulator row index")?;
        }
    }
    Ok(Sha256::digest(bytes).into())
}

fn compressed_point_poseidon_elements<C>(point: C) -> Result<[C::ScalarExt; 2], String>
where
    C: CurveAffine,
    C::ScalarExt: ff::PrimeField,
{
    if bool::from(point.is_identity()) {
        return Err("Offline Cash compiled protocol point is the identity".to_owned());
    }
    let encoding = point.to_bytes();
    if encoding.as_ref().len() != 32 {
        return Err("Offline Cash compiled protocol point is not 32 bytes".to_owned());
    }
    Ok(std::array::from_fn(|half| {
        C::ScalarExt::from_u128(u128::from_le_bytes(
            encoding.as_ref()[half * 16..(half + 1) * 16]
                .try_into()
                .expect("compressed point half has sixteen bytes"),
        ))
    }))
}

/// Complete same-parity material needed to verify one predecessor and fold its opening claim.
///
/// `protocol` must be witness-loaded and identity-constrained by the enclosing circuit before it
/// reaches [`constrain_parent_and_history_v1`].  This type is private to prevent callers from
/// accidentally treating a host-selected protocol as authenticated.
pub(super) struct OfflineCashDeferredParentWitnessV1<'a, C>
where
    C: CurveAffineExt,
{
    pub(super) instances: &'a [Vec<C::ScalarExt>],
    pub(super) proof_bytes: &'a [u8],
    pub(super) predecessor_history: &'a IpaAccumulator<C, NativeLoader>,
    pub(super) fold_proof_bytes: &'a [u8],
}

/// One ordinary helper proof verified as part of a fixed multi-proof scalar pass.
pub(super) struct OfflineCashDeferredOrdinaryProofWitnessV1<'a, C>
where
    C: CurveAffineExt,
{
    pub(super) instances: &'a [Vec<C::ScalarExt>],
    pub(super) proof_bytes: &'a [u8],
}

/// Scalar-half output that the reciprocal parity must fully constrain.
pub(super) struct OfflineCashDeferredParentOutputV1<C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    pub(super) audit: DeferredEquationWitness<C>,
    pub(super) equation_tags: Vec<u32>,
    pub(super) equation_selectors: Vec<bool>,
    pub(super) audit_digest_limbs: [AssignedValue<C::ScalarExt>; 2],
}

/// Run the complete authenticated scalar predecessor pass against a state-relation builder.
///
/// The fixed 544-byte successor history is appended to the parity proof's public instance column
/// as 34 injective `u128` limbs. The compiled-protocol identity and deferred-equation audit are
/// equality-bound to the common positions already assigned by the state relation.
pub(super) fn constrain_authenticated_scalar_parent_pass_v1<C>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    native_protocol: &PlonkProtocol<C>,
    parity: OfflineCashPastaParityV1,
    fixed_structure_digest: [u8; 32],
    witness: OfflineCashDeferredParentWitnessV1<'_, C>,
    successor_history: &[u8; super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<OfflineCashDeferredParentOutputV1<C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let (protocol_offset, audit_offset) = match parity {
        OfflineCashPastaParityV1::Eq => (
            public_instance::EQ_PROTOCOL_LO,
            public_instance::EQ_DEFERRED_AUDIT_LO,
        ),
        OfflineCashPastaParityV1::Ep => (
            public_instance::EP_PROTOCOL_LO,
            public_instance::EP_DEFERRED_AUDIT_LO,
        ),
    };
    let public = builder
        .assigned_instances
        .first()
        .ok_or(Error::InvalidInstances)?;
    let expected_protocol: [AssignedValue<C::ScalarExt>; 2] = public
        .get(protocol_offset..protocol_offset + 2)
        .ok_or(Error::InvalidInstances)?
        .try_into()
        .map_err(|_| Error::InvalidInstances)?;
    let expected_audit: [AssignedValue<C::ScalarExt>; 2] = public
        .get(audit_offset..audit_offset + 2)
        .ok_or(Error::InvalidInstances)?
        .try_into()
        .map_err(|_| Error::InvalidInstances)?;
    let range = builder.range_chip();
    let history_limbs = successor_history
        .chunks_exact(16)
        .map(|chunk| {
            let value = C::ScalarExt::from_u128(u128::from_le_bytes(
                chunk.try_into().expect("history chunk has sixteen bytes"),
            ));
            let assigned = builder.main(0).load_witness(value);
            range.range_check(builder.main(0), assigned, 128);
            assigned
        })
        .collect::<Vec<_>>();
    if history_limbs.len() != accumulator_limb_count() {
        return Err(Error::InvalidInstances);
    }
    builder
        .assigned_instances
        .first_mut()
        .ok_or(Error::InvalidInstances)?
        .extend(history_limbs.iter().copied());
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(builder, &coordinate, &scalar_integer);
    let output = constrain_authenticated_parent_and_history_v1(
        builder,
        succinct_vk,
        native_protocol,
        parity,
        fixed_structure_digest,
        &expected_protocol,
        witness,
        &history_limbs,
        loader,
    )?;
    for (actual, expected) in output.audit_digest_limbs.iter().zip(expected_audit) {
        builder.main(0).constrain_equal(actual, &expected);
    }
    Ok(output)
}

/// Bind the exact native parent protocol and then verify/fold one predecessor in a single call.
///
/// Keeping protocol authentication and proof parsing adjacent prevents a future composite circuit
/// from accidentally passing a host-selected loaded protocol to the scalar verifier.
pub(super) fn constrain_authenticated_parent_and_history_v1<C>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    native_protocol: &PlonkProtocol<C>,
    parity: OfflineCashPastaParityV1,
    fixed_structure_digest: [u8; 32],
    expected_protocol_limbs: &[AssignedValue<C::ScalarExt>; 2],
    witness: OfflineCashDeferredParentWitnessV1<'_, C>,
    successor_history_limbs: &[AssignedValue<C::ScalarExt>],
    loader: DeferredLoader<'_, C>,
) -> Result<OfflineCashDeferredParentOutputV1<C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let loaded = load_and_constrain_parent_protocol_v1(
        &loader,
        native_protocol,
        parity,
        fixed_structure_digest,
        expected_protocol_limbs,
    )?;
    constrain_parent_and_history_v1(
        builder,
        succinct_vk,
        &loaded.protocol,
        witness,
        successor_history_limbs,
        loader,
    )
}

/// Verify one predecessor proof, fold its current opening claim with its carried history, and
/// equality-bind the folded accumulator to the current proof's public history limbs.
///
/// The `protocol` argument is already loaded through the enclosing circuit's authenticated
/// dynamic-protocol relation.  This function intentionally cannot load an unconstrained native
/// protocol itself.
pub(super) fn constrain_parent_and_history_v1<'chip, C>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    witness: OfflineCashDeferredParentWitnessV1<'_, C>,
    successor_history_limbs: &[AssignedValue<C::ScalarExt>],
    loader: DeferredLoader<'chip, C>,
) -> Result<OfflineCashDeferredParentOutputV1<C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    if witness.instances.len() != protocol.num_instance.len()
        || witness
            .instances
            .iter()
            .zip(&protocol.num_instance)
            .any(|(column, expected)| column.len() != *expected)
        || successor_history_limbs.len() != accumulator_limb_count()
    {
        return Err(Error::InvalidInstances);
    }
    let instances = witness
        .instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| loader.assign_scalar(*value))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let current = verify_ordinary_proof_v1(
        &loader,
        succinct_vk,
        protocol,
        &instances,
        witness.proof_bytes,
    )?;
    let predecessor_history = load_native_accumulator(&loader, witness.predecessor_history)?;
    let folded = verify_fold(
        &loader,
        succinct_vk,
        &[current, predecessor_history],
        witness.fold_proof_bytes,
    )?;
    bind_accumulator_limbs(&loader, &folded, successor_history_limbs)?;

    finalize_deferred_audit_v1(builder, loader)
}

/// Verify/fold one parent into an existing shared loader without finalizing its audit.
///
/// When `enabled` is zero (the bootstrap base case), the parser and scalar verifier keep their
/// fixed shape but the carried successor history is selected directly from the already-valid
/// seed accumulator. The caller must use this same selector for every emitted parent equation in
/// [`finalize_deferred_audit_plan_v1`].
pub(super) fn constrain_parent_and_history_into_loader_v1<'chip, C>(
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    witness: OfflineCashDeferredParentWitnessV1<'_, C>,
    expected_predecessor_state: AssignedValue<C::ScalarExt>,
    expected_predecessor_outer: [AssignedValue<C::ScalarExt>; 2],
    enabled: AssignedValue<C::ScalarExt>,
    loader: &DeferredLoader<'chip, C>,
) -> Result<DeferredAccumulator<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    if witness.instances.len() != protocol.num_instance.len()
        || witness
            .instances
            .iter()
            .zip(&protocol.num_instance)
            .any(|(column, expected)| column.len() != *expected)
    {
        return Err(Error::InvalidInstances);
    }
    let instances = witness
        .instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| loader.assign_scalar(*value))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let current = verify_ordinary_proof_v1(
        loader,
        succinct_vk,
        protocol,
        &instances,
        witness.proof_bytes,
    )?;
    let predecessor_history = load_native_accumulator(loader, witness.predecessor_history)?;
    let parent_column = instances.first().ok_or(Error::InvalidInstances)?;
    let parent_successor = parent_column
        .get(public_instance::SUCCESSOR_STATE)
        .ok_or(Error::InvalidInstances)?;
    loader
        .ctx_mut()
        .main()
        .constrain_equal(&parent_successor.assigned(), &expected_predecessor_state);
    for (parent, expected) in parent_column
        .get(public_instance::SUCCESSOR_OUTER_LO..public_instance::SUCCESSOR_OUTER_HI + 1)
        .ok_or(Error::InvalidInstances)?
        .iter()
        .zip(expected_predecessor_outer)
    {
        loader
            .ctx_mut()
            .main()
            .constrain_equal(&parent.assigned(), &expected);
    }
    let parent_history_limbs = parent_column
        .get(super::state_relation::PUBLIC_INSTANCE_COUNT..)
        .ok_or(Error::InvalidInstances)?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    if parent_history_limbs.len() != accumulator_limb_count() {
        return Err(Error::InvalidInstances);
    }
    bind_accumulator_limbs(loader, &predecessor_history, &parent_history_limbs)?;
    let folded = verify_fold(
        loader,
        succinct_vk,
        &[current, predecessor_history.clone()],
        witness.fold_proof_bytes,
    )?;
    select_accumulator_v1(loader, &folded, &predecessor_history, enabled)
}

/// Select one complete IPA accumulator without dropping a challenge or curve coordinate.
pub(super) fn select_accumulator_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    when_true: &DeferredAccumulator<'chip, C>,
    when_false: &DeferredAccumulator<'chip, C>,
    selector: AssignedValue<C::ScalarExt>,
) -> Result<DeferredAccumulator<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    if when_true.xi.len() != when_false.xi.len() {
        return Err(Error::AssertionFailure(
            "Offline Cash accumulator selector received different round counts".to_owned(),
        ));
    }
    let selected_xi = when_true
        .xi
        .iter()
        .zip(&when_false.xi)
        .map(|(when_true, when_false)| {
            let when_true = *when_true.assigned();
            let when_false = *when_false.assigned();
            let selected = loader.ecc_chip().range().gate().select(
                loader.ctx_mut().main(),
                when_true,
                when_false,
                selector,
            );
            loader.scalar_from_assigned(selected)
        })
        .collect();
    let selected_u = loader.ecc_chip().select_point(
        &mut loader.ctx_mut(),
        &when_true.u.assigned().clone(),
        &when_false.u.assigned().clone(),
        selector,
    );
    Ok(IpaAccumulator::new(
        selected_xi,
        loader.ec_point_from_assigned(selected_u),
    ))
}

/// Snapshot one shared loader after every scalar-verifier proof has been constrained.
///
/// Sibling helper relations use this after verifying all fixed credential/helper proofs through
/// the same loader. A single audit then commits the complete source namespace and every equation,
/// so the reciprocal parity cannot omit one proof selectively.
pub(super) fn finalize_deferred_audit_v1<C>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    loader: DeferredLoader<'_, C>,
) -> Result<OfflineCashDeferredParentOutputV1<C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    finalize_tagged_deferred_audit_v1(builder, loader, OFFLINE_CASH_PARENT_EQUATION_TAG_V1)
}

/// Snapshot a combined scalar-verifier audit with a nonzero circuit-family tag.
pub(super) fn finalize_tagged_deferred_audit_v1<C>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    loader: DeferredLoader<'_, C>,
    equation_tag: u32,
) -> Result<OfflineCashDeferredParentOutputV1<C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    if equation_tag == 0 {
        return Err(Error::InvalidInstances);
    }
    let equation_count = loader.ecc_chip().equation_count();
    let assigned_selectors = {
        let mut ctx = loader.ctx_mut();
        (0..equation_count)
            .map(|_| ctx.main().load_constant(C::ScalarExt::ONE))
            .collect::<Vec<_>>()
    };
    finalize_deferred_audit_plan_v1(
        builder,
        loader,
        vec![equation_tag; equation_count],
        assigned_selectors,
        vec![true; equation_count],
    )
}

/// Snapshot one mixed-role scalar-verifier audit after every proof equation has been emitted.
///
/// The assigned selectors are constrained circuit values. The parallel booleans are their exact
/// witnesses and are replayed by the reciprocal parity when it enforces the curve equations.
pub(super) fn finalize_deferred_audit_plan_v1<C>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    loader: DeferredLoader<'_, C>,
    equation_tags: Vec<u32>,
    assigned_selectors: Vec<AssignedValue<C::ScalarExt>>,
    equation_selectors: Vec<bool>,
) -> Result<OfflineCashDeferredParentOutputV1<C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let equation_count = loader.ecc_chip().equation_count();
    if equation_count == 0
        || equation_tags.len() != equation_count
        || assigned_selectors.len() != equation_count
        || equation_selectors.len() != equation_count
        || equation_tags.iter().any(|tag| *tag == 0)
    {
        return Err(Error::InvalidInstances);
    }
    let (audit, audit_digest_limbs) = {
        let chip = loader.ecc_chip();
        let mut ctx = loader.ctx_mut();
        let elements = chip.assigned_equation_poseidon_elements_v1(
            &mut ctx,
            &equation_tags,
            &assigned_selectors,
        )?;
        drop(ctx);
        drop(chip);
        let digest = poseidon_digest_assigned(&loader, elements);
        let limbs = assigned_scalar_u128_limbs(&loader, digest);
        (loader.ecc_chip().witness(), limbs)
    };
    let output = OfflineCashDeferredParentOutputV1 {
        audit,
        equation_tags,
        equation_selectors,
        audit_digest_limbs,
    };
    *builder.pool(0) = loader.take_ctx();
    Ok(output)
}

const OFFLINE_CASH_PARENT_EQUATION_TAG_V1: u32 = 1;

/// Constrain one scalar-verifier audit in the reciprocal Pasta parity.
///
/// This is the authority-completing half of [`constrain_parent_and_history_v1`]: it assigns every
/// emitted source and coefficient, recomputes the identical field-native Poseidon audit, binds its
/// canonical two-`u128` limbs to the shared public instance, and evaluates the complete batched
/// curve equation through the dedicated dense MSM machine.
pub(super) fn constrain_reciprocal_parent_audit_v1<C>(
    builder: &mut BaseCircuitBuilder<C::Base>,
    witness: &DeferredEquationWitness<C>,
    equation_selectors: &[bool],
    expected_audit_limbs: &[AssignedValue<C::Base>; 2],
    dense_jobs: &mut PastaDenseMsmJobsV1<C>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + halo2_base::utils::ScalarField + ff::WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    constrain_reciprocal_audit_plan_v1(
        builder,
        witness,
        &vec![OFFLINE_CASH_PARENT_EQUATION_TAG_V1; witness.equations.len()],
        equation_selectors,
        expected_audit_limbs,
        dense_jobs,
    )
}

/// Constrain one tagged scalar-verifier audit in the reciprocal Pasta parity.
pub(super) fn constrain_reciprocal_tagged_audit_v1<C>(
    builder: &mut BaseCircuitBuilder<C::Base>,
    witness: &DeferredEquationWitness<C>,
    equation_selectors: &[bool],
    expected_audit_limbs: &[AssignedValue<C::Base>; 2],
    equation_tag: u32,
    dense_jobs: &mut PastaDenseMsmJobsV1<C>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + halo2_base::utils::ScalarField + ff::WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    let equation_tags = vec![equation_tag; witness.equations.len()];
    constrain_reciprocal_audit_plan_v1(
        builder,
        witness,
        &equation_tags,
        equation_selectors,
        expected_audit_limbs,
        dense_jobs,
    )
}

/// Constrain one mixed-role scalar-verifier audit in the reciprocal Pasta parity.
pub(super) fn constrain_reciprocal_audit_plan_v1<C>(
    builder: &mut BaseCircuitBuilder<C::Base>,
    witness: &DeferredEquationWitness<C>,
    equation_tags: &[u32],
    equation_selectors: &[bool],
    expected_audit_limbs: &[AssignedValue<C::Base>; 2],
    dense_jobs: &mut PastaDenseMsmJobsV1<C>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + halo2_base::utils::ScalarField + ff::WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    if witness.equations.len() != equation_selectors.len() || witness.equations.is_empty() {
        return Err("Offline Cash reciprocal deferred-audit selector shape mismatch".to_owned());
    }
    if equation_tags.len() != witness.equations.len() || equation_tags.iter().any(|tag| *tag == 0) {
        return Err("Offline Cash reciprocal deferred-audit tag shape mismatch".to_owned());
    }
    let range = builder.range_chip();
    let base = FpChip::<C::Base, C::Base>::new(&range, LIMB_BITS, LIMBS);
    let scalar = FpChip::<C::Base, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
    let mut chip = PastaCycleEccChip::<C>::new(&base, &scalar);
    let mut ctx = std::mem::take(builder.pool(0));
    let selectors = equation_selectors
        .iter()
        .copied()
        .map(|enabled| ctx.main().load_witness(C::Base::from(u64::from(enabled))))
        .collect::<Vec<_>>();
    let audit = chip.assign_deferred_equations_with_selectors(&mut ctx, witness, &selectors)?;
    let (elements, _) =
        chip.assigned_equation_poseidon_elements_v1(&mut ctx, &audit, equation_tags, &selectors)?;
    let digest = constrain_reciprocal_poseidon_v1::<C>(&mut ctx, &base, &scalar, elements);
    let digest_limbs = chip.assigned_scalar_u128_limbs(&mut ctx, &digest);
    for (actual, expected) in digest_limbs.iter().zip(expected_audit_limbs) {
        ctx.main().constrain_equal(actual, expected);
    }
    chip.constrain_deferred_equation_batch_v1(&mut ctx, &audit, &selectors, &digest, dense_jobs)?;
    *builder.pool(0) = ctx;
    Ok(())
}

/// Consume one opposite-parity scalar pass inside a state-relation builder.
///
/// The expected digest is selected by the proof curve whose equations are being consumed, not by
/// the outer circuit field. This makes Eq equations bind `EQ_DEFERRED_AUDIT_*` in the Ep circuit
/// and Ep equations bind `EP_DEFERRED_AUDIT_*` in the Eq circuit.
pub(super) fn constrain_reciprocal_parent_pass_v1<C>(
    builder: &mut BaseCircuitBuilder<C::Base>,
    parity: OfflineCashPastaParityV1,
    output: &OfflineCashDeferredParentOutputV1<C>,
    dense_jobs: &mut PastaDenseMsmJobsV1<C>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + halo2_base::utils::ScalarField + ff::WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    let offset = match parity {
        OfflineCashPastaParityV1::Eq => public_instance::EQ_DEFERRED_AUDIT_LO,
        OfflineCashPastaParityV1::Ep => public_instance::EP_DEFERRED_AUDIT_LO,
    };
    let expected: [AssignedValue<C::Base>; 2] = builder
        .assigned_instances
        .first()
        .and_then(|public| public.get(offset..offset + 2))
        .ok_or_else(|| "Offline Cash reciprocal public audit is missing".to_owned())?
        .try_into()
        .map_err(|_| "Offline Cash reciprocal public audit has the wrong shape".to_owned())?;
    constrain_reciprocal_audit_plan_v1(
        builder,
        &output.audit,
        &output.equation_tags,
        &output.equation_selectors,
        &expected,
        dense_jobs,
    )
}

fn poseidon_digest_assigned<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    elements: Vec<AssignedValue<C::ScalarExt>>,
) -> AssignedValue<C::ScalarExt>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let elements = elements
        .into_iter()
        .map(|element| loader.scalar_from_assigned(element))
        .collect::<Vec<_>>();
    let mut poseidon = Poseidon::<
        C::ScalarExt,
        DeferredScalar<'chip, C>,
        OFFLINE_CASH_IPA_POSEIDON_WIDTH_V1,
        OFFLINE_CASH_IPA_POSEIDON_RATE_V1,
    >::new::<
        OFFLINE_CASH_IPA_POSEIDON_FULL_ROUNDS_V1,
        OFFLINE_CASH_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
        OFFLINE_CASH_IPA_POSEIDON_SECURE_MDS_V1,
    >(loader);
    poseidon.update(&elements);
    poseidon.squeeze().into_assigned()
}

fn assigned_scalar_u128_limbs<C>(
    loader: &DeferredLoader<'_, C>,
    scalar: AssignedValue<C::ScalarExt>,
) -> [AssignedValue<C::ScalarExt>; 2]
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let chip = loader.ecc_chip();
    let mut ctx = loader.ctx_mut();
    let bits = chip.range().gate().num_to_bits(ctx.main(), scalar, 255);
    let pack = |ctx: &mut halo2_base::Context<C::ScalarExt>,
                bits: &[AssignedValue<C::ScalarExt>]| {
        chip.range().gate().inner_product(
            ctx,
            bits.iter().copied().map(Existing),
            (0..bits.len()).map(|index| Constant(C::ScalarExt::from_u128(1_u128 << index))),
        )
    };
    [
        pack(ctx.main(), &bits[..128]),
        pack(ctx.main(), &bits[128..]),
    ]
}

/// Construct the fixed scalar-half loader over an existing V1 circuit builder.
///
/// The returned field chips must outlive the loader, which is why construction remains a small
/// closure-style building block in the enclosing recursive circuit.
pub(super) fn deferred_loader_v1<'chip, C>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    coordinate: &'chip FpChip<'chip, C::ScalarExt, C::Base>,
    scalar_integer: &'chip FpChip<'chip, C::ScalarExt, C::ScalarExt>,
) -> DeferredLoader<'chip, C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let chip = DeferredScalarEccChip::<C>::new(coordinate, scalar_integer);
    Halo2Loader::new(chip, std::mem::take(builder.pool(0)))
}

/// Construct the fixed coordinate/scalar field chips used by a scalar-half verifier.
pub(super) fn deferred_field_chips_v1<C>(
    range: &halo2_base::gates::RangeChip<C::ScalarExt>,
) -> (
    FpChip<'_, C::ScalarExt, C::Base>,
    FpChip<'_, C::ScalarExt, C::ScalarExt>,
)
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    (
        FpChip::new(range, LIMB_BITS, LIMBS),
        FpChip::new(range, LIMB_BITS, LIMBS),
    )
}

/// Succinctly verify one ordinary proof with an already identity-bound protocol.
///
/// Multiple calls may share one loader; callers must invoke [`finalize_deferred_audit_v1`] once
/// after the last proof and enforce that combined output in the reciprocal parity.
pub(super) fn verify_ordinary_proof_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    instances: &[Vec<DeferredScalar<'chip, C>>],
    proof_bytes: &[u8],
) -> Result<DeferredAccumulator<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    validate_zk_ipa_succinct_key_v1(succinct_vk, OfflineCashIpaProofKindV1::Ordinary)?;
    if succinct_vk.domain.k != protocol.domain.k
        || succinct_vk.domain.k != OFFLINE_CASH_RECURSION_IPA_K_V1 as usize
    {
        return Err(transcript_error(
            "Offline Cash parent protocol requires the fixed zero-knowledge IPA key and domain",
        ));
    }
    let expected_len = ordinary_ipa_proof_profile_v1(protocol)
        .map_err(transcript_error)?
        .byte_len;
    if proof_bytes.len() != expected_len {
        return Err(transcript_error(format!(
            "Offline Cash parent proof has length {}, expected exactly {expected_len}",
            proof_bytes.len()
        )));
    }
    let (reader, position) = ExactReader::new(proof_bytes);
    let mut transcript =
        DeferredTranscript::new::<OFFLINE_CASH_IPA_POSEIDON_SECURE_MDS_V1>(loader, reader);
    let parsed = PlonkSuccinctVerifier::<IpaAs<C, Bgh19>>::read_proof(
        succinct_vk,
        protocol,
        instances,
        &mut transcript,
    )?;
    let mut accumulators = PlonkSuccinctVerifier::<IpaAs<C, Bgh19>>::verify(
        succinct_vk,
        protocol,
        instances,
        &parsed,
    )?;
    if position.get() != proof_bytes.len() {
        return Err(transcript_error(
            "Offline Cash parent proof has trailing bytes",
        ));
    }
    if accumulators.len() != 1 {
        return Err(Error::AssertionFailure(
            "Offline Cash parent verifier did not emit one IPA accumulator".to_owned(),
        ));
    }
    Ok(accumulators.remove(0))
}

/// Verify a fixed non-empty list of ordinary helper proofs through one identity-bound protocol.
///
/// Every proof contributes equations to the same loader. The caller must subsequently invoke
/// [`finalize_deferred_audit_v1`] exactly once so the reciprocal parity constrains the union.
pub(super) fn verify_ordinary_proofs_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    witnesses: &[OfflineCashDeferredOrdinaryProofWitnessV1<'_, C>],
) -> Result<Vec<DeferredAccumulator<'chip, C>>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    if witnesses.is_empty() {
        return Err(Error::InvalidInstances);
    }
    witnesses
        .iter()
        .map(|witness| {
            if witness.instances.len() != protocol.num_instance.len()
                || witness
                    .instances
                    .iter()
                    .zip(&protocol.num_instance)
                    .any(|(column, expected)| column.len() != *expected)
            {
                return Err(Error::InvalidInstances);
            }
            let instances = witness
                .instances
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .map(|value| loader.assign_scalar(*value))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            verify_ordinary_proof_v1(
                loader,
                succinct_vk,
                protocol,
                &instances,
                witness.proof_bytes,
            )
        })
        .collect()
}

pub(super) fn verify_fold<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    inputs: &[DeferredAccumulator<'chip, C>],
    proof_bytes: &[u8],
) -> Result<DeferredAccumulator<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    validate_zk_ipa_succinct_key_v1(succinct_vk, OfflineCashIpaProofKindV1::Fold)?;
    if succinct_vk.domain.k != OFFLINE_CASH_RECURSION_IPA_K_V1 as usize {
        return Err(transcript_error(
            "Offline Cash fold proof requires the fixed zero-knowledge IPA key and domain",
        ));
    }
    if inputs.len() != 2 || proof_bytes.len() != OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1 {
        return Err(transcript_error(
            "Offline Cash BGH19 fold has the wrong input or byte count",
        ));
    }
    let (reader, position) = ExactReader::new(proof_bytes);
    let mut transcript =
        DeferredTranscript::new::<OFFLINE_CASH_IPA_POSEIDON_SECURE_MDS_V1>(loader, reader);
    let parsed = <IpaAs<C, Bgh19> as AccumulationScheme<C, DeferredLoader<'chip, C>>>::read_proof(
        succinct_vk,
        inputs,
        &mut transcript,
    )?;
    let accumulated = <IpaAs<C, Bgh19> as AccumulationScheme<C, DeferredLoader<'chip, C>>>::verify(
        succinct_vk,
        inputs,
        &parsed,
    )?;
    if position.get() != proof_bytes.len() {
        return Err(transcript_error(
            "Offline Cash BGH19 fold has trailing bytes",
        ));
    }
    Ok(accumulated)
}

pub(super) fn load_native_accumulator<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    accumulator: &IpaAccumulator<C, NativeLoader>,
) -> Result<DeferredAccumulator<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    if accumulator.xi.len() != OFFLINE_CASH_RECURSION_IPA_K_V1 as usize {
        return Err(Error::InvalidInstances);
    }
    Ok(IpaAccumulator::new(
        accumulator
            .xi
            .iter()
            .map(|challenge| loader.assign_scalar(*challenge))
            .collect(),
        loader.assign_ec_point(accumulator.u),
    ))
}

pub(super) fn bind_accumulator_limbs<C>(
    loader: &DeferredLoader<'_, C>,
    accumulator: &DeferredAccumulator<'_, C>,
    expected: &[AssignedValue<C::ScalarExt>],
) -> Result<(), Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let challenges = accumulator
        .xi
        .iter()
        .map(|challenge| *challenge.assigned())
        .collect::<Vec<_>>();
    let encoded = loader.ecc_chip().assigned_accumulator_instance_limbs_v1(
        &mut loader.ctx_mut(),
        OFFLINE_CASH_RECURSION_IPA_K_V1,
        &challenges,
        &accumulator.u.assigned(),
    )?;
    // The shared loader's first two limbs are its legacy dynamic version/round tags. Offline Cash
    // fixes both in its authenticated profile, so its canonical public wire contains only the 34
    // scalar/point limbs.
    let encoded = encoded.get(2..).ok_or(Error::InvalidInstances)?;
    if encoded.len() != expected.len() {
        return Err(Error::InvalidInstances);
    }
    for (actual, expected) in encoded.iter().zip(expected) {
        loader.ctx_mut().main().constrain_equal(actual, expected);
    }
    Ok(())
}

pub(super) const fn accumulator_limb_count() -> usize {
    (OFFLINE_CASH_RECURSION_IPA_K_V1 as usize + 1) * 2
}

#[derive(Clone, Debug)]
struct ExactReader<'a> {
    bytes: &'a [u8],
    position: Rc<Cell<usize>>,
}

impl<'a> ExactReader<'a> {
    fn new(bytes: &'a [u8]) -> (Self, Rc<Cell<usize>>) {
        let position = Rc::new(Cell::new(0));
        (
            Self {
                bytes,
                position: Rc::clone(&position),
            },
            position,
        )
    }
}

impl Read for ExactReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let start = self.position.get();
        let available = &self.bytes[start..];
        let len = available.len().min(output.len());
        output[..len].copy_from_slice(&available[..len]);
        self.position.set(start + len);
        Ok(len)
    }
}

fn transcript_error(message: impl Into<String>) -> Error {
    Error::Transcript(io::ErrorKind::InvalidData, message.into())
}

const _: () = {
    assert!(accumulator_limb_count() == 34);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_ipa_profile_matches_measured_credential_transcript() {
        let profile = ordinary_ipa_proof_profile_from_counts_v1(129, 8, 449, 6)
            .expect("bounded credential proof profile");
        assert_eq!(profile.opening_items, 37);
        assert_eq!(profile.byte_len, 20_128);
        assert!(profile.byte_len > super::super::OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1);
    }

    #[test]
    fn ordinary_ipa_profile_rejects_count_overflow() {
        assert!(ordinary_ipa_proof_profile_from_counts_v1(usize::MAX, 1, 0, 0).is_err());
    }

    #[test]
    fn ordinary_ipa_protocol_shape_rejects_panic_prone_inputs() {
        let k = OFFLINE_CASH_RECURSION_IPA_K_V1 as usize;
        assert!(validate_ordinary_ipa_protocol_shape_v1(k, 1, 1, 1, 1).is_ok());
        assert!(validate_ordinary_ipa_protocol_shape_v1(k, 1, 1, 1, 0).is_err());
        assert!(validate_ordinary_ipa_protocol_shape_v1(k, 1, 0, 1, 1).is_err());
        assert!(validate_ordinary_ipa_protocol_shape_v1(k, 1, 1, 0, 1).is_err());
        assert!(validate_ordinary_ipa_protocol_shape_v1(k + 1, 1, 1, 1, 1).is_err());
    }

    #[test]
    fn ordinary_and_fold_admission_reject_non_zk_ipa_key() {
        use halo2_proofs::halo2curves::{
            group::prime::PrimeCurveAffine as _,
            pasta::{EqAffine, Fp},
        };
        use snark_verifier::util::arithmetic::{Domain, root_of_unity};

        let non_zk = IpaSuccinctVerifyingKey::new(
            Domain::new(
                OFFLINE_CASH_RECURSION_IPA_K_V1 as usize,
                root_of_unity::<Fp>(OFFLINE_CASH_RECURSION_IPA_K_V1 as usize),
            ),
            EqAffine::identity(),
            EqAffine::identity(),
            None,
        );
        for kind in [
            OfflineCashIpaProofKindV1::Ordinary,
            OfflineCashIpaProofKindV1::Fold,
        ] {
            let error = validate_zk_ipa_succinct_key_v1(&non_zk, kind)
                .expect_err("non-ZK keys must fail before proof parsing");
            assert!(format!("{error:?}").contains("requires a zero-knowledge IPA key"));
        }
    }

    #[test]
    fn exact_reader_tracks_trailing_bytes() {
        let (mut reader, position) = ExactReader::new(&[1, 2, 3]);
        let mut first = [0_u8; 2];
        assert_eq!(reader.read(&mut first).unwrap(), 2);
        assert_eq!(first, [1, 2]);
        assert_eq!(position.get(), 2);
        let mut rest = [0_u8; 2];
        assert_eq!(reader.read(&mut rest).unwrap(), 1);
        assert_eq!(rest, [3, 0]);
        assert_eq!(position.get(), 3);
    }
}
