---
lang: es
direction: ltr
source: docs/portal/docs/reference/account-address-status.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: dirección-cuenta-estado
título: Cumplimiento de direcciones de cuenta
descripción: Resumen del flujo de trabajo del accesorio ADDR-2 y como los equipos de SDK se mantienen sincronizados.
---

El paquete canonico ADDR-2 (`fixtures/account/address_vectors.json`) captura accesorios I105 (preferido), comprimido (`sora`, segundo mejor; ancho medio/completo), multifirma y negativos. Cada superficie de SDK + Torii se apoya en el mismo JSON para detectar cualquier deriva del códec antes de que llegue a producción. Esta página refleja el breve de estado interno (`docs/source/account_address_status.md` en el repositorio raiz) para que los lectores del portal consulten el flujo sin buscar en el mono-repo.

## Regenerar o verificar el paquete

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Banderas:

- `--stdout` - emite el JSON a stdout para inspección ad-hoc.
- `--out <path>` - escribe en una ruta diferente (p. ej., al comparar cambios localmente).
- `--verify` - compara la copia de trabajo contra contenido recién generado (no se puede combinar con `--stdout`).

El flujo de trabajo de CI **Address Vector Drift** ejecuta `cargo xtask address-vectors --verify`
cada vez que cambia el aparato, el generador o los documentos para alertar a los revisores de inmediato.

## ¿Quién consume el aparato?| Superficie | Validación |
|---------|------------|
| Modelo de datos de Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (servidor) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK de JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK rápido | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK de Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Cada arnés hace round-trip de bytes canónicos + I105 + codificaciones comprimidas y verifica que los códigos de error estilo Norito coinciden con el accesorio para los casos negativos.

## ¿Necesitas automatización?

Las herramientas de liberación pueden automatizar refrescos de accesorios con el ayudante.
`scripts/account_fixture_helper.py`, que obtiene o verifica el paquete canónico sin pasos de copiar/pegar:

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

El ayudante acepta overrides de `--source` o la variable de entorno `IROHA_ACCOUNT_FIXTURE_URL` para que los trabajos de CI de SDK apunten a su mirror preferido. Cuando se proporciona `--metrics-out`, el ayudante escribe `account_address_fixture_check_status{target="..."}` junto con el digest SHA-256 canonico (`account_address_fixture_remote_info`) para que los recopiladores de archivos de texto de Prometheus y el tablero de Grafana `account_address_fixture_status` puedan probar que cada superficie sigue en sincronía. Alerta cuando un objetivo informa `0`. Para automatización multi-superficie usa el wrapper `ci/account_fixture_metrics.sh` (acepta repetidores `--target label=path[::source]`) para que los equipos on-call publiquen un único archivo `.prom` consolidado para el recopilador de archivos de texto de node-exporter.

## ¿Necesitas el resumen completo?El estado completo de cumplimiento ADDR-2 (owners, plan de monitoreo, acciones abiertas) vive en `docs/source/account_address_status.md` dentro del repositorio junto con el Address Structure RFC (`docs/account_structure.md`). Usa esta página como recordatorio operativo rápido; para guías en profundidad, consulte los documentos del repositorio.