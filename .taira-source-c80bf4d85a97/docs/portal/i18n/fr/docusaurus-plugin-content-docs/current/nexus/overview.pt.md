---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu du lien
titre : Visa général de Sora Nexus
description : Résumé de haut niveau de l'architecture du Iroha 3 (Sora Nexus) avec les ajouts pour les documents canoniques du mono-repo.
---

Nexus (Iroha 3) est Iroha 2 pour l'exécution multivoie, des espaces de données protégés par la gouvernance et des outils partagés dans chaque SDK. Cette page affiche le nouveau résumé `docs/source/nexus_overview.md` pas de mono-repo pour que les lecteurs du portail entendent rapidement comment les pièces d'architecture sont encaixam.

## Linhas de release

- **Iroha 2** - implantacoes auto-hospedadas para consorcios ou redes privadas.
- **Iroha 3 / Sora Nexus** - un réseau public multi-voies où les opérateurs enregistrent des espaces de données (DS) et des ferraments partagés de gouvernance, de liquidation et d'observation.
- Comprend les lignes de compilation de mon espace de travail (IVM + chaîne d'outils Kotodama), ainsi que les corrections du SDK, les mises à jour d'ABI et les appareils Norito, ports permanents. Les opérateurs utilisent le bundle `iroha3-<version>-<os>.tar.zst` pour entrer dans le numéro Nexus ; consultez `docs/source/sora_nexus_operator_onboarding.md` pour la liste de contrôle en tela cheia.

## Blocs de construction| Composants | CV | Pontos do portail |
|-----------|---------|--------------|
| Espace de données (DS) | Le Dominio de execucao/armazenamento a défini la gouvernance qui possède une ou plusieurs voies, déclare les conjuntos de validadores, classe de privacidade et politica de taxas + DA. | Voir [Nexus spec](./nexus-spec) pour l'esquema du manifeste. |
| Voie | Shard déterministe de l'exécution ; émettre des compromissos que o anel global NPoS ordena. Les classes de voie comprennent `default_public`, `public_custom`, `private_permissioned` et `hybrid_confidential`. | Le [modelo de lane](./nexus-lane-model) capture la géométrie, les préfixes d'armement et de retenue. |
| Plan de transition | Espace réservé aux identifiants, phases de rotation et d'emballage de double profil accompagnant les implants de voie d'évolution unique pour Nexus. | Comme [notes de transition](./nexus-transition-notes) documentent chaque phase de migration. |
| Répertoire spatial | Contrato de registro que armazena manifestos + versoes de DS. Les opérateurs rapprochent les entrées du catalogue avec ce répertoire avant l'entrée. | Le rastreador des différences de manifeste vive em `docs/source/project_tracker/nexus_config_deltas/`. |
| Catalogue des voies | Dans la section `[nexus]`, la configuration des identifiants de voie est effectuée pour les alias, les politiques de rotation et les limites de DA. `irohad --sora --config ... --trace-config` imprimer le catalogue résolu pour les auditoriums. | Utilisez `docs/source/sora_nexus_operator_onboarding.md` pour l'étape CLI. || Rotateur de liquidation | L'opérateur de transfert XOR connecte les voies CBDC privées aux voies de liquidation publiques. | `docs/source/cbdc_lane_playbook.md` détails des boutons politiques et des portes de télémétrie. |
| Télémétrie/SLO | Les tableaux de bord + alertes sur `dashboards/grafana/nexus_*.json` capturent l'altitude des voies, l'arriéré de DA, la latence de liquidation et la profondeur du fil de gouvernance. | Le [plan de résolution de télémétrie](./nexus-telemetry-remediation) tableaux de bord détaillés, alertes et preuves de l'auditoire. |

## Instantané du déploiement| Phase | Foco | Critères de dite |
|-------|-------|--------------------|
| N0 - Beta fechada | Registrar gerenciado pelo conselho (`.sora`), manuel d'intégration des opérateurs, catalogue des voies statiques. | Manifestes de DS assassinés + transferts de gouvernance ensaiados. |
| N1 - Lancement public | Adiciona sufixos `.nexus`, leiloes, registraire libre-service, cabeamento de liquidacao XOR. | Tests de synchronisation du résolveur/passerelle, tableaux de bord de réconciliation de cobranca, exercices de litige sur la table. |
| N2 - Expansão | Présentation de `.dao`, API de revenus, analyses, portail de litiges, cartes de pointage des stewards. | Artefatos de conformité versionados, boîte à outils du jury de politique en ligne, rapports de transparence du tesouro. |
| Porte NX-12/13/14 | Moteur de conformité, tableaux de bord de télémétrie et documentation développés ensemble avant les pilotes avec les colis. | [Présentation Nexus](./nexus-overview) + [Opérations Nexus](./nexus-operations) publicados, tableaux de bord ligados, moteur de politique intégré. |

## Responsabilités de l'opérateur1. **Hygiène de configuration** - le support `config/config.toml` est synchronisé avec le catalogue publié des voies et des espaces de données ; obtenir un dit `--trace-config` dans chaque ticket de sortie.
2. **Rastreamento de manifestos** - concilier les entrées du catalogue avec le bundle le plus récent du Space Directory avant d'entrer ou d'actualiser nos.
3. **Cobertura de telemetria** - expose les tableaux de bord `nexus_lanes.json`, `nexus_settlement.json` et les tableaux de bord associés au SDK ; Connectez les alertes à PagerDuty et effectuez des révisions trimestrielles conformes au plan de résolution de télémétrie.
4. **Rapport d'incidents** - inscrit sur la matrice de gravité des [opérations Nexus](./nexus-operations) et entre les RCA dans cinq jours.
5. **Prontidao de gouvernance** - participe aux votes du conseil Nexus qui ont un impact sur vos voies et ensaie les instructions de restauration trimestrielle (rastrées via `docs/source/project_tracker/nexus_config_deltas/`).

## Voir aussi

- Visa canonique : `docs/source/nexus_overview.md`
- Spécification détaillée : [./nexus-spec](./nexus-spec)
- Géométrie des voies : [./nexus-lane-model](./nexus-lane-model)
- Plan de transition : [./nexus-transition-notes](./nexus-transition-notes)
- Plan de résolution de télémétrie : [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook des opérations : [./nexus-operations](./nexus-operations)
- Guide d'intégration des opérateurs : `docs/source/sora_nexus_operator_onboarding.md`