---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Playbook des convites en avant-première publique

## Objets du programme

Ce playbook explique comment annoncer et exécuter l'aperçu public comme tel
workflow d'intégration des réviseurs est actif. Ele mantem o roadmap DOCS-SORA honnêtement ao
garantir que cada convite saia com artefatos verificaveis, orientacao de seguranca e um
chemin clair pour les commentaires.

- **Audience :** liste des membres de la communauté, des partenaires et des responsables qui s'associent à
  politique d'utilisation aceitavel faire un aperçu.
- **Limites :** tamanho de onda padrao <= 25 réviseurs, janvier 14 jours, réponse
  un incident dans 24h.

## Checklist de porte de lancement

Complétez ces tarifs avant d'envoyer n'importe quel convite :

1. Ultimos artefatos de preview envoyés na CI (`docs-portal-preview`,
   manifeste de somme de contrôle, descripteur, bundle SoraFS).
2. `npm run --prefix docs/portal serve` (gateado por checksum) testé sans balise mésmo.
3. Billets d'intégration des réviseurs aprovados et vinculados à l'onda des convites.
4. Documents de sécurité, observabilité et incidents validés
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Formulaire de commentaires ou modèle de problème préparé (y compris les champs de gravité,
   passes de reproduction, captures d'écran et informations ambiantes).
6. Copier l'annonce révisée par Docs/DevRel + Governance.

## Pacote de conviteChaque convite doit inclure :

1. **Artefatos verificados** - Liens Forneca pour le manifeste/plan SoraFS ou pour les artefatos
   GitHub, mais le manifeste de la somme de contrôle et le descripteur. Référence explicite à la commande
   de verificacao para que os revisores possam executa-lo antes de subir le site.
2. **Instrucoes de serve** - Inclut la commande d'aperçu déclenchée par la somme de contrôle :

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Lembretes de seguranca** - Informe que les jetons expirent automatiquement, les liens ne se développent pas
   Les partages et incidents doivent être signalés immédiatement.
4. **Canal de feedback** - Lien vers le modèle/formulaire et indication du délai de réponse attendu.
5. **Données du programme** - Informe les données de démarrage/d'enregistrement, les heures de bureau ou les synchronisations, et à proximité
   Janela de rafraîchir.

Un e-mail d'exemple
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
cobre esses requisitos. Actualiser les espaces réservés du système d'exploitation (données, URL, contacts)
avant d'envoyer.

## Afficher l'hôte de l'aperçu

Alors promouvez l'hôte de l'aperçu lorsque l'intégration est terminée et le ticket de mudanca est arrivé
approuvé. Voir le [guide d'exposition de l'hôte de prévisualisation](./preview-host-exposure.md) pour les étapes
de bout en bout de build/publish/verify utilisé dans ce secao.

1. **Build e empacotamento :** Marquez la balise de publication et produisez des articles déterminés.

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
   ```Le script de pin grava `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   et `portal.dns-cutover.json` à `artifacts/sorafs/`. Anexe esses arquivos a onda de convites
   pour que chaque réviseur puisse vérifier mes bits.

2. **Publier ou alias de prévisualisation :** Roulé ou commandé avec `--skip-submit`
   (forneca `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, et une preuve d'alias émis
   par la gouvernance). Le script va amarrer ou manifester un `docs-preview.sora` et émettre
   `portal.manifest.submit.summary.json` mais `portal.pin.report.json` pour l'ensemble des preuves.

3. **Tester le déploiement :** Confirmez que la résolution de l'alias et que la somme de contrôle correspond à la balise
   antes de enviar convites.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Mantenha `npm run serve` (`scripts/serve-verified-preview.mjs`) à mao comme solution de repli pour
   que les réviseurs peuvent subir une copie locale se ou le bord de l'aperçu falhar.

## Chronologie de la communication| Dia | Açao | Propriétaire |
| --- | --- | --- |
| J-3 | Finaliser la copie de la convocation, actualiser les artéfacts, essai à sec de vérification | Docs/DevRel |
| J-2 | Signature de gouvernance + ticket de mudanca | Docs/DevRel + Gouvernance |
| J-1 | Envoyer des invitations en utilisant un modèle, actualiser le suivi avec la liste des destinations | Docs/DevRel |
| D | Appel de lancement / heures de bureau, surveiller les tableaux de bord de télémétrie | Docs/DevRel + Sur appel |
| J+7 | Digest de feedback no meio da onda, triage des problèmes bloqués | Docs/DevRel |
| J+14 | Fechar a onda, revogar acesso temporario, publier le CV dans `status.md` | Docs/DevRel |

## Suivi de l'accès et de la télémétrie

1. Enregistrez chaque destinataire, horodatage de la convocation et données de voyage avec
   aperçu de l'enregistreur de commentaires (voir
   [`preview-feedback-log`](./preview-feedback-log)) pour que chaque personne partage ou partage
   rastro de évidences :

   ```bash
   # Adicione um novo evento de convite a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Les événements pris en charge par `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened` et `access-revoked`. O log fica em
   `artifacts/docs_portal_preview/feedback_log.json` par père ; anexe ao ticket da
   onda de convites junto com os formularios de consentement. Utilisez l'assistant de
   résumé pour produire un audit roll-up avant la note d'encerclement :

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```Le résumé JSON énumération convites por onda, destinations abertos, contagens de
   les commentaires et l'horodatage de l'événement le plus récent. O helper e apoiado por
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Ainsi, mon workflow peut être exécuté localement ou en CI. Utilisez un modèle pour les digérer
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   ao publicar o recap da onda.
2. Utilisez les tableaux de bord de télémétrie avec le `DOCS_RELEASE_TAG` utilisé sur l'onde pour cela
   picos possam être correlacionados com as coortes de convite.
3. Rode `npm run probe:portal -- --expect-release=<tag>` apos o déployer pour confirmer que
   L'ambiance de l'aperçu annonce la correspondance des métadonnées de la version.
4. Enregistrez tout incident qu'aucun modèle ne fait du runbook et Vincule à votre cœur.

## Commentaires et commentaires

1. Regroupez vos commentaires dans un document partagé ou sur le tableau des problèmes. Marquer les articles com
   `docs-preview/<wave>` pour que les propriétaires établissent facilement la feuille de route.
2. Utilisez un résumé de l'enregistreur d'aperçu pour prévisualiser ou rapporter l'onde, ensuite
   resuma a coorte em `status.md` (participants, principaux achados, fixes planejados) e
   actualisez `roadmap.md` avec le marco DOCS-SORA mudou.
3. Suivez les étapes d'offboarding de
   [`reviewer-onboarding`](./reviewer-onboarding.md) : réouvrir l'accès, archiver les sollicitations et
   agradeca os participants.
4. Préparez une prochaine étape pour actualiser les artéfacts, réexécuter les portes de la somme de contrôle et
   actualisation du modèle de convocation avec de nouvelles données.Appliquer ce playbook de manière cohérente sur le long terme ou un programme de prévisualisation vérifiable et
oferece ao Docs/DevRel un chemin répétitif pour escalader vers le milieu du portail
se rapproche de GA.