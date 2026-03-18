---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-ops
título: Pin Registry عمليات
sidebar_label: Pin Registry عمليات
description: SoraFS کے Pin Registry e replicação SLA میٹرکس کی نگرانی اور ٹرائج۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/pin_registry_ops.md` کی عکاسی کرتا ہے۔ جب تک پرانی Esfinge دستاویزات ریٹائر نہ ہوں دونوں ورژنز کو ہم آہنگ رکھیں۔
:::

## جائزہ

یہ runbook بیان کرتا ہے کہ SoraFS کے Pin Registry اور replicação کے سروس لیول ایگریمنٹس (SLA) کی نگرانی اور ٹرائج کیسے کیا جائے۔ میٹرکس `iroha_torii` سے آتی ہیں اور Prometheus کے ذریعے `torii_sorafs_*` namespace میں ایکسپورٹ ہوتی ہیں۔ Torii پس منظر میں registro اسٹیٹ کو ہر 30 سیکنڈ پر سیمپل کرتا ہے، اس لیے ڈیش بورڈز اپ ٹو ڈیٹ رہتے ہیں چاہے کوئی آپریٹر `/v1/sorafs/pin/*` endpoints کو پول نہ کر رہا ہو۔ تیار شدہ ڈیش بورڈ (`docs/source/grafana_sorafs_pin_registry.json`) امپورٹ کریں تاکہ Grafana کا ایک تیار layout ملے جو نیچے O que você precisa saber sobre o que fazer

## میٹرک حوالہ

| میٹرک | Etiquetas | وضاحت |
| ----- | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | آن چین manifesta کا انوینٹری لائف سائیکل اسٹیٹ کے مطابق۔ |
| `torii_sorafs_registry_aliases_total` | — | registro میں ریکارڈ شدہ فعال aliases de manifesto کی تعداد۔ |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | pedidos de replicação کا backlog اسٹیٹس کے لحاظ سے۔ |
| `torii_sorafs_replication_backlog_total` | — | سہولت کے لیے medidor جو `pending` pedidos کو ظاہر کرتا ہے۔ |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA کاؤنٹنگ: `met` وقت پر مکمل ہونے والے pedidos کو گنتا ہے, `missed` دیر سے مکمل ہونا + expirações جمع کرتا ہے، `pending` زیر التواء pedidos کو ظاہر کرتا ہے۔ |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | latência de conclusão مجموعی طور پر (emissão e conclusão کے درمیان épocas)۔ |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | ordens pendentes کی janelas de folga (prazo menos época de emissão)۔ |

تمام medidores ہر snapshot pull پر ری سیٹ ہوتے ہیں, اس لیے ڈیش بورڈز کو `1m` یا اس سے تیز cadência پر amostra کرنا چاہیے۔

## Grafana ڈیش بورڈ

ڈیش بورڈ JSON میں سات پینلز ہیں جو آپریٹر ورک فلو کور کرتے ہیں۔ O que você precisa saber sobre o que fazer com o dinheiro

1. **Ciclo de vida do manifesto** – `torii_sorafs_registry_manifests_total` (grupo `status` کے مطابق).
2. **Tendência do catálogo de alias** – `torii_sorafs_registry_aliases_total`.
3. **Fila de pedidos por status** – `torii_sorafs_registry_orders_total` (grupo `status` کے مطابق).
4. **Backlog vs pedidos expirados** – `torii_sorafs_replication_backlog_total` e `torii_sorafs_registry_orders_total{status="expired"}` کو ملا کر saturação دکھاتا ہے۔
5. **Taxa de sucesso do SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latência vs folga de prazo** – `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` e `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}` کو اوورلے کریں۔ جب piso com folga absoluta چاہیے ہو تو Grafana transformações سے `min_over_time` visualizações شامل کریں, مثال:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Pedidos perdidos (taxa de 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Limites de alerta- **Sucesso de SLA  0**
  Limite: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Ação: manifestos de governança دیکھیں تاکہ desistência de provedores کی تصدیق ہو۔
- **Conclusão p95 > média de folga no prazo**
  Limite: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Ação: تصدیق کریں کہ prazos dos provedores سے پہلے commit کر رہے ہیں؛ reatribuições

### Prometheus Prometheus

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA ہدف سے کم"
          description: "SLA کامیابی کا تناسب 15 منٹ تک 95% سے کم رہا۔"

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog حد سے اوپر"
          description: "زیر التواء replication orders مقررہ backlog بجٹ سے بڑھ گئے۔"

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders ختم ہو گئیں"
          description: "پچھلے پانچ منٹ میں کم از کم ایک replication order ختم ہوئی۔"
```

## Fluxo de trabalho de triagem

1. **وجہ کی شناخت**
   - Faltas de SLA بڑھیں اور backlog کم رہے تو fornecedores کارکردگی پر توجہ دیں (Falhas de PoR, conclusões tardias)۔
   - اگر backlog بڑھے اور faltas مستحکم ہوں تو admissão (`/v1/sorafs/pin/*`) چیک کریں تاکہ manifestos جو کونسل کی منظوری کے منتظر ہیں واضح ہوں۔
2. **Provedores کی حالت کی توثیق**
   - `iroha app sorafs providers list` چلائیں اور دیکھیں کہ اعلان کردہ صلاحیتیں replicação تقاضوں سے میل کھاتی ہیں۔
   - Medidores `torii_sorafs_capacity_*` چیک کریں تاکہ provisionado GiB اور PoR sucesso کی تصدیق ہو۔
3. **Replicação کی reatribuir**
   - جب folga do backlog (`stat="avg"`) 5 épocas سے نیچے جائے تو `sorafs_manifest_stub capacity replication-order` کے ذریعے نئے pedidos جاری کریں (manifesto/embalagem CAR `iroha app sorafs toolkit pack` استعمال کرتا ہے)۔
   - اگر aliases کے پاس فعال ligações de manifesto نہ ہوں تو governança کو مطلع کریں (`torii_sorafs_registry_aliases_total` میں غیر متوقع کمی)۔
4. **نتیجہ دستاویزی بنائیں**
   - Log de operações SoraFS میں carimbos de data e hora e resumos de manifesto کے ساتھ notas de incidente درج کریں۔
   - اگر نئے modos de falha یا painéis آئیں تو اس runbook کو اپڈیٹ کریں۔

## Plano de implementação

پروڈکشن میں política de cache de alias کو فعال یا سخت کرتے وقت یہ مرحلہ وار طریقہ اختیار کریں:1. **Configuração da configuração**
   - `iroha_config` میں `torii.sorafs_alias_cache` (usuário -> real) کو متفقہ TTLs اور Grace Windows کے ساتھ اپڈیٹ کریں: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`۔ padrões `docs/source/sorafs_alias_policy.md` کی پالیسی سے ملتے ہیں۔
   - SDKs são necessários para camadas de configuração de camadas de configuração (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` ligações Rust / NAPI / Python میں) para gateway de aplicação do cliente سے میل کھائے۔
2. **Preparação do teste de simulação**
   - config تبدیلی کو staging کلسٹر پر deploy کریں جو topologia de produção کی نقل کرے۔
   - `cargo xtask sorafs-pin-fixtures` چلائیں تاکہ canonical alias fixtures اب بھی decodificação اور ida e volta ہوں؛ کوئی incompatibilidade upstream drift دکھاتا ہے جسے پہلے درست کرنا ہوگا۔
   - `/v1/sorafs/pin/{digest}` e `/v1/sorafs/aliases` endpoints کو provas sintéticas کے ساتھ آزمائیں جو fresco, janela de atualização, expirado اور expirado کیسز کور کریں۔ Códigos de status HTTP, cabeçalhos (`Sora-Proof-Status`, `Retry-After`, `Warning`) e campos de corpo JSON e runbook para validar
3. **Produção de produção**
   - janela de alteração padrão میں نئی config رول آؤٹ کریں۔ پہلے Torii پر لاگو کریں, پھر node لاگز میں نئی پالیسی کی تصدیق کے بعد gateways/serviços SDK کو reiniciar کریں۔
   - `docs/source/grafana_sorafs_pin_registry.json` کو Grafana میں امپورٹ کریں (یا موجودہ dashboards اپڈیٹ کریں) اور alias painéis de atualização de cache کو espaço de trabalho NOC میں pin کریں۔
4. **Verificação pós-implantação**
   - 30 meses de `torii_sorafs_alias_cache_refresh_total` e `torii_sorafs_alias_cache_age_seconds` مانیٹر کریں۔ Curvas `error`/`expired` میں picos کو atualizar janelas کے ساتھ correlacionar ہونا چاہیے؛ غیر متوقع اضافہ ہو تو operadores کو provas de alias اور provedores کی صحت چیک کرنی چاہیے۔
   - Logs do lado do cliente میں وہی decisões de política نظر آئیں (SDKs obsoletos یا prova expirada پر erros دکھائیں گے)۔ avisos de cliente não são configurados para configuração
5. **Substituição**
   - اگر emissão de alias پیچھے رہ جائے اور janela de atualização بار بار ٹرگر ہو تو `refresh_window` اور `positive_ttl` بڑھا کر پالیسی عارضی طور پر نرم کریں, پھر reimplantar کریں۔ `hard_expiry` کو برقرار رکھیں تاکہ واقعی provas obsoletas رد ہوتے رہیں۔
   - اگر telemetria میں `error` contagens بلند رہیں تو پچھلا `iroha_config` instantâneo بحال کر کے پچھلی configuração پر واپس جائیں، پھر atrasos na geração de alias کی تفتیش کے لیے incidente کھولیں۔

## متعلقہ مواد

- `docs/source/sorafs/pin_registry_plan.md` — roteiro de implementação e contexto de governança۔
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — operações do trabalhador de armazenamento, o manual de registro کو مکمل کرتا ہے۔