---
lang: es
direction: ltr
source: docs/source/swift_xcframework_device_matrix.md
status: complete
translator: manual
source_hash: 9647b58e9eaef2bd28b982b2a462ea3cad475bddbd55535bad03a89bd4f99811
source_last_modified: "2025-11-02T04:40:40.031485+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/source/swift_xcframework_device_matrix.md (Swift XCFramework Smoke Harness Device Matrix) -->

# Matriz de dispositivos del harness de smoke Swift XCFramework

Este documento recoge los destinos de dispositivo canónicos para las pruebas de smoke del
XCFramework IOS6. Los jobs de Buildkite y los scripts de verificación local deben adherirse
a esta matriz para que los equipos de Swift y de SDKs cruzados compartan unas expectativas
de cobertura idénticas.

## Principios clave

1. **Cobertura determinista.** Cada ejecución ejercita el mismo conjunto de simuladores y
   dispositivos físicos para minimizar fallos no deterministas.
2. **Paridad StrongBox.** Al menos una lane debe ejecutarse sobre hardware con Secure
   Enclave/StrongBox para validar las rutas de gestión de claves que los simuladores no
   pueden emular.
3. **Degradación gradual.** Cuando los simuladores principales no están disponibles (por
   ejemplo, retrasos tras una actualización de Xcode o saturación del pool de CI), el
   harness recurre a ejecutar el target macOS Catalyst del host mientras emite telemetría
   indicando que se ha usado el fallback.

## Lanes requeridas

| Lane ID | Destination | Propósito | Notas |
|---------|-------------|-----------|-------|
| `xcframework-smoke/iphone-sim` | `platform=iOS Simulator,name=iPhone 15` | Pruebas de smoke SIMD/Metal de base sobre el simulador de iPhone más reciente. | Garantiza que los benchmarks de Merkle/CRC se ejecutan con la aceleración Metal activada. |
| `xcframework-smoke/ipad-sim` | `platform=iOS Simulator,name=iPad (10th generation)` | Cobertura de formato tablet y chequeos de memoria con huella mayor. | Verifica el harness de UI multi‑ventana y el streaming de adjuntos. |
| `xcframework-smoke/strongbox` | `platform=iOS,name=iPhone 14 Pro,udid=<device-udid>` | Validación StrongBox/Secure Enclave sobre hardware físico. | Requiere reserva previa de dispositivo; la lane bloquea hasta que haya dispositivo disponible. |
| `xcframework-smoke/macos-fallback` | `platform=macOS,arch=arm64,variant=Designed for iPad` | Ruta de fallback Catalyst cuando faltan simuladores. | Se programa automáticamente solo cuando los simuladores necesarios no pueden arrancar. |

### Variables helper para destinos

Los jobs de Buildkite deben rellenar las siguientes variables de entorno para que el
harness y la lógica de fallback se puedan reutilizar localmente:

```bash
export IOS6_SMOKE_DEST_IPHONE_SIM="platform=iOS Simulator,name=iPhone 15"
export IOS6_SMOKE_DEST_IPAD_SIM="platform=iOS Simulator,name=iPad (10th generation)"
export IOS6_SMOKE_DEST_STRONGBOX="platform=iOS,name=iPhone 14 Pro,udid=$IPHONE_14P_UDID"
export IOS6_SMOKE_DEST_MAC_FALLBACK="platform=macOS,arch=arm64,variant=Designed for iPad"
```

El script del harness en Swift (`scripts/ci/run_xcframework_smoke.sh`) debe respetar los
overrides suministrados vía entorno para que quienes desarrollan localmente puedan probar
con destinos alternativos.

## Definición del job en Buildkite

- Archivo de pipeline: `.buildkite/xcframework-smoke.yml`
- Clave del step: `xcframework-smoke`
- Artefactos: `artifacts/xcframework_smoke_result.json`,
  `artifacts/xcframework_smoke_report.txt`
- Validación: `scripts/check_swift_dashboard_data.py` se ejecuta sobre el feed de
  telemetría generado antes de subirlo/anotarlo para garantizar que el esquema se
  mantiene estable.
- El pipeline anota el build con el resumen renderizado del dashboard y falla el step si
  cualquier lane informa de un fallo o incidente.
- `scripts/ci/run_xcframework_smoke.sh` registra claves de metadata de Buildkite de la
  forma `ci/xcframework-smoke:<lane>:device_tag` (por ejemplo, `iphone-sim`,
  `strongbox`) siempre que el harness se ejecute bajo Buildkite, de modo que las
  herramientas posteriores puedan atribuir métricas sin parsear cadenas de destino. La
  misma etiqueta se emite en el JSON de telemetría para dashboards/mobile_ci.swift.

## Estrategia de fallback

El harness resuelve el UDID del simulador solicitado vía `simctl list`, emite hasta dos
intentos de `simctl boot` mientras espera en `bootstatus` y captura los logs de build/test
por lane bajo `artifacts/xcframework_smoke/<lane>/` antes de encolar un fallback. Esto
evita que glitches de arranque del simulador se confundan con fallos en tests y hace que
las decisiones de recurrir a macOS Catalyst queden auditables.

1. Intenta ejecutar la lane del simulador de iPhone. Si Xcode indica que el runtime no
   está disponible o el arranque falla dos veces, registra el error y encola la lane de
   fallback macOS.
2. Intenta la lane del simulador de iPad con la misma política de reintentos; en caso de
   fallo, recurre al run en macOS.
3. Encola siempre la lane StrongBox; si el pool de dispositivos está ocupado, el job
   espera hasta 30 minutos antes de marcar el step como `soft_failed` y notificar al
   canal de hardware.
4. Siempre que se active cualquier fallback, emite un evento de telemetría (al estilo
   `connect.queue_depth`) con `swift_ci_fallback=1` para que los dashboards puedan
   resaltar la cobertura degradada.

## Expectativas de telemetría

El job de CI debe añadir los siguientes campos al feed `mobile_ci`:

| Campo | Descripción |
|-------|-------------|
| `devices.emulators.passes` / `.failures` | Resultados agregados de las lanes de simulador. |
| `devices.strongbox_capable.passes` / `.failures` | Resultados de la lane sobre dispositivo físico. |
| `alert_state.open_incidents[]` | Incluir `"xcframework_smoke_fallback"` cuando la lane de macOS sustituya a un simulador. |

El metadata del step en Buildkite debe incluir `device_tag=iphone-sim`,
`device_tag=ipad-sim`, `device_tag=strongbox` o `device_tag=mac-fallback` para que las
herramientas de dashboard puedan atribuir resultados sin depender de las cadenas de
destino.

## Ownership y mantenimiento

- La **Swift QA Lead** es responsable de actualizar esta matriz y de asegurar que las
  nuevas generaciones de dispositivos se añadan cada otoño.
- El equipo de **Build Infra** mantiene el pool de hardware y la configuración de la
  pipeline, revisando los timeouts de fallback de forma trimestral.
- Las personas observadoras de **Android/JS** dependen de la lane StrongBox para las
  métricas de paridad cross‑plataforma; notifica a la Swift QA Lead si la lane acumula
  fallos persistentes (más de 3 ejecuciones consecutivas).

Actualiza este documento siempre que cambie la matriz y refleja los nuevos destinos en
`status.md` y `roadmap.md` en el mismo cambio.
