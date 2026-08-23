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

The pinned `cargo-fuzz 0.13.2` implementation invokes Cargo internally against `fuzz/Cargo.toml` and exposes no
`--locked`, `--frozen`, or generic Cargo-argument pass-through. The two exact internal-validation receipt commands
also invoke `cargo fuzz run` from the source root. Without a separately reviewed lock contract, either invocation may
create or resolve `fuzz/Cargo.lock`; it cannot be evidence for an immutable source snapshot.

This is a source/gate blocker, not an execution-only gap. Before these seven targets can enter the release gate, the
release owner must explicitly authorize a tracked standalone fuzz lock (including the required `.gitignore`
exception), generate and review it with the pinned toolchain from the exact clean source, and commit it in a newly
signed source tree. The runner must bind that exact lock, force offline/no-update use, prove it unchanged before and
after both campaigns, and only then add pinned strict execution, resource bounds, crash artifact collection, and
static target-inventory tests. Until that work lands, no workflow should simulate or claim these targets.

TODO: obtain explicit authorization for the standalone fuzz-lock exception without modifying the repository's root
`Cargo.lock`, then wire the two Kagemusha targets into the candidate-bound validation receipt with at least
10,000,000 executions each and zero crashes, timeouts, or out-of-memory exits.
