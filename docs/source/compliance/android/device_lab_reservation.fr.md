---
lang: fr
direction: ltr
source: docs/source/compliance/android/device_lab_reservation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05dc578338882ddfcdf2410b0643774ceb8212f28739ba94ac83edf087b9b5dc
source_last_modified: "2026-01-03T18:07:59.245516+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Procédure de réservation du laboratoire d'appareils Android (AND6/AND7)

Ce playbook décrit comment l'équipe Android réserve, confirme et audite l'appareil.
temps de laboratoire pour les jalons **AND6** (renforcement de l'IC et de la conformité) et **AND7**
(préparation à l'observabilité). Il complète le journal de contingence
`docs/source/compliance/android/device_lab_contingency.md` en assurant la capacité
les déficits sont évités en premier lieu.

## 1. Objectifs et portée

- Maintenir les pools d'appareils généraux StrongBox + au-dessus des 80 % exigés par la feuille de route
  objectif de capacité tout au long des fenêtres de gel.
- Fournir un calendrier déterministe pour l'IC, les balayages d'attestation et le chaos
  les répétitions ne rivalisent jamais pour le même matériel.
- Capturer une piste auditable (demandes, approbations, notes post-exécution) qui alimente
  la liste de contrôle de conformité AND6 et le journal des preuves.

Cette procédure couvre les voies Pixel dédiées, le pool de secours partagé et
le dispositif de retenue externe du laboratoire StrongBox référencé dans la feuille de route. Émulateur ad hoc
l'utilisation est hors de portée.

## 2. Fenêtres de réservation

| Piscine/Allée | Matériel | Longueur de l'emplacement par défaut | Délai de réservation | Propriétaire |
|-------------|----------|-----------|-------------------|-------|
| `pixel8pro-strongbox-a` | Pixel8Pro (StrongBox) | 4h | 3 jours ouvrables | Responsable du laboratoire de matériel |
| `pixel8a-ci-b` | Pixel8a (CI général) | 2h | 2 jours ouvrables | Fondations Android TL |
| `pixel7-fallback` | Piscine partagée Pixel7 | 2h | 1 jour ouvrable | Ingénierie des versions |
| `firebase-burst` | File d'attente de fumée du laboratoire de test Firebase | 1h | 1 jour ouvrable | Fondations Android TL |
| `strongbox-external` | Support de laboratoire StrongBox externe | 8h | 7 jours calendaires | Responsable du programme |

Les créneaux horaires sont réservés en UTC ; les réservations qui se chevauchent nécessitent une approbation explicite
du responsable du laboratoire de matériel.

## 3. Flux de travail de demande

1. **Préparer le contexte**
   - Mettre à jour `docs/source/sdk/android/android_strongbox_device_matrix.md` avec
     les appareils que vous prévoyez d'exercer et l'étiquette de préparation
     (`attestation`, `ci`, `chaos`, `partner`).
   - Collectez le dernier instantané de capacité de
     `docs/source/sdk/android/android_strongbox_capture_status.md`.
2. **Soumettre la demande**
   - Déposer un ticket dans la file d'attente `_android-device-lab` en utilisant le modèle dans
     `docs/examples/android_device_lab_request.md` (propriétaire, dates, charges de travail,
     exigence de repli).
   - Joindre toutes les dépendances réglementaires (par exemple, balayage d'attestation AND6, AND7
     exercice de télémétrie) et un lien vers l’entrée pertinente de la feuille de route.
3. **Approbation**
   - Le responsable du laboratoire de matériel examine le matériel dans un délai d'un jour ouvrable et confirme sa place dans le
     calendrier partagé (`Android Device Lab – Reservations`) et met à jour le
     Colonne `device_lab_capacity_pct` dans
     `docs/source/compliance/android/evidence_log.csv`.
4. **Exécution**
   - Exécuter les travaux planifiés ; enregistrer les ID d'exécution Buildkite ou les journaux d'outillage.
   - Noter les éventuels écarts (échanges matériels, dépassements).
5. **Clôture**
   - Commentez le ticket avec les artefacts/liens.
   - Si l'exécution était liée à la conformité, mettez à jour
     `docs/source/compliance/android/and6_compliance_checklist.md` et ajoutez une ligne
     à `evidence_log.csv`.

Les demandes qui ont un impact sur les démos des partenaires (AND8) doivent mettre en copie l'ingénierie du partenaire.

## 4. Modification et annulation- **Reprogrammer :** rouvrez le ticket d'origine, proposez un nouveau créneau et mettez à jour le
  entrée de calendrier. Si le nouvel emplacement est disponible dans les 24 heures, envoyez un ping au responsable du laboratoire matériel + SRE.
  directement.
- **Annulation d'urgence :** suivre le plan d'urgence
  (`device_lab_contingency.md`) et enregistrez les lignes déclencheur/action/suivi.
- **Dépassements :** si une exécution dépasse son créneau de > 15 minutes, publiez une mise à jour et confirmez
  si la prochaine réservation peut avoir lieu ; sinon, confiez-le à la solution de secours
  piscine ou voie d'éclatement Firebase.

## 5. Preuves et audit

| Artefact | Localisation | Remarques |
|----------|----------|-------|
| Billets de réservation | File d'attente `_android-device-lab` (Jira) | Exporter le résumé hebdomadaire ; lier les ID de ticket dans le journal des preuves. |
| Exportation du calendrier | `artifacts/android/device_lab/<YYYY-WW>-calendar.{ics,json}` | Exécutez `scripts/android_device_lab_export.py --ics-url <calendar_ics_feed>` chaque vendredi ; l'assistant enregistre le fichier `.ics` filtré ainsi qu'un résumé JSON pour la semaine ISO afin que les audits puissent joindre les deux artefacts sans téléchargements manuels. |
| Instantanés de capacité | `docs/source/compliance/android/evidence_log.csv` | Mise à jour après chaque réservation/fermeture. |
| Notes post-exécution | `docs/source/compliance/android/device_lab_contingency.md` (si imprévu) ou commentaire du ticket | Nécessaire pour les audits. |

Lors des contrôles de conformité trimestriels, joignez l'export du calendrier, le résumé du ticket,
et un extrait du journal des preuves pour la soumission de la liste de contrôle AND6.

### Automatisation de l'exportation du calendrier

1. Obtenez l'URL du flux ICS (ou téléchargez un fichier `.ics`) pour « Android Device Lab – Reservations ».
2. Exécuter

   ```bash
   python3 scripts/android_device_lab_export.py \
     --ics-url "https://calendar.example/ical/export" \
     --week <ISO week, defaults to current>
   ```

   Le script écrit à la fois `artifacts/android/device_lab/<YYYY-WW>-calendar.ics`
   et `...-calendar.json`, capturant la semaine ISO sélectionnée.
3. Téléchargez les fichiers générés avec le paquet de preuves hebdomadaire et référencez le
   Résumé JSON dans `docs/source/compliance/android/evidence_log.csv` lorsque
   capacité du laboratoire du dispositif d'enregistrement.

## 6. Échelle d'escalade

1. Responsable du laboratoire de matériel (primaire)
2. Fondations Android TL
3. Responsable du programme / Ingénierie des versions (pour les fenêtres gelées)
4. Contact externe du laboratoire StrongBox (lorsque le dispositif de rétention est invoqué)

Les escalades doivent être enregistrées dans le ticket et reflétées dans l'Android hebdomadaire.
courrier d'état.

## 7. Documents associés

- `docs/source/compliance/android/device_lab_contingency.md` — journal des incidents pour
  déficits de capacité.
- `docs/source/compliance/android/and6_compliance_checklist.md` — maître
  liste de contrôle des livrables.
- `docs/source/sdk/android/android_strongbox_device_matrix.md` — matériel
  suivi de la couverture.
- `docs/source/sdk/android/android_strongbox_attestation_run_log.md` —
  Preuve d'attestation StrongBox référencée par AND6/AND7.

Le maintien de cette procédure de réservation satisfait à l’élément d’action de la feuille de route « définir
procédure de réservation du laboratoire d'appareils » et conserve les artefacts de conformité destinés aux partenaires
en synchronisation avec le reste du plan de préparation Android.

## 8. Procédure d'exploration de basculement et contacts

L’élément AND6 de la feuille de route nécessite également une répétition de basculement trimestrielle. Le plein,
des instructions étape par étape sont disponibles
`docs/source/compliance/android/device_lab_failover_runbook.md`, mais le haut
le flux de travail au niveau est résumé ci-dessous afin que les demandeurs puissent planifier des exercices parallèlement
réservations courantes.1. **Planifiez l'exercice :** Bloquez les voies concernées (`pixel8pro-strongbox-a`,
   pool de secours, `firebase-burst`, dispositif de retenue StrongBox externe) dans le partage
   calendrier et `_android-device-lab` en file d'attente au moins 7 jours avant l'exercice.
2. **Simuler une panne :** Supprimez la voie principale, déclenchez le PagerDuty
   (`AND6-device-lab`) et annotez les tâches Buildkite dépendantes avec
   l’ID d’exercice noté dans le runbook.
3. **Failover :** Promouvoir la voie de secours Pixel7, lancer la rafale Firebase
   suite et engagez le partenaire StrongBox externe dans les 6 heures. Capturer
   Buildkite exécute les URL, les exportations Firebase et les accusés de réception.
4. **Valider et restaurer :** Vérifiez l'attestation + les environnements d'exécution CI, rétablissez le
   voies d'origine et mise à jour `device_lab_contingency.md` ainsi que le journal des preuves
   avec le chemin du bundle + les sommes de contrôle.

### Référence de contact et d'escalade

| Rôle | Contact principal | Chaîne(s) | Ordre d'escalade |
|------|-------|------------|------------------|
| Responsable du laboratoire de matériel | Priya Ramanathan | `@android-lab` Slack · +81-3-5550-1234 | 1 |
| Opérations du laboratoire d'appareils | Mateo Cruz | File d'attente `_android-device-lab` | 2 |
| Fondations Android TL | Elena Vorobeva | `@android-foundations` Mou | 3 |
| Ingénierie des versions | Alexeï Morozov | `release-eng@iroha.org` | 4 |
| Laboratoire StrongBox externe | Sakura Instruments CNO | `noc@sakura.example` · +81-3-5550-9876 | 5 |

Escalader de manière séquentielle si l'exercice révèle des problèmes de blocage ou en cas de solution de repli
la voie ne peut pas être mise en ligne dans les 30 minutes. Enregistrez toujours l’escalade
notes dans le ticket `_android-device-lab` et reflétez-les dans le journal de contingence.