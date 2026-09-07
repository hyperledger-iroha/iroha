//! Read-only local full-WSV observations for multi-validator integration tests.

use super::*;

impl Kura {
    /// Read the complete committed WSV checkpoint for one independently verified block.
    ///
    /// `blocks_dir` is the exact active Kura block directory of a locally controlled test
    /// validator. The caller must authenticate `artifact` against its pinned network and
    /// validator trust context first. This compares local execution results; the returned WSV
    /// hash is not itself a public quorum-signed full-state commitment. The artifact's
    /// `post_state_root` covers execution witnesses and is deliberately not returned here.
    ///
    /// The native snapshot hash includes all serialized consensus World fields, including game
    /// sessions, asset balances, execution receipts, NFT offers, and retained custody records.
    /// Derived indexes are reconstructed from those records on restore. Only the snapshot
    /// bootstrap envelope and consensus topology caches are redacted by the canonical hasher.
    ///
    /// Returns `None` while the exact checkpoint/manifest publication is incomplete. No Kura
    /// instance is opened and no file or runtime state is changed.
    ///
    /// # Errors
    /// Rejects malformed, over-limit, mismatched, or substituted local sidecars, including a
    /// complete checkpoint whose manifest does not bind the supplied finality artifact.
    pub fn local_wsv_checkpoint_hash_for_tests(
        blocks_dir: &Path,
        artifact: &V2FinalityArtifact,
    ) -> Result<Option<Hash>> {
        let checkpoint_path = Self::wsv_checkpoint_path_for(blocks_dir, artifact.height);
        let manifest_path = Self::commit_manifest_path_for(blocks_dir, artifact.height);
        let checkpoint = Self::decode_wsv_checkpoint_at(&checkpoint_path)?;
        let manifest = Self::decode_commit_manifest_at(&manifest_path)?;
        let invalid =
            |message: &str| Error::NoritoFrame(norito::core::Error::Message(message.to_owned()));
        let Some(checkpoint) = checkpoint else {
            if manifest.is_some() {
                return Err(invalid(
                    "complete commit manifest has no local WSV checkpoint",
                ));
            }
            return Ok(None);
        };
        if artifact.height == 0
            || checkpoint.height != artifact.height
            || checkpoint.block_hash != artifact.block_hash
            || artifact.subject.block_hash != artifact.block_hash
        {
            return Err(invalid(
                "local WSV checkpoint does not match the expected finalized block",
            ));
        }
        let Some(manifest) = manifest else {
            if checkpoint.commit_manifest_hash.is_some() {
                return Err(invalid(
                    "published local WSV checkpoint has lost its commit manifest",
                ));
            }
            return Ok(None);
        };
        Self::ensure_checkpoint_matches_manifest(&checkpoint, &manifest)?;
        if !manifest.binds_authenticated_v2_commit_authority(artifact) {
            return Err(invalid(
                "local WSV manifest does not bind the expected finality artifact",
            ));
        }
        let Some(published) = checkpoint.commit_manifest_hash else {
            return Ok(None);
        };
        if published != manifest.encoded_hash() {
            return Err(invalid(
                "local WSV checkpoint names a different commit manifest digest",
            ));
        }
        Ok(Some(checkpoint.state_hash))
    }
}
