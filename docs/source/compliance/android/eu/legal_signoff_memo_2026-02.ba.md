---
lang: ba
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2025-12-29T18:16:35.926476+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 ЕС Юридик ҡултамға-офф иҫкә алыу — 2026.1 GA (Android SDK)

## Һығымта

- **Яҙылыу / Поезд:** 2026.1 ГА (Андроид СДК)
- **Тикшереү датаһы:** 2026-04-15
- **Кәңәшсе / Рецензент:** София Мартинс — үтәү һәм Юридик
- **Сокоп:** ETSI EN 319 401 хәүефһеҙлек маҡсаты, GDPR DPIA резюме, SBOM аттестацияһы, AND6 ҡоролма-лаборатория контингенты дәлилдәре
- *Билеттар менән бәйле:** `_android-device-lab` / AND6-DR-202602, AND6 идара итеү трекеры (`GOV-AND6-2026Q1`)

## Артефакт тикшерелгән исемлек

| Артефакт | SHA-256 | Урыны / Һылтанма | Иҫкәрмәләр |
|---------|----------|----------------|--------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | 2026.1 матчтар GA сығарыу идентификаторҙары һәм хәүеф моделе дельта (Torii NRPC өҫтәүҙәре). |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Һылтанмалар AND7 телеметрия сәйәсәте (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore (`android-sdk-release#4821`). | CycloneDX + провенанс тикшерелгән; матчтар Bubykite эше `android-sdk-release#4821`. |
| Дәлилдәр журналы | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (рәт `android-device-lab-failover-20260220`) | Раҫлауҙар журнал тотоп өйөм хештар + һыйҙырышлы снимок + памятка яҙма. |
| Ҡоролма-лаборатория контингенты өйөмө | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | Хэш `bundle-manifest.json`-тан алынған; Билет AND6-DR-202602 теркәлгән ҡул-законлы/ҡағиҙәһе. |

## Табыштар & Ҡатмарлыҡтар

- Блокинг мәсьәләләре асыҡланмаған. Артефакттар ETSI/GDPR талаптары менән тура килә; AND7 телеметрия паритеты DPIA резюмеһында билдәләнгән һәм өҫтәмә йомшартыуҙар кәрәкмәй.
- Тәҡдим: монитор планлы DR-2026-05-Q2 бурау (билет AND6-DR-202605) һәм һөҙөмтәлә өйөмөнә дәлилдәр журналына сираттағы идара итеү тикшерелгән пункт алдынан.

## Хуплау

- **Ҡарар:** Раҫланған
- **Ҡулым / Timematem:** _София Мартинс (һанлы рәүештә идара итеү порталы аша ҡул ҡуйылған, 2026-04-15 14:32 UTC)_
- **Эҙләү хужалары:** Ҡоролма лабораторияһы Ops (2026-05-31 тиклем DR-2026-05-Q2 дәлилдәр өйөмө)