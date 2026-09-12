# Foundational JSON contexts and native metadata ownership

This record covers an isolated full-workspace candidate. The live public JSON
API is not replaced until its remaining consumers move to the canonical API.
The candidate contains real workspace crates, manifests, lockfile and profiles;
it does not substitute dependency stubs.

## Implementation

Chain, domain, name, state-path, topology, peer and metadata owners use one
checked JSON writer with explicit context. Native values are borrowed. Key
codecs preserve validation and pass the caller's context through nested owners.

Metadata rejects a wrong native type with a fixed typed error. Formatting the
arbitrary rejected value with recursive Debug was unsafe for manually constructed
deep values. The new regression rejects and ordinarily drops a 32,768-level
value on a standard worker stack.

Native metadata now delegates to the shared `BTreeMap<Name, Json>` decoder.
This admits destination nodes and invokes the budgeted name and stored-JSON
owners. It replaces a separate loop that used an unbudgeted name constructor and
serialized/reparsed each native value. Tree admission precedes key validation;
malformed key diagnostics come from the canonical name decoder. Valid ordering,
wire identities and metadata's sequence-of-tuples binary layout remain unchanged.

The checked writer previously mapped an exhausted decode budget to an allocation
failure. It now distinguishes those failures before allocating and retains their
typed categories through the second pass and the common JSON error family.
Tests cover exact/short limits, rejected allocation attempts, actual injected
allocator failure and second-pass resource errors.

The fixture migration also accidentally changed one numeric vector to an
inferred byte array. Restoring `Vec<u64>` preserves the original `[1,2,3]` value;
an explicit JSON-text assertion prevents the wire-layout test from silently
exercising a different value. Compiler failures for the separate numeric array
and slice fixture attempts are retained; the supported vector writer is used.

## Qualification

The initial base compilation reported 55 errors from the retired JSON API.
The migrated production and all-target test builds now pass without diagnostics.
The base library passes 90 tests on ordinary stacks. Strict all-target Clippy
passes after removing two redundant test-only `usize` conversions. The coherent
five-package all-target strict Clippy run also passes without diagnostics.

The coherent five-package test build passes at source
`14773ca984f174ac295a6aeaa307d6a49a805e4d340112bc588710400cb26b1a`.
The full Norito, derive, MV, primitives and base runtime selection passes 1,962
tests across 20 executables with one existing ignore. Both compiler UI suites,
the allocation harnesses and all 90 base library tests pass with unchanged source
and ordinary worker stacks. `base-shared-context-full-runtime-2` records the
result; an initial launcher census assertion is retained separately because it
omitted the metadata allocation executable before executing any tests.

After preserving the independent live BFV diagnostic changes, the source is
`2ce3ae86ae9b728a77d9122d9fbb224790f6a1d37e36b1e81c77b31ed7370141`.
The base model passes 93 tests in each of FFI, transparent and combined modes;
combined-mode strict all-target Clippy and all three doctests pass. These are
Rust feature and runtime checks, not JNI or physical-device qualification.
The reconciled crypto build passes, and its focused runtime passes 430 tests
with one existing ignore, including both BFV diagnostic modes and conformance.
Two redundant test-only Copy clones are subsequently removed; strict crypto
Clippy passes. After the final formatting correction, the build and all 430
selected runtime tests pass again with one existing ignore and unchanged source
`a2a0693933d63b09b800928b383c76b0542b50fc2832b8f674dd3cd184a308be`.
The diagnostic and conformance cases run on ordinary worker stacks.

Reports and reviewed before/source stages are under
`target/architecture-redesign/model-base-extraction-v1/`. Resource ownership was
reviewed independently in `json-resource-owner-review-v1`.

## Remaining integration

TODO: Reconcile newer live source changes and qualify the final composed source.
Migrate downstream exhaustive writer-error mappings and remaining contextual
consumers before live cutover. The prepared asset identity extraction still
requires atomic aggregate and external caller migration. Native/device,
full-workspace, four-validator and measured release-memory qualification remain
open; none is implied by these scoped tests.
