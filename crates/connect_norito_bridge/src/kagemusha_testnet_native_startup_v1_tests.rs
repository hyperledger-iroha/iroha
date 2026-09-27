//! Native startup authenticity, exclusive lifecycle and uncertain-installation regressions.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use iroha_crypto::{Algorithm, KeyPair, SignatureOf};

use super::*;
use crate::kagemusha_mobile_bootstrap_v1::verified_test_bootstrap_v1;
use iroha_data_model::kagemusha::{
    KagemushaMobileBootstrapApprovalV1, KagemushaMobileBootstrapPackageV1,
};

struct Freshness {
    value: Mutex<KagemushaTestnetNativeStartupFreshnessV1>,
    writes: AtomicUsize,
    fail_write: AtomicBool,
    omit_write: AtomicBool,
}

impl Freshness {
    fn new() -> Self {
        Self {
            value: Mutex::new(KagemushaTestnetNativeStartupFreshnessV1 {
                trusted_now_ms: 1500,
                minimum_sequence: 1,
                previous: None,
            }),
            writes: AtomicUsize::new(0),
            fail_write: AtomicBool::new(false),
            omit_write: AtomicBool::new(false),
        }
    }
}

impl KagemushaTestnetNativeStartupFreshnessProviderV1 for Freshness {
    fn read_freshness(&self) -> Result<KagemushaTestnetNativeStartupFreshnessV1, String> {
        Ok(*self.value.lock().unwrap())
    }

    fn retain_verified_bootstrap(
        &self,
        pin: KagemushaMobileBootstrapReplayPinV1,
    ) -> Result<(), String> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        if self.fail_write.load(Ordering::SeqCst) {
            return Err("uncertain write".to_owned());
        }
        if !self.omit_write.load(Ordering::SeqCst) {
            self.value.lock().unwrap().previous = Some(pin);
        }
        Ok(())
    }
}

fn profile() -> KagemushaRecursiveVerifierProfileV1 {
    KagemushaRecursiveVerifierProfileV1 {
        inner_state_eq: Default::default(),
        inner_state_ep: Default::default(),
        state_eq: Default::default(),
        state_ep: Default::default(),
        guard_eq: Default::default(),
        guard_ep: Default::default(),
        terminal_authorization_eq: Default::default(),
        terminal_authorization_ep: Default::default(),
        commit_wrapper_eq: Default::default(),
        commit_wrapper_ep: Default::default(),
        mint_authorization_eq: Default::default(),
        mint_authorization_ep: Default::default(),
        mint_eq: Default::default(),
        mint_ep: Default::default(),
        inner_mint_authorization_eq: Default::default(),
        inner_mint_authorization_ep: Default::default(),
        inner_mint_eq: Default::default(),
        inner_mint_ep: Default::default(),
        mint_hash_shard_eq: Default::default(),
        mint_hash_shard_ep: Default::default(),
        mint_hash_claim_eq: Default::default(),
        mint_hash_claim_ep: Default::default(),
        mint_eq_protocol_digest: [1; 32],
        mint_ep_protocol_digest: [2; 32],
        mint_hash_shard_eq_protocol_digest: [3; 32],
        mint_hash_shard_ep_protocol_digest: [4; 32],
        mint_hash_claim_eq_protocol_digest: [5; 32],
        mint_hash_claim_ep_protocol_digest: [6; 32],
        mint_genesis_authorization_id: [7; 32],
    }
}

pub(super) fn context() -> KagemushaTestnetNativeStartupContextV1 {
    let token = verified_test_bootstrap_v1();
    KagemushaTestnetNativeStartupContextV1 {
        authority_policy: token.trusted_authority_policy().clone(),
        network_id: token.network_id(),
        scope: token.scope(),
        release_id: token.release_id(),
        release_attestation_digest: token.release_attestation_digest(),
        manifest_archive: Vec::new(),
        validation_receipt_archive: Vec::new(),
        release_attestation_archive: Vec::new(),
        profile: profile(),
        artifact_root: PathBuf::from("/missing-startup-test-artifacts"),
        mint_journal_path: PathBuf::from("/missing-startup-test-mint"),
        value_ledger_path: PathBuf::from("/missing-startup-test-ledger"),
        mode: KagemushaTestnetDurableObservationModeV1::Create,
        independent_anchors: BTreeMap::new(),
    }
}

fn archive(sequence: u64) -> Vec<u8> {
    let mut checkpoint = *verified_test_bootstrap_v1().checkpoint();
    checkpoint.sequence = sequence;
    let mut keys: Vec<_> = [41, 42, 43]
        .into_iter()
        .map(|byte| KeyPair::from_seed(vec![byte; 32], Algorithm::Ed25519))
        .collect();
    keys.sort_by(|a, b| a.public_key().cmp(b.public_key()));
    norito::encode_canonical(&KagemushaMobileBootstrapPackageV1 {
        checkpoint,
        approvals: keys[..2]
            .iter()
            .map(|key| KagemushaMobileBootstrapApprovalV1 {
                public_key: key.public_key().clone(),
                signature: SignatureOf::try_new(key.private_key(), &checkpoint.approval_payload())
                    .unwrap(),
            })
            .collect(),
    })
    .unwrap()
}

#[test]
fn native_startup_verifies_before_storage_and_retries_malformed_input() {
    let ctx = context();
    let provider = Freshness::new();
    let mut state = StartupState::Cold;
    assert!(activate(&ctx, &provider, &mut state, b"bad", |_| Ok(7_u8)).is_err());
    assert!(matches!(state, StartupState::Cold));
    assert_eq!(provider.writes.load(Ordering::SeqCst), 0);
    activate(&ctx, &provider, &mut state, &archive(10), |verified| {
        assert_eq!(
            provider.read_freshness()?.previous,
            Some(verified.replay_pin())
        );
        Ok(7_u8)
    })
    .unwrap();
    assert!(matches!(state, StartupState::Active { host: 7, .. }));
    assert_eq!(provider.writes.load(Ordering::SeqCst), 1);
}

#[test]
fn native_startup_exact_retry_revalidates_time_and_never_reinstalls() {
    let ctx = context();
    let provider = Freshness::new();
    let mut state = StartupState::Cold;
    let bytes = archive(10);
    activate(&ctx, &provider, &mut state, &bytes, |_| Ok(7_u8)).unwrap();
    activate(&ctx, &provider, &mut state, &bytes, |_| {
        panic!("reinstalled")
    })
    .unwrap();
    assert_eq!(provider.writes.load(Ordering::SeqCst), 1);
    assert!(
        activate(&ctx, &provider, &mut state, &archive(11), |_| panic!(
            "replaced"
        ))
        .is_err()
    );
    provider.value.lock().unwrap().trusted_now_ms = 300_000;
    assert!(activate(&ctx, &provider, &mut state, &bytes, |_| panic!("expired")).is_err());
    assert!(matches!(state, StartupState::Active { host: 7, .. }));
}

#[test]
fn native_startup_rejects_wrong_scope_and_policy_without_poisoning() {
    let mut ctx = context();
    let provider = Freshness::new();
    let mut state = StartupState::Cold;
    ctx.scope.asset_scale += 1;
    assert!(activate(&ctx, &provider, &mut state, &archive(10), |_| Ok(7_u8)).is_err());
    ctx.scope.asset_scale -= 1;
    ctx.authority_policy.authority_set_id = [99; 32];
    assert!(activate(&ctx, &provider, &mut state, &archive(10), |_| Ok(7_u8)).is_err());
    assert!(matches!(state, StartupState::Cold));
    assert_eq!(provider.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn native_startup_uncertain_persistence_and_false_success_poison_without_installing() {
    for false_success in [false, true] {
        let ctx = context();
        let provider = Freshness::new();
        provider.fail_write.store(!false_success, Ordering::SeqCst);
        provider.omit_write.store(false_success, Ordering::SeqCst);
        let mut state: StartupState<u8> = StartupState::Cold;
        assert!(
            activate(&ctx, &provider, &mut state, &archive(10), |_| panic!(
                "installed"
            ))
            .is_err()
        );
        assert!(matches!(state, StartupState::Poisoned));
        assert!(
            activate(&ctx, &provider, &mut state, &archive(10), |_| panic!(
                "retried"
            ))
            .is_err()
        );
        assert_eq!(provider.writes.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn native_startup_rechecks_freshness_after_retention_before_installing() {
    struct ExpiringFreshness(Freshness);

    impl KagemushaTestnetNativeStartupFreshnessProviderV1 for ExpiringFreshness {
        fn read_freshness(&self) -> Result<KagemushaTestnetNativeStartupFreshnessV1, String> {
            let mut current = self.0.read_freshness()?;
            if self.0.writes.load(Ordering::SeqCst) > 0 {
                current.trusted_now_ms = 300_000;
            }
            Ok(current)
        }

        fn retain_verified_bootstrap(
            &self,
            pin: KagemushaMobileBootstrapReplayPinV1,
        ) -> Result<(), String> {
            self.0.retain_verified_bootstrap(pin)
        }
    }

    let provider = ExpiringFreshness(Freshness::new());
    let mut state: StartupState<u8> = StartupState::Cold;
    assert!(
        activate(&context(), &provider, &mut state, &archive(10), |_| {
            panic!("expired retained checkpoint must not open either journal")
        })
        .is_err()
    );
    assert_eq!(provider.0.writes.load(Ordering::SeqCst), 1);
    assert!(matches!(state, StartupState::Poisoned));
}

#[test]
fn native_startup_failed_real_release_loader_never_activates() {
    let gate = TestnetPublicationGateV1::for_test();
    let publication = gate.exclusive().unwrap();
    let ctx = context();
    let provider = Freshness::new();
    let mut state = StartupState::Cold;
    assert!(
        activate(&ctx, &provider, &mut state, &archive(10), |verified| ctx
            .install_host(&publication.permit(), verified))
        .is_err()
    );
    assert!(matches!(state, StartupState::Poisoned));
}

#[test]
fn native_startup_panicking_installer_cannot_be_retried() {
    let ctx = context();
    let provider = Freshness::new();
    let mut state: StartupState<u8> = StartupState::Cold;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = activate(&ctx, &provider, &mut state, &archive(10), |_| {
            panic!("partial install")
        });
    }));
    assert!(outcome.is_err());
    assert!(matches!(state, StartupState::Poisoned));
}

#[test]
fn native_startup_provisioning_is_once_and_rejects_inherited_process_identity() {
    let slot = OnceLock::new();
    let publication = TestnetPublicationGateV1::for_test();
    install_context(&slot, &publication, context(), Box::new(Freshness::new())).unwrap();
    assert!(publication.dispatch().is_err());
    assert!(install_context(&slot, &publication, context(), Box::new(Freshness::new())).is_err());
    let provisioned = slot.get().unwrap();
    let session = provisioned.session.lock().unwrap();
    assert!(require_process_owner(provisioned.process_id).is_ok());
    assert!(require_process_owner(provisioned.process_id.wrapping_add(1)).is_err());
    assert!(matches!(session.state, StartupState::Cold));
}

#[test]
fn native_startup_c_contract_bounds_and_unprovisioned_activation() {
    let mut words = [0_u32; 2];
    unsafe {
        assert_eq!(
            connect_norito_kagemusha_testnet_native_startup_contract_v1(words.as_mut_ptr(), 1),
            ERR_BUFFER_TOO_SMALL
        );
        assert_eq!(words, [0, 0]);
        assert_eq!(
            connect_norito_kagemusha_testnet_native_startup_contract_v1(words.as_mut_ptr(), 2),
            2
        );
        assert_eq!(words, [1, 1_048_576]);
        assert_eq!(
            connect_norito_kagemusha_testnet_native_startup_contract_v1(std::ptr::null_mut(), 2),
            ERR_KAGEMUSHA_V1
        );
        assert_eq!(
            connect_norito_kagemusha_testnet_native_startup_activate_v1(std::ptr::null(), 1),
            ERR_KAGEMUSHA_V1
        );
        assert_eq!(
            connect_norito_kagemusha_testnet_native_startup_activate_v1(b"x".as_ptr(), 0),
            ERR_KAGEMUSHA_V1
        );
        assert_eq!(
            connect_norito_kagemusha_testnet_native_startup_activate_v1(b"x".as_ptr(), 1_048_577),
            ERR_KAGEMUSHA_V1
        );
        assert_eq!(
            connect_norito_kagemusha_testnet_native_startup_activate_v1(b"x".as_ptr(), 1),
            ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1
        );
    }
}

#[test]
fn native_startup_partial_owner_or_final_lease_failure_closes_all_dispatch() {
    for ledger_failure in [true, false] {
        let ctx = context();
        let provider = Freshness::new();
        let publication = TestnetPublicationGateV1::for_test();
        let mut state = StartupState::Cold;
        let owner_installed = AtomicBool::new(false);
        let ledger_installed = AtomicBool::new(false);
        let result = {
            let mut publication = publication.exclusive().unwrap();
            publish_activation(&mut publication, &mut state, |state| {
                activate_with_final_check(
                    &ctx,
                    &provider,
                    state,
                    &archive(10),
                    |_| {
                        owner_installed.store(true, Ordering::SeqCst);
                        if ledger_failure {
                            return Err("ledger replay failed".to_owned());
                        }
                        ledger_installed.store(true, Ordering::SeqCst);
                        Ok(7_u8)
                    },
                    |_| Err("final bootstrap lease expired".to_owned()),
                )
            })
        };
        assert!(result.is_err());
        assert!(owner_installed.load(Ordering::SeqCst));
        assert_eq!(ledger_installed.load(Ordering::SeqCst), !ledger_failure);
        assert!(matches!(state, StartupState::Poisoned));
        // This is the exact outer guard used by all three observation/admission exports,
        // the value-credit export, and the Rust retained-host accessor. Neither populated
        // global grants access after failure.
        assert!(publication.dispatch().is_err());
        let mut guard = publication.exclusive().unwrap();
        assert!(publish_activation(&mut guard, &mut state, |_| panic!("retried poison")).is_err());
    }
}

#[test]
fn native_startup_concurrent_dispatch_waits_for_final_publication_or_failure() {
    use std::{sync::mpsc, thread, time::Duration};

    for succeed in [false, true] {
        let publication = TestnetPublicationGateV1::for_test();
        let owner_installed = AtomicBool::new(false);
        let ledger_installed = AtomicBool::new(false);
        let (staged_tx, staged_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let (dispatch_started_tx, dispatch_started_rx) = mpsc::channel();
        let (dispatch_tx, dispatch_rx) = mpsc::channel();
        thread::scope(|threads| {
            let gate = &publication;
            let owner = &owner_installed;
            let ledger = &ledger_installed;
            let install = threads.spawn(move || {
                let ctx = context();
                let provider = Freshness::new();
                let mut state = StartupState::Cold;
                let mut guard = gate.exclusive().unwrap();
                publish_activation(&mut guard, &mut state, |state| {
                    activate_with_final_check(
                        &ctx,
                        &provider,
                        state,
                        &archive(10),
                        |_| {
                            owner.store(true, Ordering::SeqCst);
                            ledger.store(true, Ordering::SeqCst);
                            staged_tx.send(()).unwrap();
                            finish_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                            Ok(7_u8)
                        },
                        |_| {
                            if succeed {
                                Ok(())
                            } else {
                                Err("lease expired".to_owned())
                            }
                        },
                    )
                })
            });
            let staged = staged_rx.recv_timeout(Duration::from_secs(5));
            let dispatch = threads.spawn(|| {
                dispatch_started_tx.send(()).unwrap();
                let result = publication.dispatch().map(|_guard| {
                    assert!(owner_installed.load(Ordering::SeqCst));
                    assert!(ledger_installed.load(Ordering::SeqCst));
                });
                dispatch_tx.send(result.is_ok()).unwrap();
            });
            let dispatch_started = dispatch_started_rx.recv_timeout(Duration::from_secs(5));
            let blocked = dispatch_rx.recv_timeout(Duration::from_millis(50));
            // Always release the installer and join both workers before checking observations.
            let released = finish_tx.send(());
            drop(finish_tx);
            let installed = install.join();
            let completed = dispatch_rx.recv_timeout(Duration::from_secs(5));
            let dispatched = dispatch.join();
            assert_eq!(staged, Ok(()));
            assert_eq!(dispatch_started, Ok(()));
            assert_eq!(blocked, Err(mpsc::RecvTimeoutError::Timeout));
            released.unwrap();
            assert_eq!(installed.unwrap().is_ok(), succeed);
            assert_eq!(completed, Ok(succeed));
            dispatched.unwrap();
        });
    }
}

#[test]
fn native_startup_publication_rejects_panics_but_preserves_active_invalid_retry() {
    let publication = TestnetPublicationGateV1::for_test();
    let ctx = context();
    let provider = Freshness::new();
    let mut state = StartupState::Cold;
    {
        let mut guard = publication.exclusive().unwrap();
        publish_activation(&mut guard, &mut state, |state| {
            activate(&ctx, &provider, state, &archive(10), |_| Ok(7_u8))
        })
        .unwrap();
        assert!(
            publish_activation(&mut guard, &mut state, |state| {
                activate(&ctx, &provider, state, b"invalid retry", |_| {
                    panic!("reinstalled")
                })
            })
            .is_err()
        );
    }
    assert!(publication.dispatch().is_ok());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut guard = publication.exclusive().unwrap();
        let mut state: StartupState<u8> = StartupState::Cold;
        let _ = publish_activation(&mut guard, &mut state, |state| {
            activate(&ctx, &provider, state, &archive(10), |_| {
                panic!("partial install")
            })
        });
    }));
    assert!(result.is_err());
    assert!(publication.dispatch().is_err());
    assert!(publication.exclusive().is_err());
}
