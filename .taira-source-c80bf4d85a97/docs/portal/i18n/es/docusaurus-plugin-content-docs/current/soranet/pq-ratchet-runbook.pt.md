---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: pq-ratchet-runbook
título: Simulacro PQ Ratchet de SoraNet
sidebar_label: Runbook de PQ Ratchet
descripción: Pasos de ensayo de guardia para promover o rebajar una política de anonimato PQ en estaciones con validación determinística de telemetría.
---

:::nota Fuente canónica
Esta página espelha `docs/source/soranet/pq_ratchet_runbook.md`. Mantenha ambas como copias sincronizadas.
:::

## propuesta

Este runbook guía una secuencia de simulacro para una política de anonimato post-cuántico (PQ) en estaciones de SoraNet. Los operadores ensaiam promocao (Etapa A -> Etapa B -> Etapa C) y un despromocao controlado de volta a Etapa B/A cuando a oferta de PQ cai. El simulacro valida ganchos de telemetría (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) y coleta artefatos para el registro de ensayo de incidentes.

##Requisitos previos

- Último binario `sorafs_orchestrator` con ponderación de capacidad (compromiso igual o posterior a la referencia del taladro mostrado en `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Acceso a la pila Prometheus/Grafana que sirve a `dashboards/grafana/soranet_pq_ratchet.json`.
- Instantánea nominal del directorio de protección. Busque y valide una copia antes del simulacro:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Si el directorio fuente se publica solo en JSON, vuelva a codificar el binario Norito con `soranet-directory build` antes de rodar los ayudantes de rotación.

- Capture metadatos y artefactos de rotación previos a la etapa del emisor con CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```- Janela de mudanca aprobó pelos times on call de networking e observability.

## Pasos de promoción

1. **Auditoría de etapa**

   Registro o etapa inicial:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Espere `anon-guard-pq` antes de la promoción.

2. **Promova para Etapa B (PQ Mayoritaria)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Aguarde >=5 minutos para manifiestos serem atualizados.
   - No Grafana (panel de control `SoraNet PQ Ratchet Drill`) confirma que el panel "Eventos de política" muestra `outcome=met` para `stage=anon-majority-pq`.
   - Capture una captura de pantalla o JSON desde el panel y el anexo al registro de incidentes.

3. **Promova para Etapa C (PQ estricto)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Verifique que los histogramas `sorafs_orchestrator_pq_ratio_*` tengan la versión 1.0.
   - Confirme que el contador de apagón permanece plano; caso contrario, siga los pasos de despromocao.

## Despromocao / simulacro de apagón

1. **Induza uma escassez sintetica de PQ**

   Relés desativos PQ no ambiente patio de recreo reduzindo o guard directorio a entradas clásicas apenas, depois recarregue o cache do Orchestrator:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observar una telemetría de caída de tensión**

   - Panel de control: el panel "Tasa de apagones" es superior a 0.
   -PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` deve reportar `anonymity_outcome="brownout"` con `anonymity_reason="missing_majority_pq"`.

3. **Despromova para Etapa B / Etapa A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```Se a oferta de PQ aún por insuficiente, despromova para `anon-guard-pq`. El simulacro termina cuando los contadores de apagón se estabilizan y las promociones pueden ser reaplicadas.

4. **Restauración del directorio de guardia**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetría y artefactos

- **Panel de control:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** garantía de que o alerta de apagón de `sorafs_orchestrator_policy_events_total` fique abaixo do SLO configurado (&lt;5% em qualquer janela de 10 minutos).
- **Registro de incidentes:** anexe trechos de telemetria e notas do operador em `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Captura assinada:** use `cargo xtask soranet-rollout-capture` para copiar el registro de perforación y el marcador para `artifacts/soranet_pq_rollout/<timestamp>/`, calcular resúmenes BLAKE3 y producir un `rollout_capture.json` assinado.

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

Anexo de los metadatos generados y la assinatura del paquete de gobernanza.

## Revertir

Si el simulacro revela escassez real de PQ, permanece en la Etapa A, notifique o Networking TL y anexe como métricas recopiladas junto con las diferencias del directorio de guardia y el rastreador de incidentes. Utilice o exporte el directorio guardado capturado anteriormente para restaurar el servicio normal.

:::tip Cobertura de regressao
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` fornece a validacao sintetica que sustenta este simulacro.
:::