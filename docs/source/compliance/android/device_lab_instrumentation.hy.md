---
lang: hy
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

# Android սարքի լաբորատոր սարքավորման կեռիկներ (AND6)

Այս տեղեկանքը փակում է ճանապարհային քարտեզի գործողությունը «փուլ մնացած սարք-լաբորատորիա /
գործիքավորումը AND6 մեկնարկից առաջ»: Այն բացատրում է, թե ինչպես է ամեն վերապահված
սարք-լաբորատորի բնիկը պետք է ֆիքսի հեռաչափությունը, հերթը և ատեստավորման արտեֆակտները, որպեսզի
AND6 համապատասխանության ստուգաթերթը, ապացույցների գրանցամատյանը և կառավարման փաթեթները նույնն են
դետերմինիստական աշխատանքային հոսք: Զուգակցեք այս նշումը ամրագրման ընթացակարգի հետ
(`device_lab_reservation.md`) և ձախողման գրքույկը փորձերը պլանավորելիս:

## Նպատակներ և շրջանակներ

- **Դետերմինիստական ապացույց** – գործիքավորման բոլոր ելքերը գործում են
  `artifacts/android/device_lab/<slot-id>/`-ը SHA-256-ով ցույց է տալիս, որ աուդիտորները
  կարող է տարբերել փաթեթները՝ առանց զոնդերը նորից գործարկելու:
- **Սցենար-առաջին աշխատանքային հոսք** – վերօգտագործեք առկա օգնականները
  (`ci/run_android_telemetry_chaos_prep.sh`,
  `scripts/android_keystore_attestation.sh`, `scripts/android_override_tool.sh`)
  պատվերով adb հրամանների փոխարեն:
- **Ստուգացուցակները մնում են համաժամանակյա** – յուրաքանչյուր գործարկում հղում է անում այս փաստաթղթին
  AND6 համապատասխանության ստուգաթերթը և կցում է արտեֆակտները
  `docs/source/compliance/android/evidence_log.csv`.

## Արտեֆակտի դասավորություն

1. Ընտրեք եզակի բնիկի նույնացուցիչ, որը համապատասխանում է ամրագրման տոմսին, օրինակ.
   `2026-05-12-slot-a`.
2. Սերմնավորեք ստանդարտ գրացուցակները.

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. Պահպանեք յուրաքանչյուր հրամանի մատյան համապատասխան թղթապանակում (օրինակ.
   `telemetry/status.ndjson`, `attestation/pixel8pro.log`):
4. Capture SHA-256-ը դրսևորվում է բնիկի փակվելուց հետո.

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## Գործիքավորման մատրիցա

| Հոսք | Հրաման(ներ) | Ելքի գտնվելու վայրը | Ծանոթագրություններ |
|------|------------|----------------|-------|
| Հեռաչափության խմբագրում + կարգավիճակի փաթեթ | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | Վազեք բնիկի սկզբում և վերջում; կցեք CLI stdout-ը `status.log`-ին: |
| Սպասվող հերթ + քաոսի նախապատրաստում | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256` | Հայելիներ ScenarioD `readiness/labs/telemetry_lab_01.md`-ից; երկարացնել env var-ը բնիկում գտնվող յուրաքանչյուր սարքի համար: |
| Չեղյալ համարել մատյանների ամփոփում | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | Պահանջվում է նույնիսկ այն դեպքում, երբ ոչ մի անտեսում ակտիվ չէ. ապացուցել զրոյական վիճակը. |
| StrongBox / TEE ատեստավորում | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | Կրկնեք յուրաքանչյուր վերապահված սարքի համար (համապատասխանեք անունները `android_strongbox_device_matrix.md`-ում): |
| CI զրահի ատեստավորման ռեգրեսիա | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | Գրավում է նույն ապացույցը, որը CI-ն վերբեռնում է. ներառել սիմետրիայի համար ձեռքով վազքի մեջ: |
| Լինտ / կախվածության բազային | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | Գործարկել մեկ անգամ սառեցման պատուհանում; մեջբերել ամփոփագիրը համապատասխանության փաթեթներում: |

## Ստանդարտ անցք ընթացակարգ1. **Նախաթռիչք (T-24h)** – Հաստատեք ամրագրման տոմսի հղումը սա
   փաստաթուղթ, թարմացրեք սարքի մատրիցայի մուտքը և սերմացրեք արտեֆակտի արմատը:
2. **Սլոտի ընթացքում**
   - Նախ գործարկեք հեռաչափության փաթեթը + հերթերի արտահանման հրամանները: Անցում
     `--note <ticket>`-ից մինչև `ci/run_android_telemetry_chaos_prep.sh`, ուստի գրանցամատյանը
     վկայակոչում է միջադեպի ID-ն:
   - Գործարկեք ատեստավորման սցենարները յուրաքանչյուր սարքի համար: Երբ զրահը առաջացնում է ա
     `.zip`, պատճենեք այն արտեֆակտ արմատի մեջ և գրանցեք Git SHA-ն, որը տպագրվել է այստեղ
     սցենարի վերջը.
   - Կատարեք `make android-lint`-ը վերացված ամփոփիչ ճանապարհով, նույնիսկ եթե CI
     արդեն վազել; աուդիտորները ակնկալում են յուրաքանչյուր բնիկի գրանցամատյան:
3. **Հետ վազք**
   - Ստեղծեք `sha256sum.txt` և `README.md` (անվճար նշումներ) բնիկի ներսում
     թղթապանակ, որն ամփոփում է կատարված հրամանները:
   - Կցեք տող `docs/source/compliance/android/evidence_log.csv`-ի հետ
     բնիկի ID, հեշ մանիֆեստի ուղի, Buildkite հղումներ (եթե այդպիսիք կան) և վերջինը
     սարք-լաբորատորիայի հզորության տոկոսը ամրագրումների օրացույցի արտահանումից:
   - Կապեք `_android-device-lab` տոմսի բնիկի թղթապանակը, AND6-ը
     ստուգաթերթ և `docs/source/android_support_playbook.md` թողարկման հաշվետվություն:

## Խափանումների կառավարում և էսկալացիա

- Եթե որևէ հրաման ձախողվի, վերցրեք stderr ելքը `logs/`-ի տակ և հետևեք
  էսկալացիայի սանդուղք `device_lab_reservation.md`-ում §6.
- Հերթի կամ հեռաչափության թերությունները պետք է անմիջապես նշեն անտեսման կարգավիճակը
  `docs/source/sdk/android/telemetry_override_log.md` և մատնանշեք բնիկի ID-ն
  այնպես որ կառավարումը կարող է հետևել փորվածությանը:
- Ատեստավորման հետընթացները պետք է գրանցվեն
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  ձախողված սարքի սերիաների և վերևում գրանցված փաթեթների ուղիների հետ:

## Հաշվետվությունների ստուգաթերթ

Նախքան անցքը ավարտված նշելը, ստուգեք, որ հետևյալ հղումները թարմացված են.

- `docs/source/compliance/android/and6_compliance_checklist.md` — նշեք
  գործիքավորման շարքը լրացրեք և նշեք բնիկի ID-ն:
- `docs/source/compliance/android/evidence_log.csv` — ավելացնել/թարմացնել գրառումը
  բնիկի հեշը և հզորության ընթերցումը:
- `_android-device-lab` տոմս — կցեք արտեֆակտ հղումներ և Buildkite աշխատանքի ID-ներ:
- `status.md` — ներառեք հակիրճ նշում Android-ի հաջորդ պատրաստության ամփոփագրում, որպեսզի
  Ճանապարհային քարտեզ ընթերցողները գիտեն, թե որ հատվածն է ստեղծել վերջին ապացույցները:

Այս գործընթացից հետո պահպանվում են AND6-ի «սարք-լաբորատորիա + գործիքավորման կեռիկներ»
նշաձողը ստուգելի է և կանխում է ձեռքով տարաձայնությունները ամրագրման, կատարման միջև,
և հաշվետվություն: