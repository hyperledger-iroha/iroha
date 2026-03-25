---
lang: pt
direction: ltr
source: docs/source/da/rent_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7cdc46bcd87af7924817a94900c8fad2c23570607f4065f19d8a42d259fe83f
source_last_modified: "2026-01-22T15:38:30.661606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Política de aluguel e incentivos por disponibilidade de dados (DA-7)

_Status: Elaboração — Proprietários: GT de Economia / Tesouraria / Equipe de Armazenamento_

O item do roteiro **DA-7** introduz um aluguel explícito denominado XOR para cada blob
enviado para `/v1/da/ingest`, além de bônus que recompensam a execução de PDP/PoTR e
saída serviu para buscar clientes. Este documento define os parâmetros iniciais,
sua representação do modelo de dados e o fluxo de trabalho de cálculo usado por Torii,
SDKs e painéis de tesouraria.

## Estrutura política

A política é codificada como [`DaRentPolicyV1`](/crates/iroha_data_model/src/da/types.rs)
dentro do modelo de dados. Torii e ferramentas de governança persistem na política em
Cargas úteis Norito para que cotações de aluguel e livros de incentivo possam ser recalculados
determinísticamente. O esquema expõe cinco botões:

| Campo | Descrição | Padrão |
|-------|-------------|---------|
| `base_rate_per_gib_month` | XOR cobrado por GiB por mês de retenção. | `250_000` micro-XOR (0,25 XOR) |
| `protocol_reserve_bps` | Parcela do aluguel encaminhada para reserva protocolar (pontos base). | `2_000` (20%) |
| `pdp_bonus_bps` | Porcentagem de bônus por avaliação de PDP bem-sucedida. | `500` (5%) |
| `potr_bonus_bps` | Porcentagem de bônus por avaliação PoTR bem-sucedida. | `250` (2,5%) |
| `egress_credit_per_gib` | Crédito pago quando um provedor fornece 1 GiB de dados DA. | `1_500` micro XOR |

Todos os valores de ponto base são validados em relação a `BASIS_POINTS_PER_UNIT` (10000).
As atualizações de políticas devem passar pela governança, e cada nó Torii expõe o
política ativa por meio da seção de configuração `torii.da_ingest.rent_policy`
(`iroha_config`). Os operadores podem substituir os padrões em `config.toml`:

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

As ferramentas CLI (`iroha app da rent-quote`) aceitam as mesmas entradas de política Norito/JSON
e emite artefatos que espelham o `DaRentPolicyV1` ativo sem atingir
de volta ao estado Torii. Forneça o instantâneo da política usado para uma execução de ingestão para que o
a citação permanece reproduzível.

### Artefatos persistentes de cotação de aluguel

Execute `iroha app da rent-quote --gib <size> --months <months> --quote-out <path>` para
emitir o resumo na tela e um artefato JSON bem impresso. O arquivo
registra `policy_source`, o instantâneo `DaRentPolicyV1` embutido, o cálculo
`DaRentQuote` e um `ledger_projection` derivado (serializado via
[`DaRentLedgerProjection`](/crates/iroha_data_model/src/da/types.rs)) tornando-o adequado para painéis de tesouraria e ISI de razão
oleodutos. Quando `--quote-out` aponta para um diretório aninhado, a CLI cria qualquer
pais desaparecidos, para que as operadoras possam padronizar locais como
`artifacts/da/rent_quotes/<timestamp>.json` junto com outros pacotes de evidências DA.
Anexe o artefato às aprovações de aluguel ou às execuções de reconciliação para que o XOR
discriminação (aluguel base, reserva, bônus PDP/PoTR e créditos de saída) é
reproduzível. Passe `--policy-label "<text>"` para substituir automaticamente
descrição derivada `policy_source` (caminhos de arquivo, padrão incorporado, etc.) com um
tag legível por humanos, como um ticket de governança ou hash de manifesto; os cortes CLI
esse valor e rejeita strings vazias/apenas espaços em branco para que a evidência registrada
permanece auditável.

```json
{
  "policy_source": "policy JSON `configs/da/rent_policy.json`",
  "gib": 10,
  "months": 3,
  "policy": { "...": "DaRentPolicyV1 fields elided" },
  "quote": { "...": "DaRentQuote breakdown" },
  "ledger_projection": {
    "rent_due": { "micro": 7500000 },
    "protocol_reserve_due": { "micro": 1500000 },
    "provider_reward_due": { "micro": 6000000 },
    "pdp_bonus_pool": { "micro": 375000 },
    "potr_bonus_pool": { "micro": 187500 },
    "egress_credit_per_gib": { "micro": 1500 }
  }
}
```A seção de projeção do razão alimenta diretamente os ISIs do razão de aluguel do DA:
define os deltas XOR destinados à reserva de protocolo, pagamentos de provedores e
os pools de bônus por prova sem a necessidade de código de orquestração personalizado.

### Gerando planos de contabilidade de aluguel

Execute `iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc`
para converter uma cotação de aluguel persistente em transferências contábeis executáveis. O comando
analisa o `ledger_projection` incorporado, emite instruções Norito `Transfer`
que arrecadam a renda base para o tesouro, encaminha a reserva/provedor
parcelas e pré-financia os conjuntos de bônus PDP/PoTR diretamente do pagador. O
a saída JSON espelha os metadados da cotação para que as ferramentas de CI e de tesouraria possam raciocinar
sobre o mesmo artefato:

```json
{
  "quote_path": "artifacts/da/rent_quotes/2025-12-07/rent.json",
  "rent_due_micro_xor": 7500000,
  "protocol_reserve_due_micro_xor": 1500000,
  "provider_reward_due_micro_xor": 6000000,
  "pdp_bonus_pool_micro_xor": 375000,
  "potr_bonus_pool_micro_xor": 187500,
  "egress_credit_per_gib_micro_xor": 1500,
  "instructions": [
    { "Transfer": { "...": "payer -> treasury base rent instruction elided" }},
    { "Transfer": { "...": "treasury -> reserve" }},
    { "Transfer": { "...": "treasury -> provider payout" }},
    { "Transfer": { "...": "payer -> PDP bonus escrow" }},
    { "Transfer": { "...": "payer -> PoTR bonus escrow" }}
  ]
}
```

O campo final `egress_credit_per_gib_micro_xor` permite painéis e pagamentos
os programadores alinham os reembolsos de saída com a política de aluguel que produziu o
citar sem recalcular a matemática da política na cola de script.

## Exemplo de citação

```rust
use iroha_data_model::da::types::DaRentPolicyV1;

// 10 GiB retained for 3 months.
let policy = DaRentPolicyV1::default();
let quote = policy.quote(10, 3).expect("policy validated");

assert_eq!(quote.base_rent.as_micro(), 7_500_000);      // 7.5 XOR total rent
assert_eq!(quote.protocol_reserve.as_micro(), 1_500_000); // 20% reserve
assert_eq!(quote.provider_reward.as_micro(), 6_000_000);  // Direct provider payout
assert_eq!(quote.pdp_bonus.as_micro(), 375_000);          // PDP success bonus
assert_eq!(quote.potr_bonus.as_micro(), 187_500);         // PoTR success bonus
assert_eq!(quote.egress_credit_per_gib.as_micro(), 1_500);
```

A cotação pode ser reproduzida em nós Torii, SDKs e relatórios do Tesouro porque
ele usa estruturas Norito determinísticas em vez de matemática ad-hoc. Os operadores podem
anexar o `DaRentPolicyV1` codificado em JSON/CBOR às propostas de governança ou alugar
auditorias para comprovar quais parâmetros estavam em vigor para um determinado blob.

## Bônus e reservas

- **Reserva de protocolo:** `protocol_reserve_bps` financia a reserva XOR que respalda
  replicação de emergência e redução de reembolsos. O Tesouro rastreia esse balde
  separadamente para garantir que os saldos contábeis correspondam à taxa configurada.
- **Bônus PDP/PoTR:** Cada avaliação de prova bem-sucedida recebe um adicional
  pagamento derivado de `base_rent × bonus_bps`. Quando o agendador DA emite prova
  recibos, inclui as etiquetas de ponto base para que os incentivos possam ser reproduzidos.
- **Crédito de saída:** Os provedores registram GiB atendidos por manifesto, multiplicados por
  `egress_credit_per_gib` e envie os recibos via `iroha app da prove-availability`.
  A política de aluguel mantém o valor por GiB sincronizado com a governança.

## Fluxo operacional

1. **Ingestão:** `/v1/da/ingest` carrega o `DaRentPolicyV1` ativo, cita aluguel
   com base no tamanho e na retenção do blob e incorpora a cotação no Norito
   manifestar. O remetente assina uma declaração que faz referência ao hash do aluguel e
   o ID do tíquete de armazenamento.
2. **Contabilidade:** Os scripts de ingestão do Tesouro decodificam o manifesto, chamam
   `DaRentPolicyV1::quote` e preencher os livros de aluguel (aluguel base, reserva,
   bônus e créditos de saída esperados). Qualquer discrepância entre o aluguel registrado
   e cotações recalculadas falham no CI.
3. **Recompensas de prova:** Quando os agendadores de PDP/PoTR marcam um sucesso, eles emitem um recibo
   contendo o resumo do manifesto, o tipo de prova e o bônus XOR derivado de
   a política. A governança pode auditar os pagamentos recalculando a mesma cotação.
4. **Reembolso de saída:** Os orquestradores de busca enviam resumos de saída assinados.
   Torii multiplica a contagem de GiB por `egress_credit_per_gib` e emite o pagamento
   instruções contra o depósito de aluguel.

## TelemetriaOs nós Torii expõem o uso de aluguel por meio das seguintes métricas Prometheus (rótulos:
`cluster`, `storage_class`):

- `torii_da_rent_gib_months_total` — GiB-meses cotados por `/v1/da/ingest`.
- `torii_da_rent_base_micro_total` — aluguel base (micro XOR) acumulado no ingest.
- `torii_da_protocol_reserve_micro_total` — contribuições de reserva de protocolo.
- `torii_da_provider_reward_micro_total` — pagamentos de aluguel do fornecedor.
- `torii_da_pdp_bonus_micro_total` e `torii_da_potr_bonus_micro_total` —
  Conjuntos de bônus PDP/PoTR provenientes da cotação de ingestão.

Os painéis de economia dependem desses contadores para garantir ISIs contábeis, torneiras de reserva,
e as programações de bônus PDP/PoTR correspondem aos parâmetros da política em vigor para cada
cluster e classe de armazenamento. A placa SoraFS Capacidade de Saúde Grafana
(`dashboards/grafana/sorafs_capacity_health.json`) agora renderiza painéis dedicados
para distribuição de aluguel, acúmulo de bônus PDP/PoTR e captura de GiB-mês, permitindo
Tesouraria para filtrar por cluster Torii ou classe de armazenamento ao revisar a ingestão
volume e pagamentos.

##Próximas etapas

- ✅ Os recibos `/v1/da/ingest` agora incorporam `rent_quote` e as superfícies CLI/SDK exibem a cotação
  aluguel base, participação de reserva e bônus PDP/PoTR para que os remetentes possam revisar as obrigações XOR antes
  comprometendo cargas úteis.
- Integrar o livro-razão de aluguel com os próximos feeds de reputação/livro de pedidos do DA
  para provar que os provedores de alta disponibilidade estão recebendo os pagamentos corretos.