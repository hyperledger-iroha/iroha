---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w0/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa22a2b67386f5a9be051cb7318e048d35db7cde545cbaf05e9809c5af6a2e7b
source_last_modified: "2025-11-14T04:43:19.819238+00:00"
translation_last_reviewed: 2026-01-30
---

| Element | Details |
| --- | --- |
| Vague | W0 - Maintainers core |
| Date du digest | 2025-03-27 |
| Fenetre de review | 2025-03-25 -> 2025-04-08 |
| Participants | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observability-01 |
| Tag d'artefact | `preview-2025-03-24` |

## Points saillants

1. **Workflow de checksum** - Tous les reviewers ont confirme que `scripts/preview_verify.sh`
   a reussi contre le couple descriptor/archive partage. Aucun override manuel requis.
2. **Retours de navigation** - Deux problemes mineurs d'ordre du sidebar ont ete signales
   (`docs-preview/w0 #1-#2`). Les deux sont routes vers Docs/DevRel et ne bloquent pas la
   vague.
3. **Parite des runbooks SoraFS** - sorafs-ops-01 a demande des liens croises plus clairs
   entre `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. Issue de suivi ouverte;
   a traiter avant W1.
4. **Revue de telemetrie** - observability-01 a confirme que `docs.preview.integrity`,
   `TryItProxyErrors` et les logs du proxy Try-it sont restes au vert; aucune alerte n'a
   ete declenchee.

## Actions

| ID | Description | Responsable | Statut |
| --- | --- | --- | --- |
| W0-A1 | Reordonner les entrees du sidebar du devportal pour mettre en avant les docs pour reviewers (`preview-invite-*` regroupes). | Docs-core-01 | Termine - le sidebar liste maintenant les docs reviewers de facon contigue (`docs/portal/sidebars.js`). |
| W0-A2 | Ajouter un lien croise explicite entre `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Termine - chaque runbook pointe desormais vers l'autre pour que les operateurs voient les deux guides pendant les rollouts. |
| W0-A3 | Partager des snapshots de telemetrie + bundle de requetes avec le tracker de governance. | Observability-01 | Termine - bundle attache a `DOCS-SORA-Preview-W0`. |

## Resume de sortie (2025-04-08)

- Les cinq reviewers ont confirme la fin, purge les builds locaux et quitte la fenetre de
  preview; les revocations d'acces sont enregistrees dans `DOCS-SORA-Preview-W0`.
- Aucun incident ni alerte pendant la vague; les dashboards de telemetrie sont restes verts
  pendant toute la periode.
- Les actions de navigation + liens croises (W0-A1/A2) sont implementees et refletees dans
  les docs ci-dessus; la preuve telemetrie (W0-A3) est attachee au tracker.
- Bundle de preuve archive: screenshots de telemetrie, accuses d'invitation et ce digest
  sont lies depuis l'issue du tracker.

## Prochaines etapes

- Implementer les actions W0 avant d'ouvrir W1.
- Obtenir l'approbation legale et un slot de staging pour le proxy, puis suivre les etapes de
  preflight de la vague partenaires detaillees dans le [preview invite flow](../../preview-invite-flow.md).

_Ce digest est lie depuis le [preview invite tracker](../../preview-invite-tracker.md) pour
garder le roadmap DOCS-SORA tracable._
