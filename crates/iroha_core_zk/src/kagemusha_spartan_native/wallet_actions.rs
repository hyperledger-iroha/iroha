//! Parse public operation inputs. The selected slot supplies the private witness and head.
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletSlotIdV1 as Slot, KagemushaWalletSlotStatusV1 as Status,
    kagemusha_wallet_archive_object_digest_v1 as digest,
};
use iroha_data_model::{
    isi::kagemusha_wallet::load_finality::KAGEMUSHA_WALLET_LOAD_RECEIPT_MAX_BYTES_V1,
    kagemusha::kagemusha_wallet_v1::{
        KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1, KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1,
        KAGEMUSHA_WALLET_TIME_ANCHOR_MAX_BYTES_V1, KagemushaWalletLoadFinalityV1,
    },
};

fn invalid(field: &'static str) -> ProviderError {
    ProviderError::Invalid { field }
}

/// The complete CreditStatus IPM1 original a payer verifies (wallet wire v1 §§2 and 6:
/// 10,034 canonical bytes, 10,084 with the IPM1 header), as `verify_sent_credit_status`.
const ARCHIVE_SENT_STATUS_MAX_BYTES: usize = native::MAX_CREDIT_STATUS_ORIGINAL_BYTES;

pub(super) struct WalletInputs<'a>(&'a [u8]);

impl<'a> WalletInputs<'a> {
    pub(super) fn original(&mut self, maximum: usize) -> Result<&'a [u8], ProviderError> {
        let (word, rest) = self
            .0
            .split_at_checked(4)
            .ok_or_else(|| invalid("wallet action length"))?;
        let size = u32::from_le_bytes(word.try_into().unwrap()) as usize;
        if size == 0 || size > maximum {
            return Err(invalid("wallet action original size"));
        }
        let (bytes, trailing) = rest
            .split_at_checked(size)
            .ok_or_else(|| invalid("truncated wallet action"))?;
        self.0 = trailing;
        Ok(bytes)
    }

    pub(super) fn finish(self) -> Result<(), ProviderError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(invalid("wallet action trailing bytes"))
        }
    }
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    pub(super) fn prepare_wallet_action(
        &mut self,
        slot: &Slot,
        action: &[u8],
    ) -> Result<native::PreparedOperation, ProviderError> {
        let (&kind, bytes) = action
            .split_first()
            .ok_or_else(|| invalid("wallet action tag"))?;
        let mut inputs = WalletInputs(bytes);
        match kind {
            1 => {
                inputs.finish()?;
                self.wallet_bootstrap(slot)
            }
            2 => {
                let receipt = inputs.original(KAGEMUSHA_WALLET_LOAD_RECEIPT_MAX_BYTES_V1)?;
                let finality = inputs.original(KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1)?;
                inputs.finish()?;
                self.wallet_load(slot, receipt, finality, digest(action))
            }
            3 => {
                let open_request = inputs.original(12_182)?;
                inputs.finish()?;
                self.wallet_prepare_send(slot, open_request)
            }
            4 => {
                let request_id = inputs
                    .original(32)?
                    .try_into()
                    .map_err(|_| invalid("hosted Request id length"))?;
                let payment = inputs.original(12_182)?;
                inputs.finish()?;
                self.wallet_prepare_receive(slot, request_id, payment)
            }
            5 => {
                let (sent_operation, credit_status) = archive_sent_inputs(inputs)?;
                self.wallet_archive_sent(slot, &sent_operation, credit_status)
            }
            6 => {
                let request = UnloadRequest::parse(inputs)?;
                self.wallet_unload(slot, request, digest(action))
            }
            7 => {
                let (anchor, issuer) = time_anchor_inputs(inputs)?;
                self.wallet_refresh_time_anchor(slot, anchor, issuer, digest(action))
            }
            _ => Err(invalid("unknown wallet action")),
        }
    }

    fn wallet_bootstrap(
        &mut self,
        slot: &Slot,
    ) -> Result<native::PreparedOperation, ProviderError> {
        let expected = self.bootstrap_operation_id(slot)?;
        let capsule = match self.provider.status(slot)? {
            Status::Enrollment(marker) => {
                let [credential, issuer, _] = self.request_account_originals(slot)?;
                let witness = self
                    .engine
                    .bootstrap_from_enrollment(
                        &credential,
                        &issuer,
                        *marker.marker_digest(),
                        &self.parameters,
                    )
                    .map_err(provider_error)?;
                let prepared = self.prepare_private_witness(slot, &witness, &[])?;
                if prepared.capsule().operation_id() != &expected {
                    return Err(invalid("Bootstrap enrollment operation identity"));
                }
                return Ok(prepared);
            }
            Status::Pending(marker) => {
                let current = self.provider.capsule_at_output_marker(slot, &marker)?;
                current.require_marker(marker.marker())?;
                if current.operation_id() == &expected {
                    current
                } else {
                    self.provider.retained_transition(slot, &expected)?.0
                }
            }
            Status::Released(_) => self.provider.retained_transition(slot, &expected)?.0,
            Status::Terminal(_) => return Err(ProviderError::Terminal),
            _ => return Err(invalid("Bootstrap requires an enrolled slot")),
        };
        if capsule.kind() != 1 || capsule.operation_id() != &expected {
            return Err(invalid("retained Bootstrap identity"));
        }
        self.require_account_wallet(capsule.wallet())?;
        let prepared = self
            .engine
            .resume(&capsule, &self.parameters)
            .map_err(provider_error)?;
        self.require_account(slot, Some(prepared.credential()))?;
        Ok(prepared)
    }

    fn wallet_load(
        &mut self,
        slot: &Slot,
        receipt: &[u8],
        proof: &[u8],
        input: [u8; 32],
    ) -> Result<native::PreparedOperation, ProviderError> {
        self.require_account(slot, None)?;
        let key = digest(&[b"spartan-wallet-funded-Load-v1".as_slice(), receipt].concat());
        if let Some(prepared) = self.recover_transfer_action(slot, 2, &key, Some(&input), None)? {
            return Ok(prepared);
        }
        let wallet = self.authorized_wallet_id(slot)?;
        let epoch = KagemushaWalletLoadFinalityV1::decode_canonical(proof)
            .map_err(|_| invalid("Load finality original"))?
            .certificate
            .epoch_id()
            .map_err(|_| invalid("Load finality epoch"))?;
        let engine = self.engine_for_epoch(slot, wallet, Some(epoch))?;
        let Status::Released(marker) = self.provider.status(slot)? else {
            return Err(invalid("Load requires a released wallet"));
        };
        let (source, completed) = self.released_operation(slot, &marker)?;
        local_blacklist::check(
            &mut self.provider,
            slot,
            *self.parameters.manifest_digest(),
            wallet,
            None,
        )?;
        let (prepared, raw) = self.with_transfer_maps(slot, |_, parameters, maps| {
            let witness = engine.load_from_finality(
                &source,
                completed.head(),
                receipt,
                proof,
                |root, key, value| {
                    maps.insert(native::SpartanWalletMap::Recovery, *root, *key, *value)
                },
                parameters,
            )?;
            let raw = zeroize::Zeroizing::new(witness.encode()?);
            Ok((engine.prepare(&witness, completed.head(), parameters)?, raw))
        })?;
        self.retain_transfer_action(slot, key, &prepared, Some(&input), &raw)?;
        Ok(prepared)
    }

    fn wallet_archive_sent(
        &mut self,
        slot: &Slot,
        sent_operation: &[u8; 32],
        credit_status: &[u8],
    ) -> Result<native::PreparedOperation, ProviderError> {
        use crate::kagemusha_wallet_advance_v1::KagemushaWalletLookupV1 as Lookup;

        self.require_account(slot, None)?;
        self.parameters
            .require_archive_sent_program()
            .map_err(provider_error)?;
        let wallet = self.authorized_wallet_id(slot)?;
        let selected_send = match self.provider.lookup(slot, sent_operation)? {
            Lookup::Retained(row) => row.capsule_digest,
            Lookup::Archived(row) if row.kind == 3 => row.capsule_digest,
            _ => {
                return Err(ProviderError::UnavailableCustodyData {
                    object: "released Send",
                });
            }
        };
        let key = native::wallet_archive_sent_replay_key(&selected_send, &wallet, credit_status)
            .map_err(provider_error)?;
        if let Some(prepared) =
            self.recover_transfer_action(slot, 5, &key, Some(credit_status), None)?
        {
            return Ok(prepared);
        }
        let (sent, completion) = self.provider.retained_transition(slot, sent_operation)?;
        if sent.wallet() != &wallet
            || sent.kind() != 3
            || sent.operation_id() != sent_operation
            || completion.capsule_digest != selected_send
        {
            return Err(invalid("ArchiveSent released Send scope"));
        }
        let original = self
            .engine_for_capsule(slot, &sent)?
            .resume(&sent, &self.parameters)
            .map_err(provider_error)?;
        let request = self.retained_send_request(slot, &original)?.ok_or(
            ProviderError::UnavailableCustodyData {
                object: "released Send open Request",
            },
        )?;
        drop(original);
        let sent = zeroize::Zeroizing::new(sent.encode().map_err(provider_error)?);
        let Status::Released(marker) = self.provider.status(slot)? else {
            return Err(invalid("ArchiveSent requires a released wallet"));
        };
        let (source, completed) = self.released_operation(slot, &marker)?;
        let (prepared, raw) = self.with_transfer_maps(slot, |engine, parameters, maps| {
            let head = completed.head();
            let witness = engine.wallet_archive_sent_witness(
                &source,
                head,
                &sent,
                &completion.frame,
                &request,
                credit_status,
                maps,
                parameters,
            )?;
            let prepared = engine.prepare(&witness, head, parameters)?;
            let raw = zeroize::Zeroizing::new(witness.encode()?);
            Ok((prepared, raw))
        })?;
        self.retain_transfer_action(slot, key, &prepared, Some(credit_status), &raw)?;
        Ok(prepared)
    }

    fn wallet_unload(
        &mut self,
        slot: &Slot,
        request: UnloadRequest<'_>,
        input: [u8; 32],
    ) -> Result<native::PreparedOperation, ProviderError> {
        let account = self.payout_account(slot)?;
        let key = digest(&[b"spartan-wallet-Unload-request-v1".as_slice(), &request.id].concat());
        if let Some(prepared) = self.recover_transfer_action(slot, 6, &key, Some(&input), None)? {
            return Ok(prepared);
        }
        let destinations = request
            .destinations(account, &self.scheme)?
            .to_original()
            .map_err(provider_error)?;
        let Status::Released(marker) = self.provider.status(slot)? else {
            return Err(invalid("Unload requires a released wallet"));
        };
        let (source, completed) = self.released_operation(slot, &marker)?;
        let (prepared, raw) = self.with_transfer_maps(slot, |engine, parameters, maps| {
            let witness = engine.unload_from_released(
                &source,
                completed.head(),
                request.amount,
                &destinations,
                |root, key, value| {
                    maps.insert(native::SpartanWalletMap::Recovery, *root, *key, *value)
                },
                parameters,
            )?;
            let raw = zeroize::Zeroizing::new(witness.encode()?);
            Ok((engine.prepare(&witness, completed.head(), parameters)?, raw))
        })?;
        self.retain_transfer_action(slot, key, &prepared, Some(&input), &raw)?;
        Ok(prepared)
    }

    fn wallet_refresh_time_anchor(
        &mut self,
        slot: &Slot,
        anchor: &[u8],
        issuer: &[u8],
        input: [u8; 32],
    ) -> Result<native::PreparedOperation, ProviderError> {
        use iroha_data_model::kagemusha::{
            KagemushaWalletSignerCertificateV1, KagemushaWalletTimeAnchorV1,
        };
        self.require_account(slot, None)?;
        // The canonical originals and authentication of a finished direct exchange. An
        // imported anchor can raise only the signed floor; it records no local observation.
        let anchor: KagemushaWalletTimeAnchorV1 =
            decode_public(anchor, "RefreshTimeAnchor anchor")?;
        let issuer: KagemushaWalletSignerCertificateV1 =
            decode_public(issuer, "RefreshTimeAnchor issuer")?;
        anchor
            .verify(&self.scheme, &issuer)
            .map_err(|_| invalid("RefreshTimeAnchor signed anchor"))?;
        if anchor.body.wallet_id != self.authorized_wallet_id(slot)? {
            return Err(invalid("RefreshTimeAnchor wallet"));
        }
        let key = digest(
            &[
                b"spartan-wallet-RefreshTimeAnchor-v1".as_slice(),
                &anchor.time_anchor_digest(),
            ]
            .concat(),
        );
        if let Some(prepared) = self.recover_transfer_action(slot, 7, &key, Some(&input), None)? {
            return Ok(prepared);
        }
        let Status::Released(marker) = self.provider.status(slot)? else {
            return Err(invalid("RefreshTimeAnchor requires a released wallet"));
        };
        let (source, completed) = self.released_operation(slot, &marker)?;
        let mut original = anchor.body.transcript();
        original.extend_from_slice(anchor.signature.as_ref());
        let certificate =
            crate::kagemusha_wallet_enrollment_v1::spartan_signer_certificate_original_v1(&issuer);
        let witness = self
            .engine
            .wallet_refresh_time_anchor_witness(
                &source,
                completed.head(),
                &original,
                &certificate,
                &self.parameters,
            )
            .map_err(provider_error)?;
        let raw = zeroize::Zeroizing::new(witness.encode().map_err(provider_error)?);
        let prepared = self
            .engine
            .prepare(&witness, completed.head(), &self.parameters)
            .map_err(provider_error)?;
        self.retain_transfer_action(slot, key, &prepared, Some(&input), &raw)?;
        Ok(prepared)
    }
}

/// ArchiveSent (wallet wire v1 §3.3 tag 5): exactly the nonzero operation id of one of this
/// wallet's released Sends and the receiver's complete CreditStatus original. Native reads the
/// open Request, the Send capsule, completion and Payment only from protected custody.
fn archive_sent_inputs(mut inputs: WalletInputs<'_>) -> Result<([u8; 32], &[u8]), ProviderError> {
    let sent_operation: [u8; 32] = inputs
        .original(32)?
        .try_into()
        .map_err(|_| invalid("ArchiveSent Send operation length"))?;
    if sent_operation == [0; 32] {
        return Err(invalid("ArchiveSent Send operation"));
    }
    let credit_status = inputs.original(ARCHIVE_SENT_STATUS_MAX_BYTES)?;
    inputs.finish()?;
    Ok((sent_operation, credit_status))
}

/// The signed canonical anchor and its TimeAnchor-role issuer, within the direct-exchange bounds.
fn time_anchor_inputs(mut inputs: WalletInputs<'_>) -> Result<(&[u8], &[u8]), ProviderError> {
    let anchor = inputs.original(KAGEMUSHA_WALLET_TIME_ANCHOR_MAX_BYTES_V1)?;
    let issuer = inputs.original(KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1)?;
    inputs.finish()?;
    Ok((anchor, issuer))
}

struct UnloadRequest<'a> {
    id: [u8; 32],
    amount: u128,
    charge: Option<(&'a [u8], &'a [u8], &'a [u8])>,
}

fn decode_public<T>(bytes: &[u8], field: &'static str) -> Result<T, ProviderError>
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    use crate::kagemusha_wallet_state_v1 as state;
    norito::core::classify_decode_attempt(|| {
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
    })
    .map_err(
        |error| match state::decode_attempt_error(error, state::Error::Invalid(field)) {
            state::Error::Invalid(_) => invalid(field),
            error => super::custody_archive::storage_error(error),
        },
    )
}

impl<'a> UnloadRequest<'a> {
    fn parse(mut inputs: WalletInputs<'a>) -> Result<Self, ProviderError> {
        let id = inputs
            .original(32)?
            .try_into()
            .map_err(|_| invalid("Unload request id"))?;
        let amount = u128::from_le_bytes(
            inputs
                .original(16)?
                .try_into()
                .map_err(|_| invalid("Unload face amount"))?,
        );
        if id == [0; 32] || amount == 0 {
            return Err(invalid("Unload request id or face amount"));
        }
        let charge = if inputs.0.is_empty() {
            None
        } else {
            Some((
                inputs.original(1024)?,
                inputs
                    .original(iroha_data_model::kagemusha::KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)?,
                inputs
                    .original(crate::kagemusha_wallet_intake_v1::ACCOUNT_ORIGINAL_MAX_BYTES_V1)?,
            ))
        };
        inputs.finish()?;
        Ok(Self { id, amount, charge })
    }

    fn destinations(
        &self,
        account: iroha_data_model::account::AccountId,
        scheme: &iroha_data_model::kagemusha::KagemushaWalletSchemeV1,
    ) -> Result<native::UnloadDestinations, ProviderError> {
        use iroha_data_model::kagemusha::{
            KagemushaWalletCertificateSetV1, KagemushaWalletChargeQuoteV1,
            KagemushaWalletSignerRoleV1, KagemushaWalletUnloadChargeV1,
        };
        let (charge, quote_certificate) = if let Some((quote, certs, recipient)) = self.charge {
            use crate::kagemusha_wallet_state_v1 as state;
            let quote = state::read_model_original(
                || KagemushaWalletChargeQuoteV1::decode_canonical(quote, &scheme.scheme_id()),
                state::Error::Invalid("Unload charge quote"),
            )
            .map_err(super::custody_archive::storage_error)?;
            let set: KagemushaWalletCertificateSetV1 =
                decode_public(certs, "Unload charge certificates")?;
            set.validate()
                .map_err(|_| invalid("Unload charge certificates"))?;
            let issuer = set
                .certificate(
                    &quote.body.signer_certificate,
                    KagemushaWalletSignerRoleV1::RegulatoryPolicy,
                )
                .map_err(|_| invalid("Unload charge signer"))?;
            issuer
                .verify_role(scheme, KagemushaWalletSignerRoleV1::RegulatoryPolicy)
                .map_err(|_| invalid("Unload charge signer"))?;
            let certificate =
                crate::kagemusha_wallet_enrollment_v1::spartan_signer_certificate_original_v1(
                    issuer,
                );
            let beneficiary = decode_public(recipient, "Unload charge beneficiary")?;
            let digest =
                iroha_data_model::kagemusha::kagemusha_wallet_account_digest_v1(&beneficiary)
                    .map_err(|_| invalid("Unload charge beneficiary"))?;
            if digest != quote.body.beneficiary_account_digest {
                return Err(invalid("Unload charge beneficiary digest"));
            }
            (
                KagemushaWalletUnloadChargeV1::Quoted { quote, beneficiary },
                certificate.to_vec(),
            )
        } else {
            (KagemushaWalletUnloadChargeV1::None, Vec::new())
        };
        Ok(native::UnloadDestinations {
            account,
            charge,
            quote_certificate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unload_terms_preserve_unsigned_face_and_require_all_charge_originals() {
        fn append(bytes: &mut Vec<u8>, original: &[u8]) {
            bytes.extend_from_slice(&(original.len() as u32).to_le_bytes());
            bytes.extend_from_slice(original);
        }
        let mut wire = Vec::new();
        append(&mut wire, &[4; 32]);
        append(&mut wire, &u128::MAX.to_le_bytes());
        let plain = UnloadRequest::parse(WalletInputs(&wire)).unwrap();
        assert_eq!(plain.id, [4; 32]);
        assert_eq!(plain.amount, u128::MAX);
        assert!(plain.charge.is_none());
        append(&mut wire, &[7]);
        assert!(UnloadRequest::parse(WalletInputs(&wire)).is_err());
        append(&mut wire, &[8]);
        assert!(UnloadRequest::parse(WalletInputs(&wire)).is_err());
        append(&mut wire, &[9]);
        assert_eq!(
            UnloadRequest::parse(WalletInputs(&wire)).unwrap().charge,
            Some((&[7][..], &[8][..], &[9][..]))
        );
        append(&mut wire, &[10]);
        assert!(UnloadRequest::parse(WalletInputs(&wire)).is_err());
    }

    #[test]
    fn action_originals_are_bounded_and_must_consume_the_complete_input() {
        let mut valid = WalletInputs(&[2, 0, 0, 0, 11, 12, 1, 0, 0, 0, 13]);
        assert_eq!(valid.original(2).unwrap(), [11, 12]);
        assert_eq!(valid.original(1).unwrap(), [13]);
        valid.finish().unwrap();
        for bytes in [
            &[0, 0, 0][..],
            &[0, 0, 0, 0],
            &[1, 0, 0, 0],
            &[2, 0, 0, 0, 1, 2],
        ] {
            assert!(WalletInputs(bytes).original(1).is_err());
        }
        assert!(WalletInputs(&[0]).finish().is_err());
    }

    #[test]
    fn refresh_time_anchor_action_takes_exactly_two_bounded_signed_originals() {
        fn append(bytes: &mut Vec<u8>, original: &[u8]) {
            bytes.extend_from_slice(&(original.len() as u32).to_le_bytes());
            bytes.extend_from_slice(original);
        }
        // The framing both SDK overloads send after `KSO1 7`.
        let wire = [1, 0, 0, 0, 7, 2, 0, 0, 0, 8, 9];
        assert_eq!(
            time_anchor_inputs(WalletInputs(&wire)).unwrap(),
            (&[7][..], &[8, 9][..])
        );
        let mut longest = Vec::new();
        append(
            &mut longest,
            &[1; KAGEMUSHA_WALLET_TIME_ANCHOR_MAX_BYTES_V1],
        );
        append(
            &mut longest,
            &[2; KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1],
        );
        assert!(time_anchor_inputs(WalletInputs(&longest)).is_ok());
        let mut oversized = Vec::new();
        append(
            &mut oversized,
            &[1; KAGEMUSHA_WALLET_TIME_ANCHOR_MAX_BYTES_V1 + 1],
        );
        append(&mut oversized, &[2]);
        let mut trailing = wire.to_vec();
        append(&mut trailing, &[3]);
        for refused in [&wire[..5], &oversized[..], &trailing[..]] {
            assert!(time_anchor_inputs(WalletInputs(refused)).is_err());
        }
    }

    #[test]
    fn archive_sent_action_takes_one_send_operation_and_one_bounded_credit_status() {
        fn append(bytes: &mut Vec<u8>, original: &[u8]) {
            bytes.extend_from_slice(&(original.len() as u32).to_le_bytes());
            bytes.extend_from_slice(original);
        }
        fn refusal(bytes: &[u8]) -> &'static str {
            match archive_sent_inputs(WalletInputs(bytes)) {
                Err(ProviderError::Invalid { field }) => field,
                other => panic!("accepted malformed ArchiveSent input: {other:?}"),
            }
        }
        // The framing after `KSO1 5`: LE32 32 ‖ operation ‖ LE32 n ‖ CreditStatus.
        let mut wire = Vec::new();
        append(&mut wire, &[0x5a; 32]);
        append(&mut wire, &[7, 8, 9]);
        assert_eq!(wire.len(), 4 + 32 + 4 + 3);
        assert_eq!(
            archive_sent_inputs(WalletInputs(&wire)).unwrap(),
            ([0x5a; 32], &[7, 8, 9][..])
        );
        let mut longest = Vec::new();
        append(&mut longest, &[1; 32]);
        append(&mut longest, &[2; ARCHIVE_SENT_STATUS_MAX_BYTES]);
        let (_, status) = archive_sent_inputs(WalletInputs(&longest)).unwrap();
        assert_eq!(status.len(), ARCHIVE_SENT_STATUS_MAX_BYTES);

        // The operation id is exactly 32 nonzero bytes.
        let mut zero = Vec::new();
        append(&mut zero, &[0; 32]);
        append(&mut zero, &[7]);
        assert_eq!(refusal(&zero), "ArchiveSent Send operation");
        let mut short = Vec::new();
        append(&mut short, &[1; 31]);
        append(&mut short, &[7]);
        assert_eq!(refusal(&short), "ArchiveSent Send operation length");
        let mut long = Vec::new();
        append(&mut long, &[1; 33]);
        append(&mut long, &[7]);
        assert_eq!(refusal(&long), "wallet action original size");
        // The status is present, nonempty and within the payer's CreditStatus bound.
        assert_eq!(refusal(&wire[..36]), "wallet action length");
        let mut empty = wire[..36].to_vec();
        append(&mut empty, &[]);
        assert_eq!(refusal(&empty), "wallet action original size");
        let mut oversized = Vec::new();
        append(&mut oversized, &[1; 32]);
        append(&mut oversized, &[2; ARCHIVE_SENT_STATUS_MAX_BYTES + 1]);
        assert_eq!(refusal(&oversized), "wallet action original size");
        // A truncated original or any trailing byte, including a supplied Request, is refused.
        assert_eq!(refusal(&wire[..wire.len() - 1]), "truncated wallet action");
        let mut trailing = wire.clone();
        trailing.push(0);
        assert_eq!(refusal(&trailing), "wallet action trailing bytes");
        let mut supplied_request = wire.clone();
        append(&mut supplied_request, &[3; 64]);
        assert_eq!(refusal(&supplied_request), "wallet action trailing bytes");
        assert_eq!(refusal(&[]), "wallet action length");
    }
}
