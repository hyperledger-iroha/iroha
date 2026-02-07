---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de nó
título: نوڈ آپریشنز رن بک
sidebar_label: Nome da barra lateral
description: Torii کے اندر ایمبیڈڈ `sorafs-node` ڈیپلائمنٹ کی توثیق کریں۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_node_ops.md` کی عکاسی کرتا ہے۔ جب تک پرانا Esfinge ڈاکیومنٹیشن سیٹ مکمل طور پر منتقل نہ ہو جائے, دونوں نقول کو ہم آہنگ رکھیں۔
:::

## جائزہ

یہ رن بک آپریٹرز کو Torii کے اندر ایمبیڈڈ `sorafs-node` ڈیپلائمنٹ کی توثیق میں
رہنمائی کرتی ہے۔ ہر سیکشن براہِ راست SF-3 entregas سے میپ ہوتا ہے: pin/fetch
راؤنڈ ٹرپس, ری اسٹارٹ ریکوری, کوٹا ریجیکشن, اور PoR سیمپلنگ۔

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

- یقینی بنائیں کہ Torii پروسس کے پاس `data_dir` تک leitura/gravação رسائی ہو۔
- declaração ریکارڈ ہونے کے بعد `GET /v1/sorafs/capacity/state` کے ذریعے تصدیق
  کریں کہ نوڈ متوقع کپیسٹی اعلان کرتا ہے۔
- جب suavização فعال ہو، ڈیش بورڈز cru ou suavizado GiB·hora/PoR کاؤنٹرز دونوں
  دکھاتے ہیں تاکہ sem jitter رجحانات کو valores spot کے ساتھ نمایاں کیا جا سکے۔

### CLI ڈرائی رن (اختیاری)

Endpoints HTTP ایکسپوز کرنے سے پہلے آپ CLI empacotado کے ذریعے اسٹوریج backend کا
verificação de sanidade کر سکتے ہیں۔【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

یہ کمانڈز Norito Resumos JSON پرنٹ کرتی ہیں اور chunk-profile یا digest incompatibilidade
کو مسترد کرتی ہیں, جس سے یہ Fiação Torii سے پہلے Verificações de fumaça CI کے لیے مفید بنتی
ہیں۔【crates/sorafs_node/tests/cli.rs#L1】

### PoR پروف کی مشق

آپریٹرز اب گورننس کی طرف سے جاری کردہ Artefatos PoR کو Torii پر اپ لوڈ کرنے سے
پہلے لوکل طور پر ری پلے کر سکتے ہیں۔ CLI اسی `sorafs-node` ingestão e reutilização
Erros de validação ظاہر کرتے ہیں جو API HTTP واپس
لوٹاتی ہے۔

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

کمانڈ ایک Resumo JSON emit کرتی ہے (resumo do manifesto, ID do provedor, resumo da prova,
contagem de amostra, e resultado do veredicto opcional)۔ `--manifest-id=<hex>` فراہم کریں تاکہ
اسٹور شدہ manifest چیلنج digest سے میچ کرے, اور `--json-out=<path>` استعمال کریں
جب آپ resumo کو اصل artefatos کے ساتھ evidência de auditoria کے طور پر آرکائیو کرنا
چاہیں۔ `--verdict` é uma API HTTP que pode ser configurada para a API HTTP → پروف
→ veredicto لوپ کی پوری مشق کر سکتے ہیں۔

Torii لائیو ہونے کے بعد آپ وہی artefatos HTTP کے ذریعے حاصل کر سکتے ہیں:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Outros endpoints ایمبیڈڈ اسٹوریج ورکر فراہم کرتا ہے، اس لیے CLI smoketests اور
sondas de gateway

## 2. Fixar → Buscar راؤنڈ ٹرپ

1. manifesto + carga útil
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` کے ذریعے)۔
2. manifesto com codificação base64 کے ساتھ جمع کریں:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   solicitar JSON como `manifest_b64` e `payload_b64` para obter o valor desejado کامیاب
   resposta `manifest_id_hex` e resumo de carga útil واپس کرتی ہے۔
3. ڈیٹا buscar کریں fixado:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   `data_b64` فیلڈ کو decodificação base64 کریں اور تصدیق کریں کہ یہ اصل bytes سے میچ کرتی ہے۔

## 3. ری اسٹارٹ ریکوری ڈرل1. اوپر دیے گئے طریقے کے مطابق کم از کم ایک pin de manifesto کریں۔
2. Torii پروسس (یا پورا نوڈ) ری اسٹارٹ کریں۔
3. buscar ریکوئسٹ دوبارہ جمع کریں۔ carga útil
   digest ری اسٹارٹ سے پہلے والی قدر کے مطابق ہو۔
4. `GET /v1/sorafs/storage/state` چیک کریں تاکہ تصدیق ہو کہ `bytes_used` ری بوٹ کے
   بعد manifestos persistentes کو ظاہر کرتا ہے۔

## 4. کوٹا ریجیکشن ٹیسٹ

1. Use o `torii.sorafs.storage.max_capacity_bytes` para obter o valor do arquivo
   (مثلاً ایک manifesto کے سائز کے برابر)۔
2. Pin de manifesto کریں؛ ریکوئسٹ کامیاب ہونی چاہیے۔
3. اسی طرح کے سائز والا دوسرا pin de manifesto کرنے کی کوشش کریں۔ Torii کو ریکوئسٹ
   HTTP `400` کے ساتھ مسترد کرنی چاہیے اور ایرر میسج میں `storage capacity exceeded`
   شامل ہونا چاہیے۔
4. ختم ہونے پر نارمل کپیسٹی حد بحال کریں۔

## 5. PoR سیمپلنگ پروب

1. Pin de manifesto کریں۔
2. Exemplo de PoR:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. تصدیق کریں کہ resposta میں `samples` مطلوبہ contagem کے ساتھ موجود ہیں اور ہر
   prova اسٹور شدہ raiz do manifesto کے خلاف ویریفائی ہوتا ہے۔

## 6. آٹومیشن ہکس

- Testes de CI / fumaça

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  Para `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`, E
  `por_sampling_returns_verified_proofs` کو کور کرتے ہیں۔
- ڈیش بورڈز کو یہ ٹریک کرنا چاہیے:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` ou `torii_sorafs_storage_fetch_inflight`
  - `/v1/sorafs/capacity/state` کے ذریعے ظاہر کیے گئے PoR کامیابی/ناکامی کاؤنٹرز
  - `sorafs_node_deal_publish_total{result=success|failure}` کے ذریعے tentativas de publicação de liquidação

ان brocas کی پیروی سے یہ یقینی ہوتا ہے کہ ایمبیڈڈ اسٹوریج ورکر ڈیٹا ingerir کر
سکے, ری اسٹارٹس برداشت کرے, مقررہ کوٹاز کا احترام کرے, اور نوڈ کے وسیع نیٹ ورک
کو کپیسٹی anunciar کرنے سے پہلے ڈٹرمنسٹک Provas PoR تیار کرے۔