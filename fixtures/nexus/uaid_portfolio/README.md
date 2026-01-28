# UAID Portfolio Fixtures

Golden fixtures for the Nexus UAID portfolio API. These JSON snapshots are
consumed by the `iroha_torii` integration tests to guarantee that `/v1/accounts/{uaid}/portfolio`
returns deterministic data for dataspace bindings used in the roadmap’s NX-16 workstream.

## Files

- `global_default_portfolio.json` — Snapshot for a UAID bound to the default
  dataspace and a single account inside the `portfolio_fixture` domain with two
  asset positions (cash + points). The values match the seed-based builders
  exercised in
  `crates/iroha_torii/tests/accounts_portfolio.rs::accounts_portfolio_snapshot_matches_fixture`.

## Regenerating

Run the deterministic seeding test and capture the JSON output when updating the
fixture:

```bash
cargo test -p iroha_torii --test accounts_portfolio -- \
  accounts_portfolio_snapshot_matches_fixture --exact --nocapture
```

Replace `global_default_portfolio.json` with the emitted JSON and keep the file
sorted/pretty-printed so downstream SDKs can diff it easily.
