---
lang: mn
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
-->

# Android хувилбарыг шалгах хуудас (AND6)

Энэхүү хяналтын хуудас нь **AND6 — CI & Compliance Hardening** хаалгыг агуулна
`roadmap.md` (§Тэргүүлэл 5). Энэ нь Android SDK хувилбаруудыг Rust-тэй нийцүүлдэг
CI ажил, нийцлийн олдворуудыг зөв бичих замаар RFC хүлээлтийг гаргах,
GA-ийн өмнө хавсаргах ёстой төхөөрөмжийн лабораторийн нотолгоо, гарал үүслийн багцууд,
LTS буюу засварын галт тэрэг урагш хөдөлдөг.

Энэ баримт бичгийг дараахтай хамт ашиглаарай:

- `docs/source/android_support_playbook.md` — хувилбарын хуанли, SLA болон
  өргөлтийн мод.
- `docs/source/android_runbook.md` - өдөр тутмын үйл ажиллагааны runbooks.
- `docs/source/compliance/android/and6_compliance_checklist.md` — зохицуулагч
  олдворын тооллого.
- `docs/source/release_dual_track_runbook.md` — хоёр замтай хувилбарын засаглал.

## 1. Тайзны хаалгануудыг харвал

| Үе шат | Шаардлагатай хаалга | Нотлох баримт |
|-------|----------------|----------|
| **T−7 өдөр (урьдчилан хөлдөөх)** | 14 хоногийн турш шөнийн `ci/run_android_tests.sh` ногоон; `ci/check_android_fixtures.sh`, `ci/check_android_samples.sh`, `ci/check_android_docs_i18n.sh` дамжих; хөвөн/хамааралтай скан дараалалд байна. | Buildkite хяналтын самбар, бэхэлгээний ялгааны тайлан, жишээ дэлгэцийн агшин. |
| **T−3 өдөр (RC сурталчилгаа)** | Төхөөрөмжийн лабораторийн захиалгыг баталгаажуулсан; StrongBox attestation CI run (`scripts/android_strongbox_attestation_ci.sh`); Робоэлектрик/хэрэгсэлт иж бүрдэл нь хуваарьт тоног төхөөрөмж дээр хэрэгждэг; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` цэвэрхэн. | Төхөөрөмжийн матриц CSV, баталгаажуулалтын багц манифест, `artifacts/android/lint/<version>/` дор архивлагдсан Gradle тайлангууд. |
| **T−1 өдөр (явах/явахгүй)** | Телеметрийн редакцийн төлөвийн багцыг шинэчилсэн (`scripts/telemetry/check_redaction_status.py --write-cache`); `and6_compliance_checklist.md`-ийн дагуу шинэчлэгдсэн нийцлийн олдворууд; Гарал үүслийн бэлтгэл дууссан (`scripts/android_sbom_provenance.sh --dry-run`). | `docs/source/compliance/android/evidence_log.csv`, телеметрийн төлөв JSON, хуурай гүйлтийн бүртгэл. |
| **T0 (GA/LTS таслах)** | `scripts/publish_android_sdk.sh --dry-run` дууссан; гарал үүсэл + SBOM гарын үсэг зурсан; гаргах шалгах хуудсыг экспортлож, явах/хэрэггүй минутанд хавсаргасан; `ci/sdk_sorafs_orchestrator.sh` утааны ажил ногоон. | RFC хавсралт, Sigstore багц, `artifacts/android/`-ийн дагуу үрчлүүлэх олдворуудыг гарга. |
| **T+1 өдөр (таслалтын дараах)** | Залруулгын бэлэн байдлыг баталгаажуулсан (`scripts/publish_android_sdk.sh --validate-bundle`); хяналтын самбарын ялгааг хянаж үзсэн (`ci/check_android_dashboard_parity.sh`); нотлох баримтын багцыг `status.md`-д байршуулсан. | Хяналтын самбарын зөрүүг экспортлох, `status.md` оруулга руу холбох, архивласан хувилбарын багц. |

## 2. CI & Чанарын Хаалганы матриц| Хаалга | Тушаал(ууд) / Скрипт | Тэмдэглэл |
|------|--------------------|-------|
| Нэгж + интеграцийн тестүүд | `ci/run_android_tests.sh` (`ci/run_android_tests.sh` ороосон) | `artifacts/android/tests/test-summary.json` + туршилтын бүртгэлийг ялгаруулдаг. Norito кодлогч, дараалал, StrongBox нөөц, Torii үйлчлүүлэгчийн бэхэлгээний тестүүд багтана. Орой болон шошголохоос өмнө шаардлагатай. |
| Тоглолтын парите | `ci/check_android_fixtures.sh` (`scripts/check_android_fixtures.py` ороосон) | Шинэчлэгдсэн Norito бэхэлгээ нь Rust каноник багцтай тохирч байгааг баталгаажуулдаг; Хаалга эвдэрсэн үед JSON ялгааг хавсаргана уу. |
| Жишээ програмууд | `ci/check_android_samples.sh` | `examples/android/{operator-console,retail-wallet}`-г бүтээж, `scripts/android_sample_localization.py`-ээр дамжуулан орон нутгийн дэлгэцийн агшинг баталгаажуулдаг. |
| Docs/I18N | `ci/check_android_docs_i18n.sh` | Хамгаалагч README + орон нутгийн хурдан эхлүүлэх. Докны засварууд хувилбарын салбар дээр буусны дараа дахин ажиллуулна уу. |
| Хяналтын самбарын паритет | `ci/check_android_dashboard_parity.sh` | CI/экспортолсон хэмжигдэхүүнүүд Rust-ийн харьцуулагчтай нийцэж байгааг баталгаажуулна; T+1 баталгаажуулалтын үед шаардлагатай. |
| SDK батлах утаа | `ci/sdk_sorafs_orchestrator.sh` | Одоогийн SDK-тэй олон эх сурвалжийн Sorafs найруулагчийн холболтыг дасгалжуулна. Үе шаттай олдворуудыг байршуулахаас өмнө шаардлагатай. |
| Баталгаажуулалтын баталгаажуулалт | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | `artifacts/android/attestation/**` дор StrongBox/TEE баталгаажуулалтын багцуудыг нэгтгэдэг; хураангуйг GA пакетуудад хавсаргана уу. |
| Төхөөрөмжийн лабораторийн үүрний баталгаажуулалт | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | Пакетуудыг гаргахын тулд нотлох баримт хавсаргахаасаа өмнө багаж хэрэгслийн багцыг баталгаажуулдаг; CI нь `fixtures/android/device_lab/slot-sample` (телеметрийн/аттестатчилал/дараалал/логууд + `sha256sum.txt`) түүврийн үүрний эсрэг ажилладаг. |

> **Зөвлөгөө:** эдгээр ажлыг `android-release` Buildkite дамжуулах хоолойд нэмж, ингэснээр
> хөлдөөх долоо хоногуудыг гаралтын салааны үзүүрээр хаалга бүрийг автоматаар дахин ажиллуулна.

Нэгдсэн `.github/workflows/android-and6.yml` ажил нь хөвөнг ажиллуулдаг.
PR/түлхэлт бүрт туршилтын багц, баталгаажуулалтын хураангуй, төхөөрөмжийн лабораторийн шалгалт
Android эх сурвалжид хүрч, `artifacts/android/{lint,tests,attestation,device_lab}/` дор нотлох баримтыг байршуулах.

## 3. Хөвөн ба хамаарлын скан

Репо үндэсээс `scripts/android_lint_checks.sh --version <semver>`-г ажиллуул. The
скриптийг гүйцэтгэдэг:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- Тайлан болон хараат байдлын хамгаалалтын гаралтыг доор архивласан
  `artifacts/android/lint/<label>/` болон хувилбарт зориулсан `latest/` тэмдэгт холбоос
  дамжуулах хоолой.
- Амжилтгүй болсон хөвөнгийн олдворууд нь засч залруулах эсвэл хувилбарт оруулах шаардлагатай
  Хүлээн зөвшөөрөгдсөн эрсдэлийг баримтжуулсан RFC (Release Engineering + Program
  Тэргүүлэх).
- `dependencyGuardBaseline` хамаарлын түгжээг сэргээдэг; ялгааг хавсаргана уу
  явах/хэрэггүй пакет руу.

## 4. Төхөөрөмжийн лаборатори ба StrongBox хамрах хүрээ

1. Pixel + Galaxy төхөөрөмжүүдийг энд дурдсан багтаамж хянагч ашиглан нөөцлөөрэй
   `docs/source/compliance/android/device_lab_contingency.md`. Хувилбаруудыг блоклодог
   хэрэв ` дарж баталгаажуулалтын тайланг сэргээнэ үү.
3. Хэмжих хэрэгслийн матрицыг ажиллуул (төхөөрөмж дэх багц/ABI жагсаалтыг баримтжуулна
   мөрдөгч). Дахин оролдлого амжилттай болсон ч тохиолдлын бүртгэлд алдааг тэмдэглэ.
4. Firebase Test Lab руу буцах шаардлагатай бол тасалбар бөглөх; тасалбарыг холбоно уу
   доорх шалгах хуудаснаас.

## 5. Compliance & Telemetry Artefacts- ЕХ-ны хувьд `docs/source/compliance/android/and6_compliance_checklist.md`-г дагаж мөрдөөрэй
  болон JP илгээлтүүд. `docs/source/compliance/android/evidence_log.csv`-г шинэчилнэ үү
  хэш + Buildkite ажлын URL-уудтай.
- Телеметрийн редакцийн нотолгоог дамжуулан сэргээнэ үү
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`.
  Үүссэн JSON-г доор хадгална уу
  `artifacts/android/telemetry/<version>/status.json`.
- Схемийн ялгааны гаралтыг тэмдэглэ
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  Зэв экспортлогчидтой эн тэнцүү байгааг нотлох.

## 6. Provenance, SBOM, Publishing

1. Нийтлэх шугамыг хуурай ажиллуулна уу:

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. SBOM + Sigstore гарал үүсэл үүсгэх:

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. `artifacts/android/provenance/<semver>/manifest.json` хавсаргаж гарын үсэг зурна
   RFC хувилбар руу `checksums.sha256`.
4. Жинхэнэ Maven репозиторыг сурталчлахдаа дахин ажиллуул
   `scripts/publish_android_sdk.sh` `--dry-run`-гүй, консолыг барих
   бүртгэж, үүссэн олдворуудыг `artifacts/android/maven/<semver>` руу байршуулна уу.

## 7. Илгээх багцын загвар

GA/LTS/hortfix хувилбар бүр дараахь зүйлийг агуулна.

1. **Дууссан шалгах хуудас** — энэ файлын хүснэгтийг хуулж, зүйл бүрийг тэмдэглээд холбоосыг оруулна уу
   туслах олдворууд (Buildkite run, logs, doc diffs).
2. **Төхөөрөмжийн лабораторийн нотлох баримт** — баталгаажуулалтын тайлангийн хураангуй, захиалгын бүртгэл, ба
   аливаа гэнэтийн идэвхжүүлэлт.
3. **Телеметрийн багц** — редакцийн төлөв JSON, схемийн ялгаа, холбоос
   `docs/source/sdk/android/telemetry_redaction.md` шинэчлэлтүүд (хэрэв байгаа бол).
4. ** Нийцлийн олдвор** — нийцлийн хавтсанд нэмсэн/шинэчилсэн оруулгууд
   дээр нь шинэчилсэн нотлох бүртгэлийн CSV.
5. **Provenance bundle** — SBOM, Sigstore гарын үсэг, `checksums.sha256`.
6. **Хувилбарын хураангуй** — нэг хуудас тоймыг `status.md` тоймд хавсаргав
   дээрх (огноо, хувилбар, чөлөөлөгдсөн хаалганы онцлох зүйл).

Багцыг `artifacts/android/releases/<version>/` доор хадгалаад лавлана уу
`status.md` болон RFC хувилбар дээр.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` автоматаар
  хамгийн сүүлийн үеийн хулдаастай архивыг (`artifacts/android/lint/latest`) болон
  нийцлийн нотлох баримт `artifacts/android/releases/<version>/` руу нэвтэрч, тиймээс
  Илгээх пакет нь үргэлж каноник байрлалтай байдаг.

---

**Сануулга:** шинэ CI ажил, дагаж мөрдөх олдворууд,
эсвэл телеметрийн шаардлагыг нэмсэн. Замын зургийн AND6 зүйл нь огноо хүртэл нээлттэй байна
Хяналтын хуудас болон холбогдох автоматжуулалт нь хоёр дараалсан хувилбарт тогтвортой байна
галт тэрэгнүүд.