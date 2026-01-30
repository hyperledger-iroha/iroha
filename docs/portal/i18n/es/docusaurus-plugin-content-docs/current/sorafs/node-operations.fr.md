---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: node-operations
title: Runbook d’exploitation du nœud
sidebar_label: Runbook d’exploitation du nœud
description: Valider le déploiement embarqué de `sorafs-node` dans Torii.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Gardez les deux versions synchronisées jusqu’à ce que l’ensemble Sphinx soit retiré.
:::

## Vue d’ensemble

Ce runbook guide les opérateurs pour valider un déploiement `sorafs-node` embarqué dans Torii. Chaque section correspond directement aux livrables SF-3 : boucles pin/fetch, reprise après redémarrage, rejet de quotas et échantillonnage PoR.

## 1. Prérequis

- Activez le worker de stockage dans `torii.sorafs.storage` :

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Assurez-vous que le processus Torii dispose d’un accès lecture/écriture à `data_dir`.
- Confirmez que le nœud annonce la capacité attendue via `GET /v1/sorafs/capacity/state` une fois la déclaration enregistrée.
- Lorsque le lissage est activé, les dashboards exposent à la fois les compteurs GiB·hour/PoR bruts et lissés afin de mettre en évidence des tendances sans jitter à côté des valeurs instantanées.

### Exécution à blanc de la CLI (optionnelle)

Avant d’exposer les endpoints HTTP, vous pouvez valider le backend de stockage avec la CLI fournie.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

Les commandes impriment des résumés Norito JSON et refusent les divergences de profil de chunk ou de digest, ce qui les rend utiles pour des smoke checks CI avant le câblage de Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Répétition de preuve PoR

Les opérateurs peuvent désormais rejouer localement les artefacts PoR émis par la gouvernance avant de les téléverser vers Torii. La CLI réutilise le même chemin d’ingestion `sorafs-node`, de sorte que les exécutions locales exposent exactement les erreurs de validation que l’API HTTP renverrait.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

La commande émet un résumé JSON (digest de manifest, id fournisseur, digest de preuve, nombre d’échantillons, verdict optionnel). Fournissez `--manifest-id=<hex>` pour garantir que le manifest stocké correspond au digest du challenge, et `--json-out=<path>` si vous souhaitez archiver le résumé avec les artefacts d’origine comme preuve d’audit. Inclure `--verdict` permet de répéter l’ensemble du cycle challenge → proof → verdict en local avant d’appeler l’API HTTP.

Une fois Torii en ligne, vous pouvez récupérer les mêmes artefacts via HTTP :

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Les deux endpoints sont servis par le worker de stockage embarqué, afin que les smoke tests CLI et les sondes de gateway restent alignés.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Boucle Pin → Fetch

1. Produisez un bundle manifest + payload (par exemple via `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Soumettez le manifest en base64 :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   Le JSON de requête doit contenir `manifest_b64` et `payload_b64`. Une réponse réussie renvoie `manifest_id_hex` et le digest du payload.
3. Récupérez les données épinglées :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Décoder en base64 le champ `data_b64` et vérifiez qu’il correspond aux octets originaux.

## 3. Drill de reprise après redémarrage

1. Épinglez au moins un manifest comme ci-dessus.
2. Redémarrez le processus Torii (ou le nœud complet).
3. Renvoyez la requête fetch. Le payload doit rester récupérable et le digest renvoyé doit correspondre à la valeur d’avant redémarrage.
4. Inspectez `GET /v1/sorafs/storage/state` pour confirmer que `bytes_used` reflète les manifests persistés après le reboot.

## 4. Test de rejet de quota

1. Abaissez temporairement `torii.sorafs.storage.max_capacity_bytes` à une valeur faible (par exemple la taille d’un seul manifest).
2. Épinglez un manifest ; la requête doit réussir.
3. Tentez d’épingler un second manifest de taille similaire. Torii doit rejeter la requête avec HTTP `400` et un message d’erreur contenant `storage capacity exceeded`.
4. Restaurez la limite de capacité normale après le test.

## 5. Sondage d’échantillonnage PoR

1. Épinglez un manifest.
2. Demandez un échantillon PoR :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Vérifiez que la réponse contient `samples` avec le nombre demandé et que chaque preuve valide contre la racine du manifest stocké.

## 6. Hooks d’automatisation

- Les tests CI / smoke peuvent réutiliser les contrôles ciblés ajoutés dans :

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  qui couvrent `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` et `por_sampling_returns_verified_proofs`.
- Les dashboards doivent suivre :
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` et `torii_sorafs_storage_fetch_inflight`
  - les compteurs de succès/échec PoR exposés via `/v1/sorafs/capacity/state`
  - les tentatives de publication de settlement via `sorafs_node_deal_publish_total{result=success|failure}`

Suivre ces drills garantit que le worker de stockage embarqué peut ingérer des données, survivre aux redémarrages, respecter les quotas configurés et générer des preuves PoR déterministes avant que le nœud n’annonce sa capacité au reste du réseau.
