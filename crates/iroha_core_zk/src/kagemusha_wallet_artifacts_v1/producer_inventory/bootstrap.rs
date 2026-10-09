//! Exact Bootstrap A1/W0/A2 source qualification under imported Q sources.

use iroha_kagemusha_proof::a_relation::{
    AProofPlan, QProofPlan,
    native::{artifact::KeyArtifact, bootstrap as native},
};
use iroha_plonk::{keys::pk::artifact::ReadConfig, pcs::ipa::PinnedParams};
use iroha_plonk_recursion::verifier::VerifierPlan;

use super::*;

/// Bootstrap source qualification failed before any wallet capability was granted.
#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum BootstrapQualificationErrorV1 {
    /// Installation, canonical metadata or bounded original read mismatch.
    #[error(transparent)]
    Original(#[from] Error),
    /// The fixed source plan or entire signed context schema differs.
    #[error("invalid compiled Bootstrap source/context")]
    Source,
    /// Strict native original source/key import failed.
    #[error(transparent)]
    Native(#[from] native::Error),
}

/// All three strictly imported Bootstrap sources for one authenticated program.
/// Only metadata is retained; no PK, final Omega or wallet-open capability is owned.
pub struct QualifiedBootstrapProgramV1 {
    installation: ([u8; 32], [u8; 32]),
    program: u32,
    prover: native::Prover,
}
impl QualifiedBootstrapProgramV1 {
    /// Exact authenticated installation and selected program.
    pub const fn identity(&self) -> (([u8; 32], [u8; 32]), u32) {
        (self.installation, self.program)
    }
    /// Metadata-only native stage owner; use its strict imports for borrowed proving keys.
    /// This component alone cannot authorize a complete wallet or final Omega.
    pub const fn prover(&self) -> &native::Prover {
        &self.prover
    }
}

fn metadata<C: PastaCurve>(
    original: ArtifactOriginalV1,
) -> Result<KeyArtifact<C>, BootstrapQualificationErrorV1> {
    let binding = DescriptorBinding::decode_v2(&original.descriptor)
        .map_err(|_| BootstrapQualificationErrorV1::Source)?;
    let key = VerifyingKey::read(&original.verifying_key, &binding)
        .map_err(|_| BootstrapQualificationErrorV1::Source)?;
    KeyArtifact::new(binding, key).map_err(|_| BootstrapQualificationErrorV1::Source)
}

// The shared offline/intake factory reconstructs every fixed context word from
// the already-qualified Q keys and installed root. No signed context is a recipe.
pub(super) fn plan(
    scope: SourceScopeV1,
    recipe: &QProgramRecipeV1,
) -> Result<native::Plan, BootstrapQualificationErrorV1> {
    if recipe.signatures().len() != 1 || recipe.keys().len() != 2 {
        return Err(BootstrapQualificationErrorV1::Source);
    }
    let policy = scope.bootstrap()?;
    let pallas = PinnedParams::derive(16).map_err(|_| BootstrapQualificationErrorV1::Source)?;
    let vesta = PinnedParams::derive(16).map_err(|_| BootstrapQualificationErrorV1::Source)?;
    let keys = recipe
        .keys()
        .iter()
        .map(|key| {
            let verifier = VerifierPlan::new(key.binding().clone(), pallas.clone())
                .map_err(|_| BootstrapQualificationErrorV1::Source)?;
            QProofPlan::new(verifier, key.key().clone())
                .map_err(|_| BootstrapQualificationErrorV1::Source)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let operation = AProofPlan::new(
        Variant::Bootstrap,
        recipe.sigma().clone(),
        keys,
        None,
        &pallas,
    )
    .map_err(|_| BootstrapQualificationErrorV1::Source)?;
    native::Plan::new(
        operation,
        policy,
        recipe.signatures()[0].clone(),
        pallas,
        vesta,
    )
    .map_err(Into::into)
}

impl AuthenticatedProducerInventoryV1 {
    /// Reconstruct the whole fixed Bootstrap context and strictly import all three
    /// signed A1/W0/A2 originals sequentially. Intermediate metadata alone never
    /// bypasses the original source, table, commitment and full VK checks.
    /// # Errors
    /// Different installation/program/Q scope, wrong entire context, malformed
    /// metadata, capped or substituted original, or any strict source failure.
    pub fn qualify_bootstrap_program(
        &self,
        installed: &InstalledVerifierPackV1,
        qualified_q: &QualifiedQProgramV1,
        program: u32,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
    ) -> Result<QualifiedBootstrapProgramV1, BootstrapQualificationErrorV1> {
        let scheme = installed.verifier().scheme();
        let identity = (scheme.scheme_id(), installed.verifier().manifest_digest());
        if identity != self.installation() || qualified_q.identity() != (identity, program) {
            return Err(Error::Authority.into());
        }
        let record = self
            .inventory
            .operations
            .get(usize::try_from(program).map_err(|_| Error::Inventory)?)
            .ok_or(Error::Inventory)?;
        if Variant::ALL.get(
            usize::from(record.variant)
                .checked_sub(1)
                .ok_or(Error::Inventory)?,
        ) != Some(&Variant::Bootstrap)
            || record.a.len() != 2
            || record.w.len() != 1
        {
            return Err(Error::Inventory.into());
        }
        let plan = plan(SourceScopeV1::from_scheme(scheme)?, qualified_q.recipe())?;
        let schema: Vec<_> = plan
            .context()
            .schema()
            .iter()
            .map(PrimeField::to_repr)
            .collect();
        if schema != record.context {
            return Err(BootstrapQualificationErrorV1::Source);
        }
        let first = metadata(self.read_verifier_original(record.a[0], originals)?)?;
        let wrapper = metadata(self.read_verifier_original(record.w[0], originals)?)?;
        let terminal = metadata(self.read_verifier_original(record.a[1], originals)?)?;
        let prover = native::Prover::from_artifacts(plan, first, wrapper, terminal)?;
        for (role, index) in [record.a[0], record.w[0], record.a[1]]
            .into_iter()
            .enumerate()
        {
            let original = self.read_original(index, originals, config.maximum_bytes)?;
            match role {
                0 => drop(prover.import_first(&original.proving_key, config)?),
                1 => drop(prover.import_wrapper(&original.proving_key, config)?),
                2 => drop(prover.import_terminal(&original.proving_key, config)?),
                _ => unreachable!("fixed three Bootstrap roles"),
            }
            drop(original);
        }
        Ok(QualifiedBootstrapProgramV1 {
            installation: identity,
            program,
            prover,
        })
    }
}
