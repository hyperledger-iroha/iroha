//! Opaque regeneration recipes built only from the compiled source walk.

use super::*;

type Regenerate = dyn Fn(Option<&iroha_pasta::CancellationToken>) -> Result<OriginalBytes, Error>;

/// An in-memory compiled recipe, never decoded from an artifact or proof.
/// It retains unknown source layout and pinned parameters, not proving tables.
pub struct OriginalRecipe {
    generate: Box<Regenerate>,
}
impl OriginalRecipe {
    #[cfg(test)]
    pub(super) fn test_bytes(bytes: OriginalBytes) -> Self {
        // Storage-only malformed originals; never passed to a source importer.
        Self {
            generate: Box::new(move |cancellation| {
                iroha_pasta::CancellationToken::checkpoint(cancellation)?;
                Ok(bytes.clone())
            }),
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
            generate: Box::new(move |cancellation| {
                let key = iroha_plonk::keys::keygen_pk_v2_cancellable(
                    &params,
                    &source,
                    &key_config(vec![InstanceType::Bounded], true, limits),
                    cancellation,
                )
                .map_err(|error| key_error(&error))?;
                iroha_pasta::CancellationToken::checkpoint(cancellation)?;
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
            generate: Box::new(move |cancellation| {
                let key = iroha_plonk::keys::keygen_pk_v2_cancellable(
                    &params,
                    &source,
                    &key_config(OmegaPlan::instance_types().to_vec(), false, limits),
                    cancellation,
                )
                .map_err(|error| key_error(&error))?;
                iroha_pasta::CancellationToken::checkpoint(cancellation)?;
                original(&key, limits).map_err(|_| Error::Artifact)
            }),
        }
    }
    /// Explicit key regeneration. The caller must compare every original
    /// descriptor/VK/PK byte identity to retained immutable inventory before use.
    /// # Errors
    /// Source/key construction, serialization or finite original bound failure.
    pub fn regenerate(&self) -> Result<OriginalBytes, Error> {
        self.regenerate_cancellable(None)
    }

    /// Regenerate exact originals with cooperative cancellation during key generation.
    /// The caller must authenticate their identity and strictly import them before use.
    /// # Errors
    /// Cancellation, source/key construction, serialization or finite original bounds.
    pub fn regenerate_cancellable(
        &self,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<OriginalBytes, Error> {
        iroha_pasta::CancellationToken::checkpoint(cancellation)?;
        let bytes = (self.generate)(cancellation)?;
        iroha_pasta::CancellationToken::checkpoint(cancellation)?;
        Ok(bytes)
    }
}

fn key_error(error: &iroha_plonk::keys::KeyError) -> Error {
    if error.is_cancelled() {
        Error::Cancelled
    } else {
        Error::Artifact
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_recipe_has_no_success_and_fresh_retry_keeps_original_bytes() {
        let bytes = OriginalBytes {
            descriptor: vec![1],
            verifying_key: vec![2],
            proving_key: vec![3],
        };
        let recipe = OriginalRecipe::test_bytes(bytes);
        let token = iroha_pasta::CancellationToken::new();
        token.cancel();
        assert!(matches!(
            recipe.regenerate_cancellable(Some(&token)),
            Err(Error::Cancelled)
        ));
        let fresh = iroha_pasta::CancellationToken::new();
        let actual = recipe.regenerate_cancellable(Some(&fresh)).unwrap();
        assert_eq!(actual.descriptor, [1]);
        assert_eq!(actual.verifying_key, [2]);
        assert_eq!(actual.proving_key, [3]);
    }
}
