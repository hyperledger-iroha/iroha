# Native PR validation

Development Rust and OpenAPI jobs use GitHub-hosted Ubuntu 24.04 runners and
this composite action. No prebuilt CI image or registry credentials are required.
The action installs native compiler and library prerequisites, the toolchain in
`rust-toolchain.toml`, Python 3.12 with the repository's script requirements,
Temurin 21 and Node 22.23.4. Each calling job owns its Cargo cache.

The optional `toolchain` input supports checks that explicitly require another
Rust toolchain. Ordinary jobs leave it empty and follow the repository toolchain.
Numeric V1 fuzz PRs select the pinned nightly and run the unchanged strict fuzz
command and limits on a hosted runner; non-PR callers retain their existing runner.
For nested checkouts, `working-directory` selects the directory containing
`scripts/requirements.txt` and defaults to the workspace root.
The action provisions tools; the caller retains its locked builds, tests, lint,
fixture parity and result aggregation.

Python static analysis provisions Python and Poetry directly and checks the
committed locks before installation. Release, publishing and scheduled container
jobs still require their qualified immutable image; see
[the CI image contract](../../CI_IMAGE.md).

Run the hosting configuration regressions with
`python -m pytest -q scripts/tests/ci_image_preflight_test.py`. These checks do
not replace executing Rust, Python or formal suites in CI.
