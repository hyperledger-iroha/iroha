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
//! discovery. Additional concurrency primitives may be added in future core
//! releases without changing the fixed ABI v1 surface in this release.

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
/// Batch-internal FASTPQ transfer gadget syscall.
pub const SYSCALL_TRANSFER_V1: u32 = 0x24;
/// Begin a FASTPQ transfer batch; subsequent `transfer_v1` calls are coalesced.
pub const SYSCALL_TRANSFER_V1_BATCH_BEGIN: u32 = 0x29;
/// End the current FASTPQ transfer batch scope.
pub const SYSCALL_TRANSFER_V1_BATCH_END: u32 = 0x2A;
/// Submit a pre-baked FASTPQ batch via a Norito-encoded [`TransferAssetBatch`].
pub const SYSCALL_TRANSFER_V1_BATCH_APPLY: u32 = 0x2B;
/// Transfer a numeric asset balance within an explicit dataspace scope.
pub const SYSCALL_TRANSFER_ASSET_SCOPED: u32 = 0x2C;

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
/// Enumerate durable-state keys under a prefix.
///
/// Args: r10 = &Name prefix, r11 = offset, r12 = limit (0 returns an empty page;
/// limits above [`STATE_KEYS_MAX_ITEMS`] are rejected)
/// Ret:  r10 = &NoritoBytes(Vec<Name>), r11 = total matching keys, r12 = returned keys
pub const SYSCALL_STATE_KEYS: u32 = 0x01_0030;
/// Test whether a durable-state key is currently present.
///
/// Args: r10 = &Name path
/// Ret:  r10 = 1 when present, 0 when absent
pub const SYSCALL_STATE_HAS: u32 = 0x01_0031;
/// Return the byte length of a durable-state value without copying the value.
///
/// Args: r10 = &Name path
/// Ret:  r10 = value payload length, r11 = 1 when present, 0 when absent
pub const SYSCALL_STATE_LEN: u32 = 0x01_0032;
/// Count durable-state keys under a prefix without copying the key list.
///
/// Args: r10 = &Name prefix
/// Ret:  r10 = total matching keys
pub const SYSCALL_STATE_COUNT: u32 = 0x01_0033;
/// Decode one canonical Kotodama `StateMap` key from a page returned by
/// [`SYSCALL_STATE_KEYS`].
///
/// Args: r10 = &NoritoBytes(Vec<Name>), r11 = &Name(base), r12 = index
/// Ret:  r10 = &NoritoBytes(canonical key), or 0 when index is out of range
pub const SYSCALL_STATE_MAP_KEY_AT: u32 = 0x01_0034;
/// Encode one compiler-flattened typed durable value.
///
/// Args: r10 = &NoritoBytes(StateValueSchemaV1), r11 = aligned raw word table,
/// r12 = word count
/// Ret: r10 = &NoritoBytes(StateValueRecordV1)
pub const SYSCALL_STATE_VALUE_ENCODE: u32 = 0x01_0035;
/// Decode one typed durable value into an aligned compiler word table.
///
/// Args: r10 = &NoritoBytes(StateValueSchemaV1),
/// r11 = &NoritoBytes(StateValueRecordV1); zero is rejected because absence is
/// represented by `StateMap.get`'s outer `Option`, never by a typed value
/// Ret: r10 = &Blob(pad:u8 then flattened u64 words)
pub const SYSCALL_STATE_VALUE_DECODE: u32 = 0x01_0036;
/// Maximum number of entries returned by one V1 durable-state key page.
pub const STATE_KEYS_MAX_ITEMS: u64 = 64;
/// Maximum framed canonical Norito `Name` payload accepted inside a V1
/// durable-state path TLV.
///
/// The 16 KiB envelope accommodates the independently bounded 4 KiB UTF-8
/// `StateMap` base, one separator, and the lowercase-hex expansion of a 4 KiB
/// canonical key, including deterministic Norito framing overhead.
pub const STATE_MAX_PATH_BYTES: usize = 16 * 1024;
/// Maximum raw `NoritoBytes` payload stored under one V1 durable-state path.
///
/// The bound leaves enough room beneath the one-million-cycle default for a
/// prepare-time worst-case read escrow, the path, and VM instruction gas.
pub const STATE_MAX_VALUE_BYTES: usize = 512 * 1024;
/// Maximum raw canonical Norito key-payload bytes accepted by V1 `StateMap` paths.
pub const STATE_MAP_MAX_KEY_BYTES: usize = 4 * 1024;
/// Maximum framed canonical Norito `Name` payload used as a V1 `StateMap` base.
pub const STATE_MAP_MAX_BASE_BYTES: usize = 4 * 1024;
/// Maximum encoded `Vec<Name>` page accepted by `STATE_MAP_KEY_AT`.
pub const STATE_MAP_MAX_PAGE_BYTES: usize = 1024 * 1024;
/// Decode a NoritoBytes value containing a signed decimal ASCII integer and return
/// the value in `x10` as a 64-bit signed integer (two's complement).
///
/// Args: r10 = &NoritoBytes (ASCII decimal)
/// Ret:  r10 = value (as u64 bits)
pub const SYSCALL_DECODE_INT: u32 = 0x53;
/// Return payload length for a pointer-ABI TLV.
///
/// Args: r10 = &TLV
/// Ret:  r10 = payload length (u64)
pub const SYSCALL_TLV_LEN: u32 = 0x77;

/// JSON object field getters.
///
/// All JSON_GET_* syscalls return a compiler-owned `Option<T>` sum handle.
/// Missing keys, non-object roots, and type mismatches return `Option::none`.
///
/// Args: r10 = &Json, r11 = &Name key
/// Ret:  r10 = `Option<T>` sum handle whose active payload is one ABI word.
/// Active payload: one `&Json` pointer.
pub const SYSCALL_JSON_GET_JSON: u32 = 0x79;
/// Active payload: one `&Name` pointer.
pub const SYSCALL_JSON_GET_NAME: u32 = 0x7A;
/// Active payload: one `&AccountId` pointer.
pub const SYSCALL_JSON_GET_ACCOUNT_ID: u32 = 0x7B;
/// Active payload: one `&NftId` pointer.
pub const SYSCALL_JSON_GET_NFT_ID: u32 = 0x7C;
/// Active payload: one `&Blob` pointer containing decoded lowercase `0x` hex bytes.
pub const SYSCALL_JSON_GET_BLOB_HEX: u32 = 0x7D;
/// Active payload: one `&AssetDefinitionId` pointer.
pub const SYSCALL_JSON_GET_ASSET_DEFINITION_ID: u32 = 0x80;
/// Construct an empty JSON object.
///
/// Args: none
/// Ret:  r10 = &Json (INPUT pointer)
pub const SYSCALL_JSON_OBJECT: u32 = 0x81;
/// Insert or replace an integer field in a JSON object.
///
/// Args: r10 = &Json object, r11 = &Name key, r12 = value (i64 as u64)
/// Ret:  r10 = &Json (INPUT pointer)
pub const SYSCALL_JSON_SET_I64: u32 = 0x82;
/// Insert or replace an account-id field in a JSON object using canonical string encoding.
///
/// Args: r10 = &Json object, r11 = &Name key, r12 = &AccountId
/// Ret:  r10 = &Json (INPUT pointer)
pub const SYSCALL_JSON_SET_ACCOUNT_ID: u32 = 0x83;
/// Direct JSON object getter that accepts validated TLVs from any allowed pointer region.
pub const SYSCALL_JSON_GET_JSON_DIRECT: u32 = 0x85;
/// Direct JSON name getter that accepts validated TLVs from any allowed pointer region.
pub const SYSCALL_JSON_GET_NAME_DIRECT: u32 = 0x86;
/// Direct JSON account-id getter that accepts validated TLVs from any allowed pointer region.
pub const SYSCALL_JSON_GET_ACCOUNT_ID_DIRECT: u32 = 0x87;
/// Direct JSON NFT-id getter that accepts validated TLVs from any allowed pointer region.
pub const SYSCALL_JSON_GET_NFT_ID_DIRECT: u32 = 0x88;
/// Direct JSON blob getter that accepts validated TLVs from any allowed pointer region.
pub const SYSCALL_JSON_GET_BLOB_HEX_DIRECT: u32 = 0x89;
/// Direct JSON asset-definition getter that accepts validated TLVs from any allowed pointer region.
pub const SYSCALL_JSON_GET_ASSET_DEFINITION_ID_DIRECT: u32 = 0x8B;
/// Direct JSON integer setter that accepts validated TLVs from any allowed pointer region.
pub const SYSCALL_JSON_SET_I64_DIRECT: u32 = 0x8C;
/// Direct JSON account-id setter that accepts validated TLVs from any allowed pointer region.
pub const SYSCALL_JSON_SET_ACCOUNT_ID_DIRECT: u32 = 0x8D;
/// Direct path-key hashing helper that accepts validated TLVs from any allowed pointer region.
pub const SYSCALL_BUILD_PATH_KEY_NORITO_DIRECT: u32 = 0x8E;
/// Direct schema-info helper that accepts validated TLVs from any allowed pointer region.
pub const SYSCALL_SCHEMA_INFO_DIRECT: u32 = 0x8F;

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
/// `"/" + lowercase_hex(payload)`.
///
/// The encoding is injective and its lexical order is the unsigned bytewise
/// order of canonical Norito key payloads. Payloads larger than
/// [`STATE_MAP_MAX_KEY_BYTES`] are rejected.
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
/// Direct schema encode helper that accepts validated TLVs from any allowed pointer region.
pub const SYSCALL_SCHEMA_ENCODE_DIRECT: u32 = 0xD0;
/// Direct schema decode helper that accepts validated TLVs from any allowed pointer region.
pub const SYSCALL_SCHEMA_DECODE_DIRECT: u32 = 0xD1;
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
/// Read a VRF epoch seed snapshot from world state for governance sortition.
///
/// Args: `r10 = &NoritoBytes(VrfEpochSeedRequest)`
/// Return: `r10 = ptr (&NoritoBytes(VrfEpochSeedResponse)), r11 = status:u64`
pub const SYSCALL_VRF_EPOCH_SEED: u32 = 0x7E;

/// Hardware and proof generation helpers.
/// Return a deterministic Norito-encoded execution-proof summary.
pub const SYSCALL_PROVE_EXECUTION: u32 = 0xF4;
/// Increase heap size by the number of bytes in `x10`.
pub const SYSCALL_GROW_HEAP: u32 = 0xF5;
/// Verify a Norito-encoded OpenVerifyEnvelope against the on-chain verifying-key registry.
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
/// Compute SHA-256 hash of a blob (`&Blob` -> `&Blob`).
pub const SYSCALL_SHA256_HASH: u32 = 0x96;
/// Compute SHA3-256 hash of a blob (`&Blob` -> `&Blob`).
pub const SYSCALL_SHA3_HASH: u32 = 0x97;
/// Compute raw Blake2b-256 hash of a blob (`&Blob` -> `&Blob`).
pub const SYSCALL_BLAKE2B256_HASH: u32 = 0x98;
/// Compute Keccak-256 hash of a blob (`&Blob` -> `&Blob`).
pub const SYSCALL_KECCAK256_HASH: u32 = 0x99;
/// Compute Iroha's canonical ledger hash (`Hash::new`) of a blob (`&Blob` -> `&Blob`).
pub const SYSCALL_IROHA_HASH: u32 = 0x9A;
/// Developer helper: validate a public TLV and return a host-usable pointer.
///
/// Expects `x10` to hold a pointer to a valid public TLV. INPUT and allocated-HEAP host results are
/// retained in place. Immutable program literals are materialized in the host arena (INPUT when
/// space remains, otherwise allocated HEAP). Stack/output, private, unallocated, malformed, and
/// ABI-disallowed envelopes are rejected.
pub const SYSCALL_INPUT_PUBLISH_TLV: u32 = 0xE0;

// Smart-contract host shims (development API)
/// Execute a built-in instruction from an IVM smart contract (pointer-ABI).
pub const SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION: u32 = 0xA0;
/// `r11` operation tag authorizing a decoded `SubmitBallot` instruction for syscall `0xA0`.
pub const SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT: u64 = 1;
/// `r11` operation tag authorizing a decoded `Unshield` instruction for syscall `0xA0`.
pub const SMARTCONTRACT_INSTRUCTION_TAG_UNSHIELD: u64 = 2;
/// `r11` operation tag authorizing a decoded `RecordSccpMessage` instruction for syscall `0xA0`.
pub const SMARTCONTRACT_INSTRUCTION_TAG_RECORD_SCCP_MESSAGE: u64 = 3;
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
/// Resolve a canonical alias literal (for example `merchant@centralbank`) to the current AccountId.
pub const SYSCALL_RESOLVE_ACCOUNT_ALIAS: u32 = 0xA7;
/// Get the current trusted host time in unix milliseconds.
pub const SYSCALL_CURRENT_TIME_MS: u32 = 0xA8;
/// Call a deployed ABI v1 contract synchronously by contract-address literal.
pub const SYSCALL_CALL_CONTRACT: u32 = 0xA9;
/// Open and fund a native anonymous asset escrow with shielded proof material.
pub const SYSCALL_ANONYMOUS_ESCROW_OPEN_OFFER: u32 = 0xAA;
/// Accept an open native anonymous asset escrow.
pub const SYSCALL_ANONYMOUS_ESCROW_ACCEPT: u32 = 0xAB;
/// Mark accepted native anonymous escrow off-chain payment as sent.
pub const SYSCALL_ANONYMOUS_ESCROW_MARK_PAYMENT_SENT: u32 = 0xAC;
/// Release a paid native anonymous escrow to the buyer with shielded outputs.
pub const SYSCALL_ANONYMOUS_ESCROW_RELEASE: u32 = 0xAD;
/// Cancel and refund a native anonymous escrow with shielded outputs.
pub const SYSCALL_ANONYMOUS_ESCROW_CANCEL: u32 = 0xAE;
/// Open a dispute for native anonymous escrow court moderation.
pub const SYSCALL_ANONYMOUS_ESCROW_OPEN_DISPUTE: u32 = 0xAF;
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

/// Open and fund a native asset escrow.
pub const SYSCALL_ESCROW_OPEN_OFFER: u32 = 0xB8;
/// Accept an open native asset escrow.
pub const SYSCALL_ESCROW_ACCEPT: u32 = 0xB9;
/// Mark accepted escrow off-chain payment as sent.
pub const SYSCALL_ESCROW_MARK_PAYMENT_SENT: u32 = 0xBA;
/// Release a paid escrow to the buyer.
pub const SYSCALL_ESCROW_RELEASE: u32 = 0xBB;
/// Cancel and refund an escrow before payment is marked.
pub const SYSCALL_ESCROW_CANCEL: u32 = 0xBC;
/// Open a dispute for court moderation.
pub const SYSCALL_ESCROW_OPEN_DISPUTE: u32 = 0xBD;
/// Resolve a disputed escrow with a buyer/seller split.
pub const SYSCALL_ESCROW_RESOLVE_DISPUTE: u32 = 0xBE;
/// Resolve a disputed native anonymous escrow with shielded buyer/seller outputs.
pub const SYSCALL_ANONYMOUS_ESCROW_RESOLVE_DISPUTE: u32 = 0xBF;

/// Soracloud runtime host surface.
/// Read committed service-state metadata for handler-local execution.
pub const SYSCALL_SORACLOUD_READ_COMMITTED_STATE: u32 = 0xC0;
/// Emit a deterministic service-state mutation staged for authoritative write-back.
pub const SYSCALL_SORACLOUD_EMIT_STATE_MUTATION: u32 = 0xC1;
/// Emit an outbound Soracloud mailbox message.
pub const SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE: u32 = 0xC2;
/// Append deterministic runtime journal material.
pub const SYSCALL_SORACLOUD_APPEND_JOURNAL: u32 = 0xC3;
/// Publish a checkpoint artifact.
pub const SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT: u32 = 0xC4;
/// Read node-local secret material exposed only through the Soracloud host.
pub const SYSCALL_SORACLOUD_READ_SECRET: u32 = 0xC5;
/// Read node-local credential material exposed only through the Soracloud host.
pub const SYSCALL_SORACLOUD_READ_CREDENTIAL: u32 = 0xC6;
/// Perform a bounded, policy-checked egress fetch against allowlisted hosts.
pub const SYSCALL_SORACLOUD_EGRESS_FETCH: u32 = 0xC7;
/// Read authoritative service config material exposed through the Soracloud host.
pub const SYSCALL_SORACLOUD_READ_CONFIG: u32 = 0xC8;
/// Read authoritative service secret envelopes exposed through the Soracloud host.
pub const SYSCALL_SORACLOUD_READ_SECRET_ENVELOPE: u32 = 0xC9;

/// Execute an arbitrary Norito-encoded read-only query request.
pub const SYSCALL_QUERY_EXECUTE_NORITO: u32 = 0x01_0000;
/// Read one projected core-ledger entity by stable [`CoreQueryEntityTagV1`].
///
/// [`CoreQueryEntityTagV1`]: crate::core_query::CoreQueryEntityTagV1
pub const SYSCALL_CORE_QUERY_GET: u32 = 0x01_0001;
/// Read one bounded page of projected core-ledger entities by stable
/// [`CoreQueryEntityTagV1`].
///
/// [`CoreQueryEntityTagV1`]: crate::core_query::CoreQueryEntityTagV1
pub const SYSCALL_CORE_QUERY_PAGE: u32 = 0x01_0002;
/// Read one runtime/system/custom parameter by canonical name.
pub const SYSCALL_QUERY_GET_PARAMETER: u32 = 0x01_0006;
/// Read one contract manifest by code hash.
pub const SYSCALL_QUERY_GET_CONTRACT_MANIFEST: u32 = 0x01_0007;
/// Read one contract instance by address/alias.
pub const SYSCALL_QUERY_GET_CONTRACT_INSTANCE: u32 = 0x01_0008;

/// Return the current chain id as a pointer-ABI `Blob` TLV.
pub const SYSCALL_SYSVAR_CHAIN_ID: u32 = 0x01_0020;
/// Return the current block height in `r10`.
pub const SYSCALL_SYSVAR_BLOCK_HEIGHT: u32 = 0x01_0021;
/// Return the current block timestamp in milliseconds in `r10`.
pub const SYSCALL_SYSVAR_BLOCK_TIME_MS: u32 = 0x01_0022;
/// Return the current authority as an `AccountId` TLV.
pub const SYSCALL_SYSVAR_AUTHORITY: u32 = 0x01_0023;
/// Return the current contract address as a NoritoBytes TLV, or zero when not in a contract scope.
pub const SYSCALL_SYSVAR_CONTRACT_ADDRESS: u32 = 0x01_0024;
/// Return the current contract entrypoint name as a `Blob` TLV, or zero when not in a contract scope.
pub const SYSCALL_SYSVAR_ENTRYPOINT: u32 = 0x01_0025;
/// Return the immutable subject account of the currently executing deployed contract.
///
/// The result is an `AccountId` TLV in `r10`. Calls outside a deployed-contract
/// scope fail closed instead of falling back to transaction authority.
pub const SYSCALL_SYSVAR_CONTRACT_SUBJECT: u32 = 0x01_0027;
/// Decode a complete schema-bound public argument record.
///
/// Args: r10 = `&NoritoBytes(EntrypointArgumentRecordV1)` for raw hosts, or
/// the host-issued domain-separated record binding for a prepared invocation;
/// r11 = `&NoritoBytes(EntrypointArgumentSchemaV1)`.
/// Ret: r10 = `&Blob(0u8 || [u64; word_count])`; the leading byte aligns the
/// declaration-ordered flattened words, which contain sum tags, canonical
/// scalar bits, or validated pointer-ABI addresses.
pub const SYSCALL_DECODE_ARGUMENT_RECORD: u32 = 0x01_0026;

/// Set or clear native outbound-transfer freeze state for one account/asset pair.
///
/// Args: `r10 = &AccountId`, `r11 = &AssetDefinitionId`, `r12 = bool`.
pub const SYSCALL_SET_ASSET_TRANSFER_FREEZE: u32 = 0x01_0200;
/// Set the native UTC daily outbound-transfer cap for one account/asset pair.
///
/// Args: `r10 = &AccountId`, `r11 = &AssetDefinitionId`, `r12 = &Quantity`.
pub const SYSCALL_SET_ASSET_TRANSFER_DAILY_LIMIT: u32 = 0x01_0201;
/// Propose native alias-based account recovery with a replacement controller.
///
/// Args: `r10 = &Blob(alias literal)`, `r11 = &AccountId(replacement controller)`.
pub const SYSCALL_ACCOUNT_RECOVERY_PROPOSE: u32 = 0x01_0210;
/// Approve the pending native recovery request for an alias.
///
/// Args: `r10 = &Blob(alias literal)`.
pub const SYSCALL_ACCOUNT_RECOVERY_APPROVE: u32 = 0x01_0211;
/// Cancel the pending native recovery request for an alias.
///
/// Args: `r10 = &Blob(alias literal)`.
pub const SYSCALL_ACCOUNT_RECOVERY_CANCEL: u32 = 0x01_0212;
/// Finalize the pending native recovery request for an alias.
///
/// Args: `r10 = &Blob(alias literal)`.
pub const SYSCALL_ACCOUNT_RECOVERY_FINALIZE: u32 = 0x01_0213;

// Kotodama V1 exact numeric families. These numbers are deliberately grouped
// by value domain so admission, host dispatch, and generated SDK tables can
// classify them without depending on source-language spellings.

/// Construct an `int` from the two's-complement bits of an `i64` in `r10`.
pub const SYSCALL_INT_FROM_I64: u32 = 0x01_0100;
/// Construct an `int` from a `u64` in `r10`.
pub const SYSCALL_INT_FROM_U64: u32 = 0x01_0101;
/// Convert an `int` to `i64`; range failure is returned in `r11`.
pub const SYSCALL_INT_TRY_TO_I64: u32 = 0x01_0102;
/// Convert an `int` to `u64`; sign/range failure is returned in `r11`.
pub const SYSCALL_INT_TRY_TO_U64: u32 = 0x01_0103;
/// Checked integer negation.
pub const SYSCALL_INT_NEG: u32 = 0x01_0104;
/// Checked integer addition.
pub const SYSCALL_INT_ADD: u32 = 0x01_0105;
/// Checked integer subtraction.
pub const SYSCALL_INT_SUB: u32 = 0x01_0106;
/// Checked integer multiplication.
pub const SYSCALL_INT_MUL: u32 = 0x01_0107;
/// Integer division truncated toward zero.
pub const SYSCALL_INT_DIV: u32 = 0x01_0108;
/// Integer remainder paired with truncation-toward-zero division.
pub const SYSCALL_INT_REM: u32 = 0x01_0109;
/// Numeric integer equality.
pub const SYSCALL_INT_EQ: u32 = 0x01_010A;
/// Numeric integer inequality.
pub const SYSCALL_INT_NE: u32 = 0x01_010B;
/// Numeric integer less-than comparison.
pub const SYSCALL_INT_LT: u32 = 0x01_010C;
/// Numeric integer less-or-equal comparison.
pub const SYSCALL_INT_LE: u32 = 0x01_010D;
/// Numeric integer greater-than comparison.
pub const SYSCALL_INT_GT: u32 = 0x01_010E;
/// Numeric integer greater-or-equal comparison.
pub const SYSCALL_INT_GE: u32 = 0x01_010F;
/// Integer negation modulo `2^512`, interpreted in the signed domain.
pub const SYSCALL_INT_WRAP_NEG: u32 = 0x01_0110;
/// Integer addition modulo `2^512`, interpreted in the signed domain.
pub const SYSCALL_INT_WRAP_ADD: u32 = 0x01_0111;
/// Integer subtraction modulo `2^512`, interpreted in the signed domain.
pub const SYSCALL_INT_WRAP_SUB: u32 = 0x01_0112;
/// Integer multiplication modulo `2^512`, interpreted in the signed domain.
pub const SYSCALL_INT_WRAP_MUL: u32 = 0x01_0113;

/// Convert an `int` to an exact scale-zero `decimal`.
pub const SYSCALL_DECIMAL_FROM_INT: u32 = 0x01_0120;
/// Checked decimal negation.
pub const SYSCALL_DECIMAL_NEG: u32 = 0x01_0121;
/// Exact decimal addition.
pub const SYSCALL_DECIMAL_ADD: u32 = 0x01_0122;
/// Exact decimal subtraction.
pub const SYSCALL_DECIMAL_SUB: u32 = 0x01_0123;
/// Exact decimal multiplication; canonical results requiring scale above 28 fail.
pub const SYSCALL_DECIMAL_MUL: u32 = 0x01_0124;
/// Exact finite decimal division at the denominator's proven minimum scale.
pub const SYSCALL_DECIMAL_DIV_EXACT: u32 = 0x01_0125;
/// Decimal division rounded at an explicit output scale.
pub const SYSCALL_DECIMAL_DIV_ROUND: u32 = 0x01_0126;
/// Numeric decimal equality.
pub const SYSCALL_DECIMAL_EQ: u32 = 0x01_0127;
/// Numeric decimal inequality.
pub const SYSCALL_DECIMAL_NE: u32 = 0x01_0128;
/// Numeric decimal less-than comparison.
pub const SYSCALL_DECIMAL_LT: u32 = 0x01_0129;
/// Numeric decimal less-or-equal comparison.
pub const SYSCALL_DECIMAL_LE: u32 = 0x01_012A;
/// Numeric decimal greater-than comparison.
pub const SYSCALL_DECIMAL_GT: u32 = 0x01_012B;
/// Numeric decimal greater-or-equal comparison.
pub const SYSCALL_DECIMAL_GE: u32 = 0x01_012C;
/// Convert a scale-zero decimal to `int`; inexact conversion returns a status.
pub const SYSCALL_DECIMAL_TRY_TO_INT_EXACT: u32 = 0x01_012D;
/// Convert a decimal to `int` by truncating toward zero.
pub const SYSCALL_DECIMAL_TO_INT_TRUNC: u32 = 0x01_012E;
/// Convert a decimal to `int` using an explicit rounding mode.
pub const SYSCALL_DECIMAL_TO_INT_ROUND: u32 = 0x01_012F;

/// Convert a non-negative `int` to nominal `quantity`.
pub const SYSCALL_QUANTITY_TRY_FROM_INT: u32 = 0x01_0140;
/// Convert a non-negative canonical `decimal` to nominal `quantity`.
pub const SYSCALL_QUANTITY_TRY_FROM_DECIMAL: u32 = 0x01_0141;
/// Convert a `quantity` to the same-valued `decimal`.
pub const SYSCALL_QUANTITY_TO_DECIMAL: u32 = 0x01_0142;
/// Exact quantity addition.
pub const SYSCALL_QUANTITY_ADD: u32 = 0x01_0143;
/// Exact quantity subtraction; negative results fail with quantity underflow.
pub const SYSCALL_QUANTITY_SUB: u32 = 0x01_0144;
/// Multiply a quantity by a decimal; the result must remain non-negative.
pub const SYSCALL_QUANTITY_MUL_DECIMAL: u32 = 0x01_0145;
/// Divide a quantity by a decimal exactly; the result must remain non-negative.
pub const SYSCALL_QUANTITY_DIV_DECIMAL_EXACT: u32 = 0x01_0146;
/// Divide a quantity by a decimal using explicit scale and rounding.
pub const SYSCALL_QUANTITY_DIV_DECIMAL_ROUND: u32 = 0x01_0147;
/// Compute an exact decimal ratio of two quantities.
pub const SYSCALL_QUANTITY_RATIO_EXACT: u32 = 0x01_0148;
/// Compute a rounded decimal ratio of two quantities.
pub const SYSCALL_QUANTITY_RATIO_ROUND: u32 = 0x01_0149;
/// Numeric quantity equality.
pub const SYSCALL_QUANTITY_EQ: u32 = 0x01_014A;
/// Numeric quantity inequality.
pub const SYSCALL_QUANTITY_NE: u32 = 0x01_014B;
/// Numeric quantity less-than comparison.
pub const SYSCALL_QUANTITY_LT: u32 = 0x01_014C;
/// Numeric quantity less-or-equal comparison.
pub const SYSCALL_QUANTITY_LE: u32 = 0x01_014D;
/// Numeric quantity greater-than comparison.
pub const SYSCALL_QUANTITY_GT: u32 = 0x01_014E;
/// Numeric quantity greater-or-equal comparison.
pub const SYSCALL_QUANTITY_GE: u32 = 0x01_014F;

/// Parse a canonical base-10 JSON string into an exact `int` pointer.
pub const SYSCALL_JSON_GET_INT: u32 = 0x01_0160;
/// Parse a canonical base-10 JSON string into an exact `decimal` pointer.
pub const SYSCALL_JSON_GET_DECIMAL: u32 = 0x01_0161;
/// Parse a canonical non-negative base-10 JSON string into a `quantity` pointer.
pub const SYSCALL_JSON_GET_QUANTITY: u32 = 0x01_0162;
/// Direct form of [`SYSCALL_JSON_GET_INT`].
pub const SYSCALL_JSON_GET_INT_DIRECT: u32 = 0x01_0163;
/// Direct form of [`SYSCALL_JSON_GET_DECIMAL`].
pub const SYSCALL_JSON_GET_DECIMAL_DIRECT: u32 = 0x01_0164;
/// Direct form of [`SYSCALL_JSON_GET_QUANTITY`].
pub const SYSCALL_JSON_GET_QUANTITY_DIRECT: u32 = 0x01_0165;

/// Return whether `number` belongs to the exact Kotodama V1 numeric surface.
#[must_use]
pub const fn is_numeric_v1_syscall(number: u32) -> bool {
    matches!(number, 0x01_0100..=0x01_0113 | 0x01_0120..=0x01_012F | 0x01_0140..=0x01_014F)
}

/// Construct one native JSON value from a compiler-emitted schema and flattened words.
///
/// Args: r10 = `&NoritoBytes(JsonConstructionSchemaV1)`, r11 = aligned public
/// word-table address, r12 = exact word count.
/// Ret: r10 = `&Json`.
pub const SYSCALL_JSON_BUILD: u32 = 0x01_004E;

/// Return whether `number` is one of the canonical typed JSON getters.
#[must_use]
pub const fn is_json_getter_syscall(number: u32) -> bool {
    matches!(
        number,
        SYSCALL_JSON_GET_JSON
            | SYSCALL_JSON_GET_NAME
            | SYSCALL_JSON_GET_ACCOUNT_ID
            | SYSCALL_JSON_GET_NFT_ID
            | SYSCALL_JSON_GET_BLOB_HEX
            | SYSCALL_JSON_GET_ASSET_DEFINITION_ID
            | SYSCALL_JSON_GET_INT
            | SYSCALL_JSON_GET_DECIMAL
            | SYSCALL_JSON_GET_QUANTITY
    )
}

/// Kotodama test-runner helper: resolve a fixture actor alias to an `AccountId` TLV.
///
/// These host-private helpers are intentionally outside [`abi_syscall_list`].
/// Production hosts reject them, while `koto_test` opts in explicitly when it
/// executes test-mode bytecode.
pub const SYSCALL_KOTO_TEST_ACTOR_ACCOUNT: u32 = 0x00FE_0001;
/// Kotodama test-runner helper: return a fixture actor public key as a `Blob` TLV.
pub const SYSCALL_KOTO_TEST_ACTOR_PUBLIC_KEY: u32 = 0x00FE_0002;
/// Kotodama test-runner helper: sign a message with a fixture actor seed.
pub const SYSCALL_KOTO_TEST_ACTOR_SIGN: u32 = 0x00FE_0003;
/// Kotodama test-runner helper: invoke a contract entrypoint as a fixture actor.
pub const SYSCALL_KOTO_TEST_INVOKE_ENTRYPOINT_AS: u32 = 0x00FE_0004;
/// Kotodama test-runner helper: assert that an actor entrypoint invocation rejects.
pub const SYSCALL_KOTO_TEST_EXPECT_REJECT_AS: u32 = 0x00FE_0005;

/// Return whether `number` belongs to the host-private Kotodama test surface.
pub const fn is_koto_test_syscall(number: u32) -> bool {
    matches!(
        number,
        SYSCALL_KOTO_TEST_ACTOR_ACCOUNT
            | SYSCALL_KOTO_TEST_ACTOR_PUBLIC_KEY
            | SYSCALL_KOTO_TEST_ACTOR_SIGN
            | SYSCALL_KOTO_TEST_INVOKE_ENTRYPOINT_AS
            | SYSCALL_KOTO_TEST_EXPECT_REJECT_AS
    )
}

/// Map direct helper syscall aliases onto their canonical helper numbers.
///
/// The first-release ABI exposes both canonical helper syscalls and direct
/// aliases that relax pointer-region placement once the TLV has already been
/// validated. Hosts execute the canonical implementation for both forms to
/// keep behavior identical across runtimes.
pub const fn canonical_helper_syscall(number: u32) -> u32 {
    match number {
        SYSCALL_JSON_GET_JSON_DIRECT => SYSCALL_JSON_GET_JSON,
        SYSCALL_JSON_GET_NAME_DIRECT => SYSCALL_JSON_GET_NAME,
        SYSCALL_JSON_GET_ACCOUNT_ID_DIRECT => SYSCALL_JSON_GET_ACCOUNT_ID,
        SYSCALL_JSON_GET_NFT_ID_DIRECT => SYSCALL_JSON_GET_NFT_ID,
        SYSCALL_JSON_GET_BLOB_HEX_DIRECT => SYSCALL_JSON_GET_BLOB_HEX,
        SYSCALL_JSON_GET_INT_DIRECT => SYSCALL_JSON_GET_INT,
        SYSCALL_JSON_GET_DECIMAL_DIRECT => SYSCALL_JSON_GET_DECIMAL,
        SYSCALL_JSON_GET_QUANTITY_DIRECT => SYSCALL_JSON_GET_QUANTITY,
        SYSCALL_JSON_GET_ASSET_DEFINITION_ID_DIRECT => SYSCALL_JSON_GET_ASSET_DEFINITION_ID,
        SYSCALL_JSON_SET_I64_DIRECT => SYSCALL_JSON_SET_I64,
        SYSCALL_JSON_SET_ACCOUNT_ID_DIRECT => SYSCALL_JSON_SET_ACCOUNT_ID,
        SYSCALL_BUILD_PATH_KEY_NORITO_DIRECT => SYSCALL_BUILD_PATH_KEY_NORITO,
        SYSCALL_SCHEMA_INFO_DIRECT => SYSCALL_SCHEMA_INFO,
        SYSCALL_SCHEMA_ENCODE_DIRECT => SYSCALL_SCHEMA_ENCODE,
        SYSCALL_SCHEMA_DECODE_DIRECT => SYSCALL_SCHEMA_DECODE,
        _ => number,
    }
}

/// Returns whether a syscall number is allowed for the given ABI policy.
///
/// This function centralizes the mapping between `ProgramMetadata.abi_version`
/// and the set of syscalls available to programs compiled against that ABI.
/// Hosts should call this before attempting to handle a syscall to ensure
/// stable first-release behavior; unknown or disallowed numbers must be
/// rejected with `VMError::UnknownSyscall`.
pub fn is_syscall_allowed(policy: crate::SyscallPolicy, number: u32) -> bool {
    syscalls_for_policy(policy).binary_search(&number).is_ok()
}

/// Host-state access conservatively implied by an ABI syscall.
///
/// This classification is intentionally independent of compiler metadata. It
/// is used by admission and the parallel scheduler to reject or serialize
/// bytecode whose declared access set cannot be proven from its instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallAccess {
    /// The call only touches VM-local data or immutable execution context.
    None,
    /// Contract-owned durable state read.
    StateRead,
    /// Contract-owned durable state write.
    StateWrite,
    /// Ledger/world-state read with arguments not proven by the bytecode scanner.
    LedgerRead,
    /// Ledger/world-state write with arguments not proven by the bytecode scanner.
    LedgerWrite,
    /// Nested, opaque, or externally routed access; serialize conservatively.
    Dynamic,
}

/// Return the explicitly registered host-state access class for a syscall.
///
/// Unlike [`syscall_access`], this function has no conservative fallback. Host
/// metering uses it to distinguish a deliberately classified dynamic syscall
/// from a newly allowed syscall whose security metadata was never registered.
/// Such a syscall must fail closed during preparation.
#[must_use]
pub const fn registered_syscall_access(number: u32) -> Option<SyscallAccess> {
    if is_numeric_v1_syscall(number) || number == SYSCALL_JSON_BUILD {
        return Some(SyscallAccess::None);
    }
    if matches!(
        number,
        SYSCALL_STATE_MAP_KEY_AT | SYSCALL_STATE_VALUE_ENCODE | SYSCALL_STATE_VALUE_DECODE
    ) {
        return Some(SyscallAccess::None);
    }
    if matches!(
        number,
        SYSCALL_STATE_GET
            | SYSCALL_STATE_KEYS
            | SYSCALL_STATE_HAS
            | SYSCALL_STATE_LEN
            | SYSCALL_STATE_COUNT
    ) {
        return Some(SyscallAccess::StateRead);
    }
    if matches!(number, SYSCALL_STATE_SET | SYSCALL_STATE_DEL) {
        return Some(SyscallAccess::StateWrite);
    }
    if matches!(
        number,
        SYSCALL_SMARTCONTRACT_EXECUTE_QUERY
            | SYSCALL_QUERY_EXECUTE_NORITO
            | SYSCALL_CORE_QUERY_GET
            | SYSCALL_CORE_QUERY_PAGE
            | SYSCALL_QUERY_GET_PARAMETER
            | SYSCALL_QUERY_GET_CONTRACT_MANIFEST
            | SYSCALL_QUERY_GET_CONTRACT_INSTANCE
            | SYSCALL_GET_ACCOUNT_BALANCE
            | SYSCALL_RESOLVE_ACCOUNT_ALIAS
            | SYSCALL_VRF_EPOCH_SEED
            | SYSCALL_ZK_ROOTS_GET
            | SYSCALL_ZK_VOTE_GET_TALLY
            | SYSCALL_SORACLOUD_READ_COMMITTED_STATE
            | SYSCALL_SORACLOUD_READ_SECRET
            | SYSCALL_SORACLOUD_READ_CREDENTIAL
            | SYSCALL_SORACLOUD_EGRESS_FETCH
            | SYSCALL_SORACLOUD_READ_CONFIG
            | SYSCALL_SORACLOUD_READ_SECRET_ENVELOPE
    ) {
        return Some(SyscallAccess::LedgerRead);
    }
    if matches!(
        number,
        SYSCALL_REGISTER_DOMAIN
            | SYSCALL_UNREGISTER_DOMAIN
            | SYSCALL_TRANSFER_DOMAIN
            | SYSCALL_REGISTER_PEER
            | SYSCALL_UNREGISTER_PEER
            | SYSCALL_REGISTER_ACCOUNT
            | SYSCALL_UNREGISTER_ACCOUNT
            | SYSCALL_ADD_SIGNATORY
            | SYSCALL_REMOVE_SIGNATORY
            | SYSCALL_SET_ACCOUNT_QUORUM
            | SYSCALL_SET_ACCOUNT_DETAIL
            | SYSCALL_REGISTER_ASSET
            | SYSCALL_UNREGISTER_ASSET
            | SYSCALL_MINT_ASSET
            | SYSCALL_BURN_ASSET
            | SYSCALL_TRANSFER_V1
            | SYSCALL_TRANSFER_V1_BATCH_BEGIN
            | SYSCALL_TRANSFER_V1_BATCH_END
            | SYSCALL_TRANSFER_V1_BATCH_APPLY
            | SYSCALL_TRANSFER_ASSET_SCOPED
            | SYSCALL_NFT_MINT_ASSET
            | SYSCALL_NFT_TRANSFER_ASSET
            | SYSCALL_NFT_SET_METADATA
            | SYSCALL_NFT_BURN_ASSET
            | SYSCALL_CREATE_ROLE
            | SYSCALL_DELETE_ROLE
            | SYSCALL_GRANT_ROLE
            | SYSCALL_REVOKE_ROLE
            | SYSCALL_GRANT_PERMISSION
            | SYSCALL_REVOKE_PERMISSION
            | SYSCALL_CREATE_TRIGGER
            | SYSCALL_REMOVE_TRIGGER
            | SYSCALL_SET_TRIGGER_ENABLED
            | SYSCALL_REGISTER_SMART_CONTRACT_CODE
            | SYSCALL_REGISTER_SMART_CONTRACT_BYTES
            | SYSCALL_ACTIVATE_CONTRACT_INSTANCE
            | SYSCALL_DEACTIVATE_CONTRACT_INSTANCE
            | SYSCALL_REMOVE_SMART_CONTRACT_BYTES
            | SYSCALL_ZK_VERIFY_TRANSFER
            | SYSCALL_ZK_VERIFY_UNSHIELD
            | SYSCALL_ZK_VOTE_VERIFY_BALLOT
            | SYSCALL_ZK_VOTE_VERIFY_TALLY
            | SYSCALL_ZK_VERIFY_BATCH
            | SYSCALL_USE_NULLIFIER
            | SYSCALL_AXT_BEGIN
            | SYSCALL_AXT_TOUCH
            | SYSCALL_AXT_COMMIT
            | SYSCALL_VERIFY_DS_PROOF
            | SYSCALL_USE_ASSET_HANDLE
            | SYSCALL_ANONYMOUS_ESCROW_OPEN_OFFER
            | SYSCALL_ANONYMOUS_ESCROW_ACCEPT
            | SYSCALL_ANONYMOUS_ESCROW_MARK_PAYMENT_SENT
            | SYSCALL_ANONYMOUS_ESCROW_RELEASE
            | SYSCALL_ANONYMOUS_ESCROW_CANCEL
            | SYSCALL_ANONYMOUS_ESCROW_OPEN_DISPUTE
            | SYSCALL_ANONYMOUS_ESCROW_RESOLVE_DISPUTE
            | SYSCALL_ESCROW_OPEN_OFFER
            | SYSCALL_ESCROW_ACCEPT
            | SYSCALL_ESCROW_MARK_PAYMENT_SENT
            | SYSCALL_ESCROW_RELEASE
            | SYSCALL_ESCROW_CANCEL
            | SYSCALL_ESCROW_OPEN_DISPUTE
            | SYSCALL_ESCROW_RESOLVE_DISPUTE
            | SYSCALL_SUBSCRIPTION_BILL
            | SYSCALL_SUBSCRIPTION_RECORD_USAGE
            | SYSCALL_SORACLOUD_EMIT_STATE_MUTATION
            | SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE
            | SYSCALL_SORACLOUD_APPEND_JOURNAL
            | SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT
            | SYSCALL_SET_ASSET_TRANSFER_FREEZE
            | SYSCALL_SET_ASSET_TRANSFER_DAILY_LIMIT
            | SYSCALL_ACCOUNT_RECOVERY_PROPOSE
            | SYSCALL_ACCOUNT_RECOVERY_APPROVE
            | SYSCALL_ACCOUNT_RECOVERY_CANCEL
            | SYSCALL_ACCOUNT_RECOVERY_FINALIZE
    ) {
        return Some(SyscallAccess::LedgerWrite);
    }
    if matches!(
        number,
        SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION
            | SYSCALL_CALL_CONTRACT
            | SYSCALL_CREATE_NFTS_FOR_ALL_USERS
            | SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH
            | SYSCALL_COMMIT_OUTPUT
    ) {
        return Some(SyscallAccess::Dynamic);
    }
    if matches!(
        number,
        SYSCALL_EXIT
            | SYSCALL_ABORT
            | SYSCALL_DEBUG_PRINT
            | SYSCALL_DEBUG_LOG
            | SYSCALL_ALLOC
            | SYSCALL_GROW_HEAP
            | SYSCALL_GET_PUBLIC_INPUT
            | SYSCALL_GET_PRIVATE_INPUT
            | SYSCALL_VERIFY_SIGNATURE
            | SYSCALL_INPUT_PUBLISH_TLV
            | SYSCALL_SM3_HASH
            | SYSCALL_SM2_VERIFY
            | SYSCALL_SM4_GCM_SEAL
            | SYSCALL_SM4_GCM_OPEN
            | SYSCALL_SM4_CCM_SEAL
            | SYSCALL_SM4_CCM_OPEN
            | SYSCALL_SHA256_HASH
            | SYSCALL_SHA3_HASH
            | SYSCALL_BLAKE2B256_HASH
            | SYSCALL_KECCAK256_HASH
            | SYSCALL_IROHA_HASH
            | SYSCALL_PROVE_EXECUTION
            | SYSCALL_VERIFY_PROOF
            | SYSCALL_GET_MERKLE_PATH
            | SYSCALL_GET_MERKLE_COMPACT
            | SYSCALL_GET_REGISTER_MERKLE_COMPACT
            | SYSCALL_JSON_ENCODE
            | SYSCALL_JSON_DECODE
            | SYSCALL_TLV_LEN
            | SYSCALL_JSON_GET_JSON
            | SYSCALL_JSON_GET_NAME
            | SYSCALL_JSON_GET_ACCOUNT_ID
            | SYSCALL_JSON_GET_NFT_ID
            | SYSCALL_JSON_GET_BLOB_HEX
            | SYSCALL_JSON_GET_ASSET_DEFINITION_ID
            | SYSCALL_JSON_GET_INT
            | SYSCALL_JSON_GET_DECIMAL
            | SYSCALL_JSON_GET_QUANTITY
            | SYSCALL_JSON_OBJECT
            | SYSCALL_JSON_SET_I64
            | SYSCALL_JSON_SET_ACCOUNT_ID
            | SYSCALL_JSON_GET_JSON_DIRECT
            | SYSCALL_JSON_GET_NAME_DIRECT
            | SYSCALL_JSON_GET_ACCOUNT_ID_DIRECT
            | SYSCALL_JSON_GET_NFT_ID_DIRECT
            | SYSCALL_JSON_GET_BLOB_HEX_DIRECT
            | SYSCALL_JSON_GET_INT_DIRECT
            | SYSCALL_JSON_GET_DECIMAL_DIRECT
            | SYSCALL_JSON_GET_QUANTITY_DIRECT
            | SYSCALL_JSON_GET_ASSET_DEFINITION_ID_DIRECT
            | SYSCALL_JSON_SET_I64_DIRECT
            | SYSCALL_JSON_SET_ACCOUNT_ID_DIRECT
            | SYSCALL_BUILD_PATH_KEY_NORITO_DIRECT
            | SYSCALL_SCHEMA_INFO_DIRECT
            | SYSCALL_SCHEMA_ENCODE
            | SYSCALL_SCHEMA_DECODE
            | SYSCALL_SCHEMA_INFO
            | SYSCALL_SCHEMA_ENCODE_DIRECT
            | SYSCALL_SCHEMA_DECODE_DIRECT
            | SYSCALL_NAME_DECODE
            | SYSCALL_BUILD_PATH_MAP_KEY
            | SYSCALL_BUILD_PATH_KEY_NORITO
            | SYSCALL_ENCODE_INT
            | SYSCALL_DECODE_INT
            | SYSCALL_POINTER_TO_NORITO
            | SYSCALL_POINTER_FROM_NORITO
            | SYSCALL_TLV_EQ
            | SYSCALL_VRF_VERIFY
            | SYSCALL_VRF_VERIFY_BATCH
            | SYSCALL_GET_AUTHORITY
            | SYSCALL_CURRENT_TIME_MS
            | SYSCALL_SYSVAR_CHAIN_ID
            | SYSCALL_SYSVAR_BLOCK_HEIGHT
            | SYSCALL_SYSVAR_BLOCK_TIME_MS
            | SYSCALL_SYSVAR_AUTHORITY
            | SYSCALL_SYSVAR_CONTRACT_ADDRESS
            | SYSCALL_SYSVAR_CONTRACT_SUBJECT
            | SYSCALL_SYSVAR_ENTRYPOINT
            | SYSCALL_DECODE_ARGUMENT_RECORD
    ) {
        return Some(SyscallAccess::None);
    }
    None
}

/// Return the conservative host-state access class for an ABI v1 syscall.
///
/// The fallback is [`SyscallAccess::Dynamic`]. New or unknown syscalls
/// therefore still serialize conservatively, while
/// [`registered_syscall_access`] lets admission and host metering reject an
/// allowed-but-unclassified syscall outright.
#[must_use]
pub const fn syscall_access(number: u32) -> SyscallAccess {
    match registered_syscall_access(number) {
        Some(access) => access,
        None => SyscallAccess::Dynamic,
    }
}

/// Return the sorted syscall-number component of the ABI hash descriptor.
///
/// The complete hash also binds pointer types and the typed V1 query,
/// entrypoint, collection, and exact numeric protocol records. It binds contracts to
/// a specific host ABI. When comparing runtime ABI against a manifest-provided
/// `abi_hash`, nodes can reject execution if a mismatch is detected.
pub fn abi_syscall_list() -> &'static [u32] {
    syscalls_for_policy(crate::SyscallPolicy::AbiV1)
}

/// Return the canonical syscall list for the first-release ABI policy.
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
            SYSCALL_SHA256_HASH,
            SYSCALL_SHA3_HASH,
            SYSCALL_BLAKE2B256_HASH,
            SYSCALL_KECCAK256_HASH,
            SYSCALL_IROHA_HASH,
        ]);
        // Codec helpers
        v.push(SYSCALL_JSON_ENCODE);
        v.push(SYSCALL_JSON_DECODE);
        v.push(SYSCALL_TLV_LEN);
        v.extend_from_slice(&[
            SYSCALL_JSON_GET_JSON,
            SYSCALL_JSON_GET_NAME,
            SYSCALL_JSON_GET_ACCOUNT_ID,
            SYSCALL_JSON_GET_NFT_ID,
            SYSCALL_JSON_GET_BLOB_HEX,
            SYSCALL_JSON_GET_ASSET_DEFINITION_ID,
            SYSCALL_JSON_GET_INT,
            SYSCALL_JSON_GET_DECIMAL,
            SYSCALL_JSON_GET_QUANTITY,
            SYSCALL_JSON_OBJECT,
            SYSCALL_JSON_SET_I64,
            SYSCALL_JSON_SET_ACCOUNT_ID,
            SYSCALL_JSON_GET_JSON_DIRECT,
            SYSCALL_JSON_GET_NAME_DIRECT,
            SYSCALL_JSON_GET_ACCOUNT_ID_DIRECT,
            SYSCALL_JSON_GET_NFT_ID_DIRECT,
            SYSCALL_JSON_GET_BLOB_HEX_DIRECT,
            SYSCALL_JSON_GET_INT_DIRECT,
            SYSCALL_JSON_GET_DECIMAL_DIRECT,
            SYSCALL_JSON_GET_QUANTITY_DIRECT,
            SYSCALL_JSON_GET_ASSET_DEFINITION_ID_DIRECT,
            SYSCALL_JSON_SET_I64_DIRECT,
            SYSCALL_JSON_SET_ACCOUNT_ID_DIRECT,
            SYSCALL_BUILD_PATH_KEY_NORITO_DIRECT,
            SYSCALL_SCHEMA_INFO_DIRECT,
        ]);
        v.push(SYSCALL_SCHEMA_ENCODE);
        v.push(SYSCALL_SCHEMA_DECODE);
        v.push(SYSCALL_SCHEMA_INFO);
        // Legacy numeric aliases are intentionally absent. The first-release
        // surface exposes only the schema-bound exact numeric families below.
        v.extend_from_slice(&[SYSCALL_SCHEMA_ENCODE_DIRECT, SYSCALL_SCHEMA_DECODE_DIRECT]);
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
        v.push(SYSCALL_TRANSFER_V1);
        // NFT
        v.extend_from_slice(&[
            SYSCALL_NFT_MINT_ASSET,
            SYSCALL_NFT_TRANSFER_ASSET,
            SYSCALL_NFT_SET_METADATA,
            SYSCALL_NFT_BURN_ASSET,
        ]);
        v.push(SYSCALL_TRANSFER_V1_BATCH_BEGIN);
        v.push(SYSCALL_TRANSFER_V1_BATCH_END);
        v.push(SYSCALL_TRANSFER_V1_BATCH_APPLY);
        v.push(SYSCALL_TRANSFER_ASSET_SCOPED);
        // Durable state (smart contract)
        v.push(SYSCALL_STATE_GET);
        v.push(SYSCALL_STATE_SET);
        v.push(SYSCALL_STATE_DEL);
        v.push(SYSCALL_STATE_KEYS);
        v.push(SYSCALL_STATE_HAS);
        v.push(SYSCALL_STATE_LEN);
        v.push(SYSCALL_STATE_COUNT);
        v.push(SYSCALL_STATE_MAP_KEY_AT);
        v.push(SYSCALL_STATE_VALUE_ENCODE);
        v.push(SYSCALL_STATE_VALUE_DECODE);
        v.push(SYSCALL_BUILD_PATH_MAP_KEY);
        v.push(SYSCALL_BUILD_PATH_KEY_NORITO);
        v.push(SYSCALL_ENCODE_INT);
        v.push(SYSCALL_DECODE_INT);
        v.push(SYSCALL_POINTER_TO_NORITO);
        v.push(SYSCALL_POINTER_FROM_NORITO);
        v.push(SYSCALL_TLV_EQ);
        v.push(SYSCALL_JSON_BUILD);
        v.extend_from_slice(&[
            SYSCALL_INT_FROM_I64,
            SYSCALL_INT_FROM_U64,
            SYSCALL_INT_TRY_TO_I64,
            SYSCALL_INT_TRY_TO_U64,
            SYSCALL_INT_NEG,
            SYSCALL_INT_ADD,
            SYSCALL_INT_SUB,
            SYSCALL_INT_MUL,
            SYSCALL_INT_DIV,
            SYSCALL_INT_REM,
            SYSCALL_INT_EQ,
            SYSCALL_INT_NE,
            SYSCALL_INT_LT,
            SYSCALL_INT_LE,
            SYSCALL_INT_GT,
            SYSCALL_INT_GE,
            SYSCALL_INT_WRAP_NEG,
            SYSCALL_INT_WRAP_ADD,
            SYSCALL_INT_WRAP_SUB,
            SYSCALL_INT_WRAP_MUL,
            SYSCALL_DECIMAL_FROM_INT,
            SYSCALL_DECIMAL_NEG,
            SYSCALL_DECIMAL_ADD,
            SYSCALL_DECIMAL_SUB,
            SYSCALL_DECIMAL_MUL,
            SYSCALL_DECIMAL_DIV_EXACT,
            SYSCALL_DECIMAL_DIV_ROUND,
            SYSCALL_DECIMAL_EQ,
            SYSCALL_DECIMAL_NE,
            SYSCALL_DECIMAL_LT,
            SYSCALL_DECIMAL_LE,
            SYSCALL_DECIMAL_GT,
            SYSCALL_DECIMAL_GE,
            SYSCALL_DECIMAL_TRY_TO_INT_EXACT,
            SYSCALL_DECIMAL_TO_INT_TRUNC,
            SYSCALL_DECIMAL_TO_INT_ROUND,
            SYSCALL_QUANTITY_TRY_FROM_INT,
            SYSCALL_QUANTITY_TRY_FROM_DECIMAL,
            SYSCALL_QUANTITY_TO_DECIMAL,
            SYSCALL_QUANTITY_ADD,
            SYSCALL_QUANTITY_SUB,
            SYSCALL_QUANTITY_MUL_DECIMAL,
            SYSCALL_QUANTITY_DIV_DECIMAL_EXACT,
            SYSCALL_QUANTITY_DIV_DECIMAL_ROUND,
            SYSCALL_QUANTITY_RATIO_EXACT,
            SYSCALL_QUANTITY_RATIO_ROUND,
            SYSCALL_QUANTITY_EQ,
            SYSCALL_QUANTITY_NE,
            SYSCALL_QUANTITY_LT,
            SYSCALL_QUANTITY_LE,
            SYSCALL_QUANTITY_GT,
            SYSCALL_QUANTITY_GE,
        ]);
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
        v.push(SYSCALL_VRF_EPOCH_SEED);
        // Dev/vendor helpers
        v.extend_from_slice(&[
            SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION,
            SYSCALL_SMARTCONTRACT_EXECUTE_QUERY,
            SYSCALL_QUERY_EXECUTE_NORITO,
            SYSCALL_CORE_QUERY_GET,
            SYSCALL_CORE_QUERY_PAGE,
            SYSCALL_QUERY_GET_PARAMETER,
            SYSCALL_QUERY_GET_CONTRACT_MANIFEST,
            SYSCALL_QUERY_GET_CONTRACT_INSTANCE,
            SYSCALL_CREATE_NFTS_FOR_ALL_USERS,
            SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH,
            SYSCALL_GET_AUTHORITY,
            SYSCALL_CALL_CONTRACT,
            SYSCALL_CURRENT_TIME_MS,
            SYSCALL_SYSVAR_CHAIN_ID,
            SYSCALL_SYSVAR_BLOCK_HEIGHT,
            SYSCALL_SYSVAR_BLOCK_TIME_MS,
            SYSCALL_SYSVAR_AUTHORITY,
            SYSCALL_SYSVAR_CONTRACT_ADDRESS,
            SYSCALL_SYSVAR_CONTRACT_SUBJECT,
            SYSCALL_SYSVAR_ENTRYPOINT,
            SYSCALL_DECODE_ARGUMENT_RECORD,
            SYSCALL_SUBSCRIPTION_BILL,
            SYSCALL_SUBSCRIPTION_RECORD_USAGE,
            SYSCALL_RESOLVE_ACCOUNT_ALIAS,
            SYSCALL_SET_ASSET_TRANSFER_FREEZE,
            SYSCALL_SET_ASSET_TRANSFER_DAILY_LIMIT,
            SYSCALL_ACCOUNT_RECOVERY_PROPOSE,
            SYSCALL_ACCOUNT_RECOVERY_APPROVE,
            SYSCALL_ACCOUNT_RECOVERY_CANCEL,
            SYSCALL_ACCOUNT_RECOVERY_FINALIZE,
        ]);
        // Atomic cross-transaction (AXT) scaffolding
        v.extend_from_slice(&[
            SYSCALL_AXT_BEGIN,
            SYSCALL_AXT_TOUCH,
            SYSCALL_AXT_COMMIT,
            SYSCALL_VERIFY_DS_PROOF,
            SYSCALL_USE_ASSET_HANDLE,
        ]);
        // Native asset escrow
        v.extend_from_slice(&[
            SYSCALL_ANONYMOUS_ESCROW_OPEN_OFFER,
            SYSCALL_ANONYMOUS_ESCROW_ACCEPT,
            SYSCALL_ANONYMOUS_ESCROW_MARK_PAYMENT_SENT,
            SYSCALL_ANONYMOUS_ESCROW_RELEASE,
            SYSCALL_ANONYMOUS_ESCROW_CANCEL,
            SYSCALL_ANONYMOUS_ESCROW_OPEN_DISPUTE,
            SYSCALL_ESCROW_OPEN_OFFER,
            SYSCALL_ESCROW_ACCEPT,
            SYSCALL_ESCROW_MARK_PAYMENT_SENT,
            SYSCALL_ESCROW_RELEASE,
            SYSCALL_ESCROW_CANCEL,
            SYSCALL_ESCROW_OPEN_DISPUTE,
            SYSCALL_ESCROW_RESOLVE_DISPUTE,
            SYSCALL_ANONYMOUS_ESCROW_RESOLVE_DISPUTE,
        ]);
        // Soracloud runtime host surface
        v.extend_from_slice(&[
            SYSCALL_SORACLOUD_READ_COMMITTED_STATE,
            SYSCALL_SORACLOUD_EMIT_STATE_MUTATION,
            SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE,
            SYSCALL_SORACLOUD_APPEND_JOURNAL,
            SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT,
            SYSCALL_SORACLOUD_READ_SECRET,
            SYSCALL_SORACLOUD_READ_CREDENTIAL,
            SYSCALL_SORACLOUD_EGRESS_FETCH,
            SYSCALL_SORACLOUD_READ_CONFIG,
            SYSCALL_SORACLOUD_READ_SECRET_ENVELOPE,
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
    let crate::SyscallPolicy::AbiV1 = policy;
    v.as_slice()
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
        SYSCALL_SHA256_HASH => "SHA256_HASH",
        SYSCALL_SHA3_HASH => "SHA3_HASH",
        SYSCALL_BLAKE2B256_HASH => "BLAKE2B256_HASH",
        SYSCALL_KECCAK256_HASH => "KECCAK256_HASH",
        SYSCALL_IROHA_HASH => "IROHA_HASH",
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
        SYSCALL_TRANSFER_V1 => "TRANSFER_V1",
        SYSCALL_TRANSFER_ASSET_SCOPED => "TRANSFER_ASSET_SCOPED",
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
        SYSCALL_STATE_KEYS => "STATE_KEYS",
        SYSCALL_STATE_HAS => "STATE_HAS",
        SYSCALL_STATE_LEN => "STATE_LEN",
        SYSCALL_STATE_COUNT => "STATE_COUNT",
        SYSCALL_STATE_MAP_KEY_AT => "STATE_MAP_KEY_AT",
        SYSCALL_STATE_VALUE_ENCODE => "STATE_VALUE_ENCODE",
        SYSCALL_STATE_VALUE_DECODE => "STATE_VALUE_DECODE",
        SYSCALL_DECODE_INT => "DECODE_INT",
        SYSCALL_BUILD_PATH_MAP_KEY => "BUILD_PATH_MAP_KEY",
        SYSCALL_ENCODE_INT => "ENCODE_INT",
        SYSCALL_BUILD_PATH_KEY_NORITO => "BUILD_PATH_KEY_NORITO",
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
        SYSCALL_TLV_LEN => "TLV_LEN",
        SYSCALL_JSON_GET_JSON => "JSON_GET_JSON",
        SYSCALL_JSON_GET_NAME => "JSON_GET_NAME",
        SYSCALL_JSON_GET_ACCOUNT_ID => "JSON_GET_ACCOUNT_ID",
        SYSCALL_JSON_GET_NFT_ID => "JSON_GET_NFT_ID",
        SYSCALL_JSON_GET_BLOB_HEX => "JSON_GET_BLOB_HEX",
        SYSCALL_JSON_GET_ASSET_DEFINITION_ID => "JSON_GET_ASSET_DEFINITION_ID",
        SYSCALL_JSON_GET_INT => "JSON_GET_INT",
        SYSCALL_JSON_GET_DECIMAL => "JSON_GET_DECIMAL",
        SYSCALL_JSON_GET_QUANTITY => "JSON_GET_QUANTITY",
        SYSCALL_JSON_OBJECT => "JSON_OBJECT",
        SYSCALL_JSON_SET_I64 => "JSON_SET_I64",
        SYSCALL_JSON_SET_ACCOUNT_ID => "JSON_SET_ACCOUNT_ID",
        SYSCALL_JSON_GET_JSON_DIRECT => "JSON_GET_JSON_DIRECT",
        SYSCALL_JSON_GET_NAME_DIRECT => "JSON_GET_NAME_DIRECT",
        SYSCALL_JSON_GET_ACCOUNT_ID_DIRECT => "JSON_GET_ACCOUNT_ID_DIRECT",
        SYSCALL_JSON_GET_NFT_ID_DIRECT => "JSON_GET_NFT_ID_DIRECT",
        SYSCALL_JSON_GET_BLOB_HEX_DIRECT => "JSON_GET_BLOB_HEX_DIRECT",
        SYSCALL_JSON_GET_INT_DIRECT => "JSON_GET_INT_DIRECT",
        SYSCALL_JSON_GET_DECIMAL_DIRECT => "JSON_GET_DECIMAL_DIRECT",
        SYSCALL_JSON_GET_QUANTITY_DIRECT => "JSON_GET_QUANTITY_DIRECT",
        SYSCALL_JSON_GET_ASSET_DEFINITION_ID_DIRECT => "JSON_GET_ASSET_DEFINITION_ID_DIRECT",
        SYSCALL_JSON_SET_I64_DIRECT => "JSON_SET_I64_DIRECT",
        SYSCALL_JSON_SET_ACCOUNT_ID_DIRECT => "JSON_SET_ACCOUNT_ID_DIRECT",
        SYSCALL_BUILD_PATH_KEY_NORITO_DIRECT => "BUILD_PATH_KEY_NORITO_DIRECT",
        SYSCALL_SCHEMA_INFO_DIRECT => "SCHEMA_INFO_DIRECT",
        SYSCALL_SCHEMA_ENCODE => "SCHEMA_ENCODE",
        SYSCALL_SCHEMA_DECODE => "SCHEMA_DECODE",
        SYSCALL_SCHEMA_INFO => "SCHEMA_INFO",
        SYSCALL_SCHEMA_ENCODE_DIRECT => "SCHEMA_ENCODE_DIRECT",
        SYSCALL_SCHEMA_DECODE_DIRECT => "SCHEMA_DECODE_DIRECT",
        SYSCALL_NAME_DECODE => "NAME_DECODE",
        SYSCALL_POINTER_TO_NORITO => "POINTER_TO_NORITO",
        SYSCALL_POINTER_FROM_NORITO => "POINTER_FROM_NORITO",
        SYSCALL_TLV_EQ => "TLV_EQ",
        // VRF
        SYSCALL_VRF_VERIFY => "VRF_VERIFY",
        SYSCALL_VRF_VERIFY_BATCH => "VRF_VERIFY_BATCH",
        SYSCALL_VRF_EPOCH_SEED => "VRF_EPOCH_SEED",
        // Dev/vendor helpers
        SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION => "SMARTCONTRACT_EXECUTE_INSTRUCTION",
        SYSCALL_SMARTCONTRACT_EXECUTE_QUERY => "SMARTCONTRACT_EXECUTE_QUERY",
        SYSCALL_QUERY_EXECUTE_NORITO => "QUERY_EXECUTE_NORITO",
        SYSCALL_CORE_QUERY_GET => "CORE_QUERY_GET",
        SYSCALL_CORE_QUERY_PAGE => "CORE_QUERY_PAGE",
        SYSCALL_QUERY_GET_PARAMETER => "QUERY_GET_PARAMETER",
        SYSCALL_QUERY_GET_CONTRACT_MANIFEST => "QUERY_GET_CONTRACT_MANIFEST",
        SYSCALL_QUERY_GET_CONTRACT_INSTANCE => "QUERY_GET_CONTRACT_INSTANCE",
        SYSCALL_CREATE_NFTS_FOR_ALL_USERS => "CREATE_NFTS_FOR_ALL_USERS",
        SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH => "SET_SMARTCONTRACT_EXECUTION_DEPTH",
        SYSCALL_GET_AUTHORITY => "GET_AUTHORITY",
        SYSCALL_CALL_CONTRACT => "CALL_CONTRACT",
        SYSCALL_CURRENT_TIME_MS => "CURRENT_TIME_MS",
        SYSCALL_SYSVAR_CHAIN_ID => "SYSVAR_CHAIN_ID",
        SYSCALL_SYSVAR_BLOCK_HEIGHT => "SYSVAR_BLOCK_HEIGHT",
        SYSCALL_SYSVAR_BLOCK_TIME_MS => "SYSVAR_BLOCK_TIME_MS",
        SYSCALL_SYSVAR_AUTHORITY => "SYSVAR_AUTHORITY",
        SYSCALL_SYSVAR_CONTRACT_ADDRESS => "SYSVAR_CONTRACT_ADDRESS",
        SYSCALL_SYSVAR_CONTRACT_SUBJECT => "SYSVAR_CONTRACT_SUBJECT",
        SYSCALL_SYSVAR_ENTRYPOINT => "SYSVAR_ENTRYPOINT",
        SYSCALL_DECODE_ARGUMENT_RECORD => "DECODE_ARGUMENT_RECORD",
        SYSCALL_INT_FROM_I64 => "INT_FROM_I64",
        SYSCALL_INT_FROM_U64 => "INT_FROM_U64",
        SYSCALL_INT_TRY_TO_I64 => "INT_TRY_TO_I64",
        SYSCALL_INT_TRY_TO_U64 => "INT_TRY_TO_U64",
        SYSCALL_INT_NEG => "INT_NEG",
        SYSCALL_INT_ADD => "INT_ADD",
        SYSCALL_INT_SUB => "INT_SUB",
        SYSCALL_INT_MUL => "INT_MUL",
        SYSCALL_INT_DIV => "INT_DIV",
        SYSCALL_INT_REM => "INT_REM",
        SYSCALL_INT_EQ => "INT_EQ",
        SYSCALL_INT_NE => "INT_NE",
        SYSCALL_INT_LT => "INT_LT",
        SYSCALL_INT_LE => "INT_LE",
        SYSCALL_INT_GT => "INT_GT",
        SYSCALL_INT_GE => "INT_GE",
        SYSCALL_INT_WRAP_NEG => "INT_WRAP_NEG",
        SYSCALL_INT_WRAP_ADD => "INT_WRAP_ADD",
        SYSCALL_INT_WRAP_SUB => "INT_WRAP_SUB",
        SYSCALL_INT_WRAP_MUL => "INT_WRAP_MUL",
        SYSCALL_DECIMAL_FROM_INT => "DECIMAL_FROM_INT",
        SYSCALL_DECIMAL_NEG => "DECIMAL_NEG",
        SYSCALL_DECIMAL_ADD => "DECIMAL_ADD",
        SYSCALL_DECIMAL_SUB => "DECIMAL_SUB",
        SYSCALL_DECIMAL_MUL => "DECIMAL_MUL",
        SYSCALL_DECIMAL_DIV_EXACT => "DECIMAL_DIV_EXACT",
        SYSCALL_DECIMAL_DIV_ROUND => "DECIMAL_DIV_ROUND",
        SYSCALL_DECIMAL_EQ => "DECIMAL_EQ",
        SYSCALL_DECIMAL_NE => "DECIMAL_NE",
        SYSCALL_DECIMAL_LT => "DECIMAL_LT",
        SYSCALL_DECIMAL_LE => "DECIMAL_LE",
        SYSCALL_DECIMAL_GT => "DECIMAL_GT",
        SYSCALL_DECIMAL_GE => "DECIMAL_GE",
        SYSCALL_DECIMAL_TRY_TO_INT_EXACT => "DECIMAL_TRY_TO_INT_EXACT",
        SYSCALL_DECIMAL_TO_INT_TRUNC => "DECIMAL_TO_INT_TRUNC",
        SYSCALL_DECIMAL_TO_INT_ROUND => "DECIMAL_TO_INT_ROUND",
        SYSCALL_QUANTITY_TRY_FROM_INT => "QUANTITY_TRY_FROM_INT",
        SYSCALL_QUANTITY_TRY_FROM_DECIMAL => "QUANTITY_TRY_FROM_DECIMAL",
        SYSCALL_QUANTITY_TO_DECIMAL => "QUANTITY_TO_DECIMAL",
        SYSCALL_QUANTITY_ADD => "QUANTITY_ADD",
        SYSCALL_QUANTITY_SUB => "QUANTITY_SUB",
        SYSCALL_QUANTITY_MUL_DECIMAL => "QUANTITY_MUL_DECIMAL",
        SYSCALL_QUANTITY_DIV_DECIMAL_EXACT => "QUANTITY_DIV_DECIMAL_EXACT",
        SYSCALL_QUANTITY_DIV_DECIMAL_ROUND => "QUANTITY_DIV_DECIMAL_ROUND",
        SYSCALL_QUANTITY_RATIO_EXACT => "QUANTITY_RATIO_EXACT",
        SYSCALL_QUANTITY_RATIO_ROUND => "QUANTITY_RATIO_ROUND",
        SYSCALL_QUANTITY_EQ => "QUANTITY_EQ",
        SYSCALL_QUANTITY_NE => "QUANTITY_NE",
        SYSCALL_QUANTITY_LT => "QUANTITY_LT",
        SYSCALL_QUANTITY_LE => "QUANTITY_LE",
        SYSCALL_QUANTITY_GT => "QUANTITY_GT",
        SYSCALL_QUANTITY_GE => "QUANTITY_GE",
        SYSCALL_JSON_BUILD => "JSON_BUILD",
        SYSCALL_SUBSCRIPTION_BILL => "SUBSCRIPTION_BILL",
        SYSCALL_SUBSCRIPTION_RECORD_USAGE => "SUBSCRIPTION_RECORD_USAGE",
        SYSCALL_RESOLVE_ACCOUNT_ALIAS => "RESOLVE_ACCOUNT_ALIAS",
        SYSCALL_SET_ASSET_TRANSFER_FREEZE => "SET_ASSET_TRANSFER_FREEZE",
        SYSCALL_SET_ASSET_TRANSFER_DAILY_LIMIT => "SET_ASSET_TRANSFER_DAILY_LIMIT",
        SYSCALL_ACCOUNT_RECOVERY_PROPOSE => "ACCOUNT_RECOVERY_PROPOSE",
        SYSCALL_ACCOUNT_RECOVERY_APPROVE => "ACCOUNT_RECOVERY_APPROVE",
        SYSCALL_ACCOUNT_RECOVERY_CANCEL => "ACCOUNT_RECOVERY_CANCEL",
        SYSCALL_ACCOUNT_RECOVERY_FINALIZE => "ACCOUNT_RECOVERY_FINALIZE",
        SYSCALL_ANONYMOUS_ESCROW_OPEN_OFFER => "ANONYMOUS_ESCROW_OPEN_OFFER",
        SYSCALL_ANONYMOUS_ESCROW_ACCEPT => "ANONYMOUS_ESCROW_ACCEPT",
        SYSCALL_ANONYMOUS_ESCROW_MARK_PAYMENT_SENT => "ANONYMOUS_ESCROW_MARK_PAYMENT_SENT",
        SYSCALL_ANONYMOUS_ESCROW_RELEASE => "ANONYMOUS_ESCROW_RELEASE",
        SYSCALL_ANONYMOUS_ESCROW_CANCEL => "ANONYMOUS_ESCROW_CANCEL",
        SYSCALL_ANONYMOUS_ESCROW_OPEN_DISPUTE => "ANONYMOUS_ESCROW_OPEN_DISPUTE",
        SYSCALL_AXT_BEGIN => "AXT_BEGIN",
        SYSCALL_AXT_TOUCH => "AXT_TOUCH",
        SYSCALL_AXT_COMMIT => "AXT_COMMIT",
        SYSCALL_VERIFY_DS_PROOF => "VERIFY_DS_PROOF",
        SYSCALL_USE_ASSET_HANDLE => "USE_ASSET_HANDLE",
        SYSCALL_ESCROW_OPEN_OFFER => "ESCROW_OPEN_OFFER",
        SYSCALL_ESCROW_ACCEPT => "ESCROW_ACCEPT",
        SYSCALL_ESCROW_MARK_PAYMENT_SENT => "ESCROW_MARK_PAYMENT_SENT",
        SYSCALL_ESCROW_RELEASE => "ESCROW_RELEASE",
        SYSCALL_ESCROW_CANCEL => "ESCROW_CANCEL",
        SYSCALL_ESCROW_OPEN_DISPUTE => "ESCROW_OPEN_DISPUTE",
        SYSCALL_ESCROW_RESOLVE_DISPUTE => "ESCROW_RESOLVE_DISPUTE",
        SYSCALL_ANONYMOUS_ESCROW_RESOLVE_DISPUTE => "ANONYMOUS_ESCROW_RESOLVE_DISPUTE",
        SYSCALL_SORACLOUD_READ_COMMITTED_STATE => "SORACLOUD_READ_COMMITTED_STATE",
        SYSCALL_SORACLOUD_EMIT_STATE_MUTATION => "SORACLOUD_EMIT_STATE_MUTATION",
        SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE => "SORACLOUD_EMIT_MAILBOX_MESSAGE",
        SYSCALL_SORACLOUD_APPEND_JOURNAL => "SORACLOUD_APPEND_JOURNAL",
        SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT => "SORACLOUD_PUBLISH_CHECKPOINT",
        SYSCALL_SORACLOUD_READ_SECRET => "SORACLOUD_READ_SECRET",
        SYSCALL_SORACLOUD_READ_CREDENTIAL => "SORACLOUD_READ_CREDENTIAL",
        SYSCALL_SORACLOUD_EGRESS_FETCH => "SORACLOUD_EGRESS_FETCH",
        SYSCALL_SORACLOUD_READ_CONFIG => "SORACLOUD_READ_CONFIG",
        SYSCALL_SORACLOUD_READ_SECRET_ENVELOPE => "SORACLOUD_READ_SECRET_ENVELOPE",
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

// Use the generated gas asset table if present (preferred). This file is
// produced by the `gen_syscalls_doc` helper. It must define `GasAsset` and
// `pub static GAS_ASSETS: &[GasAsset]`.
#[path = "gas_spec.rs"]
pub mod gas_spec;

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

/// Re-export generated gas assets for tests/docs.
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

const ABI_V1_SURFACE_DOMAIN: &[u8] = b"IVM_ABI_V1_FULL_SURFACE\0";
const ABI_SURFACE_DESCRIPTOR_FORMAT_VERSION: u16 = 1;
const NUMERIC_MANTISSA_BITS_V1: u16 = 512;
const DECIMAL_MAX_SCALE_V1: u8 = 28;

// This is the base for invalid-surface sentinels. They cannot be emitted by
// `iroha_crypto::Hash::new`: every valid Iroha hash has the low bit of its final
// byte set. Returning an unmistakable sentinel is safer than silently hashing
// an incomplete generated registry, while release tests require the compiled
// surface to validate successfully.
const INVALID_ABI_SURFACE_HASH: [u8; 32] = [0; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AbiSyscallSurface {
    number: u32,
    args: &'static str,
    ret: &'static str,
    access: SyscallAccess,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AbiNamedTypeSurface {
    name: &'static str,
    ty: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AbiCoreQueryProjectionSurface {
    name: &'static str,
    entity_tag: u64,
    fields: Vec<AbiNamedTypeSurface>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AbiQueryPageSurface {
    name: &'static str,
    fields: Vec<AbiNamedTypeSurface>,
    items_capacity: u8,
    next_offset_semantics: &'static str,
    item_ordering: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AbiEntrypointSurface {
    schema_version: u8,
    int_kind: &'static str,
    int_pointer_type_id: u16,
    decimal_kind: &'static str,
    decimal_pointer_type_id: u16,
    quantity_kind: &'static str,
    quantity_pointer_type_id: u16,
    list_kind: &'static str,
    list_layout: &'static str,
    list_child_count: u8,
    list_capacity_is_schema_bound: bool,
    list_min_capacity: u8,
    list_max_capacity: u8,
    max_schema_nodes: u64,
    max_schema_depth: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AbiNumericRoundingSurface {
    name: &'static str,
    tag: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AbiNumericFaultSurface {
    name: &'static str,
    tag: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AbiNumericSurface {
    int_pointer_type_id: u16,
    decimal_pointer_type_id: u16,
    quantity_pointer_type_id: u16,
    mantissa_bits: u16,
    max_scale: u8,
    int_domain: &'static str,
    decimal_domain: &'static str,
    quantity_domain: &'static str,
    canonicalization: &'static str,
    integer_division: &'static str,
    wrapping_modulus: &'static str,
    rounding_modes: Vec<AbiNumericRoundingSurface>,
    faults: Vec<AbiNumericFaultSurface>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AbiIndexedLiteralSurface {
    opcode: u8,
    mnemonic: &'static str,
    table_kind: u8,
    payload_layout: &'static str,
    result: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AbiDurableStateSurface {
    semantics_version: u8,
    keys_max_items: u64,
    max_path_bytes: u64,
    max_value_bytes: u64,
    map_max_key_bytes: u64,
    map_max_base_bytes: u64,
    map_max_page_bytes: u64,
    path_size_unit: &'static str,
    value_storage: &'static str,
    ordering_version: u8,
    key_ordering: &'static str,
    prefix_match: &'static str,
    map_path_derivation_version: u8,
    map_path_derivation: &'static str,
    page_overflow: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AbiSurface {
    descriptor_format_version: u16,
    policy_tag: u8,
    syscalls: Vec<AbiSyscallSurface>,
    pointer_type_ids: Vec<u16>,
    core_query_projections: Vec<AbiCoreQueryProjectionSurface>,
    query_page: AbiQueryPageSurface,
    entrypoint: AbiEntrypointSurface,
    numeric: AbiNumericSurface,
    indexed_literals: Vec<AbiIndexedLiteralSurface>,
    durable_state: AbiDurableStateSurface,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AbiSurfaceError {
    EmptySyscallSurface,
    EmptyPointerSurface,
    SyscallsNotStrictlySorted { previous: u32, current: u32 },
    PointerTypesNotStrictlySorted { previous: u16, current: u16 },
    MissingSignature(u32),
    DuplicateSignature(u32),
    UnexpectedSignature(u32),
    EmptyArguments(u32),
    EmptyReturn(u32),
    SurfaceTooLarge,
}

fn invalid_abi_surface_hash(error: AbiSurfaceError) -> [u8; 32] {
    let (tag, first, second) = match error {
        AbiSurfaceError::EmptySyscallSurface => (1, 0, 0),
        AbiSurfaceError::EmptyPointerSurface => (2, 0, 0),
        AbiSurfaceError::SyscallsNotStrictlySorted { previous, current } => (3, previous, current),
        AbiSurfaceError::PointerTypesNotStrictlySorted { previous, current } => {
            (4, u32::from(previous), u32::from(current))
        }
        AbiSurfaceError::MissingSignature(number) => (5, number, 0),
        AbiSurfaceError::DuplicateSignature(number) => (6, number, 0),
        AbiSurfaceError::UnexpectedSignature(number) => (7, number, 0),
        AbiSurfaceError::EmptyArguments(number) => (8, number, 0),
        AbiSurfaceError::EmptyReturn(number) => (9, number, 0),
        AbiSurfaceError::SurfaceTooLarge => (10, 0, 0),
    };
    let mut sentinel = INVALID_ABI_SURFACE_HASH;
    sentinel[0] = tag;
    sentinel[1..5].copy_from_slice(&first.to_le_bytes());
    sentinel[5..9].copy_from_slice(&second.to_le_bytes());
    sentinel
}

const fn syscall_access_tag(access: SyscallAccess) -> u8 {
    match access {
        SyscallAccess::None => 0,
        SyscallAccess::StateRead => 1,
        SyscallAccess::StateWrite => 2,
        SyscallAccess::LedgerRead => 3,
        SyscallAccess::LedgerWrite => 4,
        SyscallAccess::Dynamic => 5,
    }
}

fn append_abi_field(bytes: &mut Vec<u8>, value: &[u8]) -> Result<(), AbiSurfaceError> {
    let len = u64::try_from(value.len()).map_err(|_| AbiSurfaceError::SurfaceTooLarge)?;
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(value);
    Ok(())
}

/// Canonical encoder for the ABI hash descriptor.
///
/// Every value is paired with a stable field name and both byte strings are
/// length-prefixed. Nested records and sequence items are themselves framed
/// fields, so no two different partitions of the same byte stream can alias.
/// Inputs are explicit integers, booleans, and UTF-8 protocol names; encoding
/// never depends on debug formatting, a codec configuration, or host hardware.
#[derive(Default)]
struct AbiDescriptorEncoder {
    bytes: Vec<u8>,
}

impl AbiDescriptorEncoder {
    fn field(&mut self, name: &str, value: &[u8]) -> Result<(), AbiSurfaceError> {
        append_abi_field(&mut self.bytes, name.as_bytes())?;
        append_abi_field(&mut self.bytes, value)
    }

    fn text(&mut self, name: &str, value: &str) -> Result<(), AbiSurfaceError> {
        self.field(name, value.as_bytes())
    }

    fn bool(&mut self, name: &str, value: bool) -> Result<(), AbiSurfaceError> {
        self.field(name, &[u8::from(value)])
    }

    fn u8(&mut self, name: &str, value: u8) -> Result<(), AbiSurfaceError> {
        self.field(name, &[value])
    }

    fn u16(&mut self, name: &str, value: u16) -> Result<(), AbiSurfaceError> {
        self.field(name, &value.to_le_bytes())
    }

    fn u32(&mut self, name: &str, value: u32) -> Result<(), AbiSurfaceError> {
        self.field(name, &value.to_le_bytes())
    }

    fn u64(&mut self, name: &str, value: u64) -> Result<(), AbiSurfaceError> {
        self.field(name, &value.to_le_bytes())
    }

    fn record(
        &mut self,
        name: &str,
        encode: impl FnOnce(&mut Self) -> Result<(), AbiSurfaceError>,
    ) -> Result<(), AbiSurfaceError> {
        let mut record = Self::default();
        encode(&mut record)?;
        self.field(name, &record.bytes)
    }

    fn sequence<T>(
        &mut self,
        name: &str,
        items: &[T],
        mut encode_item: impl FnMut(&mut Self, &T) -> Result<(), AbiSurfaceError>,
    ) -> Result<(), AbiSurfaceError> {
        self.record(name, |sequence| {
            let count = u64::try_from(items.len()).map_err(|_| AbiSurfaceError::SurfaceTooLarge)?;
            sequence.u64("count", count)?;
            for item in items {
                sequence.record("item", |record| encode_item(record, item))?;
            }
            Ok(())
        })
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

fn core_query_projection_surface_v1() -> Vec<AbiCoreQueryProjectionSurface> {
    use crate::core_query::CoreQueryEntityTagV1 as Tag;

    vec![
        AbiCoreQueryProjectionSurface {
            name: "AccountView",
            entity_tag: Tag::Account.as_u64(),
            fields: vec![
                AbiNamedTypeSurface {
                    name: "id",
                    ty: "AccountId",
                },
                AbiNamedTypeSurface {
                    name: "metadata",
                    ty: "Json",
                },
            ],
        },
        AbiCoreQueryProjectionSurface {
            name: "AssetView",
            entity_tag: Tag::Asset.as_u64(),
            fields: vec![
                AbiNamedTypeSurface {
                    name: "id",
                    ty: "AssetId",
                },
                AbiNamedTypeSurface {
                    name: "amount",
                    ty: "Quantity",
                },
            ],
        },
        AbiCoreQueryProjectionSurface {
            name: "AssetDefinitionView",
            entity_tag: Tag::AssetDefinition.as_u64(),
            fields: vec![
                AbiNamedTypeSurface {
                    name: "id",
                    ty: "AssetDefinitionId",
                },
                AbiNamedTypeSurface {
                    name: "name",
                    ty: "String",
                },
                AbiNamedTypeSurface {
                    name: "description",
                    ty: "Option<String>",
                },
                AbiNamedTypeSurface {
                    name: "owned_by",
                    ty: "AccountId",
                },
                AbiNamedTypeSurface {
                    name: "total_quantity",
                    ty: "Quantity",
                },
                AbiNamedTypeSurface {
                    name: "metadata",
                    ty: "Json",
                },
            ],
        },
        AbiCoreQueryProjectionSurface {
            name: "DomainView",
            entity_tag: Tag::Domain.as_u64(),
            fields: vec![
                AbiNamedTypeSurface {
                    name: "id",
                    ty: "DomainId",
                },
                AbiNamedTypeSurface {
                    name: "owned_by",
                    ty: "AccountId",
                },
                AbiNamedTypeSurface {
                    name: "metadata",
                    ty: "Json",
                },
            ],
        },
        AbiCoreQueryProjectionSurface {
            name: "NftView",
            entity_tag: Tag::Nft.as_u64(),
            fields: vec![
                AbiNamedTypeSurface {
                    name: "id",
                    ty: "NftId",
                },
                AbiNamedTypeSurface {
                    name: "owned_by",
                    ty: "AccountId",
                },
                AbiNamedTypeSurface {
                    name: "content",
                    ty: "Json",
                },
            ],
        },
    ]
}

fn semantic_abi_surface_v1() -> Result<
    (
        Vec<AbiCoreQueryProjectionSurface>,
        AbiQueryPageSurface,
        AbiEntrypointSurface,
        AbiNumericSurface,
    ),
    AbiSurfaceError,
> {
    use crate::{
        core_query::QUERY_PAGE_CAPACITY_V1,
        entrypoint::{
            MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH, MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES,
            MAX_ENTRYPOINT_LIST_CAPACITY_V1, MIN_ENTRYPOINT_LIST_CAPACITY_V1,
        },
        pointer_abi::PointerType,
    };

    let query_page_capacity =
        u8::try_from(QUERY_PAGE_CAPACITY_V1).map_err(|_| AbiSurfaceError::SurfaceTooLarge)?;
    let max_schema_nodes = u64::try_from(MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES)
        .map_err(|_| AbiSurfaceError::SurfaceTooLarge)?;
    let max_schema_depth = u64::try_from(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH)
        .map_err(|_| AbiSurfaceError::SurfaceTooLarge)?;
    let int_pointer_type_id = PointerType::Int as u16;
    let decimal_pointer_type_id = PointerType::Decimal as u16;
    let quantity_pointer_type_id = PointerType::Quantity as u16;

    Ok((
        core_query_projection_surface_v1(),
        AbiQueryPageSurface {
            name: "QueryPage",
            fields: vec![
                AbiNamedTypeSurface {
                    name: "items",
                    ty: "List<T,64>",
                },
                AbiNamedTypeSurface {
                    name: "next_offset",
                    ty: "Option<i64>",
                },
            ],
            items_capacity: query_page_capacity,
            next_offset_semantics: "present-iff-another-canonical-page-exists;some-requires-nonempty-items;nonnegative;not-less-than-item-count;from-window=offset+item-count-with-checked-i64",
            item_ordering: "canonical-entity-id-ascending",
        },
        AbiEntrypointSurface {
            schema_version: 1,
            int_kind: "Int",
            int_pointer_type_id,
            decimal_kind: "Decimal",
            decimal_pointer_type_id,
            quantity_kind: "Quantity",
            quantity_pointer_type_id,
            list_kind: "List",
            list_layout: "flat-preorder;exact-element-subtree-immediately-follows",
            list_child_count: 1,
            list_capacity_is_schema_bound: true,
            list_min_capacity: MIN_ENTRYPOINT_LIST_CAPACITY_V1,
            list_max_capacity: MAX_ENTRYPOINT_LIST_CAPACITY_V1,
            max_schema_nodes,
            max_schema_depth,
        },
        AbiNumericSurface {
            int_pointer_type_id,
            decimal_pointer_type_id,
            quantity_pointer_type_id,
            mantissa_bits: NUMERIC_MANTISSA_BITS_V1,
            max_scale: DECIMAL_MAX_SCALE_V1,
            int_domain: "-2^511..=2^511-1",
            decimal_domain: "signed-mantissa-times-10^-scale;scale=0..28;exact",
            quantity_domain: "nonnegative-decimal;nominal-ledger-quantity",
            canonicalization: "minimal-signed-little-endian;zero-empty;strip-fractional-trailing-zeroes;zero-scale-is-zero",
            integer_division: "quotient-truncates-toward-zero;remainder-sign-is-dividend",
            wrapping_modulus: "2^512;reinterpret-as-signed-domain",
            rounding_modes: vec![
                AbiNumericRoundingSurface {
                    name: "toward_zero",
                    tag: crate::numeric::RoundingModeV1::TowardZero.tag(),
                },
                AbiNumericRoundingSurface {
                    name: "away_from_zero",
                    tag: crate::numeric::RoundingModeV1::AwayFromZero.tag(),
                },
                AbiNumericRoundingSurface {
                    name: "floor",
                    tag: crate::numeric::RoundingModeV1::Floor.tag(),
                },
                AbiNumericRoundingSurface {
                    name: "ceil",
                    tag: crate::numeric::RoundingModeV1::Ceil.tag(),
                },
                AbiNumericRoundingSurface {
                    name: "nearest_even",
                    tag: crate::numeric::RoundingModeV1::NearestEven.tag(),
                },
                AbiNumericRoundingSurface {
                    name: "nearest_away",
                    tag: crate::numeric::RoundingModeV1::NearestAway.tag(),
                },
                AbiNumericRoundingSurface {
                    name: "nearest_toward_zero",
                    tag: crate::numeric::RoundingModeV1::NearestTowardZero.tag(),
                },
            ],
            faults: vec![
                AbiNumericFaultSurface {
                    name: "mantissa_overflow",
                    tag: crate::numeric::NumericFaultV1::MantissaOverflow.tag(),
                },
                AbiNumericFaultSurface {
                    name: "scale_overflow",
                    tag: crate::numeric::NumericFaultV1::ScaleOverflow.tag(),
                },
                AbiNumericFaultSurface {
                    name: "division_by_zero",
                    tag: crate::numeric::NumericFaultV1::DivisionByZero.tag(),
                },
                AbiNumericFaultSurface {
                    name: "repeating_decimal",
                    tag: crate::numeric::NumericFaultV1::RepeatingDecimal.tag(),
                },
                AbiNumericFaultSurface {
                    name: "exact_division_scale_overflow",
                    tag: crate::numeric::NumericFaultV1::ExactDivisionScaleOverflow.tag(),
                },
                AbiNumericFaultSurface {
                    name: "invalid_scale",
                    tag: crate::numeric::NumericFaultV1::InvalidScale.tag(),
                },
                AbiNumericFaultSurface {
                    name: "inexact_conversion",
                    tag: crate::numeric::NumericFaultV1::InexactConversion.tag(),
                },
                AbiNumericFaultSurface {
                    name: "negative_quantity",
                    tag: crate::numeric::NumericFaultV1::NegativeQuantity.tag(),
                },
                AbiNumericFaultSurface {
                    name: "quantity_underflow",
                    tag: crate::numeric::NumericFaultV1::QuantityUnderflow.tag(),
                },
                AbiNumericFaultSurface {
                    name: "invalid_rounding_mode",
                    tag: crate::numeric::NumericFaultV1::InvalidRoundingMode.tag(),
                },
                AbiNumericFaultSurface {
                    name: "invalid_failure_mode",
                    tag: crate::numeric::NumericFaultV1::InvalidFailureMode.tag(),
                },
                AbiNumericFaultSurface {
                    name: "reserved_register_nonzero",
                    tag: crate::numeric::NumericFaultV1::ReservedRegisterNonZero.tag(),
                },
            ],
        },
    ))
}

fn collect_abi_syscall_surface(
    syscalls: &[u32],
    docs: &[SyscallDoc],
) -> Result<Vec<AbiSyscallSurface>, AbiSurfaceError> {
    if syscalls.is_empty() {
        return Err(AbiSurfaceError::EmptySyscallSurface);
    }
    if let Some(pair) = syscalls.windows(2).find(|pair| pair[0] >= pair[1]) {
        return Err(AbiSurfaceError::SyscallsNotStrictlySorted {
            previous: pair[0],
            current: pair[1],
        });
    }
    for doc in docs {
        if syscalls.binary_search(&doc.number).is_err() {
            return Err(AbiSurfaceError::UnexpectedSignature(doc.number));
        }
    }

    let mut surface = Vec::with_capacity(syscalls.len());
    for &number in syscalls {
        let mut rows = docs.iter().filter(|doc| doc.number == number);
        let doc = rows
            .next()
            .ok_or(AbiSurfaceError::MissingSignature(number))?;
        if rows.next().is_some() {
            return Err(AbiSurfaceError::DuplicateSignature(number));
        }
        if doc.args.is_empty() {
            return Err(AbiSurfaceError::EmptyArguments(number));
        }
        if doc.ret.is_empty() {
            return Err(AbiSurfaceError::EmptyReturn(number));
        }
        surface.push(AbiSyscallSurface {
            number,
            args: doc.args,
            ret: doc.ret,
            access: syscall_access(number),
        });
    }
    Ok(surface)
}

fn encode_abi_surface(surface: &AbiSurface) -> Result<Vec<u8>, AbiSurfaceError> {
    if surface.syscalls.is_empty() {
        return Err(AbiSurfaceError::EmptySyscallSurface);
    }
    if surface.pointer_type_ids.is_empty() {
        return Err(AbiSurfaceError::EmptyPointerSurface);
    }
    if let Some(pair) = surface
        .syscalls
        .windows(2)
        .find(|pair| pair[0].number >= pair[1].number)
    {
        return Err(AbiSurfaceError::SyscallsNotStrictlySorted {
            previous: pair[0].number,
            current: pair[1].number,
        });
    }
    if let Some(pair) = surface
        .pointer_type_ids
        .windows(2)
        .find(|pair| pair[0] >= pair[1])
    {
        return Err(AbiSurfaceError::PointerTypesNotStrictlySorted {
            previous: pair[0],
            current: pair[1],
        });
    }

    for syscall in &surface.syscalls {
        if syscall.args.is_empty() {
            return Err(AbiSurfaceError::EmptyArguments(syscall.number));
        }
        if syscall.ret.is_empty() {
            return Err(AbiSurfaceError::EmptyReturn(syscall.number));
        }
    }

    let mut descriptor = AbiDescriptorEncoder::default();
    descriptor.field("domain", ABI_V1_SURFACE_DOMAIN)?;
    descriptor.u16(
        "descriptor_format_version",
        surface.descriptor_format_version,
    )?;
    descriptor.u8("policy_tag", surface.policy_tag)?;
    descriptor.sequence(
        "indexed_literals",
        &surface.indexed_literals,
        |literal, instruction| {
            literal.u8("opcode", instruction.opcode)?;
            literal.text("mnemonic", instruction.mnemonic)?;
            literal.u8("table_kind", instruction.table_kind)?;
            literal.text("payload_layout", instruction.payload_layout)?;
            literal.text("result", instruction.result)
        },
    )?;
    descriptor.sequence("syscalls", &surface.syscalls, |record, syscall| {
        record.u32("number", syscall.number)?;
        record.text("arguments", syscall.args)?;
        record.text("return", syscall.ret)?;
        record.u8("access", syscall_access_tag(syscall.access))
    })?;
    descriptor.sequence(
        "pointer_type_ids",
        &surface.pointer_type_ids,
        |record, type_id| record.u16("type_id", *type_id),
    )?;
    descriptor.record("core_query", |core_query| {
        core_query.text("singular_result", "Option<View>")?;
        core_query.sequence(
            "projections",
            &surface.core_query_projections,
            |projection_record, projection| {
                projection_record.text("name", projection.name)?;
                projection_record.u64("entity_tag", projection.entity_tag)?;
                projection_record.sequence("fields", &projection.fields, |field_record, field| {
                    field_record.text("name", field.name)?;
                    field_record.text("type", field.ty)
                })
            },
        )?;
        core_query.record("page", |page| {
            page.text("name", surface.query_page.name)?;
            page.sequence(
                "fields",
                &surface.query_page.fields,
                |field_record, field| {
                    field_record.text("name", field.name)?;
                    field_record.text("type", field.ty)
                },
            )?;
            page.u8("items_capacity", surface.query_page.items_capacity)?;
            page.text(
                "next_offset_semantics",
                surface.query_page.next_offset_semantics,
            )?;
            page.text("item_ordering", surface.query_page.item_ordering)
        })
    })?;
    descriptor.record("durable_state", |state| {
        state.u8("semantics_version", surface.durable_state.semantics_version)?;
        state.u64("keys_max_items", surface.durable_state.keys_max_items)?;
        state.u64("max_path_bytes", surface.durable_state.max_path_bytes)?;
        state.u64("max_value_bytes", surface.durable_state.max_value_bytes)?;
        state.u64("map_max_key_bytes", surface.durable_state.map_max_key_bytes)?;
        state.u64(
            "map_max_base_bytes",
            surface.durable_state.map_max_base_bytes,
        )?;
        state.u64(
            "map_max_page_bytes",
            surface.durable_state.map_max_page_bytes,
        )?;
        state.text("path_size_unit", surface.durable_state.path_size_unit)?;
        state.text("value_storage", surface.durable_state.value_storage)?;
        state.u8("ordering_version", surface.durable_state.ordering_version)?;
        state.text("key_ordering", surface.durable_state.key_ordering)?;
        state.text("prefix_match", surface.durable_state.prefix_match)?;
        state.u8(
            "map_path_derivation_version",
            surface.durable_state.map_path_derivation_version,
        )?;
        state.text(
            "map_path_derivation",
            surface.durable_state.map_path_derivation,
        )?;
        state.text("page_overflow", surface.durable_state.page_overflow)
    })?;
    descriptor.record("entrypoint", |entrypoint| {
        entrypoint.u8("schema_version", surface.entrypoint.schema_version)?;
        entrypoint.text("int_kind", surface.entrypoint.int_kind)?;
        entrypoint.u16(
            "int_pointer_type_id",
            surface.entrypoint.int_pointer_type_id,
        )?;
        entrypoint.text("decimal_kind", surface.entrypoint.decimal_kind)?;
        entrypoint.u16(
            "decimal_pointer_type_id",
            surface.entrypoint.decimal_pointer_type_id,
        )?;
        entrypoint.text("quantity_kind", surface.entrypoint.quantity_kind)?;
        entrypoint.u16(
            "quantity_pointer_type_id",
            surface.entrypoint.quantity_pointer_type_id,
        )?;
        entrypoint.text("list_kind", surface.entrypoint.list_kind)?;
        entrypoint.text("list_layout", surface.entrypoint.list_layout)?;
        entrypoint.u8("list_child_count", surface.entrypoint.list_child_count)?;
        entrypoint.bool(
            "list_capacity_is_schema_bound",
            surface.entrypoint.list_capacity_is_schema_bound,
        )?;
        entrypoint.u8("list_min_capacity", surface.entrypoint.list_min_capacity)?;
        entrypoint.u8("list_max_capacity", surface.entrypoint.list_max_capacity)?;
        entrypoint.u64("max_schema_nodes", surface.entrypoint.max_schema_nodes)?;
        entrypoint.u64("max_schema_depth", surface.entrypoint.max_schema_depth)
    })?;
    descriptor.record("numeric", |numeric| {
        numeric.u16("int_pointer_type_id", surface.numeric.int_pointer_type_id)?;
        numeric.u16(
            "decimal_pointer_type_id",
            surface.numeric.decimal_pointer_type_id,
        )?;
        numeric.u16(
            "quantity_pointer_type_id",
            surface.numeric.quantity_pointer_type_id,
        )?;
        numeric.u16("mantissa_bits", surface.numeric.mantissa_bits)?;
        numeric.u8("max_scale", surface.numeric.max_scale)?;
        numeric.text("int_domain", surface.numeric.int_domain)?;
        numeric.text("decimal_domain", surface.numeric.decimal_domain)?;
        numeric.text("quantity_domain", surface.numeric.quantity_domain)?;
        numeric.text("canonicalization", surface.numeric.canonicalization)?;
        numeric.text("integer_division", surface.numeric.integer_division)?;
        numeric.text("wrapping_modulus", surface.numeric.wrapping_modulus)?;
        numeric.sequence(
            "rounding_modes",
            &surface.numeric.rounding_modes,
            |rounding, mode| {
                rounding.text("name", mode.name)?;
                rounding.u64("tag", mode.tag)
            },
        )?;
        numeric.sequence("faults", &surface.numeric.faults, |fault, value| {
            fault.text("name", value.name)?;
            fault.u64("tag", value.tag)
        })
    })?;
    Ok(descriptor.finish())
}

fn collect_abi_surface(policy: crate::SyscallPolicy) -> Result<AbiSurface, AbiSurfaceError> {
    let crate::SyscallPolicy::AbiV1 = policy;
    let syscalls =
        collect_abi_syscall_surface(syscalls_for_policy(policy), syscalls_doc_gen::DOCS)?;
    let mut pointer_type_ids = crate::pointer_abi::policy_pointer_types(policy)
        .iter()
        .map(|pointer_type| *pointer_type as u16)
        .collect::<Vec<_>>();
    pointer_type_ids.sort_unstable();
    let (core_query_projections, query_page, entrypoint, numeric) = semantic_abi_surface_v1()?;
    let indexed_literals = vec![
        AbiIndexedLiteralSurface {
            opcode: crate::instruction::wide::memory::LDLIT,
            mnemonic: "LDLIT",
            table_kind: crate::metadata::LiteralKindV1::PointerTlv as u8,
            payload_layout: "exact ABI-v1 pointer TLV envelope",
            result: "validated code-memory pointer",
        },
        AbiIndexedLiteralSurface {
            opcode: crate::instruction::wide::memory::LDI64,
            mnemonic: "LDI64",
            table_kind: crate::metadata::LiteralKindV1::I64 as u8,
            payload_layout: "exact 8-byte little-endian signed i64",
            result: "sign-preserving 64-bit register value",
        },
    ];
    let durable_state = AbiDurableStateSurface {
        semantics_version: 1,
        keys_max_items: STATE_KEYS_MAX_ITEMS,
        max_path_bytes: u64::try_from(STATE_MAX_PATH_BYTES)
            .map_err(|_| AbiSurfaceError::SurfaceTooLarge)?,
        max_value_bytes: u64::try_from(STATE_MAX_VALUE_BYTES)
            .map_err(|_| AbiSurfaceError::SurfaceTooLarge)?,
        map_max_key_bytes: u64::try_from(STATE_MAP_MAX_KEY_BYTES)
            .map_err(|_| AbiSurfaceError::SurfaceTooLarge)?,
        map_max_base_bytes: u64::try_from(STATE_MAP_MAX_BASE_BYTES)
            .map_err(|_| AbiSurfaceError::SurfaceTooLarge)?,
        map_max_page_bytes: u64::try_from(STATE_MAP_MAX_PAGE_BYTES)
            .map_err(|_| AbiSurfaceError::SurfaceTooLarge)?,
        path_size_unit: "framed canonical Norito Name payload bytes",
        value_storage: "raw NoritoBytes payload; pointer boundary wraps exactly once",
        ordering_version: 1,
        key_ordering: "canonical Name order; StateMap hex paths preserve canonical Norito key byte order",
        prefix_match: "key equals prefix or remaining suffix begins with slash",
        map_path_derivation_version: 1,
        map_path_derivation: "base + slash + lowercase_hex(canonical_norito_key_payload)",
        page_overflow: "reject before selected-page materialization",
    };
    Ok(AbiSurface {
        descriptor_format_version: ABI_SURFACE_DESCRIPTOR_FORMAT_VERSION,
        policy_tag: 1,
        syscalls,
        pointer_type_ids,
        core_query_projections,
        query_page,
        entrypoint,
        numeric,
        indexed_literals,
        durable_state,
    })
}

fn build_abi_surface_descriptor(policy: crate::SyscallPolicy) -> Result<Vec<u8>, AbiSurfaceError> {
    encode_abi_surface(&collect_abi_surface(policy)?)
}

fn abi_surface_descriptor(policy: crate::SyscallPolicy) -> Result<&'static [u8], AbiSurfaceError> {
    use std::sync::OnceLock;

    static ABI_V1: OnceLock<Result<Vec<u8>, AbiSurfaceError>> = OnceLock::new();
    let crate::SyscallPolicy::AbiV1 = policy;
    match ABI_V1.get_or_init(|| build_abi_surface_descriptor(policy)) {
        Ok(descriptor) => Ok(descriptor.as_slice()),
        Err(error) => Err(*error),
    }
}

/// Compute the stable first-release ABI hash for the complete allowed surface.
///
/// The domain-separated, versioned, length-prefixed descriptor binds the
/// ABI-v1 policy tag; indexed-literal opcodes, table kinds, and payload layouts;
/// every sorted syscall signature and host-access class; every allowed pointer
/// type; durable-state caps, ordering, storage, paging, and path derivation;
/// typed core-query entity tags, projections, and page semantics; recursive
/// entrypoint `List`, `Int`, `Decimal`, and `Quantity` kinds; and canonical
/// numeric-domain, division, wrapping, encoding, and rounding rules. Gas prices remain bound
/// independently by the gas-schedule hash. A malformed compiled registry
/// returns a diagnostic sentinel with an invalid Iroha-hash marker; release
/// tests require that path to be unreachable, and a valid Iroha hash can never
/// equal such a sentinel.
pub fn compute_abi_hash(policy: crate::SyscallPolicy) -> [u8; 32] {
    match abi_surface_descriptor(policy) {
        Ok(descriptor) => *iroha_crypto::Hash::new(descriptor).as_ref(),
        Err(error) => invalid_abi_surface_hash(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_surface() -> AbiSurface {
        collect_abi_surface(crate::SyscallPolicy::AbiV1).expect("canonical ABI-v1 surface")
    }

    #[test]
    fn numeric_v1_ranges_are_complete_and_legacy_numbers_fail_closed() {
        let policy = crate::SyscallPolicy::AbiV1;
        let expected = (0x01_0100..=0x01_0113)
            .chain(0x01_0120..=0x01_012F)
            .chain(0x01_0140..=0x01_014F)
            .collect::<Vec<_>>();
        assert_eq!(expected.len(), 52);
        for number in expected {
            assert!(is_numeric_v1_syscall(number));
            assert!(is_syscall_allowed(policy, number));
            assert_eq!(registered_syscall_access(number), Some(SyscallAccess::None));
            assert!(syscall_name(number).is_some());
        }

        for retired in (0x69..=0x76)
            .chain(0xD2..=0xDE)
            .chain(0x01_0040..=0x01_004D)
        {
            assert!(!is_syscall_allowed(policy, retired), "retired {retired:#x}");
            assert_eq!(registered_syscall_access(retired), None);
            assert_eq!(syscall_name(retired), None);
        }
    }

    fn descriptor_hash(surface: &AbiSurface) -> [u8; 32] {
        let descriptor = encode_abi_surface(surface).expect("test surface is canonical");
        *iroha_crypto::Hash::new(descriptor).as_ref()
    }

    fn assert_surface_mutation_changes_hash(mutator: impl FnOnce(&mut AbiSurface)) {
        let canonical_surface = canonical_surface();
        let canonical_hash = descriptor_hash(&canonical_surface);
        let mut changed = canonical_surface;
        mutator(&mut changed);
        assert_ne!(descriptor_hash(&changed), canonical_hash);
    }

    #[test]
    fn canonical_helper_syscall_maps_direct_aliases() {
        let direct_pairs = [
            (SYSCALL_JSON_GET_JSON_DIRECT, SYSCALL_JSON_GET_JSON),
            (SYSCALL_JSON_GET_NAME_DIRECT, SYSCALL_JSON_GET_NAME),
            (
                SYSCALL_JSON_GET_ACCOUNT_ID_DIRECT,
                SYSCALL_JSON_GET_ACCOUNT_ID,
            ),
            (SYSCALL_JSON_GET_NFT_ID_DIRECT, SYSCALL_JSON_GET_NFT_ID),
            (SYSCALL_JSON_GET_BLOB_HEX_DIRECT, SYSCALL_JSON_GET_BLOB_HEX),
            (SYSCALL_JSON_GET_INT_DIRECT, SYSCALL_JSON_GET_INT),
            (SYSCALL_JSON_GET_DECIMAL_DIRECT, SYSCALL_JSON_GET_DECIMAL),
            (SYSCALL_JSON_GET_QUANTITY_DIRECT, SYSCALL_JSON_GET_QUANTITY),
            (
                SYSCALL_JSON_GET_ASSET_DEFINITION_ID_DIRECT,
                SYSCALL_JSON_GET_ASSET_DEFINITION_ID,
            ),
            (SYSCALL_JSON_SET_I64_DIRECT, SYSCALL_JSON_SET_I64),
            (
                SYSCALL_JSON_SET_ACCOUNT_ID_DIRECT,
                SYSCALL_JSON_SET_ACCOUNT_ID,
            ),
            (
                SYSCALL_BUILD_PATH_KEY_NORITO_DIRECT,
                SYSCALL_BUILD_PATH_KEY_NORITO,
            ),
            (SYSCALL_SCHEMA_INFO_DIRECT, SYSCALL_SCHEMA_INFO),
            (SYSCALL_SCHEMA_ENCODE_DIRECT, SYSCALL_SCHEMA_ENCODE),
            (SYSCALL_SCHEMA_DECODE_DIRECT, SYSCALL_SCHEMA_DECODE),
        ];

        for (direct, canonical) in direct_pairs {
            assert_eq!(canonical_helper_syscall(direct), canonical);
            assert_eq!(canonical_helper_syscall(canonical), canonical);
        }

        assert_eq!(
            canonical_helper_syscall(SYSCALL_STATE_GET),
            SYSCALL_STATE_GET
        );
    }

    #[test]
    fn koto_test_syscalls_are_host_private() {
        let private = [
            SYSCALL_KOTO_TEST_ACTOR_ACCOUNT,
            SYSCALL_KOTO_TEST_ACTOR_PUBLIC_KEY,
            SYSCALL_KOTO_TEST_ACTOR_SIGN,
            SYSCALL_KOTO_TEST_INVOKE_ENTRYPOINT_AS,
            SYSCALL_KOTO_TEST_EXPECT_REJECT_AS,
        ];

        for syscall in private {
            assert!(is_koto_test_syscall(syscall));
            assert!(!is_syscall_allowed(crate::SyscallPolicy::AbiV1, syscall));
            assert!(syscall > u8::MAX as u32);
        }
    }

    #[test]
    fn syscall_access_classification_is_conservative() {
        assert_eq!(syscall_access(SYSCALL_STATE_GET), SyscallAccess::StateRead);
        assert_eq!(syscall_access(SYSCALL_STATE_SET), SyscallAccess::StateWrite);
        assert_eq!(
            syscall_access(SYSCALL_STATE_MAP_KEY_AT),
            SyscallAccess::None
        );
        assert_eq!(
            syscall_access(SYSCALL_STATE_VALUE_ENCODE),
            SyscallAccess::None
        );
        assert_eq!(
            syscall_access(SYSCALL_STATE_VALUE_DECODE),
            SyscallAccess::None
        );
        assert_eq!(
            syscall_access(SYSCALL_CORE_QUERY_GET),
            SyscallAccess::LedgerRead
        );
        assert_eq!(
            syscall_access(SYSCALL_CORE_QUERY_PAGE),
            SyscallAccess::LedgerRead
        );
        assert_eq!(
            syscall_access(SYSCALL_VRF_EPOCH_SEED),
            SyscallAccess::LedgerRead
        );
        assert_eq!(
            syscall_access(SYSCALL_TRANSFER_ASSET_SCOPED),
            SyscallAccess::LedgerWrite
        );
        assert_eq!(
            syscall_access(SYSCALL_CALL_CONTRACT),
            SyscallAccess::Dynamic
        );
        assert_eq!(syscall_access(SYSCALL_SHA256_HASH), SyscallAccess::None);
        assert_eq!(syscall_access(0x00ff_fffe), SyscallAccess::Dynamic);

        for number in abi_syscall_list() {
            assert!(
                syscall_name(*number).is_some(),
                "ABI syscall 0x{number:06x} lacks a registry name"
            );
        }
    }

    #[test]
    fn abi_v1_has_one_canonical_signature_per_allowed_syscall() {
        let allowed = abi_syscall_list();
        assert_eq!(
            syscalls_doc_gen::DOCS.len(),
            allowed.len(),
            "the canonical signature registry must exactly cover ABI v1"
        );
        for (&number, row) in allowed.iter().zip(syscalls_doc_gen::DOCS) {
            assert_eq!(
                row.number, number,
                "canonical signatures must use sorted ABI-v1 syscall order"
            );
            assert!(
                syscall_name(number).is_some_and(|name| !name.is_empty()),
                "ABI syscall 0x{number:06x} must have a canonical name"
            );
            assert!(!row.args.is_empty());
            assert!(!row.ret.is_empty());
        }

        let surface = collect_abi_syscall_surface(allowed, syscalls_doc_gen::DOCS)
            .expect("the compiled ABI-v1 registry must be valid");
        assert_eq!(surface.len(), allowed.len());
        assert!(build_abi_surface_descriptor(crate::SyscallPolicy::AbiV1).is_ok());
        assert!(abi_surface_descriptor(crate::SyscallPolicy::AbiV1).is_ok());
        let hash = compute_abi_hash(crate::SyscallPolicy::AbiV1);
        assert_ne!(hash, INVALID_ABI_SURFACE_HASH);
        assert_eq!(hash[hash.len() - 1] & 1, 1, "valid Iroha hash marker");
    }

    #[test]
    fn abi_hash_descriptor_binds_every_semantic_surface_component() {
        let surface = canonical_surface();
        let canonical = descriptor_hash(&surface);
        assert_eq!(canonical, compute_abi_hash(crate::SyscallPolicy::AbiV1));

        assert_surface_mutation_changes_hash(|changed| {
            changed.descriptor_format_version += 1;
        });
        assert_surface_mutation_changes_hash(|changed| changed.policy_tag += 1);
        assert_surface_mutation_changes_hash(|changed| {
            changed.indexed_literals[1].opcode ^= 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.indexed_literals[1].table_kind ^= 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.indexed_literals[1].payload_layout = "mutated scalar layout";
        });
        assert_surface_mutation_changes_hash(|changed| {
            let last = changed.syscalls.last_mut().expect("ABI has syscalls");
            last.number = last
                .number
                .checked_add(1)
                .expect("last syscall number has headroom");
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.syscalls[0].args = "mutated arguments";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.syscalls[0].ret = "mutated return";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.syscalls[0].access = if changed.syscalls[0].access == SyscallAccess::Dynamic {
                SyscallAccess::None
            } else {
                SyscallAccess::Dynamic
            };
        });
        assert_surface_mutation_changes_hash(|changed| {
            let _ = changed.syscalls.pop();
        });
        assert_surface_mutation_changes_hash(|changed| {
            let _ = changed.pointer_type_ids.pop();
        });
    }

    #[test]
    fn abi_hash_descriptor_binds_every_durable_state_semantic() {
        let state = canonical_surface().durable_state;
        assert_eq!(state.keys_max_items, STATE_KEYS_MAX_ITEMS);
        assert_eq!(state.max_path_bytes, STATE_MAX_PATH_BYTES as u64);
        assert_eq!(state.max_value_bytes, STATE_MAX_VALUE_BYTES as u64);
        assert_eq!(state.map_max_key_bytes, STATE_MAP_MAX_KEY_BYTES as u64);
        assert_eq!(state.map_max_base_bytes, STATE_MAP_MAX_BASE_BYTES as u64);
        assert_eq!(state.map_max_page_bytes, STATE_MAP_MAX_PAGE_BYTES as u64);

        assert_surface_mutation_changes_hash(|changed| {
            changed.durable_state.semantics_version += 1
        });
        assert_surface_mutation_changes_hash(|changed| changed.durable_state.keys_max_items += 1);
        assert_surface_mutation_changes_hash(|changed| changed.durable_state.max_path_bytes += 1);
        assert_surface_mutation_changes_hash(|changed| changed.durable_state.max_value_bytes += 1);
        assert_surface_mutation_changes_hash(|changed| {
            changed.durable_state.map_max_key_bytes += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.durable_state.map_max_base_bytes += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.durable_state.map_max_page_bytes += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.durable_state.path_size_unit = "decoded UTF-8 bytes";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.durable_state.value_storage = "full pointer TLV envelope";
        });
        assert_surface_mutation_changes_hash(|changed| changed.durable_state.ordering_version += 1);
        assert_surface_mutation_changes_hash(|changed| {
            changed.durable_state.key_ordering = "host insertion order";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.durable_state.prefix_match = "raw text prefix";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.durable_state.map_path_derivation_version += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.durable_state.map_path_derivation = "base + slash + debug(key)";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.durable_state.page_overflow = "truncate selected page";
        });
    }

    #[test]
    fn abi_hash_descriptor_binds_core_query_tags_projection_shapes_and_page_semantics() {
        use crate::core_query::{CoreQueryEntityTagV1 as Tag, QUERY_PAGE_CAPACITY_V1};

        let surface = canonical_surface();
        assert_eq!(
            surface
                .core_query_projections
                .iter()
                .map(|projection| projection.entity_tag)
                .collect::<Vec<_>>(),
            vec![
                Tag::Account.as_u64(),
                Tag::Asset.as_u64(),
                Tag::AssetDefinition.as_u64(),
                Tag::Domain.as_u64(),
                Tag::Nft.as_u64(),
            ]
        );
        assert_eq!(
            surface
                .core_query_projections
                .iter()
                .map(|projection| {
                    (
                        projection.name,
                        projection
                            .fields
                            .iter()
                            .map(|field| (field.name, field.ty))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
            vec![
                (
                    "AccountView",
                    vec![("id", "AccountId"), ("metadata", "Json")]
                ),
                ("AssetView", vec![("id", "AssetId"), ("amount", "Quantity")]),
                (
                    "AssetDefinitionView",
                    vec![
                        ("id", "AssetDefinitionId"),
                        ("name", "String"),
                        ("description", "Option<String>"),
                        ("owned_by", "AccountId"),
                        ("total_quantity", "Quantity"),
                        ("metadata", "Json"),
                    ]
                ),
                (
                    "DomainView",
                    vec![
                        ("id", "DomainId"),
                        ("owned_by", "AccountId"),
                        ("metadata", "Json"),
                    ]
                ),
                (
                    "NftView",
                    vec![
                        ("id", "NftId"),
                        ("owned_by", "AccountId"),
                        ("content", "Json"),
                    ]
                ),
            ]
        );
        assert_eq!(
            usize::from(surface.query_page.items_capacity),
            QUERY_PAGE_CAPACITY_V1
        );
        assert_eq!(
            surface
                .query_page
                .fields
                .iter()
                .map(|field| (field.name, field.ty))
                .collect::<Vec<_>>(),
            vec![("items", "List<T,64>"), ("next_offset", "Option<i64>")]
        );
        assert_eq!(
            surface.query_page.next_offset_semantics,
            "present-iff-another-canonical-page-exists;some-requires-nonempty-items;nonnegative;not-less-than-item-count;from-window=offset+item-count-with-checked-i64"
        );

        for projection_index in 0..surface.core_query_projections.len() {
            assert_surface_mutation_changes_hash(|changed| {
                changed.core_query_projections[projection_index].entity_tag += 10;
            });
            assert_surface_mutation_changes_hash(|changed| {
                changed.core_query_projections[projection_index].name = "MutatedView";
            });
            for field_index in 0..surface.core_query_projections[projection_index]
                .fields
                .len()
            {
                assert_surface_mutation_changes_hash(|changed| {
                    changed.core_query_projections[projection_index].fields[field_index].name =
                        "mutated_field";
                });
                assert_surface_mutation_changes_hash(|changed| {
                    changed.core_query_projections[projection_index].fields[field_index].ty =
                        "Blob";
                });
            }
            assert_surface_mutation_changes_hash(|changed| {
                changed.core_query_projections[projection_index]
                    .fields
                    .swap(0, 1);
            });
        }
        assert_surface_mutation_changes_hash(|changed| {
            changed.query_page.items_capacity -= 1;
        });
        for field_index in 0..surface.query_page.fields.len() {
            assert_surface_mutation_changes_hash(|changed| {
                changed.query_page.fields[field_index].name = "mutated_field";
            });
            assert_surface_mutation_changes_hash(|changed| {
                changed.query_page.fields[field_index].ty = "Blob";
            });
        }
        assert_surface_mutation_changes_hash(|changed| {
            changed.query_page.fields.swap(0, 1);
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.query_page.next_offset_semantics = "present-on-every-page";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.query_page.item_ordering = "host-insertion-order";
        });
    }

    #[test]
    fn abi_hash_descriptor_binds_entrypoint_numeric_and_recursive_list_semantics() {
        use crate::{
            entrypoint::{
                MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH, MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES,
                MAX_ENTRYPOINT_LIST_CAPACITY_V1, MIN_ENTRYPOINT_LIST_CAPACITY_V1,
            },
            pointer_abi::PointerType,
        };

        let surface = canonical_surface();
        assert_eq!(surface.entrypoint.int_kind, "Int");
        assert_eq!(surface.entrypoint.decimal_kind, "Decimal");
        assert_eq!(surface.entrypoint.quantity_kind, "Quantity");
        assert_eq!(
            surface.entrypoint.int_pointer_type_id,
            PointerType::Int as u16
        );
        assert_eq!(
            surface.entrypoint.decimal_pointer_type_id,
            PointerType::Decimal as u16
        );
        assert_eq!(
            surface.entrypoint.quantity_pointer_type_id,
            PointerType::Quantity as u16
        );
        assert_eq!(
            surface.entrypoint.list_min_capacity,
            MIN_ENTRYPOINT_LIST_CAPACITY_V1
        );
        assert_eq!(
            surface.entrypoint.list_max_capacity,
            MAX_ENTRYPOINT_LIST_CAPACITY_V1
        );
        assert_eq!(
            surface.entrypoint.max_schema_nodes,
            u64::try_from(MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES).expect("node limit fits u64")
        );
        assert_eq!(
            surface.entrypoint.max_schema_depth,
            u64::try_from(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH).expect("depth limit fits u64")
        );

        assert_surface_mutation_changes_hash(|changed| {
            changed.entrypoint.schema_version += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.entrypoint.int_kind = "MutatedInt";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.entrypoint.int_pointer_type_id += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.entrypoint.decimal_kind = "MutatedDecimal";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.entrypoint.decimal_pointer_type_id += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.entrypoint.quantity_kind = "MutatedQuantity";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.entrypoint.quantity_pointer_type_id += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.entrypoint.list_layout = "nested-recursive-record";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.entrypoint.list_child_count += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.entrypoint.list_capacity_is_schema_bound = false;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.entrypoint.list_min_capacity += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.entrypoint.list_max_capacity -= 1;
        });
    }

    #[test]
    fn abi_hash_descriptor_binds_numeric_pointer_rules_and_rounding_tags() {
        use crate::pointer_abi::PointerType;

        let surface = canonical_surface();
        for expected in [
            PointerType::Int,
            PointerType::Decimal,
            PointerType::Quantity,
        ] {
            assert_eq!(
                surface
                    .pointer_type_ids
                    .iter()
                    .filter(|&&type_id| type_id == expected as u16)
                    .count(),
                1
            );
        }
        assert!(!surface.pointer_type_ids.contains(&0x0013));
        assert_eq!(surface.numeric.int_pointer_type_id, PointerType::Int as u16);
        assert_eq!(
            surface.numeric.decimal_pointer_type_id,
            PointerType::Decimal as u16
        );
        assert_eq!(
            surface.numeric.quantity_pointer_type_id,
            PointerType::Quantity as u16
        );
        assert_eq!(surface.numeric.mantissa_bits, 512);
        assert_eq!(surface.numeric.max_scale, 28);
        assert_eq!(
            surface
                .numeric
                .rounding_modes
                .iter()
                .map(|mode| (mode.name, mode.tag))
                .collect::<Vec<_>>(),
            vec![
                ("toward_zero", 0),
                ("away_from_zero", 1),
                ("floor", 2),
                ("ceil", 3),
                ("nearest_even", 4),
                ("nearest_away", 5),
                ("nearest_toward_zero", 6),
            ]
        );
        assert_eq!(
            surface
                .numeric
                .faults
                .iter()
                .map(|fault| (fault.name, fault.tag))
                .collect::<Vec<_>>(),
            vec![
                ("mantissa_overflow", 1),
                ("scale_overflow", 2),
                ("division_by_zero", 3),
                ("repeating_decimal", 4),
                ("exact_division_scale_overflow", 5),
                ("invalid_scale", 6),
                ("inexact_conversion", 7),
                ("negative_quantity", 8),
                ("quantity_underflow", 9),
                ("invalid_rounding_mode", 10),
                ("invalid_failure_mode", 11),
                ("reserved_register_nonzero", 12),
            ]
        );

        assert_surface_mutation_changes_hash(|changed| {
            let numeric_id = changed
                .pointer_type_ids
                .iter_mut()
                .find(|type_id| **type_id == PointerType::Decimal as u16)
                .expect("Decimal pointer type is allowed");
            *numeric_id += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.numeric.int_pointer_type_id += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.numeric.decimal_pointer_type_id += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.numeric.quantity_pointer_type_id += 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.numeric.mantissa_bits -= 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.numeric.max_scale -= 1;
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.numeric.int_domain = "unbounded";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.numeric.decimal_domain = "binary-float";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.numeric.quantity_domain = "signed";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.numeric.canonicalization = "preserve-source-scale";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.numeric.integer_division = "floor";
        });
        assert_surface_mutation_changes_hash(|changed| {
            changed.numeric.wrapping_modulus = "2^64";
        });
        for mode_index in 0..surface.numeric.rounding_modes.len() {
            assert_surface_mutation_changes_hash(|changed| {
                changed.numeric.rounding_modes[mode_index].name = "mutated_mode";
            });
            assert_surface_mutation_changes_hash(|changed| {
                changed.numeric.rounding_modes[mode_index].tag += 10;
            });
        }
        for fault_index in 0..surface.numeric.faults.len() {
            assert_surface_mutation_changes_hash(|changed| {
                changed.numeric.faults[fault_index].name = "mutated_fault";
            });
            assert_surface_mutation_changes_hash(|changed| {
                changed.numeric.faults[fault_index].tag += 20;
            });
        }
    }

    #[test]
    fn abi_descriptor_framing_distinguishes_ambiguous_byte_partitions() {
        let mut first = AbiDescriptorEncoder::default();
        first.field("ab", b"c").expect("small descriptor field");
        first.field("d", b"ef").expect("small descriptor field");

        let mut second = AbiDescriptorEncoder::default();
        second.field("a", b"bc").expect("small descriptor field");
        second.field("de", b"f").expect("small descriptor field");

        assert_ne!(first.finish(), second.finish());
    }

    #[test]
    fn abi_registry_validation_rejects_missing_duplicate_and_extra_rows() {
        let number = SYSCALL_EXIT;
        assert_eq!(
            collect_abi_syscall_surface(&[number], &[]),
            Err(AbiSurfaceError::MissingSignature(number))
        );

        let row = SyscallDoc {
            number,
            args: "r10=status:u64",
            ret: "u64=status",
            gas: "G_exit",
        };
        let duplicate = [
            SyscallDoc {
                number: row.number,
                args: row.args,
                ret: row.ret,
                gas: row.gas,
            },
            SyscallDoc {
                number: row.number,
                args: row.args,
                ret: row.ret,
                gas: row.gas,
            },
        ];
        assert_eq!(
            collect_abi_syscall_surface(&[number], &duplicate),
            Err(AbiSurfaceError::DuplicateSignature(number))
        );

        let extra = SyscallDoc {
            number: SYSCALL_ABORT,
            args: "-",
            ret: "u64=0",
            gas: "G_abort",
        };
        assert_eq!(
            collect_abi_syscall_surface(&[number], &[extra]),
            Err(AbiSurfaceError::UnexpectedSignature(SYSCALL_ABORT))
        );
        assert!(matches!(
            collect_abi_syscall_surface(&[SYSCALL_ABORT, SYSCALL_EXIT], &duplicate),
            Err(AbiSurfaceError::SyscallsNotStrictlySorted { .. })
        ));
    }

    #[test]
    fn invalid_surface_sentinels_cannot_be_mistaken_for_iroha_hashes() {
        let errors = [
            AbiSurfaceError::EmptySyscallSurface,
            AbiSurfaceError::EmptyPointerSurface,
            AbiSurfaceError::SyscallsNotStrictlySorted {
                previous: 2,
                current: 1,
            },
            AbiSurfaceError::PointerTypesNotStrictlySorted {
                previous: 2,
                current: 1,
            },
            AbiSurfaceError::MissingSignature(1),
            AbiSurfaceError::DuplicateSignature(1),
            AbiSurfaceError::UnexpectedSignature(1),
            AbiSurfaceError::EmptyArguments(1),
            AbiSurfaceError::EmptyReturn(1),
            AbiSurfaceError::SurfaceTooLarge,
        ];
        let mut sentinels = errors.map(invalid_abi_surface_hash);
        assert!(sentinels.iter().all(|sentinel| sentinel[31] & 1 == 0));
        sentinels.sort_unstable();
        assert!(sentinels.windows(2).all(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn gas_text_is_not_part_of_the_abi_surface_hash() {
        let canonical = &syscalls_doc_gen::DOCS[0];
        let changed_gas = SyscallDoc {
            number: canonical.number,
            args: canonical.args,
            ret: canonical.ret,
            gas: "a deliberately different gas schedule",
        };
        let canonical_surface =
            collect_abi_syscall_surface(&[canonical.number], std::slice::from_ref(canonical))
                .expect("single canonical row");
        let changed_surface = collect_abi_syscall_surface(&[canonical.number], &[changed_gas])
            .expect("single altered-gas row");
        assert_eq!(canonical_surface, changed_surface);
    }

    #[test]
    fn pointer_policy_hash_input_is_sorted_complete_and_unique() {
        let policy = crate::SyscallPolicy::AbiV1;
        let allowed = crate::pointer_abi::policy_pointer_types(policy);
        for &pointer_type in crate::pointer_abi::PointerType::all() {
            assert_eq!(
                allowed.contains(&pointer_type),
                !matches!(pointer_type, crate::pointer_abi::PointerType::TestOnly),
                "pointer policy completeness for {pointer_type:?}"
            );
        }
        let mut ids = allowed
            .iter()
            .map(|pointer_type| *pointer_type as u16)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
        let mut surface = canonical_surface();
        surface.pointer_type_ids = ids;
        assert!(encode_abi_surface(&surface).is_ok());
    }
}
