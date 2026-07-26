---
lang: mn
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf9773ecd75fc31ee89da58a3c5eda846b910eb6e131f1e042b565892e028f16
source_last_modified: "2025-12-29T18:16:35.062011+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SDK Binding & Fixture Governance

Замын зураг дээрх WP1-E нь "баримт бичиг/холбоо"-ыг баримт бичгийг хадгалах каноник газар гэж нэрлэдэг.
хэл хоорондын холболтын төлөв. Энэхүү баримт бичиг нь заавал биелүүлэх бараа материалын бүртгэл,
нөхөн сэргээх командууд, шилжилтийн хамгаалалтууд, нотлох газрууд нь GPU паритет
хаалганууд (WP1-E/F/G) болон SDK хоорондын каденцийн зөвлөл нь нэг лавлагаатай.

## Хамтын хашлага
- **Каноник тоглуулах ном:** `docs/source/norito_binding_regen_playbook.md` тодорхой байна
  Android-д зориулсан эргэлтийн бодлого, хүлээгдэж буй нотлох баримтууд болон өргөлтийн ажлын урсгал,
  Swift, Python болон ирээдүйн холбоосууд.
- **Norito схемийн паритет:** `scripts/check_norito_bindings_sync.py` (дуудагдсан
  `scripts/check_norito_bindings_sync.sh` ба CI-д хаагдсан
  `ci/check_norito_bindings_sync.sh`) нь Rust, Java, эсвэл Python програмуудыг ашиглах үед блокуудыг блоклодог.
  схемийн олдворуудын шилжилт хөдөлгөөн.
- ** Cadence watchdog:** `scripts/check_fixture_cadence.py` уншдаг
  `artifacts/*_fixture_regen_state.json` файлууд болон Мяг/Баасан (Android,
  Python) болон Wed (Swift) цонхнууд нь замын зураглалын хаалганууд нь аудит хийх боломжтой цагийн тэмдэгтэй байдаг.

## Холбогч матриц

| Холбох | Нэвтрэх цэгүүд | Бэхэлгээ / дахин сэргээх команд | Дрифт хамгаалагч | Нотлох баримт |
|---------|--------------|-------------------------|--------------|----------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (сонголтоор `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## Холбох дэлгэрэнгүй

### Android (Java)
Android SDK нь `java/iroha_android/`-ийн дагуу ажилладаг бөгөөд Norito стандартыг ашигладаг.
`scripts/android_fixture_regen.sh` үйлдвэрлэсэн бэхэлгээ. Тэр туслагч экспортолдог
Rust хэрэгслийн гинжээс шинэ `.norito` толбо, шинэчлэлтүүд
`artifacts/android_fixture_regen_state.json` ба хэмжилтийн мета өгөгдлийг бүртгэдэг
`scripts/check_fixture_cadence.py` болон засаглалын хяналтын самбарыг ашигладаг. Дрифт бол
`scripts/check_android_fixtures.py`-ээр илрүүлсэн (мөн утастай
`ci/check_android_fixtures.sh`) болон `java/iroha_android/run_tests.sh`,
JNI холболтууд, WorkManager дарааллын дахин тоглуулалт болон StrongBox-ийн нөөцийг ашигладаг.
Эргэлтийн нотлох баримт, бүтэлгүйтлийн тэмдэглэл, дахин давталтын хуулбарууд доор амьдардаг
`artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
`IrohaSwift/` `scripts/swift_fixture_regen.sh`-ээр дамжуулан ижил Norito ачааллыг тусгадаг.
Скрипт нь эргэлт эзэмшигч, хэмнэлийн шошго, эх сурвалжийг бичдэг (`live` vs `archive`)
`artifacts/swift_fixture_regen_state.json` доторх ба мета өгөгдлийг файл руу оруулдаг
хэмнэл шалгагч. `scripts/swift_fixture_archive.py` нь засварлагчдад залгих боломжийг олгодог
Зэвээр үүсгэгдсэн архивууд; `scripts/check_swift_fixtures.py` ба
`ci/check_swift_fixtures.sh` нь байт түвшний паритет болон SLA насны хязгаарыг мөрддөг.
`scripts/swift_fixture_regen.sh` гарын авлагад `SWIFT_FIXTURE_EVENT_TRIGGER` дэмждэг
эргэлтүүд. Өргөтгөсөн ажлын урсгал, KPI болон хяналтын самбарыг баримтжуулсан болно
`docs/source/swift_parity_triage.md` ба каденсийн товч танилцуулга
`docs/source/sdk/swift/`.

### Python
Python клиент (`python/iroha_python/`) нь Android төхөөрөмжүүдийг хуваалцдаг. Гүйж байна
`scripts/python_fixture_regen.sh` нь хамгийн сүүлийн үеийн `.norito` ачааллыг татаж, сэргээдэг
`python/iroha_python/tests/fixtures/` ба кадентын мета өгөгдлийг гаргах болно
`artifacts/python_fixture_regen_state.json` нэг удаа замын зураглалын дараах анхны эргэлт
баригдсан. `scripts/check_python_fixtures.py` ба
`python/iroha_python/scripts/run_checks.sh` gate pytest, mypy, ruff, and fixture
орон нутгийн болон CI дахь паритет. Төгсгөл хүртэлх баримт бичиг (`docs/source/sdk/python/…`) болон
холбох regen тоглоомын ном нь Андройдтой эргэлтийг хэрхэн зохицуулахыг тайлбарладаг
эзэд.

### JavaScript
`javascript/iroha_js/` нь локал `.norito` файлууд дээр тулгуурладаггүй, гэхдээ WP1-E замууд
түүний хувилбарын нотолгоо нь GPU CI эгнээ нь бүрэн гарал үүслийг өвлөн авдаг. Хувилбар бүр
Гарал үүслийг `npm run release:provenance` (powered by
`javascript/iroha_js/scripts/record-release-provenance.mjs`), үүсгэж, тэмдэг тавина
SBOM нь `scripts/js_sbom_provenance.sh`-тай багц бөгөөд гарын үсэг зурсан хуурай гүйлтийг ажиллуулдаг.
(`scripts/js_signed_staging.sh`) ба бүртгэлийн олдворыг баталгаажуулна.
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. Үүссэн мета өгөгдөл
`artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`,
`artifacts/js/sbom/`, болон `artifacts/js/verification/` нь тодорхойлогчийг өгдөг
JS5/JS6 болон WP1-F жишиг замын зураглалын нотолгоо. Нийтлэх тоглоомын ном
`docs/source/sdk/js/` нь автоматжуулалтыг хооронд нь холбодог.