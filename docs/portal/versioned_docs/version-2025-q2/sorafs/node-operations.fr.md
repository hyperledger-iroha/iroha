---
lang: fr
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T15:38:30.655980+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: node-operations-fr
slug: /sorafs/node-operations-fr
---

:::note Source canonique
Miroirs `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Gardez les deux copies alignées dans les versions.
:::

## Aperçu

Ce runbook guide les opérateurs dans la validation d'un déploiement `sorafs-node` intégré dans Torii. Chaque section correspond directement aux livrables du SF-3 : allers-retours de broche/récupération, redémarrage de la récupération, rejet de quota et échantillonnage PoR.

## 1. Prérequis

- Activez le gestionnaire de stockage dans `torii.sorafs.storage` :

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

- Assurez-vous que le processus Torii dispose d'un accès en lecture/écriture à `data_dir`.
- Confirmez que le nœud annonce la capacité attendue via `GET /v2/sorafs/capacity/state` une fois qu'une déclaration est enregistrée.
- Lorsque le lissage est activé, les tableaux de bord exposent les compteurs GiB·heure/PoR bruts et lissés pour mettre en évidence les tendances sans instabilité aux côtés des valeurs ponctuelles.

### Exécution à sec CLI (facultatif)

Avant d'exposer les points de terminaison HTTP, vous pouvez vérifier l'intégrité du backend de stockage avec la CLI fournie.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Les commandes impriment les résumés JSON Norito et refusent les incompatibilités de profil de bloc ou de résumé, ce qui les rend utiles pour les contrôles de fumée CI avant le câblage Torii. 【crates/sorafs_node/tests/cli.rs#L1】

Une fois Torii actif, vous pouvez récupérer les mêmes artefacts via HTTP :

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Les deux points de terminaison sont servis par le gestionnaire de stockage intégré, de sorte que les tests de fumée CLI et les sondes de passerelle restent synchronisés.

## 2. Épingler → Récupérer l'aller-retour

1. Produisez un bundle manifeste + charge utile (par exemple avec `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Soumettez le manifeste avec l'encodage base64 :

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   La requête JSON doit contenir `manifest_b64` et `payload_b64`. Une réponse réussie renvoie `manifest_id_hex` et le résumé de la charge utile.
3. Récupérez les données épinglées :

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Décodez en base64 le champ `data_b64` et vérifiez qu'il correspond aux octets d'origine.

## 3. Redémarrez l'exercice de récupération

1. Épinglez au moins un manifeste comme ci-dessus.
2. Redémarrez le processus Torii (ou le nœud entier).
3. Soumettez à nouveau la demande de récupération. La charge utile doit toujours être récupérable et le résumé renvoyé doit correspondre à la valeur de pré-redémarrage.
4. Inspectez `GET /v2/sorafs/storage/state` pour confirmer que `bytes_used` reflète les manifestes persistants après le redémarrage.

## 4. Test de rejet de quota

1. Réduisez temporairement `torii.sorafs.storage.max_capacity_bytes` à une petite valeur (par exemple la taille d'un seul manifeste).
2. Épinglez un manifeste ; la demande devrait aboutir.
3. Essayez d'épingler un deuxième manifeste de taille similaire. Torii doit rejeter la demande avec HTTP `400` et un message d'erreur contenant `storage capacity exceeded`.
4. Restaurez la limite de capacité normale une fois terminé.

## 5. Sonde d'échantillonnage PoR

1. Épinglez un manifeste.
2. Demandez un échantillon PoR :

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Vérifiez que la réponse contient `samples` avec le nombre demandé et que chaque preuve est validée par rapport à la racine du manifeste stockée.

## 6. Crochets d'automatisation

- Les tests CI/fumée peuvent réutiliser les contrôles ciblés ajoutés dans :

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```qui couvre `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` et `por_sampling_returns_verified_proofs`.
- Les tableaux de bord doivent suivre :
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  -`torii_sorafs_storage_pin_queue_depth` et `torii_sorafs_storage_fetch_inflight`
  - Compteurs de réussite/échec PoR apparus via `/v2/sorafs/capacity/state`
  - Tentatives de publication de règlement via `sorafs_node_deal_publish_total{result=success|failure}`

Suivre ces exercices garantit que l'opérateur de stockage intégré peut ingérer des données, survivre aux redémarrages, respecter les quotas configurés et générer des preuves PoR déterministes avant que le nœud n'annonce sa capacité au réseau plus large.
