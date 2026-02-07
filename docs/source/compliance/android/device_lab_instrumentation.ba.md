---
lang: ba
direction: ltr
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2025-12-29T18:16:35.924058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android ҡоролма лабораторияһы приборҙары ҡармаҡтары (AND6)

Был һылтанма юл картаһы ғәмәлен ябып ҡуя “ҡалған ҡоролма-лаборатория / сәхнәлә /
инструментация AND6 старттан алда ҡармаҡтар. Ул һәр запасҡа нисек аңлатыла
ҡоролма-лаборатория слот тейеш тотоп телеметрия, сират, һәм аттестация артефакттары шулай
AND6 үтәү тикшерелгән исемлек, дәлилдәр журналы, һәм идара итеү пакеттары шул уҡ бүлешә
детерминистик эш ағымы. Был яҙманы бронирование процедураһы менән парлаштырырға
(`device_lab_reservation.md`) һәм репетицияларҙы планлаштырғанда авариялы runbook.

## Маҡсаттар & Скоп

- **Детерминистик дәлилдәр** – бөтә приборҙар сығыштары ла 2012 йылда йәшәй.
  `artifacts/android/device_lab/<slot-id>/` SHA-256 менән шулай аудиторҙар манифестары
  зондтарҙы ҡабаттан эшләтмәйенсә, өйөмдәрҙе айыра ала.
- **Сценарий-беренсе эш ағымы** – ғәмәлдәге ярҙамсыларҙы ҡабаттан файҙаланыу .
  (`ci/run_android_telemetry_chaos_prep.sh`,
  `scripts/android_keystore_attestation.sh`, `scripts/android_override_tool.sh`)
  урынына заказ буйынса адб командалары.
- **Тикшереү исемлеге синхронлаштырыу** – һәр йүгерә был документҡа һылтанма яһай.
  AND6 үтәү тикшерелгән исемлек һәм артефакттарҙы ҡуша
  `docs/source/compliance/android/evidence_log.csv`.

## Артефакт макеты

1. Һайлап алыу үҙенсәлекле слот идентификаторы, тура килә бронирование билет, мәҫ.
   `2026-05-12-slot-a`.
2. Стандарт каталогтарын орлоҡ:

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. Һәр команда журналын тап килгән папка эсендә һаҡларға (мәҫ.
   `telemetry/status.ndjson`, `attestation/pixel8pro.log`).
4. SHA-256-ны төшөрөү слот ябыла бер тапҡыр күренә:

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## Приборҙар матрицаһы

| Ағым | Команда(тар) | Сығыш урыны | Иҫкәрмәләр |
|-----|------------|----------------|--------|
| Телеметрия редакцияһы + статус өйөмө | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | Йүгереп старт һәм аҙағында слот; беркетергә CLI stdout `status.log`. |
| Сират көтөп + хаос әҙерлек | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256` | `readiness/labs/telemetry_lab_01.md`-тан көҙгөстәр сценарийы; слоттағы һәр ҡоролма өсөн env var-ҙы оҙайтығыҙ. |
| Операцион леджер distest | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | Кәрәкле, хатта ҡасан бер ниндәй ҙә өҫтөндә әүҙем; нуль хәлен иҫбатлау. |
| Көслөбокс / ТЭЭ аттестацияһы | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | Һәр запаслы ҡоролма өсөн ҡабатлау (`android_strongbox_device_matrix.md`-тағы матч исемдәре). |
| CI йүгән аттестация регрессия | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | CI тейәүгә шул уҡ дәлилдәрҙе тота; симметрия өсөн ҡул менән эшләүгә инә. |
| Линт / бәйлелек база һыҙығы | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | Бер тапҡыр туңдырыу тәҙрәһе йүгерергә; йөкмәткеһен үтәү пакеттарында цитироваться. |

## Стандарт слот процедураһы1. **Пре-осоу (Т-24h)** – Раҫлау өсөн бронирование билет һылтанмалар был .
   документ, ҡоролма матрицаһы инеүен яңыртыу һәм артефакт тамыры орлоҡ.
2. **Слот ваҡытында**
   - Телеметрия өйөмөн + сират экспорты командаларын тәүҙә йүгертегеҙ. Үтергә
     `--note <ticket>` `ci/run_android_telemetry_chaos_prep.sh` тиклем, шулай журнал
     һылтанмалар ваҡиға идентификаторы.
   - Аттестация скрипттарын бер ҡоролмаға триггер. Ҡасан йүгән етештерә а
     `device_lab_reservation.md`, уны артефакт тамырына күсереп, Git SHA-ны 1911 йылдың 10 ғинуарында баҫтырып сығара.
     сценарий аҙағы.
   - `make android-lint` башҡарылған, әгәр ҙә CI булһа ла, өҫтөнлөклө резюме юлы менән.
     инде йүгерҙе; аудиторҙар көтә бер слот журналы.
3. **Пост-йүгерергә**
   - `sha256sum.txt` генерациялау һәм `README.md` (бушлай формалы иҫкәрмәләр) эсендә слот
     папкаһы башҡарылған командаларҙы дөйөмләштерә.
   - `docs/source/compliance/android/evidence_log.csv` менән бер рәткә ҡушылығыҙ.
     слот идентификаторы, хеш асыҡ юл, Buildkite һылтанмалар (әгәр бар икән), һәм һуңғы
     ҡоролма-лаборатория һыйҙырышлылығы проценты бронирование календары экспорты.
   - `_android-device-lab` билетында слот папкаһын, AND6-ға бәйләйбеҙ.
     тикшерелгән исемлек, һәм `docs/source/android_support_playbook.md` релиз отчеты.

## Уңышһыҙлыҡ менән эш итеү & Эскалация

- Әгәр ҙә ниндәйҙер команда уңышһыҙлыҡҡа осраһа, `logs/` аҫтында стдерер сығышын тотоп,
  `device_lab_reservation.md` §6-ла эскалация баҫҡысы.
- Сират йәки телеметрия етешһеҙлектәре шунда уҡ 1990 йылда өҫтөнлөк статусын билдәләргә тейеш.
  `docs/source/sdk/android/telemetry_override_log.md` һәм слот ID һылтанма
  тимәк, идара итеү бурауҙы эҙләй ала.
- Аттестация регрессиялары 2012 йылда теркәлергә тейеш.
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  ҡоролма сериялары һәм өҫтә теркәлгән өйөм юлдары менән.

## Отчет тикшерелгән исемлек

Слот тулы билдәләгәнсе, түбәндәге һылтанмаларҙы яңыртылған раҫлау:

- `docs/source/compliance/android/and6_compliance_checklist.md` — билдәһе.
  приборҙар рәт тулы һәм слот идентификаторын иҫәпкә алырға.
- `docs/source/compliance/android/evidence_log.csv` — өҫтәү/яңыртыу яҙма менән
  слот хеш һәм ҡәҙерле уҡыу.
- `_android-device-lab` билет — артефакт һылтанмалары һәм Buildkite эш идентификаторҙары беркетергә.
- `status.md` — ҡыҫҡаса иҫкәрмә индереү өсөн киләһе Android әҙерлек distest шулай
  юл картаһы уҡыусылар белә, ниндәй слот етештереү һуңғы дәлилдәр.

Был процесты үтәү AND6’s “ҡоролма-лаборатория + приборҙар ҡармаҡтары” һаҡлай.
1990 йылдарҙа был йүнәлештәге эштәрҙең иң мөһимдәренең береһе
һәм отчет биреүҙе.