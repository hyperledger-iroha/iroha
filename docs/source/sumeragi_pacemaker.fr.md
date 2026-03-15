---
lang: fr
direction: ltr
source: docs/source/sumeragi_pacemaker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b0e7a0ca09fb294fed62bae221a6fd82ce07d9cf0802d90d960afaa5bcec40e9
source_last_modified: "2025-12-09T14:07:26.178015+00:00"
translation_last_reviewed: 2026-01-01
---

# Sumeragi Pacemaker - Timers, backoff, et jitter

Cette note decrit la politique de pacemaker (timer) pour Sumeragi et fournit des conseils operateur et des exemples. Les timers gouvernent quand les leaders proposent et quand les validateurs suggerent/entrent dans une nouvelle vue apres inactivite.

Statut: fenetre de base derivee de l EMA, plancher RTT, et plafonds de jitter/backoff configurables implementes. L intervalle de proposition du pacemaker est maintenant borne entre la cible de block-time et le propose timeout avec plancher RTT, plafonne par `sumeragi.advanced.pacemaker.max_backoff_ms`. (`effective_block_time_ms` = `block_time_ms` scaled by `pacing_factor_bps`). Le jitter est applique de maniere deterministe par noeud et par (height, view).

## Concepts
- Fenetre de base: moyenne mobile exponentielle des phases de consensus observees
  (propose, collect_da, collect_prevote, collect_precommit, commit). L EMA est
  initialisee depuis `sumeragi.advanced.npos.timeouts.*_ms`; jusqu a suffisamment
  d echantillons elle correspond aux valeurs par defaut. L EMA de
  `collect_aggregator` est exportee pour l observabilite mais n est pas incluse
  dans la fenetre pacemaker. Les valeurs lissees apparaissent via
  `sumeragi_phase_latency_ema_ms{phase=...}`.
- Multiplicateur de backoff: `sumeragi.advanced.pacemaker.backoff_multiplier` (defaut 1). Chaque timeout ajoute `base * multiplier` a la fenetre courante.
- Plancher RTT: `avg_rtt_ms * sumeragi.advanced.pacemaker.rtt_floor_multiplier` (defaut 2). Evite des timeouts trop agressifs sur des liens a latence elevee.
- Cap: `sumeragi.advanced.pacemaker.max_backoff_ms` (defaut 60_000 ms). Plafond strict de la fenetre.
- Graine d intervalle de proposition: `max(effective_block_time_ms, propose_timeout_ms * rtt_floor_multiplier)` et jamais au-dela de `sumeragi.advanced.pacemaker.max_backoff_ms`. (`effective_block_time_ms` = `block_time_ms` scaled by `pacing_factor_bps`). C est l intervalle en regime etabli meme sans backoff.

Mise a jour de la fenetre effective sur timeout:
- `window = min(cap, max(window + base * backoff_mul, avg_rtt * rtt_floor_mul))`
- Quand il n y a pas d echantillons RTT, le plancher RTT est 0.

Telemetrie exposee (voir telemetry.md):
- Runtime: `sumeragi_pacemaker_backoff_ms`, `sumeragi_pacemaker_rtt_floor_ms`, `sumeragi_phase_latency_ema_ms{phase=...}`
- Snapshot REST: `/v2/sumeragi/phases` inclut maintenant `ema_ms` a cote des latences par phase les plus recentes afin que les dashboards puissent tracer la tendance EMA sans interroger Prometheus directement.
- Config: `sumeragi.advanced.pacemaker.backoff_multiplier`, `sumeragi.advanced.pacemaker.rtt_floor_multiplier`, `sumeragi.advanced.pacemaker.max_backoff_ms`

## Politique de jitter
Pour eviter les effets de troupeau (timeouts synchronises), le pacemaker supporte un petit jitter par noeud autour de la fenetre effective.

Jitter deterministe par noeud:
- Source: `blake2(chain_id || peer_id || height || view)` -> 64-bit, echelle en [-J, +J].
- Bande de jitter recommandee: +-10% de la `window` calculee.
- Appliquer une fois par (height, view) ; ne pas varier dans la meme vue.

Pseudocode:
```
base = ema_total_ms(view, height)  // seeded by sumeragi.advanced.npos.timeouts.*_ms
window = min(cap, max(prev + base * backoff_mul, avg_rtt * rtt_floor_mul))
seed = blake2(chain_id || peer_id || height || view)
u = (seed % 10_000) as f64 / 10_000.0  // [0, 1)
jfrac = 0.10 // 10% jitter band
jitter = (u * 2.0 - 1.0) * jfrac * window.as_millis() as f64
window_jittered_ms = (window.as_millis() as f64 + jitter).clamp(0.0, cap_ms as f64)
```

Notes:
- Le jitter est deterministe par noeud et (height, view) et ne requiert pas d alea externe.
- Garder une petite bande (<= 10%) pour preserver la reactivite tout en reduisant les stampedes.
- Le jitter affecte la liveness mais pas la securite.

Telemetrie:
- `sumeragi_pacemaker_jitter_ms` - magnitude absolue du jitter applique (ms).
- `sumeragi_pacemaker_jitter_frac_permille` - bande de jitter configuree (permille).

## Conseils operateur

LAN/PoA faible latence
- backoff_multiplier: 1-2
- rtt_floor_multiplier: 2
- max_backoff_ms: 5_000-10_000
- Rationale: garder des propositions frequentes; recuperation courte en cas de stall.

WAN geodistribue
- backoff_multiplier: 2-3
- rtt_floor_multiplier: 3-5
- max_backoff_ms: 30_000-60_000
- Rationale: eviter un churn agressif sur des liens RTT eleves; tolerer les bursts.

Reseaux a variance elevee ou mobiles
- backoff_multiplier: 3-4
- rtt_floor_multiplier: 4-6
- max_backoff_ms: 60_000
- Rationale: minimiser les changements de vue synchronises; envisager d activer le jitter si le deploiement tolere des commits plus lents.

## Exemples

1) LAN avec RTT moyen ~= 2 ms, base = 2000 ms, cap = 10_000 ms
- backoff_mul = 1, rtt_floor_mul = 2
- Premier timeout: window = max(0 + 2000, 2*2) = 2000 ms
- Deuxieme timeout: window = min(10_000, 2000 + 2000) = 4000 ms

2) WAN avec RTT moyen ~= 80 ms, base = 4000 ms, cap = 60_000 ms
- backoff_mul = 2, rtt_floor_mul = 3
- Premier timeout: window = max(0 + 8000, 80*3) = 8000 ms
- Deuxieme timeout: window = min(60_000, 8000 + 8000) = 16_000 ms

3) Avec bande de jitter 10% (illustratif, non implemente)
- window = 16_000 ms, jitter in [-1_600, +1_600] ms -> `window' in [14_400, 17_600]` ms par noeud

## Monitoring
- Tendance backoff: `avg_over_time(sumeragi_pacemaker_backoff_ms[5m])`
- Inspecter le plancher RTT: `avg_over_time(sumeragi_pacemaker_rtt_floor_ms[5m])`
- Comparer EMA vs histogrammes: `sumeragi_phase_latency_ema_ms{phase=...}` avec les percentiles correspondants de `sumeragi_phase_latency_ms{phase=...}`
- Verifier config: `max(sumeragi.advanced.pacemaker.backoff_multiplier)`, `max(sumeragi.advanced.pacemaker.rtt_floor_multiplier)`, `max(sumeragi.advanced.pacemaker.max_backoff_ms)`

## Determinisme et securite
- Timers/backoff/jitter n influencent que le moment ou les noeuds declenchent des propositions/changements de vue; ils n affectent pas la validite des signatures ni les regles de commit certificate.
- Garder toute alea deterministe par noeud et par (height, view). Eviter l heure du jour ou le RNG de l OS dans les chemins critiques du consensus.
