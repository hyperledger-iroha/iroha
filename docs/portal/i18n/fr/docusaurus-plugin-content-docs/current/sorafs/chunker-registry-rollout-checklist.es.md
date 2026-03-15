---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : liste de contrôle du déploiement du registre chunker
titre : Liste de contrôle du déploiement du registre de chunker de SoraFS
sidebar_label : Liste de contrôle du déploiement du chunker
description : Plan de déploiement pas à pas pour l'actualisation du registre de chunker.
---

:::note Source canonique
Refleja `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Assurez-vous d'avoir des copies synchronisées jusqu'à ce que le ensemble de documents Sphinx héréditaire soit retiré.
:::

# Liste de contrôle du déploiement du registre SoraFS

Cette liste de contrôle capture les étapes nécessaires pour promouvoir un nouveau profil de chunker
ou un ensemble d'admissions de fournisseurs à partir de la révision de la production après que la
la charte de gouvernement a été ratifiée.

> **Alcance :** Appliquer à toutes les versions que vous modifiez
> `sorafs_manifest::chunker_registry`, les règles d'admission des fournisseurs ou des
> bundles de luminaires canónicos (`fixtures/sorafs_chunker/*`).

## 1. Validation préalable

1. Régénérer les luminaires et vérifier le déterminisme :
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Confirmez que les hachages sont déterministes
   `docs/source/sorafs/reports/sf1_determinism.md` (ou le rapport de profil pertinent)
   coïncide avec les artefactos régénérés.
3. Assurez-vous que `sorafs_manifest::chunker_registry` compile avec
   `ensure_charter_compliance()` exécuté :
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Actualiser le dossier de la proposition :
   -`docs/source/sorafs/proposals/<profile>.json`
   - Entrée des actes du conseiller en `docs/source/sorafs/council_minutes_*.md`
   - Rapport de déterminisme

## 2. Approbation de gouvernement1. Présenter l'information du Tooling Working Group et le résumé de la proposition à
   Panel sur les infrastructures du Parlement de Sora.
2. Enregistrez les détails de l'approbation
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Publica el sobre firmado por el Parlamento junto a los luminaires :
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Vérifiez que la mer est accessible via l'aide à la recherche de gouverneur :
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Déploiement et staging

Consultez le [playbook de manifest en staging](./staging-manifest-playbook) pour un
enregistré en détail ces étapes.

1. Téléchargez Torii avec Discovery `torii.sorafs` habilité et l'application de
   admission activée (`enforce_admission = true`).
2. Sube los sobres d'admission des fournisseurs approuvés au directeur du registre
   de staging référencé par `torii.sorafs.discovery.admission.envelopes_dir`.
3. Vérifiez que les annonces du fournisseur sont diffusées via l'API de découverte :
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. Exécuter les points finaux du manifeste/plan avec les en-têtes de gouvernement :
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Confirmez que les tableaux de bord de télémétrie (`torii_sorafs_*`) et les règles de
   alerta reporten el nouveau profil sin errores.

## 4. Déploiement en production1. Répétez les étapes de mise en scène contre les nœuds Torii de production.
2. Annoncer la fenêtre d'activation (heure/heure, période de grâce, plan de restauration)
   aux canaux des opérateurs et du SDK.
3. Fusion du PR de release qui contient :
   - Luminaires et mises à jour
   - Cambios de documentación (références à la charte, rapport de déterminisme)
   - Actualiser la feuille de route/le statut
4. Étiquette de la version et archivage des artefacts confirmés pour la procédure.

## 5. Auditoría post-déploiement

1. Captura métricas finales (conteos de découverte, tasa de éxito de fetch,
   histogrammes d'erreur) 24 h après le déploiement.
2. Actualisez `status.md` avec un résumé du temps et un lien avec le rapport de déterminisme.
3. Inscrivez-vous à toute tâche de suivi (par exemple, plus guide de l'autorité de profils)
   en `roadmap.md`.