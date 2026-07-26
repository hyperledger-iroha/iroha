---
lang: mn
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

# Android төхөөрөмжийн лабораторийн багаж хэрэгслийн дэгээ (AND6)

Энэхүү лавлагаа нь замын зураглалын үйлдлийг хаадаг "үлдсэн төхөөрөмжийн лабораторийг үе шаттайгаар гаргах /
багаж хэрэгслийн дэгээ AND6 эхлэхээс өмнө". Энэ нь бүр хэрхэн нөөцлөгдсөнийг тайлбарладаг
төхөөрөмжийн лабораторийн үүр нь телеметр, дараалал, баталгаажуулалтын олдворуудыг авах ёстой
AND6 нийцлийн шалгах хуудас, нотлох баримтын бүртгэл, засаглалын багцууд ижил төстэй
тодорхойлогч ажлын урсгал. Энэ тэмдэглэлийг захиалгын журамтай хослуул
(`device_lab_reservation.md`) болон бэлтгэл сургуулилтыг төлөвлөхдөө дампуурлын дэвтэр.

## Зорилго ба хамрах хүрээ

- **Тодорхойлолт нотолгоо** – бүх багаж хэрэгслийн гаралтууд дор ажиллаж байна
  SHA-256-тай `artifacts/android/device_lab/<slot-id>/` нь аудиторууд илэрдэг
  датчикуудыг дахин ажиллуулахгүйгээр багцуудыг ялгаж чадна.
- **Скриптийн эхний ажлын урсгал** – одоо байгаа туслахуудыг дахин ашиглах
  (`ci/run_android_telemetry_chaos_prep.sh`,
  `scripts/android_keystore_attestation.sh`, `scripts/android_override_tool.sh`)
  захиалгаар adb командын оронд.
- **Хяналтын хуудсууд нь синхрончлолд үлддэг** – гүйлт бүр нь энэ баримтаас иш татдаг
  AND6-д нийцсэн эсэхийг шалгах хуудас болон олдворуудыг хавсаргана
  `docs/source/compliance/android/evidence_log.csv`.

## Олдворын зохион байгуулалт

1. Захиалгын тасалбартай тохирох өвөрмөц слот танигчийг сонгоно уу, жишээлбэл.
   `2026-05-12-slot-a`.
2. Стандарт лавлахуудыг суулгана уу:

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. Командын бүртгэл бүрийг тохирох хавтас дотор хадгал (жишээ нь.
   `telemetry/status.ndjson`, `attestation/pixel8pro.log`).
4. Слот хаагдах үед SHA-256-г авах:

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## Багажны матриц

| Урсгал | Тушаал(ууд) | Гаралтын байршил | Тэмдэглэл |
|------|------------|-----------------|-------|
| Телеметрийн засвар + төлөвийн багц | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | Слотын эхлэл ба төгсгөлд гүйх; CLI stdout-г `status.log`-д хавсаргана уу. |
| Хүлээгдэж буй дараалал + эмх замбараагүй байдлын бэлтгэл | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256` | `readiness/labs/telemetry_lab_01.md`-аас толин тусгалуудын хувилбарD; үүрэнд байгаа төхөөрөмж бүрийн env var-ыг сунгана. |
| Бүртгэлийн тоймыг хүчингүй болгох | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | Ямар ч хүчингүй болгох идэвхгүй үед ч шаардлагатай; тэг төлөвийг батлах. |
| StrongBox / TEE attestation | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | Захиалагдсан төхөөрөмж тус бүрийг давтана уу (`android_strongbox_device_matrix.md` дээрх нэрсийг тааруулна уу). |
| CI бэхэлгээний баталгаажуулалтын регресс | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | CI-ийн байршуулсан ижил нотолгоог олж авдаг; тэгш хэмийн хувьд гарын авлагын гүйлтэд оруулах. |
| Хөвөн / хараат байдлын суурь | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | Хөлдөөх цонх бүрт нэг удаа ажиллуулах; нийцлийн багцад хураангуйг иш татна. |

## Слотын стандарт журам1. **Нислэгийн өмнөх (T-24цаг)** – Тасалбарын захиалгын мэдээлэлд энэ тухай дурдсаныг баталгаажуулна уу.
   баримт бичиг, төхөөрөмжийн матрицын оруулгыг шинэчилж, олдворын үндэсийг суулгана.
2. **Слотын үеэр**
   - Эхлээд телеметрийн багц + дарааллын экспортын командуудыг ажиллуул. Дамжуулах
     `--note <ticket>`-ээс `ci/run_android_telemetry_chaos_prep.sh` хүртэл бүртгэл
     ослын ID-г иш татдаг.
   - Төхөөрөмж бүрийн баталгаажуулалтын скриптийг идэвхжүүлэх. морины оосор гаргах үед а
     `.zip`, үүнийг олдворын үндэс рүү хуулж, хэвлэсэн Git SHA-г бичнэ үү.
     скриптийн төгсгөл.
   - CI байсан ч дарагдсан хураангуй замаар `make android-lint`-г ажиллуулна.
     аль хэдийн гүйсэн; аудиторууд оролт бүрт бүртгэл хүлээж байна.
3. **Гүйлтийн дараах**
   - Слот дотор `sha256sum.txt` болон `README.md` (чөлөөт хэлбэрийн тэмдэглэл) үүсгэх
     гүйцэтгэсэн тушаалуудыг нэгтгэн харуулсан хавтас.
   - `docs/source/compliance/android/evidence_log.csv`-д мөр хавсаргана
     үүр ID, хэш манифест зам, Buildkite лавлагаа (хэрэв байгаа бол) болон хамгийн сүүлийн үеийн
     захиалгын календарийн экспортоос төхөөрөмжийн лабораторийн багтаамжийн хувь.
   - `_android-device-lab` тасалбар, AND6 дахь үүрний хавтсыг холбоно уу
     шалгах хуудас, `docs/source/android_support_playbook.md` хувилбарын тайлан.

## Бүтэлгүйтэлтэй харьцах & Даамжруулах

- Хэрэв ямар нэгэн тушаал амжилтгүй болвол `logs/` доор байгаа stderr гаралтыг аваад дараахыг дагана уу.
  `device_lab_reservation.md` §6 дахь өргөлтийн шат.
- Дараалал эсвэл телеметрийн дутагдал нь хүчингүй болсон байдлыг нэн даруй тэмдэглэх ёстой
  `docs/source/sdk/android/telemetry_override_log.md` ба үүрний ID-г лавлана уу
  тиймээс засаглал сургуулилтыг мөшгиж чадна.
- Гэрчилгээний регрессийг бүртгэх ёстой
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  бүтэлгүйтсэн төхөөрөмжийн цуваа болон дээр бичигдсэн багцын замуудтай.

## Тайлагнах хяналтын хуудас

Слотыг дуусгахын өмнө дараах лавлагаа шинэчлэгдсэн эсэхийг шалгана уу.

- `docs/source/compliance/android/and6_compliance_checklist.md` — тэмдэглэнэ үү
  багаж хэрэгслийн мөрийг бөглөж, үүрний ID-г тэмдэглэнэ үү.
- `docs/source/compliance/android/evidence_log.csv` — оруулгыг нэмэх/шинэчлэх
  үүрний хэш болон багтаамжийн уншилт.
- `_android-device-lab` тасалбар — олдворын холбоос болон Buildkite ажлын ID-г хавсаргана уу.
- `status.md` — дараагийн Android бэлэн байдлын тоймд товч тэмдэглэл оруулах
  Замын зураг уншигчид аль үүр нь хамгийн сүүлийн үеийн нотолгоог гаргасан болохыг мэддэг.

Энэ процессын дараа AND6-ийн "төхөөрөмжийн лаборатори + багаж хэрэгслийн дэгээ" хадгалагдана.
аудит хийх боломжтой бөгөөд захиалга хийх, гүйцэтгэх,
болон тайлагнах.