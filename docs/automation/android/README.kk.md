---
lang: kk
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 676798a4cf7c3e7737a0f80640f3f268a2f625f92afdd359ac528881d2aeb046
source_last_modified: "2025-12-29T18:16:35.060950+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Documentation Automation Baseline (AND5)

Жол картасының AND5 тармағы құжаттаманы, локализацияны және жариялауды талап етеді
автоматтандыру AND6 (CI және Compliance) іске қосылмай тұрып тексерілетін болуы керек. Бұл қалта
AND5/AND6 сілтемесі берілген пәрмендерді, артефактілерді және дәлелдемелерді жазады,
түсірілген жоспарларды бейнелейді
`docs/source/sdk/android/developer_experience_plan.md` және
`docs/source/sdk/android/parity_dashboard_plan.md`.

## Құбырлар және пәрмендер

| Тапсырма | Команда(лар) | Күтілетін артефактілер | Ескертпелер |
|------|------------|--------------------|-------|
| Локализацияны синхрондау | `python3 scripts/sync_docs_i18n.py` (міндетті түрде әр іске `--lang <code>` өту) | `docs/automation/android/i18n/<timestamp>-sync.log` астында сақталған журнал файлы және аударылған көшірме жазбалары | `docs/i18n/manifest.json` аударылған түйіршіктермен синхрондауды сақтайды; журнал түртілген тіл кодтарын және негізгі сызықта түсірілген git міндеттемесін жазады. |
| Norito арматура + паритет тексеру | `ci/check_android_fixtures.sh` (`python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json` орады) | Жасалған жиынтық JSON файлын `docs/automation/android/parity/<stamp>-summary.json` | ішіне көшіріңіз `java/iroha_android/src/test/resources` пайдалы жүктемелерін, манифест хэштерін және қол қойылған арматура ұзындықтарын тексереді. Жиынтықты `artifacts/android/fixture_runs/` астындағы каденс дәлелімен бірге тіркеңіз. |
| Манифест үлгісі және жариялау дәлелі | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (тесттерді іске қосады + SBOM + шығу тегі) | Шығу бумасы метадеректері және нәтижесінде `sample_manifest.json` `docs/source/sdk/android/samples/` `docs/automation/android/samples/<version>/` астында сақталған | AND5 үлгі қолданбаларын байланыстырыңыз және автоматтандыруды бірге шығарыңыз — жасалған манифестті, SBOM хэшін және бета нұсқасын қарау үшін шығу журналын түсіріңіз. |
| Паритет бақылау тақтасының арнасы | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json`, одан кейін `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | `metrics.prom` суретін немесе Grafana экспорттау JSON файлын `docs/automation/android/parity/<stamp>-metrics.prom` ішіне көшіріңіз | AND5/AND7 басқару жарамсыз жіберу есептегіштерін және телеметрияны қабылдауды тексере алатындай бақылау тақтасының жоспарын береді. |

## Дәлелдерді түсіру

1. **Барлығын уақыт белгісімен белгілеңіз.** UTC уақыт белгілерін пайдаланып файлдарды атаңыз
   (`YYYYMMDDTHHMMSSZ`) сондықтан тепе-теңдік бақылау тақталары, басқару хаттамалары және жарияланған
   docs детерминистикалық түрде бірдей іске сілтеме жасай алады.
2. **Анықтамалық орындалады.** Әрбір журнал іске қосудың git commit хэшін қамтуы керек.
   плюс кез келген сәйкес конфигурация (мысалы, `ANDROID_PARITY_PIPELINE_METADATA`).
   Құпиялылық өңдеуді қажет еткенде, ескертпе және қауіпсіз қоймаға сілтеме қосыңыз.
3. **Ең аз контекстті мұрағаттау.** Біз тек құрылымдық қорытындыларды тексереміз (JSON,
   `.prom`, `.log`). Ауыр артефактілер (APK жинақтары, скриншоттар) ішінде қалуы керек
   `artifacts/` немесе журналда жазылған қол қойылған хэш бар нысанды сақтау.
4. **Күй жазбаларын жаңарту.** `status.md` ішінде AND5 кезеңдері алға жылжығанда, сілтеме
   сәйкес файл (мысалы, `docs/automation/android/parity/20260324T010203Z-summary.json`)
   сондықтан аудиторлар CI журналдарын сызып тастамай-ақ негізгі сызықты бақылай алады.

Бұл орналасудан кейін қол жетімді "құжаттар/автоматтандырудың негізгі көрсеткіштері
AND6 Android құжаттама бағдарламасын келтіретін және сақтайтын аудит» алғы шарты
жарияланған жоспарлармен бірге.