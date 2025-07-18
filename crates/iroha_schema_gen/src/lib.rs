//! Iroha schema generation support library. Contains the
//! `build_schemas` `fn`, which is the function which decides which
//! types are included in the schema.
use iroha_data_model::{
    block::stream::{BlockMessage, BlockSubscriptionRequest},
    query::{QueryResponse, SignedQuery},
};
use iroha_schema::prelude::*;
use iroha_telemetry::metrics::Status;

macro_rules! types {
    ($($t:ty),+ $(,)?) => {
        // use all the types in a type position, so that IDE can resolve them
        const _: () = {
            use complete_data_model::*;
            $(
                let _resolve_my_type_pls: $t;
            )+
        };

        /// Apply `callback` to all types in the schema.
        #[macro_export]
        macro_rules! map_all_schema_types {
            ($callback:ident) => {{
                $( $callback!($t); )+
            }}
        }
    }
}

/// Builds the schema for the current state of Iroha.
///
/// You should only include the top-level types because other types
/// shall be included recursively.
pub fn build_schemas() -> MetaMap {
    use iroha_data_model::prelude::*;
    use iroha_executor_data_model::{isi::multisig, permission};

    macro_rules! schemas {
        ($($t:ty),* $(,)?) => {{
            let mut out = MetaMap::new(); $(
            <$t as IntoSchema>::update_schema_map(&mut out); )*
            out
        }};
    }

    schemas! {
        Peer,

        SignedTransaction,
        SignedQuery,
        QueryResponse,

        // Event stream
        EventMessage,
        EventSubscriptionRequest,

        // Block stream
        BlockMessage,
        BlockSubscriptionRequest,

        // Never referenced, but present in type signature. Like `PhantomData<X>`
        MerkleTree<SignedTransaction>,

        // Default permissions
        permission::peer::CanManagePeers,

        permission::domain::CanRegisterDomain,
        permission::domain::CanUnregisterDomain,
        permission::domain::CanModifyDomainMetadata,

        permission::account::CanRegisterAccount,
        permission::account::CanUnregisterAccount,
        permission::account::CanModifyAccountMetadata,

        permission::asset_definition::CanRegisterAssetDefinition,
        permission::asset_definition::CanUnregisterAssetDefinition,
        permission::asset_definition::CanModifyAssetDefinitionMetadata,

        permission::asset::CanMintAssetWithDefinition,
        permission::asset::CanBurnAssetWithDefinition,
        permission::asset::CanTransferAssetWithDefinition,
        permission::asset::CanMintAsset,
        permission::asset::CanBurnAsset,
        permission::asset::CanTransferAsset,

        permission::nft::CanRegisterNft,
        permission::nft::CanUnregisterNft,
        permission::nft::CanTransferNft,
        permission::nft::CanModifyNftMetadata,

        permission::parameter::CanSetParameters,
        permission::role::CanManageRoles,

        permission::trigger::CanRegisterTrigger,
        permission::trigger::CanExecuteTrigger,
        permission::trigger::CanUnregisterTrigger,
        permission::trigger::CanModifyTrigger,
        permission::trigger::CanModifyTriggerMetadata,

        permission::executor::CanUpgradeExecutor,

        // Multi-signature operations
        multisig::MultisigInstructionBox,
        // Multi-signature account metadata
        multisig::MultisigSpec,
        multisig::MultisigProposalValue,

        // Genesis file - used by SDKs to generate the genesis block
        // TODO: IMO it could/should be removed from the schema
        iroha_genesis::RawGenesisTransaction,

        // It is exposed via Torii
        Status
    }
}

types!(
    Account,
    AccountEvent,
    AccountEventFilter,
    AccountEventSet,
    AccountId,
    AccountIdPredicateAtom,
    AccountIdProjection<PredicateMarker>,
    AccountIdProjection<SelectorMarker>,
    AccountPermissionChanged,
    AccountPredicateAtom,
    AccountProjection<PredicateMarker>,
    AccountProjection<SelectorMarker>,
    AccountRoleChanged,
    Action,
    ActionPredicateAtom,
    ActionProjection<PredicateMarker>,
    ActionProjection<SelectorMarker>,
    Algorithm,
    Asset,
    AssetChanged,
    AssetDefinition,
    AssetDefinitionEvent,
    AssetDefinitionEventFilter,
    AssetDefinitionEventSet,
    AssetDefinitionId,
    AssetDefinitionIdPredicateAtom,
    AssetDefinitionIdProjection<PredicateMarker>,
    AssetDefinitionIdProjection<SelectorMarker>,
    AssetDefinitionOwnerChanged,
    AssetDefinitionPredicateAtom,
    AssetDefinitionProjection<PredicateMarker>,
    AssetDefinitionProjection<SelectorMarker>,
    AssetDefinitionTotalQuantityChanged,
    AssetEvent,
    AssetEventFilter,
    AssetEventSet,
    AssetId,
    AssetIdPredicateAtom,
    AssetIdProjection<PredicateMarker>,
    AssetIdProjection<SelectorMarker>,
    AssetPredicateAtom,
    AssetProjection<PredicateMarker>,
    AssetProjection<SelectorMarker>,
    BTreeMap<AccountId, u8>,
    BTreeMap<CustomParameterId, CustomParameter>,
    BTreeMap<Name, Json>,
    BTreeSet<AccountId>,
    BTreeSet<Permission>,
    BTreeSet<BlockSignature>,
    BTreeSet<String>,
    BlockEvent,
    BlockEventFilter,
    BlockHeaderHashPredicateAtom,
    BlockHeaderHashProjection<PredicateMarker>,
    BlockHeaderHashProjection<SelectorMarker>,
    BlockHeader,
    BlockHeaderPredicateAtom,
    BlockHeaderProjection<PredicateMarker>,
    BlockHeaderProjection<SelectorMarker>,
    BlockMessage,
    BlockParameter,
    BlockParameters,
    BlockPayload,
    BlockRejectionReason,
    BlockResult,
    BlockSignature,
    BlockStatus,
    BlockSubscriptionRequest,
    Box<AssetId>,
    Box<CompoundPredicate<Account>>,
    Box<CompoundPredicate<AssetDefinition>>,
    Box<CompoundPredicate<Asset>>,
    Box<CompoundPredicate<BlockHeader>>,
    Box<CompoundPredicate<CommittedTransaction>>,
    Box<CompoundPredicate<Domain>>,
    Box<CompoundPredicate<Nft>>,
    Box<CompoundPredicate<PeerId>>,
    Box<CompoundPredicate<Permission>>,
    Box<CompoundPredicate<RoleId>>,
    Box<CompoundPredicate<Role>>,
    Box<CompoundPredicate<SignedBlock>>,
    Box<CompoundPredicate<TriggerId>>,
    Box<CompoundPredicate<Trigger>>,
    Box<InstructionExecutionFail>,
    Box<Permission>,
    Box<RepetitionError>,
    Box<TransactionRejectionReason>,
    Burn<Numeric, Asset>,
    Burn<u32, Trigger>,
    BurnBox,
    ChainId,
    CommittedTransaction,
    CommittedTransactionPredicateAtom,
    CommittedTransactionProjection<PredicateMarker>,
    CommittedTransactionProjection<SelectorMarker>,
    CompoundPredicate<Account>,
    CompoundPredicate<AssetDefinition>,
    CompoundPredicate<Asset>,
    CompoundPredicate<BlockHeader>,
    CompoundPredicate<CommittedTransaction>,
    CompoundPredicate<Domain>,
    CompoundPredicate<Nft>,
    CompoundPredicate<PeerId>,
    CompoundPredicate<Permission>,
    CompoundPredicate<RoleId>,
    CompoundPredicate<Role>,
    CompoundPredicate<SignedBlock>,
    CompoundPredicate<TriggerId>,
    CompoundPredicate<Trigger>,
    ConfigurationEvent,
    ConfigurationEventFilter,
    ConfigurationEventSet,
    ConstString,
    ConstVec<InstructionBox>,
    ConstVec<u8>,
    CustomInstruction,
    CustomParameter,
    CustomParameterId,
    DataEvent,
    DataEventFilter,
    DataTriggerStep,
    Domain,
    DomainEvent,
    DomainEventFilter,
    DomainEventSet,
    DomainId,
    DomainIdPredicateAtom,
    DomainIdProjection<PredicateMarker>,
    DomainIdProjection<SelectorMarker>,
    DomainOwnerChanged,
    DomainPredicateAtom,
    DomainProjection<PredicateMarker>,
    DomainProjection<SelectorMarker>,
    EventBox,
    EventFilterBox,
    EventMessage,
    EventSubscriptionRequest,
    Executable,
    ExecuteTrigger,
    ExecuteTriggerEvent,
    ExecuteTriggerEventFilter,
    ExecutionStep,
    ExecutionTime,
    Executor,
    ExecutorDataModel,
    ExecutorEvent,
    ExecutorEventFilter,
    ExecutorEventSet,
    WasmPath,
    ExecutorUpgrade,
    FetchSize,
    FindAccounts,
    FindAccountsWithAsset,
    FindActiveTriggerIds,
    FindAssets,
    FindAssetsDefinitions,
    FindBlockHeaders,
    FindBlocks,
    FindDomains,
    FindError,
    FindExecutorDataModel,
    FindNfts,
    FindParameters,
    FindPeers,
    FindPermissionsByAccountId,
    FindRoleIds,
    FindRoles,
    FindRolesByAccountId,
    FindTransactions,
    FindTriggers,
    ForwardCursor,
    GenesisWasmAction,
    GenesisWasmTrigger,
    Grant<Permission, Account>,
    Grant<Permission, Role>,
    Grant<RoleId, Account>,
    GrantBox,
    Hash,
    HashOf<BlockHeader>,
    HashOf<MerkleTree<TransactionEntrypoint>>,
    HashOf<MerkleTree<TransactionResult>>,
    HashOf<SignedTransaction>,
    HashOf<TransactionEntrypoint>,
    HashOf<TransactionResult>,
    HashOf<Vec<InstructionBox>>,
    IdBox,
    InstructionBox,
    InstructionEvaluationError,
    InstructionExecutionError,
    InstructionExecutionFail,
    InstructionType,
    InvalidParameterError,
    IpfsPath,
    Ipv6Addr,
    Ipv4Addr,
    Json,
    JsonPredicateAtom,
    JsonProjection<PredicateMarker>,
    JsonProjection<SelectorMarker>,
    Level,
    Log,
    MathError,
    MerkleProof<TransactionEntrypoint>,
    MerkleProof<TransactionResult>,
    MerkleTree<SignedTransaction>,
    MerkleTree<TransactionEntrypoint>,
    MerkleTree<TransactionResult>,
    Metadata,
    MetadataChanged<AccountId>,
    MetadataChanged<AssetDefinitionId>,
    MetadataChanged<DomainId>,
    MetadataChanged<NftId>,
    MetadataChanged<TriggerId>,
    MetadataPredicateAtom,
    MetadataProjection<PredicateMarker>,
    MetadataProjection<SelectorMarker>,
    MetadataKeyProjection<PredicateMarker>,
    MetadataKeyProjection<SelectorMarker>,
    Mint<Numeric, Asset>,
    Mint<u32, Trigger>,
    MintBox,
    MintabilityError,
    Mintable,
    Mismatch<NumericSpec>,
    Name,
    NameProjection<PredicateMarker>,
    NameProjection<SelectorMarker>,
    NewAccount,
    NewAssetDefinition,
    NewDomain,
    NewNft,
    NewRole,
    Nft,
    NftEvent,
    NftEventFilter,
    NftEventSet,
    NftId,
    NftIdPredicateAtom,
    NftIdProjection<PredicateMarker>,
    NftIdProjection<SelectorMarker>,
    NftOwnerChanged,
    NftPredicateAtom,
    NftProjection<PredicateMarker>,
    NftProjection<SelectorMarker>,
    NonZeroU16,
    NonZeroU32,
    NonZeroU64,
    Numeric,
    NumericPredicateAtom,
    NumericProjection<PredicateMarker>,
    NumericProjection<SelectorMarker>,
    NumericSpec,
    Option<AccountId>,
    Option<AssetDefinitionId>,
    Option<AssetId>,
    Option<BlockStatus>,
    Option<DomainId>,
    Option<ForwardCursor>,
    Option<HashOf<BlockHeader>>,
    Option<HashOf<MerkleTree<TransactionEntrypoint>>>,
    Option<HashOf<MerkleTree<TransactionResult>>>,
    Option<HashOf<SignedTransaction>>,
    Option<HashOf<TransactionEntrypoint>>,
    Option<HashOf<TransactionResult>>,
    Option<IpfsPath>,
    Option<Name>,
    Option<NftId>,
    Option<NonZeroU32>,
    Option<NonZeroU64>,
    Option<Option<NonZeroU64>>,
    Option<Parameters>,
    Option<PeerId>,
    Option<RoleId>,
    Option<TransactionStatus>,
    Option<TriggerCompletedOutcomeType>,
    Option<TriggerId>,
    Option<bool>,
    Option<u32>,
    Option<u64>,
    Pagination,
    Parameter,
    ParameterChanged,
    Parameters,
    PeerEvent,
    PeerEventFilter,
    PeerEventSet,
    Peer,
    PeerId,
    PeerIdPredicateAtom,
    PeerIdProjection<PredicateMarker>,
    PeerIdProjection<SelectorMarker>,
    Permission,
    PermissionPredicateAtom,
    PermissionProjection<PredicateMarker>,
    PermissionProjection<SelectorMarker>,
    PipelineEventBox,
    PipelineEventFilterBox,
    PublicKey,
    PublicKeyPredicateAtom,
    PublicKeyProjection<PredicateMarker>,
    PublicKeyProjection<SelectorMarker>,
    QueryBox,
    QueryExecutionFail,
    QueryOutput,
    QueryOutputBatchBox,
    QueryOutputBatchBoxTuple,
    QueryParams,
    QueryRequest,
    QueryRequestWithAuthority,
    QueryResponse,
    QuerySignature,
    QueryWithFilter<FindAccounts>,
    QueryWithFilter<FindAccountsWithAsset>,
    QueryWithFilter<FindActiveTriggerIds>,
    QueryWithFilter<FindAssets>,
    QueryWithFilter<FindAssetsDefinitions>,
    QueryWithFilter<FindBlockHeaders>,
    QueryWithFilter<FindBlocks>,
    QueryWithFilter<FindDomains>,
    QueryWithFilter<FindNfts>,
    QueryWithFilter<FindPeers>,
    QueryWithFilter<FindPermissionsByAccountId>,
    QueryWithFilter<FindRoleIds>,
    QueryWithFilter<FindRoles>,
    QueryWithFilter<FindRolesByAccountId>,
    QueryWithFilter<FindTransactions>,
    QueryWithFilter<FindTriggers>,
    QueryWithParams,
    Register<Account>,
    Register<AssetDefinition>,
    Register<Domain>,
    Register<Nft>,
    Register<Peer>,
    Register<Role>,
    Register<Trigger>,
    RegisterBox,
    RemoveKeyValue<Account>,
    RemoveKeyValue<AssetDefinition>,
    RemoveKeyValue<Domain>,
    RemoveKeyValue<Nft>,
    RemoveKeyValue<Trigger>,
    RemoveKeyValueBox,
    Repeats,
    RepetitionError,
    Result<DataTriggerSequence, TransactionRejectionReason>,
    Revoke<Permission, Account>,
    Revoke<Permission, Role>,
    Revoke<RoleId, Account>,
    RevokeBox,
    Role,
    RoleEvent,
    RoleEventFilter,
    RoleEventSet,
    RoleId,
    RoleIdPredicateAtom,
    RoleIdProjection<PredicateMarker>,
    RoleIdProjection<SelectorMarker>,
    RolePermissionChanged,
    RolePredicateAtom,
    RoleProjection<PredicateMarker>,
    RoleProjection<SelectorMarker>,
    SelectorTuple<Account>,
    SelectorTuple<AssetDefinition>,
    SelectorTuple<Asset>,
    SelectorTuple<BlockHeader>,
    SelectorTuple<CommittedTransaction>,
    SelectorTuple<Domain>,
    SelectorTuple<Nft>,
    SelectorTuple<PeerId>,
    SelectorTuple<Permission>,
    SelectorTuple<RoleId>,
    SelectorTuple<Role>,
    SelectorTuple<SignedBlock>,
    SelectorTuple<TriggerId>,
    SelectorTuple<Trigger>,
    SetKeyValue<Account>,
    SetKeyValue<AssetDefinition>,
    SetKeyValue<Domain>,
    SetKeyValue<Nft>,
    SetKeyValue<Trigger>,
    SetKeyValueBox,
    SetParameter,
    Signature,
    SignatureOf<BlockHeader>,
    SignatureOf<QueryRequestWithAuthority>,
    SignatureOf<TransactionPayload>,
    SignedBlock,
    SignedBlockPredicateAtom,
    SignedBlockProjection<PredicateMarker>,
    SignedBlockProjection<SelectorMarker>,
    SignedBlockV1,
    SignedQuery,
    SignedQueryV1,
    SignedTransaction,
    SignedTransactionV1,
    SingularQueryBox,
    SingularQueryOutputBox,
    SmartContractParameter,
    SmartContractParameters,
    SocketAddr,
    SocketAddrHost,
    SocketAddrV4,
    SocketAddrV6,
    Sorting,
    Status,
    String,
    StringPredicateAtom,
    SumeragiParameter,
    SumeragiParameters,
    TimeEvent,
    TimeEventFilter,
    TimeInterval,
    TimeSchedule,
    TimeTriggerEntrypoint,
    TransactionEntrypoint,
    TransactionEntrypointHashPredicateAtom,
    TransactionEntrypointHashProjection<PredicateMarker>,
    TransactionEntrypointHashProjection<SelectorMarker>,
    TransactionEntrypointPredicateAtom,
    TransactionEntrypointProjection<PredicateMarker>,
    TransactionEntrypointProjection<SelectorMarker>,
    TransactionEvent,
    TransactionEventFilter,
    TransactionLimitError,
    TransactionParameter,
    TransactionParameters,
    TransactionPayload,
    TransactionRejectionReason,
    TransactionResult,
    TransactionResultHashPredicateAtom,
    TransactionResultHashProjection<PredicateMarker>,
    TransactionResultHashProjection<SelectorMarker>,
    TransactionResultPredicateAtom,
    TransactionResultProjection<PredicateMarker>,
    TransactionResultProjection<SelectorMarker>,
    TransactionSignature,
    TransactionStatus,
    Transfer<Account, AssetDefinitionId, Account>,
    Transfer<Account, DomainId, Account>,
    Transfer<Account, NftId, Account>,
    Transfer<Asset, Numeric, Account>,
    TransferBox,
    Trigger,
    TriggerCompletedEvent,
    TriggerCompletedEventFilter,
    TriggerCompletedOutcome,
    TriggerCompletedOutcomeType,
    TriggerEvent,
    TriggerEventFilter,
    TriggerEventSet,
    TriggerExecutionFail,
    TriggerId,
    TriggerIdPredicateAtom,
    TriggerIdProjection<PredicateMarker>,
    TriggerIdProjection<SelectorMarker>,
    TriggerNumberOfExecutionsChanged,
    TriggerPredicateAtom,
    TriggerProjection<PredicateMarker>,
    TriggerProjection<SelectorMarker>,
    DataTriggerSequence,
    TypeError,
    Unregister<Account>,
    Unregister<AssetDefinition>,
    Unregister<Domain>,
    Unregister<Nft>,
    Unregister<Peer>,
    Unregister<Role>,
    Unregister<Trigger>,
    UnregisterBox,
    Upgrade,
    Uptime,
    ValidationFail,
    Vec<Account>,
    Vec<AccountId>,
    Vec<Action>,
    Vec<Asset>,
    Vec<AssetId>,
    Vec<AssetDefinition>,
    Vec<AssetDefinitionId>,
    Vec<BlockHeader>,
    Vec<CommittedTransaction>,
    Vec<CompoundPredicate<Account>>,
    Vec<CompoundPredicate<AssetDefinition>>,
    Vec<CompoundPredicate<Asset>>,
    Vec<CompoundPredicate<BlockHeader>>,
    Vec<CompoundPredicate<CommittedTransaction>>,
    Vec<CompoundPredicate<Domain>>,
    Vec<CompoundPredicate<Nft>>,
    Vec<CompoundPredicate<PeerId>>,
    Vec<CompoundPredicate<Permission>>,
    Vec<CompoundPredicate<RoleId>>,
    Vec<CompoundPredicate<Role>>,
    Vec<CompoundPredicate<SignedBlock>>,
    Vec<CompoundPredicate<TriggerId>>,
    Vec<CompoundPredicate<Trigger>>,
    Vec<Domain>,
    Vec<DomainId>,
    Vec<EventFilterBox>,
    Vec<GenesisWasmTrigger>,
    Vec<InstructionBox>,
    Vec<Json>,
    Vec<Nft>,
    Vec<NftId>,
    Vec<NftProjection<SelectorMarker>>,
    Vec<Parameter>,
    Vec<PeerId>,
    Vec<Permission>,
    Vec<QueryOutputBatchBox>,
    Vec<Role>,
    Vec<RoleId>,
    Vec<SignedBlock>,
    Vec<SignedTransaction>,
    Vec<AccountProjection<SelectorMarker>>,
    Vec<AssetDefinitionProjection<SelectorMarker>>,
    Vec<AssetProjection<SelectorMarker>>,
    Vec<BlockHeaderProjection<SelectorMarker>>,
    Vec<CommittedTransactionProjection<SelectorMarker>>,
    Vec<DomainProjection<SelectorMarker>>,
    Vec<HashOf<BlockHeader>>,
    Vec<HashOf<TransactionEntrypoint>>,
    Vec<HashOf<TransactionResult>>,
    Vec<Metadata>,
    Vec<Name>,
    Vec<Numeric>,
    Vec<Option<HashOf<TransactionEntrypoint>>>,
    Vec<Option<HashOf<TransactionResult>>>,
    Vec<PeerIdProjection<SelectorMarker>>,
    Vec<PermissionProjection<SelectorMarker>>,
    Vec<PublicKey>,
    Vec<RoleIdProjection<SelectorMarker>>,
    Vec<RoleProjection<SelectorMarker>>,
    Vec<SignedBlockProjection<SelectorMarker>>,
    Vec<String>,
    Vec<TransactionEntrypoint>,
    Vec<TransactionResult>,
    Vec<TimeTriggerEntrypoint>,
    Vec<TriggerIdProjection<SelectorMarker>>,
    Vec<TriggerProjection<SelectorMarker>>,
    Vec<Trigger>,
    Vec<TriggerId>,
    Vec<u8>,
    WasmExecutionFail,
    WasmSmartContract,

    (),
    [u16; 8],
    [u8; 4],
    [u8; 32],
    bool,
    u16,
    u32,
    u64,
    u8,

    iroha_genesis::RawGenesisTransaction,
);

pub mod complete_data_model {
    //! Complete set of types participating in the schema

    pub use core::num::{NonZeroU16, NonZeroU32, NonZeroU64};
    pub use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

    pub use iroha_crypto::*;
    pub use iroha_data_model::{
        account::NewAccount,
        asset::NewAssetDefinition,
        block::{
            error::BlockRejectionReason,
            stream::{BlockMessage, BlockSubscriptionRequest},
            BlockHeader, BlockPayload, BlockResult, BlockSignature, SignedBlock, SignedBlockV1,
        },
        domain::NewDomain,
        events::pipeline::{BlockEventFilter, TransactionEventFilter},
        executor::{Executor, ExecutorDataModel},
        ipfs::IpfsPath,
        isi::{
            error::{
                InstructionEvaluationError, InstructionExecutionError, InvalidParameterError,
                MathError, MintabilityError, Mismatch, RepetitionError, TypeError,
            },
            InstructionType,
        },
        parameter::{
            BlockParameter, BlockParameters, CustomParameter, CustomParameterId, Parameter,
            Parameters, SmartContractParameter, SmartContractParameters, SumeragiParameter,
            SumeragiParameters, TransactionParameter, TransactionParameters,
        },
        prelude::*,
        query::{
            dsl::{CompoundPredicate, PredicateMarker, SelectorMarker},
            error::{FindError, QueryExecutionFail},
            parameters::{ForwardCursor, QueryParams},
            CommittedTransaction, QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple,
            QueryRequestWithAuthority, QueryResponse, QuerySignature, QueryWithFilter,
            QueryWithParams, SignedQuery, SignedQueryV1, SingularQueryOutputBox,
        },
        transaction::{
            error::TransactionLimitError, SignedTransactionV1, TransactionPayload,
            TransactionSignature,
        },
        Level,
    };
    pub use iroha_genesis::{GenesisWasmAction, GenesisWasmTrigger, WasmPath};
    pub use iroha_primitives::{
        addr::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrHost, SocketAddrV4, SocketAddrV6},
        const_vec::ConstVec,
        conststr::ConstString,
        json::Json,
    };
    pub use iroha_schema::Compact;
    pub use iroha_telemetry::metrics::{Status, Uptime};
}

#[cfg(test)]
mod tests {
    use iroha_schema::MetaMapEntry;

    use super::{complete_data_model::*, IntoSchema};

    fn is_const_generic(generic: &str) -> bool {
        generic.parse::<usize>().is_ok()
    }

    fn generate_test_map() -> BTreeMap<core::any::TypeId, String> {
        let mut map = BTreeMap::new();

        macro_rules! insert_into_test_map {
            ($t:ty) => {{
                let type_id = <$t as iroha_schema::TypeId>::id();

                if let Some(type_id) = map.insert(core::any::TypeId::of::<$t>(), type_id) {
                    panic!(
                        "{}: Duplicate type id. Make sure that type ids are unique",
                        type_id
                    );
                }
            }};
        }

        map_all_schema_types!(insert_into_test_map);

        insert_into_test_map!(Compact<u128>);
        insert_into_test_map!(Compact<u64>);
        insert_into_test_map!(Compact<u32>);

        insert_into_test_map!(iroha_executor_data_model::permission::peer::CanManagePeers);
        insert_into_test_map!(iroha_executor_data_model::permission::domain::CanRegisterDomain);
        insert_into_test_map!(iroha_executor_data_model::permission::domain::CanUnregisterDomain);
        insert_into_test_map!(
            iroha_executor_data_model::permission::domain::CanModifyDomainMetadata
        );
        insert_into_test_map!(iroha_executor_data_model::permission::account::CanRegisterAccount);
        insert_into_test_map!(iroha_executor_data_model::permission::account::CanUnregisterAccount);
        insert_into_test_map!(
            iroha_executor_data_model::permission::account::CanModifyAccountMetadata
        );
        insert_into_test_map!(
            iroha_executor_data_model::permission::asset_definition::CanRegisterAssetDefinition
        );
        insert_into_test_map!(
            iroha_executor_data_model::permission::asset_definition::CanUnregisterAssetDefinition
        );
        insert_into_test_map!(iroha_executor_data_model::permission::asset_definition::CanModifyAssetDefinitionMetadata);
        insert_into_test_map!(
            iroha_executor_data_model::permission::asset::CanMintAssetWithDefinition
        );
        insert_into_test_map!(
            iroha_executor_data_model::permission::asset::CanBurnAssetWithDefinition
        );
        insert_into_test_map!(
            iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition
        );
        insert_into_test_map!(iroha_executor_data_model::permission::asset::CanMintAsset);
        insert_into_test_map!(iroha_executor_data_model::permission::asset::CanBurnAsset);
        insert_into_test_map!(iroha_executor_data_model::permission::asset::CanTransferAsset);

        insert_into_test_map!(iroha_executor_data_model::permission::nft::CanRegisterNft);
        insert_into_test_map!(iroha_executor_data_model::permission::nft::CanUnregisterNft);
        insert_into_test_map!(iroha_executor_data_model::permission::nft::CanTransferNft);
        insert_into_test_map!(iroha_executor_data_model::permission::nft::CanModifyNftMetadata);

        insert_into_test_map!(iroha_executor_data_model::permission::parameter::CanSetParameters);
        insert_into_test_map!(iroha_executor_data_model::permission::role::CanManageRoles);

        insert_into_test_map!(iroha_executor_data_model::permission::trigger::CanRegisterTrigger);
        insert_into_test_map!(iroha_executor_data_model::permission::trigger::CanExecuteTrigger);
        insert_into_test_map!(iroha_executor_data_model::permission::trigger::CanUnregisterTrigger);
        insert_into_test_map!(iroha_executor_data_model::permission::trigger::CanModifyTrigger);
        insert_into_test_map!(
            iroha_executor_data_model::permission::trigger::CanModifyTriggerMetadata
        );
        insert_into_test_map!(iroha_executor_data_model::permission::executor::CanUpgradeExecutor);

        insert_into_test_map!(iroha_executor_data_model::isi::multisig::MultisigInstructionBox);
        insert_into_test_map!(iroha_executor_data_model::isi::multisig::MultisigRegister);
        insert_into_test_map!(iroha_executor_data_model::isi::multisig::MultisigPropose);
        insert_into_test_map!(iroha_executor_data_model::isi::multisig::MultisigApprove);
        insert_into_test_map!(iroha_executor_data_model::isi::multisig::MultisigSpec);
        insert_into_test_map!(iroha_executor_data_model::isi::multisig::MultisigProposalValue);

        map
    }

    // For `PhantomData` wrapped types schemas aren't expanded recursively.
    // This test ensures that schemas for those types are present as well.
    fn find_missing_type_params(type_names: &HashSet<String>) -> HashMap<&str, Vec<&str>> {
        let mut missing_schemas = HashMap::<&str, _>::new();

        for type_name in type_names {
            let (Some(start), Some(end)) = (type_name.find('<'), type_name.rfind('>')) else {
                continue;
            };

            assert!(start < end, "Invalid type name: {type_name}");

            for generic in type_name.split(", ") {
                if !is_const_generic(generic) {
                    continue;
                }

                if !type_names.contains(generic) {
                    missing_schemas
                        .entry(type_name)
                        .or_insert_with(Vec::new)
                        .push(generic);
                }
            }
        }

        missing_schemas
    }

    #[test]
    fn no_extra_or_missing_schemas() {
        // NOTE: Skipping Box<str> until [this PR](https://github.com/paritytech/parity-scale-codec/pull/565) is merged
        let exceptions: [core::any::TypeId; 1] = [core::any::TypeId::of::<Box<str>>()];

        let schemas_types = super::build_schemas()
            .into_iter()
            .collect::<HashMap<_, _>>();
        let map_types = generate_test_map();

        let mut extra_types = HashSet::new();
        for (type_id, schema) in &map_types {
            if !schemas_types.contains_key(type_id) {
                extra_types.insert(schema);
            }
        }
        assert!(extra_types.is_empty(), "Extra types: {extra_types:#?}");

        let mut missing_types = HashSet::new();
        for (type_id, schema) in &schemas_types {
            if !map_types.contains_key(type_id) && !exceptions.contains(type_id) {
                missing_types.insert(schema);
            }
        }
        assert!(
            missing_types.is_empty(),
            "Missing types: {missing_types:#?}",
        );
    }

    #[test]
    fn no_missing_referenced_types() {
        let type_names = super::build_schemas()
            .into_iter()
            .map(|(_, MetaMapEntry { type_id, .. })| type_id)
            .collect();
        let missing_schemas = find_missing_type_params(&type_names);

        assert!(
            missing_schemas.is_empty(),
            "Missing schemas: \n{missing_schemas:#?}"
        );
    }

    #[test]
    // NOTE: This test guards from incorrect implementation where
    // `SortedVec<T>` and `Vec<T>` start stepping over each other
    fn no_schema_type_overlap() {
        let mut schemas = super::build_schemas();
        <Vec<PublicKey>>::update_schema_map(&mut schemas);
        <BTreeSet<SignedTransactionV1>>::update_schema_map(&mut schemas);
    }
}
