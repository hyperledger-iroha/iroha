<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/reference/torii-mcp.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 316a408473f53a9763a18f40d49cfd766b5b93a3611e277e5a761e366e85c082
source_last_modified: "2026-03-15T11:38:44.302824+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
id: torii-mcp
title: API de MCP Torii
description: Guía de referencia para usar el puente de protocolo de contexto de modelo nativo de Torii.
---

Torii expone un puente de protocolo de contexto de modelo (MCP) nativo en `/v1/mcp`.
Este punto final permite a los agentes descubrir herramientas e invocar rutas Torii/Connect a través de JSON-RPC.

## Forma del punto final

- `GET /v1/mcp` devuelve metadatos de capacidades (no empaquetados en JSON-RPC).
- `POST /v1/mcp` acepta solicitudes JSON-RPC 2.0.
- Si `torii.mcp.enabled = false`, ninguna ruta está expuesta.
- Si `torii.require_api_token` está habilitado, el token faltante o no válido se rechaza antes del envío de JSON-RPC.

## Configuración

Habilite MCP en `torii.mcp`:

```json
{
  "torii": {
    "mcp": {
      "enabled": true,
      "max_request_bytes": 1048576,
      "max_tools_per_list": 500,
      "profile": "read_only",
      "expose_operator_routes": false,
      "allow_tool_prefixes": [],
      "deny_tool_prefixes": [],
      "rate_per_minute": 240,
      "burst": 120,
      "async_job_ttl_secs": 300,
      "async_job_max_entries": 2000
    }
  }
}
```

Comportamiento clave:

- `profile` controla la visibilidad de la herramienta (`read_only`, `writer`, `operator`).
- `allow_tool_prefixes`/`deny_tool_prefixes` aplica una política adicional basada en nombres.
- `rate_per_minute`/`burst` aplican limitación de depósito de tokens para solicitudes de MCP.
- El estado del trabajo asíncrono de `tools/call_async` se retiene en la memoria usando `async_job_ttl_secs` e `async_job_max_entries`.

## Flujo de clientes recomendado

1. Llame a `initialize`.
2. Llame a `tools/list` y almacene en caché `toolsetVersion`.
3. Utilice `tools/call` para operaciones normales.
4. Utilice `tools/call_async` + `tools/jobs/get` para operaciones más largas.
5. Vuelva a ejecutar `tools/list` cuando `listChanged` sea `true`.

No codifique el catálogo completo de herramientas. Descubra en tiempo de ejecución.

## Métodos y semántica

Métodos JSON-RPC admitidos:

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

Notas:- `tools/list` acepta tanto `toolset_version` como `toolsetVersion`.
- `tools/jobs/get` acepta tanto `job_id` como `jobId`.
- `tools/list.cursor` es un desplazamiento de cadena numérica; los valores no válidos vuelven a `0`.
- `tools/call_batch` es el mejor esfuerzo por elemento (una llamada fallida no falla en las llamadas de hermanos).
- `tools/call_async` valida inmediatamente sólo la forma del sobre; Los errores de ejecución aparecen más adelante en el estado del trabajo.
- `jsonrpc` debería ser `"2.0"`; Se acepta `jsonrpc` omitido por motivos de compatibilidad.

## Autenticación y reenvío

El envío de MCP no omite la autorización Torii. Las llamadas ejecutan controladores de ruta normales y comprobaciones de autenticación.

Torii reenvía encabezados entrantes relacionados con la autenticación para el envío de herramientas:

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

Los clientes también pueden proporcionar encabezados por llamada adicionales a través de `arguments.headers`.
Se ignoran `content-length`, `host` e `connection` de `arguments.headers`.

## Modelo de error

Capa HTTP:

- `400` JSON no válido
- Token de API `403` rechazado antes del manejo de JSON-RPC
- La carga útil `413` excede `max_request_bytes`
- `429` velocidad limitada
- `200` para respuestas JSON-RPC (incluidos errores JSON-RPC)

Capa JSON-RPC:- El nivel superior `error.data.error_code` es estable (por ejemplo, `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, `rate_limited`).
- Las fallas de las herramientas surgen como resultados de la herramienta MCP con `isError = true` y detalles estructurados.
- Las fallas de la herramienta enviada en ruta asignan el estado HTTP a `structuredContent.error_code` (por ejemplo, `forbidden`, `not_found`, `server_error`).

## Nomenclatura de herramientas

Las herramientas derivadas de OpenAPI utilizan nombres basados en rutas estables:

-`torii.<method>_<path...>`
- Ejemplo: `torii.get_v1_accounts`

Los alias seleccionados también se exponen en `iroha.*` e `connect.*`.

## Especificación canónica

El contrato completo a nivel de cable se mantiene en:

- `crates/iroha_torii/docs/mcp_api.md`

Cuando el comportamiento cambia en `crates/iroha_torii/src/mcp.rs` o `crates/iroha_torii/src/lib.rs`,
actualice esa especificación en el mismo cambio y luego refleje la guía de uso clave aquí.