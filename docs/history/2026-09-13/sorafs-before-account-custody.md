# SoraFS before the account-custody checkpoint

Exact displaced source excerpts; historical counts do not qualify later source.

Source: `status.md`. Excerpt SHA256: `acad78276ac1c3075a87859d23e6dd46b3f17472f8fdce8632e8b8496e26e5e8`.

SoraFS follows the [V1 implementation goals](specs/sorafs/v1_implementation_goals.md)
and the [native authority checkpoint](specs/sorafs/v1_closure_ledger.md#native-deployment-authority-and-schema-checkpoint).
The native Check checkpoint passes **5,031 tests** across complete Manifest, DataModel,
Executor, ExecutorDataModel and SchemaGen libraries; six DataModel manual fixture
printers are ignored. Another **99 Core, 45 Torii, 122 daemon and two SCCP tests**
pass on unchanged captured inputs and binaries. Check authenticates an exact fresh
signed transaction and successful result, then rechecks custody and account/role
permissions at one authenticated applied cut. The schema has 1,711 descriptors
and three byte-identical generator runs. The subsequent shared receipt-reader,
whole-lifecycle exclusion and native request-digest ownership changes are applied;
their focused follow-up passes **102 Core, 45 Torii, 132 daemon and two SCCP tests**.
The subsequent finite UTC interval check passes **106 Core, 45 Torii, 132 daemon
and two SCCP tests**, with 8,140 captured inputs unchanged. It verifies both
endpoints at one native cut; the actual clock still needs independent qualification.
The native signer authorization follow-up passes **661 runtime tests** (106 Core,
70 Torii, 483 daemon, two SCCP) and **970 release/CI contracts**. It enforces
one canonical role predicate and independent network before provider I/O.
All 656 prior runtime names plus five broker I/O/fixture regressions pass;
8,147 captured inputs, binaries and eighteen runtime controls stay unchanged.
CI selects the same names and all 38 mandatory sentinels. Earlier socket and I/O
failures and their corrections remain recorded in the closure ledger.
All **30 cosign cryptographic cases**
pass with the pinned executable; Linux qualification remains a CI step. The broader
promotion contracts pass 864 cases with two unfinished-source inventory failures;
29 omitted cryptographic cases pass in the explicit-verifier follow-up. Release
owner contracts pass 927 cases, including all 27 CI sentinels. Source-budget findings and Kagami's 92 existing
warnings remain. Actual hardware, native submission/state adapters, independent
account-key/clock/floor authority, full workspace/SDK and four-validator checks,
all 17 genuine lanes and load/soak remain open. No goal or lane is closed by this
checkpoint. Previous counts remain in the [historical record](docs/history/2026-09-13/sorafs-before-native-check.md).


Source: `specs/sorafs/v1_implementation_goals.md`. Excerpt SHA256: `a07bc59c254bbfff75e9511abac79077dfd0acb1d281fc78441ee12894b1e287`.

## G02 checkpoint — 2026-09-13

The [native Check checkpoint](v1_closure_ledger.md#native-deployment-authority-and-schema-checkpoint)
passes all 5,031 tests across the five complete Manifest/model/executor/schema
libraries, plus 99 Core, 45 Torii, 122 daemon and two SCCP tests. Six DataModel
manual printers are ignored; no selected runtime case is ignored. Captured inputs
and binaries remain unchanged. The schema has 1,711 descriptors and three
byte-identical canonical generator runs. Check authenticates the exact signed
entry/result, floor/committee continuity and current same-cut custody/permissions.
The subsequent private receipt-reader, nonblocking whole-lifecycle guard and shared
native request-digest changes pass a fresh **281-test** runtime follow-up
(102 Core, 45 Torii, 132 daemon and two SCCP), including all 13 new regressions.
The UTC interval follow-up passes **285 tests** (106 Core, 45 Torii, 132 daemon,
two SCCP), with all 8,140 captured inputs and binaries unchanged. Both finite
endpoints are checked at one native cut. All 27 CI sentinels and 927 release/owner
contracts pass; this does not independently qualify the clock or production source.
The subsequent native signer role/network hardening passes **661 runtime tests**
(106 Core, 70 Torii, 483 daemon, two SCCP), including all 656 prior names and
five broker I/O/fixture regressions. All 8,147 captured inputs, eighteen
runtime controls, binaries and build/baseline metadata stay unchanged. CI selects
exactly the same names and all 38 mandatory sentinels; **970 release/CI contracts**
pass. No request-time provider call occurs for wrong role, shape or network.
The separate final-promotion account custody and real hardware remain open.
Full workspace/SDK, strict lint and four-validator qualification remain open.

Final promotion uses one Ed25519 role and deployment purpose. The producer pins
the exact canonical reviewed statement before state/provider I/O; sign/recover
accept no replacement bytes. Fresh custody, audit and original completion fences
remain mandatory. The CLI is an offline receipt consumer. Source review identifies
separate account-signer authorization, independently qualified clock and rollback-
resistant floor authority as unfinished production boundaries. Existing software
roles, host time and local high-water files do not qualify those requirements.

Actual hardware and native transaction/state adapters remain unimplemented. Wire
the tested purpose-native Check consumer into the production source and qualify
all trust inputs and underlying hardware signer profiles together. Historical
private packets, local tests, prepared interfaces and an outer hardware signature
do not qualify deployment custody, all 17 genuine lanes or the 24-hour soak.


Source: `specs/sorafs/v1_implementation_goals.md`. Excerpt SHA256: `375e51b1af6f823ce3659bf6c284ea39ecaa1581ff9d4be93c5c1a93b9b3586a`.

   the separate account key, clock and retained floor, and use the same-lease
   receipt reader without staging rights.

Source: `specs/sorafs/signer_production_authority_inventory.md`. Excerpt SHA256: `a6c988ff15b56ad47ed4c6dc6436ee6c1695bead4239fbb8ba53f76db42ca28d`.

final-promotion transaction account requires its own capability and independently
verified key custody. It cannot use the role-14 operation to sign its own Reserve
transaction, because that operation already requires a finalized reservation.

Source: `specs/sorafs/final_promotion_native_authority_v1.md`. Excerpt SHA256: `83d945e660c8c4d429f49f811df0a7e42cd9373a10df356aa5935083c849845c`.

The Core consumer now owns a move-only pending challenge, exact signed External
bytes, a bounded monotonic deadline and the actual `Arc<State>`. It authenticates
aligned successful execution and canonical committee successors before sampling
one `FinalPromotionEligibilityTimeIntervalV1` and rechecking the current applied cut.

