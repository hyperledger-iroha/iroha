---
lang: ru
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fe5fb47af37f86d33bfa884dc920efbf66714bbe3535842a786755dd5649f65
source_last_modified: "2025-12-07T08:26:38.035018+00:00"
translation_last_reviewed: 2026-01-01
---

# Шаблон governance packet для repo (Roadmap F1)

Используйте этот шаблон при подготовке набора artefacts, требуемого пунктом roadmap
F1 (документация и tooling жизненного цикла repo). Цель — предоставить reviewers
единый Markdown-файл, перечисляющий каждый input, hash и evidence bundle, чтобы
governance council смог воспроизвести bytes, на которые ссылается предложение.

> Скопируйте шаблон в свой evidence directory (например
> `artifacts/finance/repo/2026-03-15/packet.md`), замените placeholders и
> закоммитьте/загрузите его рядом с hashed artefacts, указанными ниже.

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

Зафиксируйте staged Norito instructions, одобренные desk через
`iroha app repo ... --output`. Каждая запись должна включать hash файла и короткое
описание действия, которое будет отправлено после прохождения голосования.

| Action | File | SHA-256 | Notes |
|--------|------|---------|-------|
| Initiate | `instructions/initiate.json` | `<sha256>` | Содержит cash/collateral legs, одобренные desk + counterparty. |
| Margin call | `instructions/margin_call.json` | `<sha256>` | Фиксирует cadence + participant id, вызвавший call. |
| Unwind | `instructions/unwind.json` | `<sha256>` | Доказательство reverse-leg после выполнения условий. |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json       | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 Custodian Acknowledgements (tri-party only)

Заполните этот раздел, если repo использует `--custodian`. Governance packet
должен включать подписанный acknowledgement от каждого custodian и hash файла,
указанного в sec 2.8 `docs/source/finance/repo_ops.md`.

| Custodian | File | SHA-256 | Notes |
|-----------|------|---------|-------|
| `<i105...>` | `custodian_ack_<custodian>.md` | `<sha256>` | Подписанный SLA с custody window, routing account и drill contact. |

> Сохраните acknowledgement рядом с другими evidence (`artifacts/finance/repo/<slug>/`),
> чтобы `scripts/repo_evidence_manifest.py` записал файл в том же дереве, что и staged instructions
> и config snippets. См.
> `docs/examples/finance/repo_custodian_ack_template.md` для готового шаблона, соответствующего
> governance evidence contract.

## 3. Configuration Snippet

Вставьте TOML-блок `[settlement.repo]`, который будет развернут на кластере (включая
`collateral_substitution_matrix`). Сохраните hash рядом со snippet, чтобы auditors
могли подтвердить runtime policy, активную на момент approval repo booking.

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 Post-Approval Configuration Snapshots

После завершения referendum или governance vote и rollout изменения `[settlement.repo]`
снимите `/v2/configuration` snapshots с каждого peer, чтобы auditors могли доказать,
что утвержденная политика активна на всем кластере (см.
`docs/source/finance/repo_ops.md` sec 2.9 для workflow evidence).

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v2/configuration       | jq '.'       > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Peer / source | File | SHA-256 | Block height | Notes |
|---------------|------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | Snapshot сразу после rollout config. |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | Подтверждает, что `[settlement.repo]` совпадает со staged TOML. |

Запишите digests рядом с peer ids в `hashes.txt` (или эквивалентном summary), чтобы reviewers
могли отследить, какие nodes приняли изменение. Snapshots лежат в `config/peers/` рядом со
snippet TOML и будут автоматически подобраны `scripts/repo_evidence_manifest.py`.

## 4. Deterministic Test Artefacts

Приложите последние outputs из:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

Запишите пути файлов + hashes для log bundles или JUnit XML, созданных вашим CI.

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Lifecycle proof log | `tests/repo_lifecycle.log` | `<sha256>` | Захвачено с `--nocapture`. |
| Integration test log | `tests/repo_integration.log` | `<sha256>` | Включает substitution + margin cadence coverage. |

## 5. Lifecycle Proof Snapshot

Каждый packet должен включать deterministic lifecycle snapshot, экспортированный из
`repo_deterministic_lifecycle_proof_matches_fixture`. Запустите harness с включенными
export knobs, чтобы reviewers могли сравнить JSON frame и digest с fixture в
`crates/iroha_core/tests/fixtures/` (см.
`docs/source/finance/repo_ops.md` sec 2.7).

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json     REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt     cargo test -p iroha_core       -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

Или используйте pinned helper для регенерации fixtures и копирования их в bundle evidence за один шаг:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain>       --bundle-dir artifacts/finance/repo/<slug>
```

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | Канонический lifecycle frame, выданный proof harness. |
| Digest file | `repo_proof_digest.txt` | `<sha256>` | Uppercase hex digest, отраженный из `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`; приложите даже без изменений. |

## 6. Evidence Manifest

Сгенерируйте manifest для всего evidence directory, чтобы auditors могли проверять hashes без распаковки архива. Helper повторяет workflow, описанный в
`docs/source/finance/repo_ops.md` sec 3.2.

```bash
python3 scripts/repo_evidence_manifest.py       --root artifacts/finance/repo/<slug>       --agreement-id <repo-identifier>       --output artifacts/finance/repo/<slug>/manifest.json
```

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Evidence manifest | `manifest.json` | `<sha256>` | Укажите checksum в governance ticket / referendum notes. |

## 7. Telemetry & Event Snapshot

Экспортируйте релевантные записи `AccountEvent::Repo(*)` и любые dashboards или CSV exports, упомянутые в `docs/source/finance/repo_ops.md`. Запишите файлы + hashes здесь, чтобы reviewers могли сразу перейти к evidence.

| Export | File | SHA-256 | Notes |
|--------|------|---------|-------|
| Repo events JSON | `evidence/repo_events.ndjson` | `<sha256>` | Raw Torii event stream, отфильтрованный по desk accounts. |
| Telemetry CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Exported из Grafana через панель Repo Margin. |

## 8. Approvals & Signatures

- **Dual-control signers:** `<names + timestamps>`
- **GAR / minutes digest:** `<sha256>` от подписанного GAR PDF или minutes upload.
- **Storage location:** `governance://finance/repo/<slug>/packet/`

## 9. Checklist

Отметьте каждый пункт после завершения.

- [ ] Instruction payloads staged, hashed и приложены.
- [ ] Hash configuration snippet записан.
- [ ] Deterministic test logs получены + hashed.
- [ ] Lifecycle snapshot + digest экспортированы.
- [ ] Evidence manifest сгенерирован и hash записан.
- [ ] Event/telemetry exports собраны + hashed.
- [ ] Dual-control acknowledgements архивированы.
- [ ] GAR/minutes загружены; digest записан выше.

Поддержание этого шаблона рядом с каждым packet делает governance DAG детерминированным и дает auditors переносимый manifest для решений жизненного цикла repo.
