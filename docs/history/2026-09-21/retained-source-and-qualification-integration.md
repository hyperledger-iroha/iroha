# Retained source and qualification contract integration

This is development-source evidence for the active first-release integration
branch. All fourteen [release goals](../../../specs/first_release_completion_goals.md)
remain open. No immutable candidate, production proof admission, independent
audit, deployment qualification or promotion is established here.

## Original mask and storage owners

The first mask-block producer consumes the original retained source and RNG,
uses a distinct mask counter, and retains its memory credit with the sampled
preimage and encrypted file. The same original sealed-pair storage owner reserves
209,920,000 file bytes and 419,840,000 write/seal I/O bytes before opening handles
or consuming entropy. The first 16,384-coefficient block writes eight encrypted
records, totaling 131,200 bytes. Pre-I/O capacity refusal returns the original
owner; later error/unwind consumes the attempt and erases retained secrets.

The 22-file packet is recorded in
`target/first-release-owner-and-fixture-integration-applied-20260921/identity.json`.
The first native run found a test which incorrectly unwrapped a 16,385-element
chunk rejected by the real constructor. Its repair asserts that exact rejection;
the existing limit is unchanged. The rerun is retained under
`target/first-release-qmask-first-native-2-20260921`: production check and native
build pass, followed by 16 mask-block, 5 original-entropy, 12 retained-owner and
8 ordered-storage tests. The retained-owner suite takes 569.82 seconds.

The wrapper then required seven tests from a module containing five. It stopped
before execution; this is not a native failure or a passing collection. The
separate `resource-budget-continuation` executes all five actual tests and passes.
All these observations use the same copied binary, SHA-256
`70a8ad334585f1fc818c12b1ee2d732206858b9a71b9051ebbe75caa3b6cf31a`,
with unchanged observed source, ordinary stacks and two test workers. A subsequent
patch removes one unused outer memory-type reexport without changing behavior.

At this first-block checkpoint, the producer ends after the first stored block.
The subsequent four-opening integration is recorded below. The complete mask,
sealed root and qPCS admission remain open. Source38 ownership is not Native40 production authority. The reservation
does not qualify whole-proof resident memory: nested allocator overhead, paths,
dependency scratch and peak RSS remain separate obligations. The existing qPCS
work-bound discrepancy remains a design blocker.

## Retained governance and retired fixture surfaces

The same exact-image integration record includes a seven-file governance fixture
migration, thirteen retired-election fixture/tool changes and one inactive-key
fixture. The governance helper retains actual candidate journals, authenticates
exact three-of-four Commit QCs, persists Kura evidence and publishes through the
original owner. The intended three-candidate scenario is not yet passing: lane
predecessor admission blocks its continuation, as recorded below. Unit admission is explicit and does
not claim funded production admission. The tests retain slash, restitution,
replay and balance-conservation assertions.

Development vote-membership artifacts now use the single
`zk-dev-vote-fixture` command. The retired command has no alias. Raw IPA
verification and malformed-proof controls remain, while production election-key
registration remains closed. Fixture metadata explicitly records
`production_admissible: false`; no generated development key supplies election
authority.

The initial rebuilt Core harness fails at six compile errors in current test
sources: two private Kura accesses, two private asset-map accesses, a moved
comparison baseline, and a retired permission import. The unchanged-source log
is `target/first-release-governance-fixture-native-20260921/07-core-build.log`.
The six compiler failures are repaired using existing borrowed getters/test-only
map access, an owned comparison baseline and the current permission contract.
The next build passes; the runtime results below retain the remaining failures.

## Published qualification contract

The preceding combined native run passes production checking, test compilation,
22 confidential-spool, 13 u15, one original-owner surface, 24 topology-reducer and
12 snapshot-buffer controls. OpenAPI passes 126 of 127 tests; the failure exposes
the missing `PrivacyExact12QualificationRecordV1` published schema.

The three canonical OpenAPI mirrors now include the native qualification family:
six tagged enums and sixteen strict records. The capability manifest requires its
explicitly nullable qualification field. Four new native tests compare actual
Norito JSON record keys, enum shapes, inventory cardinalities and null semantics.
The packet is `target/first-release-privacy-qualification-openapi-20260921`.
The mirrors agree and compaction controls pass. The next native build and all
four new qualification controls pass; one canonical-byte rendering test fails
and is repaired below. Schema structure alone does not authenticate signatures,
cross-links, audit findings or deployment evidence.


## Native build and remaining runtime failures

`target/first-release-governance-qualification-native-2-20260921` records a
989.28-second successful Core/Torii/group02/group05 build against unchanged
observed inputs. The original governance carrier selection passes three tests
and fails `double_vote_slashes_plain_lock`: the normal lane planner returns an
unavailable predecessor. The retained publication helper does not yet join the
lane certificate/application-receipt frontier needed to extend that route.
No planner bypass, fabricated application receipt or second execution is added.

The independent continuation uses the same copied binaries and unchanged input
manifest. Results: ballot fixtures 13/13, key status 1/1, development proof audit
8/8 and lock refusal 1/1. Four selections remain failed: vendor latch 0/1,
synthetic STARK rejection 0/1, root-cap 1/2 and OpenAPI 130/131. The root-cap
selection sets its existing opt-in, so it executes the behavior rather than
returning early. The four new qualification OpenAPI tests pass.

The observed fixture prerequisites are corrected: Kotodama requires the named
`value` argument; non-genesis asset registration needs its actual owning domain;
and the STARK sample verifier/parameters need the existing consensus n-log2
floor of 10. No production authority or resource floor is lowered. The OpenAPI
failure contains equal parsed JSON values but noncanonical key ordering, Unicode
escapes and a trailing newline. Actual native Norito rendering from that failed
comparison supplies the corrected byte order, followed by the exact reviewed
consumer tag/description change. All three mirrors now have SHA-256
`c877faafbe61ea80cefde328abd62398209dd57a9ea26c9d474e2cdce9538138`.
Fresh native tests must still verify these changes.

## Four opening tickets and the canonical SDK consumer

The exact 34-file integration is recorded in
`target/first-release-opening-and-sdk-integration-applied-20260921/identity.json`.
It joins the reviewed 17-file first-opening producer, ten-file SDK contract and
seven-file helper split. The helper split compiles key/election helpers only for
the test consumers that use them; it adds no warning suppression or reexport.

The opening producer consumes the original S preimage, RNG, encrypted file,
canonical table and proof ledger. It installs four true fifteen-bit commitment
openings at inventory positions 27,176 through 27,179, retaining the original
zeroizing rhos. Four lower evaluations and rho storage are reserved before any
rho draw. Only pre-rho capacity refusal returns the original owner; later failure
consumes the attempt. It does not complete the remaining 6,396 S openings,
complement openings, pre-z transcript, qPCS, governed native40 or a whole proof.
`target/first-release-qmask-openings-native-20260921` records the current scoped
native run: production checking (3.24 seconds), compilation (25.42 seconds),
and all 43 selected tests pass with unchanged observed inputs and ordinary
worker stacks. All eleven new controls run: seven opening equations/failures,
two lower pre-admission controls, one actual file binding and one retained-owner
binding. The broader selection includes eight original sampler, eight lower u15
(including the two new controls), five entropy, eight storage and five budget
tests. This is component evidence, not full-S or whole-proof qualification.

The native consumer is now `JavaSourceKotlin` / `java_source_kotlin`, with explicit
binary tag 10. Retired binary tag 2 and JSON `java_android` have no accepted alias.
All ten evidence rows remain required. The existing native qualification record
is now an explicit schema-generator root, with actual transitive-closure tests.
A real distinct Java-consumer qualification artifact must still bind both JVM
and Android execution to the exact Kotlin/native packages; a renamed label does
not produce that evidence. SCCP's retired Java implementation and SoraFS's
separate six-package evidence owner are being migrated separately without
removing their original assertions.

The combined build in
`target/first-release-canonical-qualification-native-20260921` passes in
1,088.79 seconds with unchanged observed inputs. Its launcher then stops because
the SDK selector collects one schema test instead of the three consumer tests.
This collection failure is retained; no unexecuted test is called a pass.

`target/first-release-canonical-qualification-native-continuation-20260921`
uses those exact copied binaries and unchanged source with the actual SDK
selector. The selections report **172 passed, seven failed, none ignored**:
the three SDK identities, four schema root/closure checks, all 132 OpenAPI
checks, both real root-cap checks and ten key/lock/audit controls pass. The
complete STARK module passes 18 of 23; stale key lengths, an execution circuit
label and obsolete development-ballot acceptance account for its five fixture
failures. The vendor-latch assertion and the known governance lane frontier
also fail. All failures remain recorded; production admission is not relaxed.
The subsequent xtask build fails in 688.62 seconds: its developer feature does
not enable Torii fixture ownership, and config-only lane maintenance calls five
retired alias-derived path APIs. The two reported unused imports are also
preserved in the failure log. No xtask tests or Kagami generation execute in
that run. A reviewed first-release repair removes config-only archival and its
flag, inventories canonical opaque instance paths without inferring retirement
authority, and enables Torii fixtures only for developer tooling. Fresh native
validation of that repair is pending.
This remains dirty development-source evidence; full native packaging and
immutable candidate qualification are still pending.
