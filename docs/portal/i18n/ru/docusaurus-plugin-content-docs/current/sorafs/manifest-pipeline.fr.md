---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Разбивка SoraFS → Конвейер манифеста

Это дополнение к быстрому старту, прослеживающее конвейер боя и бой, который преобразует октеты брютов.
в манифестах Norito, адаптированных к реестру контактов SoraFS. Содержание адаптировано
[`docs/source/sorafs/manifest_pipeline.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
Проконсультируйтесь с этим документом по канонической спецификации и журналу изменений.

## 1. Часть детерминированного поведения

SoraFS использует профиль SF-1 (`sorafs.sf1@1.0.0`): хеш-рулет, вдохновленный FastCDC с
минимальный фрагмент размером 64 КиБ, кабель размером 256 КиБ, максимальный размер 512 КиБ и маска
разрыв `0x0000ffff`. Профиль зарегистрирован в `sorafs_manifest::chunker_registry`.

### Помощники ржавчины

- `sorafs_car::CarBuildPlan::single_file` – Устранение смещений, длинных и напряженных участков BLAKE3
  Подвеска из кусков для подготовки метадонных автомобилей CAR.
- `sorafs_car::ChunkStore` – потоковая передача полезных данных, сохранение метадонных фрагментов и их получение.
  l'arbre d'échantillonnage Proof-of-Retrivability (PoR) 64 КиБ / 4 КиБ.
- `sorafs_chunker::chunk_bytes_with_digests` – Помощник библиотеки для двух интерфейсов командной строки.

### Интерфейс командной строки Outils

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON содержит ордонные смещения, длинные детали и фрагменты. Сохранять ле
план построения манифестов или спецификаций выборки для оркестратора.

### Темоинс PoR

`ChunkStore` раскрывает `--por-proof=<chunk>:<segment>:<leaf>` и `--por-sample=<count>` ближе к этим файлам
аудиторы могут требовать от ансамблей детерминистских тем. Ассоциация с флагами
`--por-proof-out` или `--por-sample-out` для регистрации JSON.

## 2. Конверт и манифест

`ManifestBuilder` объединяет метадонные фрагменты с элементами управления:

- CID racine (dag-cbor) и CAR.
- Preuves d'alias и declarations de capacité des fournisseurs.
- Подписи совета и метадонные опции (например, идентификаторы сборки).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Важные вылазки:

- `payload.manifest` – октеты манифеста, закодированные в Norito.
- `payload.report.json` – Резюме, доступное для людей/автоматизация, включая
  `chunk_fetch_specs`, `payload_digest_hex`, empreintes CAR и метадонники под псевдонимами.
- `payload.manifest_signatures.json` – Конверт, содержащий l'empreinte BLAKE3 du Manife,
  l'empreinte SHA3 трехплановых фрагментов и подписей Ed25519.

Используйте `--manifest-signatures-in` для проверки четырех конвертов по подписям.
вне зависимости от записи, и `--chunker-profile-id` или `--chunker-profile=<handle>` для
проверьте выбор регистрации.

## 3. Публикатор и специалист по закреплению1. **Soumission à la gouvernance** – Fournissez l'empreinte du manifete et l'enveloppe de
   подписи на совете, который поможет вам получить признание. Les Auditeurs Externes Doivent
   сохраняет пространство SHA3 плана фрагментов с пространством манифеста.
2. **Закрепление полезной нагрузки** – Загрузка архива CAR (и опция индекса CAR) по ссылке
   манифест против реестра контактов. Уверяем вас в том, что манифест и участие в CAR
   мем CID расин.
3. **Регистрация телеметрии** – Сохранение связи в формате JSON, темпов PoR и всех метрик.
   получить из выпущенных артефактов. Регистрация данных в информационных панелях
   операторы и помощники воспроизводят инциденты без объемной телезарядки полезной нагрузки.

## 4. Имитация выборки нескольких пользователей

`пробег груза -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` увеличивает параллелизм для четырехходовиков (`#4` ci-dessus).
- `@<weight>` отрегулируйте уклон украшения ; ценность по умолчанию равна 1.
- `--max-peers=<n>` ограничивает число четырехплановых планов для казни Лорска ла
  открыли отсылку плюс кандидаты, которые остались в живых.
- `--expect-payload-digest` и `--expect-payload-len` защищают от коррупции.
- `--provider-advert=name=advert.to` проверьте емкость оборудования перед использованием
  в симуляции.
- `--retry-budget=<n>` заменяет номер пробного фрагмента (по умолчанию: 3) в зависимости от того, что
  CI révèle плюс ускорение регресса в панических сценариях.

`fetch_report.json` раскрывает совокупные показатели (`chunk_retry_total`, `provider_failure_rate`,
и т. д.) Адаптированные к утверждениям CI и к наблюдаемости.

## 5. Мизес в день регистрации и управления

Лоры предложения новых профилей кусков:

1. Перечитайте описание в `sorafs_manifest::chunker_registry_data`.
2. Mettez à jour `docs/source/sorafs/chunker_registry.md` и ассоциированные компании.
3. Выполните настройку приборов (`export_vectors`) и захватите манифесты знаков.
4. Подготовить соглашение о соответствии Хартии с подписями органов управления.

L'automatisation doit Privilégier les handles canoniques (`namespace.name@semver`) и др.
восстановить числовые идентификаторы, которые необходимы для регистрации.