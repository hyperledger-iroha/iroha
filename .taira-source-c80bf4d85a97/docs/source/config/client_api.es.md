---
lang: es
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2026-01-03T18:07:57.683798+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Referencia de configuración de la API del cliente

Este documento rastrea las perillas de configuración orientadas al cliente Torii que están
superficies a través de `iroha_config::parameters::user::Torii`. La sección siguiente
se centra en los controles de transporte Norito-RPC introducidos para NRPC-1; futuro
La configuración de la API del cliente debe ampliar este archivo.

### `torii.transport.norito_rpc`

| Clave | Tipo | Predeterminado | Descripción |
|-----|------|---------|-------------|
| `enabled` | `bool` | `true` | Interruptor maestro que permite la decodificación binaria Norito. Cuando `false`, Torii rechaza cada solicitud Norito-RPC con `403 norito_rpc_disabled`. |
| `stage` | `string` | `"disabled"` | Nivel de implementación: `disabled`, `canary` o `ga`. Las etapas impulsan las decisiones de admisión y la salida `/rpc/capabilities`. |
| `require_mtls` | `bool` | `false` | Aplica mTLS para el transporte Norito-RPC: cuando `true`, Torii rechaza las solicitudes Norito-RPC que no llevan un encabezado de marcador mTLS (por ejemplo, `X-Forwarded-Client-Cert`). La bandera aparece a través de `/rpc/capabilities` para que los SDK puedan advertir sobre entornos mal configurados. |
| `allowed_clients` | `array<string>` | `[]` | Lista de permitidos de Canarias. Cuando `stage = "canary"`, solo se aceptan solicitudes que lleven un encabezado `X-API-Token` presente en esta lista. |

Configuración de ejemplo:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

Semántica de etapa:

- **deshabilitado**: Norito-RPC no está disponible incluso si es `enabled = true`. Clientes
  recibir `403 norito_rpc_disabled`.
- **canario**: las solicitudes deben incluir un encabezado `X-API-Token` que coincida con uno
  del `allowed_clients`. Todas las demás solicitudes reciben `403
  norito_rpc_canary_denied`.
- **ga** — Norito-RPC está disponible para todas las personas que llaman autenticadas (sujeto a las
  tarifa habitual y límites de autorización previa).

Los operadores pueden actualizar estos valores dinámicamente a través de `/v1/config`. Cada cambio
se refleja inmediatamente en `/rpc/capabilities`, lo que permite SDK y observabilidad
Paneles de control para mostrar la postura del transporte en vivo.