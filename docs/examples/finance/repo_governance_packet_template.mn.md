---
lang: mn
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd018a94197722adfbb9d54bf02f1c486147078174ba4c81f32e9d93b8c3f6d5
source_last_modified: "2026-01-22T16:26:46.473419+00:00"
translation_last_reviewed: 2026-02-07
---

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
| Reviewed by | `<dual-control reviewer(s)>` |
| Change type | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
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
| Initiate | `instructions/initiate.json` | `<sha256>` | Contains the cash/collateral legs approved by desk + counterparty. |
| Margin call | `instructions/margin_call.json` | `<sha256>` | Captures cadence + participant id that triggered the call. |
| Unwind | `instructions/unwind.json` | `<sha256>` | Proof of the reverse-leg once conditions are met. |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json \
  | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 Custodian Acknowledgements (tri-party only)

Complete this section whenever a repo uses `--custodian`. The governance packet
must include a signed acknowledgement from each custodian plus the hash of the
file referenced in §2.8 of `docs/source/finance/repo_ops.md`.

| Custodian | File | SHA-256 | Notes |
|-----------|------|---------|-------|
| `<ih58...>` | `custodian_ack_<custodian>.md` | `<sha256>` | Signed SLA covering custody window, routing account, and drill contact. |

> Store the acknowledgement next to the other evidence (`artifacts/finance/repo/<slug>/`)
> so `scripts/repo_evidence_manifest.py` records the file in the same tree as
> the staged instructions and config snippets. See
> `docs/examples/finance/repo_custodian_ack_template.md` for a ready-to-fill
> template that matches the governance evidence contract.

## 3. Configuration Snippet

Paste the `[settlement.repo]` TOML block that will land on the cluster (including
`collateral_substitution_matrix`). Store the hash next to the snippet so
auditors can confirm the runtime policy that was active when the repo booking
was approved.

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 Post-Approval Configuration Snapshots

After the referendum or governance vote completes and the `[settlement.repo]`
change is rolled out, capture `/v1/configuration` snapshots from every peer so
auditors can prove the approved policy is live across the cluster (see
`docs/source/finance/repo_ops.md` §2.9 for the evidence workflow).

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v1/configuration \
  | jq '.' \
  > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Peer / source | File | SHA-256 | Block height | Notes |
|---------------|------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | Snapshot captured immediately after the config rollout. |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | Confirms `[settlement.repo]` matches the staged TOML. |

Record the digests alongside the peer ids in `hashes.txt` (or the equivalent
summary) so reviewers can trace which nodes ingested the change. The snapshots
live under `config/peers/` next to the TOML snippet and will be picked up
automatically by `scripts/repo_evidence_manifest.py`.

## 4. Deterministic Test Artefacts

Attach the latest outputs from:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

Record file paths + hashes for the log bundles or JUnit XML produced by your CI
system.

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Lifecycle proof log | `tests/repo_lifecycle.log` | `<sha256>` | Captured with `--nocapture` output. |
| Integration test log | `tests/repo_integration.log` | `<sha256>` | Includes substitution + margin cadence coverage. |

## 5. Lifecycle Proof Snapshot

Every packet must include the deterministic lifecycle snapshot exported from
`repo_deterministic_lifecycle_proof_matches_fixture`. Run the harness with the
export knobs enabled so reviewers can diff the JSON frame and digest against
the fixture tracked in `crates/iroha_core/tests/fixtures/` (see
`docs/source/finance/repo_ops.md` §2.7).

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
in `docs/source/finance/repo_ops.md` §3.2.

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
- [ ] Configuration snippet hash recorded.
- [ ] Deterministic test logs captured + hashed.
- [ ] Lifecycle snapshot + digest exported.
- [ ] Evidence manifest generated and hash recorded.
- [ ] Event/telemetry exports captured + hashed.
- [ ] Dual-control acknowledgements archived.
- [ ] GAR/minutes uploaded; digest recorded above.

Maintaining this template alongside every packet keeps the governance DAG
deterministic and provides auditors with a portable manifest for repo lifecycle
decisions.
