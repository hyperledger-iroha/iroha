//! Deterministic ledger tests. Fixture proofs are explicit stand-ins: the injected test
//! verifier exercises orchestration only and is not evidence of native proof validity.
use super::*;
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use std::{cell::Cell, collections::BTreeMap};

fn fixture<T>(name: &str) -> T
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let values: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../fixtures/kagemusha/wallet_v1_vectors.json"
    ))
    .unwrap();
    let row = values["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["type"].as_str() == Some(name))
        .unwrap();
    let bytes = hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap();
    norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
        .unwrap()
}
pub(super) fn signing(byte: u8) -> SigningKey {
    SigningKey::from_slice(&[byte; 32]).unwrap()
}
pub(super) fn certificate(
    scheme: &KagemushaWalletSchemeV1,
    role: KagemushaWalletSignerRoleV1,
    key_byte: u8,
) -> KagemushaWalletSignerCertificateV1 {
    let key = signing(key_byte);
    let body = KagemushaWalletSignerCertificateBodyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        role,
        key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap(),
        serial: 99,
    };
    let signature: Signature = signing(0x11).sign(&body.signing_message());
    KagemushaWalletSignerCertificateV1::sign(
        body,
        scheme,
        KagemushaWalletSignerOutputV1::Der(signature.to_der().as_bytes()),
    )
    .unwrap()
}

struct Verifier {
    accept: bool,
    calls: Cell<usize>,
}
impl Verifier {
    fn new(accept: bool) -> Self {
        Self {
            accept,
            calls: Cell::new(0),
        }
    }
}
impl NativePackageVerifier for Verifier {
    fn verify(
        &self,
        _: &KagemushaWalletSchemeV1,
        _: &KagemushaWalletCredentialV1,
        _: &KagemushaWalletPackageV1,
    ) -> Result<()> {
        self.calls.set(self.calls.get() + 1);
        if self.accept {
            Ok(())
        } else {
            Err(Error::Proof)
        }
    }
}
#[derive(Clone)]
pub(super) struct Memory {
    pub(super) registration: Registration,
    authority: AccountId,
    wallets: BTreeMap<Digest, WalletRecord>,
    issues: BTreeMap<(Digest, Digest), Issuance>,
    payouts: BTreeMap<ClaimKey, Payout>,
    balances: BTreeMap<AccountId, u128>,
    certs: BTreeMap<Digest, KagemushaWalletSignerCertificateV1>,
    unavailable: bool,
    history: bool,
    writes: usize,
}
impl Memory {
    pub(super) fn new() -> Self {
        let activation: KagemushaWalletActivationV1 = fixture("KagemushaWalletActivationV1");
        let scheme: KagemushaWalletSchemeV1 = fixture("KagemushaWalletSchemeV1");
        let unload: KagemushaWalletUnloadClaimV1 = fixture("KagemushaWalletUnloadClaimV1");
        let authorizer = certificate(
            &scheme,
            KagemushaWalletSignerRoleV1::LoadAuthorization,
            0x34,
        );
        let reserve = AccountId::new(
            iroha_crypto::KeyPair::from_seed(vec![95; 32], iroha_crypto::Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let authority = unload.account;
        Self {
            registration: Registration {
                scheme,
                asset: activation.asset,
                reserve: reserve.clone(),
                balance_scope: AssetBalanceScope::Global,
                load_authorizer: authorizer,
            },
            authority: authority.clone(),
            wallets: BTreeMap::new(),
            issues: BTreeMap::new(),
            payouts: BTreeMap::new(),
            balances: BTreeMap::from([(authority, 1000), (reserve, 1000)]),
            certs: BTreeMap::from([(authorizer.certificate_digest(), authorizer)]),
            unavailable: false,
            history: false,
            writes: 0,
        }
    }
    pub(super) fn active(&mut self) -> LoadCommand {
        let activation = fixture("KagemushaWalletActivationV1");
        activate(self, &Verifier::new(true), &activation).unwrap();
        LoadCommand {
            scheme: self.registration.scheme.scheme_id(),
            wallet: activation.credential.body.wallet_id,
            request_id: [10; 32],
            amount: 100,
            charge: None,
        }
    }
}
impl Transaction for Memory {
    fn authority(&self) -> &AccountId {
        &self.authority
    }
    fn transaction_hash(&self) -> Digest {
        [42; 32]
    }
    fn block_height(&self) -> u64 {
        23
    }
    fn registration(&self, scheme: &Digest, asset: &Digest) -> Result<Registration> {
        if *scheme != self.registration.scheme.scheme_id()
            || *asset != self.registration.asset.asset_digest()
        {
            return Err(Error::Binding);
        }
        Ok(self.registration.clone())
    }
    fn wallet(&self, _: &Digest, wallet: &Digest) -> Result<Option<WalletRecord>> {
        if self.unavailable {
            return Err(Error::Unavailable);
        }
        Ok(self.wallets.get(wallet).copied())
    }
    fn issuance(&self, _: &Digest, wallet: &Digest, request: &Digest) -> Result<Option<Issuance>> {
        if self.unavailable {
            return Err(Error::Unavailable);
        }
        Ok(self.issues.get(&(*wallet, *request)).cloned())
    }
    fn payout(&self, _: &Digest, key: ClaimKey) -> Result<Option<Payout>> {
        if self.unavailable {
            return Err(Error::Unavailable);
        }
        Ok(self.payouts.get(&key).copied())
    }
    fn certificate(
        &self,
        _: &Digest,
        digest: &Digest,
    ) -> Result<KagemushaWalletSignerCertificateV1> {
        self.certs.get(digest).copied().ok_or(Error::Unavailable)
    }
    fn fee_inputs(&self, _: &KagemushaWalletFeeClaimV1) -> Result<FeeInputs> {
        if !self.history {
            return Err(Error::Unavailable);
        }
        let values: norito::json::Value = norito::json::from_str(include_str!(
            "../../../../fixtures/kagemusha/wallet_v1_vectors.json"
        ))
        .unwrap();
        let row = values["envelopes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["variant"].as_str() == Some("Request"))
            .unwrap();
        let bytes = hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap();
        let envelope = KagemushaWalletEnvelopeV1::decode_canonical(
            &bytes,
            &self.registration.scheme.scheme_id(),
        )
        .unwrap();
        let KagemushaWalletMessageV1::Request { request } = envelope.message else {
            panic!("request fixture")
        };
        let activation: KagemushaWalletActivationV1 = fixture("KagemushaWalletActivationV1");
        Ok(FeeInputs {
            request,
            payer: activation.credential,
            certificates: activation.certificates,
        })
    }
    fn apply(&mut self, batch: Batch) -> Result<()> {
        if self.unavailable {
            return Err(Error::Unavailable);
        }
        // The in-memory adapter is test-only. Check the complete batch before any mutation.
        let mut balances = self.balances.clone();
        for transfer in batch.transfers {
            let debit = balances.entry(transfer.from).or_default();
            *debit = debit
                .checked_sub(transfer.amount)
                .ok_or(Error::InsufficientFunds)?;
            let credit = balances.entry(transfer.to).or_default();
            *credit = credit.checked_add(transfer.amount).ok_or(Error::Overflow)?;
        }
        self.balances = balances;
        if let Some((key, record)) = batch.wallet {
            self.wallets.insert(key, record);
        }
        if let Some(issue) = batch.issuance {
            self.issues
                .insert((issue.command.wallet, issue.command.request_id), issue);
        }
        if let Some(payout) = batch.payout {
            self.payouts.insert(payout.key, payout);
        }
        self.writes += 1;
        Ok(())
    }
}
struct Finalized(Option<Issuance>);
impl FinalizedSource for Finalized {
    fn issuance(&self, _: &Digest, _: &Digest, _: &Digest) -> Result<Issuance> {
        self.0.clone().ok_or(Error::NotFinalized)
    }
}
fn voucher(issue: &Issuance, tx: &Memory) -> KagemushaWalletLoadVoucherV1 {
    let signature: Signature = signing(0x34).sign(&issue.body.signing_message());
    KagemushaWalletLoadVoucherV1::sign(
        issue.body,
        &tx.registration.load_authorizer,
        KagemushaWalletSignerOutputV1::Der(signature.to_der().as_bytes()),
    )
    .unwrap()
}

pub(super) fn alternative_voucher(issue: &Issuance, tx: &Memory) -> KagemushaWalletLoadVoucherV1 {
    use p256::ecdsa::signature::RandomizedSigner as _;
    // Fixed extra RFC6979 entropy is strictly test-only and produces a second valid signature.
    struct TestEntropy;
    impl rand_core_06::RngCore for TestEntropy {
        fn next_u32(&mut self) -> u32 {
            0x61616161
        }
        fn next_u64(&mut self) -> u64 {
            0x6161616161616161
        }
        fn fill_bytes(&mut self, bytes: &mut [u8]) {
            bytes.fill(0x61);
        }
        fn try_fill_bytes(
            &mut self,
            bytes: &mut [u8],
        ) -> std::result::Result<(), rand_core_06::Error> {
            self.fill_bytes(bytes);
            Ok(())
        }
    }
    impl rand_core_06::CryptoRng for TestEntropy {}
    let signature: Signature =
        signing(0x34).sign_with_rng(&mut TestEntropy, &issue.body.signing_message());
    KagemushaWalletLoadVoucherV1::sign(
        issue.body,
        &tx.registration.load_authorizer,
        KagemushaWalletSignerOutputV1::Der(signature.to_der().as_bytes()),
    )
    .unwrap()
}

#[test]
fn activation_requires_native_proof_and_exact_retry_never_reopens_closed() {
    let mut tx = Memory::new();
    let activation = fixture("KagemushaWalletActivationV1");
    let reject = Verifier::new(false);
    assert!(matches!(
        activate(&mut tx, &reject, &activation),
        Err(Error::Proof)
    ));
    assert_eq!(reject.calls.get(), 1);
    assert!(tx.wallets.is_empty());
    let accept = Verifier::new(true);
    activate(&mut tx, &accept, &activation).unwrap();
    let wallet = activation.credential.body.wallet_id;
    tx.wallets.get_mut(&wallet).unwrap().phase = Phase::Closed;
    activate(&mut tx, &accept, &activation).unwrap();
    assert_eq!(tx.wallets[&wallet].phase, Phase::Closed);
    assert_eq!(accept.calls.get(), 2);
    assert_eq!(tx.writes, 1);
}
#[test]
fn loads_are_successive_reserve_backed_and_exactly_retried_after_close() {
    let mut tx = Memory::new();
    let mut command = tx.active();
    let first = issue_load(&mut tx, &command).unwrap();
    assert_eq!(first.body.ordinal, 0);
    assert_eq!(first.body.amount, 100);
    assert_eq!(tx.balances[&tx.authority], 900);
    assert_eq!(tx.balances[&tx.registration.reserve], 1100);
    assert_eq!(issue_load(&mut tx, &command).unwrap(), first);
    assert_eq!(tx.writes, 2);
    command.request_id = [11; 32];
    assert_eq!(issue_load(&mut tx, &command).unwrap().body.ordinal, 1);
    tx.wallets.get_mut(&command.wallet).unwrap().phase = Phase::Closed;
    assert_eq!(issue_load(&mut tx, &command).unwrap().body.ordinal, 1);
    command.request_id = [12; 32];
    assert!(matches!(
        issue_load(&mut tx, &command),
        Err(Error::Lifecycle)
    ));
}
#[test]
fn changed_retries_storage_failure_overflow_and_unfunded_loads_do_not_mutate() {
    let mut tx = Memory::new();
    let mut command = tx.active();
    issue_load(&mut tx, &command).unwrap();
    command.amount += 1;
    assert!(matches!(
        issue_load(&mut tx, &command),
        Err(Error::Conflict)
    ));
    command.request_id = [12; 32];
    command.amount = 10_000;
    assert!(matches!(
        issue_load(&mut tx, &command),
        Err(Error::InsufficientFunds)
    ));
    assert_eq!(tx.wallets[&command.wallet].next_load, 1);
    assert_eq!(tx.issues.len(), 1);
    tx.wallets.get_mut(&command.wallet).unwrap().next_load = u128::MAX;
    assert!(matches!(
        issue_load(&mut tx, &command),
        Err(Error::Overflow)
    ));
    tx.unavailable = true;
    assert!(matches!(
        issue_load(&mut tx, &command),
        Err(Error::Unavailable)
    ));
    assert_eq!(tx.writes, 2);
}
#[test]
fn voucher_publication_requires_exact_finalized_source_and_freezes_bytes() {
    let mut tx = Memory::new();
    let command = tx.active();
    let issue = issue_load(&mut tx, &command).unwrap();
    let signed = voucher(&issue, &tx);
    assert!(matches!(
        publish_voucher(
            &mut tx,
            &Finalized(None),
            command.scheme,
            command.wallet,
            command.request_id,
            &signed
        ),
        Err(Error::NotFinalized)
    ));
    assert!(tx.issues.values().next().unwrap().voucher.is_none());
    let mut wrong = issue.clone();
    wrong.body.block_height += 1;
    assert!(matches!(
        publish_voucher(
            &mut tx,
            &Finalized(Some(wrong)),
            command.scheme,
            command.wallet,
            command.request_id,
            &signed
        ),
        Err(Error::Binding)
    ));
    let alternate = alternative_voucher(&issue, &tx);
    let source = Finalized(Some(issue));
    let bytes = publish_voucher(
        &mut tx,
        &source,
        command.scheme,
        command.wallet,
        command.request_id,
        &signed,
    )
    .unwrap();
    assert_eq!(bytes, signed.to_canonical_bytes().unwrap());
    assert_eq!(
        publish_voucher(
            &mut tx,
            &source,
            command.scheme,
            command.wallet,
            command.request_id,
            &signed
        )
        .unwrap(),
        bytes
    );
    assert_eq!(tx.writes, 3);
    assert_ne!(alternate.to_canonical_bytes().unwrap(), bytes);
    assert!(matches!(
        publish_voucher(
            &mut tx,
            &source,
            command.scheme,
            command.wallet,
            command.request_id,
            &alternate
        ),
        Err(Error::Conflict)
    ));
    assert_eq!(tx.writes, 3);
}
#[test]
fn closing_requires_native_proof_and_no_outstanding_ordinal() {
    let mut tx = Memory::new();
    let command = tx.active();
    let close: KagemushaWalletCloseLoadsV1 = fixture("KagemushaWalletCloseLoadsV1");
    // Fixture closure may prove a later head; seed only the isolated ledger ordinal for this test.
    tx.wallets.insert(
        close.credential.body.wallet_id,
        WalletRecord {
            asset: close.credential.body.asset_digest,
            phase: Phase::Active,
            activation: [1; 32],
            next_load: close.package.statement.next_load + 1,
        },
    );
    assert!(matches!(
        close_loads(&mut tx, &Verifier::new(false), &close),
        Err(Error::Proof)
    ));
    assert!(matches!(
        close_loads(&mut tx, &Verifier::new(true), &close),
        Err(Error::OutstandingLoad)
    ));
    tx.wallets
        .get_mut(&close.credential.body.wallet_id)
        .unwrap()
        .next_load = close.package.statement.next_load;
    close_loads(&mut tx, &Verifier::new(true), &close).unwrap();
    close_loads(&mut tx, &Verifier::new(true), &close).unwrap();
    assert_eq!(
        tx.wallets[&close.credential.body.wallet_id].phase,
        Phase::Closed
    );
    assert_eq!(
        tx.wallets[&command.wallet].next_load,
        if command.wallet == close.credential.body.wallet_id {
            close.package.statement.next_load
        } else {
            0
        }
    );
}
#[test]
fn abandonment_is_permanent_and_cannot_replace_activation() {
    let mut tx = Memory::new();
    let abandonment: KagemushaWalletAbandonmentV1 = fixture("KagemushaWalletAbandonmentV1");
    abandon(&mut tx, &abandonment).unwrap();
    abandon(&mut tx, &abandonment).unwrap();
    assert_eq!(tx.writes, 1);
    let wallet = abandonment.control.body.wallet_id;
    assert_eq!(tx.wallets[&wallet].phase, Phase::Abandoned);
    tx.wallets.get_mut(&wallet).unwrap().phase = Phase::Active;
    assert!(matches!(
        abandon(&mut tx, &abandonment),
        Err(Error::Lifecycle)
    ));
}
#[test]
fn unload_pays_once_requires_proof_even_on_retry_and_keeps_full_value() {
    let mut tx = Memory::new();
    let claim: KagemushaWalletUnloadClaimV1 = fixture("KagemushaWalletUnloadClaimV1");
    let before = tx.balances[&tx.registration.reserve];
    assert!(matches!(
        pay_unload(&mut tx, &Verifier::new(false), &claim),
        Err(Error::Proof)
    ));
    assert_eq!(tx.balances[&tx.registration.reserve], before);
    let verifier = Verifier::new(true);
    let payout = pay_unload(&mut tx, &verifier, &claim).unwrap();
    assert_eq!(
        tx.balances[&tx.registration.reserve],
        before - payout.amount
    );
    assert_eq!(pay_unload(&mut tx, &verifier, &claim).unwrap(), payout);
    assert_eq!(tx.writes, 1);
    assert_eq!(verifier.calls.get(), 2);
    assert!(matches!(
        pay_unload(&mut tx, &Verifier::new(false), &claim),
        Err(Error::Proof)
    ));
    tx.payouts.get_mut(&payout.key).unwrap().source = [0; 32];
    assert!(matches!(
        pay_unload(&mut tx, &verifier, &claim),
        Err(Error::Conflict)
    ));
}
#[test]
fn fee_claim_cannot_substitute_caller_records_for_missing_history() {
    let mut tx = Memory::new();
    let claim = fixture("KagemushaWalletFeeClaimV1");
    let verifier = Verifier::new(true);
    assert!(matches!(
        pay_fee(&mut tx, &verifier, &claim),
        Err(Error::Unavailable)
    ));
    assert_eq!(verifier.calls.get(), 0);
    assert!(tx.payouts.is_empty());
}
#[test]
fn permanent_records_roundtrip_through_norito() {
    let mut tx = Memory::new();
    let command = tx.active();
    let issue = issue_load(&mut tx, &command).unwrap();
    let bytes = norito::to_bytes(&issue).unwrap();
    let decoded: Issuance =
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
            .unwrap();
    assert_eq!(decoded, issue);
}

#[test]
fn fee_claim_uses_historical_schedule_and_retries_original_payment() {
    let mut tx = Memory::new();
    tx.history = true;
    let claim: KagemushaWalletFeeClaimV1 = fixture("KagemushaWalletFeeClaimV1");
    let before = tx.balances[&tx.registration.reserve];
    let verifier = Verifier::new(true);
    let payout = pay_fee(&mut tx, &verifier, &claim).unwrap();
    assert_eq!(
        tx.balances[&tx.registration.reserve],
        before - payout.amount
    );
    assert_eq!(tx.balances[&claim.beneficiary], payout.amount);
    assert_eq!(pay_fee(&mut tx, &verifier, &claim).unwrap(), payout);
    assert_eq!(tx.writes, 1);
    assert_eq!(verifier.calls.get(), 2);
    assert!(matches!(
        pay_fee(&mut tx, &Verifier::new(false), &claim),
        Err(Error::Proof)
    ));
    tx.history = false;
    assert!(matches!(
        pay_fee(&mut tx, &verifier, &claim),
        Err(Error::Unavailable)
    ));
}
#[test]
fn load_charge_is_separate_and_whole_batch_failure_keeps_both_balances() {
    let mut tx = Memory::new();
    let mut command = tx.active();
    let quote: KagemushaWalletChargeQuoteV1 = fixture("KagemushaWalletChargeQuoteV1");
    let claim: KagemushaWalletFeeClaimV1 = fixture("KagemushaWalletFeeClaimV1");
    let cert: KagemushaWalletSignerCertificateV1 = fixture("KagemushaWalletSignerCertificateV1");
    tx.certs.insert(cert.certificate_digest(), cert);
    command.wallet = quote.body.wallet_id;
    tx.wallets.insert(
        command.wallet,
        WalletRecord {
            asset: quote.body.asset_digest,
            phase: Phase::Active,
            activation: [1; 32],
            next_load: quote.body.ordinal,
        },
    );
    command.amount = quote.body.net_amount;
    command.charge = Some(LoadCharge {
        quote,
        beneficiary: claim.beneficiary.clone(),
    });
    tx.balances.insert(tx.authority.clone(), command.amount);
    let before = tx.balances.clone();
    assert!(matches!(
        issue_load(&mut tx, &command),
        Err(Error::InsufficientFunds)
    ));
    assert_eq!(tx.balances, before);
    assert!(tx.issues.is_empty());
    tx.balances.insert(
        tx.authority.clone(),
        command.amount + quote.body.online_charge,
    );
    let reserve = tx.balances[&tx.registration.reserve];
    let result = issue_load(&mut tx, &command).unwrap();
    assert_eq!(result.body.online_charge, quote.body.online_charge);
    assert_eq!(
        tx.balances[&tx.registration.reserve],
        reserve + command.amount
    );
    assert_eq!(tx.balances[&claim.beneficiary], quote.body.online_charge);
    assert_eq!(tx.balances[&tx.authority], 0);
}
#[test]
fn stored_rows_reject_wrong_keys_unknown_tags_and_corrupt_canonical_bytes() {
    let mut tx = Memory::new();
    let command = tx.active();
    let issue = issue_load(&mut tx, &command).unwrap();
    let encoded = storage::encode(&issue).unwrap();
    let key = storage::issuance_key(command.scheme, command.wallet, command.request_id);
    validate_row(&key, &encoded).unwrap();
    assert!(validate_row(&storage::key(255, command.scheme, command.wallet), &encoded).is_err());
    assert!(
        validate_row(
            &storage::issuance_key(command.scheme, [99; 32], command.request_id),
            &encoded
        )
        .is_err()
    );
    assert!(validate_row(&key, &encoded[..encoded.len() - 1]).is_err());
    let wallet = tx.wallets[&command.wallet];
    let encoded = storage::encode(&wallet).unwrap();
    validate_row(
        &storage::key(storage::WALLET, command.scheme, command.wallet),
        &encoded,
    )
    .unwrap();
    let encoded = storage::encode(&tx.registration).unwrap();
    validate_row(
        &storage::key(
            storage::REGISTRATION,
            command.scheme,
            tx.registration.asset.asset_digest(),
        ),
        &encoded,
    )
    .unwrap();
}

#[test]
fn world_ledger_table_participates_in_atomic_rollback_and_committed_views() {
    use mv::storage::StorageReadOnly as _;
    let mut tx = Memory::new();
    let command = tx.active();
    let key = storage::key(storage::WALLET, command.scheme, command.wallet);
    let bytes = storage::encode(&tx.wallets[&command.wallet]).unwrap();
    let world = crate::state::World::default();
    {
        let mut block = world.block();
        block.kagemusha_wallet_ledger.insert(key, bytes.clone());
        assert_eq!(block.kagemusha_wallet_ledger.get(&key), Some(&bytes));
    }
    assert!(world.kagemusha_wallet_ledger.view().get(&key).is_none());
    {
        let mut block = world.block();
        block.kagemusha_wallet_ledger.insert(key, bytes.clone());
        block.commit();
    }
    assert_eq!(world.kagemusha_wallet_ledger.view().get(&key), Some(&bytes));
    let json = norito::json::to_json(&world.kagemusha_wallet_ledger).unwrap();
    let restored: mv::storage::Storage<LedgerKey, Vec<u8>> = norito::json::from_str(&json).unwrap();
    assert_eq!(restored.view().get(&key), Some(&bytes));
    assert!(restored.block_and_revert().get(&key).is_none());
}
#[test]
fn ledger_key_encoding_is_bounded_and_rejects_aliases() {
    use norito::json::JsonKeyCodec as _;
    let key = storage::issuance_key([0xab; 32], [0xcd; 32], [0xef; 32]);
    let mut encoded = String::new();
    key.encode_json_key(&mut encoded);
    let unquoted: String = norito::json::from_str(&encoded).unwrap();
    assert_eq!(unquoted.len(), 194);
    assert_eq!(LedgerKey::decode_json_key(&unquoted).unwrap(), key);
    assert!(LedgerKey::decode_json_key(&unquoted.to_lowercase()).is_err());
    assert!(LedgerKey::decode_json_key(&unquoted[..193]).is_err());
    let storage: mv::storage::Storage<LedgerKey, Vec<u8>> =
        [(key, vec![1, 2, 3])].into_iter().collect();
    let ordinary = norito::json::to_json(&storage).unwrap();
    assert_eq!(
        norito::json::to_json_bounded(&storage, ordinary.len()).unwrap(),
        ordinary
    );
    assert!(norito::json::to_json_bounded(&storage, ordinary.len() - 1).is_err());
}
#[test]
fn committed_world_row_without_transaction_membership_is_not_finality() {
    let mut tx = Memory::new();
    let command = tx.active();
    let issue = issue_load(&mut tx, &command).unwrap();
    let world = crate::state::World::default();
    let key = storage::issuance_key(command.scheme, command.wallet, command.request_id);
    {
        let mut block = world.block();
        block
            .kagemusha_wallet_ledger
            .insert(key, storage::encode(&issue).unwrap());
        block.commit();
    }
    let state = crate::state::State::new(
        world,
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    let view = state.view();
    assert!(matches!(
        FinalizedLedger::new(
            &view,
            iroha_data_model::sumeragi::finality::NativeFinalityLimits {
                block_bytes: 1_000_000,
                journal_bytes: 4_000_000,
                block_count: 8,
                allocated_bytes: 8_000_000
            }
        ),
        Err(Error::NotFinalized)
    ));
}

mod wsv_tests;
