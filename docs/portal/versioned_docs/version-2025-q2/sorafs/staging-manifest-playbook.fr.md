---
lang: fr
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2026-01-03T18:07:58.297179+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: staging-manifest-playbook-fr
slug: /sorafs/staging-manifest-playbook-fr
---

:::note Source canonique
Rétroviseurs `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Gardez les deux copies alignées dans les versions.
:::

## Aperçu

Ce playbook explique comment activer le profil chunker ratifié par le Parlement sur un déploiement Torii avant de promouvoir le passage à la production. Cela suppose que la charte de gouvernance SoraFS a été ratifiée et que les textes canoniques sont disponibles dans le référentiel.

## 1. Prérequis

1. Synchronisez les appareils canoniques et les signatures :

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Préparez le répertoire de l'enveloppe d'admission que Torii lira au démarrage (exemple de chemin) : `/var/lib/iroha/admission/sorafs`.
3. Assurez-vous que la configuration Torii active le cache de découverte et l'application de l'admission :

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

1. Copiez les enveloppes d'admission des prestataires agréés dans le répertoire référencé par `torii.sorafs.discovery.admission.envelopes_dir` :

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Redémarrez Torii (ou envoyez un SIGHUP si vous avez enveloppé le chargeur avec un rechargement à la volée).
3. Suivez les journaux pour les messages d'admission :

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Valider la propagation de la découverte

1. Publiez la charge utile signée de l'annonce du fournisseur (octets Norito) produite par votre
   pipeline de fournisseur :

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Recherchez le point de terminaison de découverte et confirmez que l'annonce apparaît avec des alias canoniques :

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Assurez-vous que `profile_aliases` inclut `"sorafs.sf1@1.0.0"` comme première entrée.

## 4. Manifeste d'exercice et points finaux du plan

1. Récupérez les métadonnées du manifeste (nécessite un jeton de flux si l'admission est forcée) :

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Inspectez la sortie JSON et vérifiez :
   - `chunk_profile_handle` est `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` correspond au rapport de déterminisme.
   - `chunk_digests_blake3` s'aligne avec les luminaires régénérés.

## 5. Vérifications de télémétrie

- Confirmez que Prometheus expose les nouvelles métriques de profil :

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Les tableaux de bord doivent afficher le fournisseur intermédiaire sous l'alias attendu et maintenir les compteurs de baisse de tension à zéro pendant que le profil est actif.

## 6. Préparation au déploiement

1. Capturez un court rapport avec les URL, l'ID du manifeste et l'instantané de télémétrie.
2. Partagez le rapport dans le canal de déploiement Nexus parallèlement à la fenêtre d'activation de production prévue.
3. Passez à la liste de contrôle de production (section 4 du document `chunker_registry_rollout_checklist.md`) une fois que les parties prenantes ont signé.

En gardant ce playbook à jour, chaque déploiement de chunker/admission suit les mêmes étapes déterministes tout au long de la préparation et de la production.
