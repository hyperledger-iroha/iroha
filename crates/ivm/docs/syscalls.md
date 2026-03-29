//! IVM Syscall Table (ABI v1)

This document lists IVM syscall numbers and their ABI for `abi_version = 1`.
The host and VM enforce the fixed first-release ABI v1 policy: the set of
available syscalls is fixed and unknown or disallowed numbers must be rejected
with `E_SCALL_UNKNOWN` (mapped to `VMError::UnknownSyscall`). The canonical
policy is centralized in `ivm::syscalls::is_syscall_allowed(policy, number)`.

ABI policy
- V1 (1): allows the canonical ABI surface listed here and in `abi_syscall_list()`; unknown numbers
  are rejected uniformly across all hosts. The list is kept sorted/deduplicated and the golden test
  fails if ordering or contents drift.
- First release: ABI v1 is the only supported policy. `abi_version != 1` is rejected at admission,
  and runtime upgrades must keep `abi_version = 1` without expanding the syscall or pointer‑ABI surface.

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

Query syscall (Norito)
- `0xA1` expects `r10=&NoritoBytes(QueryRequest)` and returns `r10=&NoritoBytes(QueryResponse)`. The authority is always the calling contract; embedded authorities are ignored.
- Iterable queries run in ephemeral cursor mode inside IVM; `QueryRequest::Continue` is rejected to keep query lifetimes bound to the VM run.
- `pipeline.query_max_fetch_size` caps iterable query `fetch_size` for IVM query syscalls (0 clamps to 1). Torii endpoints continue to use `torii.app_api.max_fetch_size`.
- Gas is `base + per_item + per_byte`, with per-item cost multiplied when sorting is requested and an offset penalty applied for large pagination skips.

Vendor syscall (Norito)
- `0xA0` expects `r10=&NoritoBytes(InstructionBox)` to enqueue a built-in instruction.

Examples (dev envelopes; mock WSV host only)
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

Gas enforcement (CoreHost)
- ISI syscalls charge extra gas using the native ISI schedule (`iroha_core::gas::meter_instruction`).
- FASTPQ transfer batches are charged per entry (same as individual transfers).
- ZK_VERIFY syscalls reuse the confidential verification gas schedule (base + proof size).
- GET_PUBLIC_INPUT charges a base plus a per-byte cost based on the returned TLV length.
- SMARTCONTRACT_EXECUTE_QUERY charges base + per-item + per-byte; sorting multiplies per-item cost. Pagination offsets add an extra per-item penalty for unsorted queries; for sorted queries, the per-item charge is based on all items scanned before pagination (so offsets are already included). Query materialization aborts with OutOfGas when the per-item budget is exhausted, and responses that exceed the per-byte budget are rejected before encoding when exact Norito sizing is available (otherwise after encoding).

Lifecycle / Utility
- 0x00 DEBUG_PRINT — Args: `r10=value:u64` → Return: 0 — Gas: G_debug
- 0x01 EXIT — Args: `r10=status:u64` → Return: `u64=status` — Gas: G_exit
- 0x02 ABORT — Args: none → Return: `u64=0` — Gas: G_abort (halts and marks the run failed)
- 0x03 DEBUG_LOG — Args: `r10=&Json|&Blob|&NoritoBytes` → Return: 0 — Gas: G_debug
- 0xA8 CURRENT_TIME_MS — Args: none → Return: `u64=unix_time_ms` — Gas: G_read_time
- 0xE0 INPUT_PUBLISH_TLV — Args: `r10=&Blob(TLV)` → Return: `ptr (r10)` — Gas: G_input_publish (rejects invalid TLV envelopes and disallowed pointer types)
- 0x90 SM3_HASH — Args: `r10=&Blob(message)` → Return: `ptr (&Blob(digest))` — Gas: -
- 0x91 SM2_VERIFY — Args: `r10=&Blob(msg)`, `r11=&Blob(sig)` (64-byte r∥s), `r12=&Blob(pubkey)` (SEC1), `r13=&Blob(distid)` *(optional, 0 for default)* → Return: `u64=0/1` — Gas: G_verify
- 0x92 SM4_GCM_SEAL — Args: `r10=&Blob(key16)`, `r11=&Blob(nonce12)`, `r12=&Blob(aad)` *(0 => empty)*, `r13=&Blob(plaintext)` → Return: `ptr (&Blob(ciphertext || tag16))` — Gas: -
- 0x93 SM4_GCM_OPEN — Args: `r10=&Blob(key16)`, `r11=&Blob(nonce12)`, `r12=&Blob(aad)` *(0 => empty)*, `r13=&Blob(ciphertext || tag16)` → Return: `ptr (&Blob(plaintext))` or `0` on failure — Gas: -
- 0x94 SM4_CCM_SEAL — Args: `r10=&Blob(key16)`, `r11=&Blob(nonce[7..13])`, `r12=&Blob(aad)` *(0 => empty)*, `r13=&Blob(plaintext)`, `r14=tag_len:u64` *(0 => 16)* → Return: `ptr (&Blob(ciphertext || tag))` — Gas: -
- 0x95 SM4_CCM_OPEN — Args: `r10=&Blob(key16)`, `r11=&Blob(nonce[7..13])`, `r12=&Blob(aad)` *(0 => empty)*, `r13=&Blob(ciphertext || tag)`, `r14=tag_len:u64` *(0 => 16)* → Return: `ptr (&Blob(plaintext))` or `0` on failure — Gas: -
- 0xF1 GET_PUBLIC_INPUT — Args: `r10=&Name` → Return: `ptr (&Tlv)` — Gas: G_get_pub + bytes
  - Reads a public input by name from the on-chain registry `Parameters.custom["ivm_public_inputs"]`.
  - Registry entries are JSON objects: `{ "name": "<Name>", "type_id": <u16>, "tlv_hex": "<hex>" }` with optional `gas_base`/`gas_per_byte` (`tlv_hex` is the full TLV envelope; `0x` prefix allowed).
  - Missing names return `PermissionDenied`; malformed name TLVs or ABI-disallowed types raise syscall errors. Invalid registry entries are skipped during host hydration.
- 0xFD GET_PRIVATE_INPUT — Args: `r10=index:u64` → Return: `r10=value` — Gas: G_get_priv
- 0xFE COMMIT_OUTPUT — Args: none → Return: `u64=0` — Gas: G_commit

For the SM4 calls, the host appends the authentication tag to the ciphertext output; callers supply the same layout when invoking the corresponding `OPEN` syscall. `SM4_GCM_*` always uses a 16-byte tag and 12-byte nonce. `SM4_CCM_*` accepts nonce lengths between 7 and 13 bytes and tag sizes {4,6,8,10,12,14,16}; pass the desired tag length in `r14` (use `0` to select 16). Passing `0` in `r12` denotes an empty AAD.

Kotodama intrinsics
- ``sm::hash(msg: Blob) -> Blob`` mirrors `msg` into INPUT with `INPUT_PUBLISH_TLV` and issues `SM3_HASH`, returning a pointer to the digest Blob.
- ``sm::verify(msg: Blob, sig: Blob, pk: Blob[, distid: Blob]) -> bool`` mirrors each Blob argument into INPUT, invokes `SM2_VERIFY`, and returns `true` for valid signatures. Omitting the fourth argument selects the runtime-configured default (``Sm2PublicKey::default_distid()``, sourced from `crypto.sm2_distid_default`); providing it enforces a custom distinguishing identifier.
- ``current_time_ms() -> int`` issues `CURRENT_TIME_MS` and returns the host-provided Unix epoch time in milliseconds. `CoreHost` binds this to block time; test/default hosts use their configured or wall-clock time.

Numeric helpers (Norito)
- 0x69 NUMERIC_FROM_INT — Args: `r10=value:i64` (non‑negative) → `r10=&NoritoBytes(Numeric)` (scale = 0).
- 0x6A NUMERIC_TO_INT — Args: `r10=&NoritoBytes(Numeric)` → `r10=value:i64`. Rejects negative values, fractional scales, or values outside `i64`.
- 0x6B..0x70 NUMERIC_{ADD,SUB,MUL,DIV,REM,NEG} — Args: `r10=&NoritoBytes(lhs)`, `r11=&NoritoBytes(rhs)` (NEG uses `r10` only) → `r10=&NoritoBytes(result)`. Inputs must be unsigned with scale = 0; SUB rejects underflow and NEG rejects non‑zero values. DIV/REM reject division by zero.
- 0x71..0x76 NUMERIC_{EQ,NE,LT,LE,GT,GE} — Args: `r10=&NoritoBytes(lhs)`, `r11=&NoritoBytes(rhs)` → `r10=0/1` with the comparison result (inputs must be unsigned scale = 0).
- Kotodama numeric aliases (`fixed_u128`, `Amount`, `Balance`) lower to these syscalls for deterministic unsigned, scale‑0 arithmetic.

Domains / Peers
- 0x10 REGISTER_DOMAIN — Args: `r10=&DomainId` → 0 — Gas: G_reg_domain
- 0x11 UNREGISTER_DOMAIN — Args: `r10=&DomainId` → 0 — Gas: G_unreg_domain
- 0x12 TRANSFER_DOMAIN — Args: `r10=&DomainId, r11=&AccountId` → 0 — Gas: G_xfer_domain
- 0x15 REGISTER_PEER — Args: `r10=&Json` (RegisterPeerWithPop) → 0 — Gas: G_reg_peer
  - JSON object: `{ "peer": "<public_key or public_key@addr>", "pop": [..], "activation_at": <u64?>, "expiry_at": <u64?>, "hsm": <HsmBinding?> }`
  - `peer` may be a string or an object with `public_key`/`publicKey`/`peer_id`/`peerId`/`key`; those keys are also accepted at top level.
- 0x16 UNREGISTER_PEER — Args: `r10=&Json` (peer id string or object with `peer`/`peer_id`/`peerId`/`public_key`/`publicKey`/`key`) → 0 — Gas: G_unreg_peer

Accounts
- 0x13 REGISTER_ACCOUNT — Args: `r10=&ScopedAccountId` → 0 — Gas: G_reg_acct
- 0x14 UNREGISTER_ACCOUNT — Args: `r10=&AccountId` → 0 — Gas: G_unreg_acct
- 0x17 ADD_SIGNATORY — Args: `r10=&AccountId, r11=&Json` (pubkey string or object with `public_key`/`publicKey`/`key`) → 0 — Gas: G_add_sig
- 0x18 REMOVE_SIGNATORY — Args: `r10=&AccountId, r11=&Json` (pubkey string or object with `public_key`/`publicKey`/`key`) → 0 — Gas: G_rm_sig
- 0x19 SET_ACCOUNT_QUORUM — Args: `r10=&AccountId, r11=quorum:u64` → 0 — Gas: G_set_quorum
- 0x1A SET_ACCOUNT_DETAIL — Args: `r10=&AccountId, r11=&Name, r12=&Json` → 0 — Gas: G_set_detail + bytes(val)

Notes:
- Signatory/quorum syscalls update the multisig spec stored in account metadata key `multisig/spec`.
  The target account is selected by canonical `AccountId`; signatory accounts must exist and the
  resulting spec must remain acyclic with quorum reachable.
- These syscalls update multisig roles and metadata and rekey the account controller to the
  canonical multisig id derived from the spec (signatories must be single-key accounts).

Assets (FT)
- 0x20 REGISTER_ASSET — Args: `r10=&AssetDefinitionId` → 0 — Gas: G_reg_asset
- 0x21 UNREGISTER_ASSET — Args: `r10=&AssetDefinitionId` → 0 — Gas: G_unreg_asset
- 0x22 MINT_ASSET — Args: `r10=&AccountId, r11=&AssetDefinitionId, r12=&NoritoBytes(Numeric)` → 0 — Gas: G_mint
- 0x23 BURN_ASSET — Args: `r10=&AccountId, r11=&AssetDefinitionId, r12=&NoritoBytes(Numeric)` → 0 — Gas: G_burn
- 0x24 TRANSFER_ASSET — Args: `r10=&AccountId(from), r11=&AccountId(to), r12=&AssetDefinitionId, r13=&NoritoBytes(Numeric)` → 0 — Gas: G_transfer

NFTs
- 0x25 NFT_MINT_ASSET — Args: `r10=&NftId, r11=&AccountId(owner)` → 0 — Gas: G_nft_mint_asset
- 0x26 NFT_TRANSFER_ASSET — Args: `r10=&AccountId(from), r11=&NftId, r12=&AccountId(to)` → 0 — Gas: G_nft_transfer_asset
- 0x27 NFT_SET_METADATA — Args: `r10=&NftId, r11=&Json` → 0 — Gas: G_nft_set_metadata
- 0x28 NFT_BURN_ASSET — Args: `r10=&NftId` → 0 — Gas: G_nft_burn_asset

Zero‑knowledge (verification/state‑read)
- 0x60 ZK_VERIFY_TRANSFER — Args: `r10=&NoritoBytes(iroha_data_model::zk::OpenVerifyEnvelope)` → `u64=0/1` — Gas: G_verify_proof
- 0x61 ZK_VERIFY_UNSHIELD — Args: `r10=&NoritoBytes(iroha_data_model::zk::OpenVerifyEnvelope)` → `u64=0/1` — Gas: G_verify_proof
- 0x62 ZK_VOTE_VERIFY_BALLOT — Args: `r10=&NoritoBytes(iroha_data_model::zk::OpenVerifyEnvelope)` → `u64=0/1` — Gas: G_verify_proof
- 0x63 ZK_VOTE_VERIFY_TALLY — Args: `r10=&NoritoBytes(iroha_data_model::zk::OpenVerifyEnvelope)` → `u64=0/1` — Gas: G_verify_proof
- 0x64 ZK_ROOTS_GET — Args: `r10=&NoritoBytes(RootsGetRequest)` → `ptr (NoritoBytes(RootsGetResponse))` — Gas: G_roots_get
- 0x65 ZK_VOTE_GET_TALLY — Args: `r10=&NoritoBytes(VoteGetTallyRequest)` → `ptr (NoritoBytes(VoteGetTallyResponse))` — Gas: G_vote_get

ZK gating & determinism
- `CoreHost` performs full proof verification through the configured backend verifier (`iroha_core::zk::verify_backend_with_timing`), not the legacy polynomial-opening helper.
- Verification is bound to the VK registry before cryptographic checks:
  - envelope/backend must be supported (`backend = halo2-ipa-pasta`), `vk_hash` must be present, and payload/proof sizes must respect config caps.
  - the referenced verifying key must be active and match circuit id, schema hash (`hash(public_inputs)`), namespace, and owner manifest.
  - configured curve/max_k policy is enforced from VK metadata / VK envelope parameters.
- Return conventions:
  - `r10=1`, `r11=0` on success.
  - `r10=0`, `r11=<ERR_*>` on precheck/binding failure (`ERR_DISABLED`, `ERR_BACKEND`, `ERR_CURVE`, `ERR_K`, `ERR_DECODE`, `ERR_VERIFY`, `ERR_ENVELOPE_SIZE`, `ERR_PROOF_LEN`, `ERR_VK_MISSING`, `ERR_VK_MISMATCH`, `ERR_VK_INACTIVE`, `ERR_NAMESPACE`).
- `DefaultHost` does not implement end-to-end ZK verification for these syscalls and reports disabled (`r10=0`, `r11=ERR_DISABLED`).

Roles / Permissions
- 0x30 CREATE_ROLE — Args: `r10=&Name, r11=&Json` (perm set) → 0 — Gas: G_create_role
  - Permissions JSON: array of permission strings/objects or `{ "permissions": [...] }` / `{ "perms": [...] }`.
- 0x31 DELETE_ROLE — Args: `r10=&Name` → 0 — Gas: G_delete_role
- 0x32 GRANT_ROLE — Args: `r10=&AccountId, r11=&Name` → 0 — Gas: G_grant_role
- 0x33 REVOKE_ROLE — Args: `r10=&AccountId, r11=&Name` → 0 — Gas: G_revoke_role
- 0x34 GRANT_PERMISSION — Args: `r10=&AccountId, r11=&Name|&Json(Permission)` → 0 — Gas: G_grant_perm
- 0x35 REVOKE_PERMISSION — Args: `r10=&AccountId, r11=&Name|&Json(Permission)` → 0 — Gas: G_revoke_perm

Triggers
- 0x40 CREATE_TRIGGER — Args: `r10=&Json` (trigger spec) → 0 — Gas: G_create_trig
  - Spec payloads:
    - JSON string: base64 Norito-encoded `Trigger` (canonical).
    - JSON object: `{ "id": "<trigger_id>", "action": <ActionSpec> }` where `action` is either a
      base64 Norito `Action` string or a JSON object with `executable`, `repeats`, `authority`,
      `filter`, and `metadata` fields (matching `SpecializedAction<EventFilterBox>`).
    - `EventFilterBox::TriggerCompleted` filters are rejected for triggering actions.
- 0x41 REMOVE_TRIGGER — Args: `r10=&Name` → 0 — Gas: G_remove_trig
- 0x42 SET_TRIGGER_ENABLED — Args: `r10=&Name, r11=enabled:u64` → 0 — Gas: G_set_trig
  - Writes trigger metadata key `__enabled` to `true`/`false`; missing key defaults to enabled.
- 0x43 DEACTIVATE_CONTRACT_INSTANCE — Args: `r10=&NoritoBytes(DeactivateContractInstance)` → 0 — Gas: -
- 0x44 REMOVE_SMART_CONTRACT_BYTES — Args: `r10=&NoritoBytes(RemoveSmartContractBytes)` → 0 — Gas: -
- 0x45 REGISTER_SMART_CONTRACT_CODE — Args: `r10=&NoritoBytes(RegisterSmartContractCode)` → 0 — Gas: -
- 0x46 REGISTER_SMART_CONTRACT_BYTES — Args: `r10=&NoritoBytes(RegisterSmartContractBytes)` → 0 — Gas: -
- 0x47 ACTIVATE_CONTRACT_INSTANCE — Args: `r10=&NoritoBytes(ActivateContractInstance)` → 0 — Gas: -

Lifecycle operations expect canonical Norito encodings of the corresponding ISI structs. Hosts
trim empty `reason` strings for `DeactivateContractInstance`/`RemoveSmartContractBytes` and
enforce governance permissions before queuing the instruction.

Smart‑contract helpers (Norito)
- 0xA0 EXECUTE_INSTRUCTION — Args: `r10=&NoritoBytes(InstructionBox)` → 0 — Gas: G_sci
- 0xA5 SUBSCRIPTION_BILL — Args: none → 0 — Gas: G_sub_bill
  - Uses trigger metadata `subscription_ref` to locate the subscription NFT, computes charges, updates subscription metadata (including `subscription_invoice`), and reschedules the billing trigger.
- 0xA6 SUBSCRIPTION_RECORD_USAGE — Args: none → 0 — Gas: G_sub_usage
  - Parses `SubscriptionUsageDelta` from trigger args, increments usage counters, and updates subscription metadata.

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
    - `{"type":"zk.Shield","payload":{"asset":"<asset-definition-id>","from":"<account>","amount":3,"note_commitment":[7,...,7],"enc_payload":{"version":1,"ephemeral_pubkey":[0,...,0],"nonce":[0,...,0],"ciphertext":""}}}`
  - Mint asset:
    - `{"type":"wsv.mint_asset","payload":{"account_id":"<account>","asset_id":"<base58-asset-definition-id>","amount":100}}`
- Notes:
  - The JSON envelope is intended for tests and developer tooling; production smart‑contracts should prefer Norito TLVs generated by the compiler.
  - Public asset ids are bare Base58 asset-definition ids. Internal balance buckets may bind asset and account state together, but those are not public `asset_id` values.
  - The host enforces the same permission checks as the dedicated syscalls (`MINT_ASSET`, `BURN_ASSET`, etc.).
- 0xA1 EXECUTE_QUERY — Args: `r10=&NoritoBytes(QueryRequest)` → `ptr` — Gas: G_scq
- 0xA2 CREATE_NFTS_FOR_ALL_USERS — Args: none → `u64=count` — Gas: G_create_nfts_all
- 0xA3 SET_SMARTCONTRACT_EXECUTION_DEPTH — Args: `r10=depth:u64` → `u64=prev` — Gas: G_sc_depth
- 0xA4 GET_AUTHORITY — Args: none → `ptr` (AccountId in INPUT, `r10` points to it) — Gas: G_get_auth
- 0xA7 RESOLVE_ACCOUNT_ALIAS — Args: `r10=&Blob(alias literal)` → `ptr` (AccountId in INPUT, `r10` points to it) — Gas: G_alias_resolve

AXT host flow
- 0xB0 AXT_BEGIN — Args: `r10=&AxtDescriptor`. Resets any in‑progress envelope and records the descriptor; hosts derive the canonical binding used by capability handles from this descriptor.
- 0xB1 AXT_TOUCH — Args: `r10=&DataSpaceId`, `r11=&NoritoBytes(TouchManifest)` or `0`. Declares the manifest of keys touched for the dataspace within the current envelope.
- 0xB2 AXT_COMMIT — Args: none. Validates recorded handles, manifests, and proofs for the active envelope and clears host state on success.
- 0xB3 VERIFY_DS_PROOF — Args: `r10=&DataSpaceId`, `r11=&ProofBlob` (or `0` to clear). Associates dataspace proof material with the active envelope.
- 0xB4 USE_ASSET_HANDLE — Args: `r10=&AssetHandle`, `r11=&NoritoBytes(RemoteSpendIntent)`, `r12=&ProofBlob` (optional). Validates capability bindings/budgets and records spend intents for later commit checks.
- Default and WSV hosts enforce descriptor membership, capability binding equality, budget checks, and proof presence before permitting commit.

Soracloud runtime host surface
- 0xC0 SORACLOUD_READ_COMMITTED_STATE — Args: `r10=&SoracloudRequest(ReadCommittedState)` → `r10=&SoracloudResponse(ReadCommittedState)`. Returns committed service-state metadata for one declared binding/key pair.
- 0xC1 SORACLOUD_EMIT_STATE_MUTATION — Args: `r10=&SoracloudRequest(EmitStateMutation)` → `r10=&SoracloudResponse(EmitStateMutation)`. Stages a deterministic write-back validated again by core before persistence.
- 0xC2 SORACLOUD_EMIT_MAILBOX_MESSAGE — Args: `r10=&SoracloudRequest(EmitMailboxMessage)` → `r10=&SoracloudResponse(EmitMailboxMessage)`. Emits an outbound mailbox message for authoritative queueing after receipt validation.
- 0xC3 SORACLOUD_APPEND_JOURNAL — Args: `r10=&SoracloudRequest(AppendJournal)` → `r10=&SoracloudResponse(AppendJournal)`. Stages deterministic journal material and returns its content hash.
- 0xC4 SORACLOUD_PUBLISH_CHECKPOINT — Args: `r10=&SoracloudRequest(PublishCheckpoint)` → `r10=&SoracloudResponse(PublishCheckpoint)`. Stages checkpoint material and returns its content hash.
- 0xC5 SORACLOUD_READ_SECRET — Args: `r10=&SoracloudRequest(ReadSecret)` → `r10=&SoracloudResponse(ReadSecret)`. Reads node-local secret material and is only valid from `private_update`.
- 0xC6 SORACLOUD_READ_CREDENTIAL — Args: `r10=&SoracloudRequest(ReadCredential)` → `r10=&SoracloudResponse(ReadCredential)`. Reads node-local credential material and is only valid from `private_update`.
- 0xC7 SORACLOUD_EGRESS_FETCH — Args: `r10=&SoracloudRequest(EgressFetch)` → `r10=&SoracloudResponse(EgressFetch)`. Performs a bounded host-allowlisted fetch and fails deterministically on policy or hash mismatch.
- 0xC8 SORACLOUD_READ_CONFIG — Args: `r10=&SoracloudRequest(ReadConfig)` → `r10=&SoracloudResponse(ReadConfig)`. Reads authoritative service config payload bytes for the active revision and is valid from ordinary Soracloud handlers.
- 0xC9 SORACLOUD_READ_SECRET_ENVELOPE — Args: `r10=&SoracloudRequest(ReadSecretEnvelope)` → `r10=&SoracloudResponse(ReadSecretEnvelope)`. Reads the authoritative committed secret envelope for the active revision and is valid from ordinary Soracloud handlers; plaintext/ciphertext payload reads remain on the private-runtime `READ_SECRET` path.
- All Soracloud payloads are Norito request/response envelopes carried in the Soracloud pointer-ABI types. Host failures must be deterministic and receipt-stable, and unknown numbers still map to `VMError::UnknownSyscall`.

ZK Helpers
- 0xF9 GET_ACCOUNT_BALANCE — Args: `r10=&AccountId, r11=&AssetDefinitionId` → `ptr (&NoritoBytes(Numeric))` — Gas: G_get_bal
- 0xFB USE_NULLIFIER — Args: `r10=nullifier:u64` → `u64=0` — Gas: G_use_null
- 0xFC VERIFY_SIGNATURE — Args: `r10=&Blob(message)`, `r11=&Blob(signature)`, `r12=&Blob(pubkey)`, `r13=scheme:u8` → `r10=0/1` — Gas: G_verify_sig

Hardware / Proofs
- 0xF4 PROVE_EXECUTION — Args: none → `NotImplemented` (reserved for future end-to-end proving integration) — Gas: G_prove
- 0xF5 GROW_HEAP — Args: `r10=bytes:u64` → `u64=new_limit` — Gas: G_grow_heap per page
- 0xF6 VERIFY_PROOF — Args: none → `NotImplemented` (reserved for future end-to-end execution-proof verification) — Gas: -
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

- 0x7E VRF_EPOCH_SEED — Args: `r10=&NoritoBytes(VrfEpochSeedRequest{epoch:u64, fallback_to_latest:bool})` → Return: `r10=ptr (&NoritoBytes(VrfEpochSeedResponse{found:bool, epoch:u64, seed:[u8;32]}))`, `r11=status:u64` — Gas: G_vote_get
  - Reads a world-snapshot VRF epoch seed for governance/sortition use in smart contracts.
  - If `fallback_to_latest=true` and the requested epoch is missing, the host returns the latest known epoch seed.
  - Status codes: `0=ok`, `1=type_mismatch`, `2=decode_error`, `3=oom`.

Host gating & chain binding
- When a host `chain_id` is configured, requests must match it. Otherwise:
  - Single: `r11=8 (chain_mismatch)` and `r10=0`.
  - Batch: `r11=8`, `r12` set to the first offending index, and `r10=0`.
- Output derivation uses domain separation and canonical encodings as described above; outputs are deterministic across hardware.

Notes
- All calls execute via `CoreHost` and are subject to permission checks and invariants identical to built‑in ISIs.
- Gas names reference entries in the generated gas table for the active bytecode header version.

## ABI Stability (ABI v1)

This first release fixes the syscall surface and pointer‑ABI to v1. Runtime upgrades are supported,
but they must not change the host ABI.

- `ProgramMetadata.abi_version` must be `1`; other values are rejected at admission.
- Runtime upgrade manifests must keep `abi_version = 1` and leave `added_syscalls`/`added_pointer_types` empty.
- ABI goldens (syscall list, ABI hash, pointer type IDs) are pinned for v1 and must not change.
- ABI changes are not planned; if ever required, they would be delivered as a new core release with
  updated policies, tests, and docs.

<!-- BEGIN GENERATED SYSCALLS -->
| Number | Name | Args | Return | Gas |
|---|---|---|---|---|
| 0x00 | DEBUG_PRINT | - | - | asset:gas/G_debug@ivm.core/v2 |
| 0x01 | EXIT | r10=status:u64 | u64=status | asset:gas/G_exit@ivm.core/v2 |
| 0x02 | ABORT | - | u64=0 | asset:gas/G_abort@ivm.core/v2 |
| 0x03 | DEBUG_LOG | r10=&Json | u64=0 | asset:gas/G_debug@ivm.core/v2 |
| 0x10 | REGISTER_DOMAIN | r10=&DomainId | u64=0 | asset:gas/G_reg_domain@ivm.core/v2 |
| 0x11 | UNREGISTER_DOMAIN | r10=&DomainId | u64=0 | asset:gas/G_unreg_domain@ivm.core/v2 |
| 0x12 | TRANSFER_DOMAIN | r10=&DomainId, r11=&AccountId | u64=0 | asset:gas/G_xfer_domain@ivm.core/v2 |
| 0x13 | REGISTER_ACCOUNT | r10=&ScopedAccountId | u64=0 | asset:gas/G_reg_acct@ivm.core/v2 |
| 0x14 | UNREGISTER_ACCOUNT | r10=&AccountId | u64=0 | asset:gas/G_unreg_acct@ivm.core/v2 |
| 0x15 | REGISTER_PEER | r10=&Json | u64=0 | asset:gas/G_reg_peer@ivm.core/v2 |
| 0x16 | UNREGISTER_PEER | r10=&Json | u64=0 | asset:gas/G_unreg_peer@ivm.core/v2 |
| 0x17 | ADD_SIGNATORY | r10=&AccountId, r11=&Json | u64=0 | asset:gas/G_add_sig@ivm.core/v2 |
| 0x18 | REMOVE_SIGNATORY | r10=&AccountId, r11=&Json | u64=0 | asset:gas/G_rm_sig@ivm.core/v2 |
| 0x19 | SET_ACCOUNT_QUORUM | r10=&AccountId, r11=quorum:u64 | u64=0 | asset:gas/G_set_quorum@ivm.core/v2 |
| 0x1A | SET_ACCOUNT_DETAIL | r10=&AccountId, r11=&Name, r12=&Json | u64=0 | asset:gas/G_set_detail@ivm.core/v2 + bytes(val) |
| 0x20 | REGISTER_ASSET | r10=&AssetDefinitionId | u64=0 | asset:gas/G_reg_asset@ivm.core/v2 |
| 0x21 | UNREGISTER_ASSET | r10=&AssetDefinitionId | u64=0 | asset:gas/G_unreg_asset@ivm.core/v2 |
| 0x22 | MINT_ASSET | r10=&AccountId, r11=&AssetDefinitionId, r12=&NoritoBytes(Numeric) | u64=0 | asset:gas/G_mint@ivm.core/v2 |
| 0x23 | BURN_ASSET | r10=&AccountId, r11=&AssetDefinitionId, r12=&NoritoBytes(Numeric) | u64=0 | asset:gas/G_burn@ivm.core/v2 |
| 0x24 | TRANSFER_ASSET | r10=&AccountId(from), r11=&AccountId(to), r12=&AssetDefinitionId, r13=&NoritoBytes(Numeric) | u64=0 | asset:gas/G_transfer@ivm.core/v2 |
| 0x25 | NFT_MINT_ASSET | r10=&NftId, r11=&AccountId(owner) | u64=0 | asset:gas/G_nft_mint_asset@ivm.core/v2 |
| 0x26 | NFT_TRANSFER_ASSET | r10=&AccountId(from), r11=&NftId, r12=&AccountId(to) | u64=0 | asset:gas/G_nft_transfer_asset@ivm.core/v2 |
| 0x27 | NFT_SET_METADATA | r10=&NftId, r11=&Json | u64=0 | asset:gas/G_nft_set_metadata@ivm.core/v2 |
| 0x28 | NFT_BURN_ASSET | r10=&NftId | u64=0 | asset:gas/G_nft_burn_asset@ivm.core/v2 |
| 0x29 | TRANSFER_V1_BATCH_BEGIN | - | u64=0 | - |
| 0x2A | TRANSFER_V1_BATCH_END | - | u64=0 | - |
| 0x2B | TRANSFER_V1_BATCH_APPLY | r10=&NoritoBytes(TransferAssetBatch) | u64=0 | asset:gas/G_transfer@ivm.core/v2 per entry |
| 0x30 | CREATE_ROLE | r10=&Name, r11=&Json(perms) | u64=0 | asset:gas/G_create_role@ivm.core/v2 |
| 0x31 | DELETE_ROLE | r10=&Name | u64=0 | asset:gas/G_delete_role@ivm.core/v2 |
| 0x32 | GRANT_ROLE | r10=&AccountId, r11=&Name | u64=0 | asset:gas/G_grant_role@ivm.core/v2 |
| 0x33 | REVOKE_ROLE | r10=&AccountId, r11=&Name | u64=0 | asset:gas/G_revoke_role@ivm.core/v2 |
| 0x34 | GRANT_PERMISSION | r10=&AccountId, r11=&Name | u64=0 | asset:gas/G_grant_perm@ivm.core/v2 |
| 0x35 | REVOKE_PERMISSION | r10=&AccountId, r11=&Name | u64=0 | asset:gas/G_revoke_perm@ivm.core/v2 |
| 0x40 | CREATE_TRIGGER | r10=&Json(spec) | u64=0 | asset:gas/G_create_trig@ivm.core/v2 |
| 0x41 | REMOVE_TRIGGER | r10=&Name | u64=0 | asset:gas/G_remove_trig@ivm.core/v2 |
| 0x42 | SET_TRIGGER_ENABLED | r10=&Name, r11=enabled:u64 | u64=0 | asset:gas/G_set_trig@ivm.core/v2 |
| 0x43 | DEACTIVATE_CONTRACT_INSTANCE | r10=&NoritoBytes(DeactivateContractInstance) | u64=0 | - |
| 0x44 | REMOVE_SMART_CONTRACT_BYTES | r10=&NoritoBytes(RemoveSmartContractBytes) | u64=0 | - |
| 0x45 | REGISTER_SMART_CONTRACT_CODE | r10=&NoritoBytes(RegisterSmartContractCode) | u64=0 | - |
| 0x46 | REGISTER_SMART_CONTRACT_BYTES | r10=&NoritoBytes(RegisterSmartContractBytes) | u64=0 | - |
| 0x47 | ACTIVATE_CONTRACT_INSTANCE | r10=&NoritoBytes(ActivateContractInstance) | u64=0 | - |
| 0x50 | STATE_GET | r10=&Name | r10=ptr (&NoritoBytes) or 0 | - |
| 0x51 | STATE_SET | r10=&Name, r11=&NoritoBytes | u64=0 | - |
| 0x52 | STATE_DEL | r10=&Name | u64=0 | - |
| 0x53 | DECODE_INT | r10=&NoritoBytes(ASCII decimal) or r10=&Blob(ASCII decimal) | r10=i64 | - |
| 0x54 | BUILD_PATH_MAP_KEY | r10=&Name(base), r11=key:i64 | r10=ptr (&Name) | - |
| 0x55 | ENCODE_INT | r10=value:i64 | r10=ptr (&NoritoBytes(ASCII decimal)) | - |
| 0x56 | BUILD_PATH_KEY_NORITO | r10=&Name(base), r11=&NoritoBytes(key) | r10=ptr (&Name) | - |
| 0x57 | JSON_ENCODE | r10=&Json | ptr (&NoritoBytes) | asset:gas/G_json_encode@ivm.core/v2 |
| 0x58 | JSON_DECODE | r10=&NoritoBytes(JSON bytes) | ptr (&Json) | asset:gas/G_json_decode@ivm.core/v2 |
| 0x59 | SCHEMA_ENCODE | r10=&Name(schema), r11=&Json | ptr (&NoritoBytes) | - |
| 0x5A | SCHEMA_DECODE | r10=&Name(schema), r11=&NoritoBytes | ptr (&Json) | - |
| 0x5B | SCHEMA_INFO | r10=&Name(schema) | ptr (&Json{"id":...,"version":...}) | - |
| 0x5C | NAME_DECODE | r10=&NoritoBytes(UTF-8 string) | ptr (&Name) | asset:gas/G_name_decode@ivm.core/v2 |
| 0x5D | POINTER_TO_NORITO | r10=&PointerType<T> | ptr (&NoritoBytes(TLV envelope)) | - |
| 0x5E | POINTER_FROM_NORITO | r10=&NoritoBytes(TLV envelope), r11=expected?:u16 | ptr (&PointerType<T>) | - |
| 0x5F | TLV_EQ | r10=&Tlv, r11=&Tlv | r10=1/0 | - |
| 0x60 | ZK_VERIFY_TRANSFER | r10=&NoritoBytes(OpenVerifyEnvelope) | u64=0/1 | asset:gas/G_verify_proof@ivm.core/v2 |
| 0x61 | ZK_VERIFY_UNSHIELD | r10=&NoritoBytes(OpenVerifyEnvelope) | u64=0/1 | asset:gas/G_verify_proof@ivm.core/v2 |
| 0x62 | ZK_VOTE_VERIFY_BALLOT | r10=&NoritoBytes(OpenVerifyEnvelope) | u64=0/1 | asset:gas/G_verify_proof@ivm.core/v2 |
| 0x63 | ZK_VOTE_VERIFY_TALLY | r10=&NoritoBytes(OpenVerifyEnvelope) | u64=0/1 | asset:gas/G_verify_proof@ivm.core/v2 |
| 0x64 | ZK_ROOTS_GET | r10=&NoritoBytes(RootsGetRequest) | ptr (NoritoBytes in INPUT) | asset:gas/G_roots_get@ivm.core/v2 |
| 0x65 | ZK_VOTE_GET_TALLY | r10=&NoritoBytes(VoteGetTallyRequest) | ptr (NoritoBytes in INPUT) | asset:gas/G_vote_get@ivm.core/v2 |
| 0x66 | VRF_VERIFY | r10=&NoritoBytes(VrfVerifyRequest) | r10=ptr (&Blob(32-byte output)), r11=status:u64 | asset:gas/G_verify@ivm.core/v2 |
| 0x67 | VRF_VERIFY_BATCH | r10=&NoritoBytes(VrfVerifyBatchRequest) | r10=ptr (&NoritoBytes(Vec<[u8;32]>)), r11=status:u64, r12=fail_index?:u64 | asset:gas/G_verify@ivm.core/v2 |
| 0x68 | ZK_VERIFY_BATCH | r10=&NoritoBytes(Vec<OpenVerifyEnvelope>) | r10=ptr (&NoritoBytes(Vec<u8> statuses)), r11=status:u64 | asset:gas/G_verify@ivm.core/v2 |
| 0x69 | NUMERIC_FROM_INT | r10=value:i64 | r10=ptr (&NoritoBytes(Numeric)) | - |
| 0x6A | NUMERIC_TO_INT | r10=&NoritoBytes(Numeric) | r10=value:i64 | - |
| 0x6B | NUMERIC_ADD | r10=&NoritoBytes(Numeric), r11=&NoritoBytes(Numeric) | r10=ptr (&NoritoBytes(Numeric)) | - |
| 0x6C | NUMERIC_SUB | r10=&NoritoBytes(Numeric), r11=&NoritoBytes(Numeric) | r10=ptr (&NoritoBytes(Numeric)) | - |
| 0x6D | NUMERIC_MUL | r10=&NoritoBytes(Numeric), r11=&NoritoBytes(Numeric) | r10=ptr (&NoritoBytes(Numeric)) | - |
| 0x6E | NUMERIC_DIV | r10=&NoritoBytes(Numeric), r11=&NoritoBytes(Numeric) | r10=ptr (&NoritoBytes(Numeric)) | - |
| 0x6F | NUMERIC_REM | r10=&NoritoBytes(Numeric), r11=&NoritoBytes(Numeric) | r10=ptr (&NoritoBytes(Numeric)) | - |
| 0x70 | NUMERIC_NEG | r10=&NoritoBytes(Numeric) | r10=ptr (&NoritoBytes(Numeric)) | - |
| 0x71 | NUMERIC_EQ | r10=&NoritoBytes(Numeric), r11=&NoritoBytes(Numeric) | r10=u64(0/1) | - |
| 0x72 | NUMERIC_NE | r10=&NoritoBytes(Numeric), r11=&NoritoBytes(Numeric) | r10=u64(0/1) | - |
| 0x73 | NUMERIC_LT | r10=&NoritoBytes(Numeric), r11=&NoritoBytes(Numeric) | r10=u64(0/1) | - |
| 0x74 | NUMERIC_LE | r10=&NoritoBytes(Numeric), r11=&NoritoBytes(Numeric) | r10=u64(0/1) | - |
| 0x75 | NUMERIC_GT | r10=&NoritoBytes(Numeric), r11=&NoritoBytes(Numeric) | r10=u64(0/1) | - |
| 0x76 | NUMERIC_GE | r10=&NoritoBytes(Numeric), r11=&NoritoBytes(Numeric) | r10=u64(0/1) | - |
| 0x77 | TLV_LEN | - | u64=0 | - |
| 0x78 | JSON_GET_I64 | - | u64=0 | - |
| 0x79 | JSON_GET_JSON | - | u64=0 | - |
| 0x7A | JSON_GET_NAME | - | u64=0 | - |
| 0x7B | JSON_GET_ACCOUNT_ID | - | u64=0 | - |
| 0x7C | JSON_GET_NFT_ID | - | u64=0 | - |
| 0x7D | JSON_GET_BLOB_HEX | - | u64=0 | - |
| 0x7E | VRF_EPOCH_SEED | r10=&NoritoBytes(VrfEpochSeedRequest) | r10=ptr (&NoritoBytes(VrfEpochSeedResponse)), r11=status:u64 | asset:gas/G_vote_get@ivm.core/v2 |
| 0x7F | JSON_GET_NUMERIC | - | u64=0 | - |
| 0x90 | SM3_HASH | r10=&Blob(message) | r10=ptr (&Blob(digest)) | - |
| 0x91 | SM2_VERIFY | r10=&Blob(msg), r11=&Blob(sig), r12=&Blob(pubkey), r13=&Blob(distid)? | u64=0/1 | asset:gas/G_verify@ivm.core/v2 |
| 0x92 | SM4_GCM_SEAL | r10=&Blob(key16), r11=&Blob(nonce12), r12=&Blob(aad)?, r13=&Blob(plaintext) | r10=ptr (&Blob(ciphertext || tag16)) | - |
| 0x93 | SM4_GCM_OPEN | r10=&Blob(key16), r11=&Blob(nonce12), r12=&Blob(aad)?, r13=&Blob(ciphertext || tag16) | r10=ptr (&Blob(plaintext)) or 0 | - |
| 0x94 | SM4_CCM_SEAL | r10=&Blob(key16), r11=&Blob(nonce[7..13]), r12=&Blob(aad)?, r13=&Blob(plaintext), r14=tag_len:u64 | r10=ptr (&Blob(ciphertext || tag)) | - |
| 0x95 | SM4_CCM_OPEN | r10=&Blob(key16), r11=&Blob(nonce[7..13]), r12=&Blob(aad)?, r13=&Blob(ciphertext || tag), r14=tag_len:u64 | r10=ptr (&Blob(plaintext)) or 0 | - |
| 0x96 | SHA256_HASH | - | u64=0 | - |
| 0x97 | SHA3_HASH | - | u64=0 | - |
| 0xA0 | SMARTCONTRACT_EXECUTE_INSTRUCTION | r10=&NoritoBytes(InstructionBox) | u64=0 | asset:gas/G_sci@ivm.core/v2 |
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY | r10=&NoritoBytes(QueryRequest) | r10=ptr (&NoritoBytes(QueryResponse)) | asset:gas/G_scq@ivm.core/v2 |
| 0xA2 | CREATE_NFTS_FOR_ALL_USERS | - | u64=count | asset:gas/G_create_nfts_all@ivm.core/v2 |
| 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | r10=depth:u64 | u64=prev | asset:gas/G_sc_depth@ivm.core/v2 |
| 0xA4 | GET_AUTHORITY | - | ptr (AccountId in INPUT) | asset:gas/G_get_auth@ivm.core/v2 |
| 0xA5 | SUBSCRIPTION_BILL | - | u64=0 | asset:gas/G_sub_bill@ivm.core/v2 |
| 0xA6 | SUBSCRIPTION_RECORD_USAGE | - | u64=0 | asset:gas/G_sub_usage@ivm.core/v2 |
| 0xA7 | RESOLVE_ACCOUNT_ALIAS | r10=&Blob(alias literal) | ptr (&AccountId in INPUT) | asset:gas/G_alias_resolve@ivm.core/v2 |
| 0xB0 | AXT_BEGIN | r10=&AxtDescriptor | u64=0 | - |
| 0xB1 | AXT_TOUCH | r10=&DataSpaceId, r11=&NoritoBytes(TouchManifest) or 0 | u64=0 | - |
| 0xB2 | AXT_COMMIT | - | u64=0 | - |
| 0xB3 | VERIFY_DS_PROOF | r10=&DataSpaceId, r11=&ProofBlob or 0 | u64=0/1 | asset:gas/G_verify@ivm.core/v2 |
| 0xB4 | USE_ASSET_HANDLE | r10=&AssetHandle, r11=&NoritoBytes(RemoteSpendIntent), r12=&ProofBlob? | u64=0 | - |
| 0xC0 | SORACLOUD_READ_COMMITTED_STATE | r10=&SoracloudRequest(ReadCommittedState) | r10=&SoracloudResponse(ReadCommittedState) | - |
| 0xC1 | SORACLOUD_EMIT_STATE_MUTATION | r10=&SoracloudRequest(EmitStateMutation) | r10=&SoracloudResponse(EmitStateMutation) | - |
| 0xC2 | SORACLOUD_EMIT_MAILBOX_MESSAGE | r10=&SoracloudRequest(EmitMailboxMessage) | r10=&SoracloudResponse(EmitMailboxMessage) | - |
| 0xC3 | SORACLOUD_APPEND_JOURNAL | r10=&SoracloudRequest(AppendJournal) | r10=&SoracloudResponse(AppendJournal) | - |
| 0xC4 | SORACLOUD_PUBLISH_CHECKPOINT | r10=&SoracloudRequest(PublishCheckpoint) | r10=&SoracloudResponse(PublishCheckpoint) | - |
| 0xC5 | SORACLOUD_READ_SECRET | r10=&SoracloudRequest(ReadSecret) | r10=&SoracloudResponse(ReadSecret) | - |
| 0xC6 | SORACLOUD_READ_CREDENTIAL | r10=&SoracloudRequest(ReadCredential) | r10=&SoracloudResponse(ReadCredential) | - |
| 0xC7 | SORACLOUD_EGRESS_FETCH | r10=&SoracloudRequest(EgressFetch) | r10=&SoracloudResponse(EgressFetch) | - |
| 0xC8 | SORACLOUD_READ_CONFIG | r10=&SoracloudRequest(ReadConfig) | r10=&SoracloudResponse(ReadConfig) | - |
| 0xC9 | SORACLOUD_READ_SECRET_ENVELOPE | r10=&SoracloudRequest(ReadSecretEnvelope) | r10=&SoracloudResponse(ReadSecretEnvelope) | - |
| 0xE0 | INPUT_PUBLISH_TLV | r10=&Blob(TLV) | ptr (r10) | asset:gas/G_input_publish@ivm.core/v2 |
| 0xF0 | ALLOC | r10=bytes:u64 | ptr (r10) | asset:gas/G_alloc@ivm.core/v2 + bytes |
| 0xF1 | GET_PUBLIC_INPUT | r10=&Name | ptr (&Tlv) | asset:gas/G_get_pub@ivm.core/v2 + bytes |
| 0xF4 | PROVE_EXECUTION | - | r10=0/1 | asset:gas/G_prove@ivm.core/v2 |
| 0xF5 | GROW_HEAP | r10=bytes:u64 | u64=new_limit | asset:gas/G_grow_heap@ivm.core/v2 per page |
| 0xF6 | VERIFY_PROOF | - | r10=0/1 | asset:gas/G_verify@ivm.core/v2 |
| 0xF7 | GET_MERKLE_PATH | r10=addr:u64, r11=out:u64, r12=root_out?:u64 | u64=len | asset:gas/G_mpath@ivm.core/v2 + len |
| 0xF9 | GET_ACCOUNT_BALANCE | r10=&AccountId, r11=&AssetDefinitionId | ptr (&NoritoBytes(Numeric)) | asset:gas/G_get_bal@ivm.core/v2 |
| 0xFA | GET_MERKLE_COMPACT | r10=addr, r11=out, r12=depth_cap?, r13=root_out? | u64=depth | asset:gas/G_mpath@ivm.core/v2 + depth |
| 0xFB | USE_NULLIFIER | r10=nullifier:u64 | u64=0 | asset:gas/G_use_null@ivm.core/v2 |
| 0xFC | VERIFY_SIGNATURE | r10=&Blob(message), r11=&Blob(signature), r12=&Blob(pubkey), r13=scheme:u8 | r10=0/1 | asset:gas/G_verify_sig@ivm.core/v2 |
| 0xFD | GET_PRIVATE_INPUT | r10=index:u64 | r10=value | asset:gas/G_get_priv@ivm.core/v2 |
| 0xFE | COMMIT_OUTPUT | - | u64=0 | asset:gas/G_commit@ivm.core/v2 |
| 0xFF | GET_REGISTER_MERKLE_COMPACT | r10=reg, r11=out, r12=depth_cap?, r13=root_out? | u64=depth | asset:gas/G_mpath@ivm.core/v2 + depth |
<!-- END GENERATED SYSCALLS -->
































Codec helpers
- 0x53 DECODE_INT — Args: `r10=&NoritoBytes(Norito-framed i64)` → Return: `r10=i64`
- 0x57 JSON_ENCODE — Args: `r10=&Json` → Return: `ptr (&NoritoBytes(Json))` — Gas: G_json_encode
- 0x58 JSON_DECODE — Args: `r10=&NoritoBytes(Json)` or `r10=&Blob(JSON text)` → Return: `ptr (&Json)` — Gas: G_json_decode
- 0x5A SCHEMA_DECODE — Args: `r10=&Name(schema), r11=&NoritoBytes(Json)` → Return: `ptr (&Json)`
- 0x5F TLV_EQ — Args: `r10=&Tlv, r11=&Tlv` → Return: `r10=1 if equal else 0` — Gas: -
- 0x5C NAME_DECODE — Args: `r10=&NoritoBytes(Name)` → Return: `ptr (&Name)` — Gas: G_name_decode
- NAME_DECODE validates Name grammar (non-empty, no whitespace or `@/#/$`) and normalizes the output.
- 0x5D POINTER_TO_NORITO — Args: `r10=&PointerType<T>` → Return: `ptr (&NoritoBytes(TLV envelope))`
- 0x5E POINTER_FROM_NORITO — Args: `r10=&NoritoBytes(TLV envelope), r11=expected?:u16` → Return: `ptr (&PointerType<T>)`
- Null inputs: DECODE_INT, JSON_DECODE, NAME_DECODE, and POINTER_FROM_NORITO accept `r10=0` and return `r10=0` without error.
- All other pointer-typed syscalls require explicit non-zero pointers; there is no implicit last-input fallback.
ZK (Halo2 OpenVerify)
- 0x68 ZK_VERIFY_BATCH — Args: `r10=&NoritoBytes(Vec<iroha_data_model::zk::OpenVerifyEnvelope>)` → Return: `r10=ptr (&NoritoBytes(Vec<u8> statuses))`, `r11=status:u64`, `r12=first_fail_index|u64::MAX` — Gas: G_verify
  - Per-item statuses are `1 = verified`, `0 = not verified`. Each item runs the same binding + full backend verification path as single-item ZK verify syscalls.
  - Top-level request failures (decode, disabled backend, oversized batch) return `r10=0` and set `r11` (`ERR_DECODE`, `ERR_DISABLED`, `ERR_BACKEND`, `ERR_BATCH`).
  - On vector return, `r11` carries the first observed precheck/verify error code (or `0` when all succeed).
