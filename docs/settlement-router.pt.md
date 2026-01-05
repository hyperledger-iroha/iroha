---
lang: pt
direction: ltr
source: docs/settlement-router.md
status: complete
translator: manual
source_hash: a00e929ef6088863db93ea122b7314ee921b9b37bea2c6e2c536082c29038f97
source_last_modified: "2025-11-12T08:35:29.309807+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução para português de docs/settlement-router.md (Deterministic Settlement Router) -->

# Router de Liquidação Determinística (NX-3)

**Status:** Planejado (NX-3)  
**Responsáveis:** Economics WG / Core Ledger WG / Treasury / SRE  
**Escopo:** Implementa a política unificada de conversão para XOR e de buffer descrita na
entrada do roadmap “Unified lane settlement & XOR conversion (NX-3)”.

Este documento formaliza o algoritmo, a telemetria e os contratos operacionais do router de
liquidação unificado. Ele complementa o resumo de taxas em
`docs/source/nexus_fee_model.md`, explicando como o router deriva débitos em XOR,
interage com o DEX AMM/CLMM canônico (ou RFQ governado de fallback) e expõe recibos
determinísticos para cada lane.

## Objetivos

   fluxos AMX cross‑DS—usam o mesmo pipeline de liquidação, de forma que as reconciliações
   noturnas dependem de um único modelo.
2. **Conversão determinística:** As dívidas em XOR são derivadas de parâmetros on‑chain
   (TWAP, épsilon de volatilidade, tier de liquidez) e arredondadas para cima em
   micro‑XOR, garantindo que todo nó produza o mesmo recibo.
3. **Buffers de segurança:** Cada dataspace/lane mantém um buffer XOR P95 de 72 h. O router
   debita primeiro os buffers, aplica gates de “throttle/XOR‑only” à medida que os níveis
   caem e exporta métricas para que operadores possam reabastecer de forma proativa.
4. **Liquidez governada:** As conversões preferem o DEX AMM/CLMM de lane público
   canônico; RFQs/swap‑lines governados entram em ação quando a profundidade cai ou
   circuit breakers são disparados.

## Arquitetura

| Componente | Local | Responsabilidade |
|-----------|-------|------------------|
| `crates/settlement_router/` | Crate do router (novo) | Cálculo de shadow price, haircuts, contabilidade de buffer, geração de cronograma, rastreamento de exposição P&L. |
| `crates/oracle_adapter/` | Middleware de oráculo | Mantém a janela TWAP (60 s padrão) + volatilidade móvel, emite preço de circuito se os feeds ficarem obsoletos. |
| `crates/iroha_core/src/fees` e `LaneBlockCommitment` | Core ledger | Persiste recibos de lane, variância em XOR e metadados de swap em provas de bloco para auditoria. |
| DEX AMM/CLMM canônico e RFQ governado | Time DEX/AMM | Executa swaps determinísticos; fornece profundidade + tiers de haircut (tier1=profundo → 0 bp, tier2=médio → 25 bp, tier3=rasa → 75 bp). |
| Linhas de swap do Tesouro / APIs de sponsor | Financial Services WG | Permitem que sponsors ou linhas de crédito de MM autorizadas (≤25 % de Bmin) recarreguem buffers sem depender de swaps de mercado. |
| Telemetria + dashboards | `iroha_telemetry`, `dashboards/…` | Emite `iroha_settlement_buffer_xor`, `iroha_settlement_pnl_xor`, `iroha_settlement_haircut_bp`, etc.; dashboards alertam em thresholds de buffer. |

## Pipeline de conversão

1. **Agregação de recibos**
   - Cada transação/perna AMX emite um `LaneSettlementReceipt` com valor local em micro,
     XOR devido, tier de haircut, variância realizada (`xor_variance_micro`) e metadados
     do chamador (já definido em `docs/source/nexus_fee_model.md`).
   - O router agrupa recibos por `(lane, dataspace)` durante a execução do bloco.

2. **Shadow price (`shadow_price.rs`)**
   - Busca o TWAP de 60 s (`twap_local_per_xor`) e uma amostra de volatilidade de 30 s.
   - Calcula épsilon: 25 bp base + (bucket de volatilidade × 25 bp, limitado a 75 bp).
   - Persiste o `volatility_class` resolvido (`stable`, `elevated`, `dislocated`) em
     `LaneSwapMetadata` para que auditores e dashboards consigam associar a margem extra ao
     regime de oráculo capturado em `pipeline.gas.units_per_gas`.
   - Calcula `xor_due = ceil(local_micro / twap × (1 + epsilon))`.
   - Armazena a tupla `(twap, epsilon, tier)` nos metadados de swap para auditoria.

3. **Haircut + variância (`haircut.rs`)**
   - Aplica haircuts conforme o perfil de liquidez (tier1=0 bp, tier2=25 bp, tier3=75 bp).
   - Registra `total_xor_after_haircut` e `total_xor_variance` (diferença entre o devido e
     o valor pós‑haircut) para que o Tesouro veja quanto de margem de segurança cada tier
     consome.

4. **Débito de buffer (`buffer.rs`)**
   - Cada buffer de lane/dataspace define Bmin = gasto XOR P95 em 72 h.
   - Estados: **Normal** (≥75 %), **Alert** (<75 %), **Throttle** (<25 %), **XOR‑only**
     (<10 %), **Halt** (<2 % ou oráculo obsoleto).
   - O router debita buffers por recibo; quando os thresholds são violados, emite
     telemetria determinística e aplica throttles (por exemplo, reduzindo pela metade a
     taxa de inclusão abaixo de 25 %).
   - Os metadados de lane devem declarar a reserva do Tesouro via:
     - `metadata.settlement.buffer_account` – ID da conta que mantém a reserva
       (por exemplo, `buffer::cbdc_treasury`).
     - `metadata.settlement.buffer_asset` – Definição de asset debitado para headroom
       (geralmente `xor#sora`).
     - `metadata.settlement.buffer_capacity_micro` – Capacidade em micro‑XOR (string
       decimal).
   - A telemetria expõe `iroha_settlement_buffer_xor` (headroom restante),
     `iroha_settlement_buffer_capacity_xor` e `iroha_settlement_buffer_state` (estado
     discreto).

5. **Agendamento de swaps (`schedule.rs`)**
   - Agrupa recibos por asset local e tier de liquidez para minimizar o número de swaps.
   - Respeita limites de tamanho/quantidade para cada ordem AMM/CLMM.
   - Produz um `LaneSwapSchedule` determinístico que descreve:
     - o asset local agregado;
     - o valor alvo em XOR;
     - o DEX/tier selecionado;
     - o haircut aplicado;
     - o lane/dataspace afetados.

6. **Liquidação e provas de bloco**
   - O scheduler envia ordens para a camada DEX. O resultado de cada swap é codificado em
     `LaneSwapMetadata` e anexado a `LaneBlockCommitment`.
   - Auditores conseguem reconstruir a conversão XOR‑local a partir de:
     - TWAP e volatilidade;
     - tier de liquidez;
     - haircut aplicado;
     - variância total em XOR.

## Telemetria e alertas

Métricas mínimas a serem expostas:

- `iroha_settlement_buffer_xor{dataspace,lane}` – headroom restante em XOR.
- `iroha_settlement_buffer_capacity_xor{dataspace,lane}` – capacidade configurada Bmin.
- `iroha_settlement_buffer_state{dataspace,lane}` – estado discreto (`normal`, `alert`,
  `throttle`, `xor_only`, `halt`).
- `iroha_settlement_pnl_xor{dataspace,lane}` – P&L acumulado em XOR desde o início.
- `iroha_settlement_haircut_bp{dataspace,lane,tier}` – haircut efetivo aplicado.

Dashboards e alertas:

- Disparar alertas quando o estado entrar em `alert` (<75 %) ou pior.
- Construir painéis que mostrem headroom por lane/dataspace e consumo diário.
- Destacar lanes cuja variância acumulada (`total_xor_variance`) saia da faixa esperada.

## Estratégia de rollout

Recomenda‑se habilitar o router em fases:

1. **Shadow mode:** ativar os cálculos do router + telemetria, mas manter
2. **Ativação de débitos:** ligar `settlement_debit=true` para lanes de baixo risco
   (perfil C) e verificar as reconciliações.
3. **Conversões ativas:** habilitar TWAMM slices + swaps AMM quando a telemetria indicar
   buffers estáveis; manter o RFQ de fallback pronto.
4. **Integração com AMX:** garantir que lanes AMX propaguem recibos e exponham
   `SETTLEMENT_ROUTER_UNAVAILABLE` para os SDKs, permitindo retries.
   que merges futuros respeitem thresholds da telemetria de liquidação.

Seguir este documento deve garantir que a tarefa NX‑3 do roadmap entregue comportamento
determinístico, artefatos auditáveis e orientações claras para desenvolvedores e
operadores.
