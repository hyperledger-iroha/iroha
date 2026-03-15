---
lang: ba
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2025-12-29T18:16:35.928660+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC хәүефһеҙлек менән идара итеү тикшерелгән исемлек — Android SDK

| Ялан | Ҡиммәте |
|------|-------|
| Версия | 0,1 (2026-02-12) |
| Скоп | Android SDK + операторы инструменттары ҡулланылған япон финанс таратыу |
| Хужалары | Ҡабул итеү & Юридик (Дэниэль паркы), Android программаһы етәксеһе |

## Контроль матрица

| FISC контроль | Ғәмәлгә ашырыу ентекле | Дәлилдәр / Һылтанмалар | Статус |
|------------|----------------------|---------------------|------------| |
| **Система конфигурацияһы бөтөнлөгө** | `ClientConfig` үтәй, хешинг, схема раҫлау, һәм уҡыу өсөн генә йөрөү мөмкинлеге. Конфигурация перезагрузка етешһеҙлектәре `android.telemetry.config.reload` ваҡиғалар документлаштырылған runbook. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ Өҫтөнлөк |
| **Индеть контроль & аутентификация** | SDK хөрмәт Torii TLS сәйәсәте һәм `/v1/pipeline` ҡул ҡуйҙы запростар; оператор эш ағымы һылтанма ярҙам Playbook §4–5 өсөн эскалация плюс өҫтөндә ҡапҡа аша ҡул ҡуйылған Norito артефакттары. | `docs/source/android_support_playbook.md` X; `docs/source/sdk/android/telemetry_redaction.md` (өҫтөндә эш ағымы). | ✅ Өҫтөнлөк |
| **Криптографик асҡыс менән идара итеү** | StongBox-өҫтөнлөк провайдерҙары, аттестация раҫлау, һәм ҡоролма матрица ҡаплау тәьмин итеү КМС үтәү. `artifacts/android/attestation/` буйынса архивланған аттестация йүгәндәре һәм әҙерлек матрицаһында күҙәтелә. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ Өҫтөнлөк |
| **Яҡшы, мониторинг һәм һаҡлау** | Телеметрия редакция сәйәсәте һиҙгер мәғлүмәттәргә хеш, букеттар ҡоролма атрибуттары, һәм һаҡлауҙы үтәү (7/30/90/365 көнлөк windows). Ярҙам Playbook §8 приборҙар таҡтаһы сиктәрен һүрәтләй; өҫтөнлөктәре `telemetry_override_log.md`-та теркәлгән. | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ Өҫтөнлөк |
| **Операциялар & үҙгәрештәр менән идара итеү** | GA cutover процедураһы (Ярҙам Playbook §7.2) плюс `status.md` яңыртыуҙар трек релиз әҙерлеге. 18NI000000020X аша бәйләнгән дәлилдәр (SBOM, Sigstore. | `docs/source/android_support_playbook.md`; Torii; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ Өҫтөнлөк |
| **Инцидент яуап & отчет** | Playbook билдәләй, ауырлыҡ матрицаһы, SLA яуап windows, һәм үтәү тураһында хәбәр итеү аҙымдары; телеметрия + хаос репетициялары осоусылар алдында ҡабатланыусанлыҡты тәьмин итә. | `docs/source/android_support_playbook.md` §4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ Өҫтөнлөк |
| **Мәғлүмәттәр ординатураһы / локализация** | Телеметрия коллекционерҙары өсөн JP таратыу раҫланған Токио районында эшләй; StongBox аттестация өйөмдәре һаҡланған төбәктә һәм партнер билеттарҙан һылтанма. Локализация планы бета һәм япон телендә бетаға тиклем (AND5) docs тәьмин итә. | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 Прогресста (локализация дауам итә) |

## Рецензент иҫкәрмәләр

- Ҡоролма-матрица яҙмаларын тикшерергә Galaxy S23/S24 көйләнгәнсе партнер onboarding (ҡара: әҙерлек doc рәттәре `s23-strongbox-a`, Sigstore).
- JP таратыуҙа телеметрия коллекционерҙарын тәьмин итеү шул уҡ һаҡлау/өҫтөнлөк логикаһы билдәләнгән DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`).
- Тышҡы аудиторҙарҙан раҫлауҙы алыу бер тапҡыр банк партнерҙары был тикшерелгән исемлекте ҡарап сыға.