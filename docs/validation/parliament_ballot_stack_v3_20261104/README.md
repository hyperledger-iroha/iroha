# Parliament retained-image stack-v3 diagnostic

Both retained November 4 worker images were recaptured successfully with the
reviewed analyzer and zero-inclusive disassembly. All ten selected complete
stack bounds remain `null`. This is artifact-bound static evidence, not a
current working-tree build, physical stack certificate, or release qualification.

The [receipt](receipt.json) guards the exact binaries, LLVM tool, analyzer,
tests, capture driver and reference before and after capture. All 18 original
`stack-v2` files were unchanged. The original build/capture evidence remains in
[`parliament_ballot_current_candidate_20261104`](../parliament_ballot_current_candidate_20261104/).
No historical identities or certificates were repinned. The prior review found
different live `rustup` and `Cargo.lock` bytes; this capture does not assert that
the retained workers represent the latest working tree.

## Reviewed change

The analyzer accepts bounded operand forms for LDURSB, scalar ORN, Advanced SIMD
UMAXV, MVNI and RAX1. LDURSB uses a signed nine-bit unscaled offset without
writeback; its integer destination and ORN's destination invalidate a written
FP. SIMD destinations do not write integer SP/FP. Arrangement, width, immediate
and shift restrictions are checked. Unreviewed forms remain errors.

The primary reference is [Arm DDI 0602, 2025-03](https://documentation-service.arm.com/static/67e40f3398aa3c3b6eea6a85),
printed pages 611–612 (LDURSB), 679–680 (ORN), 1663–1664 (MVNI), 1680 (RAX1),
and 2050–2051 (UMAXV). Its download identity is retained in
[arm-reference-download.json](arm-reference-download.json); the exact PDF and
extracted text paths/hashes are in the receipt. Only ordinary instruction stack
effects are modeled; SIMD feature/trap behavior supplies no runtime cost bound.

`capture()` now passes `--disassemble-zeroes`. Zero blocks cannot establish
unreachable padding. At `fatal::exhausted`, the native `__exit` edge at
`0x1000050d8` remains unresolved and its possible continuation at `0x1000050dc`
decodes as `udf #0x0`, an explicit exceptional frontier. The resolved prefix is
144 bytes (16-byte frame plus the modeled 128-byte red zone); this is not a
complete bound on the exiting process.

## Results and remaining boundaries

The following values bound only resolved regions under the analyzer's ABI
assumptions. They stop at opaque function or edge boundaries.

| Selected root | Portable region, bytes | Accelerated region, bytes | Complete bound |
| --- | ---: | ---: | --- |
| `Reservation::verify` | 15,808 | 12,864 | Unknown |
| `workspace::worker` | 16,000 | 13,056 | Unknown |
| `fatal::exhausted` | 144 | 144 | Unknown |
| Registration proof verifier | 9,744 | 6,800 | Unknown |
| Vote proof verifier | 9,744 | 6,800 | Unknown |

The portable image contains 91,775 decoded instructions and 870 functions; the
accelerated image contains 89,887 instructions and 860 functions. Whole-image
frame-error counts are 7 and 6, respectively, versus 87 in each v2 capture.
This reduction does not close a physical stack obligation.

| Worker frontier | Portable | Accelerated |
| --- | ---: | ---: |
| Native edge | 131 | 126 |
| Exceptional terminal | 65 | 67 |
| Function fallthrough | 49 | 43 |
| Indirect call | 32 | 32 |
| Indirect branch | 5 | 5 |
| Inconsistent SP merge | 1 | 1 |
| Unsupported instruction | 2 | 1 |
| Positive-weight recursive component | 1 | 1 |

The exact prior native, trap, indirect and inconsistent-merge frontiers and
recursive nodes were checked for preservation. Zero-inclusive capture resolves
the earlier missing-instruction gaps into explicit exceptional terminals.
The unproved `Shape::new_as` SP merge remains. Continuing past the reviewed
opcodes exposes `bcax.16b` in both worker graphs and `bit.16b` in the portable
graph; both remain unsupported. Four `umov.h` sites per image remain unsupported
outside the selected direct worker graph. Native endpoint costs, indirect
targets, recursive panic paths, fallthroughs, hidden unwind/signal/exception
edges and memory aliases still require separate arguments.

The historical 160-node rerun remains 47 passed and 113 failed. This diagnostic
does not repair or requalify any historical certificate.

## Validation and artifacts

Focused regression command:

```sh
/Users/takemiyamakoto/Library/Caches/iroha-script-tests/venv/bin/python \
  -m pytest -q scripts/tests/parliament_ballot_stack_audit_test.py
```

Result: **141 passed**. Coverage includes valid scalar/vector forms, invalid
operands and suffixes, FP clobbers, forbidden SP/base writeback, red-zone checks,
unknown instructions, and retention of native/trap frontiers when zero blocks
are decoded. `git diff --check` passed for the analyzer and tests.

The retained [capture driver](capture_retained_stack_v3.py.template) requires a
new output directory and exact reviewed helper identities. Its run used:

```sh
/Users/takemiyamakoto/Library/Caches/iroha-script-tests/venv/bin/python \
  /Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/parliament-current-20261104/capture_retained_stack_v3.py \
  --root /Users/takemiyamakoto/dev/iroha \
  --cache /Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/parliament-current-20261104 \
  --output /Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/parliament-current-20261104/stack-v3
```

No Rust builds, worker executions, timing campaigns or full-suite reruns were
performed. [summary.json](summary.json) maps every archived capture to its
original absolute path and both byte hashes. Large disassembly and diagnostic
JSON files use lossless gzip with deterministic timestamps. `SHA256SUMS` covers
every file in this directory except itself. Raw captures and binary/tool bytes
remain in the persistent cache; that cache is not permanent release custody.
