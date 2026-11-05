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

## Native CI fixture storage

The action creates a fresh mode-0700 directory under the effective service
account's home from the system account database. It exports that directory as
`TEST_NETWORK_TMP_DIR` and `TAIRA_TESTNET_BEACON_FIXTURE_DIR` through `GITHUB_ENV`.
It does not change `HOME` or `TMPDIR` and does not reuse a configured test root.

`scripts/ci_private_fixture_root.py` requires direct, canonical directory
ancestry owned by the service user or root, with no group/world write access.
The selected home must belong to the service user. The fresh directory must
remain outside Git and fit the native broker's complete 103-byte socket path.
Unsafe ancestry or an oversized path fails setup without a shared-temp fallback.
The native Rust custody checks still independently validate every use.

The job retains the root for its lifetime. Existing failure-artifact paths use
the exported `TEST_NETWORK_TMP_DIR` and retain their network-specific selection;
do not upload the entire root containing beacon custody. The separate
`/private/tmp` replay setup remains available for corridors that explicitly
require it.
