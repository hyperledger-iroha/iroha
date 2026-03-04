---
lang: es
direction: ltr
source: docs/portal/docs/reference/account-address-status.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: dirección-cuenta-estado
título: اکاؤنٹ ایڈریس تعمیل
descripción: Dispositivo ADDR-2 ورک فلو کا خلاصہ اور SDK ٹیموں کی ہم آہنگی کیسے برقرار رہتی ہے۔
---

paquete canónico ADDR-2 (`fixtures/account/address_vectors.json`) IH58 (preferido), comprimido (`sora`, segundo mejor; ancho medio/completo), firma múltiple, accesorios negativos y captura de pantalla ہر SDK + Torii superficie اسی JSON پر انحصار کرتی ہے تاکہ codec drift پروڈکشن تک پہنچنے سے پہلے پکڑا جا سکے۔ یہ صفحہ اندرونی resumen de estado (`docs/source/account_address_status.md` ریپوزٹری روٹ میں) کو mirror کرتا ہے تاکہ lectores de portal بغیر mono-repo میں کھوج لگائے flujo de trabajo دیکھ سکیں۔

## Paquete کو regenerar یا verificar کریں

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Banderas:

- `--stdout` — inspección ad-hoc کیلئے JSON کو stdout پر emit کرتا ہے۔
- `--out <path>` — مختلف ruta پر لکھتا ہے (مثلا لوکل diff کے وقت)۔
- `--verify` — copia de trabajo کو تازہ generar شدہ contenido کے ساتھ comparar کرتا ہے (`--stdout` کے ساتھ combinar نہیں ہو سکتا)۔

Flujo de trabajo de CI **Deriva del vector de dirección** `cargo xtask address-vectors --verify`
کیا جا سکے۔

## Calendario کون استعمال کرتا ہے؟

| Superficie | Validación |
|---------|------------|
| Modelo de datos de Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (servidor) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK de JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK rápido | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK de Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |ہر aprovechar bytes canónicos + IH58 + codificaciones comprimidas (`sora`, segunda mejor) کا ida y vuelta کرتا ہے اور casos negativos کیلئے Códigos de error de estilo Norito کو accesorio کے ساتھ coincidencia کرتا ہے۔

## Automatización چاہئے؟

Actualizaciones del accesorio de herramientas de liberación کو `scripts/account_fixture_helper.py` کے ذریعے script کر سکتی ہے، جو copiar/pegar کے بغیر paquete canónico buscar یا verificar کرتا ہے:

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

El asistente `--source` anula la variable de entorno `IROHA_ACCOUNT_FIXTURE_URL`. سکیں۔ جب `--metrics-out` دیا جاتا ہے تو helper `account_address_fixture_check_status{target="…"}` کے ساتھ resumen canónico SHA-256 (`account_address_fixture_remote_info`) لکھتا ہے تاکہ Prometheus recopiladores de archivos de texto اور Grafana tablero `account_address_fixture_status` ہر superficie کی sincronización ثابت کر سکیں۔ جب کوئی objetivo `0` رپورٹ کرے تو alerta کریں۔ automatización de múltiples superficies کیلئے wrapper `ci/account_fixture_metrics.sh` استعمال کریں (repetible `--target label=path[::source]` قبول کرتا ہے) تاکہ equipos de guardia nodo-exportador recopilador de archivos de texto کیلئے ایک consolidado `.prom` فائل publicar کر سکیں۔

## مکمل breve چاہئے؟

Estado de cumplimiento de ADDR-2 (propietarios, plan de seguimiento, elementos de acción abiertos) کی مکمل تفصیل
ریپوزٹری میں `docs/source/account_address_status.md` کے ساتھ Estructura de direcciones RFC (`docs/account_structure.md`) میں موجود ہے۔ اس صفحہ کو recordatorio operativo rápido کے طور پر استعمال کریں؛ تفصیلی رہنمائی کیلئے documentos de repositorio دیکھیں۔