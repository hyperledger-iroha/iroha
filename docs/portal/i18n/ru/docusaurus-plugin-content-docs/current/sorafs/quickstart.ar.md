---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# البدء السريع في SoraFS

Он был убит в фильме "Чанкэр" SF-1.
Он был создан в 1980-х годах, когда был выбран в качестве кандидата на пост президента США. Код SoraFS.
وازنه مع [التعمّق في خط أنابيب المانيفست](manifest-pipeline.md)
Он сказал, что это не так.

## المتطلبات الأساسية

- Установлен Rust (`rustup update`) на сайте разработчика.
- Сообщение: [записано Ed25519 в OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  لتوقيع المانيفستات.
- Условия: Node.js ≥ 18 для версии Docusaurus.

Установите `export RUST_LOG=info` и запустите интерфейс CLI.

## 1. Проверьте светильники الحتمية

أعد توليد متجهات التقسيم (разбивка на части) в SF-1. Настоящая история
Встроенное программное обеспечение `--signing-key`; استخدم `--allow-unsigned` أثناء التطوير
المحلي فقط.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Сообщение:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (إذا تم التوقيع)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Изменение полезной нагрузки

Установите `sorafs_chunker` в исходное состояние:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Ответ на вопрос:

- `profile` / `break_mask` – встроенный `sorafs.sf1@1.0.0`.
- `chunks[]` – можно использовать фрагменты BLAKE3.

Светильники и оборудование
Ответ на вопрос:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. ابنِ ووقّع مانيفست

Чтобы получить куски, вы можете использовать куски кусков, чтобы получить куски мяса.
`sorafs-manifest-stub`. يوضح الأمر أدناه payload لملف واحد؛ مرّر مسار دليل لحزم
Добавлено (открывается CLI).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Код `/tmp/docs.report.json`:

- `chunking.chunk_digest_sha3_256` – поддержка SHA3 для подключения/отключения светильников.
  بالـ чанкёр.
- `manifest.manifest_blake3` – BLAKE3 был создан в 2017 году.
- `chunk_fetch_specs[]` – Защитите себя от вируса.

Стив Дэниел Лёрдс, Дэнни Сейлор, أضف الوسيطين `--signing-key` и `--signer`.
Это сообщение было написано в журнале Ed25519 в рамках проекта.

## 4. حاكِ الاسترجاع متعدد المزوّدين

Используйте CLI, чтобы просмотреть фрагменты, которые нужно удалить.
Он был создан в 1997 году в CI в Нью-Йорке.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Ответ:

- `payload_digest_hex` был отправлен в США.
- `provider_reports[]` может быть отключен от сети.
- Установите `chunk_retry_total` для устранения противодавления.
- Код `--max-peers=<n>` был создан в рамках программы CI. مركّزة
  على المرشحين الأساسيين.
- `--retry-budget=<n>` позволяет установить фрагмент фрагмента (3)
  Он был отправлен в Лондон.

`--expect-payload-digest=<hex>` и `--expect-payload-len=<bytes>` для проверки.
Полезная нагрузка была отправлена ​​в Сан-Франциско.

## 5. Дополнительная информация- ** تكامل الحوكمة** – مرّر بصمة المانيفست و`manifest_signatures.json` إلى سير عمل المجلس
  Реестр контактов Лос-Анджелеса в Вашингтоне.
- **التفاوض مع السجل** – راجع [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  Он был убит в 2007 году. Воспользуйтесь услугами, которые вы можете получить, чтобы получить больше информации.
  (`namespace.name@semver`) Пожалуйста, проверьте.
- **أتمتة CI** – أضف الأوامر أعلاه إلى خطوط إصدار النشر حتى تنشر المستندات والـ светильники
  Он был убит Джоном Джонсом и Беннетом в Нью-Йорке.