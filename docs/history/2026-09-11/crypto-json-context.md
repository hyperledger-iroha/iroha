# Crypto JSON context and sequence ownership

This is isolated-candidate evidence for the first-release JSON API migration.
It does not qualify the live workspace or release. The preceding
[ordinary Value destruction correction](json-value-destruction.md) remains the
owner of stack-safe destruction.

## Implementation

Crypto scalar, key, Merkle and handshake owners now use the single checked JSON
writer, explicit context and native borrowed-value decoder. Fixture consumers,
benchmarks and developer tools use the canonical calls. Valid field order,
signing inputs, binary identities and shared fixture bytes are preserved.
Private keys remain redacted unless explicitly exposed; streaming secret text
uses zeroizing temporaries. Keypair decoding invokes its validating constructor.
Invalid Merkle trees return a typed writer error instead of fabricated output.
BFV digest decoding validates the fixed encoded length before hex syntax and
decodes into its fixed byte array; valid encodings and field validation remain.

The Merkle regression found duplicate sequence admission: the bounded owner
preflighted an array and then called a vector decoder that preflighted it again.
`SeqVisitor` now owns the admitted count and exact remaining vector allocation;
`Parser::parse_array` delegates to that owner. Partial collection, skipping,
empty input, resource failure and ordinary destruction share this implementation.
Count inconsistencies fail without growing beyond admitted capacity. The single
canonical trailing-comma diagnostic replaces the prior vector-specific error.

One preflight planning unit per element is a cooperative budget charge, not
proof of heap allocation. Oversized Merkle input is rejected before leaf decoding
and storage allocation after that admission. Tests compare bounded and ordinary
vector charges and exercise exact/short budgets. Final Merkle tree/queue and
backend allocation accounting are still outstanding.

## Source-scoped checks

At source `8da18c82882663294414f808cfd79510a7f0c0b46e04e93d7717ad1784523e5a`,
the complete crypto target build passes without diagnostics. The selected crypto
library cases and complete integration/tool harnesses pass 428 tests with one
existing ignore. The four-package shared codec selection passes 1,868 tests
across 18 executables with one existing ignore, including both compiler UI suites
and all six new sequence tests. Formatting and the retired-codec guard pass.
The original failed Merkle regression is retained as `crypto-context-runtime-1`;
its corrected run is `crypto-context-runtime-2`.

Subsequent test-only cleanup resolves strict Clippy findings through clearer
fixture identifiers and focused schema/conformance helpers, preserving every
original test name and literal and all 79 ordered BFV schema assertions.
At source `c901d2012a81266b70251c72407862c1eb2023e9455c707d3e6a75011401e4c0`,
the crypto all-target build and five-package strict all-target Clippy pass with
zero diagnostics. All five crypto doctests and separate minimal-JSON and Rust
FFI library checks pass. The expanded crypto runtime selection passes 432 tests
with one existing ignore at that same source. It includes 197 selected library
cases plus complete integration/tool harnesses. The modified conformance and
release-audit owners both pass; the audit case takes 2,287.58 seconds on this
host. `crypto-context-runtime-3` records ordinary stacks and unchanged source.
The configured feature-resolved normal/build dependency checks also pass all
20 shipping boundaries.

Reports, original failures, compiler artifact identities and frozen source stages
are retained under `target/architecture-redesign/model-base-extraction-v1/`.
This evidence does not execute the complete crypto library or benchmarks, and
provides no runtime-performance or release-memory claim.

## Remaining integration

TODO: Complete the model and remaining context consumers before replacing the
live JSON API. The base model stage includes a fixed typed metadata type error
and an ordinary-stack deep-value regression. Its library now passes 90 tests
and strict all-target Clippy. That work also corrected native metadata resource
ownership and distinguished writer budget failures from allocation failures;
the [coherent follow-up suite](base-json-context.md) passes 1,962 tests across
20 executables with one existing ignore. Its strict five-package lint and base
FFI/transparent variants pass. Independently changed live BFV diagnostic code
and tests have a rebased reconciliation applied to the isolated model candidate.
Base feature checks and the combined crypto build pass there; focused execution
passes 430 tests with one existing ignore, including both diagnostic modes and
conformance. Two redundant Copy clones in the retained assertions were then
removed; strict crypto Clippy passes. The final formatted source passes its
build and all 430 selected tests again with one existing ignore and unchanged
source. Newer independent repository changes still require reconciliation into
the asset extraction candidate. No historical pass substitutes for that source or for
native/device, full-workspace and four-validator gates.
