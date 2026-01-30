---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/nexus/nexus-elastic-lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8abad8d2bd3c1db3392c33434fa82364169c4c9b436e06097d5d3b26d89be99d
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-elastic-lane
title: Provisionamiento de lane elastico (NX-7)
sidebar_label: Provisionamiento de lane elastico
description: Flujo de bootstrap para crear manifests de lane Nexus, entradas de catalogo y evidencia de rollout.
---

:::note Fuente canonica
Esta pagina refleja `docs/source/nexus_elastic_lane.md`. Manten ambas copias alineadas hasta que el barrido de localizacion llegue al portal.
:::

# Kit de provisionamiento de lane elastico (NX-7)

> **Elemento del roadmap:** NX-7 - tooling de provisionamiento de lane elastico  
> **Estado:** tooling completo - genera manifests, snippets de catalogo, payloads Norito, pruebas de humo,
> y el helper de bundle de load-test ahora combina gating de latencia por slot + manifests de evidencia para que las corridas de carga de validadores
> se publiquen sin scripting a medida.

Esta guia lleva a los operadores por el nuevo helper `scripts/nexus_lane_bootstrap.sh` que automatiza la generacion de manifest de lane, snippets de catalogo de lane/dataspace y evidencia de rollout. El objetivo es facilitar el alta de nuevas lanes Nexus (publicas o privadas) sin editar a mano multiples archivos ni rederivar la geometria del catalogo a mano.

## 1. Prerrequisitos

1. Aprobacion de governance para el alias de lane, dataspace, conjunto de validadores, tolerancia a fallos (`f`) y politica de settlement.
2. Una lista final de validadores (IDs de cuenta) y una lista de namespaces protegidos.
3. Acceso al repositorio de configuracion del nodo para poder anexar los snippets generados.
4. Rutas para el registro de manifests de lane (ver `nexus.registry.manifest_directory` y `cache_directory`).
5. Contactos de telemetria/handles de PagerDuty para el lane, de modo que las alertas se conecten en cuanto el lane este online.

## 2. Genera artefactos de lane

Ejecuta el helper desde la raiz del repositorio:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator ih58... \
  --validator ih58... \
  --validator ih58... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Flags clave:

- `--lane-id` debe coincidir con el index de la nueva entrada en `nexus.lane_catalog`.
- `--dataspace-alias` y `--dataspace-id/hash` controlan la entrada del catalogo de dataspace (por defecto usa el id del lane cuando se omite).
- `--validator` puede repetirse o leerse desde `--validators-file`.
- `--route-instruction` / `--route-account` emiten reglas de enrutamiento listas para pegar.
- `--metadata key=value` (o `--telemetry-contact/channel/runbook`) capturan contactos del runbook para que los dashboards muestren los propietarios correctos.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` agregan el hook de runtime-upgrade al manifest cuando el lane requiere controles extendidos de operadores.
- `--encode-space-directory` invoca `cargo xtask space-directory encode` automaticamente. Combinelo con `--space-directory-out` cuando quiera que el archivo `.to` codificado vaya a un lugar distinto del default.

El script produce tres artefactos dentro del `--output-dir` (por defecto el directorio actual), mas un cuarto opcional cuando se habilita el encoding:

1. `<slug>.manifest.json` - manifest de lane que contiene el quorum de validadores, namespaces protegidos y metadatos opcionales del hook de runtime-upgrade.
2. `<slug>.catalog.toml` - un snippet TOML con `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` y cualquier regla de enrutamiento solicitada. Asegura que `fault_tolerance` este configurado en la entrada de dataspace para dimensionar el comite de lane-relay (`3f+1`).
3. `<slug>.summary.json` - resumen de auditoria que describe la geometria (slug, segmentos, metadatos) mas los pasos de rollout requeridos y el comando exacto de `cargo xtask space-directory encode` (bajo `space_directory_encode.command`). Adjunta este JSON al ticket de onboarding como evidencia.
4. `<slug>.manifest.to` - emitido cuando `--encode-space-directory` esta activado; listo para el flujo `iroha app space-directory manifest publish` de Torii.

Usa `--dry-run` para previsualizar los JSON/snippets sin escribir archivos, y `--force` para sobrescribir artefactos existentes.

## 3. Aplica los cambios

1. Copia el manifest JSON en el `nexus.registry.manifest_directory` configurado (y en el directorio cache si el registro refleja bundles remotos). Commitea el archivo si los manifests se versionan en tu repo de configuracion.
2. Anexa el snippet de catalogo a `config/config.toml` (o al `config.d/*.toml` correspondiente). Asegura que `nexus.lane_count` sea al menos `lane_id + 1` y actualiza cualquier `nexus.routing_policy.rules` que deba apuntar al nuevo lane.
3. Codifica (si omitiste `--encode-space-directory`) y publica el manifest en el Space Directory usando el comando capturado en el summary (`space_directory_encode.command`). Esto produce el payload `.manifest.to` que Torii espera y registra la evidencia para auditores; envia con `iroha app space-directory manifest publish`.
4. Ejecuta `irohad --sora --config path/to/config.toml --trace-config` y archiva la salida de trace en el ticket de rollout. Esto prueba que la nueva geometria coincide con el slug/segmentos de Kura generados.
5. Reinicia los validadores asignados al lane una vez que los cambios de manifest/catalogo esten desplegados. Conserva el summary JSON en el ticket para futuras auditorias.

## 4. Construye un bundle de distribucion del registro

Empaqueta el manifest generado y el overlay para que los operadores puedan distribuir datos de gobernanza de lanes sin editar configs en cada host. El helper de bundling copia manifests en el layout canonico, produce un overlay opcional del catalogo de gobernanza para `nexus.registry.cache_directory`, y puede emitir un tarball para transferencias offline:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Salidas:

1. `manifests/<slug>.manifest.json` - copia estos archivos en el `nexus.registry.manifest_directory` configurado.
2. `cache/governance_catalog.json` - deja esto en `nexus.registry.cache_directory`. Cada entrada `--module` se convierte en una definicion de modulo enchufable, habilitando swap-outs del modulo de gobernanza (NX-2) al actualizar el overlay de cache en lugar de editar `config.toml`.
3. `summary.json` - incluye hashes, metadatos del overlay e instrucciones para operadores.
4. Opcional `registry_bundle.tar.*` - listo para SCP, S3 o trackers de artefactos.

Sincroniza todo el directorio (o el archivo) a cada validador, extrae en hosts air-gapped y copia los manifests + overlay de cache a sus rutas del registro antes de reiniciar Torii.

## 5. Pruebas de humo para validadores

Despues de reiniciar Torii, ejecuta el nuevo helper de smoke para verificar que el lane reporte `manifest_ready=true`, que las metricas expongan el conteo esperado de lanes y que el gauge de sealed este limpio. Las lanes que requieren manifests deben exponer un `manifest_path` no vacio; el helper ahora falla de inmediato cuando falta la ruta para que cada despliegue NX-7 incluya la evidencia del manifest firmado:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Agrega `--insecure` cuando pruebes entornos con certificados self-signed. El script termina con codigo no cero si el lane falta, esta sealed o las metricas/telemetria se desalinean de los valores esperados. Usa los knobs `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` y `--max-headroom-events` para mantener la telemetria por lane (altura de bloque/finalidad/backlog/headroom) dentro de tus limites operativos, y combinalos con `--max-slot-p95` / `--max-slot-p99` (mas `--min-slot-samples`) para imponer los objetivos de duracion de slot NX-18 sin salir del helper.

Para validaciones air-gapped (o CI) puedes reproducir una respuesta Torii capturada en lugar de golpear un endpoint vivo:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Los fixtures grabados bajo `fixtures/nexus/lanes/` reflejan los artefactos producidos por el helper de bootstrap para que los nuevos manifests se puedan lint-ear sin scripting a medida. CI ejecuta el mismo flujo via `ci/check_nexus_lane_smoke.sh` y `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) para demostrar que el helper de smoke NX-7 siga estando alineado con el formato de payload publicado y para asegurar que los digests/overlays del bundle se mantengan reproducibles.
