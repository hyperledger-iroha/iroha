# iroha_deploy

The deployment engine behind `iroha network` and `iroha dataspace`. It turns one
hand-written TOML definition into a running network or dataspace. It derives or
observes everything else: keys, genesis, node configs, units and the edge.

The design, file formats and phase plan are in
[`specs/network_deployment.md`](../../specs/network_deployment.md).

What exists so far:

- `definition`: parsers and validation for network definitions
  (`networks/*.toml`, spec §3.2) and dataspace definitions
  (`dataspaces/*.toml`, spec §3.3). Definitions are read through
  `iroha_config_base`, and every error names its file and key. Unknown keys are
  errors.
- `verify::finality`: the light finality verifier behind gate G5 (spec §9,
  §11.2 D-7). It is anchored in an authenticated genesis or a stored
  complete native `checkpoint.norito`, verifies every contiguous successor under
  finite proof and peer budgets, and requires `2f + 1` fresh
  challenge-bound attestations from the authenticated committee. Each epoch's
  committee comes only from the boundary result its predecessor certified. A
  checkpoint that lags by more than one observation budget catches up in
  bounded, individually published pages (`catch_up`). Exact retained
  predecessor decisions permit one block of lag after checkpoint import. Within an
  observation, earlier responses count when their exact decisions were verified in
  its contiguous prefix, including across boundaries. Supplied proofs cannot select
  their own trust roots. The HTTP transport and
  the other gates come in P2.

The committed definitions are `networks/dev.toml`, `networks/ci.toml` and
`networks/perf-10k.toml`. The dataspace examples are in `dataspaces/`.

TODO: add `networks/taira.toml` at release time (P9), once the real host keys
are pinned. Until then `tests/fixtures/taira.toml` has the same shape with
generated keys.
