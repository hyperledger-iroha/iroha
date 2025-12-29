# ISI Extension Plan (v1)

This note signs off on the priority order for the new Iroha Special Instructions and captures
non-negotiable invariants for each instruction ahead of implementation. The ordering matches
security and operability risk first, UX throughput second.

## Priority Stack

1. **RotateAccountSignatory** – Required for hygienic key rotation without destructive migrations.
2. **DeactivateContractInstance** / **RemoveSmartContractBytes** – Provide deterministic contract
   kill switches and storage reclamation for compromised deployments.
3. **SetAssetKeyValue** / **RemoveAssetKeyValue** – Extend metadata parity to concrete asset
   balances so observability tooling can tag holdings.
4. **BatchMintAsset** / **BatchTransferAsset** – Deterministic fan-out helpers to keep payload size
   and VM fallback pressure manageable.

## Instruction Invariants

### SetAssetKeyValue / RemoveAssetKeyValue
- Reuse the `AssetMetadataKey` namespace (`state.rs`) so canonical WSV keys stay stable.
- Enforce JSON size and schema limits identically to account metadata helpers.
- Emit `AssetEvent::MetadataInserted` / `AssetEvent::MetadataRemoved` with the affected `AssetId`.
- Require the same permission tokens as existing asset metadata edits (definition owner OR
  `CanModifyAssetMetadata`-style grants).
- Abort if the asset record is missing (no implicit creation).

### RotateAccountSignatory
- Atomic swap of the signatory in `AccountId` while preserving account metadata and linked
  resources (assets, triggers, roles, permissions, pending events).
- Verify the current signatory matches the caller (or delegated authority via explicit token).
- Reject if the new public key already backs another account in the same domain.
- Update all canonical keys that embed the account ID and invalidate caches before commit.
- Emit a dedicated `AccountEvent::SignatoryRotated` with old/new keys for audit trails.
- Migration scaffold: introduce `AccountLabel` + `AccountRekeyRecord` (see `account::rekey`) so
  existing accounts can be mapped to stable labels during a rolling upgrade without hash breaks.

### DeactivateContractInstance
- Remove or tombstone the `(namespace, contract_id)` binding while persisting provenance data
  (who, when, reason code) for troubleshooting.
- Require the same governance permission set as activation, with policy hooks to disallow
  deactivation of core system namespaces without elevated approval.
- Reject when the instance is already inactive to keep event logs deterministic.
- Emit a `ContractInstanceEvent::Deactivated` that downstream watchers can consume.

### RemoveSmartContractBytes
- Allow pruning of stored bytecode by `code_hash` only when no manifests or active instances
  reference the artifact; otherwise fail with a descriptive error.
- Permission gate mirrors registration (`CanRegisterSmartContractCode`) plus an operator-level
  guard (e.g., `CanManageSmartContractStorage`).
- Verify the provided `code_hash` matches the stored body digest just before deletion to avoid
  stale handles.
- Emit `ContractCodeEvent::Removed` with hash and caller metadata.

### BatchMintAsset / BatchTransferAsset
- All-or-nothing semantics: either every tuple succeeds or the instruction aborts without side
  effects.
- Input vectors must be deterministically ordered (no implicit sorting) and bounded by config
  (`max_batch_isi_items`).
- Emit per-item asset events so downstream accounting stays consistent; batch context is additive,
  not a replacement.
- Permission checks reuse existing single-item logic per target (asset owner, definition owner,
  or granted capability) before state mutation.
- Advisory access sets must union all read/write keys to keep optimistic concurrency correct.

## Implementation Scaffolding

- Data model now carries `SetAssetKeyValue` / `RemoveAssetKeyValue` scaffolds for balance metadata
  edits (`transparent.rs`).
- Executor visitors expose placeholders that will gate permissions once host wiring lands
  (`default/mod.rs`).
- Rekey prototype types (`account::rekey`) provide a landing zone for rolling migrations.
- World state includes `account_rekey_records` keyed by `AccountLabel` so we can stage label →
  signatory migrations without touching the historical `AccountId` encoding.

## IVM Syscall Drafting

- Host shims for `DeactivateContractInstance` / `RemoveSmartContractBytes` ship as
  `SYSCALL_DEACTIVATE_CONTRACT_INSTANCE` (0x43) and
  `SYSCALL_REMOVE_SMART_CONTRACT_BYTES` (0x44), both consuming Norito TLVs that mirror the
  canonical ISI structs.
- Extend `abi_syscall_list()` only after host handlers mirror `iroha_core` execution paths to keep
  ABI hashes stable during development.
- Update Kotodama lowering once syscall numbers stabilize; add golden coverage for the expanded
  surface at the same time.

## Status

The above ordering and invariants are ready for implementation. Follow-up branches should reference
this document when wiring execution paths and syscall exposure.
