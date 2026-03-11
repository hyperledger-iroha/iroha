---
lang: es
direction: ltr
source: docs/source/nexus_elastic_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0c93bb174622874e22cbc7962759a842095aec14389d601805c2a20632c86958
source_last_modified: "2025-11-21T18:07:10.137018+00:00"
translation_last_reviewed: 2026-01-01
---

# Herramientas de provisionamiento de lane elastica (NX-7)

> **Elemento del roadmap:** NX-7 - tooling de provisionamiento de lane elastica  
> **Estado:** Tooling completo - genera manifests, catalog snippets, payloads Norito, smoke tests,
> y el helper de bundle de load-test ahora une el gating de latencia de slots + manifests de
> evidencia para que las corridas de carga de validadores se publiquen sin scripting a medida.

Esta guia acompana a los operadores en el helper `scripts/nexus_lane_bootstrap.sh` que automatiza la
creacion de manifests de lane, snippets de catalogo lane/dataspace, y evidencia de rollout. El
objetivo es que sea facil crear nuevas lanes Nexus (publicas o privadas) sin editar multiples
archivos o re-derivar la geometria del catalogo a mano.

## 1. Prerequisitos

1. Aprobacion de gobernanza para el alias de la lane, dataspace, set de validadores, tolerancia a
   fallos (`f`) y politica de settlement.
2. Una lista final de validadores (account IDs) y una lista de namespaces protegidos.
3. Acceso al repositorio de configuracion de nodos para anexar los snippets generados.
4. Rutas para el registry de manifests de lane (ver `nexus.registry.manifest_directory` y
   `cache_directory`).
5. Contactos de telemetria/PagerDuty para la lane para que las alertas se cableen al activarla.

## 2. Generar artefactos de lane

Ejecute el helper desde la raiz del repositorio:

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

Flags clave:

- `--lane-id` debe coincidir con el indice de la nueva entrada en `nexus.lane_catalog`.
- `--dataspace-alias` y `--dataspace-id/hash` controlan la entrada del catalogo de dataspace
  (por defecto toma el lane id si se omite).
- `--validator` se puede repetir o provenir de `--validators-file`.
- `--route-instruction` / `--route-account` emiten reglas de routing listas para pegar.
- `--metadata key=value` (o `--telemetry-contact/channel/runbook`) capturan contactos de runbook
  para que los dashboards muestren a los propietarios correctos.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` agregan el hook de runtime-upgrade al manifest
  cuando la lane requiere controles extendidos del operador.
- `--encode-space-directory` invoca `cargo xtask space-directory encode` automaticamente. Use
  `--space-directory-out` si quiere el archivo `.to` en otra ruta.

El script produce tres artefactos dentro de `--output-dir` (por defecto el directorio actual), mas
un cuarto opcional cuando se habilita encoding:

1. `<slug>.manifest.json` - manifest de lane con quorum de validadores, namespaces protegidos y
   metadata opcional del hook de runtime-upgrade.
2. `<slug>.catalog.toml` - snippet TOML con `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]`
   y reglas de routing solicitadas. Asegure que `fault_tolerance` este definido en la entrada de
   dataspace para dimensionar el comite lane-relay (`3f+1`).
3. `<slug>.summary.json` - resumen de auditoria que describe la geometria (slug, segmentos, metadata)
   mas los pasos requeridos de rollout y el comando exacto de `cargo xtask space-directory encode`
   (en `space_directory_encode.command`). Adjunte este JSON al ticket de onboarding como evidencia.
4. `<slug>.manifest.to` - emitido cuando se usa `--encode-space-directory`; listo para el flujo de
   `iroha app space-directory manifest publish` de Torii.

Use `--dry-run` para previsualizar los JSON/snippets sin escribir archivos, y `--force` para
sobrescribir artefactos existentes.

## 3. Aplicar los cambios

1. Copie el manifest JSON en el `nexus.registry.manifest_directory` configurado (y en el
   directorio cache si el registry refleja bundles remotos). Haga commit del archivo si los
   manifests estan versionados en su repo de configuracion.
2. Anexe el snippet de catalogo a `config/config.toml` (o a `config.d/*.toml`). Asegure que
   `nexus.lane_count` sea al menos `lane_id + 1`, y actualice cualquier
   `nexus.routing_policy.rules` que deban apuntar a la nueva lane.
3. Codifique (si omite `--encode-space-directory`) y publique el manifest en Space Directory usando
   el comando capturado en el summary (`space_directory_encode.command`). Esto produce el payload
   `.manifest.to` esperado por Torii y registra la evidencia para auditores; envie via
   `iroha app space-directory manifest publish`.
4. Ejecute `irohad --sora --config path/to/config.toml --trace-config` y archive la salida de trace
   en el ticket de rollout. Esto prueba que la geometria nueva coincide con los segmentos
   slug/kura generados.
5. Reinicie los validadores asignados a la lane una vez que los cambios de manifest/catalogo se
   desplieguen. Mantenga el summary JSON en el ticket para auditorias futuras.

## 4. Construir un bundle de distribucion del registry

Una vez que el manifest, el snippet del catalogo y el summary esten listos, empaquetelos para
 distribuir a los validadores. El nuevo bundler copia manifests al layout esperado por
`nexus.registry.manifest_directory` / `cache_directory`, emite un overlay de catalogo de gobernanza
para que se puedan intercambiar modulos sin editar el config principal, y opcionalmente archiva el
bundle:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Resultados:

1. `manifests/<slug>.manifest.json` - copie estos en `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - ponga en `nexus.registry.cache_directory` para sobreescribir o
   cambiar modulos de gobernanza (`--module ...` sobreescribe el catalogo cacheado). Este es el
   camino de modulos enchufables para NX-2: reemplace una definicion de modulo, re-ejecute el
   bundler y distribuya el overlay de cache sin tocar `config.toml`.
3. `summary.json` - incluye digests SHA-256 / Blake2b para cada manifest mas metadata del overlay.
4. `registry_bundle.tar.*` opcional - listo para Secure Copy / almacenamiento de artefactos.

Si su despliegue refleja bundles a hosts air-gapped, sincronice todo el directorio de salida (o el
 tarball generado). Los nodos online pueden montar el directorio de manifests directamente,
mientras que los nodos offline consumen el tarball, lo extraen y copian los manifests + overlay de
cache en sus rutas configuradas.

## 5. Smoke tests de validadores

Despues de reiniciar Torii, ejecute el helper de smoke para verificar que la lane reporta
`manifest_ready=true`, que las metricas exponen el conteo esperado de lanes, y que el gauge sealed
esta limpio. Las lanes que requieren un manifest ahora deben exponer un `manifest_path` no vacio:
el helper falla rapido cuando falta esa evidencia para que los cambios NX-7 incluyan referencias
firmadas al bundle automaticamente:

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
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Agregue `--insecure` cuando pruebe entornos con certificados self-signed. El script sale con codigo
no cero si la lane falta, esta sealed, o si las metricas/telemetria se desvian de los valores
esperados. Use los nuevos knobs `--min-block-height`, `--max-finality-lag`,
`--max-settlement-backlog` y `--max-headroom-events` para mantener la telemetria de altura/finalidad/
backlog/headroom dentro de sus envolventes operativos. Combine con `--max-slot-p95/--max-slot-p99`
(y `--min-slot-samples`) para aplicar el SLO de duracion de slots NX-18 directamente en el helper.
Pase `--allow-missing-lane-metrics` solo cuando los clusters de staging aun no expongan esos gauges
(en produccion deje los defaults activos).

El mismo helper ahora aplica telemetria de load-test del scheduler. Use `--min-teu-capacity` para
probar que cada lane reporta `nexus_scheduler_lane_teu_capacity`, limite la utilizacion de slots con
`--max-teu-slot-commit-ratio` (compara `nexus_scheduler_lane_teu_slot_committed` contra la capacidad),
y mantenga los contadores de deferral/truncation en cero via `--max-teu-deferrals` y
`--max-must-serve-truncations`. Estos knobs convierten el requisito NX-7 de "load tests mas
profundos" en un check CLI repetible: el helper falla cuando una lane difiere trabajo PQ/TEU o cuando
el TEU comprometido por slot excede el headroom configurado, y el CLI imprime el resumen por lane
para que los paquetes de evidencia capturen los mismos numeros que valido CI.

Para validaciones air-gapped (o CI) puede reproducir una respuesta capturada de Torii en lugar de
consultar un nodo vivo:

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
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Las fixtures bajo `fixtures/nexus/lanes/` reflejan los artefactos producidos por el helper de
bootstrap para que nuevos manifests se puedan lint sin scripting a medida. CI ejecuta el mismo flujo
via `ci/check_nexus_lane_smoke.sh` y tambien corre `ci/check_nexus_lane_registry_bundle.sh`
(alias: `make check-nexus-lanes`) para garantizar que el helper de smoke NX-7 siga estando alineado
con el formato de payload publicado y para asegurar que los digests/overlays del bundle sigan siendo
reproducibles.

Cuando se renombra una lane, capture los eventos de telemetria `nexus.lane.topology` (por ejemplo
con `journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) y alimentelos al
helper de smoke. El nuevo flag `--telemetry-file/--from-telemetry` acepta el log newline-delimited y
`--require-alias-migration old:new` asegura que un evento `alias_migrated` registro el rename:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
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
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

El fixture `telemetry_alias_migrated.ndjson` agrega el sample canonico de rename para que CI
verifique el path de parsing de telemetria sin contactar un nodo vivo.

## 6. Pruebas de carga de validadores (evidencia NX-7)

El roadmap **NX-7** requiere que los operadores de lane capturen una corrida de carga reproducible
antes de marcar una lane como lista para produccion. El objetivo es estresar la lane lo suficiente
para ejercitar duracion de slot, backlog de settlement, quorum DA, oraculos, scheduler headroom y
TEU metrics, luego archivar el resultado de forma que auditores puedan reproducirlo sin tooling a
medida. El nuevo helper `scripts/nexus_lane_load_test.py` une los checks de smoke, el gating de
slot-duration y el slot bundle manifest en un conjunto de artefactos para que las corridas de carga
se publiquen directamente en tickets de gobernanza.

### 6.1 Preparacion de workload

1. Cree un directorio de run y capture fixtures canonicos para la lane bajo prueba:

   ```bash
   mkdir -p artifacts/nexus/load/payments-2026q2
   cargo xtask nexus-fixtures --output artifacts/nexus/load/payments-2026q2/fixtures
   ```

   Las fixtures reflejan `fixtures/nexus/lane_commitments/*.json` y dan al generador de carga una
   semilla deterministica (registre la semilla en `artifacts/.../README.md`).
2. Baseline de la lane antes de la corrida:

   ```bash
   scripts/nexus_lane_smoke.py \
     --status-url https://torii.example.com/v1/sumeragi/status \
     --metrics-url https://torii.example.com/metrics \
     --lane-alias payments \
     --expected-lane-count 3 \
     --min-block-height 50000 \
     --max-finality-lag 4 \
     --max-settlement-backlog 0.5 \
     --min-settlement-buffer 0.25 \
     --max-slot-p95 1000 \
     --max-slot-p99 1100 \
     --min-slot-samples 50 \
     --insecure \
     > artifacts/nexus/load/payments-2026q2/smoke_before.log
   ```

   Mantenga stdout/stderr en el directorio de run para que los thresholds de smoke sean auditables.
3. Capture el log de telemetria que luego alimenta `--telemetry-file` (evidencia de alias migration)
   y `validate_nexus_telemetry_pack.py`:

   ```bash
   journalctl -u irohad -o json \
     --since "2026-05-10T09:00:00Z" \
     --until "2026-05-10T11:00:00Z" \
     > artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson
   ```

4. Inicie el workload de la lane (perfil k6, replay harness, o pruebas de ingestion federadas) y
   mantenga la semilla de workload + rango de slots a mano; la metadata se usa en el validador de
   telemetry manifest en la seccion 6.3.

5. Empaquete la evidencia de la corrida con el nuevo helper. Proporcione los payloads capturados de
   status/metrics/telemetry, los alias de lanes, y cualquier evento de alias migration que deba
   aparecer en telemetria. El helper escribe `smoke.log`, `slot_summary.json`, un slot bundle
   manifest, y `load_test_manifest.json` que une todo para revision de gobernanza:

   ```bash
   scripts/nexus_lane_load_test.py \
     --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
     --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
     --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
     --lane-alias payments \
     --lane-alias core \
     --expected-lane-count 3 \
     --slot-range 81200-81600 \
     --workload-seed NX7-PAYMENTS-2026Q2 \
     --require-alias-migration core:payments \
     --out-dir artifacts/nexus/load/payments-2026q2
   ```

   El comando aplica los mismos gates de quorum DA, oracle, settlement buffer, TEU y slot-duration
   usados en esta guia y produce un manifest listo para adjuntar sin scripting a medida.

### 6.2 Corrida instrumentada

Mientras el workload satura la lane:

1. Snapshot de status + metrics de Torii:

   ```bash
   curl -sS https://torii.example.com/v1/sumeragi/status \
     > artifacts/nexus/load/payments-2026q2/torii_status.json
   curl -sS https://torii.example.com/metrics \
     > artifacts/nexus/load/payments-2026q2/metrics.prom
   ```

2. Calcule quantiles de slot-duration y archive el summary:

   ```bash
   scripts/telemetry/check_slot_duration.py \
     artifacts/nexus/load/payments-2026q2/metrics.prom \
     --max-p95-ms 1000 \
     --max-p99-ms 1100 \
     --min-samples 200 \
     --json-out artifacts/nexus/load/payments-2026q2/slot_summary.json
   scripts/telemetry/bundle_slot_artifacts.py \
     --metrics artifacts/nexus/load/payments-2026q2/metrics.prom \
     --summary artifacts/nexus/load/payments-2026q2/slot_summary.json \
     --out-dir artifacts/nexus/load/payments-2026q2/slot_bundle \
     --metadata lane=payments \
     --metadata workload_seed=NX7-PAYMENTS-2026Q2
   ```

3. Exporte el snapshot de lane-governance como JSON + Parquet para auditorias de largo plazo:

   ```bash
   cargo xtask nexus-lane-audit \
     --status artifacts/nexus/load/payments-2026q2/torii_status.json \
     --json-out artifacts/nexus/load/payments-2026q2/lane_audit.json \
     --parquet-out artifacts/nexus/load/payments-2026q2/lane_audit.parquet \
     --captured-at 2026-05-10T10:15:00Z
   ```

   El snapshot JSON/Parquet ahora registra utilizacion TEU, niveles de trigger del scheduler,
   contadores de RBC chunk/bytes y estadisticas del grafo de transacciones por lane para que la
   evidencia de rollout muestre backlog y presion de ejecucion.

4. Ejecute el helper de smoke otra vez al pico de carga para evaluar thresholds bajo estres (escriba
   la salida en `smoke_during.log`) y re-ejecute cuando termine el workload (`smoke_after.log`).

### 6.3 Telemetry pack y manifest de gobernanza

El directorio de la corrida debe incluir un telemetry pack (`prometheus.tgz`, stream OTLP, logs
estructurados, y cualquier output del harness). Valide el pack y selle la metadata esperada por
la gobernanza:

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus/load/payments-2026q2 \
  --manifest-out artifacts/nexus/load/payments-2026q2/telemetry_manifest.json \
  --expected prometheus.tgz --expected otlp.ndjson \
  --expected torii_structured_logs.jsonl --expected B4-RB-2026Q1.log \
  --slot-range 81200-81600 --require-slot-range \
  --workload-seed NX7-PAYMENTS-2026Q2 --require-workload-seed \
  --metadata lane=payments --metadata run=2026q2-rollout
```

Finalmente, adjunte el log de telemetria capturado y exija evidencia de alias migration cuando una
lane se renombre durante la prueba:

```bash
scripts/nexus_lane_smoke.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --require-alias-migration core:payments \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-block-height 50000 \
  --max-finality-lag 4 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 200
```

Archive los siguientes artefactos para el ticket de gobernanza:

- `smoke_before.log`, `smoke_during.log`, `smoke_after.log`
- `metrics.prom`, `slot_summary.json`, `slot_bundle_manifest.json`
- `lane_audit.{json,parquet}`
- `telemetry_manifest.json` + contenido del pack (`prometheus.tgz`, `otlp.ndjson`, etc.)
- `nexus.lane.topology.ndjson` (o el slice de telemetria relevante)

La corrida ahora puede referenciarse dentro de manifests de Space Directory y trackers de
 gobernanza como el load test NX-7 canonico para la lane.

## 7. Seguimientos de telemetria y gobernanza

- Actualice los dashboards de lane (`dashboards/grafana/nexus_lanes.json` y overlays relacionados)
  con el nuevo lane id y metadata. Las keys generadas (`contact`, `channel`, `runbook`, etc.)
  facilitan prellenar labels.
- Cablee reglas de PagerDuty/Alertmanager para la nueva lane antes de habilitar admission. El
  `summary.json` refleja la checklist en `docs/source/nexus_operations.md`.
- Registre el manifest bundle en Space Directory una vez que el set de validadores este vivo. Use
  el mismo manifest JSON generado por el helper, firmado segun el runbook de gobernanza.
- Siga `docs/source/sora_nexus_operator_onboarding.md` para smoke tests (FindNetworkStatus,
  reachability de Torii) y capture la evidencia con el conjunto de artefactos producido arriba.

## 8. Ejemplo de dry-run

Para previsualizar los artefactos sin escribir archivos:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --dry-run
```

El comando imprime el JSON summary y el snippet TOML en stdout, permitiendo iteracion rapida
 durante la planificacion.

---

Para mas contexto ver:

- `docs/source/nexus_operations.md` - checklist operacional y requisitos de telemetria.
- `docs/source/sora_nexus_operator_onboarding.md` - flujo de onboarding detallado que referencia el
  nuevo helper.
- `docs/source/nexus_lanes.md` - geometria de lanes, slugs, y layout de storage usado por la
  herramienta.
