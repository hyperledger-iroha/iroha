---
lang: fr
direction: ltr
source: docs/source/compliance/android/device_lab_failover_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 473b2b49d32c32d2b884b670ba35e9aa3d0606cfd451d441a7ca927c1160311d
source_last_modified: "2026-01-03T18:07:59.262670+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Runbook d'exploration de basculement du laboratoire d'appareils Android (AND6/AND7)

Ce runbook capture la procédure, les exigences en matière de preuves et la matrice de contact
utilisé lors de l'exercice du **plan d'urgence du laboratoire d'appareils** référencé dans
`roadmap.md` (§« Approbations des produits réglementaires et éventualités du laboratoire »). Il complète
le workflow de réservation (`device_lab_reservation.md`) et le journal des incidents
(`device_lab_contingency.md`) donc les réviseurs de conformité, les conseillers juridiques et SRE
disposer d’une source unique de vérité sur la façon dont nous validons la préparation au basculement.

## Objectif et cadence

- Démontrer que les pools d'appareils généraux Android StrongBox + peuvent basculer
  aux voies de pixels de secours, au pool partagé, à la file d'attente en rafale Firebase Test Lab et
  support StrongBox externe sans manquer les SLA AND6/AND7.
- Produire un ensemble de preuves que les services juridiques peuvent joindre aux soumissions ETSI/FISC
  avant l’examen de conformité de février.
- Exécuter au moins une fois par trimestre, et à chaque fois que la liste du matériel du laboratoire change
  (nouveaux appareils, mise hors service ou maintenance supérieure à 24h).

| ID de forage | Dates | Scénario | Ensemble de preuves | Statut |
|--------------|------|----------|-----------------|--------|
| DR-2026-02-Q1 | 2026-02-20 | Panne de voie Pixel8Pro simulée + arriéré d'attestation avec répétition de télémétrie AND7 | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | ✅ Terminé – hachages de bundle enregistrés dans `docs/source/compliance/android/evidence_log.csv`. |
| DR-2026-05-Q2 | 2026-05-22 (prévu) | Chevauchement de maintenance StrongBox + répétition Nexus | `artifacts/android/device_lab_contingency/20260522-failover-drill/` *(en attente)* — Le billet `_android-device-lab` **AND6-DR-202605** détient les réservations ; le paquet sera rempli après l’exercice. | 🗓 Planifié : bloc de calendrier ajouté à « Android Device Lab – Réservations » par cadence AND6. |

## Procédure

### 1. Préparation du pré-perçage

1. Confirmez la capacité de base dans `docs/source/sdk/android/android_strongbox_capture_status.md`.
2. Exportez le calendrier de réservation pour la semaine ISO cible via
   `python3 scripts/android_device_lab_export.py --week <ISO week>`.
3. Déposer le ticket `_android-device-lab`
   `AND6-DR-<YYYYMM>` avec portée (« exercice de basculement »), emplacements prévus et concernés
   charges de travail (attestation, fumée CI, chaos de télémétrie).
4. Mettez à jour le modèle de journal d'urgence dans `device_lab_contingency.md` avec un
   ligne d'espace réservé pour la date de l'exercice.

### 2. Simuler les conditions de défaillance

1. Désactivez ou supprimez le couloir principal (`pixel8pro-strongbox-a`) à l'intérieur du laboratoire.
   planificateur et marquez l’entrée de réservation comme « exercice ».
2. Déclenchez une alerte de panne simulée dans PagerDuty (service `AND6-device-lab`) et
   capturez l’exportation de notification pour le groupe de preuves.
3. Annotez les travaux Buildkite qui consomment normalement la voie
   (`android-strongbox-attestation`, `android-ci-e2e`) avec l'ID de forage.

### 3. Exécution du basculement1. Promouvoir la voie de secours Pixel7 en cible CI principale et planifier le
   charges de travail prévues par rapport à cela.
2. Déclenchez la suite de rafales Firebase Test Lab via la voie `firebase-burst` pour
   les tests de fumée du portefeuille de détail tandis que la couverture StrongBox se déplace vers le partage
   voie. Capturez l'invocation CLI (ou l'exportation de console) dans le ticket pour l'audit
   parité.
3. Engagez le dispositif de retenue externe du laboratoire StrongBox pour un bref balayage d'attestation ;
   enregistrer l’accusé de réception du contact comme décrit ci-dessous.
4. Enregistrez tous les ID d'exécution Buildkite, les URL de tâches Firebase et les transcriptions de rétention dans
   le ticket `_android-device-lab` et le manifeste du lot de preuves.

### 4. Validation et restauration

1. Comparez les durées d'exécution des attestations/CI par rapport à la référence ; drapeau deltas >10 % par rapport au
   Responsable du laboratoire de matériel.
2. Restaurez la voie principale et mettez à jour l'instantané de capacité ainsi que l'état de préparation
   matrice une fois la validation réussie.
3. Ajoutez la dernière ligne à `device_lab_contingency.md` avec le déclencheur, les actions,
   et les suivis.
4. Mettez à jour `docs/source/compliance/android/evidence_log.csv` avec :
   chemin du bundle, manifeste SHA-256, ID d'exécution Buildkite, hachage d'exportation PagerDuty et
   approbation du réviseur.

## Disposition du lot de preuves

| Fichier | Descriptif |
|------|-------------|
| `README.md` | Résumé (ID de l'exercice, portée, propriétaires, chronologie). |
| `bundle-manifest.json` | Carte SHA-256 pour chaque fichier du bundle. |
| `calendar-export.{ics,json}` | Calendrier de réservation ISO-semaine à partir du script d'exportation. |
| `pagerduty/incident_<id>.json` | Exportation d'incident PagerDuty affichant la chronologie de l'alerte et de l'accusé de réception. |
| `buildkite/<job>.txt` | Buildkite exécute les URL et les journaux pour les tâches concernées. |
| `firebase/burst_report.json` | Résumé de l'exécution en rafale de Firebase Test Lab. |
| `retainer/acknowledgement.eml` | Confirmation du laboratoire externe StrongBox. |
| `photos/` | Photos/captures d'écran facultatives de la topologie du laboratoire si le matériel a été recâblé. |

Conservez le paquet à
`artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` et enregistrement
la somme de contrôle du manifeste dans le journal des preuves ainsi que la liste de contrôle de conformité AND6.

## Matrice de contact et d'escalade

| Rôle | Contact principal | Chaîne(s) | Remarques |
|------|-------|------------|-------|
| Responsable du laboratoire de matériel | Priya Ramanathan | `@android-lab` Slack · +81-3-5550-1234 | Possède les actions sur site et les mises à jour du calendrier. |
| Opérations du laboratoire d'appareils | Mateo Cruz | File d'attente `_android-device-lab` | Coordonne les billets de réservation + les téléchargements de forfaits. |
| Ingénierie des versions | Alexeï Morozov | Libérer Slack · `release-eng@iroha.org` | Valide les preuves Buildkite + publie les hachages. |
| Laboratoire StrongBox externe | Sakura Instruments CNO | `noc@sakura.example` · +81-3-5550-9876 | Contact de retenue ; confirmer la disponibilité dans les 6h. |
| Coordonnateur de rafales Firebase | Tessa Wright | `@android-ci` Mou | Déclenche l'automatisation de Firebase Test Lab lorsqu'une solution de secours est nécessaire. |

Escaladez dans l'ordre suivant si une analyse révèle des problèmes bloquants :
1. Responsable du laboratoire de matériel
2. Fondations Android TL
3. Responsable du programme / Ingénierie des versions
4. Responsable de la conformité + conseiller juridique (si l'exercice révèle un risque réglementaire)

## Rapports et suivis- Liez ce runbook à la procédure de réservation à chaque référencement
  préparation au basculement dans les paquets `roadmap.md`, `status.md` et de gouvernance.
- Envoyez par e-mail le récapitulatif trimestriel de l'exercice à Compliance + Legal avec le lot de preuves
  table de hachage et joignez l'exportation du ticket `_android-device-lab`.
- Mettre en miroir les indicateurs clés (délai de basculement, charges de travail restaurées, actions en cours)
  à l'intérieur de `status.md` et du tracker de liste chaude AND7 afin que les réviseurs puissent retracer le
  dépendance à une répétition concrète.