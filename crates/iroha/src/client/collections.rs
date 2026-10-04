//! Collection reads over the shared Torii list-query language.

use std::sync::Arc;

use norito::json::Value;

use super::{APPLICATION_JSON, AccountClient, AccountSigningCapability, Client, dispatch};
use crate::{
    Error, Result,
    collections::{Collection, CollectionReader, CollectionStream, ListQuery, Page},
    http::{Method, RequestBuilder as _, Response, StatusCode},
};

/// Largest page body accepted from Torii.
pub(super) const MAX_PAGE_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

impl Client {
    /// Read one page of a collection as a public, unsigned request.
    ///
    /// Sends `POST <collection>/query` with the canonical JSON body of `query`
    /// and returns the page with its `next_cursor`. Pass the cursor back with
    /// [`ListQuery::next_page`] or use [`Self::list`] to read every row. Public
    /// reads see public dataspaces only; [`AccountClient::list_page`] signs the
    /// request to widen visibility.
    ///
    /// ```no_run
    /// use iroha::{
    ///     collections::{Collection, ListQuery, SortKey, field},
    ///     data_model::asset::AssetDefinitionId,
    /// };
    ///
    /// # async fn example(
    /// #     client: &iroha::client::Client,
    /// #     rose: AssetDefinitionId,
    /// # ) -> iroha::Result<()> {
    /// let query = ListQuery::new()
    ///     .filter(field("quantity").gt(0))
    ///     .sort_by(SortKey::desc("quantity"))
    ///     .limit(20)
    ///     .include_total();
    /// let holders = Collection::AssetHolders(rose);
    /// let page = client.list_page(&holders, &query).await?;
    /// println!("{} of {:?} holders", page.items.len(), page.total);
    /// if let Some(next) = query.next_page(&page) {
    ///     let _second = client.list_page(&holders, &next).await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns [`Error::InvalidListQuery`] for a query that Torii would reject
    /// (see [`Collection::validate_query`]),
    /// [`Error::Api`] with the decoded `{code, message, details}` envelope when
    /// Torii rejects the request, and transport, deadline or decode errors.
    pub async fn list_page(
        &self,
        collection: &Collection,
        query: &ListQuery,
    ) -> Result<Page<Value>> {
        fetch_page(self, false, collection, query).await
    }

    /// Lazily read every row of a collection, following `next_cursor` page by page.
    ///
    /// No request is sent until the stream is polled. Requests are public and
    /// unsigned, exactly as [`Self::list_page`].
    ///
    /// ```no_run
    /// use iroha::collections::{Collection, ListQuery, TryStreamExt as _};
    ///
    /// # async fn example(client: &iroha::client::Client) -> iroha::Result<()> {
    /// let domains: Vec<_> = client
    ///     .list(Collection::Domains, ListQuery::new().limit(200))
    ///     .try_collect()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn list(&self, collection: Collection, query: ListQuery) -> CollectionStream {
        CollectionStream::new(
            CollectionReader::Public(Arc::new(self.clone())),
            collection,
            query,
        )
    }
}

impl AccountClient {
    /// Read one page of a collection with this account's canonical request signature.
    ///
    /// The signature only widens visibility into restricted dataspaces the
    /// account may read. Multisignature member contexts cannot produce the
    /// account's direct signature, so their reads are public and unsigned.
    /// Otherwise this is identical to [`Client::list_page`].
    ///
    /// # Errors
    /// Returns the errors of [`Client::list_page`], and [`Error::RequestSigning`]
    /// when the canonical request signature cannot be produced.
    pub async fn list_page(
        &self,
        collection: &Collection,
        query: &ListQuery,
    ) -> Result<Page<Value>> {
        fetch_page(&self.context, self.signs_reads(), collection, query).await
    }

    /// Lazily read every row of a collection, signing each page request as
    /// [`Self::list_page`] does.
    pub fn list(&self, collection: Collection, query: ListQuery) -> CollectionStream {
        CollectionStream::new(CollectionReader::Account(self.clone()), collection, query)
    }

    fn signs_reads(&self) -> bool {
        self.signing_capability == AccountSigningCapability::Direct
    }
}

async fn fetch_page(
    client: &Client,
    sign: bool,
    collection: &Collection,
    query: &ListQuery,
) -> Result<Page<Value>> {
    let operation = collection.operation();
    collection
        .validate_query(query)
        .map_err(|error| Error::InvalidListQuery { operation, error })?;
    let body =
        norito::json::to_vec(&query.to_json_value()).map_err(|error| Error::InvalidRequest {
            operation,
            details: format!("failed to encode the query body: {error}"),
        })?;
    let url = collection.query_url(&client.torii_url, client.account_chain_discriminant)?;
    let builder = if sign {
        client
            .account_signed_request(Method::POST, url, body)
            .map_err(|error| Error::RequestSigning {
                operation,
                details: error.to_string(),
            })?
    } else {
        client
            .request_without_canonical_account_auth(Method::POST, url)
            .body(body)
    };
    let builder = builder
        .replace_header(http::header::CONTENT_TYPE, APPLICATION_JSON)
        .max_response_bytes(MAX_PAGE_RESPONSE_BYTES);
    let response = dispatch::send(client, operation, builder, APPLICATION_JSON).await?;
    decode_page(operation, response)
}

fn decode_page(operation: &'static str, response: Response<Vec<u8>>) -> Result<Page<Value>> {
    if response.status() != StatusCode::OK {
        return Err(crate::error::http_error(operation, response));
    }
    let media_type = dispatch::media_type(operation, &response)?;
    if !media_type.eq_ignore_ascii_case(APPLICATION_JSON) {
        return Err(Error::Decode {
            operation,
            details: format!("expected an application/json page, got `{media_type}`"),
        });
    }
    norito::json::from_slice(response.body()).map_err(|error| Error::Decode {
        operation,
        details: format!("invalid page envelope: {error}"),
    })
}
