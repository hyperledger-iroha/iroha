# FASTPQ maximum-occupancy proof evidence

The original raw artifact bundles referenced below are absent from this checkout.
Independent replay requires those bundles or a fresh measured run.

The captured optimized ordinary producer completes at the maximum default four
update occurrences with four distinct account keys. A separate process verifies
the retained public artifact without constructing a witness or proof. The maximum
AXT-context proof and its separate no-prover artifact replay also pass.

| Completed route | Artifact bytes | Construction and self-verification | Whole test wall time | Peak process RSS | Separate replay wall / RSS |
| --- | --- | --- | --- | --- | --- |
| Maximum ordinary | 973,336 | 3,668.791228416 s | 3,677.65 s | 1,856,045,056 B | 7.70 s / 31,080,448 B |
| Maximum AXT context | 1,010,229 | 4,233.689089375 s | 4,261.25 s | 1,875,656,704 B | 26.87 s / 34,488,320 B |

The ordinary artifact SHA-256 is
`08d69a75975a4c6e7996f50bbe4207d99242a958096553ae2d4490f485366d38`.
The AXT artifact SHA-256 is
`451a65e2631b9f29eed7109f0d9390cd3ce1d1aa12ad544d75ee31ca2703d1ad`.
The maximum AXT artifact leaves 38,347 bytes below the unchanged 1 MiB outer cap.
The actual ordinary fixture reaches 127 retained SMT nodes, 128 siblings and
251 touched-node hashes, with its four canonical keys in different high-bit
quadrants. The normal API selection passes 16 tests; the 12 explicitly ignored
proof diagnostics are separately selected, not counted as passing normal tests.

Default charged construction payload (2 GiB), structural work (2^42), child
(512 KiB), and outer artifact (1 MiB) limits remain unchanged. Required Metal
hashing runs with CPU arithmetic. The independent replay also checks retained
artifact context, cap and mutation controls. Process RSS is separately measured;
this contended host run does not prove a universal RSS bound or fleet latency.

Both proofs and their independent replays run from immutable executable SHA-256
`2b4dbe02e268282c4dbd8c2fd08ceaf3392547ed98eb0c07eaccc60f2b9ed4e6`.
Ordinary Cargo recorded actual opt-level 3 for both the FASTPQ library and API
test executable. The nine-crate/root-input source capture contains 2,817 files,
manifest SHA-256
`987df33f4d322875ec6610e8cca2f7a9ea0415ad976bc079e38ea43b6909d6ab`;
its retained source archive SHA-256 is
`847b7ff74088b5b176692703eca5aaaa2f3fc64ab7bf7f57425931f7d15860d0`.
The complete frozen checkout guard covers 20,543 files, manifest SHA-256
`bec3119129d80d4587a665df0e93a16f52ab5191f3f8eb17bb65e7d6cc893b92`.
Both guards report zero drift during compilation and immutable capture.
The later dependency audit matches all 38 local Cargo artifacts (`fresh:false`)
to actual compiler invocations in the saved verbose log, all from the frozen
candidate, including `norito_derive`. Its receipt is under the build capture's
`dependency-provenance-audit/`. Registry source bytes were not separately
archived; locked offline Cargo resolution remains the recorded external scope.
Mutable post-build dep-info is retained for observation only.

The build capture is retained under
`dist/zk-remediation/2026-09-29/fastpq-maximum-occupancy-frozen-current/`.
Actual public artifacts, generation/replay logs, immutable executable and terminal
`complete-receipt.json` are retained under
`dist/zk-remediation/2026-09-29/fastpq-maximum-proof-run1/`.
That terminal receipt records both completed proofs and independent replays,
zero drift in the frozen candidate, and 104 changed live-main inputs after
capture. The latter explicitly prevents a current-main qualification claim.

This qualifies the stated immutable source/binary snapshot, including the
current source-occurrence fixture correspondence. Subsequent live checkout
changes do not invalidate these retained bytes, but this is not a qualification
of the changing main checkout, network authorization, a secure encryption
replacement, deployment hardware, or independent cryptographic review.
