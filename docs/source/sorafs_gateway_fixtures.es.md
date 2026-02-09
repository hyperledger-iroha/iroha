---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_fixtures.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4fa6bff21db4a389a869499b8ff325637c05dc0332513dfaf9f863587bf5018e
source_last_modified: "2025-11-21T13:40:07.390645+00:00"
translation_last_reviewed: "2026-01-30"
---

# Governance de fixtures del gateway SoraFS

El perfil de entrega trustless SF-5a se valida contra un bundle de fixtures
determinista que incluye manifiestos, artefactos PoR, un payload determinista y
escenarios de replay. Este documento registra la politica de mantenimiento,
metadata de release y expectativas operativas para esos fixtures.

## Release actual

| Campo | Valor |
|-------|-------|
| Version | `1.0.0` |
| Perfil | `sf1` |
| Released | 2026-02-12 00:00:00Z (`1770854400` Unix seconds) |
| Digests |
| | `fixtures_digest_blake3_hex = fa5bcbc0776fcc762c6df13b0dfa8ad15673fd65509f505ea3ea0f0ffab32cdf` |
| | `manifest_blake3_hex = ecc2e8564dda27834b8bd53a3eebdc56055d3e2cbdd30b0f96938fb9f04b216e` |
| | `payload_blake3_hex = 91275991d58858bdc7ce3eb4472b61c5289dec3ecc6cf43c6411db772c1888a8` |
| | `car_blake3_hex = ce50a9aadf84e57559208d39201621262fd1b1887ae490ca54470e2a00153f27` |

El digest anunciado en telemetria es el hash BLAKE3 del manifiesto canonico,
PoR challenge/proof, payload determinista y archivo CAR concatenados en ese
orden. Cuando los fixtures se re-derivan debe observarse el mismo digest; cualquier
drift indica que el bundle divergio del release publicado.

## Layout del directorio

Los bundles de fixtures viven bajo `fixtures/sorafs_gateway/<semver>/`. La
version `1.0.0` contiene los siguientes artefactos:

| Archivo | Descripcion |
|---------|-------------|
| `manifest_v1.to` | Manifiesto codificado en Norito alineado con el perfil de chunker `sf1` |
| `challenge_v1.to`, `proof_v1.to` | Fixtures PoR ligados al digest del manifiesto |
| `payload.bin` | Payload determinista de 1 MiB usado para generar el CAR |
| `gateway.car` | Stream CAR determinista emitido por el harness |
| `payload.blake3`, `gateway_car.blake3` | Digests de conveniencia para spot checks |
| `scenarios.json` | Matriz de replay (casos de exito + rechazo) |
| `metadata.json` | Timestamp de release, digest de fixtures, digests de manifiesto y payload |

Los mirrors JSON de los payloads Norito se incluyen junto a los artefactos `.to`
por conveniencia, pero los SDKs downstream deben seguir usando los encodings
Norito canonicos al validar manifiestos y pruebas.

## Workflow de regeneracion

1. Ejecutar `cargo xtask sorafs-gateway-fixtures --out fixtures/sorafs_gateway/<ver>`
   para materializar el bundle bajo el directorio de version deseado.
2. Verificar `metadata.json` y los helpers `*.blake3` contra las metricas de
   telemetria `torii_sorafs_gateway_fixture_info` (`version`, `profile`, `fixtures_digest`)
   y `torii_sorafs_gateway_fixture_version` (`version`). Ejecutar
   `scripts/verify_sorafs_fixtures.sh --dir fixtures/sorafs_gateway/<ver>`
   (envuelve `cargo xtask sorafs-gateway-fixtures --verify`) para realizar checks
   de digest, decodificacion Norito y diff de escenarios automaticamente.
3. Producir un bundle de atestacion via `cargo xtask sorafs-gateway-attest` y
   archivar el manifiesto/digest firmado junto a las release notes.
4. Una vez que las llaves de governance esten disponibles, actualizar
   `fixtures/sorafs_chunker/manifest_signatures.json` con el sobre del council.

## Helpers de verificacion

Los equipos que regeneran o espejan los fixtures pueden correr el probe
automatizado `scripts/verify_sorafs_fixtures.sh` para confirmar que el bundle
coincide con la metadata publicada. El wrapper invoca
`cargo xtask sorafs-gateway-fixtures --verify` para:

- decodificar los artefactos `.to` de Norito y asegurar que coinciden con el schema esperado;
- recomputar los digests de manifiesto/payload/CAR mas el agregado
  `fixtures_digest_blake3_hex`;
- confirmar que los archivos helper (`payload.blake3`, `gateway_car.blake3`) reflejan
  los hashes calculados; y
- hacer diff de `scenarios.json` contra la matriz canonica enviada con el repo.

Para CI o paquetes de governance que necesiten un resumen unico de los checks
de trustless, correr el verificador dedicado:

```bash
cargo run -p sorafs_car --bin soranet_trustless_verifier --features cli --locked -- \
  --manifest fixtures/sorafs_gateway/1.0.0/manifest_v1.to \
  --car fixtures/sorafs_gateway/1.0.0/gateway.car \
  --json-out artifacts/sorafs_gateway/trustless_summary.json
```

El resumen registra los digests de manifiesto/CAR/payload, el digest SHA3-256
reconstruido del plan de chunks y la raiz PoR para que el suite de conformidad
pruebe alineacion de chunk-plan y PoR sin scripts ad hoc.

Pasa `--dir <path>` para chequear un directorio de release distinto y
`--allow-online` si Cargo debe descargar dependencias (CI usa offline por
defecto).

## Telemetria y deteccion de drift

Torii expone dos metricas:

* `torii_sorafs_gateway_fixture_info{version="…",profile="…",fixtures_digest="…"}` –
  el gauge es el timestamp del release; las alertas deben disparar cuando
  aparezcan labels desconocidas.
* `torii_sorafs_gateway_fixture_version{version="…"}` – gauge nuevo seteado en `1`
  para la version activa. Prometheus/Alertmanager pueden detectar versiones
  inesperadas de fixtures revisando valores en cero.

Ambas metricas se pueblan al arranque via el hook de telemetria nuevo y dan a
los operadores visibilidad inmediata del drift de fixtures junto a la
telemetria TLS y GAR existente.

## Governance de releases

1. Tooling WG prepara updates de fixtures en una feature branch y regenera el
   bundle localmente.
2. Networking TL revisa manifiesto, metadata y diffs de escenarios. Cualquier
   cambio a fixtures PoR o al perfil de chunker requiere sign-off explicito.
3. Governance Council firma el sobre de manifiesto actualizado y adjunta el
   nuevo digest a las release notes.
4. Actualizar este documento y
   `docs/source/sorafs_gateway_conformance.md` con timestamp de release, digests
   y guia operativa. Taggear el commit que aterriza los fixtures y publicar
   mirrors IPFS/SoraFS si aplica.
5. Los operadores regeneran atestaciones via
   `cargo xtask sorafs-gateway-attest` y archivan el bundle firmado junto al
   output de metricas de su plano de telemetria.

## Acciones pendientes

- [x] Regenerar los fixtures canonicos y publicar los digests arriba. ✅
- [ ] Adjuntar el sobre de manifiesto firmado para la version `1.0.0` una vez que
      el material de llaves del council este disponible.
