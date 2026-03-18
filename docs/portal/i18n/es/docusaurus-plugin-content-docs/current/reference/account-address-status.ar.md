---
lang: es
direction: ltr
source: docs/portal/docs/reference/account-address-status.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: dirección-cuenta-estado
título: امتثال عنوان الحساب
descripción: Utilice el dispositivo ADDR-2 y conecte el SDK.
---

Nombre del producto ADDR-2 (`fixtures/account/address_vectors.json`) Incluye accesorios I105 y comprimidos (`sora`, segundo mejor; ancho medio/completo) y multifirma y negativo. Utilice SDK + Torii para crear un códec JSON y un código fuente. تعكس هذه الصفحة المذكرة الداخلية للحالة (`docs/source/account_address_status.md` في جذر المستودع) حتى يتمكن قراء البوابة من الرجوع الى سير العمل دون التنقيب في الـ mono-repo.

## اعادة توليد او التحقق من الحزمة

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Banderas:

- `--stdout` — Establece JSON en la salida estándar.
- `--out <path>` — يكتب الى مسار مختلف (مثلا عند مقارنة تغييرات محلية).
- `--verify` — يقارن نسخة العمل بالمحتوى المولد حديثا (لا يمكن دمجه مع `--stdout`).

يشغل مسار CI **Dirección de vector de dirección** Nombre `cargo xtask address-vectors --verify`
عند تغير accesorio او المولد او الوثائق لتنبيه المراجعين فورا.

## من يستهلك accesorio؟

| السطح | التحقق |
|---------|------------|
| Modelo de datos de Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (servidor) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK de JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK rápido | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK de Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

كل arnés يجري ida y vuelta للبايتات القياسية + I105 + الترميزات المضغوطة ويتحقق من اكواد الخطأ بنمط Norito تطابق accesorio للحالات السلبية.

## هل تحتاج الى اتمتة؟يمكن لادوات الاصدار برمجة تحديثات accesorio عبر المساعد
`scripts/account_fixture_helper.py`, الذي يجلب او يتحقق من الحزمة القياسية دون خطوات نسخ/لصق:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

Este programa anula `--source` y el programa `IROHA_ACCOUNT_FIXTURE_URL` para configurar el CI en el SDK del servidor. المفضلة. Incluye `--metrics-out` y `account_address_fixture_check_status{target="…"}` para resumir SHA-256 (`account_address_fixture_remote_info`) y recopiladores de archivos de texto en Prometheus. La unidad Grafana `account_address_fixture_status` está conectada a una computadora. Aquí está el objetivo `0`. للاتمتة متعددة الاسطح استخدم الغلاف `ci/account_fixture_metrics.sh` (يقبل تكرار `--target label=path[::source]`) حتى تمكن فرق المناوبة من نشر Para `.prom`, coloque el archivo de texto en node-exporter.

## هل تحتاج الملخص الكامل؟

حالة الامتثال الكاملة لـ ADDR-2 (propietarios وخطة المراقبة وبنود العمل المفتوحة)
Utilice `docs/source/account_address_status.md` para acceder a la estructura de direcciones RFC (`docs/account_structure.md`). استخدم هذه الصفحة كتذكير تشغيلي سريع؛ وارجع الى وثائق المستودع للارشاد المفصل.