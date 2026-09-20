# Coffee rewards: Kotodama + Musubi

Each coffee quotes ten reward points; negative counts return zero. The calculator
does not store or award points. `quote` is a read-only contract entrypoint. Four VM
tests exercise its public argument and return boundary with 0, 1, 3, and -1 coffees.

With `musubi` on `PATH`, run from this directory:

```sh
musubi check
musubi test
musubi build
```

`check` creates the local `Musubi.lock`. This example has no registry
dependencies and needs no registry or signing configuration for local checks.
Use `--frozen` on later checks, tests, or builds to require that exact graph
offline. Musubi compiles the contract to
`target/kotodama/demo/coffee-club/production/coffee-club.to`, alongside its interface
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

The default contract template creates the manifest, runnable contract, and four
standalone tests shown here. No library module is needed. Use
`--template library` when creating a reusable package instead.
Registry publication and on-chain deployment are separate operations.

For Taira, use Musubi's integrated wallet to create or import a signer, obtain
testnet XOR, and inspect its balance. `musubi network configure` binds that wallet
and the authorized contract alias; `musubi deploy` and `musubi view` use the binding.
The [public Musubi guide](https://docs.iroha.tech/guide/tutorials/musubi.html)
covers the supported workflow. Local VM tests do not qualify a live deployment;
a successful deployment requires an Applied receipt and matching chain readback.
