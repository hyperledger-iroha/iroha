---
lang: fr
direction: ltr
source: docs/source/compliance/android/and6_compliance_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0ce1be46f9c468915f50de5e38e2f34657b26bf4243fb5ea45dab175789393
source_last_modified: "2026-01-04T11:42:43.489571+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Liste de contrôle de conformité Android AND6

Cette liste de contrôle suit les livrables de conformité qui franchissent le jalon **AND6 -
Renforcement de l’IC et de la conformité**. Il consolide les artefacts réglementaires demandés
dans `roadmap.md` et définit l'agencement du stockage sous
`docs/source/compliance/android/` donc ingénierie de version, support et informations juridiques
peut référencer le même ensemble de preuves avant d’approuver les versions Android.

## Portée et propriétaires

| Zone | Livrables | Propriétaire principal | Sauvegarde / Réviseur |
|------|--------------|---------------|------------------------|
| Ensemble réglementaire de l'UE | Cible de sécurité ETSI EN 319 401, résumé DPIA RGPD, attestation SBOM, journal des preuves | Conformité et droit (Sofia Martins) | Ingénierie des versions (Alexei Morozov) |
| Ensemble réglementaire japonais | Liste de contrôle des contrôles de sécurité FISC, ensembles d'attestations StrongBox bilingues, journal des preuves | Conformité et droit (Daniel Park) | Responsable du programme Android |
| Préparation du laboratoire d'appareils | Suivi des capacités, déclencheurs d'urgence, journal des escalades | Responsable du laboratoire de matériel | Observabilité Android TL |

## Matrice d'artefacts| Artefact | Descriptif | Chemin de stockage | Actualiser la cadence | Remarques |
|--------------|-------------|--------------|-----------------|-------|
| Cible de sécurité ETSI EN 319 401 | Récit décrivant les objectifs/hypothèses de sécurité pour les binaires du SDK Android. | `docs/source/compliance/android/eu/security_target.md` | Revalidez chaque version GA + LTS. | Doit citer les hachages de provenance de construction pour le train de publication. |
| Résumé du RGPD DPIA | Évaluation d'impact sur la protection des données couvrant la télémétrie/la journalisation. | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Annuel + avant changement de télémétrie matière. | Politique de rédaction de référence dans `sdk/android/telemetry_redaction.md`. |
| Attestation SBOM | Signé SBOM plus provenance SLSA pour les artefacts Gradle/Maven. | `docs/source/compliance/android/eu/sbom_attestation.md` | Chaque version GA. | Exécutez `scripts/android_sbom_provenance.sh <version>` pour générer des rapports CycloneDX, des bundles de cosignature et des sommes de contrôle. |
| Liste de contrôle des contrôles de sécurité FISC | Liste de contrôle complétée mappant les contrôles du SDK aux exigences FISC. | `docs/source/compliance/android/jp/fisc_controls_checklist.md` | Annuel + avant les pilotes partenaires JP. | Fournir des rubriques bilingues (EN/JP). |
| Ensemble d'attestation StrongBox (JP) | Récapitulatif d'attestation par appareil + chaîne pour les régulateurs JP. | `docs/source/compliance/android/jp/strongbox_attestation.md` | Lorsqu'un nouveau matériel entre dans le pool. | Pointez vers les artefacts bruts sous `artifacts/android/attestation/<device>/`. |
| Note de signature légale | Résumé du conseil couvrant la portée de l'ETSI/GDPR/FISC, la politique de confidentialité et la chaîne de contrôle des artefacts attachés. | `docs/source/compliance/android/eu/legal_signoff_memo.md` | Chaque fois que le lot d'artefacts change ou qu'une nouvelle juridiction est ajoutée. | Le mémo fait référence aux hachages du journal des preuves et aux liens vers le package d’urgence appareil-laboratoire. |
| Journal des preuves | Index des artefacts soumis avec métadonnées de hachage/horodatage. | `docs/source/compliance/android/evidence_log.csv` | Mis à jour chaque fois qu'une entrée ci-dessus change. | Ajoutez un lien Buildkite + l'approbation du réviseur. |
| Ensemble d'instrumentation appareil-laboratoire | Preuves de télémétrie, de file d'attente et d'attestation spécifiques à un emplacement enregistrées avec le processus défini dans `device_lab_instrumentation.md`. | `artifacts/android/device_lab/<slot>/` (voir `docs/source/compliance/android/device_lab_instrumentation.md`) | Chaque emplacement réservé + exercice de basculement. | Capturez les manifestes SHA-256 et référencez l'ID de l'emplacement dans le journal des preuves + la liste de contrôle. |
| Journal de réservation du laboratoire d'appareils | Flux de travail de réservation, approbations, instantanés de capacité et échelle d'escalade utilisés pour maintenir les pools StrongBox ≥ 80 % pendant les gels. | `docs/source/compliance/android/device_lab_reservation.md` | Mettre à jour chaque fois que des réservations sont créées/modifiées. | Faites référence aux ID de ticket `_android-device-lab` et à l’exportation du calendrier hebdomadaire indiqués dans la procédure. |
| Runbook de basculement de Device-lab et ensemble d'exercices | Plan de répétition trimestriel et manifeste d'artefact démontrant les voies de secours, la file d'attente en rafale Firebase et la préparation des dispositifs de retenue StrongBox externes. | `docs/source/compliance/android/device_lab_failover_runbook.md` + `artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` | Tous les trimestres (ou après des modifications de la liste de matériel). | Enregistrez les ID d’exploration dans le journal des preuves et joignez le hachage du manifeste + l’exportation PagerDuty notée dans le runbook. |

> **Conseil :** Lorsque vous joignez des PDF ou des artefacts signés en externe, stockez un court
> Wrapper Markdown dans le chemin déposé qui renvoie à l'artefact immuable dans
> la part de gouvernance. Cela permet de garder le dépôt léger tout en préservant le
> piste d'audit.

## Paquet réglementaire de l'UE (ETSI/GDPR)Le paquet de l’UE relie les trois artefacts ci-dessus ainsi que la note juridique :

- Mettre à jour `security_target.md` avec l'identifiant de version, le hachage manifeste Torii,
  et SBOM digest afin que les auditeurs puissent faire correspondre les binaires à la portée déclarée.
- Garder le résumé DPIA aligné sur la dernière politique de rédaction de télémétrie et
  Joignez l'extrait de différence Norito référencé dans `docs/source/sdk/android/telemetry_redaction.md`.
- L'entrée d'attestation SBOM doit inclure : le hachage CycloneDX JSON, la provenance
  le hachage du bundle, la déclaration de cosignature et l'URL de la tâche Buildkite qui les a générés.
- `legal_signoff_memo.md` doit capturer le conseil/date, lister chaque artefact +
  SHA-256, décrivez tous les contrôles compensatoires et créez un lien vers la ligne du journal des preuves
  ainsi que l'ID du ticket PagerDuty qui a suivi l'approbation.

## Paquet réglementaire japonais (FISC/StrongBox)

Les régulateurs japonais s’attendent à un ensemble parallèle avec une documentation bilingue :

- `fisc_controls_checklist.md` reflète la feuille de calcul officielle ; remplir à la fois le
  Colonnes EN et JA et référencer la section spécifique de `sdk/android/security.md`
  ou le pack d'attestation StrongBox qui satisfait chaque contrôle.
- `strongbox_attestation.md` résume les dernières exécutions de
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  (enveloppes JSON + Norito par appareil). Intégrer des liens vers les artefacts immuables
  sous `artifacts/android/attestation/<device>/` et notez la cadence de rotation.
- Enregistrez le modèle de lettre de motivation bilingue fourni avec les soumissions à l'intérieur
  `docs/source/compliance/android/jp/README.md` afin que le support puisse le réutiliser.
- Mettre à jour le journal des preuves avec une seule ligne faisant référence à la liste de contrôle, au
  le hachage du paquet d'attestation et tous les identifiants de ticket de partenaire JP liés à la livraison.

## Flux de travail de soumission

1. **Brouillon** - Le propriétaire prépare l'artefact, enregistre le nom de fichier prévu à partir de
   le tableau ci-dessus et ouvre un PR contenant le stub Markdown mis à jour plus un
   somme de contrôle de la pièce jointe externe.
2. **Review** - Release Engineering confirme que les hachages de provenance correspondent à ceux mis en scène
   binaires ; La conformité vérifie le langage réglementaire ; Le support garantit les SLA et
   les politiques de télémétrie sont correctement référencées.
3. **Sign-off** - Les approbateurs ajoutent leurs noms et dates au tableau `Sign-off`.
   ci-dessous. Le journal des preuves est mis à jour avec l'URL PR et l'exécution de Buildkite.
4. **Publier** - Après l'approbation de la gouvernance SRE, liez l'artefact dans
   `status.md` et mettez à jour les références Android Support Playbook.

### Journal de signature

| Artefact | Révisé par | Dates | RP / Preuves |
|--------------|-------------|------|--------------|
| *(en attente)* | - | - | - |

## Réservation du laboratoire d'appareils et plan d'urgence

Pour atténuer le risque de **disponibilité du laboratoire d'appareils** mentionné dans la feuille de route :- Suivre la capacité hebdomadaire dans `docs/source/compliance/android/evidence_log.csv`
  (colonne `device_lab_capacity_pct`). Ingénierie de publication d'alertes si disponibilité
  tombe en dessous de 70 % pendant deux semaines consécutives.
- Réserver StrongBox/voies générales suivantes
  `docs/source/compliance/android/device_lab_reservation.md` avant chaque
  gel, répétition ou balayage de conformité afin que les demandes, les approbations et les artefacts
  sont capturés dans la file d'attente `_android-device-lab`. Liez les ID de ticket résultants
  dans le journal des preuves lors de l’enregistrement des instantanés de capacité.
- **Piscines de secours :** éclatez d'abord vers le pool de pixels partagé ; s'il est encore saturé,
  planifier les exécutions de fumée du Firebase Test Lab pour la validation CI.
- **Conservateur de laboratoire externe :** maintenez le dispositif de rétention avec le partenaire StrongBox
  laboratoire afin que nous puissions réserver le matériel pendant les fenêtres de gel (délai minimum de 7 jours).
- **Escalade :** déclenche l'incident `AND6-device-lab` dans PagerDuty lorsque les deux
  les pools principaux et de secours tombent en dessous de 50 % de leur capacité. Le responsable du laboratoire de matériel
  se coordonne avec SRE pour redéfinir la priorité des appareils.
- **Ensembles de preuves de basculement :** stockez chaque répétition sous
  `artifacts/android/device_lab_contingency/<YYYYMMDD>/` avec la réservation
  demande, exportation PagerDuty, manifeste matériel et transcription de récupération. Référence
  le bundle de `device_lab_contingency.md` et ajoutez le SHA-256 au journal des preuves
  afin que le service juridique puisse prouver que le flux de travail d'urgence a été exercé.
- **Exercices trimestriels :** exercez le runbook dans
  `docs/source/compliance/android/device_lab_failover_runbook.md`, fixez le
  chemin du bundle résultant + hachage du manifeste vers le ticket `_android-device-lab`, et
  refléter l'ID de l'exercice dans le journal d'urgence et le journal des preuves.

Documenter chaque activation du plan d’urgence dans
`docs/source/compliance/android/device_lab_contingency.md` (inclure la date,
déclencheur, actions et suivis).

## Prototype d'analyse statique

- `make android-lint` encapsule `ci/check_android_javac_lint.sh`, compilation
  `java/iroha_android` et les sources `java/norito_java` partagées avec
  `javac --release 21 -Xlint:all -Werror` (avec les catégories signalées indiquées dans
- Après compilation, le script applique la politique de dépendance AND6 avec
  `jdeps --summary`, échec si un module se trouve en dehors de la liste autorisée approuvée
  (`java.base`, `java.net.http`, `jdk.httpserver`) apparaît. Cela maintient le
  Surface Android alignée sur « pas de dépendances cachées du JDK » du conseil SDK
  exigence avant les examens de conformité StrongBox.
- CI gère désormais la même porte via
  `.github/workflows/android-lint.yml`, qui invoque
  `ci/check_android_javac_lint.sh` à chaque push/PR qui touche Android ou
  partagé Norito sources Java et téléchargements `artifacts/android/lint/jdeps-summary.txt`
  afin que les examens de conformité puissent référencer une liste de modules signés sans réexécuter le
  script localement.
- Définissez `ANDROID_LINT_KEEP_WORKDIR=1` lorsque vous devez conserver le temporaire
  espace de travail. Le script copie déjà le résumé du module généré dans
  `artifacts/android/lint/jdeps-summary.txt` ; ensemble
  `ANDROID_LINT_SUMMARY_OUT=docs/source/compliance/android/evidence/android_lint_jdeps.txt`
  (ou similaire) lorsque vous avez besoin d'un artefact versionné supplémentaire pour les audits.
  Les ingénieurs doivent toujours exécuter la commande localement avant de soumettre les PR Android
  qui touchent les sources Java et joignent le résumé/journal enregistré à la conformité
  critiques. Référencez-le dans les notes de version sous le nom « Android javac lint + dépendance
  scanner ».

## Preuve CI (peluches, tests, attestation)- `.github/workflows/android-and6.yml` gère désormais toutes les portes AND6 (javac lint +
  analyse des dépendances, suite de tests Android, vérificateur d'attestation StrongBox et
  validation de l'emplacement de l'appareil-laboratoire) à chaque PR/push touchant la surface Android.
- `ci/run_android_tests.sh` enveloppe `ci/run_android_tests.sh` et émet
  un résumé déterministe à `artifacts/android/tests/test-summary.json` tandis que
  conserver le journal de la console sur `artifacts/android/tests/test.log`. Attachez les deux
  fichiers aux paquets de conformité lors du référencement des exécutions de CI.
- `scripts/android_strongbox_attestation_ci.sh --summary-out` produit
  `artifacts/android/attestation/ci-summary.json`, validation du package
  chaînes d'attestation sous `artifacts/android/attestation/**` pour StrongBox et
  Piscines TEE.
-`scripts/check_android_device_lab_slot.py --root fixtures/android/device_lab`
  vérifie l'emplacement d'échantillon (`slot-sample/`) utilisé dans CI et peut être pointé vers
  les courses réelles sous `artifacts/android/device_lab/<slot-id>/` avec
  `--require-slot --json-out <dest>` pour prouver que les ensembles d'instruments suivent
  la mise en page documentée. CI rédige le résumé de validation à
  `artifacts/android/device_lab/summary.json` ; le créneau d'échantillonnage comprend
  espace réservé télémétrie/attestation/file d'attente/extraits de journaux plus un enregistrement
  `sha256sum.txt` pour des hachages reproductibles.

## Flux de travail d'instrumentation de périphérique-laboratoire

Chaque répétition de réservation ou de basculement doit suivre les
Guide `device_lab_instrumentation.md` donc télémétrie, file d'attente et attestation
les artefacts s'alignent avec le journal de réservation :

1. **Artefacts de fente de graine.** Créer
   `artifacts/android/device_lab/<slot>/` avec les sous-dossiers standards et exécutez
   `shasum` après la fermeture de l'emplacement (voir la section « Disposition des artefacts » du nouveau
   guider).
2. **Exécuter les commandes d'instrumentation.** Exécuter la capture de télémétrie/file d'attente,
   remplacer le résumé, le harnais StrongBox et l'analyse des peluches/dépendances exactement comme
   documenté afin que les sorties reflètent CI.
3. **Déposer des preuves.** Mise à jour
   `docs/source/compliance/android/evidence_log.csv` et le ticket de réservation
   avec l'ID d'emplacement, le chemin du manifeste SHA-256 et le tableau de bord/Buildkite correspondant
   liens.

Joignez le dossier d'artefact et le manifeste de hachage au paquet de version AND6 pour
la fenêtre de gel concernée. Les évaluateurs de la gouvernance rejetteront les listes de contrôle qui le font
ne citez pas un identifiant de slot ni le guide d'instrumentation.

### Preuves de préparation aux réservations et au basculement

L’élément de la feuille de route « Approbations des artefacts réglementaires et imprévus en laboratoire » nécessite davantage
que les instruments. Chaque paquet AND6 doit également faire référence au proactif
workflow de réservation et répétition trimestrielle du basculement :- **Playbook de réservation (`device_lab_reservation.md`).** Suivre la réservation
  tableau (délais, propriétaires, longueurs de créneaux), exporter le calendrier partagé via
  `scripts/android_device_lab_export.py` et enregistrement `_android-device-lab`
  ID de ticket ainsi que des instantanés de capacité dans `evidence_log.csv`. Le livre de jeu
  énonce l'échelle d'escalade et les déclencheurs d'urgence ; copie ces détails
  dans l'entrée de la liste de contrôle lorsque les réservations bougent ou que la capacité tombe en dessous du
  Objectif de la feuille de route de 80 %.
- **Runbook d'exploration de basculement (`device_lab_failover_runbook.md`).** Exécuter le
  répétition trimestrielle (simuler une panne → promouvoir les voies de secours → engager
  Firebase burst + partenaire StrongBox externe) et stockez les artefacts sous
  `artifacts/android/device_lab_contingency/<drill-id>/`. Chaque paquet doit
  contient le manifeste, l'exportation PagerDuty, les liens d'exécution Buildkite, la rafale Firebase
  rapport et un accusé de réception noté dans le runbook. Référencez le
  ID d'exercice, manifeste SHA-256 et ticket de suivi dans le journal des preuves et
  cette liste de contrôle.

Ensemble, ces documents prouvent que la planification de la capacité des appareils, les répétitions de pannes,
et les ensembles d'instruments partagent la même piste auditée exigée par le
feuille de route et réviseurs juridiques.

## Cadence de révision

- **Trimestriel** - Vérifier que les artefacts UE/JP sont à jour ; actualiser
  hachages du journal des preuves ; répéter la capture de provenance.
- **Pré-version** - Exécutez cette liste de contrôle lors de chaque basculement GA/LTS et attachez-la
  le journal complété dans la version RFC.
- **Post-incident** - Si un incident du mois de septembre touche la télémétrie, la signature ou
  attestation, mettre à jour les talons d'artefacts pertinents avec des notes de remédiation et
  capturer la référence dans le journal des preuves.