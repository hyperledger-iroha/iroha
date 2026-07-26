---
lang: es
direction: ltr
source: docs/source/runbooks/nexus_multilane_rehearsal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0aa4642cc60f384f6c52aaae2f97a6e4e8f741d6365c483514c2016d1ba10e82
source_last_modified: "2025-12-14T09:53:36.243318+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook de ensayo de lanzamiento multi-lane de Nexus

Este runbook guía el ensayo Nexus Phase B4. Valida que el paquete de
`iroha_config` aprobado por gobernanza y el manifiesto génesis multi‑lane se
comporten de forma determinista a través de telemetría, enrutamiento y drills de
rollback.

## Alcance

- Ejercitar los tres lanes de Nexus (`core`, `governance`, `zk`) con ingreso
  mixto en Torii (transacciones, despliegues de contratos, acciones de
  gobernanza) usando la semilla de carga firmada `NEXUS-REH-2026Q1`.
- Capturar artefactos de telemetría/trazas requeridos por la aceptación B4
  (scrape de Prometheus, export OTLP, logs estructurados, trazas de admisión
  Norito, métricas RBC).
- Ejecutar el drill de rollback `B4-RB-2026Q1` inmediatamente después del dry-run
  y confirmar que el perfil mono‑lane se re‑aplica limpiamente.

## Precondiciones

1. `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` refleja la
   aprobación GOV-2026-03-19 (manifiestos firmados + iniciales de revisores).
2. `defaults/nexus/config.toml` (sha256
   `4f57655666bb0c83221cd3b56fd37218822e4c63db07e78a6694db51077f7017`, blake2b
   `65827a4b0348a7837f181529f602dc3315eba55d6ca968aaafb85b4ef8cfb2f6759283de77590ec5ec42d67f5717b54a299a733b617a50eb2990d1259c848017`, con
   `nexus.enabled = true` incorporado) y `defaults/nexus/genesis.json` coinciden
   con los hashes aprobados; `kagami genesis bootstrap --profile nexus` reporta
   el mismo digest registrado en el tracker.
3. El catálogo de lanes coincide con el layout aprobado de tres lanes;
   `irohad --sora --config defaults/nexus/config.toml` debe emitir el banner del
   router Nexus.
4. La CI multi‑lane está en verde: `ci/check_nexus_multilane_pipeline.sh`
   (ejecuta `integration_tests/tests/nexus/multilane_pipeline.rs` vía
   `.github/workflows/integration_tests_multilane.yml`) y
   `ci/check_nexus_multilane.sh` (cobertura de router) ambos pasan para que el
   perfil Nexus siga listo para multi‑lane (`nexus.enabled = true`, hashes del
   catálogo Sora intactos, almacenamiento por lane bajo `blocks/lane_{id:03}_{slug}`
   y merge logs provisionados). Capture los digests de artefactos en el tracker
   cuando cambie el paquete de defaults.
5. Dashboards y alertas de telemetría para métricas Nexus están importados en la
   carpeta de Grafana del ensayo; las rutas de alerta apuntan al servicio de
   PagerDuty del ensayo.
6. Los lanes de Torii SDK están configurados según la tabla de políticas de
   enrutamiento y pueden reproducir la carga del ensayo localmente.

## Resumen de cronograma

| Fase | Ventana objetivo | Responsable(s) | Criterio de salida |
|-------|---------------|----------------|---------------|
| Preparación | Apr 1 – 5 2026 | @program-mgmt, @telemetry-ops | Semilla publicada, dashboards listos, nodos de ensayo provisionados. |
| Congelación de staging | Apr 8 2026 18:00 UTC | @release-eng | Hashes de config/génesis re‑verificados; aviso de congelación enviado. |
| Ejecución | Apr 9 2026 15:00 UTC | @qa-veracity, @nexus-core, @torii-sdk | Checklist completo sin incidentes bloqueantes; paquete de telemetría archivado. |
| Drill de rollback | Inmediatamente post‑ejecución | @sre-core | Checklist `B4-RB-2026Q1` completo; telemetría de rollback capturada. |
| Retrospectiva | Hasta Apr 15 2026 | @program-mgmt, @telemetry-ops, @governance | Doc de retro/lecciones aprendidas + tracker de bloqueos publicado. |

## Checklist de ejecución (Apr 9 2026 15:00 UTC)

1. **Atestación de configuración** — `iroha_cli config show --actual` en cada
   nodo; confirme que los hashes coinciden con la entrada del tracker.
2. **Warm‑up de lanes** — reproduzca la carga semilla durante 2 slots y verifique
   que `nexus_lane_state_total` muestra actividad en los tres lanes.
3. **Captura de telemetría** — registre snapshots de Prometheus `/metrics`,
   muestras de paquetes OTLP, logs estructurados de Torii (por lane/dataspace) y
   métricas RBC.
4. **Ganchos de gobernanza** — ejecute el subconjunto de transacciones de
   gobernanza y verifique el enrutamiento por lane + etiquetas de telemetría.
5. **Drill de incidente** — simule saturación de lane según el plan; asegure que
   las alertas disparen y que la respuesta quede registrada.
6. **Drill de rollback `B4-RB-2026Q1`** — aplique el perfil mono‑lane, reproduzca
   el checklist de rollback, recolecte evidencia de telemetría y re‑aplique el
   paquete Nexus.
7. **Carga de artefactos** — suba el paquete de telemetría, trazas de Torii y
   log del drill al bucket de evidencia Nexus; enlácelo en
   `docs/source/nexus_transition_notes.md`.
8. **Manifiesto/validación** — ejecute `scripts/telemetry/validate_nexus_telemetry_pack.py \
   --pack-dir <path> --slot-range <start-end> --workload-seed <value> \
   --require-slot-range --require-workload-seed` para producir
   `telemetry_manifest.json` + `.sha256`, luego adjunte el manifiesto a la
   entrada del tracker del ensayo. El helper normaliza los límites de slot
   (registrados como enteros en el manifiesto) y falla rápido cuando falta algún
   indicador, de modo que los artefactos de gobernanza permanezcan deterministas.

## Salidas

- Checklist de ensayo firmado + log del drill de incidente.
- Paquete de telemetría (`prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`).
- Manifiesto de telemetría + digest generado por el script de validación.
- Documento de retrospectiva que resume bloqueos, mitigaciones y asignaciones.

## Resumen de ejecución — Apr 9 2026

- El ensayo se ejecutó 15:00 UTC–16:12 UTC con semilla `NEXUS-REH-2026Q1`; los tres
  lanes sostuvieron ~2.4k TEU por slot y `nexus_lane_state_total` reportó
  envolturas balanceadas.
- El paquete de telemetría se archivó en `artifacts/nexus/rehearsals/2026q1/`
  (incluye `prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`, log de
  incidente y evidencia de rollback). Los checksums se registraron en
  `docs/source/project_tracker/nexus_rehearsal_2026q1.md`.
- El drill de rollback `B4-RB-2026Q1` se completó a las 16:18 UTC; el perfil
  mono‑lane se re‑aplicó en 6m42s sin lanes bloqueados, y el paquete Nexus se
  re‑habilitó tras la confirmación de telemetría.
- El incidente de saturación de lane inyectado en el slot 842 (clamp forzado de
  headroom) disparó las alertas esperadas; el playbook de mitigación cerró el
  page en 11m con cronología PagerDuty documentada.
- No hubo bloqueos que impidieran la finalización; los follow‑ups (automatización
  de logging de headroom TEU, script de validación del paquete de telemetría) se
  rastrean en la retrospectiva del Apr 15.

## Escalación

- Incidentes bloqueantes o regresiones de telemetría detienen el ensayo y
  requieren escalación a gobernanza dentro de 4 horas hábiles.
- Cualquier desviación del paquete de config/génesis aprobado debe reiniciar el
  ensayo tras re‑aprobación.

## Validación del paquete de telemetría (Completado)

Ejecute `scripts/telemetry/validate_nexus_telemetry_pack.py` después de cada
ensayo para demostrar que el bundle de telemetría contiene los artefactos
canónicos (export de Prometheus, OTLP NDJSON, logs estructurados de Torii, log
 de rollback) y capturar sus digests SHA-256. El helper escribe `telemetry_manifest.json` y el
archivo `.sha256` correspondiente para que gobernanza pueda citar directamente
los hashes de evidencia en el paquete de retro.

Para el ensayo del Apr 9 2026, el manifiesto validado vive junto a los
artefactos en `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` con su
digest en `telemetry_manifest.json.sha256`. Adjunte ambos archivos a la entrada
del tracker al publicar la retro.

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus_rehearsal_2026q1 \
  --slot-range 820-860 \
  --workload-seed NEXUS-REH-2026Q1 \
  --metadata rehearsal_id=B4-2026Q1 team=telemetry-ops
```

Pase `--require-slot-range` / `--require-workload-seed` dentro de CI para bloquear
cargas que olviden esas anotaciones. Use `--expected <name>` para añadir
artefactos extra (p. ej., recibos DA) si el plan de ensayo lo requiere.
