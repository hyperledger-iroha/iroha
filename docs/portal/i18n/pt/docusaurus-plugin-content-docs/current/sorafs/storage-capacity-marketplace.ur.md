---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mercado de capacidade de armazenamento
título: SoraFS اسٹوریج کیپیسٹی مارکیٹ پلیس
sidebar_label: کیپیسٹی مارکیٹ پلیس
description: کیپیسٹی مارکیٹ پلیس, ordens de replicação, ٹیلی میٹری اور گورننس ہکس کے لیے SF-2c پلان۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/storage_capacity_marketplace.md` کی عکاسی کرتا ہے۔ جب تک پرانا ڈاکیومنٹیشن فعال ہے دونوں لوکیشنز کو ہم آہنگ رکھیں۔
:::

# SoraFS اسٹوریج کیپیسٹی مارکیٹ پلیس (SF-2c ڈرافٹ)

روڈ میپ آئٹم SF-2c گورنڈ مارکیٹ پلیس متعارف کراتا ہے جہاں اسٹوریج capacidade comprometida dos provedores کرتے ہیں، pedidos de replicação وصول کرتے ہیں، اور فراہم کردہ disponibilidade کے متناسب taxas کماتے ہیں۔ یہ دستاویز پہلی ریلیز کے لیے درکار entregas کا دائرہ کار طے کرتی ہے اور انہیں faixas acionáveis میں تقسیم کرتی ہے۔

## مقاصد

- provedores کی compromissos de capacidade (کل bytes، ہر faixa کے limites, expiração) کو ایک قابلِ تصدیق شکل میں بیان کرنا جو governança, transporte SoraNet اور Torii Torii Cartão de crédito
- capacidade declarada, participação e restrições de política کے مطابق pinos کو provedores میں تقسیم کرنا جبکہ comportamento determinístico برقرار رکھنا۔
- entrega de armazenamento (sucesso de replicação, tempo de atividade, provas de integridade) کو ناپنا اور distribuição de taxas کے لیے exportação de telemetria کرنا۔
- revogação اور disputa کے عمل فراہم کرنا تاکہ fornecedores desonestos کو penalizar یا remover کیا جا سکے۔

## ڈومین کانسیپٹس

| Conceito | Descrição | Entrega Inicial |
|--------|-------------|---------------------|
| `CapacityDeclarationV1` | Carga útil Norito, ID do provedor, suporte ao perfil do chunker, GiB comprometido, limites específicos da pista, dicas de preços, compromisso de piquetagem e expiração بیان کرتا ہے۔ | `sorafs_manifest::capacity` esquema + validador۔ |
| `ReplicationOrder` | governança کی جانب سے جاری instrução e manifesto CID کو ایک یا زائد provedores کو atribuir کرتی ہے، جس میں nível de redundância e métricas de SLA شامل ہیں۔ | Torii کے ساتھ مشترک Esquema Norito + API de contrato inteligente۔ |
| `CapacityLedger` | registro on-chain/off-chain, declarações de capacidade ativa, pedidos de replicação, métricas de desempenho e acúmulo de taxas. | módulo de contrato inteligente یا esboço de serviço fora da cadeia مع instantâneo determinístico۔ |
| `MarketplacePolicy` | política de governança جو participação mínima، requisitos de auditoria اور curvas de penalidade متعین کرتی ہے۔ | `sorafs_manifest` میں estrutura de configuração + documento de governança۔ |

### Esquemas Implementados (Status)

## ورک بریک ڈاؤن

### 1. Esquema e camada de registro| Tarefa | Proprietário(s) | Notas |
|------|----------|-------|
| `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1` کی تعریف۔ | Equipe de Armazenamento / Governança | Norito Cartão de crédito versionamento semântico e referências de capacidade |
| `sorafs_manifest` analisador + módulos validadores لاگو کریں۔ | Equipe de armazenamento | IDs monotônicos, limites de capacidade, requisitos de participação |
| metadados de registro chunker میں ہر perfil کے لیے `min_capacity_gib` شامل کریں۔ | GT Ferramentaria | clientes کو perfil کے حساب سے requisitos mínimos de hardware نافذ کرنے میں مدد۔ |
| `MarketplacePolicy` ڈاکیومنٹ ڈرافٹ کریں جو grades de proteção de admissão اور cronograma de penalidades بیان کرے۔ | Conselho de Governança | docs میں publicar کریں اور padrões de política کے ساتھ رکھیں۔ |

#### Definições de esquema (implementado)- `CapacityDeclarationV1` ہر provedor کے لیے compromissos de capacidade assinados کو captura کرتا ہے, جن میں identificadores de chunker canônicos, referências de capacidade, limites de faixa opcionais, dicas de preços, janelas de validade e metadados شامل ہیں۔ validação یہ یقینی بناتا ہے کہ aposta غیر صفر ہو، lida com nomes canônicos ہوں, aliases desduplicados ہوں, limites de pista اعلان کردہ total کے اندر ہوں اور Contabilidade GiB monotônica ہو۔【crates/sorafs_manifest/src/capacity.rs:28】
- Manifestos `ReplicationOrderV1` کو atribuições emitidas pela governança سے باندھتا ہے, جن میں metas de redundância, limites de SLA e garantias por atribuição شامل ہیں؛ identificadores de identificadores canônicos de chunker, provedores exclusivos e restrições de prazo کریں۔【crates/sorafs_manifest/src/capacity.rs:301】
- Instantâneos de época `CapacityTelemetryV1` (GIB declarados versus utilizados, contadores de replicação, porcentagens de tempo de atividade/PoR) verificações de limites استعمال کو declarações کے اندر اور porcentagens کو 0-100% میں رکھتے ہیں۔【crates/sorafs_manifest/src/capacity.rs:476】
- Ajudantes compartilhados (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores de pista/atribuição/SLA) validação de chave determinística اور relatório de erros فراہم کرتے ہیں جنہیں CI اور reutilização de ferramentas downstream کر سکتے ہیں۔【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` اب instantâneo on-chain کو `/v2/sorafs/capacity/state` کے ذریعے expor کرتا ہے، declarações de provedor اور entradas de razão de taxas کو Norito JSON determinístico کے پیچھے جوڑ کر۔【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- Cobertura de validação aplicação de identificador canônico, detecção de duplicatas, limites por pista, guardas de atribuição de replicação e verificações de intervalo de telemetria کو exercício کرتی ہے تاکہ regressões فوراً CI میں ظاہر ہوں۔【crates/sorafs_manifest/src/capacity.rs:792】
- Ferramentas do operador: especificações legíveis por humanos `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` e cargas úteis canônicas Norito, blobs base64 e resumos JSON میں تبدیل کرتا ہے تاکہ operadores `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry` اور dispositivos de ordem de replicação کو validação local کے ساتھ estágio کر سکیں۔【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Dispositivos de referência `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) میں ہیں اور `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order` سے gerar ہوتی ہیں۔

### 2. Integração do plano de controle| Tarefa | Proprietário(s) | Notas |
|------|----------|-------|
| `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry`, `/v2/sorafs/capacity/orders` Torii manipuladores e Norito cargas úteis JSON کے ساتھ شامل کریں۔ | Equipe Torii | lógica validadora کو espelho کریں؛ Ajudantes JSON Norito reutilizam کریں۔ |
| Instantâneos `CapacityDeclarationV1` کو metadados do placar do orquestrador اور planos de busca do gateway میں propagar کریں۔ | GT Ferramentaria / Equipe Orquestrador | `provider_metadata` میں referências de capacidade شامل کریں تاکہ limites de pista de pontuação multi-fonte کا خیال رکھے۔ |
| ordens de replicação کو clientes orquestradores/gateway میں feed کریں تاکہ atribuições اور dicas de failover ڈرائیو ہوں۔ | Equipe de TL/Gateway de rede | Ordens de replicação assinadas pela governança do construtor de placar استعمال کرتا ہے۔ |
| Ferramentas CLI: `sorafs_cli` `capacity declare`, `capacity telemetry`, `capacity orders import` | GT Ferramentaria | saídas determinísticas JSON + placar |

### 3. Política e governança do mercado

| Tarefa | Proprietário(s) | Notas |
|------|----------|-------|
| `MarketplacePolicy` (aposta mínima, multiplicadores de penalidade, cadência de auditoria) کی توثیق۔ | Conselho de Governança | docs میں publicar کریں، captura de histórico de revisão کریں۔ |
| ganchos de governança شامل کریں تاکہ Declarações do Parlamento کو aprovar, renovar اور revogar کر سکے۔ | Conselho de Governança / Equipe de Contrato Inteligente | Eventos Norito + ingestão de manifesto |
| cronograma de penalidades (redução de taxas, redução de títulos) نافذ کریں جو violações de SLA telemetradas سے منسلک ہو۔ | Conselho de Governança / Tesouraria | Saídas de liquidação `DealEngine` کے ساتھ alinhar کریں۔ |
| processo de disputa e matriz de escalação کی دستاویز بنائیں۔ | Documentos / Governança | runbook de disputa + ajudantes CLI لنک کریں۔ |

### 4. Medição e distribuição de taxas

| Tarefa | Proprietário(s) | Notas |
|------|----------|-------|
| Ingestão de medição Torii کو `CapacityTelemetryV1` قبول کرنے کے لیے بڑھائیں۔ | Equipe Torii | GiB-hora, sucesso de PoR, tempo de atividade validado کریں۔ |
| Pipeline de medição `sorafs_node` کو utilização por pedido + estatísticas de SLA | Equipe de armazenamento | ordens de replicação e identificadores de chunker کے ساتھ alinhar کریں۔ |
| Pipeline de liquidação: telemetria + dados de replicação کو pagamentos denominados em XOR میں تبدیل کریں, resumos prontos para governança بنائیں، اور estado do razão ریکارڈ کریں۔ | Equipe de Tesouraria/Armazenagem | Exportações Deal Engine / Tesouro میں fio کریں۔ |
| medição de integridade (backlog de ingestão, telemetria obsoleta) e exportação de painéis/alertas | Observabilidade | SF-6/SF-7 میں ریفرنس کیے گئے Pacote Grafana کو estender کریں۔ |- Torii اب `/v2/sorafs/capacity/telemetry` اور `/v2/sorafs/capacity/state` (JSON + Norito) expor کرتا ہے تاکہ operadores instantâneos de telemetria de época جمع کر سکیں اور auditoria de inspetores یا embalagem de evidências کے لیے razão canônica
- Integração `PinProviderRegistry` یقینی بناتی ہے کہ ordens de replicação اسی endpoint کے ذریعے قابلِ رسائی ہوں؛ Auxiliares CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) اب hashing determinístico اور resolução de alias کے ساتھ execuções de automação سے telemetria validar/publicar کرتے ہیں۔
- Medição de instantâneos `CapacityTelemetrySnapshot` entradas پیدا کرتے ہیں جو `metering` instantâneo سے fixado ہوتے ہیں, اور Prometheus exporta `docs/source/grafana_sorafs_metering.json` کے placa Grafana pronta para importar کو feed کرتے ہیں تاکہ equipes de faturamento acumulação de GiB-hora, taxas nano-SORA projetadas اور conformidade com SLA کو tempo real میں مانیٹر کر سکیں۔【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- جب metering smoothing enabled ہو تو snapshot میں `smoothed_gib_hours` اور `smoothed_por_success_bps` شامل ہوتے ہیں تاکہ operadores Valores de tendência EMA کو contadores brutos کے مقابلے میں دیکھ سکیں جنہیں pagamentos de governança کے لیے استعمال کرتی ہے۔【crates/sorafs_node/src/metering.rs:401】

### 5. Tratamento de disputas e revogações

| Tarefa | Proprietário(s) | Notas |
|------|----------|-------|
| Carga útil `CapacityDisputeV1` (reclamante, evidência, provedor de destino) کی تعریف۔ | Conselho de Governança | Esquema Norito + validador۔ |
| disputas فائل کرنے اور جواب دینے کے لیے suporte CLI (anexos de evidências کے ساتھ)۔ | GT Ferramentaria | pacote de evidências کی hashing determinístico یقینی بنائیں۔ |
| Violações repetidas de SLA کے لیے verificações automatizadas شامل کریں (escalonamento automático para disputa)۔ | Observabilidade | limites de alerta e ganchos de governança۔ |
| manual de revogação (período de carência, dados fixados کی evacuação) دستاویزی بنائیں۔ | Documentos / Equipe de armazenamento | documento de política e runbook do operador. |

## Requisitos de teste e CI

- Existem validadores de esquema para testes de unidade (`sorafs_manifest`).
- testes de integração جو simular کریں: declaração → ordem de replicação → medição → pagamento۔
- Fluxo de trabalho de CI e declarações de capacidade de amostra/regeneração de telemetria کرے اور sincronização de assinaturas رکھے (`ci/check_sorafs_fixtures.sh` کو estender کریں)۔
- API de registro para testes de carga (10 mil provedores, 100 mil pedidos simulados) ۔

## Telemetria e painéis

- Painéis do painel:
  - provedor کے حساب سے declarou بمقابلہ capacidade utilizada۔
  - backlog de pedidos de replicação e atraso médio de atribuição۔
  - Conformidade com SLA (% de tempo de atividade, taxa de sucesso de PoR)۔
  - فی acumulação de taxas de época e penalidades۔
- Alertas:
  - provedor کم از کم capacidade comprometida سے نیچے۔
  - pedido de replicação SLA سے زیادہ preso۔
  - medição de falhas no pipeline۔

## Entregáveis ​​de documentação- declaração de capacidade کرنے، compromissos renovados کرنے، اور utilização مانیٹر کرنے کے لیے guia do operador۔
- declarações aprovam کرنے، ordens جاری کرنے، tratamento de disputas کرنے کے لیے guia de governança۔
- endpoints de capacidade e formato de pedido de replicação کے لیے referência de API۔
- desenvolvedores کے لیے FAQ do mercado۔

## Lista de verificação de preparação para GA

روڈ میپ آئٹم **SF-2c** contabilidade, tratamento de disputas e integração نیچے دیے گئے artefatos استعمال کریں تاکہ implementação de critérios de aceitação کے ساتھ sincronização رہے۔

### Contabilidade noturna e reconciliação XOR
- snapshot do estado de capacidade e exportação do razão XOR ایک ہی janela کے لیے نکالیں, پھر چلائیں:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  یہ assentamentos perdidos/pagos em excesso do ajudante یا penalidades پر saída diferente de zero کرتا ہے اور Prometheus resumo do arquivo de texto emitir کرتا ہے۔
- Alerta `SoraFSCapacityReconciliationMismatch` (`dashboards/alerts/sorafs_capacity_rules.yml` میں)
  métricas de reconciliação کے lacunas رپورٹ کرنے پر fogo ہوتا ہے؛ painéis `dashboards/grafana/sorafs_capacity_penalties.json` میں ہیں۔
- Resumo JSON e hashes کو `docs/examples/sorafs_capacity_marketplace_validation/` کے تحت arquivo کریں
  pacotes de governança

### Disputa e redução de evidências
- disputas `sorafs_manifest_stub capacity dispute` کے ذریعے فائل کریں (testes:
  `cargo test -p sorafs_car --test capacity_cli`) Cargas úteis canônicas رہیں۔
- `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` e suítes de penalidade
  (`record_capacity_telemetry_penalises_persistent_under_delivery`) چلائیں تاکہ disputas اور corta repetição determinística ثابت ہوں۔
- captura de evidências e escalonamento کے لیے `docs/source/sorafs/dispute_revocation_runbook.md` فالو کریں؛ aprovações de greve e relatório de validação میں واپس لنک کریں۔

### Testes de fumaça de integração e saída do provedor
- artefatos de declaração/telemetria کو `sorafs_manifest_stub capacity ...` سے regenerar کریں اور submissão سے پہلے repetição de testes CLI کریں (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`)۔
- Torii (`/v2/sorafs/capacity/declare`) کے ذریعے enviar کریں پھر `/v2/sorafs/capacity/state` اور Grafana captura de captura de tela کریں۔ `docs/source/sorafs/capacity_onboarding_runbook.md` fluxo de saída de fluxo de saída
- artefatos assinados e saídas de reconciliação کو `docs/examples/sorafs_capacity_marketplace_validation/` arquivo میں کریں۔

## Dependências e sequenciamento

1. SF-2b (política de admissão) مکمل کریں - fornecedores avaliados pelo mercado پر انحصار کرتا ہے۔
2. Integração Torii com esquema + camada de registro (یہ ڈاک) مکمل کریں۔
3. pagamentos permitem کرنے سے پہلے pipeline de medição مکمل کریں۔
4. آخری قدم: preparação میں verificação de dados de medição ہونے کے بعد distribuição de taxas controladas pela governança permitir کریں۔

progresso کو roteiro میں اس دستاویز کے حوالے کے ساتھ ٹریک کریں۔ جب ہر بڑا سیکشن (esquema, plano de controle, integração, medição, tratamento de disputas) recurso completo ہو جائے تو roteiro اپڈیٹ کریں۔