---
lang: pt
direction: ltr
source: docs/source/sumeragi_pacemaker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b0e7a0ca09fb294fed62bae221a6fd82ce07d9cf0802d90d960afaa5bcec40e9
source_last_modified: "2025-12-09T14:07:26.178015+00:00"
translation_last_reviewed: 2026-01-01
---

# Sumeragi Pacemaker - Timers, backoff e jitter

Esta nota descreve a politica de pacemaker (timer) para Sumeragi e fornece orientacao para operadores e exemplos. Timers governam quando lideres propoem e quando validadores sugerem/entram em uma nova view apos inatividade.

Status: janela base derivada de EMA, piso RTT, e limites configuraveis de jitter/backoff implementados. O intervalo de proposta do pacemaker agora e limitado entre o alvo de block-time e o propose timeout com piso RTT, limitado por `sumeragi.advanced.pacemaker.max_backoff_ms`. O jitter e aplicado de forma deterministica por no e por (height, view).

## Conceitos
- Janela base: media movel exponencial das fases de consenso observadas
  (propose, collect_da, collect_prevote, collect_precommit, commit). A EMA e
  semeada a partir de `sumeragi.advanced.npos.timeouts.*_ms`; ate haver amostras suficientes
  ela efetivamente corresponde aos defaults configurados. A EMA de
  `collect_aggregator` e exportada para observabilidade mas nao e incluida na
  janela do pacemaker. Os valores suavizados aparecem via
  `sumeragi_phase_latency_ema_ms{phase=...}`.
- Multiplicador de backoff: `sumeragi.advanced.pacemaker.backoff_multiplier` (padrao 1). Cada timeout adiciona `base * multiplier` a janela atual.
- Piso RTT: `avg_rtt_ms * sumeragi.advanced.pacemaker.rtt_floor_multiplier` (padrao 2). Evita timeouts agressivos em links de alta latencia.
- Cap: `sumeragi.advanced.pacemaker.max_backoff_ms` (padrao 60_000 ms). Teto rigido da janela.
- Semente do intervalo de proposta: `max(block_time_ms, propose_timeout_ms * rtt_floor_multiplier)` e nunca acima de `sumeragi.advanced.pacemaker.max_backoff_ms`. Este e o intervalo em estado estavel mesmo sem backoff.

Atualizacao efetiva da janela no timeout:
- `window = min(cap, max(window + base * backoff_mul, avg_rtt * rtt_floor_mul))`
- Quando nao ha amostras RTT, o piso RTT e 0.

Telemetria exposta (ver telemetry.md):
- Runtime: `sumeragi_pacemaker_backoff_ms`, `sumeragi_pacemaker_rtt_floor_ms`, `sumeragi_phase_latency_ema_ms{phase=...}`
- Snapshot REST: `/v1/sumeragi/phases` agora inclui `ema_ms` junto com as latencias por fase mais recentes para que dashboards possam plotar a tendencia EMA sem consultar Prometheus diretamente.
- Config: `sumeragi.advanced.pacemaker.backoff_multiplier`, `sumeragi.advanced.pacemaker.rtt_floor_multiplier`, `sumeragi.advanced.pacemaker.max_backoff_ms`

## Politica de jitter
Para evitar efeitos de manada (timeouts sincronizados), o pacemaker suporta um pequeno jitter por no ao redor da janela efetiva.

Jitter deterministico por no:
- Fonte: `blake2(chain_id || peer_id || height || view)` -> 64-bit, escalado para [-J, +J].
- Faixa recomendada de jitter: +-10% da `window` calculada.
- Aplicar uma vez por (height, view); nao variar dentro da mesma view.

Pseudocodigo:
```
base = ema_total_ms(view, height)  // seeded by sumeragi.advanced.npos.timeouts.*_ms
window = min(cap, max(prev + base * backoff_mul, avg_rtt * rtt_floor_mul))
seed = blake2(chain_id || peer_id || height || view)
u = (seed % 10_000) as f64 / 10_000.0  // [0, 1)
jfrac = 0.10 // 10% jitter band
jitter = (u * 2.0 - 1.0) * jfrac * window.as_millis() as f64
window_jittered_ms = (window.as_millis() as f64 + jitter).clamp(0.0, cap_ms as f64)
```

Notas:
- O jitter e deterministico por no e (height, view) e nao requer aleatoriedade externa.
- Mantenha a faixa pequena (<= 10%) para preservar a responsividade enquanto reduz stampedes.
- O jitter afeta liveness, mas nao a seguranca.

Telemetria:
- `sumeragi_pacemaker_jitter_ms` - magnitude absoluta do jitter aplicado (ms).
- `sumeragi_pacemaker_jitter_frac_permille` - faixa de jitter configurada (permille).

## Orientacao para operadores

LAN/PoA de baixa latencia
- backoff_multiplier: 1-2
- rtt_floor_multiplier: 2
- max_backoff_ms: 5_000-10_000
- Rationale: manter propostas frequentes; recuperacao curta sob stalls.

WAN geodistribuida
- backoff_multiplier: 2-3
- rtt_floor_multiplier: 3-5
- max_backoff_ms: 30_000-60_000
- Rationale: evitar churn agressivo em links de alto RTT; tolerar bursts.

Redes de alta variancia ou moveis
- backoff_multiplier: 3-4
- rtt_floor_multiplier: 4-6
- max_backoff_ms: 60_000
- Rationale: minimizar mudancas de view sincronizadas; considere habilitar jitter quando o deployment tolerar commits mais lentos.

## Exemplos

1) LAN com RTT medio ~= 2 ms, base = 2000 ms, cap = 10_000 ms
- backoff_mul = 1, rtt_floor_mul = 2
- Primeiro timeout: window = max(0 + 2000, 2*2) = 2000 ms
- Segundo timeout: window = min(10_000, 2000 + 2000) = 4000 ms

2) WAN com RTT medio ~= 80 ms, base = 4000 ms, cap = 60_000 ms
- backoff_mul = 2, rtt_floor_mul = 3
- Primeiro timeout: window = max(0 + 8000, 80*3) = 8000 ms
- Segundo timeout: window = min(60_000, 8000 + 8000) = 16_000 ms

3) Com faixa de jitter 10% (ilustrativo, nao implementado)
- window = 16_000 ms, jitter in [-1_600, +1_600] ms -> `window' in [14_400, 17_600]` ms por no

## Monitoramento
- Trend de backoff: `avg_over_time(sumeragi_pacemaker_backoff_ms[5m])`
- Inspecionar piso RTT: `avg_over_time(sumeragi_pacemaker_rtt_floor_ms[5m])`
- Comparar EMA vs histogramas: `sumeragi_phase_latency_ema_ms{phase=...}` junto com os percentis correspondentes de `sumeragi_phase_latency_ms{phase=...}`
- Verificar config: `max(sumeragi.advanced.pacemaker.backoff_multiplier)`, `max(sumeragi.advanced.pacemaker.rtt_floor_multiplier)`, `max(sumeragi.advanced.pacemaker.max_backoff_ms)`

## Determinismo e seguranca
- Timers/backoff/jitter influenciam apenas quando os nos disparam propostas/mudancas de view; nao afetam validade de assinaturas nem regras de commit certificate.
- Mantenha qualquer aleatoriedade deterministica por no e por (height, view). Evite time-of-day ou RNG do SO em caminhos criticos do consenso.
