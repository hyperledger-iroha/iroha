//! Real private-file/atomic-write controls. Native execution qualification is separate.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    AccountId,
    block::BlockHeader,
    private_transaction_counters::{
        CounterPurposeV1, PRIVATE_COUNTER_REQUEST_DOMAIN_V1, PrivateCountersRequestV1,
    },
};
use iroha_model_base::topology::DataSpaceId;
use std::num::NonZeroU64;

use crate::sumeragi::{
    driver::traits::{LogEntry, RecordStore},
    records::FreshKeyAssertion,
};

fn signed(seed: u8, time_ms: u64, nonce: [u8; 32]) -> SignedPrivateCountersRequestV1 {
    let key = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
    PrivateCountersRequestV1 {
        domain: PRIVATE_COUNTER_REQUEST_DOMAIN_V1,
        version: 1,
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"private counter durable replay fixture",
        ))),
        scope: SumeragiRootScope::Dataspace {
            parent_network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::new(b"independent parent"),
            )),
            dataspace_id: DataSpaceId::new(6647857470246403404),
        },
        authority: AccountId::new(key.public_key().clone()),
        purpose: CounterPurposeV1::WalkthroughInteractions,
        policy_hash: Hash::new(b"retained policy"),
        manifest_hash: Hash::new(b"retained manifest"),
        cut: CounterCutV1 {
            height: 2,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"fixture cut")),
            context_id: Hash::new(b"fixture native context"),
            world_root: Hash::new(b"fixture world"),
            epoch_context_id: [0x44; 32],
        },
        creation_time_ms: time_ms,
        time_to_live_ms: NonZeroU64::new(10_000).unwrap(),
        nonce,
    }
    .try_sign(&key)
    .unwrap()
}

struct Fixture {
    _directory: tempfile::TempDir,
    records: FileRecordStore,
    key: iroha_sumeragi::types::PublicKey,
    binding: ReplayBinding,
    budget: AllocationBudget,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let records = FileRecordStore::open(
            directory.path().join("records"),
            directory.path().join("installation.log"),
        )
        .unwrap();
        let key = crate::sumeragi::crypto::core_key(
            KeyPair::from_seed(vec![0xA1; 32], Algorithm::BlsNormal).public_key(),
        )
        .unwrap();
        let request = signed(91, 10_000, [1; 32]);
        let binding = ReplayBinding {
            instance: [0x11; 32],
            network_id: request.payload.network_id,
            scope: request.payload.scope,
            installed_policy_authority: Hash::new(b"independently installed authority"),
            installed_key: Hash::new(key.as_bytes()),
        };
        Self {
            _directory: directory,
            records,
            key,
            binding,
            budget: AllocationBudget::new(32 * 1024 * 1024),
        }
    }

    fn first_open(&self) -> ReplayOwner {
        let assertion = FreshKeyAssertion::from_operator_flag(true).unwrap();
        let permit = self
            .records
            .private_counter_first_installation(
                &Hash32(self.binding.instance),
                &self.key,
                Some(&assertion),
            )
            .unwrap()
            .unwrap();
        let owner = ReplayOwner::open(
            &self.records,
            self.binding,
            Some(permit),
            10_000,
            &self.budget,
        )
        .unwrap();
        // Native startup writes this durable event after ledger initialization, before serving.
        self.records.set_store_id(0x1234).unwrap();
        self.records
            .append_log(&LogEntry::Instance {
                instance: Hash32(self.binding.instance),
                key: self.key.clone(),
                store_id: 0x1234,
            })
            .unwrap();
        owner
    }

    fn reopen(&self, now_ms: u64) -> Result<ReplayOwner, PrivateCountersErrorV1> {
        // Even an accidentally repeated fresh-key flag cannot recreate known-instance custody.
        let assertion = FreshKeyAssertion::from_operator_flag(true).unwrap();
        let permit = self
            .records
            .private_counter_first_installation(
                &Hash32(self.binding.instance),
                &self.key,
                Some(&assertion),
            )
            .unwrap();
        assert!(permit.is_none());
        ReplayOwner::open(&self.records, self.binding, permit, now_ms, &self.budget)
    }
}

#[test]
fn consumed_future_dated_original_and_resigned_nonce_refuse_after_same_tick_and_later_restart() {
    let fixture = Fixture::new();
    let mut owner = fixture.first_open();
    // This was admissible before restart under the actual finite 5s skew rule.
    let original = signed(91, 14_000, [1; 32]);
    original.verify_signature().unwrap();
    owner.check_request(&original, 10_000).unwrap();
    owner.consume(&original).unwrap();
    assert_eq!(
        owner.ledger.consumed[0].request_hash,
        original.original_hash().unwrap()
    );
    assert_eq!(owner.ledger.consumed[0].cut, original.payload.cut);
    drop(owner);
    let owner = fixture.reopen(10_000).unwrap();
    assert_eq!(
        owner.check_request(&original, 10_000),
        Err(PrivateCountersErrorV1::Replay)
    );
    drop(owner);
    let owner = fixture.reopen(12_000).unwrap();
    assert_eq!(
        owner.check_request(&original, 12_000),
        Err(PrivateCountersErrorV1::Replay)
    );
    let resigned = signed(91, 12_000, [1; 32]);
    assert_eq!(
        owner.check_request(&resigned, 12_000),
        Err(PrivateCountersErrorV1::Replay)
    );
    owner
        .check_request(&signed(92, 12_000, [1; 32]), 12_000)
        .unwrap();
}

#[test]
fn changed_cut_of_signed_same_reader_nonce_cannot_bypass_durable_consumption() {
    let fixture = Fixture::new();
    let mut owner = fixture.first_open();
    let original = signed(91, 10_000, [2; 32]);
    owner.consume(&original).unwrap();
    let key = KeyPair::from_seed(vec![91; 32], Algorithm::Ed25519);
    let mut payload = original.payload.clone();
    payload.cut.block_hash = HashOf::from_untyped_unchecked(Hash::new(b"another genuine cut"));
    payload.cut.context_id = Hash::new(b"another context");
    let rebound = payload.try_sign(&key).unwrap();
    assert_ne!(
        rebound.original_hash().unwrap(),
        original.original_hash().unwrap()
    );
    assert_eq!(
        owner.check_request(&rebound, 10_000),
        Err(PrivateCountersErrorV1::Replay)
    );
}

#[test]
fn persisted_clock_high_water_refuses_rollback_after_restart_and_prunes_only_after_expiry() {
    let fixture = Fixture::new();
    let mut owner = fixture.first_open();
    let original = signed(91, 10_000, [3; 32]);
    owner.consume(&original).unwrap();
    owner.observe_clock(25_000).unwrap();
    assert_eq!(owner.consumed_len(), 1);
    drop(owner);
    assert!(matches!(
        fixture.reopen(24_999),
        Err(PrivateCountersErrorV1::Freshness)
    ));
    let mut owner = fixture.reopen(25_000).unwrap();
    assert_eq!(
        owner.check_request(&original, 25_000),
        Err(PrivateCountersErrorV1::Replay)
    );
    owner.observe_clock(25_001).unwrap();
    assert_eq!(owner.consumed_len(), 0);
    assert_eq!(
        owner.check_request(&original, 25_001),
        Err(PrivateCountersErrorV1::Freshness)
    );
}

#[test]
fn live_saturation_never_evicts_and_expired_capacity_is_durable_before_reuse() {
    let fixture = Fixture::new();
    let mut owner = fixture.first_open();
    // Build one exact bounded ledger rather than performing 1024 independent fsyncs.
    for ordinal in 0..MAX_REPLAY_ENTRIES {
        let mut nonce = [0x55; 32];
        nonce[..8].copy_from_slice(&(ordinal as u64).to_le_bytes());
        let request = signed(91, 10_000, nonce);
        owner.ledger.consumed.push(ConsumedNonce {
            authority: Hash::from(HashOf::new(&request.payload.authority)),
            nonce,
            request_hash: request.original_hash().unwrap(),
            cut: request.payload.cut,
            creation_time_ms: 10_000,
            time_to_live_ms: 10_000,
            expires_at_ms: 25_000,
        });
    }
    owner.persist().unwrap();
    let newcomer = signed(91, 10_000, [0x77; 32]);
    assert_eq!(
        owner.consume(&newcomer),
        Err(PrivateCountersErrorV1::Bounds)
    );
    assert_eq!(owner.consumed_len(), MAX_REPLAY_ENTRIES);
    drop(owner);
    let mut owner = fixture.reopen(25_001).unwrap();
    assert_eq!(owner.consumed_len(), 0);
    owner.consume(&signed(91, 25_001, [0x77; 32])).unwrap();
    drop(owner);
    let owner = fixture.reopen(25_001).unwrap();
    assert_eq!(owner.consumed_len(), 1);
}

#[test]
fn missing_ledger_or_complete_child_on_known_instance_never_reinitializes() {
    for remove_child in [false, true] {
        let fixture = Fixture::new();
        let owner = fixture.first_open();
        let path = owner.directory.path().to_owned();
        drop(owner);
        if remove_child {
            std::fs::remove_dir_all(&path).unwrap();
        } else {
            std::fs::remove_file(path.join(LEDGER_FILE)).unwrap();
        }
        assert!(matches!(
            fixture.reopen(10_000),
            Err(PrivateCountersErrorV1::Unavailable)
        ));
    }
}

#[test]
fn competing_owner_corrupt_original_and_foreign_installed_context_refuse() {
    let fixture = Fixture::new();
    let owner = fixture.first_open();
    assert!(matches!(
        fixture.reopen(10_000),
        Err(PrivateCountersErrorV1::Unavailable)
    ));
    let path = owner.directory.path().to_owned();
    drop(owner);
    let mut foreign = fixture.binding;
    foreign.installed_policy_authority = Hash::new(b"offered authority");
    assert!(matches!(
        ReplayOwner::open(&fixture.records, foreign, None, 10_000, &fixture.budget),
        Err(PrivateCountersErrorV1::Unavailable)
    ));
    let directory = PrivateDirectory::open(path).unwrap();
    let mut bytes = directory
        .read(LEDGER_FILE, MAX_LEDGER_BYTES)
        .unwrap()
        .to_vec();
    bytes.push(0);
    directory
        .write_atomic(LEDGER_FILE, &bytes, PublishMode::Replace)
        .unwrap();
    assert!(matches!(
        fixture.reopen(10_000),
        Err(PrivateCountersErrorV1::Unavailable)
    ));
}

#[test]
fn publication_refusal_poison_prevents_signature_retry_and_durable_ambiguous_consume_recovers() {
    for failure in [
        PersistenceFailure::BeforePublication,
        PersistenceFailure::AfterPublication,
    ] {
        let fixture = Fixture::new();
        let mut owner = fixture.first_open();
        let original = signed(91, 10_000, [4; 32]);
        owner.failure = Some(failure);
        assert_eq!(
            owner.consume(&original),
            Err(PrivateCountersErrorV1::Unavailable)
        );
        assert!(owner.poisoned);
        assert_eq!(
            owner.consume(&original),
            Err(PrivateCountersErrorV1::Unavailable)
        );
        drop(owner);
        let owner = fixture.reopen(10_000).unwrap();
        if failure == PersistenceFailure::AfterPublication {
            assert_eq!(
                owner.check_request(&original, 10_000),
                Err(PrivateCountersErrorV1::Replay)
            );
        } else {
            // Nothing could be signed after the failed durable-consume admission.
            owner.check_request(&original, 10_000).unwrap();
        }
    }
}

#[test]
fn changed_lock_identity_and_oversized_ledger_refuse_without_custody_reset() {
    let fixture = Fixture::new();
    let mut owner = fixture.first_open();
    let path = owner.directory.path().to_owned();
    std::fs::rename(path.join(LOCK_FILE), path.join("replaced.lock")).unwrap();
    owner.directory.create_lock(LOCK_FILE).unwrap();
    assert_eq!(
        owner.consume(&signed(91, 10_000, [5; 32])),
        Err(PrivateCountersErrorV1::Unavailable)
    );
    drop(owner);
    // Unknown inventory already refuses recovery; an oversized named original also refuses read.
    let directory = PrivateDirectory::open(path).unwrap();
    directory
        .write_atomic(
            LEDGER_FILE,
            &vec![0; MAX_LEDGER_BYTES + 1],
            PublishMode::Replace,
        )
        .unwrap();
    assert!(matches!(
        fixture.reopen(10_000),
        Err(PrivateCountersErrorV1::Unavailable)
    ));
}

#[test]
fn generated_key_fresh_provenance_and_existing_safety_record_gate_first_installation() {
    use crate::sumeragi::records::register_generated_key;
    let fixture = Fixture::new();
    assert!(
        fixture
            .records
            .private_counter_first_installation(
                &Hash32(fixture.binding.instance),
                &fixture.key,
                None
            )
            .unwrap()
            .is_none()
    );
    register_generated_key(&fixture.records, &fixture.key).unwrap();
    assert!(
        fixture
            .records
            .private_counter_first_installation(
                &Hash32(fixture.binding.instance),
                &fixture.key,
                None
            )
            .unwrap()
            .is_some()
    );
    // Existing original safety custody denies initial ledger recreation even before Instance log.
    fixture
        .records
        .write(
            &Hash32(fixture.binding.instance),
            &fixture.key,
            b"retained native safety record",
        )
        .unwrap();
    let assertion = FreshKeyAssertion::from_operator_flag(true).unwrap();
    assert!(
        fixture
            .records
            .private_counter_first_installation(
                &Hash32(fixture.binding.instance),
                &fixture.key,
                Some(&assertion)
            )
            .unwrap()
            .is_none()
    );
}

#[test]
fn mismatched_store_id_and_torn_log_never_supply_initialization_capability() {
    use crate::sumeragi::records::register_generated_key;
    use std::io::Write as _;
    let fixture = Fixture::new();
    register_generated_key(&fixture.records, &fixture.key).unwrap();
    let assertion = FreshKeyAssertion::from_operator_flag(true).unwrap();
    fixture.records.set_store_id(0xDEAD).unwrap();
    assert!(
        fixture
            .records
            .private_counter_first_installation(
                &Hash32(fixture.binding.instance),
                &fixture.key,
                Some(&assertion),
            )
            .unwrap()
            .is_none()
    );
    // Restore the genuine id, then add a torn physical suffix that the legacy reader ignores.
    let current = fixture.records.log().unwrap().last().unwrap().store_id();
    fixture.records.set_store_id(current).unwrap();
    let mut log = std::fs::OpenOptions::new()
        .append(true)
        .open(fixture.records.log_path())
        .unwrap();
    log.write_all(&[0xAA, 0xBB]).unwrap();
    log.sync_all().unwrap();
    assert!(
        fixture
            .records
            .private_counter_first_installation(
                &Hash32(fixture.binding.instance),
                &fixture.key,
                Some(&assertion),
            )
            .unwrap()
            .is_none()
    );
}
