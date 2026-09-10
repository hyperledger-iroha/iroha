# Iroha Data Model

Core data structures for the Hyperledger Iroha blockchain.

Names, state paths and their parsing errors are owned by `iroha_model_base`.
Import `iroha_model_base::name::Name`, `iroha_model_base::state_path::StatePath`
and `iroha_model_base::error::ParseError` directly. Ledger composition and the
complete built-in registry remain here. The mixed `base_wire_fixtures` tests
preserve their declared frame identities across this compilation boundary.

## Dependency boundary

The model and its tests must remain independent of node execution, node
configuration, telemetry implementations and storage runtimes. Tests that need
ABI hashes, pointer validation or AXT policy records use the owning `ivm_abi`
crate. Engine execution tests belong in IVM or Core.

Run `python3 scripts/check_dependency_budget.py --check-boundaries --offline`
from the repository root to validate the feature-resolved normal/build graph
and the default/HTTP model graph including its root development dependencies.

Protocol JSON is always available, including with `default-features = false`.
Custom parameters, admission and consensus policies, and retained service metadata
use canonical Norito JSON. Optional governance, HTTP and cryptographic capability
features remain independent of this protocol requirement.

## FFI

`iroha_data_model` exposes types over a foreign function interface with the
`ffi_export` feature:

- `ffi_export` – derive `iroha_ffi::FfiType` for exported types and
  expose helpers for building dynamic libraries.

Use in `Cargo.toml`:

```toml
iroha_data_model = { path = "path/to/iroha_data_model", features = ["ffi_export"] }
```

The feature forwards to the foundational type owners. `ffi_import` is not a
shipping Cargo feature.

## Trait Objects

Many parts of the data model operate on trait objects to allow mixing different
instruction and query types. The [`InstructionBox`] type wraps a
`Box<dyn Instruction>` and the [`QueryBox`] alias wraps a `Box<dyn Query + Send + Sync>`.

```rust
use iroha_data_model::prelude::*;

let instruction: InstructionBox = InstructionBox::from(
    Log::new(Level::INFO, "trait objects".into())
);
let query: QueryBox<Account> = Box::new(FindAccounts);
```

## Formal Verification

The [`verification`](src/verification.rs) module provides deterministic checks for
structural invariants across domains, accounts, asset definitions, and live asset
holdings. Construct a [`WorldSnapshot`] from your in-memory state and call
[`WorldSnapshot::verify`] to obtain a [`VerificationReport`] summarizing the
results, including detailed violation messages when invariants are broken.
