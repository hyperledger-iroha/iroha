//! Release metadata and exact dependency-proof validation.

use super::*;

impl MusubiReleaseMetadataV1 {
    /// Canonicalize keyword set order.
    pub fn canonicalize(&mut self) {
        self.keywords.sort();
        self.keywords.dedup();
    }
    /// Validate keyword bounds and canonical ordering.
    ///
    /// # Errors
    ///
    /// Returns an error if a metadata string or keyword is invalid, or if keywords are oversized,
    /// unsorted, or duplicated.
    pub fn validate(&self) -> Result<(), ParseError> {
        if let Some(description) = &self.description {
            description.validate()?;
        }
        if let Some(readme) = &self.readme {
            readme.validate()?;
        }
        if let Some(license) = &self.license {
            license.validate()?;
        }
        if let Some(repository) = &self.repository {
            repository.validate()?;
        }
        if self.keywords.len() > MUSUBI_MAX_KEYWORDS_V1
            || self.keywords.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ParseError::new(
                "Musubi keywords exceed their bound or are not sorted and unique",
            ));
        }
        self.keywords.iter().try_for_each(MusubiKeywordV1::validate)
    }
}

impl MusubiRegistrySnapshotV1 {
    /// Validate a non-inert finalized anchor and revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the finalized height or index revision is zero, or if the block hash
    /// is the all-zero sentinel.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.finalized_height == 0
            || self.index_revision == 0
            || digest_is_zero(&self.finalized_block_hash)
        {
            return Err(ParseError::new("Musubi registry snapshot is invalid"));
        }
        Ok(())
    }
}

impl MusubiExactDependencyEdgeV1 {
    /// Validate structural identity and requirement satisfaction.
    ///
    /// # Errors
    ///
    /// Returns an error if a nested identity or requirement is invalid, or if the exact selection
    /// belongs to another package or does not satisfy the published requirement.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        self.selected.validate()?;
        self.requirement.validate()?;
        if self.selected.package != self.package
            || !self.requirement.matches(&self.selected.version)
        {
            return Err(ParseError::new(
                "Musubi exact dependency does not satisfy its package requirement",
            ));
        }
        Ok(())
    }
}

impl MusubiVerificationNodeV1 {
    /// Validate node commitments, dependency bounds, and edge order.
    ///
    /// # Errors
    ///
    /// Returns an error if a nested identity or ABI binding is invalid, a required commitment is
    /// zero, or dependencies are non-normal, oversized, unsorted, duplicated, or invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.release.validate()?;
        self.abi.validate()?;
        if self.release_digest.is_zero()
            || self.archive_id.is_zero()
            || self.source_digest.is_zero()
            || self.interface_digest.is_zero()
            || self.dependencies.len() > MUSUBI_MAX_DEPENDENCIES_V1
            || self.dependencies.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .dependencies
                .windows(2)
                .any(|pair| pair[0].alias >= pair[1].alias)
            || self
                .dependencies
                .iter()
                .any(|dependency| dependency.kind != MusubiDependencyKindV1::Normal)
        {
            return Err(ParseError::new(
                "Musubi verification node is invalid or noncanonical",
            ));
        }
        self.dependencies
            .iter()
            .try_for_each(MusubiExactDependencyEdgeV1::validate)
    }
}

impl MusubiVerificationLockV1 {
    /// Fixed verification-lock schema label.
    pub const SCHEMA: &'static str = "musubi-verification-lock";
    /// Decode one exact canonical verification-lock bundle file under the shared V1 limits.
    ///
    /// # Errors
    ///
    /// Returns one stable payload-free error when the file is empty, oversized, malformed,
    /// trailing, noncanonical, or fails verification-lock validation.
    pub fn decode_canonical_bundle_file(bytes: &[u8]) -> Result<Self, ParseError> {
        decode_canonical_bundle_file_v1(
            bytes,
            MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1,
            MUSUBI_VERIFICATION_LOCK_DECODE_LIMITS_V1,
            Self::validate,
            "Musubi verification lock bundle file is invalid or out of bounds",
        )
    }
    /// Canonicalize all set-like vectors.
    pub fn canonicalize(&mut self) {
        self.root_dependencies.sort();
        self.root_dependencies.dedup();
        for node in &mut self.nodes {
            node.dependencies.sort();
            node.dependencies.dedup();
        }
        self.nodes
            .sort_by(|left, right| left.release.cmp(&right.release));
        self.nodes
            .dedup_by(|left, right| left.release == right.release);
    }
    /// Validate schema, graph bounds, uniqueness, reachability, cycles, and depth.
    ///
    /// # Errors
    ///
    /// Returns an error if the schema or root is invalid, graph collections are oversized or
    /// noncanonical, a root or node edge is not normal and exact, or the graph is incomplete,
    /// unreachable, cyclic, or too deep.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.root.validate()?;
        if self.schema != Self::SCHEMA
            || self.version != MUSUBI_REGISTRY_VERSION_V1
            || self.root_dependencies.len() > MUSUBI_MAX_DEPENDENCIES_V1
            || self
                .root_dependencies
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .root_dependencies
                .windows(2)
                .any(|pair| pair[0].alias >= pair[1].alias)
            || self.nodes.len() > MUSUBI_MAX_RESOLUTION_NODES_V1
            || self
                .nodes
                .windows(2)
                .any(|pair| pair[0].release >= pair[1].release)
            || self.nodes.iter().any(|node| node.release == self.root)
        {
            return Err(ParseError::new(
                "Musubi verification lock is invalid or noncanonical",
            ));
        }
        let nodes = self
            .nodes
            .iter()
            .map(|node| (&node.release, node))
            .collect::<BTreeMap<_, _>>();
        for dependency in &self.root_dependencies {
            dependency.validate()?;
            if dependency.kind != MusubiDependencyKindV1::Normal
                || !nodes.contains_key(&dependency.selected)
            {
                return Err(ParseError::new(
                    "Musubi root dependency must be normal and select an exact proof node",
                ));
            }
        }
        for node in &self.nodes {
            node.validate()?;
        }
        validate_exact_graph(&self.root_dependencies, &self.nodes)
    }
    /// Compute the normalized lock digest.
    #[must_use]
    pub fn digest(&self) -> MusubiVerificationLockDigestV1 {
        MusubiVerificationLockDigestV1(domain_hash_value(
            MUSUBI_VERIFICATION_LOCK_DIGEST_DOMAIN_V1,
            self,
        ))
    }
}

impl MusubiResolutionProofV1 {
    /// Validate the finalized anchor and exact graph.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry snapshot or verification lock is invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.snapshot.validate()?;
        self.lock.validate()
    }
}

impl MusubiSemanticReleaseManifestV1 {
    /// Decode one exact canonical semantic-release bundle file under the shared V1 limits.
    ///
    /// # Errors
    ///
    /// Returns one stable payload-free error when the file is empty, oversized, malformed,
    /// trailing, noncanonical, or fails semantic-release validation.
    pub fn decode_canonical_bundle_file(bytes: &[u8]) -> Result<Self, ParseError> {
        decode_canonical_bundle_file_v1(
            bytes,
            MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1,
            MUSUBI_SEMANTIC_RELEASE_DECODE_LIMITS_V1,
            Self::validate,
            "Musubi semantic release bundle file is invalid or out of bounds",
        )
    }
    /// Canonicalize every set-like semantic field before packaging.
    pub fn canonicalize(&mut self) {
        self.dependencies.sort();
        self.dependencies.dedup();
        self.exports.sort();
        self.exports.dedup();
        self.metadata.canonicalize();
    }
    /// Validate archive-independent release semantics and canonical ordering.
    ///
    /// # Errors
    ///
    /// Returns an error if a nested release field is invalid, collections exceed V1 bounds or are
    /// noncanonical, a required digest is zero, or the release depends on its own package.
    pub fn validate(&self) -> Result<(), ParseError> {
        streaming::validate_semantic_release_fields(
            &self.release,
            &self.abi,
            &self.dependencies,
            &self.exports,
            self.interface_digest,
            &self.metadata,
            self.verification_lock_digest,
        )
    }
    /// Validate this semantic release against its complete normalized verification lock.
    ///
    /// This is the shared bundle/publication boundary: both values must be independently valid,
    /// the lock must select this exact root and digest, and every published direct dependency must
    /// correspond one-for-one with a normal exact root edge carrying the same alias, package, and
    /// requirement.
    ///
    /// # Errors
    ///
    /// Returns an error if either value is invalid or their root, digest, direct-dependency count,
    /// dependency kind, alias, package, or requirement binding differs.
    pub fn validate_verification_lock(
        &self,
        verification_lock: &MusubiVerificationLockV1,
    ) -> Result<(), ParseError> {
        streaming::validate_semantic_release_lock(
            &self.release,
            &self.abi,
            &self.dependencies,
            &self.exports,
            &self.metadata,
            (self.interface_digest, self.verification_lock_digest),
            verification_lock,
        )
    }
    /// Domain-separated digest used inside bundles, staging receipts, and provider attestations.
    #[must_use]
    pub fn semantic_digest(&self) -> MusubiSemanticReleaseDigestV1 {
        MusubiSemanticReleaseDigestV1(domain_hash_value(
            MUSUBI_SEMANTIC_RELEASE_DIGEST_DOMAIN_V1,
            self,
        ))
    }
}

impl MusubiReleaseManifestV1 {
    /// Canonicalize set-like fields before publication.
    pub fn canonicalize(&mut self) {
        self.dependencies.sort();
        self.dependencies.dedup();
        self.exports.sort();
        self.exports.dedup();
        self.metadata.canonicalize();
    }
    /// Project the canonical archive-independent manifest embedded in the bundle.
    #[must_use]
    pub fn semantic_manifest(&self) -> MusubiSemanticReleaseManifestV1 {
        MusubiSemanticReleaseManifestV1 {
            release: self.release.clone(),
            edition: self.edition,
            abi: self.abi,
            dependencies: self.dependencies.clone(),
            exports: self.exports.clone(),
            interface_digest: self.interface_digest,
            metadata: self.metadata.clone(),
            verification_lock_digest: self.verification_lock_digest,
        }
    }
    /// Compute the archive-independent bundle/receipt/provider-attestation digest.
    #[must_use]
    pub fn semantic_digest(&self) -> MusubiSemanticReleaseDigestV1 {
        streaming::semantic_release_digest(self)
    }
    /// Explicit alias for [`Self::semantic_digest`].
    #[must_use]
    pub fn semantic_release_digest(&self) -> MusubiSemanticReleaseDigestV1 {
        self.semantic_digest()
    }
    /// Validate first-release release-manifest invariants.
    ///
    /// # Errors
    ///
    /// Returns an error if the semantic manifest is invalid or the archive identity is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        streaming::validate_semantic_release_fields(
            &self.release,
            &self.abi,
            &self.dependencies,
            &self.exports,
            self.interface_digest,
            &self.metadata,
            self.verification_lock_digest,
        )?;
        if self.archive_id.is_zero() {
            return Err(ParseError::new(
                "Musubi registry release manifest has an invalid archive identity",
            ));
        }
        Ok(())
    }
    fn validate_verification_lock(
        &self,
        verification_lock: &MusubiVerificationLockV1,
    ) -> Result<(), ParseError> {
        streaming::validate_semantic_release_lock(
            &self.release,
            &self.abi,
            &self.dependencies,
            &self.exports,
            &self.metadata,
            (self.interface_digest, self.verification_lock_digest),
            verification_lock,
        )
    }
    /// Domain-separated immutable release digest.
    #[must_use]
    pub fn release_digest(&self) -> MusubiReleaseDigestV1 {
        MusubiReleaseDigestV1(domain_hash_value(MUSUBI_RELEASE_DIGEST_DOMAIN_V1, self))
    }
}

impl MusubiPublicationV1 {
    /// Validate release, proof root, lock digest, and direct dependency selections.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest or proof is invalid, the proof does not bind the release
    /// and lock digest, or its exact direct dependencies differ from the manifest.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.manifest.validate()?;
        self.resolution.validate()?;
        self.manifest
            .validate_verification_lock(&self.resolution.lock)
    }
}

fn validate_exact_graph(
    root_dependencies: &[MusubiExactDependencyEdgeV1],
    nodes: &[MusubiVerificationNodeV1],
) -> Result<(), ParseError> {
    fn visit<'a>(
        release: &'a MusubiReleaseIdV1,
        depth: u16,
        by_release: &BTreeMap<&'a MusubiReleaseIdV1, &'a MusubiVerificationNodeV1>,
        visiting: &mut BTreeSet<&'a MusubiReleaseIdV1>,
        complete: &mut BTreeSet<&'a MusubiReleaseIdV1>,
    ) -> Result<(), ParseError> {
        if depth > MUSUBI_MAX_RESOLUTION_DEPTH_V1 {
            return Err(ParseError::new(
                "Musubi verification graph exceeds maximum depth",
            ));
        }
        if complete.contains(release) {
            return Ok(());
        }
        if !visiting.insert(release) {
            return Err(ParseError::new(
                "Musubi verification graph contains a cycle",
            ));
        }
        let node = by_release.get(release).ok_or_else(|| {
            ParseError::new("Musubi verification graph references a missing node")
        })?;
        for edge in &node.dependencies {
            visit(
                &edge.selected,
                depth.saturating_add(1),
                by_release,
                visiting,
                complete,
            )?;
        }
        visiting.remove(release);
        complete.insert(release);
        Ok(())
    }
    let by_release = nodes
        .iter()
        .map(|node| (&node.release, node))
        .collect::<BTreeMap<_, _>>();
    let mut complete = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    for dependency in root_dependencies {
        visit(
            &dependency.selected,
            1,
            &by_release,
            &mut visiting,
            &mut complete,
        )?;
    }
    if complete.len() != nodes.len() {
        return Err(ParseError::new(
            "Musubi verification graph contains unreachable exact nodes",
        ));
    }
    Ok(())
}
