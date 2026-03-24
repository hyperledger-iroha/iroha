---
lang: pt
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T15:38:30.662574+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Runbook de liquidação de recompra

Este guia documenta o fluxo determinístico para acordos de recompra e recompra reversa em Iroha.
Abrange a orquestração CLI, os auxiliares do SDK e os botões de governança esperados para que os operadores possam
iniciar, criar margens e desfazer contratos sem gravar cargas úteis Norito brutas. Para governança
listas de verificação, captura de evidências e procedimentos de fraude/reversão, consulte
[`repo_ops.md`](./repo_ops.md), que satisfaz o item F1 do roteiro.

## Comandos CLI

O comando `iroha app repo` agrupa auxiliares específicos do repositório:

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator i105... \
  --counterparty i105... \
  --custodian i105... \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1000 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1050 \
  --rate-bps 250 \
  --maturity-timestamp-ms 1704000000000 \
  --haircut-bps 1500 \
  --margin-frequency-secs 86400

# Generate the unwind leg
iroha --config client.toml --output \
  repo unwind \
  --agreement-id daily_repo \
  --initiator i105... \
  --counterparty i105... \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1005 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1055 \
  --settlement-timestamp-ms 1704086400000

# Inspect the next margin checkpoint for an active agreement
iroha --config client.toml repo margin --agreement-id daily_repo

# Trigger a margin call when cadence elapses
iroha --config client.toml repo margin-call --agreement-id daily_repo
```

* `repo initiate` e `repo unwind` respeitam `--input/--output` então o `InstructionBox` gerado
  cargas úteis podem ser canalizadas para outros fluxos CLI ou enviadas imediatamente.
* Passe `--custodian <account>` para encaminhar garantias para um custodiante tripartido. Quando omitido, o
  a contraparte recebe o penhor diretamente (repo bilateral).
* `repo margin` consulta o razão via `FindRepoAgreements` e informa a próxima margem esperada
  carimbo de data/hora (em milissegundos) juntamente com se um retorno de chamada de margem está vencido no momento.
* `repo margin-call` anexa uma instrução `RepoMarginCallIsi`, registrando o ponto de verificação de margem e
  emitindo eventos para todos os participantes. As chamadas são rejeitadas se a cadência não tiver decorrido ou se o
  a instrução é enviada por um não participante.

## Ajudantes do Python SDK

```python
from iroha_python import (
    create_torii_client,
    RepoAgreementRecord,
    RepoCashLeg,
    RepoCollateralLeg,
    RepoGovernance,
    TransactionConfig,
    TransactionDraft,
)

client = create_torii_client("client.toml")

cash = RepoCashLeg(asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1", quantity="1000")
collateral = RepoCollateralLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="1050",
    metadata={"isin": "ABC123"},
)
governance = RepoGovernance(haircut_bps=1500, margin_frequency_secs=86_400)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="i105..."))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="i105...",
    counterparty="i105...",
    cash_leg=cash,
    collateral_leg=collateral,
    rate_bps=250,
    maturity_timestamp_ms=1_704_000_000_000,
    governance=governance,
)
# ... additional instructions ...
envelope = draft.sign_with_keypair(my_keypair)
client.submit_transaction_envelope(envelope)

# Margin schedule
agreements = client.list_repo_agreements()
record = RepoAgreementRecord.from_payload(agreements[0])
next_margin = record.next_margin_check_after(at_timestamp_ms=now_ms)
```

* Ambos os auxiliares normalizam quantidades numéricas e campos de metadados antes de invocar as ligações PyO3.
* `RepoAgreementRecord` espelha o cálculo do cronograma de tempo de execução para que a automação off-ledger possa
  determine quando os retornos de chamada vencem sem recalcular a cadência manualmente.

## Assentamentos DvP / PvP

O comando `iroha app settlement` prepara instruções de entrega versus pagamento e pagamento versus pagamento:

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --delivery-quantity 10 \
  --delivery-from i105... \
  --delivery-to i105... \
  --delivery-instrument-id US0378331005 \
  --payment-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --payment-quantity 1000 \
  --payment-from i105... \
  --payment-to i105... \
  --order payment-then-delivery \
  --atomicity all-or-nothing \
  --iso-reference-crosswalk /opt/iso/isin_crosswalk.json \
  --iso-xml-out trade_dvp.xml

# Cross-currency swap (payment-versus-payment)
iroha --config client.toml --output \
  settlement pvp \
  --settlement-id trade_pvp \
  --primary-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --primary-quantity 500 \
  --primary-from i105... \
  --primary-to i105... \
  --counter-asset 5tPkFK6s2zUcd1qUHyTmY7fDVa2n \
  --counter-quantity 460 \
  --counter-from i105... \
  --counter-to i105... \
  --iso-xml-out trade_pvp.xml
```

* As quantidades de perna aceitam valores inteiros ou decimais e são validadas em relação à precisão do ativo.
* `--atomicity` aceita `all-or-nothing`, `commit-first-leg` ou `commit-second-leg`. Use estes modos
  com `--order` para expressar qual perna permanece confirmada se o processamento subsequente falhar (`commit-first-leg`
  mantém a primeira etapa aplicada; `commit-second-leg` mantém o segundo).
* As invocações CLI emitem metadados de instruções vazios hoje; use os ajudantes Python quando no nível de liquidação
  os metadados precisam ser anexados.
* Consulte [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) para obter o mapeamento de campo ISO 20022 que
  apoia estas instruções (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`).
* Passe `--iso-xml-out <path>` para que a CLI emita uma visualização XML canônica junto com Norito
  instrução; o arquivo segue o mapeamento acima (`sese.023` para DvP, `sese.025` para PvP`). Emparelhe o
  sinalizador com `--iso-reference-crosswalk <path>` para que a CLI verifique `--delivery-instrument-id` em relação ao
  mesmo instantâneo que Torii usa durante a admissão em tempo de execução.

Os auxiliares Python espelham a superfície CLI:

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="i105..."))
delivery = SettlementLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="10",
    from_account="i105...",
    to_account="i105...",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    quantity="1000",
    from_account="i105...",
    to_account="i105...",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        quantity="500",
        from_account="i105...",
        to_account="i105...",
    ),
    SettlementLeg(
        asset_definition_id="5tPkFK6s2zUcd1qUHyTmY7fDVa2n",
        quantity="460",
        from_account="i105...",
        to_account="i105...",
    ),
)
```

## Determinismo e expectativas de governança

As instruções do repositório dependem exclusivamente dos tipos numéricos codificados em Norito e do arquivo compartilhado
Lógica `RepoGovernance::with_defaults`. Tenha em mente os seguintes invariantes:* As quantidades são serializadas com valores determinísticos `NumericSpec`: uso de cash-legs
  `fractional(2)` (duas casas decimais), pernas colaterais usam `integer()`. Não envie
  valores com maior precisão – os guardas de tempo de execução os rejeitarão e os pares divergirão.
* Os repositórios tripartidos mantêm o ID da conta de custódia em `RepoAgreement`. Eventos de ciclo de vida e margem
  emitir uma carga útil `RepoAccountRole::Custodian` para que os custodiantes possam assinar e reconciliar o inventário.
* Os cortes de cabelo são fixados em 10.000 bps (100%) e as frequências de margem são segundos inteiros. Fornecer
  parâmetros de governança nessas unidades canônicas para permanecerem alinhados com as expectativas de tempo de execução.
* Os carimbos de data e hora são sempre milissegundos unix. Todos os ajudantes os encaminham inalterados para o Norito
  carga útil para que os pares obtenham programações idênticas.
* As instruções de inicialização e desenrolamento reutilizam o mesmo identificador de contrato. O tempo de execução rejeita
  IDs duplicados e desenrolamentos para contratos desconhecidos; Os auxiliares CLI/SDK revelam esses erros antecipadamente.
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` retornam a cadência canônica. Sempre
  consulte este instantâneo antes de acionar retornos de chamada para evitar a repetição de programações obsoletas.