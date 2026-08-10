# SoraFS L1 lane-evidence inventory V1

This contract authenticates the exact 17 Taira qualification summaries without
copying evidence payloads, filesystem paths, credentials, or signer secrets into
the promotion record. It is a software-key-qualified contract. It does not make
an HSM-qualified claim.

## Input contract

Every phase receives exactly 17 `--summary LANE=PATH` arguments in this order:

1. `ai_prescreen`
2. `appeal_finance`
3. `gateway_compliance`
4. `gateway_load`
5. `governance_dag`
6. `hedging_billing`
7. `moderation_panel`
8. `orderbook`
9. `pdp`
10. `pop_credentials`
11. `por`
12. `potr`
13. `reference_sdk_release`
14. `repair`
15. `reputation`
16. `reserve_rent`
17. `transparency`

Each input must be a bounded, stable, single-link regular file reached through
direct directory components. Every ancestor and the final component are opened
relative to an already anchored directory descriptor with `O_NOFOLLOW`. A
platform that cannot guarantee that traversal fails closed. JSON must be sorted,
indented canonical JSON with
exactly one terminal LF; duplicate keys, aliases, trailing bytes, non-standard
constants, a missing lane, an extra lane, a duplicate, or a reordered lane fail.

The summary must use its lane's exact V1 schema, report `status=ready`, have no
errors, and contain at least one valid recognized artifact. Every recognized
artifact binds the selected deployment ID and environment, records an explicitly
reviewed deployment context, and has a positive `generated_at_unix` no more than
24 hours old. All deployment context occurrences must agree.

Every lane also carries the same exact topology binding. The binding uses network
`taira`, chain ID `fc56984b-2be7-431d-840e-21514d1883f0`, chain discriminant
`369`, and non-zero SHA-256 digests for the topology summary, source manifest,
canonical manifest, and ordered validator IDs. Any Minamoto value is rejected;
Minamoto read-only observations never enter this inventory.

Every phase also receives those four topology digests as explicit
operator-trusted `--expected-*` arguments. Merely making all lanes agree on an
attacker-selected or stale anchor is insufficient.

The environment is hard cut to exactly `prod` or `production`; labels such as
`qa`, `staging`, `local`, or `test` are not qualification environments.

## Inventory and signature

The schema is `sorafs.l1.lane_evidence_inventory.v1`. It contains only:

- the exact Taira deployment tuple;
- the shared topology digests and evidence time bounds;
- 17 ordered rows containing the lane, expected schema, exact summary-byte
  SHA-256, recognized artifact count, and timestamp bounds;
- `summary_file_count=17` and `recognized_summary_count=17`; and
- the authenticated external signer binding and detached signature.

The signer binding is schema closed. It requires role
`l1-lane-evidence-inventory`, service kind `authenticated-external-signer`,
algorithm `ed25519`, backend `software`, distinct production service and
administrator identities, positive key and policy revisions, a non-zero policy
SHA-256, and the SHA-256 fingerprint of the operator-trusted public key.

The detached signer signs:

```text
"sorafs:l1:lane-evidence-inventory:v1\0" || canonical_compact_json(unsigned_inventory)
```

The tool accepts a public verification key and a detached signature only. There
is no private-key, seed, or signing-key argument. `finalize` and `verify` both
reopen all 17 summaries, recompute their exact byte digests and time bounds, and
compare the complete unsigned inventory before accepting the signature.

## Promotion integration

Production readiness must treat this inventory as an additional mandatory signed
input. It must independently supply the trusted public key, signer identities,
revisions, and policy digest; compare the ordered inventory rows byte-for-byte
with the 17 summaries it recognized; and copy only the inventory SHA-256 and its
payload-free validation into the aggregate.

The nine-prerequisite envelope must bind the signed inventory SHA-256 in addition
to its exact-cover lane digests. Promotion must reject any equality between this
inventory signer's service ID, administrator ID, or public-key fingerprint and
the corresponding topology, resilience, or promotion signer values. Distinct
text labels alone are insufficient when a key fingerprint is shared.

Two independent verification runs use the same explicit `--now-unix` and input
bytes and must produce byte-identical verification JSON. Changing the evaluation
clock is a new qualification run, not the same deterministic replay.
