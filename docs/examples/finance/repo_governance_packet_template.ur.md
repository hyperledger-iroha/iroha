---
lang: ur
direction: rtl
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fe5fb47af37f86d33bfa884dc920efbf66714bbe3535842a786755dd5649f65
source_last_modified: "2025-12-07T08:26:38.035018+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/finance/repo_governance_packet_template.md کا اردو ترجمہ -->

# Repo Governance Packet Template (Roadmap F1)

جب roadmap item F1 (repo lifecycle documentation & tooling) کے لئے درکار artefact bundle تیار کریں تو یہ template استعمال کریں۔ مقصد یہ ہے کہ reviewers کو ایک واحد Markdown فائل دی جائے جو ہر input، hash، اور evidence bundle کی فہرست دے تاکہ governance council تجویز میں referenced bytes کو replay کر سکے۔

> template کو اپنے evidence directory میں کاپی کریں (مثال
> `artifacts/finance/repo/2026-03-15/packet.md`)، placeholders بدلیں، اور
> نیچے referenced hashed artefacts کے ساتھ commit/upload کریں.

## 1. Metadata

| Field | Value |
|-------|-------|
| Agreement/change identifier | `<repo-yyMMdd-XX>` |
| Prepared by / date | `<desk lead> - 2026-03-15T10:00Z` |
| Reviewed by | `<dual-control reviewer(s)>` |
| Change type | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| Custodian(s) | `<custodian id(s)>` |
| Linked proposal / referendum | `<governance ticket id or GAR link>` |
| Evidence directory | ``artifacts/finance/repo/<slug>/`` |

## 2. Instruction Payloads

desk کی منظور شدہ staged Norito instructions کو
`iroha app repo ... --output` کے ذریعے ریکارڈ کریں۔ ہر entry میں emitted file کا hash اور
ایک مختصر description شامل ہو جو vote پاس ہونے پر submit ہوگی۔

| Action | File | SHA-256 | Notes |
|--------|------|---------|-------|
| Initiate | `instructions/initiate.json` | `<sha256>` | desk + counterparty سے منظور شدہ cash/collateral legs شامل ہیں۔ |
| Margin call | `instructions/margin_call.json` | `<sha256>` | cadence + participant id شامل ہے جس نے call trigger کیا۔ |
| Unwind | `instructions/unwind.json` | `<sha256>` | conditions پوری ہونے پر reverse-leg کا ثبوت۔ |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json       | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 Custodian Acknowledgements (tri-party only)

جب بھی repo `--custodian` استعمال کرے تو یہ سیکشن مکمل کریں۔ governance packet میں ہر custodian کا signed acknowledgement اور `docs/source/finance/repo_ops.md` sec 2.8 میں referenced فائل کا hash شامل ہونا چاہئے۔

| Custodian | File | SHA-256 | Notes |
|-----------|------|---------|-------|
| `<ih58...>` | `custodian_ack_<custodian>.md` | `<sha256>` | Signed SLA جو custody window، routing account اور drill contact کو cover کرتا ہے۔ |

> acknowledgement کو باقی evidence کے ساتھ (`artifacts/finance/repo/<slug>/`) محفوظ کریں تاکہ
> `scripts/repo_evidence_manifest.py` فائل کو اسی tree میں ریکارڈ کرے جہاں staged instructions
> اور config snippets ہوں۔ ملاحظہ کریں
> `docs/examples/finance/repo_custodian_ack_template.md` تاکہ تیار template مل سکے جو governance evidence contract سے میچ کرتا ہے.

## 3. Configuration Snippet

`[settlement.repo]` TOML بلاک پیسٹ کریں جو cluster پر لاگو ہوگا ( `collateral_substitution_matrix` سمیت). hash کو snippet کے ساتھ رکھیں تاکہ auditors runtime policy کی تصدیق کر سکیں جو repo booking کے وقت فعال تھی۔

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 Post-Approval Configuration Snapshots

referendum یا governance vote مکمل ہونے اور `[settlement.repo]` تبدیلی rollout ہونے کے بعد، ہر peer سے `/v1/configuration` snapshots capture کریں تاکہ auditors ثابت کر سکیں کہ approved policy cluster بھر میں live ہے (دیکھیں `docs/source/finance/repo_ops.md` sec 2.9).

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v1/configuration       | jq '.'       > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Peer / source | File | SHA-256 | Block height | Notes |
|---------------|------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | config rollout کے فوراً بعد کا snapshot. |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | `[settlement.repo]` کی staged TOML سے مطابقت کی تصدیق. |

digests کو peer ids کے ساتھ `hashes.txt` (یا برابر summary) میں ریکارڈ کریں تاکہ reviewers یہ ٹریک کر سکیں کہ کون سے nodes نے تبدیلی ingest کی۔ snapshots `config/peers/` میں TOML snippet کے ساتھ رہتے ہیں اور `scripts/repo_evidence_manifest.py` انہیں خودکار طور پر اٹھا لے گا۔

## 4. Deterministic Test Artefacts

تازہ ترین outputs منسلک کریں:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

CI کے بنائے گئے log bundles یا JUnit XML کے file paths + hashes ریکارڈ کریں.

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Lifecycle proof log | `tests/repo_lifecycle.log` | `<sha256>` | `--nocapture` کے ساتھ capture ہوا۔ |
| Integration test log | `tests/repo_integration.log` | `<sha256>` | substitution + margin cadence coverage شامل ہے۔ |

## 5. Lifecycle Proof Snapshot

ہر packet میں `repo_deterministic_lifecycle_proof_matches_fixture` سے export شدہ deterministic lifecycle snapshot شامل ہونا چاہئے۔ harness کو export knobs کے ساتھ چلائیں تاکہ reviewers JSON frame اور digest کو `crates/iroha_core/tests/fixtures/` میں موجود fixture کے خلاف compare کر سکیں (دیکھیں `docs/source/finance/repo_ops.md` sec 2.7).

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json     REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt     cargo test -p iroha_core       -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

یا pinned helper استعمال کریں تاکہ fixtures regenerate ہوں اور evidence bundle میں ایک قدم میں کاپی ہو جائیں:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain>       --bundle-dir artifacts/finance/repo/<slug>
```

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | proof harness سے نکلنے والا canonical lifecycle frame۔ |
| Digest file | `repo_proof_digest.txt` | `<sha256>` | uppercase hex digest جو `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest` سے mirror ہے؛ بغیر تبدیلی کے بھی attach کریں۔ |

## 6. Evidence Manifest

پورے evidence directory کے لئے manifest generate کریں تاکہ auditors archive unpack کئے بغیر hashes verify کر سکیں۔ helper `docs/source/finance/repo_ops.md` sec 3.2 میں بیان کردہ workflow کو mirror کرتا ہے۔

```bash
python3 scripts/repo_evidence_manifest.py       --root artifacts/finance/repo/<slug>       --agreement-id <repo-identifier>       --output artifacts/finance/repo/<slug>/manifest.json
```

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Evidence manifest | `manifest.json` | `<sha256>` | checksum کو governance ticket / referendum notes میں شامل کریں۔ |

## 7. Telemetry & Event Snapshot

متعلقہ `AccountEvent::Repo(*)` entries اور `docs/source/finance/repo_ops.md` میں referenced dashboards یا CSV exports کو export کریں۔ فائلیں + hashes یہاں ریکارڈ کریں تاکہ reviewers سیدھے evidence تک پہنچ سکیں۔

| Export | File | SHA-256 | Notes |
|--------|------|---------|-------|
| Repo events JSON | `evidence/repo_events.ndjson` | `<sha256>` | desk accounts کے لئے filtered raw Torii event stream. |
| Telemetry CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Grafana کے Repo Margin panel سے export. |

## 8. Approvals & Signatures

- **Dual-control signers:** `<names + timestamps>`
- **GAR / minutes digest:** `<sha256>` signed GAR PDF یا minutes upload کا.
- **Storage location:** `governance://finance/repo/<slug>/packet/`

## 9. Checklist

مکمل ہونے پر ہر آئٹم کو نشان زد کریں.

- [ ] Instruction payloads staged, hashed, اور attached.
- [ ] Configuration snippet hash ریکارڈ کیا گیا.
- [ ] Deterministic test logs capture + hashed.
- [ ] Lifecycle snapshot + digest export.
- [ ] Evidence manifest generate اور hash record.
- [ ] Event/telemetry exports capture + hashed.
- [ ] Dual-control acknowledgements archive.
- [ ] GAR/minutes upload; digest اوپر ریکارڈ.

اس template کو ہر packet کے ساتھ رکھنے سے governance DAG deterministic رہتا ہے اور repo lifecycle فیصلوں کے لئے auditors کو portable manifest ملتا ہے۔

</div>
