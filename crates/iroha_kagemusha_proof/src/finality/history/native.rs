//! One finite original Genesis/Append/shared-wrapper catalog for every height.
//!
//! Both source tables are imported against their actual compiled owners before
//! they can enter the two-key wrapper catalog. No online key-generation or
//! per-receipt catalog approval is involved.

use super::{GenesisSourceCircuit, HistoryAnchor, HistoryAppendCircuit, HistoryAppendPlan};
use crate::finality::continuity::{
    SourceNodeEvidence, SourceVerifier,
    producer::{Error, ImportedSource, OriginalArtifact, Prover},
    tree::NodeRandomness,
};
use iroha_pasta::{Ep, Eq};
use iroha_plonk::{
    DescriptorBinding, ProverConfig, keys::pk::artifact::ReadConfig, pcs::ipa::PinnedParams,
};
use iroha_plonk_recursion::FoldConfig;

/// Exactly three original artifacts, independently installed for one anchor/body source.
#[derive(Clone, Copy)]
pub struct HistoryArtifacts<'a> {
    /// Original genesis source tables; its carried wrapper key is a witness.
    pub genesis: OriginalArtifact<'a>,
    /// Original append source tables, depending only on the prior wrapper layout.
    pub append: OriginalArtifact<'a>,
    /// Shared wrapper admitting exactly those two original source keys in order.
    pub wrapper: OriginalArtifact<'a>,
}

/// Finite source-qualified history producer, usable for every native u64 height.
pub struct HistoryProver {
    anchor: HistoryAnchor,
    append_plan: HistoryAppendPlan,
    genesis: Prover<GenesisSourceCircuit>,
    append: Prover<HistoryAppendCircuit>,
    vesta: PinnedParams<Eq>,
}
impl HistoryProver {
    /// Mount the exact closed two-source catalog, rejecting any descriptor/key substitution.
    /// The caller independently authenticates the signed genesis and body source.
    /// # Errors
    /// Wrong original source tables, profile, catalog ordering, wrapper layout or limits.
    pub fn from_original_artifacts(
        anchor: HistoryAnchor,
        body: SourceVerifier,
        artifacts: HistoryArtifacts<'_>,
        pallas: PinnedParams<Ep>,
        vesta: PinnedParams<Eq>,
        config: ReadConfig,
    ) -> Result<Self, Error> {
        Self::from_original_artifacts_cancellable(
            anchor, body, artifacts, pallas, vesta, config, None,
        )
    }
    /// Import the exact finite history catalog with an operation cancellation signal.
    /// # Errors
    /// As [`Self::from_original_artifacts`], or cancellation without a source verdict.
    pub fn from_original_artifacts_cancellable(
        anchor: HistoryAnchor,
        body: SourceVerifier,
        artifacts: HistoryArtifacts<'_>,
        pallas: PinnedParams<Ep>,
        vesta: PinnedParams<Eq>,
        config: ReadConfig,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<Self, Error> {
        iroha_pasta::CancellationToken::checkpoint(cancellation)?;
        if artifacts.wrapper.descriptor.is_empty() || artifacts.wrapper.descriptor.len() > 1 << 20 {
            return Err(Error::Artifact);
        }
        let binding = DescriptorBinding::decode_v2(artifacts.wrapper.descriptor)
            .map_err(|_| Error::Artifact)?;
        let append_plan = HistoryAppendPlan::new(anchor, binding, body, pallas.clone())
            .map_err(|_| Error::Artifact)?;
        let genesis_layout = GenesisSourceCircuit::for_source(anchor);
        let append_layout =
            HistoryAppendCircuit::for_source(append_plan.clone()).map_err(|_| Error::Artifact)?;
        let genesis = ImportedSource::from_original_cancellable(
            &genesis_layout,
            artifacts.genesis,
            &vesta,
            config,
            cancellation,
        )?;
        let append = ImportedSource::from_original_cancellable(
            &append_layout,
            artifacts.append,
            &vesta,
            config,
            cancellation,
        )?;
        if genesis.binding().encoded() != append.binding().encoded() {
            return Err(Error::Artifact);
        }
        let catalog = [genesis.catalog_key(), append.catalog_key()];
        let genesis = Prover::from_qualified_catalog_cancellable(
            genesis,
            artifacts.wrapper,
            &catalog,
            pallas.clone(),
            vesta.clone(),
            config,
            cancellation,
        )?;
        let append = Prover::from_qualified_catalog_cancellable(
            append,
            artifacts.wrapper,
            &catalog,
            pallas,
            vesta.clone(),
            config,
            cancellation,
        )?;
        let mounted_descriptor = genesis.binding().encoded();
        let required_descriptor = append_plan.wrapper_binding().encoded();
        if mounted_descriptor != required_descriptor {
            return Err(Error::Artifact);
        }
        if genesis.verifying_key().to_bytes() != append.verifying_key().to_bytes() {
            return Err(Error::Artifact);
        }
        Ok(Self {
            anchor,
            append_plan,
            genesis,
            append,
            vesta,
        })
    }
    /// Exact finite shared wrapper to install into the terminal receipt source.
    /// # Errors
    /// Invalid imported key profile.
    pub fn qualified_source(&self) -> Result<SourceVerifier, Error> {
        self.genesis.qualified_source()
    }
    /// Prove the fixed genesis state under this catalog's actual carried wrapper digest.
    /// # Errors
    /// Invalid witnesses, source-key mismatch or failed proof/claim decision.
    pub fn genesis(
        &self,
        entropy: NodeRandomness<'_>,
        config: ProverConfig,
    ) -> Result<SourceNodeEvidence, Error> {
        let key = self
            .qualified_source()?
            .key_digest()
            .map_err(|_| Error::Artifact)?;
        let circuit = GenesisSourceCircuit::new(self.anchor, key);
        self.genesis.prove(
            &circuit,
            entropy.outer_salt,
            entropy.source,
            entropy.wrapper,
            config,
        )
    }
    /// Append a complete original block body to an already verified genesis prefix.
    /// # Errors
    /// Changed anchor/key/context, discontinuous state, invalid body or proof/fold failure.
    pub fn append(
        &self,
        previous: SourceNodeEvidence,
        body: SourceNodeEvidence,
        entropy: NodeRandomness<'_>,
        config: ProverConfig,
        fold: &FoldConfig,
    ) -> Result<SourceNodeEvidence, Error> {
        let config = ProverConfig {
            cancellation: config.cancellation.or(fold.cancellation.as_ref()),
            ..config
        };
        iroha_pasta::CancellationToken::checkpoint(config.cancellation)?;
        let mut fold = fold.clone();
        fold.cancellation = config.cancellation.cloned();
        let circuit = HistoryAppendCircuit::prepare(
            self.append_plan.clone(),
            self.genesis.verifying_key().clone(),
            [previous, body],
            &self.vesta,
            entropy.inner_salt,
            &fold,
        )
        .map_err(|error| {
            if matches!(error, iroha_plonk::frontend::Error::Cancelled) {
                Error::Cancelled
            } else {
                Error::Proof
            }
        })?;
        self.append.prove(
            &circuit,
            entropy.outer_salt,
            entropy.source,
            entropy.wrapper,
            config,
        )
    }
}

#[cfg(test)]
mod tests;
