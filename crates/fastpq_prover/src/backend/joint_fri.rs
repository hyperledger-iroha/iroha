//! Exact-degree batching of the committed quotient and mixed trace oracles.
//!
//! On the L=8N point coset, every fixed oracle has one interpolant of degree < L.
//! Checking J=Q+rho*T+sigma*X^N*T (reduced modulo the coset vanishing polynomial)
//! against degree < 2N batches Q's bound < 2N with T's bound < N. If T has degree
//! >= 2N, its unshifted term has a nonzero high coefficient. If N <= deg(T) < 2N,
//! its shifted term has degree in [2N,3N), without wrap. If only Q violates its
//! bound, a high coefficient of Q is nonzero. In every case a high coefficient
//! of J is a nonzero affine polynomial in independently sampled rho and sigma;
//! exact cancellation has probability at most 1/|Fp4| for fixed oracles.
//!
//! The unshifted T term is essential: X^N alone wraps degrees >= L-N into low
//! coefficients and would silently admit some invalid trace interpolants.
//! TODO: obtain independent review of proximity, query sampling, Fiat-Shamir and
//! multi-target composition; the exact-degree lemma is not that security proof.

use super::{AirQuotientDomain, Error, FriDomain, GoldilocksFp4V1, Result, Transcript, field_pow};
use fastpq_isi::StarkParameterSet;

/// Validated degree-alignment factors and post-commitment batching challenges.
#[derive(Clone, Debug)]
pub(crate) struct JointFriBatch {
    domain_size: usize,
    shifts: Vec<u64>,
    rho: GoldilocksFp4V1,
    sigma: GoldilocksFp4V1,
}

impl JointFriBatch {
    /// Derive independent extension-field coefficients after both oracle roots.
    pub(crate) fn from_transcript(
        params: &StarkParameterSet,
        domain_size: usize,
        transcript: &mut Transcript,
    ) -> Result<Self> {
        Self::from_challenges(
            params,
            domain_size,
            transcript.challenge_extension("fastpq:v1:joint-fri:trace"),
            transcript.challenge_extension("fastpq:v1:joint-fri:shifted-trace"),
        )
    }

    /// Use the complete joint coefficients supplied by a fixed protocol.
    ///
    /// The caller must derive both values from the post-commitment verifier
    /// message after binding the quotient and mixed roots. This constructor
    /// validates geometry and preserves those values without sampling again.
    pub(crate) fn from_challenges(
        params: &StarkParameterSet,
        domain_size: usize,
        rho: GoldilocksFp4V1,
        sigma: GoldilocksFp4V1,
    ) -> Result<Self> {
        AirQuotientDomain::new(params, domain_size)?;
        let blowup = params.fri.blowup_factor as usize;
        let trace_size = domain_size / blowup;
        let domain = FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            domain_size,
            params.omega_coset,
        )?;
        // X^N has period blowup over this coset, as does the row zerofier.
        let shifts = (0..blowup)
            .map(|index| field_pow(domain.point(index), trace_size as u64))
            .collect();
        Ok(Self {
            domain_size,
            shifts,
            rho,
            sigma,
        })
    }

    /// Link one joint FRI opening to both authenticated source oracle values.
    pub(crate) fn value_at(
        &self,
        index: usize,
        quotient: GoldilocksFp4V1,
        trace: GoldilocksFp4V1,
    ) -> Result<GoldilocksFp4V1> {
        if index >= self.domain_size {
            return Err(Error::QueryIndexOutOfRange {
                index,
                len: self.domain_size,
            });
        }
        let factor = self
            .rho
            .add(self.sigma.mul_base(self.shifts[index % self.shifts.len()]));
        Ok(quotient.add(trace.mul(factor)))
    }

    /// Form the committed first FRI layer from the two equally sized oracles.
    pub(crate) fn values(
        &self,
        quotient: &[GoldilocksFp4V1],
        trace: &[GoldilocksFp4V1],
    ) -> Result<Vec<GoldilocksFp4V1>> {
        if quotient.len() != self.domain_size || trace.len() != self.domain_size {
            return Err(Error::InvalidTraceShape {
                details: "joint FRI requires equal quotient, trace and evaluation-domain lengths"
                    .to_owned(),
            });
        }
        quotient
            .iter()
            .zip(trace)
            .enumerate()
            .map(|(index, (&q, &t))| self.value_at(index, q, t))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fastpq_isi::FASTPQ_FINAL_V1;

    fn fp4(value: u64) -> GoldilocksFp4V1 {
        GoldilocksFp4V1::from_base(value).unwrap()
    }

    #[test]
    fn joint_degree_alignment_rejects_bad_trace_or_quotient_interpolants() {
        let params = FASTPQ_FINAL_V1;
        let trace_size = 4;
        let length = 32;
        let domain = FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            length,
            params.omega_coset,
        )
        .unwrap();
        let monomial = |degree| {
            (0..length)
                .map(|i| fp4(field_pow(domain.point(i), degree)))
                .collect::<Vec<_>>()
        };
        // Use independent extension directions so no base-field high coefficient
        // can cancel either batching component in these deterministic probes.
        let rho = GoldilocksFp4V1::new([0, 1, 0, 0]).unwrap();
        let sigma = GoldilocksFp4V1::new([0, 0, 1, 0]).unwrap();
        let joint = JointFriBatch::from_challenges(&params, length, rho, sigma).unwrap();
        for (q_degree, t_degree, valid) in [
            (7, 3, true),
            (8, 3, false),
            (7, 4, false),
            (7, 7, false),
            (7, 8, false),
            (7, 28, false),
            (7, 31, false),
        ] {
            let values = joint
                .values(&monomial(q_degree), &monomial(t_degree))
                .unwrap();
            assert_eq!(
                domain
                    .evaluations_have_degree_below(&values, 2 * trace_size)
                    .unwrap(),
                valid,
                "Q degree {q_degree}, T degree {t_degree}"
            );
        }
        assert!(joint.values(&[], &monomial(0)).is_err());
        assert!(joint.value_at(length, fp4(0), fp4(0)).is_err());
    }

    #[test]
    fn sampled_joint_values_match_the_full_coset_expression() {
        let params = FASTPQ_FINAL_V1;
        let length = 64;
        let trace_size = length / params.fri.blowup_factor as usize;
        let domain = FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            length,
            params.omega_coset,
        )
        .unwrap();
        let rho = GoldilocksFp4V1::new([1, 2, 3, 4]).unwrap();
        let sigma = GoldilocksFp4V1::new([5, 6, 7, 8]).unwrap();
        let joint = JointFriBatch::from_challenges(&params, length, rho, sigma).unwrap();
        let quotients = (0..length).map(|i| fp4(i as u64 + 11)).collect::<Vec<_>>();
        let traces = (0..length)
            .map(|i| GoldilocksFp4V1::new([i as u64, 13, 17, 19]).unwrap())
            .collect::<Vec<_>>();
        let values = joint.values(&quotients, &traces).unwrap();
        for i in 0..length {
            let shift = field_pow(domain.point(i), trace_size as u64);
            let expected = quotients[i]
                .add(rho.mul(traces[i]))
                .add(sigma.mul(traces[i]).mul_base(shift));
            assert_eq!(
                joint.value_at(i, quotients[i], traces[i]).unwrap(),
                expected
            );
            assert_eq!(values[i], expected);
        }
        assert!(joint.values(&quotients, &traces[..length - 1]).is_err());
        assert!(joint.values(&quotients[..length - 1], &traces).is_err());
        assert!(JointFriBatch::from_challenges(&params, 7, rho, sigma).is_err());
        assert!(JointFriBatch::from_challenges(&params, 0, rho, sigma).is_err());
    }

    #[test]
    fn both_committed_oracle_roots_bind_the_joint_fri_challenges() {
        let params = FASTPQ_FINAL_V1;
        let challenge = |trace_byte, quotient_byte| {
            let mut transcript = Transcript::initialise(
                &crate::proof::PublicIO::default(),
                params.name,
                1,
                super::super::TRANSCRIPT_TAG_INIT,
            )
            .unwrap();
            transcript.append_message(super::super::TRANSCRIPT_TAG_ROOTS, &[trace_byte; 48]);
            transcript.append_message(super::super::TRANSCRIPT_TAG_AIR_ROOTS, &[quotient_byte; 48]);
            JointFriBatch::from_transcript(&params, 32, &mut transcript).unwrap()
        };
        let baseline = challenge(1, 2);
        let same = challenge(1, 2);
        assert_eq!((baseline.rho, baseline.sigma), (same.rho, same.sigma));
        assert_ne!(baseline.rho, baseline.sigma);
        for changed in [challenge(3, 2), challenge(1, 3)] {
            assert_ne!(baseline.rho, changed.rho);
            assert_ne!(baseline.sigma, changed.sigma);
        }
    }

    #[test]
    fn shifted_only_batch_would_hide_the_highest_trace_degrees() {
        let params = FASTPQ_FINAL_V1;
        let length = 32;
        let domain = FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            length,
            params.omega_coset,
        )
        .unwrap();
        let trace = (0..length)
            .map(|i| fp4(field_pow(domain.point(i), 28)))
            .collect::<Vec<_>>();
        let zero = vec![fp4(0); length];
        assert!(!domain.evaluations_have_degree_below(&trace, 4).unwrap());
        let shifted_only = JointFriBatch::from_challenges(&params, length, fp4(0), fp4(1)).unwrap();
        let hidden = shifted_only.values(&zero, &trace).unwrap();
        assert!(domain.evaluations_have_degree_below(&hidden, 1).unwrap());
        let complete = JointFriBatch::from_challenges(&params, length, fp4(1), fp4(1)).unwrap();
        assert!(
            !domain
                .evaluations_have_degree_below(&complete.values(&zero, &trace).unwrap(), 8)
                .unwrap()
        );
    }
}
