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
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        ChainId, NetworkId, Registrable,
        account::Account,
        asset::{Asset, AssetDefinition, AssetDefinitionId},
        block::BlockHeader,
        domain::{Domain, DomainId},
        isi::{
            SetAssetHoldingLimit,
            error::{AssetTransferAdmissionError, InstructionExecutionError},
        },
        offline::{
            KagemushaAndroidKeyMintHardwareAssertionV1, KagemushaDevicePublicKeyV2,
            KagemushaDeviceSignatureV2, KagemushaIosAppAttestHardwareAssertionV1,
        },
        permission::Permission,
        role::{Role, RoleId},
    };
    use iroha_primitives::{json::Json, numeric::Quantity};
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use p256::{
        ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _},
        elliptic_curve::sec1::ToEncodedPoint as _,
    };
    const POLICY_TEST_TIME_MS: u64 = 1_800_000_000_000;
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
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut transaction = block.transaction();
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
                "authenticate_registered_kagemusha_v2_device",
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
                .find("impl Execute for ActivateKagemushaRecursiveReleaseV4")
                .expect("V4 release activation executor");
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
    fn kagemusha_v4_activation_validates_bounded_policy_before_state_mutation() {
        let source = include_str!("../offline.rs");
        let start = source
            .find("impl Execute for ActivateKagemushaRecursiveReleaseV4")
            .expect("V4 release activation executor");
        let body = &source[start..];
        let validation = body
            .find("validate_offline_attestation_policy_for_release_activation")
            .expect("activation policy validation");
        let first_mutation = body
            .find("state_transaction.world.smart_contract_state.insert")
            .expect("activation state publication");

        assert!(
            validation < first_mutation,
            "an over-limit activation policy must be rejected before any state publication",
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
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
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
    fn release_activation_device_policy() -> OfflineDeviceAttestationPolicy {
        let mut policy = default_offline_device_attestation_policy()
            .expect("built-in roots form a valid activation-policy template");
        policy.require_ios_app_policy = true;
        policy.require_android_app_policy = true;
        policy.ios_apps = vec![ios_assertion_policy()];
        policy.android_apps = vec![OfflineAndroidAppAttestationPolicy {
            package_name: "com.pk.retailwallet".to_owned(),
            signing_certificate_sha256: vec![vec![0x55; 32]],
        }];
        policy
    }
    #[test]
    fn release_activation_device_policy_is_production_and_fail_closed() {
        let policy = release_activation_device_policy();
        validate_offline_attestation_policy_for_release_activation(&policy, 0)
            .expect("exact production policy must be activation-eligible");
        let mut missing_android_gate = policy.clone();
        missing_android_gate.require_android_app_policy = false;
        assert!(
            validate_offline_attestation_policy_for_release_activation(&missing_android_gate, 0,)
                .is_err(),
            "activation must not publish an Android fail-open policy",
        );
        let mut development_ios = policy.clone();
        development_ios.ios_apps[0].environment = "development".to_owned();
        assert!(
            validate_offline_attestation_policy_for_release_activation(&development_ios, 0)
                .is_err(),
            "activation must not publish a development App Attest policy",
        );
        let mut control_character_ios = policy.clone();
        control_character_ios.ios_apps[0].bundle_id = "io.soramitsu.\npk".to_owned();
        assert!(
            validate_offline_attestation_policy_for_release_activation(&control_character_ios, 0,)
                .is_err(),
            "activation must reject control characters in application identities",
        );
        let mut maximum_revocations = policy;
        maximum_revocations.revoked_certificate_sha256 = (1
            ..=OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1)
            .map(|index| {
                let mut digest = [0_u8; 32];
                digest[..8].copy_from_slice(&(index as u64).to_le_bytes());
                digest.to_vec()
            })
            .collect();
        validate_offline_attestation_policy_for_release_activation(&maximum_revocations, 0)
            .expect("the exact revocation-list limit remains activation eligible");
        maximum_revocations
            .revoked_certificate_sha256
            .push(vec![0xA5; 32]);
        assert!(
            validate_offline_attestation_policy_for_release_activation(&maximum_revocations, 0)
                .is_err(),
            "activation must reject a policy above the revocation-list limit",
        );
    }
    #[test]
    fn production_device_policy_constructor_binds_explicit_apps_and_builtin_roots() {
        let policy = production_offline_device_attestation_policy_v1(
            "TEAMID1234".to_owned(),
            "io.soramitsu.pk".to_owned(),
            vec![10, 4],
            vec!["42".to_owned(), "41".to_owned()],
            "com.pk.retailwallet".to_owned(),
            vec![[0x66; 32], [0x55; 32]],
            1_800_000_000_000,
        )
        .expect("explicit production app identities should build a fail-closed policy");
        assert_eq!(policy.trusted_roots.len(), 3);
        assert!(policy.require_ios_app_policy);
        assert!(policy.require_android_app_policy);
        assert_eq!(
            policy.ios_apps[0].allowed_validation_categories,
            vec![4, 10]
        );
        assert_eq!(
            policy.ios_apps[0].allowed_bundle_versions,
            vec!["41".to_owned(), "42".to_owned()]
        );
        assert_eq!(
            policy.android_apps[0].signing_certificate_sha256,
            vec![vec![0x55; 32], vec![0x66; 32]]
        );
    }
    #[test]
    fn production_device_policy_constructor_rejects_duplicate_operator_input() {
        let error = production_offline_device_attestation_policy_v1(
            "TEAMID1234".to_owned(),
            "io.soramitsu.pk".to_owned(),
            vec![4, 4],
            vec!["42".to_owned()],
            "com.pk.retailwallet".to_owned(),
            vec![[0x55; 32]],
            1_800_000_000_000,
        )
        .expect_err("duplicate policy input must not be silently normalized");
        assert!(error.contains("must not contain duplicates"));
    }
    #[test]
    fn offline_device_attestation_policy_shape_bounds_are_exact() {
        let baseline = default_offline_device_attestation_policy()
            .expect("built-in roots form a valid policy template");

        let mut roots = baseline.clone();
        roots.trusted_roots = (0..OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_V1)
            .map(|index| baseline.trusted_roots[index % 2].clone())
            .collect();
        validate_offline_attestation_policy_bounds(&roots)
            .expect("the exact total and per-platform root limits are admitted");
        roots.trusted_roots.push(baseline.trusted_roots[0].clone());
        assert!(validate_offline_attestation_policy_bounds(&roots).is_err());

        let mut platform_roots = baseline.clone();
        platform_roots.trusted_roots = vec![
            baseline.trusted_roots[0].clone();
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_PER_PLATFORM_V1
        ];
        validate_offline_attestation_policy_bounds(&platform_roots)
            .expect("the exact per-platform root limit is admitted");
        platform_roots
            .trusted_roots
            .push(baseline.trusted_roots[0].clone());
        assert!(validate_offline_attestation_policy_bounds(&platform_roots).is_err());

        let mut root_der = baseline.clone();
        root_der.trusted_roots = vec![baseline.trusted_roots[0].clone()];
        root_der.trusted_roots[0].der =
            vec![0xA5; OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1];
        validate_offline_attestation_policy_bounds(&root_der)
            .expect("the exact trusted-root DER limit is admitted");
        root_der.trusted_roots[0].der.push(0xA5);
        assert!(validate_offline_attestation_policy_bounds(&root_der).is_err());

        let mut revoked = baseline.clone();
        revoked.revoked_certificate_sha256 =
            vec![vec![0xA5; 32]; OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1];
        validate_offline_attestation_policy_bounds(&revoked)
            .expect("the exact revocation-list limit is admitted");
        revoked.revoked_certificate_sha256.push(vec![0x5A; 32]);
        assert!(validate_offline_attestation_policy_bounds(&revoked).is_err());

        let ios_app = ios_assertion_policy();
        let mut ios_apps = baseline.clone();
        ios_apps.ios_apps =
            vec![ios_app.clone(); OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_APPS_V1];
        validate_offline_attestation_policy_bounds(&ios_apps)
            .expect("the exact iOS app-count limit is admitted");
        ios_apps.ios_apps.push(ios_app.clone());
        assert!(validate_offline_attestation_policy_bounds(&ios_apps).is_err());

        let android_app = OfflineAndroidAppAttestationPolicy {
            package_name: "com.example.boundary".to_owned(),
            signing_certificate_sha256: vec![vec![0x5A; 32]],
        };
        let mut android_apps = baseline.clone();
        android_apps.android_apps =
            vec![android_app.clone(); OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_APPS_V1];
        validate_offline_attestation_policy_bounds(&android_apps)
            .expect("the exact Android app-count limit is admitted");
        android_apps.android_apps.push(android_app.clone());
        assert!(validate_offline_attestation_policy_bounds(&android_apps).is_err());

        let mut ios_nested = baseline.clone();
        ios_nested.ios_apps = vec![ios_app];
        ios_nested.ios_apps[0].team_id =
            "T".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TEAM_ID_BYTES_V1);
        ios_nested.ios_apps[0].bundle_id =
            "b".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1);
        ios_nested.ios_apps[0].allowed_validation_categories = vec![1, 2, 3, 4, 5, 6, 10];
        ios_nested.ios_apps[0].allowed_bundle_versions = (0
            ..OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSIONS_V1)
            .map(|index| index.to_string())
            .collect();
        ios_nested.ios_apps[0].allowed_bundle_versions[0] =
            "v".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSION_BYTES_V1);
        validate_offline_attestation_policy_bounds(&ios_nested)
            .expect("the exact nested iOS limits are admitted");
        ios_nested.ios_apps[0].team_id.push('T');
        assert!(validate_offline_attestation_policy_bounds(&ios_nested).is_err());
        ios_nested.ios_apps[0].team_id.pop();
        ios_nested.ios_apps[0].bundle_id.push('b');
        assert!(validate_offline_attestation_policy_bounds(&ios_nested).is_err());
        ios_nested.ios_apps[0].bundle_id.pop();
        ios_nested.ios_apps[0]
            .allowed_validation_categories
            .push(10);
        assert!(validate_offline_attestation_policy_bounds(&ios_nested).is_err());
        ios_nested.ios_apps[0].allowed_validation_categories.pop();
        ios_nested.ios_apps[0]
            .allowed_bundle_versions
            .push("overflow".to_owned());
        assert!(validate_offline_attestation_policy_bounds(&ios_nested).is_err());
        ios_nested.ios_apps[0].allowed_bundle_versions.pop();
        ios_nested.ios_apps[0].allowed_bundle_versions[0].push('v');
        assert!(validate_offline_attestation_policy_bounds(&ios_nested).is_err());

        let mut android_nested = baseline.clone();
        android_nested.android_apps = vec![android_app];
        android_nested.android_apps[0].package_name =
            "p".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1);
        android_nested.android_apps[0].signing_certificate_sha256 = vec![
            vec![0x5A; 32];
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_SIGNING_CERTIFICATES_V1
        ];
        validate_offline_attestation_policy_bounds(&android_nested)
            .expect("the exact nested Android limits are admitted");
        android_nested.android_apps[0].package_name.push('p');
        assert!(validate_offline_attestation_policy_bounds(&android_nested).is_err());
        android_nested.android_apps[0].package_name.pop();
        android_nested.android_apps[0]
            .signing_certificate_sha256
            .push(vec![0xA5; 32]);
        assert!(validate_offline_attestation_policy_bounds(&android_nested).is_err());

        let mut canonical = baseline;
        canonical.trusted_roots = vec![canonical.trusted_roots[0].clone(); 4];
        for root in &mut canonical.trusted_roots {
            root.der = vec![0xA5; OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1];
        }
        while norito::encode_canonical(&canonical)
            .expect("boundary policy encodes")
            .len()
            > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1
        {
            canonical.trusted_roots[3]
                .der
                .pop()
                .expect("four maximum roots exceed the canonical policy limit");
        }
        assert_eq!(
            norito::encode_canonical(&canonical)
                .expect("exact-boundary policy encodes")
                .len(),
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1,
        );
        validate_offline_attestation_policy_bounds(&canonical)
            .expect("the exact canonical policy limit is admitted");
        canonical.trusted_roots[3].der.push(0xA5);
        assert!(validate_offline_attestation_policy_bounds(&canonical).is_err());
    }
    #[test]
    fn release_activation_authority_requires_both_exact_governance_permissions() {
        fn authorization_result(permissions: Vec<Permission>) -> Result<(), Error> {
            let state = offline_test_state();
            let mut block = state.block(offline_test_header());
            let mut state_transaction = block.transaction();
            for permission in permissions {
                grant_permission(
                    &mut state_transaction,
                    &ALICE_ID,
                    GrantSource::Direct,
                    permission,
                );
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
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
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
    fn android_key_description_usage_count_fixture(
        software_usage_count_limit: bool,
        hardware_usage_count_limit: bool,
    ) -> Vec<u8> {
        fn authorization_list(with_usage_count_limit: bool) -> Vec<u8> {
            let body = if with_usage_count_limit {
                let one = test_der_tlv(&[0x02], &[1]);
                // Context-specific constructed high tag [405].
                test_der_tlv(&[0xBF, 0x83, 0x15], &one)
            } else {
                Vec::new()
            };
            test_der_tlv(&[0x30], &body)
        }
        let mut body = Vec::new();
        body.extend_from_slice(&test_der_tlv(&[0x02], &[3]));
        body.extend_from_slice(&test_der_tlv(&[0x0A], &[1]));
        body.extend_from_slice(&test_der_tlv(&[0x02], &[4]));
        body.extend_from_slice(&test_der_tlv(&[0x0A], &[1]));
        body.extend_from_slice(&test_der_tlv(&[0x04], &[0xA5]));
        body.extend_from_slice(&test_der_tlv(&[0x04], &[]));
        body.extend_from_slice(&authorization_list(software_usage_count_limit));
        body.extend_from_slice(&authorization_list(hardware_usage_count_limit));
        test_der_tlv(&[0x30], &body)
    }
    #[test]
    fn android_usage_count_limit_must_be_hardware_enforced() {
        let hardware = parse_android_key_description(&android_key_description_usage_count_fixture(
            false, true,
        ))
        .expect("hardware-enforced usageCountLimit is admitted");
        assert_eq!(hardware.usage_count_limit, Some(1));
        assert!(
            parse_android_key_description(&android_key_description_usage_count_fixture(
                true, false,
            ))
            .is_err(),
            "a software-only usageCountLimit must not satisfy the hardware one-use profile",
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
        policy.android_apps = vec![OfflineAndroidAppAttestationPolicy {
            package_name: "com.pk.retailwallet".to_owned(),
            signing_certificate_sha256: vec![vec![0x55; 32]],
        }];
        state_transaction.world.smart_contract_state.insert(
            (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
            norito::to_bytes(&policy).expect("canonical test policy"),
        );
        let registration_hash = canonical_registration_hash(&registration)
            .map(|hash| exact_hash_bytes(&hash))
            .expect("canonical registration hash");
        let state_key = kagemusha_online_registration_state_key(&registration_hash)
            .expect("canonical registration state key");
        let state = KagemushaOnlineRegistrationStateV3 {
            version: 3,
            admission_policy_hash: canonical_offline_device_attestation_policy_hash(&policy)
                .expect("canonical policy hash"),
            admission_height: state_transaction.block_height(),
            admission_transaction_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"test-device-registration-transaction",
            )),
            registration,
            lifecycle: KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintUnused,
        };
        state_transaction.world.smart_contract_state.insert(
            state_key.clone(),
            norito::to_bytes(&state).expect("canonical online registration state"),
        );
        state_key
    }
    fn capacity_registration(
        account: &AccountId,
        index: usize,
        expires_at_ms: u64,
    ) -> OfflineDeviceAttestationRegistration {
        let asset = offline_test_asset(account);
        let assertion_key = online_assertion_signing_key(0x71);
        let mut registration = android_online_registration(
            account,
            asset.definition(),
            &assertion_key,
            expires_at_ms,
        );
        let discriminator = u64::try_from(index)
            .expect("capacity fixture index fits u64")
            .to_be_bytes();
        registration.device_id = format!("capacity-device-{index:05}");
        registration.challenge_hash = Hash::new(discriminator);
        registration.attestation_report = [b"capacity-report:".as_slice(), &discriminator].concat();
        registration.attestation_report_hash = Hash::new(&registration.attestation_report);
        registration.evidence = [b"capacity-evidence:".as_slice(), &discriminator].concat();
        registration.evidence_hash = Hash::new(&registration.evidence);
        registration.recent_block_hash = Hash::new(
            [b"capacity-recent-block:".as_slice(), &discriminator]
                .concat(),
        );
        registration
    }
    fn install_capacity_registration(
        state_transaction: &mut StateTransaction<'_, '_>,
        account: &AccountId,
        index: usize,
        expires_at_ms: u64,
    ) -> (StatePath, [Hash; 4]) {
        let registration = capacity_registration(account, index, expires_at_ms);
        let registration_hash = canonical_registration_hash(&registration)
            .expect("canonical capacity registration hash");
        let replay_keys =
            kagemusha_registration_replay_keys(&registration, &registration_hash);
        let state_key = install_android_online_registration(state_transaction, registration);
        for replay_key in replay_keys {
            assert!(
                state_transaction
                    .world
                    .kagemusha_replay_keys
                    .insert(replay_key, ())
                    .is_none(),
                "capacity fixture replay material must be unique",
            );
        }
        (state_key, replay_keys)
    }
    #[test]
    fn online_registration_capacity_bounds_are_exact() {
        assert_eq!(
            validate_kagemusha_online_registration_capacity_v1(
                KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_GLOBAL_V1,
                1,
            ),
            Ok(()),
            "the exact global registration limit must remain valid",
        );
        assert_eq!(
            validate_kagemusha_online_registration_capacity_v1(
                KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_GLOBAL_V1 + 1,
                1,
            ),
            Err(KagemushaOnlineRegistrationCapacityErrorV1::Global),
            "one registration above the global limit must fail",
        );
        assert_eq!(
            validate_kagemusha_online_registration_capacity_v1(
                KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1,
                KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1,
            ),
            Ok(()),
            "the exact per-account registration limit must remain valid",
        );
        assert_eq!(
            validate_kagemusha_online_registration_capacity_v1(
                KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1 + 1,
                KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1 + 1,
            ),
            Err(KagemushaOnlineRegistrationCapacityErrorV1::Account),
            "one registration above the per-account limit must fail",
        );
    }
    #[test]
    fn per_account_registration_capacity_rejection_does_not_mutate_state() {
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
        let expires_at_ms = POLICY_TEST_TIME_MS + 60_000;
        for index in 0..KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1 - 1 {
            install_capacity_registration(
                &mut state_transaction,
                &ALICE_ID,
                index,
                expires_at_ms,
            );
        }
        let candidate = capacity_registration(
            &ALICE_ID,
            KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1,
            expires_at_ms,
        );
        let policy_hash = current_offline_device_attestation_policy_from_world(
            &state_transaction.world,
            POLICY_TEST_TIME_MS,
        )
        .expect("capacity fixture policy is valid")
        .1;
        plan_kagemusha_online_registration_admission_v1(
            &candidate,
            policy_hash,
            &state_transaction,
        )
        .expect("the candidate reaching the exact per-account limit must remain valid");

        install_capacity_registration(
            &mut state_transaction,
            &ALICE_ID,
            KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1 - 1,
            expires_at_ms,
        );
        let state_before = state_transaction
            .world
            .smart_contract_state
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        let replay_before = state_transaction
            .world
            .kagemusha_replay_keys
            .iter()
            .map(|(key, ())| *key)
            .collect::<Vec<_>>();
        let error = plan_kagemusha_online_registration_admission_v1(
            &candidate,
            policy_hash,
            &state_transaction,
        )
        .expect_err("one registration above the per-account limit must fail");
        assert!(
            error
                .to_string()
                .contains("offline_reason::registration_capacity_exceeded"),
            "unexpected capacity rejection: {error}",
        );
        assert_eq!(
            state_transaction
                .world
                .smart_contract_state
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>(),
            state_before,
            "capacity rejection must not mutate registration state",
        );
        assert_eq!(
            state_transaction
                .world
                .kagemusha_replay_keys
                .iter()
                .map(|(key, ())| *key)
                .collect::<Vec<_>>(),
            replay_before,
            "capacity rejection must not mutate replay protection",
        );
    }
    #[test]
    fn successful_registration_plan_prunes_expired_archive_and_replay_markers() {
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
        let (expired_state_key, expired_replay_keys) = install_capacity_registration(
            &mut state_transaction,
            &ALICE_ID,
            0,
            POLICY_TEST_TIME_MS,
        );
        let candidate = capacity_registration(&ALICE_ID, 1, POLICY_TEST_TIME_MS + 60_000);
        let policy_hash = current_offline_device_attestation_policy_from_world(
            &state_transaction.world,
            POLICY_TEST_TIME_MS,
        )
        .expect("capacity fixture policy is valid")
        .1;
        let plan = plan_kagemusha_online_registration_admission_v1(
            &candidate,
            policy_hash,
            &state_transaction,
        )
        .expect("expired registration must release capacity");
        assert_eq!(plan.expired.len(), 1);
        plan.commit(&mut state_transaction);
        assert!(
            state_transaction
                .world
                .smart_contract_state
                .get(&expired_state_key)
                .is_none(),
            "expired registration archive must be pruned",
        );
        for replay_key in expired_replay_keys {
            assert!(
                state_transaction
                    .world
                    .kagemusha_replay_keys
                    .get(&replay_key)
                    .is_none(),
                "expired replay marker must be pruned",
            );
        }
    }
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
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
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
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
        let (asset, authorization, wrong_signature, state_key) =
            committed_android_replay_fixture(&mut state_transaction);
        let unauthorized = authenticate_kagemusha_v4_redeem_submission_before_replay(
            &ALICE_ID,
            asset.definition(),
            &BOB_ID,
            &authorization,
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
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
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
        assert_eq!(resolved.registration, registration);
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
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
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
        let consumed: KagemushaOnlineRegistrationStateV3 = norito::decode_from_bytes(
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
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
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
        rotated.revoked_certificate_sha256.push(vec![0xA7; 32]);
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
    #[test]
    fn legacy_registration_state_without_policy_hash_fails_closed() {
        #[derive(Encode)]
        struct LegacyRegistrationStateV1 {
            version: u16,
            registration: OfflineDeviceAttestationRegistration,
            lifecycle: KagemushaOnlineHardwareAssertionLifecycleV1,
        }
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
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
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
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
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
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
            let state = offline_test_state();
            let mut block = state.block(offline_test_header());
            let mut state_transaction = block.transaction();
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
    fn grant_permission(
        state_transaction: &mut StateTransaction<'_, '_>,
        account: &AccountId,
        source: GrantSource,
        permission: Permission,
    ) {
        match source {
            GrantSource::Direct => {
                let _ = state_transaction
                    .world
                    .add_account_permission(account, permission);
            }
            GrantSource::Role => {
                let role_id = insert_role(
                    state_transaction,
                    "offline_test_manager",
                    account,
                    [permission],
                );
                assign_role(state_transaction, account, role_id);
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
            let state = offline_test_state();
            let mut block = state.block(offline_test_header());
            let mut state_transaction = block.transaction();
            grant_permission(
                &mut state_transaction,
                &ALICE_ID,
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
            let state = offline_test_state();
            let mut block = state.block(offline_test_header());
            let mut state_transaction = block.transaction();
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
                let state = offline_test_state();
                let mut block = state.block(offline_test_header());
                let mut state_transaction = block.transaction();
                grant_permission(
                    &mut state_transaction,
                    &ALICE_ID,
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
        let state = offline_test_state();
        let mut block = state.block(offline_test_header());
        let mut state_transaction = block.transaction();
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
            let state = offline_test_state();
            let mut block = state.block(offline_test_header());
            let mut state_transaction = block.transaction();
            match grant {
                RegistrationBoundaryGrant::None => {}
                RegistrationBoundaryGrant::ExactRole => grant_permission(
                    &mut state_transaction,
                    &ALICE_ID,
                    GrantSource::Role,
                    offline_permission(CAN_MANAGE_OFFLINE_ESCROW_PERMISSION),
                ),
                RegistrationBoundaryGrant::SameNameNonUnitRole => grant_permission(
                    &mut state_transaction,
                    &ALICE_ID,
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
            let state = offline_test_state();
            let mut block = state.block(offline_test_header());
            let mut state_transaction = block.transaction();
            grant_permission(
                &mut state_transaction,
                &ALICE_ID,
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
            candidate.revoked_certificate_sha256.push(vec![0xA5_u8; 32]);
            let state = offline_test_state();
            let mut block = state.block(offline_test_header());
            let mut state_transaction = block.transaction();
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
                    grant_permission(
                        &mut state_transaction,
                        &ALICE_ID,
                        GrantSource::Direct,
                        offline_permission_with_payload(
                            CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION,
                            Json::new(true),
                        ),
                    );
                    "unauthorized_controller"
                }
                RejectedPolicyUpdate::SameNameNonUnitRolePayload => {
                    grant_permission(
                        &mut state_transaction,
                        &ALICE_ID,
                        GrantSource::Role,
                        offline_permission_with_payload(
                            CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION,
                            Json::new("forged-scope"),
                        ),
                    );
                    "unauthorized_controller"
                }
                RejectedPolicyUpdate::UnsupportedVersion => {
                    grant_permission(
                        &mut state_transaction,
                        &ALICE_ID,
                        GrantSource::Direct,
                        offline_permission(CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION),
                    );
                    candidate.version = 2;
                    "invalid_attestation_policy"
                }
                RejectedPolicyUpdate::MissingTrustedRoots => {
                    grant_permission(
                        &mut state_transaction,
                        &ALICE_ID,
                        GrantSource::Role,
                        offline_permission(CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION),
                    );
                    candidate.trusted_roots.clear();
                    "invalid_attestation_policy"
                }
                RejectedPolicyUpdate::OversizedRevocationList => {
                    grant_permission(
                        &mut state_transaction,
                        &ALICE_ID,
                        GrantSource::Direct,
                        offline_permission(CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION),
                    );
                    candidate.revoked_certificate_sha256 = vec![
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
        let key_pair = KeyPair::try_from_seed(vec![0x52; 32], Algorithm::Ed25519)
            .expect("derive offline escrow manager fixture keypair");
        let authority = AccountId::new(key_pair.public_key().clone());
        let role_id: RoleId = "OFFLINE_ESCROW_MANAGER".parse().expect("role id");
        let wrong_direct = Permission::new(
            CAN_MANAGE_OFFLINE_ESCROW_PERMISSION.into(),
            iroha_primitives::json::Json::new("wildcard"),
        );
        let wrong_role = Role::new(role_id.clone(), authority.clone())
            .add_permission(wrong_direct.clone())
            .build(&authority);
        let mut world = World::default();
        world.account_permissions.insert(
            authority.clone(),
            [wrong_direct.clone()].into_iter().collect(),
        );
        world.roles.insert(role_id.clone(), wrong_role);
        world
            .account_roles
            .insert(RoleIdWithOwner::new(authority.clone(), role_id.clone()), ());
        let state = State::new(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut state_transaction = block.transaction();
        assert!(
            !is_offline_escrow_manager(&authority, &state_transaction),
            "matching names with non-canonical payloads must not authorize escrow control"
        );
        state_transaction.world.account_permissions.insert(
            authority.clone(),
            [offline_escrow_manager_permission()].into_iter().collect(),
        );
        assert!(
            is_offline_escrow_manager(&authority, &state_transaction),
            "the exact manager permission granted directly must authorize escrow control"
        );
        state_transaction
            .world
            .account_permissions
            .insert(authority.clone(), [wrong_direct].into_iter().collect());
        let exact_role = Role::new(role_id.clone(), authority.clone())
            .add_permission(offline_escrow_manager_permission())
            .build(&authority);
        state_transaction.world.roles.insert(role_id, exact_role);
        assert!(
            is_offline_escrow_manager(&authority, &state_transaction),
            "the exact manager permission inherited through a role must authorize escrow control"
        );
    }
    #[test]
    fn attestation_policy_manager_permission_is_exact_and_inherited_from_role() {
        let key_pair = KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519)
            .expect("derive offline policy manager fixture keypair");
        let authority = AccountId::new(key_pair.public_key().clone());
        let role_id: RoleId = "OFFLINE_ATTESTATION_POLICY_MANAGER"
            .parse()
            .expect("role id");
        let wrong_payload = Permission::new(
            CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION.into(),
            iroha_primitives::json::Json::new("wildcard"),
        );
        let role = Role::new(role_id.clone(), authority.clone())
            .add_permission(wrong_payload)
            .build(&authority);
        let mut world = World::default();
        world.roles.insert(role_id.clone(), role);
        world
            .account_roles
            .insert(RoleIdWithOwner::new(authority.clone(), role_id.clone()), ());
        let state = State::new(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut state_transaction = block.transaction();
        assert!(
            !can_manage_offline_device_attestation_policy(&state_transaction, &authority),
            "a matching name with a non-canonical payload must not authorize policy changes"
        );
        let exact = offline_device_attestation_policy_manager_permission();
        let role = Role::new(role_id.clone(), authority.clone())
            .add_permission(exact)
            .build(&authority);
        state_transaction.world.roles.insert(role_id, role);
        assert!(
            can_manage_offline_device_attestation_policy(&state_transaction, &authority),
            "the exact manager permission inherited through a role must authorize policy changes"
        );
    }
    fn ios_assertion_extension_bytes(bundle_version: &str, validation_category: u32) -> Vec<u8> {
        let value = ciborium::value::Value::Map(vec![
            (
                ciborium::value::Value::Text("bundleVersion".to_owned()),
                ciborium::value::Value::Text(bundle_version.to_owned()),
            ),
            (
                ciborium::value::Value::Text("validationCategory".to_owned()),
                ciborium::value::Value::Integer(validation_category.into()),
            ),
        ]);
        let mut encoded = Vec::new();
        ciborium::ser::into_writer(&value, &mut encoded)
            .expect("encode App Attest assertion extensions");
        encoded
    }
    fn ios_assertion_auth_data(
        rp_id_hash: [u8; 32],
        flags: u8,
        sign_count: u32,
        extension_bytes: &[u8],
    ) -> Vec<u8> {
        let mut auth_data = Vec::with_capacity(
            KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_FIXED_HEADER_BYTES_V1
                + extension_bytes.len(),
        );
        auth_data.extend_from_slice(&rp_id_hash);
        auth_data.push(flags);
        auth_data.extend_from_slice(&sign_count.to_be_bytes());
        auth_data.extend_from_slice(extension_bytes);
        auth_data
    }
    fn ios_assertion_policy() -> OfflineIosAppAttestationPolicy {
        OfflineIosAppAttestationPolicy {
            team_id: "TEAMID1234".to_owned(),
            bundle_id: "io.soramitsu.pk".to_owned(),
            environment: "production".to_owned(),
            allowed_validation_categories: vec![4],
            allowed_bundle_versions: vec!["42".to_owned()],
        }
    }
    #[test]
    fn ios_assertion_auth_data_enforces_exact_extensions_and_policy() {
        let rp_id_hash = [0xA5; 32];
        let extension_bytes = ios_assertion_extension_bytes("42", 4);
        let encoded = ios_assertion_auth_data(
            rp_id_hash,
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
            1,
            &extension_bytes,
        );
        let parsed = parse_ios_app_attest_assertion_auth_data(&encoded)
            .expect("extension-bearing assertion authData");
        assert_eq!(parsed.rp_id_hash, rp_id_hash);
        assert_eq!(parsed.sign_count, 1);
        validate_ios_app_attest_extensions_against_policy(
            &ios_assertion_policy(),
            &parsed.extensions,
        )
        .expect("the exact governed category and bundle version are accepted");
        let reverse_order = ciborium::value::Value::Map(vec![
            (
                ciborium::value::Value::Text("validationCategory".to_owned()),
                ciborium::value::Value::Integer(4_u32.into()),
            ),
            (
                ciborium::value::Value::Text("bundleVersion".to_owned()),
                ciborium::value::Value::Text("42".to_owned()),
            ),
        ]);
        let mut reverse_order_bytes = Vec::new();
        ciborium::ser::into_writer(&reverse_order, &mut reverse_order_bytes)
            .expect("encode reverse-order Apple extension map");
        let reverse_order_auth_data = ios_assertion_auth_data(
            rp_id_hash,
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
            2,
            &reverse_order_bytes,
        );
        parse_ios_app_attest_assertion_auth_data(&reverse_order_auth_data)
            .expect("Apple does not require one map-key order");
        let mut nonminimal_definite = vec![0xB8, 0x02];
        nonminimal_definite.extend_from_slice(&extension_bytes[1..]);
        let nonminimal_definite_auth_data = ios_assertion_auth_data(
            rp_id_hash,
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
            3,
            &nonminimal_definite,
        );
        parse_ios_app_attest_assertion_auth_data(&nonminimal_definite_auth_data)
            .expect("valid definite Apple CBOR is accepted without serializer byte equality");
        let wrong_category = ios_assertion_extension_bytes("42", 5);
        let wrong_category = ios_assertion_auth_data(
            rp_id_hash,
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
            2,
            &wrong_category,
        );
        let parsed = parse_ios_app_attest_assertion_auth_data(&wrong_category)
            .expect("well-formed but unlisted extension values");
        assert!(
            validate_ios_app_attest_extensions_against_policy(
                &ios_assertion_policy(),
                &parsed.extensions,
            )
            .is_err(),
            "an unlisted validation category must fail closed",
        );
        let wrong_version = ios_assertion_extension_bytes("43", 4);
        let wrong_version = ios_assertion_auth_data(
            rp_id_hash,
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
            3,
            &wrong_version,
        );
        let parsed = parse_ios_app_attest_assertion_auth_data(&wrong_version)
            .expect("well-formed but unlisted bundle version");
        assert!(
            validate_ios_app_attest_extensions_against_policy(
                &ios_assertion_policy(),
                &parsed.extensions,
            )
            .is_err(),
            "an unlisted bundle version must fail closed",
        );
    }
    #[test]
    fn ios_assertion_auth_data_rejects_bad_flags_trailing_and_unknown_extensions() {
        let rp_id_hash = [0xB6; 32];
        let extension_bytes = ios_assertion_extension_bytes("42", 4);
        for flags in [
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_USER_PRESENT,
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_ATTESTED_CREDENTIAL_DATA,
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_USER_VERIFIED,
        ] {
            let auth_data = ios_assertion_auth_data(rp_id_hash, flags, 1, &[]);
            assert!(
                parse_ios_app_attest_assertion_auth_data(&auth_data).is_err(),
                "App Attest assertion flags other than ED must fail closed",
            );
        }
        let missing_extensions = ios_assertion_auth_data(
            rp_id_hash,
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
            1,
            &[],
        );
        assert!(parse_ios_app_attest_assertion_auth_data(&missing_extensions).is_err());
        let mut indefinite_extensions = vec![0xBF];
        indefinite_extensions.extend_from_slice(&extension_bytes[1..]);
        indefinite_extensions.push(0xFF);
        let indefinite = ios_assertion_auth_data(
            rp_id_hash,
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
            1,
            &indefinite_extensions,
        );
        assert!(parse_ios_app_attest_assertion_auth_data(&indefinite).is_err());
        let extensions_without_ed = ios_assertion_auth_data(rp_id_hash, 0, 1, &extension_bytes);
        assert!(parse_ios_app_attest_assertion_auth_data(&extensions_without_ed).is_err());
        let mut trailing_extensions = extension_bytes.clone();
        trailing_extensions.push(0xF6);
        let trailing = ios_assertion_auth_data(
            rp_id_hash,
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
            1,
            &trailing_extensions,
        );
        assert!(parse_ios_app_attest_assertion_auth_data(&trailing).is_err());
        let unknown = ciborium::value::Value::Map(vec![
            (
                ciborium::value::Value::Text("bundleVersion".to_owned()),
                ciborium::value::Value::Text("42".to_owned()),
            ),
            (
                ciborium::value::Value::Text("unknown".to_owned()),
                ciborium::value::Value::Integer(7_u32.into()),
            ),
        ]);
        let mut unknown_extensions = Vec::new();
        ciborium::ser::into_writer(&unknown, &mut unknown_extensions)
            .expect("encode unknown extension fixture");
        let unknown = ios_assertion_auth_data(
            rp_id_hash,
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
            1,
            &unknown_extensions,
        );
        assert!(parse_ios_app_attest_assertion_auth_data(&unknown).is_err());
        let apple_attestation_keys = ciborium::value::Value::Map(vec![
            (
                ciborium::value::Value::Text("apple_bundle_version_01".to_owned()),
                ciborium::value::Value::Text("42".to_owned()),
            ),
            (
                ciborium::value::Value::Text("apple_validation_category_01".to_owned()),
                ciborium::value::Value::Integer(4_u32.into()),
            ),
        ]);
        let mut apple_attestation_extensions = Vec::new();
        ciborium::ser::into_writer(&apple_attestation_keys, &mut apple_attestation_extensions)
            .expect("encode attestation-only extension fixture");
        let wrong_wire_keys = ios_assertion_auth_data(
            rp_id_hash,
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
            1,
            &apple_attestation_extensions,
        );
        assert!(
            parse_ios_app_attest_assertion_auth_data(&wrong_wire_keys).is_err(),
            "attestation apple_*_01 keys must not be accepted on assertion authData",
        );
        assert!(
            decode_ios_app_attest_attestation_extensions(&extension_bytes).is_err(),
            "assertion validationCategory/bundleVersion keys must not be accepted in attestation authData",
        );
    }
    #[test]
    fn ios_assertion_extensions_and_counter_rules_are_mandatory_and_strict() {
        let rp_id_hash = [0xC7; 32];
        let without_extensions = ios_assertion_auth_data(rp_id_hash, 0, 9, &[]);
        assert!(
            parse_ios_app_attest_assertion_auth_data(&without_extensions).is_err(),
            "extension-free assertion authData must fail closed",
        );
        let extension_bytes = ios_assertion_extension_bytes("42", 4);
        let encoded = ios_assertion_auth_data(
            rp_id_hash,
            OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
            9,
            &extension_bytes,
        );
        let parsed = parse_ios_app_attest_assertion_auth_data(&encoded)
            .expect("extension-bearing assertion authData is structurally valid");
        validate_ios_app_attest_extensions_against_policy(
            &ios_assertion_policy(),
            &parsed.extensions,
        )
        .expect("the required assertion extensions satisfy the pinned policy");
        validate_ios_app_attest_assertion_binding(&parsed, rp_id_hash, 8)
            .expect("a strictly increasing counter is accepted");
        for (sign_count, last_sign_count) in [(0, 0), (8, 8), (7, 8)] {
            let candidate = IosAppAttestAssertionAuthData {
                rp_id_hash,
                sign_count,
                extensions: parsed.extensions.clone(),
            };
            assert!(
                validate_ios_app_attest_assertion_binding(&candidate, rp_id_hash, last_sign_count,)
                    .is_err(),
                "zero, equal, and decreasing counters must fail closed",
            );
        }
        assert!(
            validate_ios_app_attest_assertion_binding(&parsed, [0xD8; 32], 8).is_err(),
            "the RP/application hash must match exactly",
        );
    }
    #[test]
    fn ios_policy_rejects_reserved_or_inappropriate_validation_categories() {
        let mut policy = default_offline_device_attestation_policy()
            .expect("built-in roots form a valid test policy");
        policy.require_ios_app_policy = true;
        policy.ios_apps = vec![ios_assertion_policy()];
        validate_offline_attestation_policy(&policy, 0)
            .expect("documented category 4 is policy-valid");
        for category in [0, 7, 8, 9, 11] {
            policy.ios_apps[0].allowed_validation_categories = vec![category];
            assert!(
                validate_offline_attestation_policy(&policy, 0).is_err(),
                "validation category {category} must be rejected regardless of governance",
            );
        }
    }
    #[test]
    fn ios_app_admission_requires_explicit_pinned_policy() {
        let mut policy = default_offline_device_attestation_policy()
            .expect("built-in roots form a valid test policy");
        let app = ios_assertion_policy();
        assert!(
            ensure_ios_app_allowed_by_policy(
                &policy,
                &app.team_id,
                &app.bundle_id,
                &app.environment,
            )
            .is_err(),
            "the consensus default must not admit an arbitrary iOS app",
        );
        policy.ios_apps = vec![app.clone()];
        assert!(
            ensure_ios_app_allowed_by_policy(
                &policy,
                &app.team_id,
                &app.bundle_id,
                &app.environment,
            )
            .is_err(),
            "a pinned iOS app must remain disabled until governance enables App Attest",
        );
        policy.require_ios_app_policy = true;
        ensure_ios_app_allowed_by_policy(&policy, &app.team_id, &app.bundle_id, &app.environment)
            .expect("the exact enabled iOS app identity is accepted");
        assert!(
            ensure_ios_app_allowed_by_policy(
                &policy,
                &app.team_id,
                "pk.retail.wallet.ios.substitute",
                &app.environment,
            )
            .is_err(),
            "a substituted iOS bundle must fail closed",
        );
    }
    #[test]
    fn registration_lifetime_requires_one_continuously_active_platform_root() {
        let mut policy = default_offline_device_attestation_policy()
            .expect("built-in roots form a valid test policy");
        let mut android_roots: Vec<_> = policy
            .trusted_roots
            .iter_mut()
            .filter(|root| root.platform == OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT)
            .collect();
        assert!(
            android_roots.len() >= 2,
            "test policy needs two Android roots"
        );
        android_roots[0].not_after_ms = Some(POLICY_TEST_TIME_MS + 30_000);
        android_roots[1].not_before_ms = Some(POLICY_TEST_TIME_MS + 30_000);
        drop(android_roots);
        assert!(
            offline_attestation_policy_for_registration_lifetime(
                &policy,
                OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT,
                POLICY_TEST_TIME_MS,
                POLICY_TEST_TIME_MS + 60_000,
            )
            .is_err(),
            "different roots covering opposite endpoints must not be combined into a lifetime admission",
        );
        policy
            .trusted_roots
            .iter_mut()
            .find(|root| root.platform == OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT)
            .expect("Android test root")
            .not_after_ms = Some(POLICY_TEST_TIME_MS + 60_000);
        let lifetime = offline_attestation_policy_for_registration_lifetime(
            &policy,
            OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT,
            POLICY_TEST_TIME_MS,
            POLICY_TEST_TIME_MS + 60_000,
        )
        .expect("one root covering both endpoints is sufficient");
        assert_eq!(lifetime.trusted_roots.len(), 1);
    }
    #[test]
    fn android_app_admission_requires_explicit_pinned_policy() {
        let package_name = "com.pk.retailwallet";
        let signing_digest = [0xE9; 32];
        let mut policy = default_offline_device_attestation_policy()
            .expect("built-in roots form a valid test policy");
        assert!(
            ensure_android_app_allowed_by_policy(&policy, package_name, &signing_digest,).is_err(),
            "the consensus default must not admit arbitrary Android apps",
        );
        policy.android_apps = vec![OfflineAndroidAppAttestationPolicy {
            package_name: package_name.to_owned(),
            signing_certificate_sha256: vec![signing_digest.to_vec()],
        }];
        assert!(
            ensure_android_app_allowed_by_policy(&policy, package_name, &signing_digest,).is_err(),
            "a pinned app entry must remain disabled until governance enables Android",
        );
        policy.require_android_app_policy = true;
        ensure_android_app_allowed_by_policy(&policy, package_name, &signing_digest)
            .expect("the exact enabled package and signer are accepted");
        assert!(
            ensure_android_app_allowed_by_policy(
                &policy,
                "com.pk.retailwallet.substitute",
                &signing_digest,
            )
            .is_err(),
            "a substituted package must fail closed",
        );
        assert!(
            ensure_android_app_allowed_by_policy(&policy, package_name, &[0xEA; 32]).is_err(),
            "a substituted signing certificate must fail closed",
        );
    }
}
