//! Intermediate representation for Kotodama programs.
//!
//! The IR is a simple three-address code with basic blocks. Each temporary
//! value is assigned once and identified by a `Temp` index. Control flow is
//! expressed with explicit jumps between labeled blocks.

use std::collections::{BTreeSet, HashMap};

use iroha_primitives::numeric::Numeric;

use super::{
    ast::{BinaryOp, UnaryOp},
    semantic::{
        self, Type, TypedBlock, TypedExpr, TypedFunction, TypedItem, TypedProgram, TypedStatement,
        state_env_snapshot,
    },
};

fn state_map_base_name(expr: &semantic::TypedExpr) -> Option<String> {
    fn rec(expr: &semantic::TypedExpr, indices: &mut Vec<usize>) -> Option<String> {
        match &expr.expr {
            semantic::ExprKind::Member { object, field } => {
                let base = rec(object, indices)?;
                let idx = field.parse::<usize>().ok()?;
                indices.push(idx);
                Some(base)
            }
            semantic::ExprKind::Ident(name) => Some(name.clone()),
            _ => None,
        }
    }

    let mut path = Vec::new();
    let base = rec(expr, &mut path)?;
    if path.is_empty() {
        return Some(base);
    }
    let mut name = base;
    for idx in path {
        name.push('#');
        name.push_str(&idx.to_string());
    }
    Some(name)
}

#[derive(Clone, Debug)]
struct StateMapSpec {
    key: Type,
    value: Type,
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
    Unary {
        dest: Temp,
        op: UnaryOp,
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
        name: Temp,
        symbol: Temp,
        quantity: Temp,
        mintable: Temp,
    },
    /// Helper builtin combining registration and mint into one operation.
    CreateNewAsset {
        name: Temp,
        symbol: Temp,
        quantity: Temp,
        account: Temp,
        mintable: Temp,
    },
    /// Call the `transfer_asset` syscall with from, to, asset and amount parameters.
    TransferAsset {
        from: Temp,
        to: Temp,
        asset: Temp,
        amount: Temp,
    },
    /// Begin a FASTPQ transfer batch scope.
    TransferBatchBegin,
    /// End the current FASTPQ transfer batch scope.
    TransferBatchEnd,
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
    /// Abort execution and revert state changes if `cond` is non-zero.
    ///
    /// This is a non-ZK assertion primitive intended for fast on-chain checks.
    AbortIf {
        cond: Temp,
    },
    /// Log an info message (development only).
    Info {
        msg: Temp,
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
    /// Set JSON data for an NFT.
    SetNftData {
        nft: Temp,
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
    /// Load the current trusted host time in unix milliseconds into `dest`.
    CurrentTimeMs {
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
    /// ZK verify syscalls with NoritoBytes TLV pointer in r10.
    ZkVerify {
        /// Syscall number (0x60..0x63)
        number: u32,
        /// Temp holding pointer to NoritoBytes TLV (INPUT)
        payload: Temp,
    },
    /// Vendor bridge: SMARTCONTRACT_EXECUTE_INSTRUCTION with NoritoBytes `InstructionBox` in r10.
    VendorExecuteInstruction {
        payload: Temp,
    },
    /// Vendor bridge: SMARTCONTRACT_EXECUTE_QUERY with NoritoBytes `QueryRequest` in r10.
    VendorExecuteQuery {
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyCodec {
    Int,
    Pointer,
    NoritoBytes,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ValueCodec {
    Int,
    Pointer(DataRefKind),
    Json,
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
        Type::Blob | Type::Bytes => Some(DataRefKind::Blob),
        _ => None,
    }
}

fn is_pointer_eq_type(ty: &Type) -> bool {
    matches!(
        semantic::resolve_struct_type(ty),
        Type::String | Type::Blob | Type::Bytes | Type::Json
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
        Type::Int => Some(KeyCodec::Int),
        ty if semantic::is_wide_numeric_type(&ty) => Some(KeyCodec::NoritoBytes),
        Type::String => Some(KeyCodec::Pointer),
        other if semantic::is_pointer_type(&other) => Some(KeyCodec::Pointer),
        _ => None,
    }
}

fn value_codec_for_type(ty: &Type) -> Option<ValueCodec> {
    match semantic::resolve_struct_type(ty) {
        Type::Int => Some(ValueCodec::Int),
        ty if semantic::is_wide_numeric_type(&ty) => Some(ValueCodec::NoritoBytes),
        Type::Bool => Some(ValueCodec::Int),
        Type::Json => Some(ValueCodec::Json),
        other if semantic::is_pointer_type(&other) => {
            pointer_kind_for_type(&other).map(ValueCodec::Pointer)
        }
        Type::Blob | Type::Bytes => pointer_kind_for_type(&Type::Blob).map(ValueCodec::Pointer),
        _ => None,
    }
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
    lower_with_cap(program, 2)
}

/// Lower with a specific dynamic-iteration cap used for feature-gated dynamic bounds.
pub fn lower_with_cap(program: &TypedProgram, dyn_iter_cap: usize) -> Result<Program, String> {
    let mut functions = Vec::new();
    for item in &program.items {
        let TypedItem::Function(f) = item;
        functions.push(lower_function(f, dyn_iter_cap)?);
    }
    Ok(Program { functions })
}

fn lower_function(func: &TypedFunction, dyn_iter_cap: usize) -> Result<Function, String> {
    let mut ctx = LowerCtx::new(dyn_iter_cap);
    let entry = ctx.new_label();
    ctx.start_block(entry);
    // Ephemeral state allocation for contract-level `state` declarations.
    let mut vars = HashMap::new();
    let mut param_temps = Vec::new();
    for param in &func.params {
        let tmp = ctx.new_temp();
        param_temps.push((param.clone(), tmp));
        ctx.current_instr(Instr::LoadVar {
            dest: tmp,
            name: param.clone(),
        });
    }
    let state_env = state_env_snapshot();
    for (name, ty) in state_env.iter() {
        allocate_state_value(&mut ctx, &mut vars, name, ty, name);
    }
    for (name, tmp) in param_temps {
        vars.insert(name, tmp);
    }

    lower_block(&mut ctx, &func.body, &mut vars);
    ctx.finish_current(Terminator::Return(None));

    let function = Function {
        name: func.name.clone(),
        params: func.params.clone(),
        blocks: ctx.blocks,
        entry,
    };
    if let Some(err) = ctx.error {
        Err(err)
    } else {
        Ok(function)
    }
}

fn allocate_state_value(
    ctx: &mut LowerCtx,
    vars: &mut HashMap<String, Temp>,
    name: &str,
    ty: &Type,
    literal: &str,
) {
    ctx.state_name_literals
        .insert(name.to_string(), literal.to_string());
    let resolved = semantic::resolve_struct_type(ty);
    match &resolved {
        Type::Map(k, v) => {
            let t = ctx.new_temp();
            ctx.current_instr(Instr::MapNew { dest: t });
            vars.insert(name.to_string(), t);
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
        Type::Struct { fields, .. } => {
            for (i, (fname, fty)) in fields.iter().enumerate() {
                let child = format!("{name}#{i}");
                let child_literal = format!("{literal}_{fname}");
                allocate_state_value(ctx, vars, &child, fty, &child_literal);
            }
        }
        Type::Tuple(ts) => {
            for (i, fty) in ts.iter().enumerate() {
                let child = format!("{name}#{i}");
                let child_literal = format!("{literal}_{i}");
                allocate_state_value(ctx, vars, &child, fty, &child_literal);
            }
        }
        _ => {
            let t = ctx.new_temp();
            ctx.current_instr(Instr::Const { dest: t, value: 0 });
            vars.insert(name.to_string(), t);
        }
    }
}

fn push_copy(block: &mut BasicBlock, dest: Temp, src: Temp) {
    if dest == src {
        return;
    }
    block.instrs.push(Instr::Copy { dest, src });
}

fn initialize_loop_phi(ctx: &mut LowerCtx, vars: &HashMap<String, Temp>) -> HashMap<String, Temp> {
    let mut phi = HashMap::new();
    for (name, temp) in vars {
        let slot = ctx.new_temp();
        ctx.current_instr(Instr::Copy {
            dest: slot,
            src: *temp,
        });
        phi.insert(name.clone(), slot);
    }
    phi
}

fn copy_env_to_loop_phi(ctx: &mut LowerCtx, env: &HashMap<String, Temp>) {
    if let Some(phi) = ctx.current_loop_phi() {
        let mut copies: Vec<(Temp, Temp)> = Vec::new();
        for (name, dest) in phi {
            if let Some(src) = env.get(name) {
                copies.push((*dest, *src));
            }
        }
        for (dest, src) in copies {
            ctx.current_instr(Instr::Copy { dest, src });
        }
    }
}

fn lower_block(ctx: &mut LowerCtx, block: &TypedBlock, vars: &mut HashMap<String, Temp>) {
    for stmt in &block.statements {
        lower_statement(ctx, stmt, vars);
    }
}

fn lower_statement(ctx: &mut LowerCtx, stmt: &TypedStatement, vars: &mut HashMap<String, Temp>) {
    match stmt {
        TypedStatement::Let { name, value } => {
            let t = lower_expr(ctx, value, vars);
            vars.insert(name.clone(), t);
            if ctx.state_name_literals.contains_key(name) {
                emit_scalar_state_set(ctx, name, &value.ty, t);
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
            lower_block(ctx, then_branch, &mut then_vars);
            ctx.finish_current(Terminator::Jump(end_label));
            let then_idx = ctx.blocks.len() - 1;

            ctx.start_block(else_label);
            let mut else_vars = entry_env.clone();
            if let Some(b) = else_branch {
                lower_block(ctx, b, &mut else_vars);
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
            let loop_phi = initialize_loop_phi(ctx, vars);
            ctx.push_loop(cond_label, end_label);
            ctx.set_loop_phi(loop_phi.clone());
            ctx.finish_current(Terminator::Jump(cond_label));

            ctx.start_block(cond_label);
            let mut cond_vars = loop_phi.clone();
            let cond_t = lower_expr(ctx, cond, &mut cond_vars);
            ctx.finish_current(Terminator::Branch {
                cond: cond_t,
                then_bb: body_label,
                else_bb: end_label,
            });

            ctx.start_block(body_label);
            let mut body_vars = cond_vars;
            lower_block(ctx, body, &mut body_vars);
            copy_env_to_loop_phi(ctx, &body_vars);
            ctx.finish_current(Terminator::Jump(cond_label));

            ctx.pop_loop();
            ctx.start_block(end_label);
            *vars = loop_phi;
        }
        TypedStatement::For {
            line: _,
            init,
            cond,
            step,
            body,
        } => {
            if let Some(s) = init {
                lower_statement(ctx, s, vars);
            }
            let cond_label = ctx.new_label();
            let body_label = ctx.new_label();
            let step_label = ctx.new_label();
            let end_label = ctx.new_label();
            let loop_phi = initialize_loop_phi(ctx, vars);
            ctx.push_loop(step_label, end_label);
            ctx.set_loop_phi(loop_phi.clone());
            ctx.finish_current(Terminator::Jump(cond_label));

            ctx.start_block(cond_label);
            let mut cond_vars = loop_phi.clone();
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
            lower_block(ctx, body, &mut body_vars);
            copy_env_to_loop_phi(ctx, &body_vars);
            ctx.finish_current(Terminator::Jump(step_label));

            ctx.start_block(step_label);
            if let Some(s) = step {
                let mut step_vars = loop_phi.clone();
                lower_statement(ctx, s, &mut step_vars);
                copy_env_to_loop_phi(ctx, &step_vars);
            }
            ctx.finish_current(Terminator::Jump(cond_label));

            ctx.pop_loop();
            ctx.start_block(end_label);
            *vars = loop_phi;
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
                // If return type is a tuple, extract elements via TupleGet regardless of expression form
                if let semantic::Type::Tuple(ts) = &e.ty {
                    let tup = lower_expr(ctx, e, vars);
                    let mut outs = Vec::with_capacity(ts.len());
                    for i in 0..ts.len() {
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
        #[cfg(feature = "kotodama_dynamic_bounds")]
        TypedStatement::ForEachMap {
            key,
            value,
            map,
            body,
            start,
            bound: _,
            dyn_count: Some(dyn_n),
            dyn_start,
        } => {
            let base = lower_expr(ctx, map, vars);
            let base_start_lit: i16 = (*start as i16).max(0);
            // Lower dynamic `n` once
            let n = lower_expr(ctx, dyn_n, vars);
            // Guarded unrolling up to cap
            for i in 0..ctx._dyn_iter_cap {
                // cond = (i < n)
                let ti = ctx.new_temp();
                ctx.current_instr(Instr::Const {
                    dest: ti,
                    value: i as i64,
                });
                let cond_t = ctx.new_temp();
                ctx.current_instr(Instr::Binary {
                    dest: cond_t,
                    op: BinaryOp::Lt,
                    left: ti,
                    right: n,
                });
                let then_bb = ctx.new_label();
                let cont_bb = ctx.new_label();
                ctx.finish_current(Terminator::Branch {
                    cond: cond_t,
                    then_bb,
                    else_bb: cont_bb,
                });

                // Then block: perform iteration i
                ctx.start_block(then_bb);
                // Compute j = dyn_start.unwrap_or(literal_start) + i
                let mut index_temp = ti;
                if let Some(ds) = dyn_start {
                    let ds_t = lower_expr(ctx, ds, vars);
                    let sum = ctx.new_temp();
                    ctx.current_instr(Instr::Binary {
                        dest: sum,
                        op: BinaryOp::Add,
                        left: ds_t,
                        right: index_temp,
                    });
                    index_temp = sum;
                } else if base_start_lit != 0 {
                    // add literal start
                    let s = ctx.new_temp();
                    ctx.current_instr(Instr::Const {
                        dest: s,
                        value: base_start_lit as i64,
                    });
                    let sum = ctx.new_temp();
                    ctx.current_instr(Instr::Binary {
                        dest: sum,
                        op: BinaryOp::Add,
                        left: s,
                        right: index_temp,
                    });
                    index_temp = sum;
                }
                // bytes = index_temp * 16
                let sixteen = ctx.new_temp();
                ctx.current_instr(Instr::Const {
                    dest: sixteen,
                    value: 16,
                });
                let bytes = ctx.new_temp();
                ctx.current_instr(Instr::Binary {
                    dest: bytes,
                    op: BinaryOp::Mul,
                    left: index_temp,
                    right: sixteen,
                });
                // addr = base + bytes
                let addr = ctx.new_temp();
                ctx.current_instr(Instr::Binary {
                    dest: addr,
                    op: BinaryOp::Add,
                    left: base,
                    right: bytes,
                });
                let k = ctx.new_temp();
                let v = ctx.new_temp();
                // Load key and value using addr + 0 and addr + 8
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
                vars.insert(key.clone(), k);
                if let Some(val_name) = value {
                    vars.insert(val_name.clone(), v);
                }
                lower_block(ctx, body, vars);
                ctx.finish_current(Terminator::Jump(cont_bb));
                ctx.start_block(cont_bb);
            }
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
                && let Some(spec) = ctx.state_map_configs.get(&base_name)
                && matches!(key_codec_for_type(&spec.key), Some(KeyCodec::Int))
                && let Some(value_codec) = value_codec_for_type(&spec.value)
            {
                lower_state_foreach_int_map(
                    ctx,
                    key,
                    value,
                    map,
                    body,
                    *start,
                    *bound,
                    &base_name,
                    value_codec,
                    vars,
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
            let mut body_vars = vars.clone();
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
            ctx.push_loop(step_label, exit_label);
            lower_block(ctx, body, &mut body_vars);
            ctx.pop_loop();
            ctx.finish_current(Terminator::Jump(step_label));

            ctx.start_block(step_label);
            ctx.current_instr(Instr::Binary {
                dest: index,
                op: BinaryOp::Add,
                left: index,
                right: one,
            });
            ctx.finish_current(Terminator::Jump(loop_label));

            ctx.start_block(exit_label);
        }
        TypedStatement::MapSet { map, key, value } => {
            let key_tmp = lower_expr(ctx, key, vars);
            let value_tmp = lower_expr(ctx, value, vars);
            if let Some(bn) = state_map_base_name(map)
                && let Some(spec) = ctx.state_map_configs.get(&bn)
                && let (Some(key_codec), Some(value_codec)) = (
                    key_codec_for_type(&spec.key),
                    value_codec_for_type(&spec.value),
                )
            {
                let path = build_state_path(ctx, &bn, key_tmp, &key_codec);
                let encoded = encode_value_to_norito(ctx, value_tmp, &value_codec);
                ctx.current_instr(Instr::StateSet {
                    path,
                    value: encoded,
                });
            }
            // Always keep ephemeral path for within-run semantics
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
fn lower_state_foreach_int_map(
    ctx: &mut LowerCtx,
    key: &str,
    value: &Option<String>,
    map: &TypedExpr,
    body: &TypedBlock,
    start: usize,
    bound: Option<usize>,
    base_name: &str,
    value_codec: ValueCodec,
    vars: &mut HashMap<String, Temp>,
) {
    // Evaluate map expression for potential side effects.
    let _ = lower_expr(ctx, map, vars);
    let max_iters = bound.unwrap_or(2);
    let max_iters_i64 = (max_iters.min(i64::MAX as usize)) as i64;
    let base_start_i64 = (start.min(i64::MAX as usize)) as i64;
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
    let one = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: one,
        value: 1,
    });

    let loop_label = ctx.new_label();
    let body_label = ctx.new_label();
    let step_label = ctx.new_label();
    let exit_label = ctx.new_label();
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
    // Build path for current index and fetch durable value: Name(base/index) -> NoritoBytes
    let path = build_state_path(ctx, base_name, index, &KeyCodec::Int);
    let blob = ctx.new_temp();
    ctx.current_instr(Instr::StateGet { dest: blob, path });
    // Skip body when entry is absent (blob == 0).
    let zero = ctx.new_temp();
    ctx.current_instr(Instr::Const {
        dest: zero,
        value: 0,
    });
    let has_value = ctx.new_temp();
    ctx.current_instr(Instr::Binary {
        dest: has_value,
        op: BinaryOp::Ne,
        left: blob,
        right: zero,
    });
    let present_bb = ctx.new_label();
    ctx.finish_current(Terminator::Branch {
        cond: has_value,
        then_bb: present_bb,
        else_bb: step_label,
    });

    ctx.start_block(present_bb);
    let value_temp = decode_value_from_norito(ctx, blob, &value_codec);
    let key_temp = ctx.new_temp();
    ctx.current_instr(Instr::Copy {
        dest: key_temp,
        src: index,
    });
    let mut body_vars = vars.clone();
    body_vars.insert(key.to_string(), key_temp);
    if let Some(val_name) = value {
        body_vars.insert(val_name.clone(), value_temp);
    }
    ctx.push_loop(step_label, exit_label);
    lower_block(ctx, body, &mut body_vars);
    ctx.pop_loop();
    ctx.finish_current(Terminator::Jump(step_label));

    ctx.start_block(step_label);
    ctx.current_instr(Instr::Binary {
        dest: index,
        op: BinaryOp::Add,
        left: index,
        right: one,
    });
    ctx.finish_current(Terminator::Jump(loop_label));

    ctx.start_block(exit_label);
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
            match raw.parse::<Numeric>() {
                Ok(numeric) => {
                    if numeric.scale() != 0 || numeric.mantissa().is_negative() {
                        ctx.record_error(format!(
                            "numeric literal `{raw}` must be an unsigned integer (scale=0)"
                        ));
                        ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    } else {
                        let hex = hex::encode(numeric.encode());
                        ctx.current_instr(Instr::DataRef {
                            dest: t,
                            kind: DataRefKind::NoritoBytes,
                            value: format!("0x{hex}"),
                        });
                    }
                }
                Err(err) => {
                    ctx.record_error(format!("invalid numeric literal `{raw}`: {err}"));
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
            if let Some(literal) = ctx.state_name_literals.get(name).cloned()
                && !ctx.loaded_state_fields.contains(name)
                && let Some(value) = lower_state_field(ctx, &literal, &expr.ty)
            {
                ctx.loaded_state_fields.insert(name.clone());
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
            match name.as_str() {
                "encode_int" => {
                    let v = lower_expr(ctx, &args[0], vars);
                    let d = ctx.new_temp();
                    ctx.current_instr(Instr::EncodeInt { dest: d, value: v });
                    d
                }
                "decode_int" => {
                    let b = lower_expr(ctx, &args[0], vars);
                    let d = ctx.new_temp();
                    ctx.current_instr(Instr::DecodeInt { dest: d, blob: b });
                    d
                }
                "encode_json" => {
                    let j = lower_expr(ctx, &args[0], vars);
                    let d = ctx.new_temp();
                    ctx.current_instr(Instr::JsonEncode { dest: d, json: j });
                    d
                }
                "decode_json" => {
                    let b = lower_expr(ctx, &args[0], vars);
                    let d = ctx.new_temp();
                    ctx.current_instr(Instr::JsonDecode { dest: d, blob: b });
                    d
                }
                "tlv_len" => {
                    let v = lower_expr(ctx, &args[0], vars);
                    let d = ctx.new_temp();
                    ctx.current_instr(Instr::TlvLen { dest: d, value: v });
                    d
                }
                "json_get_int" => {
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
                "json_get_numeric" => {
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
                "json_get_json" => {
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
                "json_get_name" => {
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
                "json_get_account_id" => {
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
                "json_get_nft_id" => {
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
                "json_get_blob_hex" => {
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
                "encode_schema" => {
                    let s = lower_expr(ctx, &args[0], vars);
                    let j = lower_expr(ctx, &args[1], vars);
                    let d = ctx.new_temp();
                    ctx.current_instr(Instr::SchemaEncode {
                        dest: d,
                        schema: s,
                        json: j,
                    });
                    d
                }
                "decode_schema" => {
                    let s = lower_expr(ctx, &args[0], vars);
                    let b = lower_expr(ctx, &args[1], vars);
                    let d = ctx.new_temp();
                    ctx.current_instr(Instr::SchemaDecode {
                        dest: d,
                        schema: s,
                        blob: b,
                    });
                    d
                }
                "schema_info" => {
                    let s = lower_expr(ctx, &args[0], vars);
                    let d = ctx.new_temp();
                    ctx.current_instr(Instr::SchemaInfo { dest: d, schema: s });
                    d
                }
                "pointer_to_norito" => {
                    let v = lower_expr(ctx, &args[0], vars);
                    let d = ctx.new_temp();
                    ctx.current_instr(Instr::PointerToNorito { dest: d, value: v });
                    d
                }
                "path_map_key" => {
                    let base = lower_expr(ctx, &args[0], vars);
                    let key = lower_expr_as_int(ctx, &args[1], vars);
                    let d = ctx.new_temp();
                    ctx.current_instr(Instr::PathMapKey { dest: d, base, key });
                    d
                }
                "path_map_key_norito" => {
                    let base = lower_expr(ctx, &args[0], vars);
                    let blob = lower_expr(ctx, &args[1], vars);
                    let d = ctx.new_temp();
                    ctx.current_instr(Instr::PathMapKeyNorito {
                        dest: d,
                        base,
                        key_blob: blob,
                    });
                    d
                }
                "state_get" => {
                    let p = lower_expr(ctx, &args[0], vars);
                    let d = ctx.new_temp();
                    ctx.current_instr(Instr::StateGet { dest: d, path: p });
                    d
                }
                "state_set" => {
                    let path = lower_expr(ctx, &args[0], vars);
                    let val = lower_expr(ctx, &args[1], vars);
                    ctx.current_instr(Instr::StateSet { path, value: val });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "state_del" => {
                    let path = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::StateDel { path });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                // Pointer-ABI constructors: accept string literals (or temps derived from them)
                // and lower to typed pointers that codegen wires into Norito TLV fixups.
                "account_id" | "asset_definition" | "asset_id" | "nft_id" | "name" | "json"
                | "domain" | "domain_id" | "blob" | "norito_bytes" | "dataspace_id"
                | "axt_descriptor" | "asset_handle" | "proof_blob" => {
                    // Expect a single argument
                    if args.len() != 1 {
                        let t = ctx.new_temp();
                        ctx.current_instr(Instr::Const { dest: t, value: 0 });
                        return t;
                    }
                    let arg = &args[0];
                    let (kind, target_ty) = match name.as_str() {
                        "account_id" => (DataRefKind::Account, Type::AccountId),
                        "asset_definition" => (DataRefKind::AssetDef, Type::AssetDefinitionId),
                        "asset_id" => (DataRefKind::AssetId, Type::AssetId),
                        "nft_id" => (DataRefKind::NftId, Type::NftId),
                        "domain" | "domain_id" => (DataRefKind::Domain, Type::DomainId),
                        "name" => (DataRefKind::Name, Type::Name),
                        "json" => (DataRefKind::Json, Type::Json),
                        "blob" => (DataRefKind::Blob, Type::Bytes),
                        "norito_bytes" => (DataRefKind::NoritoBytes, Type::Bytes),
                        "dataspace_id" => (DataRefKind::DataSpaceId, Type::DataSpaceId),
                        "axt_descriptor" => (DataRefKind::AxtDescriptor, Type::AxtDescriptor),
                        "asset_handle" => (DataRefKind::AssetHandle, Type::AssetHandle),
                        "proof_blob" => (DataRefKind::ProofBlob, Type::ProofBlob),
                        _ => unreachable!(),
                    };
                    if let semantic::ExprKind::String(s) = &arg.expr {
                        let dest = ctx.new_temp();
                        ctx.current_instr(Instr::DataRef {
                            dest,
                            kind,
                            value: s.clone(),
                        });
                        return dest;
                    }
                    if name == "norito_bytes"
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
                        (Type::Blob | Type::Bytes, _) => src,
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
                "Map::new" | "map_new" => {
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::MapNew { dest: t });
                    t
                }
                "assert" => {
                    let cond = lower_expr(ctx, &args[0], vars);
                    if args.len() > 1 {
                        let _ = lower_expr(ctx, &args[1], vars);
                    }
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Unary {
                        dest: t,
                        op: UnaryOp::Not,
                        operand: cond,
                    });
                    ctx.current_instr(Instr::Assert { cond: t });
                    let u = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: u, value: 0 });
                    u
                }
                "require" => {
                    let cond = lower_expr(ctx, &args[0], vars);
                    if args.len() > 1 {
                        let _ = lower_expr(ctx, &args[1], vars);
                    }
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Unary {
                        dest: t,
                        op: UnaryOp::Not,
                        operand: cond,
                    });
                    ctx.current_instr(Instr::AbortIf { cond: t });
                    let u = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: u, value: 0 });
                    u
                }
                "info" => {
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
                "create_nfts_for_all_users" => {
                    ctx.current_instr(Instr::CreateNftsForAllUsers);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "set_execution_depth" => {
                    let v = lower_expr_as_int(ctx, &args[0], vars);
                    ctx.current_instr(Instr::SetExecutionDepth { value: v });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "setvl" => {
                    let v = lower_expr_as_int(ctx, &args[0], vars);
                    ctx.current_instr(Instr::SetVl { value: v });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "set_account_detail" => {
                    let a = lower_expr(ctx, &args[0], vars);
                    let k = lower_expr(ctx, &args[1], vars);
                    let v = lower_expr(ctx, &args[2], vars);
                    ctx.current_instr(Instr::SetAccountDetail {
                        account: a,
                        key: k,
                        value: v,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                // Typed constructors for pointer-ABI
                "authority" => {
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::GetAuthority { dest: t });
                    t
                }
                "current_time_ms" => {
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::CurrentTimeMs { dest: t });
                    t
                }
                "trigger_event" => {
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::GetTriggerEvent { dest: t });
                    t
                }
                "isqrt" => {
                    let v = lower_expr_as_int(ctx, &args[0], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Isqrt { dest: t, src: v });
                    t
                }
                "abs" => {
                    let v = lower_expr_as_int(ctx, &args[0], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Abs { dest: t, src: v });
                    t
                }
                "min" => {
                    let a = lower_expr_as_int(ctx, &args[0], vars);
                    let b = lower_expr_as_int(ctx, &args[1], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Min { dest: t, a, b });
                    t
                }
                "max" => {
                    let a = lower_expr_as_int(ctx, &args[0], vars);
                    let b = lower_expr_as_int(ctx, &args[1], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Max { dest: t, a, b });
                    t
                }
                "div_ceil" => {
                    let num = lower_expr_as_int(ctx, &args[0], vars);
                    let denom = lower_expr_as_int(ctx, &args[1], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::DivCeil {
                        dest: t,
                        num,
                        denom,
                    });
                    t
                }
                "gcd" => {
                    let a = lower_expr_as_int(ctx, &args[0], vars);
                    let b = lower_expr_as_int(ctx, &args[1], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Gcd { dest: t, a, b });
                    t
                }
                "mean" => {
                    let a = lower_expr_as_int(ctx, &args[0], vars);
                    let b = lower_expr_as_int(ctx, &args[1], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Mean { dest: t, a, b });
                    t
                }
                "poseidon2" => {
                    let a = lower_expr_as_int(ctx, &args[0], vars);
                    let b = lower_expr_as_int(ctx, &args[1], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Poseidon2 { dest: t, a, b });
                    t
                }
                "nft_mint_asset" => {
                    let nft = lower_expr(ctx, &args[0], vars);
                    let owner = lower_expr(ctx, &args[1], vars);
                    ctx.current_instr(Instr::CreateNft { nft, owner });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "nft_set_metadata" => {
                    let nft = lower_expr(ctx, &args[0], vars);
                    let json = lower_expr(ctx, &args[1], vars);
                    ctx.current_instr(Instr::SetNftData { nft, json });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "nft_burn_asset" => {
                    let nft = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::BurnNft { nft });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "nft_transfer_asset" => {
                    let from = lower_expr(ctx, &args[0], vars);
                    let nft = lower_expr(ctx, &args[1], vars);
                    let to = lower_expr(ctx, &args[2], vars);
                    ctx.current_instr(Instr::TransferNft { from, nft, to });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "poseidon6" => {
                    let mut arr = [Temp(0); 6];
                    for (i, arg) in args.iter().enumerate() {
                        arr[i] = lower_expr_as_int(ctx, arg, vars);
                    }
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Poseidon6 { dest: t, args: arr });
                    t
                }
                "sm3_hash" => {
                    let msg = lower_expr(ctx, &args[0], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Sm3Hash {
                        dest: t,
                        message: msg,
                    });
                    t
                }
                "sha256_hash" => {
                    let msg = lower_expr(ctx, &args[0], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Sha256Hash {
                        dest: t,
                        message: msg,
                    });
                    t
                }
                "sha3_hash" => {
                    let msg = lower_expr(ctx, &args[0], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Sha3Hash {
                        dest: t,
                        message: msg,
                    });
                    t
                }
                "sm2_verify" => {
                    let msg = lower_expr(ctx, &args[0], vars);
                    let sig = lower_expr(ctx, &args[1], vars);
                    let pk = lower_expr(ctx, &args[2], vars);
                    let distid = if args.len() == 4 {
                        Some(lower_expr(ctx, &args[3], vars))
                    } else {
                        None
                    };
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Sm2Verify {
                        dest: t,
                        message: msg,
                        signature: sig,
                        public_key: pk,
                        distid,
                    });
                    t
                }
                "verify_signature" => {
                    let msg = lower_expr(ctx, &args[0], vars);
                    let sig = lower_expr(ctx, &args[1], vars);
                    let pk = lower_expr(ctx, &args[2], vars);
                    let scheme = lower_expr(ctx, &args[3], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::VerifySignature {
                        dest: t,
                        message: msg,
                        signature: sig,
                        public_key: pk,
                        scheme,
                    });
                    t
                }
                "sm4_gcm_seal" => {
                    let key = lower_expr(ctx, &args[0], vars);
                    let nonce = lower_expr(ctx, &args[1], vars);
                    let aad = lower_expr(ctx, &args[2], vars);
                    let plaintext = lower_expr(ctx, &args[3], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Sm4GcmSeal {
                        dest: t,
                        key,
                        nonce,
                        aad,
                        plaintext,
                    });
                    t
                }
                "sm4_gcm_open" => {
                    let key = lower_expr(ctx, &args[0], vars);
                    let nonce = lower_expr(ctx, &args[1], vars);
                    let aad = lower_expr(ctx, &args[2], vars);
                    let cipher = lower_expr(ctx, &args[3], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Sm4GcmOpen {
                        dest: t,
                        key,
                        nonce,
                        aad,
                        ciphertext_and_tag: cipher,
                    });
                    t
                }
                "sm4_ccm_seal" => {
                    let key = lower_expr(ctx, &args[0], vars);
                    let nonce = lower_expr(ctx, &args[1], vars);
                    let aad = lower_expr(ctx, &args[2], vars);
                    let plaintext = lower_expr(ctx, &args[3], vars);
                    let tag_len = if args.len() == 5 {
                        Some(lower_expr(ctx, &args[4], vars))
                    } else {
                        None
                    };
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Sm4CcmSeal {
                        dest: t,
                        key,
                        nonce,
                        aad,
                        plaintext,
                        tag_len,
                    });
                    t
                }
                "sm4_ccm_open" => {
                    let key = lower_expr(ctx, &args[0], vars);
                    let nonce = lower_expr(ctx, &args[1], vars);
                    let aad = lower_expr(ctx, &args[2], vars);
                    let cipher = lower_expr(ctx, &args[3], vars);
                    let tag_len = if args.len() == 5 {
                        Some(lower_expr(ctx, &args[4], vars))
                    } else {
                        None
                    };
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Sm4CcmOpen {
                        dest: t,
                        key,
                        nonce,
                        aad,
                        ciphertext_and_tag: cipher,
                        tag_len,
                    });
                    t
                }
                "contains" => {
                    // contains(map, key) -> bool
                    let mexpr = &args[0];
                    let kexpr = &args[1];
                    let key_tmp = lower_expr(ctx, kexpr, vars);
                    // Durable path when map is a tracked state map name
                    if let Some(bn) = state_map_base_name(mexpr)
                        && let Some(spec) = ctx.state_map_configs.get(&bn)
                        && let Some(key_codec) = key_codec_for_type(&spec.key)
                    {
                        // Durable check: build path and STATE_GET; return (r10 != 0)
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
                    // Ephemeral path: load stored key and compare equality
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
                "has" => {
                    // Alias to contains(map, key) -> bool
                    let mexpr = &args[0];
                    let kexpr = &args[1];
                    let key_tmp = lower_expr(ctx, kexpr, vars);
                    // Durable path when map is a tracked state map name
                    if let Some(bn) = state_map_base_name(mexpr)
                        && let Some(spec) = ctx.state_map_configs.get(&bn)
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
                    // Ephemeral path
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
                "get_or_default" => {
                    let mexpr = &args[0];
                    let kexpr = &args[1];
                    let dexpr = &args[2];
                    let key_tmp = lower_expr(ctx, kexpr, vars);
                    // Durable path
                    if let Some(bn) = state_map_base_name(mexpr)
                        && let Some(spec) = ctx.state_map_configs.get(&bn)
                        && let (Some(key_codec), Some(value_codec)) = (
                            key_codec_for_type(&spec.key),
                            value_codec_for_type(&spec.value),
                        )
                    {
                        // path -> STATE_GET -> branch on blob != 0
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
                        // Build branch on t_blob != 0
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
                        // Then: decode stored value and return it
                        ctx.start_block(then_bb);
                        let decoded = decode_value_from_norito(ctx, t_blob, &value_codec);
                        ctx.current_instr(Instr::Binary {
                            dest: result,
                            op: BinaryOp::Add,
                            left: decoded,
                            right: zero,
                        });
                        ctx.finish_current(Terminator::Jump(end_bb));
                        // Else: return default expression
                        ctx.start_block(else_bb);
                        let def = lower_expr(ctx, dexpr, vars);
                        ctx.current_instr(Instr::Binary {
                            dest: result,
                            op: BinaryOp::Add,
                            left: def,
                            right: zero,
                        });
                        ctx.finish_current(Terminator::Jump(end_bb));
                        // End
                        ctx.start_block(end_bb);
                        return result; // preserve original logic
                    }
                    // Ephemeral: compare key and return value or default
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
                    // Then: return sv
                    ctx.start_block(then_bb);
                    ctx.current_instr(Instr::Binary {
                        dest: result,
                        op: BinaryOp::Add,
                        left: sv,
                        right: zero,
                    });
                    ctx.finish_current(Terminator::Jump(end_bb));
                    // Else: return d
                    ctx.start_block(else_bb);
                    ctx.current_instr(Instr::Binary {
                        dest: result,
                        op: BinaryOp::Add,
                        left: d,
                        right: zero,
                    });
                    ctx.finish_current(Terminator::Jump(end_bb));
                    // End and return the selected result
                    ctx.start_block(end_bb);
                    result
                }
                "get_or_insert_default" => {
                    // Like get_or_default(map, key, default) but inserts the default deterministically when missing.
                    let mexpr = &args[0];
                    let kexpr = &args[1];
                    let dexpr = &args[2];
                    let key_tmp = lower_expr(ctx, kexpr, vars);
                    // Durable path
                    if let Some(bn) = state_map_base_name(mexpr)
                        && let Some(spec) = ctx.state_map_configs.get(&bn)
                        && let (Some(key_codec), Some(value_codec)) = (
                            key_codec_for_type(&spec.key),
                            value_codec_for_type(&spec.value),
                        )
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
                        // Then: decode existing value and return it
                        ctx.start_block(then_bb);
                        let existing = decode_value_from_norito(ctx, t_blob, &value_codec);
                        ctx.current_instr(Instr::Binary {
                            dest: result,
                            op: BinaryOp::Add,
                            left: existing,
                            right: zero,
                        });
                        ctx.finish_current(Terminator::Jump(end_bb));
                        // Else: encode default, persist, and return default
                        ctx.start_block(else_bb);
                        let def = lower_expr(ctx, dexpr, vars);
                        let encoded = encode_value_to_norito(ctx, def, &value_codec);
                        ctx.current_instr(Instr::StateSet {
                            path: t_path,
                            value: encoded,
                        });
                        ctx.current_instr(Instr::Binary {
                            dest: result,
                            op: BinaryOp::Add,
                            left: def,
                            right: zero,
                        });
                        ctx.finish_current(Terminator::Jump(end_bb));
                        // End and return result
                        ctx.start_block(end_bb);
                        return result;
                    }
                    // Ephemeral path: compare and insert default if mismatch
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
                    // Then: return existing value
                    ctx.start_block(then_bb);
                    ctx.current_instr(Instr::Binary {
                        dest: result,
                        op: BinaryOp::Add,
                        left: sv,
                        right: zero,
                    });
                    ctx.finish_current(Terminator::Jump(end_bb));
                    // Else: insert default and return it
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
                    // End and return the selected result
                    ctx.start_block(end_bb);
                    result
                }
                "keys_take2" | "values_take2" => {
                    // Fixed-cap guarded iterator helper: return map key/value at (start + (which & 1))
                    // Only ephemeral maps are supported here.
                    let base = lower_expr(ctx, &args[0], vars);
                    let start_t = lower_expr_as_int(ctx, &args[1], vars);
                    let which_t = lower_expr_as_int(ctx, &args[2], vars);
                    // mask = which & 1
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
                    // index = start + masked
                    let idx = ctx.new_temp();
                    ctx.current_instr(Instr::Binary {
                        dest: idx,
                        op: BinaryOp::Add,
                        left: start_t,
                        right: masked,
                    });
                    // bytes = idx * 16
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
                    // addr = base + bytes
                    let addr = ctx.new_temp();
                    ctx.current_instr(Instr::Binary {
                        dest: addr,
                        op: BinaryOp::Add,
                        left: base,
                        right: bytes,
                    });
                    // Load pair at addr
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
                    if name == "keys_take2" { k } else { v }
                }
                "keys_values_take2" => {
                    // Build pair (k,v) at start + (which & 1) and return as tuple
                    let base = lower_expr(ctx, &args[0], vars);
                    let start_t = lower_expr_as_int(ctx, &args[1], vars);
                    let which_t = lower_expr_as_int(ctx, &args[2], vars);
                    // mask which to 0/1
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
                    // idx = start + masked
                    let idx = ctx.new_temp();
                    ctx.current_instr(Instr::Binary {
                        dest: idx,
                        op: BinaryOp::Add,
                        left: start_t,
                        right: masked,
                    });
                    // bytes = idx * 16
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
                    // addr = base + bytes
                    let addr = ctx.new_temp();
                    ctx.current_instr(Instr::Binary {
                        dest: addr,
                        op: BinaryOp::Add,
                        left: base,
                        right: bytes,
                    });
                    // Load key and value
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
                // ZK verify intrinsics
                "zk_verify_transfer" => {
                    let p = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::ZkVerify {
                        number: crate::syscalls::SYSCALL_ZK_VERIFY_TRANSFER,
                        payload: p,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "zk_verify_unshield" => {
                    let p = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::ZkVerify {
                        number: crate::syscalls::SYSCALL_ZK_VERIFY_UNSHIELD,
                        payload: p,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "zk_verify_batch" => {
                    let p = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::ZkVerify {
                        number: crate::syscalls::SYSCALL_ZK_VERIFY_BATCH,
                        payload: p,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "zk_vote_verify_ballot" => {
                    let p = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::ZkVerify {
                        number: crate::syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT,
                        payload: p,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "zk_vote_verify_tally" => {
                    let p = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::ZkVerify {
                        number: crate::syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY,
                        payload: p,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                // Vendor bridge wrappers
                "sc_execute_submit_ballot" | "sc_execute_unshield" | "execute_instruction" => {
                    let p = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::VendorExecuteInstruction { payload: p });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "execute_query" => {
                    let p = lower_expr(ctx, &args[0], vars);
                    let d = ctx.new_temp();
                    ctx.current_instr(Instr::VendorExecuteQuery {
                        dest: d,
                        payload: p,
                    });
                    d
                }
                "subscription_bill" => {
                    ctx.current_instr(Instr::SubscriptionBill);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "subscription_record_usage" => {
                    ctx.current_instr(Instr::SubscriptionRecordUsage);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "resolve_account_alias" => {
                    let alias = lower_expr(ctx, &args[0], vars);
                    let dest = ctx.new_temp();
                    ctx.current_instr(Instr::ResolveAccountAlias { dest, alias });
                    dest
                }
                "build_submit_ballot_inline" => {
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
                "build_unshield_inline" => {
                    let asset = lower_expr(ctx, &args[0], vars);
                    let to = lower_expr(ctx, &args[1], vars);
                    let amount = lower_expr_as_int(ctx, &args[2], vars);
                    let inputs = lower_expr(ctx, &args[3], vars);
                    let backend = lower_expr(ctx, &args[4], vars);
                    let proof = lower_expr(ctx, &args[5], vars);
                    let vk = lower_expr(ctx, &args[6], vars);
                    let dest = ctx.new_temp();
                    ctx.current_instr(Instr::BuildUnshieldInline {
                        dest,
                        asset,
                        to,
                        amount,
                        inputs,
                        backend,
                        proof,
                        vk,
                    });
                    dest
                }
                "valcom" => {
                    let v = lower_expr_as_int(ctx, &args[0], vars);
                    let bl = lower_expr_as_int(ctx, &args[1], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Valcom {
                        dest: t,
                        value: v,
                        blind: bl,
                    });
                    t
                }
                "pubkgen" => {
                    let s = lower_expr_as_int(ctx, &args[0], vars);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Pubkgen { dest: t, src: s });
                    t
                }
                "vrf_verify" => {
                    let input = lower_expr(ctx, &args[0], vars);
                    let pk = lower_expr(ctx, &args[1], vars);
                    let proof = lower_expr(ctx, &args[2], vars);
                    let variant = lower_expr(ctx, &args[3], vars);
                    let dest = ctx.new_temp();
                    ctx.current_instr(Instr::VrfVerify {
                        dest,
                        input,
                        public_key: pk,
                        proof,
                        variant,
                    });
                    dest
                }
                "vrf_verify_batch" => {
                    let batch = lower_expr(ctx, &args[0], vars);
                    let dest = ctx.new_temp();
                    ctx.current_instr(Instr::VrfVerifyBatch { dest, batch });
                    dest
                }
                "axt_begin" => {
                    let desc = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::AxtBegin { descriptor: desc });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "axt_touch" => {
                    let dsid = lower_expr(ctx, &args[0], vars);
                    let manifest = args.get(1).map(|m| lower_expr(ctx, m, vars));
                    ctx.current_instr(Instr::AxtTouch { dsid, manifest });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "verify_ds_proof" => {
                    let dsid = lower_expr(ctx, &args[0], vars);
                    let proof = args.get(1).map(|p| lower_expr(ctx, p, vars));
                    ctx.current_instr(Instr::VerifyDsProof { dsid, proof });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "use_asset_handle" => {
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
                "axt_commit" => {
                    ctx.current_instr(Instr::AxtCommit);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "mint_asset" => {
                    let acc = lower_expr(ctx, &args[0], vars);
                    let asset = lower_expr(ctx, &args[1], vars);
                    let amt = match &args[2].expr {
                        semantic::ExprKind::Number(0)
                            if !semantic::is_wide_numeric_type(&args[2].ty) =>
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
                "burn_asset" => {
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
                "transfer_asset" => {
                    let from = lower_expr(ctx, &args[0], vars);
                    let to = lower_expr(ctx, &args[1], vars);
                    let asset = lower_expr(ctx, &args[2], vars);
                    let amt = lower_expr_as_numeric(ctx, &args[3], vars);
                    ctx.current_instr(Instr::TransferAsset {
                        from,
                        to,
                        asset,
                        amount: amt,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "transfer_batch" => {
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
                        let amount =
                            if let Type::Tuple(items) = semantic::resolve_struct_type(&entry.ty) {
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
                        ctx.current_instr(Instr::TransferAsset {
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
                "transfer_v1_batch_begin" => {
                    ctx.current_instr(Instr::TransferBatchBegin);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "transfer_v1_batch_end" => {
                    ctx.current_instr(Instr::TransferBatchEnd);
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "register_domain" => {
                    let d = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::RegisterDomain { domain: d });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "register_account" => {
                    let a = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::RegisterAccount { account: a });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "unregister_domain" => {
                    let d = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::UnregisterDomain { domain: d });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "unregister_account" => {
                    let a = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::UnregisterAccount { account: a });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "unregister_asset" => {
                    let ad = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::UnregisterAsset { asset: ad });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "register_peer" => {
                    let j = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::RegisterPeer { json: j });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "unregister_peer" => {
                    let j = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::UnregisterPeer { json: j });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "create_trigger" | "register_trigger" => {
                    let j = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::CreateTrigger { json: j });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "remove_trigger" | "unregister_trigger" => {
                    let n = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::RemoveTrigger { name: n });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "set_trigger_enabled" => {
                    let n = lower_expr(ctx, &args[0], vars);
                    let e = lower_expr(ctx, &args[1], vars);
                    ctx.current_instr(Instr::SetTriggerEnabled {
                        name: n,
                        enabled: e,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "grant_permission" => {
                    let a = lower_expr(ctx, &args[0], vars);
                    let tok = lower_expr(ctx, &args[1], vars);
                    ctx.current_instr(Instr::GrantPermission {
                        account: a,
                        token: tok,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "revoke_permission" => {
                    let a = lower_expr(ctx, &args[0], vars);
                    let tok = lower_expr(ctx, &args[1], vars);
                    ctx.current_instr(Instr::RevokePermission {
                        account: a,
                        token: tok,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "create_role" => {
                    let n = lower_expr(ctx, &args[0], vars);
                    let j = lower_expr(ctx, &args[1], vars);
                    ctx.current_instr(Instr::CreateRole { name: n, json: j });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "delete_role" => {
                    let n = lower_expr(ctx, &args[0], vars);
                    ctx.current_instr(Instr::DeleteRole { name: n });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "grant_role" => {
                    let a = lower_expr(ctx, &args[0], vars);
                    let n = lower_expr(ctx, &args[1], vars);
                    ctx.current_instr(Instr::GrantRole {
                        account: a,
                        name: n,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "revoke_role" => {
                    let a = lower_expr(ctx, &args[0], vars);
                    let n = lower_expr(ctx, &args[1], vars);
                    ctx.current_instr(Instr::RevokeRole {
                        account: a,
                        name: n,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "transfer_domain" => {
                    // Signature: (from: AccountId, domain: DomainId, to: AccountId)
                    // Lowering uses only domain and to; the host infers authority as caller.
                    let d = lower_expr(ctx, &args[1], vars);
                    let to = lower_expr(ctx, &args[2], vars);
                    ctx.current_instr(Instr::TransferDomain { domain: d, to });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "register_asset" => {
                    let name = lower_expr(ctx, &args[0], vars);
                    let symbol = lower_expr(ctx, &args[1], vars);
                    let qty = lower_expr_as_numeric(ctx, &args[2], vars);
                    let mint = lower_expr(ctx, &args[3], vars);
                    ctx.current_instr(Instr::RegisterAsset {
                        name,
                        symbol,
                        quantity: qty,
                        mintable: mint,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "create_new_asset" => {
                    let name = lower_expr(ctx, &args[0], vars);
                    let symbol = lower_expr(ctx, &args[1], vars);
                    let qty = lower_expr_as_numeric(ctx, &args[2], vars);
                    let account = lower_expr(ctx, &args[3], vars);
                    let mint = lower_expr(ctx, &args[4], vars);
                    ctx.current_instr(Instr::CreateNewAsset {
                        name,
                        symbol,
                        quantity: qty,
                        account,
                        mintable: mint,
                    });
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                "assert_eq" => {
                    let l = lower_expr(ctx, &args[0], vars);
                    let r = lower_expr(ctx, &args[1], vars);
                    ctx.current_instr(Instr::AssertEq { left: l, right: r });
                    // return dummy temp for unit
                    let t = ctx.new_temp();
                    ctx.current_instr(Instr::Const { dest: t, value: 0 });
                    t
                }
                _ => {
                    // User-defined function call: pass args; capture result(s) if any.
                    let mut arg_tmps = Vec::new();
                    for a in args {
                        arg_tmps.push(lower_expr(ctx, a, vars));
                    }
                    match &expr.ty {
                        semantic::Type::Unit => {
                            ctx.current_instr(Instr::Call {
                                callee: name.clone(),
                                args: arg_tmps,
                                dest: None,
                            });
                            let t = ctx.new_temp();
                            ctx.current_instr(Instr::Const { dest: t, value: 0 });
                            t
                        }
                        semantic::Type::Tuple(ts) => {
                            // Multi-return: move r10.. into temps, then pack to a tuple temp
                            let mut items = Vec::with_capacity(ts.len());
                            for _ in 0..ts.len() {
                                items.push(ctx.new_temp());
                            }
                            ctx.current_instr(Instr::CallMulti {
                                callee: name.clone(),
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
                                callee: name.clone(),
                                args: arg_tmps,
                                dest: Some(d),
                            });
                            d
                        }
                    }
                }
            }
        }
        semantic::ExprKind::Index { target, index } => {
            let key_tmp = lower_expr(ctx, index, vars);
            // Durable read path for tracked state maps when target matches tracked map names.
            if let Some(bn) = state_map_base_name(target)
                && let Some(spec) = ctx.state_map_configs.get(&bn)
                && let (Some(key_codec), Some(value_codec)) = (
                    key_codec_for_type(&spec.key),
                    value_codec_for_type(&spec.value),
                )
            {
                let t_path = build_state_path(ctx, &bn, key_tmp, &key_codec);
                let t_blob = ctx.new_temp();
                ctx.current_instr(Instr::StateGet {
                    dest: t_blob,
                    path: t_path,
                });
                let decoded = decode_value_from_norito(ctx, t_blob, &value_codec);
                return decoded;
            }
            // Fallback to ephemeral map get
            let m = lower_expr(ctx, target, vars);
            if is_pointer_eq_type(&index.ty) || semantic::is_wide_numeric_type(&index.ty) {
                let sk = ctx.new_temp();
                let sv = ctx.new_temp();
                ctx.current_instr(Instr::MapLoadPair {
                    dest_key: sk,
                    dest_val: sv,
                    map: m,
                    offset: 0,
                });
                let flag = lower_map_key_eq(ctx, &index.ty, sk, key_tmp);
                let out = ctx.new_temp();
                ctx.current_instr(Instr::Binary {
                    dest: out,
                    op: BinaryOp::Mul,
                    left: sv,
                    right: flag,
                });
                out
            } else {
                let d = ctx.new_temp();
                ctx.current_instr(Instr::MapGet {
                    dest: d,
                    map: m,
                    key: key_tmp,
                });
                d
            }
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
                    if !ctx.loaded_state_fields.contains(&name)
                        && let Some(literal) = ctx.state_name_literals.get(&name).cloned()
                        && let Some(value) = lower_state_field(ctx, &literal, &expr.ty)
                    {
                        ctx.loaded_state_fields.insert(name.clone());
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

fn lower_state_field(ctx: &mut LowerCtx, literal: &str, ty: &Type) -> Option<Temp> {
    let resolved = semantic::resolve_struct_type(ty);
    match resolved {
        Type::Int => {
            let blob = state_get_blob(ctx, literal);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::DecodeInt { dest, blob });
            Some(dest)
        }
        ty if semantic::is_wide_numeric_type(&ty) => Some(state_get_blob(ctx, literal)),
        Type::Bool => {
            let blob = state_get_blob(ctx, literal);
            let decoded = ctx.new_temp();
            ctx.current_instr(Instr::DecodeInt {
                dest: decoded,
                blob,
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
                left: decoded,
                right: zero,
            });
            Some(out)
        }
        Type::Json => {
            let blob = state_get_blob(ctx, literal);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::JsonDecode { dest, blob });
            Some(dest)
        }
        Type::Name => {
            let blob = state_get_blob(ctx, literal);
            let dest = ctx.new_temp();
            ctx.current_instr(Instr::NameDecode { dest, blob });
            Some(dest)
        }
        Type::Blob | Type::Bytes | Type::String => Some(state_get_blob(ctx, literal)),
        Type::AccountId
        | Type::AssetDefinitionId
        | Type::AssetId
        | Type::DomainId
        | Type::NftId => {
            if let Some(kind) = pointer_kind_for_type(&resolved) {
                let blob = state_get_blob(ctx, literal);
                let dest = ctx.new_temp();
                ctx.current_instr(Instr::PointerFromNorito { dest, blob, kind });
                Some(dest)
            } else {
                // Fallback: treat as raw blob if pointer kind resolution fails.
                Some(state_get_blob(ctx, literal))
            }
        }
        _ => None,
    }
}

fn state_get_blob(ctx: &mut LowerCtx, literal: &str) -> Temp {
    let path = ctx.new_temp();
    ctx.current_instr(Instr::DataRef {
        dest: path,
        kind: DataRefKind::Name,
        value: literal.to_string(),
    });
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
    /// Metadata for state maps lowered to durable state syscalls, keyed by flattened name.
    state_map_configs: HashMap<String, StateMapSpec>,
    /// Mapping from flattened state identifiers (e.g., `s#0#1`) to Name literals used in TLVs.
    state_name_literals: HashMap<String, String>,
    /// Tracks which flattened state fields have already been loaded from durable storage.
    loaded_state_fields: std::collections::HashSet<String>,
    /// Dynamic iteration cap for feature-gated dynamic bounds.
    _dyn_iter_cap: usize,
    error: Option<String>,
}

impl LowerCtx {
    fn new(dyn_iter_cap: usize) -> Self {
        Self {
            next_temp: 0,
            next_label: 0,
            blocks: Vec::new(),
            current: None,
            loop_stack: Vec::new(),
            state_map_configs: Default::default(),
            state_name_literals: Default::default(),
            loaded_state_fields: Default::default(),
            _dyn_iter_cap: dyn_iter_cap,
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

fn build_state_path(ctx: &mut LowerCtx, name: &str, key: Temp, key_codec: &KeyCodec) -> Temp {
    let t_base = build_state_name_literal(ctx, name);
    match key_codec {
        KeyCodec::Int => {
            let t_path = ctx.new_temp();
            ctx.current_instr(Instr::PathMapKey {
                dest: t_path,
                base: t_base,
                key,
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

fn encode_scalar_state_value(ctx: &mut LowerCtx, value: Temp, ty: &Type) -> Option<Temp> {
    let codec = value_codec_for_type(ty)?;
    Some(encode_value_to_norito(ctx, value, &codec))
}

fn emit_scalar_state_set(ctx: &mut LowerCtx, name: &str, ty: &Type, value: Temp) {
    let Some(encoded) = encode_scalar_state_value(ctx, value, ty) else {
        return;
    };
    let path = build_state_name_literal(ctx, name);
    ctx.current_instr(Instr::StateSet {
        path,
        value: encoded,
    });
    ctx.loaded_state_fields.insert(name.to_string());
}

fn encode_value_to_norito(ctx: &mut LowerCtx, value: Temp, codec: &ValueCodec) -> Temp {
    match codec {
        ValueCodec::Int => {
            let t = ctx.new_temp();
            ctx.current_instr(Instr::EncodeInt { dest: t, value });
            t
        }
        ValueCodec::Json => {
            let t = ctx.new_temp();
            ctx.current_instr(Instr::JsonEncode {
                dest: t,
                json: value,
            });
            t
        }
        ValueCodec::Pointer(_) => {
            let t = ctx.new_temp();
            ctx.current_instr(Instr::PointerToNorito { dest: t, value });
            t
        }
        ValueCodec::NoritoBytes => value,
    }
}

fn decode_value_from_norito(ctx: &mut LowerCtx, blob: Temp, codec: &ValueCodec) -> Temp {
    match codec {
        ValueCodec::Int => {
            let t = ctx.new_temp();
            ctx.current_instr(Instr::DecodeInt { dest: t, blob });
            t
        }
        ValueCodec::Json => {
            let t = ctx.new_temp();
            ctx.current_instr(Instr::JsonDecode { dest: t, blob });
            t
        }
        ValueCodec::Pointer(kind) => {
            let t = ctx.new_temp();
            ctx.current_instr(Instr::PointerFromNorito {
                dest: t,
                blob,
                kind: *kind,
            });
            t
        }
        ValueCodec::NoritoBytes => blob,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parser::parse, semantic::analyze};

    #[test]
    fn lower_simple_function() {
        let src = "fn add(a, b) { let c = a + b; }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        assert_eq!(ir.functions.len(), 1);
        let f = &ir.functions[0];
        assert_eq!(f.blocks.len(), 1); // only entry block
    }

    #[test]
    fn lower_if() {
        let src = "fn f(a, b) { if a == b { let c = a; } else { let c = b; } }";
        let ir = lower(&analyze(&parse(src).unwrap()).unwrap()).expect("lower");
        assert_eq!(ir.functions[0].blocks.len(), 4); // entry, then, else, end
    }

    #[test]
    fn lower_while() {
        let src = "fn f() { while 1 < 2 { let a = 2; } }";
        let ir = lower(&analyze(&parse(src).unwrap()).unwrap()).expect("lower");
        assert_eq!(ir.functions[0].blocks.len(), 4); // entry, cond, body, end
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
                let k = name("cursor");
                let v = json("{}\n");
                let d = domain("wonderland");
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
                    if *kind == DataRefKind::Domain && value == "wonderland" {
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
    fn lower_setvl_builtin() {
        let src = "fn main() { setvl(8); }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_setvl = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                if let Instr::SetVl { .. } = instr {
                    saw_setvl = true;
                }
            }
        }
        assert!(saw_setvl, "expected SetVl instruction in lowered IR");
    }

    #[test]
    fn lower_trigger_event_builtin() {
        let src = "fn main() { let ev = trigger_event(); let _kind = json_get_name(ev, name(\"kind\")); }";
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
        let src = "fn main() { let _acct = resolve_account_alias(\"banking@sbp\"); }";
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
    fn lower_json_get_numeric_builtin() {
        let src = "fn main() { let ev = trigger_event(); let _amount: Amount = json_get_numeric(ev, name(\"amount\")); }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_json_get_numeric = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                if let Instr::JsonGetNumeric { .. } = instr {
                    saw_json_get_numeric = true;
                }
            }
        }
        assert!(
            saw_json_get_numeric,
            "expected JsonGetNumeric instruction in lowered IR"
        );
    }

    #[test]
    fn lower_state_map_sets_keep_declared_base_names() {
        let src = r#"
            seiyaku StagedMintRequest {
              state int MintRequestNextSequence;
              state MintRequestSequenceById: Map<Name, int>;
              state MintRequestSequences: Map<int, int>;
              state MintRequestRequestIds: Map<int, Name>;
              state MintRequestFiIds: Map<int, Name>;
              state MintRequestFiAuthorities: Map<int, AccountId>;
              state MintRequestToAccounts: Map<int, AccountId>;
              state MintRequestAmounts: Map<int, int>;
              state MintRequestRequestedBy: Map<int, Json>;
              state MintRequestStates: Map<int, int>;
              state MintRequestCreatedAt: Map<int, int>;
              state MintRequestExpiresAt: Map<int, int>;
              state MintRequestFinalizedAt: Map<int, int>;
              state MintRequestCanceledAt: Map<int, int>;

              fn update_record(sequence: int,
                               request_id: Name,
                               fi_id: Name,
                               fi_multisig_account_id: AccountId,
                               to_account_id: AccountId,
                               amount_i64: int,
                               requested_by_actor_id: Json,
                               state_code: int,
                               created_at_ms: int,
                               expires_at_ms: int,
                               finalized_at_ms: int,
                               canceled_at_ms: int) {
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
                if let Instr::PathMapKey { base, .. } = instr {
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
    fn lower_norito_bytes_literal_to_dataref() {
        let src = r#"fn main() { let _b = norito_bytes(b"ab"); }"#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_norito = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                if let Instr::DataRef { kind, value, .. } = instr
                    && *kind == DataRefKind::NoritoBytes
                    && value == "0x6162"
                {
                    saw_norito = true;
                }
            }
        }
        assert!(saw_norito, "expected NoritoBytes dataref for bytes literal");
    }

    #[test]
    fn lower_trigger_aliases() {
        let src = r#"
            fn main() {
                register_trigger(json("{}"));
                unregister_trigger(name("wake"));
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
                    let args = TransferArgs(domain("wonderland"), account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"));
                    transfer_domain(authority(), args.domain, args.to);
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
        let src = "fn f() { info(7); }";
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
    fn lower_encode_int_literal_uses_encode_int() {
        let src = "fn f() { let _b = encode_int(7); }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_encode = false;
        let mut saw_literal = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::EncodeInt { .. } => saw_encode = true,
                    Instr::DataRef { kind, value, .. }
                        if *kind == DataRefKind::NoritoBytes && value == "7" =>
                    {
                        saw_literal = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(saw_encode, "expected EncodeInt for literal encode_int");
        assert!(
            !saw_literal,
            "unexpected literal NoritoBytes dataref for encode_int"
        );
    }

    #[test]
    fn lower_blob_equality_uses_pointer_eq() {
        let src = "fn f() { let a = blob(\"hi\"); let b = blob(\"hi\"); let _x = a == b; }";
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
    fn lower_map_get_pointer_key_uses_pointer_eq() {
        let src = "fn f() { let m: Map<Name, int> = Map::new(); let k = name(\"alice\"); let _v = m[k]; }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_pointer_eq = false;
        let mut saw_map_get = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::PointerEq { .. } => saw_pointer_eq = true,
                    Instr::MapGet { .. } => saw_map_get = true,
                    _ => {}
                }
            }
        }
        assert!(saw_pointer_eq, "expected PointerEq for pointer map key");
        assert!(
            !saw_map_get,
            "pointer-key map lookup should not use integer MapGet"
        );
    }

    #[test]
    fn lower_map_get_numeric_key_uses_numeric_compare() {
        let src =
            "fn f() { let m: Map<Amount, int> = Map::new(); let k: Amount = 7; let _v = m[k]; }";
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let f = &ir.functions[0];
        let mut saw_numeric_compare = false;
        let mut saw_map_get = false;
        let mut saw_map_load_pair = false;
        for bb in &f.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::NumericCompare { .. } => saw_numeric_compare = true,
                    Instr::MapGet { .. } => saw_map_get = true,
                    Instr::MapLoadPair { .. } => saw_map_load_pair = true,
                    _ => {}
                }
            }
        }
        assert!(
            saw_numeric_compare,
            "expected NumericCompare for numeric map key"
        );
        assert!(
            saw_map_load_pair,
            "expected MapLoadPair for numeric map key"
        );
        assert!(
            !saw_map_get,
            "numeric-key map lookup should not use integer MapGet"
        );
    }

    #[test]
    fn lower_state_struct_field_reads_from_host() {
        let src = r#"
            seiyaku C {
                struct Ledger { counter: int; flag: bool; metadata: Json; label: Name; raw: Blob; owner: AccountId; }
                state Ledger ledger;

                fn main() {
                    let c = ledger.counter;
                    let f = ledger.flag;
                    let m = ledger.metadata;
                    let l = ledger.label;
                    let r = ledger.raw;
                    let o = ledger.owner;
                    let _ = (c, f, m, l, r, o);
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let typed = analyze(&prog).unwrap();
        let ir = lower(&typed).expect("lower");
        let main_fn = ir.functions.iter().find(|f| f.name == "main").unwrap();

        let mut saw_counter_path = false;
        let mut saw_flag_path = false;
        let mut saw_metadata_path = false;
        let mut saw_label_path = false;
        let mut saw_raw_path = false;
        let mut saw_owner_path = false;
        let mut state_gets = 0;
        let mut decode_ints = 0;
        let mut saw_bool_ne = false;
        let mut saw_json_decode = false;
        let mut saw_name_decode = false;
        let mut saw_pointer_decode = false;

        for bb in &main_fn.blocks {
            for instr in &bb.instrs {
                match instr {
                    Instr::DataRef {
                        kind: DataRefKind::Name,
                        value,
                        ..
                    } if value == "ledger_counter" => saw_counter_path = true,
                    Instr::DataRef {
                        kind: DataRefKind::Name,
                        value,
                        ..
                    } if value == "ledger_flag" => saw_flag_path = true,
                    Instr::DataRef {
                        kind: DataRefKind::Name,
                        value,
                        ..
                    } if value == "ledger_metadata" => saw_metadata_path = true,
                    Instr::DataRef {
                        kind: DataRefKind::Name,
                        value,
                        ..
                    } if value == "ledger_label" => saw_label_path = true,
                    Instr::DataRef {
                        kind: DataRefKind::Name,
                        value,
                        ..
                    } if value == "ledger_raw" => saw_raw_path = true,
                    Instr::DataRef {
                        kind: DataRefKind::Name,
                        value,
                        ..
                    } if value == "ledger_owner" => saw_owner_path = true,
                    Instr::StateGet { .. } => state_gets += 1,
                    Instr::DecodeInt { .. } => decode_ints += 1,
                    Instr::Binary {
                        op: BinaryOp::Ne, ..
                    } => saw_bool_ne = true,
                    Instr::JsonDecode { .. } => saw_json_decode = true,
                    Instr::NameDecode { .. } => saw_name_decode = true,
                    Instr::PointerFromNorito { kind, .. } if *kind == DataRefKind::Account => {
                        saw_pointer_decode = true;
                    }
                    _ => {}
                }
            }
        }

        assert!(saw_counter_path, "missing DataRef for ledger.counter path");
        assert!(saw_flag_path, "missing DataRef for ledger.flag path");
        assert!(
            saw_metadata_path,
            "missing DataRef for ledger.metadata path"
        );
        assert!(saw_label_path, "missing DataRef for ledger.label path");
        assert!(saw_raw_path, "missing DataRef for ledger.raw path");
        assert!(saw_owner_path, "missing DataRef for ledger.owner path");
        assert!(state_gets >= 6, "expected durable reads for struct fields");
        assert!(decode_ints >= 2, "expected DecodeInt for persisted scalars");
        assert!(saw_bool_ne, "expected bool normalization via Binary::Ne");
        assert!(saw_json_decode, "expected JsonDecode for persisted Json");
        assert!(saw_name_decode, "expected NameDecode for persisted Name");
        assert!(
            saw_pointer_decode,
            "expected PointerFromNorito for pointer field"
        );
    }

    #[test]
    fn value_codec_supports_blob_state_maps() {
        assert!(matches!(
            value_codec_for_type(&Type::Blob),
            Some(ValueCodec::Pointer(DataRefKind::Blob))
        ));
    }
}
