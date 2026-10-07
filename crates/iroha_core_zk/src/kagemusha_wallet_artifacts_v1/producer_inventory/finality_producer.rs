//! Actual complete finality producer from the authenticated original catalog.

use super::*;
use iroha_kagemusha_proof::finality::{
    continuity::{SourceNodeEvidence, tree::OriginalBytes},
    native::{
        ArtifactId, ArtifactSource, BlockWitnessInput, HistoryPrefix, ImportLimits,
        InstalledFinality, LoadWitnessInput, Parameters, ProvingContext,
    },
};
use iroha_pasta::msm::MemoryBudget;
use iroha_plonk::{ProverConfig, ProverRandomness, keys::pk::artifact::ReadConfig};
use iroha_plonk_recursion::FoldConfig;

/// The selected originals could not produce the complete installed finality graph.
#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum FinalityProducerErrorV1 {
    /// A selected original was absent, changed, oversized or unavailable.
    #[error(transparent)]
    Original(#[from] Error),
    /// A compiled source, native input or actual proof failed.
    #[error(transparent)]
    Source(#[from] iroha_kagemusha_proof::finality::native::Error),
}

struct Reader<'a> {
    records: &'a [ArtifactRecord],
    originals: &'a mut dyn OriginalSourceV1,
    maximum_pk_bytes: usize,
    failure: Option<Error>,
}
impl ArtifactSource for Reader<'_> {
    fn load(
        &mut self,
        id: &ArtifactId,
    ) -> Result<OriginalBytes, iroha_kagemusha_proof::finality::native::Error> {
        let result = (|| {
            let record = self
                .records
                .iter()
                .find(|record| record.matches_identity(id) == Ok(true))
                .ok_or(Error::Inventory)?;
            let blob = |index| BlobV1 {
                bytes: record.lengths[index],
                sha256: record.sha256[index],
            };
            Ok(OriginalBytes {
                descriptor: read(self.originals, blob(0), DESCRIPTOR_MAX_BYTES_V1)?,
                verifying_key: read(self.originals, blob(1), VERIFYING_KEY_MAX_BYTES_V1)?,
                proving_key: read(self.originals, blob(2), self.maximum_pk_bytes)?,
            })
        })();
        result.map_err(|error| {
            self.failure = Some(error);
            iroha_kagemusha_proof::finality::native::Error::Artifact
        })
    }
}

/// Full source-qualified graph; each proof imports its same signed original tables.
/// This owner cannot be constructed from a verifier-only receipt capability.
pub struct QualifiedFinalityProducerV1 {
    installed: InstalledFinality,
    records: Vec<ArtifactRecord>,
    read: ReadConfig,
}
impl QualifiedFinalityProducerV1 {
    pub(super) fn mount(
        inventory: &AuthenticatedProducerInventoryV1,
        anchor: iroha_kagemusha_proof::finality::history::HistoryAnchor,
        originals: &mut dyn OriginalSourceV1,
        parameters: Parameters,
        read_config: ReadConfig,
    ) -> Result<Self, FinalityProducerErrorV1> {
        let records = &inventory.inventory.finality.originals;
        let total = records.iter().try_fold(0usize, |sum, record| {
            record.lengths.iter().try_fold(sum, |sum, bytes| {
                sum.checked_add(usize::try_from(*bytes).map_err(|_| Error::Inventory)?)
                    .ok_or(Error::Inventory)
            })
        })?;
        let mut reader = Reader {
            records,
            originals,
            maximum_pk_bytes: read_config.maximum_bytes,
            failure: None,
        };
        let result = InstalledFinality::from_original_artifacts(
            anchor,
            &mut reader,
            parameters,
            ImportLimits {
                key: read_config,
                maximum_artifacts: records.len(),
                maximum_original_bytes: total,
            },
        );
        if let Some(error) = reader.failure {
            return Err(error.into());
        }
        Ok(Self {
            installed: result?,
            records: records.clone(),
            read: read_config,
        })
    }

    /// Exact installed graph; its prefix restoration re-verifies the complete original proof.
    pub const fn installed(&self) -> &InstalledFinality {
        &self.installed
    }

    fn prove<T>(
        &self,
        originals: &mut dyn OriginalSourceV1,
        budget: MemoryBudget,
        operation: impl FnOnce(
            &InstalledFinality,
            &mut ProvingContext<'_, '_>,
        ) -> Result<T, iroha_kagemusha_proof::finality::native::Error>,
    ) -> Result<T, FinalityProducerErrorV1> {
        use ff::Field;
        let mut reader = Reader {
            records: &self.records,
            originals,
            maximum_pk_bytes: self.read.maximum_bytes,
            failure: None,
        };
        let mut randomness = |_| {
            Ok(
                iroha_kagemusha_proof::finality::continuity::tree::NodeRandomness {
                    inner_salt: Fp::random(rand_core_06::OsRng),
                    outer_salt: rand::random(),
                    source: ProverRandomness::os(),
                    wrapper: ProverRandomness::os(),
                },
            )
        };
        let fold = FoldConfig {
            kernel_budget: budget,
            ..FoldConfig::default()
        };
        let mut context = ProvingContext::new(
            &mut reader,
            &mut randomness,
            ProverConfig {
                msm_budget: budget,
                cancellation: None,
            },
            &fold,
        );
        let result = operation(&self.installed, &mut context);
        drop(context);
        if let Some(error) = reader.failure {
            return Err(error.into());
        }
        Ok(result?)
    }

    /// Produce the actual fixed genesis proof under the exact installed source graph.
    /// # Errors
    /// Original source custody, entropy, proof or finite resource failure.
    pub fn genesis(
        &self,
        originals: &mut dyn OriginalSourceV1,
        budget: MemoryBudget,
    ) -> Result<HistoryPrefix, FinalityProducerErrorV1> {
        self.prove(originals, budget, |graph, context| graph.genesis(context))
    }

    /// Prove one contiguous block, retaining all native history obligations.
    /// # Errors
    /// Changed originals, invalid native witness, discontinuity or actual proof failure.
    pub fn append(
        &self,
        originals: &mut dyn OriginalSourceV1,
        previous: &HistoryPrefix,
        input: &BlockWitnessInput,
        budget: MemoryBudget,
    ) -> Result<HistoryPrefix, FinalityProducerErrorV1> {
        self.prove(originals, budget, |graph, context| {
            graph.append_block(previous, input, context)
        })
    }

    /// Produce the exact successful Load event proof over its complete authenticated prefix.
    /// # Errors
    /// Wrong prefix, original receipt/path, source custody or actual proof failure.
    pub fn receipt(
        &self,
        originals: &mut dyn OriginalSourceV1,
        prefix: &HistoryPrefix,
        input: &LoadWitnessInput,
        budget: MemoryBudget,
    ) -> Result<SourceNodeEvidence, FinalityProducerErrorV1> {
        self.prove(originals, budget, |graph, context| {
            graph.prove_receipt(prefix, input, context)
        })
    }
}
