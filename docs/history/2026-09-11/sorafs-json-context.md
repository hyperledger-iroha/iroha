# SoraFS contextual JSON and ordinary cleanup

The isolated SoraFS manifest candidate uses the sole checked JSON writer and
explicit Norito contexts. Gateway authorization keeps typed codec errors and
transfers owned strings, arrays and objects without cloning recursive values.
Wrong-type rejection safely drops a manually constructed 32,768-level tree on
ordinary worker stacks. Signature validation and rejection order are preserved.

The proof-stream HTTP writer emits fields directly in their existing lexical
order, with the same omitted optional fields, lowercase hex and standard Base64.
Transparency byte fields retain their numeric-array shape, including optional
null values. Pricing's duplicate JSON wrappers are removed; its canonical codec
caller performs the same semantic validation explicitly.

The compiled candidate preserves fixture expression types and captured bytes.
The independent observation fixture now includes the actual BeforeAdmission
current-custody phase in its full seven-phase matrix. No protocol tag, signing
preimage, schema identity, golden fixture or validation rule changes here.

Source `ec8e8a7e72b742999b26b756fa350f20f147dff24cceb70ea8a374ec396fcf36`
passes 1,083 tests: 1,009 library, 62 integration and 12 generator tests. All
seven compiler-selected test executables pass with unchanged source and ordinary
worker stacks. Integration checks include byte-identical fixture regeneration,
filesystem rejection cases and canonical signing/wire fixtures.

The first builds exposed contextual macro separators and borrowed calls to the
owned-value free function. These now use the actual canonical interfaces. The
successful build retains four unused generator imports, removed in a subsequent
source stage. Strict all-target Clippy remains unsuccessful: existing proof and
signer APIs need argument ownership, typed errors, enum-layout and test cleanup.
No lint exceptions were added and this result is not a strict-lint pass.

Exact source stages, compiler artifacts and reports are under
`target/architecture-redesign/model-base-extraction-v1/`. The scoped closeout is
`norito-context-asset-composed-v1/sorafs-context-closeout-v1.json`.

TODO: Resolve the remaining Clippy owners and aggregate/context consumers, then
qualify the combined source. This record does not establish workspace, native,
four-validator, measured memory or release qualification.
