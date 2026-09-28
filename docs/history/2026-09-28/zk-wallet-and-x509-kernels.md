# Wallet and X509 kernel validation, September 28

This is scoped implementation evidence from the shared checkout. It does not
qualify a complete credential proof, a packaged native SDK, or release activation.

## JavaScript wallet boundary

From `javascript/iroha_js`:

```sh
node --test test/confidentialProver.test.js test/confidentialProofCardinality.test.js
```

All 13 checks pass. These exercise the asynchronous public API, exact amount and
input cardinality checks, automatic relation selection, deferred native success,
worker rejection, disposal during queued work, immediate FFI-key cleanup, output
root/cardinality checks, disposal during input normalization, non-Error input
failures, and the TypeScript promise contract. Native bindings are
mocked in these boundary tests. Native Rust worker and packaged-addon execution
are separate checks; their results must not be inferred from these passes.

## Native wallet worker

```sh
cargo test --locked --offline -p iroha_js_host --lib -- \
  confidential_wallet confidential_proof_boundary_tests --nocapture
```

All nine checks pass (31.01 seconds after a 4m24s build). These include strict
network/key/note parsing, failed-input cleanup, consuming a worker job exactly
once, note derivation parity and redacted Debug output. The positive worker test
produces a real full-redemption proof and checks its self-verification, public
relation and cardinalities. This directly executes the native Rust task on a
worker thread; it does not exercise a packaged N-API addon or its JavaScript
event-loop integration. The local log is
`/tmp/iroha_js_wallet_20260928_final.log`.

A separate transaction-finalizer regression initially failed an obsolete error
message assertion: the canonical ordinary decoder now rejects the genesis domain
before the finalizer's later check. Its assertion now checks that exact decoder
failure, retaining the independent retired-admission and signature controls. The
fresh retry passes (1 check, 0.05 seconds after a 3m56s build), recorded in
`/tmp/iroha_js_finalizer_20260928_retry2.log`.

## Source and geometry contracts

```sh
python3 -m pytest \
  scripts/tests/halo2_backend_02_compaction_source_test.py \
  scripts/tests/halo2_backend_shared_circuit_source_test.py \
  scripts/tests/check_note_stark_profile_constraint_dedup_test.py \
  scripts/tests/check_zk_x509_proof_geometry_test.py \
  scripts/tests/zk_source_tokens_test.py -q
```

All 21 checks pass. Source contracts preserve substantive circuit assertions;
the geometry checks derive the codec ceiling without producing a valid proof.

## Isolated native quotient and ownership kernels

Six optimized Rust tests pass using the repository's actual Goldilocks/Fp4 and
FFT helpers, private-buffer owner implementations, and the included quotient
stripe and private-owner test modules. The temporary harness supplies only the
surrounding error namespace. Tests compare folded stripe evaluation to independent
Horner evaluation and a complete coset, check actual log-19/log-22 masked tails and
native-next translation, reject invalid geometry/field words, and verify clearing
and consuming allocation handoff. Runtime: 0.07 seconds.

| Included source | SHA-256 at execution |
| --- | --- |
| `stark/main_quotient_stripes.rs` | `4c543e61936fc355aec74f63d7410b8f476225c6a41ed2348cfcd1af11be6325` |
| `stark/main_secret_ownership_tests.rs` | `c4e0faf5b2094a7f3fc2d88b306baff70920c553a9ef6379c56b444b8d3121d9` |
| `transparent_stark.rs` helper source | `ceba00b1f6f26330663db4e5b46934eed3ff287d36eab4498a76c2c49d562da2` |
| `stark.rs` owner source | `354846b6336e9e495bc0bc7339bcc9f6c60caa7d120dae7c7e2bb71e2639e525` |

The local harness and receipt are under `/tmp/iroha-zk-stripe-kernel-20260928`;
they are not repository build prerequisites. This isolated result does not prove
that Core links or that the complete source/replay/resource integration works.

The first fresh Core test build encountered unrelated concurrent reputation/archive
API migration errors before running tests. No new Core profile digest or
transcript known-answer pin was captured from that failed build. Full Core,
maximum-shape proofs, whole-prover memory/time, and independent cryptographic
qualification remain outstanding.

A later Core retry compiled and started its 479 selected tests. A separate exact
native profile test computed the manifest with both production and independent
29-field encoders and confirmed their equality before exposing the stale literal
pin. The replacement is
`a9530f225b112f50e1ed113b285eb1ec32b7a4e8ccdea7676b97afa6afff934e`;
the source pin is updated and its fresh assertion remains pending. This binds the
reviewed joined-root/reduced-opening construction and unchanged unavailable
activation policy; it does not qualify release activation. The capture is in
`/tmp/iroha_core_x509_profile_pin_20260928.log`.

Four compact-CA controls pass in that native binary: canonical roundtrip and
binding, credential pre-auxiliary context mutations, public/root/DEEP/FRI/query/
frontier mutations, and fixed-parameter/resource gates. The native DER column
transpose also passes. These are actual proofs and adversarial verification,
not only source-shape checks.

Separate real I/O and projection proof KAT runs complete verification, canonical
re-encoding and query-uniqueness checks before failing their obsolete digest
literals (252.66 seconds combined). Replacement literals are captured below;
fresh pinned assertions remain pending. Log:
`/tmp/iroha_core_x509_protocol_kats_20260928.log`.

| Known-answer proof | Previous digest | Captured replacement |
| --- | --- | --- |
| I/O | `7e283b62798b5a626a71fad660a19a455e6fb389dd268e320a0841e83af5f94d` | `d7d747959c5632f147be02cbaae61e8f1f5dd04691477058ce1b758c13e506aa` |
| Projection | `94f29e8e0b3f9cc444794905125f5b36c03fe4b9e36e095c74aec10a10344261` | `33a900ac3f49acbf3cb92e292525fcd2c7f3d069f6f71cf365a5cf9c0f13f78d` |

The separate native wallet build found one normal-library error: proved-IVM
execution called `TxOverlay::from_queued_execution`, which was incorrectly
restricted to test builds. That restriction is removed; the successful retry is
reported above. A test-only `expect_err` was also replaced with `err().expect`
to avoid requiring Debug on a signed-transaction result, preserving its assertion.
The next retry encountered two concurrent SCCP build errors using `as_slice()`
on `ConstVec`; only those accesses were changed to its existing `AsRef<[T]>`
implementation. The passing finalizer build includes that minimal compile repair.
The historical archive verifier also passes with `--check-current`, including
the 300-line root-document limit.
