use super::*;

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
};

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    block::BlockHeader,
    domain::DomainId,
    offline::{
        offline_cash_receiver_key_reference_v1, OfflineCashPairedProofV1,
        OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
    },
};
use p256::ecdsa::{signature::Signer as _, Signature, SigningKey};
use sha2::Sha256;

const DEVICE_ID: Digest = [0x31; 32];
const POLICY_ID: Digest = [0x32; 32];
const RELEASE_ID: Digest = [0x33; 32];
const ISSUED_AT_MS: u64 = 10_000;
const EXPIRES_AT_MS: u64 = 70_000;
const TEST_OUTBOX_AUTHENTICATOR_DOMAIN: &[u8] = b"iroha:offline-cash:v1:test-outbox-authenticator";

fn signing_key() -> SigningKey {
    SigningKey::from_bytes((&[0x27_u8; 32]).into()).expect("P-256 signing key")
}

fn receiver_public_key() -> KagemushaDevicePublicKeyV2 {
    let encoded = signing_key().verifying_key().to_encoded_point(false);
    KagemushaDevicePublicKeyV2::from_sec1_bytes(encoded.as_bytes()).expect("receiver public key")
}

fn sign(bytes: &[u8]) -> KagemushaDeviceSignatureV2 {
    let signature: Signature = signing_key().sign(bytes);
    let signature = signature.normalize_s().unwrap_or(signature);
    KagemushaDeviceSignatureV2::from_raw_bytes(signature.to_bytes().as_ref())
        .expect("canonical signature")
}

fn hash_bytes(bytes: &[u8]) -> Digest {
    Sha256::digest(bytes).into()
}

#[derive(Clone)]
struct OutboxRecord {
    key: PaymentOutboxKeyV1,
    payment_digest: Digest,
    canonical_payment: Zeroizing<Vec<u8>>,
    publication_digest: Option<Digest>,
    authenticator: Digest,
}

#[derive(Default)]
struct TestOutbox {
    available: AtomicBool,
    fail_publish_once: AtomicBool,
    records: Mutex<BTreeMap<Digest, OutboxRecord>>,
}

impl TestOutbox {
    fn new() -> Self {
        Self {
            available: AtomicBool::new(true),
            fail_publish_once: AtomicBool::new(false),
            records: Mutex::new(BTreeMap::new()),
        }
    }

    fn check_available(&self) -> Result<(), AuthenticatedPaymentOutboxErrorV1> {
        if self.available.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(AuthenticatedPaymentOutboxErrorV1::Unavailable)
        }
    }

    fn mutate_staged_byte(&self, key: Digest) {
        let mut records = self.records.lock().expect("outbox lock");
        records
            .get_mut(&key)
            .expect("staged outbox record")
            .canonical_payment[0] ^= 1;
    }

    fn replace_staged_bytes_authenticated(&self, key: Digest, bytes: Vec<u8>) {
        let mut records = self.records.lock().expect("outbox lock");
        let record = records.get_mut(&key).expect("staged outbox record");
        record.canonical_payment = Zeroizing::new(bytes);
        record.authenticator = Self::authenticator(
            &record.key,
            record.payment_digest,
            &record.canonical_payment,
            record.publication_digest,
        );
    }

    fn fail_next_publish(&self) {
        self.fail_publish_once.store(true, Ordering::SeqCst);
    }

    fn authenticator(
        key: &PaymentOutboxKeyV1,
        payment_digest: Digest,
        canonical_payment: &[u8],
        publication_digest: Option<Digest>,
    ) -> Digest {
        let publication = publication_digest.unwrap_or([0; 32]);
        digest_framed(
            TEST_OUTBOX_AUTHENTICATOR_DOMAIN,
            &[
                &key.digest(),
                &payment_digest,
                canonical_payment,
                &publication,
            ],
        )
    }

    fn record_is_authenticated(record: &OutboxRecord) -> bool {
        record.authenticator
            == Self::authenticator(
                &record.key,
                record.payment_digest,
                &record.canonical_payment,
                record.publication_digest,
            )
    }
}

impl sealed::Sealed for TestOutbox {}

impl AuthenticatedPaymentOutboxBackendV1 for TestOutbox {
    fn stage_payment_or_recover(
        &self,
        key: &PaymentOutboxKeyV1,
        payment_digest: Digest,
        canonical_payment: &[u8],
    ) -> Result<(), AuthenticatedPaymentOutboxErrorV1> {
        self.check_available()?;
        if payment_digest == [0; 32] || canonical_payment.is_empty() {
            return Err(AuthenticatedPaymentOutboxErrorV1::Corrupt);
        }
        let mut records = self.records.lock().expect("outbox lock");
        if let Some(existing) = records.get(&key.digest()) {
            if Self::record_is_authenticated(existing)
                && existing.key == *key
                && existing.payment_digest == payment_digest
                && existing.canonical_payment.as_slice() == canonical_payment
            {
                return Ok(());
            }
            return Err(AuthenticatedPaymentOutboxErrorV1::Conflict);
        }
        records.insert(
            key.digest(),
            OutboxRecord {
                key: *key,
                payment_digest,
                canonical_payment: Zeroizing::new(canonical_payment.to_vec()),
                publication_digest: None,
                authenticator: Self::authenticator(key, payment_digest, canonical_payment, None),
            },
        );
        Ok(())
    }

    fn recover_staged_payment_digest(
        &self,
        key: &PaymentOutboxKeyV1,
    ) -> Result<Digest, AuthenticatedPaymentOutboxErrorV1> {
        self.check_available()?;
        let records = self.records.lock().expect("outbox lock");
        let record = records
            .get(&key.digest())
            .ok_or(AuthenticatedPaymentOutboxErrorV1::Missing)?;
        if record.key != *key || !Self::record_is_authenticated(record) {
            return Err(AuthenticatedPaymentOutboxErrorV1::Corrupt);
        }
        Ok(record.payment_digest)
    }

    fn publish_payment_or_recover(
        &self,
        authorization: &PaymentOutboxPublicationV1,
    ) -> Result<AuthenticatedPaymentOutboxRecordV1, AuthenticatedPaymentOutboxErrorV1> {
        self.check_available()?;
        if self.fail_publish_once.swap(false, Ordering::SeqCst) {
            return Err(AuthenticatedPaymentOutboxErrorV1::Unavailable);
        }
        if authorization.payment_digest() == [0; 32]
            || authorization.intent_epoch() == 0
            || authorization.intent_digest() == [0; 32]
            || authorization.authorization_digest() == [0; 32]
        {
            return Err(AuthenticatedPaymentOutboxErrorV1::Corrupt);
        }
        let mut records = self.records.lock().expect("outbox lock");
        let record = records
            .get_mut(&authorization.key().digest())
            .ok_or(AuthenticatedPaymentOutboxErrorV1::Missing)?;
        if !Self::record_is_authenticated(record)
            || record.key != *authorization.key()
            || record.payment_digest != authorization.payment_digest()
        {
            return Err(AuthenticatedPaymentOutboxErrorV1::Conflict);
        }
        match record.publication_digest {
            None => record.publication_digest = Some(authorization.authorization_digest()),
            Some(existing) if existing == authorization.authorization_digest() => {}
            Some(_) => return Err(AuthenticatedPaymentOutboxErrorV1::Conflict),
        }
        record.authenticator = Self::authenticator(
            &record.key,
            record.payment_digest,
            &record.canonical_payment,
            record.publication_digest,
        );
        Ok(AuthenticatedPaymentOutboxRecordV1::new(
            record.key,
            record.payment_digest,
            record.canonical_payment.to_vec(),
            record.publication_digest,
        ))
    }

    fn recover_published_payment(
        &self,
        key: &PaymentOutboxKeyV1,
    ) -> Result<AuthenticatedPaymentOutboxRecordV1, AuthenticatedPaymentOutboxErrorV1> {
        self.check_available()?;
        let records = self.records.lock().expect("outbox lock");
        let record = records
            .get(&key.digest())
            .ok_or(AuthenticatedPaymentOutboxErrorV1::Missing)?;
        if record.key != *key || !Self::record_is_authenticated(record) {
            return Err(AuthenticatedPaymentOutboxErrorV1::Corrupt);
        }
        if record.publication_digest.is_none() {
            return Err(AuthenticatedPaymentOutboxErrorV1::Missing);
        }
        Ok(AuthenticatedPaymentOutboxRecordV1::new(
            record.key,
            record.payment_digest,
            record.canonical_payment.to_vec(),
            record.publication_digest,
        ))
    }
}

#[derive(Clone, Copy)]
struct ActiveRecord {
    request: HardwareIntentRequestV1,
    epoch: u64,
    signing_digest: Option<Digest>,
    signature: Option<KagemushaDeviceSignatureV2>,
    bound_digest: Option<Digest>,
}

#[derive(Clone, Copy)]
struct TerminalRecord {
    outcome: HardwareTerminalOutcomeV1,
    acknowledgement_signing_digest: Option<Digest>,
    acknowledgement_signature: Option<KagemushaDeviceSignatureV2>,
}

#[derive(Default)]
struct WalletJournal {
    sequence: u64,
    next_epoch: u64,
    active: Option<ActiveRecord>,
    terminals: BTreeMap<Digest, TerminalRecord>,
}

struct TestHardware {
    available: AtomicBool,
    trusted_time_ms: AtomicU64,
    journals: Mutex<BTreeMap<Digest, WalletJournal>>,
}

impl TestHardware {
    fn new(trusted_time_ms: u64) -> Self {
        Self {
            available: AtomicBool::new(true),
            trusted_time_ms: AtomicU64::new(trusted_time_ms),
            journals: Mutex::new(BTreeMap::new()),
        }
    }

    fn set_available(&self, available: bool) {
        self.available.store(available, Ordering::SeqCst);
    }

    fn set_time(&self, trusted_time_ms: u64) {
        self.trusted_time_ms
            .store(trusted_time_ms, Ordering::SeqCst);
    }

    fn time(&self) -> u64 {
        self.trusted_time_ms.load(Ordering::SeqCst)
    }

    fn check_available(&self) -> Result<(), HardwareGuardErrorV1> {
        if self.available.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(HardwareGuardErrorV1::Unavailable)
        }
    }

    fn check_key(public_key: &KagemushaDevicePublicKeyV2) -> Result<(), HardwareGuardErrorV1> {
        if public_key == &receiver_public_key() {
            Ok(())
        } else {
            Err(HardwareGuardErrorV1::Rejected)
        }
    }

    fn check_live(&self, request: &HardwareIntentRequestV1) -> Result<u64, HardwareGuardErrorV1> {
        let now_ms = self.time();
        if now_ms < request.not_before_ms() || now_ms >= request.expires_at_ms() {
            Err(HardwareGuardErrorV1::TrustedTimeRejected)
        } else {
            Ok(now_ms)
        }
    }

    fn active_bound_digest(&self, wallet_binding: Digest) -> Option<Digest> {
        self.journals
            .lock()
            .expect("journal lock")
            .get(&wallet_binding)
            .and_then(|journal| journal.active)
            .and_then(|active| active.bound_digest)
    }

    fn terminal(
        journal: &WalletJournal,
        request: &HardwareIntentRequestV1,
    ) -> Option<TerminalRecord> {
        journal
            .terminals
            .get(&request.challenge_digest())
            .copied()
            .filter(|terminal| terminal.outcome.intent() == request)
    }
}

impl sealed::Sealed for TestHardware {}

impl ExactNextHardwareGuardBackendV1 for TestHardware {
    fn reserve_receive_intent_and_sign_or_recover(
        &self,
        request: &HardwareIntentRequestV1,
        signing_bytes: &[u8],
        receiver_public_key: &KagemushaDevicePublicKeyV2,
    ) -> Result<HardwareReceiveSigningResultV1, HardwareGuardErrorV1> {
        self.check_available()?;
        self.check_live(request)?;
        Self::check_key(receiver_public_key)?;
        if request.kind() != HardwareIntentKindV1::ReceivePending || signing_bytes.is_empty() {
            return Err(HardwareGuardErrorV1::Rejected);
        }
        let signing_digest = hash_bytes(signing_bytes);
        let mut journals = self.journals.lock().expect("journal lock");
        let journal = journals.entry(request.wallet_binding()).or_default();
        if let Some(active) = journal.active {
            if active.request == *request
                && active.signing_digest == Some(signing_digest)
                && active.signature.is_some()
            {
                return Ok(HardwareReceiveSigningResultV1::new(
                    active.epoch,
                    active.signature.expect("checked signature"),
                ));
            }
            return Err(HardwareGuardErrorV1::StaleOrConcurrent);
        }
        if request.from_sequence() != journal.sequence {
            return Err(HardwareGuardErrorV1::StaleOrConcurrent);
        }
        journal.next_epoch = journal
            .next_epoch
            .checked_add(1)
            .ok_or(HardwareGuardErrorV1::Rejected)?;
        let signature = sign(signing_bytes);
        journal.active = Some(ActiveRecord {
            request: *request,
            epoch: journal.next_epoch,
            signing_digest: Some(signing_digest),
            signature: Some(signature),
            bound_digest: None,
        });
        Ok(HardwareReceiveSigningResultV1::new(
            journal.next_epoch,
            signature,
        ))
    }

    fn recover_receive_intent_and_signature(
        &self,
        request: &HardwareIntentRequestV1,
        signing_bytes: &[u8],
        receiver_public_key: &KagemushaDevicePublicKeyV2,
    ) -> Result<HardwareReceiveSigningResultV1, HardwareGuardErrorV1> {
        self.check_available()?;
        Self::check_key(receiver_public_key)?;
        let signing_digest = hash_bytes(signing_bytes);
        let journals = self.journals.lock().expect("journal lock");
        let active = journals
            .get(&request.wallet_binding())
            .and_then(|journal| journal.active)
            .ok_or(HardwareGuardErrorV1::IntentMismatch)?;
        if active.request != *request || active.signing_digest != Some(signing_digest) {
            return Err(HardwareGuardErrorV1::IntentMismatch);
        }
        Ok(HardwareReceiveSigningResultV1::new(
            active.epoch,
            active
                .signature
                .ok_or(HardwareGuardErrorV1::IntentMismatch)?,
        ))
    }

    fn bind_receive_request_digest_or_recover(
        &self,
        request: &HardwareIntentRequestV1,
        intent_epoch: u64,
        request_digest: Digest,
    ) -> Result<(), HardwareGuardErrorV1> {
        self.check_available()?;
        if request_digest == [0; 32] {
            return Err(HardwareGuardErrorV1::Rejected);
        }
        let mut journals = self.journals.lock().expect("journal lock");
        let active = journals
            .get_mut(&request.wallet_binding())
            .and_then(|journal| journal.active.as_mut())
            .ok_or(HardwareGuardErrorV1::IntentMismatch)?;
        if active.request != *request || active.epoch != intent_epoch {
            return Err(HardwareGuardErrorV1::IntentMismatch);
        }
        match active.bound_digest {
            None => active.bound_digest = Some(request_digest),
            Some(existing) if existing == request_digest => {}
            Some(_) => return Err(HardwareGuardErrorV1::StaleOrConcurrent),
        }
        Ok(())
    }

    fn publish_send_payment_or_recover(
        &self,
        request: &HardwareIntentRequestV1,
        payment_digest: Digest,
    ) -> Result<u64, HardwareGuardErrorV1> {
        self.check_available()?;
        self.check_live(request)?;
        if request.kind() != HardwareIntentKindV1::SendPublished || payment_digest == [0; 32] {
            return Err(HardwareGuardErrorV1::Rejected);
        }
        let mut journals = self.journals.lock().expect("journal lock");
        let journal = journals.entry(request.wallet_binding()).or_default();
        if let Some(active) = journal.active {
            if active.request == *request && active.bound_digest == Some(payment_digest) {
                return Ok(active.epoch);
            }
            return Err(HardwareGuardErrorV1::StaleOrConcurrent);
        }
        if request.from_sequence() != journal.sequence {
            return Err(HardwareGuardErrorV1::StaleOrConcurrent);
        }
        journal.next_epoch = journal
            .next_epoch
            .checked_add(1)
            .ok_or(HardwareGuardErrorV1::Rejected)?;
        journal.active = Some(ActiveRecord {
            request: *request,
            epoch: journal.next_epoch,
            signing_digest: None,
            signature: None,
            bound_digest: Some(payment_digest),
        });
        Ok(journal.next_epoch)
    }

    fn recover_active_intent(
        &self,
        request: &HardwareIntentRequestV1,
    ) -> Result<HardwareActiveIntentOutcomeV1, HardwareGuardErrorV1> {
        self.check_available()?;
        let journals = self.journals.lock().expect("journal lock");
        let active = journals
            .get(&request.wallet_binding())
            .and_then(|journal| journal.active)
            .ok_or(HardwareGuardErrorV1::IntentMismatch)?;
        if active.request != *request {
            return Err(HardwareGuardErrorV1::IntentMismatch);
        }
        Ok(HardwareActiveIntentOutcomeV1::new(
            active.request,
            active.epoch,
            active
                .bound_digest
                .ok_or(HardwareGuardErrorV1::IntentMismatch)?,
        ))
    }

    fn cancel_expired_receive_or_recover(
        &self,
        request: &HardwareIntentRequestV1,
        intent_epoch: u64,
        completion_digest: Digest,
    ) -> Result<HardwareTerminalOutcomeV1, HardwareGuardErrorV1> {
        self.check_available()?;
        let now_ms = self.time();
        let mut journals = self.journals.lock().expect("journal lock");
        let journal = journals.entry(request.wallet_binding()).or_default();
        if let Some(terminal) = Self::terminal(journal, request) {
            if terminal.outcome.operation() == HardwareTerminalOperationV1::ReceiveCancelled
                && terminal.outcome.intent_epoch() == intent_epoch
                && terminal.outcome.completion_digest() == completion_digest
            {
                return Ok(terminal.outcome);
            }
            return Err(HardwareGuardErrorV1::StaleOrConcurrent);
        }
        let active = journal.active.ok_or(HardwareGuardErrorV1::IntentMismatch)?;
        if active.request != *request
            || active.epoch != intent_epoch
            || active.bound_digest.is_none()
            || request.kind() != HardwareIntentKindV1::ReceivePending
            || now_ms < request.expires_at_ms()
        {
            return Err(HardwareGuardErrorV1::TrustedTimeRejected);
        }
        let outcome = HardwareTerminalOutcomeV1::new(
            HardwareTerminalOperationV1::ReceiveCancelled,
            *request,
            intent_epoch,
            request.from_sequence(),
            request.from_sequence(),
            active.bound_digest.expect("bound receive request"),
            completion_digest,
            None,
            None,
            now_ms,
            request.current_head(),
        );
        journal.active = None;
        journal.terminals.insert(
            request.challenge_digest(),
            TerminalRecord {
                outcome,
                acknowledgement_signing_digest: None,
                acknowledgement_signature: None,
            },
        );
        Ok(outcome)
    }

    fn commit_intent_or_recover_exact_next(
        &self,
        request: &HardwareIntentCommitRequestV1,
    ) -> Result<HardwareTerminalOutcomeV1, HardwareGuardErrorV1> {
        self.check_available()?;
        let intent = request.intent();
        let mut journals = self.journals.lock().expect("journal lock");
        let journal = journals.entry(intent.wallet_binding()).or_default();
        if let Some(terminal) = Self::terminal(journal, intent) {
            let outcome = terminal.outcome;
            if outcome.intent_epoch() == request.intent_epoch()
                && outcome.intent_binding_digest() == request.intent_binding_digest()
                && outcome.from_sequence() == request.guard().from_sequence()
                && outcome.to_sequence() == request.guard().to_sequence()
                && outcome.completion_digest() == request.completion_digest()
                && outcome.payment_digest() == Some(request.payment_digest())
                && outcome.acknowledgement_digest() == request.acknowledgement_digest()
                && outcome.successor_head() == request.successor_head()
            {
                return Ok(outcome);
            }
            return Err(HardwareGuardErrorV1::StaleOrConcurrent);
        }
        let active = journal.active.ok_or(HardwareGuardErrorV1::IntentMismatch)?;
        let guard = request.guard();
        if active.request != *intent
            || active.epoch != request.intent_epoch()
            || active.bound_digest.is_none()
            || active.bound_digest != Some(request.intent_binding_digest())
            || guard.from_sequence() != journal.sequence
            || guard.to_sequence() != journal.sequence.checked_add(1).unwrap_or(u64::MAX)
            || guard.device_id() != intent.device_id()
            || guard.hardware_policy_id() != intent.hardware_policy_id()
            || guard.wallet_binding() != intent.wallet_binding()
        {
            return Err(HardwareGuardErrorV1::StaleOrConcurrent);
        }
        let operation = match intent.kind() {
            HardwareIntentKindV1::ReceivePending => {
                if request.acknowledgement_digest().is_some() {
                    return Err(HardwareGuardErrorV1::Rejected);
                }
                let now_ms = self.time();
                if now_ms < intent.not_before_ms() || now_ms >= intent.expires_at_ms() {
                    return Err(HardwareGuardErrorV1::TrustedTimeRejected);
                }
                HardwareTerminalOperationV1::ReceiveCommitted
            }
            HardwareIntentKindV1::SendPublished => {
                if active.bound_digest != Some(request.payment_digest())
                    || request.acknowledgement_digest().is_none()
                {
                    return Err(HardwareGuardErrorV1::Rejected);
                }
                HardwareTerminalOperationV1::SendCommitted
            }
        };
        let outcome = HardwareTerminalOutcomeV1::new(
            operation,
            *intent,
            request.intent_epoch(),
            guard.from_sequence(),
            guard.to_sequence(),
            active.bound_digest.expect("bound intent"),
            request.completion_digest(),
            Some(request.payment_digest()),
            request.acknowledgement_digest(),
            self.time(),
            request.successor_head(),
        );
        journal.sequence = guard.to_sequence();
        journal.active = None;
        journal.terminals.insert(
            intent.challenge_digest(),
            TerminalRecord {
                outcome,
                acknowledgement_signing_digest: None,
                acknowledgement_signature: None,
            },
        );
        Ok(outcome)
    }

    fn recover_terminal_outcome(
        &self,
        request: &HardwareIntentRequestV1,
    ) -> Result<HardwareTerminalOutcomeV1, HardwareGuardErrorV1> {
        self.check_available()?;
        self.journals
            .lock()
            .expect("journal lock")
            .get(&request.wallet_binding())
            .and_then(|journal| Self::terminal(journal, request))
            .map(|terminal| terminal.outcome)
            .ok_or(HardwareGuardErrorV1::IntentMismatch)
    }

    fn recover_receive_terminal_outcome(
        &self,
        query: &HardwareReceiveTerminalQueryV1,
    ) -> Result<HardwareTerminalOutcomeV1, HardwareGuardErrorV1> {
        self.check_available()?;
        let journals = self.journals.lock().expect("journal lock");
        let journal = journals
            .get(&query.wallet_binding())
            .ok_or(HardwareGuardErrorV1::IntentMismatch)?;
        let mut matching = journal.terminals.values().filter(|terminal| {
            let outcome = terminal.outcome;
            outcome.operation() == HardwareTerminalOperationV1::ReceiveCommitted
                && outcome.intent().device_id() == query.device_id()
                && outcome.intent().hardware_policy_id() == query.hardware_policy_id()
                && outcome.intent().wallet_binding() == query.wallet_binding()
                && outcome.intent().context_digest() == query.context_digest()
                && outcome.intent_binding_digest() == query.request_digest()
                && outcome.payment_digest() == Some(query.payment_digest())
        });
        let outcome = matching
            .next()
            .map(|record| record.outcome)
            .ok_or(HardwareGuardErrorV1::IntentMismatch)?;
        if matching.next().is_some() {
            return Err(HardwareGuardErrorV1::StaleOrConcurrent);
        }
        Ok(outcome)
    }

    fn sign_receive_acknowledgement_or_recover(
        &self,
        outcome: &HardwareTerminalOutcomeV1,
        acknowledgement_digest: Digest,
        signing_bytes: &[u8],
        receiver_public_key: &KagemushaDevicePublicKeyV2,
    ) -> Result<KagemushaDeviceSignatureV2, HardwareGuardErrorV1> {
        self.check_available()?;
        Self::check_key(receiver_public_key)?;
        if outcome.operation() != HardwareTerminalOperationV1::ReceiveCommitted
            || acknowledgement_digest == [0; 32]
            || signing_bytes.is_empty()
        {
            return Err(HardwareGuardErrorV1::Rejected);
        }
        let signing_digest = hash_bytes(signing_bytes);
        let mut journals = self.journals.lock().expect("journal lock");
        let journal = journals
            .get_mut(&outcome.intent().wallet_binding())
            .ok_or(HardwareGuardErrorV1::IntentMismatch)?;
        let terminal = journal
            .terminals
            .get_mut(&outcome.intent().challenge_digest())
            .ok_or(HardwareGuardErrorV1::IntentMismatch)?;
        if terminal.outcome.operation() != outcome.operation()
            || terminal.outcome.intent() != outcome.intent()
            || terminal.outcome.intent_epoch() != outcome.intent_epoch()
            || terminal.outcome.payment_digest() != outcome.payment_digest()
            || terminal.outcome.completion_digest() != outcome.completion_digest()
        {
            return Err(HardwareGuardErrorV1::IntentMismatch);
        }
        if let Some(signature) = terminal.acknowledgement_signature {
            if terminal.acknowledgement_signing_digest == Some(signing_digest)
                && terminal.outcome.acknowledgement_digest() == Some(acknowledgement_digest)
            {
                return Ok(signature);
            }
            return Err(HardwareGuardErrorV1::StaleOrConcurrent);
        }
        let signature = sign(signing_bytes);
        terminal.acknowledgement_signing_digest = Some(signing_digest);
        terminal.acknowledgement_signature = Some(signature);
        terminal.outcome = HardwareTerminalOutcomeV1::new(
            terminal.outcome.operation(),
            *terminal.outcome.intent(),
            terminal.outcome.intent_epoch(),
            terminal.outcome.from_sequence(),
            terminal.outcome.to_sequence(),
            terminal.outcome.intent_binding_digest(),
            terminal.outcome.completion_digest(),
            terminal.outcome.payment_digest(),
            Some(acknowledgement_digest),
            terminal.outcome.trusted_time_ms(),
            terminal.outcome.successor_head(),
        );
        Ok(signature)
    }
}

fn network_id() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"offline-cash-state-transition-tests",
    )))
}

fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "xor".parse().expect("asset name"),
    )
}

fn recipient() -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

fn balance(wallet_tag: u8, amount: u128) -> BalanceOwnerV1 {
    let context = OfflineCashStateContextV1::new(RELEASE_ID, network_id(), asset(), 4)
        .expect("state context");
    BalanceOwnerV1::restore_authenticated(
        context,
        [wallet_tag; 32],
        DEVICE_ID,
        POLICY_ID,
        amount,
        Zeroizing::new([wallet_tag.wrapping_add(1); 32]),
        [wallet_tag.wrapping_add(2); 32],
        0,
        None,
    )
    .expect("authenticated balance")
}

fn session<'a>(
    hardware: &'a TestHardware,
    balance: &BalanceOwnerV1,
) -> HardwareGuardSessionV1<'a, TestHardware> {
    HardwareGuardSessionV1::new(
        hardware,
        balance.guard_device_id,
        balance.hardware_policy_id,
        balance.wallet_binding,
    )
}

fn unsigned_request(
    balance: &BalanceOwnerV1,
    amount: u128,
    request_tag: u8,
) -> UnsignedReceiveRequestV1 {
    let public_key = receiver_public_key();
    UnsignedReceiveRequestV1::new(
        RELEASE_ID,
        network_id(),
        asset(),
        4,
        amount,
        recipient(),
        balance.head(),
        offline_cash_receiver_key_reference_v1(&public_key),
        public_key,
        [request_tag; 32],
        ISSUED_AT_MS,
        EXPIRES_AT_MS,
        POLICY_ID,
    )
    .expect("unsigned request")
}

fn open_pending(
    hardware: &TestHardware,
    balance: &mut BalanceOwnerV1,
    amount: u128,
    request_tag: u8,
) -> (PendingOwnerV1, OfflineCashPaymentRequestV1) {
    let plan = prepare_open_pending_v1(
        balance,
        unsigned_request(balance, amount, request_tag),
        ISSUED_AT_MS,
    )
    .expect("open plan");
    let guard_session = session(hardware, balance);
    apply_open_pending_v1(balance, plan, &guard_session).expect("open pending")
}

fn payment_for_plan(
    plan: &SendSplitPlanV1,
    request: &OfflineCashPaymentRequestV1,
    encrypted_tag: u8,
) -> OfflineCashPaymentV1 {
    let semantic_digest = plan
        .statement()
        .canonical_digest()
        .expect("statement digest");
    let payment = OfflineCashPaymentV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        request_digest: request.canonical_digest().expect("request digest"),
        statement: plan.statement().clone(),
        proof: OfflineCashPairedProofV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            eq_protocol_digest: [0x71; 32],
            ep_protocol_digest: [0x72; 32],
            semantic_digest,
            eq_proof: vec![0x73; 64],
            ep_proof: vec![0x74; 64],
            eq_history: vec![0x75; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            ep_history: vec![0x76; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
        },
        encrypted_credit: vec![encrypted_tag; 96],
        artifact_manifest_digest: [0x77; 32],
    };
    payment
        .validate_against(request)
        .expect("canonical payment");
    payment
}

fn bind_credit(
    pending: &PendingOwnerV1,
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
    outgoing: OutgoingCreditOwnerV1,
) -> CreditOwnerV1 {
    let payment_digest = payment
        .canonical_digest_against(request)
        .expect("payment digest");
    let verification = VerifiedOfflineCashCreditV1 {
        release_id: pending.context.release_id,
        request_digest: pending.request_digest,
        payment_digest,
        network_id: pending.context.network_id.clone(),
        asset: pending.context.asset.clone(),
        scale: pending.context.scale,
        amount: pending.amount,
        receiver_before: pending.receiver_head,
        recipient_key_reference: pending.recipient_key_reference,
        credit_commitment: outgoing.commitment,
        transition_digest: outgoing.send_transition_digest,
        encrypted_credit_digest: hash_bytes(&payment.encrypted_credit),
    };
    let opening = DecryptedCreditOpeningOwnerV1::from_authenticated_decryption(
        outgoing.opening,
        &payment.encrypted_credit,
        pending.recipient_key_reference,
    )
    .expect("authenticated opening");
    bind_verified_credit_v1(pending, &payment.statement, verification, opening)
        .map_err(|rejection| rejection.error())
        .expect("verified credit binding")
}

fn duplicate_credit(credit: &CreditOwnerV1) -> CreditOwnerV1 {
    CreditOwnerV1 {
        context: credit.context.clone(),
        request_digest: credit.request_digest,
        receiver_head: credit.receiver_head,
        recipient_key_reference: credit.recipient_key_reference,
        amount: credit.amount,
        commitment: credit.commitment,
        send_transition_digest: credit.send_transition_digest,
        payment_digest: credit.payment_digest,
        opening: credit.opening.clone(),
        verification: VerifiedOfflineCashCreditV1 {
            release_id: credit.verification.release_id(),
            request_digest: credit.verification.request_digest(),
            payment_digest: credit.verification.payment_digest(),
            network_id: credit.verification.network_id().clone(),
            asset: credit.verification.asset().clone(),
            scale: credit.verification.scale(),
            amount: credit.verification.amount(),
            receiver_before: credit.verification.receiver_before(),
            recipient_key_reference: credit.verification.recipient_key_reference(),
            credit_commitment: credit.verification.credit_commitment(),
            transition_digest: credit.verification.transition_digest(),
            encrypted_credit_digest: credit.verification.encrypted_credit_digest(),
        },
    }
}

fn acknowledgement_owner(
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
    receiver_head: Digest,
    acknowledgement_digest: Digest,
) -> VerifiedAcknowledgementOwnerV1 {
    verified_acknowledgement_for_test_v1(
        request.canonical_digest().expect("request digest"),
        payment
            .canonical_digest_against(request)
            .expect("payment digest"),
        receiver_head,
        acknowledgement_digest,
    )
}

#[test]
fn request_signing_and_intent_reservation_are_one_fail_closed_operation() {
    let hardware = TestHardware::new(ISSUED_AT_MS);
    let mut receiver = balance(0x41, 5);
    let plan = prepare_open_pending_v1(
        &receiver,
        unsigned_request(&receiver, 9, 0x81),
        ISSUED_AT_MS,
    )
    .expect("open plan");
    hardware.set_available(false);
    let guard_session = session(&hardware, &receiver);
    assert!(matches!(
        apply_open_pending_v1(&mut receiver, plan, &guard_session),
        Err(StateTransitionErrorV1::HardwareGuard(
            HardwareGuardErrorV1::Unavailable
        ))
    ));
    assert_eq!(receiver.active_request(), None);
    assert_eq!(hardware.active_bound_digest(receiver.wallet_binding), None);

    hardware.set_available(true);
    let (pending, request) = open_pending(&hardware, &mut receiver, 9, 0x81);
    assert_eq!(receiver.active_request(), Some(pending.request_digest));
    assert_eq!(
        hardware.active_bound_digest(receiver.wallet_binding),
        Some(request.canonical_digest().expect("request digest"))
    );
    request.validate().expect("hardware-signed request");
    let reconstructed = UnsignedReceiveRequestV1::from_signed_request(&request).unwrap();
    let core_signing_bytes = reconstructed.canonical_signing_bytes().unwrap();
    let model_signing_bytes = Zeroizing::new(request.canonical_signing_bytes().unwrap());
    assert_eq!(
        core_signing_bytes.as_slice(),
        model_signing_bytes.as_slice(),
        "Core and data-model request signing schemas must be byte-identical"
    );

    let mut restored = balance(0x41, 5);
    let guard_session = session(&hardware, &restored);
    let recovered = recover_pending_v1(&mut restored, &request, &guard_session)
        .expect("idempotent request recovery");
    assert_eq!(recovered.request_digest, pending.request_digest);
    assert_eq!(restored.active_request(), Some(pending.request_digest));
}

#[test]
fn staged_payment_survives_restart_but_is_exposed_only_after_hardware_cas() {
    let hardware = TestHardware::new(ISSUED_AT_MS);
    let outbox = TestOutbox::new();
    let mut receiver = balance(0x54, 0);
    let (_pending, request) = open_pending(&hardware, &mut receiver, 7, 0x8c);
    let sender = balance(0x55, 30);
    let stage_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let pre_cas_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let payment = payment_for_plan(&stage_plan, &request, 0x9a);
    let expected_bytes = Zeroizing::new(norito::encode_canonical(&payment).unwrap());
    let unpublished =
        stage_send_split_payment_v1(&sender, stage_plan, &request, payment, &outbox).unwrap();
    drop(unpublished);

    assert!(matches!(
        recover_published_send_v1(
            &sender,
            pre_cas_plan,
            &request,
            &session(&hardware, &sender),
            &outbox,
        ),
        Err(StateTransitionErrorV1::HardwareIntentMismatch)
            | Err(StateTransitionErrorV1::HardwareGuard(
                HardwareGuardErrorV1::IntentMismatch
            ))
    ));

    let recovery_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let recovered_unpublished =
        recover_unpublished_send_payment_v1(&sender, recovery_plan, &request, &outbox)
            .expect("restart recovers only opaque staged authority");
    outbox.fail_next_publish();
    assert!(matches!(
        publish_send_split_v1(
            &sender,
            recovered_unpublished,
            &request,
            &session(&hardware, &sender),
            &outbox,
            ISSUED_AT_MS,
        ),
        Err(StateTransitionErrorV1::PaymentOutbox(
            AuthenticatedPaymentOutboxErrorV1::Unavailable
        ))
    ));
    assert!(hardware
        .active_bound_digest(sender.wallet_binding)
        .is_some());

    let replay_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let (_published, recovered_payment) = recover_published_send_v1(
        &sender,
        replay_plan,
        &request,
        &session(&hardware, &sender),
        &outbox,
    )
    .expect("hardware-CAS publication recovery");
    let recovered_bytes = Zeroizing::new(norito::encode_canonical(&recovered_payment).unwrap());
    assert_eq!(recovered_bytes.as_slice(), expected_bytes.as_slice());

    let identical_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let (_published, identical_payment) = recover_published_send_v1(
        &sender,
        identical_plan,
        &request,
        &session(&hardware, &sender),
        &outbox,
    )
    .expect("published recovery is idempotent");
    assert_eq!(identical_payment, recovered_payment);
}

#[test]
fn duplicate_receive_signing_with_substituted_canonical_fields_is_rejected() {
    let hardware = TestHardware::new(ISSUED_AT_MS);
    let mut receiver = balance(0x42, 0);
    let (_pending, _request) = open_pending(&hardware, &mut receiver, 3, 0x82);
    let mut rolled_back = balance(0x42, 0);
    let conflicting = prepare_open_pending_v1(
        &rolled_back,
        unsigned_request(&rolled_back, 3, 0x83),
        ISSUED_AT_MS,
    )
    .expect("conflicting plan");
    let guard_session = session(&hardware, &rolled_back);
    assert!(matches!(
        apply_open_pending_v1(&mut rolled_back, conflicting, &guard_session),
        Err(StateTransitionErrorV1::HardwareGuard(
            HardwareGuardErrorV1::StaleOrConcurrent
        ))
    ));
    assert_eq!(rolled_back.active_request(), None);
}

#[test]
fn send_publication_is_atomic_and_binds_the_canonical_payment() {
    let hardware = TestHardware::new(ISSUED_AT_MS);
    let outbox = TestOutbox::new();
    let mut receiver = balance(0x43, 0);
    let (_pending, request) = open_pending(&hardware, &mut receiver, 7, 0x84);
    let sender = balance(0x44, 30);
    let plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).expect("send plan");
    let payment = payment_for_plan(&plan, &request, 0x91);
    let unpublished =
        stage_send_split_payment_v1(&sender, plan, &request, payment, &outbox).unwrap();
    hardware.set_available(false);
    assert!(matches!(
        publish_send_split_v1(
            &sender,
            unpublished,
            &request,
            &session(&hardware, &sender),
            &outbox,
            ISSUED_AT_MS,
        ),
        Err(StateTransitionErrorV1::HardwareGuard(
            HardwareGuardErrorV1::Unavailable
        ))
    ));
    assert_eq!(hardware.active_bound_digest(sender.wallet_binding), None);

    hardware.set_available(true);
    let plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).expect("send plan");
    let payment = payment_for_plan(&plan, &request, 0x91);
    let payment_digest = payment
        .canonical_digest_against(&request)
        .expect("payment digest");
    let unpublished =
        stage_send_split_payment_v1(&sender, plan, &request, payment, &outbox).unwrap();
    let (published, _output, returned_payment) = publish_send_split_v1(
        &sender,
        unpublished,
        &request,
        &session(&hardware, &sender),
        &outbox,
        ISSUED_AT_MS,
    )
    .expect("atomic publication");
    assert_eq!(published.payment_digest, payment_digest);
    assert_eq!(
        returned_payment.canonical_digest_against(&request).unwrap(),
        payment_digest
    );
    assert_eq!(
        hardware.active_bound_digest(sender.wallet_binding),
        Some(payment_digest)
    );
}

#[test]
fn payment_substitution_cannot_recover_a_published_send() {
    let hardware = TestHardware::new(ISSUED_AT_MS);
    let outbox = TestOutbox::new();
    let mut receiver = balance(0x45, 0);
    let (_pending, request) = open_pending(&hardware, &mut receiver, 8, 0x85);
    let sender = balance(0x46, 40);
    let first_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let conflicting_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let recovery_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let payment = payment_for_plan(&first_plan, &request, 0x92);
    let outbox_key = PaymentOutboxKeyV1::new(&sender, &first_plan);
    let unpublished =
        stage_send_split_payment_v1(&sender, first_plan, &request, payment, &outbox).unwrap();
    let conflicting_payment = payment_for_plan(&conflicting_plan, &request, 0x93);
    assert!(matches!(
        stage_send_split_payment_v1(
            &sender,
            conflicting_plan,
            &request,
            conflicting_payment,
            &outbox,
        ),
        Err(StateTransitionErrorV1::PaymentOutbox(
            AuthenticatedPaymentOutboxErrorV1::Conflict
        ))
    ));
    let (_published, _output, _payment) = publish_send_split_v1(
        &sender,
        unpublished,
        &request,
        &session(&hardware, &sender),
        &outbox,
        ISSUED_AT_MS,
    )
    .expect("publication");
    outbox.mutate_staged_byte(outbox_key.digest());
    assert!(matches!(
        recover_published_send_v1(
            &sender,
            recovery_plan,
            &request,
            &session(&hardware, &sender),
            &outbox,
        ),
        Err(StateTransitionErrorV1::PaymentOutbox(
            AuthenticatedPaymentOutboxErrorV1::Corrupt
        ))
    ));
}

#[test]
fn authenticated_outbox_bytes_still_use_the_bounded_exact_payment_decoder() {
    let hardware = TestHardware::new(ISSUED_AT_MS);
    let outbox = TestOutbox::new();
    let mut receiver = balance(0x9b, 0);
    let (_pending, request) = open_pending(&hardware, &mut receiver, 8, 0xa1);
    let sender = balance(0x9c, 40);
    let publication_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let recovery_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let outbox_key = PaymentOutboxKeyV1::new(&sender, &publication_plan);
    let payment = payment_for_plan(&publication_plan, &request, 0xa2);
    let unpublished =
        stage_send_split_payment_v1(&sender, publication_plan, &request, payment, &outbox).unwrap();
    drop(
        publish_send_split_v1(
            &sender,
            unpublished,
            &request,
            &session(&hardware, &sender),
            &outbox,
            ISSUED_AT_MS,
        )
        .expect("publication"),
    );

    outbox.replace_staged_bytes_authenticated(
        outbox_key.digest(),
        vec![0; OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 + 1],
    );
    assert!(matches!(
        recover_published_send_v1(
            &sender,
            recovery_plan,
            &request,
            &session(&hardware, &sender),
            &outbox,
        ),
        Err(StateTransitionErrorV1::PaymentOutbox(
            AuthenticatedPaymentOutboxErrorV1::Corrupt
        ))
    ));
}

#[test]
fn hardware_trusted_expiry_overrides_a_forged_local_send_time() {
    let hardware = TestHardware::new(ISSUED_AT_MS);
    let outbox = TestOutbox::new();
    let mut receiver = balance(0x47, 0);
    let (_pending, request) = open_pending(&hardware, &mut receiver, 4, 0x86);
    let sender = balance(0x48, 20);
    let plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let payment = payment_for_plan(&plan, &request, 0x93);
    let unpublished =
        stage_send_split_payment_v1(&sender, plan, &request, payment, &outbox).unwrap();
    hardware.set_time(EXPIRES_AT_MS);
    assert!(matches!(
        publish_send_split_v1(
            &sender,
            unpublished,
            &request,
            &session(&hardware, &sender),
            &outbox,
            ISSUED_AT_MS,
        ),
        Err(StateTransitionErrorV1::HardwareGuard(
            HardwareGuardErrorV1::TrustedTimeRejected
        ))
    ));
    assert_eq!(hardware.active_bound_digest(sender.wallet_binding), None);
}

#[test]
fn receive_commit_threads_payment_digest_and_only_then_mints_an_ack() {
    let hardware = TestHardware::new(ISSUED_AT_MS + 1);
    let outbox = TestOutbox::new();
    let mut receiver = balance(0x49, 11);
    let (pending, request) = open_pending(&hardware, &mut receiver, 13, 0x87);
    let mut sender = balance(0x4a, 50);
    let send_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let payment = payment_for_plan(&send_plan, &request, 0x94);
    let unpublished =
        stage_send_split_payment_v1(&sender, send_plan, &request, payment, &outbox).unwrap();
    let (published, output, payment) = publish_send_split_v1(
        &sender,
        unpublished,
        &request,
        &session(&hardware, &sender),
        &outbox,
        ISSUED_AT_MS,
    )
    .expect("publication");
    let (outgoing, statement) = output.into_parts();
    let credit = bind_credit(&pending, &request, &payment, outgoing);
    let receive_plan = prepare_receive_fold_v1(&receiver, &pending, &credit, ISSUED_AT_MS).unwrap();
    let receiver_session = session(&hardware, &receiver);
    let (received, ack_owner) = apply_receive_fold_v1(
        &mut receiver,
        pending,
        credit,
        receive_plan,
        ISSUED_AT_MS,
        &receiver_session,
    )
    .map_err(|rejection| rejection.error())
    .expect("receive commit");
    let payment_digest = payment.canonical_digest_against(&request).unwrap();
    assert_eq!(received.payment_digest, payment_digest);
    assert_eq!(received.credit_parent, statement.credit_commitment);
    assert_eq!(receiver.amount(), 24);

    let core_ack_signing_bytes = Zeroizing::new(ack_owner.signing_bytes().unwrap());
    drop(ack_owner);
    let recovered_ack_owner = recover_receive_acknowledgement_owner_v1(
        &receiver,
        &request,
        &payment,
        &session(&hardware, &receiver),
    )
    .expect("ACK owner recovery after balance persistence");
    let acknowledgement =
        issue_receive_acknowledgement_v1(&recovered_ack_owner, &session(&hardware, &receiver))
            .expect("post-commit acknowledgement");
    let model_ack_signing_bytes =
        Zeroizing::new(acknowledgement.canonical_signing_bytes().unwrap());
    assert_eq!(
        core_ack_signing_bytes.as_slice(),
        model_ack_signing_bytes.as_slice()
    );
    drop(recovered_ack_owner);
    let post_signing_owner = recover_receive_acknowledgement_owner_v1(
        &receiver,
        &request,
        &payment,
        &session(&hardware, &receiver),
    )
    .expect("ACK owner recovery after hardware signing");
    let recovered_acknowledgement =
        issue_receive_acknowledgement_v1(&post_signing_owner, &session(&hardware, &receiver))
            .expect("byte-identical acknowledgement recovery");
    assert_eq!(recovered_acknowledgement, acknowledgement);
    let mut substituted_payment = payment.clone();
    substituted_payment.encrypted_credit[0] ^= 1;
    assert!(matches!(
        recover_receive_acknowledgement_owner_v1(
            &receiver,
            &request,
            &substituted_payment,
            &session(&hardware, &receiver),
        ),
        Err(StateTransitionErrorV1::HardwareGuard(
            HardwareGuardErrorV1::IntentMismatch
        ))
    ));
    let advanced = BalanceOwnerV1::restore_authenticated(
        receiver.context.clone(),
        receiver.wallet_binding,
        receiver.guard_device_id,
        receiver.hardware_policy_id,
        receiver.amount,
        receiver.opening.clone(),
        receiver.lineage_digest,
        receiver.guard_sequence + 1,
        None,
    )
    .unwrap();
    assert_eq!(
        recover_receive_acknowledgement_owner_v1(
            &advanced,
            &request,
            &payment,
            &session(&hardware, &advanced),
        )
        .err(),
        Some(StateTransitionErrorV1::HardwareIntentMismatch),
        "a terminal ACK cannot replay after the balance advances"
    );
    acknowledgement
        .validate_against(&request, &payment)
        .expect("exact payment acknowledgement");
    let acknowledgement_digest = digest_framed(
        ACKNOWLEDGEMENT_OWNER_DOMAIN,
        &[&norito::encode_canonical(&acknowledgement).unwrap()],
    );
    let acknowledgement = acknowledgement_owner(
        &request,
        &payment,
        acknowledgement.receiver_balance_commitment,
        acknowledgement_digest,
    );
    let sender_session = session(&hardware, &sender);
    let committed =
        finalize_send_split_v1(&mut sender, published, acknowledgement, &sender_session)
            .map_err(|rejection| rejection.error())
            .expect("late sender finalization");
    assert_eq!(committed.acknowledgement_digest, acknowledgement_digest);
    assert_eq!(sender.amount(), 37);
    assert_eq!(sender.amount() + receiver.amount(), 61);
}

#[test]
fn receive_rejects_a_credit_whose_payment_digest_was_substituted() {
    let hardware = TestHardware::new(ISSUED_AT_MS);
    let outbox = TestOutbox::new();
    let mut receiver = balance(0x4b, 0);
    let (pending, request) = open_pending(&hardware, &mut receiver, 6, 0x88);
    let sender = balance(0x4c, 20);
    let send_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let payment = payment_for_plan(&send_plan, &request, 0x95);
    let unpublished =
        stage_send_split_payment_v1(&sender, send_plan, &request, payment, &outbox).unwrap();
    let (_published, output, payment) = publish_send_split_v1(
        &sender,
        unpublished,
        &request,
        &session(&hardware, &sender),
        &outbox,
        ISSUED_AT_MS,
    )
    .unwrap();
    let (outgoing, _) = output.into_parts();
    let mut credit = bind_credit(&pending, &request, &payment, outgoing);
    credit.payment_digest[0] ^= 1;
    assert_eq!(
        prepare_receive_fold_v1(&receiver, &pending, &credit, ISSUED_AT_MS).err(),
        Some(StateTransitionErrorV1::TerminalVerificationMismatch)
    );
}

#[test]
fn committed_send_is_recovered_after_crash_before_local_mutation() {
    let hardware = TestHardware::new(ISSUED_AT_MS);
    let outbox = TestOutbox::new();
    let mut receiver = balance(0x4d, 0);
    let (_pending, request) = open_pending(&hardware, &mut receiver, 10, 0x89);
    let mut sender = balance(0x4e, 100);
    let publication_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let payment = payment_for_plan(&publication_plan, &request, 0x96);
    let unpublished =
        stage_send_split_payment_v1(&sender, publication_plan, &request, payment, &outbox).unwrap();
    let (published, _output, payment) = publish_send_split_v1(
        &sender,
        unpublished,
        &request,
        &session(&hardware, &sender),
        &outbox,
        ISSUED_AT_MS,
    )
    .unwrap();
    let acknowledgement_digest = [0xa1; 32];
    drop(
        session(&hardware, &sender)
            .commit_intent_exact_next(
                &published.intent_authorization,
                &published.plan.challenge,
                published.payment_digest,
                Some(acknowledgement_digest),
                acknowledgement_digest,
                published.plan.remainder_head,
            )
            .expect("hardware commit before crash"),
    );
    assert_eq!(sender.amount(), 100);
    let recovery_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let sender_session = session(&hardware, &sender);
    let recovered = recover_committed_send_split_v1(&mut sender, recovery_plan, &sender_session)
        .expect("terminal send recovery");
    assert_eq!(
        recovered.payment_digest,
        payment.canonical_digest_against(&request).unwrap()
    );
    assert_eq!(recovered.acknowledgement_digest, acknowledgement_digest);
    assert_eq!(sender.amount(), 90);
    assert_eq!(sender.guard_sequence(), 1);
}

#[test]
fn committed_receive_is_recovered_after_crash_before_local_mutation() {
    let hardware = TestHardware::new(ISSUED_AT_MS + 2);
    let outbox = TestOutbox::new();
    let mut receiver = balance(0x4f, 2);
    let (pending_for_commit, request) = open_pending(&hardware, &mut receiver, 5, 0x8a);
    let mut restored = balance(0x4f, 2);
    let sender = balance(0x50, 20);
    let send_plan = prepare_send_split_v1(&sender, &request, ISSUED_AT_MS).unwrap();
    let payment = payment_for_plan(&send_plan, &request, 0x97);
    let unpublished =
        stage_send_split_payment_v1(&sender, send_plan, &request, payment, &outbox).unwrap();
    let (_published, output, payment) = publish_send_split_v1(
        &sender,
        unpublished,
        &request,
        &session(&hardware, &sender),
        &outbox,
        ISSUED_AT_MS,
    )
    .unwrap();
    let (outgoing, _) = output.into_parts();
    let credit_for_commit = bind_credit(&pending_for_commit, &request, &payment, outgoing);
    let (verification_for_recovery, opening_for_recovery) =
        duplicate_credit(&credit_for_commit).into_recovery_inputs();
    let (replay_verification, replay_opening) =
        duplicate_credit(&credit_for_commit).into_recovery_inputs();
    let commit_plan = prepare_receive_fold_v1(
        &receiver,
        &pending_for_commit,
        &credit_for_commit,
        ISSUED_AT_MS,
    )
    .unwrap();
    drop(
        session(&hardware, &receiver)
            .commit_intent_exact_next(
                &pending_for_commit.intent_authorization,
                &commit_plan.challenge,
                commit_plan.payment_digest,
                None,
                commit_plan.completion_digest,
                commit_plan.next_head,
            )
            .expect("hardware receive commit before crash"),
    );
    assert_eq!(receiver.amount(), 2);
    let mut substituted_payment = payment.clone();
    substituted_payment.encrypted_credit[0] ^= 1;
    let restored_session = session(&hardware, &restored);
    let rejection = match recover_committed_receive_fold_v1(
        &mut restored,
        &request,
        &substituted_payment,
        verification_for_recovery,
        opening_for_recovery,
        &restored_session,
    ) {
        Ok(_) => panic!("substituted recovery payment must fail closed"),
        Err(rejection) => rejection,
    };
    assert_eq!(rejection.error(), StateTransitionErrorV1::CreditMismatch);
    let (verification_for_recovery, opening_for_recovery) = rejection.into_owners();
    assert_eq!(restored.amount(), 2);
    hardware.set_time(EXPIRES_AT_MS + 1);
    let restored_session = session(&hardware, &restored);
    let (output, ack_owner) = recover_committed_receive_fold_v1(
        &mut restored,
        &request,
        &payment,
        verification_for_recovery,
        opening_for_recovery,
        &restored_session,
    )
    .map_err(|rejection| rejection.error())
    .expect("terminal receive recovery");
    assert_eq!(
        output.payment_digest,
        payment.canonical_digest_against(&request).unwrap()
    );
    assert_eq!(restored.amount(), 7);
    issue_receive_acknowledgement_v1(&ack_owner, &session(&hardware, &restored))
        .expect("recovered receive can issue ACK");
    let restored_session = session(&hardware, &restored);
    let replay = match recover_committed_receive_fold_v1(
        &mut restored,
        &request,
        &payment,
        replay_verification,
        replay_opening,
        &restored_session,
    ) {
        Ok(_) => panic!("committed receive must not apply twice"),
        Err(rejection) => rejection,
    };
    assert_eq!(replay.error(), StateTransitionErrorV1::RequestMismatch);
    assert_eq!(restored.amount(), 7);
}

#[test]
fn cancelled_receive_is_recovered_after_crash_before_cache_mutation() {
    let hardware = TestHardware::new(ISSUED_AT_MS);
    let mut receiver = balance(0x52, 12);
    let (pending, request) = open_pending(&hardware, &mut receiver, 3, 0x8b);
    hardware.set_time(EXPIRES_AT_MS);
    let plan = prepare_cancel_expired_pending_v1(&receiver, &pending, EXPIRES_AT_MS).unwrap();
    drop(
        session(&hardware, &receiver)
            .cancel_expired_receive(&pending.intent_authorization, plan.transition_digest())
            .expect("hardware cancellation before crash"),
    );
    assert!(receiver.active_request().is_some());
    let amount = receiver.amount();
    let head = receiver.head();
    let receiver_session = session(&hardware, &receiver);
    recover_cancelled_pending_v1(&mut receiver, &request, &receiver_session)
        .expect("terminal cancellation recovery");
    assert_eq!(receiver.active_request(), None);
    assert_eq!(receiver.amount(), amount);
    assert_eq!(receiver.head(), head);
    assert_eq!(receiver.guard_sequence(), 0);
}

#[test]
fn private_owners_are_move_only_and_redact_amounts() {
    assert!(core::mem::needs_drop::<BalanceOwnerV1>());
    assert!(core::mem::needs_drop::<SendSplitPlanV1>());
    assert!(core::mem::needs_drop::<ReceiveFoldPlanV1>());
    assert!(core::mem::needs_drop::<UnpublishedPaymentOwnerV1>());
    assert!(core::mem::needs_drop::<PaymentOutboxPublicationV1>());
    assert!(core::mem::needs_drop::<AuthenticatedPaymentOutboxRecordV1>());
    let balance = balance(0x53, 123_456_789);
    let debug = format!("{balance:?}");
    assert!(debug.contains("REDACTED"));
    assert!(!debug.contains("123456789"));
}
