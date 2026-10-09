//! One call-local imported pair, bound to reloaded originals on every actual use.

use super::super::{ArtifactSource, Error, NodeId, artifacts::load_pair};
use crate::finality::continuity::tree::OriginalPair;
use iroha_pasta::CancellationToken;
use sha2::{Digest as _, Sha256};

const HASH_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct OriginalIdentity {
    bytes: usize,
    sha256: [u8; 32],
}

/// Fixed role order is source D/V/PK, then wrapper D/V/PK. Lengths and SHA-256
/// bind the complete canonical originals, not merely descriptor or VK digests.
#[derive(Debug, PartialEq, Eq)]
struct PairIdentity([OriginalIdentity; 6]);

impl PairIdentity {
    fn read(
        pair: &OriginalPair,
        maximum_key_bytes: usize,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Self, Error> {
        CancellationToken::checkpoint(cancellation)?;
        let mut identities = [OriginalIdentity::default(); 6];
        let originals = [&pair.source, &pair.wrapper].into_iter().flat_map(|bytes| {
            [
                (&bytes.descriptor, 1 << 20),
                (&bytes.verifying_key, 1 << 18),
                (&bytes.proving_key, maximum_key_bytes),
            ]
        });
        for (identity, (bytes, maximum)) in identities.iter_mut().zip(originals) {
            if bytes.is_empty() || bytes.len() > maximum {
                return Err(Error::Artifact);
            }
            let mut hash = Sha256::new();
            for chunk in bytes.chunks(HASH_CHUNK_BYTES) {
                CancellationToken::checkpoint(cancellation)?;
                hash.update(chunk);
            }
            *identity = OriginalIdentity {
                bytes: bytes.len(),
                sha256: hash.finalize().into(),
            };
        }
        CancellationToken::checkpoint(cancellation)?;
        Ok(Self(identities))
    }
}

struct Imported<T> {
    id: NodeId,
    originals: PairIdentity,
    value: T,
}

/// Private mechanism only: `T` is an immutable, strictly imported source/wrapper
/// Prover in production. Tests use drop-tracked tokens, never proof authority.
/// The owner fixes parameters and import configuration for the entire call.
pub(super) struct LastImported<T> {
    maximum_key_bytes: usize,
    entry: Option<Imported<T>>,
}

impl<T> LastImported<T> {
    pub(super) fn new(maximum_key_bytes: usize) -> Self {
        Self {
            maximum_key_bytes,
            entry: None,
        }
    }

    /// Evict even before a different class's checkpoint or original lookup.
    pub(super) fn select(&mut self, id: &NodeId) {
        if self.entry.as_ref().is_some_and(|entry| entry.id != *id) {
            self.entry = None;
        }
    }

    pub(super) fn get_or_import(
        &mut self,
        id: &NodeId,
        artifacts: &mut dyn ArtifactSource,
        cancellation: Option<&CancellationToken>,
        import: impl FnOnce(&OriginalPair) -> Result<T, Error>,
    ) -> Result<&T, Error> {
        // Complete the drop before calling any loader or importing a new pair.
        self.select(id);
        let result = (|| {
            CancellationToken::checkpoint(cancellation)?;
            // Cache hits still exercise the original storage/custody contract.
            // This cache never substitutes for a missing or changed original.
            let pair = load_pair(artifacts, id)?;
            let originals = PairIdentity::read(&pair, self.maximum_key_bytes, cancellation)?;
            if let Some(entry) = &self.entry {
                if entry.originals != originals {
                    return Err(Error::Artifact);
                }
            } else {
                // Only successful strict compiled-source import creates an entry.
                let value = import(&pair)?;
                CancellationToken::checkpoint(cancellation)?;
                self.entry = Some(Imported {
                    id: id.clone(),
                    originals,
                    value,
                });
            }
            CancellationToken::checkpoint(cancellation)?;
            Ok(())
        })();
        if let Err(error) = result {
            // No stale authority survives an observed storage/import refusal.
            self.entry = None;
            return Err(error);
        }
        self.entry
            .as_ref()
            .map(|entry| &entry.value)
            .ok_or(Error::Artifact)
    }
}

#[cfg(test)]
#[path = "imported/tests.rs"]
mod tests;
