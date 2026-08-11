impl ValidSingularQuery for FindMusubiExactPackageV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiPackageRecordV1, QueryExecutionFail> {
        self.request.package.validate().map_err(query_invalid)?;
        state_ro
            .world()
            .musubi_packages()
            .get(&self.request.package)
            .ok_or(QueryExecutionFail::NotFound)
            .and_then(crate::smartcontracts::isi::query::own_singular_query_value)
    }
}

impl ValidSingularQuery for FindMusubiExactReleaseV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiExactReleaseSnapshotV1, QueryExecutionFail> {
        self.request.release.validate().map_err(query_invalid)?;
        let snapshot = query_snapshot(state_ro)?;
        let network_id = *state_ro.network_id();
        let world = state_ro.world();
        let home_release = world.musubi_releases().get(&self.request.release);
        let universal_release = world.musubi_resolver_index().get(&self.request.release);
        let (home_release, universal_release) = match (home_release, universal_release) {
            (None, None) => return Err(QueryExecutionFail::NotFound),
            (Some(home_release), Some(universal_release)) => (home_release, universal_release),
            (Some(_), None) | (None, Some(_)) => {
                return Err(QueryExecutionFail::Conversion(
                    "Musubi exact release home and universal projections are inconsistent"
                        .to_owned(),
                ));
            }
        };
        let response = crate::smartcontracts::isi::query::own_singular_query_struct::<
            MusubiExactReleaseSnapshotV1,
            4,
        >(
            [&network_id, &snapshot, home_release, universal_release],
            || MusubiExactReleaseSnapshotV1 {
                network_id,
                snapshot,
                home_release: home_release.clone(),
                universal_release: universal_release.clone(),
            },
        )?;
        response
            .validate_for(&self.request)
            .map_err(query_invalid)?;
        Ok(response)
    }
}

impl ValidSingularQuery for FindMusubiProviderBundleAttestationV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiProviderBundleAttestationRecordV1, QueryExecutionFail> {
        self.key.validate().map_err(query_invalid)?;
        let record = state_ro
            .world()
            .musubi_provider_bundle_attestations()
            .get(&self.key)
            .ok_or(QueryExecutionFail::NotFound)?;
        record.validate().map_err(query_invalid)?;
        record
            .attestation
            .verify(&record.attestation.payload.binding)
            .map_err(query_invalid)?;
        if record.key != self.key {
            return Err(QueryExecutionFail::Conversion(
                "Musubi provider attestation record has the wrong embedded identity".to_owned(),
            ));
        }
        crate::smartcontracts::isi::query::own_singular_query_value(record)
    }
}

/// Internal typed Musubi query failure retained through the Torii telemetry boundary.
///
/// The public query error remains [`QueryExecutionFail`]. This wrapper carries
/// a Musubi-only cursor reason in-process without changing the global query
/// wire enum or its variant indices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusubiQueryExecutionErrorV1 {
    query_error: QueryExecutionFail,
    cursor_failure: Option<MusubiCursorFailureV1>,
}

impl MusubiQueryExecutionErrorV1 {
    fn cursor(reason: MusubiCursorFailureV1) -> Self {
        Self {
            query_error: QueryExecutionFail::Expired,
            cursor_failure: Some(reason),
        }
    }

    /// Return the exact typed cursor failure, when this is a cursor error.
    #[must_use]
    pub const fn cursor_failure(&self) -> Option<MusubiCursorFailureV1> {
        self.cursor_failure
    }

    /// Drop the process-local telemetry detail and recover the stable public query error.
    #[must_use]
    pub fn into_query_error(self) -> QueryExecutionFail {
        self.query_error
    }
}

impl From<QueryExecutionFail> for MusubiQueryExecutionErrorV1 {
    fn from(query_error: QueryExecutionFail) -> Self {
        Self {
            query_error,
            cursor_failure: None,
        }
    }
}

/// Execute a paged Musubi query while retaining its exact cursor-failure reason.
///
/// Ordinary Core callers continue to use [`ValidSingularQuery`]. Torii uses
/// this trait only to observe the bounded reason before returning the same
/// public [`QueryExecutionFail`] value.
pub trait ValidMusubiSingularQuery: iroha_data_model::query::SingularQuery {
    /// Execute against one read-only state view with typed cursor diagnostics.
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<Self::Output, MusubiQueryExecutionErrorV1>;
}

macro_rules! impl_valid_singular_query_via_musubi {
    ($query:ty, $output:ty) => {
        impl ValidSingularQuery for $query {
            fn execute(
                &self,
                state_ro: &impl StateReadOnly,
            ) -> Result<$output, QueryExecutionFail> {
                self.execute_musubi(state_ro)
                    .map_err(MusubiQueryExecutionErrorV1::into_query_error)
            }
        }
    };
}

impl ValidMusubiSingularQuery for FindMusubiResolverIndexV1 {
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiResolverIndexPageV1, MusubiQueryExecutionErrorV1> {
        self.request.package.validate().map_err(query_invalid)?;
        if let Some(requirement) = &self.request.requirement {
            requirement.validate().map_err(query_invalid)?;
        }
        let snapshot = query_snapshot(state_ro)?;
        let network_id = *state_ro.network_id();
        let query_hash = resolver_query_hash(&self.request);
        let start = package_release_page_start(&self.request.package, &self.request.page)?;
        let rows = state_ro
            .world()
            .musubi_resolver_index()
            .range(start..)
            .take_while(|(release, _)| release.package == self.request.package)
            .filter(|(release, _)| {
                self.request
                    .requirement
                    .as_ref()
                    .is_none_or(|requirement| requirement.matches(&release.version))
            })
            .map(|(release, row)| {
                Ok((
                    release.version.to_string(),
                    crate::smartcontracts::isi::query::own_singular_query_value(row)?,
                ))
            });
        let (items, next_cursor) = paginate_fallible_with_json_items_budget(
            rows,
            &self.request.page,
            query_hash,
            snapshot,
            MUSUBI_RESOLVER_PAGE_JSON_ITEMS_BUDGET_BYTES_V1,
        )?;
        let page = MusubiResolverIndexPageV1 {
            query: self.request.clone(),
            network_id,
            items,
            next_cursor,
            snapshot,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}
impl_valid_singular_query_via_musubi!(FindMusubiResolverIndexV1, MusubiResolverIndexPageV1);

impl ValidMusubiSingularQuery for FindMusubiVersionsV1 {
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiVersionPageV1, MusubiQueryExecutionErrorV1> {
        self.request.package.validate().map_err(query_invalid)?;
        let snapshot = query_snapshot(state_ro)?;
        let query_hash = package_page_query_hash(b"versions", &self.request);
        let start = package_release_page_start(&self.request.package, &self.request.page)?;
        let rows = state_ro
            .world()
            .musubi_resolver_index()
            .range(start..)
            .take_while(|(release, _)| release.package == self.request.package)
            .map(|(release, _)| {
                Ok((
                    release.version.to_string(),
                    crate::smartcontracts::isi::query::own_singular_query_value(&release.version)?,
                ))
            });
        let (items, next_cursor) =
            paginate_fallible(rows, &self.request.page, query_hash, snapshot)?;
        let page = MusubiVersionPageV1 {
            query: self.request.clone(),
            items,
            next_cursor,
            snapshot,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}
impl_valid_singular_query_via_musubi!(FindMusubiVersionsV1, MusubiVersionPageV1);

impl ValidMusubiSingularQuery for FindMusubiMaintainersV1 {
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiMaintainerPageV1, MusubiQueryExecutionErrorV1> {
        self.request.package.validate().map_err(query_invalid)?;
        let snapshot = query_snapshot(state_ro)?;
        let query_hash = package_page_query_hash(b"maintainers", &self.request);
        state_ro
            .world()
            .musubi_packages()
            .get(&self.request.package)
            .ok_or(QueryExecutionFail::NotFound)?;
        let start = MusubiMaintainerDirectoryKeyV1::package_start(self.request.package.clone());
        let rows = state_ro
            .world()
            .musubi_maintainer_directory()
            .range(start..)
            .take_while(|(key, _)| key.package == self.request.package)
            .filter(|(_, entry)| {
                maintainer_directory_entry_visible_at_height(entry, snapshot.finalized_height)
            })
            .map(|(_, entry)| {
                Ok((
                    entry.cursor_key(),
                    crate::smartcontracts::isi::query::own_singular_query_value(entry)?,
                ))
            });
        let (items, next_cursor) =
            paginate_fallible(rows, &self.request.page, query_hash, snapshot)?;
        let page = MusubiMaintainerPageV1 {
            query: self.request.clone(),
            items,
            next_cursor,
            snapshot,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}
impl_valid_singular_query_via_musubi!(FindMusubiMaintainersV1, MusubiMaintainerPageV1);

impl ValidMusubiSingularQuery for FindMusubiArchiveLocationsV1 {
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiArchiveLocationPageV1, MusubiQueryExecutionErrorV1> {
        if self.request.archive_id.is_zero() {
            return Err(QueryExecutionFail::Conversion(
                "Musubi archive id must not be zero".to_owned(),
            )
            .into());
        }
        let snapshot = query_snapshot(state_ro)?;
        let network_id = *state_ro.network_id();
        let query_hash = archive_location_query_hash(&self.request);
        let archive = state_ro
            .world()
            .musubi_archives()
            .get(&self.request.archive_id)
            .ok_or(QueryExecutionFail::NotFound)?;
        let rows = archive
            .location_ids
            .iter()
            .map(|location_id| {
                let key = MusubiArchiveLocationKeyV1::new(self.request.archive_id, *location_id);
                let location = state_ro
                    .world()
                    .musubi_archive_locations()
                    .get(&key)
                    .ok_or_else(|| {
                        QueryExecutionFail::Conversion(
                            "Musubi archive location directory is inconsistent".to_owned(),
                        )
                    })?;
                Ok((
                    format!(
                        "{}:{}",
                        digest_label(key.archive_id.as_bytes()),
                        digest_label(key.location_id.as_bytes())
                    ),
                    crate::smartcontracts::isi::query::own_singular_query_value(location)?,
                ))
            });
        let (items, next_cursor) =
            paginate_fallible(rows, &self.request.page, query_hash, snapshot)?;
        let archive = crate::smartcontracts::isi::query::own_singular_query_value(archive)?;
        let page = MusubiArchiveLocationPageV1 {
            network_id,
            archive,
            items,
            next_cursor,
            snapshot,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}
impl_valid_singular_query_via_musubi!(FindMusubiArchiveLocationsV1, MusubiArchiveLocationPageV1);

impl ValidSingularQuery for FindMusubiArchiveRetentionV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiArchiveRetentionPageV1, QueryExecutionFail> {
        self.request.validate().map_err(query_invalid)?;
        let snapshot = query_snapshot(state_ro)?;
        if self
            .request
            .expected_snapshot
            .is_some_and(|expected| expected != snapshot)
        {
            return Err(QueryExecutionFail::Expired);
        }
        let network_id = *state_ro.network_id();
        let world = state_ro.world();
        let mut items = crate::smartcontracts::isi::query::singular_query_vec_with_capacity(
            self.request.archive_ids.len(),
        )?;
        for archive_id in &self.request.archive_ids {
            items.push(archive_retention_decision(*archive_id, world)?);
            crate::smartcontracts::isi::query::singular_query_ensure_value_fits(
                &items,
                usize::MAX,
            )?;
        }
        let finalized_block = state_ro.latest_block().ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "Musubi finalized snapshot block body is unavailable".to_owned(),
            )
        })?;
        let finalized_time_ms = validated_finalized_block_time(
            &snapshot,
            finalized_block.header().height().get(),
            *finalized_block.hash().as_ref(),
            finalized_block.header().creation_time_ms,
        )?;
        let page = MusubiArchiveRetentionPageV1 {
            network_id,
            items,
            snapshot,
            finalized_time_ms,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}

fn validated_finalized_block_time(
    snapshot: &MusubiRegistrySnapshotV1,
    block_height: u64,
    block_hash: [u8; 32],
    creation_time_ms: u64,
) -> Result<u64, QueryExecutionFail> {
    if block_height != snapshot.finalized_height || block_hash != snapshot.finalized_block_hash {
        return Err(QueryExecutionFail::Conversion(
            "Musubi finalized snapshot does not match its block body".to_owned(),
        ));
    }
    Ok(creation_time_ms)
}

fn archive_retention_decision(
    archive_id: ArchiveId,
    world: &impl WorldReadOnly,
) -> Result<MusubiArchiveRetentionDecisionV1, QueryExecutionFail> {
    let archive = world.musubi_archives().get(&archive_id);
    let references = world.musubi_archive_reverse_references().get(&archive_id);
    let storage = world.musubi_archive_availability().get(&archive_id);
    let Some(archive) = archive else {
        if references.is_some() || storage.is_some() {
            return Err(QueryExecutionFail::Conversion(
                "Musubi archive retention state has an orphan universal projection".to_owned(),
            ));
        }
        return Ok(MusubiArchiveRetentionDecisionV1 {
            archive_id,
            disposition: MusubiArchiveRetentionDispositionV1::RetainUnknown,
            active_releases: 0,
            yanked_releases: 0,
            taken_down_releases: 0,
            storage: None,
        });
    };
    archive.validate().map_err(query_invalid)?;
    if archive.archive_id != archive_id {
        return Err(QueryExecutionFail::Conversion(
            "Musubi archive retention storage key disagrees with its archive record".to_owned(),
        ));
    }
    let references = references.ok_or_else(|| {
        QueryExecutionFail::Conversion(
            "Musubi archive retention state is missing reverse references".to_owned(),
        )
    })?;
    references.validate().map_err(query_invalid)?;
    if references.archive_id != archive_id {
        return Err(QueryExecutionFail::Conversion(
            "Musubi archive retention reverse-reference identity is inconsistent".to_owned(),
        ));
    }
    let storage = storage.ok_or_else(|| {
        QueryExecutionFail::Conversion(
            "Musubi archive retention state is missing storage availability".to_owned(),
        )
    })?;
    storage.validate().map_err(query_invalid)?;
    if storage.archive_id != archive_id {
        return Err(QueryExecutionFail::Conversion(
            "Musubi archive retention storage identity is inconsistent".to_owned(),
        ));
    }

    let mut active_releases = 0_u16;
    let mut yanked_releases = 0_u16;
    let mut taken_down_releases = 0_u16;
    for release_id in &references.releases {
        let release = world.musubi_releases().get(release_id).ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "Musubi archive retention reference names a missing release".to_owned(),
            )
        })?;
        release.validate().map_err(query_invalid)?;
        if release.manifest.release != *release_id || release.manifest.archive_id != archive_id {
            return Err(QueryExecutionFail::Conversion(
                "Musubi archive retention reference disagrees with its release".to_owned(),
            ));
        }
        match &release.artifact_governance {
            MusubiArtifactGovernanceStateV1::Available if release.yank.yanked => {
                yanked_releases = yanked_releases.checked_add(1).ok_or_else(|| {
                    QueryExecutionFail::Conversion(
                        "Musubi archive retention yanked-release count overflow".to_owned(),
                    )
                })?;
            }
            MusubiArtifactGovernanceStateV1::Available => {
                active_releases = active_releases.checked_add(1).ok_or_else(|| {
                    QueryExecutionFail::Conversion(
                        "Musubi archive retention active-release count overflow".to_owned(),
                    )
                })?;
            }
            MusubiArtifactGovernanceStateV1::TakenDown(_) => {
                taken_down_releases = taken_down_releases.checked_add(1).ok_or_else(|| {
                    QueryExecutionFail::Conversion(
                        "Musubi archive retention takedown count overflow".to_owned(),
                    )
                })?;
            }
        }
    }
    let disposition = if active_releases > 0 || yanked_releases > 0 {
        MusubiArchiveRetentionDispositionV1::RetainReferenced
    } else if taken_down_releases > 0 {
        MusubiArchiveRetentionDispositionV1::PruneGovernedTakedown
    } else {
        MusubiArchiveRetentionDispositionV1::PruneUnreferenced
    };
    let borrowed_storage =
        crate::smartcontracts::isi::query::BorrowedSingularOption::new(Some(storage));
    let decision = crate::smartcontracts::isi::query::own_singular_query_struct::<
        MusubiArchiveRetentionDecisionV1,
        6,
    >(
        [
            &archive_id,
            &disposition,
            &active_releases,
            &yanked_releases,
            &taken_down_releases,
            &borrowed_storage,
        ],
        || MusubiArchiveRetentionDecisionV1 {
            archive_id,
            disposition,
            active_releases,
            yanked_releases,
            taken_down_releases,
            storage: Some(storage.clone()),
        },
    )?;
    decision.validate().map_err(query_invalid)?;
    Ok(decision)
}

impl ValidSingularQuery for FindMusubiAliasV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiAliasRecordV1, QueryExecutionFail> {
        self.request.alias.validate().map_err(query_invalid)?;
        state_ro
            .world()
            .musubi_aliases()
            .get(&self.request.alias)
            .ok_or(QueryExecutionFail::NotFound)
            .and_then(crate::smartcontracts::isi::query::own_singular_query_value)
    }
}

impl ValidMusubiSingularQuery for FindMusubiAliasHistoryV1 {
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiAliasHistoryPageV1, MusubiQueryExecutionErrorV1> {
        self.request.alias.validate().map_err(query_invalid)?;
        let snapshot = query_snapshot(state_ro)?;
        let query_hash = alias_history_query_hash(&self.request);
        let start = alias_history_page_start(&self.request)?;
        let rows = state_ro
            .world()
            .musubi_alias_history()
            .range(start..)
            .take_while(|(key, _)| key.alias == self.request.alias)
            .map(|(key, history)| {
                Ok((
                    format!("{}:{:020}", key.alias, key.revision),
                    crate::smartcontracts::isi::query::own_singular_query_value(history)?,
                ))
            });
        let (items, next_cursor) =
            paginate_fallible(rows, &self.request.page, query_hash, snapshot)?;
        let page = MusubiAliasHistoryPageV1 {
            query: self.request.clone(),
            items,
            next_cursor,
            snapshot,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}
impl_valid_singular_query_via_musubi!(FindMusubiAliasHistoryV1, MusubiAliasHistoryPageV1);

impl ValidMusubiSingularQuery for FindMusubiOrderedPrefixV1 {
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiOrderedPackagePageV1, MusubiQueryExecutionErrorV1> {
        self.request.prefix.validate().map_err(query_invalid)?;
        let snapshot = query_snapshot(state_ro)?;
        let network_id = *state_ro.network_id();
        let query_hash = ordered_prefix_query_hash(&self.request);
        let prefix = self.request.prefix.as_str();
        let (start, namespace, name_prefix) = directory_query_start(&self.request)?;
        let namespace_binding = state_ro
            .world()
            .musubi_namespace_bindings()
            .get(&namespace)
            .ok_or(QueryExecutionFail::NotFound)?;
        let rows = state_ro
            .world()
            .musubi_public_directory()
            .range(start..)
            .take_while(|(selector, _)| {
                selector.namespace == namespace
                    && selector.name.as_str().starts_with(name_prefix.as_str())
            })
            .filter(|(selector, _)| selector.to_string().starts_with(prefix))
            .map(|(selector, entry)| {
                Ok((
                    selector.to_string(),
                    crate::smartcontracts::isi::query::own_singular_query_value(entry)?,
                ))
            });
        let (items, next_cursor) =
            paginate_fallible(rows, &self.request.page, query_hash, snapshot)?;
        let namespace_binding =
            crate::smartcontracts::isi::query::own_singular_query_value(namespace_binding)?;
        let page = MusubiOrderedPackagePageV1 {
            query: self.request.clone(),
            network_id,
            namespace_binding,
            items,
            next_cursor,
            snapshot,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}
impl_valid_singular_query_via_musubi!(FindMusubiOrderedPrefixV1, MusubiOrderedPackagePageV1);

fn query_snapshot(
    state_ro: &impl StateReadOnly,
) -> Result<MusubiRegistrySnapshotV1, QueryExecutionFail> {
    let finalized_height = u64::try_from(state_ro.block_hashes().len()).map_err(|_| {
        QueryExecutionFail::Conversion("Musubi finalized height overflows u64".to_owned())
    })?;
    let finalized_block_hash = state_ro
        .block_hashes()
        .last()
        .map(|hash| *hash.as_ref())
        .ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "Musubi queries require at least one finalized block".to_owned(),
            )
        })?;
    let snapshot = MusubiRegistrySnapshotV1 {
        finalized_height,
        finalized_block_hash,
        index_revision: state_ro.world().musubi_resolver_index_revision(),
    };
    snapshot.validate().map_err(query_invalid)?;
    Ok(snapshot)
}

fn resolver_query_hash(request: &MusubiResolverIndexQueryV1) -> MusubiQueryHashV1 {
    let mut canonical = request.clone();
    canonical.page.cursor = None;
    query_hash(b"resolver-index", &canonical.encode())
}

fn package_page_query_hash(kind: &[u8], request: &MusubiPackagePageQueryV1) -> MusubiQueryHashV1 {
    let mut canonical = request.clone();
    canonical.page.cursor = None;
    query_hash(kind, &canonical.encode())
}

fn archive_location_query_hash(request: &MusubiArchiveLocationQueryV1) -> MusubiQueryHashV1 {
    let mut canonical = request.clone();
    canonical.page.cursor = None;
    query_hash(b"archive-locations", &canonical.encode())
}

fn alias_history_query_hash(request: &MusubiAliasQueryV1) -> MusubiQueryHashV1 {
    let mut canonical = request.clone();
    canonical.page.cursor = None;
    query_hash(b"alias-history", &canonical.encode())
}

fn alias_history_page_start(
    request: &MusubiAliasQueryV1,
) -> Result<MusubiAliasHistoryKeyV1, MusubiQueryExecutionErrorV1> {
    let revision = if let Some(cursor) = &request.page.cursor {
        let (alias, revision) = cursor.last_key.rsplit_once(':').ok_or_else(|| {
            MusubiQueryExecutionErrorV1::cursor(MusubiCursorFailureV1::LastKeyStale)
        })?;
        if alias != request.alias.as_str() || revision.len() != 20 {
            return Err(MusubiQueryExecutionErrorV1::cursor(
                MusubiCursorFailureV1::LastKeyStale,
            ));
        }
        revision
            .parse::<u64>()
            .map_err(|_| MusubiQueryExecutionErrorV1::cursor(MusubiCursorFailureV1::LastKeyStale))?
    } else {
        0
    };
    Ok(MusubiAliasHistoryKeyV1::new(
        request.alias.clone(),
        revision,
    ))
}

fn ordered_prefix_query_hash(request: &MusubiOrderedPrefixQueryV1) -> MusubiQueryHashV1 {
    let mut canonical = request.clone();
    canonical.page.cursor = None;
    query_hash(b"ordered-prefix", &canonical.encode())
}

fn directory_query_start(
    request: &MusubiOrderedPrefixQueryV1,
) -> Result<(MusubiPackageSelectorV1, MusubiNamespaceV1, String), MusubiQueryExecutionErrorV1> {
    let raw = request.prefix.as_str();
    let (namespace_raw, name_prefix) = raw.split_once('/').ok_or_else(|| {
        QueryExecutionFail::Conversion(
            "Musubi ordered directory prefix must use namespace/package-prefix".to_owned(),
        )
    })?;
    if name_prefix.contains('/')
        || name_prefix.len() > MUSUBI_MAX_PACKAGE_NAME_BYTES_V1
        || name_prefix.starts_with('-')
        || name_prefix.contains("--")
        || !name_prefix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(QueryExecutionFail::Conversion(
            "Musubi ordered directory package prefix is not portable canonical text".to_owned(),
        )
        .into());
    }
    let namespace = namespace_raw
        .parse::<MusubiNamespaceV1>()
        .map_err(query_invalid)?;
    let start = if let Some(cursor) = &request.page.cursor {
        let selector = cursor
            .last_key
            .parse::<MusubiPackageSelectorV1>()
            .map_err(|_| {
                MusubiQueryExecutionErrorV1::cursor(MusubiCursorFailureV1::LastKeyStale)
            })?;
        if selector.namespace != namespace || !selector.name.as_str().starts_with(name_prefix) {
            return Err(MusubiQueryExecutionErrorV1::cursor(
                MusubiCursorFailureV1::LastKeyStale,
            ));
        }
        selector
    } else {
        let lower_name = if name_prefix.is_empty() {
            "0".to_owned()
        } else if name_prefix.ends_with('-') {
            if name_prefix.len() == MUSUBI_MAX_PACKAGE_NAME_BYTES_V1 {
                return Err(QueryExecutionFail::Conversion(
                    "Musubi ordered directory package prefix cannot match a package".to_owned(),
                )
                .into());
            }
            format!("{name_prefix}0")
        } else {
            name_prefix.to_owned()
        };
        MusubiPackageSelectorV1 {
            namespace: namespace.clone(),
            name: lower_name.parse().map_err(query_invalid)?,
        }
    };
    Ok((start, namespace, name_prefix.to_owned()))
}

fn query_hash(domain: &[u8], encoded: &[u8]) -> MusubiQueryHashV1 {
    let mut payload = Vec::with_capacity(domain.len() + encoded.len() + 16);
    payload.extend_from_slice(
        &u64::try_from(domain.len())
            .expect("static Musubi query domain length fits u64")
            .to_le_bytes(),
    );
    payload.extend_from_slice(domain);
    payload.extend_from_slice(
        &u64::try_from(encoded.len())
            .expect("bounded Musubi query length fits u64")
            .to_le_bytes(),
    );
    payload.extend_from_slice(encoded);
    MusubiQueryHashV1::new(*Hash::new(&payload).as_ref())
}

fn paginate<T: norito::json::JsonSerialize>(
    rows: impl IntoIterator<Item = (String, T)>,
    page: &MusubiPageRequestV1,
    query_hash: MusubiQueryHashV1,
    snapshot: MusubiRegistrySnapshotV1,
) -> Result<(Vec<T>, Option<MusubiFinalizedCursorV1>), MusubiQueryExecutionErrorV1> {
    paginate_fallible(
        rows.into_iter().map(Ok),
        page,
        query_hash,
        snapshot,
    )
}

fn paginate_fallible<T: norito::json::JsonSerialize>(
    rows: impl IntoIterator<Item = Result<(String, T), QueryExecutionFail>>,
    page: &MusubiPageRequestV1,
    query_hash: MusubiQueryHashV1,
    snapshot: MusubiRegistrySnapshotV1,
) -> Result<(Vec<T>, Option<MusubiFinalizedCursorV1>), MusubiQueryExecutionErrorV1> {
    paginate_for_caller_with_json_items_budget(rows, page, query_hash, snapshot, None, None)
}

fn paginate_with_json_items_budget<T: norito::json::JsonSerialize>(
    rows: impl IntoIterator<Item = (String, T)>,
    page: &MusubiPageRequestV1,
    query_hash: MusubiQueryHashV1,
    snapshot: MusubiRegistrySnapshotV1,
    json_items_budget: usize,
) -> Result<(Vec<T>, Option<MusubiFinalizedCursorV1>), MusubiQueryExecutionErrorV1> {
    paginate_for_caller_with_json_items_budget(
        rows.into_iter().map(Ok),
        page,
        query_hash,
        snapshot,
        None,
        Some(json_items_budget),
    )
}

fn paginate_fallible_with_json_items_budget<T: norito::json::JsonSerialize>(
    rows: impl IntoIterator<Item = Result<(String, T), QueryExecutionFail>>,
    page: &MusubiPageRequestV1,
    query_hash: MusubiQueryHashV1,
    snapshot: MusubiRegistrySnapshotV1,
    json_items_budget: usize,
) -> Result<(Vec<T>, Option<MusubiFinalizedCursorV1>), MusubiQueryExecutionErrorV1> {
    paginate_for_caller_with_json_items_budget(
        rows,
        page,
        query_hash,
        snapshot,
        None,
        Some(json_items_budget),
    )
}

#[cfg(test)]
fn paginate_for_caller<T: norito::json::JsonSerialize>(
    rows: impl IntoIterator<Item = (String, T)>,
    page: &MusubiPageRequestV1,
    query_hash: MusubiQueryHashV1,
    snapshot: MusubiRegistrySnapshotV1,
    expected_caller: Option<&AccountId>,
) -> Result<(Vec<T>, Option<MusubiFinalizedCursorV1>), MusubiQueryExecutionErrorV1> {
    paginate_for_caller_with_json_items_budget(
        rows.into_iter().map(Ok),
        page,
        query_hash,
        snapshot,
        expected_caller,
        None,
    )
}

fn paginate_for_caller_with_json_items_budget<T: norito::json::JsonSerialize>(
    rows: impl IntoIterator<Item = Result<(String, T), QueryExecutionFail>>,
    page: &MusubiPageRequestV1,
    query_hash: MusubiQueryHashV1,
    snapshot: MusubiRegistrySnapshotV1,
    expected_caller: Option<&AccountId>,
    json_items_budget: Option<usize>,
) -> Result<(Vec<T>, Option<MusubiFinalizedCursorV1>), MusubiQueryExecutionErrorV1> {
    page.validate().map_err(query_invalid)?;
    let cursor_last_key = if let Some(cursor) = &page.cursor {
        if cursor.snapshot.finalized_height != snapshot.finalized_height
            || cursor.snapshot.finalized_block_hash != snapshot.finalized_block_hash
        {
            return Err(MusubiQueryExecutionErrorV1::cursor(
                MusubiCursorFailureV1::FinalizedAnchorMismatch,
            ));
        }
        if cursor.snapshot.index_revision != snapshot.index_revision {
            return Err(MusubiQueryExecutionErrorV1::cursor(
                MusubiCursorFailureV1::IndexRevisionMismatch,
            ));
        }
        if cursor.query_hash != query_hash {
            return Err(MusubiQueryExecutionErrorV1::cursor(
                MusubiCursorFailureV1::QueryMismatch,
            ));
        }
        if cursor.caller.as_ref() != expected_caller {
            return Err(MusubiQueryExecutionErrorV1::cursor(
                MusubiCursorFailureV1::CallerMismatch,
            ));
        }
        Some(cursor.last_key.as_str())
    } else {
        None
    };
    let limit = page.effective_limit();
    let mut cursor_seen = cursor_last_key.is_none();
    let mut items = crate::smartcontracts::isi::query::singular_query_vec_with_capacity(limit)
        .map_err(MusubiQueryExecutionErrorV1::from)?;
    let mut last_key = None;
    let mut json_items_bytes = 0_usize;
    let mut budget_has_more = false;
    for row in rows {
        let (key, item) = row.map_err(MusubiQueryExecutionErrorV1::from)?;
        if !cursor_seen {
            if Some(key.as_str()) == cursor_last_key {
                cursor_seen = true;
            }
            continue;
        }
        if items.len() == limit {
            budget_has_more = true;
            break;
        }
        if let Some(json_items_budget) = json_items_budget {
            let dynamic_budget =
                crate::smartcontracts::isi::query::singular_query_frame_limit(json_items_budget);
            let encoded = norito::json::to_json_bounded(&item, dynamic_budget).map_err(|error| {
                match error {
                    norito::json::BoundedJsonError::BodyTooLarge
                    | norito::json::BoundedJsonError::AllocationFailed => {
                        MusubiQueryExecutionErrorV1::from(QueryExecutionFail::CapacityLimit)
                    }
                    norito::json::BoundedJsonError::Unsupported
                    | norito::json::BoundedJsonError::LengthMismatch => {
                        MusubiQueryExecutionErrorV1::from(query_invalid(
                            iroha_data_model::ParseError::new(
                                "Musubi resolver row cannot be encoded as canonical JSON",
                            ),
                        ))
                    }
                }
            })?;
            let separator_bytes = usize::from(!items.is_empty());
            let candidate_bytes = json_items_bytes
                .checked_add(separator_bytes)
                .and_then(|bytes| bytes.checked_add(encoded.len()))
                .ok_or_else(|| {
                    query_invalid(iroha_data_model::ParseError::new(
                        "Musubi resolver JSON item budget overflow",
                    ))
                })?;
            if candidate_bytes > json_items_budget {
                if items.is_empty() {
                    return Err(query_invalid(iroha_data_model::ParseError::new(
                        "one Musubi resolver row exceeds the JSON item budget",
                    ))
                    .into());
                }
                budget_has_more = true;
                break;
            }
            json_items_bytes = candidate_bytes;
        }
        last_key = Some(key);
        items.push(item);
        crate::smartcontracts::isi::query::singular_query_ensure_value_fits(
            &items,
            usize::MAX,
        )
        .map_err(MusubiQueryExecutionErrorV1::from)?;
    }
    if !cursor_seen {
        return Err(MusubiQueryExecutionErrorV1::cursor(
            MusubiCursorFailureV1::LastKeyStale,
        ));
    }
    let has_more = budget_has_more;
    let next_cursor = if has_more {
        Some(MusubiFinalizedCursorV1 {
            snapshot,
            query_hash,
            last_key: last_key.expect("a page with a successor has at least one item"),
            caller: expected_caller.cloned(),
        })
    } else {
        None
    };
    Ok((items, next_cursor))
}

fn query_invalid(error: iroha_data_model::ParseError) -> QueryExecutionFail {
    QueryExecutionFail::Conversion(error.to_string())
}
