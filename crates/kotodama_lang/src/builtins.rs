//! Shared builtin classification for the stable Kotodama helper surface.
//!
//! The parser still sees raw identifiers, but semantic analysis, lowering, and
//! effect checks should agree on the canonical builtin set through this enum
//! instead of open-coded string matching.

/// Typed pointer ABI constructors recognized by compiler lowering.
///
/// Only constructors whose enclosing [`Builtin`] has a source-visible surface
/// are part of Kotodama V1; the remaining variants are host/compiler plumbing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerConstructor {
    AccountId,
    AssetDefinition,
    AssetId,
    NftId,
    Name,
    Json,
    Domain,
    DomainId,
    Blob,
    NoritoBytes,
    DataSpaceId,
    AxtDescriptor,
    AssetHandle,
    ProofBlob,
    SoracloudRequest,
    SoracloudResponse,
}

impl PointerConstructor {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "account_id" => Self::AccountId,
            "asset_definition" => Self::AssetDefinition,
            "asset_id" => Self::AssetId,
            "nft_id" => Self::NftId,
            "name" => Self::Name,
            "json" => Self::Json,
            "domain" => Self::Domain,
            "domain_id" => Self::DomainId,
            "blob" => Self::Blob,
            "norito_bytes" => Self::NoritoBytes,
            "dataspace_id" => Self::DataSpaceId,
            "axt_descriptor" => Self::AxtDescriptor,
            "asset_handle" => Self::AssetHandle,
            "proof_blob" => Self::ProofBlob,
            "soracloud_request" => Self::SoracloudRequest,
            "soracloud_response" => Self::SoracloudResponse,
            _ => return None,
        })
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::AccountId => "account_id",
            Self::AssetDefinition => "asset_definition",
            Self::AssetId => "asset_id",
            Self::NftId => "nft_id",
            Self::Name => "name",
            Self::Json => "json",
            Self::Domain => "domain",
            Self::DomainId => "domain_id",
            Self::Blob => "blob",
            Self::NoritoBytes => "norito_bytes",
            Self::DataSpaceId => "dataspace_id",
            Self::AxtDescriptor => "axt_descriptor",
            Self::AssetHandle => "asset_handle",
            Self::ProofBlob => "proof_blob",
            Self::SoracloudRequest => "soracloud_request",
            Self::SoracloudResponse => "soracloud_response",
        }
    }

    const fn return_type_name(self) -> &'static str {
        match self {
            Self::AccountId => "AccountId",
            Self::AssetDefinition => "AssetDefinitionId",
            Self::AssetId => "AssetId",
            Self::NftId => "NftId",
            Self::Name => "Name",
            Self::Json => "Json",
            Self::Domain | Self::DomainId => "DomainId",
            Self::Blob | Self::NoritoBytes => "bytes",
            Self::DataSpaceId => "DataSpaceId",
            Self::AxtDescriptor => "AxtDescriptor",
            Self::AssetHandle => "AssetHandle",
            Self::ProofBlob => "ProofBlob",
            Self::SoracloudRequest => "SoracloudRequest",
            Self::SoracloudResponse => "SoracloudResponse",
        }
    }
}

/// Security-relevant effects produced by a builtin call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuiltinEffects {
    /// The call can observe or mutate host-managed state beyond ordinary reads.
    pub host_side_effects: bool,
    /// The call submits an Iroha instruction or invokes another contract.
    pub emits_instructions: bool,
    /// The call mutates contract-owned durable state.
    pub mutates_durable_state: bool,
}

impl BuiltinEffects {
    /// No externally visible effects.
    pub const NONE: Self = Self {
        host_side_effects: false,
        emits_instructions: false,
        mutates_durable_state: false,
    };
    /// Host-managed effect requiring entrypoint authorization.
    pub const HOST: Self = Self {
        host_side_effects: true,
        ..Self::NONE
    };
    /// Iroha instruction emission requiring entrypoint authorization.
    pub const INSTRUCTION: Self = Self {
        emits_instructions: true,
        ..Self::NONE
    };
    /// Contract durable-state mutation requiring entrypoint authorization.
    pub const DURABLE_STATE: Self = Self {
        mutates_durable_state: true,
        ..Self::NONE
    };
}

/// Coarse scheduler access class for a builtin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuiltinAccess {
    /// No world or durable-state access.
    #[default]
    None,
    /// Contract durable-state read.
    StateRead,
    /// Contract durable-state write.
    StateWrite,
    /// Ledger read whose exact key is derived separately.
    LedgerRead,
    /// Ledger write whose exact key is derived separately.
    LedgerWrite,
    /// Dynamic access that must conservatively serialize when unresolved.
    Dynamic,
}

/// Execution mode required by a builtin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuiltinMode {
    /// Available to ordinary contracts.
    #[default]
    Any,
    /// Available only when compiler build policy enables ZK mode.
    ZkOnly,
    /// Available only to local test builds or `#[test]` functions.
    TestOnly,
    /// Available only inside a `#[test]` function (never ordinary seiyaku code).
    TestFunctionOnly,
    /// Compiler/runtime implementation detail, not a V1 source API.
    CompilerInternal,
}

/// Coarse gas model used for compiler and host consistency checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuiltinGasClass {
    /// Fixed-cost pure operation.
    #[default]
    Constant,
    /// Cost scales with an input byte or element count.
    LinearInput,
    /// The host quotes the deterministic cost before execution.
    HostQuoted,
}

/// Source-call form admitted for a builtin.
///
/// Method calls are desugared to an internal free-call shape after parsing, so
/// this classification must remain separate from [`BuiltinMode`]. In
/// particular, a method-only helper must never become a source-visible global
/// merely because lowering recognizes its internal name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinSurface {
    /// A canonical namespaced (or language-intrinsic) free function.
    Function,
    /// A receiver method only; the parser rejects the equivalent free call.
    MethodOnly,
    /// Both a canonical namespaced function and a receiver method.
    FunctionOrMethod,
    /// Not callable from V1 source.
    CompilerInternal,
}

/// How a builtin reaches the IVM host boundary.
///
/// The syscall list contains operation syscalls only. Pointer publication is
/// ABI plumbing shared by many calls and is deliberately not repeated here.
/// Keeping direct and derived calls distinct lets security tests prove that a
/// helper cannot hide a privileged operation behind apparently pure lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinLowering {
    /// Lowers entirely to deterministic IVM instructions or static data.
    Instructions,
    /// Lowers one-to-one to the single operation syscall in the spec.
    DirectSyscall,
    /// Expands to a compiler-owned sequence that can issue these operation
    /// syscalls recorded in the spec. The list is exhaustive for every
    /// control-flow path.
    DerivedSyscalls,
}

/// Machine-readable source signature for a builtin.
///
/// Parameter descriptors use canonical Kotodama type names. `A|B` denotes a
/// closed union, a trailing `?` denotes an optional final parameter, and a
/// trailing `...` denotes a homogeneous variadic tail. Generic relationships
/// such as `K`, `V`, and `same-as-arg0` are resolved by semantic analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuiltinSignature {
    /// Ordered source parameter names used by named calls.
    pub parameter_names: &'static [&'static str],
    /// Ordered parameter type descriptors.
    pub parameters: &'static [&'static str],
    /// Return type descriptor.
    pub return_type: &'static str,
}

impl BuiltinSignature {
    const fn new(parameters: &'static [&'static str], return_type: &'static str) -> Self {
        Self {
            parameter_names: default_parameter_names(parameters.len()),
            parameters,
            return_type,
        }
    }

    const fn with_names(mut self, parameter_names: &'static [&'static str]) -> Self {
        self.parameter_names = parameter_names;
        self
    }
}

const fn default_parameter_names(arity: usize) -> &'static [&'static str] {
    match arity {
        0 => &[],
        1 => &["value"],
        2 => &["first", "second"],
        3 => &["first", "second", "third"],
        4 => &["first", "second", "third", "fourth"],
        5 => &["first", "second", "third", "fourth", "fifth"],
        6 => &["first", "second", "third", "fourth", "fifth", "sixth"],
        7 => &[
            "first", "second", "third", "fourth", "fifth", "sixth", "seventh",
        ],
        8 => &[
            "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth",
        ],
        _ => &[],
    }
}

/// Source argument policy attached to a builtin declaration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuiltinCallPolicy {
    /// Positional or named arguments are admitted unless the semantic
    /// repeated-type/effect policy requires names.
    #[default]
    Flexible,
    /// Pagination calls always require explicit `offset` and `limit` names.
    Pagination,
}

/// Canonical security and lowering metadata for one builtin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuiltinSpec {
    /// Canonical source spelling.
    pub name: &'static str,
    /// Security-relevant effects.
    pub effects: BuiltinEffects,
    /// Scheduler access class.
    pub access: BuiltinAccess,
    /// Required execution mode.
    pub mode: BuiltinMode,
    /// Source-call form admitted by the V1 grammar.
    pub surface: BuiltinSurface,
    /// Gas charging class.
    pub gas: BuiltinGasClass,
    /// Exact operation-level lowering classification.
    pub lowering: BuiltinLowering,
    /// Complete set of operation syscalls reachable from the builtin.
    pub operation_syscalls: &'static [u32],
    /// Direct syscall number, when the builtin lowers one-to-one to a syscall.
    pub syscall: Option<u32>,
    /// Canonical parameter and return types.
    pub signature: BuiltinSignature,
    /// Source argument policy.
    pub call_policy: BuiltinCallPolicy,
}

/// Canonical Kotodama helper/builtin calls that are part of the current source
/// surface and are worth classifying centrally.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    PointerConstructor(PointerConstructor),
    Contains,
    GetOrDefault,
    GetOr,
    Ensure,
    StateMapRemove,
    KeysTake2,
    ValuesTake2,
    KeysValuesTake2,
    StateGet,
    StateSet,
    StateDel,
    StateKeys,
    StateHas,
    StateLen,
    StateCount,
    QueryExecuteNorito,
    QueryGetAccount,
    QueryGetAsset,
    QueryGetAssetDefinition,
    QueryGetDomain,
    QueryGetNft,
    QueryPageAccounts,
    QueryPageAssets,
    QueryPageAssetDefinitions,
    QueryPageDomains,
    QueryPageNfts,
    QueryGetParameter,
    QueryGetContractManifest,
    QueryGetContractInstance,
    RecordSccpMessage,
    ExecuteQuery,
    ScExecuteSubmitBallot,
    ScExecuteUnshield,
    ResolveAccountAlias,
    SubscriptionBill,
    SubscriptionRecordUsage,
    GetAccountBalance,
    GetPublicInput,
    DebugPrint,
    DebugLog,
    Assert,
    Require,
    Info,
    AssertEq,
    TestInvokeEntrypoint,
    TestInvokeEntrypointAs,
    TestExpectRejectAs,
    TestActorAccount,
    TestActorPublicKey,
    TestActorSign,
    SetAccountDetail,
    MintAsset,
    BurnAsset,
    TransferAsset,
    SetAssetTransferFreeze,
    SetAssetTransferDailyLimit,
    AccountRecoveryPropose,
    AccountRecoveryApprove,
    AccountRecoveryCancel,
    AccountRecoveryFinalize,
    NftMintAsset,
    NftSetMetadata,
    NftBurnAsset,
    NftTransferAsset,
    RegisterDomain,
    UnregisterDomain,
    TransferDomain,
    RegisterAccount,
    UnregisterAccount,
    RegisterAsset,
    CreateNewAsset,
    UnregisterAsset,
    RegisterPeer,
    UnregisterPeer,
    CreateTrigger,
    RegisterTrigger,
    RemoveTrigger,
    UnregisterTrigger,
    SetTriggerEnabled,
    CreateRole,
    DeleteRole,
    GrantRole,
    RevokeRole,
    GrantPermission,
    RevokePermission,
    GrantContractEntrypoint,
    RevokeContractEntrypoint,
    EscrowOpenOffer,
    EscrowAccept,
    EscrowMarkPaymentSent,
    EscrowRelease,
    EscrowCancel,
    EscrowOpenDispute,
    EscrowResolveDispute,
    AnonymousEscrowOpenOffer,
    AnonymousEscrowAccept,
    AnonymousEscrowMarkPaymentSent,
    AnonymousEscrowRelease,
    AnonymousEscrowCancel,
    AnonymousEscrowOpenDispute,
    AnonymousEscrowResolveDispute,
    GetPrivateInput,
    UseNullifier,
    CommitOutput,
    CreateNftsForAllUsers,
    SetExecutionDepth,
    TransferV1BatchBegin,
    TransferV1BatchEnd,
    TransferV1BatchApply,
    TransferBatch,
    AxtBegin,
    AxtTouch,
    VerifyDsProof,
    UseAssetHandle,
    AxtCommit,
    DeactivateContractInstance,
    RemoveSmartContractBytes,
    RegisterSmartContractCode,
    RegisterSmartContractBytes,
    ActivateContractInstance,
    ZkRootsGet,
    ZkVoteGetTally,
    ZkVerifyTransfer,
    ZkVerifyUnshield,
    ZkVerifyBatch,
    ZkVoteVerifyBallot,
    ZkVoteVerifyTally,
    BuildSubmitBallotInline,
    BuildUnshieldInline,
    VrfEpochSeed,
    VrfVerify,
    VrfVerifyBatch,
    Sm3Hash,
    Sha256Hash,
    Sha3Hash,
    Blake2b256Hash,
    Keccak256Hash,
    IrohaHash,
    Sm2Verify,
    VerifySignature,
    Sm4GcmSeal,
    Sm4GcmOpen,
    Sm4CcmSeal,
    Sm4CcmOpen,
    Alloc,
    ProveExecution,
    GrowHeap,
    VerifyProof,
    GetMerklePath,
    GetMerkleCompact,
    GetRegisterMerkleCompact,
    SoracloudReadCommittedState,
    SoracloudEmitStateMutation,
    SoracloudEmitMailboxMessage,
    SoracloudAppendJournal,
    SoracloudPublishCheckpoint,
    SoracloudReadSecret,
    SoracloudReadCredential,
    SoracloudEgressFetch,
    SoracloudReadConfig,
    SoracloudReadSecretEnvelope,
    AddSignatory,
    RemoveSignatory,
    SetAccountQuorum,
    Path,
    NameDecode,
    TlvEq,
    TlvLen,
    PointerToNorito,
    JsonObject,
    JsonSetInt,
    JsonSetAccountId,
    EncodeInt,
    DecodeInt,
    EncodeJson,
    DecodeJson,
    JsonSetIntDirect,
    JsonSetAccountIdDirect,
    JsonGetIntDirect,
    JsonGetDecimalDirect,
    JsonGetQuantityDirect,
    JsonGetJsonDirect,
    JsonGetNameDirect,
    JsonGetAccountIdDirect,
    JsonGetAssetDefinitionIdDirect,
    JsonGetNftIdDirect,
    JsonGetBlobHexDirect,
    BuildPathKeyNoritoDirect,
    SchemaEncode,
    SchemaDecode,
    SchemaInfo,
    SchemaEncodeDirect,
    SchemaDecodeDirect,
    SchemaInfoDirect,
    NumericToInt,
    NumericNeg,
    NumericAdd,
    NumericSub,
    NumericMul,
    NumericDiv,
    NumericRem,
    NumericEq,
    NumericNe,
    NumericLt,
    NumericLe,
    NumericGt,
    NumericGe,
    NumericToIntDirect,
    NumericAddDirect,
    NumericSubDirect,
    NumericMulDirect,
    NumericDivDirect,
    NumericRemDirect,
    NumericNegDirect,
    NumericEqDirect,
    NumericNeDirect,
    NumericLtDirect,
    NumericLeDirect,
    NumericGtDirect,
    NumericGeDirect,
    /// Explicit modulo-2^512 `int` addition.
    WrappingAdd,
    /// Explicit modulo-2^512 `int` subtraction.
    WrappingSub,
    /// Explicit modulo-2^512 `int` multiplication.
    WrappingMul,
    /// Explicit modulo-2^512 `int` negation.
    WrappingNeg,
    Isqrt,
    Abs,
    Min,
    Max,
    DivCeil,
    Gcd,
    Mean,
    Poseidon2,
    Poseidon6,
    Pubkgen,
    Valcom,
    SetVl,
    GetInt,
    GetDecimal,
    GetQuantity,
    GetJson,
    GetName,
    GetAccountId,
    GetAssetDefinitionId,
    GetNftId,
    GetBlobHex,
    TriggerEvent,
    Authority,
    ContractSubject,
    CurrentTimeMs,
    BlockHeight,
    BlockTimeMs,
    ChainId,
    ContractAddress,
    Entrypoint,
    SysvarAuthority,
}

impl Builtin {
    /// Every canonical builtin variant, used by fail-closed registry checks.
    pub const ALL: &'static [Self] = &[
        Self::PointerConstructor(PointerConstructor::AccountId),
        Self::PointerConstructor(PointerConstructor::AssetDefinition),
        Self::PointerConstructor(PointerConstructor::AssetId),
        Self::PointerConstructor(PointerConstructor::NftId),
        Self::PointerConstructor(PointerConstructor::Name),
        Self::PointerConstructor(PointerConstructor::Json),
        Self::PointerConstructor(PointerConstructor::Domain),
        Self::PointerConstructor(PointerConstructor::DomainId),
        Self::PointerConstructor(PointerConstructor::Blob),
        Self::PointerConstructor(PointerConstructor::NoritoBytes),
        Self::PointerConstructor(PointerConstructor::DataSpaceId),
        Self::PointerConstructor(PointerConstructor::AxtDescriptor),
        Self::PointerConstructor(PointerConstructor::AssetHandle),
        Self::PointerConstructor(PointerConstructor::ProofBlob),
        Self::PointerConstructor(PointerConstructor::SoracloudRequest),
        Self::PointerConstructor(PointerConstructor::SoracloudResponse),
        Self::Contains,
        Self::GetOrDefault,
        Self::GetOr,
        Self::Ensure,
        Self::StateMapRemove,
        Self::KeysTake2,
        Self::ValuesTake2,
        Self::KeysValuesTake2,
        Self::StateGet,
        Self::StateSet,
        Self::StateDel,
        Self::StateKeys,
        Self::StateHas,
        Self::StateLen,
        Self::StateCount,
        Self::QueryExecuteNorito,
        Self::QueryGetAccount,
        Self::QueryGetAsset,
        Self::QueryGetAssetDefinition,
        Self::QueryGetDomain,
        Self::QueryGetNft,
        Self::QueryPageAccounts,
        Self::QueryPageAssets,
        Self::QueryPageAssetDefinitions,
        Self::QueryPageDomains,
        Self::QueryPageNfts,
        Self::QueryGetParameter,
        Self::QueryGetContractManifest,
        Self::QueryGetContractInstance,
        Self::RecordSccpMessage,
        Self::ExecuteQuery,
        Self::ScExecuteSubmitBallot,
        Self::ScExecuteUnshield,
        Self::ResolveAccountAlias,
        Self::SubscriptionBill,
        Self::SubscriptionRecordUsage,
        Self::GetAccountBalance,
        Self::GetPublicInput,
        Self::DebugPrint,
        Self::DebugLog,
        Self::Assert,
        Self::Require,
        Self::Info,
        Self::AssertEq,
        Self::TestInvokeEntrypoint,
        Self::TestInvokeEntrypointAs,
        Self::TestExpectRejectAs,
        Self::TestActorAccount,
        Self::TestActorPublicKey,
        Self::TestActorSign,
        Self::SetAccountDetail,
        Self::MintAsset,
        Self::BurnAsset,
        Self::TransferAsset,
        Self::SetAssetTransferFreeze,
        Self::SetAssetTransferDailyLimit,
        Self::AccountRecoveryPropose,
        Self::AccountRecoveryApprove,
        Self::AccountRecoveryCancel,
        Self::AccountRecoveryFinalize,
        Self::NftMintAsset,
        Self::NftSetMetadata,
        Self::NftBurnAsset,
        Self::NftTransferAsset,
        Self::RegisterDomain,
        Self::UnregisterDomain,
        Self::TransferDomain,
        Self::RegisterAccount,
        Self::UnregisterAccount,
        Self::RegisterAsset,
        Self::CreateNewAsset,
        Self::UnregisterAsset,
        Self::RegisterPeer,
        Self::UnregisterPeer,
        Self::CreateTrigger,
        Self::RegisterTrigger,
        Self::RemoveTrigger,
        Self::UnregisterTrigger,
        Self::SetTriggerEnabled,
        Self::CreateRole,
        Self::DeleteRole,
        Self::GrantRole,
        Self::RevokeRole,
        Self::GrantPermission,
        Self::RevokePermission,
        Self::GrantContractEntrypoint,
        Self::RevokeContractEntrypoint,
        Self::EscrowOpenOffer,
        Self::EscrowAccept,
        Self::EscrowMarkPaymentSent,
        Self::EscrowRelease,
        Self::EscrowCancel,
        Self::EscrowOpenDispute,
        Self::EscrowResolveDispute,
        Self::AnonymousEscrowOpenOffer,
        Self::AnonymousEscrowAccept,
        Self::AnonymousEscrowMarkPaymentSent,
        Self::AnonymousEscrowRelease,
        Self::AnonymousEscrowCancel,
        Self::AnonymousEscrowOpenDispute,
        Self::AnonymousEscrowResolveDispute,
        Self::GetPrivateInput,
        Self::UseNullifier,
        Self::CommitOutput,
        Self::CreateNftsForAllUsers,
        Self::SetExecutionDepth,
        Self::TransferV1BatchBegin,
        Self::TransferV1BatchEnd,
        Self::TransferV1BatchApply,
        Self::TransferBatch,
        Self::AxtBegin,
        Self::AxtTouch,
        Self::VerifyDsProof,
        Self::UseAssetHandle,
        Self::AxtCommit,
        Self::DeactivateContractInstance,
        Self::RemoveSmartContractBytes,
        Self::RegisterSmartContractCode,
        Self::RegisterSmartContractBytes,
        Self::ActivateContractInstance,
        Self::ZkRootsGet,
        Self::ZkVoteGetTally,
        Self::ZkVerifyTransfer,
        Self::ZkVerifyUnshield,
        Self::ZkVerifyBatch,
        Self::ZkVoteVerifyBallot,
        Self::ZkVoteVerifyTally,
        Self::BuildSubmitBallotInline,
        Self::BuildUnshieldInline,
        Self::VrfEpochSeed,
        Self::VrfVerify,
        Self::VrfVerifyBatch,
        Self::Sm3Hash,
        Self::Sha256Hash,
        Self::Sha3Hash,
        Self::Blake2b256Hash,
        Self::Keccak256Hash,
        Self::IrohaHash,
        Self::Sm2Verify,
        Self::VerifySignature,
        Self::Sm4GcmSeal,
        Self::Sm4GcmOpen,
        Self::Sm4CcmSeal,
        Self::Sm4CcmOpen,
        Self::Alloc,
        Self::ProveExecution,
        Self::GrowHeap,
        Self::VerifyProof,
        Self::GetMerklePath,
        Self::GetMerkleCompact,
        Self::GetRegisterMerkleCompact,
        Self::SoracloudReadCommittedState,
        Self::SoracloudEmitStateMutation,
        Self::SoracloudEmitMailboxMessage,
        Self::SoracloudAppendJournal,
        Self::SoracloudPublishCheckpoint,
        Self::SoracloudReadSecret,
        Self::SoracloudReadCredential,
        Self::SoracloudEgressFetch,
        Self::SoracloudReadConfig,
        Self::SoracloudReadSecretEnvelope,
        Self::AddSignatory,
        Self::RemoveSignatory,
        Self::SetAccountQuorum,
        Self::Path,
        Self::NameDecode,
        Self::TlvEq,
        Self::TlvLen,
        Self::PointerToNorito,
        Self::JsonObject,
        Self::JsonSetInt,
        Self::JsonSetAccountId,
        Self::EncodeInt,
        Self::DecodeInt,
        Self::EncodeJson,
        Self::DecodeJson,
        Self::JsonSetIntDirect,
        Self::JsonSetAccountIdDirect,
        Self::JsonGetIntDirect,
        Self::JsonGetDecimalDirect,
        Self::JsonGetQuantityDirect,
        Self::JsonGetJsonDirect,
        Self::JsonGetNameDirect,
        Self::JsonGetAccountIdDirect,
        Self::JsonGetAssetDefinitionIdDirect,
        Self::JsonGetNftIdDirect,
        Self::JsonGetBlobHexDirect,
        Self::BuildPathKeyNoritoDirect,
        Self::SchemaEncode,
        Self::SchemaDecode,
        Self::SchemaInfo,
        Self::SchemaEncodeDirect,
        Self::SchemaDecodeDirect,
        Self::SchemaInfoDirect,
        Self::WrappingAdd,
        Self::WrappingSub,
        Self::WrappingMul,
        Self::WrappingNeg,
        Self::Isqrt,
        Self::Abs,
        Self::Min,
        Self::Max,
        Self::DivCeil,
        Self::Gcd,
        Self::Mean,
        Self::Poseidon2,
        Self::Poseidon6,
        Self::Pubkgen,
        Self::Valcom,
        Self::SetVl,
        Self::GetInt,
        Self::GetDecimal,
        Self::GetQuantity,
        Self::GetJson,
        Self::GetName,
        Self::GetAccountId,
        Self::GetAssetDefinitionId,
        Self::GetNftId,
        Self::GetBlobHex,
        Self::TriggerEvent,
        Self::Authority,
        Self::ContractSubject,
        Self::CurrentTimeMs,
        Self::BlockHeight,
        Self::BlockTimeMs,
        Self::ChainId,
        Self::ContractAddress,
        Self::Entrypoint,
        Self::SysvarAuthority,
    ];

    /// Resolve a builtin from its canonical compiler-internal spelling.
    ///
    /// Source resolution must use [`Self::from_source_name`] so an internal
    /// lowering name cannot accidentally become a public language feature.
    pub fn from_name(name: &str) -> Option<Self> {
        if let Some(constructor) = PointerConstructor::from_name(name) {
            return Some(Self::PointerConstructor(constructor));
        }
        Some(match name {
            "contains" => Self::Contains,
            "get_or_default" => Self::GetOrDefault,
            "get_or" => Self::GetOr,
            "ensure" => Self::Ensure,
            "remove" => Self::StateMapRemove,
            "keys_take2" => Self::KeysTake2,
            "values_take2" => Self::ValuesTake2,
            "keys_values_take2" => Self::KeysValuesTake2,
            "state_get" => Self::StateGet,
            "state_set" => Self::StateSet,
            "state_del" => Self::StateDel,
            "state_keys" => Self::StateKeys,
            "state_has" => Self::StateHas,
            "state_len" => Self::StateLen,
            "state_count" => Self::StateCount,
            "query_execute_norito" => Self::QueryExecuteNorito,
            "query_get_account" => Self::QueryGetAccount,
            "query_get_asset" => Self::QueryGetAsset,
            "query_get_asset_definition" => Self::QueryGetAssetDefinition,
            "query_get_domain" => Self::QueryGetDomain,
            "query_get_nft" => Self::QueryGetNft,
            "query_page_accounts" => Self::QueryPageAccounts,
            "query_page_assets" => Self::QueryPageAssets,
            "query_page_asset_definitions" => Self::QueryPageAssetDefinitions,
            "query_page_domains" => Self::QueryPageDomains,
            "query_page_nfts" => Self::QueryPageNfts,
            "query_get_parameter" => Self::QueryGetParameter,
            "query_get_contract_manifest" => Self::QueryGetContractManifest,
            "query_get_contract_instance" => Self::QueryGetContractInstance,
            "record_sccp_message" => Self::RecordSccpMessage,
            "execute_query" => Self::ExecuteQuery,
            "sc_execute_submit_ballot" => Self::ScExecuteSubmitBallot,
            "sc_execute_unshield" => Self::ScExecuteUnshield,
            "resolve_account_alias" => Self::ResolveAccountAlias,
            "subscription_bill" => Self::SubscriptionBill,
            "subscription_record_usage" => Self::SubscriptionRecordUsage,
            "get_account_balance" => Self::GetAccountBalance,
            "get_public_input" => Self::GetPublicInput,
            "debug_print" => Self::DebugPrint,
            "debug_log" => Self::DebugLog,
            "assert" => Self::Assert,
            "require" => Self::Require,
            "info" => Self::Info,
            "assert_eq" => Self::AssertEq,
            "invoke_entrypoint" => Self::TestInvokeEntrypoint,
            "invoke_entrypoint_as" => Self::TestInvokeEntrypointAs,
            "expect_reject_as" => Self::TestExpectRejectAs,
            "actor_account" => Self::TestActorAccount,
            "actor_public_key" => Self::TestActorPublicKey,
            "actor_sign" => Self::TestActorSign,
            "set_account_detail" => Self::SetAccountDetail,
            "mint_asset" => Self::MintAsset,
            "burn_asset" => Self::BurnAsset,
            "transfer_asset" => Self::TransferAsset,
            "set_asset_transfer_freeze" => Self::SetAssetTransferFreeze,
            "set_asset_transfer_daily_limit" => Self::SetAssetTransferDailyLimit,
            "account_recovery_propose" => Self::AccountRecoveryPropose,
            "account_recovery_approve" => Self::AccountRecoveryApprove,
            "account_recovery_cancel" => Self::AccountRecoveryCancel,
            "account_recovery_finalize" => Self::AccountRecoveryFinalize,
            "nft_mint_asset" => Self::NftMintAsset,
            "nft_set_metadata" => Self::NftSetMetadata,
            "nft_burn_asset" => Self::NftBurnAsset,
            "nft_transfer_asset" => Self::NftTransferAsset,
            "register_domain" => Self::RegisterDomain,
            "unregister_domain" => Self::UnregisterDomain,
            "transfer_domain" => Self::TransferDomain,
            "register_account" => Self::RegisterAccount,
            "unregister_account" => Self::UnregisterAccount,
            "register_asset" => Self::RegisterAsset,
            "create_new_asset" => Self::CreateNewAsset,
            "unregister_asset" => Self::UnregisterAsset,
            "register_peer" => Self::RegisterPeer,
            "unregister_peer" => Self::UnregisterPeer,
            "create_trigger" => Self::CreateTrigger,
            "register_trigger" => Self::RegisterTrigger,
            "remove_trigger" => Self::RemoveTrigger,
            "unregister_trigger" => Self::UnregisterTrigger,
            "set_trigger_enabled" => Self::SetTriggerEnabled,
            "create_role" => Self::CreateRole,
            "delete_role" => Self::DeleteRole,
            "grant_role" => Self::GrantRole,
            "revoke_role" => Self::RevokeRole,
            "grant_permission" => Self::GrantPermission,
            "revoke_permission" => Self::RevokePermission,
            "grant_contract_entrypoint" => Self::GrantContractEntrypoint,
            "revoke_contract_entrypoint" => Self::RevokeContractEntrypoint,
            "escrow_open_offer" => Self::EscrowOpenOffer,
            "escrow_accept" => Self::EscrowAccept,
            "escrow_mark_payment_sent" => Self::EscrowMarkPaymentSent,
            "escrow_release" => Self::EscrowRelease,
            "escrow_cancel" => Self::EscrowCancel,
            "escrow_open_dispute" => Self::EscrowOpenDispute,
            "escrow_resolve_dispute" => Self::EscrowResolveDispute,
            "anonymous_escrow_open_offer" => Self::AnonymousEscrowOpenOffer,
            "anonymous_escrow_accept" => Self::AnonymousEscrowAccept,
            "anonymous_escrow_mark_payment_sent" => Self::AnonymousEscrowMarkPaymentSent,
            "anonymous_escrow_release" => Self::AnonymousEscrowRelease,
            "anonymous_escrow_cancel" => Self::AnonymousEscrowCancel,
            "anonymous_escrow_open_dispute" => Self::AnonymousEscrowOpenDispute,
            "anonymous_escrow_resolve_dispute" => Self::AnonymousEscrowResolveDispute,
            "get_private_input" => Self::GetPrivateInput,
            "use_nullifier" => Self::UseNullifier,
            "commit_output" => Self::CommitOutput,
            "create_nfts_for_all_users" => Self::CreateNftsForAllUsers,
            "set_execution_depth" => Self::SetExecutionDepth,
            "transfer_v1_batch_begin" => Self::TransferV1BatchBegin,
            "transfer_v1_batch_end" => Self::TransferV1BatchEnd,
            "transfer_v1_batch_apply" => Self::TransferV1BatchApply,
            "transfer_batch" => Self::TransferBatch,
            "axt_begin" => Self::AxtBegin,
            "axt_touch" => Self::AxtTouch,
            "verify_ds_proof" => Self::VerifyDsProof,
            "use_asset_handle" => Self::UseAssetHandle,
            "axt_commit" => Self::AxtCommit,
            "deactivate_contract_instance" => Self::DeactivateContractInstance,
            "remove_smart_contract_bytes" => Self::RemoveSmartContractBytes,
            "register_smart_contract_code" => Self::RegisterSmartContractCode,
            "register_smart_contract_bytes" => Self::RegisterSmartContractBytes,
            "activate_contract_instance" => Self::ActivateContractInstance,
            "zk_roots_get" => Self::ZkRootsGet,
            "zk_vote_get_tally" => Self::ZkVoteGetTally,
            "zk_verify_transfer" => Self::ZkVerifyTransfer,
            "zk_verify_unshield" => Self::ZkVerifyUnshield,
            "zk_verify_batch" => Self::ZkVerifyBatch,
            "zk_vote_verify_ballot" => Self::ZkVoteVerifyBallot,
            "zk_vote_verify_tally" => Self::ZkVoteVerifyTally,
            "build_submit_ballot_inline" => Self::BuildSubmitBallotInline,
            "build_unshield_inline" => Self::BuildUnshieldInline,
            "vrf_epoch_seed" => Self::VrfEpochSeed,
            "vrf_verify" => Self::VrfVerify,
            "vrf_verify_batch" => Self::VrfVerifyBatch,
            "sm3_hash" => Self::Sm3Hash,
            "sha256_hash" => Self::Sha256Hash,
            "sha3_hash" => Self::Sha3Hash,
            "blake2b256_hash" => Self::Blake2b256Hash,
            "keccak256_hash" => Self::Keccak256Hash,
            "iroha_hash" => Self::IrohaHash,
            "sm2_verify" => Self::Sm2Verify,
            "verify_signature" => Self::VerifySignature,
            "sm4_gcm_seal" => Self::Sm4GcmSeal,
            "sm4_gcm_open" => Self::Sm4GcmOpen,
            "sm4_ccm_seal" => Self::Sm4CcmSeal,
            "sm4_ccm_open" => Self::Sm4CcmOpen,
            "alloc" => Self::Alloc,
            "prove_execution" => Self::ProveExecution,
            "grow_heap" => Self::GrowHeap,
            "verify_proof" => Self::VerifyProof,
            "get_merkle_path" => Self::GetMerklePath,
            "get_merkle_compact" => Self::GetMerkleCompact,
            "get_register_merkle_compact" => Self::GetRegisterMerkleCompact,
            "soracloud_read_committed_state" => Self::SoracloudReadCommittedState,
            "soracloud_emit_state_mutation" => Self::SoracloudEmitStateMutation,
            "soracloud_emit_mailbox_message" => Self::SoracloudEmitMailboxMessage,
            "soracloud_append_journal" => Self::SoracloudAppendJournal,
            "soracloud_publish_checkpoint" => Self::SoracloudPublishCheckpoint,
            "soracloud_read_secret" => Self::SoracloudReadSecret,
            "soracloud_read_credential" => Self::SoracloudReadCredential,
            "soracloud_egress_fetch" => Self::SoracloudEgressFetch,
            "soracloud_read_config" => Self::SoracloudReadConfig,
            "soracloud_read_secret_envelope" => Self::SoracloudReadSecretEnvelope,
            "add_signatory" => Self::AddSignatory,
            "remove_signatory" => Self::RemoveSignatory,
            "set_account_quorum" => Self::SetAccountQuorum,
            "path" => Self::Path,
            "name_decode" => Self::NameDecode,
            "tlv_eq" => Self::TlvEq,
            "tlv_len" => Self::TlvLen,
            "pointer_to_norito" => Self::PointerToNorito,
            "json_object" => Self::JsonObject,
            "json_set_int" => Self::JsonSetInt,
            "json_set_account_id" => Self::JsonSetAccountId,
            "encode_int" => Self::EncodeInt,
            "decode_int" => Self::DecodeInt,
            "encode_json" => Self::EncodeJson,
            "decode_json" => Self::DecodeJson,
            "json_set_int_direct" => Self::JsonSetIntDirect,
            "json_set_account_id_direct" => Self::JsonSetAccountIdDirect,
            "json_get_int_direct" => Self::JsonGetIntDirect,
            "json_get_decimal_direct" => Self::JsonGetDecimalDirect,
            "json_get_quantity_direct" => Self::JsonGetQuantityDirect,
            "json_get_json_direct" => Self::JsonGetJsonDirect,
            "json_get_name_direct" => Self::JsonGetNameDirect,
            "json_get_account_id_direct" => Self::JsonGetAccountIdDirect,
            "json_get_asset_definition_id_direct" => Self::JsonGetAssetDefinitionIdDirect,
            "json_get_nft_id_direct" => Self::JsonGetNftIdDirect,
            "json_get_blob_hex_direct" => Self::JsonGetBlobHexDirect,
            "build_path_key_norito_direct" => Self::BuildPathKeyNoritoDirect,
            "encode_schema" => Self::SchemaEncode,
            "decode_schema" => Self::SchemaDecode,
            "schema_info" => Self::SchemaInfo,
            "schema_encode_direct" => Self::SchemaEncodeDirect,
            "schema_decode_direct" => Self::SchemaDecodeDirect,
            "schema_info_direct" => Self::SchemaInfoDirect,
            "wrapping_add" => Self::WrappingAdd,
            "wrapping_sub" => Self::WrappingSub,
            "wrapping_mul" => Self::WrappingMul,
            "wrapping_neg" => Self::WrappingNeg,
            "isqrt" => Self::Isqrt,
            "abs" => Self::Abs,
            "min" => Self::Min,
            "max" => Self::Max,
            "div_ceil" => Self::DivCeil,
            "gcd" => Self::Gcd,
            "mean" => Self::Mean,
            "poseidon2" => Self::Poseidon2,
            "poseidon6" => Self::Poseidon6,
            "pubkgen" => Self::Pubkgen,
            "valcom" => Self::Valcom,
            "setvl" => Self::SetVl,
            "get_int" => Self::GetInt,
            "get_decimal" => Self::GetDecimal,
            "get_quantity" => Self::GetQuantity,
            "get_json" => Self::GetJson,
            "get_name" => Self::GetName,
            "get_account_id" => Self::GetAccountId,
            "get_asset_definition_id" => Self::GetAssetDefinitionId,
            "get_nft_id" => Self::GetNftId,
            "get_blob_hex" => Self::GetBlobHex,
            "trigger_event" => Self::TriggerEvent,
            "authority" => Self::Authority,
            "contract_subject" => Self::ContractSubject,
            "current_time_ms" => Self::CurrentTimeMs,
            "block_height" => Self::BlockHeight,
            "block_time_ms" => Self::BlockTimeMs,
            "chain_id" => Self::ChainId,
            "contract_address" => Self::ContractAddress,
            "entrypoint" => Self::Entrypoint,
            "sysvar_authority" => Self::SysvarAuthority,
            _ => return None,
        })
    }

    /// The canonical compiler-internal spelling used by typed HIR and lowering.
    pub const fn name(self) -> &'static str {
        match self {
            Self::PointerConstructor(constructor) => constructor.name(),
            Self::Contains => "contains",
            Self::GetOrDefault => "get_or_default",
            Self::GetOr => "get_or",
            Self::Ensure => "ensure",
            Self::StateMapRemove => "remove",
            Self::KeysTake2 => "keys_take2",
            Self::ValuesTake2 => "values_take2",
            Self::KeysValuesTake2 => "keys_values_take2",
            Self::StateGet => "state_get",
            Self::StateSet => "state_set",
            Self::StateDel => "state_del",
            Self::StateKeys => "state_keys",
            Self::StateHas => "state_has",
            Self::StateLen => "state_len",
            Self::StateCount => "state_count",
            Self::QueryExecuteNorito => "query_execute_norito",
            Self::QueryGetAccount => "query_get_account",
            Self::QueryGetAsset => "query_get_asset",
            Self::QueryGetAssetDefinition => "query_get_asset_definition",
            Self::QueryGetDomain => "query_get_domain",
            Self::QueryGetNft => "query_get_nft",
            Self::QueryPageAccounts => "query_page_accounts",
            Self::QueryPageAssets => "query_page_assets",
            Self::QueryPageAssetDefinitions => "query_page_asset_definitions",
            Self::QueryPageDomains => "query_page_domains",
            Self::QueryPageNfts => "query_page_nfts",
            Self::QueryGetParameter => "query_get_parameter",
            Self::QueryGetContractManifest => "query_get_contract_manifest",
            Self::QueryGetContractInstance => "query_get_contract_instance",
            Self::RecordSccpMessage => "record_sccp_message",
            Self::ExecuteQuery => "execute_query",
            Self::ScExecuteSubmitBallot => "sc_execute_submit_ballot",
            Self::ScExecuteUnshield => "sc_execute_unshield",
            Self::ResolveAccountAlias => "resolve_account_alias",
            Self::SubscriptionBill => "subscription_bill",
            Self::SubscriptionRecordUsage => "subscription_record_usage",
            Self::GetAccountBalance => "get_account_balance",
            Self::GetPublicInput => "get_public_input",
            Self::DebugPrint => "debug_print",
            Self::DebugLog => "debug_log",
            Self::Assert => "assert",
            Self::Require => "require",
            Self::Info => "info",
            Self::AssertEq => "assert_eq",
            Self::TestInvokeEntrypoint => "invoke_entrypoint",
            Self::TestInvokeEntrypointAs => "invoke_entrypoint_as",
            Self::TestExpectRejectAs => "expect_reject_as",
            Self::TestActorAccount => "actor_account",
            Self::TestActorPublicKey => "actor_public_key",
            Self::TestActorSign => "actor_sign",
            Self::SetAccountDetail => "set_account_detail",
            Self::MintAsset => "mint_asset",
            Self::BurnAsset => "burn_asset",
            Self::TransferAsset => "transfer_asset",
            Self::SetAssetTransferFreeze => "set_asset_transfer_freeze",
            Self::SetAssetTransferDailyLimit => "set_asset_transfer_daily_limit",
            Self::AccountRecoveryPropose => "account_recovery_propose",
            Self::AccountRecoveryApprove => "account_recovery_approve",
            Self::AccountRecoveryCancel => "account_recovery_cancel",
            Self::AccountRecoveryFinalize => "account_recovery_finalize",
            Self::NftMintAsset => "nft_mint_asset",
            Self::NftSetMetadata => "nft_set_metadata",
            Self::NftBurnAsset => "nft_burn_asset",
            Self::NftTransferAsset => "nft_transfer_asset",
            Self::RegisterDomain => "register_domain",
            Self::UnregisterDomain => "unregister_domain",
            Self::TransferDomain => "transfer_domain",
            Self::RegisterAccount => "register_account",
            Self::UnregisterAccount => "unregister_account",
            Self::RegisterAsset => "register_asset",
            Self::CreateNewAsset => "create_new_asset",
            Self::UnregisterAsset => "unregister_asset",
            Self::RegisterPeer => "register_peer",
            Self::UnregisterPeer => "unregister_peer",
            Self::CreateTrigger => "create_trigger",
            Self::RegisterTrigger => "register_trigger",
            Self::RemoveTrigger => "remove_trigger",
            Self::UnregisterTrigger => "unregister_trigger",
            Self::SetTriggerEnabled => "set_trigger_enabled",
            Self::CreateRole => "create_role",
            Self::DeleteRole => "delete_role",
            Self::GrantRole => "grant_role",
            Self::RevokeRole => "revoke_role",
            Self::GrantPermission => "grant_permission",
            Self::RevokePermission => "revoke_permission",
            Self::GrantContractEntrypoint => "grant_contract_entrypoint",
            Self::RevokeContractEntrypoint => "revoke_contract_entrypoint",
            Self::EscrowOpenOffer => "escrow_open_offer",
            Self::EscrowAccept => "escrow_accept",
            Self::EscrowMarkPaymentSent => "escrow_mark_payment_sent",
            Self::EscrowRelease => "escrow_release",
            Self::EscrowCancel => "escrow_cancel",
            Self::EscrowOpenDispute => "escrow_open_dispute",
            Self::EscrowResolveDispute => "escrow_resolve_dispute",
            Self::AnonymousEscrowOpenOffer => "anonymous_escrow_open_offer",
            Self::AnonymousEscrowAccept => "anonymous_escrow_accept",
            Self::AnonymousEscrowMarkPaymentSent => "anonymous_escrow_mark_payment_sent",
            Self::AnonymousEscrowRelease => "anonymous_escrow_release",
            Self::AnonymousEscrowCancel => "anonymous_escrow_cancel",
            Self::AnonymousEscrowOpenDispute => "anonymous_escrow_open_dispute",
            Self::AnonymousEscrowResolveDispute => "anonymous_escrow_resolve_dispute",
            Self::GetPrivateInput => "get_private_input",
            Self::UseNullifier => "use_nullifier",
            Self::CommitOutput => "commit_output",
            Self::CreateNftsForAllUsers => "create_nfts_for_all_users",
            Self::SetExecutionDepth => "set_execution_depth",
            Self::TransferV1BatchBegin => "transfer_v1_batch_begin",
            Self::TransferV1BatchEnd => "transfer_v1_batch_end",
            Self::TransferV1BatchApply => "transfer_v1_batch_apply",
            Self::TransferBatch => "transfer_batch",
            Self::AxtBegin => "axt_begin",
            Self::AxtTouch => "axt_touch",
            Self::VerifyDsProof => "verify_ds_proof",
            Self::UseAssetHandle => "use_asset_handle",
            Self::AxtCommit => "axt_commit",
            Self::DeactivateContractInstance => "deactivate_contract_instance",
            Self::RemoveSmartContractBytes => "remove_smart_contract_bytes",
            Self::RegisterSmartContractCode => "register_smart_contract_code",
            Self::RegisterSmartContractBytes => "register_smart_contract_bytes",
            Self::ActivateContractInstance => "activate_contract_instance",
            Self::ZkRootsGet => "zk_roots_get",
            Self::ZkVoteGetTally => "zk_vote_get_tally",
            Self::ZkVerifyTransfer => "zk_verify_transfer",
            Self::ZkVerifyUnshield => "zk_verify_unshield",
            Self::ZkVerifyBatch => "zk_verify_batch",
            Self::ZkVoteVerifyBallot => "zk_vote_verify_ballot",
            Self::ZkVoteVerifyTally => "zk_vote_verify_tally",
            Self::BuildSubmitBallotInline => "build_submit_ballot_inline",
            Self::BuildUnshieldInline => "build_unshield_inline",
            Self::VrfEpochSeed => "vrf_epoch_seed",
            Self::VrfVerify => "vrf_verify",
            Self::VrfVerifyBatch => "vrf_verify_batch",
            Self::Sm3Hash => "sm3_hash",
            Self::Sha256Hash => "sha256_hash",
            Self::Sha3Hash => "sha3_hash",
            Self::Blake2b256Hash => "blake2b256_hash",
            Self::Keccak256Hash => "keccak256_hash",
            Self::IrohaHash => "iroha_hash",
            Self::Sm2Verify => "sm2_verify",
            Self::VerifySignature => "verify_signature",
            Self::Sm4GcmSeal => "sm4_gcm_seal",
            Self::Sm4GcmOpen => "sm4_gcm_open",
            Self::Sm4CcmSeal => "sm4_ccm_seal",
            Self::Sm4CcmOpen => "sm4_ccm_open",
            Self::Alloc => "alloc",
            Self::ProveExecution => "prove_execution",
            Self::GrowHeap => "grow_heap",
            Self::VerifyProof => "verify_proof",
            Self::GetMerklePath => "get_merkle_path",
            Self::GetMerkleCompact => "get_merkle_compact",
            Self::GetRegisterMerkleCompact => "get_register_merkle_compact",
            Self::SoracloudReadCommittedState => "soracloud_read_committed_state",
            Self::SoracloudEmitStateMutation => "soracloud_emit_state_mutation",
            Self::SoracloudEmitMailboxMessage => "soracloud_emit_mailbox_message",
            Self::SoracloudAppendJournal => "soracloud_append_journal",
            Self::SoracloudPublishCheckpoint => "soracloud_publish_checkpoint",
            Self::SoracloudReadSecret => "soracloud_read_secret",
            Self::SoracloudReadCredential => "soracloud_read_credential",
            Self::SoracloudEgressFetch => "soracloud_egress_fetch",
            Self::SoracloudReadConfig => "soracloud_read_config",
            Self::SoracloudReadSecretEnvelope => "soracloud_read_secret_envelope",
            Self::AddSignatory => "add_signatory",
            Self::RemoveSignatory => "remove_signatory",
            Self::SetAccountQuorum => "set_account_quorum",
            Self::Path => "path",
            Self::NameDecode => "name_decode",
            Self::TlvEq => "tlv_eq",
            Self::TlvLen => "tlv_len",
            Self::PointerToNorito => "pointer_to_norito",
            Self::JsonObject => "json_object",
            Self::JsonSetInt => "json_set_int",
            Self::JsonSetAccountId => "json_set_account_id",
            Self::EncodeInt => "encode_int",
            Self::DecodeInt => "decode_int",
            Self::EncodeJson => "encode_json",
            Self::DecodeJson => "decode_json",
            Self::JsonSetIntDirect => "json_set_int_direct",
            Self::JsonSetAccountIdDirect => "json_set_account_id_direct",
            Self::JsonGetIntDirect => "json_get_int_direct",
            Self::JsonGetDecimalDirect => "json_get_decimal_direct",
            Self::JsonGetQuantityDirect => "json_get_quantity_direct",
            Self::JsonGetJsonDirect => "json_get_json_direct",
            Self::JsonGetNameDirect => "json_get_name_direct",
            Self::JsonGetAccountIdDirect => "json_get_account_id_direct",
            Self::JsonGetAssetDefinitionIdDirect => "json_get_asset_definition_id_direct",
            Self::JsonGetNftIdDirect => "json_get_nft_id_direct",
            Self::JsonGetBlobHexDirect => "json_get_blob_hex_direct",
            Self::BuildPathKeyNoritoDirect => "build_path_key_norito_direct",
            Self::SchemaEncode => "encode_schema",
            Self::SchemaDecode => "decode_schema",
            Self::SchemaInfo => "schema_info",
            Self::SchemaEncodeDirect => "schema_encode_direct",
            Self::SchemaDecodeDirect => "schema_decode_direct",
            Self::SchemaInfoDirect => "schema_info_direct",
            Self::NumericToInt => "numeric_to_int",
            Self::NumericNeg => "numeric_neg",
            Self::NumericAdd => "numeric_add",
            Self::NumericSub => "numeric_sub",
            Self::NumericMul => "numeric_mul",
            Self::NumericDiv => "numeric_div",
            Self::NumericRem => "numeric_rem",
            Self::NumericEq => "numeric_eq",
            Self::NumericNe => "numeric_ne",
            Self::NumericLt => "numeric_lt",
            Self::NumericLe => "numeric_le",
            Self::NumericGt => "numeric_gt",
            Self::NumericGe => "numeric_ge",
            Self::NumericToIntDirect => "numeric_to_int_direct",
            Self::NumericAddDirect => "numeric_add_direct",
            Self::NumericSubDirect => "numeric_sub_direct",
            Self::NumericMulDirect => "numeric_mul_direct",
            Self::NumericDivDirect => "numeric_div_direct",
            Self::NumericRemDirect => "numeric_rem_direct",
            Self::NumericNegDirect => "numeric_neg_direct",
            Self::NumericEqDirect => "numeric_eq_direct",
            Self::NumericNeDirect => "numeric_ne_direct",
            Self::NumericLtDirect => "numeric_lt_direct",
            Self::NumericLeDirect => "numeric_le_direct",
            Self::NumericGtDirect => "numeric_gt_direct",
            Self::NumericGeDirect => "numeric_ge_direct",
            Self::WrappingAdd => "wrapping_add",
            Self::WrappingSub => "wrapping_sub",
            Self::WrappingMul => "wrapping_mul",
            Self::WrappingNeg => "wrapping_neg",
            Self::Isqrt => "isqrt",
            Self::Abs => "abs",
            Self::Min => "min",
            Self::Max => "max",
            Self::DivCeil => "div_ceil",
            Self::Gcd => "gcd",
            Self::Mean => "mean",
            Self::Poseidon2 => "poseidon2",
            Self::Poseidon6 => "poseidon6",
            Self::Pubkgen => "pubkgen",
            Self::Valcom => "valcom",
            Self::SetVl => "setvl",
            Self::GetInt => "get_int",
            Self::GetDecimal => "get_decimal",
            Self::GetQuantity => "get_quantity",
            Self::GetJson => "get_json",
            Self::GetName => "get_name",
            Self::GetAccountId => "get_account_id",
            Self::GetAssetDefinitionId => "get_asset_definition_id",
            Self::GetNftId => "get_nft_id",
            Self::GetBlobHex => "get_blob_hex",
            Self::TriggerEvent => "trigger_event",
            Self::Authority => "authority",
            Self::ContractSubject => "contract_subject",
            Self::CurrentTimeMs => "current_time_ms",
            Self::BlockHeight => "block_height",
            Self::BlockTimeMs => "block_time_ms",
            Self::ChainId => "chain_id",
            Self::ContractAddress => "contract_address",
            Self::Entrypoint => "entrypoint",
            Self::SysvarAuthority => "sysvar_authority",
        }
    }

    /// Canonical V1 source spelling, including the public namespace.
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::PointerConstructor(constructor) => match constructor {
                PointerConstructor::AccountId => "AccountId::parse",
                PointerConstructor::AssetDefinition => "AssetDefinitionId::parse",
                PointerConstructor::AssetId => "AssetId::parse",
                PointerConstructor::NftId => "NftId::parse",
                PointerConstructor::Name => "Name::parse",
                PointerConstructor::Json => "Json::parse",
                PointerConstructor::DomainId => "DomainId::parse",
                PointerConstructor::DataSpaceId => "DataSpaceId::parse",
                // These constructors are compiler internals. Giving them an
                // internal spelling here does not make them source-visible;
                // `from_source_name` also enforces `BuiltinMode`.
                PointerConstructor::Domain
                | PointerConstructor::Blob
                | PointerConstructor::NoritoBytes
                | PointerConstructor::AxtDescriptor
                | PointerConstructor::AssetHandle
                | PointerConstructor::ProofBlob
                | PointerConstructor::SoracloudRequest
                | PointerConstructor::SoracloudResponse => constructor.name(),
            },
            Self::Contains => "contains",
            Self::GetOrDefault => "get_or_default",
            Self::GetOr => "get_or",
            Self::Ensure => "ensure",
            Self::StateMapRemove => "remove",
            Self::KeysTake2 => "state::keys_take2",
            Self::ValuesTake2 => "state::values_take2",
            Self::KeysValuesTake2 => "state::entries_take2",
            Self::Authority => "context::authority",
            Self::ContractSubject => "context::contract_subject",
            Self::CurrentTimeMs => "context::current_time_ms",
            Self::BlockHeight => "context::block_height",
            Self::BlockTimeMs => "context::block_time_ms",
            Self::ChainId => "context::chain_id",
            Self::ContractAddress => "context::contract_address",
            Self::Entrypoint => "context::entrypoint",
            Self::GetPublicInput => "context::public_input",
            Self::TriggerEvent => "context::trigger_event",
            Self::StateGet => "state::get",
            Self::StateSet => "state::set",
            Self::StateDel => "state::delete",
            Self::StateKeys => "state::keys",
            Self::StateHas => "state::contains",
            Self::StateLen => "state::len",
            Self::StateCount => "state::count",
            Self::QueryGetAccount => "ledger::query::account",
            Self::QueryGetAsset => "ledger::query::asset",
            Self::QueryGetAssetDefinition => "ledger::query::asset_definition",
            Self::QueryGetDomain => "ledger::query::domain",
            Self::QueryGetNft => "ledger::query::nft",
            Self::QueryPageAccounts => "ledger::query::accounts",
            Self::QueryPageAssets => "ledger::query::assets",
            Self::QueryPageAssetDefinitions => "ledger::query::asset_definitions",
            Self::QueryPageDomains => "ledger::query::domains",
            Self::QueryPageNfts => "ledger::query::nfts",
            Self::QueryGetParameter => "ledger::query::parameter",
            Self::QueryGetContractManifest => "ledger::query::contract_manifest",
            Self::QueryGetContractInstance => "ledger::query::contract_instance",
            Self::RecordSccpMessage => "ledger::sccp::record",
            Self::ResolveAccountAlias => "ledger::account::resolve_alias",
            Self::SubscriptionBill => "ledger::subscription::bill",
            Self::SubscriptionRecordUsage => "ledger::subscription::record_usage",
            Self::GetAccountBalance => "ledger::asset::balance",
            Self::DebugPrint => self.name(),
            Self::DebugLog => "debug::log",
            Self::Assert => "test::assert",
            Self::Require => "require",
            Self::Info => "debug::info",
            Self::AssertEq => "test::assert_eq",
            Self::TestInvokeEntrypoint => "test::invoke_entrypoint",
            Self::TestInvokeEntrypointAs => "test::invoke_entrypoint_as",
            Self::TestExpectRejectAs => "test::expect_reject_as",
            Self::TestActorAccount => "test::actor_account",
            Self::TestActorPublicKey => "test::actor_public_key",
            Self::TestActorSign => "test::actor_sign",
            Self::MintAsset => "ledger::asset::mint",
            Self::BurnAsset => "ledger::asset::burn",
            Self::TransferAsset => "ledger::asset::transfer",
            Self::SetAssetTransferFreeze => "ledger::asset::set_transfer_freeze",
            Self::SetAssetTransferDailyLimit => "ledger::asset::set_transfer_daily_limit",
            Self::RegisterAsset => "ledger::asset::register",
            Self::CreateNewAsset => "ledger::asset::create",
            Self::UnregisterAsset => "ledger::asset::unregister",
            Self::SetAccountDetail => "ledger::account::set_detail",
            Self::RegisterAccount => "ledger::account::register",
            Self::UnregisterAccount => "ledger::account::unregister",
            Self::AddSignatory => "ledger::account::add_signatory",
            Self::RemoveSignatory => "ledger::account::remove_signatory",
            Self::SetAccountQuorum => "ledger::account::set_quorum",
            Self::AccountRecoveryPropose => "ledger::account::recovery::propose",
            Self::AccountRecoveryApprove => "ledger::account::recovery::approve",
            Self::AccountRecoveryCancel => "ledger::account::recovery::cancel",
            Self::AccountRecoveryFinalize => "ledger::account::recovery::finalize",
            Self::NftMintAsset => "ledger::nft::mint",
            Self::NftSetMetadata => "ledger::nft::set_metadata",
            Self::NftBurnAsset => "ledger::nft::burn",
            Self::NftTransferAsset => "ledger::nft::transfer",
            Self::CreateNftsForAllUsers => "ledger::nft::create_for_all_users",
            Self::RegisterDomain => "ledger::domain::register",
            Self::UnregisterDomain => "ledger::domain::unregister",
            Self::TransferDomain => "ledger::domain::transfer",
            Self::RegisterPeer => "ledger::peer::register",
            Self::UnregisterPeer => "ledger::peer::unregister",
            Self::CreateTrigger => "ledger::trigger::create",
            Self::RegisterTrigger => "ledger::trigger::register",
            Self::RemoveTrigger => "ledger::trigger::remove",
            Self::UnregisterTrigger => "ledger::trigger::unregister",
            Self::SetTriggerEnabled => "ledger::trigger::set_enabled",
            Self::CreateRole => "ledger::role::create",
            Self::DeleteRole => "ledger::role::delete",
            Self::GrantRole => "ledger::role::grant",
            Self::RevokeRole => "ledger::role::revoke",
            Self::GrantPermission => "ledger::permission::grant",
            Self::RevokePermission => "ledger::permission::revoke",
            Self::GrantContractEntrypoint => "ledger::contract::grant_entrypoint",
            Self::RevokeContractEntrypoint => "ledger::contract::revoke_entrypoint",
            Self::EscrowOpenOffer => "ledger::escrow::open_offer",
            Self::EscrowAccept => "ledger::escrow::accept",
            Self::EscrowMarkPaymentSent => "ledger::escrow::mark_payment_sent",
            Self::EscrowRelease => "ledger::escrow::release",
            Self::EscrowCancel => "ledger::escrow::cancel",
            Self::EscrowOpenDispute => "ledger::escrow::open_dispute",
            Self::EscrowResolveDispute => "ledger::escrow::resolve_dispute",
            Self::AnonymousEscrowOpenOffer => "ledger::escrow::anonymous::open_offer",
            Self::AnonymousEscrowAccept => "ledger::escrow::anonymous::accept",
            Self::AnonymousEscrowMarkPaymentSent => "ledger::escrow::anonymous::mark_payment_sent",
            Self::AnonymousEscrowRelease => "ledger::escrow::anonymous::release",
            Self::AnonymousEscrowCancel => "ledger::escrow::anonymous::cancel",
            Self::AnonymousEscrowOpenDispute => "ledger::escrow::anonymous::open_dispute",
            Self::AnonymousEscrowResolveDispute => "ledger::escrow::anonymous::resolve_dispute",
            Self::SetExecutionDepth => "ledger::parameters::set_execution_depth",
            Self::TransferV1BatchBegin => "ledger::asset::batch::begin",
            Self::TransferV1BatchEnd => "ledger::asset::batch::end",
            Self::TransferV1BatchApply => "ledger::asset::batch::apply",
            Self::TransferBatch => "ledger::asset::transfer_batch",
            Self::AxtBegin => "axt::begin",
            Self::AxtTouch => "axt::touch",
            Self::VerifyDsProof => "axt::verify_proof",
            Self::UseAssetHandle => "axt::use_asset_handle",
            Self::AxtCommit => "axt::commit",
            Self::DeactivateContractInstance => "seiyaku::deactivate_instance",
            Self::RemoveSmartContractBytes => "seiyaku::remove_code",
            Self::RegisterSmartContractCode => "seiyaku::register_code",
            Self::RegisterSmartContractBytes => "seiyaku::register_bytes",
            Self::ActivateContractInstance => "seiyaku::activate_instance",
            Self::ScExecuteSubmitBallot => "ledger::governance::submit_ballot",
            Self::ScExecuteUnshield => "crypto::zk::submit_unshield",
            Self::ZkRootsGet => "crypto::zk::roots",
            Self::ZkVoteGetTally => "ledger::governance::tally",
            Self::ZkVerifyTransfer => "crypto::zk::verify_transfer",
            Self::ZkVerifyUnshield => "crypto::zk::verify_unshield",
            Self::ZkVerifyBatch => "crypto::zk::verify_batch",
            Self::ZkVoteVerifyBallot => "ledger::governance::verify_ballot",
            Self::ZkVoteVerifyTally => "ledger::governance::verify_tally",
            Self::BuildSubmitBallotInline => "ledger::governance::build_submit_ballot",
            Self::BuildUnshieldInline => "crypto::zk::build_unshield",
            Self::VrfEpochSeed => "crypto::vrf::epoch_seed",
            Self::VrfVerify => "crypto::vrf::verify",
            Self::VrfVerifyBatch => "crypto::vrf::verify_batch",
            Self::Sm3Hash => "crypto::sm3",
            Self::JsonObject => "json::object",
            // The scalar setter cannot represent Kotodama's adaptive-width
            // `int`; source must use native `json { ... }` construction,
            // which carries the exact pointer-backed value.
            Self::JsonSetInt => self.name(),
            Self::JsonSetAccountId => "json::set_account_id",
            Self::GetInt => "json::get_int",
            Self::GetDecimal => "json::get_decimal",
            Self::GetQuantity => "json::get_quantity",
            Self::GetJson => "json::get",
            Self::GetName => "json::get_name",
            Self::GetAccountId => "json::get_account_id",
            Self::GetAssetDefinitionId => "json::get_asset_definition_id",
            Self::GetNftId => "json::get_nft_id",
            Self::GetBlobHex => "json::get_bytes_hex",
            Self::Sha256Hash => "crypto::sha256",
            Self::Sha3Hash => "crypto::sha3",
            Self::Blake2b256Hash => "crypto::blake2b256",
            Self::Keccak256Hash => "crypto::keccak256",
            Self::IrohaHash => "crypto::iroha_hash",
            Self::Sm2Verify => "crypto::sm2::verify",
            Self::VerifySignature => "crypto::verify_signature",
            Self::Sm4GcmSeal => "crypto::sm4_gcm::seal",
            Self::Sm4GcmOpen => "crypto::sm4_gcm::open",
            Self::Sm4CcmSeal => "crypto::sm4_ccm::seal",
            Self::Sm4CcmOpen => "crypto::sm4_ccm::open",
            Self::ProveExecution => "crypto::prove_execution",
            Self::VerifyProof => "crypto::verify_proof",
            Self::SoracloudReadCommittedState => "soracloud::read_committed_state",
            Self::SoracloudEmitStateMutation => "soracloud::emit_state_mutation",
            Self::SoracloudEmitMailboxMessage => "soracloud::emit_mailbox_message",
            Self::SoracloudAppendJournal => "soracloud::append_journal",
            Self::SoracloudPublishCheckpoint => "soracloud::publish_checkpoint",
            Self::SoracloudReadSecret => "soracloud::read_secret",
            Self::SoracloudReadCredential => "soracloud::read_credential",
            Self::SoracloudEgressFetch => "soracloud::egress_fetch",
            Self::SoracloudReadConfig => "soracloud::read_config",
            Self::SoracloudReadSecretEnvelope => "soracloud::read_secret_envelope",
            Self::Path => "codec::path",
            Self::NameDecode => "codec::decode_name",
            Self::TlvEq => "codec::tlv_eq",
            Self::TlvLen => "codec::tlv_len",
            Self::PointerToNorito => "codec::to_norito",
            Self::EncodeInt => self.name(),
            Self::DecodeInt => self.name(),
            Self::EncodeJson => "codec::encode_json",
            Self::DecodeJson => "codec::decode_json",
            Self::SchemaEncode => "codec::schema::encode",
            Self::SchemaDecode => "codec::schema::decode",
            Self::SchemaInfo => "codec::schema::info",
            // Operators and the named V1 conversions are the only numeric
            // source surface. These registry entries remain compiler-owned
            // lowering helpers and deliberately have no source alias.
            Self::NumericToInt
            | Self::NumericNeg
            | Self::NumericAdd
            | Self::NumericSub
            | Self::NumericMul
            | Self::NumericDiv
            | Self::NumericRem
            | Self::NumericEq
            | Self::NumericNe
            | Self::NumericLt
            | Self::NumericLe
            | Self::NumericGt
            | Self::NumericGe => self.name(),
            Self::WrappingAdd => "math::wrapping_add",
            Self::WrappingSub => "math::wrapping_sub",
            Self::WrappingMul => "math::wrapping_mul",
            Self::WrappingNeg => "math::wrapping_neg",
            // These scalar IVM operations do not yet have specified 512-bit
            // Kotodama semantics. Keep them available to compiler internals,
            // but do not advertise a width-constrained source API.
            Self::Isqrt
            | Self::Abs
            | Self::Min
            | Self::Max
            | Self::DivCeil
            | Self::Gcd
            | Self::Mean => self.name(),
            Self::Poseidon2 => "crypto::poseidon2",
            Self::Poseidon6 => "crypto::poseidon6",
            Self::Pubkgen => "crypto::pubkgen",
            Self::Valcom => "crypto::valcom",
            Self::GetPrivateInput => "crypto::private_input",
            Self::UseNullifier => "crypto::use_nullifier",
            Self::CommitOutput => "crypto::commit_output",
            Self::SetVl => "runtime::set_vector_length",
            Self::Alloc
            | Self::QueryExecuteNorito
            | Self::ExecuteQuery
            | Self::GrowHeap
            | Self::GetMerklePath
            | Self::GetMerkleCompact
            | Self::GetRegisterMerkleCompact
            | Self::JsonSetIntDirect
            | Self::JsonSetAccountIdDirect
            | Self::JsonGetIntDirect
            | Self::JsonGetDecimalDirect
            | Self::JsonGetQuantityDirect
            | Self::JsonGetJsonDirect
            | Self::JsonGetNameDirect
            | Self::JsonGetAccountIdDirect
            | Self::JsonGetAssetDefinitionIdDirect
            | Self::JsonGetNftIdDirect
            | Self::JsonGetBlobHexDirect
            | Self::BuildPathKeyNoritoDirect
            | Self::SchemaEncodeDirect
            | Self::SchemaDecodeDirect
            | Self::SchemaInfoDirect
            | Self::NumericToIntDirect
            | Self::NumericAddDirect
            | Self::NumericSubDirect
            | Self::NumericMulDirect
            | Self::NumericDivDirect
            | Self::NumericRemDirect
            | Self::NumericNegDirect
            | Self::NumericEqDirect
            | Self::NumericNeDirect
            | Self::NumericLtDirect
            | Self::NumericLeDirect
            | Self::NumericGtDirect
            | Self::NumericGeDirect
            | Self::SysvarAuthority => self.name(),
        }
    }

    /// Resolve a source-visible builtin by its canonical spelling.
    pub fn from_source_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|builtin| {
            matches!(
                builtin.surface(),
                BuiltinSurface::Function | BuiltinSurface::FunctionOrMethod
            ) && builtin.source_name() == name
        })
    }

    /// Return how V1 source may call this builtin.
    pub const fn surface(self) -> BuiltinSurface {
        match self {
            Self::Contains
            | Self::GetOrDefault
            | Self::GetOr
            | Self::Ensure
            | Self::StateMapRemove => BuiltinSurface::MethodOnly,
            Self::GetInt
            | Self::GetDecimal
            | Self::GetQuantity
            | Self::GetJson
            | Self::GetName
            | Self::GetAccountId
            | Self::GetAssetDefinitionId
            | Self::GetNftId
            | Self::GetBlobHex => BuiltinSurface::FunctionOrMethod,
            builtin if matches!(builtin.mode(), BuiltinMode::CompilerInternal) => {
                BuiltinSurface::CompilerInternal
            }
            _ => BuiltinSurface::Function,
        }
    }

    /// Return the canonical effect classification for this builtin.
    pub const fn effects(self) -> BuiltinEffects {
        match self {
            Self::RecordSccpMessage | Self::ScExecuteSubmitBallot | Self::ScExecuteUnshield => {
                BuiltinEffects::INSTRUCTION
            }
            Self::Ensure | Self::StateMapRemove | Self::StateSet | Self::StateDel => {
                BuiltinEffects::DURABLE_STATE
            }
            Self::SubscriptionBill
            | Self::SubscriptionRecordUsage
            | Self::DebugPrint
            | Self::DebugLog
            | Self::Info
            | Self::TestInvokeEntrypoint
            | Self::TestInvokeEntrypointAs
            | Self::TestExpectRejectAs
            | Self::TestActorAccount
            | Self::TestActorPublicKey
            | Self::TestActorSign
            | Self::SetAccountDetail
            | Self::MintAsset
            | Self::BurnAsset
            | Self::TransferAsset
            | Self::SetAssetTransferFreeze
            | Self::SetAssetTransferDailyLimit
            | Self::AccountRecoveryPropose
            | Self::AccountRecoveryApprove
            | Self::AccountRecoveryCancel
            | Self::AccountRecoveryFinalize
            | Self::NftMintAsset
            | Self::NftSetMetadata
            | Self::NftBurnAsset
            | Self::NftTransferAsset
            | Self::RegisterDomain
            | Self::UnregisterDomain
            | Self::TransferDomain
            | Self::RegisterAccount
            | Self::UnregisterAccount
            | Self::RegisterAsset
            | Self::CreateNewAsset
            | Self::UnregisterAsset
            | Self::RegisterPeer
            | Self::UnregisterPeer
            | Self::CreateTrigger
            | Self::RegisterTrigger
            | Self::RemoveTrigger
            | Self::UnregisterTrigger
            | Self::SetTriggerEnabled
            | Self::CreateRole
            | Self::DeleteRole
            | Self::GrantRole
            | Self::RevokeRole
            | Self::GrantPermission
            | Self::RevokePermission
            | Self::GrantContractEntrypoint
            | Self::RevokeContractEntrypoint
            | Self::EscrowOpenOffer
            | Self::EscrowAccept
            | Self::EscrowMarkPaymentSent
            | Self::EscrowRelease
            | Self::EscrowCancel
            | Self::EscrowOpenDispute
            | Self::EscrowResolveDispute
            | Self::AnonymousEscrowOpenOffer
            | Self::AnonymousEscrowAccept
            | Self::AnonymousEscrowMarkPaymentSent
            | Self::AnonymousEscrowRelease
            | Self::AnonymousEscrowCancel
            | Self::AnonymousEscrowOpenDispute
            | Self::AnonymousEscrowResolveDispute
            | Self::GetPrivateInput
            | Self::UseNullifier
            | Self::CommitOutput
            | Self::CreateNftsForAllUsers
            | Self::SetExecutionDepth
            | Self::TransferV1BatchBegin
            | Self::TransferV1BatchEnd
            | Self::TransferV1BatchApply
            | Self::TransferBatch
            | Self::AxtBegin
            | Self::AxtTouch
            | Self::UseAssetHandle
            | Self::AxtCommit
            | Self::DeactivateContractInstance
            | Self::RemoveSmartContractBytes
            | Self::RegisterSmartContractCode
            | Self::RegisterSmartContractBytes
            | Self::ActivateContractInstance
            | Self::ZkVerifyTransfer
            | Self::ZkVerifyUnshield
            | Self::ZkVerifyBatch
            | Self::ZkVoteVerifyBallot
            | Self::ZkVoteVerifyTally
            | Self::SoracloudReadCommittedState
            | Self::SoracloudEmitStateMutation
            | Self::SoracloudEmitMailboxMessage
            | Self::SoracloudAppendJournal
            | Self::SoracloudPublishCheckpoint
            | Self::SoracloudReadSecret
            | Self::SoracloudReadCredential
            | Self::SoracloudEgressFetch
            | Self::SoracloudReadConfig
            | Self::SoracloudReadSecretEnvelope
            | Self::AddSignatory
            | Self::RemoveSignatory
            | Self::SetAccountQuorum => BuiltinEffects::HOST,
            _ => BuiltinEffects::NONE,
        }
    }

    /// Return the scheduler access class for this builtin.
    pub const fn access(self) -> BuiltinAccess {
        match self {
            Self::PointerConstructor(PointerConstructor::AccountId) => BuiltinAccess::LedgerRead,
            Self::Contains
            | Self::GetOrDefault
            | Self::GetOr
            | Self::StateGet
            | Self::StateKeys
            | Self::StateHas
            | Self::StateLen
            | Self::StateCount => BuiltinAccess::StateRead,
            Self::Ensure | Self::StateMapRemove | Self::StateSet | Self::StateDel => {
                BuiltinAccess::StateWrite
            }
            Self::QueryExecuteNorito
            | Self::QueryGetAccount
            | Self::QueryGetAsset
            | Self::QueryGetAssetDefinition
            | Self::QueryGetDomain
            | Self::QueryGetNft
            | Self::QueryPageAccounts
            | Self::QueryPageAssets
            | Self::QueryPageAssetDefinitions
            | Self::QueryPageDomains
            | Self::QueryPageNfts
            | Self::QueryGetParameter
            | Self::QueryGetContractManifest
            | Self::QueryGetContractInstance
            | Self::ExecuteQuery
            | Self::ResolveAccountAlias
            | Self::GetAccountBalance
            | Self::ZkRootsGet
            | Self::ZkVoteGetTally
            | Self::ZkVerifyTransfer
            | Self::ZkVerifyUnshield
            | Self::ZkVerifyBatch
            | Self::ZkVoteVerifyBallot
            | Self::ZkVoteVerifyTally
            | Self::VrfEpochSeed => BuiltinAccess::LedgerRead,
            Self::RecordSccpMessage
            | Self::TestInvokeEntrypoint
            | Self::TestInvokeEntrypointAs
            | Self::TestExpectRejectAs
            | Self::SoracloudReadCommittedState
            | Self::SoracloudEmitStateMutation
            | Self::SoracloudEmitMailboxMessage
            | Self::SoracloudAppendJournal
            | Self::SoracloudPublishCheckpoint
            | Self::SoracloudReadSecret
            | Self::SoracloudReadCredential
            | Self::SoracloudEgressFetch
            | Self::SoracloudReadConfig
            | Self::SoracloudReadSecretEnvelope => BuiltinAccess::Dynamic,
            Self::GetPrivateInput
            | Self::CommitOutput
            | Self::SetExecutionDepth
            | Self::DebugPrint
            | Self::DebugLog
            | Self::Info
            | Self::TestActorAccount
            | Self::TestActorPublicKey
            | Self::TestActorSign => BuiltinAccess::None,
            builtin
                if builtin.effects().host_side_effects || builtin.effects().emits_instructions =>
            {
                BuiltinAccess::LedgerWrite
            }
            _ => BuiltinAccess::None,
        }
    }

    /// Return the execution mode required by this builtin.
    pub const fn mode(self) -> BuiltinMode {
        match self {
            Self::GetPrivateInput | Self::UseNullifier | Self::CommitOutput => BuiltinMode::ZkOnly,
            Self::Assert | Self::AssertEq => BuiltinMode::TestOnly,
            Self::TestInvokeEntrypoint
            | Self::TestInvokeEntrypointAs
            | Self::TestExpectRejectAs
            | Self::TestActorAccount
            | Self::TestActorPublicKey
            | Self::TestActorSign => BuiltinMode::TestFunctionOnly,
            Self::PointerConstructor(
                PointerConstructor::Domain
                | PointerConstructor::Blob
                | PointerConstructor::NoritoBytes
                | PointerConstructor::AxtDescriptor
                | PointerConstructor::AssetHandle
                | PointerConstructor::ProofBlob
                | PointerConstructor::SoracloudRequest
                | PointerConstructor::SoracloudResponse,
            )
            | Self::KeysTake2
            | Self::ValuesTake2
            | Self::KeysValuesTake2
            | Self::Alloc
            | Self::QueryExecuteNorito
            | Self::ExecuteQuery
            | Self::DebugPrint
            | Self::DebugLog
            | Self::GrowHeap
            | Self::GetMerklePath
            | Self::GetMerkleCompact
            | Self::GetRegisterMerkleCompact
            | Self::AxtBegin
            | Self::AxtTouch
            | Self::VerifyDsProof
            | Self::UseAssetHandle
            | Self::AxtCommit
            | Self::SoracloudReadCommittedState
            | Self::SoracloudEmitStateMutation
            | Self::SoracloudEmitMailboxMessage
            | Self::SoracloudAppendJournal
            | Self::SoracloudPublishCheckpoint
            | Self::SoracloudReadSecret
            | Self::SoracloudReadCredential
            | Self::SoracloudEgressFetch
            | Self::SoracloudReadConfig
            | Self::SoracloudReadSecretEnvelope
            | Self::PointerToNorito
            | Self::JsonSetInt
            | Self::Path
            | Self::NameDecode
            | Self::TlvEq
            | Self::TlvLen
            | Self::EncodeInt
            | Self::DecodeInt
            | Self::EncodeJson
            | Self::DecodeJson
            | Self::SchemaEncode
            | Self::SchemaDecode
            | Self::SchemaInfo
            | Self::JsonSetIntDirect
            | Self::JsonSetAccountIdDirect
            | Self::JsonGetIntDirect
            | Self::JsonGetDecimalDirect
            | Self::JsonGetQuantityDirect
            | Self::JsonGetJsonDirect
            | Self::JsonGetNameDirect
            | Self::JsonGetAccountIdDirect
            | Self::JsonGetAssetDefinitionIdDirect
            | Self::JsonGetNftIdDirect
            | Self::JsonGetBlobHexDirect
            | Self::BuildPathKeyNoritoDirect
            | Self::SchemaEncodeDirect
            | Self::SchemaDecodeDirect
            | Self::SchemaInfoDirect
            | Self::NumericToInt
            | Self::NumericNeg
            | Self::NumericAdd
            | Self::NumericSub
            | Self::NumericMul
            | Self::NumericDiv
            | Self::NumericRem
            | Self::NumericEq
            | Self::NumericNe
            | Self::NumericLt
            | Self::NumericLe
            | Self::NumericGt
            | Self::NumericGe
            | Self::Isqrt
            | Self::Abs
            | Self::Min
            | Self::Max
            | Self::DivCeil
            | Self::Gcd
            | Self::Mean
            | Self::NumericToIntDirect
            | Self::NumericAddDirect
            | Self::NumericSubDirect
            | Self::NumericMulDirect
            | Self::NumericDivDirect
            | Self::NumericRemDirect
            | Self::NumericNegDirect
            | Self::NumericEqDirect
            | Self::NumericNeDirect
            | Self::NumericLtDirect
            | Self::NumericLeDirect
            | Self::NumericGtDirect
            | Self::NumericGeDirect
            | Self::SetExecutionDepth
            | Self::DeactivateContractInstance
            | Self::RemoveSmartContractBytes
            | Self::RegisterSmartContractCode
            | Self::RegisterSmartContractBytes
            | Self::ActivateContractInstance
            | Self::SetVl
            | Self::SysvarAuthority => BuiltinMode::CompilerInternal,
            _ => BuiltinMode::Any,
        }
    }

    /// Return the gas charging class for this builtin.
    pub const fn gas_class(self) -> BuiltinGasClass {
        match self {
            Self::Sm3Hash
            | Self::Sha256Hash
            | Self::Sha3Hash
            | Self::Blake2b256Hash
            | Self::Keccak256Hash
            | Self::IrohaHash
            | Self::VerifySignature
            | Self::VrfVerifyBatch
            | Self::EncodeJson
            | Self::DecodeJson
            | Self::SchemaEncode
            | Self::SchemaDecode => BuiltinGasClass::LinearInput,
            _ if !self.operation_syscalls().is_empty() => BuiltinGasClass::HostQuoted,
            _ => BuiltinGasClass::Constant,
        }
    }

    /// Return every operation syscall reachable from this builtin's lowering.
    ///
    /// This deliberately excludes `INPUT_PUBLISH_TLV`, which is pointer-ABI
    /// transport rather than the operation being authorized and scheduled.
    pub const fn operation_syscalls(self) -> &'static [u32] {
        use ivm_abi::syscalls as s;

        match self {
            Self::PointerConstructor(PointerConstructor::AccountId) => {
                &[s::SYSCALL_RESOLVE_ACCOUNT_ALIAS]
            }
            Self::PointerConstructor(_) => &[],
            Self::Contains => &[s::SYSCALL_BUILD_PATH_KEY_NORITO, s::SYSCALL_STATE_GET],
            Self::GetOrDefault | Self::GetOr => &[
                s::SYSCALL_BUILD_PATH_KEY_NORITO,
                s::SYSCALL_STATE_GET,
                s::SYSCALL_STATE_VALUE_DECODE,
            ],
            Self::Ensure => &[
                s::SYSCALL_BUILD_PATH_KEY_NORITO,
                s::SYSCALL_STATE_GET,
                s::SYSCALL_STATE_VALUE_DECODE,
                s::SYSCALL_STATE_VALUE_ENCODE,
                s::SYSCALL_STATE_SET,
            ],
            Self::StateMapRemove => &[
                s::SYSCALL_BUILD_PATH_KEY_NORITO,
                s::SYSCALL_STATE_GET,
                s::SYSCALL_STATE_VALUE_DECODE,
                s::SYSCALL_STATE_DEL,
            ],
            Self::KeysTake2 | Self::ValuesTake2 | Self::KeysValuesTake2 => &[],
            Self::StateGet => &[s::SYSCALL_STATE_GET],
            Self::StateSet => &[s::SYSCALL_STATE_SET],
            Self::StateDel => &[s::SYSCALL_STATE_DEL],
            Self::StateKeys => &[s::SYSCALL_STATE_KEYS],
            Self::StateHas => &[s::SYSCALL_STATE_HAS],
            Self::StateLen => &[s::SYSCALL_STATE_LEN],
            Self::StateCount => &[s::SYSCALL_STATE_COUNT],
            Self::QueryExecuteNorito => &[s::SYSCALL_QUERY_EXECUTE_NORITO],
            Self::QueryGetAccount
            | Self::QueryGetAsset
            | Self::QueryGetAssetDefinition
            | Self::QueryGetDomain
            | Self::QueryGetNft => &[s::SYSCALL_CORE_QUERY_GET],
            Self::QueryPageAccounts
            | Self::QueryPageAssets
            | Self::QueryPageAssetDefinitions
            | Self::QueryPageDomains
            | Self::QueryPageNfts => &[s::SYSCALL_CORE_QUERY_PAGE],
            Self::QueryGetParameter => &[s::SYSCALL_QUERY_GET_PARAMETER],
            Self::QueryGetContractManifest => &[s::SYSCALL_QUERY_GET_CONTRACT_MANIFEST],
            Self::QueryGetContractInstance => &[s::SYSCALL_QUERY_GET_CONTRACT_INSTANCE],
            Self::RecordSccpMessage | Self::ScExecuteSubmitBallot | Self::ScExecuteUnshield => {
                &[s::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION]
            }
            Self::ExecuteQuery => &[s::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY],
            Self::ResolveAccountAlias => &[s::SYSCALL_RESOLVE_ACCOUNT_ALIAS],
            Self::SubscriptionBill => &[s::SYSCALL_SUBSCRIPTION_BILL],
            Self::SubscriptionRecordUsage => &[s::SYSCALL_SUBSCRIPTION_RECORD_USAGE],
            Self::GetAccountBalance => &[s::SYSCALL_GET_ACCOUNT_BALANCE],
            Self::GetPublicInput | Self::TriggerEvent => &[s::SYSCALL_GET_PUBLIC_INPUT],
            Self::DebugPrint => &[s::SYSCALL_DEBUG_PRINT],
            Self::DebugLog | Self::Info => &[s::SYSCALL_DEBUG_LOG],
            Self::Assert | Self::Require | Self::AssertEq => &[s::SYSCALL_ABORT],
            Self::TestInvokeEntrypoint => &[
                s::SYSCALL_STATE_GET,
                s::SYSCALL_JSON_ENCODE,
                s::SYSCALL_STATE_SET,
                s::SYSCALL_STATE_DEL,
            ],
            Self::TestInvokeEntrypointAs => &[s::SYSCALL_KOTO_TEST_INVOKE_ENTRYPOINT_AS],
            Self::TestExpectRejectAs => &[s::SYSCALL_KOTO_TEST_EXPECT_REJECT_AS],
            Self::TestActorAccount => &[s::SYSCALL_KOTO_TEST_ACTOR_ACCOUNT],
            Self::TestActorPublicKey => &[s::SYSCALL_KOTO_TEST_ACTOR_PUBLIC_KEY],
            Self::TestActorSign => &[s::SYSCALL_KOTO_TEST_ACTOR_SIGN],
            Self::SetAccountDetail => &[s::SYSCALL_SET_ACCOUNT_DETAIL],
            Self::MintAsset => &[s::SYSCALL_MINT_ASSET],
            Self::BurnAsset => &[s::SYSCALL_BURN_ASSET],
            Self::TransferAsset => &[s::SYSCALL_TRANSFER_ASSET_SCOPED],
            Self::SetAssetTransferFreeze => &[s::SYSCALL_SET_ASSET_TRANSFER_FREEZE],
            Self::SetAssetTransferDailyLimit => &[s::SYSCALL_SET_ASSET_TRANSFER_DAILY_LIMIT],
            Self::AccountRecoveryPropose => &[s::SYSCALL_ACCOUNT_RECOVERY_PROPOSE],
            Self::AccountRecoveryApprove => &[s::SYSCALL_ACCOUNT_RECOVERY_APPROVE],
            Self::AccountRecoveryCancel => &[s::SYSCALL_ACCOUNT_RECOVERY_CANCEL],
            Self::AccountRecoveryFinalize => &[s::SYSCALL_ACCOUNT_RECOVERY_FINALIZE],
            Self::NftMintAsset => &[s::SYSCALL_NFT_MINT_ASSET],
            Self::NftSetMetadata => &[s::SYSCALL_NFT_SET_METADATA],
            Self::NftBurnAsset => &[s::SYSCALL_NFT_BURN_ASSET],
            Self::NftTransferAsset => &[s::SYSCALL_NFT_TRANSFER_ASSET],
            Self::RegisterDomain => &[s::SYSCALL_REGISTER_DOMAIN],
            Self::UnregisterDomain => &[s::SYSCALL_UNREGISTER_DOMAIN],
            Self::TransferDomain => &[s::SYSCALL_TRANSFER_DOMAIN],
            Self::RegisterAccount => &[s::SYSCALL_REGISTER_ACCOUNT],
            Self::UnregisterAccount => &[s::SYSCALL_UNREGISTER_ACCOUNT],
            Self::RegisterAsset | Self::CreateNewAsset => &[s::SYSCALL_REGISTER_ASSET],
            Self::UnregisterAsset => &[s::SYSCALL_UNREGISTER_ASSET],
            Self::RegisterPeer => &[s::SYSCALL_REGISTER_PEER],
            Self::UnregisterPeer => &[s::SYSCALL_UNREGISTER_PEER],
            Self::CreateTrigger | Self::RegisterTrigger => &[s::SYSCALL_CREATE_TRIGGER],
            Self::RemoveTrigger | Self::UnregisterTrigger => &[s::SYSCALL_REMOVE_TRIGGER],
            Self::SetTriggerEnabled => &[s::SYSCALL_SET_TRIGGER_ENABLED],
            Self::CreateRole => &[s::SYSCALL_CREATE_ROLE],
            Self::DeleteRole => &[s::SYSCALL_DELETE_ROLE],
            Self::GrantRole => &[s::SYSCALL_GRANT_ROLE],
            Self::RevokeRole => &[s::SYSCALL_REVOKE_ROLE],
            Self::GrantPermission => &[s::SYSCALL_GRANT_PERMISSION],
            Self::RevokePermission => &[s::SYSCALL_REVOKE_PERMISSION],
            Self::GrantContractEntrypoint => &[s::SYSCALL_GRANT_CONTRACT_ENTRYPOINT],
            Self::RevokeContractEntrypoint => &[s::SYSCALL_REVOKE_CONTRACT_ENTRYPOINT],
            Self::EscrowOpenOffer => &[s::SYSCALL_ESCROW_OPEN_OFFER],
            Self::EscrowAccept => &[s::SYSCALL_ESCROW_ACCEPT],
            Self::EscrowMarkPaymentSent => &[s::SYSCALL_ESCROW_MARK_PAYMENT_SENT],
            Self::EscrowRelease => &[s::SYSCALL_ESCROW_RELEASE],
            Self::EscrowCancel => &[s::SYSCALL_ESCROW_CANCEL],
            Self::EscrowOpenDispute => &[s::SYSCALL_ESCROW_OPEN_DISPUTE],
            Self::EscrowResolveDispute => &[s::SYSCALL_ESCROW_RESOLVE_DISPUTE],
            Self::AnonymousEscrowOpenOffer => &[s::SYSCALL_ANONYMOUS_ESCROW_OPEN_OFFER],
            Self::AnonymousEscrowAccept => &[s::SYSCALL_ANONYMOUS_ESCROW_ACCEPT],
            Self::AnonymousEscrowMarkPaymentSent => {
                &[s::SYSCALL_ANONYMOUS_ESCROW_MARK_PAYMENT_SENT]
            }
            Self::AnonymousEscrowRelease => &[s::SYSCALL_ANONYMOUS_ESCROW_RELEASE],
            Self::AnonymousEscrowCancel => &[s::SYSCALL_ANONYMOUS_ESCROW_CANCEL],
            Self::AnonymousEscrowOpenDispute => &[s::SYSCALL_ANONYMOUS_ESCROW_OPEN_DISPUTE],
            Self::AnonymousEscrowResolveDispute => &[s::SYSCALL_ANONYMOUS_ESCROW_RESOLVE_DISPUTE],
            Self::GetPrivateInput => &[s::SYSCALL_GET_PRIVATE_INPUT],
            Self::UseNullifier => &[s::SYSCALL_USE_NULLIFIER],
            Self::CommitOutput => &[s::SYSCALL_COMMIT_OUTPUT],
            Self::CreateNftsForAllUsers => &[s::SYSCALL_CREATE_NFTS_FOR_ALL_USERS],
            Self::SetExecutionDepth => &[s::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH],
            Self::TransferV1BatchBegin => &[s::SYSCALL_TRANSFER_V1_BATCH_BEGIN],
            Self::TransferV1BatchEnd => &[s::SYSCALL_TRANSFER_V1_BATCH_END],
            Self::TransferV1BatchApply => &[s::SYSCALL_TRANSFER_V1_BATCH_APPLY],
            Self::TransferBatch => &[
                s::SYSCALL_TRANSFER_V1_BATCH_BEGIN,
                s::SYSCALL_TRANSFER_V1,
                s::SYSCALL_TRANSFER_V1_BATCH_END,
            ],
            Self::AxtBegin => &[s::SYSCALL_AXT_BEGIN],
            Self::AxtTouch => &[s::SYSCALL_AXT_TOUCH],
            Self::VerifyDsProof => &[s::SYSCALL_VERIFY_DS_PROOF],
            Self::UseAssetHandle => &[s::SYSCALL_USE_ASSET_HANDLE],
            Self::AxtCommit => &[s::SYSCALL_AXT_COMMIT],
            Self::DeactivateContractInstance => &[s::SYSCALL_DEACTIVATE_CONTRACT_INSTANCE],
            Self::RemoveSmartContractBytes => &[s::SYSCALL_REMOVE_SMART_CONTRACT_BYTES],
            Self::RegisterSmartContractCode => &[s::SYSCALL_REGISTER_SMART_CONTRACT_CODE],
            Self::RegisterSmartContractBytes => &[s::SYSCALL_REGISTER_SMART_CONTRACT_BYTES],
            Self::ActivateContractInstance => &[s::SYSCALL_ACTIVATE_CONTRACT_INSTANCE],
            Self::ZkRootsGet => &[s::SYSCALL_ZK_ROOTS_GET],
            Self::ZkVoteGetTally => &[s::SYSCALL_ZK_VOTE_GET_TALLY],
            Self::ZkVerifyTransfer => &[s::SYSCALL_ZK_VERIFY_TRANSFER],
            Self::ZkVerifyUnshield => &[s::SYSCALL_ZK_VERIFY_UNSHIELD],
            Self::ZkVerifyBatch => &[s::SYSCALL_ZK_VERIFY_BATCH],
            Self::ZkVoteVerifyBallot => &[s::SYSCALL_ZK_VOTE_VERIFY_BALLOT],
            Self::ZkVoteVerifyTally => &[s::SYSCALL_ZK_VOTE_VERIFY_TALLY],
            Self::BuildSubmitBallotInline | Self::BuildUnshieldInline => &[],
            Self::VrfEpochSeed => &[s::SYSCALL_VRF_EPOCH_SEED],
            Self::VrfVerify => &[s::SYSCALL_VRF_VERIFY],
            Self::VrfVerifyBatch => &[s::SYSCALL_VRF_VERIFY_BATCH],
            Self::Sm3Hash => &[s::SYSCALL_SM3_HASH],
            Self::Sha256Hash => &[s::SYSCALL_SHA256_HASH],
            Self::Sha3Hash => &[s::SYSCALL_SHA3_HASH],
            Self::Blake2b256Hash => &[s::SYSCALL_BLAKE2B256_HASH],
            Self::Keccak256Hash => &[s::SYSCALL_KECCAK256_HASH],
            Self::IrohaHash => &[s::SYSCALL_IROHA_HASH],
            Self::Sm2Verify => &[s::SYSCALL_SM2_VERIFY],
            Self::VerifySignature => &[s::SYSCALL_VERIFY_SIGNATURE],
            Self::Sm4GcmSeal => &[s::SYSCALL_SM4_GCM_SEAL],
            Self::Sm4GcmOpen => &[s::SYSCALL_SM4_GCM_OPEN],
            Self::Sm4CcmSeal => &[s::SYSCALL_SM4_CCM_SEAL],
            Self::Sm4CcmOpen => &[s::SYSCALL_SM4_CCM_OPEN],
            Self::Alloc => &[s::SYSCALL_ALLOC],
            Self::ProveExecution => &[s::SYSCALL_PROVE_EXECUTION],
            Self::GrowHeap => &[s::SYSCALL_GROW_HEAP],
            Self::VerifyProof => &[s::SYSCALL_VERIFY_PROOF],
            Self::GetMerklePath => &[s::SYSCALL_GET_MERKLE_PATH],
            Self::GetMerkleCompact => &[s::SYSCALL_GET_MERKLE_COMPACT],
            Self::GetRegisterMerkleCompact => &[s::SYSCALL_GET_REGISTER_MERKLE_COMPACT],
            Self::SoracloudReadCommittedState => &[s::SYSCALL_SORACLOUD_READ_COMMITTED_STATE],
            Self::SoracloudEmitStateMutation => &[s::SYSCALL_SORACLOUD_EMIT_STATE_MUTATION],
            Self::SoracloudEmitMailboxMessage => &[s::SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE],
            Self::SoracloudAppendJournal => &[s::SYSCALL_SORACLOUD_APPEND_JOURNAL],
            Self::SoracloudPublishCheckpoint => &[s::SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT],
            Self::SoracloudReadSecret => &[s::SYSCALL_SORACLOUD_READ_SECRET],
            Self::SoracloudReadCredential => &[s::SYSCALL_SORACLOUD_READ_CREDENTIAL],
            Self::SoracloudEgressFetch => &[s::SYSCALL_SORACLOUD_EGRESS_FETCH],
            Self::SoracloudReadConfig => &[s::SYSCALL_SORACLOUD_READ_CONFIG],
            Self::SoracloudReadSecretEnvelope => &[s::SYSCALL_SORACLOUD_READ_SECRET_ENVELOPE],
            Self::AddSignatory => &[s::SYSCALL_ADD_SIGNATORY],
            Self::RemoveSignatory => &[s::SYSCALL_REMOVE_SIGNATORY],
            Self::SetAccountQuorum => &[s::SYSCALL_SET_ACCOUNT_QUORUM],
            Self::Path => &[s::SYSCALL_BUILD_PATH_KEY_NORITO],
            Self::NameDecode => &[s::SYSCALL_NAME_DECODE],
            Self::TlvEq => &[s::SYSCALL_TLV_EQ],
            Self::TlvLen => &[s::SYSCALL_TLV_LEN],
            Self::PointerToNorito => &[s::SYSCALL_POINTER_TO_NORITO],
            Self::JsonObject => &[s::SYSCALL_JSON_OBJECT],
            Self::JsonSetInt => &[s::SYSCALL_JSON_SET_I64],
            Self::JsonSetAccountId => &[s::SYSCALL_JSON_SET_ACCOUNT_ID],
            Self::EncodeInt => &[s::SYSCALL_ENCODE_INT],
            Self::DecodeInt => &[s::SYSCALL_DECODE_INT],
            Self::EncodeJson => &[s::SYSCALL_JSON_ENCODE],
            Self::DecodeJson => &[s::SYSCALL_JSON_DECODE],
            Self::JsonSetIntDirect => &[s::SYSCALL_JSON_SET_I64_DIRECT],
            Self::JsonSetAccountIdDirect => &[s::SYSCALL_JSON_SET_ACCOUNT_ID_DIRECT],
            Self::JsonGetIntDirect => &[s::SYSCALL_JSON_GET_INT_DIRECT],
            Self::JsonGetDecimalDirect => &[s::SYSCALL_JSON_GET_DECIMAL_DIRECT],
            Self::JsonGetQuantityDirect => &[s::SYSCALL_JSON_GET_QUANTITY_DIRECT],
            Self::JsonGetJsonDirect => &[s::SYSCALL_JSON_GET_JSON_DIRECT],
            Self::JsonGetNameDirect => &[s::SYSCALL_JSON_GET_NAME_DIRECT],
            Self::JsonGetAccountIdDirect => &[s::SYSCALL_JSON_GET_ACCOUNT_ID_DIRECT],
            Self::JsonGetAssetDefinitionIdDirect => {
                &[s::SYSCALL_JSON_GET_ASSET_DEFINITION_ID_DIRECT]
            }
            Self::JsonGetNftIdDirect => &[s::SYSCALL_JSON_GET_NFT_ID_DIRECT],
            Self::JsonGetBlobHexDirect => &[s::SYSCALL_JSON_GET_BLOB_HEX_DIRECT],
            Self::BuildPathKeyNoritoDirect => &[s::SYSCALL_BUILD_PATH_KEY_NORITO_DIRECT],
            Self::SchemaEncode => &[s::SYSCALL_SCHEMA_ENCODE],
            Self::SchemaDecode => &[s::SYSCALL_SCHEMA_DECODE],
            Self::SchemaInfo => &[s::SYSCALL_SCHEMA_INFO],
            Self::SchemaEncodeDirect => &[s::SYSCALL_SCHEMA_ENCODE_DIRECT],
            Self::SchemaDecodeDirect => &[s::SYSCALL_SCHEMA_DECODE_DIRECT],
            Self::SchemaInfoDirect => &[s::SYSCALL_SCHEMA_INFO_DIRECT],
            Self::NumericToInt => &[s::SYSCALL_DECIMAL_TRY_TO_INT_EXACT],
            Self::NumericNeg => &[s::SYSCALL_INT_NEG, s::SYSCALL_DECIMAL_NEG],
            Self::NumericAdd => &[
                s::SYSCALL_INT_ADD,
                s::SYSCALL_DECIMAL_ADD,
                s::SYSCALL_QUANTITY_ADD,
            ],
            Self::NumericSub => &[
                s::SYSCALL_INT_SUB,
                s::SYSCALL_DECIMAL_SUB,
                s::SYSCALL_QUANTITY_SUB,
            ],
            Self::NumericMul => &[
                s::SYSCALL_INT_MUL,
                s::SYSCALL_DECIMAL_MUL,
                s::SYSCALL_QUANTITY_MUL_DECIMAL,
            ],
            Self::NumericDiv => &[
                s::SYSCALL_INT_DIV,
                s::SYSCALL_DECIMAL_DIV_EXACT,
                s::SYSCALL_QUANTITY_DIV_DECIMAL_EXACT,
                s::SYSCALL_QUANTITY_RATIO_EXACT,
            ],
            Self::NumericRem => &[s::SYSCALL_INT_REM],
            Self::NumericEq => &[
                s::SYSCALL_INT_EQ,
                s::SYSCALL_DECIMAL_EQ,
                s::SYSCALL_QUANTITY_EQ,
            ],
            Self::NumericNe => &[
                s::SYSCALL_INT_NE,
                s::SYSCALL_DECIMAL_NE,
                s::SYSCALL_QUANTITY_NE,
            ],
            Self::NumericLt => &[
                s::SYSCALL_INT_LT,
                s::SYSCALL_DECIMAL_LT,
                s::SYSCALL_QUANTITY_LT,
            ],
            Self::NumericLe => &[
                s::SYSCALL_INT_LE,
                s::SYSCALL_DECIMAL_LE,
                s::SYSCALL_QUANTITY_LE,
            ],
            Self::NumericGt => &[
                s::SYSCALL_INT_GT,
                s::SYSCALL_DECIMAL_GT,
                s::SYSCALL_QUANTITY_GT,
            ],
            Self::NumericGe => &[
                s::SYSCALL_INT_GE,
                s::SYSCALL_DECIMAL_GE,
                s::SYSCALL_QUANTITY_GE,
            ],
            Self::NumericToIntDirect
            | Self::NumericAddDirect
            | Self::NumericSubDirect
            | Self::NumericMulDirect
            | Self::NumericDivDirect
            | Self::NumericRemDirect
            | Self::NumericNegDirect
            | Self::NumericEqDirect
            | Self::NumericNeDirect
            | Self::NumericLtDirect
            | Self::NumericLeDirect
            | Self::NumericGtDirect
            | Self::NumericGeDirect => &[],
            Self::WrappingAdd
            | Self::WrappingSub
            | Self::WrappingMul
            | Self::WrappingNeg
            | Self::Isqrt
            | Self::Abs
            | Self::Min
            | Self::Max
            | Self::DivCeil
            | Self::Gcd
            | Self::Mean
            | Self::Poseidon2
            | Self::Poseidon6
            | Self::Pubkgen
            | Self::Valcom
            | Self::SetVl => &[],
            Self::GetInt => &[s::SYSCALL_JSON_GET_INT],
            Self::GetDecimal => &[s::SYSCALL_JSON_GET_DECIMAL],
            Self::GetQuantity => &[s::SYSCALL_JSON_GET_QUANTITY],
            Self::GetJson => &[s::SYSCALL_JSON_GET_JSON],
            Self::GetName => &[s::SYSCALL_JSON_GET_NAME],
            Self::GetAccountId => &[s::SYSCALL_JSON_GET_ACCOUNT_ID],
            Self::GetAssetDefinitionId => &[s::SYSCALL_JSON_GET_ASSET_DEFINITION_ID],
            Self::GetNftId => &[s::SYSCALL_JSON_GET_NFT_ID],
            Self::GetBlobHex => &[s::SYSCALL_JSON_GET_BLOB_HEX],
            Self::Authority => &[s::SYSCALL_GET_AUTHORITY],
            Self::CurrentTimeMs => &[s::SYSCALL_CURRENT_TIME_MS],
            Self::ContractSubject => &[s::SYSCALL_SYSVAR_CONTRACT_SUBJECT],
            Self::BlockHeight => &[s::SYSCALL_SYSVAR_BLOCK_HEIGHT],
            Self::BlockTimeMs => &[s::SYSCALL_SYSVAR_BLOCK_TIME_MS],
            Self::ChainId => &[s::SYSCALL_SYSVAR_CHAIN_ID],
            Self::ContractAddress => &[s::SYSCALL_SYSVAR_CONTRACT_ADDRESS],
            Self::Entrypoint => &[s::SYSCALL_SYSVAR_ENTRYPOINT],
            Self::SysvarAuthority => &[s::SYSCALL_SYSVAR_AUTHORITY],
        }
    }

    /// Return whether syscall emission is direct or compiler-derived.
    pub const fn lowering(self) -> BuiltinLowering {
        let syscalls = self.operation_syscalls();
        if syscalls.is_empty() {
            return BuiltinLowering::Instructions;
        }
        if matches!(
            self,
            Self::PointerConstructor(PointerConstructor::AccountId)
                | Self::Contains
                | Self::GetOrDefault
                | Self::GetOr
                | Self::Ensure
                | Self::StateMapRemove
                | Self::TransferBatch
                | Self::Path
                | Self::TestInvokeEntrypoint
        ) {
            BuiltinLowering::DerivedSyscalls
        } else {
            BuiltinLowering::DirectSyscall
        }
    }

    /// Return a direct syscall number when lowering is one-to-one.
    pub const fn syscall(self) -> Option<u32> {
        if !matches!(self.lowering(), BuiltinLowering::DirectSyscall) {
            return None;
        }
        let syscalls = self.operation_syscalls();
        if syscalls.len() == 1 {
            Some(syscalls[0])
        } else {
            None
        }
    }

    /// Return the canonical source-level parameter and return types.
    ///
    /// This match is intentionally exhaustive: adding a builtin without an
    /// explicit signature is a compile error rather than an implicit `any`.
    pub const fn signature(self) -> BuiltinSignature {
        use BuiltinSignature as S;

        let signature = match self {
            Self::PointerConstructor(constructor) => {
                S::new(&["string"], constructor.return_type_name())
            }
            Self::Contains => S::new(&["StateMap<K,V>", "K"], "bool"),
            Self::GetOrDefault => S::new(&["StateMap<K,V>", "K", "V"], "V"),
            Self::GetOr | Self::Ensure => S::new(&["StateMap<K,V>", "K", "V?"], "V"),
            Self::StateMapRemove => S::new(&["StateMap<K,V>", "K"], "Option<V>"),
            Self::KeysTake2 | Self::ValuesTake2 => {
                S::new(&["StateMap<int,int>", "int", "int"], "int")
            }
            Self::KeysValuesTake2 => S::new(&["StateMap<int,int>", "int", "int"], "(int,int)"),
            Self::StateGet => S::new(&["Name"], "bytes"),
            Self::StateSet => S::new(&["Name", "bytes"], "()"),
            Self::StateDel => S::new(&["Name"], "()"),
            Self::StateKeys => S::new(&["Name", "int", "int"], "bytes"),
            Self::StateHas => S::new(&["Name"], "bool"),
            Self::StateLen | Self::StateCount => S::new(&["Name"], "int"),
            Self::QueryExecuteNorito
            | Self::QueryGetContractManifest
            | Self::ZkRootsGet
            | Self::ZkVoteGetTally
            | Self::VrfEpochSeed => S::new(&["bytes"], "bytes"),
            Self::QueryGetAccount => S::new(&["AccountId"], "Option<AccountView>"),
            Self::QueryGetAsset => S::new(&["AssetId"], "Option<AssetView>"),
            Self::QueryGetAssetDefinition => {
                S::new(&["AssetDefinitionId"], "Option<AssetDefinitionView>")
            }
            Self::QueryGetDomain => S::new(&["DomainId"], "Option<DomainView>"),
            Self::QueryGetNft => S::new(&["NftId"], "Option<NftView>"),
            Self::QueryPageAccounts => S::new(&["int", "int"], "QueryPage<AccountView>"),
            Self::QueryPageAssets => S::new(&["int", "int"], "QueryPage<AssetView>"),
            Self::QueryPageAssetDefinitions => {
                S::new(&["int", "int"], "QueryPage<AssetDefinitionView>")
            }
            Self::QueryPageDomains => S::new(&["int", "int"], "QueryPage<DomainView>"),
            Self::QueryPageNfts => S::new(&["int", "int"], "QueryPage<NftView>"),
            Self::QueryGetParameter | Self::QueryGetContractInstance => {
                S::new(&["Name|bytes"], "bytes")
            }
            Self::BuildSubmitBallotInline => S::new(
                &["string", "bytes", "bytes", "string", "bytes", "bytes"],
                "bytes",
            ),
            Self::BuildUnshieldInline => S::new(
                &[
                    "AssetDefinitionId",
                    "AccountId",
                    "int",
                    "bytes",
                    "bytes?",
                    "string",
                    "bytes",
                    "bytes",
                ],
                "bytes",
            ),
            Self::RecordSccpMessage | Self::ScExecuteSubmitBallot | Self::ScExecuteUnshield => {
                S::new(&["bytes"], "()")
            }
            Self::ExecuteQuery => S::new(&["bytes"], "bytes"),
            Self::ResolveAccountAlias => S::new(&["string|bytes"], "AccountId"),
            Self::SubscriptionBill | Self::SubscriptionRecordUsage => S::new(&[], "()"),
            Self::GetAccountBalance => S::new(&["AccountId", "AssetDefinitionId"], "quantity"),
            Self::GetPublicInput => S::new(&["Name"], "bytes"),
            Self::DebugPrint => S::new(&["int"], "()"),
            Self::DebugLog => S::new(&["string"], "()"),
            Self::Assert => S::new(&["bool", "string|int?"], "()"),
            Self::Require => S::new(&["bool", "ErrorEnum::Variant"], "()"),
            Self::Info => S::new(&["string|int"], "()"),
            Self::AssertEq => S::new(&["int", "int"], "()"),
            Self::TestInvokeEntrypoint => S::new(&["string", "Json"], "T"),
            Self::TestInvokeEntrypointAs => S::new(&["string", "string", "Json"], "T"),
            Self::TestExpectRejectAs => S::new(&["string", "string", "Json"], "()"),
            Self::TestActorAccount => S::new(&["string"], "AccountId"),
            Self::TestActorPublicKey => S::new(&["string"], "bytes"),
            Self::TestActorSign => S::new(&["string", "bytes"], "bytes"),
            Self::SetAccountDetail => S::new(&["AccountId", "Name", "Json"], "()"),
            Self::MintAsset | Self::BurnAsset => {
                S::new(&["AccountId", "AssetDefinitionId", "quantity"], "()")
            }
            Self::TransferAsset => S::new(
                &[
                    "AccountId",
                    "AccountId",
                    "AssetDefinitionId",
                    "quantity",
                    "DataSpaceId",
                ],
                "()",
            ),
            Self::SetAssetTransferFreeze => {
                S::new(&["AccountId", "AssetDefinitionId", "bool"], "()")
            }
            Self::SetAssetTransferDailyLimit => S::new(
                &["AccountId", "AssetDefinitionId", "Option<quantity>"],
                "()",
            ),
            Self::AccountRecoveryPropose => S::new(&["string", "AccountId"], "()"),
            Self::AccountRecoveryApprove
            | Self::AccountRecoveryCancel
            | Self::AccountRecoveryFinalize => S::new(&["string"], "()"),
            Self::NftMintAsset => S::new(&["NftId", "AccountId"], "()"),
            Self::NftSetMetadata => S::new(&["NftId", "Name", "Json"], "()"),
            Self::NftBurnAsset => S::new(&["NftId"], "()"),
            Self::NftTransferAsset => S::new(&["AccountId", "NftId", "AccountId"], "()"),
            Self::RegisterDomain | Self::UnregisterDomain => S::new(&["DomainId"], "()"),
            Self::TransferDomain => S::new(&["AccountId", "DomainId|Name", "AccountId"], "()"),
            Self::RegisterAccount | Self::UnregisterAccount => S::new(&["AccountId"], "()"),
            Self::RegisterAsset => S::new(&["AssetDefinitionId", "string", "int", "int"], "()"),
            Self::CreateNewAsset => S::new(
                &["AssetDefinitionId", "string", "int", "AccountId", "int"],
                "()",
            ),
            Self::UnregisterAsset => S::new(&["AssetDefinitionId"], "()"),
            Self::RegisterPeer | Self::UnregisterPeer => S::new(&["Json"], "()"),
            Self::CreateTrigger | Self::RegisterTrigger => S::new(&["Json"], "()"),
            Self::RemoveTrigger | Self::UnregisterTrigger => S::new(&["Name"], "()"),
            Self::SetTriggerEnabled => S::new(&["Name", "int"], "()"),
            Self::CreateRole => S::new(&["Name", "Json"], "()"),
            Self::DeleteRole => S::new(&["Name"], "()"),
            Self::GrantRole | Self::RevokeRole => S::new(&["AccountId", "Name"], "()"),
            Self::GrantPermission | Self::RevokePermission => {
                S::new(&["AccountId", "Name|Json"], "()")
            }
            Self::GrantContractEntrypoint | Self::RevokeContractEntrypoint => {
                S::new(&["AccountId", "string"], "()")
            }
            Self::EscrowOpenOffer => {
                S::new(&["Name", "AssetDefinitionId", "quantity", "bytes?"], "()")
            }
            Self::EscrowAccept
            | Self::EscrowMarkPaymentSent
            | Self::EscrowRelease
            | Self::EscrowCancel => S::new(&["Name"], "()"),
            Self::EscrowOpenDispute => S::new(&["Name", "bytes?"], "()"),
            Self::EscrowResolveDispute => S::new(&["Name", "quantity", "quantity", "bytes?"], "()"),
            Self::AnonymousEscrowOpenOffer
            | Self::AnonymousEscrowRelease
            | Self::AnonymousEscrowCancel
            | Self::AnonymousEscrowResolveDispute => S::new(&["bytes"], "()"),
            Self::AnonymousEscrowAccept | Self::AnonymousEscrowMarkPaymentSent => {
                S::new(&["Name"], "()")
            }
            Self::AnonymousEscrowOpenDispute => S::new(&["Name", "bytes?"], "()"),
            Self::GetPrivateInput => S::new(&["int"], "Secret<int>"),
            Self::UseNullifier => S::new(&["int"], "()"),
            Self::CommitOutput | Self::CreateNftsForAllUsers => S::new(&[], "()"),
            Self::SetExecutionDepth => S::new(&["int"], "()"),
            Self::TransferV1BatchBegin | Self::TransferV1BatchEnd => S::new(&[], "()"),
            Self::TransferV1BatchApply => S::new(&["bytes"], "()"),
            Self::TransferBatch => S::new(
                &["(AccountId,AccountId,AssetDefinitionId,quantity)..."],
                "()",
            ),
            Self::AxtBegin => S::new(&["AxtDescriptor"], "()"),
            Self::AxtTouch => S::new(&["DataSpaceId", "bytes"], "AssetHandle"),
            Self::VerifyDsProof => S::new(&["DataSpaceId", "ProofBlob"], "bool"),
            Self::UseAssetHandle => S::new(&["AssetHandle", "bytes", "ProofBlob?"], "()"),
            Self::AxtCommit => S::new(&[], "()"),
            Self::DeactivateContractInstance
            | Self::RemoveSmartContractBytes
            | Self::RegisterSmartContractCode
            | Self::RegisterSmartContractBytes
            | Self::ActivateContractInstance => S::new(&["bytes"], "()"),
            Self::ZkVerifyTransfer
            | Self::ZkVerifyUnshield
            | Self::ZkVerifyBatch
            | Self::ZkVoteVerifyBallot
            | Self::ZkVoteVerifyTally => S::new(&["bytes"], "()"),
            Self::VrfVerify => S::new(&["bytes"], "bytes"),
            Self::VrfVerifyBatch => S::new(&["bytes"], "bytes"),
            Self::Sm3Hash
            | Self::Sha256Hash
            | Self::Sha3Hash
            | Self::Blake2b256Hash
            | Self::Keccak256Hash
            | Self::IrohaHash => S::new(&["bytes"], "bytes"),
            Self::Sm2Verify => S::new(&["bytes", "bytes", "bytes", "bytes?"], "bool"),
            Self::VerifySignature => S::new(&["bytes", "bytes", "bytes", "int"], "bool"),
            Self::Sm4GcmSeal | Self::Sm4GcmOpen => {
                S::new(&["bytes", "bytes", "bytes", "bytes"], "bytes")
            }
            Self::Sm4CcmSeal | Self::Sm4CcmOpen => {
                S::new(&["bytes", "bytes", "bytes", "bytes", "int?"], "bytes")
            }
            Self::Alloc | Self::GrowHeap => S::new(&["int"], "int"),
            Self::ProveExecution => S::new(&[], "bytes"),
            Self::VerifyProof => S::new(&["bytes"], "bool"),
            Self::GetMerklePath => S::new(&["int", "int", "int?"], "int"),
            Self::GetMerkleCompact | Self::GetRegisterMerkleCompact => {
                S::new(&["int", "int", "int?", "int?"], "int")
            }
            Self::SoracloudReadCommittedState
            | Self::SoracloudEmitStateMutation
            | Self::SoracloudEmitMailboxMessage
            | Self::SoracloudAppendJournal
            | Self::SoracloudPublishCheckpoint
            | Self::SoracloudReadSecret
            | Self::SoracloudReadCredential
            | Self::SoracloudEgressFetch
            | Self::SoracloudReadConfig
            | Self::SoracloudReadSecretEnvelope => {
                S::new(&["SoracloudRequest"], "SoracloudResponse")
            }
            Self::AddSignatory | Self::RemoveSignatory => S::new(&["AccountId", "Json"], "()"),
            Self::SetAccountQuorum => S::new(&["AccountId", "int"], "()"),
            Self::Path => S::new(&["Name", "int|bytes"], "Name"),
            Self::NameDecode => S::new(&["bytes"], "Name"),
            Self::TlvEq => S::new(&["pointer-ABI", "pointer-ABI"], "bool"),
            Self::TlvLen => S::new(&["pointer-ABI"], "int"),
            Self::PointerToNorito => S::new(&["pointer-ABI"], "bytes"),
            Self::JsonObject => S::new(&[], "Json"),
            Self::JsonSetInt | Self::JsonSetIntDirect => S::new(&["Json", "Name", "int"], "Json"),
            Self::JsonSetAccountId | Self::JsonSetAccountIdDirect => {
                S::new(&["Json", "Name", "AccountId"], "Json")
            }
            Self::EncodeInt => S::new(&["int"], "bytes"),
            Self::DecodeInt => S::new(&["bytes"], "int"),
            Self::EncodeJson => S::new(&["Json"], "bytes"),
            Self::DecodeJson => S::new(&["bytes"], "Json"),
            Self::JsonGetIntDirect | Self::GetInt => S::new(&["Json", "Name"], "Option<int>"),
            Self::JsonGetDecimalDirect | Self::GetDecimal => {
                S::new(&["Json", "Name"], "Option<decimal>")
            }
            Self::JsonGetQuantityDirect | Self::GetQuantity => {
                S::new(&["Json", "Name"], "Option<quantity>")
            }
            Self::JsonGetJsonDirect | Self::GetJson => S::new(&["Json", "Name"], "Option<Json>"),
            Self::JsonGetNameDirect | Self::GetName => S::new(&["Json", "Name"], "Option<Name>"),
            Self::JsonGetAccountIdDirect | Self::GetAccountId => {
                S::new(&["Json", "Name"], "Option<AccountId>")
            }
            Self::JsonGetAssetDefinitionIdDirect | Self::GetAssetDefinitionId => {
                S::new(&["Json", "Name"], "Option<AssetDefinitionId>")
            }
            Self::JsonGetNftIdDirect | Self::GetNftId => S::new(&["Json", "Name"], "Option<NftId>"),
            Self::JsonGetBlobHexDirect | Self::GetBlobHex => {
                S::new(&["Json", "Name"], "Option<bytes>")
            }
            Self::BuildPathKeyNoritoDirect => S::new(&["Name", "bytes"], "Name"),
            Self::SchemaEncode | Self::SchemaEncodeDirect => S::new(&["Name", "Json"], "bytes"),
            Self::SchemaDecode | Self::SchemaDecodeDirect => S::new(&["Name", "bytes"], "Json"),
            Self::SchemaInfo | Self::SchemaInfoDirect => S::new(&["Name"], "Json"),
            Self::NumericToInt | Self::NumericToIntDirect => S::new(&["wide-numeric"], "int"),
            Self::NumericNeg | Self::NumericNegDirect => S::new(&["quantity"], "quantity"),
            Self::NumericAdd
            | Self::NumericSub
            | Self::NumericMul
            | Self::NumericDiv
            | Self::NumericRem
            | Self::NumericAddDirect
            | Self::NumericSubDirect
            | Self::NumericMulDirect
            | Self::NumericDivDirect
            | Self::NumericRemDirect => S::new(&["wide-numeric", "same-as-arg0"], "same-as-arg0"),
            Self::NumericEq
            | Self::NumericNe
            | Self::NumericLt
            | Self::NumericLe
            | Self::NumericGt
            | Self::NumericGe
            | Self::NumericEqDirect
            | Self::NumericNeDirect
            | Self::NumericLtDirect
            | Self::NumericLeDirect
            | Self::NumericGtDirect
            | Self::NumericGeDirect => S::new(&["wide-numeric", "same-as-arg0"], "bool"),
            Self::WrappingNeg | Self::Isqrt | Self::Abs => S::new(&["int"], "int"),
            Self::WrappingAdd | Self::WrappingSub | Self::WrappingMul => {
                S::new(&["int", "int"], "int")
            }
            Self::Min | Self::Max | Self::DivCeil | Self::Gcd | Self::Mean => {
                S::new(&["int", "int"], "int")
            }
            Self::Poseidon2 | Self::Valcom => S::new(&["int", "int"], "int"),
            Self::Poseidon6 => S::new(&["int", "int", "int", "int", "int", "int"], "int"),
            Self::Pubkgen => S::new(&["int|Secret<int>"], "int"),
            Self::SetVl => S::new(&["int"], "()"),
            Self::TriggerEvent => S::new(&[], "Json"),
            Self::Authority | Self::SysvarAuthority => S::new(&[], "AccountId"),
            Self::ContractSubject => S::new(&[], "AccountId"),
            Self::CurrentTimeMs | Self::BlockHeight | Self::BlockTimeMs => S::new(&[], "int"),
            Self::ChainId | Self::ContractAddress | Self::Entrypoint => S::new(&[], "bytes"),
        };

        match self {
            Self::PointerConstructor(_) => signature.with_names(&["value"]),
            Self::Contains => signature.with_names(&["map", "key"]),
            Self::GetOrDefault | Self::GetOr | Self::Ensure => {
                signature.with_names(&["map", "key", "default"])
            }
            Self::StateMapRemove => signature.with_names(&["map", "key"]),
            Self::KeysTake2 | Self::ValuesTake2 | Self::KeysValuesTake2 => {
                signature.with_names(&["map", "offset", "limit"])
            }
            Self::StateGet
            | Self::StateDel
            | Self::StateHas
            | Self::StateLen
            | Self::StateCount => signature.with_names(&["path"]),
            Self::StateSet => signature.with_names(&["path", "value"]),
            Self::StateKeys => signature.with_names(&["path", "offset", "limit"]),
            Self::QueryGetAccount => signature.with_names(&["id"]),
            Self::QueryGetAsset => signature.with_names(&["id"]),
            Self::QueryGetAssetDefinition => signature.with_names(&["id"]),
            Self::QueryGetDomain => signature.with_names(&["id"]),
            Self::QueryGetNft => signature.with_names(&["id"]),
            Self::QueryPageAccounts
            | Self::QueryPageAssets
            | Self::QueryPageAssetDefinitions
            | Self::QueryPageDomains
            | Self::QueryPageNfts => signature.with_names(&["offset", "limit"]),
            Self::QueryGetParameter | Self::QueryGetContractInstance => {
                signature.with_names(&["name"])
            }
            Self::QueryExecuteNorito | Self::QueryGetContractManifest | Self::ExecuteQuery => {
                signature.with_names(&["query"])
            }
            Self::BuildSubmitBallotInline => signature.with_names(&[
                "election_id",
                "ciphertext",
                "nullifier",
                "backend",
                "proof",
                "verification_key",
            ]),
            Self::BuildUnshieldInline => signature.with_names(&[
                "asset_definition",
                "destination",
                "amount",
                "inputs",
                "outputs",
                "backend",
                "proof",
                "verification_key",
            ]),
            Self::ResolveAccountAlias => signature.with_names(&["alias"]),
            Self::GetAccountBalance => signature.with_names(&["account", "asset_definition"]),
            Self::GetPublicInput => signature.with_names(&["name"]),
            Self::Assert => signature.with_names(&["condition", "message"]),
            Self::Require => signature.with_names(&["condition", "error"]),
            Self::AssertEq => signature.with_names(&["actual", "expected"]),
            Self::TestInvokeEntrypoint => signature.with_names(&["entrypoint", "arguments"]),
            Self::TestInvokeEntrypointAs | Self::TestExpectRejectAs => {
                signature.with_names(&["actor", "entrypoint", "arguments"])
            }
            Self::TestActorAccount | Self::TestActorPublicKey => signature.with_names(&["actor"]),
            Self::TestActorSign => signature.with_names(&["actor", "payload"]),
            Self::SetAccountDetail => signature.with_names(&["account", "key", "value"]),
            Self::MintAsset | Self::BurnAsset => {
                signature.with_names(&["account", "asset_definition", "amount"])
            }
            Self::TransferAsset => signature.with_names(&[
                "source",
                "destination",
                "asset_definition",
                "amount",
                "dataspace",
            ]),
            Self::SetAssetTransferFreeze => {
                signature.with_names(&["account", "asset_definition", "frozen"])
            }
            Self::SetAssetTransferDailyLimit => {
                signature.with_names(&["account", "asset_definition", "cap"])
            }
            Self::AccountRecoveryPropose => signature.with_names(&["alias", "replacement"]),
            Self::AccountRecoveryApprove
            | Self::AccountRecoveryCancel
            | Self::AccountRecoveryFinalize => signature.with_names(&["alias"]),
            Self::NftMintAsset => signature.with_names(&["nft", "owner"]),
            Self::NftSetMetadata => signature.with_names(&["nft", "key", "value"]),
            Self::NftBurnAsset => signature.with_names(&["nft"]),
            Self::NftTransferAsset => signature.with_names(&["source", "nft", "destination"]),
            Self::RegisterDomain | Self::UnregisterDomain => signature.with_names(&["domain"]),
            Self::TransferDomain => signature.with_names(&["source", "domain", "destination"]),
            Self::RegisterAccount | Self::UnregisterAccount => signature.with_names(&["account"]),
            Self::RegisterAsset => {
                signature.with_names(&["asset_definition", "name", "scale", "mintable"])
            }
            Self::CreateNewAsset => {
                signature.with_names(&["asset_definition", "name", "scale", "owner", "mintable"])
            }
            Self::UnregisterAsset => signature.with_names(&["asset_definition"]),
            Self::SetTriggerEnabled => signature.with_names(&["trigger", "enabled"]),
            Self::CreateRole => signature.with_names(&["role", "permissions"]),
            Self::GrantRole | Self::RevokeRole => signature.with_names(&["account", "role"]),
            Self::GrantPermission | Self::RevokePermission => {
                signature.with_names(&["account", "permission"])
            }
            Self::GrantContractEntrypoint | Self::RevokeContractEntrypoint => {
                signature.with_names(&["account", "entrypoint"])
            }
            Self::EscrowOpenOffer => {
                signature.with_names(&["offer", "asset_definition", "amount", "evidence"])
            }
            Self::EscrowOpenDispute | Self::AnonymousEscrowOpenDispute => {
                signature.with_names(&["offer", "evidence"])
            }
            Self::EscrowResolveDispute => {
                signature.with_names(&["offer", "buyer_amount", "seller_amount", "evidence"])
            }
            Self::TransferV1BatchApply => signature.with_names(&["batch"]),
            Self::AxtTouch => signature.with_names(&["dataspace", "proof"]),
            Self::VerifyDsProof => signature.with_names(&["dataspace", "proof"]),
            Self::UseAssetHandle => signature.with_names(&["handle", "operation", "proof"]),
            Self::VrfVerify => signature.with_names(&["request"]),
            Self::Sm2Verify => {
                signature.with_names(&["message", "signature", "public_key", "distid"])
            }
            Self::VerifySignature => {
                signature.with_names(&["message", "signature", "public_key", "scheme"])
            }
            Self::Sm4GcmSeal | Self::Sm4GcmOpen => {
                signature.with_names(&["key", "nonce", "aad", "payload"])
            }
            Self::Sm4CcmSeal | Self::Sm4CcmOpen => {
                signature.with_names(&["key", "nonce", "aad", "payload", "tag_length"])
            }
            Self::GetMerklePath => signature.with_names(&["address", "output", "root_output"]),
            Self::GetMerkleCompact | Self::GetRegisterMerkleCompact => {
                signature.with_names(&["address_or_register", "output", "max_depth", "root_output"])
            }
            Self::TlvEq => signature.with_names(&["left", "right"]),
            Self::JsonSetInt | Self::JsonSetIntDirect => {
                signature.with_names(&["object", "key", "value"])
            }
            Self::JsonSetAccountId | Self::JsonSetAccountIdDirect => {
                signature.with_names(&["object", "key", "value"])
            }
            Self::JsonGetIntDirect
            | Self::GetInt
            | Self::JsonGetDecimalDirect
            | Self::GetDecimal
            | Self::JsonGetQuantityDirect
            | Self::GetQuantity
            | Self::JsonGetJsonDirect
            | Self::GetJson
            | Self::JsonGetNameDirect
            | Self::GetName
            | Self::JsonGetAccountIdDirect
            | Self::GetAccountId
            | Self::JsonGetAssetDefinitionIdDirect
            | Self::GetAssetDefinitionId
            | Self::JsonGetNftIdDirect
            | Self::GetNftId
            | Self::JsonGetBlobHexDirect
            | Self::GetBlobHex => signature.with_names(&["object", "key"]),
            Self::NumericAdd
            | Self::NumericSub
            | Self::NumericMul
            | Self::NumericDiv
            | Self::NumericRem
            | Self::NumericEq
            | Self::NumericNe
            | Self::NumericLt
            | Self::NumericLe
            | Self::NumericGt
            | Self::NumericGe
            | Self::NumericAddDirect
            | Self::NumericSubDirect
            | Self::NumericMulDirect
            | Self::NumericDivDirect
            | Self::NumericRemDirect
            | Self::NumericEqDirect
            | Self::NumericNeDirect
            | Self::NumericLtDirect
            | Self::NumericLeDirect
            | Self::NumericGtDirect
            | Self::NumericGeDirect
            | Self::WrappingAdd
            | Self::WrappingSub
            | Self::WrappingMul
            | Self::Min
            | Self::Max
            | Self::DivCeil
            | Self::Gcd
            | Self::Mean
            | Self::Poseidon2
            | Self::Valcom => signature.with_names(&["left", "right"]),
            Self::Poseidon6 => signature.with_names(&["a", "b", "c", "d", "e", "f"]),
            _ => signature,
        }
    }

    /// Return the source argument policy attached to this builtin.
    pub const fn call_policy(self) -> BuiltinCallPolicy {
        if matches!(
            self,
            Self::KeysTake2
                | Self::ValuesTake2
                | Self::KeysValuesTake2
                | Self::StateKeys
                | Self::QueryPageAccounts
                | Self::QueryPageAssets
                | Self::QueryPageAssetDefinitions
                | Self::QueryPageDomains
                | Self::QueryPageNfts
        ) {
            BuiltinCallPolicy::Pagination
        } else {
            BuiltinCallPolicy::Flexible
        }
    }

    /// Return the canonical builtin registry record.
    pub const fn spec(self) -> BuiltinSpec {
        BuiltinSpec {
            name: self.source_name(),
            effects: self.effects(),
            access: self.access(),
            mode: self.mode(),
            surface: self.surface(),
            gas: self.gas_class(),
            lowering: self.lowering(),
            operation_syscalls: self.operation_syscalls(),
            syscall: self.syscall(),
            signature: self.signature(),
            call_policy: self.call_policy(),
        }
    }

    /// Whether the builtin is a JSON payload helper that public/view entrypoints
    /// must reject in favor of typed parameters.
    pub const fn is_payload_helper(self) -> bool {
        matches!(
            self,
            Self::GetInt
                | Self::GetDecimal
                | Self::GetQuantity
                | Self::GetJson
                | Self::GetName
                | Self::GetAccountId
                | Self::GetAssetDefinitionId
                | Self::GetNftId
                | Self::GetBlobHex
                | Self::TriggerEvent
                | Self::JsonGetIntDirect
                | Self::JsonGetDecimalDirect
                | Self::JsonGetQuantityDirect
                | Self::JsonGetJsonDirect
                | Self::JsonGetNameDirect
                | Self::JsonGetAccountIdDirect
                | Self::JsonGetAssetDefinitionIdDirect
                | Self::JsonGetNftIdDirect
                | Self::JsonGetBlobHexDirect
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        Builtin, BuiltinAccess, BuiltinCallPolicy, BuiltinEffects, BuiltinGasClass,
        BuiltinLowering, BuiltinMode, BuiltinSurface, PointerConstructor,
    };

    #[test]
    fn release_mutators_are_effectful_in_canonical_registry() {
        for name in [
            "transfer_asset",
            "create_new_asset",
            "create_nfts_for_all_users",
            "transfer_batch",
            "axt_begin",
            "axt_touch",
            "use_asset_handle",
            "axt_commit",
        ] {
            let builtin = Builtin::from_name(name).expect("registered builtin");
            assert_eq!(builtin.effects(), BuiltinEffects::HOST, "{name}");
            assert_eq!(builtin.access(), BuiltinAccess::LedgerWrite, "{name}");
            assert!(!builtin.spec().name.is_empty());
        }
    }

    #[test]
    fn raw_and_private_builtins_have_restricted_modes() {
        assert_eq!(Builtin::Alloc.mode(), BuiltinMode::CompilerInternal);
        assert_eq!(Builtin::DebugPrint.mode(), BuiltinMode::CompilerInternal);
        assert_eq!(Builtin::DebugLog.mode(), BuiltinMode::CompilerInternal);
        assert_eq!(
            Builtin::SetExecutionDepth.mode(),
            BuiltinMode::CompilerInternal
        );
        assert_eq!(Builtin::SetVl.mode(), BuiltinMode::CompilerInternal);
        assert_eq!(
            Builtin::NumericAddDirect.mode(),
            BuiltinMode::CompilerInternal
        );
        assert_eq!(Builtin::GetPrivateInput.mode(), BuiltinMode::ZkOnly);
        assert_eq!(Builtin::Assert.mode(), BuiltinMode::TestOnly);
        assert_eq!(Builtin::AssertEq.mode(), BuiltinMode::TestOnly);
        for builtin in [
            Builtin::TestInvokeEntrypoint,
            Builtin::TestInvokeEntrypointAs,
            Builtin::TestExpectRejectAs,
            Builtin::TestActorAccount,
            Builtin::TestActorPublicKey,
            Builtin::TestActorSign,
        ] {
            assert_eq!(builtin.mode(), BuiltinMode::TestFunctionOnly, "{builtin:?}");
            assert!(builtin.source_name().starts_with("test::"), "{builtin:?}");
            assert_eq!(
                Builtin::from_source_name(builtin.name()),
                None,
                "{builtin:?}"
            );
        }
        for builtin in [
            Builtin::PointerConstructor(PointerConstructor::Domain),
            Builtin::PointerConstructor(PointerConstructor::Blob),
            Builtin::PointerConstructor(PointerConstructor::NoritoBytes),
            Builtin::PointerConstructor(PointerConstructor::AxtDescriptor),
            Builtin::PointerConstructor(PointerConstructor::AssetHandle),
            Builtin::PointerConstructor(PointerConstructor::ProofBlob),
            Builtin::PointerConstructor(PointerConstructor::SoracloudRequest),
            Builtin::PointerConstructor(PointerConstructor::SoracloudResponse),
            Builtin::PointerToNorito,
            Builtin::EncodeInt,
            Builtin::DecodeInt,
            Builtin::EncodeJson,
            Builtin::DecodeJson,
            Builtin::SchemaEncode,
            Builtin::SchemaDecode,
            Builtin::DebugPrint,
            Builtin::DebugLog,
            Builtin::SetExecutionDepth,
            Builtin::DeactivateContractInstance,
            Builtin::RemoveSmartContractBytes,
            Builtin::RegisterSmartContractCode,
            Builtin::RegisterSmartContractBytes,
            Builtin::ActivateContractInstance,
            Builtin::SetVl,
            Builtin::AxtBegin,
            Builtin::AxtTouch,
            Builtin::VerifyDsProof,
            Builtin::UseAssetHandle,
            Builtin::AxtCommit,
            Builtin::SoracloudReadCommittedState,
            Builtin::SoracloudEmitStateMutation,
            Builtin::SoracloudEmitMailboxMessage,
            Builtin::SoracloudAppendJournal,
            Builtin::SoracloudPublishCheckpoint,
            Builtin::SoracloudReadSecret,
            Builtin::SoracloudReadCredential,
            Builtin::SoracloudEgressFetch,
            Builtin::SoracloudReadConfig,
            Builtin::SoracloudReadSecretEnvelope,
        ] {
            assert_eq!(builtin.mode(), BuiltinMode::CompilerInternal, "{builtin:?}");
            assert_eq!(builtin.surface(), BuiltinSurface::CompilerInternal);
            assert_eq!(Builtin::from_source_name(builtin.source_name()), None);
        }
        assert_eq!(
            Builtin::GetPrivateInput.syscall(),
            Some(ivm_abi::syscalls::SYSCALL_GET_PRIVATE_INPUT)
        );
    }

    #[test]
    fn vm_local_host_operations_do_not_claim_ledger_access() {
        for builtin in [Builtin::CommitOutput, Builtin::SetExecutionDepth] {
            assert_eq!(builtin.effects(), BuiltinEffects::HOST, "{builtin:?}");
            assert_eq!(builtin.spec().access, BuiltinAccess::None, "{builtin:?}");
        }
    }

    #[test]
    fn public_pointer_constructors_have_one_typed_canonical_spelling() {
        for (constructor, canonical) in [
            (PointerConstructor::AccountId, "AccountId::parse"),
            (
                PointerConstructor::AssetDefinition,
                "AssetDefinitionId::parse",
            ),
            (PointerConstructor::AssetId, "AssetId::parse"),
            (PointerConstructor::NftId, "NftId::parse"),
            (PointerConstructor::Name, "Name::parse"),
            (PointerConstructor::Json, "Json::parse"),
            (PointerConstructor::DomainId, "DomainId::parse"),
            (PointerConstructor::DataSpaceId, "DataSpaceId::parse"),
        ] {
            let builtin = Builtin::PointerConstructor(constructor);
            assert_eq!(builtin.source_name(), canonical);
            assert_eq!(Builtin::from_source_name(canonical), Some(builtin));
            assert_eq!(Builtin::from_source_name(constructor.name()), None);
            assert_eq!(builtin.signature().parameters, &["string"]);
        }
    }

    #[test]
    fn registry_is_exhaustive_and_canonical_names_round_trip() {
        let mut variants = HashSet::new();
        let mut internal_names = HashSet::new();
        let mut source_names = HashSet::new();
        for builtin in Builtin::ALL {
            assert!(
                variants.insert(*builtin),
                "duplicate registry variant {builtin:?}"
            );
            assert!(
                internal_names.insert(builtin.name()),
                "duplicate internal builtin spelling `{}`",
                builtin.name()
            );
            assert_eq!(
                Builtin::from_name(builtin.name()),
                Some(*builtin),
                "internal builtin spelling must resolve uniquely for {builtin:?}"
            );
            let spec = builtin.spec();
            assert!(!spec.name.is_empty(), "{builtin:?}");
            assert!(!spec.signature.return_type.is_empty(), "{builtin:?}");
            assert!(
                spec.signature
                    .parameters
                    .iter()
                    .all(|parameter| !parameter.is_empty()),
                "{builtin:?}"
            );
            if matches!(
                spec.surface,
                BuiltinSurface::Function | BuiltinSurface::FunctionOrMethod
            ) {
                assert!(
                    source_names.insert(spec.name),
                    "duplicate source spelling `{}`",
                    spec.name
                );
                assert_eq!(
                    Builtin::from_source_name(builtin.source_name()),
                    Some(*builtin),
                    "canonical source spelling must resolve uniquely for {builtin:?}"
                );
            }
            if matches!(
                spec.surface,
                BuiltinSurface::Function | BuiltinSurface::FunctionOrMethod
            ) && (builtin.effects() != BuiltinEffects::NONE
                || builtin.access() != BuiltinAccess::None)
            {
                assert!(
                    builtin.source_name().contains("::"),
                    "effectful builtin {builtin:?} must use a capability namespace"
                );
            }
        }
    }

    #[test]
    fn wrapping_arithmetic_is_explicit_and_pure() {
        for (name, source_name) in [
            ("wrapping_add", "math::wrapping_add"),
            ("wrapping_sub", "math::wrapping_sub"),
            ("wrapping_mul", "math::wrapping_mul"),
            ("wrapping_neg", "math::wrapping_neg"),
        ] {
            let builtin = Builtin::from_name(name).expect("registered wrapping builtin");
            assert_eq!(builtin.name(), name);
            assert_eq!(builtin.source_name(), source_name);
            assert_eq!(Builtin::from_source_name(source_name), Some(builtin));
            assert_eq!(Builtin::from_source_name(name), None);
            assert_eq!(builtin.effects(), BuiltinEffects::NONE);
            assert_eq!(builtin.access(), BuiltinAccess::None);
            assert_eq!(builtin.mode(), BuiltinMode::Any);
            assert_eq!(builtin.syscall(), None);
        }
    }

    #[test]
    fn signatures_publish_named_call_metadata_without_arity_drift() {
        for builtin in Builtin::ALL.iter().copied() {
            let signature = builtin.signature();
            assert_eq!(
                signature.parameter_names.len(),
                signature.parameters.len(),
                "{builtin:?}"
            );
            let mut unique = std::collections::BTreeSet::new();
            for name in signature.parameter_names {
                assert!(!name.is_empty(), "{builtin:?}");
                assert!(
                    unique.insert(name),
                    "duplicate parameter name on {builtin:?}"
                );
            }
        }
        assert_eq!(
            Builtin::TransferAsset.signature().parameter_names,
            &[
                "source",
                "destination",
                "asset_definition",
                "amount",
                "dataspace"
            ]
        );
        assert_eq!(
            Builtin::StateKeys.call_policy(),
            BuiltinCallPolicy::Pagination
        );
    }

    #[test]
    fn native_transfer_control_and_recovery_registry_is_exact() {
        use ivm_abi::syscalls as s;

        for (builtin, name, source_name, syscall, parameters, parameter_names) in [
            (
                Builtin::SetAssetTransferFreeze,
                "set_asset_transfer_freeze",
                "ledger::asset::set_transfer_freeze",
                s::SYSCALL_SET_ASSET_TRANSFER_FREEZE,
                &["AccountId", "AssetDefinitionId", "bool"][..],
                &["account", "asset_definition", "frozen"][..],
            ),
            (
                Builtin::SetAssetTransferDailyLimit,
                "set_asset_transfer_daily_limit",
                "ledger::asset::set_transfer_daily_limit",
                s::SYSCALL_SET_ASSET_TRANSFER_DAILY_LIMIT,
                &["AccountId", "AssetDefinitionId", "Option<quantity>"][..],
                &["account", "asset_definition", "cap"][..],
            ),
            (
                Builtin::AccountRecoveryPropose,
                "account_recovery_propose",
                "ledger::account::recovery::propose",
                s::SYSCALL_ACCOUNT_RECOVERY_PROPOSE,
                &["string", "AccountId"][..],
                &["alias", "replacement"][..],
            ),
            (
                Builtin::AccountRecoveryApprove,
                "account_recovery_approve",
                "ledger::account::recovery::approve",
                s::SYSCALL_ACCOUNT_RECOVERY_APPROVE,
                &["string"][..],
                &["alias"][..],
            ),
            (
                Builtin::AccountRecoveryCancel,
                "account_recovery_cancel",
                "ledger::account::recovery::cancel",
                s::SYSCALL_ACCOUNT_RECOVERY_CANCEL,
                &["string"][..],
                &["alias"][..],
            ),
            (
                Builtin::AccountRecoveryFinalize,
                "account_recovery_finalize",
                "ledger::account::recovery::finalize",
                s::SYSCALL_ACCOUNT_RECOVERY_FINALIZE,
                &["string"][..],
                &["alias"][..],
            ),
        ] {
            assert_eq!(builtin.name(), name);
            assert_eq!(builtin.source_name(), source_name);
            assert_eq!(Builtin::from_name(name), Some(builtin));
            assert_eq!(Builtin::from_source_name(source_name), Some(builtin));
            assert_eq!(builtin.operation_syscalls(), &[syscall]);
            assert_eq!(builtin.syscall(), Some(syscall));
            assert_eq!(builtin.lowering(), BuiltinLowering::DirectSyscall);
            assert_eq!(builtin.effects(), BuiltinEffects::HOST);
            assert_eq!(builtin.access(), BuiltinAccess::LedgerWrite);
            let signature = builtin.signature();
            assert_eq!(signature.parameters, parameters);
            assert_eq!(signature.parameter_names, parameter_names);
            assert_eq!(signature.return_type, "()");
        }
    }

    #[test]
    fn contract_entrypoint_capability_registry_is_exact_and_namespaced() {
        use ivm_abi::syscalls as s;

        for (builtin, internal_name, source_name, syscall) in [
            (
                Builtin::GrantContractEntrypoint,
                "grant_contract_entrypoint",
                "ledger::contract::grant_entrypoint",
                s::SYSCALL_GRANT_CONTRACT_ENTRYPOINT,
            ),
            (
                Builtin::RevokeContractEntrypoint,
                "revoke_contract_entrypoint",
                "ledger::contract::revoke_entrypoint",
                s::SYSCALL_REVOKE_CONTRACT_ENTRYPOINT,
            ),
        ] {
            assert_eq!(builtin.name(), internal_name);
            assert_eq!(builtin.source_name(), source_name);
            assert_eq!(Builtin::from_name(internal_name), Some(builtin));
            assert_eq!(Builtin::from_source_name(source_name), Some(builtin));
            assert_eq!(Builtin::from_source_name(internal_name), None);
            assert_eq!(builtin.operation_syscalls(), &[syscall]);
            assert_eq!(builtin.syscall(), Some(syscall));
            assert_eq!(builtin.lowering(), BuiltinLowering::DirectSyscall);
            assert_eq!(builtin.effects(), BuiltinEffects::HOST);
            assert_eq!(builtin.access(), BuiltinAccess::LedgerWrite);
            let signature = builtin.signature();
            assert_eq!(signature.parameters, &["AccountId", "string"]);
            assert_eq!(signature.parameter_names, &["account", "entrypoint"]);
            assert_eq!(signature.return_type, "()");
        }
    }

    #[test]
    fn escrow_open_offer_registry_matches_the_lowered_host_abi() {
        let spec = Builtin::EscrowOpenOffer.spec();
        assert_eq!(
            spec.signature.parameters,
            &["Name", "AssetDefinitionId", "quantity", "bytes?"]
        );
        assert_eq!(spec.signature.return_type, "()");
        assert_eq!(
            spec.operation_syscalls,
            &[ivm_abi::syscalls::SYSCALL_ESCROW_OPEN_OFFER]
        );
        assert_eq!(
            spec.syscall,
            Some(ivm_abi::syscalls::SYSCALL_ESCROW_OPEN_OFFER)
        );
    }

    #[test]
    fn projected_core_query_registry_is_typed_and_pages_are_named_only() {
        for (singular, plural, id, view) in [
            (
                Builtin::QueryGetAccount,
                Builtin::QueryPageAccounts,
                "AccountId",
                "AccountView",
            ),
            (
                Builtin::QueryGetAsset,
                Builtin::QueryPageAssets,
                "AssetId",
                "AssetView",
            ),
            (
                Builtin::QueryGetAssetDefinition,
                Builtin::QueryPageAssetDefinitions,
                "AssetDefinitionId",
                "AssetDefinitionView",
            ),
            (
                Builtin::QueryGetDomain,
                Builtin::QueryPageDomains,
                "DomainId",
                "DomainView",
            ),
            (
                Builtin::QueryGetNft,
                Builtin::QueryPageNfts,
                "NftId",
                "NftView",
            ),
        ] {
            assert_eq!(singular.signature().parameters, &[id]);
            assert_eq!(singular.signature().return_type, format!("Option<{view}>"));
            assert_eq!(
                singular.operation_syscalls(),
                &[ivm_abi::syscalls::SYSCALL_CORE_QUERY_GET]
            );

            assert_eq!(plural.signature().parameters, &["int", "int"]);
            assert_eq!(plural.signature().parameter_names, &["offset", "limit"]);
            assert_eq!(plural.signature().return_type, format!("QueryPage<{view}>"));
            assert_eq!(plural.call_policy(), BuiltinCallPolicy::Pagination);
            assert_eq!(
                plural.operation_syscalls(),
                &[ivm_abi::syscalls::SYSCALL_CORE_QUERY_PAGE]
            );
        }
    }

    #[test]
    fn typed_json_getter_registry_returns_active_only_options() {
        for (getter, direct, payload) in [
            (Builtin::GetInt, Builtin::JsonGetIntDirect, "int"),
            (
                Builtin::GetQuantity,
                Builtin::JsonGetQuantityDirect,
                "quantity",
            ),
            (Builtin::GetJson, Builtin::JsonGetJsonDirect, "Json"),
            (Builtin::GetName, Builtin::JsonGetNameDirect, "Name"),
            (
                Builtin::GetAccountId,
                Builtin::JsonGetAccountIdDirect,
                "AccountId",
            ),
            (
                Builtin::GetAssetDefinitionId,
                Builtin::JsonGetAssetDefinitionIdDirect,
                "AssetDefinitionId",
            ),
            (Builtin::GetNftId, Builtin::JsonGetNftIdDirect, "NftId"),
            (Builtin::GetBlobHex, Builtin::JsonGetBlobHexDirect, "bytes"),
        ] {
            let expected = format!("Option<{payload}>");
            assert_eq!(getter.signature().return_type, expected);
            assert_eq!(direct.signature().return_type, expected);
        }
        assert_eq!(Builtin::GetQuantity.source_name(), "json::get_quantity");
    }

    #[test]
    fn source_visible_helpers_are_namespaced_except_language_intrinsics() {
        for builtin in Builtin::ALL {
            if builtin.surface() == BuiltinSurface::CompilerInternal {
                continue;
            }
            let source_name = builtin.source_name();
            let is_method = matches!(
                builtin.surface(),
                BuiltinSurface::MethodOnly | BuiltinSurface::FunctionOrMethod
            );
            assert!(
                source_name.contains("::") || is_method || source_name == "require",
                "source builtin {builtin:?} must be namespaced"
            );
        }
    }

    #[test]
    fn lowering_registry_is_fail_closed_and_gas_classified() {
        for builtin in Builtin::ALL {
            let spec = builtin.spec();
            match spec.lowering {
                BuiltinLowering::Instructions => {
                    assert!(spec.operation_syscalls.is_empty(), "{builtin:?}");
                    assert_eq!(spec.syscall, None, "{builtin:?}");
                }
                BuiltinLowering::DirectSyscall => {
                    assert_eq!(spec.operation_syscalls.len(), 1, "{builtin:?}");
                    assert_eq!(
                        spec.syscall,
                        Some(spec.operation_syscalls[0]),
                        "{builtin:?}"
                    );
                }
                BuiltinLowering::DerivedSyscalls => {
                    assert!(!spec.operation_syscalls.is_empty(), "{builtin:?}");
                    assert_eq!(spec.syscall, None, "{builtin:?}");
                }
            }
            if !spec.operation_syscalls.is_empty() {
                assert_ne!(spec.gas, BuiltinGasClass::Constant, "{builtin:?}");
            }
            if matches!(
                spec.access,
                BuiltinAccess::StateWrite | BuiltinAccess::LedgerWrite | BuiltinAccess::Dynamic
            ) {
                assert!(
                    spec.effects.host_side_effects
                        || spec.effects.emits_instructions
                        || spec.effects.mutates_durable_state,
                    "privileged builtin {builtin:?} must not under-report its effects"
                );
            }
        }
    }

    #[test]
    fn state_map_helpers_are_method_only() {
        for builtin in [
            Builtin::Contains,
            Builtin::GetOrDefault,
            Builtin::GetOr,
            Builtin::Ensure,
            Builtin::StateMapRemove,
        ] {
            assert_eq!(builtin.surface(), BuiltinSurface::MethodOnly, "{builtin:?}");
            assert_eq!(
                Builtin::from_source_name(builtin.name()),
                None,
                "{builtin:?}"
            );
        }
    }

    #[test]
    fn public_input_registry_matches_the_typed_bytes_surface() {
        let signature = Builtin::GetPublicInput.signature();
        assert_eq!(signature.parameters, &["Name"]);
        assert_eq!(signature.return_type, "bytes");
        assert_eq!(
            Builtin::GetPublicInput.source_name(),
            "context::public_input"
        );
    }

    #[test]
    fn compiler_internal_seiyaku_lifecycle_names_are_branded_but_not_source_visible() {
        for (builtin, branded, english) in [
            (
                Builtin::DeactivateContractInstance,
                "seiyaku::deactivate_instance",
                "contract::deactivate_instance",
            ),
            (
                Builtin::RemoveSmartContractBytes,
                "seiyaku::remove_code",
                "contract::remove_code",
            ),
            (
                Builtin::RegisterSmartContractCode,
                "seiyaku::register_code",
                "contract::register_code",
            ),
            (
                Builtin::RegisterSmartContractBytes,
                "seiyaku::register_bytes",
                "contract::register_bytes",
            ),
            (
                Builtin::ActivateContractInstance,
                "seiyaku::activate_instance",
                "contract::activate_instance",
            ),
        ] {
            assert_eq!(builtin.source_name(), branded);
            assert_eq!(builtin.surface(), BuiltinSurface::CompilerInternal);
            assert_eq!(Builtin::from_source_name(branded), None, "{branded}");
            assert_eq!(Builtin::from_source_name(english), None, "{english}");
        }
    }

    #[test]
    fn forbidden_raw_surfaces_do_not_resolve() {
        for name in [
            "call",
            "call_contract",
            "contract::call",
            "seiyaku::call",
            "execute_instruction",
            "execute_query",
            "query_execute_norito",
            "alloc",
            "grow_heap",
            "runtime::set_vector_length",
            "setvl",
            "debug_print",
            "debug_log",
            "debug::print_i64",
            "debug::log",
        ] {
            assert_eq!(Builtin::from_source_name(name), None, "{name}");
        }
    }
}
