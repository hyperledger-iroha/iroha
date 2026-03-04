---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: pq-ratchet-runbook
título: Simulación PQ Ratchet SoraNet
sidebar_label: Trinquete Runbook PQ
descripción: Cintas de ensayo de guardia para promover o retrogradar la política de anonimato PQ y validar la telemetría determinista.
---

:::nota Fuente canónica
Esta página refleja `docs/source/soranet/pq_ratchet_runbook.md`. Gardez les deux copys alignees jusqu'a la retraite de l'ancien set dedocumentation.
:::

## Objetivo

Este runbook guía la secuencia de simulacro de incendio para la política anónima post-cuántica (PQ) etagee de SoraNet. Los operadores repiten la promoción (Etapa A -> Etapa B -> Etapa C) además de la retrogradación controlada hacia la Etapa B/A cuando la oferta PQ baisse. El taladro valida los ganchos de telemetría (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) y recoge los artefactos para el registro de ensayo del incidente.

## Requisitos previos

- Dernier binaire `sorafs_orchestrator` con ponderación de capacidad (compromiso igual o posterior a la referencia del taladro en `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Acceda a la pila Prometheus/Grafana aquí `dashboards/grafana/soranet_pq_ratchet.json`.
- Instantánea del directorio nominal du guard. Recupere y verifique una copia antes del taladro:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Si el directorio fuente no está publicado en JSON, vuelva a codificarlo en binario Norito con `soranet-directory build` antes de ejecutar los asistentes de rotación.

- Capture los metadatos y prepare previamente los artefactos de rotación del emisor con la CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```- Función de cambio aprobada por los equipos de guardia, networking y observabilidad.

## Cintas de promoción

1. **Auditoría de etapa**

   Registre la etapa de salida:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Atendida `anon-guard-pq` promoción anticipada.

2. **Promoción versus Etapa B (PQ mayoritaria)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Atender >=5 minutos para que les manifests se rafraichissent.
   - En Grafana (panel de control `SoraNet PQ Ratchet Drill`), confirme que el panel "Eventos de política" muestra `outcome=met` para `stage=anon-majority-pq`.
   - Capture una captura de pantalla o el JSON del panel y adjunte el registro del incidente.

3. **Promoción versus Etapa C (PQ estricto)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Verifique que los histogramas `sorafs_orchestrator_pq_ratio_*` tengan la versión 1.0.
   - Confirmez que le compteur brownout reste plat; sinon suivez les etapas de retrogradación.

## Simulacro de retrogradación / apagón

1. **Induire une penurie PQ synthetique**

   Desactive los relés PQ en el entorno de juegos y luego guarde el directorio en las mismas entradas clásicas y recargue el caché orquestador:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observador de la caída de telemetría**

   - Panel de control: el panel "Brownout Rate" se encuentra en 0.
   -PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` haga el reportero `anonymity_outcome="brownout"` con `anonymity_reason="missing_majority_pq"`.

3. **Retrógrador versus Etapa B / Etapa A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```Si la oferta PQ es insuficiente, retrogradez vers `anon-guard-pq`. El taladro se terminará cuando los ordenadores se apaguen y se estabilicen y las promociones puedan volver a aplicarse.

4. **Directorio del restaurante le guard**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetría y artefactos

- **Panel de control:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** asegúrese de que la alerta de caída de tensión se produzca para que `sorafs_orchestrator_policy_events_total` reste bajo la configuración de SLO (&lt;5% en cada período de 10 minutos).
- **Registro de incidentes:** agregue fragmentos de telemetría y notas operativas a `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Capturar firmante:** utilice `cargo xtask soranet-rollout-capture` para copiar el registro de perforación y el marcador en `artifacts/soranet_pq_rollout/<timestamp>/`, calcular los resúmenes de BLAKE3 y producir una firma `rollout_capture.json`.

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

Joignez les metadata generees et la firma au dossier de gobernanza.

## Revertir

Si el simulacro revela una verdadera penuria PQ, restezur sur Stage A, notifique a Networking TL y agregue las métricas recopiladas además de las diferencias del directorio de guardia en el rastreador de incidentes. Utilice la exportación de captura de directorio de guardia y todo para restaurar el servicio normal.

:::tip Cobertura de regresión
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` proporciona la validación sintética que necesita para este ejercicio.
:::