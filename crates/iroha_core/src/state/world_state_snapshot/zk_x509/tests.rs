//! Real certified State acquisition, complete governance originals and local refusals.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_data_model::{
    isi::privacy::{
        RegisterPrivacyZkX509CertificatePolicyV1, RegisterPrivacyZkX509CrlV1,
        RegisterPrivacyZkX509TrustAnchorV1, RevokePrivacyZkX509CrlV1,
        RotatePrivacyZkX509CertificatePolicyV1,
    },
    privacy::*,
};
use mv::storage::Storage;
use std::{cell::Cell, collections::BTreeSet};

// Genesis uses the production wall-clock admission before the fixture executor clock.
// A fixed historical time keeps that real future-block check meaningful on every run.
const NOW: u64 = 1_700_000_000_000;

struct Fixture {
    chain: CertifiedTestChain,
    signer: iroha_crypto::KeyPair,
    anchor: PrivacyZkX509TrustAnchorRecordV1,
    policy: PrivacyZkX509CertificatePolicyRecordV1,
    crl: PrivacyZkX509CrlRecordV1,
}

impl Fixture {
    fn new() -> Self {
        Self::with_genesis_time(NOW - 1000)
    }

    fn with_genesis_time(genesis_time: u64) -> Self {
        let anchor_id = PrivacyIssuerIdV1::new([0x31; 32]);
        let policy_id = PrivacyPolicyIdV1::new([0x32; 32]);
        let anchor = PrivacyZkX509TrustAnchorRecordV1::new(
            anchor_id,
            1,
            PrivacyX509TrustStoreDigestV1::new([0x33; 32]),
            PrivacyRootV1::new([0x34; 32]),
            1,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .unwrap();
        let policy = PrivacyZkX509CertificatePolicyRecordV1::new(
            anchor_id,
            policy_id,
            1,
            PrivacyPolicyDigestV1::new([0x35; 32]),
            PrivacyX509KeyUsageV1 {
                digital_signature: true.into(),
                content_commitment: false.into(),
                key_encipherment: false.into(),
                key_agreement: false.into(),
            },
            vec![PrivacyX509ExtendedKeyUsageV1::ClientAuthentication],
            vec![],
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .unwrap();
        let crl = PrivacyZkX509CrlRecordV1::new(
            anchor_id,
            policy_id,
            1,
            1,
            PrivacyX509CrlDerDigestV1::new([0x36; 32]),
            PrivacyX509CrlIssuerSpkiDigestV1::new([0x37; 32]),
            NOW / 1000 - 30,
            NOW / 1000 + 600,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .unwrap();
        Self::with_records(genesis_time, anchor, policy, crl)
    }

    fn with_records(
        genesis_time: u64,
        anchor: PrivacyZkX509TrustAnchorRecordV1,
        policy: PrivacyZkX509CertificatePolicyRecordV1,
        crl: PrivacyZkX509CrlRecordV1,
    ) -> Self {
        let mut config = TestChainConfig::new(World::default(), genesis_time);
        let signer = config.genesis_key.clone();
        config.world.account_permissions.insert(
            AccountId::new(signer.public_key().clone()),
            BTreeSet::from([
                iroha_executor_data_model::permission::governance::CanEnactGovernance.into(),
            ]),
        );
        // The signed genesis transaction carries one privacy action, as required
        // by the unchanged per-transaction cap.
        config.genesis_instructions = vec![RegisterPrivacyZkX509TrustAnchorV1::new(anchor).into()];
        let mut chain = CertifiedTestChain::start(config).unwrap();
        // Separate signed transactions install the policy before its CRL while
        // staying within the unchanged two-action block cap at height two.
        let register_policy = chain.sign(
            &signer,
            [RegisterPrivacyZkX509CertificatePolicyV1::new(policy.clone()).into()],
            NOW - 1,
        );
        let register_crl = chain.sign(
            &signer,
            [RegisterPrivacyZkX509CrlV1::new(crl).into()],
            NOW - 1,
        );
        assert_eq!(
            chain.commit_at(NOW, vec![register_policy, register_crl]),
            vec![true, true]
        );
        Self {
            chain,
            signer,
            anchor,
            policy,
            crl,
        }
    }

    // The test callback runs only after the public owned acquisition returns.
    fn acquire<T>(
        &self,
        consume: impl FnOnce(NativeZkX509StateV1) -> Result<T, String>,
    ) -> Result<T, WorldStateSnapshotError> {
        let owned = self.chain.state().acquire_native_zk_x509_state_v1(
            &self.chain.committed(self.chain.height()),
            self.anchor.trust_anchor_id,
            self.policy.policy_id,
            &AllocationBudget::new(32 * 1024 * 1024),
        )?;
        consume(owned).map_err(WorldStateSnapshotError::from)
    }

    fn rotate_policy(&mut self) -> PrivacyZkX509CertificatePolicyRecordV1 {
        let successor = PrivacyZkX509CertificatePolicyRecordV1::new(
            self.anchor.trust_anchor_id,
            self.policy.policy_id,
            2,
            PrivacyPolicyDigestV1::new([0x38; 32]),
            self.policy.required_key_usage,
            self.policy.required_extended_key_usages.clone(),
            vec![0],
            Some(self.policy.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .unwrap();
        let tx = self.chain.sign(
            &self.signer,
            [RotatePrivacyZkX509CertificatePolicyV1::new(
                self.policy.record_digest,
                successor.clone(),
            )
            .into()],
            NOW + 999,
        );
        assert_eq!(self.chain.commit_at(NOW + 1000, vec![tx]), vec![true]);
        successor
    }
}

#[test]
fn provider_fixture_preserves_transaction_boundaries_and_consensus_limits() {
    let fixture = Fixture::new();
    let view = fixture.chain.state().view();
    let policy = view.world().privacy_consensus_policy.get();
    assert_eq!(policy, &PrivacyConsensusPolicyV1::taira_default());
    assert_eq!(policy.current_limits.max_actions_per_transaction, 1);
    assert_eq!(policy.current_limits.max_actions_per_block, 2);
    assert_eq!(fixture.chain.height(), 2);
    let committed = fixture.chain.committed(2);
    let transactions = committed.block().external_transactions();
    assert_eq!(transactions.len(), 2);
    let expected: [iroha_data_model::isi::InstructionBox; 2] = [
        RegisterPrivacyZkX509CertificatePolicyV1::new(fixture.policy.clone()).into(),
        RegisterPrivacyZkX509CrlV1::new(fixture.crl).into(),
    ];
    for (index, (transaction, instruction)) in transactions.zip(&expected).enumerate() {
        assert!(matches!(
            transaction.instructions(),
            iroha_data_model::transaction::Executable::Instructions(instructions)
                if instructions.iter().eq(core::iter::once(instruction))
        ));
        assert!(
            committed
                .block()
                .network_output_at(u32::try_from(index).unwrap())
                .unwrap()
                .1
                .result
                .is_ok()
        );
    }
}

#[test]
fn certified_provider_joins_original_records_policy_identity_and_ledger_time() {
    let fixture = Fixture::new();
    let expected_policy = *fixture
        .chain
        .state()
        .view()
        .world()
        .privacy_consensus_policy
        .get();
    let expected_tip = fixture.chain.committed(fixture.chain.height());
    let expected_network = fixture.chain.network_id();
    fixture
        .acquire(|value| {
            assert_eq!(value.authoritative_state().trust_anchor(), fixture.anchor);
            assert_eq!(
                value.authoritative_state().certificate_policy(),
                &fixture.policy
            );
            assert_eq!(value.authoritative_state().crl_record(), fixture.crl);
            assert_eq!(value.authoritative_state().ca_membership_root_epoch(), 1);
            assert_eq!(
                value.authoritative_state().ca_membership_root(),
                fixture.anchor.ca_membership_root
            );
            assert_eq!(value.consensus_policy(), &expected_policy);
            assert_eq!(value.certified_block_id(), expected_tip.id());
            assert_eq!(value.network_id(), expected_network);
            assert_eq!(value.height(), expected_tip.height());
            assert_eq!(value.root_scope(), expected_tip.root_scope());
            assert_eq!(
                value.world_state_root(),
                expected_tip.commitment().execution.world_state_root
            );
            assert_eq!(
                value.schema_hash(),
                State::native_world_schema_hash_v1().unwrap()
            );
            assert_eq!(value.ledger_timestamp_ms(), NOW);
            Ok(())
        })
        .unwrap();
}

#[test]
fn new_policy_is_selected_and_omitting_it_from_a_genuine_cut_is_rejected() {
    let mut fixture = Fixture::new();
    let old_tip = fixture.chain.committed(fixture.chain.height());
    let successor = fixture.rotate_policy();
    fixture
        .acquire(|value| {
            assert_eq!(value.authoritative_state().certificate_policy(), &successor);
            assert_eq!(value.ledger_timestamp_ms(), NOW + 1000);
            Ok(())
        })
        .unwrap();
    let state = fixture.chain.state();
    let budget = AllocationBudget::new(32 * 1024 * 1024);
    assert!(matches!(
        state.acquire_native_zk_x509_state_v1(
            &old_tip,
            fixture.anchor.trust_anchor_id,
            fixture.policy.policy_id,
            &budget
        ),
        Err(WorldStateSnapshotError::CutChanged { .. })
    ));
    let tip = fixture.chain.committed(fixture.chain.height());
    state
        .with_native_world_state_snapshot_cut_v1(&tip, None, &budget, |snapshot, world| {
            let newer = PrivacyCommitmentKeyV1::zk_x509_certificate_policy_revision(
                fixture.anchor.trust_anchor_id,
                fixture.policy.policy_id,
                2,
            )
            .unwrap();
            let mut filtered = Storage::new();
            for (key, value) in world.privacy_commitments.iter() {
                if *key != newer {
                    filtered.insert(*key, value.clone());
                }
            }
            // All selected old records remain internally consistent. Only certified
            // completeness prevents this rollback; a loader/point-proof test cannot.
            let old = load_privacy_zk_x509_authoritative_state_v1(
                fixture.anchor.trust_anchor_id,
                fixture.policy.policy_id,
                world
                    .privacy_consensus_policy
                    .get()
                    .admission_retained_root_count(),
                &filtered.view(),
                &world.privacy_roots,
                &world.privacy_root_heads,
            )
            .unwrap();
            assert_eq!(old.certificate_policy(), &fixture.policy);
            assert!(
                authenticate_originals(
                    snapshot,
                    &filtered.view(),
                    &world.privacy_roots,
                    &world.privacy_root_heads,
                    world.privacy_consensus_policy.get()
                )
                .unwrap_err()
                .contains("omits or adds")
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn certified_originals_reject_root_head_and_policy_substitution_and_missing_ranges() {
    let fixture = Fixture::new();
    let state = fixture.chain.state();
    let tip = fixture.chain.committed(fixture.chain.height());
    state
        .with_native_world_state_snapshot_cut_v1(
            &tip,
            None,
            &AllocationBudget::new(32 * 1024 * 1024),
            |snapshot, world| {
                let commitments = &world.privacy_commitments;
                let roots = &world.privacy_roots;
                let heads = &world.privacy_root_heads;
                let policy = world.privacy_consensus_policy.get();
                authenticate_originals(snapshot, commitments, roots, heads, policy)?;
                // Each of the three governance roles is independently required, including
                // current CRL keys rather than a host-selected active row.
                for omitted in commitments.iter().map(|(key, _)| *key) {
                    let mut filtered = Storage::new();
                    for (key, value) in commitments.iter() {
                        if *key != omitted {
                            filtered.insert(*key, value.clone());
                        }
                    }
                    assert!(
                        authenticate_originals(snapshot, &filtered.view(), roots, heads, policy)
                            .is_err()
                    );
                }
                let empty_roots = Storage::<PrivacyRootKeyV1, PrivacyRootProvenanceV1>::new();
                assert!(
                    authenticate_originals(
                        snapshot,
                        commitments,
                        &empty_roots.view(),
                        heads,
                        policy
                    )
                    .is_err()
                );
                let empty_heads = Storage::<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>::new();
                assert!(
                    authenticate_originals(
                        snapshot,
                        commitments,
                        roots,
                        &empty_heads.view(),
                        policy
                    )
                    .is_err()
                );
                let mut changed_policy = *policy;
                changed_policy.current_limits.retained_root_count -= 1;
                changed_policy.validate().unwrap();
                assert!(
                    authenticate_originals(snapshot, commitments, roots, heads, &changed_policy)
                        .is_err()
                );
                // Same table cardinality cannot hide a different typed value/provenance.
                let mut changed = Storage::new();
                for (key, value) in commitments.iter() {
                    let value = match value {
                        PrivacyStateItemRecordV1::ZkX509TrustAnchorGovernance {
                            record,
                            admitted_at_height,
                        } => PrivacyStateItemRecordV1::zk_x509_trust_anchor_governance(
                            *record,
                            admitted_at_height + 1,
                        )
                        .unwrap(),
                        other => other.clone(),
                    };
                    changed.insert(*key, value);
                }
                assert!(
                    authenticate_originals(snapshot, &changed.view(), roots, heads, policy)
                        .is_err()
                );
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn committed_crl_revocation_stops_the_provider_before_its_consumer() {
    let mut fixture = Fixture::new();
    let old = fixture.crl;
    let revoked = PrivacyZkX509CrlRecordV1::new(
        old.trust_anchor_id,
        old.certificate_policy_id,
        2,
        old.crl_number,
        old.crl_der_digest,
        old.issuer_spki_digest,
        old.this_update_unix_seconds,
        old.next_update_unix_seconds,
        Some(old.record_digest),
        PrivacyZkX509RecordLifecycleV1::Revoked,
    )
    .unwrap();
    let tx = fixture.chain.sign(
        &fixture.signer,
        [RevokePrivacyZkX509CrlV1::new(old.record_digest, revoked).into()],
        NOW + 999,
    );
    assert_eq!(fixture.chain.commit_at(NOW + 1000, vec![tx]), vec![true]);
    let called = Cell::new(false);
    let error = fixture
        .acquire(|_| {
            called.set(true);
            Ok(())
        })
        .unwrap_err();
    assert!(error.to_string().contains("revoked"), "{error}");
    assert!(!called.get());
}

#[test]
fn missing_capture_budget_and_reader_refusal_return_no_owner() {
    let fixture = Fixture::new();
    let state = fixture.chain.state();
    let tip = fixture.chain.committed(fixture.chain.height());
    let zero = AllocationBudget::new(0);
    assert!(
        state
            .acquire_native_zk_x509_state_v1(
                &tip,
                fixture.anchor.trust_anchor_id,
                fixture.policy.policy_id,
                &zero
            )
            .is_err()
    );
    assert_eq!(zero.reserved_bytes(), 0);
    let budget = AllocationBudget::new(32 * 1024 * 1024);
    let held = state.latest_block_header.write();
    let expected = state.latest_block_header.try_read_or_wait().err().unwrap();
    let error = state
        .acquire_native_zk_x509_state_v1(
            &tip,
            fixture.anchor.trust_anchor_id,
            fixture.policy.policy_id,
            &budget,
        )
        .unwrap_err();
    let WorldStateSnapshotError::View(StateViewError::Busy(actual)) = error else {
        panic!("original reader refusal")
    };
    assert_eq!(actual, expected);
    assert_eq!(budget.reserved_bytes(), 0);
    drop(held);
    *state.native_world_cut.lock() = None;
    let error = state
        .acquire_native_zk_x509_state_v1(
            &tip,
            fixture.anchor.trust_anchor_id,
            fixture.policy.policy_id,
            &budget,
        )
        .unwrap_err();
    assert!(error.to_string().contains("requires native replay"));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn equal_height_foreign_network_cut_is_rejected_before_owner_return() {
    let fixture = Fixture::new();
    let foreign = Fixture::with_genesis_time(NOW - 2000);
    assert_eq!(fixture.chain.height(), foreign.chain.height());
    assert_ne!(fixture.chain.network_id(), foreign.chain.network_id());
    let budget = AllocationBudget::new(32 * 1024 * 1024);
    let error = fixture
        .chain
        .state()
        .acquire_native_zk_x509_state_v1(
            &foreign.chain.committed(foreign.chain.height()),
            fixture.anchor.trust_anchor_id,
            fixture.policy.policy_id,
            &budget,
        )
        .unwrap_err();
    assert!(error.to_string().contains("another network"));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn owned_selection_releases_all_world_writers_and_keeps_exact_retained_charges() {
    let mut fixture = Fixture::new();
    let state = fixture.chain.state().clone();
    let tip = fixture.chain.committed(fixture.chain.height());
    let budget = AllocationBudget::new(32 * 1024 * 1024);
    let owned = state
        .acquire_native_zk_x509_state_v1(
            &tip,
            fixture.anchor.trust_anchor_id,
            fixture.policy.policy_id,
            &budget,
        )
        .unwrap();
    let retained = owned.authoritative.allocation_bytes().unwrap();
    assert!(retained > 0);
    assert_eq!(
        budget.reserved_bytes(),
        retained,
        "complete capture scratch was released"
    );
    // A returned selected context must not own even one complete World writer.
    let execution_budget = state.ivm_execution_budget();
    let writer = state
        .world
        .try_block(&execution_budget)
        .expect("all World writers released");
    drop(writer);
    let successor = fixture.rotate_policy();
    assert_eq!(
        owned.authoritative_state().certificate_policy(),
        &fixture.policy
    );
    assert_eq!(owned.ledger_timestamp_ms(), NOW);
    assert!(matches!(
        state.acquire_native_zk_x509_state_v1(
            &tip,
            fixture.anchor.trust_anchor_id,
            fixture.policy.policy_id,
            &budget,
        ),
        Err(WorldStateSnapshotError::CutChanged { .. })
    ));
    let current = state
        .acquire_native_zk_x509_state_v1(
            &fixture.chain.committed(fixture.chain.height()),
            fixture.anchor.trust_anchor_id,
            fixture.policy.policy_id,
            &budget,
        )
        .unwrap();
    assert_eq!(
        current.authoritative_state().certificate_policy(),
        &successor
    );
    assert_eq!(current.ledger_timestamp_ms(), NOW + 1000);
    assert_ne!(owned.certified_block_id(), current.certified_block_id());
    let current_bytes = current.authoritative.allocation_bytes().unwrap();
    assert_eq!(budget.reserved_bytes(), retained + current_bytes);
    drop(current);
    assert_eq!(budget.reserved_bytes(), retained);
    drop(owned);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn retained_policy_partial_allocation_refusals_refund_only_destroyed_backing() {
    use iroha_allocation::AllocationCharge;
    use iroha_core_privacy::privacy_state::PrivacyZkX509StateRetentionErrorV1;
    let mut fixture = Fixture::new();
    fixture.rotate_policy(); // Both selected policy vectors are nonempty.
    fixture
        .acquire(|owned| {
            let original = owned.authoritative_state();
            let ledger = Layout::array::<AllocationCharge>(2).unwrap().size();
            let extended = Layout::array::<PrivacyX509ExtendedKeyUsageV1>(
                original
                    .certificate_policy()
                    .required_extended_key_usages
                    .len(),
            )
            .unwrap()
            .size();
            let disclosed = Layout::array::<u8>(
                original
                    .certificate_policy()
                    .required_disclosed_attribute_indices
                    .len(),
            )
            .unwrap()
            .size();
            assert!(extended > 0 && disclosed > 0);
            for available in [0, ledger, ledger + extended] {
                let budget = AllocationBudget::new(available);
                let result = original.clone().try_into_retained_v1(&budget);
                assert!(matches!(
                    result,
                    Err(PrivacyZkX509StateRetentionErrorV1::Buffer(_))
                ));
                assert_eq!(budget.reserved_bytes(), 0);
            }
            let budget = AllocationBudget::new(ledger + extended + disclosed);
            let retained = original.clone().try_into_retained_v1(&budget).unwrap();
            assert_eq!(retained.get(), original);
            assert_eq!(
                retained.allocation_bytes(),
                Some(ledger + extended + disclosed)
            );
            assert!(retained.belongs_to(&budget));
            assert_eq!(budget.reserved_bytes(), ledger + extended + disclosed);
            drop(retained);
            assert_eq!(budget.reserved_bytes(), 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn final_generation_refusal_destroys_retained_selection_before_refunding() {
    let fixture = Fixture::new();
    let state = fixture.chain.state();
    let tip = fixture.chain.committed(fixture.chain.height());
    let budget = AllocationBudget::new(32 * 1024 * 1024);
    let retained_before_recheck = Cell::new(false);
    let result =
        state.with_native_world_state_snapshot_cut_v1(&tip, None, &budget, |snapshot, world| {
            let policy = world.privacy_consensus_policy.get();
            authenticate_originals(
                snapshot,
                &world.privacy_commitments,
                &world.privacy_roots,
                &world.privacy_root_heads,
                policy,
            )?;
            let retained = load_privacy_zk_x509_authoritative_state_v1(
                fixture.anchor.trust_anchor_id,
                fixture.policy.policy_id,
                policy.admission_retained_root_count(),
                &world.privacy_commitments,
                &world.privacy_roots,
                &world.privacy_root_heads,
            )?
            .try_into_retained_v1(&budget)?;
            assert!(retained.allocation_bytes().unwrap() > 0);
            retained_before_recheck.set(true);
            // The existing internal test seam changes publication generation,
            // without exposing any callback in the public owned provider.
            let mut publication = state.state_view_publication();
            let _writer = publication.begin();
            Ok(retained)
        });
    let error = result.err().expect("final generation check must reject");
    assert!(retained_before_recheck.get());
    assert!(error.to_string().contains("generation changed"), "{error}");
    assert_eq!(budget.reserved_bytes(), 0);
}

// Existing evidence-only DER fixtures are required for this genuine holder
// preflight. This test adds no production gate and generates no credential proof.
#[cfg(feature = "privacy-release-evidence")]
#[test]
fn owned_context_stages_real_holder_outside_writers_then_rejects_rotated_selection() {
    use iroha_core_privacy::privacy_engines::zk_x509::{
        holder::{ZkX509EngineErrorV1, ZkX509HolderAttemptErrorV1, ZkX509HolderAttemptV1},
        relation::release_fixture::{
            build_zk_x509_network_release_fixture_v1, reference_statement_context_v1,
        },
    };
    struct NoEntropy(usize);
    impl rand::TryRng for NoEntropy {
        type Error = std::io::Error;
        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            self.0 += 1;
            Err(std::io::Error::other("stale attempt must not request entropy"))
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            self.0 += 1;
            Err(std::io::Error::other("stale attempt must not request entropy"))
        }
        fn try_fill_bytes(&mut self, _: &mut [u8]) -> Result<(), Self::Error> {
            self.0 += 1;
            Err(std::io::Error::other("stale attempt must not request entropy"))
        }
    }
    impl rand::TryCryptoRng for NoEntropy {}
    let wallet = iroha_test_samples::ALICE_ID.clone();
    let records = build_zk_x509_network_release_fixture_v1(
        reference_statement_context_v1(),
        NOW,
        wallet.clone(),
    )
    .unwrap();
    let mut fixture = Fixture::with_records(
        NOW - 1000,
        records.authoritative_state.trust_anchor(),
        records.authoritative_state.certificate_policy().clone(),
        records.authoritative_state.crl_record(),
    );
    let mut context = reference_statement_context_v1();
    context.network_id = fixture.chain.network_id();
    let signed = build_zk_x509_network_release_fixture_v1(context, NOW, wallet).unwrap();
    assert_eq!(signed.authoritative_state, records.authoritative_state);
    let state = fixture.chain.state().clone();
    let old_tip = fixture.chain.committed(fixture.chain.height());
    let budget = AllocationBudget::new(32 * 1024 * 1024);
    let original = state
        .acquire_native_zk_x509_state_v1(
            &old_tip,
            fixture.anchor.trust_anchor_id,
            fixture.policy.policy_id,
            &budget,
        )
        .unwrap();
    assert_eq!(original.authoritative_state(), &signed.authoritative_state);
    let original_statement = signed.statement.clone();
    let attempt = ZkX509HolderAttemptV1::prepare(
        signed.statement,
        signed.witness,
        original.authoritative_state(),
        original.ledger_timestamp_ms(),
        &original.consensus_policy().current_limits,
        *original.network_id().as_bytes(),
        PrivacyZkX509HolderBudgetV1 {
            observation_ms: 0,
            preparation_ms: 0,
            proving_ms: 300_000,
            publication_jitter_ms: 0,
            submission_ms: 0,
            clock_margin_ms: 0,
        },
        NOW,
    )
    .expect("real signed holder preflight after all World writers released");
    assert_eq!(attempt.original(), &original_statement);
    // Real signed successor commits while both the old context and owned attempt live.
    fixture.rotate_policy();
    assert!(matches!(
        state.acquire_native_zk_x509_state_v1(
            &old_tip,
            fixture.anchor.trust_anchor_id,
            fixture.policy.policy_id,
            &budget,
        ),
        Err(WorldStateSnapshotError::CutChanged { .. })
    ));
    let current = state
        .acquire_native_zk_x509_state_v1(
            &fixture.chain.committed(fixture.chain.height()),
            fixture.anchor.trust_anchor_id,
            fixture.policy.policy_id,
            &budget,
        )
        .unwrap();
    let mut entropy = NoEntropy(0);
    let result = attempt.prove_with_rng(
        current.authoritative_state(),
        current.ledger_timestamp_ms(),
        &current.consensus_policy().current_limits,
        *current.network_id().as_bytes(),
        NOW + 1000,
        &mut entropy,
    );
    assert!(matches!(
        result,
        Err(ZkX509HolderAttemptErrorV1::Engine(
            ZkX509EngineErrorV1::InvalidAuthoritativeState(_)
        ))
    ));
    assert_eq!(entropy.0, 0);
    // No sentinel proof is manufactured. Successful proving/release remains the
    // separate genuine full-proof diagnostic; current State cancels this attempt.
    drop(current);
    drop(original);
    assert_eq!(budget.reserved_bytes(), 0);
}
