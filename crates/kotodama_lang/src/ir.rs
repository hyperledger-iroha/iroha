//! Intermediate representation for Kotodama programs.
//!
//! The IR is a simple three-address code with basic blocks. Each temporary
//! value is assigned once and identified by a `Temp` index. Control flow is
//! expressed with explicit jumps between labeled blocks.

use std::collections::{BTreeSet, HashMap};

use iroha_primitives::numeric::Numeric;

use super::{
    ast::{BinaryOp, STATE_MAP_GET_INTRINSIC, UnaryOp},
    builtins::{Builtin, BuiltinLowering, PointerConstructor},
    semantic::{
        self, Type, TypedBlock, TypedExpr, TypedFunction, TypedItem, TypedParam, TypedProgram,
        TypedStateDecl, TypedStatement,
    },
};

pub const TEST_TRIGGER_EVENT_OVERRIDE_KEY: &str = "__koto_test_trigger_event_json";
const INVOKE_ENTRYPOINT_PREFIX: &str = "__invoke_entrypoint__";

fn state_map_base_name(expr: &semantic::TypedExpr) -> Option<String> {
    if let semantic::ExprKind::Ident(name) = &expr.expr {
        Some(name.clone())
    } else {
        None
    }
}

#[derive(Clone, Debug)]
struct StateMapSpec {
    key: Type,
    value: Type,
}

fn aggregate_components(ty: &Type) -> Option<Vec<Type>> {
    match semantic::resolve_struct_type(ty) {
        Type::Tuple(items) => Some(items),
        Type::Option(value) => Some(vec![Type::Bool, *value]),
        Type::Result(ok, err) => Some(vec![Type::Bool, *ok, *err]),
        _ => None,
    }
}

fn function_value_word_types(ty: &Type) -> Option<Vec<Type>> {
    fn append(ty: &Type, words: &mut Vec<Type>) {
        match semantic::resolve_struct_type(ty) {
            Type::Struct { fields, .. } => {
                for (_, field_ty) in fields {
                    append(&field_ty, words);
                }
            }
            Type::Tuple(items) => {
                for item in items {
                    append(&item, words);
                }
            }
            Type::Option(inner) => {
                words.push(Type::Bool);
                append(&inner, words);
            }
            Type::Result(ok, err) => {
                words.push(Type::Bool);
                append(&ok, words);
                append(&err, words);
            }
            leaf => words.push(leaf),
        }
    }

    if !matches!(
        semantic::resolve_struct_type(ty),
        Type::Struct { .. } | Type::Tuple(_) | Type::Option(_) | Type::Result(_, _)
    ) {
        return None;
    }
    let mut words = Vec::new();
    append(ty, &mut words);
    Some(words)
}

fn function_param_word_name(param: &str, index: usize) -> String {
    // `$` is not a source identifier character, so this compiler-owned name
    // cannot collide with a user parameter.
    format!("$abi${param}#{index}")
}

fn collect_state_handle_specs(name: &str, ty: &Type, out: &mut Vec<(String, Type)>) {
    let resolved = semantic::resolve_struct_type(ty);
    out.push((name.to_string(), resolved.clone()));
}

fn lowered_function_params(params: &[TypedParam]) -> Vec<String> {
    let mut lowered = Vec::new();
    for param in params {
        if param.is_state {
            let mut handles = Vec::new();
            collect_state_handle_specs(&param.name, &param.ty, &mut handles);
            lowered.extend(handles.into_iter().map(|(name, _)| name));
        } else if let Some(word_types) = function_value_word_types(&param.ty) {
            lowered.extend(
                word_types
                    .into_iter()
                    .enumerate()
                    .map(|(index, _)| function_param_word_name(&param.name, index)),
            );
        } else {
            lowered.push(param.name.clone());
        }
    }
    lowered
}

/// A virtual register or temporary value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Temp(pub usize);

/// Identifier for a basic block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Label(pub usize);

/// An entire lowered program.
#[derive(Debug, PartialEq)]
pub struct Program {
    pub functions: Vec<Function>,
}

/// A lowered function consisting of basic blocks.
#[derive(Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub blocks: Vec<BasicBlock>,
    pub entry: Label,
    pub location: super::ast::SourceLocation,
}

/// A single basic block in a function.
#[derive(Debug, PartialEq)]
pub struct BasicBlock {
    pub label: Label,
    pub instrs: Vec<Instr>,
    pub terminator: Terminator,
}

/// Non-control-flow instructions.
#[derive(Debug, PartialEq)]
pub enum Instr {
    Const {
        dest: Temp,
        value: i64,
    },
    /// Copy helper used by control-flow joins to materialize merged SSA temps.
    Copy {
        dest: Temp,
        src: Temp,
    },
    /// String literal constant used by pointer‑ABI codegen.
    StringConst {
        dest: Temp,
        value: String,
    },
    Binary {
        dest: Temp,
        op: BinaryOp,
        left: Temp,
        right: Temp,
    },
    /// Explicit modular `i64` addition, subtraction, or multiplication.
    WrappingBinary {
        dest: Temp,
        op: BinaryOp,
        left: Temp,
        right: Temp,
    },
    Unary {
        dest: Temp,
        op: UnaryOp,
        operand: Temp,
    },
    /// Explicit modular `i64` negation.
    WrappingNeg {
        dest: Temp,
        operand: Temp,
    },
    /// Convert a non-negative int (i64) to a Numeric NoritoBytes payload pointer (scale = 0).
    NumericFromInt {
        dest: Temp,
        value: Temp,
    },
    /// Convert a Numeric NoritoBytes payload pointer (scale = 0, unsigned) to an int (i64).
    NumericToInt {
        dest: Temp,
        value: Temp,
    },
    /// Numeric unary negation using a NoritoBytes payload.
    NumericNeg {
        dest: Temp,
        value: Temp,
    },
    /// Numeric arithmetic using NoritoBytes payloads.
    NumericBinary {
        dest: Temp,
        op: BinaryOp,
        left: Temp,
        right: Temp,
    },
    /// Numeric comparison using NoritoBytes payloads (result is 0/1).
    NumericCompare {
        dest: Temp,
        op: BinaryOp,
        left: Temp,
        right: Temp,
    },
    /// ABI helper syscall that accepts already-validated pointer-ABI operands directly.
    DirectHelperSyscall {
        dest: Temp,
        syscall: u32,
        args: Vec<Temp>,
    },
    Min {
        dest: Temp,
        a: Temp,
        b: Temp,
    },
    Max {
        dest: Temp,
        a: Temp,
        b: Temp,
    },
    Abs {
        dest: Temp,
        src: Temp,
    },
    DivCeil {
        dest: Temp,
        num: Temp,
        denom: Temp,
    },
    Gcd {
        dest: Temp,
        a: Temp,
        b: Temp,
    },
    Mean {
        dest: Temp,
        a: Temp,
        b: Temp,
    },
    Isqrt {
        dest: Temp,
        src: Temp,
    },
    /// Load the value of a variable.
    LoadVar {
        dest: Temp,
        name: String,
    },
    Poseidon2 {
        dest: Temp,
        a: Temp,
        b: Temp,
    },
    Poseidon6 {
        dest: Temp,
        args: [Temp; 6],
    },
    Pubkgen {
        dest: Temp,
        src: Temp,
    },
    Valcom {
        dest: Temp,
        value: Temp,
        blind: Temp,
    },
    /// Compute SM3 hash of a Blob pointer and return the resulting Blob pointer.
    Sm3Hash {
        dest: Temp,
        message: Temp,
    },
    /// Compute SHA-256 hash of a Blob pointer and return the resulting Blob pointer.
    Sha256Hash {
        dest: Temp,
        message: Temp,
    },
    /// Compute SHA3-256 hash of a Blob pointer and return the resulting Blob pointer.
    Sha3Hash {
        dest: Temp,
        message: Temp,
    },
    /// Compute Blake2b-256 hash of a Blob pointer and return the resulting Blob pointer.
    Blake2b256Hash {
        dest: Temp,
        message: Temp,
    },
    /// Compute Keccak-256 hash of a Blob pointer and return the resulting Blob pointer.
    Keccak256Hash {
        dest: Temp,
        message: Temp,
    },
    /// Compute Iroha's canonical ledger hash of a Blob pointer and return the resulting Blob pointer.
    IrohaHash {
        dest: Temp,
        message: Temp,
    },
    /// Verify an SM2 signature (message, signature, public key, optional distid) returning a bool.
    Sm2Verify {
        dest: Temp,
        message: Temp,
        signature: Temp,
        public_key: Temp,
        distid: Option<Temp>,
    },
    /// Verify a signature (message, signature, public key, scheme code) returning a bool.
    VerifySignature {
        dest: Temp,
        message: Temp,
        signature: Temp,
        public_key: Temp,
        scheme: Temp,
    },
    /// SM4-GCM seal: key, nonce, aad, plaintext -> ciphertext||tag blob.
    Sm4GcmSeal {
        dest: Temp,
        key: Temp,
        nonce: Temp,
        aad: Temp,
        plaintext: Temp,
    },
    /// SM4-GCM open: key, nonce, aad, ciphertext||tag -> plaintext (or 0 on failure).
    Sm4GcmOpen {
        dest: Temp,
        key: Temp,
        nonce: Temp,
        aad: Temp,
        ciphertext_and_tag: Temp,
    },
    /// SM4-CCM seal: key, nonce, aad, plaintext, optional tag length -> ciphertext||tag blob.
    Sm4CcmSeal {
        dest: Temp,
        key: Temp,
        nonce: Temp,
        aad: Temp,
        plaintext: Temp,
        tag_len: Option<Temp>,
    },
    /// SM4-CCM open: key, nonce, aad, ciphertext||tag, optional tag length -> plaintext blob (or 0).
    Sm4CcmOpen {
        dest: Temp,
        key: Temp,
        nonce: Temp,
        aad: Temp,
        ciphertext_and_tag: Temp,
        tag_len: Option<Temp>,
    },
    /// Register an asset definition with additional metadata.
    RegisterAsset {
        asset: Temp,
        symbol: Temp,
        quantity: Temp,
        mintable: Temp,
    },
    /// Helper builtin combining registration and mint into one operation.
    CreateNewAsset {
        asset: Temp,
        symbol: Temp,
        quantity: Temp,
        account: Temp,
        mintable: Temp,
    },
    /// Call the scoped `transfer_asset` syscall with from, to, asset, amount and dataspace parameters.
    TransferAsset {
        from: Temp,
        to: Temp,
        asset: Temp,
        amount: Temp,
        dataspace: Temp,
    },
    /// Add one transfer entry to the active FASTPQ transfer batch.
    TransferBatchAsset {
        from: Temp,
        to: Temp,
        asset: Temp,
        amount: Temp,
    },
    /// Open and fund a native asset escrow.
    EscrowOpenOffer {
        escrow: Temp,
        asset: Temp,
        amount: Temp,
        evidence_hashes: Option<Temp>,
    },
    /// Accept a native asset escrow.
    EscrowAccept {
        escrow: Temp,
    },
    /// Mark escrow off-chain payment as sent.
    EscrowMarkPaymentSent {
        escrow: Temp,
    },
    /// Release a paid escrow.
    EscrowRelease {
        escrow: Temp,
    },
    /// Cancel an escrow before payment is marked.
    EscrowCancel {
        escrow: Temp,
    },
    /// Open an escrow dispute.
    EscrowOpenDispute {
        escrow: Temp,
        evidence_hashes: Option<Temp>,
    },
    /// Resolve a disputed escrow with a buyer/seller split.
    EscrowResolveDispute {
        escrow: Temp,
        buyer_amount: Temp,
        seller_amount: Temp,
        evidence_hashes: Option<Temp>,
    },
    /// Open and fund a native anonymous asset escrow from an opaque request payload.
    AnonymousEscrowOpenOffer {
        request: Temp,
    },
    /// Accept a native anonymous asset escrow.
    AnonymousEscrowAccept {
        escrow: Temp,
    },
    /// Mark anonymous escrow off-chain payment as sent.
    AnonymousEscrowMarkPaymentSent {
        escrow: Temp,
    },
    /// Release a paid anonymous escrow from an opaque request payload.
    AnonymousEscrowRelease {
        request: Temp,
    },
    /// Cancel an anonymous escrow from an opaque request payload.
    AnonymousEscrowCancel {
        request: Temp,
    },
    /// Open an anonymous escrow dispute.
    AnonymousEscrowOpenDispute {
        escrow: Temp,
        evidence_hashes: Option<Temp>,
    },
    /// Resolve a disputed anonymous escrow from an opaque request payload.
    AnonymousEscrowResolveDispute {
        request: Temp,
    },
    /// Begin a FASTPQ transfer batch scope.
    TransferBatchBegin,
    /// End the current FASTPQ transfer batch scope.
    TransferBatchEnd,
    /// Submit a pre-encoded FASTPQ TransferAssetBatch Norito payload.
    TransferBatchApply {
        payload: Temp,
    },
    /// Call the `mint_asset` syscall with account, asset and amount parameters.
    MintAsset {
        account: Temp,
        asset: Temp,
        amount: Temp,
    },
    /// Call the `burn_asset` syscall with account, asset and amount parameters.
    BurnAsset {
        account: Temp,
        asset: Temp,
        amount: Temp,
    },
    AssertEq {
        left: Temp,
        right: Temp,
    },
    /// Assert that a boolean condition holds.
    Assert {
        cond: Temp,
    },
    /// Abort execution with a stable contract error code if `cond` is non-zero.
    ///
    /// This is a non-ZK assertion primitive intended for fast on-chain checks.
    AbortIf {
        cond: Temp,
        code: Temp,
    },
    /// Log an info message (development only).
    Info {
        msg: Temp,
    },
    /// Debug-print a raw integer value.
    DebugPrint {
        value: Temp,
    },
    /// Debug-log a Json/Blob/NoritoBytes TLV pointer.
    DebugLog {
        payload: Temp,
    },
    /// Allocate a new map (heap allocation of 16 bytes) and return its pointer in `dest`.
    MapNew {
        dest: Temp,
    },
    /// Build a typed pointer from a string temp at compile/runtime.
    /// Codegen maps this to a data literal when `src` originates from `StringConst`.
    PointerFromString {
        dest: Temp,
        kind: DataRefKind,
        src: Temp,
    },
    /// Get from map: dest = map[key]
    MapGet {
        dest: Temp,
        map: Temp,
        key: Temp,
    },
    /// Load both key and value from a minimal single-bucket map layout.
    /// Layout: key at [0..8), value at [8..16) from the base pointer.
    MapLoadPair {
        dest_key: Temp,
        dest_val: Temp,
        map: Temp,
        offset: i16,
    },
    /// Set map[key] = value
    MapSet {
        map: Temp,
        key: Temp,
        value: Temp,
    },
    /// Load a 64-bit value from memory at [base + imm].
    Load64Imm {
        dest: Temp,
        base: Temp,
        imm: i16,
    },
    /// Pack multiple scalar temps into a tuple value represented by `dest`.
    /// Codegen treats this as metadata; no code is emitted.
    TuplePack {
        dest: Temp,
        items: Vec<Temp>,
    },
    /// Extract the `index`-th field from a tuple temp into `dest`.
    TupleGet {
        dest: Temp,
        tuple: Temp,
        index: usize,
    },
    /// Host helper: create one NFT per known account (sample convenience).
    CreateNftsForAllUsers,
    /// Host helper: set SmartContract execution depth parameter.
    SetExecutionDepth {
        value: Temp,
    },
    /// Host helper: set logical vector length (SETVL immediate).
    SetVl {
        value: Temp,
    },
    /// Call a user-defined function by name with positional arguments.
    /// Arguments are passed in ARG_REGS; return value (if any) in r10.
    Call {
        callee: String,
        args: Vec<Temp>,
        dest: Option<Temp>,
    },
    /// Call a user-defined function returning multiple scalar values.
    /// The callee writes results into consecutive return registers starting at r10.
    /// Codegen moves r10..r(10+n-1) into the provided `dests` temps in order.
    CallMulti {
        callee: String,
        args: Vec<Temp>,
        dests: Vec<Temp>,
    },
    /// Host helper: set account detail (pointer-ABI).
    /// Accepts pointer temps produced by `account_id/name/json` builtins.
    SetAccountDetail {
        account: Temp,
        key: Temp,
        value: Temp,
    },
    /// Create an NFT with given id and owner.
    CreateNft {
        nft: Temp,
        owner: Temp,
    },
    /// Set JSON metadata for an NFT key.
    SetNftData {
        nft: Temp,
        key: Temp,
        json: Temp,
    },
    /// Burn an NFT by id.
    BurnNft {
        nft: Temp,
    },
    /// Transfer an NFT from one account to another.
    TransferNft {
        from: Temp,
        nft: Temp,
        to: Temp,
    },
    /// Register a Domain by id.
    RegisterDomain {
        domain: Temp,
    },
    /// Register an account by id.
    RegisterAccount {
        account: Temp,
    },
    /// Add a JSON-encoded PublicKey signatory to an account.
    AddSignatory {
        account: Temp,
        signatory: Temp,
    },
    /// Remove a JSON-encoded PublicKey signatory from an account.
    RemoveSignatory {
        account: Temp,
        signatory: Temp,
    },
    /// Set the non-zero multisig quorum for an account.
    SetAccountQuorum {
        account: Temp,
        quorum: Temp,
    },
    /// Unregister a Domain by id.
    UnregisterDomain {
        domain: Temp,
    },
    /// Unregister an Asset Definition by id.
    UnregisterAsset {
        asset: Temp,
    },
    /// Unregister an Account by id.
    UnregisterAccount {
        account: Temp,
    },
    /// Register a peer by JSON payload.
    RegisterPeer {
        json: Temp,
    },
    /// Unregister a peer by JSON payload.
    UnregisterPeer {
        json: Temp,
    },
    /// Create a trigger from JSON spec.
    CreateTrigger {
        json: Temp,
    },
    /// Remove a trigger by name.
    RemoveTrigger {
        name: Temp,
    },
    /// Enable/disable a trigger by name.
    SetTriggerEnabled {
        name: Temp,
        enabled: Temp,
    },
    /// Grant a permission token (Name or Json) to an account.
    GrantPermission {
        account: Temp,
        token: Temp,
    },
    /// Revoke a permission token (Name or Json) from an account.
    RevokePermission {
        account: Temp,
        token: Temp,
    },
    /// Create a role with a JSON permission set.
    CreateRole {
        name: Temp,
        json: Temp,
    },
    /// Delete a role by name.
    DeleteRole {
        name: Temp,
    },
    /// Grant a role to an account.
    GrantRole {
        account: Temp,
        name: Temp,
    },
    /// Revoke a role from an account.
    RevokeRole {
        account: Temp,
        name: Temp,
    },
    /// Transfer a Domain to a new owner account.
    TransferDomain {
        domain: Temp,
        to: Temp,
    },
    /// A typed data reference to be placed in the data section and accessed
    /// via the pointer-ABI. The compiler will emit a load from the literal
    /// table into a register, yielding a pointer to a len-prefixed Norito blob
    /// of the encoded value.
    DataRef {
        dest: Temp,
        kind: DataRefKind,
        value: String,
    },
    /// Load current authority AccountId pointer into `dest` (host-provided).
    GetAuthority {
        dest: Temp,
    },
    /// Load current authority AccountId pointer through the extended sysvar surface.
    SysvarAuthority {
        dest: Temp,
    },
    /// Load the current trusted host time in unix milliseconds into `dest`.
    CurrentTimeMs {
        dest: Temp,
    },
    /// Load the current trusted host block height into `dest`.
    BlockHeight {
        dest: Temp,
    },
    /// Load the current trusted host block time in unix milliseconds into `dest`.
    BlockTimeMs {
        dest: Temp,
    },
    /// Load the current chain identifier Blob pointer into `dest`.
    ChainId {
        dest: Temp,
    },
    /// Load the current contract address NoritoBytes pointer into `dest`.
    ContractAddress {
        dest: Temp,
    },
    /// Load the current entrypoint name Blob pointer into `dest`.
    Entrypoint {
        dest: Temp,
    },
    /// Resolve a canonical account alias string to the current AccountId.
    ResolveAccountAlias {
        dest: Temp,
        alias: Temp,
    },
    /// Synchronous deployed-contract call using a contract-address literal, entrypoint name, and Json payload.
    CallContract {
        dest: Temp,
        contract: Temp,
        entrypoint: Temp,
        payload: Temp,
    },
    /// Load trigger event payload (`Json*`) into `dest` (host-provided).
    GetTriggerEvent {
        dest: Temp,
    },
    /// Load an arbitrary host-provided public input TLV by Name key.
    GetPublicInput {
        dest: Temp,
        key: Temp,
    },
    /// Test-only nested runtime entrypoint call as a named fixture actor.
    InvokeEntrypointAs {
        dest: Option<Temp>,
        actor: Temp,
        entrypoint: Temp,
        payload: Temp,
        returns_pointer: bool,
    },
    /// Test-only nested runtime entrypoint call as a named fixture actor with
    /// multiple return values.
    InvokeEntrypointAsMulti {
        dests: Vec<Temp>,
        actor: Temp,
        entrypoint: Temp,
        payload: Temp,
        return_pointer_mask: u64,
    },
    /// Test-only assertion that a named actor's runtime entrypoint call rejects.
    ExpectRejectAs {
        actor: Temp,
        entrypoint: Temp,
        payload: Temp,
    },
    /// Test-only actor registry lookup helpers.
    ActorAccount {
        dest: Temp,
        actor: Temp,
    },
    ActorPublicKey {
        dest: Temp,
        actor: Temp,
    },
    ActorSign {
        dest: Temp,
        actor: Temp,
        message: Temp,
    },
    /// ZK verify syscalls with NoritoBytes TLV pointer in r10.
    ZkVerify {
        /// Syscall number (0x60..0x63)
        number: u32,
        /// Temp holding pointer to NoritoBytes TLV (INPUT)
        payload: Temp,
    },
    /// Execution proof summary as NoritoBytes returned by the host.
    ProveExecution {
        dest: Temp,
    },
    /// Grow VM heap by `bytes`, returning the new heap limit.
    GrowHeap {
        dest: Temp,
        bytes: Temp,
    },
    /// Write Merkle path for raw VM memory address to raw memory buffer.
    GetMerklePath {
        dest: Temp,
        address: Temp,
        output: Temp,
        root_output: Option<Temp>,
    },
    /// Write compact Merkle proof for raw VM memory address to raw memory buffer.
    GetMerkleCompact {
        dest: Temp,
        address: Temp,
        output: Temp,
        max_depth: Option<Temp>,
        root_output: Option<Temp>,
    },
    /// Write compact Merkle proof for register leaf to raw memory buffer.
    GetRegisterMerkleCompact {
        dest: Temp,
        register_index: Temp,
        output: Temp,
        max_depth: Option<Temp>,
        root_output: Option<Temp>,
    },
    /// Generic OpenVerify proof verification returning r10 = 0/1.
    VerifyProof {
        dest: Temp,
        payload: Temp,
    },
    /// Operation-specific bridge to SMARTCONTRACT_EXECUTE_INSTRUCTION.
    VendorExecuteInstruction {
        payload: Temp,
        kind: VendorInstructionKind,
    },
    /// Vendor bridge: SMARTCONTRACT_EXECUTE_QUERY with NoritoBytes `QueryRequest` in r10.
    VendorExecuteQuery {
        dest: Temp,
        payload: Temp,
    },
    /// Extended query bridge: QUERY_EXECUTE_NORITO with NoritoBytes `QueryRequest` in r10.
    QueryExecuteNorito {
        dest: Temp,
        payload: Temp,
    },
    /// Direct typed query helper: r10 = pointer key, syscall returns NoritoBytes in r10.
    QueryGet {
        dest: Temp,
        key: Temp,
        syscall: u32,
    },
    /// Host balance query: r10 = &AccountId, r11 = &AssetDefinitionId; returns &NoritoBytes(Numeric).
    GetAccountBalance {
        dest: Temp,
        account: Temp,
        asset: Temp,
    },
    /// Allocate `bytes` on the VM heap, returning the raw heap pointer.
    Alloc {
        dest: Temp,
        bytes: Temp,
    },
    /// Read a host-provided private input by numeric index.
    GetPrivateInput {
        dest: Temp,
        index: Temp,
    },
    /// Record a nullifier and reject if it was already used.
    UseNullifier {
        nullifier: Temp,
    },
    /// Commit the VM OUTPUT region to the host.
    CommitOutput,
    /// Smart-contract lifecycle governance/runtime helper with NoritoBytes request in r10.
    SmartContractLifecycle {
        payload: Temp,
        syscall: u32,
    },
    /// Read recent ZK roots with a NoritoBytes request in r10.
    ZkRootsGet {
        dest: Temp,
        payload: Temp,
    },
    /// Read finalized ZK vote tally with a NoritoBytes request in r10.
    ZkVoteGetTally {
        dest: Temp,
        payload: Temp,
    },
    /// Read VRF epoch seed with a NoritoBytes request in r10.
    VrfEpochSeed {
        dest: Temp,
        payload: Temp,
    },
    /// Subscription billing helper using trigger context.
    SubscriptionBill,
    /// Subscription usage recorder using trigger args.
    SubscriptionRecordUsage,
    /// Durable state get: r10 = &Name path; returns r10 = &NoritoBytes into dest.
    StateGet {
        dest: Temp,
        path: Temp,
    },
    /// Durable state set: r10 = &Name path; r11 = &NoritoBytes value
    StateSet {
        path: Temp,
        value: Temp,
    },
    /// Durable state delete: r10 = &Name path
    StateDel {
        path: Temp,
    },
    /// Durable state key enumeration: r10 = &Name prefix; r11 = offset; r12 = limit.
    StateKeys {
        dest: Temp,
        prefix: Temp,
        offset: Temp,
        limit: Temp,
    },
    /// Decode a canonical map key from a page returned by `STATE_KEYS`.
    StateMapKeyAt {
        dest: Temp,
        page: Temp,
        base: Temp,
        index: Temp,
    },
    /// Encode a compiler-flattened aggregate state value using a schema literal.
    StateValueEncode {
        dest: Temp,
        schema: Temp,
        words: Vec<Temp>,
    },
    /// Durable state key presence: r10 = &Name path; returns present flag in dest.
    StateHas {
        dest: Temp,
        path: Temp,
    },
    /// Durable state value payload length: r10 = &Name path; returns length in dest.
    StateLen {
        dest: Temp,
        path: Temp,
    },
    /// Durable state key count for prefix: r10 = &Name prefix; returns total in dest.
    StateCount {
        dest: Temp,
        prefix: Temp,
    },
    /// Decode a NoritoBytes blob containing an ASCII decimal integer; result in dest.
    DecodeInt {
        dest: Temp,
        blob: Temp,
    },
    /// Build a Name path for map key: r10 = &Name base; r11 = key (int); returns &Name in dest.
    PathMapKey {
        dest: Temp,
        base: Temp,
        key: Temp,
    },
    /// Build Name path: base / hash(norito_bytes_key)
    PathMapKeyNorito {
        dest: Temp,
        base: Temp,
        key_blob: Temp,
    },
    /// Encode an int into NoritoBytes (ASCII decimal) using host syscall; result pointer in dest.
    EncodeInt {
        dest: Temp,
        value: Temp,
    },
    /// Encode a pointer-ABI value into NoritoBytes via host.
    PointerToNorito {
        dest: Temp,
        value: Temp,
    },
    /// Decode NoritoBytes into a pointer-ABI value via host.
    PointerFromNorito {
        dest: Temp,
        blob: Temp,
        kind: DataRefKind,
    },
    /// Encode JSON (&Json) to NoritoBytes via host
    JsonEncode {
        dest: Temp,
        json: Temp,
    },
    /// Decode NoritoBytes to &Json via host
    JsonDecode {
        dest: Temp,
        blob: Temp,
    },
    /// Return the payload length of an arbitrary pointer-ABI TLV.
    TlvLen {
        dest: Temp,
        value: Temp,
    },
    /// Construct an empty Json object.
    JsonObject {
        dest: Temp,
    },
    /// Insert or replace an integer field in a Json object.
    JsonSetInt {
        dest: Temp,
        json: Temp,
        key: Temp,
        value: Temp,
    },
    /// Insert or replace an AccountId field in a Json object.
    JsonSetAccountId {
        dest: Temp,
        json: Temp,
        key: Temp,
        value: Temp,
    },
    /// JSON getters: (&Json, &Name key) -> int
    JsonGetInt {
        dest: Temp,
        json: Temp,
        key: Temp,
    },
    /// JSON getters: (&Json, &Name key) -> &NoritoBytes(Numeric)
    JsonGetNumeric {
        dest: Temp,
        json: Temp,
        key: Temp,
    },
    /// JSON getters: (&Json, &Name key) -> &Json
    JsonGetJson {
        dest: Temp,
        json: Temp,
        key: Temp,
    },
    /// JSON getters: (&Json, &Name key) -> &Name
    JsonGetName {
        dest: Temp,
        json: Temp,
        key: Temp,
    },
    /// JSON getters: (&Json, &Name key) -> &AccountId
    JsonGetAccountId {
        dest: Temp,
        json: Temp,
        key: Temp,
    },
    /// JSON getters: (&Json, &Name key) -> &AssetDefinitionId
    JsonGetAssetDefinitionId {
        dest: Temp,
        json: Temp,
        key: Temp,
    },
    /// JSON getters: (&Json, &Name key) -> &NftId
    JsonGetNftId {
        dest: Temp,
        json: Temp,
        key: Temp,
    },
    /// JSON getters: (&Json, &Name key) -> &Blob
    JsonGetBlobHex {
        dest: Temp,
        json: Temp,
        key: Temp,
    },
    /// Decode Name from NoritoBytes via host
    NameDecode {
        dest: Temp,
        blob: Temp,
    },
    /// Schema encode: (&Name schema, &Json) -> &NoritoBytes
    SchemaEncode {
        dest: Temp,
        schema: Temp,
        json: Temp,
    },
    /// Schema decode: (&Name schema, &NoritoBytes) -> &Json
    SchemaDecode {
        dest: Temp,
        schema: Temp,
        blob: Temp,
    },
    /// Fetch schema metadata as Json: (&Name schema) -> &Json {id, version}
    SchemaInfo {
        dest: Temp,
        schema: Temp,
    },
    /// Build a Norito-encoded SubmitBallot InstructionBox in data section and return Blob pointer
    BuildSubmitBallotInline {
        dest: Temp,
        election_id: Temp,
        ciphertext: Temp,
        nullifier: Temp,
        backend: Temp,
        proof: Temp,
        vk: Temp,
    },
    /// Build a Norito-encoded Unshield InstructionBox in data section and return Blob pointer
    BuildUnshieldInline {
        dest: Temp,
        asset: Temp,
        to: Temp,
        amount: Temp,
        inputs: Temp,
        outputs: Option<Temp>,
        backend: Temp,
        proof: Temp,
        vk: Temp,
    },
    /// Compare two pointer-ABI values for deep equality by content.
    PointerEq {
        dest: Temp,
        left: Temp,
        right: Temp,
    },
    /// Verify a VRF proof and return the output Blob pointer (or 0).
    VrfVerify {
        dest: Temp,
        input: Temp,
        public_key: Temp,
        proof: Temp,
        variant: Temp,
    },
    /// Batch VRF verification returning a NoritoBytes vector of outputs (or 0).
    VrfVerifyBatch {
        dest: Temp,
        batch: Temp,
    },
    /// Begin an AXT envelope with descriptor pointer in r10.
    AxtBegin {
        descriptor: Temp,
    },
    /// Record a dataspace touch manifest (optional manifest pointer in r11).
    AxtTouch {
        dsid: Temp,
        manifest: Option<Temp>,
    },
    /// Attach or clear a dataspace proof for the active AXT.
    VerifyDsProof {
        dsid: Temp,
        proof: Option<Temp>,
    },
    /// Use an asset handle with a NoritoBytes intent and optional proof.
    UseAssetHandle {
        handle: Temp,
        intent: Temp,
        proof: Option<Temp>,
    },
    /// Commit the active AXT envelope.
    AxtCommit,
    /// Execute a Soracloud runtime host operation with request pointer in r10.
    SoracloudHostCall {
        dest: Temp,
        request: Temp,
        syscall: u32,
    },
}

/// Instruction kind authorized by the tagged smart-contract instruction bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendorInstructionKind {
    /// Governance `SubmitBallot`.
    SubmitBallot,
    /// ZK `Unshield`.
    Unshield,
    /// Bridge `RecordSccpMessage`.
    RecordSccpMessage,
}

/// Kinds of typed data references supported by the pointer-ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataRefKind {
    Account,
    AssetDef,
    Name,
    Json,
    NftId,
    AssetId,
    Domain,
    Blob,
    /// Raw NoritoBytes TLV payload (pointer-ABI), distinct from generic Blob.
    NoritoBytes,
    DataSpaceId,
    AxtDescriptor,
    AssetHandle,
    ProofBlob,
    SoracloudRequest,
    SoracloudResponse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyCodec {
    Int,
    Pointer,
    NoritoBytes,
}

fn pointer_kind_for_type(ty: &Type) -> Option<DataRefKind> {
    match semantic::resolve_struct_type(ty) {
        Type::AccountId => Some(DataRefKind::Account),
        Type::AssetDefinitionId => Some(DataRefKind::AssetDef),
        Type::AssetId => Some(DataRefKind::AssetId),
        Type::DomainId => Some(DataRefKind::Domain),
        Type::NftId => Some(DataRefKind::NftId),
        Type::Name => Some(DataRefKind::Name),
        Type::DataSpaceId => Some(DataRefKind::DataSpaceId),
        Type::AxtDescriptor => Some(DataRefKind::AxtDescriptor),
        Type::AssetHandle => Some(DataRefKind::AssetHandle),
        Type::ProofBlob => Some(DataRefKind::ProofBlob),
        Type::SoracloudRequest => Some(DataRefKind::SoracloudRequest),
        Type::SoracloudResponse => Some(DataRefKind::SoracloudResponse),
        Type::String | Type::Bytes => Some(DataRefKind::Blob),
        _ => None,
    }
}

fn is_pointer_eq_type(ty: &Type) -> bool {
    matches!(
        semantic::resolve_struct_type(ty),
        Type::String | Type::Bytes | Type::Json
    ) || semantic::is_pointer_type(ty)
}

fn lower_map_key_eq(ctx: &mut LowerCtx, key_ty: &Type, left: Temp, right: Temp) -> Temp {
    if semantic::is_wide_numeric_type(key_ty) {
        let t = ctx.new_temp();
        ctx.current_instr(Instr::NumericCompare {
            dest: t,
            op: BinaryOp::Eq,
            left,
            right,
        });
        t
    } else if is_pointer_eq_type(key_ty) {
        let t = ctx.new_temp();
        ctx.current_instr(Instr::PointerEq {
            dest: t,
            left,
            right,
        });
        t
    } else {
        let t = ctx.new_temp();
        ctx.current_instr(Instr::Binary {
            dest: t,
            op: BinaryOp::Eq,
            left,
            right,
        });
        t
    }
}

fn key_codec_for_type(ty: &Type) -> Option<KeyCodec> {
    match semantic::resolve_struct_type(ty) {
        Type::Int | Type::Bool => Some(KeyCodec::Int),
        ty if semantic::is_wide_numeric_type(&ty) => Some(KeyCodec::NoritoBytes),
        Type::String | Type::Bytes => Some(KeyCodec::Pointer),
        other if semantic::is_pointer_type(&other) => Some(KeyCodec::Pointer),
        _ => None,
    }
}

fn state_value_kind_for_type(ty: &Type) -> Option<ivm_abi::state_value::StateValueKindV1> {
    use ivm_abi::state_value::StateValueKindV1 as Kind;

    Some(match semantic::resolve_struct_type(ty) {
        Type::Int => Kind::Int,
        Type::FixedU128 => Kind::U128,
        Type::Amount => Kind::Amount,
        Type::Bool => Kind::Bool,
        Type::String => Kind::String,
        Type::Json => Kind::Json,
        Type::Bytes => Kind::Bytes,
        Type::AccountId => Kind::AccountId,
        Type::AssetDefinitionId => Kind::AssetDefinitionId,
        Type::AssetId => Kind::AssetId,
        Type::DomainId => Kind::DomainId,
        Type::NftId => Kind::NftId,
        Type::Name => Kind::Name,
        Type::DataSpaceId => Kind::DataSpaceId,
        Type::AxtDescriptor => Kind::AxtDescriptor,
        Type::AssetHandle => Kind::AssetHandle,
        Type::ProofBlob => Kind::ProofBlob,
        Type::SoracloudRequest => Kind::SoracloudRequest,
        Type::SoracloudResponse => Kind::SoracloudResponse,
        Type::Unit
        | Type::Secret(_)
        | Type::StateMap(_, _)
        | Type::Option(_)
        | Type::Result(_, _)
        | Type::Tuple(_)
        | Type::Struct { .. }
        | Type::NamedStruct(_) => return None,
    })
}

fn append_state_value_schema_nodes(
    ty: &Type,
    nodes: &mut Vec<ivm_abi::state_value::StateValueNodeV1>,
) -> bool {
    use ivm_abi::state_value::StateValueNodeV1 as Node;

    match semantic::resolve_struct_type(ty) {
        Type::Struct { name, fields } => {
            nodes.push(Node::Struct {
                name,
                fields: fields.iter().map(|(name, _)| name.clone()).collect(),
            });
            fields
                .iter()
                .all(|(_, field_ty)| append_state_value_schema_nodes(field_ty, nodes))
        }
        Type::Tuple(items) => {
            let Ok(arity) = u16::try_from(items.len()) else {
                return false;
            };
            nodes.push(Node::Tuple { arity });
            items
                .iter()
                .all(|item| append_state_value_schema_nodes(item, nodes))
        }
        Type::Option(inner) => {
            nodes.push(Node::Option);
            append_state_value_schema_nodes(&inner, nodes)
        }
        Type::Result(ok, err) => {
            nodes.push(Node::Result);
            append_state_value_schema_nodes(&ok, nodes)
                && append_state_value_schema_nodes(&err, nodes)
        }
        leaf => {
            let Some(kind) = state_value_kind_for_type(&leaf) else {
                return false;
            };
            nodes.push(Node::Leaf(kind));
            true
        }
    }
}

fn state_value_schema(ty: &Type) -> Option<ivm_abi::state_value::StateValueSchemaV1> {
    let mut nodes = Vec::new();
    if !append_state_value_schema_nodes(ty, &mut nodes) {
        return None;
    }
    let schema = ivm_abi::state_value::StateValueSchemaV1 { nodes };
    schema.validate().then_some(schema)
}

fn emit_state_value_schema_ref(ctx: &mut LowerCtx, ty: &Type) -> Option<Temp> {
    let schema = state_value_schema(ty)?;
    let encoded = norito::to_bytes(&schema).ok()?;
    if encoded.len() > ivm_abi::state_value::MAX_STATE_VALUE_SCHEMA_BYTES {
        return None;
    }
    let schema_ref = ctx.new_temp();
    ctx.current_instr(Instr::DataRef {
        dest: schema_ref,
        kind: DataRefKind::NoritoBytes,
        value: format!("0x{}", hex::encode(encoded)),
    });
    Some(schema_ref)
}

fn collect_state_value_words(
    ctx: &mut LowerCtx,
    value: Temp,
    ty: &Type,
    words: &mut Vec<Temp>,
) -> bool {
    match semantic::resolve_struct_type(ty) {
        Type::Struct { fields, .. } => fields.iter().enumerate().all(|(index, (_, field_ty))| {
            let field = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: field,
                tuple: value,
                index,
            });
            collect_state_value_words(ctx, field, field_ty, words)
        }),
        Type::Tuple(items) => items.iter().enumerate().all(|(index, item_ty)| {
            let item = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: item,
                tuple: value,
                index,
            });
            collect_state_value_words(ctx, item, item_ty, words)
        }),
        Type::Option(inner) => {
            let tag = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: tag,
                tuple: value,
                index: 0,
            });
            words.push(tag);
            let payload = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: payload,
                tuple: value,
                index: 1,
            });
            collect_state_value_words(ctx, payload, &inner, words)
        }
        Type::Result(ok, err) => {
            let tag = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: tag,
                tuple: value,
                index: 0,
            });
            words.push(tag);
            let ok_value = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: ok_value,
                tuple: value,
                index: 1,
            });
            let err_value = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: err_value,
                tuple: value,
                index: 2,
            });
            collect_state_value_words(ctx, ok_value, &ok, words)
                && collect_state_value_words(ctx, err_value, &err, words)
        }
        leaf => {
            if state_value_kind_for_type(&leaf).is_none() {
                return false;
            }
            words.push(value);
            true
        }
    }
}

fn collect_function_value_words(ctx: &mut LowerCtx, value: Temp, ty: &Type, words: &mut Vec<Temp>) {
    match semantic::resolve_struct_type(ty) {
        Type::Struct { fields, .. } => {
            for (index, (_, field_ty)) in fields.iter().enumerate() {
                let field = ctx.new_temp();
                ctx.current_instr(Instr::TupleGet {
                    dest: field,
                    tuple: value,
                    index,
                });
                collect_function_value_words(ctx, field, field_ty, words);
            }
        }
        Type::Tuple(items) => {
            for (index, item_ty) in items.iter().enumerate() {
                let item = ctx.new_temp();
                ctx.current_instr(Instr::TupleGet {
                    dest: item,
                    tuple: value,
                    index,
                });
                collect_function_value_words(ctx, item, item_ty, words);
            }
        }
        Type::Option(inner) => {
            let tag = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: tag,
                tuple: value,
                index: 0,
            });
            words.push(tag);
            let payload = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: payload,
                tuple: value,
                index: 1,
            });
            collect_function_value_words(ctx, payload, &inner, words);
        }
        Type::Result(ok, err) => {
            let tag = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: tag,
                tuple: value,
                index: 0,
            });
            words.push(tag);
            let ok_value = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: ok_value,
                tuple: value,
                index: 1,
            });
            let err_value = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: err_value,
                tuple: value,
                index: 2,
            });
            collect_function_value_words(ctx, ok_value, &ok, words);
            collect_function_value_words(ctx, err_value, &err, words);
        }
        _ => words.push(value),
    }
}

fn encode_aggregate_state_value(ctx: &mut LowerCtx, value: Temp, ty: &Type) -> Option<Temp> {
    let schema = emit_state_value_schema_ref(ctx, ty)?;
    let mut words = Vec::new();
    if !collect_state_value_words(ctx, value, ty, &mut words)
        || words.len() > ivm_abi::state_value::MAX_STATE_VALUE_WORDS
    {
        return None;
    }
    let dest = ctx.new_temp();
    ctx.current_instr(Instr::StateValueEncode {
        dest,
        schema,
        words,
    });
    Some(dest)
}

fn load_state_value_word(ctx: &mut LowerCtx, table: Temp, index: &mut usize) -> Option<Temp> {
    let index_i16 = i16::try_from(*index).ok()?;
    let imm = ivm_abi::state_value::DECODED_STATE_VALUE_TABLE_OFFSET.checked_add(
        index_i16.checked_mul(ivm_abi::state_value::DECODED_STATE_VALUE_WORD_BYTES)?,
    )?;
    let dest = ctx.new_temp();
    ctx.current_instr(Instr::Load64Imm {
        dest,
        base: table,
        imm,
    });
    *index = index.saturating_add(1);
    Some(dest)
}

fn rebuild_state_value_from_table(
    ctx: &mut LowerCtx,
    table: Temp,
    ty: &Type,
    index: &mut usize,
) -> Option<Temp> {
    match semantic::resolve_struct_type(ty) {
        Type::Struct { fields, .. } => {
            let items = fields
                .iter()
                .map(|(_, field_ty)| rebuild_state_value_from_table(ctx, table, field_ty, index))
                .collect::<Option<Vec<_>>>()?;
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack { dest, items });
            Some(dest)
        }
        Type::Tuple(types) => {
            let items = types
                .iter()
                .map(|item_ty| rebuild_state_value_from_table(ctx, table, item_ty, index))
                .collect::<Option<Vec<_>>>()?;
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack { dest, items });
            Some(dest)
        }
        Type::Option(inner) => {
            let tag = load_state_value_word(ctx, table, index)?;
            let value = rebuild_state_value_from_table(ctx, table, &inner, index)?;
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack {
                dest,
                items: vec![tag, value],
            });
            Some(dest)
        }
        Type::Result(ok, err) => {
            let tag = load_state_value_word(ctx, table, index)?;
            let ok_value = rebuild_state_value_from_table(ctx, table, &ok, index)?;
            let err_value = rebuild_state_value_from_table(ctx, table, &err, index)?;
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack {
                dest,
                items: vec![tag, ok_value, err_value],
            });
            Some(dest)
        }
        leaf => {
            state_value_kind_for_type(&leaf)?;
            load_state_value_word(ctx, table, index)
        }
    }
}

fn rebuild_state_value_from_words(
    ctx: &mut LowerCtx,
    ty: &Type,
    words: &[Temp],
    index: &mut usize,
) -> Option<Temp> {
    match semantic::resolve_struct_type(ty) {
        Type::Struct { fields, .. } => {
            let items = fields
                .iter()
                .map(|(_, field_ty)| rebuild_state_value_from_words(ctx, field_ty, words, index))
                .collect::<Option<Vec<_>>>()?;
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack { dest, items });
            Some(dest)
        }
        Type::Tuple(types) => {
            let items = types
                .iter()
                .map(|item_ty| rebuild_state_value_from_words(ctx, item_ty, words, index))
                .collect::<Option<Vec<_>>>()?;
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack { dest, items });
            Some(dest)
        }
        Type::Option(inner) => {
            let tag = *words.get(*index)?;
            *index = index.saturating_add(1);
            let value = rebuild_state_value_from_words(ctx, &inner, words, index)?;
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack {
                dest,
                items: vec![tag, value],
            });
            Some(dest)
        }
        Type::Result(ok, err) => {
            let tag = *words.get(*index)?;
            *index = index.saturating_add(1);
            let ok_value = rebuild_state_value_from_words(ctx, &ok, words, index)?;
            let err_value = rebuild_state_value_from_words(ctx, &err, words, index)?;
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack {
                dest,
                items: vec![tag, ok_value, err_value],
            });
            Some(dest)
        }
        leaf => {
            state_value_kind_for_type(&leaf)?;
            let word = *words.get(*index)?;
            *index = index.saturating_add(1);
            Some(word)
        }
    }
}

fn rebuild_function_value_from_words(
    ctx: &mut LowerCtx,
    ty: &Type,
    words: &[Temp],
    index: &mut usize,
) -> Option<Temp> {
    match semantic::resolve_struct_type(ty) {
        Type::Struct { fields, .. } => {
            let items = fields
                .iter()
                .map(|(_, field_ty)| rebuild_function_value_from_words(ctx, field_ty, words, index))
                .collect::<Option<Vec<_>>>()?;
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack { dest, items });
            Some(dest)
        }
        Type::Tuple(types) => {
            let items = types
                .iter()
                .map(|item_ty| rebuild_function_value_from_words(ctx, item_ty, words, index))
                .collect::<Option<Vec<_>>>()?;
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack { dest, items });
            Some(dest)
        }
        Type::Option(inner) => {
            let tag = *words.get(*index)?;
            *index = index.saturating_add(1);
            let value = rebuild_function_value_from_words(ctx, &inner, words, index)?;
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack {
                dest,
                items: vec![tag, value],
            });
            Some(dest)
        }
        Type::Result(ok, err) => {
            let tag = *words.get(*index)?;
            *index = index.saturating_add(1);
            let ok_value = rebuild_function_value_from_words(ctx, &ok, words, index)?;
            let err_value = rebuild_function_value_from_words(ctx, &err, words, index)?;
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack {
                dest,
                items: vec![tag, ok_value, err_value],
            });
            Some(dest)
        }
        _ => {
            let word = *words.get(*index)?;
            *index = index.saturating_add(1);
            Some(word)
        }
    }
}

fn decode_aggregate_state_value(ctx: &mut LowerCtx, blob: Temp, ty: &Type) -> Option<Temp> {
    let schema = emit_state_value_schema_ref(ctx, ty)?;
    let table = ctx.new_temp();
    ctx.current_instr(Instr::DirectHelperSyscall {
        dest: table,
        syscall: ivm_abi::syscalls::SYSCALL_STATE_VALUE_DECODE,
        args: vec![schema, blob],
    });
    let mut index = 0;
    let value = rebuild_state_value_from_table(ctx, table, ty, &mut index)?;
    let expected = state_value_schema(ty)?.word_kinds()?.len();
    (index == expected).then_some(value)
}

fn is_aggregate_state_value_type(ty: &Type) -> bool {
    matches!(
        semantic::resolve_struct_type(ty),
        Type::Struct { .. } | Type::Tuple(_) | Type::Option(_) | Type::Result(_, _)
    )
}

fn is_canonical_state_value_type(ty: &Type) -> bool {
    state_value_schema(ty).is_some()
}

fn lower_state_map_get_value(
    ctx: &mut LowerCtx,
    base_name: &str,
    key_tmp: Temp,
    key_ty: &Type,
    value_ty: &Type,
) -> Option<Temp> {
    let key_codec = key_codec_for_type(key_ty)?;
    let path = build_state_path(ctx, base_name, key_tmp, &key_codec);
    let blob = ctx.new_temp();
    ctx.current_instr(Instr::StateGet { dest: blob, path });
    decode_state_map_value_blob(ctx, blob, value_ty)
}

fn decode_state_map_value_blob(ctx: &mut LowerCtx, blob: Temp, value_ty: &Type) -> Option<Temp> {
    let resolved = semantic::resolve_struct_type(value_ty);
    if !is_canonical_state_value_type(&resolved) {
        return None;
    }
    decode_aggregate_state_value(ctx, blob, &resolved)
}

fn lower_state_map_set_value(
    ctx: &mut LowerCtx,
    base_name: &str,
    key_tmp: Temp,
    key_ty: &Type,
    value_ty: &Type,
    value_tmp: Temp,
) -> bool {
    let resolved = semantic::resolve_struct_type(value_ty);
    let Some(key_codec) = key_codec_for_type(key_ty) else {
        return false;
    };
    let path = build_state_path(ctx, base_name, key_tmp, &key_codec);
    if !is_canonical_state_value_type(&resolved) {
        return false;
    }
    let Some(encoded) = encode_aggregate_state_value(ctx, value_tmp, &resolved) else {
        return false;
    };
    ctx.current_instr(Instr::StateSet {
        path,
        value: encoded,
    });
    true
}

/// Control-flow terminators for a block.
#[derive(Debug, PartialEq)]
pub enum Terminator {
    Return(Option<Temp>),
    /// Return a 2-tuple via r10 (first) and r11 (second).
    Return2(Temp, Temp),
    /// Return N values via r10.. in order.
    ReturnN(Vec<Temp>),
    Jump(Label),
    Branch {
        cond: Temp,
        then_bb: Label,
        else_bb: Label,
    },
}

/// Lower a semantically checked program into IR.
pub fn lower(program: &TypedProgram) -> Result<Program, String> {
    lower_with_cap(
        program,
        crate::semantic::COLLECTION_ITERATION_LIMIT as usize,
    )
}

/// Lower with the first-release dynamic-iteration cap.
pub fn lower_with_cap(program: &TypedProgram, dyn_iter_cap: usize) -> Result<Program, String> {
    lower_with_cap_and_test_mode(program, dyn_iter_cap, false)
}

/// Lower with a specific dynamic-iteration cap and optional local-test semantics.
pub fn lower_with_cap_and_test_mode(
    program: &TypedProgram,
    dyn_iter_cap: usize,
    test_mode: bool,
) -> Result<Program, String> {
    lower_with_cap_and_test_mode_diagnostics(program, dyn_iter_cap, test_mode).map_err(|failures| {
        failures
            .into_iter()
            .map(|failure| failure.message)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LoweringFailure {
    pub(crate) message: String,
    pub(crate) location: super::ast::SourceLocation,
}

pub(crate) fn lower_with_cap_and_test_mode_diagnostics(
    program: &TypedProgram,
    dyn_iter_cap: usize,
    test_mode: bool,
) -> Result<Program, Vec<LoweringFailure>> {
    let call_renames = build_entrypoint_call_renames(program);
    let function_param_specs = build_function_param_specs(program);
    let mut functions = Vec::new();
    let mut failures = Vec::new();
    for item in &program.items {
        let TypedItem::Function(f) = item;
        if needs_entrypoint_wrapper(f) {
            let impl_name = entrypoint_impl_symbol(&f.name);
            match lower_function_named(
                f,
                &impl_name,
                &program.states,
                dyn_iter_cap,
                &call_renames,
                &function_param_specs,
            ) {
                Ok(function) => functions.push(function),
                Err(message) => failures.push(LoweringFailure {
                    message,
                    location: f.location,
                }),
            }
            match lower_entrypoint_wrapper(
                f,
                &impl_name,
                dyn_iter_cap,
                &call_renames,
                &function_param_specs,
                test_mode,
            ) {
                Ok(function) => functions.push(function),
                Err(message) => failures.push(LoweringFailure {
                    message,
                    location: f.location,
                }),
            }
        } else {
            match lower_function_named(
                f,
                &f.name,
                &program.states,
                dyn_iter_cap,
                &call_renames,
                &function_param_specs,
            ) {
                Ok(function) => functions.push(function),
                Err(message) => failures.push(LoweringFailure {
                    message,
                    location: f.location,
                }),
            }
        }
    }
    if failures.is_empty() {
        Ok(Program { functions })
    } else {
        Err(failures)
    }
}

fn build_entrypoint_call_renames(program: &TypedProgram) -> HashMap<String, String> {
    let mut renames = HashMap::new();
    for item in &program.items {
        let TypedItem::Function(func) = item;
        if needs_entrypoint_wrapper(func) {
            renames.insert(func.name.clone(), entrypoint_impl_symbol(&func.name));
        }
    }
    renames
}

fn build_function_param_specs(program: &TypedProgram) -> HashMap<String, Vec<TypedParam>> {
    let mut specs = HashMap::new();
    for item in &program.items {
        let TypedItem::Function(func) = item;
        specs.insert(func.name.clone(), func.param_types.clone());
    }
    specs
}

fn entrypoint_impl_symbol(name: &str) -> String {
    format!("__entrypoint_impl__{name}")
}

// Zero-argument entrypoints can jump straight into the implementation body
// because there is no payload-decoding work for a wrapper to perform.
fn needs_entrypoint_wrapper(func: &TypedFunction) -> bool {
    (matches!(func.modifiers.kind, super::ast::FunctionKind::View)
        || func.modifiers.visibility == super::ast::FunctionVisibility::Public)
        && !func.param_types.is_empty()
}

fn lower_function_named(
    func: &TypedFunction,
    symbol_name: &str,
    states: &[TypedStateDecl],
    dyn_iter_cap: usize,
    call_renames: &HashMap<String, String>,
    function_param_specs: &HashMap<String, Vec<TypedParam>>,
) -> Result<Function, String> {
    let mut ctx = LowerCtx::new(
        dyn_iter_cap,
        call_renames.clone(),
        function_param_specs.clone(),
    );
    let entry = ctx.new_label();
    ctx.start_block(entry);
    // Ephemeral state allocation for contract-level `state` declarations.
    let mut vars = HashMap::new();
    let mut param_temps = Vec::new();
    let lowered_params = lowered_function_params(&func.param_types);
    if lowered_params.len() > crate::regalloc::MAX_ARGUMENT_VALUES {
        return Err(format!(
            "function `{}` requires {} flattened argument words, exceeding the Kotodama V1 limit of {}",
            func.name,
            lowered_params.len(),
            crate::regalloc::MAX_ARGUMENT_VALUES,
        ));
    }
    for param in &lowered_params {
        let tmp = ctx.new_temp();
        param_temps.push((param.clone(), tmp));
        ctx.current_instr(Instr::LoadVar {
            dest: tmp,
            name: param.clone(),
        });
    }
    let mut state_entries = states.iter().collect::<Vec<_>>();
    state_entries.sort_by(|left, right| left.name.cmp(&right.name));
    for state in state_entries {
        register_state_value_metadata(&mut ctx, &state.name, &state.ty, &state.name);
    }
    let loaded_params = param_temps.into_iter().collect::<HashMap<_, _>>();
    for param in &func.param_types {
        if param.is_state {
            let tmp = *loaded_params.get(&param.name).ok_or_else(|| {
                format!(
                    "internal error: missing lowered state parameter `{}`",
                    param.name
                )
            })?;
            ctx.state_runtime_roots.insert(param.name.clone(), tmp);
            if let Type::StateMap(key, value) = semantic::resolve_struct_type(&param.ty) {
                ctx.state_map_configs.insert(
                    param.name.clone(),
                    StateMapSpec {
                        key: *key,
                        value: *value,
                    },
                );
            }
        } else if let Some(word_types) = function_value_word_types(&param.ty) {
            let words = (0..word_types.len())
                .map(|index| {
                    loaded_params
                        .get(&function_param_word_name(&param.name, index))
                        .copied()
                        .ok_or_else(|| {
                            format!(
                                "internal error: missing ABI word {index} for parameter `{}`",
                                param.name
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut index = 0;
            let value = rebuild_function_value_from_words(&mut ctx, &param.ty, &words, &mut index)
                .ok_or_else(|| {
                    format!(
                        "internal error: cannot rebuild aggregate parameter `{}`",
                        param.name
                    )
                })?;
            if index != words.len() {
                return Err(format!(
                    "internal error: aggregate parameter `{}` ABI word count mismatch",
                    param.name
                ));
            }
            vars.insert(param.name.clone(), value);
        } else {
            let tmp = *loaded_params.get(&param.name).ok_or_else(|| {
                format!("internal error: missing lowered parameter `{}`", param.name)
            })?;
            vars.insert(param.name.clone(), tmp);
        }
    }

    lower_block(&mut ctx, &func.body, &mut vars);
    ctx.finish_current(Terminator::Return(None));

    let function = Function {
        name: symbol_name.to_string(),
        params: lowered_params,
        blocks: ctx.blocks,
        entry,
        location: func.location,
    };
    if let Some(err) = ctx.error {
        Err(err)
    } else {
        Ok(function)
    }
}

fn lower_entrypoint_wrapper(
    func: &TypedFunction,
    impl_name: &str,
    dyn_iter_cap: usize,
    call_renames: &HashMap<String, String>,
    function_param_specs: &HashMap<String, Vec<TypedParam>>,
    test_mode: bool,
) -> Result<Function, String> {
    let mut ctx = LowerCtx::new(
        dyn_iter_cap,
        call_renames.clone(),
        function_param_specs.clone(),
    );
    let entry = ctx.new_label();
    ctx.start_block(entry);

    let payload = if func.param_types.is_empty() {
        None
    } else {
        Some(load_entrypoint_payload(&mut ctx, test_mode))
    };

    for param in &func.param_types {
        if param.is_state {
            return Err(format!(
                "entrypoint `{}` cannot accept state parameter `{}`",
                func.name, param.name
            ));
        }
    }

    let payload = payload.ok_or_else(|| {
        format!(
            "internal error: missing payload for parameterized entrypoint `{}`",
            func.name
        )
    })?;
    let args = {
        let schema = entrypoint_argument_schema(&func.param_types)?.ok_or_else(|| {
            format!(
                "internal error: parameterized entrypoint `{}` has no argument schema",
                func.name
            )
        })?;
        let encoded_schema = norito::to_bytes(&schema)
            .map_err(|error| format!("failed to encode entrypoint argument schema: {error}"))?;
        let schema_temp = ctx.new_temp();
        ctx.current_instr(Instr::DataRef {
            dest: schema_temp,
            kind: DataRefKind::NoritoBytes,
            value: format!("0x{}", hex::encode(encoded_schema)),
        });
        let decoded_table = ctx.new_temp();
        ctx.current_instr(Instr::DirectHelperSyscall {
            dest: decoded_table,
            syscall: ivm_abi::syscalls::SYSCALL_DECODE_ARGUMENT_RECORD,
            args: vec![payload, schema_temp],
        });
        let mut word_offset = 0_usize;
        let mut args = Vec::with_capacity(func.param_types.len());
        for (param, field) in func.param_types.iter().zip(&schema.fields) {
            let word_count = field.ty.word_count().ok_or_else(|| {
                format!(
                    "entrypoint parameter `{}` has an invalid ABI v1 type schema",
                    param.name
                )
            })?;
            let mut words = Vec::with_capacity(word_count);
            for _ in 0..word_count {
                let dest = ctx.new_temp();
                let index = i16::try_from(word_offset)
                    .map_err(|_| "entrypoint argument index exceeds i16".to_owned())?;
                let imm = ivm_abi::entrypoint::DECODED_ARGUMENT_TABLE_OFFSET
                    .checked_add(
                        index
                            .checked_mul(ivm_abi::entrypoint::DECODED_ARGUMENT_WORD_BYTES)
                            .ok_or_else(|| "entrypoint argument offset overflow".to_owned())?,
                    )
                    .ok_or_else(|| "entrypoint argument offset overflow".to_owned())?;
                ctx.current_instr(Instr::Load64Imm {
                    dest,
                    base: decoded_table,
                    imm,
                });
                words.push(dest);
                word_offset = word_offset.saturating_add(1);
            }
            // Aggregate values have no runtime tuple allocation. Carry their
            // canonical words across the internal call boundary and rebuild
            // the compiler-only tuple shape in the implementation prologue.
            args.extend(words);
        }
        if Some(word_offset) != schema.word_count() {
            return Err(format!(
                "entrypoint `{}` ABI v1 argument table word count mismatch",
                func.name
            ));
        }
        args
    };

    let return_ty = func.ret_ty.as_ref().unwrap_or(&Type::Unit);
    let term = if *return_ty == Type::Unit {
        ctx.current_instr(Instr::Call {
            callee: impl_name.to_string(),
            args,
            dest: None,
        });
        Terminator::Return(None)
    } else if let Some(items) = aggregate_components(return_ty) {
        let mut dests = Vec::with_capacity(items.len());
        for _ in items {
            dests.push(ctx.new_temp());
        }
        ctx.current_instr(Instr::CallMulti {
            callee: impl_name.to_string(),
            args,
            dests: dests.clone(),
        });
        Terminator::ReturnN(dests)
    } else {
        let dest = ctx.new_temp();
        ctx.current_instr(Instr::Call {
            callee: impl_name.to_string(),
            args,
            dest: Some(dest),
        });
        Terminator::Return(Some(dest))
    };
    ctx.finish_current(term);

    let function = Function {
        name: func.name.clone(),
        params: Vec::new(),
        blocks: ctx.blocks,
        entry,
        location: func.location,
    };
    if let Some(err) = ctx.error {
        Err(err)
    } else {
        Ok(function)
    }
}

fn load_entrypoint_payload(ctx: &mut LowerCtx, test_mode: bool) -> Temp {
    if !test_mode {
        let payload = ctx.new_temp();
        ctx.current_instr(Instr::GetTriggerEvent { dest: payload });
        return payload;
    }

    let override_path = ctx.new_temp();
    ctx.current_instr(Instr::DataRef {
        dest: override_path,
        kind: DataRefKind::Name,
        value: TEST_TRIGGER_EVENT_OVERRIDE_KEY.to_string(),
    });
    let override_payload = ctx.new_temp();
    ctx.current_instr(Instr::StateGet {
        dest: override_payload,
        path: override_path,
    });
    let zero = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: zero,
        value: 0,
    });
    let has_override = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: has_override,
        op: BinaryOp::Ne,
        left: override_payload,
        right: zero,
    });

    let override_bb = ctx.new_label();
    let host_bb = ctx.new_label();
    let join_bb = ctx.new_label();
    let payload = ctx.new_temp();

    ctx.finish_current(Terminator::Branch {
        cond: has_override,
        then_bb: override_bb,
        else_bb: host_bb,
    });

    ctx.start_block(override_bb);
    let decoded_override = ctx.new_temp();
    ctx.current_instr(Instr::JsonDecode {
        dest: decoded_override,
        blob: override_payload,
    });
    ctx.current_instr(Instr::Copy {
        dest: payload,
        src: decoded_override,
    });
    ctx.finish_current(Terminator::Jump(join_bb));

    ctx.start_block(host_bb);
    let host_payload = ctx.new_temp();
    ctx.current_instr(Instr::GetTriggerEvent { dest: host_payload });
    ctx.current_instr(Instr::Copy {
        dest: payload,
        src: host_payload,
    });
    ctx.finish_current(Terminator::Jump(join_bb));

    ctx.start_block(join_bb);
    payload
}

fn lower_invoke_entrypoint_call(
    ctx: &mut LowerCtx,
    entrypoint: &str,
    payload_expr: &semantic::TypedExpr,
    result_ty: &semantic::Type,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let override_path = ctx.new_temp();
    ctx.current_instr(Instr::DataRef {
        dest: override_path,
        kind: DataRefKind::Name,
        value: TEST_TRIGGER_EVENT_OVERRIDE_KEY.to_string(),
    });
    let previous_payload = ctx.new_temp();
    ctx.current_instr(Instr::StateGet {
        dest: previous_payload,
        path: override_path,
    });

    let payload = lower_expr(ctx, payload_expr, vars);
    let encoded_payload = ctx.new_temp();
    ctx.current_instr(Instr::JsonEncode {
        dest: encoded_payload,
        json: payload,
    });
    ctx.current_instr(Instr::StateSet {
        path: override_path,
        value: encoded_payload,
    });

    let result = if *result_ty == semantic::Type::Unit {
        ctx.current_instr(Instr::Call {
            callee: entrypoint.to_string(),
            args: Vec::new(),
            dest: None,
        });
        let unit = ctx.new_temp();
        ctx.current_instr(Instr::Const {
            dest: unit,
            value: 0,
        });
        unit
    } else if let Some(items) = aggregate_components(result_ty) {
        let mut dests = Vec::with_capacity(items.len());
        for _ in items {
            dests.push(ctx.new_temp());
        }
        ctx.current_instr(Instr::CallMulti {
            callee: entrypoint.to_string(),
            args: Vec::new(),
            dests: dests.clone(),
        });
        let tuple = ctx.new_temp();
        ctx.current_instr(Instr::TuplePack {
            dest: tuple,
            items: dests,
        });
        tuple
    } else {
        let dest = ctx.new_temp();
        ctx.current_instr(Instr::Call {
            callee: entrypoint.to_string(),
            args: Vec::new(),
            dest: Some(dest),
        });
        dest
    };

    let zero = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: zero,
        value: 0,
    });
    let has_previous = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: has_previous,
        op: BinaryOp::Ne,
        left: previous_payload,
        right: zero,
    });

    let restore_bb = ctx.new_label();
    let clear_bb = ctx.new_label();
    let join_bb = ctx.new_label();
    ctx.finish_current(Terminator::Branch {
        cond: has_previous,
        then_bb: restore_bb,
        else_bb: clear_bb,
    });

    ctx.start_block(restore_bb);
    ctx.current_instr(Instr::StateSet {
        path: override_path,
        value: previous_payload,
    });
    ctx.finish_current(Terminator::Jump(join_bb));

    ctx.start_block(clear_bb);
    ctx.current_instr(Instr::StateDel {
        path: override_path,
    });
    ctx.finish_current(Terminator::Jump(join_bb));

    ctx.start_block(join_bb);
    result
}

fn lower_blob_literal(ctx: &mut LowerCtx, value: &str) -> Temp {
    let dest = ctx.new_temp();
    ctx.current_instr(Instr::DataRef {
        dest,
        kind: DataRefKind::Blob,
        value: value.to_string(),
    });
    dest
}

fn entrypoint_argument_kind(
    param_name: &str,
    ty: &Type,
) -> Result<ivm_abi::entrypoint::EntrypointArgumentKindV1, String> {
    use ivm_abi::entrypoint::EntrypointArgumentKindV1 as Kind;

    let resolved = semantic::resolve_struct_type(ty);
    Ok(match resolved {
        Type::Int => Kind::Int,
        Type::FixedU128 => Kind::U128,
        Type::Amount => Kind::Numeric,
        Type::Bool => Kind::Bool,
        Type::String => Kind::String,
        Type::Json => Kind::Json,
        Type::Name => Kind::Name,
        Type::AccountId => Kind::AccountId,
        Type::AssetDefinitionId => Kind::AssetDefinitionId,
        Type::AssetId => Kind::AssetId,
        Type::DomainId => Kind::DomainId,
        Type::NftId => Kind::NftId,
        Type::DataSpaceId => Kind::DataSpaceId,
        Type::Bytes => Kind::Blob,
        other => {
            return Err(format!(
                "entrypoint parameter `{param_name}` uses unsupported public type {:?}",
                other
            ));
        }
    })
}

fn append_entrypoint_argument_type_nodes(
    param_name: &str,
    ty: &Type,
    nodes: &mut Vec<ivm_abi::entrypoint::EntrypointArgumentTypeNodeV1>,
) -> Result<(), String> {
    use ivm_abi::entrypoint::EntrypointArgumentTypeNodeV1 as Node;

    match semantic::resolve_struct_type(ty) {
        Type::Struct { name, fields } => {
            nodes.push(Node::Struct {
                name,
                fields: fields.iter().map(|(name, _)| name.clone()).collect(),
            });
            for (_, field_ty) in fields {
                append_entrypoint_argument_type_nodes(param_name, &field_ty, nodes)?;
            }
        }
        Type::Tuple(items) => {
            let arity = u16::try_from(items.len()).map_err(|_| {
                format!("entrypoint parameter `{param_name}` tuple arity exceeds u16")
            })?;
            nodes.push(Node::Tuple { arity });
            for item in items {
                append_entrypoint_argument_type_nodes(param_name, &item, nodes)?;
            }
        }
        Type::Option(inner) => {
            nodes.push(Node::Option);
            append_entrypoint_argument_type_nodes(param_name, &inner, nodes)?;
        }
        Type::Result(ok, err) => {
            nodes.push(Node::Result);
            append_entrypoint_argument_type_nodes(param_name, &ok, nodes)?;
            append_entrypoint_argument_type_nodes(param_name, &err, nodes)?;
        }
        leaf => nodes.push(Node::Leaf(entrypoint_argument_kind(param_name, &leaf)?)),
    }
    Ok(())
}

fn entrypoint_argument_type(
    param_name: &str,
    ty: &Type,
) -> Result<ivm_abi::entrypoint::EntrypointArgumentTypeV1, String> {
    let mut nodes = Vec::new();
    append_entrypoint_argument_type_nodes(param_name, ty, &mut nodes)?;
    let ty = ivm_abi::entrypoint::EntrypointArgumentTypeV1 { nodes };
    if !ty.validate() {
        return Err(format!(
            "entrypoint parameter `{param_name}` exceeds ABI v1 type depth, node, or word limits"
        ));
    }
    Ok(ty)
}

pub(crate) fn entrypoint_argument_schema(
    params: &[TypedParam],
) -> Result<Option<ivm_abi::entrypoint::EntrypointArgumentSchemaV1>, String> {
    if params.is_empty() {
        return Ok(None);
    }
    let fields = params
        .iter()
        .map(|param| {
            Ok(ivm_abi::entrypoint::EntrypointArgumentFieldV1 {
                name: param.name.clone(),
                ty: entrypoint_argument_type(&param.name, &param.ty)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let schema = ivm_abi::entrypoint::EntrypointArgumentSchemaV1 { fields };
    if !schema.validate() {
        return Err(format!(
            "entrypoint exceeds the ABI v1 limit of {} source parameters and {} flattened words",
            ivm_abi::entrypoint::MAX_ENTRYPOINT_ARGUMENTS,
            ivm_abi::entrypoint::MAX_ENTRYPOINT_ARGUMENT_WORDS,
        ));
    }
    Ok(Some(schema))
}

fn register_state_value_metadata(ctx: &mut LowerCtx, name: &str, ty: &Type, literal: &str) {
    ctx.state_name_literals
        .insert(name.to_string(), literal.to_string());
    let resolved = semantic::resolve_struct_type(ty);
    match &resolved {
        Type::StateMap(k, v) => {
            let key_ty = semantic::resolve_struct_type(k);
            let value_ty = semantic::resolve_struct_type(v);
            ctx.state_map_configs.insert(
                name.to_string(),
                StateMapSpec {
                    key: key_ty,
                    value: value_ty,
                },
            );
        }
        Type::Struct { .. } | Type::Tuple(_) | Type::Option(_) | Type::Result(_, _) => {}
        _ => {}
    }
}

fn push_copy(block: &mut BasicBlock, dest: Temp, src: Temp) {
    if dest == src {
        return;
    }
    block.instrs.push(Instr::Copy { dest, src });
}

fn collect_expr_reads(expr: &TypedExpr, reads: &mut BTreeSet<String>) {
    match &expr.expr {
        semantic::ExprKind::Binary { left, right, .. } => {
            collect_expr_reads(left, reads);
            collect_expr_reads(right, reads);
        }
        semantic::ExprKind::Unary { expr, .. } | semantic::ExprKind::NumericCast { expr } => {
            collect_expr_reads(expr, reads);
        }
        semantic::ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_expr_reads(cond, reads);
            collect_expr_reads(then_expr, reads);
            collect_expr_reads(else_expr, reads);
        }
        semantic::ExprKind::Call { args, .. } | semantic::ExprKind::Tuple(args) => {
            for arg in args {
                collect_expr_reads(arg, reads);
            }
        }
        semantic::ExprKind::Member { object, .. } => collect_expr_reads(object, reads),
        semantic::ExprKind::Index { target, index } => {
            collect_expr_reads(target, reads);
            collect_expr_reads(index, reads);
        }
        semantic::ExprKind::Ident(name) => {
            reads.insert(name.clone());
        }
        semantic::ExprKind::Number(_)
        | semantic::ExprKind::Decimal(_)
        | semantic::ExprKind::Bool(_)
        | semantic::ExprKind::String(_)
        | semantic::ExprKind::Bytes(_) => {}
    }
}

fn collect_block_reads(block: &TypedBlock, reads: &mut BTreeSet<String>) {
    for statement in &block.statements {
        collect_statement_reads(statement, reads);
    }
}

fn collect_statement_reads(statement: &TypedStatement, reads: &mut BTreeSet<String>) {
    match statement {
        TypedStatement::Let { value, .. } | TypedStatement::Expr(value) => {
            collect_expr_reads(value, reads);
        }
        TypedStatement::Return(Some(value)) => collect_expr_reads(value, reads),
        TypedStatement::Return(None) | TypedStatement::Break | TypedStatement::Continue => {}
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_expr_reads(cond, reads);
            collect_block_reads(then_branch, reads);
            if let Some(else_branch) = else_branch {
                collect_block_reads(else_branch, reads);
            }
        }
        TypedStatement::While { cond, body } => {
            collect_expr_reads(cond, reads);
            collect_block_reads(body, reads);
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init) = init {
                collect_statement_reads(init, reads);
            }
            if let Some(cond) = cond {
                collect_expr_reads(cond, reads);
            }
            if let Some(step) = step {
                collect_statement_reads(step, reads);
            }
            collect_block_reads(body, reads);
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            collect_expr_reads(map, reads);
            collect_block_reads(body, reads);
        }
        TypedStatement::MapSet { map, key, value } => {
            collect_expr_reads(map, reads);
            collect_expr_reads(key, reads);
            collect_expr_reads(value, reads);
        }
    }
}

fn collect_block_mutations(block: &TypedBlock, mutations: &mut BTreeSet<String>) {
    for statement in &block.statements {
        collect_statement_mutations(statement, mutations);
    }
}

fn collect_statement_mutations(statement: &TypedStatement, mutations: &mut BTreeSet<String>) {
    match statement {
        TypedStatement::Let { name, .. } => {
            mutations.insert(name.clone());
        }
        TypedStatement::If {
            then_branch,
            else_branch,
            ..
        } => {
            collect_block_mutations(then_branch, mutations);
            if let Some(else_branch) = else_branch {
                collect_block_mutations(else_branch, mutations);
            }
        }
        TypedStatement::While { body, .. } => collect_block_mutations(body, mutations),
        TypedStatement::For {
            init, step, body, ..
        } => {
            if let Some(init) = init {
                collect_statement_mutations(init, mutations);
            }
            if let Some(step) = step {
                collect_statement_mutations(step, mutations);
            }
            collect_block_mutations(body, mutations);
        }
        TypedStatement::ForEachMap {
            key, value, body, ..
        } => {
            mutations.insert(key.clone());
            if let Some(value) = value {
                mutations.insert(value.clone());
            }
            collect_block_mutations(body, mutations);
        }
        TypedStatement::Expr(_)
        | TypedStatement::Return(_)
        | TypedStatement::Break
        | TypedStatement::Continue
        | TypedStatement::MapSet { .. } => {}
    }
}

fn live_before_statement(
    statement: &TypedStatement,
    live_after: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut live = match statement {
        // A return has no fallthrough edge. Keeping only its expression reads
        // prevents unreachable trailing statements from extending lifetimes.
        TypedStatement::Return(_) => BTreeSet::new(),
        _ => live_after.clone(),
    };
    if let TypedStatement::Let { name, .. } = statement {
        live.remove(name);
    }
    collect_statement_reads(statement, &mut live);
    live
}

fn block_live_after_sets(
    block: &TypedBlock,
    outer_live_after: &BTreeSet<String>,
) -> Vec<BTreeSet<String>> {
    let mut live = outer_live_after.clone();
    let mut result = vec![BTreeSet::new(); block.statements.len()];
    for (index, statement) in block.statements.iter().enumerate().rev() {
        result[index] = live.clone();
        live = if matches!(statement, TypedStatement::Break | TypedStatement::Continue) {
            // Neither statement falls through to the remaining source block.
            // The caller supplies the union of loop-header and loop-exit
            // liveness, which is conservative for both target kinds.
            outer_live_after.clone()
        } else {
            live_before_statement(statement, &live)
        };
    }
    result
}

fn loop_phi_names(
    vars: &HashMap<String, Temp>,
    mutations: &BTreeSet<String>,
    loop_reads: &BTreeSet<String>,
    live_after: &BTreeSet<String>,
) -> BTreeSet<String> {
    mutations
        .iter()
        .filter(|name| {
            vars.contains_key(*name) && (loop_reads.contains(*name) || live_after.contains(*name))
        })
        .cloned()
        .collect()
}

fn initialize_loop_phi(
    ctx: &mut LowerCtx,
    vars: &HashMap<String, Temp>,
    names: &BTreeSet<String>,
) -> HashMap<String, Temp> {
    let mut phi = HashMap::new();
    for name in names {
        let Some(temp) = vars.get(name).copied() else {
            continue;
        };
        let slot = ctx.new_temp();
        ctx.current_instr(Instr::Copy {
            dest: slot,
            src: temp,
        });
        phi.insert(name.clone(), slot);
    }
    phi
}

fn env_with_loop_phi(
    base: &HashMap<String, Temp>,
    phi: &HashMap<String, Temp>,
) -> HashMap<String, Temp> {
    let mut env = base.clone();
    for (name, temp) in phi {
        env.insert(name.clone(), *temp);
    }
    env
}

fn apply_loop_phi(env: &mut HashMap<String, Temp>, phi: &HashMap<String, Temp>) {
    for (name, temp) in phi {
        env.insert(name.clone(), *temp);
    }
}

fn copy_env_to_loop_phi(ctx: &mut LowerCtx, env: &HashMap<String, Temp>) {
    if let Some(phi) = ctx.current_loop_phi() {
        let mut copies: Vec<(Temp, Temp)> = Vec::new();
        let mut entries = phi.iter().collect::<Vec<_>>();
        entries.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (name, dest) in entries {
            if let Some(src) = env.get(name)
                && dest != src
            {
                copies.push((*dest, *src));
            }
        }
        for (dest, src) in copies {
            ctx.current_instr(Instr::Copy { dest, src });
        }
    }
}

fn lower_block(ctx: &mut LowerCtx, block: &TypedBlock, vars: &mut HashMap<String, Temp>) {
    lower_block_with_live_after(ctx, block, vars, &BTreeSet::new());
}

fn lower_block_with_live_after(
    ctx: &mut LowerCtx,
    block: &TypedBlock,
    vars: &mut HashMap<String, Temp>,
    outer_live_after: &BTreeSet<String>,
) {
    let live_after = block_live_after_sets(block, outer_live_after);
    for (statement, statement_live_after) in block.statements.iter().zip(live_after.iter()) {
        lower_statement(ctx, statement, vars, statement_live_after);
    }
}

fn lower_statement(
    ctx: &mut LowerCtx,
    stmt: &TypedStatement,
    vars: &mut HashMap<String, Temp>,
    live_after: &BTreeSet<String>,
) {
    match stmt {
        TypedStatement::Let { name, value } => {
            let t = lower_expr(ctx, value, vars);
            vars.insert(name.clone(), t);
            if ctx.state_name_literals.contains_key(name) {
                emit_state_set(ctx, name, &value.ty, t);
            }
        }
        TypedStatement::Expr(e) => {
            let _ = lower_expr(ctx, e, vars);
        }
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let entry_env = vars.clone();
            let cond_t = lower_expr(ctx, cond, vars);
            let then_label = ctx.new_label();
            let else_label = ctx.new_label();
            let end_label = ctx.new_label();
            ctx.finish_current(Terminator::Branch {
                cond: cond_t,
                then_bb: then_label,
                else_bb: else_label,
            });

            ctx.start_block(then_label);
            let mut then_vars = entry_env.clone();
            lower_block_with_live_after(ctx, then_branch, &mut then_vars, live_after);
            ctx.finish_current(Terminator::Jump(end_label));
            let then_idx = ctx.blocks.len() - 1;

            ctx.start_block(else_label);
            let mut else_vars = entry_env.clone();
            if let Some(b) = else_branch {
                lower_block_with_live_after(ctx, b, &mut else_vars, live_after);
            }
            ctx.finish_current(Terminator::Jump(end_label));
            let else_idx = ctx.blocks.len() - 1;

            let mut mutated: BTreeSet<String> = BTreeSet::new();
            for (name, entry_temp) in entry_env.iter() {
                let then_temp = then_vars.get(name).copied().unwrap_or(*entry_temp);
                let else_temp = else_vars.get(name).copied().unwrap_or(*entry_temp);
                if then_temp != *entry_temp || else_temp != *entry_temp {
                    mutated.insert(name.clone());
                }
            }
            for name in mutated {
                let join_temp = ctx.new_temp();
                let entry_temp = entry_env
                    .get(&name)
                    .copied()
                    .expect("entry env must contain variable");
                let then_temp = then_vars.get(&name).copied().unwrap_or(entry_temp);
                let else_temp = else_vars.get(&name).copied().unwrap_or(entry_temp);
                if let Some(block) = ctx.blocks.get_mut(then_idx) {
                    push_copy(block, join_temp, then_temp);
                }
                if let Some(block) = ctx.blocks.get_mut(else_idx) {
                    push_copy(block, join_temp, else_temp);
                }
                vars.insert(name, join_temp);
            }

            ctx.start_block(end_label);
        }
        TypedStatement::While { cond, body } => {
            let cond_label = ctx.new_label();
            let body_label = ctx.new_label();
            let end_label = ctx.new_label();
            let entry_vars = vars.clone();
            let mut mutations = BTreeSet::new();
            collect_block_mutations(body, &mut mutations);
            let mut loop_reads = BTreeSet::new();
            collect_expr_reads(cond, &mut loop_reads);
            collect_block_reads(body, &mut loop_reads);
            let phi_names = loop_phi_names(vars, &mutations, &loop_reads, live_after);
            let loop_phi = initialize_loop_phi(ctx, vars, &phi_names);
            let loop_env = env_with_loop_phi(&entry_vars, &loop_phi);
            ctx.push_loop(cond_label, end_label);
            ctx.set_loop_phi(loop_phi.clone());
            ctx.finish_current(Terminator::Jump(cond_label));

            ctx.start_block(cond_label);
            let mut cond_vars = loop_env;
            let cond_t = lower_expr(ctx, cond, &mut cond_vars);
            ctx.finish_current(Terminator::Branch {
                cond: cond_t,
                then_bb: body_label,
                else_bb: end_label,
            });

            ctx.start_block(body_label);
            let mut body_vars = cond_vars;
            let mut body_live_after = loop_reads;
            body_live_after.extend(live_after.iter().cloned());
            lower_block_with_live_after(ctx, body, &mut body_vars, &body_live_after);
            copy_env_to_loop_phi(ctx, &body_vars);
            ctx.finish_current(Terminator::Jump(cond_label));

            ctx.pop_loop();
            ctx.start_block(end_label);
            *vars = entry_vars;
            apply_loop_phi(vars, &loop_phi);
        }
        TypedStatement::For {
            line: _,
            init,
            cond,
            step,
            body,
        } => {
            if let Some(s) = init {
                lower_statement(ctx, s, vars, &BTreeSet::new());
            }
            let cond_label = ctx.new_label();
            let body_label = ctx.new_label();
            let step_label = ctx.new_label();
            let end_label = ctx.new_label();
            let entry_vars = vars.clone();
            let mut mutations = BTreeSet::new();
            collect_block_mutations(body, &mut mutations);
            if let Some(step) = step {
                collect_statement_mutations(step, &mut mutations);
            }
            let mut loop_reads = BTreeSet::new();
            if let Some(cond) = cond {
                collect_expr_reads(cond, &mut loop_reads);
            }
            if let Some(step) = step {
                collect_statement_reads(step, &mut loop_reads);
            }
            collect_block_reads(body, &mut loop_reads);
            let phi_names = loop_phi_names(vars, &mutations, &loop_reads, live_after);
            let loop_phi = initialize_loop_phi(ctx, vars, &phi_names);
            let loop_env = env_with_loop_phi(&entry_vars, &loop_phi);
            ctx.push_loop(step_label, end_label);
            ctx.set_loop_phi(loop_phi.clone());
            ctx.finish_current(Terminator::Jump(cond_label));

            ctx.start_block(cond_label);
            let mut cond_vars = loop_env;
            let cond_t = if let Some(c) = cond {
                lower_expr(ctx, c, &mut cond_vars)
            } else {
                let t = ctx.new_temp();
                ctx.current_instr(Instr::Const { dest: t, value: 1 });
                t
            };
            ctx.finish_current(Terminator::Branch {
                cond: cond_t,
                then_bb: body_label,
                else_bb: end_label,
            });

            ctx.start_block(body_label);
            let mut body_vars = cond_vars;
            let mut body_live_after = loop_reads.clone();
            body_live_after.extend(live_after.iter().cloned());
            lower_block_with_live_after(ctx, body, &mut body_vars, &body_live_after);
            copy_env_to_loop_phi(ctx, &body_vars);
            ctx.finish_current(Terminator::Jump(step_label));

            ctx.start_block(step_label);
            if let Some(s) = step {
                let mut step_vars = env_with_loop_phi(&entry_vars, &loop_phi);
                lower_statement(ctx, s, &mut step_vars, &body_live_after);
                copy_env_to_loop_phi(ctx, &step_vars);
            }
            ctx.finish_current(Terminator::Jump(cond_label));

            ctx.pop_loop();
            ctx.start_block(end_label);
            *vars = entry_vars;
            apply_loop_phi(vars, &loop_phi);
        }
        TypedStatement::Break => {
            if let Some((_, brk)) = ctx.loop_targets() {
                copy_env_to_loop_phi(ctx, vars);
                ctx.finish_current(Terminator::Jump(brk));
                let cont = ctx.new_label();
                ctx.start_block(cont);
            }
        }
        TypedStatement::Continue => {
            if let Some((cont, _)) = ctx.loop_targets() {
                copy_env_to_loop_phi(ctx, vars);
                ctx.finish_current(Terminator::Jump(cont));
                let next = ctx.new_label();
                ctx.start_block(next);
            }
        }
        TypedStatement::Return(opt) => {
            if let Some(e) = opt {
                // Aggregate returns use the V1 multi-register return convention.
                if let Some(components) = aggregate_components(&e.ty) {
                    let tup = lower_expr(ctx, e, vars);
                    let mut outs = Vec::with_capacity(components.len());
                    for i in 0..components.len() {
                        let d = ctx.new_temp();
                        ctx.current_instr(Instr::TupleGet {
                            dest: d,
                            tuple: tup,
                            index: i,
                        });
                        outs.push(d);
                    }
                    match outs.len() {
                        0 => ctx.finish_current(Terminator::Return(None)),
                        1 => ctx.finish_current(Terminator::Return(Some(outs[0]))),
                        2 => ctx.finish_current(Terminator::Return2(outs[0], outs[1])),
                        _ => ctx.finish_current(Terminator::ReturnN(outs)),
                    }
                } else {
                    // Non-tuple return
                    let t = lower_expr(ctx, e, vars);
                    ctx.finish_current(Terminator::Return(Some(t)));
                }
            } else {
                ctx.finish_current(Terminator::Return(None));
            }
            // Start a fresh (unreachable) block to continue lowering subsequent statements gracefully
            let cont = ctx.new_label();
            ctx.start_block(cont);
        }
        TypedStatement::ForEachMap {
            key,
            value,
            map,
            body,
            start,
            bound,
            ..
        } => {
            if let Some(base_name) = state_map_base_name(map)
                && let Some(spec) = ctx.state_map_configs.get(&base_name).cloned()
                && key_codec_for_type(&spec.key).is_some()
            {
                lower_state_foreach_map(
                    ctx,
                    key,
                    value,
                    map,
                    body,
                    *start,
                    *bound,
                    &base_name,
                    &spec.key,
                    &spec.value,
                    vars,
                    live_after,
                );
                return;
            }
            // Deterministic bounded lowering using a compact loop (stride = 16 bytes per pair).
            let base = lower_expr(ctx, map, vars);
            // In-memory maps use a fixed single-entry layout; clamp iterations to 1 to avoid
            // reading past the allocated pair.
            let max_iters = bound.unwrap_or(1).min(1);
            let max_iters_i64 = (max_iters.min(i64::MAX as usize)) as i64;
            // Non-state maps always start at index 0; clamp defensively to avoid OOB loads.
            debug_assert_eq!(*start, 0);
            let base_start_i64: i64 = 0;
            let limit_value = base_start_i64.saturating_add(max_iters_i64);

            let index = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: index,
                value: base_start_i64,
            });
            let limit = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: limit,
                value: limit_value,
            });
            let sixteen = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: sixteen,
                value: 16,
            });
            let one = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: one,
                value: 1,
            });

            let loop_label = ctx.new_label();
            let body_label = ctx.new_label();
            let step_label = ctx.new_label();
            let exit_label = ctx.new_label();
            let entry_vars = vars.clone();
            let mut mutations = BTreeSet::new();
            collect_block_mutations(body, &mut mutations);
            let mut loop_reads = BTreeSet::new();
            collect_block_reads(body, &mut loop_reads);
            let phi_names = loop_phi_names(vars, &mutations, &loop_reads, live_after);
            let loop_phi = initialize_loop_phi(ctx, vars, &phi_names);
            let loop_env = env_with_loop_phi(&entry_vars, &loop_phi);
            ctx.push_loop(step_label, exit_label);
            ctx.set_loop_phi(loop_phi.clone());
            ctx.finish_current(Terminator::Jump(loop_label));

            ctx.start_block(loop_label);
            let cond_t = ctx.new_temp();
            ctx.current_instr(Instr::Binary {
                dest: cond_t,
                op: BinaryOp::Lt,
                left: index,
                right: limit,
            });
            ctx.finish_current(Terminator::Branch {
                cond: cond_t,
                then_bb: body_label,
                else_bb: exit_label,
            });

            ctx.start_block(body_label);
            let mut body_vars = loop_env;
            let offset_bytes = ctx.new_temp();
            ctx.current_instr(Instr::Binary {
                dest: offset_bytes,
                op: BinaryOp::Mul,
                left: index,
                right: sixteen,
            });
            let addr = ctx.new_temp();
            ctx.current_instr(Instr::Binary {
                dest: addr,
                op: BinaryOp::Add,
                left: base,
                right: offset_bytes,
            });
            let key_temp = ctx.new_temp();
            ctx.current_instr(Instr::Load64Imm {
                dest: key_temp,
                base: addr,
                imm: 0,
            });
            let value_temp = ctx.new_temp();
            ctx.current_instr(Instr::Load64Imm {
                dest: value_temp,
                base: addr,
                imm: 8,
            });
            body_vars.insert(key.clone(), key_temp);
            if let Some(val_name) = value {
                body_vars.insert(val_name.clone(), value_temp);
            }
            let mut body_live_after = loop_reads;
            body_live_after.extend(live_after.iter().cloned());
            lower_block_with_live_after(ctx, body, &mut body_vars, &body_live_after);
            copy_env_to_loop_phi(ctx, &body_vars);
            ctx.finish_current(Terminator::Jump(step_label));

            ctx.start_block(step_label);
            ctx.current_instr(Instr::Binary {
                dest: index,
                op: BinaryOp::Add,
                left: index,
                right: one,
            });
            ctx.finish_current(Terminator::Jump(loop_label));

            ctx.pop_loop();
            ctx.start_block(exit_label);
            *vars = entry_vars;
            apply_loop_phi(vars, &loop_phi);
        }
        TypedStatement::MapSet { map, key, value } => {
            let key_tmp = lower_expr(ctx, key, vars);
            let value_tmp = lower_expr(ctx, value, vars);
            if let Some(bn) = state_map_base_name(map)
                && let Some(spec) = ctx.state_map_configs.get(&bn).cloned()
            {
                let _ =
                    lower_state_map_set_value(ctx, &bn, key_tmp, &spec.key, &spec.value, value_tmp);
                return;
            }
            let m = lower_expr(ctx, map, vars);
            ctx.current_instr(Instr::MapSet {
                map: m,
                key: key_tmp,
                value: value_tmp,
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_state_foreach_map(
    ctx: &mut LowerCtx,
    key: &str,
    value: &Option<String>,
    _map: &TypedExpr,
    body: &TypedBlock,
    start: usize,
    bound: Option<usize>,
    base_name: &str,
    key_ty: &Type,
    value_ty: &Type,
    vars: &mut HashMap<String, Temp>,
    live_after: &BTreeSet<String>,
) {
    let offset = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: offset,
        value: start.min(i64::MAX as usize) as i64,
    });
    let limit = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: limit,
        value: bound.unwrap_or(0).min(i64::MAX as usize) as i64,
    });
    lower_state_foreach_page(
        ctx, key, value, body, offset, limit, base_name, key_ty, value_ty, vars, live_after,
    );
}

fn decode_state_map_key(ctx: &mut LowerCtx, key_blob: Temp, key_ty: &Type) -> Option<Temp> {
    match semantic::resolve_struct_type(key_ty) {
        Type::Int | Type::Bool => {
            let key = ctx.new_temp();
            ctx.current_instr(Instr::DecodeInt {
                dest: key,
                blob: key_blob,
            });
            Some(key)
        }
        ty if semantic::is_wide_numeric_type(&ty) => Some(key_blob),
        Type::String | Type::Bytes => {
            let key = ctx.new_temp();
            ctx.current_instr(Instr::PointerFromNorito {
                dest: key,
                blob: key_blob,
                kind: DataRefKind::Blob,
            });
            Some(key)
        }
        ty if semantic::is_pointer_type(&ty) => {
            let kind = pointer_kind_for_type(&ty)?;
            let key = ctx.new_temp();
            ctx.current_instr(Instr::PointerFromNorito {
                dest: key,
                blob: key_blob,
                kind,
            });
            Some(key)
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_state_foreach_page(
    ctx: &mut LowerCtx,
    key_name: &str,
    value_name: &Option<String>,
    body: &TypedBlock,
    offset: Temp,
    limit: Temp,
    base_name: &str,
    key_ty: &Type,
    value_ty: &Type,
    vars: &mut HashMap<String, Temp>,
    live_after: &BTreeSet<String>,
) {
    let prefix = build_state_base(ctx, base_name);
    let page = ctx.new_temp();
    ctx.current_instr(Instr::StateKeys {
        dest: page,
        prefix,
        offset,
        limit,
    });

    let index = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: index,
        value: 0,
    });
    let one = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: one,
        value: 1,
    });

    let loop_label = ctx.new_label();
    let body_label = ctx.new_label();
    let step_label = ctx.new_label();
    let exit_label = ctx.new_label();
    let entry_vars = vars.clone();
    let mut mutations = BTreeSet::new();
    collect_block_mutations(body, &mut mutations);
    let mut loop_reads = BTreeSet::new();
    collect_block_reads(body, &mut loop_reads);
    let phi_names = loop_phi_names(vars, &mutations, &loop_reads, live_after);
    let loop_phi = initialize_loop_phi(ctx, vars, &phi_names);
    let loop_env = env_with_loop_phi(&entry_vars, &loop_phi);
    ctx.push_loop(step_label, exit_label);
    ctx.set_loop_phi(loop_phi.clone());
    ctx.finish_current(Terminator::Jump(loop_label));

    ctx.start_block(loop_label);
    let cond_t = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: cond_t,
        op: BinaryOp::Lt,
        left: index,
        right: limit,
    });
    ctx.finish_current(Terminator::Branch {
        cond: cond_t,
        then_bb: body_label,
        else_bb: exit_label,
    });

    ctx.start_block(body_label);
    let key_blob = ctx.new_temp();
    ctx.current_instr(Instr::StateMapKeyAt {
        dest: key_blob,
        page,
        base: prefix,
        index,
    });
    let zero = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: zero,
        value: 0,
    });
    let has_key = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: has_key,
        op: BinaryOp::Ne,
        left: key_blob,
        right: zero,
    });
    let present_bb = ctx.new_label();
    ctx.finish_current(Terminator::Branch {
        cond: has_key,
        then_bb: present_bb,
        else_bb: exit_label,
    });

    ctx.start_block(present_bb);
    let key_temp = decode_state_map_key(ctx, key_blob, key_ty).unwrap_or_else(|| {
        ctx.record_error("durable StateMap iteration key type is not decodable".into());
        zero
    });
    let value_temp = lower_state_map_get_value(ctx, base_name, key_temp, key_ty, value_ty)
        .unwrap_or_else(|| {
            ctx.record_error("durable StateMap iteration value type is not decodable".into());
            zero
        });
    let mut body_vars = loop_env;
    body_vars.insert(key_name.to_string(), key_temp);
    if let Some(val_name) = value_name {
        body_vars.insert(val_name.clone(), value_temp);
    }
    let mut body_live_after = loop_reads;
    body_live_after.extend(live_after.iter().cloned());
    lower_block_with_live_after(ctx, body, &mut body_vars, &body_live_after);
    copy_env_to_loop_phi(ctx, &body_vars);
    ctx.finish_current(Terminator::Jump(step_label));

    ctx.start_block(step_label);
    ctx.current_instr(Instr::Binary {
        dest: index,
        op: BinaryOp::Add,
        left: index,
        right: one,
    });
    ctx.finish_current(Terminator::Jump(loop_label));

    ctx.pop_loop();
    ctx.start_block(exit_label);
    *vars = entry_vars;
    apply_loop_phi(vars, &loop_phi);
}

fn lower_expr_as_int(
    ctx: &mut LowerCtx,
    expr: &TypedExpr,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let value = lower_expr(ctx, expr, vars);
    if semantic::is_wide_numeric_type(&expr.ty) {
        let out = ctx.new_temp();
        ctx.current_instr(Instr::NumericToInt { dest: out, value });
        out
    } else {
        value
    }
}

fn lower_expr_as_numeric(
    ctx: &mut LowerCtx,
    expr: &TypedExpr,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let value = lower_expr(ctx, expr, vars);
    if semantic::is_wide_numeric_type(&expr.ty) {
        value
    } else {
        let out = ctx.new_temp();
        ctx.current_instr(Instr::NumericFromInt { dest: out, value });
        out
    }
}

/// Select a source builtin's direct host operation exclusively through its
/// canonical registry record.
///
/// This is deliberately fail-closed: passing an instruction-only or derived
/// builtin is a compiler invariant violation, not an opportunity for lowering
/// to invent an unregistered syscall number.
fn direct_builtin_syscall(builtin: Builtin) -> u32 {
    let spec = builtin.spec();
    assert_eq!(
        spec.lowering,
        BuiltinLowering::DirectSyscall,
        "IR lowering requested a direct syscall for non-direct builtin {}",
        spec.name
    );
    spec.syscall.unwrap_or_else(|| {
        panic!(
            "direct builtin {} has no syscall in the canonical registry",
            spec.name
        )
    })
}

fn numeric_binary_builtin_op(builtin: Builtin) -> Option<BinaryOp> {
    Some(match builtin {
        Builtin::NumericAdd => BinaryOp::Add,
        Builtin::NumericSub => BinaryOp::Sub,
        Builtin::NumericMul => BinaryOp::Mul,
        Builtin::NumericDiv => BinaryOp::Div,
        Builtin::NumericRem => BinaryOp::Mod,
        _ => return None,
    })
}

fn numeric_compare_builtin_op(builtin: Builtin) -> Option<BinaryOp> {
    Some(match builtin {
        Builtin::NumericEq => BinaryOp::Eq,
        Builtin::NumericNe => BinaryOp::Ne,
        Builtin::NumericLt => BinaryOp::Lt,
        Builtin::NumericLe => BinaryOp::Le,
        Builtin::NumericGt => BinaryOp::Gt,
        Builtin::NumericGe => BinaryOp::Ge,
        _ => return None,
    })
}

fn lower_hash_builtin_call(
    ctx: &mut LowerCtx,
    builtin: Builtin,
    args: &[semantic::TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let message = lower_expr(ctx, &args[0], vars);
    let dest = ctx.new_temp();
    let instr = match builtin {
        Builtin::Sm3Hash => Instr::Sm3Hash { dest, message },
        Builtin::Sha256Hash => Instr::Sha256Hash { dest, message },
        Builtin::Sha3Hash => Instr::Sha3Hash { dest, message },
        Builtin::Blake2b256Hash => Instr::Blake2b256Hash { dest, message },
        Builtin::Keccak256Hash => Instr::Keccak256Hash { dest, message },
        Builtin::IrohaHash => Instr::IrohaHash { dest, message },
        _ => unreachable!("non-hash builtin passed to lower_hash_builtin_call"),
    };
    ctx.current_instr(instr);
    dest
}

fn lower_signature_builtin_call(
    ctx: &mut LowerCtx,
    builtin: Builtin,
    args: &[semantic::TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let message = lower_expr(ctx, &args[0], vars);
    let signature = lower_expr(ctx, &args[1], vars);
    let public_key = lower_expr(ctx, &args[2], vars);
    let dest = ctx.new_temp();
    match builtin {
        Builtin::Sm2Verify => {
            let distid = args.get(3).map(|arg| lower_expr(ctx, arg, vars));
            ctx.current_instr(Instr::Sm2Verify {
                dest,
                message,
                signature,
                public_key,
                distid,
            });
        }
        Builtin::VerifySignature => {
            let scheme = lower_expr(ctx, &args[3], vars);
            ctx.current_instr(Instr::VerifySignature {
                dest,
                message,
                signature,
                public_key,
                scheme,
            });
        }
        _ => unreachable!("non-signature builtin passed to lower_signature_builtin_call"),
    }
    dest
}

fn lower_sm4_builtin_call(
    ctx: &mut LowerCtx,
    builtin: Builtin,
    args: &[semantic::TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let key = lower_expr(ctx, &args[0], vars);
    let nonce = lower_expr(ctx, &args[1], vars);
    let aad = lower_expr(ctx, &args[2], vars);
    let data = lower_expr(ctx, &args[3], vars);
    let dest = ctx.new_temp();
    match builtin {
        Builtin::Sm4GcmSeal => ctx.current_instr(Instr::Sm4GcmSeal {
            dest,
            key,
            nonce,
            aad,
            plaintext: data,
        }),
        Builtin::Sm4GcmOpen => ctx.current_instr(Instr::Sm4GcmOpen {
            dest,
            key,
            nonce,
            aad,
            ciphertext_and_tag: data,
        }),
        Builtin::Sm4CcmSeal => {
            let tag_len = args.get(4).map(|arg| lower_expr(ctx, arg, vars));
            ctx.current_instr(Instr::Sm4CcmSeal {
                dest,
                key,
                nonce,
                aad,
                plaintext: data,
                tag_len,
            });
        }
        Builtin::Sm4CcmOpen => {
            let tag_len = args.get(4).map(|arg| lower_expr(ctx, arg, vars));
            ctx.current_instr(Instr::Sm4CcmOpen {
                dest,
                key,
                nonce,
                aad,
                ciphertext_and_tag: data,
                tag_len,
            });
        }
        _ => unreachable!("non-SM4 builtin passed to lower_sm4_builtin_call"),
    }
    dest
}

fn lower_direct_helper_call(
    ctx: &mut LowerCtx,
    builtin: Builtin,
    args: &[semantic::TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let syscall = direct_builtin_syscall(builtin);
    let mut lowered_args = Vec::with_capacity(args.len());
    for (idx, arg) in args.iter().enumerate() {
        let temp = if builtin == Builtin::JsonSetIntDirect && idx == 2 {
            lower_expr_as_int(ctx, arg, vars)
        } else {
            lower_expr(ctx, arg, vars)
        };
        lowered_args.push(temp);
    }
    let dest = ctx.new_temp();
    ctx.current_instr(Instr::DirectHelperSyscall {
        dest,
        syscall,
        args: lowered_args,
    });
    dest
}

fn pointer_constructor_kind_and_type(constructor: PointerConstructor) -> (DataRefKind, Type) {
    match constructor {
        PointerConstructor::AccountId => (DataRefKind::Account, Type::AccountId),
        PointerConstructor::AssetDefinition => (DataRefKind::AssetDef, Type::AssetDefinitionId),
        PointerConstructor::AssetId => (DataRefKind::AssetId, Type::AssetId),
        PointerConstructor::NftId => (DataRefKind::NftId, Type::NftId),
        PointerConstructor::Domain | PointerConstructor::DomainId => {
            (DataRefKind::Domain, Type::DomainId)
        }
        PointerConstructor::Name => (DataRefKind::Name, Type::Name),
        PointerConstructor::Json => (DataRefKind::Json, Type::Json),
        PointerConstructor::Blob => (DataRefKind::Blob, Type::Bytes),
        PointerConstructor::NoritoBytes => (DataRefKind::NoritoBytes, Type::Bytes),
        PointerConstructor::DataSpaceId => (DataRefKind::DataSpaceId, Type::DataSpaceId),
        PointerConstructor::AxtDescriptor => (DataRefKind::AxtDescriptor, Type::AxtDescriptor),
        PointerConstructor::AssetHandle => (DataRefKind::AssetHandle, Type::AssetHandle),
        PointerConstructor::ProofBlob => (DataRefKind::ProofBlob, Type::ProofBlob),
        PointerConstructor::SoracloudRequest => {
            (DataRefKind::SoracloudRequest, Type::SoracloudRequest)
        }
        PointerConstructor::SoracloudResponse => {
            (DataRefKind::SoracloudResponse, Type::SoracloudResponse)
        }
    }
}

fn lower_pointer_constructor_call(
    ctx: &mut LowerCtx,
    constructor: PointerConstructor,
    args: &[semantic::TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    if args.len() != 1 {
        let t = ctx.new_temp();
        ctx.current_instr(Instr::Const { dest: t, value: 0 });
        return t;
    }
    let arg = &args[0];
    let (kind, target_ty) = pointer_constructor_kind_and_type(constructor);
    if let semantic::ExprKind::String(s) = &arg.expr {
        if constructor == PointerConstructor::AccountId
            && account_id_literal_uses_alias_resolution(s)
        {
            let alias = ctx.new_temp();
            ctx.current_instr(Instr::DataRef {
                dest: alias,
                kind: DataRefKind::Blob,
                value: s.clone(),
            });
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::ResolveAccountAlias { dest, alias });
            return dest;
        }
        let dest = ctx.new_temp();
        ctx.current_instr(Instr::DataRef {
            dest,
            kind,
            value: s.clone(),
        });
        return dest;
    }
    if constructor == PointerConstructor::NoritoBytes
        && let semantic::ExprKind::Bytes(bytes) = &arg.expr
    {
        let dest = ctx.new_temp();
        let hex = hex::encode(bytes);
        ctx.current_instr(Instr::DataRef {
            dest,
            kind: DataRefKind::NoritoBytes,
            value: format!("0x{hex}"),
        });
        return dest;
    }
    let src = lower_expr(ctx, arg, vars);
    let resolved_arg = semantic::resolve_struct_type(&arg.ty);
    match (target_ty.clone(), resolved_arg.clone()) {
        (Type::Bytes, _) => src,
        (t, arg_ty) if t == arg_ty => src,
        (Type::Json, ty) if semantic::is_blob_like(&ty) => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::JsonDecode { dest, blob: src });
            dest
        }
        (Type::Name, ty) if semantic::is_blob_like(&ty) => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::NameDecode { dest, blob: src });
            dest
        }
        (_, Type::String) => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::PointerFromString { dest, kind, src });
            dest
        }
        (_, ty) if semantic::is_blob_like(&ty) => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::PointerFromNorito {
                dest,
                blob: src,
                kind,
            });
            dest
        }
        _ => src,
    }
}

fn lower_transfer_batch_call(
    ctx: &mut LowerCtx,
    args: &[semantic::TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    ctx.current_instr(Instr::TransferBatchBegin);
    for entry in args {
        let tuple = lower_expr(ctx, entry, vars);
        let from = ctx.new_temp();
        ctx.current_instr(Instr::TupleGet {
            dest: from,
            tuple,
            index: 0,
        });
        let to = ctx.new_temp();
        ctx.current_instr(Instr::TupleGet {
            dest: to,
            tuple,
            index: 1,
        });
        let asset = ctx.new_temp();
        ctx.current_instr(Instr::TupleGet {
            dest: asset,
            tuple,
            index: 2,
        });
        let amount_raw = ctx.new_temp();
        ctx.current_instr(Instr::TupleGet {
            dest: amount_raw,
            tuple,
            index: 3,
        });
        let amount = if let Type::Tuple(items) = semantic::resolve_struct_type(&entry.ty) {
            let entry_ty = items.get(3);
            if entry_ty.is_some_and(semantic::is_wide_numeric_type) {
                amount_raw
            } else {
                let out = ctx.new_temp();
                ctx.current_instr(Instr::NumericFromInt {
                    dest: out,
                    value: amount_raw,
                });
                out
            }
        } else {
            let out = ctx.new_temp();
            ctx.current_instr(Instr::NumericFromInt {
                dest: out,
                value: amount_raw,
            });
            out
        };
        ctx.current_instr(Instr::TransferBatchAsset {
            from,
            to,
            asset,
            amount,
        });
    }
    ctx.current_instr(Instr::TransferBatchEnd);
    let t = ctx.new_temp();
    ctx.current_instr(Instr::Const { dest: t, value: 0 });
    t
}

fn lower_surface_builtin_call(
    ctx: &mut LowerCtx,
    builtin: Builtin,
    args: &[semantic::TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    match builtin {
        Builtin::PointerConstructor(constructor) => {
            lower_pointer_constructor_call(ctx, constructor, args, vars)
        }
        Builtin::JsonSetIntDirect
        | Builtin::JsonSetAccountIdDirect
        | Builtin::JsonGetIntDirect
        | Builtin::JsonGetNumericDirect
        | Builtin::JsonGetJsonDirect
        | Builtin::JsonGetNameDirect
        | Builtin::JsonGetAccountIdDirect
        | Builtin::JsonGetAssetDefinitionIdDirect
        | Builtin::JsonGetNftIdDirect
        | Builtin::JsonGetBlobHexDirect
        | Builtin::BuildPathKeyNoritoDirect
        | Builtin::SchemaEncodeDirect
        | Builtin::SchemaDecodeDirect
        | Builtin::SchemaInfoDirect
        | Builtin::NumericToIntDirect
        | Builtin::NumericAddDirect
        | Builtin::NumericSubDirect
        | Builtin::NumericMulDirect
        | Builtin::NumericDivDirect
        | Builtin::NumericRemDirect
        | Builtin::NumericNegDirect
        | Builtin::NumericEqDirect
        | Builtin::NumericNeDirect
        | Builtin::NumericLtDirect
        | Builtin::NumericLeDirect
        | Builtin::NumericGtDirect
        | Builtin::NumericGeDirect => lower_direct_helper_call(ctx, builtin, args, vars),
        Builtin::SchemaEncode => {
            let schema = lower_expr(ctx, &args[0], vars);
            let json = lower_expr(ctx, &args[1], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::SchemaEncode { dest, schema, json });
            dest
        }
        Builtin::SchemaDecode => {
            let schema = lower_expr(ctx, &args[0], vars);
            let blob = lower_expr(ctx, &args[1], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::SchemaDecode { dest, schema, blob });
            dest
        }
        Builtin::SchemaInfo => {
            let schema = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::SchemaInfo { dest, schema });
            dest
        }
        Builtin::NumericNeg => {
            let value = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::NumericNeg { dest, value });
            dest
        }
        Builtin::JsonObject => {
            let d = ctx.new_temp();
            ctx.current_instr(Instr::JsonObject { dest: d });
            d
        }
        Builtin::JsonSetInt => {
            let j = lower_expr(ctx, &args[0], vars);
            let k = lower_expr(ctx, &args[1], vars);
            let v = lower_expr_as_int(ctx, &args[2], vars);
            let d = ctx.new_temp();
            ctx.current_instr(Instr::JsonSetInt {
                dest: d,
                json: j,
                key: k,
                value: v,
            });
            d
        }
        Builtin::JsonSetAccountId => {
            let j = lower_expr(ctx, &args[0], vars);
            let k = lower_expr(ctx, &args[1], vars);
            let v = lower_expr(ctx, &args[2], vars);
            let d = ctx.new_temp();
            ctx.current_instr(Instr::JsonSetAccountId {
                dest: d,
                json: j,
                key: k,
                value: v,
            });
            d
        }
        Builtin::EncodeInt => {
            let value = lower_expr_as_int(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::EncodeInt { dest, value });
            dest
        }
        Builtin::DecodeInt => {
            let blob = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::DecodeInt { dest, blob });
            dest
        }
        Builtin::EncodeJson => {
            let json = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::JsonEncode { dest, json });
            dest
        }
        Builtin::DecodeJson => {
            let blob = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::JsonDecode { dest, blob });
            dest
        }
        Builtin::GetInt => {
            let j = lower_expr(ctx, &args[0], vars);
            let k = lower_expr(ctx, &args[1], vars);
            let d = ctx.new_temp();
            ctx.current_instr(Instr::JsonGetInt {
                dest: d,
                json: j,
                key: k,
            });
            d
        }
        Builtin::GetNumeric => {
            let j = lower_expr(ctx, &args[0], vars);
            let k = lower_expr(ctx, &args[1], vars);
            let d = ctx.new_temp();
            ctx.current_instr(Instr::JsonGetNumeric {
                dest: d,
                json: j,
                key: k,
            });
            d
        }
        Builtin::GetJson => {
            let j = lower_expr(ctx, &args[0], vars);
            let k = lower_expr(ctx, &args[1], vars);
            let d = ctx.new_temp();
            ctx.current_instr(Instr::JsonGetJson {
                dest: d,
                json: j,
                key: k,
            });
            d
        }
        Builtin::GetName => {
            let j = lower_expr(ctx, &args[0], vars);
            let k = lower_expr(ctx, &args[1], vars);
            let d = ctx.new_temp();
            ctx.current_instr(Instr::JsonGetName {
                dest: d,
                json: j,
                key: k,
            });
            d
        }
        Builtin::GetAccountId => {
            let j = lower_expr(ctx, &args[0], vars);
            let k = lower_expr(ctx, &args[1], vars);
            let d = ctx.new_temp();
            ctx.current_instr(Instr::JsonGetAccountId {
                dest: d,
                json: j,
                key: k,
            });
            d
        }
        Builtin::GetAssetDefinitionId => {
            let j = lower_expr(ctx, &args[0], vars);
            let k = lower_expr(ctx, &args[1], vars);
            let d = ctx.new_temp();
            ctx.current_instr(Instr::JsonGetAssetDefinitionId {
                dest: d,
                json: j,
                key: k,
            });
            d
        }
        Builtin::GetNftId => {
            let j = lower_expr(ctx, &args[0], vars);
            let k = lower_expr(ctx, &args[1], vars);
            let d = ctx.new_temp();
            ctx.current_instr(Instr::JsonGetNftId {
                dest: d,
                json: j,
                key: k,
            });
            d
        }
        Builtin::GetBlobHex => {
            let j = lower_expr(ctx, &args[0], vars);
            let k = lower_expr(ctx, &args[1], vars);
            let d = ctx.new_temp();
            ctx.current_instr(Instr::JsonGetBlobHex {
                dest: d,
                json: j,
                key: k,
            });
            d
        }
        Builtin::TriggerEvent => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::GetTriggerEvent { dest });
            dest
        }
        Builtin::Authority => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::GetAuthority { dest });
            dest
        }
        Builtin::SysvarAuthority => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::SysvarAuthority { dest });
            dest
        }
        Builtin::CurrentTimeMs => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::CurrentTimeMs { dest });
            dest
        }
        Builtin::BlockHeight => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::BlockHeight { dest });
            dest
        }
        Builtin::BlockTimeMs => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::BlockTimeMs { dest });
            dest
        }
        Builtin::ChainId => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::ChainId { dest });
            dest
        }
        Builtin::ContractAddress => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::ContractAddress { dest });
            dest
        }
        Builtin::Entrypoint => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::Entrypoint { dest });
            dest
        }
        Builtin::Path => {
            let base = lower_expr(ctx, &args[0], vars);
            let d = ctx.new_temp();
            if semantic::is_numeric_type(&args[1].ty) {
                let key = lower_expr_as_int(ctx, &args[1], vars);
                ctx.current_instr(Instr::PathMapKey { dest: d, base, key });
            } else if semantic::is_blob_like(&args[1].ty) {
                let blob = lower_expr(ctx, &args[1], vars);
                ctx.current_instr(Instr::PathMapKeyNorito {
                    dest: d,
                    base,
                    key_blob: blob,
                });
            } else {
                panic!("path expects an i64-like or bytes-like key")
            }
            d
        }
        Builtin::NameDecode => {
            let blob = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::NameDecode { dest, blob });
            dest
        }
        Builtin::TlvEq => {
            let left = lower_expr(ctx, &args[0], vars);
            let right = lower_expr(ctx, &args[1], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::PointerEq { dest, left, right });
            dest
        }
        Builtin::TlvLen => {
            let value = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::TlvLen { dest, value });
            dest
        }
        Builtin::PointerToNorito => {
            let value = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::PointerToNorito { dest, value });
            dest
        }
        Builtin::NumericToInt => {
            let value = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::NumericToInt { dest, value });
            dest
        }
        builtin @ (Builtin::NumericAdd
        | Builtin::NumericSub
        | Builtin::NumericMul
        | Builtin::NumericDiv
        | Builtin::NumericRem) => {
            let left = lower_expr(ctx, &args[0], vars);
            let right = lower_expr(ctx, &args[1], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::NumericBinary {
                dest,
                op: numeric_binary_builtin_op(builtin).expect("numeric binary builtin op"),
                left,
                right,
            });
            dest
        }
        builtin @ (Builtin::NumericEq
        | Builtin::NumericNe
        | Builtin::NumericLt
        | Builtin::NumericLe
        | Builtin::NumericGt
        | Builtin::NumericGe) => {
            let left = lower_expr(ctx, &args[0], vars);
            let right = lower_expr(ctx, &args[1], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::NumericCompare {
                dest,
                op: numeric_compare_builtin_op(builtin).expect("numeric compare builtin op"),
                left,
                right,
            });
            dest
        }
        Builtin::WrappingNeg => {
            let operand = lower_expr_as_int(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::WrappingNeg { dest, operand });
            dest
        }
        builtin @ (Builtin::WrappingAdd | Builtin::WrappingSub | Builtin::WrappingMul) => {
            let left = lower_expr_as_int(ctx, &args[0], vars);
            let right = lower_expr_as_int(ctx, &args[1], vars);
            let op = match builtin {
                Builtin::WrappingAdd => BinaryOp::Add,
                Builtin::WrappingSub => BinaryOp::Sub,
                Builtin::WrappingMul => BinaryOp::Mul,
                _ => unreachable!(),
            };
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::WrappingBinary {
                dest,
                op,
                left,
                right,
            });
            dest
        }
        Builtin::Isqrt => {
            let src = lower_expr_as_int(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::Isqrt { dest, src });
            dest
        }
        Builtin::Abs => {
            let src = lower_expr_as_int(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::Abs { dest, src });
            dest
        }
        Builtin::Min => {
            let a = lower_expr_as_int(ctx, &args[0], vars);
            let b = lower_expr_as_int(ctx, &args[1], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::Min { dest, a, b });
            dest
        }
        Builtin::Max => {
            let a = lower_expr_as_int(ctx, &args[0], vars);
            let b = lower_expr_as_int(ctx, &args[1], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::Max { dest, a, b });
            dest
        }
        Builtin::DivCeil => {
            let num = lower_expr_as_int(ctx, &args[0], vars);
            let denom = lower_expr_as_int(ctx, &args[1], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::DivCeil { dest, num, denom });
            dest
        }
        Builtin::Gcd => {
            let a = lower_expr_as_int(ctx, &args[0], vars);
            let b = lower_expr_as_int(ctx, &args[1], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::Gcd { dest, a, b });
            dest
        }
        Builtin::Mean => {
            let a = lower_expr_as_int(ctx, &args[0], vars);
            let b = lower_expr_as_int(ctx, &args[1], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::Mean { dest, a, b });
            dest
        }
        Builtin::Poseidon2 => {
            let a = lower_expr_as_int(ctx, &args[0], vars);
            let b = lower_expr_as_int(ctx, &args[1], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::Poseidon2 { dest, a, b });
            dest
        }
        Builtin::Poseidon6 => {
            let mut lowered_args = [Temp(0); 6];
            for (index, arg) in args.iter().enumerate() {
                lowered_args[index] = lower_expr_as_int(ctx, arg, vars);
            }
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::Poseidon6 {
                dest,
                args: lowered_args,
            });
            dest
        }
        Builtin::Pubkgen => {
            let src = lower_expr_as_int(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::Pubkgen { dest, src });
            dest
        }
        Builtin::Valcom => {
            let value = lower_expr_as_int(ctx, &args[0], vars);
            let blind = lower_expr_as_int(ctx, &args[1], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::Valcom { dest, value, blind });
            dest
        }
        Builtin::SetVl => {
            let value = lower_expr_as_int(ctx, &args[0], vars);
            ctx.current_instr(Instr::SetVl { value });
            let temp = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: temp,
                value: 0,
            });
            temp
        }
        Builtin::StateGet => {
            let p = lower_expr(ctx, &args[0], vars);
            let d = ctx.new_temp();
            ctx.current_instr(Instr::StateGet { dest: d, path: p });
            d
        }
        Builtin::StateSet => {
            let path = lower_expr(ctx, &args[0], vars);
            let val = lower_expr(ctx, &args[1], vars);
            ctx.current_instr(Instr::StateSet { path, value: val });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::StateDel => {
            let path = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::StateDel { path });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::StateKeys => {
            let prefix = lower_expr(ctx, &args[0], vars);
            let offset = lower_expr_as_int(ctx, &args[1], vars);
            let limit = lower_expr_as_int(ctx, &args[2], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::StateKeys {
                dest,
                prefix,
                offset,
                limit,
            });
            dest
        }
        Builtin::StateHas => {
            let path = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::StateHas { dest, path });
            dest
        }
        Builtin::StateLen => {
            let path = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::StateLen { dest, path });
            dest
        }
        Builtin::StateCount => {
            let prefix = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::StateCount { dest, prefix });
            dest
        }
        Builtin::QueryExecuteNorito => {
            let payload = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::QueryExecuteNorito { dest, payload });
            dest
        }
        Builtin::QueryGetAccount
        | Builtin::QueryGetAsset
        | Builtin::QueryGetAssetDefinition
        | Builtin::QueryGetDomain
        | Builtin::QueryGetNft
        | Builtin::QueryGetParameter
        | Builtin::QueryGetContractManifest
        | Builtin::QueryGetContractInstance => {
            let key = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::QueryGet {
                dest,
                key,
                syscall: direct_builtin_syscall(builtin),
            });
            dest
        }
        Builtin::BuildSubmitBallotInline => {
            let election_id = lower_expr(ctx, &args[0], vars);
            let ciphertext = lower_expr(ctx, &args[1], vars);
            let nullifier = lower_expr(ctx, &args[2], vars);
            let backend = lower_expr(ctx, &args[3], vars);
            let proof = lower_expr(ctx, &args[4], vars);
            let vk = lower_expr(ctx, &args[5], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::BuildSubmitBallotInline {
                dest,
                election_id,
                ciphertext,
                nullifier,
                backend,
                proof,
                vk,
            });
            dest
        }
        Builtin::BuildUnshieldInline => {
            let asset = lower_expr(ctx, &args[0], vars);
            let to = lower_expr(ctx, &args[1], vars);
            let amount = lower_expr_as_int(ctx, &args[2], vars);
            let inputs = lower_expr(ctx, &args[3], vars);
            let (outputs, backend_idx) = if args.len() == 8 {
                (Some(lower_expr(ctx, &args[4], vars)), 5)
            } else {
                (None, 4)
            };
            let backend = lower_expr(ctx, &args[backend_idx], vars);
            let proof = lower_expr(ctx, &args[backend_idx + 1], vars);
            let vk = lower_expr(ctx, &args[backend_idx + 2], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::BuildUnshieldInline {
                dest,
                asset,
                to,
                amount,
                inputs,
                outputs,
                backend,
                proof,
                vk,
            });
            dest
        }
        Builtin::RecordSccpMessage
        | Builtin::ScExecuteSubmitBallot
        | Builtin::ScExecuteUnshield => {
            let payload = lower_expr(ctx, &args[0], vars);
            let kind = match builtin {
                Builtin::RecordSccpMessage => VendorInstructionKind::RecordSccpMessage,
                Builtin::ScExecuteSubmitBallot => VendorInstructionKind::SubmitBallot,
                Builtin::ScExecuteUnshield => VendorInstructionKind::Unshield,
                _ => unreachable!("matched operation-specific instruction bridge"),
            };
            ctx.current_instr(Instr::VendorExecuteInstruction { payload, kind });
            let temp = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: temp,
                value: 0,
            });
            temp
        }
        Builtin::ExecuteQuery => {
            let payload = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::VendorExecuteQuery { dest, payload });
            dest
        }
        Builtin::CallContract => {
            let contract = lower_expr(ctx, &args[0], vars);
            let entrypoint = lower_expr(ctx, &args[1], vars);
            let payload = lower_expr(ctx, &args[2], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::CallContract {
                dest,
                contract,
                entrypoint,
                payload,
            });
            dest
        }
        Builtin::ResolveAccountAlias => {
            let alias = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::ResolveAccountAlias { dest, alias });
            dest
        }
        Builtin::SubscriptionBill => {
            ctx.current_instr(Instr::SubscriptionBill);
            let temp = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: temp,
                value: 0,
            });
            temp
        }
        Builtin::SubscriptionRecordUsage => {
            ctx.current_instr(Instr::SubscriptionRecordUsage);
            let temp = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: temp,
                value: 0,
            });
            temp
        }
        Builtin::GetAccountBalance => {
            let account = lower_expr(ctx, &args[0], vars);
            let asset = lower_expr(ctx, &args[1], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::GetAccountBalance {
                dest,
                account,
                asset,
            });
            dest
        }
        Builtin::GetPublicInput => {
            let key = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::GetPublicInput { dest, key });
            dest
        }
        Builtin::DebugPrint => {
            let value = lower_expr_as_int(ctx, &args[0], vars);
            ctx.current_instr(Instr::DebugPrint { value });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::DebugLog => {
            let payload = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::DebugLog { payload });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::Assert => {
            let cond = lower_expr(ctx, &args[0], vars);
            if args.len() > 1 {
                let _ = lower_expr(ctx, &args[1], vars);
            }
            ctx.current_instr(Instr::Assert { cond });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::Require => {
            let cond = lower_expr(ctx, &args[0], vars);
            let code = lower_expr_as_int(ctx, &args[1], vars);
            let reject = ctx.new_temp();
            ctx.current_instr(Instr::Unary {
                dest: reject,
                op: UnaryOp::Not,
                operand: cond,
            });
            ctx.current_instr(Instr::AbortIf { cond: reject, code });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::Info => {
            let msg = if semantic::is_numeric_type(&args[0].ty) {
                let value = lower_expr_as_int(ctx, &args[0], vars);
                let encoded = ctx.new_temp();
                ctx.current_instr(Instr::EncodeInt {
                    dest: encoded,
                    value,
                });
                encoded
            } else {
                lower_expr(ctx, &args[0], vars)
            };
            ctx.current_instr(Instr::Info { msg });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::AssertEq => {
            let left = lower_expr(ctx, &args[0], vars);
            let right = lower_expr(ctx, &args[1], vars);
            ctx.current_instr(Instr::AssertEq { left, right });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::SetAccountDetail => {
            let account = lower_expr(ctx, &args[0], vars);
            let key = lower_expr(ctx, &args[1], vars);
            let value = lower_expr(ctx, &args[2], vars);
            ctx.current_instr(Instr::SetAccountDetail {
                account,
                key,
                value,
            });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::MintAsset => {
            let acc = lower_expr(ctx, &args[0], vars);
            let asset = lower_expr(ctx, &args[1], vars);
            let amt = match &args[2].expr {
                semantic::ExprKind::Number(0) if !semantic::is_wide_numeric_type(&args[2].ty) => {
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                _ => lower_expr_as_numeric(ctx, &args[2], vars),
            };
            ctx.current_instr(Instr::MintAsset {
                account: acc,
                asset,
                amount: amt,
            });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::BurnAsset => {
            let acc = lower_expr(ctx, &args[0], vars);
            let asset = lower_expr(ctx, &args[1], vars);
            let amt = lower_expr_as_numeric(ctx, &args[2], vars);
            ctx.current_instr(Instr::BurnAsset {
                account: acc,
                asset,
                amount: amt,
            });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::TransferAsset => {
            let from = lower_expr(ctx, &args[0], vars);
            let to = lower_expr(ctx, &args[1], vars);
            let asset = lower_expr(ctx, &args[2], vars);
            let amt = lower_expr_as_numeric(ctx, &args[3], vars);
            let dataspace = lower_expr(ctx, &args[4], vars);
            ctx.current_instr(Instr::TransferAsset {
                from,
                to,
                asset,
                amount: amt,
                dataspace,
            });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::NftMintAsset => {
            let nft = lower_expr(ctx, &args[0], vars);
            let owner = lower_expr(ctx, &args[1], vars);
            ctx.current_instr(Instr::CreateNft { nft, owner });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::NftSetMetadata => {
            let nft = lower_expr(ctx, &args[0], vars);
            let key = lower_expr(ctx, &args[1], vars);
            let json = lower_expr(ctx, &args[2], vars);
            ctx.current_instr(Instr::SetNftData { nft, key, json });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::NftBurnAsset => {
            let nft = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::BurnNft { nft });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::NftTransferAsset => {
            let from = lower_expr(ctx, &args[0], vars);
            let nft = lower_expr(ctx, &args[1], vars);
            let to = lower_expr(ctx, &args[2], vars);
            ctx.current_instr(Instr::TransferNft { from, nft, to });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::RegisterDomain => {
            let domain = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::RegisterDomain { domain });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::UnregisterDomain => {
            let domain = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::UnregisterDomain { domain });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::TransferDomain => {
            let domain = lower_expr(ctx, &args[1], vars);
            let to = lower_expr(ctx, &args[2], vars);
            ctx.current_instr(Instr::TransferDomain { domain, to });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::RegisterAccount => {
            let account = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::RegisterAccount { account });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::UnregisterAccount => {
            let account = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::UnregisterAccount { account });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::RegisterAsset => {
            let asset = lower_expr(ctx, &args[0], vars);
            let symbol = lower_expr(ctx, &args[1], vars);
            let quantity = lower_expr_as_numeric(ctx, &args[2], vars);
            let mintable = lower_expr(ctx, &args[3], vars);
            ctx.current_instr(Instr::RegisterAsset {
                asset,
                symbol,
                quantity,
                mintable,
            });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::CreateNewAsset => {
            let asset = lower_expr(ctx, &args[0], vars);
            let symbol = lower_expr(ctx, &args[1], vars);
            let quantity = lower_expr_as_numeric(ctx, &args[2], vars);
            let account = lower_expr(ctx, &args[3], vars);
            let mintable = lower_expr(ctx, &args[4], vars);
            ctx.current_instr(Instr::CreateNewAsset {
                asset,
                symbol,
                quantity,
                account,
                mintable,
            });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::UnregisterAsset => {
            let asset = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::UnregisterAsset { asset });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::RegisterPeer => {
            let json = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::RegisterPeer { json });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::UnregisterPeer => {
            let json = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::UnregisterPeer { json });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::CreateTrigger | Builtin::RegisterTrigger => {
            let json = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::CreateTrigger { json });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::RemoveTrigger | Builtin::UnregisterTrigger => {
            let name = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::RemoveTrigger { name });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::SetTriggerEnabled => {
            let name = lower_expr(ctx, &args[0], vars);
            let enabled = lower_expr(ctx, &args[1], vars);
            ctx.current_instr(Instr::SetTriggerEnabled { name, enabled });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::CreateRole => {
            let name = lower_expr(ctx, &args[0], vars);
            let json = lower_expr(ctx, &args[1], vars);
            ctx.current_instr(Instr::CreateRole { name, json });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::DeleteRole => {
            let name = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::DeleteRole { name });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::GrantRole => {
            let account = lower_expr(ctx, &args[0], vars);
            let name = lower_expr(ctx, &args[1], vars);
            ctx.current_instr(Instr::GrantRole { account, name });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::RevokeRole => {
            let account = lower_expr(ctx, &args[0], vars);
            let name = lower_expr(ctx, &args[1], vars);
            ctx.current_instr(Instr::RevokeRole { account, name });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::GrantPermission => {
            let account = lower_expr(ctx, &args[0], vars);
            let token = lower_expr(ctx, &args[1], vars);
            ctx.current_instr(Instr::GrantPermission { account, token });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::RevokePermission => {
            let account = lower_expr(ctx, &args[0], vars);
            let token = lower_expr(ctx, &args[1], vars);
            ctx.current_instr(Instr::RevokePermission { account, token });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::EscrowOpenOffer => {
            let escrow = lower_expr(ctx, &args[0], vars);
            let asset = lower_expr(ctx, &args[1], vars);
            let amount = lower_expr_as_numeric(ctx, &args[2], vars);
            let evidence_hashes = args.get(3).map(|arg| lower_expr(ctx, arg, vars));
            ctx.current_instr(Instr::EscrowOpenOffer {
                escrow,
                asset,
                amount,
                evidence_hashes,
            });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::EscrowAccept => {
            let escrow = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::EscrowAccept { escrow });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::EscrowMarkPaymentSent => {
            let escrow = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::EscrowMarkPaymentSent { escrow });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::EscrowRelease => {
            let escrow = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::EscrowRelease { escrow });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::EscrowCancel => {
            let escrow = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::EscrowCancel { escrow });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::EscrowOpenDispute => {
            let escrow = lower_expr(ctx, &args[0], vars);
            let evidence_hashes = args.get(1).map(|arg| lower_expr(ctx, arg, vars));
            ctx.current_instr(Instr::EscrowOpenDispute {
                escrow,
                evidence_hashes,
            });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::EscrowResolveDispute => {
            let escrow = lower_expr(ctx, &args[0], vars);
            let buyer_amount = lower_expr_as_numeric(ctx, &args[1], vars);
            let seller_amount = lower_expr_as_numeric(ctx, &args[2], vars);
            let evidence_hashes = args.get(3).map(|arg| lower_expr(ctx, arg, vars));
            ctx.current_instr(Instr::EscrowResolveDispute {
                escrow,
                buyer_amount,
                seller_amount,
                evidence_hashes,
            });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::AnonymousEscrowOpenOffer => {
            let request = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::AnonymousEscrowOpenOffer { request });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::AnonymousEscrowAccept => {
            let escrow = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::AnonymousEscrowAccept { escrow });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::AnonymousEscrowMarkPaymentSent => {
            let escrow = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::AnonymousEscrowMarkPaymentSent { escrow });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::AnonymousEscrowRelease => {
            let request = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::AnonymousEscrowRelease { request });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::AnonymousEscrowCancel => {
            let request = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::AnonymousEscrowCancel { request });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::AnonymousEscrowOpenDispute => {
            let escrow = lower_expr(ctx, &args[0], vars);
            let evidence_hashes = args.get(1).map(|arg| lower_expr(ctx, arg, vars));
            ctx.current_instr(Instr::AnonymousEscrowOpenDispute {
                escrow,
                evidence_hashes,
            });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::AnonymousEscrowResolveDispute => {
            let request = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::AnonymousEscrowResolveDispute { request });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::Alloc => {
            let bytes = lower_expr_as_int(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::Alloc { dest, bytes });
            dest
        }
        Builtin::GetPrivateInput => {
            let index = lower_expr_as_int(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::GetPrivateInput { dest, index });
            dest
        }
        Builtin::UseNullifier => {
            let nullifier = lower_expr_as_int(ctx, &args[0], vars);
            ctx.current_instr(Instr::UseNullifier { nullifier });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::CommitOutput => {
            ctx.current_instr(Instr::CommitOutput);
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::CreateNftsForAllUsers => {
            ctx.current_instr(Instr::CreateNftsForAllUsers);
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::SetExecutionDepth => {
            let value = lower_expr_as_int(ctx, &args[0], vars);
            ctx.current_instr(Instr::SetExecutionDepth { value });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::TransferV1BatchBegin => {
            ctx.current_instr(Instr::TransferBatchBegin);
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::TransferV1BatchEnd => {
            ctx.current_instr(Instr::TransferBatchEnd);
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::TransferV1BatchApply => {
            let payload = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::TransferBatchApply { payload });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::TransferBatch => lower_transfer_batch_call(ctx, args, vars),
        Builtin::AxtBegin => {
            let desc = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::AxtBegin { descriptor: desc });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::AxtTouch => {
            let dsid = lower_expr(ctx, &args[0], vars);
            let manifest = args.get(1).map(|m| lower_expr(ctx, m, vars));
            ctx.current_instr(Instr::AxtTouch { dsid, manifest });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::VerifyDsProof => {
            let dsid = lower_expr(ctx, &args[0], vars);
            let proof = args.get(1).map(|p| lower_expr(ctx, p, vars));
            ctx.current_instr(Instr::VerifyDsProof { dsid, proof });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::UseAssetHandle => {
            let handle = lower_expr(ctx, &args[0], vars);
            let intent = lower_expr(ctx, &args[1], vars);
            let proof = args.get(2).map(|p| lower_expr(ctx, p, vars));
            ctx.current_instr(Instr::UseAssetHandle {
                handle,
                intent,
                proof,
            });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::AxtCommit => {
            ctx.current_instr(Instr::AxtCommit);
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::DeactivateContractInstance
        | Builtin::RemoveSmartContractBytes
        | Builtin::RegisterSmartContractCode
        | Builtin::RegisterSmartContractBytes
        | Builtin::ActivateContractInstance => {
            let payload = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::SmartContractLifecycle {
                payload,
                syscall: direct_builtin_syscall(builtin),
            });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::ZkRootsGet => {
            let payload = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::ZkRootsGet { dest, payload });
            dest
        }
        Builtin::ZkVoteGetTally => {
            let payload = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::ZkVoteGetTally { dest, payload });
            dest
        }
        builtin @ (Builtin::ZkVerifyTransfer
        | Builtin::ZkVerifyUnshield
        | Builtin::ZkVerifyBatch
        | Builtin::ZkVoteVerifyBallot
        | Builtin::ZkVoteVerifyTally) => {
            let payload = lower_expr(ctx, &args[0], vars);
            ctx.current_instr(Instr::ZkVerify {
                number: direct_builtin_syscall(builtin),
                payload,
            });
            let temp = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: temp,
                value: 0,
            });
            temp
        }
        Builtin::VrfEpochSeed => {
            let payload = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::VrfEpochSeed { dest, payload });
            dest
        }
        Builtin::VrfVerify => {
            let input = lower_expr(ctx, &args[0], vars);
            let public_key = lower_expr(ctx, &args[1], vars);
            let proof = lower_expr(ctx, &args[2], vars);
            let variant = lower_expr(ctx, &args[3], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::VrfVerify {
                dest,
                input,
                public_key,
                proof,
                variant,
            });
            dest
        }
        Builtin::VrfVerifyBatch => {
            let batch = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::VrfVerifyBatch { dest, batch });
            dest
        }
        Builtin::Sm3Hash
        | Builtin::Sha256Hash
        | Builtin::Sha3Hash
        | Builtin::Blake2b256Hash
        | Builtin::Keccak256Hash
        | Builtin::IrohaHash => lower_hash_builtin_call(ctx, builtin, args, vars),
        Builtin::Sm2Verify | Builtin::VerifySignature => {
            lower_signature_builtin_call(ctx, builtin, args, vars)
        }
        Builtin::Sm4GcmSeal | Builtin::Sm4GcmOpen | Builtin::Sm4CcmSeal | Builtin::Sm4CcmOpen => {
            lower_sm4_builtin_call(ctx, builtin, args, vars)
        }
        Builtin::ProveExecution => {
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::ProveExecution { dest });
            dest
        }
        Builtin::GrowHeap => {
            let bytes = lower_expr_as_int(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::GrowHeap { dest, bytes });
            dest
        }
        Builtin::GetMerklePath => {
            let address = lower_expr_as_int(ctx, &args[0], vars);
            let output = lower_expr_as_int(ctx, &args[1], vars);
            let root_output = args.get(2).map(|arg| lower_expr_as_int(ctx, arg, vars));
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::GetMerklePath {
                dest,
                address,
                output,
                root_output,
            });
            dest
        }
        Builtin::GetMerkleCompact => {
            let address = lower_expr_as_int(ctx, &args[0], vars);
            let output = lower_expr_as_int(ctx, &args[1], vars);
            let max_depth = args.get(2).map(|arg| lower_expr_as_int(ctx, arg, vars));
            let root_output = args.get(3).map(|arg| lower_expr_as_int(ctx, arg, vars));
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::GetMerkleCompact {
                dest,
                address,
                output,
                max_depth,
                root_output,
            });
            dest
        }
        Builtin::GetRegisterMerkleCompact => {
            let register_index = lower_expr_as_int(ctx, &args[0], vars);
            let output = lower_expr_as_int(ctx, &args[1], vars);
            let max_depth = args.get(2).map(|arg| lower_expr_as_int(ctx, arg, vars));
            let root_output = args.get(3).map(|arg| lower_expr_as_int(ctx, arg, vars));
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::GetRegisterMerkleCompact {
                dest,
                register_index,
                output,
                max_depth,
                root_output,
            });
            dest
        }
        Builtin::VerifyProof => {
            let payload = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::VerifyProof { dest, payload });
            dest
        }
        Builtin::SoracloudReadCommittedState
        | Builtin::SoracloudEmitStateMutation
        | Builtin::SoracloudEmitMailboxMessage
        | Builtin::SoracloudAppendJournal
        | Builtin::SoracloudPublishCheckpoint
        | Builtin::SoracloudReadSecret
        | Builtin::SoracloudReadCredential
        | Builtin::SoracloudEgressFetch
        | Builtin::SoracloudReadConfig
        | Builtin::SoracloudReadSecretEnvelope => {
            let request = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::SoracloudHostCall {
                dest,
                request,
                syscall: direct_builtin_syscall(builtin),
            });
            dest
        }
        Builtin::AddSignatory | Builtin::RemoveSignatory => {
            let account = lower_expr(ctx, &args[0], vars);
            let signatory = lower_expr(ctx, &args[1], vars);
            if builtin == Builtin::AddSignatory {
                ctx.current_instr(Instr::AddSignatory { account, signatory });
            } else {
                ctx.current_instr(Instr::RemoveSignatory { account, signatory });
            }
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::SetAccountQuorum => {
            let account = lower_expr(ctx, &args[0], vars);
            let quorum = lower_expr_as_int(ctx, &args[1], vars);
            ctx.current_instr(Instr::SetAccountQuorum { account, quorum });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::Contains => {
            let mexpr = &args[0];
            let kexpr = &args[1];
            let key_tmp = lower_expr(ctx, kexpr, vars);
            if let Some(bn) = state_map_base_name(mexpr)
                && let Some(spec) = ctx.state_map_configs.get(&bn).cloned()
                && let Some(key_codec) = key_codec_for_type(&spec.key)
            {
                let t_path = build_state_path(ctx, &bn, key_tmp, &key_codec);
                let t_blob = ctx.new_temp();
                ctx.current_instr(Instr::StateGet {
                    dest: t_blob,
                    path: t_path,
                });
                let zero = ctx.new_temp();
                ctx.current_instr(Instr::Const {
                    dest: zero,
                    value: 0,
                });
                let out = ctx.new_temp();
                ctx.current_instr(Instr::Binary {
                    dest: out,
                    op: BinaryOp::Ne,
                    left: t_blob,
                    right: zero,
                });
                return out;
            }
            let m = lower_expr(ctx, mexpr, vars);
            let sk = ctx.new_temp();
            let dummy_v = ctx.new_temp();
            ctx.current_instr(Instr::MapLoadPair {
                dest_key: sk,
                dest_val: dummy_v,
                map: m,
                offset: 0,
            });
            lower_map_key_eq(ctx, &kexpr.ty, sk, key_tmp)
        }
        Builtin::GetOrDefault => {
            let mexpr = &args[0];
            let kexpr = &args[1];
            let dexpr = &args[2];
            let key_tmp = lower_expr(ctx, kexpr, vars);
            if let Some(bn) = state_map_base_name(mexpr)
                && let Some(spec) = ctx.state_map_configs.get(&bn).cloned()
                && let Some(key_codec) = key_codec_for_type(&spec.key)
            {
                let t_path = build_state_path(ctx, &bn, key_tmp, &key_codec);
                let t_blob = ctx.new_temp();
                ctx.current_instr(Instr::StateGet {
                    dest: t_blob,
                    path: t_path,
                });
                let zero = ctx.new_temp();
                ctx.current_instr(Instr::Const {
                    dest: zero,
                    value: 0,
                });
                let result = ctx.new_temp();
                let cond = ctx.new_temp();
                ctx.current_instr(Instr::Binary {
                    dest: cond,
                    op: BinaryOp::Ne,
                    left: t_blob,
                    right: zero,
                });
                let then_bb = ctx.new_label();
                let else_bb = ctx.new_label();
                let end_bb = ctx.new_label();
                ctx.finish_current(Terminator::Branch {
                    cond,
                    then_bb,
                    else_bb,
                });
                ctx.start_block(then_bb);
                let decoded = decode_state_map_value_blob(ctx, t_blob, &spec.value)
                    .expect("durable map value should decode");
                ctx.current_instr(Instr::Copy {
                    dest: result,
                    src: decoded,
                });
                ctx.finish_current(Terminator::Jump(end_bb));
                ctx.start_block(else_bb);
                let def = lower_expr(ctx, dexpr, vars);
                ctx.current_instr(Instr::Copy {
                    dest: result,
                    src: def,
                });
                ctx.finish_current(Terminator::Jump(end_bb));
                ctx.start_block(end_bb);
                return result;
            }
            let m = lower_expr(ctx, mexpr, vars);
            let d = lower_expr(ctx, dexpr, vars);
            let sk = ctx.new_temp();
            let sv = ctx.new_temp();
            ctx.current_instr(Instr::MapLoadPair {
                dest_key: sk,
                dest_val: sv,
                map: m,
                offset: 0,
            });
            let zero = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: zero,
                value: 0,
            });
            let result = ctx.new_temp();
            let cond = lower_map_key_eq(ctx, &kexpr.ty, sk, key_tmp);
            let then_bb = ctx.new_label();
            let else_bb = ctx.new_label();
            let end_bb = ctx.new_label();
            ctx.finish_current(Terminator::Branch {
                cond,
                then_bb,
                else_bb,
            });
            ctx.start_block(then_bb);
            ctx.current_instr(Instr::Binary {
                dest: result,
                op: BinaryOp::Add,
                left: sv,
                right: zero,
            });
            ctx.finish_current(Terminator::Jump(end_bb));
            ctx.start_block(else_bb);
            ctx.current_instr(Instr::Binary {
                dest: result,
                op: BinaryOp::Add,
                left: d,
                right: zero,
            });
            ctx.finish_current(Terminator::Jump(end_bb));
            ctx.start_block(end_bb);
            result
        }
        Builtin::GetOr => {
            let mexpr = &args[0];
            let kexpr = &args[1];
            let dexpr = &args[2];
            let key_tmp = lower_expr(ctx, kexpr, vars);
            if let Some(bn) = state_map_base_name(mexpr)
                && let Some(spec) = ctx.state_map_configs.get(&bn).cloned()
                && let Some(key_codec) = key_codec_for_type(&spec.key)
            {
                let t_path = build_state_path(ctx, &bn, key_tmp, &key_codec);
                let t_blob = ctx.new_temp();
                ctx.current_instr(Instr::StateGet {
                    dest: t_blob,
                    path: t_path,
                });
                let zero = ctx.new_temp();
                ctx.current_instr(Instr::Const {
                    dest: zero,
                    value: 0,
                });
                let result = ctx.new_temp();
                let cond = ctx.new_temp();
                ctx.current_instr(Instr::Binary {
                    dest: cond,
                    op: BinaryOp::Ne,
                    left: t_blob,
                    right: zero,
                });
                let then_bb = ctx.new_label();
                let else_bb = ctx.new_label();
                let end_bb = ctx.new_label();
                ctx.finish_current(Terminator::Branch {
                    cond,
                    then_bb,
                    else_bb,
                });
                ctx.start_block(then_bb);
                let existing = decode_state_map_value_blob(ctx, t_blob, &spec.value)
                    .expect("durable map value should decode");
                ctx.current_instr(Instr::Copy {
                    dest: result,
                    src: existing,
                });
                ctx.finish_current(Terminator::Jump(end_bb));
                ctx.start_block(else_bb);
                let def = lower_expr(ctx, dexpr, vars);
                ctx.current_instr(Instr::Copy {
                    dest: result,
                    src: def,
                });
                ctx.finish_current(Terminator::Jump(end_bb));
                ctx.start_block(end_bb);
                return result;
            }
            let m = lower_expr(ctx, mexpr, vars);
            let sk = ctx.new_temp();
            let sv = ctx.new_temp();
            ctx.current_instr(Instr::MapLoadPair {
                dest_key: sk,
                dest_val: sv,
                map: m,
                offset: 0,
            });
            let zero = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: zero,
                value: 0,
            });
            let result = ctx.new_temp();
            let cond = lower_map_key_eq(ctx, &kexpr.ty, sk, key_tmp);
            let then_bb = ctx.new_label();
            let else_bb = ctx.new_label();
            let end_bb = ctx.new_label();
            ctx.finish_current(Terminator::Branch {
                cond,
                then_bb,
                else_bb,
            });
            ctx.start_block(then_bb);
            ctx.current_instr(Instr::Binary {
                dest: result,
                op: BinaryOp::Add,
                left: sv,
                right: zero,
            });
            ctx.finish_current(Terminator::Jump(end_bb));
            ctx.start_block(else_bb);
            let def = lower_expr(ctx, dexpr, vars);
            ctx.current_instr(Instr::Binary {
                dest: result,
                op: BinaryOp::Add,
                left: def,
                right: zero,
            });
            ctx.finish_current(Terminator::Jump(end_bb));
            ctx.start_block(end_bb);
            result
        }
        Builtin::Ensure => {
            let mexpr = &args[0];
            let kexpr = &args[1];
            let dexpr = &args[2];
            let key_tmp = lower_expr(ctx, kexpr, vars);
            if let Some(bn) = state_map_base_name(mexpr)
                && let Some(spec) = ctx.state_map_configs.get(&bn).cloned()
                && let Some(key_codec) = key_codec_for_type(&spec.key)
            {
                let t_path = build_state_path(ctx, &bn, key_tmp, &key_codec);
                let t_blob = ctx.new_temp();
                ctx.current_instr(Instr::StateGet {
                    dest: t_blob,
                    path: t_path,
                });
                let zero = ctx.new_temp();
                ctx.current_instr(Instr::Const {
                    dest: zero,
                    value: 0,
                });
                let result = ctx.new_temp();
                let cond = ctx.new_temp();
                ctx.current_instr(Instr::Binary {
                    dest: cond,
                    op: BinaryOp::Ne,
                    left: t_blob,
                    right: zero,
                });
                let then_bb = ctx.new_label();
                let else_bb = ctx.new_label();
                let end_bb = ctx.new_label();
                ctx.finish_current(Terminator::Branch {
                    cond,
                    then_bb,
                    else_bb,
                });
                ctx.start_block(then_bb);
                let existing = decode_state_map_value_blob(ctx, t_blob, &spec.value)
                    .expect("durable map value should decode");
                ctx.current_instr(Instr::Copy {
                    dest: result,
                    src: existing,
                });
                ctx.finish_current(Terminator::Jump(end_bb));
                ctx.start_block(else_bb);
                let def = lower_expr(ctx, dexpr, vars);
                let _ = lower_state_map_set_value(ctx, &bn, key_tmp, &spec.key, &spec.value, def);
                ctx.current_instr(Instr::Copy {
                    dest: result,
                    src: def,
                });
                ctx.finish_current(Terminator::Jump(end_bb));
                ctx.start_block(end_bb);
                return result;
            }
            let m = lower_expr(ctx, mexpr, vars);
            let sk = ctx.new_temp();
            let sv = ctx.new_temp();
            ctx.current_instr(Instr::MapLoadPair {
                dest_key: sk,
                dest_val: sv,
                map: m,
                offset: 0,
            });
            let zero = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: zero,
                value: 0,
            });
            let result = ctx.new_temp();
            let cond = lower_map_key_eq(ctx, &kexpr.ty, sk, key_tmp);
            let then_bb = ctx.new_label();
            let else_bb = ctx.new_label();
            let end_bb = ctx.new_label();
            ctx.finish_current(Terminator::Branch {
                cond,
                then_bb,
                else_bb,
            });
            ctx.start_block(then_bb);
            ctx.current_instr(Instr::Binary {
                dest: result,
                op: BinaryOp::Add,
                left: sv,
                right: zero,
            });
            ctx.finish_current(Terminator::Jump(end_bb));
            ctx.start_block(else_bb);
            let def = lower_expr(ctx, dexpr, vars);
            ctx.current_instr(Instr::MapSet {
                map: m,
                key: key_tmp,
                value: def,
            });
            ctx.current_instr(Instr::Binary {
                dest: result,
                op: BinaryOp::Add,
                left: def,
                right: zero,
            });
            ctx.finish_current(Terminator::Jump(end_bb));
            ctx.start_block(end_bb);
            result
        }
        Builtin::StateMapRemove => lower_state_map_remove_option(ctx, args, vars),
        Builtin::KeysTake2 | Builtin::ValuesTake2 => {
            let base = lower_expr(ctx, &args[0], vars);
            let start_t = lower_expr_as_int(ctx, &args[1], vars);
            let which_t = lower_expr_as_int(ctx, &args[2], vars);
            let one = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: one,
                value: 1,
            });
            let masked = ctx.new_temp();
            ctx.current_instr(Instr::Binary {
                dest: masked,
                op: BinaryOp::And,
                left: which_t,
                right: one,
            });
            let idx = ctx.new_temp();
            ctx.current_instr(Instr::Binary {
                dest: idx,
                op: BinaryOp::Add,
                left: start_t,
                right: masked,
            });
            let sixteen = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: sixteen,
                value: 16,
            });
            let bytes = ctx.new_temp();
            ctx.current_instr(Instr::Binary {
                dest: bytes,
                op: BinaryOp::Mul,
                left: idx,
                right: sixteen,
            });
            let addr = ctx.new_temp();
            ctx.current_instr(Instr::Binary {
                dest: addr,
                op: BinaryOp::Add,
                left: base,
                right: bytes,
            });
            let k = ctx.new_temp();
            let v = ctx.new_temp();
            ctx.current_instr(Instr::Load64Imm {
                dest: k,
                base: addr,
                imm: 0,
            });
            ctx.current_instr(Instr::Load64Imm {
                dest: v,
                base: addr,
                imm: 8,
            });
            if builtin == Builtin::KeysTake2 { k } else { v }
        }
        Builtin::KeysValuesTake2 => {
            let base = lower_expr(ctx, &args[0], vars);
            let start_t = lower_expr_as_int(ctx, &args[1], vars);
            let which_t = lower_expr_as_int(ctx, &args[2], vars);
            let one = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: one,
                value: 1,
            });
            let masked = ctx.new_temp();
            ctx.current_instr(Instr::Binary {
                dest: masked,
                op: BinaryOp::And,
                left: which_t,
                right: one,
            });
            let idx = ctx.new_temp();
            ctx.current_instr(Instr::Binary {
                dest: idx,
                op: BinaryOp::Add,
                left: start_t,
                right: masked,
            });
            let sixteen = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: sixteen,
                value: 16,
            });
            let bytes = ctx.new_temp();
            ctx.current_instr(Instr::Binary {
                dest: bytes,
                op: BinaryOp::Mul,
                left: idx,
                right: sixteen,
            });
            let addr = ctx.new_temp();
            ctx.current_instr(Instr::Binary {
                dest: addr,
                op: BinaryOp::Add,
                left: base,
                right: bytes,
            });
            let k = ctx.new_temp();
            let v = ctx.new_temp();
            ctx.current_instr(Instr::Load64Imm {
                dest: k,
                base: addr,
                imm: 0,
            });
            ctx.current_instr(Instr::Load64Imm {
                dest: v,
                base: addr,
                imm: 8,
            });
            let tup = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack {
                dest: tup,
                items: vec![k, v],
            });
            tup
        }
        Builtin::TestInvokeEntrypoint
        | Builtin::TestInvokeEntrypointAs
        | Builtin::TestExpectRejectAs
        | Builtin::TestActorAccount
        | Builtin::TestActorPublicKey
        | Builtin::TestActorSign => {
            unreachable!("test helpers use their dedicated lowering paths")
        }
    }
}

fn lower_expr(ctx: &mut LowerCtx, expr: &TypedExpr, vars: &mut HashMap<String, Temp>) -> Temp {
    match &expr.expr {
        semantic::ExprKind::Tuple(elems) => {
            let mut items = Vec::with_capacity(elems.len());
            for e in elems {
                items.push(lower_expr(ctx, e, vars));
            }
            let tup = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack { dest: tup, items });
            tup
        }
        semantic::ExprKind::Number(n) => {
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: *n });
            t
        }
        semantic::ExprKind::Decimal(raw) => {
            let t = ctx.new_temp();
            match raw.parse::<u128>() {
                Ok(value) => {
                    let hex = hex::encode(Numeric::new(value, 0).encode());
                    ctx.current_instr(Instr::DataRef {
                        dest: t,
                        kind: DataRefKind::NoritoBytes,
                        value: format!("0x{hex}"),
                    });
                }
                Err(_) => {
                    ctx.record_error(format!("u128 literal `{raw}` is outside 0..={}", u128::MAX));
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                }
            }
            t
        }
        semantic::ExprKind::Bool(b) => {
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: t,
                value: if *b { 1 } else { 0 },
            });
            t
        }
        semantic::ExprKind::String(s) => {
            let t = ctx.new_temp();
            ctx.current_instr(Instr::StringConst {
                dest: t,
                value: s.clone(),
            });
            t
        }
        semantic::ExprKind::Bytes(bytes) => {
            let t = ctx.new_temp();
            let hex = hex::encode(bytes);
            ctx.current_instr(Instr::DataRef {
                dest: t,
                kind: DataRefKind::Blob,
                value: format!("0x{hex}"),
            });
            t
        }
        semantic::ExprKind::Ident(name) => {
            if (ctx.state_name_literals.contains_key(name)
                || ctx.state_runtime_roots.contains_key(name))
                && !vars.contains_key(name)
                && let Some(value) = lower_state_binding_value(ctx, name, &expr.ty)
            {
                vars.insert(name.clone(), value);
                return value;
            }
            if let Some(temp) = vars.get(name) {
                *temp
            } else {
                ctx.record_error(format!("undefined variable {name}"));
                let t = ctx.new_temp();
                ctx.current_instr(Instr::Const { dest: t, value: 0 });
                t
            }
        }
        semantic::ExprKind::Unary { op, expr: inner } => {
            if matches!(op, UnaryOp::Neg)
                && matches!(semantic::resolve_struct_type(&expr.ty), Type::Int)
            {
                match crate::checked_arithmetic::evaluate_checked_i64(expr) {
                    Ok(Some(value)) => {
                        let dest = ctx.new_temp();
                        ctx.current_instr(Instr::Const { dest, value });
                        return dest;
                    }
                    Err(error) => {
                        ctx.record_error(error.to_string());
                        let dest = ctx.new_temp();
                        ctx.current_instr(Instr::Const { dest, value: 0 });
                        return dest;
                    }
                    Ok(None) => {}
                }
            }
            let v = lower_expr(ctx, inner, vars);
            if matches!(op, UnaryOp::Neg) && semantic::is_wide_numeric_type(&inner.ty) {
                let zero_int = ctx.new_temp();
                ctx.current_instr(Instr::Const {
                    dest: zero_int,
                    value: 0,
                });
                let zero_numeric = ctx.new_temp();
                ctx.current_instr(Instr::NumericFromInt {
                    dest: zero_numeric,
                    value: zero_int,
                });
                let t = ctx.new_temp();
                ctx.current_instr(Instr::NumericBinary {
                    dest: t,
                    op: BinaryOp::Sub,
                    left: zero_numeric,
                    right: v,
                });
                t
            } else {
                let t = ctx.new_temp();
                ctx.current_instr(Instr::Unary {
                    dest: t,
                    op: *op,
                    operand: v,
                });
                t
            }
        }
        semantic::ExprKind::NumericCast { expr: inner } => {
            let v = lower_expr(ctx, inner, vars);
            let src_ty = semantic::resolve_struct_type(&inner.ty);
            let dst_ty = semantic::resolve_struct_type(&expr.ty);
            if semantic::is_wide_numeric_type(&dst_ty) && matches!(src_ty, Type::Int) {
                let t = ctx.new_temp();
                ctx.current_instr(Instr::NumericFromInt { dest: t, value: v });
                return t;
            }
            if matches!(dst_ty, Type::Int) && semantic::is_wide_numeric_type(&src_ty) {
                let t = ctx.new_temp();
                ctx.current_instr(Instr::NumericToInt { dest: t, value: v });
                return t;
            }
            v
        }
        semantic::ExprKind::Binary { op, left, right } => {
            if matches!(op, BinaryOp::And | BinaryOp::Or) {
                return lower_short_circuit_bool(ctx, *op, left, right, vars);
            }
            if matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul)
                && matches!(semantic::resolve_struct_type(&expr.ty), Type::Int)
            {
                match crate::checked_arithmetic::evaluate_checked_i64(expr) {
                    Ok(Some(value)) => {
                        let dest = ctx.new_temp();
                        ctx.current_instr(Instr::Const { dest, value });
                        return dest;
                    }
                    Err(error) => {
                        ctx.record_error(error.to_string());
                        let dest = ctx.new_temp();
                        ctx.current_instr(Instr::Const { dest, value: 0 });
                        return dest;
                    }
                    Ok(None) => {}
                }
            }
            let l = lower_expr(ctx, left, vars);
            let r = lower_expr(ctx, right, vars);
            let lhs_wide = semantic::is_wide_numeric_type(&left.ty);
            let rhs_wide = semantic::is_wide_numeric_type(&right.ty);
            if matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod
            ) && (lhs_wide || rhs_wide)
            {
                let t = ctx.new_temp();
                ctx.current_instr(Instr::NumericBinary {
                    dest: t,
                    op: *op,
                    left: l,
                    right: r,
                });
                return t;
            }
            if matches!(
                op,
                BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
            ) && (lhs_wide || rhs_wide)
            {
                let t = ctx.new_temp();
                ctx.current_instr(Instr::NumericCompare {
                    dest: t,
                    op: *op,
                    left: l,
                    right: r,
                });
                return t;
            }
            if matches!(op, BinaryOp::Eq | BinaryOp::Ne)
                && is_pointer_eq_type(&left.ty)
                && is_pointer_eq_type(&right.ty)
            {
                let t = ctx.new_temp();
                ctx.current_instr(Instr::PointerEq {
                    dest: t,
                    left: l,
                    right: r,
                });
                if *op == BinaryOp::Ne {
                    let t2 = ctx.new_temp();
                    ctx.current_instr(Instr::Unary {
                        dest: t2,
                        op: UnaryOp::Not,
                        operand: t,
                    });
                    return t2;
                }
                return t;
            }
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Binary {
                dest: t,
                op: *op,
                left: l,
                right: r,
            });
            t
        }
        semantic::ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            // Evaluate condition and branch to compute the value into a shared result temp.
            let cond_t = lower_expr(ctx, cond, vars);
            let then_label = ctx.new_label();
            let else_label = ctx.new_label();
            let end_label = ctx.new_label();
            let result = ctx.new_temp();
            // Branch on condition
            ctx.finish_current(Terminator::Branch {
                cond: cond_t,
                then_bb: then_label,
                else_bb: else_label,
            });

            // Then branch: compute then value and move into result
            ctx.start_block(then_label);
            let then_v = lower_expr(ctx, then_expr, &mut vars.clone());
            let z_then = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: z_then,
                value: 0,
            });
            ctx.current_instr(Instr::Binary {
                dest: result,
                op: BinaryOp::Add,
                left: then_v,
                right: z_then,
            });
            ctx.finish_current(Terminator::Jump(end_label));

            // Else branch: compute else value and move into result
            ctx.start_block(else_label);
            let else_v = lower_expr(ctx, else_expr, &mut vars.clone());
            let z_else = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: z_else,
                value: 0,
            });
            ctx.current_instr(Instr::Binary {
                dest: result,
                op: BinaryOp::Add,
                left: else_v,
                right: z_else,
            });
            ctx.finish_current(Terminator::Jump(end_label));

            // Merge block
            ctx.start_block(end_label);
            result
        }
        semantic::ExprKind::Call { name, args } => {
            if let Some(entrypoint) = name.strip_prefix(INVOKE_ENTRYPOINT_PREFIX) {
                return lower_invoke_entrypoint_call(ctx, entrypoint, &args[0], &expr.ty, vars);
            }
            if let Some(value) = lower_sum_type_call(ctx, name, args, vars) {
                return value;
            }
            if let Some(builtin) = Builtin::from_name(name)
                && !matches!(
                    builtin,
                    Builtin::TestInvokeEntrypoint
                        | Builtin::TestInvokeEntrypointAs
                        | Builtin::TestExpectRejectAs
                        | Builtin::TestActorAccount
                        | Builtin::TestActorPublicKey
                        | Builtin::TestActorSign
                )
            {
                return lower_surface_builtin_call(ctx, builtin, args, vars);
            }
            match name.as_str() {
                "Map::new" | "map_new" => {
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::MapNew { dest: t });
                    t
                }
                "invoke_entrypoint_as" => {
                    let actor = match &args[0].expr {
                        semantic::ExprKind::String(value) => lower_blob_literal(ctx, value),
                        _ => panic!("invoke_entrypoint_as actor must be a literal string"),
                    };
                    let entrypoint = match &args[1].expr {
                        semantic::ExprKind::String(value) => lower_blob_literal(ctx, value),
                        _ => panic!("invoke_entrypoint_as entrypoint must be a literal string"),
                    };
                    let payload = lower_expr(ctx, &args[2], vars);
                    match &expr.ty {
                        semantic::Type::Unit => {
                            ctx.current_instr(Instr::InvokeEntrypointAs {
                                dest: None,
                                actor,
                                entrypoint,
                                payload,
                                returns_pointer: false,
                            });
                            let t = ctx.new_temp();
                            ctx.current_instr(Instr::Const { dest: t, value: 0 });
                            t
                        }
                        aggregate if aggregate_components(aggregate).is_some() => {
                            let items = aggregate_components(aggregate)
                                .expect("aggregate return components checked");
                            let dests: Vec<Temp> = items.iter().map(|_| ctx.new_temp()).collect();
                            let mut return_pointer_mask = 0u64;
                            for (idx, item_ty) in items.iter().enumerate() {
                                if semantic::is_pointer_type(item_ty)
                                    || semantic::is_blob_like(item_ty)
                                    || *item_ty == semantic::Type::Json
                                {
                                    return_pointer_mask |= 1u64 << idx;
                                }
                            }
                            ctx.current_instr(Instr::InvokeEntrypointAsMulti {
                                dests: dests.clone(),
                                actor,
                                entrypoint,
                                payload,
                                return_pointer_mask,
                            });
                            let tuple = ctx.new_temp();
                            ctx.current_instr(Instr::TuplePack {
                                dest: tuple,
                                items: dests,
                            });
                            tuple
                        }
                        _ => {
                            let dest = ctx.new_temp();
                            ctx.current_instr(Instr::InvokeEntrypointAs {
                                dest: Some(dest),
                                actor,
                                entrypoint,
                                payload,
                                returns_pointer: semantic::is_pointer_type(&expr.ty)
                                    || semantic::is_blob_like(&expr.ty)
                                    || expr.ty == semantic::Type::Json,
                            });
                            dest
                        }
                    }
                }
                "expect_reject_as" => {
                    let actor = match &args[0].expr {
                        semantic::ExprKind::String(value) => lower_blob_literal(ctx, value),
                        _ => panic!("expect_reject_as actor must be a literal string"),
                    };
                    let entrypoint = match &args[1].expr {
                        semantic::ExprKind::String(value) => lower_blob_literal(ctx, value),
                        _ => panic!("expect_reject_as entrypoint must be a literal string"),
                    };
                    let payload = lower_expr(ctx, &args[2], vars);
                    ctx.current_instr(Instr::ExpectRejectAs {
                        actor,
                        entrypoint,
                        payload,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "actor_account" => {
                    let actor = match &args[0].expr {
                        semantic::ExprKind::String(value) => lower_blob_literal(ctx, value),
                        _ => panic!("actor_account actor must be a literal string"),
                    };
                    let dest = ctx.new_temp();
                    ctx.current_instr(Instr::ActorAccount { dest, actor });
                    dest
                }
                "actor_public_key" => {
                    let actor = match &args[0].expr {
                        semantic::ExprKind::String(value) => lower_blob_literal(ctx, value),
                        _ => panic!("actor_public_key actor must be a literal string"),
                    };
                    let dest = ctx.new_temp();
                    ctx.current_instr(Instr::ActorPublicKey { dest, actor });
                    dest
                }
                "actor_sign" => {
                    let actor = match &args[0].expr {
                        semantic::ExprKind::String(value) => lower_blob_literal(ctx, value),
                        _ => panic!("actor_sign actor must be a literal string"),
                    };
                    let message = lower_expr(ctx, &args[1], vars);
                    let dest = ctx.new_temp();
                    ctx.current_instr(Instr::ActorSign {
                        dest,
                        actor,
                        message,
                    });
                    dest
                }
                _ => {
                    // User-defined function call: pass args; capture result(s) if any.
                    let mut arg_tmps = Vec::new();
                    let signature = ctx.function_param_specs.get(name).cloned();
                    for (idx, a) in args.iter().enumerate() {
                        if signature
                            .as_ref()
                            .and_then(|params| params.get(idx))
                            .is_some_and(|param| param.is_state)
                        {
                            let param = signature
                                .as_ref()
                                .and_then(|params| params.get(idx))
                                .expect("checked state parameter");
                            arg_tmps.extend(lower_state_handle_args(ctx, a, &param.ty, vars));
                        } else if let Some(param) =
                            signature.as_ref().and_then(|params| params.get(idx))
                            && function_value_word_types(&param.ty).is_some()
                        {
                            let value = lower_expr(ctx, a, vars);
                            collect_function_value_words(ctx, value, &param.ty, &mut arg_tmps);
                        } else {
                            arg_tmps.push(lower_expr(ctx, a, vars));
                        }
                    }
                    match &expr.ty {
                        semantic::Type::Unit => {
                            ctx.current_instr(Instr::Call {
                                callee: ctx.call_target(name),
                                args: arg_tmps,
                                dest: None,
                            });
                            let t = ctx.new_temp();
                            ctx.current_instr(Instr::Const { dest: t, value: 0 });
                            t
                        }
                        aggregate if aggregate_components(aggregate).is_some() => {
                            let ts = aggregate_components(aggregate)
                                .expect("aggregate return components checked");
                            // Multi-return: move r10.. into temps, then pack to a tuple temp
                            let mut items = Vec::with_capacity(ts.len());
                            for _ in 0..ts.len() {
                                items.push(ctx.new_temp());
                            }
                            ctx.current_instr(Instr::CallMulti {
                                callee: ctx.call_target(name),
                                args: arg_tmps,
                                dests: items.clone(),
                            });
                            let tup = ctx.new_temp();
                            ctx.current_instr(Instr::TuplePack { dest: tup, items });
                            tup
                        }
                        _ => {
                            let d = ctx.new_temp();
                            ctx.current_instr(Instr::Call {
                                callee: ctx.call_target(name),
                                args: arg_tmps,
                                dest: Some(d),
                            });
                            d
                        }
                    }
                }
            }
        }
        semantic::ExprKind::Index { .. } => {
            // Typed source lowering never contains an rvalue map index: an
            // absent durable key must be represented by Option<V>. Keep this
            // fail-closed guard for callers that construct typed HIR directly.
            ctx.record_error(
                "E_STATE_MAP_OPTIONAL_READ: StateMap rvalue indexing is invalid; use `map.get(key)`"
                    .into(),
            );
            let invalid = ctx.new_temp();
            ctx.current_instr(Instr::Const {
                dest: invalid,
                value: 0,
            });
            invalid
        }
        semantic::ExprKind::Member { object, field } => {
            // Support nested struct field access via flattened variables: base#i#j
            fn flatten_member_chain(e: &semantic::TypedExpr) -> Option<(String, Vec<usize>)> {
                match &e.expr {
                    semantic::ExprKind::Member { object, field } => {
                        let (base, mut rest) = flatten_member_chain(object)?;
                        let idx = if let Ok(i) = field.parse::<usize>() {
                            Some(i)
                        } else {
                            match crate::semantic::resolve_struct_type(&object.ty) {
                                crate::semantic::Type::Struct { fields, .. } => {
                                    fields.iter().position(|(fname, _)| fname == field)
                                }
                                crate::semantic::Type::Tuple(_) => field.parse::<usize>().ok(),
                                _ => None,
                            }
                        }?;
                        rest.push(idx);
                        Some((base, rest))
                    }
                    semantic::ExprKind::Ident(nm) => Some((nm.clone(), Vec::new())),
                    _ => None,
                }
            }
            if let Some((base, mut indices)) = flatten_member_chain(expr)
                && !indices.is_empty()
            {
                // indices are collected from inner to outer; reverse for natural order
                indices.reverse();
                let mut name = base;
                for i in indices {
                    name.push('#');
                    name.push_str(&i.to_string());
                }
                if ctx.state_name_literals.contains_key(&name) {
                    if !vars.contains_key(&name)
                        && let Some(value) = lower_state_binding_value(ctx, &name, &expr.ty)
                    {
                        vars.insert(name.clone(), value);
                        return value;
                    }
                    if let Some(t) = vars.get(&name).copied() {
                        return t;
                    }
                } else if let Some(t) = vars.get(&name).copied() {
                    return t;
                }
            }
            // Generic tuple/struct field access via TupleGet when index is numeric.
            if let Ok(idx) = field.parse::<usize>() {
                let tup = lower_expr(ctx, object, vars);
                let out = ctx.new_temp();
                ctx.current_instr(Instr::TupleGet {
                    dest: out,
                    tuple: tup,
                    index: idx,
                });
                return out;
            }
            // Named struct fields: map to tuple index using type info.
            if let crate::semantic::Type::Struct { fields, .. } =
                crate::semantic::resolve_struct_type(&object.ty)
                && let Some((idx, _)) = fields
                    .iter()
                    .enumerate()
                    .find(|(_, (fname, _))| fname == field)
            {
                let tup = lower_expr(ctx, object, vars);
                let out = ctx.new_temp();
                ctx.current_instr(Instr::TupleGet {
                    dest: out,
                    tuple: tup,
                    index: idx,
                });
                return out;
            }
            // Fallback to zero for durable types without decode support yet; tracked under
            // the kotodama-state backlog to add composite aggregate decoders.
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
    }
}

fn lower_sum_type_call(
    ctx: &mut LowerCtx,
    name: &str,
    args: &[semantic::TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Option<Temp> {
    let tagged = |ctx: &mut LowerCtx, tag: i64, values: Vec<Temp>| {
        let tag_temp = ctx.new_temp();
        ctx.current_instr(Instr::Const {
            dest: tag_temp,
            value: tag,
        });
        let mut items = Vec::with_capacity(values.len() + 1);
        items.push(tag_temp);
        items.extend(values);
        let result = ctx.new_temp();
        ctx.current_instr(Instr::TuplePack {
            dest: result,
            items,
        });
        result
    };

    Some(match name {
        "option_some" => {
            let value = lower_expr(ctx, &args[0], vars);
            tagged(ctx, 1, vec![value])
        }
        "option_none" => {
            let placeholder = lower_expr(ctx, &args[0], vars);
            tagged(ctx, 0, vec![placeholder])
        }
        "result_ok" => {
            let value = lower_expr(ctx, &args[0], vars);
            let error_placeholder = lower_expr(ctx, &args[1], vars);
            tagged(ctx, 1, vec![value, error_placeholder])
        }
        "result_err" => {
            let value_placeholder = lower_expr(ctx, &args[0], vars);
            let error = lower_expr(ctx, &args[1], vars);
            tagged(ctx, 0, vec![value_placeholder, error])
        }
        STATE_MAP_GET_INTRINSIC => lower_state_map_get_option(ctx, args, vars),
        "is_some" | "is_ok" => {
            let tagged_value = lower_expr(ctx, &args[0], vars);
            let tag = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: tag,
                tuple: tagged_value,
                index: 0,
            });
            tag
        }
        "is_none" | "is_err" => {
            let tagged_value = lower_expr(ctx, &args[0], vars);
            let tag = ctx.new_temp();
            ctx.current_instr(Instr::TupleGet {
                dest: tag,
                tuple: tagged_value,
                index: 0,
            });
            let inverted = ctx.new_temp();
            ctx.current_instr(Instr::Unary {
                dest: inverted,
                op: UnaryOp::Not,
                operand: tag,
            });
            inverted
        }
        "unwrap_or" => lower_tagged_unwrap(ctx, &args[0], &args[1], 1, true, vars),
        "unwrap_err_or" => lower_tagged_unwrap(ctx, &args[0], &args[1], 2, false, vars),
        _ => return None,
    })
}

fn lower_tagged_unwrap(
    ctx: &mut LowerCtx,
    tagged_expr: &semantic::TypedExpr,
    fallback_expr: &semantic::TypedExpr,
    payload_index: usize,
    payload_when_tagged: bool,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let tagged = lower_expr(ctx, tagged_expr, vars);
    let tag = ctx.new_temp();
    ctx.current_instr(Instr::TupleGet {
        dest: tag,
        tuple: tagged,
        index: 0,
    });
    let payload = ctx.new_temp();
    ctx.current_instr(Instr::TupleGet {
        dest: payload,
        tuple: tagged,
        index: payload_index,
    });
    let payload_block = ctx.new_label();
    let fallback_block = ctx.new_label();
    let end_block = ctx.new_label();
    let (then_bb, else_bb) = if payload_when_tagged {
        (payload_block, fallback_block)
    } else {
        (fallback_block, payload_block)
    };

    let payload_ty = semantic::resolve_struct_type(&fallback_expr.ty);
    if is_aggregate_state_value_type(&payload_ty) {
        let Some(word_count) = state_value_schema(&payload_ty)
            .and_then(|schema| schema.word_kinds())
            .map(|words| words.len())
        else {
            ctx.record_error("aggregate sum payload is not lowerable".into());
            return payload;
        };
        let result_words = (0..word_count).map(|_| ctx.new_temp()).collect::<Vec<_>>();
        ctx.finish_current(Terminator::Branch {
            cond: tag,
            then_bb,
            else_bb,
        });

        ctx.start_block(payload_block);
        let mut payload_words = Vec::with_capacity(word_count);
        if !collect_state_value_words(ctx, payload, &payload_ty, &mut payload_words)
            || payload_words.len() != word_count
        {
            ctx.record_error("aggregate sum payload has an invalid runtime shape".into());
        }
        for (dest, src) in result_words.iter().zip(payload_words) {
            ctx.current_instr(Instr::Copy { dest: *dest, src });
        }
        ctx.finish_current(Terminator::Jump(end_block));

        ctx.start_block(fallback_block);
        let fallback = lower_expr(ctx, fallback_expr, vars);
        let mut fallback_words = Vec::with_capacity(word_count);
        if !collect_state_value_words(ctx, fallback, &payload_ty, &mut fallback_words)
            || fallback_words.len() != word_count
        {
            ctx.record_error("aggregate fallback has an invalid runtime shape".into());
        }
        for (dest, src) in result_words.iter().zip(fallback_words) {
            ctx.current_instr(Instr::Copy { dest: *dest, src });
        }
        ctx.finish_current(Terminator::Jump(end_block));

        ctx.start_block(end_block);
        let mut index = 0;
        let result = rebuild_state_value_from_words(ctx, &payload_ty, &result_words, &mut index)
            .unwrap_or_else(|| {
                ctx.record_error("aggregate sum result cannot be reconstructed".into());
                payload
            });
        if index != word_count {
            ctx.record_error("aggregate sum result word count mismatch".into());
        }
        return result;
    }

    ctx.finish_current(Terminator::Branch {
        cond: tag,
        then_bb,
        else_bb,
    });

    let result = ctx.new_temp();
    ctx.start_block(payload_block);
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: payload,
    });
    ctx.finish_current(Terminator::Jump(end_block));

    ctx.start_block(fallback_block);
    let fallback = lower_expr(ctx, fallback_expr, vars);
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: fallback,
    });
    ctx.finish_current(Terminator::Jump(end_block));
    ctx.start_block(end_block);
    result
}

/// Decode a present durable value and merge it with the unique all-zero/null
/// placeholder used by the inactive arm of `Option`.  The host codec rejects a
/// zero record pointer, so the presence branch is semantically significant: a
/// missing map key must never be mistaken for an initialized aggregate value.
fn lower_present_or_inactive_state_value(
    ctx: &mut LowerCtx,
    blob: Temp,
    present: Temp,
    value_ty: &Type,
    delete_when_present: Option<Temp>,
) -> Option<Temp> {
    let resolved = semantic::resolve_struct_type(value_ty);
    let word_count = state_value_schema(&resolved)?.word_kinds()?.len();
    let result_words = (0..word_count).map(|_| ctx.new_temp()).collect::<Vec<_>>();
    let present_block = ctx.new_label();
    let absent_block = ctx.new_label();
    let end_block = ctx.new_label();
    ctx.finish_current(Terminator::Branch {
        cond: present,
        then_bb: present_block,
        else_bb: absent_block,
    });

    ctx.start_block(present_block);
    let mut decoded_words = Vec::with_capacity(word_count);
    if let Some(decoded) = decode_aggregate_state_value(ctx, blob, &resolved) {
        if !collect_state_value_words(ctx, decoded, &resolved, &mut decoded_words)
            || decoded_words.len() != word_count
        {
            ctx.record_error("durable state value has an invalid runtime shape".into());
            decoded_words.clear();
        }
    } else {
        ctx.record_error("durable state value is not decodable".into());
    }
    if decoded_words.len() != word_count {
        let zero = ctx.new_temp();
        ctx.current_instr(Instr::Const {
            dest: zero,
            value: 0,
        });
        decoded_words.resize(word_count, zero);
    }
    for (dest, src) in result_words.iter().zip(decoded_words) {
        ctx.current_instr(Instr::Copy { dest: *dest, src });
    }
    if let Some(path) = delete_when_present {
        ctx.current_instr(Instr::StateDel { path });
    }
    ctx.finish_current(Terminator::Jump(end_block));

    ctx.start_block(absent_block);
    let zero = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: zero,
        value: 0,
    });
    for dest in &result_words {
        ctx.current_instr(Instr::Copy {
            dest: *dest,
            src: zero,
        });
    }
    ctx.finish_current(Terminator::Jump(end_block));

    ctx.start_block(end_block);
    let mut index = 0;
    let value = rebuild_state_value_from_words(ctx, &resolved, &result_words, &mut index)?;
    (index == word_count).then_some(value)
}

fn lower_state_map_get_option(
    ctx: &mut LowerCtx,
    args: &[semantic::TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let map = &args[0];
    let key = lower_expr(ctx, &args[1], vars);
    let Some(base) = state_map_base_name(map) else {
        ctx.record_error("StateMap.get receiver is not a durable state map".into());
        return lower_absent_option(ctx);
    };
    let Some(spec) = ctx.state_map_configs.get(&base).cloned() else {
        ctx.record_error(format!("StateMap.get receiver `{base}` is not declared"));
        return lower_absent_option(ctx);
    };
    let Some(key_codec) = key_codec_for_type(&spec.key) else {
        ctx.record_error("StateMap.get key type is not lowerable".into());
        return lower_absent_option(ctx);
    };
    let path = build_state_path(ctx, &base, key, &key_codec);
    let blob = ctx.new_temp();
    ctx.current_instr(Instr::StateGet { dest: blob, path });
    let zero = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: zero,
        value: 0,
    });
    let present = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: present,
        op: BinaryOp::Ne,
        left: blob,
        right: zero,
    });

    let value = lower_present_or_inactive_state_value(ctx, blob, present, &spec.value, None)
        .unwrap_or_else(|| {
            ctx.record_error("StateMap.get value type is not lowerable".into());
            zero
        });

    let option = ctx.new_temp();
    ctx.current_instr(Instr::TuplePack {
        dest: option,
        items: vec![present, value],
    });
    option
}

fn lower_state_map_remove_option(
    ctx: &mut LowerCtx,
    args: &[semantic::TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let map = &args[0];
    let key = lower_expr(ctx, &args[1], vars);
    let Some(base) = state_map_base_name(map) else {
        ctx.record_error("StateMap.remove receiver is not a durable state map".into());
        return lower_absent_option(ctx);
    };
    let Some(spec) = ctx.state_map_configs.get(&base).cloned() else {
        ctx.record_error(format!("StateMap.remove receiver `{base}` is not declared"));
        return lower_absent_option(ctx);
    };
    let Some(key_codec) = key_codec_for_type(&spec.key) else {
        ctx.record_error("StateMap.remove key type is not lowerable".into());
        return lower_absent_option(ctx);
    };
    let path = build_state_path(ctx, &base, key, &key_codec);
    let blob = ctx.new_temp();
    ctx.current_instr(Instr::StateGet { dest: blob, path });
    let zero = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: zero,
        value: 0,
    });
    let present = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: present,
        op: BinaryOp::Ne,
        left: blob,
        right: zero,
    });

    let value = lower_present_or_inactive_state_value(ctx, blob, present, &spec.value, Some(path))
        .unwrap_or_else(|| {
            ctx.record_error("StateMap.remove value type is not lowerable".into());
            zero
        });
    let option = ctx.new_temp();
    ctx.current_instr(Instr::TuplePack {
        dest: option,
        items: vec![present, value],
    });
    option
}

fn lower_absent_option(ctx: &mut LowerCtx) -> Temp {
    let zero = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: zero,
        value: 0,
    });
    let option = ctx.new_temp();
    ctx.current_instr(Instr::TuplePack {
        dest: option,
        items: vec![zero, zero],
    });
    option
}

fn account_id_literal_uses_alias_resolution(raw: &str) -> bool {
    if iroha_data_model::account::AccountId::parse_encoded(raw).is_ok() {
        return false;
    }

    // Keep alias detection intentionally broad so alias-shaped literals continue into the
    // runtime host resolver, which validates the current catalog/binding instead of failing
    // during static AccountId encoding.
    raw.contains('@')
}

fn lower_state_binding_value(ctx: &mut LowerCtx, name: &str, ty: &Type) -> Option<Temp> {
    let resolved = semantic::resolve_struct_type(ty);
    let blob = state_get_blob_for_name(ctx, name);
    if !is_canonical_state_value_type(&resolved) {
        return None;
    }
    decode_aggregate_state_value(ctx, blob, &resolved)
}

fn state_get_blob_for_name(ctx: &mut LowerCtx, name: &str) -> Temp {
    let path = build_state_base(ctx, name);
    let blob = ctx.new_temp();
    ctx.current_instr(Instr::StateGet { dest: blob, path });
    blob
}

struct LoopContext {
    continue_label: Label,
    break_label: Label,
    phi: Option<HashMap<String, Temp>>,
}

struct LowerCtx {
    next_temp: usize,
    next_label: usize,
    blocks: Vec<BasicBlock>,
    current: Option<BasicBlock>,
    loop_stack: Vec<LoopContext>,
    /// Metadata for top-level state maps lowered to durable state syscalls.
    state_map_configs: HashMap<String, StateMapSpec>,
    /// Mapping from top-level state identifiers to Name literals used in TLVs.
    state_name_literals: HashMap<String, String>,
    /// Runtime Name roots for `state` helper parameters.
    state_runtime_roots: HashMap<String, Temp>,
    /// Dynamic iteration cap for first-release dynamic bounds.
    _dyn_iter_cap: usize,
    call_renames: HashMap<String, String>,
    function_param_specs: HashMap<String, Vec<TypedParam>>,
    error: Option<String>,
}

impl LowerCtx {
    fn new(
        dyn_iter_cap: usize,
        call_renames: HashMap<String, String>,
        function_param_specs: HashMap<String, Vec<TypedParam>>,
    ) -> Self {
        Self {
            next_temp: 0,
            next_label: 0,
            blocks: Vec::new(),
            current: None,
            loop_stack: Vec::new(),
            state_map_configs: Default::default(),
            state_name_literals: Default::default(),
            state_runtime_roots: Default::default(),
            _dyn_iter_cap: dyn_iter_cap,
            call_renames,
            function_param_specs,
            error: None,
        }
    }

    fn new_temp(&mut self) -> Temp {
        let t = Temp(self.next_temp);
        self.next_temp += 1;
        t
    }

    fn new_label(&mut self) -> Label {
        let l = Label(self.next_label);
        self.next_label += 1;
        l
    }

    fn start_block(&mut self, label: Label) {
        if self.current.is_some() {
            self.record_error("internal error: current block not finished".to_string());
            self.current = None;
        }
        self.current = Some(BasicBlock {
            label,
            instrs: Vec::new(),
            terminator: Terminator::Jump(label),
        });
    }

    fn current_instr(&mut self, instr: Instr) {
        if let Some(ref mut bb) = self.current {
            bb.instrs.push(instr);
        }
    }

    fn finish_current(&mut self, term: Terminator) {
        let Some(mut bb) = self.current.take() else {
            self.record_error("internal error: no current block".to_string());
            return;
        };
        bb.terminator = term;
        self.blocks.push(bb);
    }

    fn push_loop(&mut self, cont: Label, brk: Label) {
        self.loop_stack.push(LoopContext {
            continue_label: cont,
            break_label: brk,
            phi: None,
        });
    }

    fn pop_loop(&mut self) {
        self.loop_stack.pop();
    }

    fn record_error(&mut self, message: String) {
        if self.error.is_none() {
            self.error = Some(message);
        }
    }

    fn call_target(&self, name: &str) -> String {
        self.call_renames
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    fn loop_targets(&self) -> Option<(Label, Label)> {
        self.loop_stack
            .last()
            .map(|ctx| (ctx.continue_label, ctx.break_label))
    }

    fn set_loop_phi(&mut self, phi: HashMap<String, Temp>) {
        if let Some(ctx) = self.loop_stack.last_mut() {
            ctx.phi = Some(phi);
        }
    }

    fn current_loop_phi(&self) -> Option<&HashMap<String, Temp>> {
        self.loop_stack.last().and_then(|ctx| ctx.phi.as_ref())
    }
}

fn build_state_name_literal(ctx: &mut LowerCtx, name: &str) -> Temp {
    let t_base = ctx.new_temp();
    let literal = ctx
        .state_name_literals
        .get(name)
        .cloned()
        .unwrap_or_else(|| name.to_string());
    ctx.current_instr(Instr::DataRef {
        dest: t_base,
        kind: DataRefKind::Name,
        value: literal,
    });
    t_base
}

fn build_state_base(ctx: &mut LowerCtx, name: &str) -> Temp {
    if let Some(temp) = ctx.state_runtime_roots.get(name).copied() {
        temp
    } else {
        build_state_name_literal(ctx, name)
    }
}

fn build_state_path(ctx: &mut LowerCtx, name: &str, key: Temp, key_codec: &KeyCodec) -> Temp {
    let t_base = build_state_base(ctx, name);
    match key_codec {
        KeyCodec::Int => {
            let key_blob = ctx.new_temp();
            ctx.current_instr(Instr::EncodeInt {
                dest: key_blob,
                value: key,
            });
            let t_path = ctx.new_temp();
            ctx.current_instr(Instr::PathMapKeyNorito {
                dest: t_path,
                base: t_base,
                key_blob,
            });
            t_path
        }
        KeyCodec::Pointer => {
            let key_blob = ctx.new_temp();
            ctx.current_instr(Instr::PointerToNorito {
                dest: key_blob,
                value: key,
            });
            let t_path = ctx.new_temp();
            ctx.current_instr(Instr::PathMapKeyNorito {
                dest: t_path,
                base: t_base,
                key_blob,
            });
            t_path
        }
        KeyCodec::NoritoBytes => {
            let t_path = ctx.new_temp();
            ctx.current_instr(Instr::PathMapKeyNorito {
                dest: t_path,
                base: t_base,
                key_blob: key,
            });
            t_path
        }
    }
}

fn lowerable_state_handle_name(ctx: &LowerCtx, expr: &semantic::TypedExpr) -> Option<String> {
    match &expr.expr {
        semantic::ExprKind::Ident(name) => {
            if ctx.state_runtime_roots.contains_key(name)
                || ctx.state_name_literals.contains_key(name)
            {
                Some(name.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

fn lower_short_circuit_bool(
    ctx: &mut LowerCtx,
    op: BinaryOp,
    left: &semantic::TypedExpr,
    right: &semantic::TypedExpr,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    debug_assert!(matches!(op, BinaryOp::And | BinaryOp::Or));

    let left_value = lower_expr(ctx, left, vars);
    let rhs_label = ctx.new_label();
    let short_label = ctx.new_label();
    let join_label = ctx.new_label();
    let result = ctx.new_temp();
    let (then_bb, else_bb, short_value) = match op {
        BinaryOp::And => (rhs_label, short_label, 0),
        BinaryOp::Or => (short_label, rhs_label, 1),
        _ => unreachable!("short-circuit lowering only accepts logical operators"),
    };

    ctx.finish_current(Terminator::Branch {
        cond: left_value,
        then_bb,
        else_bb,
    });

    // Materialize the short-circuit result without evaluating the right-hand side.
    // Both predecessors use `Copy` so compiler fact propagation treats `result` as
    // a genuine control-flow merge rather than a block-local constant.
    ctx.start_block(short_label);
    let short_result = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: short_result,
        value: short_value,
    });
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: short_result,
    });
    ctx.finish_current(Terminator::Jump(join_label));

    ctx.start_block(rhs_label);
    let right_value = lower_expr(ctx, right, &mut vars.clone());
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: right_value,
    });
    ctx.finish_current(Terminator::Jump(join_label));

    ctx.start_block(join_label);
    result
}

fn lower_state_handle_arg(
    ctx: &mut LowerCtx,
    expr: &semantic::TypedExpr,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    if let Some(base_name) = lowerable_state_handle_name(ctx, expr) {
        build_state_base(ctx, &base_name)
    } else {
        ctx.record_error("state parameter arguments must reference a durable state handle".into());
        let t = ctx.new_temp();
        ctx.current_instr(Instr::Const { dest: t, value: 0 });
        // Use vars to keep parity with the regular lower_expr signature.
        let _ = vars;
        t
    }
}

fn lower_state_handle_args(
    ctx: &mut LowerCtx,
    expr: &semantic::TypedExpr,
    param_ty: &Type,
    vars: &mut HashMap<String, Temp>,
) -> Vec<Temp> {
    let Some(base_name) = lowerable_state_handle_name(ctx, expr) else {
        return vec![lower_state_handle_arg(ctx, expr, vars)];
    };
    let mut handles = Vec::new();
    collect_state_handle_specs(&base_name, param_ty, &mut handles);
    handles
        .into_iter()
        .map(|(name, _)| build_state_base(ctx, &name))
        .collect()
}

fn emit_state_set(ctx: &mut LowerCtx, name: &str, ty: &Type, value: Temp) {
    let resolved = semantic::resolve_struct_type(ty);
    let Some(encoded) = encode_aggregate_state_value(ctx, value, &resolved) else {
        ctx.record_error("durable state value is not encodable".into());
        return;
    };
    let path = build_state_base(ctx, name);
    ctx.current_instr(Instr::StateSet {
        path,
        value: encoded,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parser::parse_test_fragment as parse, semantic::analyze};

    #[test]
    fn direct_syscall_lowering_is_exhaustively_registry_driven_and_fail_closed() {
        for &builtin in Builtin::ALL {
            let spec = builtin.spec();
            let selected = std::panic::catch_unwind(|| direct_builtin_syscall(builtin));
            match spec.lowering {
                BuiltinLowering::DirectSyscall => {
                    assert_eq!(
                        Some(selected.expect("direct builtin must select a syscall")),
                        spec.syscall
                    );
                }
                BuiltinLowering::Instructions | BuiltinLowering::DerivedSyscalls => {
                    assert!(
                        selected.is_err(),
                        "non-direct builtin {} selected an undeclared direct syscall",
                        spec.name
                    );
                }
            }
        }
    }

    #[test]
    fn lower_simple_function() {
        let src = "fn add(a: i64, b: i64) { let c = a + b; }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        assert_eq!(ir.functions.len(), 1);
        let f = &ir.functions[0];
        assert_eq!(f.blocks.len(), 1); // only entry block
    }

    #[test]
    fn require_lowers_declared_error_code_into_abort_ir() {
        let src = r#"
            error enum PaymentError { Unauthorized = 1001 }
            fn authorize_payment(allowed: bool) {
                require(allowed, PaymentError::Unauthorized);
            }
        "#;
        let prog = parse(src).expect("parse error enum");
        let typed = analyze(&prog).expect("analyze stable require");
        let ir = lower(&typed).expect("lower stable require");
        let function = &ir.functions[0];
        let mut constants = HashMap::new();
        let mut abort_code = None;
        for block in &function.blocks {
            for instruction in &block.instrs {
                match instruction {
                    Instr::Const { dest, value } => {
                        constants.insert(*dest, *value);
                    }
                    Instr::AbortIf { code, .. } => abort_code = Some(*code),
                    _ => {}
                }
            }
        }
        let abort_code = abort_code.expect("require must lower to AbortIf");
        assert_eq!(constants.get(&abort_code), Some(&1001));
    }

    #[test]
    fn test_mode_entrypoint_wrapper_checks_override_state_first() {
        let src = r#"
            seiyaku Demo {
                kotoage fn run(count: i64) -> i64 authorize("Entry") { return count; }
            }
        "#;
        let prog = parse(src).expect("parse wrapper test");
        let typed = analyze(&prog).expect("analyze wrapper test");
        let ir = lower_with_cap_and_test_mode(&typed, 2, true).expect("lower wrapper test");
        let wrapper = ir
            .functions
            .iter()
            .find(|function| function.name == "run")
            .expect("wrapper function");

        let mut saw_override_key = false;
        let mut saw_state_get = false;
        let mut saw_get_trigger = false;
        for block in &wrapper.blocks {
            for instr in &block.instrs {
                match instr {
                    Instr::DataRef {
                        kind: DataRefKind::Name,
                        value,
                        ..
                    } if value == TEST_TRIGGER_EVENT_OVERRIDE_KEY => saw_override_key = true,
                    Instr::StateGet { .. } => saw_state_get = true,
                    Instr::GetTriggerEvent { .. } => saw_get_trigger = true,
                    _ => {}
                }
            }
        }

        assert!(
            saw_override_key,
            "wrapper should materialize the override key"
        );
        assert!(saw_state_get, "wrapper should read the test override slot");
        assert!(
            saw_get_trigger,
            "wrapper should still fall back to host trigger input"
        );
    }

    #[test]
    fn single_json_entrypoint_uses_the_same_one_shot_argument_record() {
        let src = r#"
            seiyaku Demo {
                kotoage fn run(ev: Json) authorize("Entry") { let _payload = ev; }
            }
        "#;
        let prog = parse(src).expect("parse single json entrypoint");
        let typed = analyze(&prog).expect("analyze single json entrypoint");
        let ir =
            lower_with_cap_and_test_mode(&typed, 2, false).expect("lower single json entrypoint");
        let wrapper = ir
            .functions
            .iter()
            .find(|function| function.name == "run")
            .expect("wrapper function");

        let mut saw_get_trigger = false;
        let mut record_decodes = 0;
        let mut json_field_getters = 0;
        for block in &wrapper.blocks {
            for instr in &block.instrs {
                match instr {
                    Instr::GetTriggerEvent { .. } => saw_get_trigger = true,
                    Instr::DirectHelperSyscall { syscall, .. }
                        if *syscall == ivm_abi::syscalls::SYSCALL_DECODE_ARGUMENT_RECORD =>
                    {
                        record_decodes += 1;
                    }
                    Instr::JsonGetJson { .. } => json_field_getters += 1,
                    _ => {}
                }
            }
        }

        assert!(saw_get_trigger, "wrapper should load the trigger payload");
        assert_eq!(record_decodes, 1, "Json must use the canonical record ABI");
        assert_eq!(
            json_field_getters, 0,
            "the wrapper must not decode the transport JSON per parameter"
        );
    }

    #[test]
    fn public_wrapper_decodes_one_complete_norito_argument_record() {
        let src = r#"
            seiyaku Demo {
                kotoage fn run(
                    count: i64,
                    total: u128,
                    ready: bool,
                    text: string,
                    label: Name,
                    asset: AssetId,
                    domain: DomainId,
                    dataspace: DataSpaceId,
                    bytes: bytes
                ) authorize("Entry") {
                    let _count = count;
                    let _total = total;
                    let _ready = ready;
                    let _text = text;
                    let _label = label;
                    let _asset = asset;
                    let _domain = domain;
                    let _dataspace = dataspace;
                    let _bytes = bytes;
                }
            }
        "#;
        let prog = parse(src).expect("parse parameterized entrypoint");
        let typed = analyze(&prog).expect("analyze parameterized entrypoint");
        let ir = lower(&typed).expect("lower parameterized entrypoint");
        let wrapper = ir
            .functions
            .iter()
            .find(|function| function.name == "run")
            .expect("wrapper function");

        let mut record_decodes = 0;
        let mut table_loads = 0;
        let mut json_field_getters = 0;
        let mut decoded_schema = None;
        for block in &wrapper.blocks {
            for instr in &block.instrs {
                match instr {
                    Instr::DirectHelperSyscall { syscall, .. }
                        if *syscall == ivm_abi::syscalls::SYSCALL_DECODE_ARGUMENT_RECORD =>
                    {
                        record_decodes += 1;
                    }
                    Instr::Load64Imm { .. } => table_loads += 1,
                    Instr::JsonGetInt { .. }
                    | Instr::JsonGetNumeric { .. }
                    | Instr::JsonGetJson { .. }
                    | Instr::JsonGetName { .. }
                    | Instr::JsonGetAccountId { .. }
                    | Instr::JsonGetAssetDefinitionId { .. }
                    | Instr::JsonGetNftId { .. }
                    | Instr::JsonGetBlobHex { .. } => json_field_getters += 1,
                    Instr::DataRef {
                        kind: DataRefKind::NoritoBytes,
                        value,
                        ..
                    } => {
                        let bytes = hex::decode(value.strip_prefix("0x").expect("hex schema"))
                            .expect("decode schema hex");
                        decoded_schema = Some(
                            norito::decode_from_bytes::<
                                ivm_abi::entrypoint::EntrypointArgumentSchemaV1,
                            >(&bytes)
                            .expect("decode argument schema"),
                        );
                    }
                    _ => {}
                }
            }
        }

        assert_eq!(record_decodes, 1, "wrapper must decode the payload once");
        assert_eq!(table_loads, 9, "one fixed table load per parameter");
        assert_eq!(
            json_field_getters, 0,
            "wrapper must not re-decode JSON per field"
        );
        let schema = decoded_schema.expect("compiler-emitted Norito schema");
        assert_eq!(
            schema
                .fields
                .iter()
                .map(|field| {
                    let [ivm_abi::entrypoint::EntrypointArgumentTypeNodeV1::Leaf(kind)] =
                        field.ty.nodes.as_slice()
                    else {
                        panic!("scalar test parameter must use one leaf node");
                    };
                    (&*field.name, *kind)
                })
                .collect::<Vec<_>>(),
            vec![
                ("count", ivm_abi::entrypoint::EntrypointArgumentKindV1::Int),
                ("total", ivm_abi::entrypoint::EntrypointArgumentKindV1::U128),
                ("ready", ivm_abi::entrypoint::EntrypointArgumentKindV1::Bool),
                (
                    "text",
                    ivm_abi::entrypoint::EntrypointArgumentKindV1::String
                ),
                ("label", ivm_abi::entrypoint::EntrypointArgumentKindV1::Name),
                (
                    "asset",
                    ivm_abi::entrypoint::EntrypointArgumentKindV1::AssetId
                ),
                (
                    "domain",
                    ivm_abi::entrypoint::EntrypointArgumentKindV1::DomainId
                ),
                (
                    "dataspace",
                    ivm_abi::entrypoint::EntrypointArgumentKindV1::DataSpaceId
                ),
                ("bytes", ivm_abi::entrypoint::EntrypointArgumentKindV1::Blob),
            ]
        );
    }

    #[test]
    fn public_aggregate_arguments_cross_internal_calls_as_flat_words() {
        let src = r#"
            seiyaku Demo {
                struct Request { count: i64, ready: bool }

                view fn run(
                    request: Request,
                    pair: (i64, bool),
                    maybe: Option<i64>,
                    outcome: Result<i64, bool>
                ) -> i64 {
                    return request.count + pair.0
                        + maybe.unwrap_or(0) + outcome.unwrap_or(0);
                }
            }
        "#;
        let prog = parse(src).expect("parse aggregate entrypoint");
        let typed = analyze(&prog).expect("analyze aggregate entrypoint");
        let ir = lower(&typed).expect("lower aggregate entrypoint");
        let wrapper = ir
            .functions
            .iter()
            .find(|function| function.name == "run")
            .expect("wrapper function");
        let implementation = ir
            .functions
            .iter()
            .find(|function| function.name == "__entrypoint_impl__run")
            .expect("implementation function");

        let call_args = wrapper
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .find_map(|instr| match instr {
                Instr::Call { callee, args, .. } if callee == "__entrypoint_impl__run" => {
                    Some(args)
                }
                _ => None,
            })
            .expect("wrapper implementation call");
        assert_eq!(
            call_args.len(),
            9,
            "all recursive ABI words must cross the call"
        );
        assert_eq!(implementation.params.len(), 9);
        assert!(
            implementation
                .params
                .iter()
                .all(|name| name.starts_with("$abi$")),
            "aggregate implementation parameters must use collision-proof compiler names"
        );
        assert_eq!(
            implementation
                .blocks
                .iter()
                .flat_map(|block| &block.instrs)
                .filter(|instr| matches!(instr, Instr::TuplePack { .. }))
                .count(),
            4,
            "struct, tuple, Option, and Result shapes must be rebuilt in the callee"
        );
    }

    #[test]
    fn aggregate_argument_words_over_register_window_fail_during_lowering() {
        let src = r#"
            seiyaku WideCall {
                struct Wide {
                    f00: i64, f01: i64, f02: i64, f03: i64, f04: i64,
                    f05: i64, f06: i64, f07: i64, f08: i64, f09: i64,
                    f10: i64, f11: i64, f12: i64, f13: i64
                }
                view fn inspect(value: Wide) -> i64 { return value.f00; }
            }
        "#;
        let prog = parse(src).expect("parse oversized aggregate call");
        let typed = analyze(&prog).expect("analyze oversized aggregate call");
        let failure = lower(&typed).expect_err("lowering must reject an oversized call ABI");
        assert!(
            failure.contains(
                "requires 14 flattened argument words, exceeding the Kotodama V1 limit of 13"
            ),
            "unexpected lowering failure: {failure}"
        );
    }

    #[test]
    fn invoke_entrypoint_lowers_to_wrapper_call_with_override_restore() {
        let src = r#"
            seiyaku Demo {
                kotoage fn run(count: i64) -> i64 authorize("Entry") { return count + 1; }

                #[test]
                fn drive_run() {
                    let next = test::invoke_entrypoint("run", Json::parse("{\"count\": 7}"));
                    test::assert_eq(next, 8);
                }
            }
        "#;
        let prog = parse(src).expect("parse invoke_entrypoint");
        let typed = semantic::SemanticContext::with_capabilities(false, true)
            .analyze(&prog)
            .expect("analyze invoke_entrypoint");
        let ir = lower_with_cap_and_test_mode(&typed, 2, true).expect("lower invoke_entrypoint");
        let test_fn = ir
            .functions
            .iter()
            .find(|function| function.name == "drive_run")
            .expect("test function");

        let mut saw_wrapper_call = false;
        let mut saw_impl_call = false;
        let mut saw_override_get = false;
        let mut saw_override_set = false;
        let mut saw_override_clear = false;
        for block in &test_fn.blocks {
            for instr in &block.instrs {
                match instr {
                    Instr::Call { callee, .. } if callee == "run" => saw_wrapper_call = true,
                    Instr::Call { callee, .. } if callee == "__entrypoint_impl__run" => {
                        saw_impl_call = true
                    }
                    Instr::StateGet { .. } => saw_override_get = true,
                    Instr::StateSet { .. } => saw_override_set = true,
                    Instr::StateDel { .. } => saw_override_clear = true,
                    _ => {}
                }
            }
        }

        assert!(
            saw_wrapper_call,
            "invoke_entrypoint should call the public wrapper"
        );
        assert!(
            !saw_impl_call,
            "invoke_entrypoint must not bypass the entrypoint wrapper"
        );
        assert!(
            saw_override_get,
            "invoke_entrypoint should snapshot any previous override"
        );
        assert!(
            saw_override_set,
            "invoke_entrypoint should install a trigger override"
        );
        assert!(
            saw_override_clear,
            "invoke_entrypoint should clear the override when none existed"
        );
    }

    #[test]
    fn invoke_entrypoint_tuple_return_uses_wrapper_callmulti() {
        let src = r#"
            seiyaku Demo {
                kotoage fn run(count: i64) -> (i64, i64) authorize("Entry") { return (count, count + 1); }

                #[test]
                fn drive_run() {
                    let pair = test::invoke_entrypoint("run", Json::parse("{\"count\": 7}"));
                    test::assert_eq(pair.0, 7);
                    test::assert_eq(pair.1, 8);
                }
            }
        "#;
        let prog = parse(src).expect("parse tuple invoke_entrypoint");
        let typed = semantic::SemanticContext::with_capabilities(false, true)
            .analyze(&prog)
            .expect("analyze tuple invoke_entrypoint");
        let ir =
            lower_with_cap_and_test_mode(&typed, 2, true).expect("lower tuple invoke_entrypoint");
        let test_fn = ir
            .functions
            .iter()
            .find(|function| function.name == "drive_run")
            .expect("test function");

        let mut saw_wrapper_callmulti = false;
        let mut saw_tuple_pack = false;
        for block in &test_fn.blocks {
            for instr in &block.instrs {
                match instr {
                    Instr::CallMulti { callee, .. } if callee == "run" => {
                        saw_wrapper_callmulti = true
                    }
                    Instr::TuplePack { .. } => saw_tuple_pack = true,
                    _ => {}
                }
            }
        }

        assert!(
            saw_wrapper_callmulti,
            "tuple invoke_entrypoint should call the public wrapper via CallMulti"
        );
        assert!(
            saw_tuple_pack,
            "tuple invoke_entrypoint should pack multi-return values"
        );
    }

    #[test]
    fn invoke_entrypoint_as_lowers_to_test_host_intrinsics() {
        let src = r#"
            seiyaku Demo {
                kotoage fn run(count: i64) -> i64 authorize("Entry") { return count + 1; }

                #[test]
                fn drive_run() {
                    let next = test::invoke_entrypoint_as("issuer", "run", Json::parse("{\"count\": 7}"));
                    test::expect_reject_as("issuer", "run", Json::parse("{\"count\": -1}"));
                    let _ = next;
                }
            }
        "#;
        let prog = parse(src).expect("parse invoke_entrypoint_as");
        let typed = semantic::SemanticContext::with_capabilities(false, true)
            .analyze(&prog)
            .expect("analyze invoke_entrypoint_as");
        let ir = lower_with_cap_and_test_mode(&typed, 2, true).expect("lower invoke_entrypoint_as");
        let test_fn = ir
            .functions
            .iter()
            .find(|function| function.name == "drive_run")
            .expect("test function");

        let mut saw_invoke = false;
        let mut saw_expect_reject = false;
        for block in &test_fn.blocks {
            for instr in &block.instrs {
                match instr {
                    Instr::InvokeEntrypointAs {
                        dest: Some(_),
                        returns_pointer,
                        ..
                    } => {
                        saw_invoke = true;
                        assert!(
                            !returns_pointer,
                            "int-returning invoke_entrypoint_as should stay scalar"
                        );
                    }
                    Instr::ExpectRejectAs { .. } => saw_expect_reject = true,
                    _ => {}
                }
            }
        }

        assert!(saw_invoke, "expected InvokeEntrypointAs in lowered test");
        assert!(saw_expect_reject, "expected ExpectRejectAs in lowered test");
    }

    #[test]
    fn invoke_entrypoint_as_tuple_return_lowers_to_multi_intrinsic() {
        let src = r#"
            seiyaku Demo {
                kotoage fn run(count: i64) -> (i64, i64) authorize("Entry") { return (count, count + 1); }

                #[test]
                fn drive_run() {
                    let pair = test::invoke_entrypoint_as("issuer", "run", Json::parse("{\"count\": 7}"));
                    test::assert_eq(pair.0, 7);
                    test::assert_eq(pair.1, 8);
                }
            }
        "#;
        let prog = parse(src).expect("parse tuple invoke_entrypoint_as");
        let typed = semantic::SemanticContext::with_capabilities(false, true)
            .analyze(&prog)
            .expect("analyze tuple invoke_entrypoint_as");
        let ir = lower_with_cap_and_test_mode(&typed, 2, true)
            .expect("lower tuple invoke_entrypoint_as");
        let test_fn = ir
            .functions
            .iter()
            .find(|function| function.name == "drive_run")
            .expect("test function");

        let mut saw_multi = false;
        let mut saw_tuple_pack = false;
        for block in &test_fn.blocks {
            for instr in &block.instrs {
                match instr {
                    Instr::InvokeEntrypointAsMulti { dests, .. } => {
                        saw_multi = true;
                        assert_eq!(dests.len(), 2);
                    }
                    Instr::TuplePack { items, .. } if items.len() == 2 => saw_tuple_pack = true,
                    _ => {}
                }
            }
        }

        assert!(
            saw_multi,
            "tuple invoke_entrypoint_as should use multi-return test intrinsic"
        );
        assert!(
            saw_tuple_pack,
            "tuple invoke_entrypoint_as should pack returned values"
        );
    }

    #[test]
    fn actor_helpers_lower_to_test_host_intrinsics() {
        let src = r#"
            seiyaku Demo {
                #[test]
                fn drive_helpers() {
                    let acct = test::actor_account("issuer");
                    let pk = test::actor_public_key("issuer");
                    let sig = test::actor_sign("issuer", b"demo");
                    let _ = (acct, pk, sig);
                }
            }
        "#;
        let prog = parse(src).expect("parse actor helpers");
        let typed = semantic::SemanticContext::with_capabilities(false, true)
            .analyze(&prog)
            .expect("analyze actor helpers");
        let ir = lower_with_cap_and_test_mode(&typed, 2, true).expect("lower actor helpers");
        let test_fn = ir
            .functions
            .iter()
            .find(|function| function.name == "drive_helpers")
            .expect("test function");

        let mut saw_actor_account = false;
        let mut saw_actor_public_key = false;
        let mut saw_actor_sign = false;
        for block in &test_fn.blocks {
            for instr in &block.instrs {
                match instr {
                    Instr::ActorAccount { .. } => saw_actor_account = true,
                    Instr::ActorPublicKey { .. } => saw_actor_public_key = true,
                    Instr::ActorSign { .. } => saw_actor_sign = true,
                    _ => {}
                }
            }
        }

        assert!(saw_actor_account, "expected ActorAccount in lowered test");
        assert!(
            saw_actor_public_key,
            "expected ActorPublicKey in lowered test"
        );
        assert!(saw_actor_sign, "expected ActorSign in lowered test");
    }

    #[test]
    fn lower_if() {
        let src = "fn f(a: i64, b: i64) { if a == b { let c = a; } else { let c = b; } }";
        let ir = lower(&analyze(&parse(src).unwrap()).unwrap()).expect("lower");
        assert_eq!(ir.functions[0].blocks.len(), 4); // entry, then, else, end
    }

    #[test]
    fn logical_operators_lower_to_short_circuit_cfg() {
        let src = r#"
fn rhs() -> bool { return true; }
fn both(value: bool) -> bool { return value && rhs(); }
fn either(value: bool) -> bool { return value || rhs(); }
"#;
        let ir = lower(&analyze(&parse(src).unwrap()).unwrap()).expect("lower logical operators");

        for (name, short_value, rhs_on_then) in [("both", 0, true), ("either", 1, false)] {
            let function = ir
                .functions
                .iter()
                .find(|function| function.name == name)
                .expect("logical function");
            assert!(
                function
                    .blocks
                    .iter()
                    .all(|block| block.instrs.iter().all(|instr| !matches!(
                        instr,
                        Instr::Binary {
                            op: BinaryOp::And | BinaryOp::Or,
                            ..
                        }
                    ))),
                "{name} must not eagerly evaluate logical operands"
            );

            let rhs_block = function
                .blocks
                .iter()
                .find(|block| {
                    block
                        .instrs
                        .iter()
                        .any(|instr| matches!(instr, Instr::Call { callee, .. } if callee == "rhs"))
                })
                .expect("right-hand-side call block");
            let short_block = function
                .blocks
                .iter()
                .find(|block| {
                    block.instrs.iter().any(|instr| {
                        matches!(instr, Instr::Const { value, .. } if *value == short_value)
                    }) && block
                        .instrs
                        .iter()
                        .any(|instr| matches!(instr, Instr::Copy { .. }))
                })
                .expect("short-circuit result block");
            let entry = function
                .blocks
                .iter()
                .find(|block| block.label == function.entry)
                .expect("entry block");
            let Terminator::Branch {
                then_bb, else_bb, ..
            } = &entry.terminator
            else {
                panic!("{name} entry must branch before evaluating rhs");
            };
            if rhs_on_then {
                assert_eq!(*then_bb, rhs_block.label);
                assert_eq!(*else_bb, short_block.label);
            } else {
                assert_eq!(*then_bb, short_block.label);
                assert_eq!(*else_bb, rhs_block.label);
            }

            let result = function
                .blocks
                .iter()
                .find_map(|block| match &block.terminator {
                    Terminator::Return(Some(result)) => Some(*result),
                    _ => None,
                })
                .expect("logical result return");
            for predecessor in [rhs_block, short_block] {
                assert!(
                    predecessor
                        .instrs
                        .iter()
                        .any(|instr| matches!(instr, Instr::Copy { dest, .. } if *dest == result),),
                    "{name} predecessor must merge into the returned result"
                );
            }
        }
    }

    #[test]
    fn lower_bounded_range_loop() {
        let src = "fn f() { for index in range(2) { let value = index; } }";
        let ir = lower(&analyze(&parse(src).unwrap()).unwrap()).expect("lower");
        assert!(ir.functions[0].blocks.len() >= 4);
    }

    #[test]
    fn bounded_loop_only_materializes_live_mutated_phi_slots() {
        let src = r#"
            fn f() -> i64 {
                let invariant = 7;
                var carried = 0;
                var overwritten = 0;
                for index in range(3) {
                    carried = carried + index;
                    overwritten = 99;
                    let observed = invariant;
                }
                overwritten = 5;
                return carried + overwritten + invariant;
            }
        "#;
        let ir = lower(&analyze(&parse(src).unwrap()).unwrap()).expect("lower");
        let function = &ir.functions[0];
        let entry = function
            .blocks
            .iter()
            .find(|block| block.label == function.entry)
            .expect("entry block");
        let invariant = entry
            .instrs
            .iter()
            .find_map(|instruction| match instruction {
                Instr::Const { dest, value: 7 } => Some(*dest),
                _ => None,
            })
            .expect("invariant constant");
        let entry_copies = entry
            .instrs
            .iter()
            .filter_map(|instruction| match instruction {
                Instr::Copy { dest, src } => Some((*dest, *src)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            entry_copies.len(),
            2,
            "only the range index and carried accumulator need loop phi slots"
        );
        assert!(
            entry_copies.iter().all(|(_, src)| *src != invariant),
            "an immutable local must not be copied into loop-carried state"
        );
    }

    #[test]
    fn break_and_continue_update_only_selected_loop_phi_slots() {
        let src = r#"
            fn f() -> i64 {
                let invariant = 10;
                var carried = 0;
                for index in range(4) {
                    if index == 2 { break; }
                    carried = carried + 1;
                    if index == 1 { continue; }
                }
                return carried + invariant;
            }
        "#;
        let ir = lower(&analyze(&parse(src).unwrap()).unwrap()).expect("lower");
        let function = &ir.functions[0];
        let entry = function
            .blocks
            .iter()
            .find(|block| block.label == function.entry)
            .expect("entry block");
        let phi_destinations = entry
            .instrs
            .iter()
            .filter_map(|instruction| match instruction {
                Instr::Copy { dest, .. } => Some(*dest),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(phi_destinations.len(), 2);
        assert!(phi_destinations.iter().all(|destination| {
            function
                .blocks
                .iter()
                .flat_map(|block| block.instrs.iter())
                .filter(|instruction| {
                    matches!(instruction, Instr::Copy { dest, .. } if dest == destination)
                })
                .count()
                >= 2
        }));
    }

    #[test]
    fn state_map_foreach_carries_mutated_locals_through_break_and_continue() {
        let src = r#"
            seiyaku ForeachPhi {
                state Values: StateMap<i64, i64>;

                kotoage fn f() -> i64 authorize("WriteState") {
                    var seen = 0;
                    for (key, value) in Values.take(2) {
                        seen = seen + 1;
                        if (key == value) { continue; }
                        if (seen == 2) { break; }
                    }
                    return seen;
                }
            }
        "#;
        let ir = lower(&analyze(&parse(src).unwrap()).unwrap()).expect("lower");
        let function = ir
            .functions
            .iter()
            .find(|function| function.name == "f")
            .expect("foreach function");
        let returned = function
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                Terminator::Return(Some(result)) => Some(*result),
                _ => None,
            })
            .expect("returned accumulator");
        let entry = function
            .blocks
            .iter()
            .find(|block| block.label == function.entry)
            .expect("entry block");

        assert!(
            entry.instrs.iter().any(
                |instruction| matches!(instruction, Instr::Copy { dest, .. } if *dest == returned)
            ),
            "the returned accumulator must be initialized as a loop phi"
        );
        assert!(
            function
                .blocks
                .iter()
                .flat_map(|block| block.instrs.iter())
                .filter(|instruction| {
                    matches!(instruction, Instr::Copy { dest, .. } if *dest == returned)
                })
                .count()
                >= 3,
            "normal, break, and continue paths must update the loop accumulator"
        );
    }

    #[test]
    fn nested_loops_do_not_carry_outer_invariants() {
        let src = r#"
            fn f() -> i64 {
                let invariant = 9;
                var total = 0;
                for outer in range(2) {
                    for inner in range(2) {
                        total = total + outer + inner;
                        let observed = invariant;
                    }
                }
                return total + invariant;
            }
        "#;
        let ir = lower(&analyze(&parse(src).unwrap()).unwrap()).expect("lower");
        let function = &ir.functions[0];
        let invariant = function
            .blocks
            .iter()
            .flat_map(|block| block.instrs.iter())
            .find_map(|instruction| match instruction {
                Instr::Const { dest, value: 9 } => Some(*dest),
                _ => None,
            })
            .expect("invariant constant");

        assert!(function.blocks.iter().all(|block| {
            block.instrs.iter().all(
                |instruction| !matches!(instruction, Instr::Copy { src, .. } if *src == invariant),
            )
        }));
    }

    #[test]
    fn leaf_identity_ir_has_no_copy_or_stack_pseudo_traffic() {
        let src = "fn identity(value: i64) -> i64 { return value; }";
        let ir = lower(&analyze(&parse(src).unwrap()).unwrap()).expect("lower");
        let function = &ir.functions[0];

        assert_eq!(
            function.blocks.len(),
            2,
            "return creates one dead continuation"
        );
        assert!(function.blocks.iter().all(|block| {
            block
                .instrs
                .iter()
                .all(|instruction| !matches!(instruction, Instr::Copy { .. }))
        }));
        assert!(function.blocks.iter().any(|block| {
            matches!(block.terminator, Terminator::Return(Some(_)))
                && block
                    .instrs
                    .iter()
                    .all(|instruction| matches!(instruction, Instr::LoadVar { .. }))
        }));
    }

    #[test]
    fn lower_return() {
        let src = "fn f() -> i64 { return 1; let x = 2; }";
        let ir = lower(&analyze(&parse(src).unwrap()).unwrap()).expect("lower");
        // Expect at least a Return terminator in one block, and a following unreachable block
        let f = &ir.functions[0];
        assert!(
            f.blocks
                .iter()
                .any(|b| matches!(b.terminator, Terminator::Return(_)))
        );
    }

    #[test]
    fn lower_pointer_constructors_to_datarefs() {
        let src = r#"
            fn main() {
                let k = Name::parse("cursor");
                let v = Json::parse("{}\n");
                let d = DomainId::parse("wonderland.universal");
            }
        "#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        // Expect DataRef instructions for each constructor
        let mut saw_name = false;
        let mut saw_json = false;
        let mut saw_domain = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                if let Instr::DataRef { kind, value, .. } = instr {
                    if *kind == DataRefKind::Name && value == "cursor" {
                        saw_name = true;
                    }
                    if *kind == DataRefKind::Json && value == "{}\n" {
                        saw_json = true;
                    }
                    if *kind == DataRefKind::Domain && value == "wonderland.universal" {
                        saw_domain = true;
                    }
                }
            }
        }
        assert!(saw_name && saw_json && saw_domain);
    }

    #[test]
    fn lower_bytes_literal_to_dataref() {
        let src = r#"fn main() { let _b: bytes = b"ab"; }"#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_blob = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                if let Instr::DataRef { kind, value, .. } = instr
                    && *kind == DataRefKind::Blob
                    && value == "0x6162"
                {
                    saw_blob = true;
                }
            }
        }
        assert!(saw_blob, "expected blob dataref for bytes literal");
    }

    #[test]
    fn source_cannot_lower_setvl_builtin() {
        let src = "fn main() { runtime::set_vector_length(8); }";
        let prog = parse(src).unwrap();
        let error = analyze(&prog).expect_err("vector length is compiler-owned");
        assert!(error.message.contains("unknown function or builtin"));
    }

    #[test]
    fn lower_trigger_event_builtin() {
        let src = "fn main() { let ev = context::trigger_event(); let _kind = ev.get_name(Name::parse(\"kind\")); }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_trigger_event = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                if let Instr::GetTriggerEvent { .. } = instr {
                    saw_trigger_event = true;
                }
            }
        }
        assert!(
            saw_trigger_event,
            "expected GetTriggerEvent instruction in lowered IR"
        );
    }

    #[test]
    fn lower_resolve_account_alias_builtin() {
        let src =
            "fn main() { let _acct = ledger::account::resolve_alias(\"banking@centralbank\"); }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_resolve_account_alias = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                if let Instr::ResolveAccountAlias { .. } = instr {
                    saw_resolve_account_alias = true;
                }
            }
        }
        assert!(
            saw_resolve_account_alias,
            "expected ResolveAccountAlias instruction in lowered IR"
        );
    }

    #[test]
    fn lower_resolve_account_alias_builtin_uses_string_literal() {
        let src = r#"fn main() { let _acct = ledger::account::resolve_alias("merchant@paynet"); }"#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_alias_string = false;
        let mut saw_resolve_account_alias = false;
        let mut saw_static_account_ref = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::StringConst { value, .. } if value == "merchant@paynet" => {
                        saw_alias_string = true;
                    }
                    Instr::ResolveAccountAlias { .. } => saw_resolve_account_alias = true,
                    Instr::DataRef { kind, .. } if *kind == DataRefKind::Account => {
                        saw_static_account_ref = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(
            saw_alias_string,
            "expected builtin alias literal string constant"
        );
        assert!(
            saw_resolve_account_alias,
            "expected ResolveAccountAlias for builtin alias resolution"
        );
        assert!(
            !saw_static_account_ref,
            "resolve_account_alias builtin must not lower to a static AccountId dataref"
        );
    }

    #[test]
    fn lower_resolve_account_alias_invalid_literal_uses_string_literal() {
        let src = r#"fn main() { let _acct = ledger::account::resolve_alias("merchant@"); }"#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_alias_string = false;
        let mut saw_resolve_account_alias = false;
        let mut saw_static_account_ref = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::StringConst { value, .. } if value == "merchant@" => {
                        saw_alias_string = true;
                    }
                    Instr::ResolveAccountAlias { .. } => saw_resolve_account_alias = true,
                    Instr::DataRef { kind, .. } if *kind == DataRefKind::Account => {
                        saw_static_account_ref = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(
            saw_alias_string,
            "expected malformed builtin alias literal string constant"
        );
        assert!(
            saw_resolve_account_alias,
            "malformed builtin aliases should still lower to ResolveAccountAlias"
        );
        assert!(
            !saw_static_account_ref,
            "malformed builtin aliases must not lower to a static AccountId dataref"
        );
    }

    #[test]
    fn lower_resolve_account_alias_domain_qualified_builtin_uses_string_literal() {
        let src =
            r#"fn main() { let _acct = ledger::account::resolve_alias("merchant@bank.paynet"); }"#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_alias_string = false;
        let mut saw_resolve_account_alias = false;
        let mut saw_static_account_ref = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::StringConst { value, .. } if value == "merchant@bank.paynet" => {
                        saw_alias_string = true;
                    }
                    Instr::ResolveAccountAlias { .. } => saw_resolve_account_alias = true,
                    Instr::DataRef { kind, .. } if *kind == DataRefKind::Account => {
                        saw_static_account_ref = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(
            saw_alias_string,
            "expected domain-qualified builtin alias literal string constant"
        );
        assert!(
            saw_resolve_account_alias,
            "expected ResolveAccountAlias for domain-qualified builtin"
        );
        assert!(
            !saw_static_account_ref,
            "resolve_account_alias builtin must not lower to a static AccountId dataref"
        );
    }

    #[test]
    fn lower_resolve_account_alias_invalid_domain_qualified_literal_uses_string_literal() {
        let src = r#"fn main() { let _acct = ledger::account::resolve_alias("merchant@bank."); }"#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_alias_string = false;
        let mut saw_resolve_account_alias = false;
        let mut saw_static_account_ref = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::StringConst { value, .. } if value == "merchant@bank." => {
                        saw_alias_string = true;
                    }
                    Instr::ResolveAccountAlias { .. } => saw_resolve_account_alias = true,
                    Instr::DataRef { kind, .. } if *kind == DataRefKind::Account => {
                        saw_static_account_ref = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(
            saw_alias_string,
            "expected malformed domain-qualified builtin alias literal string constant"
        );
        assert!(
            saw_resolve_account_alias,
            "malformed domain-qualified builtin aliases should still lower to ResolveAccountAlias"
        );
        assert!(
            !saw_static_account_ref,
            "malformed domain-qualified builtin aliases must not lower to a static AccountId dataref"
        );
    }

    #[test]
    fn lower_account_id_alias_literal_to_resolve_account_alias() {
        let src = r#"fn main() { let _acct = AccountId::parse("merchant@paynet"); }"#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_alias_blob = false;
        let mut saw_resolve_account_alias = false;
        let mut saw_static_account_ref = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::DataRef { kind, value, .. }
                        if *kind == DataRefKind::Blob && value == "merchant@paynet" =>
                    {
                        saw_alias_blob = true;
                    }
                    Instr::ResolveAccountAlias { .. } => saw_resolve_account_alias = true,
                    Instr::DataRef { kind, .. } if *kind == DataRefKind::Account => {
                        saw_static_account_ref = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(saw_alias_blob, "expected alias literal blob dataref");
        assert!(
            saw_resolve_account_alias,
            "expected ResolveAccountAlias for alias shorthand"
        );
        assert!(
            !saw_static_account_ref,
            "alias shorthand must not lower to a static AccountId dataref"
        );
    }

    #[test]
    fn lower_account_id_domain_qualified_alias_literal_to_resolve_account_alias() {
        let src = r#"fn main() { let _acct = AccountId::parse("merchant@bank.paynet"); }"#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_alias_blob = false;
        let mut saw_resolve_account_alias = false;
        let mut saw_static_account_ref = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::DataRef { kind, value, .. }
                        if *kind == DataRefKind::Blob && value == "merchant@bank.paynet" =>
                    {
                        saw_alias_blob = true;
                    }
                    Instr::ResolveAccountAlias { .. } => saw_resolve_account_alias = true,
                    Instr::DataRef { kind, .. } if *kind == DataRefKind::Account => {
                        saw_static_account_ref = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(
            saw_alias_blob,
            "expected domain-qualified alias literal blob dataref"
        );
        assert!(
            saw_resolve_account_alias,
            "expected ResolveAccountAlias for domain-qualified alias shorthand"
        );
        assert!(
            !saw_static_account_ref,
            "domain-qualified alias shorthand must not lower to a static AccountId dataref"
        );
    }

    #[test]
    fn lower_account_id_invalid_non_alias_literal_keeps_static_account_dataref() {
        let src = r#"fn main() { let _acct = AccountId::parse("merchant"); }"#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_static_account_ref = false;
        let mut saw_resolve_account_alias = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::DataRef { kind, value, .. }
                        if *kind == DataRefKind::Account && value == "merchant" =>
                    {
                        saw_static_account_ref = true;
                    }
                    Instr::ResolveAccountAlias { .. } => saw_resolve_account_alias = true,
                    _ => {}
                }
            }
        }
        assert!(
            saw_static_account_ref,
            "non-alias account_id literals should stay on the static AccountId path"
        );
        assert!(
            !saw_resolve_account_alias,
            "non-alias account_id literals must not lower to runtime alias resolution"
        );
    }

    #[test]
    fn lower_account_id_canonical_literal_to_static_account_dataref() {
        let canonical = iroha_data_model::account::AccountId::new(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                .parse()
                .expect("public key"),
        )
        .to_string();
        let src = format!(r#"fn main() {{ let _acct = AccountId::parse("{canonical}"); }}"#);
        let prog = parse(&src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_static_account_ref = false;
        let mut saw_resolve_account_alias = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::DataRef { kind, value, .. }
                        if *kind == DataRefKind::Account && value == &canonical =>
                    {
                        saw_static_account_ref = true;
                    }
                    Instr::ResolveAccountAlias { .. } => saw_resolve_account_alias = true,
                    _ => {}
                }
            }
        }
        assert!(
            saw_static_account_ref,
            "canonical account literals should stay as static AccountId datarefs"
        );
        assert!(
            !saw_resolve_account_alias,
            "canonical account literals must not call alias resolution"
        );
    }

    #[test]
    fn lower_account_id_invalid_alias_shaped_literal_to_resolve_account_alias() {
        let src = r#"fn main() { let _acct = AccountId::parse("merchant@"); }"#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_resolve_account_alias = false;
        let mut saw_static_account_ref = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::ResolveAccountAlias { .. } => saw_resolve_account_alias = true,
                    Instr::DataRef { kind, .. } if *kind == DataRefKind::Account => {
                        saw_static_account_ref = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(
            saw_resolve_account_alias,
            "alias-shaped account_id literals should defer validation to the host"
        );
        assert!(
            !saw_static_account_ref,
            "invalid alias-shaped literals must not be encoded as static AccountIds"
        );
    }

    #[test]
    fn lower_account_id_invalid_domain_qualified_alias_literal_to_resolve_account_alias() {
        let src = r#"fn main() { let _acct = AccountId::parse("merchant@bank."); }"#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_resolve_account_alias = false;
        let mut saw_static_account_ref = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::ResolveAccountAlias { .. } => saw_resolve_account_alias = true,
                    Instr::DataRef { kind, .. } if *kind == DataRefKind::Account => {
                        saw_static_account_ref = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(
            saw_resolve_account_alias,
            "invalid domain-qualified alias-shaped literals should defer validation to the host"
        );
        assert!(
            !saw_static_account_ref,
            "invalid domain-qualified alias-shaped literals must not be encoded as static AccountIds"
        );
    }

    #[test]
    fn lower_get_numeric_builtin() {
        let src = "fn main() { let ev = context::trigger_event(); let _amount: Amount = ev.get_numeric(Name::parse(\"amount\")); }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_get_numeric = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                if let Instr::JsonGetNumeric { .. } = instr {
                    saw_get_numeric = true;
                }
            }
        }
        assert!(
            saw_get_numeric,
            "expected JsonGetNumeric instruction in lowered IR"
        );
    }

    #[test]
    fn lower_get_asset_definition_id_builtin() {
        let src = "fn main() { let ev = context::trigger_event(); let _asset = ev.get_asset_definition_id(Name::parse(\"asset_definition_id\")); }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_get_asset_definition_id = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                if let Instr::JsonGetAssetDefinitionId { .. } = instr {
                    saw_get_asset_definition_id = true;
                }
            }
        }
        assert!(
            saw_get_asset_definition_id,
            "expected JsonGetAssetDefinitionId instruction in lowered IR"
        );
    }

    #[test]
    fn lower_state_map_sets_keep_declared_base_names() {
        let src = r#"
            seiyaku StagedMintRequest {
              state MintRequestNextSequence: i64;
              state MintRequestSequenceById: StateMap<Name, i64>;
              state MintRequestSequences: StateMap<i64, i64>;
              state MintRequestRequestIds: StateMap<i64, Name>;
              state MintRequestFiIds: StateMap<i64, Name>;
              state MintRequestFiAuthorities: StateMap<i64, AccountId>;
              state MintRequestToAccounts: StateMap<i64, AccountId>;
              state MintRequestAmounts: StateMap<i64, i64>;
              state MintRequestRequestedBy: StateMap<i64, Json>;
              state MintRequestStates: StateMap<i64, i64>;
              state MintRequestCreatedAt: StateMap<i64, i64>;
              state MintRequestExpiresAt: StateMap<i64, i64>;
              state MintRequestFinalizedAt: StateMap<i64, i64>;
              state MintRequestCanceledAt: StateMap<i64, i64>;

              hajimari() { MintRequestNextSequence = 0; }

              fn update_record(sequence: i64,
                               request_id: Name,
                               fi_id: Name,
                               fi_multisig_account_id: AccountId,
                               to_account_id: AccountId,
                               amount_i64: i64,
                               requested_by_actor_id: Json,
                               state_code: i64,
                               created_at_ms: i64,
                               expires_at_ms: i64,
                               finalized_at_ms: i64,
                               canceled_at_ms: i64) {
                MintRequestSequences[sequence] = sequence;
                MintRequestRequestIds[sequence] = request_id;
                MintRequestFiIds[sequence] = fi_id;
                MintRequestFiAuthorities[sequence] = fi_multisig_account_id;
                MintRequestToAccounts[sequence] = to_account_id;
                MintRequestAmounts[sequence] = amount_i64;
                MintRequestRequestedBy[sequence] = requested_by_actor_id;
                MintRequestStates[sequence] = state_code;
                MintRequestCreatedAt[sequence] = created_at_ms;
                MintRequestExpiresAt[sequence] = expires_at_ms;
                MintRequestFinalizedAt[sequence] = finalized_at_ms;
                MintRequestCanceledAt[sequence] = canceled_at_ms;
              }
            }
        "#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let update_record = ir
            .functions
            .iter()
            .find(|func| func.name == "update_record")
            .expect("update_record function");

        let mut name_literals = HashMap::new();
        let mut bases = Vec::new();
        for block in &update_record.blocks {
            for instr in &block.instrs {
                if let Instr::DataRef {
                    dest,
                    kind: DataRefKind::Name,
                    value,
                } = instr
                {
                    name_literals.insert(*dest, value.clone());
                }
                if let Instr::PathMapKey { base, .. } | Instr::PathMapKeyNorito { base, .. } = instr
                {
                    let base_name = name_literals
                        .get(base)
                        .cloned()
                        .expect("PathMapKey base should originate from a Name DataRef");
                    bases.push(base_name);
                }
            }
        }

        assert_eq!(
            bases,
            vec![
                "MintRequestSequences",
                "MintRequestRequestIds",
                "MintRequestFiIds",
                "MintRequestFiAuthorities",
                "MintRequestToAccounts",
                "MintRequestAmounts",
                "MintRequestRequestedBy",
                "MintRequestStates",
                "MintRequestCreatedAt",
                "MintRequestExpiresAt",
                "MintRequestFinalizedAt",
                "MintRequestCanceledAt",
            ]
        );
    }

    #[test]
    fn lower_bytes_literal_to_blob_dataref() {
        let src = r#"fn main() { let _b = b"ab"; }"#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_blob = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                if let Instr::DataRef { kind, value, .. } = instr
                    && *kind == DataRefKind::Blob
                    && value == "0x6162"
                {
                    saw_blob = true;
                }
            }
        }
        assert!(saw_blob, "expected Blob dataref for bytes literal");
    }

    #[test]
    fn lower_trigger_aliases() {
        let src = r#"
            fn main() {
                ledger::trigger::register(Json::parse("{}"));
                ledger::trigger::remove(Name::parse("wake"));
            }
        "#;
        let ir = lower(&analyze(&parse(src).unwrap()).unwrap()).expect("lower");
        let f = &ir.functions[0];
        let mut saw_create = false;
        let mut saw_remove = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::CreateTrigger { .. } => saw_create = true,
                    Instr::RemoveTrigger { .. } => saw_remove = true,
                    _ => {}
                }
            }
        }
        assert!(saw_create && saw_remove);
    }

    #[test]
    fn lower_struct_fields_for_transfer_domain() {
        let src = r#"
            seiyaku C {
                struct TransferArgs { domain: DomainId; to: AccountId; }
                fn main() {
                    let args = TransferArgs(DomainId::parse("wonderland.universal"), AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"));
                    ledger::domain::transfer(context::authority(), args.domain, args.to);
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = ir.functions.iter().find(|f| f.name == "main").unwrap();
        let mut saw_transfer = false;
        for bb in &f.blocks {
            for ins in &bb.instrs {
                if let Instr::TransferDomain { .. } = ins {
                    saw_transfer = true;
                }
            }
        }
        assert!(saw_transfer, "expected TransferDomain in lowered IR");
    }

    #[test]
    fn lower_info_int_encodes_to_norito() {
        let src = "fn f() { debug::info(7); }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_encode = false;
        let mut saw_info = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::EncodeInt { .. } => saw_encode = true,
                    Instr::Info { .. } => saw_info = true,
                    _ => {}
                }
            }
        }
        assert!(saw_encode, "expected EncodeInt before Info");
        assert!(saw_info, "expected Info instruction");
    }

    #[test]
    fn source_cannot_lower_internal_encode_i64_builtin() {
        let src = "fn f() { let _b = codec::encode_i64(7); }";
        let prog = parse(src).unwrap();
        let error = analyze(&prog).expect_err("codec builtin is compiler-owned");
        assert!(
            error.message.contains("compiler-internal")
                || error.message.contains("unknown function or builtin"),
            "unexpected error: {}",
            error.message
        );
    }

    #[test]
    fn lower_bytes_equality_uses_pointer_eq() {
        let src = r#"fn f() { let a = b"hi"; let b = b"hi"; let _x = a == b; }"#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_pointer_eq = false;
        let mut saw_binary_eq = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::PointerEq { .. } => saw_pointer_eq = true,
                    Instr::Binary {
                        op: BinaryOp::Eq, ..
                    } => saw_binary_eq = true,
                    _ => {}
                }
            }
        }
        assert!(saw_pointer_eq, "expected PointerEq for blob comparison");
        assert!(
            !saw_binary_eq,
            "blob equality should not lower to integer compare"
        );
    }

    #[test]
    fn lower_get_or_on_state_map_reads_without_writing() {
        let src =
            "state balances: StateMap<i64, i64>; fn f() -> i64 { return balances.get_or(1, 7); }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut state_gets = 0;
        let mut state_sets = 0;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::StateGet { .. } => state_gets += 1,
                    Instr::StateSet { .. } => state_sets += 1,
                    _ => {}
                }
            }
        }
        assert!(
            state_gets >= 1,
            "expected get_or durable path to read state"
        );
        assert_eq!(state_sets, 0, "get_or must not mutate durable state");
    }

    #[test]
    fn scalar_state_map_get_reuses_presence_blob() {
        let src = "state balances: StateMap<i64, i64>; fn f() { let _value = balances.get(1); }";
        let typed = analyze(&parse(src).expect("parse StateMap.get")).expect("analyze");
        let ir = lower(&typed).expect("lower");
        let state_gets = ir.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter(|instruction| matches!(instruction, Instr::StateGet { .. }))
            .count();
        assert_eq!(state_gets, 1, "presence and scalar value share one read");
    }

    #[test]
    fn scalar_state_map_remove_reads_and_deletes_once() {
        let src = "state balances: StateMap<i64, i64>; fn f() { let _value = balances.remove(1); }";
        let typed = analyze(&parse(src).expect("parse StateMap.remove")).expect("analyze");
        let ir = lower(&typed).expect("lower");
        let instructions = ir.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .collect::<Vec<_>>();
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| matches!(instruction, Instr::StateGet { .. }))
                .count(),
            1
        );
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| matches!(instruction, Instr::StateDel { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn lower_state_struct_ident_decodes_one_schema_bound_record() {
        let src = r#"
            seiyaku C {
                struct Ledger { counter: i64; flag: bool; }
                state ledger: Ledger;

                hajimari() { ledger = Ledger(0, false); }

                fn main() {
                    let snapshot = ledger;
                    let _ = snapshot.counter;
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let main_fn = ir.functions.iter().find(|f| f.name == "main").unwrap();

        let mut saw_tuple_pack = false;
        let mut saw_root_path = false;
        let mut saw_aggregate_decode = false;
        let mut state_gets = 0;

        for bb in &main_fn.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::TuplePack { .. } => saw_tuple_pack = true,
                    Instr::DataRef {
                        kind: DataRefKind::Name,
                        value,
                        ..
                    } if value == "ledger" => saw_root_path = true,
                    Instr::DirectHelperSyscall { syscall, .. }
                        if *syscall == ivm_abi::syscalls::SYSCALL_STATE_VALUE_DECODE =>
                    {
                        saw_aggregate_decode = true;
                    }
                    Instr::StateGet { .. } => state_gets += 1,
                    _ => {}
                }
            }
        }

        assert!(
            saw_tuple_pack,
            "expected TuplePack when reconstructing struct state"
        );
        assert!(saw_root_path, "missing durable read for ledger root");
        assert!(saw_aggregate_decode, "missing aggregate state decoder");
        assert_eq!(state_gets, 1, "aggregate state must be read exactly once");
    }

    #[test]
    fn lower_state_struct_assignment_encodes_and_writes_once() {
        let src = r#"
            seiyaku C {
                struct Ledger { counter: i64; flag: bool; }
                state ledger: Ledger;

                hajimari() { ledger = Ledger(0, false); }

                fn main() {
                    ledger = Ledger(7, true);
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let main_fn = ir.functions.iter().find(|f| f.name == "main").unwrap();

        let mut saw_root_path = false;
        let mut state_sets = 0;
        let mut tuple_gets = 0;
        let mut aggregate_encodes = 0;

        for bb in &main_fn.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::DataRef {
                        kind: DataRefKind::Name,
                        value,
                        ..
                    } if value == "ledger" => saw_root_path = true,
                    Instr::StateSet { .. } => state_sets += 1,
                    Instr::TupleGet { .. } => tuple_gets += 1,
                    Instr::StateValueEncode { .. } => aggregate_encodes += 1,
                    _ => {}
                }
            }
        }

        assert!(saw_root_path, "missing durable write for ledger root");
        assert_eq!(
            state_sets, 1,
            "aggregate state must be written exactly once"
        );
        assert_eq!(aggregate_encodes, 1, "aggregate state must be encoded once");
        assert!(
            tuple_gets >= 2,
            "expected tuple extraction for the canonical aggregate record"
        );
    }

    #[test]
    fn aggregate_state_map_entry_uses_one_read_and_one_write() {
        let src = r#"
            seiyaku C {
                struct Ledger { counter: i64; flag: bool; }
                state ledgers: StateMap<i64, Ledger>;

                hajimari() {}

                fn main() {
                    ledgers[7] = Ledger(9, true);
                    let _snapshot = ledgers.get(7);
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let main_fn = ir.functions.iter().find(|f| f.name == "main").unwrap();

        let mut state_gets = 0;
        let mut state_sets = 0;
        let mut aggregate_encodes = 0;
        let mut aggregate_decodes = 0;
        let mut child_paths = Vec::new();

        for bb in &main_fn.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::DataRef {
                        kind: DataRefKind::Name,
                        value,
                        ..
                    } if value.contains('#') || value.contains("ledgers_") => {
                        child_paths.push(value.clone());
                    }
                    Instr::StateGet { .. } => state_gets += 1,
                    Instr::StateSet { .. } => state_sets += 1,
                    Instr::StateValueEncode { .. } => aggregate_encodes += 1,
                    Instr::DirectHelperSyscall { syscall, .. }
                        if *syscall == ivm_abi::syscalls::SYSCALL_STATE_VALUE_DECODE =>
                    {
                        aggregate_decodes += 1;
                    }
                    _ => {}
                }
            }
        }

        assert_eq!(state_sets, 1, "StateMap aggregate must use one host write");
        assert_eq!(state_gets, 1, "StateMap aggregate must use one host read");
        assert_eq!(aggregate_encodes, 1);
        assert_eq!(aggregate_decodes, 1);
        assert!(
            child_paths.is_empty(),
            "unexpected child paths: {child_paths:?}"
        );
    }

    #[test]
    fn scalar_state_read_after_write_reuses_the_live_value() {
        let src = r#"
            seiyaku C {
                state counter: i64;
                hajimari() { counter = 0; }
                fn main() -> i64 {
                    counter = 7;
                    return counter;
                }
            }
        "#;
        let prog = parse(src).expect("parse scalar state root");
        let typed = analyze(&prog).expect("analyze scalar state root");
        let ir = lower(&typed).expect("lower scalar state root");
        let main_fn = ir.functions.iter().find(|f| f.name == "main").unwrap();

        let encodes = main_fn
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter(|instr| matches!(instr, Instr::StateValueEncode { .. }))
            .count();
        let decodes = main_fn
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter(|instr| {
                matches!(
                    instr,
                    Instr::DirectHelperSyscall { syscall, .. }
                        if *syscall == ivm_abi::syscalls::SYSCALL_STATE_VALUE_DECODE
                )
            })
            .count();
        assert_eq!(encodes, 1, "scalar root must be encoded exactly once");
        assert_eq!(
            decodes, 0,
            "a scalar read immediately after its write must reuse the live value"
        );
    }

    #[test]
    fn missing_aggregate_map_entry_branches_before_typed_decode() {
        let src = r#"
            seiyaku C {
                struct Pair { count: i64; ready: bool }
                state values: StateMap<i64, Pair>;
                hajimari() {}
                fn main() { let _missing = values.get(7); }
            }
        "#;
        let prog = parse(src).expect("parse aggregate map read");
        let typed = analyze(&prog).expect("analyze aggregate map read");
        let ir = lower(&typed).expect("lower aggregate map read");
        let main_fn = ir.functions.iter().find(|f| f.name == "main").unwrap();

        let decode_block = main_fn
            .blocks
            .iter()
            .find(|block| {
                block.instrs.iter().any(|instr| {
                    matches!(
                        instr,
                        Instr::DirectHelperSyscall { syscall, .. }
                            if *syscall == ivm_abi::syscalls::SYSCALL_STATE_VALUE_DECODE
                    )
                })
            })
            .expect("present branch must decode the typed record");
        let presence_branch = main_fn
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                Terminator::Branch {
                    then_bb, else_bb, ..
                } if *then_bb == decode_block.label => Some((*then_bb, *else_bb)),
                _ => None,
            })
            .expect("typed decode must be guarded by presence");
        assert_ne!(presence_branch.0, presence_branch.1);
        let absent = main_fn
            .blocks
            .iter()
            .find(|block| block.label == presence_branch.1)
            .expect("absent branch");
        assert!(
            absent.instrs.iter().all(|instr| !matches!(
                instr,
                Instr::DirectHelperSyscall { syscall, .. }
                    if *syscall == ivm_abi::syscalls::SYSCALL_STATE_VALUE_DECODE
            )),
            "missing entries must never be passed to the typed decoder"
        );
    }

    #[test]
    fn bytes_state_map_uses_the_schema_bound_record_codec() {
        let source = r#"
            seiyaku C {
                state values: StateMap<i64, bytes>;
                hajimari() {}
                fn main() {
                    values[7] = b"payload";
                    let _stored = values.get(7);
                }
            }
        "#;
        let program = parse(source).expect("parse bytes state map");
        let typed = analyze(&program).expect("analyze bytes state map");
        let ir = lower(&typed).expect("lower bytes state map");
        let main = ir
            .functions
            .iter()
            .find(|function| function.name == "main")
            .expect("main function");

        let encodes = main
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter(|instruction| matches!(instruction, Instr::StateValueEncode { .. }))
            .count();
        let decodes = main
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter(|instruction| {
                matches!(
                    instruction,
                    Instr::DirectHelperSyscall { syscall, .. }
                        if *syscall == ivm_abi::syscalls::SYSCALL_STATE_VALUE_DECODE
                )
            })
            .count();
        assert_eq!(encodes, 1, "bytes map writes must use the typed codec");
        assert_eq!(decodes, 1, "bytes map reads must use the typed codec");
    }

    #[test]
    fn checked_integer_constants_fold_without_wrapping() {
        let safe = parse("fn main() -> i64 { return (9223372036854775807 - 1) + 1; }")
            .expect("parse safe constant expression");
        let safe = lower(&analyze(&safe).expect("analyze safe constant expression"))
            .expect("lower safe constant expression");
        assert!(safe.functions[0].blocks.iter().any(|block| {
            block
                .instrs
                .iter()
                .any(|instr| matches!(instr, Instr::Const { value, .. } if *value == i64::MAX))
        }));

        let overflow = parse("fn main() -> i64 { return 9223372036854775807 + 1; }")
            .expect("parse overflowing constant expression");
        let error =
            lower(&analyze(&overflow).expect("ordinary type analysis accepts runtime arithmetic"))
                .expect_err("constant overflow must not wrap");
        assert!(
            error.contains("E_INT_OVERFLOW"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn wrapping_builtins_have_distinct_ir() {
        let program = parse(
            r#"
fn main(left: i64, right: i64) -> (i64, i64, i64, i64) {
    return (
        math::wrapping_add(left, right),
        math::wrapping_sub(left, right),
        math::wrapping_mul(left, right),
        math::wrapping_neg(left)
    );
}
"#,
        )
        .expect("parse wrapping builtins");
        let program = lower(&analyze(&program).expect("analyze wrapping builtins"))
            .expect("lower wrapping builtins");
        let instructions = program.functions[0]
            .blocks
            .iter()
            .flat_map(|block| block.instrs.iter())
            .collect::<Vec<_>>();
        assert_eq!(
            instructions
                .iter()
                .filter(|instr| matches!(instr, Instr::WrappingBinary { .. }))
                .count(),
            3
        );
        assert_eq!(
            instructions
                .iter()
                .filter(|instr| matches!(instr, Instr::WrappingNeg { .. }))
                .count(),
            1
        );
    }
}
