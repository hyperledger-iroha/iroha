---
lang: fr
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-04T11:42:43.398592+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Liste de contrôle des versions Android (AND6)

Cette liste de contrôle capture les portes **AND6 — CI et renforcement de la conformité** de
`roadmap.md` (§Priorité 5). Il aligne les versions du SDK Android avec Rust
libérer les attentes RFC en précisant les tâches CI, les artefacts de conformité,
les preuves de laboratoire d'appareils et les lots de provenance qui doivent être joints avant une AG,
LTS, ou train de correctifs, avance.

Utilisez ce document avec :

- `docs/source/android_support_playbook.md` — calendrier des versions, SLA et
  arbre d'escalade.
- `docs/source/android_runbook.md` — runbooks opérationnels quotidiens.
- `docs/source/compliance/android/and6_compliance_checklist.md` — régulateur
  inventaire des objets.
- `docs/source/release_dual_track_runbook.md` — gouvernance des versions à double voie.

## 1. Les portes de scène en un coup d'œil

| Scène | Portes requises | Preuve |
|-------|----------------|----------|
| **T−7 jours (pré-congélation)** | Vert nocturne `ci/run_android_tests.sh` pendant 14 jours ; `ci/check_android_fixtures.sh`, `ci/check_android_samples.sh` et `ci/check_android_docs_i18n.sh` passant ; analyses de peluches/dépendances mises en file d'attente. | Tableaux de bord Buildkite, rapport de comparaison des appareils, exemples de captures d'écran. |
| **T−3 jours (promotion RC)** | Réservation du laboratoire d'appareils confirmée ; Exécution du CI d’attestation StrongBox (`scripts/android_strongbox_attestation_ci.sh`) ; Suites robotiques/instrumentées exercées sur du matériel programmé ; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` propre. | Matrice d'appareil CSV, manifeste du bundle d'attestation, rapports Gradle archivés sous `artifacts/android/lint/<version>/`. |
| **T−1 jour (go/no-go)** | Ensemble d'états de rédaction de télémétrie actualisé (`scripts/telemetry/check_redaction_status.py --write-cache`) ; artefacts de conformité mis à jour selon `and6_compliance_checklist.md` ; répétition de provenance terminée (`scripts/android_sbom_provenance.sh --dry-run`). | `docs/source/compliance/android/evidence_log.csv`, statut de télémétrie JSON, journal d'essai à sec de provenance. |
| **T0 (basculement GA/LTS)** | `scripts/publish_android_sdk.sh --dry-run` terminé ; provenance + signé SBOM ; liste de contrôle de publication exportée et jointe aux minutes go/no-go ; `ci/sdk_sorafs_orchestrator.sh` travail de fumée vert. | Publication des pièces jointes RFC, ensemble Sigstore, artefacts d'adoption sous `artifacts/android/`. |
| **T+1 jour (après la transition)** | Disponibilité du correctif vérifiée (`scripts/publish_android_sdk.sh --validate-bundle`) ; différences du tableau de bord examinées (`ci/check_android_dashboard_parity.sh`); paquet de preuves téléchargé sur `status.md`. | Exportation des différences du tableau de bord, lien vers l'entrée `status.md`, paquet de version archivé. |

## 2. Matrice CI et Quality Gate| Porte | Commande(s) / Script | Remarques |
|------|----------|-------|
| Tests unitaires + intégration | `ci/run_android_tests.sh` (encapsule `ci/run_android_tests.sh`) | Émet `artifacts/android/tests/test-summary.json` + journal de test. Comprend le codec Norito, la file d'attente, la solution de secours StrongBox et les tests de harnais client Torii. Obligatoire tous les soirs et avant le marquage. |
| Parité des rencontres | `ci/check_android_fixtures.sh` (encapsule `scripts/check_android_fixtures.py`) | Garantit que les appareils Norito régénérés correspondent à l'ensemble canonique Rust ; attachez le diff JSON lorsque la porte échoue. |
| Exemples d'applications | `ci/check_android_samples.sh` | Construit `examples/android/{operator-console,retail-wallet}` et valide les captures d'écran localisées via `scripts/android_sample_localization.py`. |
| Documents/I18N | `ci/check_android_docs_i18n.sh` | Guards README + démarrages rapides localisés. Réexécutez une fois que les modifications de la documentation ont atterri dans la branche de publication. |
| Parité du tableau de bord | `ci/check_android_dashboard_parity.sh` | Confirme que les métriques CI/exportées s'alignent sur les homologues de Rust ; requis lors de la vérification T+1. |
| Fumée d'adoption du SDK | `ci/sdk_sorafs_orchestrator.sh` | Exerce les liaisons de l'orchestrateur Sorafs multi-sources avec le SDK actuel. Obligatoire avant de télécharger des artefacts mis en scène. |
| Vérification des attestations | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | Regroupe les ensembles d'attestation StrongBox/TEE sous `artifacts/android/attestation/**` ; joignez le résumé aux paquets GA. |
| Validation des emplacements du laboratoire d'appareils | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | Valide les ensembles d'instruments avant de joindre des preuves aux paquets de publication ; CI s'exécute sur l'emplacement d'échantillon dans `fixtures/android/device_lab/slot-sample` (télémétrie/attestation/file d'attente/journaux + `sha256sum.txt`). |

> **Astuce :** ajoutez ces tâches au pipeline Buildkite `android-release` afin que
> les semaines de congélation réexécutent automatiquement chaque porte avec la pointe de branche de libération.

Le travail `.github/workflows/android-and6.yml` consolidé exécute le lint,
vérifications des suites de tests, des résumés d'attestation et des emplacements des laboratoires d'appareils à chaque PR/push
toucher des sources Android, télécharger des preuves sous `artifacts/android/{lint,tests,attestation,device_lab}/`.

## 3. Analyses de charpie et de dépendances

Exécutez `scripts/android_lint_checks.sh --version <semver>` à partir de la racine du dépôt. Le
le script exécute :

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- Les rapports et les sorties de dependency-guard sont archivés sous
  `artifacts/android/lint/<label>/` et un lien symbolique `latest/` pour la version
  canalisations.
- Les résultats de charpie défaillants nécessitent soit une correction, soit une entrée dans la version.
  RFC documentant le risque accepté (approuvé par Release Engineering + Program
  plomb).
- `dependencyGuardBaseline` régénère le verrou de dépendance ; joindre le différentiel
  au paquet go/no-go.

## 4. Couverture du laboratoire d'appareils et du StrongBox

1. Réservez les appareils Pixel + Galaxy à l'aide du tracker de capacité référencé dans
   `docs/source/compliance/android/device_lab_contingency.md`. Bloque les versions
   si disponibilité ` pour actualiser le rapport d’attestation.
3. Exécutez la matrice d'instrumentation (documentez la liste suite/ABI dans le périphérique
   traqueur). Capturez les échecs dans le journal des incidents même si les tentatives réussissent.
4. Déposez un ticket si le recours à Firebase Test Lab est requis ; lier le billet
   dans la liste de contrôle ci-dessous.

## 5. Artefacts de conformité et de télémétrie- Suivez `docs/source/compliance/android/and6_compliance_checklist.md` pour l'UE
  et les soumissions du JP. Mise à jour `docs/source/compliance/android/evidence_log.csv`
  avec des hachages + URL de tâches Buildkite.
- Actualiser les preuves de rédaction de télémétrie via
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --statut-url https://android-observability.example/status.json`.
  Stockez le JSON résultant sous
  `artifacts/android/telemetry/<version>/status.json`.
- Enregistrez la sortie de différence de schéma de
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  pour prouver la parité avec les exportateurs de Rust.

## 6. Provenance, SBOM et publication

1. Exécutez à sec le pipeline de publication :

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. Générer la provenance SBOM + Sigstore :

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. Joindre `artifacts/android/provenance/<semver>/manifest.json` et signé
   `checksums.sha256` à la version RFC.
4. Lors de la promotion vers le véritable référentiel Maven, réexécutez
   `scripts/publish_android_sdk.sh` sans `--dry-run`, capturez la console
   log et téléchargez les artefacts résultants sur `artifacts/android/maven/<semver>`.

## 7. Modèle de dossier de soumission

Chaque version GA/LTS/hotfix doit inclure :

1. **Liste de contrôle complétée** — copiez le tableau de ce fichier, cochez chaque élément et créez un lien
   aux artefacts de support (exécution de Buildkite, journaux, différences de documentation).
2. **Preuves de laboratoire sur les appareils** — résumé du rapport d'attestation, journal de réservation et
   toute activation d’urgence.
3. **Paquet de télémétrie** — statut de rédaction JSON, différence de schéma, lien vers
   Mises à jour `docs/source/sdk/android/telemetry_redaction.md` (le cas échéant).
4. **Artefacts de conformité** — entrées ajoutées/mises à jour dans le dossier de conformité
   ainsi que le fichier CSV actualisé du journal des preuves.
5. **Ensemble de provenance** — SBOM, signature Sigstore et `checksums.sha256`.
6. **Résumé de la version** — aperçu d'une page joint au résumé `status.md`
   ce qui précède (date, version, point culminant des éventuelles portes levées).

Stockez le paquet sous `artifacts/android/releases/<version>/` et référencez-le
dans `status.md` et la version RFC.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` automatiquement
  copie la dernière archive lint (`artifacts/android/lint/latest`) et le
  les preuves de conformité se connectent à `artifacts/android/releases/<version>/` afin que le
  le paquet de soumission a toujours un emplacement canonique.

---

**Rappel :** mettez à jour cette liste de contrôle chaque fois que de nouvelles tâches CI, des artefacts de conformité,
ou des exigences de télémétrie sont ajoutées. L'élément de la feuille de route AND6 reste ouvert jusqu'à ce que
la liste de contrôle et l'automatisation associée se révèlent stables pour deux versions consécutives
les trains.