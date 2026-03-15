---
lang: fr
direction: ltr
source: docs/source/compliance/android/eu/security_target.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0
source_last_modified: "2026-01-03T18:07:59.195967+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Cible de sécurité du SDK Android — Alignement ETSI EN 319 401

| Champ | Valeur |
|-------|-------|
| Version du document | 0,1 (2026-02-12) |
| Portée | SDK Android (bibliothèques client sous `java/iroha_android/` plus scripts/documents de prise en charge) |
| Propriétaire | Conformité et droit (Sofia Martins) |
| Réviseurs | Responsable du programme Android, ingénierie des versions, gouvernance SRE |

## 1. Description de la TOE

La cible d'évaluation (TOE) comprend le code de la bibliothèque du SDK Android (`java/iroha_android/src/main/java`), sa surface de configuration (ingestion `ClientConfig` + Norito) et les outils opérationnels référencés dans `roadmap.md` pour les jalons AND2/AND6/AND7.

Composants principaux :

1. **Ingestion de configuration** : `ClientConfig` threads les points de terminaison Torii, les politiques TLS, les tentatives et les hooks de télémétrie à partir du manifeste `iroha_config` généré et applique l'immuabilité après l'initialisation (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`).
2. **Gestion des clés / StrongBox** — La signature matérielle est implémentée via `SystemAndroidKeystoreBackend` et `AttestationVerifier`, avec des politiques documentées dans `docs/source/sdk/android/key_management.md`. La capture/validation de l'attestation utilise `scripts/android_keystore_attestation.sh` et l'assistant CI `scripts/android_strongbox_attestation_ci.sh`.
3. **Télémétrie et rédaction** — L'instrumentation transite par le schéma partagé décrit dans `docs/source/sdk/android/telemetry_redaction.md`, exportant les autorités hachées, les profils d'appareil regroupés et remplaçant les crochets d'audit appliqués par le Support Playbook.
4. **Les runbooks d'exploitation** — `docs/source/android_runbook.md` (réponse de l'opérateur) et `docs/source/android_support_playbook.md` (SLA + escalade) renforcent l'empreinte opérationnelle de la TOE avec des remplacements déterministes, des exercices de chaos et la capture de preuves.
5. **Provenance de la version** — Les versions basées sur Gradle utilisent le plugin CycloneDX ainsi que les indicateurs de version reproductibles tels que capturés dans `docs/source/sdk/android/developer_experience_plan.md` et la liste de contrôle de conformité AND6. Les artefacts de version sont signés et référencés dans `docs/source/release/provenance/android/`.

## 2. Actifs et hypothèses

| Actif | Descriptif | Objectif de sécurité |
|-------|-------------|----------|
| Manifestes de configuration | Instantanés `ClientConfig` dérivés de Norito distribués avec des applications. | Authenticité, intégrité et confidentialité au repos. |
| Clés de signature | Clés générées ou importées via les fournisseurs StrongBox/TEE. | Préférence StrongBox, journalisation des attestations, pas d'exportation de clé. |
| Flux de télémétrie | Traces/journaux/métriques OTLP exportés à partir de l'instrumentation SDK. | Pseudonymisation (autorités hachées), informations personnelles minimisées, remplacement de l'audit. |
| Interactions avec le grand livre | Charges utiles Norito, métadonnées d'admission, trafic réseau Torii. | Authentification mutuelle, requêtes résistantes à la relecture, tentatives déterministes. |

Hypothèses :

- Le système d'exploitation mobile fournit un sandboxing standard + SELinux ; Les appareils StrongBox implémentent l'interface keymaster de Google.
- Les opérateurs fournissent aux points de terminaison Torii des certificats TLS signés par des autorités de certification approuvées par le conseil.
- L'infrastructure de construction respecte les exigences de construction reproductibles avant de publier sur Maven.

## 3. Menaces et contrôles| Menace | Contrôle | Preuve |
|--------|---------|--------------|
| Manifestes de configuration falsifiés | `ClientConfig` valide les manifestes (hachage + schéma) avant de les appliquer et enregistre les rechargements refusés via `android.telemetry.config.reload`. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java` ; `docs/source/android_runbook.md` §1–2. |
| Compromission des clés de signature | Les politiques requises par StrongBox, les harnais d'attestation et les audits de matrice de périphériques identifient les dérives ; remplacements documentés par incident. | `docs/source/sdk/android/key_management.md` ; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` ; `scripts/android_strongbox_attestation_ci.sh`. |
| Fuite de PII dans la télémétrie | Autorités hachées Blake2b, profils d'appareils regroupés, omission de l'opérateur, journalisation de remplacement. | `docs/source/sdk/android/telemetry_redaction.md` ; Supporte le Playbook §8. |
| Rejouer ou rétrograder sur Torii RPC | Le générateur de requêtes `/v2/pipeline` applique l'épinglage TLS, la politique de canal de bruit et les budgets de nouvelles tentatives avec un contexte d'autorité hachée. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ToriiRequestBuilder.java` ; `docs/source/sdk/android/networking.md` (prévu). |
| Versions non signées ou non reproductibles | Attestations CycloneDX SBOM + Sigstore sécurisées par la liste de contrôle AND6 ; Les RFC de publication nécessitent des preuves dans `docs/source/release/provenance/android/`. | `docs/source/sdk/android/developer_experience_plan.md` ; `docs/source/compliance/android/eu/sbom_attestation.md`. |
| Gestion incomplète des incidents | Runbook + playbook définissent les remplacements, les exercices de chaos et l'arborescence d'escalade ; les remplacements de télémétrie nécessitent des requêtes Norito signées. | `docs/source/android_runbook.md` ; `docs/source/android_support_playbook.md`. |

## 4. Activités d'évaluation

1. **Revue de conception** — Conformité + SRE vérifient que la configuration, la gestion des clés, la télémétrie et les contrôles de publication correspondent aux objectifs de sécurité de l'ETSI.
2. **Contrôles de mise en œuvre** — Tests automatisés :
   - `scripts/android_strongbox_attestation_ci.sh` vérifie les bundles capturés pour chaque périphérique StrongBox répertorié dans la matrice.
   - `scripts/check_android_samples.sh` et Managed Device CI garantissent que les exemples d'applications respectent les contrats `ClientConfig`/télémétrie.
3. **Validation opérationnelle** — Exercices de chaos trimestriels selon `docs/source/sdk/android/telemetry_chaos_checklist.md` (exercices de rédaction + remplacement).
4. **Conservation des preuves** — Artefacts stockés sous `docs/source/compliance/android/` (ce dossier) et référencés à partir de `status.md`.

## 5. Cartographie ETSI EN 319 401| EN 319 401 Article | Contrôle du SDK |
|---------|-------------|
| 7.1 Politique de sécurité | Documenté dans cette cible de sécurité + Support Playbook. |
| 7.2 Sécurité organisationnelle | Propriété RACI + d'astreinte dans Support Playbook §2. |
| 7.3 Gestion des actifs | Objectifs des actifs de configuration, de clé et de télémétrie définis au §2 ci-dessus. |
| 7.4 Contrôle d'accès | Politiques StrongBox + workflow de remplacement nécessitant des artefacts Norito signés. |
| 7.5 Contrôles cryptographiques | Exigences de génération, de stockage et d’attestation de clés de AND2 (guide de gestion des clés). |
| 7.6 Sécurité des opérations | Hachage de télémétrie, répétitions de chaos, réponse aux incidents et libération des preuves. |
| 7.7 Sécurité des communications | `/v2/pipeline` Politique TLS + autorités hachées (doc de rédaction de télémétrie). |
| 7.8 Acquisition/développement du système | Constructions Gradle reproductibles, SBOM et portes de provenance dans les plans AND5/AND6. |
| 7.9 Relations avec les fournisseurs | Attestations Buildkite + Sigstore enregistrées aux côtés des SBOM de dépendance tiers. |
| 7.10 Gestion des incidents | Escalade Runbook/Playbook, journalisation des remplacements, compteurs d'échecs de télémétrie. |

## 6. Entretien

- Mettez à jour ce document chaque fois que le SDK introduit de nouveaux algorithmes cryptographiques, catégories de télémétrie ou modifications d'automatisation des versions.
- Liez les copies signées dans `docs/source/compliance/android/evidence_log.csv` avec les résumés SHA-256 et les approbations des réviseurs.