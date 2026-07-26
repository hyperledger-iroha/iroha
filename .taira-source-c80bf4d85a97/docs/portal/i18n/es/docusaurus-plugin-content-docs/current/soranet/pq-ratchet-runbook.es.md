---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: pq-ratchet-runbook
título: Simulacro de PQ Ratchet de SoraNet
sidebar_label: Runbook de PQ Ratchet
descripción: Pasos de ensayo para guardia al promover o degradar la política de anonimato PQ escalonada con validación de telemetría determinista.
---

:::nota Fuente canónica
Esta página refleja `docs/source/soranet/pq_ratchet_runbook.md`. Manten ambas copias sincronizadas.
:::

## propuesta

Este runbook guía la secuencia del simulacro para la política de anonimato post-quantum (PQ) escalonada de SoraNet. Los operadores ensayan tanto la promoción (Etapa A -> Etapa B -> Etapa C) como la degradación controlada de regreso a Etapa B/A cuando cae el suministro PQ. El simulacro valida los ganchos de telemetria (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) y recolecta artefactos para el registro de ensayo de incidentes.

##Requisitos previos

- Último binario `sorafs_orchestrator` con capacidad-ponderación (commit en o después de la referencia del simulacro mostrado en `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Acceso al stack de Prometheus/Grafana que sirve `dashboards/grafana/soranet_pq_ratchet.json`.
- Instantánea nominal del directorio de guardia. Trae y verifica una copia antes del simulacro:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Si el directorio fuente solo es público JSON, vuelva a codificar un binario Norito con `soranet-directory build` antes de ejecutar los ayudantes de rotación.

- Captura de metadatos y artefactos de rotación del emisor con el CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```- Ventana de cambio aprobada por los equipos on-call de networking y observabilidad.

## Pasos de promoción

1. **Auditoría de etapa**

   Registra la etapa inicial:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Espera `anon-guard-pq` antes de promocionar.

2. **Promociona a Etapa B (PQ Mayoritaria)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Espera >=5 minutos para que los manifiestos refresquen.
   - En Grafana (dashboard `SoraNet PQ Ratchet Drill`) confirma que el panel "Policy Events" muestre `outcome=met` para `stage=anon-majority-pq`.
   - Captura una captura de pantalla del panel JSON y adjuntalo al registro de incidentes.

3. **Promociona a Etapa C (PQ estricta)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Verifica que los histogramas `sorafs_orchestrator_pq_ratio_*` tiendan a 1.0.
   - Confirma que el contador de apagón permanece plano; si no, sigue los pasos de degradación.

## Simulacro de degradación / apagón

1. **Induce una escasez sintética de PQ**

   Deshabilita Relays PQ en el entorno de Playground recortando el guard directorio a entradas clásicas solamente, luego recarga el caché del orquestador:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observa la telemetría de apagón**

   - Panel de control: el panel "Brownout Rate" sube por encima de 0.
   -PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` debe reportar `anonymity_outcome="brownout"` con `anonymity_reason="missing_majority_pq"`.

3. **Degradar a Etapa B / Etapa A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```Si el suministro PQ sigue insuficiente, degrada a `anon-guard-pq`. El simulacro termina cuando los contadores de apagón se estabilizan y las promociones pueden reaplicarse.

4. **Directorio de restaurante el guardia**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetria y artefactos

- **Panel de control:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** asegúrese de que la alerta de caída de tensión de `sorafs_orchestrator_policy_events_total` se mantenga por debajo del SLO configurado (&lt;5% en cualquier ventana de 10 minutos).
- **Registro de incidentes:** adjunta los snippets de telemetria y notas del operador a `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Captura firmada:** usa `cargo xtask soranet-rollout-capture` para copiar el registro de perforación y el marcador en `artifacts/soranet_pq_rollout/<timestamp>/`, calcular resúmenes BLAKE3 y producir un `rollout_capture.json` firmado.

Ejemplo:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Adjunta los metadatos generados y la firma al paquete de gobernanza.

## Revertir

Si el simulacro descubre escasez real de PQ, permanece en Stage A, notifica al Networking TL y adjunta las métricas recolectadas junto con los diffs del guard directorio al incident tracker. Usa el export del guard directorio capturado anteriormente para restaurar el servicio normal.

:::tip Cobertura de regresión
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` proporciona la validación sintética que respalda este simulacro.
:::