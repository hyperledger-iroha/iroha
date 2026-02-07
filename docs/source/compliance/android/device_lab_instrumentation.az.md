---
lang: az
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

# Android Cihaz Laboratoriyası Alətləri Qarmaqları (AND6)

Bu arayış “qalan cihaz-laboratoriyasını mərhələliləşdirin /” yol xəritəsi hərəkətini bağlayır.
alətlər AND6 başlamasından əvvəl qarmaqlar. Bu, hər bir qorunur necə izah edir
cihaz laboratoriya yuvası telemetriya, növbə və attestasiya artefaktlarını tutmalıdır
AND6 uyğunluq yoxlama siyahısı, sübut jurnalı və idarəetmə paketləri eyni şeyi paylaşır
deterministik iş axını. Bu qeydi rezervasiya proseduru ilə birləşdirin
(`device_lab_reservation.md`) və məşqləri planlaşdırarkən uğursuzluq runbook.

## Məqsədlər və əhatə dairəsi

- **Deterministik sübut** – bütün cihaz çıxışları altında işləyir
  SHA-256 ilə `artifacts/android/device_lab/<slot-id>/` auditorlar belə göstərir
  zondları yenidən işə salmadan paketləri fərqləndirə bilər.
- **Skript-ilk iş axını** – mövcud köməkçiləri təkrar istifadə edin
  (`ci/run_android_telemetry_chaos_prep.sh`,
  `scripts/android_keystore_attestation.sh`, `scripts/android_override_tool.sh`)
  sifarişli adb əmrləri yerinə.
- **Yoxlama siyahıları sinxron qalır** – hər buraxılış bu sənədə istinad edir
  AND6 uyğunluq yoxlama siyahısı və artefaktları əlavə edir
  `docs/source/compliance/android/evidence_log.csv`.

## Artefakt Layout

1. Rezervasyon biletinə uyğun gələn unikal slot identifikatorunu seçin, məs.
   `2026-05-12-slot-a`.
2. Standart qovluqları səpin:

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. Hər bir əmr jurnalını uyğun qovluqda saxlayın (məs.
   `telemetry/status.ndjson`, `attestation/pixel8pro.log`).
4. Yuva bağlandıqdan sonra SHA-256 təzahürlərini çəkin:

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## Alətlər Matrisi

| Axın | Əmr(lər) | Çıxış yeri | Qeydlər |
|------|------------|-----------------|-------|
| Telemetriya redaktəsi + status paketi | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | Yuvanın əvvəlində və sonunda qaçın; CLI stdout-u `status.log`-ə əlavə edin. |
| Gözləyən növbə + xaos hazırlığı | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256` | `readiness/labs/telemetry_lab_01.md`-dən Güzgülər SsenariD; yuvadakı hər bir cihaz üçün env varını genişləndirin. |
| Defter həzmini ləğv edin | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | Heç bir ləğvetmə aktiv olmadıqda belə tələb olunur; sıfır vəziyyətini sübut edin. |
| StrongBox / TEE attestasiyası | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | Hər rezervasiya edilmiş cihaz üçün təkrarlayın (adları `android_strongbox_device_matrix.md`-də uyğunlaşdırın). |
| CI qoşqu attestasiya reqresiyası | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | CI-nin yüklədiyi eyni sübutları tutur; simmetriya üçün əl işləri daxil edin. |
| Lint / asılılıq bazası | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | Hər dondurma pəncərəsində bir dəfə işləyin; uyğunluq paketlərində xülasəyə istinad edin. |

## Standart Slot Proseduru1. **Uçuşdan əvvəl (T-24h)** – Rezervasiya biletinin buna istinad etdiyini təsdiqləyin
   sənədləşdirin, cihaz matris girişini yeniləyin və artefakt kökünü toxumlayın.
2. **Slot zamanı**
   - Əvvəlcə telemetriya paketini + növbə ixrac əmrlərini işə salın. keçir
     `--note <ticket>` - `ci/run_android_telemetry_chaos_prep.sh`
     hadisə identifikatoruna istinad edir.
   - Hər cihaz üçün attestasiya skriptlərini işə salın. Qoşqu istehsal etdikdə a
     `.zip`, onu artefakt kökünə köçürün və çap edilmiş Git SHA-nı qeyd edin.
     skriptin sonu.
   - `make android-lint` CI olsa belə, ləğv edilmiş xülasə yolu ilə icra edin
     artıq qaçdı; auditorlar hər yuva üçün jurnal gözləyirlər.
3. **İşdən sonra**
   - Yuva daxilində `sha256sum.txt` və `README.md` (sərbəst forma qeydləri) yaradın
     icra edilən əmrləri ümumiləşdirən qovluq.
   - ilə `docs/source/compliance/android/evidence_log.csv`-ə bir sıra əlavə edin
     slot ID, hash manifest yolu, Buildkite istinadları (əgər varsa) və ən son
     rezervasiya təqviminin ixracından cihaz-laboratoriya tutumunun faizi.
   - `_android-device-lab` biletində, AND6-da slot qovluğunu əlaqələndirin
     yoxlama siyahısı və `docs/source/android_support_playbook.md` buraxılış hesabatı.

## Uğursuzluğun İdarə Edilməsi və Eskalasiyası

- Hər hansı bir əmr uğursuz olarsa, `logs/` altında stderr çıxışını çəkin və aşağıdakılara əməl edin.
  `device_lab_reservation.md` §6-da eskalasiya nərdivanı.
- Növbə və ya telemetriya çatışmazlıqları dərhal ləğvetmə statusunu qeyd etməlidir
  `docs/source/sdk/android/telemetry_override_log.md` və slot ID-yə istinad edin
  beləliklə, idarəetmə qazmağı izləyə bilər.
- Attestasiya reqressləri qeyd edilməlidir
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  uğursuz cihaz serialları və yuxarıda qeyd olunan paket yolları ilə.

## Hesabat Yoxlama Siyahısı

Yuvanın tamamlandığını qeyd etməzdən əvvəl aşağıdakı istinadların yeniləndiyini yoxlayın:

- `docs/source/compliance/android/and6_compliance_checklist.md` — işarələyin
  alətlər sırasını tamamlayın və yuva ID-ni qeyd edin.
- `docs/source/compliance/android/evidence_log.csv` — girişi əlavə edin/yeniləyin
  yuva hash və tutum oxu.
- `_android-device-lab` bileti — artefakt bağlantılarını və Buildkite iş ID-lərini əlavə edin.
- `status.md` — növbəti Android hazırlığı həzminə qısa qeyd daxil edin
  yol xəritəsini oxuyanlar bilirlər ki, hansı slot ən son sübutları yaradıb.

Bu prosesdən sonra AND6-nın "cihaz-laboratoriya + cihaz qarmaqları" saxlanılır
yoxlanıla bilən mərhələdir və sifariş, icra,
və hesabat.