---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS کا فوری آغاز

یہ عملی رہنمائی SoraFS اسٹوریج پائپ لائن کی بنیاد بننے والے
детерминистический SF-1, предназначенный для детерминированного режима SF-1.
принести فلو پر لے جاتی ہے۔ Доступ к интерфейсу командной строки и использованию интерфейса командной строки
[конвейер манифеста может быть отключен](manifest-pipeline.md)

## ضروریات

- Rust в списке (`rustup update`) и рабочее пространство для рабочего стола.
- Сообщение: [OpenSSL с парой ключей Ed25519](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  مینی فیسٹ سائن کرنے کے لیے۔
- Условия: Node.js ≥ 18 или Docusaurus, если требуется

`export RUST_LOG=info` Доступен интерфейс командной строки для управления интерфейсом CLI.

## 1. Детерминированные светильники تازہ کریں

SF-1 Канонические векторы фрагментации جب `--signing-key` فراہم کیا
جائے تو یہ کمانڈ подписанные конверты манифеста بھی بناتی ہے؛ `--allow-unsigned` صرف
مقامی ڈیولپمنٹ میں استعمال کریں۔

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Ответ:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (اگر سائن ہوا ہو)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Размер полезной нагрузки и размер фрагмента

`sorafs_chunker` С помощью фрагмента можно получить:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Ответ:

- `profile` / `break_mask` – `sorafs.sf1@1.0.0` کے پیرامیٹرز کی تصدیق۔
- `chunks[]` – учитывает смещения, длину и дайджесты фрагментов BLAKE3.

Приспособления для тестирования и регрессии для потоковой передачи и пакетной обработки.
Разделение на части:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Манифест بنائیں اور سائن کریں

План фрагментов, псевдонимы и подписи управления `sorafs-manifest-stub` или `sorafs-manifest-stub`.
проявить میں لپیٹیں۔ Однофайловая полезная нагрузка или однофайловая полезная нагрузка درخت پیک کرنے
Путь к каталогу или путь к каталогу (CLI или лексикографический интерфейс, или лексикографический интерфейс)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` Дополнительные сведения:

- `chunking.chunk_digest_sha3_256` – смещения/длины, дайджест SHA3, приспособления для блоков и т. д.
- `manifest.manifest_blake3` – конверт манифеста, указанный в дайджесте BLAKE3.
- `chunk_fetch_specs[]` – оркестраторы могут получить доступ к ہدایات۔

جب حقیقی подписи دینے کے لیے تیار ہوں تو `--signing-key` اور `--signer` аргументы شامل
کریں۔ Конверт لککھنے سے پہلے ہر Ed25519 подпись کی توثیق کرتی ہے۔

## 4. Поиск от нескольких поставщиков

разработчик получает CLI, план фрагментов, поставщики услуг, воспроизведение воспроизведения یہ КИ
дымовые тесты اور оркестратор прототипирования کے لیے بہترین ہے۔

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Ответ:

- `payload_digest_hex` манифест رپورٹ سے ملنا چاہیے۔
- `provider_reports[]` ہر поставщик کے لیے успех/неудача засчитывается دکھاتا ہے۔
- غیر صفر `chunk_retry_total` Противодавление ایڈجسٹمنٹ دکھاتا ہے۔
- `--max-peers=<n>` کے ذریعے run میں شیڈول ہونے کی تعداد محدود کریں اور CI
  Симуляторы, которые можно использовать для моделирования
- `--retry-budget=<n>` количество повторов для каждого фрагмента (3) для переопределения количества неудачных попыток внедрения
  Использование регрессий оркестратора

`--expect-payload-digest=<hex>` اور `--expect-payload-len=<bytes>` شامل کریں تاکہ реконструировано
полезная нагрузка или манифест سے ہٹے تو فوراً error ہو جائے۔

## 5. Дополнительная информация- **Интеграция управления** – дайджест манифеста اور `manifest_signatures.json` کو Council
  рабочий процесс میں بھیجیں تاکہ Доступность реестра контактов
- **Согласование реестра**
  [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  دیکھیں۔ Можно использовать числовые идентификаторы и канонические дескрипторы (`namespace.name@semver`).
  ترجیح دینی چاہیے۔
- **Автоматизация CI** – Узнайте больше о конвейерах выпуска и документации по ним.
  светильники, артефакты, подписанные метаданные, детерминированные манифесты, детерминированные манифесты.