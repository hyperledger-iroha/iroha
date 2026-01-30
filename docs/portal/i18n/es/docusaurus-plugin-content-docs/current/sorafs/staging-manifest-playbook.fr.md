---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: staging-manifest-playbook
title: Playbook de manifest en staging
sidebar_label: Playbook de manifest en staging
description: Checklist pour activer le profil chunker ratifié par le Parlement sur les déploiements Torii de staging.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Gardez la copie Docusaurus et le Markdown hérité alignés jusqu'à la retraite complète du set Sphinx.
:::

## Vue d'ensemble

Ce playbook décrit l'activation du profil chunker ratifié par le Parlement sur un déploiement Torii de staging avant de promouvoir le changement en production. Il suppose que la charte de gouvernance SoraFS est ratifiée et que les fixtures canoniques sont disponibles dans le dépôt.

## 1. Prérequis

1. Synchronisez les fixtures canoniques et les signatures :

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Préparez le répertoire des enveloppes d'admission que Torii lira au démarrage (chemin d'exemple) : `/var/lib/iroha/admission/sorafs`.
3. Assurez-vous que la configuration Torii active le cache discovery et l'application de l'admission :

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

## 2. Publier les enveloppes d'admission

1. Copiez les enveloppes d'admission approuvées dans le répertoire référencé par `torii.sorafs.discovery.admission.envelopes_dir` :

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Redémarrez Torii (ou envoyez un SIGHUP si vous avez encapsulé le loader avec un rechargement à chaud).
3. Suivez les logs pour les messages d'admission :

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Valider la propagation discovery

1. Postez le payload signé d'advert fournisseur (octets Norito) produit par votre pipeline fournisseur :

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Interrogez l'endpoint discovery et confirmez que l'advert apparaît avec les alias canoniques :

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Assurez-vous que `profile_aliases` inclut `"sorafs.sf1@1.0.0"` en première entrée.

## 4. Exercer les endpoints manifest et plan

1. Récupérez les métadonnées du manifest (nécessite un stream token si l'admission est appliquée) :

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Inspectez la sortie JSON et vérifiez :
   - `chunk_profile_handle` est `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` correspond au rapport de déterminisme.
   - `chunk_digests_blake3` s'alignent avec les fixtures régénérées.

## 5. Vérifications de télémétrie

- Confirmez que Prometheus expose les nouvelles métriques de profil :

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Les dashboards doivent montrer le provider de staging sous l'alias attendu et garder les compteurs de brownout à zéro tant que le profil est actif.

## 6. Préparation du rollout

1. Capturez un court rapport avec les URLs, l'ID de manifest et le snapshot de télémétrie.
2. Partagez le rapport dans le canal de rollout Nexus avec la fenêtre d'activation production planifiée.
3. Passez à la checklist de production (Section 4 dans `chunker_registry_rollout_checklist.md`) une fois que les parties prenantes donnent leur accord.

Maintenir ce playbook à jour garantit que chaque rollout chunker/admission suit les mêmes étapes déterministes entre staging et production.
