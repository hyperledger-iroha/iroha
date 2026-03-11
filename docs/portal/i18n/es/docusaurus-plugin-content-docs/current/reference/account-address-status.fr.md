---
lang: es
direction: ltr
source: docs/portal/docs/reference/account-address-status.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: dirección-cuenta-estado
título: Conformite des adresses de compte
Descripción: Reanudar el flujo de trabajo del dispositivo ADDR-2 y la sincronización de los equipos SDK.
---

El paquete canonique ADDR-2 (`fixtures/account/address_vectors.json`) captura los dispositivos I105 (preferido), comprimido (`sora`, segundo mejor; ancho medio/completo), firma múltiple y negativos. Cada superficie SDK + Torii se aplica en el meme JSON para detectar todos los derivados del códec antes de la producción. Esta página refleja el breve de estatuto interno (`docs/source/account_address_status.md` en el depósito de Racine) para que los lectores del portal puedan consultar el flujo de trabajo sin seguimiento del monorrepositorio.

## Regenerar o verificar el paquete

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Banderas:

- `--stdout` - Emet le JSON vers stdout para inspección ad-hoc.
- `--out <path>` - ecrit vers un autre chemin (par ex. lors de la comparaison locale).
- `--verify` - compare la copia de trabajo con un contenido fraichement genere (ne peut pas etre combine con `--stdout`).

El flujo de trabajo CI **Dirección del vector de dirección** ejecuta `cargo xtask address-vectors --verify`
Cada vez que el dispositivo, el generador o los documentos cambian para alertar inmediatamente a los revisores.

## ¿Quié consumir el accesorio?| Superficie | Validación |
|---------|------------|
| Modelo de datos de Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (servidor) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK de JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK rápido | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK de Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Cada arnés haga un aller-retour des octetos canónicos + I105 + codificaciones compresas y verifique que los códigos de error de estilo Norito correspondientes al dispositivo para los casos negativos.

## ¿Besoin d'automatización?

Las herramientas de liberación pueden automatizar los ajustes de accesorios con el ayudante
`scripts/account_fixture_helper.py`, para recuperar o verificar el paquete canónico sin fotocopiadora/copiadora:

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

El asistente acepta las anulaciones `--source` o la variable de entorno `IROHA_ACCOUNT_FIXTURE_URL` para que los trabajos CI del SDK apuntan a su espejo preferido. Cuando `--metrics-out` está disponible, el asistente escribe `account_address_fixture_check_status{target="..."}` además del resumen SHA-256 canónico (`account_address_fixture_remote_info`) después de los recopiladores de archivos de texto Prometheus y el tablero Grafana. `account_address_fixture_status` puede comprobar que cada superficie permanece sincronizada. Alertez des qu'une cible rapporte `0`. Para la automatización de múltiples superficies, utilice el wrapper `ci/account_fixture_metrics.sh` (acepte las repeticiones `--target label=path[::source]`) después de equipar los archivos publicados con un solo archivo `.prom` consolidado para el recopilador de archivos de texto de node-exporter.

## ¿Besoin du breve completo?Le statut complet de conformite ADDR-2 (propietarios, plan de seguimiento, acciones abiertas)
Se encuentra en `docs/source/account_address_status.md` en su depósito, además de la estructura de direcciones RFC (`docs/account_structure.md`). Utilice esta página como rappel operacional rápido; Consulte los documentos del repositorio para obtener una guía apropiada.