# Coffee rewards: Kotodama + Musubi

Each coffee earns ten points; negative counts return zero. `quote` is a read-only
contract entrypoint. Four VM tests exercise its `points` helper with 0, 1, 3, and
-1 coffees.

With `musubi` on `PATH`, run from this directory:

```sh
musubi check --offline
musubi tree
musubi test --frozen
musubi build --frozen
musubi metadata
```

`check` creates the local `Musubi.lock`; `--frozen` reuses that exact package graph
offline. This example has no registry dependencies and needs no registry or
signing configuration. Musubi compiles the contract to
`target/kotodama/demo/coffee-club/debug/coffee-club.to`, alongside its interface
and manifest. The tests execute against the declared contract in the IVM.

To build Musubi from the Iroha workspace root:

```sh
cargo build --locked -p musubi --bin musubi
export PATH="$PWD/target/debug:$PATH"
cd examples/coffee-club
```

The package was initially created with:

```sh
musubi new coffee-club --namespace demo
```

The supplied manifest declares the contract and standalone tests. The empty
library module is intentional: this package has no exported library items.
Registry publication and on-chain deployment are separate operations.

Recorded validation: offline check, package tree, four passing VM tests, frozen
bytecode build, and package metadata.
