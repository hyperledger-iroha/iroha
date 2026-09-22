//! Zero round challenges stop the real IPA verifier at the prover's inverse boundary.

use super::*;
use crate::{
    arithmetic::eval_polynomial,
    poly::{
        Coeff, Polynomial,
        commitment::{Blind, Params, ParamsProver},
        ipa::commitment::create_proof,
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

struct Challenge<C: CurveAffine>(C::Scalar);
impl<C: CurveAffine> EncodedChallenge<C> for Challenge<C> {
    type Input = C::Scalar;
    fn new(input: &C::Scalar) -> Self {
        Self(*input)
    }
    fn get_scalar(&self) -> C::Scalar {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Point,
    Challenge,
    Scalar,
}

/// Every event still enters the ordinary transcript; selected challenges can be overridden.
struct Scripted<C: CurveAffine, T> {
    inner: T,
    overrides: Vec<Option<C::Scalar>>,
    challenges: usize,
    events: Vec<Event>,
}
impl<C: CurveAffine, T> Scripted<C, T> {
    fn new(inner: T, overrides: Vec<Option<C::Scalar>>) -> Self {
        Self {
            inner,
            overrides,
            challenges: 0,
            events: Vec::new(),
        }
    }
}
impl<C, T> Transcript<C, Challenge<C>> for Scripted<C, T>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    T: Transcript<C, Challenge255<C>>,
{
    fn squeeze_challenge(&mut self) -> Challenge<C> {
        self.events.push(Event::Challenge);
        let actual = self.inner.squeeze_challenge().get_scalar();
        let value = self
            .overrides
            .get(self.challenges)
            .copied()
            .flatten()
            .unwrap_or(actual);
        self.challenges += 1;
        Challenge(value)
    }
    fn common_point(&mut self, point: C) -> io::Result<()> {
        self.inner.common_point(point)
    }
    fn common_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.inner.common_scalar(scalar)
    }
}
impl<C, T> TranscriptWrite<C, Challenge<C>> for Scripted<C, T>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    T: TranscriptWrite<C, Challenge255<C>>,
{
    fn write_point(&mut self, point: C) -> io::Result<()> {
        self.events.push(Event::Point);
        self.inner.write_point(point)
    }
    fn write_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.events.push(Event::Scalar);
        self.inner.write_scalar(scalar)
    }
}
impl<C, T> TranscriptRead<C, Challenge<C>> for Scripted<C, T>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    T: TranscriptRead<C, Challenge255<C>>,
{
    fn read_point(&mut self) -> io::Result<C> {
        self.events.push(Event::Point);
        self.inner.read_point()
    }
    fn read_scalar(&mut self) -> io::Result<C::Scalar> {
        self.events.push(Event::Scalar);
        self.inner.read_scalar()
    }
}

fn absorb<C: CurveAffine>(
    transcript: &mut impl Transcript<C, Challenge<C>>,
    commitment: C,
    point: C::Scalar,
    value: C::Scalar,
) {
    transcript.common_point(commitment).unwrap();
    transcript.common_scalar(point).unwrap();
    transcript.common_scalar(value).unwrap();
}

fn check<C: CurveAffine>(k: u32, overrides: Vec<Option<C::Scalar>>, reject_rounds: bool)
where
    C::Scalar: FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(k);
    let polynomial = Polynomial::<C::Scalar, Coeff> {
        values: (0..params.n).map(|i| C::Scalar::from(7 + i * i)).collect(),
        _marker: PhantomData,
    };
    let blind = Blind(C::Scalar::from(19));
    let point = C::Scalar::from(11);
    let value = eval_polynomial(&polynomial, point);
    let commitment = params.commit(&polynomial, blind).to_affine();
    let mut writer = Scripted::new(
        Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new()),
        overrides.clone(),
    );
    absorb(&mut writer, commitment, point, value);
    create_proof(
        &params,
        ChaCha20Rng::from_seed([61; 32]),
        &mut writer,
        &polynomial,
        blind,
        point,
    )
    .unwrap();
    let events = writer.events.clone();
    let bytes = writer.inner.finalize();
    let mut reader = Scripted::new(
        Blake2bRead::<_, C, Challenge255<C>>::init(bytes.as_slice()),
        overrides.clone(),
    );
    absorb(&mut reader, commitment, point, value);
    let mut msm = params.empty_msm();
    msm.append_term(C::Scalar::ONE, commitment.into());
    let guard = verify_proof(&params, msm, &mut reader, point, value).unwrap();
    assert!(
        guard.use_challenges().check(),
        "real ordinary proof must verify"
    );
    assert_eq!(reader.events, events);
    assert_eq!(events.len(), 5 + 3 * k as usize);
    if !reject_rounds {
        return;
    }
    for round in 0..k as usize {
        let mut forced = overrides.clone();
        forced.resize(2 + k as usize, None);
        forced[2 + round] = Some(C::Scalar::ZERO);
        // Supply a complete, independently generated ordinary proof. A verifier that
        // silently batch-inverts zero would return a guard and consume the final scalars.
        let mut reader = Scripted::new(
            Blake2bRead::<_, C, Challenge255<C>>::init(bytes.as_slice()),
            forced.clone(),
        );
        absorb(&mut reader, commitment, point, value);
        let mut msm = params.empty_msm();
        msm.append_term(C::Scalar::ONE, commitment.into());
        assert!(matches!(
            verify_proof(&params, msm, &mut reader, point, value),
            Err(Error::OpeningError)
        ));
        let prefix_len = 3 + 3 * (round + 1);
        assert_eq!(reader.events, events[..prefix_len]);
        assert_eq!(reader.challenges, 3 + round);
        assert!(!reader.events.contains(&Event::Scalar));

        let mut writer = Scripted::new(
            Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new()),
            forced,
        );
        absorb(&mut writer, commitment, point, value);
        let error = create_proof(
            &params,
            ChaCha20Rng::from_seed([61; 32]),
            &mut writer,
            &polynomial,
            blind,
            point,
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            writer.events, reader.events,
            "same rejection boundary on both sides"
        );
        let prefix = writer.inner.finalize();
        assert_eq!(prefix, bytes[..prefix.len()]);
    }
}

fn nonzero<C: CurveAffine>()
where
    C::Scalar: FromUniformBytes<64>,
{
    check::<C>(0, vec![], false);
    check::<C>(
        1,
        vec![
            Some(C::Scalar::ZERO),
            Some(C::Scalar::ZERO),
            Some(C::Scalar::ONE),
        ],
        false,
    );
    check::<C>(
        4,
        vec![
            Some(C::Scalar::ONE),
            Some(C::Scalar::from(3)),
            Some(C::Scalar::ONE),
            Some(-C::Scalar::ONE),
            Some(C::Scalar::from(5)),
            Some(C::Scalar::from(7)),
        ],
        false,
    );
}

#[test]
fn both_pasta_every_zero_inner_ipa_round_challenge_rejects_at_original_prover_boundary() {
    check::<EqAffine>(4, vec![], true);
    check::<EpAffine>(4, vec![], true);
}

#[test]
fn both_pasta_inner_ipa_nonzero_rounds_and_zero_round_count_still_verify() {
    nonzero::<EqAffine>();
    nonzero::<EpAffine>();
}
