//! Complete ordinary proof transcripts using the existing native Poseidon queue.

use super::*;
use crate::zk::pasta_native_poseidon::PastaNativePoseidonTranscriptV1;
use snark_verifier::{
    system::halo2::transcript::halo2::NativeEncoding as _,
    util::transcript::{Transcript, TranscriptRead},
};

/// Closing a transcript must retain its exact proof-read cells and every assigned challenge.
pub(super) trait CompleteProofTranscriptV1<'chip, C>:
    TranscriptRead<C, DeferredLoader<'chip, C>>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    /// Finish all reserved constraints and return the original proof-read stream.
    fn finish_stream(self) -> Result<DeferredProofStreamV1<'chip, C>, Error>;
}

impl<'chip, C, R> CompleteProofTranscriptV1<'chip, C> for DeferredTranscript<'chip, C, R>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
    R: Read,
{
    fn finish_stream(self) -> Result<DeferredProofStreamV1<'chip, C>, Error> {
        Ok(self.loaded_stream)
    }
}

/// Native transcript selected before witness assignment from authenticated protocol geometry.
pub(super) struct NativeOrdinaryTranscriptV1<'jobs, 'chip, C, R>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    loader: DeferredLoader<'chip, C>,
    stream: R,
    loaded_stream: DeferredProofStreamV1<'chip, C>,
    sponge: PastaNativePoseidonTranscriptV1<'jobs, C::ScalarExt>,
}

impl<'jobs, 'chip, C, R> NativeOrdinaryTranscriptV1<'jobs, 'chip, C, R>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
    R: Read,
{
    /// Reserve the complete authenticated schedule without changing the containing lane count.
    pub(super) fn new(
        loader: &DeferredLoader<'chip, C>,
        stream: R,
        jobs: &'jobs mut PastaNativePoseidonJobsV1<C::ScalarExt>,
        schedule: &[usize],
    ) -> Result<Self, Error> {
        let sponge = jobs
            .begin_transcript(loader.ctx_mut().main(), schedule)
            .map_err(transcript_error)?;
        Ok(Self {
            loader: loader.clone(),
            stream,
            loaded_stream: Vec::new(),
            sponge,
        })
    }
}

impl<'chip, C, R> Transcript<C, DeferredLoader<'chip, C>>
    for NativeOrdinaryTranscriptV1<'_, 'chip, C, R>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
    R: Read,
{
    fn loader(&self) -> &DeferredLoader<'chip, C> {
        &self.loader
    }

    fn squeeze_challenge(&mut self) -> DeferredScalar<'chip, C> {
        let chip = self.loader.ecc_chip();
        let mut ctx = self.loader.ctx_mut();
        let output = self.sponge.squeeze(ctx.main(), chip.range().gate());
        self.loader.scalar_from_assigned(output)
    }

    fn common_scalar(&mut self, scalar: &DeferredScalar<'chip, C>) -> Result<(), Error> {
        self.sponge
            .absorb(&[*scalar.assigned()])
            .map_err(transcript_error)
    }

    fn common_ec_point(&mut self, point: &DeferredEcPoint<'chip, C>) -> Result<(), Error> {
        let encoded = self
            .loader
            .ecc_chip()
            .encode(&mut self.loader.ctx_mut(), &point.assigned())
            .map_err(|_| {
                Error::Transcript(
                    io::ErrorKind::Other,
                    "Failed to encode elliptic curve point into native field elements".to_owned(),
                )
            })?;
        self.sponge.absorb(&encoded).map_err(transcript_error)
    }
}

impl<'chip, C, R> TranscriptRead<C, DeferredLoader<'chip, C>>
    for NativeOrdinaryTranscriptV1<'_, 'chip, C, R>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
    R: Read,
{
    fn read_scalar(&mut self) -> Result<DeferredScalar<'chip, C>, Error> {
        let mut data = <C::ScalarExt as ff::PrimeField>::Repr::default();
        self.stream
            .read_exact(data.as_mut())
            .map_err(|error| Error::Transcript(error.kind(), error.to_string()))?;
        let scalar = C::ScalarExt::from_repr_vartime(data).ok_or_else(|| {
            Error::Transcript(
                io::ErrorKind::Other,
                "Invalid scalar encoding in proof".to_owned(),
            )
        })?;
        let scalar = self.loader.assign_scalar(scalar);
        self.loaded_stream
            .push(TranscriptObject::Scalar(scalar.clone()));
        self.common_scalar(&scalar)?;
        Ok(scalar)
    }

    fn read_ec_point(&mut self) -> Result<DeferredEcPoint<'chip, C>, Error> {
        let mut compressed = C::Repr::default();
        self.stream
            .read_exact(compressed.as_mut())
            .map_err(|error| Error::Transcript(error.kind(), error.to_string()))?;
        let point = Option::<C>::from(C::from_bytes(&compressed)).ok_or_else(|| {
            Error::Transcript(
                io::ErrorKind::Other,
                "Invalid elliptic curve point encoding in proof".to_owned(),
            )
        })?;
        let point = self.loader.assign_ec_point(point);
        self.loaded_stream
            .push(TranscriptObject::EcPoint(point.clone()));
        self.common_ec_point(&point)?;
        Ok(point)
    }
}

impl<'chip, C, R> CompleteProofTranscriptV1<'chip, C>
    for NativeOrdinaryTranscriptV1<'_, 'chip, C, R>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
    R: Read,
{
    fn finish_stream(self) -> Result<DeferredProofStreamV1<'chip, C>, Error> {
        // Parsing failure or a mismatched schedule leaves the queue unfinished and unusable.
        // There is no witness-selected fallback after any native challenge is assigned.
        self.sponge.finish().map_err(transcript_error)?;
        Ok(self.loaded_stream)
    }
}

#[cfg(test)]
#[path = "ordinary_proof_transcript_tests.rs"]
mod tests;
