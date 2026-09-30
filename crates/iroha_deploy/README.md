# iroha_deploy

The deployment engine being built for `iroha network` and `iroha dataspace`.
This crate owns definition parsing and finality verification. `iroha_cli` wires
`iroha dataspace plan/apply/status` to the existing native deployment engine;
the broader network/host engine remains in the phase plan below.

The design, file formats and phase plan are in
[`specs/network_deployment.md`](../../specs/network_deployment.md).

What exists so far:

- `definition`: parsers and validation for network definitions
  (`networks/*.toml`, spec §3.2) and dataspace definitions
  (`dataspaces/*.toml`, spec §3.3). Definitions are read through
  `iroha_config_base`, and every error names its file and key. Unknown keys are
  errors. Dataspaces default to restricted visibility and the parent network's
  committee; owner-run nodes require explicit `committee.source = "owner"`.
  [`dataspaces/dpn.toml`](../../dataspaces/dpn.toml) contains only dataspace
  choices, with no validator or parent-network configuration.
- `iroha dataspace plan/apply/status <definition> --trust <public-profile.json>`:
  the owner key is selected by the definition; the explicit operator credential
  authorizes protected network reads. No client TOML or hand-written native
  manifests are accepted. Plan retains an immutable local artifact; apply plans
  if necessary, then submits each retained transaction at most once; status
  performs no ledger writes. State defaults to `~/.iroha-dataspaces`.
  `max_fee` is one positive aggregate cap across catalog, bootstrap, aliases and
  recovery. Omitting `account_alias` creates no account alias. The supported
  runtime requires an HTTPS parent with exactly four authenticated NPoS
  validators. Owner-node provisioning and explicit `[monitor]` are rejected.
  Focused tests and four-validator runtime qualification remain separate gates.
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
