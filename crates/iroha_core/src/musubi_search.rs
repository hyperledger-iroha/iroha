//! Rebuildable finalized-event projection for Musubi package discovery.
//!
//! This index is deliberately process-local and absent from [`crate::state::World`].
//! Consensus resolution continues to use the universal sparse resolver index; this
//! projection serves only description, keyword, namespace, and package-name search.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    ops::Bound::{Excluded, Unbounded},
};

use iroha_data_model::{
    events::data::musubi::MusubiEvent,
    musubi::{
        MusubiDescriptionV1, MusubiKeywordV1, MusubiNamespaceV1, MusubiPackageIdV1,
        MusubiReleaseMetadataV1,
    },
};

/// Maximum accepted UTF-8 query size.
pub const MUSUBI_SEARCH_MAX_QUERY_BYTES_V1: usize = 256;
/// Maximum distinct normalized query terms.
pub const MUSUBI_SEARCH_MAX_QUERY_TERMS_V1: usize = 16;
/// Maximum results returned in one page.
pub const MUSUBI_SEARCH_MAX_PAGE_SIZE_V1: usize = 100;
/// Maximum document terms retained after deterministic priority ordering.
pub const MUSUBI_SEARCH_MAX_DOCUMENT_TERMS_V1: usize = 256;
/// Maximum candidate package rows inspected for a multi-term page.
pub const MUSUBI_SEARCH_MAX_CANDIDATE_SCAN_V1: usize = 16_384;
const MUSUBI_SEARCH_MAX_TERM_BYTES_V1: usize = 64;

/// Failure while rebuilding or querying the non-consensus search projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiSearchError {
    /// A finalized event contradicts an earlier event in the same replay.
    InconsistentFinalizedEvent,
    /// The query is empty or exceeds its byte/term bound.
    InvalidQuery,
    /// The requested result page is empty or exceeds its bound.
    InvalidPageSize,
    /// A sparse multi-term intersection exceeded its deterministic scan budget.
    QueryTooBroad,
}

impl fmt::Display for MusubiSearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InconsistentFinalizedEvent => {
                "Musubi finalized search event contradicts the rebuilt projection"
            }
            Self::InvalidQuery => "Musubi search query is empty or exceeds its V1 bounds",
            Self::InvalidPageSize => "Musubi search page size is outside its V1 bounds",
            Self::QueryTooBroad => "Musubi search query exceeds its bounded candidate scan",
        })
    }
}

impl std::error::Error for MusubiSearchError {}

/// One deterministic discovery-search result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusubiSearchHitV1 {
    /// Structural package identity; aliases never replace it.
    pub package: MusubiPackageIdV1,
    /// Immutable namespace used for the initial package claim, when replayed.
    pub claimed_namespace: Option<MusubiNamespaceV1>,
    /// Current mutable package description.
    pub description: Option<MusubiDescriptionV1>,
    /// Current sorted package keywords.
    pub keywords: Vec<MusubiKeywordV1>,
    /// Current mutable-metadata revision, or zero before its first projection.
    pub metadata_revision: u64,
}

/// One ordered page from the rebuildable discovery index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusubiSearchPageV1 {
    /// Results ordered by structural package identity.
    pub hits: Vec<MusubiSearchHitV1>,
    /// Last returned package when another page exists.
    pub next_after: Option<MusubiPackageIdV1>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct MusubiSearchDocument {
    claimed_namespace: Option<MusubiNamespaceV1>,
    metadata: MusubiReleaseMetadataV1,
    metadata_revision: u64,
    terms: BTreeSet<String>,
}

/// Process-local rich-search state rebuildable solely from finalized Musubi events.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MusubiSearchProjectionV1 {
    documents: BTreeMap<MusubiPackageIdV1, MusubiSearchDocument>,
    postings: BTreeMap<String, BTreeSet<MusubiPackageIdV1>>,
}

impl MusubiSearchProjectionV1 {
    /// Rebuild a fresh projection from a canonically ordered finalized event stream.
    pub fn rebuild(
        events: impl IntoIterator<Item = &'_ MusubiEvent>,
    ) -> Result<Self, MusubiSearchError> {
        let mut projection = Self::default();
        for event in events {
            projection.apply_finalized(event)?;
        }
        Ok(projection)
    }

    /// Apply one event after its containing block is finalized.
    ///
    /// Events unrelated to rich package discovery are intentionally ignored. Exact
    /// dependency resolution must never read this projection.
    pub fn apply_finalized(&mut self, event: &MusubiEvent) -> Result<(), MusubiSearchError> {
        match event {
            MusubiEvent::PackageClaimed(event) => {
                if let Some(existing) = self.documents.get(&event.package)
                    && existing
                        .claimed_namespace
                        .as_ref()
                        .is_some_and(|namespace| namespace != &event.namespace)
                {
                    return Err(MusubiSearchError::InconsistentFinalizedEvent);
                }
                let package = event.package.clone();
                self.remove_postings(&package);
                let document = self.documents.entry(package.clone()).or_default();
                document.claimed_namespace = Some(event.namespace.clone());
                document.terms = document_terms(&package, document);
                self.insert_postings(&package);
            }
            MusubiEvent::PackageMetadataChanged(event) => {
                event
                    .validate()
                    .map_err(|_| MusubiSearchError::InconsistentFinalizedEvent)?;
                let package = event.package.clone();
                if let Some(existing) = self.documents.get(&package) {
                    if event.revision < existing.metadata_revision
                        || (event.revision == existing.metadata_revision
                            && event.metadata != existing.metadata)
                    {
                        return Err(MusubiSearchError::InconsistentFinalizedEvent);
                    }
                    if event.revision == existing.metadata_revision {
                        return Ok(());
                    }
                }
                self.remove_postings(&package);
                let document = self.documents.entry(package.clone()).or_default();
                document.metadata = event.metadata.clone();
                document.metadata_revision = event.revision;
                document.terms = document_terms(&package, document);
                self.insert_postings(&package);
            }
            _ => {}
        }
        Ok(())
    }

    /// Search exact normalized terms without scanning the package directory.
    ///
    /// All query terms must match. `after` is the last structural package key from
    /// the preceding page, making pagination independent of process hash ordering.
    pub fn search(
        &self,
        query: &str,
        after: Option<&MusubiPackageIdV1>,
        limit: usize,
    ) -> Result<MusubiSearchPageV1, MusubiSearchError> {
        if limit == 0 || limit > MUSUBI_SEARCH_MAX_PAGE_SIZE_V1 {
            return Err(MusubiSearchError::InvalidPageSize);
        }
        let terms = query_terms(query)?;
        let mut posting_sets = Vec::with_capacity(terms.len());
        for term in &terms {
            let Some(posting) = self.postings.get(term) else {
                return Ok(MusubiSearchPageV1 {
                    hits: Vec::new(),
                    next_after: None,
                });
            };
            posting_sets.push(posting);
        }
        posting_sets.sort_by_key(|posting| posting.len());
        let anchor = posting_sets
            .first()
            .expect("validated Musubi search query has at least one term");
        let mut hits = Vec::with_capacity(limit.saturating_add(1));
        let mut scanned = 0_usize;
        let candidates: Box<dyn Iterator<Item = &MusubiPackageIdV1> + '_> = match after {
            Some(after) => Box::new(anchor.range((Excluded(after), Unbounded))),
            None => Box::new(anchor.iter()),
        };
        for package in candidates {
            scanned = scanned.saturating_add(1);
            if posting_sets.len() > 1 && scanned > MUSUBI_SEARCH_MAX_CANDIDATE_SCAN_V1 {
                return Err(MusubiSearchError::QueryTooBroad);
            }
            if !posting_sets[1..]
                .iter()
                .all(|posting| posting.contains(package))
            {
                continue;
            }
            let document = self
                .documents
                .get(package)
                .expect("Musubi search posting references a document");
            hits.push(MusubiSearchHitV1 {
                package: package.clone(),
                claimed_namespace: document.claimed_namespace.clone(),
                description: document.metadata.description.clone(),
                keywords: document.metadata.keywords.clone(),
                metadata_revision: document.metadata_revision,
            });
            if hits.len() > limit {
                break;
            }
        }
        let has_more = hits.len() > limit;
        if has_more {
            hits.pop();
        }
        let next_after = has_more.then(|| {
            hits.last()
                .expect("non-empty bounded Musubi search page")
                .package
                .clone()
        });
        Ok(MusubiSearchPageV1 { hits, next_after })
    }

    /// Number of currently projected package documents.
    #[must_use]
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    fn remove_postings(&mut self, package: &MusubiPackageIdV1) {
        let Some(document) = self.documents.get(package) else {
            return;
        };
        let terms = document.terms.iter().cloned().collect::<Vec<_>>();
        for term in terms {
            let remove_term = self.postings.get_mut(&term).is_some_and(|posting| {
                posting.remove(package);
                posting.is_empty()
            });
            if remove_term {
                self.postings.remove(&term);
            }
        }
    }

    fn insert_postings(&mut self, package: &MusubiPackageIdV1) {
        let terms = self
            .documents
            .get(package)
            .expect("Musubi search document exists before indexing")
            .terms
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        for term in terms {
            self.postings
                .entry(term)
                .or_default()
                .insert(package.clone());
        }
    }
}

fn document_terms(
    package: &MusubiPackageIdV1,
    document: &MusubiSearchDocument,
) -> BTreeSet<String> {
    let mut terms = BTreeSet::new();
    insert_exact_and_words(package.name.as_str(), &mut terms);
    if let Some(namespace) = &document.claimed_namespace {
        insert_exact_and_words(namespace.as_str(), &mut terms);
    }
    for keyword in &document.metadata.keywords {
        if terms.len() >= MUSUBI_SEARCH_MAX_DOCUMENT_TERMS_V1 {
            return terms;
        }
        insert_exact_and_words(keyword.to_string().as_str(), &mut terms);
    }
    if let Some(description) = &document.metadata.description {
        insert_words(description.as_str(), &mut terms);
    }
    terms
}

fn insert_exact_and_words(value: &str, terms: &mut BTreeSet<String>) {
    if value.len() <= MUSUBI_SEARCH_MAX_TERM_BYTES_V1
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        insert_term(value.to_ascii_lowercase(), terms);
    }
    insert_words(value, terms);
}

fn insert_words(value: &str, terms: &mut BTreeSet<String>) {
    for word in value.split(|character: char| !character.is_alphanumeric()) {
        if terms.len() >= MUSUBI_SEARCH_MAX_DOCUMENT_TERMS_V1 {
            return;
        }
        let normalized = word
            .chars()
            .flat_map(char::to_lowercase)
            .collect::<String>();
        insert_term(normalized, terms);
    }
}

fn insert_term(term: String, terms: &mut BTreeSet<String>) {
    if !term.is_empty()
        && term.len() <= MUSUBI_SEARCH_MAX_TERM_BYTES_V1
        && terms.len() < MUSUBI_SEARCH_MAX_DOCUMENT_TERMS_V1
    {
        terms.insert(term);
    }
}

fn query_terms(query: &str) -> Result<BTreeSet<String>, MusubiSearchError> {
    if query.is_empty() || query.len() > MUSUBI_SEARCH_MAX_QUERY_BYTES_V1 {
        return Err(MusubiSearchError::InvalidQuery);
    }
    let mut terms = BTreeSet::new();
    for component in query.split_whitespace() {
        insert_exact_and_words(component, &mut terms);
    }
    if terms.is_empty() || terms.len() > MUSUBI_SEARCH_MAX_QUERY_TERMS_V1 {
        return Err(MusubiSearchError::InvalidQuery);
    }
    Ok(terms)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        events::data::musubi::{MusubiEvent, MusubiPackageClaimedEventV1},
        musubi::{MusubiPackageMetadataRecordV1, MusubiPackageScopeV1, MusubiReleaseMetadataV1},
        nexus::DataSpaceId,
    };

    use super::*;

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed derives account");
        AccountId::new(keypair.public_key().clone())
    }

    fn package(name: &str) -> MusubiPackageIdV1 {
        MusubiPackageIdV1::new(
            DataSpaceId::new(9),
            MusubiPackageScopeV1::DataspaceRoot,
            name.parse().expect("package name"),
        )
    }

    fn claimed(package: MusubiPackageIdV1) -> MusubiEvent {
        MusubiEvent::PackageClaimed(MusubiPackageClaimedEventV1 {
            package,
            namespace: "sora".parse().expect("namespace"),
            claimed_by: account(1),
            governance_revision: 1,
            finalized_height: 1,
        })
    }

    fn metadata(
        package: MusubiPackageIdV1,
        revision: u64,
        description: &str,
        keywords: &[&str],
    ) -> MusubiEvent {
        MusubiEvent::PackageMetadataChanged(MusubiPackageMetadataRecordV1 {
            package,
            metadata: MusubiReleaseMetadataV1 {
                description: Some(description.parse().expect("description")),
                keywords: keywords
                    .iter()
                    .map(|keyword| keyword.parse().expect("keyword"))
                    .collect(),
                ..MusubiReleaseMetadataV1::default()
            },
            revision,
            changed_by: account(1),
            changed_at_height: revision,
        })
    }

    #[test]
    fn rebuild_and_incremental_application_are_identical() {
        let package = package("proof-kit");
        let events = vec![
            claimed(package.clone()),
            metadata(
                package,
                1,
                "Deterministic zero knowledge verifier",
                &["cryptography", "zero-knowledge"],
            ),
        ];
        let rebuilt = MusubiSearchProjectionV1::rebuild(&events).expect("rebuild projection");
        let mut incremental = MusubiSearchProjectionV1::default();
        for event in &events {
            incremental
                .apply_finalized(event)
                .expect("apply finalized event");
        }
        assert_eq!(rebuilt, incremental);
        let page = rebuilt
            .search("zero-knowledge verifier", None, 50)
            .expect("search");
        assert_eq!(page.hits.len(), 1);
        assert_eq!(page.hits[0].package.name.as_str(), "proof-kit");
    }

    #[test]
    fn metadata_replacement_removes_stale_description_tokens() {
        let package = package("codec");
        let events = [
            claimed(package.clone()),
            metadata(package.clone(), 1, "legacy binary codec", &["codec"]),
            metadata(package, 2, "canonical norito encoding", &["serialization"]),
        ];
        let projection = MusubiSearchProjectionV1::rebuild(&events).expect("rebuild projection");
        assert!(
            projection
                .search("legacy", None, 50)
                .expect("search")
                .hits
                .is_empty()
        );
        let current = projection.search("norito", None, 50).expect("search");
        assert_eq!(current.hits.len(), 1);
        assert_eq!(current.hits[0].metadata_revision, 2);
    }

    #[test]
    fn inconsistent_same_revision_metadata_is_rejected() {
        let package = package("codec");
        let events = [
            metadata(package.clone(), 1, "first description", &["codec"]),
            metadata(package, 1, "substituted description", &["codec"]),
        ];
        assert_eq!(
            MusubiSearchProjectionV1::rebuild(&events),
            Err(MusubiSearchError::InconsistentFinalizedEvent)
        );
    }
}
