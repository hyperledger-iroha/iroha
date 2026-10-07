//! Exact per-block reuse of an already proved context scan, with full re-verification.

use super::super::*;
use crate::finality::schedule::context_hash::{self, ContextHashInput, boundary_digest_native};
use iroha_pasta::Eq;
use iroha_plonk::pcs::ipa::PinnedParams;

pub(super) struct ProvedContext {
    input: ContextHashInput,
    evidence: SourceNodeEvidence,
}
fn endpoints(input: &ContextHashInput) -> [Fp; 6] {
    [
        Fp::from(context_hash::PROGRAM_ID),
        input.digest(),
        Fp::from(0),
        Fp::from(u64::from(context_hash::PROGRAM_LENGTH)),
        boundary_digest_native(input, false),
        boundary_digest_native(input, true),
    ]
}
impl ProvedContext {
    /// Called only after the installed complete program actually proves its input.
    pub(super) fn new(
        input: ContextHashInput,
        evidence: SourceNodeEvidence,
    ) -> Result<Self, Error> {
        if evidence.endpoints != endpoints(&input) {
            return Err(Error::Input);
        }
        Ok(Self { input, evidence })
    }

    // Selection grants no proof authority. Exact native source verification below
    // remains mandatory even for a cache created by this same proving session.
    fn candidate(&self, input: &ContextHashInput) -> Result<Option<&SourceNodeEvidence>, Error> {
        if self.input != *input {
            return Ok(None);
        }
        if self.evidence.endpoints != endpoints(input) {
            return Err(Error::Input);
        }
        Ok(Some(&self.evidence))
    }

    pub(super) fn reuse(
        &self,
        input: &ContextHashInput,
        source: &SourceVerifier,
        vesta: &PinnedParams<Eq>,
        budget: MemoryBudget,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<Option<SourceNodeEvidence>, Error> {
        iroha_pasta::CancellationToken::checkpoint(cancellation)?;
        let Some(evidence) = self.candidate(input)? else {
            return Ok(None);
        };
        let _opening = source
            .verify_native_cancellable(evidence, vesta, budget, cancellation)
            .map_err(|error| {
                if matches!(error, iroha_plonk::frontend::Error::Cancelled) {
                    Error::Cancelled
                } else {
                    Error::Proof
                }
            })?;
        Ok(Some(evidence.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_plonk_recursion::AccumulatorT;

    fn input() -> ContextHashInput {
        ContextHashInput {
            tape_root: Fp::from(7),
            result_len: 4096,
            payload_start: 128,
            payload_len: 512,
            checksum: 13,
            context_id: [17; 32],
        }
    }
    fn evidence(input: &ContextHashInput) -> SourceNodeEvidence {
        // Selection-only sample. It is never passed to reuse or any acceptance
        // assertion; the deliberately invalid proof confers no source authority.
        SourceNodeEvidence {
            endpoints: endpoints(input),
            proof: vec![0],
            pallas: AccumulatorT::trivial(
                &PinnedParams::derive(16).unwrap(),
                MemoryBudget::DEFAULT,
            )
            .unwrap(),
            vesta: AccumulatorT::trivial(&PinnedParams::derive(16).unwrap(), MemoryBudget::DEFAULT)
                .unwrap(),
        }
    }

    #[test]
    fn selection_requires_every_exact_context_field_and_byte() {
        let original = input();
        let cache = ProvedContext::new(original, evidence(&original)).unwrap();
        assert!(std::ptr::eq(
            cache.candidate(&original).unwrap().unwrap(),
            &raw const cache.evidence
        ));
        for changed in 0..38 {
            let mut input = original;
            match changed {
                0 => input.tape_root += Fp::from(1),
                1 => input.result_len += 1,
                2 => input.payload_start += 1,
                3 => input.payload_len += 1,
                4 => input.checksum ^= 1,
                5..=36 => input.context_id[changed - 5] ^= 1,
                _ => {
                    input.payload_start += 1;
                    input.payload_len -= 1;
                }
            }
            assert!(cache.candidate(&input).unwrap().is_none());
        }
        assert_eq!(
            cache.evidence.proof,
            [0],
            "selection never claims to accept proof bytes"
        );
    }

    #[test]
    fn changed_program_context_interval_or_state_cannot_enter_reuse() {
        let input = input();
        let original = evidence(&input);
        assert_eq!(original.endpoints[3], Fp::from(2_561));
        let mut ordinal = original.clone();
        ordinal.endpoints[3] = Fp::from(u64::from(context_hash::BATCH_LENGTH));
        assert!(ProvedContext::new(input, ordinal).is_err());
        for index in 0..6 {
            let mut changed = original.clone();
            changed.endpoints[index] += Fp::from(1);
            assert!(ProvedContext::new(input, changed).is_err());
            let mut cache = ProvedContext::new(input, original.clone()).unwrap();
            cache.evidence.endpoints[index] += Fp::from(1);
            assert!(cache.candidate(&input).is_err());
        }
    }
}
