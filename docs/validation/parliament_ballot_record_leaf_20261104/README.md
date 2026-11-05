# Parliament retained `decode_record` leaf

In each retained November 4 worker image, the exact function interval
`0x10000c2a4..0x10000c380` has 55 reachable instruction words, six RET sites and
seven paths in the ordinary CFG. Its longest path contains **34 instruction
events, including branches and RET**. Both outcomes of every conditional branch
are included; no data-dependent path is pruned. The backward branch reaches a
returning block and does not form a cycle. The longest CFG path is an upper-bound
witness, not a claim that its branch choices are jointly feasible.

The [separate helper](../../../scripts/parliament_ballot_retained_record_bound.py)
requires the exact next `LC_FUNCTION_STARTS` boundary, every aligned decoded word,
reviewed ASCII operand forms, an acyclic graph and only RET terminals. Missing
words, calls, traps, indirect transfers, unknown operands, interval escapes and
cycles refuse the sublemma. Original build, image, LLVM-tool, raw-capture and
targeted source pins are checked; all 29 original/reviewed input hashes remained
unchanged during capture. No disassembler, worker, Rust build or measurement ran.

This bounds ordinary instruction events for one valid ABI entry into this
test-codec leaf. It does not prove source semantics, memory safety, production
codec correctness, cycles, latency or complete physical resources. There is no
explicit SP decrement, but loads and stores through caller-supplied pointers can
access caller stack or other memory. Pointer-memory, exceptional-path, caller,
complete work and complete physical-stack bounds remain `null`.

All ten complete root objects from [stack-v5](../parliament_ballot_stack_v5_20261104/README.md)
are retained unchanged, including active Shape merge, native/unwind, trap,
indirect and recursive obligations. Every complete stack bound remains `null`.
Only nine targeted source files were checked against the original build; the
earlier rustup/Cargo.lock drift is retained explicitly. There is no latest-source,
profile, release or whole-workspace qualification.

The [focused tests](../../../scripts/tests/parliament_ballot_retained_record_bound_test.py)
pass **77 controls**, including independent path enumeration, malformed words,
operands and intervals, escaping/cyclic control flow, stale identities, late
input drift, preserved outer obligations and output-reuse refusal. Independent
read-only review separately enumerated all seven paths in each image, rehashed
all 29 inputs and compared all ten full root objects with their v5 originals.

The selected historical suite remains **47 passed and 113 failed**, without a
rerun or repin. The two adapter, two frontend and two typed-decoder failures
first encounter old LLVM-tool custody; the no-return failure first encounters
missing Rust 1.93.1 installed source HTML. This fresh leaf result does not repair
those historical certificates. Full adapter/coordinator/parser/typed-decoder
composition and physical frontiers require separate proofs for the retained
images; old frame sizes and call offsets cannot be transferred.

Evidence remains in the persistent cache below, not permanent release custody:

`/Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/parliament-current-20261104/`

| File below that cache | SHA-256 |
| --- | --- |
| `record-leaf-20261104T145500Z/report.json` | `95b73bececa3ce01585704f95e774c8e41f5c3672477741576810e50ec8d8da3` |
| `record-leaf-review-20261104T145418.894601Z/tests.json` | `68e716429e56898246835fb3b06dc00598d829153866f68c23558cfd889e59ec` |
| `record-leaf-review-20261104T145418.894601Z/tests.txt` | `ef842ba57c6d6737795c65d47f4165559047f31526df3b671e2e5dfff01cb1c1` |
| `record-leaf-independent-review-20261104T145449Z/review.json` | `6991dd1447d6aed856371ae24d749db09c3446bcd1bdc3ea4127bcd5f390a534` |
| `record-leaf-independent-review-20261104T145449Z/capture-review.json` | `21055c453544d4e3aabd04e0f533a99430a83bc50923647327570853b60bc160` |
| `record-leaf-root-review-20261104T145900Z.json` | `cbda9b8abb658bbaa915a0ed9ff6323c17dae74cb5e42c0ff27faf7ac56637e4` |

Reviewed helper SHA-256:
`8f9502d04178765e7e48c0d5dbe3046fe0a9b2c5b4c97f04e1ce5a6fb6961c81`.
Reviewed test SHA-256:
`2fc2c7317c87479acf71b5ca1e1525d26021364e2a7d471f42ba0acdd5e895fd`.
Exact helper/test copies are retained beside the report. No earlier evidence
was rewritten, and the new helper is not a production trust input.
