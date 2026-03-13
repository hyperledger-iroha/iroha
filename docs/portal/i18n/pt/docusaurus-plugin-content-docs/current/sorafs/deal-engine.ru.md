---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mecanismo de negociação
título: Движок сделок SoraFS
sidebar_label: Link da barra lateral
descrição: Обзор движка сделок SF-8, интеграции Torii e телеметрических поверхностей.
---

:::nota História Canônica
:::

# Движок сделок SoraFS

A estrada SF-8 é um conjunto de peças SoraFS, compatível
determinar a configuração da configuração e do método de instalação
clientes e provadores. Cargas úteis Norito,
instalado em `crates/sorafs_manifest/src/deal.rs`, usando a chave de fenda,
блокировку títulos, вероятностные микроплатежи e записи расчетов.

Trabalhador SoraFS (`sorafs_node::NodeHandle`)
экземпляр `DealEngine` para o processo de fabricação. Diabo:

- validar e registrar o arquivo `DealTermsV1`;
- начисляет encargos no XOR при отчетах об использовании репликации;
- оценивает окна вероятностных микроплатежей с помощью детерминированного
  amostragem por BLAKE3; e
- формирует snapshots razão e payloads расчетов, пригодные для публикации в governança.

Юнит-тесты покрывают валидацию, você микроплатежей e расчетные потоки, чтобы
операторы могли уверенно проверять API. Расчеты теперь выпускают cargas úteis de governança
`DealSettlementV1`, instalado no pipeline público SF-12, e instalado em série
OpenTelemetria `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) para дашбордов Torii e
Aplicação de SLO. Você pode se concentrar em cortar automaticamente, iniciar
аудиторами, и согласовании семантики отмены с политикой governança.

A telemetria usada também é a métrica `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, e também uma cópia de um bilhete
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Эти totais раскрывают вероятностный
лотерейный поток, чтобы операторы могли коррелировать выигрыши микроплатежей и
перенос кредитов с результатами расчетов.

## Integração Torii

Torii fornece endpoints valiosos, isso prova o uso e o uso
жизненный цикл сделок без специального fiação:- `POST /v2/sorafs/deal/usage` fornece o telefone `DealUsageReport` e retorna
  детерминированные результаты учета (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` janela de proteção de segurança, janela
  O `DealSettlementRecord` é baseado em base64-кодированным `DealSettlementV1`,
  готовым к публикации в governança DAG.
-Lente Torii `/v2/events/sse` теперь транслирует записи `SorafsGatewayEvent::DealUsage`,
  суммирующие каждую отправку uso (época, измеренные GiB-horas, счетчики билетов,
  taxas de determinação), nome `SorafsGatewayEvent::DealSettlement`,
  включающие канонический snapshot ledger расчетов плюс BLAKE3 digest/size/base64
  artefato de governança no disco e alertas `SorafsGatewayEvent::ProofHealth` sobre previsões
  порогов PDP/PoTR (провайдер, окно, состояние strike/cooldown, сумма штрафа). Pode ser melhorado
  фильтровать по провайдеру, чтобы реагировать на новую телеметрию, расчеты или алерты prova-saúde алерты без polling.

Os endpoints disponíveis estão na estrutura de cotas SoraFS em um novo momento
`torii.sorafs.quota.deal_telemetry`, operação de operação de transferência
частоту отправки для каждого деплоя.