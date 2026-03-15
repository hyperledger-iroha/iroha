---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : staging-manifest-playbook
titre : Playbook du manifeste de préparation SoraFS
sidebar_label : playbook du manifeste de préparation SoraFS
description : Torii pour les déploiements planifiés et le profil de chunker ratifié par le Parlement ainsi que la liste de contrôle
---

:::note مستند ماخذ
:::

## Aperçu

Mise en scène du playbook Déploiement Torii Profil de chunker ratifié par le Parlement pour la promotion de la production پہلے تصدیق ہو سکے۔ یہ فرض کرتا ہے کہ SoraFS charte de gouvernance ratifie ہو چکا ہے اور référentiel canonique d'événements

## 1. Prérequis

1. Appareils canoniques et synchronisation des signatures :

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. enveloppes d'admission کی وہ répertoire تیار کریں جسے Torii startup پر پڑھے گا (exemple de chemin) : `/var/lib/iroha/admission/sorafs`.
3. Utilisez le cache de découverte de configuration Torii pour l'application de l'admission et activez le cache :

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

## 2. Les enveloppes d'admission publient کریں

1. Enveloppes d'admission des prestataires agréés `torii.sorafs.discovery.admission.envelopes_dir` Répertoire et copie :

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Redémarrage Torii (rechargement à chaud du chargeur et enroulement et SIGHUP).
3. messages d'admission et queue des journaux:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validation de la propagation de la découverte کریں

1. Le pipeline du fournisseur contient une charge utile d'annonce de fournisseur signée (Norito octets) et une publication :

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Requête de point de terminaison de découverte pour confirmer les alias canoniques de l'annonce et confirmer les noms :

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   یقینی بنائیں کہ `profile_aliases` میں `"sorafs.sf1@1.0.0"` پہلی entrée کے طور پر شامل ہو۔

## 4. Exercice sur les points finaux du plan manifeste

1. Récupération des métadonnées du manifeste (application de l'admission et du jeton de flux):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. La sortie JSON inspecte les éléments et vérifie les éléments :
   - `chunk_profile_handle`, `sorafs.sf1@1.0.0` et
   - Rapport de déterminisme `manifest_digest_hex` et correspondance avec
   - Appareils régénérés `chunk_digests_blake3` et aligner ہوں۔

## 5. Vérifications de télémétrie

- Les métriques de profil de type Prometheus exposent les informations suivantes :

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- tableaux de bord et fournisseur de transfert alias attendu pour les compteurs de baisses de tension et le profil pour les compteurs de baisses de tension

## 6. Préparation au déploiement

1. URL, ID de manifeste, instantané de télémétrie et capture d'écran
2. Canal de déploiement Nexus et fenêtre d'activation de production planifiée
3. Les parties prenantes signent la liste de contrôle de production (Section 4 de `chunker_registry_rollout_checklist.md`).

Ce playbook comprend la mise en scène du déploiement du chunker/admission et la production et les étapes déterministes.