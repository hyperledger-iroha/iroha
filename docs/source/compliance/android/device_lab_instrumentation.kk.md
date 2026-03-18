---
lang: kk
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

# Android құрылғысының зертханалық аспаптық ілмектері (AND6)

Бұл анықтама жол картасы әрекетін жабады "қалған құрылғы-зертхананы кезеңге шығару /
аспаптар AND6 басталуынан бұрын ілгектер». Бұл әрбір резервтелгенін түсіндіреді
құрылғы-зертхана ұясы телеметрияны, кезекті және аттестаттау артефактілерін түсіруі керек
AND6 сәйкестікті тексеру тізімі, дәлелдер журналы және басқару пакеттері бірдей ортақ
детерминирленген жұмыс процесі. Бұл жазбаны брондау процедурасымен жұптаңыз
(`device_lab_reservation.md`) және репетицияларды жоспарлау кезінде орындалмайтын жұмыс кітабы.

## Мақсаттар мен ауқым

- **Детерминистік дәлел** – барлық аспап шығыстары астында жұмыс істейді
  `artifacts/android/device_lab/<slot-id>/` SHA-256 манифесттері осылайша аудиторлар
  зондтарды қайта іске қоспай-ақ дестелерді ажырата алады.
- **Сценарийдің бірінші жұмыс процесі** – бар көмекшілерді қайта пайдаланыңыз
  (`ci/run_android_telemetry_chaos_prep.sh`,
  `scripts/android_keystore_attestation.sh`, `scripts/android_override_tool.sh`)
  тапсырыс adb пәрмендерінің орнына.
- **Тексеру тізімдері синхрондалады** – әрбір іске қосу осы құжатқа сілтеме жасайды
  AND6 сәйкестігін тексеру тізімі және артефактілерді қосады
  `docs/source/compliance/android/evidence_log.csv`.

## Артефакт орналасуы

1. Брондау билетіне сәйкес келетін бірегей ұяшық идентификаторын таңдаңыз, мысалы:
   `2026-05-12-slot-a`.
2. Стандартты каталогтарды көшіру:

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. Әрбір пәрмен журналын сәйкес қалтаға сақтаңыз (мысалы,
   `telemetry/status.ndjson`, `attestation/pixel8pro.log`).
4. Слот жабылғаннан кейін SHA-256 суретін түсіру:

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## Құралдар матрицасы

| Ағын | Команда(лар) | Шығару орны | Ескертпелер |
|------|------------|-----------------|-------|
| Телеметриялық түзету + күй жинағы | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | Слоттың басында және соңында іске қосыңыз; CLI stdout файлын `status.log` файлына тіркеңіз. |
| Күтудегі кезек + хаосқа дайындық | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256` | Айналар сценарийіD `readiness/labs/telemetry_lab_01.md`; ұядағы әрбір құрылғы үшін env var кеңейтіңіз. |
| Бухгалтерлік журнал дайджестін қайта анықтау | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | Ешбір қайта анықтау белсенді болмаған кезде де қажет; нөлдік күйді дәлелдеңіз. |
| StrongBox / TEE аттестаттау | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | Әрбір сақталған құрылғы үшін қайталаңыз (`android_strongbox_device_matrix.md` ішіндегі атаулар сәйкес келеді). |
| CI аттестаттау регрессиясы | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | CI жүктеп салатын бірдей дәлелдерді түсіреді; симметрия үшін қолмен жүгірулерге қосыңыз. |
| Линт / тәуелділік базасы | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | Әр мұздату терезесіне бір рет іске қосыңыз; сәйкестік пакеттерінде түйіндемені келтіріңіз. |

## Стандартты ұяшық процедурасы1. **Ұшу алдындағы (T-24сағ)** – Брондау билетінің осыған сілтеме жасайтынын растаңыз
   құжат, құрылғы матрицасы жазбасын жаңартыңыз және артефакт түбірін септеңіз.
2. **Слот кезінде**
   - Алдымен телеметрия жинағы + кезек экспорттау пәрмендерін іске қосыңыз. Өту
     `--note <ticket>` - `ci/run_android_telemetry_chaos_prep.sh`, сондықтан журнал
     оқиға идентификаторына сілтеме жасайды.
   - Әр құрылғыда аттестаттау сценарийлерін іске қосыңыз. Әбзелдер а шығарғанда
     `.zip`, оны артефакт түбіріне көшіріңіз және басып шығарылған Git SHA жазыңыз.
     сценарийдің соңы.
   - `make android-lint` параметрін CI болса да қайта анықталған жиынтық жолымен орындаңыз
     қазірдің өзінде жүгірді; аудиторлар әрбір ұяшық журналын күтеді.
3. **Орындаудан кейінгі**
   - Слот ішінде `sha256sum.txt` және `README.md` (еркін пішінді ескертпелер) жасаңыз
     орындалған командаларды қорытындылайтын қалта.
   - арқылы `docs/source/compliance/android/evidence_log.csv` жолын қосыңыз
     ұяшық идентификаторы, хэш манифест жолы, Buildkite сілтемелері (бар болса) және соңғысы
     брондау күнтізбесінің экспортынан алынған құрылғы-зертхана сыйымдылығының пайызы.
   - `_android-device-lab` билетіндегі AND6 ұяшығы қалтасын байланыстырыңыз
     бақылау тізімі және `docs/source/android_support_playbook.md` шығарылым есебі.

## Сәтсіздіктерді өңдеу және арттыру

- Кез келген пәрмен орындалмаса, `logs/` астында stderr шығысын жазып алыңыз және
  `device_lab_reservation.md` §6 ішіндегі эскалация сатысы.
- Кезекте немесе телеметриядағы кемшіліктер бірден қайта анықтау күйін ескеруі керек
  `docs/source/sdk/android/telemetry_override_log.md` және ұяшық идентификаторына сілтеме жасаңыз
  сондықтан басқару жаттығуларды қадағалай алады.
- Аттестаттау регрессиялары тіркелуі керек
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  ақаулы құрылғы серияларымен және жоғарыда жазылған бума жолдарымен.

## Есепті тексеру тізімі

Слотты аяқталды деп белгілемес бұрын, келесі сілтемелердің жаңартылғанын тексеріңіз:

- `docs/source/compliance/android/and6_compliance_checklist.md` — белгілеңіз
  аспаптар жолын аяқтаңыз және ұяшық идентификаторына назар аударыңыз.
- `docs/source/compliance/android/evidence_log.csv` — жазбаны қосу/жаңарту
  ұяшық хэші мен сыйымдылықты оқу.
- `_android-device-lab` билеті — артефакт сілтемелерін және Buildkite жұмыс идентификаторларын тіркеңіз.
- `status.md` — Android дайындығы туралы келесі дайджестке қысқаша ескертуді қосыңыз, сондықтан
  жол картасының оқырмандары қай ұяшық соңғы дәлелдерді шығарғанын біледі.

Осы процестен кейін AND6 «құрылғы-зертхана + аспаптық ілмектері» сақталады
тексеруге болатын кезең және брондау, орындау,
және есеп беру.