---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تجزئة SoraFS → مسار المانيفست

В 1980-х годах в Лос-Анджелесе было объявлено о том, что в 1990-х годах он был рожден в Лос-Анджелесе. الخام إلى
Введите Norito. Откройте реестр контактов SoraFS. المحتوى مقتبس من
[`docs/source/sorafs/manifest_pipeline.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
Он сделал это для того, чтобы сделать это.

## 1. تجزئة حتمية

Код SoraFS для SF-1 (`sorafs.sf1@1.0.0`): хеш-код создается в FastCDC в режиме FastCDC.
Размер фрагмента: 64 КиБ, 256 КиБ, размер 512 КиБ, код `0x0000ffff`. الملف
Код для `sorafs_manifest::chunker_registry`.

### مساعدات Rust

- `sorafs_car::CarBuildPlan::single_file` – добавление кусков для BLAKE3 أثناء
  Купите автомобиль в автомобиле.
- `sorafs_car::ChunkStore` – блокировка полезных данных и потоковая передача, а также фрагменты фрагментов
  Доказательство возможности восстановления (PoR) Размер: 64 КиБ / 4 КиБ.
- `sorafs_chunker::chunk_bytes_with_digests` – используется для работы с CLI.

### Управление CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

Используйте JSON для создания и редактирования фрагментов. احتفظ بالخطة عند بناء المانيفستات
Тогда он принесет вам сказку.

### شواهد PoR

تُتيح `ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` و`--por-sample=<count>` حتى
Он был убит в 2017 году в Скарлетт-Джонсе. قرن هذه الأعلام مع `--por-proof-out` أو
`--por-sample-out` — файл JSON.

## 2. تغليف مانيفست

Для `ManifestBuilder` можно использовать фрагменты данных для проверки:

- CID الجذر (dag-cbor) وتعهدات CAR.
- Тэхен, псевдоним Найтли قدرات المزوّدين.
- Сделано в Нижнем Новгороде (сборка в стиле хай-тек).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Сообщение:

- `payload.manifest` – Вы можете установить Norito.
- `payload.report.json` – установите флажок `chunk_fetch_specs` و
  `payload_digest_hex` назвал CAR псевдонимом الوصفية.
- `payload.manifest_signatures.json` – используется для подключения BLAKE3 и SHA3 لخطة.
  куски, см. Ed25519.

استخدم `--manifest-signatures-in` للتحقق من الأظرف القادمة موقّعين خارجيين قبل إعادة
Установите `--chunker-profile-id` и `--chunker-profile=<handle>` для проверки.

## 3. Значок кнопки (булавка)

1. **Убийство ** – Убийство британского министра иностранных дел в Вашингтоне Значок يمكن قبول الـ.
   Для этого необходимо использовать SHA3 с фрагментами данных.
2. **Полезная нагрузка** – ارفع أرشيف CAR (وفهرس CAR الاختياري)
   Реестр контактов. Он был совершен в полиции и в CAR в отделе уголовного розыска США.
3. **Обработка изображений** – получение данных в формате JSON для PoR и поиск артефактов الإصدار.
   Он был выбран в качестве посредника в создании нового образа жизни. Дэн Сонделл
   полезные нагрузки.

## 4. Получить сообщение об ошибке

`пробег груза -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` находится в зоне действия (`#4` أعلاه).
- `@<weight>` يضبط انحياز الجدولة؛ Сделай это в 1.
- `--max-peers=<n>` был создан в 2017 году в 1997 году. Он родился в Вашингтоне.
- `--expect-payload-digest` و`--expect-payload-len` находится в зоне действия.
- `--provider-advert=name=advert.to` был создан в 2017 году в Колумбии.
- `--retry-budget=<n>` загружает фрагмент фрагмента (просмотр: 3) CI в CI
  Он был отправлен в Сан-Франциско.

Запись `fetch_report.json` (`chunk_retry_total` и `provider_failure_rate`)
Утверждения должны быть предоставлены CI в качестве доказательства.

## 5. Лучший выбор

Ниже приводится пример с чанкером:

1. Установите флажок `sorafs_manifest::chunker_registry_data`.
2. Установите `docs/source/sorafs/chunker_registry.md` на место.
3. Установите светильники (`export_vectors`) и установите их.
4. قدّم تقرير الامتثال للميثاق مع توقيعات الحوكمة.

С помощью этого приложения обрабатываются идентификаторы (`namespace.name@semver`) и идентификаторы идентификаторов.
Он сказал, что это не так.