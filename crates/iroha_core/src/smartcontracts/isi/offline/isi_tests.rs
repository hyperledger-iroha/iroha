#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        role::RoleIdWithOwner,
        state::{State, World},
    };
    use core::num::NonZeroU64;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
    use iroha_data_model::{
        ChainId, NetworkId, Registrable,
        account::Account,
        asset::{Asset, AssetDefinition, AssetDefinitionId},
        block::BlockHeader,
        domain::{Domain, DomainId},
        isi::{
            SetAssetHoldingLimit,
            error::{AssetTransferAdmissionError, InstructionExecutionError},
            offline::{AuthorizeKagemushaTairaCanaryV4, RecordKagemushaTairaCanaryV4},
        },
        offline::{
            KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_BODY_SCHEMA,
            KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_SCHEMA,
            KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_BODY_SCHEMA,
            KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_SCHEMA,
            KagemushaAndroidKeyMintHardwareAssertionV1, KagemushaDevicePublicKeyV2,
            KagemushaDeviceSignatureV2, KagemushaExactBytesDigestV1,
            KagemushaIosAppAttestHardwareAssertionV1, KagemushaV4PromotionBindingV1,
            KagemushaV4TairaCanaryAuthorizationBodyV1, KagemushaV4TairaCanaryPermitV1,
            KagemushaV4TairaCanaryReservationBodyV1, KagemushaV4TairaCanaryReservationV1,
            kagemusha_v4_taira_canary_transaction_metadata,
        },
        permission::Permission,
        role::{Role, RoleId},
        transaction::{Executable, ExecutableBatchItem, FeePaymentIntent, TransactionBuilder},
    };
    use iroha_primitives::{json::Json, numeric::Quantity};
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
    use p256::{
        ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _},
        elliptic_curve::sec1::ToEncodedPoint as _,
    };
    const POLICY_TEST_TIME_MS: u64 = 1_800_000_000_000;
    fn android_status_snapshot() -> OfflineAndroidAttestationStatusSnapshotV1 {
        OfflineAndroidAttestationStatusSnapshotV1 {
            version:
                iroha_data_model::offline::OFFLINE_ANDROID_ATTESTATION_STATUS_SNAPSHOT_VERSION_V1,
            payload_sha256: [0x99; 32],
            response_date_ms: POLICY_TEST_TIME_MS,
            last_modified_ms: Some(POLICY_TEST_TIME_MS),
            cache_max_age_seconds: 86_400,
            non_valid_serials: Vec::new(),
        }
    }
    macro_rules! offline_test_transaction {
        ($transaction:ident) => {
            let state = offline_test_state();
            let mut block = state.block(offline_test_header());
            let mut $transaction = block.transaction();
        };
    }
    fn test_network_id(seed: impl AsRef<[u8]>) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            seed,
        )))
    }
    #[test]
    fn redemption_context_rejects_same_label_foreign_genesis() {
        let first_display_label = ChainId::from("shared-offline-display-label");
        let second_display_label = ChainId::from("shared-offline-display-label");
        assert_eq!(first_display_label, second_display_label);
        let live_network = test_network_id(b"shared-label-live-offline-genesis");
        let foreign_network = test_network_id(b"shared-label-foreign-offline-genesis");
        assert_ne!(live_network, foreign_network);
        ensure_kagemusha_v4_redemption_live_context(
            &live_network,
            &foreign_network,
            &live_network,
            2,
            2,
            2,
        )
        .expect_err("a same-label foreign genesis must fail the exact-network gate");
        ensure_kagemusha_v4_redemption_live_context(
            &live_network,
            &live_network,
            &live_network,
            2,
            2,
            2,
        )
        .expect("the exact live NetworkId must pass the domain gate");
    }
    #[test]
    fn anchor_drawdown_is_canonical_bounded_and_cross_anchor() {
        let balances = [
            KagemushaV4AnchorDrawdownBalance {
                operation_id: [0x11; 32],
                capacity_atomic_units: 100,
                redeemed_atomic_units: 20,
            },
            KagemushaV4AnchorDrawdownBalance {
                operation_id: [0x22; 32],
                capacity_atomic_units: 50,
                redeemed_atomic_units: 10,
            },
        ];
        assert_eq!(
            allocate_kagemusha_v4_anchor_drawdown(&balances, 110),
            Some(vec![([0x11; 32], 100), ([0x22; 32], 40)])
        );
        assert_eq!(
            allocate_kagemusha_v4_anchor_drawdown(&balances, 120),
            Some(vec![([0x11; 32], 100), ([0x22; 32], 50)])
        );
        assert!(
            allocate_kagemusha_v4_anchor_drawdown(&balances, 121).is_none(),
            "redemption must not exceed aggregate unredeemed provenance"
        );
        assert!(
            allocate_kagemusha_v4_anchor_drawdown(&balances, 0).is_none(),
            "zero-value drawdown must not produce a state update"
        );
        let corrupt = [KagemushaV4AnchorDrawdownBalance {
            operation_id: [0x33; 32],
            capacity_atomic_units: 1,
            redeemed_atomic_units: 2,
        }];
        assert!(
            allocate_kagemusha_v4_anchor_drawdown(&corrupt, 1).is_none(),
            "a persisted drawdown above its anchor must fail closed"
        );
        let duplicate = [
            KagemushaV4AnchorDrawdownBalance {
                operation_id: [0x44; 32],
                capacity_atomic_units: 100,
                redeemed_atomic_units: 0,
            },
            KagemushaV4AnchorDrawdownBalance {
                operation_id: [0x44; 32],
                capacity_atomic_units: 100,
                redeemed_atomic_units: 0,
            },
        ];
        assert!(
            allocate_kagemusha_v4_anchor_drawdown(&duplicate, 101).is_none(),
            "duplicate anchor identities must fail closed before allocation"
        );
    }
    #[test]
    fn anchor_drawdown_state_is_paired_sequential_and_rollback_safe() {
        let operation_id = [0x45; 32];
        offline_test_transaction!(transaction);
        let asset = offline_test_asset(&ALICE_ID);
        let amount = iroha_data_model::offline::KagemushaScaledAmountV2::new(100, 0)
            .expect("positive anchor amount");
        let note = iroha_data_model::offline::KagemushaSpendableNoteDescriptorV2 {
            network_id: *transaction.network_id(),
            asset: asset.definition().clone(),
            note_commitment: [0x48; 32],
            spend_nullifier: [0x49; 32],
            amount,
        };
        let anchor = KagemushaRecursiveSpendTopUpAnchorV4 {
            version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4,
            network_id: *transaction.network_id(),
            payer: ALICE_ID.clone(),
            asset,
            asset_scale: 0,
            amount,
            initial_root: [0x4A; 32],
            finalized_root: [0x4B; 32],
            shield_leaf_index: 0,
            current_note: note,
            topup_operation_id: operation_id,
            shield_verifier_id: iroha_data_model::proof::VerifyingKeyId::new(
                "halo2/ipa",
                "drawdown-state-test",
            ),
            shield_verifier_commitment: [0x4C; 32],
            artifact_binding: iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4 {
                version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
                generation: "drawdown-state-test".to_owned(),
                manifest_sha256: [0x4D; 32],
            },
            finalized_height: transaction.block_height(),
            finalized_tx_hash: [0x4E; 32],
            anchor_digest: [0; 32],
        }
        .finalize_digest()
        .expect("canonical top-up anchor");
        let anchor_ref = anchor.compact_ref().expect("canonical anchor reference");
        let anchor_archive = norito::encode_canonical(&anchor).expect("canonical anchor bytes");
        persist_kagemusha_v4_topup_anchor(&anchor, &mut transaction)
            .expect("paired canonical anchor/drawdown initialization");
        let anchor_key =
            kagemusha_v4_topup_anchor_state_key(operation_id).expect("anchor state key");
        let drawdown_key =
            kagemusha_v4_topup_drawdown_state_key(operation_id).expect("drawdown state key");
        assert_eq!(
            transaction.world.smart_contract_state.get(&anchor_key),
            Some(&anchor_archive),
        );
        assert_eq!(
            transaction.world.smart_contract_state.get(&drawdown_key),
            Some(&0_u128.to_le_bytes().to_vec()),
            "paired initialization must persist an exact zero u128",
        );
        assert_eq!(
            load_kagemusha_v4_topup_anchor(operation_id, &transaction)
                .expect("persisted canonical anchor"),
            anchor,
        );
        let first =
            plan_kagemusha_v4_anchor_drawdown(core::slice::from_ref(&anchor_ref), 40, &transaction)
                .expect("first drawdown plan");
        commit_kagemusha_v4_anchor_drawdown(first, &mut transaction);
        assert_eq!(
            load_kagemusha_v4_topup_drawdown(operation_id, &transaction)
                .expect("first persisted drawdown"),
            40,
        );
        let second =
            plan_kagemusha_v4_anchor_drawdown(core::slice::from_ref(&anchor_ref), 60, &transaction)
                .expect("second drawdown plan");
        commit_kagemusha_v4_anchor_drawdown(second, &mut transaction);
        assert_eq!(
            load_kagemusha_v4_topup_drawdown(operation_id, &transaction)
                .expect("cumulative persisted drawdown"),
            100,
        );
        let assets_before = offline_asset_entries(&transaction);
        let confidential_before = transaction
            .world
            .zk_assets
            .iter()
            .map(|(id, state)| {
                (
                    id.clone(),
                    state.tree_profile,
                    state.commitments.clone(),
                    state.root_history.clone(),
                    state.nullifiers.clone(),
                )
            })
            .collect::<Vec<_>>();
        let branch_and_replay_before = transaction
            .world
            .kagemusha_replay_keys
            .iter()
            .map(|(key, ())| *key)
            .collect::<Vec<_>>();
        let receipt_key =
            kagemusha_v4_redemption_receipt_state_key([0x47; 32]).expect("receipt key");
        let receipt_before = transaction
            .world
            .smart_contract_state
            .get(&receipt_key)
            .cloned();
        let events_before = transaction.world.internal_event_buf.len();
        let overdraw =
            plan_kagemusha_v4_anchor_drawdown(core::slice::from_ref(&anchor_ref), 1, &transaction)
                .expect_err("one unit beyond cumulative capacity must fail");
        assert!(overdraw.to_string().contains("topup_drawdown_exhausted"));
        assert_eq!(offline_asset_entries(&transaction), assets_before);
        assert_eq!(
            transaction
                .world
                .zk_assets
                .iter()
                .map(|(id, state)| {
                    (
                        id.clone(),
                        state.tree_profile,
                        state.commitments.clone(),
                        state.root_history.clone(),
                        state.nullifiers.clone(),
                    )
                })
                .collect::<Vec<_>>(),
            confidential_before,
            "rejected drawdown must not change a nullifier or tree",
        );
        assert_eq!(
            transaction
                .world
                .kagemusha_replay_keys
                .iter()
                .map(|(key, ())| *key)
                .collect::<Vec<_>>(),
            branch_and_replay_before,
            "rejected drawdown must not consume branch/replay markers",
        );
        assert_eq!(
            transaction
                .world
                .smart_contract_state
                .get(&receipt_key)
                .cloned(),
            receipt_before,
            "rejected drawdown must not create a receipt",
        );
        assert_eq!(
            load_kagemusha_v4_topup_drawdown(operation_id, &transaction)
                .expect("unchanged exhausted drawdown"),
            100,
        );
        assert_eq!(transaction.world.internal_event_buf.len(), events_before);
        transaction
            .world
            .smart_contract_state
            .remove(drawdown_key.clone());
        assert!(
            plan_kagemusha_v4_anchor_drawdown(core::slice::from_ref(&anchor_ref), 1, &transaction,)
                .expect_err("orphan anchor must fail closed")
                .to_string()
                .contains("topup_drawdown_missing"),
        );
        transaction
            .world
            .smart_contract_state
            .insert(drawdown_key, vec![0; 15]);
        assert!(
            plan_kagemusha_v4_anchor_drawdown(core::slice::from_ref(&anchor_ref), 1, &transaction,)
                .expect_err("malformed drawdown must fail closed")
                .to_string()
                .contains("topup_drawdown_invalid"),
        );
    }
    #[test]
    fn offline_proof_boundary_rejects_alternate_norito_layout() {
        let envelope = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: "halo2/pasta/ipa/offline-boundary-v1".to_owned(),
            vk_hash: [7_u8; 32],
            public_inputs: b"offline-boundary-v1".to_vec(),
            proof_bytes: vec![1, 2, 3],
            aux: Vec::new(),
        };
        let canonical =
            norito::encode_canonical(&envelope).expect("canonical offline proof envelope");
        assert_eq!(
            decode_canonical_offline_proof_envelope(&canonical, "fixture")
                .expect("canonical envelope must decode"),
            envelope
        );
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&envelope).expect("alternate-layout offline proof envelope")
        };
        assert_ne!(alternate, canonical);
        let error = decode_canonical_offline_proof_envelope(
            &alternate,
            "alternate offline envelope rejected",
        )
        .expect_err("alternate-layout envelope must fail closed");
        assert!(
            error
                .to_string()
                .contains("alternate offline envelope rejected"),
            "unexpected boundary error: {error}"
        );
    }
    #[test]
    fn kagemusha_v4_chain_state_namespaces_are_version_distinct() {
        let operation_id = [0x41; 32];
        assert_ne!(
            kagemusha_v2_marker(KAGEMUSHA_V4_OPERATION_DOMAIN, &[&operation_id]),
            kagemusha_v2_marker("kagemusha-v2-operation", &[&operation_id]),
        );
        assert_ne!(
            kagemusha_v2_marker(KAGEMUSHA_V4_BRANCH_EXACT_DOMAIN, &[&operation_id]),
            kagemusha_v2_marker("kagemusha-v2-redeemed-branch", &[&operation_id]),
        );
        assert!(
            kagemusha_v4_topup_anchor_state_key(operation_id)
                .expect("valid V4 anchor key")
                .to_string()
                .starts_with("kagemusha_v4_topup_anchor_")
        );
        assert!(
            kagemusha_v4_redemption_receipt_state_key(operation_id)
                .expect("valid V4 redemption receipt key")
                .to_string()
                .starts_with("kagemusha_v4_redemption_")
        );
    }
    #[test]
    fn kagemusha_topup_note_freshness_rejects_all_state_namespace_collisions() {
        const EXISTING_COMMITMENT: [u8; 32] = [0x51; 32];
        const SPENT_NULLIFIER: [u8; 32] = [0x52; 32];
        const FRESH_COMMITMENT: [u8; 32] = [0x53; 32];
        const FRESH_NULLIFIER: [u8; 32] = [0x54; 32];
        let mut zk_state = crate::state::ZkAssetState::default();
        zk_state.commitments.push(EXISTING_COMMITMENT);
        assert!(zk_state.nullifiers.insert(SPENT_NULLIFIER));
        ensure_kagemusha_v4_topup_note_is_fresh(&zk_state, FRESH_COMMITMENT, FRESH_NULLIFIER)
            .expect("disjoint top-up note material must remain admissible");
        for (note_commitment, spend_nullifier, expected_label) in [
            (EXISTING_COMMITMENT, FRESH_NULLIFIER, "duplicate_output"),
            (FRESH_COMMITMENT, SPENT_NULLIFIER, "duplicate_nullifier"),
            (FRESH_COMMITMENT, EXISTING_COMMITMENT, "duplicate_nullifier"),
            (SPENT_NULLIFIER, FRESH_NULLIFIER, "proof_binding"),
        ] {
            let error = ensure_kagemusha_v4_topup_note_is_fresh(
                &zk_state,
                note_commitment,
                spend_nullifier,
            )
            .expect_err("every commitment/nullifier namespace collision must fail closed");
            assert!(
                error.to_string().contains(&format!(
                    "{OFFLINE_REJECTION_REASON_PREFIX}{expected_label}:"
                )),
                "unexpected collision rejection for {expected_label}: {error}"
            );
        }
    }
    #[test]
    fn kagemusha_topup_capacity_reserves_the_complete_recursive_lifecycle() {
        let tree_capacity =
            usize::try_from(iroha_data_model::offline::KAGEMUSHA_TOPUP_SHIELD_TREE_CAPACITY_V2)
                .expect("the fixed depth-16 tree capacity fits usize");
        let insertion_capacity = usize::try_from(
            iroha_data_model::offline::KAGEMUSHA_TOPUP_SHIELD_INSERTION_CAPACITY_V2,
        )
        .expect("the fixed depth-16 insertion capacity fits usize");
        assert_eq!(
            kagemusha_v4_topup_leaf_index(0, tree_capacity).expect("empty tree accepts a top-up"),
            0
        );
        assert_eq!(
            kagemusha_v4_topup_leaf_index(insertion_capacity - 1, tree_capacity)
                .expect("the last insertion retains the complete recursive lifecycle"),
            u32::try_from(insertion_capacity - 1).expect("leaf index fits u32")
        );
        for (commitment_count, capacity) in [
            (0, 0),
            (0, 72),
            (0, 73),
            (insertion_capacity, tree_capacity),
            (tree_capacity, tree_capacity),
        ] {
            let error = kagemusha_v4_topup_leaf_index(commitment_count, capacity)
                .expect_err("a top-up without a complete future lifecycle must fail closed");
            assert!(
                error.to_string().contains(&format!(
                    "{OFFLINE_REJECTION_REASON_PREFIX}topup_tree_full:"
                )),
                "unexpected capacity rejection: {error}"
            );
        }
    }
    #[test]
    fn kagemusha_redemption_state_freshness_rejects_all_namespace_collisions() {
        const EXISTING_COMMITMENT: [u8; 32] = [0x61; 32];
        const SPENT_NULLIFIER: [u8; 32] = [0x62; 32];
        const CURRENT_NULLIFIER: [u8; 32] = [0x63; 32];
        const CHANGE_COMMITMENT: [u8; 32] = [0x64; 32];
        const CHANGE_NULLIFIER: [u8; 32] = [0x65; 32];
        let mut zk_state = crate::state::ZkAssetState::default();
        zk_state.commitments.push(EXISTING_COMMITMENT);
        assert!(zk_state.nullifiers.insert(SPENT_NULLIFIER));
        ensure_kagemusha_v4_redemption_state_is_fresh(
            &zk_state,
            CURRENT_NULLIFIER,
            Some((CHANGE_COMMITMENT, CHANGE_NULLIFIER)),
        )
        .expect("disjoint redemption material must remain admissible");
        for (current_nullifier, change_note, expected_label) in [
            (SPENT_NULLIFIER, None, "duplicate_nullifier"),
            (EXISTING_COMMITMENT, None, "proof_binding"),
            (
                CURRENT_NULLIFIER,
                Some((EXISTING_COMMITMENT, CHANGE_NULLIFIER)),
                "duplicate_output",
            ),
            (
                CURRENT_NULLIFIER,
                Some((CHANGE_COMMITMENT, SPENT_NULLIFIER)),
                "duplicate_nullifier",
            ),
            (
                CURRENT_NULLIFIER,
                Some((CHANGE_COMMITMENT, CURRENT_NULLIFIER)),
                "duplicate_nullifier",
            ),
            (
                CURRENT_NULLIFIER,
                Some((CURRENT_NULLIFIER, CHANGE_NULLIFIER)),
                "proof_binding",
            ),
            (
                CURRENT_NULLIFIER,
                Some((SPENT_NULLIFIER, CHANGE_NULLIFIER)),
                "proof_binding",
            ),
            (
                CURRENT_NULLIFIER,
                Some((CHANGE_COMMITMENT, EXISTING_COMMITMENT)),
                "proof_binding",
            ),
        ] {
            let error = ensure_kagemusha_v4_redemption_state_is_fresh(
                &zk_state,
                current_nullifier,
                change_note,
            )
            .expect_err("every redemption namespace collision must fail closed");
            assert!(
                error.to_string().contains(&format!(
                    "{OFFLINE_REJECTION_REASON_PREFIX}{expected_label}:"
                )),
                "unexpected collision rejection for {expected_label}: {error}"
            );
        }
    }
    #[test]
    fn kagemusha_redemption_checks_state_freshness_before_proof_work() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/smartcontracts/isi/offline.rs"
        ));
        let redemption = source
            .split_once("impl Execute for RedeemKagemushaRecursiveV4")
            .map(|(_, redemption)| redemption)
            .expect("Kagemusha V4 redemption executor");
        let freshness = redemption
            .find("ensure_kagemusha_v4_redemption_state_is_fresh(")
            .expect("early confidential-state freshness check");
        let proof_accounting = redemption
            .find("state_transaction.register_confidential_proof(")
            .expect("recursive proof accounting");
        let recursive_verification = redemption
            .find("verify_kagemusha_v4_recursive_bundle(")
            .expect("recursive proof verification");
        assert!(freshness < proof_accounting);
        assert!(freshness < recursive_verification);
    }
    #[test]
    fn kagemusha_redemption_state_delta_commits_in_place() {
        const CURRENT_NULLIFIER: [u8; 32] = [0x71; 32];
        const CHANGE_COMMITMENT: [u8; 32] = [0x72; 32];
        const CHANGE_NULLIFIER: [u8; 32] = [0x73; 32];
        let (state, definition_id, _, _) = offline_holding_limit_test_state(None);
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
        let verifier_binding = crate::state::ZkAssetVerifierBinding {
            id: iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "post-policy-unshield"),
            commitment: [0x74; 32],
        };
        let mut zk_asset_state = crate::state::ZkAssetState::default();
        zk_asset_state.vk_unshield = Some(verifier_binding.clone());
        let initial_root = zk_asset_state.persisted_root;
        let expected_root = zk_asset_state
            .preview_commitment_root(CHANGE_COMMITMENT)
            .expect("preview change commitment");
        state_transaction
            .world
            .zk_assets
            .insert(definition_id.clone(), zk_asset_state);
        KagemushaV4ZkAssetCommitPlan {
            definition_id: definition_id.clone(),
            expected_root: initial_root,
            expected_commitment_count: 0,
            current_nullifier: CURRENT_NULLIFIER,
            change: Some(KagemushaV4ChangeStateCommit {
                note_commitment: CHANGE_COMMITMENT,
                spend_nullifier: CHANGE_NULLIFIER,
                expected_root,
            }),
        }
        .commit(&mut state_transaction)
        .expect("commit checked redemption state delta");
        let committed = state_transaction
            .world
            .zk_assets
            .get(&definition_id)
            .expect("committed shielded state");
        assert!(committed.nullifiers.contains(&CURRENT_NULLIFIER));
        assert_eq!(committed.commitments, vec![CHANGE_COMMITMENT]);
        assert_eq!(committed.persisted_root, expected_root);
        assert_eq!(committed.vk_unshield.as_ref(), Some(&verifier_binding));
    }
    #[test]
    fn kagemusha_v4_admission_authenticates_exact_release_without_global_backend_flag() {
        let source = include_str!("../offline.rs");
        let topup_start = source
            .find("impl Execute for TopUpKagemushaRecursiveV4")
            .expect("V4 top-up executor");
        let redeem_start = source
            .find("impl Execute for RedeemKagemushaRecursiveV4")
            .expect("V4 redemption executor");
        let tests_start = redeem_start
            + source[redeem_start..]
                .find("#[cfg(test)]")
                .expect("offline executor test module");
        let topup = &source[topup_start..redeem_start];
        let redeem = &source[redeem_start..tests_start];
        for (name, executor) in [("top-up", topup), ("redemption", redeem)] {
            assert!(
                !executor.contains("KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE"),
                "V4 {name} must authenticate a concrete release instead of treating compile capability as runtime readiness",
            );
            assert!(
                executor.contains("resolve_kagemusha_v4_transaction_release"),
                "V4 {name} must resolve the transaction-selected authenticated release",
            );
        }
        assert!(
            redeem.contains("verify_kagemusha_v4_recursive_bundle"),
            "full and partial redemption must verify the parent recursive bundle",
        );
        assert!(
            redeem.contains("verify_bundle_operation_v4"),
            "partial redemption must separately verify its operation-bound change bundle",
        );
    }
    #[test]
    fn kagemusha_v4_execute_replay_boundary_is_auth_before_committed_state() {
        let source = include_str!("../offline.rs");
        let assert_ordered = |label: &str, body: &str, needles: &[&str]| {
            let mut cursor = 0;
            for needle in needles {
                let offset = body[cursor..].find(needle).unwrap_or_else(|| {
                    panic!("{label} is missing required boundary step `{needle}`")
                });
                cursor += offset + needle.len();
            }
        };
        let topup_helper_start = source
            .find("fn authenticate_kagemusha_v4_topup_submission_before_replay")
            .expect("top-up pre-replay boundary");
        let redeem_helper_start = source
            .find("fn authenticate_kagemusha_v4_redeem_submission_before_replay")
            .expect("redemption pre-replay boundary");
        let helper_end = source[redeem_helper_start..]
            .find("fn validate_offline_attestation_recent_block")
            .map(|offset| redeem_helper_start + offset)
            .expect("end of redemption pre-replay boundary");
        assert_ordered(
            "top-up pre-replay boundary",
            &source[topup_helper_start..redeem_helper_start],
            &[
                "ensure_can_submit_kagemusha_topup",
                "authenticate_registered_kagemusha_v2_device",
                "kagemusha_v4_replay_status",
            ],
        );
        assert_ordered(
            "redemption pre-replay boundary",
            &source[redeem_helper_start..helper_end],
            &[
                "ensure_can_submit_kagemusha_for_account",
                "authenticate_registered_kagemusha_v2_device_against_policy",
                "kagemusha_v4_replay_status",
            ],
        );
        let topup_execute_start = source
            .find("impl Execute for TopUpKagemushaRecursiveV4")
            .expect("top-up executor");
        let redeem_execute_start = source
            .find("impl Execute for RedeemKagemushaRecursiveV4")
            .expect("redemption executor");
        let tests_start = redeem_execute_start
            + source[redeem_execute_start..]
                .find("#[cfg(test)]")
                .expect("offline executor test module");
        assert_ordered(
            "top-up executor",
            &source[topup_execute_start..redeem_execute_start],
            &[
                "validate_authorization_at",
                "authenticate_kagemusha_v4_topup_submission_before_replay",
                "match replay_status",
                "load_kagemusha_v4_topup_anchor",
            ],
        );
        assert_ordered(
            "redemption executor",
            &source[redeem_execute_start..tests_start],
            &[
                "validate_authorization_at",
                "kagemusha_release_lifecycle::redemption_policy",
                "authenticate_kagemusha_v4_redeem_submission_before_replay",
                "match replay_status",
                "ensure_kagemusha_v4_redemption_receipt_matches",
            ],
        );
    }
    #[test]
    fn kagemusha_v4_activation_overlap_inventory_is_consensus_derived() {
        let source = include_str!("../offline.rs");
        let start = source
            .find("fn ensure_kagemusha_v4_non_overlapping_issuance")
            .expect("V4 issuance-overlap validator");
        let end = start
            + source[start..]
                .find("impl Execute for TopUpKagemushaRecursiveV4")
                .expect("next V4 instruction executor");
        let validator = &source[start..end];
        assert!(validator.contains("world.smart_contract_state.iter()"));
        assert!(validator.contains("decode_kagemusha_v4_consensus_release_state"));
        assert!(validator.contains("cached.release_record() != &release_record"));
        assert!(
            !validator.contains("kagemusha_release_catalog.iter()"),
            "release-window inventory must not depend on optional local directories",
        );
    }
    #[test]
    fn kagemusha_v4_activation_validates_identity_and_policy_before_state_mutation() {
        let source = include_str!("kagemusha_activation.rs");
        let start = source
            .find("impl Execute for ActivateKagemushaRecursiveReleaseV4")
            .expect("V4 release activation executor");
        let body = &source[start..];
        let promotion_consumption_plan = body
            .find("plan_kagemusha_v4_activation_binding(")
            .expect("activation promotion-binding validation and replay check");
        let validation = body
            .find("validate_offline_attestation_policy_for_release_activation")
            .expect("activation policy validation");
        let transition_validation = body
            .find("validate_offline_attestation_policy_transition_from_state")
            .expect("activation anti-rollback policy transition validation");
        let first_mutation = body
            .find("state_transaction.world.smart_contract_state.insert")
            .expect("activation state publication");
        let promotion_consumption_commit = body
            .find("commit_v4_promotion_binding")
            .expect("atomic activation promotion-binding consumption");

        assert!(
            validation < transition_validation
                && transition_validation < promotion_consumption_plan
                && promotion_consumption_plan < first_mutation
                && first_mutation < promotion_consumption_commit,
            "bounded policy, anti-rollback transition, promotion identity, and replay must be rejected before atomic state publication",
        );
    }
    #[test]
    fn kagemusha_v4_promotion_id_consumption_accepts_fresh_and_rejects_duplicate() {
        offline_test_transaction!(transaction);
        let promotion_id = [0xA5; 32];
        let replay_keys_before = transaction.world.kagemusha_replay_keys.iter().count();
        let marker = plan_v4_promotion_id(promotion_id, &transaction)
            .expect("fresh promotion id must admit one activation");
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            replay_keys_before,
            "planning consumption must not mutate consensus state",
        );
        commit_v4_promotion_id(marker, &mut transaction);
        assert!(
            transaction
                .world
                .kagemusha_replay_keys
                .get(&marker)
                .is_some(),
            "successful activation must persist its promotion marker",
        );
        let replay_keys_after_first = transaction.world.kagemusha_replay_keys.iter().count();

        let error = plan_v4_promotion_id(promotion_id, &transaction)
            .expect_err("a second activation must not reuse the promotion id");
        assert!(error.to_string().contains("promotion_replay"));
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            replay_keys_after_first,
            "duplicate rejection must not mutate consensus state",
        );

        let distinct_marker = plan_v4_promotion_id([0xA6; 32], &transaction)
            .expect("a distinct promotion id remains eligible");
        assert_ne!(distinct_marker, marker);
        assert!(plan_v4_promotion_id([0; 32], &transaction).is_err());
    }
    #[derive(Clone)]
    struct CanaryConsensusFixture {
        permit: KagemushaV4TairaCanaryPermitV1,
        reservation: KagemushaV4TairaCanaryReservationV1,
        exact_call_hash: Hash,
        canary_transaction: SignedTransaction,
        canary_transaction_wire: Vec<u8>,
    }
    fn canary_consensus_controller() -> KeyPair {
        KeyPair::from_seed(vec![0xC4; 32], Algorithm::Ed25519)
    }
    fn canary_consensus_digest(label: &[u8]) -> KagemushaExactBytesDigestV1 {
        KagemushaExactBytesDigestV1::from_bytes(label).expect("non-empty canary test identity")
    }
    fn canary_consensus_fixture(
        state_transaction: &StateTransaction<'_, '_>,
        nonce: u32,
    ) -> CanaryConsensusFixture {
        canary_consensus_fixture_for_key(state_transaction, nonce, &ALICE_KEYPAIR)
    }
    fn canary_consensus_fixture_for_key(
        state_transaction: &StateTransaction<'_, '_>,
        nonce: u32,
        canary_key: &KeyPair,
    ) -> CanaryConsensusFixture {
        let controller = canary_consensus_controller();
        let canary_authority = AccountId::new(canary_key.public_key().clone());
        let binding = KagemushaV4PromotionBindingV1 {
            promotion_controller: controller.public_key().clone(),
            promotion_reservation: canary_consensus_digest(b"canary reservation"),
            promotion_id: [0xC5; 32],
            network_id: state_transaction.network_id().clone(),
            reviewed_source_closure_descriptor_sha256: [0xC6; 32],
            manifest_sha256: [0xC7; 32],
            release_record_sha256: [0xC8; 32],
            release_policy_source: canary_consensus_digest(b"canary release policy"),
            device_attestation_policy_norito: canary_consensus_digest(b"canary device policy"),
            signed_genesis: canary_consensus_digest(b"canary signed genesis"),
            catalog_consensus_policy_digest: [0xC9; 32],
            execution_policy_hash: Hash::new(b"canary execution policy"),
        };
        let expires_at_height = NonZeroU64::new(3).expect("non-zero canary expiry");
        let body = KagemushaV4TairaCanaryAuthorizationBodyV1 {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            binding,
            activation_expectations_artifact: canary_consensus_digest(b"canary expectations"),
            activation_finality_receipt: canary_consensus_digest(b"canary receipt"),
            canary_authority: canary_authority.clone(),
            canonical_torii_origin: "https://taira.example".to_owned(),
            authorized_at_unix_ms: POLICY_TEST_TIME_MS - 1_000,
            expires_at_unix_ms: POLICY_TEST_TIME_MS + 60_000,
            expires_at_height,
        };
        let permit_signature =
            SignatureOf::try_from_hash(controller.private_key(), body.signing_hash())
                .expect("controller signs canary permit");
        let permit = KagemushaV4TairaCanaryPermitV1 {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            body,
            signature: permit_signature,
        };
        let metadata = kagemusha_v4_taira_canary_transaction_metadata(
            permit.body.binding.promotion_id,
            permit.body.activation_finality_receipt,
            &permit.body.canonical_torii_origin,
            expires_at_height,
        );
        let mut builder = TransactionBuilder::new(
            state_transaction.network_id().clone(),
            canary_authority,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([RecordKagemushaTairaCanaryV4::new(permit.clone())])
        .with_metadata(metadata);
        builder.set_creation_time(core::time::Duration::from_millis(POLICY_TEST_TIME_MS));
        builder.set_ttl(core::time::Duration::from_millis(30_000));
        builder.set_nonce(
            core::num::NonZeroU32::new(nonce).expect("non-zero canary transaction nonce"),
        );
        let canary_transaction = builder.sign(canary_key.private_key());
        let canary_transaction_wire = canary_transaction
            .encode_wire_v1()
            .expect("canonical canary transaction wire");
        let reservation_body = KagemushaV4TairaCanaryReservationBodyV1 {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            permit: permit.clone(),
            canary_transaction_wire: KagemushaExactBytesDigestV1::from_bytes(
                &canary_transaction_wire,
            )
            .expect("exact canary wire identity"),
            canary_transaction_intent: canary_transaction.hash(),
            canary_entrypoint_hash: Hash::from(canary_transaction.hash_as_entrypoint()),
        };
        let reservation_signature =
            SignatureOf::try_from_hash(controller.private_key(), reservation_body.signing_hash())
                .expect("controller signs exact-hash canary reservation");
        let reservation = KagemushaV4TairaCanaryReservationV1 {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            body: reservation_body,
            signature: reservation_signature,
        };
        CanaryConsensusFixture {
            permit,
            exact_call_hash: Hash::from(canary_transaction.hash_as_entrypoint()),
            reservation,
            canary_transaction,
            canary_transaction_wire,
        }
    }
    fn bind_canary_consensus_wire(
        fixture: &CanaryConsensusFixture,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) {
        let identity =
            crate::smartcontracts::isi::offline::signed_kagemusha_taira_canary_wire_identity_v1(
                &fixture.canary_transaction,
            )
            .expect("derive canary signed-wire binding")
            .expect("single direct canary record has a signed-wire binding");
        assert_eq!(identity, fixture.reservation.body.canary_transaction_wire);
        state_transaction.kagemusha_taira_canary_external_entrypoint = true;
        state_transaction.kagemusha_taira_canary_wire_identity = Some(identity);
    }
    fn commit_canary_activation_binding(
        binding: &KagemushaV4PromotionBindingV1,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) {
        let (promotion, binding) = plan_v4_promotion_binding(binding, state_transaction)
            .expect("fresh canary activation binding");
        commit_v4_promotion_binding(promotion, binding, state_transaction);
    }
    #[test]
    fn taira_canary_requires_activation_before_exact_authorization() {
        offline_test_transaction!(transaction);
        let fixture = canary_consensus_fixture(&transaction, 7);
        let replay_keys_before = transaction.world.kagemusha_replay_keys.iter().count();
        let error = AuthorizeKagemushaTairaCanaryV4::new(fixture.reservation)
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("authorization without activation must fail closed");
        assert!(error.to_string().contains("canary_activation_missing"));
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            replay_keys_before,
            "failed authorization must not reserve any marker",
        );
    }
    #[test]
    fn taira_canary_rejects_fake_controller_for_real_promotion_id() {
        offline_test_transaction!(transaction);
        let fixture = canary_consensus_fixture(&transaction, 7);
        commit_canary_activation_binding(&fixture.permit.body.binding, &mut transaction);
        let markers_after_activation = transaction.world.kagemusha_replay_keys.iter().count();
        let fake_controller = KeyPair::from_seed(vec![0xDA; 32], Algorithm::Ed25519);
        let mut fake_permit = fixture.permit;
        fake_permit.body.binding.promotion_controller = fake_controller.public_key().clone();
        fake_permit.signature = SignatureOf::try_from_hash(
            fake_controller.private_key(),
            fake_permit.body.signing_hash(),
        )
        .expect("attacker self-signs a structurally valid permit");
        let mut fake_reservation = fixture.reservation;
        fake_reservation.body.permit = fake_permit;
        fake_reservation.signature = SignatureOf::try_from_hash(
            fake_controller.private_key(),
            fake_reservation.body.signing_hash(),
        )
        .expect("attacker self-signs a structurally valid reservation");
        let error = AuthorizeKagemushaTairaCanaryV4::new(fake_reservation)
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("a fake controller cannot reuse a real activated promotion id");
        assert!(error.to_string().contains("canary_activation_missing"));
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            markers_after_activation,
        );
    }
    #[test]
    fn taira_canary_reservation_withholds_wire_and_rejects_tampered_hashes() {
        offline_test_transaction!(transaction);
        let fixture = canary_consensus_fixture(&transaction, 7);
        let instruction = AuthorizeKagemushaTairaCanaryV4::new(fixture.reservation.clone());
        let encoded = norito::encode_canonical(&instruction).expect("canonical reservation ISI");
        assert!(
            !encoded
                .windows(fixture.canary_transaction_wire.len())
                .any(|window| window == fixture.canary_transaction_wire.as_slice()),
            "pre-finality authorization must not disclose the signed canary wire",
        );
        let decoded: AuthorizeKagemushaTairaCanaryV4 = norito::decode_canonical_with_limits(
            &encoded,
            norito::canonical_decode_limits(encoded.len()),
        )
        .expect("reservation-only authorization decodes");
        assert_eq!(decoded.reservation(), &fixture.reservation);

        commit_canary_activation_binding(&fixture.permit.body.binding, &mut transaction);
        let markers_before = transaction.world.kagemusha_replay_keys.iter().count();
        let mut intent = fixture.reservation.clone();
        intent.body.canary_transaction_intent =
            HashOf::from_untyped_unchecked(Hash::new(b"tampered canary intent"));
        let mut wire = fixture.reservation.clone();
        wire.body.canary_transaction_wire.sha256[0] ^= 1;
        let mut entrypoint = fixture.reservation;
        entrypoint.body.canary_entrypoint_hash = Hash::new(b"tampered canary entrypoint");
        for (case, reservation) in [
            ("intent", intent),
            ("wire", wire),
            ("entrypoint", entrypoint),
        ] {
            let error = AuthorizeKagemushaTairaCanaryV4::new(reservation)
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("tampered exact-hash reservation must fail");
            assert!(
                error
                    .to_string()
                    .contains("invalid_taira_canary_authorization"),
                "unexpected {case} rejection: {error}",
            );
            assert_eq!(
                transaction.world.kagemusha_replay_keys.iter().count(),
                markers_before,
                "tampered {case} must not reserve markers",
            );
        }

        let controller = canary_consensus_controller();
        let mut inconsistent = canary_consensus_fixture(&transaction, 7).reservation;
        inconsistent.body.canary_transaction_intent =
            HashOf::from_untyped_unchecked(Hash::new(b"signed mismatched canary intent"));
        let mut oversized = canary_consensus_fixture(&transaction, 7).reservation;
        oversized.body.canary_transaction_wire.byte_len = u64::MAX;
        for (case, mut reservation) in [("intent", inconsistent), ("wire length", oversized)] {
            reservation.signature = SignatureOf::try_from_hash(
                controller.private_key(),
                reservation.body.signing_hash(),
            )
            .expect("controller signs malformed reservation fixture");
            let error = AuthorizeKagemushaTairaCanaryV4::new(reservation)
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("signed malformed reservation must fail structurally");
            assert!(
                error
                    .to_string()
                    .contains("invalid_taira_canary_authorization"),
                "unexpected signed {case} error: {error}",
            );
        }
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            markers_before,
            "signed malformed reservations must not reserve markers",
        );
    }
    #[test]
    fn taira_canary_exact_two_step_rejects_altered_call_then_consumes_once() {
        offline_test_transaction!(transaction);
        let fixture = canary_consensus_fixture(&transaction, 7);
        commit_canary_activation_binding(&fixture.permit.body.binding, &mut transaction);
        let replay_keys_after_activation = transaction.world.kagemusha_replay_keys.iter().count();
        let foreign_authority = AuthorizeKagemushaTairaCanaryV4::new(fixture.reservation.clone())
            .execute(&BOB_ID, &mut transaction)
            .expect_err("a foreign account cannot front-run exact authorization publication");
        assert!(
            foreign_authority
                .to_string()
                .contains("invalid_taira_canary_authorization")
        );
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            replay_keys_after_activation,
            "foreign-authority rejection must not reserve any marker",
        );
        AuthorizeKagemushaTairaCanaryV4::new(fixture.reservation.clone())
            .execute(&ALICE_ID, &mut transaction)
            .expect("controller signatures authorize without a broad account permission");
        let replay_keys_after_authorization =
            transaction.world.kagemusha_replay_keys.iter().count();
        AuthorizeKagemushaTairaCanaryV4::new(fixture.reservation.clone())
            .execute(&ALICE_ID, &mut transaction)
            .expect("the same exact authorization publication is idempotent");
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            replay_keys_after_authorization,
            "idempotent authorization publication must not add markers",
        );
        let controller = canary_consensus_controller();
        let mut same_call_different_reservation = fixture.reservation.clone();
        same_call_different_reservation
            .body
            .canary_transaction_wire
            .sha256[0] ^= 1;
        same_call_different_reservation.signature = SignatureOf::try_from_hash(
            controller.private_key(),
            same_call_different_reservation.body.signing_hash(),
        )
        .expect("controller signs the conflicting exact reservation");
        let conflicting = AuthorizeKagemushaTairaCanaryV4::new(same_call_different_reservation)
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("the same call hash cannot hide a different exact reservation");
        assert!(
            conflicting
                .to_string()
                .contains("canary_authorization_replay")
        );
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            replay_keys_after_authorization,
            "conflicting exact reservation rejection must not mutate markers",
        );
        let distinct_nonce = canary_consensus_fixture(&transaction, 8);
        let different_exact = AuthorizeKagemushaTairaCanaryV4::new(distinct_nonce.reservation)
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("a different exact call cannot occupy the reserved promotion slot");
        assert!(
            different_exact
                .to_string()
                .contains("canary_authorization_replay")
        );
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            replay_keys_after_authorization,
        );
        bind_canary_consensus_wire(&fixture, &mut transaction);
        transaction.tx_call_hash = Some(Hash::new(b"altered same-permit transaction"));
        let altered = RecordKagemushaTairaCanaryV4::new(fixture.permit.clone())
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("an altered same-permit call must not consume the canary");
        assert!(altered.to_string().contains("canary_authorization_missing"));
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            replay_keys_after_authorization,
        );
        transaction.tx_call_hash = Some(fixture.exact_call_hash);
        bind_canary_consensus_wire(&fixture, &mut transaction);
        let mismatched_authority = RecordKagemushaTairaCanaryV4::new(fixture.permit.clone())
            .execute(&BOB_ID, &mut transaction)
            .expect_err("the exact call still requires the permitted relayer authority");
        assert!(
            mismatched_authority
                .to_string()
                .contains("invalid_taira_canary_permit")
        );
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            replay_keys_after_authorization,
        );

        RecordKagemushaTairaCanaryV4::new(fixture.permit.clone())
            .execute(&ALICE_ID, &mut transaction)
            .expect("the exactly authorized ordinary canary consumes its one-shot marker");
        let replay_keys_after_canary = transaction.world.kagemusha_replay_keys.iter().count();
        bind_canary_consensus_wire(&fixture, &mut transaction);
        let duplicate = RecordKagemushaTairaCanaryV4::new(fixture.permit.clone())
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("the exact canary cannot execute twice");
        assert!(duplicate.to_string().contains("canary_replay"));
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            replay_keys_after_canary,
        );

        AuthorizeKagemushaTairaCanaryV4::new(fixture.reservation)
            .execute(&ALICE_ID, &mut transaction)
            .expect("the same exact reservation stays idempotent after canary consumption");
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            replay_keys_after_canary,
            "post-consumption idempotence must not add or remove markers",
        );

        let post_consumption = canary_consensus_fixture(&transaction, 9);
        let second_authorization =
            AuthorizeKagemushaTairaCanaryV4::new(post_consumption.reservation)
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("a distinct nonce cannot reopen one consumed promotion canary");
        assert!(
            second_authorization
                .to_string()
                .contains("canary_authorization_replay")
        );
    }
    #[test]
    fn taira_canary_complete_wire_rejects_alternate_valid_proof_for_same_intent() {
        offline_test_transaction!(transaction);
        let canary_key = KeyPair::from_seed(vec![0xD4; 32], Algorithm::MlDsa);
        let authority = AccountId::new(canary_key.public_key().clone());
        let first = canary_consensus_fixture_for_key(&transaction, 7, &canary_key);
        let second = canary_consensus_fixture_for_key(&transaction, 7, &canary_key);
        for transaction in [&first.canary_transaction, &second.canary_transaction] {
            transaction
                .verify_signature()
                .expect("independent ML-DSA canary proof verifies");
        }
        assert_eq!(first.permit, second.permit);
        assert_eq!(
            first.canary_transaction.hash(),
            second.canary_transaction.hash()
        );
        assert_eq!(first.exact_call_hash, second.exact_call_hash);
        let wire_identity = |transaction: &SignedTransaction| {
            crate::smartcontracts::isi::offline::signed_kagemusha_taira_canary_wire_identity_v1(
                transaction,
            )
            .expect("derive canary wire")
        };
        let first_wire = wire_identity(&first.canary_transaction)
            .expect("first transaction has exact direct Record shape");
        let second_wire = wire_identity(&second.canary_transaction)
            .expect("second transaction has exact direct Record shape");
        assert_ne!(first_wire, second_wire);
        let multi_record = TransactionBuilder::new(
            transaction.network_id().clone(),
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([
            RecordKagemushaTairaCanaryV4::new(first.permit.clone()),
            RecordKagemushaTairaCanaryV4::new(first.permit.clone()),
        ])
        .sign(canary_key.private_key());
        assert_eq!(wire_identity(&multi_record), None);
        let batch = TransactionBuilder::new(
            transaction.network_id().clone(),
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Batch(
            vec![ExecutableBatchItem::Instruction(
                RecordKagemushaTairaCanaryV4::new(first.permit.clone()).into(),
            )]
            .into(),
        ))
        .sign(canary_key.private_key());
        assert_eq!(wire_identity(&batch), None);
        commit_canary_activation_binding(&first.permit.body.binding, &mut transaction);
        AuthorizeKagemushaTairaCanaryV4::new(first.reservation.clone())
            .execute(&authority, &mut transaction)
            .expect("first exact signed wire is authorized");
        let markers_after_authorization = transaction.world.kagemusha_replay_keys.iter().count();
        transaction.kagemusha_taira_canary_external_entrypoint = true;
        transaction.tx_call_hash = Some(second.exact_call_hash);
        transaction.kagemusha_taira_canary_wire_identity = Some(second_wire);
        let error = RecordKagemushaTairaCanaryV4::new(second.permit)
            .execute(&authority, &mut transaction)
            .expect_err("alternate valid proof wire must not consume the authorization");
        assert!(error.to_string().contains("canary_authorization_missing"));
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            markers_after_authorization
        );
        transaction.kagemusha_taira_canary_wire_identity = Some(first_wire);
        RecordKagemushaTairaCanaryV4::new(first.permit)
            .execute(&authority, &mut transaction)
            .expect("the exactly authorized complete wire consumes the canary");
    }
    #[test]
    fn taira_canary_rejects_invalid_and_expired_permits_before_marker_consumption() {
        offline_test_transaction!(transaction);
        let fixture = canary_consensus_fixture(&transaction, 7);
        commit_canary_activation_binding(&fixture.permit.body.binding, &mut transaction);
        transaction.tx_call_hash = Some(fixture.exact_call_hash);
        bind_canary_consensus_wire(&fixture, &mut transaction);
        let markers_before = transaction.world.kagemusha_replay_keys.iter().count();
        let mut invalid = fixture.permit.clone();
        invalid.body.canonical_torii_origin = "https://different.example".to_owned();
        let invalid_error = RecordKagemushaTairaCanaryV4::new(invalid)
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("tampering after permit signing must fail");
        assert!(
            invalid_error
                .to_string()
                .contains("invalid_taira_canary_permit")
        );
        let controller = canary_consensus_controller();
        let mut expired = fixture.permit;
        expired.body.expires_at_height = NonZeroU64::new(1).expect("non-zero expired height");
        expired.signature =
            SignatureOf::try_from_hash(controller.private_key(), expired.body.signing_hash())
                .expect("controller signs expired test permit");
        let expired_error = RecordKagemushaTairaCanaryV4::new(expired)
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("exclusive current-height expiry must fail");
        assert!(
            expired_error
                .to_string()
                .contains("invalid_taira_canary_permit")
        );
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            markers_before,
            "invalid and expired permits must not consume the canary marker",
        );
    }
    fn offline_permission(name: &str) -> Permission {
        Permission::new(name.to_owned(), Json::new(()))
    }
    fn offline_permission_with_payload(name: &str, payload: Json) -> Permission {
        Permission::new(name.to_owned(), payload)
    }
    fn offline_test_state() -> State {
        let alice = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob = Account::new(BOB_ID.clone()).build(&BOB_ID);
        State::new_for_testing(
            World::with([], [alice, bob], []),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }
    fn offline_holding_limit_test_state(
        escrow_balance: Option<u32>,
    ) -> (State, AssetDefinitionId, AssetId, AccountId) {
        let domain_id = DomainId::try_new("offline", "universal").expect("offline test domain");
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "cash".parse().expect("asset name"),
        );
        let definition = AssetDefinition::numeric(
            definition_id.clone(),
            "Offline Cash".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&ALICE_ID);
        let source_asset = AssetId::new(definition_id.clone(), ALICE_ID.clone());
        let chain_id = ChainId::from("offline-holding-limit-test");
        let network_id = test_network_id(b"offline-holding-limit-test-network");
        let escrow_account = crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
            &network_id,
            &definition_id,
        );
        let escrow_asset = AssetId::new(definition_id.clone(), escrow_account.clone());
        let mut assets = vec![Asset::new(source_asset.clone(), Quantity::from(10_u32))];
        if let Some(balance) = escrow_balance {
            assets.push(Asset::new(escrow_asset, Quantity::from(balance)));
        }
        let world = World::with_assets(
            [Domain::new(domain_id).build(&ALICE_ID)],
            [
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                Account::new(BOB_ID.clone()).build(&ALICE_ID),
                Account::new(escrow_account.clone()).build(&ALICE_ID),
            ],
            [definition],
            assets,
            [],
        );
        let mut state = State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            chain_id,
            network_id,
        );
        let mut settlement = iroha_config::parameters::actual::Settlement::default();
        settlement
            .offline
            .escrow_accounts
            .insert(definition_id.clone(), escrow_account.clone());
        state.set_settlement(settlement);
        (state, definition_id, source_asset, escrow_account)
    }
    fn set_offline_holding_limit(
        state_transaction: &mut StateTransaction<'_, '_>,
        account: &AccountId,
        definition_id: &AssetDefinitionId,
        limit: u32,
    ) {
        state_transaction.tx_call_hash = Some(Hash::prehashed([0xA8; Hash::LENGTH]));
        SetAssetHoldingLimit::new(
            account.clone(),
            definition_id.clone(),
            Some(Quantity::from(limit)),
        )
        .execute(&ALICE_ID, state_transaction)
        .expect("asset definition owner sets holding limit");
    }
    fn offline_asset_entries(
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Vec<(AssetId, Quantity)> {
        state_transaction
            .world
            .assets
            .iter()
            .map(|(id, asset)| (id.clone(), asset.as_ref().clone()))
            .collect()
    }
    fn assert_holding_limit_exceeded(error: &Error) {
        assert!(
            matches!(
                error,
                InstructionExecutionError::AssetTransferAdmission(
                    AssetTransferAdmissionError::HoldingLimitExceeded(_)
                )
            ),
            "expected typed holding-limit rejection, got {error:?}",
        );
    }
    #[test]
    fn offline_use_lazily_materializes_deterministic_escrow_for_any_asset() {
        let chain_id = ChainId::from("universal-offline-test");
        let network_id = test_network_id(b"universal-offline-test-network");
        let domain_id = DomainId::try_new("ordinary", "universal").expect("ordinary test domain");
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "unit".parse().expect("asset name"),
        );
        let definition = AssetDefinition::numeric(
            definition_id.clone(),
            "Ordinary Unit".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&ALICE_ID);
        let source_asset = AssetId::new(definition_id.clone(), ALICE_ID.clone());
        let world = World::with_assets(
            [Domain::new(domain_id).build(&ALICE_ID)],
            [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
            [definition],
            [Asset::new(source_asset.clone(), Quantity::from(10_u32))],
            [],
        );
        let state = State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            chain_id,
            network_id,
        );
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
        assert!(
            state_transaction
                .settlement
                .offline
                .escrow_accounts
                .is_empty()
        );
        reserve_kagemusha_escrow(
            &mut state_transaction,
            source_asset.account(),
            [0x80; 32],
            &source_asset,
            &Quantity::from(3_u32),
        )
        .expect("offline use should need no asset flag or configured catalog");
        let expected = crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
            state_transaction.network_id(),
            &definition_id,
        );
        assert_eq!(
            state_transaction
                .settlement
                .offline
                .escrow_accounts
                .get(&definition_id),
            Some(&expected)
        );
        assert!(state_transaction.world.account(&expected).is_ok());
        assert_eq!(
            state_transaction
                .world
                .assets
                .get(&AssetId::new(definition_id, expected))
                .expect("lazy escrow balance")
                .as_ref(),
            &Quantity::from(3_u32)
        );
    }
    #[test]
    fn offline_escrow_reservation_holding_limit_failure_is_atomic() {
        let (state, definition_id, source_asset, escrow_account) =
            offline_holding_limit_test_state(None);
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
        set_offline_holding_limit(&mut state_transaction, &escrow_account, &definition_id, 0);
        let entries_before = offline_asset_entries(&state_transaction);
        let events_before = state_transaction.world.internal_event_buf.len();
        let error = reserve_kagemusha_escrow(
            &mut state_transaction,
            source_asset.account(),
            [0x81; 32],
            &source_asset,
            &Quantity::from(1_u32),
        )
        .expect_err("escrow reservation above its holding limit must fail");
        assert_holding_limit_exceeded(&error);
        assert_eq!(offline_asset_entries(&state_transaction), entries_before);
        assert_eq!(
            state_transaction.world.internal_event_buf.len(),
            events_before,
            "rejected escrow reservation must not emit events",
        );
    }
    #[test]
    fn kagemusha_redemption_plan_rejects_holding_limit_without_mutation() {
        let (state, definition_id, source_asset, _) = offline_holding_limit_test_state(Some(10));
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
        set_offline_holding_limit(&mut state_transaction, &BOB_ID, &definition_id, 0);
        let entries_before = offline_asset_entries(&state_transaction);
        let events_before = state_transaction.world.internal_event_buf.len();
        let error = plan_kagemusha_v2_escrow_credit(
            [0x82; 32],
            &source_asset,
            &BOB_ID,
            &Quantity::from(1_u32),
            &state_transaction,
        )
        .expect_err("Kagemusha redemption planning must enforce the recipient holding limit");
        assert_holding_limit_exceeded(&error);
        assert_eq!(offline_asset_entries(&state_transaction), entries_before);
        assert_eq!(
            state_transaction.world.internal_event_buf.len(),
            events_before,
            "rejected Kagemusha redemption planning must not emit events",
        );
    }
    #[test]
    fn kagemusha_redemption_commit_rechecks_holding_limit_atomically() {
        let (state, definition_id, source_asset, _) = offline_holding_limit_test_state(Some(10));
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
        set_offline_holding_limit(&mut state_transaction, &BOB_ID, &definition_id, 1);
        let plan = plan_kagemusha_v2_escrow_credit(
            [0x83; 32],
            &source_asset,
            &BOB_ID,
            &Quantity::from(1_u32),
            &state_transaction,
        )
        .expect("credit at the current holding limit should plan");
        set_offline_holding_limit(&mut state_transaction, &BOB_ID, &definition_id, 0);
        let entries_before = offline_asset_entries(&state_transaction);
        let events_before = state_transaction.world.internal_event_buf.len();
        let error = plan
            .commit(&mut state_transaction)
            .expect_err("commit must recheck a holding limit changed after planning");
        assert_holding_limit_exceeded(&error);
        assert_eq!(offline_asset_entries(&state_transaction), entries_before);
        assert_eq!(
            state_transaction.world.internal_event_buf.len(),
            events_before,
            "rejected Kagemusha redemption commit must not emit events",
        );
    }
    #[test]
    fn every_offline_executor_is_independent_of_local_service_switch() {
        let source = include_str!("../offline.rs");
        let executor_names = [
            "RegisterOfflineDeviceAttestation",
            "SetOfflineDeviceAttestationPolicy",
            "ActivateKagemushaRecursiveReleaseV4",
            "TopUpKagemushaRecursiveV4",
            "RedeemKagemushaRecursiveV4",
        ];
        let starts = executor_names
            .iter()
            .map(|name| {
                source
                    .find(&format!("impl Execute for {name}"))
                    .unwrap_or_else(|| panic!("missing offline executor {name}"))
            })
            .collect::<Vec<_>>();
        let last_start = *starts.last().expect("offline executor list is non-empty");
        let tests_start = last_start
            + source[last_start..]
                .find("#[cfg(test)]")
                .expect("offline executor test module");
        for (index, name) in executor_names.iter().enumerate() {
            let end = starts.get(index + 1).copied().unwrap_or(tests_start);
            let executor = &source[starts[index]..end];
            assert!(
                !executor.contains("settlement.offline.enabled")
                    && !executor.contains("ensure_offline_enabled"),
                "{name} must not derive consensus validity from a process-local service switch"
            );
        }
    }
    #[test]
    fn offline_instruction_execution_requires_no_enablement_switch() {
        offline_test_transaction!(state_transaction);
        state_transaction.world.add_account_permission(
            &ALICE_ID,
            offline_permission(CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION),
        );
        SetOfflineDeviceAttestationPolicy::new(
            default_offline_device_attestation_policy()
                .expect("built-in policy fixture must be valid"),
        )
        .execute(&ALICE_ID, &mut state_transaction)
        .expect("process-local service switches must not affect consensus execution");
        assert!(
            state_transaction
                .world
                .smart_contract_state
                .get(&*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY)
                .is_some(),
            "valid offline instructions must execute regardless of local service state"
        );
    }
    include!("isi_attestation_policy_release_tests.rs");
    #[test]
    fn release_activation_authority_requires_both_exact_governance_permissions() {
        fn authorization_result(permissions: Vec<Permission>) -> Result<(), Error> {
            offline_test_transaction!(state_transaction);
            for permission in permissions {
                grant_alice_permission(&mut state_transaction, GrantSource::Direct, permission);
            }
            ensure_kagemusha_recursive_release_v4_activation_authorized(
                &state_transaction,
                &ALICE_ID,
            )
        }
        let activate =
            || offline_permission(CAN_ACTIVATE_KAGEMUSHA_RECURSIVE_RELEASE_V4_PERMISSION);
        let manage = || offline_permission(CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION);
        assert!(authorization_result(Vec::new()).is_err());
        let error = authorization_result(vec![activate()])
            .expect_err("activate-only authority must not publish the composite instruction");
        assert!(
            error
                .to_string()
                .contains("CanManageOfflineDeviceAttestationPolicy")
        );
        let error = authorization_result(vec![manage()])
            .expect_err("policy-only authority must not publish the composite instruction");
        assert!(
            error
                .to_string()
                .contains("CanActivateKagemushaRecursiveReleaseV4")
        );
        authorization_result(vec![activate(), manage()])
            .expect("both exact unit permissions authorize the composite guard");
        let foreign_payload = Json::new("foreign-scope");
        assert!(
            authorization_result(vec![
                offline_permission_with_payload(
                    CAN_ACTIVATE_KAGEMUSHA_RECURSIVE_RELEASE_V4_PERMISSION,
                    foreign_payload.clone(),
                ),
                manage(),
            ])
            .is_err(),
            "the activation permission payload must match exactly",
        );
        assert!(
            authorization_result(vec![
                activate(),
                offline_permission_with_payload(
                    CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION,
                    foreign_payload,
                ),
            ])
            .is_err(),
            "the device-policy permission payload must match exactly",
        );
    }
    #[test]
    fn offline_device_attestation_policy_absence_fails_closed() {
        offline_test_transaction!(state_transaction);
        let error = effective_offline_device_attestation_policy(&state_transaction)
            .expect_err("missing governed attestation policy must fail closed");
        assert!(
            error
                .to_string()
                .contains("offline_reason::invalid_attestation_policy"),
            "unexpected missing-policy rejection: {error}"
        );
        let policy = default_offline_device_attestation_policy()
            .expect("bundled roots form a valid policy template");
        state_transaction.world.smart_contract_state.insert(
            (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
            norito::to_bytes(&policy).expect("policy must encode"),
        );
        assert_eq!(
            effective_offline_device_attestation_policy(&state_transaction)
                .expect("an explicitly installed policy must be available"),
            policy
        );
    }
    fn offline_test_header() -> BlockHeader {
        BlockHeader::new(
            NonZeroU64::new(1).expect("nonzero block height"),
            None,
            None,
            None,
            POLICY_TEST_TIME_MS,
            0,
        )
    }
    fn offline_test_asset(account: &AccountId) -> AssetId {
        let definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("offline", "universal").expect("valid test domain"),
            "cash".parse().expect("valid test asset name"),
        );
        AssetId::new(definition, account.clone())
    }
    fn online_assertion_signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes((&[seed; 32]).into())
            .expect("nonzero P-256 online assertion test scalar")
    }
    fn online_assertion_signature(key: &SigningKey, message: &[u8]) -> KagemushaDeviceSignatureV2 {
        let signature: P256Signature = key.sign(message);
        let signature = signature.normalize_s().unwrap_or(signature);
        KagemushaDeviceSignatureV2::from_raw_bytes(signature.to_bytes().as_slice())
            .expect("canonical low-S online assertion fixture")
    }
    fn test_der_tlv(tag: &[u8], value: &[u8]) -> Vec<u8> {
        assert!(value.len() < 128, "test DER fixture uses one-byte lengths");
        let mut encoded = Vec::with_capacity(tag.len() + 1 + value.len());
        encoded.extend_from_slice(tag);
        encoded.push(value.len() as u8);
        encoded.extend_from_slice(value);
        encoded
    }
    fn android_root_of_trust_fixture(
        verified_boot_key: &[u8],
        device_locked: u8,
        verified_boot_state: u8,
        verified_boot_hash: &[u8],
    ) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&test_der_tlv(&[0x04], verified_boot_key));
        body.extend_from_slice(&test_der_tlv(&[0x01], &[device_locked]));
        body.extend_from_slice(&test_der_tlv(&[0x0A], &[verified_boot_state]));
        body.extend_from_slice(&test_der_tlv(&[0x04], verified_boot_hash));
        test_der_tlv(&[0x30], &body)
    }
    fn android_key_description_usage_count_fixture(
        software_usage_count_limit: bool,
        hardware_usage_count_limit: bool,
        hardware_root_of_trust: bool,
    ) -> Vec<u8> {
        fn authorization_list(with_usage_count_limit: bool, with_root: bool) -> Vec<u8> {
            let mut body = if with_usage_count_limit {
                let one = test_der_tlv(&[0x02], &[1]);
                // Context-specific constructed high tag [405].
                test_der_tlv(&[0xBF, 0x83, 0x15], &one)
            } else {
                Vec::new()
            };
            if with_root {
                let root = android_root_of_trust_fixture(&[0xA5], 0xFF, 0, &[0x5A; 32]);
                // Context-specific constructed high tag [704].
                body.extend_from_slice(&test_der_tlv(&[0xBF, 0x85, 0x40], &root));
            }
            test_der_tlv(&[0x30], &body)
        }
        let mut body = Vec::new();
        body.extend_from_slice(&test_der_tlv(&[0x02], &[3]));
        body.extend_from_slice(&test_der_tlv(&[0x0A], &[1]));
        body.extend_from_slice(&test_der_tlv(&[0x02], &[4]));
        body.extend_from_slice(&test_der_tlv(&[0x0A], &[1]));
        body.extend_from_slice(&test_der_tlv(&[0x04], &[0xA5]));
        body.extend_from_slice(&test_der_tlv(&[0x04], &[]));
        body.extend_from_slice(&authorization_list(software_usage_count_limit, false));
        body.extend_from_slice(&authorization_list(
            hardware_usage_count_limit,
            hardware_root_of_trust,
        ));
        test_der_tlv(&[0x30], &body)
    }
    #[test]
    fn android_usage_count_limit_must_be_hardware_enforced() {
        let hardware = parse_android_key_description(&android_key_description_usage_count_fixture(
            false, true, true,
        ))
        .expect("hardware-enforced usageCountLimit is admitted");
        assert_eq!(hardware.usage_count_limit, Some(1));
        let mut zero_keymint_version =
            android_key_description_usage_count_fixture(false, true, true);
        let version_offset = zero_keymint_version
            .windows(3)
            .position(|window| window == [0x02, 0x01, 0x04])
            .expect("KeyMint version fixture");
        zero_keymint_version[version_offset + 2] = 0;
        assert!(parse_android_key_description(&zero_keymint_version).is_err());
        assert!(
            parse_android_key_description(&android_key_description_usage_count_fixture(
                true, false, true,
            ))
            .is_err(),
            "a software-only usageCountLimit must not satisfy the hardware one-use profile",
        );
    }
    #[test]
    fn android_root_of_trust_must_be_hardware_verified_and_complete() {
        let valid = android_root_of_trust_fixture(&[0xA5], 0xFF, 0, &[0x5A; 32]);
        validate_android_root_of_trust(&valid).expect("locked verified-boot rootOfTrust");
        for invalid in [
            android_root_of_trust_fixture(&[], 0xFF, 0, &[0x5A; 32]),
            android_root_of_trust_fixture(&[0xA5], 0, 0, &[0x5A; 32]),
            android_root_of_trust_fixture(&[0xA5], 1, 0, &[0x5A; 32]),
            android_root_of_trust_fixture(&[0xA5], 0xFF, 1, &[0x5A; 32]),
            android_root_of_trust_fixture(&[0xA5], 0xFF, 0, &[0x5A; 31]),
        ] {
            assert!(validate_android_root_of_trust(&invalid).is_err());
        }

        let software_root = test_der_tlv(&[0xBF, 0x85, 0x40], &valid);
        assert!(parse_android_authorization_list(&software_root, false).is_err());
        assert!(
            parse_android_key_description(&android_key_description_usage_count_fixture(
                false, true, false,
            ))
            .is_err(),
            "rootOfTrust is mandatory even for an otherwise valid hardware authorization list",
        );
    }
    #[test]
    fn android_authorization_list_rejects_duplicate_unknown_tags() {
        let unknown = test_der_tlv(&[0xBF, 0x1F], &[]);
        let mut duplicated = unknown.clone();
        duplicated.extend_from_slice(&unknown);
        assert!(parse_android_authorization_list(&duplicated, false).is_err());
    }
    #[test]
    fn android_application_id_must_bind_one_exact_package_and_signer() {
        let signing_digest = [0x55; 32];
        let mut application_id = AndroidAttestationApplicationId {
            packages: vec![AndroidAttestationPackageInfo {
                package_name: "com.pk.retailwallet".to_owned(),
            }],
            signature_digests: vec![signing_digest.to_vec()],
        };
        validate_android_attestation_application_id_matches(
            &application_id,
            "com.pk.retailwallet",
            &signing_digest,
        )
        .expect("one exact package and signer must pass");

        application_id.packages.push(AndroidAttestationPackageInfo {
            package_name: "com.attacker.shareduid".to_owned(),
        });
        assert!(
            validate_android_attestation_application_id_matches(
                &application_id,
                "com.pk.retailwallet",
                &signing_digest,
            )
            .is_err()
        );
        application_id.packages.pop();
        application_id.signature_digests.push(vec![0x66; 32]);
        assert!(
            validate_android_attestation_application_id_matches(
                &application_id,
                "com.pk.retailwallet",
                &signing_digest,
            )
            .is_err()
        );
    }
    fn android_online_registration(
        account: &AccountId,
        asset: &AssetDefinitionId,
        assertion_key: &SigningKey,
        expires_at_ms: u64,
    ) -> OfflineDeviceAttestationRegistration {
        let assertion_public_key = assertion_key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes()
            .to_vec();
        let public_key = KagemushaDevicePublicKeyV2::from_sec1_bytes(&assertion_public_key)
            .expect("canonical P-256 fixture public key");
        let attestation_report = b"admitted-android-registration-fixture".to_vec();
        let evidence = b"admitted-android-evidence-fixture".to_vec();
        OfflineDeviceAttestationRegistration {
            version: 1,
            platform: OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned(),
            key_id: hex::encode(sha256_bytes(&assertion_public_key)),
            device_id: "android-online-device".to_owned(),
            account_id: account.clone(),
            asset_definition_id: Some(asset.clone()),
            ios_team_id: None,
            ios_bundle_id: None,
            ios_environment: None,
            android_package_name: Some("com.pk.retailwallet".to_owned()),
            android_signing_certificate_sha256: Some(vec![0x55; 32]),
            public_key,
            assertion_scheme: OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME.to_owned(),
            assertion_key_algorithm: OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM.to_owned(),
            assertion_public_key,
            assertion_usage_count_limit: Some(1),
            one_use: true,
            challenge_hash: Hash::new(b"admitted-android-registration-challenge"),
            attestation_report_hash: Hash::new(&attestation_report),
            attestation_report,
            evidence_hash: Hash::new(&evidence),
            evidence,
            recent_block_height: 1,
            recent_block_hash: Hash::new(b"admitted-android-registration-block"),
            expires_at_ms,
        }
    }
    fn android_online_authorization(
        registration: &OfflineDeviceAttestationRegistration,
        assertion_key: &SigningKey,
    ) -> KagemushaRequestAuthorizationV2 {
        let registration_hash = canonical_registration_hash(registration)
            .map(|hash| exact_hash_bytes(&hash))
            .expect("canonical registration hash");
        let placeholder = KagemushaDeviceSignatureV2::from_raw_bytes(&{
            let mut raw = [0_u8; 64];
            raw[31] = 1;
            raw[63] = 1;
            raw
        })
        .expect("valid low-S placeholder");
        let mut authorization = KagemushaRequestAuthorizationV2 {
            authority: registration.account_id.clone(),
            device_id: registration.device_id.clone(),
            asset_definition_id: registration
                .asset_definition_id
                .clone()
                .expect("asset-bound fixture"),
            operation_id: [0x61; 32],
            issued_at_ms: POLICY_TEST_TIME_MS,
            expires_at_ms: POLICY_TEST_TIME_MS + 30_000,
            nonce: [0x62; 32],
            payload_digest: [0x63; 32],
            registration_hash,
            hardware_assertion: KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(
                KagemushaAndroidKeyMintHardwareAssertionV1 {
                    signature: placeholder,
                },
            ),
        };
        let signing_bytes = authorization
            .signing_bytes()
            .expect("canonical online assertion preimage");
        authorization
            .set_hardware_signature(online_assertion_signature(assertion_key, &signing_bytes));
        authorization
    }
    fn install_android_online_registration(
        state_transaction: &mut StateTransaction<'_, '_>,
        registration: OfflineDeviceAttestationRegistration,
    ) -> StatePath {
        let mut policy =
            default_offline_device_attestation_policy().expect("built-in attestation roots");
        policy.require_android_app_policy = true;
        policy.android_status_snapshot = Some(android_status_snapshot());
        policy.android_apps = vec![OfflineAndroidAppAttestationPolicy {
            package_name: "com.pk.retailwallet".to_owned(),
            signing_certificate_sha256: vec![vec![0x55; 32]],
        }];
        state_transaction.world.smart_contract_state.insert(
            (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
            norito::to_bytes(&policy).expect("canonical test policy"),
        );
        let policy_hash = canonical_offline_device_attestation_policy_hash(&policy)
            .expect("canonical policy hash");
        install_android_online_registration_with_policy_hash(
            state_transaction,
            registration,
            policy_hash,
        )
    }
    fn install_android_online_registration_with_policy_hash(
        state_transaction: &mut StateTransaction<'_, '_>,
        registration: OfflineDeviceAttestationRegistration,
        admission_policy_hash: [u8; 32],
    ) -> StatePath {
        let registration_hash = canonical_registration_hash(&registration)
            .map(|hash| exact_hash_bytes(&hash))
            .expect("canonical registration hash");
        let state_key = kagemusha_online_registration_state_key(&registration_hash)
            .expect("canonical registration state key");
        let registration = compact_kagemusha_registration_projection(&registration);
        let registration_projection_hash = canonical_registration_hash(&registration)
            .map(|hash| exact_hash_bytes(&hash))
            .expect("canonical compact registration hash");
        let state = KagemushaOnlineRegistrationStateV4 {
            version: KAGEMUSHA_ONLINE_REGISTRATION_STATE_VERSION_V4,
            original_registration_hash: registration_hash,
            registration_projection_hash,
            admission_policy_hash,
            admission_height: state_transaction.block_height(),
            admission_transaction_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"test-device-registration-transaction",
            )),
            registration,
            lifecycle: KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintUnused,
        };
        state_transaction.world.smart_contract_state.insert(
            state_key.clone(),
            encode_kagemusha_online_registration_state_v4(&state)
                .expect("bounded canonical online registration state"),
        );
        state_key
    }
    include!("isi_kagemusha_registration_capacity_tests.rs");
    fn committed_android_replay_fixture(
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> (
        AssetId,
        KagemushaRequestAuthorizationV2,
        KagemushaRequestAuthorizationV2,
        StatePath,
    ) {
        let asset = offline_test_asset(&ALICE_ID);
        let assertion_key = online_assertion_signing_key(0x71);
        let wrong_key = online_assertion_signing_key(0x72);
        let registration = android_online_registration(
            &ALICE_ID,
            asset.definition(),
            &assertion_key,
            POLICY_TEST_TIME_MS + 60_000,
        );
        let authorization = android_online_authorization(&registration, &assertion_key);
        let wrong_signature = android_online_authorization(&registration, &wrong_key);
        wrong_signature
            .validate_for_payload_at(wrong_signature.payload_digest, POLICY_TEST_TIME_MS)
            .expect("the substituted signature remains structurally well formed");
        let state_key = install_android_online_registration(state_transaction, registration);
        let replay_markers = match kagemusha_v4_replay_status(&authorization, state_transaction)
            .expect("fresh replay fixture")
        {
            KagemushaV4ReplayStatus::Fresh(markers) => markers,
            KagemushaV4ReplayStatus::Committed => panic!("fixture unexpectedly committed"),
        };
        let consumption = ensure_registered_kagemusha_v2_device(
            &authorization,
            asset.definition(),
            state_transaction,
        )
        .expect("valid hardware assertion consumption plan");
        commit_kagemusha_online_hardware_assertion(consumption, state_transaction)
            .expect("consume fixture hardware assertion");
        commit_kagemusha_v4_replay_markers(replay_markers, state_transaction);
        (asset, authorization, wrong_signature, state_key)
    }
    #[test]
    fn topup_committed_replay_authenticates_submitter_and_hardware_before_lookup() {
        offline_test_transaction!(state_transaction);
        let (asset, authorization, wrong_signature, state_key) =
            committed_android_replay_fixture(&mut state_transaction);
        let unauthorized = authenticate_kagemusha_v4_topup_submission_before_replay(
            &asset,
            &BOB_ID,
            &authorization,
            &state_transaction,
        );
        let Err(error) = unauthorized else {
            panic!("an unrelated submitter must not observe a committed top-up retry")
        };
        assert!(error.to_string().contains("unauthorized_controller"));
        let malformed = authenticate_kagemusha_v4_topup_submission_before_replay(
            &asset,
            &ALICE_ID,
            &wrong_signature,
            &state_transaction,
        );
        let Err(error) = malformed else {
            panic!("a wrong hardware signature must not observe a committed top-up retry")
        };
        assert!(error.to_string().contains("invalid_authorization"));
        let registration_before = state_transaction
            .world
            .smart_contract_state
            .get(&state_key)
            .cloned()
            .expect("committed registration lifecycle");
        let (_, replay) = authenticate_kagemusha_v4_topup_submission_before_replay(
            &asset,
            &ALICE_ID,
            &authorization,
            &state_transaction,
        )
        .expect("authorized exact retry");
        assert!(matches!(replay, KagemushaV4ReplayStatus::Committed));
        assert_eq!(
            state_transaction.world.smart_contract_state.get(&state_key),
            Some(&registration_before),
            "idempotent retry authentication must not consume the lifecycle again",
        );
    }
    #[test]
    fn redeem_committed_replay_authenticates_submitter_and_hardware_before_receipt() {
        offline_test_transaction!(state_transaction);
        let (asset, authorization, wrong_signature, state_key) =
            committed_android_replay_fixture(&mut state_transaction);
        let release_policy = effective_offline_device_attestation_policy(&state_transaction)
            .expect("installed release policy");
        let unauthorized = authenticate_kagemusha_v4_redeem_submission_before_replay(
            &ALICE_ID,
            asset.definition(),
            &BOB_ID,
            &authorization,
            &release_policy,
            &state_transaction,
        );
        let Err(error) = unauthorized else {
            panic!("an unrelated submitter must not observe a committed redemption receipt")
        };
        assert!(error.to_string().contains("unauthorized_controller"));
        let malformed = authenticate_kagemusha_v4_redeem_submission_before_replay(
            &ALICE_ID,
            asset.definition(),
            &ALICE_ID,
            &wrong_signature,
            &release_policy,
            &state_transaction,
        );
        let Err(error) = malformed else {
            panic!("a wrong hardware signature must not observe a committed redemption receipt")
        };
        assert!(error.to_string().contains("invalid_authorization"));
        let registration_before = state_transaction
            .world
            .smart_contract_state
            .get(&state_key)
            .cloned()
            .expect("committed registration lifecycle");
        let (_, replay) = authenticate_kagemusha_v4_redeem_submission_before_replay(
            &ALICE_ID,
            asset.definition(),
            &ALICE_ID,
            &authorization,
            &release_policy,
            &state_transaction,
        )
        .expect("authorized exact retry");
        assert!(matches!(replay, KagemushaV4ReplayStatus::Committed));
        assert_eq!(
            state_transaction.world.smart_contract_state.get(&state_key),
            Some(&registration_before),
            "idempotent retry authentication must not consume the lifecycle again",
        );
    }
    #[test]
    fn active_receiver_snapshot_routes_one_native_registration_and_rejects_ambiguity() {
        offline_test_transaction!(state_transaction);
        let asset = offline_test_asset(&ALICE_ID).definition().clone();
        state_transaction.world.asset_definitions.insert(
            asset.clone(),
            AssetDefinition::numeric(
                asset.clone(),
                "cash".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .build(&ALICE_ID),
        );
        let assertion_key = online_assertion_signing_key(0x61);
        let registration = android_online_registration(
            &ALICE_ID,
            &asset,
            &assertion_key,
            POLICY_TEST_TIME_MS + 60_000,
        );
        install_android_online_registration(&mut state_transaction, registration.clone());
        let snapshot = derive_kagemusha_active_receiver_snapshot_v1(
            &state_transaction.world,
            1,
            POLICY_TEST_TIME_MS,
        )
        .expect("derive governed receiver snapshot");
        let key = KagemushaActiveReceiverKeyV1 {
            account_id: ALICE_ID.clone(),
            device_id: registration.device_id.clone(),
            asset_definition_id: asset.clone(),
        };
        let (entry, membership) = snapshot
            .active_membership(&key)
            .expect("one native registration is routable");
        assert!(membership.verify(&entry, &snapshot.commitment));
        let KagemushaActiveReceiverEntryV1::Active(active) = entry else {
            panic!("one native registration must produce an active entry")
        };
        let resolved = resolve_kagemusha_active_receiver_registration_v1(
            &state_transaction.world,
            &active,
            1,
            POLICY_TEST_TIME_MS,
        )
        .expect("active leaf resolves to exact native state");
        assert_eq!(
            resolved.registration,
            compact_kagemusha_registration_projection(&registration),
        );
        assert_eq!(
            resolved.registration_hash,
            canonical_registration_hash(&registration)
                .map(|hash| exact_hash_bytes(&hash))
                .expect("original registration remains hashable"),
        );
        assert!(!registration.attestation_report.is_empty());
        assert!(!registration.evidence.is_empty());
        assert!(resolved.registration.attestation_report.is_empty());
        assert!(resolved.registration.evidence.is_empty());
        let second_assertion_key = online_assertion_signing_key(0x62);
        let conflicting = android_online_registration(
            &ALICE_ID,
            &asset,
            &second_assertion_key,
            POLICY_TEST_TIME_MS + 60_000,
        );
        install_android_online_registration(&mut state_transaction, conflicting);
        let ambiguous = derive_kagemusha_active_receiver_snapshot_v1(
            &state_transaction.world,
            1,
            POLICY_TEST_TIME_MS,
        )
        .expect("derive ambiguous governed receiver snapshot");
        assert!(
            ambiguous.active_membership(&key).is_err(),
            "multiple native registrations for one tuple must fail closed"
        );
    }
    #[test]
    fn android_online_assertion_is_staged_then_consumed_exactly_once() {
        offline_test_transaction!(state_transaction);
        let asset = offline_test_asset(&ALICE_ID).definition().clone();
        let assertion_key = online_assertion_signing_key(0x61);
        let registration = android_online_registration(
            &ALICE_ID,
            &asset,
            &assertion_key,
            POLICY_TEST_TIME_MS + 60_000,
        );
        let authorization = android_online_authorization(&registration, &assertion_key);
        let state_key = install_android_online_registration(&mut state_transaction, registration);
        let before = state_transaction
            .world
            .smart_contract_state
            .get(&state_key)
            .cloned()
            .expect("installed online registration state");
        let plan =
            ensure_registered_kagemusha_v2_device(&authorization, &asset, &state_transaction)
                .expect("valid unused one-use assertion is admitted");
        assert_eq!(
            state_transaction.world.smart_contract_state.get(&state_key),
            Some(&before),
            "read-only admission must not consume the key when the transaction later fails",
        );
        commit_kagemusha_online_hardware_assertion(plan, &mut state_transaction)
            .expect("successful transaction atomically consumes the assertion");
        let consumed: KagemushaOnlineRegistrationStateV4 = norito::decode_from_bytes(
            state_transaction
                .world
                .smart_contract_state
                .get(&state_key)
                .expect("consumed registration state"),
        )
        .expect("decode consumed registration state");
        assert!(matches!(
            consumed.lifecycle,
            KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintConsumed(_)
        ));
        let error =
            ensure_registered_kagemusha_v2_device(&authorization, &asset, &state_transaction)
                .err()
                .expect("a fresh execution cannot consume the same KeyMint key twice");
        assert!(error.to_string().contains("hardware_assertion_consumed"));
    }
    #[test]
    fn attestation_policy_rotation_forces_device_reregistration() {
        offline_test_transaction!(state_transaction);
        let asset = offline_test_asset(&ALICE_ID).definition().clone();
        let assertion_key = online_assertion_signing_key(0x66);
        let registration = android_online_registration(
            &ALICE_ID,
            &asset,
            &assertion_key,
            POLICY_TEST_TIME_MS + 60_000,
        );
        let authorization = android_online_authorization(&registration, &assertion_key);
        let state_key = install_android_online_registration(&mut state_transaction, registration);
        let registration_before = state_transaction
            .world
            .smart_contract_state
            .get(&state_key)
            .cloned()
            .expect("installed registration state");
        let policy_bytes = state_transaction
            .world
            .smart_contract_state
            .get(&*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY)
            .cloned()
            .expect("installed attestation policy");
        let mut rotated: OfflineDeviceAttestationPolicy =
            norito::decode_from_bytes(&policy_bytes).expect("decode test policy");
        rotated.revoked_certificate_tbs_sha256.push(vec![0xA7; 32]);
        state_transaction.world.smart_contract_state.insert(
            (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
            norito::to_bytes(&rotated).expect("rotated policy must encode"),
        );
        let error =
            ensure_registered_kagemusha_v2_device(&authorization, &asset, &state_transaction)
                .err()
                .expect("policy rotation must invalidate the prior admission");
        assert!(
            error.to_string().contains("attestation_policy_changed"),
            "unexpected policy-rotation rejection: {error}"
        );
        assert_eq!(
            state_transaction.world.smart_contract_state.get(&state_key),
            Some(&registration_before),
            "rejected use after policy rotation must not consume the hardware lifecycle"
        );
    }
    include!("isi_kagemusha_redemption_policy_tests.rs");
    #[test]
    fn legacy_registration_state_without_policy_hash_fails_closed() {
        #[derive(Encode)]
        struct LegacyRegistrationStateV1 {
            version: u16,
            registration: OfflineDeviceAttestationRegistration,
            lifecycle: KagemushaOnlineHardwareAssertionLifecycleV1,
        }
        offline_test_transaction!(state_transaction);
        let asset = offline_test_asset(&ALICE_ID).definition().clone();
        let assertion_key = online_assertion_signing_key(0x67);
        let registration = android_online_registration(
            &ALICE_ID,
            &asset,
            &assertion_key,
            POLICY_TEST_TIME_MS + 60_000,
        );
        let authorization = android_online_authorization(&registration, &assertion_key);
        let state_key =
            install_android_online_registration(&mut state_transaction, registration.clone());
        state_transaction.world.smart_contract_state.insert(
            state_key,
            norito::to_bytes(&LegacyRegistrationStateV1 {
                version: 1,
                registration,
                lifecycle: KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintUnused,
            })
            .expect("legacy registration state must encode"),
        );
        assert!(
            ensure_registered_kagemusha_v2_device(&authorization, &asset, &state_transaction)
                .is_err(),
            "state without an admission-policy hash must require re-registration"
        );
    }
    #[test]
    fn android_online_assertion_rejects_cross_binding_and_conflicting_commit() {
        offline_test_transaction!(state_transaction);
        let asset = offline_test_asset(&ALICE_ID).definition().clone();
        let assertion_key = online_assertion_signing_key(0x62);
        let registration = android_online_registration(
            &ALICE_ID,
            &asset,
            &assertion_key,
            POLICY_TEST_TIME_MS + 60_000,
        );
        let authorization = android_online_authorization(&registration, &assertion_key);
        install_android_online_registration(&mut state_transaction, registration.clone());
        let mut cross_account = authorization.clone();
        cross_account.authority = BOB_ID.clone();
        let mut cross_device = authorization.clone();
        cross_device.device_id = "substituted-device".to_owned();
        let mut cross_asset = authorization.clone();
        cross_asset.asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("offline", "universal").expect("test domain"),
            "other_cash".parse().expect("test asset name"),
        );
        let mut cross_hash = authorization.clone();
        cross_hash.registration_hash = [0x71; 32];
        let mut cross_platform = authorization.clone();
        cross_platform.hardware_assertion = KagemushaOnlineHardwareAssertionV1::IosAppAttest(
            KagemushaIosAppAttestHardwareAssertionV1 {
                authenticator_data: ios_assertion_auth_data(
                    [0; 32],
                    OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
                    1,
                    &ios_assertion_extension_bytes("42", 4),
                ),
                signature: match &authorization.hardware_assertion {
                    KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(assertion) => {
                        assertion.signature
                    }
                    KagemushaOnlineHardwareAssertionV1::IosAppAttest(_) => unreachable!(),
                },
            },
        );
        let wrong_key = online_assertion_signing_key(0x63);
        let wrong_signature = android_online_authorization(&registration, &wrong_key);
        for (candidate, candidate_asset) in [
            (cross_account, asset.clone()),
            (cross_device, asset.clone()),
            (cross_asset, asset.clone()),
            (cross_hash, asset.clone()),
            (cross_platform, asset.clone()),
            (wrong_signature, asset.clone()),
            (
                authorization.clone(),
                AssetDefinitionId::derive_from_components(
                    DomainId::try_new("offline", "universal").expect("test domain"),
                    "substituted_cash".parse().expect("test asset name"),
                ),
            ),
        ] {
            assert!(
                ensure_registered_kagemusha_v2_device(
                    &candidate,
                    &candidate_asset,
                    &state_transaction,
                )
                .is_err(),
                "account/device/asset/platform/hash/key substitutions must fail closed",
            );
        }
        let first =
            ensure_registered_kagemusha_v2_device(&authorization, &asset, &state_transaction)
                .expect("first atomic commit plan");
        let stale =
            ensure_registered_kagemusha_v2_device(&authorization, &asset, &state_transaction)
                .expect("concurrent plan from the same unused state");
        commit_kagemusha_online_hardware_assertion(first, &mut state_transaction)
            .expect("first commit wins");
        let error = commit_kagemusha_online_hardware_assertion(stale, &mut state_transaction)
            .expect_err("stale lifecycle compare-and-swap must conflict");
        assert!(error.to_string().contains("hardware_assertion_conflict"));
    }
    #[test]
    fn expired_registration_fails_and_exact_committed_retry_precedes_consumption() {
        offline_test_transaction!(state_transaction);
        let asset = offline_test_asset(&ALICE_ID).definition().clone();
        let assertion_key = online_assertion_signing_key(0x64);
        let registration = android_online_registration(
            &ALICE_ID,
            &asset,
            &assertion_key,
            POLICY_TEST_TIME_MS + 60_000,
        );
        let authorization = android_online_authorization(&registration, &assertion_key);
        install_android_online_registration(&mut state_transaction, registration);
        let replay_markers = match kagemusha_v4_replay_status(&authorization, &state_transaction)
            .expect("fresh request")
        {
            KagemushaV4ReplayStatus::Fresh(markers) => markers,
            KagemushaV4ReplayStatus::Committed => panic!("request unexpectedly committed"),
        };
        let hardware_plan =
            ensure_registered_kagemusha_v2_device(&authorization, &asset, &state_transaction)
                .expect("fresh hardware assertion");
        commit_kagemusha_online_hardware_assertion(hardware_plan, &mut state_transaction)
            .expect("consume hardware assertion");
        commit_kagemusha_v4_replay_markers(replay_markers, &mut state_transaction);
        assert!(matches!(
            kagemusha_v4_replay_status(&authorization, &state_transaction)
                .expect("byte-identical committed retry"),
            KagemushaV4ReplayStatus::Committed,
        ));
        let mut mutated = authorization.clone();
        mutated.expires_at_ms += 1;
        assert!(
            kagemusha_v4_replay_status(&mutated, &state_transaction).is_err(),
            "same operation/nonce/payload with changed authorization bytes must conflict",
        );
        let expired_registration = android_online_registration(
            &ALICE_ID,
            &asset,
            &online_assertion_signing_key(0x65),
            POLICY_TEST_TIME_MS,
        );
        let expired_authorization = android_online_authorization(
            &expired_registration,
            &online_assertion_signing_key(0x65),
        );
        install_android_online_registration(&mut state_transaction, expired_registration);
        assert!(
            ensure_registered_kagemusha_v2_device(
                &expired_authorization,
                &asset,
                &state_transaction,
            )
            .is_err(),
            "an expired exact registration must not authorize a fresh operation",
        );
    }
    fn deliberately_invalid_registration(
        account: &AccountId,
    ) -> OfflineDeviceAttestationRegistration {
        let secret =
            p256::SecretKey::from_slice(&[1_u8; 32]).expect("fixed test scalar must be valid");
        let encoded_public_key = secret.public_key().to_encoded_point(false);
        let public_key = KagemushaDevicePublicKeyV2::from_sec1_bytes(encoded_public_key.as_bytes())
            .expect("derived test public key must be canonical");
        let attestation_report = b"authorization-boundary-report".to_vec();
        let evidence = b"authorization-boundary-evidence".to_vec();
        OfflineDeviceAttestationRegistration {
            // The unsupported version makes validation stop immediately
            // after the authorization boundary.
            version: 0,
            platform: "android-keymint".to_owned(),
            key_id: "authorization-boundary-key".to_owned(),
            device_id: "authorization-boundary-device".to_owned(),
            account_id: account.clone(),
            asset_definition_id: None,
            ios_team_id: None,
            ios_bundle_id: None,
            ios_environment: None,
            android_package_name: None,
            android_signing_certificate_sha256: None,
            public_key,
            assertion_scheme: "android-keymint".to_owned(),
            assertion_key_algorithm: "ecdsa-p256-sha256".to_owned(),
            assertion_public_key: encoded_public_key.as_bytes().to_vec(),
            assertion_usage_count_limit: Some(1),
            one_use: true,
            challenge_hash: Hash::new(b"authorization-boundary-challenge"),
            attestation_report_hash: Hash::new(&attestation_report),
            attestation_report,
            evidence_hash: Hash::new(&evidence),
            evidence,
            recent_block_height: 1,
            recent_block_hash: Hash::new(b"authorization-boundary-block"),
            expires_at_ms: POLICY_TEST_TIME_MS + 60_000,
        }
    }
    #[test]
    fn device_registration_identifier_bounds_are_exact_and_transactional() {
        let mut exact = deliberately_invalid_registration(&ALICE_ID);
        exact.version = 1;
        exact.key_id = "k".repeat(OFFLINE_DEVICE_ATTESTATION_KEY_ID_MAX_BYTES_V1);
        exact.device_id = "d".repeat(OFFLINE_DEVICE_ATTESTATION_DEVICE_ID_MAX_BYTES_V1);
        validate_offline_attestation_registration_identifiers(&exact)
            .expect("exact device and key identifier limits are admitted");

        let mut oversized_device = exact.clone();
        oversized_device.device_id.push('d');
        let mut control_device = exact.clone();
        control_device.device_id.push('\n');
        let mut oversized_key = exact;
        oversized_key.key_id.push('k');

        for (label, registration) in [
            ("oversized device id", oversized_device),
            ("control-character device id", control_device),
            ("oversized key id", oversized_key),
        ] {
            offline_test_transaction!(state_transaction);
            let replay_keys_before = state_transaction.world.kagemusha_replay_keys.iter().count();
            let contract_state_before = state_transaction.world.smart_contract_state.iter().count();

            let error = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&ALICE_ID, &mut state_transaction)
                .expect_err("over-limit registration identifier must be rejected");
            assert!(
                error
                    .to_string()
                    .contains("offline_reason::invalid_attestation"),
                "{label}: unexpected rejection: {error}",
            );
            assert_eq!(
                state_transaction.world.kagemusha_replay_keys.iter().count(),
                replay_keys_before,
                "{label}: rejection mutated replay state",
            );
            assert_eq!(
                state_transaction.world.smart_contract_state.iter().count(),
                contract_state_before,
                "{label}: rejection mutated contract state",
            );
        }
    }
    #[test]
    fn device_registration_optional_metadata_bounds_are_exact() {
        let mut registration = deliberately_invalid_registration(&ALICE_ID);
        registration.ios_team_id =
            Some("T".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TEAM_ID_BYTES_V1));
        registration.ios_bundle_id =
            Some("b".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1));
        registration.ios_environment = Some("production".to_owned());
        registration.android_package_name =
            Some("p".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1));
        validate_offline_attestation_optional_metadata(&registration)
            .expect("exact optional attestation metadata limits are admitted");

        registration
            .ios_team_id
            .as_mut()
            .expect("Team ID fixture")
            .push('T');
        assert!(validate_offline_attestation_optional_metadata(&registration).is_err());
        registration
            .ios_team_id
            .as_mut()
            .expect("Team ID fixture")
            .pop();
        registration
            .android_package_name
            .as_mut()
            .expect("package fixture")
            .push('\n');
        assert!(validate_offline_attestation_optional_metadata(&registration).is_err());
    }
    fn insert_role(
        state_transaction: &mut StateTransaction<'_, '_>,
        role_name: &str,
        grant_to: &AccountId,
        permissions: impl IntoIterator<Item = Permission>,
    ) -> RoleId {
        let role_id: RoleId = role_name.parse().expect("valid offline test role id");
        let mut role = Role::new(role_id.clone(), grant_to.clone());
        for permission in permissions {
            role = role.add_permission(permission);
        }
        let role = role.build(grant_to);
        state_transaction.world.roles.insert(role_id.clone(), role);
        role_id
    }
    fn assign_role(
        state_transaction: &mut StateTransaction<'_, '_>,
        account: &AccountId,
        role_id: RoleId,
    ) {
        state_transaction
            .world
            .account_roles
            .insert(RoleIdWithOwner::new(account.clone(), role_id), ());
    }
    #[derive(Clone, Copy, Debug)]
    enum GrantSource {
        Direct,
        Role,
    }
    fn grant_alice_permission(
        state_transaction: &mut StateTransaction<'_, '_>,
        source: GrantSource,
        permission: Permission,
    ) {
        match source {
            GrantSource::Direct => {
                let _ = state_transaction
                    .world
                    .add_account_permission(&ALICE_ID, permission);
            }
            GrantSource::Role => {
                let role_id = insert_role(
                    state_transaction,
                    "offline_test_manager",
                    &ALICE_ID,
                    [permission],
                );
                assign_role(state_transaction, &ALICE_ID, role_id);
            }
        }
    }
    fn assert_unauthorized(result: Result<(), Error>, context: &str) {
        let error = result.expect_err("offline authorization must fail closed");
        assert!(
            error
                .to_string()
                .contains("offline_reason::unauthorized_controller"),
            "{context}: unexpected offline authorization error: {error}"
        );
    }
    #[test]
    fn exact_offline_escrow_grants_and_self_submission_are_preserved() {
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let state_transaction = block.transaction();
        ensure_can_submit_kagemusha_for_account(&ALICE_ID, &ALICE_ID, &state_transaction)
            .expect("an account must remain able to submit for itself");
        ensure_can_submit_kagemusha_topup(
            &offline_test_asset(&ALICE_ID),
            &ALICE_ID,
            &state_transaction,
        )
        .expect("a payer must remain able to submit its own top-up");
        for source in [GrantSource::Direct, GrantSource::Role] {
            offline_test_transaction!(state_transaction);
            grant_alice_permission(
                &mut state_transaction,
                source,
                offline_permission(CAN_MANAGE_OFFLINE_ESCROW_PERMISSION),
            );
            ensure_can_submit_kagemusha_for_account(&BOB_ID, &ALICE_ID, &state_transaction)
                .unwrap_or_else(|error| {
                    panic!("{source:?} exact permission must authorize delegation: {error}")
                });
            ensure_can_submit_kagemusha_topup(
                &offline_test_asset(&BOB_ID),
                &ALICE_ID,
                &state_transaction,
            )
            .unwrap_or_else(|error| {
                panic!("{source:?} exact permission must authorize delegated top-up: {error}")
            });
        }
    }
    #[derive(Clone, Copy, Debug)]
    enum RejectedRoleState {
        Unassigned,
        AssignedToAnotherAccount,
        RevokedAssignment,
        MissingRoleRecord,
    }
    #[test]
    fn stale_or_unrelated_offline_escrow_roles_fail_closed() {
        for case in [
            RejectedRoleState::Unassigned,
            RejectedRoleState::AssignedToAnotherAccount,
            RejectedRoleState::RevokedAssignment,
            RejectedRoleState::MissingRoleRecord,
        ] {
            offline_test_transaction!(state_transaction);
            let role_id = insert_role(
                &mut state_transaction,
                "offline_escrow_manager",
                &ALICE_ID,
                [offline_permission(CAN_MANAGE_OFFLINE_ESCROW_PERMISSION)],
            );
            match case {
                RejectedRoleState::Unassigned => {}
                RejectedRoleState::AssignedToAnotherAccount => {
                    assign_role(&mut state_transaction, &BOB_ID, role_id);
                }
                RejectedRoleState::RevokedAssignment => {
                    let key = RoleIdWithOwner::new(ALICE_ID.clone(), role_id.clone());
                    assign_role(&mut state_transaction, &ALICE_ID, role_id);
                    assert!(
                        state_transaction.world.account_roles.remove(key).is_some(),
                        "test precondition: assignment must exist before revocation"
                    );
                }
                RejectedRoleState::MissingRoleRecord => {
                    assign_role(&mut state_transaction, &ALICE_ID, role_id.clone());
                    assert!(
                        state_transaction.world.roles.remove(role_id).is_some(),
                        "test precondition: assigned role record must exist before removal"
                    );
                }
            }
            assert_unauthorized(
                ensure_can_submit_kagemusha_for_account(&BOB_ID, &ALICE_ID, &state_transaction),
                &format!("{case:?}"),
            );
        }
    }
    #[test]
    fn same_name_non_unit_permission_payloads_are_rejected() {
        let forged_payloads = [
            ("boolean", Json::new(true)),
            ("string", Json::new("forged-scope")),
            ("array", Json::new(vec![1_u8, 2_u8])),
        ];
        for source in [GrantSource::Direct, GrantSource::Role] {
            for (payload_name, payload) in &forged_payloads {
                offline_test_transaction!(state_transaction);
                grant_alice_permission(
                    &mut state_transaction,
                    source,
                    offline_permission_with_payload(
                        CAN_MANAGE_OFFLINE_ESCROW_PERMISSION,
                        payload.clone(),
                    ),
                );
                assert_unauthorized(
                    ensure_can_submit_kagemusha_for_account(&BOB_ID, &ALICE_ID, &state_transaction),
                    &format!("{source:?} same-name {payload_name} payload"),
                );
            }
        }
    }
    #[test]
    fn only_an_exact_permission_among_multiple_roles_authorizes() {
        offline_test_transaction!(state_transaction);
        for (role_name, permission) in [
            (
                "similarly_named_offline_manager",
                offline_permission("CanManageOfflineEscrowExtra"),
            ),
            (
                "wrong_case_offline_manager",
                offline_permission("canmanageofflineescrow"),
            ),
            (
                "forged_payload_offline_manager",
                offline_permission_with_payload(
                    CAN_MANAGE_OFFLINE_ESCROW_PERMISSION,
                    Json::new(true),
                ),
            ),
        ] {
            let role_id = insert_role(&mut state_transaction, role_name, &ALICE_ID, [permission]);
            assign_role(&mut state_transaction, &ALICE_ID, role_id);
        }
        assert_unauthorized(
            ensure_can_submit_kagemusha_for_account(&BOB_ID, &ALICE_ID, &state_transaction),
            "multiple inexact roles",
        );
        let exact_role = insert_role(
            &mut state_transaction,
            "exact_offline_manager",
            &ALICE_ID,
            [offline_permission(CAN_MANAGE_OFFLINE_ESCROW_PERMISSION)],
        );
        assign_role(&mut state_transaction, &ALICE_ID, exact_role);
        ensure_can_submit_kagemusha_for_account(&BOB_ID, &ALICE_ID, &state_transaction)
            .expect("one exact assigned permission among unrelated roles must authorize");
    }
    #[derive(Clone, Copy, Debug)]
    enum RegistrationBoundaryGrant {
        None,
        ExactRole,
        SameNameNonUnitRole,
    }
    #[test]
    fn delegated_registration_enforces_role_permission_at_execute_boundary() {
        for grant in [
            RegistrationBoundaryGrant::None,
            RegistrationBoundaryGrant::ExactRole,
            RegistrationBoundaryGrant::SameNameNonUnitRole,
        ] {
            offline_test_transaction!(state_transaction);
            match grant {
                RegistrationBoundaryGrant::None => {}
                RegistrationBoundaryGrant::ExactRole => grant_alice_permission(
                    &mut state_transaction,
                    GrantSource::Role,
                    offline_permission(CAN_MANAGE_OFFLINE_ESCROW_PERMISSION),
                ),
                RegistrationBoundaryGrant::SameNameNonUnitRole => grant_alice_permission(
                    &mut state_transaction,
                    GrantSource::Role,
                    offline_permission_with_payload(
                        CAN_MANAGE_OFFLINE_ESCROW_PERMISSION,
                        Json::new(true),
                    ),
                ),
            }
            let replay_keys_before = state_transaction.world.kagemusha_replay_keys.iter().count();
            let error =
                RegisterOfflineDeviceAttestation::new(deliberately_invalid_registration(&BOB_ID))
                    .execute(&ALICE_ID, &mut state_transaction)
                    .expect_err("deliberately invalid registration must not succeed");
            match grant {
                RegistrationBoundaryGrant::ExactRole => assert!(
                    error
                        .to_string()
                        .contains("offline_reason::invalid_attestation"),
                    "exact assigned role must pass authorization before validation: {error}"
                ),
                RegistrationBoundaryGrant::None
                | RegistrationBoundaryGrant::SameNameNonUnitRole => assert!(
                    error
                        .to_string()
                        .contains("offline_reason::unauthorized_controller"),
                    "{grant:?} must fail at the authorization boundary: {error}"
                ),
            }
            assert_eq!(
                state_transaction.world.kagemusha_replay_keys.iter().count(),
                replay_keys_before,
                "{grant:?}: rejected registration mutated replay state"
            );
        }
    }
    #[test]
    fn exact_direct_and_role_policy_manager_permissions_can_update_policy() {
        for source in [GrantSource::Direct, GrantSource::Role] {
            let policy = default_offline_device_attestation_policy()
                .expect("bundled offline attestation policy must decode");
            offline_test_transaction!(state_transaction);
            grant_alice_permission(
                &mut state_transaction,
                source,
                offline_permission(CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION),
            );
            SetOfflineDeviceAttestationPolicy::new(policy.clone())
                .execute(&ALICE_ID, &mut state_transaction)
                .unwrap_or_else(|error| {
                    panic!("{source:?} exact policy permission must authorize: {error}")
                });
            let stored = state_transaction
                .world
                .smart_contract_state
                .get(&*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY)
                .expect("authorized policy update must write state");
            let decoded: OfflineDeviceAttestationPolicy =
                norito::decode_from_bytes(stored).expect("stored policy must decode");
            assert_eq!(decoded, policy, "{source:?} stored the wrong policy");
        }
    }
    #[test]
    fn stored_android_status_anti_rollback_state_cannot_be_removed() {
        let mut baseline = default_offline_device_attestation_policy()
            .expect("bundled offline attestation policy must decode");
        baseline.android_status_snapshot = Some(android_status_snapshot());
        let baseline_bytes = norito::to_bytes(&baseline).expect("baseline policy must encode");
        let mut candidate = baseline.clone();
        candidate.android_status_snapshot = None;

        offline_test_transaction!(state_transaction);
        grant_alice_permission(
            &mut state_transaction,
            GrantSource::Direct,
            offline_permission(CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION),
        );
        state_transaction.world.smart_contract_state.insert(
            (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
            baseline_bytes.clone(),
        );
        let error = SetOfflineDeviceAttestationPolicy::new(candidate)
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("an installed Android status watermark must be retained");
        assert!(
            error
                .to_string()
                .contains("anti-rollback state cannot be removed")
        );
        assert_eq!(
            state_transaction
                .world
                .smart_contract_state
                .get(&*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY),
            Some(&baseline_bytes),
        );
    }
    #[derive(Clone, Copy, Debug)]
    enum RejectedPolicyUpdate {
        NoPermission,
        SimilarPermissionName,
        SameNameNonUnitDirectPayload,
        SameNameNonUnitRolePayload,
        UnsupportedVersion,
        MissingTrustedRoots,
        OversizedRevocationList,
    }
    #[test]
    fn rejected_policy_updates_never_mutate_existing_policy() {
        for case in [
            RejectedPolicyUpdate::NoPermission,
            RejectedPolicyUpdate::SimilarPermissionName,
            RejectedPolicyUpdate::SameNameNonUnitDirectPayload,
            RejectedPolicyUpdate::SameNameNonUnitRolePayload,
            RejectedPolicyUpdate::UnsupportedVersion,
            RejectedPolicyUpdate::MissingTrustedRoots,
            RejectedPolicyUpdate::OversizedRevocationList,
        ] {
            let baseline = default_offline_device_attestation_policy()
                .expect("bundled offline attestation policy must decode");
            let baseline_bytes = norito::to_bytes(&baseline).expect("baseline policy must encode");
            let mut candidate = baseline.clone();
            candidate
                .revoked_certificate_tbs_sha256
                .push(vec![0xA5_u8; 32]);
            offline_test_transaction!(state_transaction);
            state_transaction.world.smart_contract_state.insert(
                (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
                baseline_bytes.clone(),
            );
            let expected_reason = match case {
                RejectedPolicyUpdate::NoPermission => "unauthorized_controller",
                RejectedPolicyUpdate::SimilarPermissionName => {
                    state_transaction.world.add_account_permission(
                        &ALICE_ID,
                        offline_permission("CanManageOfflineDeviceAttestationPolicyAdditional"),
                    );
                    "unauthorized_controller"
                }
                RejectedPolicyUpdate::SameNameNonUnitDirectPayload => {
                    grant_alice_permission(
                        &mut state_transaction,
                        GrantSource::Direct,
                        offline_permission_with_payload(
                            CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION,
                            Json::new(true),
                        ),
                    );
                    "unauthorized_controller"
                }
                RejectedPolicyUpdate::SameNameNonUnitRolePayload => {
                    grant_alice_permission(
                        &mut state_transaction,
                        GrantSource::Role,
                        offline_permission_with_payload(
                            CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION,
                            Json::new("forged-scope"),
                        ),
                    );
                    "unauthorized_controller"
                }
                RejectedPolicyUpdate::UnsupportedVersion => {
                    grant_alice_permission(
                        &mut state_transaction,
                        GrantSource::Direct,
                        offline_permission(CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION),
                    );
                    candidate.version = 2;
                    "invalid_attestation_policy"
                }
                RejectedPolicyUpdate::MissingTrustedRoots => {
                    grant_alice_permission(
                        &mut state_transaction,
                        GrantSource::Role,
                        offline_permission(CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION),
                    );
                    candidate.trusted_roots.clear();
                    "invalid_attestation_policy"
                }
                RejectedPolicyUpdate::OversizedRevocationList => {
                    grant_alice_permission(
                        &mut state_transaction,
                        GrantSource::Direct,
                        offline_permission(CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION),
                    );
                    candidate.revoked_certificate_tbs_sha256 = vec![
                        vec![0xA5; 32];
                        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1
                            + 1
                    ];
                    "invalid_attestation_policy"
                }
            };
            let error = SetOfflineDeviceAttestationPolicy::new(candidate)
                .execute(&ALICE_ID, &mut state_transaction)
                .expect_err("adversarial policy update must be rejected");
            assert!(
                error.to_string().contains(expected_reason),
                "{case:?}: unexpected policy rejection: {error}"
            );
            assert_eq!(
                state_transaction
                    .world
                    .smart_contract_state
                    .get(&*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY),
                Some(&baseline_bytes),
                "{case:?}: rejected update mutated the stored policy"
            );
        }
    }
    #[test]
    fn offline_escrow_manager_permission_is_exact_directly_and_through_roles() {
        offline_test_transaction!(state_transaction);
        let wrong = offline_permission_with_payload(
            CAN_MANAGE_OFFLINE_ESCROW_PERMISSION,
            Json::new("wildcard"),
        );
        grant_alice_permission(&mut state_transaction, GrantSource::Direct, wrong.clone());
        grant_alice_permission(&mut state_transaction, GrantSource::Role, wrong.clone());
        assert!(
            !is_offline_escrow_manager(&ALICE_ID, &state_transaction),
            "matching names with non-canonical payloads must not authorize escrow control"
        );
        grant_alice_permission(
            &mut state_transaction,
            GrantSource::Direct,
            offline_escrow_manager_permission(),
        );
        assert!(
            is_offline_escrow_manager(&ALICE_ID, &state_transaction),
            "the exact manager permission granted directly must authorize escrow control"
        );
        state_transaction
            .world
            .account_permissions
            .insert(ALICE_ID.clone(), [wrong].into_iter().collect());
        grant_alice_permission(
            &mut state_transaction,
            GrantSource::Role,
            offline_escrow_manager_permission(),
        );
        assert!(
            is_offline_escrow_manager(&ALICE_ID, &state_transaction),
            "the exact manager permission inherited through a role must authorize escrow control"
        );
    }
    #[test]
    fn attestation_policy_manager_permission_is_exact_and_inherited_from_role() {
        offline_test_transaction!(state_transaction);
        grant_alice_permission(
            &mut state_transaction,
            GrantSource::Role,
            offline_permission_with_payload(
                CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION,
                Json::new("wildcard"),
            ),
        );
        assert!(
            !can_manage_offline_device_attestation_policy(&state_transaction, &ALICE_ID),
            "a matching name with a non-canonical payload must not authorize policy changes"
        );
        grant_alice_permission(
            &mut state_transaction,
            GrantSource::Role,
            offline_device_attestation_policy_manager_permission(),
        );
        assert!(
            can_manage_offline_device_attestation_policy(&state_transaction, &ALICE_ID),
            "the exact manager permission inherited through a role must authorize policy changes"
        );
    }
    include!("isi_kagemusha_taira_canary_context_tests.rs");
    include!("isi_platform_policy_tests.rs");
}
