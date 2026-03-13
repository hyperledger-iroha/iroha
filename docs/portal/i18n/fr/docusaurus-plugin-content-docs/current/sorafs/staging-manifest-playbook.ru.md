---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : staging-manifest-playbook
titre : Плейбук манифеста для mise en scène
sidebar_label : Manifestation pour la mise en scène
description : Liste de contrôle pour le chunker de profil de conversion, ratifiant le paramètre, pour la mise en scène Torii.
---

:::note Канонический источник
:::

## Обзор

Cet article propose de sélectionner un chunker de profil, ratifiant le paramètre de mise en scène et de conversion Torii avant la production. изменений в прод. Avant cela, pour la mise en œuvre de la certification SoraFS, les appareils canoniques sont téléchargés dans les dépôts.

## 1. Votre hôtel précédent

1. Synchronisez les luminaires et les modules canoniques :

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Ajoutez les enveloppes d'admission du catalogue, selon le Torii, en commençant par le début (par exemple): `/var/lib/iroha/admission/sorafs`.
3. Assurez-vous que la configuration Torii active le cache de découverte et l'admission d'application :

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

## 2. Publication des enveloppes d'admission

1. Copiez les enveloppes d'admission du fournisseur dans le catalogue disponible dans `torii.sorafs.discovery.admission.envelopes_dir` :

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Utilisez Torii (ou activez SIGHUP, sinon vous activez le rechargement à chaud).
3. Следите за логами admission:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Découverte de la province

1. Publier la charge utile de l'annonce du fournisseur (par exemple Norito), pour former votre fournisseur de pipeline :

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Lancez la découverte de points de terminaison et découvrez quelle annonce est associée aux alias canoniques :

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Assurez-vous que `profile_aliases` ajoute `"sorafs.sf1@1.0.0"` à l'élément principal.

## 4. Vérifier le manifeste et le plan des points de terminaison

1. Ouvrir le manifeste de métadonnées (activer le jeton de flux, si l'admission est forcée) :

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Vérifiez le fichier JSON et vérifiez ce qui suit :
   - `chunk_profile_handle` et `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` est compatible avec le dosage externe.
   - `chunk_digests_blake3` est compatible avec les luminaires régénérés.

## 5. Tests de télémétrie

- Veuillez noter que Prometheus publie de nouveaux profils de mesures :

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Les États-Unis doivent permettre de créer un alias de mise en scène et de supprimer une baisse de tension sur le nu, lorsque le profil est actif.

## 6. Déploiement Готовность к

1. Formez la réponse à l'adresse URL, au manifeste d'identification et aux télémètres d'instantanés.
2. Sélectionnez l'ouverture du canal de déploiement Nexus pour planifier l'activation du produit.
3. Accédez à la liste des produits (Section 4 du `chunker_registry_rollout_checklist.md`) après l'installation des appareils de nettoyage.

Veuillez prendre en compte ce problème dans le cadre de la garantie actuelle, pour que chaque module de déploiement/admission soit défini et défini. между mise en scène et production.