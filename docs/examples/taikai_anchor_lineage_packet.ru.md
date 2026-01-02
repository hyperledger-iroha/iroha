---
lang: ru
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-11-21T18:09:53.463728+00:00"
translation_last_reviewed: 2026-01-01
---

# Шаблон пакета родословной якоря Taikai (SN13-C)

Пункт дорожной карты **SN13-C - Manifests & SoraNS anchors** требует, чтобы каждая ротация
alias поставляла детерминированный пакет доказательств. Скопируйте этот шаблон в каталог
артефактов rollout (например
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) и замените плейсхолдеры
перед отправкой пакета в governance.

## 1. Метаданные

| Поле | Значение |
|------|----------|
| ID события | `<taikai.event.launch-2026-07-10>` |
| Stream / rendition | `<main-stage>` |
| Namespace / имя alias | `<sora / docs>` |
| Каталог доказательств | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| Контакт оператора | `<name + email>` |
| Тикет GAR / RPT | `<governance ticket or GAR digest>` |

## Helper пакета (необязательно)

Скопируйте артефакты из spool и сформируйте JSON-резюме (при необходимости подпишите) перед
заполнением остальных разделов:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

Helper извлекает `taikai-anchor-request-*`, `taikai-trm-state-*`, `taikai-lineage-*`, envelopes
и sentinels из spool каталога Taikai (`config.da_ingest.manifest_store_dir/taikai`), чтобы
папка доказательств уже содержала точные файлы, указанные ниже.

## 2. Леджер родословной и hint

Приложите как on-disk леджер родословной, так и JSON hint, который Torii записал для этого
окна. Эти файлы берутся напрямую из
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` и
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| Артефакт | Файл | SHA-256 | Примечания |
|----------|------|---------|------------|
| Леджер родословной | `taikai-trm-state-docs.json` | `<sha256>` | Подтверждает предыдущий digest/окно манифеста. |
| Hint родословной | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | Зафиксирован до загрузки в SoraNS anchor. |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. Фиксация payload anchor

Запишите POST payload, который Torii отправил в anchor сервис. Payload включает
`envelope_base64`, `ssm_base64`, `trm_base64` и inline объект `lineage_hint`; аудит полагается
на эту фиксацию, чтобы доказать hint, отправленный в SoraNS. Torii теперь автоматически
пишет этот JSON как
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
внутри spool каталога Taikai (`config.da_ingest.manifest_store_dir/taikai/`), поэтому
операторы могут просто скопировать его вместо парсинга HTTP логов.

| Артефакт | Файл | SHA-256 | Примечания |
|----------|------|---------|------------|
| Anchor POST | `requests/2026-07-10T18-00Z.json` | `<sha256>` | Raw запрос, скопированный из `taikai-anchor-request-*.json` (Taikai spool). |

## 4. Подтверждение digest манифеста

| Поле | Значение |
|------|----------|
| Новый digest манифеста | `<hex digest>` |
| Предыдущий digest манифеста (из hint) | `<hex digest>` |
| Окно начало / конец | `<start seq> / <end seq>` |
| Время принятия | `<ISO8601>` |

Сошлитесь на hashes ledger/hint, записанные выше, чтобы reviewers могли проверить
замененное окно.

## 5. Метрики / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` snapshot: `<Prometheus query + export path>`
- `/status taikai_alias_rotations` dump (по alias): `<file path + hash>`

Предоставьте export Prometheus/Grafana или вывод `curl`, который показывает инкремент счетчика
и массив `/status` для этого alias.

## 6. Манифест каталога доказательств

Сгенерируйте детерминированный манифест каталога доказательств (spool файлы, capture payload,
снимки метрик), чтобы governance могла проверить каждый хэш без распаковки архива.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Артефакт | Файл | SHA-256 | Примечания |
|----------|------|---------|------------|
| Манифест доказательств | `manifest.json` | `<sha256>` | Приложить к governance пакету / GAR. |

## 7. Checklist

- [ ] Леджер родословной скопирован + захеширован.
- [ ] Hint родословной скопирован + захеширован.
- [ ] POST payload anchor зафиксирован и захеширован.
- [ ] Таблица digest манифеста заполнена.
- [ ] Снимки метрик экспортированы (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] Манифест сгенерирован через `scripts/repo_evidence_manifest.py`.
- [ ] Пакет загружен в governance с hashes + контактами.

Поддержание этого шаблона для каждой ротации alias делает governance bundle SoraNS
воспроизводимым и связывает lineage hints напрямую с доказательствами GAR/RPT.
