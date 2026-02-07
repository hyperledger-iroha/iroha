---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : opérations de nœud
titre : Runbook des opérations du nœud
sidebar_label : Runbook des opérations du nœud
description : Validez le despligue intégré de `sorafs-node` à l'intérieur de Torii.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Mantén ambas versionses sincronizadas hasta que se retire el conjunto de Sphinx.
:::

## CV

Ce runbook guide les opérateurs pour la validation d'un fichier `sorafs-node` intégré dans Torii. Cette section correspond directement aux éléments SF-3 : enregistrements de broches/récupérations, récupération à l'intérieur, récupération par pièce et utilisation de PoR.

## 1. Prérequis

- Habilita le travailleur de stockage en `torii.sorafs.storage` :

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

- Assurez-vous que le processus Torii donne accès à la lecture/écriture à `data_dir`.
- Confirmez que le nœud annonce la capacité attendue via `GET /v1/sorafs/capacity/state` une fois que vous avez enregistré une déclaration.
- Lorsque le suivi est autorisé, les tableaux de bord exposent tant les contadores GiB·hour/PoR en brut que les suivis pour rétablir les tendances sans gigue en même temps que les valeurs instantanées.

### Exécution en seconde partie de CLI (facultatif)

Avant les points de terminaison d'exponer HTTP, vous pouvez effectuer une vérification rapide du backend de stockage avec la CLI intégrée.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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
```Les commandes impriment les résultats Norito JSON et détectent les écarts de profil de chunk ou de digest, c'est pourquoi ils ont des outils pour les contrôles de fumée de CI avant de câbler Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Ensayo de pruebas PoR

Les opérateurs peuvent désormais reproduire les artefacts PoR émis par l'autorité locale avant de subir le Torii. La CLI réutilise la même route d'acquisition `sorafs-node`, car les exécutions locales exposent exactement les erreurs de validation provoquées par l'API HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

La commande émet un résumé JSON (résumé du manifeste, identifiant du fournisseur, résumé de l'essai, nombre de musiciens et résultat de la vérification facultative). Proporciona `--manifest-id=<hex>` pour garantir que le manifeste almacenado coïncide avec le résumé du desafío, et `--json-out=<path>` lorsque vous souhaitez archiver le curriculum vitae avec les artefacts originaux comme preuve de l'auditoire. Incluez `--verdict` pour permettre d'essayer tout le flux de travail → vérifier → vérifier hors ligne avant d'appeler l'API HTTP.

Une fois que Torii est activé, vous pouvez récupérer tous vos artefacts via HTTP :

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Ambos endpoints est servi par le travailleur de l'environnement intégré, ainsi que les tests de fumée de CLI et les sondes de la passerelle synchronisées en permanence.## 2. Recorrido Pin → Récupérer

1. Générez un paquet de manifeste + charge utile (par exemple avec `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Envoyez le manifeste avec la codification base64 :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   Le JSON de la demande doit contenir `manifest_b64` et `payload_b64`. Une réponse de sortie doit être `manifest_id_hex` et le résumé de la charge utile.
3. Récupérer les données enregistrées :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Décodez en base64 le champ `data_b64` et vérifiez que cela coïncide avec les octets originaux.

## 3. Simulation de récupération après le retour

1. Fija al menos un manifesteto como arriba.
2. Réinitialisez le processus Torii (ou le nœud complet).
3. Envoyez la demande de récupération. La charge utile doit être récupérable et le résumé doit coïncider avec la valeur précédente au reinicio.
4. Inspecciona `GET /v1/sorafs/storage/state` pour confirmer que `bytes_used` reflète les manifestes persistants pendant le règne.

## 4. Prueba de rechazo por cuota

1. Réduire temporellement `torii.sorafs.storage.max_capacity_bytes` à une petite valeur (par exemple la taille d'un manifeste solo).
2. Fija un manifeste; la sollicitude doit être réussie.
3. Essayez de faire un deuxième manifeste de taille similaire. Torii doit recevoir la sollicitude avec HTTP `400` et un message d'erreur incluant `storage capacity exceeded`.
4. Restaurez la limite de capacité normale lors de la finalisation.

## 5. Sonde de musique PoR

1. Fija un manifeste.
2. Solliciter un événement PoR :```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Vérifiez que la réponse contient `samples` avec le contenu sollicité et que chaque essai est valide contre le rayon du manifeste enregistré.

## 6. Outils d'automatisation

- Les tests CI/fumée peuvent réutiliser les vérifications dirigidas añadidas en :

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  que cubren `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` et `por_sampling_returns_verified_proofs`.
- Les tableaux de bord doivent suivre :
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` et `torii_sorafs_storage_fetch_inflight`
  - Contadores de exito/fallo de PoR expuestos via `/v1/sorafs/capacity/state`
  - intentions de publication de règlement via `sorafs_node_deal_publish_total{result=success|failure}`

Assurez-vous que ces travaux garantissent que le travailleur de l'exploitation intégré peut ingérer des données, faire revivre les reins, respecter les paramètres configurés et générer des tests pour déterminer la capacité avant que le nœud annonce la capacité du rouge le plus étendu.