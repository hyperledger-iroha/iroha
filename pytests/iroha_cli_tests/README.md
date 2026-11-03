# Native CLI smoke tests

This suite runs the delivered `iroha` executable against every peer of the disposable
four-validator network created by `scripts/test_env.py`. It checks the package version,
a signed query for the generated `universal.universal` domain, a nonempty bounded
account collection, and the invalid-limit refusal. Missing inputs or failed commands
fail the suite; no network tests are skipped or replaced by simulated responses.

Build the current `iroha3d`, `iroha`, and `kagami` binaries, then run from the repository root:

```sh
./scripts/test_env.py setup
```

Export the settings printed by setup (or put them in this directory's untracked `.env`):
`TMP_DIR`, `IROHA_CLI_BINARY`, `IROHA_CLI_CONFIG`, `TORII_API_PORT_MIN`, and
`TORII_API_PORT_MAX`. Relative binary and configuration paths resolve inside `TMP_DIR`;
a relative `TMP_DIR` resolves from the pytest working directory. The port range must
name exactly four consecutive peers. `TORII_URL` overrides are refused.

From this directory, run:

```sh
poetry install --no-root
poetry run pytest
```

Each peer query uses a temporary owner-private TOML file beside the generated
`client.toml`. Only `torii_url` changes; account credentials, exact network identity,
relative-path resolution and the bundle generator's `ordinary` transport policy are
preserved. The harness requires the exact `http://127.0.0.1:<port>/` endpoints; managed
localnets separately select `direct_loopback` and are not this suite's setup path.
The original file is untouched and the temporary copy is removed after each case.
Generated credentials remain runtime-only and must never be committed.

`test/test_harness.py` separately checks input selection, private-file cleanup and
failure propagation without starting a network. Those controls do not qualify native
CLI queries. Allure output can be requested with `--alluredir allure-results`.
After testing, run `./scripts/test_env.py cleanup` from the repository root.
