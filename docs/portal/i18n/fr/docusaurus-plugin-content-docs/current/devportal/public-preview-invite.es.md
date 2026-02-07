---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Playbook des invitations de l'aperçu public

## Objets du programme

Ce playbook explique comment annoncer et exécuter l'aperçu public une fois que le
workflow d'intégration des réviseurs est actif. Garder honnêtement la feuille de route DOCS-SORA à
assurer que chaque invitation soit envoyée avec des artefacts vérifiables, une garantie de sécurité et un
camino claro de feedback.

- **Audience :** liste des membres de la communauté, des partenaires et des mainteneurs que
  confirmer la politique d’utilisation acceptable de l’aperçu.
- **Limites :** tamano de ola por defecto <= 25 réviseurs, vente d'accès de 14 jours, réponse
  un incident en 24h.

## Checklist de porte de lancement

Complétez ces tarifs avant d'envoyer toute invitation :

1. Ultimos artefactos de preview chargés en CI (`docs-portal-preview`,
   manifeste de somme de contrôle, descripteur, bundle SoraFS).
2. `npm run --prefix docs/portal serve` (gateado por checksum) testé sur la même balise.
3. Billets d'intégration des réviseurs approuvés et envoyés à la veille des invitations.
4. Documents de sécurité, d'observabilité et d'incidents validés
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Formulaire de commentaires ou plante de problème préparée (y compris les champs de gravité,
   étapes de reproduction, captures d'écran et informations de retour).
6. Texte de l'annonce révisé par Docs/DevRel + Governance.## Paquet d'invitation

Chaque invitation doit inclure :

1. **Artefacts verificados** - Proporciona enlaces al manifiesto/plan de SoraFS o a los
   les artefacts de GitHub sont le manifeste de la somme de contrôle et le descripteur. Référence au commando
   de vérification explicite pour que les réviseurs puissent l'exécuter avant le lever du soleil
   le site.
2. **Instrucciones de serve** - Inclut la commande d'aperçu déclenchée par la somme de contrôle :

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Enregistreurs de sécurité** - Indique que les jetons expirent automatiquement, les liens
   il ne faut pas comparer et les incidents doivent être signalés immédiatement.
4. **Canal de feedback** - Affichez la plante/formulaire et déclarez les attentes de temps de réponse.
5. **Fechas del programa** - Proportiona fechas de inicio/fin, office hours o syncs, and the proxima
   fenêtre de rafraîchissement.

El email de muestra fr
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
cubre estos requisitos. Actualiser les espaces réservés (fiches, URL, contacts)
avant d'envoyer.

## Exposer l'hôte de l'aperçu

Promouvoir uniquement l'hôte en avant-première une fois que l'intégration est complète et le ticket de changement
est approuvé. Consultez la [guide d'exposition de l'hôte de prévisualisation](./preview-host-exposure.md)
pour les étapes de construction/publication/vérification de bout en bout utilisées dans cette section.

1. **Construire et empaquetado :** Marquer la balise de publication et produire des artefacts déterminés.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```Le script de pin décrit `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   et `portal.dns-cutover.json` sous `artifacts/sorafs/`. Ajouter ces archives à la main
   invitations pour que chaque réviseur puisse vérifier les mêmes bits.

2. **Publier l'alias de l'aperçu :** Répéter la commande sans `--skip-submit`
   (proporciona `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, et la vérification de l'alias émis
   par gouvernement). Le script enlazara le manifeste à `docs-preview.sora` et l'émettra
   `portal.manifest.submit.summary.json` mais `portal.pin.report.json` pour le paquet de preuves.

3. **Probar el despliegue:** Confirma que l'alias apparaît et que la somme de contrôle coïncide avec la balise
   avant d'envoyer des invitations.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Mantener `npm run serve` (`scripts/serve-verified-preview.mjs`) à la main comme solution de secours pour
   que les réviseurs peuvent faire une copie locale si le bord de l'aperçu tombe.

## Chronologie des communications| Dia | Accion | Propriétaire |
| --- | --- | --- |
| J-3 | Finaliser la copie de l'invitation, rafraîchir les artefacts, essai de vérification | Docs/DevRel |
| J-2 | Signature de la gouvernance + ticket de changement | Docs/DevRel + Gouvernance |
| J-1 | Envoyez des invitations en utilisant la plante, actualisez le tracker avec la liste des destinations | Docs/DevRel |
| D | Appel de lancement / heures de bureau, surveillance des tableaux de bord de télémétrie | Docs/DevRel + Sur appel |
| J+7 | Résumé des commentaires de mitad de ola, triage des problèmes bloqués | Docs/DevRel |
| J+14 | Cerrar ola, revocar acceso temporal, publier CV en `status.md` | Docs/DevRel |

## Suivi de l'accès et de la télémétrie

1. Enregistrer chaque destinataire, horodatage d'invitation et date de révocation avec le
   aperçu de l'enregistreur de commentaires (ver
   [`preview-feedback-log`](./preview-feedback-log)) pour que chaque personne partage le même
   rastro de preuve:

   ```bash
   # Agrega un nuevo evento de invitacion a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Los eventos soportados fils `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened` et `access-revoked`. El log vive fr
   `artifacts/docs_portal_preview/feedback_log.json` par défaut ; adjoint au ticket de
   la ola de invitaciones junto con los formularios de consentementimiento. Utiliser l'assistant de
   résumé pour produire un curriculum vitae auditable avant la note de pierre :

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```Le résumé JSON énumère les invitations à venir, les destinations ouvertes, les contenus de
   feedback et l'horodatage de l'événement plus récent. L'assistant est répondu par
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Ainsi, le même workflow peut être exécuté localement sur CI. Utiliser la plante de digestion fr
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   al publicar el recap de la ola.
2. Étiquette des tableaux de bord de télémétrie avec le `DOCS_RELEASE_TAG` utilisé pour la fenêtre pour cela
   Les photos peuvent être corrélées avec les cohortes d'invitation.
3. Exécutez `npm run probe:portal -- --expect-release=<tag>` après le déploiement pour confirmer
   que l'arrivée de l'aperçu annonce les métadonnées correctes de la version.
4. Enregistrez tout incident sur la plante du runbook et insérez-le dans la cohorte.

## Feedback et écrire1. Regroupez les commentaires dans un document partagé ou dans le tableau des problèmes. Étiquette des articles avec
   `docs-preview/<wave>` pour que les propriétaires de la feuille de route puissent consulter facilement.
2. Utiliser le résumé de sortie de l'enregistreur d'aperçu pour publier le rapport de la tâche, puis reprendre
   la cohorte en `status.md` (participantes, hallazgos principales, fixes planeados) et
   actualisez `roadmap.md` si le hito DOCS-SORA change.
3. Suivez les étapes de l'offboarding
   [`reviewer-onboarding`](./reviewer-onboarding.md) : accès refusé, demandes d'archives et
   agradece a los participants.
4. Préparez la prochaine étape pour rafraîchir les artefacts, relancez les portes de la somme de contrôle et
   actualiser la plante d'invitation avec de nouvelles demandes.

Appliquer ce playbook de manière cohérente pour maintenir le programme de prévisualisation auditable et
le fait par Docs/DevRel une forme répétitive d'escalade des invitations à travers le portail
je cherche un GA.