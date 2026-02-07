---
lang: hy
direction: ltr
source: docs/source/ivm_syscalls.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bcf280df1e00065199d386e07b9fd67d8f94c4046d73cfa3b63d1eec18228cd8
source_last_modified: "2026-01-22T16:26:46.570453+00:00"
translation_last_reviewed: 2026-02-07
---

# IVM Syscall ABI

This document defines the IVM syscall numbers, pointer-ABI calling conventions, reserved number ranges, and the canonical table of contract-facing syscalls used by Kotodama lowering. It complements `ivm.md` (architecture) and `kotodama_grammar.md` (language).

Versioning
- The set of recognized syscalls depends on the bytecode header `abi_version` field. The first release accepts only `abi_version = 1`; other values are rejected at admission. Unknown numbers for the active `abi_version` deterministically trap with `E_SCALL_UNKNOWN`.
- Runtime upgrades keep `abi_version = 1` and do not expand syscall or pointer‑ABI surfaces.
- Syscall gas costs are part of the versioned gas schedule bound to the bytecode header version. See `ivm.md` (Gas policy).

Numbering ranges
- `0x00..=0x1F`: VM core/utility (debug/exit helpers are available under `CoreHost`; remaining dev helpers are mock-host only).
- `0x20..=0x5F`: Iroha core ISI bridge (stable in ABI v1).
- `0x60..=0x7F`: extension ISIs gated by protocol features (still part of ABI v1 when enabled).
- `0x80..=0xFF`: host/crypto helpers and reserved slots; only numbers present in the ABI v1 allowlist are accepted.

Durable helpers (ABI v1)
- The durable state helper syscalls (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*, JSON/SCHEMA encode/decode) are part of the V1 ABI and included in `abi_hash` computation.
- CoreHost wires STATE_{GET,SET,DEL} to WSV-backed durable smart-contract state; dev/test hosts may persist locally but must preserve identical syscall semantics.

Pointer‑ABI calling convention (smart‑contract syscalls)
- Arguments are placed in registers `r10+` as raw `u64` values or as pointers into the INPUT region to immutable Norito TLV envelopes (e.g., `AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`).
- Scalar return values are the `u64` returned from the host. Pointer results are written by the host into `r10`.

Canonical syscall table (subset)

| Hex  | Name                       | Arguments (in `r10+`)                                                   | Returns     | Gas (base + variable)        | Notes |
|------|----------------------------|-------------------------------------------------------------------------|-------------|------------------------------|-------|
| 0x1A | SET_ACCOUNT_DETAIL         | `&AccountId`, `&Name`, `&Json`                                          | `u64=0`     | `G_set_detail + bytes(val)`  | Writes a detail for the account |
| 0x22 | MINT_ASSET                 | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)`             | `u64=0`     | `G_mint`                     | Mints `amount` of asset to account |
| 0x23 | BURN_ASSET                 | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)`             | `u64=0`     | `G_burn`                     | Burns `amount` from account |
| 0x24 | TRANSFER_ASSET             | `&AccountId(from)`, `&AccountId(to)`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0`     | `G_transfer`                 | Transfers `amount` between accounts |
| 0x29 | TRANSFER_V1_BATCH_BEGIN    | –                                                                       | `u64=0`     | `G_transfer`                 | Begin FASTPQ transfer batch scope |
| 0x2A | TRANSFER_V1_BATCH_END      | –                                                                       | `u64=0`     | `G_transfer`                 | Flush accumulated FASTPQ transfer batch |
| 0x2B | TRANSFER_V1_BATCH_APPLY    | `r10=&NoritoBytes(TransferAssetBatch)`                                  | `u64=0`     | `G_transfer`                 | Apply a Norito-encoded batch in a single syscall |
| 0x25 | NFT_MINT_ASSET             | `&NftId`, `&AccountId(owner)`                                           | `u64=0`     | `G_nft_mint_asset`           | Registers a new NFT |
| 0x26 | NFT_TRANSFER_ASSET         | `&AccountId(from)`, `&NftId`, `&AccountId(to)`                          | `u64=0`     | `G_nft_transfer_asset`       | Transfers ownership of NFT |
| 0x27 | NFT_SET_METADATA           | `&NftId`, `&Json`                                                       | `u64=0`     | `G_nft_set_metadata`         | Updates NFT metadata |
| 0x28 | NFT_BURN_ASSET             | `&NftId`                                                                | `u64=0`     | `G_nft_burn_asset`           | Burns (destroys) an NFT |
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)`                                        | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | Iterable queries run ephemerally; `QueryRequest::Continue` rejected |
| 0xA2 | CREATE_NFTS_FOR_ALL_USERS  | –                                                                       | `u64=count` | `G_create_nfts_for_all`      | Helper; feature‑gated |
| 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64`                                                         | `u64=prev`  | `G_set_depth`                | Admin; feature‑gated |
| 0xA4 | GET_AUTHORITY              | – (host writes result)                                                  | `&AccountId`| `G_get_auth`                 | Host writes pointer to current authority into `r10` |
| 0xF7 | GET_MERKLE_PATH            | `addr:u64`, `out_ptr:u64`, optional `root_out:u64`                      | `u64=len`   | `G_mpath + len`             | Writes path (leaf→root) and optional root bytes |
| 0xFA | GET_MERKLE_COMPACT         | `addr:u64`, `out_ptr:u64`, optional `depth_cap:u64`, optional `root_out:u64` | `u64=depth` | `G_mpath + depth`           | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`, `out_ptr:u64`, optional `depth_cap:u64`, optional `root_out:u64` | `u64=depth` | `G_mpath + depth`           | Same compact layout for register commitment |

Gas enforcement
- CoreHost charges extra gas for ISI syscalls using the native ISI schedule; FASTPQ batch transfers are charged per entry.
- ZK_VERIFY syscalls reuse the confidential verification gas schedule (base + proof size).
- SMARTCONTRACT_EXECUTE_QUERY charges base + per-item + per-byte; sorting multiplies per-item cost and unsorted offsets add a per-item penalty.

Notes
- All pointer arguments reference Norito TLV envelopes in the INPUT region and are validated on first dereference (`E_NORITO_INVALID` on error).
- All mutations are applied via Iroha’s standard executor (through `CoreHost`), not directly by the VM.
- Exact gas constants (`G_*`) are defined by the active gas schedule; see `ivm.md`.

Errors
- `E_SCALL_UNKNOWN`: syscall number not recognized for the active `abi_version`.
- Input validation errors propagate as VM traps (e.g., `E_NORITO_INVALID` for malformed TLVs).

Cross‑references
- Architecture and VM semantics: `ivm.md`
- Language and builtin mapping: `docs/source/kotodama_grammar.md`

Generation note
- A complete list of syscall constants can be generated from source with:
  - `make docs-syscalls` → writes `docs/source/ivm_syscalls_generated.md`
  - `make check-docs` → verifies the generated table is up to date (useful in CI)
- The subset above remains a curated, stable table for contract-facing syscalls.

## Admin/Role TLV Examples (Mock Host)

This section documents the TLV shapes and minimal JSON payloads accepted by the mock WSV host for admin‑style syscalls used in tests. All pointer arguments follow the pointer‑ABI (Norito TLV envelopes placed in INPUT). Production hosts may use richer schemas; these examples aim to clarify types and basic shapes.

- REGISTER_PEER / UNREGISTER_PEER
  - Args: `r10=&Json`
  - Example JSON: `{ "peer": "peer-id-or-info" }`
  - CoreHost note: `REGISTER_PEER` expects a `RegisterPeerWithPop` JSON object with `peer` + `pop` bytes (optional `activation_at`, `expiry_at`, `hsm`); `UNREGISTER_PEER` accepts a peer-id string or `{ "peer": "..." }`.

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  - CREATE_TRIGGER:
    - Args: `r10=&Json`
    - Minimal JSON: `{ "name": "t1" }` (additional fields ignored by the mock)
  - REMOVE_TRIGGER:
    - Args: `r10=&Name` (trigger name)
  - SET_TRIGGER_ENABLED:
    - Args: `r10=&Name`, `r11=enabled:u64` (0 = disabled, non‑zero = enabled)
  - CoreHost note: `CREATE_TRIGGER` expects a full trigger spec (base64 Norito `Trigger` string or
    `{ "id": "<trigger_id>", "action": ... }` with `action` as a base64 Norito `Action` string or
    a JSON object), and `SET_TRIGGER_ENABLED` toggles the trigger metadata key `__enabled` (missing
    defaults to enabled).

- Roles: CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - CREATE_ROLE:
    - Args: `r10=&Name` (role name), `r11=&Json` (permissions set)
    - JSON accepts either key `"perms"` or `"permissions"`, each a string array of permission names.
    - Examples:
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:ih58...", "transfer_asset:rose#wonder" ] }`
    - Supported permission name prefixes in the mock:
      - `register_domain`, `register_account`, `register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  - DELETE_ROLE:
    - Args: `r10=&Name`
    - Fails if any account is still assigned this role.
  - GRANT_ROLE / REVOKE_ROLE:
    - Args: `r10=&AccountId` (subject), `r11=&Name` (role name)
  - CoreHost note: permission JSON may be a full `Permission` object (`{ "name": "...", "payload": ... }`) or a string (payload defaults to `null`); `GRANT_PERMISSION`/`REVOKE_PERMISSION` accept `&Name` or `&Json(Permission)`.

- Unregister ops (domain/account/asset): invariants (mock)
  - UNREGISTER_DOMAIN (`r10=&DomainId`) fails if accounts or asset definitions exist in the domain.
  - UNREGISTER_ACCOUNT (`r10=&AccountId`) fails if the account has non‑zero balances or owns NFTs.
  - UNREGISTER_ASSET (`r10=&AssetDefinitionId`) fails if any balances exist for the asset.

Notes
- These examples reflect the mock WSV host used in tests; real node hosts may expose richer admin schemas or require additional validation. The pointer‑ABI rules still apply: TLVs must be in INPUT, version=1, type IDs must match, and payload hashes must validate.
