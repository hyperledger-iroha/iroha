# Parliament retained-image stack-v4 diagnostic

Both retained November 4 worker images were recaptured with bounded BCAX, BIT
and UMOV support. The analyzed function-entry CFGs contain no remaining
unsupported-instruction frontiers. Each image still has an unproved
`Shape::new_as` stack merge, and **all ten selected complete stack bounds remain
`null`**. This is artifact-bound static evidence, not a latest working-tree
build, physical stack certificate, or release qualification.

The [receipt](receipt.json) guards the exact binaries, LLVM tool, analyzer,
tests, capture driver and Arm reference before and after capture. All 71 files
in the original v2/v3 cache and repository capture directories were unchanged,
including v3's independent review. All eight raw LLVM outputs are byte-identical
to v3; only analyzer coverage changed. Earlier captures remain at
[`parliament_ballot_current_candidate_20261104/stack-v2`](../parliament_ballot_current_candidate_20261104/stack-v2/)
and [`parliament_ballot_stack_v3_20261104`](../parliament_ballot_stack_v3_20261104/).

## Reviewed change

BCAX accepts four SIMD registers with the `.16b` arrangement. BIT accepts three
SIMD registers with `.8b` or `.16b`. UMOV accepts W destinations for byte,
halfword and word lanes, and X destinations for doubleword lanes, with the
corresponding finite lane-index bounds. UMOV writes to W29/X29 invalidate the
known integer FP; SIMD register V29 does not. Invalid register classes,
arrangements, lane indices, operand counts, SP operands and writeback forms
remain errors. These checks model ordinary stack effects, not exception or
execution-time bounds.

Primary reference: [Arm DDI 0602, 2025-03](https://documentation-service.arm.com/static/67e40f3398aa3c3b6eea6a85),
printed pages 1110 (BCAX), 1130–1131 (BIT), and 2069–2070 (UMOV). The retained
download identity is in [arm-reference-download.json](arm-reference-download.json).
An independent agent reviewed the operand restrictions, FP handling and tests
without edits or execution and reported [no findings](independent-review.json).

## Results and remaining boundaries

These figures bound only resolved regions under the analyzer's ABI assumptions;
they stop at opaque function or edge boundaries and are unchanged from v3.

| Selected root | Portable region, bytes | Accelerated region, bytes | Complete bound |
| --- | ---: | ---: | --- |
| `Reservation::verify` | 15,808 | 12,864 | Unknown |
| `workspace::worker` | 16,000 | 13,056 | Unknown |
| `fatal::exhausted` | 144 | 144 | Unknown |
| Registration proof verifier | 9,744 | 6,800 | Unknown |
| Vote proof verifier | 9,744 | 6,800 | Unknown |

The portable image has 91,775 decoded instructions and 870 functions; the
accelerated image has 89,887 instructions and 860 functions. Whole-image
frame-error counts fall from v3's 7/6 to 1/1: the retained `Shape::new_as`
inconsistent-SP merge in each image. This does not prove every possible
execution path or any all-input resource bound.

| Worker frontier | Portable | Accelerated |
| --- | ---: | ---: |
| Native edge | 132 | 126 |
| Exceptional terminal | 65 | 67 |
| Function fallthrough | 49 | 43 |
| Indirect call | 32 | 32 |
| Indirect branch | 5 | 5 |
| Inconsistent SP merge | 1 | 1 |
| Unsupported instruction | 0 | 0 |
| Positive-weight recursive component | 1 | 1 |

The capture checks exact preservation of the native, trap, indirect and SP-merge
frontiers and recursive nodes from both prior captures. Continuing through BIT
exposes an additional portable `_memcpy` edge in
`aes::backends::soft::fixslice::aes192::key_schedule::<u64>` at `0x10001d134`,
with 1,264 bytes of caller depth. No target cost is assigned to it.

The fatal prefix remains a 16-byte frame plus the modeled 128-byte red zone;
native `__exit` and its possible `udf #0x0` continuation remain unresolved.
Native endpoint costs, indirect targets, recursive panic paths, fallthroughs,
hidden unwind/signal/exception edges and memory aliases still require separate
arguments. The historical 160-node suite remains 47 passed and 113 failed;
this diagnostic does not repair or requalify historical certificates.

## Validation and artifacts

```sh
/Users/takemiyamakoto/Library/Caches/iroha-script-tests/venv/bin/python \
  -m pytest -q scripts/tests/parliament_ballot_stack_audit_test.py
```

Result: **203 passed**. New regressions cover valid register and lane boundaries,
integer FP clobbers, SIMD V29 independence, forbidden SP/base/writeback forms,
and malformed arrangements, suffixes, widths, counts and indices. Existing
native/trap, unknown-instruction, red-zone and recursive-boundary controls pass.
`git diff --check` passed for the analyzer and tests.

The [capture driver](capture_retained_stack_v4.py.template) requires a new output
directory and exact reviewed helper identities. It ran as:

```sh
/Users/takemiyamakoto/Library/Caches/iroha-script-tests/venv/bin/python \
  /Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/parliament-current-20261104/capture_retained_stack_v4.py \
  --root /Users/takemiyamakoto/dev/iroha \
  --cache /Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/parliament-current-20261104 \
  --output /Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/parliament-current-20261104/stack-v4
```

No Rust builds, worker executions, timing campaigns or full-suite reruns were
performed. [summary.json](summary.json) maps each archived capture to its
original path and both byte hashes. Large disassembly and diagnostic JSON files
use lossless gzip with deterministic timestamps. `SHA256SUMS` covers every file
here except itself. Binary/tool bytes remain in the persistent cache, which is
not permanent release custody. The prior rustup/Cargo.lock drift is not resolved
by retained-image analysis; latest-source qualification remains separate.
