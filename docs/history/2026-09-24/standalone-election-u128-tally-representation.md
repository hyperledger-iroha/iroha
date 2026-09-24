# Standalone election V1 tally representation

This is a bounded F11 source change on `optimizations`, 2026-09-24. The
first-release private election finalization instruction, retained election
state, Torii tally projection, and IVM tally response now carry exact `u128`
option weights. A proof public-input limb is accepted only when its upper
16 bytes are zero. Finalization rejects an aggregate that overflows `u128`
before mutating election state; Torii and IVM reject malformed retained
aggregates before publishing a response. Tests cover a weight of `2^64`, a
nonzero high proof limb, aggregate overflow, Norito/JSON response roundtrips,
and unchanged duplicate-finalization rejection.

The source owners are
[`FinalizeElection`](../../../crates/iroha_data_model/src/isi/zk.rs),
[`ElectionState`](../../../crates/iroha_core/src/state.rs), the
[`FinalizeElection` executor](../../../crates/iroha_core/src/smartcontracts/isi/world.rs),
[`VoteGetTallyResponse`](../../../crates/ivm_abi/src/host_payload.rs), the
[`CoreHost` snapshot](../../../crates/iroha_core/src/smartcontracts/ivm/host.rs),
the [IVM mock host](../../../crates/ivm/src/mock_wsv.rs), and the
[Torii tally DTO](../../../crates/iroha_torii/src/routing.rs). The IVM syscall
number and sole V1 response frame name remain unchanged; the response payload
now contains `Vec<u128>` and no `Vec<u64>` decoder or adapter. The ABI hash
golden must be checked against the final source. Frame-identity fixture hashes
are based on the nominal frame name, so a field-width change does not itself
authorize changing those recorded hashes.

This is **representation only**. The standalone ballot and tally semantic
admission guard remains closed. The current state has no anonymous credential
latest-position map or closed accepted corpus, and the current circuits do not
prove confidential bond ownership, immutable hidden choice, smallest-unit
weight, complete aggregate or committee-free late-dropout completion. No proof
or release gate is qualified by this change.

World restore and derived-index rebuild now validate every current and
rollback-view election before publishing new read indexes. They reject an
invalid V1 selector, option count outside 2–64, a mismatched tally length,
end time before start time, or an aggregate exceeding `u128`; malformed JSON
restore attributes the failure to `elections`. The valid boundary
`[u128::MAX, 0]` is retained. Focused Core tests pass 2/2, including unchanged
derived indexes and rollback journal after a rejected view. This guards
stored representation; it does not supply the missing private ballot,
credential, bond or tally semantics.

F12 candidate-specific interface regeneration remains open. The changed
`FinalizeElection` Norito payload, `ElectionState` archive, Torii
`ZkVoteGetTallyResponseDto` JSON/Norito and IVM `VoteGetTallyResponse` require
schema/OpenAPI and canonical fixture checks together. The JavaScript
`buildFinalizeElectionInstruction` now accepts lossless numbers, bigint or
canonical decimal text through `u128::MAX`; its Norito object path emits exact
numeric JSON tokens for large weights. The authenticated JavaScript
`getElectionTally` route reads bounded lossless integer JSON and validates the
complete four-field V1 response, preventing `JSON.parse` rounding above
`Number.MAX_SAFE_INTEGER`. Four isolated builder and twelve isolated reader
tests pass, alongside the declaration fixture and targeted ESLint; the
installed native binding rejects the changed source provenance. Rebuilt
same-source native wire parity is still required.
Rust client/CLI consume the Torii response as Norito JSON. Python now has one
shared authenticated `get_election_tally` reader for both clients. Its 135
focused governance/election tests pass with current Python source and an
existing native wheel; final same-source native-package qualification remains
open. C# now has an authenticated `GetElectionTallyAsync` with bounded exact
`UInt128` parsing; its complete 5,788-test SDK suite passes. Kotlin owns one
authenticated `getElectionTally` and exact `BigInteger` response parser;
focused Kotlin and Java-source consumer tests pass 5/5 with JDK 21. Swift now
has an authenticated query and exact decimal `u128` parser; parser/model
typecheck and real-scanner smoke pass, but XCTest cannot start until the
same-source `dist/NoritoBridge.xcframework` is built. Parliament's old
parallel lexeme key/fallback was removed in the same first-release Swift cut,
with its stricter cap/non-cap policy retained and standalone smoke-tested.
Every SDK's generic Norito/schema and generated fixture parity still needs
same-candidate validation. Do not rewrite
source-pinned fixture digests or install a legacy `u64` path while interfaces
are still changing.

Validation status: the DataModel `zk_decode_from_slice_roundtrips` test passes
1/1, eight focused Core unit tests (six tally/IVM and two restore) pass against
the same corrected Core binary,
and the `ivm_abi` weight-above-`u64::MAX` roundtrip passes 1/1. The same
`ivm_abi` binary passes captured frame identity 1/1, ABI hash tests 8/8 and
the canonical selector 1/1. The IVM mock-host exact-maximum test passes 1/1;
Torii's same-source tally response tests pass 3/3, including the corrected
two-option minimum, one-option rejection, and lossless weight above
`u64::MAX` (`target/f11-torii-tally-response.log`). Full-host IVM, native
network integration and candidate-wide qualification remain open.
This record is not release evidence.
