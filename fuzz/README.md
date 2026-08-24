# Security Regression Corpora

This directory consolidates manifests, attachments, and CLI JSON DSL corpora that previously lived under
`scripts/`, `pytests/`, and scattered crate samples. CI verifies their canonical Blake2b-256 fingerprints.
That check is integrity-only: it does not submit a payload to a production parser, Torii, the ledger, or any
other validation boundary.

## Ownership & Update Cadence
- **Owners:** Security Working Group (SWG) with support from the CLI maintainers.
- **Cadence:** The full-workspace release workflow runs nightly, on pushes to `main`, on release tags, and when
  dispatched or called explicitly. Manual updates are expected when new security regression seeds are discovered;
  batch updates should land within 48 hours of discovery.
- **Escalation:** Failures stop the release job and retain any crash artifacts. Repository automation does not
  create or label an issue; the release owner must escalate the failure to the SWG through the incident process.

## Directory Layout
- `cli_dsl/` – JSON DSL payloads exercised via `iroha_cli`. Files capture transaction, query, and configuration
  flows used in regression scenarios.
- `attachments/` – Torii attachment corpora (JSON proof envelopes, verifying-key DTOs, binary ZK1 payloads).
  The `attachments/zk/` subdirectory retains helper scripts for reproducing end-to-end demos.
- `manifests/` – Canonical smart-contract manifest fixtures shared across integration tests and replay tooling.
- `corpora.json` – Metadata ledger for every seed: provenance, a declared expected `ValidationFail` outcome (if
  any), and the canonical fingerprint checked in CI. The expected outcome is a review note, not an observed result.

## Contribution Workflow
1. Add the new corpus under the appropriate subdirectory and document its provenance in `corpora.json`.
2. Run `scripts/replay_security_corpora.py --update` to refresh fingerprints.
3. Update the provenance table below if a new family is introduced.
4. Run `scripts/replay_security_corpora.py` (without `--update`) and ensure it exits successfully.
5. Mention the update in `status.md` (maintenance log). Record a `ValidationFail` as observed only when a separate
   test invokes the real production validation boundary and captures that result.

## Provenance Summary

| File | Kind | Source | Expected `ValidationFail` |
|------|------|--------|---------------------------|
| `cli_dsl/grant_permission.json` | CLI DSL | `pytests/iroha_cli_tests/common/json_isi_examples/grant_permission.json` | _None_ (should succeed) |
| `cli_dsl/revoke_permission.json` | CLI DSL | `pytests/iroha_cli_tests/common/json_isi_examples/revoke_permission.json` | _None_ |
| `cli_dsl/unregister_asset.json` | CLI DSL | `pytests/iroha_cli_tests/common/json_isi_examples/unregister_asset.json` | _None_ |
| `cli_dsl/multisig_set_key_value.json` | CLI DSL | `scripts/tests/multisig.instructions.json` | _None_ |
| `cli_dsl/transaction_log_message.json` | CLI DSL | `crates/iroha_cli/samples/instructions.json` | _None_ |
| `cli_dsl/iterable_accounts_query.json` | CLI DSL | `crates/iroha_cli/samples/query.json` | _None_ |
| `cli_dsl/sumeragi_block_time_parameter.json` | CLI DSL | `crates/iroha_cli/samples/parameter.json` | _None_ |
| `attachments/zk/*.json` | Attachment | `crates/iroha_cli/samples/zk/*` | Declared expectation: typically `ValidationFail::NotPermitted`; not executed by the fingerprint checker |
| `attachments/zk/zk1_min.b64` | Attachment | `crates/iroha_cli/samples/zk/zk1_min.b64` | Declared expectation: `ValidationFail::IvmAdmission`; not executed by the fingerprint checker |
| `manifests/torii_contract_minimal.json` | Manifest | Extracted from `integration_tests/tests/contracts.rs` | _None_ (used as successful template) |
| `sorafs_chunker/sf1_profile_v1_input.bin` | Chunker | `crates/sorafs_chunker/src/bin/export_vectors.rs` | _None_ |
| `sorafs_chunker/sf1_profile_v1_backpressure.json` | Chunker | `crates/sorafs_chunker/src/bin/export_vectors.rs` | _None_ |

## Replay Harness
The integrity harness lives in `scripts/replay_security_corpora.py`. It canonicalises each corpus, computes a
Blake2b-256 fingerprint, and compares it against `corpora.json`. Base64 corpora are decoded prior to hashing.
Resource limits are applied (default: 120 CPU seconds, 1 GiB RAM) to bound this integrity operation. The harness
does not interpret `expected_validation_fail`, execute a semantic validator, or produce validation evidence.

To update fingerprints after editing corpora:

```bash
scripts/replay_security_corpora.py --update
scripts/replay_security_corpora.py
```

## Determinism Notes
- Keep JSON corpora canonical (sorted keys, stable whitespace). The replay script re-serialises JSON to a stable
  representation before hashing.
- When adding attachments, identify whether `expected_validation_fail` is only a declared expectation or link it to
  a separate test that invokes the real production validation path deterministically.
- Manifests may contain placeholders such as `{CODE_HASH}`; consumers are responsible for substituting runtime values
  prior to submission while leaving the canonical structure intact.

## Release-Wired Fuzzing Entry Points

`scripts/fuzz_smoke.sh --strict` runs all currently executable fuzz packages with the pinned nightly and
`cargo-fuzz` version, per-target run/time/RSS bounds, and fail-closed prerequisite checks. Its exact inventory is:

- Norito: `json_parse_string`, `json_parse_string_ref`, `json_skip_value`, `json_from_json_equiv`,
  `aos_view_optstr_equiv`, `aos_view_optu32_equiv`, `aos_view_enum_equiv`, `aos_ncb_equiv_str_u32`, and
  `aos_ncb_equiv_bytes_u32`.
- IVM: `tlv_validate`, `kotodama_lower`, and `numeric_v1`.

The full-workspace release workflow invokes that strict mode and archives crash artifacts. A fake-tool test pins
the exact 9+3 invocation inventory without treating the fake tools as execution evidence.

## Blocked Top-Level Fuzz Targets

`fuzz/Cargo.toml` declares seven additional libFuzzer binaries: `da_replay_cache`, `proof_stream_transport`,
`da_ingest_schema`, `soranet_handshake`, `lane_relay_envelope`, `kagemusha_v4_release_bundle_parser`, and
`kagemusha_v4_recursive_topology`. The manifest is an explicit standalone cargo-fuzz workspace, but these targets are
not release-wired and must not be reported as executed. There is no tracked `fuzz/Cargo.lock`: the repository ignores
nested lockfiles, and the root lock does not contain the standalone fuzz package, `libfuzzer-sys`, or `arbitrary`.
Moving the package into the root workspace would therefore require changing the signed root `Cargo.lock`.

The pinned `cargo-fuzz 0.13.2` implementation invokes `cargo` internally against `fuzz/Cargo.toml` and exposes no
`--locked`, `--frozen`, or generic Cargo-argument pass-through. `CARGO_NET_OFFLINE=true` can prevent network access,
but Cargo has no configuration equivalent for `--locked`; offline resolution alone is not an immutable-lock proof.
Without a separately reviewed lock contract, either nested Cargo invocation may create or resolve
`fuzz/Cargo.lock`; it cannot be evidence for an immutable source snapshot.

The candidate-bound internal-validation receipt now fails closed on the missing production pieces instead of
describing an unverifiable `cargo fuzz` dispatch. It launches an authenticated `cargo-fuzz` role directly, requires
the exact `cargo-fuzz 0.13.2` version output, and separately binds the sanitizer rustc executable and its complete
`rustc -Vv` output (full commit hash/date, host, release, and LLVM version). It also requires an authenticated
locking/offline Cargo proxy for cargo-fuzz's nested launches, binds an exact tracked `fuzz/Cargo.lock`
path/mode/OID/digest/size descriptor, remeasures both locks and the source tree after each campaign, and places target,
corpus, and crash outputs under the sibling
`kagemusha-internal-validation-v1` runtime tree. Cargo-fuzz still calls `create_dir_all` for its default per-target
artifact directory before applying the supplied libFuzzer artifact prefix; the production runner must create those
two empty directories before making the source projection read-only and must reject any file appearing there.
The runner's closed environment manifest must likewise place `TMPDIR`, `HOME`, `CARGO_HOME`, cache locks, and every
compiler/build temporary below its private external runtime root; reviewed dependency and toolchain inputs remain
read-only, and `RUSTC_BOOTSTRAP` and compiler-wrapper variables must be absent.

Those requirements describe mandatory candidate inputs, not local executable readiness. This checkout has no
`cargo-fuzz` executable, its unqualified `nightly` is an older incompatible compiler, and it has no authenticated
locking proxy or production validation runner. The separately named `nightly-2025-11-01` installation is useful only
as the reviewed version-output candidate; its presence does not authenticate its bytes or satisfy the missing runner,
proxy, and lock inputs.

This is a source/gate blocker, not an execution-only gap. Repository policy currently forbids changing any
`Cargo.lock`, so the mandatory standalone lock intentionally remains absent and the receipt cannot honestly be
produced. The minimal external exception is explicit release-owner authorization to add only the reviewed
`fuzz/Cargo.lock` plus its `.gitignore` exception, generated from the exact clean source with the reviewed nightly
Cargo and committed in a newly signed source tree; the root `Cargo.lock` must remain byte-identical. Cargo-fuzz
hardcodes the executable name `cargo`, so a production validation runner must select its authenticated proxy through a
closed `PATH`, not merely set `CARGO`. That proxy must force locked/offline behavior for metadata, build, and run,
while the runner binds the exact cargo-fuzz/proxy/nightly-rustc executable identities, proves the source and both locks
unchanged before and after both campaigns, and retains the canonical external corpus, engine report, logs, and crash
directory. Until that work lands, no workflow should simulate or claim these targets.

The existing authenticated controller's generic `run-v1` contract cannot serve as that runner: it mandates
`--deny-tool-process-spawn` and admits only the one selected executable, while cargo-fuzz necessarily launches Cargo,
which launches rustc, linkers, and authenticated build-script executables. Merely teaching the controller to answer to
the basename `cargo` would leave that process-tree and writable-root policy unauthenticated. A specialized controller
operation therefore needs its own closed-PATH proxy dispatch, exact executable closure, external writable roots, and
OS-isolation qualification before it can mint production evidence.

TODO: obtain explicit authorization for the standalone fuzz-lock exception without modifying the repository's root
`Cargo.lock`, implement and qualify the authenticated validation runner/Cargo proxy, and execute the two already
receipt-wired Kagemusha targets with at least 10,000,000 executions each and zero crashes, timeouts, or out-of-memory
exits.
