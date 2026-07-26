---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Онбординг ревьюеров aperçu

## Обзор

DOCS-SORA ouvre le portail des robots de l'opérateur. Сборки с gate по checksum
(`npm run serve`) et photos utiles Essayez-le en ouvrant l'étape suivante : envois confirmés
ревьюеров до широкого открытия aperçu public. C'est ce que je dis, pour comprendre les choses,
vérifiez la qualité, effectuez le téléchargement et assurez-vous de le faire. См.
[Aperçu du flux d'invitation](./preview-invite-flow.md) pour la planification du processus, les délais d'exécution
et les télécommunications d'exportation; Vous n'avez pas besoin de vous concentrer sur les projets après avoir lu le rapport.

- **Dans les formats :** les critiques qui doivent être téléchargées dans les documents d'aperçu (`docs-preview.sora`,
  (pages GitHub ou groupes SoraFS) pour GA.
- **Вне рамок:** opérateurs Torii ou SoraFS (pour les systèmes d'intégration) et
  продакшн-развертывания портала (см.
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Rôles et tracas| Rôle | Sels typiques | Objets d'art à collectionner | Première |
| --- | --- | --- | --- |
| Responsable du noyau | Prouvez de nouveaux gars, effectuez des tests de fumée. | Poignée GitHub, contactez Matrix, en utilisant CLA. | Vous pouvez également utiliser la commande GitHub `docs-preview` ; все равно подайте заявку, чтобы доступ был аудируем. |
| Réviseur partenaire | Vérifiez les captures d'écran ou le contenu du SDK pour les mettre à jour par la publication. | Courriel professionnel, POC légal, conditions d'aperçu avancées. | Cela devrait permettre de travailler sur des appareils télémétriques et des données sur les robots. |
| Bénévole communautaire | Ces commentaires sont relatifs à l'utilisation des joueurs. | Poignée GitHub, contact précédent, actuellement en cours, connecté à CoC. | Держите когорты небольшими; приоритет ревьюерам, подписавшим accord de contribution. |

Voici les types de commentaires pour ceux-ci :

1. Подтвердить политику допустимого использования aperçu-artefactows.
2. Procéder à la sécurité/observabilité
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Veuillez installer `docs/portal/scripts/preview_verify.sh` avant cela, comme
   обслуживать любой локальный instantané.

## Apport du processus1. Попросите заявителя заполнить
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   форму (или вставить ее в issue). Зафиксируйте как miniмум: личность, способ связи,
   GitHub handle, planifie la publication des données et la mise à jour des fichiers de sécurité.
2. Téléchargez votre compte dans le navigateur `docs-preview` (problème GitHub ou mise à jour du ticket)
   et désignez l'approbateur.
3. Vérifiez le travail :
   - CLA / соглашение контрибьютора в наличии (ou contrat de partenariat).
   - Подтверждение допустимого использования хранится в заявке.
   - Риск-оценка завершена (например, партнерские ревьюеры одобрены Legal).
4. L'approbateur prend en charge la recherche et le suivi du problème avec l'utilisateur.
   gestion du changement (par exemple : `DOCS-SORA-Preview-####`).

## Provisioning et instruments

1. **Prendre l'artéfact** — Prévisualiser le descripteur d'aperçu + l'archive depuis
   Flux de travail CI ou broche SoraFS (artefact `docs-portal-preview`). Nom du commentaire :

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Servir avec la somme de contrôle d'application** — Укажите команду с gate по checksum :

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Ceci utilise `scripts/serve-verified-preview.mjs`, ce qui ne permet pas de build
   не запускался случайно.

3. ** Téléchargez sur GitHub (officiellement) ** — Si vous n'êtes pas public, ajoutez-le
   commentaires sur la commande GitHub `docs-preview` pendant la période de publication et de configuration des tâches
   в заявке.4. **Коммуникация каналов поддержки** — Prend en charge le contact de garde (Matrix/Slack) et
   la procédure contre les incidents est [`incident-runbooks`](./incident-runbooks.md).

5. **Телеметрия + feedback** — Напомните, что собирается анонимизированная ANALYTIQUE
   (см. [`observability`](./observability.md)). Donnez votre avis ou votre problème,
   указанный в приглашении, и зафиксируйте событие через helper
   [`preview-feedback-log`](./preview-feedback-log), ce qui est actuellement le cas.

## Чеклист ревьюера

Avant de télécharger l'aperçu, les commentaires doivent être affichés :

1. Vérifiez les objets d'art recherchés (`preview_verify.sh`).
2. Ouvrez le portail à partir de `npm run serve` (ou `serve:verified`), pour que la somme de contrôle de garde soit activée.
3. Vérifiez vos paramètres de sécurité et d'observabilité.
4. Vérifiez OAuth/Try it avec la connexion par code de l'appareil (par exemple) et ne l'utilisez pas.
   production токены.
5. Fixez les documents dans votre voyage (problème, document ou formulaire) et modifiez-les
   их релизным тегом aperçu.

## Ответственности мейнтейнеров и offboarding| Faza | Fête |
| --- | --- |
| Coup d'envoi | Pour que le système d'admission soit utilisé pour la commande, vous pouvez ajouter des articles d'art + des instructions, puis ajouter `invite-sent` ici. [`preview-feedback-log`](./preview-feedback-log), et planifiez la synchronisation anticipée, si vous ne récupérez plus rien. |
| Surveillance | Ouvrir l'aperçu télémétrique (pas de trafic Essayez-le, mais sondez) et effectuez l'analyse des incidents en fonction des résultats. Enregistrez l'appareil `feedback-submitted`/`issue-opened` à la simple fin de l'opération, les mesures correspondant à vos besoins. |
| Débarquement | Installez le nouveau site GitHub ou SoraFS, puis `access-revoked`, archiver la page (en incluant le résumé des commentaires + l'ouverture du projet), et обновить реестр ревьюеров. Попросить ревьюера удалить local сборки и приложить digest из [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Utilisez ce processus pour faire tourner les moteurs de vos vols. Сохранение следов в репозитории
(issue + шаблоны) помогает DOCS-SORA оставаться аудируемым и позволяет gouvernance подтвердить,
что доступ к aperçu следовал документированным контролям.

## Les voyages et les randonnées- Начинайте каждое обращение с файла
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  Dans le fichier mini-fichier, les instructions pour l'aperçu et l'affichage de la somme de contrôle,
  Ce sont des critiques politiques qui utilisent efficacement.
- Lors de la restauration de la table, placez les appareils `<preview_tag>`, `<request_ticket>` et les canaux correspondants.
  Examinez la copie finale de la demande d'admission, celle des réviseurs, des approbateurs et des auditeurs
  могли сослаться на точный текст.
- Après l'installation d'une feuille de calcul de suivi ou d'un problème avec l'horodatage `invite_sent_at`
  et je pense que c'est ce qu'il y a de mieux
  [aperçu du flux d'invitation](./preview-invite-flow.md) автоматически подхватил когорту.