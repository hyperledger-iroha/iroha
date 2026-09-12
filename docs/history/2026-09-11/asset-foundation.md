# Asset identity ownership and reconciled callers

This checkpoint is an isolated candidate, not a live API cutover or release
qualification. Source, before-images and compiler reports are retained under
`target/architecture-redesign/model-base-extraction-v1/`.

`iroha_model_base::asset` owns `AssetDefinitionId` and `AssetBalanceScope`.
Constructors, explicit schema identities, canonical UUID/Base58 validation,
binary fields, JSON and storage keys move with the types. Aggregate `AssetId`,
asset registration, ledger ownership and `IdBox` composition remain in the
ledger model. Superseded aggregate and IVM exports are removed.

Confidential discard invokes the existing erasure primitive on the owner's
actual private UUID field. Aggregate and CLI cleanup sites use that operation;
the test inspects the erased owned bytes through the value getter. The field
remains private under the transparent feature.

The caller stages cover aggregate, CLI and remaining Rust consumers. A complete
live-source reconciliation preserves 621 independently changed or added paths.
Six conflicting files were resolved explicitly, retaining newer validation and
test decomposition and the previously qualified BFV diagnostic cleanup. Four
surviving or moved files then required additional canonical asset paths.
The final static scope review examines 1,999 actual module/include sources and
resolves 3,220 uses. Eleven existing guarded or enum-variant groups were reviewed;
no unresolved binding or further binding edit remains. Compilation of every
consumer remains required.

The generated privacy fixture input list now includes the actual asset owner
and its manifest inputs. Kotlin parity CI includes foundational-model changes.
The selected Musubi fixture types do not use the moved asset types, so their
input list is unchanged.

## Scoped results

- The foundational model build passes without diagnostics at source
  `5c0226d7a7d0207bfdd484714b5940f593503ed8084b78038b9f79361198c9b7`.
  All 107 library and three allocation tests pass on ordinary worker stacks,
  including the immutable pre-extraction asset envelope/storage-key capture.
- Strict all-target base Clippy passes after the generated-input and dependency
  policy edits, at source
  `035010a359947f461cdc1bcbf2cb0218cf298e4d0e9daa5891c2a1cb9adf012c`.
- All 20 feature-resolved normal/build dependency boundaries pass. The lockfile
  changes only the approved dependency edges; package versions are unchanged.
  Source-graph limits account for those reviewed edges, with layer prohibitions
  and local-package limits preserved.
- All 797 dependency-budget, generated-artifact and release-automation Python
  tests pass. Reproducible temporary fixture repositories were removed after
  completion; source stages and result records remain.

The subsequent coherent feature qualification uses source
`af90b776b7c1d261a7493d61719554c885eac0fbd3622b9212e6f2bf890867e3`:

- Norito plus the base model with both `ffi_export` and `transparent_api` pass
  1,508 tests across 13 executables, with one existing ignored test.
- The FFI-only and transparent-only base selections each pass all 110 tests.
- Strict Norito/base all-target Clippy passes with both features enabled.
- All five base compile-fail documentation tests pass, including direct
  construction and mutation rejection for the private asset UUID.

The shared codec now preserves allocation failure as a distinct typed JSON
error instead of classifying it as a decode-budget rejection. The first combined
run also exposed interference in a process-global allocation test counter.
That test owner now measures each thread separately and resets its measurement
on unwind; the original allocation ceiling is unchanged. The rerun above uses
ordinary stacks and parallel workers. These results do not qualify aggregate
consumers or native ABI execution.

## Remaining qualification

TODO: Finish aggregate and external contextual JSON consumers, then compile and
exercise their wire fixtures. The [version owner qualification](version-json-context.md)
and [SoraFS owner qualification](sorafs-json-context.md) pass their scoped runtime
suites; SoraFS strict Clippy retains proof/signer findings. Source-size violations
elsewhere, model compilation memory, full workspace, native/device and
four-validator gates remain open.
