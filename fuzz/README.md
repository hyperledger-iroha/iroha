# Security Regression Corpora

This directory consolidates manifests, attachments, and CLI JSON DSL corpora that previously lived under
`scripts/`, `pytests/`, and scattered crate samples. The corpora are replayed by CI to guard against
regressions in validation, manifest handling, and attachment sanitisation.

## Ownership & Update Cadence
- **Owners:** Security Working Group (SWG) with support from the CLI maintainers.
- **Cadence:** Nightly scheduled workflow plus every push/PR touching `fuzz/` or replay tooling. Manual updates are
  expected when new security regression seeds are discovered; batch updates should land within 48 hours of discovery.
- **Escalation:** Failures automatically page the SWG via a GitHub issue (`security-regression` label).

## Directory Layout
- `cli_dsl/` – JSON DSL payloads exercised via `iroha_cli`. Files capture transaction, query, and configuration
  flows used in regression scenarios.
- `attachments/` – Torii attachment corpora (JSON proof envelopes, verifying-key DTOs, binary ZK1 payloads).
  The `attachments/zk/` subdirectory retains helper scripts for reproducing end-to-end demos.
- `manifests/` – Canonical smart-contract manifest fixtures shared across integration tests and replay tooling.
- `corpora.json` – Metadata ledger for every seed: provenance, expected `ValidationFail` outcome (if any), and
  the canonical fingerprint checked in CI.

## Contribution Workflow
1. Add the new corpus under the appropriate subdirectory and document its provenance in `corpora.json`.
2. Run `scripts/replay_security_corpora.py --update` to refresh fingerprints.
3. Update the provenance table below if a new family is introduced.
4. Run `scripts/replay_security_corpora.py` (without `--update`) and ensure it exits successfully.
5. Mention the update in `status.md` (maintenance log) and include any observed `ValidationFail` details.

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
| `attachments/zk/*.json` | Attachment | `crates/iroha_cli/samples/zk/*` | Typically `ValidationFail::NotPermitted` when replayed against hardened Torii |
| `attachments/zk/zk1_min.b64` | Attachment | `crates/iroha_cli/samples/zk/zk1_min.b64` | `ValidationFail::IvmAdmission` (oversized/truncated TLV) |
| `manifests/torii_contract_minimal.json` | Manifest | Extracted from `integration_tests/tests/contracts.rs` | _None_ (used as successful template) |
| `sorafs_chunker/sf1_profile_v1_input.bin` | Chunker | `crates/sorafs_chunker/src/bin/export_vectors.rs` | _None_ |
| `sorafs_chunker/sf1_profile_v1_backpressure.json` | Chunker | `crates/sorafs_chunker/src/bin/export_vectors.rs` | _None_ |

## Replay Harness
The replay harness lives in `scripts/replay_security_corpora.py`. It canonicalises each corpus, computes a
Blake2b-256 fingerprint, and compares it against `corpora.json`. Base64 corpora are decoded prior to hashing.
Resource limits are applied (default: 120 CPU seconds, 1 GiB RAM) to catch runaway regressions early.

To update fingerprints after editing corpora:

```bash
scripts/replay_security_corpora.py --update
scripts/replay_security_corpora.py
```

## Determinism Notes
- Keep JSON corpora canonical (sorted keys, stable whitespace). The replay script re-serialises JSON to a stable
  representation before hashing.
- When adding attachments, include either a real-world failing sample (with a recorded `ValidationFail` variant) or
  a minimal placeholder that still exercises the validation path deterministically.
- Manifests may contain placeholders such as `{CODE_HASH}`; consumers are responsible for substituting runtime values
  prior to submission while leaving the canonical structure intact.

## Fuzzing Entry Points

- `da_replay_cache` (libFuzzer): Exercises the data-availability replay cache (`crates/iroha_core::da`)
  by issuing random insert/clear operations across lane and epoch windows. Run with `cargo fuzz run da_replay_cache`
  from this directory (requires nightly Rust and `cargo-fuzz`). Ensure LLVM sanitiser tooling is installed locally
  before executing the harness.
