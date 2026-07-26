---
lang: ba
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-05T09:28:11.999717+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
--> X

# Android сығарыу исемлеге (AND6)

Был тикшерелгән исемлек **AND6 — CI & Ҡатышлау ** ҡапҡалары 2012 йылдан алып 1990 й.
`roadmap.md` (§Приоритет 5). Ул тура килә Android SDK менән релиздар Rust .
RFC өмөттәрен сығарыу, CI эш урындарын яҙып, үтәү артефакттары,
ҡоролма-лаборатория дәлилдәре, һәм провенанс өйөмдәре, уларҙы GA алдынан беркетергә тейеш,
ЛТС, йәки ҡыҙыу поезд алға бара.

Был документ менән бергә ҡулланыу:

- `docs/source/android_support_playbook.md` — релиз календары, SLAs, һәм
  эскалация ағасы.
- `docs/source/android_runbook.md` — көн‐көн оператив runbooks.
- `docs/source/compliance/android/and6_compliance_checklist.md` — көйләүсе
  артефакт инвентаризацияһы.
- `docs/source/release_dual_track_runbook.md` — ике юллы релиз идара итеү.

## 1. Этап ҡапҡалар бер ҡараштан

| Этап | Кәрәкле ҡапҡалар | Дәлилдәр |
|-------|----------------|---------- |
| **Т−7 көн (алдан туңдырыу)** | Төнгө `ci/run_android_tests.sh` йәшел 14 көн; `ci/check_android_fixtures.sh`, `ci/check_android_samples.sh`, һәм `ci/check_android_docs_i18n.sh` үткән; линт/зависимый сканерлай сират. | Buildkite приборҙар таҡтаһы, ҡоролма дифф отчет, өлгө скриншот снимоктары. |
| **Т−3 көн (РК промоушен)** | Ҡоролма-лаборатория бронирование раҫланған; КөслөБокс аттестация CI йүгерә (`scripts/android_strongbox_attestation_ci.sh`); Планлаштырылған аппаратта ҡулланылған роболектр/инструментлы люкс; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` таҙа. | Ҡоролма матрицаһы CSV, аттестация өйөмө манифест, Градл хәбәр итә, архивланған `artifacts/android/lint/<version>/`. |
| **Т−1 көн (г/юҡ)*** | Телеметрия редакция статусы өйөмдәре яңыртылған (`scripts/telemetry/check_redaction_status.py --write-cache`); үтәү артефакттар яңыртылған `and6_compliance_checklist.md`; провенанс репетицияһы тамамланды (`scripts/android_sbom_provenance.sh --dry-run`). | `docs/source/compliance/android/evidence_log.csv`, телеметрия статусы JSON, провенанс ҡоро-йүгерә журнал. |
| **Т0 (GA/LTS өҙөк)** | `scripts/publish_android_sdk.sh --dry-run` тамамланды; провенанс + SBOM ҡул ҡуйылған; сығарыу тикшерелгән исемлек экспорт һәм беркетелгән барырға/юҡ минут; `ci/sdk_sorafs_orchestrator.sh` төтөн эш йәшел. | RFC ҡушымталарын сығарыу, Sigstore өйөмө, `artifacts/android/` буйынса ҡабул итеү артефакттары. |
| **Т+1 көн (пост-кучал)** | Ҡайнарфикс әҙерлек раҫланған (Sigstore); приборҙар таҡтаһы диффтары тикшерелгән (`ci/check_android_dashboard_parity.sh`); дәлилдәр пакеты тейәлгән `status.md`. | Приборҙар таҡтаһы дифф экспорты, һылтанма `status.md` яҙма, архивланған релиз пакет. |

## 2. CI & Сифат ҡапҡа матрица| Ҡапҡа | Команда(тар) / Сценарий | Иҫкәрмәләр |
|-----|--------------------|------- |
| Блок + интеграция һынауҙары | `ci/run_android_tests.sh` (уҡыу `ci/run_android_tests.sh`) | `artifacts/android/tests/test-summary.json` + тест журналы. Norito кодек, сират, StrongBox fallback, һәм Torii клиент йүгән һынауҙарын үҙ эсенә ала. Төндә һәм теглау алдынан талап ителә. |
| Фикстура паритеты | `ci/check_android_fixtures.sh` (уҡыу `scripts/check_android_fixtures.py`) | Norito ҡоролмалары регенерацияланғанын тәьмин итә, был Rust канонлы комплектҡа тап килә; беркетергә JSON diff, ҡасан ҡапҡа етешһеҙлектәр. |
| Өлгө ҡушымталар | `ci/check_android_samples.sh` | `examples/android/{operator-console,retail-wallet}` төҙөй һәм локалләштерелгән скриншоттарҙы `scripts/android_sample_localization.py` аша раҫлай. |
| Док/I18N | `ci/check_android_docs_i18n.sh` | Һаҡсылар УҠЫҒЫҘ + локалләштерелгән тиҙ старттар. Тағы ла йүгереп һуң doc монтажлау ерләү филиалында ер. |
| Приборҙар таҡтаһы паритеты | `ci/check_android_dashboard_parity.sh` | CI/экспортлы метрика раҫлауҙары Rust коллегалары менән тура килә; Т+1 тикшерелеүе ваҡытында талап ителә. |
| SDK ҡабул итеү төтөн | `ci/sdk_sorafs_orchestrator.sh` | Күнекмәләр күп сығанаҡлы Сораф оркестрҙары менән бәйләүҙәр ағымдағы SDK. Сәхнә артефакттарын тейәү алдынан талап ителә. |
| Аттестация тикшерелеүе | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | `artifacts/android/attestation/**` буйынса КөслөБокс/ТЭЭ аттестация өйөмдәрен агрегаттар; резюмены GA пакеттарына беркетергә. |
| Ҡоролма-лаборатория слот раҫлау | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | Приборҙар өйөмдәрен раҫлай, пакеттарҙы сығарыу өсөн дәлилдәр беркеткәнсе; CI ҡаршы йүгерә өлгө слот `fixtures/android/device_lab/slot-sample` (телеметрия/аттестация/сират/логтар + `sha256sum.txt`). |

> **Кәңәш:** был эштәрҙе өҫтәп `android-release` Buildkite торбаһы, шулай итеп, тип
> туңдырыу аҙналары автоматик рәүештә яңынан йүгерә һәр ҡапҡа менән сығарыу филиалы осо.

Консолидацияланған `.github/workflows/android-and6.yml` эше линт эшләй,
һынау-люкс, аттестация-йомғаҡлау, һәм ҡоролма-лаборатория слот тикшерелгән һәр PR/push
ҡағыла Android сығанаҡтары, тейәп дәлилдәр аҫтында `artifacts/android/{lint,tests,attestation,device_lab}/`.

## 3. Линт һәм сканерлауға бәйлелек

Run `scripts/android_lint_checks.sh --version <semver>` репо тамырҙан. 1990 й.
сценарий башҡарыла:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- Отчеттар һәм бәйлелек-һаҡсыллыҡ сығыштары 2012 йылға тиклем архивланған.
  `artifacts/android/lint/<label>/` һәм `latest/` symlink
  торбалар.
- Уңышһыҙлыҡ линт табыштары йә төҙәтеү йәки релизға яҙма талап итә.
  RFC документлаштырыу ҡабул ителгән хәүеф (раҫланған сығарыу инженерияһы + Программа .
  Алып барырға).
- `dependencyGuardBaseline` тергеҙелә бәйлелек блокировкаһы; диффты беркетергә
  йөрөү/юҡ пакетҡа.

## 4. Ҡоролма лабораторияһы & Көслөбокс ҡаплау

.
   `docs/source/compliance/android/device_lab_contingency.md`. Блоктар сығарыла
   әгәр ` аттестация отчетын яңыртыу өсөн.
3. Приборҙар матрицаһын эшләтеү (ҡоролмала люкс/АБИ исемлеген документлаштырыу
   трекер). Инцидент журналында уңышһыҙлыҡтар алыу хатта ретиялар уңышҡа өлгәшһә лә.
4. Билет файл, әгәр fallback Firebase һынау лабораторияһы кәрәк; билетты һылтанма
   түбәндәге тикшерелгән исемлектә.

## 5. Ҡабул итеү & Телеметрия артефакттары- ЕС өсөн `docs/source/compliance/android/and6_compliance_checklist.md` эҙләп
  һәм JP тапшырыуҙары. Яңыртыу `docs/source/compliance/android/evidence_log.csv`
  хештар менән + Buildkite эш URL-адрестары.
- Яңыртыу телеметрия редакция дәлилдәре аша .
  `сценарий/телеметрия/тикшереү_редация_статус.пи --яҙыу-кэш \.
   --статус-урл https://android-observability.example/status.json`.
  Һөҙөмтәлә барлыҡҡа килгән JSON 1990 йылдарҙа һаҡлағыҙ.
  `artifacts/android/telemetry/<version>/status.json`.
- Схема диффы сығышын яҙып алығыҙ.
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  экспортерҙар менән паритет иҫбатлау өсөн.

## 6. Провенанс, СБОМ, һәм нәшриәт

1. Ҡоро-йүгерергә нәшер итеү торбаһы:

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. SBOM + Sigstore провенанс генерациялау:

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. `artifacts/android/provenance/<semver>/manifest.json` беркетергә һәм ҡул ҡуйған
   `checksums.sha256` RFC сығарыуға.
4. Ысын Maven репозиторийына пропагандалау ваҡытында, ҡабаттан эшләгеҙ
   `scripts/publish_android_sdk.sh` `--dry-run` булмаған
   лог, һәм һөҙөмтәлә алынған артефакттарҙы `artifacts/android/maven/<semver>` XX.

## 7. тапшырыу пакеты ҡалып

Һәр GA/LTS/hotfix релиз тейеш:

1. **Башланған тикшерелгән исемлек ** — был файлды күсерергә’таблица, һәр әйберҙе галочкой, һәм һылтанма .
   ярҙам итеү өсөн артефакттар (Buildkite йүгерә, логтар, doc diffs).
2. **Ҡоролма лаборатория дәлилдәре** — аттестация тураһында отчет дөйөм, бронирование журналы, һәм
   теләһә ниндәй ғәҙәттән тыш хәлдәр активацияһы.
3. **Телеметрия пакет** — редакция статусы JSON, схема дифф, һылтанма
   `docs/source/sdk/android/telemetry_redaction.md` яңыртыуҙар (әгәр бар икән).
4. **Тотолоу артефакттары ** — өҫтәлгән/яңыртылған яҙмаларҙы үтәү папкаһында
   плюс яңыртылған дәлилдәр журналы CSV.
5. **Провенанс өйөм** — СБОМ, Sigstore ҡултамғаһы, һәм `checksums.sha256`.
6. **Яҙма резюме** — бер бит дөйөм мәғлүмәт беркетелгән `status.md` summarizing
   өҫтәгеләр (дата, версия, теләһә ниндәй баш тартылған ҡапҡаларҙағы айырыу).

Пакетты һаҡлау өсөн `artifacts/android/releases/<version>/` һәм уны һылтанма
`status.md` һәм RFC сығарыу.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` автоматик рәүештә
  һуңғы линт архивын күсерәләр (`artifacts/android/lint/latest`) һәм .
  үтәү дәлилдәре `artifacts/android/releases/<version>/` шулай итеп, шулай
  тапшырыу пакеты һәр ваҡыт канонлы урынлашҡан.

---

**Иҫкә ҡаршы:** был тикшерелгән исемлекте яңыртыу, ҡасан яңы CI эш урындары, үтәү артефакттары,
йәки телеметрия талаптары өҫтәлә. Юл картаһы әйбер AND6 асыҡ ҡала, тиклем .
тикшерелгән исемлек һәм уның менән бәйле автоматлаштырыу ике рәттән сығарыу өсөн тотороҡло булып сыға
поездар.