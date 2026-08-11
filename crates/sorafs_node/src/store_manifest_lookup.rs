/// Returns a clone of the stored manifest metadata, looked up by digest.
#[must_use]
pub fn manifest_by_digest(&self, digest: &[u8; 32]) -> Option<StoredManifest> {
    let manifest_id = hex::encode(digest);
    self.state
        .read()
        .expect("storage state poisoned")
        .manifests
        .get(&manifest_id)
        .filter(|manifest| manifest.manifest_digest == *digest)
        .cloned()
}

/// Return the deterministic preferred manifest for a content CID without cloning the store.
///
/// Site manifests (those with a root `index.html`) rank ahead of blob-only variants, followed
/// by file count, manifest digest, and manifest identifier. The final two keys preserve the
/// previous deterministic tie-breaking of the sorted full-snapshot implementation while this
/// lookup retains and clones only the selected manifest.
#[must_use]
pub fn manifest_by_cid_prefer_site(&self, cid: &[u8]) -> Option<StoredManifest> {
    let state = self.state.read().expect("storage state poisoned");
    state
        .manifests
        .values()
        .filter(|manifest| manifest.manifest_cid() == cid)
        .max_by(|left, right| {
            let left_has_index = left
                .files()
                .iter()
                .any(|file| file.path.len() == 1 && file.path[0] == "index.html");
            let right_has_index = right
                .files()
                .iter()
                .any(|file| file.path.len() == 1 && file.path[0] == "index.html");
            (left_has_index, left.files().len())
                .cmp(&(right_has_index, right.files().len()))
                .then_with(|| left.manifest_digest().cmp(right.manifest_digest()))
                .then_with(|| left.manifest_id().cmp(right.manifest_id()))
        })
        .cloned()
}
