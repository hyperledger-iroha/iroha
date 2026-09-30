# First-release merge resolution — September 29

This record concerns the merge of `2478995058` into `d7dfb1eaa8`, initially
containing 209 unmerged paths, and the subsequent merge of `5f96a983a3` into
`a828b9f26a`, which introduced seven further conflicts. All are resolved.
The latter merge was committed externally as `a43bfb38f1`; its tree exactly
matches the reviewed staged tree `34dbf83296b9791ead18faf9c5ae556d48099e23`.
This record does not supersede the separate
[earlier merge validation](merge-validation.md) or qualify a release.

## Decisions

- Keep the native Sumeragi core and node driver as the sole consensus path.
  Retire old V2 driver, QueuePlan, autonomous-lane and synthetic proofs used as
  substitutes for actual execution evidence instead of restoring compatibility owners.
- Execute signed genesis and successor proposals through the actual native
  executor, certificate and publication path in positive chain fixtures.
  Pure proof-codec, quorum and checkpoint-custody controls explicitly label their
  synthetic application results and do not qualify World execution.
- Keep raw Kura reads separate from authenticated execution custody. Strict
  canonical corruption checks preserve the original bytes and reject damage.
  The stable commit marker cannot be rewound by an unpublished temporary marker
  or reconstructed from damaged journals. Exact retained compaction/rewrite
  stages own recovery across interrupted file replacement.
- Keep consensus fault injection in the deterministic simulator. The separate
  private-settlement HTTP route controller uses a dedicated non-shipping daemon,
  exact command digests, protected files and serialized command revisions.
- Bind supervised snapshot export and storage-budget maintenance to successful
  native recovery, revoking publication after worker failure. Nonempty snapshot
  World restoration still requires original genesis-backed execution: current
  native results commit witnessed writes rather than the complete World.
- Source-bound release bundles contain the four shipping executables only.
  HTTP route-control, Parliament fixture and disposable-broker binaries are
  excluded from release bundle resolution.
- Preserve canonical Norito framing, domainless account identities and the
  current transaction payload, without retired admission-intent or lane-relay
  compatibility fields.
- Freeze FASTPQ execution context from the committed native lane incarnation
  and its anchor admission, rather than the retired physical lane catalog.
  Authenticated genesis execution failures are reported before schedule
  finalization inspects the rolled-back validator registrations.
- Keep physical application policy separate from native execution provenance.
  Require one physical route with the authenticated execution dataspace, including
  when physical and native lane identifiers collide. Batch transfers derive the
  same asset/account routing and policy selection as their original individual legs.
- Carry an authenticated genesis capability through stateful admission so bootstrap
  can run under mandatory fraud assessment. It binds the original header, signed
  transaction bytes and input position to empty committed history; ordinary and
  component execution retain the configured fraud policy.
- Validate ISO payment expiry in both the host clock and canonical millisecond
  domains before inserting admission records. Persisted timestamp readers
  reject unrepresentable times instead of overflowing.
- Register the original genesis peer topology before custom setup transactions,
  so validator staking executes against the signed, populated topology without
  duplicate registration instructions.
- Attribute deterministic rejection events only after authenticating the exact
  native proposal and committed instance, parent, result and epoch. The pristine
  execution writer rechecks the same context; local resource refusals and foreign
  or stale sources do not emit proposal rejection events.
- Derive Torii test identity from the retained ledger State and sign app requests
  for that exact network, retaining fail-closed network separation without generic
  fixture identity defaults.
- Execute captured release validators and their complete local code dependency
  chain from retained, verified bytes. Deterministic file replacement tests cover
  the interval after capture and before execution.
- Remove inert commitment, activity and pipeline diagnostic owners and their public
  DTOs rather than serving permanent zero values from retired consensus machinery.
  Queue diagnostics read the actual Queue owner; native lane status stays separate
  from physical catalog, portfolio and governance views. SDKs reject retired fields.
- Izanami retains the complete typed native status and committed-height delta instead
  of silently defaulting removed consensus fields to zero.

## Completed scoped evidence

Unless explicitly identified below as subsequent-merge evidence, these checks
ran before the second merge. Their pass counts do not qualify its additional
routing, configuration, cryptography or SDK changes.

| Check | Result |
| --- | --- |
| Unmerged index entries | 0 |
| Workspace production check | Passed after the native FASTPQ and diagnostics repairs; existing warnings remain |
| Configuration tests | 256 passed |
| Sumeragi simulator unit tests | 377 passed, 2 ignored after the final verification refactor |
| Sumeragi specification and production size | 3 passed; 7,998 of 8,000 production lines |
| Selected Sumeragi mutation gate | Baseline passed; all seven selected mutations killed, no survivors or errors |
| Deploy tests | 64 passed |
| Signature tests | 245 passed, 1 ignored |
| RAM program tests | 5 passed |
| Hash marker and schema regressions | Passed |
| Current OpenAPI and static contract scripts | 98 passed, including the actual native status corpus; all three JSON mirrors match canonical Norito rendering |
| Release bundle custody and shell tests | 55 passed |
| Release feature graph | 68 passed after current source review and seal update |
| Current native release inventory, gate and captured controller scripts | 382 passed, 4,443 subtests passed; 155 exact native test selectors |
| Captured release controller ownership | 134 passed, 142 subtests passed |
| Python Torii and native account corpus | 1,769 passed, 11 subtests with rebuilt ABI 25 installed package |
| Full current Python SDK corpus | 2,534 passed, 10 subtests after rejection of all three retired diagnostic fields; installed native wheel unchanged from the authentic finality/Nexus build |
| Nexus shared transaction fixture | Two native captures byte-identical; all 121 consumer tests passed |
| Prepared transaction fixture consumers | 243 Python and 59 C# tests passed against identical repeated native captures |
| JavaScript release schema boundary tests | 5 passed |
| JavaScript native build profile/provenance | 133 passed |
| C# release governance, current native bridge | 7 passed, no skips; pinned .NET 8.0.419 |
| C# current transaction and confidential bridge controls | RPC/TTL/fee 46 passed; retired-field/detached 11 passed; all four actual confidential bridge tests passed |
| Full C# SDK suite | 5,785 passed, no failures or skips; build has no warnings/errors; current ABI 25 host bridge and regenerated Rust fixtures |
| Fresh native bridge admission, bootstrap and transaction inclusion | 16 passed; shared KAGEMUSHA authority check also passed and reproduced the original fixture bytes |
| Current Python native finality verifier | All 4 genuine Rust execution/inclusion tests passed; installed canonical wheel also passed 3 positive and 10 hostile proof controls |
| Native one/four-lane SDK evidence | Genuine Kagami capture and no-update replay both passed; exact fixture bytes reproduced |
| Canonical RPC and coordinator fixtures | All 27 RPC entries regenerated and verified; fresh Rust archive comparison passed against the current receipt |
| Current Taira configuration fixture | Passed |
| Fresh profile generation and admission | 32 tests passed, including exact template spacing, profile-keyed transport identities and actual generated configuration admission |
| Full Kotlin/JVM and Java consumer runtime | 1,503 tests passed, no failures or skips; fresh Rust generator, canonical transaction/status/evidence/archive fixtures and current ABI 25 host bridge |
| Kotlin mandatory native consumers | 16 Java privacy/signer/SoraFS cases, 6 accelerator host controls and 4 confidential prover cases passed without native-unavailable early returns; accelerator CPU fallback allowed |
| Native model protocol/finality/checkpoint tests | 25 passed; lane admission 9 passed |
| Versioned wire identity tests | 5 passed |
| Izanami matrix scripts | 11 passed |
| Full private-settlement script corpus | 491 passed, 8,997 subtests; nine failures plus 16 failed subtests confined to the old scope-producer module loaded before its repair |
| Retained scope producer and collector | 19 passed, 26 subtests; full registered-plan retention, timeout denominators, and hostile mutation controls preserved |
| Current Core/Torii/CLI/Kagami/daemon/test-network/Izanami/integration test-target compilation | Passed with the private-settlement daemon and integration features; existing warnings remain |
| Fresh Core native fixtures / driver / snapshots | 16 / 56 / 176 passed |
| Fresh Core initial supply / original recorder | 1 / 1 passed |
| Current Core FASTPQ native source / physical policy | 10 / 2 passed |
| Current Core native node lifecycle and replay | Latest quiet run: 7 passed; autoscale workload stayed below its signed threshold, so the bounded signed-input fixture is being corrected before final qualification |
| Fresh Core Kura suite | Initial broad run: 546 passed, 9 fixture failures; final three publication-fence and staged-rewrite regressions now pass |
| Fresh Core block and SCCP hook selection | 302 passed after genuine native fixture migration, including all three sealed-execution regressions |
| Fresh Core output producer selection | Initial 89 passed; repaired periodic recorder-order regression now passes on the next native harness |
| Fresh Core batch routing and custom genesis staking | 4 routing tests and the actual signed-genesis topology/staking regression passed |
| Fresh Torii ISO and SoraFS controls | All 431 ISO tests and 4 SoraFS policy/revocation/native-manifest tests passed |
| Broad current Nexus route corpus | Initial run: 131 passed, 22 failed in readiness, stream authentication, route authority, fee-quote and staking fixtures; corrected fixtures await the next harness |
| Fresh Taira application and signed-query routes | All 38 faucet/onboarding/contract tests and signed-query regression passed after fixture identity migration |
| Current diagnostic consumers | Model roundtrip, strict rejection and schema identity/census 6 passed; Rust client 11 passed; CLI summaries 3 passed; Izanami typed native status 1 passed; Kotlin and Java HTTP consumers 10 passed |
| Mochi replay fixture custody | All six fixtures captured by the actual Rust owner and checked in place; diagnostics fields removed, two JSON files changed key order only; 3 ordinary integration tests passed |
| Mochi current lane and status views | 5 focused GUI tests passed for physical catalog, governance, DA cursors and current metrics |
| ZK attachment routes | All 5 tests passed after using the actual durable-store initializer in the router-only fixture; original signing, replay, deletion and unavailable-store assertions preserved |
| Fresh daemon configuration, genesis and snapshot controls | 62 passed |
| Test-network private route controller, feature isolation and release custody | 17 passed with exactly four shipping programs and separate Taira binding |
| Manual frame identity | 11 passed, 3 explicit capture tests ignored |
| Retired-codec guard | Passed |
| Workspace Rust formatting | Passed |
| Maintained historical archive | Verified 64,736 records and 67,311 occurrences |

These results are scoped to their recorded checks and source point. A complete
workspace test run, current full release qualification, hardware qualification
and Swift native bridge parity are not established here.
The available Swift XCFramework advertises ABI 21 while the current SDK requires 25.
The rebuilt host bridge exports ABI 25 and passes the affected C# runtime suite;
it does not substitute for the complete Apple artifact.

The Rust compilation checks used the current workspace graph and the explicit
private-settlement test features:

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental -- check --workspace --keep-going
scripts/cargo_fast.sh --stable-local-metadata --incremental -- check \
  -p iroha_core -p iroha_torii -p iroha_cli -p iroha_kagami -p irohad \
  -p iroha_test_network -p izanami -p integration_tests \
  --features irohad/test-network-private-settlement-route-control,integration_tests/atomic-private-settlement-release \
  --tests --keep-going
cargo fmt --all --check
bash scripts/check_no_legacy_codec.sh
```

## Validation in progress

The second merge preserves original native execution fixtures, the active native
lane-incarnation map, authenticated queue diagnostics and strict SDK wire cuts.
Private-dataspace routing now returns an explicit refusal when no active fixed
lane exists; every caller must handle that refusal without routing privately
scoped work onto the global lane. The incoming configuration, identifier and
RAM-LFE changes require fresh evidence.

The tenth combined build overlapped the external merge and is invalid as current
compilation evidence. The eleventh build captured identical file trees before
and after execution, while recording the external HEAD change separately. It
found six real Kagami caller errors after the routing return type changed to
`Option<LaneId>`; those callers require repair before qualification.

Subsequent-merge scoped checks so far:

- All three OpenAPI mirrors are identical (3,762,213 bytes, SHA-256
  `83f2f03ed991840f19c2f6b8d2749fa04b759e34aa08a4d5f63b18e443f3fff3`).
  The five OpenAPI/static-contract suites pass 98 tests.
- JavaScript native build-profile tests pass 123 tests; this is build-owner
  coverage, not evidence of a newly built native module or the full SDK runtime.
- Python identifier and strict-wire controls pass 28 tests.
- The immutable eleventh-build harnesses pass model fixture controls (4), CLI
  ballot custody (35), bridge Parliament custody (8), CLI dataspace/definition/
  operator controls (116), and Mochi current lane/status views (5). Actual model
  captures reproduce the original chain, network and both checkpoint byte strings;
  this remains structural checkpoint evidence, not World execution qualification.
- Core native source-context (15), routing (3), physical-policy (2), and merge
  (5) tests pass. The private-dataspace runtime test passes with original signed
  activation, certified private input and actual global merge. Inventory tests
  pass 94 and fail five fixtures missing original signed routing context; that
  context is being bound before signing, without changing producer validation.
- Current Kotlin identifier/RAM-LFE/Java-consumer scopes pass 117 tests without
  skips. Current C# source passes 5,865 tests without skips against the existing
  host bridge; this does not establish a new native artifact's source provenance.
- Compile-only Swift checks pass all 160 shipping source files against the
  maintained C header after completing the unavailable-encryption error case.
  This is neither Apple framework publication nor Swift runtime evidence.
- The combined FASTPQ build, panic-boundary and OpenAPI-call suites pass 124
  tests and fail the current panic-boundary inventory check. Its stale inventory,
  four undeclared executor support files and missing bootstrap issuer test module
  require source review and repair, not an exception to the guard.

Fresh native fixture, private-dataspace, API, configuration and SDK runs remain
in progress. The final Apple artifact and JavaScript native build must be produced
from their final frozen source inputs; older native binaries do not qualify the
new cryptographic boundary.

Native full-body transport does not supply the required
signed RS16 PayloadManifest/PayloadChunk availability proof; that qualification
must fail closed until the actual integration exists, as recorded in
`specs/sumeragi_goals.md`.
