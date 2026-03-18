---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu du lien
titre : Обзор Sora Nexus
description : Votre architecte d'intérieur Iroha 3 (Sora Nexus) est en contact avec les documents canoniques.
---

Nexus (Iroha 3) permet d'utiliser Iroha 2 pour une utilisation multivoie, permettant d'améliorer la gouvernance et les exigences outils pour le SDK. Cette page présente le nouvel observateur `docs/source/nexus_overview.md` dans les monorepos, qui sont sur le portail du plus grand architecte d'éléments. сочетаются.

## Lignes de lecture

- **Iroha 2** - самостоятельные развертывания для консорциумов или частных сетей.
- **Iroha 3 / Sora Nexus** - Il s'agit d'un ensemble multivoie public, les opérateurs s'enregistrent pour les clients (DS) et peuvent consulter instruments de gouvernance, de règlement et d'observabilité.
- Les lignes qui composent votre propre espace de travail (IVM + chaîne d'outils Kotodama), l'utilisation du SDK, la mise à jour d'ABI et les fictions Norito est à l'origine de problèmes. Les opérateurs recherchent le paquet `iroha3-<version>-<os>.tar.zst`, qui utilise Nexus ; Vérifiez le `docs/source/sora_nexus_operator_onboarding.md` sur la liste de contrôle générale.

## Blocs de construction| Composant | Résumé | Portail complet |
|-----------|---------|--------------|
| Les filles de Prostraнство (DS) | Les règles de gouvernance en vigueur, les règles de gouvernance ou celles qui ne sont pas disponibles, sont prises en compte par les validateurs, les classes privées et политику комиссий + DA. | См. [Spécification Nexus](./nexus-spec) pour les schémas de manifestation. |
| Voie | Utilisation de shards déterminirovaniens; Nous avons pris des engagements pour soutenir le groupe NPoS mondial. Les voies de classe incluent `default_public`, `public_custom`, `private_permissioned` et `hybrid_confidential`. | [Modèle de voie](./nexus-lane-model) décrit la géométrie, les paramètres et la configuration. |
| Plan avant | Placeholder-identifiants, modes de conversion et d'exploitation de votre profil, pour une conversion simple en termes d'évolution Nexus. | [Notes de transition](./nexus-transition-notes) documenter la phase de migration. |
| Répertoire spatial | Contrat de restauration, voici les manifestations + versions DS. Les opérateurs ont toujours consulté le catalogue avec ce catalogue avant l'installation. | Les collecteurs de différentiel Treker sont disponibles dans `docs/source/project_tracker/nexus_config_deltas/`. |
| Catalogue voie | La configuration secrète `[nexus]` prend en charge la voie des identifiants avec des pseudonymes, des marchés politiques et des responsables de DA. `irohad --sora --config … --trace-config` fournit un catalogue de périphériques pour les auditeurs. | Utilisez `docs/source/sora_nexus_operator_onboarding.md` pour la procédure pas à pas CLI. || Règlement du routeur | L'opérateur XOR s'est associé à la voie CBDC avec la voie de liquidité publique. | `docs/source/cbdc_lane_playbook.md` décrit les portes politiques et les portes télémétriques. |
| Télémétrie/SLO | Les dossiers + alertes dans `dashboards/grafana/nexus_*.json` indiquent la voie, l'arriéré DA, le règlement et la gouvernance du groupe. | [Plan de remédiation par télémétrie](./nexus-telemetry-remediation) описывает дашbordы, алерты и аудит-доказательства. |

## Снимок развертывания| Faza | Focus | Critères à considérer |
|-------|-------|--------------------|
| N0 - Fermeture bêta | Le registraire est en charge de la mise en œuvre du système (`.sora`), des opérateurs d'intégration rapide, de la voie du catalogue statique. | Подписанные DS manifestes + отрепетированные передачи gouvernance. |
| N1 - Prise en charge publique | Добавляет суффиксы `.nexus`, аукционы, registraire en libre-service, pour le règlement XOR. | Les tests de synchronisation du résolveur/passerelle, les paiements à bord, sont effectués à l'occasion. |
| N2 - Résolution | Consultez `.dao`, API revendeur, analyses, portails, cartes de pointage pour les stewards. | Версионированные артефакты, politique en ligne-jury boîte à outils, отчеты прозрачности казначейства. |
| Vorota NX-12/13/14 | Le moteur de conformité, les frontières télémétriques et la documentation sont disponibles auprès des pilotes partenaires. | [Présentation Nexus](./nexus-overview) + [Opérations Nexus](./nexus-operations) опубликованы, дашборды подключены, Policy Engine слит. |

## Ответственность операторов1. **Configuration** - connectez `config/config.toml` à la synchronisation avec la voie du catalogue disponible et les espaces de données ; ARCHIVIRуйте вывод `--trace-config` с каждым релизным тикетом.
2. **Отслеживание манифестов** - сверяйте записи каталога споследним пакетом Space Directory avant l'installation ou la mise à jour des utilisateurs.
3. **Покрытие телеметрией** - publiez `nexus_lanes.json`, `nexus_settlement.json` et votre SDK ; Envoyez des alertes sur PagerDuty et déclenchez des alertes sur le plan de remédiation.
4. **Détecter les incidents** - Sélectionnez les services de matrice dans [Opérations Nexus](./nexus-operations) et activez RCA dans les techniques des utilisateurs. дней.
5. **Détails de la gouvernance** - sélectionnez le système Nexus, passez à votre voie et suivez les instructions de restauration du quartier. (отслеживается через `docs/source/project_tracker/nexus_config_deltas/`).

## См. также

-Observateur canonique : `docs/source/nexus_overview.md`
- Spécifications supplémentaires : [./nexus-spec](./nexus-spec)
- Voie géométrique : [./nexus-lane-model](./nexus-lane-model)
- Plan précédent : [./nexus-transition-notes](./nexus-transition-notes)
- Plan de remédiation télémétrique : [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Fonctionnement du Runbook : [./nexus-operations](./nexus-operations)
- Nom des opérateurs d'intégration : `docs/source/sora_nexus_operator_onboarding.md`