# Canonical model enum codecs after the September 22 merge

The merged KAGEMUSHA beacon-binding and scheduling-decision declarations did
not compile: the schema derive rejects named enum fields, and JSON enums require
explicit tags. The correction gives the installed beacon one public
`InstalledBeaconEpochBindingV1` body, held by
`BeaconEpochBindingV1::Installed(body)`. Schema, JSON and binary codecs derive
from that one declaration. The sole Core circuit caller reads the same fields
from that body.

Both enums use a closed, snake-case `kind`/`value` JSON envelope. Installed
beacons carry `session_id` followed by `transcript_hash`, each represented by an
exact 32-byte numeric array. Bootstrap and the four scheduling decisions carry
null values. Unknown tags, extra fields, missing values, duplicate envelope
fields and malformed fixed-width byte arrays are rejected.

This is the sole first-release representation. Fixed-v1 bare Norito encoding of
an installed beacon is 71 bytes: the four-byte discriminant 1, a compact body
length of 66, then two length-prefixed 32-byte fields. The tuple body introduces
one outer length byte relative to the previously unbuildable declaration. No
old-layout decoder, alias or additional ABI version is introduced. Bootstrap
remains discriminant 0, and scheduling decisions remain discriminants 0–3.

The signed authorization identity hashes its explicitly ordered fixed-width
fields, independently of the enclosing Norito body framing. Its SHA-256 golden
remains `c44b44f659b5854ccdcd49bbaac9a0a4d26410627ec92ab593225e79042541d0`.
Five new tests cover exact JSON and bytes, binary/JSON roundtrips, malformed input,
schema ownership and field order, discriminants and the complete authorization
digest. The supported payload-body design also avoids parallel manual schema or
JSON implementations.

The first coordinated seven-package build reaches the corrected enum derives,
then fails on five required `Copy` implementations in the parallel staking model.
Its 160.72-second source-sealed failure is retained as
`dist/sumeragi-main-work/generation167-checkpoint-binding/checkpoint-build5/`.
No tests executed in that build.

The next source-sealed build (`checkpoint-build6`) reaches the migrated model
but fails with eight SCCP fixture errors in 160.59 seconds. Those callers still
used the removed epoch-roster layout. The failures are retained; neither build
ran tests. The subsequent consumer migration separates immutable key generations
from scheduling authorizations and uses the new fallible
`KagemushaMintFinalityEpochAuthorizationV1::genesis(&authority, last_height)`
constructor. It validates the complete authority and canonical genesis interval;
authority and authorization identifiers remain distinct. Successor fixtures
inherit exact records or validate explicit contiguous retention/activation.

Signed genesis now carries one generation-zero template. Its NPoS initial epoch
must span at least three heights: the boundary follows a pulse anchored in a
committed positive-height block. This is a protocol shape minimum, not evidence
that an initial ceremony can complete within three blocks. Operator bootstrap
checks retain their larger execution-window requirements.

The nine-package `checkpoint-build7` stops after 76.56 seconds at the genesis
registration caller, which omits the new required monetary plan. Captured Rust
inputs remain unchanged, and no tests execute. The repair requires the plan in
structured genesis JSON and preserves every field when encoding. Missing, null
and incomplete plans are rejected; the default template explicitly names the
prefunded source, configured escrow, quantity and genesis precondition. Candidate
fixtures bind all monetary-plan fields to peer consent. No implicit custody
destination or compatibility decoder is added.

Reward claim plans, state cursors, record references and source payouts reject
unknown JSON fields. Nullable prior-state fields require an explicit value,
including explicit null for absence. This prevents omission from silently
inventing the state covered by a signed plan. Two model regressions exercise
both JSON decode paths, every missing field, retired bound-only layouts and
canonical instruction roundtrips; their runtime qualification is pending.

Genesis JSON projection and its tests now occupy their own named module and
test file. The crate entry point falls from 6,232 to 4,131 lines; the new files
contain 1,195 and 865 lines. Module paths and test identities are unchanged.
The obsolete genesis size exception is removed. The complete source-budget
check reports no genesis findings, 168 remaining exceptions and 275 findings
elsewhere; the repository-wide budget gate still fails.

`checkpoint-build8` passes the previous genesis compilation boundary but stops
in SCCP/Core after 308.30 seconds, with captured inputs unchanged. Its diagnostics
identify the curve byte-representation wrapper, a foundational `PeerId` import,
the reward-cursor key migration, unavailable instruction constants and a denied
function-pointer cast. No test executable is emitted. These failures remain
recorded and do not count as runtime passes.

`checkpoint-build9` emits seven usable library test binaries but fails aggregate
compilation in the model tests and Torii. Exact emitted artifacts pass 922 SDK,
321 Torii-shared and 53 storage-client tests with ordinary stacks. The separately
owned Core review passes 327 controls. These scoped passes do not turn the failed
aggregate build into a passing candidate.

The Genesis artifact passes 115 tests, fails five and ignores four. The failed
template readers require explicit block time-invocation and execution-output
limits. All six shipped source templates now declare the bootstrap policy. Local
and Taira NPoS payloads explicitly bind their known canonical XOR definition;
public Nexus still requires an operator-supplied canonical identity. The dev
template's asset registration, funding and all four signed monetary plans use
the configured fee/stake XOR and escrow. New tests check these exact identities,
funding order and every shipped execution policy. The build9 source capture
included the dev and Taira templates, but not all other runtime-read templates;
build10 expands that capture to all six.

SCCP's focused fixture selection passes ten and fails three: the finality
descendant fixtures sign empty proposals without declaring complete execution.
The corrected fixture installs the complete empty output set under explicit
limits, then verifies execution presence and unchanged resultless proposal
bytes. The production execution-result requirement remains enforced.

Torii's pending-reward projection now reads one lane/account processing cursor,
retained accruals keyed by exact custody asset, and subsequent records. It counts
retained dust once, distinguishes an absent cursor from processed epoch zero,
and rejects a historical cutoff before the retained cursor. Tests cover separate
owners/scopes, filtering, malformed orphan accruals, zero amounts and JSON cursor
semantics. This projection reports unpaid entitlement before the payout dust
threshold; it does not prepare or authorize a claim.

TODO: Qualify these corrections in the next captured build and runtime batch.
The parallel staking model has its own tests in
`nexus/staking/monetary_codec_tests.rs`; canonical fixture regeneration remains
pending an emitted current model test harness.
