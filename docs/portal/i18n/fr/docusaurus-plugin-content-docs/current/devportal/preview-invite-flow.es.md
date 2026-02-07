---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Flux d'invitations de prévisualisation

## Proposition

L'élément de la feuille de route **DOCS-SORA** identifie l'intégration des réviseurs et le programme d'invitations de prévisualisation publique comme les bloqueurs finaux avant que le portail ne lance la version bêta. Cette page décrit comment ouvrir chaque invitation, que les artefacts doivent envoyer avant le mandat d'invitation et comment démontrer que le flux est auditable. Utilisez-le avec :

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) pour le travail du réviseur.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) pour garantir la somme de contrôle.
- [`devportal/observability`](./observability.md) pour les exportations de télémétrie et les crochets d'alerte.

## Plan de vacances| Ola | Audience | Critères d'entrée | Critères de sortie | Notes |
| --- | --- | --- | --- | --- |
| **W0 - Noyau des responsables** | Les responsables de Docs/SDK valident le contenu du jour. | Equipé de GitHub `docs-portal-preview`, porte de somme de contrôle et `npm run serve` en vert, Alertmanager silencieux pendant 7 jours. | Tous les documents P0 révisés, étiquette du backlog, sans incidents bloqués. | Utilisez-le pour valider le flux ; pas d'e-mail d'invitation, seul vous partagerez les artefacts de prévisualisation. |
| **W1 - Partenaires** | Opérateurs SoraFS, intégrateurs Torii, réviseurs de gouvernement sous NDA. | W0 certifié, termes légaux approuvés, proxy Try-it et staging. | Sign-off des partenaires (émission du formulaire ferme) reconnu, la télémétrie doit être =2 versions de la documentation envoyées via le pipeline de prévisualisation sans restauration. | Limiter les invitations concurrentes (<=25) et s'inscrire automatiquement. |

Documenta qui est activé sur `status.md` et sur le tracker de sollicitudes de prévisualisation pour que la gouvernance voie l'état d'une vue.

## Checklist de contrôle en amontComplétez ces actions **avant** de programmer des invitations pour une personne :

1. **Artefacts de CI disponibles**
   - Le dernier `docs-portal-preview` + descripteur chargé par `.github/workflows/docs-portal-preview.yml`.
   - Pin de SoraFS annoté en `docs/portal/docs/devportal/deploy-guide.md` (descripteur de cutover présent).
2. **Application de la somme de contrôle**
   - `docs/portal/scripts/serve-verified-preview.mjs` invoqué via `npm run serve`.
   - Instrucciones de `scripts/preview_verify.sh` testées sur macOS + Linux.
3. **Base de télémétrie**
   - `dashboards/grafana/docs_portal.json` muestra trafico Essayez-le saluablement et l'alerte `docs.preview.integrity` est en vert.
   - Dernière annexe de `docs/portal/docs/devportal/observability.md` actualisée avec les liens de Grafana.
4. **Artefacts de gouvernement**
   - Émettre la liste de suivi des invitations (un problème pour moi).
   - Plantilla de registro de revisores copiada (ver [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Aprobaciónes legales y de SRE requeridas adjuntas a la issue.

Enregistrez la finalisation du contrôle en amont dans le tracker d'invitation avant d'envoyer n'importe quel courrier.

## Pas de flux1. **Sélectionner les candidats**
   - Extraire de la journée d'espoir ou du cadeau des partenaires.
   - Assurez-vous que chaque candidat ait la plante de sollicitude complète.
2. **Accès autorisé**
   - Attribuer un demandeur au numéro du tracker d'invitation.
   - Vérifier les prérequis (CLA/contrato, uso acceptable, brief de seguridad).
3. **Envoyer des invitations**
   - Compléter les espaces réservés de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contacts).
   - Ajouter le descripteur + hash de l'archive, URL de staging de Try it et canaux de support.
   - Garder l'e-mail final (ou la transcription de Matrix/Slack) dans le numéro.
4. **Intégration de Rastrear**
   - Actualiser le tracker d'invitation avec `invite_sent_at`, `expected_exit_at` et l'état (`pending`, `active`, `complete`, `revoked`).
   - Demander la demande d'admission du réviseur pour l'auditabilité.
5. **Télémétrie de surveillance arrière**
   - Vigilar `docs.preview.session_active` et alertes `TryItProxyErrors`.
   - Réparer un incident si la télémétrie est effectuée via la ligne de base et enregistrer le résultat conjointement à l'entrée de l'invitation.
6. **Recolectar feedback and certar**
   - Cerrar invitaciones cuando el feedback llegue o `expected_exit_at` se cumpla.
   - Actualiser le numéro de la ola avec un résumé court (hallazgos, incidents, suivantes actions) avant de passer à la cohorte suivante.

## Preuves et rapports| Artefact | Donde guardar | Cadence de mise à jour |
| --- | --- | --- |
| Problème de suivi des invitations | Projet GitHub `docs-portal-preview` | Actualiser après chaque invitation. |
| Exporter la liste des réviseurs | Registre inscrit en `docs/portal/docs/devportal/reviewer-onboarding.md` | Sémanal. |
| Instantanés de télémétrie | `docs/source/sdk/android/readiness/dashboards/<date>/` (reusar bundle de télémétrie) | Por ola + malgré les incidents. |
| Résumé des retours d'expérience | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (créer un tapis pour une personne) | Dans les 5 jours pour sortir de l'ola. |
| Note de réunion de gouvernement | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Compléter avant chaque synchronisation de gouvernement DOCS-SORA. |

Éjecté `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
après chaque lot pour produire un résumé lisible pour les machines. Ajoutez le rendu JSON au numéro de l'ola pour que les réviseurs d'État confirment les conteos d'invitations sans reproduire tout le journal.

Ajoutez la liste des preuves à `status.md` chaque fois qu'une chose se termine pour que l'entrée de la feuille de route puisse être actualisée rapidement.

## Critères de restauration et de pause

Mettez en pause le flux des invitations (et notifiez le gouvernement) lorsque vous recevez n'importe quel cas de ce type :- Un incident de proxy Essayez-le qui nécessite une restauration (`npm run manage:tryit-proxy`).
- Liste des alertes : > 3 pages d'alerte pour les points finaux en un seul aperçu dans les 7 jours.
- Brecha de cumplimiento: invitacion enviada sin terminos firmados o sin registrar la plantilla de sollicitud.
- Risque d'intégrité : non-concordance de la somme de contrôle détectée par `scripts/preview_verify.sh`.

Je n'ai qu'à documenter la correction sur le tracker d'invitation et à confirmer que le tableau de bord de télémétrie est prêt au moins 48 heures.