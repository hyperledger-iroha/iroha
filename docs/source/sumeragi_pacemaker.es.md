---
lang: es
direction: ltr
source: docs/source/sumeragi_pacemaker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b0e7a0ca09fb294fed62bae221a6fd82ce07d9cf0802d90d960afaa5bcec40e9
source_last_modified: "2025-12-09T14:07:26.178015+00:00"
translation_last_reviewed: 2026-01-01
---

# Sumeragi Pacemaker - Temporizadores, backoff y jitter

Esta nota describe la politica de pacemaker (timer) para Sumeragi y ofrece guia para operadores y ejemplos. Los temporizadores gobiernan cuando los lideres proponen y cuando los validadores sugieren/entran a una nueva vista tras inactividad.

Estado: implementada la ventana base derivada de EMA, el piso RTT y los topes configurables de jitter/backoff. El intervalo de propuesta del pacemaker ahora se limita entre el objetivo de block-time y el propose timeout con piso RTT, limitado por `sumeragi.advanced.pacemaker.max_backoff_ms`. (`effective_block_time_ms` = `block_time_ms` scaled by `pacing_factor_bps`). El jitter se aplica de forma determinista por nodo y por (height, view).

## Conceptos
- Ventana base: media movil exponencial de las fases de consenso observadas
  (propose, collect_da, collect_prevote, collect_precommit, commit). La EMA se
  inicia desde `sumeragi.advanced.npos.timeouts.*_ms`; hasta que lleguen suficientes
  muestras, coincide efectivamente con los valores por defecto. La EMA de
  `collect_aggregator` se exporta para observabilidad pero no se incluye en la
  ventana del pacemaker. Los valores suavizados aparecen via
  `sumeragi_phase_latency_ema_ms{phase=...}`.
- Multiplicador de backoff: `sumeragi.advanced.pacemaker.backoff_multiplier` (por defecto 1). Cada timeout agrega `base * multiplier` a la ventana actual.
- Piso RTT: `avg_rtt_ms * sumeragi.advanced.pacemaker.rtt_floor_multiplier` (por defecto 2). Evita timeouts demasiado agresivos en enlaces de alta latencia.
- Cap: `sumeragi.advanced.pacemaker.max_backoff_ms` (por defecto 60_000 ms). Techo duro de la ventana.
- Semilla del intervalo de propuesta: `max(effective_block_time_ms, propose_timeout_ms * rtt_floor_multiplier)` y nunca por encima de `sumeragi.advanced.pacemaker.max_backoff_ms`. (`effective_block_time_ms` = `block_time_ms` scaled by `pacing_factor_bps`). Este es el intervalo en estado estable incluso sin backoff.

Actualizacion de ventana efectiva en timeout:
- `window = min(cap, max(window + base * backoff_mul, avg_rtt * rtt_floor_mul))`
- Cuando no hay muestras RTT, el piso RTT es 0.

Telemetria expuesta (ver telemetry.md):
- Runtime: `sumeragi_pacemaker_backoff_ms`, `sumeragi_pacemaker_rtt_floor_ms`, `sumeragi_phase_latency_ema_ms{phase=...}`
- REST snapshot: `/v2/sumeragi/phases` ahora incluye `ema_ms` junto con las latencias por fase mas recientes para que los dashboards puedan graficar la tendencia EMA sin consultar Prometheus directamente.
- Config: `sumeragi.advanced.pacemaker.backoff_multiplier`, `sumeragi.advanced.pacemaker.rtt_floor_multiplier`, `sumeragi.advanced.pacemaker.max_backoff_ms`

## Politica de jitter
Para evitar efectos de manada (timeouts sincronizados), el pacemaker soporta un jitter pequeno por nodo alrededor de la ventana efectiva.

Jitter determinista por nodo:
- Fuente: `blake2(chain_id || peer_id || height || view)` -> 64-bit, escalado a [-J, +J].
- Banda de jitter recomendada: +-10% de la `window` calculada.
- Aplicar una vez por (height, view); no variar dentro de la misma vista.

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
- El jitter es determinista por nodo y (height, view) y no requiere aleatoriedad externa.
- Mantenga la banda pequena (<= 10%) para preservar la respuesta y reducir estampidas.
- El jitter afecta la liveness pero no la seguridad.

Telemetria:
- `sumeragi_pacemaker_jitter_ms` - magnitud absoluta de jitter (ms) aplicada.
- `sumeragi_pacemaker_jitter_frac_permille` - banda de jitter configurada (permille).

## Guia para operadores

LAN/PoA de baja latencia
- backoff_multiplier: 1-2
- rtt_floor_multiplier: 2
- max_backoff_ms: 5_000-10_000
- Razon: mantener propuestas frecuentes; recuperacion corta bajo stalls.

WAN geodistribuida
- backoff_multiplier: 2-3
- rtt_floor_multiplier: 3-5
- max_backoff_ms: 30_000-60_000
- Razon: evitar churn agresivo en enlaces de alto RTT; tolerar bursts.

Redes de alta variancia o moviles
- backoff_multiplier: 3-4
- rtt_floor_multiplier: 4-6
- max_backoff_ms: 60_000
- Razon: minimizar cambios de vista sincronizados; considere habilitar jitter cuando el despliegue tolere commits mas lentos.

## Ejemplos

1) LAN con RTT promedio ~= 2 ms, base = 2000 ms, cap = 10_000 ms
- backoff_mul = 1, rtt_floor_mul = 2
- Primer timeout: window = max(0 + 2000, 2*2) = 2000 ms
- Segundo timeout: window = min(10_000, 2000 + 2000) = 4000 ms

2) WAN con RTT promedio ~= 80 ms, base = 4000 ms, cap = 60_000 ms
- backoff_mul = 2, rtt_floor_mul = 3
- Primer timeout: window = max(0 + 8000, 80*3) = 8000 ms
- Segundo timeout: window = min(60_000, 8000 + 8000) = 16_000 ms

3) Con banda de jitter 10% (ilustrativo, no implementado)
- window = 16_000 ms, jitter in [-1_600, +1_600] ms -> `window' in [14_400, 17_600]` ms por nodo

## Monitoreo
- Trend de backoff: `avg_over_time(sumeragi_pacemaker_backoff_ms[5m])`
- Inspeccionar piso RTT: `avg_over_time(sumeragi_pacemaker_rtt_floor_ms[5m])`
- Comparar EMA vs histogramas: `sumeragi_phase_latency_ema_ms{phase=...}` junto con el percentil correspondiente de `sumeragi_phase_latency_ms{phase=...}`
- Verificar config: `max(sumeragi.advanced.pacemaker.backoff_multiplier)`, `max(sumeragi.advanced.pacemaker.rtt_floor_multiplier)`, `max(sumeragi.advanced.pacemaker.max_backoff_ms)`

## Determinismo y seguridad
- Temporizadores/backoff/jitter solo influyen en cuando los nodos disparan propuestas/cambios de vista; no afectan la validez de firmas ni las reglas de commit certificate.
- Mantenga cualquier aleatoriedad determinista por nodo y por (height, view). Evite hora del dia o RNG del SO en rutas criticas de consenso.
