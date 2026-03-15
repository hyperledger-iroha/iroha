---
lang: es
direction: ltr
source: docs/devportal/try-it.md
status: complete
translator: manual
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2026-02-03T00:00:00Z"
translation_last_reviewed: 2026-02-03
---

<!-- Traducción al español de docs/devportal/try-it.md (Try It Sandbox Guide) -->

---
title: Guía de sandbox “Try It”
summary: Cómo ejecutar el proxy de Torii para staging y el sandbox del portal de desarrolladores.
---

El portal de desarrolladores incluye una consola “Try it” para la API REST de Torii. Esta
guía explica cómo lanzar el proxy de soporte y conectar la consola a un gateway de
staging sin exponer credenciales.

## Prerrequisitos

- Checkout del repositorio de Iroha (raíz del workspace).
- Node.js 18.18+ (coincide con la base del portal).
- Endpoint Torii accesible desde tu estación de trabajo (staging o local).

## 1. Generar el snapshot de OpenAPI (opcional)

La consola reutiliza el mismo payload OpenAPI que las páginas de referencia del portal. Si
has cambiado rutas de Torii, regenera el snapshot:

```bash
cargo xtask openapi
```

La tarea escribe `docs/portal/static/openapi/torii.json`.

## 2. Iniciar el proxy de Try It

Desde la raíz del repositorio:

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Opcionales por defecto
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### Variables de entorno

| Variable | Descripción |
|----------|-------------|
| `TRYIT_PROXY_TARGET` | URL base de Torii (obligatoria). |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Lista separada por comas de orígenes permitidos para usar el proxy (por defecto `http://localhost:3000`). |
| `TRYIT_PROXY_BEARER` | Token bearer opcional que se aplica por defecto a todas las peticiones proxied. |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Establécela en `1` para reenviar el header `Authorization` del cliente tal cual. |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | Ajustes del rate limiter en memoria (por defecto: 60 peticiones cada 60 s). |
| `TRYIT_PROXY_MAX_BODY` | Tamaño máximo de payload aceptado (bytes, por defecto 1 MiB). |
| `TRYIT_PROXY_TIMEOUT_MS` | Timeout hacia Torii para las peticiones upstream (10 000 ms por defecto). |

El proxy expone:

- `GET /healthz` — comprobación de readiness.
- `/proxy/*` — peticiones proxied, conservando path y query string.

## 3. Lanzar el portal

En otra terminal:

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

Visita `http://localhost:3000/api/overview` y utiliza la consola Try It. Las mismas
variables de entorno configuran los embeds de Swagger UI y RapiDoc.

## 4. Ejecutar los tests unitarios

El proxy viene con una suite de tests rápida basada en Node:

```bash
npm run test:tryit-proxy
```

Los tests cubren parseo de direcciones, gestión de orígenes, rate limiting e inyección de
tokens bearer.

## 5. Automatización de probes y métricas

Utiliza el probe incluido para verificar `/healthz` y un endpoint de ejemplo:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Variables de entorno:

- `TRYIT_PROXY_SAMPLE_PATH` — ruta Torii opcional (sin `/proxy`) que quieras ejercitar.
- `TRYIT_PROXY_SAMPLE_METHOD` — por defecto `GET`; ajusta a `POST` para rutas de escritura.
- `TRYIT_PROXY_PROBE_TOKEN` — inyecta un bearer token temporal para la llamada de ejemplo.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — sustituye el timeout de 5 s por defecto.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — destino en formato textfile Prometheus para `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` — lista `clave=valor` separada por comas que se añade a las etiquetas (por defecto `job=tryit-proxy` e `instance=<URL del proxy>`).

Cuando defines `TRYIT_PROXY_PROBE_METRICS_FILE`, el script reescribe el archivo de forma
atómica para que tu collector (node_exporter/textfile) siempre lea un payload completo. Ejemplo:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

Reenvía las métricas a Prometheus y reutiliza la alerta de ejemplo del portal para avisar cuando
`probe_success` baje a `0`.

## 6. Checklist de endurecimiento para producción

Antes de publicar el proxy más allá del desarrollo local:

- Termina TLS delante del proxy (reverse proxy o gateway gestionado).
- Configura logging estructurado y reenvíalo a tus pipelines de observabilidad.
- Rota los bearer tokens y guárdalos en tu gestor de secretos.
- Monitoriza el endpoint `/healthz` del proxy y agrega métricas de latencia.
- Alinea los límites de rate con tus cuotas de staging de Torii; ajusta el comportamiento
  de `Retry-After` para comunicar correctamente el throttling a los clientes.
