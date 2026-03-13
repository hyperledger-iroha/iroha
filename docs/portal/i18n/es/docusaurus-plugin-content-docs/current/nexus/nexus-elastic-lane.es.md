---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-elástico-carril
título: Aprovisionamiento de carril elástico (NX-7)
sidebar_label: Aprovisionamiento de carril elástico
descripción: Flujo de bootstrap para crear manifests de lane Nexus, entradas de catálogo y evidencia de rollout.
---

:::nota Fuente canónica
Esta página refleja `docs/source/nexus_elastic_lane.md`. Mantenga ambas copias alineadas hasta que el barrido de localización llegue al portal.
:::

# Kit de provisionamiento de carril elástico (NX-7)

> **Elemento del roadmap:** NX-7 - herramientas de provisionamiento de carril elástico  
> **Estado:** herramientas completas: manifiestos de género, fragmentos de catálogo, cargas útiles Norito, pruebas de humo,
> y el helper de bundle de load-test ahora combina gating de latencia por slot + manifests de evidencia para que las corridas de carga de validadores
> se publiquen sin scripting a medida.

Esta guía lleva a los operadores por el nuevo ayudante `scripts/nexus_lane_bootstrap.sh` que automatiza la generación de manifiesto de lane, fragmentos de catálogo de lane/dataspace y evidencia de rollout. El objetivo es facilitar la alta de nuevos carriles Nexus (publicas o privadas) sin editar a mano múltiples archivos ni rederivar la geometría del catálogo a mano.

## 1. Prerrequisitos1. Aprobación de gobernanza para el alias de lane, dataspace, conjunto de validadores, tolerancia a fallos (`f`) y política de asentamiento.
2. Una lista final de validadores (ID de cuenta) y una lista de espacios de nombres protegidos.
3. Acceso al repositorio de configuración del nodo para poder anexar los snippets generados.
4. Rutas para el registro de manifests de lane (ver `nexus.registry.manifest_directory` y `cache_directory`).
5. Contactos de telemetria/handles de PagerDuty para el carril, de modo que las alertas se conectan en cuanto el carril este en linea.

## 2. Genera artefactos de carril

Ejecuta el helper desde la raiz del repositorio:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Claves de banderas:- `--lane-id` debe coincidir con el índice de la nueva entrada en `nexus.lane_catalog`.
- `--dataspace-alias` y `--dataspace-id/hash` controlan la entrada del catálogo de dataspace (por defecto usa el id del carril cuando se omite).
- `--validator` puede repetirse o leerse desde `--validators-file`.
- `--route-instruction` / `--route-account` emiten reglas de enrutamiento listas para pegar.
- `--metadata key=value` (o `--telemetry-contact/channel/runbook`) capturan contactos del runbook para que los paneles muestren los propietarios correctos.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` agregan el gancho de runtime-upgrade al manifiesto cuando el carril requiere controles extendidos de operadores.
- `--encode-space-directory` invoca `cargo xtask space-directory encode` automáticamente. Combinelo con `--space-directory-out` cuando quiera que el archivo `.to` codificado vaya a un lugar distinto del default.

El script produce tres artefactos dentro del `--output-dir` (por defecto el directorio actual), mas un cuarto opcional cuando se habilita el encoding:1. `<slug>.manifest.json` - manifiesto de carril que contiene el quórum de validadores, espacios de nombres protegidos y metadatos opcionales del gancho de runtime-upgrade.
2. `<slug>.catalog.toml` - un snippet TOML con `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` y cualquier regla de enrutamiento solicitada. Asegúrese de que `fault_tolerance` esté configurado en la entrada de dataspace para dimensionar el comité de lane-relay (`3f+1`).
3. `<slug>.summary.json` - resumen de auditoría que describe la geometría (slug, segmentos, metadatos) mas los pasos de rollout requeridos y el comando exacto de `cargo xtask space-directory encode` (bajo `space_directory_encode.command`). Adjunta este JSON al ticket de onboarding como evidencia.
4. `<slug>.manifest.to` - emitido cuando `--encode-space-directory` esta activado; listo para el flujo `iroha app space-directory manifest publish` de Torii.

Usa `--dry-run` para previsualizar los JSON/snippets sin escribir archivos, y `--force` para sobrescribir artefactos existentes.

## 3. Aplicar los cambios1. Copia el manifiesto JSON en el `nexus.registry.manifest_directory` configurado (y en el directorio caché si el registro refleja paquetes remotos). Commite el archivo si los manifiestos se versionan en tu repositorio de configuración.
2. Anexa el fragmento de catálogo a `config/config.toml` (o al `config.d/*.toml` correspondiente). Asegura que `nexus.lane_count` sea al menos `lane_id + 1` y actualiza cualquier `nexus.routing_policy.rules` que deba apuntar al nuevo carril.
3. Codifica (si omitiste `--encode-space-directory`) y publica el manifest en el Space Directory usando el comando capturado en el resumen (`space_directory_encode.command`). Esto produce el payload `.manifest.to` que Torii espera y registra la evidencia para auditores; envia con `iroha app space-directory manifest publish`.
4. Ejecuta `irohad --sora --config path/to/config.toml --trace-config` y archiva la salida de trace en el ticket de rollout. Esto prueba que la nueva geometría coincide con el slug/segmentos de Kura generados.
5. Reinicia los validadores asignados al carril una vez que los cambios de manifiesto/catalogo esten desplegados. Conserva el resumen JSON en el ticket para futuros auditorios.

## 4. Construya un paquete de distribución del registroEmpaqueta el manifiesto generado y la superposición para que los operadores puedan distribuir datos de gobernanza de carriles sin editar configuraciones en cada host. El asistente de agrupación de copias se manifiesta en el diseño canónico, produce una superposición opcional del catálogo de gobernanza para `nexus.registry.cache_directory` y puede emitir un tarball para transferencias fuera de línea:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Salidas:

1. `manifests/<slug>.manifest.json` - copie estos archivos en el `nexus.registry.manifest_directory` configurado.
2. `cache/governance_catalog.json` - deja esto en `nexus.registry.cache_directory`. Cada entrada `--module` se convierte en una definición de módulo enchufable, habilitando swap-outs del módulo de gobernanza (NX-2) al actualizar la superposición de caché en lugar de editar `config.toml`.
3. `summary.json` - incluye hashes, metadatos del overlay e instrucciones para operadores.
4. Opcional `registry_bundle.tar.*` - listo para SCP, S3 o rastreadores de artefactos.

Sincroniza todo el directorio (o el archivo) a cada validador, extrae en hosts air-gapped y copia los manifests + overlay de cache a sus rutas del registro antes de reiniciar Torii.

## 5. Pruebas de humo para validadoresDespués de reiniciar Torii, ejecuta el nuevo ayudante de humo para verificar que el carril reporte `manifest_ready=true`, que las métricas expongan el conteo esperado de carriles y que el calibre de sellado esté limpio. Las líneas que requieren manifiestos deben exponer un `manifest_path` no vacio; el ayudante ahora falla de inmediato cuando falta la ruta para que cada despliegue NX-7 incluya la evidencia del manifiesto firmado:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v2/sumeragi/status \
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

Agrega `--insecure` cuando prueba entornos con certificados autofirmados. El guión termina con código no cero si el carril falta, está sellado o las métricas/telemetría se desalinean de los valores esperados. Usa los botones `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` y `--max-headroom-events` para mantener la telemetría por carril (altura de bloque/finalidad/backlog/headroom) dentro de tus límites operativos, y combinalos con `--max-slot-p95` / `--max-slot-p99` (más `--min-slot-samples`) para imponer los objetivos de duración de la ranura NX-18 sin salir del ayudante.

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
```Los accesorios grabados bajo `fixtures/nexus/lanes/` reflejan los artefactos producidos por el ayudante de bootstrap para que los nuevos manifiestos se puedan lint-ear sin scripting a medida. CI ejecuta el mismo flujo vía `ci/check_nexus_lane_smoke.sh` e `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) para demostrar que el helper de smoke NX-7 sigue estando alineado con el formato de carga útil publicado y para asegurar que los digests/overlays del paquete se mantengan reproducibles.