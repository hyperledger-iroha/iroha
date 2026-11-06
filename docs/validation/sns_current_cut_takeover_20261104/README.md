# SNS current-cut component validation

The current-cut reader passes 36 focused tests: 12 matching `current_cut`
(including eight adapter regressions), 16 shared certificate tests, one native
SNS snapshot test, one exact State registry-schema binding test, and six Torii
SNS lease tests. There are no failures, ignored tests, or zero-match groups.

The adapter retires each fully checked original graph before reusing its local
source stage. The tests preserve inherited logical decoder charges, invalid
versus local-refusal outcomes, original payload admission order, and State
snapshot/certified-cut equality. The original public snapshot entry point was
retired; a test-only reference remains for byte/error parity.

Two maximum-body runs each observed 31,540,179 bytes of peak live allocator
usage and zero retained allocations. The Torii regression admits the same
299,098,096 cumulative logical bytes with unchanged 240,000,000-byte decoder
workspace limit. Its 307,108,864-byte peak is a conservative custody reservation
within the 384,000,000-byte request grant; it is not measured allocator usage.

`receipt.json` binds the current lock, exact harness digests/features, logs,
11 reviewed SNS sources, and 173 authority-registry Rust inputs. The focused
build completed in 61m52s including lock wait, using the existing warm target.
Other-owner Torii manifest/push/JWT inputs changed outside this capture. This
evidence does **not** establish full transitive Torii build custody, whole-node
qualification, network readiness, or release readiness. Binary files remain in
the warm target and are identified by digest rather than copied here.

To rebuild the focused harnesses in the same warm workspace:

```sh
cargo iroha-fast -- test --locked -p iroha_core -p iroha_torii \
  --features iroha_core/state-commitment-prototypes --lib --no-run
```

The exact schema test is recorded in `receipt.json`. The other retained logs
name every executed test. `SHA256SUMS` covers the retained evidence, excluding
itself. `verify.py` checks those artifacts, their reported test totals, and the
current scoped source/lock identity without rebuilding or claiming broader
custody.
