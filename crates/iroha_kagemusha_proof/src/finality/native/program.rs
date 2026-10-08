//! Metadata-only installed leaves and complete fixed interval programs.

use super::artifacts::{borrowed, load_pair};
use super::*;
use crate::finality::continuity::{
    producer::{Prover, SourceCircuit},
    tree::{IntervalTree, SourceIdentity, TreeImportConfig},
};
use core::marker::PhantomData;
use ff::PrimeField;
use std::collections::BTreeMap;

pub(super) fn identity(source: &SourceVerifier) -> Result<SourceIdentity, Error> {
    Ok(SourceIdentity {
        descriptor: *source.binding().digest(),
        key: source.key_digest().map_err(|_| Error::Artifact)?.to_repr(),
    })
}
/// Internal graph assembly mode. Only a complete opaque catalog can select the
/// metadata-only arm; untrusted artifact loaders always take strict original import.
pub(super) enum GraphSource<'a> {
    Original(&'a mut dyn ArtifactSource),
    Qualified(&'a crate::finality::catalog::ServerRecipes),
}

pub(super) struct Mounted<C: SourceCircuit> {
    id: NodeId,
    pub(super) source: SourceVerifier,
    circuit: PhantomData<C>,
}
impl<C: SourceCircuit> Mounted<C> {
    pub(super) fn mount(
        id: NodeId,
        layout: &C,
        artifacts: &mut GraphSource<'_>,
        params: &Parameters,
        limits: ImportLimits,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<Self, Error> {
        iroha_pasta::CancellationToken::checkpoint(cancellation)?;
        if let GraphSource::Qualified(catalog) = artifacts {
            return Ok(Self {
                source: catalog.selected_source(&id)?,
                id,
                circuit: PhantomData,
            });
        }
        let GraphSource::Original(originals) = artifacts else {
            return Err(Error::Artifact);
        };
        let pair = load_pair(*originals, &id)?;
        let prover = Prover::from_original_artifacts_cancellable(
            layout,
            borrowed(&pair.source),
            borrowed(&pair.wrapper),
            params.pallas.clone(),
            params.vesta.clone(),
            limits.key,
            cancellation,
        )?;
        Ok(Self {
            id,
            source: prover.qualified_source()?,
            circuit: PhantomData,
        })
    }
    pub(super) fn prove(
        &self,
        witness: &C,
        params: &Parameters,
        limits: ImportLimits,
        context: &mut ProvingContext<'_, '_>,
    ) -> Result<SourceNodeEvidence, Error> {
        let entropy = context.entropy(self.id.clone())?;
        self.prove_with(witness, params, limits, context, entropy)
    }
    pub(super) fn prepare_and_prove(
        &self,
        params: &Parameters,
        limits: ImportLimits,
        context: &mut ProvingContext<'_, '_>,
        prepare: impl FnOnce(Fp, &FoldConfig) -> Result<C, iroha_plonk::frontend::Error>,
    ) -> Result<SourceNodeEvidence, Error> {
        let entropy = context.entropy(self.id.clone())?;
        iroha_pasta::CancellationToken::checkpoint(context.proof.cancellation)?;
        let mut fold = context.fold.clone();
        fold.cancellation = context.proof.cancellation.cloned();
        let witness = circuit(prepare(entropy.inner_salt, &fold))?;
        self.prove_with(&witness, params, limits, context, entropy)
    }
    pub(super) fn prove_with(
        &self,
        witness: &C,
        params: &Parameters,
        limits: ImportLimits,
        context: &mut ProvingContext<'_, '_>,
        entropy: crate::finality::continuity::tree::NodeRandomness<'_>,
    ) -> Result<SourceNodeEvidence, Error> {
        iroha_pasta::CancellationToken::checkpoint(context.proof.cancellation)?;
        let exports = witness.exports(
            &params.pallas,
            &params.vesta,
            context.proof.msm_budget,
            context.proof.cancellation,
        )?;
        if let Some(proof) =
            context.restore_source(&self.source, exports.endpoints, &params.vesta)?
        {
            return Ok(proof);
        }
        let pair = load_pair(context.artifacts, &self.id)?;
        let prover = Prover::from_original_artifacts_cancellable(
            witness,
            borrowed(&pair.source),
            borrowed(&pair.wrapper),
            params.pallas.clone(),
            params.vesta.clone(),
            limits.key,
            context.proof.cancellation,
        )?;
        if identity(&prover.qualified_source()?)? != identity(&self.source)? {
            return Err(Error::Artifact);
        }
        let proof = prover.prove(
            witness,
            entropy.outer_salt,
            entropy.source,
            entropy.wrapper,
            context.proof,
        )?;
        context.retain_source(&self.source, &proof, &params.vesta)?;
        Ok(proof)
    }
}

pub(super) struct InstalledProgram<C: SourceCircuit> {
    program: Program,
    leaves: BTreeMap<u32, Mounted<C>>,
    sequence: Vec<u32>,
    tree: IntervalTree,
}
impl<C: SourceCircuit> InstalledProgram<C> {
    pub(super) fn mount(
        program: Program,
        count: u32,
        mut layout: impl FnMut(u32) -> Result<(u32, C), Error>,
        artifacts: &mut GraphSource<'_>,
        params: &Parameters,
        limits: ImportLimits,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<Self, Error> {
        iroha_pasta::CancellationToken::checkpoint(cancellation)?;
        let count_usize = usize::try_from(count).map_err(|_| Error::Artifact)?;
        if !(2..=crate::finality::continuity::tree::MAX_PROGRAM_LEAVES).contains(&count_usize) {
            return Err(Error::Artifact);
        }
        let mut leaves = BTreeMap::new();
        let mut sequence = Vec::with_capacity(count_usize);
        for cursor in 0..count {
            iroha_pasta::CancellationToken::checkpoint(cancellation)?;
            // This map comes only from the compiled program owner below. In
            // particular no offered VK, descriptor or proof selects a source class.
            let (class, source) = layout(cursor)?;
            if class >= count {
                return Err(Error::Artifact);
            }
            if let std::collections::btree_map::Entry::Vacant(entry) = leaves.entry(class) {
                entry.insert(Mounted::mount(
                    NodeId::Leaf(program, class),
                    &source,
                    artifacts,
                    params,
                    limits,
                    cancellation,
                )?);
            }
            sequence.push(class);
        }
        let sources = sequence
            .iter()
            .map(|class| leaves[class].source.clone())
            .collect();
        let tree = match artifacts {
            GraphSource::Original(originals) => IntervalTree::from_original_artifacts_cancellable(
                sources,
                |pair| load_pair(*originals, &NodeId::Merge(Box::new(pair))),
                params.pallas.clone(),
                params.vesta.clone(),
                TreeImportConfig {
                    key: limits.key,
                    maximum_keys: limits.maximum_artifacts,
                    maximum_original_bytes: limits.maximum_original_bytes,
                },
                cancellation,
            )?,
            GraphSource::Qualified(catalog) => {
                IntervalTree::from_qualified_catalog(sources, catalog, cancellation)?
            }
        };
        Ok(Self {
            program,
            leaves,
            sequence,
            tree,
        })
    }
    pub(super) fn source(&self) -> SourceVerifier {
        self.tree.qualified_source()
    }
    pub(super) fn prove(
        &self,
        witnesses: Vec<C>,
        params: &Parameters,
        limits: ImportLimits,
        context: &mut ProvingContext<'_, '_>,
    ) -> Result<SourceNodeEvidence, Error> {
        if witnesses.len() != self.sequence.len() {
            return Err(Error::Input);
        }
        let mut leaves = Vec::with_capacity(witnesses.len());
        for (witness, class) in witnesses.into_iter().zip(&self.sequence) {
            let installed = &self.leaves[class];
            leaves.push(installed.prove(&witness, params, limits, context)?);
        }
        let ProvingContext {
            artifacts,
            randomness,
            proof,
            fold,
            sequence,
            checkpoints,
        } = context;
        self.tree.prove(
            leaves,
            |pair| load_pair(*artifacts, &NodeId::Merge(Box::new(pair))),
            |index| {
                let current = *sequence;
                *sequence = current.checked_add(1).ok_or(Error::Input)?;
                // The tree independently selects the exact mounted child pair.
                (randomness)(ProofRequest {
                    node: NodeId::ProgramMerge(
                        self.program,
                        u32::try_from(index).map_err(|_| Error::Input)?,
                    ),
                    sequence: current,
                })
            },
            *proof,
            fold,
            checkpoints.as_mut().map(|store| {
                let reborrow: &mut dyn ProofCheckpointStore = &mut **store;
                reborrow
            }),
        )
    }
}
