---
lang: fr
direction: ltr
source: docs/source/runbooks/torii_norito_rpc_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 969f24fd53af5e5714eb4b257dc086b3990550a5085c9464805f689c7d8ee324
source_last_modified: "2025-12-14T09:53:36.247266+00:00"
translation_last_reviewed: 2025-12-28
---

# FAQ opérateur Norito-RPC

Cette FAQ synthétise les leviers de rollout/rollback, la télémétrie et les
artefacts de preuve référencés dans les items **NRPC-2** et **NRPC-4** afin que
les opérateurs disposent d’une page unique lors des canaries, brownouts ou drills
d’incident. À traiter comme la porte d’entrée des handoffs d’astreinte ; les
procédures détaillées restent dans
`docs/source/torii/norito_rpc_rollout_plan.md` et
`docs/source/runbooks/torii_norito_rpc_canary.md`.

## 1. Paramètres de configuration

| Chemin | Objectif | Valeurs autorisées / notes |
|------|---------|------------------------|
| `torii.transport.norito_rpc.enabled` | Interrupteur on/off dur du transport Norito. | `true` garde les handlers HTTP enregistrés ; `false` les désactive quel que soit le stage. |
| `torii.transport.norito_rpc.require_mtls` | Imposer mTLS pour les endpoints Norito. | Défaut `true`. À désactiver uniquement en pools de staging isolés. |
| `torii.transport.norito_rpc.allowed_clients` | Liste blanche des comptes de service / tokens API autorisés. | Fournir blocs CIDR, hashes de tokens, ou OIDC client IDs selon le déploiement. |
| `torii.transport.norito_rpc.stage` | Stage de rollout annoncé aux SDKs. | `disabled` (rejette Norito, force JSON), `canary` (allowlist uniquement, télémétrie renforcée), `ga` (par défaut pour chaque client authentifié). |
| `torii.preauth_scheme_limits.norito_rpc` | Concurrence par schéma + budget de burst. | Reprend les clés des throttles HTTP/WS (ex. `max_in_flight`, `rate_per_sec`). Augmenter le cap sans mettre à jour Alertmanager annule la protection de rollout. |
| `transport.norito_rpc.*` dans `docs/source/config/client_api.md` | Overrides côté client (CLI / SDK discovery). | Utiliser `cargo xtask client-api-config diff` pour inspecter les changements avant push vers Torii. |

**Flux brownout recommandé**

1. Définir `torii.transport.norito_rpc.stage=disabled`.
2. Garder `enabled=true` pour que les probes/tests d’alerte continuent de solliciter les handlers.
3. Mettre `torii.preauth_scheme_limits.norito_rpc.max_in_flight` à zéro si un arrêt immédiat est requis
   (par ex. en attendant la propagation de config).
4. Mettre à jour le journal opérateur et joindre le digest de nouvelle config au rapport de stage.

## 2. Checklists opérationnelles

- **Canary / staging** — suivre `docs/source/runbooks/torii_norito_rpc_canary.md`.
  Ce runbook référence les mêmes clés de config et liste les artefacts de preuve
  capturés par `scripts/run_norito_rpc_smoke.sh` +
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh`.
- **Promotion production** — exécuter le modèle de stage report dans
  `docs/source/torii/norito_rpc_stage_reports.md`. Enregistrer le hash de config,
  le hash de l’allowlist, le digest du bundle smoke, le hash d’export Grafana et
  l’identifiant du drill d’alertes.
- **Rollback** — remettre `stage` à `disabled`, conserver l’allowlist et
  documenter le switch dans le stage report + journal d’incident. Une fois la
  cause racine corrigée, rejouer la checklist canary avant de remettre `stage=ga`.

## 3. Télémétrie & alertes

| Actif | Emplacement | Notes |
|-------|----------|-------|
| Dashboard | `dashboards/grafana/torii_norito_rpc_observability.json` | Suit le taux de requêtes, codes d’erreur, tailles de payload, échecs de décodage et % d’adoption. |
| Alertes | `dashboards/alerts/torii_norito_rpc_rules.yml` | Gates `NoritoRpcErrorBudget`, `NoritoRpcDecodeFailures`, et `NoritoRpcFallbackSpike`. |
| Script chaos | `scripts/telemetry/test_torii_norito_rpc_alerts.sh` | Fait échouer la CI si les expressions d’alerte dérivent. À lancer après chaque changement de config. |
| Smoke tests | `python/iroha_python/scripts/run_norito_rpc_smoke.sh`, `cargo xtask norito-rpc-verify` | Inclure leurs logs dans le bundle de preuve pour chaque promotion. |

Les dashboards doivent être exportés et attachés au ticket de release (`make
docs-portal-dashboards` en CI) afin que les astreintes puissent rejouer les
métriques sans accès à Grafana production.

## 4. Questions fréquentes

**Comment autoriser un nouveau SDK pendant le canary ?**  
Ajoutez le compte de service/token à `torii.transport.norito_rpc.allowed_clients`,
rechargez Torii et consignez le changement dans `docs/source/torii/norito_rpc_tracker.md`
sous NRPC-2R. Le propriétaire SDK doit aussi capturer une exécution de fixtures via
`scripts/run_norito_rpc_fixtures.sh --sdk <label>`.

**Que se passe‑t‑il si le décodage Norito échoue en plein rollout ?**  
Laissez `stage=canary`, gardez `enabled=true` et triez les échecs via
`torii_norito_decode_failures_total`. Les owners SDK peuvent revenir à JSON en
omis `Accept: application/x-norito` ; Torii continuera à servir JSON jusqu’au
retour à `ga`.

**Comment prouver que la gateway sert le bon manifeste ?**  
Exécutez `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario norito-rpc --host
<gateway-host>` pour que la sonde enregistre les headers `Sora-Proof` en plus du
config digest Norito. Joindre la sortie JSON au stage report.

**Où capturer les overrides de redaction ?**  
Documentez chaque override temporaire dans la colonne `Notes` du stage report et
consignez le patch de config Norito sous change control. Les overrides expirent
automatiquement dans le fichier de config ; cette FAQ rappelle aux astreintes de
faire le nettoyage après incident.

Pour toute question non couverte, escalader via les canaux listés dans le runbook
canary (`docs/source/runbooks/torii_norito_rpc_canary.md`).

## 5. Extrait de note de release (suivi OPS-NRPC)

L’item **OPS-NRPC** requiert une note de release prête à l’emploi afin que les
opérateurs annoncent le rollout Norito‑RPC de façon cohérente. Copiez le bloc
ci‑dessous dans le prochain post de release (remplacez les champs entre crochets)
et joignez le bundle de preuve décrit ensuite.

> **Transport Torii Norito-RPC** — Les enveloppes Norito sont désormais servies
> aux côtés de l’API JSON. Le flag `torii.transport.norito_rpc.stage` est livré
> à **[stage: disabled/canary/ga]** et suit la checklist de rollout par étapes
> dans `docs/source/torii/norito_rpc_rollout_plan.md`. Les opérateurs peuvent se
> désengager temporairement en définissant `torii.transport.norito_rpc.stage=disabled`
> tout en conservant `torii.transport.norito_rpc.enabled=true` ; les SDKs
> retombent automatiquement sur JSON. Les dashboards télémétrie
> (`dashboards/grafana/torii_norito_rpc_observability.json`) et drills d’alerte
> (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) restent obligatoires
> avant d’élever le stage, et les artefacts canary/smoke capturés par
> `python/iroha_python/scripts/run_norito_rpc_smoke.sh` doivent être joints au
> ticket de release.

Avant publication :

1. Remplacez le marqueur **[stage: …]** par le stage annoncé dans Torii.
2. Liez le ticket de release au stage report le plus récent dans
   `docs/source/torii/norito_rpc_stage_reports.md`.
3. Téléversez les exports Grafana/Alertmanager mentionnés ci‑dessus avec les
   hashes du bundle smoke de `scripts/run_norito_rpc_smoke.sh`.

Cet extrait satisfait l’exigence de note OPS-NRPC sans forcer les incident
commanders à reformuler le statut du rollout à chaque fois.
