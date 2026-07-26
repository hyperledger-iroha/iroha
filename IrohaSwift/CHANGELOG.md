# Changelog

All notable changes to `IrohaSwift` are documented in this file.

## [Unreleased]

- Bound the `CancelAssetLock` lock-ID preimage to the public V1 limit of 4,096
  UTF-8 bytes while preserving the fixed 32-byte `EscrowId` wire field.
- Added strict typed `CancelAssetLock` V1 parity. Swift now derives the native
  marked Blake2b-256 escrow id from a clean lock id, requires an exact positive
  `expected_remaining_amount`, emits a transaction-ready schema-bound Norito
  frame, and rejects the retired one-field JSON/Norito layout, aliases, extras,
  malformed identifiers, noncanonical quantities, and trailing bytes.
