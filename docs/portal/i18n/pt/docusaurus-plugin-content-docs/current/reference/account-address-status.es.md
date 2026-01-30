---
lang: pt
direction: ltr
source: docs/portal/docs/reference/account-address-status.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: account-address-status
title: Cumplimiento de direcciones de cuenta
description: Resumen del flujo de trabajo del fixture ADDR-2 y como los equipos de SDK se mantienen sincronizados.
---

El paquete canonico ADDR-2 (`fixtures/account/address_vectors.json`) captura fixtures IH58 (preferred), compressed (`sora`, second-best; half/full width), multisignature y negativos. Cada superficie de SDK + Torii se apoya en el mismo JSON para detectar cualquier deriva del codec antes de que llegue a produccion. Esta pagina refleja el brief de estado interno (`docs/source/account_address_status.md` en el repositorio raiz) para que los lectores del portal consulten el flujo sin buscar en el mono-repo.

## Regenerar o verificar el paquete

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Flags:

- `--stdout` - emite el JSON a stdout para inspeccion ad-hoc.
- `--out <path>` - escribe en una ruta diferente (p. ej., al comparar cambios localmente).
- `--verify` - compara la copia de trabajo contra contenido recien generado (no se puede combinar con `--stdout`).

El workflow de CI **Address Vector Drift** ejecuta `cargo xtask address-vectors --verify`
cada vez que cambia el fixture, el generador o los docs para alertar a los reviewers de inmediato.

## Quien consume el fixture?

| Superficie | Validacion |
|---------|------------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Cada harness hace round-trip de bytes canonicos + IH58 + codificaciones comprimidas y verifica que los codigos de error estilo Norito coincidan con el fixture para los casos negativos.

## Necesitas automatizacion?

Las herramientas de release pueden automatizar refrescos de fixtures con el helper
`scripts/account_fixture_helper.py`, que obtiene o verifica el paquete canonico sin pasos de copiar/pegar:

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

El helper acepta overrides de `--source` o la variable de entorno `IROHA_ACCOUNT_FIXTURE_URL` para que los jobs de CI de SDK apunten a su mirror preferido. Cuando se proporciona `--metrics-out`, el helper escribe `account_address_fixture_check_status{target="..."}` junto con el digest SHA-256 canonico (`account_address_fixture_remote_info`) para que los textfile collectors de Prometheus y el dashboard de Grafana `account_address_fixture_status` puedan probar que cada superficie sigue en sincronia. Alerta cuando un target reporte `0`. Para automatizacion multi-superficie usa el wrapper `ci/account_fixture_metrics.sh` (acepta `--target label=path[::source]` repetidos) para que los equipos on-call publiquen un unico archivo `.prom` consolidado para el textfile collector de node-exporter.

## Necesitas el brief completo?

El estado completo de cumplimiento ADDR-2 (owners, plan de monitoreo, acciones abiertas) vive en `docs/source/account_address_status.md` dentro del repositorio junto con el Address Structure RFC (`docs/account_structure.md`). Usa esta pagina como recordatorio operativo rapido; para guias en profundidad, consulta los docs del repo.
