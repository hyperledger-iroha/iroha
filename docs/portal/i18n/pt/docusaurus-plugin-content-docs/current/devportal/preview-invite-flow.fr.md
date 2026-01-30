---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Flux d'invitation preview

## Objectif

L'element de roadmap **DOCS-SORA** cite l'onboarding des relecteurs et le programme d'invitations preview public comme derniers bloqueurs avant la sortie de beta. Cette page decrit comment ouvrir chaque vague d'invitations, quels artefacts doivent etre livres avant d'envoyer les invites et comment prouver que le flux est auditable. Utilisez-la avec:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) pour la gestion par relecteur.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) pour les garanties de checksum.
- [`devportal/observability`](./observability.md) pour les exports de telemetrie et hooks d'alerting.

## Plan des vagues

| Vague | Audience | Criteres d'entree | Criteres de sortie | Notes |
| --- | --- | --- | --- | --- |
| **W0 - Maintainers core** | Maintainers Docs/SDK validant le contenu jour un. | Equipe GitHub `docs-portal-preview` peuplee, gate checksum `npm run serve` en vert, Alertmanager silencieux 7 jours. | Tous les docs P0 relus, backlog tague, aucun incident bloquant. | Sert a valider le flux; pas d'email d'invitation, seulement partage des artefacts preview. |
| **W1 - Partners** | Operateurs SoraFS, integrateurs Torii, relecteurs gouvernance sous NDA. | W0 termine, termes juridiques approuves, proxy Try-it en staging. | Sign-off partners collecte (issue ou formulaire signe), telemetrie montre <=10 relecteurs concurrents, pas de regressions securite pendant 14 jours. | Imposer template d'invitation + tickets de demande. |
| **W2 - Communaute** | Contributeurs selectionnes depuis la liste d'attente communaute. | W1 termine, drills d'incidents repetes, FAQ publique mise a jour. | Feedback digere, >=2 releases doc expediees via pipeline preview sans rollback. | Limiter invites concurrentes (<=25) et batcher chaque semaine. |

Documentez quelle vague est active dans `status.md` et dans le tracker des demandes preview afin que la gouvernance voie le statut d'un coup d'oeil.

## Checklist preflight

Terminez ces actions **avant** de planifier des invitations pour une vague:

1. **Artefacts CI disponibles**
   - Dernier `docs-portal-preview` + descriptor charge par `.github/workflows/docs-portal-preview.yml`.
   - Pin SoraFS note dans `docs/portal/docs/devportal/deploy-guide.md` (descriptor de cutover present).
2. **Application du checksum**
   - `docs/portal/scripts/serve-verified-preview.mjs` invoque via `npm run serve`.
   - Instructions `scripts/preview_verify.sh` testees sur macOS + Linux.
3. **Baseline telemetrie**
   - `dashboards/grafana/docs_portal.json` montre un trafic Try it sain et l'alerte `docs.preview.integrity` est au vert.
   - Derniere annexe de `docs/portal/docs/devportal/observability.md` mise a jour avec des liens Grafana.
4. **Artefacts gouvernance**
   - Issue du invite tracker prete (une issue par vague).
   - Template de registre des relecteurs copie (voir [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Approbations legales et SRE requises attachees a l'issue.

Enregistrez l'achevement du preflight dans le invite tracker avant d'envoyer le moindre email.

## Etapes du flux

1. **Selectionner les candidats**
   - Tirer depuis la liste d'attente ou la file partners.
   - S'assurer que chaque candidat a un template de demande complete.
2. **Approuver l'acces**
   - Assigner un approbateur a l'issue du invite tracker.
   - Verifier les prerequis (CLA/contrat, usage acceptable, brief securite).
3. **Envoyer les invitations**
   - Completer les placeholders de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contacts).
   - Joindre le descriptor + hash d'archive, l'URL de staging Try it et les canaux support.
   - Stocker l'email final (ou transcript Matrix/Slack) dans l'issue.
4. **Suivre l'onboarding**
   - Mettre a jour le invite tracker avec `invite_sent_at`, `expected_exit_at`, et le statut (`pending`, `active`, `complete`, `revoked`).
   - Lier la demande d'entree du relecteur pour auditabilite.
5. **Surveiller la telemetrie**
   - Surveiller `docs.preview.session_active` et les alertes `TryItProxyErrors`.
   - Ouvrir un incident si la telemetrie devie du baseline et enregistrer le resultat a cote de l'entree d'invitation.
6. **Collecter le feedback et sortir**
   - Clore les invitations lorsque le feedback arrive ou que `expected_exit_at` est atteint.
   - Mettre a jour l'issue de vague avec un court resume (constats, incidents, prochaines actions) avant de passer au cohort suivant.

## Evidence & reporting

| Artefact | Ou stocker | Cadence de mise a jour |
| --- | --- | --- |
| Issue du invite tracker | Projet GitHub `docs-portal-preview` | Mettre a jour apres chaque invite. |
| Export du roster relecteurs | Registre lie dans `docs/portal/docs/devportal/reviewer-onboarding.md` | Hebdomadaire. |
| Snapshots telemetrie | `docs/source/sdk/android/readiness/dashboards/<date>/` (reutiliser le bundle telemetrie) | Par vague + apres incidents. |
| Digest feedback | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (creer un dossier par vague) | Dans les 5 jours suivant la sortie de vague. |
| Note de reunion gouvernance | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | A remplir avant chaque sync gouvernance DOCS-SORA. |

Lancez `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
apres chaque batch pour produire un digest lisible par machine. Joignez le JSON rendu a l'issue de vague pour que les relecteurs gouvernance confirment les comptes d'invitations sans rejouer tout le log.

Joignez la liste de preuves a `status.md` a chaque fin de vague afin que l'entree roadmap puisse etre mise a jour rapidement.

## Criteres de rollback et de pause

Mettez en pause le flux d'invitations (et notifiez la gouvernance) lorsque l'un des cas suivants survient:

- Incident proxy Try it ayant necessite un rollback (`npm run manage:tryit-proxy`).
- Fatigue d'alertes: >3 alert pages pour les endpoints preview-only sur 7 jours.
- Ecart de conformite: invitation envoyee sans termes signes ou sans enregistrement du template de demande.
- Risque d'integrite: mismatch de checksum detecte par `scripts/preview_verify.sh`.

Reprenez uniquement apres avoir documente la remediation dans le invite tracker et confirme que le dashboard telemetrie est stable au moins 48 heures.
