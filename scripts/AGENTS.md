# AGENTS Instructions

These guidelines apply to the `scripts/` directory.

## Purpose and layout
- Shell and Python helpers that back CI (`ci/`, `buildkite/`), release automation, fixture regeneration, GPU experiments, etc.
- `test_env.py` provisions a local multi-peer network that is consumed by the Python test suites and manual QA.
- `requirements.txt` is the exact pinned Python dependency set shared by the
  scripts (BLAKE3, JSON Schema validation, pytest, requests, the pre-3.11 TOML
  backport, and `tomli_w`). Its security-patched pytest and HTTP stack require
  Python 3.10+
  even though dependency-free helpers may retain an older syntax floor.
  Install it with `python3 -m pip install -r scripts/requirements.txt` before
  running helpers that import Python modules.

## Development workflow
- Keep scripts idempotent and portable across macOS/Linux. Prefer POSIX shell or Python 3.11+; for long workflows use Python so we can add tests.
- For faster local Rust iteration, prefer `scripts/cargo_fast.sh -- <cargo args...>`; it auto-enables `sccache` without restarting a shared daemon and otherwise leaves the cache location and system linker unchanged. Reuse the workspace target by default, or use a stable `--target-slot <name>` for each concurrent build lane instead of creating one-off target directories. Cargo's native jobserver is the default; `--jobs <N>` is an explicit override and one job is intended only for constrained or evidence lanes. Use `--incremental` for repeated focused test builds in the same warm target, `--no-incremental` for sccache-heavy builds, and `--stable-local-metadata` only for non-release local loops where a fixed `VERGEN_GIT_SHA` is acceptable. Alternative linkers remain available through explicit `--linker auto` or `--linker <name>` selection.
- Every script must document:
  - Purpose and prerequisites at the top of the file.
  - Required environment variables or paths (e.g., `BIN_IROHAD` overrides in `test_env.py`).
  - Safe defaults—never perform destructive actions unless `--force`/explicit flags are provided.
- Provide `--help` output via `argparse`, `click`, or `getopts` so CI pipelines can discover options.
- When updating scripts that feed CI dashboards (`render_*`, `run_sumeragi_*`, `swift_status_export.py`, etc.) also update the consuming documentation under `docs/` or `status.md`.
- Run script unit tests when they exist: `pytest pytests/scripts` covers repository helpers, validation guards, and release automation.
- Treat SDK parity helpers as multi-SDK tooling now: when touching `check_norito_bindings_sync.py`, `check_android_fixtures.py`, `norito_fixture_alignment.py`, or fixture regeneration scripts, keep Java and Kotlin parity expectations aligned.

## Notes
- `test_env.py` assumes the Rust workspace has already been built; run `cargo build --workspace` first so it can reuse the binaries.
- Long-running orchestration scripts (SoraFS, Sumeragi stress, Norito feature matrix) capture artefacts under `run/` directories—ensure they clean up on success and clearly report the paths for later inspection.
- If a script affects build/test flows, mention it in the relevant README or developer doc so other contributors can discover it.
