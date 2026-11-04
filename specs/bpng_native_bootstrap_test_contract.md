# BPNG native bootstrap test contract

`integration_tests/tests/alias_registry_bootstrap_network.rs` has two explicit
entries into one four-validator scenario. Both exercise paid SNS acquisition,
catalog expansion, exact validator bindings, certified lane execution, retained
Kura history and Strict restart. A missing network fails both entries. Neither
entry is ignored, and the scenario does not branch on the qualification mode.

The default `network_functional` harness runs
`alias_registry_bootstrap_network::bpng_development_bootstrap_survives_four_peer_retained_kura_catalog_expansion`.
It requires explicit canonical `TEST_NETWORK_BIN_IROHAD`,
`TEST_NETWORK_BIN_IROHA` and `KAGAMI_BIN` paths. Their actual version responses
must match the test executable's full compiled `VERGEN_GIT_SHA` and package
version. The ordinary PR producer and consumer bind that revision to checkout
HEAD. Probes run without configuration or inherited environment in an empty
temporary directory. Development admission rejects release anchors; a failed
release invocation never falls back to this entry. This result is development
regression evidence only.

The `bpng-release-qualification` feature adds the nonignored strict entry
`alias_registry_bootstrap_network::bpng_native_bootstrap_survives_four_peer_retained_kura_catalog_expansion`.
It retains `required_prebuilt_binaries()` unchanged: exact release profile,
lookup-only programs, clean read-only sealed source at the compiled path,
retained source identity, pinned Git/Python-only PATH, and native manifest-bound
program admission. Enabling the feature supplies none of these prerequisites.
The default development entry remains separately named and cannot satisfy the
strict filter.

An authenticated release build owner compiles the harness from the exact sealed
source using its existing external target and Cargo custody:

```sh
cargo test --locked --release -p integration_tests --test network_functional \
  --features bpng-release-qualification --no-run --message-format=json-render-diagnostics
```

The owner selects and retains that exact Cargo executable artifact, then invokes
it under its admitted release environment and pinned runtime PATH:

```sh
"$qualified_network_functional_harness" \
  --exact alias_registry_bootstrap_network::bpng_native_bootstrap_survives_four_peer_retained_kura_catalog_expansion \
  --nocapture --test-threads=1
```

The release owner must verify the exact test inventory before execution and
retain the successful one-test result alongside source, artifact and network
evidence. An unqualified environment fails before network startup. The feature
and invocation contract are checked by
`pytests/scripts/bpng_native_test_contract_test.py`; source checks alone do not
establish network or release qualification.

Staking plans use the original policy already signed by the custom genesis
factory. The fixture does not override retired node-local staking fields or
rewrite signed history to choose different custody.
