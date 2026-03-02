# AGENTS Instructions

These guidelines apply to the `scripts/` directory.

## Purpose and layout
- Shell and Python helpers that back CI (`ci/`, `buildkite/`), release automation, fixture regeneration, GPU experiments, etc.
- `test_env.py` provisions a local multi-peer network that is consumed by the Python test suites and manual QA.
- `requirements.txt` lists the minimal Python dependencies shared by the scripts (currently `tomli_w`). Install them with `python3 -m pip install -r scripts/requirements.txt` before running helpers that import Python modules.

## Development workflow
- Keep scripts idempotent and portable across macOS/Linux. Prefer POSIX shell or Python 3.11+; for long workflows use Python so we can add tests.
- For faster local Rust iteration, prefer `scripts/cargo_fast.sh -- <cargo args...>`; it auto-enables `sccache` and a supported fast linker when available, supports `--zero-debug` for opt-in compile-throughput mode (`CARGO_PROFILE_{DEV,TEST}_DEBUG=0`), and safely falls back to defaults when not.
- Every script must document:
  - Purpose and prerequisites at the top of the file.
  - Required environment variables or paths (e.g., `BIN_IROHAD` overrides in `test_env.py`).
  - Safe defaults—never perform destructive actions unless `--force`/explicit flags are provided.
- Provide `--help` output via `argparse`, `click`, or `getopts` so CI pipelines can discover options.
- When updating scripts that feed CI dashboards (`render_*`, `run_sumeragi_*`, `swift_status_export.py`, etc.) also update the consuming documentation under `docs/` or `status.md`.
- Run script unit tests when they exist: `pytest pytests/scripts` covers `scripts/test_env.py`, `run_sumeragi_da.py`, and translation helpers.

## Notes
- `test_env.py` assumes the Rust workspace has already been built; run `cargo build --workspace` first so it can reuse the binaries.
- Long-running orchestration scripts (SoraFS, Sumeragi stress, Norito feature matrix) capture artefacts under `run/` directories—ensure they clean up on success and clearly report the paths for later inspection.
- If a script affects build/test flows, mention it in the relevant README or developer doc so other contributors can discover it.
