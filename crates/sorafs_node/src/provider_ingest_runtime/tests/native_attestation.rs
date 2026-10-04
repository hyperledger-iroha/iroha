// Real retained Node/CAR/journal/inventory component coverage. The existing signed
// source fixture is synthetic ledger evidence, not native consensus qualification.
mod native_attestation_capture {
    use super::*;
    use crate::{
        provider_attestation_journal::{
            MusubiProviderAttestationClaimOwnerV1, MusubiProviderAttestationSignerErrorV1,
            MusubiProviderAttestationSignerQualificationV1, MusubiProviderAttestationSignerV1,
            musubi_provider_attestation_controller_policy_digest_v1,
        },
        provider_attestation_native::NativeMusubiProviderAttestationCustodyV1,
    };

    struct ComponentSigner {
        key: KeyPair,
        account: AccountId,
        calls: AtomicUsize,
    }
    impl ComponentSigner {
        fn new() -> Self {
            let key = KeyPair::try_from_seed(vec![8; 32], Algorithm::Ed25519).unwrap();
            Self {
                account: AccountId::new(key.public_key().clone()),
                key,
                calls: AtomicUsize::new(0),
            }
        }
    }
    impl MusubiProviderAttestationSignerV1 for ComponentSigner {
        fn runtime_handle(&self) -> &str {
            "software://sorafs/provider-attestation-approval"
        }
        fn authority(&self) -> &AccountId {
            &self.account
        }
        fn qualification(
            &self,
        ) -> Result<
            MusubiProviderAttestationSignerQualificationV1,
            MusubiProviderAttestationSignerErrorV1,
        > {
            let controller = musubi_provider_attestation_controller_policy_digest_v1(&self.account)
                .map_err(|_| MusubiProviderAttestationSignerErrorV1::Rejected)?;
            Ok(MusubiProviderAttestationSignerQualificationV1::new(
                1,
                controller,
                self.signer_policy(),
                self.account.clone(),
                controller,
            ))
        }
        fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
            completion_signer_policy(1)
        }
        fn current_eligibility(
            &self,
        ) -> Result<ProviderIngestCompletionSignerPolicyV1, MusubiProviderAttestationSignerErrorV1>
        {
            // Component adapter only: no claim that this is a native current-state read.
            Ok(self.signer_policy())
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
                if request.payload().binding.completed_by != self.account
                    || request.signer_policy() != self.signer_policy()
                {
                    return Err(MusubiProviderAttestationSignerErrorV1::Rejected);
                }
                self.calls.fetch_add(1, Ordering::SeqCst);
                let payload = request.payload().clone();
                let signature =
                    SignatureOf::try_from_hash(self.key.private_key(), payload.signing_hash())
                        .map_err(|_| MusubiProviderAttestationSignerErrorV1::Rejected)?;
                Ok(MusubiProviderBundleVerificationAttestationV1 {
                    payload,
                    approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
                        public_key: self.key.public_key().clone(),
                        signature,
                    }],
                })
            })
        }
    }

    #[test]
    fn actual_bundle_native_journal_handoff_and_reopen_preserve_single_signature() {
        // The test alone installs one synchronous original allowance around a current-thread
        // executor. Production never keeps a thread-local decode guard across an async await.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let policy = MusubiProviderAttestationJournalPolicyV1 {
            max_entries: 4,
            checkpoint_max_bytes: 4 * 1024 * 1024,
            ..MusubiProviderAttestationJournalPolicyV1::default()
        };
        let worker_allowance = 8 * (policy.checkpoint_max_bytes + 4096);
        let page_limits = norito::DecodeLimits::new(
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            32 * 1024 * 1024,
            64 * worker_allowance + 16 * 1024 * 1024,
            128,
        );
        let mut driver = runtime.block_on(async {
            let fixture = verified_attestation_bundle_fixture(0xE9);
            let manifest = completed_attestation_manifest(&fixture);
            let row = completed_attestation_capture_source_row(&fixture, &manifest);
            let ledger = Arc::new(CaptureScannerLedgerV1::new(
                vec![row],
                8,
                CaptureScannerLedgerFaultV1::None,
            ));
            let root = tempfile::tempdir().unwrap();
            let node = capture_coordinator_test_handle(root.path());
            node.ingest_manifest(&manifest, &fixture.plan, &mut fixture.payload.as_slice())
                .unwrap();
            let provider = ProviderId::new(LOCAL_PROVIDER);
            NativeMusubiProviderAttestationCustodyV1::initialize(
                root.path(),
                test_network_id(),
                provider,
                policy,
            )
            .unwrap();
            let custody = NativeMusubiProviderAttestationCustodyV1::open(
                root.path(),
                test_network_id(),
                provider,
                policy,
            )
            .unwrap();
            let inventory = custody.inventory();
            let signer = Arc::new(ComponentSigner::new());
            let coordinator = node
                .take_provider_ingest_completed_musubi_capture_coordinator(
                    test_network_id(),
                    1,
                    ledger,
                )
                .unwrap();
            let driver = node
                .bind_provider_ingest_completed_musubi_attestation_driver_v1(
                    coordinator,
                    custody.runtime(),
                    MusubiProviderAttestationClaimOwnerV1::new([0x5A; 32]).unwrap(),
                    policy,
                    signer.clone(),
                    inventory.clone(),
                )
                .unwrap();
            // No native signature or intent exists yet. One worker allowance covers the clock
            // phase only; the next retained read must refuse instead of renewing caller capacity.
            let original =
                std::fs::read(root.path().join("provider-attestation-native/journal.nrt")).unwrap();
            (driver, custody, inventory, signer, fixture, root, original)
        });
        let tight = norito::DecodeLimits::new(
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            32 * 1024 * 1024,
            worker_allowance,
            128,
        );
        assert!(
            norito::with_decode_limits_scope(tight, || {
                runtime.block_on(driver.0.drive_one_bounded_page())
            })
            .is_err()
        );
        assert_eq!(driver.3.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            std::fs::read(
                driver
                    .5
                    .path()
                    .join("provider-attestation-native/journal.nrt")
            )
            .unwrap(),
            driver.6
        );
        norito::with_decode_limits_scope(page_limits, || {
            runtime.block_on(async move {
                let (mut driver, custody, inventory, signer, fixture, root, _) = driver;
                let provider = ProviderId::new(LOCAL_PROVIDER);
                let first = driver.drive_one_bounded_page().await.unwrap();
                assert_eq!(first.capture_candidates, 1);
                assert_eq!(first.approvals_inserted, 1);
                assert_eq!(first.approvals_stored, 1);
                assert_eq!(first.handoffs_delivered, 0);
                let second = driver.drive_one_bounded_page().await.unwrap();
                assert_eq!(second.handoffs_delivered, 1);
                assert_eq!(second.approvals_stored, 0);
                assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
                let expected = completed_attestation_inventory_item(&fixture, false);
                let readback = inventory
                    .get(expected.scope(), expected.key())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(readback.item(), &expected);
                let revision = readback.inventory_revision();
                drop(readback);
                drop(driver);
                drop(inventory);
                drop(custody);
                let reopened = NativeMusubiProviderAttestationCustodyV1::open(
                    root.path(),
                    test_network_id(),
                    provider,
                    policy,
                )
                .unwrap();
                let inventory = reopened.inventory();
                let retained = inventory
                    .get(expected.scope(), expected.key())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(retained.item(), &expected);
                assert_eq!(retained.inventory_revision(), revision);
                assert_eq!(inventory.put(expected.clone()).await.unwrap(), revision);
                let exact_set = inventory
                    .inventory(expected.scope())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(exact_set.items(), &[expected]);
                assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
                assert!(
                    reopened
                        .runtime()
                        .ready_handoff_page(None, 1)
                        .await
                        .unwrap()
                        .is_empty()
                );
            })
        });
    }
}
