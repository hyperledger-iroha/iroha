<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Repo Governance Packet Template (Roadmap F1)

Use this template when preparing the artefact bundle required by roadmap item
F1 (repo lifecycle documentation & tooling). The goal is to hand reviewers a
single Markdown file that lists every input, hash, and evidence bundle so the
governance council can replay the bytes referenced in the proposal.

> Copy the template into your own evidence directory (for example
> `artifacts/finance/repo/2026-03-15/packet.md`), replace the placeholders, and
> commit/upload it next to the hashed artefacts referenced below.

## 1. Metadata

| Field | Value |
|-------|-------|
| Agreement/change identifier | `<repo-yyMMdd-XX>` |
| Prepared by / date | `<desk lead> – 2026-03-15T10:00Z` |
| Reviewed by | `<cash owner / collateral holder / desk reviewer>` |
| Packet type | `New immutable agreement / Margin evidence / Maturity settlement` |
| Custodian(s) | `<custodian id(s)>` |
| Linked proposal / referendum | `<governance ticket id or GAR link>` |
| Evidence directory | ``artifacts/finance/repo/<slug>/`` |

## 2. Instruction Payloads

Record the staged Norito instructions that desks signed off on via
`iroha app repo ... --output`. Each entry should include the hash of the emitted
file and a short description of the action that will be submitted once the vote
passes.

| Action | File | SHA-256 | Notes |
|--------|------|---------|-------|
| Initiate | `instructions/initiate.json` | `<sha256>` | Exact proposal used to derive both owner-issued consent hashes. |
| Margin call | `instructions/margin_call.json` | `<sha256>` | Captures cadence + participant id that triggered the call. |
| Maturity settlement | `instructions/unwind.json` | `<sha256>` | Contains only the agreement id; every economic term comes from stored state. |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json \
  | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 Custodian Acknowledgements (tri-party only)

Complete this section whenever a repo uses `--custodian`. The governance packet
must include the custodian's owner-signed maturity
`CanExecuteSettlement` Grant. An operational acknowledgement may be retained
as supplementary evidence, but it does not authorize a ledger debit.

| Custodian | File | SHA-256 | Notes |
|-----------|------|---------|-------|
| `<i105...>` | `grants/custodian_maturity_grant.norito` | `<sha256>` | Owner-signed exact collateral release permission for the canonical proposal. |

> Store the acknowledgement next to the other evidence (`artifacts/finance/repo/<slug>/`)
> so `scripts/repo_evidence_manifest.py` records the file in the same tree as
> the staged instructions and consent records. See
> `docs/examples/finance/repo_custodian_ack_template.md` for a ready-to-fill
> template that matches the governance evidence contract.

## 3. Exact On-Chain Consent

Record both permissions derived from the byte-identical proposal. Repo
admission does not replace agreement terms with node-local configuration and
does not support collateral substitution.

| Phase | Permission owner | Exact debited `AssetId` | Intent hash | Grant transaction / finality receipt |
|-------|------------------|--------------------------|-------------|--------------------------------------|
| Cash at open | `<counterparty>` | `<cash definition + account + scope>` | `<RepoIsi::initiation_intent_hash()>` | `<paths + hashes>` |
| Collateral at maturity | `<counterparty or custodian>` | `<collateral definition + account + scope>` | `<RepoIsi::maturity_intent_hash()>` | `<paths + hashes>` |

Both permissions must target the initiator and use
`settlement_id = RepoIsi::settlement_id()`. Only
`debited_asset.account()` may issue the corresponding Grant.

### 3.1 Agreement and Balance Snapshots

After open, archive the accepted `RepoAgreement` and exact scoped balances:

| Snapshot | File | SHA-256 | Block height | Notes |
|----------|------|---------|--------------|-------|
| Accepted agreement | `state/agreement_open.json` | `<sha256>` | `<block-height>` | Includes `cash_source`, `collateral_custody_asset`, and active status. |
| Exact source balances | `state/balances_open.json` | `<sha256>` | `<block-height>` | Records only the consent-selected scopes. |
| Settled tombstone | `state/agreement_settled.json` | `<sha256>` | `<block-height>` | `settlement_timestamp_ms` equals recorded maturity. |

## 4. Deterministic Test Artefacts

Attach the latest outputs from:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

Record file paths + hashes for the log bundles or JUnit XML produced by your CI
system.

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Lifecycle proof log | `tests/repo_lifecycle.log` | `<sha256>` | Captured with `--nocapture` output. |
| Integration test log | `tests/repo_integration.log` | `<sha256>` | Includes owner-issued Grants, maturity gating, replay rejection, and margin cadence. |

## 5. Lifecycle Proof Snapshot

Every packet must include the deterministic lifecycle snapshot exported from
`repo_deterministic_lifecycle_proof_matches_fixture`. Run the harness with the
export knobs enabled so reviewers can diff the JSON frame and digest against
the fixture tracked in `crates/iroha_core/tests/fixtures/` (see
the lifecycle-proof evidence checklist in `docs/source/finance/repo_ops.md`).

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

Or use the pinned helper to regenerate the fixtures and copy them into your
evidence bundle in one step:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
  --bundle-dir artifacts/finance/repo/<slug>
```

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | Canonical lifecycle frame emitted by the proof harness. |
| Digest file | `repo_proof_digest.txt` | `<sha256>` | Uppercase hex digest mirrored from `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`; attach even when unchanged. |

## 6. Evidence Manifest

Generate the manifest for the entire evidence directory so auditors can verify
hashes without unpacking the archive. The helper mirrors the workflow described
in the evidence-manifest checklist in `docs/source/finance/repo_ops.md`.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/<slug> \
  --agreement-id <repo-identifier> \
  --output artifacts/finance/repo/<slug>/manifest.json
```

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Evidence manifest | `manifest.json` | `<sha256>` | Include the checksum in the governance ticket / referendum notes. |

## 7. Telemetry & Event Snapshot

Export the relevant `AccountEvent::Repo(*)` entries and any dashboards or CSV
exports referenced in `docs/source/finance/repo_ops.md`. Record the files +
hashes here so reviewers can jump straight to the evidence.

| Export | File | SHA-256 | Notes |
|--------|------|---------|-------|
| Repo events JSON | `evidence/repo_events.ndjson` | `<sha256>` | Raw Torii event stream filtered to the desk accounts. |
| Telemetry CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Exported from Grafana using the Repo Margin panel. |

## 8. Approvals & Signatures

- **Dual-control signers:** `<names + timestamps>`
- **GAR / minutes digest:** `<sha256>` of the signed GAR PDF or minutes upload.
- **Storage location:** `governance://finance/repo/<slug>/packet/`

## 9. Checklist

Mark each item once complete.

- [ ] Instruction payloads staged, hashed, and attached.
- [ ] Both owner-issued Grant transactions and finality receipts attached.
- [ ] Exact scoped balance and agreement snapshots attached.
- [ ] Deterministic test logs captured + hashed.
- [ ] Lifecycle snapshot + digest exported.
- [ ] Evidence manifest generated and hash recorded.
- [ ] Event/telemetry exports captured + hashed.
- [ ] Dual-control acknowledgements archived.
- [ ] GAR/minutes uploaded; digest recorded above.

Maintaining this template alongside every packet keeps the governance DAG
deterministic and provides auditors with a portable manifest for repo lifecycle
decisions.
