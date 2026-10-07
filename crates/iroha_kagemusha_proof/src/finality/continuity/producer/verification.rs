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
        verifier: Arc::new(
            VerifierPlan::new(binding.clone(), pallas.clone()).map_err(|_| Error::Artifact)?,
        ),
        key: Arc::new(key.clone()),
    };
    Ok((source, binding, key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finality::history::{GenesisSourceCircuit, HistoryAnchor};

    #[test]
    #[ignore = "actual k16 sealed Genesis and wrapper VK derivation; optimized explicitly"]
    fn source_clones_share_qualified_storage_and_keep_exact_original_identity() {
        let pallas = PinnedParams::derive(16).unwrap();
        let vesta = PinnedParams::derive(16).unwrap();
        let layout = GenesisSourceCircuit::for_source(HistoryAnchor {
            network: [1; 32],
            instance: [2; 32],
            initial_context: [3; 32],
            initial_epoch: 0,
            parameters: [1000, 2000, 3000, 4000, 1 << 20, 100],
        });
        let derived = DerivedSource::derive(&layout, &vesta, MemoryBudget::DEFAULT).unwrap();
        let (source, binding, key) =
            derive_wrapper(&[derived], &pallas, &vesta, MemoryBudget::DEFAULT).unwrap();
        let original_digest = source.key_digest().unwrap();
        let shared = source.clone();
        assert!(Arc::ptr_eq(&source.verifier, &shared.verifier));
        assert!(Arc::ptr_eq(&source.key, &shared.key));
        assert_eq!(shared.binding().encoded(), binding.encoded());
        assert_eq!(shared.verifying_key().to_bytes(), key.to_bytes());
        // Independent input still undergoes strict decoding; sharing is only by ownership.
        let decoded = VerifyingKey::<Ep>::read(key.to_bytes(), &binding).unwrap();
        assert_eq!(&decoded, shared.verifying_key());
        let mut changed = key.to_bytes().to_vec();
        changed[0] ^= 1;
        assert!(VerifyingKey::<Ep>::read(&changed, &binding).is_err());
        let mut changed = key.to_bytes().to_vec();
        changed.push(0);
        assert!(VerifyingKey::<Ep>::read(&changed, &binding).is_err());
        drop(source);
        assert_eq!(shared.key_digest().unwrap(), original_digest);
        assert_eq!(shared.verifying_key().to_bytes(), key.to_bytes());
    }
}
