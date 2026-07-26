---
lang: fr
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb91ce03aee552c65d15ed1c019da4b3b3db9d48d299b3374ca78b4a8c6c1781
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Gouvernance des bindings et des fixtures SDK

WP1-E dans le roadmap désigne « docs/bindings » comme l'emplacement canonique
pour suivre l'état des bindings multi-langages. Ce document récapitule l'inventaire
des bindings, les commandes de régénération, les garde-fous de dérive et les
emplacements des preuves afin que les gates de parité GPU (WP1-E/F/G) et le
conseil de cadence inter-SDK disposent d'une référence unique.

## Garde-fous partagés
- **Playbook canonique :** `docs/source/norito_binding_regen_playbook.md` décrit
  la politique de rotation, les preuves attendues et le flux d'escalade pour
  Android, Swift, Python et les futurs bindings.
- **Parité du schéma Norito :** `scripts/check_norito_bindings_sync.py` (appelé via
  `scripts/check_norito_bindings_sync.sh` et bloqué en CI par
  `ci/check_norito_bindings_sync.sh`) empêche les builds lorsque les artefacts de
  schéma Rust, Java ou Python dérivent.
- **Watchdog de cadence :** `scripts/check_fixture_cadence.py` lit les fichiers
  `artifacts/*_fixture_regen_state.json` et impose les fenêtres Mar/Ven (Android,
  Python) et Mer (Swift) afin que les gates du roadmap aient des timestamps
  auditables.

## Matrice des bindings

| Binding | Points d'entrée | Commande fixtures/régénération | Garde-fous de dérive | Preuves |
|---------|-----------------|-------------------------------|----------------------|---------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (optionnellement `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## Détails des bindings

### Android (Java)
Le SDK Android se trouve dans `java/iroha_android/` et consomme les fixtures
Norito canoniques produites par `scripts/android_fixture_regen.sh`. Ce helper
exporte des blobs `.norito` frais depuis l'outillage Rust, met à jour
`artifacts/android_fixture_regen_state.json` et consigne les métadonnées de
cadence consommées par `scripts/check_fixture_cadence.py` et les dashboards de
 gouvernance. La dérive est détectée par `scripts/check_android_fixtures.py` (aussi
branché sur `ci/check_android_fixtures.sh`) et par `java/iroha_android/run_tests.sh`,
qui exerce les bindings JNI, la relecture de file WorkManager et les fallbacks
StrongBox. Les preuves de rotation, notes d'échec et transcriptions de rerun
vivent dans `artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
`IrohaSwift/` reflète les mêmes payloads Norito via `scripts/swift_fixture_regen.sh`.
Le script enregistre le propriétaire de rotation, l'étiquette de cadence et la
source (`live` vs `archive`) dans `artifacts/swift_fixture_regen_state.json` et
alimente le cadence checker. `scripts/swift_fixture_archive.py` permet aux
mainteneurs d'ingérer des archives générées par Rust; `scripts/check_swift_fixtures.py`
et `ci/check_swift_fixtures.sh` imposent la parité byte à byte et les limites d'âge
SLA, tandis que `scripts/swift_fixture_regen.sh` supporte `SWIFT_FIXTURE_EVENT_TRIGGER`
pour les rotations manuelles. Le workflow d'escalade, les KPI et les dashboards
sont documentés dans `docs/source/swift_parity_triage.md` et les briefs de cadence
sous `docs/source/sdk/swift/`.

### Python
Le client Python (`python/iroha_python/`) partage les fixtures Android. L'exécution
de `scripts/python_fixture_regen.sh` récupère les derniers payloads `.norito`,
actualise `python/iroha_python/tests/fixtures/`, et émettra des métadonnées de
cadence dans `artifacts/python_fixture_regen_state.json` après la première rotation
post-roadmap. `scripts/check_python_fixtures.py` et
`python/iroha_python/scripts/run_checks.sh` bloquent pytest, mypy, ruff et la parité
 des fixtures localement et en CI. La documentation end-to-end
(`docs/source/sdk/python/…`) et le playbook de régénération décrivent comment
coordonner les rotations avec les propriétaires Android.

### JavaScript
`javascript/iroha_js/` ne dépend pas de fichiers `.norito` locaux, mais WP1-E suit
ses preuves de release afin que les lanes GPU héritent d'une provenance complète.
Chaque release capture la provenance via `npm run release:provenance` (propulsé par
`javascript/iroha_js/scripts/record-release-provenance.mjs`), génère et signe les
bundles SBOM avec `scripts/js_sbom_provenance.sh`, exécute le staging signé
(`scripts/js_signed_staging.sh`) et vérifie l'artefact du registre avec
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. Les métadonnées résultantes
atterrissent dans `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`,
`artifacts/js/sbom/` et `artifacts/js/verification/`, offrant des preuves
 déterministes pour les runs roadmap JS5/JS6 et WP1-F. Le playbook de publication
 dans `docs/source/sdk/js/` relie toute l'automatisation.
