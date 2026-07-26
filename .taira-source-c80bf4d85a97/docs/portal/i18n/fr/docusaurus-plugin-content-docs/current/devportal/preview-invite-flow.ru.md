---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Aperçu de l'installation du pot

## Назначение

Les cartes **DOCS-SORA** permettent d'obtenir des rapports d'intégration et un programme de prévisualisation publique en cas de blocage avant votre port. c'est la version bêta. Cet article explique que pour ouvrir la page de votre installation, les objets d'art doivent être ouverts avant de commencer à fonctionner. доказать, что поток аудируем. Utilisez-le avec :

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) pour les robots avec chaque révision.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) pour la somme de contrôle de garantie.
- [`devportal/observability`](./observability.md) pour l'exportation de téléphones et de crochets.

## Plan vol| Volna | Auditoire | Critères de sélection | Critères à considérer | Première |
| --- | --- | --- | --- | --- |
| **W0 - Mainteneurs principaux** | Mainteneurs Docs/SDK, validant le contenu pour vous. | La commande GitHub `docs-portal-preview` a été installée, la somme de contrôle de la porte dans `npm run serve` a été lancée, Alertmanager a eu lieu le 7 juin. | Все P0 доки просмотрены, backlog промаркирован, bloкирующих инцидентов нет. | Используется для проверки потока; email-инвайтов нет, только обмен aperçu des articles. |
| **W1 - Partenaires** | Les opérateurs SoraFS, les intégrateurs Torii, examinent la gouvernance selon NDA. | W0 s'occupe de l'utilisation de votre proxy, Try-it proxy dans la mise en scène. | Les partenaires de signature (problème ou formulaire de demande), le télémètre envoie =2 les documents ont été publiés pour voir le pipeline d'aperçu sans restauration. | Organisez les programmations régulières (<=25) et battez-les chaque fois. |

Documentez le volume d'activité dans `status.md` et dans le suivi des demandes d'aperçu, ce qui indique l'état de la gouvernance selon le niveau d'accès.

## Contrôle de contrôle en amont

Veuillez planifier ce projet **avant** la planification des vols :1. **Артефакты CI доступны**
   - Suivant `docs-portal-preview` + descripteur ajouté `.github/workflows/docs-portal-preview.yml`.
   - La broche SoraFS est ouverte sur `docs/portal/docs/devportal/deploy-guide.md` (le basculement du descripteur est pris en compte).
2. **Somme de contrôle totale**
   - `docs/portal/scripts/serve-verified-preview.mjs` s'applique à `npm run serve`.
   - Instructions `scripts/preview_verify.sh` pour macOS + Linux.
3. **Базовая телеметрия**
   - `dashboards/grafana/docs_portal.json` vous indique le trafic Essayez-le et alertez `docs.preview.integrity`.
   - L'application ultérieure du `docs/portal/docs/devportal/observability.md` s'applique désormais aux Grafana.
4. **Gouvernance des articles**
   - Émettez le suivi des invitations готов (одна issue на волну).
   - Шаблон реестра ревьюеров скопирован (см. [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Юридические и SRE approbations приложены к issue.

Assurez-vous de vérifier en amont dans le système de suivi des invitations avant l'ouverture du processus.

## Shagi потока1. **Выбор кандидатов**
   - Consultez la feuille de calcul de liste d'attente ou la file d'attente des partenaires.
   - Indiquez quel modèle de demande votre candidat a proposé.
2. **Prise en charge**
   - Назначить approbateur на issue invitation tracker.
   - Проверить требования (CLA/contract, utilisation acceptable, briefing de sécurité).
3. **Отправка приглашений**
   - Installez les téléphones dans [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contacts).
   - Ajoutez le descripteur + l'archive de hachage, essayez l'URL de mise en scène et les canaux disponibles.
   - Сохранить финальное письмо (ou la transcription Matrix/Slack) dans le numéro.
4. **Démarrage de l'intégration**
   - Activez le suivi des invitations pour `invite_sent_at`, `expected_exit_at` et le statut (`pending`, `active`, `complete`, `revoked`).
   - Привязать demande d'admission ревьюера для аудитабельности.
5. **Surveillance télémétrique**
   - Sélectionnez `docs.preview.session_active` et les alertes `TryItProxyErrors`.
   - Prévenez l'incident lors de l'ouverture du téléphone par rapport à la ligne de base et affichez le résultat lors de la configuration.
6. **Сбор фидбека и выход**
   - Fermez l'installation après la mise en service ou la mise en service `expected_exit_at`.
   - Обновить issue волны кратким резюме (находки, инциденты, следующие шаги) перед переходом к следующей когорте.

## Preuves et отчетность| Artefact | Где хранить | Частота обновления |
| --- | --- | --- |
| Suivi des invitations à émettre | Projet GitHub `docs-portal-preview` | Обновлять после каждого приглашения. |
| Liste des exportations | Registre suisse pour `docs/portal/docs/devportal/reviewer-onboarding.md` | Еженедельно. |
| Films télémétriques | `docs/source/sdk/android/readiness/dashboards/<date>/` (ensemble de télémétrie) | Dans chaque cas, + après les incidents. |
| Résumé des commentaires | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (paquet de papier pour ordinateur portable) | Après 5 jours de technologie, vous serez en mesure de le faire. |
| Note de réunion sur la gouvernance | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Заполнять до каждого DOCS-SORA synchronisation de gouvernance. |

Запускайте `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
Après chaque lot, vous devez préparer un résumé machinique. En utilisant le rendu JSON pour les problèmes, les rapports sur la gouvernance peuvent améliorer la configuration de votre logo.

Veuillez utiliser les preuves secrètes `status.md` pour obtenir des informations sur la carte que vous pouvez utiliser.

## Critères de restauration et pauses

Il y a un projet de loi (et une gouvernance avancée), si vous envisagez ce sujet comme suit :

- Инцидент Essayez-le proxy, потребовавший rollback (`npm run manage:tryit-proxy`).
- Installation des alertes : >3 pages d'alerte pour les points de terminaison en aperçu uniquement dans les 7 jours.
- Le complexe de recherche : la possibilité d'utiliser un modèle de demande sans utiliser l'utilisateur concerné ou sans télécharger un modèle de demande.
- Risque de sécurité : inadéquation de la somme de contrôle, обнаруженный `scripts/preview_verify.sh`.Vous devez ensuite documenter la correction dans le système de suivi des invitations et assurer la stabilité des télémètres pendant un minimum de 48 heures.