//! Fixed complete catalog assembly, with one active source/wrapper pair at a time.

use super::*;
use crate::finality::{
    certificate::CertificateCircuit,
    certified_result::CertifiedResultCircuit,
    history::{
        GenesisSourceCircuit, HistoryAppendCircuit, HistoryAppendPlan, HistoryArtifacts,
        HistoryProver, HistoryStepCircuit,
    },
    native::source_layout,
    receipt_finality::ReceiptFinalityCircuit,
    schedule::complete::ScheduleCircuit,
    scheduled_result::ScheduledResultCircuit,
};

/// Completed source compilation and strict graph import, without a live proof or
/// authenticated installation decision. The caller retains the provenance and
/// originals and must independently authenticate their eventual deployment.
pub struct Compilation {
    /// Import-only installed graph from every emitted original artifact.
    pub graph: InstalledFinality,
    /// Caller-recorded source provenance, never silently synthesized by the compiler.
    pub provenance: SourceProvenance,
    /// Exact terminal source descriptor/key identity from the compiled graph.
    pub terminal: SourceIdentity,
}

/// Single fixed graph traversal shared by offline original compilation and
/// verifier-only source qualification. Implementations cannot select topology.
pub(super) trait Assembler {
    fn params(&self) -> &Parameters;
    fn source<C: SourceCircuit + 'static>(
        &mut self,
        id: NodeId,
        source: &C,
    ) -> Result<SourceVerifier, CompileError>;
    fn history(
        &mut self,
        anchor: HistoryAnchor,
        body: SourceVerifier,
    ) -> Result<SourceVerifier, CompileError>;

    fn program<C: SourceCircuit + 'static>(
        &mut self,
        program: Program,
        factory: fn(u32) -> Result<(u32, C), Error>,
    ) -> Result<SourceVerifier, CompileError> {
        let mut level = Vec::new();
        for position in 0..source_layout::length(program) {
            let (class, source) = factory(position)?;
            level.push(self.source(NodeId::Leaf(program, class), &source)?);
        }
        // Identical odd-node carry and ordered balanced topology as IntervalTree.
        // Final strict graph mounting independently checks every selected identity.
        while level.len() > 1 {
            let mut next = Vec::with_capacity(level.len().div_ceil(2));
            for children in level.chunks(2) {
                if children.len() == 1 {
                    next.push(children[0].clone());
                } else {
                    let pair = [children[0].clone(), children[1].clone()];
                    let id = NodeId::Merge(Box::new([identity(&pair[0])?, identity(&pair[1])?]));
                    let plan = layout(SourcePairPlan::new(pair, &self.params().pallas))?;
                    next.push(self.source(id, &layout(SourceMergeCircuit::for_source(plan))?)?);
                }
            }
            level = next;
        }
        level.pop().ok_or_else(|| Error::Artifact.into())
    }

    fn pair<C: SourceCircuit + 'static>(
        &mut self,
        kind: Composition,
        children: [SourceVerifier; 2],
        factory: impl FnOnce(SourcePairPlan) -> Result<C, iroha_plonk::frontend::Error>,
    ) -> Result<SourceVerifier, CompileError> {
        let plan = layout(SourcePairPlan::new(children, &self.params().pallas))?;
        let source = layout(factory(plan))?;
        self.source(NodeId::Composition(kind), &source)
    }
}

impl Compiler<'_> {
    fn emit_source<C: SourceCircuit + 'static>(
        &mut self,
        id: NodeId,
        source: &C,
    ) -> Result<(DescriptorBinding, VerifyingKey<Eq>), CompileError> {
        self.sink.register_recipe(
            &ArtifactId::Source(id.clone()),
            OriginalRecipe::source(source, &self.params, self.limits),
        )?;
        let key = keygen_pk_v2(
            &self.params.vesta,
            &source.without_witnesses(),
            &key_config(vec![InstanceType::Bounded], true, self.limits),
        )
        .map_err(|error| failure(Some(id.clone()), "source key", error))?;
        let metadata = (key.binding().clone(), key.vk().clone());
        let bytes = original(&key, self.limits)
            .map_err(|error| failure(Some(id.clone()), "source original", error))?;
        self.emit(ArtifactId::Source(id), &bytes)?;
        Ok(metadata)
    }

    fn history(
        &mut self,
        anchor: HistoryAnchor,
        body: SourceVerifier,
    ) -> Result<SourceVerifier, CompileError> {
        let genesis = GenesisSourceCircuit::for_source(anchor);
        let (binding, genesis_vk) = self.emit_source(NodeId::Genesis, &genesis)?;
        // The body wrapper provides the canonical future-wrapper layout. The
        // eventual shared wrapper's exact descriptor is independently checked by
        // HistoryProver; a shape assumption cannot silently authorize its key.
        let append_plan = layout(HistoryAppendPlan::new(
            anchor,
            body.binding().clone(),
            body.clone(),
            self.params.pallas.clone(),
        ))?;
        let append = layout(HistoryAppendCircuit::for_source(append_plan))?;
        let (append_binding, append_vk) = self.emit_source(NodeId::Append, &append)?;
        if binding.encoded() != append_binding.encoded() {
            return Err(Error::Artifact.into());
        }
        let blank = wrapper(binding, vec![genesis_vk, append_vk], &self.params)?;
        self.sink.register_recipe(
            &ArtifactId::HistoryWrapper,
            OriginalRecipe::wrapper(&blank, &self.params, self.limits),
        )?;
        let key = keygen_pk_v2(
            &self.params.pallas,
            &blank,
            &key_config(OmegaPlan::instance_types().to_vec(), false, self.limits),
        )
        .map_err(|error| failure(None, "history wrapper key", error))?;
        let bytes = original(&key, self.limits)
            .map_err(|error| failure(None, "history wrapper original", error))?;
        self.emit(ArtifactId::HistoryWrapper, &bytes)?;
        drop(key);
        let genesis = self.sink.load(&ArtifactId::Source(NodeId::Genesis))?;
        let append = self.sink.load(&ArtifactId::Source(NodeId::Append))?;
        let outer = self.sink.load(&ArtifactId::HistoryWrapper)?;
        HistoryProver::from_original_artifacts(
            anchor,
            body,
            HistoryArtifacts {
                genesis: borrowed(&genesis),
                append: borrowed(&append),
                wrapper: borrowed(&outer),
            },
            self.params.pallas.clone(),
            self.params.vesta.clone(),
            self.limits.key,
        )?
        .qualified_source()
        .map_err(Into::into)
    }
}

/// Explicitly compile every exact source in the six programs, all ordered
/// interval merges, semantic compositions and finite Genesis/Append catalog.
/// Originals are emitted as each active key is finished, never accumulated as
/// proving keys. Key generation and strict import still require their own peak
/// process-memory qualification; inventory limits are not an RSS claim.
///
/// # Errors
/// Any layout overflow, wrong parameter/bound, failed original serialization or
/// source import, sink refusal or changed source/key identity. Partial outputs
/// remain available for diagnosis; no graph is returned before complete import.
pub fn compile(
    anchor: HistoryAnchor,
    sink: &mut dyn ArtifactSink,
    params: Parameters,
    limits: ImportLimits,
    provenance: SourceProvenance,
) -> Result<Compilation, CompileError> {
    store::check_limits(limits)?;
    if params.pallas.k() != 16
        || params.vesta.k() != 16
        || provenance.revision.is_empty()
        || provenance.revision.len() > 4096
        || provenance.source_manifest_sha256 == [0; 32]
    {
        return Err(Error::Artifact.into());
    }
    let mut compiler = Compiler {
        sink,
        params: params.clone(),
        limits,
        cache: BTreeMap::new(),
        emitted: BTreeMap::new(),
        total: 0,
    };
    let receipt = assemble(&mut compiler, anchor)?;
    let terminal = identity(&receipt)?;
    drop(compiler);
    let graph = InstalledFinality::from_original_artifacts(anchor, sink, params, limits)?;
    if identity(&graph.qualified_source())? != terminal {
        return Err(Error::Artifact.into());
    }
    Ok(Compilation {
        graph,
        provenance,
        terminal,
    })
}

impl Assembler for Compiler<'_> {
    fn params(&self) -> &Parameters {
        &self.params
    }
    fn source<C: SourceCircuit + 'static>(
        &mut self,
        id: NodeId,
        source: &C,
    ) -> Result<SourceVerifier, CompileError> {
        Compiler::source(self, id, source)
    }
    fn history(
        &mut self,
        anchor: HistoryAnchor,
        body: SourceVerifier,
    ) -> Result<SourceVerifier, CompileError> {
        Compiler::history(self, anchor, body)
    }
}

pub(super) fn assemble(
    assembler: &mut impl Assembler,
    anchor: HistoryAnchor,
) -> Result<SourceVerifier, CompileError> {
    let aggregate = assembler.program(Program::Aggregation, source_layout::aggregation)?;
    let bls = assembler.program(Program::Bls, source_layout::bls)?;
    let result = assembler.program(Program::Result, source_layout::result)?;
    let parser = assembler.program(Program::Schedule, source_layout::schedule)?;
    let context = assembler.program(Program::Context, source_layout::context)?;
    let load = assembler.program(Program::Load, source_layout::load)?;
    let certificate = assembler.pair(
        Composition::Certificate,
        [aggregate, bls],
        CertificateCircuit::for_source,
    )?;
    let certified = assembler.pair(
        Composition::CertifiedResult,
        [certificate, result],
        CertifiedResultCircuit::for_source,
    )?;
    let schedule = assembler.pair(
        Composition::Schedule,
        [parser, context],
        ScheduleCircuit::for_source,
    )?;
    let scheduled = assembler.pair(
        Composition::ScheduledResult,
        [certified, schedule.clone()],
        ScheduledResultCircuit::for_source,
    )?;
    let step = assembler.pair(Composition::HistoryStep, [scheduled, schedule], |plan| {
        HistoryStepCircuit::for_source(anchor, plan)
    })?;
    let history = assembler.history(anchor, step)?;
    let receipt = assembler.pair(Composition::Receipt, [history, load], |plan| {
        ReceiptFinalityCircuit::for_source(anchor, plan)
    })?;
    Ok(receipt)
}
