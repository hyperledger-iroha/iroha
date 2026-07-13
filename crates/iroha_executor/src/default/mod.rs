//! Definition of Iroha default executor and accompanying execute functions.

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
    visit_set_asset_key_value, visit_set_asset_transfer_control, visit_set_asset_transfer_freeze,
    visit_transfer_asset_quantity,
};
/// Re-export asset-definition visitor helpers used by the default executor.
pub use asset_definition::{
    visit_register_asset_definition, visit_remove_asset_definition_key_value,
    visit_set_asset_definition_key_value, visit_transfer_asset_definition,
    visit_unregister_asset_definition,
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
    visit_enact_referendum, visit_propose_sccp_route_governance, visit_register_citizen,
};
use iroha_smart_contract::data_model::{
    isi::{
        AcceptSorafsModerationJurorAssignment, ActivatePublicLaneValidator,
        ActivateSorafsModerationCase, ApprovePinManifest, BindManifestAlias,
        CancelSorafsOrderbookOrder, CommitSorafsPopCredentialBatch, CompleteReplicationOrder,
        ExitPublicLaneValidator, ExpireReplicationOrder, FinalizeSorafsModerationCase,
        FinalizeSorafsModerationSortition, IssueReplicationOrder, PublishSorafsPopRevocationList,
        RaiseSorafsModerationChallenge, RecordCapacityTelemetry,
        RecordSorafsOrderbookSettlementReceipt, RegisterCapacityDeclaration,
        RegisterCapacityDispute, RegisterPeerWithPop, RegisterPinManifest, RegisterProviderOwner,
        RegisterPublicLaneValidator, RegisterSorafsModerationJurorEligibility, RemoveAssetKeyValue,
        ResolveSorafsModerationChallenge, RetirePinManifest, SetAssetKeyValue,
        SetLaneRelayEmergencyValidators, SetPricingSchedule, SetSorafsModerationPolicy,
        SetSorafsOrderbookPolicy, SetSorafsPopIssuerPolicy, SubmitSorafsModerationAppeal,
        SubmitSorafsModerationCommit, SubmitSorafsModerationReveal, SubmitSorafsOrderbookOrder,
        UnregisterProviderOwner, UpsertProviderCredit,
        bridge::{ApplySccpRouteGovernance, RecordBridgeReceipt},
        contract_alias::SetContractAlias,
        defi::DeFiInstructionBox,
        governance::{EnactReferendum, ProposeSccpRouteGovernance, RegisterCitizen},
        repo::{RepoInstructionBox, RepoIsi, RepoMarginCallIsi, ReverseRepoIsi},
        settlement::SettlementInstructionBox,
        smart_contract_code::{
            ActivateContractInstance, CancelSmartContractCodeUpload, DeactivateContractInstance,
            FinalizeSmartContractCodeUpload, RegisterSmartContractBytes, RegisterSmartContractCode,
            RemoveSmartContractBytes, UploadSmartContractCodeChunk,
        },
    },
    prelude::*,
    query::error::{FindError, QueryExecutionFail},
    visit::Visit,
};
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
/// Re-export permission-checked `SoraFS` orderbook query visitors.
pub use sorafs::{
    visit_find_sorafs_moderation_appeal, visit_find_sorafs_moderation_case,
    visit_find_sorafs_moderation_challenge, visit_find_sorafs_moderation_commit,
    visit_find_sorafs_moderation_juror_eligibility, visit_find_sorafs_moderation_no_show,
    visit_find_sorafs_moderation_outcome, visit_find_sorafs_moderation_policy,
    visit_find_sorafs_moderation_reveal, visit_find_sorafs_moderation_status,
    visit_find_sorafs_orderbook_cancellation_by_order_id, visit_find_sorafs_orderbook_order_by_id,
    visit_find_sorafs_orderbook_orders, visit_find_sorafs_orderbook_policy,
    visit_find_sorafs_orderbook_receipt_by_id, visit_find_sorafs_orderbook_receipts,
    visit_find_sorafs_orderbook_status, visit_find_sorafs_pop_audit_digest_by_sequence,
    visit_find_sorafs_pop_commitment_root_by_version,
    visit_find_sorafs_pop_credential_commitment_by_digest, visit_find_sorafs_pop_issuer_policy,
    visit_find_sorafs_pop_registry_status, visit_find_sorafs_pop_revocation_by_nonce_commitment,
    visit_find_sorafs_pop_revocation_publication_by_version,
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

use crate::{
    Execute, deny, execute,
    permission::{AnyPermission, ExecutorPermission as _},
};

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
/// Torii can onboard a transaction authority that does not exist yet, but the
/// ordinary `CanRegisterSmartContractCode` grant policy is genesis-only. Keep
/// this exception bound to an auditable, ordered native-deployment prefix:
/// register the transaction authority, grant that exact authority the exact
/// deployment permission, then either upload code or register a manifest for
/// code that is already present. Whether the account is actually absent is
/// checked against pre-transaction state in [`visit_transaction`].
fn has_contract_deployment_self_bootstrap_prefix(
    authority: &AccountId,
    instructions: &[InstructionBox],
) -> bool {
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
    use std::num::NonZeroU64;

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

    use super::*;
    use crate::{Iroha, prelude};

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
        let contract_address =
            ContractAddress::derive(0x1234, &authority, 7, DataSpaceId::UNIVERSAL)
                .expect("contract address");
        let instructions: Vec<InstructionBox> = vec![
            RegisterSmartContractCode {
                manifest: manifest(),
            }
            .into(),
            DeactivateContractInstance {
                contract_address: contract_address.clone(),
                reason: Some("dispatch fixture".to_owned()),
            }
            .into(),
            ActivateContractInstance {
                contract_address: contract_address.clone(),
                code_hash,
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
            Json::from_string_unchecked("{\"unexpected\":true}".to_owned()),
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

/// Execute [`InstructionBox`] by delegating to the appropriate visitor
/// implementation.
pub fn visit_instruction<V: Execute + Visit + ?Sized>(executor: &mut V, isi: &InstructionBox) {
    isi.dispatch(executor);
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
        if let Some(isi) = any.downcast_ref::<SetAssetKeyValue>() {
            visit_set_asset_key_value(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetAssetTransferFreeze>() {
            asset::visit_set_asset_transfer_freeze(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetAssetTransferControl>() {
            asset::visit_set_asset_transfer_control(executor, isi);
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
        if let Some(isi) = any.downcast_ref::<IssueReplicationOrder>() {
            sorafs::visit_issue_replication_order(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<CompleteReplicationOrder>() {
            sorafs::visit_complete_replication_order(executor, isi);
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
        if let Some(isi) = any.downcast_ref::<EnactReferendum>() {
            governance::visit_enact_referendum(executor, isi);
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
        if let Some(isi) = any.downcast_ref::<SetPricingSchedule>() {
            sorafs::visit_set_pricing_schedule(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<UpsertProviderCredit>() {
            sorafs::visit_upsert_provider_credit(executor, isi);
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
    use iroha_executor_data_model::permission::settlement::{
        CanManageFxCorridors, CanSetFxCorridorPolicy, CanSettleFxCorridor,
    };

    use super::*;

    /// Dispatch a settlement instruction, enforcing typed corridor policy scopes.
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
            SettlementInstructionBox::SettleFxCorridor(settle) => {
                let exact = CanSettleFxCorridor {
                    policy_id: settle.policy_id().clone(),
                };
                if CanManageFxCorridors.is_owned_by(authority, executor.host())
                    || exact.is_owned_by(authority, executor.host())
                {
                    execute!(executor, isi);
                }
                deny!(
                    executor,
                    "FX settlement requires an exact typed corridor permission"
                );
            }
            SettlementInstructionBox::Dvp(_) | SettlementInstructionBox::Pvp(_) => {
                execute!(executor, isi);
            }
        }
    }
}

/// Permission-aware dispatch for SCCP governance proposal instructions.
pub mod governance {
    use super::*;

    /// Dispatch a typed SCCP route-governance proposal to Core, which admits registered citizens
    /// or holders of `CanProposeSccpRouteGovernance` (including role grants).
    pub fn visit_propose_sccp_route_governance<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ProposeSccpRouteGovernance,
    ) {
        execute!(executor, isi)
    }

    /// Dispatch referendum enactment to Core, which enforces the typed enactment permission and
    /// idempotent proposal lifecycle.
    pub fn visit_enact_referendum<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &EnactReferendum,
    ) {
        execute!(executor, isi)
    }

    /// Dispatch citizen registration to Core, which enforces self-registration and the configured
    /// citizenship bond floor against committed governance parameters.
    pub fn visit_register_citizen<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterCitizen,
    ) {
        execute!(executor, isi)
    }
}

/// Permission-checked visitors for peer management instructions.
pub mod peer {
    use iroha_executor_data_model::permission::peer::CanManagePeers;

    use super::*;

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
    use iroha_executor_data_model::permission::peer::CanManagePeers;

    use super::*;

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
    use iroha_executor_data_model::permission::peer::CanManageLaneRelayEmergency;

    use super::*;

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
    use iroha_executor_data_model::permission::sorafs::{
        CanApproveSorafsPin, CanBindSorafsAlias, CanCompleteSorafsReplicationOrder,
        CanFileSorafsCapacityDispute, CanIssueSorafsReplicationOrder, CanManageSorafsModeration,
        CanManageSorafsPopRegistry, CanOperateSorafsPopIssuer, CanRegisterSorafsProviderOwner,
        CanRetireSorafsPin, CanSetSorafsPricing, CanUnregisterSorafsProviderOwner,
        CanUpsertSorafsProviderCredit,
    };

    use super::*;
    use iroha_smart_contract::data_model::query::sorafs::prelude::{
        FindSorafsModerationAppeal, FindSorafsModerationCase, FindSorafsModerationChallenge,
        FindSorafsModerationCommit, FindSorafsModerationJurorEligibility,
        FindSorafsModerationNoShow, FindSorafsModerationOutcome, FindSorafsModerationPolicy,
        FindSorafsModerationReveal, FindSorafsModerationStatus,
        FindSorafsOrderbookCancellationByOrderId, FindSorafsOrderbookOrderById,
        FindSorafsOrderbookOrders, FindSorafsOrderbookPolicy, FindSorafsOrderbookReceiptById,
        FindSorafsOrderbookReceipts, FindSorafsOrderbookStatus, FindSorafsPopAuditDigestBySequence,
        FindSorafsPopCommitmentRootByVersion, FindSorafsPopCredentialCommitmentByDigest,
        FindSorafsPopIssuerPolicy, FindSorafsPopRegistryStatus,
        FindSorafsPopRevocationByNonceCommitment, FindSorafsPopRevocationPublicationByVersion,
    };

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

    /// Validate permission to read the active authoritative orderbook policy.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "the generated Visit dispatch ABI passes every query operation by shared reference"
    )]
    pub fn visit_find_sorafs_orderbook_policy<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        _query: &FindSorafsOrderbookPolicy,
    ) {
        visit_orderbook_read(executor);
    }

    /// Validate permission to read an authoritative order.
    pub fn visit_find_sorafs_orderbook_order_by_id<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        _query: &FindSorafsOrderbookOrderById,
    ) {
        visit_orderbook_read(executor);
    }

    /// Validate permission to read an authoritative cancellation.
    pub fn visit_find_sorafs_orderbook_cancellation_by_order_id<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        _query: &FindSorafsOrderbookCancellationByOrderId,
    ) {
        visit_orderbook_read(executor);
    }

    /// Validate permission to read an authoritative settlement receipt.
    pub fn visit_find_sorafs_orderbook_receipt_by_id<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        _query: &FindSorafsOrderbookReceiptById,
    ) {
        visit_orderbook_read(executor);
    }

    /// Validate permission to read authoritative orderbook counters.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "the generated Visit dispatch ABI passes every query operation by shared reference"
    )]
    pub fn visit_find_sorafs_orderbook_status<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        _query: &FindSorafsOrderbookStatus,
    ) {
        visit_orderbook_read(executor);
    }

    /// Validate permission to list authoritative orders.
    pub fn visit_find_sorafs_orderbook_orders<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        _query: &FindSorafsOrderbookOrders,
    ) {
        visit_orderbook_read(executor);
    }

    /// Validate permission to list authoritative settlement receipts.
    pub fn visit_find_sorafs_orderbook_receipts<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        _query: &FindSorafsOrderbookReceipts,
    ) {
        visit_orderbook_read(executor);
    }

    /// `PoP` issuer policy is public transparency state.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "the generated Visit dispatch ABI passes every query operation by shared reference"
    )]
    pub fn visit_find_sorafs_pop_issuer_policy<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsPopIssuerPolicy,
    ) {
    }

    /// Payload-free credential commitments are public transparency state.
    pub fn visit_find_sorafs_pop_credential_commitment_by_digest<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsPopCredentialCommitmentByDigest,
    ) {
    }

    /// Signed commitment-root publications are public transparency state.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "the generated Visit dispatch ABI passes every query operation by shared reference"
    )]
    pub fn visit_find_sorafs_pop_commitment_root_by_version<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsPopCommitmentRootByVersion,
    ) {
    }

    /// Signed revocation publications are public transparency state.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "the generated Visit dispatch ABI passes every query operation by shared reference"
    )]
    pub fn visit_find_sorafs_pop_revocation_publication_by_version<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsPopRevocationPublicationByVersion,
    ) {
    }

    /// Payload-free revocation commitments are public transparency state.
    pub fn visit_find_sorafs_pop_revocation_by_nonce_commitment<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsPopRevocationByNonceCommitment,
    ) {
    }

    /// Registry audit links are public transparency state.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "the generated Visit dispatch ABI passes every query operation by shared reference"
    )]
    pub fn visit_find_sorafs_pop_audit_digest_by_sequence<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsPopAuditDigestBySequence,
    ) {
    }

    /// Registry anchors and counters are public transparency state.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "the generated Visit dispatch ABI passes every query operation by shared reference"
    )]
    pub fn visit_find_sorafs_pop_registry_status<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsPopRegistryStatus,
    ) {
    }

    /// Authoritative moderation policy is public transparency state.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "the generated Visit dispatch ABI passes every query operation by shared reference"
    )]
    pub fn visit_find_sorafs_moderation_policy<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsModerationPolicy,
    ) {
    }

    /// Appeal intake, pinned roots, and deterministic roster are public transparency state.
    pub fn visit_find_sorafs_moderation_appeal<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsModerationAppeal,
    ) {
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

    /// Authoritative moderation case headers are public transparency state.
    pub fn visit_find_sorafs_moderation_case<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsModerationCase,
    ) {
    }

    /// Sealed commitment digests and provenance are public transparency state.
    pub fn visit_find_sorafs_moderation_commit<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsModerationCommit,
    ) {
    }

    /// Accepted reveals are public after their commit-bound submission.
    pub fn visit_find_sorafs_moderation_reveal<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsModerationReveal,
    ) {
    }

    /// Payload-free challenge records are public transparency state.
    pub fn visit_find_sorafs_moderation_challenge<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsModerationChallenge,
    ) {
    }

    /// Terminal moderation outcomes are public transparency state.
    pub fn visit_find_sorafs_moderation_outcome<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsModerationOutcome,
    ) {
    }

    /// Derived no-show penalty records are public transparency state.
    pub fn visit_find_sorafs_moderation_no_show<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsModerationNoShow,
    ) {
    }

    /// Authoritative moderation counters are public transparency state.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "the generated Visit dispatch ABI passes every query operation by shared reference"
    )]
    pub fn visit_find_sorafs_moderation_status<V: Execute + Visit + ?Sized>(
        _executor: &mut V,
        _query: &FindSorafsModerationStatus,
    ) {
    }

    /// Register a `SoraFS` pin manifest.
    ///
    /// Public submissions rely on the universal-lane Nexus fee schedule instead
    /// of an additional executor permission gate.
    pub fn visit_register_pin_manifest<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterPinManifest,
    ) {
        execute!(executor, isi);
    }

    /// Approve a pending `SoraFS` pin manifest when permitted.
    pub fn visit_approve_pin_manifest<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ApprovePinManifest,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanApproveSorafsPin.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't approve SoraFS pin manifest");
    }

    /// Retire a `SoraFS` pin manifest when permitted.
    pub fn visit_retire_pin_manifest<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RetirePinManifest,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanRetireSorafsPin.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't retire SoraFS pin manifest");
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

    /// Register a capacity declaration when permitted.
    pub fn visit_register_capacity_declaration<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterCapacityDeclaration,
    ) {
        execute!(executor, isi);
    }

    /// Record a capacity telemetry snapshot when permitted.
    pub fn visit_record_capacity_telemetry<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RecordCapacityTelemetry,
    ) {
        execute!(executor, isi);
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

    /// Register or update the owner binding for a `SoraFS` provider when permitted.
    pub fn visit_register_provider_owner<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterProviderOwner,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanRegisterSorafsProviderOwner
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(executor, "Can't register SoraFS provider owner binding");
    }

    /// Remove the owner binding for a `SoraFS` provider when permitted.
    pub fn visit_unregister_provider_owner<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &UnregisterProviderOwner,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanUnregisterSorafsProviderOwner
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(executor, "Can't unregister SoraFS provider owner binding");
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

    /// Submit a signed order; native execution enforces owner and signer binding.
    pub fn visit_submit_orderbook_order<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SubmitSorafsOrderbookOrder,
    ) {
        execute!(executor, isi);
    }

    /// Cancel an order; native execution enforces owner and signer binding.
    pub fn visit_cancel_orderbook_order<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &CancelSorafsOrderbookOrder,
    ) {
        execute!(executor, isi);
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

    /// Submit an authority-bound appeal intake; native execution checks appellant identity.
    pub fn visit_submit_moderation_appeal<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SubmitSorafsModerationAppeal,
    ) {
        execute!(executor, isi);
    }

    /// Register an authority-bound private `PoP` eligibility proof.
    pub fn visit_register_moderation_juror_eligibility<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterSorafsModerationJurorEligibility,
    ) {
        execute!(executor, isi);
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

    /// Accept an authority-bound primary juror assignment.
    pub fn visit_accept_moderation_juror_assignment<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &AcceptSorafsModerationJurorAssignment,
    ) {
        execute!(executor, isi);
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

    /// Submit a juror commitment; native execution binds it to the authority.
    pub fn visit_submit_moderation_commit<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SubmitSorafsModerationCommit,
    ) {
        execute!(executor, isi);
    }

    /// Raise an authenticated payload-free moderation challenge.
    pub fn visit_raise_moderation_challenge<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RaiseSorafsModerationChallenge,
    ) {
        execute!(executor, isi);
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

    /// Submit a juror reveal; native execution verifies the stored commitment.
    pub fn visit_submit_moderation_reveal<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SubmitSorafsModerationReveal,
    ) {
        execute!(executor, isi);
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
    use iroha_executor_data_model::permission::domain::{
        CanModifyDomainMetadata, CanRegisterDomain, CanUnregisterDomain,
    };
    use iroha_smart_contract::data_model::{asset::AssetDefinitionId, domain::DomainId};

    use super::*;
    use crate::permission::{
        account::is_account_owner, domain::is_domain_owner, revoke_permissions,
    };

    /// Registers a domain when genesis or a caller with the register-domain permission requests it.
    pub fn visit_register_domain<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Register<Domain>,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanRegisterDomain.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't register domain");
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
            let err = revoke_permissions(executor, |permission| {
                is_permission_domain_associated(permission, domain_id)
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

    #[allow(clippy::too_many_lines)]
    pub(crate) fn is_permission_domain_associated(
        permission: &Permission,
        domain_id: &DomainId,
    ) -> bool {
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        let asset_definition_matches_domain =
            |definition: &AssetDefinitionId| definition.try_domain() == Some(domain_id);
        match permission {
            AnyPermission::CanUnregisterDomain(permission) => &permission.domain == domain_id,
            AnyPermission::CanModifyDomainMetadata(permission) => &permission.domain == domain_id,
            AnyPermission::CanRegisterAccount(permission) => &permission.domain == domain_id,
            AnyPermission::CanResolveAccountAlias(permission) => {
                matches!(
                    permission.scope,
                    iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Domain(ref domain)
                        if domain == domain_id
                )
            }
            AnyPermission::CanManageAccountAlias(permission) => {
                matches!(
                    permission.scope,
                    iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Domain(ref domain)
                        if domain == domain_id
                )
            }
            AnyPermission::CanUnregisterAssetDefinition(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanModifyAssetDefinitionMetadata(permission) => {
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
            AnyPermission::CanMintAsset(permission) => {
                asset_definition_matches_domain(permission.asset.definition())
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
            AnyPermission::CanSetAssetTransferFreeze(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanSetAssetTransferDailyLimit(permission) => {
                asset_definition_matches_domain(&permission.asset_definition)
            }
            AnyPermission::CanManageZkAceIdentityForAccount(permission) => {
                asset_definition_matches_domain(&permission.asset)
            }
            AnyPermission::CanRegisterNft(permission) => &permission.domain == domain_id,
            AnyPermission::CanUnregisterNft(permission) => permission.nft.domain() == domain_id,
            AnyPermission::CanTransferNft(permission) => permission.nft.domain() == domain_id,
            AnyPermission::CanModifyNftMetadata(permission) => permission.nft.domain() == domain_id,
            AnyPermission::CanUseFeeSponsor(_)
            | AnyPermission::CanUnregisterAccount(_)
            | AnyPermission::CanModifyAccountMetadata(_)
            | AnyPermission::CanReplaceAccountController(_)
            | AnyPermission::CanRegisterTrigger(_)
            | AnyPermission::CanUnregisterTrigger(_)
            | AnyPermission::CanExecuteTrigger(_)
            | AnyPermission::CanModifyTrigger(_)
            | AnyPermission::CanModifyTriggerMetadata(_)
            | AnyPermission::CanManagePeers(_)
            | AnyPermission::CanManageLaneRelayEmergency(_)
            | AnyPermission::CanRegisterDomain(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanManageSccpGovernance(_)
            | AnyPermission::CanProposeSccpRouteGovernance(_)
            | AnyPermission::CanManageRoles(_)
            | AnyPermission::CanUpgradeExecutor(_)
            | AnyPermission::CanRegisterSmartContractCode(_)
            | AnyPermission::CanInvokeContractEntrypoint(_)
            | AnyPermission::CanManageFxCorridors(_)
            | AnyPermission::CanSetFxCorridorPolicy(_)
            | AnyPermission::CanSettleFxCorridor(_)
            | AnyPermission::CanRegisterSorafsPin(_)
            | AnyPermission::CanApproveSorafsPin(_)
            | AnyPermission::CanRetireSorafsPin(_)
            | AnyPermission::CanBindSorafsAlias(_)
            | AnyPermission::CanDeclareSorafsCapacity(_)
            | AnyPermission::CanSubmitSorafsTelemetry(_)
            | AnyPermission::CanFileSorafsCapacityDispute(_)
            | AnyPermission::CanIssueSorafsReplicationOrder(_)
            | AnyPermission::CanCompleteSorafsReplicationOrder(_)
            | AnyPermission::CanSetSorafsPricing(_)
            | AnyPermission::CanManageSorafsModeration(_)
            | AnyPermission::CanManageSorafsPopRegistry(_)
            | AnyPermission::CanOperateSorafsPopIssuer(_)
            | AnyPermission::CanUpsertSorafsProviderCredit(_)
            | AnyPermission::CanRegisterSorafsProviderOwner(_)
            | AnyPermission::CanUnregisterSorafsProviderOwner(_)
            | AnyPermission::CanIngestSoranetPrivacy(_)
            | AnyPermission::CanRegisterOracleFeed(_)
            | AnyPermission::CanProposeOracleChange(_)
            | AnyPermission::CanVoteOracleChangeStage(_)
            | AnyPermission::CanRollbackOracleChange(_)
            | AnyPermission::CanResolveOracleDispute(_)
            | AnyPermission::CanManageTwitterBindings(_)
            | AnyPermission::CanPublishSpaceDirectoryManifest(_) => false,
        }
    }
}

/// Permission-checked visitors for account management instructions.
pub mod account {
    use iroha_executor_data_model::permission::account::{
        CanModifyAccountMetadata, CanReplaceAccountController, CanUnregisterAccount,
    };

    use super::*;
    use crate::permission::{account::is_account_owner, revoke_permissions};

    /// Registers a canonical account.
    pub fn visit_register_account<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Register<Account>,
    ) {
        execute!(executor, isi);
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

    /// Delegates proposal authorisation to the core recovery state machine.
    pub fn visit_propose_account_recovery<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ProposeAccountRecovery,
    ) {
        execute!(executor, isi);
    }

    /// Delegates approval authorisation to the core recovery state machine.
    pub fn visit_approve_account_recovery<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ApproveAccountRecovery,
    ) {
        execute!(executor, isi);
    }

    /// Delegates cancellation authorisation to the core recovery state machine.
    pub fn visit_cancel_account_recovery<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &CancelAccountRecovery,
    ) {
        execute!(executor, isi);
    }

    /// Delegates finalization authorisation to the core recovery state machine.
    pub fn visit_finalize_account_recovery<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &FinalizeAccountRecovery,
    ) {
        execute!(executor, isi);
    }

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
            AnyPermission::CanMintAsset(permission) => permission.asset.account() == account_id,
            AnyPermission::CanBurnAsset(permission) => permission.asset.account() == account_id,
            AnyPermission::CanTransferAsset(permission) => permission.asset.account() == account_id,
            AnyPermission::CanModifyAssetMetadata(permission) => {
                permission.asset.account() == account_id
            }
            AnyPermission::CanManageZkAceIdentityForAccount(permission) => {
                permission.account == *account_id
            }
            AnyPermission::CanUseFeeSponsor(permission) => permission.sponsor == *account_id,
            AnyPermission::CanInvokeContractEntrypoint(permission) => {
                permission.contract.subject_id() == *account_id
            }
            AnyPermission::CanRegisterTrigger(permission) => permission.authority == *account_id,
            AnyPermission::CanUnregisterTrigger(_)
            | AnyPermission::CanExecuteTrigger(_)
            | AnyPermission::CanModifyTrigger(_)
            | AnyPermission::CanModifyTriggerMetadata(_)
            | AnyPermission::CanResolveAccountAlias(_)
            | AnyPermission::CanManageAccountAlias(_)
            | AnyPermission::CanManagePeers(_)
            | AnyPermission::CanManageLaneRelayEmergency(_)
            | AnyPermission::CanRegisterDomain(_)
            | AnyPermission::CanUnregisterDomain(_)
            | AnyPermission::CanModifyDomainMetadata(_)
            | AnyPermission::CanRegisterAccount(_)
            | AnyPermission::CanUnregisterAssetDefinition(_)
            | AnyPermission::CanModifyAssetDefinitionMetadata(_)
            | AnyPermission::CanMintAssetWithDefinition(_)
            | AnyPermission::CanBurnAssetWithDefinition(_)
            | AnyPermission::CanTransferAssetWithDefinition(_)
            | AnyPermission::CanModifyAssetMetadataWithDefinition(_)
            | AnyPermission::CanSetAssetTransferFreeze(_)
            | AnyPermission::CanSetAssetTransferDailyLimit(_)
            | AnyPermission::CanRegisterNft(_)
            | AnyPermission::CanUnregisterNft(_)
            | AnyPermission::CanTransferNft(_)
            | AnyPermission::CanModifyNftMetadata(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanManageSccpGovernance(_)
            | AnyPermission::CanProposeSccpRouteGovernance(_)
            | AnyPermission::CanManageRoles(_)
            | AnyPermission::CanUpgradeExecutor(_)
            | AnyPermission::CanRegisterSmartContractCode(_)
            | AnyPermission::CanManageFxCorridors(_)
            | AnyPermission::CanSetFxCorridorPolicy(_)
            | AnyPermission::CanSettleFxCorridor(_)
            | AnyPermission::CanRegisterSorafsPin(_)
            | AnyPermission::CanApproveSorafsPin(_)
            | AnyPermission::CanRetireSorafsPin(_)
            | AnyPermission::CanBindSorafsAlias(_)
            | AnyPermission::CanDeclareSorafsCapacity(_)
            | AnyPermission::CanSubmitSorafsTelemetry(_)
            | AnyPermission::CanFileSorafsCapacityDispute(_)
            | AnyPermission::CanIssueSorafsReplicationOrder(_)
            | AnyPermission::CanCompleteSorafsReplicationOrder(_)
            | AnyPermission::CanSetSorafsPricing(_)
            | AnyPermission::CanManageSorafsModeration(_)
            | AnyPermission::CanManageSorafsPopRegistry(_)
            | AnyPermission::CanOperateSorafsPopIssuer(_)
            | AnyPermission::CanUpsertSorafsProviderCredit(_)
            | AnyPermission::CanRegisterSorafsProviderOwner(_)
            | AnyPermission::CanUnregisterSorafsProviderOwner(_)
            | AnyPermission::CanIngestSoranetPrivacy(_)
            | AnyPermission::CanRegisterOracleFeed(_)
            | AnyPermission::CanProposeOracleChange(_)
            | AnyPermission::CanVoteOracleChangeStage(_)
            | AnyPermission::CanRollbackOracleChange(_)
            | AnyPermission::CanResolveOracleDispute(_)
            | AnyPermission::CanManageTwitterBindings(_)
            | AnyPermission::CanPublishSpaceDirectoryManifest(_) => false,
        }
    }
}

/// Permission-checked visitors for asset definition instructions.
pub mod asset_definition {
    use iroha_executor_data_model::permission::asset_definition::{
        CanModifyAssetDefinitionMetadata, CanUnregisterAssetDefinition,
    };
    use iroha_smart_contract::data_model::asset::AssetDefinitionId;

    use super::*;
    use crate::permission::{
        account::is_account_owner, asset_definition::is_asset_definition_owner, revoke_permissions,
    };

    /// Registers an asset definition.
    pub fn visit_register_asset_definition<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Register<AssetDefinition>,
    ) {
        execute!(executor, isi);
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
            AnyPermission::CanMintAsset(permission) => {
                permission.asset.definition() == asset_definition_id
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
            AnyPermission::CanManageZkAceIdentityForAccount(permission) => {
                &permission.asset == asset_definition_id
            }
            AnyPermission::CanSetAssetTransferFreeze(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanSetAssetTransferDailyLimit(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanUnregisterAccount(_)
            | AnyPermission::CanModifyAccountMetadata(_)
            | AnyPermission::CanReplaceAccountController(_)
            | AnyPermission::CanResolveAccountAlias(_)
            | AnyPermission::CanManageAccountAlias(_)
            | AnyPermission::CanRegisterTrigger(_)
            | AnyPermission::CanUnregisterTrigger(_)
            | AnyPermission::CanExecuteTrigger(_)
            | AnyPermission::CanModifyTrigger(_)
            | AnyPermission::CanModifyTriggerMetadata(_)
            | AnyPermission::CanManagePeers(_)
            | AnyPermission::CanManageLaneRelayEmergency(_)
            | AnyPermission::CanRegisterDomain(_)
            | AnyPermission::CanUnregisterDomain(_)
            | AnyPermission::CanModifyDomainMetadata(_)
            | AnyPermission::CanRegisterAccount(_)
            | AnyPermission::CanRegisterNft(_)
            | AnyPermission::CanUnregisterNft(_)
            | AnyPermission::CanTransferNft(_)
            | AnyPermission::CanModifyNftMetadata(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanManageSccpGovernance(_)
            | AnyPermission::CanProposeSccpRouteGovernance(_)
            | AnyPermission::CanManageRoles(_)
            | AnyPermission::CanUpgradeExecutor(_)
            | AnyPermission::CanRegisterSmartContractCode(_)
            | AnyPermission::CanInvokeContractEntrypoint(_)
            | AnyPermission::CanManageFxCorridors(_)
            | AnyPermission::CanSetFxCorridorPolicy(_)
            | AnyPermission::CanSettleFxCorridor(_)
            | AnyPermission::CanRegisterSorafsPin(_)
            | AnyPermission::CanApproveSorafsPin(_)
            | AnyPermission::CanRetireSorafsPin(_)
            | AnyPermission::CanBindSorafsAlias(_)
            | AnyPermission::CanDeclareSorafsCapacity(_)
            | AnyPermission::CanSubmitSorafsTelemetry(_)
            | AnyPermission::CanFileSorafsCapacityDispute(_)
            | AnyPermission::CanIssueSorafsReplicationOrder(_)
            | AnyPermission::CanCompleteSorafsReplicationOrder(_)
            | AnyPermission::CanSetSorafsPricing(_)
            | AnyPermission::CanManageSorafsModeration(_)
            | AnyPermission::CanManageSorafsPopRegistry(_)
            | AnyPermission::CanOperateSorafsPopIssuer(_)
            | AnyPermission::CanUpsertSorafsProviderCredit(_)
            | AnyPermission::CanRegisterSorafsProviderOwner(_)
            | AnyPermission::CanUnregisterSorafsProviderOwner(_)
            | AnyPermission::CanIngestSoranetPrivacy(_)
            | AnyPermission::CanRegisterOracleFeed(_)
            | AnyPermission::CanProposeOracleChange(_)
            | AnyPermission::CanVoteOracleChangeStage(_)
            | AnyPermission::CanRollbackOracleChange(_)
            | AnyPermission::CanResolveOracleDispute(_)
            | AnyPermission::CanManageTwitterBindings(_)
            | AnyPermission::CanPublishSpaceDirectoryManifest(_)
            | AnyPermission::CanUseFeeSponsor(_) => false,
        }
    }
}

/// Permission-checked visitors for asset operations.
pub mod asset {
    use iroha_executor_data_model::permission::asset::{
        CanBurnAsset, CanBurnAssetWithDefinition, CanMintAsset, CanMintAssetWithDefinition,
        CanModifyAssetMetadata, CanModifyAssetMetadataWithDefinition,
        CanSetAssetTransferDailyLimit, CanSetAssetTransferFreeze, CanTransferAsset,
        CanTransferAssetWithDefinition,
    };
    use iroha_smart_contract::data_model::isi::{
        BuiltInInstruction, RemoveAssetKeyValue, SetAssetKeyValue,
    };
    use norito::NoritoSerialize;

    use super::*;
    use crate::permission::{asset::is_asset_owner, asset_definition::is_asset_definition_owner};

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

    /// Sets an account transfer freeze when genesis or the asset-definition owner invokes it.
    pub fn visit_set_asset_transfer_freeze<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetAssetTransferFreeze,
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

        let (account_domain, account_dataspace) =
            match target_account_scope(executor, isi.account_id()) {
                Ok(scope) => scope,
                Err(err) => deny!(executor, ValidationFail::NotPermitted(err)),
            };
        let permission = CanSetAssetTransferFreeze {
            asset_definition: isi.asset_definition_id().clone(),
            account_domain,
            account_dataspace,
        };
        if permission.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "transfer freeze requires an asset- and account-domain-scoped permission"
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

    fn execute_mint_asset<V, Q>(executor: &mut V, isi: &Mint<Q, Asset>)
    where
        V: Execute + Visit + ?Sized,
        Q: Into<Numeric>,
        Mint<Q, Asset>: BuiltInInstruction + NoritoSerialize,
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
        let can_mint_user_asset_token = CanMintAsset {
            asset: asset_id.clone(),
        };
        if can_mint_user_asset_token.is_owned_by(&executor.context().authority, executor.host()) {
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

    fn execute_burn_asset<V, Q>(executor: &mut V, isi: &Burn<Q, Asset>)
    where
        V: Execute + Visit + ?Sized,
        Q: Into<Numeric>,
        Burn<Q, Asset>: BuiltInInstruction + NoritoSerialize,
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
        match is_asset_definition_owner(
            asset_id.definition(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
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
        use core::num::NonZeroU64;

        use iroha_crypto::{Algorithm, KeyPair};

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
                prelude::{Json, Numeric, Quantity},
                repo::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance},
            },
            prelude::{Context, Visit},
        };

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
                let asset_definition = AssetDefinitionId::new(
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
                PeerId::from(validator.signatory().clone()),
                validator,
                Numeric::from(1u64),
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
                amount: 0,
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
            let domain: DomainId =
                DomainId::try_new("wonderland", "universal").expect("valid domain");
            let counterparty_keypair = fixture_key_pair(9);
            let counterparty = AccountId::new(counterparty_keypair.public_key().clone());
            let cash_def =
                AssetDefinitionId::new(domain.clone(), "cash".parse::<Name>().expect("valid name"));
            let collateral_def =
                AssetDefinitionId::new(domain, "collateral".parse::<Name>().expect("valid name"));
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
    }
}

/// Permission-checked visitors for non-fungible asset instructions.
pub mod nft {
    use iroha_executor_data_model::permission::nft::{
        CanModifyNftMetadata, CanRegisterNft, CanTransferNft, CanUnregisterNft,
    };
    use norito::NoritoSerialize;

    use super::*;
    use crate::{
        data_model::isi::BuiltInInstruction,
        permission::{
            account::is_account_owner,
            nft::{is_nft_full_owner, is_nft_weak_owner},
            revoke_permissions,
        },
    };

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
    use iroha_executor_data_model::permission::parameter::CanSetParameters;

    use super::*;

    const SCCP_REGISTRY_PARAMETER_ID: &str = "sccp_registry_v1";

    fn updates_sccp_governance(isi: &SetParameter) -> bool {
        matches!(
            isi.inner(),
            Parameter::Custom(parameter)
                if parameter.id().name().as_ref() == SCCP_REGISTRY_PARAMETER_ID
        )
    }

    /// Applies a network parameter change when genesis or a parameter manager invokes it.
    pub fn visit_set_parameter<V: Execute + Visit + ?Sized>(executor: &mut V, isi: &SetParameter) {
        if updates_sccp_governance(isi) {
            deny!(
                executor,
                "The reserved SCCP registry cannot be changed through SetParameter; use ApplySccpRouteGovernance"
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
    use iroha_executor_data_model::permission::role::CanManageRoles;
    use iroha_smart_contract::{Iroha, data_model::role::Role};

    use super::*;

    macro_rules! impl_execute_grant_revoke_account_role {
        ($executor:ident, $isi:ident) => {
            let role_id = $isi.object();

            if $executor.context().curr_block.is_genesis()
                || find_account_roles($executor.context().authority.clone(), $executor.host())
                    .any(|authority_role_id| authority_role_id == *role_id)
            {
                execute!($executor, $isi)
            }

            deny!($executor, "Can't grant or revoke role to another account");
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

    /// Registers a role and seeds its permissions when the caller controls role governance.
    pub fn visit_register_role<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Register<Role>,
    ) {
        let role = isi.object();
        let grant_role = &Grant::account_role(role.id().clone(), role.grant_to().clone());
        let mut new_role = Role::new(role.id().clone(), role.grant_to().clone());

        // Exception for multisig roles
        {
            use crate::permission::domain::is_domain_owner;

            const DELIMITER: char = '/';
            const MULTISIG_SIGNATORY: &str = "MULTISIG_SIGNATORY";

            fn multisig_home_domain_from(role: &RoleId) -> Option<DomainId> {
                role.name()
                    .as_ref()
                    .strip_prefix(&format!("{MULTISIG_SIGNATORY}{DELIMITER}"))?
                    .split_once(DELIMITER)
                    .and_then(|(domain, _)| DomainId::parse_fully_qualified(domain).ok())
            }

            if role.id().name().as_ref().starts_with(MULTISIG_SIGNATORY) {
                let Some(home_domain) = multisig_home_domain_from(role.id()) else {
                    deny!(executor, "violates multisig role name format")
                };
                if is_domain_owner(&home_domain, &executor.context().authority, executor.host())
                    .unwrap_or_default()
                {
                    deny!(
                        executor,
                        "reserved multisig role names may not be registered"
                    );
                }
                deny!(
                    executor,
                    "only the domain owner can register multisig roles"
                )
            }
        }

        for permission in role.inner().permissions() {
            iroha_smart_contract::log::debug!(&format!("Checking `{permission:?}`"));

            let Ok(any_permission) = AnyPermission::try_from(permission) else {
                deny!(
                    executor,
                    ValidationFail::NotPermitted(format!("{permission:?}: Unknown permission"))
                );
            };
            if !executor.context().curr_block.is_genesis()
                && let Err(error) = crate::permission::ValidateGrantRevoke::validate_grant(
                    &any_permission,
                    role.grant_to(),
                    executor.context(),
                    executor.host(),
                )
            {
                deny!(executor, error);
            }
            new_role = new_role.add_permission(any_permission);
        }

        if executor.context().curr_block.is_genesis()
            || CanManageRoles.is_owned_by(&executor.context().authority, executor.host())
        {
            let isi = &Register::role(new_role);
            if let Err(err) = executor.host().submit(isi) {
                deny!(executor, err);
            }

            execute!(executor, grant_role);
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
        impl_execute_grant_revoke_account_role!(executor, isi);
    }

    /// Revokes a role from an account after verifying role management permissions.
    pub fn visit_revoke_account_role<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Revoke<RoleId, Account>,
    ) {
        impl_execute_grant_revoke_account_role!(executor, isi);
    }

    /// Grants a permission to a role after ensuring the caller may mutate role permissions.
    pub fn visit_grant_role_permission<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Grant<Permission, Role>,
    ) {
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
    use iroha_executor_data_model::permission::trigger::{
        CanExecuteTrigger, CanModifyTrigger, CanModifyTriggerMetadata, CanRegisterTrigger,
        CanUnregisterTrigger,
    };
    use iroha_smart_contract::data_model::trigger::Trigger;

    use super::*;
    use crate::permission::{revoke_permissions, trigger::is_trigger_owner};

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
            AnyPermission::CanRegisterTrigger(_)
            | AnyPermission::CanManagePeers(_)
            | AnyPermission::CanManageLaneRelayEmergency(_)
            | AnyPermission::CanRegisterDomain(_)
            | AnyPermission::CanUnregisterDomain(_)
            | AnyPermission::CanModifyDomainMetadata(_)
            | AnyPermission::CanRegisterAccount(_)
            | AnyPermission::CanUnregisterAccount(_)
            | AnyPermission::CanModifyAccountMetadata(_)
            | AnyPermission::CanReplaceAccountController(_)
            | AnyPermission::CanResolveAccountAlias(_)
            | AnyPermission::CanManageAccountAlias(_)
            | AnyPermission::CanUnregisterAssetDefinition(_)
            | AnyPermission::CanModifyAssetDefinitionMetadata(_)
            | AnyPermission::CanModifyAssetMetadataWithDefinition(_)
            | AnyPermission::CanMintAssetWithDefinition(_)
            | AnyPermission::CanBurnAssetWithDefinition(_)
            | AnyPermission::CanTransferAssetWithDefinition(_)
            | AnyPermission::CanMintAsset(_)
            | AnyPermission::CanBurnAsset(_)
            | AnyPermission::CanModifyAssetMetadata(_)
            | AnyPermission::CanTransferAsset(_)
            | AnyPermission::CanSetAssetTransferFreeze(_)
            | AnyPermission::CanSetAssetTransferDailyLimit(_)
            | AnyPermission::CanManageZkAceIdentityForAccount(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanManageSccpGovernance(_)
            | AnyPermission::CanProposeSccpRouteGovernance(_)
            | AnyPermission::CanManageRoles(_)
            | AnyPermission::CanRegisterNft(_)
            | AnyPermission::CanUnregisterNft(_)
            | AnyPermission::CanTransferNft(_)
            | AnyPermission::CanModifyNftMetadata(_)
            | AnyPermission::CanUpgradeExecutor(_)
            | AnyPermission::CanRegisterSmartContractCode(_)
            | AnyPermission::CanInvokeContractEntrypoint(_)
            | AnyPermission::CanManageFxCorridors(_)
            | AnyPermission::CanSetFxCorridorPolicy(_)
            | AnyPermission::CanSettleFxCorridor(_)
            | AnyPermission::CanRegisterSorafsPin(_)
            | AnyPermission::CanApproveSorafsPin(_)
            | AnyPermission::CanRetireSorafsPin(_)
            | AnyPermission::CanBindSorafsAlias(_)
            | AnyPermission::CanDeclareSorafsCapacity(_)
            | AnyPermission::CanSubmitSorafsTelemetry(_)
            | AnyPermission::CanFileSorafsCapacityDispute(_)
            | AnyPermission::CanIssueSorafsReplicationOrder(_)
            | AnyPermission::CanCompleteSorafsReplicationOrder(_)
            | AnyPermission::CanSetSorafsPricing(_)
            | AnyPermission::CanManageSorafsModeration(_)
            | AnyPermission::CanManageSorafsPopRegistry(_)
            | AnyPermission::CanOperateSorafsPopIssuer(_)
            | AnyPermission::CanUpsertSorafsProviderCredit(_)
            | AnyPermission::CanRegisterSorafsProviderOwner(_)
            | AnyPermission::CanUnregisterSorafsProviderOwner(_)
            | AnyPermission::CanIngestSoranetPrivacy(_)
            | AnyPermission::CanRegisterOracleFeed(_)
            | AnyPermission::CanProposeOracleChange(_)
            | AnyPermission::CanVoteOracleChangeStage(_)
            | AnyPermission::CanRollbackOracleChange(_)
            | AnyPermission::CanResolveOracleDispute(_)
            | AnyPermission::CanManageTwitterBindings(_)
            | AnyPermission::CanPublishSpaceDirectoryManifest(_)
            | AnyPermission::CanUseFeeSponsor(_) => false,
        }
    }

    #[cfg(test)]
    mod tests {
        use core::str::FromStr as _;

        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_executor_data_model::permission::{
            account::{AccountAliasPermissionScope, CanManageAccountAlias, CanResolveAccountAlias},
            asset::{CanModifyAssetMetadata, CanModifyAssetMetadataWithDefinition},
            nexus::CanUseFeeSponsor,
            sccp::CanManageSccpGovernance,
            sorafs::{
                CanApproveSorafsPin, CanBindSorafsAlias, CanCompleteSorafsReplicationOrder,
                CanDeclareSorafsCapacity, CanFileSorafsCapacityDispute,
                CanIssueSorafsReplicationOrder, CanManageSorafsModeration,
                CanManageSorafsPopRegistry, CanOperateSorafsPopIssuer, CanRegisterSorafsPin,
                CanRegisterSorafsProviderOwner, CanRetireSorafsPin, CanSetSorafsPricing,
                CanSubmitSorafsTelemetry, CanUnregisterSorafsProviderOwner,
                CanUpsertSorafsProviderCredit,
            },
            soranet::CanIngestSoranetPrivacy,
        };

        use super::*;
        use crate::data_model::{
            account::AccountId,
            asset::{AssetDefinitionId, AssetId},
            domain::DomainId,
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
                AnyPermission::CanRegisterSorafsPin(CanRegisterSorafsPin),
                AnyPermission::CanApproveSorafsPin(CanApproveSorafsPin),
                AnyPermission::CanRetireSorafsPin(CanRetireSorafsPin),
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
                AnyPermission::CanUpsertSorafsProviderCredit(CanUpsertSorafsProviderCredit),
                AnyPermission::CanRegisterSorafsProviderOwner(CanRegisterSorafsProviderOwner),
                AnyPermission::CanUnregisterSorafsProviderOwner(CanUnregisterSorafsProviderOwner),
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
            let asset_definition_id = AssetDefinitionId::new(
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
        fn sora_permissions_not_domain_account_or_definition_associated() {
            let domain_id =
                DomainId::try_new("test", "universal").expect("domain id must be valid");
            let account_id = sample_account_id(0x12, &domain_id);
            let asset_definition_id = AssetDefinitionId::new(
                DomainId::try_new("test", "universal").unwrap(),
                "token".parse().unwrap(),
            );

            for permission in sora_permissions() {
                let permission = Permission::from(permission);
                assert!(
                    !domain::is_permission_domain_associated(&permission, &domain_id),
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
            let asset_definition_id = AssetDefinitionId::new(
                DomainId::try_new("test", "universal").unwrap(),
                "token".parse().unwrap(),
            );
            let trigger_id =
                TriggerId::from_str("fee_sponsor_trigger").expect("trigger id must be valid");

            let permission = Permission::from(AnyPermission::CanUseFeeSponsor(CanUseFeeSponsor {
                sponsor: sponsor.clone(),
                policy: "default".parse().expect("fee sponsor policy name is valid"),
            }));

            assert!(
                !domain::is_permission_domain_associated(&permission, &domain_id),
                "fee sponsor permission should not bind to domains"
            );
            assert!(
                !domain::is_permission_domain_associated(&permission, &other_domain),
                "fee sponsor permission should not bind to unrelated domains"
            );
            assert!(
                account::is_permission_account_associated(&permission, &sponsor),
                "fee sponsor permission should bind to sponsor account"
            );
            assert!(
                !account::is_permission_account_associated(&permission, &other_account),
                "fee sponsor permission should not bind to unrelated accounts"
            );
            assert!(
                !asset_definition::is_permission_asset_definition_associated(
                    &permission,
                    &asset_definition_id
                ),
                "fee sponsor permission should not bind to asset definitions"
            );
            assert!(
                !is_permission_trigger_associated(&permission, &trigger_id),
                "fee sponsor permission should not bind to triggers"
            );
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
            let manage_permission = Permission::from(AnyPermission::CanManageAccountAlias(
                CanManageAccountAlias {
                    scope: AccountAliasPermissionScope::Domain(domain_id.clone()),
                },
            ));

            assert!(
                domain::is_permission_domain_associated(&resolve_permission, &domain_id),
                "alias resolve permission should bind to the matching domain"
            );
            assert!(
                !domain::is_permission_domain_associated(&resolve_permission, &other_domain),
                "alias resolve permission should not bind to other domains"
            );
            assert!(
                domain::is_permission_domain_associated(&manage_permission, &domain_id),
                "alias manage permission should bind to the matching domain"
            );
            assert!(
                !domain::is_permission_domain_associated(&manage_permission, &other_domain),
                "alias manage permission should not bind to other domains"
            );
        }
    }
}

#[cfg(test)]
mod sorafs_permission_tests {
    use core::num::NonZeroU64;

    use iroha_crypto::PublicKey;
    use iroha_data_model::{
        account::AccountId,
        block::BlockHeader,
        isi::sorafs::{
            AcceptSorafsModerationJurorAssignment, ActivateSorafsModerationCase,
            ApprovePinManifest, BindManifestAlias, CommitSorafsPopCredentialBatch,
            CompleteReplicationOrder, ExpireReplicationOrder, FinalizeSorafsModerationSortition,
            IssueReplicationOrder, PublishSorafsPopRevocationList, RecordCapacityTelemetry,
            RegisterCapacityDeclaration, RegisterCapacityDispute, RegisterPinManifest,
            RegisterProviderOwner, RegisterSorafsModerationJurorEligibility, RetirePinManifest,
            SetPricingSchedule, SetSorafsModerationPolicy, SetSorafsPopIssuerPolicy,
            SubmitSorafsModerationAppeal, SubmitSorafsModerationCommit, UnregisterProviderOwner,
            UpsertProviderCredit,
        },
        metadata::Metadata,
        permission::Permission as PermissionObject,
        prelude::ValidationFail,
        query::sorafs::prelude::{
            FindSorafsModerationAppeal, FindSorafsModerationJurorEligibility,
            FindSorafsModerationPolicy, FindSorafsModerationStatus,
            FindSorafsOrderbookCancellationByOrderId, FindSorafsOrderbookOrderById,
            FindSorafsOrderbookOrders, FindSorafsOrderbookPolicy, FindSorafsOrderbookReceiptById,
            FindSorafsOrderbookReceipts, FindSorafsOrderbookStatus,
            FindSorafsPopAuditDigestBySequence, FindSorafsPopCommitmentRootByVersion,
            FindSorafsPopCredentialCommitmentByDigest, FindSorafsPopIssuerPolicy,
            FindSorafsPopRegistryStatus, FindSorafsPopRevocationByNonceCommitment,
            FindSorafsPopRevocationPublicationByVersion,
        },
        sorafs::{
            capacity::{
                CapacityDeclarationRecord, CapacityDisputeEvidence, CapacityDisputeId,
                CapacityDisputeRecord, CapacityTelemetryRecord, ProviderId,
            },
            moderation_ledger::{
                MODERATION_APPEAL_INTAKE_VERSION_V1, MODERATION_LEDGER_POLICY_VERSION_V1,
                ModerationAppealIntakeV1, ModerationLedgerPolicyV1,
            },
            pin_registry::{ManifestAliasBinding, ManifestDigest, ReplicationOrderId},
            pop_registry::{POP_ISSUER_POLICY_VERSION_V1, PopIssuerPolicyV1},
            pricing::{PricingScheduleRecord, ProviderCreditRecord},
        },
    };
    use iroha_executor_data_model::permission::sorafs::{
        CanApproveSorafsPin, CanBindSorafsAlias, CanCompleteSorafsReplicationOrder,
        CanFileSorafsCapacityDispute, CanIssueSorafsReplicationOrder, CanManageSorafsModeration,
        CanManageSorafsPopRegistry, CanOperateSorafsPopIssuer, CanRegisterSorafsPin,
        CanRegisterSorafsProviderOwner, CanRetireSorafsPin, CanSetSorafsPricing,
        CanUnregisterSorafsProviderOwner, CanUpsertSorafsProviderCredit,
    };
    use iroha_executor_data_model::permission::{
        parameter::CanSetParameters, sccp::CanManageSccpGovernance,
    };

    use super::*;
    use crate::{Iroha, prelude, tests::with_mock_permissions};

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
            .signatory()
            .try_to_bytes()
            .expect("authority public key bytes");
        bytes.try_into().expect("Ed25519 public key length")
    }

    #[derive(Debug)]
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

    impl Visit for MockExecutor {}

    fn assert_denied_without_permission<T: Clone>(
        instruction: T,
        visit: impl Fn(&mut MockExecutor, &T),
    ) {
        with_mock_permissions(vec![PermissionObject::from(CanRegisterSorafsPin)], || {
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
            1,
            None,
            None,
        )
    }

    fn approve_pin_manifest() -> ApprovePinManifest {
        ApprovePinManifest::new(sample_manifest_digest(), 2, None, None)
    }

    fn retire_pin_manifest() -> RetirePinManifest {
        RetirePinManifest::new(sample_manifest_digest(), 3, None)
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

    fn issue_replication_order() -> IssueReplicationOrder {
        IssueReplicationOrder::new(ReplicationOrderId::new([0x11; 32]), vec![0x22], 1, 2)
    }

    fn complete_replication_order() -> CompleteReplicationOrder {
        CompleteReplicationOrder::new(ReplicationOrderId::new([0x11; 32]), 3)
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
            1,
            0,
            0,
            0,
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
            max_panel_size: 8,
            max_candidate_pool_size: 32,
            max_waitlist_size: 8,
            max_exclusions_per_case: 16,
            max_total_window_ms: 60_000,
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
            challenge_deadline_unix_ms: 4_000,
            reveal_deadline_unix_ms: 5_000,
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

    sorafs_permission_case!(
        approve_pin_manifest_requires_permission,
        approve_pin_manifest(),
        CanApproveSorafsPin,
        sorafs::visit_approve_pin_manifest
    );

    sorafs_permission_case!(
        retire_pin_manifest_requires_permission,
        retire_pin_manifest(),
        CanRetireSorafsPin,
        sorafs::visit_retire_pin_manifest
    );

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
        expire_replication_order_requires_permission,
        expire_replication_order(),
        CanIssueSorafsReplicationOrder,
        sorafs::visit_expire_replication_order
    );

    sorafs_permission_case!(
        register_provider_owner_requires_permission,
        register_provider_owner(),
        CanRegisterSorafsProviderOwner,
        sorafs::visit_register_provider_owner
    );

    sorafs_permission_case!(
        unregister_provider_owner_requires_permission,
        unregister_provider_owner(),
        CanUnregisterSorafsProviderOwner,
        sorafs::visit_unregister_provider_owner
    );

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

    fn custom_parameter(name: &str) -> SetParameter {
        let id = iroha_smart_contract::data_model::parameter::CustomParameterId::new(
            name.parse().expect("test custom parameter id"),
        );
        SetParameter::new(Parameter::Custom(
            iroha_smart_contract::data_model::parameter::CustomParameter::new(id, Json::new(())),
        ))
    }

    fn remove_sccp_route() -> ApplySccpRouteGovernance {
        ApplySccpRouteGovernance::new(
            iroha_smart_contract::data_model::isi::bridge::SccpRouteGovernanceActionV1::Remove(
                iroha_smart_contract::data_model::bridge::SccpRouteKeyV1 {
                    lane_id: iroha_smart_contract::data_model::bridge::SccpLaneIdV1 {
                        source:
                            iroha_smart_contract::data_model::bridge::SccpNetworkV1::EthereumMainnet,
                        target: iroha_smart_contract::data_model::bridge::SccpNetworkV1::SoraTaira,
                    },
                    route_id: "taira_eth_xor".to_owned(),
                    asset_key: "xor".to_owned(),
                    revision: 1,
                },
            ),
        )
    }

    #[test]
    fn sccp_route_governance_dispatch_requires_dedicated_permission() {
        let instruction = remove_sccp_route();
        assert_denied_without_permission(
            instruction.clone(),
            bridge::visit_apply_sccp_route_governance,
        );
        assert_denied_with_permission(
            instruction.clone(),
            PermissionObject::from(CanSetParameters),
            bridge::visit_apply_sccp_route_governance,
        );
        assert_allowed_with_permission(
            instruction.clone(),
            PermissionObject::from(CanManageSccpGovernance),
            bridge::visit_apply_sccp_route_governance,
        );

        with_mock_permissions(
            vec![PermissionObject::from(CanManageSccpGovernance)],
            || {
                let mut executor = MockExecutor::new(false);
                visit_instruction(&mut executor, &InstructionBox::from(instruction));
                assert!(
                    executor.verdict().is_ok(),
                    "known SCCP governance ISI must reach its permission-checked dispatcher"
                );
            },
        );
    }

    #[test]
    fn pop_registry_transparency_queries_are_public() {
        assert_allowed_without_permission(
            FindSorafsPopIssuerPolicy,
            sorafs::visit_find_sorafs_pop_issuer_policy,
        );
        assert_allowed_without_permission(
            FindSorafsPopCredentialCommitmentByDigest::new([1; 32]),
            sorafs::visit_find_sorafs_pop_credential_commitment_by_digest,
        );
        assert_allowed_without_permission(
            FindSorafsPopCommitmentRootByVersion::new(1),
            sorafs::visit_find_sorafs_pop_commitment_root_by_version,
        );
        assert_allowed_without_permission(
            FindSorafsPopRevocationPublicationByVersion::new(1),
            sorafs::visit_find_sorafs_pop_revocation_publication_by_version,
        );
        assert_allowed_without_permission(
            FindSorafsPopRevocationByNonceCommitment::new([2; 32]),
            sorafs::visit_find_sorafs_pop_revocation_by_nonce_commitment,
        );
        assert_allowed_without_permission(
            FindSorafsPopAuditDigestBySequence::new(1),
            sorafs::visit_find_sorafs_pop_audit_digest_by_sequence,
        );
        assert_allowed_without_permission(
            FindSorafsPopRegistryStatus,
            sorafs::visit_find_sorafs_pop_registry_status,
        );
    }

    macro_rules! orderbook_query_permission_case {
        ($name:ident, $query:expr, $visitor:path) => {
            #[test]
            fn $name() {
                let query = $query;
                assert_denied_without_permission(query.clone(), $visitor);
                assert_allowed_with_permission(
                    query.clone(),
                    PermissionObject::from(CanSetSorafsPricing),
                    $visitor,
                );
                assert_allowed_with_permission(
                    query,
                    PermissionObject::from(CanCompleteSorafsReplicationOrder),
                    $visitor,
                );
            }
        };
    }

    orderbook_query_permission_case!(
        orderbook_policy_query_requires_operator_permission,
        FindSorafsOrderbookPolicy,
        sorafs::visit_find_sorafs_orderbook_policy
    );
    orderbook_query_permission_case!(
        orderbook_order_query_requires_operator_permission,
        FindSorafsOrderbookOrderById::new([0x11; 32]),
        sorafs::visit_find_sorafs_orderbook_order_by_id
    );
    orderbook_query_permission_case!(
        orderbook_cancellation_query_requires_operator_permission,
        FindSorafsOrderbookCancellationByOrderId::new([0x12; 32]),
        sorafs::visit_find_sorafs_orderbook_cancellation_by_order_id
    );
    orderbook_query_permission_case!(
        orderbook_receipt_query_requires_operator_permission,
        FindSorafsOrderbookReceiptById::new([0x13; 32]),
        sorafs::visit_find_sorafs_orderbook_receipt_by_id
    );
    orderbook_query_permission_case!(
        orderbook_status_query_requires_operator_permission,
        FindSorafsOrderbookStatus,
        sorafs::visit_find_sorafs_orderbook_status
    );
    orderbook_query_permission_case!(
        orderbook_order_page_query_requires_operator_permission,
        FindSorafsOrderbookOrders::new(None, None, 10),
        sorafs::visit_find_sorafs_orderbook_orders
    );
    orderbook_query_permission_case!(
        orderbook_receipt_page_query_requires_operator_permission,
        FindSorafsOrderbookReceipts::new(None, None, 10),
        sorafs::visit_find_sorafs_orderbook_receipts
    );

    #[test]
    fn sccp_and_generic_parameter_permissions_are_separated() {
        let sccp = custom_parameter("sccp_registry_v1");
        assert_denied_with_permission(
            sccp.clone(),
            PermissionObject::from(CanSetParameters),
            parameter::visit_set_parameter,
        );
        assert_denied_with_permission(
            sccp.clone(),
            PermissionObject::from(CanManageSccpGovernance),
            parameter::visit_set_parameter,
        );
        let mut genesis = MockExecutor::new(true);
        parameter::visit_set_parameter(&mut genesis, &sccp);
        assert!(genesis.verdict().is_err());

        let unrelated = custom_parameter("unrelated_parameter");
        assert_denied_with_permission(
            unrelated.clone(),
            PermissionObject::from(CanManageSccpGovernance),
            parameter::visit_set_parameter,
        );
        assert_allowed_with_permission(
            unrelated,
            PermissionObject::from(CanSetParameters),
            parameter::visit_set_parameter,
        );
    }

    #[test]
    fn genesis_can_apply_typed_sccp_governance_without_seeded_permission() {
        let mut executor = MockExecutor::new(true);
        bridge::visit_apply_sccp_route_governance(&mut executor, &remove_sccp_route());
        assert!(executor.verdict().is_ok());
    }
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

/// Permission-checked visitor for executor upgrade instructions.
pub mod executor {
    use iroha_executor_data_model::permission::executor::CanUpgradeExecutor;

    use super::*;

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

    /// Emits a log instruction directly because logging has no permission gates.
    pub fn visit_log<V: Execute + Visit + ?Sized>(executor: &mut V, isi: &Log) {
        execute!(executor, isi)
    }
}

/// Permission-checked visitors for bridge instructions.
pub mod bridge {
    use iroha_executor_data_model::permission::sccp::CanManageSccpGovernance;
    use iroha_smart_contract::data_model::isi::BuiltInInstruction;
    use norito::NoritoSerialize;

    use super::*;

    fn visit_sccp_governance<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &(impl BuiltInInstruction + NoritoSerialize),
    ) {
        if executor.context().curr_block.is_genesis()
            || CanManageSccpGovernance.is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't apply SCCP route governance without CanManageSccpGovernance"
        );
    }

    /// Records a bridge receipt without additional permission gates.
    pub fn visit_record_bridge_receipt<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RecordBridgeReceipt,
    ) {
        execute!(executor, isi)
    }

    /// Applies one typed governed SCCP registry action.
    pub fn visit_apply_sccp_route_governance<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ApplySccpRouteGovernance,
    ) {
        visit_sccp_governance(executor, isi)
    }
}
