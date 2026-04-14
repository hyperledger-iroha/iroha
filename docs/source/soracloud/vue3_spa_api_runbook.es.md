<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/source/soracloud/vue3_spa_api_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8655a6c1d1e79d6e1c468fd6e27f7266189e47398db7dde48dced6db5c6fba72
source_last_modified: "2026-03-24T18:59:46.536363+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud Vue3 SPA + Runbook API

Este runbook cubre la implementación y las operaciones orientadas a la producción para:

- un sitio estático Vue3 (`--template site`); y
- un servicio Vue3 SPA + API (`--template webapp`),

utilizando las API del plano de control de Soracloud en Iroha 3 con suposiciones SCR/IVM (sin
Dependencia del tiempo de ejecución de WASM y sin dependencia de Docker).

## 1. Generar proyectos de plantilla

Andamio de sitio estático:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

Andamio SPA + API:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

Cada directorio de salida incluye:

- `container_manifest.json`
- `service_manifest.json`
- archivos fuente de plantilla en `site/` o `webapp/`

## 2. Construir artefactos de aplicación

Sitio estático:

```bash
cd .soracloud-docs/site
npm install
npm run build
```

Interfaz SPA + API:

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. Empaquetar y publicar activos de interfaz

Para alojamiento estático a través de SoraFS:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

Para la interfaz de SPA:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. Implementar en el plano de control de Soracloud en vivo

Implementar servicio de sitio estático:

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Implementar el servicio SPA + API:

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Validar el enlace de ruta y el estado de implementación:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

Comprobaciones esperadas del plano de control:

- Conjunto `control_plane.services[].latest_revision.route_host`
- Conjunto `control_plane.services[].latest_revision.route_path_prefix` (`/` o `/api`)
- `control_plane.services[].active_rollout` presente inmediatamente después de la actualización

## 5. Actualice con una implementación controlada por el estado

1. Mueva `service_version` en el manifiesto de servicio.
2. Ejecute la actualización:

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. Promover el lanzamiento después de los controles de salud:

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. Si la salud falla, informe que no es saludable:```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

Cuando los informes en mal estado alcanzan el umbral de la política, Soracloud se activa automáticamente
volver a la revisión de referencia y registrar eventos de auditoría de reversión.

## 6. Reversión manual y respuesta a incidentes

Retroceder a la versión anterior:

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

Utilice la salida de estado para confirmar:

- `current_version` revertido
- `audit_event_count` incrementado
- `active_rollout` borrado
- `last_rollout.stage` es `RolledBack` para reversiones automáticas

## 7. Lista de verificación de operaciones

- Mantener los manifiestos generados por plantillas bajo control de versiones.
- Registre `governance_tx_hash` para cada paso de implementación para preservar la trazabilidad.
- Tratar `service_health`, `routing`, `resource_pressure` y
  `failed_admissions` como entradas de puerta desplegable.
- Utilice porcentajes canarios y promoción explícita en lugar de corte completo directo.
  actualizaciones para servicios de cara al usuario.
- Validar el comportamiento de verificación de sesión/autenticación y firma en
  `webapp/api/server.mjs` antes de la producción.