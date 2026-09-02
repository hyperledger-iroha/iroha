# Autonomous lifecycle terminal-outcome wire fixtures

These fixtures freeze bytes emitted by the historical Kura producer, rather
than bytes synthesized by the current compatibility encoder. They cover the
two layouts that advertised the V1 terminal-outcome schema before the current
V2 layout separated its schema and hash domain.

## Deterministic capture input

Both producer revisions ran the existing unit test
`retired_release_pending_and_complete_progress_at_the_original_exact_limit`
with a fresh Kura `TempDir`. The test-only capture hook replaced its random
BLS-normal signer with `KeyPair::try_from_seed(vec![0xC1; 32],
Algorithm::BlsNormal)` and wrote the exact Kura-read Pending and Complete
frames plus each decoded raw `outcome_hash`. The height-context input was
`Hash::new(b"release-terminal-capacity-context")`; the canonical payload salt
was `0x51`.

The capture hook did not modify the terminal layout, Kura persistence, Norito,
or cryptographic production sources. Its `git diff -- <test-file>` SHA-256 was:

- legacy V1: `75c814923e67959325ccd1339797885dc9f180b51fbc892072f623e9226712bd`
- basis-bearing V1: `e8ed232c67bec3df58c87e90307ec3e5c0020d4b3b120bfe2f1cb62c22ff628a`

The legacy checkout also needed one test-only type annotation in
`kagemusha_operation.rs` to compile with the current toolchain. Its isolated
diff SHA-256 was
`65bfd4c6ece613dcd66a97fa104cb6562a0e97e15444ebb0a5785a69fa6cceec`.
It did not affect the fixture test, codec, terminal-outcome code, signer, or
wire bytes.

## Producer revisions

The legacy-layout producer was commit
`2725144ae0632637078edd700a77f03f4eeb9397`, tree
`81f0bc646c0f6651967b678acd3295100ce9341b`, parent
`40f44e84cde4ffca106a47db9db8438cd063989f`, authored
`2026-08-31T22:28:23+10:00` with subject `update torii`.

That revision has byte-identical Kura root, terminal-layout, autonomous
capacity, selected-test, test-helper, and Norito blobs to commit
`77b369ce5403fcbc612d377c068d7a386420e248`. The latter could not execute the
test because of twenty unrelated checkout-wide compile errors. The crypto
delta between them adds only zeroization APIs and tests and does not change BLS
key generation or signing.

The short-lived basis-bearing V1 producer was commit
`b31a27c10f3dd9df9bd375095f1595ece62cdd79`, tree
`2eba15bb550e32bac656a48e56956fadd272747e`, parent
`77b369ce5403fcbc612d377c068d7a386420e248`, authored
`2026-09-01T17:46:51+10:00` with subject `update iroha core`. It was the commit
that added the V1 `basis` field before the V2 compatibility repair.

## Reproduction result

Each revision was built with its own target directory and executed twice using
`cargo test --locked -p iroha_core --lib
retired_release_pending_and_complete_progress_at_the_original_exact_limit --
--nocapture`. The legacy runs each reported one pass, zero failures, and 13,068
filtered tests. The basis-bearing runs each reported one pass, zero failures,
and 13,126 filtered tests. All four same-revision Pending, Complete, and raw
hash pairs compared byte-identically. Each raw 32-byte hash was also found
inside its corresponding frame.

| Fixture | Bytes | SHA-256 | Embedded outcome hash |
| --- | ---: | --- | --- |
| `legacy_v1.pending.norito` | 1016 | `438d79815fbfbbae800953ff46bafc34bd4345e3ebdf01f5ddd405b5d6a1b11b` | `717564335d9ed503228fc0d79d3ba02a22481a67c0ae92da7d2b882d5d27708b` |
| `legacy_v1.complete.norito` | 1016 | `67038e93f4dc0cfb2514ae785fb18fb299d0eba7977c7076573a2a22a5246ef7` | `b2f95b9477c5ba85f950ef4ad5c5134b582a21c3180729c91716669cb2ff271d` |
| `basis_v1.pending.norito` | 1021 | `79244e2a1abf156cb0ab283f85abf85b8dd5d9e36e3d859ca286e5f1586cb2e7` | `c385e13df8f562f0525d3febda1b54f59b26099d3215a966587c2fa85c738aa9` |
| `basis_v1.complete.norito` | 1021 | `168cb40c187543ed7ac74f2299171006b668c2ba725a24f6a135422ad2778754` | `8c52af56bea7718431af2269cd6ea7d17557999be3f809e2bbb5ba200ea2b73f` |

The companion `.outcome_hash` files are 32 raw bytes. Their SHA-256 digests
were, in the table order above,
`edf793a9c67af9765355e8957d5ac4650a97043205953fc6c1a8c01c7d7f62c3`,
`21de34eaa24499e8bff336251f9cd15f1c5e2abf47d0d75927c649fdced68fd2`,
`de10422e53e7b2a67c230abe252290670692434b586205c2166acdcf143a2d37`,
and `16f42166244ecff17357d0f8d1788b01069b424fe5c0e5125721751ead233d18`.

The checked-in artifacts are ASCII hex encodings of those immutable binary
captures. Tests decode the hex before exercising the production decoder,
restart inventory, compare-and-swap completion, and exact-layout preservation.
