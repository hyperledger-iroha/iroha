---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-default-lane-quickstart
título: faixa padrão کوئیک اسٹارٹ (NX-5)
sidebar_label: faixa padrão
description: Nexus کے default lane fallback کو configure e verify کریں تاکہ Torii اور SDKs public lanes میں lane_id omit کر سکیں۔
---

:::nota Fonte Canônica
یہ صفحہ `docs/source/quickstart/default_lane.md` کی عکاسی کرتا ہے۔ جب تک varredura de localização پورٹل تک نہیں پہنچتی, دونوں کاپیوں کو alinhado رکھیں۔
:::

# faixa padrão کوئیک اسٹارٹ (NX-5)

> **Contexto do roteiro:** NX-5 - integração padrão de via pública۔ runtime `nexus.routing_policy.default_lane` fallback ظاہر کرتا ہے تاکہ Torii REST/gRPC endpoints اور ہر SDK اس وقت `lane_id` محفوظ طریقے سے omitir کر سکیں جب ٹریفک via pública canônica سے تعلق رکھتا ہو۔ یہ گائیڈ operadores کو configuração de catálogo کرنے, `/status` میں verificação de fallback کرنے, اور exercício de comportamento do cliente de ponta a ponta کرنے میں رہنمائی کرتی ہے۔

## Pré-requisitos

- `irohad` کا Sora/Nexus build ( `irohad --sora --config ...` چلائیں ).
- repositório de configuração تک رسائی تاکہ `nexus.*` seções editar کیے جا سکیں۔
- `iroha_cli` جو cluster de destino سے بات کرنے کے لئے configurado ہو۔
- Torii `/status` inspeção de carga útil کرنے کے لئے `curl`/`jq` (یا equivalente).

## 1. pista no catálogo de espaço de dados بیان کریں

rede پر موجود ہونے والے pistas اور espaços de dados کو declarar کریں۔ Este é o snippet (`defaults/nexus/config.toml` سے) nas vias públicas e no registro de aliases de espaço de dados correspondente.

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

ہر `index` منفرد اور ہونا چاہیے۔ Valores de IDs de espaço de dados de 64 bits ہیں؛ اوپر والے مثالیں وضاحت کے لئے índices de pista کے برابر valores numéricos استعمال کرتی ہیں۔

## 2. padrões de roteamento e substituições opcionais

`nexus.routing_policy` سیکشن pista de fallback کو controle کرتا ہے اور مخصوص instruções یا prefixos de conta کے لئے substituição de roteamento کرنے دیتا ہے۔ اگر کوئی correspondência de regra نہ کرے تو agendador ٹرانزیکشن کو configurado `default_lane` اور `default_dataspace` پر rota کرتا ہے۔ Lógica do roteador `crates/iroha_core/src/queue/router.rs` میں ہے اور Torii Superfícies REST/gRPC پر پالیسی شفاف انداز میں apply کرتا ہے۔

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```


## 3. پالیسی کے ساتھ inicialização do nó کریں

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

inicialização do nó کے دوران política de roteamento derivada لاگ کرتا ہے۔ کوئی بھی erros de validação (índices ausentes, aliases duplicados, ids de espaço de dados inválidos) fofocas

## 4. estado de governança da pista کنفرم کریں

node online ہونے کے بعد، CLI helper استعمال کریں تاکہ faixa padrão selada (manifesto carregado) اور tráfego کے لئے pronto ہو۔ Visualização resumida da faixa کے لئے ایک linha پرنٹ کرتا ہے:

```bash
iroha_cli app nexus lane-report --summary
```

Exemplo de saída:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

A faixa padrão `sealed` é definida como permissão de tráfego externo کرنے سے پہلے runbook de governança de faixa فالو کریں۔ `--fail-on-sealed` sinalizador CI کے لئے مفید ہے۔

## 5. Cargas úteis de status Torii inspecionam کریں

Política de roteamento de resposta `/status` اور فی-lane Scheduler Snapshot دونوں Expor کرتا ہے۔ `curl`/`jq` استعمال کر کے padrões configurados کی تصدیق کریں اور چیک کریں کہ produção de telemetria de pista alternativa کر رہا ہے:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Exemplo de saída:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

lane `0` کے لئے contadores do agendador ao vivo دیکھنے کے لئے:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

یہ کنفرم کرتا ہے کہ TEU snapshot, metadados de alias, e configuração de sinalizadores de manifesto کے ساتھ alinhar ہیں۔ یہی painéis Grafana de carga útil کے painel de ingestão de pista میں استعمال ہوتا ہے۔

## 6. exercício de padrões do cliente کریں

- **Rust/CLI.** `iroha_cli` اور Rust client crate `lane_id` campo کو omitir کرتے ہیں جب آپ `--lane-id` / `LaneSelector` pass نہیں کرتے۔ O roteador de fila `default_lane` é um substituto para o roteador de fila Sinalizadores `--lane-id`/`--dataspace-id` explícitos صرف pista não padrão کو alvo کرتے وقت استعمال کریں۔
- **JS / Swift / Android. کرتے ہیں۔ Política de roteamento کو preparação اور produção میں sincronização رکھیں تاکہ aplicativos móveis کو reconfigurações de emergência نہ کرنی پڑیں۔
- **Testes de pipeline/SSE.** filtros de eventos de transação Predicados `tx_lane_id == <u32>` قبول کرتے ہیں (دیکھیں `docs/source/pipeline.md`). `/v2/pipeline/events/transactions` کو اس filter کے ساتھ subscribe کریں تاکہ یہ ثابت ہو کہ pista explícita کے بغیر بھیجی گئی grava o ID da pista de fallback کے تحت پہنچتی ہیں۔

## 7. Observabilidade e ganchos de governança- `/status` `nexus_lane_governance_sealed_total` e `nexus_lane_governance_sealed_aliases` بھی publicar کرتا ہے تاکہ Alertmanager warning کر سکے جب کوئی lane اپنا manifest کھو sim ان alertas کو devnets میں بھی habilitado رکھیں۔
- mapa de telemetria do agendador e painel de governança de pista (`dashboards/grafana/nexus_lanes.json`) catálogo کے campos de alias/slug esperados کرتے ہیں۔ اگر آپ alias renomear کریں تو متعلقہ Diretórios Kura کو relabel کریں تاکہ auditores caminhos determinísticos رکھ سکیں (NX-1 کے تحت faixa ہوتا ہے)۔
- faixas padrão کے لئے aprovações do parlamento میں plano de reversão شامل ہونا چاہیے۔ hash de manifesto اور evidência de governança کو اس início rápido کے ساتھ اپنے runbook do operador میں registro کریں تاکہ rotações futuras مطلوبہ estado کا اندازہ نہ لگائیں۔