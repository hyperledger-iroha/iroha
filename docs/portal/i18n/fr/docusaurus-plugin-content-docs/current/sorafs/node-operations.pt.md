---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : opérations de nœud
titre : Le Runbook des opéras ne fonctionne pas
sidebar_label : Le Runbook des opérations ne fonctionne pas
description : Valide un implant embutida de `sorafs-node` dentro do Torii.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Mantenha ambas as versoes syncronizadas ate que o conjunto Sphinx seja retirado.
:::

## Visa général

Ce runbook guide les opérateurs pour la validation d'un implant `sorafs-node` intégré dans le Torii. Cela se connecte directement à l'entrée SF-3 : ciclos pin/fetch, récupération après le retour, rejet de quota et amostragem PoR.

## 1. Pré-requis

- Habilite ou travailleur de stockage em `torii.sorafs.storage` :

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

- Garanta que le processus Torii aura accès à la lecture/écriture à `data_dir`.
- Confirmez qu'il n'y a pas d'annonce de capacité attendue via `GET /v1/sorafs/capacity/state` lorsqu'une déclaration d'enregistrement est effectuée.
- Lorsque le lissage est activé, les tableaux de bord affichent des contadores GiB·heure/PoR bruts et suavizados pour destacar tendances sem jitter ao lado de valeurs instantanées.

### Essai à sec de CLI (facultatif)

Avant l'exportation des points de terminaison HTTP, vous pouvez valider le backend de stockage avec une CLI embarquée.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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
```Les commandes impriment les résumés Norito JSON et récusent les divergences de chunk-profile ou digest, tornando-os uteis pour les contrôles de fumée de CI avant le câblage de Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Ensaio de prova PoR

Les opérateurs peuvent désormais reproduire les artefatos PoR émis par la gouvernance locale avant l'envoi vers Torii. La CLI réutilise le même chemin d'acquisition `sorafs-node`, ce qui signifie que les exécutions locales expliquent exactement les erreurs de validation que l'API HTTP renvoie.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

La commande émet un résumé JSON (résumé du manifeste, identifiant du fournisseur, résumé de la preuve, contagem de amostras et veredito facultatif). Forneca `--manifest-id=<hex>` pour garantir que le manifeste armazenado correspond au résumé du défi, et `--json-out=<path>` lorsque vous souhaitez archiver le curriculum vitae des œuvres d'origine comme preuve de l'auditoire. Incluir `--verdict` permet d'enregistrer le flux complet de configuration → tester → vérifier hors ligne avant de démarrer l'API HTTP.

Ensuite, le Torii est là pour pouvoir récupérer vos objets via HTTP :

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Les points de terminaison sont également des services du travailleur de stockage embauché, portant des tests de fumée de CLI et des sondes de passerelle synchronisées en permanence.

## 2. Cyclo Pin → Récupérer1. Obtenez un bundle de manifeste + charge utile (par exemple avec `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Envie du manifeste avec la base de codage64 :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   Le JSON requis doit contenir `manifest_b64` et `payload_b64`. Une réponse de réussite renvoyée `manifest_id_hex` et le résumé de la charge utile.
3. Vérifiez les données réparées :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Décodez le champ `data_b64` en base64 et vérifiez qu'il correspond aux octets d'origine.

## 3. Exercice de récupération après récupération

1. Fixe pelo menos um manifest como acima.
2. Réinitialisez le processus Torii (ou pas intérieurement).
3. Revenir à la demande de récupération. La charge utile doit être continuellement disponible et le résumé renvoyé doit coïncider avec la valeur antérieure au reinicio.
4. Inspecione `GET /v1/sorafs/storage/state` pour confirmer que `bytes_used` reflète les manifestes persistants après le redémarrage.

## 4. Test de rejet de quota

1. Réduire temporairement `torii.sorafs.storage.max_capacity_bytes` pour une petite valeur (par exemple, le tamanho d'un manifeste unique).
2. Corriger un manifeste ; a requisicao deve ter successo.
3. Essayez de fixer un deuxième manifeste de tamanho semelhante. Le Torii doit refuser la demande de HTTP `400` et un message d'erreur concernant le `storage capacity exceeded`.
4. Restaurer la limite de capacité normale avant de finaliser.

## 5. Sonde d'amorage PoR

1. Corrigez le manifeste.
2. Sollicitez un PoR amostra :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```3. Vérifiez votre réponse concernant `samples` à la demande de contagion et se prova valida contre la raison du manifeste armazenado.

## 6. Ganchos de automacão

- Les tests CI/fumée peuvent être réutilisés comme vérifications indiquées dans les instructions :

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  qui cobrem `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` et `por_sampling_returns_verified_proofs`.
- Les tableaux de bord sont développés pour accompagner :
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` et `torii_sorafs_storage_fetch_inflight`
  - contadores de successo/falha PoR expostos via `/v1/sorafs/capacity/state`
  - tentativas de publicacao de règlement via `sorafs_node_deal_publish_total{result=success|failure}`

Suivez ces exercices garantissant que le travailleur de stockage embutido consiga ingerir dados, sobreviver a reinicios, respeitar quotas configuradas e gerar provas PoR déterministicas avant de o no anunciar capacité para a rede plus ample.