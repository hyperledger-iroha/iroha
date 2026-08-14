use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
use iroha_data_model::{
    NetworkId,
    block::BlockHeader,
    musubi::{
        ArchiveId, MUSUBI_REGISTRY_VERSION_V1, MusubiContentDigestV1,
        MusubiProviderBundleVerificationApprovalV1, MusubiSemanticReleaseDigestV1,
        MusubiVerificationLockDigestV1,
    },
    sorafs::capacity::ProviderId,
    sorafs::pin_registry::{ProviderIngestCompletionAuthorityV1, ProviderIngestFinalizedAnchorV1},
};
use super::*;
#[derive(Debug, Clone, Copy)]
struct FixedJournalTime(u64);
impl MusubiProviderAttestationJournalTimeV1 for FixedJournalTime {
    fn now_unix_ms<'a>(
        &'a self,
    ) -> ProviderIngestFutureV1<'a, Result<u64, MusubiProviderAttestationJournalErrorV1>> {
        Box::pin(async move {
            if self.0 == 0 {
                Err(MusubiProviderAttestationJournalErrorV1::ClockRollback)
            } else {
                Ok(self.0)
            }
        })
    }
}
const fn clock_at(now_unix_ms: u64) -> FixedJournalTime {
    FixedJournalTime(now_unix_ms)
}
#[test]
fn production_checkpoint_encoding_ignores_ambient_norito_flags() {
    let policy = MusubiProviderAttestationJournalPolicyV1::default();
    let checkpoint = StoredJournalCheckpointV1 {
        version: JOURNAL_CHECKPOINT_VERSION_V1,
        checkpoint_sequence: 1,
        next_intent_sequence: 1,
        last_observed_unix_ms: 0,
        entries: Vec::new(),
    };
    let expected_bytes =
        encode_checkpoint(&checkpoint, policy).expect("encode the canonical production checkpoint");
    let expected_reserve = checkpoint_future_reserve_bytes(&checkpoint)
        .expect("measure the canonical production reserve");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
    assert_eq!(
        encode_checkpoint(&checkpoint, policy)
            .expect("ambient flags cannot change production checkpoint bytes"),
        expected_bytes
    );
    assert_eq!(
        checkpoint_future_reserve_bytes(&checkpoint)
            .expect("ambient flags cannot change production reserve sizing"),
        expected_reserve
    );
}
#[derive(Debug)]
struct SequenceJournalTime {
    samples: Mutex<VecDeque<u64>>,
    sealed_floor: AtomicU64,
}
impl SequenceJournalTime {
    fn new(samples: impl IntoIterator<Item = u64>) -> Self {
        Self {
            samples: Mutex::new(samples.into_iter().collect()),
            sealed_floor: AtomicU64::new(0),
        }
    }
    fn sealed_floor(&self) -> u64 {
        self.sealed_floor.load(Ordering::SeqCst)
    }
}
impl MusubiProviderAttestationJournalTimeV1 for SequenceJournalTime {
    fn now_unix_ms<'a>(
        &'a self,
    ) -> ProviderIngestFutureV1<'a, Result<u64, MusubiProviderAttestationJournalErrorV1>> {
        Box::pin(async move {
            let sampled = self
                .samples
                .lock()
                .map_err(|_| MusubiProviderAttestationJournalErrorV1::ClockUnavailable)?
                .pop_front()
                .ok_or(MusubiProviderAttestationJournalErrorV1::ClockUnavailable)?;
            let floor = self.sealed_floor.load(Ordering::SeqCst);
            if sampled == 0 || sampled < floor {
                return Err(MusubiProviderAttestationJournalErrorV1::ClockRollback);
            }
            self.sealed_floor.store(sampled, Ordering::SeqCst);
            Ok(sampled)
        })
    }
}
#[derive(Debug)]
struct DelayedSecondJournalTime {
    samples: Mutex<VecDeque<u64>>,
    calls: AtomicUsize,
    delay_ms: u64,
}
impl DelayedSecondJournalTime {
    fn new(first: u64, second: u64, delay_ms: u64) -> Self {
        Self {
            samples: Mutex::new(VecDeque::from([first, second])),
            calls: AtomicUsize::new(0),
            delay_ms,
        }
    }
}
impl MusubiProviderAttestationJournalTimeV1 for DelayedSecondJournalTime {
    fn now_unix_ms<'a>(
        &'a self,
    ) -> ProviderIngestFutureV1<'a, Result<u64, MusubiProviderAttestationJournalErrorV1>> {
        Box::pin(async move {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 1 {
                tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
            }
            self.samples
                .lock()
                .map_err(|_| MusubiProviderAttestationJournalErrorV1::ClockUnavailable)?
                .pop_front()
                .ok_or(MusubiProviderAttestationJournalErrorV1::ClockUnavailable)
        })
    }
}
#[derive(Default)]
struct MemoryJournalStore {
    latest: Mutex<MusubiProviderAttestationJournalStoreSnapshotV1>,
}
impl Default for MusubiProviderAttestationJournalStoreSnapshotV1 {
    fn default() -> Self {
        Self::empty()
    }
}
impl MusubiProviderAttestationJournalStoreV1 for MemoryJournalStore {
    fn load<'a>(
        &'a self,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            MusubiProviderAttestationJournalStoreSnapshotV1,
            MusubiProviderAttestationJournalStoreErrorV1,
        >,
    > {
        Box::pin(async move {
            self.latest
                .lock()
                .map(|snapshot| snapshot.clone())
                .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Unavailable)
        })
    }
    fn compare_and_swap<'a>(
        &'a self,
        expected_revision: Option<[u8; 32]>,
        replacement_checkpoint_bytes: Vec<u8>,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            MusubiProviderAttestationJournalCasOutcomeV1,
            MusubiProviderAttestationJournalStoreErrorV1,
        >,
    > {
        Box::pin(async move {
            let replacement =
                MusubiProviderAttestationJournalStoreSnapshotV1::from_checkpoint_bytes(
                    replacement_checkpoint_bytes,
                )?;
            let mut latest = self
                .latest
                .lock()
                .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Unavailable)?;
            let revision = replacement
                .revision()
                .ok_or(MusubiProviderAttestationJournalStoreErrorV1::Rejected)?;
            if *latest == replacement {
                return Ok(MusubiProviderAttestationJournalCasOutcomeV1::Stored { revision });
            }
            if latest.revision() != expected_revision {
                return Ok(MusubiProviderAttestationJournalCasOutcomeV1::Conflict);
            }
            *latest = replacement;
            Ok(MusubiProviderAttestationJournalCasOutcomeV1::Stored { revision })
        })
    }
}
struct MemoryInventory {
    entries: Mutex<Vec<(MusubiProviderAttestationInventoryItemV1, u64)>>,
    put_calls: AtomicUsize,
    get_calls: AtomicUsize,
    inventory_calls: AtomicUsize,
    runtime_handle_calls: AtomicUsize,
    qualification_calls: AtomicUsize,
    readiness_calls: AtomicUsize,
    delay_ms: AtomicU64,
    get_delay_ms: AtomicU64,
    omit_readback: AtomicBool,
    fail_after_put_once: AtomicBool,
    readback_revision_override: AtomicU64,
    readback_item_override: Mutex<Option<MusubiProviderAttestationInventoryItemV1>>,
    invalid_runtime_handle: AtomicBool,
    test_runtime_handle: AtomicBool,
    drifted_runtime_handle: AtomicBool,
    drift_handle_after_put: AtomicBool,
    drift_handle_after_get: AtomicBool,
    adapter_revision: AtomicU64,
    policy_digest: Mutex<[u8; 32]>,
    drift_qualification_after_put: AtomicBool,
    drift_qualification_after_get: AtomicBool,
    qualification_error: Mutex<Option<MusubiProviderAttestationInventoryRuntimeErrorV1>>,
    readiness_error: Mutex<Option<MusubiProviderAttestationInventoryRuntimeErrorV1>>,
}
impl Default for MemoryInventory {
    fn default() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
            put_calls: AtomicUsize::new(0),
            get_calls: AtomicUsize::new(0),
            inventory_calls: AtomicUsize::new(0),
            runtime_handle_calls: AtomicUsize::new(0),
            qualification_calls: AtomicUsize::new(0),
            readiness_calls: AtomicUsize::new(0),
            delay_ms: AtomicU64::new(0),
            get_delay_ms: AtomicU64::new(0),
            omit_readback: AtomicBool::new(false),
            fail_after_put_once: AtomicBool::new(false),
            readback_revision_override: AtomicU64::new(0),
            readback_item_override: Mutex::new(None),
            invalid_runtime_handle: AtomicBool::new(false),
            test_runtime_handle: AtomicBool::new(false),
            drifted_runtime_handle: AtomicBool::new(false),
            drift_handle_after_put: AtomicBool::new(false),
            drift_handle_after_get: AtomicBool::new(false),
            adapter_revision: AtomicU64::new(1),
            policy_digest: Mutex::new([0xB7; 32]),
            drift_qualification_after_put: AtomicBool::new(false),
            drift_qualification_after_get: AtomicBool::new(false),
            qualification_error: Mutex::new(None),
            readiness_error: Mutex::new(None),
        }
    }
}
impl MusubiProviderAttestationInventorySinkV1 for MemoryInventory {
    fn put<'a>(
        &'a self,
        item: MusubiProviderAttestationInventoryItemV1,
    ) -> ProviderIngestFutureV1<'a, Result<u64, MusubiProviderAttestationInventoryErrorV1>> {
        Box::pin(async move {
            self.put_calls.fetch_add(1, Ordering::SeqCst);
            let delay_ms = self.delay_ms.load(Ordering::SeqCst);
            if delay_ms != 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
            item.validate()?;
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Unavailable)?;
            let revision = if let Some((retained, revision)) = entries
                .iter()
                .find(|(retained, _)| retained.scope == item.scope && retained.key == item.key)
            {
                if retained != &item {
                    return Err(MusubiProviderAttestationInventoryErrorV1::Conflict);
                }
                *revision
            } else {
                let revision = u64::try_from(entries.len())
                    .ok()
                    .and_then(|count| count.checked_add(1))
                    .ok_or(MusubiProviderAttestationInventoryErrorV1::Rejected)?;
                entries.push((item.clone(), revision));
                revision
            };
            if self.fail_after_put_once.swap(false, Ordering::SeqCst) {
                return Err(MusubiProviderAttestationInventoryErrorV1::Unavailable);
            }
            if self.drift_handle_after_put.load(Ordering::SeqCst) {
                self.drifted_runtime_handle.store(true, Ordering::SeqCst);
            }
            if self.drift_qualification_after_put.load(Ordering::SeqCst) {
                self.adapter_revision.fetch_add(1, Ordering::SeqCst);
            }
            Ok(revision)
        })
    }
}
impl MusubiProviderAttestationInventoryReaderV1 for MemoryInventory {
    fn get<'a>(
        &'a self,
        scope: &'a MusubiProviderAttestationInventoryScopeV1,
        key: MusubiProviderBundleAttestationKeyV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            Option<MusubiProviderAttestationInventoryReadbackV1>,
            MusubiProviderAttestationInventoryErrorV1,
        >,
    > {
        Box::pin(async move {
            self.get_calls.fetch_add(1, Ordering::SeqCst);
            let delay_ms = self.get_delay_ms.load(Ordering::SeqCst);
            if delay_ms != 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
            scope.validate()?;
            key.validate()
                .map_err(|_| MusubiProviderAttestationInventoryErrorV1::InvalidItem)?;
            if self.omit_readback.load(Ordering::SeqCst) {
                return Ok(None);
            }
            let entries = self
                .entries
                .lock()
                .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Unavailable)?;
            let retained = entries
                .iter()
                .find(|(item, _)| item.scope == *scope && item.key == key)
                .cloned();
            let Some((retained_item, retained_revision)) = retained else {
                return Ok(None);
            };
            let item = self
                .readback_item_override
                .lock()
                .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Unavailable)?
                .clone()
                .unwrap_or(retained_item);
            let revision_override = self.readback_revision_override.load(Ordering::SeqCst);
            let revision = if revision_override == 0 {
                retained_revision
            } else {
                revision_override
            };
            let readback =
                MusubiProviderAttestationInventoryReadbackV1::try_new(item, revision).map(Some);
            if self.drift_handle_after_get.load(Ordering::SeqCst) {
                self.drifted_runtime_handle.store(true, Ordering::SeqCst);
            }
            if self.drift_qualification_after_get.load(Ordering::SeqCst) {
                self.adapter_revision.fetch_add(1, Ordering::SeqCst);
            }
            readback
        })
    }
    fn inventory<'a>(
        &'a self,
        scope: &'a MusubiProviderAttestationInventoryScopeV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            Option<MusubiProviderAttestationInventoryV1>,
            MusubiProviderAttestationInventoryErrorV1,
        >,
    > {
        Box::pin(async move {
            self.inventory_calls.fetch_add(1, Ordering::SeqCst);
            scope.validate()?;
            let entries = self
                .entries
                .lock()
                .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Unavailable)?;
            let items = entries
                .iter()
                .filter(|(item, _)| item.scope == *scope)
                .map(|(item, _)| item.clone())
                .collect::<Vec<_>>();
            if items.is_empty() {
                Ok(None)
            } else {
                MusubiProviderAttestationInventoryV1::new(scope.clone(), items).map(Some)
            }
        })
    }
}
impl MusubiProviderAttestationInventoryRuntimeV1 for MemoryInventory {
    fn runtime_handle(&self) -> &str {
        self.runtime_handle_calls.fetch_add(1, Ordering::SeqCst);
        if self.invalid_runtime_handle.load(Ordering::SeqCst) {
            ""
        } else if self.test_runtime_handle.load(Ordering::SeqCst) {
            "inventory://sorafs/musubi/test"
        } else if self.drifted_runtime_handle.load(Ordering::SeqCst) {
            "inventory://sorafs/musubi/secondary"
        } else {
            "inventory://sorafs/musubi/primary"
        }
    }
    fn qualification(
        &self,
    ) -> Result<
        MusubiProviderAttestationInventoryQualificationV1,
        MusubiProviderAttestationInventoryRuntimeErrorV1,
    > {
        self.qualification_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = *self
            .qualification_error
            .lock()
            .map_err(|_| MusubiProviderAttestationInventoryRuntimeErrorV1::Unavailable)?
        {
            return Err(error);
        }
        Ok(MusubiProviderAttestationInventoryQualificationV1::new(
            self.adapter_revision.load(Ordering::SeqCst),
            *self
                .policy_digest
                .lock()
                .map_err(|_| MusubiProviderAttestationInventoryRuntimeErrorV1::Unavailable)?,
        ))
    }
    fn check_readiness<'a>(
        &'a self,
    ) -> ProviderIngestFutureV1<'a, Result<(), MusubiProviderAttestationInventoryRuntimeErrorV1>>
    {
        Box::pin(async move {
            self.readiness_calls.fetch_add(1, Ordering::SeqCst);
            match *self
                .readiness_error
                .lock()
                .map_err(|_| MusubiProviderAttestationInventoryRuntimeErrorV1::Unavailable)?
            {
                Some(error) => Err(error),
                None => Ok(()),
            }
        })
    }
}
struct Fixture {
    request: ProviderIngestMusubiAttestationApprovalRequestV1,
    owner_key: KeyPair,
}
fn signer_policy(revision: u64) -> ProviderIngestCompletionSignerPolicyV1 {
    ProviderIngestCompletionSignerPolicyV1 {
        policy_id: [0x31; 32],
        revision,
        predecessor_digest: (revision > 1).then_some([0x32; 32]),
        policy_digest: [u8::try_from(0x40 + revision).expect("small revision"); 32],
    }
}
fn fixture(provider_seed: u8, claim_seed: u8) -> Fixture {
    fixture_with_provider([provider_seed; 32], claim_seed)
}
fn fixture_with_provider(provider_id: [u8; 32], claim_seed: u8) -> Fixture {
    let owner_key = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519)
        .expect("provider owner fixture key");
    let owner = AccountId::new(owner_key.public_key().clone());
    let policy = signer_policy(1);
    let payload = MusubiProviderBundleVerificationPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding: MusubiProviderBundleVerificationBindingV1 {
            network_id: NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x21; 32])),
            ),
            provider_id: ProviderId::new(provider_id),
            completed_by: owner.clone(),
            completion_authority: ProviderIngestCompletionAuthorityV1::new(owner, policy),
            replication_order: ReplicationOrderId::new([0x23; 32]),
            assignment_revision: 3,
            completion_epoch: 9,
            finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                height: 77,
                block_hash: [0x24; 32],
            },
            archive_id: ArchiveId::new([0x25; 32]),
            bundle_digest: MusubiContentDigestV1::new([0x26; 32]),
            descriptor_digest: MusubiContentDigestV1::new([0x27; 32]),
            semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x28; 32]),
            verification_lock_digest: MusubiVerificationLockDigestV1::new([0x29; 32]),
            source_tree_digest: MusubiContentDigestV1::new([0x2A; 32]),
        },
    };
    payload.validate().expect("valid fixture payload");
    let request = ProviderIngestMusubiAttestationApprovalRequestV1::test_fixture(
        payload,
        [claim_seed; 32],
        ProviderIngestFinalizedCursorV1 {
            height: 80,
            block_hash: [0x2B; 32],
        },
        policy,
    )
    .expect("valid approval request fixture");
    Fixture { request, owner_key }
}
fn signed_attestation(fixture: &Fixture) -> MusubiProviderBundleVerificationAttestationV1 {
    let payload = fixture.request.payload().clone();
    let approval = MusubiProviderBundleVerificationApprovalV1 {
        public_key: fixture.owner_key.public_key().clone(),
        signature: SignatureOf::try_from_hash(
            fixture.owner_key.private_key(),
            payload.signing_hash(),
        )
        .expect("sign fixture attestation"),
    };
    let attestation = MusubiProviderBundleVerificationAttestationV1 {
        payload,
        approvals: vec![approval],
    };
    attestation
        .verify(&attestation.payload.binding)
        .expect("fixture attestation verifies");
    attestation
}
struct FakeApprovalSigner {
    handle: String,
    owner: AccountId,
    owner_key: KeyPair,
    policy: Mutex<ProviderIngestCompletionSignerPolicyV1>,
    rotate_after_approval: AtomicBool,
    rotate_adapter_after_approval: AtomicBool,
    approve_calls: AtomicUsize,
    controller_policy_digest: [u8; 32],
    adapter_policy_digest: Mutex<[u8; 32]>,
    delay_ms: AtomicU64,
}
impl FakeApprovalSigner {
    fn new(fixture: &Fixture) -> Self {
        let owner = fixture.request.payload().binding.completed_by.clone();
        Self {
            handle: "hsm://sorafs/musubi/provider-attestation/primary".to_owned(),
            controller_policy_digest: musubi_provider_attestation_controller_policy_digest_v1(
                &owner,
            )
            .expect("fixture controller digest"),
            owner,
            owner_key: fixture.owner_key.clone(),
            policy: Mutex::new(fixture.request.signer_policy()),
            rotate_after_approval: AtomicBool::new(false),
            rotate_adapter_after_approval: AtomicBool::new(false),
            approve_calls: AtomicUsize::new(0),
            adapter_policy_digest: Mutex::new([0xA7; 32]),
            delay_ms: AtomicU64::new(0),
        }
    }
    fn policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
        *self.policy.lock().expect("fake signer policy lock")
    }
}
impl MusubiProviderAttestationSignerV1 for FakeApprovalSigner {
    fn runtime_handle(&self) -> &str {
        &self.handle
    }
    fn authority(&self) -> &AccountId {
        &self.owner
    }
    fn qualification(
        &self,
    ) -> Result<
        MusubiProviderAttestationSignerQualificationV1,
        MusubiProviderAttestationSignerErrorV1,
    > {
        Ok(MusubiProviderAttestationSignerQualificationV1::new(
            1,
            *self
                .adapter_policy_digest
                .lock()
                .map_err(|_| MusubiProviderAttestationSignerErrorV1::Unavailable)?,
            self.policy(),
            self.owner.clone(),
            self.controller_policy_digest,
        ))
    }
    fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
        self.policy()
    }
    fn current_eligibility(
        &self,
    ) -> Result<ProviderIngestCompletionSignerPolicyV1, MusubiProviderAttestationSignerErrorV1>
    {
        Ok(self.policy())
    }
    fn approve<'a>(
        &'a self,
        request: &'a ProviderIngestMusubiAttestationApprovalRequestV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            MusubiProviderBundleVerificationAttestationV1,
            MusubiProviderAttestationSignerErrorV1,
        >,
    > {
        Box::pin(async move {
            self.approve_calls.fetch_add(1, Ordering::SeqCst);
            let delay_ms = self.delay_ms.load(Ordering::SeqCst);
            if delay_ms != 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
            let payload = request.payload().clone();
            let attestation = MusubiProviderBundleVerificationAttestationV1 {
                approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
                    public_key: self.owner_key.public_key().clone(),
                    signature: SignatureOf::try_from_hash(
                        self.owner_key.private_key(),
                        payload.signing_hash(),
                    )
                    .map_err(|_| MusubiProviderAttestationSignerErrorV1::Rejected)?,
                }],
                payload,
            };
            if self.rotate_after_approval.load(Ordering::SeqCst) {
                *self
                    .policy
                    .lock()
                    .map_err(|_| MusubiProviderAttestationSignerErrorV1::Unavailable)? =
                    signer_policy(2);
            }
            if self.rotate_adapter_after_approval.load(Ordering::SeqCst) {
                *self
                    .adapter_policy_digest
                    .lock()
                    .map_err(|_| MusubiProviderAttestationSignerErrorV1::Unavailable)? = [0xA8; 32];
            }
            Ok(attestation)
        })
    }
}
fn test_policy() -> MusubiProviderAttestationJournalPolicyV1 {
    MusubiProviderAttestationJournalPolicyV1 {
        max_entries: 16,
        max_attempts: 2,
        lease_ttl_ms: 10,
        approval_timeout_ms: 5,
        handoff_timeout_ms: 5,
        retry_delay_ms: 5,
        checkpoint_max_bytes: 8 * 1024 * 1024,
        max_cas_retries: 4,
    }
}
async fn prepare_handoff_claim(
    journal: &MusubiProviderAttestationJournalV1,
    fixture: &Fixture,
    owner_seed: u8,
) -> (
    MusubiProviderAttestationApprovalIdV1,
    MusubiProviderAttestationHandoffClaimV1,
) {
    let approval_id = journal
        .enqueue(&fixture.request)
        .await
        .expect("enqueue handoff fixture")
        .approval_id();
    let approval = journal
        .claim_approval(
            approval_id,
            MusubiProviderAttestationClaimOwnerV1::new([owner_seed; 32]).expect("approval owner"),
            100,
        )
        .await
        .expect("claim approval")
        .expect("approval work");
    let signer = FakeApprovalSigner::new(fixture);
    journal
        .approve_claim_with_signer(&approval, &fixture.request, &signer, &clock_at(101))
        .await
        .expect("approve handoff fixture");
    let handoff = journal
        .claim_handoff(
            approval_id,
            MusubiProviderAttestationClaimOwnerV1::new([owner_seed.wrapping_add(1); 32])
                .expect("handoff owner"),
            103,
        )
        .await
        .expect("claim handoff")
        .expect("handoff work");
    (approval_id, handoff)
}
#[test]
fn journal_policy_defaults_and_hard_limits_share_config_bounds() {
    let defaults = MusubiProviderAttestationJournalPolicyV1::default();
    assert_eq!(
        defaults.max_entries,
        provider_attestation_journal_defaults::MAX_ENTRIES
    );
    assert_eq!(
        defaults.checkpoint_max_bytes,
        usize::try_from(provider_attestation_journal_defaults::CHECKPOINT_MAX_BYTES.0)
            .expect("default checkpoint bound fits usize")
    );
    assert!(
        provider_attestation_journal_defaults::SINGLE_ACTIVE_ENTRY_RESERVE_BYTES_V1
            <= provider_attestation_journal_defaults::CHECKPOINT_MIN_BYTES
    );
    assert!(
        provider_attestation_journal_defaults::CHECKPOINT_MIN_BYTES
            <= defaults.checkpoint_max_bytes
    );
    defaults.validate().expect("shared defaults are valid");
    let mut exact_minimum = defaults;
    exact_minimum.checkpoint_max_bytes =
        provider_attestation_journal_defaults::CHECKPOINT_MIN_BYTES;
    exact_minimum
        .validate()
        .expect("exact shared checkpoint minimum is valid");
    let mutations: [fn(&mut MusubiProviderAttestationJournalPolicyV1); 7] = [
        |policy: &mut MusubiProviderAttestationJournalPolicyV1| {
            policy.max_attempts = provider_attestation_journal_defaults::MAX_ATTEMPTS_LIMIT + 1;
        },
        |policy: &mut MusubiProviderAttestationJournalPolicyV1| {
            policy.lease_ttl_ms = provider_attestation_journal_defaults::LEASE_TTL_MAX_MS + 1;
        },
        |policy: &mut MusubiProviderAttestationJournalPolicyV1| {
            policy.retry_delay_ms = provider_attestation_journal_defaults::RETRY_DELAY_MAX_MS + 1;
        },
        |policy: &mut MusubiProviderAttestationJournalPolicyV1| {
            policy.max_cas_retries =
                provider_attestation_journal_defaults::MAX_CAS_RETRIES_LIMIT + 1;
        },
        |policy: &mut MusubiProviderAttestationJournalPolicyV1| {
            policy.checkpoint_max_bytes = 0;
        },
        |policy: &mut MusubiProviderAttestationJournalPolicyV1| {
            policy.checkpoint_max_bytes =
                provider_attestation_journal_defaults::CHECKPOINT_MIN_BYTES - 1;
        },
        |policy: &mut MusubiProviderAttestationJournalPolicyV1| {
            policy.checkpoint_max_bytes =
                provider_attestation_journal_defaults::CHECKPOINT_MAX_BYTES_LIMIT + 1;
        },
    ];
    for mutate in mutations {
        let mut policy = defaults;
        mutate(&mut policy);
        assert_eq!(
            policy.validate(),
            Err(MusubiProviderAttestationJournalErrorV1::InvalidPolicy)
        );
    }
}
#[test]
fn journal_policy_digest_is_stable_and_commits_every_bound() {
    let policy = test_policy();
    let expected = [
        0x03, 0xbb, 0x0e, 0x39, 0xde, 0x37, 0x4f, 0x94, 0xfc, 0x8c, 0x3e, 0x94, 0xa6, 0x75, 0x84,
        0x76, 0x0a, 0x7d, 0xeb, 0x9f, 0x6e, 0xba, 0x82, 0x3c, 0x62, 0x8c, 0xd3, 0x9e, 0xfe, 0x44,
        0xde, 0xab,
    ];
    assert_eq!(policy.digest().expect("valid policy digest"), expected);
    assert_eq!(
        policy.digest().expect("repeat policy digest"),
        expected,
        "the same fixed-width policy must hash deterministically"
    );
    let mutations: [fn(&mut MusubiProviderAttestationJournalPolicyV1); 8] = [
        |value| value.max_entries += 1,
        |value| value.max_attempts += 1,
        |value| value.lease_ttl_ms += 1,
        |value| value.approval_timeout_ms += 1,
        |value| value.handoff_timeout_ms += 1,
        |value| value.retry_delay_ms += 1,
        |value| value.checkpoint_max_bytes += 1,
        |value| value.max_cas_retries += 1,
    ];
    for mutate in mutations {
        let mut changed = policy;
        mutate(&mut changed);
        assert_ne!(
            changed.digest().expect("changed policy remains valid"),
            expected,
            "each policy field must affect the deployment commitment"
        );
    }
}
fn assert_send<T: Send>(_: &T) {}
fn awaiting_checkpoint(fixture: &Fixture) -> StoredJournalCheckpointV1 {
    let intent = intent_from_request(&fixture.request, 1).expect("fixture intent");
    StoredJournalCheckpointV1 {
        version: JOURNAL_CHECKPOINT_VERSION_V1,
        checkpoint_sequence: 1,
        next_intent_sequence: 2,
        last_observed_unix_ms: 0,
        entries: vec![StoredJournalEntryV1 {
            intent,
            generation: 0,
            state: StoredJournalStateV1::AwaitingApproval {
                attempts: 0,
                next_attempt_after_ms: 0,
            },
        }],
    }
}
#[test]
fn checkpoint_writer_roundtrips_private_receipt_and_rejects_byte_budget() {
    let fixture = fixture(0x11, 0x12);
    let mut checkpoint = awaiting_checkpoint(&fixture);
    let attestation = signed_attestation(&fixture);
    let item =
        MusubiProviderAttestationInventoryItemV1::new(attestation.clone()).expect("inventory item");
    let receipt =
        MusubiProviderAttestationInventoryReceiptV1::new(&item, 7).expect("opaque receipt");
    checkpoint.last_observed_unix_ms = 100;
    checkpoint.checkpoint_sequence = 2;
    checkpoint.entries[0].generation = 1;
    checkpoint.entries[0].state = StoredJournalStateV1::Delivered {
        attestation: Box::new(attestation),
        receipt: Box::new(StoredInventoryReceiptV1::from_public(&receipt)),
    };
    let policy = test_policy();
    let bytes = encode_checkpoint(&checkpoint, policy).expect("reloadable checkpoint");
    let snapshot =
        MusubiProviderAttestationJournalStoreSnapshotV1::from_checkpoint_bytes(bytes.clone())
            .expect("checkpoint snapshot");
    assert_eq!(
        decode_checkpoint(&snapshot, policy).expect("bounded canonical decode"),
        checkpoint
    );
    let binding = &fixture.request.payload().binding;
    assert_eq!(
        validate_musubi_provider_attestation_journal_checkpoint_metadata_v1(
            &bytes,
            policy,
            &binding.network_id,
            binding.provider_id,
        )
        .expect("sealed-head metadata projection"),
        (
            checkpoint.checkpoint_sequence,
            checkpoint.last_observed_unix_ms
        )
    );
    let mut too_small = policy;
    too_small.checkpoint_max_bytes = bytes.len() - 1;
    assert_eq!(
        encode_checkpoint(&checkpoint, too_small),
        Err(MusubiProviderAttestationJournalErrorV1::CapacityExceeded)
    );
}
#[test]
fn corrupt_checkpoint_rejects_unmarked_network_identity() {
    let corrupt_identity_fixture = fixture(0x13, 0x14);
    let mut checkpoint = awaiting_checkpoint(&corrupt_identity_fixture);
    checkpoint.entries[0].intent.payload.binding.network_id = NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0; 32])),
    );
    let bytes = norito::to_bytes(&checkpoint).expect("encode deliberately corrupt checkpoint");
    let snapshot = MusubiProviderAttestationJournalStoreSnapshotV1::from_checkpoint_bytes(bytes)
        .expect("content-address corrupt bytes");
    assert_eq!(
        decode_checkpoint(&snapshot, test_policy()),
        Err(MusubiProviderAttestationJournalErrorV1::CorruptCheckpoint)
    );
    let impossible_deadline_fixture = fixture(0x15, 0x16);
    let mut impossible_deadline = awaiting_checkpoint(&impossible_deadline_fixture);
    impossible_deadline.checkpoint_sequence = 2;
    impossible_deadline.last_observed_unix_ms = 1;
    impossible_deadline.entries[0].generation = 1;
    impossible_deadline.entries[0].state = StoredJournalStateV1::ApprovalClaimed {
        attempts: 1,
        owner: [0x33; 32],
        lease_expires_at_ms: u64::MAX,
    };
    let bytes =
        norito::to_bytes(&impossible_deadline).expect("encode checkpoint with impossible deadline");
    let snapshot = MusubiProviderAttestationJournalStoreSnapshotV1::from_checkpoint_bytes(bytes)
        .expect("content-address impossible deadline");
    assert_eq!(
        decode_checkpoint(&snapshot, test_policy()),
        Err(MusubiProviderAttestationJournalErrorV1::CorruptCheckpoint)
    );
    let mut impossible_history = impossible_deadline;
    impossible_history.checkpoint_sequence = 1;
    impossible_history.entries[0].state = StoredJournalStateV1::ApprovalClaimed {
        attempts: 1,
        owner: [0x33; 32],
        lease_expires_at_ms: 10,
    };
    let bytes =
        norito::to_bytes(&impossible_history).expect("encode impossible history checkpoint");
    let snapshot = MusubiProviderAttestationJournalStoreSnapshotV1::from_checkpoint_bytes(bytes)
        .expect("content-address impossible history");
    assert_eq!(
        decode_checkpoint(&snapshot, test_policy()),
        Err(MusubiProviderAttestationJournalErrorV1::CorruptCheckpoint)
    );
}
#[tokio::test]
async fn abstract_store_exact_replay_is_idempotent_before_predecessor_check() {
    let store = MemoryJournalStore::default();
    let replacement = vec![0x11, 0x22, 0x33];
    let revision = musubi_provider_attestation_journal_checkpoint_revision_v1(&replacement);
    assert_eq!(
        store.compare_and_swap(None, replacement.clone()).await,
        Ok(MusubiProviderAttestationJournalCasOutcomeV1::Stored { revision })
    );
    assert_eq!(
        store
            .compare_and_swap(Some([0xA5; 32]), replacement.clone())
            .await,
        Ok(MusubiProviderAttestationJournalCasOutcomeV1::Stored { revision })
    );
    assert_eq!(
        store.compare_and_swap(Some([0xA5; 32]), vec![0x44]).await,
        Ok(MusubiProviderAttestationJournalCasOutcomeV1::Conflict)
    );
    assert_eq!(
        store.load().await.expect("load exact replay winner"),
        MusubiProviderAttestationJournalStoreSnapshotV1::from_checkpoint_bytes(replacement)
            .expect("valid memory checkpoint")
    );
}
#[tokio::test]
async fn exact_enqueue_replays_and_same_key_substitution_conflicts() {
    let store = Arc::new(MemoryJournalStore::default());
    let journal =
        MusubiProviderAttestationJournalV1::new(store, test_policy()).expect("construct journal");
    let initial_fixture = fixture(0x41, 0x42);
    let inserted = journal
        .enqueue(&initial_fixture.request)
        .await
        .expect("insert intent");
    assert!(matches!(
        inserted,
        MusubiProviderAttestationEnqueueOutcomeV1::Inserted { .. }
    ));
    assert!(matches!(
        journal
            .enqueue(&initial_fixture.request)
            .await
            .expect("replay intent"),
        MusubiProviderAttestationEnqueueOutcomeV1::Existing { .. }
    ));
    let later_request = ProviderIngestMusubiAttestationApprovalRequestV1::test_fixture(
        initial_fixture.request.payload().clone(),
        initial_fixture.request.completion_claim_digest(),
        ProviderIngestFinalizedCursorV1 {
            height: 81,
            block_hash: [0x91; 32],
        },
        initial_fixture.request.signer_policy(),
    )
    .expect("later finalized request");
    assert!(matches!(
        journal
            .enqueue(&later_request)
            .await
            .expect("later cursor resumes exact intent"),
        MusubiProviderAttestationEnqueueOutcomeV1::Existing { .. }
    ));
    let lower_request = ProviderIngestMusubiAttestationApprovalRequestV1::test_fixture(
        initial_fixture.request.payload().clone(),
        initial_fixture.request.completion_claim_digest(),
        ProviderIngestFinalizedCursorV1 {
            height: 79,
            block_hash: [0x92; 32],
        },
        initial_fixture.request.signer_policy(),
    )
    .expect("lower but structurally valid cursor");
    assert_eq!(
        journal.enqueue(&lower_request).await,
        Err(MusubiProviderAttestationJournalErrorV1::IntentConflict)
    );
    let forked_request = ProviderIngestMusubiAttestationApprovalRequestV1::test_fixture(
        initial_fixture.request.payload().clone(),
        initial_fixture.request.completion_claim_digest(),
        ProviderIngestFinalizedCursorV1 {
            height: 80,
            block_hash: [0x93; 32],
        },
        initial_fixture.request.signer_policy(),
    )
    .expect("same-height fork is structurally valid above the completion anchor");
    assert_eq!(
        journal.enqueue(&forked_request).await,
        Err(MusubiProviderAttestationJournalErrorV1::IntentConflict)
    );
    let conflicting = fixture(0x41, 0x43);
    assert_eq!(
        journal.enqueue(&conflicting.request).await,
        Err(MusubiProviderAttestationJournalErrorV1::IntentConflict)
    );
}
#[tokio::test]
async fn pre_enqueue_probe_checks_retained_key_before_inventory() {
    let journal = MusubiProviderAttestationJournalV1::new(
        Arc::new(MemoryJournalStore::default()),
        test_policy(),
    )
    .expect("construct journal");
    let retained = fixture(0x42, 0x52);
    journal
        .enqueue(&retained.request)
        .await
        .expect("retain exact request");
    let inventory = MemoryInventory::default();
    *inventory.readiness_error.lock().expect("readiness lock") =
        Some(MusubiProviderAttestationInventoryRuntimeErrorV1::Unavailable);
    assert_eq!(
        journal
            .probe_pre_enqueue_with_inventory(&retained.request, &inventory)
            .await,
        Ok(MusubiProviderAttestationPreEnqueueProbeV1::RetainedExact)
    );
    let conflicting = fixture(0x42, 0x53);
    assert_eq!(
        journal
            .probe_pre_enqueue_with_inventory(&conflicting.request, &inventory)
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::IntentConflict)
    );
    assert_eq!(inventory.runtime_handle_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.qualification_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.inventory_calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn pre_enqueue_probe_suppresses_only_an_exact_inventory_payload() {
    let journal = MusubiProviderAttestationJournalV1::new(
        Arc::new(MemoryJournalStore::default()),
        test_policy(),
    )
    .expect("construct journal");
    let fixture = fixture(0x43, 0x54);
    let inventory = MemoryInventory::default();
    assert_eq!(
        journal
            .probe_pre_enqueue_with_inventory(&fixture.request, &inventory)
            .await,
        Ok(MusubiProviderAttestationPreEnqueueProbeV1::Absent)
    );
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 2);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 1);
    let exact_item = MusubiProviderAttestationInventoryItemV1::new(signed_attestation(&fixture))
        .expect("exact inventory item");
    inventory
        .entries
        .lock()
        .expect("inventory entries lock")
        .push((exact_item, 7));
    assert_eq!(
        journal
            .probe_pre_enqueue_with_inventory(&fixture.request, &inventory)
            .await,
        Ok(MusubiProviderAttestationPreEnqueueProbeV1::InventoryExact)
    );
    let mut substituted_payload = fixture.request.payload().clone();
    substituted_payload.binding.bundle_digest = MusubiContentDigestV1::new([0xD4; 32]);
    let substituted_attestation = MusubiProviderBundleVerificationAttestationV1 {
        approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
            public_key: fixture.owner_key.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                fixture.owner_key.private_key(),
                substituted_payload.signing_hash(),
            )
            .expect("sign substituted inventory payload"),
        }],
        payload: substituted_payload,
    };
    let substituted_item = MusubiProviderAttestationInventoryItemV1::new(substituted_attestation)
        .expect("valid same-key substituted inventory item");
    let mut entries = inventory.entries.lock().expect("inventory entries lock");
    entries.clear();
    entries.push((substituted_item, 8));
    drop(entries);
    assert_eq!(
        journal
            .probe_pre_enqueue_with_inventory(&fixture.request, &inventory)
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryRejected)
    );
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 6);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 3);
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.inventory_calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn pre_enqueue_probe_bounds_inventory_readback_and_preserves_absence() {
    let mut policy = test_policy();
    policy.handoff_timeout_ms = 2;
    let journal =
        MusubiProviderAttestationJournalV1::new(Arc::new(MemoryJournalStore::default()), policy)
            .expect("construct journal");
    let fixture = fixture(0x44, 0x55);
    let inventory = MemoryInventory::default();
    inventory.get_delay_ms.store(10, Ordering::SeqCst);
    assert_eq!(
        journal
            .probe_pre_enqueue_with_inventory(&fixture.request, &inventory)
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryUnavailable)
    );
    inventory.get_delay_ms.store(0, Ordering::SeqCst);
    assert_eq!(
        journal
            .probe_pre_enqueue_with_inventory(&fixture.request, &inventory)
            .await,
        Ok(MusubiProviderAttestationPreEnqueueProbeV1::Absent)
    );
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.inventory_calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn pre_enqueue_probe_rejects_inventory_qualification_drift_after_get() {
    let journal = MusubiProviderAttestationJournalV1::new(
        Arc::new(MemoryJournalStore::default()),
        test_policy(),
    )
    .expect("construct journal");
    let fixture = fixture(0x45, 0x56);
    let item = MusubiProviderAttestationInventoryItemV1::new(signed_attestation(&fixture))
        .expect("exact inventory item");
    let inventory = MemoryInventory::default();
    inventory
        .entries
        .lock()
        .expect("inventory entries lock")
        .push((item, 1));
    inventory
        .drift_qualification_after_get
        .store(true, Ordering::SeqCst);
    assert_eq!(
        journal
            .probe_pre_enqueue_with_inventory(&fixture.request, &inventory)
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryRejected)
    );
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 1);
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 2);
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn expired_reclaim_fences_the_stale_approval_claim() {
    let store = Arc::new(MemoryJournalStore::default());
    let journal =
        MusubiProviderAttestationJournalV1::new(store, test_policy()).expect("construct journal");
    let fixture = fixture(0x44, 0x45);
    let id = journal
        .enqueue(&fixture.request)
        .await
        .expect("enqueue")
        .approval_id();
    let first = journal
        .claim_approval(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x51; 32]).expect("owner"),
            100,
        )
        .await
        .expect("first claim")
        .expect("ready work");
    let second = journal
        .claim_approval(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x52; 32]).expect("owner"),
            first.lease_expires_at_ms(),
        )
        .await
        .expect("reclaim")
        .expect("expired work is reclaimable");
    assert!(second.generation() > first.generation());
    assert_eq!(
        journal
            .record_approval_failure(
                &first,
                first.lease_expires_at_ms(),
                MusubiProviderAttestationFailureClassV1::Retryable,
            )
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::StaleClaim)
    );
}
#[tokio::test]
async fn durable_unix_floor_rejects_backward_clock_after_restart() {
    let store = Arc::new(MemoryJournalStore::default());
    let fixture = fixture(0x45, 0x46);
    let journal = MusubiProviderAttestationJournalV1::new(store.clone(), test_policy())
        .expect("construct journal");
    let id = journal
        .enqueue(&fixture.request)
        .await
        .expect("enqueue")
        .approval_id();
    journal
        .claim_approval(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x55; 32]).expect("owner"),
            100,
        )
        .await
        .expect("claim");
    drop(journal);
    let restarted =
        MusubiProviderAttestationJournalV1::new(store, test_policy()).expect("restart journal");
    assert_eq!(
        restarted.ready_approval_page(99, None, 1).await,
        Err(MusubiProviderAttestationJournalErrorV1::ClockRollback)
    );
    assert_eq!(
        restarted
            .ready_approval_page(110, None, 1)
            .await
            .expect("expired lease is discoverable")[0]
            .approval_id(),
        id
    );
}
#[tokio::test]
async fn approved_state_survives_restart_and_hands_off_idempotently() {
    let store = Arc::new(MemoryJournalStore::default());
    let fixture = fixture(0x46, 0x47);
    let journal = MusubiProviderAttestationJournalV1::new(store.clone(), test_policy())
        .expect("construct journal");
    let id = journal
        .enqueue(&fixture.request)
        .await
        .expect("enqueue")
        .approval_id();
    drop(journal);
    let journal = MusubiProviderAttestationJournalV1::new(store.clone(), test_policy())
        .expect("restart before approval");
    assert_eq!(
        journal
            .ready_approval_page(100, None, MUSUBI_PROVIDER_ATTESTATION_READY_PAGE_MAX_V1,)
            .await
            .expect("discover approval after restart")
            .into_iter()
            .map(MusubiProviderAttestationJournalScanKeyV1::approval_id)
            .collect::<Vec<_>>(),
        vec![id],
    );
    let approval_claim = journal
        .claim_approval(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x53; 32]).expect("owner"),
            100,
        )
        .await
        .expect("claim")
        .expect("approval ready");
    let signer = FakeApprovalSigner::new(&fixture);
    let approval_clock = clock_at(101);
    let approval_future = journal.approve_claim_with_signer(
        &approval_claim,
        &fixture.request,
        &signer,
        &approval_clock,
    );
    assert_send(&approval_future);
    approval_future
        .await
        .expect("approve through qualified signer");
    drop(journal);
    let restarted =
        MusubiProviderAttestationJournalV1::new(store, test_policy()).expect("restart journal");
    assert_eq!(
        restarted
            .ready_handoff_page(200, None, MUSUBI_PROVIDER_ATTESTATION_READY_PAGE_MAX_V1,)
            .await
            .expect("discover handoff after restart")
            .into_iter()
            .map(MusubiProviderAttestationJournalScanKeyV1::approval_id)
            .collect::<Vec<_>>(),
        vec![id],
    );
    let handoff = restarted
        .claim_handoff(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x54; 32]).expect("owner"),
            200,
        )
        .await
        .expect("claim handoff")
        .expect("approved handoff survived restart");
    let inventory = MemoryInventory::default();
    let handoff_clock = clock_at(201);
    let handoff_future =
        restarted.handoff_claim_with_inventory(&handoff, &inventory, &handoff_clock);
    assert_send(&handoff_future);
    handoff_future.await.expect("persist delivery");
    let runtime_calls_after_delivery = (
        inventory.runtime_handle_calls.load(Ordering::SeqCst),
        inventory.qualification_calls.load(Ordering::SeqCst),
        inventory.readiness_calls.load(Ordering::SeqCst),
    );
    assert_eq!(
        restarted
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(202))
            .await,
        Ok(MusubiProviderAttestationDeliveryOutcomeV1::Existing)
    );
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 1);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        (
            inventory.runtime_handle_calls.load(Ordering::SeqCst),
            inventory.qualification_calls.load(Ordering::SeqCst),
            inventory.readiness_calls.load(Ordering::SeqCst),
        ),
        runtime_calls_after_delivery,
        "an already-delivered preflight must not call the inventory runtime"
    );
    assert_eq!(
        restarted
            .status(id)
            .await
            .expect("read status")
            .expect("retained status")
            .stage,
        MusubiProviderAttestationJournalStageV1::Delivered
    );
}
#[tokio::test]
async fn external_effect_completion_time_is_sealed_before_journal_commit_and_restart() {
    let store = Arc::new(MemoryJournalStore::default());
    let fixture = fixture(0x5A, 0x6A);
    let journal = MusubiProviderAttestationJournalV1::new(store.clone(), test_policy())
        .expect("construct journal");
    let id = journal
        .enqueue(&fixture.request)
        .await
        .expect("enqueue")
        .approval_id();
    let approval = journal
        .claim_approval(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0xA1; 32]).expect("approval owner"),
            100,
        )
        .await
        .expect("claim approval")
        .expect("approval ready");
    let clock = SequenceJournalTime::new([101, 104, 105, 107]);
    let signer = FakeApprovalSigner::new(&fixture);
    journal
        .approve_claim_with_signer(&approval, &fixture.request, &signer, &clock)
        .await
        .expect("persist signer result at second sealed sample");
    assert_eq!(clock.sealed_floor(), 104);
    let after_approval = store.load().await.expect("load approval checkpoint");
    assert_eq!(
        decode_checkpoint(&after_approval, test_policy())
            .expect("decode approval checkpoint")
            .last_observed_unix_ms,
        clock.sealed_floor(),
        "journal floor must equal, never exceed, the external seal"
    );
    drop(journal);
    let restarted = MusubiProviderAttestationJournalV1::new(store.clone(), test_policy())
        .expect("restart after approval");
    assert_eq!(
        restarted
            .ready_handoff_page(104, None, 1)
            .await
            .expect("restart accepts the sealed approval floor")
            .first()
            .map(|key| key.approval_id()),
        Some(id)
    );
    let handoff = restarted
        .claim_handoff(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0xA2; 32]).expect("handoff owner"),
            104,
        )
        .await
        .expect("claim handoff")
        .expect("handoff ready");
    restarted
        .handoff_claim_with_inventory(&handoff, &MemoryInventory::default(), &clock)
        .await
        .expect("persist inventory result at second sealed sample");
    assert_eq!(clock.sealed_floor(), 107);
    let after_handoff = store.load().await.expect("load handoff checkpoint");
    assert_eq!(
        decode_checkpoint(&after_handoff, test_policy())
            .expect("decode handoff checkpoint")
            .last_observed_unix_ms,
        clock.sealed_floor(),
        "delivered journal floor must remain covered by the external seal"
    );
    drop(restarted);
    let restarted = MusubiProviderAttestationJournalV1::new(store, test_policy())
        .expect("restart after handoff");
    assert_eq!(
        restarted
            .status(id)
            .await
            .expect("read restarted status")
            .expect("retained entry")
            .stage,
        MusubiProviderAttestationJournalStageV1::Delivered
    );
}
#[tokio::test]
async fn handoff_dead_letter_retains_evidence_and_requeues_after_restart() {
    let store = Arc::new(MemoryJournalStore::default());
    let fixture = fixture(0x47, 0x57);
    let journal = MusubiProviderAttestationJournalV1::new(store.clone(), test_policy())
        .expect("construct journal");
    let id = journal
        .enqueue(&fixture.request)
        .await
        .expect("enqueue")
        .approval_id();
    let approval = journal
        .claim_approval(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x61; 32]).expect("owner"),
            100,
        )
        .await
        .expect("claim approval")
        .expect("approval work");
    let signer = FakeApprovalSigner::new(&fixture);
    journal
        .approve_claim_with_signer(&approval, &fixture.request, &signer, &clock_at(101))
        .await
        .expect("approve");
    let handoff = journal
        .claim_handoff(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x62; 32]).expect("owner"),
            103,
        )
        .await
        .expect("claim handoff")
        .expect("handoff work");
    journal
        .record_handoff_failure(
            &handoff,
            104,
            MusubiProviderAttestationFailureClassV1::Permanent,
        )
        .await
        .expect("dead-letter handoff");
    drop(journal);
    let restarted =
        MusubiProviderAttestationJournalV1::new(store, test_policy()).expect("restart journal");
    let status = restarted
        .status(id)
        .await
        .expect("status")
        .expect("dead letter retained");
    assert_eq!(
        status.stage,
        MusubiProviderAttestationJournalStageV1::DeadLetter
    );
    assert!(status.dead_letter_has_approved_attestation);
    assert_eq!(status.dead_letter_attempts, Some(1));
    assert_eq!(status.dead_lettered_at_unix_ms, Some(104));
    let dead_page = restarted
        .dead_letter_page(None, 1)
        .await
        .expect("rediscover dead letter");
    assert_eq!(dead_page[0].approval_id(), id);
    assert_eq!(
        restarted
            .requeue_dead_letter(id, status.generation, 105)
            .await
            .expect("requeue retained evidence"),
        MusubiProviderAttestationJournalStageV1::ApprovedPendingHandoff
    );
    assert_eq!(
        restarted
            .ready_handoff_page(105, None, 1)
            .await
            .expect("handoff ready again")[0]
            .approval_id(),
        id
    );
    let repaired_claim = restarted
        .claim_handoff(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x63; 32]).expect("owner"),
            106,
        )
        .await
        .expect("claim repaired handoff")
        .expect("repaired work");
    restarted
        .record_handoff_failure(
            &repaired_claim,
            107,
            MusubiProviderAttestationFailureClassV1::Permanent,
        )
        .await
        .expect("return to dead letter");
    let terminal_generation = restarted
        .status(id)
        .await
        .expect("status")
        .expect("dead letter")
        .generation;
    restarted
        .acknowledge_dead_letter(id, terminal_generation)
        .await
        .expect("explicitly acknowledge inspected dead letter");
    assert!(restarted.status(id).await.expect("status").is_none());
}
#[tokio::test]
async fn stale_or_mismatched_claims_cause_no_external_calls() {
    let primary_fixture = fixture(0x48, 0x58);
    let journal = MusubiProviderAttestationJournalV1::new(
        Arc::new(MemoryJournalStore::default()),
        test_policy(),
    )
    .expect("construct journal");
    let id = journal
        .enqueue(&primary_fixture.request)
        .await
        .expect("enqueue")
        .approval_id();
    let stale = journal
        .claim_approval(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x71; 32]).expect("owner"),
            100,
        )
        .await
        .expect("claim")
        .expect("approval work");
    journal
        .record_approval_failure(
            &stale,
            101,
            MusubiProviderAttestationFailureClassV1::Retryable,
        )
        .await
        .expect("return work to retry");
    let signer = FakeApprovalSigner::new(&primary_fixture);
    assert_eq!(
        journal
            .approve_claim_with_signer(&stale, &primary_fixture.request, &signer, &clock_at(102),)
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::StaleClaim)
    );
    assert_eq!(signer.approve_calls.load(Ordering::SeqCst), 0);
    let current = journal
        .claim_approval(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x72; 32]).expect("owner"),
            106,
        )
        .await
        .expect("reclaim")
        .expect("approval ready");
    let unrelated = fixture(0x49, 0x59);
    assert_eq!(
        journal
            .approve_claim_with_signer(&current, &unrelated.request, &signer, &clock_at(107))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InvalidAttestation)
    );
    assert_eq!(signer.approve_calls.load(Ordering::SeqCst), 0);
    journal
        .approve_claim_with_signer(&current, &primary_fixture.request, &signer, &clock_at(107))
        .await
        .expect("approve current claim");
    let handoff = journal
        .claim_handoff(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x73; 32]).expect("owner"),
            109,
        )
        .await
        .expect("claim handoff")
        .expect("handoff work");
    journal
        .record_handoff_failure(
            &handoff,
            110,
            MusubiProviderAttestationFailureClassV1::Retryable,
        )
        .await
        .expect("return handoff to retry");
    let inventory = MemoryInventory::default();
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(111))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::StaleClaim)
    );
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.runtime_handle_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.qualification_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn shared_deadline_prevents_late_external_results_from_becoming_durable() {
    let mut policy = test_policy();
    policy.lease_ttl_ms = 20;
    let fixture = fixture(0x4A, 0x5A);
    let journal =
        MusubiProviderAttestationJournalV1::new(Arc::new(MemoryJournalStore::default()), policy)
            .expect("construct journal");
    let id = journal
        .enqueue(&fixture.request)
        .await
        .expect("enqueue")
        .approval_id();
    let approval = journal
        .claim_approval(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x74; 32]).expect("owner"),
            100,
        )
        .await
        .expect("claim")
        .expect("approval work");
    let delayed_signer = FakeApprovalSigner::new(&fixture);
    delayed_signer.delay_ms.store(10, Ordering::SeqCst);
    assert_eq!(
        journal
            .approve_claim_with_signer(
                &approval,
                &fixture.request,
                &delayed_signer,
                &clock_at(101),
            )
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::SignerUnavailable)
    );
    assert_eq!(
        journal
            .status(id)
            .await
            .expect("status")
            .expect("claim retained")
            .stage,
        MusubiProviderAttestationJournalStageV1::ApprovalClaimed
    );
    let signer = FakeApprovalSigner::new(&fixture);
    journal
        .approve_claim_with_signer(&approval, &fixture.request, &signer, &clock_at(106))
        .await
        .expect("retry exact approval");
    let handoff = journal
        .claim_handoff(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x75; 32]).expect("owner"),
            108,
        )
        .await
        .expect("claim handoff")
        .expect("handoff work");
    let delayed_inventory = MemoryInventory::default();
    delayed_inventory.delay_ms.store(10, Ordering::SeqCst);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &delayed_inventory, &clock_at(109))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryUnavailable)
    );
    assert_eq!(
        journal
            .status(id)
            .await
            .expect("status")
            .expect("handoff claim retained")
            .stage,
        MusubiProviderAttestationJournalStageV1::HandoffClaimed
    );
}
#[tokio::test]
async fn post_effect_clock_timeout_is_classified_as_clock_unavailable() {
    let mut policy = test_policy();
    policy.lease_ttl_ms = 20;
    let fixture = fixture(0x4B, 0x5B);
    let journal =
        MusubiProviderAttestationJournalV1::new(Arc::new(MemoryJournalStore::default()), policy)
            .expect("construct journal");
    let id = journal
        .enqueue(&fixture.request)
        .await
        .expect("enqueue")
        .approval_id();
    let approval = journal
        .claim_approval(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x76; 32]).expect("owner"),
            100,
        )
        .await
        .expect("claim")
        .expect("approval work");
    let signer = FakeApprovalSigner::new(&fixture);
    let delayed_approval_clock = DelayedSecondJournalTime::new(101, 102, 20);
    assert_eq!(
        journal
            .approve_claim_with_signer(
                &approval,
                &fixture.request,
                &signer,
                &delayed_approval_clock,
            )
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::ClockUnavailable)
    );
    assert_eq!(
        journal
            .status(id)
            .await
            .expect("status")
            .expect("approval claim retained")
            .stage,
        MusubiProviderAttestationJournalStageV1::ApprovalClaimed
    );
    journal
        .approve_claim_with_signer(&approval, &fixture.request, &signer, &clock_at(106))
        .await
        .expect("retry exact approval");
    let handoff = journal
        .claim_handoff(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x77; 32]).expect("owner"),
            108,
        )
        .await
        .expect("claim handoff")
        .expect("handoff work");
    let inventory = MemoryInventory::default();
    let delayed_handoff_clock = DelayedSecondJournalTime::new(109, 110, 20);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &delayed_handoff_clock)
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::ClockUnavailable)
    );
    assert_eq!(
        journal
            .status(id)
            .await
            .expect("status")
            .expect("handoff claim retained")
            .stage,
        MusubiProviderAttestationJournalStageV1::HandoffClaimed
    );
}
#[tokio::test]
async fn capacity_prunes_only_oldest_delivered_entry() {
    let store = Arc::new(MemoryJournalStore::default());
    let mut policy = test_policy();
    policy.max_entries = 3;
    let journal =
        MusubiProviderAttestationJournalV1::new(store, policy).expect("construct bounded journal");
    let delivered_fixture = fixture(0x31, 0x81);
    let delivered_id = journal
        .enqueue(&delivered_fixture.request)
        .await
        .expect("enqueue delivered candidate")
        .approval_id();
    let approval_claim = journal
        .claim_approval(
            delivered_id,
            MusubiProviderAttestationClaimOwnerV1::new([0x81; 32]).expect("owner"),
            100,
        )
        .await
        .expect("claim approval")
        .expect("approval work");
    journal
        .store_approved(
            &approval_claim,
            &delivered_fixture.request,
            signed_attestation(&delivered_fixture),
            101,
        )
        .await
        .expect("store attestation");
    let handoff_claim = journal
        .claim_handoff(
            delivered_id,
            MusubiProviderAttestationClaimOwnerV1::new([0x82; 32]).expect("owner"),
            102,
        )
        .await
        .expect("claim handoff")
        .expect("handoff work");
    let receipt = MusubiProviderAttestationInventoryReceiptV1::new(handoff_claim.item(), 1)
        .expect("exact receipt");
    journal
        .mark_delivered(&handoff_claim, receipt, 103)
        .await
        .expect("deliver first entry");
    let dead_fixture = fixture(0x32, 0x82);
    let dead_id = journal
        .enqueue(&dead_fixture.request)
        .await
        .expect("enqueue dead-letter candidate")
        .approval_id();
    let dead_claim = journal
        .claim_approval(
            dead_id,
            MusubiProviderAttestationClaimOwnerV1::new([0x83; 32]).expect("owner"),
            200,
        )
        .await
        .expect("claim dead-letter candidate")
        .expect("approval work");
    journal
        .record_approval_failure(
            &dead_claim,
            201,
            MusubiProviderAttestationFailureClassV1::Permanent,
        )
        .await
        .expect("dead-letter exact entry");
    let active_fixture = fixture(0x33, 0x83);
    let active_id = journal
        .enqueue(&active_fixture.request)
        .await
        .expect("enqueue active entry")
        .approval_id();
    let replacement_fixture = fixture(0x34, 0x84);
    let replacement_id = journal
        .enqueue(&replacement_fixture.request)
        .await
        .expect("delivered tombstone makes room")
        .approval_id();
    assert!(
        journal
            .status(delivered_id)
            .await
            .expect("status")
            .is_none()
    );
    assert_eq!(
        journal
            .status(dead_id)
            .await
            .expect("dead status")
            .expect("dead letter retained")
            .stage,
        MusubiProviderAttestationJournalStageV1::DeadLetter
    );
    assert_eq!(
        journal
            .status(active_id)
            .await
            .expect("active status")
            .expect("active entry retained")
            .stage,
        MusubiProviderAttestationJournalStageV1::AwaitingApproval
    );
    assert!(
        journal
            .status(replacement_id)
            .await
            .expect("replacement status")
            .is_some()
    );
    let inventory = MemoryInventory::default();
    let delivered_item =
        MusubiProviderAttestationInventoryItemV1::new(signed_attestation(&delivered_fixture))
            .expect("pruned delivery inventory item");
    inventory
        .entries
        .lock()
        .expect("inventory entries lock")
        .push((delivered_item, 1));
    assert_eq!(
        journal
            .probe_pre_enqueue_with_inventory(&delivered_fixture.request, &inventory)
            .await,
        Ok(MusubiProviderAttestationPreEnqueueProbeV1::InventoryExact)
    );
}
#[tokio::test]
async fn minimum_capacity_carries_one_entry_through_delivery() {
    let fixture = fixture(0x35, 0x85);
    let checkpoint = awaiting_checkpoint(&fixture);
    let encoded_len = norito::encode_canonical(&checkpoint)
        .expect("encode awaiting checkpoint")
        .len();
    let required_capacity = encoded_len
        .checked_add(checkpoint_future_reserve_bytes(&checkpoint).expect("bounded future reserve"))
        .expect("fixture capacity");
    let mut accounting_policy = test_policy();
    accounting_policy.checkpoint_max_bytes = required_capacity;
    encode_checkpoint(&checkpoint, accounting_policy).expect("exact future reserve fits");
    accounting_policy.checkpoint_max_bytes = required_capacity - 1;
    assert_eq!(
        encode_checkpoint(&checkpoint, accounting_policy),
        Err(MusubiProviderAttestationJournalErrorV1::CapacityExceeded)
    );
    assert!(
        required_capacity <= provider_attestation_journal_defaults::CHECKPOINT_MIN_BYTES,
        "shared minimum must cover one fixture's complete future reserve"
    );
    let mut policy = test_policy();
    policy.checkpoint_max_bytes = provider_attestation_journal_defaults::CHECKPOINT_MIN_BYTES;
    let store = Arc::new(MemoryJournalStore::default());
    let journal = MusubiProviderAttestationJournalV1::new(store, policy).expect("near-cap journal");
    let id = journal
        .enqueue(&fixture.request)
        .await
        .expect("reservation admits complete lifecycle")
        .approval_id();
    let approval = journal
        .claim_approval(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x91; 32]).expect("owner"),
            100,
        )
        .await
        .expect("claim within fixed footprint")
        .expect("approval work");
    let signer = FakeApprovalSigner::new(&fixture);
    journal
        .approve_claim_with_signer(&approval, &fixture.request, &signer, &clock_at(101))
        .await
        .expect("attestation consumes reserved capacity");
    let handoff = journal
        .claim_handoff(
            id,
            MusubiProviderAttestationClaimOwnerV1::new([0x92; 32]).expect("owner"),
            103,
        )
        .await
        .expect("claim handoff")
        .expect("handoff work");
    let inventory = MemoryInventory::default();
    journal
        .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(104))
        .await
        .expect("receipt remains durable at the capacity edge");
    assert_eq!(
        journal
            .status(id)
            .await
            .expect("status")
            .expect("newly delivered target must not be pruned")
            .stage,
        MusubiProviderAttestationJournalStageV1::Delivered
    );
    let mut below_minimum_policy = policy;
    below_minimum_policy.checkpoint_max_bytes =
        provider_attestation_journal_defaults::CHECKPOINT_MIN_BYTES - 1;
    assert_eq!(
        below_minimum_policy.validate(),
        Err(MusubiProviderAttestationJournalErrorV1::InvalidPolicy)
    );
}
#[tokio::test]
async fn dead_letter_scan_pages_every_retained_identity_after_restart() {
    const ENTRY_COUNT: usize = MUSUBI_PROVIDER_ATTESTATION_READY_PAGE_MAX_V1 + 1;
    let mut policy = test_policy();
    policy.max_entries = ENTRY_COUNT;
    policy.checkpoint_max_bytes = MUSUBI_PROVIDER_ATTESTATION_JOURNAL_CHECKPOINT_MAX_BYTES_V1;
    let mut entries = Vec::with_capacity(ENTRY_COUNT);
    for index in 0..ENTRY_COUNT {
        let mut provider_id = [0xA5; 32];
        let encoded_index = u16::try_from(index)
            .expect("bounded fixture index")
            .to_be_bytes();
        provider_id[..2].copy_from_slice(&encoded_index);
        let fixture = fixture_with_provider(provider_id, 0xA6);
        let sequence = u64::try_from(index)
            .expect("bounded sequence")
            .checked_add(1)
            .expect("non-zero sequence");
        entries.push(StoredJournalEntryV1 {
            intent: intent_from_request(&fixture.request, sequence).expect("valid intent"),
            generation: 1,
            state: StoredJournalStateV1::DeadLetter {
                reason: MusubiProviderAttestationDeadLetterReasonV1::ApprovalRejected,
                attestation: None,
                attempts: 1,
                dead_lettered_at_unix_ms: 1,
            },
        });
    }
    entries.sort_by_key(|entry| entry.intent.approval_id);
    let checkpoint = StoredJournalCheckpointV1 {
        version: JOURNAL_CHECKPOINT_VERSION_V1,
        checkpoint_sequence: u64::try_from(ENTRY_COUNT)
            .expect("bounded checkpoint sequence")
            .checked_mul(2)
            .expect("enqueue plus generation writes"),
        next_intent_sequence: u64::try_from(ENTRY_COUNT)
            .expect("bounded next sequence")
            .checked_add(1)
            .expect("next sequence"),
        last_observed_unix_ms: 1,
        entries,
    };
    let bytes = encode_checkpoint(&checkpoint, policy).expect("bounded DLQ checkpoint");
    let store = Arc::new(MemoryJournalStore::default());
    *store.latest.lock().expect("journal store lock") =
        MusubiProviderAttestationJournalStoreSnapshotV1::from_checkpoint_bytes(bytes)
            .expect("persist checkpoint");
    let restarted =
        MusubiProviderAttestationJournalV1::new(store, policy).expect("restart journal");
    let mut after = None;
    let mut scanned = Vec::new();
    loop {
        let page = restarted
            .dead_letter_page(after, 128)
            .await
            .expect("scan dead-letter page");
        if page.is_empty() {
            break;
        }
        after = page.last().copied();
        scanned.extend(page);
    }
    assert_eq!(scanned.len(), ENTRY_COUNT);
    assert!(scanned.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        scanned
            .iter()
            .map(|cursor| cursor.approval_id())
            .collect::<BTreeSet<_>>()
            .len(),
        ENTRY_COUNT
    );
}
#[tokio::test]
async fn handoff_requires_exact_inventory_item_and_revision_readback() {
    let primary_fixture = fixture(0x65, 0x75);
    let journal = MusubiProviderAttestationJournalV1::new(
        Arc::new(MemoryJournalStore::default()),
        test_policy(),
    )
    .expect("construct journal");
    let (approval_id, handoff) = prepare_handoff_claim(&journal, &primary_fixture, 0xA3).await;
    let inventory = MemoryInventory::default();
    inventory.omit_readback.store(true, Ordering::SeqCst);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(104))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InvalidInventoryReceipt)
    );
    inventory.omit_readback.store(false, Ordering::SeqCst);
    inventory
        .readback_revision_override
        .store(2, Ordering::SeqCst);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(104))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InvalidInventoryReceipt)
    );
    inventory
        .readback_revision_override
        .store(0, Ordering::SeqCst);
    let substituted =
        MusubiProviderAttestationInventoryItemV1::new(signed_attestation(&fixture(0x66, 0x76)))
            .expect("valid substituted item");
    *inventory
        .readback_item_override
        .lock()
        .expect("readback override lock") = Some(substituted);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(104))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InvalidInventoryReceipt)
    );
    *inventory
        .readback_item_override
        .lock()
        .expect("readback override lock") = None;
    assert_eq!(
        journal
            .status(approval_id)
            .await
            .expect("status")
            .expect("handoff retained")
            .stage,
        MusubiProviderAttestationJournalStageV1::HandoffClaimed
    );
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 3);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(105))
            .await,
        Ok(MusubiProviderAttestationDeliveryOutcomeV1::Delivered)
    );
    assert_eq!(
        journal
            .status(approval_id)
            .await
            .expect("status")
            .expect("delivery retained")
            .stage,
        MusubiProviderAttestationJournalStageV1::Delivered
    );
}
#[tokio::test]
async fn handoff_recovers_when_put_commits_before_unavailable_response() {
    let fixture = fixture(0x67, 0x77);
    let journal = MusubiProviderAttestationJournalV1::new(
        Arc::new(MemoryJournalStore::default()),
        test_policy(),
    )
    .expect("construct journal");
    let (approval_id, handoff) = prepare_handoff_claim(&journal, &fixture, 0xA5).await;
    let inventory = MemoryInventory::default();
    inventory.fail_after_put_once.store(true, Ordering::SeqCst);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(104))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryUnavailable)
    );
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 1);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        journal
            .status(approval_id)
            .await
            .expect("status")
            .expect("handoff retained")
            .stage,
        MusubiProviderAttestationJournalStageV1::HandoffClaimed
    );
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(105))
            .await,
        Ok(MusubiProviderAttestationDeliveryOutcomeV1::Delivered)
    );
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 2);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 1);
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 4);
    assert_eq!(inventory.entries.lock().expect("entries lock").len(), 1);
    assert_eq!(
        journal
            .status(approval_id)
            .await
            .expect("status")
            .expect("delivery retained")
            .stage,
        MusubiProviderAttestationJournalStageV1::Delivered
    );
}
#[tokio::test]
async fn handoff_readback_timeout_never_marks_delivery_and_retries_exactly() {
    let fixture = fixture(0x68, 0x78);
    let journal = MusubiProviderAttestationJournalV1::new(
        Arc::new(MemoryJournalStore::default()),
        test_policy(),
    )
    .expect("construct journal");
    let (approval_id, handoff) = prepare_handoff_claim(&journal, &fixture, 0xA7).await;
    let inventory = MemoryInventory::default();
    inventory.get_delay_ms.store(10, Ordering::SeqCst);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(104))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryUnavailable)
    );
    assert_eq!(
        journal
            .status(approval_id)
            .await
            .expect("status")
            .expect("handoff retained")
            .stage,
        MusubiProviderAttestationJournalStageV1::HandoffClaimed
    );
    inventory.get_delay_ms.store(0, Ordering::SeqCst);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(105))
            .await,
        Ok(MusubiProviderAttestationDeliveryOutcomeV1::Delivered)
    );
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 2);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 2);
    assert_eq!(inventory.entries.lock().expect("entries lock").len(), 1);
}
#[test]
fn inventory_runtime_binding_rejects_invalid_handles_and_inert_qualification() {
    let qualification = MusubiProviderAttestationInventoryQualificationV1::new(7, [0xC1; 32]);
    assert_eq!(qualification.adapter_revision(), 7);
    assert_eq!(qualification.policy_digest(), [0xC1; 32]);
    assert_eq!(
        validate_musubi_provider_attestation_inventory_binding_v1(
            "inventory://sorafs/musubi/primary",
            &qualification,
        ),
        Ok(())
    );
    for handle in ["", "inventory://sorafs/musubi/test"] {
        assert_eq!(
            validate_musubi_provider_attestation_inventory_binding_v1(handle, &qualification,),
            Err(MusubiProviderAttestationInventoryBindingErrorV1::InvalidRuntimeHandle)
        );
    }
    let mut unsupported = qualification;
    unsupported.version = INVENTORY_RUNTIME_QUALIFICATION_VERSION_V1 + 1;
    assert_eq!(
        unsupported.validate(),
        Err(MusubiProviderAttestationInventoryBindingErrorV1::InvalidQualification)
    );
    assert_eq!(
        MusubiProviderAttestationInventoryQualificationV1::new(0, [0xC1; 32]).validate(),
        Err(MusubiProviderAttestationInventoryBindingErrorV1::InvalidQualification)
    );
    assert_eq!(
        MusubiProviderAttestationInventoryQualificationV1::new(1, [0; 32]).validate(),
        Err(MusubiProviderAttestationInventoryBindingErrorV1::InvalidQualification)
    );
}
#[tokio::test]
async fn handoff_rejects_unqualified_inventory_without_put_or_readback() {
    let fixture = fixture(0x69, 0x79);
    let journal = MusubiProviderAttestationJournalV1::new(
        Arc::new(MemoryJournalStore::default()),
        test_policy(),
    )
    .expect("construct journal");
    let (_, handoff) = prepare_handoff_claim(&journal, &fixture, 0xA9).await;
    let inventory = MemoryInventory::default();
    inventory
        .invalid_runtime_handle
        .store(true, Ordering::SeqCst);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(104))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryRejected)
    );
    inventory
        .invalid_runtime_handle
        .store(false, Ordering::SeqCst);
    inventory.test_runtime_handle.store(true, Ordering::SeqCst);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(104))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryRejected)
    );
    inventory.test_runtime_handle.store(false, Ordering::SeqCst);
    inventory.adapter_revision.store(0, Ordering::SeqCst);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(104))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryRejected)
    );
    inventory.adapter_revision.store(1, Ordering::SeqCst);
    *inventory.policy_digest.lock().expect("policy digest lock") = [0; 32];
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(104))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryRejected)
    );
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn handoff_fails_closed_when_inventory_readiness_is_unavailable_or_rejected() {
    let fixture = fixture(0x6A, 0x7A);
    let journal = MusubiProviderAttestationJournalV1::new(
        Arc::new(MemoryJournalStore::default()),
        test_policy(),
    )
    .expect("construct journal");
    let (_, handoff) = prepare_handoff_claim(&journal, &fixture, 0xAA).await;
    let inventory = MemoryInventory::default();
    let readiness = inventory.check_readiness();
    assert_send(&readiness);
    readiness.await.expect("default inventory is ready");
    *inventory.readiness_error.lock().expect("readiness lock") =
        Some(MusubiProviderAttestationInventoryRuntimeErrorV1::Unavailable);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(104))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryUnavailable)
    );
    *inventory.readiness_error.lock().expect("readiness lock") =
        Some(MusubiProviderAttestationInventoryRuntimeErrorV1::Rejected);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(104))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryRejected)
    );
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn handoff_rejects_inventory_identity_or_qualification_drift() {
    let fixture = fixture(0x6B, 0x7B);
    let journal = MusubiProviderAttestationJournalV1::new(
        Arc::new(MemoryJournalStore::default()),
        test_policy(),
    )
    .expect("construct journal");
    let (approval_id, handoff) = prepare_handoff_claim(&journal, &fixture, 0xAB).await;
    let inventory = MemoryInventory::default();
    inventory
        .drift_handle_after_put
        .store(true, Ordering::SeqCst);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(104))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryRejected)
    );
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 1);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 1);
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        journal
            .status(approval_id)
            .await
            .expect("status")
            .expect("handoff retained")
            .stage,
        MusubiProviderAttestationJournalStageV1::HandoffClaimed
    );
    inventory
        .drift_handle_after_put
        .store(false, Ordering::SeqCst);
    inventory
        .drifted_runtime_handle
        .store(false, Ordering::SeqCst);
    inventory
        .drift_qualification_after_put
        .store(true, Ordering::SeqCst);
    assert_eq!(
        journal
            .handoff_claim_with_inventory(&handoff, &inventory, &clock_at(105))
            .await,
        Err(MusubiProviderAttestationJournalErrorV1::InventoryRejected)
    );
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 2);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 2);
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 4);
    assert_eq!(
        journal
            .status(approval_id)
            .await
            .expect("status")
            .expect("handoff retained")
            .stage,
        MusubiProviderAttestationJournalStageV1::HandoffClaimed
    );
}
#[tokio::test]
async fn inventory_replays_exact_item_and_rejects_same_key_different_digest() {
    let first_fixture = fixture(0x48, 0x49);
    let first = MusubiProviderAttestationInventoryItemV1::new(signed_attestation(&first_fixture))
        .expect("first item");
    let inventory = MemoryInventory::default();
    let inserted = inventory.put(first.clone()).await.expect("insert item");
    let replayed = inventory.put(first.clone()).await.expect("replay item");
    assert_eq!(
        inserted, replayed,
        "identical replay returns the exact inventory revision"
    );
    let readback = inventory
        .get(first.scope(), first.key())
        .await
        .expect("read exact item")
        .expect("item exists");
    assert_eq!(readback.item(), &first);
    assert_eq!(readback.inventory_revision(), inserted);
    assert_eq!(
        MusubiProviderAttestationInventoryReadbackV1::try_new(first.clone(), 0),
        Err(MusubiProviderAttestationInventoryErrorV1::InvalidReceipt)
    );
    let inserted_receipt = MusubiProviderAttestationInventoryReceiptV1::new(&first, inserted)
        .expect("journal constructs receipt");
    let replayed_receipt = MusubiProviderAttestationInventoryReceiptV1::new(&first, replayed)
        .expect("journal reconstructs exact receipt");
    assert_eq!(inserted_receipt, replayed_receipt);
    let mut conflicting_attestation = signed_attestation(&first_fixture);
    conflicting_attestation.payload.binding.bundle_digest = MusubiContentDigestV1::new([0xEE; 32]);
    let payload = conflicting_attestation.payload.clone();
    conflicting_attestation.approvals = vec![MusubiProviderBundleVerificationApprovalV1 {
        public_key: first_fixture.owner_key.public_key().clone(),
        signature: SignatureOf::try_from_hash(
            first_fixture.owner_key.private_key(),
            payload.signing_hash(),
        )
        .expect("sign conflicting valid attestation"),
    }];
    let conflicting = MusubiProviderAttestationInventoryItemV1::new(conflicting_attestation)
        .expect("different digest remains structurally valid");
    assert_eq!(first.key(), conflicting.key());
    assert_ne!(first.attestation_digest(), conflicting.attestation_digest());
    assert_eq!(
        inventory.put(conflicting).await,
        Err(MusubiProviderAttestationInventoryErrorV1::Conflict)
    );
}
#[test]
fn inventory_is_canonicalized_by_unique_provider_identity() {
    let item_three =
        MusubiProviderAttestationInventoryItemV1::new(signed_attestation(&fixture(0x63, 0x73)))
            .expect("provider three item");
    let scope = item_three.scope().clone();
    let item_one =
        MusubiProviderAttestationInventoryItemV1::new(signed_attestation(&fixture(0x61, 0x71)))
            .expect("provider one item");
    let item_two =
        MusubiProviderAttestationInventoryItemV1::new(signed_attestation(&fixture(0x62, 0x72)))
            .expect("provider two item");
    let inventory =
        MusubiProviderAttestationInventoryV1::new(scope, vec![item_three, item_one, item_two])
            .expect("canonical inventory");
    assert_eq!(
        inventory
            .items()
            .iter()
            .map(|item| *item.key().provider_id.as_bytes())
            .collect::<Vec<_>>(),
        vec![[0x61; 32], [0x62; 32], [0x63; 32]]
    );
}
#[tokio::test]
async fn signer_validation_rechecks_eligibility_after_approval() {
    let fixture = fixture(0x64, 0x74);
    let signer = FakeApprovalSigner::new(&fixture);
    let qualification = signer.qualification().expect("signer qualification");
    assert_eq!(qualification.adapter_revision(), 1);
    assert_eq!(qualification.adapter_policy_digest(), [0xA7; 32]);
    qualification
        .validate()
        .expect("valid signer qualification");
    let mut unsupported = qualification.clone();
    unsupported.version = APPROVAL_SIGNER_QUALIFICATION_VERSION_V1 + 1;
    assert_eq!(
        unsupported.validate(),
        Err(MusubiProviderAttestationSignerBindingErrorV1::InvalidQualification)
    );
    let mut zero_revision = qualification.clone();
    zero_revision.adapter_revision = 0;
    assert_eq!(
        zero_revision.validate(),
        Err(MusubiProviderAttestationSignerBindingErrorV1::InvalidQualification)
    );
    let mut shared_digest_bytes = qualification.clone();
    shared_digest_bytes.adapter_policy_digest = shared_digest_bytes.signer_policy.policy_digest;
    shared_digest_bytes
        .validate()
        .expect("semantic independence does not require byte inequality");
    assert_eq!(
        MusubiProviderAttestationSignerQualificationV1::new(
            1,
            [0; 32],
            qualification.signer_policy,
            qualification.authority.clone(),
            qualification.controller_policy_digest,
        )
        .validate(),
        Err(MusubiProviderAttestationSignerBindingErrorV1::InvalidQualification)
    );
    let attestation = approve_musubi_provider_attestation_v1(&signer, &fixture.request, 5)
        .await
        .expect("qualified signer succeeds");
    assert_eq!(attestation.payload, *fixture.request.payload());
    assert_eq!(
        approve_musubi_provider_attestation_v1(&signer, &fixture.request, u64::MAX).await,
        Err(MusubiProviderAttestationApprovalErrorV1::InvalidRequest)
    );
    let rotating = FakeApprovalSigner::new(&fixture);
    rotating.rotate_after_approval.store(true, Ordering::SeqCst);
    assert_eq!(
        approve_musubi_provider_attestation_v1(&rotating, &fixture.request, 5).await,
        Err(MusubiProviderAttestationApprovalErrorV1::EligibilityChanged)
    );
    let rotating_adapter = FakeApprovalSigner::new(&fixture);
    rotating_adapter
        .rotate_adapter_after_approval
        .store(true, Ordering::SeqCst);
    assert_eq!(
        approve_musubi_provider_attestation_v1(&rotating_adapter, &fixture.request, 5).await,
        Err(MusubiProviderAttestationApprovalErrorV1::EligibilityChanged)
    );
    let invalid_adapter = FakeApprovalSigner::new(&fixture);
    *invalid_adapter
        .adapter_policy_digest
        .lock()
        .expect("adapter-policy lock") = [0; 32];
    assert_eq!(
        approve_musubi_provider_attestation_v1(&invalid_adapter, &fixture.request, 5).await,
        Err(MusubiProviderAttestationApprovalErrorV1::InvalidSignerQualification)
    );
    assert_eq!(invalid_adapter.approve_calls.load(Ordering::SeqCst), 0);
    let mut substituted_controller = FakeApprovalSigner::new(&fixture);
    substituted_controller.controller_policy_digest = [0xFF; 32];
    assert_eq!(
        approve_musubi_provider_attestation_v1(&substituted_controller, &fixture.request, 5,).await,
        Err(MusubiProviderAttestationApprovalErrorV1::InvalidSignerQualification)
    );
    assert_eq!(
        substituted_controller.approve_calls.load(Ordering::SeqCst),
        0
    );
}
