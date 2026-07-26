---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 25812f21710b66abc2bdf23e466806aa0c5e2f54ec7b286fb563b5d3ec7c26c0
source_last_modified: "2025-11-11T10:55:08.297924+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: pq-ratchet-runbook
title: Simulacro de PQ Ratchet de SoraNet
sidebar_label: Runbook de PQ Ratchet
description: Pasos de ensayo para guardia al promover o degradar la politica de anonimato PQ escalonada con validacion de telemetria determinista.
---

:::note Fuente canonica
Esta pagina refleja `docs/source/soranet/pq_ratchet_runbook.md`. Manten ambas copias sincronizadas.
:::

## Proposito

Este runbook guia la secuencia del simulacro para la politica de anonimato post-quantum (PQ) escalonada de SoraNet. Los operadores ensayan tanto la promocion (Stage A -> Stage B -> Stage C) como la degradacion controlada de regreso a Stage B/A cuando cae el supply PQ. El simulacro valida los hooks de telemetria (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) y recolecta artefactos para el log de rehearsal de incidentes.

## Prerequisitos

- Ultimo binario `sorafs_orchestrator` con capability-weighting (commit en o despues de la referencia del simulacro mostrada en `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Acceso al stack de Prometheus/Grafana que sirve `dashboards/grafana/soranet_pq_ratchet.json`.
- Snapshot nominal del guard directory. Trae y verifica una copia antes del simulacro:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Si el source directory solo publica JSON, re-encodealo a Norito binario con `soranet-directory build` antes de ejecutar los helpers de rotacion.

- Captura metadata y pre-stagea artefactos de rotacion del issuer con el CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Ventana de cambio aprobada por los equipos on-call de networking y observability.

## Pasos de promocion

1. **Stage audit**

   Registra el stage inicial:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Espera `anon-guard-pq` antes de promocionar.

2. **Promociona a Stage B (Majority PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Espera >=5 minutos para que los manifests refresquen.
   - En Grafana (dashboard `SoraNet PQ Ratchet Drill`) confirma que el panel "Policy Events" muestre `outcome=met` para `stage=anon-majority-pq`.
   - Captura un screenshot o el panel JSON y adjuntalo al log de incidentes.

3. **Promociona a Stage C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Verifica que los histogramas `sorafs_orchestrator_pq_ratio_*` tiendan a 1.0.
   - Confirma que el contador de brownout permanezca plano; si no, sigue los pasos de degradacion.

## Simulacro de degradacion / brownout

1. **Induce una escasez sintetica de PQ**

   Deshabilita relays PQ en el entorno de playground recortando el guard directory a entradas clasicas solamente, luego recarga el cache del orchestrator:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observa la telemetria de brownout**

   - Dashboard: el panel "Brownout Rate" sube por encima de 0.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` debe reportar `anonymity_outcome="brownout"` con `anonymity_reason="missing_majority_pq"`.

3. **Degrada a Stage B / Stage A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Si el supply PQ sigue insuficiente, degrada a `anon-guard-pq`. El simulacro termina cuando los contadores de brownout se estabilizan y las promociones pueden reaplicarse.

4. **Restaura el guard directory**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetria y artefactos

- **Dashboard:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** asegurate de que la alerta de brownout de `sorafs_orchestrator_policy_events_total` se mantenga por debajo del SLO configurado (&lt;5% en cualquier ventana de 10 minutos).
- **Incident log:** adjunta los snippets de telemetria y notas del operador a `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Captura firmada:** usa `cargo xtask soranet-rollout-capture` para copiar el drill log y el scoreboard en `artifacts/soranet_pq_rollout/<timestamp>/`, calcular digests BLAKE3 y producir un `rollout_capture.json` firmado.

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

Adjunta los metadatos generados y la firma al paquete de governance.

## Rollback

Si el simulacro descubre escasez real de PQ, permanece en Stage A, notifica al Networking TL y adjunta las metricas recolectadas junto con los diffs del guard directory al incident tracker. Usa el export del guard directory capturado anteriormente para restaurar el servicio normal.

:::tip Cobertura de regresion
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` proporciona la validacion sintetica que respalda este simulacro.
:::
