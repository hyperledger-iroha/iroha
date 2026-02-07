---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: pq-ratchet-runbook
título: Учебная тревога PQ Ratchet SoraNet
sidebar_label: Trinquete Runbook PQ
descripción: Ensayo de guardia para la política de anonimato de PQ y telemetría determinada.
---

:::nota Канонический источник
Esta página está cerrada `docs/source/soranet/pq_ratchet_runbook.md`. Asegúrese de copiar las copias sincronizadas, ya que los documentos disponibles no contienen información exclusiva.
:::

## Назначение

Este runbook describe el simulacro de incendio posterior a la política de anonimato post-cuántica (PQ) de SoraNet. Los operadores participan en la promoción (Etapa A -> Etapa B -> Etapa C), así como en la degradación controlada en la Etapa B/A para suministrar PQ. Perforar ganchos de telemetría válidos (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) y recuperar artefactos del registro de ensayo de incidentes.

## Requisitos previos

- Самый свежий `sorafs_orchestrator` binario con ponderación de capacidad (comprometer равен или позже ejercicio de referencia из `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Descarga en la pila Prometheus/Grafana, que está obligatoriamente en `dashboards/grafana/soranet_pq_ratchet.json`.
- Instantánea del directorio de guardia nominal. Busque y guarde copias del ejercicio:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Si el directorio fuente está publicado en JSON, utilice el binario Norito y el `soranet-directory build` antes de usar los ayudantes de rotación.

- Actualizar metadatos y artefactos previos a la etapa del emisor mediante la CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```- Cambiar ventana de одобрен de guardia командами networking y observabilidad.

## Pasos de promoción

1. **Auditoría de etapa**

   Зафиксируйте стартовый etapa:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Перед promoción ожидайте `anon-guard-pq`.

2. **Promoción en Etapa B (PQ mayoritaria)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Подождите >=5 minут для обновления manifiestos.
   - En Grafana (panel de control `SoraNet PQ Ratchet Drill`), coloque el panel "Eventos de política" en `outcome=met` para `stage=anon-majority-pq`.
   - Realice capturas de pantalla o paneles JSON y registre el registro de incidentes.

3. **Promoción en etapa C (PQ estricto)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Proverte, что histograma `sorafs_orchestrator_pq_ratio_*` стремится к 1.0.
   - Tenga cuidado con el contador de caídas de tensión; иначе выполните шаги degradación.

## Simulacro de degradación/apagón

1. **Deficiencia sintética PQ**

   Coloque los relés PQ en la página de juegos, elimine el directorio de guardia de las entradas clásicas y guarde la caché del orquestador:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Apagón de telemetría**

   - Panel de control: el panel "Tasa de apagones" подскакивает выше 0.
   -PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` conecta `anonymity_outcome="brownout"` con `anonymity_reason="missing_majority_pq"`.

3. **Degradación a la Etapa B / Etapa A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Si el suministro de PQ no es necesario, consulte `anon-guard-pq`. Taladro завершен, когда contadores de apagones стабилизируются и promociones можно повторно применить.4. **Directorio de guardia de Восстановление**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetría y artefactos

- **Panel de control:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** убедитесь, что brownout alert `sorafs_orchestrator_policy_events_total` остается ниже настроенного SLO (&lt;5% в любом 10-минутном окне).
- **Registro de incidentes:** solicite fragmentos de telemetría y guarde el operador en `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Captura firmada:** utilice `cargo xtask soranet-rollout-capture`, consulte el registro de perforación y el marcador en `artifacts/soranet_pq_rollout/<timestamp>/`, consulte los resúmenes de BLAKE3 y los archivos adjuntos. `rollout_capture.json`.

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

Utilice metadatos y firmas de generación para la gobernanza del paquete.

## Revertir

Si taladra una verdadera necesidad de PQ, instale la Etapa A, elimine Networking TL y aplique métricas sobrantes con diferencias en el directorio de guardia y rastreador de incidentes. Utilice la opción de exportar el directorio de guardia, ya que necesita servicios normales.

:::tip Cobertura de regresión
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` previa validación sintética, que permite este taladro.
:::