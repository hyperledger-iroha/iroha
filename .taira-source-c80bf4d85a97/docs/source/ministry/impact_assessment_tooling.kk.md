---
lang: kk
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

# Әсерді бағалау құралы (MINFO‑4b)

Жол картасының анықтамасы: **MINFO‑4b — Әсерді бағалау құралы.**  
Меншік иесі: Басқару кеңесі / Аналитика

Бұл жазба қазір `cargo xtask ministry-agenda impact` пәрменін құжаттайды
референдум пакеттеріне қажетті автоматтандырылған хэш-отбасылық дифференциалды шығарады. The
құрал күн тәртібі кеңесінің расталған ұсыныстарын, қайталанатын тізілімді және
шолушылар нақты қайсысын көре алатындай қосымша бас тарту тізімі/саясат суреті
саусақ іздері жаңа, олар бар саясатқа қайшы келеді және қанша жазба
әрбір хэш отбасы өз үлесін қосады.

## Кіріс

1. **Күн тәртібі бойынша ұсыныстар.** Бір немесе бірнеше файлдар
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md).
   Оларды `--proposal <path>` арқылы анық беріңіз немесе пәрменді a нүктесіне көрсетіңіз
   `--proposal-dir <dir>` және сол жолдың астындағы әрбір `*.json` файлы арқылы каталог
   кіреді.
2. **Тізілімнің қайталануы (міндетті емес).** JSON файлының сәйкестігі
   `docs/examples/ministry/agenda_duplicate_registry.json`. Қақтығыстар
   `source = "duplicate_registry"` астында хабарлады.
3. **Саясат суреті (міндетті емес).** Әрбір тізімі бар жеңіл манифест
   саусақ ізі GAR/Министрлік саясатымен бекітілген. Жүктеуші күтеді
   схемасы төменде көрсетілген (қараңыз
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   толық үлгі үшін):

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

`hash_family:hash_hex` саусақ ізі ұсыныс мақсатына сәйкес келетін кез келген жазба
`source = "policy_snapshot"` астында `policy_id` сілтемесі бар хабарланды.

## Қолдану

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

Қосымша ұсыныстарды қайталанатын `--proposal` жалаушалары арқылы немесе
бүкіл референдум пакетін қамтитын каталогты қамтамасыз ету:

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

`--out` жіберілген кезде пәрмен жасалған JSON файлын stdout файлына басып шығарады.

## Шығару

Есеп қол қойылған артефакт болып табылады (оны референдум пакетінің астына жазыңыз
`artifacts/ministry/impact/` каталогы) келесі құрылыммен:

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

Осы JSON-ды әрбір референдум құжатына бейтарап қорытындымен бірге тіркеңіз
панелистер, алқабилер және басқару бақылаушылары нақты жарылыс радиусын көре алады
әрбір ұсыныс. Шығару детерминирленген (хэш тобы бойынша сұрыпталған) және қауіпсіз
CI/runbooks жүйесіне қосу; қайталанатын тізбе немесе саясат суреті өзгерсе,
пәрменді қайта орындаңыз және дауыс беру ашылғанға дейін жаңартылған артефактты тіркеңіз.

> **Келесі қадам:** жасалған әсер есебін жіберіңіз
> [`cargo xtask ministry-panel packet`](referendum_packet.md) сондықтан
> `ReferendumPacketV1` файлында хэш-отбасылық бөлшектену және
> қаралып жатқан ұсынысқа қатысты жанжалдардың егжей-тегжейлі тізімі.