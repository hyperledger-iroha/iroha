---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Плейбук приглашений на public preview

## Ce programme

Ce jeu vous permettra d'annoncer et de proposer un aperçu public après cela, car
workflow онбординга ревьюеров запущен. Sur la carte DOCS-SORA,
garantie, pour que vous puissiez installer des articles d'art, des instructions
по безопасности и ясным canal обратной связи.

- **Auditeur :** курированный список членов сообщества, партнеров и мейнтейнеров, которые
  подписали политику acceptable-use для aperçu.
- **Ограничения:** размер волны по умолчанию <= 25 ревьюеров, окно доступа 14 дней, реакция на
  инциденты в течение 24h.

## Чеклист gate перед запуском

Vérifiez cela avant d'ouvrir la session :

1. Après avoir téléchargé les éléments d'aperçu dans CI (`docs-portal-preview`,
   manifeste de somme de contrôle, descripteur, bundle SoraFS).
2. `npm run --prefix docs/portal serve` (porte de contrôle) contrôle cette balise.
3. Les billets d'avion pour les voyages et les voyages avec votre voyage.
4. Documents sur la sécurité, l'observabilité et la preuve des incidents
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Ajouter un formulaire de commentaires ou un modèle de problème (pour la gravité, la situation,
   (écrans et informations sur l'achat).
6. Texte rédigé par Docs/DevRel + Governance.

## Пакет приглашения

Chaque fois que vous effectuez une opération, vous devez effectuer :1. **Artéfacts testés** — Recherchez le manifeste/plan SoraFS ou l'artefact GitHub,
   Il s'agit d'un manifeste de somme de contrôle et d'un descripteur. Il vous suffit de demander la vérification des commandes
   Les critiques peuvent le faire avant de le dire.
2. **Инструкции serve** — Cliquez sur la commande d'aperçu de la porte de contrôle :

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Напоминания по безопасности** — Укажите, что токены истекают автоматически, ссылки нельзя
   делиться, а инциденты нужно сообщать немедленно.
4. **Канал обратной связи** — Choisissez le modèle/forme de problème et choisissez la réponse à votre demande.
5. **Programmes de programme** — Sélectionnez les dates de votre choix, les heures de bureau ou la synchronisation des heures et effectuez éventuellement une actualisation.

Exemple de chatte
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
покрывает эти требования. Afficher les espaces réservés (données, URL, contacts)
перед отправкой.

## Hôte de l'aperçu de la publication

Продвигайте Preview Host только после завершения онбординга и утверждения changement ticket.
См. [руководство по exhibition preview host](./preview-host-exposure.md) pour les parties de bout en bout
construire/publier/vérifier, utilisé dans cette période.

1. **Construire et ajouter :** Ajoutez la balise de version et sauvegardez les objets déterminés.

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
   ```

   La broche de script indique `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   et `portal.dns-cutover.json` à `artifacts/sorafs/`. Приложите эти файлы к волне
   приглашений, чтобы каждый ревьюер мог проверить те же биты.2. **Alias d'aperçu de la publication :** Envoyer la commande sans `--skip-submit`
   (utilisez `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]` et recherchez un alias de gouvernance).
   Le script fournit le manifeste `docs-preview.sora` et vous
   `portal.manifest.submit.summary.json` et `portal.pin.report.json` pour le lot de preuves.

3. **Déploiement :** Indiquez quel alias est défini et la somme de contrôle correspond à la balise
   avant l'ouverture du service.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Держите `npm run serve` (`scripts/serve-verified-preview.mjs`) под рукой как repli,
   Vous pouvez télécharger une copie locale, si vous avez un aperçu du bord à votre place.

## Таймлайн коммуникаций

| Jour | Réception | Propriétaire |
| --- | --- | --- |
| J-3 | Finaliser la configuration du texte, obtenir des articles d'art, vérifier à sec | Docs/DevRel |
| J-2 | Approbation de la gouvernance + ticket de modification | Docs/DevRel + Gouvernance |
| J-1 | Ouvrir la connexion à votre compte, activer le tracker pour les professionnels | Docs/DevRel |
| D | Appel de lancement / heures de bureau, surveillance télémétrique | Docs/DevRel + Sur appel |
| J+7 | Résumé des commentaires préliminaires, triage des problèmes bloqués | Docs/DevRel |
| J+14 | Fermez votre ordinateur, ouvrez le téléchargement actuel et publiez le résumé dans `status.md` | Docs/DevRel |

## Трекинг доступа и телеметрия1. Sélectionnez le lieu, l'horodatage et les données affichées dans l'enregistreur de commentaires d'aperçu
   (см. [`preview-feedback-log`](./preview-feedback-log)), que каждая волна разделяла
   один и тот же preuve trail:

   ```bash
   # Добавить новое событие приглашения в artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Sociétés concernées : `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, et `access-revoked`. Лог по умолчанию находится в
   `artifacts/docs_portal_preview/feedback_log.json` ; приложите его к тикету волны
   вместе с формами согласия. Utilisez le résumé de l'assistant pour ajouter du contenu audio
   roll-up avant la fin finale :

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   Résumé JSON s'applique aux utilisateurs, aux utilisateurs ouverts et aux commentaires détaillés
   et l'horodatage après l'enregistrement. Helper основан на
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Ainsi, ce flux de travail peut être exécuté localement ou dans CI. Utiliser le résumé de Hablon dans
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   при публикации récapitulation волны.
2. Installez les tableaux de bord télémétriques `DOCS_RELEASE_TAG`, utilisés pour les moteurs, les boutons
   Vous pouvez vous associer à une cohorte de personnes.
3. Installez `npm run probe:portal -- --expect-release=<tag>` après le déploiement pour le mettre à jour.
   cet aperçu sera disponible pour les métadonnées de version correctes.
4. Assurez-vous que les criminels se connectent au runbook de votre ordinateur et s'associent à la cohorte.

## Commentaires et commentaires1. Rédigez les commentaires dans le document actuel ou dans le forum de discussion. Marquez les éléments `docs-preview/<wave>`,
   La feuille de route des propriétaires est la meilleure pour tous.
2. Utilisez le résumé de l'enregistreur d'aperçu pour afficher le contenu, afin de définir le contenu dans
   `status.md` (éléments, clés, plans de travail) et consulter `roadmap.md`,
   если jalon DOCS-SORA изменился.
3. Commencez votre offboarding depuis
   [`reviewer-onboarding`](./reviewer-onboarding.md) : terminer le téléchargement, archiver les fichiers et
   поблагодарите участников.
4. Ajoutez des objets, des objets d'art visibles, des portes de contrôle de contrôle et
   Il est impératif de configurer les nouvelles données avec de nouvelles données.

Après l'introduction de ce programme, le programme est prévisualisé et auditable.
Docs/DevRel est actuellement en mesure de développer la configuration sur le seul portail de GA.