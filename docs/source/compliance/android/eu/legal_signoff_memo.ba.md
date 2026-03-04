---
lang: ba
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2025-12-29T18:16:35.926037+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 ЕС Юридик ҡултамға-офф иҫтәлек ҡалыптары

Был хәтирәләр теркәлә хоҡуҡи тикшерелгән талап юл картаһы әйбер **AND6** алдынан .
ЕС (ETSI/GDPR) артефакт пакеты көйләүселәргә тапшырыла. Адвокат тейеш клон .
был ҡалыпҡа бер релиз, түбәнге яландарҙы тултырып, ҡул ҡуйылған күсермәһен һаҡлағыҙ
памяткала һылтанма яһалған үҙгәрмәй торған артефакттар менән бер рәттән.

## Һығымта

- **Релиз / Поезд:** `<e.g., 2026.1 GA>`
- **Тикшереү көнө:** `<YYYY-MM-DD>`
- **Кәңәш / Рецензент:** `<name + organisation>`
- **Корона:** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- **Билеттар менән бәйле:** `<governance or legal issue IDs>`

## Артефакт тикшерелгән исемлек

| Артефакт | SHA-256 | Урыны / Һылтанма | Иҫкәрмәләр |
|---------|----------|----------------|--------|
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + идара итеү архивы | Раҫлау идентификаторҙары сығарыу & хәүеф моделе көйләүҙәр. |
| `gdpr_dpia_summary.md` | `<hash>` | Шул уҡ каталог / локализация көҙгөләре | Редакция сәйәсәте һылтанмалар тап `sdk/android/telemetry_redaction.md` тура килә. |
| `sbom_attestation.md` | `<hash>` | Шул уҡ каталог + cosign өйөм дәлилдәр биҙрә | CycloneDX + провенанс ҡултамғаларын тикшерергә. |
| Дәлилдәр журнал рәт | `<hash>` | `docs/source/compliance/android/evidence_log.csv` | 1990 йылда был һан `<n>` |
| Ҡоролма-лаборатория контингенты өйөмө | `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` | Был релизға бәйле репетицияны раҫлай. |

> Өҫтәмә рәттәрҙе беркетергә, әгәр пакетта күберәк файлдар бар (мәҫәлән, хосусилыҡ
> ҡушымталар йәки DPIA тәржемәләре). Һәр артефакт үҙенең үҙгәрмәй торғанына мөрәжәғәт итергә тейеш
> тейәп маҡсатлы һәм Buildkite эше, уны етештерә.

## Табыштар & Ҡатмарлыҡтар

- `None.` *(Ҡалдыҡ хәүефтәрҙе ҡаплаусы пуля исемлеге менән алмаштырыу, компенсациялау
  контроль, йәки кәрәкле эҙмә-эҙлекле ғәмәлдәр.)*

## Хуплау

- **Ҡара:** `<Approved / Approved with conditions / Blocked>`
- **Ҡулым / Ваҡыт тамғаһы:** `<digital signature or email reference>`
- **Эҙләү хужалары:** `<team + due date for any conditions>`

Һуңғы памятка менән идара итеү дәлилдәре биҙрә тейәп, SHA-256 күсермәһенә күсерергә.
`docs/source/compliance/android/evidence_log.csv`, һәм тейәү юлын бәйләй.
`status.md`. Әгәр ҡарар “Блокацияланған” булһа, AND6 рульгә йүнәлергә кәрәк.
комитет һәм документтарҙы тергеҙеү аҙымдары ла юл картаһы эҫе-исемлегендә һәм
ҡоролма-лаборатория контингенты журналы.