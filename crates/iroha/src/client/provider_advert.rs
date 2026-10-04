//! Bounded self-authenticating provider advert dispatch through the owning SDK transport.

use super::*;
use sorafs_manifest::provider_advert::{PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1, ProviderAdvertV1};

const RESPONSE_MAX: usize = 16 * 1024;

impl Client {
    /// Send one already signed provider advert to the configured Torii endpoint.
    ///
    /// The protocol envelope authenticates its provider. This performs one bounded dispatch,
    /// with no transaction signing or automatic retry. A successful HTTP acknowledgement is
    /// not current admission, custody, capacity or package-read authority; native discovery
    /// remains required. [`Self::with_request_deadline`] bounds the entire dispatch.
    /// # Errors
    /// Refuses another network, invalid/expired advert, invalid signature, excessive canonical
    /// bytes, elapsed request deadline or transport failure. Non-success HTTP responses remain
    /// visible to the caller with their original status and bounded body.
    pub fn post_sorafs_provider_advert(
        &self,
        advert: &ProviderAdvertV1,
    ) -> Result<Response<Vec<u8>>> {
        if advert.network_id != *self.network_id.as_bytes() || !advert.signature_strict {
            return Err(eyre!("provider advert network or signature policy differs"));
        }
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let length = norito::core::encoded_frame_len(advert)?;
        if length > PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1 {
            return Err(eyre!("provider advert exceeds canonical frame bound"));
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        advert.validate_with_body(now)?;
        advert.verify_signature()?;
        let body = norito::core::to_bytes_bounded(advert, length)?;
        let url = join_torii_url(
            &self.torii_url,
            iroha_torii_shared::route_catalog::sorafs::PROVIDER_ADVERT
                .path()
                .trim_start_matches('/'),
        );
        self.send_builder(
            self.request_without_canonical_account_auth(HttpMethod::POST, url)
                .header(http::header::CONTENT_TYPE, APPLICATION_NORITO)
                .header(http::header::ACCEPT, APPLICATION_JSON)
                .max_response_bytes(RESPONSE_MAX)
                .body(body),
        )
    }
}

#[cfg(test)]
#[path = "provider_advert_tests.rs"]
mod tests;
