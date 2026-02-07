---
lang: ba
direction: ltr
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2025-12-29T18:16:35.978367+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Һөнәри баһалау инструменты (МИНФО-4б)

Юл картаһы һылтанма: **МИНФО-4b — Impact баһалау инструменттары.**  
Хужаһы: Идара итеү советы / Аналитика

Был иҫкәрмә документы `cargo xtask ministry-agenda impact` командаһы, тип хәҙер
референдум пакеттары өсөн кәрәкле автоматлаштырылған хеш-ғаилә диффын етештерә. 1990 й.
ҡорал ҡулланыу раҫланған көн тәртибе советы тәҡдимдәре, дубликаты реестры, һәм
опциональ денилист/сәйәсәт снимок шулай рецензенттар аныҡ күрә ала, ниндәй
бармаҡ эҙҙәре яңы, улар ғәмәлдәге сәйәсәт менән бәрелешә, һәм күпме яҙмалар .
һәр хеш ғаиләһе үҙ өлөшөн индерә.

##

1. **Көн һайын тәҡдимдәр.** Бер йәки бер нисә файл, улар үтәй
   [`docs/source/ministry/agenda_council_proposal.md`] (agenda_council_proposal.md).
   Уларҙы асыҡтан-асыҡ `--proposal <path>` менән йәки команданы күрһәтеү өсөн үтергә
   каталогы аша `--proposal-dir <dir>` һәм һәр `*.json` файл был юл аҫтында
   инә.
2. **Дупликат реестры (теләкле).** JSON файл тап килтереп
   `docs/examples/ministry/agenda_duplicate_registry.json`. Конфликттар 1990 й.
   `source = "duplicate_registry"` буйынса хәбәр иткән.
3. **Сәйәсәт снимок (теләһәгеҙ).** Еңел генә манифест, тип исемлеккә һәр .
   бармаҡ эҙҙәре инде тормошҡа ашырылған GAR/Министрлыҡ сәйәсәте. Йөкләүсе көтә
   аҫта күрһәтелгән схема (ҡара:
   [`docs/examples/ministry/policy_snapshot_example.json`] (../../examples/ministry/policy_snapshot_example.json X)
   тулы өлгө өсөн):

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

Теләһә ниндәй яҙма, уның `hash_family:hash_hex` бармаҡ эҙҙәре тәҡдим маҡсатына тап килә
`source = "policy_snapshot"` буйынса хәбәр ителгән `policy_id` һылтанмаһы менән.

## Ҡулланыу

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

Өҫтәмә тәҡдимдәрҙе ҡабатланған `--proposal` флагы аша йәки 2020 йылға күсерергә мөмкин.
каталогты тәьмин итеү, унда тотош референдум партияһы бар:

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

Команда генерацияланған JSON stdout баҫтырып сығара, ҡасан `--out` төшөрөп ҡалдырылған.

## Сығыш

Отчет - был ҡул ҡуйылған артефак
`artifacts/ministry/impact/` каталогы) түбәндәге структура менән:

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

Был JSON беркетергә һәр референдум досье менән бер рәттән нейтраль резюме шулай
панелисттар, присяжныйҙар һәм идара итеү күҙәтеүселәре 2012 йылғы теүәл шартлау радиусын күрә ала.
һәр тәҡдим. Сығыш детерминистик (хеш ғаиләһе тарафынан сорттарға бүлергә) һәм хәүефһеҙ
CI/runbooks индереү; әгәр дубликаты реестр йәки сәйәсәт снимок үҙгәрештәр,
бойороҡто яңынан эшләтеп, тауыш биреүҙең асылғансы яңыртылған артефактты беркетергә.

> **Киләһе аҙым:** генерацияланған йоғонто отчетын туҡландырыу
> [`cargo xtask ministry-panel packet`] (referendum_packet.md) шулай итеп
> `ReferendumPacketV1` досьела хеш-ғаилә тарҡалыуын да, 1990 йылда ла үҙ эсенә ала.
> ентекле конфликт исемлеге өсөн тәҡдим тикшерелгән.