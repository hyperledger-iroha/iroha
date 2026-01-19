//! Iroha-specific syscall number definitions.
//!
//! The VM uses the `SCALL` instruction to invoke host-provided ledger
//! operations.  Section 7 of the IVM specification assigns concrete numeric
//! codes to those operations.  The constants below mirror that table so that VM
//! users can refer to the syscalls symbolically.  Actual behaviour and gas
//! charges are implemented by the host. These calls are collectively known as
//! **Iroha Special Instructions** (ISI).
//!
//! The table below includes helper syscalls used for cryptographic proof
//! generation and verification, Merkle path queries and hardware feature
//! discovery. Additional concurrency primitives may be added in future
//! versions of the specification.

/// Debug helper for development; part of the ABI v1 surface.
pub const SYSCALL_DEBUG_PRINT: u32 = 0;

/// Lifecycle and utility syscalls.
/// Gracefully terminate the program and return a value.
pub const SYSCALL_EXIT: u32 = 0x01;
/// Abort execution and revert state changes.
pub const SYSCALL_ABORT: u32 = 0x02;
/// Output a debug message (development only).
pub const SYSCALL_DEBUG_LOG: u32 = 0x03;

/// Helper syscalls for inputs/outputs; part of the ABI v1 surface.
/// Retrieve a piece of public input provided by the host.
pub const SYSCALL_GET_PUBLIC_INPUT: u32 = 0xF1;
/// Allocate heap memory.
pub const SYSCALL_ALLOC: u32 = 0xF0;

/// Domain and peer management.
pub const SYSCALL_REGISTER_DOMAIN: u32 = 0x10;
pub const SYSCALL_UNREGISTER_DOMAIN: u32 = 0x11;
pub const SYSCALL_TRANSFER_DOMAIN: u32 = 0x12;
pub const SYSCALL_REGISTER_PEER: u32 = 0x15;
pub const SYSCALL_UNREGISTER_PEER: u32 = 0x16;

/// Account management.
pub const SYSCALL_REGISTER_ACCOUNT: u32 = 0x13;
pub const SYSCALL_UNREGISTER_ACCOUNT: u32 = 0x14;
/// Add a signatory for an account.
pub const SYSCALL_ADD_SIGNATORY: u32 = 0x17;
/// Remove a signatory from an account.
pub const SYSCALL_REMOVE_SIGNATORY: u32 = 0x18;
/// Update account quorum.
pub const SYSCALL_SET_ACCOUNT_QUORUM: u32 = 0x19;
pub const SYSCALL_SET_ACCOUNT_DETAIL: u32 = 0x1A;

/// Asset definitions.
pub const SYSCALL_REGISTER_ASSET: u32 = 0x20;
pub const SYSCALL_UNREGISTER_ASSET: u32 = 0x21;
pub const SYSCALL_MINT_ASSET: u32 = 0x22;
pub const SYSCALL_BURN_ASSET: u32 = 0x23;
pub const SYSCALL_TRANSFER_ASSET: u32 = 0x24;
/// Alias for the canonical FASTPQ transfer gadget syscall.
pub const SYSCALL_TRANSFER_V1: u32 = SYSCALL_TRANSFER_ASSET;
/// Begin a FASTPQ transfer batch; subsequent `transfer_v1` calls are coalesced.
pub const SYSCALL_TRANSFER_V1_BATCH_BEGIN: u32 = 0x29;
/// End the current FASTPQ transfer batch scope.
pub const SYSCALL_TRANSFER_V1_BATCH_END: u32 = 0x2A;
/// Submit a pre-baked FASTPQ batch via a Norito-encoded [`TransferAssetBatch`].
pub const SYSCALL_TRANSFER_V1_BATCH_APPLY: u32 = 0x2B;

/// Non‑fungible asset (NFT) operations (canonical names).
pub const SYSCALL_NFT_MINT_ASSET: u32 = 0x25;
pub const SYSCALL_NFT_TRANSFER_ASSET: u32 = 0x26;
pub const SYSCALL_NFT_SET_METADATA: u32 = 0x27;
pub const SYSCALL_NFT_BURN_ASSET: u32 = 0x28;

/// Smart-contract durable state (key-value by path).
///
/// Pointer-ABI arguments: paths use `&Name` TLV; values use `&NoritoBytes` TLV.
///
/// GET:  r10 = &Name path  -> On success, r10 = &NoritoBytes value (mirrored into INPUT); if missing, r10 = 0.
/// SET:  r10 = &Name path, r11 = &NoritoBytes value  -> stores value, returns 0.
/// DEL:  r10 = &Name path  -> deletes value if present, returns 0.
pub const SYSCALL_STATE_GET: u32 = 0x50;
pub const SYSCALL_STATE_SET: u32 = 0x51;
pub const SYSCALL_STATE_DEL: u32 = 0x52;
/// Decode a NoritoBytes value containing a signed decimal ASCII integer and return
/// the value in `x10` as a 64-bit signed integer (two's complement).
///
/// Args: r10 = &NoritoBytes (ASCII decimal)
/// Ret:  r10 = value (as u64 bits)
pub const SYSCALL_DECODE_INT: u32 = 0x53;
/// Construct a Numeric (mantissa+scale) from a non-negative 64-bit integer and return
/// a NoritoBytes TLV pointer. The Numeric is encoded with scale = 0.
///
/// Args: r10 = value (i64 as u64)
/// Ret:  r10 = &NoritoBytes (Numeric payload)
pub const SYSCALL_NUMERIC_FROM_INT: u32 = 0x69;
/// Convert a Numeric NoritoBytes payload into a signed 64-bit integer.
/// The payload must be unsigned and use scale = 0; otherwise the host rejects it.
///
/// Args: r10 = &NoritoBytes (Numeric payload)
/// Ret:  r10 = value (i64 as u64)
pub const SYSCALL_NUMERIC_TO_INT: u32 = 0x6A;
/// Numeric addition: r10 = &NoritoBytes(lhs), r11 = &NoritoBytes(rhs)
/// -> r10 = &NoritoBytes(result).
pub const SYSCALL_NUMERIC_ADD: u32 = 0x6B;
/// Numeric subtraction: r10 = &NoritoBytes(lhs), r11 = &NoritoBytes(rhs)
/// -> r10 = &NoritoBytes(result). Rejects underflow (negative result).
pub const SYSCALL_NUMERIC_SUB: u32 = 0x6C;
/// Numeric multiplication: r10 = &NoritoBytes(lhs), r11 = &NoritoBytes(rhs)
/// -> r10 = &NoritoBytes(result).
pub const SYSCALL_NUMERIC_MUL: u32 = 0x6D;
/// Numeric division: r10 = &NoritoBytes(lhs), r11 = &NoritoBytes(rhs)
/// -> r10 = &NoritoBytes(result).
pub const SYSCALL_NUMERIC_DIV: u32 = 0x6E;
/// Numeric remainder: r10 = &NoritoBytes(lhs), r11 = &NoritoBytes(rhs)
/// -> r10 = &NoritoBytes(result).
pub const SYSCALL_NUMERIC_REM: u32 = 0x6F;
/// Numeric negation: r10 = &NoritoBytes(value) -> r10 = &NoritoBytes(result).
/// Rejects non-zero inputs (numeric aliases are unsigned).
pub const SYSCALL_NUMERIC_NEG: u32 = 0x70;
/// Numeric equality: r10 = &NoritoBytes(lhs), r11 = &NoritoBytes(rhs)
/// -> r10 = 1 if equal else 0.
pub const SYSCALL_NUMERIC_EQ: u32 = 0x71;
/// Numeric inequality: r10 = &NoritoBytes(lhs), r11 = &NoritoBytes(rhs)
/// -> r10 = 1 if not equal else 0.
pub const SYSCALL_NUMERIC_NE: u32 = 0x72;
/// Numeric less-than: r10 = &NoritoBytes(lhs), r11 = &NoritoBytes(rhs)
/// -> r10 = 1 if lhs < rhs else 0.
pub const SYSCALL_NUMERIC_LT: u32 = 0x73;
/// Numeric less-or-equal: r10 = &NoritoBytes(lhs), r11 = &NoritoBytes(rhs)
/// -> r10 = 1 if lhs <= rhs else 0.
pub const SYSCALL_NUMERIC_LE: u32 = 0x74;
/// Numeric greater-than: r10 = &NoritoBytes(lhs), r11 = &NoritoBytes(rhs)
/// -> r10 = 1 if lhs > rhs else 0.
pub const SYSCALL_NUMERIC_GT: u32 = 0x75;
/// Numeric greater-or-equal: r10 = &NoritoBytes(lhs), r11 = &NoritoBytes(rhs)
/// -> r10 = 1 if lhs >= rhs else 0.
pub const SYSCALL_NUMERIC_GE: u32 = 0x76;
/// Build a state path from a base Name and an integer key: returns a new `&Name` TLV
/// in INPUT with the canonical form "<base>/<key>" (decimal).
///
/// Args: r10 = &Name base, r11 = key (i64 as u64)
/// Ret:  r10 = &Name (INPUT pointer)
pub const SYSCALL_BUILD_PATH_MAP_KEY: u32 = 0x54;
/// Encode a 64-bit signed integer in ASCII decimal and return a `&NoritoBytes` TLV pointer in INPUT.
///
/// Args: r10 = value (i64 as u64)
/// Ret:  r10 = &NoritoBytes (ASCII decimal)
pub const SYSCALL_ENCODE_INT: u32 = 0x55;
/// Build a state path from a base Name and a NoritoBytes key by appending
/// `"/" + hex(hash32(payload))` where `hash32` is `iroha_crypto::Hash` of the payload bytes.
///
/// Args: r10 = &Name base, r11 = &NoritoBytes key
/// Ret:  r10 = &Name (INPUT pointer)
pub const SYSCALL_BUILD_PATH_KEY_NORITO: u32 = 0x56;
/// JSON <-> NoritoBytes helpers (developer convenience):
/// ENCODE_JSON: r10 = &Json -> r10 = &NoritoBytes (minified JSON bytes)
pub const SYSCALL_JSON_ENCODE: u32 = 0x57;
/// DECODE_JSON: r10 = &NoritoBytes (JSON bytes) -> r10 = &Json (minified)
pub const SYSCALL_JSON_DECODE: u32 = 0x58;
/// Schema-based Norito encode: r10 = &Name schema, r11 = &Json value -> r10 = &NoritoBytes
pub const SYSCALL_SCHEMA_ENCODE: u32 = 0x59;
/// Schema-based Norito decode: r10 = &Name schema, r11 = &NoritoBytes -> r10 = &Json
pub const SYSCALL_SCHEMA_DECODE: u32 = 0x5A;
/// Schema info: r10 = &Name schema -> r10 = &Json {"id":"<hex>", "version":N}
pub const SYSCALL_SCHEMA_INFO: u32 = 0x5B;
/// Decode a Name from a NoritoBytes TLV (UTF-8) and return a `&Name` TLV pointer in INPUT.
///
/// Args: r10 = &NoritoBytes (UTF-8 string)
/// Ret:  r10 = &Name (minified string)
pub const SYSCALL_NAME_DECODE: u32 = 0x5C;
/// Encode an arbitrary pointer-ABI TLV into NoritoBytes by copying its envelope bytes.
///
/// Args: r10 = &PointerType::<T>
/// Ret:  r10 = &NoritoBytes(payload = TLV bytes)
pub const SYSCALL_POINTER_TO_NORITO: u32 = 0x5D;
/// Decode a NoritoBytes payload produced by [`SYSCALL_POINTER_TO_NORITO`] back into the
/// original pointer-ABI TLV. Expects the payload to begin with the canonical TLV header
/// `(type_id, version, len, payload…)`.
///
/// Args: r10 = &NoritoBytes, r11 = expected pointer type id (u16)
/// Ret:  r10 = &PointerType::<T>
pub const SYSCALL_POINTER_FROM_NORITO: u32 = 0x5E;
/// Compare two pointer-ABI TLVs for deep equality by content (header + payload).
///
/// Args: r10 = &TLV, r11 = &TLV
/// Ret:  r10 = 1 if equal, 0 if not
pub const SYSCALL_TLV_EQ: u32 = 0x5F;

/// Roles and permissions.
pub const SYSCALL_CREATE_ROLE: u32 = 0x30;
pub const SYSCALL_DELETE_ROLE: u32 = 0x31;
pub const SYSCALL_GRANT_ROLE: u32 = 0x32;
pub const SYSCALL_REVOKE_ROLE: u32 = 0x33;
pub const SYSCALL_GRANT_PERMISSION: u32 = 0x34;
pub const SYSCALL_REVOKE_PERMISSION: u32 = 0x35;

/// Triggers.
pub const SYSCALL_CREATE_TRIGGER: u32 = 0x40;
pub const SYSCALL_REMOVE_TRIGGER: u32 = 0x41;
pub const SYSCALL_SET_TRIGGER_ENABLED: u32 = 0x42;
/// Governance kill-switch for contract instances.
pub const SYSCALL_DEACTIVATE_CONTRACT_INSTANCE: u32 = 0x43;
/// Governance removal of stored smart contract bytecode.
pub const SYSCALL_REMOVE_SMART_CONTRACT_BYTES: u32 = 0x44;
/// Governance registration of smart contract metadata (manifest only).
pub const SYSCALL_REGISTER_SMART_CONTRACT_CODE: u32 = 0x45;
/// Governance registration of compiled contract bytecode.
pub const SYSCALL_REGISTER_SMART_CONTRACT_BYTES: u32 = 0x46;
/// Governance activation of a contract instance binding.
pub const SYSCALL_ACTIVATE_CONTRACT_INSTANCE: u32 = 0x47;

/// Zero-knowledge mode helpers.
pub const SYSCALL_GET_ACCOUNT_BALANCE: u32 = 0xF9;
pub const SYSCALL_USE_NULLIFIER: u32 = 0xFB;
pub const SYSCALL_VERIFY_SIGNATURE: u32 = 0xFC;
pub const SYSCALL_GET_PRIVATE_INPUT: u32 = 0xFD;
pub const SYSCALL_COMMIT_OUTPUT: u32 = 0xFE;

// ZK verification and state-read syscalls (pointer-ABI NoritoBytes payloads)
/// Verify a shielded transfer proof (no state mutation).
pub const SYSCALL_ZK_VERIFY_TRANSFER: u32 = 0x60;
/// Verify an unshield proof (no state mutation).
pub const SYSCALL_ZK_VERIFY_UNSHIELD: u32 = 0x61;
/// Verify a ballot proof for an election (no state mutation).
pub const SYSCALL_ZK_VOTE_VERIFY_BALLOT: u32 = 0x62;
/// Verify a tally proof for an election (no state mutation).
pub const SYSCALL_ZK_VOTE_VERIFY_TALLY: u32 = 0x63;
/// Read recent Merkle roots for an asset's shielded ledger.
pub const SYSCALL_ZK_ROOTS_GET: u32 = 0x64;
/// Read finalized tally for an election, if present.
pub const SYSCALL_ZK_VOTE_GET_TALLY: u32 = 0x65;
/// Batch verification of Halo2 OpenVerify envelopes.
pub const SYSCALL_ZK_VERIFY_BATCH: u32 = 0x68;

/// Verify a BLS-based VRF proof and return the 32-byte output in a Blob TLV.
///
/// Args:
/// - r10 = &Blob input
/// - r11 = &Blob public key (BLS compressed bytes; variant-dependent size)
/// - r12 = &Blob proof (BLS signature bytes; variant-dependent size)
/// - r13 = variant code (1 = BLS Normal, 2 = BLS Small)
///
/// Return:
/// - On success, `r10` = pointer to `&Blob` TLV with 32-byte VRF output.
/// - On failure (bad inputs or verify), `r10 = 0`.
pub const SYSCALL_VRF_VERIFY: u32 = 0x66;
/// Batch VRF verification: verify multiple tuples and return a Norito-encoded
/// vector of 32-byte outputs on success.
pub const SYSCALL_VRF_VERIFY_BATCH: u32 = 0x67;

/// Hardware and proof generation helpers.
pub const SYSCALL_PROVE_EXECUTION: u32 = 0xF4;
/// Increase heap size by the number of bytes in `x10`.
pub const SYSCALL_GROW_HEAP: u32 = 0xF5;
/// Verify the collected execution proof.
pub const SYSCALL_VERIFY_PROOF: u32 = 0xF6;
/// Write the Merkle path for address `x10` to memory at `x11`.
/// Optional: if `x12 != 0`, write the current Merkle root to the 32-byte
/// buffer at `x12`.
pub const SYSCALL_GET_MERKLE_PATH: u32 = 0xF7;
/// Write a compact Merkle proof for address `x10` to memory at `x11` using the
/// layout: `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]`. If `x12 != 0`,
/// cap the depth to `min(x12, 32)`. If `x13 != 0`, write the current 32-byte
/// Merkle root to `x13`.
pub const SYSCALL_GET_MERKLE_COMPACT: u32 = 0xFA;
/// Write a compact Merkle proof for the register leaf at index `x10` to memory
/// at `x11` using the same layout as `GET_MERKLE_COMPACT`. If `x12 != 0`, cap
/// depth; if `x13 != 0`, write the 32-byte register Merkle root to `x13`.
pub const SYSCALL_GET_REGISTER_MERKLE_COMPACT: u32 = 0xFF;

/// Compute SM3 hash of a blob (`&Blob` -> `&Blob`).
pub const SYSCALL_SM3_HASH: u32 = 0x90;
/// Verify an SM2 signature (`&Blob` message, `&Blob` signature, `&Blob` public key, optional `&Blob` distid).
pub const SYSCALL_SM2_VERIFY: u32 = 0x91;
/// SM4-GCM encrypt: returns ciphertext||tag in Blob TLV.
pub const SYSCALL_SM4_GCM_SEAL: u32 = 0x92;
/// SM4-GCM decrypt: returns plaintext Blob TLV or 0 on failure.
pub const SYSCALL_SM4_GCM_OPEN: u32 = 0x93;
/// SM4-CCM encrypt: returns ciphertext||tag in Blob TLV.
pub const SYSCALL_SM4_CCM_SEAL: u32 = 0x94;
/// SM4-CCM decrypt: returns plaintext Blob TLV or 0 on failure.
pub const SYSCALL_SM4_CCM_OPEN: u32 = 0x95;
/// Developer helper: copy a TLV from program memory into the INPUT region and return its pointer.
///
/// Expects `x10` to hold a pointer to a valid TLV in program memory (data/heap). The host validates
/// the header and payload and mirrors it into INPUT via the internal allocator, returning the new
/// INPUT pointer in `x10`.
pub const SYSCALL_INPUT_PUBLISH_TLV: u32 = 0xE0;

// Smart-contract host shims (development API)
/// Execute a built-in instruction from an IVM smart contract (pointer-ABI).
pub const SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION: u32 = 0xA0;
/// Execute a query from an IVM smart contract (pointer-ABI).
pub const SYSCALL_SMARTCONTRACT_EXECUTE_QUERY: u32 = 0xA1;
/// Convenience syscall used by samples: create one NFT per known account.
pub const SYSCALL_CREATE_NFTS_FOR_ALL_USERS: u32 = 0xA2;
/// Set SmartContract execution depth parameter to the value in `x10`.
/// Development/testing helper for trigger samples.
pub const SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH: u32 = 0xA3;
/// Get current authority AccountId (writes a Norito-encoded blob to INPUT and returns pointer in x10)
pub const SYSCALL_GET_AUTHORITY: u32 = 0xA4;
/// Execute subscription billing based on trigger metadata and subscription state.
pub const SYSCALL_SUBSCRIPTION_BILL: u32 = 0xA5;
/// Record subscription usage from trigger args payload.
pub const SYSCALL_SUBSCRIPTION_RECORD_USAGE: u32 = 0xA6;
/// Begin an atomic cross-transaction (AXT) envelope.
pub const SYSCALL_AXT_BEGIN: u32 = 0xB0;
/// Declare a DS touch within an active AXT.
pub const SYSCALL_AXT_TOUCH: u32 = 0xB1;
/// Commit the active AXT envelope if all invariants hold.
pub const SYSCALL_AXT_COMMIT: u32 = 0xB2;
/// Verify a DS proof bundle inside an AXT.
pub const SYSCALL_VERIFY_DS_PROOF: u32 = 0xB3;
/// Use a capability handle granted by an asset DS inside an AXT.
pub const SYSCALL_USE_ASSET_HANDLE: u32 = 0xB4;

/// Returns whether a syscall number is allowed for the given ABI policy.
///
/// This function centralizes the mapping between `ProgramMetadata.abi_version`
/// and the set of syscalls available to programs compiled against that ABI.
/// Hosts should call this before attempting to handle a syscall to ensure
/// stable behavior across versions; unknown or disallowed numbers must be
/// rejected with `VMError::UnknownSyscall`.
pub fn is_syscall_allowed(policy: crate::SyscallPolicy, number: u32) -> bool {
    syscalls_for_policy(policy).binary_search(&number).is_ok()
}

/// Return a sorted list of syscall numbers considered for ABI hashing.
///
/// The ABI hash is a stable digest of the allowed syscall surface for a given
/// `SyscallPolicy`. It binds contracts to a specific host ABI. When comparing
/// runtime ABI against a manifest-provided `abi_hash`, nodes can reject
/// execution if a mismatch is detected.
pub fn abi_syscall_list() -> &'static [u32] {
    syscalls_for_policy(crate::SyscallPolicy::AbiV1)
}

/// Return the canonical syscall list for an ABI policy.
///
/// ABI v1 is fixed; future versions must explicitly define their surface.
pub fn syscalls_for_policy(policy: crate::SyscallPolicy) -> &'static [u32] {
    use std::sync::OnceLock;
    static ABI_V1: OnceLock<Vec<u32>> = OnceLock::new();
    let v = ABI_V1.get_or_init(|| {
        let mut v = vec![
            // Lifecycle / utility
            SYSCALL_EXIT,
            SYSCALL_ABORT,
            SYSCALL_DEBUG_LOG,
            // Heaps and IO
            SYSCALL_ALLOC,
            SYSCALL_GROW_HEAP,
            SYSCALL_GET_PUBLIC_INPUT,
            SYSCALL_GET_PRIVATE_INPUT,
            SYSCALL_COMMIT_OUTPUT,
            SYSCALL_VERIFY_SIGNATURE,
            // Hardware / helpers
            SYSCALL_PROVE_EXECUTION,
            SYSCALL_VERIFY_PROOF,
            SYSCALL_GET_MERKLE_PATH,
            SYSCALL_GET_MERKLE_COMPACT,
            SYSCALL_INPUT_PUBLISH_TLV,
        ];
        v.extend_from_slice(&[
            SYSCALL_SM3_HASH,
            SYSCALL_SM2_VERIFY,
            SYSCALL_SM4_GCM_SEAL,
            SYSCALL_SM4_GCM_OPEN,
            SYSCALL_SM4_CCM_SEAL,
            SYSCALL_SM4_CCM_OPEN,
        ]);
        // Codec helpers
        v.push(SYSCALL_JSON_ENCODE);
        v.push(SYSCALL_JSON_DECODE);
        v.push(SYSCALL_SCHEMA_ENCODE);
        v.push(SYSCALL_SCHEMA_DECODE);
        v.push(SYSCALL_SCHEMA_INFO);
        // Numeric helpers (mantissa+scale)
        v.extend_from_slice(&[
            SYSCALL_NUMERIC_FROM_INT,
            SYSCALL_NUMERIC_TO_INT,
            SYSCALL_NUMERIC_ADD,
            SYSCALL_NUMERIC_SUB,
            SYSCALL_NUMERIC_MUL,
            SYSCALL_NUMERIC_DIV,
            SYSCALL_NUMERIC_REM,
            SYSCALL_NUMERIC_NEG,
            SYSCALL_NUMERIC_EQ,
            SYSCALL_NUMERIC_NE,
            SYSCALL_NUMERIC_LT,
            SYSCALL_NUMERIC_LE,
            SYSCALL_NUMERIC_GT,
            SYSCALL_NUMERIC_GE,
        ]);
        // Name decode is part of base ABI in V1
        v.push(SYSCALL_NAME_DECODE);
        // Account and asset ops (bridged by hosts)
        v.push(SYSCALL_REGISTER_DOMAIN);
        v.push(SYSCALL_UNREGISTER_DOMAIN);
        v.push(SYSCALL_TRANSFER_DOMAIN);
        v.push(SYSCALL_REGISTER_PEER);
        v.push(SYSCALL_UNREGISTER_PEER);
        v.push(SYSCALL_REGISTER_ACCOUNT);
        v.push(SYSCALL_UNREGISTER_ACCOUNT);
        v.push(SYSCALL_ADD_SIGNATORY);
        v.push(SYSCALL_REMOVE_SIGNATORY);
        v.push(SYSCALL_SET_ACCOUNT_QUORUM);
        v.push(SYSCALL_SET_ACCOUNT_DETAIL);
        v.push(SYSCALL_REGISTER_ASSET);
        v.push(SYSCALL_UNREGISTER_ASSET);
        v.push(SYSCALL_MINT_ASSET);
        v.push(SYSCALL_BURN_ASSET);
        v.push(SYSCALL_TRANSFER_ASSET);
        v.push(SYSCALL_TRANSFER_V1_BATCH_BEGIN);
        v.push(SYSCALL_TRANSFER_V1_BATCH_END);
        v.push(SYSCALL_TRANSFER_V1_BATCH_APPLY);
        // NFT
        v.extend_from_slice(&[
            SYSCALL_NFT_MINT_ASSET,
            SYSCALL_NFT_TRANSFER_ASSET,
            SYSCALL_NFT_SET_METADATA,
            SYSCALL_NFT_BURN_ASSET,
        ]);
        // Durable state (smart contract)
        v.push(SYSCALL_STATE_GET);
        v.push(SYSCALL_STATE_SET);
        v.push(SYSCALL_STATE_DEL);
        v.push(SYSCALL_BUILD_PATH_MAP_KEY);
        v.push(SYSCALL_BUILD_PATH_KEY_NORITO);
        v.push(SYSCALL_ENCODE_INT);
        v.push(SYSCALL_DECODE_INT);
        v.push(SYSCALL_POINTER_TO_NORITO);
        v.push(SYSCALL_POINTER_FROM_NORITO);
        v.push(SYSCALL_TLV_EQ);
        // Roles/permissions
        v.extend_from_slice(&[
            SYSCALL_CREATE_ROLE,
            SYSCALL_DELETE_ROLE,
            SYSCALL_GRANT_ROLE,
            SYSCALL_REVOKE_ROLE,
            SYSCALL_GRANT_PERMISSION,
            SYSCALL_REVOKE_PERMISSION,
        ]);
        // Triggers
        v.extend_from_slice(&[
            SYSCALL_CREATE_TRIGGER,
            SYSCALL_REMOVE_TRIGGER,
            SYSCALL_SET_TRIGGER_ENABLED,
            SYSCALL_REGISTER_SMART_CONTRACT_CODE,
            SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
            SYSCALL_ACTIVATE_CONTRACT_INSTANCE,
            SYSCALL_DEACTIVATE_CONTRACT_INSTANCE,
            SYSCALL_REMOVE_SMART_CONTRACT_BYTES,
        ]);
        // ZK verification/state-read
        v.extend_from_slice(&[
            SYSCALL_ZK_VERIFY_TRANSFER,
            SYSCALL_ZK_VERIFY_UNSHIELD,
            SYSCALL_ZK_VOTE_VERIFY_BALLOT,
            SYSCALL_ZK_VOTE_VERIFY_TALLY,
            SYSCALL_ZK_ROOTS_GET,
            SYSCALL_ZK_VOTE_GET_TALLY,
            SYSCALL_ZK_VERIFY_BATCH,
        ]);
        // VRF
        v.push(SYSCALL_VRF_VERIFY);
        v.push(SYSCALL_VRF_VERIFY_BATCH);
        // Dev/vendor helpers
        v.extend_from_slice(&[
            SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION,
            SYSCALL_SMARTCONTRACT_EXECUTE_QUERY,
            SYSCALL_CREATE_NFTS_FOR_ALL_USERS,
            SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH,
            SYSCALL_GET_AUTHORITY,
            SYSCALL_SUBSCRIPTION_BILL,
            SYSCALL_SUBSCRIPTION_RECORD_USAGE,
        ]);
        // Atomic cross-transaction (AXT) scaffolding
        v.extend_from_slice(&[
            SYSCALL_AXT_BEGIN,
            SYSCALL_AXT_TOUCH,
            SYSCALL_AXT_COMMIT,
            SYSCALL_VERIFY_DS_PROOF,
            SYSCALL_USE_ASSET_HANDLE,
        ]);
        // ZK extras
        v.extend_from_slice(&[
            SYSCALL_GET_ACCOUNT_BALANCE,
            SYSCALL_USE_NULLIFIER,
            SYSCALL_GET_REGISTER_MERKLE_COMPACT,
        ]);
        // Debug helper
        v.push(SYSCALL_DEBUG_PRINT);
        v.sort_unstable();
        v.dedup();
        debug_assert!(
            v.windows(2).all(|w| w[0] < w[1]),
            "abi_syscall_list must stay sorted and unique"
        );
        v
    });
    match policy {
        crate::SyscallPolicy::AbiV1 => v.as_slice(),
        crate::SyscallPolicy::Experimental(_) => &[],
    }
}

/// Return a symbolic name for a syscall number, when known.
/// This is used for documentation/tests; hosts should match on numbers directly.
pub fn syscall_name(number: u32) -> Option<&'static str> {
    Some(match number {
        // Lifecycle / utility
        SYSCALL_EXIT => "EXIT",
        SYSCALL_ABORT => "ABORT",
        SYSCALL_DEBUG_LOG => "DEBUG_LOG",
        // Heaps and IO
        SYSCALL_ALLOC => "ALLOC",
        SYSCALL_GROW_HEAP => "GROW_HEAP",
        SYSCALL_GET_PUBLIC_INPUT => "GET_PUBLIC_INPUT",
        SYSCALL_GET_PRIVATE_INPUT => "GET_PRIVATE_INPUT",
        SYSCALL_COMMIT_OUTPUT => "COMMIT_OUTPUT",
        SYSCALL_VERIFY_SIGNATURE => "VERIFY_SIGNATURE",
        SYSCALL_INPUT_PUBLISH_TLV => "INPUT_PUBLISH_TLV",
        SYSCALL_SM3_HASH => "SM3_HASH",
        SYSCALL_SM2_VERIFY => "SM2_VERIFY",
        SYSCALL_SM4_GCM_SEAL => "SM4_GCM_SEAL",
        SYSCALL_SM4_GCM_OPEN => "SM4_GCM_OPEN",
        SYSCALL_SM4_CCM_SEAL => "SM4_CCM_SEAL",
        SYSCALL_SM4_CCM_OPEN => "SM4_CCM_OPEN",
        // Hardware / helpers
        SYSCALL_PROVE_EXECUTION => "PROVE_EXECUTION",
        SYSCALL_VERIFY_PROOF => "VERIFY_PROOF",
        SYSCALL_GET_MERKLE_PATH => "GET_MERKLE_PATH",
        SYSCALL_GET_MERKLE_COMPACT => "GET_MERKLE_COMPACT",
        // Account and asset ops
        SYSCALL_REGISTER_DOMAIN => "REGISTER_DOMAIN",
        SYSCALL_UNREGISTER_DOMAIN => "UNREGISTER_DOMAIN",
        SYSCALL_TRANSFER_DOMAIN => "TRANSFER_DOMAIN",
        SYSCALL_REGISTER_PEER => "REGISTER_PEER",
        SYSCALL_UNREGISTER_PEER => "UNREGISTER_PEER",
        SYSCALL_REGISTER_ACCOUNT => "REGISTER_ACCOUNT",
        SYSCALL_UNREGISTER_ACCOUNT => "UNREGISTER_ACCOUNT",
        SYSCALL_ADD_SIGNATORY => "ADD_SIGNATORY",
        SYSCALL_REMOVE_SIGNATORY => "REMOVE_SIGNATORY",
        SYSCALL_SET_ACCOUNT_QUORUM => "SET_ACCOUNT_QUORUM",
        SYSCALL_SET_ACCOUNT_DETAIL => "SET_ACCOUNT_DETAIL",
        SYSCALL_REGISTER_ASSET => "REGISTER_ASSET",
        SYSCALL_UNREGISTER_ASSET => "UNREGISTER_ASSET",
        SYSCALL_MINT_ASSET => "MINT_ASSET",
        SYSCALL_BURN_ASSET => "BURN_ASSET",
        SYSCALL_TRANSFER_ASSET => "TRANSFER_ASSET",
        SYSCALL_TRANSFER_V1_BATCH_BEGIN => "TRANSFER_V1_BATCH_BEGIN",
        SYSCALL_TRANSFER_V1_BATCH_END => "TRANSFER_V1_BATCH_END",
        SYSCALL_TRANSFER_V1_BATCH_APPLY => "TRANSFER_V1_BATCH_APPLY",
        // NFT
        SYSCALL_NFT_MINT_ASSET => "NFT_MINT_ASSET",
        SYSCALL_NFT_TRANSFER_ASSET => "NFT_TRANSFER_ASSET",
        SYSCALL_NFT_SET_METADATA => "NFT_SET_METADATA",
        SYSCALL_NFT_BURN_ASSET => "NFT_BURN_ASSET",
        // Durable state
        SYSCALL_STATE_GET => "STATE_GET",
        SYSCALL_STATE_SET => "STATE_SET",
        SYSCALL_STATE_DEL => "STATE_DEL",
        SYSCALL_DECODE_INT => "DECODE_INT",
        SYSCALL_BUILD_PATH_MAP_KEY => "BUILD_PATH_MAP_KEY",
        SYSCALL_ENCODE_INT => "ENCODE_INT",
        SYSCALL_BUILD_PATH_KEY_NORITO => "BUILD_PATH_KEY_NORITO",
        SYSCALL_NUMERIC_FROM_INT => "NUMERIC_FROM_INT",
        SYSCALL_NUMERIC_TO_INT => "NUMERIC_TO_INT",
        SYSCALL_NUMERIC_ADD => "NUMERIC_ADD",
        SYSCALL_NUMERIC_SUB => "NUMERIC_SUB",
        SYSCALL_NUMERIC_MUL => "NUMERIC_MUL",
        SYSCALL_NUMERIC_DIV => "NUMERIC_DIV",
        SYSCALL_NUMERIC_REM => "NUMERIC_REM",
        SYSCALL_NUMERIC_NEG => "NUMERIC_NEG",
        SYSCALL_NUMERIC_EQ => "NUMERIC_EQ",
        SYSCALL_NUMERIC_NE => "NUMERIC_NE",
        SYSCALL_NUMERIC_LT => "NUMERIC_LT",
        SYSCALL_NUMERIC_LE => "NUMERIC_LE",
        SYSCALL_NUMERIC_GT => "NUMERIC_GT",
        SYSCALL_NUMERIC_GE => "NUMERIC_GE",
        // Roles/permissions
        SYSCALL_CREATE_ROLE => "CREATE_ROLE",
        SYSCALL_DELETE_ROLE => "DELETE_ROLE",
        SYSCALL_GRANT_ROLE => "GRANT_ROLE",
        SYSCALL_REVOKE_ROLE => "REVOKE_ROLE",
        SYSCALL_GRANT_PERMISSION => "GRANT_PERMISSION",
        SYSCALL_REVOKE_PERMISSION => "REVOKE_PERMISSION",
        // Triggers
        SYSCALL_CREATE_TRIGGER => "CREATE_TRIGGER",
        SYSCALL_REMOVE_TRIGGER => "REMOVE_TRIGGER",
        SYSCALL_SET_TRIGGER_ENABLED => "SET_TRIGGER_ENABLED",
        SYSCALL_REGISTER_SMART_CONTRACT_CODE => "REGISTER_SMART_CONTRACT_CODE",
        SYSCALL_REGISTER_SMART_CONTRACT_BYTES => "REGISTER_SMART_CONTRACT_BYTES",
        SYSCALL_ACTIVATE_CONTRACT_INSTANCE => "ACTIVATE_CONTRACT_INSTANCE",
        SYSCALL_DEACTIVATE_CONTRACT_INSTANCE => "DEACTIVATE_CONTRACT_INSTANCE",
        SYSCALL_REMOVE_SMART_CONTRACT_BYTES => "REMOVE_SMART_CONTRACT_BYTES",
        // ZK verification/state-read
        SYSCALL_ZK_VERIFY_TRANSFER => "ZK_VERIFY_TRANSFER",
        SYSCALL_ZK_VERIFY_UNSHIELD => "ZK_VERIFY_UNSHIELD",
        SYSCALL_ZK_VOTE_VERIFY_BALLOT => "ZK_VOTE_VERIFY_BALLOT",
        SYSCALL_ZK_VOTE_VERIFY_TALLY => "ZK_VOTE_VERIFY_TALLY",
        SYSCALL_ZK_ROOTS_GET => "ZK_ROOTS_GET",
        SYSCALL_ZK_VOTE_GET_TALLY => "ZK_VOTE_GET_TALLY",
        SYSCALL_ZK_VERIFY_BATCH => "ZK_VERIFY_BATCH",
        // Codec helpers
        // Codec helpers (dev)
        SYSCALL_JSON_ENCODE => "JSON_ENCODE",
        SYSCALL_JSON_DECODE => "JSON_DECODE",
        SYSCALL_SCHEMA_ENCODE => "SCHEMA_ENCODE",
        SYSCALL_SCHEMA_DECODE => "SCHEMA_DECODE",
        SYSCALL_SCHEMA_INFO => "SCHEMA_INFO",
        SYSCALL_NAME_DECODE => "NAME_DECODE",
        SYSCALL_POINTER_TO_NORITO => "POINTER_TO_NORITO",
        SYSCALL_POINTER_FROM_NORITO => "POINTER_FROM_NORITO",
        SYSCALL_TLV_EQ => "TLV_EQ",
        // VRF
        SYSCALL_VRF_VERIFY => "VRF_VERIFY",
        SYSCALL_VRF_VERIFY_BATCH => "VRF_VERIFY_BATCH",
        // Dev/vendor helpers
        SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION => "SMARTCONTRACT_EXECUTE_INSTRUCTION",
        SYSCALL_SMARTCONTRACT_EXECUTE_QUERY => "SMARTCONTRACT_EXECUTE_QUERY",
        SYSCALL_CREATE_NFTS_FOR_ALL_USERS => "CREATE_NFTS_FOR_ALL_USERS",
        SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH => "SET_SMARTCONTRACT_EXECUTION_DEPTH",
        SYSCALL_GET_AUTHORITY => "GET_AUTHORITY",
        SYSCALL_SUBSCRIPTION_BILL => "SUBSCRIPTION_BILL",
        SYSCALL_SUBSCRIPTION_RECORD_USAGE => "SUBSCRIPTION_RECORD_USAGE",
        SYSCALL_AXT_BEGIN => "AXT_BEGIN",
        SYSCALL_AXT_TOUCH => "AXT_TOUCH",
        SYSCALL_AXT_COMMIT => "AXT_COMMIT",
        SYSCALL_VERIFY_DS_PROOF => "VERIFY_DS_PROOF",
        SYSCALL_USE_ASSET_HANDLE => "USE_ASSET_HANDLE",
        // ZK extras
        SYSCALL_GET_ACCOUNT_BALANCE => "GET_ACCOUNT_BALANCE",
        SYSCALL_USE_NULLIFIER => "USE_NULLIFIER",
        SYSCALL_GET_REGISTER_MERKLE_COMPACT => "GET_REGISTER_MERKLE_COMPACT",
        // Debug helper
        SYSCALL_DEBUG_PRINT => "DEBUG_PRINT",
        _ => return None,
    })
}

/// Render a minimal syscall list as markdown lines `- 0xNN NAME`.
pub fn render_syscalls_min_list() -> String {
    let mut nums: Vec<u32> = abi_syscall_list().to_vec();
    nums.sort_unstable();
    let mut out = String::new();
    for n in nums {
        if let Some(name) = syscall_name(n) {
            out.push_str(&format!("- 0x{n:02X} {name}\n"));
        } else {
            out.push_str(&format!("- 0x{n:02X}\n"));
        }
    }
    out
}

/// Structured doc row for a syscall.
pub struct SyscallDoc {
    pub number: u32,
    pub args: &'static str,
    pub ret: &'static str,
    pub gas: &'static str,
}

// Use the generated syscall docs table if present (preferred). This file is
// produced by the `gen_syscalls_doc` helper. It must define
// `pub static DOCS: &[SyscallDoc]`.
#[path = "syscalls_doc_gen.rs"]
mod syscalls_doc_gen;

// Minimal placeholder for gas spec documentation; avoid module dependency during tests.
// When code generation is enabled (via `gen_syscalls_doc.rs`), a generated file
// with the same items can be placed at `src/gas_spec.rs` and imported instead.
pub mod gas_spec {
    #[derive(Clone, Copy)]
    pub struct GasAsset {
        pub key: &'static str,
        pub asset_id: &'static str,
        pub unit: &'static str,
        pub version: &'static str,
        pub group: &'static str,
    }
    pub static GAS_ASSETS: &[GasAsset] = &[];
}

/// Render a markdown table with columns: Number, Name, Args, Return, Gas.
pub fn render_syscalls_markdown_table() -> String {
    let mut nums: Vec<u32> = abi_syscall_list().to_vec();
    nums.sort_unstable();
    let docs = syscalls_doc_gen::DOCS;
    let mut out = String::new();
    out.push_str("| Number | Name | Args | Return | Gas |\n");
    out.push_str("|---|---|---|---|---|\n");
    for n in nums {
        let name = syscall_name(n).unwrap_or("");
        let (mut args, mut ret, mut gas) = ("-", "-", "-");
        if let Some(d) = docs.iter().find(|d| d.number == n) {
            args = d.args;
            ret = d.ret;
            gas = d.gas;
        }
        out.push_str(&format!(
            "| 0x{n:02X} | {name} | {args} | {ret} | {gas} |\n"
        ));
    }
    out
}

/// Re-export generated gas assets for tests/docs (disabled in this build).
pub use gas_spec as _gas_spec_placeholder;

/// Render a markdown table with ABI policy names and their hashes.
///
/// The table is used in docs to surface the canonical `abi_hash` values
/// for the currently supported policies. Keep output stable (lowercase hex).
pub fn render_abi_hashes_markdown_table() -> String {
    fn hex_lower(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            use core::fmt::Write as _;
            let _ = write!(&mut s, "{b:02x}");
        }
        s
    }

    let items: &[(&str, crate::SyscallPolicy)] = &[("ABI v1", crate::SyscallPolicy::AbiV1)];

    let mut out = String::new();
    out.push_str("| Policy | abi_hash (hex) |\n");
    out.push_str("|---|---|\n");
    for (name, pol) in items {
        let h = compute_abi_hash(*pol);
        let hex = hex_lower(&h);
        let _ = core::fmt::Write::write_fmt(&mut out, format_args!("| {name} | {hex} |\n"));
    }
    out
}

/// Compute a stable 32-byte hash of the allowed syscall surface under `policy`.
///
/// The hash is computed over the domain tag `b"IVM_ABI_V1"`, the policy tag,
/// and the sorted list of allowed syscall numbers as little-endian `u32`.
pub fn compute_abi_hash(policy: crate::SyscallPolicy) -> [u8; 32] {
    use iroha_crypto::Hash;
    // Domain tag + policy tag
    let mut bytes = Vec::with_capacity(8 + 1 + syscalls_for_policy(policy).len() * 4);
    bytes.extend_from_slice(b"IVM_ABI_V1");
    let policy_tag: u8 = match policy {
        crate::SyscallPolicy::AbiV1 => 1,
        crate::SyscallPolicy::Experimental(v) => v.saturating_add(0x80),
    };
    bytes.push(policy_tag);
    // Policy-specific list is already sorted/deduped
    for n in syscalls_for_policy(policy) {
        bytes.extend_from_slice(&n.to_le_bytes());
    }
    *Hash::new(&bytes).as_ref()
}
