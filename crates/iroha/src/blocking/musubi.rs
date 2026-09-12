//! Musubi registry operations on the account's reusable blocking runtime.

use super::{AccountClient, RuntimeOwner};
use crate::{Result, client::musubi::QueryResult};
use iroha_data_model::musubi::{
    MusubiAliasHistoryPageV1, MusubiAliasQueryV1, MusubiAliasRecordV1, MusubiArchiveLocationPageV1,
    MusubiArchiveLocationQueryV1, MusubiArchiveRetentionPageV1, MusubiArchiveRetentionQueryV1,
    MusubiExactPackageQueryV1, MusubiExactReleaseQueryV1, MusubiExactReleaseSnapshotV1,
    MusubiMaintainerPageV1, MusubiOrderedPackagePageV1, MusubiOrderedPrefixQueryV1,
    MusubiPackagePageQueryV1, MusubiPackageRecordV1, MusubiProviderBundleAttestationKeyV1,
    MusubiProviderBundleAttestationRecordV1, MusubiResolverIndexPageV1, MusubiResolverIndexQueryV1,
    MusubiSearchPageV1, MusubiSearchQueryV1, MusubiVersionPageV1,
};

/// Blocking registry reads backed by the canonical asynchronous account capability.
///
/// An unbound public client cannot issue authenticated queries:
/// ```compile_fail
/// fn read_registry(client: &iroha::blocking::Client) {
///     let _ = client.musubi();
/// }
/// ```
///
/// Operator authority does not imply account authority:
/// ```compile_fail
/// fn read_registry(operator: &iroha::blocking::OperatorClient) {
///     let _ = operator.musubi();
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Musubi<'a> {
    inner: crate::client::musubi::Musubi<'a>,
    runtime: &'a RuntimeOwner,
}

impl AccountClient {
    /// Read finalized Musubi records using this facade's bound account and runtime.
    #[must_use]
    pub fn musubi(&self) -> Musubi<'_> {
        Musubi {
            inner: self.inner.musubi(),
            runtime: &self.runtime,
        }
    }
}

impl Musubi<'_> {
    /// Fetch one exact structural package record.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn exact_package(
        &self,
        query: &MusubiExactPackageQueryV1,
    ) -> Result<QueryResult<MusubiPackageRecordV1>> {
        self.runtime.block_on(self.inner.exact_package(query))?
    }

    /// Fetch one paired finalized home and universal release snapshot.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn exact_release(
        &self,
        query: &MusubiExactReleaseQueryV1,
    ) -> Result<QueryResult<MusubiExactReleaseSnapshotV1>> {
        self.runtime.block_on(self.inner.exact_release(query))?
    }

    /// Fetch one immutable provider bundle-attestation record.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn provider_bundle_attestation(
        &self,
        query: &MusubiProviderBundleAttestationKeyV1,
    ) -> Result<QueryResult<MusubiProviderBundleAttestationRecordV1>> {
        self.runtime
            .block_on(self.inner.provider_bundle_attestation(query))?
    }

    /// Fetch one finalized resolver-index page.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn resolver_index(
        &self,
        query: &MusubiResolverIndexQueryV1,
    ) -> Result<QueryResult<MusubiResolverIndexPageV1>> {
        self.runtime.block_on(self.inner.resolver_index(query))?
    }

    /// Fetch one finalized structured-version page.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn versions(
        &self,
        query: &MusubiPackagePageQueryV1,
    ) -> Result<QueryResult<MusubiVersionPageV1>> {
        self.runtime.block_on(self.inner.versions(query))?
    }

    /// Fetch accepted package members and pending invitations.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn maintainers(
        &self,
        query: &MusubiPackagePageQueryV1,
    ) -> Result<QueryResult<MusubiMaintainerPageV1>> {
        self.runtime.block_on(self.inner.maintainers(query))?
    }

    /// Fetch one finalized archive-location page.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn archive_locations(
        &self,
        query: &MusubiArchiveLocationQueryV1,
    ) -> Result<QueryResult<MusubiArchiveLocationPageV1>> {
        self.runtime.block_on(self.inner.archive_locations(query))?
    }

    /// Fetch exact finalized cache-retention decisions for a bounded batch.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn archive_retention(
        &self,
        query: &MusubiArchiveRetentionQueryV1,
    ) -> Result<QueryResult<MusubiArchiveRetentionPageV1>> {
        self.runtime.block_on(self.inner.archive_retention(query))?
    }

    /// Fetch one permanent global alias.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn alias(&self, query: &MusubiAliasQueryV1) -> Result<QueryResult<MusubiAliasRecordV1>> {
        self.runtime.block_on(self.inner.alias(query))?
    }

    /// Fetch one finalized permanent-alias history page.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn alias_history(
        &self,
        query: &MusubiAliasQueryV1,
    ) -> Result<QueryResult<MusubiAliasHistoryPageV1>> {
        self.runtime.block_on(self.inner.alias_history(query))?
    }

    /// Fetch one finalized byte-ordered package-prefix page.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn ordered_prefix(
        &self,
        query: &MusubiOrderedPrefixQueryV1,
    ) -> Result<QueryResult<MusubiOrderedPackagePageV1>> {
        self.runtime.block_on(self.inner.ordered_prefix(query))?
    }

    /// Search the finalized-event metadata projection.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn search(&self, query: &MusubiSearchQueryV1) -> Result<QueryResult<MusubiSearchPageV1>> {
        self.runtime.block_on(self.inner.search(query))?
    }
}
