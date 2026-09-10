# Cargo test compilation repair

The workspace now declares framed Norito identities independently of payload
codecs. Model, Core, Torii, daemon, SDK, service and test consumers use the
current trait contract. Borrowed producers project to their owned frame while
retaining logical generic identities. The schema exporter reads declared names.

Both default-member and workspace test builds complete without errors or
warnings. Final build commands use `CARGO_BUILD_JOBS=3` and the normal test
profile:

| Command | Result | Local log |
| --- | --- | --- |
| `cargo test --no-run` | Pass, zero diagnostics | `default-test-no-run-16.log` |
| `cargo test --workspace --no-run` | Pass, zero diagnostics | `workspace-test-no-run-15.log` |
| `cargo test --doc --no-fail-fast -- --test-threads=2` | 76 passed, 4 existing ignored; zero diagnostics | `default-doc-11.log` |

Workspace Rustdoc validation passes 109 tests with 20 existing ignored examples
and zero diagnostics (`workspace-doc-14.log`):

```sh
cargo test --workspace --doc \
  --exclude iroha_js_host --exclude jsonstage1_cuda --exclude jsonstage1_metal \
  --no-fail-fast -- --test-threads=2
```

These three native-only libraries have no Rust doctest examples and Cargo
cannot run doctests for their `cdylib` targets. Normal `cargo test` handles them
without warnings. Their manifests and dependencies are unchanged.

SCCP capability/discovery records have one owner in `iroha_sccp::api`, and PoR
status pages have one owner in `sorafs_manifest::por`. Typed producers and
consumers import those declarations directly; each root accepts one identity.
The [schema contract](../specs/norito_schema_identity.md) records these rules.

Missing proof and observer test modules are restored under filenames that can
be tracked. P2P tests use documented, independent current identity vectors;
original signed/frame fixtures remain unchanged. Unused implementation graphs
are removed, and helpers used exclusively by tests are compiled with their
tests. The native bridge's unconnected lifecycle kernels remain test-only;
their assertions do not qualify an installed backend.
Rustdoc checks select libraries with a supported Rust crate type; native-only
targets are covered by the test builds.

Local artifacts are ignored under `dist/cargo-test-fixes/`. Focused execution
includes owned/borrowed frame equivalence, canonical container identities,
wrong-root rejection, P2P signing frames, bridge authentication and coordinator
tests, Node persistence tests, and all 159 executor and 46 schema-exporter tests.
The restored full-geometry FASTPQ selection passes all 16 proof/codec tests,
including false-statement rejection. Its raw tests use the existing explicit
64 MiB diagnostic allocation policy and retain 32 MiB rejection controls;
production proof and allocation limits are unchanged.

Seven focused SCCP tests pass across Torii, SDK and CLI. The Torii regression
builds real capability/discovery responses, decodes them through the SDK's HTTP
transport and rejects competing root identities. Both canonical PoR page tests
also pass, including wrong-root rejection and cursor bounds.
The SCCP roundtrip/identity and Core proxy regressions also pass on distinct
default-feature artifacts. Both local integration helper regressions pass on
the final workspace artifact, including the corrected caller-argument order.

Workspace formatting, the retired-codec guard, Norito codec-contract checks,
historical-archive verification and `git diff --check` pass. The root status
and roadmap remain within their 300-line limits.

This is local compilation and focused regression evidence. Full workspace
runtime execution, strict workspace Clippy, native-device qualification and
four-validator release qualification require their own results.
