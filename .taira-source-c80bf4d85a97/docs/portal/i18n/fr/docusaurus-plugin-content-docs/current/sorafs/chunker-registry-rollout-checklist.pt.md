---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : liste de contrôle du déploiement du registre chunker
titre : Liste de contrôle du déploiement de l'enregistrement du chunker da SoraFS
sidebar_label : Liste de contrôle du déploiement du chunker
description : Plan de déploiement étape par étape pour actualiser l'enregistrement du chunker.
---

:::note Fonte canonica
Reflète `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Mantenha ambas comme copies synchronisées.
:::

# Liste de contrôle du déploiement de l'enregistrement du SoraFS

Cette liste de contrôle capture les étapes nécessaires pour promouvoir un nouveau profil de chunker
ou bundle d'admission de fournisseur de révision pour produire après la charte
de gouvernance pour ratificado.

> **Escopo :** Appliquer toutes les versions à modifier
> `sorafs_manifest::chunker_registry`, enveloppes d'admission prestataire, ou liasses
> de luminaires canonicos (`fixtures/sorafs_chunker/*`).

## 1. Pré-vol Validacao

1. Régénérer les luminaires et vérifier le déterminisme :
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Confirmez que les hachages sont déterministes
   `docs/source/sorafs/reports/sf1_determinism.md` (ou le rapport de profil
   pertinente) batem com os artefatos régénérados.
3. Garanta que `sorafs_manifest::chunker_registry` compile avec
   `ensure_charter_compliance()` exécuté :
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Actualisez le dossier de la proposition :
   -`docs/source/sorafs/proposals/<profile>.json`
   - Entrée des données du conseil dans `docs/source/sorafs/council_minutes_*.md`
   - Rapport de déterminisme

## 2. Signature de la gouvernance1. Présenter le rapport du groupe de travail sur l'outillage et le résumé de la proposition
   Panel sur les infrastructures du Parlement de Sora.
2. Registre détaillé de l'approbation
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Publique ou enveloppe assinado pelo Parlamento junto com os luminaires :
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Vérifiez si l'enveloppe est disponible via l'aide à la gouvernance :
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Déployez la mise en scène

Consultez le [playbook de manifest em staging](./staging-manifest-playbook) pour un
passo a passo detalhado.

1. Implanter Torii avec découverte `torii.sorafs` habilité et application de l'admission
   ligado (`enforce_admission = true`).
2. Envie les enveloppes d'admission du fournisseur approuvées pour le répertoire de registre
   de staging référencé par `torii.sorafs.discovery.admission.envelopes_dir`.
3. Vérifiez que les annonces du fournisseur sont diffusées via une API de découverte :
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Exercer les points finaux du manifeste/plan avec les en-têtes de gouvernance :
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Confirmez que les tableaux de bord de télémétrie (`torii_sorafs_*`) et les registres d'alerte
   rapporter un nouveau profil sem erreurs.

## 4. Déployer en production1. Répétez les étapes de staging de nos nœuds Torii de production.
2. Annoncer la période d'activation (données/heure, période de grâce, plan de restauration) ici
   canaux d'opérateurs et SDK.
3. Fusionner le PR de la version actuelle :
   - Luminaires et enveloppe actualisés
   - Mudancas na documentacao (références à la charte, rapport de déterminisme)
   - Actualiser la feuille de route/le statut
4. Tagueie a release e archive os artefatos assinados para provenance.

## 5. Déploiement pos-Auditoria

1. Capturer les métriques finales (nombre de découvertes, taxons de réussite de récupération, histogrammes
   par erreur) 24h après le déploiement.
2. Actualisez `status.md` avec un résumé et un lien pour le rapport de déterminisme.
3. Registre tarefas de acompanhamento (ex., orientacao adicional para authoring
   de perfis) em `roadmap.md`.