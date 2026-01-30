---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: chunker-registry-rollout-checklist
title: Checklist de rollout du registre chunker SoraFS
sidebar_label: Checklist rollout chunker
description: Plan de rollout pas à pas pour les mises à jour du registre chunker.
---

:::note Source canonique
Reflète `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Gardez les deux copies synchronisées jusqu'à la retraite complète du set Sphinx hérité.
:::

# Checklist de rollout du registre SoraFS

Cette checklist détaille les étapes nécessaires pour promouvoir un nouveau profil
chunker ou un bundle d'admission fournisseur de la revue à la production après
ratification de la charte de gouvernance.

> **Portée :** S'applique à toutes les releases qui modifient
> `sorafs_manifest::chunker_registry`, les envelopes d'admission fournisseurs ou
> les bundles de fixtures canoniques (`fixtures/sorafs_chunker/*`).

## 1. Validation préliminaire

1. Régénérez les fixtures et vérifiez le déterminisme :
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Confirmez que les hashes de déterminisme dans
   `docs/source/sorafs/reports/sf1_determinism.md` (ou le rapport de profil
   pertinent) correspondent aux artefacts régénérés.
3. Assurez-vous que `sorafs_manifest::chunker_registry` compile avec
   `ensure_charter_compliance()` en lançant :
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Mettez à jour le dossier de proposition :
   - `docs/source/sorafs/proposals/<profile>.json`
   - Entrée de minutes du conseil sous `docs/source/sorafs/council_minutes_*.md`
   - Rapport de déterminisme

## 2. Validation gouvernance

1. Présentez le rapport du Tooling Working Group et le digest de la proposition
   au Sora Parliament Infrastructure Panel.
2. Enregistrez les détails d'approbation dans
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Publiez l'envelope signée par le Parlement à côté des fixtures :
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Vérifiez que l'envelope est accessible via le helper de fetch de gouvernance :
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Rollout staging

Référez-vous au [playbook manifest staging](./staging-manifest-playbook) pour une
procédure détaillée.

1. Déployez Torii avec discovery `torii.sorafs` activé et l'application de
   l'admission activée (`enforce_admission = true`).
2. Poussez les envelopes d'admission fournisseurs approuvées dans le répertoire
   de registre staging référencé par `torii.sorafs.discovery.admission.envelopes_dir`.
3. Vérifiez que les provider adverts se propagent via l'API discovery :
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Exercez les endpoints manifest/plan avec des headers de gouvernance :
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Confirmez que les dashboards de télémétrie (`torii_sorafs_*`) et les règles
   d'alerte reportent le nouveau profil sans erreurs.

## 4. Rollout production

1. Répétez les étapes de staging sur les nœuds Torii de production.
2. Annoncez la fenêtre d'activation (date/heure, période de grâce, plan de rollback)
   aux canaux opérateurs et SDK.
3. Mergez le PR de release contenant :
   - Fixtures et envelope mises à jour
   - Changements de documentation (références à la charte, rapport de déterminisme)
   - Refresh roadmap/status
4. Taguez la release et archivez les artefacts signés pour la provenance.

## 5. Audit post-rollout

1. Capturez les métriques finales (comptes discovery, taux de succès fetch,
   histogrammes d'erreurs) 24 h après le rollout.
2. Mettez à jour `status.md` avec un bref résumé et un lien vers le rapport de déterminisme.
3. Consignez les tâches de suivi (ex. guidance d'authoring de profils) dans
   `roadmap.md`.
