# First-release merge validation

This record covers conflict resolution between `a3973c833e` and
`8a99f3f5ba`. It records local checks of the merged working tree, not a release
qualification or an attestation of either parent.

The resolution retains the current SCCP design, native Sumeragi status, the
canonical masked FASTPQ producer and source-occurrence checks. Retired IVM
replay-binding proof services, key generation commands and synthetic lane-relay
proof generation remain removed. No compatibility decoder or API alias was
introduced. Borrowed byte serialization now shares the canonical owned-byte
layout without copying private material.

KAGEMUSHA OpenAPI and maintained SDK projections require the current network,
release purpose, experiment scope and app-attestation policy fields. The three
authored OpenAPI copies also carry the current native committee and BFV profile
schemas. Negative controls reject missing fields and invalid purpose variants.

## Local checks

- Workspace metadata resolves all 101 members with no missing target sources.
- Workspace formatting and the retired-codec checks pass.
- FASTPQ library, tests and binary targets pass `cargo check` with default
  features and with `fastpq-gpu,dev-tools`. The optional build uses its existing
  runtime Metal source-compilation fallback because the offline Metal toolchain
  is absent; this check does not establish GPU execution parity.
- All four focused Norito borrowed-byte tests pass, including complete named
  frame equivalence, zero-copy decode, truncation and sequence-limit checks.
- All 17 Norito codec and scratch-retirement source-contract tests pass.
- The focused Python tooling selection passes 95 tests and 41 subtests:
  OpenAPI contracts, static contract assets, shared Halo2 circuit contracts and
  immutable CLI harness lifetime.
- JavaScript bundle/schema/retired-verifier checks pass 25 tests; the focused
  Python governance schema/SCCP checks pass 4 tests. Swift syntax parses.

## Remaining qualification limits

The combined Core/Torii/CLI library, test and binary check with
`iroha_core/zk-tests` does not pass. After repairing the SCCP TON export mismatch,
Core's production library reports 23 errors from the unfinished consensus
cutover: retained queue/Kura/State/Sumeragi owners import deleted v2 and merge
modules, and `LaneExecutor` lacks the current application-control methods.
The missing modules and their retained imports coexist in incoming commit
`8a99f3f5ba`; the lane trait and incomplete implementation also match that parent.
Retired implementations and compatibility aliases were not restored. Torii/CLI
validation cannot be claimed through this failed dependency build.

The broader release-script run exposes stale selections of removed consensus
and lifecycle tests and missing selections of current beacon tests. These
guards remain enforced; passing narrower tests does not qualify the release
gate. The source-file budget and panic-recovery inventory also retain broad
pre-existing drift. Only the reviewed conflict and directly affected source
contracts were reconciled, rather than accepting every current source hash as
audited or increasing every budget.

Full Swift tests require the native XCFramework; native SDK tests require
current bridge artifacts; Kotlin execution requires a JDK. Full workspace
tests, real-network qualification and release signing are not established by
this merge resolution.
