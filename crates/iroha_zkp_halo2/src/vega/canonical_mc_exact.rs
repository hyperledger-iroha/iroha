//! Exact-lock Microsoft Vega-MC boundary for the released Figure 9 relation.
//!
//! The public facade, envelope, governed profile identity, and canonical wire
//! remain fixed while the upstream implementation is internalized. The
//! production Figure 9 path is deliberately fail-closed until its exact
//! verifier key, setup, and prover are all first-party code.

use super::{
    MAX_VEGA_PROOF_BYTES_V1, VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1, VegaT256ScalarV1 as Scalar,
    engine::{
        VEGA_MDL_ACTION_INDEX_V1, VegaMdlProofContextV1, VegaMdlProofDimensionsV1,
        VegaMdlProofErrorV1, VegaMdlProverConfigV1, VegaRandomSourceV1,
    },
    figure9::VegaMdlFigure9WitnessV1,
    microsoft_mc,
    sponge::keccak256,
};

const ENVELOPE_MAGIC: &[u8; 8] = b"IROVEGMC";
const ENVELOPE_VERSION: u8 = 1;
const ENVELOPE_HEADER_BYTES: usize = ENVELOPE_MAGIC.len() + 1 + 32;
const CONTEXT_DOMAIN: &[u8] = b"iroha.vega.figure9.microsoft-mc.context.v1";
const PINNED_SOURCE_COMMIT: &[u8] = b"c0ee259053cd12eaf43ed71b5cde375452b3ee4d";
const FIGURE9_MC_ENGINE_READY: bool = false;

/// Reject proving until the exact Microsoft Figure 9 prover is internalized.
pub(super) fn prove_figure9_mc<R: VegaRandomSourceV1>(
    context: &VegaMdlProofContextV1<'_>,
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    witness: &VegaMdlFigure9WitnessV1<'_>,
    config: VegaMdlProverConfigV1,
    random: &mut R,
) -> Result<Vec<u8>, VegaMdlProofErrorV1> {
    validate_context(context)?;
    let _ = (public_inputs, witness, config, random);
    debug_assert!(!FIGURE9_MC_ENGINE_READY);
    // TODO: Port the exact first-party Figure 9 verifier key/setup/prover and
    // remove this gate only after canonical Microsoft proof bytes, constraint
    // order, and the pinned verifier digest pass cross-conformance tests.
    Err(VegaMdlProofErrorV1::InvalidCompiledProfile)
}

/// Parse the fixed envelope and reject until equation verification is enabled.
pub(super) fn verify_figure9_mc(
    context: &VegaMdlProofContextV1<'_>,
    _public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
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
    debug_assert!(!FIGURE9_MC_ENGINE_READY);
    // A structurally canonical proof is not accepted without the exact key and
    // every Microsoft equation check. This is intentionally fail-closed.
    Err(VegaMdlProofErrorV1::VerificationFailed)
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
        .iter()
        .any(|digest| *digest == [0; 32])
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::{
        engine::{VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1, VegaRandomSourceErrorV1},
        figure9_layout::FIGURE9_LAYOUT,
    };

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
    fn production_prover_is_explicitly_fail_closed() {
        assert!(!FIGURE9_MC_ENGINE_READY);
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
}
