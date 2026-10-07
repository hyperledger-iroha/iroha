//! Exact native A/W source qualification for every compiled logical route.
//!
//! The signed Omega verifier is a candidate dependency here. No complete producer
//! capability follows until that very key is independently reconstructed from the
//! complete source-qualified terminal catalog by the canonical Omega owner.

use iroha_kagemusha_proof::a_relation::{
    AProofPlan, QProofPlan,
    context::ContextPlan,
    native::{archive, artifact::KeyArtifact, consuming, load, receive, refresh, send},
    schedule::compiled::OperationRoute,
};
use iroha_plonk::{keys::pk::artifact::ReadConfig, pcs::ipa::PinnedParams};
use iroha_plonk_recursion::verifier::VerifierPlan;

use super::*;

/// A logical route failed exact source reconstruction or original-key intake.
#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum OperationQualificationErrorV1 {
    /// Installation, route, inventory or bounded original mismatch.
    #[error(transparent)]
    Original(#[from] Error),
    /// The canonical native source, complete context or finality dependency differs.
    #[error("invalid compiled operation source or context")]
    Source,
    /// An exact Bootstrap source failed qualification.
    #[error(transparent)]
    Bootstrap(#[from] BootstrapQualificationErrorV1),
    /// The original A source at this zero-based stage failed strict import.
    #[error("operation A source {0} failed original import")]
    A(usize),
    /// The original W source at this zero-based stage failed strict import.
    #[error("operation W source {0} failed original import")]
    W(usize),
}

/// The compiled native metadata owner whose complete A/W sources were imported.
/// The containing route authenticates its identity; this enum alone is not a
/// wallet-open capability or qualification of the candidate Omega dependency.
pub enum QualifiedOperationOwnerV1 {
    /// Initial state, with no predecessor dependency.
    Bootstrap(Box<QualifiedBootstrapProgramV1>),
    /// Ordinary finalized receipt loading.
    Load(Box<load::Prover>),
    /// One exact Send control mask.
    Send(Box<send::Prover>),
    /// Ordinary or renewed Receive under exact own/incoming sigma classes.
    Receive(Box<receive::Prover>),
    /// Receive-evidence or Status-evidence Archive.
    Archive(Box<archive::Prover>),
    /// Unload or Retiring.
    Consuming(Box<consuming::Prover>),
    /// One of the five distinct Refresh operations.
    Refresh(Box<refresh::Prover>),
}

/// One authenticated logical route and its source-qualified A/W metadata.
/// It retains no proving keys. Every logical route must be qualified before
/// terminal deduplication, followed by exact canonical Omega source qualification.
pub struct QualifiedOperationRouteV1 {
    installation: ([u8; 32], [u8; 32]),
    route: u32,
    program: u32,
    owner: QualifiedOperationOwnerV1,
    terminal: KeyArtifact<Eq>,
    omega: Option<KeyArtifact<Ep>>,
}
impl QualifiedOperationRouteV1 {
    /// Authenticated installation, logical route index and signed program index.
    pub const fn identity(&self) -> (([u8; 32], [u8; 32]), u32, u32) {
        (self.installation, self.route, self.program)
    }
    /// Exact typed metadata owner, ready only for its purpose-limited stage work.
    pub const fn owner(&self) -> &QualifiedOperationOwnerV1 {
        &self.owner
    }
    /// Source-qualified terminal A key; compare complete descriptor and VK bytes.
    pub const fn terminal(&self) -> &KeyArtifact<Eq> {
        &self.terminal
    }
    /// Exact signed candidate Omega dependency, absent only for Bootstrap.
    /// This key still needs the complete canonical terminal-catalog source check.
    pub const fn candidate_omega(&self) -> Option<&KeyArtifact<Ep>> {
        self.omega.as_ref()
    }
}

fn source<T, E>(result: Result<T, E>) -> Result<T, OperationQualificationErrorV1> {
    result.map_err(|_| OperationQualificationErrorV1::Source)
}
fn metadata<C: PastaCurve>(
    original: ArtifactOriginalV1,
) -> Result<KeyArtifact<C>, OperationQualificationErrorV1> {
    let binding = source(DescriptorBinding::decode_v2(&original.descriptor))?;
    let key = source(VerifyingKey::read(&original.verifying_key, &binding))?;
    source(KeyArtifact::new(binding, key))
}

// A source class may group actual selectors, but may never quietly admit an
// unrelated operation's sigma tag. Route coverage and descriptor equality are
// separately checked by the signed inventory and strict Q owner.
pub(super) fn require_route(
    record: &OperationV1,
    route: OperationRoute,
) -> Result<(), OperationQualificationErrorV1> {
    let routes = compiled_routes();
    if variant(record.variant)? != route.variant
        || !record.own_class.contains(&route.own)
        || route
            .incoming
            .is_some_and(|i| !record.incoming_class.contains(&i))
        || record.incoming_class.is_empty() != route.incoming.is_none()
        || record.own_class.iter().any(|own| {
            !routes
                .iter()
                .any(|r| r.variant == route.variant && r.own == *own)
        })
        || record.incoming_class.iter().any(|incoming| {
            !routes
                .iter()
                .any(|r| r.variant == route.variant && r.incoming == Some(*incoming))
        })
    {
        return Err(OperationQualificationErrorV1::Source);
    }
    Ok(())
}

pub(super) enum Plan {
    Load(load::Plan),
    Send(send::Plan),
    Receive(receive::Plan),
    Archive(archive::Plan),
    Consuming(consuming::Plan),
    Refresh(refresh::Plan),
}
impl Plan {
    pub(super) fn context(&self) -> &ContextPlan {
        match self {
            Self::Load(p) => p.context(),
            Self::Send(p) => p.context(),
            Self::Receive(p) => p.context(),
            Self::Archive(p) => p.context(),
            Self::Consuming(p) => p.context(),
            Self::Refresh(p) => p.context(),
        }
    }
    fn install(
        self,
        a: Vec<KeyArtifact<Eq>>,
        w: Vec<KeyArtifact<Ep>>,
    ) -> Result<QualifiedOperationOwnerV1, OperationQualificationErrorV1> {
        Ok(match self {
            Self::Load(p) => QualifiedOperationOwnerV1::Load(Box::new(source(
                load::Prover::from_artifacts(p, source(a.try_into())?, source(w.try_into())?),
            )?)),
            Self::Send(p) => QualifiedOperationOwnerV1::Send(Box::new(source(
                send::Prover::from_artifacts(p, source(a.try_into())?, source(w.try_into())?),
            )?)),
            Self::Receive(p) => QualifiedOperationOwnerV1::Receive(Box::new(source(
                receive::Prover::from_artifacts(p, source(a.try_into())?, source(w.try_into())?),
            )?)),
            Self::Archive(p) => QualifiedOperationOwnerV1::Archive(Box::new(source(
                archive::Prover::from_artifacts(p, source(a.try_into())?, source(w.try_into())?),
            )?)),
            Self::Consuming(p) => QualifiedOperationOwnerV1::Consuming(Box::new(source(
                consuming::Prover::from_artifacts(p, source(a.try_into())?, source(w.try_into())?),
            )?)),
            Self::Refresh(p) => QualifiedOperationOwnerV1::Refresh(Box::new(source(
                refresh::Prover::from_artifacts(p, a, w),
            )?)),
        })
    }
}

pub(super) fn plan(
    route: OperationRoute,
    scope: SourceScopeV1,
    recipe: &QProgramRecipeV1,
    omega: &KeyArtifact<Ep>,
    receipt: Option<ReceiptSourceRecipeV1<'_>>,
) -> Result<Plan, OperationQualificationErrorV1> {
    let policy = source(scope.own())?;
    let pallas = source(PinnedParams::derive(16))?;
    let vesta = source(PinnedParams::derive(16))?;
    let keys = recipe
        .keys()
        .iter()
        .map(|key| {
            let verifier = source(VerifierPlan::new(key.binding().clone(), pallas.clone()))?;
            source(QProofPlan::new(verifier, key.key().clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let predecessor = source(VerifierPlan::new(omega.binding().clone(), pallas.clone()))?;
    let operation = source(AProofPlan::new(
        route.variant,
        recipe.sigma().clone(),
        keys,
        Some(predecessor),
        &pallas,
    ))?;
    let key = omega.key().clone();
    Ok(match route.variant {
        Variant::Bootstrap => return Err(OperationQualificationErrorV1::Source),
        Variant::Load => {
            let receipt = receipt.ok_or(OperationQualificationErrorV1::Source)?;
            let signatures = source(recipe.signatures().to_vec().try_into())?;
            Plan::Load(source(load::Plan::new(
                operation,
                policy,
                signatures,
                key,
                load::FinalityPolicy::new(receipt.source.clone(), *receipt.anchor),
                pallas,
                vesta,
            ))?)
        }
        Variant::Send => {
            let mask = route
                .own
                .checked_sub(2)
                .ok_or(OperationQualificationErrorV1::Source)?;
            let signatures = recipe
                .signatures()
                .first()
                .ok_or(OperationQualificationErrorV1::Source)?
                .clone();
            Plan::Send(source(send::Plan::new(
                operation, mask, policy, signatures, key, pallas, vesta,
            ))?)
        }
        Variant::Receive | Variant::ReceiveRenewed => Plan::Receive(source(receive::Plan::new(
            operation, policy, key, pallas, vesta,
        ))?),
        Variant::ArchiveReceive | Variant::ArchiveStatus => Plan::Archive(source(
            archive::Plan::new(operation, policy, key, pallas, vesta),
        )?),
        Variant::Unload | Variant::Retiring => {
            let signatures = recipe
                .signatures()
                .first()
                .ok_or(OperationQualificationErrorV1::Source)?
                .clone();
            Plan::Consuming(source(consuming::Plan::new(
                operation, policy, signatures, key, pallas, vesta,
            ))?)
        }
        Variant::RefreshCredential
        | Variant::RefreshSchemePolicy
        | Variant::RefreshBlacklist
        | Variant::RefreshTimeAnchor
        | Variant::RefreshQuotaShare => Plan::Refresh(source(refresh::Plan::new(
            operation, policy, key, pallas, vesta,
        ))?),
    })
}

impl QualifiedOperationOwnerV1 {
    pub(super) fn import_a(
        &self,
        stage: usize,
        original: &[u8],
        config: ReadConfig,
    ) -> Result<iroha_plonk::ProvingKey<Eq>, OperationQualificationErrorV1> {
        let result = match self {
            Self::Bootstrap(p) => match stage {
                0 => p.prover().import_first(original, config),
                1 => p.prover().import_terminal(original, config),
                _ => return Err(OperationQualificationErrorV1::A(stage)),
            }
            .map_err(|_| ()),
            Self::Load(p) => p.import_a(stage, original, config).map_err(|_| ()),
            Self::Send(p) => p.import_a(stage, original, config).map_err(|_| ()),
            Self::Receive(p) => p.import_a(stage, original, config).map_err(|_| ()),
            Self::Archive(p) => p.import_a(stage, original, config).map_err(|_| ()),
            Self::Consuming(p) => p.import_a(stage, original, config).map_err(|_| ()),
            Self::Refresh(p) => p.import_a(stage, original, config).map_err(|_| ()),
        };
        result.map_err(|()| OperationQualificationErrorV1::A(stage))
    }
    pub(super) fn import_w(
        &self,
        stage: usize,
        original: &[u8],
        config: ReadConfig,
    ) -> Result<iroha_plonk::ProvingKey<Ep>, OperationQualificationErrorV1> {
        let result = match self {
            Self::Bootstrap(p) => {
                if stage != 0 {
                    return Err(OperationQualificationErrorV1::W(stage));
                }
                p.prover().import_wrapper(original, config).map_err(|_| ())
            }
            Self::Load(p) => p.import_w(stage, original, config).map_err(|_| ()),
            Self::Send(p) => p.import_w(stage, original, config).map_err(|_| ()),
            Self::Receive(p) => p.import_w(stage, original, config).map_err(|_| ()),
            Self::Archive(p) => p.import_w(stage, original, config).map_err(|_| ()),
            Self::Consuming(p) => p.import_w(stage, original, config).map_err(|_| ()),
            Self::Refresh(p) => p.import_w(stage, original, config).map_err(|_| ()),
        };
        result.map_err(|()| OperationQualificationErrorV1::W(stage))
    }
}

impl AuthenticatedProducerInventoryV1 {
    /// Qualify every original A/W source for one exact compiled selector route.
    /// The complete native plan is reconstructed before comparing the signed context.
    /// Each original PK is read, strictly imported and dropped before the next one.
    /// Load additionally requires the same installation's source-qualified receipt owner.
    /// Candidate Omega metadata does not confer complete-catalog or wallet readiness.
    /// # Errors
    /// Another installation/Q program/route, foreign selector class, missing qualified
    /// finality, changed context, capped original or any strict source/key mismatch.
    pub fn qualify_operation_route(
        &self,
        installed: &InstalledVerifierPackV1,
        qualified_q: &QualifiedQProgramV1,
        route_index: u32,
        receipt: Option<&QualifiedReceiptSourceV1>,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
    ) -> Result<QualifiedOperationRouteV1, OperationQualificationErrorV1> {
        let identity = (
            installed.verifier().scheme().scheme_id(),
            installed.verifier().manifest_digest(),
        );
        let index = usize::try_from(route_index).map_err(|_| Error::Inventory)?;
        let route = *compiled_routes().get(index).ok_or(Error::Inventory)?;
        let program = *self.inventory.routes.get(index).ok_or(Error::Inventory)?;
        if identity != self.installation() || qualified_q.identity() != (identity, program) {
            return Err(Error::Authority.into());
        }
        let record = self
            .inventory
            .operations
            .get(usize::try_from(program).map_err(|_| Error::Inventory)?)
            .ok_or(Error::Inventory)?;
        require_route(record, route)?;
        if route.variant == Variant::Bootstrap {
            let owner =
                self.qualify_bootstrap_program(installed, qualified_q, program, originals, config)?;
            let terminal = metadata(
                self.read_verifier_original(*record.a.last().ok_or(Error::Inventory)?, originals)?,
            )?;
            return Ok(QualifiedOperationRouteV1 {
                installation: identity,
                route: route_index,
                program,
                owner: QualifiedOperationOwnerV1::Bootstrap(Box::new(owner)),
                terminal,
                omega: None,
            });
        }
        if route.variant == Variant::Load && receipt.is_none_or(|r| r.installation() != identity) {
            return Err(Error::Authority.into());
        }
        let omega = metadata(self.read_verifier_original(self.inventory.omega, originals)?)?;
        let plan = plan(
            route,
            SourceScopeV1::from_scheme(installed.verifier().scheme())?,
            qualified_q.recipe(),
            &omega,
            receipt.map(|r| ReceiptSourceRecipeV1::new(r.source(), r.anchor())),
        )?;
        let schema: Vec<_> = plan
            .context()
            .schema()
            .iter()
            .map(PrimeField::to_repr)
            .collect();
        if schema != record.context
            || plan.context().stage_count() != record.a.len()
            || record.w.len() + 1 != record.a.len()
        {
            return Err(OperationQualificationErrorV1::Source);
        }
        let a = record
            .a
            .iter()
            .map(|index| metadata(self.read_verifier_original(*index, originals)?))
            .collect::<Result<Vec<_>, OperationQualificationErrorV1>>()?;
        let w = record
            .w
            .iter()
            .map(|index| metadata(self.read_verifier_original(*index, originals)?))
            .collect::<Result<Vec<_>, OperationQualificationErrorV1>>()?;
        let terminal = a.last().ok_or(Error::Inventory)?.clone();
        let owner = plan.install(a, w)?;
        for stage in 0..record.a.len() {
            let original = self.read_original(record.a[stage], originals, config.maximum_bytes)?;
            drop(owner.import_a(stage, &original.proving_key, config)?);
            drop(original);
            if let Some(index) = record.w.get(stage) {
                let original = self.read_original(*index, originals, config.maximum_bytes)?;
                drop(owner.import_w(stage, &original.proving_key, config)?);
                drop(original);
            }
        }
        Ok(QualifiedOperationRouteV1 {
            installation: identity,
            route: route_index,
            program,
            owner,
            terminal,
            omega: Some(omega),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_rejects_foreign_class_tags_and_missing_selected_members() {
        for route in compiled_routes() {
            let variant = u8::try_from(
                Variant::ALL
                    .iter()
                    .position(|v| *v == route.variant)
                    .unwrap()
                    + 1,
            )
            .unwrap();
            let record = OperationV1 {
                variant,
                own_class: vec![route.own],
                incoming_class: route.incoming.into_iter().collect(),
                context: vec![],
                q: vec![],
                a: vec![],
                w: vec![],
            };
            require_route(&record, route).unwrap();
            let mut changed = record.clone();
            changed.own_class.clear();
            assert!(require_route(&changed, route).is_err());
            for selector in 0..16 {
                if !compiled_routes()
                    .iter()
                    .any(|r| r.variant == route.variant && r.own == selector)
                {
                    let mut changed = record.clone();
                    changed.own_class.push(selector);
                    assert!(require_route(&changed, route).is_err());
                }
                if !compiled_routes()
                    .iter()
                    .any(|r| r.variant == route.variant && r.incoming == Some(selector))
                {
                    let mut changed = record.clone();
                    changed.incoming_class.push(selector);
                    assert!(require_route(&changed, route).is_err());
                }
            }
            if route.incoming.is_some() {
                let mut changed = record;
                changed.incoming_class.clear();
                assert!(require_route(&changed, route).is_err());
            }
        }
    }
}
