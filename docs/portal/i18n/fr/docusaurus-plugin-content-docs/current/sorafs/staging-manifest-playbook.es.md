---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : staging-manifest-playbook
titre : Playbook de manifeste en mise en scène
sidebar_label : Playbook de manifeste et de mise en scène
description : Liste de contrôle pour autoriser le profil de chunker ratifié par le Parlement en despliegues Torii de staging.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Mantén est une copie synchronisée.
:::

## CV

Ce manuel décrit comment autoriser le profil de gros morceau ratifié par le Parlement dans un document Torii de mise en scène avant de promouvoir le changement de production. Supposons que la charte d'administration de SoraFS ait été ratifiée et que les éléments canoniques soient disponibles dans le référentiel.

## 1. Prérequis

1. Synchronisation des appareils canoniques et des entreprises :

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Préparez le répertoire des données d'admission que Torii consultera au début (itinéraire par exemple) : `/var/lib/iroha/admission/sorafs`.
3. Assurez-vous que la configuration de Torii permet le cache de découverte et l'application d'admission :

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

## 2. Publier les conditions d'admission

1. Copie des notes d'admission approuvées par le directeur référencé par `torii.sorafs.discovery.admission.envelopes_dir` :

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Reinicia Torii (ou envoyer un SIGHUP si vous utilisez le chargeur avec recharge en chaleur).
3. Révisez les journaux pour les messages d'admission :

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Valider la propagation de la découverte

1. Publier la charge utile de l'annonce du fournisseur (octets Norito) produite par votre pipeline de fournisseur :

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Consultez le point de terminaison de découverte et confirmez que l'annonce apparaît avec les alias canoniques :

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Assurez-vous que `profile_aliases` inclut `"sorafs.sf1@1.0.0"` comme première entrée.

## 4. Vérifier les points finaux du manifeste et du plan

1. Obtenez les métadonnées du manifeste (nécessitez un jeton de flux si l'admission est activée) :

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Inspectez la sortie JSON et vérifiez :
   - `chunk_profile_handle` et `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` coïncide avec le rapport de déterminisme.
   - `chunk_digests_blake3` est aligné avec les luminaires régénérés.

## 5. Comprobations de télémétrie

- Confirmez que Prometheus expose les nouvelles valeurs du profil :

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Les tableaux de bord doivent montrer le fournisseur de staging sous l'alias attendu et maintenir les contadores de baisse de tension en zéro pendant que le profil est actif.

## 6. Préparation au déploiement

1. Capturez un rapport court avec les URL, l'ID du manifeste et l'instantané de télémétrie.
2. Comparez le rapport sur le canal de déploiement de Nexus avec la fenêtre planifiée d'activation en production.
3. Continuez avec la liste de contrôle de production (Section 4 du `chunker_registry_rollout_checklist.md`) une fois que les parties intéressées par la vue sont bonnes.

Maintenir ce playbook actualisé garantit que chaque déploiement de chunker/admission suit les mêmes étapes déterminées entre la mise en scène et la production.