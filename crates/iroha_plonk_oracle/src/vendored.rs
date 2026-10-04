//! Oracle access to vendored behaviour that has no public entry point.
//!
//! - [`generator_collapse`] reproduces the crate-private IPA generator fold
//!   `parallel_generator_collapse` of
//!   `vendor/halo2-axiom/src/poly/ipa/commitment/prover.rs`, statement for
//!   statement, over the public vendored API (`parallelize`, the
//!   `halo2curves` group law and `batch_normalize`). The fold parity tests
//!   additionally drive the real vendored IPA prover and verifier, so the
//!   reproduction is not the only witness of the vendored behaviour.
//! - [`Recording`] wraps any vendored transcript and records every absorbed
//!   element, every proof element and every challenge, so tests can recover
//!   round challenges from real vendored proofs and diff transcripts event by
//!   event.

use std::io;

use halo2_axiom::{
    arithmetic::{CurveAffine, parallelize},
    halo2curves::group::Curve,
    transcript::{EncodedChallenge, Transcript, TranscriptRead, TranscriptWrite},
};

/// Folds the active generator halves: for `h = g.len() / 2`,
/// `g[i] <- g[i] + challenge * g[i + h]` for `i < h`, normalised to affine.
///
/// This is the body of the vendored `parallel_generator_collapse`; the caller
/// truncates `g` to `h` afterwards, as the vendored prover does.
// The statements are kept as the vendored source writes them.
#[allow(clippy::op_ref)]
pub fn generator_collapse<C: CurveAffine>(g: &mut [C], challenge: C::Scalar) {
    let len = g.len() / 2;
    let (g_lo, g_hi) = g.split_at_mut(len);

    parallelize(g_lo, |g_lo, start| {
        let g_hi = &g_hi[start..];
        let mut tmp = Vec::with_capacity(g_lo.len());
        for (g_lo, g_hi) in g_lo.iter().zip(g_hi.iter()) {
            tmp.push(g_lo.to_curve() + &(*g_hi * challenge));
        }
        C::Curve::batch_normalize(&tmp, g_lo);
    });
}

/// One transcript operation observed by [`Recording`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranscriptEvent<C: CurveAffine> {
    /// `common_point`.
    CommonPoint(C),
    /// `common_scalar`.
    CommonScalar(C::Scalar),
    /// `write_point`.
    WritePoint(C),
    /// `write_scalar`.
    WriteScalar(C::Scalar),
    /// `read_point`, with the point read.
    ReadPoint(C),
    /// `read_scalar`, with the scalar read.
    ReadScalar(C::Scalar),
    /// `squeeze_challenge`, with the challenge scalar.
    Challenge(C::Scalar),
}

/// A transcript wrapper that records every successful operation of `T`.
///
/// It forwards every call unchanged, so the wrapped transcript produces the
/// same bytes and challenges as it would on its own.
#[derive(Clone, Debug)]
pub struct Recording<C: CurveAffine, T> {
    inner: T,
    events: Vec<TranscriptEvent<C>>,
}

impl<C: CurveAffine, T> Recording<C, T> {
    /// Wraps `inner` with an empty record.
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            events: Vec::new(),
        }
    }

    /// The recorded operations, in order.
    pub fn events(&self) -> &[TranscriptEvent<C>] {
        &self.events
    }

    /// The squeezed challenges, in order.
    pub fn challenges(&self) -> Vec<C::Scalar> {
        self.events
            .iter()
            .filter_map(|event| match event {
                TranscriptEvent::Challenge(challenge) => Some(*challenge),
                _ => None,
            })
            .collect()
    }

    /// The wrapped transcript.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Unwraps the transcript, discarding the record.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<C, E, T> Transcript<C, E> for Recording<C, T>
where
    C: CurveAffine,
    E: EncodedChallenge<C>,
    T: Transcript<C, E>,
{
    fn squeeze_challenge(&mut self) -> E {
        let challenge = self.inner.squeeze_challenge();
        self.events
            .push(TranscriptEvent::Challenge(challenge.get_scalar()));
        challenge
    }

    fn common_point(&mut self, point: C) -> io::Result<()> {
        self.inner.common_point(point)?;
        self.events.push(TranscriptEvent::CommonPoint(point));
        Ok(())
    }

    fn common_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.inner.common_scalar(scalar)?;
        self.events.push(TranscriptEvent::CommonScalar(scalar));
        Ok(())
    }
}

impl<C, E, T> TranscriptWrite<C, E> for Recording<C, T>
where
    C: CurveAffine,
    E: EncodedChallenge<C>,
    T: TranscriptWrite<C, E>,
{
    fn write_point(&mut self, point: C) -> io::Result<()> {
        self.inner.write_point(point)?;
        self.events.push(TranscriptEvent::WritePoint(point));
        Ok(())
    }

    fn write_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.inner.write_scalar(scalar)?;
        self.events.push(TranscriptEvent::WriteScalar(scalar));
        Ok(())
    }
}

impl<C, E, T> TranscriptRead<C, E> for Recording<C, T>
where
    C: CurveAffine,
    E: EncodedChallenge<C>,
    T: TranscriptRead<C, E>,
{
    fn read_point(&mut self) -> io::Result<C> {
        let point = self.inner.read_point()?;
        self.events.push(TranscriptEvent::ReadPoint(point));
        Ok(point)
    }

    fn read_scalar(&mut self) -> io::Result<C::Scalar> {
        let scalar = self.inner.read_scalar()?;
        self.events.push(TranscriptEvent::ReadScalar(scalar));
        Ok(scalar)
    }
}

#[cfg(test)]
mod tests {
    use halo2_axiom::{
        halo2curves::{
            ff::Field,
            group::{Group, prime::PrimeCurveAffine},
            pasta::{Eq, EqAffine, Fp},
        },
        transcript::{
            Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
        },
    };
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    use super::*;

    #[test]
    fn generator_collapse_matches_the_definition() {
        let mut rng = ChaCha20Rng::seed_from_u64(1);
        let g: Vec<EqAffine> = (0..10).map(|_| Eq::random(&mut rng).to_affine()).collect();
        let u = Fp::random(&mut rng);
        let mut folded = g.clone();
        generator_collapse(&mut folded, u);
        for i in 0..5 {
            assert_eq!(folded[i], (g[i] + g[i + 5] * u).to_affine());
        }
        assert_eq!(folded[5..], g[5..], "the upper half is left for truncation");
        let mut single = vec![g[0]];
        generator_collapse(&mut single, u);
        assert_eq!(single, vec![g[0]]);
    }

    #[test]
    fn recording_forwards_and_records_every_operation() {
        let g = EqAffine::generator();
        let s = Fp::from(5);
        let mut writer = Recording::new(Blake2bWrite::<_, EqAffine, Challenge255<_>>::init(
            Vec::new(),
        ));
        writer.common_point(g).unwrap();
        writer.common_scalar(s).unwrap();
        let first = writer.squeeze_challenge().get_scalar();
        writer.write_point(-g).unwrap();
        writer.write_scalar(-s).unwrap();
        let second = writer.squeeze_challenge().get_scalar();
        assert_eq!(writer.challenges(), vec![first, second]);
        assert_eq!(writer.events().len(), 6);
        let _: &Blake2bWrite<Vec<u8>, EqAffine, Challenge255<EqAffine>> = writer.inner();
        assert!(writer.common_point(EqAffine::identity()).is_err());
        assert_eq!(
            writer.events().len(),
            6,
            "failed operations are not recorded"
        );

        let bytes = writer.clone().into_inner().finalize();
        let mut plain = Blake2bWrite::<_, EqAffine, Challenge255<_>>::init(Vec::new());
        plain.common_point(g).unwrap();
        plain.common_scalar(s).unwrap();
        let _ = plain.squeeze_challenge();
        plain.write_point(-g).unwrap();
        plain.write_scalar(-s).unwrap();
        assert_eq!(
            plain.finalize(),
            bytes,
            "recording does not change the bytes"
        );

        let mut reader = Recording::new(Blake2bRead::<_, EqAffine, Challenge255<_>>::init(
            bytes.as_slice(),
        ));
        reader.common_point(g).unwrap();
        reader.common_scalar(s).unwrap();
        let _ = reader.squeeze_challenge();
        assert_eq!(reader.read_point().unwrap(), -g);
        assert_eq!(reader.read_scalar().unwrap(), -s);
        let _ = reader.squeeze_challenge();
        assert_eq!(reader.challenges(), writer.challenges());
        assert_eq!(
            reader.events()[3..5],
            [
                TranscriptEvent::ReadPoint(-g),
                TranscriptEvent::ReadScalar(-s)
            ]
        );
        assert!(reader.read_scalar().is_err(), "the stream is exhausted");
    }
}
