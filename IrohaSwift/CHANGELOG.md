# Changelog

All notable changes to `IrohaSwift` are documented in this file.

## [Unreleased]

- Removed generic shield, shielded-transfer, and unshield request/encoder/native
  signer surfaces from ABI V1. Typed Kagemusha top-up/redemption and their
  underlying proof codecs remain available.
- Replaced the governance mutation boundary with closed public-only request
  types. Deploy proposals no longer expose ignored limits and now use typed
  manifest provenance; ZK public inputs are exact and shared across legacy,
  v1-envelope, and nested BallotProof routes; Parliament ballots use canonical
  enums; and plain-ballot durations encode as canonical decimal JSON strings.
  Added Swift client and `IrohaSDK` helpers for the v1-envelope, BallotProof,
  and Parliament ballot endpoints. Governance windows are ordered across REST
  and local transaction builders, ZK backends are exact tokens, enactment
  requests expose only the authoritative proposal id, deploy builders require
  ABI V1, and locally signed ZK ballots use the same closed typed public-input
  model with recursive private-key-alias defence.
- Bound the `CancelAssetLock` lock-ID preimage to the public V1 limit of 4,096
  UTF-8 bytes while preserving the fixed 32-byte `EscrowId` wire field.
- Added strict typed `CancelAssetLock` V1 parity. Swift now derives the native
  marked Blake2b-256 escrow id from a clean lock id, requires an exact positive
  `expected_remaining_amount`, emits a transaction-ready schema-bound Norito
  frame, and rejects the retired one-field JSON/Norito layout, aliases, extras,
  malformed identifiers, noncanonical quantities, and trailing bytes.
