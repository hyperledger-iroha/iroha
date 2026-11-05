# Parliament retained frontend composition

The [separate helper](../../../scripts/parliament_ballot_retained_frontend_bound.py)
bounds instruction events in one ordinary `Reservation::verify` activation and
its direct [record-leaf](../parliament_ballot_record_leaf_20261104/README.md)
activation. Every other callee remains opaque. Both retained November 4 worker
images give the following result at `0x100000b40..0x100000e84`:

| Quantity | Both images |
| --- | ---: |
| Complete decoded interval | 209 words |
| Ordinary entry-reachable graph | 192 words, 13 paths |
| Wrapper instruction events | At most 105 |
| Wrapper plus record instruction events | At most 139 |
| Explicit local SP decrement | 2,512 bytes |
| Inherited v5 local frame plus modeled red zone | 2,640 bytes |

Both conditional outcomes are retained. The count includes wrapper BL and leaf
RET instructions; only the exact, rederived record leaf contributes callee
events. Opaque callee work is unknown, not assigned zero. These bounds exclude
exceptional and reentrant activations and do not establish elapsed time, memory
safety or complete physical stack/work limits. Caller-pointer memory remains
unknown; the red-zone figure is an inherited model, not a physical certificate.

Direct-call coefficients are at most eight calls per ordinary wrapper path:
profile and record each at most one, combined typed decoders at most one,
combined role verifiers at most one, memcpy at most two, direct deallocation at
most one and registration-context drop at most one. The memcpy request arguments
are 697, 424 and 368 bytes, with a path maximum of 1,121 requested bytes. Neither
these requests nor call counts bound native work or memory footprint. Parser,
typed-decoder, verifier, allocator and cleanup internals are not composed.

All **17 words outside ordinary entry reachability** remain explicit active
obligations, including five calls, three to `__Unwind_Resume`, and the possible
end fallthrough `0x100000e80→0x100000e84`. Their entry conditions and caller depths
are `null`; no dead-code or native-nonreturn assumption removes them. All ten
full [v5 root objects](../parliament_ballot_stack_v5_20261104/README.md) are
unchanged, including Shape, native, indirect, trap, unwind and recursive
frontiers. Every complete stack bound remains `null`.

The [67 focused controls](../../../scripts/tests/parliament_ballot_retained_frontend_bound_test.py)
pass. They cover independent path enumeration, SP merges/returns, operand and
identity refusals, record-only weighting, request definitions, suffix entry,
full obligation preservation, late drift and output reuse. The test receipt
records source hashes checked **after** execution; it does not claim a
pre-execution hash observation. Independent read-only review separately parsed
the original words, enumerated all paths, checked every SP state/coefficient,
and verified all 33 capture input pins before and after its review.

Only the original nine targeted source files are checked. Prior rustup/lock
drift remains recorded; there is no latest-source, profile, release or
whole-workspace qualification. The historical result remains **47 passed and
113 failed**, without rerun or repin. This work changed no earlier evidence,
State input or production trust path, and ran no Rust build, disassembler or
worker.

External evidence below remains persistent cache custody, not permanent release
custody:

`/Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/parliament-current-20261104/`

| File below that cache | SHA-256 |
| --- | --- |
| `record-frontend-20261104153709Z/report.json` | `77109d327011804954392e6d7983b9181f98001663fdf0085bd43f3c9084f60d` |
| `record-frontend-review-20261104T152753.994407Z/tests.json` | `af19909cff20d1ce87782d2f1a04b40e65a20aecc411114cd3f9c215824c2e12` |
| `record-frontend-review-20261104T152753.994407Z/tests.txt` | `4648582f009be3e75121b6cecc00260e3c900607a6bb98658950d760b5bb18c1` |
| `record-frontend-independent-review-20261104T153925Z/review.json` | `0a76705955d4231347a9581a149285c7313de6564a829002f49a52a4ad17e2f8` |
| `record-frontend-root-review.json` | `77f71623cd95cf02bfb831963e74ab3ed713c4ffb564e80eabc060b5a8fd74a2` |

Reviewed helper SHA-256:
`5c9af52de090473d12b5658139720aa333fb0aa451060ed4a9385c3b662a6082`.
Reviewed test SHA-256:
`f6d7f036d10e3f82c9d6f1b97c8ecc31108a4838a6ed265bcfdc7de066e18ee1`.
Exact helper/test copies are retained beside the report.
