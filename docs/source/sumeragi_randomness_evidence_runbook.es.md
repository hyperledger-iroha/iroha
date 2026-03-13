---
lang: es
direction: ltr
source: docs/source/sumeragi_randomness_evidence_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9a7b2f030cb798b78947c0d7cb298ccbd8a94a006be2e804f1ed043dc15dabc
source_last_modified: "2025-11-15T08:00:21.780712+00:00"
translation_last_reviewed: 2026-01-01
---

# Runbook de aleatoriedad y evidencia de Sumeragi

Esta guia cumple el hito A6 del roadmap que requeria procedimientos de operador actualizados
para la aleatoriedad VRF y la evidencia de slashing. Usala junto con
{doc}`sumeragi` y {doc}`sumeragi_chaos_performance_runbook` cuando prepares
un nuevo build de validador o captures artefactos de readiness para governance.


Note: For the v1 release, VRF penalties jail offenders after the activation lag, and consensus slashing is delayed by `sumeragi.npos.reconfig.slashing_delay_blocks` (default 259200 blocks, ~3 days at 1s) so governance can cancel with `CancelConsensusEvidencePenalty` before it applies.

## Alcance y prerrequisitos

- `iroha_cli` configurado para el cluster objetivo (ver `docs/source/cli.md`).
- `curl`/`jq` para extraer el payload `/status` de Torii al preparar entradas.
- Acceso a Prometheus (o exportaciones snapshot) para las metricas `sumeragi_vrf_*`.
- Conocer la epoca y el roster actuales para poder emparejar la salida del CLI con el
  snapshot de staking o el manifiesto de governance.

## 1. Confirmar seleccion de modo y contexto de epoca

1. Ejecuta `iroha --output-format text ops sumeragi params` para probar que el binario cargo
   `sumeragi.consensus_mode="npos"` y registrar `k_aggregators`,
   `redundant_send_r`, la longitud de la epoca y los offsets de commit/reveal VRF.
2. Inspecciona la vista runtime:

   ```bash
   iroha --output-format text ops sumeragi status
   iroha --output-format text ops sumeragi collectors
   iroha --output-format text ops sumeragi rbc status
   ```

   La linea `status` imprime la tupla leader/view, el backlog RBC, los reintentos DA,
   offsets de epoca y deferrals del pacemaker; `collectors` mapea indices de collectors
   a IDs de peers para mostrar que validadores llevan tareas de aleatoriedad en la
   altura inspeccionada.
3. Captura el numero de epoca que quieres auditar:

   ```bash
   EPOCH=$(curl -s "$TORII/status" | jq '.sumeragi.epoch.height // 0')
   printf "auditing epoch %s\n" "$EPOCH"
   ```

   Guarda el valor (decimal o con prefijo `0x`) para los comandos VRF de abajo.

## 2. Snapshot de epocas VRF y penalizaciones

Usa los subcomandos dedicados del CLI para extraer los registros VRF persistidos de
cada validador:

```bash
iroha --output-format text ops sumeragi vrf-epoch --epoch "$EPOCH"
iroha ops sumeragi vrf-epoch --epoch "$EPOCH" > artifacts/vrf_epoch_${EPOCH}.json

iroha --output-format text ops sumeragi vrf-penalties --epoch "$EPOCH"
iroha ops sumeragi vrf-penalties --epoch "$EPOCH" > artifacts/vrf_penalties_${EPOCH}.json
```

Los resumenes muestran si la epoca esta finalizada, cuantos participantes
presentaron commits/reveals, la longitud del roster y la seed derivada. El JSON
captura la lista de participantes, el estado de penalizacion por firmante y el
valor `seed_hex` usado por el pacemaker. Compara el conteo de participantes con el
roster de staking y verifica que las penalizaciones reflejen las alertas
activadas durante las pruebas de caos (late reveals deben aparecer en `late_reveals`,
validadores forfeited bajo `no_participation`).

## 3. Monitorear telemetria VRF y alertas

Prometheus expone los contadores requeridos por el roadmap:

- `sumeragi_vrf_commits_emitted_total`
- `sumeragi_vrf_reveals_emitted_total`
- `sumeragi_vrf_reveals_late_total`
- `sumeragi_vrf_non_reveal_penalties_total`
- `sumeragi_vrf_non_reveal_by_signer{signer="peer_id"}`
- `sumeragi_vrf_no_participation_total`
- `sumeragi_vrf_no_participation_by_signer{signer="peer_id"}`
- `sumeragi_vrf_rejects_total_by_reason{reason="..."}`

Ejemplo de PromQL para el reporte semanal:

```promql
increase(sumeragi_vrf_non_reveal_by_signer[1w]) > 0
```

Durante los drills de readiness confirma que:

- `sumeragi_vrf_commits_emitted_total` y `..._reveals_emitted_total` aumentan
  para cada bloque dentro de las ventanas de commit/reveal.
- Los escenarios de late-reveal disparan `sumeragi_vrf_reveals_late_total` y limpian
  la entrada correspondiente en el JSON `vrf_penalties`.
- `sumeragi_vrf_no_participation_total` solo sube cuando intencionalmente retienes
  commits durante las pruebas de caos.

El overview de Grafana (`docs/source/grafana_sumeragi_overview.json`) incluye paneles
para cada contador; captura screenshots despues de cada corrida y adjuntalos al
paquete de artefactos referenciado en {doc}`sumeragi_chaos_performance_runbook`.

## 4. Ingestion y streaming de evidence

La evidencia de slashing debe recolectarse en cada validador y reenviarse a Torii.
Usa los helpers CLI para demostrar paridad con los endpoints HTTP documentados en
{doc}`torii/sumeragi_evidence_app_api`:

```bash
# Count and list persisted evidence
iroha --output-format text ops sumeragi evidence count
iroha --output-format text ops sumeragi evidence list --limit 5

# Show JSON for audits
iroha ops sumeragi evidence list --limit 100 > artifacts/evidence_snapshot.json
```

Verifica que el `total` reportado coincide con el widget de Grafana alimentado por
`sumeragi_evidence_records_total`, y confirma que los registros mas antiguos que
`sumeragi.npos.reconfig.evidence_horizon_blocks` se rechazan (el CLI imprime el
motivo). Al probar alerting, envia un payload conocido y valido via:

```bash
iroha --output-format text ops sumeragi evidence submit --evidence-hex-file fixtures/evidence/double_prevote.hex
```

Monitorea `/v2/events/sse` con un stream filtrado para probar que los SDKs ven los
mismos datos: reutiliza el one-liner de Python de {doc}`torii/sumeragi_evidence_app_api`
para construir el filtro y captura los frames `data:` sin procesar. Los payloads
SSE deben reflejar el kind de evidencia y el signer que aparecio en la salida del CLI.

## 5. Empaquetado de evidencia y reporte

Para cada rehearsal o release candidate:

1. Guarda los JSON del CLI (`vrf_epoch_*.json`, `vrf_penalties_*.json`,
   `evidence_snapshot.json`) bajo el directorio de artefactos del run (el mismo
   root usado por los scripts de caos/performance).
2. Registra los resultados de Prometheus o exportaciones snapshot para los
   contadores listados arriba.
3. Adjunta la captura SSE y los acknowledgements de alertas al README de artefactos.
4. Actualiza `status.md` y
   `docs/source/project_tracker/npos_sumeragi_phase_a.md` con las rutas de
   artefactos y el numero de epoca inspeccionado.

Seguir este checklist mantiene auditables las pruebas de aleatoriedad VRF y la
 evidencia de slashing durante el rollout NPoS y ofrece a governance un rastro
 determinista hacia los datos capturados y los snapshots del CLI.

## 6. Senales de troubleshooting

- **Mode selection mismatch** - Si `iroha --output-format text ops sumeragi params` muestra
  `consensus_mode="permissioned"` o `k_aggregators` difiere del manifiesto,
  elimina los artefactos capturados, corrige `iroha_config`, reinicia el validador
  y vuelve a ejecutar el flujo de validacion descrito en {doc}`sumeragi`.
- **Missing commits or reveals** - Una serie plana de `sumeragi_vrf_commits_emitted_total`
  o `sumeragi_vrf_reveals_emitted_total` significa que Torii no reenvia frames VRF.
  Revisa los logs del validador para errores `handle_vrf_*`, luego reenvia el payload
  manualmente via los helpers POST documentados arriba.
- **Unexpected penalties** - Cuando `sumeragi_vrf_no_participation_total` se dispara,
  revisa el archivo `vrf_penalties_<epoch>.json` para confirmar el signer ID y
  comparalo con el roster de staking. Penalizaciones que no coinciden con las
  pruebas de caos indican desvio de reloj del validador o proteccion de replay en Torii;
  corrige el peer afectado antes de repetir la prueba.
- **Evidence ingestion stalls** - Cuando `sumeragi_evidence_records_total`
  se estanca mientras las pruebas de caos emiten fallas, ejecuta
  `iroha ops sumeragi evidence count` en varios validadores y confirma que
  `/v2/sumeragi/evidence/count` coincide con la salida del CLI. Cualquier divergencia
  implica que consumidores SSE/webhook pueden estar desactualizados, asi que reenvia
  un fixture conocido y escala a los mantenedores de Torii si el contador no incrementa.
