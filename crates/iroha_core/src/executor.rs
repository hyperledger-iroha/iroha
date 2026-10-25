// Detached executor note: Keep this handler minimal and side-effect free; record only deltas.
// Perform complex checks during `StateBlock::merge_into`; extend cautiously for new ISIs.
//! Structures and impls related to processing Iroha Virtual Machine (IVM) runtime executors.
#[cfg(feature = "zk-preverify")]
use crate::zk::PreverifyResult;
use crate::{
    gas as isi_gas,
    smartcontracts::{
        Execute as _, code,
        ivm::cache::{ExecutableProgramSummary, IvmCache, ProgramSummary},
    },
    state::{
        StateReadOnly, StateTransaction, WorldReadOnly, fee_sponsor_revision_safe_activation_height,
    },
    status::{self, NexusFeeEvent, NexusFeePayer},
};
use base64::Engine as _;
#[cfg(test)]
use core::str::FromStr;
use core::{
    convert::TryFrom,
    ops::{Deref, DerefMut},
};
use derive_more::Debug;
use iroha_config::parameters::actual::{ConfidentialGas, Pipeline};
use iroha_crypto::Hash;
use iroha_data_model::block::consensus::RootFeePolicy;
#[cfg(test)]
use iroha_data_model::nexus::{
    VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX, VerifiedFeeSponsorVaultAllocation,
};
#[cfg(test)]
use iroha_data_model::prelude::Domain;
use iroha_data_model::smart_contract::manifest::{
    ContractPermissionScopeV1, EntrypointAuthorizationV1,
};
use iroha_data_model::{
    Identifiable as _, ValidationFail,
    account::AccountId,
    asset::{
        AssetBalancePolicy, AssetDefinition, ResolvedAssetDefinitionAliasV1,
        id::{AssetBalanceScope, AssetDefinitionId, AssetId},
        value::Asset,
    },
    block::BlockHeader,
    executor::{self as data_model_executor, ExecutorDataModel},
    isi::{
        CustomInstruction, Grant, GrantBox, InstructionBox, InstructionBox as DMInstructionBox,
        RemoveKeyValueBox, Revoke, RevokeBox, SetKeyValueBox, TransferBox, UnregisterBox,
        error::InstructionExecutionError, mint_burn::MintBox, register::RegisterBox,
    },
    nexus::{
        FeeDebitSource, FeeRejectionCode, FeeSponsorBeneficiaryEpochBudgetWindow,
        FeeSponsorBlockBudgetWindow, FeeSponsorBudgetCounter, FeeSponsorBudgetCounterKey,
        FeeSponsorBudgetWindow, FeeSponsorEligibility, FeeSponsorEnrollmentKey,
        FeeSponsorMultisigOperation, FeeSponsorProgramEpochBudgetWindow, FeeSponsorProgramId,
        FeeSponsorProgramLifecycle, FeeSponsorProgramRevision, FeeSponsorProgramRevisionKey,
        FeeSponsorRuleEffect, FeeSponsorRuleSelector, FeeSponsorVaultKey,
    },
    permission::Permission,
    prelude::{Account, Burn, Mint, Register, Transfer, Trigger, Unregister},
    query::{
        self as data_model_query, AnyQueryBox, QueryItemKind, QueryRequest, QueryWithParams,
        SingularQueryBox,
    },
    role::{Role, RoleId},
    smart_contract::payloads::{ExecutorContext, Validate as ValidatePayload},
    transaction::{
        Executable, ExecutableBatchItem, FeeChargeKind, FeeChargeLimit, FeePaymentIntent,
        SignedTransaction, executable::ContractInvocation, signed::TransactionPayload,
    },
    trigger::TriggerId,
};
use iroha_executor_data_model::{
    isi::multisig::MultisigInstructionBox, permission as executor_permission,
};
use iroha_logger::{debug, trace, warn};
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::state_path::StatePath;
use iroha_model_base::topology::DataSpaceId;
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, Quantity},
};
use ivm::runtime::IvmConfig;
use ivm::{IVM, Memory, RuntimeTemplate, VMError};
use mv::storage::StorageReadOnly;
use norito::{
    codec::{Decode, Encode},
    json::{self, JsonDeserialize as JsonDeserializeTrait, JsonSerialize as JsonSerializeTrait},
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Arc, Mutex},
};
pub(crate) mod attachment_custody;
mod attachment_direct;
mod executable_source;
#[path = "executor_execution_fee.rs"]
mod execution_fee;
mod fee_sponsor_multisig;
mod settlement_replay;
/// Test-only observations of the original signed executor input.
#[cfg(test)]
pub(crate) mod signed_source_observation;
pub(crate) use settlement_replay::reject_committed_direct_payment;
pub(crate) mod root_fees;
/// Authenticated root scope for native and contract-generated instruction effects.
pub(crate) mod root_scope;
pub(crate) use execution_fee::{
    ExecutionFeeExemption, ExecutionFeeMeter, ExecutionFeeSettlementError,
};
#[path = "executor_execution_effects.rs"]
mod execution_effects;
pub(crate) use execution_effects::ExecutionEffects;
#[path = "executor_fastpq_rejection_tail.rs"]
mod fastpq_rejection_tail;
/// Signature-bound inputs already captured before executable dispatch.
struct RawIvmTransactionInputs<'a> {
    metadata: &'a Metadata,
    gas_limit_md: Option<u64>,
    gas_asset_opt: Option<String>,
    fee_sponsor: Option<FeeSponsorProgramId>,
    skip_nexus_fee: bool,
    tx_bytes_len: usize,
}

/// Borrow only the authored native instructions in one original mixed batch.
/// Counting does not encode, clone or allocate; iteration preserves source order.
#[derive(Clone)]
struct AuthoredBatchInstructions<'a> {
    items: core::slice::Iter<'a, ExecutableBatchItem>,
    remaining: usize,
}
impl<'a> AuthoredBatchInstructions<'a> {
    fn new(items: &'a [ExecutableBatchItem]) -> Self {
        Self {
            items: items.iter(),
            remaining: items
                .iter()
                .filter(|item| matches!(item, ExecutableBatchItem::Instruction(_)))
                .count(),
        }
    }
}
impl<'a> Iterator for AuthoredBatchInstructions<'a> {
    type Item = &'a InstructionBox;
    fn next(&mut self) -> Option<Self::Item> {
        for item in self.items.by_ref() {
            if let ExecutableBatchItem::Instruction(instruction) = item {
                self.remaining -= 1;
                return Some(instruction);
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for AuthoredBatchInstructions<'_> {}

/// One-shot proof that the executor debited one exact sponsored fee charge.
pub(crate) struct VerifiedFeeSponsorCharge {
    submitting_authority: AccountId,
    program_id: FeeSponsorProgramId,
    kind: FeeChargeKind,
    source_id: AssetId,
    destination: Option<AccountId>,
    amount: Quantity,
    // Retained only from the original executor invocation after its signed fee
    // limit and sponsor debit checks. Test-only capability fixtures leave this
    // absent and cannot establish quantity-source ownership.
    quantity_source: Option<(Hash, Hash, u64)>,
}
impl VerifiedFeeSponsorCharge {
    fn transfer(
        submitting_authority: AccountId,
        program_id: FeeSponsorProgramId,
        kind: FeeChargeKind,
        source_id: AssetId,
        destination: AccountId,
        amount: Quantity,
        quantity_source: Option<(Hash, Hash, u64)>,
    ) -> Self {
        Self {
            submitting_authority,
            program_id,
            kind,
            source_id,
            destination: Some(destination),
            amount,
            quantity_source,
        }
    }
    fn burn(
        submitting_authority: AccountId,
        program_id: FeeSponsorProgramId,
        kind: FeeChargeKind,
        source_id: AssetId,
        amount: Quantity,
        quantity_source: Option<(Hash, Hash, u64)>,
    ) -> Self {
        Self {
            submitting_authority,
            program_id,
            kind,
            source_id,
            destination: None,
            amount,
            quantity_source,
        }
    }
    #[cfg(test)]
    pub(crate) fn transfer_for_test(
        submitting_authority: AccountId,
        program_id: FeeSponsorProgramId,
        source_id: AssetId,
        destination: AccountId,
        amount: Quantity,
    ) -> Self {
        Self::transfer(
            submitting_authority,
            program_id,
            FeeChargeKind::PipelineGas,
            source_id,
            destination,
            amount,
            None,
        )
    }
    #[cfg(test)]
    pub(crate) fn burn_for_test(
        submitting_authority: AccountId,
        program_id: FeeSponsorProgramId,
        source_id: AssetId,
        amount: Quantity,
    ) -> Self {
        Self::burn(
            submitting_authority,
            program_id,
            FeeChargeKind::Nexus,
            source_id,
            amount,
            None,
        )
    }
    pub(crate) fn into_parts(
        self,
    ) -> (
        AccountId,
        FeeSponsorProgramId,
        FeeChargeKind,
        AssetId,
        Option<AccountId>,
        Quantity,
        Option<(Hash, Hash, u64)>,
    ) {
        (
            self.submitting_authority,
            self.program_id,
            self.kind,
            self.source_id,
            self.destination,
            self.amount,
            self.quantity_source,
        )
    }
}
// NoritoDecode alias is unused; keep Decode via norito::codec where needed inline
const EXECUTOR_LENGTH_PREFIX_BYTES: usize = 8;
const EXECUTOR_LENGTH_PREFIX_BYTES_U64: u64 = 8;
/// Maximum accepted size of one framed executor result, including its prefix.
///
/// Executor results conventionally share the VM's one-MiB heap window with their input. Keeping the
/// host-side limit at that deterministic region size prevents a guest-controlled length prefix from
/// requesting an unbounded host allocation, while retaining the entire addressable result envelope.
const MAX_EXECUTOR_OUTPUT_BYTES: u64 = Memory::HEAP_MAX_SIZE;
#[derive(Debug, Clone, PartialEq, Eq)]
enum NativeQueryAccess {
    /// Control-plane or public-definition data available to any registered account.
    Registered,
    /// Private data for one exact account.
    Account(AccountId),
    /// Exact account identity or direct permissions eligible for scoped DPN enrollment checks.
    EnrollmentAccount(AccountId),
    /// Private UID membership and aggregate activity for an exact asset's owner.
    AssetUidOwner(AssetDefinitionId),
    /// Exact current native owner and incarnation; general read grants never substitute.
    AssetEscrowOwner(
        AssetDefinitionId,
        iroha_data_model::nexus::AxtAssetIncarnationV1,
    ),
    /// Exact asset owner or wallet owner; never global/public ledger read.
    AssetFiOwner(AssetDefinitionId, AccountId),
    /// Ledger-wide state requiring the genesis-issued read root.
    AllLedger,
}
fn decode_native_iterable_payload_exact<Q>(payload: &[u8]) -> Option<Q>
where
    Q: Decode + Encode,
{
    let mut input = payload;
    let query = Q::decode(&mut input).ok()?;
    (query.encode() == payload).then_some(query)
}
#[allow(clippy::too_many_lines)]
fn native_singular_query_access(query: &SingularQueryBox) -> NativeQueryAccess {
    match query {
        SingularQueryBox::FindAccountById(query) => {
            NativeQueryAccess::EnrollmentAccount(query.account_id().clone())
        }
        SingularQueryBox::FindAliasesByAccountId(query) => {
            NativeQueryAccess::Account(query.account_id().clone())
        }
        SingularQueryBox::FindAssetById(query) => {
            NativeQueryAccess::Account(query.asset_id().account().clone())
        }
        SingularQueryBox::FindFeeSponsorProgramById(query) => {
            NativeQueryAccess::Account(query.id().sponsor.clone())
        }
        SingularQueryBox::FindSettlementReceiptById(query) => {
            NativeQueryAccess::Account(query.id.owner().clone())
        }
        // Runtime/control-plane definitions and explicitly routed subsystem state do not expose
        // the general account roster, balances, or transaction history. Alias reads still pass
        // through their separate exact-scope gate below, and protected SoraFS records pass
        // through the subsystem-specific gates.
        SingularQueryBox::FindExecutorDataModel(_)
        | SingularQueryBox::FindGameSessionById(_)
        | SingularQueryBox::FindExecutionProofVerificationById(_)
        | SingularQueryBox::FindParameters(_)
        | SingularQueryBox::FindAccountRecoveryPolicyByAlias(_)
        | SingularQueryBox::FindAccountRecoveryRequestByAlias(_)
        | SingularQueryBox::FindContractManifestByArtifactId(_)
        | SingularQueryBox::FindAbiVersion(_)
        | SingularQueryBox::FindAssetDefinitionById(_)
        | SingularQueryBox::FindAssetDefinitionDirectHome(_)
        | SingularQueryBox::FindOracleFeedById(_)
        | SingularQueryBox::FindDomainEndorsementPolicy(_)
        | SingularQueryBox::FindDomainCommittee(_)
        | SingularQueryBox::FindSorafsProviderOwner(_)
        | SingularQueryBox::FindSorafsPinManifest(_)
        | SingularQueryBox::FindSorafsPinManifests(_)
        | SingularQueryBox::FindSorafsOrderbookPolicy(_)
        | SingularQueryBox::FindSorafsOrderbookOrderById(_)
        | SingularQueryBox::FindSorafsOrderbookCancellationByOrderId(_)
        | SingularQueryBox::FindSorafsOrderbookReceiptById(_)
        | SingularQueryBox::FindSorafsOrderbookTradeById(_)
        | SingularQueryBox::FindSorafsOrderbookChannelById(_)
        | SingularQueryBox::FindSorafsOrderbookStatus(_)
        | SingularQueryBox::FindSorafsOrderbookOrders(_)
        | SingularQueryBox::FindSorafsOrderbookReceipts(_)
        | SingularQueryBox::FindSorafsOrderbookTrades(_)
        | SingularQueryBox::FindSorafsOrderbookChannels(_)
        | SingularQueryBox::FindSorafsOrderbookEvents(_)
        | SingularQueryBox::FindSorafsReservePolicy(_)
        | SingularQueryBox::FindSorafsReserveProviderById(_)
        | SingularQueryBox::FindSorafsReserveMovementById(_)
        | SingularQueryBox::FindSorafsReserveAppealById(_)
        | SingularQueryBox::FindSorafsReserveProviders(_)
        | SingularQueryBox::FindSorafsReserveMovements(_)
        | SingularQueryBox::FindSorafsReserveAppeals(_)
        | SingularQueryBox::FindSorafsReserveEvents(_)
        | SingularQueryBox::FindSorafsPopIssuerPolicy(_)
        | SingularQueryBox::FindSorafsPopCredentialCommitmentByDigest(_)
        | SingularQueryBox::FindSorafsPopCommitmentRootByVersion(_)
        | SingularQueryBox::FindSorafsPopRevocationPublicationByVersion(_)
        | SingularQueryBox::FindSorafsPopRevocationByNonceCommitment(_)
        | SingularQueryBox::FindSorafsPopAuditDigestBySequence(_)
        | SingularQueryBox::FindSorafsPopRegistryStatus(_)
        | SingularQueryBox::FindSorafsRepairTask(_)
        | SingularQueryBox::FindSorafsRepairTasks(_)
        | SingularQueryBox::FindSorafsRepairStatus(_)
        | SingularQueryBox::FindSorafsRepairEvents(_)
        | SingularQueryBox::FindSorafsProofOutcome(_)
        | SingularQueryBox::FindSorafsProofOutcomeEvents(_)
        | SingularQueryBox::FindSorafsReputationJournalAuthorityPolicy(_)
        | SingularQueryBox::FindSorafsReputationJournalEventBySourceId(_)
        | SingularQueryBox::FindSorafsReputationJournalEvents(_)
        | SingularQueryBox::FindSorafsModerationPolicy(_)
        | SingularQueryBox::FindSorafsModerationAppeal(_)
        | SingularQueryBox::FindSorafsModerationJurorEligibility(_)
        | SingularQueryBox::FindSorafsModerationCase(_)
        | SingularQueryBox::FindSorafsModerationCommit(_)
        | SingularQueryBox::FindSorafsModerationReveal(_)
        | SingularQueryBox::FindSorafsModerationChallenge(_)
        | SingularQueryBox::FindSorafsModerationOutcome(_)
        | SingularQueryBox::FindSorafsModerationNoShow(_)
        | SingularQueryBox::FindSorafsModerationStatus(_)
        | SingularQueryBox::FindSorafsModerationSnapshot(_)
        | SingularQueryBox::FindSorafsModerationEvents(_)
        | SingularQueryBox::FindDataspaceNameOwnerById(_)
        | SingularQueryBox::FindMusubiExactPackageV1(_)
        | SingularQueryBox::FindMusubiExactReleaseV1(_)
        | SingularQueryBox::FindMusubiProviderBundleAttestationV1(_)
        | SingularQueryBox::FindMusubiResolverIndexV1(_)
        | SingularQueryBox::FindMusubiVersionsV1(_)
        | SingularQueryBox::FindMusubiMaintainersV1(_)
        | SingularQueryBox::FindMusubiArchiveLocationsV1(_)
        | SingularQueryBox::FindMusubiArchiveRetentionV1(_)
        | SingularQueryBox::FindMusubiAliasV1(_)
        | SingularQueryBox::FindMusubiAliasHistoryV1(_)
        | SingularQueryBox::FindMusubiOrderedPrefixV1(_)
        | SingularQueryBox::FindAccountByAlias(_)
        | SingularQueryBox::FindDomainById(_) => NativeQueryAccess::Registered,
        SingularQueryBox::FindProofRecordById(_)
        | SingularQueryBox::FindAssetEscrowById(_)
        | SingularQueryBox::FindBondById(_)
        | SingularQueryBox::FindTriggerById(_)
        | SingularQueryBox::FindTwitterBindingByHash(_)
        | SingularQueryBox::FindOracleDisputeById(_)
        | SingularQueryBox::FindOracleChangeById(_)
        | SingularQueryBox::FindOracleProviderStatsByKey(_)
        | SingularQueryBox::FindLatestDefiOracleAttestation(_)
        | SingularQueryBox::FindDomainEndorsements(_)
        | SingularQueryBox::FindDaPinIntentByTicket(_)
        | SingularQueryBox::FindDaPinIntentByManifest(_)
        | SingularQueryBox::FindDaPinIntentByAlias(_)
        | SingularQueryBox::FindDaPinIntentByLaneEpochSequence(_)
        | SingularQueryBox::FindFxCorridorPolicyRegistry(_)
        | SingularQueryBox::FindFxCorridorPolicyById(_)
        | SingularQueryBox::FindSorafsCitizenBondBySerialCommitment(_)
        | SingularQueryBox::FindSorafsCitizenBondSnapshot(_)
        | SingularQueryBox::FindNftById(_)
        | SingularQueryBox::FindNftSaleOfferById(_) => NativeQueryAccess::AllLedger,
    }
}
#[allow(clippy::too_many_lines)]
fn native_iterable_query_access(
    query: &QueryWithParams,
) -> Result<NativeQueryAccess, ValidationFail> {
    if query.item == QueryItemKind::AssetEscrowScopeOriginalV1 {
        let query = decode_native_iterable_payload_exact::<
            data_model_query::asset_escrow_scope::FindAssetEscrowsByAssetDefinitionV1,
        >(&query.query_payload)
        .ok_or_else(invalid_native_iterable_query)?;
        return Ok(NativeQueryAccess::AssetEscrowOwner(
            query.asset_definition_id,
            query.expected_incarnation,
        ));
    }
    if query.item == QueryItemKind::AssetUidPolicyViewV1 {
        let query = decode_native_iterable_payload_exact::<
            data_model_query::asset_uid::FindAssetUidPoliciesV1,
        >(&query.query_payload)
        .ok_or_else(invalid_native_iterable_query)?;
        return Ok(NativeQueryAccess::AssetUidOwner(query.asset_definition_id));
    }
    if query.item == QueryItemKind::AssetFiContextSnapshotV1 {
        let query = decode_native_iterable_payload_exact::<
            data_model_query::asset_fi_context::FindAssetFiContextV1,
        >(&query.query_payload)
        .ok_or_else(invalid_native_iterable_query)?;
        query
            .expected_incarnation
            .validate()
            .map_err(|_| invalid_native_iterable_query())?;
        if query.expected_uid == [0; 32] {
            return Err(invalid_native_iterable_query());
        }
        let selected =
            crate::smartcontracts::isi::asset_fi_context::target_selection(&query.target)
                .map_err(|_| invalid_native_iterable_query())?;
        return Ok(NativeQueryAccess::AssetUidOwner(
            selected.asset_definition_id.clone(),
        ));
    }
    if query.item == QueryItemKind::AssetFiTransferReceiptV1 {
        let query = decode_native_iterable_payload_exact::<
            data_model_query::asset_fi::FindAssetFiTransferReceiptV1,
        >(&query.query_payload)
        .ok_or_else(invalid_native_iterable_query)?;
        query
            .expected_incarnation
            .validate()
            .map_err(|_| invalid_native_iterable_query())?;
        return Ok(NativeQueryAccess::AssetFiOwner(
            query.asset_definition_id,
            query.account_id,
        ));
    }
    if query.item == QueryItemKind::AssetFiRelationshipViewV1 {
        let query = decode_native_iterable_payload_exact::<
            data_model_query::asset_fi::FindAssetFiLinkV1,
        >(&query.query_payload)
        .ok_or_else(invalid_native_iterable_query)?;
        return Ok(NativeQueryAccess::AssetFiOwner(
            query.asset_definition_id,
            query.account_id,
        ));
    }
    if query.item == QueryItemKind::AssetUidEnrollmentViewV1 {
        let query = decode_native_iterable_payload_exact::<
            data_model_query::asset_uid_enrollment::FindAssetUidEnrollmentV1,
        >(&query.query_payload)
        .ok_or_else(invalid_native_iterable_query)?;
        return Ok(NativeQueryAccess::AssetUidOwner(query.asset_definition_id));
    }
    macro_rules! payload_for {
        ($item:ty, $kind:ident) => {{ (query.item == QueryItemKind::$kind).then_some(query.query_payload.as_slice()) }};
    }
    macro_rules! any_exact {
        ($payload:expr; $($query_ty:path),+ $(,)?) => {
            false $(|| decode_native_iterable_payload_exact::<$query_ty>($payload).is_some())+
        };
    }
    if let Some(payload) = payload_for!(iroha_data_model::role::Role, Role) {
        if any_exact!(payload; data_model_query::role::prelude::FindRoles) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(RoleId, RoleId) {
        if any_exact!(payload; data_model_query::role::prelude::FindRoleIds) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        if let Some(query) = decode_native_iterable_payload_exact::<
            data_model_query::role::prelude::FindRolesByAccountId,
        >(payload)
        {
            return Ok(NativeQueryAccess::Account(query.account_id().clone()));
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(Permission, Permission) {
        if let Some(query) = decode_native_iterable_payload_exact::<
            data_model_query::permission::prelude::FindPermissionsByAccountId,
        >(payload)
        {
            return Ok(NativeQueryAccess::EnrollmentAccount(
                query.account_id().clone(),
            ));
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(Account, Account) {
        if any_exact!(
            payload;
            data_model_query::account::prelude::FindAccounts,
            data_model_query::account::prelude::FindAccountsWithAsset,
        ) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(AccountId, AccountId) {
        if any_exact!(payload; data_model_query::account::prelude::FindAccountIds) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(iroha_data_model::asset::value::Asset, Asset) {
        if any_exact!(payload; data_model_query::asset::prelude::FindAssets) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        if let Some(query) = decode_native_iterable_payload_exact::<
            data_model_query::asset::prelude::FindAssetsByAccountId,
        >(payload)
        {
            return Ok(NativeQueryAccess::Account(query.account_id().clone()));
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(
        iroha_data_model::asset::definition::AssetDefinition,
        AssetDefinition
    ) {
        if any_exact!(payload; data_model_query::asset::prelude::FindAssetDefinitions) {
            // Listing every definition would disclose privately homed definitions.
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(iroha_data_model::repo::RepoAgreement, RepoAgreement) {
        if any_exact!(payload; data_model_query::repo::FindRepoAgreements) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    // Bond records carry every investor's record-date holdings.
    if let Some(payload) = payload_for!(iroha_data_model::bond::BondRecord, BondRecord) {
        if any_exact!(payload; data_model_query::bond::prelude::FindBonds) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(iroha_data_model::nft::Nft, Nft) {
        if any_exact!(payload; data_model_query::nft::prelude::FindNfts) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        if let Some(query) = decode_native_iterable_payload_exact::<
            data_model_query::nft::prelude::FindNftsByAccountId,
        >(payload)
        {
            return Ok(NativeQueryAccess::Account(query.account_id().clone()));
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(iroha_data_model::rwa::Rwa, Rwa) {
        if any_exact!(payload; data_model_query::rwa::prelude::FindRwas) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(iroha_data_model::domain::Domain, Domain) {
        if any_exact!(payload; data_model_query::domain::prelude::FindDomains) {
            return Ok(NativeQueryAccess::Registered);
        }
        if let Some(query) = decode_native_iterable_payload_exact::<
            data_model_query::domain::prelude::FindDomainsByAccountId,
        >(payload)
        {
            return Ok(NativeQueryAccess::Account(query.account_id().clone()));
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(iroha_model_base::peer::PeerId, PeerId) {
        if any_exact!(payload; data_model_query::peer::prelude::FindPeers) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(TriggerId, TriggerId) {
        if any_exact!(payload; data_model_query::trigger::prelude::FindActiveTriggerIds) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(Trigger, Trigger) {
        if any_exact!(payload; data_model_query::trigger::prelude::FindTriggers) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) =
        payload_for!(data_model_query::CommittedTransaction, CommittedTransaction)
    {
        if any_exact!(
            payload;
            data_model_query::transaction::prelude::FindTransactions
        ) {
            if let Ok(Some(authority)) = query.exact_transaction_read_authority_with_limits(
                norito::canonical_decode_limits(query.predicate_bytes.len()),
            ) {
                return Ok(NativeQueryAccess::Account(authority));
            }
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(iroha_data_model::block::SignedBlock, SignedBlock) {
        if any_exact!(payload; data_model_query::block::prelude::FindBlocks) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(BlockHeader, BlockHeader) {
        if any_exact!(payload; data_model_query::block::prelude::FindBlockHeaders) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(iroha_data_model::proof::ProofRecord, ProofRecord) {
        if any_exact!(
            payload;
            data_model_query::proof::prelude::FindProofRecords,
            data_model_query::proof::prelude::FindProofRecordsByBackend,
            data_model_query::proof::prelude::FindProofRecordsByStatus,
        ) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(
        iroha_data_model::nexus::FeeSponsorProgram,
        FeeSponsorProgram
    ) {
        if any_exact!(payload; data_model_query::nexus::prelude::FindFeeSponsorPrograms) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        if let Some(query) = decode_native_iterable_payload_exact::<
            data_model_query::nexus::prelude::FindFeeSponsorProgramsBySponsor,
        >(payload)
        {
            return Ok(NativeQueryAccess::Account(query.sponsor().clone()));
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(
        iroha_data_model::nexus::FeeSponsorProgramId,
        FeeSponsorProgramId
    ) {
        if any_exact!(payload; data_model_query::nexus::prelude::FindFeeSponsorProgramIds) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(iroha_data_model::oracle::FeedConfig, OracleFeedConfig) {
        if any_exact!(payload; data_model_query::oracle::prelude::FindOracleFeeds) {
            return Ok(NativeQueryAccess::Registered);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(
        iroha_data_model::events::data::oracle::FeedEventRecord,
        OracleFeedEventRecord
    ) {
        if any_exact!(payload; data_model_query::oracle::prelude::FindOracleHistoryByFeedId) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(
        iroha_data_model::oracle::OracleProviderStatsRecord,
        OracleProviderStatsRecord
    ) {
        if any_exact!(payload; data_model_query::oracle::prelude::FindOracleProviderStatsByFeedId) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(iroha_data_model::oracle::OracleDispute, OracleDispute) {
        if any_exact!(
            payload;
            data_model_query::oracle::prelude::FindOracleDisputes,
            data_model_query::oracle::prelude::FindOracleDisputesByFeedId,
        ) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(
        iroha_data_model::oracle::OracleChangeProposal,
        OracleChangeProposal
    ) {
        if any_exact!(payload; data_model_query::oracle::prelude::FindOracleChanges) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(
        iroha_data_model::oracle::TwitterBindingRecord,
        TwitterBindingRecord
    ) {
        if any_exact!(payload; data_model_query::oracle::prelude::FindTwitterBindingsByUaid) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(
        iroha_data_model::oracle::DefiOracleAttestation,
        DefiOracleAttestation
    ) {
        if any_exact!(payload; data_model_query::oracle::prelude::FindDefiOracleAttestationsByKey) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(
        iroha_data_model::escrow::AssetEscrowRecord,
        AssetEscrowRecord
    ) {
        if any_exact!(payload; data_model_query::escrow::prelude::FindAssetEscrows) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(
        iroha_data_model::escrow::AssetEscrowRecord,
        AssetEscrowsBySeller
    ) {
        if let Some(query) = decode_native_iterable_payload_exact::<
            data_model_query::escrow::prelude::FindAssetEscrowsBySeller,
        >(payload)
        {
            return Ok(NativeQueryAccess::Account(query.seller.clone()));
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(
        iroha_data_model::escrow::AssetEscrowRecord,
        AssetEscrowsByBuyer
    ) {
        if let Some(query) = decode_native_iterable_payload_exact::<
            data_model_query::escrow::prelude::FindAssetEscrowsByBuyer,
        >(payload)
        {
            return Ok(NativeQueryAccess::Account(query.buyer.clone()));
        }
        return Err(invalid_native_iterable_query());
    }
    if let Some(payload) = payload_for!(
        iroha_data_model::escrow::AssetEscrowRecord,
        AssetEscrowsByStatus
    ) {
        if any_exact!(payload; data_model_query::escrow::prelude::FindAssetEscrowsByStatus) {
            return Ok(NativeQueryAccess::AllLedger);
        }
        return Err(invalid_native_iterable_query());
    }
    Err(invalid_native_iterable_query())
}
fn invalid_native_iterable_query() -> ValidationFail {
    ValidationFail::NotPermitted(
        "iterable query is malformed or is not part of the native authorization matrix".to_owned(),
    )
}
/// Return whether one of the two exact native enrollment observations is permitted by a direct
/// domain-scoped DPN capability and the target's current UAID/SNS binding.
///
/// This narrow predicate is also used by Torii route admission. It never authorizes balances,
/// roster queries, alias lists, roles, or arbitrary reads from the same restricted dataspace.
///
/// # Errors
/// Returns native state or permission decoding errors without treating malformed state as absent.
pub fn can_observe_dpn_enrollment_query(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    query: &QueryRequest,
    now_ms: u64,
) -> Result<bool, crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
    let access = match query {
        QueryRequest::Singular(query) => native_singular_query_access(query),
        QueryRequest::Start(query) => native_iterable_query_access(query)?,
        QueryRequest::Continue(_) => return Ok(false),
    };
    match access {
        NativeQueryAccess::EnrollmentAccount(account) => {
            authority_can_enroll_dpn_account(world, authority, &account, now_ms)
        }
        _ => Ok(false),
    }
}
fn validate_builtin_native_query_permission(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    query: &QueryRequest,
    now_ms: u64,
) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
    world.account(authority).map_err(|_| {
        ValidationFail::NotPermitted(format!(
            "query authority `{authority}` is not a registered account"
        ))
    })?;
    let access = match query {
        QueryRequest::Singular(query) => native_singular_query_access(query),
        QueryRequest::Start(query) => native_iterable_query_access(query)?,
        // The store-aware continuation corridor revalidates the archived Start request before
        // validating and advancing this cursor. Raw public validation cannot construct Continue.
        QueryRequest::Continue(_) => return Ok(()),
    };
    let has_global = || authority_has_native_global_read_permission(world, authority);
    let enrollment_query = matches!(&access, NativeQueryAccess::EnrollmentAccount(_));
    match access {
        NativeQueryAccess::Registered => Ok(()),
        NativeQueryAccess::AssetFiOwner(definition, account) => {
            let asset = world.asset_definition(&definition).map_err(|_| {
                ValidationFail::NotPermitted("FI relationship asset is unavailable".into())
            })?;
            if authority == &account || asset.owned_by() == authority {
                Ok(())
            } else {
                Err(ValidationFail::NotPermitted(
                    "only the exact wallet or asset owner may query FI relationships".into(),
                )
                .into())
            }
        }
        NativeQueryAccess::AssetEscrowOwner(definition, incarnation) => {
            let asset = world.asset_definition(&definition).map_err(|_| {
                ValidationFail::NotPermitted("escrow scope asset is unavailable".into())
            })?;
            if asset.owned_by() == authority
                && world.axt_asset_incarnations().get(&definition) == Some(&incarnation)
            {
                Ok(())
            } else {
                Err(ValidationFail::NotPermitted(
                    "escrow scope requires the exact current asset owner and live incarnation"
                        .into(),
                )
                .into())
            }
        }
        NativeQueryAccess::AssetUidOwner(definition) => {
            let asset = world.asset_definitions().get(&definition).ok_or_else(|| {
                ValidationFail::NotPermitted(
                    "UID policy asset is not available to the requester".into(),
                )
            })?;
            if asset.owned_by() == authority {
                Ok(())
            } else {
                Err(ValidationFail::NotPermitted(
                    "only the exact asset-definition owner may query UID policies".into(),
                )
                .into())
            }
        }
        NativeQueryAccess::AllLedger => has_global()?
            .then_some(())
            .ok_or_else(|| {
                ValidationFail::NotPermitted(
                    "CanReadAllLedgerData permission is required for this query".to_owned(),
                )
            })
            .map_err(Into::into),
        NativeQueryAccess::Account(account) | NativeQueryAccess::EnrollmentAccount(account) => {
            if account == *authority || has_global()? {
                return Ok(());
            }
            if enrollment_query
                && authority_can_enroll_dpn_account(world, authority, &account, now_ms)?
            {
                return Ok(());
            }
            authority_has_native_account_read_permission(world, authority, &account)?
                .then_some(())
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "exact CanReadAccountData permission is required to read account `{account}`"
                    ))
                }).map_err(Into::into)
        }
    }
}
/// Borrow the exact parameterless grant without allocating a JSON token under a read budget.
fn authority_has_native_global_read_permission(
    world: &impl WorldReadOnly,
    authority: &AccountId,
) -> Result<bool, ValidationFail> {
    let exact = |permission: &Permission| {
        permission.name() == "CanReadAllLedgerData" && permission.payload().get().as_str() == "null"
    };
    let permissions = world.account_permissions_iter(authority).map_err(|error| {
        ValidationFail::InstructionFailed(InstructionExecutionError::Find(error))
    })?;
    Ok(permissions.into_iter().any(exact)
        || world.account_roles_iter(authority).any(|role_id| {
            world
                .roles()
                .get(role_id)
                .is_some_and(|role| role.permissions().any(exact))
        }))
}

/// Read the existing exact grant. `Json` has canonical lexical form; the strict
/// one-field adapter and AccountId's canonical-I105 decoder preserve exact token semantics.
/// Unlike constructing a fresh token, inherited decoder refusal remains fallible here.
fn authority_has_native_account_read_permission(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    target: &AccountId,
) -> Result<bool, crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
    #[derive(crate::json_macros::JsonDeserialize)]
    #[norito(deny_unknown_fields)]
    struct AccountReadGrant {
        account: AccountId,
    }
    authority_has_borrowed_permission(world, authority, "CanReadAccountData", |permission| {
        read_permission_payload::<AccountReadGrant>(permission)
            .map(|grant| grant.is_some_and(|grant| grant.account == *target))
    })
}

/// Decode only an existing exact-name grant; incomplete local inspection is not a denial.
fn read_permission_payload<T: norito::json::JsonDeserialize>(
    permission: &Permission,
) -> Result<Option<T>, crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
    match norito::json::from_str(permission.payload().get()) {
        Ok(grant) => Ok(Some(grant)),
        Err(
            norito::json::Error::DecodeResourceLimit
            | norito::json::Error::DecodeResource(
                norito::core::DecodeResourceError::ArchiveLengthExceeded { .. }
                | norito::core::DecodeResourceError::SequenceLengthExceeded { .. }
                | norito::core::DecodeResourceError::FieldLengthExceeded { .. }
                | norito::core::DecodeResourceError::TotalElementsExceeded { .. }
                | norito::core::DecodeResourceError::TotalAllocationExceeded { .. },
            ),
        ) if !cfg!(all(test, sumeragi_core_mutation = "HC30")) => {
            Err(crate::execution_attempt::ExecutionAttemptError::Deferred(
                ivm::error::ExecutionDeferral::ActiveMemoryCapacity.into(),
            ))
        }
        Err(
            norito::json::Error::AllocationFailed
            | norito::json::Error::DecodeResource(
                norito::core::DecodeResourceError::AllocationFailed { .. },
            ),
        ) if !cfg!(all(test, sumeragi_core_mutation = "HC30")) => {
            Err(crate::execution_attempt::ExecutionAttemptError::Deferred(
                ivm::error::ExecutionDeferral::AllocationUnavailable.into(),
            ))
        }
        Err(_) => Ok(None),
    }
}

/// Preserve account existence, direct grants and assigned-role authority while inspecting
/// canonical stored payloads without constructing a second permission token.
fn authority_has_borrowed_permission(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    name: &str,
    inspect: impl Fn(
        &Permission,
    )
        -> Result<bool, crate::execution_attempt::ExecutionAttemptError<ValidationFail>>,
) -> Result<bool, crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
    let permissions = world.account_permissions_iter(authority).map_err(|error| {
        ValidationFail::InstructionFailed(InstructionExecutionError::Find(error))
    })?;
    for permission in permissions {
        if permission.name() == name && inspect(permission)? {
            return Ok(true);
        }
    }
    for role_id in world.account_roles_iter(authority) {
        if let Some(role) = world.roles().get(role_id) {
            for permission in role.permissions() {
                if permission.name() == name && inspect(permission)? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn validate_builtin_account_alias_query_permission(
    world: &impl WorldReadOnly,
    latest_block: Option<&BlockHeader>,
    authority: &AccountId,
    query: &QueryRequest,
) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
    let deny = || {
        ValidationFail::NotPermitted(
            "exact CanResolveAccountAlias permission is required for this alias query".to_owned(),
        )
    };
    let require_alias = |alias: &iroha_data_model::account::rekey::AccountAlias| -> Result<
        (),
        crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
    > {
        crate::alias::authority_can_resolve_account_alias(world, authority, alias)
            .map_err(|error| {
                error.into_attempt_error(|error| ValidationFail::InternalError(error.to_string()))
            })?
            .then_some(())
            .ok_or_else(|| deny().into())
    };
    let QueryRequest::Singular(query) = query else {
        return Ok(());
    };
    match query {
        SingularQueryBox::FindAccountByAlias(query) => {
            require_alias(query.alias()).map_err(Into::into)
        }
        SingularQueryBox::FindAccountRecoveryPolicyByAlias(query) => {
            require_alias(query.alias()).map_err(Into::into)
        }
        SingularQueryBox::FindAccountRecoveryRequestByAlias(query) => {
            require_alias(query.alias()).map_err(Into::into)
        }
        SingularQueryBox::FindAliasesByAccountId(query) => {
            let catalog = world.dataspace_catalog();
            let dataspace_filter = query
                .dataspace()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|dataspace| {
                    catalog
                        .by_alias(dataspace)
                        .map(|entry| entry.id)
                        .ok_or_else(&deny)
                })
                .transpose()?;
            let domain_filter = query
                .domain()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|domain| {
                    domain
                        .parse::<iroha_data_model::account::rekey::AccountAliasDomain>()
                        .map_err(|_| deny())
                })
                .transpose()?;
            if domain_filter.is_some() && dataspace_filter.is_none() {
                return Err(deny().into());
            }
            if let Some(dataspace) = dataspace_filter {
                let probe = iroha_data_model::account::rekey::AccountAlias::new(
                    "lookup".parse().expect("static alias label"),
                    domain_filter.clone(),
                    dataspace,
                );
                require_alias(&probe)?;
            }
            let now_ms = latest_block.map_or(0, |header| {
                u64::try_from(header.creation_time().as_millis()).unwrap_or(u64::MAX)
            });
            let labels = world
                .account_aliases_by_account()
                .get(query.account_id())
                .cloned()
                .unwrap_or_default();
            for alias in labels {
                let resolved =
                    crate::sns::resolve_active_account_alias(world, catalog, &alias, now_ms)
                        .map_err(|error| {
                            error.into_attempt_error(|error| {
                                ValidationFail::InternalError(error.to_string())
                            })
                        })?;
                if dataspace_filter.is_some_and(|dataspace| alias.dataspace != dataspace)
                    || domain_filter
                        .as_ref()
                        .is_some_and(|domain| alias.domain.as_ref() != Some(domain))
                    || resolved.as_ref() != Some(query.account_id())
                {
                    continue;
                }
                require_alias(&alias)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
fn validate_builtin_subsystem_query_permission(
    world: &impl WorldReadOnly,
    latest_block: Option<&BlockHeader>,
    authority: &AccountId,
    query: &QueryRequest,
) -> Result<(), ValidationFail> {
    // Preserve the stricter SoraFS confidentiality gates independently of the
    // mandatory native account/global read policy and the pluggable executor.
    if latest_block.is_none_or(BlockHeader::is_genesis) {
        return Ok(());
    }
    let QueryRequest::Singular(query) = query else {
        return Ok(());
    };
    match query {
        SingularQueryBox::FindSorafsOrderbookPolicy(_)
        | SingularQueryBox::FindSorafsOrderbookOrderById(_)
        | SingularQueryBox::FindSorafsOrderbookCancellationByOrderId(_)
        | SingularQueryBox::FindSorafsOrderbookReceiptById(_)
        | SingularQueryBox::FindSorafsOrderbookTradeById(_)
        | SingularQueryBox::FindSorafsOrderbookChannelById(_)
        | SingularQueryBox::FindSorafsOrderbookStatus(_)
        | SingularQueryBox::FindSorafsOrderbookOrders(_)
        | SingularQueryBox::FindSorafsOrderbookReceipts(_)
        | SingularQueryBox::FindSorafsOrderbookTrades(_)
        | SingularQueryBox::FindSorafsOrderbookChannels(_)
        | SingularQueryBox::FindSorafsOrderbookEvents(_) => {
            let can_set_pricing: Permission =
                executor_permission::sorafs::CanSetSorafsPricing.into();
            // Provider-scoped completion authority grants no global orderbook inventory.
            if authority_has_permission(world, authority, &can_set_pricing)? {
                Ok(())
            } else {
                Err(ValidationFail::NotPermitted(
                    "Can't read authoritative SoraFS orderbook state".to_owned(),
                ))
            }
        }
        SingularQueryBox::FindSorafsReservePolicy(_)
        | SingularQueryBox::FindSorafsReserveProviderById(_)
        | SingularQueryBox::FindSorafsReserveMovementById(_)
        | SingularQueryBox::FindSorafsReserveAppealById(_)
        | SingularQueryBox::FindSorafsReserveProviders(_)
        | SingularQueryBox::FindSorafsReserveMovements(_)
        | SingularQueryBox::FindSorafsReserveAppeals(_)
        | SingularQueryBox::FindSorafsReserveEvents(_) => {
            let can_set_reserve_policy: Permission =
                executor_permission::sorafs::CanSetSorafsReservePolicy.into();
            if authority_has_permission(world, authority, &can_set_reserve_policy)? {
                Ok(())
            } else {
                Err(ValidationFail::NotPermitted(
                    "Can't read authoritative SoraFS reserve state".to_owned(),
                ))
            }
        }
        SingularQueryBox::FindSorafsReputationJournalAuthorityPolicy(_) => {
            let can_manage_reputation_policy: Permission =
                executor_permission::sorafs::CanManageSorafsReputationJournalPolicy.into();
            let can_record_reputation: Permission =
                executor_permission::sorafs::CanRecordSorafsReputationJournal.into();
            let can_resolve_dispute: Permission =
                executor_permission::sorafs::CanResolveSorafsCapacityDispute.into();
            if authority_has_permission(world, authority, &can_manage_reputation_policy)?
                || authority_has_permission(world, authority, &can_record_reputation)?
                || authority_has_permission(world, authority, &can_resolve_dispute)?
            {
                Ok(())
            } else {
                Err(ValidationFail::NotPermitted(
                    "Can't read the active authoritative SoraFS reputation-journal authority policy"
                        .to_owned(),
                ))
            }
        }
        SingularQueryBox::FindSorafsModerationJurorEligibility(query) => {
            let can_manage_moderation: Permission =
                executor_permission::sorafs::CanManageSorafsModeration.into();
            if authority == &query.juror
                || authority_has_permission(world, authority, &can_manage_moderation)?
            {
                Ok(())
            } else {
                Err(ValidationFail::NotPermitted(
                    "Can't read another juror's moderation PoP eligibility record".to_owned(),
                ))
            }
        }
        SingularQueryBox::FindSorafsModerationSnapshot(_) => {
            let can_manage_moderation: Permission =
                executor_permission::sorafs::CanManageSorafsModeration.into();
            if authority_has_permission(world, authority, &can_manage_moderation)? {
                Ok(())
            } else {
                Err(ValidationFail::NotPermitted(
                    "Can't read the complete authoritative SoraFS moderation snapshot".to_owned(),
                ))
            }
        }
        _ => Ok(()),
    }
}
fn encode_executor_input<T: Encode>(payload: &T) -> Result<Vec<u8>, ValidationFail> {
    let payload_bytes = payload.encode();
    let total_len = EXECUTOR_LENGTH_PREFIX_BYTES
        .checked_add(payload_bytes.len())
        .and_then(|len| u64::try_from(len).ok())
        .ok_or_else(|| {
            ValidationFail::InternalError(
                "executor input length exceeds the fixed u64 framing domain".to_owned(),
            )
        })?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(total_len).expect("executor input length originated as usize"),
    );
    bytes.extend_from_slice(&total_len.to_le_bytes());
    bytes.extend_from_slice(&payload_bytes);
    Ok(bytes)
}
fn executor_output_payload<'ivm>(
    ivm: &'ivm IVM,
    ret_ptr: u64,
    output_kind: &str,
) -> Result<&'ivm [u8], crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
    let returned_len = ivm.memory.load_u64(ret_ptr).map_err(|error| {
        crate::execution_attempt::vm_attempt_error(error, |error| {
            ValidationFail::InternalError(format!(
                "executor {output_kind} length prefix is not readable: {error}"
            ))
        })
    })?;
    if returned_len < EXECUTOR_LENGTH_PREFIX_BYTES_U64 {
        return Err(ValidationFail::InternalError(format!(
            "executor {output_kind} is shorter than its fixed u64 length prefix"
        ))
        .into());
    }
    if returned_len > MAX_EXECUTOR_OUTPUT_BYTES {
        return Err(ValidationFail::InternalError(format!(
            "executor {output_kind} length exceeds the {MAX_EXECUTOR_OUTPUT_BYTES}-byte limit"
        ))
        .into());
    }
    let framed = ivm
        .memory
        .load_region(ret_ptr, returned_len)
        .map_err(|error| {
            crate::execution_attempt::vm_attempt_error(error, |error| {
                ValidationFail::InternalError(format!(
                    "executor {output_kind} is not fully readable: {error}"
                ))
            })
        })?;
    Ok(&framed[EXECUTOR_LENGTH_PREFIX_BYTES..])
}

#[cfg(test)]
pub(crate) fn build_program_from_encoded_result(result_bytes: &[u8]) -> Vec<u8> {
    ivm::prebuilt_fixtures::build_encoded_result_program(result_bytes)
}
#[cfg(test)]
mod encoded_result_program_tests {
    use super::*;
    #[test]
    fn encoded_result_program_is_admitted_and_copies_exact_bytes() {
        let result = [0xde, 0xad, 0xbe, 0xef, 0x42];
        let program = build_program_from_encoded_result(&result);
        let parsed = ivm::ProgramMetadata::parse(&program).expect("program metadata parses");
        assert!(
            parsed
                .literal_section
                .is_some_and(|literals| literals.count > 0),
            "encoded bytes must live in authenticated typed literals"
        );
        let mut vm = ivm::IVM::new(1_000_000);
        vm.load_program(&program)
            .expect("encoded-result program passes strict admission");
        vm.set_register(10, ivm::Memory::OUTPUT_START);
        vm.run().expect("encoded-result program runs");
        let output = vm.read_output_used();
        assert_eq!(
            u64::from_le_bytes(output[..8].try_into().expect("fixed u64 prefix")),
            8 + u64::try_from(result.len()).expect("bounded result length")
        );
        assert_eq!(&output[8..8 + result.len()], result);
    }
    #[test]
    fn executor_input_framing_uses_a_fixed_u64_prefix() {
        let payload = 42_u64;
        let payload_bytes = payload.encode();
        let framed = encode_executor_input(&payload).expect("frame executor input");
        assert_eq!(EXECUTOR_LENGTH_PREFIX_BYTES, 8);
        assert_eq!(
            u64::from_le_bytes(framed[..8].try_into().expect("fixed u64 prefix")),
            u64::try_from(8 + payload_bytes.len()).expect("bounded framed length")
        );
        assert_eq!(&framed[8..], payload_bytes);
    }
}
#[cfg(test)]
fn generate_verdict_program(verdict: &Result<(), ValidationFail>) -> Vec<u8> {
    let verdict_bytes = verdict.encode();
    build_program_from_encoded_result(&verdict_bytes)
}
/// Build a user executor that rejects every validation request with a stable message.
#[cfg(test)]
pub(crate) fn denying_executor_for_testing(message: &str) -> Executor {
    let verdict = Err(ValidationFail::NotPermitted(message.to_owned()));
    let bytecode = generate_verdict_program(&verdict);
    let raw = data_model_executor::Executor::new(
        iroha_data_model::transaction::executable::IvmBytecode::from_compiled(bytecode),
    );
    Executor::UserProvided(LoadedExecutor::load(raw).expect("load deny-all test executor"))
}
/// Build an installed test executor whose actual VM returns an accepting verdict.
/// This fixture grants no production permission or native proof authority.
#[cfg(test)]
pub(crate) fn allowing_executor_for_testing() -> Executor {
    let bytecode = generate_verdict_program(&Ok(()));
    let raw = data_model_executor::Executor::new(
        iroha_data_model::transaction::executable::IvmBytecode::from_compiled(bytecode),
    );
    Executor::UserProvided(LoadedExecutor::load(raw).expect("load accepting test executor"))
}
const SORA_V2_CLAIM_TX_HASH_METADATA_KEY: &str = "sora_v2_claim_tx_hash";
const SORA_NEXUS_CLAIM_RECIPIENT_METADATA_KEY: &str = "sora_nexus_claim_recipient";
#[cfg(test)]
/// Execute a single instruction in a detached overlay for delta regression tests.
///
/// This regression-test helper pre-applies side-effect-free instructions without
/// borrowing a live `StateBlock`. Unsupported instructions return `ValidationFail::InternalError`
/// to preserve the reference fallback boundary exercised by the tests.
#[allow(clippy::too_many_lines)]
pub(crate) fn execute_instruction_detached(
    authority: &AccountId,
    instruction: &iroha_data_model::isi::InstructionBox,
    delta: &mut crate::state::DetachedStateTransactionDelta,
) -> Result<(), ValidationFail> {
    use iroha_data_model::isi::{
        BurnBox, GrantBox, MintBox, RegisterBox, RemoveKeyValueBox, RevokeBox, SetKeyValueBox,
        TransferBox, UnregisterBox,
    };
    if mutates_contract_deployment_permission(instruction) {
        return Err(ValidationFail::InternalError(
            "detached: CanManageSmartContractCode permission mutation requires the sequential consensus gate"
                .to_owned(),
        ));
    }
    let any = instruction.as_any();
    // These mutations all depend on live ownership, permission, reserved-key, or
    // trigger state. A detached delta has no authoritative world view, so recording
    // them here would bypass the Initial executor's consensus authorization. Force
    // the caller onto the sequential path instead.
    if any.downcast_ref::<SetKeyValueBox>().is_some()
        || any.downcast_ref::<RemoveKeyValueBox>().is_some()
        || any.downcast_ref::<MintBox>().is_some()
        || any.downcast_ref::<BurnBox>().is_some()
        || any
            .downcast_ref::<iroha_data_model::isi::SetParameter>()
            .is_some()
        || any
            .downcast_ref::<iroha_data_model::isi::ExecuteTrigger>()
            .is_some()
    {
        return Err(ValidationFail::InternalError(
            "detached: live authorization requires sequential execution".to_owned(),
        ));
    }
    // Transfers
    if let Some(tb) = any.downcast_ref::<TransferBox>() {
        match tb {
            TransferBox::Asset(t) => {
                if t.source.account() != authority {
                    return Err(ValidationFail::InternalError(
                        "detached: delegated asset transfer requires sequential authorization"
                            .to_owned(),
                    ));
                }
                let src = t.source.clone();
                let qty = t.object.clone();
                delta.transfer_asset(src, t.destination.clone(), qty);
            }
            TransferBox::Domain(_) | TransferBox::AssetDefinition(_) | TransferBox::Nft(_) => {
                return Err(ValidationFail::InternalError(
                    "detached: ownership transfer requires sequential authorization".to_owned(),
                ));
            }
        }
        return Ok(());
    }
    // Registration and removal depend on live ownership and permission state.
    if any
        .downcast_ref::<iroha_data_model::isi::register::RegisterCommitteePeerWithPop>()
        .is_some()
    {
        return Err(ValidationFail::InternalError(
            "detached: registration requires sequential authorization".to_owned(),
        ));
    }
    if let Some(rb) = any.downcast_ref::<RegisterBox>() {
        match rb {
            RegisterBox::Peer(_) => {}
            RegisterBox::Domain(_)
            | RegisterBox::Account(_)
            | RegisterBox::AssetDefinition(_)
            | RegisterBox::Nft(_)
            | RegisterBox::Role(_)
            | RegisterBox::Trigger(_) => {}
        }
        return Err(ValidationFail::InternalError(
            "detached: registration requires sequential authorization".to_owned(),
        ));
    }
    if let Some(ub) = any.downcast_ref::<UnregisterBox>() {
        match ub {
            UnregisterBox::Peer(_) => {}
            UnregisterBox::Domain(_)
            | UnregisterBox::Account(_)
            | UnregisterBox::AssetDefinition(_)
            | UnregisterBox::Nft(_)
            | UnregisterBox::Role(_)
            | UnregisterBox::Trigger(_) => {}
        }
        return Err(ValidationFail::InternalError(
            "detached: removal requires sequential authorization".to_owned(),
        ));
    }
    // Permission and role mutation depends on live authority, ownership, and role state.
    // Never pre-apply it to a detached delta: force the sequential executor path so the
    // same consensus authorization is evaluated for every scheduling profile.
    if any.downcast_ref::<GrantBox>().is_some() || any.downcast_ref::<RevokeBox>().is_some() {
        return Err(ValidationFail::InternalError(
            "detached: permission and role mutation requires sequential authorization".to_owned(),
        ));
    }
    // Unknown instruction kind – signal fallback
    Err(ValidationFail::InternalError(
        "detached: unsupported instruction".to_owned(),
    ))
}
/// Executor that verifies that operation is valid and executes it.
///
/// Executing is done in order to verify dependent instructions in transaction.
/// Can be upgraded with [`Upgrade`](iroha_data_model::isi::Upgrade) instruction.
#[derive(Debug, Default, Clone)]
pub enum Executor {
    /// Initial executor with minimal built-in permission checks for critical instructions.
    #[default]
    Initial,
    /// User-provided executor with arbitrary logic.
    UserProvided(LoadedExecutor),
}
/// Migration outcome at the State-owning instruction boundary.
#[derive(Debug, thiserror::Error)]
pub enum ExecutorMigrationError {
    /// A completed migration error retains its original VM fields.
    #[error("executor migration rejected: {0}")]
    Rejected(#[source] VMError),
    /// The transaction retains the first local refusal. This notice owns no cause and cannot
    /// become a completed rejection; the outer State terminal must export the original once.
    #[error("local executor migration did not complete; original refusal retained in State")]
    LocalRefusalRetained,
}

/// Execution profile applied when running native ISIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstructionExecutionProfile {
    /// Full runtime behaviour (logging, telemetry, and policy hooks).
    #[default]
    Runtime,
    /// Lightweight execution for benchmarks/tests lacking a global logger.
    Bench,
}
impl JsonSerializeTrait for Executor {
    fn json_serialize(&self, out: &mut String) {
        let bytes =
            executor_norito::to_bytes(self).unwrap_or_else(|e| panic!("norito encode failed: {e}"));
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        out.push('{');
        json::write_json_string("norito", out);
        out.push(':');
        json::write_json_string(&encoded, out);
        out.push('}');
    }
}
impl JsonDeserializeTrait for Executor {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        parse_executor_value(value)
    }
}
fn parse_executor_value(value: json::Value) -> Result<Executor, json::Error> {
    match value {
        json::Value::Object(mut map) => {
            if let Some(inner) = map.remove("norito").or_else(|| map.remove("bytes")) {
                let bytes = decode_executor_bytes(inner, "norito")?;
                return executor_norito::from_bytes(&bytes).map_err(json::Error::Message);
            }
            if !map.is_empty() {
                for key in map.keys() {
                    trace!(target: "executor::deserialize", field = %key, "ignoring unknown executor field");
                }
            }
            Err(json::Error::Message(
                "invalid executor object: expected {\"norito\": ...}".into(),
            ))
        }
        json::Value::String(s) => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(s)
                .map_err(|e| json::Error::Message(e.to_string()))?;
            executor_norito::from_bytes(&bytes).map_err(json::Error::Message)
        }
        other => Err(json::Error::Message(format!(
            "invalid executor JSON: expected object or string, got {other:?}"
        ))),
    }
}
fn decode_executor_bytes(value: json::Value, context: &str) -> Result<Vec<u8>, json::Error> {
    match value {
        json::Value::String(s) => {
            base64::engine::general_purpose::STANDARD
                .decode(s)
                .map_err(|e| json::Error::InvalidField {
                    field: context.into(),
                    message: e.to_string(),
                })
        }
        json::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for v in arr {
                let byte = v
                    .as_u64()
                    .and_then(|byte| u8::try_from(byte).ok())
                    .ok_or_else(|| json::Error::InvalidField {
                        field: context.into(),
                        message: "expected byte in range 0..=255".into(),
                    })?;
                out.push(byte);
            }
            Ok(out)
        }
        other => Err(json::Error::InvalidField {
            field: context.into(),
            message: format!("expected base64 string or byte array, got {other:?}"),
        }),
    }
}
fn execute_system_fee_instruction(
    instr: DMInstructionBox,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let previous_tx_dataspace_id = state_transaction.current_dataspace_id;
    let previous_world_dataspace_id = state_transaction.world.current_dataspace_id;
    state_transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let result = instr.execute(authority, state_transaction);
    state_transaction.current_dataspace_id = previous_tx_dataspace_id;
    state_transaction.world.current_dataspace_id = previous_world_dataspace_id;
    result
}
fn execute_gas_fee_transfer_instruction(
    definition: &AssetDefinition,
    instr: DMInstructionBox,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    if definition.balance_scope_policy() == AssetBalancePolicy::Global {
        execute_system_fee_instruction(instr, authority, state_transaction)
    } else {
        instr.execute(authority, state_transaction)
    }
}
fn metadata_string(
    metadata: &Metadata,
    key: &str,
) -> Result<Option<String>, crate::execution_attempt::ExecutionDeferred> {
    let Some(raw) = metadata.get(key) else {
        return Ok(None);
    };
    let mut value = match norito::json::from_str::<String>(raw.get()) {
        Ok(value) => value,
        Err(error) => match crate::execution_attempt::json_decode_attempt_error(error, |_| ()) {
            crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                return Err(reason);
            }
            crate::execution_attempt::ExecutionAttemptError::Rejected(()) => return Ok(None),
        },
    };
    // Trim in the original decoded String; no second unreserved metadata allocation.
    let start = value.len() - value.trim_start().len();
    let end = start + value.trim().len();
    value.truncate(end);
    value.drain(..start);
    Ok((!value.is_empty()).then_some(value))
}
fn should_charge_pipeline_gas_asset(
    skip_nexus_fee: bool,
    nexus_fees: &RootFeePolicy,
    gas_asset_opt: Option<&str>,
) -> bool {
    !skip_nexus_fee && gas_asset_opt.is_some() && nexus_fees.per_gas_unit_fee.is_zero()
}
fn is_sora_v2_tx_hash_literal(value: &str) -> bool {
    let hex = value.strip_prefix("0x").unwrap_or(value);
    hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn successful_claim_fee_authority_allowed(fees: &RootFeePolicy, authority: &AccountId) -> bool {
    fees.successful_claim_fee_exempt_authorities
        .contains(authority)
}
fn successful_claim_fee_exempt_instructions(
    world: &impl WorldReadOnly,
    fees: &RootFeePolicy,
    authority: &AccountId,
    metadata: &Metadata,
    instructions: &[InstructionBox],
    observation_time_ms: u64,
) -> Result<bool, crate::execution_attempt::ExecutionDeferred> {
    if !successful_claim_fee_authority_allowed(fees, authority) {
        return Ok(false);
    }
    let Some(claim_tx_hash) = metadata_string(metadata, SORA_V2_CLAIM_TX_HASH_METADATA_KEY)? else {
        return Ok(false);
    };
    if !is_sora_v2_tx_hash_literal(&claim_tx_hash) {
        return Ok(false);
    }
    let Some(recipient_literal) =
        metadata_string(metadata, SORA_NEXUS_CLAIM_RECIPIENT_METADATA_KEY)?
    else {
        return Ok(false);
    };
    let recipient = match parse_account_id_literal(
        world,
        world.dataspace_catalog(),
        &recipient_literal,
        observation_time_ms,
    ) {
        Ok(Some(recipient)) => recipient,
        Err(crate::sns::SnsError::Deferred(reason)) => return Err(reason),
        Ok(None) | Err(_) => return Ok(false),
    };
    let Some(asset_def) = crate::block::resolve_network_xor_asset_definition(
        world,
        &fees.fee_asset_id.canonical_address(),
        observation_time_ms,
    )?
    else {
        return Ok(false);
    };
    let [instruction] = instructions else {
        return Ok(false);
    };
    let Some(mint) = instruction.as_any().downcast_ref::<MintBox>() else {
        return Ok(false);
    };
    Ok(match mint {
        MintBox::Asset(mint) => {
            mint.destination.account() == &recipient
                && mint.destination.definition() == &asset_def
                && !mint.object.is_zero()
        }
        MintBox::TriggerRepetitions(_) => false,
    })
}
fn successful_claim_fee_exempt_transaction(
    world: &impl WorldReadOnly,
    fees: &RootFeePolicy,
    transaction: &SignedTransaction,
    observation_time_ms: u64,
) -> Result<bool, crate::execution_attempt::ExecutionDeferred> {
    successful_claim_fee_exempt_payload(world, fees, transaction.payload(), observation_time_ms)
}
fn successful_claim_fee_exempt_payload(
    world: &impl WorldReadOnly,
    fees: &RootFeePolicy,
    payload: &TransactionPayload,
    observation_time_ms: u64,
) -> Result<bool, crate::execution_attempt::ExecutionDeferred> {
    let Executable::Instructions(instructions) = &payload.instructions else {
        return Ok(false);
    };
    let result = successful_claim_fee_exempt_instructions(
        world,
        fees,
        &payload.authority,
        &payload.metadata,
        instructions.as_ref(),
        observation_time_ms,
    );
    if cfg!(all(test, sumeragi_core_mutation = "HC36")) && result.is_err() {
        return Ok(false);
    }
    result
}
fn nexus_protocol_fee_exempt_instruction(instruction: &InstructionBox) -> bool {
    let any = instruction.as_any();
    any.downcast_ref::<iroha_data_model::isi::nexus::RegisterVerifiedFeeSponsorVaultAllocation>()
        .is_some()
}
fn nexus_fee_exempt_instruction(instruction: &InstructionBox) -> bool {
    nexus_protocol_fee_exempt_instruction(instruction)
}
fn nexus_fee_exempt_instructions(instructions: &[InstructionBox]) -> bool {
    !instructions.is_empty() && instructions.iter().all(nexus_fee_exempt_instruction)
}
fn nexus_fee_exempt_transaction(transaction: &SignedTransaction) -> bool {
    nexus_fee_exempt_payload(transaction.payload())
}
fn nexus_fee_exempt_payload(payload: &TransactionPayload) -> bool {
    let Executable::Instructions(instructions) = &payload.instructions else {
        return false;
    };
    nexus_fee_exempt_instructions(instructions.as_ref())
}
fn fee_exempt_payload(
    world: &impl WorldReadOnly,
    fees: &RootFeePolicy,
    payload: &TransactionPayload,
    observation_time_ms: u64,
    sccp_committed_time_ms: Option<u64>,
) -> Result<bool, crate::execution_attempt::ExecutionDeferred> {
    Ok(root_fees::permits_public_exemption(world)?
        && (nexus_fee_exempt_payload(payload)
            || successful_claim_fee_exempt_payload(world, fees, payload, observation_time_ms)?
            || crate::smartcontracts::isi::sccp::fees::exempt_on_success(
                world,
                payload,
                sccp_committed_time_ms,
            )))
}
/// Return whether `transaction` skips the Nexus fee when it succeeds, judged against the
/// committed `world` the next block executes on (queue admission). Execution uses
/// [`transaction_fee_exemption`], which also says whether a failure is charged.
fn fee_exempt_transaction(
    world: &impl WorldReadOnly,
    fees: &RootFeePolicy,
    transaction: &SignedTransaction,
    observation_time_ms: u64,
    sccp_committed_time_ms: Option<u64>,
) -> Result<bool, crate::execution_attempt::ExecutionDeferred> {
    // `world` is the committed parent World, the one SCCP eligibility is judged against.
    Ok(root_fees::permits_public_exemption(world)?
        && (nexus_fee_exempt_transaction(transaction)
            || successful_claim_fee_exempt_transaction(
                world,
                fees,
                transaction,
                observation_time_ms,
            )?
            || crate::smartcontracts::isi::sccp::fees::exempt_on_success(
                world,
                transaction.payload(),
                sccp_committed_time_ms,
            )))
}
/// Return how the Nexus fee applies to `transaction` executing in `state_transaction`, with
/// its SCCP exempt class when it has one.
///
/// Protocol and successful-claim exemptions are judged against the executing World and hold
/// whatever the outcome. SCCP exemptions are judged against the committed parent World of the
/// executing block, exactly like queue admission, fee quoting and the per-block exempt cap,
/// and hold on success only: a failure is charged the ordinary fee (`specs/sccp.md` §4.19).
fn transaction_fee_exemption(
    state_transaction: &StateTransaction<'_, '_>,
    transaction: &SignedTransaction,
) -> Result<
    (
        ExecutionFeeExemption,
        Option<crate::smartcontracts::isi::sccp::admission::SccpExemptClassV1>,
    ),
    crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
> {
    let world = &state_transaction.world;
    if !root_fees::permits_public_exemption(world)
        .map_err(crate::execution_attempt::ExecutionAttemptError::Deferred)?
    {
        return Ok((ExecutionFeeExemption::Charged, None));
    }
    if nexus_fee_exempt_transaction(transaction)
        || successful_claim_fee_exempt_transaction(
            world,
            root_fees::execution_policy(state_transaction).map_err(|reason| {
                crate::execution_attempt::ExecutionAttemptError::Rejected(
                    ValidationFail::InternalError(reason.into()),
                )
            })?,
            transaction,
            state_transaction.block_unix_timestamp_ms(),
        )
        .map_err(crate::execution_attempt::ExecutionAttemptError::Deferred)?
    {
        return Ok((ExecutionFeeExemption::Exempt, None));
    }
    Ok(
        match crate::smartcontracts::isi::sccp::fees::exempt_class_in_block(
            state_transaction,
            transaction.payload(),
        )
        .map_err(|error| {
            crate::execution_attempt::ExecutionAttemptError::Rejected(
                ValidationFail::InternalError(error.to_string()),
            )
        })? {
            Some(class) => (ExecutionFeeExemption::ExemptOnSuccess, Some(class)),
            None => (ExecutionFeeExemption::Charged, None),
        },
    )
}
#[derive(Clone, Copy)]
enum PermissionOrRoleMutation<'a> {
    AccountPermission {
        permission: &'a Permission,
        destination: &'a AccountId,
        is_revoke: bool,
    },
    AccountRole {
        role: &'a RoleId,
        is_revoke: bool,
    },
    RolePermission {
        permission: &'a Permission,
        role: &'a RoleId,
        is_revoke: bool,
    },
}
fn extract_permission_or_role_mutation(
    instruction: &InstructionBox,
) -> Option<PermissionOrRoleMutation<'_>> {
    fn from_grant(grant: &GrantBox) -> PermissionOrRoleMutation<'_> {
        match grant {
            GrantBox::Permission(grant) => PermissionOrRoleMutation::AccountPermission {
                permission: grant.object(),
                destination: grant.destination(),
                is_revoke: false,
            },
            GrantBox::Role(grant) => PermissionOrRoleMutation::AccountRole {
                role: grant.object(),
                is_revoke: false,
            },
            GrantBox::RolePermission(grant) => PermissionOrRoleMutation::RolePermission {
                permission: grant.object(),
                role: grant.destination(),
                is_revoke: false,
            },
        }
    }
    fn from_revoke(revoke: &RevokeBox) -> PermissionOrRoleMutation<'_> {
        match revoke {
            RevokeBox::Permission(revoke) => PermissionOrRoleMutation::AccountPermission {
                permission: revoke.object(),
                destination: revoke.destination(),
                is_revoke: true,
            },
            RevokeBox::Role(revoke) => PermissionOrRoleMutation::AccountRole {
                role: revoke.object(),
                is_revoke: true,
            },
            RevokeBox::RolePermission(revoke) => PermissionOrRoleMutation::RolePermission {
                permission: revoke.object(),
                role: revoke.destination(),
                is_revoke: true,
            },
        }
    }
    let any = instruction.as_any();
    if let Some(grant) = any.downcast_ref::<GrantBox>() {
        return Some(from_grant(grant));
    }
    if let Some(revoke) = any.downcast_ref::<RevokeBox>() {
        return Some(from_revoke(revoke));
    }
    if let Some(grant) = any.downcast_ref::<Grant<Permission, Account>>() {
        return Some(PermissionOrRoleMutation::AccountPermission {
            permission: grant.object(),
            destination: grant.destination(),
            is_revoke: false,
        });
    }
    if let Some(revoke) = any.downcast_ref::<Revoke<Permission, Account>>() {
        return Some(PermissionOrRoleMutation::AccountPermission {
            permission: revoke.object(),
            destination: revoke.destination(),
            is_revoke: true,
        });
    }
    if let Some(grant) = any.downcast_ref::<Grant<RoleId, Account>>() {
        return Some(PermissionOrRoleMutation::AccountRole {
            role: grant.object(),
            is_revoke: false,
        });
    }
    if let Some(revoke) = any.downcast_ref::<Revoke<RoleId, Account>>() {
        return Some(PermissionOrRoleMutation::AccountRole {
            role: revoke.object(),
            is_revoke: true,
        });
    }
    if let Some(grant) = any.downcast_ref::<Grant<Permission, Role>>() {
        return Some(PermissionOrRoleMutation::RolePermission {
            permission: grant.object(),
            role: grant.destination(),
            is_revoke: false,
        });
    }
    any.downcast_ref::<Revoke<Permission, Role>>()
        .map(|revoke| PermissionOrRoleMutation::RolePermission {
            permission: revoke.object(),
            role: revoke.destination(),
            is_revoke: true,
        })
}
#[cfg(test)]
fn mutates_contract_deployment_permission(instruction: &InstructionBox) -> bool {
    matches!(
        extract_permission_or_role_mutation(instruction),
        Some(
            PermissionOrRoleMutation::AccountPermission { permission, .. }
                | PermissionOrRoleMutation::RolePermission { permission, .. }
        ) if matches!(permission.name(), "CanManageSmartContractCode" | "CanGrantSmartContractCodeManagement")
    )
}
fn ensure_contract_code_management_permission_lifecycle(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    permission: &Permission,
) -> Result<(), ValidationFail> {
    let is_genesis = is_initial_genesis_context(state_transaction);
    match permission.name() {
        "CanGrantSmartContractCodeManagement" => {
            executor_permission::smart_contract::CanGrantSmartContractCodeManagement::try_from(
                permission,
            )
            .map_err(|error| invalid_initial_permission_payload(permission, error))?;
            if is_genesis {
                Ok(())
            } else {
                Err(ValidationFail::NotPermitted(
                    "CanGrantSmartContractCodeManagement is only granted or revoked in genesis"
                        .to_owned(),
                ))
            }
        }
        "CanManageSmartContractCode" => {
            executor_permission::smart_contract::CanManageSmartContractCode::try_from(permission)
                .map_err(|error| invalid_initial_permission_payload(permission, error))?;
            let manager: Permission =
                executor_permission::smart_contract::CanGrantSmartContractCodeManagement.into();
            if is_genesis
                || authority_has_permission(&state_transaction.world, authority, &manager)?
            {
                Ok(())
            } else {
                Err(ValidationFail::NotPermitted(
                    "granting or revoking CanManageSmartContractCode requires CanGrantSmartContractCodeManagement".to_owned(),
                ))
            }
        }
        _ => Ok(()),
    }
}
fn ensure_contract_deployment_permission_mutation_allowed(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &InstructionBox,
) -> Result<(), ValidationFail> {
    let validate = |permission: &Permission| {
        ensure_contract_code_management_permission_lifecycle(
            state_transaction,
            authority,
            permission,
        )
    };
    if let Some(register) = extract_register_role(instruction) {
        for permission in register.object().inner().permissions() {
            validate(permission)?;
        }
    }
    let role_id = match extract_permission_or_role_mutation(instruction) {
        Some(
            PermissionOrRoleMutation::AccountPermission { permission, .. }
            | PermissionOrRoleMutation::RolePermission { permission, .. },
        ) => {
            return validate(permission);
        }
        Some(PermissionOrRoleMutation::AccountRole { role, .. }) => Some(role.clone()),
        None => extract_unregister_role(instruction).map(|unregister| unregister.object().clone()),
    };
    if let Some(role_id) = role_id
        && let Some(role) = state_transaction.world.roles().get(&role_id)
    {
        for permission in role.permissions() {
            validate(permission)?;
        }
    }
    Ok(())
}
/// Enforce exact entrypoint delegation before any executor can approve a mutation.
/// This boundary also receives instructions emitted by IVM hosts and triggers.
fn ensure_contract_entrypoint_authorization_mutation_allowed(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &InstructionBox,
) -> Result<(), ValidationFail> {
    let validate = |permission: &Permission| {
        if permission.name() == "CanUseContractPermission" {
            return if contract_permission_delegation_allowed(
                state_transaction,
                authority,
                permission,
            )? {
                Ok(())
            } else {
                Err(ValidationFail::NotPermitted("only an exact holder or the current account lifecycle owner may grant or revoke a declared instance permission".to_owned()))
            };
        }
        if permission.name() != "CanInvokeContractEntrypoint" {
            return Ok(());
        }
        if contract_entrypoint_authorization_delegation_allowed(
            state_transaction,
            authority,
            permission,
        )? {
            return Ok(());
        }
        Err(ValidationFail::NotPermitted(
            "only genesis, an exact holder, a smart-contract code manager, or the current account lifecycle owner may grant or revoke an exact contract entrypoint permission".to_owned(),
        ))
    };
    if let Some(register) = extract_register_role(instruction) {
        for permission in register.object().inner().permissions() {
            validate(permission)?;
        }
    }
    let role_id = match extract_permission_or_role_mutation(instruction) {
        Some(
            PermissionOrRoleMutation::AccountPermission { permission, .. }
            | PermissionOrRoleMutation::RolePermission { permission, .. },
        ) => return validate(permission),
        Some(PermissionOrRoleMutation::AccountRole { role, .. }) => Some(role.clone()),
        None => extract_unregister_role(instruction).map(|unregister| unregister.object().clone()),
    };
    if let Some(role_id) = role_id
        && let Some(role) = state_transaction.world.roles().get(&role_id)
    {
        for permission in role.permissions() {
            validate(permission)?;
        }
    }
    Ok(())
}
fn ensure_contract_runtime_permission_mutation_allowed(
    authority: &AccountId,
    instruction: &InstructionBox,
    contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
) -> Result<(), ValidationFail> {
    let Some(context) = contract_runtime_context else {
        return Ok(());
    };
    let mutation = extract_permission_or_role_mutation(instruction);
    let mutates_role = extract_register_role(instruction).is_some()
        || extract_unregister_role(instruction).is_some()
        || matches!(
            mutation,
            Some(
                PermissionOrRoleMutation::AccountRole { .. }
                    | PermissionOrRoleMutation::RolePermission { .. }
            )
        );
    if mutates_role {
        return Err(ValidationFail::NotPermitted(
            "deployed contracts may not register, unregister, or mutate role membership or role permissions"
                .to_owned(),
        ));
    }
    let Some(PermissionOrRoleMutation::AccountPermission { permission, .. }) = mutation else {
        return Ok(());
    };
    let scoped =
        executor_permission::smart_contract::CanUseContractPermission::try_from(permission)
            .map_err(|_| {
                ValidationFail::NotPermitted(
            "deployed contracts may grant or revoke only exact CanUseContractPermission tokens"
                .to_owned(),
        )
            })?;
    if authority != &context.contract_subject
        || scoped.contract != context.contract_address
        || scoped.contract.subject_id() != *authority
        || context.contract_address.subject_id() != context.contract_subject
    {
        return Err(ValidationFail::NotPermitted(
            "deployed contract permission mutation must be bound to its immutable subject, address, and a declared instance permission"
                .to_owned(),
        ));
    }
    Ok(())
}
fn parse_account_id_literal(
    world: &impl WorldReadOnly,
    dataspace_catalog: &iroha_data_model::nexus::DataSpaceCatalog,
    literal: &str,
    now_ms: u64,
) -> Result<Option<AccountId>, crate::sns::SnsError> {
    crate::block::parse_account_literal_with_world(world, dataspace_catalog, literal, now_ms)
}
/// Deterministic reason a Nexus fee quote or admission check failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NexusFeeAdmissionError {
    /// The selected fee payer cannot satisfy deterministic admission.
    Rejected {
        /// Stable machine-readable rejection code.
        code: FeeRejectionCode,
        /// Human-readable diagnostic detail.
        reason: String,
    },
    /// Node or persisted fee configuration is invalid.
    ConfigInvalid(String),
}
impl core::fmt::Display for NexusFeeAdmissionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.reason())
    }
}
impl std::error::Error for NexusFeeAdmissionError {}
impl NexusFeeAdmissionError {
    fn rejected(code: FeeRejectionCode, reason: impl Into<String>) -> Self {
        Self::Rejected {
            code,
            reason: reason.into(),
        }
    }
    fn sponsor(code: FeeRejectionCode, reason: impl Into<String>) -> Self {
        Self::rejected(code, reason)
    }
    /// Stable fee-admission denial code.
    pub const fn code(&self) -> FeeRejectionCode {
        match self {
            Self::Rejected { code, .. } => *code,
            Self::ConfigInvalid(_) => FeeRejectionCode::InvalidProgramConfiguration,
        }
    }
    /// Human-readable detail suitable for authorized diagnostics.
    pub fn reason(&self) -> &str {
        match self {
            Self::Rejected { reason, .. } | Self::ConfigInvalid(reason) => reason,
        }
    }
}
#[cfg(test)]
fn smart_contract_state_name(
    raw: String,
    context: &'static str,
) -> Result<StatePath, NexusFeeAdmissionError> {
    StatePath::from_str(&raw).map_err(|_| {
        NexusFeeAdmissionError::ConfigInvalid(format!(
            "invalid smart-contract state key: {context}"
        ))
    })
}
#[cfg(test)]
fn decode_verified_fee_sponsor_vault_allocation_state(
    payload: &[u8],
) -> Result<VerifiedFeeSponsorVaultAllocation, NexusFeeAdmissionError> {
    let json: Json = norito::decode_from_bytes(payload).map_err(|err| {
        NexusFeeAdmissionError::ConfigInvalid(format!(
            "verified fee sponsor vault allocation state decode failed: {err}"
        ))
    })?;
    norito::json::from_slice(json.get().as_bytes()).map_err(|err| {
        NexusFeeAdmissionError::ConfigInvalid(format!(
            "verified fee sponsor vault allocation JSON decode failed: {err}"
        ))
    })
}
#[cfg(test)]
fn fee_sponsor_vault_allocation_usage_state_key(
    lease_id: &iroha_crypto::Hash,
) -> Result<StatePath, NexusFeeAdmissionError> {
    smart_contract_state_name(
        VerifiedFeeSponsorVaultAllocation::usage_state_key_for(lease_id),
        "verified fee sponsor vault allocation usage",
    )
}
#[cfg(test)]
fn fee_sponsor_vault_allocation_settled_usage_state_key(
    lease_id: &iroha_crypto::Hash,
) -> Result<StatePath, NexusFeeAdmissionError> {
    smart_contract_state_name(
        VerifiedFeeSponsorVaultAllocation::settled_usage_state_key_for(lease_id),
        "settled verified fee sponsor vault allocation usage",
    )
}
#[cfg(test)]
fn fee_sponsor_vault_allocation_quantity_at(
    world: &impl WorldReadOnly,
    key: &StatePath,
) -> Result<Quantity, NexusFeeAdmissionError> {
    world.smart_contract_state().get(key).map_or_else(
        || Ok(Quantity::zero()),
        |payload| {
            norito::decode_from_bytes(payload).map_err(|err| {
                NexusFeeAdmissionError::ConfigInvalid(format!(
                    "verified fee sponsor vault allocation usage decode failed: {err}"
                ))
            })
        },
    )
}
#[cfg(test)]
fn fee_sponsor_vault_allocation_spent(
    world: &impl WorldReadOnly,
    lease_id: &iroha_crypto::Hash,
) -> Result<Quantity, NexusFeeAdmissionError> {
    let executed_key = fee_sponsor_vault_allocation_usage_state_key(lease_id)?;
    let settled_key = fee_sponsor_vault_allocation_settled_usage_state_key(lease_id)?;
    let executed = fee_sponsor_vault_allocation_quantity_at(world, &executed_key)?;
    let settled = fee_sponsor_vault_allocation_quantity_at(world, &settled_key)?;
    Ok(core::cmp::max(executed, settled))
}
#[cfg(test)]
fn select_fee_sponsor_relay_lease(
    world: &impl WorldReadOnly,
    program_id: &FeeSponsorProgramId,
    program_revision: u64,
    asset_definition_id: &AssetDefinitionId,
    route_dataspace_id: Option<DataSpaceId>,
    admission_height: u64,
    required: &Quantity,
) -> Result<(VerifiedFeeSponsorVaultAllocation, Quantity), NexusFeeAdmissionError> {
    let source_dataspace_id = route_dataspace_id.unwrap_or(DataSpaceId::UNIVERSAL);
    let mut candidates = Vec::new();
    for (key, payload) in world.smart_contract_state().iter() {
        if !key
            .to_string()
            .starts_with(VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX)
        {
            continue;
        }
        let record = decode_verified_fee_sponsor_vault_allocation_state(payload)?;
        let canonical_key = VerifiedFeeSponsorVaultAllocation::state_key_for(
            &record.program_id,
            &record.asset_definition_id,
            &record.lease_id,
        );
        if key.to_string() != canonical_key {
            return Err(NexusFeeAdmissionError::ConfigInvalid(format!(
                "verified fee sponsor vault allocation `{}` is stored under a non-canonical key",
                record.lease_id
            )));
        }
        if &record.program_id != program_id
            || record.program_revision != program_revision
            || &record.asset_definition_id != asset_definition_id
            || record.source_dataspace_id != source_dataspace_id
            || record.source_height > admission_height
            || record.verified_at_height > admission_height
            || record.expires_at_height < admission_height
        {
            continue;
        }
        let spent = fee_sponsor_vault_allocation_spent(world, &record.lease_id)?;
        let remaining = record
            .verified_allocation
            .checked_sub(&spent)
            .map_err(|_| {
                NexusFeeAdmissionError::ConfigInvalid(format!(
                    "verified fee sponsor vault allocation `{}` is overspent",
                    record.lease_id
                ))
            })?;
        if remaining >= *required {
            candidates.push((record, remaining));
        }
    }
    candidates.sort_by(|(left, _), (right, _)| left.lease_id.as_ref().cmp(right.lease_id.as_ref()));
    candidates.into_iter().next().ok_or_else(|| {
        NexusFeeAdmissionError::sponsor(
            FeeRejectionCode::RelayCapacityUnavailable,
            format!(
                "no unexpired verified spend lease covers sponsor program `{program_id}` revision {program_revision}, route dataspace {}, asset `{asset_definition_id}`, and charge {required}",
                source_dataspace_id.as_u64(),
            ),
        )
    })
}
#[cfg(test)]
fn select_fee_sponsor_relay_leases(
    world: &impl WorldReadOnly,
    program_id: &FeeSponsorProgramId,
    program_revision: u64,
    route_dataspace_id: Option<DataSpaceId>,
    admission_height: u64,
    charges: &[FeeChargeBound],
) -> Result<BTreeMap<AssetDefinitionId, FeeSponsorRelayLeaseCapacity>, NexusFeeAdmissionError> {
    let mut required_by_asset = BTreeMap::<AssetDefinitionId, Quantity>::new();
    for charge in charges {
        let current = required_by_asset
            .get(&charge.asset_definition_id)
            .cloned()
            .unwrap_or_else(Quantity::zero);
        required_by_asset.insert(
            charge.asset_definition_id.clone(),
            checked_quantity_add(&current, &charge.max_bound, "relay spend-lease charge")?,
        );
    }
    let mut selections = BTreeMap::new();
    for (asset_definition_id, required) in required_by_asset {
        let (record, remaining) = select_fee_sponsor_relay_lease(
            world,
            program_id,
            program_revision,
            &asset_definition_id,
            route_dataspace_id,
            admission_height,
            &required,
        )?;
        selections.insert(
            asset_definition_id,
            FeeSponsorRelayLeaseCapacity {
                lease_id: record.lease_id,
                remaining,
            },
        );
    }
    Ok(selections)
}
fn validation_fail_to_nexus_fee_admission_error(err: ValidationFail) -> NexusFeeAdmissionError {
    match err {
        ValidationFail::InternalError(reason) => NexusFeeAdmissionError::ConfigInvalid(reason),
        other => {
            NexusFeeAdmissionError::rejected(FeeRejectionCode::InvalidFeeIntent, other.to_string())
        }
    }
}
fn nexus_fee_admission_error_to_validation_fail(err: NexusFeeAdmissionError) -> ValidationFail {
    match err {
        NexusFeeAdmissionError::Rejected { reason, .. } => ValidationFail::NotPermitted(reason),
        NexusFeeAdmissionError::ConfigInvalid(reason) => ValidationFail::InternalError(reason),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum FeeSponsorOperation {
    NativeInstruction {
        wire_id: String,
        asset_definition_id: Option<AssetDefinitionId>,
    },
    Multisig {
        operation: FeeSponsorMultisigOperation,
        account_id: AccountId,
    },
    ContractCall {
        contract_address: iroha_data_model::smart_contract::ContractAddress,
        code_hash: iroha_crypto::Hash,
        entrypoint: String,
    },
    Ivm {
        code_hash: iroha_crypto::Hash,
        proved: bool,
    },
}
/// One deterministic fee component quoted by Core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeChargeBound {
    /// Fee component represented by this bound.
    pub kind: FeeChargeKind,
    /// Canonical asset in which the component is charged.
    pub asset_definition_id: AssetDefinitionId,
    /// Deterministic maximum charge for the supplied payload and state.
    pub max_bound: Quantity,
}
/// Remaining capacity for one program asset at the observed state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeSponsorCapacity {
    /// Current isolated program-vault balance.
    pub vault_balance: Quantity,
    /// Balance that must remain after the charge.
    pub reserve_floor: Quantity,
    /// Capacity remaining in the observed block window before this quote.
    pub block_remaining: Quantity,
    /// Capacity remaining in the observed program epoch before this quote.
    pub program_epoch_remaining: Quantity,
    /// Capacity remaining for this beneficiary epoch before this quote.
    pub beneficiary_epoch_remaining: Quantity,
}
/// Exact proof-bound spend lease selected for one sponsored fee asset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeSponsorRelayLeaseCapacity {
    /// Canonical lease selected for the program revision, asset, and route.
    pub lease_id: iroha_crypto::Hash,
    /// Remaining verified allocation on the lease before this quote.
    pub remaining: Quantity,
}
/// Read-only deterministic fee quote shared by queue admission and Torii.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeAdmissionQuote {
    /// Deterministic charge bounds, canonically ordered by component.
    pub charges: Vec<FeeChargeBound>,
    /// Exact account or isolated program vault selected by the payload.
    pub debit_source: FeeDebitSource,
    /// Active immutable sponsor revision, when sponsored.
    pub program_revision: Option<u64>,
    /// Canonically selected proof-bound spend lease for each sponsored fee asset.
    ///
    /// This map is populated only for receipt-lane settlement. All charge components using the same
    /// asset share one selection and consume its aggregate maximum.
    pub relay_leases: BTreeMap<AssetDefinitionId, FeeSponsorRelayLeaseCapacity>,
    /// Per-asset sponsor capacity snapshot, empty for authority payment.
    pub capacities: BTreeMap<AssetDefinitionId, FeeSponsorCapacity>,
    /// Exact authority balance buckets observed for account-paid charges.
    ///
    /// Keys include global versus dataspace-restricted scope so queue
    /// reservations cannot overbook two components against the same bucket.
    pub authority_balances: BTreeMap<AssetId, Quantity>,
    /// Exact authority balance bucket selected for each charge component.
    pub authority_charge_assets: BTreeMap<FeeChargeKind, AssetId>,
}
/// Unsigned quote result with the exact fee intent that should be signed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeAdmissionDraftQuote {
    /// Admission result for the returned signature-bound intent.
    pub quote: FeeAdmissionQuote,
    /// Exact payer selection, revision, gas limit, assets, and charge maxima to sign.
    pub recommended_intent: FeePaymentIntent,
}
#[derive(Clone, Debug)]
struct ResolvedSponsorProgram {
    id: FeeSponsorProgramId,
    revision: FeeSponsorProgramRevision,
}
fn fee_sponsor_asset_transfer_definition_id(
    instruction: &InstructionBox,
) -> Option<AssetDefinitionId> {
    let any = instruction.as_any();
    if let Some(transfer) = any.downcast_ref::<TransferBox>() {
        return match transfer {
            TransferBox::Asset(transfer) => Some(transfer.source.definition().clone()),
            TransferBox::Domain(_) | TransferBox::AssetDefinition(_) | TransferBox::Nft(_) => None,
        };
    }
    any.downcast_ref::<Transfer<Asset, Quantity, Account>>()
        .map(|transfer| transfer.source.definition().clone())
}
fn fee_sponsor_instruction_operation(
    instruction: &InstructionBox,
) -> Result<
    FeeSponsorOperation,
    crate::execution_attempt::ExecutionAttemptError<NexusFeeAdmissionError>,
> {
    use crate::execution_attempt::ExecutionAttemptError;
    let multisig = match MultisigInstructionBox::try_from(instruction) {
        Ok(value) => Some(value),
        Err(error) => {
            match crate::smartcontracts::isi::multisig::multisig_instruction_decode_attempt(
                error,
                |_| (),
            ) {
                ExecutionAttemptError::Deferred(reason) => {
                    return Err(ExecutionAttemptError::Deferred(reason));
                }
                ExecutionAttemptError::Rejected(()) => None,
            }
        }
    };
    if let Some(multisig) = multisig {
        let (operation, account_id) = match multisig {
            MultisigInstructionBox::Propose(propose) => {
                (FeeSponsorMultisigOperation::Propose, propose.account)
            }
            MultisigInstructionBox::Approve(approve) => {
                (FeeSponsorMultisigOperation::Approve, approve.account)
            }
            MultisigInstructionBox::Cancel(cancel) => {
                (FeeSponsorMultisigOperation::Cancel, cancel.account)
            }
            MultisigInstructionBox::InvalidateOutstanding(invalidate) => {
                (FeeSponsorMultisigOperation::Cancel, invalidate.account)
            }
            MultisigInstructionBox::Register(register) => {
                (FeeSponsorMultisigOperation::Register, register.account)
            }
        };
        return Ok(FeeSponsorOperation::Multisig {
            operation,
            account_id,
        });
    }
    let wire_id = iroha_data_model::isi::instruction_wire_id(instruction)
        .ok_or_else(|| {
            NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::InvalidProgramConfiguration,
                "fee sponsor program could not resolve native instruction wire id",
            )
        })?
        .to_owned();
    Ok(FeeSponsorOperation::NativeInstruction {
        wire_id,
        asset_definition_id: fee_sponsor_asset_transfer_definition_id(instruction),
    })
}
fn fee_sponsor_operations(
    executable: &Executable,
) -> Result<
    Vec<FeeSponsorOperation>,
    crate::execution_attempt::ExecutionAttemptError<NexusFeeAdmissionError>,
> {
    match executable {
        Executable::Instructions(instructions) => instructions
            .iter()
            .map(fee_sponsor_instruction_operation)
            .collect(),
        Executable::ContractCall(invocation) => Ok(vec![FeeSponsorOperation::ContractCall {
            contract_address: invocation.contract_address.clone(),
            code_hash: invocation.expected_code_hash,
            entrypoint: invocation.entrypoint.clone(),
        }]),
        Executable::Batch(items) => items
            .iter()
            .map(|item| match item {
                ExecutableBatchItem::Instruction(instruction) => {
                    fee_sponsor_instruction_operation(instruction)
                }
                ExecutableBatchItem::ContractCall(invocation) => {
                    Ok(FeeSponsorOperation::ContractCall {
                        contract_address: invocation.contract_address.clone(),
                        code_hash: invocation.expected_code_hash,
                        entrypoint: invocation.entrypoint.clone(),
                    })
                }
            })
            .collect(),
        Executable::Ivm(bytecode) => Ok(vec![FeeSponsorOperation::Ivm {
            code_hash: iroha_crypto::Hash::new(bytecode.as_ref()),
            proved: false,
        }]),
        Executable::IvmProved(proved) => Ok(vec![FeeSponsorOperation::Ivm {
            code_hash: iroha_crypto::Hash::new(proved.bytecode.as_ref()),
            proved: true,
        }]),
    }
}
fn fee_sponsor_selector_matches_operation(
    selector: &FeeSponsorRuleSelector,
    operation: &FeeSponsorOperation,
) -> bool {
    match (selector, operation) {
        (
            FeeSponsorRuleSelector::NativeInstruction(selector),
            FeeSponsorOperation::NativeInstruction {
                wire_id,
                asset_definition_id,
            },
        ) => {
            selector.wire_id == *wire_id
                && selector
                    .asset_definition_id
                    .as_ref()
                    .is_none_or(|selected| asset_definition_id.as_ref() == Some(selected))
        }
        (
            FeeSponsorRuleSelector::Multisig(selector),
            FeeSponsorOperation::Multisig {
                operation,
                account_id,
            },
        ) => {
            selector.operations.binary_search(operation).is_ok()
                && selector.account_ids.binary_search(account_id).is_ok()
        }
        (
            FeeSponsorRuleSelector::ContractCall(selector),
            FeeSponsorOperation::ContractCall {
                contract_address,
                code_hash,
                entrypoint,
            },
        ) => {
            selector.contract_address == *contract_address
                && selector.code_hash == *code_hash
                && (selector.entrypoints.is_empty() || selector.entrypoints.contains(entrypoint))
        }
        (
            FeeSponsorRuleSelector::Ivm(selector),
            FeeSponsorOperation::Ivm {
                code_hash,
                proved: false,
            },
        )
        | (
            FeeSponsorRuleSelector::IvmProved(selector),
            FeeSponsorOperation::Ivm {
                code_hash,
                proved: true,
            },
        ) => selector.code_hash == *code_hash,
        _ => false,
    }
}
fn validate_fee_sponsor_rules(
    world: &impl WorldReadOnly,
    revision: &FeeSponsorProgramRevision,
    beneficiary: &AccountId,
    payload: &TransactionPayload,
    context: fee_sponsor_multisig::RuleContext,
) -> Result<(), crate::execution_attempt::ExecutionAttemptError<NexusFeeAdmissionError>> {
    let operations = fee_sponsor_operations(&payload.instructions)?;
    if operations.is_empty() {
        return Err(NexusFeeAdmissionError::sponsor(
            FeeRejectionCode::OperationNotAllowed,
            "fee sponsor program cannot authorize an empty executable",
        )
        .into());
    }
    for (index, operation) in operations.iter().enumerate() {
        let mut allowed = false;
        // Evaluate every matching deny before accepting an allow. Typed resource
        // deferral must never become a non-match that silently skips a deny.
        for effect in [FeeSponsorRuleEffect::Deny, FeeSponsorRuleEffect::Allow] {
            for rule in revision.rules.iter().filter(|rule| rule.effect == effect) {
                for selector in &rule.selectors {
                    let matches = match selector {
                        FeeSponsorRuleSelector::EnrolledMultisigContractCall(selector) => {
                            fee_sponsor_multisig::selector_matches(
                                world,
                                &revision.program_id,
                                beneficiary,
                                selector,
                                payload,
                                index,
                                context,
                            )?
                        }
                        _ => fee_sponsor_selector_matches_operation(selector, operation),
                    };
                    if matches {
                        if effect == FeeSponsorRuleEffect::Deny {
                            return Err(NexusFeeAdmissionError::sponsor(
                                FeeRejectionCode::OperationDenied,
                                "signed operation matches an explicit fee sponsor deny rule",
                            )
                            .into());
                        }
                        allowed = true;
                    }
                }
            }
        }
        if !allowed {
            return Err(NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::OperationNotAllowed,
                "signed operation is not covered by a fee sponsor allow rule",
            )
            .into());
        }
    }
    Ok(())
}
fn resolve_fee_sponsor_program(
    world: &impl WorldReadOnly,
    nexus: &iroha_config::parameters::actual::Nexus,
    program_id: &FeeSponsorProgramId,
    signed_revision: u64,
    beneficiary: &AccountId,
    payload: &TransactionPayload,
    route_dataspace_id: Option<DataSpaceId>,
    block_height: u64,
    context: fee_sponsor_multisig::RuleContext,
) -> Result<
    ResolvedSponsorProgram,
    crate::execution_attempt::ExecutionAttemptError<NexusFeeAdmissionError>,
> {
    let program = world
        .fee_sponsor_programs()
        .get(program_id)
        .ok_or_else(|| {
            NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::ProgramNotFound,
                format!("fee sponsor program `{program_id}` does not exist"),
            )
        })?;
    let due_revision = program
        .scheduled_activation
        .filter(|activation| activation.activate_at_height <= block_height)
        .map(|activation| {
            fee_sponsor_revision_safe_activation_height(
                world,
                program_id,
                activation.revision,
                block_height,
                block_height,
            )
            .map(|safe_height| (safe_height == block_height).then_some(activation.revision))
            .map_err(NexusFeeAdmissionError::ConfigInvalid)
        })
        .transpose()?
        .flatten();
    let effective_revision = due_revision.or(program.active_revision);
    let lifecycle_accepts = program.lifecycle == FeeSponsorProgramLifecycle::Active
        || (due_revision.is_some()
            && !matches!(
                program.lifecycle,
                FeeSponsorProgramLifecycle::Closing | FeeSponsorProgramLifecycle::Closed
            ));
    if !lifecycle_accepts {
        return Err(NexusFeeAdmissionError::sponsor(
            FeeRejectionCode::ProgramNotActive,
            format!(
                "fee sponsor program `{program_id}` is {:?}",
                program.lifecycle
            ),
        )
        .into());
    }
    if effective_revision != Some(signed_revision) {
        return Err(NexusFeeAdmissionError::sponsor(
            FeeRejectionCode::RevisionNotActive,
            format!(
                "fee sponsor program `{program_id}` active revision is {effective_revision:?}; transaction selected {signed_revision}"
            ),
        ).into());
    }
    let revision = world
        .fee_sponsor_program_revisions()
        .get(&FeeSponsorProgramRevisionKey::new(
            program_id.clone(),
            signed_revision,
        ))
        .cloned()
        .ok_or_else(|| {
            NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::RevisionNotFound,
                format!("fee sponsor program `{program_id}` revision {signed_revision} is missing"),
            )
        })?;
    if revision.program_id != *program_id {
        return Err(NexusFeeAdmissionError::sponsor(
            FeeRejectionCode::InvalidProgramConfiguration,
            "fee sponsor revision key does not match its embedded program id",
        )
        .into());
    }
    let enrollment_key = FeeSponsorEnrollmentKey {
        program_id: program_id.clone(),
        beneficiary: beneficiary.clone(),
    };
    let enrolled = world
        .fee_sponsor_enrollments()
        .get(&enrollment_key)
        .is_some();
    let exact_route_default = route_dataspace_id.is_some_and(|dataspace| {
        nexus.dataspace_fee_sponsor_program_ids.get(&dataspace) == Some(program_id)
    });
    let eligible = enrolled
        || (revision.eligibility == FeeSponsorEligibility::EnrolledOrRouteDefault
            && exact_route_default);
    if !eligible {
        return Err(NexusFeeAdmissionError::sponsor(
            FeeRejectionCode::BeneficiaryNotEligible,
            format!(
                "beneficiary `{beneficiary}` is not enrolled and `{program_id}` is not the eligible exact route default"
            ),
        ).into());
    }
    validate_fee_sponsor_rules(world, &revision, beneficiary, payload, context)?;
    Ok(ResolvedSponsorProgram {
        id: program_id.clone(),
        revision,
    })
}
fn checked_quantity_add(
    lhs: &Quantity,
    rhs: &Quantity,
    context: &'static str,
) -> Result<Quantity, NexusFeeAdmissionError> {
    lhs.checked_add(rhs).map_err(|_| {
        NexusFeeAdmissionError::sponsor(
            FeeRejectionCode::InvalidProgramConfiguration,
            format!("fee sponsor {context} arithmetic overflow"),
        )
    })
}
fn counter_spent(world: &impl WorldReadOnly, key: &FeeSponsorBudgetCounterKey) -> Quantity {
    world
        .fee_sponsor_budget_counters()
        .get(key)
        .map_or_else(Quantity::zero, |counter| counter.spent.clone())
}
fn remaining_capacity(limit: &Quantity, spent: &Quantity) -> Quantity {
    limit
        .checked_sub(spent)
        .unwrap_or_else(|_| Quantity::zero())
}
fn evaluate_fee_sponsor_capacity(
    world: &impl WorldReadOnly,
    resolved: &ResolvedSponsorProgram,
    beneficiary: &AccountId,
    block_height: u64,
    charges: &[FeeChargeBound],
) -> Result<BTreeMap<AssetDefinitionId, FeeSponsorCapacity>, NexusFeeAdmissionError> {
    let mut totals = BTreeMap::<AssetDefinitionId, Quantity>::new();
    for charge in charges {
        let total = totals
            .get(&charge.asset_definition_id)
            .cloned()
            .unwrap_or_else(Quantity::zero);
        totals.insert(
            charge.asset_definition_id.clone(),
            checked_quantity_add(&total, &charge.max_bound, "per-transaction charge")?,
        );
    }
    let mut capacities = BTreeMap::new();
    for (asset_definition_id, amount) in totals {
        let definition = world.asset_definition(&asset_definition_id).map_err(|_| {
            NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::InvalidProgramConfiguration,
                format!("fee sponsor asset `{asset_definition_id}` is not registered"),
            )
        })?;
        if definition.balance_scope_policy() != AssetBalancePolicy::Global {
            return Err(NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::InvalidProgramConfiguration,
                format!("fee sponsor asset `{asset_definition_id}` must use Global balance scope"),
            ));
        }
        let budget = resolved
            .revision
            .asset_budgets
            .iter()
            .find(|budget| budget.asset_definition_id == asset_definition_id)
            .ok_or_else(|| {
                NexusFeeAdmissionError::sponsor(
                    FeeRejectionCode::FeeAssetNotCovered,
                    format!(
                        "fee sponsor revision {} does not cover asset `{asset_definition_id}`",
                        resolved.revision.revision
                    ),
                )
            })?;
        if amount > budget.per_transaction {
            return Err(NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::ProgramTransactionLimitExceeded,
                format!(
                    "fee sponsor per-transaction budget for `{asset_definition_id}` is {}; requires {amount}",
                    budget.per_transaction
                ),
            ));
        }
        let epoch = block_height.saturating_sub(1) / budget.epoch_length_blocks.get();
        let block_key = FeeSponsorBudgetCounterKey {
            program_id: resolved.id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::Block(FeeSponsorBlockBudgetWindow {
                height: block_height,
            }),
        };
        let program_epoch_key = FeeSponsorBudgetCounterKey {
            program_id: resolved.id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::ProgramEpoch(FeeSponsorProgramEpochBudgetWindow {
                epoch,
            }),
        };
        let beneficiary_epoch_key = FeeSponsorBudgetCounterKey {
            program_id: resolved.id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::BeneficiaryEpoch(
                FeeSponsorBeneficiaryEpochBudgetWindow {
                    epoch,
                    beneficiary: beneficiary.clone(),
                },
            ),
        };
        let block_spent = counter_spent(world, &block_key);
        let program_spent = counter_spent(world, &program_epoch_key);
        let beneficiary_spent = counter_spent(world, &beneficiary_epoch_key);
        let block_after = checked_quantity_add(&block_spent, &amount, "block budget")?;
        if block_after > budget.per_block {
            return Err(NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::ProgramBlockBudgetExhausted,
                format!("fee sponsor block budget for `{asset_definition_id}` is exhausted"),
            ));
        }
        let program_after = checked_quantity_add(&program_spent, &amount, "program epoch budget")?;
        if program_after > budget.per_program_epoch {
            return Err(NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::ProgramEpochBudgetExhausted,
                format!(
                    "fee sponsor program epoch budget for `{asset_definition_id}` is exhausted"
                ),
            ));
        }
        let beneficiary_after =
            checked_quantity_add(&beneficiary_spent, &amount, "beneficiary epoch budget")?;
        if beneficiary_after > budget.per_beneficiary_epoch {
            return Err(NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::BeneficiaryEpochBudgetExhausted,
                format!(
                    "fee sponsor beneficiary epoch budget for `{asset_definition_id}` is exhausted"
                ),
            ));
        }
        let vault_key = FeeSponsorVaultKey {
            program_id: resolved.id.clone(),
            asset_definition_id: asset_definition_id.clone(),
        };
        let vault_balance = world
            .fee_sponsor_vaults()
            .get(&vault_key)
            .map_or_else(Quantity::zero, |vault| vault.balance.clone());
        let required = checked_quantity_add(&amount, &budget.reserve_floor, "vault reserve")?;
        if vault_balance < required {
            return Err(NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::VaultInsufficient,
                format!(
                    "fee sponsor vault for `{asset_definition_id}` requires {required}; available {vault_balance}"
                ),
            ));
        }
        capacities.insert(
            asset_definition_id,
            FeeSponsorCapacity {
                vault_balance,
                reserve_floor: budget.reserve_floor.clone(),
                block_remaining: remaining_capacity(&budget.per_block, &block_spent),
                program_epoch_remaining: remaining_capacity(
                    &budget.per_program_epoch,
                    &program_spent,
                ),
                beneficiary_epoch_remaining: remaining_capacity(
                    &budget.per_beneficiary_epoch,
                    &beneficiary_spent,
                ),
            },
        );
    }
    Ok(capacities)
}
fn validate_signed_charge_limits(
    intent: &FeePaymentIntent,
    charges: &[FeeChargeBound],
) -> Result<(), NexusFeeAdmissionError> {
    for charge in charges {
        let limit = intent
            .charge_limits()
            .iter()
            .find(|limit| limit.kind == charge.kind)
            .ok_or_else(|| {
                NexusFeeAdmissionError::sponsor(
                    FeeRejectionCode::InvalidFeeIntent,
                    format!(
                        "signed fee intent is missing {:?} charge limit",
                        charge.kind
                    ),
                )
            })?;
        if limit.asset_definition_id != charge.asset_definition_id {
            return Err(NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::FeeAssetNotCovered,
                format!(
                    "signed {:?} fee asset `{}` does not match required `{}`",
                    charge.kind, limit.asset_definition_id, charge.asset_definition_id
                ),
            ));
        }
        if limit.max_amount < charge.max_bound {
            return Err(NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::SignedLimitExceeded,
                format!(
                    "computed {:?} fee {} exceeds signed maximum {}",
                    charge.kind, charge.max_bound, limit.max_amount
                ),
            ));
        }
    }
    Ok(())
}
/// Return the explicit signature-bound executable gas limit.
pub(crate) fn transaction_gas_limit(transaction: &SignedTransaction) -> Option<u64> {
    transaction
        .fee_payment_intent()
        .gas_limit()
        .map(core::num::NonZeroU64::get)
}
fn overlay_build_error_to_attempt_validation_fail(
    state: &mut StateTransaction<'_, '_>,
    error: crate::pipeline::overlay::OverlayBuildError,
) -> ValidationFail {
    match error.into_execution_deferral() {
        Ok(reason) => state.defer_execution(reason),
        Err(error) => overlay_build_error_to_validation_fail(error),
    }
}
fn overlay_build_error_to_validation_fail(
    error: crate::pipeline::overlay::OverlayBuildError,
) -> ValidationFail {
    match error {
        crate::pipeline::overlay::OverlayBuildError::ExecutionOwner(message) => {
            ValidationFail::InternalError(message)
        }
        crate::pipeline::overlay::OverlayBuildError::HeaderPolicy(error) => {
            ValidationFail::IvmAdmission(error)
        }
        crate::pipeline::overlay::OverlayBuildError::AxtReject(context) => {
            ValidationFail::AxtReject(context)
        }
        crate::pipeline::overlay::OverlayBuildError::InvalidAxtPolicySnapshot(error) => {
            ValidationFail::InternalError(format!("invalid AXT policy snapshot: {error}"))
        }
        other => ValidationFail::NotPermitted(other.to_string()),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreparedIvmZkAvailability {
    RequireLocalBackend,
    GovernedProof,
}
fn validate_prepared_ivm_execution_policy_with_availability<R: StateReadOnly>(
    state: &R,
    metadata: &ivm::ProgramMetadata,
    zk_availability: PreparedIvmZkAvailability,
) -> Result<std::num::NonZeroU64, ValidationFail> {
    crate::pipeline::overlay::validate_header_policy(metadata)
        .map_err(ValidationFail::IvmAdmission)?;
    if zk_availability == PreparedIvmZkAvailability::RequireLocalBackend
        && metadata.mode & ivm::ivm_mode::ZK != 0
        && !(state.zk().pipa_r.enabled || state.zk().stark.enabled)
    {
        return Err(ValidationFail::IvmAdmission(
            iroha_data_model::executor::IvmAdmissionError::UnsupportedFeatureBits(
                ivm::ivm_mode::ZK,
            ),
        ));
    }
    let effective_cycles = crate::smartcontracts::ivm::validate_cycle_limits(
        metadata,
        state.pipeline().ivm_max_cycles_upper_bound,
        state.world().parameters().smart_contract().fuel(),
    )
    .map_err(ValidationFail::IvmAdmission)?;
    crate::pipeline::overlay::enforce_pre_execution_policy(
        state.pipeline().ivm_max_cycles_upper_bound,
        metadata,
    )
    .map_err(overlay_build_error_to_validation_fail)?;
    Ok(effective_cycles)
}
/// Apply the canonical first-release IVM admission policy to an already prepared program.
///
/// Preparation authenticates and predecodes the image, while this check binds local execution to
/// the node/governance limits and requires a locally enabled backend for ZK-mode execution. The
/// metadata-only interface prevents warm dispatch from rewalking authenticated opcode bytes.
pub(crate) fn validate_prepared_ivm_execution_policy<R: StateReadOnly>(
    state: &R,
    metadata: &ivm::ProgramMetadata,
) -> Result<std::num::NonZeroU64, ValidationFail> {
    validate_prepared_ivm_execution_policy_with_availability(
        state,
        metadata,
        PreparedIvmZkAvailability::RequireLocalBackend,
    )
}
// Proof-carrying execution uses the same deterministic header/resource limits, while native
// verification is selected by the governed on-chain verifier record rather than local proving
// availability toggles. `verify_ivm_proved_execution` performs that governed verification.
fn validate_governed_ivm_proved_execution_policy<R: StateReadOnly>(
    state: &R,
    metadata: &ivm::ProgramMetadata,
) -> Result<std::num::NonZeroU64, ValidationFail> {
    validate_prepared_ivm_execution_policy_with_availability(
        state,
        metadata,
        PreparedIvmZkAvailability::GovernedProof,
    )
}
#[derive(Clone, Debug)]
pub(crate) struct ContractRuntimeExecutionContext {
    pub(crate) contract_address: iroha_data_model::smart_contract::ContractAddress,
    pub(crate) contract_subject: AccountId,
    // Retained as canonical provenance for queued/nested calls. Authorization must never branch
    // on this value; caller metadata is canonicalized against WSV before this context is built.
    pub(crate) contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    pub(crate) entrypoint: String,
}
/// Immutable authorization selected before a contract invocation is decoded or executed.
///
/// This snapshot deliberately carries the permission name chosen from the validated artifact.
/// Apply paths must validate this exact value and must not derive a replacement from mutable
/// world state after the VM has queued effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContractEntrypointAuthorizationSnapshot {
    pub(crate) lifecycle_revision: u64,
    pub(crate) authority: AccountId,
    pub(crate) entrypoint: String,
    pub(crate) authorization: EntrypointAuthorizationV1,
    pub(crate) contract_address: iroha_data_model::smart_contract::ContractAddress,
    pub(crate) contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    pub(crate) contract_alias_binding: Option<crate::state::ContractAliasBindingRecord>,
    pub(crate) code_hash: iroha_crypto::Hash,
    parent: Option<Box<ContractEntrypointAuthorizationSnapshot>>,
}
impl ContractEntrypointAuthorizationSnapshot {
    /// Capture the exact live identity and selected artifact permission at dispatch time.
    pub(crate) fn new(
        authority: AccountId,
        entrypoint: String,
        authorization: EntrypointAuthorizationV1,
        identity: &code::BoundContractIdentity,
    ) -> Self {
        Self {
            lifecycle_revision: identity.lifecycle_revision,
            authority,
            entrypoint,
            authorization,
            contract_address: identity.contract_address.clone(),
            contract_alias: identity.contract_alias.clone(),
            contract_alias_binding: identity.contract_alias_binding.clone(),
            code_hash: identity.code_hash,
            parent: None,
        }
    }
    /// Attach the complete caller authorization chain for a nested invocation.
    #[must_use]
    pub(crate) fn with_parent(
        mut self,
        parent: Option<ContractEntrypointAuthorizationSnapshot>,
    ) -> Self {
        self.parent = parent.map(Box::new);
        self
    }
    /// Return whether this snapshot is the root or retains it in its caller chain.
    pub(crate) fn descends_from(&self, root: &Self) -> bool {
        self == root
            || self
                .parent
                .as_deref()
                .is_some_and(|parent| parent.descends_from(root))
    }
    /// Return whether this snapshot represents a top-level invocation.
    pub(crate) fn is_root(&self) -> bool {
        self.parent.is_none()
    }
    /// Return whether `path` is owned by the exact contract instance captured by this snapshot.
    ///
    /// Durable contract state is namespaced by the immutable contract address rather than by a
    /// movable alias. Lifecycle markers use the same address digest in their reserved namespace.
    /// Keeping this check on the snapshot prevents a valid permission for one contract from being
    /// attached to a durable write targeting another contract's namespace.
    pub(crate) fn owns_durable_state_path(&self, path: &StatePath) -> bool {
        let address = self.contract_address.to_string();
        let digest = hex::encode(iroha_crypto::Hash::new(address.as_bytes()).as_ref());
        let path: &str = path.as_ref();
        path.strip_prefix("sc/")
            .and_then(|suffix| suffix.strip_prefix(&digest))
            .is_some_and(|suffix| suffix.starts_with('/'))
            || path == code::contract_lifecycle_state_key(&self.contract_address).as_ref()
    }
    /// Validate the immutable caller relationship between every adjacent invocation.
    ///
    /// A nested contract executes as the subject account derived from its immediate caller's
    /// address. Merely retaining an arbitrary ancestor is insufficient: without this adjacency
    /// check a forged leaf could borrow an unrelated caller's permission while still embedding a
    /// valid root snapshot somewhere in its chain.
    pub(crate) fn validate_chain_structure(
        &self,
        world: &impl WorldReadOnly,
    ) -> Result<(), ValidationFail> {
        let Some(parent) = self.parent.as_deref() else {
            return Ok(());
        };
        parent.validate_chain_structure(world)?;
        let parent_subject = world
            .contract_subject_bindings()
            .get(&parent.contract_address)
            .ok_or_else(|| {
                ValidationFail::NotPermitted(format!(
                    "parent contract instance `{}` has no subject binding",
                    parent.contract_address
                ))
            })?;
        parent_subject
            .validate_for(&parent.contract_address)
            .map_err(ValidationFail::NotPermitted)?;
        if self.authority != parent_subject.subject {
            return Err(ValidationFail::NotPermitted(
                "nested contract authorization caller does not match its immediate parent contract"
                    .to_owned(),
            ));
        }
        Ok(())
    }
    /// Revalidate the captured caller permission and the exact forward/reverse live binding.
    pub(crate) fn validate(
        &self,
        world: &impl WorldReadOnly,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        self.validate_chain_structure(world)?;
        self.validate_live(world)
    }
    /// Revalidate the snapshot and require every captured invocation to be executable now.
    pub(crate) fn validate_at_height(
        &self,
        world: &impl WorldReadOnly,
        execution_height: u64,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        self.validate(world)?;
        self.validate_execution_allowed(world, execution_height)
            .map_err(Into::into)
    }
    fn validate_execution_allowed(
        &self,
        world: &impl WorldReadOnly,
        execution_height: u64,
    ) -> Result<(), ValidationFail> {
        if let Some(parent) = self.parent.as_deref() {
            parent.validate_execution_allowed(world, execution_height)?;
        }
        code::ensure_contract_execution_allowed(world, &self.contract_address, execution_height)
            .map_err(ValidationFail::NotPermitted)
    }
    fn validate_live(
        &self,
        world: &impl WorldReadOnly,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        root_scope::ensure_committed_contract_scope(world, &self.contract_address)?;
        if let Some(parent) = self.parent.as_deref() {
            parent.validate_live(world)?;
        }
        let live_code_hash = world
            .contract_instances()
            .get(&self.contract_address)
            .copied()
            .ok_or_else(|| {
                ValidationFail::NotPermitted(format!(
                    "contract instance `{}` is no longer active",
                    self.contract_address
                ))
            })?;
        if live_code_hash != self.code_hash {
            return Err(ValidationFail::NotPermitted(format!(
                "contract instance `{}` changed code binding while its call was prepared: captured `{}`, live `{}`",
                self.contract_address, self.code_hash, live_code_hash
            )).into());
        }
        let binding = world
            .contract_subject_bindings()
            .get(&self.contract_address)
            .ok_or_else(|| {
                ValidationFail::NotPermitted(
                    "contract lifecycle binding disappeared during execution".to_owned(),
                )
            })?;
        binding
            .validate_for(&self.contract_address)
            .map_err(ValidationFail::NotPermitted)?;
        if binding.lifecycle.revision != self.lifecycle_revision {
            return Err(ValidationFail::NotPermitted(format!(
                "contract instance `{}` changed lifecycle revision while its call was prepared",
                self.contract_address
            ))
            .into());
        }
        let live_alias_binding = world
            .contract_alias_bindings()
            .get(&self.contract_address)
            .cloned();
        if live_alias_binding != self.contract_alias_binding {
            return Err(ValidationFail::NotPermitted(format!(
                "contract instance `{}` changed alias binding while its call was prepared",
                self.contract_address
            ))
            .into());
        }
        let reverse_alias = live_alias_binding
            .as_ref()
            .map(|binding| binding.alias.clone());
        if reverse_alias != self.contract_alias {
            return Err(ValidationFail::NotPermitted(format!(
                "contract instance `{}` has inconsistent captured alias binding metadata",
                self.contract_address
            ))
            .into());
        }
        if let Some(alias) = self.contract_alias.as_ref()
            && world.contract_aliases().get(alias) != Some(&self.contract_address)
        {
            return Err(ValidationFail::NotPermitted(format!(
                "contract instance `{}` has an inconsistent live alias binding",
                self.contract_address
            ))
            .into());
        }
        if world.contract_aliases().iter().any(|(alias, address)| {
            address == &self.contract_address && Some(alias) != self.contract_alias.as_ref()
        }) {
            return Err(ValidationFail::NotPermitted(format!(
                "contract instance `{}` has a non-canonical forward alias binding",
                self.contract_address
            ))
            .into());
        }
        enforce_named_contract_entrypoint_authorization(
            world,
            &self.authority,
            &self.contract_address,
            &self.entrypoint,
            &self.authorization,
        )
        .map_err(Into::into)
    }
    /// Validate the snapshot and require the apply-time caller to be the captured caller.
    pub(crate) fn validate_for_authority(
        &self,
        world: &impl WorldReadOnly,
        authority: &AccountId,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        if authority != &self.authority {
            return Err(ValidationFail::NotPermitted(
                "prepared contract authorization caller changed before apply".to_owned(),
            )
            .into());
        }
        self.validate(world)
    }
    /// Validate the apply-time caller and require the captured invocation chain to be executable.
    pub(crate) fn validate_for_authority_at_height(
        &self,
        world: &impl WorldReadOnly,
        authority: &AccountId,
        execution_height: u64,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        if authority != &self.authority {
            return Err(ValidationFail::NotPermitted(
                "prepared contract authorization caller changed before apply".to_owned(),
            )
            .into());
        }
        self.validate_at_height(world, execution_height)
    }
}
/// Reject binding mutations emitted from a lifecycle hook before executor dispatch.
///
/// This guard runs ahead of both initial and user-provided executors and is shared by owned and
/// borrowed overlay paths. Without it, a hook could deactivate/reactivate its address and let the
/// completion tombstone erase the newly staged lifecycle record.
pub(crate) fn ensure_lifecycle_hook_cannot_mutate_contract_binding(
    context: Option<&ContractRuntimeExecutionContext>,
    instruction: &InstructionBox,
) -> Result<(), ValidationFail> {
    let Some(context) = context else {
        return Ok(());
    };
    if !matches!(
        context.entrypoint.as_str(),
        "hajimari" | "始まり" | "kaizen" | "改善"
    ) {
        return Ok(());
    }
    let instruction = instruction.as_any();
    if instruction
        .downcast_ref::<iroha_data_model::isi::smart_contract_code::ActivateContractInstance>()
        .is_none()
        && instruction
            .downcast_ref::<iroha_data_model::isi::smart_contract_code::CommitContractDeployment>()
            .is_none()
        && instruction
            .downcast_ref::<iroha_data_model::isi::smart_contract_code::DeactivateContractInstance>(
            )
            .is_none()
    {
        return Ok(());
    }
    Err(ValidationFail::NotPermitted(format!(
        "lifecycle entrypoint `{}` cannot activate or deactivate contract bindings",
        context.entrypoint
    )))
}
#[derive(Clone, Debug)]
/// Parsed contract dispatch metadata used to configure IVM execution.
pub struct ContractCallExecutionContext {
    pub(crate) contract_address: Option<iroha_data_model::smart_contract::ContractAddress>,
    pub(crate) contract_subject: Option<AccountId>,
    pub(crate) contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    pub(crate) entrypoint: Option<String>,
    pub(crate) entrypoint_pc: Option<u64>,
    pub(crate) entrypoint_authorization: EntrypointAuthorizationV1,
    pub(crate) args: Json,
    pub(crate) argument_record: Option<ivm::PreparedArgumentRecord>,
}
/// Cache-independent inputs resolved for one deployed-contract invocation.
///
/// Keeping the prepared summary owned lets trigger execution release the outer IVM cache mutex
/// before guest-emitted instructions are applied and potentially invoke another VM-backed trigger.
#[derive(Debug)]
pub(crate) struct ResolvedContractInvocation {
    identity: code::BoundContractIdentity,
    contract_subject: AccountId,
    summary: ProgramSummary,
}
/// Effects and metering information returned by one deployed-contract invocation.
#[derive(Debug)]
pub(crate) struct ContractInvocationOutcome {
    /// Gas consumed by the VM, including execution that ended in a later artifact error.
    pub(crate) gas_used: u64,
    /// Guest-emitted instructions that were successfully applied.
    pub(crate) executed_instructions: Vec<InstructionBox>,
    /// Trigger-local NFT sequence after successful guest execution.
    pub(crate) next_nft_sequence: Option<u64>,
}
impl ContractCallExecutionContext {
    pub(crate) fn runtime_context(&self) -> Option<ContractRuntimeExecutionContext> {
        let contract_address = self.contract_address.clone()?;
        let contract_subject = self.contract_subject.clone()?;
        Some(ContractRuntimeExecutionContext {
            contract_subject,
            contract_address,
            contract_alias: self.contract_alias.clone(),
            entrypoint: self.entrypoint.clone()?,
        })
    }
    pub(crate) fn bind_runtime_identity(
        &mut self,
        identity: code::BoundContractIdentity,
        contract_subject: AccountId,
    ) {
        self.contract_address = Some(identity.contract_address);
        self.contract_subject = Some(contract_subject);
        self.contract_alias = identity.contract_alias;
    }
    pub(crate) fn entrypoint_pc(&self) -> Option<u64> {
        self.entrypoint_pc
    }
    pub(crate) fn entrypoint_authorization(&self) -> &EntrypointAuthorizationV1 {
        &self.entrypoint_authorization
    }
    pub(crate) fn args(&self) -> &Json {
        &self.args
    }
    #[cfg(test)]
    pub(crate) fn argument_record(&self) -> Option<&[u8]> {
        self.argument_record
            .as_ref()
            .map(ivm::PreparedArgumentRecord::canonical_bytes)
    }
    pub(crate) fn prepared_argument_record(&self) -> Option<&ivm::PreparedArgumentRecord> {
        self.argument_record.as_ref()
    }
}
pub(crate) fn encode_contract_argument_record(
    schema: Option<&ivm::EntrypointArgumentSchemaV1>,
    payload: Option<&Json>,
) -> Result<Option<Vec<u8>>, ValidationFail> {
    match (schema, payload) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(ValidationFail::NotPermitted(
            "zero-parameter entrypoint must not receive a payload".to_owned(),
        )),
        (Some(_), None) => Err(ValidationFail::NotPermitted(
            "parameterized entrypoint requires a payload".to_owned(),
        )),
        (Some(schema), Some(payload)) => {
            ivm_abi::arguments::encode_argument_record_from_json(schema, payload)
                .map(Some)
                .map_err(|error| {
                    ValidationFail::NotPermitted(format!(
                        "contract payload does not match the entrypoint argument schema: {error}"
                    ))
                })
        }
    }
}
fn prepare_contract_argument_record_from_json(
    schema: Option<&ivm::EntrypointArgumentSchemaV1>,
    payload: Option<&Json>,
    gas_limit: u64,
) -> Result<
    Option<ivm::PreparedArgumentRecord>,
    crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
> {
    let canonical = encode_contract_argument_record(schema, payload)?;
    match (schema, canonical) {
        (None, None) => Ok(None),
        (Some(schema), Some(canonical)) => {
            ivm::prepare_argument_record_with_gas_limit(schema, Arc::from(canonical), gas_limit)
                .map(Some)
                .map_err(|error| {
                    crate::execution_attempt::vm_attempt_error(error, |error| {
                        ValidationFail::NotPermitted(format!(
                            "failed to prepare canonical contract arguments: {error}"
                        ))
                    })
                })
        }
        _ => Err(ValidationFail::InternalError(
            "contract argument schema and canonical record diverged".to_owned(),
        )
        .into()),
    }
}
fn prepare_validated_contract_argument_record(
    schema: Option<&ivm::EntrypointArgumentSchemaV1>,
    arguments: Option<&[u8]>,
    gas_limit: u64,
) -> Result<
    Option<ivm::PreparedArgumentRecord>,
    crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
> {
    match (schema, arguments) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(ValidationFail::NotPermitted(
            "zero-parameter entrypoint must not carry an argument record".to_owned(),
        )
        .into()),
        (Some(_), None) => Err(ValidationFail::NotPermitted(
            "parameterized entrypoint requires an argument record".to_owned(),
        )
        .into()),
        (Some(schema), Some(arguments)) => ivm::prepare_argument_record_with_gas_limit(
            schema,
            Arc::<[u8]>::from(arguments),
            gas_limit,
        )
        .map(Some)
        .map_err(|error| {
            crate::execution_attempt::vm_attempt_error(error, |error| {
                ValidationFail::NotPermitted(format!("invalid contract argument record: {error}"))
            })
        }),
    }
}
type ResolvedContractEntrypoint = (
    u64,
    EntrypointAuthorizationV1,
    Option<ivm::EntrypointArgumentSchemaV1>,
);
#[cfg(test)]
fn resolve_callable_contract_entrypoint(
    bytecode: &[u8],
    selector: &str,
    interface_required_message: &'static str,
) -> Result<ResolvedContractEntrypoint, ValidationFail> {
    let parsed = ivm::ProgramMetadata::parse(bytecode).map_err(|err| {
        ValidationFail::NotPermitted(format!(
            "invalid contract artifact for contract call dispatch: {err}"
        ))
    })?;
    let prefix_len = parsed.prefix_len() as u64;
    let contract_interface = parsed
        .contract_interface
        .as_ref()
        .ok_or_else(|| ValidationFail::NotPermitted(interface_required_message.to_owned()))?;
    let descriptor = contract_interface
        .entrypoints
        .iter()
        .find(|candidate| candidate.name == selector)
        .ok_or_else(|| {
            ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
        })?;
    let permission = callable_contract_entrypoint_authorization(descriptor, selector)?;
    Ok((
        prefix_len + descriptor.entry_pc,
        permission,
        descriptor.argument_schema.clone(),
    ))
}
fn resolve_raw_contract_entrypoint(
    bytecode: &[u8],
    selector: &str,
    interface_required_message: &'static str,
) -> Result<ResolvedContractEntrypoint, ValidationFail> {
    let parsed = ivm::ProgramMetadata::parse(bytecode).map_err(|err| {
        ValidationFail::NotPermitted(format!(
            "invalid contract artifact for contract call dispatch: {err}"
        ))
    })?;
    let prefix_len = parsed.prefix_len() as u64;
    let contract_interface = parsed
        .contract_interface
        .as_ref()
        .ok_or_else(|| ValidationFail::NotPermitted(interface_required_message.to_owned()))?;
    let descriptor = contract_interface
        .entrypoints
        .iter()
        .find(|candidate| candidate.name == selector)
        .ok_or_else(|| {
            ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
        })?;
    let permission = raw_contract_entrypoint_authorization(descriptor, selector)?;
    Ok((
        prefix_len + descriptor.entry_pc,
        permission,
        descriptor.argument_schema.clone(),
    ))
}
fn resolve_prepared_contract_entrypoint(
    contract: &ivm::PreparedContract,
    selector: &str,
) -> Result<ResolvedContractEntrypoint, ValidationFail> {
    let descriptor = contract.entrypoint_descriptor(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
    })?;
    let entrypoint_pc = contract.entrypoint_pc(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!(
            "contract entrypoint `{selector}` has no validated program counter"
        ))
    })?;
    reject_unavailable_private_input_entrypoint(contract, selector)?;
    let permission = callable_contract_entrypoint_authorization(descriptor, selector)?;
    Ok((
        entrypoint_pc,
        permission,
        descriptor.argument_schema.clone(),
    ))
}
fn resolve_prepared_contract_view_entrypoint(
    contract: &ivm::PreparedContract,
    selector: &str,
) -> Result<ResolvedContractEntrypoint, ValidationFail> {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    let descriptor = contract.entrypoint_descriptor(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
    })?;
    if descriptor.kind != EntryPointKind::View {
        return Err(ValidationFail::NotPermitted(format!(
            "contract entrypoint `{selector}` is not a read-only view"
        )));
    }
    let entrypoint_pc = contract.entrypoint_pc(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!(
            "contract entrypoint `{selector}` has no validated program counter"
        ))
    })?;
    reject_unavailable_private_input_entrypoint(contract, selector)?;
    Ok((
        entrypoint_pc,
        descriptor.authorization.clone(),
        descriptor.argument_schema.clone(),
    ))
}
fn resolve_prepared_raw_contract_entrypoint(
    contract: &ivm::PreparedContract,
    selector: &str,
) -> Result<ResolvedContractEntrypoint, ValidationFail> {
    let descriptor = contract.entrypoint_descriptor(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
    })?;
    let entrypoint_pc = contract.entrypoint_pc(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!(
            "contract entrypoint `{selector}` has no validated program counter"
        ))
    })?;
    reject_unavailable_private_input_entrypoint(contract, selector)?;
    let permission = raw_contract_entrypoint_authorization(descriptor, selector)?;
    Ok((
        entrypoint_pc,
        permission,
        descriptor.argument_schema.clone(),
    ))
}
fn reject_unavailable_private_input_entrypoint(
    contract: &ivm::PreparedContract,
    selector: &str,
) -> Result<(), ValidationFail> {
    // ABI V1 deliberately has no consensus transport for private witnesses.
    // Any future proof-carrying invocation ABI must bind the seiyaku address
    // and code hash, selector, public arguments, authority and chain, state
    // root and exact read/write sets, outputs and events, gas schedule and
    // ceiling, and circuit and verifier-key versions. Raw private witnesses
    // must never enter signed transport or deterministic validator replay.
    match contract.entrypoint_requires_private_inputs(selector) {
        Some(false) => Ok(()),
        Some(true) => Err(ValidationFail::NotPermitted(format!(
            "seiyaku declaration `{selector}` reads Secret<T> private witnesses; ABI V1 consensus execution rejects raw witness transport until a complete proof-carrying invocation statement replaces deterministic replay"
        ))),
        None => Err(ValidationFail::InternalError(format!(
            "validated seiyaku selector `{selector}` is missing its bytecode-derived private-input policy"
        ))),
    }
}
/// Resolve authorization for a top-level deployed-contract transaction entrypoint.
pub(crate) fn callable_contract_entrypoint_authorization(
    descriptor: &ivm::EmbeddedEntrypointDescriptor,
    selector: &str,
) -> Result<EntrypointAuthorizationV1, ValidationFail> {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    match descriptor.kind {
        EntryPointKind::Kotoage => Ok(descriptor.authorization.clone()),
        EntryPointKind::View => Err(ValidationFail::NotPermitted(format!(
            "contract entrypoint `{selector}` is read-only and cannot be invoked as a transaction"
        ))),
        EntryPointKind::Hajimari | EntryPointKind::Kaizen => {
            Ok(EntrypointAuthorizationV1::RuntimeLifecycle)
        }
    }
}
/// Resolve authorization for raw-IVM source dispatch.
///
/// Lifecycle hooks require a consensus-bound deployed-instance transition and therefore can only
/// be selected through `Executable::ContractCall`.
pub(crate) fn raw_contract_entrypoint_authorization(
    descriptor: &ivm::EmbeddedEntrypointDescriptor,
    selector: &str,
) -> Result<EntrypointAuthorizationV1, ValidationFail> {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    match descriptor.kind {
        EntryPointKind::Kotoage => Ok(descriptor.authorization.clone()),
        EntryPointKind::View => Err(ValidationFail::NotPermitted(format!(
            "contract entrypoint `{selector}` is read-only and cannot be invoked as a transaction"
        ))),
        EntryPointKind::Hajimari | EntryPointKind::Kaizen => {
            Err(ValidationFail::NotPermitted(format!(
                "`{selector}` is a hajimari/始まり or kaizen/改善 entrypoint and requires a top-level deployed ContractCall"
            )))
        }
    }
}
/// Resolve authorization for an ordinary nested contract call.
///
/// Nested calls may invoke `kotoage`/`言挙げ` and `view` entrypoints, but lifecycle
/// hooks remain reserved for the deployment and `kaizen`/`改善` state machine.
pub(crate) fn nested_contract_entrypoint_authorization(
    descriptor: &ivm::EmbeddedEntrypointDescriptor,
    selector: &str,
) -> Result<EntrypointAuthorizationV1, ValidationFail> {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    match descriptor.kind {
        EntryPointKind::Kotoage | EntryPointKind::View => Ok(descriptor.authorization.clone()),
        EntryPointKind::Hajimari | EntryPointKind::Kaizen => {
            Err(ValidationFail::NotPermitted(format!(
                "`{selector}` is a hajimari/始まり or kaizen/改善 entrypoint and cannot be invoked by a nested call"
            )))
        }
    }
}
fn is_self_describing_contract(bytecode: &[u8]) -> bool {
    ivm::ProgramMetadata::parse(bytecode)
        .ok()
        .and_then(|parsed| parsed.contract_interface)
        .is_some()
}
enum ContractDispatchSource<'a> {
    Bytecode(&'a [u8]),
    Prepared(&'a ivm::PreparedContract),
}
impl ContractDispatchSource<'_> {
    fn resolve(
        &self,
        selector: &str,
        interface_required_message: &'static str,
    ) -> Result<ResolvedContractEntrypoint, ValidationFail> {
        match self {
            Self::Bytecode(bytecode) => {
                resolve_raw_contract_entrypoint(bytecode, selector, interface_required_message)
            }
            Self::Prepared(contract) => {
                resolve_prepared_raw_contract_entrypoint(contract, selector)
            }
        }
    }
    fn is_self_describing(&self) -> bool {
        match self {
            Self::Bytecode(bytecode) => is_self_describing_contract(bytecode),
            Self::Prepared(_) => true,
        }
    }
}
#[cfg(test)]
pub(crate) fn parse_contract_call_execution_context(
    metadata: &Metadata,
    bytecode: &[u8],
) -> Result<
    Option<ContractCallExecutionContext>,
    crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
> {
    parse_contract_call_execution_context_from_source(
        metadata,
        ContractDispatchSource::Bytecode(bytecode),
        ContractArgumentSource::Metadata,
        u64::MAX,
    )
}
pub(crate) fn parse_prepared_contract_call_execution_context(
    metadata: &Metadata,
    contract: &ivm::PreparedContract,
    gas_limit: u64,
) -> Result<
    Option<ContractCallExecutionContext>,
    crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
> {
    parse_contract_call_execution_context_from_source(
        metadata,
        ContractDispatchSource::Prepared(contract),
        ContractArgumentSource::Metadata,
        gas_limit,
    )
}
/// Read and normalize the explicitly selected contract entrypoint.
///
/// Callers use this cheap metadata-only step to authorize a selector before
/// argument records are decoded or materialized.
pub(crate) fn requested_contract_entrypoint(
    metadata: &Metadata,
) -> Result<Option<String>, ValidationFail> {
    let entrypoint = metadata
        .get("contract_entrypoint")
        .map(|raw| {
            raw.try_into_any_norito::<String>().map_err(|err| {
                ValidationFail::NotPermitted(format!("invalid contract_entrypoint metadata: {err}"))
            })
        })
        .transpose()?
        .map(|value| value.trim().to_owned());
    if entrypoint.as_deref().is_some_and(str::is_empty) {
        return Err(ValidationFail::NotPermitted(
            "contract_entrypoint must not be empty".to_owned(),
        ));
    }
    Ok(entrypoint)
}
/// Require a by-reference invocation to match the exact live code binding authorized by its signer.
pub(crate) fn ensure_contract_invocation_code_hash(
    invocation: &ContractInvocation,
    actual_code_hash: iroha_crypto::Hash,
) -> Result<(), ValidationFail> {
    if invocation.expected_code_hash != actual_code_hash {
        return Err(ValidationFail::NotPermitted(format!(
            "contract instance `{}` is bound to code `{actual_code_hash}`, not signed expected code `{}`",
            invocation.contract_address, invocation.expected_code_hash
        )));
    }
    Ok(())
}
/// Bind a top-level by-reference contract invocation to the exact human-reviewable
/// transaction metadata carried by the same signed payload.
///
/// Wallets can validate canonical JSON metadata without embedding a Kotodama ABI
/// encoder. Consensus therefore reconstructs the schema-bound argument record
/// from that metadata and refuses any executable whose opaque argument bytes
/// encode a different payload. A `contract_alias` entry must name the address's
/// `live_alias`, so indexers can attribute the call to that alias.
///
/// Metadata that carries any of these keys must carry the full identity binding
/// (address, code hash, entrypoint). Other keys, such as `contract_module` or
/// `contract_event_*`, are never checked and must not be trusted by readers.
fn ensure_contract_invocation_metadata_binding(
    invocation: &ContractInvocation,
    metadata: &Metadata,
    contract: &ivm::PreparedContract,
    live_alias: Option<&iroha_data_model::smart_contract::ContractAlias>,
) -> Result<(), ValidationFail> {
    let carries_reviewable_binding = [
        "contract_address",
        "contract_alias",
        "contract_code_hash",
        "contract_entrypoint",
        "contract_payload",
    ]
    .iter()
    .any(|key| metadata.get(*key).is_some());
    if !carries_reviewable_binding {
        return Ok(());
    }
    let metadata_address = requested_contract_address(metadata)?.ok_or_else(|| {
        ValidationFail::NotPermitted(
            "top-level ContractCall requires contract_address metadata".to_owned(),
        )
    })?;
    if metadata_address != invocation.contract_address {
        return Err(ValidationFail::NotPermitted(
            "top-level ContractCall address differs from contract_address metadata".to_owned(),
        ));
    }
    if let Some(metadata_alias) = requested_contract_alias(metadata)?
        && live_alias != Some(&metadata_alias)
    {
        return Err(ValidationFail::NotPermitted(format!(
            "contract_alias metadata `{metadata_alias}` is not the live alias of `{}`",
            invocation.contract_address
        )));
    }
    let code_hash_literal = metadata
        .get("contract_code_hash")
        .ok_or_else(|| {
            ValidationFail::NotPermitted(
                "top-level ContractCall requires contract_code_hash metadata".to_owned(),
            )
        })?
        .try_into_any_norito::<String>()
        .map_err(|error| {
            ValidationFail::NotPermitted(format!("invalid contract_code_hash metadata: {error}"))
        })?;
    if code_hash_literal.trim() != code_hash_literal {
        return Err(ValidationFail::NotPermitted(
            "contract_code_hash metadata must use canonical spelling".to_owned(),
        ));
    }
    let metadata_code_hash = code_hash_literal
        .parse::<iroha_crypto::Hash>()
        .map_err(|error| {
            ValidationFail::NotPermitted(format!(
                "invalid contract_code_hash metadata literal `{code_hash_literal}`: {error}"
            ))
        })?;
    if metadata_code_hash != invocation.expected_code_hash {
        return Err(ValidationFail::NotPermitted(
            "top-level ContractCall code hash differs from contract_code_hash metadata".to_owned(),
        ));
    }
    let metadata_entrypoint = requested_contract_entrypoint(metadata)?.ok_or_else(|| {
        ValidationFail::NotPermitted(
            "top-level ContractCall requires contract_entrypoint metadata".to_owned(),
        )
    })?;
    if metadata_entrypoint != invocation.entrypoint {
        return Err(ValidationFail::NotPermitted(
            "top-level ContractCall entrypoint differs from contract_entrypoint metadata"
                .to_owned(),
        ));
    }
    let descriptor = contract
        .entrypoint_descriptor(&metadata_entrypoint)
        .ok_or_else(|| {
            ValidationFail::NotPermitted(format!(
                "unknown contract entrypoint `{metadata_entrypoint}`"
            ))
        })?;
    let payload = metadata.get("contract_payload");
    let canonical_arguments =
        encode_contract_argument_record(descriptor.argument_schema.as_ref(), payload)?;
    if canonical_arguments.as_deref() != invocation.arguments.as_deref() {
        return Err(ValidationFail::NotPermitted(
            "top-level ContractCall arguments differ from the canonical contract_payload metadata"
                .to_owned(),
        ));
    }
    Ok(())
}
pub(crate) fn requested_contract_address(
    metadata: &Metadata,
) -> Result<Option<iroha_data_model::smart_contract::ContractAddress>, ValidationFail> {
    metadata
        .get("contract_address")
        .map(|raw| {
            raw.try_into_any_norito::<String>().map_err(|err| {
                ValidationFail::NotPermitted(format!("invalid contract_address metadata: {err}"))
            })
        })
        .transpose()?
        .map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(ValidationFail::NotPermitted(
                    "contract_address must not be empty".to_owned(),
                ));
            }
            trimmed.parse().map_err(|err| {
                ValidationFail::NotPermitted(format!(
                    "invalid contract_address metadata literal `{trimmed}`: {err}"
                ))
            })
        })
        .transpose()
}
fn requested_contract_alias(
    metadata: &Metadata,
) -> Result<Option<iroha_data_model::smart_contract::ContractAlias>, ValidationFail> {
    metadata
        .get("contract_alias")
        .map(|raw| {
            raw.try_into_any_norito::<String>().map_err(|err| {
                ValidationFail::NotPermitted(format!("invalid contract_alias metadata: {err}"))
            })
        })
        .transpose()?
        .map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(ValidationFail::NotPermitted(
                    "contract_alias must not be empty".to_owned(),
                ));
            }
            trimmed.parse().map_err(|err| {
                ValidationFail::NotPermitted(format!(
                    "invalid contract_alias metadata literal `{trimmed}`: {err}"
                ))
            })
        })
        .transpose()
}
/// Resolve raw-IVM identity metadata exclusively through live world-state bindings.
///
/// User metadata selects an identity; it never supplies the trusted alias or
/// contract subject used by runtime authorization exceptions and state scope.
pub(crate) fn resolve_raw_contract_runtime_identity(
    world: &impl WorldReadOnly,
    code_hash: iroha_crypto::Hash,
    metadata: &Metadata,
) -> Result<Option<code::BoundContractIdentity>, ValidationFail> {
    let requested_address = requested_contract_address(metadata)?;
    let requested_alias = requested_contract_alias(metadata)?;
    let alias_address = requested_alias
        .as_ref()
        .map(|alias| {
            world.contract_aliases().get(alias).cloned().ok_or_else(|| {
                ValidationFail::NotPermitted(format!(
                    "contract alias `{alias}` is not bound in live state"
                ))
            })
        })
        .transpose()?;
    if let (Some(requested), Some(resolved)) = (&requested_address, &alias_address)
        && requested != resolved
    {
        return Err(ValidationFail::NotPermitted(format!(
            "contract alias metadata resolves to `{resolved}`, not requested address `{requested}`"
        )));
    }
    let Some(contract_address) = requested_address.or(alias_address) else {
        return Ok(None);
    };
    let bound_code_hash = world
        .contract_instances()
        .get(&contract_address)
        .copied()
        .ok_or_else(|| {
            ValidationFail::NotPermitted(format!(
                "contract instance `{contract_address}` not found in live state"
            ))
        })?;
    if bound_code_hash != code_hash {
        return Err(ValidationFail::NotPermitted(format!(
            "contract instance `{contract_address}` is bound to code `{bound_code_hash}`, not executing code `{code_hash}`"
        )));
    }
    let live_alias_binding = world
        .contract_alias_bindings()
        .get(&contract_address)
        .cloned();
    let live_alias = live_alias_binding
        .as_ref()
        .map(|binding| binding.alias.clone());
    if let Some(alias) = live_alias.as_ref()
        && world.contract_aliases().get(alias) != Some(&contract_address)
    {
        return Err(ValidationFail::NotPermitted(format!(
            "contract instance `{contract_address}` has an inconsistent live alias binding"
        )));
    }
    if requested_alias.as_ref().is_some_and(|requested| {
        live_alias.as_ref() != Some(requested)
            || world.contract_aliases().get(requested) != Some(&contract_address)
    }) {
        return Err(ValidationFail::NotPermitted(format!(
            "contract alias metadata does not match the live alias for `{contract_address}`"
        )));
    }
    Ok(Some(code::BoundContractIdentity {
        lifecycle_revision: world
            .contract_subject_bindings()
            .get(&contract_address)
            .ok_or_else(|| {
                ValidationFail::NotPermitted(
                    "contract instance has no lifecycle binding".to_owned(),
                )
            })?
            .lifecycle
            .revision,
        contract_address,
        contract_alias: live_alias,
        contract_alias_binding: live_alias_binding,
        code_hash,
    }))
}
/// Resolve the mandatory live identity for a selected raw-IVM contract entrypoint.
///
/// A selected entrypoint is contract dispatch, even when its descriptor has no named
/// permission. It therefore cannot execute with an anonymous/state-free runtime identity.
pub(crate) fn require_raw_contract_runtime_identity(
    world: &impl WorldReadOnly,
    code_hash: iroha_crypto::Hash,
    metadata: &Metadata,
) -> Result<code::BoundContractIdentity, ValidationFail> {
    resolve_raw_contract_runtime_identity(world, code_hash, metadata)?.ok_or_else(|| {
        ValidationFail::NotPermitted(
            "raw-IVM contract entrypoint dispatch requires a live contract_address or contract_alias binding"
                .to_owned(),
        )
    })
}
#[derive(Clone, Copy)]
enum ContractArgumentSource<'a> {
    Metadata,
    TriggerEvent(&'a Json),
    SchemaOnly,
}
/// Resolve a self-describing IVM trigger callback and bind the current event
/// arguments to its compiler-emitted schema.
///
/// Trigger actions select the callback with `contract_entrypoint` metadata, but their payload is
/// supplied by the event that fired the trigger. The payload is converted here, once, into the same
/// schema-bound canonical Norito record used by ordinary contract calls. A fixed `contract_payload`
/// in trigger metadata is rejected so it cannot shadow the signed event arguments.
pub(crate) fn parse_prepared_trigger_call_execution_context(
    metadata: &Metadata,
    contract: &ivm::PreparedContract,
    event_args: &Json,
    gas_limit: u64,
) -> Result<
    ContractCallExecutionContext,
    crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
> {
    parse_contract_call_execution_context_from_source(
        metadata,
        ContractDispatchSource::Prepared(contract),
        ContractArgumentSource::TriggerEvent(event_args),
        gas_limit,
    )?
    .ok_or_else(|| {
        ValidationFail::NotPermitted(
            "self-describing IVM trigger action did not resolve a callback".to_owned(),
        )
    })
    .map_err(Into::into)
}
/// Bind a deployed trigger callback to its firing event using the signed schema.
///
/// A registered callback cannot shadow event arguments with a fixed call record.
/// Zero-argument callbacks still receive the event through `context::trigger_event`.
pub(crate) fn parse_prepared_trigger_invocation_execution_context(
    invocation: &ContractInvocation,
    contract: &ivm::PreparedContract,
    contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    contract_subject: AccountId,
    event_args: &Json,
    gas_limit: u64,
) -> Result<
    ContractCallExecutionContext,
    crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
> {
    if invocation.arguments.is_some() {
        return Err(ValidationFail::NotPermitted(
            "trigger callbacks must take arguments from the firing event, not a fixed call record"
                .to_owned(),
        )
        .into());
    }
    let selector = invocation.entrypoint.as_str();
    let (entrypoint_pc, entrypoint_authorization, argument_schema) =
        resolve_prepared_raw_contract_entrypoint(contract, selector)?;
    let argument_record = prepare_contract_argument_record_from_json(
        argument_schema.as_ref(),
        argument_schema.as_ref().map(|_| event_args),
        gas_limit,
    )?;
    Ok(ContractCallExecutionContext {
        contract_address: Some(invocation.contract_address.clone()),
        contract_subject: Some(contract_subject),
        contract_alias,
        entrypoint: Some(selector.to_owned()),
        entrypoint_pc: Some(entrypoint_pc),
        entrypoint_authorization,
        args: event_args.clone(),
        argument_record,
    })
}
/// Validate trigger callback selection at registration without fabricating an
/// event payload for a parameterized callback.
pub(crate) fn validate_trigger_call_execution_context(
    metadata: &Metadata,
    bytecode: &[u8],
) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
    parse_contract_call_execution_context_from_source(
        metadata,
        ContractDispatchSource::Bytecode(bytecode),
        ContractArgumentSource::SchemaOnly,
        u64::MAX,
    )?
    .ok_or_else(|| {
        ValidationFail::NotPermitted(
            "self-describing IVM trigger action did not resolve a callback".to_owned(),
        )
    })?;
    Ok(())
}
fn parse_contract_call_execution_context_from_source(
    metadata: &Metadata,
    source: ContractDispatchSource<'_>,
    argument_source: ContractArgumentSource<'_>,
    gas_limit: u64,
) -> Result<
    Option<ContractCallExecutionContext>,
    crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
> {
    let contract_address = requested_contract_address(metadata)?;
    let contract_alias = requested_contract_alias(metadata)?;
    let entrypoint = requested_contract_entrypoint(metadata)?;
    let metadata_payload = metadata.get("contract_payload").cloned();
    if !matches!(argument_source, ContractArgumentSource::Metadata) && metadata_payload.is_some() {
        return Err(ValidationFail::NotPermitted(
            "IVM trigger actions must take arguments from the triggering event, not contract_payload metadata"
                .to_owned(),
        ).into());
    }
    let (entrypoint, entrypoint_pc, entrypoint_authorization, argument_schema) =
        if let Some(selector) = entrypoint.as_deref() {
            let (entrypoint_pc, entrypoint_authorization, argument_schema) = source.resolve(
                selector,
                "contract call entrypoint metadata requires a self-describing contract artifact",
            )?;
            (
                Some(selector.to_owned()),
                Some(entrypoint_pc),
                entrypoint_authorization,
                argument_schema,
            )
        } else if source.is_self_describing() {
            return Err(ValidationFail::NotPermitted(
                "self-describing contract calls require explicit contract_entrypoint metadata"
                    .to_owned(),
            )
            .into());
        } else if metadata_payload.is_none() {
            return Ok(None);
        } else {
            (None, None, EntrypointAuthorizationV1::Anyone, None)
        };
    let payload = match argument_source {
        ContractArgumentSource::Metadata => metadata_payload,
        ContractArgumentSource::TriggerEvent(event_args) => {
            argument_schema.as_ref().map(|_| event_args.clone())
        }
        ContractArgumentSource::SchemaOnly => None,
    };
    let argument_record = if matches!(argument_source, ContractArgumentSource::SchemaOnly) {
        None
    } else {
        prepare_contract_argument_record_from_json(
            argument_schema.as_ref(),
            payload.as_ref(),
            gas_limit,
        )?
    };
    let args = match argument_source {
        ContractArgumentSource::TriggerEvent(event_args) => event_args.clone(),
        ContractArgumentSource::Metadata | ContractArgumentSource::SchemaOnly => {
            payload.unwrap_or_default()
        }
    };
    Ok(Some(ContractCallExecutionContext {
        contract_address,
        contract_subject: None,
        contract_alias,
        entrypoint,
        entrypoint_pc,
        entrypoint_authorization,
        args,
        argument_record,
    }))
}
#[cfg(test)]
pub(crate) fn parse_contract_invocation_execution_context(
    invocation: &ContractInvocation,
    bytecode: &[u8],
    contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    contract_subject: AccountId,
) -> Result<
    ContractCallExecutionContext,
    crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
> {
    let selector = invocation.entrypoint.trim();
    if selector.is_empty() {
        return Err(ValidationFail::NotPermitted(
            "contract entrypoint must not be empty".to_owned(),
        )
        .into());
    }
    let (entrypoint_pc, entrypoint_authorization, argument_schema) =
        resolve_callable_contract_entrypoint(
            bytecode,
            selector,
            "contract call requires a self-describing contract artifact",
        )?;
    let args = Json::default();
    let argument_record = prepare_validated_contract_argument_record(
        argument_schema.as_ref(),
        invocation.arguments.as_deref(),
        u64::MAX,
    )?;
    Ok(ContractCallExecutionContext {
        contract_address: Some(invocation.contract_address.clone()),
        contract_subject: Some(contract_subject),
        contract_alias,
        entrypoint: Some(selector.to_owned()),
        entrypoint_pc: Some(entrypoint_pc),
        entrypoint_authorization,
        args,
        argument_record,
    })
}
pub(crate) fn parse_prepared_contract_invocation_execution_context(
    invocation: &ContractInvocation,
    contract: &ivm::PreparedContract,
    contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    contract_subject: AccountId,
    gas_limit: u64,
) -> Result<
    ContractCallExecutionContext,
    crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
> {
    let selector = invocation.entrypoint.trim();
    if selector.is_empty() {
        return Err(ValidationFail::NotPermitted(
            "contract entrypoint must not be empty".to_owned(),
        )
        .into());
    }
    let (entrypoint_pc, entrypoint_authorization, argument_schema) =
        resolve_prepared_contract_entrypoint(contract, selector)?;
    let args = Json::default();
    let argument_record = prepare_validated_contract_argument_record(
        argument_schema.as_ref(),
        invocation.arguments.as_deref(),
        gas_limit,
    )?;
    Ok(ContractCallExecutionContext {
        contract_address: Some(invocation.contract_address.clone()),
        contract_subject: Some(contract_subject),
        contract_alias,
        entrypoint: Some(selector.to_owned()),
        entrypoint_pc: Some(entrypoint_pc),
        entrypoint_authorization,
        args,
        argument_record,
    })
}
/// Validate a top-level deployed entrypoint against the instance lifecycle state.
pub(crate) fn validate_prepared_contract_lifecycle_call(
    world: &impl WorldReadOnly,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    code_hash: iroha_crypto::Hash,
    contract: &ivm::PreparedContract,
    selector: &str,
) -> Result<Option<code::PendingContractLifecycle>, ValidationFail> {
    let descriptor = contract.entrypoint_descriptor(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
    })?;
    code::validate_contract_lifecycle_call(world, contract_address, code_hash, descriptor.kind)
}
pub(crate) fn compute_nexus_fee_amount(
    cfg: &iroha_data_model::block::consensus::RootFeePolicy,
    tx_bytes_len: usize,
    instruction_count: usize,
    gas_used: u64,
) -> Result<Quantity, ValidationFail> {
    let tx_bytes_u64 = u64::try_from(tx_bytes_len).map_err(|_| {
        ValidationFail::InternalError("transaction too large for fee accounting".to_owned())
    })?;
    let instr_u64 = u64::try_from(instruction_count).map_err(|_| {
        ValidationFail::InternalError("instruction count too large for fee accounting".to_owned())
    })?;
    let mut fee = cfg.base_fee.clone();
    for (unit, count) in [
        (&cfg.per_byte_fee, tx_bytes_u64),
        (&cfg.per_instruction_fee, instr_u64),
        (&cfg.per_gas_unit_fee, gas_used),
    ] {
        let delta = unit.try_mul_decimal(&Numeric::from(count)).map_err(|_| {
            ValidationFail::NotPermitted("fee amount exceeds supported numeric bounds".to_owned())
        })?;
        fee = fee.checked_add(&delta).map_err(|_| {
            ValidationFail::NotPermitted("fee amount exceeds supported numeric bounds".to_owned())
        })?;
    }
    Ok(fee)
}
fn fee_bound_for_admission_payload(
    payload: &TransactionPayload,
    zk_gas: &ConfidentialGas,
) -> Result<
    (usize, usize, u64),
    crate::execution_attempt::ExecutionAttemptError<NexusFeeAdmissionError>,
> {
    let tx_bytes_len = norito::canonical_frame_len(payload).map_err(|err| {
        NexusFeeAdmissionError::ConfigInvalid(format!(
            "failed to encode transaction for fee metering: {err}"
        ))
    })?;
    let (instruction_count, gas_used) = match &payload.instructions {
        Executable::Instructions(instructions) => (
            instructions.len(),
            isi_gas::meter_instructions(instructions.as_ref(), zk_gas).map_err(|original| {
                crate::execution_attempt::ExecutionAttemptError::Deferred(original.into())
            })?,
        ),
        Executable::ContractCall(_) | Executable::Ivm(_) => {
            let gas_limit = payload
                .fee_payment
                .gas_limit()
                .map(core::num::NonZeroU64::get)
                .ok_or_else(|| {
                    NexusFeeAdmissionError::rejected(
                        FeeRejectionCode::InvalidGasLimit,
                        "missing gas limit in fee payment intent",
                    )
                })?;
            (0, gas_limit)
        }
        Executable::IvmProved(proved) => {
            let gas_limit = payload
                .fee_payment
                .gas_limit()
                .map(core::num::NonZeroU64::get)
                .ok_or_else(|| {
                    NexusFeeAdmissionError::rejected(
                        FeeRejectionCode::InvalidGasLimit,
                        "missing gas limit in fee payment intent",
                    )
                })?;
            (proved.overlay.len(), gas_limit)
        }
        Executable::Batch(items) => {
            let instructions: Vec<_> = items
                .iter()
                .filter_map(|item| match item {
                    ExecutableBatchItem::Instruction(instruction) => Some(instruction.clone()),
                    ExecutableBatchItem::ContractCall(_) => None,
                })
                .collect();
            let contains_contract_call = items
                .iter()
                .any(|item| matches!(item, ExecutableBatchItem::ContractCall(_)));
            let gas_used = if contains_contract_call {
                payload
                    .fee_payment
                    .gas_limit()
                    .map(core::num::NonZeroU64::get)
                    .ok_or_else(|| {
                        NexusFeeAdmissionError::rejected(
                            FeeRejectionCode::InvalidGasLimit,
                            "missing gas limit in fee payment intent",
                        )
                    })?
            } else {
                isi_gas::meter_instructions(&instructions, zk_gas).map_err(|original| {
                    crate::execution_attempt::ExecutionAttemptError::Deferred(original.into())
                })?
            };
            (instructions.len(), gas_used)
        }
    };
    Ok((tx_bytes_len, instruction_count, gas_used))
}
fn fee_bound_for_admission(
    transaction: &SignedTransaction,
    zk_gas: &ConfidentialGas,
) -> Result<
    (usize, usize, u64),
    crate::execution_attempt::ExecutionAttemptError<NexusFeeAdmissionError>,
> {
    fee_bound_for_admission_payload(transaction.payload(), zk_gas)
}
fn pipeline_gas_component_enabled(fees: &RootFeePolicy, pipeline: &Pipeline) -> bool {
    !pipeline.gas.accepted_assets.is_empty() && fees.per_gas_unit_fee.is_zero()
}
fn resolve_pipeline_gas_quote_asset(
    world: &impl WorldReadOnly,
    pipeline: &Pipeline,
    payload: &TransactionPayload,
    validate_charge_limits: bool,
) -> Result<(AssetDefinitionId, AssetDefinition, u64), NexusFeeAdmissionError> {
    let requested = payload
        .fee_payment
        .charge_limits()
        .iter()
        .find(|limit| limit.kind == FeeChargeKind::PipelineGas)
        .map(|limit| limit.asset_definition_id.canonical_address());
    let requested_is_accepted = requested.as_ref().is_some_and(|requested| {
        pipeline
            .gas
            .accepted_assets
            .iter()
            .any(|accepted| accepted == requested)
    });
    let sponsor_revision =
        payload
            .fee_payment
            .sponsor_program()
            .and_then(|(program_id, revision)| {
                world
                    .fee_sponsor_program_revisions()
                    .get(&FeeSponsorProgramRevisionKey::new(
                        program_id.clone(),
                        revision,
                    ))
            });
    let sponsor_covers = |asset: &str| {
        sponsor_revision.is_none_or(|revision| {
            revision
                .asset_budgets
                .iter()
                .any(|budget| budget.asset_definition_id.canonical_address() == asset)
        })
    };
    let selected = if validate_charge_limits {
        let requested = requested.ok_or_else(|| {
            NexusFeeAdmissionError::rejected(
                FeeRejectionCode::InvalidFeeIntent,
                "signed fee intent is missing PipelineGas charge limit",
            )
        })?;
        if !requested_is_accepted {
            return Err(NexusFeeAdmissionError::rejected(
                FeeRejectionCode::FeeAssetNotCovered,
                format!("pipeline gas asset `{requested}` is not accepted by committed policy"),
            ));
        }
        requested
    } else if requested_is_accepted && requested.as_deref().is_some_and(&sponsor_covers) {
        requested.expect("accepted requested pipeline gas asset exists")
    } else {
        pipeline
            .gas
            .accepted_assets
            .iter()
            .find(|asset| sponsor_covers(asset))
            .cloned()
            .ok_or_else(|| {
                NexusFeeAdmissionError::ConfigInvalid(
                    "pipeline gas is enabled without an accepted asset".to_owned(),
                )
            })?
    };
    let rate = pipeline
        .gas
        .units_per_gas
        .iter()
        .find(|rate| rate.asset == selected)
        .ok_or_else(|| {
            NexusFeeAdmissionError::ConfigInvalid(format!(
                "missing pipeline gas units_per_gas mapping for `{selected}`"
            ))
        })?;
    if rate.units_per_gas == 0 {
        return Err(NexusFeeAdmissionError::ConfigInvalid(format!(
            "pipeline gas units_per_gas mapping for `{selected}` must be positive"
        )));
    }
    let parsed = AssetDefinitionId::parse_address_literal(&selected).map_err(|_| {
        NexusFeeAdmissionError::ConfigInvalid(format!(
            "invalid pipeline gas asset `{selected}`; expected a canonical asset definition address"
        ))
    })?;
    let (asset_definition_id, definition) = if let Ok(definition) = world.asset_definition(&parsed)
    {
        (definition.id().clone(), definition)
    } else {
        world
            .asset_definitions()
            .iter()
            .find(|(id, _)| id.canonical_address() == selected)
            .map(|(id, definition)| (id.clone(), definition.clone()))
            .ok_or_else(|| {
                NexusFeeAdmissionError::ConfigInvalid(format!(
                    "pipeline gas asset `{selected}` is not registered"
                ))
            })?
    };
    Ok((asset_definition_id, definition, rate.units_per_gas))
}
fn authority_fee_asset_id(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    route_dataspace_id: Option<DataSpaceId>,
    charge: &FeeChargeBound,
) -> Result<AssetId, crate::execution_attempt::ExecutionAttemptError<NexusFeeAdmissionError>> {
    if charge.kind == FeeChargeKind::Nexus {
        let scope = root_fees::private_scope(world, route_dataspace_id)?
            .map_or(AssetBalanceScope::Global, AssetBalanceScope::Dataspace);
        return Ok(AssetId::with_scope(
            charge.asset_definition_id.clone(),
            authority.clone(),
            scope,
        ));
    }
    let definition = world
        .asset_definition(&charge.asset_definition_id)
        .map_err(|_| {
            NexusFeeAdmissionError::ConfigInvalid(format!(
                "quoted fee asset `{}` disappeared before authority balance evaluation",
                charge.asset_definition_id
            ))
        })?;
    let scope = match definition.balance_scope_policy() {
        AssetBalancePolicy::Global => AssetBalanceScope::Global,
        AssetBalancePolicy::DataspaceRestricted => {
            AssetBalanceScope::Dataspace(route_dataspace_id.unwrap_or(DataSpaceId::UNIVERSAL))
        }
    };
    Ok(AssetId::with_scope(
        charge.asset_definition_id.clone(),
        authority.clone(),
        scope,
    ))
}
fn evaluate_nexus_fee_admission_payload(
    world: &impl WorldReadOnly,
    nexus: &iroha_config::parameters::actual::Nexus,
    captured_fees: &RootFeePolicy,
    pipeline: &Pipeline,
    zk_gas: &ConfidentialGas,
    payload: &TransactionPayload,
    observation_time_ms: u64,
    next_block_height: u64,
    route_dataspace_id: Option<DataSpaceId>,
    validate_charge_limits: bool,
) -> Result<
    FeeAdmissionQuote,
    crate::execution_attempt::ExecutionAttemptError<NexusFeeAdmissionError>,
> {
    let fees = root_fees::effective(world, captured_fees)?;
    // TODO: Private sponsorship needs vaults with exact dataspace custody. Public
    // sponsor-program identifiers alone cannot authorize a private monetary debit.
    if root_fees::private_scope(world, route_dataspace_id)?.is_some()
        && payload.fee_payment.sponsor_program().is_some()
    {
        return Err(NexusFeeAdmissionError::ConfigInvalid(
            "private-root fee sponsors require a scoped vault owner".into(),
        )
        .into());
    }
    let (tx_bytes_len, instruction_count, gas_used) =
        fee_bound_for_admission_payload(payload, zk_gas)?;
    let mut charges = Vec::with_capacity(2);
    let fee = compute_nexus_fee_amount(&fees, tx_bytes_len, instruction_count, gas_used)
        .map_err(validation_fail_to_nexus_fee_admission_error)?;
    let asset_definition_id = root_fees::currency(world, &fees, observation_time_ms)?;
    if !fee.is_zero() {
        charges.push(FeeChargeBound {
            kind: FeeChargeKind::Nexus,
            asset_definition_id,
            max_bound: fee,
        });
    }
    if !pipeline.gas.accepted_assets.is_empty() && fees.per_gas_unit_fee.is_zero() && gas_used > 0 {
        let (asset_definition_id, _definition, units_per_gas) =
            resolve_pipeline_gas_quote_asset(world, pipeline, payload, validate_charge_limits)?;
        let max_bound = Quantity::from(u128::from(gas_used) * u128::from(units_per_gas));
        if !max_bound.is_zero() {
            charges.push(FeeChargeBound {
                kind: FeeChargeKind::PipelineGas,
                asset_definition_id,
                max_bound,
            });
        }
    }
    charges.sort_by_key(|charge| charge.kind);
    let intent = &payload.fee_payment;
    if validate_charge_limits {
        validate_signed_charge_limits(intent, &charges)?;
    }
    match intent.sponsor_program() {
        None => {
            let mut required_by_asset = BTreeMap::<AssetId, Quantity>::new();
            let mut authority_charge_assets = BTreeMap::new();
            for charge in &charges {
                let payer_asset =
                    authority_fee_asset_id(world, &payload.authority, route_dataspace_id, charge)?;
                authority_charge_assets.insert(charge.kind, payer_asset.clone());
                let current = required_by_asset
                    .get(&payer_asset)
                    .cloned()
                    .unwrap_or_else(Quantity::zero);
                required_by_asset.insert(
                    payer_asset,
                    checked_quantity_add(&current, &charge.max_bound, "authority charge")?,
                );
            }
            let mut authority_balances = BTreeMap::new();
            for (payer_asset, required) in required_by_asset {
                let available = world
                    .assets()
                    .get(&payer_asset)
                    .map_or_else(Quantity::zero, |balance| balance.as_ref().clone());
                if available < required {
                    return Err(NexusFeeAdmissionError::rejected(
                        FeeRejectionCode::AuthorityPayerInsufficient,
                        format!(
                            "fee balance `{payer_asset}` for authority `{}` is insufficient: requires {required}, available {available}",
                            payload.authority
                        ),
                    ).into());
                }
                authority_balances.insert(payer_asset, available);
            }
            Ok(FeeAdmissionQuote {
                charges,
                debit_source: FeeDebitSource::Account(payload.authority.clone()),
                program_revision: None,
                relay_leases: BTreeMap::new(),
                capacities: BTreeMap::new(),
                authority_balances,
                authority_charge_assets,
            })
        }
        Some((program_id, program_revision)) => {
            let resolved = resolve_fee_sponsor_program(
                world,
                nexus,
                program_id,
                program_revision,
                &payload.authority,
                payload,
                route_dataspace_id,
                next_block_height,
                fee_sponsor_multisig::RuleContext::admission(observation_time_ms),
            )?;
            let capacities = evaluate_fee_sponsor_capacity(
                world,
                &resolved,
                &payload.authority,
                next_block_height,
                &charges,
            )?;
            Ok(FeeAdmissionQuote {
                charges,
                debit_source: FeeDebitSource::SponsorProgram(program_id.clone()),
                program_revision: Some(program_revision),
                relay_leases: BTreeMap::new(),
                capacities,
                authority_balances: BTreeMap::new(),
                authority_charge_assets: BTreeMap::new(),
            })
        }
    }
}
/// Quote and validate the exact fee funding source for a canonical unsigned draft.
///
/// The fee byte component is measured over the canonical [`TransactionPayload`] encoding, so this
/// result remains identical after the payload is signed. The supplied intent must already contain
/// adequate signature-bound charge limits. Protocol and successful-claim exemptions return an
/// accepted zero-component quote because execution skips the same exact signed payloads.
/// `sccp_committed_time_ms` must come from the same original committed source as `world`
/// ([`crate::smartcontracts::isi::sccp::hold::committed_time_ms`]); it is distinct from the
/// fee observation horizon. `None` represents an empty predecessor and cannot expire a pause.
#[allow(clippy::too_many_arguments)] // Separate fee horizon and original SCCP ledger time.
///
/// # Errors
/// Returns a rejection for unsupported settlement configuration, invalid gas
/// or signed charge limits, unresolved fee policy or assets, insufficient
/// funding, or invalid sponsor authorization/capacity. Original local policy
/// and exemption-read refusals remain `Deferred`.
pub fn quote_nexus_fee_admission_payload(
    world: &impl WorldReadOnly,
    nexus: &iroha_config::parameters::actual::Nexus,
    pipeline: &Pipeline,
    zk_gas: &ConfidentialGas,
    payload: &TransactionPayload,
    observation_time_ms: u64,
    sccp_committed_time_ms: Option<u64>,
    next_block_height: u64,
    route_dataspace_id: Option<DataSpaceId>,
) -> Result<
    FeeAdmissionQuote,
    crate::execution_attempt::ExecutionAttemptError<NexusFeeAdmissionError>,
> {
    let fees = root_fees::quote_policy(world)?;
    if fee_exempt_payload(
        world,
        fees,
        payload,
        observation_time_ms,
        sccp_committed_time_ms,
    )
    .map_err(crate::execution_attempt::ExecutionAttemptError::Deferred)?
    {
        return Ok(fee_exempt_admission_quote(payload));
    }
    evaluate_nexus_fee_admission_payload(
        world,
        nexus,
        fees,
        pipeline,
        zk_gas,
        payload,
        observation_time_ms,
        next_block_height,
        route_dataspace_id,
        true,
    )
}
fn fee_intent_with_exact_bounds(
    template: &FeePaymentIntent,
    charges: &[FeeChargeBound],
) -> FeePaymentIntent {
    let limits = charges
        .iter()
        .map(|charge| {
            FeeChargeLimit::new(
                charge.kind,
                charge.asset_definition_id.clone(),
                charge.max_bound.clone(),
            )
        })
        .collect();
    match template {
        FeePaymentIntent::Authority(payment) => {
            FeePaymentIntent::authority(limits, payment.gas_limit)
        }
        FeePaymentIntent::Sponsor(payment) => FeePaymentIntent::sponsor(
            payment.program_id.clone(),
            payment.program_revision,
            limits,
            payment.gas_limit,
        ),
    }
}
fn fee_exempt_admission_quote(payload: &TransactionPayload) -> FeeAdmissionQuote {
    let (debit_source, program_revision) = payload.fee_payment.sponsor_program().map_or_else(
        || (FeeDebitSource::Account(payload.authority.clone()), None),
        |(program_id, revision)| {
            (
                FeeDebitSource::SponsorProgram(program_id.clone()),
                Some(revision),
            )
        },
    );
    FeeAdmissionQuote {
        charges: Vec::new(),
        debit_source,
        program_revision,
        relay_leases: BTreeMap::new(),
        capacities: BTreeMap::new(),
        authority_balances: BTreeMap::new(),
        authority_charge_assets: BTreeMap::new(),
    }
}
/// Discover the exact charge limits for an unsigned transaction draft.
///
/// Callers may supply empty or stale limits. Core deterministically reaches a fixed point because
/// the limits themselves contribute to the canonical byte fee, then returns the exact
/// [`FeePaymentIntent`] to place in the payload before signing. Exempt payloads canonicalize
/// directly to empty charge limits. Queue admission intentionally uses the strict quote API.
/// Supply the original parent's SCCP time separately from the fee observation horizon, as
/// required by [`quote_nexus_fee_admission_payload`].
#[allow(clippy::too_many_arguments)] // Separate fee horizon and original SCCP ledger time.
///
/// # Errors
/// Returns the same policy, gas, asset and funding errors as strict fee quoting,
/// or a configuration error if canonical charge limits do not converge within
/// 32 iterations. Original local read refusals remain `Deferred`.
pub fn quote_nexus_fee_admission_draft(
    world: &impl WorldReadOnly,
    nexus: &iroha_config::parameters::actual::Nexus,
    pipeline: &Pipeline,
    zk_gas: &ConfidentialGas,
    payload: &TransactionPayload,
    observation_time_ms: u64,
    sccp_committed_time_ms: Option<u64>,
    next_block_height: u64,
    route_dataspace_id: Option<DataSpaceId>,
) -> Result<
    FeeAdmissionDraftQuote,
    crate::execution_attempt::ExecutionAttemptError<NexusFeeAdmissionError>,
> {
    let fees = root_fees::quote_policy(world)?;
    if fee_exempt_payload(
        world,
        fees,
        payload,
        observation_time_ms,
        sccp_committed_time_ms,
    )
    .map_err(crate::execution_attempt::ExecutionAttemptError::Deferred)?
    {
        // TODO(ws31): an SCCP payload is exempt on success only, and a failure is charged
        // only within its signed limits (`specs/sccp.md` §4.19). Recommend the ordinary
        // limits for such payloads (without requiring the payer to hold the fee asset now),
        // so a funded signer of an eligible SCCP transaction makes its failure chargeable.
        return Ok(FeeAdmissionDraftQuote {
            quote: fee_exempt_admission_quote(payload),
            recommended_intent: fee_intent_with_exact_bounds(&payload.fee_payment, &[]),
        });
    }
    let mut candidate = payload.clone();
    // Canonical numeric and sequence encodings settle after only a few boundary
    // crossings. Keep a hard deterministic guard against malformed schedules.
    for _ in 0..32 {
        let quote = evaluate_nexus_fee_admission_payload(
            world,
            nexus,
            fees,
            pipeline,
            zk_gas,
            &candidate,
            observation_time_ms,
            next_block_height,
            route_dataspace_id,
            false,
        )?;
        let recommended_intent =
            fee_intent_with_exact_bounds(&candidate.fee_payment, &quote.charges);
        if candidate.fee_payment == recommended_intent {
            let quote = evaluate_nexus_fee_admission_payload(
                world,
                nexus,
                fees,
                pipeline,
                zk_gas,
                &candidate,
                observation_time_ms,
                next_block_height,
                route_dataspace_id,
                true,
            )?;
            return Ok(FeeAdmissionDraftQuote {
                quote,
                recommended_intent,
            });
        }
        candidate.fee_payment = recommended_intent;
    }
    Err(NexusFeeAdmissionError::ConfigInvalid(
        "fee quote did not converge to a canonical charge-limit fixed point".to_owned(),
    )
    .into())
}
/// Quote and validate the fee funding source selected by a signed transaction.
///
/// This is the queue/admission wrapper around
/// [`quote_nexus_fee_admission_payload`]; signatures do not affect fee bytes.
#[allow(clippy::too_many_arguments)] // Separate fee horizon and original SCCP ledger time.
///
/// # Errors
/// Propagates the signed-limit, policy, funding and local-read errors from
/// [`quote_nexus_fee_admission_payload`] for the transaction payload.
pub fn quote_nexus_fee_admission(
    world: &impl WorldReadOnly,
    nexus: &iroha_config::parameters::actual::Nexus,
    pipeline: &Pipeline,
    zk_gas: &ConfidentialGas,
    transaction: &SignedTransaction,
    observation_time_ms: u64,
    sccp_committed_time_ms: Option<u64>,
    next_block_height: u64,
    route_dataspace_id: Option<DataSpaceId>,
) -> Result<
    FeeAdmissionQuote,
    crate::execution_attempt::ExecutionAttemptError<NexusFeeAdmissionError>,
> {
    quote_nexus_fee_admission_payload(
        world,
        nexus,
        pipeline,
        zk_gas,
        transaction.payload(),
        observation_time_ms,
        sccp_committed_time_ms,
        next_block_height,
        route_dataspace_id,
    )
}
#[cfg(test)]
pub(crate) mod sorafs_admission_hash_probe {
    //! Per-thread observation of the actual initializer helper's outer hash call.
    use std::cell::Cell;

    thread_local! {
        static HASHES: Cell<Option<usize>> = const { Cell::new(None) };
    }

    /// Observe the next original outer hash only while this thread has an active probe.
    pub(crate) fn note_outer_hash() {
        HASHES.with(|hashes| {
            if let Some(count) = hashes.get() {
                hashes.set(Some(count + 1));
            }
        });
    }

    /// Count actual helper hashes in one lexical call, restoring prior state even on unwind.
    pub(crate) fn count<R>(body: impl FnOnce() -> R) -> (R, usize) {
        struct Restore(Option<usize>);
        impl Drop for Restore {
            fn drop(&mut self) {
                HASHES.with(|hashes| hashes.set(self.0));
            }
        }
        let restore = Restore(HASHES.with(|hashes| hashes.replace(Some(0))));
        let result = body();
        let count = HASHES.with(|hashes| hashes.get().expect("active hash observation"));
        drop(restore);
        (result, count)
    }
}

/// Return whether execution is running inside the chain's initial genesis block.
///
/// The empty committed-block history keeps a genesis-shaped header replayed against live state
/// outside every bootstrap-only permission and fee exception.
pub(crate) fn is_initial_genesis_context(state_transaction: &StateTransaction<'_, '_>) -> bool {
    state_transaction._curr_block.is_genesis() && state_transaction.block_hashes.is_empty()
}
#[allow(clippy::too_many_arguments)] // Preserve the same explicit quote inputs.
pub(crate) fn quote_external_nexus_fee_admission(
    world: &impl WorldReadOnly,
    nexus: &iroha_config::parameters::actual::Nexus,
    pipeline: &Pipeline,
    zk_gas: &ConfidentialGas,
    transaction: &SignedTransaction,
    observation_time_ms: u64,
    sccp_committed_time_ms: Option<u64>,
    next_block_height: u64,
    route_dataspace_id: Option<DataSpaceId>,
) -> Result<
    Option<FeeAdmissionQuote>,
    crate::execution_attempt::ExecutionAttemptError<NexusFeeAdmissionError>,
> {
    let fees = root_fees::quote_policy(world)?;
    if fee_exempt_transaction(
        world,
        fees,
        transaction,
        observation_time_ms,
        sccp_committed_time_ms,
    )
    .map_err(crate::execution_attempt::ExecutionAttemptError::Deferred)?
    {
        return Ok(None);
    }
    quote_nexus_fee_admission(
        world,
        nexus,
        pipeline,
        zk_gas,
        transaction,
        observation_time_ms,
        sccp_committed_time_ms,
        next_block_height,
        route_dataspace_id,
    )
    .map(|quote| (!quote.charges.is_empty()).then_some(quote))
}
/// Revalidate the exact signed fee intent against the state used for block execution.
///
/// Queue reservations are an availability optimization, not consensus authority. Except for the
/// authentic initial genesis bootstrap, every execution path, including overlay application, must
/// run this check before applying business effects so a block producer cannot bypass signed maxima,
/// sponsor rules, budgets, or payer balance checks.
pub(crate) fn validate_transaction_fee_admission(
    state_transaction: &mut StateTransaction<'_, '_>,
    transaction: &SignedTransaction,
) -> Result<(), ValidationFail> {
    if is_initial_genesis_context(state_transaction) {
        return Ok(());
    }
    Executor::validate_root_fee_policy(state_transaction)?;
    if transaction_fee_exemption(state_transaction, transaction)
        .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?
        .0
        .skips_fee_on_success()
    {
        return Ok(());
    }
    evaluate_nexus_fee_admission_payload(
        &state_transaction.world,
        &state_transaction.nexus,
        root_fees::execution_policy(state_transaction)
            .map_err(|reason| ValidationFail::InternalError(reason.into()))?,
        &state_transaction.pipeline,
        &state_transaction.zk.gas,
        transaction.payload(),
        state_transaction.block_unix_timestamp_ms(),
        state_transaction.block_height(),
        state_transaction.current_dataspace_id,
        true,
    )
    .map_err(|error| {
        state_transaction.attempt_error_to_validation_fail(
            error.map_rejection(nexus_fee_admission_error_to_validation_fail),
        )
    })?;
    Ok(())
}
#[cfg(test)]
fn applied_overlay_base_gas(
    transaction: &SignedTransaction,
    overlay: &crate::pipeline::overlay::TxOverlay,
    zk_gas: &ConfidentialGas,
) -> Result<(u64, usize), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
    let gas_limit = transaction_gas_limit(transaction);
    let (gas_used, instruction_count, require_gas_limit) = match transaction.instructions() {
        Executable::ContractCall(_) | Executable::Ivm(_) => (
            overlay.ivm_gas_used().ok_or_else(|| {
                ValidationFail::InternalError(
                    "missing IVM gas usage metadata for overlay-applied transaction".to_owned(),
                )
            })?,
            0,
            true,
        ),
        Executable::Instructions(_) => (
            overlay
                .instructions()
                .try_fold(0_u64, |gas, instruction| {
                    isi_gas::meter_instruction(instruction, zk_gas)
                        .map(|cost| gas.saturating_add(cost))
                })
                .map_err(|original| {
                    crate::execution_attempt::ExecutionAttemptError::Deferred(original.into())
                })?,
            overlay.instruction_count(),
            false,
        ),
        Executable::IvmProved(_) => (
            overlay.ivm_gas_used().ok_or_else(|| {
                ValidationFail::InternalError(
                    "missing replayed IVM gas usage metadata for proved overlay transaction"
                        .to_owned(),
                )
            })?,
            overlay.instruction_count(),
            true,
        ),
        Executable::Batch(_) => {
            return Err(ValidationFail::InternalError(
                "mixed batch reached overlay fee settlement".to_owned(),
            )
            .into());
        }
    };
    if require_gas_limit && gas_limit.is_none() {
        return Err(ValidationFail::NotPermitted(
            "missing gas limit in fee payment intent".to_owned(),
        )
        .into());
    }
    if let Some(limit) = gas_limit
        && gas_used > limit
    {
        return Err(ValidationFail::NotPermitted(format!(
            "out of gas: used {gas_used} > limit {limit}"
        ))
        .into());
    }
    Ok((gas_used, instruction_count))
}

#[cfg(test)]
/// Reference gas and SORA Nexus fee settlement for overlay regression tests.
///
/// The caller snapshots metering before consuming the overlay, then settles against
/// the pre-effect fee policy after applying its owned effects.
fn charge_fees_for_applied_overlay(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    transaction: &SignedTransaction,
    gas_used: u64,
    instruction_count: usize,
) -> Result<(), ValidationFail> {
    // Genesis transactions are bootstrap operations and must remain fee-free.
    if is_initial_genesis_context(state_transaction) {
        return Ok(());
    }
    let tx_bytes_len = norito::canonical_frame_len(transaction.payload()).map_err(|err| {
        ValidationFail::InternalError(format!(
            "failed to encode transaction payload for fee metering: {err}"
        ))
    })?;
    let fee_sponsor = transaction
        .fee_payment_intent()
        .sponsor_program()
        .map(|(program_id, _)| program_id.clone());
    let skip_nexus_fee = transaction_fee_exemption(state_transaction, transaction)
        .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?
        .0
        .skips_fee_on_success();
    // Admission captured the governed gas policy before business effects were applied.
    // Keep that immutable snapshot for settlement so this transaction cannot alter its
    // own fee asset, rate, or destination account through the overlay.
    let gas_asset_opt = transaction
        .fee_payment_intent()
        .charge_limits()
        .iter()
        .find(|limit| limit.kind == FeeChargeKind::PipelineGas)
        .map(|limit| limit.asset_definition_id.canonical_address());
    let pipeline_gas_bound = fee_bound_for_admission(transaction, &state_transaction.zk.gas)
        .map_err(|attempt| {
            state_transaction.attempt_error_to_validation_fail(
                attempt.map_rejection(nexus_fee_admission_error_to_validation_fail),
            )
        })?
        .2;
    let pipeline_gas = &state_transaction.pipeline.gas;
    if !skip_nexus_fee
        && pipeline_gas_component_enabled(
            root_fees::execution_policy(state_transaction)
                .map_err(|reason| ValidationFail::InternalError(reason.into()))?,
            &state_transaction.pipeline,
        )
        && pipeline_gas_bound > 0
    {
        let Some(ref gas_asset_id_str) = gas_asset_opt else {
            return Err(ValidationFail::NotPermitted(
                "missing pipeline gas charge limit in fee payment intent".to_owned(),
            ));
        };
        if !pipeline_gas
            .accepted_assets
            .iter()
            .any(|a| a == gas_asset_id_str)
        {
            return Err(ValidationFail::NotPermitted(format!(
                "gas asset `{gas_asset_id_str}` is not accepted by committed policy"
            )));
        }
    }
    state_transaction.last_tx_gas_used =
        state_transaction.last_tx_gas_used.saturating_add(gas_used);
    Executor::enforce_transaction_gas_fits_block(
        state_transaction,
        state_transaction.last_tx_gas_used,
    )?;
    if !skip_nexus_fee
        && should_charge_pipeline_gas_asset(
            skip_nexus_fee,
            root_fees::execution_policy(state_transaction)
                .map_err(|reason| ValidationFail::InternalError(reason.into()))?,
            gas_asset_opt.as_deref(),
        )
        && let Some(gas_asset_id_str) = gas_asset_opt
    {
        Executor::charge_pipeline_gas_asset_fee(
            state_transaction,
            authority,
            transaction,
            &gas_asset_id_str,
            gas_used,
            fee_sponsor.as_ref(),
        )?;
    }
    if !skip_nexus_fee {
        Executor::charge_nexus_fees(
            state_transaction,
            authority,
            transaction,
            fee_sponsor,
            tx_bytes_len,
            instruction_count,
            gas_used,
        )?;
    }
    Ok(())
}
#[cfg(test)]
fn live_batch_overlay_byte_size(instructions: &[InstructionBox]) -> u64 {
    instructions.iter().fold(0_u64, |total, instruction| {
        total.saturating_add(u64::try_from(instruction.encode().len()).unwrap_or(u64::MAX))
    })
}
fn live_batch_contract_execution_limit(
    signed_gas_limit: Option<u64>,
    direct_gas_used: u64,
    block_remaining_at_start: u64,
    accountable_gas_used: u64,
) -> u64 {
    signed_gas_limit
        .unwrap_or(u64::MAX)
        .saturating_sub(direct_gas_used)
        .min(block_remaining_at_start.saturating_sub(accountable_gas_used))
}
fn is_reserved_multisig_role_id(role_id: &RoleId) -> bool {
    const MULTISIG_SIGNATORY_NAMESPACE: &str = "MULTISIG_SIGNATORY";
    let name = role_id.name().as_ref();
    name == MULTISIG_SIGNATORY_NAMESPACE
        || name
            .strip_prefix(MULTISIG_SIGNATORY_NAMESPACE)
            .is_some_and(|suffix| suffix.starts_with('/'))
}
impl Executor {
    fn resolve_pipeline_gas_asset_definition(
        state_transaction: &StateTransaction<'_, '_>,
        gas_asset_id_str: &str,
    ) -> Result<(AssetDefinitionId, AssetDefinition), ValidationFail> {
        let parsed = AssetDefinitionId::parse_address_literal(gas_asset_id_str).map_err(|_| {
            ValidationFail::NotPermitted(
                "invalid gas_asset_id; expected an unprefixed Base58 asset definition id"
                    .to_owned(),
            )
        })?;
        if let Ok(definition) = state_transaction.world.asset_definition(&parsed) {
            return Ok((definition.id().clone(), definition));
        }
        state_transaction
            .world
            .asset_definitions()
            .iter()
            .find(|(id, _)| id.canonical_address() == gas_asset_id_str)
            .map(|(id, definition)| (id.clone(), definition.clone()))
            .ok_or_else(|| {
                ValidationFail::NotPermitted(format!(
                    "gas asset `{gas_asset_id_str}` is not registered"
                ))
            })
    }
    fn enforce_transaction_gas_fits_block(
        state_transaction: &StateTransaction<'_, '_>,
        gas_used: u64,
    ) -> Result<(), ValidationFail> {
        let limit = state_transaction.gas_limit_per_block;
        let used_in_block = state_transaction.gas_used_in_block_so_far;
        if !crate::gas::gas_components_fit_block_limit(limit, [used_in_block, gas_used]) {
            let total = u128::from(used_in_block) + u128::from(gas_used);
            return Err(ValidationFail::NotPermitted(format!(
                "block gas limit exceeded: {total} > {limit}"
            )));
        }
        Ok(())
    }

    #[cfg(test)]
    fn consume_fee_sponsor_relay_lease(
        state_transaction: &mut StateTransaction<'_, '_>,
        program_id: &FeeSponsorProgramId,
        program_revision: u64,
        asset_definition_id: &AssetDefinitionId,
        amount: &Quantity,
        directly_settled: bool,
    ) -> Result<iroha_crypto::Hash, ValidationFail> {
        let (record, _) = select_fee_sponsor_relay_lease(
            &state_transaction.world,
            program_id,
            program_revision,
            asset_definition_id,
            state_transaction.current_dataspace_id,
            state_transaction.block_height(),
            amount,
        )
        .map_err(nexus_fee_admission_error_to_validation_fail)?;
        let executed_key = fee_sponsor_vault_allocation_usage_state_key(&record.lease_id)
            .map_err(nexus_fee_admission_error_to_validation_fail)?;
        let settled_key = fee_sponsor_vault_allocation_settled_usage_state_key(&record.lease_id)
            .map_err(nexus_fee_admission_error_to_validation_fail)?;
        let executed =
            fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &executed_key)
                .map_err(nexus_fee_admission_error_to_validation_fail)?;
        let settled =
            fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &settled_key)
                .map_err(nexus_fee_admission_error_to_validation_fail)?;
        let spent = core::cmp::max(executed, settled.clone());
        let updated_executed = spent.checked_add(amount).map_err(|_| {
            ValidationFail::InternalError(
                "verified fee sponsor vault allocation usage overflow".to_owned(),
            )
        })?;
        if updated_executed > record.verified_allocation {
            return Err(ValidationFail::NotPermitted(format!(
                "verified fee sponsor spend lease `{}` is insufficient",
                record.lease_id
            )));
        }
        let encoded = norito::to_bytes(&updated_executed).map_err(|err| {
            ValidationFail::InternalError(format!(
                "failed to encode verified fee sponsor vault allocation usage: {err}"
            ))
        })?;
        state_transaction
            .world
            .smart_contract_state
            .insert(executed_key, encoded);
        if directly_settled {
            let updated_settled = settled.checked_add(amount).map_err(|_| {
                ValidationFail::InternalError(
                    "settled verified fee sponsor vault allocation usage overflow".to_owned(),
                )
            })?;
            let settled_encoded = norito::to_bytes(&updated_settled).map_err(|err| {
                ValidationFail::InternalError(format!(
                    "failed to encode settled verified fee sponsor vault allocation usage: {err}"
                ))
            })?;
            state_transaction
                .world
                .smart_contract_state
                .insert(settled_key, settled_encoded);
        }
        Ok(record.lease_id)
    }
    fn increment_fee_sponsor_counter(
        state_transaction: &mut StateTransaction<'_, '_>,
        key: FeeSponsorBudgetCounterKey,
        amount: &Quantity,
    ) -> Result<(), ValidationFail> {
        let spent = state_transaction
            .world
            .fee_sponsor_budget_counters
            .get(&key)
            .map_or_else(Quantity::zero, |counter| counter.spent.clone())
            .checked_add(amount)
            .map_err(|_| {
                ValidationFail::InternalError(
                    "fee sponsor budget counter arithmetic overflow".to_owned(),
                )
            })?;
        state_transaction
            .world
            .fee_sponsor_budget_counters
            .insert(key.clone(), FeeSponsorBudgetCounter { key, spent });
        Ok(())
    }
    fn debit_fee_sponsor_program(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        transaction: &SignedTransaction,
        program_id: &FeeSponsorProgramId,
        kind: FeeChargeKind,
        asset_definition_id: &AssetDefinitionId,
        amount: &Quantity,
    ) -> Result<(), ValidationFail> {
        let Some((selected_program, program_revision)) =
            transaction.fee_payment_intent().sponsor_program()
        else {
            return Err(ValidationFail::InternalError(
                "sponsor-program debit requested for an authority-paid transaction".to_owned(),
            ));
        };
        if selected_program != program_id {
            return Err(ValidationFail::InternalError(
                "sponsor-program debit does not match the signed fee intent".to_owned(),
            ));
        }
        let entrypoint_hash = transaction.try_hash_as_entrypoint().map_err(|error| {
            state_transaction.attempt_error_to_validation_fail(
                crate::smartcontracts::isi::multisig::multisig_instruction_decode_attempt(
                    error,
                    |error| {
                        ValidationFail::InternalError(format!("fee settlement entrypoint: {error}"))
                    },
                ),
            )
        })?;
        let authenticated_entrypoint = (state_transaction.tx_call_hash
            == Some(Hash::from(entrypoint_hash)))
        .then_some(*entrypoint_hash.as_ref());
        let observation_time_ms = state_transaction
            ._curr_block
            .creation_time()
            .as_millis()
            .try_into()
            .map_err(|_| {
                ValidationFail::InternalError(
                    "fee settlement block timestamp exceeds u64".to_owned(),
                )
            })?;
        let resolved = resolve_fee_sponsor_program(
            &state_transaction.world,
            &state_transaction.nexus,
            program_id,
            program_revision,
            authority,
            transaction.payload(),
            state_transaction.current_dataspace_id,
            state_transaction.block_height(),
            fee_sponsor_multisig::RuleContext::settlement(
                observation_time_ms,
                authenticated_entrypoint,
                state_transaction.block_height(),
            ),
        )
        .map_err(|error| {
            state_transaction.attempt_error_to_validation_fail(
                error.map_rejection(nexus_fee_admission_error_to_validation_fail),
            )
        })?;
        let charge = FeeChargeBound {
            kind,
            asset_definition_id: asset_definition_id.clone(),
            max_bound: amount.clone(),
        };
        validate_signed_charge_limits(
            transaction.fee_payment_intent(),
            core::slice::from_ref(&charge),
        )
        .map_err(nexus_fee_admission_error_to_validation_fail)?;
        evaluate_fee_sponsor_capacity(
            &state_transaction.world,
            &resolved,
            authority,
            state_transaction.block_height(),
            core::slice::from_ref(&charge),
        )
        .map_err(nexus_fee_admission_error_to_validation_fail)?;
        let budget = resolved
            .revision
            .asset_budgets
            .iter()
            .find(|budget| &budget.asset_definition_id == asset_definition_id)
            .ok_or_else(|| {
                ValidationFail::InternalError(
                    "validated fee sponsor budget disappeared before debit".to_owned(),
                )
            })?;
        let vault_key = FeeSponsorVaultKey {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
        };
        let mut vault = state_transaction
            .world
            .fee_sponsor_vaults
            .get(&vault_key)
            .cloned()
            .ok_or_else(|| {
                ValidationFail::InternalError(
                    "validated fee sponsor vault disappeared before debit".to_owned(),
                )
            })?;
        vault.balance = vault.balance.checked_sub(amount).map_err(|_| {
            ValidationFail::InternalError(
                "validated fee sponsor vault became insufficient before debit".to_owned(),
            )
        })?;
        state_transaction
            .world
            .fee_sponsor_vaults
            .insert(vault_key, vault);
        let epoch =
            state_transaction.block_height().saturating_sub(1) / budget.epoch_length_blocks.get();
        for window in [
            FeeSponsorBudgetWindow::Block(FeeSponsorBlockBudgetWindow {
                height: state_transaction.block_height(),
            }),
            FeeSponsorBudgetWindow::ProgramEpoch(FeeSponsorProgramEpochBudgetWindow { epoch }),
            FeeSponsorBudgetWindow::BeneficiaryEpoch(FeeSponsorBeneficiaryEpochBudgetWindow {
                epoch,
                beneficiary: authority.clone(),
            }),
        ] {
            Self::increment_fee_sponsor_counter(
                state_transaction,
                FeeSponsorBudgetCounterKey {
                    program_id: program_id.clone(),
                    asset_definition_id: asset_definition_id.clone(),
                    window,
                },
                amount,
            )?;
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn charge_pipeline_gas_asset_fee(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        transaction: &SignedTransaction,
        gas_asset_id_str: &str,
        gas_used: u64,
        fee_sponsor: Option<&FeeSponsorProgramId>,
    ) -> Result<(), ValidationFail> {
        Self::validate_root_fee_policy(state_transaction)?;
        let gas_rate = state_transaction
            .pipeline
            .gas
            .units_per_gas
            .iter()
            .find(|rate| rate.asset == gas_asset_id_str)
            .ok_or_else(|| {
                ValidationFail::NotPermitted(format!(
                    "missing units_per_gas mapping for `{gas_asset_id_str}`"
                ))
            })?;
        let units_per_gas = gas_rate.units_per_gas;
        if gas_used == 0 || units_per_gas == 0 {
            return Ok(());
        }
        let tech_account = crate::pipeline_gas_policy::receiver_id(
            &state_transaction.pipeline.gas.tech_account_id,
        )
        .map_err(|error| {
            state_transaction.attempt_error_to_validation_fail(
                crate::execution_attempt::norito_decode_attempt_error(error, |error| {
                    ValidationFail::InternalError(format!(
                        "invalid committed pipeline gas receiver: {error}"
                    ))
                }),
            )
        })?;
        let (asset_definition_id, definition) =
            Self::resolve_pipeline_gas_asset_definition(state_transaction, gas_asset_id_str)?;
        // The product of two `u64` values is always exactly representable in
        // `u128`; keep fee consensus arithmetic exact instead of silently
        // selecting a saturation policy that can never be reached here.
        let fee_u128 = u128::from(gas_used) * u128::from(units_per_gas);
        if fee_u128 == 0 {
            return Ok(());
        }
        let qty = Quantity::from(fee_u128);
        let actual_charge = FeeChargeBound {
            kind: FeeChargeKind::PipelineGas,
            asset_definition_id: asset_definition_id.clone(),
            max_bound: qty.clone(),
        };
        validate_signed_charge_limits(
            transaction.fee_payment_intent(),
            core::slice::from_ref(&actual_charge),
        )
        .map_err(nexus_fee_admission_error_to_validation_fail)?;
        let payer = if let Some(program_id) = fee_sponsor {
            Self::debit_fee_sponsor_program(
                state_transaction,
                authority,
                transaction,
                program_id,
                FeeChargeKind::PipelineGas,
                &asset_definition_id,
                &qty,
            )?;
            root_fees::execution_policy(state_transaction)
                .map_err(|reason| ValidationFail::InternalError(reason.into()))?
                .sponsor_vault_custody_account_id
                .clone()
        } else {
            authority.clone()
        };
        let payer_scope = match definition.balance_scope_policy() {
            AssetBalancePolicy::Global => AssetBalanceScope::Global,
            AssetBalancePolicy::DataspaceRestricted => AssetBalanceScope::Dataspace(
                state_transaction
                    .current_dataspace_id
                    .unwrap_or(DataSpaceId::UNIVERSAL),
            ),
        };
        let payer_asset = AssetId::with_scope(asset_definition_id.clone(), payer, payer_scope);
        let transfer_result = if let Some(program_id) = fee_sponsor {
            let charge = VerifiedFeeSponsorCharge::transfer(
                authority.clone(),
                program_id.clone(),
                FeeChargeKind::PipelineGas,
                payer_asset,
                tech_account,
                qty.clone(),
                state_transaction.tx_call_hash.and_then(|entry| {
                    transaction
                        .fee_payment_intent()
                        .sponsor_program()
                        .map(|(_, revision)| (entry, Hash::from(transaction.hash()), revision))
                }),
            );
            crate::smartcontracts::isi::asset::isi::execute_verified_fee_sponsor_charge(
                state_transaction,
                charge,
            )
        } else {
            let transfer = iroha_data_model::isi::Transfer::<
                Asset,
                Quantity,
                iroha_data_model::account::Account,
            >::asset_quantity(payer_asset, qty.clone(), tech_account);
            let instr: DMInstructionBox = transfer.into();
            execute_gas_fee_transfer_instruction(&definition, instr, authority, state_transaction)
        };
        transfer_result.map_err(|err| {
            iroha_logger::debug!(
                ?err,
                authority = %authority,
                "gas fee transfer failed to apply"
            );
            ValidationFail::from(err)
        })?;
        #[cfg(feature = "telemetry")]
        {
            let delta = u64::try_from(fee_u128.min(u128::from(u64::MAX))).unwrap_or(u64::MAX);
            state_transaction.stage_block_fee_amount(Quantity::from(delta));
        }
        // This actual scoped transfer is the fee settlement. No synthetic XOR
        // conversion quote or lane-relay receipt can stand in for a balance effect.
        Ok(())
    }
    #[allow(clippy::too_many_lines)]
    fn charge_nexus_fees(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        transaction: &SignedTransaction,
        sponsor: Option<FeeSponsorProgramId>,
        tx_bytes_len: usize,
        instruction_count: usize,
        gas_used: u64,
    ) -> Result<(), ValidationFail> {
        Self::validate_root_fee_policy(state_transaction)?;
        let original_parameters = root_fees::execution_parameters(state_transaction).clone();
        let original_fees = original_parameters.root_fees().ok_or_else(|| {
            ValidationFail::InternalError("original committed root fee policy is absent".into())
        })?;
        let cfg =
            root_fees::effective(&state_transaction.world, original_fees).map_err(|error| {
                match error {
                    crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                        state_transaction.defer_execution(reason)
                    }
                    crate::execution_attempt::ExecutionAttemptError::Rejected(error) => {
                        nexus_fee_admission_error_to_validation_fail(error)
                    }
                }
            })?;
        let private_scope = root_fees::private_scope(
            &state_transaction.world,
            state_transaction.current_dataspace_id,
        )
        .map_err(|error| match error {
            crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                state_transaction.defer_execution(reason)
            }
            crate::execution_attempt::ExecutionAttemptError::Rejected(error) => {
                nexus_fee_admission_error_to_validation_fail(error)
            }
        })?;
        if private_scope.is_some() && sponsor.is_some() {
            return Err(ValidationFail::NotPermitted(
                "private-root fee sponsors require a scoped vault owner".into(),
            ));
        }
        let fee = compute_nexus_fee_amount(&cfg, tx_bytes_len, instruction_count, gas_used)?;
        if fee.is_zero() {
            return Ok(());
        }
        let payer_kind = if sponsor.is_some() {
            NexusFeePayer::Sponsor
        } else {
            NexusFeePayer::Payer
        };
        let asset_def = root_fees::currency(
            &state_transaction.world,
            &cfg,
            state_transaction.block_unix_timestamp_ms(),
        )
        .map_err(|error| match error {
            crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                state_transaction.defer_execution(reason)
            }
            crate::execution_attempt::ExecutionAttemptError::Rejected(error) => {
                nexus_fee_admission_error_to_validation_fail(error)
            }
        })?;
        let actual_charge = FeeChargeBound {
            kind: FeeChargeKind::Nexus,
            asset_definition_id: asset_def.clone(),
            max_bound: fee.clone(),
        };
        validate_signed_charge_limits(
            transaction.fee_payment_intent(),
            core::slice::from_ref(&actual_charge),
        )
        .map_err(nexus_fee_admission_error_to_validation_fail)?;
        let receipt = state_transaction.prepare_actual_nexus_fee_receipt(
            authority,
            transaction,
            sponsor.as_ref(),
            &asset_def,
            &fee,
            &cfg,
            tx_bytes_len,
            instruction_count,
            gas_used,
        )?;
        let (payer, payer_id, program_revision) = if let Some(program_id) = sponsor.as_ref() {
            let program_revision = transaction
                .fee_payment_intent()
                .sponsor_program()
                .map(|(_, revision)| revision)
                .ok_or_else(|| {
                    ValidationFail::InternalError(
                        "sponsored Nexus fee is missing its signed program revision".to_owned(),
                    )
                })?;
            Self::debit_fee_sponsor_program(
                state_transaction,
                authority,
                transaction,
                program_id,
                FeeChargeKind::Nexus,
                &asset_def,
                &fee,
            )?;
            (
                root_fees::execution_policy(state_transaction)
                    .map_err(|reason| ValidationFail::InternalError(reason.into()))?
                    .sponsor_vault_custody_account_id
                    .clone(),
                program_id.to_string(),
                Some(program_revision),
            )
        } else {
            (authority.clone(), authority.to_string(), None)
        };
        let payer_kind_label = match payer_kind {
            NexusFeePayer::Payer => "payer",
            NexusFeePayer::Sponsor => "sponsor",
        };
        let payer_asset = AssetId::with_scope(
            asset_def,
            payer.clone(),
            private_scope.map_or(AssetBalanceScope::Global, AssetBalanceScope::Dataspace),
        );
        let asset_label = payer_asset.definition().to_string();
        let previous_tx_dataspace_id = state_transaction.current_dataspace_id;
        let previous_world_dataspace_id = state_transaction.world.current_dataspace_id;
        state_transaction.current_dataspace_id =
            Some(private_scope.unwrap_or(DataSpaceId::UNIVERSAL));
        state_transaction.world.current_dataspace_id =
            Some(private_scope.unwrap_or(DataSpaceId::UNIVERSAL));
        let fee_burn_result = if matches!(payer_kind, NexusFeePayer::Sponsor) {
            let program_id = sponsor.clone().ok_or_else(|| {
                ValidationFail::InternalError(
                    "sponsored Nexus burn lost its verified program id".to_owned(),
                )
            })?;
            let charge = VerifiedFeeSponsorCharge::burn(
                authority.clone(),
                program_id,
                FeeChargeKind::Nexus,
                payer_asset,
                fee.clone(),
                state_transaction.tx_call_hash.and_then(|entry| {
                    program_revision
                        .map(|revision| (entry, Hash::from(transaction.hash()), revision))
                }),
            );
            crate::smartcontracts::isi::asset::isi::execute_verified_fee_sponsor_charge(
                state_transaction,
                charge,
            )
        } else {
            let burn = Burn::asset_quantity(fee.clone(), payer_asset);
            let instr: DMInstructionBox = burn.into();
            instr.execute(authority, state_transaction)
        };
        state_transaction.current_dataspace_id = previous_tx_dataspace_id;
        state_transaction.world.current_dataspace_id = previous_world_dataspace_id;
        fee_burn_result.map_err(|err| {
            let reason = format!("nexus fee burn failed to apply: {err}");
            status::record_nexus_fee_event(NexusFeeEvent::TransferFailed {
                payer_kind,
                payer_id: payer_id.clone(),
                amount: fee.clone(),
                asset_id: asset_label.clone(),
                reason: reason.clone(),
            });
            warn!(
                target: "economics",
                ?err,
                payer = %payer_id,
                payer_kind = payer_kind_label,
                fee_amount = %fee,
                asset = %asset_label,
                "nexus fee burn failed"
            );
            ValidationFail::from(err)
        })?;
        state_transaction.pending_nexus_fee_receipt = Some(receipt);
        // Stage the charged event so rejected transactions don't report successful debits.
        state_transaction.stage_nexus_fee_event(NexusFeeEvent::Charged {
            payer_kind,
            payer_id,
            amount: fee,
            asset_id: asset_label,
        });
        Ok(())
    }
    /// Validate the predecessor's complete monetary policy before any fee exemption.
    /// Neither SetParameter nor other transaction-local changes refresh this snapshot.
    fn validate_root_fee_policy(
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), ValidationFail> {
        if !is_initial_genesis_context(state_transaction) {
            let original_parameters = root_fees::execution_parameters(state_transaction).clone();
            let original_fees = original_parameters.root_fees().ok_or_else(|| {
                ValidationFail::InternalError("original committed root fee policy is absent".into())
            })?;
            root_fees::effective(&state_transaction.world, original_fees).map_err(|error| {
                match error {
                    crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                        state_transaction.defer_execution(reason)
                    }
                    crate::execution_attempt::ExecutionAttemptError::Rejected(error) => {
                        nexus_fee_admission_error_to_validation_fail(error)
                    }
                }
            })?;
        }
        Ok(())
    }
    /// Identify a role-11 instruction in the exact outer signed Network entry.
    ///
    /// Sealed reveals use an inner execution call hash, while their output proof names the
    /// distinct outer entry. Contract-emitted instructions have no direct signed ordinal.
    pub(crate) fn direct_stream_token_instruction_index(
        state_transaction: &StateTransaction<'_, '_>,
        transaction: &SignedTransaction,
        instruction: &InstructionBox,
        index: usize,
        direct_body: bool,
    ) -> Result<Option<u32>, ValidationFail> {
        use iroha_data_model::isi::sorafs::MutateSorafsStreamTokenAuthority;
        if !direct_body {
            return Ok(None);
        }
        let submitted = match transaction.instructions() {
            Executable::Instructions(instructions) => instructions.get(index),
            Executable::Batch(items) => items.get(index).and_then(|item| match item {
                ExecutableBatchItem::Instruction(instruction) => Some(instruction),
                ExecutableBatchItem::ContractCall(_) => None,
            }),
            Executable::ContractCall(_) | Executable::Ivm(_) | Executable::IvmProved(_) => None,
        };
        let expected = submitted.and_then(|item| {
            item.as_any()
                .downcast_ref::<MutateSorafsStreamTokenAuthority>()
        });
        let actual = instruction
            .as_any()
            .downcast_ref::<MutateSorafsStreamTokenAuthority>();
        if expected.is_none() || expected != actual {
            return Ok(None);
        }
        let signed_entry_hash = transaction.hash_as_entrypoint();
        if state_transaction.current_network_entrypoint_hash != Some(signed_entry_hash)
            || state_transaction.tx_call_hash != Some(iroha_crypto::Hash::from(signed_entry_hash))
            || state_transaction.current_tx_hash != Some(transaction.hash())
            || state_transaction
                .current_entrypoint_index
                .and_then(|value| u32::try_from(value).ok())
                .is_none()
        {
            return Ok(None);
        }
        u32::try_from(index).map(Some).map_err(|_| {
            ValidationFail::NotPermitted("signed stream-token instruction index exceeds u32".into())
        })
    }
    /// Identify a gateway instruction in the exact outer signed Network entry.
    ///
    /// Sealed reveals use an inner execution call hash, while their output proof names the
    /// distinct outer entry. Contract-emitted instructions have no direct signed ordinal.
    pub(crate) fn direct_stream_token_gateway_instruction_index(
        state_transaction: &StateTransaction<'_, '_>,
        transaction: &SignedTransaction,
        instruction: &InstructionBox,
        index: usize,
        direct_body: bool,
    ) -> Result<Option<u32>, ValidationFail> {
        use iroha_data_model::isi::sorafs::MutateSorafsStreamTokenGateway;
        if !direct_body || transaction.network_id() != Some(state_transaction.network_id()) {
            return Ok(None);
        }
        let submitted = match transaction.instructions() {
            Executable::Instructions(instructions) => instructions.get(index),
            Executable::Batch(items) => items.get(index).and_then(|item| match item {
                ExecutableBatchItem::Instruction(instruction) => Some(instruction),
                ExecutableBatchItem::ContractCall(_) => None,
            }),
            Executable::ContractCall(_) | Executable::Ivm(_) | Executable::IvmProved(_) => None,
        };
        let expected = submitted.and_then(|item| {
            item.as_any()
                .downcast_ref::<MutateSorafsStreamTokenGateway>()
        });
        let actual = instruction
            .as_any()
            .downcast_ref::<MutateSorafsStreamTokenGateway>();
        if expected.is_none() || expected != actual {
            return Ok(None);
        }
        // Governance cancellation is one complete signed operation; it cannot borrow a
        // manager's authority from an enclosing batch or accompanying instruction.
        if actual.is_some_and(|instruction| matches!(instruction.request.action,
            iroha_data_model::sorafs::stream_token_gateway::native::StreamTokenGatewayActionV1::CancelReputationDelivery { .. }))
            && !matches!(transaction.instructions(), Executable::Instructions(items)
                if items.len() == 1 && index == 0)
        {
            return Ok(None);
        }
        let signed_entry_hash = transaction.hash_as_entrypoint();
        if state_transaction.current_network_entrypoint_hash != Some(signed_entry_hash)
            || state_transaction.tx_call_hash != Some(iroha_crypto::Hash::from(signed_entry_hash))
            || state_transaction.current_tx_hash != Some(transaction.hash())
            || state_transaction
                .current_entrypoint_index
                .and_then(|value| u32::try_from(value).ok())
                .is_none()
        {
            return Ok(None);
        }
        u32::try_from(index).map(Some).map_err(|_| {
            ValidationFail::NotPermitted(
                "signed stream-token gateway instruction index exceeds u32".into(),
            )
        })
    }
    /// Capture the exact original signed SetPolicy; genesis requires its opaque source owner.
    pub(crate) fn direct_reputation_policy_origin(
        state: &StateTransaction<'_, '_>,
        transaction: &SignedTransaction,
        instruction: &InstructionBox,
        index: usize,
        direct_body: bool,
    ) -> Option<iroha_data_model::sorafs::reputation::ReputationJournalPolicyOriginV1> {
        use iroha_data_model::{
            isi::sorafs::SetSorafsReputationJournalAuthorityPolicy,
            sorafs::{
                reputation::ReputationJournalPolicyOriginV1 as Origin,
                stream_token_gateway::native::StreamTokenGatewayExecutionV1 as Execution,
            },
        };
        if !direct_body
            || !instruction
                .as_any()
                .is::<SetSorafsReputationJournalAuthorityPolicy>()
        {
            return None;
        }
        let Executable::Instructions(items) = transaction.instructions() else {
            return None;
        };
        if items.get(index) != Some(instruction)
            || state.current_network_entrypoint_hash != Some(transaction.hash_as_entrypoint())
            || state.tx_call_hash
                != Some(iroha_crypto::Hash::from(transaction.hash_as_entrypoint()))
            || state.current_tx_hash != Some(transaction.hash())
        {
            return None;
        }
        let execution = Execution {
            height: state._curr_block.height().get(),
            transaction_hash: *transaction.hash_as_entrypoint().as_ref(),
            entry_index: u32::try_from(state.current_entrypoint_index?).ok()?,
            instruction_index: u32::try_from(index).ok()?,
            recorded_at_unix_ms: state.block_unix_timestamp_ms(),
            authority: transaction.authority().clone(),
        };
        let origin = if is_initial_genesis_context(state)
            && transaction.network_id().is_none()
            && state
                .genesis_execution_scope
                .as_ref()
                .and_then(|scope| scope.for_transaction(state))
                .is_some()
        {
            Origin::Genesis(execution)
        } else if !is_initial_genesis_context(state)
            && transaction.network_id() == Some(state.network_id())
            && items.len() == 1
            && index == 0
        {
            Origin::Network(execution)
        } else {
            return None;
        };
        origin.validate().ok()?;
        Some(origin)
    }
    /// Bind only the sole exact external stream-token reputation append, including all signed fields.
    pub(crate) fn direct_stream_token_reputation_payload(
        state_transaction: &StateTransaction<'_, '_>,
        transaction: &SignedTransaction,
        instruction: &InstructionBox,
        direct_body: bool,
    ) -> Option<iroha_data_model::transaction::TransactionPayload> {
        use iroha_data_model::isi::sorafs::AppendSorafsStreamTokenReputationJournalEntry;
        let Executable::Instructions(items) = transaction.instructions() else {
            return None;
        };
        if !direct_body
            || items.len() != 1
            || items.first() != Some(instruction)
            || !instruction
                .as_any()
                .is::<AppendSorafsStreamTokenReputationJournalEntry>()
            || transaction.network_id() != Some(state_transaction.network_id())
            || transaction.multisig_signatures().is_some()
            || transaction
                .authority()
                .try_signatory()
                .is_none_or(|key| key.algorithm() != iroha_crypto::Algorithm::Ed25519)
            || state_transaction.current_network_entrypoint_hash
                != Some(transaction.hash_as_entrypoint())
            || state_transaction.tx_call_hash
                != Some(iroha_crypto::Hash::from(transaction.hash_as_entrypoint()))
            || state_transaction.current_tx_hash != Some(transaction.hash())
            || state_transaction.current_entrypoint_index.is_none()
        {
            return None;
        }
        Some(transaction.payload().clone())
    }
    /// Recognize only the exact direct instruction in the original signed genesis transaction.
    pub(crate) fn direct_sorafs_admission_initialization(
        state_transaction: &StateTransaction<'_, '_>,
        transaction: &SignedTransaction,
        instruction: &InstructionBox,
        index: usize,
        direct_body: bool,
    ) -> bool {
        use iroha_data_model::isi::sorafs::InitializeSorafsProviderAdmissionV1;
        if !direct_body
            || !is_initial_genesis_context(state_transaction)
            || !instruction
                .as_any()
                .is::<InitializeSorafsProviderAdmissionV1>()
        {
            return false;
        }
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return false;
        };
        if instructions.get(index) != Some(instruction) {
            return false;
        }
        // Authenticate the complete signed intent only for its exact initializer. This helper
        // is called for every genesis instruction, including unrelated instructions.
        #[cfg(test)]
        sorafs_admission_hash_probe::note_outer_hash();
        let outer = transaction.hash_as_entrypoint();
        state_transaction.current_network_entrypoint_hash == Some(outer)
            && state_transaction.tx_call_hash == Some(iroha_crypto::Hash::from(outer))
            && state_transaction.current_tx_hash == Some(transaction.hash())
            && state_transaction.current_entrypoint_index.is_some()
            && transaction.network_id().is_none()
    }
    /// Bind one role-15 operation to its sole direct signed External Network entry.
    ///
    /// Contract, IVM, sealed-reveal and mixed-batch effects cannot acquire this token.
    /// Local allocation refusals retain their first nonwire retry owner and cannot become a
    /// completed source rejection, fee, or output.
    pub(crate) fn direct_final_promotion_operation_origin(
        state_transaction: &mut StateTransaction<'_, '_>,
        transaction: &SignedTransaction,
        instruction: &InstructionBox,
        direct_body: bool,
    ) -> Result<
        Option<
            iroha_data_model::sorafs::final_promotion_authority::FinalPromotionOperationOriginV1,
        >,
        ValidationFail,
    > {
        use crate::execution_attempt::{ExecutionAttemptError, norito_decode_attempt_error};
        use iroha_data_model::{
            isi::sorafs::MutateSorafsFinalPromotionAuthority,
            sorafs::final_promotion_authority::{
                FinalPromotionAuthorityActionV1, FinalPromotionOperationOriginV1,
            },
        };
        state_transaction.current_direct_final_promotion_operation_origin = None;
        if !direct_body {
            return Ok(None);
        }
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Ok(None);
        };
        if instructions.len() != 1 || instructions.first() != Some(instruction) {
            return Ok(None);
        }
        let Some(native) = instruction
            .as_any()
            .downcast_ref::<MutateSorafsFinalPromotionAuthority>()
        else {
            return Ok(None);
        };
        if !matches!(
            &native.action,
            FinalPromotionAuthorityActionV1::Reserve(_)
                | FinalPromotionAuthorityActionV1::Complete(_)
        ) {
            return Ok(None);
        }
        let outer = transaction.try_hash_as_entrypoint().map_err(|error| {
            state_transaction.attempt_error_to_validation_fail(norito_decode_attempt_error(
                error,
                |_| {
                    ValidationFail::NotPermitted("invalid final-promotion original encoding".into())
                },
            ))
        })?;
        let signed_hash =
            iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::from(outer));
        if state_transaction.current_network_entrypoint_hash != Some(outer)
            || state_transaction.tx_call_hash != Some(iroha_crypto::Hash::from(outer))
            || state_transaction.current_tx_hash != Some(signed_hash)
            || transaction.network_id() != Some(&state_transaction.network_id)
        {
            return Ok(None);
        }
        match crate::query::signer_check::native_signed_transaction_frame_attempt_v1(transaction) {
            Ok(_) => {}
            Err(ExecutionAttemptError::Rejected(_)) => return Ok(None),
            Err(ExecutionAttemptError::Deferred(reason)) => {
                return Err(state_transaction.defer_execution(reason));
            }
        }
        let Some(entry_index) = state_transaction
            .current_entrypoint_index
            .and_then(|index| u32::try_from(index).ok())
        else {
            return Ok(None);
        };
        Ok(Some(FinalPromotionOperationOriginV1 {
            entry_hash: *outer.as_ref(),
            entry_index,
        }))
    }
    #[allow(clippy::too_many_lines)]
    fn execute_metered_instructions(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        transaction: &SignedTransaction,
        instructions: &[InstructionBox],
        ivm_proved_replay: Option<crate::pipeline::overlay::IvmProvedReplay>,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
        entrypoint_authorization: Option<&ContractEntrypointAuthorizationSnapshot>,
        tx_bytes_len: usize,
        gas_limit_md: Option<u64>,
        require_gas_limit: bool,
        gas_asset_opt: Option<String>,
        fee_sponsor: Option<FeeSponsorProgramId>,
        skip_nexus_fee: bool,
        mut attachment_custody: Option<&mut attachment_custody::AttachmentCustody<'_>>,
    ) -> Result<(), ValidationFail> {
        state_transaction.current_direct_privacy_instruction_index = None;
        state_transaction.clear_direct_parliament_candidate_testing();
        state_transaction.current_direct_sccp_forgery = false;
        state_transaction.clear_direct_sccp_pause();
        state_transaction.current_direct_kagemusha_load_instruction_index = None;
        state_transaction.current_direct_amx_instruction_index = None;
        if require_gas_limit && gas_limit_md.is_none() {
            return Err(ValidationFail::NotPermitted(
                "missing gas limit in fee payment intent".to_owned(),
            ));
        }
        if let Some(replay) = ivm_proved_replay.as_ref() {
            if state_transaction.last_tx_gas_used != replay.gas_used {
                return Err(ValidationFail::InternalError(
                    "proved replay lost its retained execution work".into(),
                ));
            }
            crate::validation_fee::enforce_ivm_proved_completed_axt_admission(
                replay.completed_axt.len(),
                state_transaction,
            )?;
        }
        // 1) Deterministically meter the instruction batch. Proved IVM transactions retain the
        // verified replay gas because the plain overlay does not account for VM execution cost.
        let used = match ivm_proved_replay.as_ref() {
            Some(replay) => replay.gas_used,
            None => isi_gas::meter_instructions(instructions, &state_transaction.zk.gas).map_err(
                |original| {
                    state_transaction.defer_execution(
                        crate::execution_attempt::ExecutionDeferred::from(original),
                    )
                },
            )?,
        };
        // Finish both fallible measurements before publishing an authored effect or fee basis.
        let confidential_delta =
            crate::gas::sum_confidential_gas_costs(instructions.iter(), &state_transaction.zk.gas)
                .map_err(|original| {
                    state_transaction.defer_execution(
                        crate::execution_attempt::ExecutionDeferred::from(original),
                    )
                })?;
        // 2) Enforce optional payer-provided gas limit (caps fee exposure).
        if let Some(limit) = gas_limit_md
            && used > limit
        {
            return Err(ValidationFail::NotPermitted(format!(
                "out of gas: used {used} > limit {limit}"
            )));
        }
        Self::enforce_transaction_gas_fits_block(state_transaction, used)?;
        // Preserve deterministic work accounting even if an instruction later
        // rejects. The block corridor decides whether the rejected live
        // transaction is fee-eligible and carries these counters forward.
        if ivm_proved_replay.is_some() {
            // The verifier's actual run already transferred its work, before
            // any AXT, gas or effect admission below could reject the replay.
            state_transaction.admit_replayed_execution_effects(instructions.iter())?;
        } else {
            // A refused authored set never starts, so it creates no work/fee basis.
            state_transaction.admit_authored_execution_effects(instructions.iter())?;
            state_transaction.last_tx_gas_used = used;
            state_transaction.record_execution_fee_instructions(instructions.len(), used)?;
        }
        if let Some(replay) = ivm_proved_replay.as_ref() {
            if entrypoint_authorization.is_none()
                && replay.queued.iter().any(|effect| {
                    effect.contract_runtime_context.is_some()
                        || effect.entrypoint_authorization.is_some()
                })
            {
                return Err(ValidationFail::NotPermitted(
                    "proved contract effects are missing their root authorization snapshot".into(),
                ));
            }
            crate::deferred_authority::reject_opaque_instruction_authority(
                replay
                    .queued
                    .iter()
                    .filter_map(|queued| queued.instruction()),
                state_transaction,
            )
            .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
        }
        if confidential_delta > 0 {
            state_transaction.record_confidential_gas_delta(confidential_delta);
        }
        match (contract_runtime_context, entrypoint_authorization) {
            (Some(context), Some(authorization)) => {
                if !authorization.is_root() {
                    return Err(ValidationFail::NotPermitted(
                        "proved overlay root authorization contains a parent invocation".to_owned(),
                    ));
                }
                let live_subject = code::fetch_bound_contract_subject(
                    state_transaction,
                    &context.contract_address,
                )
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract instance `{}` has no valid subject binding",
                        context.contract_address
                    ))
                })?;
                if context.contract_subject != live_subject
                    || context.contract_address != authorization.contract_address
                    || context.contract_alias != authorization.contract_alias
                    || context.entrypoint != authorization.entrypoint
                {
                    return Err(ValidationFail::NotPermitted(
                        "proved overlay runtime context does not match its immutable authorization snapshot"
                            .to_owned(),
                    ));
                }
                authorization
                    .validate_for_authority(&state_transaction.world, authority)
                    .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
            }
            (Some(_), None) => {
                return Err(ValidationFail::NotPermitted(
                    "proved contract overlay is missing its immutable authorization snapshot"
                        .to_owned(),
                ));
            }
            (None, Some(_)) => {
                return Err(ValidationFail::InternalError(
                    "proved entrypoint authorization has no contract runtime context".to_owned(),
                ));
            }
            (None, None) => {}
        }
        if let Some(replay) = ivm_proved_replay.as_ref()
            && !replay.durable_state_overlay.is_empty()
        {
            let root = entrypoint_authorization.ok_or_else(|| {
                ValidationFail::NotPermitted(
                    "proved durable-state replay is missing its root authorization snapshot"
                        .to_owned(),
                )
            })?;
            crate::pipeline::overlay::validate_ivm_proved_durable_authorizations(
                &state_transaction.world,
                &replay.durable_state_overlay,
                &replay.durable_state_authorizations,
                root,
            )
            .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
        }
        let instruction_count = instructions.len();
        // 3) Execute ISIs in order.
        let execution_result = (|| -> Result<(), ValidationFail> {
            if let Some(replay) = ivm_proved_replay {
                for queued in replay.queued {
                    match (
                        queued.contract_runtime_context.as_ref(),
                        queued.entrypoint_authorization.as_ref(),
                    ) {
                        (Some(context), Some(authorization)) => {
                            if let Some(root) = entrypoint_authorization
                                && !authorization.descends_from(root)
                            {
                                return Err(ValidationFail::NotPermitted(
                                    "proved overlay effect authorization does not descend from its root invocation"
                                        .to_owned(),
                                ));
                            }
                            let live_subject = code::fetch_bound_contract_subject(
                                state_transaction,
                                &context.contract_address,
                            )
                            .ok_or_else(|| {
                                ValidationFail::NotPermitted(format!(
                                    "contract instance `{}` has no valid subject binding",
                                    context.contract_address
                                ))
                            })?;
                            if context.contract_subject != live_subject
                                || context.contract_address != authorization.contract_address
                                || context.contract_alias != authorization.contract_alias
                                || context.entrypoint != authorization.entrypoint
                                || queued.authority != context.contract_subject
                            {
                                return Err(ValidationFail::NotPermitted(
                                    "proved overlay effect runtime context does not match its immutable authorization snapshot"
                                        .to_owned(),
                                ));
                            }
                            authorization
                                .validate(&state_transaction.world)
                                .map_err(|error| {
                                    state_transaction.attempt_error_to_validation_fail(error)
                                })?;
                        }
                        (Some(_), None) => {
                            return Err(ValidationFail::NotPermitted(
                                "proved contract effect is missing its immutable authorization snapshot"
                                    .to_owned(),
                            ));
                        }
                        (None, Some(_)) => {
                            return Err(ValidationFail::InternalError(
                                "proved effect authorization has no contract runtime context"
                                    .to_owned(),
                            ));
                        }
                        (None, None) => {}
                    }
                    match queued.payload {
                        crate::smartcontracts::ivm::host::QueuedEffectPayload::Instruction(
                            instruction,
                        ) => {
                            self.execute_instruction_with_contract_runtime_context(
                                state_transaction,
                                &queued.authority,
                                instruction,
                                queued.contract_runtime_context.as_ref(),
                            )?;
                        }
                        crate::smartcontracts::ivm::host::QueuedEffectPayload::Emission(
                            emission,
                        ) => {
                            crate::smartcontracts::ivm::host::HostExecutionArtifacts::validate_emission_provenance(
                                &state_transaction.world, &emission.value, queued.contract_runtime_context.as_ref(), queued.entrypoint_authorization.as_ref(),
                            )?;
                            state_transaction.record_contract_emission(emission)?;
                        }
                    }
                    if let Some(root) = entrypoint_authorization {
                        root.validate_for_authority(&state_transaction.world, authority)
                            .map_err(|error| {
                                state_transaction.attempt_error_to_validation_fail(error)
                            })?;
                    }
                    if let Some(authorization) = queued.entrypoint_authorization.as_ref() {
                        authorization
                            .validate(&state_transaction.world)
                            .map_err(|error| {
                                state_transaction.attempt_error_to_validation_fail(error)
                            })?;
                    }
                }
                if !replay.durable_state_overlay.is_empty() {
                    let root = entrypoint_authorization.ok_or_else(|| {
                        ValidationFail::NotPermitted(
                            "proved durable-state replay is missing its root authorization snapshot"
                                .to_owned(),
                        )
                    })?;
                    root.validate_for_authority(&state_transaction.world, authority)
                        .map_err(|error| {
                            state_transaction.attempt_error_to_validation_fail(error)
                        })?;
                    // A queued instruction can revoke the selected permission or replace a live
                    // contract binding. Validate the complete set before recording any replay
                    // artifact or writing the first durable key, so rejection remains atomic.
                    crate::pipeline::overlay::validate_ivm_proved_durable_authorizations(
                        &state_transaction.world,
                        &replay.durable_state_overlay,
                        &replay.durable_state_authorizations,
                        root,
                    )
                    .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
                }
                crate::smartcontracts::ivm::host::HostExecutionArtifacts::record_completed_axt_states(
                    state_transaction,
                    replay.completed_axt,
                )?;
                for (path, value) in replay.durable_state_overlay {
                    let authorization = replay
                        .durable_state_authorizations
                        .get(&path)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| {
                            ValidationFail::InternalError(format!(
                                "proved durable state path `{path}` lost its authorization snapshot before apply"
                            ))
                        })?;
                    authorization
                        .validate(&state_transaction.world)
                        .map_err(|error| {
                            state_transaction.attempt_error_to_validation_fail(error)
                        })?;
                    if !authorization.owns_durable_state_path(&path) {
                        return Err(ValidationFail::NotPermitted(format!(
                            "proved durable state path `{path}` does not belong to its contract authorization snapshot"
                        )));
                    }
                    if let Some(stored) = value {
                        state_transaction
                            .world
                            .smart_contract_state
                            .insert(path, stored);
                    } else {
                        state_transaction.world.smart_contract_state.remove(path);
                    }
                }
            } else {
                for (index, isi) in instructions.iter().enumerate() {
                    if let Some(authorization) = entrypoint_authorization {
                        authorization
                            .validate_for_authority(&state_transaction.world, authority)
                            .map_err(|error| {
                                state_transaction.attempt_error_to_validation_fail(error)
                            })?;
                    }
                    let direct_index = Self::direct_stream_token_instruction_index(
                        state_transaction,
                        transaction,
                        isi,
                        index,
                        contract_runtime_context.is_none() && entrypoint_authorization.is_none(),
                    )?;
                    state_transaction.current_direct_stream_token_instruction_index = direct_index;
                    state_transaction.current_direct_reputation_policy_origin =
                        Self::direct_reputation_policy_origin(
                            state_transaction,
                            transaction,
                            isi,
                            index,
                            contract_runtime_context.is_none()
                                && entrypoint_authorization.is_none(),
                        );
                    state_transaction.current_direct_stream_token_reputation_payload =
                        Self::direct_stream_token_reputation_payload(
                            state_transaction,
                            transaction,
                            isi,
                            contract_runtime_context.is_none()
                                && entrypoint_authorization.is_none(),
                        );
                    state_transaction.current_direct_stream_token_gateway_instruction_index =
                        Self::direct_stream_token_gateway_instruction_index(
                            state_transaction,
                            transaction,
                            isi,
                            index,
                            contract_runtime_context.is_none()
                                && entrypoint_authorization.is_none(),
                        )?;
                    state_transaction.current_direct_sorafs_admission_initialization =
                        Self::direct_sorafs_admission_initialization(
                            state_transaction,
                            transaction,
                            isi,
                            index,
                            contract_runtime_context.is_none()
                                && entrypoint_authorization.is_none(),
                        );
                    state_transaction.current_direct_final_promotion_operation_origin =
                        Self::direct_final_promotion_operation_origin(
                            state_transaction,
                            transaction,
                            isi,
                            contract_runtime_context.is_none()
                                && entrypoint_authorization.is_none(),
                        )?;
                    state_transaction.current_direct_musubi_pin_outbox_origin =
                        crate::smartcontracts::isi::musubi::capture_pin_outbox_operation_origin(
                            state_transaction,
                            transaction,
                            isi,
                            contract_runtime_context.is_none()
                                && entrypoint_authorization.is_none(),
                        )?;
                    state_transaction.arm_direct_sccp_forgery(
                        transaction,
                        authority,
                        isi,
                        index,
                        contract_runtime_context.is_none() && entrypoint_authorization.is_none(),
                    );
                    state_transaction.arm_direct_parliament_candidate_testing(
                        transaction,
                        authority,
                        &isi,
                        index,
                        contract_runtime_context.is_none() && entrypoint_authorization.is_none(),
                    );
                    state_transaction.current_direct_kagemusha_load_instruction_index =
                        (contract_runtime_context.is_none() && entrypoint_authorization.is_none())
                            .then_some(index);
                    state_transaction.current_direct_amx_instruction_index =
                        (contract_runtime_context.is_none()
                            && entrypoint_authorization.is_none()
                            && isi
                                .as_any()
                                .downcast_ref::<iroha_data_model::isi::sumeragi_amx::PrepareAmxV1>()
                                .is_some())
                        .then_some(index);
                    state_transaction.arm_direct_privacy_instruction_v1(
                        transaction,
                        authority,
                        isi,
                        index,
                        contract_runtime_context.is_none() && entrypoint_authorization.is_none(),
                    );
                    let result = if contract_runtime_context.is_none()
                        && entrypoint_authorization.is_none()
                        && attachment_custody.is_some()
                    {
                        self.execute_original_direct_instruction(
                            state_transaction,
                            authority,
                            isi,
                            index,
                            attachment_custody.as_deref_mut(),
                        )
                    } else {
                        self.execute_instruction_with_profile_and_contract_runtime_context(
                            state_transaction,
                            authority,
                            isi,
                            InstructionExecutionProfile::Runtime,
                            contract_runtime_context,
                        )
                    };
                    state_transaction.current_direct_privacy_instruction_index = None;
                    state_transaction.clear_direct_parliament_candidate_testing();
                    state_transaction.current_direct_sccp_forgery = false;
                    state_transaction.clear_direct_sccp_pause();
                    state_transaction.current_direct_kagemusha_load_instruction_index = None;
                    state_transaction.current_direct_amx_instruction_index = None;
                    state_transaction.current_direct_stream_token_instruction_index = None;
                    state_transaction.current_direct_stream_token_gateway_instruction_index = None;
                    state_transaction.current_direct_stream_token_reputation_payload = None;
                    state_transaction.current_direct_reputation_policy_origin = None;
                    state_transaction.current_direct_sorafs_admission_initialization = false;
                    state_transaction.current_direct_final_promotion_operation_origin = None;
                    state_transaction.current_direct_musubi_pin_outbox_origin = None;
                    result?;
                    if let Some(authorization) = entrypoint_authorization {
                        authorization
                            .validate_for_authority(&state_transaction.world, authority)
                            .map_err(|error| {
                                state_transaction.attempt_error_to_validation_fail(error)
                            })?;
                    }
                }
            }
            if let Some(authorization) = entrypoint_authorization {
                authorization
                    .validate_for_authority(&state_transaction.world, authority)
                    .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
            }
            Ok(())
        })();
        execution_result?;
        // 4) Charge gas fees when configured and the transaction specified a gas asset.
        if !skip_nexus_fee
            && should_charge_pipeline_gas_asset(
                skip_nexus_fee,
                root_fees::execution_policy(state_transaction)
                    .map_err(|reason| ValidationFail::InternalError(reason.into()))?,
                gas_asset_opt.as_deref(),
            )
            && let Some(gas_asset_id_str) = gas_asset_opt
        {
            Self::charge_pipeline_gas_asset_fee(
                state_transaction,
                authority,
                transaction,
                &gas_asset_id_str,
                used,
                fee_sponsor.as_ref(),
            )?;
        }
        if !skip_nexus_fee {
            Self::charge_nexus_fees(
                state_transaction,
                authority,
                transaction,
                fee_sponsor,
                tx_bytes_len,
                instruction_count,
                used,
            )?;
        }
        Ok(())
    }
    /// Resolve the cache-bound inputs for one deployed-contract invocation.
    ///
    /// The returned value owns its prepared-program handle and can therefore be executed after
    /// the caller releases any mutex guard used to access `ivm_cache`.
    pub(crate) fn resolve_contract_invocation(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        call: &ContractInvocation,
        ivm_cache: &mut IvmCache,
    ) -> Result<ResolvedContractInvocation, ValidationFail> {
        root_scope::ensure_contract_scope(state_transaction, &call.contract_address)?;
        code::ensure_contract_execution_allowed(
            &state_transaction.world,
            &call.contract_address,
            state_transaction.block_height(),
        )
        .map_err(ValidationFail::NotPermitted)?;
        let identity =
            code::fetch_bound_contract_identity(state_transaction, &call.contract_address)
                .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract instance `{}` not found in WSV",
                        call.contract_address
                    ))
                })?;
        ensure_contract_invocation_code_hash(call, identity.code_hash)?;
        let contract_subject =
            code::fetch_bound_contract_subject(state_transaction, &identity.contract_address)
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract instance `{}` has no valid subject binding",
                        identity.contract_address
                    ))
                })?;
        let code_bytes = state_transaction
            .world
            .contract_code()
            .get(&iroha_data_model::smart_contract::ContractArtifactId::new(
                identity.contract_address.dataspace_id().map_err(|_| {
                    ValidationFail::NotPermitted("invalid contract dataspace".into())
                })?,
                identity.code_hash,
            ))
            .ok_or_else(|| {
                ValidationFail::NotPermitted(format!(
                    "contract bytecode `{}` not found in WSV",
                    identity.code_hash
                ))
            })?;
        let summary = if let Some(summary) = ivm_cache
            .cached_program_summary(identity.code_hash)
            .map_err(|error| ValidationFail::InternalError(error.to_string()))?
        {
            summary
        } else {
            ivm_cache
                .summarize_program_with_hash(identity.code_hash, code_bytes.as_ref())
                .map_err(|error| ValidationFail::InternalError(error.to_string()))?
        };
        if summary.prepared_contract().artifact() != code_bytes.as_slice() {
            return Err(ValidationFail::NotPermitted(format!(
                "cached contract bytecode `{}` does not match live WSV",
                identity.code_hash
            )));
        }
        Ok(ResolvedContractInvocation {
            identity,
            contract_subject,
            summary,
        })
    }
    /// Execute one deployed-contract invocation against the current transaction view.
    ///
    /// Fee settlement and block-gas accounting deliberately remain with the enclosing
    /// executable. This lets a mixed batch invoke this helper multiple times while sharing one
    /// signed gas limit and settling exactly once.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn execute_contract_invocation(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        call: &ContractInvocation,
        ivm_cache: &mut IvmCache,
        effective_limit: u64,
        logical_time_ms: u64,
        trigger_context: Option<(&TriggerId, u64)>,
    ) -> Result<ContractInvocationOutcome, ValidationFail> {
        // A contract frame cannot inherit a native instruction's signed ordinal.
        state_transaction.current_direct_privacy_instruction_index = None;
        state_transaction.clear_direct_parliament_candidate_testing();
        state_transaction.current_direct_sccp_forgery = false;
        state_transaction.clear_direct_sccp_pause();
        state_transaction.current_direct_kagemusha_load_instruction_index = None;
        state_transaction.current_direct_amx_instruction_index = None;
        state_transaction.current_direct_stream_token_gateway_instruction_index = None;
        state_transaction.current_direct_stream_token_reputation_payload = None;
        state_transaction.current_direct_reputation_policy_origin = None;
        state_transaction.current_direct_musubi_pin_outbox_origin = None;
        let resolved = self.resolve_contract_invocation(state_transaction, call, ivm_cache)?;
        self.execute_resolved_contract_invocation(
            state_transaction,
            authority,
            call,
            resolved,
            effective_limit,
            logical_time_ms,
            trigger_context,
        )
    }
    /// Execute a previously resolved deployed-contract invocation without accessing `IvmCache`.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn execute_resolved_contract_invocation(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        call: &ContractInvocation,
        resolved: ResolvedContractInvocation,
        effective_limit: u64,
        logical_time_ms: u64,
        trigger_context: Option<(&TriggerId, u64)>,
    ) -> Result<ContractInvocationOutcome, ValidationFail> {
        // A contract frame cannot inherit a native instruction's signed ordinal.
        use crate::smartcontracts::ivm::host::CoreHostImpl as CoreCoreHost;
        state_transaction.current_direct_privacy_instruction_index = None;
        state_transaction.clear_direct_parliament_candidate_testing();
        state_transaction.current_direct_sccp_forgery = false;
        state_transaction.clear_direct_sccp_pause();
        state_transaction.current_direct_kagemusha_load_instruction_index = None;
        state_transaction.current_direct_amx_instruction_index = None;
        state_transaction.current_direct_stream_token_gateway_instruction_index = None;
        state_transaction.current_direct_stream_token_reputation_payload = None;
        state_transaction.current_direct_reputation_policy_origin = None;
        state_transaction.current_direct_musubi_pin_outbox_origin = None;
        root_scope::ensure_contract_scope(state_transaction, &call.contract_address)?;
        let ResolvedContractInvocation {
            identity,
            contract_subject,
            summary,
        } = resolved;
        let effective_cycles =
            validate_prepared_ivm_execution_policy(state_transaction, &summary.metadata)?;
        let effective_limit = effective_limit.min(
            crate::smartcontracts::ivm::gas_limit_for_cycles(effective_cycles),
        );
        let manifest = state_transaction
            .world
            .contract_manifests()
            .get(&iroha_data_model::smart_contract::ContractArtifactId::new(
                identity.contract_address.dataspace_id().map_err(|_| {
                    ValidationFail::NotPermitted("invalid contract dataspace".into())
                })?,
                identity.code_hash,
            ))
            .ok_or_else(|| {
                ValidationFail::NotPermitted(format!(
                    "contract instance `{}` has no manifest",
                    identity.contract_address
                ))
            })?;
        crate::smartcontracts::ivm::validate_manifest_hashes(
            manifest,
            summary.code_hash,
            summary.abi_hash,
        )
        .map_err(ValidationFail::IvmAdmission)?;
        let lifecycle_transition = validate_prepared_contract_lifecycle_call(
            &state_transaction.world,
            &call.contract_address,
            identity.code_hash,
            summary.prepared_contract(),
            &call.entrypoint,
        )?;
        let entrypoint_authorization = authorize_prepared_contract_selector(
            &state_transaction.world,
            authority,
            summary.prepared_contract(),
            &call.entrypoint,
            &identity,
        )
        .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
        let contract_call_context = parse_prepared_contract_invocation_execution_context(
            call,
            summary.prepared_contract(),
            identity.contract_alias.clone(),
            contract_subject,
            effective_limit,
        )
        .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
        let heap_limit = state_transaction
            .world
            .parameters
            .get()
            .smart_contract()
            .memory()
            .get();
        let mut runtime = summary
            .checkout_runtime(effective_limit, heap_limit)
            .map_err(|error| {
                state_transaction.vm_error_to_validation_fail(error, |error| {
                    ValidationFail::InternalError(error.to_string())
                })
            })?;
        runtime.set_max_cycles(effective_cycles.get());
        runtime.set_gas_limit(effective_limit);
        if let Some(entrypoint_pc) = contract_call_context.entrypoint_pc {
            let code_len = runtime.memory.code_len();
            runtime.set_register(1, code_len);
            runtime.set_program_counter(entrypoint_pc).map_err(|err| {
                let selector = contract_call_context
                    .entrypoint
                    .as_deref()
                    .unwrap_or("main");
                ValidationFail::NotPermitted(format!(
                    "contract entrypoint `{selector}` resolved to invalid pc: {err}"
                ))
            })?;
        }
        let precharge_result = contract_call_context
            .argument_record
            .as_ref()
            .map_or(Ok(()), |record| record.precharge_vm(&mut runtime));
        if let Err(error) = &precharge_result {
            runtime.record_boundary_fault(
                error,
                iroha_data_model::executor::fault::IvmFaultPositionV1::Initialization,
            );
        }
        let contract_runtime_context = contract_call_context.runtime_context();
        let accounts = state_transaction.accounts_snapshot();
        let mut host = CoreCoreHost::with_accounts_and_argument_record(
            authority.clone(),
            Arc::clone(&accounts),
            contract_call_context.argument_record,
        );
        host.set_output_limits_from_parameters(
            state_transaction.world.parameters.get().smart_contract(),
        );
        host.set_prepared_contract_cache(summary.prepared_contract_cache());
        host.hydrate_axt_state(state_transaction).map_err(|error| {
            ValidationFail::InternalError(format!("invalid AXT policy snapshot: {error}"))
        })?;
        // User contract calls execute before the enclosing block has a finalized creation
        // timestamp, so their caller supplies the deterministic logical time. Trigger callers
        // additionally bind the trigger id and deterministic NFT sequence base.
        host.set_block_time_ms(logical_time_ms);
        if let Some((trigger_id, nft_seq_base)) = trigger_context {
            host.set_trigger_id(trigger_id.clone());
            host.set_nft_seq_base(nft_seq_base);
        }
        host.set_crypto_config(Arc::clone(&state_transaction.crypto));
        host.set_zk_config(&state_transaction.zk);
        host.set_public_inputs_from_parameters(state_transaction.world.parameters.get());
        let vrf_epoch_length =
            crate::smartcontracts::ivm::host::vrf_epoch_length_from_state(state_transaction)
                .map_err(|error| {
                    state_transaction.attempt_error_to_validation_fail(
                        error.map_rejection(ValidationFail::InternalError),
                    )
                })?;
        state_transaction.hydrate_zk_snapshots_for_execution(&mut host)?;
        host.set_query_state_with_vrf_epoch_seeds(state_transaction, vrf_epoch_length);
        host.set_contract_runtime_context(contract_runtime_context.clone());
        host.set_contract_entrypoint_authorization(Some(entrypoint_authorization));
        if let Some(pending) = lifecycle_transition {
            host.set_contract_lifecycle_transition(&call.contract_address, pending);
        }
        host.set_chain_id(&state_transaction.chain_id);
        #[cfg(feature = "telemetry")]
        host.set_telemetry(state_transaction.telemetry.clone());

        let cycle_budget = state_transaction.execution_cycle_budget()?;
        let run_result = precharge_result
            .and_then(|()| match cycle_budget {
                Some(budget) => runtime.run_with_host_and_cycle_budget(&mut host, budget),
                None => runtime.run_with_host(&mut host),
            })
            .and_then(|()| host.finish_contract_result(&runtime));
        let gas_used = effective_limit.saturating_sub(runtime.remaining_gas());
        if let Err(err) = run_result {
            let attempt =
                crate::smartcontracts::ivm::map_vm_error_with_context_to_validation(&runtime, err);
            drop(host);
            let error = match attempt {
                crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                    return Err(state_transaction.defer_execution(reason));
                }
                crate::execution_attempt::ExecutionAttemptError::Rejected(error) => error,
            };
            // Retain attempted VM work even when the guest traps. Live-batch rejection discards
            // business state but uses this counter for block budgeting and rejected fees.
            state_transaction.last_tx_gas_used =
                state_transaction.last_tx_gas_used.saturating_add(gas_used);
            if trigger_context.is_none() {
                state_transaction.record_execution_fee_vm_work(gas_used)?;
            }
            return Err(error);
        }
        let returned_error = matches!(run_result, Ok(true));
        let lifecycle_transition = lifecycle_transition.filter(|_| !returned_error);
        let next_nft_sequence = trigger_context.map(|(_, initial_sequence)| {
            if returned_error {
                initial_sequence
            } else {
                host.next_nft_sequence()
            }
        });
        let runtime_origin = contract_runtime_context.as_ref().map(|context| {
            crate::validation_fee::OpaqueDeferredRuntimeOrigin::new(
                context,
                summary.prepared_contract().artifact(),
            )
        });
        let artifacts = host.into_execution_artifacts(contract_runtime_context.clone());
        let artifacts = match artifacts {
            Err(crate::execution_attempt::ExecutionAttemptError::Deferred(original)) => {
                return Err(state_transaction.defer_execution(original));
            }
            completed => completed,
        };
        // Retain completed VM work even when artifact validation or application later fails.
        state_transaction.last_tx_gas_used =
            state_transaction.last_tx_gas_used.saturating_add(gas_used);
        if trigger_context.is_none() {
            state_transaction.record_execution_fee_vm_work(gas_used)?;
        }
        let artifacts = artifacts
            .map_err(|attempt| state_transaction.attempt_error_to_validation_fail(attempt))?;
        let validation_outcome = crate::validation_fee::enforce_opaque_deferred_instruction_groups(
            &artifacts.queued_instructions_by_authority(),
            &artifacts.queued_instructions_with_authority(),
            state_transaction,
            runtime_origin,
        )
        .map_err(|rejection| match rejection {
            crate::tx::TransactionRejectionReason::Validation(fail) => fail,
            other => ValidationFail::NotPermitted(format!(
                "validation-fee policy resolution failed during deployed contract execution: {other:?}"
            )),
        })?;
        if validation_outcome == crate::validation_fee::OpaqueDeferredValidationOutcome::NoOp {
            return Ok(ContractInvocationOutcome {
                gas_used,
                executed_instructions: Vec::new(),
                next_nft_sequence,
            });
        }
        if let Some(pending) = lifecycle_transition {
            code::validate_contract_lifecycle_completion(
                &state_transaction.world,
                &call.contract_address,
                pending,
            )?;
        }
        let executed = artifacts.apply_to_transaction_with_lifecycle(
            state_transaction,
            authority,
            lifecycle_transition.map(|pending| (&call.contract_address, pending)),
        )?;
        Ok(ContractInvocationOutcome {
            gas_used,
            executed_instructions: executed,
            next_nft_sequence,
        })
    }
    #[allow(clippy::too_many_arguments)]
    fn settle_live_transaction_fees(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        transaction: &SignedTransaction,
        direct_gas_used: u64,
        instruction_count: usize,
        tx_bytes_len: usize,
        gas_asset_opt: Option<String>,
        fee_sponsor: Option<FeeSponsorProgramId>,
        skip_nexus_fee: bool,
    ) -> Result<(), ValidationFail> {
        if state_transaction.last_tx_gas_used < direct_gas_used {
            return Err(ValidationFail::InternalError(
                "live fee settlement lost staged direct gas".to_owned(),
            ));
        }
        let accountable_gas_used = state_transaction.last_tx_gas_used;
        Self::enforce_transaction_gas_fits_block(state_transaction, accountable_gas_used)?;
        if !skip_nexus_fee
            && should_charge_pipeline_gas_asset(
                skip_nexus_fee,
                root_fees::execution_policy(state_transaction)
                    .map_err(|reason| ValidationFail::InternalError(reason.into()))?,
                gas_asset_opt.as_deref(),
            )
            && let Some(gas_asset_id_str) = gas_asset_opt
        {
            Self::charge_pipeline_gas_asset_fee(
                state_transaction,
                authority,
                transaction,
                &gas_asset_id_str,
                direct_gas_used,
                fee_sponsor.as_ref(),
            )?;
        }
        if !skip_nexus_fee {
            Self::charge_nexus_fees(
                state_transaction,
                authority,
                transaction,
                fee_sponsor,
                tx_bytes_len,
                instruction_count,
                direct_gas_used,
            )?;
        }
        Ok(())
    }
    /// Check bounded proof attachments against native registry metadata and block dedup state.
    ///
    /// This performs the same preliminary check used by transaction execution. It neither
    /// verifies a complete operation relation nor grants execution or publication authority.
    ///
    /// # Errors
    /// Returns the original structural, registry, activity, namespace, budget or duplicate error.
    #[cfg(feature = "zk-preverify")]
    pub fn preverify_attachments(
        state_transaction: &mut StateTransaction<'_, '_>,
        transaction: &SignedTransaction,
    ) -> Result<(), ValidationFail> {
        use iroha_data_model::proof::ProofAttachment;
        let md = transaction.metadata();
        let namespace_hint = md
            .get("contract_alias")
            .and_then(|value| value.try_into_any_norito::<String>().ok())
            .and_then(|raw| {
                raw.trim()
                    .parse::<iroha_data_model::smart_contract::ContractAlias>()
                    .ok()
            })
            .map(|alias| alias.dataspace_segment().to_owned())
            .or_else(|| {
                md.get("contract_address")
                    .and_then(|value| value.try_into_any_norito::<String>().ok())
                    .and_then(|raw| {
                        raw.trim()
                            .parse::<iroha_data_model::smart_contract::ContractAddress>()
                            .ok()
                    })
                    .and_then(|contract_address| contract_address.dataspace_id().ok())
                    .and_then(|dataspace_id| {
                        state_transaction
                            .nexus
                            .dataspace_catalog
                            .by_id(dataspace_id)
                            .map(|entry| entry.alias.clone())
                    })
            });
        // Process the signed transaction's bounded ZK attachments.
        if transaction.attachments().is_some() {
            // Canonicalize verification order for determinism.
            let mut list_sorted =
                transaction.proof_attachments_v1().map_err(|error| {
                    match error.into_local_refusal() {
                        Ok(original) => state_transaction.defer_execution(original),
                        Err(completed) => ValidationFail::NotPermitted(format!(
                            "malformed signed attachments: {completed}"
                        )),
                    }
                })?;
            list_sorted.sort_by(|a, b| {
                let ah = crate::zk::hash_proof(&a.proof);
                let bh = crate::zk::hash_proof(&b.proof);
                (a.backend.as_str(), ah).cmp(&(b.backend.as_str(), bh))
            });
            for attachment in list_sorted {
                if let Some((field, message)) = attachment.structural_error() {
                    return Err(ValidationFail::NotPermitted(format!(
                        "malformed proof attachment: {field} {message}"
                    )));
                }
                let ProofAttachment {
                    backend,
                    proof,
                    vk_ref,
                    vk_commitment,
                    ..
                } = attachment;
                // Sanity: proof.backend should match attachment backend
                if proof.backend != backend {
                    return Err(ValidationFail::NotPermitted(
                        "proof backend mismatch".to_owned(),
                    ));
                }
                if vk_ref.backend != backend {
                    return Err(ValidationFail::NotPermitted(
                        "verifying key backend mismatch".to_owned(),
                    ));
                }
                if crate::zk::is_production_claim_backend_label(backend.as_str()) {
                    return Err(ValidationFail::NotPermitted(
                        "readiness-claim proof backends are not supported".to_owned(),
                    ));
                }
                if crate::zk::is_trusted_setup_backend_label(backend.as_str()) {
                    return Err(ValidationFail::NotPermitted(
                        "trusted-setup proof backends are not supported".to_owned(),
                    ));
                }
                if crate::zk::is_developer_only_backend_label(backend.as_str()) {
                    return Err(ValidationFail::NotPermitted(
                        "developer-only proof backends are not supported".to_owned(),
                    ));
                }
                if !crate::zk::is_verifier_backend_registry_label_v1(backend.as_str()) {
                    return Err(ValidationFail::NotPermitted(
                        "unsupported proof backends are not supported".to_owned(),
                    ));
                }
                // If a VK reference is provided without a commitment, check existence in
                // WSV. If a commitment is provided, skip the lookup to keep pre-verify
                // stateless and cheap.
                if vk_commitment.is_none()
                    && state_transaction
                        .world
                        .verifying_keys
                        .get(&vk_ref)
                        .is_none()
                {
                    return Err(ValidationFail::NotPermitted(format!(
                        "referenced verifying key missing: {}::{}",
                        vk_ref.backend, vk_ref.name
                    )));
                }
                // Perform lightweight pre-verify (dedup + tag sanity).
                let block_height = state_transaction.block_height();
                let (expected_commitment, vk_active) =
                    if let Some(rec) = state_transaction.world.verifying_keys.get(&vk_ref) {
                        if let Some(ns_hint) = namespace_hint.as_deref() {
                            if !rec.namespace.is_empty() && rec.namespace != ns_hint {
                                return Err(ValidationFail::NotPermitted(
                                    "verifying key namespace/manifest mismatch".to_owned(),
                                ));
                            }
                        }
                        (Some(rec.commitment), rec.is_active_at(block_height))
                    } else {
                        (vk_commitment, false)
                    };
                let res = state_transaction
                    .preverify_proof(
                        &proof,
                        None,
                        state_transaction.zk.preverify_budget_bytes,
                        vk_commitment,
                        expected_commitment,
                        vk_active,
                    )
                    .map_err(|original| {
                        state_transaction.defer_execution(
                            crate::execution_attempt::ExecutionDeferred::from(original),
                        )
                    })?;
                match res {
                    PreverifyResult::Accepted => {}
                    PreverifyResult::Duplicate => {
                        return Err(ValidationFail::NotPermitted(
                            "duplicate proof in block".to_owned(),
                        ));
                    }
                    PreverifyResult::UnsupportedBackend => {
                        return Err(ValidationFail::NotPermitted(
                            "unsupported proof backend".to_owned(),
                        ));
                    }
                    PreverifyResult::ProofTooBig => {
                        return Err(ValidationFail::NotPermitted("proof too big".to_owned()));
                    }
                    PreverifyResult::MalformedProof => {
                        return Err(ValidationFail::NotPermitted("malformed proof".to_owned()));
                    }
                    PreverifyResult::PreverifyBudgetExceeded => {
                        return Err(ValidationFail::NotPermitted(
                            "pre-verify budget exceeded".to_owned(),
                        ));
                    }
                    PreverifyResult::VerifyingKeyMissing => {
                        return Err(ValidationFail::NotPermitted(
                            "verifying key missing".to_owned(),
                        ));
                    }
                    PreverifyResult::VerifyingKeyMismatch => {
                        return Err(ValidationFail::NotPermitted(
                            "verifying key mismatch".to_owned(),
                        ));
                    }
                    PreverifyResult::NamespaceMismatch => {
                        return Err(ValidationFail::NotPermitted(
                            "verifying key namespace/manifest mismatch".to_owned(),
                        ));
                    }
                    PreverifyResult::VerifyingKeyInactive => {
                        return Err(ValidationFail::NotPermitted(
                            "verifying key inactive".to_owned(),
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Execute the original borrowed [`SignedTransaction`].
    ///
    /// Fee, authority, attachment and executable inputs remain bound to this signed owner.
    /// # Errors
    /// Returns an error when IVM preparation or execution fails, or the executor denies the operation.
    pub fn execute_transaction(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        transaction: &SignedTransaction,
        ivm_cache: &mut IvmCache,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        state_transaction.bind_kagemusha_load_entrypoint_v1(None);
        state_transaction.bind_privacy_transaction_intent_v1(None);
        state_transaction
            .ensure_execution_ready()
            .map_err(crate::execution_attempt::ExecutionAttemptError::Deferred)?;
        let result =
            self.execute_transaction_body(state_transaction, authority, transaction, ivm_cache);
        state_transaction.clear_sccp_forgery_source();
        // The self-claim marker belongs to the signed body only, never to later callbacks.
        state_transaction.sccp_exempt_self_claim = None;
        state_transaction.bind_kagemusha_load_entrypoint_v1(None);
        state_transaction.bind_privacy_transaction_intent_v1(None);
        // A local refusal has no completed execution, fee, gas, or effect result.
        // The transaction overlay remains poisoned until its owner drops it.
        state_transaction
            .ensure_execution_ready()
            .map_err(crate::execution_attempt::ExecutionAttemptError::Deferred)?;
        let effects = state_transaction.finish_execution_effect_budget();
        state_transaction.close_execution_fee_meter();
        effects?;
        result.map_err(Into::into)
    }

    /// Run one admitted signed body under its actual root effect and fee owners.
    #[allow(clippy::too_many_lines)]
    fn execute_transaction_body(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        transaction: &SignedTransaction,
        ivm_cache: &mut IvmCache,
    ) -> Result<(), ValidationFail> {
        #[cfg(test)]
        signed_source_observation::record(transaction);
        root_scope::ensure_executable_scope(state_transaction, transaction.instructions())?;
        state_transaction.bind_privacy_transaction_intent_v1(None);
        state_transaction.bind_private_settlement_carrier_v1(None);
        state_transaction.bind_governance_ballot_entrypoint_v1(None);
        if transaction.authority() != authority {
            return Err(ValidationFail::InternalError(
                "signed authority mismatch".into(),
            ));
        }
        reject_committed_direct_payment(state_transaction, transaction)
            .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
        trace!("Running transaction execution");
        let privacy_intent_binding =
            crate::privacy::signed_privacy_transaction_intent_binding_v1(transaction)?;
        state_transaction.bind_privacy_transaction_intent_v1(privacy_intent_binding);
        state_transaction.bind_privacy_transaction_source_v1(&transaction)?;
        let private_settlement_carrier_binding =
            crate::private_settlement::carrier::signed_private_settlement_carrier_binding_v1(
                transaction,
            )?;
        state_transaction.bind_private_settlement_carrier_v1(private_settlement_carrier_binding);
        let governance_ballot_binding =
            crate::state::standalone_governance_ballot_instruction_v1(transaction.instructions())
                .map_err(|message| ValidationFail::NotPermitted(message.to_owned()))?;
        let call_hash = transaction.hash_as_entrypoint();
        state_transaction.tx_call_hash = Some(iroha_crypto::Hash::from(call_hash));
        let tx_hash = transaction.hash();
        state_transaction.current_tx_hash = Some(tx_hash.clone());
        let tx_bytes_len = norito::canonical_frame_len(transaction.payload()).map_err(|err| {
            ValidationFail::InternalError(format!(
                "failed to encode transaction for fee metering: {err}"
            ))
        })?;
        let fee_sponsor = transaction
            .fee_payment_intent()
            .sponsor_program()
            .map(|(program_id, _)| program_id.clone());
        Self::validate_root_fee_policy(state_transaction)?;
        let (fee_exemption, sccp_class) = if is_initial_genesis_context(state_transaction) {
            (ExecutionFeeExemption::Exempt, None)
        } else {
            transaction_fee_exemption(state_transaction, transaction)
                .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?
        };
        let skip_nexus_fee = fee_exemption.skips_fee_on_success();
        // Only an exempt self-claim makes the self-claim fee due at release (§4.12.4).
        state_transaction.sccp_exempt_self_claim = (sccp_class
            == Some(crate::smartcontracts::isi::sccp::admission::SccpExemptClassV1::SelfClaim))
        .then(|| authority.clone());
        // Quote the predecessor-frozen pipeline fee snapshot shared with block execution.
        let fee_quote = if !skip_nexus_fee {
            Some(
                evaluate_nexus_fee_admission_payload(
                    &state_transaction.world,
                    &state_transaction.nexus,
                    root_fees::execution_policy(state_transaction)
                        .map_err(|reason| ValidationFail::InternalError(reason.into()))?,
                    &state_transaction.pipeline,
                    &state_transaction.zk.gas,
                    transaction.payload(),
                    state_transaction.block_unix_timestamp_ms(),
                    state_transaction.block_height(),
                    state_transaction.current_dataspace_id,
                    true,
                )
                .map_err(|error| {
                    state_transaction.attempt_error_to_validation_fail(
                        error.map_rejection(nexus_fee_admission_error_to_validation_fail),
                    )
                })?,
            )
        } else {
            None
        };
        fastpq_rejection_tail::preflight(
            state_transaction,
            authority,
            transaction,
            fee_quote.as_ref(),
            governance_ballot_binding.as_ref(),
        )?;
        state_transaction.bind_governance_ballot_entrypoint_v1(governance_ballot_binding);
        state_transaction.bind_kagemusha_load_entrypoint_v1(Some(transaction));
        state_transaction.begin_execution_fee_meter(transaction, tx_bytes_len, fee_exemption)?;
        state_transaction.begin_execution_effect_budget(transaction)?;
        // Multisig participation is signed by individual signatories. Approved
        // inner instructions execute separately under the multisig account.
        {
            if let Ok(account) = state_transaction.world.account(authority) {
                if account.id().controller().multisig_policy().is_some() {
                    #[cfg(feature = "telemetry")]
                    crate::telemetry::record_social_rejection(
                        state_transaction.telemetry,
                        "multisig_direct_sign",
                    );
                    return Err(ValidationFail::NotPermitted(
                        "direct signing with multisig accounts is forbidden; use multisig propose/approve"
                            .to_owned(),
                    ));
                }
            }
        }
        // Gas asset and limit are explicit signature-bound fee intent fields.
        let md = transaction.metadata();
        let gas_asset_opt = transaction
            .fee_payment_intent()
            .charge_limits()
            .iter()
            .find(|limit| limit.kind == FeeChargeKind::PipelineGas)
            .map(|limit| limit.asset_definition_id.canonical_address());
        let gas_limit_md = transaction_gas_limit(transaction);
        let pipeline_gas_bound = fee_bound_for_admission(transaction, &state_transaction.zk.gas)
            .map_err(|attempt| {
                state_transaction.attempt_error_to_validation_fail(
                    attempt.map_rejection(nexus_fee_admission_error_to_validation_fail),
                )
            })?
            .2;
        let pipeline_gas = &state_transaction.pipeline.gas;
        if !skip_nexus_fee
            && pipeline_gas_component_enabled(
                root_fees::execution_policy(state_transaction)
                    .map_err(|reason| ValidationFail::InternalError(reason.into()))?,
                &state_transaction.pipeline,
            )
            && pipeline_gas_bound > 0
        {
            let Some(ref gas_asset_id_str) = gas_asset_opt else {
                return Err(ValidationFail::NotPermitted(
                    "missing pipeline gas charge limit in fee payment intent".to_owned(),
                ));
            };
            if !pipeline_gas
                .accepted_assets
                .iter()
                .any(|a| a == gas_asset_id_str)
            {
                return Err(ValidationFail::NotPermitted(format!(
                    "gas asset `{gas_asset_id_str}` is not accepted by committed policy"
                )));
            }
        }
        let execution_dataspace = root_scope::captured_dataspace(state_transaction)?;
        enforce_transaction_contract_permission_before_proof_verification(
            state_transaction,
            authority,
            transaction,
            ivm_cache,
            state_transaction.block_height(),
            execution_dataspace,
        )
        .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
        // Admission refuses a declared attachment-backed action that no route
        // consumes; a transaction that reaches the executor another way is
        // refused here as well, in every build.
        if let Some(error) = crate::tx::first_unsupported_attachment_operation(transaction) {
            return Err(ValidationFail::NotPermitted(format!(
                "malformed signed attachments: {error}"
            )));
        }
        #[cfg(feature = "zk-preverify")]
        Self::preverify_attachments(state_transaction, transaction)?;
        let mut proved_contract_runtime_context = None;
        let mut proved_entrypoint_authorization = None;
        // Verify proof-carrying executables while borrowing the actual signed payload.
        // Fee, authority, attachment and execution bindings share this same source.
        let ivm_proved_replay = if let Executable::IvmProved(proved) = transaction.instructions() {
            if gas_limit_md.is_none() {
                return Err(ValidationFail::NotPermitted(
                    "missing gas limit in fee payment intent".to_owned(),
                ));
            }
            let summary = ivm_cache
                .summarize_program(proved.bytecode.as_ref())
                .map_err(|e| ValidationFail::InternalError(e.to_string()))?;
            let meta = summary.metadata.clone();
            validate_governed_ivm_proved_execution_policy(state_transaction, &meta)?;
            crate::pipeline::overlay::validate_contract_binding(
                state_transaction,
                transaction.payload(),
                &summary,
            )
            .map_err(|error| {
                overlay_build_error_to_attempt_validation_fail(state_transaction, error)
            })?;
            let selector = requested_contract_entrypoint(transaction.metadata())?.ok_or_else(|| {
                ValidationFail::NotPermitted(
                    "self-describing proved raw-IVM contract dispatch requires explicit contract_entrypoint metadata"
                        .to_owned(),
                )
            })?;
            let identity = require_raw_contract_runtime_identity(
                &state_transaction.world,
                summary.code_hash,
                transaction.metadata(),
            )?;
            let authorization = authorize_prepared_raw_contract_selector(
                &state_transaction.world,
                authority,
                summary.prepared_contract(),
                &selector,
                &identity,
            )
            .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
            let contract_subject =
                code::fetch_bound_contract_subject(state_transaction, &identity.contract_address)
                    .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract instance `{}` has no valid subject binding",
                        identity.contract_address
                    ))
                })?;
            proved_contract_runtime_context = Some(ContractRuntimeExecutionContext {
                contract_subject,
                contract_address: identity.contract_address,
                contract_alias: identity.contract_alias,
                entrypoint: selector,
            });
            proved_entrypoint_authorization = Some(authorization);
            crate::pipeline::overlay::enforce_manifest_is_pre_registered(
                state_transaction,
                transaction.payload(),
                summary.code_hash,
            )
            .map_err(|error| {
                overlay_build_error_to_attempt_validation_fail(state_transaction, error)
            })?;
            let mut replay_work = crate::pipeline::overlay::IvmProvedReplayWork::default();
            let replay_result = crate::pipeline::overlay::verify_ivm_proved_execution(
                state_transaction,
                transaction,
                proved,
                &summary,
                state_transaction.execution_cycle_budget()?,
                &mut replay_work,
            );
            let observed_gas = replay_work
                .gas_used()
                .map_err(|message| ValidationFail::InternalError(message.into()))?;
            if let Some(gas) = observed_gas {
                // Transfer actual work before any fallible binding or replay
                // admission. Even an otherwise valid replay can exceed AXT or
                // remaining block capacity; its completed VM work still exists.
                state_transaction.last_tx_gas_used =
                    state_transaction.last_tx_gas_used.saturating_add(gas);
                match &replay_result {
                    Ok(replay) => state_transaction.record_execution_fee_instructions(
                        replay
                            .queued
                            .iter()
                            .filter(|effect| effect.instruction().is_some())
                            .count(),
                        gas,
                    )?,
                    Err(_) => state_transaction.record_execution_fee_vm_work(gas)?,
                }
            }
            let replay = replay_result.map_err(|error| {
                overlay_build_error_to_attempt_validation_fail(state_transaction, error)
            })?;
            if observed_gas != Some(replay.gas_used) {
                return Err(ValidationFail::InternalError(
                    "verified replay lost its actual gas owner".into(),
                ));
            }
            crate::pipeline::overlay::require_ivm_proved_gas_within_limit(
                transaction,
                replay.gas_used,
            )
            .map_err(overlay_build_error_to_validation_fail)?;
            Some(replay)
        } else {
            None
        };
        let tx_creation_time_ms =
            u64::try_from(transaction.creation_time().as_millis()).unwrap_or(u64::MAX);
        debug_assert_eq!(transaction.authority(), authority, "authority mismatch");
        // Every executable borrows its original signed source. Native invocation
        // authority still requires separate exact attachment custody.
        let needs_native_custody = transaction.attachments().is_some_and(|attachments| {
            attachments.actions().iter().any(|action| {
                matches!(
                    action.operation,
                    iroha_data_model::transaction::signed_attachments::ActionOperationV1::NativeInstruction(_)
                )
            })
        });
        let mut attachment_custody = if needs_native_custody {
            Some(
                attachment_custody::AttachmentCustody::new(transaction).map_err(|error| {
                    attachment_direct::custody_validation_error(state_transaction, error)
                })?,
            )
        } else {
            None
        };
        let execution_result = match (
            self,
            executable_source::Body::original(transaction.instructions()),
        ) {
            (
                Self::Initial | Self::UserProvided(_),
                executable_source::Body::Instructions(instructions),
            ) => self.execute_metered_instructions(
                state_transaction,
                authority,
                transaction,
                instructions,
                None,
                None,
                None,
                tx_bytes_len,
                gas_limit_md,
                false,
                gas_asset_opt,
                fee_sponsor,
                skip_nexus_fee,
                attachment_custody.as_mut(),
            ),
            (Self::Initial | Self::UserProvided(_), executable_source::Body::IvmProved) => {
                let replay = ivm_proved_replay
                    .expect("proved execution must retain the deterministic replay verified above");
                let instructions: Vec<_> = replay
                    .queued
                    .iter()
                    .filter_map(|queued| queued.instruction().cloned())
                    .collect();
                self.execute_metered_instructions(
                    state_transaction,
                    authority,
                    transaction,
                    &instructions,
                    Some(replay),
                    proved_contract_runtime_context.as_ref(),
                    proved_entrypoint_authorization.as_ref(),
                    tx_bytes_len,
                    gas_limit_md,
                    true,
                    gas_asset_opt,
                    fee_sponsor,
                    false,
                    None,
                )
            }
            (
                Self::Initial | Self::UserProvided(_),
                executable_source::Body::ContractCall(call),
            ) => {
                let gas_limit = gas_limit_md.ok_or_else(|| {
                    ValidationFail::NotPermitted(
                        "missing gas limit in fee payment intent".to_owned(),
                    )
                })?;
                let block_remaining = if state_transaction.gas_limit_per_block == 0 {
                    u64::MAX
                } else {
                    state_transaction
                        .gas_limit_per_block
                        .saturating_sub(state_transaction.gas_used_in_block_so_far)
                };
                let effective_limit = gas_limit.min(block_remaining);
                let outcome = self.execute_contract_invocation(
                    state_transaction,
                    authority,
                    call,
                    ivm_cache,
                    effective_limit,
                    tx_creation_time_ms,
                    None,
                )?;
                Self::settle_live_transaction_fees(
                    state_transaction,
                    authority,
                    transaction,
                    outcome.gas_used,
                    0,
                    tx_bytes_len,
                    gas_asset_opt,
                    fee_sponsor,
                    skip_nexus_fee,
                )
            }
            (Self::Initial | Self::UserProvided(_), executable_source::Body::Batch(items)) => {
                if items.is_empty() {
                    return Err(ValidationFail::NotPermitted(
                        "executable batch must not be empty".to_owned(),
                    ));
                }
                let contains_contract_call = items
                    .iter()
                    .any(|item| matches!(item, ExecutableBatchItem::ContractCall(_)));
                let gas_limit = if contains_contract_call {
                    Some(gas_limit_md.ok_or_else(|| {
                        ValidationFail::NotPermitted(
                            "missing gas limit in fee payment intent".to_owned(),
                        )
                    })?)
                } else {
                    gas_limit_md
                };
                let explicit_instructions = AuthoredBatchInstructions::new(items.as_ref());
                let explicit_gas = explicit_instructions
                    .clone()
                    .try_fold(0_u64, |total, instruction| {
                        isi_gas::meter_instruction(instruction, &state_transaction.zk.gas)
                            .map(|gas| total.saturating_add(gas))
                    })
                    .map_err(|original| {
                        state_transaction.defer_execution(
                            crate::execution_attempt::ExecutionDeferred::from(original),
                        )
                    })?;
                let confidential_delta = crate::gas::sum_confidential_gas_costs(
                    explicit_instructions.clone(),
                    &state_transaction.zk.gas,
                )
                .map_err(|original| {
                    state_transaction.defer_execution(
                        crate::execution_attempt::ExecutionDeferred::from(original),
                    )
                })?;
                if let Some(limit) = gas_limit
                    && explicit_gas > limit
                {
                    return Err(ValidationFail::NotPermitted(format!(
                        "out of gas: used {explicit_gas} > limit {limit}"
                    )));
                }
                let block_remaining = if state_transaction.gas_limit_per_block == 0 {
                    u64::MAX
                } else {
                    state_transaction
                        .gas_limit_per_block
                        .saturating_sub(state_transaction.gas_used_in_block_so_far)
                };
                if explicit_gas > block_remaining {
                    return Err(ValidationFail::NotPermitted(format!(
                        "block gas limit exceeded: {} > {}",
                        state_transaction
                            .gas_used_in_block_so_far
                            .saturating_add(explicit_gas),
                        state_transaction.gas_limit_per_block
                    )));
                }
                let mut gas_used = explicit_gas;
                state_transaction
                    .admit_authored_execution_effects(explicit_instructions.clone())?;
                // Native ISIs are metered as one authored set, matching the existing
                // `Executable::Instructions` rejected-business fee behavior.
                state_transaction.last_tx_gas_used = explicit_gas;
                state_transaction
                    .record_execution_fee_instructions(explicit_instructions.len(), explicit_gas)?;
                if confidential_delta > 0 {
                    state_transaction.record_confidential_gas_delta(confidential_delta);
                }
                for (index, item) in items.iter().enumerate() {
                    match item {
                        ExecutableBatchItem::Instruction(instruction) => {
                            // Mixed batches intentionally do not inherit the pure-ISI
                            // deployment self-bootstrap exception.
                            let direct_index = Self::direct_stream_token_instruction_index(
                                state_transaction,
                                transaction,
                                instruction,
                                index,
                                true,
                            )?;
                            state_transaction.current_direct_stream_token_instruction_index =
                                direct_index;
                            state_transaction.current_direct_stream_token_reputation_payload = None;
                            state_transaction.current_direct_reputation_policy_origin = None;
                            state_transaction
                                .current_direct_stream_token_gateway_instruction_index =
                                Self::direct_stream_token_gateway_instruction_index(
                                    state_transaction,
                                    transaction,
                                    instruction,
                                    index,
                                    true,
                                )?;
                            state_transaction.current_direct_final_promotion_operation_origin =
                                None;
                            state_transaction.current_direct_musubi_pin_outbox_origin = None;
                            state_transaction.arm_direct_privacy_instruction_v1(
                                transaction,
                                authority,
                                instruction,
                                index,
                                true,
                            );
                            let result = if attachment_custody.is_some() {
                                self.execute_original_direct_instruction(
                                    state_transaction,
                                    authority,
                                    instruction,
                                    index,
                                    attachment_custody.as_mut(),
                                )
                            } else {
                                self.execute_instruction_with_profile_and_contract_runtime_context(
                                    state_transaction,
                                    authority,
                                    instruction,
                                    InstructionExecutionProfile::Runtime,
                                    None,
                                )
                            };
                            state_transaction.current_direct_privacy_instruction_index = None;
                            state_transaction.current_direct_kagemusha_load_instruction_index =
                                None;
                            state_transaction.current_direct_stream_token_instruction_index = None;
                            state_transaction.current_direct_stream_token_reputation_payload = None;
                            state_transaction.current_direct_reputation_policy_origin = None;
                            state_transaction
                                .current_direct_stream_token_gateway_instruction_index = None;
                            state_transaction.current_direct_final_promotion_operation_origin =
                                None;
                            state_transaction.current_direct_musubi_pin_outbox_origin = None;
                            result?;
                        }
                        ExecutableBatchItem::ContractCall(call) => {
                            state_transaction.current_direct_privacy_instruction_index = None;
                            state_transaction.current_direct_kagemusha_load_instruction_index =
                                None;
                            state_transaction.current_direct_stream_token_instruction_index = None;
                            state_transaction.current_direct_stream_token_reputation_payload = None;
                            state_transaction.current_direct_reputation_policy_origin = None;
                            state_transaction
                                .current_direct_stream_token_gateway_instruction_index = None;
                            state_transaction.current_direct_final_promotion_operation_origin =
                                None;
                            state_transaction.current_direct_musubi_pin_outbox_origin = None;
                            let remaining = live_batch_contract_execution_limit(
                                gas_limit,
                                gas_used,
                                block_remaining,
                                state_transaction.last_tx_gas_used,
                            );
                            let outcome = self.execute_contract_invocation(
                                state_transaction,
                                authority,
                                call,
                                ivm_cache,
                                remaining,
                                tx_creation_time_ms,
                                None,
                            )?;
                            gas_used = gas_used.saturating_add(outcome.gas_used);
                        }
                    }
                }
                Self::settle_live_transaction_fees(
                    state_transaction,
                    authority,
                    transaction,
                    gas_used,
                    explicit_instructions.len(),
                    tx_bytes_len,
                    gas_asset_opt,
                    fee_sponsor,
                    skip_nexus_fee,
                )
            }
            (Self::Initial | Self::UserProvided(_), executable_source::Body::Ivm(bytes)) => self
                .execute_raw_ivm_transaction(
                    state_transaction,
                    authority,
                    transaction,
                    ivm_cache,
                    bytes,
                    RawIvmTransactionInputs {
                        metadata: md,
                        gas_limit_md,
                        gas_asset_opt,
                        fee_sponsor,
                        skip_nexus_fee,
                        tx_bytes_len,
                    },
                ),
        };
        execution_result?;
        if let Some(custody) = attachment_custody {
            custody.finish().map_err(|error| {
                attachment_direct::custody_validation_error(state_transaction, error)
            })?;
        }
        Ok(())
    }

    /// Run only the selected raw-IVM branch, retaining its original fee and effect order.
    /// Native instructions do not reserve the runtime and host locals from this stage.
    #[allow(clippy::too_many_lines)]
    fn execute_raw_ivm_transaction(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        transaction_for_fee: &SignedTransaction,
        ivm_cache: &mut IvmCache,
        bytes: &iroha_data_model::transaction::executable::IvmBytecode,
        inputs: RawIvmTransactionInputs<'_>,
    ) -> Result<(), ValidationFail> {
        use crate::smartcontracts::ivm::host::CoreHostImpl as CoreCoreHost;
        let RawIvmTransactionInputs {
            metadata: md,
            gas_limit_md,
            gas_asset_opt,
            fee_sponsor,
            skip_nexus_fee,
            tx_bytes_len,
        } = inputs;
        // IVM path: run the bytecode through the VM with CoreHost, enqueueing ISIs,
        // then apply them via the standard executor logic.
        // Set gas limit per transaction (payer-provided), clamped to remaining block budget.
        // Read the signature-bound payer cap borrowed from the original transaction.
        let gas_limit_md = gas_limit_md.ok_or_else(|| {
            ValidationFail::NotPermitted("missing gas limit in fee payment intent".to_owned())
        })?;
        let block_remaining = if state_transaction.gas_limit_per_block == 0 {
            u64::MAX
        } else {
            state_transaction
                .gas_limit_per_block
                .saturating_sub(state_transaction.gas_used_in_block_so_far)
        };
        let effective_limit = gas_limit_md.min(block_remaining);
        let admitted = ivm_cache
            .summarize_executable(bytes.as_ref())
            .map_err(|error| {
                state_transaction.vm_error_to_validation_fail(
                    error,
                    crate::smartcontracts::ivm::program_admission_error,
                )
            })?;
        let summary = match admitted {
            ExecutableProgramSummary::Contract(summary) => summary,
            ExecutableProgramSummary::Generic(summary) => {
                let artifact =
                    root_scope::captured_artifact_id(state_transaction, summary.code_hash)?;
                crate::smartcontracts::ivm::validate_generic_execution_context(
                    &state_transaction.world,
                    md,
                    artifact,
                )?;
                let effective_cycles =
                    validate_prepared_ivm_execution_policy(state_transaction, &summary.metadata)?;
                let prepared_contract_cache = ivm_cache.prepared_contract_cache();
                let amx_analysis =
                    ivm_cache
                        .analyze_generic_program(&summary)
                        .map_err(|error| {
                            state_transaction
                                .program_analysis_error_to_validation_fail(error, "generic-program")
                        })?;
                let streaming_metadata = crate::pipeline::overlay::resolve_streaming_metadata(
                    state_transaction,
                    authority,
                );
                code::validate_bound_contract_records_by_subject(state_transaction)
                    .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
                let heap_limit = state_transaction
                    .world
                    .parameters
                    .get()
                    .smart_contract()
                    .memory()
                    .get();
                let mut runtime = ivm_cache
                    .checkout_generic_runtime(&summary, effective_limit, heap_limit)
                    .map_err(|error| {
                        state_transaction.vm_error_to_validation_fail(error, |error| {
                            ValidationFail::InternalError(error.to_string())
                        })
                    })?;
                runtime.set_max_cycles(effective_cycles.get());
                runtime.set_gas_limit(effective_limit);
                let accounts = state_transaction.accounts_snapshot();
                let mut host =
                    CoreCoreHost::with_accounts(authority.clone(), Arc::clone(&accounts));
                host.set_output_limits_from_parameters(
                    state_transaction.world.parameters.get().smart_contract(),
                );
                host.set_generic_execution();
                host.set_prepared_contract_cache(prepared_contract_cache);
                host.set_amx_analysis(amx_analysis);
                host.set_amx_limits(
                    crate::smartcontracts::ivm::host::CoreHost::amx_limits_from_config(
                        state_transaction.pipeline(),
                    ),
                );
                host.hydrate_axt_state(state_transaction).map_err(|error| {
                    ValidationFail::InternalError(format!("invalid AXT policy snapshot: {error}"))
                })?;
                host.set_crypto_config(Arc::clone(&state_transaction.crypto));
                host.set_zk_config(&state_transaction.zk);
                host.set_public_inputs_from_parameters(state_transaction.world.parameters.get());
                let vrf_epoch_length =
                    crate::smartcontracts::ivm::host::vrf_epoch_length_from_state(
                        state_transaction,
                    )
                    .map_err(|error| {
                        state_transaction.attempt_error_to_validation_fail(
                            error.map_rejection(ValidationFail::InternalError),
                        )
                    })?;
                state_transaction.hydrate_zk_snapshots_for_execution(&mut host)?;
                host.set_query_state_with_vrf_epoch_seeds(state_transaction, vrf_epoch_length);
                crate::pipeline::overlay::apply_streaming_metadata(&mut host, streaming_metadata);
                host.set_chain_id(&state_transaction.chain_id);
                #[cfg(feature = "telemetry")]
                host.set_telemetry(state_transaction.telemetry.clone());

                let run_result = match state_transaction.execution_cycle_budget()? {
                    Some(budget) => runtime.run_with_host_and_cycle_budget(&mut host, budget),
                    None => runtime.run_with_host(&mut host),
                }
                .and_then(|()| host.finish_contract_result(&runtime).map(|_| ()));
                let gas_used = effective_limit.saturating_sub(runtime.remaining_gas());
                if let Err(err) = run_result {
                    let attempt =
                        crate::smartcontracts::ivm::map_vm_error_with_context_to_validation(
                            &runtime, err,
                        );
                    drop(host);
                    let error = match attempt {
                        crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                            return Err(state_transaction.defer_execution(reason));
                        }
                        crate::execution_attempt::ExecutionAttemptError::Rejected(error) => error,
                    };
                    // The actual VM consumed this work even though business effects
                    // will roll back. Retain it before returning the original failure.
                    state_transaction.last_tx_gas_used = gas_used;
                    state_transaction.record_execution_fee_vm_work(gas_used)?;
                    return Err(error);
                }
                let artifacts = host.into_execution_artifacts(None);
                let artifacts = match artifacts {
                    Err(crate::execution_attempt::ExecutionAttemptError::Deferred(original)) => {
                        return Err(state_transaction.defer_execution(original));
                    }
                    completed => completed,
                };
                // Consume the host borrow, then retain root work before artifact
                // validation/application can reject. Later nested work must not be
                // overwritten by a post-application root-gas assignment.
                state_transaction.last_tx_gas_used = gas_used;
                state_transaction.record_execution_fee_vm_work(gas_used)?;
                let artifacts = artifacts.map_err(|attempt| {
                    state_transaction.attempt_error_to_validation_fail(attempt)
                })?;
                let _executed = artifacts.apply_to_transaction(state_transaction, authority)?;
                Self::enforce_transaction_gas_fits_block(state_transaction, gas_used)?;
                if !skip_nexus_fee
                    && should_charge_pipeline_gas_asset(
                        skip_nexus_fee,
                        root_fees::execution_policy(state_transaction)
                            .map_err(|reason| ValidationFail::InternalError(reason.into()))?,
                        gas_asset_opt.as_deref(),
                    )
                    && let Some(gas_asset_id_str) = gas_asset_opt
                {
                    Self::charge_pipeline_gas_asset_fee(
                        state_transaction,
                        authority,
                        transaction_for_fee,
                        &gas_asset_id_str,
                        gas_used,
                        fee_sponsor.as_ref(),
                    )?;
                }
                if !skip_nexus_fee {
                    Self::charge_nexus_fees(
                        state_transaction,
                        authority,
                        transaction_for_fee,
                        fee_sponsor,
                        tx_bytes_len,
                        0,
                        gas_used,
                    )?;
                }
                return Ok(());
            }
        };
        let effective_cycles =
            validate_prepared_ivm_execution_policy(state_transaction, &summary.metadata)?;
        crate::pipeline::overlay::validate_contract_binding(
            state_transaction,
            transaction_for_fee.payload(),
            &summary,
        )
        .map_err(|error| {
            overlay_build_error_to_attempt_validation_fail(state_transaction, error)
        })?;
        let selector = requested_contract_entrypoint(md)?.ok_or_else(|| {
            ValidationFail::NotPermitted(
                "self-describing raw-IVM contract dispatch requires explicit contract_entrypoint metadata"
                    .to_owned(),
            )
        })?;
        let runtime_identity =
            require_raw_contract_runtime_identity(&state_transaction.world, summary.code_hash, md)?;
        code::ensure_contract_execution_allowed(
            &state_transaction.world,
            &runtime_identity.contract_address,
            state_transaction.block_height(),
        )
        .map_err(ValidationFail::NotPermitted)?;
        let entrypoint_authorization = authorize_prepared_raw_contract_selector(
            &state_transaction.world,
            authority,
            summary.prepared_contract(),
            &selector,
            &runtime_identity,
        )
        .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
        let contract_subject = code::fetch_bound_contract_subject(
            state_transaction,
            &runtime_identity.contract_address,
        )
        .ok_or_else(|| {
            ValidationFail::NotPermitted(format!(
                "contract instance `{}` has no valid subject binding",
                runtime_identity.contract_address
            ))
        })?;
        let transition = validate_prepared_contract_lifecycle_call(
            &state_transaction.world,
            &runtime_identity.contract_address,
            runtime_identity.code_hash,
            summary.prepared_contract(),
            &selector,
        )?;
        debug_assert!(
            transition.is_none(),
            "raw lifecycle selectors are rejected before state validation"
        );
        let mut contract_call_context = parse_prepared_contract_call_execution_context(
            md,
            summary.prepared_contract(),
            effective_limit,
        )
        .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
        if let Some(context) = contract_call_context.as_mut() {
            context.bind_runtime_identity(runtime_identity, contract_subject);
        }
        if let Some(context) = contract_call_context.as_ref() {
            enforce_contract_entrypoint_authorization(&state_transaction.world, authority, context)
                .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
        }
        let heap_limit = state_transaction
            .world
            .parameters
            .get()
            .smart_contract()
            .memory()
            .get();
        let mut runtime = summary
            .checkout_runtime(effective_limit, heap_limit)
            .map_err(|error| {
                state_transaction.vm_error_to_validation_fail(error, |error| {
                    ValidationFail::InternalError(error.to_string())
                })
            })?;
        runtime.set_max_cycles(effective_cycles.get());
        runtime.set_gas_limit(effective_limit);
        if let Some(context) = contract_call_context.as_ref() {
            if let Some(entrypoint_pc) = context.entrypoint_pc {
                let code_len = runtime.memory.code_len();
                runtime.set_register(1, code_len);
                runtime.set_program_counter(entrypoint_pc).map_err(|err| {
                    let selector = context.entrypoint.as_deref().unwrap_or("main");
                    ValidationFail::NotPermitted(format!(
                        "contract entrypoint `{selector}` resolved to invalid pc: {err}"
                    ))
                })?;
            }
        }
        let precharge_result = contract_call_context
            .as_ref()
            .and_then(ContractCallExecutionContext::prepared_argument_record)
            .map_or(Ok(()), |record| record.precharge_vm(&mut runtime));
        if let Err(error) = &precharge_result {
            runtime.record_boundary_fault(
                error,
                iroha_data_model::executor::fault::IvmFaultPositionV1::Initialization,
            );
        }
        let contract_runtime_context = contract_call_context
            .as_ref()
            .and_then(ContractCallExecutionContext::runtime_context);
        // Attach host with a snapshot of known accounts for vendor helpers when present.
        let accounts = state_transaction.accounts_snapshot();
        let mut host = if let Some(context) = contract_call_context {
            CoreCoreHost::with_accounts_and_argument_record(
                authority.clone(),
                Arc::clone(&accounts),
                context.argument_record,
            )
        } else {
            CoreCoreHost::with_accounts(authority.clone(), Arc::clone(&accounts))
        };
        host.set_output_limits_from_parameters(
            state_transaction.world.parameters.get().smart_contract(),
        );
        host.set_prepared_contract_cache(summary.prepared_contract_cache());
        host.hydrate_axt_state(state_transaction).map_err(|error| {
            ValidationFail::InternalError(format!("invalid AXT policy snapshot: {error}"))
        })?;
        host.set_crypto_config(Arc::clone(&state_transaction.crypto));
        host.set_zk_config(&state_transaction.zk);
        host.set_public_inputs_from_parameters(state_transaction.world.parameters.get());
        let vrf_epoch_length =
            crate::smartcontracts::ivm::host::vrf_epoch_length_from_state(state_transaction)
                .map_err(|error| {
                    state_transaction.attempt_error_to_validation_fail(
                        error.map_rejection(ValidationFail::InternalError),
                    )
                })?;
        state_transaction.hydrate_zk_snapshots_for_execution(&mut host)?;
        host.set_query_state_with_vrf_epoch_seeds(state_transaction, vrf_epoch_length);
        host.set_contract_runtime_context(contract_runtime_context.clone());
        host.set_contract_entrypoint_authorization(Some(entrypoint_authorization));
        // Keep the human-readable label available through SYSVAR_CHAIN_ID; AXT
        // hydration above installs the exact NetworkId used by VRF verification.
        host.set_chain_id(&state_transaction.chain_id);
        #[cfg(feature = "telemetry")]
        host.set_telemetry(state_transaction.telemetry.clone());

        let cycle_budget = state_transaction.execution_cycle_budget()?;
        let run_result = precharge_result
            .and_then(|()| match cycle_budget {
                Some(budget) => runtime.run_with_host_and_cycle_budget(&mut host, budget),
                None => runtime.run_with_host(&mut host),
            })
            .and_then(|()| host.finish_contract_result(&runtime).map(|_| ()));
        let gas_used = effective_limit.saturating_sub(runtime.remaining_gas());
        if let Err(err) = run_result {
            let attempt =
                crate::smartcontracts::ivm::map_vm_error_with_context_to_validation(&runtime, err);
            drop(host);
            let error = match attempt {
                crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                    return Err(state_transaction.defer_execution(reason));
                }
                crate::execution_attempt::ExecutionAttemptError::Rejected(error) => error,
            };
            state_transaction.last_tx_gas_used = gas_used;
            state_transaction.record_execution_fee_vm_work(gas_used)?;
            return Err(error);
        }
        // Retain completed root work before artifact validation/application;
        // a rejected artifact must not make an executed raw contract free.
        let artifacts = host.into_execution_artifacts(contract_runtime_context);
        let artifacts = match artifacts {
            Err(crate::execution_attempt::ExecutionAttemptError::Deferred(original)) => {
                return Err(state_transaction.defer_execution(original));
            }
            completed => completed,
        };
        state_transaction.last_tx_gas_used = gas_used;
        state_transaction.record_execution_fee_vm_work(gas_used)?;
        let artifacts = artifacts
            .map_err(|attempt| state_transaction.attempt_error_to_validation_fail(attempt))?;
        let _executed = artifacts.apply_to_transaction(state_transaction, authority)?;
        Self::enforce_transaction_gas_fits_block(state_transaction, gas_used)?;
        // Charge gas fees: if a gas asset was provided and accepted by policy.
        if !skip_nexus_fee
            && should_charge_pipeline_gas_asset(
                skip_nexus_fee,
                root_fees::execution_policy(state_transaction)
                    .map_err(|reason| ValidationFail::InternalError(reason.into()))?,
                gas_asset_opt.as_deref(),
            )
            && let Some(gas_asset_id_str) = gas_asset_opt
        {
            Self::charge_pipeline_gas_asset_fee(
                state_transaction,
                authority,
                transaction_for_fee,
                &gas_asset_id_str,
                gas_used,
                fee_sponsor.as_ref(),
            )?;
        }
        if !skip_nexus_fee {
            Self::charge_nexus_fees(
                state_transaction,
                authority,
                transaction_for_fee,
                fee_sponsor,
                tx_bytes_len,
                0,
                gas_used,
            )?;
        }
        Ok(())
    }
    /// Execute [`InstructionBox`].
    ///
    /// # Errors
    ///
    /// - Failed to prepare the IVM runtime;
    /// - Failed to execute the entrypoint of the IVM bytecode;
    /// - Executor denied the operation.
    pub fn execute_instruction(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
    ) -> Result<(), ValidationFail> {
        self.execute_instruction_with_profile_and_contract_runtime_context(
            state_transaction,
            authority,
            &instruction,
            InstructionExecutionProfile::Runtime,
            None,
        )
    }
    /// Execute [`InstructionBox`] using the runtime profile and an optional
    /// contract execution context for nested contract-originated instructions.
    pub(crate) fn execute_instruction_with_contract_runtime_context(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
    ) -> Result<(), ValidationFail> {
        if contract_runtime_context.is_some() {
            state_transaction.current_direct_privacy_instruction_index = None;
        }
        self.execute_instruction_with_profile_and_contract_runtime_context(
            state_transaction,
            authority,
            &instruction,
            InstructionExecutionProfile::Runtime,
            contract_runtime_context,
        )
    }
    /// Execute an overlay instruction through the same borrowed validation and apply path.
    pub(crate) fn execute_borrowed_overlay_instruction(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: &InstructionBox,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
    ) -> Result<(), ValidationFail> {
        state_transaction.clear_direct_parliament_candidate_testing();
        self.execute_instruction_with_profile_and_contract_runtime_context(
            state_transaction,
            authority,
            instruction,
            InstructionExecutionProfile::Runtime,
            contract_runtime_context,
        )
    }
    /// Execute [`InstructionBox`] using a specific execution profile.
    ///
    /// `InstructionExecutionProfile::Runtime` mirrors production behaviour.
    /// `InstructionExecutionProfile::Bench` disables logging so benchmarks/tests
    /// can run without installing the global logger while still enforcing policy checks.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationFail`] when the delegated executor rejects the instruction,
    /// or if preparing or running the IVM bytecode fails.
    pub fn execute_instruction_with_profile(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
        profile: InstructionExecutionProfile,
    ) -> Result<(), ValidationFail> {
        self.execute_instruction_with_profile_and_contract_runtime_context(
            state_transaction,
            authority,
            &instruction,
            profile,
            None,
        )
    }
    fn execute_instruction_with_profile_and_contract_runtime_context(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: &InstructionBox,
        profile: InstructionExecutionProfile,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
    ) -> Result<(), ValidationFail> {
        self.execute_instruction_with_policy_context(
            state_transaction,
            authority,
            instruction,
            profile,
            contract_runtime_context,
            None,
        )
    }
    fn execute_instruction_with_attachment_context(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: &InstructionBox,
        profile: InstructionExecutionProfile,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
        attachment_invocation: Option<&mut attachment_custody::Invocation<'_, '_>>,
    ) -> Result<(), ValidationFail> {
        // An attachment invocation carries no original parliament genesis authority.
        state_transaction.clear_direct_parliament_candidate_testing();
        self.execute_instruction_with_policy_context(
            state_transaction,
            authority,
            instruction,
            profile,
            contract_runtime_context,
            attachment_invocation,
        )
    }
    fn execute_instruction_with_policy_context(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: &InstructionBox,
        profile: InstructionExecutionProfile,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
        attachment_invocation: Option<&mut attachment_custody::Invocation<'_, '_>>,
    ) -> Result<(), ValidationFail> {
        // A user executor/contract cannot inherit original native genesis authority.
        if contract_runtime_context.is_some() || matches!(self, Self::UserProvided(_)) {
            state_transaction.clear_direct_parliament_candidate_testing();
        }
        root_scope::ensure_instruction_scope(instruction, state_transaction)?;
        ensure_contract_deployment_permission_mutation_allowed(
            state_transaction,
            authority,
            instruction,
        )?;
        ensure_contract_entrypoint_authorization_mutation_allowed(
            state_transaction,
            authority,
            instruction,
        )?;
        ensure_lifecycle_hook_cannot_mutate_contract_binding(
            contract_runtime_context,
            instruction,
        )?;
        ensure_contract_runtime_permission_mutation_allowed(
            authority,
            instruction,
            contract_runtime_context,
        )?;
        if let Some(result) = crate::smartcontracts::isi::escrow::controlled::execute_instruction(
            instruction,
            authority,
            state_transaction,
            contract_runtime_context,
        ) {
            return result.map_err(ValidationFail::InstructionFailed);
        }
        if let Some(result) = crate::smartcontracts::isi::asset_fi::execute_instruction(
            instruction,
            authority,
            state_transaction,
            contract_runtime_context,
        ) {
            return result.map_err(ValidationFail::InstructionFailed);
        }
        trace!("Running borrowed instruction execution");
        let instr_id = instruction.id();
        let result = match self {
            Self::Initial => Self::execute_initial_instruction(
                state_transaction,
                authority,
                instruction,
                profile,
                contract_runtime_context,
                attachment_invocation,
            ),
            Self::UserProvided(loaded_executor) => dispatch_instruction_with_ivm(
                loaded_executor,
                state_transaction,
                authority,
                instruction,
                attachment_invocation,
            ),
        };
        if let Err(err) = &result {
            iroha_logger::error!(
                ?profile,
                instr = %instr_id,
                ?err,
                "borrowed instruction execution failed"
            );
        }
        result
    }
    #[allow(clippy::too_many_lines, clippy::items_after_statements)]
    fn execute_initial_instruction(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: &InstructionBox,
        profile: InstructionExecutionProfile,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
        attachment_invocation: Option<&mut attachment_custody::Invocation<'_, '_>>,
    ) -> Result<(), ValidationFail> {
        fn has_modify_nft_metadata_permission(
            state_transaction: &mut StateTransaction<'_, '_>,
            authority: &AccountId,
            nft_id: &iroha_data_model::nft::NftId,
        ) -> Result<bool, ValidationFail> {
            let is_target_permission = |permission: &Permission| -> bool {
                permission
                    .payload()
                    .try_into_any_norito::<executor_permission::nft::CanModifyNftMetadata>()
                    .is_ok_and(|token| token.nft == *nft_id)
            };
            {
                let permissions = state_transaction
                    .world
                    .account_permissions_iter(authority)
                    .map_err(|err| {
                        ValidationFail::InstructionFailed(InstructionExecutionError::Find(err))
                    })?;
                if permissions.into_iter().any(is_target_permission) {
                    return Ok(true);
                }
            }
            for role_id in state_transaction.world.account_roles_iter(authority) {
                if let Some(role) = state_transaction.world.roles.get(role_id) {
                    if role.permissions.iter().any(is_target_permission) {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        }
        if matches!(profile, InstructionExecutionProfile::Runtime) {
            iroha_logger::trace!(
                instr = %instruction.id(),
                "executing instruction (Initial executor)"
            );
        }
        match MultisigInstructionBox::try_from(instruction) {
            Ok(multisig) => {
                return crate::smartcontracts::isi::multisig::execute_multisig_instruction(
                    state_transaction,
                    authority,
                    multisig,
                );
            }
            Err(err) => {
                let err =
                    match crate::smartcontracts::isi::multisig::multisig_instruction_decode_attempt(
                        err,
                        std::convert::identity,
                    ) {
                        crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                            return Err(state_transaction.defer_execution(reason));
                        }
                        crate::execution_attempt::ExecutionAttemptError::Rejected(error) => error,
                    };
                if let Some(custom) = instruction.as_any().downcast_ref::<CustomInstruction>() {
                    iroha_logger::error!(
                        ?err,
                        instr = %instruction.id(),
                        payload = %custom.payload(),
                        "failed to decode multisig custom instruction"
                    );
                }
            }
        }
        if instruction
            .as_any()
            .downcast_ref::<CustomInstruction>()
            .is_some()
        {
            return Err(ValidationFail::NotPermitted(
                "custom instructions require an executor upgrade".to_owned(),
            ));
        }
        let is_genesis = is_initial_genesis_context(state_transaction);
        validate_initial_permission_or_role_mutation(
            state_transaction,
            authority,
            instruction,
            is_genesis,
        )
        .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
        validate_initial_native_instruction_authority(
            state_transaction,
            authority,
            instruction,
            is_genesis,
        )
        .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
        if instruction
            .as_any()
            .downcast_ref::<iroha_data_model::isi::TransferAssetBatch>()
            .is_some()
            && !valid_contract_runtime_subject(
                &state_transaction.world,
                authority,
                contract_runtime_context,
            )
        {
            return Err(ValidationFail::NotPermitted(
                "Can't transfer asset batch from an inactive or inconsistent contract subject"
                    .to_owned(),
            ));
        }
        if let Some(register_role) = extract_register_role(instruction) {
            if is_reserved_multisig_role_id(register_role.object().id()) {
                return Err(ValidationFail::NotPermitted(
                    "reserved multisig role names may not be registered".to_owned(),
                ));
            }
            let role = register_role.object();
            let mut normalized_role = Role::new(role.id().clone(), role.grant_to().clone());
            for permission in role.inner().permissions() {
                let normalized =
                    normalize_role_permission_for_initial_executor(state_transaction, permission)?;
                if !is_genesis
                    && !initial_permission_delegation_allowed(
                        state_transaction,
                        authority,
                        &normalized,
                    )
                    .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?
                {
                    return Err(ValidationFail::NotPermitted(format!(
                        "Can't seed role with permission `{}`",
                        normalized.name()
                    )));
                }
                normalized_role = normalized_role.add_permission(normalized);
            }
            if !is_genesis {
                let can_manage_roles: Permission = executor_permission::role::CanManageRoles.into();
                let has_manage_roles = authority_has_permission(
                    &state_transaction.world,
                    authority,
                    &can_manage_roles,
                )?;
                if !has_manage_roles {
                    return Err(ValidationFail::NotPermitted(
                        "Can't register role".to_owned(),
                    ));
                }
            }
            Register::role(normalized_role)
                .execute(authority, state_transaction)
                .map_err(ValidationFail::from)?;
            return Ok(());
        }
        if extract_unregister_role(instruction).is_some() && !is_genesis {
            let can_manage_roles: Permission = executor_permission::role::CanManageRoles.into();
            if !authority_has_permission(&state_transaction.world, authority, &can_manage_roles)? {
                return Err(ValidationFail::NotPermitted(
                    "Can't unregister role".to_owned(),
                ));
            }
        }
        // Native fail-safe authorization remains active until an on-chain executor is
        // installed. Keep the specialized validation below for registration invariants
        // and CBDC-specific authority relationships that go beyond the generic gates.
        // Only attempt to decode as Register<Trigger> when the dynamic type matches.
        // Guard against panics in Norito deserialization for mismatched schemas.
        let is_reg_trigger = instruction
            .id()
            .starts_with(core::any::type_name::<Register<Trigger>>());
        let reg_trg = if is_reg_trigger {
            iroha_panic_hook::catch_unwind_suppressed(|| {
                Register::<Trigger>::decode(&mut &instruction.dyn_encode()[..])
            })
            .ok()
            .and_then(Result::ok)
        } else {
            None
        };
        if let Some(reg_trg) = reg_trg {
            // Allow in genesis, for the exact trigger owner, or if tx authority has
            // explicit CanRegisterTrigger { authority: <owner> }. Alias-domain
            // ownership does not authorize the canonical, domainless account.
            let trg_owner = reg_trg.object().action().authority().clone();
            let is_owner = authority == &trg_owner;
            // Prefer cached permission check; parse once per tx/account.
            let has_permission =
                (!is_genesis) && state_transaction.can_register_trigger_for(authority, &trg_owner);
            if !(is_genesis || is_owner || has_permission) {
                return Err(ValidationFail::NotPermitted(
                    "Can't register trigger owned by another account".to_owned(),
                ));
            }
        }
        if let Some(reg) = extract_register_dataspace_asset_definition(instruction) {
            crate::smartcontracts::isi::domain::isi::ensure_dataspace_asset_definition_registration_allowed(
                state_transaction, authority, &reg,
            ).map_err(ValidationFail::InstructionFailed)?;
        }
        if let Some(reg_asset_definition) = extract_register_asset_definition(instruction) {
            ensure_asset_definition_registration_allowed(
                state_transaction,
                authority,
                &reg_asset_definition,
            )?;
        }
        if !is_genesis
            && let Some(mint) = extract_mint_asset(instruction)
            && !can_mint_asset(&state_transaction.world, authority, mint.destination())?
        {
            return Err(ValidationFail::NotPermitted(
                "Can't mint an asset without owning its definition or an exact mint permission"
                    .to_owned(),
            ));
        }
        if !is_genesis
            && let Some(asset_definition_id) = extract_asset_definition_metadata_target(instruction)
            && !can_modify_asset_definition_metadata(
                &state_transaction.world,
                authority,
                &asset_definition_id,
            )?
        {
            return Err(ValidationFail::NotPermitted(
                "Can't modify asset-definition metadata without ownership or an exact permission"
                    .to_owned(),
            ));
        }
        if let Some(account_id) = extract_account_metadata_target(instruction) {
            if !is_genesis
                && !can_modify_account_metadata(&state_transaction.world, authority, &account_id)?
            {
                return Err(ValidationFail::NotPermitted(
                    "Can't set value to the metadata of another account".to_owned(),
                ));
            }
        }
        if let Some(nft_id) = instruction
            .as_any()
            .downcast_ref::<SetKeyValueBox>()
            .and_then(|kv| match kv {
                SetKeyValueBox::Nft(set) => Some(set.object.clone()),
                _ => None,
            })
            .or_else(|| {
                instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::SetKeyValue<iroha_data_model::nft::Nft>>()
                    .map(|set| set.object.clone())
            })
            .or_else(|| {
                instruction
                    .as_any()
                    .downcast_ref::<RemoveKeyValueBox>()
                    .and_then(|rm| match rm {
                        RemoveKeyValueBox::Nft(rm) => Some(rm.object.clone()),
                        _ => None,
                    })
            })
            .or_else(|| {
                instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::RemoveKeyValue<iroha_data_model::nft::Nft>>()
                    .map(|rm| rm.object.clone())
            })
        {
            if !is_initial_genesis_context(state_transaction) {
                let domain_owner = state_transaction
                    .world
                    .domain(nft_id.domain())
                    .map(|domain| domain.owned_by().clone())
                    .map_err(|err| {
                        ValidationFail::InstructionFailed(InstructionExecutionError::Find(err))
                    })?;
                if &domain_owner != authority
                    && !has_modify_nft_metadata_permission(state_transaction, authority, &nft_id)?
                {
                    return Err(ValidationFail::NotPermitted(
                        "Can't modify NFT from domain owned by another account".to_owned(),
                    ));
                }
            }
        }
        if let Some(transfer_domain) = extract_transfer_domain(instruction)
            && !can_transfer_domain(
                &state_transaction.world,
                authority,
                &transfer_domain,
                state_transaction.block_unix_timestamp_ms(),
            )
            .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?
        {
            return Err(ValidationFail::NotPermitted(
                "Can't transfer domain of another account".to_owned(),
            ));
        }
        if let Some(transfer_asset_definition) = extract_transfer_asset_definition(instruction)
            && !can_transfer_asset_definition(
                &state_transaction.world,
                authority,
                &transfer_asset_definition,
            )?
        {
            return Err(ValidationFail::NotPermitted(
                "Can't transfer asset definition of another account".to_owned(),
            ));
        }
        if let Some(transfer_nft) = extract_transfer_nft(instruction)
            && !can_transfer_nft(&state_transaction.world, authority, &transfer_nft)?
        {
            return Err(ValidationFail::NotPermitted(
                "Can't transfer NFT of another account".to_owned(),
            ));
        }
        if !is_genesis
            && let Some(transfer_asset) = extract_transfer_asset(instruction)
            && !can_transfer_asset(
                &state_transaction.world,
                authority,
                contract_runtime_context,
                &transfer_asset,
            )?
        {
            return Err(ValidationFail::NotPermitted(
                "Can't transfer asset: source asset owner must sign the transaction".to_owned(),
            ));
        }
        let instruction_id = instruction.id();
        attachment_direct::apply_authorized_instruction(
            instruction,
            authority,
            state_transaction,
            attachment_invocation,
        )
        .map_err(|err| {
            if matches!(profile, InstructionExecutionProfile::Runtime) {
                iroha_logger::debug!(
                    ?err,
                    %instruction_id,
                    authority = %authority,
                    "initial executor rejected instruction during application"
                );
            }
            ValidationFail::from(err)
        })
    }
    /// Validate [`QueryRequest`].
    ///
    /// # Errors
    ///
    /// - Failed to prepare the IVM runtime;
    /// - Failed to execute the entrypoint of the IVM bytecode;
    /// - Executor denied the operation.
    pub(crate) fn validate_query<S: StateReadOnly>(
        &self,
        state_ro: &S,
        authority: &AccountId,
        query: &QueryRequest,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        let latest_block = state_ro
            .latest_block()
            .map_err(|error| {
                error.map_rejection(|error| {
                    ValidationFail::QueryFailed(
                        iroha_data_model::query::error::QueryExecutionFail::CanonicalHistory(error),
                    )
                })
            })?
            .map(|block| block.header());
        self.validate_query_with_world_parts(
            state_ro.world(),
            latest_block.as_ref(),
            authority,
            query,
            &state_ro.execution_budget(),
        )
    }
    /// Validate [`QueryRequest`] using world-state and latest committed block header.
    ///
    /// This variant avoids requiring a full [`StateReadOnly`] snapshot in callers that
    /// already have a world view and can cheaply resolve the latest block header.
    ///
    /// # Errors
    ///
    /// - Failed to prepare the IVM runtime;
    /// - Failed to execute the entrypoint of the IVM bytecode;
    /// - Executor denied the operation.
    pub(crate) fn validate_query_with_world_parts(
        &self,
        world_ro: &impl WorldReadOnly,
        latest_block: Option<&BlockHeader>,
        authority: &AccountId,
        query: &QueryRequest,
        budget: &iroha_allocation::AllocationBudget,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        trace!("Running query validation");
        // This native boundary is mandatory for Initial and user-provided executors alike.
        // A custom executor may further restrict a query, but can never widen these grants.
        validate_builtin_native_query_permission(
            world_ro,
            authority,
            query,
            latest_block.map_or(0, |header| {
                u64::try_from(header.creation_time().as_millis()).unwrap_or(u64::MAX)
            }),
        )?;
        let query_box = match query {
            QueryRequest::Singular(singular) => AnyQueryBox::Singular(singular.clone()),
            QueryRequest::Start(iterable) => AnyQueryBox::Iterable(iterable.clone()),
            QueryRequest::Continue(_) => {
                // The iterable query was validated when it started. Execution still
                // binds the cursor to this request's authority in LiveQueryStore
                // before advancing any stored state.
                return Ok(());
            }
        };
        // Alias reads carry privacy and routing authority independent of the pluggable executor.
        // Enforce the exact built-in dataspace/domain grants first so neither the permissive
        // initial executor nor an incomplete user executor visitor can bypass them.
        validate_builtin_account_alias_query_permission(world_ro, latest_block, authority, query)?;
        validate_builtin_subsystem_query_permission(world_ro, latest_block, authority, query)?;
        match self {
            Self::Initial => Ok(()),
            Self::UserProvided(loaded_executor) => {
                let curr_block = latest_block.copied().unwrap_or_else(|| {
                    BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, 0, 0)
                });
                let context = ExecutorContext {
                    authority: authority.clone(),
                    curr_block,
                };
                let payload = ValidatePayload {
                    context,
                    target: query_box,
                };
                let query_label = match query {
                    QueryRequest::Singular(_) => "query::singular",
                    QueryRequest::Start(_) => "query::start",
                    QueryRequest::Continue(_) => unreachable!("continue queries return early"),
                };
                let executor_parameters = world_ro.parameters().executor();
                let gas_limit = executor_parameters.fuel().get();
                let heap_limit = executor_parameters.memory().get();
                let report = run_executor_validation(
                    loaded_executor,
                    &payload,
                    query_label,
                    gas_limit,
                    heap_limit,
                    budget,
                )?;
                match report.verdict {
                    Ok(()) => Ok(()),
                    Err(err) => {
                        iroha_logger::debug!(
                            ?err,
                            authority = %authority,
                            query = %query_label,
                            "executor validation rejected query"
                        );
                        Err(err.into())
                    }
                }
            }
        }
    }
    /// Migrate executor to a new user-provided one.
    ///
    /// Execute `migrate()` entrypoint of the `raw_executor` and set `self` to
    /// [`UserProvided`](Executor::UserProvided) with `raw_executor`.
    ///
    /// # Errors
    ///
    /// - The caller is outside initial genesis and does not hold `CanUpgradeExecutor`;
    /// - Failed to load `raw_executor`;
    /// - Failed to prepare the IVM runtime;
    /// - Failed to execute the entrypoint of the IVM bytecode.
    pub fn migrate(
        &mut self,
        raw_executor: data_model_executor::Executor,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> Result<(), ExecutorMigrationError> {
        trace!("Running executor migration");
        if state_transaction.is_execution_deferred() {
            return Err(ExecutorMigrationError::LocalRefusalRetained);
        }
        let can_upgrade: Permission = executor_permission::executor::CanUpgradeExecutor.into();
        if !is_initial_genesis_context(state_transaction)
            && !authority_has_permission(&state_transaction.world, authority, &can_upgrade)
                .map_err(|_| ExecutorMigrationError::Rejected(VMError::PermissionDenied))?
        {
            return Err(ExecutorMigrationError::Rejected(VMError::PermissionDenied));
        }
        // Load new executor bytecode
        let loaded_executor = LoadedExecutor::load(raw_executor).map_err(|error| {
            match crate::execution_attempt::ExecutionDeferred::try_from_vm_error(error) {
                Ok(original) => {
                    state_transaction.defer_execution(original);
                    ExecutorMigrationError::LocalRefusalRetained
                }
                Err(error) => ExecutorMigrationError::Rejected(error),
            }
        })?;
        let curr_block = state_transaction._curr_block;
        let context = ExecutorContext {
            authority: authority.clone(),
            curr_block,
        };
        let executor_parameters = state_transaction.world.parameters.get().executor();
        let gas_limit = executor_parameters.fuel().get();
        let heap_limit = executor_parameters.memory().get();
        let maybe_data_model = run_executor_migration(
            &loaded_executor,
            &context,
            gas_limit,
            heap_limit,
            &state_transaction.execution_budget(),
        )
        .map_err(|error| match error {
            crate::execution_attempt::ExecutionAttemptError::Rejected(error) => {
                ExecutorMigrationError::Rejected(map_migration_fail_to_vm_error(error))
            }
            crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                state_transaction.defer_execution(reason);
                ExecutorMigrationError::LocalRefusalRetained
            }
        })?;
        if let Some(data_model) = maybe_data_model {
            debug!("executor migrate entrypoint supplied a new data model");
            state_transaction
                .world
                .apply_executor_data_model(data_model)
                .map_err(|error| match error {
                    crate::execution_attempt::ExecutionAttemptError::Deferred(original) => {
                        state_transaction.defer_execution(original);
                        ExecutorMigrationError::LocalRefusalRetained
                    }
                    crate::execution_attempt::ExecutionAttemptError::Rejected(original) => {
                        debug!(error = %original, "executor parameter reconciliation rejected");
                        ExecutorMigrationError::Rejected(VMError::DecodeError)
                    }
                })?;
        }
        *self = Self::UserProvided(loaded_executor);
        Ok(())
    }
}
struct ExecutorValidationReport {
    verdict: Result<(), ValidationFail>,
    gas_used: u64,
}
fn initial_executor_permission_names() -> BTreeSet<String> {
    INITIAL_EXECUTOR_PERMISSION_NAMES
        .iter()
        .map(|permission| (*permission).to_owned())
        .collect()
}
pub(crate) fn initial_executor_data_model_fallback() -> ExecutorDataModel {
    ExecutorDataModel::new(
        iroha_data_model::parameter::CustomParameters::new(),
        BTreeSet::new(),
        initial_executor_permission_names(),
        Json::new(()),
    )
}
fn run_executor_validation<T>(
    executor: &LoadedExecutor,
    payload: &ValidatePayload<T>,
    verdict_context: &str,
    gas_limit: u64,
    heap_limit: u64,
    budget: &iroha_allocation::AllocationBudget,
) -> Result<ExecutorValidationReport, crate::execution_attempt::ExecutionAttemptError<ValidationFail>>
where
    ValidatePayload<T>: Encode,
{
    let mut ivm = executor
        .checkout_runtime_for_gas_limit(gas_limit, heap_limit, budget)
        .map_err(|error| {
            crate::execution_attempt::vm_attempt_error(error, |error| {
                ValidationFail::InternalError(error.to_string())
            })
        })?;
    ivm.set_host(ivm::host::DefaultHost::default());
    let bytes = encode_executor_input(payload)?;
    let ptr = Memory::HEAP_START;
    ivm.store_bytes(ptr, &bytes).map_err(|error| {
        crate::execution_attempt::vm_attempt_error(error, |error| {
            ValidationFail::InternalError(error.to_string())
        })
    })?;
    ivm.set_register(10, ptr);
    ivm.set_gas_limit(gas_limit);
    let run_result = ivm.run();
    let gas_used = gas_limit.saturating_sub(ivm.remaining_gas());
    if let Err(err) = run_result {
        if matches!(err, VMError::ExceededMaxCycles | VMError::OutOfGas) {
            return Ok(ExecutorValidationReport {
                verdict: Err(ValidationFail::TooComplex),
                gas_used,
            });
        }
        return Err(crate::execution_attempt::vm_attempt_error(err, |error| {
            ValidationFail::InternalError(error.to_string())
        }));
    }
    let ret_ptr = ivm.register(10);
    let mut slice = executor_output_payload(&ivm, ret_ptr, "validation verdict")?;
    let verdict: Result<(), ValidationFail> = Decode::decode(&mut slice).map_err(|err| {
        ValidationFail::InternalError(format!(
            "executor returned undecodable verdict: {verdict_context}: {err}"
        ))
    })?;
    if !slice.is_empty() {
        return Err(ValidationFail::InternalError(format!(
            "executor returned a verdict with trailing bytes: {verdict_context}"
        ))
        .into());
    }
    Ok(ExecutorValidationReport {
        verdict: verdict.map_err(validate_executor_rejection_origin),
        gas_used,
    })
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::executor::MigrationResultPayload")]
#[derive(Debug, Decode, Encode)]
enum MigrationResultPayload {
    Ok(ExecutorDataModel),
    Err(ValidationFail),
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::executor::MigrationUnitPayload")]
#[derive(Debug, Decode, Encode)]
enum MigrationUnitPayload {
    Ok(()),
    Err(ValidationFail),
}
fn run_executor_migration(
    executor: &LoadedExecutor,
    context: &ExecutorContext,
    gas_limit: u64,
    heap_limit: u64,
    budget: &iroha_allocation::AllocationBudget,
) -> Result<
    Option<ExecutorDataModel>,
    crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
> {
    let mut ivm = executor
        .checkout_runtime_for_gas_limit(gas_limit, heap_limit, budget)
        .map_err(|error| {
            crate::execution_attempt::vm_attempt_error(error, |error| {
                ValidationFail::InternalError(error.to_string())
            })
        })?;
    ivm.set_host(ivm::host::DefaultHost::default());
    let bytes = encode_executor_input(context)?;
    let ptr = Memory::HEAP_START;
    ivm.store_bytes(ptr, &bytes).map_err(|error| {
        crate::execution_attempt::vm_attempt_error(error, |error| {
            ValidationFail::InternalError(error.to_string())
        })
    })?;
    ivm.set_register(10, ptr);
    ivm.set_gas_limit(gas_limit);
    ivm.run().map_err(|error| {
        crate::execution_attempt::vm_attempt_error(error, |error| {
            ValidationFail::InternalError(error.to_string())
        })
    })?;
    let ret_ptr = ivm.register(10);
    let payload = executor_output_payload(&ivm, ret_ptr, "migration result")?;
    let mut slice = payload;
    if let Ok(verdict) = MigrationResultPayload::decode(&mut slice)
        && slice.is_empty()
    {
        return match verdict {
            MigrationResultPayload::Ok(model) => Ok(Some(model)),
            MigrationResultPayload::Err(fail) => {
                Err(validate_executor_rejection_origin(fail).into())
            }
        };
    }
    let mut slice_unit = payload;
    if let Ok(verdict) = MigrationUnitPayload::decode(&mut slice_unit)
        && slice_unit.is_empty()
    {
        return match verdict {
            MigrationUnitPayload::Ok(()) => Ok(None),
            MigrationUnitPayload::Err(fail) => Err(validate_executor_rejection_origin(fail).into()),
        };
    }
    Err(ValidationFail::InternalError(
        "executor migrate entrypoint returned an undecodable or non-canonical result".to_owned(),
    )
    .into())
}
/// Only the native execution boundary may certify a runtime fault origin.
fn validate_executor_rejection_origin(fail: ValidationFail) -> ValidationFail {
    if matches!(fail, ValidationFail::IvmFault(_)) {
        ValidationFail::InternalError(
            "executor verdict cannot supply a runtime fault origin".to_owned(),
        )
    } else {
        fail
    }
}
fn map_migration_fail_to_vm_error(fail: ValidationFail) -> VMError {
    match fail {
        ValidationFail::NotPermitted(reason) => {
            debug!(
                reason = %reason,
                "executor migrate entrypoint rejected migration"
            );
            VMError::PermissionDenied
        }
        ValidationFail::TooComplex => VMError::ExceededMaxCycles,
        ValidationFail::IvmAdmission(info) => {
            debug!(
                info = ?info,
                "executor migrate entrypoint failed admission checks"
            );
            VMError::DecodeError
        }
        ValidationFail::InstructionFailed(err) => {
            debug!(
                err = ?err,
                "executor migrate entrypoint instruction failure"
            );
            VMError::DecodeError
        }
        ValidationFail::IvmFault(_) => VMError::DecodeError,
        ValidationFail::SettlementReplay(_) => VMError::PermissionDenied,
        ValidationFail::ContractRejected(rejection) => {
            debug!(
                ?rejection,
                "executor migrate entrypoint returned a declared contract rejection"
            );
            VMError::PermissionDenied
        }
        ValidationFail::QueryFailed(err) => {
            debug!(
                err = ?err,
                "executor migrate entrypoint query failure"
            );
            VMError::DecodeError
        }
        ValidationFail::InternalError(message) => {
            debug!(
                message = %message,
                "executor migrate entrypoint reported internal error"
            );
            VMError::DecodeError
        }
        ValidationFail::AxtReject(ctx) => {
            debug!(?ctx, "executor migrate entrypoint rejected AXT payload");
            VMError::PermissionDenied
        }
    }
}
fn dispatch_instruction_with_ivm(
    executor: &LoadedExecutor,
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &InstructionBox,
    attachment_invocation: Option<&mut attachment_custody::Invocation<'_, '_>>,
) -> Result<(), ValidationFail> {
    let latest_block = state_transaction
        .latest_block()
        .map_err(|error| match error {
            crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                state_transaction.defer_execution(reason)
            }
            crate::execution_attempt::ExecutionAttemptError::Rejected(error) => {
                ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::CanonicalHistory(error),
                )
            }
        })?;
    let curr_block = latest_block.map_or_else(
        || BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, 0, 0),
        |b| b.header(),
    );
    let context = ExecutorContext {
        authority: authority.clone(),
        curr_block,
    };
    let payload = ValidatePayload {
        context,
        target: norito::core::PayloadRef(instruction),
    };
    let instruction_id = instruction.id();
    let gas_limit = state_transaction.executor_fuel_remaining;
    let heap_limit = state_transaction
        .world
        .parameters
        .get()
        .executor()
        .memory()
        .get();
    let report = run_executor_validation(
        executor,
        &payload,
        instruction_id,
        gas_limit,
        heap_limit,
        &state_transaction.execution_budget(),
    )
    .map_err(|error| state_transaction.attempt_error_to_validation_fail(error))?;
    state_transaction.executor_fuel_remaining = state_transaction
        .executor_fuel_remaining
        .saturating_sub(report.gas_used);
    match report.verdict {
        Ok(()) => {
            if execute_multisig_custom_instruction_if_present(
                state_transaction,
                authority,
                instruction,
            )? {
                return Ok(());
            }
            let applied = if attachment_invocation.is_some() {
                attachment_direct::apply_authorized_instruction(
                    instruction,
                    authority,
                    state_transaction,
                    attachment_invocation,
                )
            } else {
                crate::smartcontracts::isi::execute_borrowed_instruction(
                    instruction,
                    authority,
                    state_transaction,
                )
            };
            applied.map_err(|err| {
                iroha_logger::debug!(
                    ?err,
                    %instruction_id,
                    authority = %authority,
                    "state application of executor-approved instruction failed"
                );
                ValidationFail::from(err)
            })
        }
        Err(e) => {
            iroha_logger::debug!(
                ?e,
                %instruction_id,
                authority = %authority,
                "executor validation rejected instruction"
            );
            Err(e)
        }
    }
}
fn execute_multisig_custom_instruction_if_present(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &InstructionBox,
) -> Result<bool, ValidationFail> {
    if instruction
        .as_any()
        .downcast_ref::<CustomInstruction>()
        .is_none()
    {
        return Ok(false);
    }
    let Ok(multisig) = MultisigInstructionBox::try_from(instruction) else {
        return Ok(false);
    };
    crate::smartcontracts::isi::multisig::execute_multisig_instruction(
        state_transaction,
        authority,
        multisig,
    )?;
    Ok(true)
}
fn extract_register_role(instruction: &InstructionBox) -> Option<Register<Role>> {
    let instr_any = instruction.as_any();
    if let Some(reg) = instr_any.downcast_ref::<Register<Role>>() {
        return Some(reg.clone());
    }
    if let Some(reg_box) = instr_any.downcast_ref::<RegisterBox>() {
        return match reg_box {
            RegisterBox::Role(reg) => Some(reg.clone()),
            _ => None,
        };
    }
    None
}
fn extract_unregister_role(instruction: &InstructionBox) -> Option<Unregister<Role>> {
    let instr_any = instruction.as_any();
    if let Some(unregister) = instr_any.downcast_ref::<Unregister<Role>>() {
        return Some(unregister.clone());
    }
    if let Some(unregister_box) = instr_any.downcast_ref::<UnregisterBox>() {
        return match unregister_box {
            UnregisterBox::Role(unregister) => Some(unregister.clone()),
            _ => None,
        };
    }
    None
}
fn extract_account_metadata_target(instruction: &InstructionBox) -> Option<AccountId> {
    instruction
        .as_any()
        .downcast_ref::<SetKeyValueBox>()
        .and_then(|set| match set {
            SetKeyValueBox::Account(set) => Some(set.object.clone()),
            _ => None,
        })
        .or_else(|| {
            instruction
                .as_any()
                .downcast_ref::<iroha_data_model::isi::SetKeyValue<iroha_data_model::account::Account>>()
                .map(|set| set.object.clone())
        })
        .or_else(|| {
            instruction
                .as_any()
                .downcast_ref::<RemoveKeyValueBox>()
                .and_then(|rm| match rm {
                    RemoveKeyValueBox::Account(rm) => Some(rm.object.clone()),
                    _ => None,
                })
        })
        .or_else(|| {
            instruction
                .as_any()
                .downcast_ref::<iroha_data_model::isi::RemoveKeyValue<iroha_data_model::account::Account>>()
                .map(|rm| rm.object.clone())
        })
}
fn extract_asset_definition_metadata_target(
    instruction: &InstructionBox,
) -> Option<AssetDefinitionId> {
    instruction
        .as_any()
        .downcast_ref::<SetKeyValueBox>()
        .and_then(|set| match set {
            SetKeyValueBox::AssetDefinition(set) => Some(set.object.clone()),
            _ => None,
        })
        .or_else(|| {
            instruction
                .as_any()
                .downcast_ref::<
                    iroha_data_model::isi::SetKeyValue<
                        iroha_data_model::asset::AssetDefinition,
                    >,
                >()
                .map(|set| set.object.clone())
        })
        .or_else(|| {
            instruction
                .as_any()
                .downcast_ref::<RemoveKeyValueBox>()
                .and_then(|remove| match remove {
                    RemoveKeyValueBox::AssetDefinition(remove) => Some(remove.object.clone()),
                    _ => None,
                })
        })
        .or_else(|| {
            instruction
                .as_any()
                .downcast_ref::<
                    iroha_data_model::isi::RemoveKeyValue<
                        iroha_data_model::asset::AssetDefinition,
                    >,
                >()
                .map(|remove| remove.object.clone())
        })
}
fn extract_mint_asset(instruction: &InstructionBox) -> Option<Mint<Quantity, Asset>> {
    let any = instruction.as_any();
    if let Some(mint) = any.downcast_ref::<Mint<Quantity, Asset>>() {
        return Some(mint.clone());
    }
    if let Some(mint) = any.downcast_ref::<MintBox>() {
        return match mint {
            MintBox::Asset(mint) => Some(mint.clone()),
            MintBox::TriggerRepetitions(_) => None,
        };
    }
    if !instruction_has_concrete_type::<Mint<Quantity, Asset>>(instruction) {
        return None;
    }
    let bytes = instruction.dyn_encode();
    iroha_panic_hook::catch_unwind_suppressed(|| {
        Mint::<Quantity, Asset>::decode(&mut bytes.as_slice()).ok()
    })
    .ok()
    .flatten()
}
fn extract_transfer_asset(
    instruction: &InstructionBox,
) -> Option<Transfer<Asset, Quantity, Account>> {
    let instr_any = instruction.as_any();
    if let Some(transfer) = instr_any.downcast_ref::<Transfer<Asset, Quantity, Account>>() {
        return Some(transfer.clone());
    }
    if let Some(transfer_box) = instr_any.downcast_ref::<TransferBox>() {
        return match transfer_box {
            TransferBox::Asset(transfer) => Some(transfer.clone()),
            _ => None,
        };
    }
    if !instruction_has_concrete_type::<Transfer<Asset, Quantity, Account>>(instruction) {
        return None;
    }
    let bytes = instruction.dyn_encode();
    iroha_panic_hook::catch_unwind_suppressed(|| {
        let mut slice = &bytes[..];
        Transfer::<Asset, Quantity, Account>::decode(&mut slice).ok()
    })
    .ok()
    .flatten()
}
fn extract_transfer_domain(
    instruction: &InstructionBox,
) -> Option<Transfer<Account, DomainId, Account>> {
    let instr_any = instruction.as_any();
    if let Some(transfer) = instr_any.downcast_ref::<Transfer<Account, DomainId, Account>>() {
        return Some(transfer.clone());
    }
    if let Some(transfer_box) = instr_any.downcast_ref::<TransferBox>() {
        return match transfer_box {
            TransferBox::Domain(transfer) => Some(transfer.clone()),
            _ => None,
        };
    }
    if !instruction_has_concrete_type::<Transfer<Account, DomainId, Account>>(instruction) {
        return None;
    }
    let bytes = instruction.dyn_encode();
    iroha_panic_hook::catch_unwind_suppressed(|| {
        let mut slice = &bytes[..];
        Transfer::<Account, DomainId, Account>::decode(&mut slice).ok()
    })
    .ok()
    .flatten()
}
fn extract_transfer_asset_definition(
    instruction: &InstructionBox,
) -> Option<Transfer<Account, AssetDefinitionId, Account>> {
    let instr_any = instruction.as_any();
    if let Some(transfer) =
        instr_any.downcast_ref::<Transfer<Account, AssetDefinitionId, Account>>()
    {
        return Some(transfer.clone());
    }
    if let Some(transfer_box) = instr_any.downcast_ref::<TransferBox>() {
        return match transfer_box {
            TransferBox::AssetDefinition(transfer) => Some(transfer.clone()),
            _ => None,
        };
    }
    if !instruction_has_concrete_type::<Transfer<Account, AssetDefinitionId, Account>>(instruction)
    {
        return None;
    }
    let bytes = instruction.dyn_encode();
    iroha_panic_hook::catch_unwind_suppressed(|| {
        let mut slice = &bytes[..];
        Transfer::<Account, AssetDefinitionId, Account>::decode(&mut slice).ok()
    })
    .ok()
    .flatten()
}
fn extract_transfer_nft(
    instruction: &InstructionBox,
) -> Option<Transfer<Account, iroha_data_model::NftId, Account>> {
    let instr_any = instruction.as_any();
    if let Some(transfer) =
        instr_any.downcast_ref::<Transfer<Account, iroha_data_model::NftId, Account>>()
    {
        return Some(transfer.clone());
    }
    if let Some(transfer_box) = instr_any.downcast_ref::<TransferBox>() {
        return match transfer_box {
            TransferBox::Nft(transfer) => Some(transfer.clone()),
            _ => None,
        };
    }
    if !instruction_has_concrete_type::<Transfer<Account, iroha_data_model::NftId, Account>>(
        instruction,
    ) {
        return None;
    }
    let bytes = instruction.dyn_encode();
    iroha_panic_hook::catch_unwind_suppressed(|| {
        let mut slice = &bytes[..];
        Transfer::<Account, iroha_data_model::NftId, Account>::decode(&mut slice).ok()
    })
    .ok()
    .flatten()
}
fn authority_has_permission(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    target: &Permission,
) -> Result<bool, ValidationFail> {
    if authority_has_direct_permission(world, authority, target)? {
        return Ok(true);
    }
    for role_id in world.account_roles_iter(authority) {
        if let Some(role) = world.roles().get(role_id)
            && role.permissions.contains(target)
        {
            return Ok(true);
        }
    }
    Ok(false)
}
/// Whether `authority` holds `target` directly (not through a role).
///
/// The account must exist. Membership is one ordered-set lookup rather than a
/// scan of every permission granted to the account.
fn authority_has_direct_permission(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    target: &Permission,
) -> Result<bool, ValidationFail> {
    world
        .account(authority)
        .map_err(|err| ValidationFail::InstructionFailed(InstructionExecutionError::Find(err)))?;
    Ok(world.account_contains_inherent_permission(authority, target))
}
fn authority_has_role(world: &impl WorldReadOnly, authority: &AccountId, role_id: &RoleId) -> bool {
    world
        .account_roles_iter(authority)
        .any(|assigned| assigned == role_id)
}
const INITIAL_GENESIS_ONLY_PERMISSION_NAMES: &[&str] = &[
    "CanManagePeers",
    "CanRegisterDomain",
    "CanManageRoles",
    "CanUpgradeExecutor",
    "CanGrantSmartContractCodeManagement",
    "CanReadAllLedgerData",
    "CanReadRestrictedDataspace",
    "CanManageFxCorridors",
    "CanProposeSccpRouteGovernance",
];
include!("executor_initial_permission_authority.rs");
fn is_builtin_initial_permission_name(permission_name: &str) -> bool {
    INITIAL_EXECUTOR_PERMISSION_NAMES.contains(&permission_name)
}
/// Return the `Register<AssetDefinition>` carried by `instruction`, whether it is typed,
/// wrapped in a [`RegisterBox`], or an encoded instruction of that concrete type.
pub(crate) fn extract_register_asset_definition(
    instruction: &InstructionBox,
) -> Option<Register<AssetDefinition>> {
    let instr_any = instruction.as_any();
    if let Some(reg) = instr_any.downcast_ref::<Register<AssetDefinition>>() {
        return Some(reg.clone());
    }
    if let Some(reg_box) = instr_any.downcast_ref::<RegisterBox>() {
        return match reg_box {
            RegisterBox::AssetDefinition(reg) => Some(reg.clone()),
            _ => None,
        };
    }
    if !instruction_has_concrete_type::<Register<AssetDefinition>>(instruction) {
        return None;
    }
    let bytes = instruction.dyn_encode();
    iroha_panic_hook::catch_unwind_suppressed(|| {
        let mut slice = &bytes[..];
        Register::<AssetDefinition>::decode(&mut slice).ok()
    })
    .ok()
    .flatten()
}
/// Extract only the distinct direct-dataspace registration wire instruction.
pub(crate) fn extract_register_dataspace_asset_definition(
    instruction: &InstructionBox,
) -> Option<iroha_data_model::isi::RegisterDataspaceAssetDefinition> {
    use iroha_data_model::isi::RegisterDataspaceAssetDefinition;
    if let Some(reg) = instruction
        .as_any()
        .downcast_ref::<RegisterDataspaceAssetDefinition>()
    {
        return Some(reg.clone());
    }
    if !instruction_has_concrete_type::<RegisterDataspaceAssetDefinition>(instruction) {
        return None;
    }
    let bytes = instruction.dyn_encode();
    iroha_panic_hook::catch_unwind_suppressed(|| {
        RegisterDataspaceAssetDefinition::decode(&mut &bytes[..]).ok()
    })
    .ok()
    .flatten()
}
/// Authorize registration against the signed owning domain and its current owner.
/// Domainless definitions are restricted to the authenticated genesis context;
/// optional aliases never supply registration authority.
pub(crate) fn ensure_asset_definition_registration_allowed(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    reg_asset_definition: &Register<AssetDefinition>,
) -> Result<(), ValidationFail> {
    let is_genesis_context = is_initial_genesis_context(state_transaction)
        && state_transaction
            .world
            .domain(&iroha_genesis::GENESIS_DOMAIN_ID)
            .is_ok();
    if is_genesis_context {
        return Ok(());
    }
    // Ownership is an explicit signed field. Optional display aliases cannot grant
    // registration authority or replace the canonical owning-domain binding.
    let Some(domain_id) = reg_asset_definition.object().owning_domain.as_ref() else {
        return Err(ValidationFail::NotPermitted(
            "domainless asset definitions may only be registered in genesis".to_owned(),
        ));
    };
    let domain_owner = state_transaction
        .world
        .domain(domain_id)
        .map(|domain| domain.owned_by().clone())
        .map_err(|err| ValidationFail::InstructionFailed(InstructionExecutionError::Find(err)))?;
    if &domain_owner == authority {
        return Ok(());
    }
    Err(ValidationFail::NotPermitted(
        "Can't register asset definition".to_owned(),
    ))
}
#[cfg(test)]
#[path = "executor_runtime_memory_tests.rs"]
mod runtime_memory_tests;

#[cfg(test)]
#[path = "executor_public_pin_admission_tests.rs"]
mod public_pin_admission_tests;

#[cfg(test)]
#[path = "executor_opaque_monetary_tests.rs"]
mod opaque_monetary_tests;

#[cfg(test)]
#[path = "executor_opaque_sccp_record_tests.rs"]
mod opaque_sccp_record_tests;

#[cfg(test)]
fn executor_test_budget() -> iroha_allocation::AllocationBudget {
    thread_local! {
        static BUDGET: iroha_allocation::AllocationBudget =
            iroha_allocation::AllocationBudget::new(512 * 1024 * 1024);
    }
    BUDGET.with(Clone::clone)
}

/// [`Executor`] with cached [`IVM`] for execution.
#[derive(Debug, Clone)]
#[debug("LoadedExecutor {{ runtime: <IVM> }}")]
pub struct LoadedExecutor {
    runtime_pool: Arc<Mutex<ExecutorRuntimePool>>,
    pool_memory: Arc<ivm::cache_memory::MemoryReservation>,
    _eviction: ivm::cache_memory::CacheEvictionRegistration,
    /// Sharing immutable bytecode makes executor clones cheap.
    /// See [`crate::tx::TransactionExecutor::validate_with_runtime_executor`].
    raw_executor: ivm::cache_memory::SharedValue<data_model_executor::Executor>,
}
// Stack sizing and the governed heap ceiling define distinct VM memory
// authorities. Keep a small bounded LRU so adversarial governance/gas
// variation cannot retain an unbounded number of complete memory images and
// Merkle baselines.
const EXECUTOR_RUNTIME_VARIANT_CAPACITY: usize = 4;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ExecutorRuntimeKey {
    stack_limit: u64,
    heap_limit: u64,
}
impl ExecutorRuntimeKey {
    fn for_limits(gas_limit: u64, heap_limit: u64) -> Self {
        // The exact gas limit is replenished on every checkout. Keying by it
        // would create distinct variants with identical memory layouts.
        Self {
            stack_limit: stack_limit_for_gas(gas_limit),
            heap_limit,
        }
    }
}
struct ExecutorRuntimeVariant {
    identity: ivm::cache_memory::SharedValue<()>,
    available: Option<(RuntimeTemplate, IVM)>,
}
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ExecutorRuntimePoolStats {
    hits: u64,
    misses: u64,
    program_loads: u64,
    template_builds: u64,
    dirty_resets: u64,
    evictions: u64,
}
#[derive(Clone, Copy)]
enum ExecutorRuntimePoolEvent {
    Hit,
    Miss,
    ProgramLoad,
    TemplateBuild,
    DirtyReset,
    Eviction,
}
struct ExecutorRuntimePool {
    // Bound once to the first State owner. Foreign State clones execute uncached.
    execution_budget: Option<iroha_allocation::AllocationBudget>,
    variants: BTreeMap<ExecutorRuntimeKey, ExecutorRuntimeVariant>,
    order: VecDeque<ExecutorRuntimeKey>,
    capacity: usize,
    #[cfg(test)]
    stats: ExecutorRuntimePoolStats,
    // Release aggregate charges after every owned allocation above is destroyed.
    index_memory: ivm::cache_memory::MemoryReservation,
}
impl ExecutorRuntimePool {
    fn new(capacity: usize) -> Self {
        Self {
            execution_budget: None,
            index_memory: ivm::cache_memory::MemoryReservation::active(0),
            variants: BTreeMap::new(),
            order: VecDeque::new(),
            capacity,
            #[cfg(test)]
            stats: ExecutorRuntimePoolStats::default(),
        }
    }
    fn clear_storage(&mut self) {
        #[cfg(test)]
        {
            self.stats.evictions = self
                .stats
                .evictions
                .saturating_add(self.variants.len() as u64);
        }
        self.variants = BTreeMap::new();
        self.order = VecDeque::new();
        self.index_memory.set_known_bytes(0);
    }
    fn retain_index_or_clear(&mut self) {
        let bytes = norito::core::owned_btree_allocation_bytes::<
            ExecutorRuntimeKey,
            ExecutorRuntimeVariant,
        >(self.variants.len())
        .ok()
        .and_then(|bytes| {
            bytes.checked_add(
                self.order
                    .capacity()
                    .checked_mul(core::mem::size_of::<ExecutorRuntimeKey>())?,
            )
        });
        let Some(bytes) = bytes else {
            self.clear_storage();
            return;
        };
        // BTree removal may retain spare nodes. Preserve the high-water charge
        // until clear_storage drops the original container allocation.
        self.index_memory
            .set_known_bytes(bytes.max(self.index_memory.bytes()));
        if !self.index_memory.try_retain() {
            self.clear_storage();
        }
    }
    fn record(&mut self, event: ExecutorRuntimePoolEvent) {
        #[cfg(test)]
        match event {
            ExecutorRuntimePoolEvent::Hit => {
                self.stats.hits = self.stats.hits.saturating_add(1);
            }
            ExecutorRuntimePoolEvent::Miss => {
                self.stats.misses = self.stats.misses.saturating_add(1);
            }
            ExecutorRuntimePoolEvent::ProgramLoad => {
                self.stats.program_loads = self.stats.program_loads.saturating_add(1);
            }
            ExecutorRuntimePoolEvent::TemplateBuild => {
                self.stats.template_builds = self.stats.template_builds.saturating_add(1);
            }
            ExecutorRuntimePoolEvent::DirtyReset => {
                self.stats.dirty_resets = self.stats.dirty_resets.saturating_add(1);
            }
            ExecutorRuntimePoolEvent::Eviction => {
                self.stats.evictions = self.stats.evictions.saturating_add(1);
            }
        }
        #[cfg(not(test))]
        let _ = event;
    }
    // Move at most one original idle owner out for destruction after unlocking. Keep
    // its bounded key/identity row: active borrowers and the index high-water charge
    // are independent owners and are never reclaimed by this retention retry.
    fn take_idle_runtime_for_retention(
        &mut self,
        returning_key: ExecutorRuntimeKey,
        returning_identity: &ivm::cache_memory::SharedValue<()>,
    ) -> Option<(RuntimeTemplate, IVM)> {
        let returning = self.variants.get(&returning_key)?;
        if !ivm::cache_memory::SharedValue::ptr_eq(&returning.identity, returning_identity)
            || returning.available.is_some()
        {
            return None;
        }
        let idle_key = self.order.iter().copied().find(|key| {
            *key != returning_key
                && self
                    .variants
                    .get(key)
                    .is_some_and(|variant| variant.available.is_some())
        })?;
        let idle = self.variants.get_mut(&idle_key)?.available.take()?;
        self.record(ExecutorRuntimePoolEvent::Eviction);
        Some(idle)
    }
    fn touch(&mut self, key: ExecutorRuntimeKey) {
        if let Some(position) = self.order.iter().position(|candidate| *candidate == key) {
            self.order.remove(position);
        }
        self.order.push_back(key);
    }
    fn insert_variant(&mut self, key: ExecutorRuntimeKey) -> ivm::cache_memory::SharedValue<()> {
        while self.variants.len() >= self.capacity {
            let Some(evicted) = self.order.pop_front() else {
                break;
            };
            if self.variants.remove(&evicted).is_some() {
                self.record(ExecutorRuntimePoolEvent::Eviction);
            }
        }
        let identity = ivm::cache_memory::SharedValue::new((), Some(0));
        if self.capacity == 0 || !identity.try_retain() {
            return identity;
        }
        self.variants.insert(
            key,
            ExecutorRuntimeVariant {
                identity: identity.clone(),
                available: None,
            },
        );
        self.touch(key);
        self.retain_index_or_clear();
        identity
    }
}
/// The actual pool guard determines refund custody; equal limits are not pool identity.
/// Every guard drops inside its original scope, including on unwind. State writers
/// retain the enclosing same-pool notifications through their existing refund owner.
fn with_executor_runtime_pool<R>(
    pool: &Mutex<ExecutorRuntimePool>,
    operation: impl FnOnce(&mut ExecutorRuntimePool) -> R,
) -> R {
    let mut guard = pool.lock().unwrap_or_else(|error| error.into_inner());
    let budget = guard.execution_budget.clone();
    let run = move || operation(&mut guard);
    match budget {
        Some(budget) => budget.with_deferred_refund_notifications(move |_| run()),
        None => run(),
    }
}

struct ExecutorRuntimeLease {
    pool: Arc<Mutex<ExecutorRuntimePool>>,
    _pool_memory: Arc<ivm::cache_memory::MemoryReservation>,
    key: ExecutorRuntimeKey,
    variant_identity: Option<ivm::cache_memory::SharedValue<()>>,
    execution_budget: iroha_allocation::AllocationBudget,
    baseline: RuntimeTemplate,
    vm: Option<IVM>,
}
impl Deref for ExecutorRuntimeLease {
    type Target = IVM;
    fn deref(&self) -> &Self::Target {
        self.vm
            .as_ref()
            .expect("executor runtime lease always owns a VM")
    }
}
impl DerefMut for ExecutorRuntimeLease {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.vm
            .as_mut()
            .expect("executor runtime lease always owns a VM")
    }
}
impl Drop for ExecutorRuntimeLease {
    fn drop(&mut self) {
        let Some(vm) = self.vm.take() else {
            return;
        };
        // The VM moves into the scope before any path can destroy it. The baseline
        // field's later final release also occurs outside every physical pool guard.
        let budget = self.execution_budget.clone();
        budget.with_deferred_refund_notifications(move |_| self.return_runtime(vm));
    }
}
impl ExecutorRuntimeLease {
    fn return_runtime(&self, mut vm: IVM) {
        let Some(identity) = self.variant_identity.as_ref() else {
            return;
        };
        let (can_return, mut idle_reclaims_left) = with_executor_runtime_pool(&self.pool, |pool| {
            (
                pool.variants.get(&self.key).is_some_and(|variant| {
                    ivm::cache_memory::SharedValue::ptr_eq(&variant.identity, identity)
                        && variant.available.is_none()
                }),
                pool.capacity,
            )
        });
        if !can_return || vm.reset_from_runtime_template(&self.baseline).is_err() {
            return;
        }
        while !self.baseline.try_retain_cache_allocations() || !vm.try_retain_cache_allocations() {
            if idle_reclaims_left == 0 {
                return;
            }
            idle_reclaims_left -= 1;
            let idle = with_executor_runtime_pool(&self.pool, |pool| {
                pool.take_idle_runtime_for_retention(self.key, identity)
            });
            let Some(idle) = idle else {
                return;
            };
            drop(idle);
        }
        with_executor_runtime_pool(&self.pool, move |pool| {
            let stored = pool.variants.get_mut(&self.key).is_some_and(|variant| {
                if !ivm::cache_memory::SharedValue::ptr_eq(&variant.identity, identity)
                    || variant.available.is_some()
                {
                    return false;
                }
                variant.available = Some((self.baseline.clone(), vm));
                true
            });
            if stored {
                pool.record(ExecutorRuntimePoolEvent::DirtyReset);
                pool.touch(self.key);
                pool.retain_index_or_clear();
            }
        });
    }
}
fn stack_limit_for_gas(gas_limit: u64) -> u64 {
    IvmConfig::new(gas_limit).stack_limit_for_gas()
}
impl LoadedExecutor {
    pub(crate) fn load(raw_executor: data_model_executor::Executor) -> Result<Self, VMError> {
        // Restore validates the same static image as load_program, but has no State
        // resource owner yet. Mutable runtimes are funded only at live checkout.
        struct PoolControl {
            weak: std::sync::Weak<Mutex<ExecutorRuntimePool>>,
            _memory: Arc<ivm::cache_memory::MemoryReservation>,
        }
        impl PoolControl {
            fn clear(&self) {
                if let Some(pool) = self.weak.upgrade() {
                    with_executor_runtime_pool(&pool, ExecutorRuntimePool::clear_storage);
                }
            }
        }
        IVM::validate_program(raw_executor.bytecode().as_ref())?;
        // Normalize caller spare Vec capacity, then charge the actual owned Vec.
        let bytecode = raw_executor.bytecode().as_ref().to_vec();
        let bytecode_capacity = bytecode.capacity();
        let raw_executor = ivm::cache_memory::SharedValue::new(
            data_model_executor::Executor::new(
                iroha_data_model::transaction::IvmBytecode::from_compiled(bytecode),
            ),
            Some(bytecode_capacity),
        );
        let runtime_pool = Arc::new(Mutex::new(ExecutorRuntimePool::new(
            EXECUTOR_RUNTIME_VARIANT_CAPACITY,
        )));
        let pool_memory = Arc::new(ivm::cache_memory::MemoryReservation::active(
            norito::core::owned_arc_allocation_bytes::<Mutex<ExecutorRuntimePool>>()
                .expect("executor pool fits")
                + norito::core::owned_arc_allocation_bytes::<ivm::cache_memory::MemoryReservation>(
                )
                .expect("pool accounting fits"),
        ));
        let control = PoolControl {
            weak: Arc::downgrade(&runtime_pool),
            _memory: Arc::clone(&pool_memory),
        };
        let eviction = ivm::cache_memory::register_cache_evictor(move || control.clear());
        Ok(Self {
            runtime_pool,
            pool_memory,
            _eviction: eviction,
            raw_executor,
        })
    }
    fn load_runtime(
        raw_executor: &data_model_executor::Executor,
        gas_limit: u64,
        heap_limit: u64,
        budget: &iroha_allocation::AllocationBudget,
    ) -> Result<IVM, VMError> {
        let mut vm = IVM::try_new_with_memory_budget(gas_limit, budget)?;
        vm.memory.set_heap_max_limit(heap_limit)?;
        vm.load_program(raw_executor.bytecode().as_ref())?;
        vm.set_gas_limit(gas_limit);
        Ok(vm)
    }
    fn checkout_runtime_for_gas_limit(
        &self,
        gas_limit: u64,
        heap_limit: u64,
        budget: &iroha_allocation::AllocationBudget,
    ) -> Result<ExecutorRuntimeLease, VMError> {
        let key = ExecutorRuntimeKey::for_limits(gas_limit, heap_limit);
        let (variant_identity, runtime) = with_executor_runtime_pool(&self.runtime_pool, |pool| {
            let original = pool.execution_budget.get_or_insert_with(|| budget.clone());
            if !original.same_pool(budget) {
                // Cloned Worlds can belong to different States. Do not transfer a
                // retained image's original credit, or evict it beneath foreign writers.
                return (None, None);
            }
            if let Some(variant) = pool.variants.get_mut(&key) {
                let identity = variant.identity.clone();
                let runtime = variant.available.take();
                pool.record(if runtime.is_some() {
                    ExecutorRuntimePoolEvent::Hit
                } else {
                    ExecutorRuntimePoolEvent::Miss
                });
                pool.touch(key);
                (Some(identity), runtime)
            } else {
                pool.record(ExecutorRuntimePoolEvent::Miss);
                (Some(pool.insert_variant(key)), None)
            }
        });
        // Admission never waits or holds a pool lock. Each new baseline reserves
        // its complete clone demand from this same VM budget before allocation.
        let (baseline, mut vm) = if let Some(runtime) = runtime {
            runtime
        } else {
            let mut vm = Self::load_runtime(&self.raw_executor, gas_limit, heap_limit, budget)?;
            let baseline = vm.try_runtime_template()?;
            if variant_identity.is_some() {
                with_executor_runtime_pool(&self.runtime_pool, |pool| {
                    pool.record(ExecutorRuntimePoolEvent::ProgramLoad);
                    pool.record(ExecutorRuntimePoolEvent::TemplateBuild);
                });
            }
            (baseline, vm)
        };
        vm.activate_cached_runtime();
        vm.set_gas_limit(gas_limit);
        Ok(ExecutorRuntimeLease {
            pool: Arc::clone(&self.runtime_pool),
            _pool_memory: Arc::clone(&self.pool_memory),
            key,
            variant_identity,
            execution_budget: budget.clone(),
            baseline,
            vm: Some(vm),
        })
    }
    #[cfg(test)]
    fn runtime_pool_snapshot(&self) -> (ExecutorRuntimePoolStats, usize) {
        let pool = self
            .runtime_pool
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (pool.stats, pool.variants.len())
    }
    #[cfg(test)]
    fn runtime_variant_capacity(&self) -> usize {
        self.runtime_pool
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .capacity
    }
}
/// Norito encode/decode helpers for the runtime `Executor`.
///
/// These helpers serialize the core `Executor` enum into a compact Norito payload using a local DTO
/// and provide a materialization path that loads a `LoadedExecutor` when required.
pub mod executor_norito {
    use super::*;
    /// Local DTO used for Norito encoding of `Executor`.
    #[derive(Encode, Decode, norito::NoritoSchema)]
    #[norito_schema(name = "iroha_core::executor::executor_norito::ExecutorDto")]
    enum ExecutorDto {
        Initial,
        UserProvided(iroha_data_model::executor::Executor),
    }
    /// Serialize the given `Executor` to Norito bytes.
    /// Serialize an [`Executor`] into Norito-encoded bytes.
    ///
    /// # Errors
    /// Returns an error if Norito encoding fails for the provided executor variant.
    pub fn to_bytes(executor: &Executor) -> Result<Vec<u8>, norito::core::Error> {
        let dto = match executor {
            Executor::Initial => ExecutorDto::Initial,
            Executor::UserProvided(le) => {
                // Serialize the raw executor (data_model)
                ExecutorDto::UserProvided((*le.raw_executor).clone())
            }
        };
        norito::to_bytes(&dto)
    }
    /// Encode the complete recoverable raw executor for the keyed State cell.
    /// Loaded VM instances, compiled artifacts and runtime-pool history are excluded.
    pub(crate) fn encode_state_authority(
        executor: &Executor,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), norito::core::Error> {
        // A reference delegates the existing payload without cloning raw code. The
        // derive requires SerializePayload on its field type, which plain &T lacks.
        struct BorrowedRawExecutor<'a>(&'a iroha_data_model::executor::Executor);
        impl norito::core::SerializePayload for BorrowedRawExecutor<'_> {
            fn serialize(
                &self,
                writer: &mut norito::core::Encoder<'_>,
            ) -> Result<(), norito::core::Error> {
                norito::core::SerializePayload::serialize(self.0, writer)
            }
        }
        #[derive(Encode)]
        enum BorrowedExecutorDto<'a> {
            Initial,
            UserProvided(BorrowedRawExecutor<'a>),
        }
        let borrowed = match executor {
            Executor::Initial => BorrowedExecutorDto::Initial,
            Executor::UserProvided(loaded) => {
                BorrowedExecutorDto::UserProvided(BorrowedRawExecutor(&loaded.raw_executor))
            }
        };
        let mut writer = writer;
        norito::codec::encode_adaptive_into(&borrowed, &mut writer).map(|_| ())
    }

    /// Hash the executor's borrowed semantic payload for the private World delta.
    /// Loaded VM variants, caches and pool history are not State values.
    pub(crate) fn net_state_hash(executor: &Executor) -> Result<iroha_crypto::Hash, String> {
        const DOMAIN: &[u8] = b"iroha:world-net-delta:executor:v1\0";
        match executor {
            Executor::Initial => Ok(iroha_crypto::Hash::new_from_chunks(&[DOMAIN, &[0]])),
            Executor::UserProvided(loaded) => {
                let raw = crate::state::world_projection::hash_value(&*loaded.raw_executor)?;
                Ok(iroha_crypto::Hash::new_from_chunks(&[
                    DOMAIN,
                    &[1],
                    raw.as_ref(),
                ]))
            }
        }
    }

    /// Deserialize Norito bytes into a materialized `Executor`.
    ///
    /// For `UserProvided` DTO, loads the IVM program to construct a `LoadedExecutor`.
    /// Deserialize an [`Executor`] from Norito-encoded bytes.
    ///
    /// # Errors
    /// Returns an error if the byte slice does not represent a valid executor value.
    pub fn from_bytes(bytes: &[u8]) -> Result<Executor, String> {
        let decoded =
            iroha_panic_hook::catch_unwind_suppressed(|| norito::decode_from_bytes(bytes))
                .map_err(|_| "executor decode failed: panic during Norito decode".to_owned())?;
        let dto: ExecutorDto = decoded.map_err(|e| format!("executor decode failed: {e}"))?;
        match dto {
            ExecutorDto::Initial => Ok(Executor::Initial),
            ExecutorDto::UserProvided(raw) => LoadedExecutor::load(raw)
                .map(Executor::UserProvided)
                .map_err(|e| format!("executor load failed: {e}")),
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn initial_roundtrip() {
            let exec = Executor::Initial;
            let bytes = to_bytes(&exec).expect("encode");
            assert_eq!(
                <ExecutorDto as norito::NoritoSchema>::nominal_name(),
                "iroha_core::executor::executor_norito::ExecutorDto"
            );
            assert_eq!(
                bytes[6..22],
                norito::schema::identity::frame_hash::<ExecutorDto>()
            );
            assert!(matches!(
                norito::decode_canonical::<ExecutorDto>(&bytes).expect("DTO frame"),
                ExecutorDto::Initial
            ));
            let mut substituted = bytes.clone();
            substituted[6] ^= 1;
            assert!(matches!(
                norito::decode_canonical::<ExecutorDto>(&substituted),
                Err(norito::Error::SchemaMismatch)
            ));
            assert!(from_bytes(&substituted).is_err());
            assert!(from_bytes(&bytes[..bytes.len() - 1]).is_err());
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(from_bytes(&trailing).is_err());
            let dec = from_bytes(&bytes).expect("decode");
            assert_eq!(
                to_bytes(&dec).expect("materialized executor re-encodes"),
                bytes
            );
            match dec {
                Executor::Initial => {}
                _ => panic!("expected Initial variant"),
            }
        }
        #[test]
        fn net_state_hash_binds_raw_executor_and_ignores_loaded_runtime_cache() {
            let first = denying_executor_for_testing("projection one");
            let same = denying_executor_for_testing("projection one");
            let other = denying_executor_for_testing("projection two");
            let expected = net_state_hash(&first).unwrap();
            assert_eq!(expected, net_state_hash(&same).unwrap());
            assert_ne!(expected, net_state_hash(&other).unwrap());
            assert_ne!(expected, net_state_hash(&Executor::Initial).unwrap());
            let Executor::UserProvided(loaded) = &first else {
                panic!("fixture must load its actual executor");
            };
            let parameters = iroha_data_model::parameter::SmartContractParameters::default();
            let before = loaded.runtime_pool_snapshot();
            {
                let _lease = loaded
                    .checkout_runtime_for_gas_limit(
                        parameters.fuel().get(),
                        parameters.memory().get(),
                        &executor_test_budget(),
                    )
                    .unwrap();
                assert_eq!(expected, net_state_hash(&first).unwrap());
            }
            assert_ne!(
                before,
                loaded.runtime_pool_snapshot(),
                "actual runtime pool changed"
            );
            assert_eq!(expected, net_state_hash(&first).unwrap());
        }

        #[test]
        fn complete_state_executor_bytes_match_the_owned_raw_dto() {
            for executor in [
                Executor::Initial,
                denying_executor_for_testing("recoverable raw executor"),
            ] {
                let mut actual = Vec::new();
                encode_state_authority(&executor, &mut actual).unwrap();
                let owned = match &executor {
                    Executor::Initial => ExecutorDto::Initial,
                    Executor::UserProvided(loaded) => {
                        ExecutorDto::UserProvided((*loaded.raw_executor).clone())
                    }
                };
                assert_eq!(actual, norito::codec::encode_adaptive(&owned));
                let restored = norito::codec::decode_adaptive::<ExecutorDto>(&actual).unwrap();
                assert_eq!(norito::codec::encode_adaptive(&restored), actual);
                assert!(
                    norito::codec::decode_adaptive::<ExecutorDto>(&actual[..actual.len() - 1])
                        .is_err()
                );
                actual.push(0);
                assert!(norito::codec::decode_adaptive::<ExecutorDto>(&actual).is_err());
            }
        }

        #[test]
        fn userprovided_encodes_but_load_may_fail() {
            // Construct a dummy data-model executor with some bytecode; loading may fail,
            // but encoding itself should succeed.
            let raw = iroha_data_model::executor::Executor::new(
                iroha_data_model::transaction::IvmBytecode::from_compiled(vec![0x00, 0x01, 0x02]),
            );
            let bytes = norito::to_bytes(&ExecutorDto::UserProvided(raw)).expect("encode dto");
            // Decoding to materialized `Executor` may fail due to invalid bytecode; assert the error is surfaced.
            let res = from_bytes(&bytes);
            assert!(res.is_err());
        }
    }
}
#[cfg(test)]
#[path = "executor_tests.rs"]
mod tests;

#[cfg(test)]
mod resource_return_tests;

#[cfg(test)]
mod runtime_fault_origin_tests {
    //! Executor verdicts cannot forge native execution-fault provenance.
    use super::*;
    use iroha_data_model::executor::fault::{
        IvmFaultKindV1, IvmFaultPositionV1, IvmFaultSiteV1, IvmFaultV1, IvmInvocationSelectorV1,
    };
    #[test]
    fn executor_verdict_rejects_guest_supplied_runtime_origin() {
        let forged = ValidationFail::IvmFault(IvmFaultV1 {
            kind: IvmFaultKindV1::PermissionDenied,
            site: IvmFaultSiteV1 {
                code_hash: Hash::new(b"unrelated artifact"),
                selector: IvmInvocationSelectorV1::Entrypoint(3),
                position: IvmFaultPositionV1::Execute { pc_offset: 16 },
            },
        });
        assert!(
            matches!(validate_executor_rejection_origin(forged), ValidationFail::InternalError(message) if message.contains("cannot supply a runtime fault origin"))
        );
        let native_permission = ValidationFail::NotPermitted("actual executor policy".into());
        assert_eq!(
            validate_executor_rejection_origin(native_permission.clone()),
            native_permission
        );
    }
}
