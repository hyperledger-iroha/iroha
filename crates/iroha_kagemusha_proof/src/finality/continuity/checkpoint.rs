//! Untrusted durable source checkpoints, reverified under exact installed keys.
//!
//! Checkpoints restore a previously proved fact; they do not shorten its interval
//! or substitute host-selected claims. Storage must bound reads before allocation.

use ff::PrimeField;
use iroha_pasta::{Eq, Fp, msm::MemoryBudget};
use iroha_plonk::pcs::ipa::PinnedParams;

use super::{SourceNodeEvidence, SourceVerifier, producer::Error, tree::SourceIdentity};

/// Complete identity of one source fact under an independently installed key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProofIdentity {
    /// Exact qualified wrapper descriptor and verifying-key digest.
    pub source: SourceIdentity,
    /// Program, context, interval start/end and state before/after, canonically encoded.
    pub endpoints: [[u8; 32]; 6],
}
impl ProofIdentity {
    /// Derive the storage identity from installed metadata and all expected endpoints.
    /// # Errors
    /// Invalid installed key digest.
    pub fn new(source: &SourceVerifier, endpoints: [Fp; 6]) -> Result<Self, Error> {
        Ok(Self {
            source: SourceIdentity {
                descriptor: *source.binding().digest(),
                key: source.key_digest().map_err(|_| Error::Artifact)?.to_repr(),
            },
            endpoints: endpoints.map(|word| word.to_repr()),
        })
    }
}

/// Bounded untrusted storage for exact source proof bytes and both original claims.
/// Absence is distinct from malformed, unavailable or changed stored data.
pub trait ProofCheckpointStore {
    /// Read a complete candidate under finite byte bounds, or report genuine absence.
    /// # Errors
    /// Unavailable storage, changed identity, malformed bytes or excessive bounds.
    fn load(&mut self, identity: &ProofIdentity) -> Result<Option<SourceNodeEvidence>, Error>;
    /// Durably retain a newly verified complete proof; never replace differing originals.
    /// # Errors
    /// Storage failure, excessive bounds or an existing different retained payload.
    fn store(&mut self, identity: &ProofIdentity, proof: &SourceNodeEvidence) -> Result<(), Error>;
}

/// Restore only a complete proof that passes the exact source and both claim decisions.
/// # Errors
/// Changed endpoints, malformed proof/claims, foreign key or storage failure.
pub fn restore(
    store: &mut dyn ProofCheckpointStore,
    source: &SourceVerifier,
    endpoints: [Fp; 6],
    params: &PinnedParams<Eq>,
    budget: MemoryBudget,
) -> Result<Option<SourceNodeEvidence>, Error> {
    let identity = ProofIdentity::new(source, endpoints)?;
    let Some(proof) = store.load(&identity)? else {
        return Ok(None);
    };
    if proof.endpoints != endpoints {
        return Err(Error::Input);
    }
    let _opening = source
        .verify_native(&proof, params, budget)
        .map_err(|_| Error::Proof)?;
    Ok(Some(proof))
}

/// Verify and persist every original obligation before exposing a durable checkpoint.
/// # Errors
/// Invalid proof/claims, changed identity or storage failure.
pub fn retain(
    store: &mut dyn ProofCheckpointStore,
    source: &SourceVerifier,
    proof: &SourceNodeEvidence,
    params: &PinnedParams<Eq>,
    budget: MemoryBudget,
) -> Result<(), Error> {
    let _opening = source
        .verify_native(proof, params, budget)
        .map_err(|_| Error::Proof)?;
    store.store(&ProofIdentity::new(source, proof.endpoints)?, proof)
}

#[cfg(test)]
pub(crate) fn exercise_actual_checkpoint(
    source: &SourceVerifier,
    proof: &SourceNodeEvidence,
    params: &PinnedParams<Eq>,
) {
    use ff::Field;
    use iroha_pasta::{Ep, Fq};
    use iroha_plonk_recursion::AccumulatorT;
    #[derive(Default)]
    struct Memory {
        value: Option<SourceNodeEvidence>,
        unavailable: bool,
    }
    impl ProofCheckpointStore for Memory {
        fn load(&mut self, _: &ProofIdentity) -> Result<Option<SourceNodeEvidence>, Error> {
            if self.unavailable {
                Err(Error::Artifact)
            } else {
                Ok(self.value.clone())
            }
        }
        fn store(&mut self, _: &ProofIdentity, value: &SourceNodeEvidence) -> Result<(), Error> {
            self.value = Some(value.clone());
            Ok(())
        }
    }
    let budget = MemoryBudget::DEFAULT;
    let mut store = Memory::default();
    assert!(
        restore(&mut store, source, proof.endpoints, params, budget)
            .unwrap()
            .is_none()
    );
    retain(&mut store, source, proof, params, budget).unwrap();
    let restored = restore(&mut store, source, proof.endpoints, params, budget)
        .unwrap()
        .unwrap();
    assert_eq!(restored.proof, proof.proof);
    assert_eq!(restored.pallas, proof.pallas);
    assert_eq!(restored.vesta, proof.vesta);
    for i in 0..6 {
        let mut endpoints = proof.endpoints;
        endpoints[i] += Fp::ONE;
        assert!(restore(&mut store, source, endpoints, params, budget).is_err());
    }
    for i in 0..3 {
        let mut changed = proof.clone();
        match i {
            0 => changed.proof[32] ^= 1,
            1 => {
                let mut challenges = *changed.pallas.challenges();
                challenges[0] += Fq::ONE;
                changed.pallas = AccumulatorT::<Ep>::new(*changed.pallas.g(), challenges).unwrap();
            }
            _ => {
                let mut challenges = *changed.vesta.challenges();
                challenges[0] += Fp::ONE;
                changed.vesta = AccumulatorT::<Eq>::new(*changed.vesta.g(), challenges).unwrap();
            }
        }
        store.value = Some(changed);
        assert!(restore(&mut store, source, proof.endpoints, params, budget).is_err());
    }
    store.value = None;
    store.unavailable = true;
    assert!(restore(&mut store, source, proof.endpoints, params, budget).is_err());
}
