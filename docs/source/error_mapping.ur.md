---
lang: ur
direction: rtl
source: docs/source/error_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cba8780bcec4ebf562dc9c5725f328b0ea2d9009517efa5b5a504e2fb6be81fe
source_last_modified: "2026-01-18T05:31:56.950113+00:00"
translation_last_reviewed: 2026-01-30
---

# Error Mapping Guide

Last updated: 2025-08-21

This guide maps common failure modes in Iroha to stable error categories surfaced by the data model. Use it to design tests and to make client error handling predictable.

Principles
- Instruction and query paths emit structured enums. Avoid panics; report a specific category wherever possible.
- Categories are stable, messages may evolve. Clients should match on categories, not on free‑form strings.

Categories
- InstructionExecutionError::Find: Entity missing (asset, account, domain, NFT, role, trigger, permission, public key, block, transaction). Example: removing a non‑existent metadata key yields Find(MetadataKey).
- InstructionExecutionError::Repetition: Duplicate registration or conflicting ID. Contains the instruction type and the repeated IdBox.
- InstructionExecutionError::Mintability: Mintability invariant violated (`Once` exhausted twice, `Limited(n)` overdrawn, or attempts to disable `Infinitely`). Examples: minting an asset defined as `Once` twice yields `Mintability(MintUnmintable)`; configuring `Limited(0)` yields `Mintability(InvalidMintabilityTokens)`.
- InstructionExecutionError::Math: Numeric domain errors (overflow, divide‑by‑zero, negative value, not enough quantity). Example: burning more than available amount yields Math(NotEnoughQuantity).
- InstructionExecutionError::InvalidParameter: Invalid instruction parameter or configuration (e.g., time trigger in the past). Use for malformed contract payloads.
- InstructionExecutionError::Evaluate: DSL/spec mismatch for instruction shape or types. Example: wrong numeric spec for an asset value yields Evaluate(Type(AssetNumericSpec(..))).
- InstructionExecutionError::InvariantViolation: Violation of a system invariant that cannot be expressed in other categories. Example: attempting to remove the last signatory.
- InstructionExecutionError::Query: Wrapping of QueryExecutionFail when a query fails during instruction execution.

QueryExecutionFail
- Find: Missing entity in query context.
- Conversion: Wrong type expected by a query.
- NotFound: Missing live query cursor.
- CursorMismatch / CursorDone: Cursor protocol errors.
- FetchSizeTooBig: Server‑enforced limit exceeded.
- GasBudgetExceeded: Query execution exceeded the gas/materialization budget.
- InvalidSingularParameters: Unsupported parameters for singular queries.
- CapacityLimit: Live query store capacity reached.

Testing Tips
- Prefer unit tests close to the origin of an error. For example, asset numeric spec mismatch can be generated in data‑model tests.
- Integration tests should cover end‑to‑end mapping for representative cases (e.g., duplicate register, missing key on remove, transfer without ownership).
- Keep assertions resilient by matching enum variants instead of message substrings.
