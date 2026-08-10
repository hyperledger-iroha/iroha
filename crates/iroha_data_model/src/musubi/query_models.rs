// Resolver, finalized-query, and search wire models included at Musubi module scope.
/// Compact universal sparse-index row used by exact resolution.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiResolverReleaseRowV1 {
    /// Exact release identity.
    pub release: MusubiReleaseIdV1,
    /// Immutable release digest.
    pub release_digest: MusubiReleaseDigestV1,
    /// Archive identity.
    pub archive_id: ArchiveId,
    /// Source-tree digest.
    pub source_digest: MusubiContentDigestV1,
    /// Typed-interface digest.
    pub interface_digest: MusubiContentDigestV1,
    /// ABI binding.
    pub abi: MusubiAbiBindingV1,
    /// Sorted normal dependency ranges with unique parent-local aliases.
    pub dependencies: Vec<MusubiDependencyReqV1>,
    /// Independent selection state.
    pub selection: MusubiReleaseSelectionStateV1,
    /// Universal index revision.
    pub index_revision: u64,
}

impl MusubiResolverReleaseRowV1 {
    /// Validate compact resolver commitments and canonical dependency order.
    ///
    /// # Errors
    ///
    /// Returns an error if a nested projection is invalid, commitments or revision are zero,
    /// dependencies are oversized or noncanonical, selection identities do not match the row, or
    /// the availability projection is newer than the resolver row.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.release.validate()?;
        self.abi.validate()?;
        self.selection.validate()?;
        if self.release_digest.is_zero()
            || self.archive_id.is_zero()
            || self.source_digest.is_zero()
            || self.interface_digest.is_zero()
            || self.index_revision == 0
            || self.dependencies.len() > MUSUBI_MAX_DEPENDENCIES_V1
            || self.dependencies.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .dependencies
                .windows(2)
                .any(|pair| pair[0].alias >= pair[1].alias)
            || self.selection.yank.release != self.release
            || self.selection.storage.archive_id != self.archive_id
            || self.selection.storage.index_revision > self.index_revision
        {
            return Err(ParseError::new(
                "Musubi resolver row is invalid or noncanonical",
            ));
        }
        self.dependencies
            .iter()
            .try_for_each(MusubiDependencyReqV1::validate)
    }
}

/// Paired home-dataspace and universal-index view of one exact release at finality.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiExactReleaseSnapshotV1 {
    /// Exact deployment identity derived from the committed genesis header.
    pub network_id: NetworkId,
    /// Finalized universal registry snapshot shared by both projections.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Authoritative release record from the stable home dataspace.
    pub home_release: MusubiReleaseRecordV1,
    /// Exact resolver-grade release row from the universal dataspace.
    pub universal_release: MusubiResolverReleaseRowV1,
}

impl MusubiExactReleaseSnapshotV1 {
    /// Validate deployment identity, paired content/state, revisions, and finalized anchors.
    ///
    /// # Errors
    ///
    /// Returns an error if a nested projection or deployment identity is invalid, the home and
    /// universal views disagree, or a revision, transition, or storage anchor is not finalized.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.snapshot.validate()?;
        self.home_release.validate()?;
        self.universal_release.validate()?;

        let manifest = &self.home_release.manifest;
        let universal = &self.universal_release;
        let storage = &universal.selection.storage;
        let takedown_height = match &self.home_release.artifact_governance {
            MusubiArtifactGovernanceStateV1::Available => 0,
            MusubiArtifactGovernanceStateV1::TakenDown(takedown) => takedown.applied_at_height,
        };
        if self.network_id.as_bytes()[31] & 1 != 1
            || manifest.release != universal.release
            || self.home_release.release_digest != universal.release_digest
            || manifest.archive_id != universal.archive_id
            || manifest.interface_digest != universal.interface_digest
            || manifest.abi != universal.abi
            || manifest.dependencies != universal.dependencies
            || self.home_release.yank != universal.selection.yank
            || self.home_release.artifact_governance != universal.selection.governance
            || self.home_release.revisions.yank > self.snapshot.index_revision
            || self.home_release.revisions.artifact_governance > self.snapshot.index_revision
            || universal.index_revision > self.snapshot.index_revision
            || storage.index_revision > universal.index_revision
            || storage.index_revision > self.snapshot.index_revision
            || self.home_release.published_at_height > self.snapshot.finalized_height
            || self.home_release.yank.changed_at_height < self.home_release.published_at_height
            || self.home_release.yank.changed_at_height > self.snapshot.finalized_height
            || (takedown_height != 0 && takedown_height < self.home_release.published_at_height)
            || takedown_height > self.snapshot.finalized_height
            || storage.finalized_height > self.snapshot.finalized_height
            || (self.snapshot.finalized_height == 1
                && self.network_id.as_bytes() != &self.snapshot.finalized_block_hash)
            || (storage.finalized_height == self.snapshot.finalized_height
                && storage.finalized_block_hash != self.snapshot.finalized_block_hash)
        {
            return Err(ParseError::new(
                "Musubi exact release snapshot is inconsistent or not finalized",
            ));
        }
        Ok(())
    }

    /// Validate this paired result for one exact requested release.
    ///
    /// # Errors
    ///
    /// Returns an error if the query release or snapshot is invalid, or either paired projection
    /// carries a different release.
    pub fn validate_for(&self, query: &MusubiExactReleaseQueryV1) -> Result<(), ParseError> {
        query.release.validate()?;
        self.validate()?;
        if self.home_release.manifest.release != query.release
            || self.universal_release.release != query.release
        {
            return Err(ParseError::new(
                "Musubi exact release snapshot carries a different release",
            ));
        }
        Ok(())
    }
}

/// Finalized cursor binding its exact query, last key, index revision, and optional caller.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiFinalizedCursorV1 {
    /// Finalized registry snapshot.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Canonical query hash.
    pub query_hash: MusubiQueryHashV1,
    /// Last returned ordered key.
    pub last_key: String,
    /// Caller binding for authorization-sensitive queries.
    pub caller: Option<AccountId>,
}

impl MusubiFinalizedCursorV1 {
    /// Validate all cursor bindings.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot or caller is invalid, the query hash is zero, or the last
    /// key is empty, overlong, or contains control characters.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.snapshot.validate()?;
        if let Some(caller) = &self.caller {
            validate_musubi_account_id_v1(caller)?;
        }
        if self.query_hash.is_zero()
            || self.last_key.is_empty()
            || self.last_key.len() > MUSUBI_MAX_CURSOR_KEY_BYTES_V1
            || self.last_key.chars().any(char::is_control)
        {
            return Err(ParseError::new("Musubi finalized cursor is invalid"));
        }
        Ok(())
    }
}

/// Explicit stale-cursor classification returned instead of silently restarting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiCursorFailureV1 {
    /// Finalized height/hash no longer matches the requested snapshot.
    FinalizedAnchorMismatch,
    /// Query hash differs.
    QueryMismatch,
    /// Universal sparse-index revision differs.
    IndexRevisionMismatch,
    /// Caller binding differs.
    CallerMismatch,
    /// Last key is absent from the requested ordered index.
    LastKeyStale,
}

macro_rules! musubi_page_type {
    ($name:ident, $item:ty, $doc:literal, $noncanonical_order:expr) => {
        #[doc = $doc]
        #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
        #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
        pub struct $name {
            /// Ordered result items.
            pub items: Vec<$item>,
            /// Cursor for the next page, absent at the end.
            pub next_cursor: Option<MusubiFinalizedCursorV1>,
            /// Finalized snapshot shared by every item.
            pub snapshot: MusubiRegistrySnapshotV1,
        }

        impl $name {
            /// Validate page size, snapshot, and cursor.
            ///
            /// # Errors
            ///
            /// Returns an error if the snapshot or an item is invalid, items exceed the page
            /// bound or are noncanonical, or the next cursor is invalid or changes snapshots.
            pub fn validate(&self) -> Result<(), ParseError> {
                self.snapshot.validate()?;
                if self.items.len() > MUSUBI_MAX_PAGE_SIZE_V1
                    || self.items.windows(2).any($noncanonical_order)
                {
                    return Err(ParseError::new(
                        "Musubi query page exceeds its item bound or is not strictly ordered",
                    ));
                }
                self.items.iter().try_for_each(<$item>::validate)?;
                if let Some(cursor) = &self.next_cursor {
                    cursor.validate()?;
                    if cursor.snapshot != self.snapshot {
                        return Err(ParseError::new(
                            "Musubi query page cursor uses a different finalized snapshot",
                        ));
                    }
                }
                Ok(())
            }
        }
    };
}

musubi_page_type!(
    MusubiPackagePageV1,
    MusubiPackageRecordV1,
    "Ordered page of exact package records.",
    |pair: &[MusubiPackageRecordV1]| pair[0].package >= pair[1].package
);
musubi_page_type!(
    MusubiReleasePageV1,
    MusubiReleaseRecordV1,
    "Ordered page of release records with yank, takedown, and revision projections.",
    |pair: &[MusubiReleaseRecordV1]| pair[0].manifest.release >= pair[1].manifest.release
);
/// Ordered page of structured package versions bound to its exact request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiVersionPageV1 {
    /// Exact request whose results this page carries.
    pub query: MusubiPackagePageQueryV1,
    /// Ordered structured versions for `query.package`.
    pub items: Vec<MusubiVersionV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by every item.
    pub snapshot: MusubiRegistrySnapshotV1,
}

impl MusubiVersionPageV1 {
    /// Validate request identity, page bounds, strict order, and cursor binding.
    ///
    /// # Errors
    ///
    /// Returns an error if query, snapshot, version, or cursor data is invalid, versions are not
    /// strictly ordered, the page does not advance its cursor, or page bounds do not match.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.query.validate()?;
        self.snapshot.validate()?;
        if self.items.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ParseError::new(
                "Musubi version page is not strictly ordered",
            ));
        }
        self.items.iter().try_for_each(MusubiVersionV1::validate)?;
        if let Some(cursor) = &self.query.page.cursor {
            let previous = cursor
                .last_key
                .parse::<MusubiVersionV1>()
                .map_err(|_| ParseError::new("Musubi version cursor key is invalid"))?;
            if self
                .items
                .first()
                .is_some_and(|version| version <= &previous)
            {
                return Err(ParseError::new(
                    "Musubi version page does not advance its structured cursor",
                ));
            }
        }
        let first_key = self.items.first().map(ToString::to_string);
        let last_key = self.items.last().map(ToString::to_string);
        validate_finalized_response_page(
            &self.query.page,
            self.items.len(),
            first_key.as_deref(),
            last_key.as_deref(),
            self.next_cursor.as_ref(),
            self.snapshot,
        )
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    ///
    /// # Errors
    ///
    /// Returns an error if the page is invalid or its embedded query differs from `query`.
    pub fn validate_for(&self, query: &MusubiPackagePageQueryV1) -> Result<(), ParseError> {
        self.validate()?;
        if &self.query != query {
            return Err(ParseError::new(
                "Musubi version page carries a different request context",
            ));
        }
        Ok(())
    }
}

/// Ordered page of package members and invitations bound to its exact request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiMaintainerPageV1 {
    /// Exact request whose results this page carries.
    pub query: MusubiPackagePageQueryV1,
    /// Ordered accepted package members and pending invitations.
    pub items: Vec<MusubiMaintainerDirectoryEntryV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by every item.
    pub snapshot: MusubiRegistrySnapshotV1,
}

impl MusubiMaintainerPageV1 {
    /// Validate request identity, package membership, bounds, order, and cursor binding.
    ///
    /// # Errors
    ///
    /// Returns an error if query, snapshot, entry, or cursor data is invalid, entries are not
    /// strictly ordered or belong to another package, or page bounds do not match.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.query.validate()?;
        self.snapshot.validate()?;
        if self
            .items
            .windows(2)
            .any(|pair| pair[0].key() >= pair[1].key())
        {
            return Err(ParseError::new(
                "Musubi maintainer page is not strictly ordered",
            ));
        }
        for entry in &self.items {
            entry.validate()?;
            if entry.key().package != self.query.package {
                return Err(ParseError::new(
                    "Musubi maintainer page item belongs to a different package",
                ));
            }
        }
        if let Some(cursor) = &self.query.page.cursor
            && (!maintainer_cursor_key_is_canonical_v1(&cursor.last_key)
                || self
                    .items
                    .iter()
                    .any(|entry| entry.cursor_key() == cursor.last_key))
        {
            return Err(ParseError::new(
                "Musubi maintainer page does not advance its exact cursor boundary",
            ));
        }
        let first_key = self
            .items
            .first()
            .map(MusubiMaintainerDirectoryEntryV1::cursor_key);
        let last_key = self
            .items
            .last()
            .map(MusubiMaintainerDirectoryEntryV1::cursor_key);
        validate_finalized_response_page(
            &self.query.page,
            self.items.len(),
            first_key.as_deref(),
            last_key.as_deref(),
            self.next_cursor.as_ref(),
            self.snapshot,
        )
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    ///
    /// # Errors
    ///
    /// Returns an error if the page is invalid or its embedded query differs from `query`.
    pub fn validate_for(&self, query: &MusubiPackagePageQueryV1) -> Result<(), ParseError> {
        self.validate()?;
        if &self.query != query {
            return Err(ParseError::new(
                "Musubi maintainer page carries a different request context",
            ));
        }
        Ok(())
    }
}
/// Ordered renewable locations plus their authoritative immutable archive commitment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArchiveLocationPageV1 {
    /// Exact deployment identity used by locks and archive admission.
    pub network_id: NetworkId,
    /// Current authoritative archive record and full source commitment.
    ///
    /// [`MusubiArchiveRecordV1::registration_projection`] excludes this record's mutable
    /// location directory for finality checks that outlive the named snapshot.
    pub archive: MusubiArchiveRecordV1,
    /// Ordered current non-retired locations for the archive.
    pub items: Vec<MusubiArchiveLocationV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by the archive and every location.
    pub snapshot: MusubiRegistrySnapshotV1,
}

/// Authoritative cache-retention classification for one exact archive identity.
///
/// An identity unknown to the queried registry is retained fail-closed because
/// the user cache is content-addressed but not chain-scoped. Replication health
/// never makes a published archive prunable: locked consumers may still need a
/// cached below-quorum or unavailable archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
pub enum MusubiArchiveRetentionDispositionV1 {
    /// This registry does not know the archive, so it cannot authorize deletion.
    RetainUnknown,
    /// At least one governance-available active or yanked release references the archive.
    RetainReferenced,
    /// The registered archive has no published release references.
    PruneUnreferenced,
    /// Every published release reference has an enacted Parliament takedown.
    PruneGovernedTakedown,
}

impl MusubiArchiveRetentionDispositionV1 {
    /// Whether this finalized classification requires the local cache entry to remain.
    #[must_use]
    pub const fn must_retain(self) -> bool {
        matches!(self, Self::RetainUnknown | Self::RetainReferenced)
    }
}

/// One exact finalized cache-retention decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArchiveRetentionDecisionV1 {
    /// Exact content-addressed archive identity.
    pub archive_id: ArchiveId,
    /// Fail-closed retention or explicit prune classification.
    pub disposition: MusubiArchiveRetentionDispositionV1,
    /// Governance-available, non-yanked release references.
    pub active_releases: u16,
    /// Governance-available, yanked release references.
    pub yanked_releases: u16,
    /// Parliament-taken-down release references.
    pub taken_down_releases: u16,
    /// Authoritative storage projection, absent only for an unknown archive.
    pub storage: Option<MusubiArchiveAvailabilityV1>,
}

impl MusubiArchiveRetentionDecisionV1 {
    /// Return whether this exact finalized decision requires retention.
    #[must_use]
    pub const fn must_retain(&self) -> bool {
        self.disposition.must_retain()
    }

    /// Validate identity, bounded counts, storage binding, and disposition semantics.
    ///
    /// # Errors
    ///
    /// Returns an error if the archive is zero, release counts overflow or exceed V1 bounds, a
    /// storage projection is invalid or mismatched, or the disposition contradicts the counts.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.archive_id.is_zero() {
            return Err(ParseError::new(
                "Musubi archive retention decision uses the zero archive identity",
            ));
        }
        let referenced = usize::from(self.active_releases)
            .checked_add(usize::from(self.yanked_releases))
            .and_then(|count| count.checked_add(usize::from(self.taken_down_releases)))
            .ok_or_else(|| ParseError::new("Musubi archive retention count overflow"))?;
        if referenced > MUSUBI_MAX_RESOLUTION_NODES_V1 {
            return Err(ParseError::new(
                "Musubi archive retention decision exceeds the release-reference bound",
            ));
        }
        if let Some(storage) = &self.storage {
            storage.validate()?;
            if storage.archive_id != self.archive_id {
                return Err(ParseError::new(
                    "Musubi archive retention storage projection has a different identity",
                ));
            }
        }

        let available = usize::from(self.active_releases)
            .checked_add(usize::from(self.yanked_releases))
            .expect("two u16 Musubi release counts fit usize");
        let canonical = match self.disposition {
            MusubiArchiveRetentionDispositionV1::RetainUnknown => {
                referenced == 0 && self.storage.is_none()
            }
            MusubiArchiveRetentionDispositionV1::RetainReferenced => {
                available > 0 && self.storage.is_some()
            }
            MusubiArchiveRetentionDispositionV1::PruneUnreferenced => {
                referenced == 0 && self.storage.is_some()
            }
            MusubiArchiveRetentionDispositionV1::PruneGovernedTakedown => {
                available == 0 && self.taken_down_releases > 0 && self.storage.is_some()
            }
        };
        if !canonical {
            return Err(ParseError::new(
                "Musubi archive retention decision is internally inconsistent",
            ));
        }
        Ok(())
    }
}

/// Bounded exact finalized cache-retention request.
///
/// `expected_snapshot` is absent on the first batch and binds every later batch
/// in the same prune operation. A node must reject a mismatching anchor instead
/// of combining decisions from different finalized registry states.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArchiveRetentionQueryV1 {
    /// Sorted, distinct, non-zero exact archive identities.
    pub archive_ids: Vec<ArchiveId>,
    /// Exact finalized snapshot established by the first batch, when present.
    pub expected_snapshot: Option<MusubiRegistrySnapshotV1>,
}

impl MusubiArchiveRetentionQueryV1 {
    /// Validate the exact batch bound, order, identities, and optional snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if archive identities are empty, oversized, zero, unsorted, or duplicated,
    /// or if the expected snapshot is invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.archive_ids.is_empty()
            || self.archive_ids.len() > MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1
            || self.archive_ids.iter().any(ArchiveId::is_zero)
            || self.archive_ids.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ParseError::new(
                "Musubi archive retention batch is empty, oversized, or noncanonical",
            ));
        }
        self.expected_snapshot
            .as_ref()
            .map_or(Ok(()), MusubiRegistrySnapshotV1::validate)
    }
}

/// Exact finalized cache-retention decisions for one bounded request batch.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArchiveRetentionPageV1 {
    /// Exact deployment identity queried for these decisions.
    pub network_id: NetworkId,
    /// Decisions in the exact order of the canonical request identities.
    pub items: Vec<MusubiArchiveRetentionDecisionV1>,
    /// Finalized universal registry snapshot shared by every decision.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Consensus-committed creation time of the block named by `snapshot`.
    ///
    /// This may be zero for bootstrap fixtures. A publication expiry proof requires it to be
    /// strictly later than the exact signed transaction and receipt validity window.
    pub finalized_time_ms: u64,
}

impl MusubiArchiveRetentionPageV1 {
    /// Validate deployment identity, bounded strict order, decisions, and snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if deployment or snapshot data is invalid, decisions are empty,
    /// oversized, noncanonical, or invalid, or storage anchors exceed the page snapshot.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.snapshot.validate()?;
        if self.network_id.as_bytes()[31] & 1 != 1
            || self.items.is_empty()
            || self.items.len() > MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1
            || self
                .items
                .windows(2)
                .any(|pair| pair[0].archive_id >= pair[1].archive_id)
            || self.items.iter().any(|decision| {
                decision.storage.is_some_and(|storage| {
                    storage.finalized_height > self.snapshot.finalized_height
                        || storage.index_revision > self.snapshot.index_revision
                        || (storage.finalized_height == self.snapshot.finalized_height
                            && storage.finalized_block_hash != self.snapshot.finalized_block_hash)
                })
            })
        {
            return Err(ParseError::new(
                "Musubi archive retention page has an invalid deployment or item bound",
            ));
        }
        self.items
            .iter()
            .try_for_each(MusubiArchiveRetentionDecisionV1::validate)
    }
}

impl MusubiArchiveLocationPageV1 {
    /// Validate deployment identity, archive commitment, items, snapshot, and cursor.
    ///
    /// # Errors
    ///
    /// Returns an error if deployment, archive, snapshot, location, or cursor data is invalid,
    /// locations are oversized or noncanonical, or an item is not current at the snapshot.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.archive.validate()?;
        self.snapshot.validate()?;
        if self.network_id.as_bytes()[31] & 1 != 1
            || self.archive.staging_receipt.payload.binding.network_id != self.network_id
            || self.archive.registered_at_height > self.snapshot.finalized_height
            || self.items.len() > MUSUBI_MAX_ARCHIVE_LOCATIONS_V1
            || self
                .items
                .windows(2)
                .any(|pair| pair[0].location_id >= pair[1].location_id)
        {
            return Err(ParseError::new(
                "Musubi archive-location page has an inconsistent deployment or item bound",
            ));
        }
        for location in &self.items {
            location.validate()?;
            if location.archive_id != self.archive.archive_id
                || self
                    .archive
                    .location_ids
                    .binary_search(&location.location_id)
                    .is_err()
                || location.finalized_height > self.snapshot.finalized_height
                || location.revision > self.archive.location_revision
                || location.state == MusubiArchiveLocationStateV1::Retired
            {
                return Err(ParseError::new(
                    "Musubi archive-location page item is not a current archive location",
                ));
            }
        }
        if let Some(cursor) = &self.next_cursor {
            cursor.validate()?;
            if cursor.snapshot != self.snapshot {
                return Err(ParseError::new(
                    "Musubi archive-location page cursor uses a different finalized snapshot",
                ));
            }
        }
        Ok(())
    }
}
/// Ordered page of permanent alias history bound to its exact request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiAliasHistoryPageV1 {
    /// Exact request whose results this page carries.
    pub query: MusubiAliasQueryV1,
    /// Ordered permanent history for `query.alias`.
    pub items: Vec<MusubiAliasHistoryEntryV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by every item.
    pub snapshot: MusubiRegistrySnapshotV1,
}

impl MusubiAliasHistoryPageV1 {
    /// Validate request identity, alias membership, bounds, order, and cursor binding.
    ///
    /// # Errors
    ///
    /// Returns an error if query, snapshot, entry, or cursor data is invalid, entries are not
    /// strictly ordered or belong to another alias, or page bounds do not match.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.query.validate()?;
        self.snapshot.validate()?;
        if self
            .items
            .windows(2)
            .any(|pair| pair[0].key() >= pair[1].key())
        {
            return Err(ParseError::new(
                "Musubi alias-history page is not strictly ordered",
            ));
        }
        for entry in &self.items {
            entry.validate()?;
            if entry.alias != self.query.alias {
                return Err(ParseError::new(
                    "Musubi alias-history page item belongs to a different alias",
                ));
            }
        }
        if let Some(cursor) = &self.query.page.cursor {
            let (alias, revision) = cursor
                .last_key
                .rsplit_once(':')
                .ok_or_else(|| ParseError::new("Musubi alias-history cursor key is invalid"))?;
            if revision.len() != 20 {
                return Err(ParseError::new(
                    "Musubi alias-history cursor key is invalid",
                ));
            }
            let revision = revision
                .parse::<u64>()
                .map_err(|_| ParseError::new("Musubi alias-history cursor key is invalid"))?;
            if alias != self.query.alias.as_str()
                || self.items.first().is_some_and(|entry| {
                    entry.key() <= MusubiAliasHistoryKeyV1::new(self.query.alias.clone(), revision)
                })
            {
                return Err(ParseError::new(
                    "Musubi alias-history page does not advance its structured cursor",
                ));
            }
        }
        let cursor_key =
            |entry: &MusubiAliasHistoryEntryV1| format!("{}:{:020}", entry.alias, entry.revision);
        let first_key = self.items.first().map(cursor_key);
        let last_key = self.items.last().map(cursor_key);
        validate_finalized_response_page(
            &self.query.page,
            self.items.len(),
            first_key.as_deref(),
            last_key.as_deref(),
            self.next_cursor.as_ref(),
            self.snapshot,
        )
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    ///
    /// # Errors
    ///
    /// Returns an error if the page is invalid or its embedded query differs from `query`.
    pub fn validate_for(&self, query: &MusubiAliasQueryV1) -> Result<(), ParseError> {
        self.validate()?;
        if &self.query != query {
            return Err(ParseError::new(
                "Musubi alias-history page carries a different request context",
            ));
        }
        Ok(())
    }
}
/// Ordered page of universal resolver-index rows with authoritative lock identity.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiResolverIndexPageV1 {
    /// Exact request whose rows this page carries.
    pub query: MusubiResolverIndexQueryV1,
    /// Exact deployment identity used by generated lockfiles.
    pub network_id: NetworkId,
    /// Ordered universal resolver-index rows.
    pub items: Vec<MusubiResolverReleaseRowV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by every item.
    pub snapshot: MusubiRegistrySnapshotV1,
}

impl MusubiResolverIndexPageV1 {
    /// Validate request identity, lock identity, page bounds, rows, and cursor binding.
    ///
    /// # Errors
    ///
    /// Returns an error if deployment, query, snapshot, row, or cursor data is invalid, rows are
    /// noncanonical or outside the requested package/range, or page bounds do not match.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.query.validate()?;
        if self.network_id.as_bytes()[31] & 1 != 1
            || self
                .items
                .windows(2)
                .any(|pair| pair[0].release >= pair[1].release)
        {
            return Err(ParseError::new(
                "Musubi resolver page has an invalid network identity or item bound",
            ));
        }
        for row in &self.items {
            row.validate()?;
            if row.release.package != self.query.package
                || self
                    .query
                    .requirement
                    .as_ref()
                    .is_some_and(|requirement| !requirement.matches(&row.release.version))
            {
                return Err(ParseError::new(
                    "Musubi resolver row does not match its response request context",
                ));
            }
        }
        if let Some(cursor) = &self.query.page.cursor {
            let previous = cursor
                .last_key
                .parse::<MusubiVersionV1>()
                .map_err(|_| ParseError::new("Musubi resolver cursor key is invalid"))?;
            if self
                .items
                .first()
                .is_some_and(|row| row.release.version <= previous)
            {
                return Err(ParseError::new(
                    "Musubi resolver page does not advance its structured cursor",
                ));
            }
        }
        self.snapshot.validate()?;
        let first_key = self
            .items
            .first()
            .map(|row| row.release.version.to_string());
        let last_key = self.items.last().map(|row| row.release.version.to_string());
        validate_finalized_response_page_with_cursor_cardinality(
            &self.query.page,
            self.items.len(),
            first_key.as_deref(),
            last_key.as_deref(),
            self.next_cursor.as_ref(),
            self.snapshot,
            false,
        )?;
        #[cfg(feature = "json")]
        {
            let encoded = norito::json::to_json(self).map_err(|_| {
                ParseError::new("Musubi resolver page cannot be encoded as canonical JSON")
            })?;
            if encoded.len() > MUSUBI_PUBLIC_QUERY_MAX_RESPONSE_BYTES_V1 {
                return Err(ParseError::new(
                    "Musubi resolver page exceeds the public JSON response ceiling",
                ));
            }
        }
        Ok(())
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    ///
    /// # Errors
    ///
    /// Returns an error if the page is invalid or its embedded query differs from `query`.
    pub fn validate_for(&self, query: &MusubiResolverIndexQueryV1) -> Result<(), ParseError> {
        self.validate()?;
        if &self.query != query {
            return Err(ParseError::new(
                "Musubi resolver page carries a different request context",
            ));
        }
        Ok(())
    }
}

/// Compact ordered directory row; rich fuzzy search may rebuild this projection.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiOrderedPackageEntryV1 {
    /// Human-facing namespace/package selector.
    pub selector: MusubiPackageSelectorV1,
    /// Structural package identity stored in manifests and locks.
    pub package: MusubiPackageIdV1,
    /// Highest fresh-selectable version, if any.
    pub latest_selectable: Option<MusubiVersionV1>,
    /// Package metadata revision projected into the directory.
    pub metadata_revision: u64,
    /// Universal directory revision.
    pub index_revision: u64,
}

impl MusubiOrderedPackageEntryV1 {
    /// Validate non-zero revisions and any structured version.
    ///
    /// # Errors
    ///
    /// Returns an error if selector, package, or optional version data is invalid, or if either
    /// directory revision is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.selector.validate()?;
        self.package.validate()?;
        if self.metadata_revision == 0 || self.index_revision == 0 {
            return Err(ParseError::new(
                "Musubi ordered package entry has an invalid revision",
            ));
        }
        self.latest_selectable
            .as_ref()
            .map_or(Ok(()), MusubiVersionV1::validate)
    }
}

/// Ordered package-directory response with authoritative lock identity.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiOrderedPackagePageV1 {
    /// Exact request whose directory rows this page carries.
    pub query: MusubiOrderedPrefixQueryV1,
    /// Exact deployment identity used by generated lockfiles.
    pub network_id: NetworkId,
    /// Authoritative immutable namespace binding, present even when no package matches.
    pub namespace_binding: MusubiNamespaceBindingV1,
    /// Ordered public-directory entries.
    pub items: Vec<MusubiOrderedPackageEntryV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by every item.
    pub snapshot: MusubiRegistrySnapshotV1,
}

impl MusubiOrderedPackagePageV1 {
    /// Validate request identity, lock identity, rows, bounds, and cursor binding.
    ///
    /// # Errors
    ///
    /// Returns an error if deployment, query, binding, snapshot, item, or cursor data is invalid,
    /// items are noncanonical or inconsistent with the prefix/binding, or page bounds do not match.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.query.validate()?;
        self.namespace_binding.validate()?;
        let (namespace, _) = self.query.prefix.components()?;
        if self.network_id.as_bytes()[31] & 1 != 1
            || namespace != self.namespace_binding.namespace
            || self
                .items
                .windows(2)
                .any(|pair| pair[0].selector >= pair[1].selector)
        {
            return Err(ParseError::new(
                "Musubi directory page has an invalid network identity or item bound",
            ));
        }
        for item in &self.items {
            item.validate()?;
            if item.selector.namespace != self.namespace_binding.namespace
                || item.package.home_dataspace != self.namespace_binding.home_dataspace
                || item.package.scope != self.namespace_binding.scope
                || item.package.name != item.selector.name
                || !item
                    .selector
                    .to_string()
                    .starts_with(self.query.prefix.as_str())
            {
                return Err(ParseError::new(
                    "Musubi directory page item disagrees with its request or namespace binding",
                ));
            }
        }
        if let Some(cursor) = &self.query.page.cursor {
            let previous = cursor
                .last_key
                .parse::<MusubiPackageSelectorV1>()
                .map_err(|_| ParseError::new("Musubi directory cursor key is invalid"))?;
            if previous.namespace != namespace
                || !previous.to_string().starts_with(self.query.prefix.as_str())
                || self
                    .items
                    .first()
                    .is_some_and(|item| item.selector <= previous)
            {
                return Err(ParseError::new(
                    "Musubi directory page does not advance its structured cursor",
                ));
            }
        }
        self.snapshot.validate()?;
        let first_key = self.items.first().map(|item| item.selector.to_string());
        let last_key = self.items.last().map(|item| item.selector.to_string());
        validate_finalized_response_page(
            &self.query.page,
            self.items.len(),
            first_key.as_deref(),
            last_key.as_deref(),
            self.next_cursor.as_ref(),
            self.snapshot,
        )
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    ///
    /// # Errors
    ///
    /// Returns an error if the page is invalid or its embedded query differs from `query`.
    pub fn validate_for(&self, query: &MusubiOrderedPrefixQueryV1) -> Result<(), ParseError> {
        self.validate()?;
        if &self.query != query {
            return Err(ParseError::new(
                "Musubi directory page carries a different request context",
            ));
        }
        Ok(())
    }
}

/// Exact package lookup request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiExactPackageQueryV1 {
    /// Structural package identity.
    pub package: MusubiPackageIdV1,
}

/// Exact release lookup request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiExactReleaseQueryV1 {
    /// Exact release identity.
    pub release: MusubiReleaseIdV1,
}

/// Bounded ordered-prefix selector for deterministic directory/index queries.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiOrderedPrefixV1(String);

impl MusubiOrderedPrefixV1 {
    /// Parse a canonical `namespace/package-prefix` directory prefix.
    ///
    /// # Errors
    ///
    /// Returns an error if `raw` is empty, noncanonical, overlong, lacks its separator, contains
    /// an invalid namespace, or has a nonportable package-name prefix.
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        parse_clean(
            raw,
            "Musubi ordered prefix must not be empty",
            "Musubi ordered prefix is invalid",
        )?;
        if raw.len() > MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1 {
            return Err(ParseError::new("Musubi ordered prefix exceeds its bound"));
        }
        let prefix = Self(raw.to_owned());
        prefix.components()?;
        Ok(prefix)
    }

    /// Return prefix text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return the structural namespace and portable package-name prefix.
    ///
    /// # Errors
    ///
    /// Returns an error if the prefix lacks its separator, contains an invalid namespace, or has
    /// an overlong or nonportable package-name component.
    pub fn components(&self) -> Result<(MusubiNamespaceV1, &str), ParseError> {
        let (namespace, name_prefix) = self.0.split_once('/').ok_or_else(|| {
            ParseError::new("Musubi ordered prefix must use namespace/package-prefix")
        })?;
        if name_prefix.contains('/')
            || name_prefix.len() > MUSUBI_MAX_PACKAGE_NAME_BYTES_V1
            || name_prefix.starts_with('-')
            || name_prefix.contains("--")
            || !name_prefix
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(ParseError::new(
                "Musubi ordered package prefix is not portable canonical text",
            ));
        }
        Ok((namespace.parse()?, name_prefix))
    }

    /// Validate prefix text obtained through decoding.
    ///
    /// # Errors
    ///
    /// Returns an error if the decoded prefix is empty, noncanonical, overlong, or structurally
    /// invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        Self::new(&self.0).map(|_| ())
    }
}

/// Shared finalized page request for versions, members, locations, aliases, and prefix scans.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiPageRequestV1 {
    /// Requested count; zero selects [`MUSUBI_DEFAULT_PAGE_SIZE_V1`].
    pub limit: u32,
    /// Continuation cursor.
    pub cursor: Option<MusubiFinalizedCursorV1>,
}

impl MusubiPageRequestV1 {
    /// Effective page size capped by the consensus maximum.
    #[must_use]
    pub fn effective_limit(&self) -> usize {
        let requested = if self.limit == 0 {
            MUSUBI_DEFAULT_PAGE_SIZE_V1
        } else {
            self.limit
        };
        usize::try_from(requested)
            .unwrap_or(usize::MAX)
            .min(MUSUBI_MAX_PAGE_SIZE_V1)
    }

    /// Validate the requested bound and any supplied cursor.
    ///
    /// # Errors
    ///
    /// Returns an error if a nonzero limit exceeds the V1 page maximum or the cursor is invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.limit != 0
            && usize::try_from(self.limit).map_or(true, |limit| limit > MUSUBI_MAX_PAGE_SIZE_V1)
        {
            return Err(ParseError::new(
                "Musubi query page limit exceeds the consensus maximum",
            ));
        }
        self.cursor
            .as_ref()
            .map_or(Ok(()), MusubiFinalizedCursorV1::validate)
    }
}

fn validate_finalized_response_page(
    request: &MusubiPageRequestV1,
    item_count: usize,
    first_key: Option<&str>,
    last_key: Option<&str>,
    next_cursor: Option<&MusubiFinalizedCursorV1>,
    snapshot: MusubiRegistrySnapshotV1,
) -> Result<(), ParseError> {
    validate_finalized_response_page_with_cursor_cardinality(
        request,
        item_count,
        first_key,
        last_key,
        next_cursor,
        snapshot,
        true,
    )
}

fn validate_finalized_response_page_with_cursor_cardinality(
    request: &MusubiPageRequestV1,
    item_count: usize,
    first_key: Option<&str>,
    last_key: Option<&str>,
    next_cursor: Option<&MusubiFinalizedCursorV1>,
    snapshot: MusubiRegistrySnapshotV1,
    next_cursor_requires_full_page: bool,
) -> Result<(), ParseError> {
    request.validate()?;
    if item_count > request.effective_limit()
        || (item_count == 0 && (first_key.is_some() || last_key.is_some()))
        || (item_count > 0 && (first_key.is_none() || last_key.is_none()))
    {
        return Err(ParseError::new(
            "Musubi response page exceeds its requested bound or has invalid keys",
        ));
    }
    if let Some(cursor) = &request.cursor
        && (cursor.snapshot != snapshot || cursor.caller.is_some())
    {
        return Err(ParseError::new(
            "Musubi response page does not continue its request cursor",
        ));
    }
    if let Some(cursor) = next_cursor {
        cursor.validate()?;
        if cursor.snapshot != snapshot
            || cursor.caller.is_some()
            || (next_cursor_requires_full_page && item_count != request.effective_limit())
            || Some(cursor.last_key.as_str()) != last_key
            || request
                .cursor
                .as_ref()
                .is_some_and(|previous| previous.query_hash != cursor.query_hash)
        {
            return Err(ParseError::new(
                "Musubi response next cursor does not bind its exact response page",
            ));
        }
    }
    Ok(())
}

/// Resolver-index range request; exact resolution never uses fuzzy search.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiResolverIndexQueryV1 {
    /// Stable package identity.
    pub package: MusubiPackageIdV1,
    /// Optional `SemVer` filtering requirement.
    pub requirement: Option<MusubiVersionReqV1>,
    /// Page controls and finalized cursor.
    pub page: MusubiPageRequestV1,
}

impl MusubiResolverIndexQueryV1 {
    /// Validate structural package, optional requirement, and page controls.
    ///
    /// # Errors
    ///
    /// Returns an error if the package, optional version requirement, or page controls are invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        self.requirement
            .as_ref()
            .map_or(Ok(()), MusubiVersionReqV1::validate)?;
        self.page.validate()
    }
}

/// Package-scoped versions/members query.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiPackagePageQueryV1 {
    /// Stable package identity.
    pub package: MusubiPackageIdV1,
    /// Page controls.
    pub page: MusubiPageRequestV1,
}

impl MusubiPackagePageQueryV1 {
    /// Validate structural package identity and page controls.
    ///
    /// # Errors
    ///
    /// Returns an error if the package identity or page controls are invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        self.page.validate()
    }
}

/// Archive-location query.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArchiveLocationQueryV1 {
    /// Archive identity.
    pub archive_id: ArchiveId,
    /// Page controls.
    pub page: MusubiPageRequestV1,
}

/// Exact alias lookup or history query.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiAliasQueryV1 {
    /// Permanent alias.
    pub alias: MusubiAliasNameV1,
    /// Page controls used by history; ignored by exact lookup.
    pub page: MusubiPageRequestV1,
}

impl MusubiAliasQueryV1 {
    /// Validate permanent alias identity and page controls.
    ///
    /// # Errors
    ///
    /// Returns an error if the alias or page controls are invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.alias.validate()?;
        self.page.validate()
    }
}

/// Ordered-prefix registry query.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiOrderedPrefixQueryV1 {
    /// Canonical structural index prefix.
    pub prefix: MusubiOrderedPrefixV1,
    /// Page controls.
    pub page: MusubiPageRequestV1,
}

impl MusubiOrderedPrefixQueryV1 {
    /// Validate canonical structural prefix and page controls.
    ///
    /// # Errors
    ///
    /// Returns an error if the ordered prefix or page controls are invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.prefix.validate()?;
        self.page.validate()
    }
}

/// Snapshot of the process-local finalized-event package-search projection.
///
/// This anchor is deliberately distinct from [`MusubiRegistrySnapshotV1`]. Search
/// projection revisions are not resolver-index revisions and must never be used to
/// select a dependency release.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSearchSnapshotV1 {
    /// Finalized height through which the search projection has been applied.
    pub finalized_height: u64,
    /// Finalized block hash at `finalized_height`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub finalized_block_hash: [u8; 32],
    /// Process-local projection revision, changed on every visible rebuild/update.
    pub projection_revision: u64,
}

impl MusubiSearchSnapshotV1 {
    /// Validate a non-inert finalized search anchor.
    ///
    /// # Errors
    ///
    /// Returns an error if the finalized height, block hash, or projection revision is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.finalized_height == 0
            || digest_is_zero(&self.finalized_block_hash)
            || self.projection_revision == 0
        {
            return Err(ParseError::new("Musubi search snapshot is invalid"));
        }
        Ok(())
    }
}

/// Continuation cursor for the rebuildable package-search projection.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSearchCursorV1 {
    /// Exact finalized search projection used by the preceding page.
    pub snapshot: MusubiSearchSnapshotV1,
    /// Domain-separated hash of canonical search parameters excluding this cursor.
    pub query_hash: MusubiQueryHashV1,
    /// Last structural package returned by the preceding page.
    pub last_package: MusubiPackageIdV1,
}

impl MusubiSearchCursorV1 {
    /// Validate every cursor binding.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot or last package is invalid, or the query hash is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.snapshot.validate()?;
        self.last_package.validate()?;
        if self.query_hash.is_zero() {
            return Err(ParseError::new(
                "Musubi search cursor query hash is invalid",
            ));
        }
        Ok(())
    }
}

/// Page controls for rich package discovery.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSearchPageRequestV1 {
    /// Requested count; zero selects [`MUSUBI_DEFAULT_PAGE_SIZE_V1`].
    pub limit: u32,
    /// Continuation cursor returned by the same normalized search.
    pub cursor: Option<MusubiSearchCursorV1>,
}

impl MusubiSearchPageRequestV1 {
    /// Effective page size capped by the public V1 maximum.
    #[must_use]
    pub fn effective_limit(&self) -> usize {
        let requested = if self.limit == 0 {
            MUSUBI_DEFAULT_PAGE_SIZE_V1
        } else {
            self.limit
        };
        usize::try_from(requested)
            .unwrap_or(usize::MAX)
            .min(MUSUBI_MAX_PAGE_SIZE_V1)
    }

    /// Validate the page bound and continuation cursor.
    ///
    /// # Errors
    ///
    /// Returns an error if a nonzero limit exceeds the V1 page maximum or the cursor is invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.limit != 0
            && usize::try_from(self.limit).map_or(true, |limit| limit > MUSUBI_MAX_PAGE_SIZE_V1)
        {
            return Err(ParseError::new(
                "Musubi search page limit exceeds the public V1 maximum",
            ));
        }
        self.cursor
            .as_ref()
            .map_or(Ok(()), MusubiSearchCursorV1::validate)
    }
}

/// Bounded exact-token query for the rebuildable package discovery projection.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSearchQueryV1 {
    /// Description, keyword, namespace, or package-name terms joined by whitespace.
    pub query: String,
    /// Search-specific page controls and cursor.
    pub page: MusubiSearchPageRequestV1,
}

impl MusubiSearchQueryV1 {
    /// Return sorted, distinct, Unicode-lowercased exact search terms.
    ///
    /// Hyphenated ASCII components contribute both their complete spelling and
    /// their alphanumeric words. No prefix, edit-distance, or fuzzy expansion is
    /// performed.
    ///
    /// # Errors
    ///
    /// Returns an error if the query is empty, noncanonical, or overlong, or if normalization
    /// yields no terms, an overlong term, or more terms than the V1 bound.
    pub fn normalized_terms(&self) -> Result<Vec<String>, ParseError> {
        if self.query.is_empty()
            || self.query.len() > MUSUBI_MAX_SEARCH_QUERY_BYTES_V1
            || self.query.trim() != self.query
            || self.query.chars().any(char::is_control)
        {
            return Err(ParseError::new(
                "Musubi search query is empty, noncanonical, or exceeds its byte bound",
            ));
        }
        let mut terms = BTreeSet::new();
        for component in self.query.split_whitespace() {
            if component.len() <= MUSUBI_MAX_SEARCH_TERM_BYTES_V1
                && component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            {
                terms.insert(component.to_ascii_lowercase());
            }
            for word in component.split(|character: char| !character.is_alphanumeric()) {
                if word.is_empty() {
                    continue;
                }
                let normalized = word
                    .chars()
                    .flat_map(char::to_lowercase)
                    .collect::<String>();
                if normalized.len() > MUSUBI_MAX_SEARCH_TERM_BYTES_V1 {
                    return Err(ParseError::new(
                        "Musubi search term exceeds its UTF-8 byte bound",
                    ));
                }
                terms.insert(normalized);
                if terms.len() > MUSUBI_MAX_SEARCH_QUERY_TERMS_V1 {
                    return Err(ParseError::new(
                        "Musubi search query exceeds its normalized term bound",
                    ));
                }
            }
        }
        if terms.is_empty() || terms.len() > MUSUBI_MAX_SEARCH_QUERY_TERMS_V1 {
            return Err(ParseError::new(
                "Musubi search query has no bounded normalized terms",
            ));
        }
        Ok(terms.into_iter().collect())
    }

    /// Validate query normalization and page controls.
    ///
    /// # Errors
    ///
    /// Returns an error if term normalization or page-control validation fails.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.normalized_terms()?;
        self.page.validate()
    }
}

/// One deterministic rich package-discovery result.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSearchHitV1 {
    /// Stable structural package identity.
    pub package: MusubiPackageIdV1,
    /// Immutable namespace used for the first package claim.
    pub claimed_namespace: MusubiNamespaceV1,
    /// Current mutable package description.
    pub description: Option<MusubiDescriptionV1>,
    /// Current sorted package keywords.
    pub keywords: Vec<MusubiKeywordV1>,
    /// Current mutable-metadata revision.
    pub metadata_revision: u64,
}

impl MusubiSearchHitV1 {
    /// Validate structural identity, namespace scope, metadata, and revision.
    ///
    /// # Errors
    ///
    /// Returns an error if package, namespace, or metadata is invalid, the namespace scope does
    /// not match the package, or the metadata revision is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        self.claimed_namespace.validate()?;
        let namespace_scope_matches =
            match (&self.package.scope, self.claimed_namespace.domain_segment()) {
                (MusubiPackageScopeV1::DataspaceRoot, None) => true,
                (MusubiPackageScopeV1::Domain(domain), Some(text)) => domain.as_ref() == text,
                _ => false,
            };
        let metadata = MusubiReleaseMetadataV1 {
            description: self.description.clone(),
            keywords: self.keywords.clone(),
            ..MusubiReleaseMetadataV1::default()
        };
        metadata.validate()?;
        if !namespace_scope_matches || self.metadata_revision == 0 {
            return Err(ParseError::new("Musubi search hit is invalid"));
        }
        Ok(())
    }
}

/// One deterministic page from the rebuildable package discovery projection.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSearchPageV1 {
    /// Exact bounded request whose discovery results this page carries.
    pub query: MusubiSearchQueryV1,
    /// Results ordered by structural package identity.
    pub items: Vec<MusubiSearchHitV1>,
    /// Continuation cursor, absent at the end of the result set.
    pub next_cursor: Option<MusubiSearchCursorV1>,
    /// Finalized search projection shared by every result.
    pub snapshot: MusubiSearchSnapshotV1,
}

impl MusubiSearchPageV1 {
    /// Validate request identity, page bounds, strict ordering, and cursor binding.
    ///
    /// # Errors
    ///
    /// Returns an error if query, snapshot, hit, or cursor data is invalid, hits are oversized or
    /// noncanonical, or a request/response cursor does not bind the page exactly.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.query.validate()?;
        self.snapshot.validate()?;
        if self.items.len() > self.query.page.effective_limit()
            || self
                .items
                .windows(2)
                .any(|pair| pair[0].package >= pair[1].package)
        {
            return Err(ParseError::new(
                "Musubi search page exceeds its item bound or is not strictly ordered",
            ));
        }
        self.items
            .iter()
            .try_for_each(MusubiSearchHitV1::validate)?;
        if let Some(cursor) = &self.query.page.cursor
            && (cursor.snapshot != self.snapshot
                || self
                    .items
                    .first()
                    .is_some_and(|item| item.package <= cursor.last_package))
        {
            return Err(ParseError::new(
                "Musubi search page does not continue its request cursor",
            ));
        }
        if let Some(cursor) = &self.next_cursor {
            cursor.validate()?;
            if cursor.snapshot != self.snapshot
                || self.items.last().map(|hit| &hit.package) != Some(&cursor.last_package)
                || self.items.len() != self.query.page.effective_limit()
                || self
                    .query
                    .page
                    .cursor
                    .as_ref()
                    .is_some_and(|previous| previous.query_hash != cursor.query_hash)
            {
                return Err(ParseError::new(
                    "Musubi search page cursor does not bind its final result",
                ));
            }
        }
        Ok(())
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    ///
    /// # Errors
    ///
    /// Returns an error if the page is invalid or its embedded query differs from `query`.
    pub fn validate_for(&self, query: &MusubiSearchQueryV1) -> Result<(), ParseError> {
        self.validate()?;
        if &self.query != query {
            return Err(ParseError::new(
                "Musubi search page carries a different request context",
            ));
        }
        Ok(())
    }
}
