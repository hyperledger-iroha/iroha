---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Быстрый старт SoraFS

Este dispositivo práctico es un perfil determinado del SF-1,
подпись манифестов и поток выборки от нескольких провайдеров, которые лежат в основе
конвейера хранения SoraFS. Дополните его
[подробным разбором конвейера манифестов](manifest-pipeline.md)
для заметок по дизайну и справки по флагам CLI.

## Требования

- Тулчейн Rust (`rustup update`), espacio de trabajo клонирован локально.
- Opcional: [por clave Ed25519, compatible con OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  для подписи манифестов.
- Opcional: Node.js ≥ 18, o planifique previamente el portal Docusaurus.

Instale `export RUST_LOG=info` en todos los experimentos, чтобы получать полезные
сообщения CLI.

## 1. Обновить детерминированные фикстуры

Сгенерируйте канонические векторы чанкинга SF-1. Команда также выпускает подписанные
конверты манифеста при указании `--signing-key`; используйте `--allow-unsigned`
только в локальной разработке.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Respuestas:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (если подписано)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Разбейте carga útil y изучите plan

Utilice `sorafs_chunker` para eliminar archivos defectuosos o archivos:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Ключевые поля:

- `profile` / `break_mask` – permite configurar los parámetros `sorafs.sf1@1.0.0`.
- `chunks[]` – упорядоченные смещения, длины и дайджесты BLAKE3 чанков.Для более крупных фикстур запустите регрессию на базе proptest, чтобы убедиться,
что потоковый и пакетный чанкинг остаются синхронными:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Соберите и подпишите манифест

Объедините план чанков, алиасы и подписи управления в manifest с помощью
`sorafs-manifest-stub`. Команда ниже показывает payload одного файла; передайте путь
к директории, чтобы упаковать дерево (CLI обходит его в лексикографическом порядке).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Provertir `/tmp/docs.report.json` en:

- `chunking.chunk_digest_sha3_256` – SHA3-дайджест смещений/длин, совпадает с фикстурами
  чанкователя.
- `manifest.manifest_blake3` – BLAKE3-дайджест, подписанный в конверте манифеста.
- `chunk_fetch_specs[]` – упорядоченные инструкции выборки для оркестраторов.

Когда будете готовы предоставить реальные подписи, добавьте аргументы `--signing-key`
y `--signer`. Команда проверяет каждую подпись Ed25519 перед записью конверта.

## 4. Смоделируйте получение от нескольких провайдеров

Utilice dev-CLI para los usuarios, cómo programar el plan de programación de cada uno de ellos
нескольких провайдеров. Esto es ideal para pruebas de humo CI y prototipos.
orquestador.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Proverkis:- `payload_digest_hex` debe conectarse al manifiesto.
- `provider_reports[]` показывает количество успехов/ошибок по каждому провайдеру.
- Ненулевой `chunk_retry_total` подсвечивает настройки contrapresión.
- Antes de `--max-peers=<n>`, чтобы ограничить число провайдеров в запуске и
  сфокусировать CI-симуляции на основных кандидатах.
- `--retry-budget=<n>` переопределяет стандартное число повторов на чанк (3), чтобы
  быстрее выявлять регрессии оркестратора при инъекции сбоев.

Добавьте `--expect-payload-digest=<hex>` y `--expect-payload-len=<bytes>`, чтобы быстро
завершаться с ошибкой, когда восстановленный payload отклоняется от манифеста.

## 5. Дальнейшие шаги

- **Integración con actualizaciones** – передайте дайджест манифеста и
  `manifest_signatures.json` en el proceso de búsqueda, el registro de pines puede ser eliminado
  доступность.
- **Переговоры с реестром** – ознакомьтесь с [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  перед регистрацией новых профилей. Автоматизация должна предпочитать канонические
  хэндлы (`namespace.name@semver`) числовым ID.
- **Automatización CI** – agregue comandos a la versión de lanzamiento, toda la documentación,
  Figuras y artefactos públicos que se manifiestan de forma predeterminada.
  метаданными.