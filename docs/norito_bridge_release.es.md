---
lang: es
direction: ltr
source: docs/norito_bridge_release.md
status: complete
translator: manual
source_hash: bc7c766ff5fb0504f4da43a017bf294758800b9c815affc8f97b9bcc94ae8e15
source_last_modified: "2025-11-02T04:40:28.805628+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/norito_bridge_release.md (NoritoBridge Release Packaging) -->

# Empaquetado de NoritoBridge para lanzamientos

Esta guía describe los pasos necesarios para publicar los bindings Swift de
`NoritoBridge` como un XCFramework consumible desde Swift Package Manager y CocoaPods. El
flujo mantiene los artefactos de Swift en lock‑step con las versiones del crate Rust que
envía el codec Norito de Iroha. Para instrucciones de extremo a extremo sobre cómo
consumir los artefactos publicados en una app (configuración del proyecto Xcode, uso de
ChaChaPoly, etc.), consulta `docs/connect_swift_integration.md`.

> **Nota:** La automatización de CI para este flujo llegará cuando haya builders macOS con
> el tooling de Apple requerido (seguido en el backlog de macOS builders de Release
> Engineering). Hasta entonces, los pasos siguientes deben ejecutarse manualmente en un
> Mac de desarrollo.

## Prerrequisitos

- Un host macOS con las Xcode Command Line Tools estables más recientes instaladas.
- Toolchain de Rust que coincida con `rust-toolchain.toml` en la raíz del workspace.
- Toolchain de Swift 5.7 o posterior.
- CocoaPods (vía Ruby gems) si se va a publicar en el repositorio central de specs.
- Acceso a las claves de firma de lanzamientos de Hyperledger Iroha para etiquetar los
  artefactos de Swift.

## Modelo de versionado

1. Determina la versión del crate Rust para el codec Norito (`crates/norito/Cargo.toml`).
2. Etiqueta el workspace con el identificador de release (por ejemplo, `v2.1.0`).
3. Usa la misma versión semántica para el paquete Swift y el podspec de CocoaPods.
4. Cuando el crate Rust incremente su versión, repite el proceso y publica un artefacto
   Swift coincidente. Durante las pruebas se permiten sufijos de metadatos (por ejemplo,
   `-alpha.1`).

## Pasos de build

1. Desde la raíz del repositorio, ejecuta el script helper para ensamblar el XCFramework:

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   El script compila la biblioteca bridge de Rust para los targets de iOS y macOS y agrupa
   las librerías estáticas resultantes en un único directorio XCFramework.

2. Crea un zip del XCFramework para su distribución:

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. Actualiza el manifest del paquete Swift (`IrohaSwift/Package.swift`) para apuntar a la
   nueva versión y checksum:

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   Registra el checksum en `Package.swift` al definir el binary target.

4. Actualiza `IrohaSwift/IrohaSwift.podspec` con la nueva versión, checksum y URL del
   archivo.

5. **Regenera headers si el bridge expone nuevas funciones.** El bridge de Swift ahora
   expone `connect_norito_set_acceleration_config` para que `AccelerationSettings` pueda
   activar backends Metal/GPU. Asegúrate de que
   `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h` coincida con
   `crates/connect_norito_bridge/include/connect_norito_bridge.h` antes de crear el zip.

6. Ejecuta la suite de validación en Swift antes de etiquetar:

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   El primer comando comprueba que el paquete Swift (incluido `AccelerationSettings`)
   siga compilando correctamente; el segundo valida la paridad de fixtures, renderiza los
   dashboards de paridad/CI y ejerce las mismas comprobaciones de telemetría que se
   aplican en Buildkite (incluida la obligación de publicar el metadata
   `ci/xcframework-smoke:<lane>:device_tag`).

7. Haz commit de los artefactos generados en una rama de release y etiqueta el commit.

## Publicación

### Swift Package Manager

- Empuja la tag al repositorio Git público.
- Verifica que la tag sea visible para el índice de paquetes (Apple o el mirror de la
  comunidad).
- Los consumidores pueden depender de
  `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`.

### CocoaPods

1. Valida el pod localmente:

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. Sube el podspec actualizado:

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. Confirma que la nueva versión aparezca en el índice de CocoaPods.

## Consideraciones de CI

- Crea un job de macOS que ejecute el script de empaquetado, archive artefactos y suba el
  checksum generado como output del workflow.
- Haz que los lanzamientos dependan de que la app demo en Swift se construya contra el
  framework recién producido.
- Conserva los logs de build para ayudar a diagnosticar fallos.

## Ideas de automatización adicionales

- Utilizar `xcodebuild -create-xcframework` directamente cuando todos los targets
  necesarios estén expuestos.
- Integrar firmado/notarización para distribución fuera de máquinas de desarrollo.
- Mantener las pruebas de integración en lock‑step con la versión empaquetada fijando la
  dependencia SPM a la tag de release.

