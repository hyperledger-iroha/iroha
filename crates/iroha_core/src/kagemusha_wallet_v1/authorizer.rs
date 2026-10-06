//! Narrow online load signer. Its public entrypoint accepts only a source-verified finalized
//! ledger cut; it cannot sign caller-supplied bodies, enrollment objects or arbitrary messages.
use super::*;
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
mod worker;
pub use worker::{
    LOAD_AUTHORIZER_KEYRING_MAX_BYTES, LOAD_AUTHORIZER_MAX_KEYS, LoadAuthorizerKeyV1,
    LoadAuthorizerKeyringV1, PublicationWorker,
};

/// Role-separated software custodian for one ledger-recorded finalized-load signing key.
/// Key loading belongs to the node's explicit configuration/secret owner; keys are never
/// serialized here. This provides no offline handset custody authority.
pub struct LoadAuthorizer {
    scheme: KagemushaWalletSchemeV1,
    certificate: KagemushaWalletSignerCertificateV1,
    key: SigningKey,
}
impl LoadAuthorizer {
    /// Bind an actual private key to one root-certified LoadAuthorization role and scheme.
    ///
    /// # Errors
    /// Rejects another role, another scheme, an invalid root signature, or a different key.
    pub fn new(
        scheme: KagemushaWalletSchemeV1,
        certificate: KagemushaWalletSignerCertificateV1,
        key: SigningKey,
    ) -> Result<Self> {
        certificate.verify_role(&scheme, KagemushaWalletSignerRoleV1::LoadAuthorization)?;
        let public = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )?;
        if certificate.body.key != public {
            return Err(Error::Binding);
        }
        Ok(Self {
            scheme,
            certificate,
            key,
        })
    }
    /// Prepare a voucher only from the original finalized issuance. The result exposes only
    /// a publication command, not an API response claiming durable completion.
    ///
    /// # Errors
    /// Rejects missing finality/issuance, another scheme, or a historical signer mismatch.
    pub fn prepare(
        &self,
        source: &FinalizedLedger<'_, '_>,
        wallet: Digest,
        request: Digest,
    ) -> Result<PreparedVoucher> {
        let scheme = self.scheme.scheme_id();
        let issuance = source.issuance(&scheme, &wallet, &request)?;
        self.prepare_issuance(issuance)
    }
    fn prepare_issuance(&self, issuance: Issuance) -> Result<PreparedVoucher> {
        if issuance.body.scheme_id != self.scheme.scheme_id()
            || issuance.body.authorizer_certificate != self.certificate.certificate_digest()
            || issuance.command.scheme != issuance.body.scheme_id
            || issuance.command.wallet != issuance.body.wallet_id
        {
            return Err(Error::Binding);
        }
        let voucher = if let Some(bytes) = &issuance.voucher {
            let retained =
                KagemushaWalletLoadVoucherV1::decode_canonical(bytes, &issuance.body.scheme_id)?;
            if retained.body != issuance.body {
                return Err(Error::Binding);
            }
            retained.verify(&self.scheme, &self.certificate)?;
            retained
        } else {
            // Deterministic RFC6979 and canonical signature normalization give stable initial
            // preparation; once published, the exact retained encoding always takes precedence.
            let signature: Signature = self.key.sign(&issuance.body.signing_message());
            KagemushaWalletLoadVoucherV1::sign(
                issuance.body,
                &self.certificate,
                KagemushaWalletSignerOutputV1::Der(signature.to_der().as_bytes()),
            )?
        };
        Ok(PreparedVoucher {
            scheme: self.scheme.scheme_id(),
            wallet: issuance.command.wallet,
            request: issuance.command.request_id,
            voucher,
        })
    }
}

/// Source-bound pending publication. No codec, public fields, or arbitrary-body constructor.
/// The client response is obtained separately from a verified finalized ledger generation.
pub struct PreparedVoucher {
    scheme: Digest,
    wallet: Digest,
    request: Digest,
    voucher: KagemushaWalletLoadVoucherV1,
}
impl PreparedVoucher {
    /// Produce the closed command for the configured online submitter to sign and submit.
    /// Submission is not completion: only a finalized ledger read releases the retained result.
    ///
    /// # Errors
    /// Rejects a voucher that cannot encode canonically.
    pub fn into_publication_instruction(
        self,
    ) -> Result<iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLedgerV1> {
        use iroha_data_model::isi::kagemusha_wallet::{
            KagemushaWalletLedgerActionV1, KagemushaWalletLedgerV1,
        };
        Ok(KagemushaWalletLedgerV1::new(
            self.scheme,
            KagemushaWalletLedgerActionV1::PublishVoucher {
                request_id: self.request,
                voucher: self.voucher.to_canonical_bytes()?,
            },
        ))
    }
    /// Revalidate the original source and retain the first canonical voucher before release.
    /// The transaction owner must make its successful batch durable before returning bytes
    /// to a client. A later failed transport retries the original retained result.
    ///
    /// # Errors
    /// Rejects unavailable/changed source, changed signer bindings, or failed persistence.
    pub fn publish(
        self,
        tx: &mut impl Transaction,
        source: &FinalizedLedger<'_, '_>,
    ) -> Result<Vec<u8>> {
        publish_voucher(
            tx,
            source,
            self.scheme,
            self.wallet,
            self.request,
            &self.voucher,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Memory, alternative_voucher, certificate, signing};
    use super::*;
    #[test]
    fn signer_role_and_private_key_are_exact() {
        let memory = Memory::new();
        let r = &memory.registration;
        assert!(LoadAuthorizer::new(r.scheme, r.load_authorizer, signing(0x34)).is_ok());
        assert!(LoadAuthorizer::new(r.scheme, r.load_authorizer, signing(0x35)).is_err());
        let wrong = certificate(&r.scheme, KagemushaWalletSignerRoleV1::Enrollment, 0x34);
        assert!(LoadAuthorizer::new(r.scheme, wrong, signing(0x34)).is_err());
    }
    #[test]
    fn deterministic_preparation_preserves_original_body_and_historical_signer() {
        let mut memory = Memory::new();
        let command = memory.active();
        let issuance = issue_load(&mut memory, &command).unwrap();
        let r = &memory.registration;
        let signer = LoadAuthorizer::new(r.scheme, r.load_authorizer, signing(0x34)).unwrap();
        let first = signer.prepare_issuance(issuance.clone()).unwrap();
        let second = signer.prepare_issuance(issuance.clone()).unwrap();
        assert_eq!(
            first.voucher.to_canonical_bytes().unwrap(),
            second.voucher.to_canonical_bytes().unwrap()
        );
        assert_eq!(first.voucher.body, issuance.body);
        first.voucher.verify(&r.scheme, &r.load_authorizer).unwrap();
        let command = second.into_publication_instruction().unwrap();
        assert_eq!(command.scheme, issuance.body.scheme_id);
        let iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLedgerActionV1::PublishVoucher { request_id, voucher } = command.action else { panic!("closed publication action") };
        assert_eq!(request_id, issuance.command.request_id);
        assert_eq!(
            KagemushaWalletLoadVoucherV1::decode_canonical(&voucher, &command.scheme)
                .unwrap()
                .body,
            issuance.body
        );
        let mut changed = issuance;
        let alternate = alternative_voucher(&changed, &memory);
        changed.voucher = Some(alternate.to_canonical_bytes().unwrap());
        assert_eq!(
            signer.prepare_issuance(changed.clone()).unwrap().voucher,
            alternate
        );
        changed.body.authorizer_certificate = [8; 32];
        assert!(signer.prepare_issuance(changed).is_err());
    }
}
