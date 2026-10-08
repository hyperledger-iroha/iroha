//! Bounded account-authenticated event path transport; native finality admits its meaning.

use super::{ActivationEvidenceReadAuth, Client, Kagemusha, ensure_deadline};
use crate::{Error, Result, client::dispatch, http::StatusCode};
use iroha_crypto::MerkleProof;
use iroha_data_model::events::EventBox;

const OP: &str = "kagemusha.wallet.load_event_proof.read";
const MAX_BYTES: usize = 8_192;

impl Kagemusha<'_> {
    /// Retrieve one Load event inclusion path under this account's exact network signature.
    ///
    /// This performs one bounded canonical Norito GET for nonzero scheme, wallet and
    /// request identities, with no alternate codec, compatibility probe or automatic retry.
    /// The response is DATA: a path alone binds neither a payer nor a receipt. Independently
    /// verify it against the counted event commitment of a native-finality-admitted block
    /// and the exact retained receipt before constructing Load proof evidence.
    ///
    /// # Errors
    /// Rejects invalid identities or a non-direct signer, elapsed deadlines, transport/HTTP
    /// failures, oversized/noncanonical responses or a path exceeding the 32-level limit.
    pub async fn load_event_proof(
        &self,
        scheme: &[u8; 32],
        wallet: &[u8; 32],
        request: &[u8; 32],
    ) -> Result<MerkleProof<EventBox>> {
        let client = &self.account.context;
        if [scheme, wallet, request]
            .into_iter()
            .any(|id| id == &[0; 32])
        {
            return Err(Error::InvalidRequest {
                operation: OP,
                details: "load identities must be nonzero".to_owned(),
            });
        }
        if client.account.controller.single_signatory() != Some(client.key_pair.public_key())
            || client
                .headers
                .keys()
                .any(|name| name.eq_ignore_ascii_case("X-Iroha-Witness"))
        {
            return Err(Error::InvalidRequest {
                operation: OP,
                details: "load reads require the direct payer signer without witness headers"
                    .to_owned(),
            });
        }
        ensure_deadline(client, OP)?;
        let path = iroha_torii_shared::route_catalog::contracts_and_verification_keys::KAGEMUSHA_LOAD_EVENT_PROOF_GET
            .path()
            .replace("{scheme}", &hex::encode(scheme))
            .replace("{wallet}", &hex::encode(wallet))
            .replace("{request}", &hex::encode(request));
        let builder = client
            .canonical_norito_get_request(&path, MAX_BYTES, ActivationEvidenceReadAuth::Account)
            .map_err(|error| Error::RequestSigning {
                operation: OP,
                details: error.to_string(),
            })?;
        let response = dispatch::send(client, OP, builder, "application/x-norito").await?;
        if response.status() != StatusCode::OK {
            return Err(Error::Http {
                operation: OP,
                status: response.status().as_u16(),
                retry_after: crate::error::retry_after(response.headers()),
                body: response.into_body(),
            });
        }
        let proof: MerkleProof<EventBox> =
            Client::decode_canonical_norito_response(&response, MAX_BYTES, OP)?;
        if proof.audit_path().len() > 32 {
            return Err(Error::Decode {
                operation: OP,
                details: "Load event path exceeds 32 levels".to_owned(),
            });
        }
        ensure_deadline(client, OP)?;
        Ok(proof)
    }
}
