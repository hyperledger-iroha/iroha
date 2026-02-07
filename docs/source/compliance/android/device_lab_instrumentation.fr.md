---
lang: fr
direction: ltr
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2026-01-03T18:07:59.259775+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Crochets d'instrumentation de laboratoire pour appareils Android (AND6)

Cette référence clôt l'action de la feuille de route « mettre en scène le périphérique-laboratoire restant /
l’instrumentation s’accroche avant le coup d’envoi de l’AND6 ». Il explique comment chaque réservé
L'emplacement du laboratoire d'appareils doit capturer les artefacts de télémétrie, de file d'attente et d'attestation afin que le
La liste de contrôle de conformité AND6, le journal des preuves et les paquets de gouvernance partagent la même chose
flux de travail déterministe. Associez cette note à la procédure de réservation
(`device_lab_reservation.md`) et le runbook de basculement lors de la planification des répétitions.

## Objectifs et portée

- **Preuve déterministe** – toutes les sorties d'instruments vivent sous
  `artifacts/android/device_lab/<slot-id>/` avec SHA-256 se manifeste pour les auditeurs
  peut comparer les bundles sans réexécuter les sondes.
- **Workflow basé sur le script** : réutilisez les assistants existants
  (`ci/run_android_telemetry_chaos_prep.sh`,
  `scripts/android_keystore_attestation.sh`, `scripts/android_override_tool.sh`)
  au lieu de commandes adb sur mesure.
- **Les listes de contrôle restent synchronisées** – chaque exécution fait référence à ce document du
  Liste de contrôle de conformité AND6 et ajoute les artefacts à
  `docs/source/compliance/android/evidence_log.csv`.

## Disposition des artefacts

1. Choisissez un identifiant de créneau unique qui correspond au ticket de réservation, par ex.
   `2026-05-12-slot-a`.
2. Amorcez les répertoires standards :

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. Enregistrez chaque journal de commandes dans le dossier correspondant (par ex.
   `telemetry/status.ndjson`, `attestation/pixel8pro.log`).
4. Capturez les manifestes SHA-256 une fois l'emplacement fermé :

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## Matrice d'instrumentation

| Flux | Commande(s) | Emplacement de sortie | Remarques |
|------|------------|-------|-------|
| Rédaction de télémétrie + bundle de statut | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | Exécutez au début et à la fin du créneau ; attachez la sortie standard CLI à `status.log`. |
| File d'attente en attente + préparation du chaos | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256` | ScénarioD miroir de `readiness/labs/telemetry_lab_01.md` ; étendez la variable d'environnement pour chaque périphérique de l'emplacement. |
| Remplacer le résumé du grand livre | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | Obligatoire même lorsqu’aucun remplacement n’est actif ; prouver l'état zéro. |
| Attestation StrongBox / TEE | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | Répétez l’opération pour chaque appareil réservé (faites correspondre les noms dans `android_strongbox_device_matrix.md`). |
| Régression d'attestation de harnais CI | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | Capture les mêmes preuves que celles téléchargées par CI ; inclure dans les exécutions manuelles pour la symétrie. |
| Ligne de base de charpie/dépendance | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | Exécuter une fois par fenêtre de gel ; citer le résumé dans les dossiers de conformité. |

## Procédure d'emplacement standard1. **Pré-vol (T-24h)** – Confirmez les références du billet de réservation ici
   document, mettez à jour l'entrée de la matrice de périphériques et amorcez la racine de l'artefact.
2. **Pendant le créneau**
   - Exécutez d'abord les commandes d'exportation du bundle de télémétrie + de la file d'attente. Passer
     `--note <ticket>` à `ci/run_android_telemetry_chaos_prep.sh` donc le journal
     fait référence à l’ID de l’incident.
   - Déclenchez les scripts d'attestation par appareil. Lorsque le harnais produit un
     `.zip`, copiez-le dans la racine de l'artefact et enregistrez le Git SHA imprimé sur
     la fin du scénario.
   - Exécuter `make android-lint` avec le chemin récapitulatif remplacé même si CI
     déjà couru; les auditeurs attendent un journal par emplacement.
3. **Post-exécution**
   - Générez `sha256sum.txt` et `README.md` (notes de forme libre) à l'intérieur du slot
     dossier résumant les commandes exécutées.
   - Ajoutez une ligne à `docs/source/compliance/android/evidence_log.csv` avec le
     ID d'emplacement, chemin du manifeste de hachage, références Buildkite (le cas échéant) et la dernière version
     Pourcentage de capacité du laboratoire de l'appareil à partir de l'exportation du calendrier de réservation.
   - Lier le dossier slot dans le ticket `_android-device-lab`, l'AND6
     liste de contrôle et rapport de version `docs/source/android_support_playbook.md`.

## Gestion et escalade des échecs

- Si une commande échoue, capturez la sortie stderr sous `logs/` et suivez les instructions
  échelle d'escalade dans `device_lab_reservation.md` §6.
- Les déficits de file d'attente ou de télémétrie doivent immédiatement noter l'état de priorité dans
  `docs/source/sdk/android/telemetry_override_log.md` et référencez l'ID de l'emplacement
  afin que la gouvernance puisse retracer l'exercice.
- Les régressions d'attestation doivent être enregistrées dans
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  avec les numéros de série des appareils défaillants et les chemins de bundle enregistrés ci-dessus.

## Liste de contrôle pour les rapports

Avant de marquer l'emplacement comme terminé, vérifiez que les références suivantes sont mises à jour :

- `docs/source/compliance/android/and6_compliance_checklist.md` — marquez le
  complétez la rangée d'instruments et notez l'ID de l'emplacement.
- `docs/source/compliance/android/evidence_log.csv` — ajouter/mettre à jour l'entrée avec
  le hachage du slot et la lecture de la capacité.
- Ticket `_android-device-lab` — joignez des liens d'artefact et des ID de travail Buildkite.
- `status.md` — incluez une brève note dans le prochain résumé de préparation d'Android afin
  les lecteurs de la feuille de route savent quel emplacement a produit les dernières preuves.

Suivre ce processus conserve les « crochets appareil-laboratoire + instrumentation » d'AND6
jalon vérifiable et empêche toute divergence manuelle entre la réservation, l'exécution,
et le reporting.