//! Forced challenge coincidences reject without a panic or an extra transcript event.

use super::*;
use crate::{
    poly::{
        Coeff, Polynomial, ProverQuery,
        commitment::{ParamsProver, Prover},
        ipa::multiopen::ProverIPA,
    },
    transcript::{
        Blake2bRead, Blake2bWrite, Challenge255, Transcript, TranscriptReadBuffer, TranscriptWrite,
        TranscriptWriterBuffer,
    },
};
use ff::FromUniformBytes;
use group::Curve;
use halo2curves::pasta::{EpAffine, EqAffine};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use std::{io, marker::PhantomData};

struct ForcedChallenge<C: CurveAffine>(C::Scalar);
impl<C: CurveAffine> EncodedChallenge<C> for ForcedChallenge<C> {
    type Input = C::Scalar;
    fn new(input: &C::Scalar) -> Self {
        Self(*input)
    }
    fn get_scalar(&self) -> C::Scalar {
        self.0
    }
}

/// The ordinary transcript absorbs every event; only the requested x3 scalar is overridden.
struct ForcedX3<C: CurveAffine, T> {
    inner: T,
    x3: C::Scalar,
    squeezes: usize,
    points: usize,
    scalars: usize,
}
impl<C: CurveAffine, T> ForcedX3<C, T> {
    fn new(inner: T, x3: C::Scalar) -> Self {
        Self {
            inner,
            x3,
            squeezes: 0,
            points: 0,
            scalars: 0,
        }
    }
}
impl<C, T> Transcript<C, ForcedChallenge<C>> for ForcedX3<C, T>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    T: Transcript<C, Challenge255<C>>,
{
    fn squeeze_challenge(&mut self) -> ForcedChallenge<C> {
        let ordinary = self.inner.squeeze_challenge().get_scalar();
        self.squeezes += 1;
        ForcedChallenge(if self.squeezes == 3 {
            self.x3
        } else {
            ordinary
        })
    }
    fn common_point(&mut self, point: C) -> io::Result<()> {
        self.inner.common_point(point)
    }
    fn common_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.inner.common_scalar(scalar)
    }
}
impl<C, T> TranscriptWrite<C, ForcedChallenge<C>> for ForcedX3<C, T>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    T: TranscriptWrite<C, Challenge255<C>>,
{
    fn write_point(&mut self, point: C) -> io::Result<()> {
        self.points += 1;
        self.inner.write_point(point)
    }
    fn write_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.scalars += 1;
        self.inner.write_scalar(scalar)
    }
}
impl<C, T> TranscriptRead<C, ForcedChallenge<C>> for ForcedX3<C, T>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    T: TranscriptRead<C, Challenge255<C>>,
{
    fn read_point(&mut self) -> io::Result<C> {
        self.points += 1;
        self.inner.read_point()
    }
    fn read_scalar(&mut self) -> io::Result<C::Scalar> {
        self.scalars += 1;
        self.inner.read_scalar()
    }
}

fn check_coincidences<C: CurveAffine>()
where
    C::Scalar: FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let polynomials: Vec<Polynomial<C::Scalar, Coeff>> = (0..2)
        .map(|p| Polynomial {
            values: (0..16)
                .map(|r| C::Scalar::from((13 + p * 41 + r * r) as u64))
                .collect(),
            _marker: PhantomData,
        })
        .collect();
    let blinds = [
        crate::poly::commitment::Blind(C::Scalar::from(3)),
        crate::poly::commitment::Blind(C::Scalar::from(5)),
    ];
    let commitments: Vec<_> = polynomials
        .iter()
        .zip(blinds)
        .map(|(poly, blind)| params.commit(poly, blind).to_affine())
        .collect();
    let points = [C::Scalar::ZERO, C::Scalar::ONE, C::Scalar::from(17)];
    // Two point sets, with a coincidence at either division of the first set or in the last set.
    let bindings = [(0, 0), (0, 1), (1, 2)];
    let prover_queries: Vec<_> = bindings
        .iter()
        .map(|&(poly, point)| ProverQuery {
            point: points[point],
            poly: &polynomials[poly],
            blind: blinds[poly],
        })
        .collect();
    let verifier_queries: Vec<_> = bindings
        .iter()
        .map(|&(poly, point)| {
            VerifierQuery::new_commitment(
                &commitments[poly],
                points[point],
                eval_polynomial(&polynomials[poly], points[point]),
            )
        })
        .collect();
    for x3 in points.into_iter().chain([C::Scalar::from(29)]) {
        let singular = points.contains(&x3);
        let mut writer = ForcedX3::new(Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new()), x3);
        for commitment in &commitments {
            writer.common_point(*commitment).unwrap();
        }
        ProverIPA::<C>::new(&params)
            .create_proof(
                ChaCha20Rng::from_seed([83; 32]),
                &mut writer,
                prover_queries.clone(),
            )
            .unwrap();
        let bytes = writer.inner.finalize();
        let mut reader = ForcedX3::new(
            Blake2bRead::<_, C, Challenge255<C>>::init(bytes.as_slice()),
            x3,
        );
        for commitment in &commitments {
            reader.common_point(*commitment).unwrap();
        }
        let result = VerifierIPA::<C>::new(&params).verify_proof(
            &mut reader,
            verifier_queries.clone(),
            params.empty_msm(),
        );
        if singular {
            assert!(matches!(result, Err(Error::OpeningError)));
            assert_eq!(reader.squeezes, 3, "no x4 or resampling after rejection");
            assert_eq!(reader.points, 1, "only q-prime has been read");
            assert_eq!(reader.scalars, 2, "the original two u values were read");
        } else {
            assert!(result.unwrap().use_challenges().check());
            assert_eq!(reader.squeezes, writer.squeezes);
            assert_eq!(reader.points, writer.points);
            assert_eq!(reader.scalars, writer.scalars);
        }
    }
}

#[test]
fn both_pasta_singular_multiopening_challenge_rejects_without_resampling_or_panic() {
    check_coincidences::<EqAffine>();
    check_coincidences::<EpAffine>();
}
