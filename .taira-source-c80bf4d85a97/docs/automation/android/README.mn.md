---
lang: mn
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

# Android баримтжуулалтын автоматжуулалтын суурь (AND5)

Замын зургийн AND5 зүйлд баримтжуулалт, нутагшуулах, хэвлэн нийтлэх шаардлагатай
AND6 (CI & Compliance) эхлэхээс өмнө автоматжуулалтыг шалгах боломжтой. Энэ хавтас
AND5/AND6-д дурдсан тушаалууд, эд өлгийн зүйлс, нотлох баримтын бүдүүвчийг бүртгэж,
баригдсан төлөвлөгөөг тусгах
`docs/source/sdk/android/developer_experience_plan.md` ба
`docs/source/sdk/android/parity_dashboard_plan.md`.

## Дамжуулах хоолой ба тушаалууд

| Даалгавар | Тушаал(ууд) | Хүлээгдэж буй олдворууд | Тэмдэглэл |
|------|------------|-------------------|-------|
| Локалчлалын sub синк | `python3 scripts/sync_docs_i18n.py` (заавал гүйлт бүрт `--lang <code>` дамжуулна) | `docs/automation/android/i18n/<timestamp>-sync.log` дээр хадгалагдсан бүртгэлийн файл дээр орчуулагдсан stub commits | `docs/i18n/manifest.json`-г орчуулсан бүдүүвчтэй синхрончилдог; бүртгэлд хүрсэн хэлний кодууд болон үндсэн шугамд баригдсан git commit зэргийг бичдэг. |
| Norito бэхэлгээ + паритет баталгаажуулалт | `ci/check_android_fixtures.sh` (`python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json` ороосон) | Үүсгэсэн хураангуй JSON-г `docs/automation/android/parity/<stamp>-summary.json` | руу хуулна уу `java/iroha_android/src/test/resources` ачаалал, манифест хэш болон гарын үсэг зурсан бэхэлгээний уртыг баталгаажуулна. Хураангуйг `artifacts/android/fixture_runs/`-ийн дагуу хэмжилтийн нотолгоог хавсаргана уу. |
| Манифест ба хэвлэлийн нотолгооны жишээ | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (туршилтууд + SBOM + гарал үүсэл) | `docs/automation/android/samples/<version>/` доор хадгалагдсан `docs/source/sdk/android/samples/`-аас гарсан `sample_manifest.json` гарал үүслийн багц метадата | AND5 жишээ апп-уудыг холбож, автоматжуулалтыг хамтад нь гаргана уу — үүсгэсэн манифест, SBOM хэш, гарал үүслийн бүртгэлийг бета шалгалтанд оруулна. |
| Паритын хяналтын самбарын хангамж | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json`, дараа нь `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | `metrics.prom` хормын хувилбар эсвэл Grafana экспортын JSON-г `docs/automation/android/parity/<stamp>-metrics.prom` руу хуулна уу | Хяналтын самбарын төлөвлөгөөг хангадаг тул AND5/AND7 засаглал нь хүчингүй илгээх тоолуур болон телеметрийн үр дүнг баталгаажуулах боломжтой. |

## Нотлох баримт авах

1. **Бүх зүйлийг цаг тэмдэглэнэ үү.** UTC цагийн тэмдэг ашиглан файлуудыг нэрлэнэ үү
   (`YYYYMMDDTHHMMSSZ`) тул паритын хяналтын самбар, засаглалын тэмдэглэл, нийтлэгдсэн
   docs нь ижил гүйлтийг тодорхойлон зааж өгч болно.
2. **Лавлагаа.** Бүртгэл бүр нь гүйлтийн git commit хэшийг агуулсан байх ёстой.
   дээр нь холбогдох тохиргоо (жишээ нь, `ANDROID_PARITY_PIPELINE_METADATA`).
   Хувийн нууцлалыг засварлах шаардлагатай үед тэмдэглэл оруулаад аюулгүй хадгалах сан руу холбоно уу.
3. **Хамгийн бага контекстийг архивлана.** Бид зөвхөн бүтэцлэгдсэн хураангуйг (JSON,
   `.prom`, `.log`). Хүнд олдворууд (APK багц, дэлгэцийн агшин) дотор үлдэх ёстой
   `artifacts/` эсвэл бүртгэлд бүртгэгдсэн гарын үсэг бүхий хэш бүхий объектын хадгалалт.
4. **Төлөвийн бичилтүүдийг шинэчлэх.** `status.md`-д AND5 үе шат ахих үед иш татна
   харгалзах файл (жишээ нь, `docs/automation/android/parity/20260324T010203Z-summary.json`)
   Ингэснээр аудиторууд CI бүртгэлийг хусахгүйгээр үндсэн үзүүлэлтийг хянах боломжтой.

Энэхүү бүдүүвчийг дагаж мөрдөх нь "баримт бичиг/автоматжуулалтын үндсэн үзүүлэлтүүдийг хангана
аудит" гэсэн урьдчилсан нөхцөл нь AND6 нь Android баримтжуулалтын программыг иш татаж, хадгалж байдаг
хэвлэгдсэн төлөвлөгөөний дагуу .