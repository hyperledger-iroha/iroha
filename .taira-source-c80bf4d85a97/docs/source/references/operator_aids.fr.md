---
lang: fr
direction: ltr
source: docs/source/references/operator_aids.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf412f2645cea9d5468f4541ff48d4ace67bc6f3d60a97e68561dde4949ff9be
source_last_modified: "2025-12-13T10:25:50.323533+00:00"
translation_last_reviewed: 2026-01-01
---

# Points de terminaison Torii — aide opérateur (référence rapide)

Cette page répertorie les endpoints non consensuels, destinés aux opérateurs, qui aident à la visibilité et au dépannage. Les réponses sont en JSON sauf indication contraire.

Consensus (Sumeragi)
- Métriques : les jauges `sumeragi_new_view_receipts_by_hv{height,view}` reflètent les comptes.
- GET `/v1/sumeragi/status`
  - Instantané de l’index du leader, Highest/Locked QCs (`highest_qc`/`locked_qc`, hauteurs, vues, hachages de sujet), compteurs des collecteurs/VRF, reports du pacemaker, profondeur de la file des transactions et santé du store RBC (`rbc_store.{sessions,bytes,pressure_level,persist_drops_total,evictions_total,recent_evictions[...]}`).
- GET `/v1/sumeragi/status/sse`
  - Flux SSE (≈1 s) du même payload que `/v1/sumeragi/status` pour les tableaux de bord en direct.
- GET `/v1/sumeragi/qc`
  - Instantané des highest/locked QCs ; inclut `subject_block_hash` pour le highest QC lorsqu’il est connu.
- GET `/v1/sumeragi/pacemaker`
  - Minuteries/configuration du pacemaker : `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`.
- GET `/v1/sumeragi/leader`
  - Instantané de l’index du leader. En mode NPoS, inclut le contexte PRF : `{ height, view, epoch_seed }`.
- GET `/v1/sumeragi/telemetry`
  - Aggregated consensus telemetry: `availability.collectors` contains observed collector indices, peer IDs, and ingested-vote counts; `rbc_backlog` contains missing-chunk totals; `rbc_pending` contains bounded pre-session queue totals, drops, and limits. This is not a deterministic collector plan or a per-session RBC contract.
- GET `/v1/sumeragi/params`
  - Instantané des paramètres Sumeragi on-chain `{ block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`.
  - When `da_enabled` is true, availability evidence is tracked but does not gate commit; local payload is required and can be satisfied via RBC `DELIVER` or block sync. Use the aggregated telemetry endpoint, Prometheus counters, status snapshots, and logs to diagnose payload transport.

Preuves (audit ; hors consensus)
- GET `/v1/sumeragi/evidence/count` → `{ "count": <u64> }`
- GET `/v1/sumeragi/evidence` → `{ "total": <u64>, "items": [...] }`
  - Inclut des champs de base (p. ex., DoublePrepare/DoubleCommit, InvalidQc, InvalidProposal) pour inspection.
  - Exemples :
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence/count | jq .`
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence | jq .`
- POST `/v1/sumeragi/evidence` → `{ "status": "accepted", "kind": "<variant>" }`
  - Aides CLI :
    - `iroha --output-format text ops sumeragi evidence list`
    - `iroha --output-format text ops sumeragi evidence count`
    - `iroha ops sumeragi evidence submit --evidence-hex <hex>` (ou `--evidence-hex-file <path>`)

Authentification opérateur (WebAuthn/mTLS)
- POST `/v1/operator/auth/registration/options`
  - Retourne les options d’inscription WebAuthn (`publicKey`) pour l’enrôlement initial des identifiants.
- POST `/v1/operator/auth/registration/verify`
  - Vérifie la charge d’attestation WebAuthn et persiste l’identifiant opérateur.
- POST `/v1/operator/auth/login/options`
  - Retourne les options d’authentification WebAuthn (`publicKey`) pour la connexion opérateur.
- POST `/v1/operator/auth/login/verify`
  - Vérifie la charge d’assertion WebAuthn et renvoie un jeton de session opérateur.
- En-têtes :
  - `x-iroha-operator-session` : jeton de session pour les endpoints opérateur (émis par login verify).
  - `x-iroha-operator-token` : jeton bootstrap (autorisé lorsque `torii.operator_auth.token_fallback` le permet).
  - `x-api-token` : requis lorsque `torii.require_api_token = true` ou `torii.operator_auth.token_source = "api"`.
  - `x-forwarded-client-cert` : requis lorsque `torii.operator_auth.require_mtls = true` (défini par le proxy d’entrée).
- Flux d’enrôlement :
  1. Appelez registration options avec un jeton bootstrap (autorisé uniquement avant l’enrôlement du premier identifiant lorsque `token_fallback = "bootstrap"`).
  2. Exécutez `navigator.credentials.create` dans l’UI opérateur et envoyez l’attestation à registration verify.
  3. Appelez login options puis login verify pour obtenir `x-iroha-operator-session`.
  4. Envoyez `x-iroha-operator-session` sur les endpoints opérateur.

Notes
- Ces endpoints sont des vues locales au nœud (en mémoire lorsque indiqué) et n’affectent ni le consensus ni la persistance.
- L’accès peut être protégé par des jetons API, l’authentification opérateur (WebAuthn/mTLS) et des limites de débit selon la configuration Torii.
