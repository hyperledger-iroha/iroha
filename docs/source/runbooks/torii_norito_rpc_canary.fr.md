---
lang: fr
direction: ltr
source: docs/source/runbooks/torii_norito_rpc_canary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44aa7b618a377a4a62f3be210ce6b9ccc6906b7791c4bbf6d3f60899fe08e7b8
source_last_modified: "2025-12-14T09:53:36.245431+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook canary Torii Norito-RPC (NRPC-2C)

Ce runbook opérationnalise le plan de rollout **NRPC-2** en décrivant comment
promouvoir le transport Norito‑RPC depuis la validation en labo staging vers le
stage “canary” en production. À lire en parallèle de :

- [`docs/source/torii/nrpc_spec.md`](../torii/nrpc_spec.md) (contrat protocolaire)
- [`docs/source/torii/norito_rpc_rollout_plan.md`](../torii/norito_rpc_rollout_plan.md)
- [`docs/source/torii/norito_rpc_telemetry.md`](../torii/norito_rpc_telemetry.md)

## Rôles & entrées

| Rôle | Responsabilité |
|------|----------------|
| Torii Platform TL | Approuve les deltas de config, signe les smoke tests. |
| NetOps | Applique les changements ingress/envoy et surveille la santé du pool canary. |
| Liaison observabilité | Vérifie dashboards/alertes et capture les preuves. |
| Platform Ops | Porte le ticket de changement, coordonne le rehearsal rollback, met à jour les trackers. |

Artefacts requis :

- Dernier patch Norito `iroha_config` avec `transport.norito_rpc.stage = "canary"` et
  `transport.norito_rpc.allowed_clients` renseigné.
- Snippet de config Envoy/Nginx qui préserve `Content-Type: application/x-norito` et
  impose le profil mTLS des clients canary (`defaults/torii_ingress_mtls.yaml`).
- Allowlist de tokens (YAML ou manifeste Norito) pour les clients canary.
- URL Grafana + token API pour `dashboards/grafana/torii_norito_rpc_observability.json`.
- Accès au harness smoke de parité
  (`python/iroha_python/scripts/run_norito_rpc_smoke.sh`) et au script de drill d’alertes
  (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`).

## Checklist pre‑flight

1. **Freeze spec confirmé.** Assurez‑vous que le hash de `docs/source/torii/nrpc_spec.md`
   correspond au dernier release signé et qu’aucune PR en attente ne touche le header/layout Norito.
2. **Validation config.** Exécutez
   ```bash
   cargo xtask validate-config --config <patch.json> --schema client_api
   ```
   pour confirmer que les nouvelles entrées `transport.norito_rpc.*` se parsènt.
3. **Caps par schéma.** Définir `torii.preauth_scheme_limits.norito_rpc` de manière
   conservatrice (ex. 25 connexions concurrentes) pour que les appels binaires ne
   saturent pas le trafic JSON.
4. **Rehearsal ingress.** Appliquer le patch Envoy en staging, rejouer le test négatif
   (`cargo test -p iroha_torii -- norito_ingress`) et confirmer que les headers
   retirés sont rejetés avec HTTP 415.
5. **Sanity télémétrie.** En staging, exécuter `scripts/telemetry/test_torii_norito_rpc_alerts.sh
   --env staging --dry-run` et joindre le bundle de preuves généré.
6. **Inventaire tokens.** Vérifier que l’allowlist canary inclut au moins deux opérateurs
   par région ; stocker le manifeste dans `artifacts/norito_rpc/<YYYYMMDD>/allowlist.json`.
7. **Ticketing.** Ouvrir le ticket de changement avec fenêtre start/end, plan de rollback,
   et liens vers ce runbook + preuves télémétrie.

## Procédure de promotion canary

1. **Appliquer le patch de config.**
   - Déployer le delta `iroha_config` (stage=`canary`, allowlist renseignée,
     limites de schéma configurées) via admission.
   - Redémarrer ou hot‑reload Torii, confirmer que le patch est reconnu via les
     logs `torii.config.reload`.
2. **Mettre à jour l’ingress.**
   - Déployer la config Envoy/Nginx qui active le routage d’en‑têtes Norito / profil mTLS
     pour le pool canary.
   - Vérifier que les réponses `curl -vk --cert <client.pem>` incluent les en‑têtes
     Norito `X-Iroha-Error-Code` lorsque requis.
3. **Smoke tests.**
   - Exécuter `python/iroha_python/scripts/run_norito_rpc_smoke.sh --profile canary`
     depuis le bastion canary. Capturer les transcriptions JSON + Norito et les
     stocker sous `artifacts/norito_rpc/<YYYYMMDD>/smoke/`.
   - Enregistrer les hashes dans `docs/source/torii/norito_rpc_stage_reports.md`.
4. **Observer la télémétrie.**
   - Surveiller `torii_active_connections_total{scheme="norito_rpc"}` et
     `torii_request_duration_seconds_bucket{scheme="norito_rpc"}` pendant au moins 30 minutes.
   - Exporter le dashboard Grafana via API et l’attacher au ticket de changement.
5. **Rehearsal alertes.**
   - Exécuter `scripts/telemetry/test_torii_norito_rpc_alerts.sh --env canary` pour
     injecter des enveloppes Norito malformées ; s’assurer qu’Alertmanager enregistre
     l’incident synthétique et se nettoie automatiquement.
6. **Capture des preuves.**
   - Mettre à jour `docs/source/torii/norito_rpc_stage_reports.md` avec :
     - Digest de config
     - Hash du manifeste allowlist
     - Timestamp du smoke test
     - Checksum de l’export Grafana
     - ID du drill d’alertes
   - Téléverser les artefacts vers `artifacts/norito_rpc/<YYYYMMDD>/`.

## Monitoring & critères de sortie

Rester en canary jusqu’à ce que toutes les conditions suivantes soient vraies pendant ≥72 h :

- Taux d’erreur (`torii_request_failures_total{scheme="norito_rpc"}`) ≤1 % et pas de
  pics soutenus sur `torii_norito_decode_failures_total`.
- Parité de latence (`p95` Norito vs JSON) dans un écart de 10 %.
- Dashboard d’alertes silencieux hors drills planifiés.
- Opérateurs de l’allowlist soumettent des rapports de parité sans mismatch de schéma.

Documenter l’état quotidien dans le ticket de changement et capturer des snapshots dans
`docs/source/status/norito_rpc_canary_log.md` (si présent).

## Procédure de rollback

1. Repassez `transport.norito_rpc.stage` à `"disabled"` et videz `allowed_clients` ;
   appliquez via admission.
2. Retirez le stanza route/mTLS Envoy/Nginx, rechargez les proxies et confirmez que
   les nouvelles connexions Norito sont refusées.
3. Révoquez les tokens canary (ou désactivez les credentials bearer) pour faire tomber
   les sessions en cours.
4. Surveillez `torii_active_connections_total{scheme="norito_rpc"}` jusqu’à zéro.
5. Relancez le harness smoke JSON‑only pour garantir la fonctionnalité de base.
6. Créez un stub de post‑mortem dans `docs/source/postmortems/norito_rpc_rollback.md`
   sous 24 h et mettez à jour le ticket de changement avec un résumé d’impact + métriques.

## Post‑canary

Lorsque les critères de sortie sont satisfaits :

1. Mettre à jour `docs/source/torii/norito_rpc_stage_reports.md` avec la recommandation GA.
2. Ajouter une entrée `status.md` résumant les résultats canary et les bundles de preuve.
3. Notifier les leads SDK pour qu’ils basculent les fixtures staging vers Norito pour la parité.
4. Préparer le patch de config GA (stage=`ga`, suppression de l’allowlist) et planifier
   la promotion selon le plan NRPC-2.

Suivre ce runbook garantit que chaque promotion canary collecte les mêmes preuves,
maintient un rollback déterministe et satisfait les critères d’acceptation NRPC-2.
