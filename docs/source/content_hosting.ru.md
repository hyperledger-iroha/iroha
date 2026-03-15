---
lang: ru
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T17:57:58.226177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% полосы хостинга контента
% Iroha Ядро

# Линия хостинга контента

Линия контента хранит в цепочке небольшие статические пакеты (tar-архивы) и обслуживает
отдельные файлы непосредственно из Torii.

- **Публикация**: отправьте `PublishContentBundle` с архивом tar, срок действия не обязателен.
  height и необязательный манифест. Идентификатор пакета — это хэш blake2b
  тарбол. Записи Tar должны быть обычными файлами; имена представляют собой нормализованные пути UTF-8.
  Ограничения размера/пути/количества файлов берутся из конфигурации `content` (`max_bundle_bytes`,
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`).
  Манифесты включают хеш индекса Norito, пространство данных/полосу, политику кэширования.
  (`max_age_seconds`, `immutable`), режим аутентификации (`public`/`role:<role>`/
  `sponsor:<uaid>`), заполнитель политики хранения и переопределения MIME.
- **Дедупликация**: полезные данные tar разбиваются на фрагменты (по умолчанию 64 КБ) и сохраняются один раз для каждого файла.
  хэш со счетчиком ссылок; удаление пакета уменьшает и сокращает куски.
- **Служба**: Torii предоставляет доступ к `GET /v2/content/{bundle}/{path}`. Поток ответов
  непосредственно из хранилища фрагментов с помощью `ETag` = хэш файла, `Accept-Ranges: bytes`,
  Поддержка диапазона и Cache-Control, полученный из манифеста. Читает в честь
  режим манифестной аутентификации: для ролевых и спонсорских ответов требуется канонический
  заголовки запроса (`X-Iroha-Account`, `X-Iroha-Signature`) для подписанных
  счет; отсутствующие/просроченные пакеты возвращают 404.
- **CLI**: `iroha content publish --bundle <path.tar>` (или `--root <dir>`) сейчас.
  автоматически генерирует манифест, выдает необязательный `--manifest-out/--bundle-out` и
  принимает `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  и `--expires-at-height` переопределяет. Сборки `iroha content pack --root <dir>`
  детерминированный архив + манифест без отправки чего-либо.
- **Конфигурация**: ручки кэша/авторизации находятся под `content.*` в `iroha_config`.
  (И18НИ00000038Х, И18НИ00000039Х, И18НИ00000040Х,
  `default_auth_mode`) и применяются во время публикации.
- **SLO + пределы**: `content.max_requests_per_second` / `request_burst` и
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` крышка со стороны чтения
  пропускная способность; Torii применяет как перед обработкой байтов, так и перед экспортом.
  `torii_content_requests_total`, `torii_content_request_duration_seconds` и
  Метрики `torii_content_response_bytes_total` с метками результатов. Задержка
  цели живут под `content.target_p50_latency_ms` /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **Контроль злоупотреблений**: сегменты ставок кодируются токеном UAID/API/удаленным IP-адресом, а также
  дополнительная защита PoW (`content.pow_difficulty_bits`, `content.pow_header`) может
  быть обязательным перед чтением. Значения по умолчанию для макета полосы DA взяты из
  `content.stripe_layout` и отображаются в хэшах квитанций/манифестов.
- **Квитанции и доказательства DA**: успешные ответы прилагаются.
  `sora-content-receipt` (байты base64 Norito в кадре `ContentDaReceipt`), несущие
  `bundle_id`, `path`, `file_hash`, `served_bytes`, диапазон обслуживаемых байтов,
  `chunk_root` / `stripe_layout`, необязательное обязательство PDP и временная метка, поэтому
  клиенты могут закрепить полученную информацию, не перечитывая тело.

Ключевые ссылки:- Модель данных: `crates/iroha_data_model/src/content.rs`
- Исполнение: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Обработчик Torii: `crates/iroha_torii/src/content.rs`
- Помощник CLI: `crates/iroha_cli/src/content.rs`