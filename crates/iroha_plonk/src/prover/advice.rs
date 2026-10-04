//! Instances and advice (spec 6.3 and section 7, row 1; `BlindingScheduleV1`
//! item 1).
//!
//! - Instance columns are zero-padded to `n` rows. In Committed mode each is
//!   committed in evaluation form with the default blind (`... + W`) and
//!   absorbed as a common point; in Direct mode every value is absorbed as a
//!   common scalar, column-major. Both forms keep the coefficient form for
//!   the quotient and the openings.
//! - Advice columns get random values on rows `u..n` (column by column),
//!   then one random blind per column, and are committed in evaluation form
//!   with the secret MSM posture and written to the proof in column order.

use iroha_pasta::{PastaCurve, msm::MemoryBudget};
use rand_core_06::RngCore;
use rayon::prelude::*;

use super::{ProverError, Witness, random_values, write_point};
use crate::{
    keys::ProvingKey,
    pcs::ipa::{PinnedParams, commit::Secrecy},
    protocol::{Shape, instance_commitment},
    transcript::{Transcript, TranscriptWrite},
};

/// The instance columns in both forms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct InstanceColumns<F> {
    /// Zero-padded evaluation form (`n` rows each).
    pub(super) values: Vec<Vec<F>>,
    /// Coefficient form.
    pub(super) polys: Vec<Vec<F>>,
    /// The unpadded lengths.
    lengths: Vec<usize>,
}

impl<F: iroha_pasta::PastaField> InstanceColumns<F> {
    /// Pads and interpolates the instance columns of a witness.
    ///
    /// # Errors
    ///
    /// [`ProverError::Fft`] on a transform failure.
    pub(super) fn new<C: PastaCurve<ScalarExt = F>>(
        pk: &ProvingKey<C>,
        instances: &[Vec<F>],
    ) -> Result<Self, ProverError> {
        let n = pk.binding().n();
        let values: Vec<Vec<F>> = instances
            .iter()
            .map(|column| {
                let mut padded = column.clone();
                padded.resize(n, F::ZERO);
                padded
            })
            .collect();
        let polys = values
            .iter()
            .map(|column| {
                let mut coeffs = column.clone();
                pk.domain().ifft(&mut coeffs)?;
                Ok(coeffs)
            })
            .collect::<Result<Vec<_>, ProverError>>()?;
        Ok(Self {
            values,
            polys,
            lengths: instances.iter().map(Vec::len).collect(),
        })
    }

    /// Absorbs the instances (spec 6.3 step 3).
    ///
    /// # Errors
    ///
    /// [`ProverError::IdentityInstanceCommitment`] for an identity
    /// commitment (negligible: the default blind adds `W`).
    pub(super) fn absorb<C, T>(
        &self,
        params: &PinnedParams<C>,
        shape: &Shape,
        transcript: &mut T,
        budget: MemoryBudget,
    ) -> Result<(), ProverError>
    where
        C: PastaCurve<ScalarExt = F>,
        T: Transcript<C>,
    {
        if shape.committed_instances {
            for (column, (values, length)) in self.values.iter().zip(&self.lengths).enumerate() {
                let commitment = instance_commitment(params, &values[..*length], budget);
                transcript
                    .common_point(&commitment)
                    .map_err(|_| ProverError::IdentityInstanceCommitment { column })?;
            }
        } else {
            for (values, length) in self.values.iter().zip(&self.lengths) {
                for value in &values[..*length] {
                    transcript.common_scalar(value);
                }
            }
        }
        Ok(())
    }
}

/// The committed advice columns.
pub(super) struct Advice<F: iroha_pasta::PastaField> {
    /// Evaluation form with the blinding rows (`n` rows each).
    pub(super) values: Vec<Vec<F>>,
    /// Coefficient form.
    pub(super) polys: Vec<Vec<F>>,
    /// The commitment blinds.
    pub(super) blinds: Vec<F>,
}

impl<F: iroha_pasta::PastaField> Drop for Advice<F> {
    fn drop(&mut self) {
        for value in self
            .values
            .iter_mut()
            .chain(self.polys.iter_mut())
            .flatten()
            .chain(self.blinds.iter_mut())
        {
            value.zeroize();
        }
    }
}

/// Blinds, commits and writes the advice columns (`BlindingScheduleV1` item
/// 1).
///
/// # Errors
///
/// [`ProverError::Msm`], [`ProverError::Fft`] or [`ProverError::Transcript`].
pub(super) fn commit<C, T, R>(
    params: &PinnedParams<C>,
    pk: &ProvingKey<C>,
    shape: &Shape,
    witness: &Witness<C::ScalarExt>,
    rng: &mut R,
    transcript: &mut T,
    budget: MemoryBudget,
) -> Result<Advice<C::ScalarExt>, ProverError>
where
    C: PastaCurve,
    T: TranscriptWrite<C>,
    R: RngCore,
{
    let mut advice = Advice {
        values: witness.advice().to_vec(),
        polys: Vec::new(),
        blinds: Vec::new(),
    };
    let blinding_rows = shape.n - shape.usable_rows;
    for column in &mut advice.values {
        let random: Vec<C::ScalarExt> = random_values(rng, blinding_rows);
        column[shape.usable_rows..].copy_from_slice(&random);
    }
    advice.blinds = random_values(rng, shape.num_advice);
    let tables = pk.commitment_tables();
    let commitments = advice
        .values
        .par_iter()
        .zip(advice.blinds.par_iter())
        .map(|(values, blind)| {
            tables
                .commit_lagrange(params.params(), values, blind, Secrecy::Secret, budget)
                .map(|point| point.to_affine())
        })
        .collect::<Result<Vec<_>, _>>()?;
    for commitment in &commitments {
        write_point(transcript, commitment)?;
    }
    advice.polys = advice
        .values
        .par_iter()
        .map(|values| {
            let mut coeffs = values.clone();
            pk.domain().ifft(&mut coeffs).map(|()| coeffs)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(advice)
}
