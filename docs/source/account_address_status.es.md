---
lang: es
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c1214f4d0ad86449c0ef4b8f8cbaa38fe265bab4afcc2930cd30a57c089e6d7
source_last_modified: "2025-11-15T05:05:33.914289+00:00"
translation_last_reviewed: 2026-01-01
---

## Estado de cumplimiento de direccion de cuenta (ADDR-2)

Estado: Aceptado 2026-03-30  
Propietarios: Equipo de modelo de datos / Gremio de QA  
Referencia de roadmap: ADDR-2 - Dual-Format Compliance Suite

### 1. Resumen

- Fixture: `fixtures/account/address_vectors.json` (IH58 + compressed + multisig casos positivos/negativos).
- Alcance: payloads V1 deterministas que cubren implicit-default, Local-12, Global registry y controladores multisig con taxonomia completa de errores.
- Distribucion: compartido entre Rust data-model, Torii, SDKs JS/TS, Swift y Android; CI falla si algun consumidor se desvia.
- Fuente de verdad: el generador vive en `crates/iroha_data_model/src/account/address/compliance_vectors.rs` y se expone via `cargo xtask address-vectors`.

### 2. Regeneracion y verificacion

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

Opciones:

- `--out <path>` - anulacion opcional al producir bundles ad hoc (por defecto `fixtures/account/address_vectors.json`).
- `--stdout` - emite JSON a stdout en lugar de escribir a disco.
- `--verify` - compara el archivo actual con el contenido recien generado (falla rapido ante drift; no se puede combinar con `--stdout`).

### 3. Matriz de artefactos

| Superficie | Aplicacion | Notas |
|---------|-------------|-------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` | Analiza el JSON, reconstruye payloads canonicos y comprueba conversiones IH58/compressed/canonical + errores estructurados. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | Valida codecs del lado servidor para que Torii rechace payloads IH58/compressed malformados de forma determinista. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | Replica fixtures V1 (IH58/compressed/fullwidth) y afirma codigos de error estilo Norito para cada caso negativo. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | Ejercita decodificacion IH58/compressed, payloads multisig y exposicion de errores en plataformas Apple. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Garantiza que los bindings Kotlin/Java sigan alineados con el fixture canonico. |

### 4. Monitoreo y trabajo pendiente

- Informe de estado: este documento esta enlazado desde `status.md` y el roadmap para que las revisiones semanales verifiquen la salud del fixture.
- Resumen en portal de desarrolladores: ver **Reference -> Account address compliance** en el portal de docs (`docs/portal/docs/reference/account-address-status.md`).
- Prometheus y dashboards: cuando verifiques una copia del SDK, ejecuta el helper con `--metrics-out` (y opcionalmente `--metrics-label`) para que el textfile collector de Prometheus ingiera `account_address_fixture_check_status{target=...}`. El dashboard de Grafana **Account Address Fixture Status** (`dashboards/grafana/account_address_fixture_status.json`) muestra conteos pass/fail por superficie y expone el digest SHA-256 canonico como evidencia de auditoria. Alertar cuando cualquier target reporte `0`.
- Torii metrics: `torii_address_domain_total{endpoint,domain_kind}` ahora emite para cada account literal parseado con exito, reflejando `torii_address_invalid_total`/`torii_address_local8_total`. Alertar sobre trafico `domain_kind="local12"` en produccion y espejar los contadores en el dashboard SRE `address_ingest` para que la retirada de Local-12 tenga evidencia auditada.
- Fixture helper: `scripts/account_fixture_helper.py` descarga o verifica el JSON canonico para que la automatizacion de releases del SDK pueda obtener/verificar el bundle sin copiar/pegar manual, y opcionalmente escriba metricas de Prometheus. Ejemplo:

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \
    --target path/to/sdk/address_vectors.json \
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
    --metrics-label android
  ```

  El helper escribe `account_address_fixture_check_status{target="android"} 1` cuando el target coincide, mas los gauges `account_address_fixture_remote_info` / `account_address_fixture_local_info` que exponen digests SHA-256. Archivos ausentes reportan `account_address_fixture_local_missing`.
  Automation wrapper: llama a `ci/account_fixture_metrics.sh` desde cron/CI para emitir un textfile consolidado (por defecto `artifacts/account_fixture/address_fixture.prom`). Pasa entradas repetidas de `--target label=path` (opcionalmente anade `::https://mirror/...` por target para sobreescribir la fuente) para que Prometheus raspe un solo archivo que cubra cada copia SDK/CLI. El workflow de GitHub `address-vectors-verify.yml` ya ejecuta este helper contra el fixture canonico y sube el artefacto `account-address-fixture-metrics` para ingestion SRE.
