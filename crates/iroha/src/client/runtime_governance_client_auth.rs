//! Compact exact-network account-authenticated transport helpers for runtime/governance reads.
use super::{Client, DefaultRequestBuilder, HttpMethod, Response};
use eyre::Result;
use url::Url;
impl Client {
    pub(super) fn account_signed_get_request(&self, url: Url) -> Result<DefaultRequestBuilder> {
        self.account_signed_request(HttpMethod::GET, url, Vec::new())
    }
    pub(super) fn send_account_signed_get(&self, url: Url) -> Result<Response<Vec<u8>>> {
        self.send_builder(self.account_signed_get_request(url)?)
    }
}
