//! Wallet load reads on the account facade's reusable asynchronous runtime.

use super::{AccountClient, RuntimeOwner};
use crate::Result;
use iroha_data_model::isi::kagemusha_wallet::load_finality::KagemushaWalletLoadReceiptV1;

/// Blocking access to the canonical account-authenticated wallet read capability.
#[derive(Clone, Copy, Debug)]
pub struct Kagemusha<'a> {
    inner: crate::client::kagemusha::Kagemusha<'a>,
    runtime: &'a RuntimeOwner,
}

impl AccountClient {
    /// Retrieve load originals through this account's immutable authority and owned runtime.
    #[must_use]
    pub fn kagemusha(&self) -> Kagemusha<'_> {
        Kagemusha {
            inner: self.inner.kagemusha(),
            runtime: &self.runtime,
        }
    }
}

impl Kagemusha<'_> {
    /// Retrieve one original load transaction receipt without granting balance authority.
    ///
    /// # Errors
    /// Returns the canonical asynchronous failure or a typed blocking-runtime rejection.
    pub fn load_issuance(
        &self,
        scheme: &[u8; 32],
        wallet: &[u8; 32],
        request: &[u8; 32],
    ) -> Result<KagemushaWalletLoadReceiptV1> {
        self.runtime
            .block_on(self.inner.load_issuance(scheme, wallet, request))?
    }
}
