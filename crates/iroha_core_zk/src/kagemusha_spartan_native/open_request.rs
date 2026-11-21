//! Receiver-issued open Requests with a one-use signing intent and exact durable delivery.
use super::custody_archive::{Checkpoint, Objects, storage_error};
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletSlotIdV1 as Slot, KagemushaWalletSlotStatusV1 as Status,
    kagemusha_wallet_archive_object_digest_v1 as digest,
};
use crate::kagemusha_wallet_state_v1::{IndexRoot, ObjectStore};
use iroha_data_model::kagemusha::{
    KagemushaDevicePublicKeyV1, KagemushaWalletSigningDomainV1 as Domain,
    kagemusha_wallet_signing_message_v1, kagemusha_wallet_verify_signature_v1,
};
use rand::rand_core::TryRng as _;

const PLAN_LIMIT: usize = 8 * 1024;
const WIRE_LIMIT: usize = 12_182;

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.spartan.OpenRequestIntentV1")]
struct Plan {
    id: [u8; 32],
    action: [u8; 32],
    source: [u8; 32],
    body: Vec<u8>,
    credential: Vec<u8>,
    certificates: Vec<u8>,
    schedule: Vec<u8>,
    account: Vec<u8>,
    signing_started: bool,
    output: Option<[u8; 32]>,
}

fn invalid(field: &'static str) -> ProviderError {
    ProviderError::Invalid { field }
}
fn lost(object: &'static str) -> ProviderError {
    ProviderError::UnavailableCustodyData { object }
}

fn action_digest(amount: u128, fee: &[u8], issuer: &[u8]) -> Result<[u8; 32], ProviderError> {
    if amount == 0 || !matches!((fee.len(), issuer.len()), (0, 0) | (255, 172)) {
        return Err(invalid("future Request amount or fee originals"));
    }
    Ok(digest(
        &[
            b"future-open-request-action-v1".as_slice(),
            &amount.to_le_bytes(),
            fee,
            issuer,
        ]
        .concat(),
    ))
}
impl Plan {
    fn key(&self) -> Result<KagemushaDevicePublicKeyV1, ProviderError> {
        if self.body.len() != 418 || self.credential.len() != 596 || self.body[386..418] == [0; 32]
        {
            return Err(lost("future Request plan"));
        }
        KagemushaDevicePublicKeyV1::try_from(&self.credential[130..195])
            .map_err(|_| lost("future Request key"))
    }
    fn signature_address(&self) -> [u8; 32] {
        digest(
            &[
                b"future-open-request-signature-v1".as_slice(),
                &self.id,
                &self.source,
                &self.body,
            ]
            .concat(),
        )
    }
    fn wire(&self, signature: &[u8]) -> Result<Vec<u8>, ProviderError> {
        let key = self.key()?;
        let signature = KagemushaDeviceSignatureV1::from_raw_bytes(signature)
            .map_err(|_| lost("future Request signature encoding"))?;
        let signed = kagemusha_wallet_signing_message_v1(Domain::Request, &self.body);
        kagemusha_wallet_verify_signature_v1(&key, Domain::Request, &signed, &signature)
            .map_err(|_| lost("future Request retained hardware signature"))?;
        let request = [self.body.as_slice(), signature.as_raw_bytes()].concat();
        native::encode_open_request_peer(
            &request,
            &self.credential,
            &self.certificates,
            &self.schedule,
            &self.account,
        )
        .map_err(provider_error)
    }
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    fn read_request_plan(
        &mut self,
        slot: &Slot,
        root: [u8; 32],
        id: &[u8; 32],
    ) -> Result<Option<Plan>, ProviderError> {
        self.provider.with_archive(slot, |archive| {
            let mut store = Objects(archive);
            let Some(address) = IndexRoot(root).get(&mut store, id).map_err(storage_error)? else {
                return Ok(None);
            };
            let address = address
                .try_into()
                .map_err(|_| lost("future Request plan address"))?;
            let bytes = store
                .read_object(&address, PLAN_LIMIT)
                .map_err(storage_error)?;
            let plan: Plan = norito::core::classify_decode_attempt(|| {
                norito::decode_canonical_with_limits(
                    &bytes,
                    norito::canonical_decode_limits(bytes.len()),
                )
            })
            .map_err(|error| {
                storage_error(crate::kagemusha_wallet_state_v1::decode_attempt_error(
                    error,
                    crate::kagemusha_wallet_state_v1::Error::WitnessLost("future Request plan"),
                ))
            })?;
            if &plan.id != id {
                return Err(lost("future Request plan identity"));
            }
            plan.key()?;
            Ok(Some(plan))
        })
    }
    fn publish_request_plan(
        &mut self,
        slot: &Slot,
        expected: [u8; 32],
        checkpoint: &mut Checkpoint,
        plan: &Plan,
    ) -> Result<(), ProviderError> {
        let bytes =
            norito::encode_canonical(plan).map_err(|_| invalid("future Request plan encoding"))?;
        checkpoint.requests = self.provider.with_archive(slot, |archive| {
            let mut store = Objects(archive);
            let address = store
                .write_object(&bytes, PLAN_LIMIT)
                .map_err(storage_error)?;
            Ok(IndexRoot(checkpoint.requests)
                .set(&mut store, plan.id, &address)
                .map_err(storage_error)?
                .0)
        })?;
        self.publish_custody_checkpoint(slot, expected, checkpoint)
    }

    pub(super) fn issued_request_wire(
        &mut self,
        slot: &Slot,
        request_id: &[u8; 32],
    ) -> Result<Vec<u8>, ProviderError> {
        self.require_account(slot, None)?;
        let wallet = self.authorized_wallet_id(slot)?;
        let (_, checkpoint) = self.custody_checkpoint(slot, wallet)?;
        let plan = self
            .read_request_plan(slot, checkpoint.requests, request_id)?
            .ok_or_else(|| invalid("future CreditStatus Request id"))?;
        let address = plan
            .output
            .ok_or_else(|| lost("future CreditStatus unsigned Request"))?;
        self.provider.with_archive(slot, |archive| {
            Objects(archive)
                .read_object(&address, WIRE_LIMIT)
                .map_err(storage_error)
        })
    }

    /// Construct a peer CreditStatus solely from this receiver's released operation,
    /// its original incoming Payment and the previously emitted exact open Request.
    /// This path reads custody and verifies the proof without selecting any operation.
    ///
    /// # Errors
    /// Requires the matching issued Request and released Receive, a valid Payment and an
    /// authenticated credit opening that fits the supported peer transport.
    pub fn credit_status_peer(
        &mut self,
        slot: &Slot,
        request_id: &[u8; 32],
        receive_operation: &[u8; 32],
    ) -> Result<Vec<u8>, ProviderError> {
        self.receiver_credit_status(slot, request_id, receive_operation, None)
    }

    /// Issue read-only delivery evidence from the original released Receive or a later
    /// released Send by the receiver whose authenticated credit map covers the supplied Payment.
    ///
    /// # Errors
    /// Rejects a different Payment original as well as any invalid Request, release or proof.
    pub fn credit_status_for_payment(
        &mut self,
        slot: &Slot,
        request_id: &[u8; 32],
        head_operation: &[u8; 32],
        payment: &[u8],
    ) -> Result<Vec<u8>, ProviderError> {
        self.receiver_credit_status(slot, request_id, head_operation, Some(payment))
    }

    fn receiver_credit_status(
        &mut self,
        slot: &Slot,
        request_id: &[u8; 32],
        head_operation: &[u8; 32],
        payment: Option<&[u8]>,
    ) -> Result<Vec<u8>, ProviderError> {
        self.require_account(slot, None)?;
        let wallet = self.authorized_wallet_id(slot)?;
        let request = self.issued_request_wire(slot, request_id)?;
        let (capsule, retained) = self.provider.retained_transition(slot, head_operation)?;
        if capsule.wallet() != &wallet
            || capsule.operation_id() != head_operation
            || retained.record.wallet() != &wallet
        {
            return Err(invalid("CreditStatus released head scope"));
        }
        if capsule.kind() == 3 {
            let original = payment.ok_or(invalid(
                "later Send CreditStatus requires exact original Payment",
            ))?;
            let original = self.verify_payment(original).map_err(provider_error)?;
            let (_, released) = self.verify_retained_release(slot, &capsule, &retained)?;
            let bytes = released
                .completion()
                .output_part(1)
                .map_err(provider_error)?;
            let later = self.verify_payment(&bytes).map_err(provider_error)?;
            if released.head().profile() != later.profile() {
                return Err(invalid("later Send CreditStatus release binding"));
            }
            return self.with_transfer_maps(slot, |engine, parameters, maps| {
                engine.encode_credit_status_at_send_head(
                    &later, &original, &request, maps, parameters,
                )
            });
        }
        if capsule.kind() != 4 {
            return Err(invalid(
                "CreditStatus requires released Receive or covering Send",
            ));
        }
        let proof = match payment {
            Some(original) => {
                let verified = self.verify_payment(original).map_err(provider_error)?;
                self.engine.encode_credit_status_peer(
                    &capsule, &retained.record, &request, &verified, &self.parameters,
                )
            }
            None => self.engine.encode_retained_credit_status_peer(
                &capsule, &retained.record, &request, &self.parameters,
            ),
        };
        proof.map_err(provider_error)
    }

    /// Verify a receiver's proof against the exact outgoing Payment released by this payer.
    /// A verified delivery status cannot authorize a second transfer or a refund.
    ///
    /// # Errors
    /// Requires the local admitted account and a completed matching Send. An empty Request
    /// reads the exact peer original from the wallet-owned action. Refuses a changed Request,
    /// a different payer or receiver, malformed proofs and missing retained originals.
    pub fn verify_sent_credit_status(
        &mut self,
        slot: &Slot,
        sent_operation: &[u8; 32],
        open_request: &[u8],
        status: &[u8],
    ) -> Result<native::VerifiedCreditStatus, ProviderError> {
        self.require_account(slot, None)?;
        let wallet = self.authorized_wallet_id(slot)?;
        let (capsule, retained) = self.provider.retained_transition(slot, sent_operation)?;
        if capsule.wallet() != &wallet
            || capsule.kind() != 3
            || capsule.operation_id() != sent_operation
            || retained.record.wallet() != &wallet
        {
            return Err(invalid("future CreditStatus released Send scope"));
        }
        let (prepared, released) = self
            .engine
            .verify_released_operation(&capsule, &retained.record, &self.parameters)
            .map_err(provider_error)?;
        let original = self.retained_send_request(slot, &prepared)?;
        let open_request = match (original.as_deref(), open_request.is_empty()) {
            (Some(original), false) if original != open_request => {
                return Err(invalid(
                    "released Send Request differs from supplied original",
                ));
            }
            (Some(original), _) => original,
            (None, true) => return Err(lost("released Send open Request original")),
            (None, false) => {
                prepared.verify_recovered_send_request(open_request).map_err(provider_error)?;
                open_request
            }
        };
        self.engine
            .verify_sent_delivery(&prepared, &released, open_request, status, &self.parameters)
            .map(|delivery| delivery.outcome())
            .map_err(provider_error)
    }

    /// Issue a hardware-signed open Request from the current Released opening for a nonzero
    /// amount. Retrying the same id returns the exact original even after a later head.
    ///
    /// # Errors
    /// Refuses a changed action, unavailable source, a restricted account, invalid fee
    /// originals, or a previous signing attempt whose result cannot be recovered.
    pub fn issue_open_request(
        &mut self,
        slot: &Slot,
        id: [u8; 32],
        amount: u128,
        fee: &[u8],
        issuer: &[u8],
    ) -> Result<Vec<u8>, ProviderError> {
        self.require_account(slot, None)?;
        if id == [0; 32] {
            return Err(invalid("future Request id"));
        }
        let action = action_digest(amount, fee, issuer)?;
        let wallet = self.authorized_wallet_id(slot)?;
        let (mut expected, mut checkpoint) = self.custody_checkpoint(slot, wallet)?;
        let prior = self.read_request_plan(slot, checkpoint.requests, &id)?;
        if let Some(prior) = &prior {
            if prior.action != action {
                return Err(invalid("future Request id already names another action"));
            }
            if let Some(address) = prior.output {
                return self.provider.with_archive(slot, |archive| {
                    Objects(archive)
                        .read_object(&address, WIRE_LIMIT)
                        .map_err(storage_error)
                });
            }
        }
        let mut plan = if prior.as_ref().is_some_and(|p| p.signing_started) {
            prior.ok_or_else(|| lost("future Request intent"))?
        } else {
            let required = crate::kagemusha_wallet_state_v1::open_request_storage_required(
                PLAN_LIMIT as u64,
                WIRE_LIMIT as u64,
            );
            if self.provider.runtime_resources()?.storage_bytes < required {
                return Err(ProviderError::NoSpace);
            }
            let Status::Released(marker) = self.provider.status(slot)? else {
                return Err(invalid("future Request requires Released"));
            };
            let (prepared, completed) = self.released_operation(slot, &marker)?;
            let [credential, enrollment, account] = self.request_account_originals(slot)?;
            if account.len() > 2048 {
                return Err(invalid("future Request account companion bound"));
            }
            self.engine
                .authenticate_credential_originals(&credential, &enrollment, &self.parameters)
                .map_err(provider_error)?;
            let source = marker
                .head()
                .ok_or_else(|| lost("future Request Released head"))?
                .2;
            let mut nonce = [0; 32];
            if let Some(saved) = prior.as_ref().filter(|p| p.source == source) {
                nonce.copy_from_slice(&saved.body[386..418]);
            } else {
                rand::rngs::SysRng
                    .try_fill_bytes(&mut nonce)
                    .map_err(|_| invalid("future Request nonce entropy"))?;
            }
            let (body, certificates) = self
                .engine
                .open_request_terms(
                    &prepared,
                    completed.head(),
                    &enrollment,
                    fee,
                    issuer,
                    amount,
                    nonce,
                    &self.parameters,
                )
                .map_err(provider_error)?;
            native::OpenRequestPeer {
                request: &body,
                receiver_credential: &credential,
                certificates: &certificates,
                fee_schedule: fee,
                destination_account: &account,
            }
            .validate_for_issuance()
            .map_err(provider_error)?;
            let incoming = Plan {
                id,
                action,
                source,
                body,
                credential,
                certificates,
                schedule: fee.to_vec(),
                account,
                signing_started: false,
                output: None,
            };
            if let Some(saved) = prior.as_ref().filter(|p| p.source == source) {
                if saved.body != incoming.body
                    || saved.credential != incoming.credential
                    || saved.certificates != incoming.certificates
                    || saved.schedule != incoming.schedule
                    || saved.account != incoming.account
                {
                    return Err(lost(
                        "future Request intent differs from authenticated originals",
                    ));
                }
            } else {
                self.publish_request_plan(slot, expected, &mut checkpoint, &incoming)?;
                (expected, checkpoint) = self.custody_checkpoint(slot, wallet)?;
                let saved = self
                    .read_request_plan(slot, checkpoint.requests, &id)?
                    .ok_or_else(|| lost("future Request unsigned intent readback"))?;
                if saved.source != source
                    || saved.body != incoming.body
                    || saved.signing_started
                    || saved.action != action
                    || saved.output.is_some()
                {
                    return Err(lost("future Request unsigned intent readback"));
                }
            }
            incoming
        };
        let address = plan.signature_address();
        let signature = if !plan.signing_started {
            let Status::Released(marker) = self.provider.status(slot)? else {
                return Err(invalid("future Request cannot sign from another head"));
            };
            if marker.head().map(|(_, _, source)| source) != Some(plan.source)
                || marker.payment_key() != &plan.key()?
            {
                return Err(invalid("future Request unsigned source changed"));
            }
            // An unsigned plan may survive a crash and still sign once. The following
            // checkpoint distinguishes it from the narrow interval of uncertain hardware I/O.
            self.provider
                .preflight_setup_signing(slot, plan.source, &plan.key()?)?;
            plan.signing_started = true;
            self.publish_request_plan(slot, expected, &mut checkpoint, &plan)?;
            (expected, checkpoint) = self.custody_checkpoint(slot, wallet)?;
            let started = self
                .read_request_plan(slot, checkpoint.requests, &id)?
                .ok_or_else(|| lost("future Request signing intent"))?;
            if !started.signing_started
                || started.signature_address() != address
                || started.output.is_some()
            {
                return Err(lost("future Request signing intent readback"));
            }
            self.provider.sign_setup_to_archive(
                slot,
                plan.source,
                &plan.key()?,
                &plan.body,
                &address,
            )?
        } else {
            // A process may have died while a hardware signing call was in flight. Only a
            // durably staged result may be adopted. A missing signature never triggers another.
            self.provider.with_archive(slot, |archive| {
                archive
                    .read_record(&address, 64)?
                    .map(|bytes| bytes.as_slice().to_vec())
                    .ok_or_else(|| lost("future Request signing result"))
            })?
        };
        let wire = plan.wire(&signature)?;
        let address = self.provider.with_archive(slot, |archive| {
            Objects(archive)
                .write_object(&wire, WIRE_LIMIT)
                .map_err(storage_error)
        })?;
        plan.output = Some(address);
        self.publish_request_plan(slot, expected, &mut checkpoint, &plan)?;
        self.provider.with_archive(slot, |archive| {
            Objects(archive)
                .read_object(&address, WIRE_LIMIT)
                .map_err(storage_error)
        })
    }
}
