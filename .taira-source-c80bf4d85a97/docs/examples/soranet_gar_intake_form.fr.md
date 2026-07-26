---
lang: fr
direction: ltr
source: docs/examples/soranet_gar_intake_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6cd4da7e590d581719ed2607994d7d9eb16d153fbd06f85655d0da37c727853a
source_last_modified: "2025-11-21T15:16:55.013762+00:00"
translation_last_reviewed: 2026-01-01
---

# Modele d'intake GAR SoraNet

Utilisez ce formulaire d'intake lors d'une demande d'action GAR (purge, ttl override, rate
ceiling, moderation directive, geofence, ou legal hold). Le formulaire soumis doit etre
epingle a cote des outputs de `gar_controller` afin que les logs d'audit et les recus
citent les memes URI de preuve.

| Champ | Valeur | Notes |
|-------|--------|-------|
| ID de demande |  | ID de ticket guardian/ops. |
| Demande par |  | Compte + contact. |
| Date/heure (UTC) |  | Quand l'action doit demarrer. |
| Nom GAR |  | par ex., `docs.sora`. |
| Hote canonique |  | par ex., `docs.gw.sora.net`. |
| Action |  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`. |
| Override TTL (secondes) |  | Requis uniquement pour `ttl_override`. |
| Plafond de taux (RPS) |  | Requis uniquement pour `rate_limit_override`. |
| Regions autorisees |  | Liste de regions ISO lors d'une demande `geo_fence`. |
| Regions refusees |  | Liste de regions ISO lors d'une demande `geo_fence`. |
| Slugs de moderation |  | Doit correspondre aux directives de moderation GAR. |
| Tags de purge |  | Tags a purger avant diffusion. |
| Labels |  | Labels machine (incident id, drill name, pop scope). |
| URI de preuve |  | Logs/dashboards/specs justifiant la demande. |
| URI d'audit |  | URI d'audit par pop si differente des defaults. |
| Expiration demandee |  | Unix timestamp ou RFC3339; laisser vide pour le default. |
| Raison |  | Explication cote utilisateur; apparait dans les recus et dashboards. |
| Approbateur |  | Approbateur guardian/comite pour la demande. |

### Etapes de soumission

1. Remplissez le tableau et joignez-le au ticket de governance.
2. Mettez a jour la config du GAR controller (`policies`/`pops`) avec des `labels`/`evidence_uris`/`expires_at_unix` correspondants.
3. Lancez `cargo xtask soranet-gar-controller ...` pour emettre des evenements/recus.
4. Deposez `gar_controller_summary.json`, `gar_reconciliation_report.json`, `gar_metrics.prom`, et `gar_audit_log.jsonl` dans le meme ticket. L'approbateur confirme que le nombre de recus correspond a la liste PoP avant envoi.
