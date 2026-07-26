---
lang: es
direction: ltr
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-04T11:42:43.493867+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM y certificación de procedencia: SDK de Android

| Campo | Valor |
|-------|-------|
| Alcance | SDK de Android (`java/iroha_android`) + aplicaciones de muestra (`examples/android/*`) |
| Propietario del flujo de trabajo | Ingeniería de lanzamiento (Alexei Morozov) |
| Última verificación | 2026-02-11 (Cometa de construcción `android-sdk-release#4821`) |

## 1. Flujo de trabajo de generación

Ejecute el script auxiliar (agregado para la automatización AND6):

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

El script realiza lo siguiente:

1. Ejecuta `ci/run_android_tests.sh` e `scripts/check_android_samples.sh`.
2. Invoca el contenedor Gradle en `examples/android/` para crear SBOM CycloneDX para
   `:android-sdk`, `:operator-console` e `:retail-wallet` con el suministrado
   `-PversionName`.
3. Copia cada SBOM en `artifacts/android/sbom/<sdk-version>/` con nombres canónicos
   (`iroha-android.cyclonedx.json`, etc.).

## 2. Procedencia y firma

El mismo script firma cada SBOM con `cosign sign-blob --bundle <file>.sigstore --yes`
y emite `checksums.txt` (SHA-256) en el directorio de destino. Configure el `COSIGN`
Variable de entorno si el binario vive fuera de `$PATH`. Una vez finalizado el guión,
registre las rutas del paquete/suma de comprobación más la identificación de ejecución de Buildkite en
`docs/source/compliance/android/evidence_log.csv`.

## 3. Verificación

Para verificar un SBOM publicado:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

Compare el SHA de salida con el valor enumerado en `checksums.txt`. Los revisores también diferencian el SBOM de la versión anterior para garantizar que los deltas de dependencia sean intencionales.

## 4. Instantánea de la evidencia (2026-02-11)

| Componente | SBOM | SHA-256 | Paquete Sigstore |
|-----------|--------------|---------|-----------------|
| SDK de Android (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | Paquete `.sigstore` almacenado junto a SBOM |
| Ejemplo de consola del operador | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| Muestra de billetera minorista | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Los hashes capturados de Buildkite ejecutan `android-sdk-release#4821`; reprodúzcalos mediante el comando de verificación anterior).*

## 5. Trabajo sobresaliente

- Automatizar los pasos de SBOM + cofirmante dentro del proceso de lanzamiento antes de GA.
- Refleje los SBOM en el depósito de artefactos públicos una vez que AND6 marque la lista de verificación como completa.
- Coordine con Docs para vincular las ubicaciones de descarga de SBOM desde las notas de la versión orientadas a los socios.