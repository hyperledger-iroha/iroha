<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e41e5ed0d7b74fe8ea1ac5bda290088ebb54572db240ae9c2546d719e0c6815f
source_last_modified: "2025-11-19T13:44:41.615366+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: constant-rate-profiles
title: Perfis de taxa constante do SoraNet
sidebar_label: Perfis de taxa constante
description: Catalogo de presets SNNet-17B1 para relays core/home de producao e o perfil null de dogfood SNNet-17A2, com matematica tick->bandwidth, helpers de CLI e guardrails de MTU.
---

:::note Fonte canonica
Esta pagina espelha `docs/source/soranet/constant_rate_profiles.md`. Mantenha ambas as copias sincronizadas.
:::

SNNet-17B introduz lanes de transporte de taxa fixa para que os relays movam trafego em celulas de 1,024 B independentemente do tamanho do payload. Os operadores escolhem entre tres presets:

- **core** - relays de data center ou hospedagem profissional que podem dedicar >=30 Mbps para cobrir o trafego.
- **home** - operadores residenciais ou de uplink baixo que ainda precisam de fetches anonimos para circuitos sensiveis a privacidade.
- **null** - preset dogfood do SNNet-17A2. Mantem os mesmos TLVs/envelope, mas alonga o tick e o ceiling para staging de baixa largura de banda.

## Resumo de presets

| Perfil | Tick (ms) | Celula (B) | Limite de lanes | Piso dummy | Payload por lane (Mb/s) | Payload teto (Mb/s) | % de uplink no teto | Uplink recomendado (Mb/s) | Limite de vizinhos | Gatilho de auto-desativacao (%) |
|---------|-----------|----------|----------|-------------|-------------------------|------------------------|---------------------|----------------------------|--------------|--------------------------|
| core    | 5.0       | 1024     | 12       | 4           | 1.64                    | 19.50                  | 65                  | 30.0                       | 8            | 85                       |
| home    | 10.0      | 1024     | 4        | 2           | 0.82                    | 4.00                   | 40                  | 10.0                       | 2            | 70                       |
| null    | 20.0      | 1024     | 2        | 1           | 0.41                    | 0.75                   | 15                  | 5.0                        | 1            | 55                       |

- **Lane cap** - maximo de vizinhos concorrentes de taxa constante. O relay rejeita circuitos extras quando o limite e atingido e incrementa `soranet_handshake_capacity_reject_total`.
- **Dummy floor** - numero minimo de lanes que permanecem ativas com trafego dummy mesmo quando a demanda real e menor.
- **Ceiling payload** - orcamento de uplink dedicado aos lanes de taxa constante apos aplicar a fracao de ceiling. Os operadores nunca devem exceder esse orcamento mesmo se houver largura de banda extra.
- **Auto-disable trigger** - percentual de saturacao sustentada (media por preset) que faz o runtime cair para o piso dummy. A capacidade e restaurada apos o limiar de recuperacao (75% para `core`, 60% para `home`, 45% para `null`).

**Importante:** o preset `null` e apenas para staging e dogfooding de capacidades; ele nao atende as garantias de privacidade exigidas para circuitos de producao.

## Tabela tick -> bandwidth

Cada celula de payload carrega 1,024 B, portanto a coluna KiB/sec equivale ao numero de celulas emitidas por segundo. Use o helper para estender a tabela com ticks personalizados.

| Tick (ms) | Celulas/seg | Payload KiB/sec | Payload Mb/s |
|-----------|-----------|-----------------|--------------|
| 5.0       | 200.00    | 200.00          | 1.64         |
| 7.5       | 133.33    | 133.33          | 1.09         |
| 10.0      | 100.00    | 100.00          | 0.82         |
| 15.0      | 66.67     | 66.67           | 0.55         |
| 20.0      | 50.00     | 50.00           | 0.41         |

Formula:

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

Helper de CLI:

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

O `--format markdown` emite tabelas estilo GitHub tanto para o resumo de presets quanto para a tabela opcional de ticks, para que voce possa colar a saida deterministica no portal. Combine com `--json-out` para arquivar os dados renderizados como evidencia de governanca.

## Configuracao e overrides

`tools/soranet-relay` expoe os presets tanto em arquivos de configuracao quanto em overrides de runtime:

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

O campo de configuracao aceita `core`, `home` ou `null` (padrao `core`). Overrides via CLI sao uteis para drills de staging ou solicitacoes de SOC que reduzem temporariamente o duty cycle sem reescrever configs.

## Guardrails de MTU

- Celulas de payload usam 1,024 B mais ~96 B de framing Norito+Noise e os headers QUIC/UDP minimos, mantendo cada datagrama abaixo do MTU minimo de IPv6 de 1,280 B.
- Quando tuneis (WireGuard/IPsec) adicionam encapsulamento extra voce **deve** reduzir `padding.cell_size` para que `cell_size + framing <= 1,280 B`. O validador do relay aplica `padding.cell_size <= 1,136 B` (1,280 B - 48 B de overhead UDP/IPv6 - 96 B de framing).
- Perfis `core` devem fixar >=4 neighbors mesmo em idle para que lanes dummy sempre cubram um subconjunto de guards PQ. Perfis `home` podem limitar circuitos de taxa constante a wallets/aggregators, mas devem aplicar back-pressure quando a saturacao exceder 70% por tres janelas de telemetry.

## Telemetria e alertas

Relays exportam as seguintes metricas por preset:

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

Alerta quando:

1. O dummy ratio fica abaixo do piso do preset (`core >= 4/8`, `home >= 2/2`, `null >= 1/1`) por mais de duas janelas.
2. `soranet_constant_rate_ceiling_hits_total` cresce mais rapido do que um hit a cada cinco minutos.
3. `soranet_constant_rate_degraded` muda para `1` fora de um drill planejado.

Registre o label do preset e a lista de neighbors nos relatorios de incidente para que auditores possam comprovar que as politicas de taxa constante corresponderam aos requisitos do roadmap.
