//! IVM Syscall Table (ABI v1)

This document lists IVM syscall numbers and their ABI for `abi_version = 1`.
The host and VM enforce an ABI‑version gating policy: for each `abi_version` the
set of available syscalls is fixed and unknown or disallowed numbers must be
rejected with `E_SCALL_UNKNOWN` (mapped to `VMError::UnknownSyscall`). The
canonical policy is centralized in `ivm::syscalls::is_syscall_allowed(policy, number)`.

ABI policy
- V1 (1): allows the canonical ABI surface listed here and in `abi_syscall_list()`; unknown numbers
  are rejected uniformly across all hosts. The list is kept sorted/deduplicated and the golden test
  fails if ordering or contents drift.
- Experimental(x): no syscalls are enabled until the versioned surface is defined; unknown numbers
  are rejected by the central policy.

Admission/host guardrails
- Admission enforces manifest `code_hash`/`abi_hash` equality for both inline metadata manifests and
  WSV‑stored manifests before execution, returning `ManifestCodeHashMismatch`/`ManifestAbiHashMismatch`
  deterministically.
- Admission decodes the instruction stream and rejects `SCALL` numbers outside the ABI surface with
  `ValidationFail::NotPermitted` before execution, so mutated or malformed bytecode never reaches the
  host.
- Runtime hosts must return `VMError::UnknownSyscall` for disallowed syscall numbers; the executor
  surfaces the failure during validation so contracts cannot rely on undefined syscalls.
- Regression tests cover host-side `UnknownSyscall` rejections, admission-time `SCALL` gating
  (including manifest-backed programs), and manifest `abi_hash` enforcement across both metadata and
  WSV manifests to keep the ABI surface deterministic end-to-end.

Numbers are 8‑bit in bytecode; the host receives a `u32` but only the low byte is valid. Structured arguments use the pointer‑ABI (Norito TLV in INPUT); scalar values are passed in `r10+`. Return values are `u64` unless noted; pointer results are returned in `r10`.

Examples (dev envelopes)
- Execute query (JSON envelope) `0xA1`: set `r10` to a `&Json` TLV with `{ "type": "wsv.get_balance", "payload": { "account_id": "…", "asset_id": "…" } }`. On success, `r10` receives a pointer to a `&Json` TLV like `{ "balance": 42 }` in INPUT.
- List triggers (JSON envelope): `{ "type": "wsv.list_triggers", "payload": {} }` → `{ "triggers": [{"name":"…","enabled":true}, …] }` via `r10`.

JSON Envelope Matrix (dev)
- Queries via `0xA1` (EXECUTE_QUERY): `wsv.get_balance`, `wsv.list_triggers`, `wsv.has_permission` (returns a `&Json` TLV in `r10`).
- Admin via `0xA0` (EXECUTE_INSTRUCTION): `wsv.create_role`, `wsv.grant_role`, `wsv.revoke_role`, `wsv.grant_permission`, `wsv.revoke_permission`, `wsv.create_trigger`, `wsv.set_trigger_enabled`, `wsv.remove_trigger`, and helpers for FT/NFT mint/burn/transfer.
- Notes: The JSON envelope path is intended for tests/dev tooling; production contracts should prefer Norito TLVs. Hosts enforce the same permission checks as dedicated syscalls.

Minimal envelope table
| Envelope id              | Opcode | Args TLV  | Return        |
|--------------------------|--------|----------|---------------|
| `wsv.get_balance`        | 0xA1   | `&Json`  | `ptr (&Json)` |
| `wsv.list_triggers`      | 0xA1   | `&Json`  | `ptr (&Json)` |
| `wsv.has_permission`     | 0xA1   | `&Json`  | `ptr (&Json)` |
| `wsv.create_role`        | 0xA0   | `&Json`  | `u64=0`       |
| `wsv.grant_role`         | 0xA0   | `&Json`  | `u64=0`       |
| `wsv.revoke_role`        | 0xA0   | `&Json`  | `u64=0`       |
| `wsv.grant_permission`   | 0xA0   | `&Json`  | `u64=0`       |
| `wsv.revoke_permission`  | 0xA0   | `&Json`  | `u64=0`       |
| `wsv.create_trigger`     | 0xA0   | `&Json`  | `u64=0`       |
| `wsv.set_trigger_enabled`| 0xA0   | `&Json`  | `u64=0`       |
| `wsv.remove_trigger`     | 0xA0   | `&Json`  | `u64=0`       |

Ordering and OUTPUT
- Syscalls execute in program order. Hosts must apply their side effects in the order received.
- `COMMIT_OUTPUT (0xFE)` makes the VM OUTPUT region visible to the host. Programs may write multiple times to OUTPUT, but content becomes observable only after `COMMIT_OUTPUT` runs. If `COMMIT_OUTPUT` is called multiple times, hosts should treat the last call’s contents as final for that run.
- The VM clears OUTPUT (and resets its append-only cursor) when loading a program; within a run, OUTPUT writes must move forward (rewinds trap).
- Event emission that reflects syscall outcomes must preserve syscall order. VM implementations must not reorder syscalls, including under acceleration. Deterministic overlays and commit phases in the node preserve this ordering across the pipeline.
- Host lifecycle: `begin_tx`/`finish_tx` return `Result`; hosts must surface overlay flush errors (e.g., durable state writes) instead of swallowing them, clear staged overlays on failure, and rely on checkpoints to restore pre-tx state when a VM run aborts.

Legend
- Args: registers and pointer types; `&Type` indicates a pointer to a Norito TLV in INPUT.
- Return: `u64` or `ptr` (pointer in `r10`).
- Gas: base component name; variable components are added for byte or item counts.

Lifecycle / Utility
- 0x01 EXIT — Args: `r10=status:u64` → Return: `u64=status` — Gas: G_exit
- 0x02 ABORT — Args: none → Return: `u64=0` — Gas: G_abort
- 0x03 DEBUG_LOG — Args: `r10=&Json` → Return: 0 — Gas: G_debug
- 0xE0 INPUT_PUBLISH_TLV — Args: `r10=&Blob(TLV)` → Return: `ptr (r10)` — Gas: G_input_publish
- 0x90 SM3_HASH — Args: `r10=&Blob(message)` → Return: `ptr (&Blob(digest))` — Gas: G_sm3
- 0x91 SM2_VERIFY — Args: `r10=&Blob(msg)`, `r11=&Blob(sig)` (64-byte r∥s), `r12=&Blob(pubkey)` (SEC1), `r13=&Blob(distid)` *(optional, 0 for default)* → Return: `u64=0/1` — Gas: G_sm2_verify
- 0x92 SM4_GCM_SEAL — Args: `r10=&Blob(key16)`, `r11=&Blob(nonce12)`, `r12=&Blob(aad)` *(0 => empty)*, `r13=&Blob(plaintext)` → Return: `ptr (&Blob(ciphertext || tag16))` — Gas: G_sm4_seal
- 0x93 SM4_GCM_OPEN — Args: `r10=&Blob(key16)`, `r11=&Blob(nonce12)`, `r12=&Blob(aad)` *(0 => empty)*, `r13=&Blob(ciphertext || tag16)` → Return: `ptr (&Blob(plaintext))` or `0` on failure — Gas: G_sm4_open
- 0x94 SM4_CCM_SEAL — Args: `r10=&Blob(key16)`, `r11=&Blob(nonce[7..13])`, `r12=&Blob(aad)` *(0 => empty)*, `r13=&Blob(plaintext)`, `r14=tag_len:u64` *(0 => 16)* → Return: `ptr (&Blob(ciphertext || tag))` — Gas: G_sm4_seal
- 0x95 SM4_CCM_OPEN — Args: `r10=&Blob(key16)`, `r11=&Blob(nonce[7..13])`, `r12=&Blob(aad)` *(0 => empty)*, `r13=&Blob(ciphertext || tag)`, `r14=tag_len:u64` *(0 => 16)* → Return: `ptr (&Blob(plaintext))` or `0` on failure — Gas: G_sm4_open
- 0xFD GET_PRIVATE_INPUT — Args: `r10=index:u64` → Return: `r10=value` — Gas: G_get_priv
- 0xFE COMMIT_OUTPUT — Args: none → Return: `u64=0` — Gas: G_commit

For the SM4 calls, the host appends the authentication tag to the ciphertext output; callers supply the same layout when invoking the corresponding `OPEN` syscall. `SM4_GCM_*` always uses a 16-byte tag and 12-byte nonce. `SM4_CCM_*` accepts nonce lengths between 7 and 13 bytes and tag sizes {4,6,8,10,12,14,16}; pass the desired tag length in `r14` (use `0` to select 16). Passing `0` in `r12` denotes an empty AAD.

Kotodama intrinsics
- ``sm::hash(msg: Blob) -> Blob`` mirrors `msg` into INPUT with `INPUT_PUBLISH_TLV` and issues `SM3_HASH`, returning a pointer to the digest Blob.
- ``sm::verify(msg: Blob, sig: Blob, pk: Blob[, distid: Blob]) -> bool`` mirrors each Blob argument into INPUT, invokes `SM2_VERIFY`, and returns `true` for valid signatures. Omitting the fourth argument selects the runtime-configured default (``Sm2PublicKey::default_distid()``, sourced from `crypto.sm2_distid_default`); providing it enforces a custom distinguishing identifier.

Domains / Peers
- 0x10 REGISTER_DOMAIN — Args: `r10=&DomainId` → 0 — Gas: G_reg_domain
- 0x11 UNREGISTER_DOMAIN — Args: `r10=&DomainId` → 0 — Gas: G_unreg_domain
- 0x12 TRANSFER_DOMAIN — Args: `r10=&DomainId, r11=&AccountId` → 0 — Gas: G_xfer_domain
- 0x15 REGISTER_PEER — Args: `r10=&Json` (peer info) → 0 — Gas: G_reg_peer
- 0x16 UNREGISTER_PEER — Args: `r10=&Json` → 0 — Gas: G_unreg_peer

Accounts
- 0x13 REGISTER_ACCOUNT — Args: `r10=&AccountId` → 0 — Gas: G_reg_acct
- 0x14 UNREGISTER_ACCOUNT — Args: `r10=&AccountId` → 0 — Gas: G_unreg_acct
- 0x17 ADD_SIGNATORY — Args: `r10=&AccountId, r11=&Json` (pubkey) → 0 — Gas: G_add_sig
- 0x18 REMOVE_SIGNATORY — Args: `r10=&AccountId, r11=&Json` → 0 — Gas: G_rm_sig
- 0x19 SET_ACCOUNT_QUORUM — Args: `r10=&AccountId, r11=quorum:u64` → 0 — Gas: G_set_quorum
- 0x1A SET_ACCOUNT_DETAIL — Args: `r10=&AccountId, r11=&Name, r12=&Json` → 0 — Gas: G_set_detail + bytes(val)

Assets (FT)
- 0x20 REGISTER_ASSET — Args: `r10=&AssetDefinitionId` → 0 — Gas: G_reg_asset
- 0x21 UNREGISTER_ASSET — Args: `r10=&AssetDefinitionId` → 0 — Gas: G_unreg_asset
- 0x22 MINT_ASSET — Args: `r10=&AccountId, r11=&AssetDefinitionId, r12=amount:u64` → 0 — Gas: G_mint
- 0x23 BURN_ASSET — Args: `r10=&AccountId, r11=&AssetDefinitionId, r12=amount:u64` → 0 — Gas: G_burn
- 0x24 TRANSFER_ASSET — Args: `r10=&AccountId(from), r11=&AccountId(to), r12=&AssetDefinitionId, r13=amount:u64` → 0 — Gas: G_transfer

NFTs
- 0x25 NFT_MINT_ASSET — Args: `r10=&NftId, r11=&AccountId(owner)` → 0 — Gas: G_nft_mint_asset
- 0x26 NFT_TRANSFER_ASSET — Args: `r10=&AccountId(from), r11=&NftId, r12=&AccountId(to)` → 0 — Gas: G_nft_transfer_asset
- 0x27 NFT_SET_METADATA — Args: `r10=&NftId, r11=&Json` → 0 — Gas: G_nft_set_metadata
- 0x28 NFT_BURN_ASSET — Args: `r10=&NftId` → 0 — Gas: G_nft_burn_asset

Zero‑knowledge (verification/state‑read)
- 0x60 ZK_VERIFY_TRANSFER — Args: `r10=&NoritoBytes(OpenVerifyEnvelope)` → `u64=0/1` — Gas: G_verify_proof
- 0x61 ZK_VERIFY_UNSHIELD — Args: `r10=&NoritoBytes(OpenVerifyEnvelope)` → `u64=0/1` — Gas: G_verify_proof
- 0x62 ZK_VOTE_VERIFY_BALLOT — Args: `r10=&NoritoBytes(OpenVerifyEnvelope)` → `u64=0/1` — Gas: G_verify_proof
- 0x63 ZK_VOTE_VERIFY_TALLY — Args: `r10=&NoritoBytes(OpenVerifyEnvelope)` → `u64=0/1` — Gas: G_verify_proof
- 0x64 ZK_ROOTS_GET — Args: `r10=&NoritoBytes(RootsGetRequest)` → `ptr (NoritoBytes(RootsGetResponse))` — Gas: G_roots_get
- 0x65 ZK_VOTE_GET_TALLY — Args: `r10=&NoritoBytes(VoteGetTallyRequest)` → `ptr (NoritoBytes(VoteGetTallyResponse))` — Gas: G_vote_get

ZK gating & determinism
- Hosts enforce deterministic gating before verification. A call returns `0` (failure) if any gate fails:
  - `zk.enabled=false`, unsupported backend (must be IPA), or disallowed curve (host-configured set; e.g., Pallas/Pasta/Bn254).
  - `max_k` exceeded for the envelope’s params (where `n = 2^k`).
  - Envelope must include `vk_commitment` and `public_inputs_schema_hash` that resolve to an active registry entry; rejects use `r11=11` (missing), `r11=12` (mismatch), or `r11=13` (inactive).
  - Namespace/manifest/domain binding: VK namespace and manifest must match the caller and the domain tag must match the recomputed hash; `r11=14` or `r11=15` on mismatch.
  - Envelope too large (`r11=8`), proof payload too large (`r11=10`), or transcript label rejected (`r11=9`, non-ASCII/too long/wrong label for the syscall).
- On success, the host returns `1`. Verification itself is deterministic; accelerators, when enabled, must match scalar results.
- Payloads use the pointer‑ABI `NoritoBytes` type; the TLV must be mirrored into INPUT and pass hash validation.
  - Required transcript labels: `zk_verify_transfer/v1`, `zk_verify_unshield/v1`, `zk_verify_ballot/v1`, `zk_verify_tally/v1`, and `zk_verify_batch/v1` (batch). Labels must be ASCII and ≤64 bytes (configurable).

Roles / Permissions
- 0x30 CREATE_ROLE — Args: `r10=&Name, r11=&Json` (perm set) → 0 — Gas: G_create_role
- 0x31 DELETE_ROLE — Args: `r10=&Name` → 0 — Gas: G_delete_role
- 0x32 GRANT_ROLE — Args: `r10=&AccountId, r11=&Name` → 0 — Gas: G_grant_role
- 0x33 REVOKE_ROLE — Args: `r10=&AccountId, r11=&Name` → 0 — Gas: G_revoke_role
- 0x34 GRANT_PERMISSION — Args: `r10=&AccountId, r11=&Name` → 0 — Gas: G_grant_perm
- 0x35 REVOKE_PERMISSION — Args: `r10=&AccountId, r11=&Name` → 0 — Gas: G_revoke_perm

Triggers
- 0x40 CREATE_TRIGGER — Args: `r10=&Json` (trigger spec) → 0 — Gas: G_create_trig
- 0x41 REMOVE_TRIGGER — Args: `r10=&Name` → 0 — Gas: G_remove_trig
- 0x42 SET_TRIGGER_ENABLED — Args: `r10=&Name, r11=enabled:u64` → 0 — Gas: G_set_trig
- 0x43 DEACTIVATE_CONTRACT_INSTANCE — Args: `r10=&NoritoBytes(DeactivateContractInstance)` → 0 — Gas: G_deactivate_contract_instance
- 0x44 REMOVE_SMART_CONTRACT_BYTES — Args: `r10=&NoritoBytes(RemoveSmartContractBytes)` → 0 — Gas: G_remove_contract_bytes
- 0x45 REGISTER_SMART_CONTRACT_CODE — Args: `r10=&NoritoBytes(RegisterSmartContractCode)` → 0 — Gas: G_register_contract_code
- 0x46 REGISTER_SMART_CONTRACT_BYTES — Args: `r10=&NoritoBytes(RegisterSmartContractBytes)` → 0 — Gas: G_register_contract_bytes
- 0x47 ACTIVATE_CONTRACT_INSTANCE — Args: `r10=&NoritoBytes(ActivateContractInstance)` → 0 — Gas: G_activate_contract_instance

Lifecycle operations expect canonical Norito encodings of the corresponding ISI structs. Hosts
trim empty `reason` strings for `DeactivateContractInstance`/`RemoveSmartContractBytes` and
enforce governance permissions before queuing the instruction.

Smart‑contract helpers (dev)
- 0xA0 EXECUTE_INSTRUCTION — Args: `r10=&Json` → 0 — Gas: G_sci

JSON envelope support for EXECUTE_INSTRUCTION
- The mock host accepts a JSON “envelope” in INPUT for `EXECUTE_INSTRUCTION` to execute a subset of instructions directly without relying on Norito bytes.
- Envelope format:
  - `{ "type": "<id>", "payload": { ... } }`
  - `<id>` may be one of:
    - ZK: `zk.RegisterZkAsset`, `zk.Shield`, `zk.ZkTransfer`, `zk.Unshield`, `zk.CreateElection`, `zk.SubmitBallot`, `zk.FinalizeElection`
    - WSV helpers: `wsv.mint_asset`, `wsv.burn_asset`, `wsv.transfer_asset`,
      `wsv.nft_mint_asset`,
      `wsv.nft_transfer_asset`, `wsv.nft_burn_asset`, `wsv.nft_set_metadata`,
      `wsv.register_domain`, `wsv.register_account`, `wsv.register_asset_definition`,
      `wsv.create_role`, `wsv.delete_role`, `wsv.grant_role`, `wsv.revoke_role`,
      `wsv.grant_permission`, `wsv.revoke_permission`, `wsv.create_trigger`,
      `wsv.remove_trigger`, `wsv.set_trigger_enabled`, `wsv.register_peer`, `wsv.unregister_peer`
- Payload examples:
  - Shield:
    - `{"type":"zk.Shield","payload":{"asset":"rose#domain","from":"<account>","amount":3,"note_commitment":[7,...,7],"enc_payload":{"version":1,"ephemeral_pubkey":[0,...,0],"nonce":[0,...,0],"ciphertext":""}}}`
  - Mint asset:
    - `{"type":"wsv.mint_asset","payload":{"account_id":"<account>","asset_id":"rose#domain","amount":100}}`
- Notes:
  - The JSON envelope is intended for tests and developer tooling; production smart‑contracts should prefer Norito TLVs generated by the compiler.
  - The host enforces the same permission checks as the dedicated syscalls (`MINT_ASSET`, `BURN_ASSET`, etc.).
- 0xA1 EXECUTE_QUERY — Args: `r10=&Json` → `ptr` — Gas: G_scq
- 0xA2 CREATE_NFTS_FOR_ALL_USERS — Args: none → `u64=count` — Gas: G_create_nfts_all
- 0xA3 SET_SMARTCONTRACT_EXECUTION_DEPTH — Args: `r10=depth:u64` → `u64=prev` — Gas: G_sc_depth
- 0xA4 GET_AUTHORITY — Args: none → `ptr` (AccountId in INPUT, `r10` points to it) — Gas: G_get_auth

AXT host flow
- 0xB0 AXT_BEGIN — Args: `r10=&AxtDescriptor`. Resets any in‑progress envelope and records the descriptor; hosts derive the canonical binding used by capability handles from this descriptor.
- 0xB1 AXT_TOUCH — Args: `r10=&DataSpaceId`, `r11=&NoritoBytes(TouchManifest)` or `0`. Declares the manifest of keys touched for the dataspace within the current envelope.
- 0xB2 AXT_COMMIT — Args: none. Validates recorded handles, manifests, and proofs for the active envelope and clears host state on success.
- 0xB3 VERIFY_DS_PROOF — Args: `r10=&DataSpaceId`, `r11=&ProofBlob` (or `0` to clear). Associates dataspace proof material with the active envelope.
- 0xB4 USE_ASSET_HANDLE — Args: `r10=&AssetHandle`, `r11=&NoritoBytes(RemoteSpendIntent)`, `r12=&ProofBlob` (optional). Validates capability bindings/budgets and records spend intents for later commit checks.
- Default and WSV hosts enforce descriptor membership, capability binding equality, budget checks, and proof presence before permitting commit.

ZK Helpers
- 0xF9 GET_ACCOUNT_BALANCE — Args: `r10=&AccountId, r11=&AssetDefinitionId` → `u64=amount` — Gas: G_get_bal
- 0xFB USE_NULLIFIER — Args: `r10=nullifier:u64` → `u64=0` — Gas: G_use_null
  
Experimental (not part of ABI hash)
- 0xFC VERIFY_SIGNATURE — Args: `r10=&Blob(message)`, `r11=&Blob(signature)`, `r12=&Blob(pubkey)`, `r13=scheme:u8` → `r10=0/1` — Gas: G_verify_sig

Hardware / Proofs
- 0xF4 PROVE_EXECUTION — Args: none → `r10=0/1` *(1 when the captured trace verifies)* — Gas: G_prove
- 0xF5 GROW_HEAP — Args: `r10=bytes:u64` → `u64=new_limit` — Gas: G_grow_heap per page
- 0xF6 VERIFY_PROOF — Args: none → `r10=0/1` *(1 when verification passes)* — Gas: G_verify
- 0xF7 GET_MERKLE_PATH — Args: `r10=addr:u64, r11=out_ptr:u64, r12=root_out:u64?` → `u64=len` — Gas: G_mpath + path_len
  - Writes the authentication path (leaf→root) to `out_ptr`. If `r12 != 0`, also writes the 32‑byte Merkle root to `root_out`.
- 0xFA GET_MERKLE_COMPACT — Args: `r10=addr:u64, r11=out_ptr:u64, r12=depth_cap:u64?, r13=root_out:u64?` → `u64=depth` — Gas: G_mpath + depth
  - Writes a compact proof to `out_ptr` using the layout `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` with siblings ordered leaf→root.
  - `dirs` encodes, for each level i, whether the running accumulator was the left (0) or right (1) child. Missing siblings are encoded as a 32‑byte zero array (promotion).
  - Caps the depth to `min(depth_cap, 32)` if `r12 != 0`, otherwise uses full path depth up to 32. If `r13 != 0`, writes the 32‑byte Merkle root at `root_out`.
- 0xFF GET_REGISTER_MERKLE_COMPACT — Args: `r10=reg_index:u64, r11=out_ptr:u64, r12=depth_cap:u64?, r13=root_out:u64?` → `u64=depth` — Gas: G_mpath + depth
  - Writes a compact proof for the register commitment using the same layout as GET_MERKLE_COMPACT.

VRF
- 0x66 VRF_VERIFY — Args: `r10=&NoritoBytes(VrfVerifyRequest{variant:u8, pk:bytes, proof:bytes, chain_id:bytes, input:bytes})` → Return: `r10=ptr (&Blob(32-byte output))`, `r11=status:u64` — Gas: G_verify
  - Status codes: `0=ok`, `1=type_mismatch`, `2=decode_error`, `3=unknown_variant`, `4=bad_pk`, `5=bad_proof`, `6=verify_fail`, `7=oom`.
  - When the host is configured with a chain_id, requests with a different `chain_id` are rejected with `r11=8 (chain_mismatch)`.
  - Proof: BLS signature over `Hash("iroha:vrf:v1:input|" || chain_id || "|" || input)` using VRF-specific DSTs:
    - G2 hash: `"BLS12381G2_XMD:SHA-256_SSWU_RO_IROHA_VRF_V1"`
    - G1 hash: `"BLS12381G1_XMD:SHA-256_SSWU_RO_IROHA_VRF_V1"`
    - Output: `Hash("iroha:vrf:v1:output" || canonical_proof_bytes)`.
  - Encodings: pk and proof MUST be canonical compressed encodings; infinity/non-subgroup are rejected.
  - Variants: `1 = SigInG2 (pk=G1 48B, proof=G2 96B)`, `2 = SigInG1 (pk=G2 96B, proof=G1 48B)`.

- 0x67 VRF_VERIFY_BATCH — Args: `r10=&NoritoBytes(VrfVerifyBatchRequest{items: [VrfVerifyRequest]})` → Return: `r10=ptr (&NoritoBytes(Vec<[u8;32]>))`, `r11=status:u64`, `r12=fail_index?:u64` — Gas: G_verify
  - Verifies each item; on success returns a Norito-encoded vector of 32‑byte outputs (order preserved). On failure, returns `r10=0`, `r11` = error code, `r12` = index (0‑based) of the first failing item.
  - If the host is configured with a chain_id, all items must match it; otherwise batch fails with `r11=8 (chain_mismatch)` and `r12` set to the first offending index.

Host gating & chain binding
- When a host `chain_id` is configured, requests must match it. Otherwise:
  - Single: `r11=8 (chain_mismatch)` and `r10=0`.
  - Batch: `r11=8`, `r12` set to the first offending index, and `r10=0`.
- Output derivation uses domain separation and canonical encodings as described above; outputs are deterministic across hardware.

Notes
- All calls execute via `CoreHost` and are subject to permission checks and invariants identical to built‑in ISIs.
- Gas names reference entries in the generated gas table for the active bytecode header version.

## ABI Evolution

This section summarizes how to safely evolve the IVM ABI (syscall surface and pointer‑ABI) without breaking determinism.

- Versioning model:
  - `ProgramMetadata.abi_version` declares which ABI a program targets. The runtime maps it to a `SyscallPolicy`.
  - Nodes enforce the ABI at admission and during execution. Programs compiled for an unsupported `abi_version` are rejected at admission.

- Adding/removing syscalls:
  - Update `ivm::syscalls::abi_syscall_list()` with the new numbers (keep the list in ascending order for readability).
  - The central gate `ivm::syscalls::is_syscall_allowed(policy, number)` applies across all hosts; no duplication is needed.
  - `abi_syscall_list()` is deduplicated and sorted at build/test time; the golden test fails if ordering or contents change unexpectedly.
  - Admission performs a static scan of decoded bytecode and rejects programs that reference syscalls outside the active policy, so unknown numbers never reach runtime execution.
  - Update golden tests:
    - `crates/ivm/tests/abi_syscall_list_golden.rs` (expected list)
    - `crates/ivm/tests/abi_hash_versions.rs` (stability and version separation)
  - Ensure hosts implement (or intentionally reject) handling for the new numbers; unknown numbers must map to `VMError::UnknownSyscall`.

- Pointer‑ABI types:
  - `ivm::pointer_abi::PointerType` numeric IDs are wire‑stable. Changing existing IDs is a breaking change; add new types with new IDs only.
  - Update policy allowlist in `ivm::pointer_abi::is_type_allowed_for_policy` for new ABI versions when introducing new types.
  - Update golden test `crates/ivm/tests/pointer_type_ids_golden.rs` when adding new types/IDs.

- Manifests and ABI hash:
  - `ivm::syscalls::compute_abi_hash(policy)` produces a stable 32‑byte digest that binds a program to an ABI policy.
  - When evolving the ABI, manifests must embed the new `abi_hash`. Nodes will reject programs whose runtime policy hash differs from the manifest.

- Migration checklist:
  1) Add/adjust syscall numbers in `abi_syscall_list()` and host implementations.
  2) If introducing an ABI version, define its policy mapping (e.g., `SyscallPolicy::V2`) and update the compiler to emit the new `abi_version`.
  3) Update golden tests (syscall list, ABI hash stability, pointer type IDs if applicable).
  4) Regenerate/re‑register manifests with the new `abi_hash`.
  5) Add node/IVM tests for allowed/disallowed syscalls and pointer‑ABI types under the new version.

<!-- BEGIN GENERATED SYSCALLS -->
| Number | Name | Args | Return | Gas |
|---|---|---|---|---|
| 0x00 | DEBUG_PRINT | - | - | asset:gas/G_debug@ivm.core/v1 |
| 0x01 | EXIT | r10=status:u64 | u64=status | asset:gas/G_exit@ivm.core/v1 |
| 0x02 | ABORT | - | u64=0 | asset:gas/G_abort@ivm.core/v1 |
| 0x03 | DEBUG_LOG | r10=&Json | u64=0 | asset:gas/G_debug@ivm.core/v1 |
| 0x10 | REGISTER_DOMAIN | r10=&DomainId | u64=0 | asset:gas/G_reg_domain@ivm.core/v1 |
| 0x11 | UNREGISTER_DOMAIN | r10=&DomainId | u64=0 | asset:gas/G_unreg_domain@ivm.core/v1 |
| 0x12 | TRANSFER_DOMAIN | r10=&DomainId, r11=&AccountId | u64=0 | asset:gas/G_xfer_domain@ivm.core/v1 |
| 0x13 | REGISTER_ACCOUNT | r10=&AccountId | u64=0 | asset:gas/G_reg_acct@ivm.core/v1 |
| 0x14 | UNREGISTER_ACCOUNT | r10=&AccountId | u64=0 | asset:gas/G_unreg_acct@ivm.core/v1 |
| 0x15 | REGISTER_PEER | r10=&Json | u64=0 | asset:gas/G_reg_peer@ivm.core/v1 |
| 0x16 | UNREGISTER_PEER | r10=&Json | u64=0 | asset:gas/G_unreg_peer@ivm.core/v1 |
| 0x17 | ADD_SIGNATORY | r10=&AccountId, r11=&Json(pubkey) | u64=0 | asset:gas/G_add_sig@ivm.core/v1 |
| 0x18 | REMOVE_SIGNATORY | r10=&AccountId, r11=&Json(pubkey) | u64=0 | asset:gas/G_rm_sig@ivm.core/v1 |
| 0x19 | SET_ACCOUNT_QUORUM | r10=&AccountId, r11=quorum:u64 | u64=0 | asset:gas/G_set_quorum@ivm.core/v1 |
| 0x1A | SET_ACCOUNT_DETAIL | r10=&AccountId, r11=&Name, r12=&Json | u64=0 | asset:gas/G_set_detail@ivm.core/v1 + bytes(val) |
| 0x20 | REGISTER_ASSET | r10=&AssetDefinitionId | u64=0 | asset:gas/G_reg_asset@ivm.core/v1 |
| 0x21 | UNREGISTER_ASSET | r10=&AssetDefinitionId | u64=0 | asset:gas/G_unreg_asset@ivm.core/v1 |
| 0x22 | MINT_ASSET | r10=&AccountId, r11=&AssetDefinitionId, r12=amount:u64 | u64=0 | asset:gas/G_mint@ivm.core/v1 |
| 0x23 | BURN_ASSET | r10=&AccountId, r11=&AssetDefinitionId, r12=amount:u64 | u64=0 | asset:gas/G_burn@ivm.core/v1 |
| 0x24 | TRANSFER_ASSET | r10=&AccountId(from), r11=&AccountId(to), r12=&AssetDefinitionId, r13=amount:u64 | u64=0 | asset:gas/G_transfer@ivm.core/v1 |
| 0x25 | NFT_MINT_ASSET | r10=&NftId, r11=&AccountId(owner) | u64=0 | asset:gas/G_nft_mint_asset@ivm.core/v1 |
| 0x26 | NFT_TRANSFER_ASSET | r10=&AccountId(from), r11=&NftId, r12=&AccountId(to) | u64=0 | asset:gas/G_nft_transfer_asset@ivm.core/v1 |
| 0x27 | NFT_SET_METADATA | r10=&NftId, r11=&Json | u64=0 | asset:gas/G_nft_set_metadata@ivm.core/v1 |
| 0x28 | NFT_BURN_ASSET | r10=&NftId | u64=0 | asset:gas/G_nft_burn_asset@ivm.core/v1 |
| 0x29 | TRANSFER_V1_BATCH_BEGIN | - | u64=0 | - |
| 0x2A | TRANSFER_V1_BATCH_END | - | u64=0 | - |
| 0x2B | TRANSFER_V1_BATCH_APPLY | - | u64=0 | - |
| 0x30 | CREATE_ROLE | r10=&Name, r11=&Json(perms) | u64=0 | asset:gas/G_create_role@ivm.core/v1 |
| 0x31 | DELETE_ROLE | r10=&Name | u64=0 | asset:gas/G_delete_role@ivm.core/v1 |
| 0x32 | GRANT_ROLE | r10=&AccountId, r11=&Name | u64=0 | asset:gas/G_grant_role@ivm.core/v1 |
| 0x33 | REVOKE_ROLE | r10=&AccountId, r11=&Name | u64=0 | asset:gas/G_revoke_role@ivm.core/v1 |
| 0x34 | GRANT_PERMISSION | r10=&AccountId, r11=&Name | u64=0 | asset:gas/G_grant_perm@ivm.core/v1 |
| 0x35 | REVOKE_PERMISSION | r10=&AccountId, r11=&Name | u64=0 | asset:gas/G_revoke_perm@ivm.core/v1 |
| 0x40 | CREATE_TRIGGER | r10=&Json(spec) | u64=0 | asset:gas/G_create_trig@ivm.core/v1 |
| 0x41 | REMOVE_TRIGGER | r10=&Name | u64=0 | asset:gas/G_remove_trig@ivm.core/v1 |
| 0x42 | SET_TRIGGER_ENABLED | r10=&Name, r11=enabled:u64 | u64=0 | asset:gas/G_set_trig@ivm.core/v1 |
| 0x43 | DEACTIVATE_CONTRACT_INSTANCE | - | u64=0 | - |
| 0x44 | REMOVE_SMART_CONTRACT_BYTES | - | u64=0 | - |
| 0x45 | REGISTER_SMART_CONTRACT_CODE | - | u64=0 | - |
| 0x46 | REGISTER_SMART_CONTRACT_BYTES | - | u64=0 | - |
| 0x47 | ACTIVATE_CONTRACT_INSTANCE | - | u64=0 | - |
| 0x50 | STATE_GET | - | u64=0 | - |
| 0x51 | STATE_SET | - | u64=0 | - |
| 0x52 | STATE_DEL | - | u64=0 | - |
| 0x53 | DECODE_INT | - | u64=0 | - |
| 0x54 | BUILD_PATH_MAP_KEY | - | u64=0 | - |
| 0x55 | ENCODE_INT | - | u64=0 | - |
| 0x56 | BUILD_PATH_KEY_NORITO | - | u64=0 | - |
| 0x57 | JSON_ENCODE | r10=&Json | ptr (&NoritoBytes) | asset:gas/G_json_encode@ivm.core/v1 |
| 0x58 | JSON_DECODE | r10=&NoritoBytes(JSON bytes) | ptr (&Json) | asset:gas/G_json_decode@ivm.core/v1 |
| 0x59 | SCHEMA_ENCODE | r10=&Name(schema), r11=&Json | ptr (&NoritoBytes) | - |
| 0x5A | SCHEMA_DECODE | r10=&Name(schema), r11=&NoritoBytes | ptr (&Json) | - |
| 0x5B | SCHEMA_INFO | r10=&Name(schema) | ptr (&Json{"id":...,"version":...}) | - |
| 0x5C | NAME_DECODE | r10=&NoritoBytes(UTF-8 string) | ptr (&Name) | asset:gas/G_name_decode@ivm.core/v1 |
| 0x5D | POINTER_TO_NORITO | r10=&PointerType<T> | ptr (&NoritoBytes(TLV envelope)) | - |
| 0x5E | POINTER_FROM_NORITO | r10=&NoritoBytes(TLV envelope), r11=expected?:u16 | ptr (&PointerType<T>) | - |
| 0x5F | TLV_EQ | r10=&Tlv, r11=&Tlv | r10=1/0 | - |
| 0x60 | ZK_VERIFY_TRANSFER | r10=&NoritoBytes(OpenVerifyEnvelope) | u64=0/1 | asset:gas/G_verify_proof@ivm.core/v1 |
| 0x61 | ZK_VERIFY_UNSHIELD | r10=&NoritoBytes(OpenVerifyEnvelope) | u64=0/1 | asset:gas/G_verify_proof@ivm.core/v1 |
| 0x62 | ZK_VOTE_VERIFY_BALLOT | r10=&NoritoBytes(OpenVerifyEnvelope) | u64=0/1 | asset:gas/G_verify_proof@ivm.core/v1 |
| 0x63 | ZK_VOTE_VERIFY_TALLY | r10=&NoritoBytes(OpenVerifyEnvelope) | u64=0/1 | asset:gas/G_verify_proof@ivm.core/v1 |
| 0x64 | ZK_ROOTS_GET | r10=&NoritoBytes(RootsGetRequest) | ptr (NoritoBytes in INPUT) | asset:gas/G_roots_get@ivm.core/v1 |
| 0x65 | ZK_VOTE_GET_TALLY | r10=&NoritoBytes(VoteGetTallyRequest) | ptr (NoritoBytes in INPUT) | asset:gas/G_vote_get@ivm.core/v1 |
| 0x66 | VRF_VERIFY | r10=&NoritoBytes(VrfVerifyRequest) | r10=ptr (&Blob(32-byte output)), r11=status:u64 | asset:gas/G_verify@ivm.core/v1 |
| 0x67 | VRF_VERIFY_BATCH | r10=&NoritoBytes(VrfVerifyBatchRequest) | r10=ptr (&NoritoBytes(Vec<[u8;32]>)), r11=status:u64, r12=fail_index?:u64 | asset:gas/G_verify@ivm.core/v1 |
| 0x68 | ZK_VERIFY_BATCH | r10=&NoritoBytes(Vec<OpenVerifyEnvelope>) | r10=ptr (&NoritoBytes(Vec<u8> statuses)), r11=status:u64 | asset:gas/G_verify@ivm.core/v1 |
| 0x90 | SM3_HASH | - | u64=0 | - |
| 0x91 | SM2_VERIFY | - | u64=0/1 | asset:gas/G_verify@ivm.core/v1 |
| 0x92 | SM4_GCM_SEAL | - | u64=0 | - |
| 0x93 | SM4_GCM_OPEN | - | u64=0 | - |
| 0x94 | SM4_CCM_SEAL | - | u64=0 | - |
| 0x95 | SM4_CCM_OPEN | - | u64=0 | - |
| 0xA0 | SMARTCONTRACT_EXECUTE_INSTRUCTION | r10=&Json | u64=0 | asset:gas/G_sci@ivm.core/v1 |
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY | r10=&Json | ptr (r10) | asset:gas/G_scq@ivm.core/v1 |
| 0xA2 | CREATE_NFTS_FOR_ALL_USERS | - | u64=count | asset:gas/G_create_nfts_all@ivm.core/v1 |
| 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | r10=depth:u64 | u64=prev | asset:gas/G_sc_depth@ivm.core/v1 |
| 0xA4 | GET_AUTHORITY | - | ptr (AccountId in INPUT) | asset:gas/G_get_auth@ivm.core/v1 |
| 0xB0 | AXT_BEGIN | - | u64=0 | - |
| 0xB1 | AXT_TOUCH | - | u64=0 | - |
| 0xB2 | AXT_COMMIT | - | u64=0 | - |
| 0xB3 | VERIFY_DS_PROOF | - | u64=0/1 | asset:gas/G_verify@ivm.core/v1 |
| 0xB4 | USE_ASSET_HANDLE | - | u64=0 | - |
| 0xE0 | INPUT_PUBLISH_TLV | r10=&Blob(TLV) | ptr (r10) | asset:gas/G_input_publish@ivm.core/v1 |
| 0xF0 | ALLOC | r10=bytes:u64 | ptr (r10) | asset:gas/G_alloc@ivm.core/v1 + bytes |
| 0xF1 | GET_PUBLIC_INPUT | r10=&Name | ptr (r10) | asset:gas/G_get_pub@ivm.core/v1 |
| 0xF4 | PROVE_EXECUTION | - | r10=0/1 | asset:gas/G_prove@ivm.core/v1 |
| 0xF5 | GROW_HEAP | r10=bytes:u64 | u64=new_limit | asset:gas/G_grow_heap@ivm.core/v1 per page |
| 0xF6 | VERIFY_PROOF | - | r10=0/1 | asset:gas/G_verify@ivm.core/v1 |
| 0xF7 | GET_MERKLE_PATH | r10=addr:u64, r11=out:u64, r12=root_out?:u64 | u64=len | asset:gas/G_mpath@ivm.core/v1 + len |
| 0xF9 | GET_ACCOUNT_BALANCE | r10=&AccountId, r11=&AssetDefinitionId | u64=amount | asset:gas/G_get_bal@ivm.core/v1 |
| 0xFA | GET_MERKLE_COMPACT | r10=addr, r11=out, r12=depth_cap?, r13=root_out? | u64=depth | asset:gas/G_mpath@ivm.core/v1 + depth |
| 0xFB | USE_NULLIFIER | r10=nullifier:u64 | u64=0 | asset:gas/G_use_null@ivm.core/v1 |
| 0xFD | GET_PRIVATE_INPUT | r10=index:u64 | r10=value | asset:gas/G_get_priv@ivm.core/v1 |
| 0xFE | COMMIT_OUTPUT | - | u64=0 | asset:gas/G_commit@ivm.core/v1 |
| 0xFF | GET_REGISTER_MERKLE_COMPACT | r10=reg, r11=out, r12=depth_cap?, r13=root_out? | u64=depth | asset:gas/G_mpath@ivm.core/v1 + depth |
<!-- END GENERATED SYSCALLS -->













Codec helpers
- 0x57 JSON_ENCODE — Args: `r10=&Json` → Return: `ptr (&NoritoBytes)` — Gas: G_json_encode
- 0x58 JSON_DECODE — Args: `r10=&NoritoBytes(JSON bytes)` → Return: `ptr (&Json)` — Gas: G_json_decode
- 0x5F TLV_EQ — Args: `r10=&Tlv, r11=&Tlv` → Return: `r10=1 if equal else 0` — Gas: -
- 0x5C NAME_DECODE — Args: `r10=&NoritoBytes(UTF‑8 string)` → Return: `ptr (&Name)` — Gas: G_name_decode
- 0x5D POINTER_TO_NORITO — Args: `r10=&PointerType<T>` → Return: `ptr (&NoritoBytes(TLV envelope))`
- 0x5E POINTER_FROM_NORITO — Args: `r10=&NoritoBytes(TLV envelope), r11=expected?:u16` → Return: `ptr (&PointerType<T>)`
ZK (Halo2 OpenVerify)
- 0x68 ZK_VERIFY_BATCH — Args: `r10=&NoritoBytes(Vec<OpenVerifyEnvelope>)` → Return: `r10=ptr (&NoritoBytes(Vec<u8> statuses))`, `r11=status:u64` — Gas: G_verify
  - Verifies a batch of Halo2 OpenVerify envelopes (transparent IPA). Per‑item statuses are `1 = verified true`, `0 = verified false` (including verification failure or precheck gating).
  - Host gating: disabled/backend/curve/max_k checks apply uniformly to each item. Overall request failures (e.g., disabled host or batch size exceeds `verifier_max_batch`) are signaled via `r11` with no output pointer:
    - `1 = ERR_DISABLED`, `2 = ERR_BACKEND`, `3 = ERR_CURVE`, `4 = ERR_K`, `5 = ERR_DECODE`, `7 = ERR_BATCH`.
  - Types: `OpenVerifyEnvelope` is the Norito-serializable container defined in `iroha_zkp_halo2` with fields `{ params: IpaParams, public: PolyOpenPublic, proof: IpaProofData, transcript_label: String }`.
