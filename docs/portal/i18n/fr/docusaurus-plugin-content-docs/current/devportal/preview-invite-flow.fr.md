---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Aperçu du flux d'invitation

## Objectif

L'élément de roadmap **DOCS-SORA** cite l'onboarding des rélecteurs et le programme d'invitations preview public comme derniers bloqueurs avant la sortie de beta. Cette page décrit comment ouvrir chaque vague d'invitations, quels artefacts doivent être livres avant d'envoyer les invitations et comment prouver que le flux est auditable. Utilisez-la avec :

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) pour la gestion par récepteur.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) pour les garanties de somme de contrôle.
- [`devportal/observability`](./observability.md) pour les exports de télémétrie et hooks d'alerte.

## Plan des vagues| Vague | Public | Critères d'entrée | Critères de sortie | Remarques |
| --- | --- | --- | --- | --- |
| **W0 - Noyau des responsables** | Les Mainteneurs Docs/SDK valident le contenu jour un. | Equipe GitHub `docs-portal-preview` peuplee, gate checksum `npm run serve` en vert, Alertmanager silencieux 7 jours. | Tous les docs P0 relus, backlog tague, aucun incident bloquant. | Sert à valider le flux ; pas d'email d'invitation, seulement partage des artefacts en avant-première. |
| **W1 - Partenaires** | Opérateurs SoraFS, intégrateurs Torii, rélecteurs gouvernance sous NDA. | W0 termine, termes juridiques approuvés, proxy Try-it en staging. | Signature des partenaires collectés (issue ou formulaire signé), télémétrie montre =2 versions de la documentation expédiées via l'aperçu du pipeline sans restauration. | Limiter les invitations concurrentes (<=25) et batcher chaque semaine. |

Documentez quelle vague est active dans `status.md` et dans le tracker des demandes preview afin que la gouvernance voie le statut d'un coup d'oeil.

## Liste de contrôle en amont

Terminez ces actions **avant** de planifier des invitations pour une vague :1. **Artefacts CI disponibles**
   - Dernier `docs-portal-preview` + descripteur charge par `.github/workflows/docs-portal-preview.yml`.
   - Note du pin SoraFS dans `docs/portal/docs/devportal/deploy-guide.md` (descripteur de cutover présent).
2. **Application du checksum**
   - `docs/portal/scripts/serve-verified-preview.mjs` invoqué via `npm run serve`.
   - Instructions `scripts/preview_verify.sh` testés sur macOS + Linux.
3. **Télémétrie de base**
   - `dashboards/grafana/docs_portal.json` montre un trafic Try it sain et l'alerte `docs.preview.integrity` est au vert.
   - Dernière annexe de `docs/portal/docs/devportal/observability.md` mise à jour avec des liens Grafana.
4. **Gouvernance des artefacts**
   - Issue du invitation tracker prete (une issue par vague).
   - Modèle de registre des rélecteurs copie (voir [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Approbations légales et SRE exigences attachées à l'émission.

Enregistrez l'achèvement du contrôle en amont dans le tracker d'invitation avant d'envoyer le moindre email.

## Étapes du flux1. **Sélectionner les candidats**
   - Tirer depuis la liste d'attente ou la file partenaires.
   - S'assurer que chaque candidat a un modèle de demande complet.
2. **Approuver l'accès**
   - Assigner un approbateur à l'issue du invitation tracker.
   - Vérifier les prérequis (CLA/contrat, usage acceptable, brève sécurité).
3. **Envoyer les invitations**
   - Compléter les espaces réservés de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contacts).
   - Joindre le descripteur + hash d'archive, l'URL de staging Try it et les canaux support.
   - Stocker l'email final (ou transcript Matrix/Slack) dans le numéro.
4. **Suivre l'onboarding**
   - Mettre à jour le tracker d'invitation avec `invite_sent_at`, `expected_exit_at`, et le statut (`pending`, `active`, `complete`, `revoked`).
   - Lier la demande d'entrée du récepteur pour auditabilité.
5. **Surveiller la télémétrie**
   - Surveiller `docs.preview.session_active` et les alertes `TryItProxyErrors`.
   - Ouvrir un incident si la télémétrie du système de base et enregistrer le résultat à côté de l'entrée d'invitation.
6. **Collecter le feedback et sortir**
   - Clore les invitations lorsque le feedback arrive ou que `expected_exit_at` est atteint.
   - Mettre à jour l'issue de vague avec un court résumé (statuts, incidents, prochaines actions) avant de passer au cohorte suivante.

## Preuves et rapports| Artefact | Ou stocker | Cadence de mise à jour |
| --- | --- | --- |
| Issue du tracker d'invitation | Projet GitHub `docs-portal-preview` | Mettre un jour après chaque invitation. |
| Export du roster rélecteurs | S'inscrire dans `docs/portal/docs/devportal/reviewer-onboarding.md` | Hebdomadaire. |
| Instantanés télémétrie | `docs/source/sdk/android/readiness/dashboards/<date>/` (réutiliser le bundle télémétrie) | Par vague + après incidents. |
| Résumé des commentaires | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (créer un dossier par vague) | Dans les 5 jours suivant la sortie de vague. |
| Note de gouvernance de la Réunion | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | A remplir avant chaque synchronisation gouvernance DOCS-SORA. |

Lancez `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
après chaque lot pour produire un digest lisible par machine. Joignez le JSON rendu à l'issue de vague pour que les rélecteurs gouvernance confirment les comptes d'invitations sans rejouer tout le log.

Joignez la liste de preuves a `status.md` à chaque fin de vague afin que l'entrée roadmap puisse être mise à jour rapidement.

## Critères de rollback et de pause

Mettez en pause le flux d'invitations (et notifiez la gouvernance) lorsque l'un des cas suivants survient :- Incident proxy Try it ayant nécessité un rollback (`npm run manage:tryit-proxy`).
- Fatigue d'alertes : >3 pages d'alerte pour les endpoints en aperçu uniquement sur 7 jours.
- Ecart de conformité : invitation envoyée sans termes signes ou sans enregistrement du modèle de demande.
- Risque d'intégrité : non-concordance de somme de contrôle détectée par `scripts/preview_verify.sh`.

Reprenez uniquement après avoir documenté la remédiation dans le invite tracker et confirmez que la télémétrie du tableau de bord est stable au moins 48 heures.