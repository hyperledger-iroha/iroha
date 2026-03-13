---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : liste de contrôle du déploiement du registre chunker
titre : Déploiement du composant реестра chunker SoraFS
sidebar_label : Chunker de déploiement Checklist
description : Déploiement du plan de démarrage pour le chunker de restauration actuel.
---

:::note Канонический источник
Utilisez `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Vous pouvez acheter des copies de synchronisation pour la documentation de Sphinx qui ne vous intéresse pas.
:::

# Tableau de déploiement de la liste SoraFS

Ce package est difficile à utiliser pour la production d'un nouveau profil chunker
ou l'admission du fournisseur de forfaits en fonction des résultats du produit après les ratifications
charte de gouvernance.

> **Область:** применяется ко всем релизам, которые меняют
> `sorafs_manifest::chunker_registry`, enveloppes d'admission du prestataire ou
> Ensembles de luminaires canoniques (`fixtures/sorafs_chunker/*`).

## 1. Validation préalable

1. Préparer les luminaires et vérifier les mesures :
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Assurez-vous que les hachages soient déterminés
   `docs/source/sorafs/reports/sf1_determinism.md` (ou le numéro pertinent
   профиля) совпадают с регенерированными артефактами.
3. Assurez-vous que `sorafs_manifest::chunker_registry` soit compilé avec
   `ensure_charter_compliance()` à propos de :
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Обновите dossier предложения:
   -`docs/source/sorafs/proposals/<profile>.json`
   - Afficher les minutes du `docs/source/sorafs/council_minutes_*.md`
   - Отчет о детерминизме

## 2. Approbation de la gouvernance1. Avant d'ouvrir le groupe de travail sur l'outillage et de résumer les préparatifs de
   Panel sur les infrastructures du Parlement de Sora.
2. Vérifiez les détails de l'activité physique
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Опубликуйте enveloppe, подписанный парламентом, вместе с luminaires :
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Vérifiez que l'enveloppe est fournie par l'assistant de gouvernance :
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Déploiement de la mise en scène

Procédure pas à pas Подробный см. в [playbook de manifeste de mise en scène](./staging-manifest-playbook).

1. Vérifiez Torii avec la découverte `torii.sorafs` et l'application de la loi.
   admission (`enforce_admission = true`).
2. Inscrivez les enveloppes d'admission des prestataires agréés dans le répertoire du registre de préparation,
   указанный в `torii.sorafs.discovery.admission.envelopes_dir`.
3. Vérifiez que les annonces des fournisseurs utilisent l'API de découverte :
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. Programmer le manifeste/plan des points finaux avec les en-têtes de gouvernance :
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Comprendre les tableaux de bord de télémétrie (`torii_sorafs_*`) et les règles d'alerte
   отображают новый profil без ошибок.

## 4. Déploiement de la production

1. Sélectionnez la mise en scène pour le produit Torii-uslax.
2. Vérifiez les activités (temps/temps, délai de grâce, plan de restauration) dans les canaux
   opérateurs et SDK.
3. Assurez-vous de faire des relations publiques avec :
   - Luminaires et enveloppe améliorés
   - Documents relatifs aux détails (concernant la charte, отчет о детерминизме)
   - Mise à jour de la feuille de route/du statut
4. Postez votre publication et archivez les objets d'art selon leur provenance.## 5. Audit post-déploiement

1. Afficher les mesures finales (nombre de découvertes, taux de réussite de récupération, erreur
   histogrammes) 24 heures après le déploiement.
2. Ouvrez le formulaire `status.md` et utilisez le détergent supplémentaire.
3. Donnez des instructions de suivi (par exemple, des conseils supplémentaires pour la création
   profil) dans `roadmap.md`.