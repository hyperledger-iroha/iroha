//! Complete-receipt broker backend over the exact durable hardware producer.
use super::{SignerOperationErrorV1, SignerStreamTokenErrorV1, SignerStreamTokenServiceV1};
use iroha_torii::sorafs::{
    StreamTokenHardwareCallErrorV1 as Error, StreamTokenHardwareClientV1,
    StreamTokenHardwareReceiptV1,
};
use sorafs_manifest::{StreamTokenBodyV1, signer::stream_token::SignerStreamTokenExpectedV1};

impl StreamTokenHardwareClientV1 for SignerStreamTokenServiceV1 {
    fn handle(&self) -> &str {
        &self.coordinator.binding.runtime_handle
    }

    fn sign(
        &self,
        expected: &SignerStreamTokenExpectedV1,
        body: &StreamTokenBodyV1,
    ) -> Result<StreamTokenHardwareReceiptV1, Error> {
        let payload = self.prepare_transport_payload(expected, body)?;
        // A later custody, journal or publication fence may fail after physical signing or CAS.
        // The transport has no proof of non-mutation then. Preserve the exact operation for
        // read-only recovery; never translate a post-invocation error into permission to re-sign.
        let receipt = SignerStreamTokenServiceV1::sign(self, &payload)
            .map_err(|_| Error::AmbiguousCompletion)?;
        StreamTokenHardwareReceiptV1::new(receipt.bytes().to_vec())
            .map_err(|_| Error::AmbiguousCompletion)
    }

    fn recover(
        &self,
        expected: &SignerStreamTokenExpectedV1,
        body: &StreamTokenBodyV1,
    ) -> Result<StreamTokenHardwareReceiptV1, Error> {
        let payload = self.prepare_transport_payload(expected, body)?;
        let receipt =
            SignerStreamTokenServiceV1::recover(self, &payload).map_err(|error| match error {
                SignerStreamTokenErrorV1::Journal
                | SignerStreamTokenErrorV1::Operation(SignerOperationErrorV1::StateUnavailable) => {
                    Error::Unavailable
                }
                SignerStreamTokenErrorV1::Receipt(_) => Error::InvalidResponse,
                SignerStreamTokenErrorV1::Operation(_) => Error::Refused,
            })?;
        StreamTokenHardwareReceiptV1::new(receipt.bytes().to_vec())
    }
}

impl SignerStreamTokenServiceV1 {
    fn prepare_transport_payload(
        &self,
        expected: &SignerStreamTokenExpectedV1,
        body: &StreamTokenBodyV1,
    ) -> Result<Vec<u8>, Error> {
        // The service's own immutable binding is the only authority for this derivation.
        // Expected/body transport arguments are never an alternate binding or key source.
        let actual = SignerStreamTokenExpectedV1::new(body, &self.coordinator.binding)
            .map_err(|_| Error::Refused)?;
        if &actual != expected {
            return Err(Error::Refused);
        }
        body.signing_payload_bytes().map_err(|_| Error::Refused)
    }
}
