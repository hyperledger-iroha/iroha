---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Быстрое удаление SoraFS

Это практическое руководство, прошедшее в обзоре профиля детерминированного SF-1,
la подпись манифестов и поток восстановления энергии для нескольких поваров, которые
су-тендент ле конвейер де складе SoraFS. Полное соответствие
l'[анализ аппрофонди конвейера манифестов](manifest-pipeline.md)
для заметок о концепции и ссылок на флаги CLI.

## Предварительные требования

- Toolchain Rust (`rustup update`), локализация клона рабочей области.
- Опция: [пара ключей Ed25519, созданная для OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  для подписания манифестов.
- Вариант: Node.js ≥ 18, если вы предварительно выполнили предварительный просмотр порта Docusaurus.

Définissez `export RUST_LOG=info` подвеска для эссе для добавления сообщений с помощью утилит CLI.

## 1. Рафраичир детерминированные светильники

Обработайте канонические векторы декупажа SF-1. La Commande Produit Aussi Des
конверты манифеста подписаны lorsque `--signing-key` est fourni ; использовать
`--allow-unsigned` — уникальная локальная разработка.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Вылеты:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (подпись)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Декупирование полезной нагрузки и проверка плана

Используйте `sorafs_chunker` для копирования файлов или произвольного архива:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Клесные поля:

- `profile` / `break_mask` – подтверждение параметров `sorafs.sf1@1.0.0`.
- `chunks[]` – смещает ordonnés, longueurs и empreintes BLAKE3 кусков.

Для светильников и объемов, выполните регрессию на основе предположений в ближайшее время.
Уверяю вас в том, что декупаж будет воспроизведен в потоковом режиме и будет полностью синхронизирован:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Создайте и подпишите манифест

Конвертируйте план фрагментов, псевдонимы и подписи управления в ООН.
манифест через `sorafs-manifest-stub`. La Commande Ci-dessous иллюстрирует полезную нагрузку
Фишер уникальный; passez un chemin de répertoire pour empaqueter un arbre (la CLI le
паркур в лексикографическом порядке).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Проверьте `/tmp/docs.report.json`:

- `chunking.chunk_digest_sha3_256` – empreinte SHA3 des offsets/longueurs, соответствует aux.
  светильники du chunker.
- `manifest.manifest_blake3` – подпись BLAKE3 в конверте манифеста.
- `chunk_fetch_specs[]` – инструкции по восстановлению ордонезий для оркестраторов.

Когда вы готовы к использованию подписей, прочитайте аргументы
`--signing-key` и `--signer`. La Commande Verifie Chaque подпись Ed25519 avant
d'écrire l'конверт.

## 4. Simuler une recupération multi-fournisseurs

Используйте CLI для извлечения данных разработки, чтобы обновить план фрагментов,
плюсьеры четырениссеры. Это идеал для дымовых тестов CI и прототипов
оркестратор.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Проверки:- `payload_digest_hex` соответствует взаимопониманию манифеста.
- `provider_reports[]` раскрывает список успешных/проверенных результатов.
- `chunk_retry_total` не соответствует требованиям регулировки противодавления.
- Passez `--max-peers=<n>` для ограничения числа четырехплановых планировок для одного
  выполнение и контроль моделирования CI-центров для принципиальных кандидатов.
- `--retry-budget=<n>` заменить номер пробного фрагмента по умолчанию (3) в конце
  Mettre en évidence plus vite les rewards de l'orchestrateur lors de l'injection
  d'échecs.

Добавьте `--expect-payload-digest=<hex>` и `--expect-payload-len=<bytes>` для ответа
ускорение восстановления полезной нагрузки в карточке манифеста.

## 5. Следующие этапы

- **Интеграционное управление** – acheminer l'empreinte du манифест и др.
  `manifest_signatures.json` в потоке совета, который можно использовать в реестре контактов
  объявляет о возможности.
- **Переговоры о регистрации** – обратитесь к [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  перед регистрацией новых профилей. L'automatization doit Privilégier les handles
  канонические (`namespace.name@semver`) значения, которые соответствуют цифровым идентификаторам.
- **Автоматизация CI** – включает команды ci-dessus aux Pipelines de Release для того, чтобы
  документация, приспособления и опубликованные артефакты детерминированных манифестов
  с подписанными метадонами.