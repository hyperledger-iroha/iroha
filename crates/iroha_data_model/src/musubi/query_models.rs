// Query and search wire models included at Musubi module scope.
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
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        parse_clean(
            raw,
            "Musubi ordered prefix must not be empty",
            "Musubi ordered prefix is invalid",
        )?;
        if raw.len() > MUSUBI_MAX_CURSOR_KEY_BYTES_V1 {
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
    next_cursor: &Option<MusubiFinalizedCursorV1>,
    snapshot: MusubiRegistrySnapshotV1,
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
    if let Some(cursor) = &request.cursor {
        if cursor.snapshot != snapshot || cursor.caller.is_some() {
            return Err(ParseError::new(
                "Musubi response page does not continue its request cursor",
            ));
        }
    }
    if let Some(cursor) = next_cursor {
        cursor.validate()?;
        if cursor.snapshot != snapshot
            || cursor.caller.is_some()
            || item_count != request.effective_limit()
            || Some(cursor.last_key.as_str()) != last_key
            || request
                .cursor
                .as_ref()
                .is_some_and(|previous| previous.query_hash != cursor.query_hash)
        {
            return Err(ParseError::new(
                "Musubi response next cursor does not bind its exact full page",
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
    /// Optional SemVer filtering requirement.
    pub requirement: Option<MusubiVersionReqV1>,
    /// Page controls and finalized cursor.
    pub page: MusubiPageRequestV1,
}

impl MusubiResolverIndexQueryV1 {
    /// Validate structural package, optional requirement, and page controls.
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
        if let Some(cursor) = &self.query.page.cursor {
            if cursor.snapshot != self.snapshot
                || self
                    .items
                    .first()
                    .is_some_and(|item| item.package <= cursor.last_package)
            {
                return Err(ParseError::new(
                    "Musubi search page does not continue its request cursor",
                ));
            }
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
