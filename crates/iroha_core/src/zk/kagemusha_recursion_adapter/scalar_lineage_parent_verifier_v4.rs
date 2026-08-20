// Production V4 parent-proof parsing and the shared BGH19 lineage fold.

#[cfg(not(feature = "kagemusha-generation-memory-lab"))]
fn verify_ordinary_parent<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    instances: &[Vec<DeferredLoadedScalar<'chip, C>>],
    proof_bytes: &[u8],
    max_proof_bytes: usize,
) -> Result<DeferredAccumulator<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    if max_proof_bytes == 0 || proof_bytes.is_empty() || proof_bytes.len() > max_proof_bytes {
        return Err(transcript_error(
            "Kagemusha parent proof violates the fixed proof slot",
        ));
    }
    let (reader, position) = ExactReader::new(proof_bytes);
    let mut transcript = DeferredTranscript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(loader, reader);
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
            "Kagemusha parent proof has trailing bytes",
        ));
    }
    if accumulators.len() != 1 {
        return Err(Error::AssertionFailure(
            "Kagemusha fixed parent verifier did not emit one IPA accumulator".to_owned(),
        ));
    }
    Ok(accumulators.remove(0))
}

fn verify_fold<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    inputs: &[DeferredAccumulator<'chip, C>],
    proof_bytes: &[u8],
    expected_proof_bytes: usize,
) -> Result<DeferredAccumulator<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    if inputs.len() < 2 || expected_proof_bytes == 0 || proof_bytes.len() != expected_proof_bytes {
        return Err(transcript_error(
            "Kagemusha BGH19 fold has the wrong input or byte count",
        ));
    }
    let (reader, position) = ExactReader::new(proof_bytes);
    let mut transcript = DeferredTranscript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(loader, reader);
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
        return Err(transcript_error("Kagemusha BGH19 fold has trailing bytes"));
    }
    Ok(accumulated)
}
