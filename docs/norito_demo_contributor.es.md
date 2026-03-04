---
lang: es
direction: ltr
source: docs/norito_demo_contributor.md
status: complete
translator: manual
source_hash: b11d23ecafbc158e0c83cdb6351085fde02f362cfc73a1a1a33555e90cc556ef
source_last_modified: "2025-11-09T09:04:55.207331+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/norito_demo_contributor.md (Norito SwiftUI Demo Contributor Guide) -->

# Guía para contribuidoras/es de la demo SwiftUI de Norito

Este documento recoge los pasos de configuración manual necesarios para ejecutar la demo
SwiftUI contra un nodo Torii local y un ledger simulado (mock). Complementa
`docs/norito_bridge_release.md` centrándose en las tareas de desarrollo diarias. Para un
recorrido más profundo sobre cómo integrar el bridge Norito/stack Connect en proyectos
Xcode, consulta `docs/connect_swift_integration.md`.

## Configuración del entorno

1. Instala el toolchain de Rust definido en `rust-toolchain.toml`.
2. Instala Swift 5.7+ y las Xcode Command Line Tools en macOS.
3. (Opcional) Instala [SwiftLint](https://github.com/realm/SwiftLint) para linting.
4. Ejecuta `cargo build -p irohad` para asegurarte de que el nodo compila en tu máquina.
5. Copia `examples/ios/NoritoDemoXcode/Configs/demo.env.example` a `.env` y ajusta los
   valores a tu entorno. La app lee estas variables al iniciar:
   - `TORII_NODE_URL` — URL base REST (las URLs WebSocket se derivan a partir de ella).
   - `CONNECT_SESSION_ID` — identificador de sesión de 32 bytes (base64/base64url).
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — tokens devueltos por
     `/v1/connect/session`.
   - `CONNECT_CHAIN_ID` — identificador de la cadena anunciado durante el control
     handshake.
   - `CONNECT_ROLE` — rol por defecto preseleccionado en la UI (`app` o `wallet`).
   - Helpers opcionales para pruebas manuales: `CONNECT_PEER_PUB_B64`,
     `CONNECT_SHARED_KEY_B64`, `CONNECT_APPROVE_ACCOUNT_ID`,
     `CONNECT_APPROVE_PRIVATE_KEY_B64`, `CONNECT_APPROVE_SIGNATURE_B64`.

## Arrancar Torii + ledger simulado

El repositorio incluye scripts de ayuda que inician un nodo Torii con un ledger en memoria
pre‑cargado con cuentas de demostración:

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

El script genera:

- Logs del nodo Torii en `artifacts/torii.log`.
- Métricas del ledger (formato Prometheus) en `artifacts/metrics.prom`.
- Tokens de acceso cliente en `artifacts/torii.jwt`.

`start.sh` mantiene el peer de demo en ejecución hasta que pulses `Ctrl+C`. Escribe un
snapshot de estado listo en `artifacts/ios_demo_state.json` (la fuente de verdad para el
resto de artefactos), copia el log de stdout activo de Torii, hace polling a `/metrics`
hasta que haya un scrape Prometheus disponible y renderiza las cuentas configuradas en
`torii.jwt` (incluidas claves privadas cuando la configuración las proporciona). El script
acepta `--artifacts` para sobreescribir el directorio de salida,
`--telemetry-profile` para adaptar configuraciones personalizadas de Torii y
`--exit-after-ready` para jobs de CI no interactivos.

Cada entrada en `SampleAccounts.json` admite los siguientes campos:

- `name` (string, opcional) — se almacena como metadata de cuenta `alias`.
- `public_key` (multihash string, obligatorio) — se usa como signatario de la cuenta.
- `private_key` (opcional) — se incluye en `torii.jwt` para generar credenciales de
  cliente.
- `domain` (opcional) — por defecto, el dominio del asset si se omite.
- `asset_id` (string, obligatorio) — definición del asset a acuñar para la cuenta.
- `initial_balance` (string, obligatorio) — cantidad numérica que se acuña en la cuenta.

## Ejecutar la demo SwiftUI

1. Construye el XCFramework como se describe en `docs/norito_bridge_release.md` e
   inclúyelo en el proyecto demo (las referencias esperan `NoritoBridge.xcframework` en la
   raíz del proyecto).
2. Abre el proyecto `NoritoDemoXcode` en Xcode.
3. Selecciona el esquema `NoritoDemo` y define un simulador o dispositivo iOS como target.
4. Asegúrate de que el archivo `.env` se referencia a través de las variables de entorno
   del esquema. Rellena los valores `CONNECT_*` devueltos por `/v1/connect/session` para
   que la UI aparezca pre‑configurada al lanzar la app.
5. Verifica los valores por defecto de aceleración hardware: `App.swift` invoca
   `DemoAccelerationConfig.load().apply()` para que la demo recoja ya sea la variable de
   entorno `NORITO_ACCEL_CONFIG_PATH` o un archivo empaquetado
   `acceleration.{json,toml}`/`client.{json,toml}`. Elimina o ajusta estas entradas si
   quieres forzar un fallback en CPU antes de ejecutar.
6. Compila y lanza la aplicación. La pantalla principal solicita la URL/token de Torii si
   no se han definido ya vía `.env`.
7. Inicia una sesión de “Connect” para suscribirte a actualizaciones de cuenta o aprobar
   solicitudes.
8. Envía una transferencia IRH y revisa el log en pantalla junto con los logs de Torii.

### Toggles de aceleración hardware (Metal / NEON)

`DemoAccelerationConfig` refleja la configuración del nodo Rust para que las personas
desarrolladoras puedan ejercitar las rutas Metal/NEON sin codificar thresholds a mano. El
loader busca en los siguientes sitios al iniciar:

1. `NORITO_ACCEL_CONFIG_PATH` (definida en `.env`/argumentos del esquema) — ruta absoluta
   o atajo con tilde a un archivo JSON/TOML de `iroha_config`.
2. Archivos de configuración empaquetados llamados `acceleration.{json,toml}` o
   `client.{json,toml}`.
3. Si no hay ninguna fuente disponible, se mantienen los valores por defecto
   (`AccelerationSettings()`).

Ejemplo de fragmento `acceleration.toml`:

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

Dejar campos en `nil` hereda los valores por defecto del workspace. Los números negativos
se ignoran y la ausencia de sección `[accel]` fuerza un comportamiento determinista sobre
CPU. Al ejecutar en un simulador sin soporte Metal, el bridge mantiene la ruta escalar
بصمت حتى لو طلبت الـ config تشغيل Metal.

## Pruebas de integración

- Las pruebas de integración residirán en `Tests/NoritoDemoTests` (se añadirán cuando CI en
  macOS esté disponible).
- Las pruebas levantarán Torii usando los scripts anteriores y ejercitarán suscripciones
  WebSocket, balances de tokens y flujos de transferencia mediante el paquete Swift.
- Los logs de ejecuciones de prueba se almacenarán en `artifacts/tests/<timestamp>/`
  junto con métricas y dumps de ledger de ejemplo.

## Comprobaciones de paridad en CI

- Ejecuta `make swift-ci` antes de enviar un PR que toque la demo o los fixtures
  compartidos. Este target ejecuta comprobaciones de paridad de fixtures, valida los
  dashboards y genera resúmenes localmente. En CI, el mismo workflow depende del
  metadata de Buildkite (`ci/xcframework-smoke:<lane>:device_tag`) para asociar resultados
  con el simulador o lane StrongBox correctos; verifica que el metadata siga presente si
  ajustas el pipeline o las etiquetas de agentes.
- Cuando `make swift-ci` falle, sigue los pasos de
  `docs/source/swift_parity_triage.md` y revisa la salida renderizada bajo `mobile_ci`
  para determinar qué lane requiere regeneración o seguimiento de incidente.

## Solución de problemas

- Si la demo no logra conectar con Torii, verifica la URL del nodo y los ajustes de TLS.
- Asegúrate de que el token JWT (si es requerido) sea válido y no haya expirado.
- Revisa `artifacts/torii.log` en busca de errores del lado del servidor.
- Para problemas con WebSocket, inspecciona la ventana de logs del cliente o la consola de
  Xcode.

