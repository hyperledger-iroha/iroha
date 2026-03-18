---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de operações
título: SoraFS آپریشنز پلے بک
sidebar_label: آپریشنز پلے بک
description: SoraFS آپریٹرز کے لیے resposta a incidentes گائیڈز اور broca de caos طریقۂ کار۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs_ops_playbook.md` میں برقرار رکھے گئے runbook کی عکاسی کرتا ہے۔ جب تک Esfinge ڈاکیومنٹیشن مکمل طور پر منتقل نہ ہو جائے دونوں نقول کو ہم آہنگ رکھیں۔
:::

## کلیدی حوالہ جات

- Ativos de observabilidade: `dashboards/grafana/` میں Painéis Grafana e `dashboards/alerts/` میں Regras de alerta Prometheus دیکھیں۔
- Catálogo métrico: `docs/source/sorafs_observability_plan.md`.
- Superfícies de telemetria do orquestrador: `docs/source/sorafs_orchestrator_plan.md`.

## Matriz de escalonamento

| ترجیح | ٹرگر مثالیں | بنیادی de plantão | بیک اپ | Não |
|----|-------------|---------------|--------|------|
| P1 | عالمی interrupção do gateway, taxa de falha PoR > 5% (15 meses), backlog de replicação ہر 10 منٹ میں دوگنا | Armazenamento SRE | Observabilidade TL | اگر اثر 30 منٹ سے زیادہ ہو تو conselho de governança کو envolver کریں۔ |
| P2 | علاقائی latência do gateway violação de SLO, pico de novas tentativas do orquestrador بغیر SLA اثر کے | Observabilidade TL | Armazenamento SRE | lançamento جاری رکھیں مگر نئے manifesta portão کریں۔ |
| P3 | Alertas de غیر اہم (desatualização manifesta, capacidade 80–90%) | Triagem de admissão | Guilda de operações | اگلے کاروباری دن میں نمٹا دیں۔ |

## Interrupção do gateway/disponibilidade degradada

**Detecção**

- Alertas: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Painel: `dashboards/grafana/sorafs_gateway_overview.json`.

**Ações imediatas**

1. painel de taxa de solicitação کے ذریعے escopo کنفرم کریں (provedor único vs frota)۔
2. O multi-provedor e a configuração de operações (`docs/source/sorafs_gateway_self_cert.md`) ou `sorafs_gateway_route_weights` são os provedores de roteamento Torii e os provedores que você usa. کریں۔
3. Existem provedores de serviços de busca e clientes CLI/SDK que usam o substituto de “busca direta” (`docs/source/sorafs_node_client_protocol.md`).

**Triagem**

- `sorafs_gateway_stream_token_limit` کے مقابل utilização de token de fluxo دیکھیں۔
- TLS یا erros de admissão کے لیے logs de gateway inspecionam کریں۔
- `scripts/telemetry/run_schema_diff.sh` چلائیں تاکہ gateway کے esquema exportado کا versão esperada سے correspondência ثابت ہو۔

**Opções de correção**

- صرف متاثرہ processo de gateway ری اسٹارٹ کریں؛ پورے cluster کو reciclar نہ کریں جب تک متعدد fornecedores فیل نہ ہوں۔
- A saturação é کنفرم ہو تو limite de token de fluxo کو عارضی طور پر 10–15% بڑھائیں۔
- استحکام کے بعد autocertificação دوبارہ چلائیں (`scripts/sorafs_gateway_self_cert.sh`).

**Pós-incidente**

- `docs/source/sorafs/postmortem_template.md` استعمال کرتے ہوئے P1 postmortem فائل کریں۔
- remediação میں intervenções manuais شامل ہوں تو exercício de caos de acompanhamento شیڈول کریں۔

## Pico de falha de prova (PoR / PoTR)

**Detecção**

- Alertas: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Painel: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetria: eventos `torii_sorafs_proof_stream_events_total` e `sorafs.fetch.error` ou `provider_reason=corrupt_proof` ہو۔

**Ações imediatas**

1. registro de manifesto کو sinalizador کر کے نئی congelamento de admissões de manifesto کریں (`docs/source/sorafs/manifest_pipeline.md`).
2. Governança کو notificar کریں تاکہ متاثرہ provedores کے incentivos pausar ہوں۔

**Triagem**- Profundidade da fila de desafio PoR کو `sorafs_node_replication_backlog_total` کے مقابل چیک کریں۔
- حالیہ implantações کے لیے pipeline de verificação de prova (`crates/sorafs_node/src/potr.rs`) validar کریں۔
- versões de firmware do provedor کو registro do operador سے comparar کریں۔

**Opções de correção**

- تازہ ترین manifest کے ساتھ `sorafs_cli proof stream` استعمال کر کے PoR replays trigger کریں۔
- اگر provas مسلسل falhar ہوں تو registro de governança اپڈیٹ کر کے provedor کو conjunto ativo سے نکالیں اور placares do orquestrador کو atualizar کرنے پر مجبور کریں۔

**Pós-incidente**

- اگلے پروڈکشن implantar سے پہلے cenário de simulação de caos PoR چلائیں۔
- modelo postmortem میں captura de lições کریں اور lista de verificação de qualificação do provedor اپڈیٹ کریں۔

## Atraso na replicação/crescimento do backlog

**Detecção**

- Alertas: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. `dashboards/alerts/sorafs_capacity_rules.yml` Cartão de crédito `dashboards/alerts/sorafs_capacity_rules.yml`
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  promoção سے پہلے چلائیں تاکہ Limites documentados do Alertmanager کو refletir کرے۔
- Painel: `dashboards/grafana/sorafs_capacity_health.json`.
- Métricas: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Ações imediatas**

1. backlog کا escopo چیک کریں (provedor único یا frota) اور غیر ضروری tarefas de replicação روکیں۔
2. Backlog isolado e agendador de replicação کے ذریعے نئے pedidos کو عارضی طور پر provedores alternativos کو reatribuir کریں۔

**Triagem**

- telemetria do orquestrador میں rajadas de repetição دیکھیں جو backlog بڑھا سکتے ہیں۔
- metas de armazenamento کے لیے headroom کنفرم کریں (`sorafs_node_capacity_utilisation_percent`).
- حالیہ alterações de configuração (atualizações de perfil em partes, cadência de prova) ریویو کریں۔

**Opções de correção**

- `sorafs_cli` کو `--rebalance` کے ساتھ چلائیں تاکہ redistribuir conteúdo ہو۔
- Provedor متاثرہ کے لیے trabalhadores de replicação کو escala horizontalmente کریں۔
- Alinhamento de janelas TTL کرنے کے لیے gatilho de atualização de manifesto کریں۔

**Pós-incidente**

- falha de saturação do provedor پر مرکوز broca de capacidade شیڈول کریں۔
- Documentação de SLA de replicação کو `docs/source/sorafs_node_client_protocol.md` میں اپڈیٹ کریں۔

## Cadência de treino do caos

- **Trimestralmente**: interrupção combinada do gateway + simulação de tempestade de novas tentativas do orquestrador.
- **Bianual**: Injeção de falha PoR/PoTR دو provedores پر recuperação کے ساتھ.
- **Verificação mensal**: manifestos de teste em um cenário de atraso de replicação.
- exercícios e log de runbook compartilhado (`ops/drill-log.md`) میں اس طرح ٹریک کریں:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- commits para validar log:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- آنے والے exercícios کے لیے `--status scheduled`, مکمل executa کے لیے `pass`/`fail`, اور کھلے itens de ação کے لیے `follow-up` Cartão de crédito
- simulações یا verificação automatizada کے لیے destino `--log` سے substituição کریں؛ ورنہ اسکرپٹ `ops/drill-log.md` کو اپڈیٹ کرتا رہے گا۔

## Modelo pós-morte

ہر Incidente P1/P2 e retrospectivas de exercícios de caos کے لیے `docs/source/sorafs/postmortem_template.md` استعمال کریں۔ یہ modelo de cronograma, quantificação de impacto, fatores contribuintes, ações corretivas, e tarefas de verificação de acompanhamento کور کرتا ہے۔