---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: pq-ratchet-runbook
título: Taladro contra incendios con trinquete SoraNet PQ
sidebar_label: Manual de ejecución de PQ Ratchet
Descripción: Política de anonimato de PQ y promoción para promover y degradar a pasos de ensayo de guardia y validación de telemetría determinista.
---

:::nota Fuente canónica
یہ صفحہ `docs/source/soranet/pq_ratchet_runbook.md` کی عکاسی کرتا ہے۔ جب تک پرانا conjunto de documentación retirar نہ ہو، دونوں کاپیاں sincronización رکھیں۔
:::

## مقصد

یہ runbook SoraNet کی política de anonimato poscuántica (PQ) por etapas کے لئے secuencia de simulacro de incendio گائیڈ کرتا ہے۔ Promoción de operadores (Etapa A -> Etapa B -> Etapa C) اور Suministro de PQ کم ہونے پر degradación controlada واپس Etapa B/A دونوں ensayo کرتے ہیں۔ Ganchos de telemetría de perforación (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) validan کرتا ہے اور registro de ensayo de incidentes کے لئے artefactos جمع کرتا ہے۔

## Requisitos previos

- Ponderación de capacidad کے ساتھ تازہ ترین `sorafs_orchestrator` binario (referencia de perforación de confirmación کے برابر یا بعد میں جو `docs/source/soranet/reports/pq_ratchet_validation.md` میں دکھایا گیا ہے)۔
- Prometheus/Grafana pila تک رسائی جو `dashboards/grafana/soranet_pq_ratchet.json` servir کرتا ہے۔
- Instantánea del directorio de guardia nominal۔ taladrar سے پہلے copiar buscar اور verificar کریں:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Directorio de origen, publicación JSON, ayudantes de rotación, `soranet-directory build`, Norito, recodificación binaria.

- CLI para captura de metadatos y artefactos de rotación del emisor antes de la etapa:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```- Networking اور observabilidad equipos de guardia کی منظور شدہ cambiar ventana۔

## Pasos de promoción

1. **Auditoría de etapa**

   ابتدا کا etapa ریکارڈ کریں:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Promoción سے پہلے `anon-guard-pq` esperar کریں۔

2. **Etapa B (PQ mayoritaria) para promover کریں**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Actualización de manifiestos ہونے کے لئے >=5 منٹ انتظار کریں۔
   - Grafana میں (tablero `SoraNet PQ Ratchet Drill`) Panel "Eventos de política" پر `stage=anon-majority-pq` کے لئے `outcome=met` کی تصدیق کریں۔
   - Captura de pantalla یا panel de captura JSON کریں اور registro de incidentes میں adjuntar کریں۔

3. **Etapa C (PQ estricto) para promover کریں**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Histogramas `sorafs_orchestrator_pq_ratio_*` کو 1.0 کی طرف جاتا ہوا verificar کریں۔
   - Contador de caída de tensión کا plano رہنا confirmar کریں؛ ورنہ pasos de degradación فالو کریں۔

## Simulacro de degradación/apagón

1. **Escasez de PQ sintético پیدا کریں**

   Entorno de juegos میں directorio de guardia کو صرف entradas clásicas تک recortar کر کے Los relés PQ desactivan کریں، پھر recarga de caché del orquestador کریں:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observación de telemetría de apagón کریں**

   - Panel de control: panel "Tasa de apagones" 0 سے اوپر pico کرے۔
   -PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` کو `anonymity_outcome="brownout"` اور `anonymity_reason="missing_majority_pq"` رپورٹ کرنا چاہئے۔

3. **Etapa B / Etapa A para degradar کریں**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```اگر suministro de PQ اب بھی ناکافی ہو تو `anon-guard-pq` پر degradar کریں۔ Taladro اس وقت مکمل ہوتا ہے جب los contadores de caídas de tensión se resuelven ہوں اور promociones دوبارہ لاگو ہو سکیں۔

4. **Directorio de guardia بحال کریں**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetría y artefactos

- **Panel de control:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** یقینی بنائیں کہ `sorafs_orchestrator_policy_events_total` alerta de caída configurada SLO کے نیچے رہے (&lt;5% کسی بھی ventana de 10 minutos میں)۔
- **Registro de incidentes:** fragmentos de telemetría y notas del operador کو `docs/examples/soranet_pq_ratchet_fire_drill.log` میں anexar کریں۔
- **Captura firmada:** `cargo xtask soranet-rollout-capture` استعمال کریں تاکہ registro de perforación اور marcador کو `artifacts/soranet_pq_rollout/<timestamp>/` میں copia کیا جا سکے، BLAKE3 resúmenes computar ہوں، اور firmado `rollout_capture.json` Negro

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

Metadatos generados اور firma کو paquete de gobernanza کے ساتھ adjuntar کریں۔

## Revertir

اگر simulacro حقیقی Escasez de PQ ظاہر کرے تو Etapa A پر رہیں، Redes TL کو مطلع کریں، اور métricas recopiladas کے ساتھ guardar diferencias de directorio کو rastreador de incidentes میں adjuntar کریں۔ پہلے capturar کیا گیا guardar directorio exportar استعمال کر کے restauración del servicio normal کریں۔

:::tip Cobertura de regresión
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` اس taladro کو soporte کرنے والی validación sintética فراہم کرتا ہے۔
:::