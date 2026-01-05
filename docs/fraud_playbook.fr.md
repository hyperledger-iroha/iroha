---
lang: fr
direction: ltr
source: docs/fraud_playbook.md
status: complete
translator: manual
source_hash: b3253ff47a513529c1dba6ef44faf38087ee0e5f5520f8c3fd770ab8d36c7786
source_last_modified: "2025-11-02T04:40:28.812006+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/fraud_playbook.md (Fraud Governance Playbook) -->

# Guide de Gouvernance de la Fraude

Ce document résume l’infrastructure nécessaire pour la stack de fraude PSP
pendant que les micro‑services complets et SDKs sont encore en développement
actif. Il formalise les attentes en matière d’analytique, de workflows
d’audit et de procédures de repli, afin que les futures implémentations
puissent se brancher sur le registre en toute sécurité.

## Vue d’ensemble des services

1. **API Gateway** – reçoit des payloads `RiskQuery` synchrones, les transmet à
   l’agrégation de features et renvoie les réponses `FraudAssessment` vers les
   flux du registre. Une haute disponibilité (actif‑actif) est requise ; utilisez
   des paires régionales avec hashing déterministe pour éviter les déséquilibres
   de charge.
2. **Agrégation de Features** – compose des vecteurs de caractéristiques pour le
   scoring. N’émet que des hashes `FeatureInput` ; les payloads sensibles restent
   off‑chain. L’observabilité doit publier des histogrammes de latence, des
   jauges de profondeur de file et des compteurs de replays par tenant.
3. **Moteur de Risque** – évalue des règles/modèles et produit des sorties
   `FraudAssessment` déterministes. Assurez que l’ordre d’exécution des règles
   soit stable et capturez des journaux d’audit pour chaque identifiant
   d’évaluation.

## Analytique et promotion des modèles

- **Détection d’anomalies** : maintenir un job de streaming qui signale les
  écarts dans les taux de décision par tenant. Acheminer les alertes vers le
  tableau de bord de gouvernance et conserver des résumés pour les revues
  trimestrielles.
- **Analyse de graphes** : exécuter chaque nuit des traversées de graphe sur les
  exports relationnels pour identifier des clusters de collusion. Exporter les
  résultats dans le portail de gouvernance via `GovernanceExport` avec
  références aux preuves associées.
- **Ingestion de feedback** : consolider les résultats de revue manuelle et les
  rapports de chargeback. Les convertir en deltas de features et les intégrer
  aux jeux de données d’entraînement. Publier des métriques d’état
  d’ingestion pour que l’équipe risque puisse détecter les flux bloqués.
- **Pipeline de promotion de modèles** : automatiser l’évaluation des
  candidats (métriques offline, scoring canari, préparation au rollback).
  Chaque promotion doit émettre un ensemble d’échantillons `FraudAssessment`
  signé et mettre à jour le champ `model_version` dans `GovernanceExport`.

## Workflow d’audit

1. Prendre un snapshot du `GovernanceExport` le plus récent et vérifier que le
   `policy_digest` correspond au manifest fourni par l’équipe risque.
2. Vérifier que les agrégats de règles se recoupent avec les totaux de décisions
   côté registre sur la fenêtre échantillonnée.
3. Examiner les rapports de détection d’anomalies et d’analyse de graphes pour
   les problèmes encore ouverts. Documenter les escalades et les responsables
   attendus des actions correctives.
4. Signer et archiver la checklist de revue. Stocker les artefacts encodés en
   Norito dans le portail de gouvernance pour assurer la reproductibilité.

## Playbooks de repli

- **Panne du moteur** : si le moteur de risque est indisponible plus de
  60 secondes, la gateway doit passer en mode revue uniquement, en émettant
  `AssessmentDecision::Review` pour toutes les requêtes et en alertant les
  opérateurs.
- **Gap de télémétrie** : lorsque les métriques ou traces prennent du retard
  (absence pendant 5 minutes), suspendre les promotions automatiques de modèles
  et prévenir l’ingénieur d’astreinte.
- **Régression de modèle** : si le feedback après déploiement indique une
  hausse des pertes liées à la fraude, revenir au bundle de modèle signé
  précédent et mettre à jour la feuille de route avec des actions correctives.

## Accords de partage de données

- Maintenir des annexes spécifiques par juridiction couvrant rétention,
  chiffrement et SLA de notification d’incident. Les partenaires doivent signer
  l’annexe avant de recevoir des exports `FraudAssessment`.
- Documenter les pratiques de minimisation des données pour chaque intégration
  (par exemple hachage des identifiants de compte, troncature des numéros de
  carte).
- Actualiser les accords chaque année ou dès que les exigences réglementaires
  évoluent.

## Exercices de Red Team

- Les exercices ont lieu chaque trimestre. La prochaine session est prévue pour
  le **2026‑01‑15**, avec des scénarios couvrant l’empoisonnement de features,
  l’amplification de replays et des tentatives de falsification de signatures.
- Consigner les constats dans le modèle de menace fraude et ajouter les tâches
  résultantes à `roadmap.md` dans le workstream
  « Fraud & Telemetry Governance Loop ».

## Schémas d’API

La gateway expose désormais des enveloppes JSON concrètes qui se mettent en
correspondance un‑à‑un avec les types Norito implémentés dans
`crates/iroha_data_model::fraud` :

- **Entrée de risque** – `POST /v1/fraud/query` accepte le schéma `RiskQuery` :
  - `query_id` (`[u8; 32]`, encodé en hex)
  - `subject` (`AccountId`, `<name>@<domain>`)
  - `operation` (enum tagué correspondant à `RiskOperation` ; le discriminant
    JSON `type` reflète la variante de l’enum)
  - `related_asset` (`AssetId`, optionnel)
  - `features` (tableau de `{ key: String, value_hash: hex32 }` dérivé de
    `FeatureInput`)
  - `issued_at_ms` (`u64`)
  - `context` (`RiskContext` ; contient `tenant_id`, un `session_id` optionnel
    et un `reason` optionnel)
- **Décision de risque** – `POST /v1/fraud/assessment` consomme le payload
  `FraudAssessment` (également reflété dans les exports de gouvernance) :
  - `query_id`, `engine_id`, `risk_score_bps`, `confidence_bps`,
    `decision` (enum `AssessmentDecision`), `rule_outcomes`
    (tableau de `{ rule_id, score_delta_bps, rationale? }`)
  - `generated_at_ms`
  - `signature` (base64 optionnel englobant l’évaluation `FraudAssessment`
    encodée en Norito)
- **Export de gouvernance** – `GET /v1/fraud/governance/export` renvoie la
  structure `GovernanceExport` lorsque la feature `governance` est activée,
  incluant les paramètres actifs, le dernier enactment, la version de modèle,
  le digest de politique et l’histogramme `DecisionAggregate`.

Des tests de round‑trip dans `crates/iroha_data_model/src/fraud/types.rs`
garantissent que ces schémas restent alignés sur le format binaire du codec
Norito, et `integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs`
exerce le pipeline d’entrée/décision de bout en bout.

## Références SDK PSP

Les stubs de langage suivants suivent les exemples d’intégration côté PSP :

- **Rust** – `integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs`
  utilise le client `iroha` du workspace pour construire la metadata
  `RiskQuery` et valider les succès/échecs d’admission.
- **TypeScript** – `docs/source/governance_api.md` documente la surface REST
  consommée par la gateway Torii légère utilisée dans le tableau de bord de
  démonstration PSP ; le client scripté se trouve dans
  `scripts/ci/schedule_fraud_scoring.sh` pour les drills de smoke.
- **Swift et Kotlin** – les SDK existants (`IrohaSwift` et les références dans
  `crates/iroha_cli/docs/multisig.md`) exposent les hooks de metadata Torii
  nécessaires pour attacher les champs `fraud_assessment_*`. Les helpers
  spécifiques PSP sont suivis dans le milestone « Fraud & Telemetry Governance
  Loop » dans `status.md` et réutilisent les builders de transactions de ces
  SDKs.

Ces références sont maintenues synchronisées avec la gateway de micro‑services,
de sorte que les implémenteurs PSP disposent toujours d’un schéma et d’un
chemin d’exemple à jour pour chaque langage pris en charge.
