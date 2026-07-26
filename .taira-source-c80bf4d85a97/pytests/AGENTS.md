# AGENTS Instructions

These guidelines apply to the `pytests/` directory containing the Python test suites.

## Structure
- `iroha_cli_tests/` – end-to-end coverage for the CLI. Managed via Poetry; see its `README.md` for full environment notes.
- `iroha_torii_tests/` – REST/Torii API coverage, also Poetry-managed and Allure-enabled.
- `scripts/` – light-weight unit tests for helper scripts (e.g., `scripts/test_env.py`, `scripts/run_sumeragi_da.py`).
- `test_run_sumeragi_da.py` – exercises report/fixture helpers for the DA stress scripts.

## Environment
- Install shared Python deps once: `python3 -m pip install -r scripts/requirements.txt`.
- Build the Rust workspace (`cargo build --workspace`) before starting the Python suites; `./scripts/test_env.py setup` reuses those binaries to spawn a 4-peer network for CLI/Torii tests.
- CLI tests expect a `.env` file in `pytests/iroha_cli_tests/` that points at the temp directory created by `test_env.py` (see `README.md` for the `TMP_DIR`, `IROHA_CLI_BINARY`, `IROHA_CLI_CONFIG`, `TORII_API_PORT_*` keys).
- Torii tests rely on the same environment and assume Allure is available on `PATH` if you plan to serve reports.

## Development workflow
- Inside each Poetry-managed suite (`iroha_cli_tests`, `iroha_torii_tests`):
  1. `cd pytests/<suite>`
  2. `poetry install --no-root`
  3. `poetry run pytest` (add `-k <expr>` for subsets or `--alluredir allure-results` to emit reports).
- Keep tests deterministic: prefer explicit waits/polling for Torii responses over blind `sleep`.
- Follow the formatters configured in Poetry (`black`, `isort`) and lint with `poetry run flake8`/`poetry run pylint` as defined per suite.
- Top-level unit tests (for scripts) can be run from the repo root with `python3 -m pytest pytests`.
- When tests depend on `test_env.py`, document the exact `./scripts/test_env.py` commands in the README or test doc comment so other contributors can reproduce failures.

## Reporting
- Both suites integrate with Allure. Store artifacts via `pytest --alluredir <dir>` and preview with `allure serve <dir>`.
- Publish any regenerated fixtures (e.g., client configs) alongside the tests to keep CI deterministic.
