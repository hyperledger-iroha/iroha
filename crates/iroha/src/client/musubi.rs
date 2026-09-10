//! Account-authorized asynchronous Musubi registry queries.

use super::{AccountClient, dispatch};
use crate::{
    Error, Result,
    http::{Method, StatusCode},
};
use iroha_data_model::account::address::ChainDiscriminantGuard;
use iroha_data_model::musubi::{
    MusubiAliasHistoryPageV1, MusubiAliasQueryV1, MusubiAliasRecordV1, MusubiArchiveLocationPageV1,
    MusubiArchiveLocationQueryV1, MusubiArchiveRetentionPageV1, MusubiArchiveRetentionQueryV1,
    MusubiExactPackageQueryV1, MusubiExactReleaseQueryV1, MusubiExactReleaseSnapshotV1,
    MusubiMaintainerPageV1, MusubiOrderedPackagePageV1, MusubiOrderedPrefixQueryV1,
    MusubiPackagePageQueryV1, MusubiPackageRecordV1, MusubiProviderBundleAttestationKeyV1,
    MusubiProviderBundleAttestationRecordV1, MusubiResolverIndexPageV1, MusubiResolverIndexQueryV1,
    MusubiSearchPageV1, MusubiSearchQueryV1, MusubiVersionPageV1,
};
use norito::json::{JsonDeserialize, JsonSerialize};

pub(super) const MAX_RESPONSE_BYTES: usize =
    iroha_data_model::musubi::MUSUBI_PUBLIC_QUERY_MAX_RESPONSE_BYTES_V1;

/// Finalized result of one exact Musubi query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryResult<T> {
    /// The requested record or finalized page was returned.
    Found(T),
    /// No exact record exists at the requested finalized state.
    NotFound,
    /// The supplied finalized cursor has expired and must not be silently restarted.
    StaleCursor,
}

/// Typed registry reads bound to an immutable account and exact network.
///
/// Public contexts cannot issue authenticated Musubi queries:
/// ```compile_fail
/// fn read_registry(client: &iroha::client::Client) {
///     let _ = client.musubi();
/// }
/// ```
///
/// ```compile_fail
/// fn read_registry(operator: &iroha::client::OperatorClient) {
///     let _ = operator.musubi();
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Musubi<'a> {
    account: &'a AccountClient,
}

impl AccountClient {
    /// Access finalized Musubi records using this account's canonical signature.
    #[must_use]
    pub const fn musubi(&self) -> Musubi<'_> {
        Musubi { account: self }
    }
}

impl Musubi<'_> {
    /// Fetch one exact structural package record.
    ///
    /// # Errors
    /// Returns request-validation, signing, transport, deadline, HTTP, response-bound, decoding
    /// or exact response-binding errors.
    pub async fn exact_package(
        &self,
        query: &MusubiExactPackageQueryV1,
    ) -> Result<QueryResult<MusubiPackageRecordV1>> {
        let operation = "musubi.v1.query.exact_package";
        validate_request(operation, query.package.validate())?;
        let result = self
            .query(operation, "/v1/musubi/queries/exact-package", query)
            .await?;
        validate_found(result, operation, |record: &MusubiPackageRecordV1| {
            record.validate().map_err(|_| "package")?;
            if record.package != query.package {
                return Err("package");
            }
            Ok(())
        })
    }

    /// Fetch one paired finalized home and universal release snapshot.
    ///
    /// # Errors
    /// Returns request-validation, signing, transport, deadline, HTTP, response-bound, decoding
    /// or exact response-binding errors.
    pub async fn exact_release(
        &self,
        query: &MusubiExactReleaseQueryV1,
    ) -> Result<QueryResult<MusubiExactReleaseSnapshotV1>> {
        let operation = "musubi.v1.query.exact_release";
        validate_request(operation, query.release.validate())?;
        let result = self
            .query(operation, "/v1/musubi/queries/exact-release", query)
            .await?;
        validate_found(
            result,
            operation,
            |record: &MusubiExactReleaseSnapshotV1| {
                record.validate_for(query).map_err(|_| "release")?;
                if record.network_id != self.account.context.network_id {
                    return Err("network_id");
                }
                Ok(())
            },
        )
    }

    /// Fetch one immutable provider bundle-attestation record.
    ///
    /// # Errors
    /// Returns request-validation, signing, transport, deadline, HTTP, response-bound, decoding
    /// or exact response-binding errors.
    pub async fn provider_bundle_attestation(
        &self,
        query: &MusubiProviderBundleAttestationKeyV1,
    ) -> Result<QueryResult<MusubiProviderBundleAttestationRecordV1>> {
        let operation = "musubi.v1.query.provider_bundle_attestation";
        validate_request(operation, query.validate())?;
        let result = self
            .query(
                operation,
                "/v1/musubi/queries/provider-bundle-attestation",
                query,
            )
            .await?;
        validate_found(
            result,
            operation,
            |record: &MusubiProviderBundleAttestationRecordV1| {
                record.validate().map_err(|_| "attestation")?;
                record
                    .attestation
                    .verify(&record.attestation.payload.binding)
                    .map_err(|_| "attestation_signature")?;
                if &record.key != query {
                    return Err("attestation_key");
                }
                if record.attestation.payload.binding.network_id != self.account.context.network_id
                {
                    return Err("network_id");
                }
                Ok(())
            },
        )
    }

    /// Fetch one finalized resolver-index page.
    ///
    /// # Errors
    /// Returns request-validation, signing, transport, deadline, HTTP, response-bound, decoding
    /// or exact response-binding errors.
    pub async fn resolver_index(
        &self,
        query: &MusubiResolverIndexQueryV1,
    ) -> Result<QueryResult<MusubiResolverIndexPageV1>> {
        let operation = "musubi.v1.query.resolver_index";
        validate_request(operation, query.validate())?;
        let result = self
            .query(operation, "/v1/musubi/queries/resolver-index", query)
            .await?;
        validate_found(result, operation, |record: &MusubiResolverIndexPageV1| {
            record.validate_for(query).map_err(|_| "resolver_page")?;
            if record.network_id != self.account.context.network_id {
                return Err("network_id");
            }
            Ok(())
        })
    }

    /// Fetch one finalized structured-version page.
    ///
    /// # Errors
    /// Returns request-validation, signing, transport, deadline, HTTP, response-bound, decoding
    /// or exact response-binding errors.
    pub async fn versions(
        &self,
        query: &MusubiPackagePageQueryV1,
    ) -> Result<QueryResult<MusubiVersionPageV1>> {
        let operation = "musubi.v1.query.versions";
        validate_request(operation, query.validate())?;
        let result = self
            .query(operation, "/v1/musubi/queries/versions", query)
            .await?;
        validate_found(result, operation, |record: &MusubiVersionPageV1| {
            record.validate_for(query).map_err(|_| "version_page")
        })
    }

    /// Fetch accepted package members and pending invitations.
    ///
    /// # Errors
    /// Returns request-validation, signing, transport, deadline, HTTP, response-bound, decoding
    /// or exact response-binding errors.
    pub async fn maintainers(
        &self,
        query: &MusubiPackagePageQueryV1,
    ) -> Result<QueryResult<MusubiMaintainerPageV1>> {
        let operation = "musubi.v1.query.maintainers";
        validate_request(operation, query.validate())?;
        let result = self
            .query(operation, "/v1/musubi/queries/maintainers", query)
            .await?;
        validate_found(result, operation, |record: &MusubiMaintainerPageV1| {
            record.validate_for(query).map_err(|_| "maintainer_page")
        })
    }

    /// Fetch one finalized archive-location page.
    ///
    /// # Errors
    /// Returns request-validation, signing, transport, deadline, HTTP, response-bound, decoding
    /// or exact response-binding errors.
    pub async fn archive_locations(
        &self,
        query: &MusubiArchiveLocationQueryV1,
    ) -> Result<QueryResult<MusubiArchiveLocationPageV1>> {
        let operation = "musubi.v1.query.archive_locations";
        if query.archive_id.is_zero() {
            return Err(Error::InvalidRequest {
                operation,
                details: "Musubi archive id must not be the all-zero sentinel".to_owned(),
            });
        }
        validate_request(operation, query.page.validate())?;
        let result = self
            .query(operation, "/v1/musubi/queries/archive-locations", query)
            .await?;
        validate_found(result, operation, |record: &MusubiArchiveLocationPageV1| {
            record.validate().map_err(|_| "archive_page")?;
            if record.archive.archive_id != query.archive_id {
                return Err("archive_id");
            }
            if record.network_id != self.account.context.network_id {
                return Err("network_id");
            }
            if record.items.len() > query.page.effective_limit()
                || query
                    .page
                    .cursor
                    .as_ref()
                    .is_some_and(|cursor| cursor.snapshot != record.snapshot)
            {
                return Err("archive_page");
            }
            Ok(())
        })
    }

    /// Fetch exact finalized cache-retention decisions for a bounded batch.
    ///
    /// # Errors
    /// Returns request-validation, signing, transport, deadline, HTTP, response-bound, decoding
    /// or exact response-binding errors.
    pub async fn archive_retention(
        &self,
        query: &MusubiArchiveRetentionQueryV1,
    ) -> Result<QueryResult<MusubiArchiveRetentionPageV1>> {
        let operation = "musubi.v1.query.archive_retention";
        validate_request(operation, query.validate())?;
        let result = self
            .query(operation, "/v1/musubi/queries/archive-retention", query)
            .await?;
        validate_found(
            result,
            operation,
            |record: &MusubiArchiveRetentionPageV1| {
                record.validate().map_err(|_| "retention_page")?;
                if record.network_id != self.account.context.network_id {
                    return Err("network_id");
                }
                if query
                    .expected_snapshot
                    .is_some_and(|snapshot| snapshot != record.snapshot)
                    || record
                        .items
                        .iter()
                        .map(|item| item.archive_id)
                        .ne(query.archive_ids.iter().copied())
                {
                    return Err("retention_request");
                }
                Ok(())
            },
        )
    }

    /// Fetch one permanent global alias.
    ///
    /// # Errors
    /// Returns request-validation, signing, transport, deadline, HTTP, response-bound, decoding
    /// or exact response-binding errors.
    pub async fn alias(
        &self,
        query: &MusubiAliasQueryV1,
    ) -> Result<QueryResult<MusubiAliasRecordV1>> {
        let operation = "musubi.v1.query.alias";
        validate_request(operation, query.validate())?;
        let result = self
            .query(operation, "/v1/musubi/queries/alias", query)
            .await?;
        validate_found(result, operation, |record: &MusubiAliasRecordV1| {
            // Pricing validation needs a separately trusted policy; this response carries none.
            record.alias.validate().map_err(|_| "alias")?;
            record.target.validate().map_err(|_| "alias_target")?;
            iroha_data_model::musubi::validate_musubi_account_id_v1(&record.registered_by)
                .map_err(|_| "alias_registered_by")?;
            if record.alias != query.alias
                || record.pricing_revision == 0
                || record.paid_xor == 0
                || record.registered_at_height == 0
                || record.history_revision == 0
            {
                return Err("alias");
            }
            Ok(())
        })
    }

    /// Fetch one finalized permanent-alias history page.
    ///
    /// # Errors
    /// Returns request-validation, signing, transport, deadline, HTTP, response-bound, decoding
    /// or exact response-binding errors.
    pub async fn alias_history(
        &self,
        query: &MusubiAliasQueryV1,
    ) -> Result<QueryResult<MusubiAliasHistoryPageV1>> {
        let operation = "musubi.v1.query.alias_history";
        validate_request(operation, query.validate())?;
        let result = self
            .query(operation, "/v1/musubi/queries/alias-history", query)
            .await?;
        validate_found(result, operation, |record: &MusubiAliasHistoryPageV1| {
            record.validate_for(query).map_err(|_| "alias_history_page")
        })
    }

    /// Fetch one finalized byte-ordered package-prefix page.
    ///
    /// # Errors
    /// Returns request-validation, signing, transport, deadline, HTTP, response-bound, decoding
    /// or exact response-binding errors.
    pub async fn ordered_prefix(
        &self,
        query: &MusubiOrderedPrefixQueryV1,
    ) -> Result<QueryResult<MusubiOrderedPackagePageV1>> {
        let operation = "musubi.v1.query.ordered_prefix";
        validate_request(operation, query.validate())?;
        let result = self
            .query(operation, "/v1/musubi/queries/ordered-prefix", query)
            .await?;
        validate_found(result, operation, |record: &MusubiOrderedPackagePageV1| {
            record.validate_for(query).map_err(|_| "directory_page")?;
            if record.network_id != self.account.context.network_id {
                return Err("network_id");
            }
            Ok(())
        })
    }

    /// Search the finalized-event metadata projection.
    ///
    /// # Errors
    /// Returns request-validation, signing, transport, deadline, HTTP, response-bound, decoding
    /// or exact response-binding errors.
    pub async fn search(
        &self,
        query: &MusubiSearchQueryV1,
    ) -> Result<QueryResult<MusubiSearchPageV1>> {
        let operation = "musubi.v1.query.search";
        validate_request(operation, query.validate())?;
        let result = self
            .query(operation, "/v1/musubi/queries/search", query)
            .await?;
        validate_found(result, operation, |record: &MusubiSearchPageV1| {
            record.validate_for(query).map_err(|_| "search_page")
        })
    }

    async fn query<Q: JsonSerialize + Sync + ?Sized, R: JsonDeserialize>(
        &self,
        operation: &'static str,
        path: &'static str,
        query: &Q,
    ) -> Result<QueryResult<R>> {
        let client = &self.account.context;
        if client.account.controller.single_signatory() != Some(client.key_pair.public_key())
            || client
                .headers
                .keys()
                .any(|name| name.eq_ignore_ascii_case("X-Iroha-Witness"))
        {
            return Err(Error::InvalidRequest {
                operation,
                details: "Musubi queries require a direct account signer without witness headers"
                    .to_owned(),
            });
        }
        // Account JSON and canonical request headers use the immutable context's
        // address discriminant. This synchronous scope ends before awaiting I/O;
        // it never changes the process default or leaks onto another task.
        let builder = {
            let _format = ChainDiscriminantGuard::enter(client.account_chain_discriminant);
            let url = client
                .torii_url
                .join(path)
                .map_err(|error| Error::InvalidRequest {
                    operation,
                    details: error.to_string(),
                })?;
            let body = norito::json::to_vec(query).map_err(|error| Error::InvalidRequest {
                operation,
                details: error.to_string(),
            })?;
            client
                .account_signed_request(Method::POST, url, body)
                .map_err(|error| Error::RequestSigning {
                    operation,
                    details: error.to_string(),
                })?
                .replace_header(http::header::CONTENT_TYPE, "application/json")
                .max_response_bytes(MAX_RESPONSE_BYTES)
        };
        let response = dispatch::send(client, operation, builder, "application/json").await?;
        match response.status() {
            StatusCode::NOT_FOUND => return Ok(QueryResult::NotFound),
            StatusCode::GONE => return Ok(QueryResult::StaleCursor),
            StatusCode::OK => {}
            status => {
                return Err(Error::Http {
                    operation,
                    status: status.as_u16(),
                    retry_after: crate::error::retry_after(response.headers()),
                    body: response.into_body(),
                });
            }
        }
        if !dispatch::media_type(operation, &response)?.eq_ignore_ascii_case("application/json") {
            return Err(Error::Decode {
                operation,
                details: "expected application/json Musubi response".to_owned(),
            });
        }
        let _format = ChainDiscriminantGuard::enter(client.account_chain_discriminant);
        norito::json::from_slice(response.body())
            .map(QueryResult::Found)
            .map_err(|error| Error::Decode {
                operation,
                details: error.to_string(),
            })
    }
}

// These helpers own validation only; no transport or alternate request inventory is exposed.
fn validate_request(
    operation: &'static str,
    validation: core::result::Result<(), impl core::fmt::Display>,
) -> Result<()> {
    validation.map_err(|error| Error::InvalidRequest {
        operation,
        details: error.to_string(),
    })
}

fn validate_found<T>(
    result: QueryResult<T>,
    operation: &'static str,
    validate: impl FnOnce(&T) -> core::result::Result<(), &'static str>,
) -> Result<QueryResult<T>> {
    if let QueryResult::Found(record) = &result {
        validate(record).map_err(|field| Error::ResponseBinding { operation, field })?;
    }
    Ok(result)
}
