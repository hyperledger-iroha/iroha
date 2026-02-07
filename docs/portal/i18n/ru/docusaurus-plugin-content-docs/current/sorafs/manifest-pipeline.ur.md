---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS фрагментирование → конвейер манифеста

Краткое руководство по созданию трубопровода для создания системы Norito
манифесты مواد
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
سے ماخوذ ہے؛ مستند وضاحت اور تبدیلی لاگکے لیے اسی دستاویز سے رجوع کریں۔

## 1. Разделение на части

SoraFS для SF-1 (`sorafs.sf1@1.0.0`) Доступен: FastCDC, установленный на сервере. میں
Размер чанка: 64 КиБ, 256 КиБ, размер файла 512 КиБ, размер `0x0000ffff`
ہے۔ یہ پروفائل `sorafs_manifest::chunker_registry` میں رجسٹر ہے۔

### Ржавчина

- `sorafs_car::CarBuildPlan::single_file` – CAR позволяет использовать куски и смещения,
  لمبائیاں اور BLAKE3 дайджесты جاری کرتا ہے۔
- `sorafs_car::ChunkStore` – полезные нагрузки, которые можно использовать для фрагментов, которые могут быть использованы в дальнейшем.
  64 КиБ / 4 КиБ с доказательством возможности восстановления (PoR)
- `sorafs_chunker::chunk_bytes_with_digests` – Помощник по интерфейсу командной строки

### Интерфейс командной строки

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

Формат JSON и смещения, а также дайджесты фрагментов и дайджесты фрагментов. манифест یا آرکسٹریٹر fetch
اسپیسفیکیشن بناتے وقت اس پلان کو محفوظ رکھیں۔

### PoR گواہیاں

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` اور `--por-sample=<count>` فراہم کرتا ہے
تاکہ آڈیٹرز حتمی گواہی سیٹ مانگ سکیں۔ Флаги: `--por-proof-out` или `--por-sample-out`.
Использование JSON-файлов и файлов JSON

## 2. проявить کو لپیٹنا

`ManifestBuilder` чанки могут быть использованы для следующих целей:

- روٹ CID (dag-cbor) اور CAR обязательства۔
- псевдоним ثبوت اور فراہم کنندہ صلاحیت کے دعوے۔
- Чтобы получить доступ к идентификаторам сборки (необходимые идентификаторы сборки).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

В ответ:

- `payload.manifest` – Norito Манифест манифеста
- `payload.report.json` – انسان/آٹومیشن کیلئے قابلِ فہم خلاصہ, جس میں `chunk_fetch_specs`,
  `payload_digest_hex`, CAR дайджест اور псевдоним میٹا ڈیٹا شامل ہیں۔
- `payload.manifest_signatures.json` – Для получения манифеста дайджеста BLAKE3 и фрагмента необходимо
  Дайджест SHA3 создан в Ed25519 دستخط شامل ہیں۔

`--manifest-signatures-in` Для получения дополнительной информации о том, как сделать это, вы можете использовать لکھنے
سے پہلے تصدیق کیا جا سکے، اور `--chunker-profile-id` یا `--chunker-profile=<handle>` کے ذریعے
رجسٹری انتخاب کو لاک کریں۔

## 3. اشاعت اور پننگ

1. **گورننس میں جمع کرانا** – дайджест манифеста, который поможет вам установить контакт
   منظور ہو سکے۔ Вы можете использовать чанк для дайджеста манифеста SHA3, чтобы получить доступ к нему.
2. **Полезная нагрузка в зависимости от типа** – манифестация соответствия CAR آرکائیو (اور اختیاری CAR انڈیکس)
   Регистрация контактов или регистрация контактов Декларация CAR и CID, декларация CID и т. д.
3. **Отображение данных** – JSON позволяет получить данные PoR ریلیز
   آرٹیفیکٹس میں محفوظ کریں۔ Как получить полезные нагрузки
   Если вы хотите, чтобы вы выбрали лучший вариант для себя,

## 4. ملٹی پروائیڈر fetch سمیولیشن

`пробег груза -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` ہر پرووائیڈر کے لیے параллелизм بڑھاتا ہے (`#4` اوپر)۔
- `@<weight>` شیڈولنگ смещение کو ایڈجسٹ کرتا ہے؛ ڈیفالٹ 1 ہے۔
- `--max-peers=<n>` دریافت میں بہت سے امیدوار آنے پر رن کے لیے منتخب پرووائیڈرز کی تعداد محدود کرتا ہے۔
- `--expect-payload-digest` اور `--expect-payload-len` خاموش کرپشن سے بچاتے ہیں۔
- `--provider-advert=name=advert.to` سمیولیشن سے پہلے پرووائیڈر کی صلاحیتوں کی توثیق کرتا ہے۔
- `--retry-budget=<n>` ہر chunk کی ری ٹرائی تعداد (ڈیفالٹ: 3) بدلتا ہے تاکہ CI ناکامی کے
  منظرناموں میں رگریشنز جلد ظاہر کرے۔

`fetch_report.json` مجموعی میٹرکس (`chunk_retry_total`, `provider_failure_rate` وغیرہ)
Утверждения CI

## 5. رجسٹری اپڈیٹس اور گورننس

Ниже приведены примеры чанкеров:

1. `sorafs_manifest::chunker_registry_data` дескриптор لکھیں۔
2. `docs/source/sorafs/chunker_registry.md` اور متعلقہ чартеры, которые можно использовать
3. светильники (`export_vectors`) دوبارہ جنریٹ کریں اور подписанные манифесты
4. Как обеспечить соблюдение устава?

Если есть канонические дескрипторы (`namespace.name@semver`)
Вы можете получить доступ к идентификаторам и идентификаторам, которые вы хотите использовать.