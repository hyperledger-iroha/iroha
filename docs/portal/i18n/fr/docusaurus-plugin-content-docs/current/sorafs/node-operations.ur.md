---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : opérations de nœud
titre : نوڈ آپریشنز رن بک
sidebar_label : نوڈ آپریشنز رن بک
description: Torii Pièce de rechange `sorafs-node` Pièce de rechange
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_node_ops.md` کی عکاسی کرتا ہے۔ Il s'agit d'un Sphinx qui est en train de s'en sortir avec un ami. رکھیں۔
:::

## جائزہ

یہ رن بک آپریٹرز کو Torii کے اندر ایمبیڈڈ `sorafs-node` ڈیپلائمنٹ کی توثیق میں
رہنمائی کرتی ہے۔ Voici les livrables du SF-3 : broches/récupération
راؤنڈ ٹرپس، ری اسٹارٹ ریکوری، کوٹا ریجیکشن، اور PoR سیمپلنگ۔

## 1. پیشگی تقاضے

- `torii.sorafs.storage` میں اسٹوریج ورکر کو فعال کریں:

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

- Le lecteur Torii est en lecture/écriture et le lecteur `data_dir` est en lecture/écriture.
- déclaration ریکارڈ ہونے کے بعد `GET /v1/sorafs/capacity/state` کے ذریعے تصدیق
  کریں کہ نوڈ متوقع کپیسٹی اعلان کرتا ہے۔
- Le lissage des valeurs est brut et lissé GiB·heure/PoR.
  Des valeurs ponctuelles sans gigue et des valeurs ponctuelles sont prises en compte par les utilisateurs.

### CLI ڈرائی رن (اختیاری)

Points de terminaison HTTP pour le backend et la CLI groupée
contrôle d'intégrité et vérification de l'intégrité【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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
```Il existe des résumés JSON Norito et une incompatibilité de résumé de profil de bloc
Câblage Torii et contrôles de fumée CI pour les contrôles de fumée
ہیں۔【crates/sorafs_node/tests/cli.rs#L1】

### PoR پروف کی مشق

Il s'agit d'artefacts PoR tels que Torii pour les artefacts PoR.
پہلے لوکل طور پر ری پلے کر سکتے ہیں۔ CLI pour l'ingestion `sorafs-node` et la réutilisation
Il y a beaucoup d'erreurs de validation et d'erreurs de validation avec l'API HTTP
لوٹاتی ہے۔

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Le résumé JSON émet un résumé du manifeste, un identifiant du fournisseur, un résumé de la preuve.
nombre d'échantillons, et résultat facultatif du verdict)۔ `--manifest-id=<hex>` فراہم کریں تاکہ
اسٹور شدہ manifeste چیلنج digest سے میچ کرے، اور `--json-out=<path>` استعمال کریں
Il s'agit d'un résumé des artefacts et des éléments probants d'audit.
چاہیں۔ `--verdict` Installer l'API HTTP pour créer un lien vers l'API → پروف
→ verdict لوپ کی پوری مشق کر سکتے ہیں۔

Torii vous permet de créer des artefacts HTTP et de créer des liens :

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Voici les points de terminaison pour les tests de fumée CLI et les tests de fumée CLI.
sondes de passerelle

## 2. Épingler → Récupérer راؤنڈ ٹرپ1. manifeste + charge utile بنڈل تیار کریں (مثلاً
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` کے ذریعے)۔
2. manifeste et encodage base64 pour votre texte :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   request JSON میں `manifest_b64` اور `payload_b64` شامل ہونا چاہیے۔ کامیاب
   réponse `manifest_id_hex` et résumé de la charge utile et réponse
3. épinglé ڈیٹا chercher کریں :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   `data_b64` décode en base64 les bits et les octets de base de données `data_b64`

## 3. ری اسٹارٹ ریکوری ڈرل

1. اوپر دیے گئے طریقے کے مطابق کم از کم ایک manifeste pin کریں۔
2. Torii پروسس (یا پورا نوڈ) ری اسٹارٹ کریں۔
3. Récupérez les fichiers ریکوئسٹ دوبارہ جمع کریں۔ payload بدستور دستیاب رہے اور واپس آنے والا
   digest ری اسٹارٹ سے پہلے والی قدر کے مطابق ہو۔
4. `GET /v1/sorafs/storage/state` چیک کریں تاکہ تصدیق ہو کہ `bytes_used` ری بوٹ کے
   بعد persiste manifestes کو ظاہر کرتا ہے۔

## 4. کوٹا ریجیکشن ٹیسٹ

1. عارضی طور پر `torii.sorafs.storage.max_capacity_bytes` کو چھوٹی قدر پر لائیں
   (مثلاً ایک manifeste کے سائز کے برابر)۔
2. La broche du manifeste est affichée ریکوئسٹ کامیاب ہونی چاہیے۔
3. La broche du manifeste est ouverte et la broche manifeste est ouverte. Torii en stock
   HTTP `400` est connecté à Internet et est connecté à Internet `storage capacity exceeded`
   شامل ہونا چاہیے۔
4. ختم ہونے پر نارمل کپیسٹی حد بحال کریں۔

## 5. PoR سیمپلنگ پروب

1. La broche du manifeste est affichée
2. Échantillon PoR en cours :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```3. تصدیق کریں کہ réponse میں `samples` مطلوبہ count کے ساتھ موجود ہیں اورہر
   preuve اسٹور شدہ racine manifeste کے خلاف ویریفائی ہوتا ہے۔

## 6. آٹومیشن ہکس

- Tests CI/fumée pour les tests de dépistage des fumées :

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  par `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`, et
  `por_sampling_returns_verified_proofs` pour le client
- ڈیش بورڈز کو یہ ٹریک کرنا چاہیے:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` et `torii_sorafs_storage_fetch_inflight`
  - `/v1/sorafs/capacity/state` کے ذریعے ظاہر کیے گئے PoR کامیابی/ناکامی کاؤنٹرز
  - `sorafs_node_deal_publish_total{result=success|failure}` pour les tentatives de publication de règlement

ان forets کی پیروی سے یہ یقینی ہوتا ہے کہ ایمبیڈڈ اسٹوریج ورکر ڈیٹا ingérer کر
سکے، ری اسٹارٹس برداشت کرے، مقررہ کوٹاز کا احترام کرے، اور نوڈ کے وسیع نیٹ ورک
کو کپیسٹی publicité pour les preuves PoR تیار کرے۔