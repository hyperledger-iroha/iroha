//! Opaque offline regeneration recipes built only from the compiled source walk.

use super::*;

/// An in-memory offline compiler recipe, never decoded from an artifact or proof.
/// It retains unknown source layout and pinned parameters, not proving tables.
pub struct OriginalRecipe {
    generate: Box<dyn Fn() -> Result<OriginalBytes, Error>>,
}
impl OriginalRecipe {
    #[cfg(test)]
    pub(super) fn test_bytes(bytes: OriginalBytes) -> Self {
        // Storage-only malformed originals; never passed to a source importer.
        Self {
            generate: Box::new(move || Ok(bytes.clone())),
        }
    }
    pub(super) fn source<C: SourceCircuit + 'static>(
        source: &C,
        params: &Parameters,
        limits: ImportLimits,
    ) -> Self {
        let source = source.without_witnesses();
        let params = params.vesta.clone();
        Self {
            generate: Box::new(move || {
                let key = keygen_pk_v2(
                    &params,
                    &source,
                    &key_config(vec![InstanceType::Bounded], true, limits),
                )
                .map_err(|_| Error::Artifact)?;
                original(&key, limits).map_err(|_| Error::Artifact)
            }),
        }
    }
    pub(super) fn wrapper(
        source: &OmegaCircuit,
        params: &Parameters,
        limits: ImportLimits,
    ) -> Self {
        let source = source.without_witnesses();
        let params = params.pallas.clone();
        Self {
            generate: Box::new(move || {
                let key = keygen_pk_v2(
                    &params,
                    &source,
                    &key_config(OmegaPlan::instance_types().to_vec(), false, limits),
                )
                .map_err(|_| Error::Artifact)?;
                original(&key, limits).map_err(|_| Error::Artifact)
            }),
        }
    }
    /// Explicit offline key regeneration. The caller must compare every original
    /// descriptor/VK/PK byte identity to retained immutable inventory before use.
    /// # Errors
    /// Source/key construction, serialization or finite original bound failure.
    pub fn regenerate(&self) -> Result<OriginalBytes, Error> {
        (self.generate)()
    }
}
