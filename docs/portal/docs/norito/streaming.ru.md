---
lang: ru
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 10ba7b91d73d6723c4b66491951c3257c48557273ab5424d81119e01c8f2c6e3
source_last_modified: "2025-12-09T06:48:00.858874+00:00"
translation_last_reviewed: 2025-12-30
---

# Norito Streaming

Norito Streaming задает формат на проводе, управляющие кадры и эталонный кодек, используемые для live медиа-потоков через Torii и SoraNet. Каноническая спецификация находится в `norito_streaming.md` в корне workspace; эта страница выделяет части, нужные операторам и авторам SDK, вместе с точками конфигурации.

## Формат на проводе и управляющая плоскость

- **Manifests и frames.** `ManifestV1` и `PrivacyRoute*` описывают таймлайн сегментов, описатели чанков и подсказки маршрутов. Управляющие кадры (`KeyUpdate`, `ContentKeyUpdate` и feedback каденса) живут рядом с manifest, чтобы зрители могли проверить commitments до декодирования.
- **Базовый кодек.** `BaselineEncoder`/`BaselineDecoder` обеспечивают монотонные id чанков, арифметику таймстемпов и проверку commitments. Хосты должны вызывать `EncodedSegment::verify_manifest` перед тем как отдавать зрителям или релеям.
- **Биты features.** Согласование возможностей объявляет `streaming.feature_bits` (дефолт `0b11` = baseline feedback + provider маршрута приватности), чтобы реле и клиенты детерминированно отклоняли несовместимые peer.

## Ключи, сьюты и каденс

- **Требования к идентичности.** Контрольные кадры стриминга всегда подписываются Ed25519. Выделенные ключи можно задать через `streaming.identity_public_key`/`streaming.identity_private_key`; иначе повторно используется идентичность узла.
- **HPKE сьюты.** `KeyUpdate` выбирает наименьшую общую сьюту; сьюта #1 обязательна (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`), с опциональным путем апгрейда на `Kyber1024`. Выбор сьюты хранится в сессии и проверяется при каждом обновлении.
- **Ротация.** Паблишеры выпускают подписанный `KeyUpdate` каждые 64 MiB или 5 минут. `key_counter` должен строго расти; регрессия - критическая ошибка. `ContentKeyUpdate` распределяет вращающийся Group Content Key, обернутый в согласованную HPKE сьюту, и ограничивает расшифровку сегментов по ID + окну валидности.
- **Снапшоты.** `StreamingSession::snapshot_state` и `restore_from_snapshot` сохраняют `{session_id, key_counter, suite, sts_root, cadence state}` в `streaming.session_store_dir` (по умолчанию `./storage/streaming`). Транспортные ключи пересчитываются при восстановлении, чтобы сбои не раскрывали секреты сессии.

## Конфигурация рантайма

- **Ключевой материал.** Задайте выделенные ключи через `streaming.identity_public_key`/`streaming.identity_private_key` (Ed25519 multihash) и опциональный материал Kyber через `streaming.kyber_public_key`/`streaming.kyber_secret_key`. При переопределении дефолтов должны присутствовать все четыре; `streaming.kyber_suite` принимает `mlkem512|mlkem768|mlkem1024` (алиасы `kyber512/768/1024`, по умолчанию `mlkem768`).
- **Маршруты SoraNet.** `streaming.soranet.*` управляет анонимным транспортом: `exit_multiaddr` (дефолт `/dns/torii/udp/9443/quic`), `padding_budget_ms` (дефолт 25 ms), `access_kind` (`authenticated` vs `read-only`), опциональный `channel_salt`, `provision_spool_dir` (дефолт `./storage/streaming/soranet_routes`), `provision_spool_max_bytes` (дефолт 0, без лимита), `provision_window_segments` (дефолт 4) и `provision_queue_capacity` (дефолт 256).
- **Sync gate.** `streaming.sync` включает контроль дрейфа для аудиовизуальных потоков: `enabled`, `observe_only`, `ewma_threshold_ms` и `hard_cap_ms` определяют, когда сегменты отклоняются из-за таймингового дрейфа.

## Валидация и фикстуры

- Канонические определения типов и хелперы находятся в `crates/iroha_crypto/src/streaming.rs`.
- Интеграционное покрытие проверяет HPKE handshake, распределение content-key и жизненный цикл снапшотов (`crates/iroha_crypto/tests/streaming_handshake.rs`). Запустите `cargo test -p iroha_crypto streaming_handshake`, чтобы проверить стриминговую поверхность локально.
- Для глубокой проработки layout, обработки ошибок и будущих апгрейдов прочитайте `norito_streaming.md` в корне репозитория.
