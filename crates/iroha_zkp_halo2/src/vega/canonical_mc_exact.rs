//! Exact-lock Microsoft Vega-MC boundary for the released Figure 9 relation.
//!
//! The public facade, envelope, governed profile identity, and canonical wire remain fixed while
//! the upstream implementation is internalized. Verification uses only the crate-owned codec and
//! equations after an exact governed verifier-key artifact is installed. Proving-key artifacts
//! are also decoded and paired exactly. The native Figure 9 split and historical SHA step-rest
//! allocation, all nine application commitments, and the semantic 47-round Microsoft engine are
//! prepared exactly, including the fresh random verifier instance, its Nova fold, and the complete
//! relaxed-Spartan tail, streamed final Hyrax binding, and linear IPA. A proof is returned only
//! after canonical re-encoding and a complete first-party verifier replay under the governed key.
use super::{
    MAX_VEGA_PROOF_BYTES_V1, VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1, VegaT256ScalarV1 as Scalar,
    engine::{
        VEGA_MDL_ACTION_INDEX_V1, VegaMdlProofContextV1, VegaMdlProofDimensionsV1,
        VegaMdlProofErrorV1, VegaMdlProverConfigV1, VegaRandomSourceV1,
    },
    figure9::{VegaMdlFigure9WitnessV1, synthesize_figure9_mc_material},
    microsoft_mc,
    sponge::keccak256,
};
const ENVELOPE_MAGIC: &[u8; 8] = b"IROVEGMC";
const ENVELOPE_VERSION: u8 = 1;
const ENVELOPE_HEADER_BYTES: usize = ENVELOPE_MAGIC.len() + 1 + 32;
const CONTEXT_DOMAIN: &[u8] = b"iroha.vega.figure9.microsoft-mc.context.v1";
const PINNED_SOURCE_COMMIT: &[u8] = b"c0ee259053cd12eaf43ed71b5cde375452b3ee4d";
const CONTEXT_PUBLIC_SCALARS: usize = 4;

/// Install one exact governed Figure 9 verifier-key artifact.
pub(super) fn install_figure9_verifier_key(verifier_key: &[u8]) -> Result<(), VegaMdlProofErrorV1> {
    microsoft_mc::install_governed_figure9_verifier_key(verifier_key)
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)
}

/// Install one exact governed Microsoft proving-key/verifier-key pair.
pub(super) fn install_figure9_prover_artifacts(
    proving_key: &[u8],
    verifier_key: &[u8],
) -> Result<(), VegaMdlProofErrorV1> {
    microsoft_mc::install_governed_figure9_prover_artifacts(proving_key, verifier_key)
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)
}

/// Produce one canonical, governed, fully self-verified Figure 9 proof.
pub(super) fn prove_figure9_mc<R: VegaRandomSourceV1>(
    context: &VegaMdlProofContextV1<'_>,
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    witness: &VegaMdlFigure9WitnessV1<'_>,
    config: VegaMdlProverConfigV1,
    random: &mut R,
) -> Result<Vec<u8>, VegaMdlProofErrorV1> {
    let context_digest = validate_context(context)?;
    microsoft_mc::preflight_governed_figure9_prover_artifacts()
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;

    // Synthesis checks the complete native Figure 9 relation only after the
    // governed artifacts have passed their exact pairing checks, and before
    // the caller's random source can be touched.
    let material = synthesize_figure9_mc_material(public_inputs, witness)
        .map_err(|_| VegaMdlProofErrorV1::UnsatisfiedWitness)?;
    let (step_public, core_public) =
        canonical_microsoft_public_values(public_inputs, context_digest);
    let dimensions = microsoft_mc::canonical_figure9_dimensions();
    if material.assignment.public_inputs.as_slice() != public_inputs.as_slice()
        || step_public.len() != dimensions.num_steps
        || step_public
            .iter()
            .any(|values| values.len() != dimensions.step_public_values)
        || core_public.len() != dimensions.core_public_values
    {
        return Err(VegaMdlProofErrorV1::InvalidCompiledProfile);
    }
    let raw_proof = microsoft_mc::prepare_governed_figure9_application(
        &material,
        &step_public,
        &core_public,
        config.worker_count(),
        random,
    )
    .map_err(|error| match error {
        microsoft_mc::Figure9ApplicationPrepError::RandomSource(error) => {
            VegaMdlProofErrorV1::RandomSource(error)
        }
        microsoft_mc::Figure9ApplicationPrepError::DegenerateRandomness => {
            VegaMdlProofErrorV1::DegenerateRandomness
        }
        microsoft_mc::Figure9ApplicationPrepError::Split(_)
        | microsoft_mc::Figure9ApplicationPrepError::InvalidGovernedKey
        | microsoft_mc::Figure9ApplicationPrepError::Commitment
        | microsoft_mc::Figure9ApplicationPrepError::Transcript
        | microsoft_mc::Figure9ApplicationPrepError::Semantic(_)
        | microsoft_mc::Figure9ApplicationPrepError::RandomNova(_)
        | microsoft_mc::Figure9ApplicationPrepError::RelaxedSpartan(_)
        | microsoft_mc::Figure9ApplicationPrepError::FinalOpening(_) => {
            VegaMdlProofErrorV1::InvalidCompiledProfile
        }
    })?;

    let envelope_len = ENVELOPE_HEADER_BYTES
        .checked_add(raw_proof.len())
        .ok_or(VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    if envelope_len > MAX_VEGA_PROOF_BYTES_V1 {
        return Err(VegaMdlProofErrorV1::ProofTooLarge {
            actual: envelope_len,
            max: MAX_VEGA_PROOF_BYTES_V1,
        });
    }
    let mut envelope = Vec::new();
    envelope
        .try_reserve_exact(envelope_len)
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    envelope.extend_from_slice(ENVELOPE_MAGIC);
    envelope.push(ENVELOPE_VERSION);
    envelope.extend_from_slice(&context_digest);
    envelope.extend_from_slice(&raw_proof);

    // Keep proof authority at the exact installed VK boundary: emission is
    // impossible unless the public API verifier accepts this same envelope.
    verify_figure9_mc(context, public_inputs, &envelope)?;
    Ok(envelope)
}
/// Parse the fixed envelope and run every first-party Microsoft equation.
pub(super) fn verify_figure9_mc(
    context: &VegaMdlProofContextV1<'_>,
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    envelope: &[u8],
) -> Result<(), VegaMdlProofErrorV1> {
    if envelope.len() > MAX_VEGA_PROOF_BYTES_V1 {
        return Err(VegaMdlProofErrorV1::ProofTooLarge {
            actual: envelope.len(),
            max: MAX_VEGA_PROOF_BYTES_V1,
        });
    }
    if envelope.len() < ENVELOPE_HEADER_BYTES
        || &envelope[..ENVELOPE_MAGIC.len()] != ENVELOPE_MAGIC
        || envelope[ENVELOPE_MAGIC.len()] != ENVELOPE_VERSION
    {
        return Err(VegaMdlProofErrorV1::InvalidProofEncoding);
    }
    let expected_context_digest = validate_context(context)?;
    if envelope[ENVELOPE_MAGIC.len() + 1..ENVELOPE_HEADER_BYTES] != expected_context_digest {
        return Err(VegaMdlProofErrorV1::VerificationFailed);
    }
    microsoft_mc::scan_canonical_figure9_proof(&envelope[ENVELOPE_HEADER_BYTES..])
        .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)?;
    let (step_public, core_public) = microsoft_mc::verify_governed_figure9_proof(
        &envelope[ENVELOPE_HEADER_BYTES..],
    )
    .map_err(|error| match error {
        microsoft_mc::Figure9VerificationError::MissingGovernedVerifierKey => {
            VegaMdlProofErrorV1::InvalidCompiledProfile
        }
        microsoft_mc::Figure9VerificationError::InvalidProofEncoding => {
            VegaMdlProofErrorV1::InvalidProofEncoding
        }
        microsoft_mc::Figure9VerificationError::VerificationFailed => {
            VegaMdlProofErrorV1::VerificationFailed
        }
    })?;
    let (expected_steps, expected_core) =
        canonical_microsoft_public_values(public_inputs, expected_context_digest);
    if step_public != expected_steps || core_public != expected_core {
        return Err(VegaMdlProofErrorV1::VerificationFailed);
    }
    Ok(())
}
/// Return the governed Microsoft verifier-key digest.
pub(super) fn verifier_digest() -> Result<[u8; 32], VegaMdlProofErrorV1> {
    Ok(microsoft_mc::canonical_figure9_verifier_digest())
}
/// Return every governed Microsoft proof sequence dimension.
pub(super) fn proof_dimensions() -> Result<VegaMdlProofDimensionsV1, VegaMdlProofErrorV1> {
    Ok(microsoft_mc::canonical_figure9_dimensions())
}
/// Validate an independently generated Microsoft fixture with first-party code.
pub(super) fn validate_microsoft_fixture(
    verifier_key: &[u8],
    proof: &[u8],
) -> Result<([u8; 32], VegaMdlProofDimensionsV1, usize, usize), VegaMdlProofErrorV1> {
    let (digest, dimensions, steps, core) = microsoft_mc::validate_fixture(verifier_key, proof)
        .map_err(|_| VegaMdlProofErrorV1::VerificationFailed)?;
    Ok((digest, dimensions, steps.len(), core.len()))
}
fn validate_context(context: &VegaMdlProofContextV1<'_>) -> Result<[u8; 32], VegaMdlProofErrorV1> {
    if context.action_index != VEGA_MDL_ACTION_INDEX_V1
        || context.chain_id.is_empty()
        || context.chain_id.len() > u8::MAX.into()
        || context.verifier_digest != microsoft_mc::canonical_figure9_verifier_digest()
        || [
            context.genesis_hash,
            context.parameter_id,
            context.parameter_digest,
            context.verifier_digest,
            context.statement_schema_digest,
            context.engine_manifest_digest,
        ]
        .contains(&[0; 32])
    {
        return Err(VegaMdlProofErrorV1::InvalidContext);
    }
    let mut frame = Vec::with_capacity(320);
    push_context_field(&mut frame, CONTEXT_DOMAIN)?;
    push_context_field(&mut frame, PINNED_SOURCE_COMMIT)?;
    push_context_field(&mut frame, context.chain_id)?;
    push_context_field(&mut frame, &context.genesis_hash)?;
    push_context_field(&mut frame, &context.action_index.to_le_bytes())?;
    push_context_field(&mut frame, &context.parameter_id)?;
    push_context_field(&mut frame, &context.parameter_digest)?;
    push_context_field(&mut frame, &context.verifier_digest)?;
    push_context_field(&mut frame, &context.statement_schema_digest)?;
    push_context_field(&mut frame, &context.engine_manifest_digest)?;
    Ok(keccak256(&frame))
}
fn push_context_field(output: &mut Vec<u8>, field: &[u8]) -> Result<(), VegaMdlProofErrorV1> {
    output.extend_from_slice(
        &u64::try_from(field.len())
            .map_err(|_| VegaMdlProofErrorV1::InvalidContext)?
            .to_le_bytes(),
    );
    output.extend_from_slice(field);
    Ok(())
}

fn context_public_values(digest: [u8; 32]) -> [Scalar; CONTEXT_PUBLIC_SCALARS] {
    core::array::from_fn(|index| {
        Scalar::from_u64(u64::from_le_bytes(
            digest[index * 8..(index + 1) * 8]
                .try_into()
                .expect("fixed context digest chunk"),
        ))
    })
}

fn canonical_microsoft_public_values(
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    context_digest: [u8; 32],
) -> (Vec<Vec<Scalar>>, Vec<Scalar>) {
    let dimensions = microsoft_mc::canonical_figure9_dimensions();
    let step_public = (0..dimensions.num_steps)
        .map(|index| vec![Scalar::from_u64(index as u64)])
        .collect();
    let mut core_public =
        Vec::with_capacity(VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1 + CONTEXT_PUBLIC_SCALARS);
    core_public.extend_from_slice(public_inputs);
    core_public.extend_from_slice(&context_public_values(context_digest));
    (step_public, core_public)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::{
        engine::{VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1, VegaRandomSourceErrorV1},
        figure9_layout::FIGURE9_LAYOUT,
    };
    const PYTHON_VK: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/vega-prover/reference/fixtures/cubic/python_vk.bin"
    ));

    struct PanicRandom;
    impl VegaRandomSourceV1 for PanicRandom {
        fn fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), VegaRandomSourceErrorV1> {
            panic!("disabled prover must not request randomness")
        }
    }
    fn context() -> VegaMdlProofContextV1<'static> {
        VegaMdlProofContextV1 {
            chain_id: b"fail-closed-test",
            genesis_hash: [1; 32],
            action_index: VEGA_MDL_ACTION_INDEX_V1,
            parameter_id: [2; 32],
            parameter_digest: [3; 32],
            verifier_digest: VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1,
            statement_schema_digest: [4; 32],
            engine_manifest_digest: [5; 32],
        }
    }
    #[test]
    fn production_prover_rejects_missing_governed_artifacts_before_rng() {
        let one = [1_u8; 32];
        let witness = VegaMdlFigure9WitnessV1::new(
            &FIGURE9_LAYOUT.issuer_template,
            &FIGURE9_LAYOUT.birth_template,
            &one,
            &one,
            &one,
            &one,
        )
        .expect("well-formed witness container");
        let mut random = PanicRandom;
        assert_eq!(
            prove_figure9_mc(
                &context(),
                &[Scalar::zero(); VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
                &witness,
                VegaMdlProverConfigV1::new(1).expect("released worker count"),
                &mut random,
            ),
            Err(VegaMdlProofErrorV1::InvalidCompiledProfile)
        );
    }
    #[test]
    fn production_verifier_accepts_no_placeholder_or_alternate_wire() {
        let context = context();
        let context_digest = validate_context(&context).expect("valid context");
        let mut alternate = Vec::from(ENVELOPE_MAGIC);
        alternate.push(ENVELOPE_VERSION);
        alternate.extend_from_slice(&context_digest);
        alternate.extend_from_slice(b"alternate-proof-wire");
        assert!(
            verify_figure9_mc(
                &context,
                &[Scalar::zero(); VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
                &alternate,
            )
            .is_err()
        );
        assert_eq!(
            verify_figure9_mc(
                &context,
                &[Scalar::zero(); VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
                b"not-an-envelope",
            ),
            Err(VegaMdlProofErrorV1::InvalidProofEncoding)
        );
    }

    #[test]
    fn governed_key_install_rejects_malformed_and_wrong_profile_artifacts() {
        assert_eq!(
            install_figure9_verifier_key(&PYTHON_VK[..PYTHON_VK.len() - 1]),
            Err(VegaMdlProofErrorV1::InvalidCompiledProfile)
        );
        assert_eq!(
            install_figure9_verifier_key(PYTHON_VK),
            Err(VegaMdlProofErrorV1::InvalidCompiledProfile)
        );
        let mut trailing = PYTHON_VK.to_vec();
        trailing.push(0);
        assert_eq!(
            install_figure9_verifier_key(&trailing),
            Err(VegaMdlProofErrorV1::InvalidCompiledProfile)
        );
        assert_eq!(
            install_figure9_prover_artifacts(&[], PYTHON_VK),
            Err(VegaMdlProofErrorV1::InvalidCompiledProfile)
        );
    }

    #[test]
    fn envelope_parser_rejects_header_context_and_size_corruption_strictly() {
        let context = context();
        let digest = validate_context(&context).expect("valid context");
        let mut header = Vec::from(ENVELOPE_MAGIC);
        header.push(ENVELOPE_VERSION);
        header.extend_from_slice(&digest);
        assert_eq!(
            verify_figure9_mc(
                &context,
                &[Scalar::zero(); VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
                &header,
            ),
            Err(VegaMdlProofErrorV1::InvalidProofEncoding)
        );

        let mut wrong_magic = header.clone();
        wrong_magic[0] ^= 1;
        let mut wrong_version = header.clone();
        wrong_version[ENVELOPE_MAGIC.len()] = ENVELOPE_VERSION + 1;
        for malformed in [
            &header[..ENVELOPE_HEADER_BYTES - 1],
            &wrong_magic,
            &wrong_version,
        ] {
            assert_eq!(
                verify_figure9_mc(
                    &context,
                    &[Scalar::zero(); VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
                    malformed,
                ),
                Err(VegaMdlProofErrorV1::InvalidProofEncoding)
            );
        }

        let mut wrong_context = header;
        wrong_context[ENVELOPE_MAGIC.len() + 1] ^= 1;
        assert_eq!(
            verify_figure9_mc(
                &context,
                &[Scalar::zero(); VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
                &wrong_context,
            ),
            Err(VegaMdlProofErrorV1::VerificationFailed)
        );

        let oversized = vec![0_u8; MAX_VEGA_PROOF_BYTES_V1 + 1];
        assert_eq!(
            verify_figure9_mc(
                &context,
                &[Scalar::zero(); VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
                &oversized,
            ),
            Err(VegaMdlProofErrorV1::ProofTooLarge {
                actual: MAX_VEGA_PROOF_BYTES_V1 + 1,
                max: MAX_VEGA_PROOF_BYTES_V1,
            })
        );
    }

    #[test]
    fn context_frame_rejects_invalid_fields_and_binds_every_admitted_field() {
        let baseline = context();
        let baseline_digest = validate_context(&baseline).expect("valid context");
        let mut changed_contexts = Vec::new();
        let mut changed = baseline;
        changed.chain_id = b"another-chain";
        changed_contexts.push(changed);
        let mut changed = baseline;
        changed.genesis_hash[0] ^= 1;
        changed_contexts.push(changed);
        let mut changed = baseline;
        changed.parameter_id[0] ^= 1;
        changed_contexts.push(changed);
        let mut changed = baseline;
        changed.parameter_digest[0] ^= 1;
        changed_contexts.push(changed);
        let mut changed = baseline;
        changed.statement_schema_digest[0] ^= 1;
        changed_contexts.push(changed);
        let mut changed = baseline;
        changed.engine_manifest_digest[0] ^= 1;
        changed_contexts.push(changed);
        assert!(changed_contexts.iter().all(|changed| {
            validate_context(changed).expect("admitted changed context") != baseline_digest
        }));

        let mut wrong_action = baseline;
        wrong_action.action_index += 1;
        assert_eq!(
            validate_context(&wrong_action),
            Err(VegaMdlProofErrorV1::InvalidContext)
        );
        let mut wrong_key = baseline;
        wrong_key.verifier_digest[0] ^= 1;
        assert_eq!(
            validate_context(&wrong_key),
            Err(VegaMdlProofErrorV1::InvalidContext)
        );
        let mut zero_field = baseline;
        zero_field.parameter_digest = [0; 32];
        assert_eq!(
            validate_context(&zero_field),
            Err(VegaMdlProofErrorV1::InvalidContext)
        );
        let empty_chain = VegaMdlProofContextV1 {
            chain_id: b"",
            ..baseline
        };
        assert_eq!(
            validate_context(&empty_chain),
            Err(VegaMdlProofErrorV1::InvalidContext)
        );
        let overlong_chain = vec![0x41; usize::from(u8::MAX) + 1];
        let overlong = VegaMdlProofContextV1 {
            chain_id: &overlong_chain,
            ..baseline
        };
        assert_eq!(
            validate_context(&overlong),
            Err(VegaMdlProofErrorV1::InvalidContext)
        );
    }

    #[test]
    fn context_public_scalars_are_the_four_little_endian_digest_limbs() {
        let digest = core::array::from_fn(|index| index as u8);
        let scalars = context_public_values(digest);
        for (index, scalar) in scalars.iter().copied().enumerate() {
            assert_eq!(
                scalar,
                Scalar::from_u64(u64::from_le_bytes(
                    digest[index * 8..(index + 1) * 8]
                        .try_into()
                        .expect("fixed digest limb"),
                ))
            );
        }
    }

    #[test]
    fn microsoft_public_schedule_is_exactly_eight_indices_and_bound_core_context() {
        let public_inputs = core::array::from_fn(|index| Scalar::from_u64(index as u64 + 11));
        let digest = core::array::from_fn(|index| index as u8);
        let (steps, core) = canonical_microsoft_public_values(&public_inputs, digest);
        assert_eq!(steps.len(), 8);
        for (index, values) in steps.iter().enumerate() {
            assert_eq!(values, &[Scalar::from_u64(index as u64)]);
        }
        assert_eq!(
            &core[..VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
            public_inputs.as_slice()
        );
        assert_eq!(
            &core[VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1..],
            &context_public_values(digest)
        );
    }
}
