//! Intermediate representation for Kotodama programs.
//!
//! This lowering IR is three-address code with explicit basic-block jumps. It
//! retains deterministic edge assignments as the compiler's de-SSA transport
//! form. Before code generation, [`crate::ssa`] converts it to strict SSA MIR
//! with explicit Phi nodes, verifies dominance and definition uniqueness, and
//! deterministically lowers it back for register allocation.

use std::collections::{BTreeSet, HashMap};

use super::{
    abi_schema::{json_construction_schema, state_value_kind_for_type, state_value_schema},
    ast::{BinaryOp, PatternBinding, STATE_MAP_GET_INTRINSIC, SumVariant, UnaryOp},
    builtins::{Builtin, BuiltinLowering, PointerConstructor},
    semantic::{
        self, Type, TypedBlock, TypedExpr, TypedFunction, TypedItem, TypedParam, TypedProgram,
        TypedStateDecl, TypedStatement,
    },
};

pub const TEST_TRIGGER_EVENT_OVERRIDE_KEY: &str = "__koto_test_trigger_event_json";
const INVOKE_ENTRYPOINT_PREFIX: &str = "__invoke_entrypoint__";

fn state_map_base_name(expr: &semantic::TypedExpr) -> Option<String> {
    if let semantic::ExprKind::Ident(name) = expr.kind() {
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
            // Sums are compiler-owned heap values. Their complete ABI value is
            // one validated raw handle; inactive branches are never flattened
            // into public registers or populated with placeholders.
            handle @ (Type::Option(_) | Type::Result(_, _) | Type::List(_, _)) => {
                words.push(handle);
            }
            leaf => words.push(leaf),
        }
    }

    if !matches!(
        semantic::resolve_struct_type(ty),
        Type::Struct { .. }
            | Type::Tuple(_)
            | Type::Option(_)
            | Type::Result(_, _)
            | Type::List(_, _)
    ) {
        return None;
    }
    let mut words = Vec::new();
    append(ty, &mut words);
    Some(words)
}

fn runtime_value_word_types(ty: &Type) -> Vec<Type> {
    let mut words = Vec::new();
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
            handle @ (Type::Option(_) | Type::Result(_, _) | Type::List(_, _)) => {
                words.push(handle);
            }
            leaf => words.push(leaf),
        }
    }
    append(ty, &mut words);
    words
}

fn runtime_word_is_pointer(ty: &Type) -> bool {
    matches!(
        semantic::resolve_struct_type(ty),
        Type::Int
            | Type::Decimal
            | Type::Quantity
            | Type::String
            | Type::Bytes
            | Type::Json
            | Type::Option(_)
            | Type::Result(_, _)
            | Type::List(_, _)
    ) || semantic::is_pointer_type(ty)
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

/// Nominal wide-numeric ABI family selected by semantic typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WideNumericKind {
    /// Signed adaptive-width integer.
    Int,
    /// Exact bounded decimal.
    Decimal,
    /// Nominal non-negative quantity.
    Quantity,
}

/// Rounded exact-decimal operation selected by the typed source method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericRoundOp {
    /// `decimal / decimal -> decimal`.
    DecimalDiv,
    /// `quantity / decimal -> quantity`.
    QuantityDiv,
    /// `quantity / quantity -> decimal`.
    QuantityRatio,
}

/// Exact-decimal to integer conversion policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecimalToIntOp {
    /// Discard the fractional component toward zero.
    Truncate,
    /// Apply the explicit rounding mode operand.
    Round,
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
    /// Explicit modulo-2^512 `int` addition, subtraction, or multiplication.
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
    /// Explicit modulo-2^512 `int` negation.
    WrappingNeg {
        dest: Temp,
        operand: Temp,
    },
    /// Convert an internal signed IVM scalar into a source `int` pointer.
    IntFromI64 {
        dest: Temp,
        value: Temp,
    },
    /// Convert an internal unsigned IVM scalar into a source `int` pointer.
    IntFromU64 {
        dest: Temp,
        value: Temp,
    },
    /// Fallibly convert a source `int` pointer to an internal signed IVM scalar.
    IntTryToI64 {
        dest: Temp,
        value: Temp,
    },
    /// Fallibly convert a source `int` pointer to an internal unsigned IVM scalar.
    IntTryToU64 {
        dest: Temp,
        value: Temp,
    },
    /// Convert between source numeric pointer domains.
    NumericConvert {
        dest: Temp,
        value: Temp,
        source: WideNumericKind,
        destination: WideNumericKind,
    },
    /// Recoverable conversion that preserves the ABI numeric-fault status.
    NumericTryConvert {
        dest: Temp,
        value: Temp,
        source: WideNumericKind,
        destination: WideNumericKind,
    },
    /// Capture the status register immediately after a recoverable numeric syscall.
    NumericStatus {
        dest: Temp,
    },
    /// Checked source numeric negation.
    NumericNeg {
        dest: Temp,
        value: Temp,
        kind: WideNumericKind,
    },
    /// Numeric arithmetic using NoritoBytes payloads.
    NumericBinary {
        dest: Temp,
        op: BinaryOp,
        left: Temp,
        right: Temp,
        left_kind: WideNumericKind,
        right_kind: WideNumericKind,
        result_kind: WideNumericKind,
    },
    /// Rounded numeric division with an explicit scale and rounding tag.
    NumericRound {
        dest: Temp,
        dividend: Temp,
        divisor: Temp,
        scale: Temp,
        mode: Temp,
        op: NumericRoundOp,
        result_kind: WideNumericKind,
    },
    /// Convert a decimal pointer to an integer pointer with an explicit policy.
    DecimalToInt {
        dest: Temp,
        value: Temp,
        mode: Option<Temp>,
        op: DecimalToIntOp,
    },
    /// Numeric comparison using NoritoBytes payloads (result is 0/1).
    NumericCompare {
        dest: Temp,
        op: BinaryOp,
        left: Temp,
        right: Temp,
        kind: WideNumericKind,
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
    /// Abort execution with a stable seiyaku error code if `cond` is non-zero.
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
    /// Load a 64-bit value from an already-computed memory address.
    Load64 {
        dest: Temp,
        address: Temp,
    },
    /// Store a 64-bit value to memory at `[base + imm]`.
    ///
    /// Bounded collection lowering uses this primitive to initialise and
    /// mutate its single contiguous compiler-owned allocation. The offset is
    /// expressed in bytes and code generation expands addresses outside the
    /// instruction immediate range without changing observable behaviour.
    Store64Imm {
        base: Temp,
        imm: i16,
        value: Temp,
    },
    /// Store a 64-bit value to an already-computed memory address.
    Store64 {
        address: Temp,
        value: Temp,
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
    /// Grant an exact entrypoint capability for the executing contract address.
    GrantContractEntrypoint {
        account: Temp,
        entrypoint: Temp,
    },
    /// Revoke an exact entrypoint capability for the executing contract address.
    RevokeContractEntrypoint {
        account: Temp,
        entrypoint: Temp,
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
    /// Specialist byte-returning query helper retained outside the five V1
    /// projected core families.
    QueryGet {
        dest: Temp,
        key: Temp,
        syscall: u32,
    },
    /// Typed singular core query selected by a stable V1 entity tag.
    CoreQueryGet {
        dest: Temp,
        key: Temp,
        entity: ivm_abi::core_query::CoreQueryEntityTagV1,
    },
    /// Typed plural core query selected by a stable V1 entity tag.
    CoreQueryPage {
        /// Raw `List<View, 64>` handle returned in syscall register r10.
        items_dest: Temp,
        /// Raw `Option<i64>` next-offset handle returned in syscall register r11.
        next_offset_dest: Temp,
        entity: ivm_abi::core_query::CoreQueryEntityTagV1,
        offset: Temp,
        limit: Temp,
    },
    /// Host balance query: r10 = &AccountId, r11 = &AssetDefinitionId; returns &QuantityV1.
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
    /// Build a schema-bound Name path from canonical pointer-envelope key bytes.
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
    /// JSON getter returning one active-only typed numeric `Option<T>` handle.
    JsonGetNumeric {
        dest: Temp,
        json: Temp,
        key: Temp,
        kind: WideNumericKind,
    },
    /// JSON getter returning one active-only `Option<Json>` handle.
    JsonGetJson {
        dest: Temp,
        json: Temp,
        key: Temp,
    },
    /// JSON getter returning one active-only `Option<Name>` handle.
    JsonGetName {
        dest: Temp,
        json: Temp,
        key: Temp,
    },
    /// JSON getter returning one active-only `Option<AccountId>` handle.
    JsonGetAccountId {
        dest: Temp,
        json: Temp,
        key: Temp,
    },
    /// JSON getter returning one active-only `Option<AssetDefinitionId>` handle.
    JsonGetAssetDefinitionId {
        dest: Temp,
        json: Temp,
        key: Temp,
    },
    /// JSON getter returning one active-only `Option<NftId>` handle.
    JsonGetNftId {
        dest: Temp,
        json: Temp,
        key: Temp,
    },
    /// JSON getter returning one active-only `Option<bytes>` handle.
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
        request: Temp,
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
    /// Canonical signed adaptive-width integer.
    Int,
    /// Canonical exact decimal.
    Decimal,
    /// Canonical non-negative quantity.
    Quantity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyCodec {
    Int,
    Pointer,
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
        Type::Int => Some(DataRefKind::Int),
        Type::Decimal => Some(DataRefKind::Decimal),
        Type::Quantity => Some(DataRefKind::Quantity),
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

fn wide_numeric_kind_for_type(ty: &Type) -> Option<WideNumericKind> {
    match semantic::resolve_struct_type(ty) {
        Type::Int => Some(WideNumericKind::Int),
        Type::Decimal => Some(WideNumericKind::Decimal),
        Type::Quantity => Some(WideNumericKind::Quantity),
        _ => None,
    }
}

fn lower_map_key_eq(ctx: &mut LowerCtx, key_ty: &Type, left: Temp, right: Temp) -> Temp {
    if semantic::is_wide_numeric_type(key_ty) {
        let t = ctx.new_temp();
        ctx.current_instr(Instr::NumericCompare {
            dest: t,
            op: BinaryOp::Eq,
            left,
            right,
            kind: wide_numeric_kind_for_type(key_ty)
                .expect("wide numeric map key has a nominal ABI kind"),
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
        Type::Bool => Some(KeyCodec::Int),
        ty if semantic::is_wide_numeric_type(&ty) => Some(KeyCodec::Pointer),
        Type::String | Type::Bytes => Some(KeyCodec::Pointer),
        other if semantic::is_pointer_type(&other) => Some(KeyCodec::Pointer),
        _ => None,
    }
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
        Type::Option(_) | Type::Result(_, _) | Type::List(_, _) => {
            words.push(value);
            true
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

fn collect_json_construction_words(
    ctx: &mut LowerCtx,
    expr: &TypedExpr,
    vars: &mut HashMap<String, Temp>,
    words: &mut Vec<Temp>,
) -> bool {
    match expr.kind() {
        semantic::ExprKind::JsonObject(entries) => entries
            .iter()
            .all(|(_, value)| collect_json_construction_words(ctx, value, vars, words)),
        semantic::ExprKind::JsonArray(elements) => elements
            .iter()
            .all(|element| collect_json_construction_words(ctx, element, vars, words)),
        _ => {
            let value = lower_expr(ctx, expr, vars);
            collect_state_value_words(ctx, value, &expr.ty, words)
        }
    }
}

fn lower_json_construction(
    ctx: &mut LowerCtx,
    expr: &TypedExpr,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let construction = match json_construction_schema(expr) {
        Ok(construction) => construction,
        Err(error) => {
            ctx.record_error(format!("internal error: {error}"));
            return emit_i64_const(ctx, 0);
        }
    };
    let expected_words = construction.word_count;
    let encoded_schema = construction.encoded;

    let schema_ref = ctx.new_temp();
    ctx.current_instr(Instr::DataRef {
        dest: schema_ref,
        kind: DataRefKind::NoritoBytes,
        value: format!("0x{}", hex::encode(encoded_schema)),
    });

    let mut words = Vec::with_capacity(expected_words);
    if !collect_json_construction_words(ctx, expr, vars, &mut words)
        || words.len() != expected_words
        || words.len() > ivm_abi::state_value::MAX_STATE_VALUE_WORDS
    {
        ctx.record_error("internal error: native JSON value-word schema mismatch".into());
        return emit_i64_const(ctx, 0);
    }

    let table = if words.is_empty() {
        emit_i64_const(ctx, 0)
    } else {
        let Some(byte_len) = words
            .len()
            .checked_mul(std::mem::size_of::<u64>())
            .and_then(|bytes| i64::try_from(bytes).ok())
        else {
            ctx.record_error("native JSON value table exceeds the V1 byte limit".into());
            return emit_i64_const(ctx, 0);
        };
        let bytes = emit_i64_const(ctx, byte_len);
        let table = ctx.new_temp();
        ctx.current_instr(Instr::Alloc { dest: table, bytes });
        for (index, word) in words.into_iter().enumerate() {
            let Some(offset) = index
                .checked_mul(std::mem::size_of::<u64>())
                .and_then(|offset| i16::try_from(offset).ok())
            else {
                ctx.record_error("native JSON value-table offset exceeds the V1 limit".into());
                return emit_i64_const(ctx, 0);
            };
            ctx.current_instr(Instr::Store64Imm {
                base: table,
                imm: offset,
                value: word,
            });
        }
        table
    };
    let word_count = emit_i64_const(ctx, i64::try_from(expected_words).unwrap_or(i64::MAX));
    let dest = ctx.new_temp();
    ctx.current_instr(Instr::DirectHelperSyscall {
        dest,
        syscall: ivm_abi::syscalls::SYSCALL_JSON_BUILD,
        args: vec![schema_ref, table, word_count],
    });
    dest
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
        Type::Option(_) | Type::Result(_, _) | Type::List(_, _) => words.push(value),
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
        Type::Option(_) | Type::Result(_, _) | Type::List(_, _) => {
            load_state_value_word(ctx, table, index)
        }
        leaf => {
            state_value_kind_for_type(&leaf)?;
            load_state_value_word(ctx, table, index)
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
        Type::Option(_) | Type::Result(_, _) | Type::List(_, _) => {
            let word = *words.get(*index)?;
            *index = index.saturating_add(1);
            Some(word)
        }
        _ => {
            let word = *words.get(*index)?;
            *index = index.saturating_add(1);
            Some(word)
        }
    }
}

fn sum_layout_for_type(ty: &Type) -> Option<ivm_abi::sum::SumLayoutV1> {
    let word_count = |payload: &Type| u64::try_from(runtime_value_word_types(payload).len()).ok();
    match semantic::resolve_struct_type(ty) {
        Type::Option(payload) => ivm_abi::sum::SumLayoutV1::option(word_count(&payload)?).ok(),
        // The canonical tag is zero for `err` and one for `ok`.
        Type::Result(ok, err) => {
            ivm_abi::sum::SumLayoutV1::try_new(word_count(&err)?, word_count(&ok)?).ok()
        }
        _ => None,
    }
}

fn sum_active_payload_type(ty: &Type, tag: u64) -> Option<Option<Type>> {
    match (semantic::resolve_struct_type(ty), tag) {
        (Type::Option(_), 0) => Some(None),
        (Type::Option(payload), 1) => Some(Some(*payload)),
        (Type::Result(_, err), 0) => Some(Some(*err)),
        (Type::Result(ok, _), 1) => Some(Some(*ok)),
        _ => None,
    }
}

/// Allocate one canonical active-only sum value.
///
/// The allocation reserves the larger branch once, writes the discriminant,
/// and writes only the selected branch. In particular, this helper never
/// evaluates or constructs an inactive payload.
fn emit_sum_value(ctx: &mut LowerCtx, sum_ty: &Type, tag: u64, payload: Option<Temp>) -> Temp {
    let Some(layout) = sum_layout_for_type(sum_ty) else {
        ctx.record_error("internal error: invalid sum layout".into());
        let invalid = ctx.new_temp();
        ctx.current_instr(Instr::Const {
            dest: invalid,
            value: 0,
        });
        return invalid;
    };
    let Some(payload_ty) = sum_active_payload_type(sum_ty, tag) else {
        ctx.record_error("internal error: invalid sum tag".into());
        let invalid = ctx.new_temp();
        ctx.current_instr(Instr::Const {
            dest: invalid,
            value: 0,
        });
        return invalid;
    };

    let mut payload_words = Vec::new();
    match (payload, payload_ty.as_ref()) {
        (Some(value), Some(payload_ty)) => {
            collect_function_value_words(ctx, value, payload_ty, &mut payload_words);
        }
        (None, None) => {}
        _ => ctx.record_error("internal error: sum active payload mismatch".into()),
    }
    let actual_words = u64::try_from(payload_words.len()).unwrap_or(u64::MAX);
    if layout.validate_active_width(tag, actual_words).is_err() {
        ctx.record_error("internal error: sum active payload width mismatch".into());
    }

    let bytes = layout
        .allocation_bytes()
        .ok()
        .and_then(|bytes| i64::try_from(bytes).ok())
        .unwrap_or_else(|| {
            ctx.record_error("internal error: sum allocation exceeds V1 limits".into());
            8
        });
    let byte_count = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: byte_count,
        value: bytes,
    });
    let value = ctx.new_temp();
    ctx.current_instr(Instr::Alloc {
        dest: value,
        bytes: byte_count,
    });
    let tag_temp = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: tag_temp,
        value: i64::try_from(tag).expect("canonical sum tag fits int"),
    });
    ctx.current_instr(Instr::Store64Imm {
        base: value,
        imm: 0,
        value: tag_temp,
    });
    for (index, word) in payload_words.into_iter().enumerate() {
        let offset = index
            .checked_add(1)
            .and_then(|word_index| word_index.checked_mul(8))
            .and_then(|offset| i16::try_from(offset).ok());
        let Some(imm) = offset else {
            ctx.record_error("internal error: sum payload offset exceeds V1 limits".into());
            break;
        };
        ctx.current_instr(Instr::Store64Imm {
            base: value,
            imm,
            value: word,
        });
    }
    value
}

fn load_sum_tag(ctx: &mut LowerCtx, value: Temp) -> Temp {
    let tag = ctx.new_temp();
    ctx.current_instr(Instr::Load64Imm {
        dest: tag,
        base: value,
        imm: 0,
    });
    tag
}

fn load_sum_payload(ctx: &mut LowerCtx, value: Temp, payload_ty: &Type) -> Temp {
    let word_types = runtime_value_word_types(payload_ty);
    let mut words = Vec::with_capacity(word_types.len());
    for index in 0..word_types.len() {
        let imm = index
            .checked_add(1)
            .and_then(|word_index| word_index.checked_mul(8))
            .and_then(|offset| i16::try_from(offset).ok())
            .unwrap_or_else(|| {
                ctx.record_error("internal error: sum payload offset exceeds V1 limits".into());
                0
            });
        let word = ctx.new_temp();
        ctx.current_instr(Instr::Load64Imm {
            dest: word,
            base: value,
            imm,
        });
        words.push(word);
    }
    let mut index = 0;
    let payload = rebuild_function_value_from_words(ctx, payload_ty, &words, &mut index)
        .unwrap_or_else(|| {
            ctx.record_error("internal error: cannot rebuild sum payload".into());
            value
        });
    if index != words.len() {
        ctx.record_error("internal error: sum payload word count mismatch".into());
    }
    payload
}

fn list_layout_for_type(ty: &Type) -> Option<(Type, ivm_abi::list::ListLayoutV1)> {
    let Type::List(element, capacity) = semantic::resolve_struct_type(ty) else {
        return None;
    };
    let element_words = u64::try_from(runtime_value_word_types(&element).len()).ok()?;
    let layout = ivm_abi::list::ListLayoutV1::try_new(u64::from(capacity), element_words).ok()?;
    Some((*element, layout))
}

fn emit_i64_const(ctx: &mut LowerCtx, value: i64) -> Temp {
    let temp = ctx.new_temp();
    ctx.current_instr(Instr::Const { dest: temp, value });
    temp
}

/// Cross the internal scalar/source-value boundary explicitly.
///
/// Some internal arithmetic protocols return signed 64-bit words. A Kotodama
/// `int` is instead a nominal canonical 512-bit TLV value, so a raw word must
/// never escape from an intrinsic whose semantic result is `int`.
fn emit_int_from_i64(ctx: &mut LowerCtx, value: Temp) -> Temp {
    let dest = ctx.new_temp();
    ctx.current_instr(Instr::IntFromI64 { dest, value });
    dest
}

/// Materialize a non-negative machine word as a canonical source `int`.
fn emit_int_from_u64(ctx: &mut LowerCtx, value: Temp) -> Temp {
    let dest = ctx.new_temp();
    ctx.current_instr(Instr::IntFromU64 { dest, value });
    dest
}

fn emit_int_try_to_u64(ctx: &mut LowerCtx, value: Temp) -> Temp {
    let dest = ctx.new_temp();
    ctx.current_instr(Instr::IntTryToU64 { dest, value });
    dest
}

fn emit_list_allocation(ctx: &mut LowerCtx, list_ty: &Type, initial_len: u64) -> Temp {
    let Some((_, layout)) = list_layout_for_type(list_ty) else {
        ctx.record_error("internal error: invalid List layout".into());
        return emit_i64_const(ctx, 0);
    };
    if initial_len > u64::from(layout.capacity()) {
        ctx.record_error("internal error: List initial length exceeds capacity".into());
    }
    let bytes = layout
        .allocation_bytes()
        .ok()
        .and_then(|bytes| i64::try_from(bytes).ok())
        .unwrap_or_else(|| {
            ctx.record_error("internal error: List allocation exceeds V1 limits".into());
            16
        });
    let bytes = emit_i64_const(ctx, bytes);
    let list = ctx.new_temp();
    ctx.current_instr(Instr::Alloc { dest: list, bytes });
    let len = emit_i64_const(ctx, i64::try_from(initial_len).unwrap_or(i64::MAX));
    ctx.current_instr(Instr::Store64Imm {
        base: list,
        imm: 0,
        value: len,
    });
    let capacity = emit_i64_const(ctx, i64::from(layout.capacity()));
    ctx.current_instr(Instr::Store64Imm {
        base: list,
        imm: 8,
        value: capacity,
    });
    list
}

fn emit_list_slot_base(ctx: &mut LowerCtx, list: Temp, index: Temp, element_words: usize) -> Temp {
    let stride = element_words
        .checked_mul(8)
        .and_then(|stride| i64::try_from(stride).ok())
        .unwrap_or_else(|| {
            ctx.record_error("internal error: List element stride exceeds V1 limits".into());
            8
        });
    let stride = emit_i64_const(ctx, stride);
    let offset = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: offset,
        op: BinaryOp::Mul,
        left: index,
        right: stride,
    });
    let slot = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: slot,
        op: BinaryOp::Add,
        left: list,
        right: offset,
    });
    slot
}

fn emit_list_word_address(ctx: &mut LowerCtx, slot: Temp, word_index: usize) -> Temp {
    let offset = word_index
        .checked_add(2)
        .and_then(|word| word.checked_mul(8))
        .and_then(|offset| i64::try_from(offset).ok())
        .unwrap_or_else(|| {
            ctx.record_error("internal error: List word offset exceeds V1 limits".into());
            16
        });
    let offset = emit_i64_const(ctx, offset);
    let address = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: address,
        op: BinaryOp::Add,
        left: slot,
        right: offset,
    });
    address
}

fn load_list_element(ctx: &mut LowerCtx, list: Temp, index: Temp, element_ty: &Type) -> Temp {
    let word_types = runtime_value_word_types(element_ty);
    let slot = emit_list_slot_base(ctx, list, index, word_types.len());
    let mut words = Vec::with_capacity(word_types.len());
    for word_index in 0..word_types.len() {
        let address = emit_list_word_address(ctx, slot, word_index);
        let word = ctx.new_temp();
        ctx.current_instr(Instr::Load64 {
            dest: word,
            address,
        });
        words.push(word);
    }
    let mut word_index = 0;
    let element = rebuild_function_value_from_words(ctx, element_ty, &words, &mut word_index)
        .unwrap_or_else(|| {
            ctx.record_error("internal error: cannot rebuild List element".into());
            list
        });
    if word_index != words.len() {
        ctx.record_error("internal error: List element word count mismatch".into());
    }
    element
}

fn store_list_element(
    ctx: &mut LowerCtx,
    list: Temp,
    index: Temp,
    element: Temp,
    element_ty: &Type,
) {
    let word_types = runtime_value_word_types(element_ty);
    let mut words = Vec::with_capacity(word_types.len());
    collect_function_value_words(ctx, element, element_ty, &mut words);
    if words.len() != word_types.len() {
        ctx.record_error("internal error: List element word count mismatch".into());
        return;
    }
    let slot = emit_list_slot_base(ctx, list, index, words.len());
    for (word_index, word) in words.into_iter().enumerate() {
        let address = emit_list_word_address(ctx, slot, word_index);
        ctx.current_instr(Instr::Store64 {
            address,
            value: word,
        });
    }
}

fn clear_list_element(ctx: &mut LowerCtx, list: Temp, index: Temp, element_ty: &Type) {
    let element_words = runtime_value_word_types(element_ty).len();
    let slot = emit_list_slot_base(ctx, list, index, element_words);
    let zero = emit_i64_const(ctx, 0);
    for word_index in 0..element_words {
        let address = emit_list_word_address(ctx, slot, word_index);
        ctx.current_instr(Instr::Store64 {
            address,
            value: zero,
        });
    }
}

fn emit_list_index_is_present(ctx: &mut LowerCtx, index: Temp, len: Temp) -> Temp {
    let zero = ctx.new_temp();
    ctx.current_instr(Instr::DataRef {
        dest: zero,
        kind: DataRefKind::Int,
        value: "0".to_owned(),
    });
    let len = emit_int_from_u64(ctx, len);
    let non_negative = ctx.new_temp();
    ctx.current_instr(Instr::NumericCompare {
        dest: non_negative,
        op: BinaryOp::Ge,
        left: index,
        right: zero,
        kind: WideNumericKind::Int,
    });
    let below_len = ctx.new_temp();
    ctx.current_instr(Instr::NumericCompare {
        dest: below_len,
        op: BinaryOp::Lt,
        left: index,
        right: len,
        kind: WideNumericKind::Int,
    });
    let present = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: present,
        op: BinaryOp::And,
        left: non_negative,
        right: below_len,
    });
    present
}

fn lower_list_literal(
    ctx: &mut LowerCtx,
    elements: &[TypedExpr],
    list_ty: &Type,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let Some((element_ty, _)) = list_layout_for_type(list_ty) else {
        ctx.record_error("internal error: typed List literal lacks List type".into());
        return emit_i64_const(ctx, 0);
    };
    let list = emit_list_allocation(
        ctx,
        list_ty,
        u64::try_from(elements.len()).unwrap_or(u64::MAX),
    );
    for (index, element) in elements.iter().enumerate() {
        let value = lower_expr(ctx, element, vars);
        let index = emit_i64_const(ctx, i64::try_from(index).unwrap_or(i64::MAX));
        store_list_element(ctx, list, index, value, &element_ty);
    }
    list
}

fn lower_list_comprehension(
    ctx: &mut LowerCtx,
    expression: &TypedExpr,
    item: &str,
    source: &TypedExpr,
    condition: Option<&TypedExpr>,
    list_ty: &Type,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let source_list = lower_expr(ctx, source, vars);
    let Type::List(source_element, _) = semantic::resolve_struct_type(&source.ty) else {
        ctx.record_error("internal error: List comprehension source lost its List type".into());
        return emit_i64_const(ctx, 0);
    };
    let Some((result_element, _)) = list_layout_for_type(list_ty) else {
        ctx.record_error("internal error: List comprehension result lost its List type".into());
        return emit_i64_const(ctx, 0);
    };
    let result = emit_list_allocation(ctx, list_ty, 0);
    let source_len = ctx.new_temp();
    ctx.current_instr(Instr::Load64Imm {
        dest: source_len,
        base: source_list,
        imm: 0,
    });
    let index = emit_i64_const(ctx, 0);
    let one = emit_i64_const(ctx, 1);
    let header = ctx.new_label();
    let body = ctx.new_label();
    let append = ctx.new_label();
    let step = ctx.new_label();
    let end = ctx.new_label();
    ctx.finish_current(Terminator::Jump(header));

    ctx.start_block(header);
    let keep_going = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: keep_going,
        op: BinaryOp::Lt,
        left: index,
        right: source_len,
    });
    ctx.finish_current(Terminator::Branch {
        cond: keep_going,
        then_bb: body,
        else_bb: end,
    });

    ctx.start_block(body);
    let item_value = load_list_element(ctx, source_list, index, &source_element);
    let mut comprehension_vars = vars.clone();
    comprehension_vars.insert(item.to_owned(), item_value);
    if let Some(condition) = condition {
        let condition = lower_expr(ctx, condition, &mut comprehension_vars);
        ctx.finish_current(Terminator::Branch {
            cond: condition,
            then_bb: append,
            else_bb: step,
        });
    } else {
        ctx.finish_current(Terminator::Jump(append));
    }

    ctx.start_block(append);
    let value = lower_expr(ctx, expression, &mut comprehension_vars);
    let result_len = ctx.new_temp();
    ctx.current_instr(Instr::Load64Imm {
        dest: result_len,
        base: result,
        imm: 0,
    });
    store_list_element(ctx, result, result_len, value, &result_element);
    ctx.current_instr(Instr::Binary {
        dest: result_len,
        op: BinaryOp::Add,
        left: result_len,
        right: one,
    });
    ctx.current_instr(Instr::Store64Imm {
        base: result,
        imm: 0,
        value: result_len,
    });
    ctx.finish_current(Terminator::Jump(step));

    ctx.start_block(step);
    ctx.current_instr(Instr::Binary {
        dest: index,
        op: BinaryOp::Add,
        left: index,
        right: one,
    });
    ctx.finish_current(Terminator::Jump(header));

    ctx.start_block(end);
    result
}

fn emit_product_value_eq(
    ctx: &mut LowerCtx,
    left: Temp,
    right: Temp,
    fields: impl IntoIterator<Item = Type>,
) -> Temp {
    let mut result = emit_i64_const(ctx, 1);
    for (index, field_ty) in fields.into_iter().enumerate() {
        let left_field = ctx.new_temp();
        ctx.current_instr(Instr::TupleGet {
            dest: left_field,
            tuple: left,
            index,
        });
        let right_field = ctx.new_temp();
        ctx.current_instr(Instr::TupleGet {
            dest: right_field,
            tuple: right,
            index,
        });
        let equal = emit_typed_value_eq(ctx, left_field, right_field, &field_ty);
        let combined = ctx.new_temp();
        ctx.current_instr(Instr::Binary {
            dest: combined,
            op: BinaryOp::And,
            left: result,
            right: equal,
        });
        result = combined;
    }
    result
}

/// Compare two compiler-owned sums without observing their inactive payloads.
fn emit_sum_value_eq(ctx: &mut LowerCtx, left: Temp, right: Temp, ty: &Type) -> Temp {
    let left_tag = load_sum_tag(ctx, left);
    let right_tag = load_sum_tag(ctx, right);
    let tags_equal = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: tags_equal,
        op: BinaryOp::Eq,
        left: left_tag,
        right: right_tag,
    });

    let matching_tags = ctx.new_label();
    let different_tags = ctx.new_label();
    let end = ctx.new_label();
    let result = ctx.new_temp();
    ctx.finish_current(Terminator::Branch {
        cond: tags_equal,
        then_bb: matching_tags,
        else_bb: different_tags,
    });

    ctx.start_block(different_tags);
    let false_value = emit_i64_const(ctx, 0);
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: false_value,
    });
    ctx.finish_current(Terminator::Jump(end));

    ctx.start_block(matching_tags);
    match semantic::resolve_struct_type(ty) {
        Type::Option(payload) => {
            let some = ctx.new_label();
            let none = ctx.new_label();
            ctx.finish_current(Terminator::Branch {
                cond: left_tag,
                then_bb: some,
                else_bb: none,
            });

            ctx.start_block(none);
            let true_value = emit_i64_const(ctx, 1);
            ctx.current_instr(Instr::Copy {
                dest: result,
                src: true_value,
            });
            ctx.finish_current(Terminator::Jump(end));

            ctx.start_block(some);
            let left_payload = load_sum_payload(ctx, left, &payload);
            let right_payload = load_sum_payload(ctx, right, &payload);
            let equal = emit_typed_value_eq(ctx, left_payload, right_payload, &payload);
            ctx.current_instr(Instr::Copy {
                dest: result,
                src: equal,
            });
            ctx.finish_current(Terminator::Jump(end));
        }
        Type::Result(ok, err) => {
            let success = ctx.new_label();
            let failure = ctx.new_label();
            ctx.finish_current(Terminator::Branch {
                cond: left_tag,
                then_bb: success,
                else_bb: failure,
            });

            ctx.start_block(failure);
            let left_error = load_sum_payload(ctx, left, &err);
            let right_error = load_sum_payload(ctx, right, &err);
            let equal = emit_typed_value_eq(ctx, left_error, right_error, &err);
            ctx.current_instr(Instr::Copy {
                dest: result,
                src: equal,
            });
            ctx.finish_current(Terminator::Jump(end));

            ctx.start_block(success);
            let left_value = load_sum_payload(ctx, left, &ok);
            let right_value = load_sum_payload(ctx, right, &ok);
            let equal = emit_typed_value_eq(ctx, left_value, right_value, &ok);
            ctx.current_instr(Instr::Copy {
                dest: result,
                src: equal,
            });
            ctx.finish_current(Terminator::Jump(end));
        }
        _ => {
            ctx.record_error("internal error: aggregate equality expected Option or Result".into());
            let false_value = emit_i64_const(ctx, 0);
            ctx.current_instr(Instr::Copy {
                dest: result,
                src: false_value,
            });
            ctx.finish_current(Terminator::Jump(end));
        }
    }

    ctx.start_block(end);
    result
}

/// Compare two bounded Lists by length and recursively by their active elements.
fn emit_list_value_eq(ctx: &mut LowerCtx, left: Temp, right: Temp, element_ty: &Type) -> Temp {
    let left_len = ctx.new_temp();
    ctx.current_instr(Instr::Load64Imm {
        dest: left_len,
        base: left,
        imm: 0,
    });
    let right_len = ctx.new_temp();
    ctx.current_instr(Instr::Load64Imm {
        dest: right_len,
        base: right,
        imm: 0,
    });
    let lengths_equal = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: lengths_equal,
        op: BinaryOp::Eq,
        left: left_len,
        right: right_len,
    });

    let compare_elements = ctx.new_label();
    let header = ctx.new_label();
    let body = ctx.new_label();
    let step = ctx.new_label();
    let different = ctx.new_label();
    let equal = ctx.new_label();
    let end = ctx.new_label();
    let result = ctx.new_temp();
    let index = emit_i64_const(ctx, 0);
    let one = emit_i64_const(ctx, 1);
    ctx.finish_current(Terminator::Branch {
        cond: lengths_equal,
        then_bb: compare_elements,
        else_bb: different,
    });

    ctx.start_block(compare_elements);
    ctx.finish_current(Terminator::Jump(header));

    ctx.start_block(header);
    let keep_going = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: keep_going,
        op: BinaryOp::Lt,
        left: index,
        right: left_len,
    });
    ctx.finish_current(Terminator::Branch {
        cond: keep_going,
        then_bb: body,
        else_bb: equal,
    });

    ctx.start_block(body);
    let left_element = load_list_element(ctx, left, index, element_ty);
    let right_element = load_list_element(ctx, right, index, element_ty);
    let elements_equal = emit_typed_value_eq(ctx, left_element, right_element, element_ty);
    ctx.finish_current(Terminator::Branch {
        cond: elements_equal,
        then_bb: step,
        else_bb: different,
    });

    ctx.start_block(step);
    ctx.current_instr(Instr::Binary {
        dest: index,
        op: BinaryOp::Add,
        left: index,
        right: one,
    });
    ctx.finish_current(Terminator::Jump(header));

    ctx.start_block(different);
    let false_value = emit_i64_const(ctx, 0);
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: false_value,
    });
    ctx.finish_current(Terminator::Jump(end));

    ctx.start_block(equal);
    let true_value = emit_i64_const(ctx, 1);
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: true_value,
    });
    ctx.finish_current(Terminator::Jump(end));

    ctx.start_block(end);
    result
}

/// Emit canonical structural equality for one List element schema.
fn emit_typed_value_eq(ctx: &mut LowerCtx, left: Temp, right: Temp, ty: &Type) -> Temp {
    match semantic::resolve_struct_type(ty) {
        Type::Struct { fields, .. } => emit_product_value_eq(
            ctx,
            left,
            right,
            fields.into_iter().map(|(_, field_ty)| field_ty),
        ),
        Type::Tuple(items) => emit_product_value_eq(ctx, left, right, items),
        Type::Option(_) | Type::Result(_, _) => emit_sum_value_eq(ctx, left, right, ty),
        Type::List(element, _) => emit_list_value_eq(ctx, left, right, &element),
        leaf => {
            let equal = ctx.new_temp();
            if let Some(kind) = wide_numeric_kind_for_type(&leaf) {
                ctx.current_instr(Instr::NumericCompare {
                    dest: equal,
                    op: BinaryOp::Eq,
                    left,
                    right,
                    kind,
                });
            } else if is_pointer_eq_type(&leaf) {
                ctx.current_instr(Instr::PointerEq {
                    dest: equal,
                    left,
                    right,
                });
            } else if matches!(leaf, Type::Int | Type::Bool) {
                ctx.current_instr(Instr::Binary {
                    dest: equal,
                    op: BinaryOp::Eq,
                    left,
                    right,
                });
            } else {
                ctx.record_error(format!(
                    "internal error: List.contains cannot compare `{leaf:?}`"
                ));
                let false_value = emit_i64_const(ctx, 0);
                ctx.current_instr(Instr::Copy {
                    dest: equal,
                    src: false_value,
                });
            }
            equal
        }
    }
}

fn lower_list_get(
    ctx: &mut LowerCtx,
    args: &[TypedExpr],
    result_ty: &Type,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let list = lower_expr(ctx, &args[0], vars);
    let index = lower_expr(ctx, &args[1], vars);
    let len = ctx.new_temp();
    ctx.current_instr(Instr::Load64Imm {
        dest: len,
        base: list,
        imm: 0,
    });
    let present = emit_list_index_is_present(ctx, index, len);
    let some = ctx.new_label();
    let none = ctx.new_label();
    let end = ctx.new_label();
    let result = ctx.new_temp();
    ctx.finish_current(Terminator::Branch {
        cond: present,
        then_bb: some,
        else_bb: none,
    });

    ctx.start_block(some);
    let Type::Option(element_ty) = semantic::resolve_struct_type(result_ty) else {
        ctx.record_error("internal error: List.get result is not Option<T>".into());
        return emit_i64_const(ctx, 0);
    };
    // The dominating exact-int comparisons prove `0 <= index < len <= 64`,
    // so this scalar conversion cannot fail. Keeping it inside the active arm
    // makes arbitrary-width out-of-range indices return `Option::none`.
    let index = emit_int_try_to_u64(ctx, index);
    let value = load_list_element(ctx, list, index, &element_ty);
    let value = emit_sum_value(ctx, result_ty, 1, Some(value));
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: value,
    });
    ctx.finish_current(Terminator::Jump(end));

    ctx.start_block(none);
    let value = emit_sum_value(ctx, result_ty, 0, None);
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: value,
    });
    ctx.finish_current(Terminator::Jump(end));
    ctx.start_block(end);
    result
}

fn lower_list_try_set(
    ctx: &mut LowerCtx,
    args: &[TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let list = lower_expr(ctx, &args[0], vars);
    let index = lower_expr(ctx, &args[1], vars);
    let value = lower_expr(ctx, &args[2], vars);
    let Type::List(element_ty, _) = semantic::resolve_struct_type(&args[0].ty) else {
        ctx.record_error("internal error: List.try_set receiver lost List type".into());
        return emit_i64_const(ctx, 0);
    };
    let len = ctx.new_temp();
    ctx.current_instr(Instr::Load64Imm {
        dest: len,
        base: list,
        imm: 0,
    });
    let present = emit_list_index_is_present(ctx, index, len);
    let success = ctx.new_label();
    let failure = ctx.new_label();
    let end = ctx.new_label();
    let result = ctx.new_temp();
    ctx.finish_current(Terminator::Branch {
        cond: present,
        then_bb: success,
        else_bb: failure,
    });

    ctx.start_block(success);
    // Conversion is reached only after the exact-int bounds proof above.
    let index = emit_int_try_to_u64(ctx, index);
    store_list_element(ctx, list, index, value, &element_ty);
    let one = emit_i64_const(ctx, 1);
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: one,
    });
    ctx.finish_current(Terminator::Jump(end));

    ctx.start_block(failure);
    let zero = emit_i64_const(ctx, 0);
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: zero,
    });
    ctx.finish_current(Terminator::Jump(end));
    ctx.start_block(end);
    result
}

fn lower_list_try_push(
    ctx: &mut LowerCtx,
    args: &[TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let list = lower_expr(ctx, &args[0], vars);
    let value = lower_expr(ctx, &args[1], vars);
    let Type::List(element_ty, capacity) = semantic::resolve_struct_type(&args[0].ty) else {
        ctx.record_error("internal error: List.try_push receiver lost List type".into());
        return emit_i64_const(ctx, 0);
    };
    let len = ctx.new_temp();
    ctx.current_instr(Instr::Load64Imm {
        dest: len,
        base: list,
        imm: 0,
    });
    let capacity = emit_i64_const(ctx, i64::from(capacity));
    let has_capacity = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: has_capacity,
        op: BinaryOp::Lt,
        left: len,
        right: capacity,
    });
    let success = ctx.new_label();
    let failure = ctx.new_label();
    let end = ctx.new_label();
    let result = ctx.new_temp();
    ctx.finish_current(Terminator::Branch {
        cond: has_capacity,
        then_bb: success,
        else_bb: failure,
    });

    ctx.start_block(success);
    store_list_element(ctx, list, len, value, &element_ty);
    let one = emit_i64_const(ctx, 1);
    let new_len = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: new_len,
        op: BinaryOp::Add,
        left: len,
        right: one,
    });
    ctx.current_instr(Instr::Store64Imm {
        base: list,
        imm: 0,
        value: new_len,
    });
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: one,
    });
    ctx.finish_current(Terminator::Jump(end));

    ctx.start_block(failure);
    let zero = emit_i64_const(ctx, 0);
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: zero,
    });
    ctx.finish_current(Terminator::Jump(end));
    ctx.start_block(end);
    result
}

fn lower_list_pop(
    ctx: &mut LowerCtx,
    args: &[TypedExpr],
    result_ty: &Type,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let list = lower_expr(ctx, &args[0], vars);
    let Type::Option(element_ty) = semantic::resolve_struct_type(result_ty) else {
        ctx.record_error("internal error: List.pop result is not Option<T>".into());
        return emit_i64_const(ctx, 0);
    };
    let len = ctx.new_temp();
    ctx.current_instr(Instr::Load64Imm {
        dest: len,
        base: list,
        imm: 0,
    });
    let zero = emit_i64_const(ctx, 0);
    let non_empty = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: non_empty,
        op: BinaryOp::Gt,
        left: len,
        right: zero,
    });
    let some = ctx.new_label();
    let none = ctx.new_label();
    let end = ctx.new_label();
    let result = ctx.new_temp();
    ctx.finish_current(Terminator::Branch {
        cond: non_empty,
        then_bb: some,
        else_bb: none,
    });

    ctx.start_block(some);
    let one = emit_i64_const(ctx, 1);
    let new_len = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: new_len,
        op: BinaryOp::Sub,
        left: len,
        right: one,
    });
    let value = load_list_element(ctx, list, new_len, &element_ty);
    clear_list_element(ctx, list, new_len, &element_ty);
    ctx.current_instr(Instr::Store64Imm {
        base: list,
        imm: 0,
        value: new_len,
    });
    let value = emit_sum_value(ctx, result_ty, 1, Some(value));
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: value,
    });
    ctx.finish_current(Terminator::Jump(end));

    ctx.start_block(none);
    let value = emit_sum_value(ctx, result_ty, 0, None);
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: value,
    });
    ctx.finish_current(Terminator::Jump(end));
    ctx.start_block(end);
    result
}

fn lower_list_contains(
    ctx: &mut LowerCtx,
    args: &[TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let list = lower_expr(ctx, &args[0], vars);
    let needle = lower_expr(ctx, &args[1], vars);
    let Type::List(element_ty, _) = semantic::resolve_struct_type(&args[0].ty) else {
        ctx.record_error("internal error: List.contains receiver lost List type".into());
        return emit_i64_const(ctx, 0);
    };
    let len = ctx.new_temp();
    ctx.current_instr(Instr::Load64Imm {
        dest: len,
        base: list,
        imm: 0,
    });
    let index = emit_i64_const(ctx, 0);
    let one = emit_i64_const(ctx, 1);
    let result = emit_i64_const(ctx, 0);
    let header = ctx.new_label();
    let body = ctx.new_label();
    let found = ctx.new_label();
    let step = ctx.new_label();
    let end = ctx.new_label();
    ctx.finish_current(Terminator::Jump(header));

    ctx.start_block(header);
    let keep_going = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: keep_going,
        op: BinaryOp::Lt,
        left: index,
        right: len,
    });
    ctx.finish_current(Terminator::Branch {
        cond: keep_going,
        then_bb: body,
        else_bb: end,
    });

    ctx.start_block(body);
    let candidate = load_list_element(ctx, list, index, &element_ty);
    let equal = emit_typed_value_eq(ctx, candidate, needle, &element_ty);
    ctx.finish_current(Terminator::Branch {
        cond: equal,
        then_bb: found,
        else_bb: step,
    });

    ctx.start_block(found);
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: one,
    });
    ctx.finish_current(Terminator::Jump(end));

    ctx.start_block(step);
    ctx.current_instr(Instr::Binary {
        dest: index,
        op: BinaryOp::Add,
        left: index,
        right: one,
    });
    ctx.finish_current(Terminator::Jump(header));
    ctx.start_block(end);
    result
}

fn lower_list_take(
    ctx: &mut LowerCtx,
    args: &[TypedExpr],
    result_ty: &Type,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let source = lower_expr(ctx, &args[0], vars);
    let limit = lower_expr_as_u64(ctx, &args[1], vars);
    let Type::List(source_element, _) = semantic::resolve_struct_type(&args[0].ty) else {
        ctx.record_error("internal error: List.take receiver lost List type".into());
        return emit_i64_const(ctx, 0);
    };
    let Some((result_element, _)) = list_layout_for_type(result_ty) else {
        ctx.record_error("internal error: List.take result lost List type".into());
        return emit_i64_const(ctx, 0);
    };
    let result = emit_list_allocation(ctx, result_ty, 0);
    let source_len = ctx.new_temp();
    ctx.current_instr(Instr::Load64Imm {
        dest: source_len,
        base: source,
        imm: 0,
    });
    let index = emit_i64_const(ctx, 0);
    let one = emit_i64_const(ctx, 1);
    let header = ctx.new_label();
    let body = ctx.new_label();
    let end = ctx.new_label();
    ctx.finish_current(Terminator::Jump(header));

    ctx.start_block(header);
    let below_len = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: below_len,
        op: BinaryOp::Lt,
        left: index,
        right: source_len,
    });
    let below_limit = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: below_limit,
        op: BinaryOp::Lt,
        left: index,
        right: limit,
    });
    let keep_going = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: keep_going,
        op: BinaryOp::And,
        left: below_len,
        right: below_limit,
    });
    ctx.finish_current(Terminator::Branch {
        cond: keep_going,
        then_bb: body,
        else_bb: end,
    });

    ctx.start_block(body);
    let value = load_list_element(ctx, source, index, &source_element);
    store_list_element(ctx, result, index, value, &result_element);
    let new_len = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: new_len,
        op: BinaryOp::Add,
        left: index,
        right: one,
    });
    ctx.current_instr(Instr::Store64Imm {
        base: result,
        imm: 0,
        value: new_len,
    });
    ctx.current_instr(Instr::Copy {
        dest: index,
        src: new_len,
    });
    ctx.finish_current(Terminator::Jump(header));
    ctx.start_block(end);
    result
}

fn lower_list_enumerate(
    ctx: &mut LowerCtx,
    args: &[TypedExpr],
    result_ty: &Type,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let source = lower_expr(ctx, &args[0], vars);
    let Type::List(source_element, _) = semantic::resolve_struct_type(&args[0].ty) else {
        ctx.record_error("internal error: List.enumerate receiver lost List type".into());
        return emit_i64_const(ctx, 0);
    };
    let Some((result_element, _)) = list_layout_for_type(result_ty) else {
        ctx.record_error("internal error: List.enumerate result lost List type".into());
        return emit_i64_const(ctx, 0);
    };
    let result = emit_list_allocation(ctx, result_ty, 0);
    let source_len = ctx.new_temp();
    ctx.current_instr(Instr::Load64Imm {
        dest: source_len,
        base: source,
        imm: 0,
    });
    let index = emit_i64_const(ctx, 0);
    let one = emit_i64_const(ctx, 1);
    let header = ctx.new_label();
    let body = ctx.new_label();
    let end = ctx.new_label();
    ctx.finish_current(Terminator::Jump(header));

    ctx.start_block(header);
    let keep_going = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: keep_going,
        op: BinaryOp::Lt,
        left: index,
        right: source_len,
    });
    ctx.finish_current(Terminator::Branch {
        cond: keep_going,
        then_bb: body,
        else_bb: end,
    });

    ctx.start_block(body);
    let value = load_list_element(ctx, source, index, &source_element);
    let source_index = emit_int_from_u64(ctx, index);
    let pair = ctx.new_temp();
    ctx.current_instr(Instr::TuplePack {
        dest: pair,
        items: vec![source_index, value],
    });
    store_list_element(ctx, result, index, pair, &result_element);
    let new_len = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: new_len,
        op: BinaryOp::Add,
        left: index,
        right: one,
    });
    ctx.current_instr(Instr::Store64Imm {
        base: result,
        imm: 0,
        value: new_len,
    });
    ctx.current_instr(Instr::Copy {
        dest: index,
        src: new_len,
    });
    ctx.finish_current(Terminator::Jump(header));
    ctx.start_block(end);
    result
}

fn lower_list_intrinsic(
    ctx: &mut LowerCtx,
    name: &str,
    args: &[TypedExpr],
    result_ty: &Type,
    vars: &mut HashMap<String, Temp>,
) -> Option<Temp> {
    Some(match name {
        semantic::LIST_LEN_INTRINSIC => {
            let list = lower_expr(ctx, &args[0], vars);
            let len = ctx.new_temp();
            ctx.current_instr(Instr::Load64Imm {
                dest: len,
                base: list,
                imm: 0,
            });
            emit_int_from_u64(ctx, len)
        }
        semantic::LIST_GET_INTRINSIC => lower_list_get(ctx, args, result_ty, vars),
        semantic::LIST_TRY_SET_INTRINSIC => lower_list_try_set(ctx, args, vars),
        semantic::LIST_TRY_PUSH_INTRINSIC => lower_list_try_push(ctx, args, vars),
        semantic::LIST_POP_INTRINSIC => lower_list_pop(ctx, args, result_ty, vars),
        semantic::LIST_CONTAINS_INTRINSIC => lower_list_contains(ctx, args, vars),
        semantic::LIST_TAKE_INTRINSIC => lower_list_take(ctx, args, result_ty, vars),
        semantic::LIST_ENUMERATE_INTRINSIC => lower_list_enumerate(ctx, args, result_ty, vars),
        _ => return None,
    })
}

fn lower_numeric_round_intrinsic(
    ctx: &mut LowerCtx,
    name: &str,
    args: &[TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Option<Temp> {
    let (op, result_kind) = match name {
        semantic::DECIMAL_DIV_ROUND_INTRINSIC => {
            (NumericRoundOp::DecimalDiv, WideNumericKind::Decimal)
        }
        semantic::QUANTITY_DIV_ROUND_INTRINSIC => {
            (NumericRoundOp::QuantityDiv, WideNumericKind::Quantity)
        }
        semantic::QUANTITY_RATIO_ROUND_INTRINSIC => {
            (NumericRoundOp::QuantityRatio, WideNumericKind::Decimal)
        }
        _ => return None,
    };
    if args.len() != 4 {
        ctx.record_error(
            "internal error: rounded numeric division requires dividend, divisor, scale, and mode"
                .into(),
        );
        return Some(emit_i64_const(ctx, 0));
    }
    let dividend = lower_expr(ctx, &args[0], vars);
    let divisor = lower_expr(ctx, &args[1], vars);
    let scale = lower_expr(ctx, &args[2], vars);
    let mode = lower_expr_as_u64(ctx, &args[3], vars);
    let dest = ctx.new_temp();
    ctx.current_instr(Instr::NumericRound {
        dest,
        dividend,
        divisor,
        scale,
        mode,
        op,
        result_kind,
    });
    Some(dest)
}

fn lower_decimal_to_int_intrinsic(
    ctx: &mut LowerCtx,
    name: &str,
    args: &[TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Option<Temp> {
    let (op, expected_args) = match name {
        semantic::DECIMAL_TO_INT_TRUNC_INTRINSIC => (DecimalToIntOp::Truncate, 1),
        semantic::DECIMAL_TO_INT_ROUND_INTRINSIC => (DecimalToIntOp::Round, 2),
        _ => return None,
    };
    if args.len() != expected_args {
        ctx.record_error("internal error: malformed decimal-to-int intrinsic".into());
        return Some(emit_i64_const(ctx, 0));
    }
    let value = lower_expr(ctx, &args[0], vars);
    let mode = args.get(1).map(|mode| lower_expr_as_u64(ctx, mode, vars));
    let dest = ctx.new_temp();
    ctx.current_instr(Instr::DecimalToInt {
        dest,
        value,
        mode,
        op,
    });
    Some(dest)
}

fn sum_pattern_tag(pattern: &semantic::TypedSumPattern) -> u64 {
    match pattern.pattern.variant {
        SumVariant::OptionNone | SumVariant::ResultErr => 0,
        SumVariant::OptionSome | SumVariant::ResultOk => 1,
    }
}

fn bind_sum_pattern(
    ctx: &mut LowerCtx,
    pattern: &semantic::TypedSumPattern,
    value: Temp,
    vars: &mut HashMap<String, Temp>,
) {
    let Some(payload_ty) = &pattern.payload_type else {
        return;
    };
    let Some(binding) = &pattern.pattern.binding else {
        return;
    };
    if let PatternBinding::Name(name) = binding {
        let payload = load_sum_payload(ctx, value, payload_ty);
        vars.insert(name.clone(), payload);
    }
}

fn propagation_match_return<'a>(
    value_ty: &Type,
    return_ty: &Type,
    arms: &'a [semantic::TypedMatchArm],
) -> Option<(&'a semantic::TypedMatchArm, &'a semantic::TypedMatchArm)> {
    if arms.len() != 2 {
        return None;
    }
    let success = arms.iter().find(|arm| sum_pattern_tag(&arm.pattern) == 1)?;
    let failure = arms.iter().find(|arm| sum_pattern_tag(&arm.pattern) == 0)?;

    let success_binding = match success.pattern.pattern.binding.as_ref()? {
        PatternBinding::Name(name) => name,
        PatternBinding::Wildcard => return None,
    };
    if !success.body.statements.is_empty()
        || !matches!(
            success.body.tail.as_deref(),
            Some(TypedExpr {
                expr: semantic::ExprKind::Ident(name),
                ..
            }) if name == success_binding
        )
    {
        return None;
    }

    let [TypedStatement::Return(Some(returned))] = failure.body.statements.as_slice() else {
        return None;
    };
    if failure.body.tail.is_some()
        || semantic::resolve_struct_type(&returned.ty) != semantic::resolve_struct_type(return_ty)
    {
        return None;
    }

    match (
        semantic::resolve_struct_type(value_ty),
        semantic::resolve_struct_type(return_ty),
    ) {
        (Type::Option(_), Type::Option(_)) => (failure.pattern.pattern.variant
            == SumVariant::OptionNone
            && failure.pattern.pattern.binding.is_none()
            && matches!(&returned.expr, semantic::ExprKind::OptionNone))
        .then_some((success, failure)),
        (Type::Result(_, source_error), Type::Result(_, target_error))
            if source_error == target_error =>
        {
            let failure_binding = match failure.pattern.pattern.binding.as_ref()? {
                PatternBinding::Name(name) => name,
                PatternBinding::Wildcard => return None,
            };
            match &returned.expr {
                semantic::ExprKind::ResultErr { error }
                    if matches!(
                        error.as_ref(),
                        TypedExpr {
                            expr: semantic::ExprKind::Ident(name),
                            ..
                        } if name == failure_binding
                    ) =>
                {
                    Some((success, failure))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn lower_propagation(
    ctx: &mut LowerCtx,
    value: &TypedExpr,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let sum = lower_expr(ctx, value, vars);
    let tag = load_sum_tag(ctx, sum);
    let success_label = ctx.new_label();
    let failure_label = ctx.new_label();
    ctx.finish_current(Terminator::Branch {
        cond: tag,
        then_bb: success_label,
        else_bb: failure_label,
    });

    ctx.start_block(failure_label);
    let return_ty = ctx.function_return_type.clone();
    let propagated = match (
        sum_layout_for_type(&value.ty),
        sum_layout_for_type(&return_ty),
    ) {
        // A failed sum has no active success payload. Reusing its canonical
        // handle is sound when the complete source and destination allocations
        // have the same shape.
        (Some(source), Some(target)) if source == target => sum,
        (Some(_), Some(_)) => match (
            semantic::resolve_struct_type(&value.ty),
            semantic::resolve_struct_type(&return_ty),
        ) {
            (Type::Option(_), Type::Option(_)) => emit_sum_value(ctx, &return_ty, 0, None),
            (Type::Result(_, source_error), Type::Result(_, _)) => {
                // Semantic analysis requires the exact same error type, with
                // no implicit conversion. Only that active payload is read and
                // copied into the destination layout; the differently-sized
                // inactive success branch remains canonical zero.
                let error = load_sum_payload(ctx, sum, &source_error);
                emit_sum_value(ctx, &return_ty, 0, Some(error))
            }
            _ => {
                ctx.record_error("internal error: propagation changed Option/Result family".into());
                sum
            }
        },
        _ => {
            ctx.record_error("internal error: invalid propagation return layout".into());
            sum
        }
    };
    finish_value_return(ctx, propagated, &return_ty);

    ctx.start_block(success_label);
    let payload_ty = match semantic::resolve_struct_type(&value.ty) {
        Type::Option(payload) | Type::Result(payload, _) => *payload,
        _ => {
            ctx.record_error("internal error: propagation operand is not a sum".into());
            Type::Unit
        }
    };
    load_sum_payload(ctx, sum, &payload_ty)
}

fn copy_runtime_value_words(ctx: &mut LowerCtx, value: Temp, ty: &Type, destinations: &[Temp]) {
    let mut words = Vec::with_capacity(destinations.len());
    collect_function_value_words(ctx, value, ty, &mut words);
    if words.len() != destinations.len() {
        ctx.record_error("internal error: branch result word count mismatch".into());
    }
    for (dest, src) in destinations.iter().zip(words) {
        ctx.current_instr(Instr::Copy { dest: *dest, src });
    }
}

fn rebuild_runtime_value(ctx: &mut LowerCtx, ty: &Type, words: &[Temp]) -> Temp {
    let mut index = 0;
    let value =
        rebuild_function_value_from_words(ctx, ty, words, &mut index).unwrap_or_else(|| {
            ctx.record_error("internal error: cannot rebuild branch result".into());
            words.first().copied().unwrap_or_else(|| {
                let invalid = ctx.new_temp();
                ctx.current_instr(Instr::Const {
                    dest: invalid,
                    value: 0,
                });
                invalid
            })
        });
    if index != words.len() {
        ctx.record_error("internal error: branch result ABI width mismatch".into());
    }
    value
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
    !matches!(func.modifiers.kind, super::ast::FunctionKind::Private)
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
        func.ret_ty.clone().unwrap_or(Type::Unit),
        dyn_iter_cap,
        call_renames.clone(),
        function_param_specs.clone(),
    );
    let entry = ctx.new_label();
    ctx.start_block(entry);
    // Ephemeral state allocation for seiyaku-level `state` declarations.
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

    let tail = lower_block_tail_with_live_after(&mut ctx, &func.body, &mut vars, &BTreeSet::new());
    if let Some(value) = tail {
        let tail_ty = func
            .body
            .tail
            .as_ref()
            .map(|tail| tail.ty.clone())
            .unwrap_or(Type::Unit);
        finish_value_return(&mut ctx, value, &tail_ty);
    } else {
        ctx.finish_current(Terminator::Return(None));
    }

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
        func.ret_ty.clone().unwrap_or(Type::Unit),
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
    } else if let Some(word_types) = function_value_word_types(return_ty) {
        let mut dests = Vec::with_capacity(word_types.len());
        for _ in word_types {
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
    } else if let Some(word_types) = function_value_word_types(result_ty) {
        let mut dests = Vec::with_capacity(word_types.len());
        for _ in word_types {
            dests.push(ctx.new_temp());
        }
        ctx.current_instr(Instr::CallMulti {
            callee: entrypoint.to_string(),
            args: Vec::new(),
            dests: dests.clone(),
        });
        let mut index = 0_usize;
        let value = rebuild_function_value_from_words(ctx, result_ty, &dests, &mut index)
            .expect("validated aggregate return type must rebuild from ABI words");
        debug_assert_eq!(index, dests.len());
        value
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

fn entrypoint_value_kind(
    value_name: &str,
    ty: &Type,
) -> Result<ivm_abi::entrypoint::EntrypointValueKindV1, String> {
    use ivm_abi::entrypoint::EntrypointValueKindV1 as Kind;

    let resolved = semantic::resolve_struct_type(ty);
    Ok(match resolved {
        Type::Int => Kind::Int,
        Type::Decimal => Kind::Decimal,
        Type::Quantity => Kind::Quantity,
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
                "entrypoint value `{value_name}` uses unsupported public type {:?}",
                other,
            ));
        }
    })
}

fn append_entrypoint_value_type_nodes(
    value_name: &str,
    ty: &Type,
    nodes: &mut Vec<ivm_abi::entrypoint::EntrypointValueTypeNodeV1>,
) -> Result<(), String> {
    use ivm_abi::entrypoint::{
        EntrypointListTypeNodeV1 as ListNode, EntrypointStructTypeNodeV1 as StructNode,
        EntrypointValueTypeNodeV1 as Node,
    };

    match semantic::resolve_struct_type(ty) {
        Type::Struct { name, fields } => {
            nodes.push(Node::Struct(StructNode {
                name,
                fields: fields.iter().map(|(name, _)| name.clone()).collect(),
            }));
            for (_, field_ty) in fields {
                append_entrypoint_value_type_nodes(value_name, &field_ty, nodes)?;
            }
        }
        Type::Tuple(items) => {
            let arity = u16::try_from(items.len())
                .map_err(|_| format!("entrypoint value `{value_name}` tuple arity exceeds u16"))?;
            nodes.push(Node::Tuple(arity));
            for item in items {
                append_entrypoint_value_type_nodes(value_name, &item, nodes)?;
            }
        }
        Type::Option(inner) => {
            nodes.push(Node::Option);
            append_entrypoint_value_type_nodes(value_name, &inner, nodes)?;
        }
        Type::Result(ok, err) => {
            nodes.push(Node::Result);
            append_entrypoint_value_type_nodes(value_name, &ok, nodes)?;
            append_entrypoint_value_type_nodes(value_name, &err, nodes)?;
        }
        Type::List(element, capacity) => {
            nodes.push(Node::List(ListNode { capacity }));
            append_entrypoint_value_type_nodes(value_name, &element, nodes)?;
        }
        leaf => nodes.push(Node::Leaf(entrypoint_value_kind(value_name, &leaf)?)),
    }
    Ok(())
}

fn entrypoint_value_type(
    value_name: &str,
    ty: &Type,
) -> Result<ivm_abi::entrypoint::EntrypointValueTypeV1, String> {
    let mut nodes = Vec::new();
    append_entrypoint_value_type_nodes(value_name, ty, &mut nodes)?;
    let ty = ivm_abi::entrypoint::EntrypointValueTypeV1 { nodes };
    if !ty.validate() {
        return Err(format!(
            "entrypoint value `{value_name}` exceeds ABI v1 type depth, node, or word limits"
        ));
    }
    Ok(ty)
}

/// Build the exact recursive schema for a non-unit public return value.
pub(crate) fn entrypoint_return_schema(
    entrypoint_name: &str,
    ty: Option<&Type>,
) -> Result<Option<ivm_abi::entrypoint::EntrypointValueTypeV1>, String> {
    let Some(ty) = ty else {
        return Ok(None);
    };
    if matches!(semantic::resolve_struct_type(ty), Type::Unit) {
        return Ok(None);
    }
    let schema = entrypoint_value_type(entrypoint_name, ty)?;
    let words = schema
        .word_count()
        .ok_or_else(|| format!("entrypoint `{entrypoint_name}` has an invalid return schema"))?;
    if words > ivm_abi::entrypoint::MAX_ENTRYPOINT_RETURN_WORDS {
        return Err(format!(
            "entrypoint `{entrypoint_name}` returns {words} flattened words, exceeding the ABI v1 public register limit of {}",
            ivm_abi::entrypoint::MAX_ENTRYPOINT_RETURN_WORDS,
        ));
    }
    Ok(Some(schema))
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
                ty: entrypoint_value_type(&param.name, &param.ty)?,
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

fn merge_conditional_envs(
    ctx: &mut LowerCtx,
    entry_env: &HashMap<String, Temp>,
    then_env: &HashMap<String, Temp>,
    else_env: &HashMap<String, Temp>,
    then_exit: usize,
    else_exit: usize,
    vars: &mut HashMap<String, Temp>,
) {
    let mut mutated = BTreeSet::new();
    for (name, entry_temp) in entry_env {
        let then_temp = then_env.get(name).copied().unwrap_or(*entry_temp);
        let else_temp = else_env.get(name).copied().unwrap_or(*entry_temp);
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
        let then_temp = then_env.get(&name).copied().unwrap_or(entry_temp);
        let else_temp = else_env.get(&name).copied().unwrap_or(entry_temp);
        if let Some(block) = ctx.blocks.get_mut(then_exit) {
            push_copy(block, join_temp, then_temp);
        }
        if let Some(block) = ctx.blocks.get_mut(else_exit) {
            push_copy(block, join_temp, else_temp);
        }
        vars.insert(name, join_temp);
    }
}

fn collect_expr_reads(expr: &TypedExpr, reads: &mut BTreeSet<String>) {
    match expr.kind() {
        semantic::ExprKind::Binary { left, right, .. } => {
            collect_expr_reads(left, reads);
            collect_expr_reads(right, reads);
        }
        semantic::ExprKind::Unary { expr, .. }
        | semantic::ExprKind::NumericCast { expr }
        | semantic::ExprKind::NumericTryCast { expr } => {
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
        semantic::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_reads(condition, reads);
            collect_block_reads(then_branch, reads);
            collect_block_reads(else_branch, reads);
        }
        semantic::ExprKind::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expr_reads(value, reads);
            collect_block_reads(then_branch, reads);
            collect_block_reads(else_branch, reads);
        }
        semantic::ExprKind::Match { value, arms } => {
            collect_expr_reads(value, reads);
            for arm in arms {
                collect_block_reads(&arm.body, reads);
            }
        }
        semantic::ExprKind::JsonObject(entries) => {
            for (_, value) in entries {
                collect_expr_reads(value, reads);
            }
        }
        semantic::ExprKind::JsonArray(elements) => {
            for element in elements {
                collect_expr_reads(element, reads);
            }
        }
        semantic::ExprKind::OptionSome { value }
        | semantic::ExprKind::ResultOk { value }
        | semantic::ExprKind::ResultErr { error: value }
        | semantic::ExprKind::Propagate { value } => collect_expr_reads(value, reads),
        semantic::ExprKind::Call { args, .. }
        | semantic::ExprKind::NamedCall { args, .. }
        | semantic::ExprKind::Tuple(args)
        | semantic::ExprKind::List(args) => {
            for arg in args {
                collect_expr_reads(arg, reads);
            }
        }
        semantic::ExprKind::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            collect_expr_reads(source, reads);
            collect_expr_reads(expression, reads);
            if let Some(condition) = condition {
                collect_expr_reads(condition, reads);
            }
        }
        semantic::ExprKind::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expr_reads(value, reads);
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
        semantic::ExprKind::IntLiteral(_)
        | semantic::ExprKind::DecimalLiteral { .. }
        | semantic::ExprKind::Bool(_)
        | semantic::ExprKind::String(_)
        | semantic::ExprKind::Bytes(_)
        | semantic::ExprKind::OptionNone => {}
    }
}

fn collect_block_reads(block: &TypedBlock, reads: &mut BTreeSet<String>) {
    for statement in &block.statements {
        collect_statement_reads(statement, reads);
    }
    if let Some(tail) = &block.tail {
        collect_expr_reads(tail, reads);
    }
}

fn collect_statement_reads(statement: &TypedStatement, reads: &mut BTreeSet<String>) {
    match statement.kind() {
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
        TypedStatement::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expr_reads(value, reads);
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
    match statement.kind() {
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
        TypedStatement::IfLet {
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
    let mut live = match statement.kind() {
        // A return has no fallthrough edge. Keeping only its expression reads
        // prevents unreachable trailing statements from extending lifetimes.
        TypedStatement::Return(_) => BTreeSet::new(),
        _ => live_after.clone(),
    };
    if let TypedStatement::Let { name, .. } = statement.kind() {
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
    if let Some(tail) = &block.tail {
        collect_expr_reads(tail, &mut live);
    }
    let mut result = vec![BTreeSet::new(); block.statements.len()];
    for (index, statement) in block.statements.iter().enumerate().rev() {
        result[index] = live.clone();
        live = if matches!(
            statement.kind(),
            TypedStatement::Break | TypedStatement::Continue
        ) {
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

fn lower_block_with_live_after(
    ctx: &mut LowerCtx,
    block: &TypedBlock,
    vars: &mut HashMap<String, Temp>,
    outer_live_after: &BTreeSet<String>,
) {
    let _ = lower_block_tail_with_live_after(ctx, block, vars, outer_live_after);
}

fn lower_block_tail_with_live_after(
    ctx: &mut LowerCtx,
    block: &TypedBlock,
    vars: &mut HashMap<String, Temp>,
    outer_live_after: &BTreeSet<String>,
) -> Option<Temp> {
    let live_after = block_live_after_sets(block, outer_live_after);
    for (statement, statement_live_after) in block.statements.iter().zip(live_after.iter()) {
        lower_statement(ctx, statement, vars, statement_live_after);
    }
    block.tail.as_ref().map(|tail| lower_expr(ctx, tail, vars))
}

fn lower_expression_block(
    ctx: &mut LowerCtx,
    block: &TypedBlock,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    lower_block_tail_with_live_after(ctx, block, vars, &BTreeSet::new()).unwrap_or_else(|| {
        let unit = ctx.new_temp();
        ctx.current_instr(Instr::Const {
            dest: unit,
            value: 0,
        });
        unit
    })
}

fn finish_value_return(ctx: &mut LowerCtx, value: Temp, ty: &Type) {
    if function_value_word_types(ty).is_some() {
        let mut outs = Vec::new();
        collect_function_value_words(ctx, value, ty, &mut outs);
        match outs.as_slice() {
            [] => ctx.finish_current(Terminator::Return(None)),
            [only] => ctx.finish_current(Terminator::Return(Some(*only))),
            [first, second] => ctx.finish_current(Terminator::Return2(*first, *second)),
            _ => ctx.finish_current(Terminator::ReturnN(outs)),
        }
    } else {
        ctx.finish_current(Terminator::Return(Some(value)));
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

            merge_conditional_envs(
                ctx, &entry_env, &then_vars, &else_vars, then_idx, else_idx, vars,
            );

            ctx.start_block(end_label);
        }
        TypedStatement::IfLet {
            pattern,
            value,
            then_branch,
            else_branch,
        } => {
            let entry_env = vars.clone();
            let sum = lower_expr(ctx, value, vars);
            let tag = load_sum_tag(ctx, sum);
            let then_label = ctx.new_label();
            let else_label = ctx.new_label();
            let end_label = ctx.new_label();
            let (tag_one, tag_zero) = if sum_pattern_tag(pattern) == 1 {
                (then_label, else_label)
            } else {
                (else_label, then_label)
            };
            ctx.finish_current(Terminator::Branch {
                cond: tag,
                then_bb: tag_one,
                else_bb: tag_zero,
            });

            ctx.start_block(then_label);
            let mut then_vars = entry_env.clone();
            bind_sum_pattern(ctx, pattern, sum, &mut then_vars);
            lower_block_with_live_after(ctx, then_branch, &mut then_vars, live_after);
            ctx.finish_current(Terminator::Jump(end_label));
            let then_idx = ctx.blocks.len() - 1;

            ctx.start_block(else_label);
            let mut else_vars = entry_env.clone();
            if let Some(else_branch) = else_branch {
                lower_block_with_live_after(ctx, else_branch, &mut else_vars, live_after);
            }
            ctx.finish_current(Terminator::Jump(end_label));
            let else_idx = ctx.blocks.len() - 1;

            merge_conditional_envs(
                ctx, &entry_env, &then_vars, &else_vars, then_idx, else_idx, vars,
            );
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
                let value = lower_expr(ctx, e, vars);
                finish_value_return(ctx, value, &e.ty);
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
        Type::Bool => {
            let key = ctx.new_temp();
            ctx.current_instr(Instr::DecodeInt {
                dest: key,
                blob: key_blob,
            });
            Some(key)
        }
        ty if semantic::is_wide_numeric_type(&ty) => {
            let kind = pointer_kind_for_type(&ty)?;
            let key = ctx.new_temp();
            ctx.current_instr(Instr::PointerFromNorito {
                dest: key,
                blob: key_blob,
                kind,
            });
            Some(key)
        }
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

fn lower_expr_as_i64(
    ctx: &mut LowerCtx,
    expr: &TypedExpr,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    if let semantic::ExprKind::IntLiteral(value) = expr.kind() {
        if let Some(value) = value.try_to_i64() {
            return emit_i64_const(ctx, value);
        }
        ctx.record_error(format!(
            "constant `{value}` is outside the signed 64-bit range required by this host boundary"
        ));
        return emit_i64_const(ctx, 0);
    }
    let value = lower_expr(ctx, expr, vars);
    if matches!(semantic::resolve_struct_type(&expr.ty), Type::Int) {
        let out = ctx.new_temp();
        ctx.current_instr(Instr::IntTryToI64 { dest: out, value });
        out
    } else {
        value
    }
}

fn lower_expr_as_u64(
    ctx: &mut LowerCtx,
    expr: &TypedExpr,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    if let semantic::ExprKind::IntLiteral(value) = expr.kind() {
        if let Some(value) = value.try_to_u64() {
            return emit_i64_const(ctx, i64::from_le_bytes(value.to_le_bytes()));
        }
        ctx.record_error(format!(
            "constant `{value}` is outside the unsigned 64-bit range required by this host boundary"
        ));
        return emit_i64_const(ctx, 0);
    }
    let value = lower_expr(ctx, expr, vars);
    if matches!(semantic::resolve_struct_type(&expr.ty), Type::Int) {
        let out = ctx.new_temp();
        ctx.current_instr(Instr::IntTryToU64 { dest: out, value });
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
    if !matches!(semantic::resolve_struct_type(&expr.ty), Type::Quantity) {
        ctx.record_error(format!(
            "ledger quantity boundary received {}; use quantity::try_from_decimal and handle its status first",
            semantic::type_name(&expr.ty)
        ));
    }
    value
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
            let scheme = lower_expr_as_u64(ctx, &args[3], vars);
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
            let tag_len = args.get(4).map(|arg| lower_expr_as_u64(ctx, arg, vars));
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
            let tag_len = args.get(4).map(|arg| lower_expr_as_u64(ctx, arg, vars));
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
            lower_expr_as_i64(ctx, arg, vars)
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
    if let semantic::ExprKind::String(s) = arg.kind() {
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
        && let semantic::ExprKind::Bytes(bytes) = arg.kind()
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
        let amount = amount_raw;
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
        | Builtin::JsonGetDecimalDirect
        | Builtin::JsonGetQuantityDirect
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
        | Builtin::NumericGeDirect
        | Builtin::SetAssetTransferFreeze
        | Builtin::SetAssetTransferDailyLimit
        | Builtin::AccountRecoveryPropose
        | Builtin::AccountRecoveryApprove
        | Builtin::AccountRecoveryCancel
        | Builtin::AccountRecoveryFinalize
        | Builtin::ContractSubject => lower_direct_helper_call(ctx, builtin, args, vars),
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
            if matches!(
                wide_numeric_kind_for_type(&args[0].ty),
                Some(WideNumericKind::Quantity)
            ) {
                ctx.record_error("quantity is non-negative and cannot be negated".into());
                let dest = ctx.new_temp();
                ctx.current_instr(Instr::Const { dest, value: 0 });
                return dest;
            }
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::NumericNeg {
                dest,
                value,
                kind: wide_numeric_kind_for_type(&args[0].ty)
                    .expect("numeric negation operand has an ABI kind"),
            });
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
            let v = lower_expr_as_i64(ctx, &args[2], vars);
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
            let value = lower_expr_as_i64(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::EncodeInt { dest, value });
            dest
        }
        Builtin::DecodeInt => {
            let blob = lower_expr(ctx, &args[0], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::DecodeInt { dest: scalar, blob });
            emit_int_from_i64(ctx, scalar)
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
        builtin @ (Builtin::GetInt | Builtin::GetDecimal | Builtin::GetQuantity) => {
            let j = lower_expr(ctx, &args[0], vars);
            let k = lower_expr(ctx, &args[1], vars);
            let d = ctx.new_temp();
            ctx.current_instr(Instr::JsonGetNumeric {
                dest: d,
                json: j,
                key: k,
                kind: match builtin {
                    Builtin::GetInt => WideNumericKind::Int,
                    Builtin::GetDecimal => WideNumericKind::Decimal,
                    Builtin::GetQuantity => WideNumericKind::Quantity,
                    _ => unreachable!("matched typed numeric JSON getter"),
                },
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
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::CurrentTimeMs { dest: scalar });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::BlockHeight => {
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::BlockHeight { dest: scalar });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::BlockTimeMs => {
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::BlockTimeMs { dest: scalar });
            emit_int_from_u64(ctx, scalar)
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
            if semantic::is_numeric_type(&args[1].ty) || semantic::is_blob_like(&args[1].ty) {
                let key = lower_expr(ctx, &args[1], vars);
                let blob = ctx.new_temp();
                ctx.current_instr(Instr::PointerToNorito {
                    dest: blob,
                    value: key,
                });
                ctx.current_instr(Instr::PathMapKeyNorito {
                    dest: d,
                    base,
                    key_blob: blob,
                });
            } else {
                panic!("path expects a canonical numeric or bytes-like key")
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
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::TlvLen {
                dest: scalar,
                value,
            });
            emit_int_from_u64(ctx, scalar)
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
            ctx.current_instr(Instr::NumericConvert {
                dest,
                value,
                source: wide_numeric_kind_for_type(&args[0].ty)
                    .expect("numeric conversion operand has an ABI kind"),
                destination: WideNumericKind::Int,
            });
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
                left_kind: wide_numeric_kind_for_type(&args[0].ty)
                    .expect("numeric binary left operand has an ABI kind"),
                right_kind: wide_numeric_kind_for_type(&args[1].ty)
                    .expect("numeric binary right operand has an ABI kind"),
                result_kind: wide_numeric_kind_for_type(&args[0].ty)
                    .expect("numeric binary result has an ABI kind"),
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
                kind: wide_numeric_kind_for_type(&args[0].ty)
                    .expect("numeric comparison operand has a nominal ABI kind"),
            });
            dest
        }
        Builtin::WrappingNeg => {
            let operand = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::WrappingNeg { dest, operand });
            dest
        }
        builtin @ (Builtin::WrappingAdd | Builtin::WrappingSub | Builtin::WrappingMul) => {
            let left = lower_expr(ctx, &args[0], vars);
            let right = lower_expr(ctx, &args[1], vars);
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
            let src = lower_expr_as_u64(ctx, &args[0], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::Isqrt { dest: scalar, src });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::Abs => {
            let src = lower_expr_as_i64(ctx, &args[0], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::Abs { dest: scalar, src });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::Min => {
            let a = lower_expr_as_i64(ctx, &args[0], vars);
            let b = lower_expr_as_i64(ctx, &args[1], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::Min { dest: scalar, a, b });
            emit_int_from_i64(ctx, scalar)
        }
        Builtin::Max => {
            let a = lower_expr_as_i64(ctx, &args[0], vars);
            let b = lower_expr_as_i64(ctx, &args[1], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::Max { dest: scalar, a, b });
            emit_int_from_i64(ctx, scalar)
        }
        Builtin::DivCeil => {
            let num = lower_expr_as_i64(ctx, &args[0], vars);
            let denom = lower_expr_as_i64(ctx, &args[1], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::DivCeil {
                dest: scalar,
                num,
                denom,
            });
            emit_int_from_i64(ctx, scalar)
        }
        Builtin::Gcd => {
            let a = lower_expr_as_i64(ctx, &args[0], vars);
            let b = lower_expr_as_i64(ctx, &args[1], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::Gcd { dest: scalar, a, b });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::Mean => {
            let a = lower_expr_as_i64(ctx, &args[0], vars);
            let b = lower_expr_as_i64(ctx, &args[1], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::Mean { dest: scalar, a, b });
            emit_int_from_i64(ctx, scalar)
        }
        Builtin::Poseidon2 => {
            let a = lower_expr_as_u64(ctx, &args[0], vars);
            let b = lower_expr_as_u64(ctx, &args[1], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::Poseidon2 { dest: scalar, a, b });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::Poseidon6 => {
            let mut lowered_args = [Temp(0); 6];
            for (index, arg) in args.iter().enumerate() {
                lowered_args[index] = lower_expr_as_u64(ctx, arg, vars);
            }
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::Poseidon6 {
                dest: scalar,
                args: lowered_args,
            });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::Pubkgen => {
            let src = lower_expr_as_u64(ctx, &args[0], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::Pubkgen { dest: scalar, src });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::Valcom => {
            let value = lower_expr_as_u64(ctx, &args[0], vars);
            let blind = lower_expr_as_u64(ctx, &args[1], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::Valcom {
                dest: scalar,
                value,
                blind,
            });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::SetVl => {
            let value = lower_expr_as_u64(ctx, &args[0], vars);
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
            let offset = lower_expr_as_u64(ctx, &args[1], vars);
            let limit = lower_expr_as_u64(ctx, &args[2], vars);
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
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::StateLen { dest: scalar, path });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::StateCount => {
            let prefix = lower_expr(ctx, &args[0], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::StateCount {
                dest: scalar,
                prefix,
            });
            emit_int_from_u64(ctx, scalar)
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
        | Builtin::QueryGetNft => {
            let key = lower_expr(ctx, &args[0], vars);
            let entity = match builtin {
                Builtin::QueryGetAccount => ivm_abi::core_query::CoreQueryEntityTagV1::Account,
                Builtin::QueryGetAsset => ivm_abi::core_query::CoreQueryEntityTagV1::Asset,
                Builtin::QueryGetAssetDefinition => {
                    ivm_abi::core_query::CoreQueryEntityTagV1::AssetDefinition
                }
                Builtin::QueryGetDomain => ivm_abi::core_query::CoreQueryEntityTagV1::Domain,
                Builtin::QueryGetNft => ivm_abi::core_query::CoreQueryEntityTagV1::Nft,
                _ => unreachable!("matched only V1 core-query builtins"),
            };
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::CoreQueryGet { dest, key, entity });
            dest
        }
        Builtin::QueryPageAccounts
        | Builtin::QueryPageAssets
        | Builtin::QueryPageAssetDefinitions
        | Builtin::QueryPageDomains
        | Builtin::QueryPageNfts => {
            let offset = lower_expr_as_u64(ctx, &args[0], vars);
            let limit = lower_expr_as_u64(ctx, &args[1], vars);
            let entity = match builtin {
                Builtin::QueryPageAccounts => ivm_abi::core_query::CoreQueryEntityTagV1::Account,
                Builtin::QueryPageAssets => ivm_abi::core_query::CoreQueryEntityTagV1::Asset,
                Builtin::QueryPageAssetDefinitions => {
                    ivm_abi::core_query::CoreQueryEntityTagV1::AssetDefinition
                }
                Builtin::QueryPageDomains => ivm_abi::core_query::CoreQueryEntityTagV1::Domain,
                Builtin::QueryPageNfts => ivm_abi::core_query::CoreQueryEntityTagV1::Nft,
                _ => unreachable!("matched only V1 core-query page builtins"),
            };
            let items_dest = ctx.new_temp();
            let next_offset_dest = ctx.new_temp();
            ctx.current_instr(Instr::CoreQueryPage {
                items_dest,
                next_offset_dest,
                entity,
                offset,
                limit,
            });
            let page = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack {
                dest: page,
                items: vec![items_dest, next_offset_dest],
            });
            page
        }
        Builtin::QueryGetParameter
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
            // The protocol field is `u128`, so retain the canonical source
            // `int` pointer here. Code generation requires a literal and
            // performs the explicit non-negative u128 conversion.
            let amount = lower_expr(ctx, &args[2], vars);
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
            let value = lower_expr_as_i64(ctx, &args[0], vars);
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
            let code = lower_expr_as_u64(ctx, &args[1], vars);
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
                let value = lower_expr_as_i64(ctx, &args[0], vars);
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
            let equal = ctx.new_temp();
            ctx.current_instr(Instr::NumericCompare {
                dest: equal,
                op: BinaryOp::Eq,
                left,
                right,
                kind: WideNumericKind::Int,
            });
            ctx.current_instr(Instr::Assert { cond: equal });
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
            let amt = match args[2].kind() {
                semantic::ExprKind::IntLiteral(value)
                    if value.is_zero() && !semantic::is_wide_numeric_type(&args[2].ty) =>
                {
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
            let quantity = lower_expr_as_u64(ctx, &args[2], vars);
            let mintable = lower_expr_as_u64(ctx, &args[3], vars);
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
            let quantity = lower_expr_as_u64(ctx, &args[2], vars);
            let account = lower_expr(ctx, &args[3], vars);
            let mintable = lower_expr_as_u64(ctx, &args[4], vars);
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
            let enabled = lower_expr_as_u64(ctx, &args[1], vars);
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
        Builtin::GrantContractEntrypoint => {
            let account = lower_expr(ctx, &args[0], vars);
            let entrypoint = lower_expr(ctx, &args[1], vars);
            ctx.current_instr(Instr::GrantContractEntrypoint {
                account,
                entrypoint,
            });
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            t
        }
        Builtin::RevokeContractEntrypoint => {
            let account = lower_expr(ctx, &args[0], vars);
            let entrypoint = lower_expr(ctx, &args[1], vars);
            ctx.current_instr(Instr::RevokeContractEntrypoint {
                account,
                entrypoint,
            });
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
            let bytes = lower_expr_as_u64(ctx, &args[0], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::Alloc {
                dest: scalar,
                bytes,
            });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::GetPrivateInput => {
            let index = lower_expr_as_u64(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::GetPrivateInput { dest, index });
            dest
        }
        Builtin::UseNullifier => {
            let nullifier = lower_expr_as_u64(ctx, &args[0], vars);
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
            let value = lower_expr_as_u64(ctx, &args[0], vars);
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
            let request = lower_expr(ctx, &args[0], vars);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::VrfVerify { dest, request });
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
            let bytes = lower_expr_as_u64(ctx, &args[0], vars);
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::GrowHeap {
                dest: scalar,
                bytes,
            });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::GetMerklePath => {
            let address = lower_expr_as_u64(ctx, &args[0], vars);
            let output = lower_expr_as_u64(ctx, &args[1], vars);
            let root_output = args.get(2).map(|arg| lower_expr_as_u64(ctx, arg, vars));
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::GetMerklePath {
                dest: scalar,
                address,
                output,
                root_output,
            });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::GetMerkleCompact => {
            let address = lower_expr_as_u64(ctx, &args[0], vars);
            let output = lower_expr_as_u64(ctx, &args[1], vars);
            let max_depth = args.get(2).map(|arg| lower_expr_as_u64(ctx, arg, vars));
            let root_output = args.get(3).map(|arg| lower_expr_as_u64(ctx, arg, vars));
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::GetMerkleCompact {
                dest: scalar,
                address,
                output,
                max_depth,
                root_output,
            });
            emit_int_from_u64(ctx, scalar)
        }
        Builtin::GetRegisterMerkleCompact => {
            let register_index = lower_expr_as_u64(ctx, &args[0], vars);
            let output = lower_expr_as_u64(ctx, &args[1], vars);
            let max_depth = args.get(2).map(|arg| lower_expr_as_u64(ctx, arg, vars));
            let root_output = args.get(3).map(|arg| lower_expr_as_u64(ctx, arg, vars));
            let scalar = ctx.new_temp();
            ctx.current_instr(Instr::GetRegisterMerkleCompact {
                dest: scalar,
                register_index,
                output,
                max_depth,
                root_output,
            });
            emit_int_from_u64(ctx, scalar)
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
            let quorum = lower_expr_as_u64(ctx, &args[1], vars);
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
                let result_words = runtime_value_word_types(&spec.value)
                    .into_iter()
                    .map(|_| ctx.new_temp())
                    .collect::<Vec<_>>();
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
                copy_runtime_value_words(ctx, decoded, &spec.value, &result_words);
                ctx.finish_current(Terminator::Jump(end_bb));
                ctx.start_block(else_bb);
                let def = lower_expr(ctx, dexpr, vars);
                copy_runtime_value_words(ctx, def, &spec.value, &result_words);
                ctx.finish_current(Terminator::Jump(end_bb));
                ctx.start_block(end_bb);
                return rebuild_runtime_value(ctx, &spec.value, &result_words);
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
                let result_words = runtime_value_word_types(&spec.value)
                    .into_iter()
                    .map(|_| ctx.new_temp())
                    .collect::<Vec<_>>();
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
                copy_runtime_value_words(ctx, existing, &spec.value, &result_words);
                ctx.finish_current(Terminator::Jump(end_bb));
                ctx.start_block(else_bb);
                let def = lower_expr(ctx, dexpr, vars);
                copy_runtime_value_words(ctx, def, &spec.value, &result_words);
                ctx.finish_current(Terminator::Jump(end_bb));
                ctx.start_block(end_bb);
                return rebuild_runtime_value(ctx, &spec.value, &result_words);
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
                let result_words = runtime_value_word_types(&spec.value)
                    .into_iter()
                    .map(|_| ctx.new_temp())
                    .collect::<Vec<_>>();
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
                copy_runtime_value_words(ctx, existing, &spec.value, &result_words);
                ctx.finish_current(Terminator::Jump(end_bb));
                ctx.start_block(else_bb);
                let def = lower_expr(ctx, dexpr, vars);
                let _ = lower_state_map_set_value(ctx, &bn, key_tmp, &spec.key, &spec.value, def);
                copy_runtime_value_words(ctx, def, &spec.value, &result_words);
                ctx.finish_current(Terminator::Jump(end_bb));
                ctx.start_block(end_bb);
                return rebuild_runtime_value(ctx, &spec.value, &result_words);
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
            let start_t = lower_expr_as_u64(ctx, &args[1], vars);
            let which_t = lower_expr_as_u64(ctx, &args[2], vars);
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
            let start_t = lower_expr_as_u64(ctx, &args[1], vars);
            let which_t = lower_expr_as_u64(ctx, &args[2], vars);
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

/// Return whether a named argument must be captured at its source position.
///
/// Literal leaves are total and independent of mutable compiler state, so a
/// builtin may continue to consume them directly (and, for literal-only ABI
/// fields, entirely at compile time). Every other value is captured exactly
/// once before ABI-slot permutation. `StateMap` handles are declarations, not
/// runtime values, and their lowering is performed by the callee ABI helper.
fn named_argument_requires_capture(argument: &TypedExpr) -> bool {
    if matches!(
        semantic::resolve_struct_type(&argument.ty),
        Type::StateMap(_, _)
    ) {
        return false;
    }
    !matches!(
        argument.kind(),
        semantic::ExprKind::IntLiteral(_)
            | semantic::ExprKind::DecimalLiteral { .. }
            | semantic::ExprKind::Bool(_)
            | semantic::ExprKind::String(_)
            | semantic::ExprKind::Bytes(_)
    )
}

/// Lower a named call by capturing observable argument evaluations in source
/// order and then reusing their temporary identities in declaration/ABI order.
///
/// The cache is compiler-only: it emits no copies, tuple shuffles, or heap
/// allocations. Literal-only operands remain available to builtin lowering so
/// compile-time ABI fields retain the exact code generated by positional calls.
fn lower_named_call(
    ctx: &mut LowerCtx,
    result_ty: &Type,
    name: &str,
    args: &[TypedExpr],
    evaluation_order: &[usize],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let mut seen = vec![false; args.len()];
    let mut captured = vec![None; args.len()];
    for &index in evaluation_order {
        if index >= args.len() || seen[index] {
            ctx.record_error(format!(
                "internal error: named call `{name}` has an invalid source evaluation order"
            ));
            continue;
        }
        seen[index] = true;
        if named_argument_requires_capture(&args[index]) {
            captured[index] = Some(lower_expr(ctx, &args[index], vars));
        }
    }
    for (index, was_seen) in seen.iter().copied().enumerate() {
        if !was_seen {
            ctx.record_error(format!(
                "internal error: named call `{name}` omits ABI argument slot {index} from its source evaluation order"
            ));
            if named_argument_requires_capture(&args[index]) {
                captured[index] = Some(lower_expr(ctx, &args[index], vars));
            }
        }
    }

    // Re-enter the ordinary call lowering path with the validated ABI-ordered
    // expressions. The Vec allocation belongs to the compiler only; its
    // backing storage stays stable while the scoped pointer-to-temp cache is
    // active.
    let positional = TypedExpr {
        expr: semantic::ExprKind::Call {
            name: name.to_owned(),
            args: args.to_vec(),
        },
        ty: result_ty.clone(),
    };
    let positional_args = match positional.kind() {
        semantic::ExprKind::Call { args, .. } => args,
        _ => unreachable!("constructed a positional call"),
    };
    let scope = captured
        .into_iter()
        .enumerate()
        .filter_map(|(index, temp)| {
            temp.map(|temp| (std::ptr::from_ref(&positional_args[index]), temp))
        })
        .collect();
    ctx.prelowered_argument_scopes.push(scope);
    let value = lower_expr(ctx, &positional, vars);
    ctx.prelowered_argument_scopes
        .pop()
        .expect("named argument cache scope must be balanced");
    value
}

fn lower_expr(ctx: &mut LowerCtx, expr: &TypedExpr, vars: &mut HashMap<String, Temp>) -> Temp {
    if let Some(temp) = ctx.prelowered_argument(expr) {
        return temp;
    }
    match &expr.expr {
        semantic::ExprKind::JsonObject(_) | semantic::ExprKind::JsonArray(_) => {
            lower_json_construction(ctx, expr, vars)
        }
        semantic::ExprKind::StructLiteral { fields, .. } => {
            let lowered = fields
                .iter()
                .map(|(name, value)| (name.as_str(), lower_expr(ctx, value, vars)))
                .collect::<Vec<_>>();
            let semantic::Type::Struct {
                fields: declared_fields,
                ..
            } = &expr.ty
            else {
                ctx.record_error("named struct literal lost its declared field layout".into());
                let value = ctx.new_temp();
                ctx.current_instr(Instr::Const {
                    dest: value,
                    value: 0,
                });
                return value;
            };
            let mut items = Vec::with_capacity(declared_fields.len());
            for (declared_name, _) in declared_fields {
                let Some((_, value)) = lowered
                    .iter()
                    .find(|(source_name, _)| *source_name == declared_name.as_str())
                else {
                    ctx.record_error(format!(
                        "named struct literal is missing lowered field `{declared_name}`"
                    ));
                    let placeholder = ctx.new_temp();
                    ctx.current_instr(Instr::Const {
                        dest: placeholder,
                        value: 0,
                    });
                    items.push(placeholder);
                    continue;
                };
                items.push(*value);
            }
            let value = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack { dest: value, items });
            value
        }
        semantic::ExprKind::Tuple(elems) => {
            let mut items = Vec::with_capacity(elems.len());
            for e in elems {
                items.push(lower_expr(ctx, e, vars));
            }
            let tup = ctx.new_temp();
            ctx.current_instr(Instr::TuplePack { dest: tup, items });
            tup
        }
        semantic::ExprKind::List(elements) => lower_list_literal(ctx, elements, &expr.ty, vars),
        semantic::ExprKind::ListComprehension {
            expression,
            item,
            source,
            condition,
        } => lower_list_comprehension(
            ctx,
            expression,
            item,
            source,
            condition.as_deref(),
            &expr.ty,
            vars,
        ),
        semantic::ExprKind::IntLiteral(n) => {
            let t = ctx.new_temp();
            ctx.current_instr(Instr::DataRef {
                dest: t,
                kind: DataRefKind::Int,
                value: n.to_string(),
            });
            t
        }
        semantic::ExprKind::DecimalLiteral { value, .. } => {
            let t = ctx.new_temp();
            let kind = pointer_kind_for_type(&expr.ty)
                .filter(|kind| matches!(kind, DataRefKind::Decimal | DataRefKind::Quantity))
                .unwrap_or_else(|| {
                    ctx.record_error(format!(
                        "decimal literal reached lowering with non-decimal type {}",
                        semantic::type_name(&expr.ty)
                    ));
                    DataRefKind::Decimal
                });
            ctx.current_instr(Instr::DataRef {
                dest: t,
                kind,
                value: value.to_string(),
            });
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
            let v = lower_expr(ctx, inner, vars);
            if matches!(op, UnaryOp::Neg) && semantic::is_wide_numeric_type(&inner.ty) {
                let kind = wide_numeric_kind_for_type(&inner.ty)
                    .expect("wide numeric unary operand has a nominal ABI kind");
                if kind == WideNumericKind::Quantity {
                    ctx.record_error("quantity is non-negative and cannot be negated".into());
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    return t;
                }
                let t = ctx.new_temp();
                ctx.current_instr(Instr::NumericNeg {
                    dest: t,
                    value: v,
                    kind,
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
            if src_ty != dst_ty
                && semantic::is_wide_numeric_type(&src_ty)
                && semantic::is_wide_numeric_type(&dst_ty)
            {
                let t = ctx.new_temp();
                ctx.current_instr(Instr::NumericConvert {
                    dest: t,
                    value: v,
                    source: wide_numeric_kind_for_type(&src_ty)
                        .expect("numeric cast source has an ABI kind"),
                    destination: wide_numeric_kind_for_type(&dst_ty)
                        .expect("numeric cast destination has an ABI kind"),
                });
                return t;
            }
            v
        }
        semantic::ExprKind::NumericTryCast { expr: inner } => {
            let value = lower_expr(ctx, inner, vars);
            let source = wide_numeric_kind_for_type(&inner.ty)
                .expect("recoverable numeric cast source has an ABI kind");
            let Type::Result(ok_type, error_type) = semantic::resolve_struct_type(&expr.ty) else {
                ctx.record_error(
                    "internal error: recoverable numeric cast has non-Result type".into(),
                );
                return emit_i64_const(ctx, 0);
            };
            if semantic::resolve_struct_type(&error_type) != Type::Int {
                ctx.record_error("internal error: numeric fault payload must be int".into());
                return emit_i64_const(ctx, 0);
            }
            let destination = wide_numeric_kind_for_type(&ok_type)
                .expect("recoverable numeric cast destination has an ABI kind");
            let converted = ctx.new_temp();
            let status = ctx.new_temp();
            ctx.current_instr(Instr::NumericTryConvert {
                dest: converted,
                value,
                source,
                destination,
            });
            ctx.current_instr(Instr::NumericStatus { dest: status });
            let zero = emit_i64_const(ctx, 0);
            let succeeded = ctx.new_temp();
            ctx.current_instr(Instr::Binary {
                dest: succeeded,
                op: BinaryOp::Eq,
                left: status,
                right: zero,
            });
            let success = ctx.new_label();
            let failure = ctx.new_label();
            let end = ctx.new_label();
            let result = ctx.new_temp();
            ctx.finish_current(Terminator::Branch {
                cond: succeeded,
                then_bb: success,
                else_bb: failure,
            });

            ctx.start_block(success);
            let ok = emit_sum_value(ctx, &expr.ty, 1, Some(converted));
            ctx.current_instr(Instr::Copy {
                dest: result,
                src: ok,
            });
            ctx.finish_current(Terminator::Jump(end));

            ctx.start_block(failure);
            let fault = ctx.new_temp();
            ctx.current_instr(Instr::IntFromU64 {
                dest: fault,
                value: status,
            });
            let error = emit_sum_value(ctx, &expr.ty, 0, Some(fault));
            ctx.current_instr(Instr::Copy {
                dest: result,
                src: error,
            });
            ctx.finish_current(Terminator::Jump(end));
            ctx.start_block(end);
            result
        }
        semantic::ExprKind::Binary { op, left, right } => {
            if matches!(op, BinaryOp::And | BinaryOp::Or) {
                return lower_short_circuit_bool(ctx, *op, left, right, vars);
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
                    left_kind: wide_numeric_kind_for_type(&left.ty)
                        .expect("numeric binary left operand has an ABI kind"),
                    right_kind: wide_numeric_kind_for_type(&right.ty)
                        .expect("numeric binary right operand has an ABI kind"),
                    result_kind: wide_numeric_kind_for_type(&expr.ty)
                        .expect("numeric binary result has an ABI kind"),
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
                    kind: wide_numeric_kind_for_type(&left.ty)
                        .or_else(|| wide_numeric_kind_for_type(&right.ty))
                        .expect("wide numeric comparison has a nominal ABI kind"),
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
            let cond_t = lower_expr(ctx, cond, vars);
            let then_label = ctx.new_label();
            let else_label = ctx.new_label();
            let end_label = ctx.new_label();
            let result_words = runtime_value_word_types(&expr.ty)
                .into_iter()
                .map(|_| ctx.new_temp())
                .collect::<Vec<_>>();
            ctx.finish_current(Terminator::Branch {
                cond: cond_t,
                then_bb: then_label,
                else_bb: else_label,
            });

            ctx.start_block(then_label);
            let then_v = lower_expr(ctx, then_expr, &mut vars.clone());
            copy_runtime_value_words(ctx, then_v, &expr.ty, &result_words);
            ctx.finish_current(Terminator::Jump(end_label));

            ctx.start_block(else_label);
            let else_v = lower_expr(ctx, else_expr, &mut vars.clone());
            copy_runtime_value_words(ctx, else_v, &expr.ty, &result_words);
            ctx.finish_current(Terminator::Jump(end_label));

            ctx.start_block(end_label);
            rebuild_runtime_value(ctx, &expr.ty, &result_words)
        }
        semantic::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let condition = lower_expr(ctx, condition, vars);
            let then_label = ctx.new_label();
            let else_label = ctx.new_label();
            let end_label = ctx.new_label();
            let result_words = runtime_value_word_types(&expr.ty)
                .into_iter()
                .map(|_| ctx.new_temp())
                .collect::<Vec<_>>();
            ctx.finish_current(Terminator::Branch {
                cond: condition,
                then_bb: then_label,
                else_bb: else_label,
            });

            ctx.start_block(then_label);
            let then_value = lower_expression_block(ctx, then_branch, &mut vars.clone());
            copy_runtime_value_words(ctx, then_value, &expr.ty, &result_words);
            ctx.finish_current(Terminator::Jump(end_label));

            ctx.start_block(else_label);
            let else_value = lower_expression_block(ctx, else_branch, &mut vars.clone());
            copy_runtime_value_words(ctx, else_value, &expr.ty, &result_words);
            ctx.finish_current(Terminator::Jump(end_label));

            ctx.start_block(end_label);
            rebuild_runtime_value(ctx, &expr.ty, &result_words)
        }
        semantic::ExprKind::IfLet {
            pattern,
            value,
            then_branch,
            else_branch,
        } => {
            let sum = lower_expr(ctx, value, vars);
            let tag = load_sum_tag(ctx, sum);
            let then_label = ctx.new_label();
            let else_label = ctx.new_label();
            let end_label = ctx.new_label();
            let result_words = runtime_value_word_types(&expr.ty)
                .into_iter()
                .map(|_| ctx.new_temp())
                .collect::<Vec<_>>();
            let (tag_one, tag_zero) = if sum_pattern_tag(pattern) == 1 {
                (then_label, else_label)
            } else {
                (else_label, then_label)
            };
            ctx.finish_current(Terminator::Branch {
                cond: tag,
                then_bb: tag_one,
                else_bb: tag_zero,
            });

            ctx.start_block(then_label);
            let mut then_vars = vars.clone();
            bind_sum_pattern(ctx, pattern, sum, &mut then_vars);
            let then_value = lower_expression_block(ctx, then_branch, &mut then_vars);
            copy_runtime_value_words(ctx, then_value, &expr.ty, &result_words);
            ctx.finish_current(Terminator::Jump(end_label));

            ctx.start_block(else_label);
            let else_value = lower_expression_block(ctx, else_branch, &mut vars.clone());
            copy_runtime_value_words(ctx, else_value, &expr.ty, &result_words);
            ctx.finish_current(Terminator::Jump(end_label));

            ctx.start_block(end_label);
            rebuild_runtime_value(ctx, &expr.ty, &result_words)
        }
        semantic::ExprKind::Match { value, arms } => {
            // Normalize the canonical exhaustive early-return spelling of
            // same-family propagation before CFG construction. This keeps
            // postfix `?` genuine zero-cost syntax sugar: both source forms
            // enter the exact same lowering helper and therefore produce the
            // same optimized IR and executable bytes.
            if propagation_match_return(&value.ty, &ctx.function_return_type, arms).is_some() {
                return lower_propagation(ctx, value, vars);
            }
            let sum = lower_expr(ctx, value, vars);
            let tag = load_sum_tag(ctx, sum);
            let tag_one_arm = arms
                .iter()
                .find(|arm| sum_pattern_tag(&arm.pattern) == 1)
                .expect("semantic analysis guarantees the tag-one arm");
            let tag_zero_arm = arms
                .iter()
                .find(|arm| sum_pattern_tag(&arm.pattern) == 0)
                .expect("semantic analysis guarantees the tag-zero arm");
            let tag_one_label = ctx.new_label();
            let tag_zero_label = ctx.new_label();
            let end_label = ctx.new_label();
            let result_words = runtime_value_word_types(&expr.ty)
                .into_iter()
                .map(|_| ctx.new_temp())
                .collect::<Vec<_>>();
            ctx.finish_current(Terminator::Branch {
                cond: tag,
                then_bb: tag_one_label,
                else_bb: tag_zero_label,
            });

            ctx.start_block(tag_one_label);
            let mut tag_one_vars = vars.clone();
            bind_sum_pattern(ctx, &tag_one_arm.pattern, sum, &mut tag_one_vars);
            let tag_one_value = lower_expression_block(ctx, &tag_one_arm.body, &mut tag_one_vars);
            copy_runtime_value_words(ctx, tag_one_value, &expr.ty, &result_words);
            ctx.finish_current(Terminator::Jump(end_label));

            ctx.start_block(tag_zero_label);
            let mut tag_zero_vars = vars.clone();
            bind_sum_pattern(ctx, &tag_zero_arm.pattern, sum, &mut tag_zero_vars);
            let tag_zero_value =
                lower_expression_block(ctx, &tag_zero_arm.body, &mut tag_zero_vars);
            copy_runtime_value_words(ctx, tag_zero_value, &expr.ty, &result_words);
            ctx.finish_current(Terminator::Jump(end_label));

            ctx.start_block(end_label);
            rebuild_runtime_value(ctx, &expr.ty, &result_words)
        }
        semantic::ExprKind::OptionSome { value } => {
            let payload = lower_expr(ctx, value, vars);
            emit_sum_value(ctx, &expr.ty, 1, Some(payload))
        }
        semantic::ExprKind::OptionNone => emit_sum_value(ctx, &expr.ty, 0, None),
        semantic::ExprKind::ResultOk { value } => {
            let payload = lower_expr(ctx, value, vars);
            emit_sum_value(ctx, &expr.ty, 1, Some(payload))
        }
        semantic::ExprKind::ResultErr { error } => {
            let payload = lower_expr(ctx, error, vars);
            emit_sum_value(ctx, &expr.ty, 0, Some(payload))
        }
        semantic::ExprKind::Propagate { value } => lower_propagation(ctx, value, vars),
        semantic::ExprKind::NamedCall {
            name,
            args,
            evaluation_order,
        } => lower_named_call(ctx, &expr.ty, name, args, evaluation_order, vars),
        semantic::ExprKind::Call { name, args } => {
            if let Some(value) = lower_numeric_round_intrinsic(ctx, name, args, vars) {
                return value;
            }
            if let Some(value) = lower_decimal_to_int_intrinsic(ctx, name, args, vars) {
                return value;
            }
            if let Some(value) = lower_list_intrinsic(ctx, name, args, &expr.ty, vars) {
                return value;
            }
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
                    let actor = match args[0].kind() {
                        semantic::ExprKind::String(value) => lower_blob_literal(ctx, value),
                        _ => panic!("invoke_entrypoint_as actor must be a literal string"),
                    };
                    let entrypoint = match args[1].kind() {
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
                        aggregate if function_value_word_types(aggregate).is_some() => {
                            let word_types = function_value_word_types(aggregate)
                                .expect("aggregate return word types checked");
                            let dests: Vec<Temp> =
                                word_types.iter().map(|_| ctx.new_temp()).collect();
                            let mut return_pointer_mask = 0u64;
                            for (idx, item_ty) in word_types.iter().enumerate() {
                                if runtime_word_is_pointer(item_ty) {
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
                            let mut index = 0_usize;
                            let value = rebuild_function_value_from_words(
                                ctx, aggregate, &dests, &mut index,
                            )
                            .expect("validated aggregate return must rebuild from ABI words");
                            debug_assert_eq!(index, dests.len());
                            value
                        }
                        _ => {
                            let dest = ctx.new_temp();
                            ctx.current_instr(Instr::InvokeEntrypointAs {
                                dest: Some(dest),
                                actor,
                                entrypoint,
                                payload,
                                returns_pointer: runtime_word_is_pointer(&expr.ty),
                            });
                            dest
                        }
                    }
                }
                "expect_reject_as" => {
                    let actor = match args[0].kind() {
                        semantic::ExprKind::String(value) => lower_blob_literal(ctx, value),
                        _ => panic!("expect_reject_as actor must be a literal string"),
                    };
                    let entrypoint = match args[1].kind() {
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
                    let actor = match args[0].kind() {
                        semantic::ExprKind::String(value) => lower_blob_literal(ctx, value),
                        _ => panic!("actor_account actor must be a literal string"),
                    };
                    let dest = ctx.new_temp();
                    ctx.current_instr(Instr::ActorAccount { dest, actor });
                    dest
                }
                "actor_public_key" => {
                    let actor = match args[0].kind() {
                        semantic::ExprKind::String(value) => lower_blob_literal(ctx, value),
                        _ => panic!("actor_public_key actor must be a literal string"),
                    };
                    let dest = ctx.new_temp();
                    ctx.current_instr(Instr::ActorPublicKey { dest, actor });
                    dest
                }
                "actor_sign" => {
                    let actor = match args[0].kind() {
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
                        aggregate if function_value_word_types(aggregate).is_some() => {
                            let word_types = function_value_word_types(aggregate)
                                .expect("aggregate return word types checked");
                            // Multi-return: move the flattened ABI words from r10 onward, then
                            // rebuild the compiler-only aggregate shape.
                            let mut words = Vec::with_capacity(word_types.len());
                            for _ in word_types {
                                words.push(ctx.new_temp());
                            }
                            ctx.current_instr(Instr::CallMulti {
                                callee: ctx.call_target(name),
                                args: arg_tmps,
                                dests: words.clone(),
                            });
                            let mut index = 0_usize;
                            let value = rebuild_function_value_from_words(
                                ctx, aggregate, &words, &mut index,
                            )
                            .expect("validated aggregate return must rebuild from ABI words");
                            debug_assert_eq!(index, words.len());
                            value
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
                match e.kind() {
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
    Some(match name {
        STATE_MAP_GET_INTRINSIC => lower_state_map_get_option(ctx, args, vars),
        "is_some" | "is_ok" => {
            let tagged_value = lower_expr(ctx, &args[0], vars);
            load_sum_tag(ctx, tagged_value)
        }
        "is_none" | "is_err" => {
            let tagged_value = lower_expr(ctx, &args[0], vars);
            let tag = load_sum_tag(ctx, tagged_value);
            let inverted = ctx.new_temp();
            ctx.current_instr(Instr::Unary {
                dest: inverted,
                op: UnaryOp::Not,
                operand: tag,
            });
            inverted
        }
        "unwrap_or" => lower_tagged_unwrap(ctx, &args[0], &args[1], true, vars),
        "unwrap_err_or" => lower_tagged_unwrap(ctx, &args[0], &args[1], false, vars),
        _ => return None,
    })
}

fn lower_tagged_unwrap(
    ctx: &mut LowerCtx,
    tagged_expr: &semantic::TypedExpr,
    fallback_expr: &semantic::TypedExpr,
    payload_when_tagged: bool,
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let tagged = lower_expr(ctx, tagged_expr, vars);
    // `unwrap_or` is eager (the lazy spelling would be `unwrap_or_else`, which
    // V1 does not expose), so capture the fallback before branching. This also
    // keeps positional and named-call evaluation semantics identical.
    let fallback = lower_expr(ctx, fallback_expr, vars);
    let tag = load_sum_tag(ctx, tagged);
    let payload_block = ctx.new_label();
    let fallback_block = ctx.new_label();
    let end_block = ctx.new_label();
    let (then_bb, else_bb) = if payload_when_tagged {
        (payload_block, fallback_block)
    } else {
        (fallback_block, payload_block)
    };
    ctx.finish_current(Terminator::Branch {
        cond: tag,
        then_bb,
        else_bb,
    });

    let payload_ty = semantic::resolve_struct_type(&fallback_expr.ty);
    let result_words = runtime_value_word_types(&payload_ty)
        .into_iter()
        .map(|_| ctx.new_temp())
        .collect::<Vec<_>>();
    ctx.start_block(payload_block);
    let payload = load_sum_payload(ctx, tagged, &payload_ty);
    copy_runtime_value_words(ctx, payload, &payload_ty, &result_words);
    ctx.finish_current(Terminator::Jump(end_block));

    ctx.start_block(fallback_block);
    copy_runtime_value_words(ctx, fallback, &payload_ty, &result_words);
    ctx.finish_current(Terminator::Jump(end_block));
    ctx.start_block(end_block);
    rebuild_runtime_value(ctx, &payload_ty, &result_words)
}

/// Decode a present durable value and materialize the selected `Option` arm.
///
/// The absent branch allocates only a tag-bearing `Option::none`; it never
/// constructs or evaluates a value payload.
fn lower_present_or_inactive_state_value(
    ctx: &mut LowerCtx,
    blob: Temp,
    present: Temp,
    value_ty: &Type,
    delete_when_present: Option<Temp>,
) -> Option<Temp> {
    let resolved = semantic::resolve_struct_type(value_ty);
    state_value_schema(&resolved)?.word_kinds()?;
    let option_ty = Type::Option(Box::new(resolved.clone()));
    let result = ctx.new_temp();
    let present_block = ctx.new_label();
    let absent_block = ctx.new_label();
    let end_block = ctx.new_label();
    ctx.finish_current(Terminator::Branch {
        cond: present,
        then_bb: present_block,
        else_bb: absent_block,
    });

    ctx.start_block(present_block);
    let decoded = decode_aggregate_state_value(ctx, blob, &resolved)?;
    let some = emit_sum_value(ctx, &option_ty, 1, Some(decoded));
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: some,
    });
    if let Some(path) = delete_when_present {
        ctx.current_instr(Instr::StateDel { path });
    }
    ctx.finish_current(Terminator::Jump(end_block));

    ctx.start_block(absent_block);
    let none = emit_sum_value(ctx, &option_ty, 0, None);
    ctx.current_instr(Instr::Copy {
        dest: result,
        src: none,
    });
    ctx.finish_current(Terminator::Jump(end_block));

    ctx.start_block(end_block);
    Some(result)
}

fn state_map_value_type(map: &semantic::TypedExpr) -> Option<Type> {
    match semantic::resolve_struct_type(&map.ty) {
        Type::StateMap(_, value) => Some(*value),
        _ => None,
    }
}

fn lower_state_map_get_option(
    ctx: &mut LowerCtx,
    args: &[semantic::TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let map = &args[0];
    let key = lower_expr(ctx, &args[1], vars);
    let declared_value_ty = state_map_value_type(map).unwrap_or(Type::Unit);
    let Some(base) = state_map_base_name(map) else {
        ctx.record_error("StateMap.get receiver is not a durable state map".into());
        return lower_absent_option(ctx, &declared_value_ty);
    };
    let Some(spec) = ctx.state_map_configs.get(&base).cloned() else {
        ctx.record_error(format!("StateMap.get receiver `{base}` is not declared"));
        return lower_absent_option(ctx, &declared_value_ty);
    };
    let Some(key_codec) = key_codec_for_type(&spec.key) else {
        ctx.record_error("StateMap.get key type is not lowerable".into());
        return lower_absent_option(ctx, &spec.value);
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

    lower_present_or_inactive_state_value(ctx, blob, present, &spec.value, None).unwrap_or_else(
        || {
            ctx.record_error("StateMap.get value type is not lowerable".into());
            lower_absent_option(ctx, &spec.value)
        },
    )
}

fn lower_state_map_remove_option(
    ctx: &mut LowerCtx,
    args: &[semantic::TypedExpr],
    vars: &mut HashMap<String, Temp>,
) -> Temp {
    let map = &args[0];
    let key = lower_expr(ctx, &args[1], vars);
    let declared_value_ty = state_map_value_type(map).unwrap_or(Type::Unit);
    let Some(base) = state_map_base_name(map) else {
        ctx.record_error("StateMap.remove receiver is not a durable state map".into());
        return lower_absent_option(ctx, &declared_value_ty);
    };
    let Some(spec) = ctx.state_map_configs.get(&base).cloned() else {
        ctx.record_error(format!("StateMap.remove receiver `{base}` is not declared"));
        return lower_absent_option(ctx, &declared_value_ty);
    };
    let Some(key_codec) = key_codec_for_type(&spec.key) else {
        ctx.record_error("StateMap.remove key type is not lowerable".into());
        return lower_absent_option(ctx, &spec.value);
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

    lower_present_or_inactive_state_value(ctx, blob, present, &spec.value, Some(path))
        .unwrap_or_else(|| {
            ctx.record_error("StateMap.remove value type is not lowerable".into());
            lower_absent_option(ctx, &spec.value)
        })
}

fn lower_absent_option(ctx: &mut LowerCtx, value_ty: &Type) -> Temp {
    emit_sum_value(
        ctx,
        &Type::Option(Box::new(semantic::resolve_struct_type(value_ty))),
        0,
        None,
    )
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
    /// Declared return type used when `?` must materialize a differently-sized
    /// failure value for the enclosing function.
    function_return_type: Type,
    call_renames: HashMap<String, String>,
    function_param_specs: HashMap<String, Vec<TypedParam>>,
    /// Scoped compiler-only identities for named arguments already evaluated
    /// in source order. Raw pointers are compared only while the owning typed
    /// expression Vec is alive and are never dereferenced.
    prelowered_argument_scopes: Vec<Vec<(*const TypedExpr, Temp)>>,
    error: Option<String>,
}

impl LowerCtx {
    fn new(
        function_return_type: Type,
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
            function_return_type,
            call_renames,
            function_param_specs,
            prelowered_argument_scopes: Vec::new(),
            error: None,
        }
    }

    fn prelowered_argument(&self, expression: &TypedExpr) -> Option<Temp> {
        let identity = std::ptr::from_ref(expression);
        self.prelowered_argument_scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.iter())
            .find_map(|(candidate, temp)| std::ptr::eq(*candidate, identity).then_some(*temp))
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
    }
}

fn lowerable_state_handle_name(ctx: &LowerCtx, expr: &semantic::TypedExpr) -> Option<String> {
    match expr.kind() {
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
    fn source_int_and_internal_scalar_have_an_explicit_ir_boundary() {
        let mut context = LowerCtx::new(Type::Unit, 64, HashMap::new(), HashMap::new());
        let entry = context.new_label();
        context.start_block(entry);
        let scalar = emit_i64_const(&mut context, 17);
        let source_int = emit_int_from_i64(&mut context, scalar);
        context.finish_current(Terminator::Return(Some(source_int)));

        assert!(matches!(
            context.blocks[0].instrs.as_slice(),
            [
                Instr::Const {
                    dest: actual_scalar,
                    value: 17,
                },
                Instr::IntFromI64 {
                    dest: actual_source,
                    value,
                },
            ] if *actual_scalar == scalar && *actual_source == source_int && *value == scalar
        ));

        let mut context = LowerCtx::new(Type::Unit, 64, HashMap::new(), HashMap::new());
        let entry = context.new_label();
        context.start_block(entry);
        let scalar = emit_i64_const(&mut context, -1);
        let source_int = emit_int_from_u64(&mut context, scalar);
        context.finish_current(Terminator::Return(Some(source_int)));
        assert!(matches!(
            context.blocks[0].instrs.last(),
            Some(Instr::IntFromU64 { dest, value })
                if *dest == source_int && *value == scalar
        ));
    }

    #[test]
    fn wide_numeric_state_keys_use_canonical_pointer_norito() {
        assert_eq!(key_codec_for_type(&Type::Bool), Some(KeyCodec::Int));
        for ty in [Type::Int, Type::Decimal, Type::Quantity] {
            assert_eq!(key_codec_for_type(&ty), Some(KeyCodec::Pointer));

            let mut context = LowerCtx::new(Type::Unit, 64, HashMap::new(), HashMap::new());
            let entry = context.new_label();
            context.start_block(entry);
            let blob = emit_i64_const(&mut context, 8);
            let decoded = decode_state_map_key(&mut context, blob, &ty)
                .expect("wide numeric state key must decode");
            context.finish_current(Terminator::Return(Some(decoded)));
            let expected_kind = pointer_kind_for_type(&ty).expect("numeric pointer kind");

            assert!(context.blocks[0].instrs.iter().any(|instruction| {
                matches!(
                    instruction,
                    Instr::PointerFromNorito {
                        dest,
                        blob: actual_blob,
                        kind,
                    } if *dest == decoded && *actual_blob == blob && *kind == expected_kind
                )
            }));
            assert!(
                context.blocks[0]
                    .instrs
                    .iter()
                    .all(|instruction| !matches!(instruction, Instr::DecodeInt { .. }))
            );
        }
    }

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
        let src = "fn add(int a, int b) { let c = a + b; }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        assert_eq!(ir.functions.len(), 1);
        let f = &ir.functions[0];
        assert_eq!(f.blocks.len(), 1); // only entry block
    }

    #[test]
    fn named_calls_evaluate_in_source_order_and_permute_only_temp_references() {
        let src = r#"
            fn first() -> int { 1 }
            fn second() -> int { 2 }
            fn combine(int left, int right) -> int { left * 10 + right }
            fn run() -> int { combine(right: second(), left: first()) }
        "#;
        let typed = analyze(&parse(src).expect("parse named call")).expect("analyze named call");
        let ir = lower(&typed).expect("lower named call");
        let run = ir
            .functions
            .iter()
            .find(|function| function.name == "run")
            .expect("run function");

        let calls = run
            .blocks
            .iter()
            .flat_map(|block| block.instrs.iter())
            .filter_map(|instruction| match instruction {
                Instr::Call { callee, args, dest } => {
                    Some((callee.as_str(), args.as_slice(), *dest))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            calls
                .iter()
                .map(|(callee, _, _)| *callee)
                .collect::<Vec<_>>(),
            ["second", "first", "combine"]
        );
        let second = calls[0].2.expect("second returns a value");
        let first = calls[1].2.expect("first returns a value");
        assert_eq!(calls[2].1, [first, second]);
    }

    #[test]
    fn named_list_intrinsic_evaluates_source_order_before_abi_slots() {
        let source = r#"
            fn index() -> int { 0 }
            fn replacement() -> int { 9 }
            fn mutate() -> bool {
                var List<int, 2> values = [1];
                values.try_set(value: replacement(), index: index())
            }
        "#;
        let lowered = lower(
            &analyze(&parse(source).expect("parse named List intrinsic"))
                .expect("analyze named List intrinsic"),
        )
        .expect("lower named List intrinsic");
        let mutate = lowered
            .functions
            .iter()
            .find(|function| function.name == "mutate")
            .expect("mutate function");
        let calls = mutate
            .blocks
            .iter()
            .flat_map(|block| block.instrs.iter())
            .filter_map(|instruction| match instruction {
                Instr::Call { callee, .. } => Some(callee.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(calls, ["replacement", "index"]);
    }

    #[test]
    fn named_quantity_intrinsic_evaluates_dynamic_arguments_in_source_order() {
        let source = r#"
            fn divisor() -> decimal { 2 }
            fn scale() -> int { 2 }
            fn rounded(quantity value) -> quantity {
                value.div_round(
                    scale: scale(),
                    mode: Rounding::floor,
                    divisor: divisor(),
                )
            }
        "#;
        let lowered = lower(
            &analyze(&parse(source).expect("parse named quantity intrinsic"))
                .expect("analyze named quantity intrinsic"),
        )
        .expect("lower named quantity intrinsic");
        let rounded = lowered
            .functions
            .iter()
            .find(|function| function.name == "rounded")
            .expect("rounded function");
        let calls = rounded
            .blocks
            .iter()
            .flat_map(|block| block.instrs.iter())
            .filter_map(|instruction| match instruction {
                Instr::Call { callee, .. } => Some(callee.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(calls, ["scale", "divisor"]);
    }

    #[test]
    fn require_lowers_declared_error_code_into_abort_ir() {
        let src = r#"
            error enum PaymentError { Unauthorized = 1001 }
            fn authorize_payment(bool allowed) {
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
                kotoage fn run(int count) -> int authorize("Entry") { return count; }
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
                kotoage fn run(Json ev) authorize("Entry") { let _payload = ev; }
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
                    int count,
                    int total,
                    bool ready,
                    string text,
                    Name label,
                    AssetId asset,
                    DomainId domain,
                    DataSpaceId dataspace,
                    bytes bytes
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
                    Instr::JsonGetNumeric { .. }
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
                    let [ivm_abi::entrypoint::EntrypointValueTypeNodeV1::Leaf(kind)] =
                        field.ty.nodes.as_slice()
                    else {
                        panic!("scalar test parameter must use one leaf node");
                    };
                    (&*field.name, *kind)
                })
                .collect::<Vec<_>>(),
            vec![
                ("count", ivm_abi::entrypoint::EntrypointValueKindV1::Int),
                ("total", ivm_abi::entrypoint::EntrypointValueKindV1::Int),
                ("ready", ivm_abi::entrypoint::EntrypointValueKindV1::Bool),
                ("text", ivm_abi::entrypoint::EntrypointValueKindV1::String),
                ("label", ivm_abi::entrypoint::EntrypointValueKindV1::Name),
                ("asset", ivm_abi::entrypoint::EntrypointValueKindV1::AssetId),
                (
                    "domain",
                    ivm_abi::entrypoint::EntrypointValueKindV1::DomainId
                ),
                (
                    "dataspace",
                    ivm_abi::entrypoint::EntrypointValueKindV1::DataSpaceId
                ),
                ("bytes", ivm_abi::entrypoint::EntrypointValueKindV1::Blob),
            ]
        );
    }

    #[test]
    fn public_aggregate_arguments_cross_internal_calls_as_flat_words() {
        let src = r#"
            seiyaku Demo {
                struct Request { int count, bool ready }

                view fn run(
                    Request request,
                    (int, bool) pair,
                    Option<int> maybe,
                    Result<int, bool> outcome
                ) -> int {
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
            6,
            "products flatten recursively while each sum crosses as one raw handle"
        );
        assert_eq!(implementation.params.len(), 6);
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
            2,
            "only product shapes are rebuilt; Option and Result remain raw handles"
        );
    }

    #[test]
    fn nested_aggregate_returns_use_every_flattened_abi_word() {
        let src = r#"
            seiyaku AggregateReturn {
                struct Pair { int count, bool ready }

                fn make() -> Result<Option<Pair>, (string, bool)> {
                    return Result::ok(Option::some(Pair { count: 7, ready: true }));
                }

                view fn inspect(int seed) -> Result<Option<Pair>, (string, bool)> {
                    let _ = seed;
                    return make();
                }
            }
        "#;
        let program = parse(src).expect("parse nested aggregate return");
        let typed = analyze(&program).expect("analyze nested aggregate return");
        let ir = lower(&typed).expect("lower nested aggregate return");

        for name in ["make", "__entrypoint_impl__inspect"] {
            let function = ir
                .functions
                .iter()
                .find(|function| function.name == name)
                .unwrap_or_else(|| panic!("missing function `{name}`"));
            assert!(
                function
                    .blocks
                    .iter()
                    .any(|block| matches!(&block.terminator, Terminator::Return(Some(_)))),
                "`{name}` must return one active-only sum handle"
            );
        }

        let wrapper = ir
            .functions
            .iter()
            .find(|function| function.name == "inspect")
            .expect("entrypoint wrapper");
        assert!(wrapper.blocks.iter().any(|block| {
            matches!(&block.terminator, Terminator::ReturnN(words) if words.len() == 1)
        }));

        let implementation = ir
            .functions
            .iter()
            .find(|function| function.name == "__entrypoint_impl__inspect")
            .expect("entrypoint implementation");
        assert!(implementation.blocks.iter().any(|block| {
            block.instrs.iter().any(|instruction| {
                matches!(
                    instruction,
                    Instr::CallMulti { callee, dests, .. }
                        if callee == "make" && dests.len() == 1
                )
            })
        }));
    }

    #[test]
    fn inactive_sum_has_no_payload_construction_or_store() {
        let source = r#"
            seiyaku InactivePlaceholder {
                state int counter;

                hajimari() { counter = 0; }

                fn poison() -> int {
                    counter = counter + 1;
                    return 99;
                }

                view fn inspect() -> Option<int> {
                    return Option::none;
                }
            }
        "#;
        let program = parse(source).expect("parse active-only Option");
        let typed = analyze(&program).expect("analyze active-only Option");
        let ir = lower(&typed).expect("lower active-only Option");
        let implementation = ir
            .functions
            .iter()
            .find(|function| function.name == "inspect")
            .expect("view implementation");
        assert!(implementation.blocks.iter().all(|block| {
            block.instrs.iter().all(|instruction| {
                !matches!(instruction, Instr::Call { callee, .. } if callee == "poison")
            })
        }));
        let stores = implementation
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter_map(|instruction| match instruction {
                Instr::Store64Imm { imm, .. } => Some(*imm),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(stores, vec![0], "Option::none writes only its tag");
        assert!(
            implementation
                .blocks
                .iter()
                .any(|block| { matches!(&block.terminator, Terminator::Return(Some(_))) })
        );
    }

    #[test]
    fn tail_expression_lowers_identically_to_explicit_return() {
        let tail = lower(
            &analyze(&parse("fn identity(int value) -> int { value }").expect("parse tail"))
                .expect("analyze tail"),
        )
        .expect("lower tail");
        let explicit = lower(
            &analyze(
                &parse("fn identity(int value) -> int { return value; }").expect("parse return"),
            )
            .expect("analyze return"),
        )
        .expect("lower return");
        fn reachable_blocks(function: &Function) -> Vec<&BasicBlock> {
            let mut pending = vec![function.entry];
            let mut reachable = BTreeSet::new();
            while let Some(label) = pending.pop() {
                if !reachable.insert(label.0) {
                    continue;
                }
                let block = function
                    .blocks
                    .iter()
                    .find(|block| block.label == label)
                    .expect("terminator target must exist");
                match block.terminator {
                    Terminator::Jump(target) => pending.push(target),
                    Terminator::Branch {
                        then_bb, else_bb, ..
                    } => {
                        pending.push(then_bb);
                        pending.push(else_bb);
                    }
                    Terminator::Return(_) | Terminator::Return2(_, _) | Terminator::ReturnN(_) => {}
                }
            }
            function
                .blocks
                .iter()
                .filter(|block| reachable.contains(&block.label.0))
                .collect()
        }

        assert_eq!(
            reachable_blocks(&tail.functions[0]),
            reachable_blocks(&explicit.functions[0]),
            "tail-expression sugar must add no reachable runtime work"
        );
    }

    #[test]
    fn exhaustive_match_reads_only_the_selected_sum_payload() {
        let source = r#"
            fn project(Option<int> value) -> int {
                match value {
                    Option::some(item) => item,
                    Option::none => 0,
                }
            }
        "#;
        let lowered = lower(&analyze(&parse(source).expect("parse match")).expect("analyze match"))
            .expect("lower match");
        let instructions = lowered.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .collect::<Vec<_>>();
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| matches!(instruction, Instr::Load64Imm { imm: 0, .. }))
                .count(),
            1,
            "match reads the discriminant once"
        );
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| matches!(instruction, Instr::Load64Imm { imm: 8, .. }))
                .count(),
            1,
            "only the payload-bearing arm reads offset 8"
        );
    }

    #[test]
    fn propagation_returns_original_error_handle_without_conversion() {
        let source = r#"
            fn propagate(Result<int, bool> value) -> Result<int, bool> {
                let payload = value?;
                Result::ok(payload)
            }
        "#;
        let lowered = lower(
            &analyze(&parse(source).expect("parse propagation")).expect("analyze propagation"),
        )
        .expect("lower propagation");
        let function = &lowered.functions[0];
        assert_eq!(
            function
                .blocks
                .iter()
                .filter(|block| matches!(block.terminator, Terminator::Return(Some(_))))
                .count(),
            2,
            "error and success paths return independently"
        );
        assert_eq!(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.instrs)
                .filter(|instruction| matches!(instruction, Instr::Alloc { .. }))
                .count(),
            1,
            "the propagated error reuses its handle; only Result::ok allocates"
        );
    }

    #[test]
    fn propagation_reallocates_failure_when_the_success_layout_changes() {
        fn memory_profile(function: &Function) -> (usize, usize, usize) {
            let instructions = function.blocks.iter().flat_map(|block| block.instrs.iter());
            instructions.fold((0, 0, 0), |mut counts, instruction| {
                match instruction {
                    Instr::Alloc { .. } => counts.0 += 1,
                    Instr::Load64Imm { .. } => counts.1 += 1,
                    Instr::Store64Imm { .. } => counts.2 += 1,
                    _ => {}
                }
                counts
            })
        }

        let propagated = r#"
            fn widen(Result<int, bool> value) -> Result<(int, int), bool> {
                let payload = value?;
                Result::ok((payload, payload))
            }
        "#;
        let explicit = r#"
            fn widen(Result<int, bool> value) -> Result<(int, int), bool> {
                let payload = match value {
                    Result::ok(payload) => payload,
                    Result::err(failure) => { return Result::err(failure); },
                };
                Result::ok((payload, payload))
            }
        "#;
        let propagated = lower(
            &analyze(&parse(propagated).expect("parse propagation")).expect("analyze propagation"),
        )
        .expect("lower propagation");
        let explicit = lower(
            &analyze(&parse(explicit).expect("parse explicit match"))
                .expect("analyze explicit match"),
        )
        .expect("lower explicit match");

        assert_eq!(
            propagated.functions[0], explicit.functions[0],
            "postfix Result propagation and its canonical exhaustive match must share exact IR"
        );

        assert_eq!(
            memory_profile(&propagated.functions[0]),
            memory_profile(&explicit.functions[0]),
            "`?` must have the same allocation/load/store profile as the canonical explicit match"
        );
        assert_eq!(
            memory_profile(&propagated.functions[0]).0,
            2,
            "the widened error and success paths each allocate the destination layout once"
        );

        let option = r#"
            fn widen(Option<int> value) -> Option<(int, int)> {
                let payload = value?;
                Option::some((payload, payload))
            }
        "#;
        let explicit_option = r#"
            fn widen(Option<int> value) -> Option<(int, int)> {
                let payload = match value {
                    Option::some(payload) => payload,
                    Option::none => { return Option::none; },
                };
                Option::some((payload, payload))
            }
        "#;
        let option = lower(
            &analyze(&parse(option).expect("parse Option propagation"))
                .expect("analyze Option propagation"),
        )
        .expect("lower Option propagation");
        let explicit_option = lower(
            &analyze(&parse(explicit_option).expect("parse explicit Option match"))
                .expect("analyze explicit Option match"),
        )
        .expect("lower explicit Option match");
        assert_eq!(
            option.functions[0], explicit_option.functions[0],
            "postfix Option propagation and its canonical exhaustive match must share exact IR"
        );
        assert_eq!(
            memory_profile(&option.functions[0]).0,
            2,
            "a widened Option::none must reserve the destination layout"
        );
    }

    #[test]
    fn typed_query_page_lowers_to_one_host_call_and_two_typed_handles() {
        let source = r#"
            fn page(int offset, int limit) -> QueryPage<AccountView> {
                ledger::query::accounts(offset: offset, limit: limit)
            }

            fn account(AccountId id) -> Option<AccountView> {
                ledger::query::account(id)
            }
        "#;
        let lowered = lower(
            &analyze(&parse(source).expect("parse typed query page"))
                .expect("analyze typed query page"),
        )
        .expect("lower typed query page");
        let function = lowered
            .functions
            .iter()
            .find(|function| function.name == "page")
            .expect("page function");
        let pages = function
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter_map(|instruction| match instruction {
                Instr::CoreQueryPage {
                    items_dest,
                    next_offset_dest,
                    entity,
                    ..
                } => Some((*items_dest, *next_offset_dest, *entity)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(pages.len(), 1, "one source page performs one host query");
        assert_eq!(
            pages[0].2,
            ivm_abi::core_query::CoreQueryEntityTagV1::Account
        );
        assert!(function.blocks.iter().any(|block| {
            block.instrs.iter().any(|instruction| {
                matches!(
                    instruction,
                    Instr::TuplePack { items, .. }
                        if items.as_slice() == [pages[0].0, pages[0].1]
                )
            })
        }));
        assert!(
            function
                .blocks
                .iter()
                .any(|block| { matches!(block.terminator, Terminator::Return2(_, _)) })
        );

        let singular = lowered
            .functions
            .iter()
            .find(|function| function.name == "account")
            .expect("account function");
        assert_eq!(
            singular
                .blocks
                .iter()
                .flat_map(|block| &block.instrs)
                .filter(|instruction| {
                    matches!(
                        instruction,
                        Instr::CoreQueryGet {
                            entity: ivm_abi::core_query::CoreQueryEntityTagV1::Account,
                            ..
                        }
                    )
                })
                .count(),
            1,
            "one singular source query performs one typed host query"
        );
        assert!(
            singular
                .blocks
                .iter()
                .any(|block| { matches!(block.terminator, Terminator::Return(Some(_))) })
        );
    }

    #[test]
    fn native_json_lowers_to_one_schema_bound_build_and_one_word_table() {
        let source = r#"
            fn build(
                AccountId owner,
                string label,
                Option<quantity> maybe,
            ) -> Json {
                let List<string, 4> labels = ["secondary", label];
                json {
                    owner: owner,
                    amount: 1.25,
                    primary: json ["primary", label],
                    labels: labels,
                    maybe: maybe,
                }
            }
        "#;
        let lowered = lower(
            &analyze(&parse(source).expect("parse native JSON")).expect("analyze native JSON"),
        )
        .expect("lower native JSON");
        let function = lowered
            .functions
            .iter()
            .find(|function| function.name == "build")
            .expect("build function");
        let instructions = function
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .collect::<Vec<_>>();
        let builds = instructions
            .iter()
            .filter_map(|instruction| match instruction {
                Instr::DirectHelperSyscall {
                    syscall,
                    args,
                    dest,
                } if *syscall == ivm_abi::syscalls::SYSCALL_JSON_BUILD => {
                    Some((*dest, args.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            builds.len(),
            1,
            "native JSON performs exactly one host build"
        );
        let [schema_ref, table, word_count] = builds[0].1.as_slice() else {
            panic!("JSON_BUILD must receive schema, word table, and word count");
        };
        let encoded_schema = instructions
            .iter()
            .find_map(|instruction| match instruction {
                Instr::DataRef {
                    dest,
                    kind: DataRefKind::NoritoBytes,
                    value,
                } if dest == schema_ref => value.strip_prefix("0x"),
                _ => None,
            })
            .expect("static JSON construction schema");
        let schema_bytes = hex::decode(encoded_schema).expect("schema hex");
        let schema: ivm_abi::json::JsonConstructionSchemaV1 =
            norito::decode_from_bytes(&schema_bytes).expect("decode construction schema");
        assert!(schema.validate());
        assert_eq!(schema.word_count(), Some(6));
        assert!(matches!(
            &schema.nodes[0],
            ivm_abi::json::JsonConstructionNodeV1::Object { keys }
                if keys.iter().map(String::as_str).eq([
                    "owner", "amount", "primary", "labels", "maybe"
                ])
        ));
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| {
                    matches!(instruction, Instr::Store64Imm { base, .. } if base == table)
                })
                .count(),
            6,
            "every schema word is written once into one contiguous table"
        );
        assert!(instructions.iter().any(|instruction| {
            matches!(instruction, Instr::Const { dest, value: 6 } if dest == word_count)
        }));
        assert!(
            instructions
                .iter()
                .all(|instruction| !matches!(instruction, Instr::JsonObject { .. }))
        );
    }

    #[test]
    fn list_layout_flattens_nested_sum_handles_to_one_word() {
        let nested = Type::List(
            Box::new(Type::Option(Box::new(Type::Result(
                Box::new(Type::Quantity),
                Box::new(Type::Bool),
            )))),
            64,
        );
        let (element, layout) = list_layout_for_type(&nested).expect("valid nested List layout");
        assert!(matches!(element, Type::Option(_)));
        assert_eq!(layout.capacity(), 64);
        assert_eq!(layout.element_words(), 1);
        assert_eq!(layout.allocation_bytes(), Ok((2 + 64) * 8));

        let enumerated = Type::List(Box::new(Type::Tuple(vec![Type::Int, Type::Quantity])), 4);
        let (_, layout) = list_layout_for_type(&enumerated).expect("pair List layout");
        assert_eq!(layout.element_words(), 2);
    }

    #[test]
    fn list_len_and_enumerate_materialize_source_int_values() {
        let source = "fn indices() -> List<(int, int), 4> {\
                 let List<int, 4> values = [1, 2];\
                 let length = values.len();\
                 values.enumerate()\
             }";
        let lowered = lower(
            &analyze(&parse(source).expect("parse List scalar boundaries"))
                .expect("analyze List scalar boundaries"),
        )
        .expect("lower List scalar boundaries");
        let instructions = lowered.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .collect::<Vec<_>>();
        let materialized = instructions
            .iter()
            .filter_map(|instruction| match instruction {
                Instr::IntFromU64 { dest, value } => Some((*dest, *value)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(
            materialized.len() >= 2,
            "List.len and List.enumerate indices must each cross the scalar/int boundary"
        );
        assert!(instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instr::TuplePack { items, .. }
                    if materialized.iter().any(|(source_int, _)| items.first() == Some(source_int))
            )
        }));
    }

    #[test]
    fn list_get_converts_an_arbitrary_width_index_only_after_bounds_proof() {
        let source =
            "fn probe(List<int, 4> values, int index) -> Option<int> { values.get(index) }";
        let lowered = lower(
            &analyze(&parse(source).expect("parse List.get bounds proof"))
                .expect("analyze List.get bounds proof"),
        )
        .expect("lower List.get bounds proof");
        let function = &lowered.functions[0];
        let conversion_block = function
            .blocks
            .iter()
            .find(|block| {
                block
                    .instrs
                    .iter()
                    .any(|instruction| matches!(instruction, Instr::IntTryToU64 { .. }))
            })
            .expect("successful List.get arm converts its proven index");
        let predecessor = function
            .blocks
            .iter()
            .find(|block| {
                matches!(
                    block.terminator,
                    Terminator::Branch { then_bb, .. } if then_bb == conversion_block.label
                )
            })
            .expect("bounds-check branch dominates scalar conversion");

        assert!(predecessor.instrs.iter().any(|instruction| {
            matches!(
                instruction,
                Instr::NumericCompare {
                    op: BinaryOp::Ge,
                    kind: WideNumericKind::Int,
                    ..
                }
            )
        }));
        assert!(predecessor.instrs.iter().any(|instruction| {
            matches!(
                instruction,
                Instr::NumericCompare {
                    op: BinaryOp::Lt,
                    kind: WideNumericKind::Int,
                    ..
                }
            )
        }));
        assert!(
            predecessor
                .instrs
                .iter()
                .all(|instruction| !matches!(instruction, Instr::IntTryToU64 { .. })),
            "out-of-range indices must reach Option::none without a narrowing fault"
        );
    }

    #[test]
    fn scalar_protocol_arguments_use_checked_adaptive_int_conversion() {
        let source = r#"
            fn boundaries(
                bytes payload,
                int scheme,
                int tag_length,
                int enabled,
            ) {
                let _verified = crypto::verify_signature(
                    message: payload,
                    signature: payload,
                    public_key: payload,
                    scheme: scheme,
                );
                let _sealed = crypto::sm4_ccm::seal(
                    key: payload,
                    nonce: payload,
                    aad: payload,
                    payload: payload,
                    tag_length: tag_length,
                );
                ledger::trigger::set_enabled(Name::parse("scheduled"), enabled);
                let _vrf = crypto::vrf::verify(request: payload);
            }
        "#;
        let lowered = lower(
            &analyze(&parse(source).expect("parse checked scalar boundaries"))
                .expect("analyze checked scalar boundaries"),
        )
        .expect("lower checked scalar boundaries");
        let instructions = lowered.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .collect::<Vec<_>>();
        let converted = instructions
            .iter()
            .filter_map(|instruction| match instruction {
                Instr::IntTryToU64 { dest, .. } => Some(*dest),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(converted.len(), 3);
        for scalar in instructions
            .iter()
            .filter_map(|instruction| match instruction {
                Instr::VerifySignature { scheme, .. } => Some(*scheme),
                Instr::Sm4CcmSeal {
                    tag_len: Some(tag_len),
                    ..
                } => Some(*tag_len),
                Instr::SetTriggerEnabled { enabled, .. } => Some(*enabled),
                _ => None,
            })
        {
            assert!(
                converted.contains(&scalar),
                "scalar protocol register must receive a checked u64 conversion"
            );
        }
    }

    #[test]
    fn scalar_protocol_literals_outside_u64_fail_lowering() {
        for value in ["-1", "18446744073709551616"] {
            let source = format!(
                "fn verify(bytes payload) {{ let _ok = crypto::verify_signature(\
                    message: payload, signature: payload, public_key: payload, scheme: {value}); }}"
            );
            let typed = analyze(&parse(&source).expect("parse scalar boundary overflow"))
                .expect("analyze scalar boundary overflow");
            let error = lower(&typed).expect_err("out-of-range scalar argument must fail closed");
            assert!(
                error.contains("outside the unsigned 64-bit range required by this host boundary"),
                "unexpected error for {value}: {error}"
            );
        }
    }

    #[test]
    fn state_map_int_keys_are_encoded_from_the_canonical_int_pointer() {
        let source = "state StateMap<int, int> balances; fn set(int key, int value) { balances[key] = value; }";
        let lowered = lower(
            &analyze(&parse(source).expect("parse numeric StateMap key"))
                .expect("analyze numeric StateMap key"),
        )
        .expect("lower numeric StateMap key");
        let instructions = lowered.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .collect::<Vec<_>>();
        let encoded_keys = instructions
            .iter()
            .filter_map(|instruction| match instruction {
                Instr::PointerToNorito { dest, value } => Some((*dest, *value)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            encoded_keys.len(),
            1,
            "one canonical key encoding per access"
        );
        assert!(instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instr::PathMapKeyNorito { key_blob, .. }
                    if *key_blob == encoded_keys[0].0
            )
        }));
        assert!(
            instructions
                .iter()
                .all(|instruction| !matches!(instruction, Instr::EncodeInt { .. })),
            "source int keys must not use the retired scalar/ASCII key codec"
        );
    }

    #[test]
    fn list_literal_and_comprehension_use_only_contiguous_ir_operations() {
        let source = r#"
            fn doubled() -> List<int, 4> {
                let List<int, 4> values = [1, 2, 3];
                [value * 2 for value in values if value > 1]
            }
        "#;
        let lowered = lower(
            &analyze(&parse(source).expect("parse List comprehension"))
                .expect("analyze List comprehension"),
        )
        .expect("lower List comprehension");
        let instructions = lowered.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .collect::<Vec<_>>();
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| matches!(instruction, Instr::Alloc { .. }))
                .count(),
            2,
            "literal and result each use one contiguous allocation"
        );
        assert!(
            instructions
                .iter()
                .any(|instruction| matches!(instruction, Instr::Load64 { .. }))
        );
        assert!(
            instructions
                .iter()
                .any(|instruction| matches!(instruction, Instr::Store64 { .. }))
        );
        assert!(instructions.iter().all(|instruction| {
            !matches!(instruction, Instr::Call { callee, .. } if callee.contains("list"))
        }));
    }

    #[test]
    fn identity_comprehension_does_not_exceed_bounded_copy_baseline() {
        fn instruction_count(source: &str) -> usize {
            lower(
                &analyze(&parse(source).expect("parse bounded copy"))
                    .expect("analyze bounded copy"),
            )
            .expect("lower bounded copy")
            .functions[0]
                .blocks
                .iter()
                .map(|block| block.instrs.len() + 1)
                .sum()
        }

        let comprehension = instruction_count(
            "fn copy() -> List<int, 4> { let List<int, 4> source = [1, 2]; [value for value in source] }",
        );
        let bounded_baseline = instruction_count(
            "fn copy() -> List<int, 4> { let List<int, 4> source = [1, 2]; source.take(4) }",
        );
        assert!(
            comprehension <= bounded_baseline,
            "identity List sugar emitted {comprehension} operations versus {bounded_baseline} for the bounded copy baseline"
        );
    }

    #[test]
    fn every_list_method_lowers_without_runtime_helper_calls() {
        let source = r#"
            fn methods() {
                var List<int, 4> values = [1, 2];
                values.len();
                values.get(0);
                values.try_set(index: 0, value: 3);
                values.try_push(4);
                values.contains(3);
                values.pop();
                values.take(2);
                values.enumerate();
            }
        "#;
        let lowered = lower(
            &analyze(&parse(source).expect("parse List methods")).expect("analyze List methods"),
        )
        .expect("lower List methods");
        let instructions = lowered.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .collect::<Vec<_>>();
        assert!(instructions.iter().all(|instruction| {
            !matches!(instruction, Instr::Call { callee, .. } if callee.starts_with("__kotodama_list_"))
        }));
        assert!(
            instructions
                .iter()
                .any(|instruction| matches!(instruction, Instr::Alloc { .. }))
        );
        assert!(
            instructions
                .iter()
                .any(|instruction| matches!(instruction, Instr::Load64Imm { imm: 0, .. }))
        );
        assert!(
            instructions
                .iter()
                .any(|instruction| matches!(instruction, Instr::Store64Imm { imm: 0, .. }))
        );
    }

    #[test]
    fn list_take_zero_lowers_to_a_bounded_empty_copy() {
        let source =
            "fn empty() -> List<int, 1> { let List<int, 4> values = [1, 2]; values.take(0) }";
        let lowered = lower(
            &analyze(&parse(source).expect("parse List.take(0)")).expect("analyze List.take(0)"),
        )
        .expect("lower List.take(0)");
        let function = &lowered.functions[0];
        assert!(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.instrs)
                .any(|instruction| matches!(instruction, Instr::Const { value: 0, .. })),
            "the zero limit must remain an explicit deterministic bound"
        );
        assert!(function.blocks.iter().all(|block| {
            block.instrs.iter().all(|instruction| {
                !matches!(instruction, Instr::Call { callee, .. } if callee.starts_with("__kotodama_list_"))
            })
        }));
    }

    #[test]
    fn recursive_list_contains_dereferences_aggregate_handles() {
        let source = r#"
            struct Envelope {
                Option<List<int, 2>> labels,
                Result<(int, bool), int> outcome,
            }

            fn contains_nested(Envelope needle) -> bool {
                let List<Envelope, 2> values = [
                    Envelope {
                        labels: Option::some([1, 2]),
                        outcome: Result::ok((7, true)),
                    },
                ];
                values.contains(needle)
            }
        "#;
        let lowered = lower(
            &analyze(&parse(source).expect("parse recursive List.contains"))
                .expect("analyze recursive List.contains"),
        )
        .expect("lower recursive List.contains");
        let function = &lowered.functions[0];
        let instructions = function
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .collect::<Vec<_>>();

        assert!(
            instructions
                .iter()
                .all(|instruction| !matches!(instruction, Instr::PointerEq { .. })),
            "compiler-owned aggregate handles must never be compared by pointer identity"
        );
        assert!(
            instructions
                .iter()
                .filter(|instruction| matches!(instruction, Instr::Load64Imm { imm: 0, .. }))
                .count()
                >= 5,
            "nested List lengths and active sum tags must be loaded structurally"
        );
        assert!(
            instructions
                .iter()
                .any(|instruction| matches!(instruction, Instr::TupleGet { .. })),
            "struct and tuple fields must be compared structurally"
        );
        assert!(
            function
                .blocks
                .iter()
                .filter(|block| matches!(block.terminator, Terminator::Branch { .. }))
                .count()
                >= 6,
            "sum tags and nested List bounds must guard active payload reads"
        );
    }

    #[test]
    fn failed_list_mutation_branches_have_no_stores() {
        let source = r#"
            fn set(int index) -> bool {
                var List<int, 1> values = [1];
                values.try_set(index: index, value: 2)
            }

            fn push() -> bool {
                var List<int, 1> values = [1];
                values.try_push(2)
            }
        "#;
        let lowered = lower(
            &analyze(&parse(source).expect("parse failed mutation paths"))
                .expect("analyze failed mutation paths"),
        )
        .expect("lower failed mutation paths");
        for function in &lowered.functions {
            let branch = function
                .blocks
                .iter()
                .find_map(|block| match block.terminator {
                    Terminator::Branch { else_bb, .. } => Some(else_bb),
                    _ => None,
                })
                .expect("mutation has a bounds branch");
            let failure = function
                .blocks
                .iter()
                .find(|block| block.label == branch)
                .expect("failure block exists");
            assert!(failure.instrs.iter().all(|instruction| !matches!(
                instruction,
                Instr::Store64 { .. } | Instr::Store64Imm { .. }
            )));
        }
    }

    #[test]
    fn entrypoint_list_schema_is_recursive_and_capacity_bound() {
        use ivm_abi::entrypoint::EntrypointValueTypeNodeV1 as Node;

        let ty = Type::List(Box::new(Type::Option(Box::new(Type::Quantity))), 64);
        let schema = entrypoint_return_schema("items", Some(&ty))
            .expect("build List return schema")
            .expect("non-unit return schema");
        let [Node::List(list), Node::Option, Node::Leaf(_)] = schema.nodes.as_slice() else {
            panic!("expected one flat List<Option<quantity>> preorder tape");
        };
        assert_eq!(list.capacity, 64);
        assert_eq!(schema.word_count(), Some(1));
    }

    #[test]
    fn query_page_entrypoint_schemas_are_structural_and_roundtrip_for_all_views() {
        use ivm_abi::entrypoint::EntrypointValueTypeNodeV1 as Node;

        let source = r#"
            seiyaku TypedPages {
                view fn accounts(int offset, int limit) -> QueryPage<AccountView> {
                    ledger::query::accounts(offset: offset, limit: limit)
                }
                view fn assets(int offset, int limit) -> QueryPage<AssetView> {
                    ledger::query::assets(offset: offset, limit: limit)
                }
                view fn asset_definitions(int offset, int limit) -> QueryPage<AssetDefinitionView> {
                    ledger::query::asset_definitions(offset: offset, limit: limit)
                }
                view fn domains(int offset, int limit) -> QueryPage<DomainView> {
                    ledger::query::domains(offset: offset, limit: limit)
                }
                view fn nfts(int offset, int limit) -> QueryPage<NftView> {
                    ledger::query::nfts(offset: offset, limit: limit)
                }
            }
        "#;
        let typed = analyze(&parse(source).expect("parse all typed query pages"))
            .expect("analyze all typed query pages");
        let mut encoded_schemas = BTreeSet::new();

        for (entrypoint_name, view_name) in [
            ("accounts", "AccountView"),
            ("assets", "AssetView"),
            ("asset_definitions", "AssetDefinitionView"),
            ("domains", "DomainView"),
            ("nfts", "NftView"),
        ] {
            let function = typed
                .items
                .iter()
                .map(|item| match item {
                    TypedItem::Function(function) => function,
                })
                .find(|function| function.name == entrypoint_name)
                .unwrap_or_else(|| panic!("missing {entrypoint_name} entrypoint"));
            let schema = entrypoint_return_schema(entrypoint_name, function.ret_ty.as_ref())
                .expect("build structural query-page schema")
                .expect("query page has a public return schema");
            let [
                Node::Struct(page),
                Node::List(items),
                Node::Struct(view),
                ..,
            ] = schema.nodes.as_slice()
            else {
                panic!("unexpected {entrypoint_name} schema: {schema:?}");
            };
            assert_eq!(page.name, "QueryPage");
            assert_eq!(page.fields, ["items", "next_offset"]);
            assert_eq!(items.capacity, 64);
            assert_eq!(view.name, view_name);
            assert!(matches!(
                schema
                    .nodes
                    .as_slice()
                    .get(schema.nodes.len().saturating_sub(2)..),
                Some([
                    Node::Option,
                    Node::Leaf(ivm_abi::entrypoint::EntrypointValueKindV1::Int)
                ])
            ));
            assert!(schema.validate());
            assert_eq!(schema.word_count(), Some(2));

            let encoded = norito::to_bytes(&schema).expect("encode query-page schema");
            let decoded: ivm_abi::entrypoint::EntrypointValueTypeV1 =
                norito::decode_from_bytes(&encoded).expect("decode query-page schema");
            assert_eq!(decoded, schema);
            assert!(
                encoded_schemas.insert(encoded),
                "{view_name} specialization must have a distinct structural schema"
            );
        }
        assert_eq!(encoded_schemas.len(), 5);
    }

    #[test]
    fn aggregate_argument_words_over_register_window_fail_during_lowering() {
        let src = r#"
            seiyaku WideCall {
                struct Wide {
                    int f00, int f01, int f02, int f03, int f04,
                    int f05, int f06, int f07, int f08, int f09,
                    int f10, int f11, int f12, int f13
                }
                view fn inspect(Wide value) -> int { return value.f00; }
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
                kotoage fn run(int count) -> int authorize("Entry") { return count + 1; }

                #[test]
                fn drive_run() {
                    let next = test::invoke_entrypoint(entrypoint: "run", arguments: Json::parse("{\"count\": 7}"));
                    test::assert_eq(actual: next, expected: 8);
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
        let mut saw_numeric_assertion = false;
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
                    Instr::NumericCompare {
                        op: BinaryOp::Eq,
                        kind: WideNumericKind::Int,
                        ..
                    } => saw_numeric_assertion = true,
                    Instr::AssertEq { .. } => {
                        panic!("adaptive int assertions must not compare pointer addresses")
                    }
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
        assert!(
            saw_numeric_assertion,
            "test::assert_eq must use canonical numeric equality"
        );
    }

    #[test]
    fn invoke_entrypoint_tuple_return_uses_wrapper_callmulti() {
        let src = r#"
            seiyaku Demo {
                kotoage fn run(int count) -> (int, int) authorize("Entry") { return (count, count + 1); }

                #[test]
                fn drive_run() {
                    let pair = test::invoke_entrypoint(entrypoint: "run", arguments: Json::parse("{\"count\": 7}"));
                    test::assert_eq(actual: pair.0, expected: 7);
                    test::assert_eq(actual: pair.1, expected: 8);
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
                kotoage fn run(int count) -> int authorize("Entry") { return count + 1; }

                #[test]
                fn drive_run() {
                    let next = test::invoke_entrypoint_as(
                        actor: "issuer",
                        entrypoint: "run",
                        arguments: Json::parse("{\"count\": 7}"),
                    );
                    test::expect_reject_as(
                        actor: "issuer",
                        entrypoint: "run",
                        arguments: Json::parse("{\"count\": -1}"),
                    );
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
                            *returns_pointer,
                            "adaptive int returns through its canonical pointer ABI"
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
                kotoage fn run(int count) -> (int, int) authorize("Entry") { return (count, count + 1); }

                #[test]
                fn drive_run() {
                    let pair = test::invoke_entrypoint_as(
                        actor: "issuer",
                        entrypoint: "run",
                        arguments: Json::parse("{\"count\": 7}"),
                    );
                    test::assert_eq(actual: pair.0, expected: 7);
                    test::assert_eq(actual: pair.1, expected: 8);
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
        let src = "fn f(int a, int b) { if a == b { let c = a; } else { let c = b; } }";
        let ir = lower(&analyze(&parse(src).unwrap()).unwrap()).expect("lower");
        assert_eq!(ir.functions[0].blocks.len(), 4); // entry, then, else, end
    }

    #[test]
    fn logical_operators_lower_to_short_circuit_cfg() {
        let src = r#"
fn rhs() -> bool { return true; }
fn both(bool value) -> bool { return value && rhs(); }
fn either(bool value) -> bool { return value || rhs(); }
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
            fn f() -> int {
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
                Instr::DataRef {
                    dest,
                    kind: DataRefKind::Int,
                    value,
                } if value == "7" => Some(*dest),
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
            fn f() -> int {
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
                state StateMap<int, int> Values;

                kotoage fn f() -> int authorize("WriteState") {
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
            fn f() -> int {
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
                Instr::DataRef {
                    dest,
                    kind: DataRefKind::Int,
                    value,
                } if value == "9" => Some(*dest),
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
        let src = "fn identity(int value) -> int { return value; }";
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
        let src = "fn f() -> int { return 1; let x = 2; }";
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
        let src = r#"fn main() { let bytes _b = b"ab"; }"#;
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
    fn lower_get_quantity_builtin() {
        let src = "fn main() { let ev = context::trigger_event(); let Option<quantity> value = ev.get_quantity(Name::parse(\"value\")); }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_get_numeric = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                if let Instr::JsonGetNumeric {
                    kind: WideNumericKind::Quantity,
                    ..
                } = instr
                {
                    saw_get_numeric = true;
                }
            }
        }
        assert!(
            saw_get_numeric,
            "expected quantity JsonGetNumeric instruction in lowered IR"
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
              state int MintRequestNextSequence;
              state StateMap<Name, int> MintRequestSequenceById;
              state StateMap<int, int> MintRequestSequences;
              state StateMap<int, Name> MintRequestRequestIds;
              state StateMap<int, Name> MintRequestFiIds;
              state StateMap<int, AccountId> MintRequestFiAuthorities;
              state StateMap<int, AccountId> MintRequestToAccounts;
              state StateMap<int, int> MintRequestAmounts;
              state StateMap<int, Json> MintRequestRequestedBy;
              state StateMap<int, int> MintRequestStates;
              state StateMap<int, int> MintRequestCreatedAt;
              state StateMap<int, int> MintRequestExpiresAt;
              state StateMap<int, int> MintRequestFinalizedAt;
              state StateMap<int, int> MintRequestCanceledAt;

              hajimari() { MintRequestNextSequence = 0; }

              fn update_record(int sequence,
                               Name request_id,
                               Name fi_id,
                               AccountId fi_multisig_account_id,
                               AccountId to_account_id,
                               int amount_i64,
                               Json requested_by_actor_id,
                               int state_code,
                               int created_at_ms,
                               int expires_at_ms,
                               int finalized_at_ms,
                               int canceled_at_ms) {
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
                if let Instr::PathMapKeyNorito { base, .. } = instr {
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
                struct TransferArgs { DomainId domain; AccountId to; }
                fn main() {
                    let args = TransferArgs {
                        domain: DomainId::parse("wonderland.universal"),
                        to: AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"),
                    };
                    ledger::domain::transfer(
                        source: context::authority(),
                        domain: args.domain,
                        destination: args.to,
                    );
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
        let src = "state StateMap<int, int> balances; fn f() -> int { return balances.get_or(key: 1, default: 7); }";
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
        let src = "state StateMap<int, int> balances; fn f() { let _value = balances.get(1); }";
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
        let src = "state StateMap<int, int> balances; fn f() { let _value = balances.remove(1); }";
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
                struct Ledger { int counter; bool flag; }
                state Ledger ledger;

                hajimari() { ledger = Ledger { counter: 0, flag: false }; }

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
    fn named_struct_literal_lowers_in_declaration_order() {
        let program = parse(
            r#"
            module NamedStruct {
                struct Pair { int first; int second; }
                fn main() -> Pair { return Pair { second: 2, first: 1 }; }
            }
            "#,
        )
        .expect("parse named struct literal");
        let typed = analyze(&program).expect("analyze named struct literal");
        let lowered = lower(&typed).expect("lower named struct literal");
        let main = lowered
            .functions
            .iter()
            .find(|function| function.name == "main")
            .expect("main function");
        let constants = main
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter_map(|instruction| match instruction {
                Instr::DataRef {
                    dest,
                    kind: DataRefKind::Int,
                    value,
                } => Some((*dest, value.as_str())),
                _ => None,
            })
            .collect::<HashMap<_, _>>();
        let items = main
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .find_map(|instruction| match instruction {
                Instr::TuplePack { items, .. } if items.len() == 2 => Some(items),
                _ => None,
            })
            .expect("struct TuplePack");
        assert_eq!(constants.get(&items[0]), Some(&"1"));
        assert_eq!(constants.get(&items[1]), Some(&"2"));
    }

    #[test]
    fn named_struct_literal_evaluates_fields_in_source_order_before_layout() {
        let program = parse(
            r#"
            module NamedStructEffects {
                struct Pair { int first; int second; }
                fn first() -> int { 1 }
                fn second() -> int { 2 }
                fn main() -> Pair { Pair { second: second(), first: first() } }
            }
            "#,
        )
        .expect("parse effectful named struct literal");
        let lowered = lower(&analyze(&program).expect("analyze named struct effects"))
            .expect("lower named struct effects");
        let main = lowered
            .functions
            .iter()
            .find(|function| function.name == "main")
            .expect("main function");
        let calls = main
            .blocks
            .iter()
            .flat_map(|block| block.instrs.iter())
            .filter_map(|instruction| match instruction {
                Instr::Call { callee, dest, .. } => Some((callee.as_str(), *dest)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            calls.iter().map(|(callee, _)| *callee).collect::<Vec<_>>(),
            ["second", "first"]
        );
        let packed = main
            .blocks
            .iter()
            .flat_map(|block| block.instrs.iter())
            .find_map(|instruction| match instruction {
                Instr::TuplePack { items, .. } if items.len() == 2 => Some(items),
                _ => None,
            })
            .expect("struct TuplePack");
        assert_eq!(packed, &[calls[1].1.unwrap(), calls[0].1.unwrap()]);
    }

    #[test]
    fn lower_state_struct_assignment_encodes_and_writes_once() {
        let src = r#"
            seiyaku C {
                struct Ledger { int counter; bool flag; }
                state Ledger ledger;

                hajimari() { ledger = Ledger { counter: 0, flag: false }; }

                fn main() {
                    ledger = Ledger { counter: 7, flag: true };
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
                struct Ledger { int counter; bool flag; }
                state StateMap<int, Ledger> ledgers;

                hajimari() {}

                fn main() {
                    ledgers[7] = Ledger { counter: 9, flag: true };
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
    fn aggregate_unwrap_or_lowers_one_eager_fallback_call() {
        let source = r#"
            seiyaku C {
                struct PolicyState {
                    int version,
                    bytes document,
                    bytes document_hash,
                    AccountId approved_by,
                    int applied_at_ms,
                    Name change_id,
                }
                state StateMap<Name, PolicyState> Policies;
                state StateMap<int, int> FallbackCalls;

                fn observed_fallback() -> PolicyState {
                    let count = FallbackCalls.get(1).unwrap_or(0);
                    FallbackCalls[1] = count + 1;
                    return PolicyState {
                        version: 0,
                        document: b"fallback",
                        document_hash: b"fallback-hash",
                        approved_by: context::authority(),
                        applied_at_ms: 0,
                        change_id: Name::parse("fallback"),
                    };
                }

                kotoage fn main() -> int authorize("WriteState") {
                    let policy = Policies.get(Name::parse("spend")).unwrap_or(
                        observed_fallback(),
                    );
                    return policy.version;
                }
            }
        "#;
        let program = parse(source).expect("parse mixed aggregate unwrap_or");
        let typed = analyze(&program).expect("analyze mixed aggregate unwrap_or");
        let lowered = lower(&typed).expect("lower mixed aggregate unwrap_or");
        let main = lowered
            .functions
            .iter()
            .find(|function| function.name == "main")
            .expect("main function");
        let fallback_calls = main
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter(|instruction| {
                matches!(
                    instruction,
                    Instr::CallMulti { callee, .. } if callee == "observed_fallback"
                )
            })
            .count();
        assert_eq!(
            fallback_calls, 1,
            "eager unwrap_or must evaluate an effectful fallback exactly once"
        );
    }

    #[test]
    fn tuple_binding_projects_one_captured_call_result() {
        let source = r#"
            seiyaku C {
                state StateMap<int, int> Observations;

                fn observed_pair() -> (int, int) {
                    let count = Observations.get(1).unwrap_or(0);
                    Observations[1] = count + 1;
                    return (1, 2);
                }

                kotoage fn main() -> int authorize("WriteState") {
                    let pair = observed_pair();
                    return pair.0 + pair.1;
                }
            }
        "#;
        let program = parse(source).expect("parse tuple call binding");
        let typed = analyze(&program).expect("analyze tuple call binding");
        let lowered = lower(&typed).expect("lower tuple call binding");
        let main = lowered
            .functions
            .iter()
            .find(|function| function.name == "main")
            .expect("main function");
        let calls = main
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter(|instruction| {
                matches!(
                    instruction,
                    Instr::CallMulti { callee, .. } if callee == "observed_pair"
                )
            })
            .count();
        assert_eq!(
            calls, 1,
            "tuple field bindings must project one captured call result"
        );
    }

    #[test]
    fn tuple_destructuring_projects_one_captured_call_result() {
        let source = r#"
            seiyaku C {
                state StateMap<int, int> Observations;

                fn observed_pair() -> (int, int) {
                    let count = Observations.get(1).unwrap_or(0);
                    Observations[1] = count + 1;
                    return (1, 2);
                }

                kotoage fn main() -> int authorize("WriteState") {
                    let (left, right) = observed_pair();
                    return left + right;
                }
            }
        "#;
        let program = parse(source).expect("parse tuple destructuring call");
        let typed = analyze(&program).expect("analyze tuple destructuring call");
        let lowered = lower(&typed).expect("lower tuple destructuring call");
        let main = lowered
            .functions
            .iter()
            .find(|function| function.name == "main")
            .expect("main function");
        let calls = main
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter(|instruction| {
                matches!(
                    instruction,
                    Instr::CallMulti { callee, .. } if callee == "observed_pair"
                )
            })
            .count();
        assert_eq!(
            calls, 1,
            "tuple destructuring must project one captured call result"
        );
    }

    #[test]
    fn nested_struct_binding_projects_one_captured_call_result() {
        let source = r#"
            seiyaku C {
                struct Inner { int value, bytes marker }
                struct Outer { Inner inner, int version }
                state StateMap<int, int> Observations;

                fn observed_outer() -> Outer {
                    let count = Observations.get(1).unwrap_or(0);
                    Observations[1] = count + 1;
                    return Outer {
                        inner: Inner { value: 40, marker: b"nested" },
                        version: 2,
                    };
                }

                kotoage fn main() -> int authorize("WriteState") {
                    let outer = observed_outer();
                    return outer.inner.value + outer.version;
                }
            }
        "#;
        let program = parse(source).expect("parse nested struct call binding");
        let typed = analyze(&program).expect("analyze nested struct call binding");
        let lowered = lower(&typed).expect("lower nested struct call binding");
        let main = lowered
            .functions
            .iter()
            .find(|function| function.name == "main")
            .expect("main function");
        let calls = main
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter(|instruction| {
                matches!(
                    instruction,
                    Instr::CallMulti { callee, .. } if callee == "observed_outer"
                )
            })
            .count();
        assert_eq!(
            calls, 1,
            "nested struct fields must project one captured call result"
        );
    }

    #[test]
    fn scalar_state_read_after_write_reuses_the_live_value() {
        let src = r#"
            seiyaku C {
                state int counter;
                hajimari() { counter = 0; }
                fn main() -> int {
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
                struct Pair { int count; bool ready }
                state StateMap<int, Pair> values;
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
                state StateMap<int, bytes> values;
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
        const MAXIMUM: &str = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047";
        let safe = parse(&format!(
            "fn main() -> int {{ return ({MAXIMUM} - 1) + 1; }}"
        ))
        .expect("parse safe constant expression");
        let safe = lower(&analyze(&safe).expect("analyze safe constant expression"))
            .expect("lower safe constant expression");
        assert!(safe.functions[0].blocks.iter().any(|block| {
            block.instrs.iter().any(|instr| {
                matches!(
                    instr,
                    Instr::DataRef {
                        kind: DataRefKind::Int,
                        value,
                        ..
                    } if value == MAXIMUM
                )
            })
        }));

        let overflow = parse(&format!("fn main() -> int {{ return {MAXIMUM} + 1; }}"))
            .expect("parse overflowing constant expression");
        let error = analyze(&overflow).expect_err("constant overflow must not wrap");
        assert!(error.code == "E_INT_OVERFLOW", "unexpected error: {error}");
    }

    #[test]
    fn exact_decimal_constants_fold_and_invalid_arithmetic_is_diagnosed() {
        let safe = parse("fn main() -> decimal { return 1.0 / 8.0; }")
            .expect("parse exact decimal constant");
        let safe = lower(&analyze(&safe).expect("analyze exact decimal constant"))
            .expect("lower exact decimal constant");
        assert!(safe.functions[0].blocks.iter().any(|block| {
            block.instrs.iter().any(|instruction| {
                matches!(
                    instruction,
                    Instr::DataRef {
                        kind: DataRefKind::Decimal,
                        value,
                        ..
                    } if value == "0.125"
                )
            })
        }));

        for (source, code) in [
            (
                "fn main() -> quantity { return 1 - 2; }",
                "E_QUANTITY_UNDERFLOW",
            ),
            (
                "fn main() -> decimal { return 1.0 / 3.0; }",
                "E_REPEATING_DECIMAL",
            ),
        ] {
            let program = parse(source).expect("parse invalid constant numeric arithmetic");
            let error = analyze(&program)
                .expect_err("invalid constant numeric arithmetic must fail semantic checking");
            assert_eq!(error.code, code);
        }
    }

    #[test]
    fn rounded_numeric_division_is_constant_folded_or_one_numeric_round_instruction() {
        let dynamic = parse(
            "fn rounded(quantity value, decimal divisor, int scale) -> quantity { \
                return value.div_round( \
                    divisor: divisor, \
                    scale: scale, \
                    mode: Rounding::nearest_even, \
                ); \
            }",
        )
        .expect("parse dynamic rounded quantity division");
        let dynamic = lower(&analyze(&dynamic).expect("analyze rounded quantity division"))
            .expect("lower rounded quantity division");
        let calls = dynamic.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter(|instruction| {
                matches!(
                    instruction,
                    Instr::NumericRound {
                        op: NumericRoundOp::QuantityDiv,
                        ..
                    }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);

        let folded = parse(
            "fn rounded() -> decimal { \
                return 1.0.div_round( \
                    divisor: 8.0, \
                    scale: 2, \
                    mode: Rounding::nearest_even, \
                ); \
            }",
        )
        .expect("parse constant rounded decimal division");
        let folded = lower(&analyze(&folded).expect("analyze constant rounded decimal division"))
            .expect("lower constant rounded decimal division");
        assert!(folded.functions[0].blocks.iter().any(|block| {
            block.instrs.iter().any(|instruction| {
                matches!(
                    instruction,
                    Instr::DataRef {
                        kind: DataRefKind::Decimal,
                        value,
                        ..
                    } if value == "0.12"
                )
            })
        }));
        assert!(folded.functions[0].blocks.iter().all(|block| {
            block
                .instrs
                .iter()
                .all(|instruction| !matches!(instruction, Instr::NumericRound { .. }))
        }));
    }

    #[test]
    fn wrapping_builtins_have_distinct_ir() {
        let program = parse(
            r#"
fn main(int left, int right) -> (int, int, int, int) {
    return (
        math::wrapping_add(left: left, right: right),
        math::wrapping_sub(left: left, right: right),
        math::wrapping_mul(left: left, right: right),
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
