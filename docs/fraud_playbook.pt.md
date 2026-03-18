---
lang: pt
direction: ltr
source: docs/fraud_playbook.md
status: complete
translator: manual
source_hash: b3253ff47a513529c1dba6ef44faf38087ee0e5f5520f8c3fd770ab8d36c7786
source_last_modified: "2025-11-02T04:40:28.812006+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução em português de docs/fraud_playbook.md (Fraud Governance Playbook) -->

# Playbook de Governança de Fraude

Este documento resume a infraestrutura necessária para a stack de fraude PSP
enquanto os microserviços completos e os SDKs ainda estão em desenvolvimento
ativo. Ele captura expectativas para analítica, fluxos de trabalho de
auditoria e procedimentos de fallback, de forma que as próximas implementações
possam se acoplar ao ledger com segurança.

## Visão geral dos serviços

1. **API Gateway** – recebe payloads síncronos `RiskQuery`, encaminha‑os para a
   agregação de features e devolve respostas `FraudAssessment` aos fluxos do
   ledger. Exige alta disponibilidade (ativo‑ativo); use pares regionais com
   hashing determinista para evitar viés na distribuição de requisições.
2. **Agregação de Features** – compõe vetores de features para scoring. Emite
   apenas hashes de `FeatureInput`; payloads sensíveis permanecem off‑chain. A
   observabilidade deve publicar histogramas de latência, gauges de profundidade
   de fila e contadores de replay por tenant.
3. **Motor de Risco** – avalia regras/modelos e produz saídas determinísticas
   `FraudAssessment`. Garanta que a ordem de execução de regras seja estável e
   capture logs de auditoria por ID de avaliação.

## Analítica e promoção de modelos

- **Detecção de anomalias**: manter um job de streaming que sinalize
  desvios nas taxas de decisão por tenant. Encaminhe alertas para o dashboard
  de governança e armazene resumos para revisões trimestrais.
- **Análise de grafos**: executar diariamente travessias de grafo sobre exports
  relacionais para identificar clusters de conluio. Exportar achados para o
  portal de governança via `GovernanceExport`, com referências às evidências de
  suporte.
- **Ingestão de feedback**: consolidar resultados de revisão manual e relatórios
  de chargeback. Convertê‑los em deltas de features e incorporá‑los em
  datasets de treinamento. Publicar métricas de status de ingestão para que o
  time de risco consiga identificar feeds parados.
- **Pipeline de promoção de modelos**: automatizar a avaliação de candidatos
  (métricas offline, scoring canário, prontidão para rollback). As promoções
  devem emitir um conjunto de amostras `FraudAssessment` assinado e atualizar o
  campo `model_version` em `GovernanceExport`.

## Fluxo de trabalho do auditor

1. Tirar um snapshot do `GovernanceExport` mais recente e verificar se o
   `policy_digest` corresponde ao manifest fornecido pelo time de risco.
2. Validar se os agregados de regras reconciliam com os totais de decisões do
   lado do ledger na janela amostrada.
3. Revisar os relatórios de detecção de anomalias e análise de grafos em busca
   de pendências. Documentar escalonamentos e responsáveis esperados pela
   remediação.
4. Assinar e arquivar a checklist de revisão. Armazenar os artefatos
   codificados em Norito no portal de governança para garantir
   reprodutibilidade.

## Playbooks de fallback

- **Indisponibilidade do motor**: se o motor de risco ficar indisponível por
  mais de 60 segundos, o gateway deve alternar para modo somente revisão,
  emitindo `AssessmentDecision::Review` para todas as requisições e alertando
  os operadores.
- **Falha de telemetria**: quando métricas ou traces ficarem atrasados (sem
  dados por 5 minutos), interromper promoções automáticas de modelo e notificar
  o engenheiro de plantão.
- **Regressão de modelo**: se o feedback pós‑deploy indicar aumento de perdas
  por fraude, fazer rollback para o bundle de modelo assinado anterior e
  atualizar o roadmap com ações corretivas.

## Acordos de compartilhamento de dados

- Manter anexos específicos por jurisdição cobrindo retenção, criptografia e
  SLAs de notificação de incidentes. Parceiros devem assinar o anexo antes de
  receber exports de `FraudAssessment`.
- Documentar práticas de minimização de dados para cada integração (por
  exemplo, hashing de identificadores de conta, truncamento de números de
  cartão).
- Renovar os acordos anualmente ou sempre que requisitos regulatórios mudarem.

## Exercícios de Red Team

- Os exercícios são realizados trimestralmente. A próxima sessão está
  agendada para **2026‑01‑15**, com cenários que cobrem envenenamento de
  features, amplificação de replays e tentativas de falsificação de assinaturas.
- Registrar os achados no modelo de ameaças de fraude e adicionar as tarefas
  resultantes a `roadmap.md` no workstream
  “Fraud & Telemetry Governance Loop”.

## Esquemas de API

O gateway agora expõe envelopes JSON concretos que mapeiam um‑para‑um com os
tipos Norito implementados em `crates/iroha_data_model::fraud`:

- **Entrada de risco** – `POST /v1/fraud/query` aceita o schema `RiskQuery`:
  - `query_id` (`[u8; 32]`, codificado em hex)
  - `subject` (`AccountId`, `domainless encoded literal; canonical I105 only (i105-default `sora...` rejected)`)
  - `operation` (enum etiquetado que corresponde a `RiskOperation`; o campo
    `type` no JSON reflete a variante do enum)
  - `related_asset` (`AssetId`, opcional)
  - `features` (array de `{ key: String, value_hash: hex32 }` mapeado a partir
    de `FeatureInput`)
  - `issued_at_ms` (`u64`)
  - `context` (`RiskContext`; carrega `tenant_id`, `session_id` opcional e
    `reason` opcional)
- **Decisão de risco** – `POST /v1/fraud/assessment` consome o payload
  `FraudAssessment` (também refletido nos exports de governança):
  - `query_id`, `engine_id`, `risk_score_bps`, `confidence_bps`,
    `decision` (enum `AssessmentDecision`), `rule_outcomes`
    (array de `{ rule_id, score_delta_bps, rationale? }`)
  - `generated_at_ms`
  - `signature` (base64 opcional envolvendo o `FraudAssessment` codificado em
    Norito)
- **Export de governança** – `GET /v1/fraud/governance/export` retorna a
  estrutura `GovernanceExport` quando a feature `governance` está habilitada,
  agregando parâmetros ativos, o último enactment, versão do modelo, digest de
  política e o histograma `DecisionAggregate`.

Testes de round‑trip em `crates/iroha_data_model/src/fraud/types.rs` garantem
que esses schemas permaneçam binariamente compatíveis com o codec Norito, e
`integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs`
exercita o pipeline de entrada/decisão de ponta a ponta.

## Referências de SDK PSP

Os seguintes stubs de linguagem acompanham os exemplos de integração voltados a
PSPs:

- **Rust** – `integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs`
  usa o cliente `iroha` do workspace para construir metadata `RiskQuery` e
  validar sucessos/falhas de admissão.
- **TypeScript** – `docs/source/governance_api.md` documenta a superfície REST
  consumida pelo gateway Torii leve usado no dashboard de demo PSP; o cliente
  scriptado vive em `scripts/ci/schedule_fraud_scoring.sh` para drills de
  smoke.
- **Swift e Kotlin** – os SDKs existentes (`IrohaSwift` e as referências em
  `crates/iroha_cli/docs/multisig.md`) expõem os hooks de metadata Torii
  necessários para anexar campos `fraud_assessment_*`. Helpers específicos de
  PSP são rastreados no milestone “Fraud & Telemetry Governance Loop” em
  `status.md` e reutilizam os builders de transações desses SDKs.

Essas referências serão mantidas em sincronia com o gateway de microserviços
para que implementadores PSP tenham sempre um schema e um caminho de exemplo
atualizados para cada linguagem suportada.

