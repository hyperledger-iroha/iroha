//! Account-authenticated retrieval of canonical wallet load issuance originals.

use super::{AccountClient, ActivationEvidenceReadAuth, Client, dispatch};
use crate::{Error, Result, http::StatusCode};
use iroha_data_model::{
    isi::kagemusha_wallet::KagemushaWalletLoadIssuanceV1, kagemusha::KagemushaWalletLoadVoucherV1,
};

// A voucher is at most 1,024 bytes. This independent response capacity also covers the
// complete original body, canonical account controller and Norito framing.
pub(super) const MAX_RESPONSE_BYTES: usize = 16 * 1024;
const READ: &str = "kagemusha.wallet.load_issuance.read";

/// Load issuance reads using one immutable payer and network context.
///
/// These records are transport data. The shared native wallet still verifies the signed
/// voucher and its complete authenticated Load relation before Advance changes a balance.
///
/// ```compile_fail
/// fn read_load(client: &iroha::client::Client) {
///     let _ = client.kagemusha();
/// }
/// ```
///
/// ```compile_fail
/// fn read_load(operator: &iroha::client::OperatorClient) {
///     let _ = operator.kagemusha();
/// }
#[derive(Clone, Copy, Debug)]
pub struct Kagemusha<'a> {
    account: &'a AccountClient,
}

impl AccountClient {
    /// Retrieve retained wallet load records using this account's exact network signature.
    #[must_use]
    pub const fn kagemusha(&self) -> Kagemusha<'_> {
        Kagemusha { account: self }
    }
}

impl Kagemusha<'_> {
    /// Retrieve one exact issuance and its original voucher, if publication has completed.
    ///
    /// All three identities must be nonzero. The request signs the canonical lowercase
    /// route for this context's payer and network. It performs one bounded asynchronous
    /// GET, without a compatibility probe, alternate codec or automatic retry. A pending
    /// publication remains `voucher: None`; an unavailable source remains an error.
    /// Neither response form grants finality, completion or offline balance authority.
    ///
    /// # Errors
    /// Rejects invalid identities or a non-direct signer, signing/transport/deadline/HTTP
    /// failures, noncanonical binary responses, foreign payer/scope or a substituted voucher.
    pub async fn load_issuance(
        &self,
        scheme: &[u8; 32],
        wallet: &[u8; 32],
        request: &[u8; 32],
    ) -> Result<KagemushaWalletLoadIssuanceV1> {
        let client = &self.account.context;
        for identity in [scheme, wallet, request] {
            if identity == &[0; 32] {
                return Err(Error::InvalidRequest {
                    operation: READ,
                    details: "load identities must be nonzero".to_owned(),
                });
            }
        }
        if client.account.controller.single_signatory() != Some(client.key_pair.public_key())
            || client
                .headers
                .keys()
                .any(|name| name.eq_ignore_ascii_case("X-Iroha-Witness"))
        {
            return Err(Error::InvalidRequest {
                operation: READ,
                details: "load reads require the direct payer signer without witness headers"
                    .to_owned(),
            });
        }
        ensure_deadline(client)?;
        let path = iroha_torii_shared::route_catalog::contracts_and_verification_keys::KAGEMUSHA_LOAD_ISSUANCE_GET
            .path()
            .replace("{scheme}", &hex::encode(scheme))
            .replace("{wallet}", &hex::encode(wallet))
            .replace("{request}", &hex::encode(request));
        let builder = client
            .canonical_norito_get_request(
                &path,
                MAX_RESPONSE_BYTES,
                ActivationEvidenceReadAuth::Account,
            )
            .map_err(|error| Error::RequestSigning {
                operation: READ,
                details: error.to_string(),
            })?;
        let response = dispatch::send(client, READ, builder, "application/x-norito").await?;
        if response.status() != StatusCode::OK {
            return Err(Error::Http {
                operation: READ,
                status: response.status().as_u16(),
                retry_after: crate::error::retry_after(response.headers()),
                body: response.into_body(),
            });
        }
        let issuance =
            Client::decode_canonical_norito_response(&response, MAX_RESPONSE_BYTES, READ)?;
        validate_response(&issuance, self.account.authority(), scheme, wallet, request)?;
        ensure_deadline(client)?;
        Ok(issuance)
    }
}

fn ensure_deadline(client: &Client) -> Result<()> {
    if client
        .http_transport
        .deadline()
        .is_some_and(|deadline| std::time::Instant::now() >= deadline)
    {
        return Err(Error::Timeout { operation: READ });
    }
    Ok(())
}

fn validate_response(
    issuance: &KagemushaWalletLoadIssuanceV1,
    payer: &iroha_data_model::account::AccountId,
    scheme: &[u8; 32],
    wallet: &[u8; 32],
    request: &[u8; 32],
) -> Result<()> {
    let mismatch = |field| Error::ResponseBinding {
        operation: READ,
        field,
    };
    if issuance.request_id != *request {
        return Err(mismatch("request_id"));
    }
    if &issuance.payer != payer {
        return Err(mismatch("payer"));
    }
    if issuance.body.scheme_id != *scheme || issuance.body.wallet_id != *wallet {
        return Err(mismatch("wallet scope"));
    }
    issuance
        .body
        .validate()
        .map_err(|_| mismatch("issuance body"))?;
    if let Some(raw) = &issuance.voucher {
        let voucher = KagemushaWalletLoadVoucherV1::decode_canonical(raw, scheme)
            .map_err(|_| mismatch("voucher original"))?;
        if voucher.body != issuance.body {
            return Err(mismatch("voucher body"));
        }
    }
    Ok(())
}
