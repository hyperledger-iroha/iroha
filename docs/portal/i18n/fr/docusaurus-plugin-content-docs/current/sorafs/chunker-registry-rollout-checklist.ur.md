---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : liste de contrôle du déploiement du registre chunker
titre : Déploiement du registre du chunker SoraFS ici
sidebar_label : Déploiement de chunker ici
description : mises à jour du registre chunker en cours de déploiement
---

:::note مستند ماخذ
:::

# Déploiement du registre SoraFS ici

Nous avons un profil de chunker et un ensemble d'admissions de fournisseurs ainsi qu'un examen et une production.
تک promouvoir کرنے کے لیے درکار مراحل کو capturer کرتی ہے جب charte de gouvernance
ratifier ہو چکا ہو۔

> **Portée :** Les versions پر لاگو ہے جو
> `sorafs_manifest::chunker_registry`, enveloppes d'admission des prestataires, canonique
> faisceaux de luminaires (`fixtures/sorafs_chunker/*`)

## 1. Validation avant vol

1. Les appareils génèrent des éléments et vérifient le déterminisme :
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. `docs/source/sorafs/reports/sf1_determinism.md` (rapport de profil یا متعلقہ) میں
   le déterminisme hache les artefacts régénérés et correspond à
3. یقینی بنائیں کہ `sorafs_manifest::chunker_registry`,
   `ensure_charter_compliance()` consiste à compiler un fichier :
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. dossier de proposition اپڈیٹ کریں :
   -`docs/source/sorafs/proposals/<profile>.json`
   - Inscription du procès-verbal du conseil `docs/source/sorafs/council_minutes_*.md`
   - Rapport de déterminisme

## 2. Approbation de la gouvernance1. Rapport du groupe de travail sur l'outillage et résumé de la proposition ici
   Panel sur les infrastructures du Parlement de Sora
2. détails de l'approbation ici
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md` میں ریکارڈ کریں۔
3. Enveloppe signée par le Parlement et calendrier publié:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Aide à la récupération de la gouvernance pour enveloppe accessible en ligne :
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Déploiement de la mise en scène

Voici les étapes de la procédure pas à pas pour [playbook du manifeste de mise en scène] (./staging-manifest-playbook) دیکھیں۔

1. La découverte Torii et `torii.sorafs` a activé l'application de l'admission sur
   (`enforce_admission = true`) Pour déployer le système
2. Enveloppes d'admission des prestataires approuvés et répertoire du registre intermédiaire en mode push
   Voir `torii.sorafs.discovery.admission.envelopes_dir` se référer à کرتا ہے۔
3. API de découverte pour les annonces des fournisseurs et la vérification de la propagation :
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. En-têtes de gouvernance et exercice des points finaux du manifeste/plan :
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. tableaux de bord de télémétrie (`torii_sorafs_*`) et règles d'alerte et profil de profil
   رپورٹنگ بغیر erreurs کے confirmer کریں۔

## 4. Déploiement de la production1. étapes de préparation pour la production des nœuds Torii et répétition
2. fenêtre d'activation (date/heure, délai de grâce, plan de restauration) par l'opérateur et par le SDK
   les chaînes پر annoncent کریں۔
3. Release PR merge ici :
   - luminaires mis à jour et enveloppe
   - modifications de la documentation (références de charte, rapport de déterminisme)
   - feuille de route/actualisation du statut
4. étiquette de sortie pour les artefacts signés et la provenance et les archives

## 5. Audit post-déploiement

1. déploiement en 24 heures sur les métriques finales (nombre de découvertes, taux de réussite de la récupération, taux d'erreur)
   histogrammes) capturent کریں۔
2. `status.md` pour le résumé et le rapport de déterminisme et le lien pour la mise à jour
3. tâches de suivi (guides de création de profils pour la création de profils) et `roadmap.md` pour la création de profils