---
lang: uz
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

# Android Device Lab Instrumentation Hooks (AND6)

Ushbu ma'lumot yo'l xaritasi harakatini yopadi "qolgan qurilma-laboratoriyani bosqichga o'tkazish /
asboblar AND6 boshlanishidan oldin ilgaklar”. Bu qanday qilib har bir zahiralanganligini tushuntiradi
qurilma-laboratoriya uyasi telemetriya, navbat va attestatsiya artefaktlarini yozib olishi kerak
AND6 muvofiqligini tekshirish roʻyxati, dalillar jurnali va boshqaruv paketlari bir xil boʻladi
deterministik ish jarayoni. Ushbu eslatmani bron qilish tartibi bilan bog'lang
(`device_lab_reservation.md`) va takroriy takroriy ishlanmalar kitobi mashqlarni rejalashtirishda.

## Maqsadlar va qamrov

- **Deterministik dalillar** - barcha asboblar chiqishlari ostida ishlaydi
  SHA-256 bilan `artifacts/android/device_lab/<slot-id>/` auditorlar shunday namoyon bo'ladi
  problarni qayta ishga tushirmasdan to'plamlarni farqlashi mumkin.
- **Skript-birinchi ish jarayoni** – mavjud yordamchilarni qayta ishlatish
  (`ci/run_android_telemetry_chaos_prep.sh`,
  `scripts/android_keystore_attestation.sh`, `scripts/android_override_tool.sh`)
  buyurtma adb buyruqlari o'rniga.
- **Tekshiruv roʻyxatlari sinxronlashtiriladi** – har bir ishga tushirish ushbu hujjatga havola qiladi
  AND6 muvofiqligini tekshirish roʻyxati va artefaktlarni unga qoʻshadi
  `docs/source/compliance/android/evidence_log.csv`.

## Artifakt tartibi

1. Bandlov chiptasiga mos keladigan noyob slot identifikatorini tanlang, masalan.
   `2026-05-12-slot-a`.
2. Standart kataloglarni ekish:

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. Har bir buyruq jurnalini mos keladigan papkaga saqlang (masalan,
   `telemetry/status.ndjson`, `attestation/pixel8pro.log`).
4. SHA-256 ni suratga olish uyasi yopilgandan keyin namoyon bo'ladi:

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## Asboblar matritsasi

| Oqim | Buyruq(lar) | Chiqish joyi | Eslatmalar |
|------|------------|-----------------|-------|
| Telemetriyani tahrirlash + holat to'plami | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | Slotning boshida va oxirida ishga tushirish; CLI stdout-ni `status.log` ga biriktiring. |
| Kutilayotgan navbat + tartibsizlikka tayyorgarlik | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256` | `readiness/labs/telemetry_lab_01.md` dan Mirrors ScenarioD; Slotdagi har bir qurilma uchun env varni kengaytiring. |
| Buxgalteriya jurnali dayjestini bekor qilish | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | Hech qanday bekor qilish faol bo'lmaganda ham talab qilinadi; nol holatini isbotlang. |
| StrongBox / TEE attestatsiyasi | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | Har bir zahiraga olingan qurilma uchun takrorlang (`android_strongbox_device_matrix.md` da nomlar mos keladi). |
| CI jabduqlar attestatsiyasi regressiyasi | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | CI yuklagan bir xil dalillarni oladi; simmetriya uchun qo'lda ishlashga qo'shing. |
| Lint / qaramlik asosi | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | Har bir muzlatish oynasida bir marta ishga tushirish; muvofiqlik paketlarida xulosani keltiring. |

## Slotning standart protsedurasi1. **Parvozdan oldin (T-24h)** – Chiptani bron qilishda buni tasdiqlang
   hujjat, qurilma matritsasi yozuvini yangilang va artefakt ildizini eking.
2. **Slot davomida**
   - Avval telemetriya to'plamini + navbatni eksport qilish buyruqlarini ishga tushiring. O'tish
     `--note <ticket>` dan `ci/run_android_telemetry_chaos_prep.sh` gacha, shuning uchun jurnal
     voqea identifikatoriga havola qiladi.
   - Har bir qurilma uchun attestatsiya skriptlarini ishga tushiring. Jabduqlar a hosil qilganda
     `.zip`, uni artefakt ildiziga nusxalang va chop etilgan Git SHA-ni yozib oling.
     skriptning oxiri.
   - `make android-lint` ni CI bo'lsa ham bekor qilingan xulosa yo'li bilan bajaring
     allaqachon yugurgan; auditorlar boshiga uyasi jurnali kutish.
3. **Post-yugurish**
   - Slot ichida `sha256sum.txt` va `README.md` (erkin shakldagi eslatmalar) yarating
     bajarilgan buyruqlarni jamlagan papka.
   - `docs/source/compliance/android/evidence_log.csv` qatoriga qator qo'shing
     slot identifikatori, xash-manifest yo'li, Buildkite havolalari (agar mavjud bo'lsa) va eng so'nggi
     bron taqvimi eksportidan qurilma-laboratoriya sig'imi foizi.
   - `_android-device-lab` chiptasidagi slot papkasini AND6 bilan bog'lang
     nazorat roʻyxati va `docs/source/android_support_playbook.md` reliz hisoboti.

## Muvaffaqiyatsizlikni bartaraf etish va eskalatsiya

- Agar biron bir buyruq bajarilmasa, `logs/` ostidagi stderr chiqishini oling va quyidagi amallarni bajaring.
  `device_lab_reservation.md` §6 da eskalatsiya narvonlari.
- Navbat yoki telemetriyadagi kamchiliklar darhol bekor qilish holatini qayd etishi kerak
  `docs/source/sdk/android/telemetry_override_log.md` va slot identifikatoriga murojaat qiling
  shuning uchun boshqaruv matkapni kuzatishi mumkin.
- Attestatsiya regresslari qayd etilishi kerak
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  ishlamay qolgan qurilma seriyalari va yuqorida qayd etilgan to'plam yo'llari bilan.

## Hisobotlarni tekshirish ro'yxati

Slot tugallanganligini belgilashdan oldin quyidagi havolalar yangilanganligini tekshiring:

- `docs/source/compliance/android/and6_compliance_checklist.md` - belgilang
  asboblar qatorini to'ldiring va slot identifikatoriga e'tibor bering.
- `docs/source/compliance/android/evidence_log.csv` - yozuvni qo'shish/yangilash
  uyasi xesh va sig'im o'qish.
- `_android-device-lab` chiptasi - artefakt havolalari va Buildkite ish identifikatorlarini biriktiring.
- `status.md` — Android tayyorligi haqidagi navbatdagi dayjestga qisqacha eslatma kiriting
  yo'l xaritasi o'quvchilari qaysi uyasi so'nggi dalillarni ishlab chiqarilganini bilishadi.

Ushbu jarayondan so'ng AND6 ning "qurilma-laboratoriya + asboblar ilgaklari" saqlanadi.
muhim bosqich tekshirilishi mumkin va bron qilish, bajarish,
va hisobot berish.