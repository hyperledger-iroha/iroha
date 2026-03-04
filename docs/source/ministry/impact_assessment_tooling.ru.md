---
lang: ru
direction: ltr
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2026-01-03T18:07:57.641039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Инструментарий оценки воздействия (МИНФО‑4b)

Roadmap reference: **MINFO‑4b — Impact assessment tooling.**  
Владелец: Совет управления / Аналитика

В этом примечании описана команда `cargo xtask ministry-agenda impact`, которая теперь
производит автоматический анализ хэш-семейства, необходимый для пакетов референдума.
инструмент использует проверенные предложения Совета по повестке дня, дублирующийся реестр и
дополнительный снимок списка запретов/политики, чтобы рецензенты могли видеть, какие именно
новые отпечатки пальцев, которые противоречат существующей политике, и сколько записей
каждое семейство хешей вносит свой вклад.

## Входы

1. **Предложения по повестке дня.** Один или несколько следующих файлов.
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md).
   Передайте их явно с помощью `--proposal <path>` или укажите команду на
   каталог через `--proposal-dir <dir>` и каждый файл `*.json` по этому пути
   включено.
2. **Дублирование реестра (необязательно).** Соответствующий файл JSON.
   `docs/examples/ministry/agenda_duplicate_registry.json`. Конфликты
   сообщается под `source = "duplicate_registry"`.
3. **Снимок политики (необязательно).** Упрощенный манифест, в котором перечислены все
   Отпечаток пальца уже предусмотрен политикой GAR/министерства. Загрузчик ожидает
   схема показана ниже (см.
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   для полной выборки):

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

Любая запись, чей отпечаток `hash_family:hash_hex` соответствует цели предложения,
сообщается под `source = "policy_snapshot"` со ссылкой `policy_id`.

## Использование

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

Дополнительные предложения можно добавить с помощью повторяющихся флагов `--proposal` или с помощью
предоставление каталога, содержащего весь пакет референдума:

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

Команда выводит сгенерированный JSON на стандартный вывод, если `--out` опущен.

## Вывод

Отчет является подписанным артефактом (запишите его под пакетом референдума).
`artifacts/ministry/impact/`) со следующей структурой:

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

Прикрепите этот JSON к каждому досье референдума вместе с нейтральным резюме, чтобы
члены дискуссии, присяжные и наблюдатели за управлением могут видеть точный радиус взрыва
каждое предложение. Вывод детерминирован (отсортирован по семейству хешей) и безопасен для обработки.
включить в CI/runbooks; если дубликат реестра или моментальный снимок политики изменится,
повторите команду и прикрепите обновленный артефакт до начала голосования.

> **Следующий шаг**: добавьте созданный отчет о влиянии в
> [`cargo xtask ministry-panel packet`](referendum_packet.md), поэтому
> Досье `ReferendumPacketV1` содержит как разбивку по хеш-семейству, так и
> подробный список конфликтов для рассматриваемого предложения.