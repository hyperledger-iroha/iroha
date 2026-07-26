---
lang: fr
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 27b5ac3c7adb19a87f0b3d076f3c9618b188602898ed3954808ac9f7a52b3a62
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Baseline d'automatisation de la documentation Android (AND5)

L'item AND5 du roadmap exige que l'automatisation de la documentation, de la
localisation et de la publication soit auditable avant le démarrage d'AND6
(CI & Compliance). Ce dossier consigne les commandes, les artefacts et la
structure des preuves que AND5/AND6 référencent, en miroir des plans décrits
dans `docs/source/sdk/android/developer_experience_plan.md` et
`docs/source/sdk/android/parity_dashboard_plan.md`.

## Pipelines et commandes

| Tâche | Commande(s) | Artefacts attendus | Notes |
|------|-------------|--------------------|------|
| Synchronisation des stubs de localisation | `python3 scripts/sync_docs_i18n.py` (optionnellement `--lang <code>` par exécution) | Log stocké dans `docs/automation/android/i18n/<timestamp>-sync.log` plus les commits de stubs traduits | Maintient `docs/i18n/manifest.json` aligné avec les stubs traduits; le log enregistre les codes langue touchés et le commit capturé dans la baseline. |
| Vérification des fixtures + parité Norito | `ci/check_android_fixtures.sh` (enveloppe `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`) | Copier le résumé JSON généré dans `docs/automation/android/parity/<stamp>-summary.json` | Vérifie les payloads de `java/iroha_android/src/test/resources`, les hashes de manifestes et les tailles de fixtures signées. Joindre le résumé avec les preuves de cadence sous `artifacts/android/fixture_runs/`. |
| Manifeste d'exemples et preuve de publication | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (exécute tests + SBOM + provenance) | Métadonnées du bundle de provenance et `sample_manifest.json` issu de `docs/source/sdk/android/samples/` stocké dans `docs/automation/android/samples/<version>/` | Relie les applis exemples AND5 à l'automatisation des releases : capturer le manifeste généré, le hash du SBOM et le log de provenance pour la revue beta. |
| Flux du tableau de bord de parité | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` suivi de `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | Copier le snapshot `metrics.prom` ou l'export JSON Grafana dans `docs/automation/android/parity/<stamp>-metrics.prom` | Alimente le plan de dashboard afin que AND5/AND7 puissent vérifier les compteurs de soumissions invalides et l'adoption de la télémétrie. |

## Capture des preuves

1. **Horodatez tout.** Nommez les fichiers avec des timestamps UTC
   (`YYYYMMDDTHHMMSSZ`) pour que les dashboards de parité, les minutes de
   gouvernance et les docs publiées puissent référencer la même exécution.
2. **Référencez les commits.** Chaque log doit inclure le hash du commit de
   l'exécution et toute configuration pertinente (par exemple
   `ANDROID_PARITY_PIPELINE_METADATA`). En cas de contraintes de confidentialité,
   ajoutez une note et un lien vers le coffre sécurisé.
3. **Archivez un contexte minimal.** Seuls les résumés structurés (JSON, `.prom`,
   `.log`) sont versionnés. Les artefacts lourds (bundles APK, captures d'écran)
   restent dans `artifacts/` ou dans un stockage d'objets, avec un hash signé
   consigné dans le log.
4. **Mettez à jour les entrées de statut.** Lorsque les jalons AND5 progressent
   dans `status.md`, citez le fichier correspondant (par exemple
   `docs/automation/android/parity/20260324T010203Z-summary.json`) afin que les
   auditeurs puissent remonter la baseline sans parcourir les logs CI.

Suivre cette structure satisfait le prérequis AND6 « baselines docs/automation
accessibles pour audit » et garde le programme de documentation Android aligné
sur les plans publiés.
