---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : staging-manifest-playbook
titre : Playbook de manifester dans la mise en scène
sidebar_label : Playbook du manifeste dans la mise en scène
description : Liste de contrôle pour activer le profil du chunker ratifié par le Parlement dans les déploiements Torii de staging.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Mantenha ambas comme copies synchronisées.
:::

## Visa général

Ce playbook décrit comment habiliter le profil de chunker ratifié par le Parlement dans un déploiement Torii de mise en scène avant de promouvoir un changement pour la production. Nous supposons que la charte de gouvernance de la SoraFS a été ratifiée et que les installations canoniques ne sont pas disponibles dans un dépôt.

## 1. Prérequis

1. Synchronisez les appareils canoniques et assinaturas :

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Préparez les enveloppes du répertoire d'admission que le Torii ne démarrera pas (caminho exemple) : `/var/lib/iroha/admission/sorafs`.
3. Garantissez que la configuration du Torii permet le cache de découverte et l'application de l'admission :

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. Enveloppes d'admission publiques

1. Copiez les enveloppes d'admission du fournisseur approuvées pour le diretorio referenciado por `torii.sorafs.discovery.admission.envelopes_dir` :

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Réinitialisez le Torii (vous souhaitez un SIGHUP se lancer dans le chargeur avec un rechargement à chaud).
3. Accompagner les journaux pour les messages d'admission :

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Valider la propagande de découverte

1. Publique ou charge utile assinée par l'annonce du fournisseur (octets Norito) produite par le pipeline du fournisseur :

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Consultez le point de terminaison de découverte et confirmez que l'annonce apparaît avec les alias canoniques :

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Garanta que `profile_aliases` inclut `"sorafs.sf1@1.0.0"` comme première entrée.

## 4. Exercer les points finaux du manifeste et du plan

1. Assurez-vous que les métadonnées se manifestent (exigez que le jeton de flux soit appliqué) :

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Inspectez le JSON et vérifiez :
   - `chunk_profile_handle` et `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` correspond au rapport de déterminisme.
   - `chunk_digests_blake3` alinham avec luminaires régénérés.

## 5. Vérifications de télémétrie

- Confirmez que l'exposition Prometheus correspond aux nouvelles mesures du profil :

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Les tableaux de bord doivent montrer le fournisseur de staging sous l'alias attendu et gérer les contadores de baisse de tension à zéro en ce qui concerne le profil actif.

## 6. Préparation au déploiement

1. Capturez un lien vers les URL, l'ID du manifeste et l'instantané de télémétrie.
2. Comparez le rapport sur le canal de déploiement du Nexus avec la première plan d'activation en production.
3. Proposez la liste de contrôle de production (Section 4 dans `chunker_registry_rollout_checklist.md`) lorsque les parties intéressées sont approuvées.

De cette manière, ce playbook est actualisé et garantit que chaque déploiement de chunker/admission implique que nous avons passé des étapes déterminées entre la mise en scène et la production.