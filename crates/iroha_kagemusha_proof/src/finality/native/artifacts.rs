//! Bounded independently selected original artifact addressing and admission.

use super::*;
use crate::finality::continuity::{
    producer::OriginalArtifact,
    tree::{OriginalBytes, OriginalPair, SourceIdentity},
};
use std::collections::BTreeMap;

/// The six fixed leaf programs of the ordinary finality source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Program {
    /// Exact native BLS signature verification.
    Bls,
    /// Ordered committee aggregation.
    Aggregation,
    /// Complete canonical result hashing.
    Result,
    /// Complete result/schedule byte parsing.
    Schedule,
    /// Exact native epoch CRC and hashing.
    Context,
    /// Original receipt and counted event inclusion.
    Load,
}
/// Fixed two-child composition owners, with child order determined by the code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Composition {
    /// Complete aggregation and BLS programs.
    Certificate,
    /// Certificate and full result scan.
    CertifiedResult,
    /// Schedule parser and native context hash.
    Schedule,
    /// Certified result and its current schedule.
    ScheduledResult,
    /// Scheduled result and its authorized successor schedule.
    HistoryStep,
    /// Complete history and successful Load event.
    Receipt,
}
/// Source identity in the independently installed artifact graph.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeId {
    /// Compiled leaf source class, named by its first program position.
    /// Positions with identical witnessless source layouts share this entry.
    Leaf(Program, u32),
    /// Generic interval merge of this exact ordered child-key pair.
    Merge(Box<[SourceIdentity; 2]>),
    /// Execution ordinal for merge randomness; artifact lookup uses `Merge` identities.
    ProgramMerge(Program, u32),
    /// Fixed semantic composition.
    Composition(Composition),
    /// Fixed signed-genesis state.
    Genesis,
    /// Finite history continuation.
    Append,
}
/// One fixed compiled original identity; no runtime witness or caller selects a new key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArtifactId {
    /// Original Vesta source tables and verifying key.
    Source(NodeId),
    /// Original mandatory Pallas wrapper.
    Wrapper(NodeId),
    /// Shared history wrapper admitting exactly original Genesis and Append.
    HistoryWrapper,
}
/// Storage loader selected by the installation owner, not by a receipt proposal.
/// Implementations must bound reads before allocating returned buffers. All
/// returned material is independently source-qualified; this trait grants no trust.
pub trait ArtifactSource {
    /// Read exactly one original descriptor, verifying key and proving table set.
    /// # Errors
    /// Missing independently installed material, malformed storage or resource refusal.
    fn load(&mut self, id: &ArtifactId) -> Result<OriginalBytes, Error>;
}

pub(super) fn borrowed(bytes: &OriginalBytes) -> OriginalArtifact<'_> {
    OriginalArtifact {
        descriptor: &bytes.descriptor,
        verifying_key: &bytes.verifying_key,
        proving_key: &bytes.proving_key,
    }
}
pub(super) fn load_pair(
    source: &mut dyn ArtifactSource,
    id: &NodeId,
) -> Result<OriginalPair, Error> {
    Ok(OriginalPair {
        source: source.load(&ArtifactId::Source(id.clone()))?,
        wrapper: source.load(&ArtifactId::Wrapper(id.clone()))?,
    })
}
pub(super) struct Admission<'a> {
    source: &'a mut dyn ArtifactSource,
    limits: ImportLimits,
    visited: BTreeMap<ArtifactId, usize>,
    bytes: usize,
}
impl<'a> Admission<'a> {
    pub(super) fn new(
        source: &'a mut dyn ArtifactSource,
        limits: ImportLimits,
    ) -> Result<Self, Error> {
        if limits.maximum_artifacts == 0
            || limits.maximum_artifacts == usize::MAX
            || limits.maximum_original_bytes == 0
            || limits.maximum_original_bytes == usize::MAX
            || limits.key.maximum_bytes == 0
            || limits.key.maximum_bytes == usize::MAX
            || limits.key.maximum_rows < 1 << 16
        {
            return Err(Error::Artifact);
        }
        Ok(Self {
            source,
            limits,
            visited: BTreeMap::new(),
            bytes: 0,
        })
    }
}
impl ArtifactSource for Admission<'_> {
    fn load(&mut self, id: &ArtifactId) -> Result<OriginalBytes, Error> {
        if !self.visited.contains_key(id) && self.visited.len() >= self.limits.maximum_artifacts {
            return Err(Error::Artifact);
        }
        let bytes = self.source.load(id)?;
        if bytes.descriptor.is_empty()
            || bytes.descriptor.len() > 1 << 20
            || bytes.verifying_key.is_empty()
            || bytes.verifying_key.len() > 1 << 18
            || bytes.proving_key.is_empty()
            || bytes.proving_key.len() > self.limits.key.maximum_bytes
        {
            return Err(Error::Artifact);
        }
        let length = bytes
            .descriptor
            .len()
            .checked_add(bytes.verifying_key.len())
            .and_then(|n| n.checked_add(bytes.proving_key.len()))
            .ok_or(Error::Artifact)?;
        if let Some(previous) = self.visited.get(id) {
            if *previous != length {
                return Err(Error::Artifact);
            }
        } else {
            self.bytes = self
                .bytes
                .checked_add(length)
                .filter(|n| *n <= self.limits.maximum_original_bytes)
                .ok_or(Error::Artifact)?;
            self.visited.insert(id.clone(), length);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_plonk::keys::CosetCachePolicy;
    struct Bytes(usize);
    impl ArtifactSource for Bytes {
        fn load(&mut self, _: &ArtifactId) -> Result<OriginalBytes, Error> {
            Ok(OriginalBytes {
                descriptor: vec![1],
                verifying_key: vec![2],
                proving_key: vec![3; self.0],
            })
        }
    }
    fn limits() -> ImportLimits {
        ImportLimits {
            key: ReadConfig {
                maximum_bytes: 8,
                maximum_rows: 1 << 16,
                coset_cache: CosetCachePolicy::OnDemand,
                msm_budget: MemoryBudget::DEFAULT,
            },
            maximum_artifacts: 2,
            maximum_original_bytes: 8,
        }
    }
    #[test]
    fn repeated_inventory_entry_keeps_exact_size_without_resetting_total() {
        // These are resource-accounting bytes, not source artifacts or proof evidence.
        let mut bytes = Bytes(2);
        let mut admission = Admission::new(&mut bytes, limits()).unwrap();
        let first = ArtifactId::Source(NodeId::Genesis);
        let second = ArtifactId::Source(NodeId::Append);
        assert!(admission.load(&first).is_ok());
        assert!(admission.load(&first).is_ok());
        assert_eq!(admission.bytes, 4);
        assert!(admission.load(&second).is_ok());
        assert_eq!(admission.bytes, 8);
        assert!(admission.load(&ArtifactId::HistoryWrapper).is_err());
    }
    #[test]
    fn changed_inventory_size_and_total_overflow_are_refused() {
        struct Changing(u8);
        impl ArtifactSource for Changing {
            fn load(&mut self, _: &ArtifactId) -> Result<OriginalBytes, Error> {
                self.0 += 1;
                Bytes(usize::from(self.0)).load(&ArtifactId::HistoryWrapper)
            }
        }
        let mut changing = Changing(1);
        let mut admission = Admission::new(&mut changing, limits()).unwrap();
        let id = ArtifactId::Source(NodeId::Genesis);
        assert!(admission.load(&id).is_ok());
        assert!(admission.load(&id).is_err());
        assert_eq!(admission.bytes, 4);
        let mut bytes = Bytes(3);
        let mut admission = Admission::new(&mut bytes, limits()).unwrap();
        assert!(admission.load(&id).is_ok());
        assert!(admission.load(&ArtifactId::HistoryWrapper).is_err());
        assert_eq!(admission.bytes, 5);
    }
}
