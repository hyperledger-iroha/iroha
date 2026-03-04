---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-ratchet-runbook
título: Simulação de incêndio com catraca SoraNet PQ
sidebar_label: Runbook PQ Ratchet
descrição: مرحلہ وار Política de anonimato PQ کو promover یا rebaixar کرنے کے لئے etapas de ensaio de plantão e validação de telemetria determinística.
---

:::nota Fonte Canônica
یہ صفحہ `docs/source/soranet/pq_ratchet_runbook.md` کی عکاسی کرتا ہے۔ جب تک پرانا conjunto de documentação retirar نہ ہو، دونوں کاپیاں sincronização رکھیں۔
:::

## مقصد

یہ runbook SoraNet کی política de anonimato pós-quântica encenada (PQ) کے لئے sequência de simulação de incêndio گائیڈ کرتا ہے۔ Promoção de operadores (Estágio A -> Estágio B -> Estágio C) اور Fornecimento de QP کم ہونے پر rebaixamento controlado واپس Estágio B/A دونوں ensaio کرتے ہیں۔ Ganchos de telemetria de perfuração (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) validam کرتا ہے اور registro de ensaio de incidente کے لئے artefatos جمع کرتا ہے۔

## Pré-requisitos

- Ponderação de capacidade کے ساتھ تازہ ترین `sorafs_orchestrator` binário (commit drill reference کے برابر یا بعد میں جو `docs/source/soranet/reports/pq_ratchet_validation.md` میں دکھایا گیا ہے)۔
- Pilha Prometheus/Grafana تک رسائی جو `dashboards/grafana/soranet_pq_ratchet.json` serve کرتا ہے۔
- Instantâneo do diretório de guarda nominal۔ drill سے پہلے copy fetch e verify کریں:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

اگر diretório de origem صرف JSON publicar کرتا ہے تو ajudantes de rotação چلانے سے پہلے `soranet-directory build` سے اسے Norito binário میں recodificar کریں۔

- CLI سے captura de metadados کریں اور artefatos de rotação do emissor pré-estágio کریں:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Networking e observabilidade de equipes de plantão کی منظور شدہ alterar janela۔

## Etapas da promoção

1. **Auditoria de etapa**

   ابتدا کا estágio ریکارڈ کریں:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Promoção سے پہلے `anon-guard-pq` esperar کریں۔

2. **Estágio B (Maioria PQ) para promover o کریں**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Atualização de manifestos ہونے کے لئے >=5 منٹ انتظار کریں۔
   - Grafana میں (painel `SoraNet PQ Ratchet Drill`) Painel "Eventos de política" پر `stage=anon-majority-pq` کے لئے `outcome=met` کی تصدیق کریں۔
   - Captura de tela یا painel Captura JSON کریں اور registro de incidentes میں anexar کریں۔

3. **Estágio C (PQ estrito) para promover o negócio**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Histogramas `sorafs_orchestrator_pq_ratio_*` کو 1.0 کی طرف جاتا ہوا verificar کریں۔
   - Contador de brownout کا flat رہنا confirmar کریں؛ ورنہ etapas de rebaixamento فالو کریں۔

## Exercício de rebaixamento/queda de energia

1. **Escassez de PQ sintético پیدا کریں**

   Ambiente de playground میں diretório de proteção کو صرف entradas clássicas تک trim کر کے Relés PQ desativados کریں, پھر recarga de cache do orquestrador کریں:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. ** Observar telemetria de brownout کریں **

   - Painel: painel "Taxa de brownout" 0 سے اوپر pico کرے۔
   -PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` کو `anonymity_outcome="brownout"` e `anonymity_reason="missing_majority_pq"` رپورٹ کرنا چاہئے۔

3. **Estágio B / Estágio A é rebaixado **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   O fornecimento de PQ é um problema que pode ser feito por `anon-guard-pq` para rebaixar o valor Broca اس وقت مکمل ہوتا ہے جب contadores de queda de energia se instalam ہوں اور promoções دوبارہ لاگو ہو سکیں۔

4. **Guardar diretório بحال کریں**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetria e artefatos- **Painel:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** یقینی بنائیں کہ Alerta de brownout `sorafs_orchestrator_policy_events_total` configurado SLO کے نیچے رہے (&lt;5% کسی بھی janela de 10 minutos میں)۔
- **Registro de incidentes:** trechos de telemetria e notas do operador کو `docs/examples/soranet_pq_ratchet_fire_drill.log` میں anexar کریں۔
- **Captura assinada:** `cargo xtask soranet-rollout-capture` استعمال کریں تاکہ log de perfuração e placar کو `artifacts/soranet_pq_rollout/<timestamp>/` میں copiar کیا جا سکے, BLAKE3 digere cálculo ہوں، Ele assinou `rollout_capture.json` بنے۔

Exemplo:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Metadados gerados اور assinatura کو pacote de governança کے ساتھ anexar کریں۔

## Reversão

اگر broca حقیقی Escassez de PQ ظاہر کرے تو Estágio A پر رہیں, Networking TL کو مطلع کریں, اور métricas coletadas کے ساتھ guarda diretório diffs کو rastreador de incidentes میں anexar کریں۔ پہلے captura کیا گیا guarda exportação de diretório استعمال کر کے restauração de serviço normal کریں۔

:::tip Cobertura de regressão
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` اس broca کو suporte کرنے والی validação sintética فراہم کرتا ہے۔
:::