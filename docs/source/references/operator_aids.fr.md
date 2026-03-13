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
- GET `/v2/sumeragi/new_view`
  - Instantané des comptes de réception NEW_VIEW par `(height, view)`.
  - Format : `{ "ts_ms": <u64>, "items": [{ "height": <u64>, "view": <u64>, "count": <u64> }, ...] }`
  - Exemple :
    - `curl -s http://127.0.0.1:8080/v2/sumeragi/new_view | jq .`
- GET `/v2/sumeragi/new_view/sse` (SSE)
  - Flux périodique (≈1 s) du même payload pour les tableaux de bord.
  - Exemple :
    - `curl -Ns http://127.0.0.1:8080/v2/sumeragi/new_view/sse`
- Métriques : les jauges `sumeragi_new_view_receipts_by_hv{height,view}` reflètent les comptes.
- GET `/v2/sumeragi/status`
  - Instantané de l’index du leader, Highest/Locked QCs (`highest_qc`/`locked_qc`, hauteurs, vues, hachages de sujet), compteurs des collecteurs/VRF, reports du pacemaker, profondeur de la file des transactions et santé du store RBC (`rbc_store.{sessions,bytes,pressure_level,persist_drops_total,evictions_total,recent_evictions[...]}`).
- GET `/v2/sumeragi/status/sse`
  - Flux SSE (≈1 s) du même payload que `/v2/sumeragi/status` pour les tableaux de bord en direct.
- GET `/v2/sumeragi/qc`
  - Instantané des highest/locked QCs ; inclut `subject_block_hash` pour le highest QC lorsqu’il est connu.
- GET `/v2/sumeragi/pacemaker`
  - Minuteries/configuration du pacemaker : `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`.
- GET `/v2/sumeragi/leader`
  - Instantané de l’index du leader. En mode NPoS, inclut le contexte PRF : `{ height, view, epoch_seed }`.
- GET `/v2/sumeragi/collectors`
  - Plan déterministe des collecteurs dérivé de la topologie engagée et des paramètres on-chain : exporte `mode`, le plan `(height, view)` (avec `height` égal à la hauteur actuelle de la chaîne), `collectors_k`, `redundant_send_r`, `proxy_tail_index`, `min_votes_for_commit`, la liste ordonnée des collecteurs et `epoch_seed` (hex) lorsque NPoS est actif.
- GET `/v2/sumeragi/params`
  - Instantané des paramètres Sumeragi on-chain `{ block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`.
  - Lorsque `da_enabled` est true, la preuve de disponibilité (`availability evidence` ou RBC `READY`) est suivie mais le commit ne l’attend pas ; le `DELIVER` local RBC n’est pas non plus requis. Les opérateurs peuvent confirmer la santé du transport des payloads via les endpoints RBC ci-dessous.
- GET `/v2/sumeragi/rbc`
  - Compteurs agrégés de Reliable Broadcast : `{ sessions_active, sessions_pruned_total, ready_broadcasts_total, ready_rebroadcasts_skipped_total, deliver_broadcasts_total, payload_bytes_delivered_total, payload_rebroadcasts_skipped_total }`.
- GET `/v2/sumeragi/rbc/sessions`
  - Instantané de l’état par session (hash de bloc, height/view, comptes de chunks, indicateur delivered, marqueur `invalid`, hash de payload, booléen recovered) pour diagnostiquer les livraisons RBC bloquées et mettre en évidence les sessions récupérées après redémarrage.
  - Raccourci CLI : `iroha --output-format text ops sumeragi rbc sessions` imprime `hash`, `height/view`, progression des chunks, compteur de ready et indicateurs invalid/delivered.

Preuves (audit ; hors consensus)
- GET `/v2/sumeragi/evidence/count` → `{ "count": <u64> }`
- GET `/v2/sumeragi/evidence` → `{ "total": <u64>, "items": [...] }`
  - Inclut des champs de base (p. ex., DoublePrepare/DoubleCommit, InvalidQc, InvalidProposal) pour inspection.
  - Exemples :
    - `curl -s http://127.0.0.1:8080/v2/sumeragi/evidence/count | jq .`
    - `curl -s http://127.0.0.1:8080/v2/sumeragi/evidence | jq .`
- POST `/v2/sumeragi/evidence` → `{ "status": "accepted", "kind": "<variant>" }`
  - Aides CLI :
    - `iroha --output-format text ops sumeragi evidence list`
    - `iroha --output-format text ops sumeragi evidence count`
    - `iroha ops sumeragi evidence submit --evidence-hex <hex>` (ou `--evidence-hex-file <path>`)

Authentification opérateur (WebAuthn/mTLS)
- POST `/v2/operator/auth/registration/options`
  - Retourne les options d’inscription WebAuthn (`publicKey`) pour l’enrôlement initial des identifiants.
- POST `/v2/operator/auth/registration/verify`
  - Vérifie la charge d’attestation WebAuthn et persiste l’identifiant opérateur.
- POST `/v2/operator/auth/login/options`
  - Retourne les options d’authentification WebAuthn (`publicKey`) pour la connexion opérateur.
- POST `/v2/operator/auth/login/verify`
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

Extraits CLI de surveillance (bash)

- Interroger le snapshot JSON toutes les 2 s (affiche les 10 dernières entrées) :

```bash
#!/usr/bin/env bash
set -euo pipefail
TORII="${TORII:-http://127.0.0.1:8080}"
INTERVAL="${INTERVAL:-2}"
TOKEN="${TOKEN:-}"
HDR=()
if [[ -n "$TOKEN" ]]; then HDR=(-H "x-api-token: $TOKEN"); fi
while true; do
  curl -s "${HDR[@]}" "$TORII/v2/sumeragi/new_view" \
    | jq -c '{ts_ms, items:(.items|sort_by([.height,.view])|reverse|.[:10])}'
  sleep "$INTERVAL"
done
```

- Suivre le flux SSE et formater (10 dernières entrées) :

```bash
#!/usr/bin/env bash
set -euo pipefail
TORII="${TORII:-http://127.0.0.1:8080}"
TOKEN="${TOKEN:-}"
HDR=()
if [[ -n "$TOKEN" ]]; then HDR=(-H "x-api-token: $TOKEN"); fi
curl -Ns "${HDR[@]}" "$TORII/v2/sumeragi/new_view/sse" \
  | awk '/^data:/{sub(/^data: /,""); print}' \
  | jq -c '{ts_ms, items:(.items|sort_by([.height,.view])|reverse|.[:10])}'
```
