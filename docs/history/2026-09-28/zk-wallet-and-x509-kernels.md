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

The fresh Core test build encountered unrelated concurrent reputation/archive
API migration errors before running tests. No new Core profile digest or
transcript known-answer pin was captured from that failed build. Full Core,
maximum-shape proofs, whole-prover memory/time, and independent cryptographic
qualification remain outstanding.

The separate native wallet build found one normal-library error: proved-IVM
execution called `TxOverlay::from_queued_execution`, which was incorrectly
restricted to test builds. That restriction is removed, and native validation
is retried. No successful native worker result is claimed from the failed build.
The historical archive verifier also passes with `--check-current`, including
the 300-line root-document limit.
