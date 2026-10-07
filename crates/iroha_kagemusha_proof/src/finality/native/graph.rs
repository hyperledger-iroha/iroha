//! Exact fixed graph mounting, retaining qualified metadata rather than proving tables.

use super::*;
use super::{
    artifacts::{Admission, borrowed},
    program::{InstalledProgram, Mounted, identity},
};
use crate::finality::{
    aggregate::AggregateLeafCircuit,
    bls::BlsBatchCircuit,
    certificate::CertificateCircuit,
    certified_result::CertifiedResultCircuit,
    continuity::{SourcePairPlan, producer::SourceCircuit},
    history::{HistoryArtifacts, HistoryProver, HistoryStepCircuit},
    load_source::LoadSourceCircuit,
    receipt_finality::ReceiptFinalityCircuit,
    result_scan::ResultScanBatchCircuit,
    schedule::{
        complete::ScheduleCircuit, context_hash::ContextHashBatchCircuit,
        source::ScheduleSourceCircuit,
    },
    scheduled_result::ScheduledResultCircuit,
};

pub(super) struct Programs {
    pub(super) bls: InstalledProgram<BlsBatchCircuit>,
    pub(super) aggregation: InstalledProgram<AggregateLeafCircuit>,
    pub(super) result: InstalledProgram<ResultScanBatchCircuit>,
    pub(super) schedule: InstalledProgram<ScheduleSourceCircuit>,
    pub(super) context: InstalledProgram<ContextHashBatchCircuit>,
    pub(super) load: InstalledProgram<LoadSourceCircuit>,
}
impl Programs {
    fn mount(
        artifacts: &mut dyn ArtifactSource,
        params: &Parameters,
        limits: ImportLimits,
    ) -> Result<Self, Error> {
        use super::source_layout as layout;
        let bls = InstalledProgram::mount(
            Program::Bls,
            layout::length(Program::Bls),
            layout::bls,
            artifacts,
            params,
            limits,
        )?;
        let aggregation = InstalledProgram::mount(
            Program::Aggregation,
            layout::length(Program::Aggregation),
            layout::aggregation,
            artifacts,
            params,
            limits,
        )?;
        let result = InstalledProgram::mount(
            Program::Result,
            layout::length(Program::Result),
            layout::result,
            artifacts,
            params,
            limits,
        )?;
        let schedule = InstalledProgram::mount(
            Program::Schedule,
            layout::length(Program::Schedule),
            layout::schedule,
            artifacts,
            params,
            limits,
        )?;
        let context = InstalledProgram::mount(
            Program::Context,
            layout::length(Program::Context),
            layout::context,
            artifacts,
            params,
            limits,
        )?;
        let load = InstalledProgram::mount(
            Program::Load,
            layout::length(Program::Load),
            layout::load,
            artifacts,
            params,
            limits,
        )?;
        Ok(Self {
            bls,
            aggregation,
            result,
            schedule,
            context,
            load,
        })
    }
}

pub(super) struct Pair<C: SourceCircuit> {
    pub(super) node: Mounted<C>,
    pub(super) plan: SourcePairPlan,
}
impl<C: SourceCircuit> Pair<C> {
    fn mount(
        kind: Composition,
        children: [SourceVerifier; 2],
        layout: impl FnOnce(SourcePairPlan) -> Result<C, iroha_plonk::frontend::Error>,
        artifacts: &mut dyn ArtifactSource,
        params: &Parameters,
        limits: ImportLimits,
    ) -> Result<Self, Error> {
        let plan = circuit(SourcePairPlan::new(children, &params.pallas))?;
        let node = Mounted::mount(
            NodeId::Composition(kind),
            &circuit(layout(plan.clone()))?,
            artifacts,
            params,
            limits,
        )?;
        Ok(Self { node, plan })
    }
}

pub(super) struct History {
    pub(super) source: SourceVerifier,
    body: SourceVerifier,
}
impl History {
    fn mount(
        anchor: HistoryAnchor,
        body: SourceVerifier,
        artifacts: &mut dyn ArtifactSource,
        params: &Parameters,
        limits: ImportLimits,
    ) -> Result<Self, Error> {
        let prover = Self::import(anchor, body.clone(), artifacts, params, limits)?;
        Ok(Self {
            source: prover.qualified_source()?,
            body,
        })
    }
    fn import(
        anchor: HistoryAnchor,
        body: SourceVerifier,
        artifacts: &mut dyn ArtifactSource,
        params: &Parameters,
        limits: ImportLimits,
    ) -> Result<HistoryProver, Error> {
        let genesis = artifacts.load(&ArtifactId::Source(NodeId::Genesis))?;
        let append = artifacts.load(&ArtifactId::Source(NodeId::Append))?;
        let wrapper = artifacts.load(&ArtifactId::HistoryWrapper)?;
        HistoryProver::from_original_artifacts(
            anchor,
            body,
            HistoryArtifacts {
                genesis: borrowed(&genesis),
                append: borrowed(&append),
                wrapper: borrowed(&wrapper),
            },
            params.pallas.clone(),
            params.vesta.clone(),
            limits.key,
        )
    }
    pub(super) fn reload(
        &self,
        anchor: HistoryAnchor,
        artifacts: &mut dyn ArtifactSource,
        params: &Parameters,
        limits: ImportLimits,
    ) -> Result<HistoryProver, Error> {
        let prover = Self::import(anchor, self.body.clone(), artifacts, params, limits)?;
        if identity(&prover.qualified_source()?)? != identity(&self.source)? {
            return Err(Error::Artifact);
        }
        Ok(prover)
    }
}

/// Complete source graph rooted in one independently authenticated signed genesis.
/// All fields are private. Mounting imports every original against its fixed
/// source and exact children; callers cannot inject a raw verifying-key capability.
pub struct InstalledFinality {
    pub(super) anchor: HistoryAnchor,
    pub(super) params: Parameters,
    pub(super) limits: ImportLimits,
    pub(super) programs: Programs,
    pub(super) certificate: Pair<CertificateCircuit>,
    pub(super) certified: Pair<CertifiedResultCircuit>,
    pub(super) schedule: Pair<ScheduleCircuit>,
    pub(super) scheduled: Pair<ScheduledResultCircuit>,
    pub(super) step: Pair<HistoryStepCircuit>,
    pub(super) history: History,
    pub(super) receipt: Pair<ReceiptFinalityCircuit>,
}
impl InstalledFinality {
    /// Mount the entire fixed graph using independently installed originals.
    /// The application must independently authenticate the exact complete anchor
    /// from signed global genesis before selecting this installation.
    /// # Errors
    /// Any missing/mutated original, source or child-key mismatch, wrong profile,
    /// oversized finite inventory or parameter/resource refusal.
    pub fn from_original_artifacts(
        anchor: HistoryAnchor,
        artifacts: &mut dyn ArtifactSource,
        params: Parameters,
        limits: ImportLimits,
    ) -> Result<Self, Error> {
        if params.pallas.k() != 16 || params.vesta.k() != 16 {
            return Err(Error::Artifact);
        }
        let mut artifacts = Admission::new(artifacts, limits)?;
        let programs = Programs::mount(&mut artifacts, &params, limits)?;
        let certificate = Pair::mount(
            Composition::Certificate,
            [programs.aggregation.source(), programs.bls.source()],
            CertificateCircuit::for_source,
            &mut artifacts,
            &params,
            limits,
        )?;
        let certified = Pair::mount(
            Composition::CertifiedResult,
            [certificate.node.source.clone(), programs.result.source()],
            CertifiedResultCircuit::for_source,
            &mut artifacts,
            &params,
            limits,
        )?;
        let schedule = Pair::mount(
            Composition::Schedule,
            [programs.schedule.source(), programs.context.source()],
            ScheduleCircuit::for_source,
            &mut artifacts,
            &params,
            limits,
        )?;
        let scheduled = Pair::mount(
            Composition::ScheduledResult,
            [certified.node.source.clone(), schedule.node.source.clone()],
            ScheduledResultCircuit::for_source,
            &mut artifacts,
            &params,
            limits,
        )?;
        let step = Pair::mount(
            Composition::HistoryStep,
            [scheduled.node.source.clone(), schedule.node.source.clone()],
            |plan| HistoryStepCircuit::for_source(anchor, plan),
            &mut artifacts,
            &params,
            limits,
        )?;
        let history = History::mount(
            anchor,
            step.node.source.clone(),
            &mut artifacts,
            &params,
            limits,
        )?;
        let receipt = Pair::mount(
            Composition::Receipt,
            [history.source.clone(), programs.load.source()],
            |plan| ReceiptFinalityCircuit::for_source(anchor, plan),
            &mut artifacts,
            &params,
            limits,
        )?;
        Ok(Self {
            anchor,
            params,
            limits,
            programs,
            certificate,
            certified,
            schedule,
            scheduled,
            step,
            history,
            receipt,
        })
    }
    /// Complete independently installed global signed-genesis anchor.
    pub const fn anchor(&self) -> &HistoryAnchor {
        &self.anchor
    }
    /// Exact terminal source capability to pin in the Load relation.
    pub fn qualified_source(&self) -> SourceVerifier {
        self.receipt.node.source.clone()
    }
    /// Require the complete terminal receipt endpoints, exact installed proof and all claims.
    /// # Errors
    /// Changed receipt/root, malformed proof, wrong source key or failed generator decision.
    pub fn verify_receipt_evidence(
        &self,
        receipt_digest: Fp,
        evidence: &SourceNodeEvidence,
        budget: MemoryBudget,
    ) -> Result<(), Error> {
        crate::finality::receipt_finality::verify_receipt_evidence(
            &self.anchor,
            &self.receipt.node.source,
            &self.params.vesta,
            receipt_digest,
            evidence,
            budget,
        )
    }
}
