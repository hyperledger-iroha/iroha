# Election, signer, provider-ingest and JavaScript checkpoint

Work stayed in `/Users/takemiyamakoto/devstuff/iroha` on `optimizations` at
`1b8e5f92b6dadb1dcdd16f945286560e31b347ff`. These are separate local
implementation and diagnostic scopes, not one frozen release candidate.

The V1 election shape now admits 2–64 options and requires exactly one tally
counter per option. The Rust data model, Core election application, IVM mock,
JavaScript builders, tests and standalone-election contract use the same bound;
the former zero-option error name is removed rather than retained as an alias.
The focused Rust selection reported 15 passes, and the JavaScript source/dist
browser selection reported three passes. This shape correction does not supply
anonymous credential authorization, confidential bonds, a committee-free
dropout-resilient tally construction, or a sound tally proof. The standalone
election release gate remains open.

The daemon has an opaque software credential key-operation provider. It reads
a canonical private-key credential through the owner-only, no-follow runtime
reader, verifies the enrolled public key and custody record, and reobserves
active custody and the exact reservation before signing. Four focused tests
passed for the enrolled key, wrong or unsafe credentials, lost reservation,
revocation and active-record substitution. Its state-source tests use a fixture:
production `SignerOperationStateSourceV1`, finalized operation/completion sources
and purpose dispatch for protected roles 11, 13, 14 and 15 are still absent.
No HSM is required for the remaining software signer integration.
The role-14 Current Check handoff now consumes Core's successfully applied and
finalized Check against its original State/Kura cut, checks the exact observer,
subject, custody and audit pair, then advances an independent durable floor and
resamples qualified UTC before returning a non-signing observation. Five
focused daemon adversarial tests pass. Both Core
`applied_check_without_durable_finality_is_rejected` selections pass on the
current source. Configured UTC and floor providers,
exact submission/reconciliation, and the complete production signer state
source are still absent; Reserve, Complete and signature release remain off.
An additional G02 audit found no SoraFS hardware-only admission in the
configured custody, daemon/Torii signing or promotion paths. Nine focused
Python evidence tests pass; current signer inventory and release-plan prose
now describe the software provider and the still-missing state source.
KAGEMUSHA's separate offline monetary-authority policy was not changed.

For SoraFS G06, Core now reads one assignment directly at an exact durable
archive key, including its provider root and canonical source-provider IDs.
The direct lookup reaches an order beyond the 1,000-row page limit and does
not move a worker page cursor. The daemon's private current-assignment lookup
checks the committed State/Kura key and archive generation before and after the
read, and rejects a changed revision, stale source, tampered pin or Musubi
binding, and an unavailable current head. Three focused Core lookup tests, one
existing corruption test extended to the lookup, and three daemon lookup tests
passed. The lookup returns no grant. The production admission, advert, pin,
token and revocation resolver, provider backends and distributed qualification
remain open. Production `cargo check -p iroha_core -p irohad --lib` passes.
Strict Clippy remains open: the unchanged vendor dependency stops the workspace
run, and a no-dependencies diagnostic reaches 217 existing daemon lint errors
after the Core pass under three narrowly identified baseline allowances.

A follow-on G06 daemon check binds the payload-free source request to that
current assignment's exact source inventory, nonzero revision, and Musubi row.
It brackets the already-issued-grant council/advert/transport evidence check
with two assignment reads plus an archive-generation fence. This returns only
validation, never a grant or readiness capability. The lower-height immutable
admission cursor is only a nonfuture bound; an independent finalized admission
service must prove its ancestry and current revocation before grant issue and
again at use. The correct `irohad --lib current_assignment_` selection passes
3/3; the separate otherwise-valid admission/advert but missing-finality
evidence selection passes 1/1. An earlier `--bin iroha3d` invocation compiled
but ran zero tests and is not counted. Shared DataModel/Core edits overlapped
the fresh dependency build, so these focused results are local provisional
evidence pending a coherent-source rerun. Admission/revocation, finalized
advert, governed origin/token-key/DER-root pins, bounded token grant issuance,
and a production resolver with use-boundary checks remain absent. The
target-only `first-release-sorafs-g06-https-grant-handoff-20260923.md` records
the exact backend contract; G06 remains open.

The first current macOS 27/SDK 27 native-addon build was rejected by dyld at a
misaligned LINKEDIT string pool even though `codesign --verify` passed. A
header-edited target-only copy isolated the SDK-tag behavior but is not a
publishable artifact. A genuine rebuild using the installed macOS 26.5 SDK and
deployment target 11.0 produced an SDK-26.5-tagged, signed `.node` that loaded
124 exports through Node; the published SHA-256 at that scope was
`8737a3adbdee1f49b8c8ea53cf0dd6d6687d36b55aa99ac733832c586dc56abe`.
The native build provenance predates the subsequent G06 and test-fixture edits.
The initial three-suite native-backed JavaScript run passed 183 of 206 tests;
23 failed because test inputs omitted the required explicit `networkPrefix`.
The two test files now supply canonical prefix 753; syntax, scoped ESLint and
diff checks passed. The first rerun correctly refused stale native source
provenance. That V3 scope did not qualify the revised JavaScript source; the
V4 build below supersedes it. The earlier binary and counts do not qualify
release packaging.
The target-only diagnosis is
`target/first-release-js-native-linkedit-diagnostic-20260923/DIAGNOSIS.md`;
the first TAP output is
`target/first-release-js-native-tests-20260923/combined.tap`.

Native packaging now uses strict V4 provenance. On macOS it records the actual
SDK root/version/settings digest, deployment target and selected Apple compiler
and linker identities, and changes Cargo metadata when that identity changes.
The V4 loader and publisher reject older V3 receipts. Its 250 focused
build/provenance/publication controls and 36 loader controls pass. A genuine
V4 SDK-26.5 rebuild completed in 17 minutes 41 seconds and published SHA-256
`62e20bffa8dfa2114bd77edfa1089914e4960348fbe6d16077ac67bd6103c4f8`.
The Mach-O declares SDK 26.5/minimum macOS 11.0, `codesign --verify` passes,
Node loads 124 exports, and the native-backed governance/proof and crypto
selections pass 206/206 and 34/34 tests. These are scoped local SDK checks;
the final immutable candidate, five native targets and installation smokes
remain open. Build and TAP logs are under
`target/first-release-js-native-v4-20260923/`. The
generated V3 binary and manifest were preserved under
`target/first-release-js-native-v3-retired-20260923/` with their original
SHA-256 values, rather than accepted by a compatibility path.

The promotion checker now binds the exact five inner approval files and all
17 ordered lane summaries to the positive replay manifest, replays the signed
lane inventory against those summary bytes, and validates the full signed
foundational envelope against reviewed sequence, predecessor and freshness
inputs. It checks independently pinned public signer tuples and the
foundational external signer receipt. A direct signed inner-chain fixture
passes those checks but cannot authorize promotion; tampered lane bytes and a
re-signed invalid prerequisite fail closed.
Private exact-byte snapshots use an owner-private temporary directory outside
the source tree; a source-tree temporary root fails before any file is created.
The Python 3.12 promotion, evidence and cosign selection passes 396 tests.
Purpose-owned native completed-operation and finalized-state verification for
the inner approvals remains absent, so the checker still rejects promotion.

A read-only confidential-asset audit found that an otherwise authorized
ordinary transfer or burn can debit Orchard or private-IVM public reserve
backing without consuming a pool transition. Its target-only
`first-release-confidential-asset-reserve-review-20260923` design records the
required exact reserve-owner index and debit fences. Activating a simple
derived `mv::Storage` guard would introduce unfunded physical allocations and
an incorrect consensus-invalid capacity path. The funded, transaction-atomic
State owner, configuration and local-retry channel must precede the guard.
No reserve code was changed, and authority/conservation remains a release
blocker.
One focused MV composition test now passes for original funded child rollback,
full-pool retry, detached capture and copy-free publication. Core does not yet
retain that owner through the World/State transaction lifecycle, so it does
not change the asset admission result.

The 18-step Apalache model and config still match the prior exact run. That
run ended without a pass after 411,980.3 seconds, at state 15; repeating the
same inputs would not supply new qualification. A target-only diagnostic under
`target/first-release-apalache-18-diagnostic-20260923/` records the pinned
tool hashes, command, internal VC bottlenecks and the next bounded SMT profile.
The formal gate remains open.

The MKHE ordered materialization handoff now revalidates original source
records and the sealed pair's exact context before returning its local
verified owner. A forged full cursor without source authority fails closed.
Eight focused handoff tests, one sealed-context test and one 9,288-coordinate
ticket/rho equality test pass. The last test uses synthetic upstream source;
full native40 source-to-pair execution and production composite admission
remain open.

A target-only FASTPQ geometry screen under
`target/first-release-fastpq-compact-geometry-20260923/` reproduces the
4,279,877-byte current maximal segment. A hypothetical 474,213-byte narrower
profile fits the byte target but fails the current soundness bound and raises
raw LDE memory to 2 GiB before scratch. A batched-opening route still needs a
new binding, privacy and qROM argument. Neither candidate authorizes a compact
proof or Core cutover; F07 remains open.

A read-only Native lane cutover map under
`target/first-release-native-prepayload-cutover-20260923/` confirms that the
inactive four-validator Native driver already handles a silent initial author
and produces a signed 3-of-4 Decision. Production still constructs the older
autonomous signer and rejects Native lane ingress. Starting only the
pre-payload timeout would create two safety authorities; the process-lived
driver, shared reducer, original Apply owner and retired signer must cut over
together. The map identifies the exact source seams and acceptance tests; F03
remains open.

BFV's arithmetic trace now commits each 32-byte statement hash as eight
little-endian `u32` Goldilocks limbs in a 38-column row. The retired four-word
modulo-field packing let different hash bytes map to the same trace limbs.
The collision/public-opening regression, governed-trace test and locally
signed-package rejection each pass one focused crypto test. The two-slot
canonical conformance material regenerated twice byte-identically at 890,808
bytes (SHA-256
`779b61f424f3d981f7096cc642745600b8695f60eb03883c8d0368b798e4bf2c`),
and its test passes. The dependent Core native-STARK fixture selection has not
yet run against this identity. Full BFV relation, eight-party and independently
audited parameter/noise/qROM qualification remain open; the production gate
still rejects.

The Node24 runtime input path now has local physical owners for original image
bytes, exact absent candidate leaves and literal symlink targets. They retain
no-follow ancestry, reject changed image or leaf facts, and are assembled under
a 2,048-descriptor preflight bound in a one-shot input scope. A failed scope
entry, partial construction, and changed exit now close the acquired originals,
and the focused custody/pure-input selection passes 133 tests. Physical alias resolution,
the fixed-child/EOF join and actual mapped-image observation remain absent;
the recorded local runtime and release gate remain rejected.
The release-automation plus custody selection passes 1,482 tests; the workflow
checker and shell syntax check also pass on this source slice. The full
`sorafs_javascript_*test.py` Python selection passes 779 tests.

All privacy, SoraFS, multilane, SDK, audit, deployment and promotion gates
remain open. This checkpoint adds no compatibility path or hardware custody
prerequisite.
