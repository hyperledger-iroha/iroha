fn verify_ordinary_parent<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    instances: &[Vec<DeferredLoadedScalar<'chip, C>>],
    proof_bytes: &[u8],
    max_proof_bytes: usize,
    serialized_phase_zero_rank: Option<usize>,
) -> Result<
    (
        DeferredAccumulator<'chip, C>,
        Option<super::super::kagemusha_cycle_loader::DeferredScalarPoint<C>>,
    ),
    Error,
>
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
    let serialized_advice_commitment = serialized_phase_zero_rank
        .map(|rank| {
            let phase_zero_witnesses = protocol.num_witness.first().copied().unwrap_or(0);
            if rank >= phase_zero_witnesses {
                return Err(Error::InvalidInstances);
            }
            parsed
                .witnesses
                .get(rank)
                .cloned()
                .map(|point| point.into_assigned())
                .ok_or(Error::InvalidInstances)
        })
        .transpose()?;
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
    Ok((accumulators.remove(0), serialized_advice_commitment))
}
