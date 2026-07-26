---
title: SoraFS Reference SDK Error Codes
summary: Stable ValidationOutcomeV1 codes emitted by sorafs-validate and the reference validator APIs.
---

# SoraFS Reference SDK Error Codes

`sorafs-validate` emits `ValidationOutcomeV1` records with stable `code`,
`category`, and `status` fields. Automation should branch on `code` and
`category`, not on the human-readable `message`.

## Implemented Codes

| Code | Category | Meaning | Operator action |
|------|----------|---------|-----------------|
| `SFS-OK-000` | `validation` | Payload accepted. | Keep the generated outcome with release or rollout evidence. |
| `SFS-PDP-DIAG-000` | `validation` | PDP structure, pair binding, or exhaustive witness verification succeeded only for the diagnostic scope named in `context`; governed admission was not evaluated. | Never treat this code as production proof acceptance. Require `SFS-OK-000` from the admission-bound production verifier and verify `production_acceptance=true`. |
| `SFS-NORITO-001` | `norito` | Input could not be decoded with the expected SoraFS Norito schema. | Re-encode the payload with the current SoraFS Norito type and retry. |
| `SFS-CAR-001` | `validation` | CAR stream replay failed against the manifest commitments. | Regenerate or refetch the CAR stream so its roots, chunk plan, digest, and size match the manifest. |
| `SFS-CAR-002` | `internal` | Trustless manifest/CAR replay could not derive expected metadata after parsing. | Update the trustless verifier config or report the replay metadata derivation failure to maintainers. |
| `SFS-POL-001` | `policy` | Provider advert timestamp or TTL policy failed. | Regenerate the advert inside the allowed validity window. |
| `SFS-POL-002` | `policy` | PoR proof deadline policy failed. | Treat the proof as late unless a governed policy override explicitly extends the deadline. |
| `SFS-POL-003` | `policy` | Replication order deadline or SLA policy failed. | Reissue the order with a deadline after `issued_at` and SLA percentages inside the allowed range. |
| `SFS-POL-004` | `policy` | Provider admission retention policy failed. | Reissue the envelope with `issued_at` not greater than the governed retention epoch. |
| `SFS-POL-005` | `policy` | Repair timestamp or SLA deadline policy failed. | Correct repair lifecycle timestamps or SLA/deadline ordering before submitting the payload. |
| `SFS-POL-006` | `policy` | Manifest pin policy failed during manifest/CAR replay. | Regenerate the manifest under the current SoraFS pin-registry policy and retry manifest/CAR replay. |
| `SFS-POL-007` | `policy` | Orderbook timestamp, nonce, or fee policy failed. | Regenerate the orderbook payload from canonical state with non-zero timestamps/nonces and governed fee basis points. |
| `SFS-POR-001` | `validation` | PoR proof sample coverage does not match the challenge. | Regenerate the proof with exactly the sample indices requested by the challenge. |
| `SFS-POR-003` | `validation` | PoR challenge/proof binding failed. | Regenerate the proof for the exact challenge, manifest, and provider named by the challenge. |
| `SFS-PDP-001` | `validation` | PDP sample window, coverage, geometry, bounds, or authentication-path shape failed validation. | Regenerate the PDP challenge and proof with the exact deterministic sample labels, geometry, and canonical paths required by the commitment. |
| `SFS-PDP-002` | `validation` | PDP commitment, challenge, or proof structure failed validation. | Regenerate the PDP payload from canonical manifest/provider state and inspect the `context` fields for the invalid field. |
| `SFS-PDP-003` | `validation` | PDP commitment/challenge/proof binding or authenticated root comparison failed. | Regenerate the PDP challenge and proof for the exact commitment, manifest, provider, epoch, sampled bytes, and both Merkle roots. |
| `SFS-PDP-004` | `policy` | Production PDP verification lacks a council-verified active admission record or names a different admitted provider. | Resolve the provider from the current revocation-aware admission registry and retry the admission-bound verifier; never upgrade an integrity-only record to trusted state. |
| `SFS-POTR-001` | `policy` | PoTR receipt reports a successful retrieval that missed its deadline. | Treat the receipt as late unless a governed profile override explicitly extends the deadline. |
| `SFS-POTR-002` | `validation` | PoTR receipt tier does not match the requested profile. | Validate the receipt against the tier profile that was requested, or rerun retrieval for the requested tier. |
| `SFS-OBK-001` | `validation` | Orderbook payload structure failed validation outside the more specific code families. | Regenerate the order, cancellation, trade, channel, or receipt payload from canonical orderbook state and inspect the `context` fields. |
| `SFS-OBK-002` | `validation` | Orderbook settlement accounting failed validation. | Regenerate the settlement receipt so provider credit plus fee exactly equals the buyer debit in micro-XOR. |
| `SFS-BND-001` | `validation` | Fixture bundle payload validation failed, or the bundle contains fewer than two linkable SoraFS artifacts. | Fix the invalid payload named in the outcome, or point `--bundle` at a directory containing at least two known fixture artifacts. |
| `SFS-BND-002` | `validation` | Fixture bundle artifacts name different manifest digests. | Regenerate the order, PoR/PoTR, and repair payloads so manifest-bearing artifacts reference the same canonical manifest digest. |
| `SFS-BND-003` | `validation` | Fixture bundle provider links do not agree with each other or with replication-order assignments. | Regenerate provider-admission artifacts from the same provider metadata and ensure manifest-bearing provider artifacts are assigned by the replication order. |
| `SFS-FFI-001` | `internal` | Reference FFI caller supplied an invalid ABI argument such as a null/misaligned pointer, oversized input/bundle, invalid UTF-8 or control-bearing label, or unsupported selector. | Fix the SDK binding to enforce the published input bounds, canonical labels, pointer contracts, and selector constants before retrying. |
| `SFS-FFI-002` | `internal` | Reference FFI panicked or could not render the outcome JSON. | Treat the input as not accepted and report the validator version plus payload to maintainers. |
| `SFS-REP-001` | `validation` | Repair evidence is incomplete or internally invalid. | Attach complete evidence with non-zero PoR samples, latency, or replica shortfall details. |
| `SFS-REP-002` | `validation` | Repair evidence, report, reference task record, slash proposal, escalation policy/approval, task event, or audit event structure failed validation. | Regenerate the payload from canonical governed repair state. |
| `SFS-GOV-001` | `validation` | Governance log node structure, canonical 32-byte CID, publisher peer bound, or embedded payload validation failed. | Regenerate the node CID from the canonical unsigned node payload and use bounded publisher metadata plus governed signature material. |
| `SFS-GOV-002` | `policy` | Repair escalation or slash governance payload failed validation. | Regenerate the escalation policy, approval, or slash proposal from governed policy state. |
| `SFS-GOV-003` | `validation` | Governance log node CID does not match the expected CID. | Pass the node CID that belongs to this governance log node or regenerate the fixture from the canonical node payload. |
| `SFS-GOV-004` | `validation` | Governance DAG block CID does not match the expected CID or canonical block payload. | Pass the block CID that belongs to this block, or regenerate the block from canonical node bytes, parent linkage, timestamp, and publisher metadata. |
| `SFS-GOV-005` | `validation` | Governance DAG block structure, exact CID widths, root/parent semantics, or embedded-node binding failed. | Rebuild the block and node with canonical 32-byte CIDs, bounded matching peer IDs, one Ed25519 key, valid parent semantics, and canonical signatures. |
| `SFS-GOV-006` | `validation` | Governance DAG block/node continuity, ordering, uniqueness, timestamp progression, signer identity, or expected-head binding failed. | Provide a contiguous oldest-to-head root history or checkpoint tail with unique block and node CIDs, exact parent links, checked sequences, and one governed Ed25519 identity. |
| `SFS-GOV-007` | `validation` | Governance DAG head manifest structure, CID width, peer bound, or Ed25519 signature failed validation. | Regenerate the head with exact 32-byte head/checkpoint CIDs, a non-zero block count, bounded publisher identity, and the governed Ed25519 key. |
| `SFS-GOV-008` | `validation` | Governance DAG full-history count or 64-block checkpoint-window binding failed validation. | For at most 64 blocks, pass the complete root history with no checkpoint; for longer histories, bind the first block CID in the newest 64 and pass either the full history or exactly that tail. |
| `SFS-SIG-001` | `signature` | Provider advert Ed25519 signature failed or had malformed key/signature material. | Resign the canonical advert body with the governed provider key. |
| `SFS-SIG-002` | `signature` | Provider admission council signature material is missing, malformed, or failed verification. | Regenerate council signatures over the canonical proposal digest with the governed signer keys. |
| `SFS-SIG-003` | `signature` | PoTR receipt detached signature is malformed or failed verification. | Regenerate the receipt signature over the canonical unsigned receipt payload with the advertised signer key. |
| `SFS-SIG-005` | `signature` | Governance log node publisher signature material is missing, malformed, or failed verification. | Attach governed Ed25519 or Dilithium3/ML-DSA publisher signature material over the canonical governance node signing bytes before publishing or validating the node. |
| `SFS-SIG-006` | `signature` | Signed replication order or governance DAG block signature material is missing, unsupported, malformed, or failed verification. | Resign the `SignedReplicationOrderV1` envelope over the domain-separated canonical order signing bytes, or resign the governance DAG block over its canonical block signing bytes with the governed publisher key. |
| `SFS-SIG-007` | `signature` | Orderbook signature material or governance DAG head signature material is missing, malformed, or failed verification. | Resign the canonical orderbook payload or governance DAG head manifest with the governed Ed25519 key material before submitting or validating it. |
| `SFS-SIG-008` | `signature` | PDP or PoR proof signature material is missing, malformed, untrusted, or failed verification. | Regenerate the proof and sign its canonical domain-separated payload with the governance-admitted provider key. |
| `SFS-VAL-001` | `validation` | Required digest or identifier bytes are invalid or mismatched for a payload being validated. | Recompute the digest or identifier from canonical Norito bytes and regenerate the payload. |
| `SFS-VAL-002` | `validation` | Unsupported SoraFS schema version. | Upgrade the producer or validator so both use the same SoraFS schema version. |
| `SFS-VAL-003` | `validation` | Chunk profile handle or required aliases are incompatible with the registry. | Use a registered chunker profile handle and include its required aliases. |
| `SFS-VAL-004` | `validation` | Provider advert structure failed validation outside the more specific code families. | Regenerate the advert from governed provider metadata and inspect the `context` fields. |
| `SFS-VAL-005` | `validation` | Replication order structure failed validation outside the more specific code families. | Regenerate the order from governed manifest metadata and provider assignments, then inspect the `context` fields. |
| `SFS-VAL-006` | `validation` | Provider admission envelope structure failed validation outside the more specific code families. | Regenerate the envelope from governed provider metadata and endpoint attestations, then inspect the `context` fields. |
| `SFS-VAL-007` | `validation` | Provider admission digest binding failed. | Recompute the proposal and advert-body digests from canonical Norito bytes and regenerate the envelope. |
| `SFS-VAL-008` | `validation` | PoR challenge structure failed validation outside the more specific code families. | Regenerate the challenge from canonical manifest, provider, epoch, and randomness inputs. |
| `SFS-VAL-009` | `validation` | PoR proof structure failed validation outside the more specific code families. | Regenerate the proof from the challenged chunks and canonical authentication path. |
| `SFS-VAL-010` | `validation` | PoTR receipt structure failed validation outside the more specific code families. | Regenerate the receipt from the canonical timed retrieval observation. |

## CLI Exit Codes

| Exit code | Meaning |
|-----------|---------|
| `0` | Validation succeeded. |
| `2` | Validation, policy, signature, or Norito payload error. |
| `3` | Input/output error. |
| `4` | Command-line configuration error. |
| `10` | Internal fault. |

Exit code `0` can represent diagnostic success such as
`SFS-PDP-DIAG-000`. Production automation must additionally require the exact
acceptance code and `production_acceptance=true`; it must not branch on exit
status or `status: Ok` alone.
