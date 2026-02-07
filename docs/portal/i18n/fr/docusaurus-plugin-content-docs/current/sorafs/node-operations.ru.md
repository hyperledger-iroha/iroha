---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : opérations de nœud
titre : Ранбук операций узла
sidebar_label : Ранбук операций узла
description : Proverberka встроенного деплоя `sorafs-node` внутри Torii.
---

:::note Канонический источник
Cette page correspond à `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Vous pouvez utiliser des copies de synchronisation pour créer des documents sur Sphinx et vous n'en avez pas besoin.
:::

## Обзор

Ce projet fournit aux opérateurs la preuve du déploiement `sorafs-node` à l'intérieur
Torii. Каждый раздел напрямую соответствует livrables SF-3 : broche/extraction de lampe,
Après l'achat, vous devrez acheter des billets et utiliser le PoR.

## 1. Votre hôtel précédent

- Voir le travailleur de stockage dans `torii.sorafs.storage` :

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

- Pour que le processus Torii soit téléchargé/installé sur `data_dir`.
- Подтвердите, что узел объявляет ожидаемую ёмкость через
  `GET /v1/sorafs/capacity/state` suite à cette déclaration.
- Lors de l'ouverture des frontières du bord, vous pourrez trouver le sang, ainsi que le liquide.
  счётчики GiB·hour/PoR, чтобы подчёркивать тренды без джиттера рядом с
  мгновенными значениями.

### Problème de démarrage de la CLI (officiellement)

Avant l'ouverture des ports HTTP, vous pouvez prouver la fonctionnalité backend de vérification de l'intégrité du backend
Utilisez la CLI.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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
```Les commandes permettent d'obtenir le code JSON Norito et de modifier les paramètres de profil ou
digest, ce qui est prévu pour les solutions anti-fumée CI avant la publication Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Répétition PoR-доказательства

Les opérateurs теперь могут локально воспроизводить артефакты PoR, выпущенные gouvernance,
avant de les télécharger dans Torii. La CLI permet d'ingérer `sorafs-node`, ainsi
Ce sont des programmes locaux qui vous permettent de valider, via l'API HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

La commande affiche le fichier JSON (digest manifesta, id провайдера, digest доказательства,
количество сэмплов, опциональный результат verdict). Prenez `--manifest-id=<hex>`,
чтобы убедиться, что сохранённый manifeste совпадает с digest вызова, и
`--json-out=<path>`, vous ne pouvez pas archiver votre CV avec vos articles d'art
как доказательство для аудита. La mise à jour `--verdict` permet de désactiver le courant
Cliquez sur → Documentation → Consultez la page d'accueil de l'API HTTP.

Après avoir téléchargé Torii, vous pouvez utiliser les articles correspondant à HTTP :

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Pour une entreprise travaillant dans le secteur du stockage, des tests de fumée CLI et
La passerelle est actuellement synchronisée.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Cliquez sur la broche → Récupérer1. Formez le manifeste du paquet + la charge utile (par exemple, avec cela).
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Ouvrez le manifeste dans le code base64 :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   JSON doit utiliser `manifest_b64` et `payload_b64`. Réponse claire
   возвращает `manifest_id_hex` et digest payload.
3. Получите закреплённые данные:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Décodez le pôle `data_b64` en base64 et vérifiez qu'il est uniquement compatible avec vos batteries.

## 3. Entraînement après la mise en service

1. Prenez le minimum de manifeste, comme vous le souhaitez.
2. Exécutez le processus Torii (ou votre utilisation).
3. Повторно отправьте запрос fetch. La charge utile est maintenant disponible, un résumé dans
   ответе должен совпасть с предшествующим перезапуску.
4. Vérifiez le `GET /v1/sorafs/storage/state` pour savoir ce que `bytes_used` indique.
   сохранённые se manifeste après перезагрузки.

## 4. Testez votre achat

1. Vérifiez simplement le `torii.sorafs.storage.max_capacity_bytes` pour le petit détail
   (par exemple, размера одного manifeste).
2. Закрепите один manifeste; запрос должен пройти успешно.
3. Попробуйте закрепить второй manifest сопоставимого размера. Torii pour l'ouverture
   En utilisant HTTP `400` et en utilisant le code `storage capacity exceeded`.
4. Lorsque vous avez besoin d'aide, veuillez vous assurer que le produit est prêt à l'emploi.

## 5. Probab PoR-сэмплинга

1. Закрепите manifeste.
2. Choisissez PoR-выборку :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```3. Vérifiez que vous êtes en train d'afficher `samples` avec le colis et ce que vous avez acheté.
   доказательство валидируется относительно корня сохранённого manifesta.

## 6. Les automatismes

- Les tests CI/fumée peuvent généralement utiliser certaines preuves, adaptées à :

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  porte-clés `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`
  et `por_sampling_returns_verified_proofs`.
- Les Daschbords должны отслеживать:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` et `torii_sorafs_storage_fetch_inflight`
  - счётчики успехов/неудач PoR, публикуемые через `/v1/sorafs/capacity/state`
  - Règlement des publications populaires par `sorafs_node_deal_publish_total{result=success|failure}`

Assurez-vous de bénéficier de la garantie que le travailleur de stockage professionnel a accepté
ingestirovatь данные, переживать перезапуски, соблюдать заданные квоты и генерировать
Déterminez le PoR-доказательства до того, как узел объявит ёмкость широкой сети.