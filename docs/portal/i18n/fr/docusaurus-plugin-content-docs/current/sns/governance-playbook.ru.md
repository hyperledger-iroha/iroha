---
lang: fr
direction: ltr
source: docs/portal/docs/sns/governance-playbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Cette page indique `docs/source/sns/governance_playbook.md` et prend en charge
служит канонической копией портала. Il s'agit d'un rendez-vous pour les relations publiques.
:::

# Mise en service du service de noms Sora (SN-6)

**État :** Publié le 2026-03-24 — Cet article est destiné aux appareils SN-1/SN-6  
**Feuille de route :** SN-6 "Conformité et résolution des litiges", SN-7 "Résolveur et synchronisation de passerelle", adresse politique ADDR-1/ADDR-5  
**Propriétés :** Liste de schéma dans [`registry-schema.md`](./registry-schema.md), API du registraire de contrat dans [`registrar-api.md`](./registrar-api.md), adresses UX dans [`address-display-guidelines.md`](./address-display-guidelines.md), et avez défini la structure des comptes dans [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Cet article explique pourquoi l'organisation met en œuvre le service de noms Sora (SNS)
chartes, утверждают регистрации, эскалируют споры и подтверждают синхронность
состояний résolveur et passerelle. Sur la feuille de route de travail pour cette CLI
`sns governance ...`, les manifestes Norito et les appareils audio utilisés
операторский источник до N1 (публичного запуска).

## 1. Exploitation et écoute

Document fourni pour :- Le Conseil de gouvernance de Clenov, les dirigeants de la Charte, les conseillers politiques et
  результатам споров.
- Panneau de gardien de clé, qui contient des éléments d'extension et de protection
  откаты.
- Steward по суффиксам, ведущих очереди registrar, утверждающих аукционы и
  управляющих распределением дохода.
- Les opérateurs résolveur/passerelle, disponibles pour la diffusion de SoraDNS, mise à jour
  GAR и телеметрические garde-corps.
- Le groupe de commandement, les gestionnaires et les sous-traitants, qui doivent recevoir ce document
  каждое действие управления оставило аудируемые артефACTы Norito.

Dans le mode de fermeture des piles (N0), de mise hors tension publique (N1) et de déconnexion (N2),
Перечисленные в `roadmap.md`, связывая каждый workflow с необходимыми доказательствами,
дашбORDами и путями эскалации.

## 2. Contacts rôles et cartes| Rôle | Основные обязанности | Objets d'art et de télévision modernes | Escalade |
|------|----------------------|------------------------|---------------|
| Conseil de gouvernance | Il s'agit d'une charte politique et d'un intendant politique. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, les bulletins d'information sont disponibles auprès de `sns governance charter submit`. | Председатель совета + трекер повестки управления. |
| Conseil des gardiens | Выпускает soft/hard заморозки, экстренные каноны и 72 h обзоры. | Les tickets de tuteur, correspondants à `sns governance freeze`, remplacent les commandes dans `artifacts/sns/guardian/*`. | Gardien de garde de garde (<=15 min ACK). |
| Intendants de suffixe | Il s'agit du registraire, des ventes aux enchères, des ventes aux enchères et des communications avec les clients ; подтверждают комплаенс. | L'intendant politique dans `SuffixPolicyV1`, listes de remerciements, l'intendant est dû au mémo réglementaire. | Le programme Lid steward + PagerDuty est suffisant. |
| Opérations de registraire et de facturation | Utilisez le connecteur `/v1/sns/*`, les plaques de service, la télévision et les images CLI. | API du registraire ([`registrar-api.md`](./registrar-api.md)), mesures `sns_registrar_status_total`, plaque de documentation dans `artifacts/sns/payments/*`. | Gestionnaire de service registraire et agent de liaison. || Opérateurs de résolution et de passerelle | Prend en charge SoraDNS, GAR et la passerelle d'hébergement en synchronisation avec le registraire; стримят метрики прозрачности. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Résolveur SRE d'astreinte + passerelle opérationnelle. |
| Trésorerie et Finances | Il s'agit principalement de 70/30, d'exclusions de référence, d'ouvertures de droits/de demandes et d'attestations SLA. | Les manifestes de la société Stripe/Cartes, utilisant des KPI complets dans `docs/source/sns/regulatory/`. | Contrôleur financier + opérateur complet. |
| Conformité et liaison réglementaire | L'organisation mondiale (EU DSA et dr.) met à jour les engagements KPI et permet de les conclure. | Réglez le mémo sur `docs/source/sns/regulatory/`, les platines de référence, les instructions `ops/drill-log.md` sur les répétitions de table. | Le programme du couvercle est complet. |
| Support / SRE d'astreinte | Il s'agit de résoudre les incidents (collision, facturation directe, résolveur de tâches), de coordonner la gestion des clients et le runbook. | Шаблоны инцидентов, `ops/drill-log.md`, laboratoire de formation, transcriptions Slack/war-room dans `incident/`. | Ротация SNS de garde + SRE менеджмент. |

## 3. Objets d'art canoniques et données historiques| Artefact | Délais | Назначение |
|--------------|-------------|---------------|
| Charte + KPI | `docs/source/sns/governance_addenda/` | Nous proposons une charte avec la version de contrôle, les clauses KPI et la mise en œuvre des résolutions, selon la CLI-Gолоса. |
| Schéma de restauration | [`registry-schema.md`](./registry-schema.md) | Structures canoniques Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Registre des contrats | [`registrar-api.md`](./registrar-api.md) | Charges utiles REST/gRPC, mesures `sns_registrar_status_total` et hook de gouvernance. |
| Adresses UX | [`address-display-guidelines.md`](./address-display-guidelines.md) | Les canons i105 (avant) et les véhicules (vers l'extérieur) utilisent des cadres/appareils électriques. |
| Documents SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Déterminez l'hôte, les robots de transparence et les alertes. |
| Mémo de régulation | `docs/source/sns/regulatory/` | Заметки приема по юрисдикциям (par exemple, EU DSA), responsable des remerciements, шаблонные приложения. |
| Journal de forage | `ops/drill-log.md` | Les réponses au problème et à la résolution IR ont été effectuées avant votre arrivée. |
| Production d'œuvres d'art | `artifacts/sns/` | Le document de travail, le tuteur de tickets, le résolveur de diff, les KPI et la CLI sont issus de `sns governance ...`. |

Vous devez créer un minimum d'objets pour l'art d'Odin dans les tableaux
Oui, les auditeurs peuvent prendre une décision technique pendant 24 heures.## 4. Les achats en ligne de vélo

### 4.1 Charte et steward-mois

| Шаг | Владелец | CLI / Doc. | Première |
|-----|----------|----------------------|---------------|
| Description détaillée et KPI delta | Докладчик совета + лидер steward | Tableau Markdown dans `docs/source/sns/governance_addenda/YY/` | Cliquez sur le contrat ID KPI, les crochets télémétriques et les activités d'utilisation. |
| Подача предложения | La soviet précédente | `sns governance charter submit --input SN-CH-YYYY-NN.md` (pour `CharterMotionV1`) | CLI envoie le manifeste Norito à `artifacts/sns/governance/<id>/charter_motion.json`. |
| Голосование и tuteur reconnaissance | Совет + tuteurs | `sns governance ballot cast --proposal <id>` et `sns governance guardian-ack --proposal <id>` | Utilisez les protocoles et les documents pertinents. |
| Intendant principal | Responsable du programme | `sns governance steward-ack --proposal <id> --signature <file>` | Требуется до смены политик суффикса; сохранить конверт в `artifacts/sns/governance/<id>/steward_ack.json`. |
| Activation | Opérations du registraire | Ouvrez `SuffixPolicyV1`, connectez-vous au registraire de votre téléphone, ouvrez le compte dans `status.md`. | Cette activation s'effectue dans `sns_governance_activation_total`. |
| Journal d'audit | Compléments | Placez-le dans le `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` et dans le journal de forage, ou utilisez-le sur une table. | Créez des liens avec les pays télémétriques et les différences de politique. |

### 4.2 Enregistrement, vente aux enchères et prix1. **Preflight :** le registraire indique `SuffixPolicyV1`, pour mettre à jour le scénario.
   уровень, доступные сроки и окна grâce/rédemption. Поддерживайте ценовые listes
   Synchronisation avec les heures du tableau 3/4/5/6-9/10+ (années inférieures +
   коэффициенты суффикса), описанной в feuille de route.
2. **Enchères sous pli cacheté :** Pour les achats premium, vous recevrez un engagement de 72 h / 24 h
   révéler через `sns governance auction commit` / `... reveal`. Опубликуйте список
   commit (только хеши) dans `artifacts/sns/auctions/<name>/commit.json`, ici
   Les auditeurs peuvent prouver leur efficacité.
3. **Plaque de commande :** Le registraire valide `PaymentProofV1` pour la livraison des dossiers
   казначейства (70 % de trésorerie / 30 % d'intendant avec exclusion de référence <=10 %). Сохраните
   Norito JSON dans `artifacts/sns/payments/<tx>.json` et vous permettre de répondre
   registraire (`RevenueAccrualEventV1`).
4. **Crochet de gouvernance :** Ajoutez `GovernanceHookV1` pour les locaux premium/gardés
   ссылкой на IDs предложений совета и подписи steward. Отсутствие hook приводит к
   `sns_err_governance_missing`.
5. **Activation + résolveur de synchronisation :** Après que Torii ouvre le serveur,
   installez le résolveur de queue de transparence, pour améliorer la mise à jour du nouveau
   состояния GAR/zone (см. 4.5).
6. **Taille client :** Ouvrir le grand livre client (portefeuille/explorateur) ici
   общие luminaires в [`address-display-guidelines.md`](./address-display-guidelines.md),
   Il s'agit également de l'i105 et de la suppression de la copie/QR.### 4.3 Procédures, facturation et gestion des factures- **Produits du workflow :** le registraire prévoit un délai de grâce de 30 jours + un rachat bien sûr
  60 jours, acheté dans `SuffixPolicyV1`. Через 60 дней автоматически запускается
  L'Allemagne va rouvrir après (7 jours, commission 10x avec une réduction de 15%/jour)
  ici `sns governance reopen`.
- **Détails des documents :** Possibilité de procéder ou de transférer des données
  `RevenueAccrualEventV1`. Les panneaux de revêtement de sol (CSV/Parquet) sont disponibles
  сопоставляться с этими событиями ежедневно; прикладывайте доказательства в
  `artifacts/sns/treasury/<date>.json`.
- **Exclusions de référence :** La référence de référence n'est pas disponible
  суффиксу через `referral_share` в политике steward. registraire публикуют итоговое
  распределение и хранят référence manifestes рядом с доказательством оплаты.
- ** Points d'intérêt : ** Les finances publient des KPI spécifiques
  (enregistrement, livraison, ARPU, utilisation des liens/obligations) dans
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Les vols à bord doivent être effectués sur
  Vous avez consulté les tableaux qui correspondent au Grafana fourni avec le grand livre des documents.
- **Ежемесячный KPI обзор :** Чекпоинт первого вторника объединяет финансового лида,
  steward en service и programme PM. Ouvrir [Tableau de bord SNS KPI](./kpi-dashboard.md)
  (intégration intégrée `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  экспортируйте таблицы débit + revenu registraire, зафиксируйте дельты в
  приложении и приложите артефACTы к мемо. Запускайте инцидент при обнаруженииSLA нарушений (окна gel >72 h, всплески ошибок registrar, дрейф ARPU).

### 4.4 Paramètres, sports et appels

| Faza | Владелец | Projets et documentation | ANS |
|------|----------|----------------------------|-----|
| Après le gel doux | Steward / поддержка | Achetez le billet `SNS-DF-<id>` sur la plate-forme de documentation, en contact avec la société obligataire et les sélectionneurs. | <=4 heures après la mise en service. |
| Billet de gardien | Conseil des gardiens | `sns governance freeze --selector <i105> --reason <text> --until <ts>` correspond à `GuardianFreezeTicketV1`. Téléchargez JSON dans `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h выполнение. |
| Ratification de la résolution | Conseil de gouvernance | En changeant ou en ouvrant les portes, j'ai décidé de résoudre le problème du billet de tuteur et de digérer la relation obligataire. | Следующее заседание совета или асинхронное голосование. |
| Panneau d'arbitrage | Комплаенс + steward | Installez le panneau de 7 étapes (feuille de route correspondante) avec les bulletins suivants `sns governance dispute ballot`. Приложить анонимные квитанции голосов к пакету инцидента. | Вердикт <=7 дней после внесения caution. |
| Appellation | Gardien + совет | Les appels à la création de liens et au processus de prise de pouvoir; записать manifeste Norito `DisputeAppealV1` и сослаться на первичный тикет. | <=10 jours. |
| Réparation et réparation | Opérations de registraire + résolveur | Sélectionnez `sns governance unfreeze --selector <i105> --ticket <id>`, ouvrez le statut du registraire et diffusez le diff GAR/résolveur. | Сразу после вердикта. |Экстренные каноны (заморозки, инициированные gardien <=72 h) следуют тому же
Cependant, vous ne pouvez pas vous préoccuper rétrospectivement de la situation et des mesures de sécurité dans
`docs/source/sns/regulatory/`.

### 4.5 Installation du résolveur et de la passerelle

1. **Event hook :** каждое событие реестра отправляется в поток событий résolveur
   (`tools/soradns-resolver` SSE). Les opérations de résolution permettent de résoudre et de supprimer les différences ici
   queue de transparence (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Обновление GAR шаблона:** gateways должны обновить GAR шаблоны, на которые
   Ссылается `canonical_gateway_suffix()`, et переподписать список `host_pattern`.
   Сохранить diff dans `artifacts/sns/gar/<date>.patch`.
3. **Ajouter le fichier de zone :** Utiliser le fichier de zone squelette, décrit dans `roadmap.md`
   (nom, ttl, cid, preuve), et ouvrez-le dans Torii/SoraFS. Архивируйте Norito JSON
   dans `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Programmes de démarrage :** Sélectionnez `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   чтобы убедиться, что алерты зеленые. Приложите текстовый вывод Prometheus к
   Il est généralement possible d'obtenir des informations sur les problèmes.
5. **Gateway gateway :** Téléchargez les fichiers `Sora-*` (politique de cache, CSP, résumé GAR)
   et utilisez-les pour l'exploitation du site, que les opérateurs peuvent télécharger, cette passerelle
   обслужил новое имя с нужными garde-corps.

## 5. Télémétrie et détection| Signal | Источник | Description / Conception |
|--------|----------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Обработчики registraire Torii | Documents d'enregistrement/d'enregistrement, de livraison, de transfert, de transfert ; L'alerte sur la liste `result="error"` est disponible. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Mesures Torii | SLO по латентности для API обработчиков; utilisé dans le pays depuis `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` et `soradns_bundle_cid_drift_total` | Résolveur de transparence | Vous avez un médecin ou un GAR ; garde-corps определены в `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | Gouvernance CLI | Счетчик, увеличивающийся при активации чартеров/приложений; используется для сверки решений совета и опубликованных addenda. |
| Jauge `guardian_freeze_active` | CLI gardien | Appuyez sur la congélation douce/dure avec le sélecteur ; Si vous utilisez SRE, si vous indiquez `1`, vous devez utiliser le SLA. |
| KPI pour les pays | Finances / Documentation | Le rollup général est publié avec le mémo réglementaire ; Le portail fournit le [Tableau de bord SNS KPI] (./kpi-dashboard.md), les stewards et les régulateurs ont des vidéos sur Grafana. |

## 6. Recherche de documents et d'audit| Réception | Доказательства для архива | Хранилище |
|--------------|----------------------------|---------------|
| Révision de la Charte / Politique | Manifeste Norito, transcription CLI, différence KPI, accusé de réception de gestion. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Inscription/Prodution | Charge utile `RegisterNameRequestV1`, `RevenueAccrualEventV1`, plaque de documentation. | `artifacts/sns/payments/<tx>.json`, API du registraire de logiciels. |
| Vente aux enchères | Valider/révéler les manifestes, les graines случайности, таблица расчета победителя. | `artifacts/sns/auctions/<name>/`. |
| Заморозка / разморозка | Guardian ticket, votre société de gestion, l'URL du journal des incidents, les liens de communication avec les clients. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Распространение résolveur | Zonefile/GAR diff, JSONL tailer, capture d'écran Prometheus. | `artifacts/sns/resolver/<date>/` + отчеты о прозрачности. |
| Apport régulé | Mémo d'admission, трекер дедлайнов, accusé de réception steward, сводка KPI изменений. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Le chef de file des voyages| Faza | Critères à considérer | Paquets de documentation |
|------|------------------|--------------------|
| N0 — Fermeture bêta | Схема реестра SN-1/SN-2, registraire simple CLI, завершенный Guardian Drill. | Charte motion + steward ACK, registraire des journaux d'exécution à sec, recherche du résolveur de projets, enregistré dans `ops/drill-log.md`. |
| N1 — Poste public | Ventes + services de réparation pour `.sora`/`.nexus`, registraire en libre-service, résolveur de synchronisation automatique, facturation aux frontières. | Listes de différences, résultats du registraire CI, applications de plates-formes/KPI, suivi de la transparence, informations sur la répétition des incidents. |
| N2 — Résolution | `.dao`, API revendeur, portail споров, cartes de pointage steward, аналитические дашbordы. | Скриншоты портала, SLA метрики споров, выгрузки steward scorecards, обновленный чартер управления с политиками revendeur. |

Выход из фаз требует записанных perceuses de table (счастLIвый путь регистрации,
заморозка, résolveur de panne) avec les articles dans `ops/drill-log.md`.

## 8. Localisation des incidents et des épidémies| Déclencheur | Уровень | Немедленный владелец | Conception d'objets |
|---------|---------|------------|-----------------------|
| Trouver un résolveur/GAR ou un document de recherche | 1 septembre | Résolveur SRE + carte gardienne | Пейджить résolveur de garde, собрать вывод tailer, решить вопрос о заморозке затронутых имен, публиковать status каждые 30 min. |
| Différents bureaux d'enregistrement, de facturation ou de gros enregistrements API | 1 septembre | Responsable de service du registraire | Ouvrez de nouvelles enchères, en cliquant sur la CLI principale, et activez les stewards/commandes, en utilisant le journal Torii pour l'incident. |
| Sports pour les hommes, les plates-formes de négociation ou les demandes de clients | 2 septembre | Intendant + support principal | Собрать доказательства платежа, определить необходимость soft freeze, ответить заявителю в SLA, записать результат в трекере спора. |
| Équipements audio | 2 septembre | Agent de conformité | Si vous souhaitez planifier des mesures correctives, consultez la note `docs/source/sns/regulatory/` et planifiez la session suivante. |
| Exercice ou répétition | 3 septembre | Programme PM | Sélectionnez le scénario `ops/drill-log.md`, archivez les objets, et examinez les résultats de la feuille de route. |

Vous devez supprimer `incident/YYYY-MM-DD-sns-<slug>.md` des tableaux
владения, журналами команд et ссылками на доказательства, собранные по этому
плейбуку.

## 9. Les filles- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (options SNS, DG, ADDR)

Cliquez ici pour lire le texte actuel de la carte, CLI поверхностей
ou des contrats télémétriques ; feuille de route des éléments, ссылающиеся на
`docs/source/sns/governance_playbook.md`, maintenant vous êtes prêt à le faire
последней редакции.