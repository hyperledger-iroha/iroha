//! Exact-network authenticated public Musubi query helpers.

use std::time::Duration;

use super::{APPLICATION_JSON, Client};
use crate::http::{Method as HttpMethod, RequestBuilder as _, StatusCode};
use eyre::{Result, WrapErr as _, eyre};

// Exact-release JSON repeats the bounded dependency vector in the authoritative home record and
// the universal resolver row. Byte-budgeted resolver pages share this Musubi-specific ceiling,
// which remains below the shared HTTP client's 64 MiB default.
pub(super) const MUSUBI_PUBLIC_QUERY_MAX_RESPONSE_BYTES: usize =
    iroha_data_model::musubi::MUSUBI_PUBLIC_QUERY_MAX_RESPONSE_BYTES_V1;

/// First-release authenticated public Musubi query endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicMusubiQueryPathV1 {
    /// Fetch one exact structural package record.
    ExactPackage,
    /// Fetch one exact paired home/universal release snapshot.
    ExactRelease,
    /// Fetch one exact immutable provider bundle-attestation record.
    ProviderBundleAttestation,
    /// Fetch one finalized resolver-index page.
    ResolverIndex,
    /// Fetch one finalized structured-version page.
    Versions,
    /// Fetch one finalized package-member page.
    Maintainers,
    /// Fetch one finalized archive-location page.
    ArchiveLocations,
    /// Fetch bounded exact finalized archive cache-retention decisions.
    ArchiveRetention,
    /// Fetch one exact permanent global alias.
    Alias,
    /// Fetch one finalized permanent-alias history page.
    AliasHistory,
    /// Fetch one finalized byte-ordered package-prefix page.
    OrderedPrefix,
    /// Search the finalized-event package metadata projection.
    Search,
}

impl PublicMusubiQueryPathV1 {
    pub(super) const fn path(self) -> &'static str {
        match self {
            Self::ExactPackage => "/v1/musubi/queries/exact-package",
            Self::ExactRelease => "/v1/musubi/queries/exact-release",
            Self::ProviderBundleAttestation => "/v1/musubi/queries/provider-bundle-attestation",
            Self::ResolverIndex => "/v1/musubi/queries/resolver-index",
            Self::Versions => "/v1/musubi/queries/versions",
            Self::Maintainers => "/v1/musubi/queries/maintainers",
            Self::ArchiveLocations => "/v1/musubi/queries/archive-locations",
            Self::ArchiveRetention => "/v1/musubi/queries/archive-retention",
            Self::Alias => "/v1/musubi/queries/alias",
            Self::AliasHistory => "/v1/musubi/queries/alias-history",
            Self::OrderedPrefix => "/v1/musubi/queries/ordered-prefix",
            Self::Search => "/v1/musubi/queries/search",
        }
    }
}

/// Result of an authenticated public Musubi query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicMusubiQueryResultV1<T> {
    /// The exact record or finalized page was returned and decoded.
    Found(T),
    /// No exact record exists at the requested finalized state.
    NotFound,
    /// A supplied finalized cursor is stale and must not be silently restarted.
    StaleCursor,
}

/// Execute one bounded public Musubi V1 query with the client's exact-network account signer.
///
/// This function deliberately accepts only the fixed typed-query route inventory. It
/// signs the exact POST path and raw JSON body with fresh timestamp/nonce headers. The shared
/// blocking transport is redirect-free and retry-free, so the authenticated body is one-shot.
///
/// # Errors
/// Returns an error when the client or base URL is unsuitable, request signing or JSON fails,
/// transport fails, or Torii returns any status other than success, not-found, or stale-cursor.
pub fn post_public_musubi_query_v1<Q, R>(
    client: &Client,
    path: PublicMusubiQueryPathV1,
    query: &Q,
    timeout: Duration,
) -> Result<PublicMusubiQueryResultV1<R>>
where
    Q: norito::json::JsonSerialize + ?Sized,
    R: norito::json::JsonDeserialize,
{
    if !matches!(client.torii_url.scheme(), "http" | "https")
        || !client.torii_url.username().is_empty()
        || client.torii_url.password().is_some()
        || client.account.controller.single_signatory() != Some(client.key_pair.public_key())
        || client
            .headers
            .keys()
            .any(|name| name.eq_ignore_ascii_case("X-Iroha-Witness"))
    {
        return Err(eyre!("invalid authenticated Musubi client"));
    }
    let url = client
        .torii_url
        .join(path.path())
        .wrap_err("failed to build authenticated Musubi query URL")?;
    let body =
        norito::json::to_vec(query).wrap_err("failed to encode authenticated Musubi query")?;
    let mut builder = client
        .account_signed_request(HttpMethod::POST, url, body)
        .wrap_err("failed to sign authenticated Musubi query")?
        .header("Content-Type", APPLICATION_JSON)
        .header("Accept", APPLICATION_JSON)
        .max_response_bytes(MUSUBI_PUBLIC_QUERY_MAX_RESPONSE_BYTES);
    if timeout != Duration::ZERO {
        builder = builder.timeout(timeout);
    }
    let response = builder
        .build()
        .wrap_err("failed to build authenticated Musubi query")?
        .send()
        .wrap_err("authenticated Musubi query transport failed")?;
    match response.status() {
        StatusCode::OK => norito::json::from_slice(response.body())
            .map(PublicMusubiQueryResultV1::Found)
            .wrap_err("authenticated Musubi query response was invalid"),
        StatusCode::NOT_FOUND => Ok(PublicMusubiQueryResultV1::NotFound),
        StatusCode::GONE => Ok(PublicMusubiQueryResultV1::StaleCursor),
        status => Err(eyre!(
            "authenticated Musubi query failed with HTTP status {}",
            status.as_u16()
        )),
    }
}
