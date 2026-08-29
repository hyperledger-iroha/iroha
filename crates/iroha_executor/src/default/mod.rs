//! Definition of Iroha default executor and accompanying execute functions.
use crate::{
    Execute, deny, execute,
    permission::{AnyPermission, ExecutorPermission as _},
};
/// Re-export account visitor helpers used by the default executor.
pub use account::{
    visit_approve_account_recovery, visit_cancel_account_recovery,
    visit_clear_account_recovery_policy, visit_finalize_account_recovery,
    visit_propose_account_recovery, visit_register_account, visit_remove_account_key_value,
    visit_replace_account_controller, visit_set_account_key_value,
    visit_set_account_recovery_policy, visit_unregister_account,
};
/// Re-export asset visitor helpers used by the default executor.
pub use asset::{
    visit_burn_asset_quantity, visit_mint_asset_quantity, visit_remove_asset_key_value,
    visit_set_asset_holding_limit, visit_set_asset_key_value,
    visit_set_asset_transfer_availability, visit_set_asset_transfer_control,
    visit_transfer_asset_quantity,
};
/// Re-export asset-definition visitor helpers used by the default executor.
pub use asset_definition::{
    visit_register_asset_definition, visit_remove_asset_definition_key_value,
    visit_set_asset_definition_alias, visit_set_asset_definition_key_value,
    visit_transfer_asset_definition, visit_unregister_asset_definition,
};
/// Re-export bridge visitor helpers.
pub use bridge::{visit_apply_sccp_route_governance, visit_record_bridge_receipt};
/// Re-export domain visitor helpers used by the default executor.
pub use domain::{
    visit_register_domain, visit_remove_domain_key_value, visit_set_domain_key_value,
    visit_transfer_domain, visit_unregister_domain,
};
/// Re-export upgrade visitor helper.
pub use executor::visit_upgrade;
/// Re-export governance visitors handled by the default executor.
pub use governance::{
    visit_propose_sccp_route_governance, visit_propose_sorafs_provider_governance,
    visit_propose_validation_fee_policy, visit_register_citizen,
};
use iroha_smart_contract::Iroha;
use iroha_smart_contract::data_model::{
    executor::Result,
    isi::{
        AcceptSorafsModerationJurorAssignment, ActivatePublicLaneValidator,
        ActivateSorafsModerationCase, AdvanceSorafsReserveLifecycle,
        AppendSorafsPorReputationJournalEntry, AppendSorafsStreamTokenReputationJournalEntry,
        ApplySorafsRepairTaskAction, ApprovePinManifest, BindManifestAlias,
        CancelSorafsOrderbookOrder, ChargeSorafsReserveRent, CommitSorafsPopCredentialBatch,
        CompleteReplicationOrder, DecideSorafsReserveAppeal, DecideSorafsReserveMovement,
        DrawSorafsReserveCredit, ExitPublicLaneValidator, ExpireReplicationOrder,
        ExpireSorafsModerationChallenge, FinalizeSorafsModerationCase,
        FinalizeSorafsModerationSortition, IssueReplicationOrder, MaintainSorafsOrderbook,
        MatchSorafsOrderbook, PublishSorafsPopRevocationList, RaiseSorafsModerationChallenge,
        RecordCapacityTelemetry, RecordSorafsOrderbookSettlementReceipt,
        RegisterCapacityDeclaration, RegisterCapacityDispute, RegisterPeerWithPop,
        RegisterPinManifest, RegisterProviderOwner, RegisterPublicLaneValidator,
        RegisterSorafsModerationJurorEligibility, RegisterSorafsReserveAccount,
        RemoveAssetKeyValue, RepaySorafsReserveCredit, RequestSorafsReserveMovement,
        ResolveSorafsCapacityDispute, ResolveSorafsModerationChallenge, RetirePinManifest,
        ReviseReplicationOrderAssignments, RevokeProviderIngestCompletionAuthority,
        SetAssetKeyValue, SetLaneRelayEmergencyValidators, SetPricingSchedule,
        SetProviderIngestCompletionAuthority, SetSorafsModerationPolicy, SetSorafsOrderbookPolicy,
        SetSorafsPopIssuerPolicy, SetSorafsReputationJournalAuthorityPolicy,
        SetSorafsReservePolicy, SubmitSorafsModerationAppeal, SubmitSorafsModerationCommit,
        SubmitSorafsModerationReveal, SubmitSorafsOrderbookOrder, SubmitSorafsRepairAppeal,
        SubmitSorafsRepairTask, SubmitSorafsReserveAppeal, UnregisterProviderOwner,
        UpsertProviderCredit,
        alias_setup::{
            CompareAndSetPrimaryAccountAlias, ConfigureAliasAutoRenew, EnsureAlias,
            RebindAccountAlias, RenewAliasLease,
        },
        asset_alias::SetAssetDefinitionAlias,
        bridge::{ApplySccpRouteGovernance, RecordBridgeReceipt},
        contract_alias::SetContractAlias,
        defi::DeFiInstructionBox,
        governance::{
            ProposeContractEmergencyHold, ProposeContractLifecycleGovernance,
            ProposeSccpRouteGovernance, ProposeSorafsProviderGovernance,
            ProposeValidationFeePayoutLifecycle, ProposeValidationFeePolicy, RegisterCitizen,
        },
        nexus::{
            ActivateFeeSponsorProgramRevision, BeginCloseFeeSponsorProgram, CloseFeeSponsorProgram,
            CreateFeeSponsorProgram, EnrollFeeSponsorBeneficiary, FundFeeSponsorProgram,
            PauseFeeSponsorProgram, RegisterVerifiedFeeSponsorVaultAllocation,
            RegisterVerifiedLaneRelay, StageFeeSponsorProgramRevision,
            UnenrollFeeSponsorBeneficiary, WithdrawFeeSponsorProgram,
        },
        offline::{
            ActivateKagemushaRecursiveReleaseV4, AuthorizeKagemushaTairaCanaryV4,
            CancelKagemushaRecursiveReleaseV4, DeactivateKagemushaRecursiveIssuanceV4,
            EnableKagemushaRecursiveIssuanceV4, RecordKagemushaTairaCanaryV4,
            RedeemKagemushaRecursiveV4, RegisterOfflineDeviceAttestation,
            SetOfflineDeviceAttestationPolicy, TopUpKagemushaRecursiveV4,
        },
        repo::{RepoInstructionBox, RepoIsi, RepoMarginCallIsi, ReverseRepoIsi},
        settlement::SettlementInstructionBox,
        smart_contract_code::{
            AcceptContractOwnership, ActivateContractInstance, CancelContractOwnershipOffer,
            CancelSmartContractCodeUpload, CommitContractDeployment, DeactivateContractInstance,
            FinalizeSmartContractCodeUpload, OfferContractOwnership, RegisterSmartContractBytes,
            RegisterSmartContractCode, RemoveSmartContractBytes, SetContractParliamentDelegation,
            UploadSmartContractCodeChunk,
        },
        vpn::{OpenVpnLeaseEscrow, RefundExpiredVpnLease, SettleVpnLease},
    },
    prelude::*,
    query::error::{FindError, QueryExecutionFail},
    visit::Visit,
};
macro_rules! declare_execute_visitors {
    (
        $(
            $(#[$attribute:meta])*
            $name:ident($instruction:ty);
        )+
    ) => {
        $(
            $(#[$attribute])*
            pub fn $name<V: Execute + Visit + ?Sized>(
                executor: &mut V,
                isi: &$instruction,
            ) {
                execute!(executor, isi);
            }
        )+
    };
}
macro_rules! declare_query_visitors {
    (
        no_op;
        $(
            $(#[$attribute:meta])*
            $name:ident($query:ty);
        )+
    ) => {
        $(
            $(#[$attribute])*
            pub fn $name<V: Execute + Visit + ?Sized>(
                _executor: &mut V,
                _query: &$query,
            ) {
            }
        )+
    };
    (
        via $helper:ident;
        $(
            $(#[$attribute:meta])*
            $name:ident($query:ty);
        )+
    ) => {
        $(
            $(#[$attribute])*
            pub fn $name<V: Execute + Visit + ?Sized>(
                executor: &mut V,
                _query: &$query,
            ) {
                $helper(executor);
            }
        )+
    };
}
/// Re-export dispatch for custom instructions.
pub use isi::visit_custom_instruction;
/// Re-export logging instruction visitor helper.
pub use log::visit_log;
/// Re-export Nexus visitor helper used by the default executor.
pub use nexus::visit_set_lane_relay_emergency_validators;
/// Re-export NFT visitor helpers used by the default executor.
pub use nft::{
    visit_register_nft, visit_remove_nft_key_value, visit_set_nft_key_value, visit_transfer_nft,
    visit_unregister_nft,
};
/// Re-export parameter visitor helpers used by the default executor.
pub use parameter::visit_set_parameter;
/// Re-export peer visitor helpers used by the default executor.
pub use peer::{visit_register_peer, visit_unregister_peer};
/// Re-export permission visitor helpers used by the default executor.
pub use permission::{visit_grant_account_permission, visit_revoke_account_permission};
/// Re-export role visitor helpers used by the default executor.
pub use role::{
    visit_grant_account_role, visit_grant_role_permission, visit_register_role,
    visit_revoke_account_role, visit_revoke_role_permission, visit_unregister_role,
};
/// Re-export permission-checked `SoraFS` query visitors.
pub use sorafs::{
    visit_find_sorafs_moderation_appeal, visit_find_sorafs_moderation_case,
    visit_find_sorafs_moderation_challenge, visit_find_sorafs_moderation_commit,
    visit_find_sorafs_moderation_events, visit_find_sorafs_moderation_juror_eligibility,
    visit_find_sorafs_moderation_no_show, visit_find_sorafs_moderation_outcome,
    visit_find_sorafs_moderation_policy, visit_find_sorafs_moderation_reveal,
    visit_find_sorafs_moderation_snapshot, visit_find_sorafs_moderation_status,
    visit_find_sorafs_orderbook_cancellation_by_order_id,
    visit_find_sorafs_orderbook_channel_by_id, visit_find_sorafs_orderbook_channels,
    visit_find_sorafs_orderbook_events, visit_find_sorafs_orderbook_order_by_id,
    visit_find_sorafs_orderbook_orders, visit_find_sorafs_orderbook_policy,
    visit_find_sorafs_orderbook_receipt_by_id, visit_find_sorafs_orderbook_receipts,
    visit_find_sorafs_orderbook_status, visit_find_sorafs_orderbook_trade_by_id,
    visit_find_sorafs_orderbook_trades, visit_find_sorafs_pop_audit_digest_by_sequence,
    visit_find_sorafs_pop_commitment_root_by_version,
    visit_find_sorafs_pop_credential_commitment_by_digest, visit_find_sorafs_pop_issuer_policy,
    visit_find_sorafs_pop_registry_status, visit_find_sorafs_pop_revocation_by_nonce_commitment,
    visit_find_sorafs_pop_revocation_publication_by_version, visit_find_sorafs_repair_events,
    visit_find_sorafs_repair_status, visit_find_sorafs_repair_task, visit_find_sorafs_repair_tasks,
    visit_find_sorafs_reputation_journal_authority_policy,
    visit_find_sorafs_reputation_journal_event_by_source_id,
    visit_find_sorafs_reputation_journal_events, visit_find_sorafs_reserve_appeal_by_id,
    visit_find_sorafs_reserve_appeals, visit_find_sorafs_reserve_events,
    visit_find_sorafs_reserve_movement_by_id, visit_find_sorafs_reserve_movements,
    visit_find_sorafs_reserve_policy, visit_find_sorafs_reserve_provider_by_id,
    visit_find_sorafs_reserve_providers,
};
/// Re-export staking visitor helpers used by the default executor.
pub use staking::{
    visit_activate_public_lane_validator, visit_exit_public_lane_validator,
    visit_register_public_lane_validator,
};
/// Re-export trigger visitor helpers used by the default executor.
pub use trigger::{
    visit_burn_trigger_repetitions, visit_execute_trigger, visit_mint_trigger_repetitions,
    visit_register_trigger, visit_remove_trigger_key_value, visit_set_trigger_key_value,
    visit_unregister_trigger,
};
fn is_reserved_multisig_role_id(role_id: &RoleId) -> bool {
    const MULTISIG_SIGNATORY_NAMESPACE: &str = "MULTISIG_SIGNATORY";
    let name = role_id.name().as_ref();
    name == MULTISIG_SIGNATORY_NAMESPACE
        || name
            .strip_prefix(MULTISIG_SIGNATORY_NAMESPACE)
            .is_some_and(|suffix| suffix.starts_with('/'))
}
#[cfg(test)]
mod multisig_role_namespace_tests {
    use super::*;
    #[test]
    fn reservation_is_exact_and_does_not_parse_process_local_addresses() {
        for name in [
            "MULTISIG_SIGNATORY",
            "MULTISIG_SIGNATORY/domain/address",
            "MULTISIG_SIGNATORY//opaque",
        ] {
            let role_id: RoleId = name.parse().expect("valid reserved role id");
            assert!(
                is_reserved_multisig_role_id(&role_id),
                "must reserve {name}"
            );
        }
        for name in [
            "MULTISIG_SIGNATORY_ADJACENT",
            "MULTISIG_SIGNATORY2/domain/address",
            "ordinary-role",
        ] {
            let role_id: RoleId = name.parse().expect("valid ordinary role id");
            assert!(
                !is_reserved_multisig_role_id(&role_id),
                "must not reserve {name}"
            );
        }
    }
}
/// Helpers shared by custom instruction integrations.
pub mod isi;
// NOTE: If any new `visit_..` functions are introduced in this module, one should
// not forget to update the default executor boilerplate too, specifically the
// `iroha_executor::derive::default::impl_derive_visit` function
// signature list.
#[derive(norito::derive::JsonDeserialize)]
struct IvmProvedJsonView {
    bytecode: IvmBytecode,
    overlay: Vec<InstructionBox>,
    events_commitment: Hash,
    gas_policy_commitment: Hash,
}
fn decode_ivm_proved_view(proved: &IvmProved) -> Option<(IvmBytecode, Vec<InstructionBox>)> {
    let rendered = norito::json::to_json(proved).ok()?;
    let parsed: IvmProvedJsonView = norito::json::from_str(&rendered).ok()?;
    let _ = (parsed.events_commitment, parsed.gas_policy_commitment);
    Some((parsed.bytecode, parsed.overlay))
}
/// Recognize the sole non-genesis path that may grant deployment authority.
///
/// Torii can onboard a transaction authority that does not exist yet, but the ordinary
/// `CanRegisterSmartContractCode` grant policy is genesis-only. Keep this exception bound to an
/// auditable, ordered native-deployment prefix: register the transaction authority, grant that
/// exact authority the exact deployment permission, then either upload code or register a manifest
/// for code that is already present. Whether the account is actually absent is checked against
/// pre-transaction state in [`visit_transaction`].
fn has_contract_deployment_self_bootstrap_prefix(
    authority: &AccountId,
    instructions: &[InstructionBox],
) -> bool {
    if instructions
        .iter()
        .any(|instruction| instruction.as_any().is::<CommitContractDeployment>())
    {
        // Atomic deployment consumes a pre-existing authority's reserved nonce. It must never
        // inherit the narrow upload-only account bootstrap exception.
        return false;
    }
    let Some([register, grant, deployment]) = instructions.get(..3) else {
        return false;
    };
    let Some(RegisterBox::Account(register)) = register.as_any().downcast_ref::<RegisterBox>()
    else {
        return false;
    };
    if register.object().id() != authority
        || !register.object().metadata().is_empty()
        || register.object().label().is_some()
        || register.object().uaid().is_some()
        || !register.object().opaque_ids().is_empty()
    {
        return false;
    }
    let Some(GrantBox::Permission(grant)) = grant.as_any().downcast_ref::<GrantBox>() else {
        return false;
    };
    let expected_permission: Permission =
        iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode.into();
    if grant.destination() != authority || grant.object() != &expected_permission {
        return false;
    }
    deployment
        .as_any()
        .downcast_ref::<UploadSmartContractCodeChunk>()
        .is_some_and(|upload| *upload.chunk_index() == 0)
        || deployment.as_any().is::<RegisterSmartContractCode>()
}
fn classify_contract_deployment_account_lookup<T>(
    authority: &AccountId,
    result: Result<T, ValidationFail>,
) -> Result<bool, ValidationFail> {
    match result {
        Ok(_) => Ok(true),
        Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::Account(missing))))
            if &missing == authority =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}
fn account_exists_before_transaction<V: Execute + Visit + ?Sized>(
    executor: &V,
    authority: &AccountId,
) -> Result<bool, ValidationFail> {
    classify_contract_deployment_account_lookup(
        authority,
        executor
            .host()
            .query_single(FindAccountById::new(authority.clone())),
    )
}
#[cfg(test)]
mod contract_deployment_bootstrap_tests {
    use super::*;
    use crate::{Iroha, prelude};
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        account::{AccountAlias, NewAccount, OpaqueAccountId},
        isi::smart_contract_code::{
            CancelSmartContractCodeUpload, RegisterSmartContractCode, UploadSmartContractCodeChunk,
        },
        metadata::Metadata,
        nexus::{DataSpaceId, UniversalAccountId},
        permission::Permission,
        prelude::Json,
        smart_contract::manifest::ContractManifest,
    };
    use std::num::NonZeroU64;
    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("non-zero deterministic key seed");
        AccountId::new(key_pair.public_key().clone())
    }
    fn upload_instruction() -> InstructionBox {
        UploadSmartContractCodeChunk {
            code_hash: Hash::new(b"executor deployment bootstrap fixture"),
            total_size: 1,
            chunk_index: 0,
            chunk_count: 1,
            chunk: vec![0x01],
        }
        .into()
    }
    fn bootstrap_prefix(
        registered_account: NewAccount,
        grant_destination: AccountId,
        permission: Permission,
        deployment: InstructionBox,
    ) -> Vec<InstructionBox> {
        vec![
            Register::account(registered_account).into(),
            Grant::account_permission(permission, grant_destination).into(),
            deployment,
        ]
    }
    fn deployment_permission() -> Permission {
        iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode.into()
    }
    fn manifest() -> ContractManifest {
        ContractManifest {
            seiyaku_name: None,
            code_hash: Some(Hash::new(b"executor bootstrap manifest code")),
            abi_hash: Some(Hash::new(b"executor bootstrap manifest ABI")),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            error_codes: None,
            kotoba: None,
            provenance: None,
        }
    }
    fn manifest_instruction() -> InstructionBox {
        RegisterSmartContractCode {
            manifest: manifest(),
        }
        .into()
    }
    #[derive(Debug)]
    struct TestExecutor {
        host: Iroha,
        context: prelude::Context,
        verdict: crate::data_model::executor::Result<(), ValidationFail>,
    }
    impl TestExecutor {
        fn non_genesis(authority: AccountId) -> Self {
            Self {
                host: Iroha,
                context: prelude::Context {
                    authority,
                    curr_block: BlockHeader::new(
                        NonZeroU64::new(2).expect("non-zero block height"),
                        None,
                        None,
                        None,
                        0,
                        0,
                    ),
                },
                verdict: Ok(()),
            }
        }
    }
    impl Execute for TestExecutor {
        fn host(&self) -> &Iroha {
            &self.host
        }
        fn context(&self) -> &prelude::Context {
            &self.context
        }
        fn context_mut(&mut self) -> &mut prelude::Context {
            &mut self.context
        }
        fn verdict(&self) -> &crate::data_model::executor::Result<(), ValidationFail> {
            &self.verdict
        }
        fn deny(&mut self, reason: ValidationFail) {
            self.verdict = Err(reason);
        }
    }
    impl Visit for TestExecutor {}
    #[test]
    fn exact_native_upload_self_bootstrap_prefix_is_recognized() {
        let authority = account(1);
        let instructions = bootstrap_prefix(
            Account::new(authority.clone()),
            authority.clone(),
            deployment_permission(),
            upload_instruction(),
        );
        assert!(has_contract_deployment_self_bootstrap_prefix(
            &authority,
            &instructions
        ));
    }
    #[test]
    fn exact_matching_code_manifest_self_bootstrap_prefix_is_recognized() {
        let authority = account(1);
        let instructions = bootstrap_prefix(
            Account::new(authority.clone()),
            authority.clone(),
            deployment_permission(),
            manifest_instruction(),
        );
        assert!(has_contract_deployment_self_bootstrap_prefix(
            &authority,
            &instructions
        ));
    }
    #[test]
    fn deployment_account_lookup_only_treats_exact_missing_authority_as_absent() {
        let authority = account(1);
        let other = account(2);
        assert_eq!(
            classify_contract_deployment_account_lookup(&authority, Ok::<_, ValidationFail>(())),
            Ok(true)
        );
        assert_eq!(
            classify_contract_deployment_account_lookup::<()>(
                &authority,
                Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(
                    FindError::Account(authority.clone()),
                ))),
            ),
            Ok(false)
        );
        let wrong_missing =
            ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::Account(other)));
        assert_eq!(
            classify_contract_deployment_account_lookup::<()>(
                &authority,
                Err(wrong_missing.clone()),
            ),
            Err(wrong_missing)
        );
        let unrelated = ValidationFail::QueryFailed(QueryExecutionFail::NotFound);
        assert_eq!(
            classify_contract_deployment_account_lookup::<()>(&authority, Err(unrelated.clone()),),
            Err(unrelated)
        );
    }
    #[test]
    fn direct_non_genesis_deployment_permission_grant_remains_genesis_only() {
        use crate::permission::ValidateGrantRevoke as _;
        let authority = account(1);
        let executor = TestExecutor::non_genesis(authority.clone());
        let permission =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
        let error = permission
            .validate_grant(&authority, executor.context(), executor.host())
            .expect_err("ordinary non-genesis self-grant must remain genesis-only");
        assert!(matches!(error, ValidationFail::NotPermitted(message) if
            message.contains("only allowed inside the genesis block")));
    }
    #[test]
    fn contract_lifecycle_instructions_reach_core_dispatch() {
        let authority = account(1);
        let code_hash = Hash::new(b"executor lifecycle dispatch code");
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            7,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let instructions: Vec<InstructionBox> = vec![
            RegisterSmartContractCode {
                manifest: manifest(),
            }
            .into(),
            DeactivateContractInstance {
                contract_address: contract_address.clone(),
                expected_revision: 1,
                reason: Some("dispatch fixture".to_owned()),
            }
            .into(),
            ActivateContractInstance {
                contract_address: contract_address.clone(),
                expected_revision: 1,
                code_hash,
            }
            .into(),
            CommitContractDeployment {
                expected_deploy_nonce: 7,
                contract_address: contract_address.clone(),
                code_hash,
                contract_alias: "payments::universal".parse().expect("contract alias"),
                lease_expiry_ms: None,
                expected_previous_contract_address: None,
            }
            .into(),
            RegisterSmartContractBytes {
                code_hash,
                code: vec![0x01],
            }
            .into(),
            UploadSmartContractCodeChunk {
                code_hash,
                total_size: 1,
                chunk_index: 0,
                chunk_count: 1,
                chunk: vec![0x01],
            }
            .into(),
            FinalizeSmartContractCodeUpload {
                code_hash,
                total_size: 1,
                chunk_count: 1,
            }
            .into(),
            CancelSmartContractCodeUpload { code_hash }.into(),
            RemoveSmartContractBytes {
                code_hash,
                reason: Some("dispatch fixture".to_owned()),
            }
            .into(),
            SetContractAlias::clear(contract_address).into(),
        ];
        for instruction in instructions {
            let mut executor = TestExecutor::non_genesis(authority.clone());
            visit_instruction(&mut executor, &instruction);
            assert!(
                executor.verdict().is_ok(),
                "known lifecycle instruction must reach Core dispatch: {instruction:?}"
            );
        }
    }
    #[test]
    fn deployment_self_bootstrap_rejects_adversarial_shapes() {
        let authority = account(1);
        let other = account(2);
        let wrong_account = bootstrap_prefix(
            Account::new(other.clone()),
            authority.clone(),
            deployment_permission(),
            upload_instruction(),
        );
        assert!(!has_contract_deployment_self_bootstrap_prefix(
            &authority,
            &wrong_account
        ));
        let wrong_destination = bootstrap_prefix(
            Account::new(authority.clone()),
            other,
            deployment_permission(),
            upload_instruction(),
        );
        assert!(!has_contract_deployment_self_bootstrap_prefix(
            &authority,
            &wrong_destination
        ));
        let malformed_permission = Permission::new(
            "CanRegisterSmartContractCode".into(),
            Json::from_raw_json("{\"unexpected\":true}".to_owned()).expect("valid JSON fixture"),
        );
        let malformed_grant = bootstrap_prefix(
            Account::new(authority.clone()),
            authority.clone(),
            malformed_permission,
            upload_instruction(),
        );
        assert!(!has_contract_deployment_self_bootstrap_prefix(
            &authority,
            &malformed_grant
        ));
        let cleanup = bootstrap_prefix(
            Account::new(authority.clone()),
            authority.clone(),
            deployment_permission(),
            CancelSmartContractCodeUpload {
                code_hash: Hash::new(b"executor deployment cleanup fixture"),
            }
            .into(),
        );
        assert!(!has_contract_deployment_self_bootstrap_prefix(
            &authority, &cleanup
        ));
        let non_initial_upload = bootstrap_prefix(
            Account::new(authority.clone()),
            authority.clone(),
            deployment_permission(),
            UploadSmartContractCodeChunk {
                code_hash: Hash::new(b"executor non-initial bootstrap fixture"),
                total_size: 2,
                chunk_index: 1,
                chunk_count: 2,
                chunk: vec![0x02],
            }
            .into(),
        );
        assert!(!has_contract_deployment_self_bootstrap_prefix(
            &authority,
            &non_initial_upload
        ));
    }
    #[test]
    fn deployment_self_bootstrap_rejects_adversarial_sequences() {
        let authority = account(1);
        let mut reordered = bootstrap_prefix(
            Account::new(authority.clone()),
            authority.clone(),
            deployment_permission(),
            upload_instruction(),
        );
        reordered.swap(1, 2);
        assert!(!has_contract_deployment_self_bootstrap_prefix(
            &authority, &reordered
        ));
        let exact = bootstrap_prefix(
            Account::new(authority.clone()),
            authority.clone(),
            deployment_permission(),
            upload_instruction(),
        );
        for truncated in 0..3 {
            assert!(!has_contract_deployment_self_bootstrap_prefix(
                &authority,
                &exact[..truncated]
            ));
        }
        let mut shifted = exact.clone();
        shifted.insert(0, upload_instruction());
        assert!(!has_contract_deployment_self_bootstrap_prefix(
            &authority, &shifted
        ));
        let mut atomic_deployment = exact.clone();
        atomic_deployment.push(
            CommitContractDeployment {
                expected_deploy_nonce: 0,
                contract_address: ContractAddress::derive(
                    &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                        .parse()
                        .expect("canonical test network id"),
                    &authority,
                    0,
                    DataSpaceId::UNIVERSAL,
                )
                .expect("atomic deployment contract address"),
                code_hash: Hash::new(b"executor atomic deployment bootstrap fixture"),
                contract_alias: "payments::universal".parse().expect("contract alias"),
                lease_expiry_ms: None,
                expected_previous_contract_address: None,
            }
            .into(),
        );
        assert!(!has_contract_deployment_self_bootstrap_prefix(
            &authority,
            &atomic_deployment
        ));
    }
    #[test]
    fn deployment_self_bootstrap_rejects_decorated_accounts() {
        let authority = account(1);
        let mut metadata = Metadata::default();
        metadata.insert("bootstrap".parse().expect("metadata key"), "forbidden");
        let decorated_accounts = [
            Account::new(authority.clone()).with_metadata(metadata),
            Account::new(authority.clone()).with_label(Some(AccountAlias::domainless(
                "bootstrap".parse().expect("alias label"),
                DataSpaceId::UNIVERSAL,
            ))),
            Account::new(authority.clone()).with_uaid(Some(UniversalAccountId::from_hash(
                Hash::new(b"executor bootstrap UAID"),
            ))),
            Account::new(authority.clone()).with_opaque_ids(vec![OpaqueAccountId::from_hash(
                Hash::new(b"executor bootstrap opaque id"),
            )]),
        ];
        for decorated_account in decorated_accounts {
            let decorated = bootstrap_prefix(
                decorated_account,
                authority.clone(),
                deployment_permission(),
                upload_instruction(),
            );
            assert!(!has_contract_deployment_self_bootstrap_prefix(
                &authority, &decorated
            ));
        }
    }
}
#[cfg(test)]
mod ivm_proved_decode_tests {
    use super::*;
    #[test]
    fn decode_ivm_proved_view_roundtrips_minimal_payload() {
        let expected_bytecode = IvmBytecode::from_compiled(vec![0xA1, 0xB2, 0xC3]);
        let expected_events_commitment = Hash::new(b"executor-events");
        let expected_gas_commitment = Hash::new(b"executor-gas");
        let bytecode_json = norito::json::to_json(&expected_bytecode).expect("bytecode json");
        let events_json =
            norito::json::to_json(&expected_events_commitment).expect("events commitment json");
        let gas_json = norito::json::to_json(&expected_gas_commitment).expect("gas json");
        let proved_json = format!(
            "{{\"bytecode\":{bytecode_json},\"overlay\":[],\"events_commitment\":{events_json},\"gas_policy_commitment\":{gas_json}}}"
        );
        let proved: IvmProved =
            norito::json::from_str(&proved_json).expect("IvmProved JSON payload should decode");
        let (bytecode, overlay) =
            decode_ivm_proved_view(&proved).expect("helper should decode proved payload");
        assert_eq!(bytecode.as_ref(), expected_bytecode.as_ref());
        assert!(overlay.is_empty(), "overlay should roundtrip as empty");
    }
}
/// Execute [`SignedTransaction`].
///
/// Transaction is executed following successful validation.
///
/// # Warning
///
/// [`Executable::Ivm`] is not executed because it is validated on the host side.
pub fn visit_transaction<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    transaction: &SignedTransaction,
) {
    match transaction.instructions() {
        Executable::Ivm(bytecode) => executor.visit_ivm(bytecode),
        Executable::IvmProved(proved) => {
            let (bytecode, overlay) =
                decode_ivm_proved_view(proved).expect("IvmProved payload must decode");
            executor.visit_ivm(&bytecode);
            for isi in &overlay {
                if executor.verdict().is_ok() {
                    executor.visit_instruction(isi);
                }
            }
        }
        Executable::ContractCall(_) => {}
        Executable::Batch(items) => {
            for item in items {
                if executor.verdict().is_err() {
                    break;
                }
                if let iroha_smart_contract::data_model::transaction::ExecutableBatchItem::Instruction(
                    isi,
                ) = item
                {
                    executor.visit_instruction(isi);
                }
            }
        }
        Executable::Instructions(instructions) => {
            let allow_deployment_self_bootstrap =
                has_contract_deployment_self_bootstrap_prefix(
                    transaction.authority(),
                    instructions,
                ) && match account_exists_before_transaction(executor, transaction.authority()) {
                    Ok(exists) => !exists,
                    Err(error) => {
                        executor.deny(error);
                        return;
                    }
                };
            for (index, isi) in instructions.iter().enumerate() {
                if executor.verdict().is_ok() {
                    if allow_deployment_self_bootstrap && index == 1 {
                        let Some(GrantBox::Permission(grant)) =
                            isi.as_any().downcast_ref::<GrantBox>()
                        else {
                            executor.deny(ValidationFail::InternalError(
                                "validated deployment bootstrap grant changed shape".to_owned(),
                            ));
                            return;
                        };
                        if let Err(error) = executor.host().submit(grant) {
                            executor.deny(error);
                        }
                    } else {
                        executor.visit_instruction(isi);
                    }
                }
            }
        }
    }
}
/// Execute [`InstructionBox`] by delegating to the appropriate visitor implementation.
pub fn visit_instruction<V: Execute + Visit + ?Sized>(executor: &mut V, isi: &InstructionBox) {
    isi.dispatch(executor);
}
declare_execute_visitors! {
    /// Forward declarative alias setup to Core's consensus-critical classifier and executor.
    visit_ensure_alias(EnsureAlias);
    /// Forward guarded alias lease renewal to Core's expiry-CAS executor.
    visit_renew_alias_lease(RenewAliasLease);
    /// Forward alias auto-renew configuration to Core's owner-only CAS executor.
    visit_configure_alias_auto_renew(ConfigureAliasAutoRenew);
    /// Forward explicit alias rebinding to Core's target-account CAS executor.
    visit_rebind_account_alias(RebindAccountAlias);
    /// Forward primary-alias compare-and-set to Core's lifecycle executor.
    visit_compare_and_set_primary_account_alias(CompareAndSetPrimaryAccountAlias);
}
trait InstructionDispatch {
    fn dispatch<V: Execute + Visit + ?Sized>(&self, executor: &mut V);
}
impl InstructionDispatch for InstructionBox {
    #[allow(clippy::too_many_lines)]
    fn dispatch<V: Execute + Visit + ?Sized>(&self, executor: &mut V) {
        // InstructionBox wraps a trait object. Downcast to known built-ins.
        let any = self.as_any();
        if let Some(isi) = any.downcast_ref::<SetParameter>() {
            executor.visit_set_parameter(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<Log>() {
            executor.visit_log(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ExecuteTrigger>() {
            executor.visit_execute_trigger(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<EnsureAlias>() {
            executor.visit_ensure_alias(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RenewAliasLease>() {
            executor.visit_renew_alias_lease(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ConfigureAliasAutoRenew>() {
            executor.visit_configure_alias_auto_renew(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RebindAccountAlias>() {
            executor.visit_rebind_account_alias(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<CompareAndSetPrimaryAccountAlias>() {
            executor.visit_compare_and_set_primary_account_alias(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<BurnBox>() {
            executor.visit_burn(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<GrantBox>() {
            executor.visit_grant(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterPeerWithPop>() {
            visit_register_peer(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<MintBox>() {
            executor.visit_mint(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterBox>() {
            if let RegisterBox::Peer(peer) = isi {
                visit_register_peer(executor, peer);
                return;
            }
            executor.visit_register(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RemoveKeyValueBox>() {
            executor.visit_remove_key_value(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RevokeBox>() {
            executor.visit_revoke(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetKeyValueBox>() {
            executor.visit_set_key_value(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ReplaceAccountController>() {
            account::visit_replace_account_controller(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetAccountRecoveryPolicy>() {
            account::visit_set_account_recovery_policy(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ClearAccountRecoveryPolicy>() {
            account::visit_clear_account_recovery_policy(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ProposeAccountRecovery>() {
            account::visit_propose_account_recovery(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ApproveAccountRecovery>() {
            account::visit_approve_account_recovery(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<CancelAccountRecovery>() {
            account::visit_cancel_account_recovery(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<FinalizeAccountRecovery>() {
            account::visit_finalize_account_recovery(executor, isi);
            return;
        }
        // Core owns the consensus-critical lifecycle, governance, and owner-scope checks for
        // these instructions. The default executor must forward them so those checks run.
        if let Some(isi) = any.downcast_ref::<RegisterSmartContractCode>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<DeactivateContractInstance>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<ActivateContractInstance>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<SetContractParliamentDelegation>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<OfferContractOwnership>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<AcceptContractOwnership>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<CancelContractOwnershipOffer>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<CommitContractDeployment>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<RegisterSmartContractBytes>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<UploadSmartContractCodeChunk>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<FinalizeSmartContractCodeUpload>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<CancelSmartContractCodeUpload>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<RemoveSmartContractBytes>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<SetContractAlias>() {
            execute!(executor, isi);
        }
        // Core owns offline note/device validation and the exact governance permissions for
        // attestation-policy mutations. Forward every native offline instruction so those
        // consensus-critical checks run.
        if let Some(isi) = any.downcast_ref::<TopUpKagemushaRecursiveV4>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<RedeemKagemushaRecursiveV4>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<ActivateKagemushaRecursiveReleaseV4>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<EnableKagemushaRecursiveIssuanceV4>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<CancelKagemushaRecursiveReleaseV4>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<DeactivateKagemushaRecursiveIssuanceV4>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<AuthorizeKagemushaTairaCanaryV4>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<RecordKagemushaTairaCanaryV4>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<RegisterOfflineDeviceAttestation>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<SetOfflineDeviceAttestationPolicy>() {
            execute!(executor, isi);
        }
        // Core owns the signature, chain/client binding, canonical policy,
        // active-account, address-slot, escrow, and lifecycle invariants. The
        // three VPN instructions form one indivisible native surface and must
        // all reach those consensus checks.
        if let Some(isi) = any.downcast_ref::<OpenVpnLeaseEscrow>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<SettleVpnLease>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<RefundExpiredVpnLease>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<SetAssetKeyValue>() {
            visit_set_asset_key_value(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetAssetDefinitionAlias>() {
            asset_definition::visit_set_asset_definition_alias(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetAssetTransferAvailability>() {
            asset::visit_set_asset_transfer_availability(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetAssetTransferControl>() {
            asset::visit_set_asset_transfer_control(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetAssetHoldingLimit>() {
            asset::visit_set_asset_holding_limit(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RemoveAssetKeyValue>() {
            visit_remove_asset_key_value(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterPublicLaneValidator>() {
            visit_register_public_lane_validator(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ActivatePublicLaneValidator>() {
            visit_activate_public_lane_validator(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ExitPublicLaneValidator>() {
            visit_exit_public_lane_validator(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetLaneRelayEmergencyValidators>() {
            nexus::visit_set_lane_relay_emergency_validators(executor, isi);
            return;
        }
        // Core performs the consensus-critical proof, lifecycle, immutable-revision, vault,
        // and exact delegated-permission checks for the typed fee-program surface.
        if let Some(isi) = any.downcast_ref::<RegisterVerifiedLaneRelay>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<RegisterVerifiedFeeSponsorVaultAllocation>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<CreateFeeSponsorProgram>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<StageFeeSponsorProgramRevision>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<ActivateFeeSponsorProgramRevision>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<PauseFeeSponsorProgram>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<BeginCloseFeeSponsorProgram>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<CloseFeeSponsorProgram>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<EnrollFeeSponsorBeneficiary>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<UnenrollFeeSponsorBeneficiary>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<FundFeeSponsorProgram>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<WithdrawFeeSponsorProgram>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<TransferBox>() {
            executor.visit_transfer(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RepoInstructionBox>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<RepoIsi>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<ReverseRepoIsi>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<RepoMarginCallIsi>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<DeFiInstructionBox>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<SettlementInstructionBox>() {
            settlement::visit_settlement_instruction(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterPinManifest>() {
            sorafs::visit_register_pin_manifest(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ApprovePinManifest>() {
            sorafs::visit_approve_pin_manifest(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RetirePinManifest>() {
            sorafs::visit_retire_pin_manifest(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<BindManifestAlias>() {
            sorafs::visit_bind_manifest_alias(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterCapacityDeclaration>() {
            sorafs::visit_register_capacity_declaration(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RecordCapacityTelemetry>() {
            sorafs::visit_record_capacity_telemetry(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterCapacityDispute>() {
            sorafs::visit_register_capacity_dispute(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ResolveSorafsCapacityDispute>() {
            sorafs::visit_resolve_capacity_dispute(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetSorafsReputationJournalAuthorityPolicy>() {
            sorafs::visit_set_reputation_journal_authority_policy(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<AppendSorafsPorReputationJournalEntry>() {
            sorafs::visit_append_por_reputation_journal_entry(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<AppendSorafsStreamTokenReputationJournalEntry>() {
            sorafs::visit_append_stream_token_reputation_journal_entry(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<IssueReplicationOrder>() {
            sorafs::visit_issue_replication_order(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<CompleteReplicationOrder>() {
            sorafs::visit_complete_replication_order(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ReviseReplicationOrderAssignments>() {
            sorafs::visit_revise_replication_order_assignments(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ExpireReplicationOrder>() {
            sorafs::visit_expire_replication_order(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RecordBridgeReceipt>() {
            bridge::visit_record_bridge_receipt(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ApplySccpRouteGovernance>() {
            bridge::visit_apply_sccp_route_governance(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ProposeSccpRouteGovernance>() {
            governance::visit_propose_sccp_route_governance(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ProposeContractLifecycleGovernance>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<ProposeContractEmergencyHold>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<ProposeSorafsProviderGovernance>() {
            governance::visit_propose_sorafs_provider_governance(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ProposeValidationFeePolicy>() {
            governance::visit_propose_validation_fee_policy(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ProposeValidationFeePayoutLifecycle>() {
            governance::visit_propose_validation_fee_payout_lifecycle(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterCitizen>() {
            governance::visit_register_citizen(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterProviderOwner>() {
            sorafs::visit_register_provider_owner(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<UnregisterProviderOwner>() {
            sorafs::visit_unregister_provider_owner(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetProviderIngestCompletionAuthority>() {
            sorafs::visit_set_provider_ingest_completion_authority(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RevokeProviderIngestCompletionAuthority>() {
            sorafs::visit_revoke_provider_ingest_completion_authority(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetPricingSchedule>() {
            sorafs::visit_set_pricing_schedule(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<UpsertProviderCredit>() {
            sorafs::visit_upsert_provider_credit(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SubmitSorafsRepairTask>() {
            sorafs::visit_submit_repair_task(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ApplySorafsRepairTaskAction>() {
            sorafs::visit_apply_repair_task_action(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SubmitSorafsRepairAppeal>() {
            sorafs::visit_submit_repair_appeal(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetSorafsOrderbookPolicy>() {
            sorafs::visit_set_orderbook_policy(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SubmitSorafsOrderbookOrder>() {
            sorafs::visit_submit_orderbook_order(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<CancelSorafsOrderbookOrder>() {
            sorafs::visit_cancel_orderbook_order(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RecordSorafsOrderbookSettlementReceipt>() {
            sorafs::visit_record_orderbook_settlement_receipt(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<MatchSorafsOrderbook>() {
            sorafs::visit_match_orderbook(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<MaintainSorafsOrderbook>() {
            sorafs::visit_maintain_orderbook(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetSorafsReservePolicy>() {
            sorafs::visit_set_reserve_policy(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterSorafsReserveAccount>() {
            sorafs::visit_register_reserve_account(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RequestSorafsReserveMovement>() {
            sorafs::visit_request_reserve_movement(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<DecideSorafsReserveMovement>() {
            sorafs::visit_decide_reserve_movement(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ChargeSorafsReserveRent>() {
            sorafs::visit_charge_reserve_rent(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<AdvanceSorafsReserveLifecycle>() {
            sorafs::visit_advance_reserve_lifecycle(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<DrawSorafsReserveCredit>() {
            sorafs::visit_draw_reserve_credit(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RepaySorafsReserveCredit>() {
            sorafs::visit_repay_reserve_credit(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SubmitSorafsReserveAppeal>() {
            sorafs::visit_submit_reserve_appeal(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<DecideSorafsReserveAppeal>() {
            sorafs::visit_decide_reserve_appeal(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetSorafsPopIssuerPolicy>() {
            sorafs::visit_set_pop_issuer_policy(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<CommitSorafsPopCredentialBatch>() {
            sorafs::visit_commit_pop_credential_batch(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<PublishSorafsPopRevocationList>() {
            sorafs::visit_publish_pop_revocation_list(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetSorafsModerationPolicy>() {
            sorafs::visit_set_moderation_policy(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SubmitSorafsModerationAppeal>() {
            sorafs::visit_submit_moderation_appeal(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterSorafsModerationJurorEligibility>() {
            sorafs::visit_register_moderation_juror_eligibility(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<FinalizeSorafsModerationSortition>() {
            sorafs::visit_finalize_moderation_sortition(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<AcceptSorafsModerationJurorAssignment>() {
            sorafs::visit_accept_moderation_juror_assignment(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ActivateSorafsModerationCase>() {
            sorafs::visit_activate_moderation_case(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SubmitSorafsModerationCommit>() {
            sorafs::visit_submit_moderation_commit(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RaiseSorafsModerationChallenge>() {
            sorafs::visit_raise_moderation_challenge(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ResolveSorafsModerationChallenge>() {
            sorafs::visit_resolve_moderation_challenge(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ExpireSorafsModerationChallenge>() {
            sorafs::visit_expire_moderation_challenge(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SubmitSorafsModerationReveal>() {
            sorafs::visit_submit_moderation_reveal(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<FinalizeSorafsModerationCase>() {
            sorafs::visit_finalize_moderation_case(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<UnregisterBox>() {
            executor.visit_unregister(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<Upgrade>() {
            executor.visit_upgrade(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<CustomInstruction>() {
            executor.visit_custom_instruction(isi);
            return;
        }
        deny!(executor, "unexpected instruction type");
    }
}
/// Permission-checked visitors for native settlement instructions.
pub mod settlement {
    use super::*;
    use iroha_executor_data_model::permission::settlement::{
        CanManageFxCorridors, CanSetFxCorridorPolicy,
    };
    /// Dispatch a settlement instruction, gating policy updates and deferring settlement-source
    /// authorization to Core.
    pub fn visit_settlement_instruction<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SettlementInstructionBox,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        let authority = &executor.context().authority;
        match isi {
            SettlementInstructionBox::SetFxCorridorPolicy(set) => {
                let exact = CanSetFxCorridorPolicy {
                    policy_id: set.policy().policy_id.clone(),
                };
                if CanManageFxCorridors.is_owned_by(authority, executor.host())
                    || exact.is_owned_by(authority, executor.host())
                {
                    execute!(executor, isi);
                }
                deny!(
                    executor,
                    "FX corridor policy updates require an exact typed policy permission"
                );
            }
            // Core binds native FX funding/refunds to the immutable owner and settlement source
            // debits to the signing account; no manager permission can authorize either debit.
            SettlementInstructionBox::FundFxCorridorEscrow(_)
            | SettlementInstructionBox::RefundFxCorridorEscrow(_)
            | SettlementInstructionBox::SettleFxCorridor(_) => execute!(executor, isi),
            SettlementInstructionBox::Dvp(_) | SettlementInstructionBox::Pvp(_) => {
                execute!(executor, isi);
            }
        }
    }
}
#[cfg(test)]
mod core_authorization_dispatch_tests {
    use super::*;
    use crate::{Iroha, prelude};
    use core::num::NonZeroU64;
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        alias_setup::{
            AliasDataSpaceIntentV1, AliasIntentV1, AliasLeaseAcquisitionV1, AliasQuoteGuardV1,
            AliasTargetV1, ResolvedAccountAliasV1, ResolvedDataSpaceV1,
        },
        block::BlockHeader,
        isi::{
            alias_setup::{
                CompareAndSetPrimaryAccountAlias, ConfigureAliasAutoRenew, EnsureAlias,
                RebindAccountAlias, RenewAliasLease,
            },
            settlement::{FxCorridorOracleEvidence, SettleFxCorridor},
        },
        nexus::DataSpaceId,
        offline::{
            KagemushaDevicePublicKeyV2, OfflineDeviceAttestationPolicy,
            OfflineDeviceAttestationRegistration,
        },
        oracle::{FeedConfigVersion, FeedEvent, FeedEventOutcome, FeedSuccess, ObservationValue},
        prelude::{AccountId, AssetDefinitionId, DomainId, Quantity, ValidationFail},
    };
    #[derive(Debug)]
    struct TestExecutor {
        host: Iroha,
        context: prelude::Context,
        verdict: crate::data_model::executor::Result<(), ValidationFail>,
    }
    impl TestExecutor {
        fn new(authority: AccountId) -> Self {
            Self {
                host: Iroha,
                context: prelude::Context {
                    authority,
                    curr_block: BlockHeader::new(
                        NonZeroU64::new(2).expect("non-genesis block height"),
                        None,
                        None,
                        None,
                        0,
                        0,
                    ),
                },
                verdict: Ok(()),
            }
        }
    }
    impl Execute for TestExecutor {
        fn host(&self) -> &Iroha {
            &self.host
        }
        fn context(&self) -> &prelude::Context {
            &self.context
        }
        fn context_mut(&mut self) -> &mut prelude::Context {
            &mut self.context
        }
        fn verdict(&self) -> &crate::data_model::executor::Result<(), ValidationFail> {
            &self.verdict
        }
        fn deny(&mut self, reason: ValidationFail) {
            self.verdict = Err(reason);
        }
    }
    impl Visit for TestExecutor {
        fn visit_ensure_alias(&mut self, operation: &EnsureAlias) {
            super::visit_ensure_alias(self, operation);
        }
        fn visit_renew_alias_lease(&mut self, operation: &RenewAliasLease) {
            super::visit_renew_alias_lease(self, operation);
        }
        fn visit_configure_alias_auto_renew(&mut self, operation: &ConfigureAliasAutoRenew) {
            super::visit_configure_alias_auto_renew(self, operation);
        }
        fn visit_rebind_account_alias(&mut self, operation: &RebindAccountAlias) {
            super::visit_rebind_account_alias(self, operation);
        }
        fn visit_compare_and_set_primary_account_alias(
            &mut self,
            operation: &CompareAndSetPrimaryAccountAlias,
        ) {
            super::visit_compare_and_set_primary_account_alias(self, operation);
        }
    }
    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked FX executor fixture keypair");
        AccountId::new(key_pair.public_key().clone())
    }
    fn asset(domain: &str, name: &str) -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new(domain, "universal").expect("valid FX asset domain"),
            name.parse().expect("valid FX asset name"),
        )
    }
    fn offline_attestation_registration(
        account_id: AccountId,
    ) -> OfflineDeviceAttestationRegistration {
        let public_key = KagemushaDevicePublicKeyV2::from_sec1_bytes(&[
            0x04, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63,
            0xa4, 0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39,
            0x45, 0xd8, 0x98, 0xc2, 0x96, 0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e,
            0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e, 0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e,
            0xce, 0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51, 0xf5,
        ])
        .expect("canonical uncompressed P-256 generator point");
        let attestation_report = b"executor-offline-attestation-report".to_vec();
        let evidence = b"executor-offline-attestation-evidence".to_vec();
        OfflineDeviceAttestationRegistration {
            version: 1,
            platform: "android-keymint".to_owned(),
            key_id: "executor-offline-key".to_owned(),
            device_id: "executor-offline-device".to_owned(),
            account_id,
            asset_definition_id: None,
            ios_team_id: None,
            ios_bundle_id: None,
            ios_environment: None,
            android_package_name: Some("org.hyperledger.iroha.executor".to_owned()),
            android_signing_certificate_sha256: Some(vec![0x51; 32]),
            public_key,
            assertion_scheme: "android-keymint".to_owned(),
            assertion_key_algorithm: "ecdsa-p256-sha256".to_owned(),
            assertion_public_key: vec![0x52; 65],
            assertion_usage_count_limit: Some(1),
            one_use: true,
            challenge_hash: Hash::new(b"executor-offline-attestation-challenge"),
            attestation_report_hash: Hash::new(&attestation_report),
            attestation_report,
            evidence_hash: Hash::new(&evidence),
            evidence,
            recent_block_height: 42,
            recent_block_hash: Hash::new(b"executor-offline-attestation-block"),
            expires_at_ms: 2_000_000_000_000,
        }
    }
    #[test]
    fn fx_settlement_reaches_core_without_executor_permission() {
        let authority = account(0x41);
        let request_hash = Hash::new(b"executor-fx-oracle-request");
        let oracle_event = FeedEvent {
            feed_id: "mobile_aed_pkr_rate".parse().expect("valid feed id"),
            feed_config_version: FeedConfigVersion(1),
            slot: 1,
            request_hash,
            outcome: FeedEventOutcome::Success(FeedSuccess {
                value: ObservationValue::new(76, 0),
                entries: Vec::new(),
            }),
        };
        let settlement = SettlementInstructionBox::SettleFxCorridor(SettleFxCorridor {
            policy_id: "mobile_aed_pkr".parse().expect("valid FX policy name"),
            expected_policy_revision: 1,
            source_asset_definition_id: asset("cbuae", "aed"),
            destination_asset_definition_id: asset("sbp", "pkr"),
            settlement_id: "mobile_fx_1".parse().expect("valid settlement id"),
            recipient: account(0x42),
            source_amount: Quantity::from(10_u32),
            expected_destination_amount: Quantity::from(760_u32),
            oracle_evidence: FxCorridorOracleEvidence {
                feed_id: oracle_event.feed_id.clone(),
                feed_config_version: oracle_event.feed_config_version,
                slot: oracle_event.slot,
                request_hash: oracle_event.request_hash,
                event_hash: HashOf::new(&oracle_event),
            },
        });
        let mut executor = TestExecutor::new(authority);
        settlement::visit_settlement_instruction(&mut executor, &settlement);
        assert!(
            executor.verdict().is_ok(),
            "the default executor must defer FX source authorization to Core"
        );
    }
    #[test]
    fn offline_attestation_instructions_reach_core_authorization() {
        let authority = account(0x43);
        let instructions = [
            InstructionBox::from(RegisterOfflineDeviceAttestation::new(
                offline_attestation_registration(authority.clone()),
            )),
            SetOfflineDeviceAttestationPolicy::new(OfflineDeviceAttestationPolicy {
                version: 1,
                trusted_roots: Vec::new(),
                revoked_certificate_tbs_sha256: Vec::new(),
                ios_apps: Vec::new(),
                android_apps: Vec::new(),
                android_status_snapshot: None,
                require_ios_app_policy: false,
                require_android_app_policy: false,
            })
            .into(),
        ];
        for instruction in instructions {
            let mut executor = TestExecutor::new(authority.clone());
            visit_instruction(&mut executor, &instruction);
            assert!(
                executor.verdict().is_ok(),
                "offline instructions must reach Core authorization"
            );
        }
    }
    #[test]
    fn default_executor_forwards_the_complete_kagemusha_canary_lifecycle() {
        let source = include_str!("mod.rs");
        let start = source
            .find("// Core owns offline note/device validation")
            .expect("Kagemusha dispatch marker");
        let tail = &source[start..];
        let end = tail
            .find("// Core owns the signature, chain/client binding")
            .expect("Kagemusha dispatch terminator");
        let dispatch = &tail[..end];
        for instruction in [
            "ActivateKagemushaRecursiveReleaseV4",
            "EnableKagemushaRecursiveIssuanceV4",
            "CancelKagemushaRecursiveReleaseV4",
            "DeactivateKagemushaRecursiveIssuanceV4",
            "AuthorizeKagemushaTairaCanaryV4",
            "RecordKagemushaTairaCanaryV4",
        ] {
            assert!(
                dispatch.contains(instruction),
                "default executor Kagemusha dispatch omitted {instruction}"
            );
        }
    }
    #[test]
    fn alias_lifecycle_instructions_reach_core_dispatch() {
        let authority = account(0x44);
        let replacement = account(0x45);
        let dataspace = ResolvedDataSpaceV1::new(
            "universal".parse().expect("canonical dataspace alias"),
            DataSpaceId::UNIVERSAL,
        );
        let target = AliasTargetV1::Dataspace(dataspace.clone());
        let account_alias = ResolvedAccountAliasV1::new(
            "merchant@universal"
                .parse()
                .expect("canonical account alias"),
            DataSpaceId::UNIVERSAL,
        );
        let guard = AliasQuoteGuardV1 {
            expected_policy_version: 1,
            expected_payment_asset: "61CtjvNd9T3THAR65GsMVHr82Bjc"
                .parse()
                .expect("payment asset definition id"),
            max_amount: Quantity::one(),
            valid_until_ms: u64::MAX,
        };
        let instructions: Vec<InstructionBox> = vec![
            EnsureAlias::new(
                AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
                    dataspace,
                    owner: authority.clone(),
                }),
                AliasLeaseAcquisitionV1::new(1, None),
                guard.clone(),
            )
            .into(),
            RenewAliasLease::new(target.clone(), 1, 2, guard).into(),
            ConfigureAliasAutoRenew::new(target, 0, None).into(),
            RebindAccountAlias::new(account_alias.clone(), authority.clone(), replacement).into(),
            CompareAndSetPrimaryAccountAlias::new(authority.clone(), None, Some(account_alias))
                .into(),
        ];
        for instruction in instructions {
            let mut executor = TestExecutor::new(authority.clone());
            visit_instruction(&mut executor, &instruction);
            assert!(
                executor.verdict().is_ok(),
                "registered alias instruction must reach Core dispatch: {instruction:?}"
            );
        }
    }
}
/// Permission-aware dispatch for SCCP governance proposal instructions.
pub mod governance {
    use super::*;
    declare_execute_visitors! {
        /// Dispatch a typed SCCP route-governance proposal to Core, which admits registered citizens
        /// or holders of `CanProposeSccpRouteGovernance` (including role grants).
        visit_propose_sccp_route_governance(ProposeSccpRouteGovernance);
        /// Dispatch a typed `SoraFS` provider-owner proposal to Core.
        ///
        /// Core admits bonded citizens as proposal authors; only exact-due automatic execution of
        /// a successful Parliament certificate can mutate the owner registry.
        visit_propose_sorafs_provider_governance(ProposeSorafsProviderGovernance);
        /// Dispatch a bonded-citizen validation-fee proposal to the Parliament lifecycle in Core.
        visit_propose_validation_fee_policy(ProposeValidationFeePolicy);
        /// Dispatch a bonded-citizen payout-lifecycle proposal to the Parliament lifecycle in Core.
        visit_propose_validation_fee_payout_lifecycle(ProposeValidationFeePayoutLifecycle);
        /// Dispatch citizen registration to Core, which enforces self-registration and the configured
        /// citizenship bond floor against committed governance parameters.
        visit_register_citizen(RegisterCitizen);
    }
}
/// Permission-checked visitors for peer management instructions.
pub mod peer {
    use super::*;
    use iroha_executor_data_model::permission::peer::CanManagePeers;
    /// Registers a peer when genesis or a peer manager submits the instruction.
    pub fn visit_register_peer<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterPeerWithPop,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManagePeers.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't register peer");
    }
    /// Unregisters a peer if the caller has peer management privileges.
    pub fn visit_unregister_peer<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<Peer>,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManagePeers.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't unregister peer");
    }
}
/// Permission-checked visitors for public-lane validator lifecycle instructions.
pub mod staking {
    use super::*;
    use iroha_executor_data_model::permission::peer::CanManagePeers;
    /// Register a public-lane validator when the caller is authorised or during genesis.
    pub fn visit_register_public_lane_validator<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterPublicLaneValidator,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManagePeers.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't register public-lane validator");
    }
    /// Activate a pending public-lane validator when the caller is authorised or during genesis.
    pub fn visit_activate_public_lane_validator<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ActivatePublicLaneValidator,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManagePeers.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't activate public-lane validator");
    }
    /// Mark a validator as exiting when the caller is authorised or during genesis.
    pub fn visit_exit_public_lane_validator<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ExitPublicLaneValidator,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManagePeers.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't exit public-lane validator");
    }
}
/// Permission-checked visitors for Nexus lane relay recovery instructions.
pub mod nexus {
    use super::*;
    use iroha_executor_data_model::permission::peer::CanManageLaneRelayEmergency;
    /// Set or clear emergency lane relay validators when the caller is authorised or during genesis.
    pub fn visit_set_lane_relay_emergency_validators<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetLaneRelayEmergencyValidators,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManageLaneRelayEmergency.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't set lane relay emergency validators");
    }
}
/// Permission-checked visitors for `SoraFS` registry and pricing instructions.
pub mod sorafs {
    use super::*;
    use iroha_executor_data_model::permission::sorafs::{
        CanBindSorafsAlias, CanCompleteSorafsReplicationOrder, CanFileSorafsCapacityDispute,
        CanIssueSorafsReplicationOrder, CanManageSorafsModeration, CanManageSorafsPopRegistry,
        CanManageSorafsReputationJournalPolicy, CanOperateSorafsPopIssuer,
        CanRecordSorafsReputationJournal, CanResolveSorafsCapacityDispute, CanSetSorafsPricing,
        CanSetSorafsReservePolicy, CanUpsertSorafsProviderCredit,
    };
    use iroha_smart_contract::data_model::query::sorafs::prelude::{
        FindSorafsModerationAppeal, FindSorafsModerationCase, FindSorafsModerationChallenge,
        FindSorafsModerationCommit, FindSorafsModerationEvents,
        FindSorafsModerationJurorEligibility, FindSorafsModerationNoShow,
        FindSorafsModerationOutcome, FindSorafsModerationPolicy, FindSorafsModerationReveal,
        FindSorafsModerationSnapshot, FindSorafsModerationStatus,
        FindSorafsOrderbookCancellationByOrderId, FindSorafsOrderbookChannelById,
        FindSorafsOrderbookChannels, FindSorafsOrderbookEvents, FindSorafsOrderbookOrderById,
        FindSorafsOrderbookOrders, FindSorafsOrderbookPolicy, FindSorafsOrderbookReceiptById,
        FindSorafsOrderbookReceipts, FindSorafsOrderbookStatus, FindSorafsOrderbookTradeById,
        FindSorafsOrderbookTrades, FindSorafsPopAuditDigestBySequence,
        FindSorafsPopCommitmentRootByVersion, FindSorafsPopCredentialCommitmentByDigest,
        FindSorafsPopIssuerPolicy, FindSorafsPopRegistryStatus,
        FindSorafsPopRevocationByNonceCommitment, FindSorafsPopRevocationPublicationByVersion,
        FindSorafsRepairEvents, FindSorafsRepairStatus, FindSorafsRepairTask,
        FindSorafsRepairTasks, FindSorafsReputationJournalAuthorityPolicy,
        FindSorafsReputationJournalEventBySourceId, FindSorafsReputationJournalEvents,
        FindSorafsReserveAppealById, FindSorafsReserveEvents, FindSorafsReserveMovementById,
        FindSorafsReservePolicy, FindSorafsReserveProviderById,
    };
    declare_query_visitors! {
        no_op;
        /// Authoritative repair tasks are public operational state.
        visit_find_sorafs_repair_task(FindSorafsRepairTask);
        /// Authoritative repair-task pages are public operational state.
        visit_find_sorafs_repair_tasks(FindSorafsRepairTasks);
        /// Authoritative repair counters are public operational state.
        visit_find_sorafs_repair_status(FindSorafsRepairStatus);
        /// Committed repair-ledger event pages are public operational state.
        visit_find_sorafs_repair_events(FindSorafsRepairEvents);
        /// The payload-free finalized reputation journal is public transparency state.
        visit_find_sorafs_reputation_journal_events(FindSorafsReputationJournalEvents);
        /// One payload-free finalized reputation source result is public transparency state.
        visit_find_sorafs_reputation_journal_event_by_source_id(
            FindSorafsReputationJournalEventBySourceId
        );
    }
    /// Validate permission to read the active reputation-journal authority policy.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "the generated Visit dispatch ABI passes every query operation by shared reference"
    )]
    pub fn visit_find_sorafs_reputation_journal_authority_policy<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        _query: &FindSorafsReputationJournalAuthorityPolicy,
    ) {
        if executor.context().curr_block.is_genesis()
            || CanManageSorafsReputationJournalPolicy
                .is_owned_by(&executor.context().authority, executor.host())
            || CanRecordSorafsReputationJournal
                .is_owned_by(&executor.context().authority, executor.host())
            || CanResolveSorafsCapacityDispute
                .is_owned_by(&executor.context().authority, executor.host())
        {
            return;
        }
        deny!(
            executor,
            "Can't read the active authoritative SoraFS reputation-journal authority policy"
        );
    }
    fn visit_orderbook_read<V: Execute + Visit + ?Sized>(executor: &mut V) {
        if executor.context().curr_block.is_genesis()
            || CanSetSorafsPricing.is_owned_by(&executor.context().authority, executor.host())
            || CanCompleteSorafsReplicationOrder
                .is_owned_by(&executor.context().authority, executor.host())
        {
            return;
        }
        deny!(executor, "Can't read authoritative SoraFS orderbook state");
    }
    declare_query_visitors! {
        via visit_orderbook_read;
        /// Validate permission to read the active authoritative orderbook policy.
        visit_find_sorafs_orderbook_policy(FindSorafsOrderbookPolicy);
        /// Validate permission to read an authoritative order.
        visit_find_sorafs_orderbook_order_by_id(FindSorafsOrderbookOrderById);
        /// Validate permission to read an authoritative cancellation.
        visit_find_sorafs_orderbook_cancellation_by_order_id(
            FindSorafsOrderbookCancellationByOrderId
        );
        /// Validate permission to read an authoritative settlement receipt.
        visit_find_sorafs_orderbook_receipt_by_id(FindSorafsOrderbookReceiptById);
        /// Validate permission to read an authoritative matched trade.
        visit_find_sorafs_orderbook_trade_by_id(FindSorafsOrderbookTradeById);
        /// Validate permission to read an authoritative settlement channel.
        visit_find_sorafs_orderbook_channel_by_id(FindSorafsOrderbookChannelById);
        /// Validate permission to read authoritative orderbook counters.
        visit_find_sorafs_orderbook_status(FindSorafsOrderbookStatus);
        /// Validate permission to list authoritative orders.
        visit_find_sorafs_orderbook_orders(FindSorafsOrderbookOrders);
        /// Validate permission to list authoritative settlement receipts.
        visit_find_sorafs_orderbook_receipts(FindSorafsOrderbookReceipts);
        /// Validate permission to list authoritative matched trades.
        visit_find_sorafs_orderbook_trades(FindSorafsOrderbookTrades);
        /// Validate permission to list authoritative settlement channels.
        visit_find_sorafs_orderbook_channels(FindSorafsOrderbookChannels);
        /// Validate permission to list committed authoritative orderbook events.
        visit_find_sorafs_orderbook_events(FindSorafsOrderbookEvents);
    }
    fn visit_reserve_read<V: Execute + Visit + ?Sized>(executor: &mut V) {
        if executor.context().curr_block.is_genesis()
            || CanSetSorafsReservePolicy.is_owned_by(&executor.context().authority, executor.host())
        {
            return;
        }
        deny!(executor, "Can't read authoritative SoraFS reserve state");
    }
    declare_query_visitors! {
        via visit_reserve_read;
        /// Validate permission to read the active reserve policy.
        visit_find_sorafs_reserve_policy(FindSorafsReservePolicy);
        /// Validate permission to read a provider reserve account.
        visit_find_sorafs_reserve_provider_by_id(FindSorafsReserveProviderById);
        /// Validate permission to read a reserve custody movement.
        visit_find_sorafs_reserve_movement_by_id(FindSorafsReserveMovementById);
        /// Validate permission to read a reserve lifecycle appeal.
        visit_find_sorafs_reserve_appeal_by_id(FindSorafsReserveAppealById);
        /// Validate permission to list provider reserve accounts.
        visit_find_sorafs_reserve_providers(FindSorafsReserveProviders);
        /// Validate permission to list reserve custody movements.
        visit_find_sorafs_reserve_movements(FindSorafsReserveMovements);
        /// Validate permission to list reserve lifecycle appeals.
        visit_find_sorafs_reserve_appeals(FindSorafsReserveAppeals);
        /// Validate permission to list committed authoritative reserve events.
        visit_find_sorafs_reserve_events(FindSorafsReserveEvents);
    }
    declare_query_visitors! {
        no_op;
        /// `PoP` issuer policy is public transparency state.
        visit_find_sorafs_pop_issuer_policy(FindSorafsPopIssuerPolicy);
        /// Payload-free credential commitments are public transparency state.
        visit_find_sorafs_pop_credential_commitment_by_digest(
            FindSorafsPopCredentialCommitmentByDigest
        );
        /// Signed commitment-root publications are public transparency state.
        visit_find_sorafs_pop_commitment_root_by_version(FindSorafsPopCommitmentRootByVersion);
        /// Signed revocation publications are public transparency state.
        visit_find_sorafs_pop_revocation_publication_by_version(
            FindSorafsPopRevocationPublicationByVersion
        );
        /// Payload-free revocation commitments are public transparency state.
        visit_find_sorafs_pop_revocation_by_nonce_commitment(
            FindSorafsPopRevocationByNonceCommitment
        );
        /// Registry audit links are public transparency state.
        visit_find_sorafs_pop_audit_digest_by_sequence(FindSorafsPopAuditDigestBySequence);
        /// Registry anchors and counters are public transparency state.
        visit_find_sorafs_pop_registry_status(FindSorafsPopRegistryStatus);
        /// Authoritative moderation policy is public transparency state.
        visit_find_sorafs_moderation_policy(FindSorafsModerationPolicy);
        /// Appeal intake, pinned roots, and deterministic roster are public transparency state.
        visit_find_sorafs_moderation_appeal(FindSorafsModerationAppeal);
    }
    /// A payload-free eligibility record is visible to its juror and moderation operators.
    pub fn visit_find_sorafs_moderation_juror_eligibility<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        query: &FindSorafsModerationJurorEligibility,
    ) {
        if executor.context().curr_block.is_genesis()
            || executor.context().authority == query.juror
            || can_manage_moderation(executor)
        {
            return;
        }
        deny!(
            executor,
            "Can't read another juror's moderation PoP eligibility record"
        );
    }
    declare_query_visitors! {
        no_op;
        /// Authoritative moderation case headers are public transparency state.
        visit_find_sorafs_moderation_case(FindSorafsModerationCase);
        /// Sealed commitment digests and provenance are public transparency state.
        visit_find_sorafs_moderation_commit(FindSorafsModerationCommit);
        /// Accepted reveals are public after their commit-bound submission.
        visit_find_sorafs_moderation_reveal(FindSorafsModerationReveal);
        /// Payload-free challenge records are public transparency state.
        visit_find_sorafs_moderation_challenge(FindSorafsModerationChallenge);
        /// Terminal moderation outcomes are public transparency state.
        visit_find_sorafs_moderation_outcome(FindSorafsModerationOutcome);
        /// Derived no-show penalty records are public transparency state.
        visit_find_sorafs_moderation_no_show(FindSorafsModerationNoShow);
        /// Authoritative moderation counters are public transparency state.
        visit_find_sorafs_moderation_status(FindSorafsModerationStatus);
    }
    /// A complete snapshot includes every juror eligibility record and requires moderation access.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "the generated Visit dispatch ABI passes every query operation by shared reference"
    )]
    pub fn visit_find_sorafs_moderation_snapshot<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        _query: &FindSorafsModerationSnapshot,
    ) {
        if executor.context().curr_block.is_genesis() || can_manage_moderation(executor) {
            return;
        }
        deny!(
            executor,
            "Can't read the complete authoritative SoraFS moderation snapshot"
        );
    }
    declare_query_visitors! {
        no_op;
        /// Payload-free committed moderation events are public transparency state.
        visit_find_sorafs_moderation_events(FindSorafsModerationEvents);
    }
    declare_execute_visitors! {
        /// Register a `SoraFS` pin manifest.
        ///
        /// Public submissions rely on the universal-lane Nexus fee schedule instead
        /// of an additional executor permission gate.
        visit_register_pin_manifest(RegisterPinManifest);
        /// Submit a threshold-signed approval for a pending `SoraFS` pin manifest.
        ///
        /// Core validates the governed approval envelope. The submitting account
        /// does not receive broad pin-registry authority merely by relaying it.
        visit_approve_pin_manifest(ApprovePinManifest);
        /// Retire an account-owned `SoraFS` pin manifest.
        ///
        /// Core requires the authenticated transaction authority to be the exact
        /// original submitter.
        visit_retire_pin_manifest(RetirePinManifest);
    }
    /// Bind or update a `SoraFS` manifest alias when permitted.
    pub fn visit_bind_manifest_alias<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &BindManifestAlias,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanBindSorafsAlias.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't bind SoraFS manifest alias");
    }
    declare_execute_visitors! {
        /// Register a capacity declaration when permitted.
        visit_register_capacity_declaration(RegisterCapacityDeclaration);
        /// Record a capacity telemetry snapshot when permitted.
        visit_record_capacity_telemetry(RecordCapacityTelemetry);
    }
    /// File a capacity dispute when permitted.
    pub fn visit_register_capacity_dispute<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterCapacityDispute,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanFileSorafsCapacityDispute.is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't file SoraFS capacity dispute");
    }
    /// Resolve an authoritative capacity dispute when permitted.
    pub fn visit_resolve_capacity_dispute<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ResolveSorafsCapacityDispute,
    ) {
        if executor.context().curr_block.is_genesis()
            || CanResolveSorafsCapacityDispute
                .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't resolve SoraFS capacity dispute");
    }
    /// Activate or rotate the governed reputation-recorder policy when permitted.
    pub fn visit_set_reputation_journal_authority_policy<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetSorafsReputationJournalAuthorityPolicy,
    ) {
        if executor.context().curr_block.is_genesis()
            || CanManageSorafsReputationJournalPolicy
                .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't manage the authoritative SoraFS reputation recorder policy"
        );
    }
    /// Append a governed `PoR` reputation projection when permitted.
    pub fn visit_append_por_reputation_journal_entry<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &AppendSorafsPorReputationJournalEntry,
    ) {
        if executor.context().curr_block.is_genesis()
            || CanRecordSorafsReputationJournal
                .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't record an authoritative SoraFS reputation event"
        );
    }
    /// Append a governed stream-token reputation projection when permitted.
    pub fn visit_append_stream_token_reputation_journal_entry<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &AppendSorafsStreamTokenReputationJournalEntry,
    ) {
        if executor.context().curr_block.is_genesis()
            || CanRecordSorafsReputationJournal
                .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't record an authoritative SoraFS reputation event"
        );
    }
    /// Issue a replication order when permitted.
    pub fn visit_issue_replication_order<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &IssueReplicationOrder,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanIssueSorafsReplicationOrder
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't issue SoraFS replication order");
    }
    /// Complete a replication order when permitted.
    pub fn visit_complete_replication_order<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &CompleteReplicationOrder,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanCompleteSorafsReplicationOrder
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't complete SoraFS replication order");
    }
    /// Revise a pending replication order's assignments when permitted.
    pub fn visit_revise_replication_order_assignments<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ReviseReplicationOrderAssignments,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanIssueSorafsReplicationOrder
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't revise SoraFS replication order assignments"
        );
    }
    /// Expire a replication order when permitted.
    pub fn visit_expire_replication_order<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ExpireReplicationOrder,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanIssueSorafsReplicationOrder
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't expire SoraFS replication order");
    }
    declare_execute_visitors! {
        /// Dispatch the retired direct owner-registration surface so Core can reject it uniformly.
        visit_register_provider_owner(RegisterProviderOwner);
        /// Dispatch the retired direct owner-removal surface so Core can reject it uniformly.
        visit_unregister_provider_owner(UnregisterProviderOwner);
        /// Dispatch completion-authority rotation; Core requires the exact governed owner.
        visit_set_provider_ingest_completion_authority(SetProviderIngestCompletionAuthority);
        /// Dispatch completion-authority revocation; Core requires the exact governed owner.
        visit_revoke_provider_ingest_completion_authority(
            RevokeProviderIngestCompletionAuthority
        );
    }
    /// Update the `SoraFS` pricing schedule when permitted.
    pub fn visit_set_pricing_schedule<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetPricingSchedule,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanSetSorafsPricing.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't set SoraFS pricing schedule");
    }
    /// Upsert a `SoraFS` provider credit record when permitted.
    pub fn visit_upsert_provider_credit<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &UpsertProviderCredit,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanUpsertSorafsProviderCredit.is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't upsert SoraFS provider credit");
    }
    declare_execute_visitors! {
        /// Submit an authority-bound repair report; native execution enforces the
        /// provider-scoped operator permission and source identity.
        visit_submit_repair_task(SubmitSorafsRepairTask);
        /// Apply a revision-checked repair action; native execution enforces lease
        /// ownership, expiry, and provider scope.
        visit_apply_repair_task_action(ApplySorafsRepairTaskAction);
        /// Submit the single provider-owner appeal against an escalated repair.
        visit_submit_repair_appeal(SubmitSorafsRepairAppeal);
    }
    /// Activate the next authoritative orderbook policy revision when permitted.
    pub fn visit_set_orderbook_policy<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetSorafsOrderbookPolicy,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanSetSorafsPricing.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't set SoraFS orderbook policy");
    }
    declare_execute_visitors! {
        /// Submit a signed order; native execution enforces owner and signer binding.
        visit_submit_orderbook_order(SubmitSorafsOrderbookOrder);
        /// Cancel an order; native execution enforces owner and signer binding.
        visit_cancel_orderbook_order(CancelSorafsOrderbookOrder);
    }
    /// Record a settlement receipt when the matcher/settlement authority is permitted.
    pub fn visit_record_orderbook_settlement_receipt<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RecordSorafsOrderbookSettlementReceipt,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanCompleteSorafsReplicationOrder
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't record SoraFS orderbook settlement receipt");
    }
    /// Run deterministic order matching when settlement permission is present.
    pub fn visit_match_orderbook<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &MatchSorafsOrderbook,
    ) {
        if executor.context().curr_block.is_genesis()
            || CanCompleteSorafsReplicationOrder
                .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't match authoritative SoraFS orders");
    }
    /// Expire orders and settlement channels when settlement permission is present.
    pub fn visit_maintain_orderbook<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &MaintainSorafsOrderbook,
    ) {
        if executor.context().curr_block.is_genesis()
            || CanCompleteSorafsReplicationOrder
                .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't maintain the authoritative SoraFS orderbook"
        );
    }
    fn execute_with_reserve_governance<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &(impl iroha_smart_contract::data_model::isi::Instruction + norito::NoritoSerialize),
    ) {
        if executor.context().curr_block.is_genesis()
            || CanSetSorafsReservePolicy.is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't govern the authoritative SoraFS reserve ledger"
        );
    }
    /// Activate the next reserve policy revision when governance is permitted.
    pub fn visit_set_reserve_policy<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetSorafsReservePolicy,
    ) {
        execute_with_reserve_governance(executor, isi);
    }
    declare_execute_visitors! {
        /// Register a provider reserve partition through the exact governed service account.
        visit_register_reserve_account(RegisterSorafsReserveAccount);
        /// Admit a provider-signed reserve movement request.
        visit_request_reserve_movement(RequestSorafsReserveMovement);
        /// Decide a reserve movement through the exact governed decision account.
        visit_decide_reserve_movement(DecideSorafsReserveMovement);
        /// Charge deterministic provider rent through the exact governed operations account.
        visit_charge_reserve_rent(ChargeSorafsReserveRent);
        /// Advance reserve lifecycle state through the exact governed operations account.
        visit_advance_reserve_lifecycle(AdvanceSorafsReserveLifecycle);
        /// Draw protocol reserve credit through the exact governed operations account.
        visit_draw_reserve_credit(DrawSorafsReserveCredit);
        /// Admit a provider-signed credit repayment.
        visit_repay_reserve_credit(RepaySorafsReserveCredit);
        /// Admit a provider-signed reserve lifecycle appeal.
        visit_submit_reserve_appeal(SubmitSorafsReserveAppeal);
        /// Decide a reserve lifecycle appeal through the exact governed decision account.
        visit_decide_reserve_appeal(DecideSorafsReserveAppeal);
    }
    /// Activate a `PoP` issuer policy when governance permission is present.
    pub fn visit_set_pop_issuer_policy<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetSorafsPopIssuerPolicy,
    ) {
        if executor.context().curr_block.is_genesis()
            || CanManageSorafsPopRegistry
                .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't manage the authoritative SoraFS PoP issuer policy"
        );
    }
    /// Commit an issuer-authenticated credential batch when permitted.
    pub fn visit_commit_pop_credential_batch<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &CommitSorafsPopCredentialBatch,
    ) {
        if executor.context().curr_block.is_genesis()
            || CanOperateSorafsPopIssuer.is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't operate the authoritative SoraFS PoP issuer"
        );
    }
    /// Publish a signed revocation-list extension when permitted.
    pub fn visit_publish_pop_revocation_list<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &PublishSorafsPopRevocationList,
    ) {
        if executor.context().curr_block.is_genesis()
            || CanOperateSorafsPopIssuer.is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't operate the authoritative SoraFS PoP issuer"
        );
    }
    fn can_manage_moderation<V: Execute + Visit + ?Sized>(executor: &V) -> bool {
        executor.context().curr_block.is_genesis()
            || CanManageSorafsModeration.is_owned_by(&executor.context().authority, executor.host())
    }
    /// Activate a moderation policy when the caller is authorised.
    pub fn visit_set_moderation_policy<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetSorafsModerationPolicy,
    ) {
        if can_manage_moderation(executor) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't manage authoritative SoraFS moderation state"
        );
    }
    declare_execute_visitors! {
        /// Submit an authority-bound appeal intake; native execution checks appellant identity.
        visit_submit_moderation_appeal(SubmitSorafsModerationAppeal);
        /// Register an authority-bound private `PoP` eligibility proof.
        visit_register_moderation_juror_eligibility(RegisterSorafsModerationJurorEligibility);
    }
    /// Finalize deterministic panel sortition when the caller is authorised.
    pub fn visit_finalize_moderation_sortition<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &FinalizeSorafsModerationSortition,
    ) {
        if can_manage_moderation(executor) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't manage authoritative SoraFS moderation sortition"
        );
    }
    declare_execute_visitors! {
        /// Accept an authority-bound primary juror assignment.
        visit_accept_moderation_juror_assignment(AcceptSorafsModerationJurorAssignment);
    }
    /// Apply deterministic failover and activate the case when authorised.
    pub fn visit_activate_moderation_case<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ActivateSorafsModerationCase,
    ) {
        if can_manage_moderation(executor) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't activate authoritative SoraFS moderation cases"
        );
    }
    declare_execute_visitors! {
        /// Submit a juror commitment; native execution binds it to the authority.
        visit_submit_moderation_commit(SubmitSorafsModerationCommit);
        /// Raise an authenticated, bonded, payload-free public moderation challenge.
        visit_raise_moderation_challenge(RaiseSorafsModerationChallenge);
        /// Permissionlessly expire a pending challenge after its resolution grace.
        visit_expire_moderation_challenge(ExpireSorafsModerationChallenge);
    }
    /// Resolve a moderation challenge when the caller is authorised.
    pub fn visit_resolve_moderation_challenge<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ResolveSorafsModerationChallenge,
    ) {
        if can_manage_moderation(executor) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't manage authoritative SoraFS moderation state"
        );
    }
    declare_execute_visitors! {
        /// Submit a juror reveal; native execution verifies the stored commitment.
        visit_submit_moderation_reveal(SubmitSorafsModerationReveal);
    }
    /// Finalize a closed case when the caller is authorised.
    pub fn visit_finalize_moderation_case<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &FinalizeSorafsModerationCase,
    ) {
        if can_manage_moderation(executor) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't manage authoritative SoraFS moderation state"
        );
    }
}
/// Permission-checked visitors for domain lifecycle instructions.
pub mod domain {
    use super::*;
    use crate::permission::{
        account::is_account_owner, domain::is_domain_owner, revoke_permissions,
    };
    use iroha_executor_data_model::permission::domain::{
        CanModifyDomainMetadata, CanUnregisterDomain,
    };
    use iroha_smart_contract::data_model::{asset::AssetDefinitionId, domain::DomainId};
    /// Registers a domain only while applying genesis.
    ///
    /// Ordinary signed transactions must use the declarative `EnsureAlias` instruction so lease
    /// acquisition, catalog resolution, and ownership checks stay atomic.
    pub fn visit_register_domain<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Register<Domain>,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Raw domain registration is reserved for genesis; use EnsureAlias"
        );
    }
    /// Unregisters a domain after checking that the caller governs the domain or holds the revoke permission.
    pub fn visit_unregister_domain<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<Domain>,
    ) {
        let domain_id = isi.object();
        if executor.context().curr_block.is_genesis()
            || match is_domain_owner(domain_id, &executor.context().authority, executor.host()) {
                Err(err) => deny!(executor, err),
                Ok(is_domain_owner) => is_domain_owner,
            }
            || {
                let can_unregister_domain_token = CanUnregisterDomain {
                    domain: domain_id.clone(),
                };
                can_unregister_domain_token
                    .is_owned_by(&executor.context().authority, executor.host())
            }
        {
            let domain_asset_definition_ids =
                match asset_definition_ids_owned_by_domain(executor.host(), domain_id) {
                    Ok(ids) => ids,
                    Err(err) => deny!(executor, err),
                };
            let err = revoke_permissions(executor, |permission| {
                is_permission_domain_associated(permission, domain_id, &domain_asset_definition_ids)
            });
            if let Err(err) = err {
                deny!(executor, err);
            }
            execute!(executor, isi);
        }
        deny!(executor, "Can't unregister domain");
    }
    /// Transfers domain ownership when the caller owns the source account or domain.
    pub fn visit_transfer_domain<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Transfer<Account, DomainId, Account>,
    ) {
        let source_id = isi.source();
        let domain_id = isi.object();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_account_owner(source_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        match is_domain_owner(domain_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        deny!(executor, "Can't transfer domain of another account");
    }
    /// Sets domain metadata after verifying the caller's authority.
    pub fn visit_set_domain_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetKeyValue<Domain>,
    ) {
        let domain_id = isi.object();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_domain_owner(domain_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_set_key_value_in_domain_token = CanModifyDomainMetadata {
            domain: domain_id.clone(),
        };
        if can_set_key_value_in_domain_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't set key value in domain metadata");
    }
    /// Removes domain metadata when the caller holds the relevant modify permission.
    pub fn visit_remove_domain_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RemoveKeyValue<Domain>,
    ) {
        let domain_id = isi.object();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_domain_owner(domain_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_remove_key_value_in_domain_token = CanModifyDomainMetadata {
            domain: domain_id.clone(),
        };
        if can_remove_key_value_in_domain_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't remove key value in domain metadata");
    }
    fn asset_definition_ids_owned_by_domain(
        host: &Iroha,
        domain_id: &DomainId,
    ) -> Result<Vec<AssetDefinitionId>> {
        let mut ids = Vec::new();
        for result in host.query(FindAssetsDefinitions).execute()? {
            let definition = result?;
            if definition.owning_domain().as_ref() == Some(domain_id) {
                ids.push(definition.id().clone());
            }
        }
        Ok(ids)
    }
    #[allow(clippy::too_many_lines)]
    pub(crate) fn is_permission_domain_associated(
        permission: &Permission,
        domain_id: &DomainId,
        asset_definition_ids: &[AssetDefinitionId],
    ) -> bool {
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        let asset_definition_matches_domain =
            |definition: &AssetDefinitionId| asset_definition_ids.contains(definition);
        match permission {
            AnyPermission::CanUnregisterDomain(permission) => &permission.domain == domain_id,
            AnyPermission::CanModifyDomainMetadata(permission) => &permission.domain == domain_id,
            AnyPermission::CanRegisterAccount(permission) => &permission.domain == domain_id,
            AnyPermission::CanResolveAccountAlias(permission) => {
                match &permission.scope {
                    iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Domain(domain) => domain == domain_id,
                    iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Alias(alias) => {
                        alias.canonical_name.domain.as_ref() == Some(domain_id.name())
                            && &alias.canonical_name.dataspace == domain_id.dataspace()
                    }
                    iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Dataspace(_) => false,
                }
            }
            AnyPermission::CanDelegateAccountAliasResolution(permission) => {
                match &permission.scope {
                    iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Domain(domain) => domain == domain_id,
                    iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Alias(alias) => {
                        alias.canonical_name.domain.as_ref() == Some(domain_id.name())
                            && &alias.canonical_name.dataspace == domain_id.dataspace()
                    }
                    iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Dataspace(_) => false,
                }
            }
            AnyPermission::CanManageAccountAlias(permission) => {
                match &permission.scope {
                    iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Domain(domain) => domain == domain_id,
                    iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Alias(alias) => {
                        alias.canonical_name.domain.as_ref() == Some(domain_id.name())
                            && &alias.canonical_name.dataspace == domain_id.dataspace()
                    }
                    iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Dataspace(_) => false,
                }
            }
            AnyPermission::CanManageAssetDefinitionAlias(permission) => {
                match &permission.scope {
                    iroha_executor_data_model::permission::asset_definition::AssetDefinitionAliasPermissionScope::Domain(domain) => domain == domain_id,
                    iroha_executor_data_model::permission::asset_definition::AssetDefinitionAliasPermissionScope::Alias(alias) => {
                        alias.canonical_name.domain_segment() == Some(domain_id.name().as_ref())
                            && alias.canonical_name.dataspace_segment() == domain_id.dataspace().as_ref()
                    }
                    iroha_executor_data_model::permission::asset_definition::AssetDefinitionAliasPermissionScope::Dataspace(_) => false,
                }
            }
            AnyPermission::CanUnregisterAssetDefinition(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanModifyAssetDefinitionMetadata(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanManageAssetDefinitionConfidentialPolicy(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanMintAssetWithDefinition(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanBurnAssetWithDefinition(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanTransferAssetWithDefinition(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanModifyAssetMetadataWithDefinition(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanMintAssetToAccount(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanBurnAsset(permission) => {
                asset_definition_matches_domain(permission.asset.definition())
            }
            AnyPermission::CanTransferAsset(permission) => {
                asset_definition_matches_domain(permission.asset.definition())
            }
            AnyPermission::CanModifyAssetMetadata(permission) => {
                asset_definition_matches_domain(permission.asset.definition())
            }
            AnyPermission::CanSetAssetTransferAvailability(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanSetAssetTransferDailyLimit(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanSetAssetHoldingLimit(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanExecuteSettlement(permission) => {
                asset_definition_matches_domain(permission.debited_asset.definition())
            }
            AnyPermission::CanRegisterNft(permission) => &permission.domain == domain_id,
            AnyPermission::CanUnregisterNft(permission) => permission.nft.domain() == domain_id,
            AnyPermission::CanTransferNft(permission) => permission.nft.domain() == domain_id,
            AnyPermission::CanModifyNftMetadata(permission) => permission.nft.domain() == domain_id,
            AnyPermission::CanPublishSpaceDirectoryManifestForAccountDomain(permission) => {
                &permission.domain == domain_id
            }
            AnyPermission::DpnAdmin(_)
            | AnyPermission::DpnUser(_)
            | AnyPermission::DpnInori(_)
            | AnyPermission::DpnSettlement(_)
            | AnyPermission::DpnEprGuard(_)
            | AnyPermission::CanManageFeeSponsorProgram(_)
            | AnyPermission::CanEnrollFeeSponsorProgram(_)
            | AnyPermission::CanUnregisterAccount(_)
            | AnyPermission::CanModifyAccountMetadata(_)
            | AnyPermission::CanReplaceAccountController(_)
            | AnyPermission::CanReadAllLedgerData(_)
            | AnyPermission::CanReadAccountData(_)
            | AnyPermission::CanReadRestrictedDataspace(_)
            | AnyPermission::CanRegisterGlobalDataTrigger(_)
            | AnyPermission::CanRegisterTrigger(_)
            | AnyPermission::CanUnregisterTrigger(_)
            | AnyPermission::CanExecuteTrigger(_)
            | AnyPermission::CanModifyTrigger(_)
            | AnyPermission::CanModifyTriggerMetadata(_)
            | AnyPermission::CanManagePeers(_)
            | AnyPermission::CanManageLaneRelayEmergency(_)
            | AnyPermission::CanManageRuntimeUpgrades(_)
            | AnyPermission::CanManageConsensusKeys(_)
            | AnyPermission::CanManageConfidentialParams(_)
            | AnyPermission::CanRegisterDomain(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanSetHijiriParameters(_)
            | AnyPermission::CanManageSccpGovernance(_)
            | AnyPermission::CanProposeSccpRouteGovernance(_)
            | AnyPermission::CanManageOfflineEscrow(_)
            | AnyPermission::CanActivateKagemushaRecursiveReleaseV4(_)
            | AnyPermission::CanManageOfflineDeviceAttestationPolicy(_)
            | AnyPermission::CanManageRoles(_)
            | AnyPermission::CanUpgradeExecutor(_)
            | AnyPermission::CanRegisterSmartContractCode(_)
            | AnyPermission::CanInvokeContractEntrypoint(_)
            | AnyPermission::CanManageFxCorridors(_)
            | AnyPermission::CanSetFxCorridorPolicy(_)
            | AnyPermission::CanBindSorafsAlias(_)
            | AnyPermission::CanDeclareSorafsCapacity(_)
            | AnyPermission::CanSubmitSorafsTelemetry(_)
            | AnyPermission::CanFileSorafsCapacityDispute(_)
            | AnyPermission::CanIssueSorafsReplicationOrder(_)
            | AnyPermission::CanCompleteSorafsReplicationOrder(_)
            | AnyPermission::CanSetSorafsPricing(_)
            | AnyPermission::CanSetSorafsReservePolicy(_)
            | AnyPermission::CanManageSorafsModeration(_)
            | AnyPermission::CanManageSorafsPopRegistry(_)
            | AnyPermission::CanOperateSorafsPopIssuer(_)
            | AnyPermission::CanUpsertSorafsProviderCredit(_)
            | AnyPermission::CanManageSoranetVpnQuoteIssuers(_)
            | AnyPermission::CanIssueSoranetVpnQuote(_)
            | AnyPermission::CanIngestSoranetPrivacy(_)
            | AnyPermission::CanRegisterOracleFeed(_)
            | AnyPermission::CanProposeOracleChange(_)
            | AnyPermission::CanVoteOracleChangeStage(_)
            | AnyPermission::CanRollbackOracleChange(_)
            | AnyPermission::CanResolveOracleDispute(_)
            | AnyPermission::CanManageTwitterBindings(_)
            | AnyPermission::CanPublishSpaceDirectoryManifest(_)
            | AnyPermission::CanPublishSpaceDirectoryManifestForUaid(_) => false,
        }
    }
}
/// Permission-checked visitors for account management instructions.
pub mod account {
    use super::*;
    use crate::permission::{account::is_account_owner, revoke_permissions};
    use iroha_executor_data_model::permission::account::{
        CanModifyAccountMetadata, CanReplaceAccountController, CanUnregisterAccount,
    };
    declare_execute_visitors! {
        /// Registers a canonical account.
        visit_register_account(Register<Account>);
    }
    /// Unregisters an account when the caller owns it or has the unregister permission.
    pub fn visit_unregister_account<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<Account>,
    ) {
        let account_id = isi.object();
        if executor.context().curr_block.is_genesis()
            || is_account_owner(account_id, &executor.context().authority, executor.host())
            || {
                let can_unregister_user_account = CanUnregisterAccount {
                    account: account_id.clone(),
                };
                can_unregister_user_account
                    .is_owned_by(&executor.context().authority, executor.host())
            }
        {
            let err = revoke_permissions(executor, |permission| {
                is_permission_account_associated(permission, account_id)
            });
            if let Err(err) = err {
                deny!(executor, err);
            }
            execute!(executor, isi);
        }
        deny!(executor, "Can't unregister another account");
    }
    /// Sets account metadata after verifying ownership or the metadata permission.
    pub fn visit_set_account_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetKeyValue<Account>,
    ) {
        if isi.key().as_ref() == iroha_data_model::asset::ASSET_TRANSFER_CONTROL_METADATA_KEY {
            deny!(
                executor,
                ValidationFail::NotPermitted(format!(
                    "account metadata key `{}` is reserved for native asset transfer controls",
                    isi.key()
                ))
            );
        }
        if crate::default::isi::is_reserved_multisig_metadata_key(isi.key()) {
            deny!(
                executor,
                ValidationFail::NotPermitted(format!(
                    "account metadata key `{}` is reserved for native multisig state",
                    isi.key()
                ))
            );
        }
        let account_id = isi.object();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_account_owner(account_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        let can_set_key_value_in_user_account_token = CanModifyAccountMetadata {
            account: account_id.clone(),
        };
        if can_set_key_value_in_user_account_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't set value to the metadata of another account"
        );
    }
    /// Removes account metadata provided the caller is authorised.
    pub fn visit_remove_account_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RemoveKeyValue<Account>,
    ) {
        if isi.key().as_ref() == iroha_data_model::asset::ASSET_TRANSFER_CONTROL_METADATA_KEY {
            deny!(
                executor,
                ValidationFail::NotPermitted(format!(
                    "account metadata key `{}` is reserved for native asset transfer controls",
                    isi.key()
                ))
            );
        }
        if crate::default::isi::is_reserved_multisig_metadata_key(isi.key()) {
            deny!(
                executor,
                ValidationFail::NotPermitted(format!(
                    "account metadata key `{}` is reserved for native multisig state",
                    isi.key()
                ))
            );
        }
        let account_id = isi.object();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_account_owner(account_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        let can_remove_key_value_in_user_account_token = CanModifyAccountMetadata {
            account: account_id.clone(),
        };
        if can_remove_key_value_in_user_account_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't remove value from the metadata of another account"
        );
    }
    /// Replaces the controller for an account when the caller owns it or has the replacement permission.
    pub fn visit_replace_account_controller<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ReplaceAccountController,
    ) {
        let account_id = isi.account();
        if executor.context().curr_block.is_genesis()
            || is_account_owner(account_id, &executor.context().authority, executor.host())
            || (CanReplaceAccountController {
                account: account_id.clone(),
            })
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't replace another account controller");
    }
    /// Sets an account recovery policy when the caller owns the account or has replacement rights.
    pub fn visit_set_account_recovery_policy<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetAccountRecoveryPolicy,
    ) {
        let account_id = isi.account();
        if executor.context().curr_block.is_genesis()
            || is_account_owner(account_id, &executor.context().authority, executor.host())
            || (CanReplaceAccountController {
                account: account_id.clone(),
            })
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't set another account recovery policy");
    }
    /// Clears an account recovery policy when the caller owns the account or has replacement rights.
    pub fn visit_clear_account_recovery_policy<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ClearAccountRecoveryPolicy,
    ) {
        let account_id = isi.account();
        if executor.context().curr_block.is_genesis()
            || is_account_owner(account_id, &executor.context().authority, executor.host())
            || (CanReplaceAccountController {
                account: account_id.clone(),
            })
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't clear another account recovery policy");
    }
    declare_execute_visitors! {
        /// Delegates proposal authorisation to the core recovery state machine.
        visit_propose_account_recovery(ProposeAccountRecovery);
        /// Delegates approval authorisation to the core recovery state machine.
        visit_approve_account_recovery(ApproveAccountRecovery);
        /// Delegates cancellation authorisation to the core recovery state machine.
        visit_cancel_account_recovery(CancelAccountRecovery);
        /// Delegates finalization authorisation to the core recovery state machine.
        visit_finalize_account_recovery(FinalizeAccountRecovery);
    }
    #[expect(
        clippy::too_many_lines,
        reason = "the exhaustive permission match keeps account-association policy auditable"
    )]
    pub(crate) fn is_permission_account_associated(
        permission: &Permission,
        account_id: &AccountId,
    ) -> bool {
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        match permission {
            AnyPermission::CanUnregisterAccount(permission) => permission.account == *account_id,
            AnyPermission::CanModifyAccountMetadata(permission) => {
                permission.account == *account_id
            }
            AnyPermission::CanReplaceAccountController(permission) => {
                permission.account == *account_id
            }
            AnyPermission::CanReadAccountData(permission) => permission.account == *account_id,
            AnyPermission::CanMintAssetToAccount(permission) => &permission.account == account_id,
            AnyPermission::CanBurnAsset(permission) => permission.asset.account() == account_id,
            AnyPermission::CanTransferAsset(permission) => permission.asset.account() == account_id,
            AnyPermission::CanModifyAssetMetadata(permission) => {
                permission.asset.account() == account_id
            }
            AnyPermission::CanManageFeeSponsorProgram(permission) => {
                permission.sponsor == *account_id
            }
            AnyPermission::CanEnrollFeeSponsorProgram(permission) => {
                permission.program_id.sponsor == *account_id
            }
            AnyPermission::CanInvokeContractEntrypoint(permission) => {
                permission.contract.subject_id() == *account_id
            }
            AnyPermission::CanSetAssetTransferAvailability(permission) => {
                permission.account == *account_id
            }
            AnyPermission::CanSetAssetHoldingLimit(permission) => permission.account == *account_id,
            AnyPermission::CanExecuteSettlement(permission) => {
                permission.debited_asset.account() == account_id
            }
            AnyPermission::CanRegisterTrigger(permission) => permission.authority == *account_id,
            AnyPermission::DpnAdmin(_)
            | AnyPermission::DpnUser(_)
            | AnyPermission::DpnInori(_)
            | AnyPermission::DpnSettlement(_)
            | AnyPermission::DpnEprGuard(_)
            | AnyPermission::CanUnregisterTrigger(_)
            | AnyPermission::CanExecuteTrigger(_)
            | AnyPermission::CanModifyTrigger(_)
            | AnyPermission::CanModifyTriggerMetadata(_)
            | AnyPermission::CanResolveAccountAlias(_)
            | AnyPermission::CanDelegateAccountAliasResolution(_)
            | AnyPermission::CanManageAccountAlias(_)
            | AnyPermission::CanManageAssetDefinitionAlias(_)
            | AnyPermission::CanReadAllLedgerData(_)
            | AnyPermission::CanReadRestrictedDataspace(_)
            | AnyPermission::CanRegisterGlobalDataTrigger(_)
            | AnyPermission::CanManagePeers(_)
            | AnyPermission::CanManageLaneRelayEmergency(_)
            | AnyPermission::CanManageRuntimeUpgrades(_)
            | AnyPermission::CanManageConsensusKeys(_)
            | AnyPermission::CanManageConfidentialParams(_)
            | AnyPermission::CanRegisterDomain(_)
            | AnyPermission::CanUnregisterDomain(_)
            | AnyPermission::CanModifyDomainMetadata(_)
            | AnyPermission::CanRegisterAccount(_)
            | AnyPermission::CanUnregisterAssetDefinition(_)
            | AnyPermission::CanModifyAssetDefinitionMetadata(_)
            | AnyPermission::CanManageAssetDefinitionConfidentialPolicy(_)
            | AnyPermission::CanMintAssetWithDefinition(_)
            | AnyPermission::CanBurnAssetWithDefinition(_)
            | AnyPermission::CanTransferAssetWithDefinition(_)
            | AnyPermission::CanModifyAssetMetadataWithDefinition(_)
            | AnyPermission::CanSetAssetTransferDailyLimit(_)
            | AnyPermission::CanRegisterNft(_)
            | AnyPermission::CanUnregisterNft(_)
            | AnyPermission::CanTransferNft(_)
            | AnyPermission::CanModifyNftMetadata(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanSetHijiriParameters(_)
            | AnyPermission::CanManageSccpGovernance(_)
            | AnyPermission::CanProposeSccpRouteGovernance(_)
            | AnyPermission::CanManageOfflineEscrow(_)
            | AnyPermission::CanActivateKagemushaRecursiveReleaseV4(_)
            | AnyPermission::CanManageOfflineDeviceAttestationPolicy(_)
            | AnyPermission::CanManageRoles(_)
            | AnyPermission::CanUpgradeExecutor(_)
            | AnyPermission::CanRegisterSmartContractCode(_)
            | AnyPermission::CanManageFxCorridors(_)
            | AnyPermission::CanSetFxCorridorPolicy(_)
            | AnyPermission::CanBindSorafsAlias(_)
            | AnyPermission::CanDeclareSorafsCapacity(_)
            | AnyPermission::CanSubmitSorafsTelemetry(_)
            | AnyPermission::CanFileSorafsCapacityDispute(_)
            | AnyPermission::CanIssueSorafsReplicationOrder(_)
            | AnyPermission::CanCompleteSorafsReplicationOrder(_)
            | AnyPermission::CanSetSorafsPricing(_)
            | AnyPermission::CanSetSorafsReservePolicy(_)
            | AnyPermission::CanManageSorafsModeration(_)
            | AnyPermission::CanManageSorafsPopRegistry(_)
            | AnyPermission::CanOperateSorafsPopIssuer(_)
            | AnyPermission::CanUpsertSorafsProviderCredit(_)
            | AnyPermission::CanManageSoranetVpnQuoteIssuers(_)
            | AnyPermission::CanIssueSoranetVpnQuote(_)
            | AnyPermission::CanIngestSoranetPrivacy(_)
            | AnyPermission::CanRegisterOracleFeed(_)
            | AnyPermission::CanProposeOracleChange(_)
            | AnyPermission::CanVoteOracleChangeStage(_)
            | AnyPermission::CanRollbackOracleChange(_)
            | AnyPermission::CanResolveOracleDispute(_)
            | AnyPermission::CanManageTwitterBindings(_)
            | AnyPermission::CanPublishSpaceDirectoryManifest(_)
            | AnyPermission::CanPublishSpaceDirectoryManifestForUaid(_)
            | AnyPermission::CanPublishSpaceDirectoryManifestForAccountDomain(_) => false,
        }
    }
}
/// Permission-checked visitors for asset definition instructions.
pub mod asset_definition {
    use super::*;
    use crate::permission::{
        account::is_account_owner, asset_definition::is_asset_definition_owner,
        domain::is_domain_owner, revoke_permissions,
    };
    use iroha_executor_data_model::permission::asset_definition::{
        CanModifyAssetDefinitionMetadata, CanUnregisterAssetDefinition,
    };
    use iroha_smart_contract::data_model::asset::AssetDefinitionId;
    /// Registers an asset definition.
    pub fn visit_register_asset_definition<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Register<AssetDefinition>,
    ) {
        let Some(domain_id) = isi.object().owning_domain.as_ref() else {
            execute!(executor, isi);
        };
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_domain_owner(domain_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        deny!(
            executor,
            "Only the owning-domain owner may register a domain-owned asset definition"
        );
    }
    /// Unregisters an asset definition after confirming ownership or revoke permission.
    pub fn visit_unregister_asset_definition<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<AssetDefinition>,
    ) {
        let asset_definition_id = isi.object();
        if executor.context().curr_block.is_genesis()
            || match is_asset_definition_owner(
                asset_definition_id,
                &executor.context().authority,
                executor.host(),
            ) {
                Err(err) => deny!(executor, err),
                Ok(is_asset_definition_owner) => is_asset_definition_owner,
            }
            || {
                let can_unregister_asset_definition_token = CanUnregisterAssetDefinition {
                    asset_definition: asset_definition_id.clone(),
                };
                can_unregister_asset_definition_token
                    .is_owned_by(&executor.context().authority, executor.host())
            }
        {
            let err = revoke_permissions(executor, |permission| {
                is_permission_asset_definition_associated(permission, asset_definition_id)
            });
            if let Err(err) = err {
                deny!(executor, err);
            }
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't unregister asset definition owned by another account"
        );
    }
    /// Transfers an asset definition when the caller owns the source account or definition.
    pub fn visit_transfer_asset_definition<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Transfer<Account, AssetDefinitionId, Account>,
    ) {
        let source_id = isi.source();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_account_owner(source_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't transfer asset definition of another account"
        );
    }
    /// Sets metadata on an asset definition provided the caller is authorised.
    pub fn visit_set_asset_definition_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetKeyValue<AssetDefinition>,
    ) {
        let asset_definition_id = isi.object();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            asset_definition_id,
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_set_key_value_in_asset_definition_token = CanModifyAssetDefinitionMetadata {
            asset_definition: asset_definition_id.clone(),
        };
        if can_set_key_value_in_asset_definition_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't set value to the asset definition metadata created by another account"
        );
    }
    /// Removes metadata from an asset definition when the caller holds modify rights.
    pub fn visit_remove_asset_definition_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RemoveKeyValue<AssetDefinition>,
    ) {
        let asset_definition_id = isi.object();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            asset_definition_id,
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_remove_key_value_in_asset_definition_token = CanModifyAssetDefinitionMetadata {
            asset_definition: asset_definition_id.clone(),
        };
        if can_remove_key_value_in_asset_definition_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't remove value from the asset definition metadata created by another account"
        );
    }
    /// Enforces the asset-owner half of an asset-definition alias update.
    ///
    /// Core independently requires the matching
    /// [`CanManageAssetDefinitionAlias`](iroha_executor_data_model::permission::asset_definition::CanManageAssetDefinitionAlias)
    /// namespace capability before applying the mutation.
    pub fn visit_set_asset_definition_alias<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetAssetDefinitionAlias,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            &isi.asset_definition_id,
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        deny!(
            executor,
            "Only the asset-definition owner may change its alias"
        );
    }
    #[expect(
        clippy::too_many_lines,
        reason = "keeping the exhaustive permission association matrix in one match makes this security boundary auditable"
    )]
    pub(crate) fn is_permission_asset_definition_associated(
        permission: &Permission,
        asset_definition_id: &AssetDefinitionId,
    ) -> bool {
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        match permission {
            AnyPermission::CanUnregisterAssetDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanModifyAssetDefinitionMetadata(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanManageAssetDefinitionConfidentialPolicy(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanMintAssetWithDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanBurnAssetWithDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanTransferAssetWithDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanModifyAssetMetadataWithDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanMintAssetToAccount(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanBurnAsset(permission) => {
                permission.asset.definition() == asset_definition_id
            }
            AnyPermission::CanTransferAsset(permission) => {
                permission.asset.definition() == asset_definition_id
            }
            AnyPermission::CanModifyAssetMetadata(permission) => {
                permission.asset.definition() == asset_definition_id
            }
            AnyPermission::CanSetAssetTransferAvailability(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanSetAssetTransferDailyLimit(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanSetAssetHoldingLimit(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanExecuteSettlement(permission) => {
                permission.debited_asset.definition() == asset_definition_id
            }
            AnyPermission::CanManageAssetDefinitionAlias(permission) => match permission.scope {
                iroha_executor_data_model::permission::asset_definition::AssetDefinitionAliasPermissionScope::Alias(alias) => {
                    &alias.asset_definition_id == asset_definition_id
                }
                iroha_executor_data_model::permission::asset_definition::AssetDefinitionAliasPermissionScope::Domain(_)
                | iroha_executor_data_model::permission::asset_definition::AssetDefinitionAliasPermissionScope::Dataspace(_) => false,
            },
            AnyPermission::DpnAdmin(_)
            | AnyPermission::DpnUser(_)
            | AnyPermission::DpnInori(_)
            | AnyPermission::DpnSettlement(_)
            | AnyPermission::DpnEprGuard(_)
            | AnyPermission::CanUnregisterAccount(_)
            | AnyPermission::CanModifyAccountMetadata(_)
            | AnyPermission::CanReplaceAccountController(_)
            | AnyPermission::CanResolveAccountAlias(_)
            | AnyPermission::CanDelegateAccountAliasResolution(_)
            | AnyPermission::CanManageAccountAlias(_)
            | AnyPermission::CanReadAllLedgerData(_)
            | AnyPermission::CanReadAccountData(_)
            | AnyPermission::CanReadRestrictedDataspace(_)
            | AnyPermission::CanRegisterGlobalDataTrigger(_)
            | AnyPermission::CanRegisterTrigger(_)
            | AnyPermission::CanUnregisterTrigger(_)
            | AnyPermission::CanExecuteTrigger(_)
            | AnyPermission::CanModifyTrigger(_)
            | AnyPermission::CanModifyTriggerMetadata(_)
            | AnyPermission::CanManagePeers(_)
            | AnyPermission::CanManageLaneRelayEmergency(_)
            | AnyPermission::CanManageRuntimeUpgrades(_)
            | AnyPermission::CanManageConsensusKeys(_)
            | AnyPermission::CanManageConfidentialParams(_)
            | AnyPermission::CanRegisterDomain(_)
            | AnyPermission::CanUnregisterDomain(_)
            | AnyPermission::CanModifyDomainMetadata(_)
            | AnyPermission::CanRegisterAccount(_)
            | AnyPermission::CanRegisterNft(_)
            | AnyPermission::CanUnregisterNft(_)
            | AnyPermission::CanTransferNft(_)
            | AnyPermission::CanModifyNftMetadata(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanSetHijiriParameters(_)
            | AnyPermission::CanManageSccpGovernance(_)
            | AnyPermission::CanProposeSccpRouteGovernance(_)
            | AnyPermission::CanManageOfflineEscrow(_)
            | AnyPermission::CanActivateKagemushaRecursiveReleaseV4(_)
            | AnyPermission::CanManageOfflineDeviceAttestationPolicy(_)
            | AnyPermission::CanManageRoles(_)
            | AnyPermission::CanUpgradeExecutor(_)
            | AnyPermission::CanRegisterSmartContractCode(_)
            | AnyPermission::CanInvokeContractEntrypoint(_)
            | AnyPermission::CanManageFxCorridors(_)
            | AnyPermission::CanSetFxCorridorPolicy(_)
            | AnyPermission::CanBindSorafsAlias(_)
            | AnyPermission::CanDeclareSorafsCapacity(_)
            | AnyPermission::CanSubmitSorafsTelemetry(_)
            | AnyPermission::CanFileSorafsCapacityDispute(_)
            | AnyPermission::CanIssueSorafsReplicationOrder(_)
            | AnyPermission::CanCompleteSorafsReplicationOrder(_)
            | AnyPermission::CanSetSorafsPricing(_)
            | AnyPermission::CanSetSorafsReservePolicy(_)
            | AnyPermission::CanManageSorafsModeration(_)
            | AnyPermission::CanManageSorafsPopRegistry(_)
            | AnyPermission::CanOperateSorafsPopIssuer(_)
            | AnyPermission::CanUpsertSorafsProviderCredit(_)
            | AnyPermission::CanManageSoranetVpnQuoteIssuers(_)
            | AnyPermission::CanIssueSoranetVpnQuote(_)
            | AnyPermission::CanIngestSoranetPrivacy(_)
            | AnyPermission::CanRegisterOracleFeed(_)
            | AnyPermission::CanProposeOracleChange(_)
            | AnyPermission::CanVoteOracleChangeStage(_)
            | AnyPermission::CanRollbackOracleChange(_)
            | AnyPermission::CanResolveOracleDispute(_)
            | AnyPermission::CanManageTwitterBindings(_)
            | AnyPermission::CanPublishSpaceDirectoryManifest(_)
            | AnyPermission::CanPublishSpaceDirectoryManifestForUaid(_)
            | AnyPermission::CanPublishSpaceDirectoryManifestForAccountDomain(_)
            | AnyPermission::CanManageFeeSponsorProgram(_)
            | AnyPermission::CanEnrollFeeSponsorProgram(_) => false,
        }
    }
}
/// Permission-checked visitors for asset operations.
pub mod asset {
    use super::*;
    use crate::permission::{asset::is_asset_owner, asset_definition::is_asset_definition_owner};
    use iroha_executor_data_model::permission::asset::{
        CanBurnAsset, CanBurnAssetWithDefinition, CanMintAssetToAccount,
        CanMintAssetWithDefinition, CanModifyAssetMetadata, CanModifyAssetMetadataWithDefinition,
        CanSetAssetHoldingLimit, CanSetAssetTransferAvailability, CanSetAssetTransferDailyLimit,
        CanTransferAsset, CanTransferAssetWithDefinition,
    };
    use iroha_smart_contract::data_model::isi::{
        BuiltInInstruction, RemoveAssetKeyValue, SetAssetKeyValue,
    };
    use norito::NoritoSerialize;
    fn target_account_scope(
        executor: &(impl Execute + Visit + ?Sized),
        account_id: &AccountId,
    ) -> Result<(AccountAliasDomain, DataSpaceId), String> {
        let accounts = executor
            .host()
            .query(FindAccounts)
            .execute()
            .map_err(|err| format!("failed to query transfer-control target account: {err}"))?;
        for account in accounts {
            let account = account
                .map_err(|err| format!("failed to read transfer-control target account: {err}"))?;
            if account.id() != account_id {
                continue;
            }
            return account.label().map_or_else(
                || {
                    Err(format!(
                        "transfer-control target account `{account_id}` has no canonical on-chain alias label"
                    ))
                },
                |label| {
                    let account_domain = label.domain.clone().ok_or_else(|| {
                        format!(
                            "transfer-control target account `{account_id}` has no canonical on-chain domain label"
                        )
                    })?;
                    Ok((account_domain, label.dataspace))
                },
            );
        }
        Err(format!(
            "transfer-control target account `{account_id}` does not exist"
        ))
    }
    /// Sets account transfer availability when genesis or the asset-definition owner invokes it.
    pub fn visit_set_asset_transfer_availability<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetAssetTransferAvailability,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            isi.asset_definition_id(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let permission = CanSetAssetTransferAvailability {
            account: isi.account_id().clone(),
            asset_definition: isi.asset_definition_id().clone(),
        };
        if permission.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "transfer availability requires an exact account-and-asset permission"
        );
    }
    /// Sets account transfer limits when genesis or the asset-definition owner invokes it.
    pub fn visit_set_asset_transfer_control<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetAssetTransferControl,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            isi.asset_definition_id(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        if isi.limits().len() != 1 || isi.limits()[0].window != AssetTransferControlWindow::Day {
            deny!(
                executor,
                "delegated daily-limit permission accepts exactly one DAY limit"
            );
        }
        let (account_domain, account_dataspace) =
            match target_account_scope(executor, isi.account_id()) {
                Ok(scope) => scope,
                Err(err) => deny!(executor, ValidationFail::NotPermitted(err)),
            };
        let permission = CanSetAssetTransferDailyLimit {
            asset_definition: isi.asset_definition_id().clone(),
            account_domain,
            account_dataspace,
        };
        if permission.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "daily limit requires an asset- and account-domain-scoped permission"
        );
    }
    /// Sets a native holding limit when invoked by genesis, the asset-definition
    /// owner, or an explicitly provisioned exact account-and-asset authority.
    pub fn visit_set_asset_holding_limit<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetAssetHoldingLimit,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            isi.asset_definition_id(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let permission = CanSetAssetHoldingLimit {
            account: isi.account_id().clone(),
            asset_definition: isi.asset_definition_id().clone(),
        };
        if permission.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "holding limit requires an exact account-and-asset permission"
        );
    }
    fn execute_mint_asset<V>(executor: &mut V, isi: &Mint<Quantity, Asset>)
    where
        V: Execute + Visit + ?Sized,
        Mint<Quantity, Asset>: BuiltInInstruction + NoritoSerialize,
    {
        let asset_id = isi.destination();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            asset_id.definition(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_mint_assets_with_definition_token = CanMintAssetWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if can_mint_assets_with_definition_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        let can_mint_to_account_token = CanMintAssetToAccount {
            asset_definition: asset_id.definition().clone(),
            account: asset_id.account().clone(),
        };
        if can_mint_to_account_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't mint assets with definitions registered by other accounts"
        );
    }
    /// Mints an asset quantity when the caller owns the definition or has explicit permission.
    pub fn visit_mint_asset_quantity<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Mint<Quantity, Asset>,
    ) {
        execute_mint_asset(executor, isi);
    }
    fn execute_burn_asset<V>(executor: &mut V, isi: &Burn<Quantity, Asset>)
    where
        V: Execute + Visit + ?Sized,
        Burn<Quantity, Asset>: BuiltInInstruction + NoritoSerialize,
    {
        let asset_id = isi.destination();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_asset_owner(asset_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            asset_id.definition(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_burn_assets_with_definition_token = CanBurnAssetWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if can_burn_assets_with_definition_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        let can_burn_user_asset_token = CanBurnAsset {
            asset: asset_id.clone(),
        };
        if can_burn_user_asset_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't burn assets from another account");
    }
    /// Burns an asset quantity if the caller controls the asset or holds burn permission.
    pub fn visit_burn_asset_quantity<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Burn<Quantity, Asset>,
    ) {
        execute_burn_asset(executor, isi);
    }
    /// Placeholder visitor for asset metadata insertion.
    pub fn visit_set_asset_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetAssetKeyValue,
    ) {
        let asset_id = isi.asset();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_asset_owner(asset_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            asset_id.definition(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let definition_token = CanModifyAssetMetadataWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if definition_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        let asset_token = CanModifyAssetMetadata {
            asset: asset_id.clone(),
        };
        if asset_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't set metadata for an asset without ownership or explicit permission"
        );
    }
    /// Modify asset metadata by removing a key if ownership or grants allow it.
    pub fn visit_remove_asset_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RemoveAssetKeyValue,
    ) {
        let asset_id = isi.asset();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_asset_owner(asset_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            asset_id.definition(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let definition_token = CanModifyAssetMetadataWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if definition_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        let asset_token = CanModifyAssetMetadata {
            asset: asset_id.clone(),
        };
        if asset_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't remove metadata for an asset without ownership or explicit permission"
        );
    }
    /// Transfers an asset quantity after verifying ownership or transfer permission.
    pub fn visit_transfer_asset_quantity<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Transfer<Asset, Quantity, Account>,
    ) {
        let asset_id = isi.source();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_asset_owner(asset_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        let can_transfer_assets_with_definition_token = CanTransferAssetWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if can_transfer_assets_with_definition_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        let can_transfer_user_asset_token = CanTransferAsset {
            asset: asset_id.clone(),
        };
        if can_transfer_user_asset_token.is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(executor, "Can't transfer assets of another account");
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{
            Execute, Iroha,
            data_model::{
                ValidationFail,
                account::AccountId,
                asset::{AssetDefinitionId, AssetId},
                block::BlockHeader,
                bridge::BridgeReceipt,
                domain::DomainId,
                executor::Result as ExecResult,
                isi::{
                    InstructionBox, RegisterPublicLaneValidator,
                    bridge::RecordBridgeReceipt,
                    governance::RegisterCitizen,
                    repo::{RepoInstructionBox, RepoIsi},
                },
                metadata::Metadata,
                name::Name,
                nexus::LaneId,
                peer::PeerId,
                prelude::{Json, Quantity},
                repo::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance},
            },
            prelude::{Context, Visit},
        };
        use core::num::NonZeroU64;
        use iroha_crypto::{Algorithm, KeyPair};
        fn fixture_key_pair(seed: u8) -> KeyPair {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("fixture seed must derive a valid keypair")
        }
        fn fixture_key_pair_from_height(height: u64) -> KeyPair {
            let mut seed = vec![0; 32];
            seed[..core::mem::size_of::<u64>()].copy_from_slice(&height.to_le_bytes());
            KeyPair::try_from_seed(seed, Algorithm::Ed25519)
                .expect("fixture block height must derive a valid keypair")
        }
        struct StubExecutor {
            host: Iroha,
            context: Context,
            verdict: ExecResult,
        }
        impl StubExecutor {
            fn new(height: u64) -> (Self, AssetId) {
                let domain: DomainId =
                    DomainId::try_new("test_domain", "universal").expect("valid domain");
                let keypair = fixture_key_pair_from_height(height);
                let account = AccountId::new(keypair.public_key().clone());
                let asset_definition = AssetDefinitionId::derive_from_components(
                    domain.clone(),
                    "sample_asset".parse::<Name>().expect("valid name"),
                );
                let asset = AssetId::new(asset_definition, account.clone());
                let header = BlockHeader::new(
                    NonZeroU64::new(height).expect("height > 0"),
                    None,
                    None,
                    None,
                    0,
                    0,
                );
                let context = Context {
                    authority: account,
                    curr_block: header,
                };
                (
                    Self {
                        host: Iroha,
                        context,
                        verdict: Ok(()),
                    },
                    asset,
                )
            }
        }
        impl Execute for StubExecutor {
            fn host(&self) -> &Iroha {
                &self.host
            }
            fn context(&self) -> &Context {
                &self.context
            }
            fn context_mut(&mut self) -> &mut Context {
                &mut self.context
            }
            fn verdict(&self) -> &ExecResult {
                &self.verdict
            }
            fn deny(&mut self, reason: ValidationFail) {
                self.verdict = Err(reason);
            }
        }
        impl Visit for StubExecutor {}
        #[test]
        fn fixture_key_pair_uses_checked_seed_derivation() {
            assert_eq!(fixture_key_pair(1).algorithm(), Algorithm::Ed25519);
            assert_eq!(
                fixture_key_pair_from_height(1).algorithm(),
                Algorithm::Ed25519
            );
            assert!(
                KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
                "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
            );
        }
        #[test]
        fn set_asset_key_value_genesis_returns_ok() {
            let (mut executor, asset) = StubExecutor::new(1);
            let instruction = SetAssetKeyValue::new(
                asset,
                "tag".parse().expect("valid name"),
                Json::new("value"),
            );
            visit_set_asset_key_value(&mut executor, &instruction);
            assert!(executor.verdict().is_ok());
        }
        #[test]
        fn set_asset_key_value_non_genesis_owner_allows() {
            let (mut executor, asset) = StubExecutor::new(2);
            let instruction = SetAssetKeyValue::new(
                asset.clone(),
                "tag".parse().expect("valid name"),
                Json::new("value"),
            );
            visit_set_asset_key_value(&mut executor, &instruction);
            assert!(
                executor.verdict().is_ok(),
                "asset owner should be able to set metadata outside genesis"
            );
        }
        #[test]
        fn set_asset_key_value_non_owner_denied() {
            let (mut executor, asset) = StubExecutor::new(2);
            let intruder_key = fixture_key_pair(7);
            let intruder = AccountId::new(intruder_key.public_key().clone());
            executor.context_mut().authority = intruder;
            let instruction = SetAssetKeyValue::new(
                asset.clone(),
                "tag".parse().expect("valid name"),
                Json::new("value"),
            );
            visit_set_asset_key_value(&mut executor, &instruction);
            assert!(
                matches!(
                    executor.verdict(),
                    Err(ValidationFail::InstructionFailed(_) | ValidationFail::NotPermitted(_))
                ),
                "expected denial for non-owner, got {:?}",
                executor.verdict()
            );
        }
        #[test]
        fn visit_instruction_dispatches_register_peer_with_pop() {
            let (mut executor, _) = StubExecutor::new(1);
            let peer_keypair = fixture_key_pair(42);
            let peer_id = PeerId::from(peer_keypair.public_key().clone());
            let instruction = RegisterPeerWithPop::new(peer_id, vec![1, 2, 3]);
            let instruction_box: InstructionBox = instruction.into();
            visit_instruction(&mut executor, &instruction_box);
            assert!(
                executor.verdict().is_ok(),
                "register peer with pop should succeed during genesis"
            );
        }
        #[test]
        fn visit_instruction_dispatches_register_public_lane_validator() {
            let (mut executor, asset) = StubExecutor::new(1);
            let validator = asset.account().clone();
            let instruction = RegisterPublicLaneValidator::new(
                LaneId::SINGLE,
                validator.clone(),
                PeerId::from(validator.expect_single_signatory().clone()),
                validator,
                Quantity::from(1_u64),
                Metadata::default(),
            );
            let instruction_box: InstructionBox = instruction.into();
            visit_instruction(&mut executor, &instruction_box);
            assert!(
                executor.verdict().is_ok(),
                "register public-lane validator should succeed during genesis"
            );
        }
        #[test]
        fn visit_instruction_dispatches_register_citizen() {
            let (mut executor, _) = StubExecutor::new(2);
            let instruction = RegisterCitizen {
                owner: executor.context().authority.clone(),
                amount: Quantity::zero(),
            };
            let instruction_box: InstructionBox = instruction.into();
            visit_instruction(&mut executor, &instruction_box);
            assert!(
                executor.verdict().is_ok(),
                "citizen self-registration must reach Core for owner and bond validation"
            );
        }
        #[test]
        fn visit_instruction_dispatches_record_bridge_receipt() {
            let (mut executor, _) = StubExecutor::new(1);
            let receipt = BridgeReceipt {
                lane: LaneId::new(2),
                direction: b"mint".to_vec(),
                source_tx: [0x11; 32],
                dest_tx: None,
                proof_hash: [0x22; 32],
                amount: 1_u64.into(),
                asset_id: b"wBTC#btc".to_vec(),
                recipient: b"alice@main".to_vec(),
            };
            let instruction = RecordBridgeReceipt::new(receipt);
            let instruction_box: InstructionBox = instruction.into();
            visit_instruction(&mut executor, &instruction_box);
            assert!(
                executor.verdict().is_ok(),
                "record bridge receipt should succeed during genesis"
            );
        }
        #[test]
        fn visit_instruction_dispatches_repo_instruction_box() {
            let (mut executor, _) = StubExecutor::new(1);
            let domain: DomainId = DomainId::try_new("fixture", "universal").expect("valid domain");
            let counterparty_keypair = fixture_key_pair(9);
            let counterparty = AccountId::new(counterparty_keypair.public_key().clone());
            let cash_def = AssetDefinitionId::derive_from_components(
                domain.clone(),
                "cash".parse::<Name>().expect("valid name"),
            );
            let collateral_def = AssetDefinitionId::derive_from_components(
                domain,
                "collateral".parse::<Name>().expect("valid name"),
            );
            let agreement_id: RepoAgreementId = "repo_dispatch".parse().expect("repo id");
            let repo_instruction = RepoIsi::new(
                agreement_id,
                executor.context().authority.clone(),
                counterparty,
                None,
                RepoCashLeg {
                    asset_definition_id: cash_def,
                    quantity: Quantity::from(1u32),
                },
                RepoCollateralLeg::new(collateral_def, 1u32),
                0,
                1,
                RepoGovernance::with_defaults(0, 0),
            );
            let instruction_box: InstructionBox = RepoInstructionBox::from(repo_instruction).into();
            visit_instruction(&mut executor, &instruction_box);
            assert!(
                executor.verdict().is_ok(),
                "repo instruction box should dispatch through executor"
            );
        }
        #[test]
        fn remove_asset_key_value_non_genesis_owner_allows() {
            let (mut executor, asset) = StubExecutor::new(2);
            let instruction =
                RemoveAssetKeyValue::new(asset.clone(), "tag".parse().expect("valid name"));
            visit_remove_asset_key_value(&mut executor, &instruction);
            assert!(
                executor.verdict().is_ok(),
                "asset owner should be able to remove metadata outside genesis"
            );
        }
        #[test]
        fn remove_asset_key_value_non_owner_denied() {
            let (mut executor, asset) = StubExecutor::new(2);
            let intruder_key = fixture_key_pair(9);
            let intruder = AccountId::new(intruder_key.public_key().clone());
            executor.context_mut().authority = intruder;
            let instruction =
                RemoveAssetKeyValue::new(asset.clone(), "tag".parse().expect("valid name"));
            visit_remove_asset_key_value(&mut executor, &instruction);
            assert!(
                matches!(
                    executor.verdict(),
                    Err(ValidationFail::InstructionFailed(_) | ValidationFail::NotPermitted(_))
                ),
                "expected denial for non-owner, got {:?}",
                executor.verdict()
            );
        }
        #[test]
        fn transfer_asset_quantity_source_owner_allows() {
            let (mut executor, asset) = StubExecutor::new(2);
            let destination = AccountId::new(fixture_key_pair(11).public_key().clone());
            let instruction = Transfer::asset_quantity(asset, Quantity::one(), destination);
            visit_transfer_asset_quantity(&mut executor, &instruction);
            assert!(
                executor.verdict().is_ok(),
                "the source account owner must be allowed to transfer"
            );
        }
        #[test]
        fn transfer_asset_quantity_non_owner_without_exact_permission_is_denied() {
            let (mut executor, asset) = StubExecutor::new(2);
            executor.context_mut().authority =
                AccountId::new(fixture_key_pair(12).public_key().clone());
            let destination = AccountId::new(fixture_key_pair(13).public_key().clone());
            let instruction = Transfer::asset_quantity(asset, Quantity::one(), destination);
            visit_transfer_asset_quantity(&mut executor, &instruction);
            assert!(
                matches!(
                    executor.verdict(),
                    Err(ValidationFail::InstructionFailed(_) | ValidationFail::NotPermitted(_))
                ),
                "an unrelated authority must need an exact asset or definition permission"
            );
        }
    }
}
/// Permission-checked visitors for non-fungible asset instructions.
pub mod nft {
    use super::*;
    use crate::{
        data_model::isi::BuiltInInstruction,
        permission::{
            account::is_account_owner,
            nft::{is_nft_full_owner, is_nft_weak_owner},
            revoke_permissions,
        },
    };
    use iroha_executor_data_model::permission::nft::{
        CanModifyNftMetadata, CanRegisterNft, CanTransferNft, CanUnregisterNft,
    };
    use norito::NoritoSerialize;
    /// Registers an NFT when the caller owns the domain or has the registration permission.
    pub fn visit_register_nft<V: Execute + Visit + ?Sized>(executor: &mut V, isi: &Register<Nft>) {
        let domain_id = isi.object().id().domain();
        match crate::permission::domain::is_domain_owner(
            domain_id,
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_register_nft_in_domain_token = CanRegisterNft {
            domain: domain_id.clone(),
        };
        if can_register_nft_in_domain_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't register NFT in a domain owned by another account"
        );
    }
    /// Unregisters an NFT once the caller proves ownership or holds the revoke token.
    pub fn visit_unregister_nft<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<Nft>,
    ) {
        let nft_id = isi.object();
        if executor.context().curr_block.is_genesis()
            || match is_nft_full_owner(nft_id, &executor.context().authority, executor.host()) {
                Err(err) => deny!(executor, err),
                Ok(is_owner) => is_owner,
            }
            || {
                let can_unregister_token = CanUnregisterNft {
                    nft: nft_id.clone(),
                };
                can_unregister_token.is_owned_by(&executor.context().authority, executor.host())
            }
        {
            let err = revoke_permissions(executor, |permission| {
                is_permission_nft_associated(permission, nft_id)
            });
            if let Err(err) = err {
                deny!(executor, err);
            }
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't unregister NFT in a domain owned by another account"
        );
    }
    fn is_permission_nft_associated(permission: &Permission, nft_id: &NftId) -> bool {
        use AnyPermission::*;
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        match permission {
            CanUnregisterNft(permission) => &permission.nft == nft_id,
            CanTransferNft(permission) => &permission.nft == nft_id,
            CanModifyNftMetadata(permission) => &permission.nft == nft_id,
            _ => false,
        }
    }
    /// Transfers an NFT after verifying the caller's ownership or transfer permission.
    pub fn visit_transfer_nft<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Transfer<Account, NftId, Account>,
    ) {
        let source_id = isi.source();
        let nft_id = isi.object();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_account_owner(source_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        match is_nft_weak_owner(nft_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_transfer_nft_token = CanTransferNft {
            nft: nft_id.clone(),
        };
        if can_transfer_nft_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't transfer NFT of another account");
    }
    /// Sets NFT metadata when the caller is authorised to mutate it.
    pub fn visit_set_nft_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetKeyValue<Nft>,
    ) {
        execute_modify_nft_key_value(executor, isi.object(), isi);
    }
    /// Removes NFT metadata once appropriate permissions are confirmed.
    pub fn visit_remove_nft_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RemoveKeyValue<Nft>,
    ) {
        execute_modify_nft_key_value(executor, isi.object(), isi);
    }
    fn execute_modify_nft_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        nft_id: &NftId,
        isi: &(impl BuiltInInstruction + NoritoSerialize),
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match crate::permission::domain::is_domain_owner(
            nft_id.domain(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_modify_nft_token = CanModifyNftMetadata {
            nft: nft_id.clone(),
        };
        if can_modify_nft_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't modify NFT from domain owned by another account"
        );
    }
}
/// Permission-checked visitors for network parameter updates.
pub mod parameter {
    use super::*;
    use iroha_executor_data_model::permission::parameter::{
        CanSetHijiriParameters, CanSetParameters,
    };
    const SCCP_REGISTRY_PARAMETER_ID: &str = "sccp_registry_v1";
    fn updates_sccp_governance(isi: &SetParameter) -> bool {
        matches!(
            isi.inner(),
            Parameter::Custom(parameter)
                if parameter.id().name().as_ref() == SCCP_REGISTRY_PARAMETER_ID
        )
    }
    fn updates_validation_fee_governance(isi: &SetParameter) -> bool {
        matches!(
            isi.inner(),
            Parameter::Custom(parameter)
                if iroha_data_model::validation_fee::is_reserved_validation_fee_parameter_id(
                    parameter.id()
                )
        )
    }
    fn updates_hijiri_parameters(isi: &SetParameter) -> bool {
        matches!(
            isi.inner(),
            Parameter::Custom(parameter)
                if iroha_data_model::hijiri::is_hijiri_parameter_id(parameter.id())
        )
    }
    /// Applies a network parameter change when genesis or a parameter manager invokes it.
    pub fn visit_set_parameter<V: Execute + Visit + ?Sized>(executor: &mut V, isi: &SetParameter) {
        if updates_sccp_governance(isi) {
            deny!(
                executor,
                "The reserved SCCP registry cannot be changed through SetParameter; an exact due Parliament certificate must apply the typed SCCP action"
            );
        }
        if updates_validation_fee_governance(isi) {
            deny!(
                executor,
                "Validation-fee governance parameters can only be changed by an enacted SORA Parliament proposal"
            );
        }
        if updates_hijiri_parameters(isi) {
            if executor.context().curr_block.is_genesis() {
                execute!(executor, isi);
            }
            if CanSetHijiriParameters.is_owned_by(&executor.context().authority, executor.host()) {
                execute!(executor, isi);
            }
            deny!(
                executor,
                "Can't set Hijiri parameters without CanSetHijiriParameters"
            );
        }
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanSetParameters.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't set executor configuration parameters without permission"
        );
    }
}
/// Permission-checked visitors for role registration and mutation.
pub mod role {
    use super::*;
    use iroha_executor_data_model::permission::role::CanManageRoles;
    use iroha_smart_contract::{Iroha, data_model::role::Role};
    #[derive(Clone, Copy)]
    pub(super) enum RoleDelegationOperation {
        Grant,
        Revoke,
    }
    pub(super) fn validate_role_delegation_permissions(
        role: &Role,
        authority: &AccountId,
        context: &crate::prelude::Context,
        host: &Iroha,
        operation: RoleDelegationOperation,
    ) -> Result<(), ValidationFail> {
        for permission in role.permissions() {
            let any_permission = AnyPermission::try_from(permission).map_err(|_| {
                ValidationFail::NotPermitted(format!("{permission:?}: Unknown permission"))
            })?;
            if matches!(operation, RoleDelegationOperation::Grant)
                && any_permission.is_dpn_application_permission()
            {
                return Err(ValidationFail::NotPermitted(
                    "NEVO DPN permissions must be granted directly to exact accounts, never through roles"
                        .to_owned(),
                ));
            }
            match operation {
                RoleDelegationOperation::Grant => {
                    crate::permission::ValidateGrantRevoke::validate_grant(
                        &any_permission,
                        authority,
                        context,
                        host,
                    )?;
                }
                RoleDelegationOperation::Revoke => {
                    crate::permission::ValidateGrantRevoke::validate_revoke(
                        &any_permission,
                        authority,
                        context,
                        host,
                    )?;
                }
            }
        }
        Ok(())
    }
    macro_rules! impl_execute_grant_revoke_account_role {
        ($executor:ident, $isi:ident, $operation:ident) => {
            let role_id = $isi.object();
            if $executor.context().curr_block.is_genesis() {
                execute!($executor, $isi)
            }
            if !find_account_roles($executor.context().authority.clone(), $executor.host())
                .any(|authority_role_id| authority_role_id == *role_id)
            {
                deny!(
                    $executor,
                    "Can't grant or revoke a role the authority does not hold"
                );
            }
            let Some(role) = find_role(role_id, $executor.host()) else {
                deny!($executor, "Can't grant or revoke an unknown role");
            };
            if let Err(error) = validate_role_delegation_permissions(
                &role,
                &$executor.context().authority,
                $executor.context(),
                $executor.host(),
                RoleDelegationOperation::$operation,
            ) {
                deny!($executor, error);
            }
            execute!($executor, $isi)
        };
    }
    macro_rules! impl_execute_grant_revoke_role_permission {
        ($executor:ident, $isi:ident, $method:ident, $isi_type:ty) => {
            let role_id = $isi.destination().clone();
            let permission = $isi.object();
            if let Ok(any_permission) = AnyPermission::try_from(permission) {
                if !$executor.context().curr_block.is_genesis() {
                    if !find_account_roles($executor.context().authority.clone(), $executor.host())
                        .any(|authority_role_id| authority_role_id == role_id)
                    {
                        deny!($executor, "Can't modify role");
                    }
                    if let Err(error) = crate::permission::ValidateGrantRevoke::$method(
                        &any_permission,
                        &$executor.context().authority,
                        $executor.context(),
                        $executor.host(),
                    ) {
                        deny!($executor, error);
                    }
                }
                let isi = &<$isi_type>::role_permission(any_permission, role_id);
                execute!($executor, isi);
            }
            deny!(
                $executor,
                ValidationFail::NotPermitted(format!("{permission:?}: Unknown permission"))
            );
        };
    }
    fn find_account_roles(account_id: AccountId, host: &Iroha) -> impl Iterator<Item = RoleId> {
        use iroha_smart_contract::DebugExpectExt as _;
        host.query(FindRolesByAccountId::new(account_id))
            .execute()
            .dbg_expect("INTERNAL BUG: `FindRolesByAccountId` must never fail")
            .map(|role| role.dbg_expect("Failed to get role from cursor"))
    }
    fn find_role(role_id: &RoleId, host: &Iroha) -> Option<Role> {
        use iroha_smart_contract::DebugExpectExt as _;
        host.query(FindRoles)
            .execute()
            .dbg_expect("INTERNAL BUG: `FindAllRoles` must never fail")
            .map(|role| role.dbg_expect("Failed to get role from cursor"))
            .find(|role| role.id() == role_id)
    }
    pub(super) fn validated_role_registration_permissions(
        role: &Role,
        authority: &AccountId,
        context: &crate::prelude::Context,
        host: &Iroha,
    ) -> Result<Vec<AnyPermission>, ValidationFail> {
        let mut permissions = Vec::with_capacity(role.permissions().len());
        for permission in role.permissions() {
            let any_permission = AnyPermission::try_from(permission).map_err(|_| {
                ValidationFail::NotPermitted(format!("{permission:?}: Unknown permission"))
            })?;
            if any_permission.is_dpn_application_permission() {
                return Err(ValidationFail::NotPermitted(
                    "NEVO DPN permissions must be granted directly to exact accounts, never embedded in roles"
                        .to_owned(),
                ));
            }
            if !context.curr_block.is_genesis() {
                crate::permission::ValidateGrantRevoke::validate_grant(
                    &any_permission,
                    authority,
                    context,
                    host,
                )?;
            }
            permissions.push(any_permission);
        }
        Ok(permissions)
    }
    /// Registers a role and seeds its permissions when the caller controls role governance.
    pub fn visit_register_role<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Register<Role>,
    ) {
        let role = isi.object();
        let mut new_role = Role::new(role.id().clone(), role.grant_to().clone());
        if is_reserved_multisig_role_id(role.id()) {
            deny!(
                executor,
                "reserved multisig role names may not be registered"
            );
        }
        let permissions = match validated_role_registration_permissions(
            role.inner(),
            &executor.context().authority,
            executor.context(),
            executor.host(),
        ) {
            Ok(permissions) => permissions,
            Err(error) => deny!(executor, error),
        };
        for any_permission in permissions {
            new_role = new_role.add_permission(any_permission);
        }
        if executor.context().curr_block.is_genesis()
            || CanManageRoles.is_owned_by(&executor.context().authority, executor.host())
        {
            let isi = &Register::role(new_role);
            if let Err(err) = executor.host().submit(isi) {
                deny!(executor, err);
            }
            // Core's `Register<Role>` execution atomically assigns the initial owner.
            return;
        }
        deny!(executor, "Can't register role");
    }
    /// Unregisters a role if genesis or a role manager invokes the instruction.
    pub fn visit_unregister_role<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<Role>,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManageRoles.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't unregister role");
    }
    /// Grants a role to an account when the caller is authorised to manage roles.
    pub fn visit_grant_account_role<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Grant<RoleId, Account>,
    ) {
        impl_execute_grant_revoke_account_role!(executor, isi, Grant);
    }
    /// Revokes a role from an account after verifying role management permissions.
    pub fn visit_revoke_account_role<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Revoke<RoleId, Account>,
    ) {
        impl_execute_grant_revoke_account_role!(executor, isi, Revoke);
    }
    /// Grants a permission to a role after ensuring the caller may mutate role permissions.
    pub fn visit_grant_role_permission<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Grant<Permission, Role>,
    ) {
        if AnyPermission::try_from(isi.object())
            .is_ok_and(|permission| permission.is_dpn_application_permission())
        {
            deny!(
                executor,
                "NEVO DPN permissions must be granted directly to exact accounts, never to roles"
            );
        }
        impl_execute_grant_revoke_role_permission!(executor, isi, validate_grant, Grant<Permission, Role>);
    }
    /// Revokes a permission from a role once the caller passes the permission gate.
    pub fn visit_revoke_role_permission<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Revoke<Permission, Role>,
    ) {
        impl_execute_grant_revoke_role_permission!(executor, isi, validate_revoke, Revoke<Permission, Role>);
    }
}
/// Permission-checked visitors for trigger lifecycle and metadata instructions.
pub mod trigger {
    use super::*;
    use crate::permission::{revoke_permissions, trigger::is_trigger_owner};
    use iroha_executor_data_model::permission::trigger::{
        CanExecuteTrigger, CanModifyTrigger, CanModifyTriggerMetadata, CanRegisterTrigger,
        CanUnregisterTrigger,
    };
    use iroha_smart_contract::data_model::trigger::Trigger;
    /// Registers a trigger when the caller is genesis, the trigger authority, or holds the grant token.
    pub fn visit_register_trigger<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Register<Trigger>,
    ) {
        let trigger = isi.object();
        let is_genesis = executor.context().curr_block.is_genesis();
        if is_genesis || trigger.action().authority() == &executor.context().authority || {
            let can_register_user_trigger_token = CanRegisterTrigger {
                authority: isi.object().action().authority().clone(),
            };
            can_register_user_trigger_token
                .is_owned_by(&executor.context().authority, executor.host())
        } {
            // Execute via core `Execute` implementation to ensure all invariants
            // and state mutations happen atomically after permission gating.
            execute!(executor, isi);
        }
        deny!(executor, "Can't register trigger owned by another account");
    }
    /// Unregisters a trigger once the caller is authorised and revokes related permissions.
    pub fn visit_unregister_trigger<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<Trigger>,
    ) {
        let trigger_id = isi.object();
        if executor.context().curr_block.is_genesis()
            || match is_trigger_owner(trigger_id, &executor.context().authority, executor.host()) {
                Err(err) => deny!(executor, err),
                Ok(is_trigger_owner) => is_trigger_owner,
            }
            || {
                let can_unregister_user_trigger_token = CanUnregisterTrigger {
                    trigger: trigger_id.clone(),
                };
                can_unregister_user_trigger_token
                    .is_owned_by(&executor.context().authority, executor.host())
            }
        {
            let err = revoke_permissions(executor, |permission| {
                is_permission_trigger_associated(permission, trigger_id)
            });
            if let Err(err) = err {
                deny!(executor, err);
            }
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't unregister trigger owned by another account"
        );
    }
    /// Increments the trigger repetition counter when the caller controls the trigger.
    pub fn visit_mint_trigger_repetitions<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Mint<u32, Trigger>,
    ) {
        let trigger_id = isi.destination();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_trigger_owner(trigger_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_mint_user_trigger_token = CanModifyTrigger {
            trigger: trigger_id.clone(),
        };
        if can_mint_user_trigger_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't mint execution count for trigger owned by another account"
        );
    }
    /// Decrements the trigger repetition counter for authorised callers.
    pub fn visit_burn_trigger_repetitions<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Burn<u32, Trigger>,
    ) {
        let trigger_id = isi.destination();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_trigger_owner(trigger_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_mint_user_trigger_token = CanModifyTrigger {
            trigger: trigger_id.clone(),
        };
        if can_mint_user_trigger_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't burn execution count for trigger owned by another account"
        );
    }
    /// Executes a trigger when the caller is the owner or holds the execute permission.
    pub fn visit_execute_trigger<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ExecuteTrigger,
    ) {
        let trigger_id = isi.trigger();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        let authority = &executor.context().authority;
        match is_trigger_owner(trigger_id, authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_execute_trigger_token = CanExecuteTrigger {
            trigger: trigger_id.clone(),
        };
        if can_execute_trigger_token.is_owned_by(authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't execute trigger owned by another account");
    }
    /// Sets metadata for a trigger when the caller may mutate its key-value store.
    pub fn visit_set_trigger_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetKeyValue<Trigger>,
    ) {
        let trigger_id = isi.object();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_trigger_owner(trigger_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_set_key_value_in_user_trigger_token = CanModifyTriggerMetadata {
            trigger: trigger_id.clone(),
        };
        if can_set_key_value_in_user_trigger_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't set value to the metadata of another trigger"
        );
    }
    /// Removes trigger metadata after verifying the caller may modify it.
    pub fn visit_remove_trigger_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RemoveKeyValue<Trigger>,
    ) {
        let trigger_id = isi.object();
        let isi = RemoveKeyValueBox::from(isi.clone());
        let isi = &isi;
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_trigger_owner(trigger_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_remove_key_value_in_trigger_token = CanModifyTriggerMetadata {
            trigger: trigger_id.clone(),
        };
        if can_remove_key_value_in_trigger_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't remove value from the metadata of another trigger"
        );
    }
    fn is_permission_trigger_associated(permission: &Permission, trigger_id: &TriggerId) -> bool {
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        match permission {
            AnyPermission::CanUnregisterTrigger(permission) => &permission.trigger == trigger_id,
            AnyPermission::CanExecuteTrigger(permission) => &permission.trigger == trigger_id,
            AnyPermission::CanModifyTrigger(permission) => &permission.trigger == trigger_id,
            AnyPermission::CanModifyTriggerMetadata(permission) => {
                &permission.trigger == trigger_id
            }
            AnyPermission::DpnAdmin(_)
            | AnyPermission::DpnUser(_)
            | AnyPermission::DpnInori(_)
            | AnyPermission::DpnSettlement(_)
            | AnyPermission::DpnEprGuard(_)
            | AnyPermission::CanRegisterGlobalDataTrigger(_)
            | AnyPermission::CanRegisterTrigger(_)
            | AnyPermission::CanManagePeers(_)
            | AnyPermission::CanManageLaneRelayEmergency(_)
            | AnyPermission::CanManageRuntimeUpgrades(_)
            | AnyPermission::CanManageConsensusKeys(_)
            | AnyPermission::CanManageConfidentialParams(_)
            | AnyPermission::CanRegisterDomain(_)
            | AnyPermission::CanUnregisterDomain(_)
            | AnyPermission::CanModifyDomainMetadata(_)
            | AnyPermission::CanRegisterAccount(_)
            | AnyPermission::CanUnregisterAccount(_)
            | AnyPermission::CanModifyAccountMetadata(_)
            | AnyPermission::CanReplaceAccountController(_)
            | AnyPermission::CanResolveAccountAlias(_)
            | AnyPermission::CanDelegateAccountAliasResolution(_)
            | AnyPermission::CanManageAccountAlias(_)
            | AnyPermission::CanManageAssetDefinitionAlias(_)
            | AnyPermission::CanReadAllLedgerData(_)
            | AnyPermission::CanReadAccountData(_)
            | AnyPermission::CanReadRestrictedDataspace(_)
            | AnyPermission::CanUnregisterAssetDefinition(_)
            | AnyPermission::CanModifyAssetDefinitionMetadata(_)
            | AnyPermission::CanManageAssetDefinitionConfidentialPolicy(_)
            | AnyPermission::CanModifyAssetMetadataWithDefinition(_)
            | AnyPermission::CanMintAssetWithDefinition(_)
            | AnyPermission::CanBurnAssetWithDefinition(_)
            | AnyPermission::CanTransferAssetWithDefinition(_)
            | AnyPermission::CanMintAssetToAccount(_)
            | AnyPermission::CanBurnAsset(_)
            | AnyPermission::CanModifyAssetMetadata(_)
            | AnyPermission::CanTransferAsset(_)
            | AnyPermission::CanSetAssetTransferAvailability(_)
            | AnyPermission::CanSetAssetTransferDailyLimit(_)
            | AnyPermission::CanSetAssetHoldingLimit(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanSetHijiriParameters(_)
            | AnyPermission::CanManageSccpGovernance(_)
            | AnyPermission::CanProposeSccpRouteGovernance(_)
            | AnyPermission::CanManageOfflineEscrow(_)
            | AnyPermission::CanActivateKagemushaRecursiveReleaseV4(_)
            | AnyPermission::CanManageOfflineDeviceAttestationPolicy(_)
            | AnyPermission::CanManageRoles(_)
            | AnyPermission::CanRegisterNft(_)
            | AnyPermission::CanUnregisterNft(_)
            | AnyPermission::CanTransferNft(_)
            | AnyPermission::CanModifyNftMetadata(_)
            | AnyPermission::CanUpgradeExecutor(_)
            | AnyPermission::CanRegisterSmartContractCode(_)
            | AnyPermission::CanInvokeContractEntrypoint(_)
            | AnyPermission::CanExecuteSettlement(_)
            | AnyPermission::CanManageFxCorridors(_)
            | AnyPermission::CanSetFxCorridorPolicy(_)
            | AnyPermission::CanBindSorafsAlias(_)
            | AnyPermission::CanDeclareSorafsCapacity(_)
            | AnyPermission::CanSubmitSorafsTelemetry(_)
            | AnyPermission::CanFileSorafsCapacityDispute(_)
            | AnyPermission::CanIssueSorafsReplicationOrder(_)
            | AnyPermission::CanCompleteSorafsReplicationOrder(_)
            | AnyPermission::CanSetSorafsPricing(_)
            | AnyPermission::CanSetSorafsReservePolicy(_)
            | AnyPermission::CanManageSorafsModeration(_)
            | AnyPermission::CanManageSorafsPopRegistry(_)
            | AnyPermission::CanOperateSorafsPopIssuer(_)
            | AnyPermission::CanUpsertSorafsProviderCredit(_)
            | AnyPermission::CanManageSoranetVpnQuoteIssuers(_)
            | AnyPermission::CanIssueSoranetVpnQuote(_)
            | AnyPermission::CanIngestSoranetPrivacy(_)
            | AnyPermission::CanRegisterOracleFeed(_)
            | AnyPermission::CanProposeOracleChange(_)
            | AnyPermission::CanVoteOracleChangeStage(_)
            | AnyPermission::CanRollbackOracleChange(_)
            | AnyPermission::CanResolveOracleDispute(_)
            | AnyPermission::CanManageTwitterBindings(_)
            | AnyPermission::CanPublishSpaceDirectoryManifest(_)
            | AnyPermission::CanPublishSpaceDirectoryManifestForUaid(_)
            | AnyPermission::CanPublishSpaceDirectoryManifestForAccountDomain(_)
            | AnyPermission::CanManageFeeSponsorProgram(_)
            | AnyPermission::CanEnrollFeeSponsorProgram(_) => false,
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::data_model::{
            account::AccountId,
            asset::{AssetDefinitionId, AssetId},
            domain::DomainId,
            nexus::FeeSponsorProgramId,
        };
        use core::str::FromStr as _;
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_executor_data_model::permission::{
            account::{
                AccountAliasPermissionScope, CanDelegateAccountAliasResolution,
                CanManageAccountAlias, CanResolveAccountAlias,
            },
            asset::{
                CanMintAssetWithDefinition, CanModifyAssetMetadata,
                CanModifyAssetMetadataWithDefinition,
            },
            asset_definition::{
                AssetDefinitionAliasPermissionScope, CanManageAssetDefinitionAlias,
            },
            nexus::{
                CanEnrollFeeSponsorProgram, CanManageFeeSponsorProgram,
                CanPublishSpaceDirectoryManifestForAccountDomain,
            },
            sccp::CanManageSccpGovernance,
            settlement::CanExecuteSettlement,
            sorafs::{
                CanBindSorafsAlias, CanCompleteSorafsReplicationOrder, CanDeclareSorafsCapacity,
                CanFileSorafsCapacityDispute, CanIssueSorafsReplicationOrder,
                CanManageSorafsModeration, CanManageSorafsPopRegistry, CanOperateSorafsPopIssuer,
                CanSetSorafsPricing, CanSetSorafsReservePolicy, CanSubmitSorafsTelemetry,
                CanUpsertSorafsProviderCredit,
            },
            soranet::{
                CanIngestSoranetPrivacy, CanIssueSoranetVpnQuote, CanManageSoranetVpnQuoteIssuers,
            },
        };
        fn fixture_key_pair(seed: u8) -> KeyPair {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("fixture seed must derive a valid keypair")
        }
        fn sample_account_id(seed: u8, _domain_id: &DomainId) -> AccountId {
            let keypair = fixture_key_pair(seed);
            AccountId::new(keypair.public_key().clone())
        }
        #[test]
        fn fixture_key_pair_uses_checked_seed_derivation() {
            assert_eq!(fixture_key_pair(1).algorithm(), Algorithm::Ed25519);
            assert!(
                KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
                "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
            );
        }
        fn sora_permissions() -> Vec<AnyPermission> {
            vec![
                AnyPermission::CanBindSorafsAlias(CanBindSorafsAlias),
                AnyPermission::CanDeclareSorafsCapacity(CanDeclareSorafsCapacity),
                AnyPermission::CanSubmitSorafsTelemetry(CanSubmitSorafsTelemetry),
                AnyPermission::CanFileSorafsCapacityDispute(CanFileSorafsCapacityDispute),
                AnyPermission::CanIssueSorafsReplicationOrder(CanIssueSorafsReplicationOrder),
                AnyPermission::CanCompleteSorafsReplicationOrder(CanCompleteSorafsReplicationOrder),
                AnyPermission::CanManageSorafsModeration(CanManageSorafsModeration),
                AnyPermission::CanManageSorafsPopRegistry(CanManageSorafsPopRegistry),
                AnyPermission::CanOperateSorafsPopIssuer(CanOperateSorafsPopIssuer),
                AnyPermission::CanSetSorafsPricing(CanSetSorafsPricing),
                AnyPermission::CanSetSorafsReservePolicy(CanSetSorafsReservePolicy),
                AnyPermission::CanUpsertSorafsProviderCredit(CanUpsertSorafsProviderCredit),
                AnyPermission::CanManageSoranetVpnQuoteIssuers(CanManageSoranetVpnQuoteIssuers),
                AnyPermission::CanIssueSoranetVpnQuote(CanIssueSoranetVpnQuote),
                AnyPermission::CanIngestSoranetPrivacy(CanIngestSoranetPrivacy),
                AnyPermission::CanManageSccpGovernance(CanManageSccpGovernance),
            ]
        }
        #[test]
        fn asset_metadata_permissions_not_trigger_associated() {
            let trigger_id =
                TriggerId::from_str("metadata_cleanup").expect("trigger id must be valid");
            let domain_id =
                DomainId::try_new("test", "universal").expect("domain id must be valid");
            let asset_definition_id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("test", "universal").unwrap(),
                "token".parse().unwrap(),
            );
            let account_id = sample_account_id(0x11, &domain_id);
            let asset_id = AssetId::new(asset_definition_id.clone(), account_id);
            let permission = Permission::from(AnyPermission::CanModifyAssetMetadataWithDefinition(
                CanModifyAssetMetadataWithDefinition {
                    asset_definition: asset_definition_id,
                },
            ));
            assert!(
                !is_permission_trigger_associated(&permission, &trigger_id),
                "metadata-with-definition permission must not bind to triggers"
            );
            let permission = Permission::from(AnyPermission::CanModifyAssetMetadata(
                CanModifyAssetMetadata { asset: asset_id },
            ));
            assert!(
                !is_permission_trigger_associated(&permission, &trigger_id),
                "asset-metadata permission must not bind to triggers"
            );
        }
        #[test]
        fn sora_permissions_not_trigger_associated() {
            let trigger_id =
                TriggerId::from_str("metadata_cleanup").expect("trigger id must be valid");
            for permission in sora_permissions() {
                let permission = Permission::from(permission);
                assert!(
                    !is_permission_trigger_associated(&permission, &trigger_id),
                    "Sora-specific permissions must not bind to triggers"
                );
            }
        }
        #[test]
        fn default_executor_forwards_the_complete_vpn_lifecycle() {
            let source = include_str!("mod.rs");
            let start = source
                .find("// Core owns the signature, chain/client binding, canonical policy,")
                .expect("VPN lifecycle dispatch marker");
            let tail = &source[start..];
            let end = tail
                .find("if let Some(isi) = any.downcast_ref::<SetAssetKeyValue>()")
                .expect("VPN lifecycle dispatch terminator");
            let dispatch = &tail[..end];
            for instruction in [
                "OpenVpnLeaseEscrow",
                "SettleVpnLease",
                "RefundExpiredVpnLease",
            ] {
                assert!(
                    dispatch.contains(instruction),
                    "default executor VPN lifecycle dispatch omitted {instruction}"
                );
            }
        }
        #[test]
        fn sora_permissions_not_domain_account_or_definition_associated() {
            let domain_id =
                DomainId::try_new("test", "universal").expect("domain id must be valid");
            let account_id = sample_account_id(0x12, &domain_id);
            let asset_definition_id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("test", "universal").unwrap(),
                "token".parse().unwrap(),
            );
            for permission in sora_permissions() {
                let permission = Permission::from(permission);
                assert!(
                    !domain::is_permission_domain_associated(&permission, &domain_id, &[]),
                    "Sora-specific permissions must not bind to domains"
                );
                assert!(
                    !account::is_permission_account_associated(&permission, &account_id),
                    "Sora-specific permissions must not bind to accounts"
                );
                assert!(
                    !asset_definition::is_permission_asset_definition_associated(
                        &permission,
                        &asset_definition_id
                    ),
                    "Sora-specific permissions must not bind to asset definitions"
                );
            }
        }
        #[test]
        fn fee_sponsor_permission_associations() {
            let domain_id =
                DomainId::try_new("test", "universal").expect("domain id must be valid");
            let sponsor = sample_account_id(0x21, &domain_id);
            let other_account = sample_account_id(0x22, &domain_id);
            let other_domain =
                DomainId::try_new("other", "universal").expect("domain id must be valid");
            let asset_definition_id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("test", "universal").unwrap(),
                "token".parse().unwrap(),
            );
            let trigger_id =
                TriggerId::from_str("fee_sponsor_trigger").expect("trigger id must be valid");
            let program_id = FeeSponsorProgramId::new(
                sponsor.clone(),
                "default"
                    .parse()
                    .expect("fee sponsor program name is valid"),
            );
            let permissions = [
                Permission::from(AnyPermission::CanManageFeeSponsorProgram(
                    CanManageFeeSponsorProgram {
                        sponsor: sponsor.clone(),
                    },
                )),
                Permission::from(AnyPermission::CanEnrollFeeSponsorProgram(
                    CanEnrollFeeSponsorProgram { program_id },
                )),
            ];
            for permission in permissions {
                assert!(!domain::is_permission_domain_associated(
                    &permission,
                    &domain_id,
                    &[]
                ));
                assert!(!domain::is_permission_domain_associated(
                    &permission,
                    &other_domain,
                    &[]
                ));
                assert!(account::is_permission_account_associated(
                    &permission,
                    &sponsor
                ));
                assert!(!account::is_permission_account_associated(
                    &permission,
                    &other_account
                ));
                assert!(
                    !asset_definition::is_permission_asset_definition_associated(
                        &permission,
                        &asset_definition_id
                    )
                );
                assert!(!is_permission_trigger_associated(&permission, &trigger_id));
            }
        }
        #[test]
        fn settlement_consent_follows_account_definition_and_domain_lifecycles() {
            let domain_id =
                DomainId::try_new("settlement", "universal").expect("domain id must be valid");
            let other_domain =
                DomainId::try_new("other", "universal").expect("domain id must be valid");
            let debited_account = sample_account_id(0x24, &domain_id);
            let other_account = sample_account_id(0x25, &other_domain);
            let asset_definition_id = AssetDefinitionId::derive_from_components(
                domain_id.clone(),
                "cash".parse().expect("asset name"),
            );
            let permission =
                Permission::from(AnyPermission::CanExecuteSettlement(CanExecuteSettlement {
                    debited_asset: AssetId::new(
                        asset_definition_id.clone(),
                        debited_account.clone(),
                    ),
                    settlement_id: "cleanup_consent".parse().expect("settlement id"),
                    intent_hash: Hash::new(b"cleanup-bound settlement consent"),
                }));
            assert!(account::is_permission_account_associated(
                &permission,
                &debited_account
            ));
            assert!(!account::is_permission_account_associated(
                &permission,
                &other_account
            ));
            assert!(asset_definition::is_permission_asset_definition_associated(
                &permission,
                &asset_definition_id
            ));
            assert!(domain::is_permission_domain_associated(
                &permission,
                &domain_id,
                core::slice::from_ref(&asset_definition_id),
            ));
            assert!(!domain::is_permission_domain_associated(
                &permission,
                &other_domain,
                &[],
            ));
        }
        #[test]
        fn account_domain_manifest_and_program_associations_remain_independent() {
            let hbl_domain = DomainId::try_new("hbl", "sbp").expect("HBL domain must be valid");
            let ubl_domain = DomainId::try_new("ubl", "sbp").expect("UBL domain must be valid");
            let sponsor = sample_account_id(0x31, &hbl_domain);
            let unrelated = sample_account_id(0x33, &ubl_domain);
            let publisher = Permission::from(
                AnyPermission::CanPublishSpaceDirectoryManifestForAccountDomain(
                    CanPublishSpaceDirectoryManifestForAccountDomain {
                        dataspace: DataSpaceId::new(10),
                        domain: hbl_domain.clone(),
                    },
                ),
            );
            assert!(domain::is_permission_domain_associated(
                &publisher,
                &hbl_domain,
                &[]
            ));
            assert!(!domain::is_permission_domain_associated(
                &publisher,
                &ubl_domain,
                &[]
            ));
            let enrollment = Permission::from(AnyPermission::CanEnrollFeeSponsorProgram(
                CanEnrollFeeSponsorProgram {
                    program_id: FeeSponsorProgramId::new(
                        sponsor.clone(),
                        "retail".parse().expect("retail sponsor program"),
                    ),
                },
            ));
            assert!(!domain::is_permission_domain_associated(
                &enrollment,
                &hbl_domain,
                &[]
            ));
            assert!(account::is_permission_account_associated(
                &enrollment,
                &sponsor
            ));
            assert!(!account::is_permission_account_associated(
                &enrollment,
                &unrelated
            ));
        }
        #[test]
        fn asset_permission_domain_association_uses_authoritative_ownership_set() {
            let domain_id = DomainId::try_new("issuer", "universal").expect("domain id");
            let other_domain = DomainId::try_new("other", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::derive_from_components(
                other_domain,
                "token".parse().expect("asset name"),
            );
            let permission = Permission::from(AnyPermission::CanMintAssetWithDefinition(
                CanMintAssetWithDefinition {
                    asset_definition: definition_id.clone(),
                },
            ));
            assert!(domain::is_permission_domain_associated(
                &permission,
                &domain_id,
                core::slice::from_ref(&definition_id),
            ));
            assert!(!domain::is_permission_domain_associated(
                &permission,
                &domain_id,
                &[],
            ));
        }
        #[test]
        fn account_alias_domain_permissions_match_qualified_domain() {
            let domain_id =
                DomainId::try_new("test", "universal").expect("domain id must be valid");
            let other_domain =
                DomainId::try_new("other", "universal").expect("domain id must be valid");
            let resolve_permission = Permission::from(AnyPermission::CanResolveAccountAlias(
                CanResolveAccountAlias {
                    scope: AccountAliasPermissionScope::Domain(domain_id.clone()),
                },
            ));
            let delegate_permission =
                Permission::from(AnyPermission::CanDelegateAccountAliasResolution(
                    CanDelegateAccountAliasResolution {
                        scope: AccountAliasPermissionScope::Domain(domain_id.clone()),
                    },
                ));
            let manage_permission = Permission::from(AnyPermission::CanManageAccountAlias(
                CanManageAccountAlias {
                    scope: AccountAliasPermissionScope::Domain(domain_id.clone()),
                },
            ));
            let manage_asset_alias_permission = Permission::from(
                AnyPermission::CanManageAssetDefinitionAlias(CanManageAssetDefinitionAlias {
                    scope: AssetDefinitionAliasPermissionScope::Domain(domain_id.clone()),
                }),
            );
            assert!(
                domain::is_permission_domain_associated(&resolve_permission, &domain_id, &[]),
                "alias resolve permission should bind to the matching domain"
            );
            assert!(
                !domain::is_permission_domain_associated(&resolve_permission, &other_domain, &[]),
                "alias resolve permission should not bind to other domains"
            );
            assert!(
                domain::is_permission_domain_associated(&delegate_permission, &domain_id, &[]),
                "alias resolve-delegation permission should bind to the matching domain"
            );
            assert!(
                !domain::is_permission_domain_associated(&delegate_permission, &other_domain, &[]),
                "alias resolve-delegation permission should not bind to other domains"
            );
            assert!(
                domain::is_permission_domain_associated(&manage_permission, &domain_id, &[]),
                "alias manage permission should bind to the matching domain"
            );
            assert!(
                !domain::is_permission_domain_associated(&manage_permission, &other_domain, &[]),
                "alias manage permission should not bind to other domains"
            );
            assert!(
                domain::is_permission_domain_associated(
                    &manage_asset_alias_permission,
                    &domain_id,
                    &[]
                ),
                "asset-alias manage permission should bind to the matching domain"
            );
            assert!(
                !domain::is_permission_domain_associated(
                    &manage_asset_alias_permission,
                    &other_domain,
                    &[]
                ),
                "asset-alias manage permission should not bind to other domains"
            );
        }
    }
}
#[cfg(test)]
mod sorafs_permission_tests {
    use super::*;
    use crate::{Iroha, prelude, tests::with_mock_permissions};
    use core::num::NonZeroU64;
    use iroha_crypto::PublicKey;
    use iroha_data_model::{
        account::AccountId,
        block::BlockHeader,
        isi::sorafs::{
            AcceptSorafsModerationJurorAssignment, ActivateSorafsModerationCase,
            AppendSorafsPorReputationJournalEntry, AppendSorafsStreamTokenReputationJournalEntry,
            ApprovePinManifest, BindManifestAlias, CommitSorafsPopCredentialBatch,
            CompleteReplicationOrder, ExpireReplicationOrder, ExpireSorafsModerationChallenge,
            FinalizeSorafsModerationCase, FinalizeSorafsModerationSortition, IssueReplicationOrder,
            PublishSorafsPopRevocationList, RaiseSorafsModerationChallenge,
            RecordCapacityTelemetry, RegisterCapacityDeclaration, RegisterCapacityDispute,
            RegisterPinManifest, RegisterProviderOwner, RegisterSorafsModerationJurorEligibility,
            ResolveSorafsCapacityDispute, ResolveSorafsModerationChallenge, RetirePinManifest,
            ReviseReplicationOrderAssignments, RevokeProviderIngestCompletionAuthority,
            SetPricingSchedule, SetProviderIngestCompletionAuthority, SetSorafsModerationPolicy,
            SetSorafsPopIssuerPolicy, SetSorafsReputationJournalAuthorityPolicy,
            SubmitSorafsModerationAppeal, SubmitSorafsModerationCommit,
            SubmitSorafsModerationReveal, UnregisterProviderOwner, UpsertProviderCredit,
        },
        metadata::Metadata,
        permission::Permission as PermissionObject,
        prelude::{Quantity, ValidationFail},
        query::sorafs::prelude::{
            FindSorafsModerationAppeal, FindSorafsModerationEvents,
            FindSorafsModerationJurorEligibility, FindSorafsModerationPolicy,
            FindSorafsModerationSnapshot, FindSorafsModerationStatus,
            FindSorafsOrderbookCancellationByOrderId, FindSorafsOrderbookChannelById,
            FindSorafsOrderbookChannels, FindSorafsOrderbookEvents, FindSorafsOrderbookOrderById,
            FindSorafsOrderbookOrders, FindSorafsOrderbookPolicy, FindSorafsOrderbookReceiptById,
            FindSorafsOrderbookReceipts, FindSorafsOrderbookStatus, FindSorafsOrderbookTradeById,
            FindSorafsOrderbookTrades, FindSorafsPopAuditDigestBySequence,
            FindSorafsPopCommitmentRootByVersion, FindSorafsPopCredentialCommitmentByDigest,
            FindSorafsPopIssuerPolicy, FindSorafsPopRegistryStatus,
            FindSorafsPopRevocationByNonceCommitment, FindSorafsPopRevocationPublicationByVersion,
            FindSorafsReputationJournalAuthorityPolicy, FindSorafsReputationJournalEventBySourceId,
            FindSorafsReputationJournalEvents, FindSorafsReserveEvents,
        },
        sorafs::{
            capacity::{
                CapacityDeclarationRecord, CapacityDisputeEvidence, CapacityDisputeId,
                CapacityDisputeOutcome, CapacityDisputeRecord, CapacityTelemetryRecord, ProviderId,
            },
            moderation_ledger::{
                MODERATION_APPEAL_INTAKE_VERSION_V1, MODERATION_LEDGER_POLICY_VERSION_V1,
                ModerationAppealIntakeV1, ModerationChallengeDecisionV1, ModerationChallengeKindV1,
                ModerationFinalizedCursorV1, ModerationLedgerPolicyV1,
            },
            pin_registry::{
                ManifestAliasBinding, ManifestDigest, ProviderIngestCompletionAuthorityV1,
                ProviderIngestCompletionSignerPolicyV1, ProviderIngestFinalizedAnchorV1,
                ReplicationOrderId,
            },
            pop_registry::{POP_ISSUER_POLICY_VERSION_V1, PopIssuerPolicyV1},
            pricing::{PricingScheduleRecord, ProviderCreditRecord},
            reputation::{
                PorTerminalOutcomeV1, PorTerminalStatusV1,
                REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1, ReputationJournalAuthorityPolicyV1,
                ReputationJournalEntryV1, ReputationJournalFinalizedCursorV1,
                ReputationJournalPayloadV1, ReputationJournalSourceIdV1,
                StreamTokenValidationBindingV1, StreamTokenValidationOutcomeV1,
                StreamTokenValidationStatusV1,
            },
            reserve::ReserveFinalizedCursorV1,
        },
    };
    use iroha_executor_data_model::permission::sorafs::{
        CanBindSorafsAlias, CanCompleteSorafsReplicationOrder, CanFileSorafsCapacityDispute,
        CanIssueSorafsReplicationOrder, CanManageSorafsModeration, CanManageSorafsPopRegistry,
        CanManageSorafsReputationJournalPolicy, CanOperateSorafsPopIssuer,
        CanRecordSorafsReputationJournal, CanResolveSorafsCapacityDispute, CanSetSorafsPricing,
        CanSetSorafsReservePolicy, CanUpsertSorafsProviderCredit,
    };
    use iroha_executor_data_model::permission::{
        domain::CanRegisterDomain,
        parameter::{CanSetHijiriParameters, CanSetParameters},
        sccp::CanManageSccpGovernance,
    };
    const AUTHORITY_PUBLIC_KEY: &str =
        "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";
    const OWNER_PUBLIC_KEY: &str =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
    fn account_id_from_public_key_hex(hex_literal: &str) -> AccountId {
        let public_key: PublicKey = hex_literal
            .parse()
            .expect("test public key literal should parse");
        AccountId::new(public_key)
    }
    fn authority_account_id() -> AccountId {
        account_id_from_public_key_hex(AUTHORITY_PUBLIC_KEY)
    }
    fn owner_account_id() -> AccountId {
        account_id_from_public_key_hex(OWNER_PUBLIC_KEY)
    }
    fn authority_public_key_bytes() -> [u8; 32] {
        let authority = authority_account_id();
        let (_, bytes) = authority
            .expect_single_signatory()
            .try_to_bytes()
            .expect("authority public key bytes");
        bytes.try_into().expect("Ed25519 public key length")
    }
    #[derive(Debug, iroha_executor_derive::Visit)]
    struct MockExecutor {
        host: Iroha,
        ctx: prelude::Context,
        verdict: crate::data_model::executor::Result<(), ValidationFail>,
    }
    impl MockExecutor {
        fn new(genesis: bool) -> Self {
            let height = if genesis { 1 } else { 2 };
            let header = BlockHeader::new(
                NonZeroU64::new(height).expect("nonzero height"),
                None,
                None,
                None,
                0,
                0,
            );
            let authority = authority_account_id();
            Self {
                host: Iroha,
                ctx: prelude::Context {
                    authority,
                    curr_block: header,
                },
                verdict: Ok(()),
            }
        }
    }
    impl Execute for MockExecutor {
        fn host(&self) -> &Iroha {
            &self.host
        }
        fn context(&self) -> &prelude::Context {
            &self.ctx
        }
        fn context_mut(&mut self) -> &mut prelude::Context {
            &mut self.ctx
        }
        fn verdict(&self) -> &crate::data_model::executor::Result<(), ValidationFail> {
            &self.verdict
        }
        fn deny(&mut self, reason: ValidationFail) {
            self.verdict = Err(reason);
        }
    }
    fn assert_denied_without_permission<T: Clone>(
        instruction: T,
        visit: impl Fn(&mut MockExecutor, &T),
    ) {
        with_mock_permissions(vec![PermissionObject::from(CanBindSorafsAlias)], || {
            let mut executor = MockExecutor::new(false);
            visit(&mut executor, &instruction);
            assert!(
                executor.verdict().is_err(),
                "expected denial without permission"
            );
        });
    }
    fn assert_allowed_without_permission<T: Clone>(
        instruction: T,
        visit: impl Fn(&mut MockExecutor, &T),
    ) {
        let mut executor = MockExecutor::new(false);
        visit(&mut executor, &instruction);
        assert!(
            executor.verdict().is_ok(),
            "expected instruction to be permitted without permission"
        );
    }
    fn assert_allowed_with_permission<T: Clone>(
        instruction: T,
        permission: PermissionObject,
        visit: impl Fn(&mut MockExecutor, &T),
    ) {
        with_mock_permissions(vec![permission], || {
            let mut executor = MockExecutor::new(false);
            visit(&mut executor, &instruction);
            assert!(
                executor.verdict().is_ok(),
                "expected instruction to be permitted with permission"
            );
        });
    }
    fn assert_denied_with_permission<T: Clone>(
        instruction: T,
        permission: PermissionObject,
        visit: impl Fn(&mut MockExecutor, &T),
    ) {
        with_mock_permissions(vec![permission], || {
            let mut executor = MockExecutor::new(false);
            visit(&mut executor, &instruction);
            assert!(
                executor.verdict().is_err(),
                "expected instruction to remain denied with unrelated permission"
            );
        });
    }
    fn sample_provider_id() -> ProviderId {
        ProviderId::new([0xAB; 32])
    }
    fn sample_manifest_digest() -> ManifestDigest {
        ManifestDigest::new([0xCD; 32])
    }
    fn register_pin_manifest() -> RegisterPinManifest {
        RegisterPinManifest::new(
            include_bytes!("../../../../fixtures/sorafs_gateway/1.0.0/manifest_v1.to").to_vec(),
            None,
            None,
        )
    }
    fn approve_pin_manifest() -> ApprovePinManifest {
        ApprovePinManifest::new(sample_manifest_digest(), None, None)
    }
    fn retire_pin_manifest() -> RetirePinManifest {
        RetirePinManifest::new(sample_manifest_digest(), None)
    }
    fn bind_manifest_alias() -> BindManifestAlias {
        BindManifestAlias::new(
            sample_manifest_digest(),
            ManifestAliasBinding {
                name: "docs".to_owned(),
                namespace: "sora".to_owned(),
                proof: Vec::new(),
            },
            4,
            5,
        )
    }
    fn set_pop_issuer_policy() -> SetSorafsPopIssuerPolicy {
        SetSorafsPopIssuerPolicy::new(PopIssuerPolicyV1 {
            version: POP_ISSUER_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            issuer_account: authority_account_id(),
            issuer_public_key: authority_public_key_bytes(),
            max_credentials_per_batch: 16,
            max_revocations_per_publication: 16,
            max_credential_lifetime_secs: 86_400,
            max_future_clock_skew_secs: 30,
            paused: false,
        })
    }
    fn register_capacity_declaration() -> RegisterCapacityDeclaration {
        RegisterCapacityDeclaration::new(CapacityDeclarationRecord::new(
            sample_provider_id(),
            vec![0xAA],
            100,
            1,
            1,
            2,
            Metadata::default(),
        ))
    }
    fn record_capacity_telemetry() -> RecordCapacityTelemetry {
        RecordCapacityTelemetry::new(
            CapacityTelemetryRecord::new(
                sample_provider_id(),
                1,
                2,
                100,
                90,
                80,
                1,
                1,
                10_000,
                10_000,
                0,
                0,
                0,
                0,
                0,
            )
            .with_nonce(0),
        )
    }
    fn register_capacity_dispute() -> RegisterCapacityDispute {
        RegisterCapacityDispute::new(CapacityDisputeRecord::new_pending(
            CapacityDisputeId::new([0x01; 32]),
            sample_provider_id(),
            [0x02; 32],
            None,
            0,
            1,
            "desc".to_owned(),
            None,
            CapacityDisputeEvidence {
                digest: [0x03; 32],
                media_type: None,
                uri: None,
                size_bytes: None,
            },
            vec![0x04],
        ))
    }
    fn reputation_policy() -> ReputationJournalAuthorityPolicyV1 {
        let authority = authority_account_id();
        ReputationJournalAuthorityPolicyV1 {
            version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            por_recorder_authority: authority.clone(),
            dispute_recorder_authority: authority.clone(),
            token_recorder_authority: authority,
            max_source_age_ms: 24 * 60 * 60 * 1_000,
        }
    }
    fn por_reputation_entry() -> ReputationJournalEntryV1 {
        let policy = reputation_policy();
        ReputationJournalEntryV1::try_new(
            sample_provider_id(),
            policy.canonical_digest().expect("reputation policy digest"),
            authority_account_id(),
            1_700_000_000_000,
            None,
            ReputationJournalPayloadV1::PorTerminal(PorTerminalOutcomeV1 {
                challenge_id: [0x31; 32],
                manifest_digest: [0x32; 32],
                epoch_id: 1,
                drand_round: 2,
                forced: false,
                sample_count: 4,
                failed_samples: 0,
                issued_at_unix_ms: 1_699_999_998_000,
                deadline_at_unix_ms: 1_700_000_000_000,
                responded_at_unix_ms: Some(1_699_999_999_000),
                decided_at_unix_ms: 1_700_000_000_000,
                proof_digest: Some([0x33; 32]),
                repair_task_id: None,
                verifier_latency_ms: Some(7),
                status: PorTerminalStatusV1::Verified,
            }),
        )
        .expect("canonical PoR reputation fixture")
    }
    fn token_reputation_entry() -> ReputationJournalEntryV1 {
        let policy = reputation_policy();
        ReputationJournalEntryV1::try_new(
            sample_provider_id(),
            policy.canonical_digest().expect("reputation policy digest"),
            authority_account_id(),
            1_700_000_000_000,
            None,
            ReputationJournalPayloadV1::StreamTokenValidation(StreamTokenValidationOutcomeV1 {
                binding: StreamTokenValidationBindingV1 {
                    gateway_id: [0x41; 32],
                    gateway_sequence: 1,
                    request_context_digest: [0x42; 32],
                },
                token_body_digest: Some([0x43; 32]),
                token_key_version: Some(1),
                validated_at_unix_ms: 1_700_000_000_000,
                status: StreamTokenValidationStatusV1::Accepted,
            }),
        )
        .expect("canonical reputation fixture")
    }
    fn resolve_capacity_dispute() -> ResolveSorafsCapacityDispute {
        ResolveSorafsCapacityDispute::new(
            CapacityDisputeId::new([0x01; 32]),
            reputation_policy()
                .canonical_digest()
                .expect("reputation policy digest"),
            CapacityDisputeOutcome::Upheld,
            [0x44; 32],
            Some("upheld".to_owned()),
        )
    }
    fn issue_replication_order() -> IssueReplicationOrder {
        IssueReplicationOrder::new(ReplicationOrderId::new([0x11; 32]), vec![0x22], 1, 2)
    }
    fn provider_ingest_completion_authority() -> ProviderIngestCompletionAuthorityV1 {
        ProviderIngestCompletionAuthorityV1::new(
            owner_account_id(),
            ProviderIngestCompletionSignerPolicyV1 {
                policy_id: [0x13; 32],
                revision: 1,
                predecessor_digest: None,
                policy_digest: [0x14; 32],
            },
        )
    }
    fn complete_replication_order() -> CompleteReplicationOrder {
        CompleteReplicationOrder::new(
            ReplicationOrderId::new([0x11; 32]),
            ProviderId::new([0x12; 32]),
            3,
            provider_ingest_completion_authority(),
            1,
            ProviderIngestFinalizedAnchorV1 {
                height: 2,
                block_hash: [0x15; 32],
            },
        )
    }
    fn revise_replication_order_assignments() -> ReviseReplicationOrderAssignments {
        ReviseReplicationOrderAssignments::new(
            ReplicationOrderId::new([0x11; 32]),
            1,
            2,
            Vec::new(),
        )
    }
    fn set_provider_ingest_completion_authority() -> SetProviderIngestCompletionAuthority {
        SetProviderIngestCompletionAuthority::new(
            ProviderId::new([0x12; 32]),
            None,
            provider_ingest_completion_authority(),
        )
    }
    fn revoke_provider_ingest_completion_authority() -> RevokeProviderIngestCompletionAuthority {
        RevokeProviderIngestCompletionAuthority::new(
            ProviderId::new([0x12; 32]),
            provider_ingest_completion_authority(),
        )
    }
    fn expire_replication_order() -> ExpireReplicationOrder {
        ExpireReplicationOrder::new(ReplicationOrderId::new([0x11; 32]), 4)
    }
    fn set_pricing_schedule() -> SetPricingSchedule {
        SetPricingSchedule::new(PricingScheduleRecord::launch_default())
    }
    fn upsert_provider_credit() -> UpsertProviderCredit {
        UpsertProviderCredit::new(ProviderCreditRecord::new(
            sample_provider_id(),
            Quantity::from(1_u32),
            Quantity::zero(),
            Quantity::zero(),
            Quantity::zero(),
            0,
            0,
            Metadata::default(),
        ))
    }
    fn set_moderation_policy() -> SetSorafsModerationPolicy {
        SetSorafsModerationPolicy::new(ModerationLedgerPolicyV1 {
            version: MODERATION_LEDGER_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            challenge_voting_asset_id:
                iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                    iroha_data_model::domain::DomainId::try_new("sora", "universal")
                        .expect("governance domain"),
                    "xor".parse().expect("governance asset name"),
                ),
            challenge_bond_amount: Quantity::from(
                iroha_data_model::sorafs::moderation_ledger::MODERATION_CHALLENGE_BOND_AMOUNT_V1,
            ),
            challenge_escrow_account: authority_account_id(),
            challenge_slash_receiver_account: authority_account_id(),
            challenge_rejected_slash_bps:
                iroha_data_model::sorafs::moderation_ledger::MODERATION_CHALLENGE_REJECTED_SLASH_BPS_V1,
            challenge_resolution_grace_ms:
                iroha_data_model::sorafs::moderation_ledger::MODERATION_CHALLENGE_RESOLUTION_GRACE_MS_V1,
            max_panel_size: 8,
            max_candidate_pool_size: 32,
            max_waitlist_size: 8,
            max_exclusions_per_case: 16,
            max_total_window_ms: 90_000_000,
            max_challenges_per_case: 4,
            missing_commit_penalty_points: 10,
            unrevealed_commit_penalty_points: 20,
        })
    }
    fn moderation_appeal_intake() -> ModerationAppealIntakeV1 {
        let appellant = authority_account_id();
        ModerationAppealIntakeV1 {
            version: MODERATION_APPEAL_INTAKE_VERSION_V1,
            case_id: "appeal-case".to_owned(),
            round_id: "round-1".to_owned(),
            appellant: appellant.clone(),
            appealed_decision_digest: [0x11; 32],
            proof_token_digest: [0x12; 32],
            evidence_bundle_digest: [0x13; 32],
            appeal_deposit_lock_digest: [0x14; 32],
            appeal_finance_config_version: "finance-v1".to_owned(),
            policy_reference: "moderation-v1".to_owned(),
            evidence_uri: None,
            panel_size: 3,
            waitlist_size: 2,
            quorum: 2,
            exclusions: vec![appellant],
            registration_deadline_unix_ms: 1_000,
            acceptance_deadline_unix_ms: 2_000,
            commit_deadline_unix_ms: 3_000,
            challenge_submission_deadline_unix_ms: 4_000,
            challenge_resolution_deadline_unix_ms: 86_404_000,
            reveal_deadline_unix_ms: 86_405_000,
            policy_digest: [0x15; 32],
        }
    }
    fn register_provider_owner() -> RegisterProviderOwner {
        RegisterProviderOwner::new(sample_provider_id(), owner_account_id())
    }
    fn unregister_provider_owner() -> UnregisterProviderOwner {
        UnregisterProviderOwner::new(sample_provider_id())
    }
    macro_rules! sorafs_permission_case {
        ($name:ident, $instruction:expr, $permission:expr, $visitor:path) => {
            #[test]
            fn $name() {
                let instruction = $instruction;
                assert_denied_without_permission(instruction.clone(), $visitor);
                assert_allowed_with_permission(
                    instruction,
                    PermissionObject::from($permission),
                    $visitor,
                );
            }
        };
    }
    #[test]
    fn register_pin_manifest_is_public() {
        assert_allowed_without_permission(
            register_pin_manifest(),
            sorafs::visit_register_pin_manifest,
        );
    }
    #[test]
    fn approve_pin_manifest_relays_governed_envelopes_without_permission() {
        assert_allowed_without_permission(
            approve_pin_manifest(),
            sorafs::visit_approve_pin_manifest,
        );
    }
    #[test]
    fn retire_pin_manifest_defers_exact_owner_check_to_core() {
        assert_allowed_without_permission(retire_pin_manifest(), sorafs::visit_retire_pin_manifest);
    }
    sorafs_permission_case!(
        bind_manifest_alias_requires_permission,
        bind_manifest_alias(),
        CanBindSorafsAlias,
        sorafs::visit_bind_manifest_alias
    );
    #[test]
    fn register_capacity_declaration_is_public() {
        assert_allowed_without_permission(
            register_capacity_declaration(),
            sorafs::visit_register_capacity_declaration,
        );
    }
    #[test]
    fn record_capacity_telemetry_is_public() {
        assert_allowed_without_permission(
            record_capacity_telemetry(),
            sorafs::visit_record_capacity_telemetry,
        );
    }
    sorafs_permission_case!(
        record_capacity_dispute_requires_permission,
        register_capacity_dispute(),
        CanFileSorafsCapacityDispute,
        sorafs::visit_register_capacity_dispute
    );
    sorafs_permission_case!(
        resolve_capacity_dispute_requires_permission,
        resolve_capacity_dispute(),
        CanResolveSorafsCapacityDispute,
        sorafs::visit_resolve_capacity_dispute
    );
    sorafs_permission_case!(
        set_reputation_policy_requires_permission,
        SetSorafsReputationJournalAuthorityPolicy::new(reputation_policy()),
        CanManageSorafsReputationJournalPolicy,
        sorafs::visit_set_reputation_journal_authority_policy
    );
    sorafs_permission_case!(
        append_por_reputation_requires_permission,
        AppendSorafsPorReputationJournalEntry::new(por_reputation_entry()),
        CanRecordSorafsReputationJournal,
        sorafs::visit_append_por_reputation_journal_entry
    );
    sorafs_permission_case!(
        append_stream_token_reputation_requires_permission,
        AppendSorafsStreamTokenReputationJournalEntry::new(token_reputation_entry()),
        CanRecordSorafsReputationJournal,
        sorafs::visit_append_stream_token_reputation_journal_entry
    );
    sorafs_permission_case!(
        issue_replication_order_requires_permission,
        issue_replication_order(),
        CanIssueSorafsReplicationOrder,
        sorafs::visit_issue_replication_order
    );
    sorafs_permission_case!(
        complete_replication_order_requires_permission,
        complete_replication_order(),
        CanCompleteSorafsReplicationOrder,
        sorafs::visit_complete_replication_order
    );
    sorafs_permission_case!(
        revise_replication_order_assignments_requires_permission,
        revise_replication_order_assignments(),
        CanIssueSorafsReplicationOrder,
        sorafs::visit_revise_replication_order_assignments
    );
    sorafs_permission_case!(
        expire_replication_order_requires_permission,
        expire_replication_order(),
        CanIssueSorafsReplicationOrder,
        sorafs::visit_expire_replication_order
    );
    #[test]
    fn retired_direct_provider_owner_instructions_reach_core_for_uniform_rejection() {
        assert_allowed_without_permission(
            register_provider_owner(),
            sorafs::visit_register_provider_owner,
        );
        assert_allowed_without_permission(
            unregister_provider_owner(),
            sorafs::visit_unregister_provider_owner,
        );
    }
    #[test]
    fn completion_authority_instructions_reach_core_owner_check() {
        assert_allowed_without_permission(
            set_provider_ingest_completion_authority(),
            sorafs::visit_set_provider_ingest_completion_authority,
        );
        assert_allowed_without_permission(
            revoke_provider_ingest_completion_authority(),
            sorafs::visit_revoke_provider_ingest_completion_authority,
        );
    }
    sorafs_permission_case!(
        set_pricing_schedule_requires_permission,
        set_pricing_schedule(),
        CanSetSorafsPricing,
        sorafs::visit_set_pricing_schedule
    );
    sorafs_permission_case!(
        upsert_provider_credit_requires_permission,
        upsert_provider_credit(),
        CanUpsertSorafsProviderCredit,
        sorafs::visit_upsert_provider_credit
    );
    sorafs_permission_case!(
        set_moderation_policy_requires_permission,
        set_moderation_policy(),
        CanManageSorafsModeration,
        sorafs::visit_set_moderation_policy
    );
    #[test]
    fn moderation_appeal_eligibility_and_acceptance_are_public_at_executor_layer() {
        assert_allowed_without_permission(
            SubmitSorafsModerationAppeal::new(moderation_appeal_intake()),
            sorafs::visit_submit_moderation_appeal,
        );
        assert_allowed_without_permission(
            RegisterSorafsModerationJurorEligibility::new(
                "appeal-case".to_owned(),
                "round-1".to_owned(),
                vec![0x01],
            ),
            sorafs::visit_register_moderation_juror_eligibility,
        );
        assert_allowed_without_permission(
            AcceptSorafsModerationJurorAssignment::new(
                "appeal-case".to_owned(),
                "round-1".to_owned(),
                [0x02; 32],
            ),
            sorafs::visit_accept_moderation_juror_assignment,
        );
    }
    sorafs_permission_case!(
        finalize_moderation_sortition_requires_permission,
        FinalizeSorafsModerationSortition::new(
            "appeal-case".to_owned(),
            "round-1".to_owned(),
            [0x03; 32],
            [0x04; 32],
            vec![authority_account_id()],
            Vec::new(),
        ),
        CanManageSorafsModeration,
        sorafs::visit_finalize_moderation_sortition
    );
    sorafs_permission_case!(
        activate_moderation_case_requires_permission,
        ActivateSorafsModerationCase::new(
            "appeal-case".to_owned(),
            "round-1".to_owned(),
            [0x04; 32],
        ),
        CanManageSorafsModeration,
        sorafs::visit_activate_moderation_case
    );
    sorafs_permission_case!(
        set_pop_issuer_policy_requires_permission,
        set_pop_issuer_policy(),
        CanManageSorafsPopRegistry,
        sorafs::visit_set_pop_issuer_policy
    );
    sorafs_permission_case!(
        commit_pop_credential_batch_requires_permission,
        CommitSorafsPopCredentialBatch::new(vec![0x01]),
        CanOperateSorafsPopIssuer,
        sorafs::visit_commit_pop_credential_batch
    );
    sorafs_permission_case!(
        publish_pop_revocations_requires_permission,
        PublishSorafsPopRevocationList::new(vec![0x01], [1; 32]),
        CanOperateSorafsPopIssuer,
        sorafs::visit_publish_pop_revocation_list
    );
    #[test]
    fn moderation_commit_submission_is_public_at_executor_layer() {
        assert_allowed_without_permission(
            SubmitSorafsModerationCommit::new(vec![0x01]),
            sorafs::visit_submit_moderation_commit,
        );
    }
    #[test]
    fn moderation_challenge_raise_expiry_and_reveal_are_public_at_executor_layer() {
        assert_allowed_without_permission(
            RaiseSorafsModerationChallenge::new(
                "appeal-case".to_owned(),
                "round-1".to_owned(),
                "challenge-1".to_owned(),
                ModerationChallengeKindV1::Other,
                None,
                [0x31; 32],
                "public challenge".to_owned(),
            ),
            sorafs::visit_raise_moderation_challenge,
        );
        assert_allowed_without_permission(
            ExpireSorafsModerationChallenge::new(
                "appeal-case".to_owned(),
                "round-1".to_owned(),
                "challenge-1".to_owned(),
            ),
            sorafs::visit_expire_moderation_challenge,
        );
        assert_allowed_without_permission(
            SubmitSorafsModerationReveal::new(vec![0x01]),
            sorafs::visit_submit_moderation_reveal,
        );
    }
    sorafs_permission_case!(
        resolve_moderation_challenge_requires_permission,
        ResolveSorafsModerationChallenge::new(
            "appeal-case".to_owned(),
            "round-1".to_owned(),
            "challenge-1".to_owned(),
            ModerationChallengeDecisionV1::Accepted,
        ),
        CanManageSorafsModeration,
        sorafs::visit_resolve_moderation_challenge
    );
    sorafs_permission_case!(
        finalize_moderation_case_requires_permission,
        FinalizeSorafsModerationCase::new("appeal-case".to_owned(), "round-1".to_owned()),
        CanManageSorafsModeration,
        sorafs::visit_finalize_moderation_case
    );
    #[test]
    fn moderation_transparency_queries_are_public() {
        assert_allowed_without_permission(
            FindSorafsModerationPolicy,
            sorafs::visit_find_sorafs_moderation_policy,
        );
        assert_allowed_without_permission(
            FindSorafsModerationStatus,
            sorafs::visit_find_sorafs_moderation_status,
        );
        assert_allowed_without_permission(
            FindSorafsModerationAppeal::new("appeal-case".to_owned(), "round-1".to_owned()),
            sorafs::visit_find_sorafs_moderation_appeal,
        );
        assert_allowed_without_permission(
            FindSorafsModerationEvents::new(
                ModerationFinalizedCursorV1 {
                    height: 7,
                    block_hash: [0x44; 32],
                },
                None,
                16,
            ),
            sorafs::visit_find_sorafs_moderation_events,
        );
    }
    #[test]
    fn reputation_journal_query_is_public_transparency_state() {
        let cursor = ReputationJournalFinalizedCursorV1 {
            height: 7,
            block_hash: [0x45; 32],
            finalized_at_unix_ms: 1_700_000_000_000,
        };
        assert_allowed_without_permission(
            FindSorafsReputationJournalEvents::new(Some(cursor), None, 16),
            sorafs::visit_find_sorafs_reputation_journal_events,
        );
        assert_allowed_without_permission(
            FindSorafsReputationJournalEventBySourceId::new(
                ReputationJournalSourceIdV1([0x46; 32]),
                Some(cursor),
            ),
            super::visit_find_sorafs_reputation_journal_event_by_source_id,
        );
    }
    #[test]
    fn reputation_journal_authority_policy_query_requires_operator_permission() {
        let query = FindSorafsReputationJournalAuthorityPolicy;
        assert_denied_without_permission(
            query,
            sorafs::visit_find_sorafs_reputation_journal_authority_policy,
        );
        assert_allowed_with_permission(
            query,
            PermissionObject::from(CanManageSorafsReputationJournalPolicy),
            sorafs::visit_find_sorafs_reputation_journal_authority_policy,
        );
        assert_allowed_with_permission(
            query,
            PermissionObject::from(CanRecordSorafsReputationJournal),
            sorafs::visit_find_sorafs_reputation_journal_authority_policy,
        );
        assert_allowed_with_permission(
            query,
            PermissionObject::from(CanResolveSorafsCapacityDispute),
            sorafs::visit_find_sorafs_reputation_journal_authority_policy,
        );
    }
    #[test]
    fn complete_moderation_snapshot_is_manager_only() {
        let query = FindSorafsModerationSnapshot::new(8, 16);
        assert_denied_without_permission(query, sorafs::visit_find_sorafs_moderation_snapshot);
        assert_allowed_with_permission(
            query,
            PermissionObject::from(CanManageSorafsModeration),
            sorafs::visit_find_sorafs_moderation_snapshot,
        );
    }
    #[test]
    fn moderation_eligibility_query_is_self_or_manager_only() {
        assert_allowed_without_permission(
            FindSorafsModerationJurorEligibility::new(
                "appeal-case".to_owned(),
                "round-1".to_owned(),
                authority_account_id(),
            ),
            sorafs::visit_find_sorafs_moderation_juror_eligibility,
        );
        let other_juror = FindSorafsModerationJurorEligibility::new(
            "appeal-case".to_owned(),
            "round-1".to_owned(),
            owner_account_id(),
        );
        assert_denied_without_permission(
            other_juror.clone(),
            sorafs::visit_find_sorafs_moderation_juror_eligibility,
        );
        assert_allowed_with_permission(
            other_juror,
            PermissionObject::from(CanManageSorafsModeration),
            sorafs::visit_find_sorafs_moderation_juror_eligibility,
        );
    }
    #[test]
    fn derived_default_visit_dispatches_private_juror_eligibility_query() {
        with_mock_permissions(vec![PermissionObject::from(CanBindSorafsAlias)], || {
            let query = iroha_smart_contract::data_model::query::AnyQueryBox::Singular(
                FindSorafsModerationJurorEligibility::new(
                    "appeal-case".to_owned(),
                    "round-1".to_owned(),
                    owner_account_id(),
                )
                .into(),
            );
            let mut executor = MockExecutor::new(false);
            executor.visit_query(&query);
            assert!(
                executor.verdict().is_err(),
                "derived default Visit dispatch must not bypass foreign juror privacy"
            );
        });
    }
    fn orderbook_page_queries() -> Vec<iroha_smart_contract::data_model::query::AnyQueryBox> {
        [
            FindSorafsOrderbookTrades::new(None, None, 10).into(),
            FindSorafsOrderbookChannels::new(None, None, None, 10).into(),
            FindSorafsOrderbookEvents::new(None, None, 10).into(),
        ]
        .into_iter()
        .map(iroha_smart_contract::data_model::query::AnyQueryBox::Singular)
        .collect()
    }
    #[test]
    fn derived_default_visit_dispatches_orderbook_pages_through_permission_checks() {
        with_mock_permissions(vec![PermissionObject::from(CanBindSorafsAlias)], || {
            for query in orderbook_page_queries() {
                let mut executor = MockExecutor::new(false);
                executor.visit_query(&query);
                assert!(
                    executor.verdict().is_err(),
                    "derived dispatch must reject an unrelated SoraFS permission"
                );
            }
        });
        for permission in [
            PermissionObject::from(CanSetSorafsPricing),
            PermissionObject::from(CanCompleteSorafsReplicationOrder),
        ] {
            with_mock_permissions(vec![permission], || {
                for query in orderbook_page_queries() {
                    let mut executor = MockExecutor::new(false);
                    executor.visit_query(&query);
                    assert!(
                        executor.verdict().is_ok(),
                        "derived dispatch must accept an orderbook operator permission"
                    );
                }
            });
        }
    }
    include!("sccp_route_governance_permission_tests.rs");
    include!("governance_query_tail_tests.rs");
}
/// Permission-checked visitors for direct permission grants and revocations.
pub mod permission {
    use super::*;
    macro_rules! impl_execute {
        ($executor:ident, $isi:ident, $method:ident, $isi_type:ty) => {
            let account_id = $isi.destination().clone();
            let permission = $isi.object();
            if let Ok(any_permission) = AnyPermission::try_from(permission) {
                if !$executor.context().curr_block.is_genesis() {
                    if let Err(error) = crate::permission::ValidateGrantRevoke::$method(
                        &any_permission,
                        &$executor.context().authority,
                        $executor.context(),
                        $executor.host(),
                    ) {
                        deny!($executor, error);
                    }
                }
                let isi = &<$isi_type>::account_permission(any_permission, account_id);
                execute!($executor, isi);
            }
            deny!(
                $executor,
                ValidationFail::NotPermitted(format!("{permission:?}: Unknown permission"))
            );
        };
    }
    /// Grants an account-level permission after validating the caller's authority.
    pub fn visit_grant_account_permission<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Grant<Permission, Account>,
    ) {
        impl_execute!(executor, isi, validate_grant, Grant<Permission, Account>);
    }
    /// Revokes an account-level permission once the caller passes permission checks.
    pub fn visit_revoke_account_permission<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Revoke<Permission, Account>,
    ) {
        impl_execute!(executor, isi, validate_revoke, Revoke<Permission, Account>);
    }
}
include!("governed_offline_permission_tests.rs");
include!("dpn_permission_tests.rs");
/// Permission-checked visitor for executor upgrade instructions.
pub mod executor {
    use super::*;
    use iroha_executor_data_model::permission::executor::CanUpgradeExecutor;
    /// Upgrades the executor when invoked during genesis or by an authorised authority.
    pub fn visit_upgrade<V: Execute + Visit + ?Sized>(executor: &mut V, isi: &Upgrade) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanUpgradeExecutor.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't upgrade executor");
    }
}
/// Visitor for log instructions which are always permitted.
pub mod log {
    use super::*;
    declare_execute_visitors! {
        /// Emits a log instruction directly because logging has no permission gates.
        visit_log(Log);
    }
}
/// Permission-checked visitors for bridge instructions.
pub mod bridge {
    use super::*;
    declare_execute_visitors! {
        /// Records a bridge receipt without additional permission gates.
        visit_record_bridge_receipt(RecordBridgeReceipt);
    }
    /// Applies one typed governed SCCP registry action.
    pub fn visit_apply_sccp_route_governance<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        _isi: &ApplySccpRouteGovernance,
    ) {
        deny!(
            executor,
            "direct SCCP route mutation is retired; an exact due Parliament certificate must apply the action"
        )
    }
}
