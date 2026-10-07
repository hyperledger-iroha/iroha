//! Derive verifier metadata from sealed sources without retaining or generating PKs.

use super::*;
use iroha_plonk::keys::{CosetCachePolicy, KeygenConfigV2, keygen_vk_with_binding_v2};

/// Opaque result of actual sealed-source VK derivation, never decoded metadata.
pub(in crate::finality) struct DerivedSource(QualifiedSourceKey);

impl DerivedSource {
    pub(in crate::finality) fn derive<C: SourceCircuit>(
        source: &C,
        vesta: &PinnedParams<Eq>,
        budget: MemoryBudget,
    ) -> Result<Self, Error> {
        if vesta.k() != 16 {
            return Err(Error::Artifact);
        }
        let mut config = KeygenConfigV2::pipa_r(vec![InstanceType::Bounded]);
        config.compress_selectors = false;
        config.coset_cache = CosetCachePolicy::OnDemand;
        config.msm_budget = budget;
        let (binding, key) = keygen_vk_with_binding_v2(vesta, &source.without_witnesses(), &config)
            .map_err(|_| Error::Artifact)?;
        Ok(Self(QualifiedSourceKey { binding, key }))
    }

    pub(in crate::finality) fn binding(&self) -> &DescriptorBinding {
        &self.0.binding
    }

    pub(in crate::finality) fn key(&self) -> &VerifyingKey<Eq> {
        &self.0.key
    }
}

/// The wrapper is derived from opaque compiled source keys in their exact order.
/// No public/raw-metadata constructor can mint a qualified `SourceVerifier`.
pub(in crate::finality) fn derive_wrapper(
    sources: &[DerivedSource],
    pallas: &PinnedParams<Ep>,
    vesta: &PinnedParams<Eq>,
    budget: MemoryBudget,
) -> Result<(SourceVerifier, DescriptorBinding, VerifyingKey<Ep>), Error> {
    if pallas.k() != 16 || vesta.k() != 16 {
        return Err(Error::Artifact);
    }
    let catalog = sources
        .iter()
        .map(|source| source.0.clone())
        .collect::<Vec<_>>();
    let (_, circuit) = wrapper_source(&catalog, vesta.clone())?;
    let mut config = KeygenConfigV2::pipa_r(OmegaPlan::instance_types().to_vec());
    config.coset_cache = CosetCachePolicy::OnDemand;
    config.msm_budget = budget;
    let (binding, key) =
        keygen_vk_with_binding_v2(pallas, &circuit, &config).map_err(|_| Error::Artifact)?;
    let source = SourceVerifier {
        verifier: VerifierPlan::new(binding.clone(), pallas.clone())
            .map_err(|_| Error::Artifact)?,
        key: key.clone(),
    };
    Ok((source, binding, key))
}
