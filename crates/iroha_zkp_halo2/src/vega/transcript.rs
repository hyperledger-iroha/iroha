//! Closed-domain Keccak Fiat--Shamir transcript for canonical Vega proofs.
use super::{
    VegaCurveError, VegaT256ScalarV1,
    commitment::{Commitment, CommitmentError},
    sponge::keccak256,
};
use thiserror::Error;
const PERSONA_TAG: &[u8] = b"NoTR";
const DOM_SEP_TAG: &[u8] = b"NoDS";
const MAX_PENDING_BYTES: usize = 16 * 1024 * 1024;
/// Failure while operating the bounded canonical Vega transcript.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaTranscriptError {
    /// The fixed-width round counter would wrap.
    #[error("Vega transcript round counter exhausted")]
    RoundCounterExhausted,
    /// Material absorbed between squeezes exceeded the consensus cap.
    #[error("Vega transcript pending material exceeds {MAX_PENDING_BYTES} bytes")]
    PendingMaterialTooLarge,
    /// A point representation was not valid for transcript absorption.
    #[error(transparent)]
    Point(#[from] VegaCurveError),
    /// A commitment had an invalid transcript representation.
    #[error("Vega commitment has no canonical transcript representation")]
    CommitmentEncoding,
}
impl From<CommitmentError> for VegaTranscriptError {
    fn from(_: CommitmentError) -> Self {
        Self::CommitmentEncoding
    }
}
/// Exact Keccak-256 Fiat--Shamir transcript used by the pinned Vega engine.
///
/// Production construction fixes the upstream `neutronnova_prove` domain.
/// Raw labels and representations remain crate-private so higher-level proof
/// code can expose only fixed schedules and fixed-size representations,
/// preventing length-splitting ambiguity and cross-domain replay.
#[derive(Clone)]
pub struct VegaTranscriptV1 {
    round: u16,
    state: [u8; 64],
    pending: Vec<u8>,
}
impl VegaTranscriptV1 {
    /// Start the canonical multi-circuit NeutronNova Vega transcript.
    #[must_use]
    pub fn new_neutron_nova() -> Self {
        Self::new_raw(b"neutronnova_prove")
    }
    pub(super) fn absorb_scalar(
        &mut self,
        label: &'static [u8],
        scalar: VegaT256ScalarV1,
    ) -> Result<(), VegaTranscriptError> {
        self.absorb_raw(label, &scalar.to_be_bytes())
    }
    pub(super) fn absorb_scalars(
        &mut self,
        label: &'static [u8],
        scalars: &[VegaT256ScalarV1],
    ) -> Result<(), VegaTranscriptError> {
        let byte_len = scalars
            .len()
            .checked_mul(32)
            .ok_or(VegaTranscriptError::PendingMaterialTooLarge)?;
        self.reserve_absorb(label.len(), byte_len)?;
        self.pending.extend_from_slice(label);
        for scalar in scalars {
            self.pending.extend_from_slice(&scalar.to_be_bytes());
        }
        Ok(())
    }
    pub(super) fn absorb_commitment(
        &mut self,
        label: &'static [u8],
        commitment: &Commitment,
    ) -> Result<(), VegaTranscriptError> {
        self.absorb_raw(label, &commitment.transcript_bytes()?)
    }
    pub(super) fn absorb_r1cs_instance(
        &mut self,
        label: &'static [u8],
        commitment: &Commitment,
        public_inputs: &[VegaT256ScalarV1],
    ) -> Result<(), VegaTranscriptError> {
        let mut representation = commitment.transcript_bytes()?;
        representation.reserve(
            public_inputs
                .len()
                .checked_mul(32)
                .ok_or(VegaTranscriptError::PendingMaterialTooLarge)?,
        );
        for input in public_inputs {
            representation.extend_from_slice(&input.to_be_bytes());
        }
        self.absorb_raw(label, &representation)
    }
    pub(super) fn absorb_relaxed_r1cs_instance(
        &mut self,
        label: &'static [u8],
        witness_commitment: &Commitment,
        error_commitment: &Commitment,
        relaxation: VegaT256ScalarV1,
        public_inputs: &[VegaT256ScalarV1],
    ) -> Result<(), VegaTranscriptError> {
        let mut representation = witness_commitment.transcript_bytes()?;
        representation.extend_from_slice(&error_commitment.transcript_bytes()?);
        representation.extend_from_slice(&relaxation.to_be_bytes());
        representation.reserve(
            public_inputs
                .len()
                .checked_mul(32)
                .ok_or(VegaTranscriptError::PendingMaterialTooLarge)?,
        );
        for input in public_inputs {
            representation.extend_from_slice(&input.to_be_bytes());
        }
        self.absorb_raw(label, &representation)
    }
    pub(super) fn absorb_univariate(
        &mut self,
        label: &'static [u8],
        coefficients_except_linear: &[VegaT256ScalarV1],
    ) -> Result<(), VegaTranscriptError> {
        let byte_len = coefficients_except_linear
            .len()
            .checked_mul(32)
            .ok_or(VegaTranscriptError::PendingMaterialTooLarge)?;
        self.reserve_absorb(label.len(), byte_len)?;
        self.pending.extend_from_slice(label);
        for coefficient in coefficients_except_linear {
            // This is the one pinned scalar transcript representation that
            // uses the field's native little-endian proof encoding.
            self.pending.extend_from_slice(&coefficient.to_le_bytes());
        }
        Ok(())
    }
    pub(super) fn domain_separator(
        &mut self,
        domain: &'static [u8],
    ) -> Result<(), VegaTranscriptError> {
        self.reserve_absorb(DOM_SEP_TAG.len(), domain.len())?;
        self.pending.extend_from_slice(DOM_SEP_TAG);
        self.pending.extend_from_slice(domain);
        Ok(())
    }
    pub(super) fn absorb_raw(
        &mut self,
        label: &'static [u8],
        representation: &[u8],
    ) -> Result<(), VegaTranscriptError> {
        self.reserve_absorb(label.len(), representation.len())?;
        self.pending.extend_from_slice(label);
        self.pending.extend_from_slice(representation);
        Ok(())
    }
    pub(super) fn squeeze(
        &mut self,
        label: &'static [u8],
    ) -> Result<VegaT256ScalarV1, VegaTranscriptError> {
        let next_round = self
            .round
            .checked_add(1)
            .ok_or(VegaTranscriptError::RoundCounterExhausted)?;
        let mut input = Vec::with_capacity(
            self.pending.len() + DOM_SEP_TAG.len() + 2 + self.state.len() + label.len(),
        );
        input.extend_from_slice(&self.pending);
        input.extend_from_slice(DOM_SEP_TAG);
        input.extend_from_slice(&self.round.to_le_bytes());
        input.extend_from_slice(&self.state);
        input.extend_from_slice(label);
        let output = updated_state(&input);
        self.round = next_round;
        self.state = output;
        self.pending.clear();
        Ok(VegaT256ScalarV1::from_uniform_le_bytes(output))
    }
    fn new_raw(label: &'static [u8]) -> Self {
        let mut input = Vec::with_capacity(PERSONA_TAG.len() + label.len());
        input.extend_from_slice(PERSONA_TAG);
        input.extend_from_slice(label);
        Self {
            round: 0,
            state: updated_state(&input),
            pending: Vec::new(),
        }
    }
    fn reserve_absorb(
        &mut self,
        label_len: usize,
        representation_len: usize,
    ) -> Result<(), VegaTranscriptError> {
        let additional = label_len
            .checked_add(representation_len)
            .ok_or(VegaTranscriptError::PendingMaterialTooLarge)?;
        let final_len = self
            .pending
            .len()
            .checked_add(additional)
            .ok_or(VegaTranscriptError::PendingMaterialTooLarge)?;
        if final_len > MAX_PENDING_BYTES {
            return Err(VegaTranscriptError::PendingMaterialTooLarge);
        }
        self.pending.reserve(additional);
        Ok(())
    }
}
fn updated_state(input: &[u8]) -> [u8; 64] {
    let mut low = Vec::with_capacity(input.len() + 1);
    low.extend_from_slice(input);
    low.push(0);
    let mut high = input.to_vec();
    high.push(1);
    let mut output = [0_u8; 64];
    output[..32].copy_from_slice(&keccak256(&low));
    output[32..].copy_from_slice(&keccak256(&high));
    output
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::VegaT256PointV1;
    fn scalar_hex(scalar: VegaT256ScalarV1) -> String {
        hex::encode(scalar.to_le_bytes())
    }
    #[test]
    fn transcript_matches_independent_pinned_reference_vector() {
        let mut transcript = VegaTranscriptV1::new_raw(b"refimpl_vector");
        transcript
            .absorb_scalar(b"s1", VegaT256ScalarV1::from_u64(2))
            .expect("bounded");
        let c1 = transcript.squeeze(b"c1").expect("round available");
        transcript
            .absorb_raw(
                b"g",
                &VegaT256PointV1::canonical_generator()
                    .expect("canonical generator")
                    .to_transcript_bytes()
                    .expect("non-identity point"),
            )
            .expect("valid point");
        transcript
            .absorb_scalars(
                b"vs",
                &[VegaT256ScalarV1::from_u64(7), VegaT256ScalarV1::from_u64(9)],
            )
            .expect("bounded");
        let c2 = transcript.squeeze(b"c2").expect("round available");
        transcript.absorb_scalar(b"c", c1).expect("bounded");
        transcript.absorb_scalar(b"c", c2).expect("bounded");
        let c3 = transcript.squeeze(b"c3").expect("round available");
        let c4 = transcript.squeeze(b"c4").expect("round available");
        assert_eq!(
            scalar_hex(c1),
            "64c77efc9d66c2754055360c0346286f1cf76c0b5f5cdbc879fe7f31ea1f944b"
        );
        assert_eq!(
            scalar_hex(c2),
            "47216317153f45a73618f040694fc441558b857c3ec4cef11f9e28a68eb105b8"
        );
        assert_eq!(
            scalar_hex(c3),
            "c65e2384ce2cd86eced030ea5e4d5c2b6f964461fc834b29f5c2848f29c75d98"
        );
        assert_eq!(
            scalar_hex(c4),
            "983abdecf960c20256bfad6062326f42fae747ef12f227c7c9b1a1517ded2374"
        );
    }
    #[test]
    fn domain_and_schedule_changes_cannot_replay_challenges() {
        let mut production = VegaTranscriptV1::new_neutron_nova();
        production
            .absorb_scalar(b"c", VegaT256ScalarV1::from_u64(9))
            .expect("bounded");
        let production_challenge = production.squeeze(b"r").expect("round available");
        let mut other_domain = VegaTranscriptV1::new_raw(b"neutronnova_verify");
        other_domain
            .absorb_scalar(b"c", VegaT256ScalarV1::from_u64(9))
            .expect("bounded");
        assert_ne!(
            production_challenge,
            other_domain.squeeze(b"r").expect("round available")
        );
        let mut other_label = VegaTranscriptV1::new_neutron_nova();
        other_label
            .absorb_scalar(b"d", VegaT256ScalarV1::from_u64(9))
            .expect("bounded");
        assert_ne!(
            production_challenge,
            other_label.squeeze(b"r").expect("round available")
        );
    }
    #[test]
    fn oversize_and_round_overflow_are_rejected() {
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        assert_eq!(
            transcript.absorb_raw(b"x", &vec![0; MAX_PENDING_BYTES]),
            Err(VegaTranscriptError::PendingMaterialTooLarge)
        );
        transcript.round = u16::MAX;
        assert_eq!(
            transcript.squeeze(b"r"),
            Err(VegaTranscriptError::RoundCounterExhausted)
        );
    }
    #[test]
    fn scalar_and_commitment_representations_are_fixed_width_and_unambiguous() {
        let scalar = VegaT256ScalarV1::from_u64(1);
        let point = VegaT256PointV1::canonical_generator().expect("canonical generator");
        let commitment = Commitment::from_points(vec![point]).expect("non-identity commitment");
        let mut typed = VegaTranscriptV1::new_raw(b"typed");
        typed.absorb_scalar(b"s", scalar).expect("bounded");
        typed.absorb_commitment(b"p", &commitment).expect("bounded");
        let mut explicit = VegaTranscriptV1::new_raw(b"typed");
        explicit
            .absorb_raw(b"s", &scalar.to_be_bytes())
            .expect("bounded");
        explicit
            .absorb_raw(
                b"p",
                &commitment.transcript_bytes().expect("canonical commitment"),
            )
            .expect("bounded");
        assert_eq!(
            typed.squeeze(b"r").expect("round available"),
            explicit.squeeze(b"r").expect("round available")
        );
    }
}
