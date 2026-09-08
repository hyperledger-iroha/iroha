# Mochi genesis and error ownership checkpoint

This records scoped development qualification after the canonical SDK stream
migration. It does not qualify the complete workspace or first release.

## Implementation

- Mochi decodes the server-owned `iroha_torii_shared::ErrorEnvelope`. The removed
  private two-field record had a different Norito frame identity and omitted
  canonical details; its old test encoded the same private duplicate. Four new
  regressions exercise actual shared envelopes, details, empty codes, truncated
  frames and existing JSON/text fallbacks.
- `supervisor/genesis_material.rs` owns genesis artifacts, temporary signing
  material, Kagami generation/signing/verification, exact record reads and
  generation validation. Its private seven-field request describes one Kagami
  command and replaces eight positional arguments.
- Independent review accounts for all 246 original functions: 244 bodies are
  byte-identical; two differ only in typed request construction/destructuring.
  All 141 existing genesis/supervisor tests remain byte-identical. Key permissions,
  create-new semantics, zeroization, cleanup, bounded/no-follow reads and complete
  block/manifest validation retain their original behavior and ordering.
- Four GUI test lint corrections preserve 96 tests and all 348 assertion sites.
  No production GUI behavior changes in this checkpoint.

## Qualification

Strict Clippy passes all targets of `mochi-core`, `mochi-integration` and
`mochi-ui`, with GUI/dev-tools features, dependencies excluded and zero warnings.
The combined test build and strict lint run capture the same 180-input source
fingerprint, `f495f99eb4469a37c059e96ac09e2a5687d8cf66495ea0b1a70144401304d7d3`.
The exact scoped sources and eight executable artifacts are retained locally.
All runtime runs use the default stack and preserve their source inputs:

| Suite | Passed | Ignored |
| --- | ---: | ---: |
| core-runtime | 449 | 1 |
| gui-runtime | 181 | 0 |
| readiness-runtime | 12 | 1 |
| mock-runtime | 9 | 0 |
| integration-runtime | 3 | 1 |
| streams-runtime | 2 | 0 |
| real-kagami-runtime | 1 | 0 |

The real-Kagami case is explicitly selected and verifies its environment-selected
executable before and after execution. That retained source-built Kagami has an
earlier 68-file direct Kagami/genesis capture; this is not a same-revision full
transitive release build. Readiness includes the ordinary supervisor scenarios;
its ignored real-Kagami case is executed separately. Formatting, codec and
historical-archive guards pass on their recorded inputs.

## Remaining limits

The first lint attempt found relocation imports; the next caught two call-site
omissions during import cleanup. Both calls were restored exactly, including
argument/field order, and the complete retained parent bodies match the original.
A third lint attempt exposed four existing GUI test style issues. All failed
logs remain alongside the final passing checks.

The supervisor is reduced from 6,335 to 5,428 lines; its new genesis owner is
969 lines. The parent still exceeds the 5,000-line production limit. Snapshot
restore transaction/journal ownership remains a separate decomposition. No
source-budget exceptions, dependency limits, model optimization, memory ceilings,
codec layouts, ABI versions or stack settings change here. The original three
Native AMX stack-root owners retain their qualified hashes.

Full SDK/workspace qualification of concurrent merged work, mandatory real
four-validator scenarios, native/device delivery and pinned baseline/candidate
memory profiles remain open. The complete redesign goal remains active.

Beforeimages, source fingerprints, replayable patch, independent reviews, compiler
identities and runtime logs are retained under
`target/architecture-redesign/mochi-genesis-owner/`; the canonical error DTO's
original beforeimages are under `target/architecture-redesign/mochi-error-envelope/`.
