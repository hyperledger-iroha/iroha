//! Test-only observations from the unchanged dense grouping and multiopening arithmetic.

use super::*;

/// Exercise the original duplicate check with both identical and conflicting evaluations.
pub(crate) fn duplicate_evaluations_rejected<F: Field + Ord>(
    point: F,
    evaluations: [F; 2],
) -> (bool, usize) {
    use crate::poly::query::Query;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Clone)]
    struct ValueQuery<F> {
        point: F,
        value: F,
        calls: Arc<AtomicUsize>,
    }
    impl<F: Field> Query<F> for ValueQuery<F> {
        type Commitment = usize;
        type Eval = F;
        fn get_point(&self) -> F {
            self.point
        }
        fn get_commitment(&self) -> usize {
            0
        }
        fn get_eval(&self) -> F {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.value
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let queries = evaluations.map(|value| ValueQuery {
        point,
        value,
        calls: Arc::clone(&calls),
    });
    let rejected = construct_intermediate_sets(queries).is_none();
    (rejected, calls.load(Ordering::SeqCst))
}

/// One dense source's ordinary grouping result in first-encounter order.
pub(crate) struct Source<F> {
    /// Index of the first original query naming this polynomial object.
    pub(crate) first_query: usize,
    /// Ordinary point-set index.
    pub(crate) set: usize,
    /// Sorted first-encounter point indexes for this source.
    pub(crate) points: Vec<usize>,
    /// Ordinary get_eval results in sorted-point order.
    pub(crate) evaluations: Vec<F>,
}
/// Dense stage outputs observed independently of stored planning and arithmetic.
pub(crate) struct Prepared<C: CurveAffine> {
    /// Original query rows (first-encounter source index, point index).
    pub(crate) queries: Vec<(usize, usize)>,
    /// Original source-map rows.
    pub(crate) sources: Vec<Source<C::Scalar>>,
    /// First-encounter scalar point table.
    pub(crate) points: Vec<C::Scalar>,
    /// First source index representing each point set.
    pub(crate) sets: Vec<usize>,
    /// Initial query evaluations in original query order.
    pub(crate) evaluations: Vec<C::Scalar>,
    /// Reconstructed Q polynomials in set order.
    pub(crate) q: Vec<Vec<C::Scalar>>,
    /// Corresponding Q blinds.
    pub(crate) q_blinds: Vec<Blind<C::Scalar>>,
    /// Divided Q polynomials padded to n.
    pub(crate) divided: Vec<Vec<C::Scalar>>,
    /// Complete q-prime polynomial before P folding.
    pub(crate) q_prime: Polynomial<C::Scalar, Coeff>,
    /// Original q-prime blind draw.
    pub(crate) q_prime_blind: Blind<C::Scalar>,
    /// Original q-prime commitment.
    pub(crate) commitment: C,
    /// Q(x3) scalars in set order.
    pub(crate) u: Vec<C::Scalar>,
    /// Original final P polynomial.
    pub(crate) p: Polynomial<C::Scalar, Coeff>,
    /// Original final P blind.
    pub(crate) p_blind: Blind<C::Scalar>,
    /// Exact ordinary typed challenge values in order.
    pub(crate) challenges: [C::Scalar; 4],
    /// Original nominal x3 type, passed unchanged to inner IPA.
    pub(crate) x3: ChallengeX3<C>,
}

/// Observe the actual private dense planner/fold/division implementations through P.
pub(crate) fn prepare<C, R, T, E>(
    params: &ParamsIPA<C>,
    mut rng: R,
    transcript: &mut T,
    queries: Vec<ProverQuery<'_, C>>,
) -> io::Result<Prepared<C>>
where
    C: CurveAffine,
    R: RngCore,
    E: EncodedChallenge<C>,
    T: TranscriptWrite<C, E>,
{
    let x1: ChallengeX1<C> = transcript.squeeze_challenge_scalar();
    let x2: ChallengeX2<C> = transcript.squeeze_challenge_scalar();
    let (map, point_sets) = construct_intermediate_sets(queries.clone())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "duplicate dense query"))?;
    let mut points = Vec::new();
    let mut query_rows = Vec::new();
    let mut evaluations = Vec::new();
    for query in &queries {
        let point = points
            .iter()
            .position(|p| *p == query.point)
            .unwrap_or_else(|| {
                points.push(query.point);
                points.len() - 1
            });
        let source = map
            .iter()
            .position(|data| std::ptr::eq(data.commitment.poly, query.poly))
            .unwrap();
        query_rows.push((source, point));
        // Use the get_eval result already produced by the original helper, without a second
        // witness evaluation or a replacement algorithm for the expected value.
        let index = point_sets[map[source].set_index]
            .iter()
            .position(|p| *p == query.point)
            .unwrap();
        evaluations.push(map[source].evals[index]);
    }
    let sources = map
        .iter()
        .map(|data| {
            let first_query = queries
                .iter()
                .position(|query| std::ptr::eq(query.poly, data.commitment.poly))
                .unwrap();
            let mut ids = data.point_indices.clone();
            ids.sort_unstable();
            Source {
                first_query,
                set: data.set_index,
                points: ids,
                evaluations: data.evals.clone(),
            }
        })
        .collect();
    let sets = (0..point_sets.len())
        .map(|set| map.iter().position(|data| data.set_index == set).unwrap())
        .collect();
    let mut q = Vec::new();
    let mut q_blinds = Vec::new();
    let mut divided = Vec::new();
    let mut scratch = Polynomial {
        values: Vec::with_capacity(params.n as usize),
        _marker: PhantomData,
    };
    let mut q_prime = None;
    for (set, points) in point_sets.iter().enumerate() {
        q_blinds.push(reconstruct_q(&map, set, *x1, &mut scratch));
        q.push(scratch.values.clone());
        for point in points {
            kate_division_in_place(&mut scratch.values, *point);
        }
        scratch.values.resize(params.n as usize, C::Scalar::ZERO);
        divided.push(scratch.values.clone());
        if let Some(acc) = &mut q_prime {
            fold_polynomial_in_place(acc, *x2, &scratch);
        } else {
            q_prime = Some(scratch.clone());
        }
    }
    let q_prime =
        q_prime.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "empty dense query"))?;
    let q_prime_blind = Blind(C::Scalar::random(&mut rng));
    let commitment = params.commit(&q_prime, q_prime_blind).to_affine();
    transcript.write_point(commitment)?;
    let x3: ChallengeX3<C> = transcript.squeeze_challenge_scalar();
    let mut u = Vec::new();
    for set in 0..point_sets.len() {
        reconstruct_q(&map, set, *x1, &mut scratch);
        let value = eval_polynomial(&scratch, *x3);
        transcript.write_scalar(value)?;
        u.push(value);
    }
    let x4: ChallengeX4<C> = transcript.squeeze_challenge_scalar();
    let mut p = q_prime.clone();
    let mut p_blind = q_prime_blind;
    for set in 0..point_sets.len() {
        let blind = reconstruct_q(&map, set, *x1, &mut scratch);
        fold_polynomial_in_place(&mut p, *x4, &scratch);
        p_blind = Blind(p_blind.0 * *x4 + blind.0);
    }
    Ok(Prepared {
        queries: query_rows,
        sources,
        points,
        sets,
        evaluations,
        q,
        q_blinds,
        divided,
        q_prime,
        q_prime_blind,
        commitment,
        u,
        p,
        p_blind,
        challenges: [*x1, *x2, *x3, *x4],
        x3,
    })
}
