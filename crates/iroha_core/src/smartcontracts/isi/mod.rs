//! This module contains enumeration of all possible Iroha Special
//! Instructions, generic instruction types and related
//! implementations.
pub mod account;
mod account_admission;
pub mod asset;
pub mod block;
/// Content lane instruction handlers.
pub mod content;
pub mod domain;
pub mod kaigi;
pub mod multisig;
pub mod nft;
/// Offline allowance settlement instruction handlers.
pub mod offline;
/// Oracle feed admission and aggregation instruction handlers.
pub mod oracle;
pub mod query;
pub mod repo;
pub mod settlement;
/// Viral social incentive instruction handlers.
pub mod social;
pub mod soradns;
/// `SoraFS` pin registry instruction handlers.
pub mod sorafs;
pub mod space_directory;
/// Public lane staking instruction handlers.
pub mod staking;
pub mod triggers;
pub mod tx;
pub mod world;

use eyre::Result;
pub use iroha_data_model::Registrable;
use iroha_data_model::{
    isi::{error::InstructionExecutionError as Error, *},
    prelude::*,
};
use iroha_logger::prelude::*;
use mv::storage::StorageReadOnly;

use super::Execute;
use crate::{
    smartcontracts::triggers::set::SetReadOnly,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};

type InstructionHandler =
    fn(&InstructionBox, &AccountId, &mut StateTransaction<'_, '_>) -> Option<Result<(), Error>>;

fn dispatch_instruction<T: Execute + Clone + 'static>(
    instruction: &InstructionBox,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Option<Result<(), Error>> {
    instruction
        .as_any()
        .downcast_ref::<T>()
        .map(|isi| isi.clone().execute(authority, state_transaction))
}

const INSTRUCTION_HANDLERS: &[InstructionHandler] = &[
    dispatch_instruction::<RegisterPeerWithPop>,
    dispatch_instruction::<RegisterBox>,
    dispatch_instruction::<UnregisterBox>,
    dispatch_instruction::<MintBox>,
    dispatch_instruction::<BurnBox>,
    dispatch_instruction::<TransferBox>,
    dispatch_instruction::<SetKeyValueBox>,
    dispatch_instruction::<RemoveKeyValueBox>,
    dispatch_instruction::<SetAssetKeyValue>,
    dispatch_instruction::<RemoveAssetKeyValue>,
    dispatch_instruction::<AddSignatory>,
    dispatch_instruction::<RemoveSignatory>,
    dispatch_instruction::<SetAccountQuorum>,
    dispatch_instruction::<GrantBox>,
    dispatch_instruction::<RevokeBox>,
    dispatch_instruction::<ExecuteTrigger>,
    dispatch_instruction::<SetParameter>,
    dispatch_instruction::<Upgrade>,
    dispatch_instruction::<Log>,
    dispatch_instruction::<iroha_data_model::isi::InvalidInstruction>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::CreateKaigi>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::JoinKaigi>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::LeaveKaigi>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::EndKaigi>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::RecordKaigiUsage>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::SetKaigiRelayManifest>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::RegisterKaigiRelay>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::ReportKaigiRelayHealth>,
    dispatch_instruction::<runtime_upgrade::ProposeRuntimeUpgrade>,
    dispatch_instruction::<runtime_upgrade::ActivateRuntimeUpgrade>,
    dispatch_instruction::<runtime_upgrade::CancelRuntimeUpgrade>,
    dispatch_instruction::<Mint<Numeric, Asset>>,
    dispatch_instruction::<Burn<Numeric, Asset>>,
    dispatch_instruction::<Transfer<Asset, Numeric, Account>>,
    dispatch_instruction::<TransferAssetBatch>,
    dispatch_instruction::<iroha_data_model::isi::repo::RepoInstructionBox>,
    dispatch_instruction::<iroha_data_model::isi::repo::RepoIsi>,
    dispatch_instruction::<iroha_data_model::isi::repo::ReverseRepoIsi>,
    dispatch_instruction::<iroha_data_model::isi::repo::RepoMarginCallIsi>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::RegisterPinManifest>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::ApprovePinManifest>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::RetirePinManifest>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::RegisterProviderOwner>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::UnregisterProviderOwner>,
    dispatch_instruction::<iroha_data_model::isi::content::PublishContentBundle>,
    dispatch_instruction::<iroha_data_model::isi::content::RetireContentBundle>,
    dispatch_instruction::<iroha_data_model::isi::soradns::SubmitDirectoryDraft>,
    dispatch_instruction::<iroha_data_model::isi::soradns::PublishDirectory>,
    dispatch_instruction::<iroha_data_model::isi::soradns::RevokeResolver>,
    dispatch_instruction::<iroha_data_model::isi::soradns::UnrevokeResolver>,
    dispatch_instruction::<iroha_data_model::isi::soradns::AddReleaseSigner>,
    dispatch_instruction::<iroha_data_model::isi::soradns::RemoveReleaseSigner>,
    dispatch_instruction::<iroha_data_model::isi::soradns::SetDirectoryRotationPolicy>,
    dispatch_instruction::<iroha_data_model::isi::space_directory::PublishSpaceDirectoryManifest>,
    dispatch_instruction::<iroha_data_model::isi::space_directory::RevokeSpaceDirectoryManifest>,
    dispatch_instruction::<iroha_data_model::isi::domain_link::LinkAccountDomain>,
    dispatch_instruction::<iroha_data_model::isi::domain_link::UnlinkAccountDomain>,
    dispatch_instruction::<iroha_data_model::isi::SetAssetDefinitionAlias>,
    dispatch_instruction::<iroha_data_model::isi::offline::RegisterOfflineAllowance>,
    dispatch_instruction::<iroha_data_model::isi::offline::SubmitOfflineToOnlineTransfer>,
    dispatch_instruction::<iroha_data_model::isi::offline::RegisterOfflineVerdictRevocation>,
    dispatch_instruction::<iroha_data_model::isi::offline::ReclaimExpiredOfflineAllowance>,
    dispatch_instruction::<iroha_data_model::isi::social::ClaimTwitterFollowReward>,
    dispatch_instruction::<iroha_data_model::isi::social::SendToTwitter>,
    dispatch_instruction::<iroha_data_model::isi::social::CancelTwitterEscrow>,
    dispatch_instruction::<iroha_data_model::isi::oracle::RegisterOracleFeed>,
    dispatch_instruction::<iroha_data_model::isi::oracle::SubmitOracleObservation>,
    dispatch_instruction::<iroha_data_model::isi::oracle::AggregateOracleFeed>,
    dispatch_instruction::<iroha_data_model::isi::staking::ActivatePublicLaneValidator>,
    dispatch_instruction::<iroha_data_model::isi::staking::ExitPublicLaneValidator>,
    dispatch_instruction::<iroha_data_model::isi::nexus::SetLaneRelayEmergencyValidators>,
    dispatch_instruction::<iroha_data_model::isi::staking::RegisterPublicLaneValidator>,
    dispatch_instruction::<iroha_data_model::isi::staking::BondPublicLaneStake>,
    dispatch_instruction::<iroha_data_model::isi::staking::SchedulePublicLaneUnbond>,
    dispatch_instruction::<iroha_data_model::isi::staking::FinalizePublicLaneUnbond>,
    dispatch_instruction::<iroha_data_model::isi::staking::SlashPublicLaneValidator>,
    dispatch_instruction::<iroha_data_model::isi::staking::CancelConsensusEvidencePenalty>,
    dispatch_instruction::<iroha_data_model::isi::staking::RecordPublicLaneRewards>,
    dispatch_instruction::<iroha_data_model::isi::settlement::SettlementInstructionBox>,
    dispatch_instruction::<iroha_data_model::isi::settlement::DvpIsi>,
    dispatch_instruction::<iroha_data_model::isi::settlement::PvpIsi>,
    dispatch_instruction::<SetKeyValue<Trigger>>,
    dispatch_instruction::<iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode>,
    dispatch_instruction::<iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes>,
    dispatch_instruction::<iroha_data_model::isi::smart_contract_code::ActivateContractInstance>,
    dispatch_instruction::<iroha_data_model::isi::smart_contract_code::DeactivateContractInstance>,
    dispatch_instruction::<verifying_keys::RegisterVerifyingKey>,
    dispatch_instruction::<verifying_keys::UpdateVerifyingKey>,
    dispatch_instruction::<zk::RegisterZkAsset>,
    dispatch_instruction::<zk::ScheduleConfidentialPolicyTransition>,
    dispatch_instruction::<zk::CancelConfidentialPolicyTransition>,
    dispatch_instruction::<zk::Shield>,
    dispatch_instruction::<zk::ZkTransfer>,
    dispatch_instruction::<zk::Unshield>,
    dispatch_instruction::<zk::CreateElection>,
    dispatch_instruction::<zk::SubmitBallot>,
    dispatch_instruction::<zk::FinalizeElection>,
    dispatch_instruction::<zk::VerifyProof>,
    dispatch_instruction::<zk::PruneProofs>,
    dispatch_instruction::<iroha_data_model::isi::bridge::SubmitBridgeProof>,
    dispatch_instruction::<iroha_data_model::isi::bridge::RecordBridgeReceipt>,
    dispatch_instruction::<confidential::PublishPedersenParams>,
    dispatch_instruction::<confidential::SetPedersenParamsLifecycle>,
    dispatch_instruction::<confidential::PublishPoseidonParams>,
    dispatch_instruction::<confidential::SetPoseidonParamsLifecycle>,
    dispatch_instruction::<iroha_data_model::isi::consensus_keys::RegisterConsensusKey>,
    dispatch_instruction::<iroha_data_model::isi::consensus_keys::RotateConsensusKey>,
    dispatch_instruction::<iroha_data_model::isi::consensus_keys::DisableConsensusKey>,
    dispatch_instruction::<iroha_data_model::isi::endorsement::RegisterDomainCommittee>,
    dispatch_instruction::<iroha_data_model::isi::endorsement::SetDomainEndorsementPolicy>,
    dispatch_instruction::<iroha_data_model::isi::endorsement::SubmitDomainEndorsement>,
    dispatch_instruction::<iroha_data_model::isi::governance::ProposeDeployContract>,
    dispatch_instruction::<iroha_data_model::isi::governance::ProposeRuntimeUpgradeProposal>,
    dispatch_instruction::<iroha_data_model::isi::governance::CastZkBallot>,
    dispatch_instruction::<iroha_data_model::isi::governance::CastPlainBallot>,
    dispatch_instruction::<iroha_data_model::isi::governance::EnactReferendum>,
    dispatch_instruction::<iroha_data_model::isi::governance::FinalizeReferendum>,
    dispatch_instruction::<iroha_data_model::isi::governance::ApproveGovernanceProposal>,
    dispatch_instruction::<iroha_data_model::isi::governance::PersistCouncilForEpoch>,
    dispatch_instruction::<iroha_data_model::isi::governance::RecordCitizenServiceOutcome>,
    dispatch_instruction::<iroha_data_model::isi::governance::RegisterCitizen>,
    dispatch_instruction::<iroha_data_model::isi::governance::UnregisterCitizen>,
    dispatch_instruction::<iroha_data_model::isi::governance::SlashGovernanceLock>,
    dispatch_instruction::<iroha_data_model::isi::governance::RestituteGovernanceLock>,
];

impl Execute for InstructionBox {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        iroha_logger::debug!(isi=%self, "Executing");

        if let Some(result) = INSTRUCTION_HANDLERS
            .iter()
            .find_map(|handler| handler(&self, authority, state_transaction))
        {
            return result;
        }

        // Custom instructions are expected to be handled by a custom executor
        if self.as_any().downcast_ref::<CustomInstruction>().is_some() {
            return Err(Error::from(
                "Custom instructions require an executor upgrade",
            ));
        }

        // If we reach here, the instruction type is unknown or unregistered
        Err(Error::from("Unknown instruction type"))
    }
}

impl Execute for iroha_data_model::isi::InvalidInstruction {
    fn execute(
        self,
        _authority: &AccountId,
        _state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        Err(Error::from(format!(
            "invalid instruction payload: wire_id={} payload_hash={} message={}",
            self.wire_id,
            hex::encode(self.payload_hash),
            self.message
        )))
    }
}

impl Execute for RegisterBox {
    #[iroha_logger::log(name = "register", skip_all, fields(id))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Peer(isi) => isi.execute(authority, state_transaction),
            Self::Domain(isi) => isi.execute(authority, state_transaction),
            Self::Account(isi) => isi.execute(authority, state_transaction),
            Self::AssetDefinition(isi) => isi.execute(authority, state_transaction),
            Self::Nft(isi) => isi.execute(authority, state_transaction),
            Self::Role(isi) => isi.execute(authority, state_transaction),
            Self::Trigger(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for UnregisterBox {
    #[iroha_logger::log(name = "unregister", skip_all, fields(id))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Peer(isi) => isi.execute(authority, state_transaction),
            Self::Domain(isi) => isi.execute(authority, state_transaction),
            Self::Account(isi) => isi.execute(authority, state_transaction),
            Self::AssetDefinition(isi) => isi.execute(authority, state_transaction),
            Self::Nft(isi) => isi.execute(authority, state_transaction),
            Self::Role(isi) => isi.execute(authority, state_transaction),
            Self::Trigger(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for MintBox {
    #[iroha_logger::log(name = "Mint", skip_all, fields(destination))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Asset(isi) => isi.execute(authority, state_transaction),
            Self::TriggerRepetitions(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for BurnBox {
    #[iroha_logger::log(name = "burn", skip_all, fields(destination))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Asset(isi) => isi.execute(authority, state_transaction),
            Self::TriggerRepetitions(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for TransferBox {
    #[iroha_logger::log(name = "transfer", skip_all, fields(from, to))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Domain(isi) => isi.execute(authority, state_transaction),
            Self::AssetDefinition(isi) => isi.execute(authority, state_transaction),
            Self::Asset(isi) => isi.execute(authority, state_transaction),
            Self::Nft(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for SetKeyValueBox {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Domain(isi) => isi.execute(authority, state_transaction),
            Self::Account(isi) => isi.execute(authority, state_transaction),
            Self::AssetDefinition(isi) => isi.execute(authority, state_transaction),
            Self::Nft(isi) => isi.execute(authority, state_transaction),
            Self::Trigger(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for RemoveKeyValueBox {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Domain(isi) => isi.execute(authority, state_transaction),
            Self::Account(isi) => isi.execute(authority, state_transaction),
            Self::AssetDefinition(isi) => isi.execute(authority, state_transaction),
            Self::Nft(isi) => isi.execute(authority, state_transaction),
            Self::Trigger(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for GrantBox {
    #[iroha_logger::log(name = "grant", skip_all, fields(object))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Permission(sub_isi) => sub_isi.execute(authority, state_transaction),
            Self::Role(sub_isi) => sub_isi.execute(authority, state_transaction),
            Self::RolePermission(sub_isi) => sub_isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for RevokeBox {
    #[iroha_logger::log(name = "revoke", skip_all, fields(object))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Permission(sub_isi) => sub_isi.execute(authority, state_transaction),
            Self::Role(sub_isi) => sub_isi.execute(authority, state_transaction),
            Self::RolePermission(sub_isi) => sub_isi.execute(authority, state_transaction),
        }
    }
}

pub mod prelude {
    //! Re-export important traits and types for glob import `(::*)`
    pub use super::*;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        events::execute_trigger::ExecuteTriggerEventFilter, isi::error::InvalidParameterError,
        permission,
    };
    use iroha_executor_data_model::permission::trigger::CanRegisterTrigger;
    use iroha_test_samples::{
        ALICE_ID, ALICE_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
        gen_account_in,
    };
    use tokio::test;

    use super::*;
    use crate::{
        block::ValidBlock,
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
        tx::{AcceptTransactionFail, AcceptedTransaction},
    };

    fn state_with_test_domains(kura: &Arc<Kura>) -> Result<State> {
        let world = World::with([], [], []);
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura.clone(), query_handle);
        let asset_definition_id =
            iroha_data_model::asset::AssetDefinitionId::new("wonderland".parse()?, "rose".parse()?);
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let wonderland: DomainId = "wonderland".parse()?;
        Register::domain(Domain::new(wonderland.clone()))
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;
        Register::account(Account::new(ALICE_ID.clone().to_account_id(wonderland)))
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;
        let trigger_perm: permission::Permission = CanRegisterTrigger {
            authority: ALICE_ID.clone(),
        }
        .into();
        Grant::account_permission(trigger_perm, ALICE_ID.clone())
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;
        Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id.clone())
                .with_name(asset_definition_id.name().to_string()),
        )
        .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;
        state_transaction.apply();
        state_block.commit().unwrap();
        Ok(state)
    }

    #[test]
    async fn nft() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let nft_id: NftId = "rose$wonderland".parse()?;
        let key = "Bytes".parse::<Name>()?;
        Register::nft(Nft::new(nft_id.clone(), Metadata::default()))
            .execute(&account_id, &mut state_transaction)?;
        SetKeyValue::nft(nft_id.clone(), key.clone(), vec![1_u32, 2_u32, 3_u32])
            .execute(&account_id, &mut state_transaction)?;
        state_transaction.apply();
        state_block.commit().unwrap();
        let state_view = state.view();
        let nft = state_view.world.nft(&nft_id)?;
        let value = nft.content.get(&key).cloned();
        assert_eq!(value, Some(vec![1_u32, 2_u32, 3_u32,].into()));
        Ok(())
    }

    #[test]
    async fn account_metadata() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let key = "Bytes".parse::<Name>()?;
        SetKeyValue::account(account_id.clone(), key.clone(), vec![1_u32, 2_u32, 3_u32])
            .execute(&account_id, &mut state_transaction)?;
        state_transaction.apply();
        state_block.commit().unwrap();
        let bytes = state.view().world.map_account(&account_id, |account| {
            account.value().metadata().get(&key).cloned()
        })?;
        assert_eq!(bytes, Some(vec![1_u32, 2_u32, 3_u32,].into()));
        Ok(())
    }

    #[test]
    async fn account_metadata_limit() -> Result<()> {
        use std::str::FromStr as _;

        use iroha_data_model::{
            parameter::{CustomParameter, CustomParameterId},
            prelude::Parameter,
        };
        use iroha_primitives::json::Json;

        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();

        // Set a very small metadata size limit via custom parameter
        let param_id = CustomParameterId::from_str("max_metadata_value_bytes")?;
        let small_limit = 16_u64;
        let set_param = SetParameter::new(Parameter::Custom(CustomParameter::new(
            param_id,
            Json::new(small_limit),
        )));
        set_param.execute(&account_id, &mut state_transaction)?;

        // Attempt to set a metadata value exceeding the limit
        let key = "TooBig".parse::<Name>()?;
        let big = Json::new("X".repeat(32)); // 32 > 16
        let res = SetKeyValue::account(account_id.clone(), key.clone(), big)
            .execute(&account_id, &mut state_transaction);
        assert!(matches!(res, Err(Error::InvalidParameter(_))));

        // Now lower the value and ensure it succeeds
        let ok = Json::new("Y".repeat(8));
        SetKeyValue::account(account_id.clone(), key.clone(), ok)
            .execute(&account_id, &mut state_transaction)?;

        state_transaction.apply();
        state_block.commit().unwrap();
        Ok(())
    }

    #[test]
    async fn register_contract_manifest_requires_permission_and_is_queryable() -> Result<()> {
        use iroha_crypto::Hash;
        use iroha_data_model::{
            isi::smart_contract_code, permission, prelude as dm, query::smart_contract::prelude,
            smart_contract::manifest,
        };

        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let alice = ALICE_ID.clone();
        // Build a dummy manifest with a random code_hash
        let h = Hash::new(b"dummy_code");
        let manifest = manifest::ContractManifest {
            code_hash: Some(h),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&ALICE_KEYPAIR);

        // Attempt to register without permission should fail
        let res = smart_contract_code::RegisterSmartContractCode {
            manifest: manifest.clone(),
        }
        .execute(&alice, &mut stx);
        assert!(matches!(
            res,
            Err(Error::InvariantViolation(msg)) if msg.as_ref().contains("CanRegisterSmartContractCode")
        ));

        // Grant the permission to Alice and try again
        let token =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
        let perm: permission::Permission = token.into();
        dm::Grant::account_permission(perm, alice.clone()).execute(&alice, &mut stx)?;

        smart_contract_code::RegisterSmartContractCode {
            manifest: manifest.clone(),
        }
        .execute(&alice, &mut stx)?;

        stx.apply();
        state_block.commit().unwrap();

        // Verify it is stored
        let got = state.view().world().contract_manifests().get(&h).cloned();
        assert_eq!(got, Some(manifest.clone()));

        // Verify query returns it
        let q = prelude::FindContractManifestByCodeHash { code_hash: h };
        let out = <_ as crate::smartcontracts::ValidSingularQuery>::execute(&q, &state.view())?;
        assert_eq!(out, manifest);

        Ok(())
    }

    #[test]
    async fn register_contract_manifest_requires_provenance() -> Result<()> {
        use iroha_crypto::Hash;
        use iroha_data_model::{
            isi::smart_contract_code, permission, prelude as dm, smart_contract::manifest,
        };

        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let alice = ALICE_ID.clone();
        let h = Hash::new(b"dummy_code");
        let manifest = manifest::ContractManifest {
            code_hash: Some(h),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        };

        let token =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
        let perm: permission::Permission = token.into();
        dm::Grant::account_permission(perm, alice.clone()).execute(&alice, &mut stx)?;

        let err = smart_contract_code::RegisterSmartContractCode { manifest }
            .execute(&alice, &mut stx)
            .expect_err("missing provenance must fail");
        match err {
            Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                assert!(msg.contains("provenance"), "unexpected msg: {msg}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
        Ok(())
    }

    #[test]
    async fn register_contract_manifest_rejects_wrong_signer() -> Result<()> {
        use iroha_crypto::Hash;
        use iroha_data_model::{
            isi::smart_contract_code, permission, prelude as dm, smart_contract::manifest,
        };

        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let alice = ALICE_ID.clone();
        let h = Hash::new(b"dummy_code");
        let manifest = manifest::ContractManifest {
            code_hash: Some(h),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&KeyPair::random());

        let token =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
        let perm: permission::Permission = token.into();
        dm::Grant::account_permission(perm, alice.clone()).execute(&alice, &mut stx)?;

        let err = smart_contract_code::RegisterSmartContractCode { manifest }
            .execute(&alice, &mut stx)
            .expect_err("wrong signer must fail");
        match err {
            Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                assert!(
                    msg.contains("not authorised"),
                    "unexpected msg for wrong signer: {msg}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
        Ok(())
    }

    #[test]
    async fn burning_trigger_to_zero_removes_it() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let trigger_id = "will_be_removed".parse::<TriggerId>()?;

        // Register the trigger with Exactly(1) repeats
        let register_trigger = Register::trigger(Trigger::new(
            trigger_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Exactly(1),
                account_id.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(account_id.clone()),
            ),
        ));
        register_trigger.execute(&account_id, &mut state_transaction)?;

        // Burn 1 repeat to reach zero; the trigger should be removed immediately
        Burn::trigger_repetitions(1, trigger_id.clone())
            .execute(&account_id, &mut state_transaction)?;

        state_transaction.apply();
        state_block.commit().unwrap();

        // Verify trigger is no longer active
        let active = state
            .view()
            .world
            .triggers()
            .inspect_by_id(&trigger_id, |_| ())
            .is_some();
        assert!(!active, "trigger should be removed at zero repeats");

        Ok(())
    }

    #[test]
    async fn registering_zero_repeat_trigger_is_noop() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let trigger_id = "no_effect".parse::<TriggerId>()?;

        // Attempt to register a trigger with Exactly(0) repeats
        let register_trigger = Register::trigger(Trigger::new(
            trigger_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Exactly(0),
                account_id.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(account_id.clone()),
            ),
        ));
        register_trigger.execute(&account_id, &mut state_transaction)?;

        state_transaction.apply();
        state_block.commit().unwrap();

        // The trigger should not be present/active
        let active = state
            .view()
            .world
            .triggers()
            .inspect_by_id(&trigger_id, |_| ())
            .is_some();
        assert!(!active, "zero-repeat triggers must not be registered");

        Ok(())
    }

    #[test]
    async fn register_box_trigger_executes() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let trigger_id = "boxed_trigger".parse::<TriggerId>()?;

        let trigger = Trigger::new(
            trigger_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Indefinitely,
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(ALICE_ID.clone()),
            ),
        );
        RegisterBox::Trigger(Register::trigger(trigger))
            .execute(&ALICE_ID, &mut state_transaction)?;

        state_transaction.apply();
        state_block.commit().unwrap();

        let registered = state
            .view()
            .world
            .triggers()
            .inspect_by_id(&trigger_id, |_| ())
            .is_some();
        assert!(registered, "trigger should be registered via RegisterBox");

        Ok(())
    }

    #[test]
    async fn asset_definition_metadata() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let definition_id = AssetDefinitionId::new("wonderland".parse()?, "rose".parse()?);
        let account_id = ALICE_ID.clone();
        let key = "Bytes".parse::<Name>()?;
        SetKeyValue::asset_definition(
            definition_id.clone(),
            key.clone(),
            vec![1_u32, 2_u32, 3_u32],
        )
        .execute(&account_id, &mut state_transaction)?;
        state_transaction.apply();
        state_block.commit().unwrap();
        let value = state
            .view()
            .world
            .asset_definition(&definition_id)?
            .metadata()
            .get(&key)
            .cloned();
        assert_eq!(value, Some(vec![1_u32, 2_u32, 3_u32,].into()));
        Ok(())
    }

    #[test]
    async fn instruction_box_handles_asset_metadata() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let asset_definition_id = AssetDefinitionId::new("wonderland".parse()?, "rose".parse()?);
        let asset_id = AssetId::new(asset_definition_id, account_id.clone());
        Mint::asset_numeric(numeric!(1), asset_id.clone())
            .execute(&account_id, &mut state_transaction)?;

        let key = "note".parse::<Name>()?;
        let value = Json::from(norito::json!("demo"));
        InstructionBox::from(SetAssetKeyValue::new(asset_id.clone(), key.clone(), value))
            .execute(&account_id, &mut state_transaction)?;
        InstructionBox::from(RemoveAssetKeyValue::new(asset_id.clone(), key))
            .execute(&account_id, &mut state_transaction)?;

        state_transaction.apply();
        state_block.commit().unwrap();

        let view = state.view();
        let metadata = view.world.asset_metadata().get(&asset_id);
        assert!(metadata.is_none(), "asset metadata should be cleared");
        Ok(())
    }

    #[test]
    async fn domain_metadata() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let domain_id = "wonderland".parse::<DomainId>()?;
        let account_id = ALICE_ID.clone();
        let key = "Bytes".parse::<Name>()?;
        SetKeyValue::domain(domain_id.clone(), key.clone(), vec![1_u32, 2_u32, 3_u32])
            .execute(&account_id, &mut state_transaction)?;
        state_transaction.apply();
        state_block.commit().unwrap();
        let bytes = state
            .view()
            .world
            .domain(&domain_id)?
            .metadata()
            .get(&key)
            .cloned();
        assert_eq!(bytes, Some(vec![1_u32, 2_u32, 3_u32,].into()));
        Ok(())
    }

    #[test]
    async fn executing_unregistered_trigger_should_return_error() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let trigger_id = "test_trigger_id".parse()?;

        assert!(matches!(
            ExecuteTrigger::new(trigger_id)
                .execute(&account_id, &mut state_transaction)
                .expect_err("Error expected"),
            Error::Find(_)
        ));

        state_transaction.apply();
        state_block.commit().unwrap();

        Ok(())
    }

    #[test]
    async fn unauthorized_trigger_execution_should_return_error() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let wonderland: DomainId = "wonderland".parse()?;
        let (fake_account_id, _fake_account_keypair) = gen_account_in("wonderland");
        let trigger_id = "test_trigger_id".parse::<TriggerId>()?;

        // register fake account
        let register_account = Register::account(Account::new(
            fake_account_id.clone().to_account_id(wonderland),
        ));
        register_account.execute(&account_id, &mut state_transaction)?;

        // register the trigger
        let register_trigger = Register::trigger(Trigger::new(
            trigger_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Indefinitely,
                account_id.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(account_id.clone()),
            ),
        ));

        register_trigger.execute(&account_id, &mut state_transaction)?;

        // execute with the valid account
        ExecuteTrigger::new(trigger_id.clone()).execute(&account_id, &mut state_transaction)?;

        // execute with the fake account
        assert!(matches!(
            ExecuteTrigger::new(trigger_id)
                .execute(&fake_account_id, &mut state_transaction)
                .expect_err("Error expected"),
            Error::InvariantViolation(_)
        ));

        state_transaction.apply();
        state_block.commit().unwrap();

        Ok(())
    }

    #[test]
    async fn time_trigger_with_single_execution_is_not_mintable() -> Result<()> {
        use iroha_data_model::events::time::{ExecutionTime, Schedule, TimeEventFilter};

        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let trigger_id = "single_time".parse::<TriggerId>()?;

        // Schedule with no period (single execution) is not mintable; repeats must be Exactly(1)
        let filter = TimeEventFilter::new(ExecutionTime::Schedule(Schedule {
            start_ms: 0,
            period_ms: None,
        }));

        let bad = Register::trigger(Trigger::new(
            trigger_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Exactly(2), // invalid for non-mintable filter
                account_id.clone(),
                filter,
            ),
        ));
        assert!(matches!(
            bad.execute(&account_id, &mut state_transaction)
                .expect_err("expected error"),
            Error::Math(_)
        ));

        state_transaction.apply();
        state_block.commit().unwrap();
        Ok(())
    }

    #[test]
    async fn not_allowed_to_register_genesis_domain_or_account() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        assert!(matches!(
            Register::domain(Domain::new("genesis".parse()?))
                .execute(&account_id, &mut state_transaction)
                .expect_err("Error expected"),
            Error::InvariantViolation(_)
        ));
        let wonderland: DomainId = "wonderland".parse()?;
        let register_account = Register::account(Account::new(
            SAMPLE_GENESIS_ACCOUNT_ID.clone().to_account_id(wonderland),
        ));
        assert!(matches!(
            register_account
                .execute(&account_id, &mut state_transaction)
                .expect_err("Error expected"),
            Error::InvariantViolation(_)
        ));
        state_transaction.apply();
        state_block.commit().unwrap();

        Ok(())
    }

    #[test]
    async fn transaction_signed_by_genesis_account_should_be_rejected() -> Result<()> {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };

        let tx = TransactionBuilder::new(chain_id.clone(), SAMPLE_GENESIS_ACCOUNT_ID.clone())
            .with_instructions::<InstructionBox>([])
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
        let crypto_cfg = state.crypto();
        assert!(matches!(
            AcceptedTransaction::accept(
                tx,
                &chain_id,
                max_clock_drift,
                tx_limits,
                crypto_cfg.as_ref()
            ),
            Err(AcceptTransactionFail::UnexpectedGenesisAccountSignature)
        ));
        Ok(())
    }
}
